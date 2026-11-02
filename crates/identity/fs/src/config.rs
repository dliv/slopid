use anyhow::{Context, Result, bail};
use slop_id_identity_core::{InitConfigResult, ProjectConfig, RootResult};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

pub fn load_project_config(cwd: &Path) -> Result<ProjectConfig> {
    let cwd = cwd
        .canonicalize()
        .with_context(|| format!("resolve current directory {}", cwd.display()))?;
    let (base, contents) = discover_config(&cwd)?;
    slop_id_identity_core::config::parse_project_config(base, contents.as_deref())
}
/// Find the nearest upward config. A missing path is skipped; a present but
/// unreadable path (including a dangling symlink) is an error at that level.
fn discover_config(cwd: &Path) -> Result<(PathBuf, Option<String>)> {
    for dir in cwd.ancestors() {
        let config_path = dir.join(".sid");
        match fs::read_to_string(&config_path) {
            Ok(contents) => {
                return Ok((dir.to_path_buf(), Some(contents)));
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

    Ok((cwd.to_path_buf(), None))
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
pub fn cmd_root(cwd: &Path) -> Result<RootResult> {
    let config = load_project_config(cwd)?;
    Ok(RootResult {
        base: config.base,
        root: config.root,
    })
}
