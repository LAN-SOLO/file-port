//! Verbindungs-Profile: Metadaten als JSON im App-Config-Ordner,
//! Geheimnisse (Passwörter, Secret Keys, Passphrasen) ausschließlich im
//! OS-Schlüsselbund (Keychain / Credential Manager / Secret Service).

use std::path::PathBuf;
use std::sync::Arc;

use fileport_core::{
    host_key_fingerprint, probe_host_key, AzureConfig, Backend, FpError, FtpBackend, FtpConfig,
    FtpSecurity, GcsConfig, ObjectBackend, RsyncBackend, S3Config, ScpBackend, SftpBackend,
    SmbBackend, SmbConfig, SshAuth, SshConfig, WebdavBackend, WebdavConfig,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::engine::Engine;

const KEYRING_SERVICE: &str = "com.lan-solo.fileport";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Sftp,
    Scp,
    Rsync,
    /// Nur noch für alte Profile: verbindet seit 0.2.8 immer per AUTH TLS,
    /// unverschlüsseltes FTP gibt es nicht mehr.
    Ftp,
    /// Unverschlüsseltes FTP — verbindet nur mit `insecure_confirmed`.
    /// Eigener Wert statt `ftp`, damit alte Profile nie stillschweigend
    /// von TLS auf Klartext fallen.
    #[serde(rename = "ftp_plain")]
    FtpPlain,
    Ftps,
    #[serde(rename = "ftps_implicit")]
    FtpsImplicit,
    Webdav,
    Smb,
    S3,
    Azure,
    Gcs,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// Leer bei neuen Profilen — wird beim Speichern vergeben.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub host: String,
    #[serde(default)]
    pub port: u16,
    #[serde(default)]
    pub user: String,
    /// SFTP/SCP/rsync: Pfad zur SSH-Schlüsseldatei (statt Passwort);
    /// GCS: Pfad zur Service-Account-JSON.
    #[serde(default)]
    pub key_file: String,
    /// SFTP/SCP/rsync: gemerkter Host-Key (Trust-on-first-use).
    #[serde(default)]
    pub host_key: String,
    /// SMB: Name der Freigabe (Share).
    #[serde(default)]
    pub share: String,
    /// WebDAV: Basis-URL der DAV-Wurzel.
    #[serde(default)]
    pub base_url: String,
    /// Azure: Name des Storage-Kontos.
    #[serde(default)]
    pub account: String,
    /// S3: Endpoint (leer für AWS), Region, Bucket, Access Key, Path-Style.
    /// `bucket` dient bei Azure als Container, bei GCS als Bucket.
    #[serde(default)]
    pub endpoint: String,
    #[serde(default)]
    pub region: String,
    #[serde(default)]
    pub bucket: String,
    #[serde(default)]
    pub access_key: String,
    #[serde(default)]
    pub path_style: bool,
    /// FTPS/WebDAV/S3: selbstsignierte Zertifikate akzeptieren.
    #[serde(default)]
    pub accept_invalid_certs: bool,
    /// FTP (unverschlüsselt): Nutzerin hat die Warnung gelesen und
    /// bestätigt, dass sie die Verbindung trotzdem aufbauen will.
    #[serde(default)]
    pub insecure_confirmed: bool,
}

const INSECURE_NOT_CONFIRMED: &str =
    "Unverschlüsseltes FTP nicht bestätigt — im Profil das Risiko bestätigen oder FTPS verwenden";

fn profiles_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("profiles.json"))
}

fn load_profiles(app: &tauri::AppHandle) -> Result<Vec<Profile>, String> {
    let path = profiles_path(app)?;
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| fileport_core::io_text(&e))?;
    // Von Hand in Windows-Notepad bearbeitet → UTF-8-BOM tolerieren.
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    serde_json::from_str(raw).map_err(|e| format!("profiles.json: {e}"))
}

