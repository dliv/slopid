use crate::support;
use arch_gates::graph::{Edge, check_coverage, check_direction, crate_edges, module_id};
use std::collections::BTreeSet;
use support::{MODULES, Role};

#[test]
fn every_source_file_has_exactly_one_class() {
    let problems = check_coverage(&support::discovered_files(), &support::module_files());
    assert!(
        problems.is_empty(),
        "source coverage violations: {problems:?}"
    );
}

#[test]
fn production_source_roles_match_owning_packages() {
    let problems = support::source_role_violations(
        &support::workspace_members(),
        MODULES,
        support::PROOF_MODULES,
    );
    assert!(
        problems.is_empty(),
        "source/package role violations: {problems:?}"
    );
}

#[test]
fn reclassifying_production_cannot_disable_its_package_profile() {
    let members = support::workspace_members();
    for (file, wrong_role) in [
        ("crates/identity/core/src/lib.rs", Role::Effectful),
        ("crates/identity/fs/src/lib.rs", Role::Root),
        ("src/lib.rs", Role::Pure),
    ] {
        let changed: Vec<_> = MODULES
            .iter()
            .map(|(path, role)| (*path, if *path == file { wrong_role } else { *role }))
            .collect();
        let problems = support::source_role_violations(&members, &changed, support::PROOF_MODULES);
        assert!(
            problems
                .iter()
                .any(|p| p.contains("role mismatch") && p.contains(file)),
            "classification must not escape package profile: {file}: {problems:?}"
        );
    }
}

#[test]
fn proof_role_exceptions_are_exact_and_nonstale() {
    let members = support::workspace_members();
    let without_proof: Vec<_> = MODULES
        .iter()
        .copied()
        .filter(|(file, _)| {
            !support::PROOF_MODULES
                .iter()
                .any(|(proof, _)| proof == file)
        })
        .collect();
    assert!(
        support::source_role_violations(&members, &without_proof, support::PROOF_MODULES)
            .iter()
            .any(|p| p.contains("missing proof"))
    );
    let mut duplicated = support::PROOF_MODULES.to_vec();
    duplicated.push(support::PROOF_MODULES[0]);
    assert!(
        support::source_role_violations(&members, MODULES, &duplicated)
            .iter()
            .any(|p| p.contains("duplicate proof"))
    );
    let mut unknown = MODULES.to_vec();
    unknown.push(("crates/not-a-member/src/lib.rs", Role::Pure));
    assert!(
        support::source_role_violations(&members, &unknown, support::PROOF_MODULES)
            .iter()
            .any(|p| p.contains("no workspace owner"))
    );
}

#[test]
fn every_import_follows_local_direction_or_an_exact_exception() {
    let root = support::root();
    let mut edges = BTreeSet::new();
    for (file, _) in MODULES {
        let id = module_id(file).unwrap();
        let known: Vec<String> = MODULES
            .iter()
            .map(|(file, _)| module_id(file).unwrap())
            .filter(|known| known.crate_dir == id.crate_dir)
            .map(|id| id.path)
            .collect();
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        for to in crate_edges(&id.path, &source, &known) {
            edges.insert(Edge {
                crate_dir: id.crate_dir.clone(),
                from: id.path.clone(),
                to,
            });
        }
    }
    assert!(!edges.is_empty(), "direction scan found no edges");
    let problems = check_direction(&edges, &[], support::permits);
    assert!(
        problems.is_empty(),
        "import direction violations: {problems:?}"
    );
}
