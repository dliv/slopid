use anyhow::Result;
use chrono::{DateTime, Local, SecondsFormat};
use clap::ValueEnum;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::load_project_config;
use crate::refcode;
use crate::scan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListSort {
    /// Most recently touched first (entry and direct-child mtimes)
    Recent,
    /// Ascending by id (allocation order within a period)
    Id,
}

#[derive(Debug, Serialize)]
pub struct ListedTask {
    pub id: String,
    pub period: String,
    pub sid_ref: String,
    pub slug: String,
    pub modified: String,
    pub root: PathBuf,
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ListResult {
    pub tasks: Vec<ListedTask>,
    /// Project base for human rendering; not part of the JSON contract.
    #[serde(skip)]
    pub base: PathBuf,
}

/// Render for direct human use (ADR 0006). Explicitly not a machine
/// interface: the format may change without a contract update; agents must
/// parse the default JSON instead.
pub fn render_human(result: &ListResult) -> String {
    let id_width = result
        .tasks
        .iter()
        .map(|task| task.id.len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for task in &result.tasks {
        let modified = task
            .modified
            .get(..16)
            .unwrap_or(&task.modified)
            .replace('T', " ");
        let root = task
            .root
            .strip_prefix(&result.base)
            .unwrap_or(&task.root)
            .display();
        out.push_str(&format!("{modified}  {:<id_width$}  {root}\n", task.id));
    }
    out
}

/// List every recognized-ref entry across the active root and configured
/// scan roots: the same namespace view allocation uses. Listing and
/// filtering are name-based — file types are never inspected, so
/// non-directory reservations (zipped exports) appear too. Recency reads
/// entry and direct-child modification times; metadata errors degrade to
/// "old", never to failure.
pub fn cmd_list(terms: Vec<String>, sort: ListSort, cwd: &Path) -> Result<ListResult> {
    let config = load_project_config(cwd)?;
    let terms: Vec<String> = terms.iter().map(|term| term.to_lowercase()).collect();
    let mut entries: Vec<(SystemTime, ListedTask)> = Vec::new();

    for root in &config.scan_roots {
        for child in scan::scan_period_prefixed_children(root)? {
            if !refcode::is_recognized_ref(&child.ref_segment) {
                continue;
            }

            let slug = child.name.splitn(3, '_').nth(2).unwrap_or("").to_string();
            let slug_lower = slug.to_lowercase();
            if !terms
                .iter()
                .all(|term| child.ref_segment.starts_with(term) || slug_lower.contains(term))
            {
                continue;
            }

            let touched = touch_time(&child.path);
            entries.push((
                touched,
                ListedTask {
                    id: child.name,
                    period: child.period,
                    sid_ref: child.ref_segment,
                    slug,
                    modified: DateTime::<Local>::from(touched)
                        .to_rfc3339_opts(SecondsFormat::Secs, false),
                    root: root.clone(),
                    path: child.path,
                },
            ));
        }
    }

    match sort {
        ListSort::Recent => entries.sort_by(|(at, a), (bt, b)| {
            bt.cmp(at)
                .then_with(|| a.id.cmp(&b.id))
                .then_with(|| a.path.cmp(&b.path))
        }),
        ListSort::Id => {
            entries.sort_by(|(_, a), (_, b)| a.id.cmp(&b.id).then_with(|| a.path.cmp(&b.path)))
        }
    }

    Ok(ListResult {
        tasks: entries.into_iter().map(|(_, task)| task).collect(),
        base: config.base,
    })
}

/// "Most recently touched" proxy: the entry's own mtime or its newest direct
/// child's, whichever is later. Direct-child edits count; deeper edits do
/// not (no recursion, by design), and child stats are lstat-like, so edits
/// behind a symlinked child do not surface the folder either. Errors degrade
/// to the epoch so a vanished entry sorts old instead of failing.
fn touch_time(path: &Path) -> SystemTime {
    let own = fs::symlink_metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or(UNIX_EPOCH);

    let Ok(children) = fs::read_dir(path) else {
        return own;
    };

    let mut latest = own;
    for child in children.flatten() {
        if let Ok(modified) = child.metadata().and_then(|meta| meta.modified()) {
            latest = latest.max(modified);
        }
    }
    latest
}
