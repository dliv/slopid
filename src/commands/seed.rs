use crate::documents;
use anyhow::{Context, Result, bail};
use chrono::Local;
use serde_json::json;
use std::collections::HashSet;
use std::io::{ErrorKind, Write};
use std::path::Path;

use super::{
    NewCandidate, NewPlan, NewResult, current_period, load_project_config, plan_random_new,
    scan_roots,
};

#[derive(Debug)]
pub struct SeedInputs {
    pub title: String,
    pub origins: Vec<String>,
    pub dry_run: bool,
    pub body: String,
}

pub fn cmd_seed(inputs: SeedInputs, cwd: &Path) -> Result<NewResult> {
    let config = load_project_config(cwd)?;
    let mut seen = HashSet::new();
    for origin in &inputs.origins {
        if !seen.insert(origin.clone()) {
            bail!("origin id is repeated: {origin}");
        }
    }
    let index = documents::scan_sources(&config.document_sources());
    for origin in &inputs.origins {
        if !index.nodes.contains_key(origin) {
            bail!("origin id does not resolve exactly: {origin}");
        }
    }
    let snapshot = scan_roots(&config.allocation_roots())?;
    let task_plan = plan_random_new(
        inputs.title,
        config.seed_root.clone(),
        current_period(),
        &snapshot,
    )?;
    let seed_plan = seed_paths(task_plan);
    if inputs.dry_run {
        return Ok(seed_result(&seed_plan, &seed_plan.candidates[0], true));
    }
    let bytes = render_seed(&seed_plan, &inputs.origins, &inputs.body)?;
    std::fs::create_dir_all(&seed_plan.root)
        .with_context(|| format!("create seed root {}", seed_plan.root.display()))?;
    for candidate in &seed_plan.candidates {
        let mut temporary = tempfile::NamedTempFile::new_in(&seed_plan.root)
            .with_context(|| format!("create temporary seed in {}", seed_plan.root.display()))?;
        temporary
            .write_all(bytes.as_bytes())
            .context("write temporary seed")?;
        temporary.flush().context("flush temporary seed")?;
        match temporary.persist_noclobber(&candidate.path) {
            Ok(_) => return Ok(seed_result(&seed_plan, candidate, false)),
            Err(err) if err.error.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err.error)
                    .with_context(|| format!("persist seed {}", candidate.path.display()));
            }
        }
    }
    bail!(
        "could not create a seed for {} after {} attempts",
        seed_plan.period,
        seed_plan.candidates.len()
    )
}

fn seed_paths(mut plan: NewPlan) -> NewPlan {
    for candidate in &mut plan.candidates {
        candidate.path = candidate.path.with_extension("md");
    }
    plan
}

fn seed_result(plan: &NewPlan, candidate: &NewCandidate, dry_run: bool) -> NewResult {
    NewResult {
        title: plan.title.clone(),
        slug: plan.slug.clone(),
        period: plan.period.clone(),
        sid_ref: candidate.sid_ref.clone(),
        id: candidate.id.clone(),
        path: candidate.path.clone(),
        dry_run,
    }
}

fn render_seed(plan: &NewPlan, origins: &[String], body: &str) -> Result<String> {
    let candidate = &plan.candidates[0];
    let mut output = format!(
        "---\ntype: {}\nid: {}\ntitle: {}\ntimestamp: {}\n",
        serde_json::to_string("seed")?,
        serde_json::to_string(&candidate.sid_ref)?,
        serde_json::to_string(&plan.title)?,
        serde_json::to_string(&Local::now().format("%Y-%m-%d").to_string())?,
    );
    if !origins.is_empty() {
        output.push_str(&format!("origin: {}\n", json!(origins)));
    }
    output.push_str("---\n");
    if !origins.is_empty() {
        output.push_str("\n## Related Work\n\n");
        for origin in origins {
            output.push_str("- ");
            output.push(char::from(96));
            output.push_str(origin);
            output.push(char::from(96));
            output.push_str(" — Origin: This seed was captured from ");
            output.push(char::from(96));
            output.push_str(origin);
            output.push(char::from(96));
            output.push_str(".\n");
        }
    }
    if !body.is_empty() {
        if !origins.is_empty() {
            output.push('\n');
        }
        output.push_str(body);
    }
    Ok(output)
}
