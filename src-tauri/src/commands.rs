//! Tauri IPC command surface. Every handler returns `CmdResult<T>` so
//! failures arrive at the frontend as `{ message }` — except `exit_app`,
//! whose only operation is infallible.

use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, MutexGuard,
    },
};

use serde::Serialize;
use serde_json::Value;
use tauri::Manager;

use crate::{
    encoding::{read_text_file, write_text_file, DetectedEncoding, EolStyle},
    error::{io_error, AppError, AppResult, CmdResult, CommandError},
    persistence::PersistedState,
    plugins::{PluginConfig, PluginInfo, PluginManager},
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
        let disabled = super::plugins::PluginConfig::load(&config_dir).disabled_set();
        let plugins = PluginManager::discover(&plugin_dirs, &disabled);
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

/// Plugin search paths. The user's config plugins dir is ALWAYS first —
/// it is the install/remove target (`PluginManager::user_dir`) — followed
/// by bundled resources, exe dir, and (in source checkouts) the repo's
/// `plugins/` directory. The user dir is kept even if it does not exist
/// yet, so installs have a stable destination.
pub fn plugin_dirs(app: &tauri::AppHandle, config_dir: &std::path::Path) -> Vec<PathBuf> {
    let user_dir = config_dir.join("plugins");
    let mut rest: Vec<PathBuf> = Vec::new();
    if let Ok(resource_dir) = app.path().resource_dir() {
        rest.push(resource_dir.join("plugins"));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            rest.push(parent.join("plugins"));
        }
    }
    if let Some(manifest_dir) = option_env!("CARGO_MANIFEST_DIR") {
        rest.push(PathBuf::from(manifest_dir).join("..").join("plugins"));
    }
    rest.retain(|d| d.is_dir());
    // Normalize `..`/`.` components so plugin dirs read cleanly in the UI.
    rest = rest
        .into_iter()
        .map(|d| d.canonicalize().unwrap_or(d))
        .collect();
    rest.sort();
    rest.dedup();
    let user_dir = user_dir.canonicalize().unwrap_or(user_dir);
    let mut dirs = vec![user_dir];
    for dir in rest {
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
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
// App lifecycle
// ---------------------------------------------------------------------------

/// Set only by `exit_app`, i.e. after the frontend's guarded close flow got
/// the user's approval. Menu/window exit requests (⌘Q / Quit menu via the
/// custom menu item in `lib.rs`, window close, and the `ExitRequested`
/// backstop) are routed to that flow first; only approved exits proceed.
/// Known tao limitation: Dock-Quit / system logout bypass the flow.
static EXIT_APPROVED: AtomicBool = AtomicBool::new(false);

/// True once the frontend's guarded close flow approved exiting (reads
/// `EXIT_APPROVED`; see its comment for the interception design).
pub fn exit_approved() -> bool {
    EXIT_APPROVED.load(Ordering::SeqCst)
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    EXIT_APPROVED.store(true, Ordering::SeqCst);
    app.exit(0);
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

#[tauri::command(async)]
pub fn reload_plugins(state: tauri::State<AppState>) -> CmdResult<PluginStatus> {
    let old = plugin_snapshot(&state);
    old.stop_all();
    let disabled = PluginConfig::load(&state.config_dir).disabled_set();
    let fresh = PluginManager::discover(&state.plugin_dirs, &disabled);
    let status = plugin_status(&fresh);
    *lock(&state.plugins) = Arc::new(fresh);
    Ok(status)
}

/// Enable or disable a plugin at runtime; the choice persists across
/// restarts (`plugins.json` in the config dir). The config is persisted
/// first: a failed save must never leave memory and disk diverged.
#[tauri::command]
pub fn set_plugin_enabled(
    state: tauri::State<AppState>,
    name: String,
    enabled: bool,
) -> CmdResult<PluginStatus> {
    let mut config = PluginConfig::load(&state.config_dir);
    config.disabled.retain(|n| n != &name);
    if !enabled {
        config.disabled.push(name.clone());
    }
    config.save(&state.config_dir)?;
    let manager = plugin_snapshot(&state);
    manager.set_enabled(&name, enabled)?;
    Ok(plugin_status(&manager))
}

fn copy_dir_all(src: &Path, dst: &Path) -> AppResult<()> {
    std::fs::create_dir_all(dst).map_err(|err| io_error(dst, err))?;
    for entry in std::fs::read_dir(src).map_err(|err| io_error(src, err))? {
        let entry = entry.map_err(|err| io_error(src, err))?;
        let file_type = entry.file_type().map_err(|err| io_error(src, err))?;
        let target = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else if file_type.is_file() {
            std::fs::copy(entry.path(), &target).map_err(|err| io_error(&target, err))?;
        }
        // Symlinks and special files are skipped deliberately: a plugin is
        // plain data (manifest + scripts) and must be self-contained.
    }
    Ok(())
}

/// Copy `source` (a plugin folder) into `user_dir` under its folder name.
/// The copy stages into a temporary sibling and is renamed into place, so
/// a failed copy never leaves a half-install and a symlink appearing under
/// the target name cannot redirect writes outside the plugin dir.
fn install_plugin_dir(user_dir: &Path, source: &Path) -> AppResult<PathBuf> {
    let folder = source
        .file_name()
        .ok_or_else(|| AppError::Plugin("plugin source has no folder name".to_owned()))?;
    let target = user_dir.join(folder);
    if target.exists() {
        return Err(AppError::Plugin(format!(
            "{} is already installed — remove it first",
            folder.to_string_lossy()
        )));
    }
    // Copying the plugin dir (or one of its ancestors) into itself would
    // recurse without bound and shred sibling plugins along the way.
    let user_dir = user_dir
        .canonicalize()
        .unwrap_or_else(|_| user_dir.to_owned());
    let source = source.canonicalize().unwrap_or_else(|_| source.to_owned());
    if user_dir.starts_with(&source) {
        return Err(AppError::Plugin(format!(
            "{} contains the plugin directory itself",
            source.display()
        )));
    }
    let staging = user_dir.join(format!(
        ".install-{}-{}",
        std::process::id(),
        folder.to_string_lossy()
    ));
    if let Err(err) = copy_dir_all(&source, &staging) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(err);
    }
    if let Err(err) = std::fs::rename(&staging, &target) {
        let _ = std::fs::remove_dir_all(&staging);
        return Err(io_error(&target, err));
    }
    Ok(target)
}

/// Delete an installed plugin directory. Only directories strictly inside
/// `user_dir` may be removed — bundled plugins and the user dir itself are
/// refused so removal can never escape the install area.
fn remove_plugin_dir(user_dir: &Path, dir: &Path) -> AppResult<()> {
    let user_dir = user_dir
        .canonicalize()
        .unwrap_or_else(|_| user_dir.to_owned());
    let dir = dir.canonicalize().map_err(|err| io_error(dir, err))?;
    if dir == user_dir || !dir.starts_with(&user_dir) {
        return Err(AppError::Plugin(
            "not an installed plugin — disable it instead of removing".to_owned(),
        ));
    }
    std::fs::remove_dir_all(&dir).map_err(|err| io_error(&dir, err))
}

/// Install a plugin by copying its directory into the user plugin dir.
/// The source must contain a `plugin.json`; existing installs are never
/// overwritten (remove first). Runs off the main thread (recursive fs I/O).
#[tauri::command(async)]
pub fn install_plugin(state: tauri::State<AppState>, source: String) -> CmdResult<PluginStatus> {
    let source = PathBuf::from(&source);
    let source = source
        .canonicalize()
        .map_err(|err| io_error(&source, err))?;
    if !source.join("plugin.json").is_file() {
        return Err(
            AppError::Plugin(format!("{} does not contain plugin.json", source.display())).into(),
        );
    }
    let manager = plugin_snapshot(&state);
    install_plugin_dir(manager.user_dir(), &source)?;
    reload_after_change(&state)
}

/// Remove an installed plugin (only plugins inside the user plugin dir).
/// Runs off the main thread (recursive fs I/O).
#[tauri::command(async)]
pub fn remove_plugin(state: tauri::State<AppState>, name: String) -> CmdResult<PluginStatus> {
    let manager = plugin_snapshot(&state);
    manager.stop(&name)?;
    let dir = manager.plugin_dir(&name)?;
    remove_plugin_dir(manager.user_dir(), &dir)?;
    // Scrub the disabled list so a later reinstall of the same plugin
    // starts enabled instead of silently inheriting a stale "disabled".
    let mut config = PluginConfig::load(&state.config_dir);
    if config.disabled.iter().any(|n| n == &name) {
        config.disabled.retain(|n| n != &name);
        config.save(&state.config_dir)?;
    }
    reload_after_change(&state)
}

/// Re-discover after install/remove and swap the shared manager.
fn reload_after_change(state: &AppState) -> CmdResult<PluginStatus> {
    let old = plugin_snapshot(state);
    old.stop_all();
    let disabled = PluginConfig::load(&state.config_dir).disabled_set();
    let fresh = PluginManager::discover(&state.plugin_dirs, &disabled);
    let status = plugin_status(&fresh);
    *lock(&state.plugins) = Arc::new(fresh);
    Ok(status)
}

#[cfg(test)]
mod tests {
    use super::{copy_dir_all, install_plugin_dir, remove_plugin_dir};
    use crate::error::AppError;

    #[test]
    fn copy_dir_all_copies_nested_files() {
        let src = tempfile::tempdir().expect("tempdir");
        let dst = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(src.path().join("inner")).expect("mkdir");
        std::fs::write(src.path().join("plugin.json"), "{}").expect("write");
        std::fs::write(src.path().join("inner").join("main.py"), "x").expect("write");

        let target = dst.path().join("copied");
        copy_dir_all(src.path(), &target).expect("copy");
        assert_eq!(
            std::fs::read_to_string(target.join("plugin.json")).expect("read"),
            "{}"
        );
        assert_eq!(
            std::fs::read_to_string(target.join("inner").join("main.py")).expect("read"),
            "x"
        );
    }

    #[cfg(unix)]
    #[test]
    fn copy_dir_all_skips_symlinks() {
        let src = tempfile::tempdir().expect("tempdir");
        let dst = tempfile::tempdir().expect("tempdir");
        std::fs::write(src.path().join("real.txt"), "x").expect("write");
        std::os::unix::fs::symlink(src.path().join("real.txt"), src.path().join("link.txt"))
            .expect("symlink");

        let target = dst.path().join("copied");
        copy_dir_all(src.path(), &target).expect("copy");
        assert!(target.join("real.txt").is_file());
        assert!(
            !target.join("link.txt").exists(),
            "symlinks must not be copied"
        );
    }

    #[test]
    fn install_copies_and_refuses_overwrite_and_self_copy() {
        let user = tempfile::tempdir().expect("tempdir");
        let src = tempfile::tempdir().expect("tempdir");
        std::fs::write(src.path().join("plugin.json"), "{}").expect("write");

        let installed = install_plugin_dir(user.path(), src.path()).expect("install");
        assert!(installed.join("plugin.json").is_file());

        // Installing the same folder again must refuse and change nothing.
        let err = install_plugin_dir(user.path(), src.path()).expect_err("overwrite");
        assert!(matches!(err, AppError::Plugin(_)));

        // Installing the user plugin dir into itself (or an ancestor of it)
        // would recurse without bound — must be refused outright.
        let err = install_plugin_dir(user.path(), user.path()).expect_err("self copy");
        assert!(matches!(err, AppError::Plugin(_)));
        let err = install_plugin_dir(user.path(), user.path().parent().expect("parent"))
            .expect_err("ancestor");
        assert!(matches!(err, AppError::Plugin(_)));
    }

    #[cfg(unix)]
    #[test]
    fn install_cleans_up_after_a_failed_copy() {
        use std::os::unix::fs::PermissionsExt;

        let user = tempfile::tempdir().expect("tempdir");
        let src = tempfile::tempdir().expect("tempdir");
        std::fs::write(src.path().join("plugin.json"), "{}").expect("write");
        let locked = src.path().join("locked");
        std::fs::write(&locked, "x").expect("write");
        std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o000)).expect("chmod");
        if std::fs::read(&locked).is_ok() {
            // Running as root: permissions are not enforced, skip.
            return;
        }

        let err = install_plugin_dir(user.path(), src.path()).expect_err("copy must fail");
        assert!(matches!(err, AppError::PermissionDenied(_)));
        let leftovers: Vec<_> = std::fs::read_dir(user.path())
            .expect("read")
            .map(|e| e.expect("entry").file_name())
            .collect();
        assert!(
            leftovers.is_empty(),
            "failed install must leave nothing behind: {leftovers:?}"
        );
    }

    #[test]
    fn remove_deletes_installed_dir_but_refuses_everything_else() {
        let user = tempfile::tempdir().expect("tempdir");
        let bundled = tempfile::tempdir().expect("tempdir");
        let installed = user.path().join("installed");
        std::fs::create_dir_all(&installed).expect("mkdir");
        std::fs::write(installed.join("plugin.json"), "{}").expect("write");
        std::fs::create_dir_all(bundled.path().join("bundled")).expect("mkdir");

        // A bundled plugin (outside the user dir) must survive.
        let err =
            remove_plugin_dir(user.path(), &bundled.path().join("bundled")).expect_err("bundled");
        assert!(matches!(err, AppError::Plugin(_)));
        assert!(bundled.path().join("bundled").exists());

        // The user dir itself is never deleted wholesale.
        let err = remove_plugin_dir(user.path(), user.path()).expect_err("user dir");
        assert!(matches!(err, AppError::Plugin(_)));
        assert!(user.path().is_dir());

        // The installed plugin is removed.
        remove_plugin_dir(user.path(), &installed).expect("remove");
        assert!(!installed.exists());
    }
}
