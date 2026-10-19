//! Laufende Transfers: jeder Up-/Download läuft als eigener Task und
//! meldet Fortschritt und Abschluss als Events an die UI
//! (`transfer_progress`, `transfer_done`). Abbruch über CancelToken.
//!
//! Downloads landen erst in einer versteckten Teildatei neben dem Ziel
//! (`.<name>.fileport-part`) und ersetzen das Ziel erst nach vollständigem
//! Erfolg — eine bestehende Datei bleibt bei Abbruch/Fehler unangetastet.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
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
    /// Abbrüche, die eintreffen, bevor `transfer_start` den Transfer
    /// registriert hat (beide Commands laufen nebenläufig) — sonst ginge
    /// ein früher Klick auf „Abbrechen" verloren.
    early_cancel: Mutex<HashSet<u32>>,
}

/// Prüft den lokalen Zielpfad eines Downloads. Der Dateiname stammt vom
/// Server — ein bösartiger Server könnte `..\\..\\Autostart\\x.exe` (Windows)
/// o. Ä. als Namen liefern. Erlaubt ist nur ein absoluter Pfad ohne `..`,
/// dessen letzter Teil ein gewöhnlicher Dateiname ist.
pub(crate) fn validate_local_target(local: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(local);
    if local.contains('\0') || !path.is_absolute() {
        return Err(format!("Ungültiger lokaler Zielpfad: {local}"));
    }
    if path.components().any(|c| matches!(c, Component::ParentDir | Component::CurDir)) {
        return Err(format!("Ungültiger lokaler Zielpfad (enthält „..“): {local}"));
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("Ungültiger lokaler Zielpfad: {local}"))?;
    if !is_safe_local_name(name) {
        return Err(format!(
            "Der Dateiname „{name}“ ist auf diesem System nicht zulässig — bitte auf dem Server umbenennen"
        ));
    }
    Ok(path)
}

/// Dateiname, den das lokale System ohne Überraschungen anlegen kann.
pub(crate) fn is_safe_local_name(name: &str) -> bool {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') {
        return false;
    }
    if cfg!(windows) {
        // Windows: keine Pfadtrenner, kein ADS-Doppelpunkt, keine reservierten
        // Zeichen/Gerätenamen, kein Punkt/Leerzeichen am Ende.
        if name.chars().any(|c| "\\:*?\"<>|".contains(c) || (c as u32) < 32) {
            return false;
        }
        if name.ends_with('.') || name.ends_with(' ') {
            return false;
        }
        let stem = name.split('.').next().unwrap_or("").to_uppercase();
        const RESERVED: [&str; 32] = [
            "CON", "PRN", "AUX", "NUL", "CONIN$", "CONOUT$", "COM0", "COM1", "COM2", "COM3",
            "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "COM¹", "COM²", "COM³", "LPT0",
            "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9", "LPT¹",
            "LPT²", "LPT³",
        ];
        if RESERVED.contains(&stem.trim_end()) {
            return false;
        }
    }
    true
}

/// Versteckte Teildatei neben dem Ziel: `/a/b.txt` → `/a/.b.txt.fileport-part`.
pub(crate) fn part_path(target: &Path) -> PathBuf {
    let name = target
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    // Sehr lange Namen: Präfix + Suffix sprengten die 255-Byte-Grenze der
    // Dateisysteme — dann eine kurze, eindeutige Teildatei verwenden.
    if name.len() > 200 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        name.hash(&mut h);
        return target.with_file_name(format!(".fileport-{:016x}.fileport-part", h.finish()));
    }
    target.with_file_name(format!(".{name}.fileport-part"))
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

/// Lokales Ziel eines Downloads: Zielordner + Dateiname vom Server.
/// Der Name wird hier (nicht nur in der UI) geprüft — ein Server-Name wie
/// `a\\b` (unter Linux legal) adressierte unter Windows sonst einen
/// Unterordner, `a:b` einen NTFS-Datenstrom, `CON.txt` ein Gerät.
pub(crate) fn download_target(dir: &str, name: &str) -> Result<PathBuf, String> {
    if !is_safe_local_name(name) {
        return Err(format!(
            "Der Dateiname „{name}“ ist auf diesem System nicht zulässig — bitte auf dem Server umbenennen"
        ));
    }
    let dir_path = PathBuf::from(crate::engine::normalize_local_dir(dir));
    if dir.contains('\0') || !dir_path.is_absolute() {
        return Err(format!("Ungültiger lokaler Zielpfad: {dir}"));
    }
    if dir_path
        .components()
        .any(|c| matches!(c, Component::ParentDir | Component::CurDir))
    {
        return Err(format!("Ungültiger lokaler Zielpfad (enthält „..“): {dir}"));
    }
    let target = dir_path.join(name);
    // Gegenprobe: genau eine Komponente angehängt, Elternordner unverändert.
    if target.parent() != Some(dir_path.as_path()) {
        return Err(format!("Ungültiger lokaler Zielpfad: {dir}"));
    }
    validate_local_target(&target.to_string_lossy())
}

