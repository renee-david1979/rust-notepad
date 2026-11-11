//! Map file extensions and shebangs to Monaco language ids.

/// Detect a Monaco language id from a file extension.
pub fn language_for_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "rs" => "rust",
        "js" | "mjs" | "cjs" | "jsx" => "javascript",
        "ts" | "mts" | "cts" | "tsx" => "typescript",
        "json" | "jsonc" | "geojson" | "lock" => "json",
        "xml" | "xsl" | "xslt" | "xsd" | "wsdl" | "rss" | "opml" | "svg" | "plist" | "csproj"
        | "fsproj" | "vbproj" | "props" | "targets" | "nuspec" | "config" | "xaml" | "resx" => {
            "xml"
        }
        "html" | "htm" | "vue" | "svelte" | "jsp" | "erb" | "haml" => "html",
        "css" => "css",
        "scss" | "sass" => "scss",
        "less" => "less",
        "md" | "markdown" | "mdx" => "markdown",
        "py" | "pyw" | "pyi" => "python",
        "go" => "go",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hxx" | "hh" => "cpp",
        "cs" | "csx" => "csharp",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "swift" => "swift",
        "rb" | "rake" | "gemspec" => "ruby",
        "php" => "php",
        "sh" | "bash" | "zsh" | "fish" | "command" => "shell",
        "ps1" | "psm1" | "psd1" => "powershell",
        "bat" | "cmd" => "bat",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "ini" | "cfg" | "conf" | "properties" | "editorconfig" => "ini",
        "sql" | "mysql" | "pgsql" => "sql",
        "lua" => "lua",
        "r" => "r",
        "pl" | "pm" | "t" => "perl",
        "m" | "mm" => "objective-c",
        "hs" => "haskell",
        "clj" | "cljs" | "edn" => "clojure",
        "scala" | "sbt" => "scala",
        "erl" | "hrl" => "erlang",
        "ex" | "exs" => "elixir",
        "dart" => "dart",
        "groovy" | "gradle" => "groovy",
        "graphql" | "gql" => "graphql",
        "proto" => "proto",
        "tex" | "ltx" => "latex",
        "diff" | "patch" => "diff",
        "dockerfile" => "dockerfile",
        "makefile" | "mk" => "plaintext",
        "txt" | "log" | "csv" | "tsv" | "" => "plaintext",
        _ => "plaintext",
    }
}

/// Detect a Monaco language id from a shebang line.
pub fn language_from_shebang(content: &str) -> Option<&'static str> {
    let first_line = content.lines().next()?;
    if !first_line.starts_with("#!") {
        return None;
    }
    let shebang = first_line.to_lowercase();
    if shebang.contains("python") {
        Some("python")
    } else if shebang.contains("ruby") {
        Some("ruby")
    } else if shebang.contains("node") || shebang.contains("deno") {
        Some("javascript")
    } else if shebang.contains("bash") || shebang.contains("/sh") || shebang.contains("/zsh") {
        Some("shell")
    } else if shebang.contains("perl") {
        Some("perl")
    } else if shebang.contains("lua") {
        Some("lua")
    } else if shebang.contains("php") {
        Some("php")
    } else {
        None
    }
}

/// Full detection: extension wins when it maps to something meaningful;
/// otherwise fall back to shebang sniffing.
pub fn detect_language(path: Option<&str>, content: &str) -> &'static str {
    let ext = path
        .and_then(|p| std::path::Path::new(p).extension())
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let by_ext = language_for_extension(ext);
    if by_ext != "plaintext" || !ext.is_empty() {
        return by_ext;
    }
    language_from_shebang(content).unwrap_or("plaintext")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_map_to_monaco_ids() {
        assert_eq!(language_for_extension("rs"), "rust");
        assert_eq!(language_for_extension("TS"), "typescript");
        assert_eq!(language_for_extension("csproj"), "xml");
        assert_eq!(language_for_extension("yml"), "yaml");
        assert_eq!(language_for_extension("unknown-ext"), "plaintext");
    }

    #[test]
    fn shebang_detection() {
        assert_eq!(
            language_from_shebang("#!/usr/bin/env python3\nprint(1)"),
            Some("python")
        );
        assert_eq!(language_from_shebang("#!/bin/bash\necho hi"), Some("shell"));
        assert_eq!(language_from_shebang("plain text"), None);
    }

    #[test]
    fn extension_wins_over_shebang() {
        assert_eq!(
            detect_language(Some("script.py"), "#!/bin/bash\nprint"),
            "python"
        );
    }

    #[test]
    fn shebang_used_for_extensionless_files() {
        assert_eq!(
            detect_language(Some("runme"), "#!/usr/bin/env python3\nprint"),
            "python"
        );
        assert_eq!(detect_language(None, "hello"), "plaintext");
    }
}
