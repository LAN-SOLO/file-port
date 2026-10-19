// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod engine;
mod profiles;
mod transfers;

use serde::Serialize;

#[derive(Serialize)]
pub struct UpdateInfoDto {
    pub version: String,
    pub notes: Option<String>,
    /// Veröffentlichung als Unix-Zeit (Sekunden, UTC) — die UI formatiert in
    /// der lokalen Zeitzone. `OffsetDateTime::to_string()` („2026-10-01
    /// 12:00:00.0 +00:00:00") könnte `new Date()` nicht lesen.
    pub date: Option<i64>,
}

/// Check GitHub Releases (latest.json) for a newer version. Returns None when
/// the app is up to date; errors only on an unreachable/invalid endpoint.
#[tauri::command]
async fn check_update(app: tauri::AppHandle) -> Result<Option<UpdateInfoDto>, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => Ok(Some(UpdateInfoDto {
            version: update.version.clone(),
            notes: update.body.clone(),
            date: update.date.map(|d| d.unix_timestamp()),
        })),
        Ok(None) => Ok(None),
        Err(e) => Err(format!("Update-Prüfung fehlgeschlagen: {e}")),
    }
}

/// Download, verify (signed) and install the update in place, then relaunch.
/// User data and settings live outside the app bundle and are untouched.
#[tauri::command]
async fn install_update(app: tauri::AppHandle) -> Result<(), String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    let update = updater
        .check()
        .await
        .map_err(|e| format!("Update-Prüfung fehlgeschlagen: {e}"))?
        .ok_or("Kein Update verfügbar")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| format!("Update fehlgeschlagen: {e}"))?;
    // Der Neustart beendet laufende Downloads hart — Teildateien vorher abräumen.
    use tauri::Manager;
    app.state::<transfers::Transfers>().shutdown();
    app.restart();
}

fn main() {
    use tauri::Manager;
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(engine::Engine::new())
        .manage(transfers::Transfers::default())
        .setup(|app| {
            // Journal der Teildateien laufender Downloads; Reste eines früheren
            // Laufs (Fenster mitten im Download geschlossen, Absturz) verschwinden hier.
            if let Ok(dir) = app.path().app_config_dir() {
                let _ = std::fs::create_dir_all(&dir);
                app.state::<transfers::Transfers>()
                    .init_journal(dir.join("pending-parts.txt"));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_update,
            install_update,
            engine::fs_initial_dir,
            engine::fs_list,
            engine::fs_mkdir,
            engine::fs_remove,
            engine::fs_rename,
            profiles::profiles_list,
            profiles::profile_save,
            profiles::profile_delete,
            profiles::profile_reset_host_key,
            profiles::connect,
            profiles::disconnect,
            transfers::transfer_start,
            transfers::transfer_cancel,
        ])
        .build(tauri::generate_context!())
        .expect("error while running fileport");
    app.run(|handle, event| {
        // Letztes Fenster geschlossen → App endet: laufende Downloads abbrechen
        // und ihre versteckten Teildateien entfernen.
        if let tauri::RunEvent::Exit = event {
            handle.state::<transfers::Transfers>().shutdown();
        }
    });
}
