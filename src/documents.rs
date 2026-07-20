use chrono::NaiveDate;
use serde::Serialize;
use serde_json::{Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct CanonicalNode {
    pub path: PathBuf,
    pub frontmatter: Map<String, Value>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum EdgeType {
    Origin,
    Supersedes,
    Related,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Edge {
    #[serde(rename = "type")]
    pub edge_type: EdgeType,
    pub source: String,
    pub target: String,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    // Fixed wire-contract value reserved for later advisory lint rules.
    #[allow(dead_code)]
    Warning,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum FindingCode {
    MissingEntrypoint,
    MalformedFolderRef,
    FrontmatterMissing,
    FrontmatterUnclosed,
    FrontmatterSyntax,
    DuplicateFrontmatterKey,
    MissingRequiredField,
    InvalidRequiredField,
    UnsupportedType,
    ForbiddenStatus,
    IdFolderMismatch,
    DuplicateId,
    InvalidEdgeList,
    DuplicateEdge,
    SelfEdge,
    DanglingEdge,
    MissingEdgeReason,
    UnreadableEntry,
    UnreadableRoot,
    StaleInboxMessage,
    StaleCaptureNote,
    InvalidInboxEnvelope,
    UnreadableInboxMessage,
    UnreadableCaptureNote,
    RelinkUnresolvedRef,
    RelinkAmbiguousRef,
    RelinkMissingInternalTarget,
    RelinkConcurrentChange,
    RelinkWriteFailed,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Finding {
    pub code: FindingCode,
    pub severity: Severity,
    pub message: String,
    pub id: Option<String>,
    pub path: Option<PathBuf>,
    pub line: Option<usize>,
}

impl Finding {
    pub(crate) fn error(
        code: FindingCode,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            id: None,
            path,
            line: None,
        }
    }

    pub(crate) fn warning(
        code: FindingCode,
        message: impl Into<String>,
        path: Option<PathBuf>,
    ) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            id: None,
            path,
            line: None,
        }
    }

    pub fn affects_completeness(&self) -> bool {
        matches!(
            self.code,
            FindingCode::MissingEntrypoint
                | FindingCode::FrontmatterMissing
                | FindingCode::FrontmatterUnclosed
                | FindingCode::FrontmatterSyntax
                | FindingCode::DuplicateFrontmatterKey
                | FindingCode::UnsupportedType
                | FindingCode::DuplicateId
                | FindingCode::InvalidEdgeList
                | FindingCode::DanglingEdge
                | FindingCode::UnreadableEntry
                | FindingCode::UnreadableRoot
        ) || (matches!(
            self.code,
            FindingCode::MissingRequiredField | FindingCode::InvalidRequiredField
        ) && self.message.contains("id"))
    }

    pub fn is_operational(&self) -> bool {
        matches!(
            self.code,
            FindingCode::UnreadableEntry
                | FindingCode::UnreadableRoot
                | FindingCode::UnreadableInboxMessage
                | FindingCode::UnreadableCaptureNote
                | FindingCode::RelinkConcurrentChange
                | FindingCode::RelinkWriteFailed
        )
    }
}

#[derive(Clone, Debug)]
pub struct DocumentRecord {
    pub node: CanonicalNode,
    pub source_kind: CanonicalSourceKind,
    /// Folder owner for task/review records; the file itself for seed/topic.
    pub owner_path: PathBuf,
    pub sort_key: RecordSortKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanonicalSourceKind {
    TaskOwner,
    Seed,
    Topic,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecordSortKey {
    Task(String),
    File(String),
}

#[derive(Clone, Debug)]
pub struct DocumentSources {
    pub task_roots: Vec<PathBuf>,
    pub seed_root: PathBuf,
    pub topic_roots: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct DocumentIndex {
    pub nodes: BTreeMap<String, DocumentRecord>,
    pub edges: Vec<Edge>,
    pub findings: Vec<Finding>,
}

impl DocumentIndex {
    pub fn graph_findings(&self) -> Vec<Finding> {
        self.findings
            .iter()
            .filter(|finding| finding.affects_completeness())
            .cloned()
            .collect()
    }

    pub fn complete(&self) -> bool {
        !self.findings.iter().any(Finding::affects_completeness)
    }

    pub fn operationally_complete(&self) -> bool {
        !self.findings.iter().any(Finding::is_operational)
    }
}

#[derive(Debug)]
struct ParsedDocument {
    id: String,
    record: DocumentRecord,
    body: String,
    key_lines: HashMap<String, usize>,
}

const EDGE_TYPES: [(EdgeType, &str); 3] = [
    (EdgeType::Origin, "origin"),
    (EdgeType::Supersedes, "supersedes"),
    (EdgeType::Related, "related"),
];
const REQUIRED_FIELDS: [&str; 4] = ["type", "id", "title", "timestamp"];

pub fn scan_sources(sources: &DocumentSources) -> DocumentIndex {
    let mut index = DocumentIndex::default();
    let mut parsed = Vec::new();

    scan_task_roots(&sources.task_roots, &mut parsed, &mut index.findings);
    scan_seed_root(&sources.seed_root, &mut parsed, &mut index.findings);
    for root in &sources.topic_roots {
        scan_topic_root(root, &mut parsed, &mut index.findings);
    }

    finish_index(parsed, index)
}

/// Task-only compatibility helper retained for focused document unit tests.
#[cfg(test)]
pub fn scan(roots: &[PathBuf]) -> DocumentIndex {
    let mut index = DocumentIndex::default();
    let mut parsed = Vec::new();
    scan_task_roots(roots, &mut parsed, &mut index.findings);
    finish_index(parsed, index)
}

fn scan_task_roots(
    roots: &[PathBuf],
    parsed: &mut Vec<ParsedDocument>,
    findings: &mut Vec<Finding>,
) {
    for root in roots {
        let entries = match fs::read_dir(root) {
            Ok(entries) => entries,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                findings.push(Finding::error(
                    FindingCode::UnreadableRoot,
                    format!("cannot read configured task root: {err}"),
                    Some(absolute_path(root)),
                ));
                continue;
            }
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    findings.push(Finding::error(
                        FindingCode::UnreadableRoot,
                        format!("cannot read configured task-root entry: {err}"),
                        Some(absolute_path(root)),
                    ));
                    continue;
                }
            };
            let path = entry.path();
            if !fs::metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some((folder_id, folder_ref)) = folder_parts(&name) else {
                continue;
            };
            let entrypoint = path.join("CURRENT_STATE.md");
            let entrypoint = absolute_path(&entrypoint);
            let contents = match fs::read_to_string(&entrypoint) {
                Ok(contents) => contents,
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    findings.push(Finding::error(
                        FindingCode::MissingEntrypoint,
                        "task folder has no CURRENT_STATE.md",
                        Some(entrypoint),
                    ));
                    continue;
                }
                Err(err) => {
                    findings.push(Finding::error(
                        FindingCode::UnreadableEntry,
                        format!("cannot read task entrypoint: {err}"),
                        Some(entrypoint),
                    ));
                    continue;
                }
            };

            match parse_document(
                &entrypoint,
                SourcePolicy::Task {
                    folder_id: &folder_id,
                    folder_ref: &folder_ref,
                    owner_path: &path,
                },
                &contents,
            ) {
                Ok((document, mut document_findings)) => {
                    findings.append(&mut document_findings);
                    parsed.push(document);
                }
                Err(mut document_findings) => findings.append(&mut document_findings),
            }
        }
    }
}

fn scan_seed_root(root: &Path, parsed: &mut Vec<ParsedDocument>, findings: &mut Vec<Finding>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return,
        Err(err) => {
            findings.push(Finding::error(
                FindingCode::UnreadableRoot,
                format!("cannot read configured seed root: {err}"),
                Some(absolute_path(root)),
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                findings.push(Finding::error(
                    FindingCode::UnreadableRoot,
                    format!("cannot read configured seed-root entry: {err}"),
                    Some(absolute_path(root)),
                ));
                continue;
            }
        };
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md")
            || !fs::metadata(&path).is_ok_and(|metadata| metadata.is_file())
        {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|value| value.to_str()) else {
            continue;
        };
        let Some((_, file_ref)) = folder_parts(stem) else {
            continue;
        };
        scan_canonical_file(
            &path,
            SourcePolicy::Seed {
                file_ref: &file_ref,
            },
            parsed,
            findings,
        );
    }
}

fn scan_topic_root(root: &Path, parsed: &mut Vec<ParsedDocument>, findings: &mut Vec<Finding>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return,
        Err(err) => {
            findings.push(Finding::error(
                FindingCode::UnreadableRoot,
                format!("cannot read configured topic root: {err}"),
                Some(absolute_path(root)),
            ));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(err) => {
                findings.push(Finding::error(
                    FindingCode::UnreadableRoot,
                    format!("cannot read configured topic-root entry: {err}"),
                    Some(absolute_path(root)),
                ));
                continue;
            }
        };
        let path = entry.path();
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_dir() => scan_topic_root(&path, parsed, findings),
            Ok(metadata)
                if metadata.is_file()
                    && path.extension().and_then(|value| value.to_str()) == Some("md") =>
            {
                scan_canonical_file(&path, SourcePolicy::Topic, parsed, findings);
            }
            Ok(_) => {}
            Err(err) => findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                format!("cannot inspect topic document: {err}"),
                Some(absolute_path(&path)),
            )),
        }
    }
}

