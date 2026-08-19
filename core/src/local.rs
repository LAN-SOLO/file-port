use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use async_trait::async_trait;

use crate::backend::{Backend, Entry, EntryKind};
use crate::error::FpError;
use crate::transfer::{copy_with_progress, TransferCtl, TransferResult};

/// Referenz-Backend: das lokale Dateisystem. Dient zugleich als
/// Messlatte dafür, wie sich alle Protokoll-Backends verhalten müssen.
pub struct LocalBackend;

fn map_io(path: &str, e: std::io::Error) -> FpError {
    match e.kind() {
        std::io::ErrorKind::NotFound => FpError::NotFound(path.to_string()),
        std::io::ErrorKind::PermissionDenied => FpError::Denied(path.to_string()),
        _ => FpError::Io(e),
    }
}

impl LocalBackend {
    pub async fn entry_for(path: &Path) -> Result<Entry, FpError> {
        let meta = tokio::fs::symlink_metadata(path)
            .await
            .map_err(|e| map_io(&path.display().to_string(), e))?;
        let kind = if meta.file_type().is_symlink() {
            EntryKind::Symlink
        } else if meta.is_dir() {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        Ok(Entry {
            name: path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "/".to_string()),
            path: path.display().to_string(),
            kind,
            size: meta.len(),
            modified: meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64),
        })
    }
}

#[async_trait]
impl Backend for LocalBackend {
    fn label(&self) -> String {
        "local".to_string()
    }

    async fn initial_dir(&self) -> Result<String, FpError> {
        Ok(dirs_home().display().to_string())
    }

    async fn list(&self, path: &str) -> Result<Vec<Entry>, FpError> {
        let mut rd = tokio::fs::read_dir(path).await.map_err(|e| map_io(path, e))?;
        let mut out = Vec::new();
        while let Some(item) = rd.next_entry().await.map_err(|e| map_io(path, e))? {
            // Nicht lesbare Einträge (z. B. tote Symlinks) still überspringen
            if let Ok(entry) = LocalBackend::entry_for(&item.path()).await {
                out.push(entry);
            }
        }
        Ok(out)
    }

    async fn download(
        &self,
        remote: &str,
        local: &Path,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let src = tokio::fs::File::open(remote).await.map_err(|e| map_io(remote, e))?;
        let total = src.metadata().await.ok().map(|m| m.len());
        let dst = tokio::fs::File::create(local)
            .await
            .map_err(|e| map_io(&local.display().to_string(), e))?;
        copy_with_progress(src, dst, total, ctl).await
    }

    async fn upload(
        &self,
        local: &Path,
        remote: &str,
        ctl: &TransferCtl,
    ) -> Result<TransferResult, FpError> {
        let src = tokio::fs::File::open(local)
            .await
            .map_err(|e| map_io(&local.display().to_string(), e))?;
        let total = src.metadata().await.ok().map(|m| m.len());
        let dst = tokio::fs::File::create(remote).await.map_err(|e| map_io(remote, e))?;
        copy_with_progress(src, dst, total, ctl).await
    }

    async fn mkdir(&self, path: &str) -> Result<(), FpError> {
        tokio::fs::create_dir(path).await.map_err(|e| map_io(path, e))
    }

    async fn remove(&self, path: &str, is_dir: bool) -> Result<(), FpError> {
        if is_dir {
            tokio::fs::remove_dir_all(path).await.map_err(|e| map_io(path, e))
        } else {
            tokio::fs::remove_file(path).await.map_err(|e| map_io(path, e))
        }
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FpError> {
        tokio::fs::rename(from, to).await.map_err(|e| map_io(from, e))
    }
}

fn dirs_home() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip_with_checksum_and_progress() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        tokio::fs::write(&src, &data).await.unwrap();

        let be = LocalBackend;
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let seen2 = seen.clone();
        let ctl = TransferCtl {
            progress: std::sync::Arc::new(move |done, total| {
                assert_eq!(total, Some(200_000));
                seen2.store(done, std::sync::atomic::Ordering::SeqCst);
            }),
            cancel: crate::transfer::CancelToken::new(),
        };

        let dst = dir.path().join("dst.bin");
        let down = be
            .download(&src.display().to_string(), &dst, &ctl)
            .await
            .unwrap();
        assert_eq!(down.bytes, 200_000);
        assert_eq!(seen.load(std::sync::atomic::Ordering::SeqCst), 200_000);
        assert_eq!(down.blake3, blake3::hash(&data).to_hex().to_string());
        assert_eq!(tokio::fs::read(&dst).await.unwrap(), data);

        let up_dst = dir.path().join("up.bin");
        let up = be
            .upload(&dst, &up_dst.display().to_string(), &TransferCtl::noop())
            .await
            .unwrap();
        assert_eq!(up.blake3, down.blake3);
    }

    #[tokio::test]
    async fn cancel_aborts_transfer() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src.bin");
        tokio::fs::write(&src, vec![0u8; 1024]).await.unwrap();

        let ctl = TransferCtl::noop();
        ctl.cancel.cancel();
        let err = LocalBackend
            .download(&src.display().to_string(), &dir.path().join("x"), &ctl)
            .await
            .unwrap_err();
        assert!(matches!(err, FpError::Cancelled));
    }

    #[tokio::test]
    async fn list_mkdir_rename_remove() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().display().to_string();
        let be = LocalBackend;

        let sub = dir.path().join("sub").display().to_string();
        be.mkdir(&sub).await.unwrap();
        tokio::fs::write(dir.path().join("a.txt"), b"hi").await.unwrap();

        let mut names: Vec<String> = be.list(&root).await.unwrap().into_iter().map(|e| e.name).collect();
        names.sort();
        assert_eq!(names, vec!["a.txt", "sub"]);

        let renamed = dir.path().join("b.txt").display().to_string();
        be.rename(&dir.path().join("a.txt").display().to_string(), &renamed)
            .await
            .unwrap();
        be.remove(&renamed, false).await.unwrap();
        be.remove(&sub, true).await.unwrap();
        assert!(be.list(&root).await.unwrap().is_empty());

        let err = be.list(&dir.path().join("nope").display().to_string()).await.unwrap_err();
        assert!(matches!(err, FpError::NotFound(_)));
    }
}
