use std::io::{Read, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use async_trait::async_trait;
use suppaftp::native_tls::TlsConnector;
use suppaftp::types::FileType;
use suppaftp::{Mode, NativeTlsConnector, NativeTlsFtpStream};

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::rpath;
use crate::transfer::{TransferCtl, TransferResult};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FtpSecurity {
    /// Explizites FTPS (AUTH TLS auf dem Standardport 21).
    ExplicitTls,
    /// Implizites FTPS (TLS ab dem ersten Byte, Standardport 990).
    ImplicitTls,
}

pub struct FtpConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub security: FtpSecurity,
    /// Selbstsignierte Zertifikate akzeptieren (bei FTP-Servern verbreitet) —
    /// bewusste Entscheidung der Nutzerin im Verbindungs-Profil.
    pub accept_invalid_certs: bool,
}

/// FTPS über suppaftp (blocking) — immer TLS-verschlüsselt, unverschlüsseltes
/// FTP wird nicht unterstützt. Jede Operation nimmt die Verbindung
/// kurz aus dem Mutex und arbeitet in `spawn_blocking`, damit der
/// Tokio-Reaktor frei bleibt.
pub struct FtpBackend {
    conn: tokio::sync::Mutex<Option<NativeTlsFtpStream>>,
    label: String,
}

fn map_ftp(path: &str, e: suppaftp::FtpError) -> FpError {
    use suppaftp::FtpError;
    match &e {
        FtpError::UnexpectedResponse(resp) => {
            let code = resp.status as u32;
            match code {
                550 => FpError::NotFound(path.to_string()),
                530 => FpError::Auth(resp.to_string()),
                532 | 533 => FpError::Denied(path.to_string()),
                _ => FpError::Protocol(format!("{path}: {resp}")),
            }
        }
        _ => FpError::Protocol(e.to_string()),
    }
}

impl FtpBackend {
    pub async fn connect(cfg: FtpConfig) -> Result<Self, FpError> {
        let label = format!("ftps://{}@{}", cfg.user, cfg.host);
        let stream = tokio::task::spawn_blocking(move || -> Result<NativeTlsFtpStream, FpError> {
            let addr = format!("{}:{}", cfg.host, cfg.port);
            let tls = TlsConnector::builder()
                .danger_accept_invalid_certs(cfg.accept_invalid_certs)
                .build()
                .map_err(|e| FpError::Connect(e.to_string()))?;
            let mut stream = match cfg.security {
                FtpSecurity::ExplicitTls => NativeTlsFtpStream::connect(&addr)
                    .map_err(|e| FpError::Connect(e.to_string()))?
                    .into_secure(NativeTlsConnector::from(tls), &cfg.host)
                    .map_err(|e| FpError::Connect(format!("TLS: {e}")))?,
                FtpSecurity::ImplicitTls => NativeTlsFtpStream::connect_secure_implicit(
                    &addr,
                    NativeTlsConnector::from(tls),
                    &cfg.host,
                )
                .map_err(|e| FpError::Connect(format!("TLS: {e}")))?,
            };
            stream
                .login(&cfg.user, &cfg.password)
                .map_err(|e| FpError::Auth(e.to_string()))?;
            stream.set_mode(Mode::Passive);
            stream
                .transfer_type(FileType::Binary)
                .map_err(|e| FpError::Connect(e.to_string()))?;
            Ok(stream)
        })
        .await
        .map_err(|e| FpError::Connect(e.to_string()))??;

        Ok(FtpBackend {
            conn: tokio::sync::Mutex::new(Some(stream)),
            label,
        })
    }

    /// Führt `f` mit der Verbindung in einem Blocking-Task aus und legt
    /// die Verbindung danach zurück — Operationen laufen strikt seriell.
    async fn with_conn<T, F>(&self, f: F) -> Result<T, FpError>
    where
        T: Send + 'static,
        F: FnOnce(&mut NativeTlsFtpStream) -> Result<T, FpError> + Send + 'static,
    {
        let mut guard = self.conn.lock().await;
        let mut stream = guard
            .take()
            .ok_or_else(|| FpError::Connect("FTP-Verbindung ist geschlossen".into()))?;
        let (stream, result) = tokio::task::spawn_blocking(move || {
            let result = f(&mut stream);
            (stream, result)
        })
        .await
        .map_err(|e| FpError::Protocol(e.to_string()))?;
        *guard = Some(stream);
        result
    }
}

