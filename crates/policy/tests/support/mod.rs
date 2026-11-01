use arch_gates::graph::{Edge, module_id};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Root,
    Pure,
    Effectful,
}

pub const MODULES: &[(&str, Role)] = &[
    ("src/bin/sid.rs", Role::Root),
    ("src/lib.rs", Role::Root),
    ("src/cli.rs", Role::Root),
    ("src/failure.rs", Role::Root),
    ("src/refcode.rs", Role::Pure),
    ("src/slug.rs", Role::Pure),
    ("src/commands/agent_instructions.rs", Role::Pure),
    ("src/scan.rs", Role::Effectful),
    ("src/commands/mod.rs", Role::Effectful),
    ("src/commands/config.rs", Role::Effectful),
    ("src/commands/folders.rs", Role::Effectful),
    ("src/commands/lint.rs", Role::Effectful),
    ("src/commands/list.rs", Role::Effectful),
    ("src/commands/new.rs", Role::Effectful),
    ("src/commands/resolve.rs", Role::Effectful),
    ("src/commands/root.rs", Role::Effectful),
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
    let metadata: serde_json::Value =
        serde_json::from_str(&slop_id_policy::cargo_metadata(&root).unwrap()).unwrap();
    let members = metadata["workspace_members"].as_array().unwrap();
    assert!(!members.is_empty(), "workspace has no members");
    let mut found = BTreeSet::new();
    for member in members {
        let package = metadata["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|package| package["id"] == *member)
            .expect("workspace member absent from packages");
        if package["name"] == "slop-id-policy" {
            continue;
        }
        let manifest = Path::new(package["manifest_path"].as_str().unwrap());
        walk(&root, &manifest.parent().unwrap().join("src"), &mut found);
    }
    assert!(!found.is_empty(), "no production Rust sources discovered");
    found
}
