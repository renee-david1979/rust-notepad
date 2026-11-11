//! Plugin discovery and lifecycle across plugin directories.
//!
//! Discovery: each configured directory may itself contain `plugin.json`,
//! and every subdirectory containing `plugin.json` is a plugin. Invalid
//! entries are skipped and reported as warnings (surfaced in the UI).
//!
//! Each plugin host sits behind its own `Mutex`, so one slow or hung plugin
//! call never blocks listing, reload, or other plugins.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::host::{load_manifest, PluginHost, PluginManifest, MAX_TIMEOUT};

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub dir: String,
    pub running: bool,
}

pub struct PluginEntry {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
    pub host: Arc<Mutex<PluginHost>>,
}

#[derive(Default)]
pub struct PluginManager {
    entries: Vec<PluginEntry>,
    warnings: Vec<String>,
}

fn lock_host(host: &Arc<Mutex<PluginHost>>) -> std::sync::MutexGuard<'_, PluginHost> {
    host.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl PluginManager {
    /// Discover plugins in `dirs` (and their immediate subdirectories).
    pub fn discover(dirs: &[PathBuf]) -> Self {
        let mut manager = PluginManager::default();
        for dir in dirs {
            if !dir.is_dir() {
                continue;
            }
            manager.load_dir(dir);
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    manager.load_dir(&path);
                }
            }
        }
        manager.entries.sort_by(|a, b| {
            a.manifest
                .name
                .to_lowercase()
                .cmp(&b.manifest.name.to_lowercase())
        });
        manager
    }

    fn load_dir(&mut self, dir: &Path) {
        if !dir.join("plugin.json").is_file() {
            return;
        }
        match load_manifest(dir) {
            Ok(manifest) => {
                if self
                    .entries
                    .iter()
                    .any(|e| e.manifest.name == manifest.name)
                {
                    self.warnings.push(format!(
                        "duplicate plugin name '{}' in {} (skipped)",
                        manifest.name,
                        dir.display()
                    ));
                    return;
                }
                if manifest.timeout_clamped() {
                    self.warnings.push(format!(
                        "plugin '{}' requests timeout_ms={} — clamped to {} ms",
                        manifest.name,
                        manifest.timeout_ms,
                        MAX_TIMEOUT.as_millis()
                    ));
                }
                self.entries.push(PluginEntry {
                    host: Arc::new(Mutex::new(PluginHost::new(
                        manifest.clone(),
                        dir.to_path_buf(),
                    ))),
                    manifest,
                    dir: dir.to_path_buf(),
                });
            }
            Err(err) => {
                self.warnings.push(format!("{}: {err}", dir.display()));
            }
        }
    }

    pub fn list(&self) -> Vec<PluginInfo> {
        self.entries
            .iter()
            .map(|entry| {
                let mut host = lock_host(&entry.host);
                PluginInfo {
                    name: entry.manifest.name.clone(),
                    version: entry.manifest.version.clone(),
                    description: entry.manifest.description.clone(),
                    capabilities: entry.manifest.capabilities.clone(),
                    dir: entry.dir.display().to_string(),
                    running: host.is_running(),
                }
            })
            .collect()
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Invoke a capability on a named plugin (lazy process start). Only
    /// this plugin's lock is held for the duration of the call.
    pub fn invoke(&self, name: &str, method: &str, params: Value) -> AppResult<Value> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.manifest.name == name)
            .ok_or_else(|| AppError::Plugin(format!("no plugin named '{name}'")))?;
        let mut host = lock_host(&entry.host);
        host.invoke(method, params)
    }

    pub fn stop_all(&self) {
        for entry in &self.entries {
            lock_host(&entry.host).stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_plugin(dir: &Path, name: &str, caps: &[&str]) {
        std::fs::create_dir_all(dir).expect("mkdir");
        let manifest = serde_json::json!({
            "name": name,
            "version": "1.0.0",
            "command": ["python3", "${pluginDir}/main.py"],
            "capabilities": caps,
        });
        std::fs::write(
            dir.join("plugin.json"),
            serde_json::to_string(&manifest).expect("ser"),
        )
        .expect("write manifest");
        std::fs::write(dir.join("main.py"), "print('noop')").expect("write script");
    }

    #[test]
    fn discovers_nested_and_flat_plugins() {
        let root = tempfile::tempdir().expect("tempdir");
        write_plugin(&root.path().join("alpha"), "alpha", &["a"]);
        write_plugin(root.path(), "flat", &["b"]); // plugin.json directly in root

        let manager = PluginManager::discover(&[root.path().to_path_buf()]);
        let names: Vec<String> = manager.list().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["alpha", "flat"]);
    }

    #[test]
    fn invalid_manifest_becomes_warning_not_panic() {
        let root = tempfile::tempdir().expect("tempdir");
        let bad = root.path().join("bad");
        std::fs::create_dir_all(&bad).expect("mkdir");
        std::fs::write(bad.join("plugin.json"), "{not json").expect("write");

        let manager = PluginManager::discover(&[root.path().to_path_buf()]);
        assert!(manager.entries.is_empty());
        assert_eq!(manager.warnings().len(), 1);
    }

    #[test]
    fn duplicate_names_are_skipped_with_warning() {
        let root = tempfile::tempdir().expect("tempdir");
        write_plugin(&root.path().join("one"), "same", &["a"]);
        write_plugin(&root.path().join("two"), "same", &["a"]);

        let manager = PluginManager::discover(&[root.path().to_path_buf()]);
        assert_eq!(manager.entries.len(), 1);
        assert_eq!(manager.warnings().len(), 1);
    }

    #[test]
    fn oversized_timeout_is_clamped_with_warning() {
        let root = tempfile::tempdir().expect("tempdir");
        let dir = root.path().join("slow");
        std::fs::create_dir_all(&dir).expect("mkdir");
        std::fs::write(
            dir.join("plugin.json"),
            r#"{"name":"slow","version":"1","command":["x"],"capabilities":[],"timeout_ms":3600000}"#,
        )
        .expect("write");

        let manager = PluginManager::discover(&[root.path().to_path_buf()]);
        assert_eq!(manager.entries.len(), 1);
        assert!(manager.warnings().iter().any(|w| w.contains("clamped")));
    }

    #[test]
    fn invoke_unknown_plugin_errors() {
        let manager = PluginManager::default();
        let err = manager
            .invoke("ghost", "x", Value::Null)
            .expect_err("must fail");
        assert!(matches!(err, AppError::Plugin(_)));
    }
}
