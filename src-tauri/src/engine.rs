//! Brücke zwischen fileport-core und der UI: Verbindungs-Registry
//! plus Tauri-Commands für Browsen und Dateioperationen.
//!
//! Verbindung 0 ist immer das lokale Dateisystem; Protokoll-Verbindungen
//! bekommen beim Verbinden fortlaufende IDs.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use fileport_core::{Backend, Entry, LocalBackend};
use tokio::sync::Mutex;

pub const LOCAL_CONN: u32 = 0;

pub struct Engine {
    conns: Mutex<HashMap<u32, Arc<dyn Backend>>>,
    next_id: AtomicU32,
}

impl Engine {
    pub fn new() -> Self {
        let mut conns: HashMap<u32, Arc<dyn Backend>> = HashMap::new();
        conns.insert(LOCAL_CONN, Arc::new(LocalBackend));
        Engine {
            conns: Mutex::new(conns),
            next_id: AtomicU32::new(1),
        }
    }

    pub async fn get(&self, id: u32) -> Result<Arc<dyn Backend>, String> {
        self.conns
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or_else(|| "Verbindung ist nicht mehr offen".to_string())
    }

    pub async fn insert(&self, backend: Arc<dyn Backend>) -> u32 {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        self.conns.lock().await.insert(id, backend);
        id
    }

    pub async fn remove(&self, id: u32) {
        if id != LOCAL_CONN {
            self.conns.lock().await.remove(&id);
        }
    }
}

#[tauri::command]
pub async fn fs_initial_dir(
    engine: tauri::State<'_, Engine>,
    conn: u32,
) -> Result<String, String> {
    let be = engine.get(conn).await?;
    be.initial_dir().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_list(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    path: String,
) -> Result<Vec<Entry>, String> {
    let be = engine.get(conn).await?;
    be.list(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_mkdir(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    path: String,
) -> Result<(), String> {
    let be = engine.get(conn).await?;
    be.mkdir(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_remove(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    path: String,
    is_dir: bool,
) -> Result<(), String> {
    let be = engine.get(conn).await?;
    be.remove(&path, is_dir).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_rename(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    from: String,
    to: String,
) -> Result<(), String> {
    let be = engine.get(conn).await?;
    be.rename(&from, &to).await.map_err(|e| e.to_string())
}
