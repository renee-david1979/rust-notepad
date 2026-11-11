//! Tauri IPC command surface. Every handler returns `CmdResult<T>` so
//! failures arrive at the frontend as `{ message }`.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use serde::Serialize;
use serde_json::Value;
use tauri::Manager;

use crate::{
    encoding::{read_text_file, write_text_file, DetectedEncoding, EolStyle},
    error::{CmdResult, CommandError},
    persistence::PersistedState,
    plugins::{PluginInfo, PluginManager},
    search::{project_search, ProjectHit, SearchOptions},
    syntax,
    watcher::FileWatcher,
};

// ---------------------------------------------------------------------------
// Shared state
// ---------------------------------------------------------------------------

pub struct AppState {
    pub config_dir: PathBuf,
    pub plugin_dirs: Vec<PathBuf>,
    /// `Arc` snapshots are taken under the lock so plugin calls never hold
    /// it for the duration of a process round-trip.
    pub plugins: Mutex<Arc<PluginManager>>,
    pub watcher: Mutex<FileWatcher>,
}

impl AppState {
    pub fn new(config_dir: PathBuf, plugin_dirs: Vec<PathBuf>) -> Self {
        let plugins = PluginManager::discover(&plugin_dirs);
        Self {
            config_dir,
            plugin_dirs,
            plugins: Mutex::new(Arc::new(plugins)),
            watcher: Mutex::new(FileWatcher::new()),
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Plugin search paths: user config dir, bundled resources, exe dir, and
/// (in source checkouts) the repo's `plugins/` directory.
pub fn plugin_dirs(app: &tauri::AppHandle, config_dir: &std::path::Path) -> Vec<PathBuf> {
    let mut dirs = vec![config_dir.join("plugins")];
    if let Ok(resource_dir) = app.path().resource_dir() {
        dirs.push(resource_dir.join("plugins"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.join("plugins"));
        }
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        dirs.push(PathBuf::from(manifest_dir).join("..").join("plugins"));
    }
    dirs.retain(|d| d.is_dir());
    // Normalize `..`/`.` components so plugin dirs read cleanly in the UI.
    dirs = dirs
        .into_iter()
        .map(|d| d.canonicalize().unwrap_or(d))
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

// ---------------------------------------------------------------------------
// IPC payloads
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct FilePayload {
    pub path: String,
    pub title: String,
    pub content: String,
    pub encoding: String,
    pub eol: String,
    pub language: String,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavePayload {
    pub path: String,
    pub size: u64,
    pub mtime_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct PluginStatus {
    pub plugins: Vec<PluginInfo>,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Session / settings
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn load_session(state: tauri::State<AppState>) -> CmdResult<PersistedState> {
    Ok(PersistedState::load_from(&state.config_dir))
}

#[tauri::command]
pub fn save_session(state: tauri::State<AppState>, session: PersistedState) -> CmdResult<()> {
    session.save_to(&state.config_dir)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// File I/O
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn read_file(path: String) -> CmdResult<FilePayload> {
    let file_path = PathBuf::from(&path);
    let file = read_text_file(&file_path)?;
    let title = file_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("untitled")
        .to_owned();
    let language = syntax::detect_language(Some(&path), &file.content);
    Ok(FilePayload {
        path,
        title,
        content: file.content,
        encoding: file.encoding.label().to_owned(),
        eol: file.eol.label().to_owned(),
        language: language.to_owned(),
        size: file.size,
        mtime_ms: file.mtime_ms,
    })
}

#[tauri::command]
pub fn save_file(
    path: String,
    content: String,
    encoding: String,
    eol: String,
) -> CmdResult<SavePayload> {
    let file_path = PathBuf::from(&path);
    let encoding = DetectedEncoding::from_label(&encoding)?;
    let eol = EolStyle::from_label(&eol)?;
    let mtime_ms = write_text_file(&file_path, &content, encoding, eol)?;
    let size = std::fs::metadata(&file_path)
        .map(|meta| meta.len())
        .unwrap_or(0);
    Ok(SavePayload {
        path,
        size,
        mtime_ms,
    })
}

#[tauri::command]
pub fn watch_files(
    app: tauri::AppHandle,
    state: tauri::State<AppState>,
    paths: Vec<String>,
) -> CmdResult<()> {
    let files: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    lock(&state.watcher)
        .reconfigure(app, &files)
        .map_err(|err| CommandError {
            message: format!("file watcher failed: {err}"),
        })
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn find_in_files(
    root: String,
    needle: String,
    options: SearchOptions,
) -> CmdResult<Vec<ProjectHit>> {
    // The recursive walk is blocking work — keep it off the async runtime.
    let joined = tauri::async_runtime::spawn_blocking(move || {
        project_search(std::path::Path::new(&root), &needle, &options)
    })
    .await;
    match joined {
        Ok(Ok(hits)) => Ok(hits),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(CommandError {
            message: "search task failed".to_owned(),
        }),
    }
}

// ---------------------------------------------------------------------------
// Plugins
// ---------------------------------------------------------------------------

fn plugin_snapshot(state: &AppState) -> Arc<PluginManager> {
    lock(&state.plugins).clone()
}

fn plugin_status(manager: &PluginManager) -> PluginStatus {
    PluginStatus {
        plugins: manager.list(),
        warnings: manager.warnings().to_vec(),
    }
}

#[tauri::command]
pub fn list_plugins(state: tauri::State<AppState>) -> CmdResult<PluginStatus> {
    let manager = plugin_snapshot(&state);
    Ok(plugin_status(&manager))
}

#[tauri::command]
pub async fn invoke_plugin(
    state: tauri::State<'_, AppState>,
    name: String,
    method: String,
    params: Value,
) -> CmdResult<Value> {
    // Blocking process I/O runs on the blocking pool, not the async
    // runtime; only this plugin's own lock is held during the call.
    let manager = plugin_snapshot(&state);
    let joined =
        tauri::async_runtime::spawn_blocking(move || manager.invoke(&name, &method, params)).await;
    match joined {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(err)) => Err(err.into()),
        Err(_) => Err(CommandError {
            message: "plugin call task failed".to_owned(),
        }),
    }
}

#[tauri::command]
pub fn reload_plugins(state: tauri::State<AppState>) -> CmdResult<PluginStatus> {
    let old = plugin_snapshot(&state);
    old.stop_all();
    let fresh = PluginManager::discover(&state.plugin_dirs);
    let status = plugin_status(&fresh);
    *lock(&state.plugins) = Arc::new(fresh);
    Ok(status)
}
