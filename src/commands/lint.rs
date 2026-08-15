//! Folder-identity lint over the configured allocation namespace.
//!
//! Slopid owns immutable folder identity; task-memory meaning above those
//! folders belongs to Slopdeck. This command therefore inspects direct-child
//! *names* only. It never opens `CURRENT_STATE.md`, an inbox message, a
//! capture note, a topic document, or any other authored byte, and it never
//! follows or stats an entry — a matching file or symlink is a valid ref
//! reservation because allocation must not reuse its ref.

use crate::commands::config::ProjectConfig;
use crate::documents::Severity;
use crate::refcode::is_recognized_ref;
use crate::scan::split_task_folder_name;
use anyhow::Result;
use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// The complete version-1 identity finding vocabulary. Slopdeck decodes this
/// through a closed enum, so adding a spelling is a coordinated change.
#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub enum SidIdentityFindingCode {
    #[serde(rename = "identity-root-unreadable")]
    RootUnreadable,
    #[serde(rename = "identity-entry-unreadable")]
    EntryUnreadable,
    #[serde(rename = "identity-folder-ref-invalid")]
    FolderRefInvalid,
    #[serde(rename = "identity-ref-duplicate")]
    RefDuplicate,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SidIdentityFinding {
    pub code: SidIdentityFindingCode,
    pub severity: Severity,
    pub message: String,
    pub ref_id: Option<String>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SidLintReport {
    /// Coverage: every configured allocation root was fully enumerated.
    pub complete: bool,
    /// Validity: no error finding was observed.
    pub healthy: bool,
    pub findings: Vec<SidIdentityFinding>,
}

pub struct LintExecution {
    pub report: SidLintReport,
    pub exit_code: i32,
}

impl SidIdentityFindingCode {
    fn as_str(self) -> &'static str {
        match self {
            Self::RootUnreadable => "identity-root-unreadable",
            Self::EntryUnreadable => "identity-entry-unreadable",
            Self::FolderRefInvalid => "identity-folder-ref-invalid",
            Self::RefDuplicate => "identity-ref-duplicate",
        }
    }

    /// Operational codes mean the scan could not observe something it needed,
    /// so coverage is incomplete. Data codes mean a defect was fully observed.
    fn is_operational(self) -> bool {
        match self {
            Self::RootUnreadable | Self::EntryUnreadable => true,
            Self::FolderRefInvalid | Self::RefDuplicate => false,
        }
    }
}

pub fn cmd_lint(cwd: &Path) -> Result<LintExecution> {
    let config = super::load_project_config(cwd)?;
    let report = scan_identity(&config);
    let exit_code = exit_code(&report);
    Ok(LintExecution { report, exit_code })
}

/// Incomplete coverage outranks an unhealthy verdict: an agent that cannot see
/// the whole namespace must not act on a partial "unhealthy" answer as if the
/// scan were authoritative. Warnings alone stay successful.
fn exit_code(report: &SidLintReport) -> i32 {
    if !report.complete {
        2
    } else if !report.healthy {
        1
    } else {
        0
    }
}

fn scan_identity(config: &ProjectConfig) -> SidLintReport {
    let mut findings = Vec::new();
    let mut reservations: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();

    for root in config.allocation_roots() {
        let relative_root = relative_to_base(&config.base, &root);
        let entries = match fs::read_dir(&root) {
            Ok(entries) => entries,
            // A configured root that does not exist yet reserves nothing.
            // Allocation creates it on demand, so absence is an empty
            // namespace rather than a defect.
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(error) => {
                findings.push(SidIdentityFinding {
                    code: SidIdentityFindingCode::RootUnreadable,
                    severity: Severity::Error,
                    message: format!("cannot read configured allocation root: {error}"),
                    ref_id: None,
                    path: Some(relative_root),
                });
                continue;
            }
        };
        scan_root_entries(
            &relative_root,
            entries.map(|entry| entry.map(|entry| entry.file_name().to_str().map(str::to_owned))),
            &mut reservations,
            &mut findings,
        );
    }

    for (sid_ref, locators) in reservations {
        if locators.len() < 2 {
            continue;
        }
        let locators = locators
            .iter()
            .map(|locator| locator.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        findings.push(SidIdentityFinding {
            code: SidIdentityFindingCode::RefDuplicate,
            severity: Severity::Error,
            message: format!("ref is reserved by more than one direct child: {locators}"),
            ref_id: Some(sid_ref),
            path: None,
        });
    }

    findings.sort_by(compare_identity_findings);
    SidLintReport {
        complete: !findings.iter().any(|finding| finding.code.is_operational()),
        healthy: !findings
            .iter()
            .any(|finding| finding.severity == Severity::Error),
        findings,
    }
}

/// Classify one root's direct children. Entry names arrive through an iterator
/// rather than `fs::ReadDir` so a mid-iteration failure is a deterministic test
/// seam instead of a racy filesystem trick.
fn scan_root_entries<I>(
    relative_root: &Path,
    entries: I,
    reservations: &mut BTreeMap<String, BTreeSet<PathBuf>>,
    findings: &mut Vec<SidIdentityFinding>,
) where
    I: Iterator<Item = std::io::Result<Option<String>>>,
{
    for entry in entries {
        let name = match entry {
            Ok(Some(name)) => name,
            // A name the platform will not hand back as UTF-8 cannot spell a
            // recognized ref, so it is ignored like any other nonmatching
            // entry rather than reported.
            Ok(None) => continue,
            Err(error) => {
                findings.push(SidIdentityFinding {
                    code: SidIdentityFindingCode::EntryUnreadable,
                    severity: Severity::Error,
                    message: format!(
                        "cannot read a directory entry under the configured allocation root: {error}"
                    ),
                    ref_id: None,
                    path: Some(relative_root.to_path_buf()),
                });
                continue;
            }
        };
        let Some((_, ref_segment)) = split_task_folder_name(&name) else {
            continue;
        };
        let locator = relative_root.join(&name);
        if !is_recognized_ref(ref_segment) {
            findings.push(SidIdentityFinding {
                code: SidIdentityFindingCode::FolderRefInvalid,
                severity: Severity::Error,
                message: "direct child has a valid six-digit period prefix but an unrecognized ref"
                    .to_string(),
                ref_id: Some(ref_segment.to_string()),
                path: Some(locator),
            });
            continue;
        }
        reservations
            .entry(ref_segment.to_string())
            .or_default()
            .insert(locator);
    }
}

/// Locators are reported relative to the discovered project base so no report
/// carries an expanded private path. Configured roots are always
/// `base.join(<validated relative path>)`, so the strip cannot fail; the
/// fallback keeps an unexpected path out of the report instead of leaking it.
fn relative_to_base(base: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(base)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(path.file_name().unwrap_or_default()))
}

