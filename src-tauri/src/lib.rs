//! RustNotepad: a Notepad++-style editor — Tauri shell + Monaco editor +
//! process-based plugin host.

pub mod commands;
pub mod encoding;
pub mod error;
pub mod persistence;
pub mod plugins;
pub mod search;
pub mod syntax;
pub mod watcher;

use std::path::PathBuf;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .unwrap_or_else(|_| PathBuf::from("."));
            std::fs::create_dir_all(&config_dir)?;
            let dirs = commands::plugin_dirs(app.handle(), &config_dir);
            app.manage(commands::AppState::new(config_dir, dirs));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::load_session,
            commands::save_session,
            commands::read_file,
            commands::save_file,
            commands::watch_files,
            commands::find_in_files,
            commands::list_plugins,
            commands::invoke_plugin,
            commands::reload_plugins,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
