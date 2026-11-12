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

use tauri::{
    menu::{Menu, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder},
    Emitter, Manager,
};

const MENU_APP_QUIT: &str = "app-quit";
const MENU_FILE_CLOSE: &str = "file-close-window";

/// Native menu bar. The Quit item is custom (NOT `PredefinedMenuItem::quit`)
/// on purpose: tao implements no `applicationShouldTerminate`, so the stock
/// Quit lets macOS terminate the app before any prompt can run. A custom
/// item only fires a menu event, which routes ⌘Q through the frontend's
/// guarded close flow while the webview is still alive. The same applies to
/// Close Window, routed through the normal window-close flow.
///
/// Known tao limitation: Dock-Quit and system logout/shutdown also call
/// `terminate:` and bypass this menu entirely — they exit without a prompt
/// (the frontend's debounced draft autosave is the safety net there).
fn build_menu(app: &tauri::App) -> tauri::Result<Menu<tauri::Wry>> {
    let app_menu = SubmenuBuilder::new(app, "RustNotepad")
        .item(&PredefinedMenuItem::about(app, None, None)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&PredefinedMenuItem::services(app, None)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&PredefinedMenuItem::hide(app, None)?)
        .item(&PredefinedMenuItem::hide_others(app, None)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(
            &MenuItemBuilder::with_id(MENU_APP_QUIT, "Quit RustNotepad")
                .accelerator("Cmd+Q")
                .build(app)?,
        )
        .build()?;
    // No accelerator on Close Window: the app maps ⌘W to close-tab
    // (Notepad++-style) and a native accelerator would swallow it.
    let file_menu = SubmenuBuilder::new(app, "File")
        .item(&MenuItemBuilder::with_id(MENU_FILE_CLOSE, "Close Window").build(app)?)
        .build()?;
    let edit_menu = SubmenuBuilder::new(app, "Edit")
        .item(&PredefinedMenuItem::undo(app, None)?)
        .item(&PredefinedMenuItem::redo(app, None)?)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&PredefinedMenuItem::cut(app, None)?)
        .item(&PredefinedMenuItem::copy(app, None)?)
        .item(&PredefinedMenuItem::paste(app, None)?)
        .item(&PredefinedMenuItem::select_all(app, None)?)
        .build()?;
    MenuBuilder::new(app)
        .items(&[&app_menu, &file_menu, &edit_menu])
        .build()
}

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
            app.set_menu(build_menu(app)?)?;
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().0.as_str() {
            MENU_APP_QUIT => {
                if let Err(err) = app.emit("app-quit-requested", ()) {
                    eprintln!("failed to forward quit request: {err}");
                }
            }
            MENU_FILE_CLOSE => {
                // Single-window app: closing the window quits via the
                // guarded flow (deliberate HIG deviation, keeps one code
                // path). `close()` emits CloseRequested like a user click.
                if let Some(window) = app.get_webview_window("main") {
                    if let Err(err) = window.close() {
                        eprintln!("failed to close window: {err}");
                    }
                }
            }
            _ => {}
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
            commands::set_plugin_enabled,
            commands::install_plugin,
            commands::remove_plugin,
            commands::exit_app,
        ])
        .build(tauri::generate_context!())
        .expect("error while running tauri application")
        .run(|app, event| {
            // This handler ignores `code`; the runtime applies it to the
            // process exit itself. `exit_app` is the only `app.exit` caller
            // today and always uses 0 — any future caller must also set
            // EXIT_APPROVED (or accept depending on a live webview listener).
            //
            // Backstop for exit paths that bypass the native menu (see
            // build_menu): forward to the frontend's guarded close flow
            // while a webview can still answer. With no windows left
            // nothing can prompt, so let the exit proceed.
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if !commands::exit_approved() && !app.webview_windows().is_empty() {
                    api.prevent_exit();
                    if let Err(err) = app.emit("app-quit-requested", ()) {
                        eprintln!("failed to forward quit request: {err}");
                    }
                }
            }
        });
}
