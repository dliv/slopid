use anyhow::{Context, Result};
use slop_id_identity_core::names::{PeriodPrefixedChild, split_task_folder_name};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

/// Scan the direct children of a task root. A missing root is an empty
/// snapshot; an unreadable root (including a mid-iteration read error, per
/// contract 0001's fail-closed race policy) fails closed. Entry shapes never
/// fail the scan: directories, files, and symlinks alike contribute a child
/// when their name parses (zipped task-folder exports legitimately sit next
/// to their source folders), and everything else is skipped. File types are
/// never inspected.
pub fn scan_period_prefixed_children(root: &Path) -> Result<Vec<PeriodPrefixedChild>> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err).with_context(|| format!("scan task root {}", root.display())),
    };

    let mut children = Vec::new();
    for entry in entries {
        let entry = entry?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some((period, ref_segment)) = split_task_folder_name(&name) else {
            continue;
        };
        children.push(PeriodPrefixedChild {
            period: period.to_string(),
            ref_segment: ref_segment.to_string(),
            path: entry.path(),
            name,
        });
    }

    Ok(children)
}
