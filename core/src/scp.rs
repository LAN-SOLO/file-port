//! SCP über SSH: Dateitransfers sprechen das klassische SCP-Protokoll
//! (`scp -f`/`scp -t` auf der Gegenstelle), Verzeichnis-Operationen laufen
//! als Shell-Kommandos über die gemeinsame [`SshSession`]. Für Server,
//! die kein SFTP-Subsystem anbieten, aber SSH mit Shell erlauben.

use std::path::Path;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::backend::{Backend, Entry};
use crate::error::FpError;
use crate::rpath;
use crate::ssh::{connect_ssh, shell_quote, SshConfig, SshSession};
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

pub struct ScpBackend {
    ssh: SshSession,
    host_key: String,
    label: String,
}

impl ScpBackend {
    pub async fn connect(cfg: SshConfig) -> Result<Self, FpError> {
        let ssh = connect_ssh(&cfg).await?;
        let be = ScpBackend {
            host_key: ssh.host_key.clone(),
            label: format!("scp://{}@{}", cfg.user, cfg.host),
            ssh,
        };
        // Exec-Fähigkeit sofort prüfen — SFTP-only-Server (z. B. mit
        // ForceCommand internal-sftp) sollen beim Verbinden scheitern,
        // nicht erst beim Browsen.
        be.ssh.initial_dir().await.map_err(|e| {
            FpError::Connect(format!(
                "Server erlaubt keine Shell-Kommandos (für SCP nötig): {e}"
            ))
        })?;
        Ok(be)
    }

    /// Der beim Verbinden gesehene Host-Key (OpenSSH-Format) — wie bei SFTP
    /// speichert ihn die App im Profil (Trust on first use).
    pub fn host_key(&self) -> &str {
        &self.host_key
    }
}

/// Liest eine `\n`-terminierte Protokollzeile (SCP-Header sind kurz).
async fn read_line<R: AsyncRead + Unpin>(stream: &mut R) -> Result<String, FpError> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream.read_exact(&mut byte).await.map_err(scp_eof)?;
        if byte[0] == b'\n' {
            break;
        }
        line.push(byte[0]);
        if line.len() > 4096 {
            return Err(FpError::Protocol("SCP: überlange Protokollzeile".into()));
        }
    }
    Ok(String::from_utf8_lossy(&line).into_owned())
}

/// Liest ein SCP-Bestätigungsbyte; 1/2 sind (fatale) Fehler mit Meldung.
async fn read_ack<R: AsyncRead + Unpin>(stream: &mut R) -> Result<(), FpError> {
    let mut byte = [0u8; 1];
    stream.read_exact(&mut byte).await.map_err(scp_eof)?;
    match byte[0] {
        0 => Ok(()),
        1 | 2 => {
            let msg = read_line(stream).await.unwrap_or_default();
            Err(FpError::Protocol(format!("SCP: {}", msg.trim())))
        }
        other => Err(FpError::Protocol(format!(
            "SCP: unerwartete Antwort ({other:#04x})"
        ))),
    }
}

fn scp_eof(e: std::io::Error) -> FpError {
    if e.kind() == std::io::ErrorKind::UnexpectedEof {
        FpError::Protocol(
            "SCP: Server hat die Sitzung beendet — ist scp auf dem Server installiert?".into(),
        )
    } else {
        FpError::Protocol(e.to_string())
    }
}

async fn ack<W: AsyncWrite + Unpin>(stream: &mut W) -> Result<(), FpError> {
    stream
        .write_all(&[0])
        .await
        .map_err(|e| FpError::Protocol(e.to_string()))?;
    stream
        .flush()
        .await
        .map_err(|e| FpError::Protocol(e.to_string()))?;
    Ok(())
}

#[async_trait]
impl Backend for ScpBackend {
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
        let channel = self
            .ssh
            .handle
            .channel_open_session()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        channel
            .exec(true, format!("scp -f {}", shell_quote(remote)))
            .await
            .map_err(|e| FpError::Protocol(e.to_string()))?;
        let mut stream = channel.into_stream();

        ack(&mut stream).await?;
        let header = read_line(&mut stream).await?;
        let rest = match header.as_bytes().first().copied() {
            Some(b'C') => &header[1..],
            Some(1) | Some(2) => {
                return Err(FpError::Protocol(format!("SCP: {}", header[1..].trim())))
            }
            _ => {
                return Err(FpError::Protocol(format!(
                    "SCP: unerwarteter Header „{header}“"
                )))
            }
        };
        // Header: „C<mode> <größe> <name>"
        let size: u64 = rest
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| FpError::Protocol(format!("SCP: unlesbarer Header „{header}“")))?;
        ack(&mut stream).await?;

        let dst = tokio::fs::File::create(local).await?;
        let result = {
            let limited = (&mut stream).take(size);
            copy_with_progress(limited, dst, Some(size), ctl).await?
        };
        // Status-Byte des Servers, dann letzte Bestätigung von uns.
        read_ack(&mut stream).await?;
        ack(&mut stream).await?;
        Ok(result)
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let src = tokio::fs::File::open(local).await?;
        let size = src.metadata().await?.len();

        let channel = self
            .ssh
            .handle
            .channel_open_session()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        channel
            .exec(true, format!("scp -t {}", shell_quote(remote)))
            .await
            .map_err(|e| FpError::Protocol(e.to_string()))?;
        let mut stream = channel.into_stream();

        read_ack(&mut stream).await?;
        let name = rpath::file_name(remote);
        stream
            .write_all(format!("C0644 {size} {name}\n").as_bytes())
            .await
            .map_err(|e| FpError::Protocol(e.to_string()))?;
        stream
            .flush()
            .await
            .map_err(|e| FpError::Protocol(e.to_string()))?;
        read_ack(&mut stream).await?;

        let result = copy_with_progress(src, &mut stream, Some(size), ctl).await?;
        ack(&mut stream).await?;
        read_ack(&mut stream).await?;
        Ok(result)
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
