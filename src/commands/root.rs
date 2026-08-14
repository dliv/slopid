use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::commands::load_project_config;

#[derive(Debug, Serialize)]
pub struct RootResult {
    pub root: PathBuf,
}

pub fn cmd_root(cwd: &Path) -> Result<RootResult> {
    let config = load_project_config(cwd)?;
    Ok(RootResult {
        root: config.active_root,
    })
}

pub fn render_root_human(result: &RootResult) -> String {
    format!("{}\n", result.root.display())
}