fn store_profiles(app: &tauri::AppHandle, profiles: &[Profile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    let raw = serde_json::to_string_pretty(profiles).map_err(|e| e.to_string())?;
    // Atomar schreiben: erst Temp-Datei, dann umbenennen — ein Absturz mitten
    // im Schreiben hinterlässt nie eine halbe profiles.json.
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, raw).map_err(|e| fileport_core::io_text(&e))?;
    // Windows: Virenscanner/Backup-Tools halten die Datei gern kurz offen.
    fileport_core::rename_with_retry_blocking(&tmp, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        fileport_core::io_text(&e)
    })
}

/// Serialisiert alle Lese-Ändern-Schreiben-Zyklen auf profiles.json —
/// sonst schriebe z. B. ein laufendes `connect` (merkt sich den Host-Key)
/// ein inzwischen gelöschtes Profil mit seiner alten Liste zurück.
static PROFILES_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

impl Protocol {
    fn is_ssh(&self) -> bool {
        matches!(self, Protocol::Sftp | Protocol::Scp | Protocol::Rsync)
    }

    /// Protokolle mit Host-/Port-Feldern (alles außer WebDAV und Clouds).
    fn uses_host(&self) -> bool {
        !matches!(
            self,
            Protocol::Webdav | Protocol::S3 | Protocol::Azure | Protocol::Gcs
        )
    }

    fn default_port(&self) -> u16 {
        match self {
            Protocol::Sftp | Protocol::Scp | Protocol::Rsync => 22,
            Protocol::FtpsImplicit => 990,
            Protocol::Ftp | Protocol::FtpPlain | Protocol::Ftps => 21,
            Protocol::Smb => 445,
            _ => 0,
        }
    }
}

/// `~` bzw. `~/…` (auch `~\…` unter Windows) auf das Home-Verzeichnis
/// abbilden — das Formular schlägt `~/.ssh/id_ed25519` vor.
pub(crate) fn expand_home(path: &str) -> PathBuf {
    let trimmed = path.trim();
    let rest = if trimmed == "~" {
        Some("")
    } else {
        trimmed
            .strip_prefix("~/")
            .or_else(|| trimmed.strip_prefix("~\\"))
    };
    match (rest, home_dir()) {
        (Some(""), Some(home)) => home,
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(trimmed),
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// Host-Feld prüfen: nicht leer, keine Leerzeichen/Steuerzeichen und kein
/// führendes `-` (würde beim gespawnten ssh/rsync als Option gelesen).
pub(crate) fn validate_host(host: &str) -> Result<(), String> {
    let h = host.trim();
    if h.is_empty() {
        return Err("Host fehlt".into());
    }
    if h.starts_with('-') || h.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(format!("Ungültiger Host: {h}"));
    }
    Ok(())
}

/// Benutzername für SSH/rsync: kein führendes `-`, keine Leer-/Steuerzeichen,
/// kein `@`/`:` (würde die rsync-Zieladresse verfälschen).
fn validate_ssh_user(user: &str) -> Result<(), String> {
    if user.is_empty() {
        return Err("Benutzer fehlt".into());
    }
    if user.starts_with('-')
        || user
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || c == '@' || c == ':')
    {
        return Err(format!("Ungültiger Benutzername: {user}"));
    }
    Ok(())
}

/// Geheimnis eines Profils im Schlüsselbund ablegen/lesen/löschen.
/// keyring blockiert — deshalb immer über `spawn_blocking` aufrufen.
fn secret_entry(profile_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, profile_id).map_err(|e| e.to_string())
}

