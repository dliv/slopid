use arch_gates::graph::{Edge, module_id};
use arch_gates::packages::CrateClass;
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Root,
    Pure,
    Effectful,
}

pub const MODULES: &[(&str, Role)] = &[
    ("src/agent_instructions.rs", Role::Root),
    ("src/bin/sid.rs", Role::Root),
    ("src/cli.rs", Role::Root),
    ("src/failure.rs", Role::Root),
    ("src/inputs.rs", Role::Root),
    ("src/lib.rs", Role::Root),
    ("src/refcode.rs", Role::Root),
    ("src/scan.rs", Role::Root),
    ("src/slug.rs", Role::Root),
    ("crates/identity/core/src/allocation.rs", Role::Pure),
    ("crates/identity/core/src/config.rs", Role::Pure),
    ("crates/identity/core/src/folders.rs", Role::Pure),
    ("crates/identity/core/src/lib.rs", Role::Pure),
    ("crates/identity/core/src/lint.rs", Role::Pure),
    ("crates/identity/core/src/list.rs", Role::Pure),
    ("crates/identity/core/src/names.rs", Role::Pure),
    ("crates/identity/core/src/refcode.rs", Role::Pure),
    ("crates/identity/core/src/resolve.rs", Role::Pure),
    ("crates/identity/core/src/slug.rs", Role::Pure),
    ("crates/identity/fs/src/allocation.rs", Role::Effectful),
    ("crates/identity/fs/src/config.rs", Role::Effectful),
    ("crates/identity/fs/src/folders.rs", Role::Effectful),
    ("crates/identity/fs/src/lib.rs", Role::Effectful),
    ("crates/identity/fs/src/lint.rs", Role::Effectful),
    ("crates/identity/fs/src/list.rs", Role::Effectful),
    ("crates/identity/fs/src/resolve.rs", Role::Effectful),
    ("crates/identity/fs/src/scan.rs", Role::Effectful),
    (
        "crates/identity/fs/test-support/src/inspect.rs",
        Role::Effectful,
    ),
    (
        "crates/identity/fs/test-support/src/second_root.rs",
        Role::Root,
    ),
];

pub fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

pub fn module_files() -> Vec<&'static str> {
    MODULES.iter().map(|(file, _)| *file).collect()
}

pub fn class_of(crate_dir: &str, path: &str) -> Role {
    MODULES
        .iter()
        .find_map(|(file, role)| {
            let id = module_id(file).unwrap();
            (id.crate_dir == crate_dir && id.path == path).then_some(*role)
        })
        .unwrap_or_else(|| panic!("unclassified module {crate_dir}:{path}"))
}

pub fn permits(edge: &Edge) -> bool {
    match class_of(&edge.crate_dir, &edge.from) {
        Role::Root => true,
        Role::Pure => class_of(&edge.crate_dir, &edge.to) == Role::Pure,
        Role::Effectful => matches!(
            class_of(&edge.crate_dir, &edge.to),
            Role::Pure | Role::Effectful
        ),
    }
}

