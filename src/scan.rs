//! Shared direct-child scanning for task roots. Both `sid` and `stm` walk the
//! same tree shape but recognize different ref families, so this module
//! yields raw period-prefixed children and leaves ref recognition to the
//! caller.

use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// A direct child of a task root whose name parses as `{YYYYMM}_{segment}` or
/// `{YYYYMM}_{segment}_{slug}`. The ref segment is not validated here; callers
/// apply their own recognition rules.
#[derive(Debug)]
pub struct PeriodPrefixedChild {
    pub name: String,
    pub period: String,
    pub ref_segment: String,
    pub path: PathBuf,
}

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

/// Split `{YYYYMM}_{segment}[_{slug}]` into period and ref segment, requiring
/// a valid period shape. The ref segment is returned unvalidated.
pub fn split_task_folder_name(name: &str) -> Option<(&str, &str)> {
    let mut parts = name.splitn(3, '_');
    let period = parts.next()?;
    let ref_segment = parts.next()?;

    if !is_valid_period(period) {
        return None;
    }

    Some((period, ref_segment))
}

/// Six ASCII digits. Calendar-valid month enforcement is deferred.
pub fn is_valid_period(period: &str) -> bool {
    period.len() == 6 && period.chars().all(|c| c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_slugged_slugless_and_rejects_invalid_periods() {
        assert_eq!(
            split_task_folder_name("202605_sa2a2_fix-auth"),
            Some(("202605", "sa2a2"))
        );
        assert_eq!(
            split_task_folder_name("202605_sa2a2"),
            Some(("202605", "sa2a2"))
        );
        assert_eq!(split_task_folder_name("202605"), None);
        assert_eq!(split_task_folder_name("2026_05_sa2a2"), None);
        assert_eq!(split_task_folder_name("notes_202605_sa2a2"), None);
        // Both sides of the six-digit period boundary.
        assert_eq!(split_task_folder_name("20265_sa2a2_x"), None);
        assert_eq!(split_task_folder_name("2026056_sa2a2_x"), None);
    }
}
