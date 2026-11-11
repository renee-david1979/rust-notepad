//! Project-wide search (Find in Files).
//!
//! Matching honors the panel's toggles (match case / whole word / regex).
//! The walk is defensive: directory symlinks are never followed (cycle
//! guard and no read-escape outside the chosen root), and depth, scanned
//! file count and per-file size are all capped.

use std::{fs, path::Path};

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

const MAX_SCAN_FILES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectHit {
    pub file: String,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchOptions {
    pub include_extensions: Vec<String>,
    pub exclude_dirs: Vec<String>,
    pub max_results: usize,
    #[serde(default)]
    pub match_case: bool,
    #[serde(default)]
    pub whole_word: bool,
    #[serde(default)]
    pub use_regex: bool,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            include_extensions: Vec::new(),
            exclude_dirs: ["target", "node_modules", ".git", "dist", "build"]
                .iter()
                .map(|s| (*s).to_owned())
                .collect(),
            max_results: 500,
            match_case: false,
            whole_word: false,
            use_regex: false,
        }
    }
}

/// A line matcher built from the search term and options. Literal terms
/// are regex-escaped so both modes share one Unicode-aware engine.
struct Matcher(Regex);

impl Matcher {
    fn build(needle: &str, options: &SearchOptions) -> AppResult<Self> {
        let pattern = if options.use_regex {
            needle.to_owned()
        } else {
            regex::escape(needle)
        };
        let pattern = if options.whole_word {
            format!(r"\b(?:{pattern})\b")
        } else {
            pattern
        };
        let regex = RegexBuilder::new(&pattern)
            .case_insensitive(!options.match_case)
            .build()
            .map_err(|err| AppError::Validation(format!("invalid search pattern: {err}")))?;
        Ok(Matcher(regex))
    }

    fn is_match(&self, line: &str) -> bool {
        self.0.is_match(line)
    }
}

/// Find `needle` in text files under `root`. Binary files, excluded
/// directories and symlinks are skipped.
pub fn project_search(
    root: &Path,
    needle: &str,
    options: &SearchOptions,
) -> AppResult<Vec<ProjectHit>> {
    if needle.is_empty() {
        return Ok(Vec::new());
    }
    let matcher = Matcher::build(needle, options)?;
    let mut walker = Walker {
        matcher: &matcher,
        options,
        hits: Vec::new(),
        scanned: 0,
    };
    walker.walk(root, 0);
    Ok(walker.hits)
}

struct Walker<'a> {
    matcher: &'a Matcher,
    options: &'a SearchOptions,
    hits: Vec<ProjectHit>,
    scanned: usize,
}

impl Walker<'_> {
    fn walk(&mut self, dir: &Path, depth: usize) {
        if depth > MAX_DEPTH || self.done() {
            return;
        }
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            if self.done() {
                return;
            }
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            // Never follow symlinks: prevents cycles and keeps the search
            // from reading outside the chosen root.
            if file_type.is_symlink() {
                continue;
            }
            let path = entry.path();
            if file_type.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if self
                    .options
                    .exclude_dirs
                    .iter()
                    .any(|excluded| excluded == name)
                {
                    continue;
                }
                self.walk(&path, depth + 1);
                continue;
            }
            if !extension_allowed(&path, self.options) {
                continue;
            }
            self.scanned += 1;
            self.scan_file(&path);
        }
    }

    fn done(&self) -> bool {
        self.hits.len() >= self.options.max_results || self.scanned >= MAX_SCAN_FILES
    }

    fn scan_file(&mut self, path: &Path) {
        let Ok(meta) = fs::metadata(path) else {
            return;
        };
        if meta.len() > MAX_FILE_BYTES {
            return;
        }
        let Ok(bytes) = fs::read(path) else {
            return;
        };
        if bytes.contains(&0) {
            return; // binary
        }
        let Ok(content) = String::from_utf8(bytes) else {
            return;
        };
        for (line_idx, line) in content.lines().enumerate() {
            if self.matcher.is_match(line) {
                self.hits.push(ProjectHit {
                    file: path.display().to_string(),
                    line: line_idx + 1,
                    text: truncate(line, 240),
                });
                if self.done() {
                    return;
                }
            }
        }
    }
}

