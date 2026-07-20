use crate::documents::{
    self, CanonicalNode, CanonicalSourceKind, Finding, FindingCode, compare_findings,
};
use crate::scan;
use crate::walk;
use anyhow::{Result, bail};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use super::{is_reserved_note_file, load_project_config};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchOwnerKind {
    Canonical,
    Note,
    Unindexed,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SearchRank {
    Id,
    Title,
    Metadata,
    Path,
    Body,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchExcerpt {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub text: Option<String>,
    pub truncated: bool,
    #[serde(skip)]
    priority: u8,
}

#[derive(Clone, Debug, Serialize)]
pub struct SearchResult {
    pub owner_kind: SearchOwnerKind,
    pub path: PathBuf,
    pub node: Option<CanonicalNode>,
    pub rank: SearchRank,
    pub match_count: usize,
    pub excerpts: Vec<SearchExcerpt>,
}

#[derive(Debug, Serialize)]
pub struct SearchResults {
    pub complete: bool,
    pub total: usize,
    pub results: Vec<SearchResult>,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum OwnerKey {
    Canonical(String),
    Note(PathBuf),
    Unindexed(PathBuf),
}

struct Group {
    result: SearchResult,
    matching_files: BTreeSet<PathBuf>,
    paths: Vec<PathBuf>,
}

pub fn cmd_search(terms: Vec<String>, limit: usize, cwd: &Path) -> Result<SearchResults> {
    if terms.is_empty() || terms.iter().any(|term| term.is_empty()) {
        bail!("search requires at least one nonempty term");
    }
    if limit == 0 {
        bail!("search limit must be positive");
    }
    let terms = terms
        .into_iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    let config = load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    let mut findings = index
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.code,
                FindingCode::UnreadableEntry | FindingCode::UnreadableRoot
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut complete = findings.is_empty();
    let mut usable = false;
    let mut candidate_paths = BTreeSet::new();

    for root in &config.scan_roots {
        let mut exclusions = vec![config.seed_root.clone(), config.note_root.clone()];
        exclusions.extend(config.topic_roots.iter().cloned());
        exclusions.extend(
            config
                .scan_roots
                .iter()
                .filter(|other| *other != root && other.starts_with(root))
                .cloned(),
        );
        let outcome = walk::walk_files(root, &exclusions, true);
        usable |= outcome.usable;
        complete &= outcome.findings.is_empty();
        findings.extend(outcome.findings);
        candidate_paths.extend(outcome.paths);
    }
    for root in &config.topic_roots {
        let mut exclusions = vec![config.seed_root.clone(), config.note_root.clone()];
        exclusions.extend(config.scan_roots.iter().cloned());
        exclusions.extend(
            config
                .topic_roots
                .iter()
                .filter(|other| *other != root && other.starts_with(root))
                .cloned(),
        );
        let outcome = walk::walk_files(root, &exclusions, false);
        usable |= outcome.usable;
        complete &= outcome.findings.is_empty();
        findings.extend(outcome.findings);
        candidate_paths.extend(outcome.paths);
    }
    usable |= add_direct_files(
        &config.seed_root,
        false,
        &mut candidate_paths,
        &mut findings,
    );
    usable |= add_direct_files(&config.note_root, true, &mut candidate_paths, &mut findings);
    complete &= !findings.iter().any(|finding| {
        matches!(
            finding.code,
            FindingCode::UnreadableEntry
                | FindingCode::UnreadableRoot
                | FindingCode::UnreadableCaptureNote
        )
    });
    if !usable {
        bail!("search has no usable configured source");
    }

    let mut groups: BTreeMap<OwnerKey, Group> = BTreeMap::new();
    for path in candidate_paths {
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableEntry,
                    format!("cannot read searchable source: {err}"),
                    Some(path),
                ));
                continue;
            }
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let searchable_path = path.to_string_lossy().to_lowercase();
        let lower = text.to_lowercase();
        if !terms
            .iter()
            .all(|term| lower.contains(term) || searchable_path.contains(term))
        {
            continue;
        }
        let (owner_key, owner_kind, owner_path, node) = owner_for(&path, &config, &index);
        let count: usize = terms
            .iter()
            .map(|term| {
                nonoverlapping_count(&lower, term) + nonoverlapping_count(&searchable_path, term)
            })
            .sum();
        let group = groups.entry(owner_key).or_insert_with(|| Group {
            result: SearchResult {
                owner_kind,
                path: owner_path,
                node,
                rank: SearchRank::Body,
                match_count: 0,
                excerpts: Vec::new(),
            },
            matching_files: BTreeSet::new(),
            paths: Vec::new(),
        });
        group.result.match_count += count;
        group.matching_files.insert(path.clone());
        group.paths.push(path.clone());
        add_excerpts(
            &mut group.result.excerpts,
            &path,
            &text,
            &terms,
            &searchable_path,
        );
    }

    let mut ranked = groups
        .into_values()
        .map(|mut group| {
            group.result.rank = rank_group(&group.result, &group.paths, &terms);
            group.result.excerpts.sort_by(|left, right| {
                (left.priority, &left.path, left.line).cmp(&(
                    right.priority,
                    &right.path,
                    right.line,
                ))
            });
            group.result.excerpts.truncate(3);
            (group.matching_files.len(), group.result)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|(left_files, left), (right_files, right)| {
        left.rank
            .cmp(&right.rank)
            .then_with(|| right_files.cmp(left_files))
            .then_with(|| right.match_count.cmp(&left.match_count))
            .then_with(|| left.path.cmp(&right.path))
    });
    let total = ranked.len();
    let results = ranked
        .into_iter()
        .take(limit)
        .map(|(_, result)| result)
        .collect();
    findings.sort_by(compare_findings);
    Ok(SearchResults {
        complete,
        total,
        results,
        findings,
    })
}

fn add_direct_files(
    root: &Path,
    notes: bool,
    paths: &mut BTreeSet<PathBuf>,
    findings: &mut Vec<Finding>,
) -> bool {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return false,
        Err(err) => {
            findings.push(Finding::error(
                if notes {
                    FindingCode::UnreadableCaptureNote
                } else {
                    FindingCode::UnreadableRoot
                },
                format!("cannot read searchable typed root: {err}"),
                Some(root.to_path_buf()),
            ));
            return false;
        }
    };
    for entry in entries {
        match entry {
            Ok(entry) if entry.file_type().is_ok_and(|kind| kind.is_file()) => {
                let path = entry.path();
                if !notes || !is_reserved_note_file(&path) {
                    paths.insert(path);
                }
            }
            Ok(_) => {}
            Err(err) => findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                format!("cannot read typed source entry: {err}"),
                Some(root.to_path_buf()),
            )),
        }
    }
    true
}

