//! JSON types matching `difft --display json`.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use crate::line_ending::{normalize_line, split_logical_lines};
use serde::Deserialize;

/// One changed file entry from directory-mode `difft` JSON output.
#[derive(Debug, Clone, Deserialize)]
pub struct DiffFile {
    /// Relative path from the compared directory root.
    pub path: String,
    pub language: String,
    pub status: DiffStatus,
    pub extra_info: Option<String>,
    pub aligned_lines: Vec<AlignedLine>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DiffStatus {
    Unchanged,
    Changed,
    Created,
    Deleted,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AlignedLine {
    pub lhs_line: Option<u32>,
    pub rhs_line: Option<u32>,
    pub lhs_text: String,
    pub rhs_text: String,
    pub is_novel_lhs: bool,
    pub is_novel_rhs: bool,
    #[serde(default)]
    pub lhs_spans: Vec<crate::segments::TextSpan>,
    #[serde(default)]
    pub rhs_spans: Vec<crate::segments::TextSpan>,
}

#[derive(Debug, Deserialize)]
struct NewDiffFile {
    path: String,
    language: String,
    status: DiffStatus,
    #[serde(default)]
    extra_info: Option<String>,
    #[serde(default)]
    aligned_lines: Vec<(Option<u32>, Option<u32>)>,
    #[serde(default)]
    chunks: Vec<Vec<NewChunkLine>>,
}

#[derive(Debug, Deserialize)]
struct NewChunkLine {
    #[serde(default)]
    lhs: Option<NewSide>,
    #[serde(default)]
    rhs: Option<NewSide>,
}

#[derive(Debug, Deserialize)]
struct NewSide {
    line_number: u32,
    #[serde(default)]
    changes: Vec<NewChange>,
}

#[derive(Debug, Deserialize)]
struct NewChange {
    start: u32,
    end: u32,
    content: String,
    highlight: crate::segments::Highlight,
}

/// One row ready for the Slint diff panes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisplayLine {
    pub lhs_line: Option<u32>,
    pub rhs_line: Option<u32>,
    pub lhs_text: String,
    pub rhs_text: String,
    pub is_novel_lhs: bool,
    pub is_novel_rhs: bool,
    pub lhs_spans: Vec<crate::segments::TextSpan>,
    pub rhs_spans: Vec<crate::segments::TextSpan>,
}

fn uses_legacy_aligned_lines(value: &serde_json::Value) -> bool {
    value
        .get("aligned_lines")
        .and_then(|lines| lines.as_array())
        .is_some_and(|lines| lines.first().is_some_and(|line| line.is_object()))
}

fn read_file_lines(path: &Path) -> Result<Vec<String>, String> {
    if !path.is_file() {
        return Ok(vec![]);
    }
    let bytes = fs::read(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    Ok(split_logical_lines(&String::from_utf8_lossy(&bytes)))
}

fn read_text_file(path: &Path) -> Result<String, String> {
    if !path.is_file() {
        return Err(format!("Failed to read {}: file not found", path.display()));
    }
    let bytes = fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn normalize_diff_file(file: &mut DiffFile) {
    for line in &mut file.aligned_lines {
        line.lhs_text = normalize_line(&line.lhs_text);
        line.rhs_text = normalize_line(&line.rhs_text);
        for span in &mut line.lhs_spans {
            span.content = normalize_line(&span.content);
        }
        for span in &mut line.rhs_spans {
            span.content = normalize_line(&span.content);
        }
    }
}

fn line_text(lines: &[String], line: Option<u32>) -> String {
    line.and_then(|n| lines.get(n as usize))
        .cloned()
        .unwrap_or_default()
}

fn changes_to_spans(changes: &[NewChange]) -> Vec<crate::segments::TextSpan> {
    changes
        .iter()
        .map(|change| crate::segments::TextSpan {
            start: change.start,
            end: change.end,
            content: normalize_line(&change.content),
            highlight: change.highlight,
            is_novel: true,
        })
        .collect()
}

fn aligned_pairs_for_status(
    status: DiffStatus,
    lhs_lines: &[String],
    rhs_lines: &[String],
) -> Vec<(Option<u32>, Option<u32>)> {
    match status {
        DiffStatus::Unchanged => {
            let count = lhs_lines.len().max(rhs_lines.len());
            (0..count as u32)
                .map(|line| (Some(line), Some(line)))
                .collect()
        }
        DiffStatus::Created => (0..rhs_lines.len() as u32)
            .map(|line| (None, Some(line)))
            .collect(),
        DiffStatus::Deleted => (0..lhs_lines.len() as u32)
            .map(|line| (Some(line), None))
            .collect(),
        DiffStatus::Changed => Vec::new(),
    }
}

fn convert_new_diff_json(raw: NewDiffFile, path_a: &Path, path_b: &Path) -> Result<DiffFile, String> {
    let lhs_lines = read_file_lines(path_a)?;
    let rhs_lines = read_file_lines(path_b)?;

    let pairs = if raw.aligned_lines.is_empty() {
        aligned_pairs_for_status(raw.status, &lhs_lines, &rhs_lines)
    } else {
        raw.aligned_lines
    };

    let mut chunk_map: HashMap<(Option<u32>, Option<u32>), NewChunkLine> = HashMap::new();
    for chunk in raw.chunks {
        for line in chunk {
            let key = (
                line.lhs.as_ref().map(|side| side.line_number),
                line.rhs.as_ref().map(|side| side.line_number),
            );
            chunk_map.insert(key, line);
        }
    }

    let aligned_lines = pairs
        .into_iter()
        .map(|(lhs_line, rhs_line)| {
            let chunk = chunk_map.get(&(lhs_line, rhs_line));
            let lhs_spans = chunk
                .and_then(|line| line.lhs.as_ref())
                .map(|side| changes_to_spans(&side.changes))
                .unwrap_or_default();
            let rhs_spans = chunk
                .and_then(|line| line.rhs.as_ref())
                .map(|side| changes_to_spans(&side.changes))
                .unwrap_or_default();
            let is_novel_lhs = match chunk {
                Some(line) => {
                    line.lhs
                        .as_ref()
                        .is_some_and(|side| !side.changes.is_empty())
                        || rhs_line.is_none()
                }
                None => false,
            };
            let is_novel_rhs = match chunk {
                Some(line) => {
                    line.rhs
                        .as_ref()
                        .is_some_and(|side| !side.changes.is_empty())
                        || lhs_line.is_none()
                }
                None => false,
            };
            AlignedLine {
                lhs_line,
                rhs_line,
                lhs_text: line_text(&lhs_lines, lhs_line),
                rhs_text: line_text(&rhs_lines, rhs_line),
                is_novel_lhs,
                is_novel_rhs,
                lhs_spans,
                rhs_spans,
            }
        })
        .collect();

    let mut file = DiffFile {
        path: raw.path,
        language: raw.language,
        status: raw.status,
        extra_info: raw.extra_info,
        aligned_lines,
    };
    normalize_diff_file(&mut file);
    Ok(file)
}

fn parse_diff_entry(
    value: serde_json::Value,
    path_a: &Path,
    path_b: &Path,
) -> Result<DiffFile, String> {
    if uses_legacy_aligned_lines(&value) {
        let mut file: DiffFile =
            serde_json::from_value(value).map_err(|e| format!("invalid JSON: {e}"))?;
        normalize_diff_file(&mut file);
        Ok(file)
    } else {
        let raw: NewDiffFile =
            serde_json::from_value(value).map_err(|e| format!("invalid JSON: {e}"))?;
        convert_new_diff_json(raw, path_a, path_b)
    }
}

#[cfg(test)]
pub fn parse_diff_json(stdout: &[u8], path_a: &Path, path_b: &Path) -> Result<DiffFile, String> {
    let trimmed = std::str::from_utf8(stdout)
        .map_err(|e| e.to_string())?
        .trim();
    if trimmed.is_empty() {
        return Err("difft produced no JSON output.".to_owned());
    }

    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("invalid JSON: {e}"))?;
    let value = if let Some(array) = value.as_array() {
        array
            .first()
            .cloned()
            .ok_or_else(|| "difft returned an empty JSON array.".to_owned())?
    } else {
        value
    };

    parse_diff_entry(value, path_a, path_b)
}

/// Parse directory-mode stdout as a JSON array (or a single object fallback).
pub fn parse_diff_results(
    stdout: &[u8],
    dir_a: &Path,
    dir_b: &Path,
) -> Result<Vec<DiffFile>, String> {
    let stdout = stdout
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(stdout);
    let trimmed = std::str::from_utf8(stdout)
        .map_err(|e| e.to_string())?
        .trim()
        .trim_start_matches('\u{FEFF}');
    if trimmed.is_empty() {
        return Err("difft produced no JSON output.".to_owned());
    }

    let value: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("invalid JSON: {e}"))?;

    let entries: Vec<serde_json::Value> = if let Some(array) = value.as_array() {
        if array.is_empty() {
            return Err("difft returned an empty JSON array (no differences).".to_owned());
        }
        array.clone()
    } else {
        vec![value]
    };

    let mut files = Vec::with_capacity(entries.len());
    let mut warnings = Vec::new();
    for entry in entries {
        let relative = entry
            .get("path")
            .and_then(|path| path.as_str())
            .unwrap_or("")
            .to_owned();
        let path_a = dir_a.join(&relative);
        let path_b = dir_b.join(&relative);
        match parse_diff_entry(entry, &path_a, &path_b) {
            Ok(file) => files.push(file),
            Err(err) => warnings.push(format!("{relative}: {err}")),
        }
    }

    if files.is_empty() {
        if warnings.is_empty() {
            return Err("difft returned an empty JSON array (no differences).".to_owned());
        }
        return Err(warnings.join("\n"));
    }

    if !warnings.is_empty() {
        let note = format!(
            "Skipped {} file(s) due to read/parse errors:\n{}",
            warnings.len(),
            warnings.join("\n")
        );
        if let Some(first) = files.first_mut() {
            first.extra_info = Some(match &first.extra_info {
                Some(existing) if !existing.is_empty() => format!("{existing}\n{note}"),
                _ => note,
            });
        }
    }

    Ok(files)
}

fn aligned_to_display(lines: &[AlignedLine]) -> Vec<DisplayLine> {
    lines
        .iter()
        .map(|line| DisplayLine {
            lhs_line: line.lhs_line,
            rhs_line: line.rhs_line,
            lhs_text: line.lhs_text.clone(),
            rhs_text: line.rhs_text.clone(),
            is_novel_lhs: line.is_novel_lhs,
            is_novel_rhs: line.is_novel_rhs,
            lhs_spans: line.lhs_spans.clone(),
            rhs_spans: line.rhs_spans.clone(),
        })
        .collect()
}

fn lines_from_file(path: &Path, side: DiffStatus) -> Result<Vec<DisplayLine>, String> {
    let content = read_text_file(path)?;

    Ok(split_logical_lines(&content)
        .into_iter()
        .enumerate()
        .map(|(idx, line)| {
            let line_num = idx as u32;
            match side {
                DiffStatus::Deleted => DisplayLine {
                    lhs_line: Some(line_num),
                    rhs_line: None,
                    lhs_text: line,
                    rhs_text: String::new(),
                    is_novel_lhs: true,
                    is_novel_rhs: false,
                    lhs_spans: vec![],
                    rhs_spans: vec![],
                },
                DiffStatus::Created => DisplayLine {
                    lhs_line: None,
                    rhs_line: Some(line_num),
                    lhs_text: String::new(),
                    rhs_text: line,
                    is_novel_lhs: false,
                    is_novel_rhs: true,
                    lhs_spans: vec![],
                    rhs_spans: vec![],
                },
                _ => unreachable!(),
            }
        })
        .collect())
}

pub fn display_lines(
    file: &DiffFile,
    path_a: &Path,
    path_b: &Path,
) -> Result<Vec<DisplayLine>, String> {
    if !file.aligned_lines.is_empty() {
        return Ok(aligned_to_display(&file.aligned_lines));
    }

    match file.status {
        DiffStatus::Deleted => lines_from_file(&path_a.join(&file.path), DiffStatus::Deleted),
        DiffStatus::Created => lines_from_file(&path_b.join(&file.path), DiffStatus::Created),
        _ => Ok(Vec::new()),
    }
}

/// Normalize a CLI extension (`cpp`, `.cpp`, `CPP` → `cpp`).
pub fn normalize_extension(ext: &str) -> Option<String> {
    let ext = ext.trim().trim_start_matches('.');
    if ext.is_empty() {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// Whether `difft` treated this entry as a text diff (not binary).
pub fn is_text_diff_file(file: &DiffFile) -> bool {
    !file.language.eq_ignore_ascii_case("Binary")
}

fn path_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
}

/// Whether a relative path ends with one of the normalized extensions.
pub fn path_matches_extensions(path: &str, extensions: &[String]) -> bool {
    let Some(file_ext) = path_extension(path) else {
        return false;
    };
    extensions.iter().any(|ext| file_ext == *ext)
}

/// Keep text diffs; when `extensions` is non-empty, keep only matching suffixes.
pub fn filter_diff_files(mut files: Vec<DiffFile>, extensions: &[String]) -> Vec<DiffFile> {
    files.retain(|file| {
        if !is_text_diff_file(file) {
            return false;
        }
        extensions.is_empty() || path_matches_extensions(&file.path, extensions)
    });
    files
}

pub fn status_label(status: DiffStatus) -> &'static str {
    match status {
        DiffStatus::Changed => "M",
        DiffStatus::Created => "A",
        DiffStatus::Deleted => "D",
        DiffStatus::Unchanged => "=",
    }
}

pub fn warning_message(file: &DiffFile) -> Option<String> {
    if let Some(info) = &file.extra_info {
        if !info.is_empty() {
            return Some(info.clone());
        }
    }
    if file.language.starts_with("Text (") {
        return Some(file.language.clone());
    }
    None
}

pub fn file_info_message(file: &DiffFile, visible_line_count: usize) -> String {
    let mut parts = Vec::new();
    if let Some(warning) = warning_message(file) {
        parts.push(warning);
    }
    if visible_line_count == 0 {
        match file.status {
            DiffStatus::Created => parts.push("File created.".to_owned()),
            DiffStatus::Deleted => parts.push("File deleted.".to_owned()),
            _ => {}
        }
    }
    parts.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;

    fn write_temp_file(name: &str, content: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "difft-dir-viewer-{name}-{}.txt",
            std::process::id()
        ));
        let mut file = fs::File::create(&path).unwrap();
        write!(file, "{content}").unwrap();
        path
    }

    fn sample_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../difft-file-viewer/difftastic/sample_files")
    }

    #[test]
    fn parse_directory_json_array_legacy() {
        let json = br#"[
            {"path":"a.rs","language":"Rust","status":"changed","extra_info":null,"aligned_lines":[
                {"lhs_text":"old","rhs_text":"new","is_novel_lhs":true,"is_novel_rhs":true}
            ]},
            {"path":"b.rs","language":"Rust","status":"created","extra_info":null,"aligned_lines":[]}
        ]"#;
        let dir_a = write_temp_file("dir-a", "");
        let dir_b = write_temp_file("dir-b", "");
        let files = parse_diff_results(json, &dir_a, &dir_b).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.rs");
        assert_eq!(files[1].status, DiffStatus::Created);
    }

    #[test]
    fn parse_new_json_format_reads_source_lines() {
        let path_a = write_temp_file("a", "hello\nkeep");
        let path_b = write_temp_file("b", "world\nkeep");
        let json = br#"{
            "aligned_lines": [[0,0],[1,1]],
            "chunks": [[{
                "lhs": {"line_number": 0, "changes": [{"start": 0, "end": 5, "content": "hello", "highlight": "normal"}]},
                "rhs": {"line_number": 0, "changes": [{"start": 0, "end": 5, "content": "world", "highlight": "normal"}]}
            }]],
            "language": "Text",
            "path": "a.txt",
            "status": "changed"
        }"#;

        let diff = parse_diff_json(json, &path_a, &path_b).unwrap();
        assert_eq!(diff.aligned_lines.len(), 2);
        assert_eq!(diff.aligned_lines[0].lhs_text, "hello");
        assert_eq!(diff.aligned_lines[0].rhs_text, "world");
        assert!(diff.aligned_lines[0].is_novel_lhs);
        assert!(diff.aligned_lines[0].is_novel_rhs);
        assert!(!diff.aligned_lines[1].is_novel_lhs);
        assert_eq!(diff.aligned_lines[1].lhs_text, "keep");
    }

    #[test]
    fn parse_directory_array_new_format() {
        let dir_a = std::env::temp_dir().join(format!(
            "difft-dir-viewer-a-{}",
            std::process::id()
        ));
        let dir_b = std::env::temp_dir().join(format!(
            "difft-dir-viewer-b-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join("file.txt"), "left\n").unwrap();
        fs::write(dir_b.join("file.txt"), "right\n").unwrap();
        let json = br#"[{
            "aligned_lines": [[0,0]],
            "chunks": [[{
                "lhs": {"line_number": 0, "changes": [{"start": 0, "end": 4, "content": "left", "highlight": "normal"}]},
                "rhs": {"line_number": 0, "changes": [{"start": 0, "end": 5, "content": "right", "highlight": "normal"}]}
            }]],
            "language": "Text",
            "path": "file.txt",
            "status": "changed"
        }]"#;
        let files = parse_diff_results(json, &dir_a, &dir_b).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].aligned_lines[0].lhs_text, "left");
        assert_eq!(files[0].aligned_lines[0].rhs_text, "right");
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn display_lines_for_deleted_reads_from_dir_a() {
        let root = sample_root();
        let file = DiffFile {
            path: "dir_1/only_in_1.c".to_owned(),
            language: "C".to_owned(),
            status: DiffStatus::Deleted,
            extra_info: None,
            aligned_lines: vec![],
        };
        let lines = display_lines(&file, &root, &root).unwrap();
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|line| line.lhs_text.contains("#include")));
        assert!(lines.iter().all(|line| line.rhs_text.is_empty()));
    }

    #[test]
    fn parse_directory_json_skips_invalid_utf8_via_lossy_read() {
        let dir_a = std::env::temp_dir().join(format!(
            "difft-dir-viewer-lossy-a-{}",
            std::process::id()
        ));
        let dir_b = std::env::temp_dir().join(format!(
            "difft-dir-viewer-lossy-b-{}",
            std::process::id()
        ));
        fs::create_dir_all(&dir_a).unwrap();
        fs::create_dir_all(&dir_b).unwrap();
        fs::write(dir_a.join("gbk.cpp"), b"left \xba\xc2\n").unwrap();
        fs::write(dir_b.join("gbk.cpp"), b"right \xba\xc2\n").unwrap();
        let json = br#"[{
            "aligned_lines": [[0,0]],
            "chunks": [[{
                "lhs": {"line_number": 0, "changes": [{"start": 0, "end": 4, "content": "left", "highlight": "normal"}]},
                "rhs": {"line_number": 0, "changes": [{"start": 0, "end": 5, "content": "right", "highlight": "normal"}]}
            }]],
            "language": "Text",
            "path": "gbk.cpp",
            "status": "changed"
        }]"#;
        let files = parse_diff_results(json, &dir_a, &dir_b).unwrap();
        assert_eq!(files.len(), 1);
        assert!(!files[0].aligned_lines[0].lhs_text.is_empty());
        let _ = fs::remove_dir_all(&dir_a);
        let _ = fs::remove_dir_all(&dir_b);
    }

    #[test]
    fn normalize_extension_accepts_dotted_and_uppercase() {
        assert_eq!(normalize_extension("cpp").as_deref(), Some("cpp"));
        assert_eq!(normalize_extension(".CPP").as_deref(), Some("cpp"));
        assert_eq!(normalize_extension("  .h  ").as_deref(), Some("h"));
        assert!(normalize_extension(".").is_none());
        assert!(normalize_extension("").is_none());
    }

    #[test]
    fn filter_drops_binary_files() {
        let files = vec![
            DiffFile {
                path: "a.rs".into(),
                language: "Rust".into(),
                status: DiffStatus::Changed,
                extra_info: None,
                aligned_lines: vec![],
            },
            DiffFile {
                path: "logo.png".into(),
                language: "Binary".into(),
                status: DiffStatus::Changed,
                extra_info: None,
                aligned_lines: vec![],
            },
        ];
        let kept = filter_diff_files(files, &[]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, "a.rs");
    }

    #[test]
    fn filter_by_extension_is_case_insensitive() {
        let files = vec![
            DiffFile {
                path: "src/main.cpp".into(),
                language: "C++".into(),
                status: DiffStatus::Changed,
                extra_info: None,
                aligned_lines: vec![],
            },
            DiffFile {
                path: "src/main.rs".into(),
                language: "Rust".into(),
                status: DiffStatus::Changed,
                extra_info: None,
                aligned_lines: vec![],
            },
            DiffFile {
                path: "Makefile".into(),
                language: "Text".into(),
                status: DiffStatus::Changed,
                extra_info: None,
                aligned_lines: vec![],
            },
        ];
        let kept = filter_diff_files(files, &["cpp".to_owned()]);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].path, "src/main.cpp");
    }

    #[test]
    fn parse_saved_g130_g132_json_when_paths_exist() {
        let json_path = PathBuf::from(
            r"C:\Users\Proitav\.cursor\projects\d-myrust-slint-viewer\agent-tools\difft-out.json",
        );
        let dir_a = Path::new(r"Z:\dongle\g130app");
        let dir_b = Path::new(r"Z:\dongle\g132app");
        if !json_path.exists() || !dir_a.is_dir() || !dir_b.is_dir() {
            return;
        }
        let json = fs::read(json_path).unwrap();
        let files = parse_diff_results(&json, dir_a, dir_b).unwrap();
        assert!(files.len() >= 50, "expected many files, got {}", files.len());
        let lines = display_lines(&files[0], dir_a, dir_b).unwrap();
        assert!(!lines.is_empty(), "first file should have display lines");
    }
}
