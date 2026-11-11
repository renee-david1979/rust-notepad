//! External file-change detection.
//!
//! Watches the parent directories of open files (non-recursively) so that
//! atomic save-and-replace edits by other programs are still seen. Matching
//! events are emitted to the frontend as `file-changed` with the file's
//! current mtime so the frontend can ignore echoes of its own saves.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::Emitter;

use crate::encoding;

#[derive(Debug, Clone, Serialize)]
pub struct FileChangedPayload {
    pub path: String,
    pub mtime_ms: u64,
}

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    /// canonical path -> the path as the user opened it (what we emit).
    tracked: Arc<Mutex<HashMap<PathBuf, PathBuf>>>,
}

impl Default for FileWatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            tracked: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Re-target the watcher on exactly these files (canonicalized).
    /// Passing an empty list stops watching.
    pub fn reconfigure(&mut self, app: tauri::AppHandle, files: &[PathBuf]) -> notify::Result<()> {
        // Dropping the old watcher unwatches everything.
        self.watcher = None;

        let mut tracked: HashMap<PathBuf, PathBuf> = HashMap::new();
        for file in files {
            if let Ok(canonical) = file.canonicalize() {
                tracked.insert(canonical, file.clone());
            }
        }

        let mut dirs: Vec<PathBuf> = tracked
            .keys()
            .filter_map(|f| f.parent().map(Path::to_path_buf))
            .collect();
        dirs.sort();
        dirs.dedup();

        {
            let mut guard = self
                .tracked
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *guard = tracked;
        }

        if dirs.is_empty() {
            return Ok(());
        }

        let tracked_for_cb = Arc::clone(&self.tracked);
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            let Ok(event) = result else {
                return;
            };
            if !matches!(
                event.kind,
                EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
            ) {
                return;
            }
            for path in event.paths {
                let original = {
                    let guard = tracked_for_cb
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());
                    let canonical = path.canonicalize().ok();
                    canonical
                        .and_then(|c| guard.get(&c).cloned())
                        .or_else(|| guard.values().find(|orig| **orig == path).cloned())
                        .or_else(|| {
                            // Removals: canonicalize fails on a deleted
                            // path — match by parent + file name.
                            let name = path.file_name();
                            let parent = path.parent();
                            guard
                                .values()
                                .find(|orig| orig.file_name() == name && orig.parent() == parent)
                                .cloned()
                        })
                };
                let Some(original) = original else {
                    continue;
                };
                let mtime_ms = std::fs::metadata(&path)
                    .ok()
                    .map(|meta| encoding::mtime_ms(&meta))
                    .unwrap_or(0);
                let payload = FileChangedPayload {
                    path: original.display().to_string(),
                    mtime_ms,
                };
                let _ = app.emit("file-changed", payload);
            }
        })?;

        for dir in dirs {
            // Non-recursive: only the immediate directory of each open file.
            watcher.watch(&dir, RecursiveMode::NonRecursive)?;
        }
        self.watcher = Some(watcher);
        Ok(())
    }
}
