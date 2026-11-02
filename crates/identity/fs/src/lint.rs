use slop_id_identity_core::{
    ProjectConfig, SidLintReport,
    lint::{
        Severity, SidIdentityFinding, SidIdentityFindingCode, finish_report, relative_to_base,
        scan_root_entries,
    },
};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
pub fn cmd_lint(config: &ProjectConfig) -> SidLintReport {
    scan_identity(config)
}
fn scan_identity(config: &ProjectConfig) -> SidLintReport {
    let mut findings = Vec::new();
    let mut reservations: BTreeMap<String, BTreeSet<PathBuf>> = BTreeMap::new();

    for root in [&config.root] {
        let relative_root = relative_to_base(&config.base, root);
        let entries = match fs::read_dir(root) {
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
            entries
                .map(|entry| entry.map(|entry| entry.file_name().to_str().map(str::to_owned)))
                .collect::<Vec<_>>()
                .into_iter(),
            &mut reservations,
            &mut findings,
        );
    }

    finish_report(findings, reservations)
}
