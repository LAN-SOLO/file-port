//! rsync über SSH: Verzeichnis-Operationen laufen wie bei SCP als
//! Shell-Kommandos über die gemeinsame [`SshSession`] (russh); die
//! eigentlichen Transfers übernimmt das lokale `rsync`-Programm mit
//! OpenSSH als Transport (`rsync -e ssh`). Deshalb gilt:
//!
//! * `rsync` und `ssh` müssen lokal installiert sein (macOS/Linux: ja,
//!   Windows: nur mit installiertem rsync, z. B. über MSYS2/Cygwin),
//! * Transfers brauchen eine SSH-Schlüsseldatei ohne Passphrase-Abfrage
//!   (`BatchMode=yes` — Passwort-Prompts gibt es headless nicht).

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
        Command::new("rsync")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|_| {
                FpError::Connect(
                    "Kein rsync-Programm gefunden — bitte rsync lokal installieren".into(),
                )
            })?;

        let ssh = connect_ssh(&cfg).await?;
        let be = RsyncBackend {
            host_key: ssh.host_key.clone(),
            host: cfg.host.clone(),
            port: cfg.port,
            user: cfg.user.clone(),
            key_file,
            label: format!("rsync://{}@{}", cfg.user, cfg.host),
            ssh,
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
    /// neue Host-Keys akzeptieren, Port und Schlüsseldatei aus dem Profil.
    fn ssh_transport(&self) -> String {
        format!(
            "ssh -o BatchMode=yes -o StrictHostKeyChecking=accept-new -p {} -i {}",
            self.port,
            shell_quote(&self.key_file.to_string_lossy())
        )
    }

    fn remote_arg(&self, remote: &str) -> String {
        // rsync reicht den Pfad durch die Remote-Shell — deshalb quoten.
        format!("{}@{}:{}", self.user, self.host, shell_quote(remote))
    }

    /// Startet rsync, meldet Fortschritt aus `--info=progress2`-Zeilen und
    /// bricht ab, sobald das Abbruch-Token feuert.
    async fn run_rsync(
        &self,
        from: &str,
        to: &str,
        total: Option<u64>,
        ctl: &TransferCtl,
    ) -> Result<(), FpError> {
        // `--progress` statt `--info=progress2`: macOS liefert openrsync,
        // das die GNU-Flags nicht kennt; beide drucken „<bytes> <pct>% …".
        let mut child = Command::new("rsync")
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
        self.run_rsync(
            &self.remote_arg(remote),
            &local.to_string_lossy(),
            total,
            ctl,
        )
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
        self.run_rsync(
            &local.to_string_lossy(),
            &self.remote_arg(remote),
            total,
            ctl,
        )
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
