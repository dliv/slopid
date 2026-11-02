use crate::folders::folder_snapshot;
use slop_id_identity_core::resolve::{
    ResolveError, ResolveResult, parse_selector, resolve_matches,
};
use std::path::Path;
pub fn cmd_resolve(selector: &str, root: &Path) -> Result<ResolveResult, ResolveError> {
    let selector_kind = parse_selector(selector).ok_or_else(|| {
        ResolveError::Invalid(format!(
            "selector must be an exact recognized ref or complete identity folder name: {selector:?}"
        ))
    })?;
    let folders = folder_snapshot(root).map_err(ResolveError::Unavailable)?;
    resolve_matches(selector, selector_kind, folders)
}
