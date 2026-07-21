use anyhow::{Context, Result, bail};
use chrono::Local;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::commands::{ProjectConfig, load_project_config};
use crate::documents::{self, CanonicalSourceKind};
use crate::refcode::{self, RefTail};
use crate::scan::{self, is_valid_period};
use crate::slug::slugify;

const CANDIDATE_BUDGET: usize = 100;
const TAIL_SPACE_SIZE: usize = refcode::SLOP30_CHARS.len() * refcode::SLOP30_CHARS.len();

#[derive(Debug)]
pub struct NewInputs {
    pub title: Option<String>,
    pub from_seed: Option<String>,
    pub dry_run: bool,
    pub period: Option<String>,
    pub into: Option<String>,
}

#[derive(Debug)]
pub struct ScanSnapshot {
    occupied: HashMap<String, HashSet<String>>,
    max_seq: HashMap<String, u16>,
}

impl ScanSnapshot {
    pub fn empty() -> Self {
        Self {
            occupied: HashMap::new(),
            max_seq: HashMap::new(),
        }
    }

    fn record(&mut self, period: String, sid_ref: String, seq: u16) {
        self.occupied
            .entry(period.clone())
            .or_default()
            .insert(sid_ref);
        self.max_seq
            .entry(period)
            .and_modify(|max| *max = (*max).max(seq))
            .or_insert(seq);
    }

    fn is_occupied(&self, period: &str, sid_ref: &str) -> bool {
        self.occupied
            .get(period)
            .is_some_and(|refs| refs.contains(sid_ref))
    }

    fn next_seq(&self, period: &str) -> Result<u16> {
        if !is_valid_period(period) {
            return Err(anyhow::anyhow!("invalid period: {period}"));
        }

        match self.max_seq.get(period) {
            Some(max) => max
                .checked_add(1)
                .filter(|seq| *seq <= refcode::MAX_SEQ)
                .ok_or_else(|| anyhow::anyhow!("monthly sequence exhausted for {period}")),
            None => refcode::deterministic_seq_start(period)
                .ok_or_else(|| anyhow::anyhow!("invalid period: {period}")),
        }
    }
}

#[derive(Debug)]
pub struct NewPlan {
    pub title: String,
    pub slug: String,
    pub period: String,
    pub seq: u16,
    pub root: PathBuf,
    pub candidates: Vec<NewCandidate>,
}

#[derive(Debug, Clone)]
pub struct NewCandidate {
    pub sid_ref: String,
    pub id: String,
    pub path: PathBuf,
}

#[derive(Debug, Serialize)]
pub struct NewResult {
    pub title: String,
    pub slug: String,
    pub period: String,
    pub sid_ref: String,
    pub id: String,
    pub path: PathBuf,
    pub dry_run: bool,
}

pub fn cmd_new(inputs: NewInputs, cwd: &Path) -> Result<NewResult> {
    match (inputs.title, inputs.from_seed) {
        (Some(title), None) => {
            cmd_new_title(title, inputs.dry_run, inputs.period, inputs.into, cwd)
        }
        (None, Some(seed_id)) => {
            if inputs.period.is_some() {
                bail!("--period cannot be used with --from-seed");
            }
            graduate_seed(&seed_id, inputs.dry_run, inputs.into.as_deref(), cwd)
        }
        _ => bail!("new requires exactly one title or --from-seed ID"),
    }
}

fn cmd_new_title(
    title: String,
    dry_run: bool,
    period: Option<String>,
    into: Option<String>,
    cwd: &Path,
) -> Result<NewResult> {
    let config = load_project_config(cwd)?;
    let root = match &into {
        None => config.active_root.clone(),
        Some(into) => resolve_destination_root(into, &config)?,
    };
    let period = period.unwrap_or_else(current_period);
    let snapshot = scan_roots(&config.allocation_roots())?;
    let tails = randomized_tail_space();
    let plan = plan_new(
        title,
        root,
        period,
        &snapshot,
        &config.ref_prefix_policy,
        tails,
    )?;

    if dry_run {
        return Ok(plan_to_result(&plan, true));
    }

    execute_new(&plan)
}