async fn secret_set(profile_id: String, secret: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || {
        secret_entry(&profile_id)?
            .set_password(&secret)
            .map_err(|e| format!("Schlüsselbund: {e}"))
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn secret_get(profile_id: String) -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(move || match secret_entry(&profile_id)?.get_password() {
        Ok(secret) => Ok(Some(secret)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Schlüsselbund: {e}")),
    })
    .await
    .map_err(|e| e.to_string())?
}

async fn secret_delete(profile_id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || match secret_entry(&profile_id)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("Schlüsselbund: {e}")),
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn profiles_list(app: tauri::AppHandle) -> Result<Vec<Profile>, String> {
    let _guard = PROFILES_LOCK.lock().await;
    load_profiles(&app)
}

/// Legt ein Profil an oder aktualisiert es; `secret` (Passwort/Secret Key/
/// Passphrase) wandert in den Schlüsselbund, nie in die JSON-Datei.
/// `clear_secret` entfernt ein gespeichertes Geheimnis (z. B. beim Wechsel
/// von Passwort auf Schlüsseldatei ohne Passphrase).
///
/// Der gemerkte Host-Key gehört dem Backend: Die UI kann ihn weder setzen
/// noch überschreiben. Ändern sich Host, Port oder Protokoll, wird er
/// verworfen (neuer Server → neue Bestätigung).
#[tauri::command]
pub async fn profile_save(
    app: tauri::AppHandle,
    mut profile: Profile,
    secret: Option<String>,
    clear_secret: Option<bool>,
) -> Result<Profile, String> {
    let _guard = PROFILES_LOCK.lock().await;
    let mut profiles = load_profiles(&app)?;
    profile.name = profile.name.trim().to_string();
    if profile.name.is_empty() {
        return Err("Name fehlt".into());
    }
    profile.host = profile.host.trim().to_string();
    if profile.protocol.uses_host() && !profile.host.is_empty() {
        validate_host(&profile.host)?;
    }
    if profile.protocol == Protocol::FtpPlain && !profile.insecure_confirmed {
        return Err(INSECURE_NOT_CONFIRMED.into());
    }
    if profile.id.is_empty() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let mut id = format!("p{millis}");
        while profiles.iter().any(|p| p.id == id) {
            id.push('x');
        }
        profile.id = id;
        profile.host_key = String::new();
    } else {
        profile.host_key = match profiles.iter().find(|p| p.id == profile.id) {
            Some(old)
                if old.host == profile.host
                    && old.port == profile.port
                    && old.protocol.is_ssh() == profile.protocol.is_ssh() =>
            {
                old.host_key.clone()
            }
            _ => String::new(),
        };
    }
    if let Some(secret) = secret.filter(|s| !s.is_empty()) {
        secret_set(profile.id.clone(), secret).await?;
    } else if clear_secret.unwrap_or(false) {
        secret_delete(profile.id.clone()).await?;
    }
    match profiles.iter_mut().find(|p| p.id == profile.id) {
        Some(slot) => *slot = profile.clone(),
        None => profiles.push(profile.clone()),
    }
    store_profiles(&app, &profiles)?;
    Ok(profile)
}

#[tauri::command]
pub async fn profile_delete(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = PROFILES_LOCK.lock().await;
    let mut profiles = load_profiles(&app)?;
    profiles.retain(|p| p.id != id);
    store_profiles(&app, &profiles)?;
    // Fehler beim Entfernen aus dem Schlüsselbund melden statt verschlucken —
    // das Profil ist dann weg, das Geheimnis aber noch da.
    secret_delete(id).await
}

/// Gemerkten Host-Key eines Profils verwerfen — die nächste Verbindung
/// fragt wieder nach (z. B. nach einer Server-Neuinstallation).
#[tauri::command]
pub async fn profile_reset_host_key(app: tauri::AppHandle, id: String) -> Result<(), String> {
    let _guard = PROFILES_LOCK.lock().await;
    let mut profiles = load_profiles(&app)?;
    let profile = profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("Profil nicht gefunden")?;
    profile.host_key.clear();
    store_profiles(&app, &profiles)
}

/// Rückfrage zu einem SSH-Host-Key, bevor Zugangsdaten an den Server gehen.
#[derive(Serialize)]
pub struct HostKeyPrompt {
    /// SHA256-Fingerprint, wie ihn `ssh-keygen -lf` zeigt.
    pub fingerprint: String,
    /// Der Key selbst (OpenSSH-Format) — die UI reicht ihn bei Bestätigung
    /// als `trust_host_key` zurück.
    pub key: String,
    /// true: Der Server zeigt einen ANDEREN Key als den gespeicherten.
    pub changed: bool,
    pub old_fingerprint: Option<String>,
}

/// Ergebnis von `connect`: entweder eine offene Verbindung (`conn`) oder
/// eine Host-Key-Rückfrage (`host_key`), die die UI bestätigen muss.
#[derive(Serialize)]
pub struct ConnectResult {
    pub conn: Option<u32>,
    pub label: String,
    pub host_key: Option<HostKeyPrompt>,
}

/// SSH-Verbindungsdaten (SFTP/SCP/rsync) aus einem Profil bauen —
/// Schlüsseldatei vor Passwort, Secret ist dann die Passphrase.
fn ssh_config(profile: &Profile, secret: String, expected: String) -> SshConfig {
    let auth = if profile.key_file.trim().is_empty() {
        SshAuth::Password(secret)
    } else {
        SshAuth::KeyFile {
            path: expand_home(&profile.key_file),
            passphrase: (!secret.is_empty()).then_some(secret),
        }
    };
    SshConfig {
        host: profile.host.clone(),
        port: if profile.port == 0 { 22 } else { profile.port },
        user: profile.user.clone(),
        auth,
        expected_host_key: Some(expected),
    }
}

fn fingerprint(key: &str) -> String {
    host_key_fingerprint(key).unwrap_or_else(|| key.to_string())
}

/// Baut die Verbindung zu einem gespeicherten Profil auf.
///
/// SFTP, SCP und rsync: Ist noch kein Host-Key gemerkt, holt fileport nur
/// den Key (ohne Anmeldung!) und gibt ihn als Rückfrage zurück; erst mit
/// `trust_host_key` (dem bestätigten Key) wird verbunden und der Key im
/// Profil gespeichert. Zeigt der Server später einen anderen Key, kommt
/// ebenfalls eine Rückfrage (`changed = true`) statt einer Verbindung.
#[tauri::command]
pub async fn connect(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    id: String,
    trust_host_key: Option<String>,
) -> Result<ConnectResult, String> {
    // Nur kurz unter der Sperre lesen — der Verbindungsaufbau selbst darf
    // Speichern/Löschen anderer Profile nicht blockieren.
    let mut profile = {
        let _guard = PROFILES_LOCK.lock().await;
        load_profiles(&app)?
            .into_iter()
            .find(|p| p.id == id)
            .ok_or("Profil nicht gefunden")?
    };
    let profile = &mut profile;
    let trust = trust_host_key
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty());

    if profile.protocol.is_ssh() {
        validate_host(&profile.host)?;
        validate_ssh_user(&profile.user)?;
    } else if profile.protocol.uses_host() {
        validate_host(&profile.host)?;
    }

    // SSH: unbekannter Server → erst Key zeigen, dann (nach Bestätigung) anmelden.
    let mut ssh_expected = String::new();
    if profile.protocol.is_ssh() {
        ssh_expected = match (&trust, profile.host_key.is_empty()) {
            (Some(k), _) => k.clone(),
            (None, false) => profile.host_key.clone(),
            (None, true) => {
                let port = if profile.port == 0 {
                    profile.protocol.default_port()
                } else {
                    profile.port
                };
                let key = probe_host_key(&profile.host, port)
                    .await
                    .map_err(|e| e.to_string())?;
                return Ok(ConnectResult {
                    conn: None,
                    label: String::new(),
                    host_key: Some(HostKeyPrompt {
                        fingerprint: fingerprint(&key),
                        key,
                        changed: false,
                        old_fingerprint: None,
                    }),
                });
            }
        };
    }

    let secret = secret_get(profile.id.clone()).await?.unwrap_or_default();
    let ssh_result = |e: FpError, profile: &Profile| -> Result<ConnectResult, String> {
        match e {
            FpError::HostKeyMismatch { seen, .. } if !seen.is_empty() => Ok(ConnectResult {
                conn: None,
                label: String::new(),
                host_key: Some(HostKeyPrompt {
                    fingerprint: fingerprint(&seen),
                    key: seen,
                    changed: true,
                    old_fingerprint: (!profile.host_key.is_empty())
                        .then(|| fingerprint(&profile.host_key)),
                }),
            }),
            other => Err(other.to_string()),
        }
    };

    let backend: Arc<dyn Backend> = match profile.protocol {
        Protocol::Sftp | Protocol::Scp | Protocol::Rsync => {
            let cfg = ssh_config(profile, secret, ssh_expected);
            let connected: Result<(Arc<dyn Backend>, String), FpError> = match profile.protocol {
                Protocol::Sftp => SftpBackend::connect(cfg).await.map(|be| {
                    let key = be.host_key().to_string();
                    (Arc::new(be) as Arc<dyn Backend>, key)
                }),
                Protocol::Scp => ScpBackend::connect(cfg).await.map(|be| {
                    let key = be.host_key().to_string();
                    (Arc::new(be) as Arc<dyn Backend>, key)
                }),
                _ => RsyncBackend::connect(cfg).await.map(|be| {
                    let key = be.host_key().to_string();
                    (Arc::new(be) as Arc<dyn Backend>, key)
                }),
            };
            match connected {
                Ok((be, key)) => {
                    if !key.is_empty() && key.trim() != profile.host_key.trim() {
                        // Frisch einlesen und nur den Host-Key setzen: wurde das
                        // Profil inzwischen gelöscht, bleibt es gelöscht.
                        let _guard = PROFILES_LOCK.lock().await;
                        let mut current = load_profiles(&app)?;
                        if let Some(p) = current.iter_mut().find(|p| p.id == profile.id) {
                            p.host_key = key.clone();
                            store_profiles(&app, &current)?;
                        }
                        profile.host_key = key;
                    }
                    be
                }
                Err(e) => return ssh_result(e, profile),
            }
        }
        Protocol::Smb => Arc::new(
            SmbBackend::connect(SmbConfig {
                host: profile.host.clone(),
                port: profile.port,
                share: profile.share.clone(),
                user: profile.user.clone(),
                password: secret,
            })
            .await
            .map_err(|e| e.to_string())?,
        ),
        // Alte „FTP"-Profile (vor `ftp_plain`) werden weiter auf AUTH TLS
        // gehoben; Klartext nur mit eigenem Protokoll + Bestätigung.
        Protocol::Ftp | Protocol::FtpPlain | Protocol::Ftps | Protocol::FtpsImplicit => {
            let implicit = profile.protocol == Protocol::FtpsImplicit;
            let security = match profile.protocol {
                Protocol::FtpsImplicit => FtpSecurity::ImplicitTls,
                Protocol::FtpPlain if profile.insecure_confirmed => FtpSecurity::Plain,
                Protocol::FtpPlain => return Err(INSECURE_NOT_CONFIRMED.into()),
                _ => FtpSecurity::ExplicitTls,
            };
            let default_port = if implicit { 990 } else { 21 };
            Arc::new(
                FtpBackend::connect(FtpConfig {
                    host: profile.host.clone(),
                    port: if profile.port == 0 { default_port } else { profile.port },
                    user: profile.user.clone(),
                    password: secret,
                    security,
                    accept_invalid_certs: profile.accept_invalid_certs,
                })
                .await
                .map_err(|e| e.to_string())?,
            )
        }
        Protocol::Webdav => {
            // Nur verschlüsselte Verbindungen: unverschlüsseltes WebDAV ablehnen.
            if !profile.base_url.starts_with("https://") {
                return Err(
                    "Nur https://-URLs — unverschlüsseltes WebDAV wird nicht unterstützt".into(),
                );
            }
            Arc::new(
                WebdavBackend::connect(WebdavConfig {
                    base_url: profile.base_url.clone(),
                    user: profile.user.clone(),
                    password: secret,
                    accept_invalid_certs: profile.accept_invalid_certs,
                })
                .await
                .map_err(|e| e.to_string())?,
            )
        }
        Protocol::S3 => {
            // Eigene Endpoints nur über TLS — Klartext-HTTP ablehnen.
            if !profile.endpoint.is_empty() && !profile.endpoint.starts_with("https://") {
                return Err(
                    "Nur https://-Endpoints — unverschlüsselte Verbindungen sind deaktiviert"
                        .into(),
                );
            }
            Arc::new(
                ObjectBackend::connect_s3(S3Config {
                    endpoint: (!profile.endpoint.is_empty()).then(|| profile.endpoint.clone()),
                    region: if profile.region.is_empty() {
                        "us-east-1".to_string()
                    } else {
                        profile.region.clone()
                    },
                    bucket: profile.bucket.clone(),
                    access_key: profile.access_key.clone(),
                    secret_key: secret,
                    path_style: profile.path_style,
                    accept_invalid_certs: profile.accept_invalid_certs,
                })
                .await
                .map_err(|e| e.to_string())?,
            )
        }
        Protocol::Azure => Arc::new(
            ObjectBackend::connect_azure(AzureConfig {
                account: profile.account.clone(),
                container: profile.bucket.clone(),
                access_key: secret,
            })
            .await
            .map_err(|e| e.to_string())?,
        ),
        Protocol::Gcs => Arc::new(
            ObjectBackend::connect_gcs(GcsConfig {
                bucket: profile.bucket.clone(),
                key_file: expand_home(&profile.key_file).to_string_lossy().into_owned(),
            })
            .await
            .map_err(|e| e.to_string())?,
        ),
    };

    let label = backend.label();
    let conn = engine.insert(backend).await;
    Ok(ConnectResult {
        conn: Some(conn),
        label,
        host_key: None,
    })
}