fn scan_canonical_file(
    path: &Path,
    policy: SourcePolicy<'_>,
    parsed: &mut Vec<ParsedDocument>,
    findings: &mut Vec<Finding>,
) {
    let path = absolute_path(path);
    let contents = match fs::read_to_string(&path) {
        Ok(contents) => contents,
        Err(err) => {
            findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                format!("cannot read canonical document: {err}"),
                Some(path),
            ));
            return;
        }
    };
    match parse_document(&path, policy, &contents) {
        Ok((document, mut document_findings)) => {
            findings.append(&mut document_findings);
            parsed.push(document);
        }
        Err(mut document_findings) => findings.append(&mut document_findings),
    }
}

fn finish_index(parsed: Vec<ParsedDocument>, mut index: DocumentIndex) -> DocumentIndex {
    let mut by_id: BTreeMap<String, Vec<ParsedDocument>> = BTreeMap::new();
    for document in parsed {
        by_id.entry(document.id.clone()).or_default().push(document);
    }
    let mut unique = Vec::new();
    for (id, mut documents) in by_id {
        if documents.len() > 1 {
            for document in documents {
                let mut finding = Finding::error(
                    FindingCode::DuplicateId,
                    format!("canonical id {id} is authored by more than one entrypoint"),
                    Some(document.node_path()),
                );
                finding.id = Some(id.clone());
                finding.line = document.key_lines.get("id").copied();
                index.findings.push(finding);
            }
        } else {
            unique.push(documents.pop().expect("one document"));
        }
    }

    let ids = unique
        .iter()
        .map(|document| document.id.clone())
        .collect::<BTreeSet<_>>();
    for document in unique {
        collect_edges(&document, &ids, &mut index);
        index.nodes.insert(document.id.clone(), document.record);
    }

    index.edges.sort_by(|left, right| {
        (&left.edge_type, &left.source, &left.target).cmp(&(
            &right.edge_type,
            &right.source,
            &right.target,
        ))
    });
    index.findings.sort_by(compare_findings);
    index
}

