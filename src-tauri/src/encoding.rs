//! Encoding detection and text file round-tripping.
//!
//! Files are decoded to a `String` that is **normalized to LF** for editing
//! (Monaco models always use LF); the detected `EolStyle` and
//! `DetectedEncoding` are kept and re-applied on save so bytes round-trip.

use std::{path::Path, time::UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::error::{io_error, AppError, AppResult};

pub const BOM_UTF8: [u8; 3] = [0xEF, 0xBB, 0xBF];
pub const BOM_UTF16LE: [u8; 2] = [0xFF, 0xFE];
pub const BOM_UTF16BE: [u8; 2] = [0xFE, 0xFF];

// ---------------------------------------------------------------------------
// EOL style
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EolStyle {
    LF,
    CRLF,
    CR,
}

impl EolStyle {
    pub fn label(&self) -> &'static str {
        match self {
            EolStyle::LF => "LF",
            EolStyle::CRLF => "CRLF",
            EolStyle::CR => "CR",
        }
    }

    pub fn sequence(&self) -> &'static str {
        match self {
            EolStyle::LF => "\n",
            EolStyle::CRLF => "\r\n",
            EolStyle::CR => "\r",
        }
    }

    /// Parse a label coming back over IPC ("LF" / "CRLF" / "CR").
    pub fn from_label(label: &str) -> AppResult<Self> {
        match label {
            "LF" => Ok(EolStyle::LF),
            "CRLF" => Ok(EolStyle::CRLF),
            "CR" => Ok(EolStyle::CR),
            other => Err(AppError::Validation(format!("unknown EOL style: {other}"))),
        }
    }

    /// Detect the predominant EOL style from raw content.
    pub fn detect(content: &str) -> Self {
        let crlf = content.matches("\r\n").count();
        let lf_total = content.matches('\n').count();
        let cr_total = content.matches('\r').count();
        let lf = lf_total.saturating_sub(crlf);
        let cr = cr_total.saturating_sub(crlf);

        if crlf >= lf && crlf >= cr && crlf > 0 {
            EolStyle::CRLF
        } else if cr > lf && cr > 0 {
            EolStyle::CR
        } else {
            EolStyle::LF
        }
    }

    /// Convert content to use this EOL style (normalizing through LF first).
    pub fn apply(&self, content: &str) -> String {
        let normalized = content.replace("\r\n", "\n").replace('\r', "\n");
        match self {
            EolStyle::LF => normalized,
            EolStyle::CRLF => normalized.replace('\n', "\r\n"),
            EolStyle::CR => normalized.replace('\n', "\r"),
        }
    }
}

impl Default for EolStyle {
    fn default() -> Self {
        if cfg!(target_os = "windows") {
            EolStyle::CRLF
        } else {
            EolStyle::LF
        }
    }
}

// ---------------------------------------------------------------------------
// Detected encoding
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DetectedEncoding {
    #[default]
    Utf8,
    Utf8Bom,
    Utf16Le,
    Utf16Be,
    Windows1252,
}

impl DetectedEncoding {
    pub fn label(&self) -> &'static str {
        match self {
            DetectedEncoding::Utf8 => "UTF-8",
            DetectedEncoding::Utf8Bom => "UTF-8 BOM",
            DetectedEncoding::Utf16Le => "UTF-16 LE",
            DetectedEncoding::Utf16Be => "UTF-16 BE",
            DetectedEncoding::Windows1252 => "Windows-1252",
        }
    }

    /// Parse a label coming back over IPC.
    pub fn from_label(label: &str) -> AppResult<Self> {
        match label {
            "UTF-8" => Ok(DetectedEncoding::Utf8),
            "UTF-8 BOM" => Ok(DetectedEncoding::Utf8Bom),
            "UTF-16 LE" => Ok(DetectedEncoding::Utf16Le),
            "UTF-16 BE" => Ok(DetectedEncoding::Utf16Be),
            "Windows-1252" => Ok(DetectedEncoding::Windows1252),
            other => Err(AppError::Validation(format!("unknown encoding: {other}"))),
        }
    }
}

// ---------------------------------------------------------------------------
// Decode / encode
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedText {
    pub content: String,
    pub encoding: DetectedEncoding,
}

