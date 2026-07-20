use crate::documents::{Finding, FindingCode};
use ignore::WalkBuilder;
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct WalkOutcome {
    pub paths: Vec<PathBuf>,
    pub findings: Vec<Finding>,
    pub usable: bool,
}

/// Deterministically walk one configured source without inheriting global
/// user ignores or ignore rules from an outer repository. Explicit exclusions
/// are absolute subtrees owned by another typed source or a verbatim queue.
pub fn walk_files(root: &Path, exclusions: &[PathBuf], prune_inbox: bool) -> WalkOutcome {
    match fs::metadata(root) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => {
            return WalkOutcome {
                findings: vec![Finding::error(
                    FindingCode::UnreadableRoot,
                    "configured source root is not a directory",
                    Some(absolute(root)),
                )],
                ..WalkOutcome::default()
            };
        }
        Err(err) if err.kind() == ErrorKind::NotFound => return WalkOutcome::default(),
        Err(err) => {
            return WalkOutcome {
                findings: vec![Finding::error(
                    FindingCode::UnreadableRoot,
                    format!("cannot inspect configured source root: {err}"),
                    Some(absolute(root)),
                )],
                ..WalkOutcome::default()
            };
        }
    }

    let root = absolute(root);
    let exclusions = exclusions
        .iter()
        .map(|path| absolute(path))
        .collect::<Vec<_>>();
    let mut builder = WalkBuilder::new(&root);
    builder
        .parents(false)
        .hidden(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .follow_links(false)
        .threads(1);
    add_boundary_ignores(&mut builder, &root);
    builder.filter_entry(move |entry| {
        let path = entry.path();
        if path == root {
            return true;
        }
        if exclusions.iter().any(|excluded| path.starts_with(excluded)) {
            return false;
        }
        let name = entry.file_name().to_string_lossy();
        if entry.file_type().is_some_and(|kind| kind.is_dir())
            && matches!(name.as_ref(), ".git" | ".hg" | ".svn" | "tmp")
        {
            return false;
        }
        !(prune_inbox && entry.file_type().is_some_and(|kind| kind.is_dir()) && name == "inbox")
    });

    let mut outcome = WalkOutcome {
        usable: true,
        ..WalkOutcome::default()
    };
    let mut paths = BTreeSet::new();
    for entry in builder.build() {
        match entry {
            Ok(entry) if entry.file_type().is_some_and(|kind| kind.is_file()) => {
                paths.insert(absolute(entry.path()));
            }
            Ok(_) => {}
            Err(err) => outcome.findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                format!("cannot walk source entry: {err}"),
                None,
            )),
        }
    }
    outcome.paths = paths.into_iter().collect();
    outcome
}

fn add_boundary_ignores(builder: &mut WalkBuilder, root: &Path) {
    let boundary = root
        .ancestors()
        .find(|ancestor| ancestor.join(".git").exists());
    let Some(boundary) = boundary else {
        return;
    };
    let mut ancestors = root
        .ancestors()
        .take_while(|ancestor| *ancestor != boundary)
        .collect::<Vec<_>>();
    ancestors.push(boundary);
    ancestors.reverse();
    for ancestor in ancestors {
        for name in [".gitignore", ".ignore"] {
            let ignore = ancestor.join(name);
            if ignore.is_file() {
                let _ = builder.add_ignore(ignore);
            }
        }
    }
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir().unwrap_or_default().join(path)
    }
}