impl ParsedDocument {
    fn node_path(&self) -> PathBuf {
        self.record.node.path.clone()
    }
}

#[derive(Clone, Copy)]
enum SourcePolicy<'a> {
    Task {
        folder_id: &'a str,
        folder_ref: &'a str,
        owner_path: &'a Path,
    },
    Seed {
        file_ref: &'a str,
    },
    Topic,
}

fn parse_document(
    path: &Path,
    source: SourcePolicy<'_>,
    contents: &str,
) -> Result<(ParsedDocument, Vec<Finding>), Vec<Finding>> {
    let mut findings = Vec::new();
    let mut lines = contents.lines();
    if lines.next() != Some("---") {
        return Err(vec![Finding::error(
            FindingCode::FrontmatterMissing,
            "frontmatter must begin at byte zero",
            Some(path.to_path_buf()),
        )]);
    }

    let mut frontmatter = Map::new();
    let mut key_lines = HashMap::new();
    let mut body_start = None;
    for (offset, line) in contents.lines().enumerate().skip(1) {
        let line_number = offset + 1;
        if line == "---" {
            body_start = Some(offset + 1);
            break;
        }
        if line.trim().is_empty() {
            continue;
        }
        let Some((key, raw_value)) = line.split_once(':') else {
            let mut finding = Finding::error(
                FindingCode::FrontmatterSyntax,
                "frontmatter line must contain a key and JSON value",
                Some(path.to_path_buf()),
            );
            finding.line = Some(line_number);
            return Err(vec![finding]);
        };
        let key = key.trim();
        if key.is_empty() || raw_value.trim().is_empty() {
            let mut finding = Finding::error(
                FindingCode::FrontmatterSyntax,
                "frontmatter key and JSON value must be nonempty",
                Some(path.to_path_buf()),
            );
            finding.line = Some(line_number);
            return Err(vec![finding]);
        }
        if frontmatter.contains_key(key) {
            let mut finding = Finding::error(
                FindingCode::DuplicateFrontmatterKey,
                format!("frontmatter key {key} appears more than once"),
                Some(path.to_path_buf()),
            );
            finding.line = Some(line_number);
            return Err(vec![finding]);
        }
        let value = match serde_json::from_str::<Value>(raw_value.trim()) {
            Ok(value) => value,
            Err(err) => {
                let mut finding = Finding::error(
                    FindingCode::FrontmatterSyntax,
                    format!("frontmatter value is not JSON-compatible: {err}"),
                    Some(path.to_path_buf()),
                );
                finding.line = Some(line_number);
                return Err(vec![finding]);
            }
        };
        key_lines.insert(key.to_string(), line_number);
        frontmatter.insert(key.to_string(), value);
    }
    let Some(body_start) = body_start else {
        return Err(vec![Finding::error(
            FindingCode::FrontmatterUnclosed,
            "frontmatter has no closing delimiter",
            Some(path.to_path_buf()),
        )]);
    };

    let id = match nonempty_string(frontmatter.get("id")) {
        Some(id) => id.to_string(),
        None => {
            let code = if frontmatter.contains_key("id") {
                FindingCode::InvalidRequiredField
            } else {
                FindingCode::MissingRequiredField
            };
            let mut finding = Finding::error(
                code,
                "required field id must be a nonempty string",
                Some(path.to_path_buf()),
            );
            finding.line = key_lines.get("id").copied();
            return Err(vec![finding]);
        }
    };

    for field in REQUIRED_FIELDS {
        if field == "id" {
            continue;
        }
        match nonempty_string(frontmatter.get(field)) {
            Some(_) => {}
            None => {
                let code = if frontmatter.contains_key(field) {
                    FindingCode::InvalidRequiredField
                } else {
                    FindingCode::MissingRequiredField
                };
                let mut finding = Finding::error(
                    code,
                    format!("required field {field} must be a nonempty string"),
                    Some(path.to_path_buf()),
                );
                finding.id = Some(id.clone());
                finding.line = key_lines.get(field).copied();
                findings.push(finding);
            }
        }
    }

    let source_kind = match (source, nonempty_string(frontmatter.get("type"))) {
        (SourcePolicy::Task { .. }, Some("task" | "review")) => CanonicalSourceKind::TaskOwner,
        (SourcePolicy::Seed { .. }, Some("seed")) => CanonicalSourceKind::Seed,
        (SourcePolicy::Topic, Some("topic")) => CanonicalSourceKind::Topic,
        other => {
            let description = other.1.unwrap_or("missing or invalid");
            let source_name = match source {
                SourcePolicy::Task { .. } => "task root",
                SourcePolicy::Seed { .. } => "seed root",
                SourcePolicy::Topic => "topic root",
            };
            let mut finding = Finding::error(
                FindingCode::UnsupportedType,
                format!("source type {description} is not supported in a {source_name}"),
                Some(path.to_path_buf()),
            );
            finding.id = Some(id);
            finding.line = key_lines.get("type").copied();
            return Err(vec![finding]);
        }
    };

    if let Some(timestamp) = nonempty_string(frontmatter.get("timestamp"))
        && NaiveDate::parse_from_str(timestamp, "%Y-%m-%d").is_err()
    {
        let mut finding = Finding::error(
            FindingCode::InvalidRequiredField,
            "required field timestamp must be a real YYYY-MM-DD date",
            Some(path.to_path_buf()),
        );
        finding.id = Some(id.clone());
        finding.line = key_lines.get("timestamp").copied();
        findings.push(finding);
    }

    if frontmatter.contains_key("status") {
        let mut finding = Finding::error(
            FindingCode::ForbiddenStatus,
            "canonical frontmatter must not contain status",
            Some(path.to_path_buf()),
        );
        finding.id = Some(id.clone());
        finding.line = key_lines.get("status").copied();
        findings.push(finding);
    }
    match source {
        SourcePolicy::Task { folder_ref, .. } => {
            if !valid_folder_ref(folder_ref) {
                let mut finding = Finding::error(
                    FindingCode::MalformedFolderRef,
                    format!("folder ref {folder_ref} is not a legacy or current STM id"),
                    Some(path.to_path_buf()),
                );
                finding.id = Some(id.clone());
                findings.push(finding);
            }
            if id != folder_ref {
                let mut finding = Finding::error(
                    FindingCode::IdFolderMismatch,
                    format!("frontmatter id {id} does not match folder ref {folder_ref}"),
                    Some(path.to_path_buf()),
                );
                finding.id = Some(id.clone());
                finding.line = key_lines.get("id").copied();
                findings.push(finding);
            }
        }
        SourcePolicy::Seed { file_ref } => {
            if !valid_folder_ref(file_ref) {
                let mut finding = Finding::error(
                    FindingCode::MalformedFolderRef,
                    format!("seed ref {file_ref} is not a legacy or current STM id"),
                    Some(path.to_path_buf()),
                );
                finding.id = Some(id.clone());
                findings.push(finding);
            }
            if id != file_ref {
                let mut finding = Finding::error(
                    FindingCode::IdFolderMismatch,
                    format!("frontmatter id {id} does not match seed filename ref {file_ref}"),
                    Some(path.to_path_buf()),
                );
                finding.id = Some(id.clone());
                finding.line = key_lines.get("id").copied();
                findings.push(finding);
            }
        }
        SourcePolicy::Topic => {}
    }

    let body = contents
        .lines()
        .skip(body_start)
        .collect::<Vec<_>>()
        .join("\n");
    let owner_path = match source {
        SourcePolicy::Task { owner_path, .. } => absolute_path(owner_path),
        SourcePolicy::Seed { .. } | SourcePolicy::Topic => path.to_path_buf(),
    };
    let sort_key = match source {
        SourcePolicy::Task { folder_id, .. } => RecordSortKey::Task(folder_id.to_string()),
        SourcePolicy::Seed { .. } | SourcePolicy::Topic => RecordSortKey::File(
            nonempty_string(frontmatter.get("timestamp"))
                .unwrap_or_default()
                .to_string(),
        ),
    };
    Ok((
        ParsedDocument {
            id,
            record: DocumentRecord {
                node: CanonicalNode {
                    path: path.to_path_buf(),
                    frontmatter,
                },
                source_kind,
                owner_path,
                sort_key,
            },
            body,
            key_lines,
        },
        findings,
    ))
}