fn graduate_seed(
    seed_id: &str,
    dry_run: bool,
    into: Option<&str>,
    cwd: &Path,
) -> Result<NewResult> {
    let config = load_project_config(cwd)?;
    let index = documents::scan_sources(&config.document_sources());
    let Some(record) = index.nodes.get(seed_id) else {
        bail!("no unique valid seed has canonical id {seed_id}");
    };
    if record.source_kind != CanonicalSourceKind::Seed {
        bail!("canonical id {seed_id} is not a seed");
    }
    let stem = record
        .node
        .path
        .file_stem()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow::anyhow!("seed filename is not valid UTF-8"))?;
    let mut parts = stem.splitn(3, '_');
    let period = parts.next().unwrap_or_default();
    let filename_ref = parts.next().unwrap_or_default();
    let slug = parts.next().unwrap_or_default();
    if filename_ref != seed_id || slug.is_empty() || !is_valid_period(period) {
        bail!("seed filename does not preserve period/ref/slug identity");
    }
    let root = match into {
        None => config.active_root.clone(),
        Some(into) => resolve_destination_root(into, &config)?,
    };
    let destination = root.join(stem);
    if destination.exists() || destination.join("napkin.md").exists() {
        bail!(
            "seed graduation destination already exists: {}",
            destination.display()
        );
    }
    let result = NewResult {
        title: record.node.frontmatter["title"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        slug: slug.to_string(),
        period: period.to_string(),
        sid_ref: seed_id.to_string(),
        id: stem.to_string(),
        path: destination.clone(),
        dry_run,
    };
    if dry_run {
        return Ok(result);
    }
    fs::create_dir_all(&root).with_context(|| format!("create task root {}", root.display()))?;
    execute_graduation(&record.node.path, &destination, |from, to| {
        fs::rename(from, to)
    })?;
    Ok(result)
}

fn execute_graduation<F>(source: &Path, destination: &Path, rename: F) -> Result<()>
where
    F: FnOnce(&Path, &Path) -> std::io::Result<()>,
{
    let napkin = destination.join("napkin.md");
    fs::create_dir(destination).with_context(|| {
        format!(
            "create seed graduation destination {}",
            destination.display()
        )
    })?;
    if let Err(err) = rename(source, &napkin) {
        let cleanup_note = match fs::remove_dir(destination) {
            Ok(()) => String::new(),
            Err(cleanup_err) => {
                format!("; newly created destination was not removed: {cleanup_err}")
            }
        };
        bail!(
            "move seed {} to {}: {err}{cleanup_note}",
            source.display(),
            napkin.display()
        );
    }
    Ok(())
}

pub(crate) fn current_period() -> String {
    Local::now().format("%Y%m").to_string()
}

