use std::path::PathBuf;

#[derive(Debug)]
pub struct PeriodPrefixedChild {
    pub name: String,
    pub period: String,
    pub ref_segment: String,
    pub path: PathBuf,
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
