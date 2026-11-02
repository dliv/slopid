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
fn production_packages_have_no_mixed_root_allowances() {
    assert!(
        MODULES
            .iter()
            .all(|(file, role)| !file.starts_with("src/") || *role == Role::Root)
    );
    assert!(MODULES.iter().any(|(_, role)| *role == Role::Pure));
    assert!(MODULES.iter().any(|(_, role)| *role == Role::Effectful));
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