/// Resolve `--into` against the configured roots (active root included) by
/// component-wise suffix match, so `.slow`, `./.slow`, `stm/.slow`, and the
/// canonicalized absolute path all name the same root. Anything that does
/// not match exactly one configured root fails closed.
pub(crate) fn resolve_destination_root(into: &str, config: &ProjectConfig) -> Result<PathBuf> {
    let needle = Path::new(into);
    // A leading `./` (shell tab completion) is noise; `Path::ends_with`
    // already normalizes interior `.` components but preserves a leading one.
    let needle = needle.strip_prefix(".").unwrap_or(needle);
    let matches: Vec<&PathBuf> = if needle.as_os_str().is_empty() {
        // `Path::ends_with("")` is true for every path; an empty needle
        // ("", ".", "./") must fail closed instead of matching all roots.
        Vec::new()
    } else {
        config
            .scan_roots
            .iter()
            .filter(|root| root.ends_with(needle))
            .collect()
    };

    match matches.as_slice() {
        [root] => Ok((*root).clone()),
        [] => bail!(
            "--into is not a configured task root: \"{into}\" (configured roots: {})",
            display_roots(config)
        ),
        _ => bail!(
            "--into is ambiguous: \"{into}\" matches {}",
            matches
                .iter()
                .map(|root| display_root(root, config))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn display_root(root: &Path, config: &ProjectConfig) -> String {
    root.strip_prefix(&config.base)
        .unwrap_or(root)
        .display()
        .to_string()
}

fn display_roots(config: &ProjectConfig) -> String {
    config
        .scan_roots
        .iter()
        .map(|root| display_root(root, config))
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn scan_roots(roots: &[PathBuf]) -> Result<ScanSnapshot> {
    let mut snapshot = ScanSnapshot::empty();

    for root in roots {
        scan_root_into(&mut snapshot, root)?;
    }

    Ok(snapshot)
}

#[cfg(test)]
fn scan_root(root: &Path) -> Result<ScanSnapshot> {
    let mut snapshot = ScanSnapshot::empty();
    scan_root_into(&mut snapshot, root)?;
    Ok(snapshot)
}

fn scan_root_into(snapshot: &mut ScanSnapshot, root: &Path) -> Result<()> {
    for child in scan::scan_period_prefixed_children(root)? {
        // Files, symlinks, and other non-directories reserve best-effort;
        // unexpected entry shapes never fail allocation.
        let Some(seq) = refcode::decode_seq(&child.ref_segment) else {
            continue;
        };
        snapshot.record(child.period, child.ref_segment, seq);
    }

    Ok(())
}

pub(crate) fn plan_new<I>(
    title: String,
    root: PathBuf,
    period: String,
    snapshot: &ScanSnapshot,
    ref_prefix_policy: &refcode::RefPrefixPolicy,
    tails: I,
) -> Result<NewPlan>
where
    I: IntoIterator<Item = RefTail>,
{
    let slug = slugify(&title);
    if slug.is_empty() {
        bail!("task title must contain at least one ASCII letter or digit");
    }

    let mut seq = snapshot.next_seq(&period)?;
    while !ref_prefix_policy.allows_any_in_sequence(seq) {
        seq = seq
            .checked_add(1)
            .filter(|seq| *seq <= refcode::MAX_SEQ)
            .ok_or_else(|| anyhow::anyhow!("monthly sequence exhausted for {period}"))?;
    }
    let mut candidates = Vec::new();

    for tail in tails.into_iter().take(TAIL_SPACE_SIZE) {
        let Some(sid_ref) = refcode::encode_ref(seq, tail) else {
            continue;
        };
        if ref_prefix_policy.denies(&sid_ref) {
            continue;
        }
        if snapshot.is_occupied(&period, &sid_ref) {
            continue;
        }

        let id = format!("{period}_{sid_ref}_{slug}");
        candidates.push(NewCandidate {
            sid_ref,
            path: root.join(&id),
            id,
        });
        if candidates.len() == CANDIDATE_BUDGET {
            break;
        }
    }

    if candidates.is_empty() {
        bail!("could not allocate an unoccupied ref for {period} from the candidate tail space");
    }

    Ok(NewPlan {
        title,
        slug,
        period,
        seq,
        root,
        candidates,
    })
}

pub(crate) fn plan_random_new(
    title: String,
    root: PathBuf,
    period: String,
    snapshot: &ScanSnapshot,
    ref_prefix_policy: &refcode::RefPrefixPolicy,
) -> Result<NewPlan> {
    plan_new(
        title,
        root,
        period,
        snapshot,
        ref_prefix_policy,
        randomized_tail_space(),
    )
}

pub fn execute_new(plan: &NewPlan) -> Result<NewResult> {
    fs::create_dir_all(&plan.root)
        .with_context(|| format!("create task root {}", plan.root.display()))?;

    // v1 accepts a same-ref/different-slug race; exact path collisions retry.
    for candidate in &plan.candidates {
        match fs::create_dir(&candidate.path) {
            Ok(()) => return Ok(candidate_to_result(plan, candidate, false)),
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("create task folder {}", candidate.path.display()));
            }
        }
    }

    bail!(
        "could not create a task folder for {} after {} attempts",
        plan.period,
        plan.candidates.len()
    );
}

pub(crate) fn plan_to_result(plan: &NewPlan, dry_run: bool) -> NewResult {
    candidate_to_result(plan, &plan.candidates[0], dry_run)
}

fn candidate_to_result(plan: &NewPlan, candidate: &NewCandidate, dry_run: bool) -> NewResult {
    debug_assert_eq!(refcode::decode_seq(&candidate.sid_ref), Some(plan.seq));

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

fn randomized_tail_space() -> Vec<RefTail> {
    let mut tails = Vec::with_capacity(TAIL_SPACE_SIZE);
    for hi in refcode::SLOP30_CHARS {
        for lo in refcode::SLOP30_CHARS {
            tails.push(RefTail { hi, lo });
        }
    }
    fastrand::shuffle(&mut tails);
    tails
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tail(hi: char, lo: char) -> RefTail {
        RefTail { hi, lo }
    }

    fn prude_policy() -> refcode::RefPrefixPolicy {
        refcode::RefPrefixPolicy::prude()
    }

    fn planner_snapshot_with_occupied_ref_only(period: &str, sid_ref: &str) -> ScanSnapshot {
        // This fixture intentionally omits max_seq so the planner must handle an
        // occupied ref in the same sequence it is currently trying to allocate.
        let mut refs = HashSet::new();
        refs.insert(sid_ref.to_string());

        let mut occupied = HashMap::new();
        occupied.insert(period.to_string(), refs);

        ScanSnapshot {
            occupied,
            max_seq: HashMap::new(),
        }
    }

    #[test]
    fn graduation_rename_failure_rolls_back_only_the_new_empty_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("seed.md");
        let destination = tmp.path().join("task");
        fs::write(&source, "seed bytes").unwrap();
        let error = execute_graduation(&source, &destination, |_, _| {
            Err(std::io::Error::other("injected rename failure"))
        })
        .unwrap_err();
        assert!(error.to_string().contains("injected rename failure"));
        assert!(source.exists());
        assert!(!destination.exists());
    }

    #[test]
    fn plan_empty_month_uses_deterministic_start() {
        let snapshot = ScanSnapshot::empty();

        let plan = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert_eq!(
            plan.seq,
            refcode::deterministic_seq_start("202605").unwrap()
        );
        assert_eq!(plan.candidates[0].id, "202605_seaa2_fix-auth-state");
    }

    #[test]
    fn plan_uses_next_sequence_after_scanned_siblings() {
        let mut snapshot = ScanSnapshot::empty();
        snapshot.record("202605".to_string(), "sa2a2".to_string(), 0);
        snapshot.record("202605".to_string(), "sa3a2".to_string(), 1);

        let plan = plan_new(
            "review login".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert_eq!(plan.seq, 2);
        assert_eq!(plan.candidates[0].id, "202605_sa4a2_review-login");
    }

    #[test]
    fn plan_allocates_before_and_skips_over_prude_blocked_sequence() {
        let mut before_snapshot = ScanSnapshot::empty();
        before_snapshot.record("202605".to_string(), "sev2a".to_string(), 145);

        let before_plan = plan_new(
            "before reserved".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &before_snapshot,
            &prude_policy(),
            [tail('2', 'a')],
        )
        .unwrap();

        assert_eq!(before_plan.seq, 146);
        assert_eq!(before_plan.candidates[0].sid_ref, "sew2a");

        let mut skip_snapshot = ScanSnapshot::empty();
        skip_snapshot.record("202605".to_string(), "sew2a".to_string(), 146);

        let skip_plan = plan_new(
            "after reserved".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &skip_snapshot,
            &prude_policy(),
            [tail('2', 'a')],
        )
        .unwrap();

        assert_eq!(skip_plan.seq, 148);
        assert_eq!(skip_plan.candidates[0].sid_ref, "sey2a");
    }

    #[test]
    fn plan_filters_every_prude_four_character_prefix() {
        let period = "202605";
        let policy = prude_policy();

        for reserved in [
            "shat", "smut", "spaz", "stfu", "scat", "scum", "shag", "suck",
        ] {
            let reserved_ref = format!("{reserved}2");
            assert!(refcode::is_generated_ref(&reserved_ref), "{reserved_ref}");
            let seq = refcode::decode_seq(&reserved_ref).unwrap();
            let mut max_seq = HashMap::new();
            max_seq.insert(period.to_string(), seq - 1);
            let snapshot = ScanSnapshot {
                occupied: HashMap::new(),
                max_seq,
            };
            let reserved_tail_hi = reserved.chars().nth(3).unwrap();

            let plan = plan_new(
                format!("avoid {reserved}"),
                PathBuf::from("/tmp/stm"),
                period.to_string(),
                &snapshot,
                &policy,
                [tail(reserved_tail_hi, '2'), tail('v', '2')],
            )
            .unwrap();

            assert_eq!(plan.candidates.len(), 1, "{reserved}");
            assert!(
                !policy.denies(&plan.candidates[0].sid_ref),
                "{reserved}: {}",
                plan.candidates[0].sid_ref
            );
        }
    }

    #[test]
    fn denied_raw_tails_do_not_consume_the_viable_candidate_budget() {
        let denied_prefixes = refcode::SLOP30_CHARS
            .into_iter()
            .filter(|tail_hi| *tail_hi != 'z')
            .map(|tail_hi| format!("sea{tail_hi}"))
            .collect();
        let policy = refcode::RefPrefixPolicy::try_from_prefixes(denied_prefixes).unwrap();
        let tails = refcode::SLOP30_CHARS.into_iter().flat_map(|hi| {
            refcode::SLOP30_CHARS
                .into_iter()
                .map(move |lo| tail(hi, lo))
        });

        let plan = plan_new(
            "sparse custom policy".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &ScanSnapshot::empty(),
            &policy,
            tails,
        )
        .unwrap();

        assert_eq!(plan.candidates.len(), 8);
        assert!(
            plan.candidates
                .iter()
                .all(|candidate| candidate.sid_ref.starts_with("seaz"))
        );
    }

    #[test]
    fn randomized_tail_space_is_complete_and_without_replacement() {
        let tails = randomized_tail_space();
        let unique = tails
            .iter()
            .map(|tail| (tail.hi, tail.lo))
            .collect::<HashSet<_>>();

        assert_eq!(tails.len(), TAIL_SPACE_SIZE);
        assert_eq!(unique.len(), tails.len());
    }

    #[test]
    fn planner_retains_at_most_the_unique_candidate_budget() {
        let policy = refcode::RefPrefixPolicy::try_from_prefixes(Vec::new()).unwrap();
        let plan = plan_new(
            "candidate budget".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &ScanSnapshot::empty(),
            &policy,
            randomized_tail_space(),
        )
        .unwrap();
        let unique = plan
            .candidates
            .iter()
            .map(|candidate| candidate.sid_ref.as_str())
            .collect::<HashSet<_>>();

        assert_eq!(plan.candidates.len(), CANDIDATE_BUDGET);
        assert_eq!(unique.len(), plan.candidates.len());
    }

    #[test]
    fn scan_root_records_slugged_and_slugless_direct_child_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202605_sa2a2_fix-auth")).unwrap();
        // Slugless task-shaped directories are accepted as conservative
        // namespace reservations, even though generated ids include a slug.
        fs::create_dir(root.join("202605_sa3a2")).unwrap();
        fs::write(root.join("notes_202605_sa4a2"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert!(snapshot.is_occupied("202605", "sa3a2"));
        assert!(!snapshot.is_occupied("202605", "sa4a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 2);
    }

    #[test]
    fn scan_root_treats_missing_root_as_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");

        let snapshot = scan_root(&root).unwrap();

        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_ignores_non_task_shaped_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("README.md"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(!snapshot.is_occupied("202605", "sa2a2"));
        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_ignores_period_prefixed_non_recognized_ref_files() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("202605_notaref_x"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(!snapshot.is_occupied("202605", "notaref"));
        assert_eq!(
            snapshot.next_seq("202605").unwrap(),
            refcode::deterministic_seq_start("202605").unwrap()
        );
    }

    #[test]
    fn scan_root_reserves_namespace_for_task_shaped_files() {
        // Zipped task-folder exports legitimately sit next to their source
        // folders; unexpected entry shapes reserve best-effort instead of
        // failing allocation.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::write(root.join("202605_sa2a2_fix-auth.zip"), "").unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn scan_root_reserves_namespace_for_task_shaped_symlinks() {
        use std::os::unix::fs::symlink;

        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("target")).unwrap();
        symlink(root.join("target"), root.join("202605_sa2a2_link")).unwrap();
        symlink(root.join("missing"), root.join("202605_sa3a2_dangling")).unwrap();

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa2a2"));
        assert!(snapshot.is_occupied("202605", "sa3a2"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 2);
    }

    #[test]
    fn scan_root_records_recognized_refs_even_when_not_generated() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202605_sa222_legacy")).unwrap();

        assert!(refcode::is_recognized_ref("sa222"));
        assert!(!refcode::is_generated_ref("sa222"));

        let snapshot = scan_root(&root).unwrap();

        assert!(snapshot.is_occupied("202605", "sa222"));
        assert_eq!(snapshot.next_seq("202605").unwrap(), 1);
    }

    #[test]
    fn scan_root_partitions_refs_and_max_sequence_by_period() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join("202604_szza2_exhausted-other-month")).unwrap();
        fs::create_dir(root.join("202604_seaa2_same-ref-other-month")).unwrap();

        let snapshot = scan_root(&root).unwrap();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert_eq!(
            plan.seq,
            refcode::deterministic_seq_start("202605").unwrap()
        );
        assert_eq!(plan.candidates[0].sid_ref, "seaa2");
    }

    #[test]
    fn plan_errors_when_monthly_sequence_is_exhausted() {
        let mut snapshot = ScanSnapshot::empty();
        snapshot.record("202605".to_string(), "szza2".to_string(), refcode::MAX_SEQ);

        let err = plan_new(
            "next task".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap_err();

        assert!(err.to_string().contains("monthly sequence exhausted"));
    }

    #[test]
    fn plan_errors_when_policy_blocks_the_final_monthly_sequence() {
        let mut snapshot = ScanSnapshot::empty();
        snapshot.record(
            "202605".to_string(),
            "szy2a".to_string(),
            refcode::MAX_SEQ - 1,
        );
        let policy = refcode::RefPrefixPolicy::try_from_prefixes(vec!["szz".to_string()]).unwrap();

        let err = plan_new(
            "blocked final slot".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &policy,
            [tail('2', 'a')],
        )
        .unwrap_err();

        assert!(err.to_string().contains("monthly sequence exhausted"));
    }

    #[test]
    fn plan_allocates_final_monthly_sequence_before_exhaustion() {
        let mut snapshot = ScanSnapshot::empty();
        snapshot.record(
            "202605".to_string(),
            "szy2a".to_string(),
            refcode::MAX_SEQ - 1,
        );

        let plan = plan_new(
            "final slot".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a')],
        )
        .unwrap();

        assert_eq!(plan.seq, refcode::MAX_SEQ);
        assert_eq!(refcode::decode_seq(&plan.candidates[0].sid_ref), Some(659));
        assert_eq!(plan.candidates[0].sid_ref, "szz2a");
    }

    #[test]
    fn plan_skips_invalid_drawn_tails() {
        let snapshot = ScanSnapshot::empty();
        let period = "202605".to_string();

        let plan = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            period,
            &snapshot,
            &prude_policy(),
            [tail('a', 'b'), tail('2', 'a')],
        )
        .unwrap();

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(
            plan.candidates[0].sid_ref,
            refcode::encode_ref(plan.seq, tail('2', 'a')).unwrap()
        );
    }

    #[test]
    fn plan_skips_tails_with_chars_outside_slop30() {
        let snapshot = ScanSnapshot::empty();

        let plan = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('1', 'a'), tail('a', '2')],
        )
        .unwrap();

        assert_eq!(plan.candidates.len(), 1);
        assert_eq!(plan.candidates[0].sid_ref, "seaa2");
    }

    #[test]
    fn plan_errors_when_all_drawn_tails_are_invalid() {
        let snapshot = ScanSnapshot::empty();

        let err = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', 'b'), tail('c', 'd')],
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("could not allocate an unoccupied ref")
        );
    }

    #[test]
    fn plan_occupied_ref_blocks_slug_disambiguation() {
        let period = "202605";
        let seq = refcode::deterministic_seq_start(period).unwrap();
        let occupied_ref = refcode::encode_ref(seq, tail('2', 'a')).unwrap();
        let next_ref = refcode::encode_ref(seq, tail('3', 'a')).unwrap();
        let snapshot = planner_snapshot_with_occupied_ref_only(period, &occupied_ref);

        let plan = plan_new(
            "new slug for same ref".to_string(),
            PathBuf::from("/tmp/stm"),
            period.to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        assert_eq!(plan.candidates[0].sid_ref, next_ref);
        assert!(
            !plan
                .candidates
                .iter()
                .any(|candidate| candidate.sid_ref == occupied_ref)
        );
    }

    #[test]
    fn plan_errors_when_all_candidate_refs_are_occupied() {
        let period = "202605";
        let seq = refcode::deterministic_seq_start(period).unwrap();
        let occupied_ref = refcode::encode_ref(seq, tail('2', 'a')).unwrap();
        let snapshot = planner_snapshot_with_occupied_ref_only(period, &occupied_ref);

        let err = plan_new(
            "new slug for same ref".to_string(),
            PathBuf::from("/tmp/stm"),
            period.to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a')],
        )
        .unwrap_err();

        assert!(
            err.to_string()
                .contains("could not allocate an unoccupied ref")
        );
    }

    #[test]
    fn plan_rejects_injected_malformed_period_even_with_scanned_max() {
        // This impossible scanner state guards the planner's dependency-injected
        // period boundary: callers may not make malformed periods valid by
        // pairing them with matching snapshot data.
        let mut max_seq = HashMap::new();
        max_seq.insert("2026_05".to_string(), 0);
        let snapshot = ScanSnapshot {
            occupied: HashMap::new(),
            max_seq,
        };

        let err = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            "2026_05".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid period"));
    }

    #[test]
    fn plan_rejects_injected_malformed_period_with_empty_snapshot() {
        let snapshot = ScanSnapshot::empty();

        let err = plan_new(
            "fix auth state".to_string(),
            PathBuf::from("/tmp/stm"),
            "2026_05".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap_err();

        assert!(err.to_string().contains("invalid period"));
    }

    #[test]
    fn plan_allows_ref_text_inside_title_slug() {
        let snapshot = ScanSnapshot::empty();

        let plan = plan_new(
            "follow up on sa2a2 regression".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a')],
        )
        .unwrap();

        assert!(
            plan.candidates[0]
                .id
                .ends_with("_follow-up-on-sa2a2-regression")
        );
    }

    #[test]
    fn execute_new_retries_next_candidate_when_a_file_occupies_the_path() {
        // A racing zip export (or any file) at the exact candidate path must
        // hit the AlreadyExists retry, not a hard error.
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        fs::create_dir_all(&plan.root).unwrap();
        fs::write(&plan.candidates[0].path, "zip bytes").unwrap();

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, plan.candidates[1].id);
        assert!(plan.candidates[1].path.is_dir());
    }

    #[test]
    fn execute_new_retries_next_candidate_on_exact_path_already_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        let second_id = plan.candidates[1].id.clone();
        let second_path = plan.candidates[1].path.clone();
        fs::create_dir_all(&plan.candidates[0].path).unwrap();

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, second_id);
        assert!(second_path.is_dir());
    }

    #[test]
    fn execute_new_creates_missing_task_root_and_chosen_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root.clone(),
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('a', '2')],
        )
        .unwrap();

        assert!(!root.exists());

        let result = execute_new(&plan).unwrap();

        assert_eq!(result.id, plan.candidates[0].id);
        assert!(root.is_dir());
        assert!(result.path.is_dir());
    }

    #[test]
    fn execute_new_errors_when_all_candidate_paths_already_exist() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("stm");
        let snapshot = ScanSnapshot::empty();
        let plan = plan_new(
            "fix auth state".to_string(),
            root,
            "202605".to_string(),
            &snapshot,
            &prude_policy(),
            [tail('2', 'a'), tail('3', 'a')],
        )
        .unwrap();

        for candidate in &plan.candidates {
            fs::create_dir_all(&candidate.path).unwrap();
        }

        let err = execute_new(&plan).unwrap_err();

        assert!(err.to_string().contains("could not create a task folder"));
    }
}
