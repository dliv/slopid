//! Consumer-owned native baseline: engine profiles cannot silently widen it.
use crate::support;
use std::collections::BTreeSet;

// Snapshot of this product's adopted controls, not the union of consumer policies.
const REQUIRED_METHODS: &[&str] = &[
    "std::env::args",
    "std::env::args_os",
    "std::env::var",
    "std::env::var_os",
    "std::env::current_dir",
    "std::env::set_current_dir",
    "std::time::SystemTime::now",
    "std::time::Instant::now",
    "chrono::Local::now",
    "chrono::Utc::now",
    "fastrand::shuffle",
    "std::process::exit",
    "std::process::abort",
    "std::process::Command::new",
    "std::io::stdin",
    "std::io::stdout",
    "std::io::stderr",
];
const REQUIRED_MACROS: &[&str] = &[
    "std::print",
    "std::println",
    "std::eprint",
    "std::eprintln",
    "std::dbg",
];
const REQUIRED_LINTS: &[(&str, &str, &str)] = &[
    ("rust", "unfulfilled_lint_expectations", "deny"),
    ("clippy", "disallowed_methods", "deny"),
    ("clippy", "disallowed_macros", "deny"),
];

fn configured_names(config: &toml::Value, key: &str) -> BTreeSet<String> {
    let Some(entries) = config.get(key) else {
        return BTreeSet::new();
    };
    let entries = entries.as_array().expect("native list must be an array");
    let names: BTreeSet<String> = entries
        .iter()
        .map(|value| {
            value
                .as_str()
                .or_else(|| value.get("path").and_then(toml::Value::as_str))
                .expect("native entry must name a path")
                .to_owned()
        })
        .collect();
    assert_eq!(
        names.len(),
        entries.len(),
        "duplicate native paths in {key}"
    );
    names
}

#[test]
fn native_controls_match_local_baseline() {
    let root = support::root();
    let clippy: toml::Value = std::fs::read_to_string(root.join("clippy.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let manifest: toml::Value = std::fs::read_to_string(root.join("Cargo.toml"))
        .unwrap()
        .parse()
        .unwrap();
    let methods = configured_names(&clippy, "disallowed-methods");
    let macros = configured_names(&clippy, "disallowed-macros");
    let mut problems = Vec::new();
    for name in REQUIRED_METHODS {
        if !methods.contains(*name) {
            problems.push(format!("missing native method control: {name}"));
        }
    }
    for name in REQUIRED_MACROS {
        if !macros.contains(*name) {
            problems.push(format!("missing native macro control: {name}"));
        }
    }
    for (group, name, expected) in REQUIRED_LINTS {
        let value = manifest
            .get("workspace")
            .and_then(|v| v.get("lints"))
            .and_then(|v| v.get(*group))
            .and_then(|v| v.get(*name));
        let level = value.and_then(|v| {
            v.as_str()
                .or_else(|| v.get("level").and_then(toml::Value::as_str))
        });
        if level != Some(*expected)
            && level != Some("forbid")
            && !(level == Some("deny") && *expected == "warn")
        {
            problems.push(format!(
                "native lint {group}.{name} must remain {expected} or stronger"
            ));
        }
    }
    // ArgsOs is a type; all permitted environment functions must be natively controlled.
    for name in support::ROOT_ALLOWED_ENV {
        if *name != "ArgsOs" && !methods.contains(&format!("std::env::{name}")) {
            problems.push(format!(
                "Root environment permission has no native control: std::env::{name}"
            ));
        }
    }
    assert!(problems.is_empty(), "native policy drift: {problems:?}");
}
