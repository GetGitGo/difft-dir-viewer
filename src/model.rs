//! JSON types matching `difft --display json` (GuiDiffFile schema).

use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DiffFile {
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
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {e}", path.display()))?;

    Ok(content
        .lines()
        .enumerate()
        .map(|(idx, line)| {
            let line_num = idx as u32;
            match side {
                DiffStatus::Deleted => DisplayLine {
                    lhs_line: Some(line_num),
                    rhs_line: None,
                    lhs_text: line.to_owned(),
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
                    rhs_text: line.to_owned(),
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

/// Build A/B lines for the viewer, filling in content for added/deleted files.
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

pub fn parse_diff_results(stdout: &[u8]) -> Result<Vec<DiffFile>, String> {
    let trimmed = std::str::from_utf8(stdout)
        .map_err(|e| e.to_string())?
        .trim();
    if trimmed.is_empty() {
        return Err("difft produced no JSON output.".to_owned());
    }

    if trimmed.starts_with('[') {
        let files: Vec<DiffFile> =
            serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON array: {e}"))?;
        if files.is_empty() {
            return Err("difft returned an empty JSON array (no differences).".to_owned());
        }
        Ok(files)
    } else {
        let file: DiffFile =
            serde_json::from_str(trimmed).map_err(|e| format!("invalid JSON: {e}"))?;
        Ok(vec![file])
    }
}

pub fn status_label(status: DiffStatus) -> &'static str {
    match status {
        DiffStatus::Changed => "M",
        DiffStatus::Created => "A",
        DiffStatus::Deleted => "D",
        DiffStatus::Unchanged => "=",
    }
}

/// Warning text when diff succeeded but fell back (parse errors, byte limit, etc.).
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
    use std::path::PathBuf;

    #[test]
    fn parse_directory_json_array() {
        let json = br#"[
            {"path":"a.rs","language":"Rust","status":"changed","extra_info":null,"aligned_lines":[
                {"lhs_text":"old","rhs_text":"new","is_novel_lhs":true,"is_novel_rhs":true}
            ]},
            {"path":"b.rs","language":"Rust","status":"created","extra_info":null,"aligned_lines":[]}
        ]"#;
        let files = parse_diff_results(json).unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "a.rs");
        assert_eq!(files[1].status, DiffStatus::Created);
    }

    #[test]
    fn display_lines_for_deleted_reads_from_dir_a() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sample_files");
        let file = DiffFile {
            path: "only_in_1.c".to_owned(),
            language: "C".to_owned(),
            status: DiffStatus::Deleted,
            extra_info: None,
            aligned_lines: vec![],
        };
        let lines = display_lines(&file, &root.join("dir_1"), &root.join("dir_2")).unwrap();
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|line| line.lhs_text.contains("#include")));
        assert!(lines.iter().all(|line| line.rhs_text.is_empty()));
    }

    #[test]
    fn display_lines_for_created_reads_from_dir_b() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sample_files");
        let file = DiffFile {
            path: "only_in_2.rs".to_owned(),
            language: "Rust".to_owned(),
            status: DiffStatus::Created,
            extra_info: None,
            aligned_lines: vec![],
        };
        let lines = display_lines(&file, &root.join("dir_1"), &root.join("dir_2")).unwrap();
        assert!(!lines.is_empty());
        assert!(lines.iter().any(|line| line.rhs_text.contains("fn main")));
        assert!(lines.iter().all(|line| line.lhs_text.is_empty()));
    }
}
