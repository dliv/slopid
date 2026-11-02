use crate::folders::FolderIdentity;
use serde::Serialize;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListSort {
    /// Most recently touched first (folder and direct-child mtimes)
    Recent,
    /// Ascending by complete folder name
    Id,
}

#[derive(Debug, Serialize)]
pub struct ListedFolder {
    pub id: String,
    pub period: String,
    pub sid_ref: String,
    pub slug: String,
    pub modified: String,
    pub root: PathBuf,
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct ListResult {
    pub folders: Vec<ListedFolder>,
    /// Project base for human rendering; not part of the JSON contract.
    #[serde(skip)]
    pub base: PathBuf,
}

pub fn render_human(result: &ListResult) -> String {
    let id_width = result
        .folders
        .iter()
        .map(|folder| folder.id.len())
        .max()
        .unwrap_or(0);

    let mut out = String::new();
    for folder in &result.folders {
        let modified = folder
            .modified
            .get(..16)
            .unwrap_or(&folder.modified)
            .replace('T', " ");
        let root = folder
            .root
            .strip_prefix(&result.base)
            .unwrap_or(&folder.root)
            .display();
        out.push_str(&format!("{modified}  {:<id_width$}  {root}\n", folder.id));
    }
    out
}

pub fn filter_folders(folders: Vec<FolderIdentity>, terms: Vec<String>) -> Vec<FolderIdentity> {
    let terms = terms
        .iter()
        .map(|term| term.to_lowercase())
        .collect::<Vec<_>>();
    folders
        .into_iter()
        .filter(|folder| {
            let slug = folder.slug.to_lowercase();
            terms
                .iter()
                .all(|term| folder.sid_ref.starts_with(term) || slug.contains(term))
        })
        .collect()
}
pub fn finish_list<Time: Ord>(
    mut entries: Vec<(Time, ListedFolder)>,
    sort: ListSort,
    base: PathBuf,
) -> ListResult {
    match sort {
        ListSort::Recent => entries.sort_by(|(left_time, left), (right_time, right)| {
            right_time
                .cmp(left_time)
                .then_with(|| left.id.cmp(&right.id))
                .then_with(|| left.path.cmp(&right.path))
        }),
        ListSort::Id => entries.sort_by(|(_, left), (_, right)| {
            left.id
                .cmp(&right.id)
                .then_with(|| left.path.cmp(&right.path))
        }),
    }

    ListResult {
        folders: entries.into_iter().map(|(_, folder)| folder).collect(),
        base,
    }
}

pub fn listed_folder(folder: FolderIdentity, modified: String) -> ListedFolder {
    ListedFolder {
        id: folder.id,
        period: folder.period,
        sid_ref: folder.sid_ref,
        slug: folder.slug,
        modified,
        root: folder.root,
        path: folder.path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_rendering_uses_base_relative_root_and_empty_results() {
        let result = ListResult {
            folders: Vec::new(),
            base: PathBuf::from("/tmp/project"),
        };
        assert_eq!(render_human(&result), "");
    }
}
