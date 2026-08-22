use std::path::Path;

use async_trait::async_trait;
use russh::client;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::AsyncWriteExt;

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::ssh::{connect_ssh, HostKeyCheck, SshConfig};
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

pub struct SftpBackend {
    sftp: SftpSession,
    // Hält die SSH-Session am Leben, solange das Backend existiert.
    _handle: client::Handle<HostKeyCheck>,
    host_key: String,
    label: String,
}

impl SftpBackend {
    pub async fn connect(cfg: SshConfig) -> Result<Self, FpError> {
        let session = connect_ssh(&cfg).await?;
        let channel = session
            .handle
            .channel_open_session()
            .await
            .map_err(|e| FpError::Connect(e.to_string()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| FpError::Connect(format!("SFTP-Subsystem: {e}")))?;
        let sftp = SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| FpError::Connect(format!("SFTP-Handshake: {e}")))?;

        Ok(SftpBackend {
            sftp,
            _handle: session.handle,
            host_key: session.host_key,
            label: format!("sftp://{}@{}", cfg.user, cfg.host),
        })
    }

    /// Der beim Verbinden gesehene Host-Key (OpenSSH-Format) — die App
    /// speichert ihn im Profil und reicht ihn beim nächsten Mal als
    /// `expected_host_key` wieder herein.
    pub fn host_key(&self) -> &str {
        &self.host_key
    }
}

fn map_sftp(path: &str, e: russh_sftp::client::error::Error) -> FpError {
    use russh_sftp::protocol::StatusCode;
    match e {
        russh_sftp::client::error::Error::Status(status) => match status.status_code {
            StatusCode::NoSuchFile => FpError::NotFound(path.to_string()),
            StatusCode::PermissionDenied => FpError::Denied(path.to_string()),
            _ => FpError::Protocol(format!("{path}: {}", status.error_message)),
        },
        other => FpError::Protocol(other.to_string()),
    }
}

#[async_trait]
impl Backend for SftpBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        self.sftp.canonicalize(".").await.map_err(|e| map_sftp(".", e))
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let dir = self.sftp.read_dir(path).await.map_err(|e| map_sftp(path, e))?;
        let mut out = Vec::new();
        for item in dir {
            let name = item.file_name();
            if name == "." || name == ".." {
                continue;
            }
            let meta = item.metadata();
            let kind = if item.file_type().is_dir() {
                EntryKind::Dir
            } else if item.file_type().is_symlink() {
                EntryKind::Symlink
            } else {
                EntryKind::File
            };
            out.push(Entry {
                path: rpath::join(path, &name),
                name,
                kind,
                size: meta.size.unwrap_or(0),
                modified: meta.mtime.map(|t| t as i64),
            });
        }
        Ok(out)
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let total = self
            .sftp
            .metadata(remote)
            .await
            .ok()
            .and_then(|m| m.size);
        let src = self
            .sftp
            .open_with_flags(remote, OpenFlags::READ)
            .await
            .map_err(|e| map_sftp(remote, e))?;
        let dst = tokio::fs::File::create(local).await?;
        copy_with_progress(src, dst, total, ctl).await
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let src = tokio::fs::File::open(local).await?;
        let total = src.metadata().await.ok().map(|m| m.len());
        let mut dst = self
            .sftp
            .open_with_flags(
                remote,
                OpenFlags::CREATE | OpenFlags::WRITE | OpenFlags::TRUNCATE,
            )
            .await
            .map_err(|e| map_sftp(remote, e))?;
        let result = copy_with_progress(src, &mut dst, total, ctl).await?;
        dst.shutdown().await?;
        Ok(result)
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        self.sftp.create_dir(path).await.map_err(|e| map_sftp(path, e))
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        if is_dir {
            self.sftp.remove_dir(path).await.map_err(|e| map_sftp(path, e))
        } else {
            self.sftp.remove_file(path).await.map_err(|e| map_sftp(path, e))
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        self.sftp.rename(from, to).await.map_err(|e| map_sftp(from, e))
    }
}