/// Decode raw bytes, detecting BOM-based encodings first, then UTF-8,
/// falling back to Windows-1252 (which never fails).
pub fn decode_bytes(bytes: &[u8]) -> DecodedText {
    if bytes.starts_with(&BOM_UTF8) {
        let body = &bytes[BOM_UTF8.len()..];
        return DecodedText {
            content: String::from_utf8_lossy(body).into_owned(),
            encoding: DetectedEncoding::Utf8Bom,
        };
    }
    if bytes.starts_with(&BOM_UTF16LE) {
        let (cow, _, _) = encoding_rs::UTF_16LE.decode(&bytes[BOM_UTF16LE.len()..]);
        return DecodedText {
            content: cow.into_owned(),
            encoding: DetectedEncoding::Utf16Le,
        };
    }
    if bytes.starts_with(&BOM_UTF16BE) {
        let (cow, _, _) = encoding_rs::UTF_16BE.decode(&bytes[BOM_UTF16BE.len()..]);
        return DecodedText {
            content: cow.into_owned(),
            encoding: DetectedEncoding::Utf16Be,
        };
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => DecodedText {
            content: text.to_owned(),
            encoding: DetectedEncoding::Utf8,
        },
        Err(_) => {
            let (cow, _, _) = encoding_rs::WINDOWS_1252.decode(bytes);
            DecodedText {
                content: cow.into_owned(),
                encoding: DetectedEncoding::Windows1252,
            }
        }
    }
}

/// Encode edited content (LF-normalized) back to bytes in the file's
/// original encoding and EOL style.
pub fn encode_text(content: &str, encoding: DetectedEncoding, eol: EolStyle) -> Vec<u8> {
    let with_eol = eol.apply(content);
    match encoding {
        DetectedEncoding::Utf8 => with_eol.into_bytes(),
        DetectedEncoding::Utf8Bom => {
            let mut out = BOM_UTF8.to_vec();
            out.extend_from_slice(with_eol.as_bytes());
            out
        }
        DetectedEncoding::Utf16Le => {
            let mut out = BOM_UTF16LE.to_vec();
            for unit in with_eol.encode_utf16() {
                out.extend_from_slice(&unit.to_le_bytes());
            }
            out
        }
        DetectedEncoding::Utf16Be => {
            let mut out = BOM_UTF16BE.to_vec();
            for unit in with_eol.encode_utf16() {
                out.extend_from_slice(&unit.to_be_bytes());
            }
            out
        }
        DetectedEncoding::Windows1252 => encode_windows1252(&with_eol),
    }
}

