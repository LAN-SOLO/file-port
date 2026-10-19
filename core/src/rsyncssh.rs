//! rsync über SSH: Verzeichnis-Operationen laufen wie bei SCP als
//! Shell-Kommandos über die gemeinsame [`SshSession`] (russh); die
//! eigentlichen Transfers übernimmt das lokale `rsync`-Programm mit
//! OpenSSH als Transport (`rsync -e ssh`). Deshalb gilt:
//!
//! * `rsync` und `ssh` müssen lokal installiert sein (macOS/Linux: ja,
//!   Windows: nur mit installiertem rsync, z. B. über MSYS2/Cygwin),
//! * Transfers brauchen eine SSH-Schlüsseldatei ohne Passphrase-Abfrage
//!   (`BatchMode=yes` — Passwort-Prompts gibt es headless nicht).
//!
//! Sicherheit: Das gespawnte ssh prüft den Server gegen genau den Host-Key,
//! den fileport beim Verbinden (TOFU/gespeichert) verifiziert hat — über eine
//! eigene, temporäre known_hosts-Datei mit `StrictHostKeyChecking=yes`. Die
//! known_hosts der Nutzerin werden weder gelesen noch beschrieben.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::process::Command;

use crate::backend::{Backend, Entry};
use crate::error::FpError;
use crate::ssh::{connect_ssh, shell_quote, SshAuth, SshConfig, SshSession};
use crate::transfer::{TransferCtl, TransferResult};

pub struct RsyncBackend {
    ssh: SshSession,
    host: String,
    port: u16,
    user: String,
    key_file: PathBuf,
    host_key: String,
    label: String,
    /// Temporäre known_hosts-Datei mit dem verifizierten Host-Key.
    known_hosts: PathBuf,
    /// Remote-Pfade selbst quoten? Nein bei GNU rsync ≥ 3.2.4 — das
    /// escaped Argumente für die Remote-Shell inzwischen selbst.
    quote_remote: bool,
}

impl Drop for RsyncBackend {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.known_hosts);
    }
}

/// rsync-Prozess ohne aufblitzendes Konsolenfenster (Windows-GUI-App).
fn rsync_command() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("rsync");
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Alias, unter dem der Host-Key in der temporären known_hosts steht —
/// unabhängig von Host-Schreibweise und Port.
const HOST_KEY_ALIAS: &str = "fileport-pinned";

/// Entscheidet anhand von `rsync --version`, ob Remote-Pfade für die
/// Remote-Shell gequotet werden müssen: openrsync (macOS) und GNU rsync
/// vor 3.2.4 reichen sie roh durch, GNU rsync ab 3.2.4 escaped selbst
/// (doppeltes Quoten würde die Anführungszeichen Teil des Namens machen).
pub(crate) fn remote_args_need_quoting(version_output: &str) -> bool {
    if version_output.contains("openrsync") {
        return true;
    }
    let Some(rest) = version_output.split("version").nth(1) else {
        return true;
    };
    let ver = rest.split_whitespace().next().unwrap_or("").trim_start_matches('v');
    let mut parts = ver.split('.').map(|p| {
        p.chars()
            .take_while(|c| c.is_ascii_digit())
            .collect::<String>()
            .parse::<u32>()
            .unwrap_or(0)
    });
    let (major, minor, patch) = (
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    );
    (major, minor, patch) < (3, 2, 4)
}

/// Eindeutiger Pfad für die temporäre known_hosts-Datei.
fn known_hosts_path() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "fileport-known_hosts-{}-{nanos}-{n}",
        std::process::id()
    ))
}

/// Lokalen Pfad für rsync aufbereiten: rsync liest `x:y` als Remote-Ziel
/// (fatal bei Windows-Laufwerksbuchstaben wie `C:\…`). Deshalb läuft rsync
/// im Elternverzeichnis und bekommt nur `./name`.
fn local_rsync_arg(path: &Path) -> Result<(PathBuf, String), FpError> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = path
        .file_name()
        .ok_or_else(|| FpError::Protocol(format!("{}: kein Dateiname", path.display())))?
        .to_string_lossy()
        .into_owned();
    Ok((parent, format!("./{name}")))
}

