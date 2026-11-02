use crate::folders::{FolderIdentity, parse_identity_name};
use crate::refcode::is_recognized_ref;
use serde::Serialize;
use std::path::PathBuf;
#[derive(Clone, Debug, Serialize)]
pub struct AmbiguousCandidate {
    pub id: String,
    pub path: PathBuf,
}
#[derive(Debug)]
pub enum ResolveError {
    Invalid(String),
    NotFound(String),
    Ambiguous {
        message: String,
        candidates: Vec<AmbiguousCandidate>,
    },
    Unavailable(anyhow::Error),
}

#[derive(Debug, Serialize)]
pub struct ResolveResult {
    pub folder: FolderIdentity,
}

#[derive(Debug)]
pub enum Selector {
    Ref(String),
    FolderName(String),
}

pub fn resolve_matches(
    selector: &str,
    selector_kind: Selector,
    folders: Vec<FolderIdentity>,
) -> Result<ResolveResult, ResolveError> {
    let matches = folders
        .into_iter()
        .filter(|folder| match &selector_kind {
            Selector::Ref(sid_ref) => folder.sid_ref == *sid_ref,
            Selector::FolderName(id) => folder.id == *id,
        })
        .collect::<Vec<_>>();

    match matches.as_slice() {
        [] => Err(ResolveError::NotFound(format!(
            "no identity folder matches {selector:?}"
        ))),
        [folder] => Ok(ResolveResult {
            folder: folder.clone(),
        }),
        _ => {
            let candidates = matches
                .into_iter()
                .map(|folder| AmbiguousCandidate {
                    id: folder.id,
                    path: folder.path,
                })
                .collect();
            Err(ResolveError::Ambiguous {
                message: format!("short ref {selector:?} identifies multiple folders"),
                candidates,
            })
        }
    }
}

pub fn parse_selector(selector: &str) -> Option<Selector> {
    if selector.bytes().any(|byte| matches!(byte, b'/' | b'\\')) {
        return None;
    }
    if is_recognized_ref(selector) {
        return Some(Selector::Ref(selector.to_string()));
    }
    parse_identity_name(selector)?;
    Some(Selector::FolderName(selector.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_short_refs_and_complete_folder_names_are_the_only_selector_shapes() {
        for selector in ["sa2a7", "202610_sa2a7", "202610_sa2a7_slug"] {
            assert!(parse_selector(selector).is_some(), "{selector}");
        }
        for selector in [
            "sa2",
            "SA2A7",
            "../202610_sa2a7_slug",
            "202610_sa2a7_",
            "202610_sa2a7_slug/child",
            "202610_sa2a7_slug/",
            "202610_sa2a7_slug\\child",
        ] {
            assert!(parse_selector(selector).is_none(), "{selector}");
        }
    }
}