fn collect_edges(document: &ParsedDocument, ids: &BTreeSet<String>, index: &mut DocumentIndex) {
    for (edge_type, edge_key) in EDGE_TYPES {
        let Some(value) = document.record.node.frontmatter.get(edge_key) else {
            continue;
        };
        let Some(values) = value.as_array() else {
            push_edge_finding(
                index,
                document,
                edge_key,
                FindingCode::InvalidEdgeList,
                "relationship field must be an array of strings",
            );
            continue;
        };
        if values.iter().any(|value| !value.is_string()) {
            push_edge_finding(
                index,
                document,
                edge_key,
                FindingCode::InvalidEdgeList,
                "relationship field must contain only strings",
            );
            continue;
        }
        let mut targets = BTreeSet::new();
        for value in values {
            let target = value.as_str().expect("validated string relation value");
            if !targets.insert(target.to_string()) {
                push_edge_finding(
                    index,
                    document,
                    edge_key,
                    FindingCode::DuplicateEdge,
                    format!("{edge_key} repeats target {target}"),
                );
                continue;
            }
            if target == document.id {
                push_edge_finding(
                    index,
                    document,
                    edge_key,
                    FindingCode::SelfEdge,
                    format!("{edge_key} targets its own source id"),
                );
                continue;
            }
            if !ids.contains(target) {
                push_edge_finding(
                    index,
                    document,
                    edge_key,
                    FindingCode::DanglingEdge,
                    format!("{edge_key} target {target} does not exist"),
                );
                continue;
            }
            index.edges.push(Edge {
                edge_type,
                source: document.id.clone(),
                target: target.to_string(),
            });
            if !related_section_contains(&document.body, target) {
                push_edge_finding(
                    index,
                    document,
                    edge_key,
                    FindingCode::MissingEdgeReason,
                    format!("related section does not mention target {target}"),
                );
            }
        }
    }
}

