#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::scan_apps,
            commands::analyze,
            commands::default_selection,
            commands::uninstall,
            commands::export_report,
            commands::open_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PerfectUninstaller");
}