/// Blocking-Kopie mit Fortschritt, Abbruch und BLAKE3 — das synchrone
/// Gegenstück zu `copy_with_progress` für Backends ohne Async-IO.
fn copy_blocking<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    total: Option<u64>,
    ctl: &TransferCtl,
) -> Result<TransferResult, FpError> {
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut hasher = blake3::Hasher::new();
    loop {
        if ctl.cancel.is_cancelled() {
            return Err(FpError::Cancelled);
        }
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        hasher.update(&buf[..n]);
        done += n as u64;
        (ctl.progress)(done, total);
    }
    writer.flush()?;
    Ok(TransferResult {
        bytes: done,
        blake3: hasher.finalize().to_hex().to_string(),
    })
}

#[async_trait]
impl Backend for FtpBackend {
    fn label(&self) -> String {
        self.label.clone()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        self.with_conn(|c| c.pwd().map_err(|e| map_ftp(".", e))).await
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let dir = path.to_string();
        self.with_conn(move |c| {
            let lines = c.list(Some(&dir)).map_err(|e| map_ftp(&dir, e))?;
            let mut out = Vec::new();
            for line in lines {
                let Ok(f) = suppaftp::list::File::try_from(line.as_str()) else {
                    continue;
                };
                let name = f.name().to_string();
                if name == "." || name == ".." {
                    continue;
                }
                out.push(Entry {
                    path: rpath::join(&dir, &name),
                    name,
                    kind: if f.is_directory() {
                        EntryKind::Dir
                    } else if f.is_symlink() {
                        EntryKind::Symlink
                    } else {
                        EntryKind::File
                    },
                    size: f.size() as u64,
                    modified: f
                        .modified()
                        .duration_since(UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs() as i64),
                });
            }
            Ok(out)
        })
        .await
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let remote = remote.to_string();
        let local = local.to_path_buf();
        let ctl = ctl.clone();
        self.with_conn(move |c| {
            let total = c.size(&remote).ok().map(|s| s as u64);
            let mut src = c.retr_as_stream(&remote).map_err(|e| map_ftp(&remote, e))?;
            let dst = std::fs::File::create(&local)?;
            let result = copy_blocking(&mut src, dst, total, &ctl);
            // Auch nach Abbruch/Fehler muss der Datenkanal sauber schließen,
            // sonst bleibt die Steuerverbindung in der Schwebe.
            let fin = c.finalize_retr_stream(src);
            let result = result?;
            fin.map_err(|e| map_ftp(&remote, e))?;
            Ok(result)
        })
        .await
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let remote = remote.to_string();
        let local = local.to_path_buf();
        let ctl = ctl.clone();
        self.with_conn(move |c| {
            let src = std::fs::File::open(&local)?;
            let total = src.metadata().ok().map(|m| m.len());
            let mut dst = c.put_with_stream(&remote).map_err(|e| map_ftp(&remote, e))?;
            let result = copy_blocking(src, &mut dst, total, &ctl);
            let fin = c.finalize_put_stream(dst);
            let result = result?;
            fin.map_err(|e| map_ftp(&remote, e))?;
            Ok(result)
        })
        .await
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        let path = path.to_string();
        self.with_conn(move |c| c.mkdir(&path).map_err(|e| map_ftp(&path, e))).await
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        let path = path.to_string();
        self.with_conn(move |c| {
            if is_dir {
                c.rmdir(&path).map_err(|e| map_ftp(&path, e))
            } else {
                c.rm(&path).map_err(|e| map_ftp(&path, e))
            }
        })
        .await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        let from = from.to_string();
        let to = to.to_string();
        self.with_conn(move |c| c.rename(&from, &to).map_err(|e| map_ftp(&from, e))).await
    }
}