impl RsyncBackend {
    pub async fn connect(cfg: SshConfig) -> Result<Self, FpError> {
        let key_file = match &cfg.auth {
            SshAuth::KeyFile { path, .. } => path.clone(),
            SshAuth::Password(_) => {
                return Err(FpError::Auth(
                    "rsync-Transfers laufen über das System-SSH und brauchen eine \
                     SSH-Schlüsseldatei — Passwort-Anmeldung wird hier nicht unterstützt"
                        .into(),
                ))
            }
        };
        // Lokales rsync muss vorhanden sein, sonst früh und klar scheitern.
        let version = rsync_command()
            .arg("--version")
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .await
            .map_err(|_| {
                FpError::Connect(
                    "Kein rsync-Programm gefunden — bitte rsync lokal installieren".into(),
                )
            })?;
        let quote_remote =
            remote_args_need_quoting(&String::from_utf8_lossy(&version.stdout));

        let ssh = connect_ssh(&cfg).await?;
        if ssh.host_key.trim().is_empty() {
            return Err(FpError::Connect("Server hat keinen Host-Key gezeigt".into()));
        }
        let known_hosts = known_hosts_path();
        std::fs::write(
            &known_hosts,
            format!("{HOST_KEY_ALIAS} {}\n", ssh.host_key.trim()),
        )?;
        let be = RsyncBackend {
            host_key: ssh.host_key.clone(),
            host: cfg.host.clone(),
            port: cfg.port,
            user: cfg.user.clone(),
            key_file,
            label: format!("rsync://{}@{}", cfg.user, cfg.host),
            ssh,
            known_hosts,
            quote_remote,
        };
        be.ssh.initial_dir().await.map_err(|e| {
            FpError::Connect(format!(
                "Server erlaubt keine Shell-Kommandos (für rsync nötig): {e}"
            ))
        })?;
        Ok(be)
    }

    /// Der beim Verbinden gesehene Host-Key (OpenSSH-Format) — wie bei SFTP
    /// speichert ihn die App im Profil (Trust on first use).
    pub fn host_key(&self) -> &str {
        &self.host_key
    }

    /// SSH-Transport für das gespawnte rsync: Batch-Mode (keine Prompts),
    /// Host-Key strikt gegen den verifizierten Key, nur die Profil-Schlüsseldatei.
    fn ssh_transport(&self) -> String {
        format!(
            "ssh -o BatchMode=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile={} \
             -o HostKeyAlias={HOST_KEY_ALIAS} -o CheckHostIP=no -o IdentitiesOnly=yes \
             -p {} -i {}",
            rsync_quote(&self.known_hosts.to_string_lossy()),
            self.port,
            rsync_quote(&self.key_file.to_string_lossy())
        )
    }

    fn remote_arg(&self, remote: &str) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host) // IPv6
        } else {
            self.host.clone()
        };
        let path = if self.quote_remote {
            // Älteres rsync/openrsync reicht den Pfad roh durch die Remote-Shell.
            shell_quote(remote)
        } else {
            remote.to_string()
        };
        format!("{}@{}:{}", self.user, host, path)
    }

    /// Startet rsync, meldet Fortschritt aus `--info=progress2`-Zeilen und
    /// bricht ab, sobald das Abbruch-Token feuert.
    async fn run_rsync(
        &self,
        cwd: &Path,
        from: &str,
        to: &str,
        total: Option<u64>,
        ctl: &TransferCtl,
    ) -> Result<(), FpError> {
        // `--progress` statt `--info=progress2`: macOS liefert openrsync,
        // das die GNU-Flags nicht kennt; beide drucken „<bytes> <pct>% …".
        let mut child = rsync_command()
            .current_dir(cwd)
            .kill_on_drop(true)
            .arg("--progress")
            .arg("-e")
            .arg(self.ssh_transport())
            .arg(from)
            .arg(to)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| FpError::Connect(format!("rsync ließ sich nicht starten: {e}")))?;

        // Fortschritt: erste Zahl jeder Prozent-Zeile = übertragene Bytes.
        let stdout = child.stdout.take();
        let progress = ctl.progress.clone();
        let reader = tokio::spawn(async move {
            let Some(stdout) = stdout else { return };
            let mut reader = BufReader::new(stdout);
            let mut buf = [0u8; 4096];
            let mut line = Vec::new();
            while let Ok(n) = reader.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                for &b in &buf[..n] {
                    if b == b'\r' || b == b'\n' {
                        let text = String::from_utf8_lossy(&line);
                        if text.contains('%') {
                            let digits: String = text
                                .split_whitespace()
                                .next()
                                .unwrap_or("")
                                .chars()
                                .filter(|c| c.is_ascii_digit())
                                .collect();
                            if let Ok(done) = digits.parse::<u64>() {
                                progress(done, total);
                            }
                        }
                        line.clear();
                    } else {
                        line.push(b);
                    }
                }
            }
        });

        let mut stderr_pipe = child.stderr.take();
        let status = tokio::select! {
            status = child.wait() => status.map_err(|e| FpError::Protocol(e.to_string()))?,
            _ = ctl.cancel.cancelled() => {
                let _ = child.kill().await;
                reader.abort();
                return Err(FpError::Cancelled);
            }
        };
        let _ = reader.await;

        if !status.success() {
            let mut stderr = String::new();
            if let Some(pipe) = stderr_pipe.as_mut() {
                let mut raw = Vec::new();
                let _ = pipe.read_to_end(&mut raw).await;
                stderr = String::from_utf8_lossy(&raw).into_owned();
            }
            let msg = stderr.trim();
            if msg.contains("No such file or directory") {
                return Err(FpError::NotFound(from.to_string()));
            }
            if msg.contains("Permission denied") || msg.contains("permission denied") {
                return Err(FpError::Auth(format!(
                    "rsync/SSH: {msg} — funktioniert die Schlüsseldatei ohne Passphrase-Abfrage?"
                )));
            }
            return Err(FpError::Protocol(format!("rsync: {msg}")));
        }
        Ok(())
    }
}

