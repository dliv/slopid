use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use crate::refcode::RefPrefixPolicy;

const DEFAULT_ROOT: &str = "stm";

#[derive(Debug)]
pub struct ProjectConfig {
    /// Directory containing the discovered `.sid`, or cwd when none exists.
    pub base: PathBuf,
    /// The one logical identity root. It need not exist yet.
    pub root: PathBuf,
    pub(crate) ref_prefix_policy: RefPrefixPolicy,
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

pub fn load_project_config(cwd: &Path) -> Result<ProjectConfig> {
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolve current directory {}", cwd.display()))?;
    let (base, config) = discover_config(&cwd)?;
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

/// Find the nearest upward config. A missing path is skipped; a present but
/// unreadable path (including a dangling symlink) is an error at that level.
fn discover_config(cwd: &Path) -> Result<(PathBuf, SidConfigFile)> {
    for dir in cwd.ancestors() {
        let config_path = dir.join(".sid");
        match fs::read_to_string(&config_path) {
            Ok(contents) => {
                let config = toml::from_str::<SidConfigFile>(&contents)
                    .with_context(|| format!("parse project config {}", config_path.display()))?;
                return Ok((dir.to_path_buf(), config));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match fs::symlink_metadata(&config_path) {
                    Err(metadata_error) if metadata_error.kind() == ErrorKind::NotFound => continue,
                    Ok(_) => {
                        return Err(error).with_context(|| {
                            format!("read project config {}", config_path.display())
                        });
                    }
                    Err(metadata_error) => {
                        return Err(metadata_error).with_context(|| {
                            format!("inspect project config {}", config_path.display())
                        });
                    }
                }
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("read project config {}", config_path.display()));
            }
        }
    }

    Ok((cwd.to_path_buf(), SidConfigFile::default()))
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

pub fn init_config(cwd: &Path) -> Result<InitConfigResult> {
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolve current directory {}", cwd.display()))?;
    let path = cwd.join(".sid");
    let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            if path
                .symlink_metadata()
                .is_ok_and(|metadata| metadata.file_type().is_symlink())
                && matches!(fs::metadata(&path), Err(error) if error.kind() == ErrorKind::NotFound)
            {
                bail!(
                    "project config path is a dangling symlink: {}",
                    path.display()
                );
            }
            bail!("project config already exists: {}", path.display());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("create project config {}", path.display()));
        }
    };

    file.write_all(b"root = \"stm\"\n")
        .with_context(|| format!("write project config {}", path.display()))?;

    Ok(InitConfigResult {
        path,
        created: true,
    })
}
