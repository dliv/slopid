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
use unicode_segmentation::UnicodeSegmentation;

use super::{is_reserved_note_file, load_project_config};

/// Excerpt text is capped at this many Unicode scalar values, elision markers
/// included (contract 0005).
const EXCERPT_CHARS: usize = 240;

/// Leading context kept ahead of the matched term, so the match does not sit
/// flush against the left edge of the window.
const EXCERPT_LEAD: usize = EXCERPT_CHARS / 3;

/// How far a window start may look back for a word boundary before giving up.
const EXCERPT_WORD_SCAN: usize = 20;

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
            excerpts.push(SearchExcerpt {
                path: path.to_path_buf(),
                line: Some(offset + 1),
                text: Some(window_excerpt(line, &lower, terms)),
                truncated: line.chars().count() > EXCERPT_CHARS,
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

/// Excerpt text for one matching line: a capped window over the first match.
///
/// Keeping the leading `EXCERPT_CHARS` characters instead drops the match on
/// any longer line, which is the whole point of the excerpt. Measured over a
/// real corpus, 6 of 15 truncated excerpts carried neither search term. A
/// window that opens ahead of the match keeps it visible under the same cap.
///
/// A match near the end of the line yields a window shorter than the cap
/// rather than sliding left for more leading context: everything after the
/// match is shown either way, and a fixed window start is what keeps the match
/// provably inside the cap.
///
/// Both edges land on grapheme cluster boundaries whenever the mandatory span
/// fits the content budget, so the window does not orphan a combining mark or
/// split a ZWJ emoji sequence. When it cannot fit, the scalar cap and required
/// source span take precedence. The cap counts Unicode scalars, per contract
/// 0005.
fn window_excerpt(line: &str, lowered_line: &str, terms: &[String]) -> String {
    let chars = line.chars().collect::<Vec<_>>();
    if chars.len() <= EXCERPT_CHARS {
        return line.to_string();
    }
    // Reserve both elision markers, so the cap holds whichever edges get cut.
    let budget = EXCERPT_CHARS - 2;
    let located_match = first_match(&chars, lowered_line, terms);
    let (match_start, match_end) = located_match.unwrap_or((0, 0));
    // What the window is obliged to hold. A term too long to fit cannot survive
    // whole, so only its first scalar is required and it opens there rather than
    // trailing off its end. With no locatable term, the first scalar keeps the
    // line-start fallback from collapsing to an empty interval.
    let overlong = match_end - match_start > budget;
    let required_end = if located_match.is_none() {
        1
    } else if overlong {
        match_start + 1
    } else {
        match_end
    };
    // Where the window would rather open, before feasibility is considered.
    let preferred = if overlong {
        match_start
    } else {
        back_to_word_boundary(&chars, match_start.saturating_sub(EXCERPT_LEAD))
    };

    // Whole clusters the window is obliged to hold: a partial cluster renders as
    // an orphaned mark or half an emoji. Both edges are chosen together against
    // that span, because picking them independently satisfies each rule on its
    // own and still lands between them.
    let clusters = cluster_starts(line, chars.len());
    let span_start = previous_cluster_start(&clusters, match_start);
    let span_end = next_cluster_start(&clusters, required_end);
    let (start, end) = if span_end - span_start <= budget {
        // A cluster-aligned window fits. `earliest` is the leftmost boundary
        // that still closes past the span, so the range it forms with
        // `span_start` is nonempty and every choice inside it is feasible.
        let earliest = next_cluster_start(&clusters, span_end.saturating_sub(budget));
        let start = previous_cluster_start(&clusters, preferred)
            .max(earliest)
            .min(span_start);
        (
            start,
            previous_cluster_start(&clusters, (start + budget).min(chars.len())),
        )
    } else {
        // One run of clusters is wider than the whole budget, so no aligned
        // window can hold it. The cap and the match win; the edges stay on
        // scalars and the excerpt may show a partial cluster.
        let start = preferred.max(required_end.saturating_sub(budget));
        (start, (start + budget).min(chars.len()))
    };

    let mut text = String::new();
    if start > 0 {
        text.push('…');
    }
    text.extend(&chars[start..end]);
    if end < chars.len() {
        text.push('…');
    }
    text
}

/// Original-scalar range of the earliest term on the line, when the line
/// carries one.
///
/// Terms are located in the same whole-string-lowercased line the caller used
/// to admit the line, because folding differently here loses matches the caller
/// found: `İ` lowercases to two scalars, and a word-final `Σ` lowercases to `ς`
/// rather than `σ`. Offsets come back through a map from each lowered scalar to
/// the original that produced it, so the window lands on original text.
///
/// The map lines up because the only context rule `str::to_lowercase` applies —
/// word-final sigma — substitutes one scalar for one. If that ever stops
/// holding, report no match and window from the line start rather than drift.
fn first_match(chars: &[char], lowered_line: &str, terms: &[String]) -> Option<(usize, usize)> {
    let mut origin = Vec::new();
    for (index, scalar) in chars.iter().enumerate() {
        origin.extend(scalar.to_lowercase().map(|_| index));
    }
    let lowered = lowered_line.chars().collect::<Vec<_>>();
    if lowered.len() != origin.len() {
        return None;
    }
    terms
        .iter()
        .filter_map(|term| {
            let needle = term.chars().collect::<Vec<_>>();
            if needle.is_empty() {
                return None;
            }
            let at = lowered
                .windows(needle.len())
                .position(|window| window == needle.as_slice())?;
            Some((origin[at], origin[at + needle.len() - 1] + 1))
        })
        // Earliest match, and the longest one when several start together.
        .min_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)))
}

