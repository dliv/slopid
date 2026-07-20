use crate::documents::{self, CanonicalSourceKind, Finding, Severity, compare_findings};
use anyhow::Result;
use chrono::Utc;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Serialize)]
pub struct LintResult {
    pub findings: Vec<Finding>,
}

pub struct LintExecution {
    pub result: Option<LintResult>,
    pub exit_code: i32,
}

pub fn cmd_lint(cwd: &Path) -> Result<LintExecution> {
    let config = super::load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    let today = Utc::now().date_naive();
    let mut findings = index.findings.clone();
    for record in index
        .nodes
        .values()
        .filter(|record| record.source_kind == CanonicalSourceKind::TaskOwner)
    {
        findings
            .extend(super::scan_inbox(&record.owner_path, config.stale_after_days, today).findings);
    }
    let (_, note_findings, _) =
        super::scan_notes(&config.note_root, config.stale_after_days, today);
    findings.extend(note_findings);
    findings.sort_by(compare_findings);
    if !index.operationally_complete() || findings.iter().any(Finding::is_operational) {
        return Ok(LintExecution {
            result: None,
            exit_code: 2,
        });
    }
    let exit_code = lint_status(&findings);
    Ok(LintExecution {
        result: Some(LintResult { findings }),
        exit_code,
    })
}

fn lint_status(findings: &[Finding]) -> i32 {
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        1
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lint_status_keeps_warning_only_results_successful() {
        let warning = Finding {
            code: crate::documents::FindingCode::MissingEdgeReason,
            severity: Severity::Warning,
            message: "advisory only".into(),
            id: None,
            path: None,
            line: None,
        };
        assert_eq!(lint_status(&[warning]), 0);
    }
}
