use std::path::PathBuf;

use pu_core::{Engine, InstalledApp, RemovalPlan, RemovalReport};
use serde::Serialize;
use tauri::ipc::Channel;

use crate::icons;

/// Progress emitted while `Engine::uninstall` walks the selection.
#[derive(Clone, Serialize)]
pub struct Progress {
    pub path: String,
    pub done: usize,
    pub total: usize,
}

/// Icon for one installed app, keyed by the app id.
///
/// `None` means "no artwork found"; the UI falls back to a letter avatar, so
/// this never fails the whole batch.
#[derive(Clone, Serialize)]
pub struct AppIcon {
    pub id: String,
    pub data_url: Option<String>,
}

/// Extracts icons for the given apps in one round-trip.
///
/// The front end requests this once per scan rather than per row, which keeps
/// a 70-app list to a single IPC call.
#[tauri::command]
pub async fn app_icons(apps: Vec<InstalledApp>) -> Vec<AppIcon> {
    tauri::async_runtime::spawn_blocking(move || {
        apps.into_iter()
            .map(|app| {
                // Try each install location until one yields artwork.
                let data_url = app
                    .install_paths
                    .iter()
                    .find_map(|p| icons::icon_data_url(p));
                AppIcon {
                    id: app.id,
                    data_url,
                }
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

#[tauri::command]
pub async fn scan_apps() -> Result<Vec<InstalledApp>, String> {
    let engine = Engine::new();
    tauri::async_runtime::spawn_blocking(move || engine.scan())
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn analyze(app: InstalledApp) -> Result<RemovalPlan, String> {
    let engine = Engine::new();
    tauri::async_runtime::spawn_blocking(move || engine.analyze(&app))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn default_selection(plan: RemovalPlan) -> Vec<String> {
    Engine::default_selection(&plan)
        .into_iter()
        .map(|p| p.display().to_string())
        .collect()
}

#[tauri::command]
pub async fn uninstall(
    plan: RemovalPlan,
    selected: Vec<PathBuf>,
    on_progress: Channel<Progress>,
) -> Result<RemovalReport, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let engine = Engine::new();
        let mut emit = |path: &std::path::Path, done: usize, total: usize| {
            let _ = on_progress.send(Progress {
                path: path.display().to_string(),
                done,
                total,
            });
        };
        engine.uninstall(&plan, &selected, &mut emit)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn export_report(plan: RemovalPlan, report: RemovalReport) -> Result<String, String> {
    let engine = Engine::new();
    let exported = engine
        .export_report(&plan, &report)
        .map_err(|e| e.to_string())?;
    Ok(exported.dir.display().to_string())
}

#[tauri::command]
pub fn open_path(path: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(target_os = "windows")]
    let program = "explorer";
    #[cfg(all(unix, not(target_os = "macos")))]
    let program = "xdg-open";

    std::process::Command::new(program)
        .arg(&path)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
}