fn extension_allowed(path: &Path, options: &SearchOptions) -> bool {
    if options.include_extensions.is_empty() {
        return true;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    options
        .include_extensions
        .iter()
        .any(|inc| inc.eq_ignore_ascii_case(&ext))
}

fn truncate(line: &str, max: usize) -> String {
    if line.len() <= max {
        line.to_owned()
    } else {
        let mut end = max;
        while !line.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &line[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_search_finds_and_excludes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ignored = dir.path().join("target");
        fs::create_dir_all(&ignored).expect("mkdir");
        fs::write(dir.path().join("a.txt"), "abc\nneedle\n").expect("write a");
        fs::write(dir.path().join("b.rs"), "needle\nneedle").expect("write b");
        fs::write(ignored.join("c.rs"), "needle").expect("write c");

        let options = SearchOptions {
            include_extensions: vec!["rs".into()],
            ..Default::default()
        };
        let hits = project_search(dir.path(), "needle", &options).expect("search");
        assert_eq!(hits.len(), 2);
        assert!(hits[0].file.ends_with("b.rs"));

        let hits_all = project_search(dir.path(), "needle", &SearchOptions::default()).expect("s");
        assert_eq!(hits_all.len(), 3);
    }

    #[test]
    fn case_insensitive_by_default_and_exact_when_requested() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.txt"), "Needle\nneedle\nNEEDLE").expect("write");

        let hits = project_search(dir.path(), "needle", &SearchOptions::default()).expect("search");
        assert_eq!(hits.len(), 3);

        let options = SearchOptions {
            match_case: true,
            ..Default::default()
        };
        let hits = project_search(dir.path(), "needle", &options).expect("search");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn whole_word_respects_boundaries() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.txt"), "needle\nneedles\na-needle-b").expect("write");

        let options = SearchOptions {
            whole_word: true,
            ..Default::default()
        };
        let hits = project_search(dir.path(), "needle", &options).expect("search");
        // "needles" has no boundary after; "a-needle-b" does (hyphens are
        // non-word characters), matching standard \b semantics.
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].line, 1);
        assert_eq!(hits[1].line, 3);
    }

    #[test]
    fn regex_mode_matches_patterns() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.txt"), "foo123bar\nplain").expect("write");

        let options = SearchOptions {
            use_regex: true,
            ..Default::default()
        };
        let hits = project_search(dir.path(), r"foo\d+bar", &options).expect("search");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn invalid_regex_reports_validation_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let options = SearchOptions {
            use_regex: true,
            ..Default::default()
        };
        let err = project_search(dir.path(), "([unclosed", &options).expect_err("must fail");
        assert!(matches!(err, AppError::Validation(_)));
    }

    #[test]
    fn project_search_respects_max_results() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.txt"), "needle\n".repeat(10)).expect("write");
        let options = SearchOptions {
            max_results: 3,
            ..Default::default()
        };
        assert_eq!(
            project_search(dir.path(), "needle", &options)
                .expect("search")
                .len(),
            3
        );
    }

    #[test]
    fn project_search_skips_binary_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("bin.dat"), b"needle\x00needle").expect("write");
        assert!(
            project_search(dir.path(), "needle", &SearchOptions::default())
                .expect("search")
                .is_empty()
        );
    }

    #[cfg(unix)]
    #[test]
    fn directory_symlinks_are_not_followed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("outside");
        fs::write(outside.path().join("secret.txt"), "needle").expect("write secret");
        fs::write(dir.path().join("a.txt"), "needle").expect("write a");
        std::os::unix::fs::symlink(outside.path(), dir.path().join("escape")).expect("symlink");

        // Only the real file is hit; the symlinked dir is neither followed
        // (cycle guard) nor read (no escape outside the root).
        let hits = project_search(dir.path(), "needle", &SearchOptions::default()).expect("search");
        assert_eq!(hits.len(), 1);
        assert!(hits[0].file.ends_with("a.txt"));
    }

    #[test]
    fn empty_needle_returns_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(project_search(dir.path(), "", &SearchOptions::default())
            .expect("search")
            .is_empty());
    }

    #[test]
    fn hit_serializes_to_ipc_shape() {
        let hit = ProjectHit {
            file: "/tmp/x".to_owned(),
            line: 1,
            text: "t".into(),
        };
        let json = serde_json::to_string(&hit).expect("serialize");
        assert!(json.contains("\"file\""));
    }
}
