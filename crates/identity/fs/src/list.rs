use crate::folders::folder_snapshot;
use anyhow::Result;
use slop_id_identity_core::{
    ListResult, ListSort, ProjectConfig,
    list::{filter_folders, finish_list, listed_folder},
};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
pub fn cmd_list(
    terms: Vec<String>,
    sort: ListSort,
    config: &ProjectConfig,
    mut format_time: impl FnMut(SystemTime) -> String,
) -> Result<ListResult> {
    let folders = filter_folders(folder_snapshot(&config.root)?, terms);
    let entries = folders
        .into_iter()
        .map(|folder| {
            let touched = touch_time(&folder.path);
            let modified = format_time(touched);
            (touched, listed_folder(folder, modified))
        })
        .collect();
    Ok(finish_list(entries, sort, config.base.clone()))
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
        let parsed = slop_id_identity_core::folders::parse_identity_name("202610_sa2a7").unwrap();
        assert_eq!(
            parsed,
            ("202610".to_string(), "sa2a7".to_string(), String::new())
        );
    }
}