/// Name eines vorhandenen Eintrags auf der Gegenstelle, der denselben Namen
/// wie `remote` trägt — ohne Groß-/Kleinschreibung (SMB, NAS, S3-Gateways
/// auf Windows). `None`, wenn der Name frei ist.
pub(crate) async fn remote_existing(
    backend: &dyn fileport_core::Backend,
    remote: &str,
) -> Result<Option<String>, String> {
    let name = fileport_core::rfile_name(remote).to_lowercase();
    let siblings = backend
        .list(&fileport_core::rparent(remote))
        .await
        .map_err(|e| e.to_string())?;
    Ok(siblings
        .into_iter()
        .find(|e| e.name.to_lowercase() == name)
        .map(|e| e.name))
}

/// Startet einen Transfer; Fortschritt und Ergebnis kommen als Events.
/// `id` vergibt die UI (Schlüssel des Warteschlangen-Eintrags) — so kann
/// kein Event vor der Antwort dieses Commands „verloren" gehen. `remote`
/// ist der Pfad auf der Gegenstelle. Upload: `local` ist die Quelldatei.
/// Download: `local` ist der Zielordner und `name` der Dateiname. Ohne
/// `overwrite` bricht ein Transfer (beide Richtungen) ab, wenn das Ziel
/// schon existiert — die Rückfrage der UI beruht nur auf der zuletzt
/// geladenen Ansicht, die veraltet sein kann.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn transfer_start(
    app: tauri::AppHandle,
    engine: tauri::State<'_, Engine>,
    transfers: tauri::State<'_, Transfers>,
    id: u32,
    direction: Direction,
    conn: u32,
    remote: String,
    local: String,
    name: Option<String>,
    overwrite: Option<bool>,
) -> Result<u32, String> {
    let backend = engine.get(conn).await?;
    if remote.trim().is_empty() || remote.contains('\0') {
        return Err("Ungültiger Pfad auf der Gegenstelle".into());
    }
    let local_path = match direction {
        Direction::Download => {
            let name = name.unwrap_or_else(|| fileport_core::rfile_name(&remote).to_string());
            let target = download_target(&local, &name)?;
            if !overwrite.unwrap_or(false) && tokio::fs::symlink_metadata(&target).await.is_ok() {
                return Err(format!("„{name}“ existiert im Zielordner bereits"));
            }
            target
        }
        Direction::Upload => {
            let source = validate_local_target(&local)?;
            if !overwrite.unwrap_or(false) {
                // Die Rückfrage der UI beruht auf der zuletzt geladenen
                // Ansicht — hier gegen den aktuellen Stand der Gegenstelle
                // prüfen (ohne Groß-/Kleinschreibung: SMB, viele NAS).
                if let Some(hit) = remote_existing(backend.as_ref(), &remote).await? {
                    return Err(format!("„{hit}“ existiert im Zielordner bereits"));
                }
            }
            source
        }
    };
    let cancel = CancelToken::new();
    {
        let mut active = transfers.active.lock().await;
        if active.contains_key(&id) {
            return Err(format!("Transfer {id} läuft bereits"));
        }
        active.insert(id, cancel.clone());
        if transfers.early_cancel.lock().await.remove(&id) {
            cancel.cancel();
        }
    }

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
        let result = match direction {
            Direction::Download => {
                let part = part_path(&local_path);
                let r = backend.download(&remote, &part, &ctl).await;
                match r {
                    // Mit Wiederholung: unter Windows hält oft ein Virenscanner
                    // die frische Teildatei kurz offen (Sharing Violation).
                    Ok(done) => match fileport_core::rename_with_retry(&part, &local_path).await {
                        Ok(()) => Ok(done),
                        Err(e) => {
                            let _ = tokio::fs::remove_file(&part).await;
                            Err(fileport_core::FpError::Io(e))
                        }
                    },
                    Err(e) => {
                        // Abgebrochene/fehlgeschlagene Downloads hinterlassen
                        // keine halbe Datei — und das alte Ziel bleibt erhalten.
                        let _ = tokio::fs::remove_file(&part).await;
                        Err(e)
                    }
                }
            }
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
    let active = transfers.active.lock().await;
    match active.get(&id) {
        Some(token) => token.cancel(),
        // Noch nicht gestartet: vormerken, transfer_start bricht dann sofort ab.
        None => {
            transfers.early_cancel.lock().await.insert(id);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(rel: &str) -> String {
        std::env::temp_dir().join(rel).to_string_lossy().into_owned()
    }

    #[test]
    fn accepts_plain_absolute_targets() {
        let p = validate_local_target(&abs("datei ä.txt")).unwrap();
        assert_eq!(p.file_name().unwrap(), "datei ä.txt");
    }

    #[test]
    fn rejects_traversal_and_relative_targets() {
        assert!(validate_local_target("relativ.txt").is_err());
        let sep = std::path::MAIN_SEPARATOR;
        assert!(validate_local_target(&abs(&format!("..{sep}..{sep}evil.txt"))).is_err());
        assert!(validate_local_target(&abs(&format!("a{sep}..{sep}evil.txt"))).is_err());
        assert!(validate_local_target(&abs("nul\0byte")).is_err());
    }

    #[test]
    fn windows_specific_names() {
        assert!(!is_safe_local_name(".."));
        assert!(!is_safe_local_name(""));
        assert!(!is_safe_local_name("a/b"));
        if cfg!(windows) {
            assert!(!is_safe_local_name("a\\b"));
            assert!(!is_safe_local_name("datei.txt:stream"));
            assert!(!is_safe_local_name("CON"));
            assert!(!is_safe_local_name("com1.txt"));
            assert!(!is_safe_local_name("ende."));
        } else {
            // Unix: Backslash und Doppelpunkt sind gewöhnliche Namenszeichen
            assert!(is_safe_local_name("a\\b"));
            assert!(is_safe_local_name("datei:1"));
        }
        assert!(is_safe_local_name("Bericht 2026 (final).pdf"));
    }

    #[test]
    fn download_target_validates_server_names_in_backend() {
        let dir = std::env::temp_dir().to_string_lossy().into_owned();
        let ok = download_target(&dir, "Bericht März 2026.pdf").unwrap();
        assert_eq!(ok.file_name().unwrap(), "Bericht März 2026.pdf");
        assert_eq!(ok.parent().unwrap(), std::path::Path::new(&dir));
        assert!(download_target(&dir, "..").is_err());
        assert!(download_target(&dir, "a/b.txt").is_err());
        assert!(download_target(&dir, "").is_err());
        assert!(download_target("relativ", "x.txt").is_err());
        if cfg!(windows) {
            for bad in ["a\\b.txt", "a:b.txt", "CON.txt", "nul", "x?.txt", "ende.", "lpt1.log"] {
                assert!(download_target(&dir, bad).is_err(), "{bad}");
            }
        } else {
            // macOS/Linux: Backslash und Doppelpunkt sind normale Namenszeichen
            assert!(download_target(&dir, "a\\b.txt").is_ok());
        }
    }

    #[tokio::test]
    async fn upload_target_collision_ignores_case() {
        let dir = tempfile::tempdir().unwrap();
        // Remote-Pfade sind „/“-Pfade — unter macOS/Linux deckt das LocalBackend das ab.
        if cfg!(windows) {
            return;
        }
        std::fs::write(dir.path().join("Report.pdf"), b"x").unwrap();
        let be = fileport_core::LocalBackend;
        let base = dir.path().to_string_lossy().into_owned();
        let hit = remote_existing(&be, &format!("{base}/report.PDF")).await.unwrap();
        assert_eq!(hit.as_deref(), Some("Report.pdf"));
        assert!(remote_existing(&be, &format!("{base}/frei ä.pdf")).await.unwrap().is_none());
    }

    #[test]
    fn part_file_sits_next_to_target() {
        let target = std::env::temp_dir().join("x.bin");
        let part = part_path(&target);
        assert_eq!(part.parent(), target.parent());
        assert_eq!(part.file_name().unwrap(), ".x.bin.fileport-part");

        let long = std::env::temp_dir().join(format!("{}.bin", "ä".repeat(120)));
        let part = part_path(&long);
        assert_eq!(part.parent(), long.parent());
        assert!(part.file_name().unwrap().len() < 64);
    }
}
