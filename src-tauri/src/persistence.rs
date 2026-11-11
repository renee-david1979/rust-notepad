//! Persisted settings + session state.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{io_error, AppResult};

/// One restored tab. `path` is `None` for untitled buffers, whose content
/// is persisted inline in the session file so unsaved work survives a
/// restart. File tabs persist draft content only while dirty (crash
/// recovery for unsaved edits).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedTab {
    pub path: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default = "default_cursor")]
    pub cursor_line: u32,
    #[serde(default = "default_cursor")]
    pub cursor_column: u32,
}

fn default_cursor() -> u32 {
    1
}

impl Default for SavedTab {
    fn default() -> Self {
        Self {
            path: None,
            content: None,
            cursor_line: 1,
            cursor_column: 1,
        }
    }
}

/// Persisted application state saved to disk between sessions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PersistedState {
    pub theme: String,
    pub font_size: f32,
    #[serde(default = "default_true")]
    pub show_line_numbers: bool,
    #[serde(default)]
    pub word_wrap: bool,
    #[serde(default)]
    pub show_whitespace: bool,
    #[serde(default = "default_true")]
    pub show_minimap: bool,
    #[serde(default = "default_tab_size")]
    pub tab_size: u8,
    #[serde(default = "default_true")]
    pub auto_indent: bool,
    /// Auto-reload clean tabs when the file changes on disk.
    #[serde(default = "default_true")]
    pub auto_reload: bool,
    pub open_tabs: Vec<SavedTab>,
    pub active_tab: usize,
    #[serde(default)]
    pub recent_files: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_tab_size() -> u8 {
    4
}

impl Default for PersistedState {
    fn default() -> Self {
        Self {
            theme: "dark".to_string(),
            font_size: 14.0,
            show_line_numbers: true,
            word_wrap: false,
            show_whitespace: false,
            show_minimap: true,
            tab_size: 4,
            auto_indent: true,
            auto_reload: true,
            open_tabs: Vec::new(),
            active_tab: 0,
            recent_files: Vec::new(),
        }
    }
}

impl PersistedState {
    pub fn session_path(dir: &Path) -> PathBuf {
        dir.join("session.json")
    }

    /// Load from `dir`, falling back to defaults on any error.
    pub fn load_from(dir: &Path) -> Self {
        let path = Self::session_path(dir);
        match std::fs::read_to_string(&path) {
            Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save to `dir`, creating it if needed.
    pub fn save_to(&self, dir: &Path) -> AppResult<()> {
        let path = Self::session_path(dir);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| io_error(parent, err))?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json).map_err(|err| io_error(&path, err))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = PersistedState {
            open_tabs: vec![
                SavedTab {
                    path: Some("/tmp/a.txt".into()),
                    ..Default::default()
                },
                SavedTab {
                    content: Some("unsaved draft".into()),
                    cursor_line: 3,
                    cursor_column: 7,
                    ..Default::default()
                },
            ],
            active_tab: 1,
            recent_files: vec!["/tmp/a.txt".into()],
            ..Default::default()
        };
        state.save_to(dir.path()).expect("save");
        let loaded = PersistedState::load_from(dir.path());
        assert_eq!(loaded, state);
    }

    #[test]
    fn session_load_falls_back_to_defaults() {
        let dir = tempfile::tempdir().expect("tempdir");
        let loaded = PersistedState::load_from(dir.path());
        assert_eq!(loaded, PersistedState::default());

        // Corrupt file must not panic either.
        std::fs::write(dir.path().join("session.json"), "{not json").expect("write");
        let loaded = PersistedState::load_from(dir.path());
        assert_eq!(loaded, PersistedState::default());
    }

    #[test]
    fn old_session_json_with_missing_fields_still_loads() {
        // A session written by an older version lacks the newer flags.
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(
            dir.path().join("session.json"),
            r#"{"theme":"light","font_size":12.0,"show_line_numbers":true,
                "show_toolbar":true,"show_status_bar":true,"word_wrap":false,
                "show_whitespace":false,"open_tabs":[],"active_tab":0}"#,
        )
        .expect("write");
        let loaded = PersistedState::load_from(dir.path());
        assert_eq!(loaded.theme, "light");
        assert!(loaded.auto_reload);
        assert_eq!(loaded.tab_size, 4);
    }

    #[test]
    fn saved_tab_defaults_cursor_to_one() {
        let tab = SavedTab::default();
        assert_eq!(tab.cursor_line, 1);
        assert_eq!(tab.cursor_column, 1);
    }
}