fn push_edge_finding(
    index: &mut DocumentIndex,
    document: &ParsedDocument,
    edge_type: &str,
    code: FindingCode,
    message: impl Into<String>,
) {
    let mut finding = Finding::error(code, message, Some(document.node_path()));
    finding.id = Some(document.id.clone());
    finding.line = document.key_lines.get(edge_type).copied();
    index.findings.push(finding);
}

fn related_section_contains(body: &str, target: &str) -> bool {
    let mut in_related = false;
    for line in body.lines() {
        if line.starts_with("## ") {
            if in_related {
                break;
            }
            in_related = line.starts_with("## Related");
            continue;
        }
        if in_related && line.contains(target) {
            return true;
        }
    }
    false
}

fn nonempty_string(value: Option<&Value>) -> Option<&str> {
    value
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}

fn folder_parts(name: &str) -> Option<(String, String)> {
    let mut parts = name.split('_');
    let period = parts.next()?;
    let folder_ref = parts.next()?;
    if period.len() != 6
        || !period.bytes().all(|byte| byte.is_ascii_digit())
        || folder_ref.is_empty()
    {
        return None;
    }
    Some((name.to_string(), folder_ref.to_string()))
}

fn valid_folder_ref(value: &str) -> bool {
    let bytes = value.as_bytes();
    (bytes.len() == 4
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()))
        || (bytes.len() == 5
            && bytes[0] == b's'
            && bytes[1..]
                .iter()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()))
}

