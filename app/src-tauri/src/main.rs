#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

use tauri::Manager;

fn main() {
    let mut builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    {
        builder = builder.setup(|app| {
            if let Some(window) = app.get_webview_window("main") {
                // Real macOS vibrancy. In Tauri the webview is a child view, so the
                // effect view sits *behind* the content and the CSS blur composites on top.
                let _ = window_vibrancy::apply_vibrancy(
                    &window,
                    window_vibrancy::NSVisualEffectMaterial::UnderWindowBackground,
                    Some(window_vibrancy::NSVisualEffectState::Active),
                    Some(0.0),
                );
            }
            Ok(())
        });
    }

    builder
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