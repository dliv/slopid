use anyhow::Result;
use chrono::{DateTime, Local, SecondsFormat};
use clap::ValueEnum;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::commands::ProjectConfig;
use crate::commands::folders::{FolderIdentity, folder_snapshot};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListSort {
    /// Most recently touched first (folder and direct-child mtimes)
    Recent,
    /// Ascending by complete folder name
    Id,
}

#[derive(Debug, Serialize)]
pub struct ListedFolder {
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
    pub folders: Vec<ListedFolder>,
    /// Project base for human rendering; not part of the JSON contract.
    #[serde(skip)]
    pub base: PathBuf,
}

pub fn render_human(result: &ListResult) -> String {
    let id_width = result
        .folders
        .iter()
        .map(|folder| folder.id.len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for folder in &result.folders {
        let modified = folder
            .modified
            .get(..16)
            .unwrap_or(&folder.modified)
            .replace('T', " ");
        let root = folder
            .root
            .strip_prefix(&result.base)
            .unwrap_or(&folder.root)
            .display();
        out.push_str(&format!("{modified}  {:<id_width$}  {root}\n", folder.id));
    }
    out
}

pub fn cmd_list(terms: Vec<String>, sort: ListSort, config: &ProjectConfig) -> Result<ListResult> {
    let terms = terms
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    let folders = folder_snapshot(&config.root)?;
    let mut entries = folders
        .into_iter()
        .filter(|folder| {
            let slug = folder.slug.to_lowercase();
            terms
                .iter()
                .all(|term| folder.sid_ref.starts_with(term) || slug.contains(term))
        })
        .map(|folder| {
            let touched = touch_time(&folder.path);
            let listed = listed_folder(folder, touched);
            (touched, listed)
        })
        .collect::<Vec<_>>();

    match sort {
        ListSort::Recent => entries.sort_by(|(left_time, left), (right_time, right)| {
            right_time
                .cmp(left_time)
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| left.path.cmp(&right.path))
        }),
        ListSort::Id => entries.sort_by(|(_, left), (_, right)| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.path.cmp(&right.path))
        }),
    }

    Ok(ListResult {
        folders: entries.into_iter().map(|(_, folder)| folder).collect(),
        base: config.base.clone(),
    })
}

fn listed_folder(folder: FolderIdentity, touched: SystemTime) -> ListedFolder {
    ListedFolder {
        id: folder.id,
        period: folder.period,
        sid_ref: folder.sid_ref,
        slug: folder.slug,
        modified: DateTime::<Local>::from(touched).to_rfc3339_opts(SecondsFormat::Secs, false),
        root: folder.root,
        path: folder.path,
    }
}

/// Directory and direct-child mtimes form a best-effort recency proxy.
/// Child symlinks are inspected with symlink_metadata so target writes do not
/// affect the parent folder's order; failures degrade to the epoch.
fn touch_time(path: &Path) -> SystemTime {
    let own = fs::symlink_metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(UNIX_EPOCH);
    let Ok(children) = fs::read_dir(path) else {
        return own;
    };
    children.flatten().fold(own, |latest, child| {
        fs::symlink_metadata(child.path())
            .and_then(|metadata| metadata.modified())
            .map_or(latest, |modified| latest.max(modified))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn human_rendering_uses_base_relative_root_and_empty_results() {
        let result = ListResult {
            folders: Vec::new(),
            base: PathBuf::from("/tmp/project"),
        };
        assert_eq!(render_human(&result), "");
    }

    #[test]
    fn missing_root_snapshot_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(
            folder_snapshot(&tmp.path().join("missing"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn recency_io_failure_degrades_to_epoch() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("missing");

        assert_eq!(touch_time(&missing), UNIX_EPOCH);
    }

    #[test]
    fn real_directory_identity_name_parser_keeps_slugless_names() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("202610_sa2a7");
        fs::create_dir(&path).unwrap();
        let parsed = super::super::folders::parse_identity_name("202610_sa2a7").unwrap();
        assert_eq!(
            parsed,
            ("202610".to_string(), "sa2a7".to_string(), String::new())
        );
    }
}
