use crate::scan;
use anyhow::{Context, Result, bail};
use slop_id_identity_core::{
    NewPlan, NewResult, ProjectConfig, ScanSnapshot,
    allocation::{candidate_to_result, plan_new, plan_to_result},
    refcode::RefTail,
};
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

pub fn cmd_new(
    title: String,
    dry_run: bool,
    period: String,
    config: &ProjectConfig,
    tails: impl FnOnce() -> Vec<RefTail>,
) -> Result<NewResult> {
    let snapshot = scan_root(&config.root)?;
    let plan = plan_new(
        title,
        config.root.clone(),
        period,
        &snapshot,
        &config.ref_prefix_policy,
        tails(),
    )?;
    if dry_run {
        return Ok(plan_to_result(&plan, true));
    }
    execute_new(&plan)
}
pub fn scan_root(root: &Path) -> Result<ScanSnapshot> {
    let mut snapshot = ScanSnapshot::empty();
    scan_root_into(&mut snapshot, root)?;
    Ok(snapshot)
}

fn scan_root_into(snapshot: &mut ScanSnapshot, root: &Path) -> Result<()> {
    for child in scan::scan_period_prefixed_children(root)? {
        // Files, symlinks, and other non-directories reserve best-effort;
        // unexpected entry shapes never fail allocation.
        snapshot.observe(child.period, child.ref_segment);
    }

    Ok(())
}

pub fn execute_new(plan: &NewPlan) -> Result<NewResult> {
    fs::create_dir_all(plan.root())
        .with_context(|| format!("create task root {}", plan.root().display()))?;

    // v1 accepts a same-ref/different-slug race; exact path collisions retry.
    for candidate in plan.candidates() {
        match fs::create_dir(&candidate.path) {
            Ok(()) => return Ok(candidate_to_result(plan, candidate, false)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create task folder {}", candidate.path.display()));
            }
        }
    }

    bail!(
        "could not create a task folder for {} after {} attempts",
        plan.period(),
        plan.candidates().len()
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use slop_id_identity_core::refcode;

    fn tail(hi: char, lo: char) -> RefTail {
        RefTail { hi, lo }
    }

    fn prude_policy() -> refcode::RefPrefixPolicy {
        refcode::RefPrefixPolicy::prude()
    }

    #[test]
    fn scan_root_records_slugged_and_slugless_direct_child_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202605_sa2a2_fix-auth")).unwrap();
        // Slugless task-shaped directories are accepted as conservative
        // namespace reservations, even though generated ids include a slug.
        fs::create_dir(root.join("202605_sa3a2")).unwrap();
        fs::write(root.join("notes_202605_sa4a2"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert!(snapshot.is_occupied("202605", "sa3a2"));
        assert!(!snapshot.is_occupied("202605", "sa4a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 2);
    }

    #[test]
    fn scan_root_treats_missing_root_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");

        let snapshot = scan_root(&root).unwrap();

        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_ignores_non_task_shaped_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("README.md"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(!snapshot.is_occupied("202605", "sa2a2"));
        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_ignores_period_prefixed_non_recognized_ref_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("202605_notaref_x"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(!snapshot.is_occupied("202605", "notaref"));
        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_reserves_namespace_for_task_shaped_files() {
        // Zipped task-folder exports legitimately sit next to their source
        // folders; unexpected entry shapes reserve best-effort instead of
        // failing allocation.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("202605_sa2a2_fix-auth.zip"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn scan_root_reserves_namespace_for_task_shaped_symlinks() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        symlink(root.join("target"), root.join("202605_sa2a2_link")).unwrap();
        symlink(root.join("missing"), root.join("202605_sa3a2_dangling")).unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert!(snapshot.is_occupied("202605", "sa3a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 2);
    }

    #[test]
    fn scan_root_records_recognized_refs_even_when_not_generated() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202605_sa222_legacy")).unwrap();

        assert!(refcode::is_recognized_ref("sa222"));
        assert!(
            refcode::encode_ref(refcode::decode_seq("sa222").unwrap(), tail('2', '2')).is_none()
        );

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa222"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 1);
    }

    #[test]
    fn scan_root_partitions_refs_and_max_sequence_by_period() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202604_szza2_exhausted-other-month")).unwrap();
        fs::create_dir(root.join("202604_seaa2_same-ref-other-month")).unwrap();

        let snapshot = scan_root(&root).unwrap();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert_eq!(
            refcode::decode_seq(&plan.candidates()[0].sid_ref).unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
        assert_eq!(plan.candidates()[0].sid_ref, "seaa2");
    }

    #[test]
    fn execute_new_retries_next_candidate_when_a_file_occupies_the_path() {
        // A racing zip export (or any file) at the exact candidate path must
        // hit the AlreadyExists retry, not a hard error.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        fs::create_dir_all(plan.root()).unwrap();
        fs::write(&plan.candidates()[0].path, "zip bytes").unwrap();

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, plan.candidates()[1].id);
        assert!(plan.candidates()[1].path.is_dir());
    }

    #[test]
    fn execute_new_retries_next_candidate_on_exact_path_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        let second_id = plan.candidates()[1].id.clone();
        let second_path = plan.candidates()[1].path.clone();
        fs::create_dir_all(&plan.candidates()[0].path).unwrap();

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, second_id);
        assert!(second_path.is_dir());
    }

    #[test]
    fn execute_new_creates_missing_task_root_and_chosen_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root.clone(),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert!(!root.exists());

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, plan.candidates()[0].id);
        assert!(root.is_dir());
        assert!(result.path.is_dir());
    }

    #[test]
    fn execute_new_errors_when_all_candidate_paths_already_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        for candidate in plan.candidates() {
            fs::create_dir_all(&candidate.path).unwrap();
        }

        let err = execute_new(&plan).unwrap_err();

        assert!(err.to_string().contains("could not create a task folder"));
    }
}
