//! Effectful assertions for the second root's assembled proof.
use std::path::{Path, PathBuf};

pub fn assert_absent(path: &Path) {
    assert!(!path.exists());
}

pub fn assert_only_empty_folder(root: &Path, folder: &Path) {
    assert!(folder.is_dir());
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
    assert_eq!(std::fs::read_dir(folder).unwrap().count(), 0);
}

pub fn create_blocking_file(base: &Path) -> PathBuf {
    let path = base.join("blocked");
    std::fs::write(&path, "not a directory").unwrap();
    path
}