fn compare_identity_findings(left: &SidIdentityFinding, right: &SidIdentityFinding) -> Ordering {
    severity_rank(left.severity)
        .cmp(&severity_rank(right.severity))
        .then_with(|| left.code.as_str().cmp(right.code.as_str()))
        .then_with(|| left.ref_id.cmp(&right.ref_id))
        .then_with(|| left.path.cmp(&right.path))
        .then_with(|| left.message.cmp(&right.message))
}

fn severity_rank(severity: Severity) -> u8 {
    match severity {
        Severity::Error => 0,
        Severity::Warning => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    fn finding(code: SidIdentityFindingCode, severity: Severity) -> SidIdentityFinding {
        SidIdentityFinding {
            code,
            severity,
            message: "sample".into(),
            ref_id: None,
            path: None,
        }
    }

    fn report(findings: Vec<SidIdentityFinding>) -> SidLintReport {
        SidLintReport {
            complete: !findings.iter().any(|finding| finding.code.is_operational()),
            healthy: !findings
                .iter()
                .any(|finding| finding.severity == Severity::Error),
            findings,
        }
    }

    #[test]
    fn exit_code_covers_both_sides_of_every_status_boundary() {
        assert_eq!(exit_code(&report(vec![])), 0);
        // Warning-only stays successful: no version-1 identity code is a
        // warning yet, so this boundary is proven against the contract itself.
        assert_eq!(
            exit_code(&report(vec![finding(
                SidIdentityFindingCode::RefDuplicate,
                Severity::Warning
            )])),
            0
        );
        assert_eq!(
            exit_code(&report(vec![finding(
                SidIdentityFindingCode::RefDuplicate,
                Severity::Error
            )])),
            1
        );
        assert_eq!(
            exit_code(&report(vec![finding(
                SidIdentityFindingCode::RootUnreadable,
                Severity::Error
            )])),
            2
        );
        // Incomplete coverage outranks an unhealthy complete verdict.
        assert_eq!(
            exit_code(&report(vec![
                finding(SidIdentityFindingCode::FolderRefInvalid, Severity::Error),
                finding(SidIdentityFindingCode::EntryUnreadable, Severity::Error),
            ])),
            2
        );
    }

    #[test]
    fn a_mid_iteration_entry_failure_is_reported_without_losing_earlier_entries() {
        let mut reservations = BTreeMap::new();
        let mut findings = Vec::new();
        scan_root_entries(
            Path::new("stm"),
            [
                Ok(Some("202401_sa2a7_first".to_string())),
                Err(io::Error::from(io::ErrorKind::PermissionDenied)),
                Ok(Some("202402_sb3b8_second".to_string())),
            ]
            .into_iter(),
            &mut reservations,
            &mut findings,
        );

        assert_eq!(
            reservations.keys().collect::<Vec<_>>(),
            ["sa2a7", "sb3b8"],
            "entries before and after the failure must still be classified"
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].code, SidIdentityFindingCode::EntryUnreadable);
        assert_eq!(findings[0].path.as_deref(), Some(Path::new("stm")));
        assert!(findings[0].code.is_operational());
    }

    #[test]
    fn non_utf8_and_nonmatching_entry_names_are_ignored_rather_than_reported() {
        let mut reservations = BTreeMap::new();
        let mut findings = Vec::new();
        scan_root_entries(
            Path::new("stm"),
            [
                Ok(None),
                Ok(Some(".seeds".to_string())),
                Ok(Some("README.md".to_string())),
                Ok(Some("2026_sa2a7_short-period".to_string())),
                Ok(Some("2026056_sa2a7_long-period".to_string())),
            ]
            .into_iter(),
            &mut reservations,
            &mut findings,
        );
        assert!(reservations.is_empty());
        assert!(findings.is_empty(), "{findings:?}");
    }

    #[test]
    fn locators_stay_relative_to_the_project_base() {
        assert_eq!(
            relative_to_base(Path::new("/base"), Path::new("/base/stm/.archive")),
            PathBuf::from("stm/.archive")
        );
        // Unreachable through validated config, but never leak an absolute path.
        assert_eq!(
            relative_to_base(Path::new("/base"), Path::new("/elsewhere/stm")),
            PathBuf::from("stm")
        );
    }
}
