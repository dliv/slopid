use crate::refcode::RefPrefixPolicy;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

const DEFAULT_ROOT: &str = "stm";

#[derive(Debug)]
pub struct ProjectConfig {
    /// Directory containing the discovered `.sid`, or cwd when none exists.
    pub base: PathBuf,
    /// The one logical identity root. It need not exist yet.
    pub root: PathBuf,
    pub ref_prefix_policy: RefPrefixPolicy,
}

#[derive(Debug, Serialize)]
pub struct InitConfigResult {
    pub path: PathBuf,
    pub created: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidConfigFile {
    root: Option<PathBuf>,
    #[serde(default, rename = "ref")]
    ref_config: SidRefConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidRefConfig {
    deny_prefixes: Option<Vec<String>>,
}

pub fn parse_project_config(base: PathBuf, contents: Option<&str>) -> Result<ProjectConfig> {
    let config = match contents {
        Some(contents) => toml::from_str::<SidConfigFile>(contents)
            .with_context(|| format!("parse project config {}", base.join(".sid").display()))?,
        None => SidConfigFile::default(),
    };
    let root = config.root.unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT));
    validate_relative_path(&root)?;

    let ref_prefix_policy = match config.ref_config.deny_prefixes {
        Some(prefixes) => RefPrefixPolicy::try_from_prefixes(prefixes)
            .with_context(|| format!("validate project config {}", base.join(".sid").display()))?,
        None => RefPrefixPolicy::prude(),
    };

    Ok(ProjectConfig {
        root: base.join(root),
        base,
        ref_prefix_policy,
    })
}

fn validate_relative_path(path: &Path) -> Result<()> {
    if path.is_absolute() {
        bail!("project config paths must be relative: {}", path.display());
    }
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        bail!(
            "project config paths may not contain '..': {}",
            path.display()
        );
    }
    if !path
        .components()
        .any(|component| matches!(component, Component::Normal(_)))
    {
        bail!(
            "project config paths must name a directory inside the project: \"{}\"",
            path.display()
        );
    }
    Ok(())
}

#[derive(Debug, Serialize)]
pub struct RootResult {
    pub base: PathBuf,
    pub root: PathBuf,
}

pub fn render_root_human(result: &RootResult) -> String {
    format!("{}\n", result.root.display())
}
