//! Laufende Transfers: jeder Up-/Download läuft als eigener Task und
//! meldet Fortschritt und Abschluss als Events an die UI
//! (`transfer_progress`, `transfer_done`). Abbruch über CancelToken.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use fileport_core::{CancelToken, TransferCtl};
use serde::{Deserialize, Serialize};
use tauri::Emitter;
use tokio::sync::Mutex;

use crate::engine::Engine;

#[derive(Default)]
pub struct Transfers {
    /// Im Arc, damit Transfer-Tasks sich nach Abschluss selbst austragen.
    active: Arc<Mutex<HashMap<u32, CancelToken>>>,
    next_id: AtomicU32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Download,
    Upload,
}

#[derive(Clone, Serialize)]
struct ProgressEvent {
    id: u32,
    done: u64,
    total: Option<u64>,
}

#[derive(Clone, Serialize)]
struct DoneEvent {
    id: u32,
    ok: bool,
    error: Option<String>,
    bytes: u64,
    blake3: Option<String>,
    cancelled: bool,
}

/// Startet einen Transfer und gibt sofort dessen ID zurück; Fortschritt
/// und Ergebnis kommen als Events. `remote` ist der Pfad auf der
/// Gegenstelle, `local` der volle lokale Dateipfad.
#[tauri::command]
pub async fn transfer_start(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    transfers: tauri::State<'_, Transfers>,
    direction: Direction,
    conn: u32,
    remote: String,
    local: String,
) -> Result<u32, String> {
    let backend = engine.get(conn).await?;
    let id = transfers.next_id.fetch_add(1, Ordering::SeqCst) + 1;
    let cancel = CancelToken::new();
    transfers.active.lock().await.insert(id, cancel.clone());

    // Fortschritt drosseln: höchstens alle ~80 ms ein Event, plus das letzte.
    let emitter = app.clone();
    let last = StdMutex::new((Instant::now() - Duration::from_secs(1), 0u64));
    let progress = move |done: u64, total: Option<u64>| {
        let mut guard = last.lock().unwrap();
        let is_final = total.is_some_and(|t| done >= t);
        if is_final || guard.0.elapsed() >= Duration::from_millis(80) {
            *guard = (Instant::now(), done);
            let _ = emitter.emit("transfer_progress", ProgressEvent { id, done, total });
        }
    };
    let ctl = TransferCtl {
        progress: Arc::new(progress),
        cancel: cancel.clone(),
    };

    let app = app.clone();
    let transfers_map = transfers.active.clone();
    tauri::async_runtime::spawn(async move {
        let local_path = PathBuf::from(&local);
        let result = match direction {
            Direction::Download => backend.download(&remote, &local_path, &ctl).await,
            Direction::Upload => backend.upload(&local_path, &remote, &ctl).await,
        };
        transfers_map.lock().await.remove(&id);
        let event = match result {
            Ok(r) => DoneEvent {
                id,
                ok: true,
                error: None,
                bytes: r.bytes,
                blake3: Some(r.blake3),
                cancelled: false,
            },
            Err(e) => {
                let cancelled = matches!(e, fileport_core::FpError::Cancelled);
                // Abgebrochene/fehlgeschlagene Downloads hinterlassen keine halbe Datei
                if direction == Direction::Download {
                    let _ = tokio::fs::remove_file(&local_path).await;
                }
                DoneEvent {
                    id,
                    ok: false,
                    error: Some(e.to_string()),
                    bytes: 0,
                    blake3: None,
                    cancelled,
                }
            }
        };
        let _ = app.emit("transfer_done", event);
    });

    Ok(id)
}

#[tauri::command]
pub async fn transfer_cancel(
    transfers: tauri::State<'_, Transfers>,
    id: u32,
) -> Result<(), String> {
    if let Some(token) = transfers.active.lock().await.get(&id) {
        token.cancel();
    }
    Ok(())
}
