use std::path::Path;

// File selection is Slopid policy; mixed files await later separation.
const PURE_FILES: &[&str] = &[
    "src/refcode.rs",
    "src/slug.rs",
    "src/commands/agent_instructions.rs",
];

#[test]
fn pure_modules_have_no_ambient_effects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    assert!(!PURE_FILES.is_empty());
    let mut violations = Vec::new();
    for file in PURE_FILES {
        let source = std::fs::read_to_string(root.join(file)).unwrap();
        violations.extend(arch_gates::check_pure_source(file, &source));
    }
    assert!(
        violations.is_empty(),
        "Pure source violations: {violations:?}"
    );
}

#[test]
fn metadata_observation_uses_the_consumer_root_and_fails_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let metadata = slop_id_policy::cargo_metadata(&root).unwrap();
    assert!(metadata.contains("slop-id-policy"));
    assert!(slop_id_policy::cargo_metadata(&root.join("missing-policy-root")).is_err());
}

#[test]
fn shared_checker_control() {
    assert!(
        arch_gates::check_pure_source("valid.rs", "fn f(s: &str) -> usize { s.len() }").is_empty()
    );
    let found = arch_gates::check_pure_source(
        "planted.rs",
        "use std::fs as disk; fn f() { disk::read(\"x\"); }",
    );
    assert_eq!(found[0].form, "std::fs");
}