fn absolute_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}

pub(crate) fn compare_findings(left: &Finding, right: &Finding) -> Ordering {
    (&left.path, &left.line, &left.code, &left.id).cmp(&(
        &right.path,
        &right.line,
        &right.code,
        &right.id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(root: &Path, folder: &str, frontmatter: &str, body: &str) {
        let dir = root.join(folder);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("CURRENT_STATE.md"),
            format!("---\n{frontmatter}---\n{body}"),
        )
        .unwrap();
    }

    #[test]
    fn preserves_unknown_values_and_indexes_legacy_and_malformed_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        entry(
            &root,
            "202401_816d_old",
            "type: \"task\"\nid: \"816d\"\ntitle: \"Old\"\ntimestamp: \"2024-01-01\"\nextension: {\"nested\": true}\n",
            "",
        );
        entry(
            &root,
            "202402_sdmr192_review",
            "type: \"review\"\nid: \"sdmr192\"\ntitle: \"Review\"\ntimestamp: \"2024-02-01\"\n",
            "",
        );
        let index = scan(&[root]);
        assert_eq!(index.nodes.len(), 2);
        assert_eq!(
            index.nodes["816d"].node.frontmatter["extension"]["nested"],
            true
        );
        assert!(
            index
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::MalformedFolderRef
                    && finding.id.as_deref() == Some("sdmr192"))
        );
        assert!(index.complete());
    }

    #[test]
    fn detects_duplicate_keys_before_mapping_insertion() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        entry(
            &root,
            "202401_sa2a7_bad",
            "type: \"task\"\nid: \"sa2a7\"\nid: \"other\"\ntitle: \"Bad\"\ntimestamp: \"2024-01-01\"\n",
            "",
        );
        let index = scan(&[root]);
        assert!(index.nodes.is_empty());
        assert_eq!(index.findings[0].code, FindingCode::DuplicateFrontmatterKey);
        assert!(!index.complete());
    }

    #[test]
    fn normalizes_duplicate_and_self_edges_without_marking_graph_incomplete() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        entry(
            &root,
            "202401_sa2a7_first",
            "type: \"task\"\nid: \"sa2a7\"\ntitle: \"First\"\ntimestamp: \"2024-01-01\"\nrelated: [\"sb3b8\", \"sb3b8\", \"sa2a7\"]\n",
            "## Related work\n- sb3b8\n",
        );
        entry(
            &root,
            "202402_sb3b8_second",
            "type: \"task\"\nid: \"sb3b8\"\ntitle: \"Second\"\ntimestamp: \"2024-02-01\"\n",
            "",
        );
        let index = scan(&[root]);
        assert_eq!(index.edges.len(), 1);
        assert!(
            index
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::DuplicateEdge)
        );
        assert!(
            index
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::SelfEdge)
        );
        assert!(index.complete());
    }

    #[test]
    fn missing_entrypoints_and_duplicate_ids_are_omissions() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir_all(root.join("202401_sa2a7_missing")).unwrap();
        for folder in ["202402_sb3b8_one", "202403_sb3b8_two"] {
            entry(
                &root,
                folder,
                "type: \"task\"\nid: \"sb3b8\"\ntitle: \"Same\"\ntimestamp: \"2024-02-01\"\n",
                "",
            );
        }
        let index = scan(&[root]);
        assert!(index.nodes.is_empty());
        assert_eq!(
            index
                .findings
                .iter()
                .filter(|finding| finding.code == FindingCode::DuplicateId)
                .count(),
            2
        );
        assert!(!index.complete());
    }

    #[test]
    fn unsupported_or_missing_type_and_mixed_edge_lists_are_omitted() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        entry(
            &root,
            "202401_sa2a7_missing-type",
            "id: \"sa2a7\"\ntitle: \"Missing type\"\ntimestamp: \"2024-01-01\"\n",
            "",
        );
        entry(
            &root,
            "202402_sb3b8_source",
            "type: \"task\"\nid: \"sb3b8\"\ntitle: \"Source\"\ntimestamp: \"2024-02-01\"\nrelated: [\"sc4c9\", 7]\n",
            "## Related\n- sc4c9\n",
        );
        entry(
            &root,
            "202403_sc4c9_target",
            "type: \"task\"\nid: \"sc4c9\"\ntitle: \"Target\"\ntimestamp: \"2024-03-01\"\n",
            "",
        );
        let index = scan(&[root]);
        assert!(!index.nodes.contains_key("sa2a7"));
        assert!(index.edges.is_empty());
        assert!(
            index
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::UnsupportedType)
        );
        assert!(
            index
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::InvalidEdgeList)
        );
        assert!(!index.complete());
    }

    #[test]
    fn findings_sort_null_locations_before_paths() {
        let mut findings = [
            Finding::error(
                FindingCode::UnreadableRoot,
                "with path",
                Some(PathBuf::from("/a")),
            ),
            Finding::error(FindingCode::DanglingEdge, "without path", None),
        ];
        findings.sort_by(compare_findings);
        assert_eq!(findings[0].path, None);
    }
}
