//! Verbindungs-Profile: Metadaten als JSON im App-Config-Ordner,
//! Geheimnisse (Passwörter, Secret Keys, Passphrasen) ausschließlich im
//! OS-Schlüsselbund (Keychain / Credential Manager / Secret Service).

use std::path::PathBuf;
use std::sync::Arc;

use fileport_core::{
    AzureConfig, Backend, FtpBackend, FtpConfig, FtpSecurity, GcsConfig, ObjectBackend, S3Config,
    SftpAuth, SftpBackend, SftpConfig, WebdavBackend, WebdavConfig,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::engine::Engine;

const KEYRING_SERVICE: &str = "com.lan-solo.fileport";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Sftp,
    /// Nur noch für alte Profile: verbindet seit 0.2.8 immer per AUTH TLS,
    /// unverschlüsseltes FTP gibt es nicht mehr.
    Ftp,
    Ftps,
    #[serde(rename = "ftps_implicit")]
    FtpsImplicit,
    Webdav,
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
    /// SFTP: Pfad zur Schlüsseldatei (statt Passwort).
    #[serde(default)]
    pub key_file: String,
    /// SFTP: gemerkter Host-Key (Trust-on-first-use).
    #[serde(default)]
    pub host_key: String,
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
}

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
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| format!("profiles.json: {e}"))
}

fn store_profiles(app: &tauri::AppHandle, profiles: &[Profile]) -> Result<(), String> {
    let path = profiles_path(app)?;
    let raw = serde_json::to_string_pretty(profiles).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
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

async fn secret_delete(profile_id: String) {
    let _ = tokio::task::spawn_blocking(move || {
        if let Ok(entry) = secret_entry(&profile_id) {
            let _ = entry.delete_credential();
        }
    })
    .await;
}

#[tauri::command]
pub async fn profiles_list(app: tauri::AppHandle) -> Result<Vec<Profile>, String> {
    load_profiles(&app)
}

/// Legt ein Profil an oder aktualisiert es; `secret` (Passwort/Secret Key/
/// Passphrase) wandert in den Schlüsselbund, nie in die JSON-Datei.
#[tauri::command]
pub async fn profile_save(
    app: tauri::AppHandle,
    mut profile: Profile,
    secret: Option<String>,
) -> Result<Profile, String> {
    let mut profiles = load_profiles(&app)?;
    if profile.id.is_empty() {
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        profile.id = format!("p{millis}");
    }
    if let Some(secret) = secret.filter(|s| !s.is_empty()) {
        secret_set(profile.id.clone(), secret).await?;
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
    let mut profiles = load_profiles(&app)?;
    profiles.retain(|p| p.id != id);
    store_profiles(&app, &profiles)?;
    secret_delete(id).await;
    Ok(())
}

#[derive(Serialize)]
pub struct ConnectResult {
    pub conn: u32,
    pub label: String,
}

/// Baut die Verbindung zu einem gespeicherten Profil auf. Bei SFTP wird
/// der Host-Key nach Trust-on-first-use im Profil gemerkt; ein später
/// geänderter Server-Key lässt die Verbindung scheitern.
#[tauri::command]
pub async fn connect(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    id: String,
) -> Result<ConnectResult, String> {
    let mut profiles = load_profiles(&app)?;
    let profile = profiles
        .iter_mut()
        .find(|p| p.id == id)
        .ok_or("Profil nicht gefunden")?;
    let secret = secret_get(profile.id.clone()).await?.unwrap_or_default();

    let backend: Arc<dyn Backend> = match profile.protocol {
        Protocol::Sftp => {
            let auth = if profile.key_file.is_empty() {
                SftpAuth::Password(secret)
            } else {
                SftpAuth::KeyFile {
                    path: profile.key_file.clone().into(),
                    passphrase: (!secret.is_empty()).then_some(secret),
                }
            };
            let be = SftpBackend::connect(SftpConfig {
                host: profile.host.clone(),
                port: if profile.port == 0 { 22 } else { profile.port },
                user: profile.user.clone(),
                auth,
                expected_host_key: (!profile.host_key.is_empty())
                    .then(|| profile.host_key.clone()),
            })
            .await
            .map_err(|e| e.to_string())?;
            if profile.host_key.is_empty() {
                profile.host_key = be.host_key().to_string();
                store_profiles(&app, &profiles.clone())?;
            }
            Arc::new(be)
        }
        // Alte „FTP"-Profile werden stillschweigend auf AUTH TLS gehoben —
        // Klartext-FTP baut fileport grundsätzlich nicht mehr auf.
        Protocol::Ftp | Protocol::Ftps | Protocol::FtpsImplicit => {
            let implicit = profile.protocol == Protocol::FtpsImplicit;
            let security = if implicit {
                FtpSecurity::ImplicitTls
            } else {
                FtpSecurity::ExplicitTls
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
                key_file: profile.key_file.clone(),
            })
            .await
            .map_err(|e| e.to_string())?,
        ),
    };

    let label = backend.label();
    let conn = engine.insert(backend).await;
    Ok(ConnectResult { conn, label })
}

#[tauri::command]
pub async fn disconnect(engine: tauri::State<'_, Engine>, conn: u32) -> Result<(), String> {
    engine.remove(conn).await;
    Ok(())
}
