use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Component, Path, PathBuf};

use crate::refcode::RefPrefixPolicy;

const DEFAULT_ROOT: &str = "stm";
const DEFAULT_SCAN_ROOT_NAMES: [&str; 4] = [".pending", ".prs", ".slow", ".archive"];
const DEFAULT_SEED_ROOT_NAME: &str = ".seeds";
const DEFAULT_NOTE_ROOT_NAME: &str = ".notes";
const DEFAULT_STALE_AFTER_DAYS: u64 = 7;

#[derive(Debug)]
pub struct ProjectConfig {
    /// Project base: the directory containing the discovered `.sid`, or the
    /// current working directory when none exists.
    pub base: PathBuf,
    pub active_root: PathBuf,
    pub scan_roots: Vec<PathBuf>,
    pub seed_root: PathBuf,
    pub note_root: PathBuf,
    pub topic_roots: Vec<PathBuf>,
    pub stale_after_days: u64,
    pub(crate) ref_prefix_policy: RefPrefixPolicy,
}

impl ProjectConfig {
    pub fn document_sources(&self) -> crate::documents::DocumentSources {
        crate::documents::DocumentSources {
            task_roots: self.scan_roots.clone(),
            seed_root: self.seed_root.clone(),
            topic_roots: self.topic_roots.clone(),
        }
    }

    /// Roots that reserve allocation refs. The seed pool shares the namespace
    /// but deliberately remains outside task listing and `new --into`.
    pub fn allocation_roots(&self) -> Vec<PathBuf> {
        let mut roots = self.scan_roots.clone();
        if !roots.contains(&self.seed_root) {
            roots.push(self.seed_root.clone());
        }
        roots
    }
}

