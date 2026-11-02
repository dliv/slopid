use crate::names::is_valid_period;
use crate::refcode::{self, RefTail};
use crate::slug::slugify;
use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const CANDIDATE_BUDGET: usize = 100;
const TAIL_SPACE_SIZE: usize = refcode::SLOP30_CHARS.len() * refcode::SLOP30_CHARS.len();

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

    pub fn is_occupied(&self, period: &str, sid_ref: &str) -> bool {
        self.occupied
            .get(period)
            .is_some_and(|refs| refs.contains(sid_ref))
    }

    pub fn next_seq(&self, period: &str) -> Result<u16> {
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
    title: String,
    slug: String,
    period: String,
    seq: u16,
    root: PathBuf,
    candidates: Vec<NewCandidate>,
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

impl NewPlan {
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn period(&self) -> &str {
        &self.period
    }
    pub fn candidates(&self) -> &[NewCandidate] {
        &self.candidates
    }
}
impl ScanSnapshot {
    /// Record a name-only observation using the same recognized-ref decoder as allocation.
    pub fn observe(&mut self, period: String, sid_ref: String) {
        if let Some(seq) = refcode::decode_seq(&sid_ref) {
            self.record(period, sid_ref, seq);
        }
    }
}
pub fn plan_new<I>(
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

pub fn plan_to_result(plan: &NewPlan, dry_run: bool) -> NewResult {
    candidate_to_result(plan, &plan.candidates[0], dry_run)
}

pub fn candidate_to_result(plan: &NewPlan, candidate: &NewCandidate, dry_run: bool) -> NewResult {
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
    fn planner_retains_at_most_the_unique_candidate_budget() {
        let policy = refcode::RefPrefixPolicy::try_from_prefixes(Vec::new()).unwrap();
        let plan = plan_new(
            "candidate budget".to_string(),
            PathBuf::from("/tmp/stm"),
            "202605".to_string(),
            &ScanSnapshot::empty(),
            &policy,
            refcode::SLOP30_CHARS.into_iter().flat_map(|hi| {
                refcode::SLOP30_CHARS
                    .into_iter()
                    .map(move |lo| RefTail { hi, lo })
            }),
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
}
