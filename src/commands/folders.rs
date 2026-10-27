use anyhow::{Context, Result};
use serde::Serialize;
use std::fs;
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

use crate::refcode::is_recognized_ref;
use crate::scan::split_task_folder_name;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FolderIdentity {
    pub id: String,
    pub period: String,
    pub sid_ref: String,
    pub slug: String,
    pub root: PathBuf,
    pub path: PathBuf,
}

/// Take a complete, direct-child snapshot of real identity directories.
/// Name-based reservation scans remain separate in allocation and lint.
pub fn folder_snapshot(root: &Path) -> Result<Vec<FolderIdentity>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(error).with_context(|| format!("scan identity root {}", root.display()));
        }
    };
    let paths = entries.map(|entry| entry.map(|entry| entry.path()));
    snapshot_paths(root, paths, |path| {
        fs::symlink_metadata(path).map(|metadata| metadata.file_type().is_dir())
    })
}

fn snapshot_paths<I, F>(root: &Path, entries: I, mut is_directory: F) -> Result<Vec<FolderIdentity>>
where
    I: IntoIterator<Item = io::Result<PathBuf>>,
    F: FnMut(&Path) -> io::Result<bool>,
{
    let mut folders = Vec::new();
    for entry in entries {
        let path =
            entry.with_context(|| format!("read identity root entry under {}", root.display()))?;
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some((period, sid_ref, slug)) = parse_identity_name(name) else {
            continue;
        };
        let is_directory = is_directory(&path)
            .with_context(|| format!("inspect identity entry {}", path.display()))?;
        if !is_directory {
            continue;
        }
        folders.push(FolderIdentity {
            id: name.to_string(),
            period,
            sid_ref,
            slug,
            root: root.to_path_buf(),
            path,
        });
    }
    folders.sort_by(|left, right| {
        left.id
            .cmp(&right.id)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(folders)
}

/// Parse the complete folder spelling. Slugs are opaque, nonempty suffixes;
/// legacy slugless identities remain valid.
pub(crate) fn parse_identity_name(name: &str) -> Option<(String, String, String)> {
    let (period, sid_ref) = split_task_folder_name(name)?;
    if !is_recognized_ref(sid_ref) {
        return None;
    }
    let slug = match name.splitn(3, '_').nth(2) {
        None => String::new(),
        Some("") => return None,
        Some(slug) => slug.to_string(),
    };
    Some((period.to_string(), sid_ref.to_string(), slug))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterator_failure_after_a_match_discards_the_partial_snapshot() {
        let root = PathBuf::from("/tmp/stm");
        let entries = vec![
            Ok(root.join("202610_sa2a7_first")),
            Err(io::Error::other("injected iterator failure")),
        ];
        let result = snapshot_paths(&root, entries, |_| Ok(true));
        assert!(result.is_err());
        assert!(format!("{:#}", result.unwrap_err()).contains("injected iterator failure"));
    }

    #[test]
    fn matching_entry_metadata_failure_discards_the_partial_snapshot() {
        let root = PathBuf::from("/tmp/stm");
        let first = root.join("202610_sa2a7_first");
        let second = root.join("202611_sb3b8_second");
        let entries = vec![Ok(first.clone()), Ok(second.clone())];
        let result = snapshot_paths(&root, entries, |path| {
            if path == second {
                Err(io::Error::other("injected matching-entry metadata failure"))
            } else {
                Ok(true)
            }
        });
        assert!(result.is_err());
        assert!(
            format!("{:#}", result.unwrap_err())
                .contains("injected matching-entry metadata failure")
        );
    }

    #[test]
    fn nonmatching_names_do_not_trigger_matching_entry_metadata_reads() {
        let root = PathBuf::from("/tmp/stm");
        let entries = vec![
            Ok(root.join("README.md")),
            Ok(root.join("202610_sdmr192_bad")),
        ];
        let folders =
            snapshot_paths(&root, entries, |_| panic!("nonmatching path was inspected")).unwrap();
        assert!(folders.is_empty());
    }
}