fn owner_for(
    path: &Path,
    config: &super::ProjectConfig,
    index: &documents::DocumentIndex,
) -> (OwnerKey, SearchOwnerKind, PathBuf, Option<CanonicalNode>) {
    let mut task_records = index
        .nodes
        .iter()
        .filter(|(_, record)| record.source_kind == CanonicalSourceKind::TaskOwner)
        .collect::<Vec<_>>();
    task_records
        .sort_by_key(|(_, record)| std::cmp::Reverse(record.owner_path.components().count()));
    for (id, record) in task_records {
        if path.starts_with(&record.owner_path) {
            return (
                OwnerKey::Canonical(id.clone()),
                SearchOwnerKind::Canonical,
                record.node.path.clone(),
                Some(record.node.clone()),
            );
        }
    }
    for (id, record) in &index.nodes {
        if record.node.path == path {
            return (
                OwnerKey::Canonical(id.clone()),
                SearchOwnerKind::Canonical,
                record.node.path.clone(),
                Some(record.node.clone()),
            );
        }
    }
    if path.parent() == Some(config.note_root.as_path()) {
        return (
            OwnerKey::Note(path.to_path_buf()),
            SearchOwnerKind::Note,
            path.to_path_buf(),
            None,
        );
    }
    for root in &config.scan_roots {
        if let Ok(relative) = path.strip_prefix(root)
            && let Some(first) = relative.components().next()
        {
            let owner = root.join(first.as_os_str());
            if owner != *path
                && owner
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(scan::split_task_folder_name)
                    .is_some()
            {
                return (
                    OwnerKey::Unindexed(owner.clone()),
                    SearchOwnerKind::Unindexed,
                    owner,
                    None,
                );
            }
        }
    }
    (
        OwnerKey::Unindexed(path.to_path_buf()),
        SearchOwnerKind::Unindexed,
        path.to_path_buf(),
        None,
    )
}

fn rank_group(result: &SearchResult, paths: &[PathBuf], terms: &[String]) -> SearchRank {
    if let Some(node) = &result.node {
        let id = node.frontmatter["id"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        if terms.len() == 1 && terms[0] == id {
            return SearchRank::Id;
        }
        let title = node.frontmatter["title"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        if terms.iter().all(|term| title.contains(term)) {
            return SearchRank::Title;
        }
        let metadata = [
            node.frontmatter.get("tags"),
            node.frontmatter.get("description"),
        ]
        .into_iter()
        .flatten()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase();
        if terms.iter().all(|term| metadata.contains(term)) {
            return SearchRank::Metadata;
        }
    }
    if paths.iter().any(|path| {
        let path = path.to_string_lossy().to_lowercase();
        terms.iter().all(|term| path.contains(term))
    }) {
        SearchRank::Path
    } else {
        SearchRank::Body
    }
}

fn add_excerpts(
    excerpts: &mut Vec<SearchExcerpt>,
    path: &Path,
    text: &str,
    terms: &[String],
    searchable_path: &str,
) {
    for (offset, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        if terms.iter().any(|term| lower.contains(term)) {
            let chars = line.chars().collect::<Vec<_>>();
            let truncated = chars.len() > 240;
            excerpts.push(SearchExcerpt {
                path: path.to_path_buf(),
                line: Some(offset + 1),
                text: Some(chars.into_iter().take(240).collect()),
                truncated,
                priority: excerpt_priority(line),
            });
        }
    }
    if excerpts.iter().all(|excerpt| excerpt.path != path)
        && terms.iter().all(|term| searchable_path.contains(term))
    {
        excerpts.push(SearchExcerpt {
            path: path.to_path_buf(),
            line: None,
            text: None,
            truncated: false,
            priority: 3,
        });
    }
}

fn excerpt_priority(line: &str) -> u8 {
    let trimmed = line.trim_start();
    if trimmed.starts_with("id:") {
        0
    } else if trimmed.starts_with("title:") {
        1
    } else if trimmed.starts_with("tags:") || trimmed.starts_with("description:") {
        2
    } else {
        4
    }
}

fn nonoverlapping_count(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    haystack.match_indices(needle).count()
}