fn walk(root: &Path, directory: &Path, found: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(root, &path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.insert(
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

/// Consumer-local observation; discover unknown product members as well as the current app.
pub fn discovered_files() -> BTreeSet<String> {
    let root = root();
    let mut found = BTreeSet::new();
    for member in workspace_members() {
        if member.name == "slop-id-policy" {
            continue;
        }
        walk(&root, &root.join(&member.dir).join("src"), &mut found);
    }
    walk(
        &root,
        &root.join("crates/identity/fs/test-support/src"),
        &mut found,
    );
    assert!(!found.is_empty(), "no production Rust sources discovered");
    found
}

/// Observation stays local; engine parses only caller-supplied JSON/manifest text.
pub fn workspace_members() -> Vec<arch_gates::metadata::Member> {
    arch_gates::metadata::parse_members(&slop_id_policy::cargo_metadata(&root()).unwrap())
        .unwrap()
        .into_iter()
        .map(|listed| {
            let text = std::fs::read_to_string(&listed.manifest_path).unwrap();
            listed.with_manifest(&text)
        })
        .collect()
}

// Environment names accepted by the Root source scan. Callable names are paired
// with the real native ban list in checks/native.rs. No current_exe permission.
pub const ROOT_ALLOWED_ENV: &[&str] = &["args_os", "ArgsOs", "var", "var_os", "current_dir"];

// Current selected Pure modules need only these external dependencies.
pub const PURE_ALLOWED: &[(&str, &[&str])] = &[
    ("anyhow", &[]),
    ("serde", &["derive"]),
    ("toml", &["parse"]),
];

// One folder-identity feature, with build-enforced pure/filesystem boundaries.
pub const CRATES: &[(&str, CrateClass)] = &[
    ("slop-id", CrateClass::Binary),
    ("slop-id-policy", CrateClass::Tooling),
    (
        "slop-id-identity-core",
        CrateClass::Pure {
            feature: "identity",
        },
    ),
    (
        "slop-id-identity-fs",
        CrateClass::Effectful {
            feature: "identity",
        },
    ),
];

pub const PROOF_MODULES: &[(&str, Role)] = &[
    (
        "crates/identity/fs/test-support/src/second_root.rs",
        Role::Root,
    ),
    (
        "crates/identity/fs/test-support/src/inspect.rs",
        Role::Effectful,
    ),
];

/// Bind source profiles to actual workspace ownership, with exact test-target roles.
pub fn source_role_violations(
    members: &[arch_gates::metadata::Member],
    modules: &[(&str, Role)],
    proof: &[(&str, Role)],
) -> Vec<String> {
    let mut problems = Vec::new();
    let mut seen = BTreeSet::new();
    for (file, _) in proof {
        if !seen.insert(*file) {
            problems.push(format!("duplicate proof source role: {file}"));
        }
        if modules.iter().filter(|(source, _)| source == file).count() != 1 {
            problems.push(format!(
                "missing proof source or duplicate classification: {file}"
            ));
        }
    }
    for (file, actual) in modules {
        let Some(id) = module_id(file) else {
            problems.push(format!("invalid source path: {file}"));
            continue;
        };
        let exception = proof.iter().find(|(source, _)| source == file);
        let owner_dir = if exception.is_some() {
            // Only this package's named test-support target is a second root.
            let Some(owner) = id.crate_dir.strip_suffix("/test-support") else {
                problems.push(format!(
                    "proof source is not in a named test-support target: {file}"
                ));
                continue;
            };
            owner
        } else {
            &id.crate_dir
        };
        let Some(owner) = members.iter().find(|member| member.dir == owner_dir) else {
            problems.push(format!("source has no workspace owner: {file}"));
            continue;
        };
        let Some((_, class)) = CRATES.iter().find(|(name, _)| *name == owner.name) else {
            problems.push(format!(
                "source owner has no package class: {file}: {}",
                owner.name
            ));
            continue;
        };
        let required = if let Some((_, role)) = exception {
            if owner.name != "slop-id-identity-fs" || !matches!(class, CrateClass::Effectful { .. })
            {
                problems.push(format!(
                    "proof source has an unsupported owner: {file}: {}",
                    owner.name
                ));
                continue;
            }
            *role
        } else {
            match class {
                CrateClass::Pure { .. } => Role::Pure,
                CrateClass::Effectful { .. } => Role::Effectful,
                CrateClass::Binary => Role::Root,
                CrateClass::Edge | CrateClass::Tooling => {
                    problems.push(format!(
                        "unsupported production source owner: {file}: {}",
                        owner.name
                    ));
                    continue;
                }
            }
        };
        if *actual != required {
            problems.push(format!("source/package role mismatch: {file} is {actual:?}, package {} requires {required:?}", owner.name));
        }
    }
    problems
}
