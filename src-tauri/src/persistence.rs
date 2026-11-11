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
    /// Which split pane (1 or 2) this tab belongs to.
    #[serde(default = "default_pane")]
    pub pane: u32,
    /// Marks the active tab within its pane.
    #[serde(default)]
    pub active: bool,
}

fn default_cursor() -> u32 {
    1
}

fn default_pane() -> u32 {
    1
}

impl Default for SavedTab {
    fn default() -> Self {
        Self {
            path: None,
            content: None,
            cursor_line: 1,
            cursor_column: 1,
            pane: 1,
            active: false,
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
    /// Which split pane has keyboard focus (1 or 2).
    #[serde(default = "default_pane")]
    pub active_pane: u32,
    /// Pane-1 share of the split width when split view was last used.
    #[serde(default)]
    pub split_ratio: Option<f32>,
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
            active_pane: 1,
            split_ratio: None,
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
            split_ratio: Some(0.35),
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
        assert_eq!(loaded.split_ratio, None);
    }

    #[test]
    fn saved_tab_defaults_cursor_to_one() {
        let tab = SavedTab::default();
        assert_eq!(tab.cursor_line, 1);
        assert_eq!(tab.cursor_column, 1);
    }

    #[test]
    fn split_ratio_keeps_values_verbatim() {
        // Out-of-range values are stored as-is: clamping is the frontend's
        // job (PaneManager.setSplitRatio), the session file is neutral.
        let dir = tempfile::tempdir().expect("tempdir");
        let cases = [
            (r#""split_ratio":0.35"#, Some(0.35_f32)),
            (r#""split_ratio":null"#, None),
            (r#""split_ratio":5.0"#, Some(5.0_f32)),
        ];
        for (field, expected) in cases {
            let json = format!(
                r#"{{"theme":"dark","font_size":14.0,"open_tabs":[],"active_tab":0,{field}}}"#
            );
            std::fs::write(dir.path().join("session.json"), json).expect("write");
            let loaded = PersistedState::load_from(dir.path());
            assert_eq!(loaded.split_ratio, expected);
        }
    }
}
