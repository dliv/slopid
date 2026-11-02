//! A second composition root: real filesystem workflows without the sid app.
use slop_id_identity_core::{LintVerdict, ListSort, refcode::RefTail};
use slop_id_identity_fs as identity;
use std::cell::Cell;

mod inspect;

#[test]
fn a_second_root_composes_real_identity_operations() {
    let workspace = tempfile::tempdir().unwrap();
    identity::init_config(workspace.path()).unwrap();
    let config = identity::load_project_config(workspace.path()).unwrap();
    let calls = Cell::new(0);
    let tails = || {
        calls.set(calls.get() + 1);
        vec![RefTail { hi: '2', lo: 'a' }]
    };
    let preview =
        identity::cmd_new("second host".into(), true, "202605".into(), &config, tails).unwrap();
    inspect::assert_absent(&config.root);
    let created =
        identity::cmd_new("second host".into(), false, "202605".into(), &config, tails).unwrap();
    assert_eq!(preview.id, created.id);
    assert_eq!(calls.get(), 2);
    inspect::assert_only_empty_folder(&config.root, &created.path);
    let listed = identity::cmd_list(vec![], ListSort::Id, &config, |_| {
        "2000-01-01T00:00:00+00:00".into()
    })
    .unwrap();
    assert_eq!(listed.folders.len(), 1);
    assert_eq!(listed.folders[0].id, created.id);
    assert_eq!(
        identity::cmd_resolve(&created.sid_ref, &config.root)
            .unwrap()
            .folder
            .id,
        created.id
    );
    assert_eq!(identity::cmd_lint(&config).verdict(), LintVerdict::Healthy);
}

#[test]
fn entropy_is_lazy_once_and_precedes_planner_validation() {
    let workspace = tempfile::tempdir().unwrap();
    let mut config = identity::load_project_config(workspace.path()).unwrap();
    config.root = inspect::create_blocking_file(workspace.path());
    let calls = Cell::new(0);
    let tails = || {
        calls.set(calls.get() + 1);
        vec![RefTail { hi: '2', lo: 'a' }]
    };
    assert!(identity::cmd_new("title".into(), true, "202605".into(), &config, tails).is_err());
    assert_eq!(calls.get(), 0);
    config.root = workspace.path().join("missing");
    let tails = || {
        calls.set(calls.get() + 1);
        vec![RefTail { hi: '2', lo: 'a' }]
    };
    assert!(identity::cmd_new("...".into(), true, "202605".into(), &config, tails).is_err());
    assert_eq!(calls.get(), 1);
    inspect::assert_absent(&config.root);
}

#[test]
fn formatting_is_supplied_only_for_filtered_folders() {
    let workspace = tempfile::tempdir().unwrap();
    let config = identity::load_project_config(workspace.path()).unwrap();
    for title in ["keep", "skip"] {
        identity::cmd_new(title.into(), false, "202605".into(), &config, || {
            vec![RefTail { hi: '2', lo: 'a' }]
        })
        .unwrap();
    }
    let calls = Cell::new(0);
    let result = identity::cmd_list(vec!["keep".into()], ListSort::Id, &config, |_| {
        calls.set(calls.get() + 1);
        "timestamp".into()
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(result.folders.len(), 1);
    assert_eq!(result.folders[0].modified, "timestamp");
}
