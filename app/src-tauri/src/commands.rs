use std::path::PathBuf;

use pu_core::{Engine, InstalledApp, RemovalPlan, RemovalReport};
use serde::Serialize;
use tauri::ipc::Channel;

/// Progress emitted while `Engine::uninstall` walks the selection.
#[derive(Clone, Serialize)]
pub struct Progress {
    pub path: String,
    pub done: usize,
    pub total: usize,
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