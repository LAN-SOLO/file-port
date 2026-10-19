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

/// Windows: nacktes Laufwerk `C:` meint das *aktuelle Verzeichnis* des
/// Laufwerks, nicht dessen Wurzel — immer als `C:\\` behandeln.
pub(crate) fn normalize_local_dir(path: &str) -> String {
    let b = path.as_bytes();
    if cfg!(windows) && b.len() == 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
        format!("{path}\\")
    } else {
        path.to_string()
    }
}

/// Letzte Komponente eines Pfads — lokal mit `\\` und `/` (Windows), remote nur `/`.
fn last_component(path: &str, local: bool) -> &str {
    if local {
        std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
    } else {
        fileport_core::rfile_name(path)
    }
}

fn parent_of(path: &str, local: bool) -> String {
    if local {
        std::path::Path::new(path)
            .parent()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default()
    } else {
        fileport_core::rparent(path)
    }
}

/// Neuer lokaler Name (Ordner anlegen / umbenennen): dieselben Regeln wie
/// für Downloads — unter Windows keine `\\ : * ? " < > |`, keine Gerätenamen.
fn check_local_new_name(path: &str) -> Result<(), String> {
    let name = last_component(path, true);
    if crate::transfers::is_safe_local_name(name) {
        Ok(())
    } else {
        Err(format!("Der Dateiname „{name}“ ist auf diesem System nicht zulässig"))
    }
}

/// Umbenennen ohne stilles Überschreiben: `rename` ersetzt lokal (und bei
/// FTP/WebDAV/Objektspeichern oft auch remote) ein vorhandenes Ziel. Der
/// Vergleich ignoriert Groß-/Kleinschreibung (APFS/NTFS/SMB); reines
/// Umschreiben der Groß-/Kleinschreibung desselben Eintrags bleibt erlaubt.
pub(crate) async fn rename_checked(
    be: &dyn Backend,
    local: bool,
    from: &str,
    to: &str,
) -> Result<(), String> {
    if local {
        check_local_new_name(to)?;
    }
    let to_name = last_component(to, local).to_lowercase();
    let from_name = last_component(from, local).to_lowercase();
    let same_dir = parent_of(from, local) == parent_of(to, local);
    let case_only = same_dir && from_name == to_name;
    if !case_only {
        let siblings = be
            .list(&parent_of(to, local))
            .await
            .map_err(|e| e.to_string())?;
        if let Some(hit) = siblings.iter().find(|e| e.name.to_lowercase() == to_name) {
            return Err(format!("„{}“ existiert im Zielordner bereits", hit.name));
        }
    }
    be.rename(from, to).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_list(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    path: String,
) -> Result<Vec<Entry>, String> {
    let be = engine.get(conn).await?;
    let path = if conn == LOCAL_CONN { normalize_local_dir(&path) } else { path };
    be.list(&path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fs_mkdir(
    engine: tauri::State<'_, Engine>,
    conn: u32,
    path: String,
) -> Result<(), String> {
    let be = engine.get(conn).await?;
    if conn == LOCAL_CONN {
        check_local_new_name(&path)?;
    }
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
    rename_checked(be.as_ref(), conn == LOCAL_CONN, &from, &to).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn rename_never_silently_replaces() {
        let dir = tempfile::tempdir().unwrap();
        let p = |n: &str| dir.path().join(n).to_string_lossy().into_owned();
        std::fs::write(p("a.txt"), b"a").unwrap();
        std::fs::write(p("b.txt"), b"b").unwrap();
        let be = LocalBackend;

        // Ziel existiert → Fehler, beide Dateien unverändert
        let err = rename_checked(&be, true, &p("a.txt"), &p("b.txt")).await.unwrap_err();
        assert!(err.contains("existiert im Zielordner bereits"), "{err}");
        // auch bei anderer Groß-/Kleinschreibung (APFS/NTFS)
        assert!(rename_checked(&be, true, &p("a.txt"), &p("B.TXT")).await.is_err());
        assert_eq!(std::fs::read(p("b.txt")).unwrap(), b"b");

        // nur Groß-/Kleinschreibung desselben Eintrags ändern ist erlaubt
        rename_checked(&be, true, &p("a.txt"), &p("A.txt")).await.unwrap();
        // freier Name mit Umlaut und Leerzeichen
        rename_checked(&be, true, &p("A.txt"), &p("Größe neu.txt")).await.unwrap();
        assert!(std::path::Path::new(&p("Größe neu.txt")).exists());
    }

    #[test]
    fn drive_only_paths_become_roots_on_windows() {
        if cfg!(windows) {
            assert_eq!(normalize_local_dir("C:"), "C:\\");
            assert_eq!(normalize_local_dir("d:"), "d:\\");
        } else {
            assert_eq!(normalize_local_dir("C:"), "C:");
        }
        assert_eq!(normalize_local_dir("/home"), "/home");
    }

    #[test]
    fn local_new_names_follow_platform_rules() {
        let base = std::env::temp_dir();
        let ok = base.join("Neuer Ordner ä").to_string_lossy().into_owned();
        assert!(check_local_new_name(&ok).is_ok());
        if cfg!(windows) {
            for bad in ["a:b", "CON", "x?", "ende."] {
                let p = base.join(bad).to_string_lossy().into_owned();
                assert!(check_local_new_name(&p).is_err(), "{bad}");
            }
        }
    }
}
