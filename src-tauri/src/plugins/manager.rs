//! Plugin discovery and lifecycle across plugin directories.
//!
//! Discovery: each configured directory may itself contain `plugin.json`,
//! and every subdirectory containing `plugin.json` is a plugin. Invalid
//! entries are skipped and reported as warnings (surfaced in the UI).
//!
//! Each plugin host sits behind its own `Mutex`, so one slow or hung plugin
//! call never blocks listing, reload, or other plugins.

use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{io_error, AppError, AppResult};

use super::host::{load_manifest, PluginHost, PluginManifest, MAX_TIMEOUT};

#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub capabilities: Vec<String>,
    pub dir: String,
    pub running: bool,
    pub enabled: bool,
    /// True for plugins in the user plugin dir (installed ones) — bundled
    /// plugins live in app resources and can only be disabled, not removed.
    pub removable: bool,
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
    /// The user's plugin directory (`plugin_dirs[0]`): installs land here
    /// and only plugins inside it may be removed.
    user_dir: PathBuf,
    disabled: Mutex<HashSet<String>>,
}

/// Persisted plugin settings (`plugins.json` in the config dir). Kept out
/// of the session file: plugin configuration is app-level, not per-session.
#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginConfig {
    #[serde(default)]
    pub disabled: Vec<String>,
}

impl PluginConfig {
    pub fn path(config_dir: &Path) -> PathBuf {
        config_dir.join("plugins.json")
    }

