//! Plugin discovery and lifecycle across plugin directories.
//!
//! Discovery: each configured directory may itself contain `plugin.json`,
//! and every subdirectory containing `plugin.json` is a plugin. Invalid
//! entries are skipped and reported as warnings (surfaced in the UI).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, AppResult};

use super::host::{load_manifest, PluginHost, PluginManifest};

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
    pub host: PluginHost,
}

#[derive(Default)]
pub struct PluginManager {
    entries: Vec<PluginEntry>,
    warnings: Vec<String>,
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
            a.host
                .manifest()
                .name
                .to_lowercase()
                .cmp(&b.host.manifest().name.to_lowercase())
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
                    .any(|e| e.host.manifest().name == manifest.name)
                {
                    self.warnings.push(format!(
                        "duplicate plugin name '{}' in {} (skipped)",
                        manifest.name,
                        dir.display()
                    ));
                    return;
                }
                self.entries.push(PluginEntry {
                    host: PluginHost::new(manifest, dir.to_path_buf()),
                });
            }
            Err(err) => {
                self.warnings.push(format!("{}: {err}", dir.display()));
            }
        }
    }

    pub fn list(&mut self) -> Vec<PluginInfo> {
        self.entries
            .iter_mut()
            .map(|entry| {
                let manifest: &PluginManifest = entry.host.manifest();
                PluginInfo {
                    name: manifest.name.clone(),
                    version: manifest.version.clone(),
                    description: manifest.description.clone(),
                    capabilities: manifest.capabilities.clone(),
                    dir: entry.host.plugin_dir().display().to_string(),
                    running: entry.host.is_running(),
                }
            })
            .collect()
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Invoke a capability on a named plugin (lazy process start).
    pub fn invoke(&mut self, name: &str, method: &str, params: Value) -> AppResult<Value> {
        let entry = self
            .entries
            .iter_mut()
            .find(|e| e.host.manifest().name == name)
            .ok_or_else(|| AppError::Plugin(format!("no plugin named '{name}'")))?;
        entry.host.invoke(method, params)
    }

    pub fn stop_all(&mut self) {
        for entry in &mut self.entries {
            entry.host.stop();
        }
    }

    /// Stop all processes and re-run discovery on the same directories.
    pub fn reload(&mut self, dirs: &[PathBuf]) {
        self.stop_all();
        *self = PluginManager::discover(dirs);
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
        let names: Vec<String> = {
            let mut m = manager;
            m.list().into_iter().map(|p| p.name).collect()
        };
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
    fn invoke_unknown_plugin_errors() {
        let mut manager = PluginManager::default();
        let err = manager
            .invoke("ghost", "x", Value::Null)
            .expect_err("must fail");
        assert!(matches!(err, AppError::Plugin(_)));
    }
}
