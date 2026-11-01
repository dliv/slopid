//! Source policy over the consumer-owned role map; trait acceptance is not adopted here.
use crate::support::{self, Role};
use arch_gates::manifest::dependency_crate_names;
use arch_gates::scan::{external_crate_references, root_ambient_violations};
use std::collections::BTreeSet;

#[test]
fn root_modules_have_no_forbidden_effects() {
    let mut scanned = 0;
    let mut problems = Vec::new();
    for (file, role) in support::MODULES {
        if *role != Role::Root {
            continue;
        }
        scanned += 1;
        let source = std::fs::read_to_string(support::root().join(file)).unwrap();
        for form in root_ambient_violations(&source, support::ROOT_ALLOWED_ENV) {
            problems.push(format!("{file}: {form}"));
        }
    }
    assert!(scanned > 0, "no Root files scanned");
    assert!(problems.is_empty(), "Root source violations: {problems:?}");
}

#[test]
fn root_profile_keeps_slopid_permissions() {
    for source in [
        "fn f() { let _ = std::env::current_exe(); }",
        "use std::env as env; fn f() { let _ = env::current_exe(); }",
        "fn f() { let _ = std::fs::read(\"x\"); }",
    ] {
        assert!(
            !root_ambient_violations(source, support::ROOT_ALLOWED_ENV).is_empty(),
            "{source}"
        );
    }
    for source in [
        "fn f() { let _ = std::env::current_dir(); }",
        "use std::env::var as read_var; fn f() { let _ = read_var(\"X\"); }",
        "fn f() { let _ = std::env::args_os(); }",
    ] {
        assert!(
            root_ambient_violations(source, support::ROOT_ALLOWED_ENV).is_empty(),
            "{source}"
        );
    }
}

#[test]
fn pure_modules_use_only_local_external_allowlist() {
    let allowed: BTreeSet<String> = support::PURE_ALLOWED
        .iter()
        .map(|(name, _)| (*name).into())
        .collect();
    let mut used = BTreeSet::new();
    let mut problems = Vec::new();
    let root = support::root();
    let mut scanned = 0;
    for (file, role) in support::MODULES {
        if *role != Role::Pure {
            continue;
        }
        scanned += 1;
        let id = arch_gates::graph::module_id(file).unwrap();
        let manifest = std::fs::read_to_string(root.join(id.crate_dir).join("Cargo.toml")).unwrap();
        let dependencies = dependency_crate_names(&manifest);
        assert!(
            !dependencies.is_empty(),
            "no dependencies observed for {file}"
        );
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        let references = external_crate_references(&source, &dependencies);
        for name in references.difference(&allowed) {
            problems.push(format!("{file}: crate {name}"));
        }
        used.extend(references);
    }
    assert!(scanned > 0, "no Pure files scanned");
    for name in allowed.difference(&used) {
        problems.push(format!("stale Pure external allowance: {name}"));
    }
    assert!(
        problems.is_empty(),
        "Pure external dependency violations: {problems:?}"
    );
}
