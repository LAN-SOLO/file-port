use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::FpError;

pub use tokio_util::sync::CancellationToken as CancelToken;

/// Steuerung eines laufenden Transfers: Fortschritts-Callback
/// (übertragene Bytes, Gesamtgröße falls bekannt) und Abbruch-Token.
pub struct TransferCtl {
    pub progress: Box<dyn Fn(u64, Option<u64>) + Send + Sync>,
    pub cancel: CancelToken,
}

impl TransferCtl {
    /// Steuerung ohne Fortschrittsanzeige und ohne Abbruchmöglichkeit —
    /// praktisch für Tests und kleine interne Kopien.
    pub fn noop() -> Self {
        TransferCtl {
            progress: Box::new(|_, _| {}),
            cancel: CancelToken::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TransferResult {
    pub bytes: u64,
    /// BLAKE3-Prüfsumme über die übertragenen Bytes (hex).
    pub blake3: String,
}

/// Kopiert `reader` → `writer` in 64-KiB-Blöcken, meldet Fortschritt,
/// prüft das Abbruch-Token und bildet nebenbei die BLAKE3-Prüfsumme.
pub async fn copy_with_progress<R, W>(
    mut reader: R,
    mut writer: W,
    total: Option<u64>,
    ctl: &TransferCtl,
) -> Result<TransferResult, FpError>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buf = vec![0u8; 64 * 1024];
    let mut done: u64 = 0;
    let mut hasher = blake3::Hasher::new();
    loop {
        if ctl.cancel.is_cancelled() {
            return Err(FpError::Cancelled);
        }
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).await?;
        hasher.update(&buf[..n]);
        done += n as u64;
        (ctl.progress)(done, total);
    }
    writer.flush().await?;
    Ok(TransferResult {
        bytes: done,
        blake3: hasher.finalize().to_hex().to_string(),
    })
}
