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
    cmd_seed_for_period(inputs, cwd, current_period())
}

fn cmd_seed_for_period(inputs: SeedInputs, cwd: &Path, period: String) -> Result<NewResult> {
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
        period,
        &snapshot,
        &config.ref_prefix_policy,
    )?;
    let seed_plan = seed_paths(task_plan);
    if inputs.dry_run {
        return Ok(seed_result(&seed_plan, &seed_plan.candidates[0], true));
    }
    let timestamp = Local::now().format("%Y-%m-%d").to_string();
    persist_seed(&seed_plan, &inputs.origins, &inputs.body, &timestamp)
}

fn persist_seed(
    plan: &NewPlan,
    origins: &[String],
    body: &str,
    timestamp: &str,
) -> Result<NewResult> {
    std::fs::create_dir_all(&plan.root)
        .with_context(|| format!("create seed root {}", plan.root.display()))?;
    for candidate in &plan.candidates {
        let bytes = render_seed(plan, candidate, origins, body, timestamp)?;
        let mut temporary = tempfile::NamedTempFile::new_in(&plan.root)
            .with_context(|| format!("create temporary seed in {}", plan.root.display()))?;
        temporary
            .write_all(bytes.as_bytes())
            .context("write temporary seed")?;
        temporary.flush().context("flush temporary seed")?;
        match temporary.persist_noclobber(&candidate.path) {
            Ok(_) => return Ok(seed_result(plan, candidate, false)),
            Err(err) if err.error.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err.error)
                    .with_context(|| format!("persist seed {}", candidate.path.display()));
            }
        }
    }
    bail!(
        "could not create a seed for {} after {} attempts",
        plan.period,
        plan.candidates.len()
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

fn render_seed(
    plan: &NewPlan,
    candidate: &NewCandidate,
    origins: &[String],
    body: &str,
    timestamp: &str,
) -> Result<String> {
    let mut output = format!(
        "---\ntype: {}\nid: {}\ntitle: {}\ntimestamp: {}\n",
        serde_json::to_string("seed")?,
        serde_json::to_string(&candidate.sid_ref)?,
        serde_json::to_string(&plan.title)?,
        serde_json::to_string(timestamp)?,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_collision_retry_preserves_the_successful_candidates_identity() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("seeds");
        std::fs::create_dir_all(&root).unwrap();
        let first = NewCandidate {
            sid_ref: "sa2a2".to_string(),
            id: "202605_sa2a2_retry-identity".to_string(),
            path: root.join("202605_sa2a2_retry-identity.md"),
        };
        let second = NewCandidate {
            sid_ref: "sa2a3".to_string(),
            id: "202605_sa2a3_retry-identity".to_string(),
            path: root.join("202605_sa2a3_retry-identity.md"),
        };
        std::fs::write(
            &first.path,
            "---\ntype: \"seed\"\nid: \"sa2a2\"\ntitle: \"occupied\"\ntimestamp: \"2026-07-21\"\n---\n",
        )
        .unwrap();
        let plan = NewPlan {
            title: "retry identity".to_string(),
            slug: "retry-identity".to_string(),
            period: "202605".to_string(),
            seq: 0,
            root: root.clone(),
            candidates: vec![first, second.clone()],
        };

        let result = persist_seed(&plan, &[], "body\n", "2026-07-21").unwrap();

        assert_eq!(result.sid_ref, second.sid_ref);
        assert_eq!(result.id, second.id);
        assert_eq!(result.path, second.path);
        let persisted = std::fs::read_to_string(&second.path).unwrap();
        assert!(persisted.contains("\nid: \"sa2a3\"\n"), "{persisted}");
        let index = documents::scan_sources(&documents::DocumentSources {
            task_roots: Vec::new(),
            seed_root: root,
            topic_roots: Vec::new(),
        });
        assert!(index.nodes.contains_key("sa2a3"));
        assert!(index.complete(), "{:?}", index.findings);
    }

    #[test]
    fn seed_uses_configured_ref_deny_prefixes_at_injected_period() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(".sid"),
            "[ref]\ndeny_prefixes = [\"sey\"]\n",
        )
        .unwrap();
        let root = tmp.path().join("stm");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir(root.join("202605_sex2a_existing")).unwrap();

        let result = cmd_seed_for_period(
            SeedInputs {
                title: "custom seed policy".to_string(),
                origins: Vec::new(),
                dry_run: true,
                body: String::new(),
            },
            tmp.path(),
            "202605".to_string(),
        )
        .unwrap();

        assert_eq!(result.period, "202605");
        assert!(result.sid_ref.starts_with("sez"), "{}", result.sid_ref);
    }
}
