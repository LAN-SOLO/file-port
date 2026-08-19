use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use russh::client;
use russh::keys::{HashAlg, PrivateKeyWithHashAlg};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use tokio::io::AsyncWriteExt;

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

pub enum SftpAuth {
    Password(String),
    KeyFile {
        path: PathBuf,
        passphrase: Option<String>,
    },
}

pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SftpAuth,
    /// Bekannter Host-Key (OpenSSH-Format) aus einer früheren Verbindung.
    /// `None` = Trust-on-first-use: der Key wird akzeptiert und über
    /// [`SftpBackend::host_key`] zurückgemeldet, damit die App ihn speichert.
    pub expected_host_key: Option<String>,
}

/// Prüft den Server-Key gegen den gespeicherten (TOFU) und reicht den
/// tatsächlich gesehenen Key nach außen.
struct HostKeyCheck {
    expected: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
}

impl client::Handler for HostKeyCheck {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        key: &russh::keys::PublicKey,
    ) -> Result<bool, Self::Error> {
        let openssh = key.to_openssh().map_err(|_| russh::Error::UnknownKey)?;
        *self.seen.lock().unwrap() = Some(openssh.clone());
        match &self.expected {
            Some(expected) => Ok(expected.trim() == openssh.trim()),
            None => Ok(true),
        }
    }
}

pub struct SftpBackend {
    sftp: SftpSession,
    // Hält die SSH-Session am Leben, solange das Backend existiert.
    _handle: client::Handle<HostKeyCheck>,
    host_key: String,
    label: String,
}

impl SftpBackend {
    pub async fn connect(cfg: SftpConfig) -> Result<Self, FpError> {
        let seen = Arc::new(Mutex::new(None));
        let handler = HostKeyCheck {
            expected: cfg.expected_host_key.clone(),
            seen: seen.clone(),
        };
        let config = Arc::new(client::Config::default());
        let mut handle = client::connect(config, (cfg.host.as_str(), cfg.port), handler)
            .await
            .map_err(|e| match e {
                russh::Error::UnknownKey => FpError::Connect(format!(
                    "Host-Key von {} stimmt nicht mit dem gespeicherten überein",
                    cfg.host
                )),
                other => FpError::Connect(other.to_string()),
            })?;

        let authed = match &cfg.auth {
            SftpAuth::Password(pw) => handle
                .authenticate_password(&cfg.user, pw)
                .await
                .map_err(|e| FpError::Auth(e.to_string()))?,
            SftpAuth::KeyFile { path, passphrase } => {
                let key = russh::keys::load_secret_key(path, passphrase.as_deref())
                    .map_err(|e| FpError::Auth(format!("Schlüsseldatei: {e}")))?;
                let hash: Option<HashAlg> = handle
                    .best_supported_rsa_hash()
                    .await
                    .map_err(|e| FpError::Auth(e.to_string()))?
                    .flatten();
                handle
                    .authenticate_publickey(
                        &cfg.user,
                        PrivateKeyWithHashAlg::new(Arc::new(key), hash),
                    )
                    .await
                    .map_err(|e| FpError::Auth(e.to_string()))?
            }
        };
        if !authed.success() {
            return Err(FpError::Auth(format!(
                "Server hat die Anmeldung als {} abgelehnt",
                cfg.user
            )));
        }

        let channel = handle
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

        let host_key = seen.lock().unwrap().clone().unwrap_or_default();
        Ok(SftpBackend {
            sftp,
            _handle: handle,
            host_key,
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