#[tauri::command]
pub async fn disconnect(engine: tauri::State<'_, Engine>, conn: u32) -> Result<(), String> {
    engine.remove(conn).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tilde_expands_to_home() {
        let home = home_dir().expect("HOME/USERPROFILE gesetzt");
        assert_eq!(expand_home("~/.ssh/id_ed25519"), home.join(".ssh/id_ed25519"));
        assert_eq!(expand_home("~\\.ssh\\id"), home.join(".ssh\\id"));
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home(" /abs/key "), PathBuf::from("/abs/key"));
        assert_eq!(expand_home("~bob/key"), PathBuf::from("~bob/key"));
    }

    #[test]
    fn host_and_user_validation() {
        assert!(validate_host("example.com").is_ok());
        assert!(validate_host("::1").is_ok());
        assert!(validate_host("").is_err());
        assert!(validate_host("-oProxyCommand=evil").is_err());
        assert!(validate_host("a b").is_err());
        assert!(validate_ssh_user("anna").is_ok());
        assert!(validate_ssh_user("DOM\\anna").is_ok());
        assert!(validate_ssh_user("-x").is_err());
        assert!(validate_ssh_user("a@b").is_err());
        assert!(validate_ssh_user("").is_err());
    }

    #[test]
    fn old_ftp_profiles_still_parse() {
        let raw = r#"[{"id":"p1","name":"alt","protocol":"ftp","host":"h"}]"#;
        let p: Vec<Profile> = serde_json::from_str(raw).unwrap();
        assert_eq!(p[0].protocol, Protocol::Ftp);
        assert_eq!(p[0].protocol.default_port(), 21);
        assert!(p[0].host_key.is_empty());
        assert!(!p[0].insecure_confirmed);
    }

    #[test]
    fn plain_ftp_profiles_parse() {
        let raw = r#"[{"id":"p1","name":"klar","protocol":"ftp_plain","host":"h","insecure_confirmed":true}]"#;
        let p: Vec<Profile> = serde_json::from_str(raw).unwrap();
        assert_eq!(p[0].protocol, Protocol::FtpPlain);
        assert_eq!(p[0].protocol.default_port(), 21);
        assert!(p[0].insecure_confirmed);
    }
}