/// Windows-1252 cannot represent all of Unicode. `encoding_rs`'s
/// `encode()` substitutes HTML numeric references (it is designed for
/// HTML serialization); for file output we substitute `?` like classic
/// editors do. Files decoded *from* Windows-1252 always round-trip
/// unchanged — replacement only happens for newly typed unmappable text.
fn encode_windows1252(text: &str) -> Vec<u8> {
    let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.encode(text);
    if !had_errors {
        return cow.into_owned();
    }
    let mut out = Vec::with_capacity(text.len());
    for ch in text.chars() {
        let single = ch.to_string();
        let (cow, _, had_errors) = encoding_rs::WINDOWS_1252.encode(&single);
        if had_errors {
            out.push(b'?');
        } else {
            out.extend_from_slice(&cow);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// File-level API
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    /// LF-normalized content for editing.
    pub content: String,
    pub encoding: DetectedEncoding,
    pub eol: EolStyle,
    pub size: u64,
    pub mtime_ms: u64,
}

/// Modified time in milliseconds since the UNIX epoch (0 if unavailable).
pub fn mtime_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub fn read_text_file(path: &Path) -> AppResult<TextFile> {
    let bytes = std::fs::read(path).map_err(|err| io_error(path, err))?;
    let meta = std::fs::metadata(path).map_err(|err| io_error(path, err))?;
    let decoded = decode_bytes(&bytes);
    let eol = EolStyle::detect(&decoded.content);
    Ok(TextFile {
        content: EolStyle::LF.apply(&decoded.content),
        encoding: decoded.encoding,
        eol,
        size: meta.len(),
        mtime_ms: mtime_ms(&meta),
    })
}

pub fn write_text_file(
    path: &Path,
    content: &str,
    encoding: DetectedEncoding,
    eol: EolStyle,
) -> AppResult<u64> {
    let bytes = encode_text(content, encoding, eol);
    std::fs::write(path, bytes).map_err(|err| io_error(path, err))?;
    let meta = std::fs::metadata(path).map_err(|err| io_error(path, err))?;
    Ok(mtime_ms(&meta))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eol_detect_lf() {
        assert_eq!(EolStyle::detect("hello\nworld\n"), EolStyle::LF);
    }

    #[test]
    fn eol_detect_crlf() {
        assert_eq!(EolStyle::detect("hello\r\nworld\r\n"), EolStyle::CRLF);
    }

    #[test]
    fn eol_detect_cr() {
        assert_eq!(EolStyle::detect("hello\rworld\r"), EolStyle::CR);
    }

    #[test]
    fn eol_apply_converts() {
        assert_eq!(EolStyle::CRLF.apply("a\nb\nc"), "a\r\nb\r\nc");
        assert_eq!(EolStyle::LF.apply("a\r\nb\r\nc"), "a\nb\nc");
    }

    #[test]
    fn decode_utf8_plain() {
        let d = decode_bytes("héllo".as_bytes());
        assert_eq!(d.encoding, DetectedEncoding::Utf8);
        assert_eq!(d.content, "héllo");
    }

    #[test]
    fn decode_utf8_bom() {
        let mut bytes = BOM_UTF8.to_vec();
        bytes.extend_from_slice("héllo".as_bytes());
        let d = decode_bytes(&bytes);
        assert_eq!(d.encoding, DetectedEncoding::Utf8Bom);
        assert_eq!(d.content, "héllo");
    }

    #[test]
    fn decode_utf16_roundtrip() {
        for enc in [DetectedEncoding::Utf16Le, DetectedEncoding::Utf16Be] {
            let bytes = encode_text("héllo 中文", enc, EolStyle::LF);
            let d = decode_bytes(&bytes);
            assert_eq!(d.encoding, enc);
            assert_eq!(d.content, "héllo 中文");
        }
    }

    #[test]
    fn decode_windows1252_fallback() {
        // 0xE9 alone is invalid UTF-8; Windows-1252 reads it as é.
        let bytes = [0x63, 0x61, 0x66, 0xE9]; // café
        let d = decode_bytes(&bytes);
        assert_eq!(d.encoding, DetectedEncoding::Windows1252);
        assert_eq!(d.content, "café");
    }

    #[test]
    fn roundtrip_all_encodings_preserve_bytes() {
        let original = "first line\nhéllo café résumé\nlast";
        for enc in [
            DetectedEncoding::Utf8,
            DetectedEncoding::Utf8Bom,
            DetectedEncoding::Utf16Le,
            DetectedEncoding::Utf16Be,
            DetectedEncoding::Windows1252,
        ] {
            let bytes = encode_text(original, enc, EolStyle::LF);
            let d = decode_bytes(&bytes);
            assert_eq!(d.encoding, enc);
            assert_eq!(d.content, original);
            assert_eq!(encode_text(&d.content, enc, EolStyle::LF), bytes);
        }
    }

    #[test]
    fn windows1252_replaces_unmappable_chars_with_question_marks() {
        // 中文 is not representable in Windows-1252; classic editors
        // write '?' rather than corrupting the file.
        assert_eq!(
            encode_text("a中b文", DetectedEncoding::Windows1252, EolStyle::LF),
            b"a?b?"
        );
    }

    #[test]
    fn utf16_roundtrips_all_of_unicode() {
        for enc in [DetectedEncoding::Utf16Le, DetectedEncoding::Utf16Be] {
            let bytes = encode_text("héllo 中文 🎉", enc, EolStyle::LF);
            let d = decode_bytes(&bytes);
            assert_eq!(d.content, "héllo 中文 🎉");
        }
    }

    #[test]
    fn roundtrip_crlf_preserved_through_file_api() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("crlf.txt");
        let original = "a\r\nb\r\nc";
        std::fs::write(&path, original).expect("write");

        let loaded = read_text_file(&path).expect("read");
        assert_eq!(loaded.eol, EolStyle::CRLF);
        assert_eq!(loaded.content, "a\nb\nc"); // normalized for editing

        write_text_file(&path, &loaded.content, loaded.encoding, loaded.eol).expect("write back");
        assert_eq!(std::fs::read(&path).expect("bytes"), original.as_bytes());
    }

    #[test]
    fn read_missing_file_errors() {
        let err = read_text_file(Path::new("/nonexistent/nope.txt")).expect_err("must fail");
        assert!(matches!(err, AppError::MissingFile(_)));
    }

    #[test]
    fn labels_roundtrip() {
        for enc in [
            DetectedEncoding::Utf8,
            DetectedEncoding::Utf8Bom,
            DetectedEncoding::Utf16Le,
            DetectedEncoding::Utf16Be,
            DetectedEncoding::Windows1252,
        ] {
            assert_eq!(DetectedEncoding::from_label(enc.label()), Ok(enc));
        }
        for eol in [EolStyle::LF, EolStyle::CRLF, EolStyle::CR] {
            assert_eq!(EolStyle::from_label(eol.label()), Ok(eol));
        }
    }
}