/// Which scalar offsets begin a grapheme cluster — what a reader thinks of as a
/// character, and what an editor moves the caret across. A `char` is one scalar
/// below that: `e` plus a combining acute is two scalars but one cluster.
fn cluster_starts(line: &str, scalars: usize) -> Vec<bool> {
    let mut starts = vec![false; scalars];
    let mut offset = 0;
    for cluster in line.graphemes(true) {
        starts[offset] = true;
        offset += cluster.chars().count();
    }
    starts
}

/// First cluster boundary at or after `offset`, or the line end.
fn next_cluster_start(clusters: &[bool], offset: usize) -> usize {
    (offset..clusters.len())
        .find(|index| clusters[*index])
        .unwrap_or(clusters.len())
}

/// Last cluster boundary at or before `offset`. The line end counts as one.
fn previous_cluster_start(clusters: &[bool], offset: usize) -> usize {
    if offset >= clusters.len() {
        return offset;
    }
    (0..=offset)
        .rev()
        .find(|index| clusters[*index])
        .unwrap_or(0)
}

/// Nudge a window start back to just past the nearest whitespace, so the window
/// does not open mid-word. Gives up rather than scanning far, since a long
/// unbroken run holds no boundary worth finding.
fn back_to_word_boundary(chars: &[char], start: usize) -> usize {
    let limit = start.saturating_sub(EXCERPT_WORD_SCAN);
    (limit..start)
        .rev()
        .find(|index| chars[*index].is_whitespace())
        .map_or(start, |index| index + 1)
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

#[cfg(test)]
mod tests {
    use super::{EXCERPT_CHARS, window_excerpt};

    /// Terms reach `window_excerpt` already whole-string lowercased by
    /// `cmd_search`; lowercase here too so tests exercise the real pairing.
    fn terms(list: &[&str]) -> Vec<String> {
        list.iter().map(|term| term.to_lowercase()).collect()
    }

    /// `add_excerpts` hands over the lowercased line it already computed.
    fn window(line: &str, terms: &[String]) -> String {
        window_excerpt(line, &line.to_lowercase(), terms)
    }

    fn scalars(text: &str) -> usize {
        text.chars().count()
    }

    #[test]
    fn line_within_the_cap_is_returned_whole() {
        let line = "  - deployment notes stay indented  ";
        assert_eq!(window(line, &terms(&["deployment"])), line);
    }

    #[test]
    fn line_exactly_at_the_cap_is_not_windowed() {
        let line = format!("{}needle", "f".repeat(EXCERPT_CHARS - 6));
        assert_eq!(scalars(&line), EXCERPT_CHARS);
        assert_eq!(window(&line, &terms(&["needle"])), line);
    }

    #[test]
    fn line_one_past_the_cap_is_windowed_onto_the_match() {
        let line = format!("{}needle", "f".repeat(EXCERPT_CHARS - 5));
        assert_eq!(scalars(&line), EXCERPT_CHARS + 1);
        let text = window(&line, &terms(&["needle"]));
        assert!(text.contains("needle"), "match must survive: {text}");
        assert!(text.starts_with('…'), "left edge is elided: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn long_line_keeps_the_match_visible() {
        // Prefix truncation drops a match this far along the line.
        let line = format!("{}NEEDLE {}", "filler ".repeat(120), "tail ".repeat(60));
        let text = window(&line, &terms(&["needle"]));
        assert!(
            text.to_lowercase().contains("needle"),
            "match must survive: {text}"
        );
        assert!(
            text.starts_with('…') && text.ends_with('…'),
            "both edges are elided: {text}"
        );
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn window_opens_on_a_word_boundary() {
        let line = format!("{}NEEDLE tail", "filler ".repeat(120));
        let text = window(&line, &terms(&["needle"]));
        assert!(text.starts_with("…filler"), "no half word: {text}");
    }

    #[test]
    fn match_near_the_line_start_leaves_the_left_edge_alone() {
        let line = format!("NEEDLE {}", "filler ".repeat(120));
        let text = window(&line, &terms(&["needle"]));
        assert!(text.starts_with("NEEDLE"), "nothing to elide: {text}");
        assert!(text.ends_with('…'), "right edge is elided: {text}");
    }

    #[test]
    fn earliest_of_several_terms_wins_the_window() {
        let line = format!(
            "{}SECOND {}FIRST {}",
            "filler ".repeat(60),
            "filler ".repeat(60),
            "tail ".repeat(60)
        );
        let text = window(&line, &terms(&["first", "second"]));
        assert!(text.contains("SECOND"), "earliest match centers: {text}");
        assert!(
            !text.contains("FIRST"),
            "later match is out of reach: {text}"
        );
    }

    #[test]
    fn line_without_a_locatable_term_falls_back_to_the_line_start() {
        let line = "filler ".repeat(120);
        let text = window(&line, &terms(&["absent"]));
        assert!(text.starts_with("filler"), "show the start: {text}");
        assert!(text.ends_with('…'), "right edge is elided: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn no_locatable_term_with_initial_cluster_at_budget_stays_aligned() {
        let cluster = format!("e{}", "\u{301}".repeat(EXCERPT_CHARS - 3));
        assert_eq!(scalars(&cluster), EXCERPT_CHARS - 2, "exactly the budget");
        let line = format!("{cluster} tail");
        assert!(scalars(&line) > EXCERPT_CHARS, "fixture must be windowed");
        let text = window(&line, &terms(&["absent"]));
        assert!(
            text.starts_with(cluster.as_str()),
            "whole first cluster survives: {text:?}"
        );
        assert!(text.ends_with('…'), "right edge is elided: {text:?}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text:?}");
    }

    #[test]
    fn no_locatable_term_with_initial_cluster_one_past_budget_shows_line_start() {
        let cluster = format!("e{}", "\u{301}".repeat(EXCERPT_CHARS - 2));
        assert_eq!(scalars(&cluster), EXCERPT_CHARS - 1, "one past the budget");
        let line = format!("{cluster} tail");
        assert!(scalars(&line) > EXCERPT_CHARS, "fixture must be windowed");
        let text = window(&line, &terms(&["absent"]));
        assert!(
            text.starts_with("e\u{301}"),
            "show the line-start prefix: {text:?}"
        );
        assert!(text.ends_with('…'), "right edge is elided: {text:?}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text:?}");
    }

    #[test]
    fn multibyte_line_is_cut_between_scalars() {
        // A byte-indexed slice would panic inside these characters.
        let line = format!(
            "{}NEEDLE{}",
            "é".repeat(EXCERPT_CHARS),
            "é".repeat(EXCERPT_CHARS)
        );
        let text = window(&line, &terms(&["needle"]));
        assert!(text.contains("NEEDLE"), "match must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn term_that_fits_the_budget_survives_whole() {
        // Bounding only the distance to the match start clipped a term this
        // long, even though it fits inside the content budget.
        let term = format!("{}abcdefghij", "abcdefghijklmnopqrst".repeat(8));
        assert_eq!(scalars(&term), 170);
        let line = format!("{}{term} tail words", "F".repeat(300));
        let text = window(&line, &terms(&[&term]));
        assert!(text.contains(&term), "whole term must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn term_too_long_for_the_budget_is_shown_from_its_start() {
        // The prefix is unique, so this fails if the window opens even one
        // scalar late — a uniform term would not notice.
        let term = format!("QQOPENSHERE{}", "z".repeat(EXCERPT_CHARS));
        let line = format!("{}{term} tail", "F".repeat(300));
        let text = window(&line, &terms(&[&term]));
        assert!(
            text.starts_with("…QQOPENSHERE"),
            "opens on the term's first scalar: {text}"
        );
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn term_of_exactly_the_budget_survives_whole() {
        let term = format!("QQFITS{}", "y".repeat(EXCERPT_CHARS - 2 - 6));
        assert_eq!(scalars(&term), EXCERPT_CHARS - 2, "exactly the budget");
        let line = format!("{}{term}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&[&term]));
        assert!(text.contains(&term), "whole term must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn term_one_past_the_budget_opens_on_its_first_scalar() {
        let term = format!("QQOVER{}", "y".repeat(EXCERPT_CHARS - 1 - 6));
        assert_eq!(scalars(&term), EXCERPT_CHARS - 1, "one past the budget");
        let line = format!("{}{term}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&[&term]));
        assert!(!text.contains(&term), "cannot fit whole: {text}");
        assert!(
            text.starts_with("…QQOVER"),
            "opens on the term's first scalar: {text}"
        );
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn window_opens_on_the_cluster_holding_the_match() {
        // One 101-scalar cluster, with a unique combining grave buried inside
        // it. Snapping each edge on its own opened the window on that grave.
        let cluster = format!("e{}\u{300}{}", "\u{301}".repeat(90), "\u{301}".repeat(9));
        assert_eq!(scalars(&cluster), 101);
        let line = format!("{}{cluster}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&["\u{300}"]));
        assert!(text.starts_with('…'), "expected a windowed excerpt: {text}");
        assert_eq!(
            text.chars().nth(1),
            Some('e'),
            "window opened mid-cluster: {text}"
        );
        assert!(text.contains(&cluster), "whole cluster survives: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn cluster_after_the_match_is_not_cut_when_it_fits() {
        // The match is the cluster's base letter, and the 159 marks hanging off
        // it fit the budget. Snapping the closing edge alone discarded them.
        let cluster = format!("e{}", "\u{301}".repeat(159));
        assert_eq!(scalars(&cluster), 160);
        let line = format!("{}{cluster}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&["e"]));
        assert!(text.contains(&cluster), "whole cluster survives: {text}");
        assert!(text.ends_with('…'), "right edge is elided: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn over_budget_term_closes_on_a_cluster_boundary() {
        // A 239-scalar term ending in a 9-scalar decomposed cluster. The cap
        // lands seven marks into that cluster, so the edge must snap back.
        let term = format!("QQOVER{}e{}", "Q".repeat(224), "\u{301}".repeat(8));
        assert_eq!(scalars(&term), 239);
        let line = format!("{}{term}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&[&term]));
        assert!(text.starts_with("…QQOVER"), "opens on the term: {text}");
        assert!(
            text.ends_with("Q…"),
            "closes before the split cluster: {text}"
        );
        assert!(
            !text.contains('\u{301}'),
            "no orphaned marks from the cut cluster: {text}"
        );
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn mandatory_cluster_span_of_exactly_the_budget_stays_aligned() {
        let cluster = format!("e{}", "\u{301}".repeat(EXCERPT_CHARS - 2 - 1));
        assert_eq!(scalars(&cluster), EXCERPT_CHARS - 2, "exactly the budget");
        let line = format!("{}{cluster}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&["e"]));
        assert!(text.contains(&cluster), "whole cluster survives: {text}");
        assert_eq!(
            text.chars().nth(1),
            Some('e'),
            "opens on the cluster base: {text}"
        );
    }

    #[test]
    fn mandatory_cluster_span_one_past_the_budget_falls_back_to_scalars() {
        // No aligned window can hold this cluster, so the cap and the match win
        // and the excerpt shows a partial cluster — the documented fallback.
        let cluster = format!("e{}", "\u{301}".repeat(EXCERPT_CHARS - 1 - 1));
        assert_eq!(scalars(&cluster), EXCERPT_CHARS - 1, "one past the budget");
        let line = format!("{}{cluster}{}", "F".repeat(300), " tail".repeat(60));
        let text = window(&line, &terms(&["e"]));
        assert!(!text.contains(&cluster), "cannot fit whole: {text}");
        assert!(text.contains('e'), "match must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn term_whose_lowercase_expands_is_still_located() {
        // `İ` lowercases to two scalars, which drifts any per-scalar fold off
        // the match and used to fall back to the line start.
        let line = format!("{}İ tail words", "G".repeat(300));
        let text = window(&line, &terms(&["İ"]));
        assert!(text.contains('İ'), "match must survive: {text}");
        assert!(
            text.starts_with('…'),
            "windowed, not from the start: {text}"
        );
    }

    #[test]
    fn term_with_a_word_final_sigma_is_still_located() {
        // Whole-string lowercasing maps a word-final `Σ` to `ς`; a per-scalar
        // fold yields `σ`, and the mismatch used to lose the match.
        let line = format!("{}ΟΣ tail words", "H".repeat(300));
        let text = window(&line, &terms(&["ΟΣ"]));
        assert!(text.contains("ΟΣ"), "match must survive: {text}");
        assert!(
            text.starts_with('…'),
            "windowed, not from the start: {text}"
        );
    }

    #[test]
    fn window_does_not_open_on_a_combining_mark() {
        // Decomposed e-acute. A scalar cut lands between the base letter and
        // its accent, and the orphaned accent then renders on the `…` itself.
        // Trailing filler forces a real closing cut as well as an opening one.
        let line = format!("{} NEEDLE{}", "e\u{301}".repeat(200), " tail".repeat(60));
        let text = window(&line, &terms(&["needle"]));
        assert!(
            text.starts_with("…e\u{301}"),
            "opens on a whole cluster: {text}"
        );
        assert!(text.ends_with('…'), "right edge is elided: {text}");
        assert!(text.contains("NEEDLE"), "match must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn window_does_not_split_a_zwj_sequence() {
        // Each family is woman + ZWJ + woman + ZWJ + girl: one cluster, five
        // scalars. A scalar cut leaves a dangling joiner and half a family.
        // Asserting the whole sequence matters — `woman` also appears mid
        // cluster, so a first-scalar check would pass on a split.
        let family = "\u{1F469}\u{200D}\u{1F469}\u{200D}\u{1F467}";
        let line = format!("{} NEEDLE{}", family.repeat(70), " tail".repeat(60));
        let text = window(&line, &terms(&["needle"]));
        assert!(
            text.starts_with(&format!("…{family}")),
            "opens on a whole family sequence: {text}"
        );
        assert!(text.ends_with('…'), "right edge is elided: {text}");
        assert!(
            !text.contains("\u{200D}…"),
            "window closed on a dangling joiner: {text}"
        );
        assert!(text.contains("NEEDLE"), "match must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }

    #[test]
    fn multibyte_term_matches_case_insensitively() {
        // Terms reach here already lowercased, and the line is located with the
        // caller's own whole-string fold, so offsets map back to originals.
        let line = format!("{}ÉCLAIR{}", "x".repeat(400), "y".repeat(400));
        let text = window(&line, &terms(&["éclair"]));
        assert!(text.contains("ÉCLAIR"), "match must survive: {text}");
        assert!(scalars(&text) <= EXCERPT_CHARS, "cap holds: {text}");
    }
}
