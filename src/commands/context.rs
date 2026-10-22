use crate::documents::{
    self, CanonicalNode, CanonicalSourceKind, Finding, FindingCode, compare_findings,
};
use anyhow::{Result, bail};
use chrono::{DateTime, NaiveDate, SecondsFormat, Utc};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{GraphDirection, GraphResult, ProjectConfig, cmd_graph, load_project_config};

#[derive(Debug, Serialize)]
pub struct InboxMessage {
    pub path: PathBuf,
    pub frontmatter: Map<String, Value>,
}

#[derive(Debug, Serialize)]
pub struct InboxResult {
    pub complete: bool,
    pub messages: Vec<InboxMessage>,
    pub findings: Vec<Finding>,
}

#[derive(Debug, Serialize)]
pub struct ContextResult {
    pub complete: bool,
    pub node: CanonicalNode,
    pub graph: GraphResult,
    pub inbox: InboxResult,
}

#[derive(Debug, Serialize)]
pub struct CaptureNote {
    pub path: PathBuf,
    pub modified: String,
    pub bytes: u64,
}

#[derive(Debug, Serialize)]
pub struct CapturesResult {
    pub complete: bool,
    pub notes: Vec<CaptureNote>,
    pub seeds: Vec<CanonicalNode>,
    pub findings: Vec<Finding>,
}

pub fn cmd_context(id: &str, depth: Option<usize>, cwd: &Path) -> Result<ContextResult> {
    let config = load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    let Some(record) = index.nodes.get(id) else {
        bail!("no unique STM entrypoint has canonical id {id}");
    };
    if record.source_kind != CanonicalSourceKind::TaskOwner {
        bail!("context requires a folder-backed task or review id: {id}");
    }
    let node = record.node.clone();
    let inbox = scan_inbox(
        &record.owner_path,
        config.stale_after_days,
        Utc::now().date_naive(),
    );
    let graph = cmd_graph(id, depth, GraphDirection::Both, &[], cwd)?;
    Ok(ContextResult {
        complete: graph.complete && inbox.complete,
        node,
        graph,
        inbox,
    })
}

pub fn cmd_captures(cwd: &Path) -> Result<CapturesResult> {
    let config = load_project_config(cwd)?;
    Ok(captures_with_config(&config, Utc::now().date_naive()))
}

pub(crate) fn captures_with_config(config: &ProjectConfig, today: NaiveDate) -> CapturesResult {
    let (notes, mut findings, notes_complete) =
        scan_notes(&config.note_root, config.stale_after_days, today);
    let seed_index = documents::scan_sources(&config.document_sources());
    let mut seeds = seed_index
        .nodes
        .values()
        .filter(|record| record.source_kind == CanonicalSourceKind::Seed)
        .map(|record| record.node.clone())
        .collect::<Vec<_>>();
    seeds.sort_by(|left, right| {
        let left_timestamp = left.frontmatter["timestamp"].as_str().unwrap_or_default();
        let right_timestamp = right.frontmatter["timestamp"].as_str().unwrap_or_default();
        right_timestamp
            .cmp(left_timestamp)
            .then_with(|| {
                left.frontmatter["id"]
                    .as_str()
                    .cmp(&right.frontmatter["id"].as_str())
            })
            .then_with(|| left.path.cmp(&right.path))
    });
    let seed_findings = seed_index
        .findings
        .into_iter()
        .filter(|finding| {
            finding
                .path
                .as_ref()
                .is_some_and(|path| path.starts_with(&config.seed_root))
        })
        .collect::<Vec<_>>();
    let seed_complete = !seed_findings
        .iter()
        .any(|finding| finding.severity == documents::Severity::Error);
    findings.extend(seed_findings);
    findings.sort_by(compare_findings);
    CapturesResult {
        complete: notes_complete && seed_complete,
        notes,
        seeds,
        findings,
    }
}

pub(crate) fn scan_notes(
    note_root: &Path,
    stale_after_days: u64,
    today: NaiveDate,
) -> (Vec<CaptureNote>, Vec<Finding>, bool) {
    let entries = match fs::read_dir(note_root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return (Vec::new(), Vec::new(), true),
        Err(err) => {
            return (
                Vec::new(),
                vec![Finding::error(
                    FindingCode::UnreadableCaptureNote,
                    format!("cannot read capture note root: {err}"),
                    Some(note_root.to_path_buf()),
                )],
                false,
            );
        }
    };
    let mut notes = Vec::new();
    let mut findings = Vec::new();
    let mut complete = true;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableCaptureNote,
                    format!("cannot read capture note entry: {err}"),
                    Some(note_root.to_path_buf()),
                ));
                continue;
            }
        };
        let path = entry.path();
        if !is_pending_note_file(&path) {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => continue,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableCaptureNote,
                    format!("cannot inspect capture note: {err}"),
                    Some(path),
                ));
                continue;
            }
        };
        let modified = match metadata.modified() {
            Ok(modified) => modified,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableCaptureNote,
                    format!("cannot read capture note mtime: {err}"),
                    Some(path),
                ));
                continue;
            }
        };
        let modified_utc = DateTime::<Utc>::from(modified);
        if is_stale(modified_utc.date_naive(), today, stale_after_days) {
            findings.push(Finding::warning(
                FindingCode::StaleCaptureNote,
                format!("capture note is at least {stale_after_days} UTC calendar days old"),
                Some(path.clone()),
            ));
        }
        notes.push((
            modified,
            CaptureNote {
                path,
                modified: modified_utc.to_rfc3339_opts(SecondsFormat::Secs, true),
                bytes: metadata.len(),
            },
        ));
    }
    notes.sort_by(|(left_time, left), (right_time, right)| {
        right_time
            .cmp(left_time)
            .then_with(|| left.path.cmp(&right.path))
    });
    findings.sort_by(compare_findings);
    (
        notes.into_iter().map(|(_, note)| note).collect(),
        findings,
        complete,
    )
}