    /// Load from `config_dir`, falling back to defaults on any error.
    /// A missing file is the normal first-run case; anything else is
    /// logged so a corrupt or unreadable file is never a silent data drop.
    pub fn load(config_dir: &Path) -> Self {
        match std::fs::read_to_string(Self::path(config_dir)) {
            Ok(json) => match serde_json::from_str(&json) {
                Ok(config) => config,
                Err(err) => {
                    eprintln!("plugins.json is corrupt ({err}); starting with defaults");
                    Self::default()
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(err) => {
                eprintln!("plugins.json unreadable ({err}); starting with defaults");
                Self::default()
            }
        }
    }

    /// Save to `config_dir`, creating it if needed.
    pub fn save(&self, config_dir: &Path) -> AppResult<()> {
        let path = Self::path(config_dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| io_error(parent, err))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json).map_err(|err| io_error(&path, err))?;
        Ok(())
    }

    pub fn disabled_set(&self) -> HashSet<String> {
        self.disabled.iter().cloned().collect()
    }
}

fn lock_host(host: &Arc<Mutex<PluginHost>>) -> std::sync::MutexGuard<'_, PluginHost> {
    host.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

impl PluginManager {
    /// Discover plugins in `dirs` (and their immediate subdirectories).
    /// `dirs[0]` is the user plugin directory (see `commands::plugin_dirs`).
    /// `disabled` names are listed but cannot be invoked until enabled.
    pub fn discover(dirs: &[PathBuf], disabled: &HashSet<String>) -> Self {
        let mut manager = PluginManager {
            user_dir: dirs.first().cloned().unwrap_or_default(),
            disabled: Mutex::new(disabled.clone()),
            ..PluginManager::default()
        };
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
                    enabled: self.is_enabled(&entry.manifest.name),
                    removable: entry.dir.starts_with(&self.user_dir) && entry.dir != self.user_dir,
                }
            })
            .collect()
    }

    /// The user plugin directory installs land in.
    pub fn user_dir(&self) -> &Path {
        &self.user_dir
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        !self
            .disabled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .contains(name)
    }

    /// Enable or disable a plugin at runtime. Disabling stops its process;
    /// enabling just allows the next invoke to start it.
    pub fn set_enabled(&self, name: &str, enabled: bool) -> AppResult<()> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.manifest.name == name)
            .ok_or_else(|| AppError::Plugin(format!("no plugin named '{name}'")))?;
        let mut disabled = self
            .disabled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if enabled {
            disabled.remove(name);
        } else {
            disabled.insert(name.to_owned());
            drop(disabled);
            lock_host(&entry.host).stop();
        }
        Ok(())
    }

    /// Stop a single plugin's process (used before removing it).
    pub fn stop(&self, name: &str) -> AppResult<()> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.manifest.name == name)
            .ok_or_else(|| AppError::Plugin(format!("no plugin named '{name}'")))?;
        lock_host(&entry.host).stop();
        Ok(())
    }

    /// The on-disk directory of a discovered plugin (exact PathBuf, not a
    /// lossy display string).
    pub fn plugin_dir(&self, name: &str) -> AppResult<PathBuf> {
        self.entries
            .iter()
            .find(|e| e.manifest.name == name)
            .map(|e| e.dir.clone())
            .ok_or_else(|| AppError::Plugin(format!("no plugin named '{name}'")))
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Invoke a capability on a named plugin (lazy process start). Only
    /// this plugin's lock is held for the duration of the call.
    pub fn invoke(&self, name: &str, method: &str, params: Value) -> AppResult<Value> {
        if !self.is_enabled(name) {
            return Err(AppError::Plugin(format!("plugin '{name}' is disabled")));
        }
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

        let manager = PluginManager::discover(&[root.path().to_path_buf()], &HashSet::new());
        let names: Vec<String> = manager.list().into_iter().map(|p| p.name).collect();
        assert_eq!(names, vec!["alpha", "flat"]);
    }

    #[test]
    fn invalid_manifest_becomes_warning_not_panic() {
        let root = tempfile::tempdir().expect("tempdir");
        let bad = root.path().join("bad");
        std::fs::create_dir_all(&bad).expect("mkdir");
        std::fs::write(bad.join("plugin.json"), "{not json").expect("write");

        let manager = PluginManager::discover(&[root.path().to_path_buf()], &HashSet::new());
        assert!(manager.entries.is_empty());
        assert_eq!(manager.warnings().len(), 1);
    }

    #[test]
    fn duplicate_names_are_skipped_with_warning() {
        let root = tempfile::tempdir().expect("tempdir");
        write_plugin(&root.path().join("one"), "same", &["a"]);
        write_plugin(&root.path().join("two"), "same", &["a"]);

        let manager = PluginManager::discover(&[root.path().to_path_buf()], &HashSet::new());
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

        let manager = PluginManager::discover(&[root.path().to_path_buf()], &HashSet::new());
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

    #[test]
    fn disabled_plugins_are_listed_but_not_invokable() {
        let root = tempfile::tempdir().expect("tempdir");
        write_plugin(root.path(), "off", &["a"]);
        let disabled: HashSet<String> = ["off".to_string()].into_iter().collect();

        let manager = PluginManager::discover(&[root.path().to_path_buf()], &disabled);
        assert!(!manager.list()[0].enabled);
        let err = manager
            .invoke("off", "a", Value::Null)
            .expect_err("disabled must refuse");
        assert!(matches!(err, AppError::Plugin(_)));

        manager.set_enabled("off", true).expect("enable");
        assert!(manager.list()[0].enabled);
    }

    #[test]
    fn set_enabled_on_unknown_plugin_errors() {
        let manager = PluginManager::default();
        assert!(manager.set_enabled("ghost", true).is_err());
    }

    #[test]
    fn removable_is_true_only_for_installed_plugins() {
        let user = tempfile::tempdir().expect("tempdir");
        let bundled = tempfile::tempdir().expect("tempdir");
        write_plugin(&user.path().join("installed"), "installed", &["a"]);
        write_plugin(user.path(), "flat", &["b"]); // flat in user dir: never removable
        write_plugin(&bundled.path().join("bundled"), "bundled", &["c"]);

        let manager = PluginManager::discover(
            &[user.path().to_path_buf(), bundled.path().to_path_buf()],
            &HashSet::new(),
        );
        let removable: Vec<(String, bool)> = manager
            .list()
            .into_iter()
            .map(|p| (p.name, p.removable))
            .collect();
        assert_eq!(
            removable,
            vec![
                ("bundled".to_string(), false),
                ("flat".to_string(), false),
                ("installed".to_string(), true),
            ]
        );
    }

    #[test]
    fn config_roundtrips_and_tolerates_missing_old_or_corrupt_json() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert_eq!(PluginConfig::load(dir.path()), PluginConfig::default());

        // Unknown fields must not break loading (forward compatibility).
        std::fs::write(
            dir.path().join("plugins.json"),
            r#"{"disabled":["a","b"],"future_field":true}"#,
        )
        .expect("write");
        let loaded = PluginConfig::load(dir.path());
        assert_eq!(loaded.disabled, vec!["a".to_string(), "b".to_string()]);

        let config = PluginConfig {
            disabled: vec!["x".into()],
        };
        config.save(dir.path()).expect("save");
        assert_eq!(PluginConfig::load(dir.path()), config);

        std::fs::write(dir.path().join("plugins.json"), "{not json").expect("write");
        assert_eq!(PluginConfig::load(dir.path()), PluginConfig::default());
    }
}
