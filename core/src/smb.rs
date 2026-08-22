//! SMB 2/3 (Windows-/NAS-Freigaben) über die Pure-Rust-Crate `smb2`.
//! Es wird ausschließlich SMB2/3 gesprochen (mit Signierung und — wo der
//! Server es aushandelt — SMB3-Verschlüsselung); SMB1/CIFS gibt es nicht.
//! UI-Pfade (`/a/b`) werden auf Share-relative SMB-Pfade (`a\b`) abgebildet.

use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use async_trait::async_trait;
use smb2::{ClientConfig, SmbClient, Tree};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::transfer::{TransferCtl, TransferResult};

pub struct SmbConfig {
    pub host: String,
    /// 0 = Standardport 445.
    pub port: u16,
    pub share: String,
    /// Bei Domänen-Konten `DOMÄNE\benutzer`.
    pub user: String,
    pub password: String,
}

pub struct SmbBackend {
    /// Client und Tree brauchen `&mut` — Operationen laufen strikt seriell.
    /// Transfers halten den Mutex nur zum Öffnen: Reader/Writer besitzen
    /// einen eigenen Connection-Klon und streamen ohne Lock.
    inner: Mutex<(SmbClient, Tree)>,
    label: String,
}

fn map_smb(path: &str, e: smb2::Error) -> FpError {
    use smb2::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => FpError::NotFound(path.to_string()),
        ErrorKind::AccessDenied => FpError::Denied(path.to_string()),
        ErrorKind::AuthRequired => FpError::Auth(e.to_string()),
        _ => FpError::Protocol(format!("{path}: {e}")),
    }
}

/// UI-Pfad (`/a/b`) → Share-relativer Pfad (`a/b`). Die smb2-Crate nutzt
/// `/` als einzigen Pfadtrenner; `\` ist dort ein normales Namenszeichen.
fn smb_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

impl SmbBackend {
    pub async fn connect(cfg: SmbConfig) -> Result<Self, FpError> {
        let port = if cfg.port == 0 { 445 } else { cfg.port };
        // `DOMÄNE\benutzer` im Benutzerfeld → Domäne + Konto trennen.
        let (domain, user) = match cfg.user.split_once('\\') {
            Some((domain, user)) => (domain.to_string(), user.to_string()),
            None => (String::new(), cfg.user.clone()),
        };
        let client = SmbClient::connect(ClientConfig {
            addr: format!("{}:{port}", cfg.host),
            timeout: Duration::from_secs(15),
            username: user,
            password: cfg.password.clone(),
            domain,
            auto_reconnect: true,
            compression: true,
            dfs_enabled: true,
            dfs_target_overrides: Default::default(),
        })
        .await
        .map_err(|e| match e.kind() {
            smb2::ErrorKind::AuthRequired | smb2::ErrorKind::AccessDenied => {
                FpError::Auth(format!("Server hat die Anmeldung als {} abgelehnt", cfg.user))
            }
            _ => FpError::Connect(e.to_string()),
        })?;
        let mut client = client;
        let tree = client
            .connect_share(&cfg.share)
            .await
            .map_err(|e| FpError::Connect(format!("Freigabe {}: {e}", cfg.share)))?;

        let be = SmbBackend {
            inner: Mutex::new((client, tree)),
            label: format!("smb://{}/{}", cfg.host, cfg.share),
        };
        // Freigabe sofort prüfen, nicht erst beim Browsen.
        be.list("/").await?;
        Ok(be)
    }
}

#[async_trait]
impl Backend for SmbBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        Ok("/".to_string())
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let mut guard = self.inner.lock().await;
        let (client, tree) = &mut *guard;
        let entries = client
            .list_directory(tree, &smb_path(path))
            .await
            .map_err(|e| map_smb(path, e))?;
        let mut out = Vec::new();
        for e in entries {
            if e.name == "." || e.name == ".." {
                continue;
            }
            out.push(Entry {
                path: rpath::join(path, &e.name),
                kind: if e.is_directory {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                },
                size: e.size,
                modified: e
                    .modified
                    .to_system_time()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64),
                name: e.name,
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
        let reader = {
            let guard = self.inner.lock().await;
            let (client, tree) = &*guard;
            client
                .open_file_reader(tree, &smb_path(remote))
                .await
                .map_err(|e| map_smb(remote, e))?
        };
        let total = reader.size();

        let mut dst = tokio::fs::File::create(local).await?;
        let mut hasher = blake3::Hasher::new();
        let mut pos: u64 = 0;
        const CHUNK: u64 = 256 * 1024;
        while pos < total {
            if ctl.cancel.is_cancelled() {
                let _ = reader.close().await;
                return Err(FpError::Cancelled);
            }
            let len = CHUNK.min(total - pos);
            let data = reader
                .read_at(pos, len)
                .await
                .map_err(|e| map_smb(remote, e))?;
            if data.is_empty() {
                break;
            }
            dst.write_all(&data).await?;
            hasher.update(&data);
            pos += data.len() as u64;
            (ctl.progress)(pos, Some(total));
        }
        dst.flush().await?;
        reader.close().await.map_err(|e| map_smb(remote, e))?;
        Ok(TransferResult {
            bytes: pos,
            blake3: hasher.finalize().to_hex().to_string(),
        })
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let mut src = tokio::fs::File::open(local).await?;
        let total = src.metadata().await?.len();

        let mut writer = {
            let guard = self.inner.lock().await;
            let (client, tree) = &*guard;
            client
                .create_file_writer(tree, &smb_path(remote))
                .await
                .map_err(|e| map_smb(remote, e))?
        };

        let mut hasher = blake3::Hasher::new();
        let mut buf = vec![0u8; 256 * 1024];
        let mut pos: u64 = 0;
        loop {
            if ctl.cancel.is_cancelled() {
                return Err(FpError::Cancelled);
            }
            let n = src.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            writer
                .write_chunk(&buf[..n])
                .await
                .map_err(|e| map_smb(remote, e))?;
            hasher.update(&buf[..n]);
            pos += n as u64;
            (ctl.progress)(pos, Some(total));
        }
        writer.finish().await.map_err(|e| map_smb(remote, e))?;
        Ok(TransferResult {
            bytes: pos,
            blake3: hasher.finalize().to_hex().to_string(),
        })
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        let mut guard = self.inner.lock().await;
        let (client, tree) = &mut *guard;
        client
            .create_directory(tree, &smb_path(path))
            .await
            .map_err(|e| map_smb(path, e))
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        if is_dir {
            // SMB löscht nur leere Verzeichnisse — Inhalt rekursiv abräumen.
            for entry in self.list(path).await? {
                Box::pin(self.remove(&entry.path, entry.kind == EntryKind::Dir)).await?;
            }
        }
        let mut guard = self.inner.lock().await;
        let (client, tree) = &mut *guard;
        if is_dir {
            client
                .delete_directory(tree, &smb_path(path))
                .await
                .map_err(|e| map_smb(path, e))
        } else {
            client
                .delete_file(tree, &smb_path(path))
                .await
                .map_err(|e| map_smb(path, e))
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        let mut guard = self.inner.lock().await;
        let (client, tree) = &mut *guard;
        client
            .rename(tree, &smb_path(from), &smb_path(to))
            .await
            .map_err(|e| map_smb(from, e))
    }
}