pub(crate) fn is_pending_note_file(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name != "log.md" && !name.as_encoded_bytes().starts_with(b"."))
}

pub(crate) fn scan_inbox(
    owner_path: &Path,
    stale_after_days: u64,
    today: NaiveDate,
) -> InboxResult {
    let inbox = owner_path.join("inbox");
    let entries = match fs::read_dir(&inbox) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return InboxResult {
                complete: true,
                messages: Vec::new(),
                findings: Vec::new(),
            };
        }
        Err(err) => {
            return InboxResult {
                complete: false,
                messages: Vec::new(),
                findings: vec![Finding::error(
                    FindingCode::UnreadableInboxMessage,
                    format!("cannot read task inbox: {err}"),
                    Some(inbox),
                )],
            };
        }
    };
    let mut messages = Vec::new();
    let mut findings = Vec::new();
    let mut complete = true;
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableInboxMessage,
                    format!("cannot read inbox entry: {err}"),
                    Some(inbox.clone()),
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md")
            || !entry.file_type().is_ok_and(|kind| kind.is_file())
        {
            continue;
        }
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableInboxMessage,
                    format!("cannot read inbox message: {err}"),
                    Some(path),
                ));
                continue;
            }
        };
        let frontmatter = match parse_envelope(&contents) {
            Ok(frontmatter) => frontmatter,
            Err(message) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::InvalidInboxEnvelope,
                    message,
                    Some(path),
                ));
                continue;
            }
        };
        let date = frontmatter["date"]
            .as_str()
            .and_then(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").ok())
            .expect("validated inbox date");
        if is_stale(date, today, stale_after_days) {
            findings.push(Finding::warning(
                FindingCode::StaleInboxMessage,
                format!("inbox message is at least {stale_after_days} UTC calendar days old"),
                Some(path.clone()),
            ));
        }
        messages.push((date, InboxMessage { path, frontmatter }));
    }
    messages.sort_by(|(left_date, left), (right_date, right)| {
        left_date
            .cmp(right_date)
            .then_with(|| left.path.cmp(&right.path))
    });
    findings.sort_by(compare_findings);
    InboxResult {
        complete,
        messages: messages.into_iter().map(|(_, message)| message).collect(),
        findings,
    }
}

fn parse_envelope(contents: &str) -> Result<Map<String, Value>, String> {
    let mut lines = contents.lines();
    if lines.next() != Some("---") {
        return Err("inbox frontmatter must begin at byte zero".into());
    }
    let mut frontmatter = Map::new();
    let mut closed = false;
    let mut key_lines = HashMap::new();
    for (offset, line) in contents.lines().enumerate().skip(1) {
        if line == "---" {
            closed = true;
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let Some((key, raw)) = line.split_once(':') else {
            return Err(format!(
                "inbox frontmatter line {} has no colon",
                offset + 1
            ));
        };
        let key = key.trim();
        if key.is_empty() || raw.trim().is_empty() || key_lines.insert(key, offset + 1).is_some() {
            return Err(format!(
                "invalid inbox frontmatter key on line {}",
                offset + 1
            ));
        }
        let value = serde_json::from_str(raw.trim()).map_err(|err| {
            format!(
                "invalid inbox frontmatter value on line {}: {err}",
                offset + 1
            )
        })?;
        frontmatter.insert(key.to_string(), value);
    }
    if !closed {
        return Err("inbox frontmatter has no closing delimiter".into());
    }
    for field in ["from", "date", "subject"] {
        if frontmatter
            .get(field)
            .and_then(Value::as_str)
            .is_none_or(|value| value.is_empty())
        {
            return Err(format!("inbox field {field} must be a nonempty string"));
        }
    }
    if NaiveDate::parse_from_str(frontmatter["date"].as_str().unwrap(), "%Y-%m-%d").is_err() {
        return Err("inbox field date must be a real YYYY-MM-DD date".into());
    }
    Ok(frontmatter)
}

fn is_stale(date: NaiveDate, today: NaiveDate, stale_after_days: u64) -> bool {
    stale_after_days > 0 && today.signed_duration_since(date).num_days() >= stale_after_days as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stale_boundary_accepts_six_and_warns_at_seven_and_can_be_disabled() {
        let today = NaiveDate::from_ymd_opt(2026, 7, 13).unwrap();
        assert!(!is_stale(
            NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
            today,
            7
        ));
        assert!(is_stale(
            NaiveDate::from_ymd_opt(2026, 7, 6).unwrap(),
            today,
            7
        ));
        assert!(!is_stale(
            NaiveDate::from_ymd_opt(2020, 1, 1).unwrap(),
            today,
            0
        ));
    }
}