/// Quoting für die `-e`-Kommandozeile: rsync trennt an Leerzeichen und
/// versteht einfache/doppelte Anführungszeichen, aber keine Backslash-Escapes.
fn rsync_quote(s: &str) -> String {
    if s.contains('\'') {
        format!("\"{s}\"")
    } else {
        format!("'{s}'")
    }
}

/// BLAKE3 über eine lokale Datei — rsync überträgt selbst, deshalb wird die
/// Prüfsumme nach dem Transfer über das lokale Ergebnis (bzw. die Quelle)
/// gebildet, damit [`TransferResult`] überall dieselbe Bedeutung hat.
async fn blake3_file(path: &Path) -> Result<(u64, String), FpError> {
    let mut file = tokio::fs::File::open(path).await?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 128 * 1024];
    let mut bytes: u64 = 0;
    loop {
        let n = file.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        bytes += n as u64;
    }
    Ok((bytes, hasher.finalize().to_hex().to_string()))
}

#[async_trait]
impl Backend for RsyncBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        self.ssh.initial_dir().await
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        self.ssh.list_dir(path).await
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let total = self.ssh.file_size(remote).await.ok();
        let (cwd, local_arg) = local_rsync_arg(local)?;
        self.run_rsync(&cwd, &self.remote_arg(remote), &local_arg, total, ctl)
            .await?;
        let (bytes, blake3) = blake3_file(local).await?;
        Ok(TransferResult { bytes, blake3 })
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let total = tokio::fs::metadata(local).await.ok().map(|m| m.len());
        let (cwd, local_arg) = local_rsync_arg(local)?;
        self.run_rsync(&cwd, &local_arg, &self.remote_arg(remote), total, ctl)
            .await?;
        let (bytes, blake3) = blake3_file(local).await?;
        Ok(TransferResult { bytes, blake3 })
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        self.ssh.mkdir(path).await
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        self.ssh.remove(path, is_dir).await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        self.ssh.rename(from, to).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting_depends_on_rsync_flavour() {
        // macOS: openrsync
        assert!(remote_args_need_quoting(
            "openrsync: protocol version 29\nrsync version 2.6.9 compatible\n"
        ));
        // GNU rsync alt / neu
        assert!(remote_args_need_quoting("rsync  version 3.1.3  protocol version 31\n"));
        assert!(remote_args_need_quoting("rsync  version 3.2.3  protocol version 31\n"));
        assert!(!remote_args_need_quoting("rsync  version 3.2.4  protocol version 31\n"));
        assert!(!remote_args_need_quoting("rsync  version v3.2.7  protocol version 31\n"));
        assert!(!remote_args_need_quoting("rsync  version 3.4.1  protocol version 32\n"));
        // Unbekannt → sicherheitshalber quoten
        assert!(remote_args_need_quoting(""));
    }

    #[test]
    fn local_arg_avoids_colon_paths() {
        let p = Path::new("some").join("dir").join("file.bin");
        let (cwd, arg) = local_rsync_arg(&p).unwrap();
        assert_eq!(cwd, Path::new("some").join("dir"));
        assert_eq!(arg, "./file.bin");
        let (cwd, arg) = local_rsync_arg(Path::new("solo.txt")).unwrap();
        assert_eq!(cwd, PathBuf::from("."));
        assert_eq!(arg, "./solo.txt");
    }

    #[test]
    fn rsync_quote_handles_quotes() {
        assert_eq!(rsync_quote("/a b/key"), "'/a b/key'");
        assert_eq!(rsync_quote("/it's/key"), "\"/it's/key\"");
    }
}
