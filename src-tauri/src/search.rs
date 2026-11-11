//! Search across open tabs and across a project directory.

use std::{fs, path::Path};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    pub tab_title: String,
    pub line: usize,
    pub start: usize,
    pub matched: String,
}

/// Find all occurrences of `needle` across open documents
/// (title, content) pairs.
pub fn find_in_open_tabs(docs: &[(String, String)], needle: &str) -> Vec<SearchHit> {
    let mut out = Vec::new();
    if needle.is_empty() {
        return out;
    }

    for (title, content) in docs {
        for (line_idx, line) in content.lines().enumerate() {
            let mut offset = 0;
            while offset < line.len() {
                let Some(found) = line[offset..].find(needle) else {
                    break;
                };
                let start = offset + found;
                out.push(SearchHit {
                    tab_title: title.clone(),
                    line: line_idx + 1,
                    start,
                    matched: needle.to_owned(),
                });
                offset = start + needle.len().max(1);
            }
        }
    }
    out
}

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
        }
    }
}

/// Find `needle` in every text file under `root` (substring match,
/// case-sensitive). Binary files and excluded directories are skipped.
pub fn project_search(root: &Path, needle: &str, options: &SearchOptions) -> Vec<ProjectHit> {
    let mut hits = Vec::new();
    if needle.is_empty() {
        return hits;
    }
    walk(root, needle, options, &mut hits);
    hits
}

fn walk(dir: &Path, needle: &str, options: &SearchOptions, hits: &mut Vec<ProjectHit>) {
    if hits.len() >= options.max_results {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if hits.len() >= options.max_results {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if options.exclude_dirs.iter().any(|ex| ex == name) {
                continue;
            }
            walk(&path, needle, options, hits);
            continue;
        }
        if !extension_allowed(&path, options) {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        if bytes.contains(&0) {
            continue; // binary
        }
        let Ok(content) = String::from_utf8(bytes) else {
            continue;
        };
        let mut modified = false;
        for (line_idx, line) in content.lines().enumerate() {
            if line.contains(needle) {
                hits.push(ProjectHit {
                    file: path.display().to_string(),
                    line: line_idx + 1,
                    text: truncate(line, 240),
                });
                modified = true;
                if hits.len() >= options.max_results {
                    return;
                }
            }
        }
        let _ = modified;
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
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn search_groups_hits_across_tabs() {
        let docs = vec![
            ("a.txt".to_string(), "hello\nneedle".to_string()),
            ("b.txt".to_string(), "needle needle".to_string()),
        ];
        let hits = find_in_open_tabs(&docs, "needle");
        assert_eq!(hits.len(), 3);
        assert_eq!(hits[0].tab_title, "a.txt");
        assert_eq!(hits[0].line, 2);
    }

    #[test]
    fn search_skips_empty_needle() {
        assert!(find_in_open_tabs(&[("a".into(), "x".into())], "").is_empty());
    }

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
        let hits = project_search(dir.path(), "needle", &options);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].file.ends_with("b.rs"));

        let hits_all = project_search(dir.path(), "needle", &SearchOptions::default());
        assert_eq!(hits_all.len(), 3);
    }

    #[test]
    fn project_search_respects_max_results() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("a.txt"), "needle\n".repeat(10)).expect("write");
        let options = SearchOptions {
            max_results: 3,
            ..Default::default()
        };
        assert_eq!(project_search(dir.path(), "needle", &options).len(), 3);
    }

    #[test]
    fn project_search_skips_binary_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("bin.dat"), b"needle\x00needle").expect("write");
        assert!(project_search(dir.path(), "needle", &SearchOptions::default()).is_empty());
    }

    #[test]
    fn hit_serializes_to_ipc_shape() {
        let hit = ProjectHit {
            file: PathBuf::from("/tmp/x").display().to_string(),
            line: 1,
            text: "t".into(),
        };
        let json = serde_json::to_string(&hit).expect("serialize");
        assert!(json.contains("\"file\""));
    }
}
