use crate::{names::split_task_folder_name, refcode::is_recognized_ref};
use serde::Serialize;
use std::path::PathBuf;
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct FolderIdentity {
    pub id: String,
    pub period: String,
    pub sid_ref: String,
    pub slug: String,
    pub root: PathBuf,
    pub path: PathBuf,
}

/// Parse the complete folder spelling. Slugs are opaque, nonempty suffixes;
/// legacy slugless identities remain valid.
pub fn parse_identity_name(name: &str) -> Option<(String, String, String)> {
    let (period, sid_ref) = split_task_folder_name(name)?;
    if !is_recognized_ref(sid_ref) {
        return None;
    }
    let slug = match name.splitn(3, '_').nth(2) {
        None => String::new(),
        Some("") => return None,
        Some(slug) => slug.to_string(),
    };
    Some((period.to_string(), sid_ref.to_string(), slug))
}