#[derive(Debug, Serialize)]
pub struct InitConfigResult {
    pub path: PathBuf,
    pub created: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidConfigFile {
    #[serde(default)]
    task: SidTaskConfig,
    #[serde(default)]
    seed: SidSingleRootConfig,
    #[serde(default)]
    note: SidSingleRootConfig,
    #[serde(default)]
    topic: SidTopicConfig,
    #[serde(default)]
    queue: SidQueueConfig,
    #[serde(default, rename = "ref")]
    ref_config: SidRefConfig,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidTaskConfig {
    root: Option<PathBuf>,
    scan_roots: Option<Vec<PathBuf>>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidSingleRootConfig {
    root: Option<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidTopicConfig {
    #[serde(default)]
    roots: Vec<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidQueueConfig {
    stale_after_days: Option<u64>,
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

    let ref_prefix_policy = match config.ref_config.deny_prefixes {
        Some(prefixes) => RefPrefixPolicy::try_from_prefixes(prefixes)
            .with_context(|| format!("validate project config {}", base.join(".sid").display()))?,
        None => RefPrefixPolicy::prude(),
    };

    let configured_root = config
        .task
        .root
        .unwrap_or_else(|| PathBuf::from(DEFAULT_ROOT));
    validate_relative_path(&configured_root)?;
    let active_root = base.join(&configured_root);
    let scan_roots = config
        .task
        .scan_roots
        .unwrap_or_else(|| default_scan_roots_for(&configured_root));

    // Dedupe while preserving order: scan_roots means *additional* roots,
    // but a hand-written config may repeat the active root or an entry, and
    // duplicates would double list output and self-collide --into matching.
    let mut resolved_scan_roots = Vec::with_capacity(scan_roots.len() + 1);
    resolved_scan_roots.push(active_root.clone());
    for root in scan_roots {
        validate_relative_path(&root)?;
        let resolved = base.join(root);
        if !resolved_scan_roots.contains(&resolved) {
            resolved_scan_roots.push(resolved);
        }
    }

    let seed_root = config
        .seed
        .root
        .unwrap_or_else(|| configured_root.join(DEFAULT_SEED_ROOT_NAME));
    validate_relative_path(&seed_root)?;
    let note_root = config
        .note
        .root
        .unwrap_or_else(|| configured_root.join(DEFAULT_NOTE_ROOT_NAME));
    validate_relative_path(&note_root)?;
    let mut topic_roots = Vec::with_capacity(config.topic.roots.len());
    for root in config.topic.roots {
        validate_relative_path(&root)?;
        let resolved = base.join(root);
        if !topic_roots.contains(&resolved) {
            topic_roots.push(resolved);
        }
    }

    let seed_root = base.join(seed_root);
    let note_root = base.join(note_root);

    Ok(ProjectConfig {
        base,
        active_root,
        scan_roots: resolved_scan_roots,
        seed_root,
        note_root,
        topic_roots,
        stale_after_days: config
            .queue
            .stale_after_days
            .unwrap_or(DEFAULT_STALE_AFTER_DAYS),
        ref_prefix_policy,
    })
}

/// Walk from `cwd` to the filesystem root and load the nearest `.sid`. The
/// directory containing it is the project base that configured paths resolve
/// against, so `sid` allocates into the same tree from any subdirectory of a
/// configured project. With no `.sid` anywhere, the base is `cwd` itself and
/// defaults apply. A `.sid` that exists but cannot be read (including a
/// dangling symlink) fails closed at the level where it is found.
fn discover_config(cwd: &Path) -> Result<(PathBuf, SidConfigFile)> {
    for dir in cwd.ancestors() {
        let config_path = dir.join(".sid");
        match fs::read_to_string(&config_path) {
            Ok(contents) => {
                let config = toml::from_str::<SidConfigFile>(&contents)
                    .with_context(|| format!("parse project config {}", config_path.display()))?;
                return Ok((dir.to_path_buf(), config));
            }
            Err(err)
                if err.kind() == ErrorKind::NotFound && config_path.symlink_metadata().is_err() => {
            }
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("read project config {}", config_path.display()));
            }
        }
    }

    Ok((cwd.to_path_buf(), SidConfigFile::default()))
}

fn default_scan_roots_for(root: &Path) -> Vec<PathBuf> {
    DEFAULT_SCAN_ROOT_NAMES
        .into_iter()
        .map(|name| root.join(name))
        .collect()
}

fn default_config_toml() -> String {
    let scan_roots = default_scan_root_texts(DEFAULT_ROOT);
    let scan_roots_text = scan_roots
        .iter()
        .map(|root| format!("\"{root}\""))
        .collect::<Vec<_>>()
        .join(", ");

    format!(
        "[task]\nroot = \"{DEFAULT_ROOT}\"\nscan_roots = [{scan_roots_text}]\n\n[seed]\nroot = \"{DEFAULT_ROOT}/{DEFAULT_SEED_ROOT_NAME}\"\n\n[note]\nroot = \"{DEFAULT_ROOT}/{DEFAULT_NOTE_ROOT_NAME}\"\n\n[topic]\nroots = []\n\n[queue]\nstale_after_days = {DEFAULT_STALE_AFTER_DAYS}\n"
    )
}

fn default_scan_root_texts(root: &str) -> Vec<String> {
    DEFAULT_SCAN_ROOT_NAMES
        .into_iter()
        .map(|name| format!("{root}/{name}"))
        .collect()
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
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {
            // create_new refuses dangling symlinks with EEXIST even though no
            // config content exists; say so instead of "already exists". Only
            // NotFound counts as dangling — loops and permission errors keep
            // the conservative message.
            if path
                .symlink_metadata()
                .is_ok_and(|meta| meta.file_type().is_symlink())
                && matches!(fs::metadata(&path), Err(err) if err.kind() == ErrorKind::NotFound)
            {
                bail!(
                    "project config path is a dangling symlink: {}",
                    path.display()
                );
            }
            bail!("project config already exists: {}", path.display());
        }
        Err(err) => {
            return Err(err).with_context(|| format!("create project config {}", path.display()));
        }
    };

    let config = default_config_toml();
    file.write_all(config.as_bytes())
        .with_context(|| format!("write project config {}", path.display()))?;

    Ok(InitConfigResult {
        path,
        created: true,
    })
}
