use crate::support;
use arch_gates::graph::{Edge, check_coverage, check_direction, crate_edges, module_id};
use std::collections::BTreeSet;
use support::{MODULES, Role};

// Current root-package exceptions, removed with the later production split.
const MIXED_ROOT_FILES: &[&str] = &[
    "src/scan.rs",
    "src/commands/mod.rs",
    "src/commands/config.rs",
    "src/commands/folders.rs",
    "src/commands/lint.rs",
    "src/commands/list.rs",
    "src/commands/new.rs",
    "src/commands/resolve.rs",
    "src/commands/root.rs",
];

fn exceptions() -> Vec<Edge> {
    // resolve needs AmbiguousCandidate, currently colocated with root output/exit.
    // Remove this exact exception when outcome types move to the pure core.
    vec![Edge {
        crate_dir: "".into(),
        from: "commands::resolve".into(),
        to: "failure".into(),
    }]
}

#[test]
fn every_source_file_has_exactly_one_class() {
    let problems = check_coverage(&support::discovered_files(), &support::module_files());
    assert!(
        problems.is_empty(),
        "source coverage violations: {problems:?}"
    );
}

#[test]
fn mixed_root_exceptions_are_exact_and_nonstale() {
    let actual: BTreeSet<&str> = MODULES
        .iter()
        .filter(|(_, role)| *role == Role::Effectful)
        .map(|(file, _)| *file)
        .collect();
    let expected: BTreeSet<&str> = MIXED_ROOT_FILES.iter().copied().collect();
    assert_eq!(
        expected.len(),
        MIXED_ROOT_FILES.len(),
        "duplicate mixed-root exception"
    );
    assert_eq!(
        actual, expected,
        "mixed-root exceptions must match current Effectful files; remove stale allowances and review additions"
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
    let problems = check_direction(&edges, &exceptions(), support::permits);
    assert!(
        problems.is_empty(),
        "import direction violations: {problems:?}"
    );
}
