//! Move-projected relinking: repair only the Markdown destinations that one
//! exact planned owner move would change, under a whole-plan digest the caller
//! must present back to write.
//!
//! Global relink answers "which destinations are wrong now?". This module
//! answers "which destinations would this one move make wrong?", which is a
//! strictly narrower authority: a lifecycle coordinator can approve a scoped
//! plan and apply it without also accepting a corpus-wide rewrite.

use anyhow::{Context, Result, bail};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, Permissions};
use std::path::{Path, PathBuf};

use super::{
    PlannedFile, RelinkChange, RelinkPlan, RelinkProjection, RelinkResult, Replacement,
    TargetPresence, TargetResolution, apply_plan_with, atomic_replace, canonical_destination_text,
    compare_changes, has_overlapping_spans, normalize, parse_destination_ref,
    parse_local_destination, push_candidate_finding, render_destination_text,
    render_failure_finding, resolve_target_path, scan_relink_sources, source_parent_of,
    splice_preserves_destinations, target_presence, unknown_target_finding,
    unresolved_ref_finding_code, unresolved_ref_message,
};
use crate::commands::ProjectConfig;
use crate::commands::new::resolve_destination_root;
use crate::documents::{self, CanonicalSourceKind, Finding, FindingCode, compare_findings};
use crate::markdown;

/// Wire-stable marker for the hashed approval object. Changing the shape of
/// `MoveRelinkAuthority` must change this string so an old digest can never
/// accidentally match a new plan encoding.
const MOVE_CONTRACT: &str = "sid-relink-move-v2";

/// One authored Markdown file that carries at least one move-scoped
/// destination, hashed over its whole scanned byte sequence.
#[derive(Debug, Serialize)]
struct EffectSourceAuthority {
    path: PathBuf,
    sha256: String,
}

/// Exactly what a projected approval covers. This is the hash preimage, so it
/// deliberately excludes the expected digest, `applied`, the transient
/// `relink-plan-changed` finding, mtimes, and any display prose: an unrelated
/// authored edit outside the effect set must not create close friction, while a
/// new or changed relevant link must invalidate approval.
#[derive(Debug, Serialize)]
struct MoveRelinkAuthority {
    contract: &'static str,
    projection: RelinkProjection,
    complete: bool,
    effect_sources: Vec<EffectSourceAuthority>,
    changes: Vec<RelinkChange>,
    findings: Vec<Finding>,
}

#[derive(Debug)]
struct MoveRelinkPlan {
    authority: MoveRelinkAuthority,
    files: Vec<PlannedFile>,
    usable: bool,
}

/// What one scoped destination contributes. Anything other than `Irrelevant` is
/// inside the move effect set and therefore makes its source file bind the
/// digest, even when the move causes no replacement.
enum ProjectedOutcome {
    /// Outside the effect set: neither the target id nor the source moves.
    Irrelevant,
    /// In the effect set and already correct, or unaffected by the move.
    Unchanged,
    /// In approval scope with a warning, but no replacement or completeness
    /// failure. The source bytes and finding still bind the plan digest.
    Advisory,
    /// In the effect set but not safely planable; a finding was recorded.
    Blocked,
    Planned(Replacement),
}

#[derive(Debug)]
struct GenericProjection {
    current_candidate: PathBuf,
    authored_after: PathBuf,
    projected_candidate: PathBuf,
    current_valid: bool,
    projected_valid: bool,
}

/// The three lexical readings of one generic local destination that decide both
/// relevance and planning. Shared so a destination whose raw span could not be
/// located is scoped by exactly the same boundary as one that was located.
#[derive(Debug)]
struct GenericCandidates {
    future_source_parent: PathBuf,
    current_candidate: PathBuf,
    authored_after: PathBuf,
    projected_candidate: PathBuf,
}

fn generic_candidates(
    source_parent: &Path,
    resolution_path_text: &str,
    projection: &RelinkProjection,
) -> GenericCandidates {
    let future_source_parent = project_path(source_parent, projection);
    let current_candidate = normalize(&source_parent.join(Path::new(resolution_path_text)));
    let authored_after = normalize(&future_source_parent.join(Path::new(resolution_path_text)));
    let projected_candidate = unproject_path(&authored_after, projection);
    GenericCandidates {
        future_source_parent,
        current_candidate,
        authored_after,
        projected_candidate,
    }
}

impl GenericCandidates {
    fn is_relevant(&self, projection: &RelinkProjection, source_moves: bool) -> bool {
        source_moves
            || self.current_candidate.starts_with(&projection.from_owner)
            || self.authored_after.starts_with(&projection.to_owner)
            || self.projected_candidate.starts_with(&projection.from_owner)
    }
}

/// Is this semantic destination inside the move effect set?
///
/// A destination whose raw span could not be proven still has an identity, and
/// that identity must pass the same boundary a located destination would. An
/// unrelated parse failure must neither block this move nor perturb its
/// approval digest; only a relevant one may.
fn projected_destination_is_relevant(
    source: &Path,
    resolved: &str,
    destination_extensions: &[super::RelinkDestinationExtension],
    projection: &RelinkProjection,
    source_moves: bool,
) -> bool {
    if let Some(parsed) = parse_destination_ref(resolved, destination_extensions) {
        return source_moves || parsed.target_id == projection.id;
    }
    let Some(parsed) = parse_local_destination(resolved, destination_extensions) else {
        // Empty, fragment-only, external, and protocol-relative destinations are
        // outside every local authority, located or not.
        return false;
    };
    generic_candidates(
        source_parent_of(source),
        parsed.resolution_path_text,
        projection,
    )
    .is_relevant(projection, source_moves)
}

pub(super) fn cmd_relink_move(
    config: &ProjectConfig,
    move_id: &str,
    into: &str,
    write: bool,
    expected_plan_sha256: Option<&str>,
) -> Result<RelinkResult> {
    let plan = plan_move_relink(config, move_id, into)?;
    if !plan.usable {
        bail!("relink could not obtain any usable authored Markdown source");
    }
    let digest = plan_digest(&plan.authority)?;

    if !write {
        if expected_plan_sha256.is_some() {
            bail!("--expected-plan-sha256 requires --write");
        }
        return Ok(preview_result(plan.authority, digest));
    }

    let Some(expected) = expected_plan_sha256 else {
        bail!("projected relink --write requires --expected-plan-sha256 <SHA256>");
    };

    // Both refusals happen before the first file is opened for replacement, so
    // a stale or unsafe approval can never leave a half-applied corpus.
    if expected != digest {
        return Ok(refused_result(
            plan.authority,
            digest,
            Some(plan_changed_finding()),
        ));
    }
    if !plan.authority.complete {
        return Ok(refused_result(plan.authority, digest, None));
    }

    Ok(apply_move_plan_with(plan, digest, atomic_replace))
}

fn plan_move_relink(config: &ProjectConfig, move_id: &str, into: &str) -> Result<MoveRelinkPlan> {
    let index = documents::scan_sources(&config.document_sources());
    let projection = resolve_projection(config, &index, move_id, into)?;
    let scan = scan_relink_sources(config, &index);
    let mut findings = scan.findings;
    let mut complete = scan.complete;
    let mut effect_sources = Vec::new();
    let mut files = Vec::new();

    for path in scan.paths {
        let scanned = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                // Coverage failures stay authority-bearing: an unreadable
                // source might conceal an inbound link to the moving owner.
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableEntry,
                    format!("cannot read authored Markdown for relink: {err}"),
                    Some(path),
                ));
                continue;
            }
        };
        let text = match std::str::from_utf8(&scanned) {
            Ok(text) => text,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::UnreadableEntry,
                    format!("authored Markdown is not UTF-8: {err}"),
                    Some(path),
                ));
                continue;
            }
        };

        let source_moves = path.starts_with(&projection.from_owner);
        let scan = markdown::destinations(text);
        let mut replacements = Vec::new();
        let mut in_effect_set = false;
        // A destination the parser saw but whose replaceable bytes could not be
        // proven is only *this command's* problem when it is inside the move
        // effect set. Then it must block, bind its source into authority, and
        // never leave an empty change list looking like clean convergence.
        for issue in &scan.issues {
            if !projected_destination_is_relevant(
                &path,
                &issue.resolved,
                &config.relink_destination_extensions,
                &projection,
                source_moves,
            ) {
                continue;
            }
            in_effect_set = true;
            complete = false;
            let mut finding = Finding::error(
                FindingCode::UnreadableEntry,
                format!(
                    "cannot locate the replaceable bytes of a move-scoped destination: {}",
                    issue.message
                ),
                Some(path.clone()),
            );
            // The line is the construct's, because an unlocatable destination has
            // no span of its own to report.
            finding.line = Some(issue.line);
            finding.id =
                parse_destination_ref(&issue.resolved, &config.relink_destination_extensions)
                    .map(|parsed| parsed.target_id);
            findings.push(finding);
        }
        for destination in scan.destinations.iter().cloned() {
            let outcome = resolve_projected_destination(
                &path,
                destination,
                &index,
                &config.relink_destination_extensions,
                &projection,
                source_moves,
                &mut findings,
            );
            match outcome {
                ProjectedOutcome::Irrelevant => {}
                ProjectedOutcome::Unchanged => in_effect_set = true,
                ProjectedOutcome::Advisory => in_effect_set = true,
                ProjectedOutcome::Blocked => {
                    in_effect_set = true;
                    complete = false;
                }
                ProjectedOutcome::Planned(replacement) => {
                    in_effect_set = true;
                    replacements.push(replacement);
                }
            }
        }
        replacements.sort_by_key(|replacement| replacement.span.start);
        if has_overlapping_spans(&replacements) {
            complete = false;
            findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                "overlapping Markdown destination spans cannot be safely planned",
                Some(path.clone()),
            ));
            replacements.clear();
        }
        if !replacements.is_empty()
            && !splice_preserves_destinations(&scanned, &scan, &replacements)
        {
            // The move-scoped replacements are individually proven, but together
            // they would change how this file parses. Refuse the file rather
            // than approve a mutation whose effect nobody verified.
            in_effect_set = true;
            complete = false;
            findings.push(Finding::error(
                FindingCode::UnreadableEntry,
                "planned replacements would change how this file parses, so no repair here is safe",
                Some(path.clone()),
            ));
            replacements.clear();
        }
        if in_effect_set {
            effect_sources.push(EffectSourceAuthority {
                path: path.clone(),
                sha256: hex_sha256(&scanned),
            });
        }
        // Only files with planned work need their scanned bytes retained; the
        // per-file apply loop skips the rest anyway.
        if !replacements.is_empty() {
            files.push(PlannedFile {
                path,
                scanned,
                replacements,
            });
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    effect_sources.sort_by(|left, right| left.path.cmp(&right.path));
    findings.sort_by(compare_findings);
    let mut changes = files
        .iter()
        .flat_map(|file| {
            file.replacements
                .iter()
                .map(|replacement| replacement.change.clone())
        })
        .collect::<Vec<_>>();
    changes.sort_by(compare_changes);

    Ok(MoveRelinkPlan {
        authority: MoveRelinkAuthority {
            contract: MOVE_CONTRACT,
            projection,
            complete,
            effect_sources,
            changes,
            findings,
        },
        files,
        usable: scan.usable,
    })
}

/// What inspecting the projected owner path actually established.
#[derive(Debug, PartialEq, Eq)]
enum ProjectedDestinationState {
    /// Some filesystem entry is already there, of any type.
    Occupied,
    /// Proven absent. Only `NotFound` establishes this.
    Absent,
    /// Neither proven present nor proven absent: a permission failure or any
    /// other I/O error. Treating this as absence let a scoped write rewrite
    /// every inbound link before an impossible rename.
    Unknown(std::io::ErrorKind),
}

fn classify_projected_destination<T>(inspection: &std::io::Result<T>) -> ProjectedDestinationState {
    match inspection {
        Ok(_) => ProjectedDestinationState::Occupied,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => ProjectedDestinationState::Absent,
        Err(err) => ProjectedDestinationState::Unknown(err.kind()),
    }
}

/// Resolve one exact stable id and one configured root into a projected owner.
/// A caller may never supply an arbitrary future filesystem path: the
/// destination is always a configured root plus the owner's unchanged basename.
fn resolve_projection(
    config: &ProjectConfig,
    index: &documents::DocumentIndex,
    move_id: &str,
    into: &str,
) -> Result<RelinkProjection> {
    let root = resolve_destination_root(into, config)?;
    let Some(record) = index.nodes.get(move_id) else {
        bail!("no unique canonical task or review has id {move_id}");
    };
    if record.source_kind != CanonicalSourceKind::TaskOwner {
        bail!("canonical id {move_id} is not a folder-backed task or review owner");
    }
    let from_owner = record.owner_path.clone();
    let Some(basename) = from_owner.file_name() else {
        bail!(
            "task owner path has no folder name: {}",
            from_owner.display()
        );
    };
    if record.node.path != from_owner.join("CURRENT_STATE.md") {
        bail!(
            "task owner entrypoint is not CURRENT_STATE.md: {}",
            record.node.path.display()
        );
    }
    let to_owner = root.join(basename);
    let settled = to_owner == from_owner;
    if !settled {
        // `exists()` follows symlinks, so a dangling symlink at the destination
        // would pass this guard, the scoped write would apply, and the caller's
        // later rename would then fail with the links already pointing at a
        // location the move cannot reach. Ask about the entry itself, as
        // `config.rs` does — and only `NotFound` proves it absent.
        let inspection = to_owner.symlink_metadata();
        match classify_projected_destination(&inspection) {
            ProjectedDestinationState::Occupied => bail!(
                "projected move destination already exists: {}",
                to_owner.display()
            ),
            ProjectedDestinationState::Absent => {}
            ProjectedDestinationState::Unknown(_) => {
                // `Unknown` is only produced from an error, so this cannot panic.
                let err = inspection
                    .expect_err("an unknown destination state implies an inspection error");
                return Err(err).with_context(|| {
                    format!(
                        "cannot determine whether projected move destination {} exists",
                        to_owner.display()
                    )
                });
            }
        }
    }
    Ok(RelinkProjection {
        id: move_id.to_string(),
        from_owner,
        to_owner,
        settled,
    })
}

#[allow(clippy::too_many_arguments)]
fn resolve_projected_destination(
    source: &Path,
    destination: markdown::MarkdownDestination,
    index: &documents::DocumentIndex,
    destination_extensions: &[super::RelinkDestinationExtension],
    projection: &RelinkProjection,
    source_moves: bool,
    findings: &mut Vec<Finding>,
) -> ProjectedOutcome {
    if parse_destination_ref(&destination.resolved, destination_extensions).is_none() {
        return resolve_generic_projected_destination(
            source,
            destination,
            destination_extensions,
            projection,
            source_moves,
            findings,
        );
    }
    let resolved = destination.resolved;
    let Some(parsed) = parse_destination_ref(&resolved, destination_extensions) else {
        return ProjectedOutcome::Irrelevant;
    };
    // The effect set is not the global repair set: a destination matters when
    // the moving owner is its target, or when its own source directory moves.
    if !source_moves && parsed.target_id != projection.id {
        return ProjectedOutcome::Irrelevant;
    }
    let target_id = parsed.target_id;

    let Some(record) = index.nodes.get(&target_id) else {
        let code = unresolved_ref_finding_code(index, &target_id);
        push_candidate_finding(
            findings,
            code,
            unresolved_ref_message(code, &target_id),
            source,
            &target_id,
            destination.line,
        );
        return ProjectedOutcome::Blocked;
    };
    let target = match resolve_target_path(record, &parsed.suffix, parsed.seed_destination) {
        TargetResolution::Resolved(target) => target,
        TargetResolution::FileBackedSuffix => {
            push_candidate_finding(
                findings,
                FindingCode::RelinkMissingInternalTarget,
                format!("file-backed target {target_id} cannot preserve an internal suffix"),
                source,
                &target_id,
                destination.line,
            );
            return ProjectedOutcome::Blocked;
        }
    };
    // The future owner legitimately does not exist yet, but the current target
    // must, or the plan is guessing about what it is repairing.
    match target_presence(&target, parsed.locator) {
        TargetPresence::Present => {}
        TargetPresence::Absent(missing_target) => {
            push_candidate_finding(
                findings,
                FindingCode::RelinkMissingInternalTarget,
                format!(
                    "preserved internal target does not exist: {}",
                    missing_target.display()
                ),
                source,
                &target_id,
                destination.line,
            );
            return ProjectedOutcome::Blocked;
        }
        TargetPresence::Unknown(path, err) => {
            findings.push(unknown_target_finding(
                source,
                &path,
                &err,
                destination.line,
                Some(&target_id),
            ));
            return ProjectedOutcome::Blocked;
        }
    }

    let source_parent = source_parent_of(source);
    let current =
        canonical_destination_text(source_parent, &target, parsed.locator, parsed.fragment);

    if projection.settled
        && record.source_kind == CanonicalSourceKind::TaskOwner
        && !normalize(&target).starts_with(normalize(&record.owner_path))
    {
        push_candidate_finding(
            findings,
            FindingCode::RelinkProjectionDrift,
            format!(
                "settled destination {resolved} for {target_id} resolves outside the intended indexed owner {}",
                record.owner_path.display()
            ),
            source,
            &target_id,
            destination.line,
        );
        return ProjectedOutcome::Blocked;
    }

    // Source and target move together and the *authored bytes* still land on the
    // same file afterwards, so the move causes no change here. Any ordinary
    // normalization this destination still needs belongs to global relink.
    // For an identity projection, this reduces to proving that the authored
    // destination resolves to the exact indexed target now.
    //
    // This must test the authored path, not `projected == current`. Canonical
    // text equality is not sufficient: an authored spelling that traverses above
    // the moving owner and re-descends resolves correctly today and breaks after
    // the move, even though both canonical strings are identical. Testing only
    // the canonical texts reported `complete:true` with zero changes for exactly
    // that case and left the link broken.
    if move_cannot_change_destination(
        source_parent,
        parsed.resolution_path_text,
        &target,
        projection,
    ) {
        return ProjectedOutcome::Unchanged;
    }

    if projection.settled {
        // The rename already happened, so there is nothing to project: this is
        // verification that the authored destination resolves to the intended
        // indexed target, independent of harmless spelling differences.
        push_candidate_finding(
            findings,
            FindingCode::RelinkProjectionDrift,
            format!(
                "settled destination {resolved} for {target_id} does not resolve to the intended target {current}"
            ),
            source,
            &target_id,
            destination.line,
        );
        return ProjectedOutcome::Blocked;
    }

    let projected = canonical_destination_text(
        &project_path(source_parent, projection),
        &project_path(&target, projection),
        parsed.locator,
        parsed.fragment,
    );
    if resolved == current {
        let rendered = match render_destination_text(&projected, destination.form) {
            Ok(rendered) => rendered,
            Err(err) => {
                // Never construct a partial replacement: unprovable bytes must
                // not reach the digest or the writer.
                findings.push(render_failure_finding(
                    &err,
                    source,
                    destination.line,
                    Some(&target_id),
                ));
                return ProjectedOutcome::Blocked;
            }
        };
        return ProjectedOutcome::Planned(Replacement {
            span: destination.span,
            semantic: projected,
            change: RelinkChange {
                path: source.to_path_buf(),
                line: destination.line,
                column: destination.column,
                id: Some(target_id),
                from: destination.original,
                to: rendered,
            },
        });
    }
    // Already at its future destination: a retry after a lost response must
    // settle, never reverse the repair because the folder has not moved yet.
    if resolved == projected {
        return ProjectedOutcome::Unchanged;
    }
    push_candidate_finding(
        findings,
        FindingCode::RelinkProjectionDrift,
        format!(
            "destination for {target_id} matches neither the current nor the projected canonical path; current {current}, projected {projected}"
        ),
        source,
        &target_id,
        destination.line,
    );
    ProjectedOutcome::Blocked
}

#[allow(clippy::too_many_arguments)]
fn resolve_generic_projected_destination(
    source: &Path,
    destination: markdown::MarkdownDestination,
    destination_extensions: &[super::RelinkDestinationExtension],
    projection: &RelinkProjection,
    source_moves: bool,
    findings: &mut Vec<Finding>,
) -> ProjectedOutcome {
    let Some(parsed) = parse_local_destination(&destination.resolved, destination_extensions)
    else {
        return ProjectedOutcome::Irrelevant;
    };
    let source_parent = source_parent_of(source);
    let candidates = generic_candidates(source_parent, parsed.resolution_path_text, projection);
    if !candidates.is_relevant(projection, source_moves) {
        return ProjectedOutcome::Irrelevant;
    }
    let GenericCandidates {
        future_source_parent,
        current_candidate,
        authored_after,
        projected_candidate,
    } = candidates;

    // Unproven candidate state must block before any valid/invalid reasoning:
    // reading an inspection error as absence is what let relink assert a
    // destination did not exist when it demonstrably did.
    let current_presence = target_presence(&current_candidate, parsed.locator);
    let projected_presence = target_presence(&projected_candidate, parsed.locator);
    for presence in [&current_presence, &projected_presence] {
        if let TargetPresence::Unknown(path, err) = presence {
            findings.push(unknown_target_finding(
                source,
                path,
                err,
                destination.line,
                None,
            ));
            return ProjectedOutcome::Blocked;
        }
    }
    let both_absent = matches!(
        (&current_presence, &projected_presence),
        (TargetPresence::Absent(_), TargetPresence::Absent(_))
    );

    let generic = GenericProjection {
        current_valid: matches!(current_presence, TargetPresence::Present),
        projected_valid: matches!(projected_presence, TargetPresence::Present)
            && authored_after == project_path(&projected_candidate, projection),
        current_candidate,
        authored_after,
        projected_candidate,
    };

    if generic.current_valid
        && generic.projected_valid
        && generic.current_candidate != generic.projected_candidate
    {
        push_generic_projection_finding(
            findings,
            format!(
                "local destination is ambiguous across current {} and projected {}",
                generic.current_candidate.display(),
                generic.projected_candidate.display()
            ),
            source,
            destination.line,
        );
        return ProjectedOutcome::Blocked;
    }

    if !generic.current_valid {
        if generic.projected_valid {
            return ProjectedOutcome::Unchanged;
        }
        if both_absent {
            // When settled the two candidates are the same path, so naming both
            // reads as a bug in the message rather than a fact about the corpus.
            let message = if generic.current_candidate == generic.projected_candidate {
                format!(
                    "local destination does not resolve: {}",
                    generic.current_candidate.display()
                )
            } else {
                format!(
                    "local destination resolves to neither current {} nor projected {}",
                    generic.current_candidate.display(),
                    generic.projected_candidate.display()
                )
            };
            push_unresolved_local_warning(findings, message, source, destination.line);
            return ProjectedOutcome::Advisory;
        }
        // When settled the two candidates are the same path, so naming both reads
        // as a bug in the message rather than a fact about the corpus.
        let message = if generic.current_candidate == generic.projected_candidate {
            format!(
                "local destination does not resolve: {}",
                generic.current_candidate.display()
            )
        } else {
            format!(
                "local destination resolves to neither current {} nor projected {}",
                generic.current_candidate.display(),
                generic.projected_candidate.display()
            )
        };
        push_generic_projection_finding(findings, message, source, destination.line);
        return ProjectedOutcome::Blocked;
    }

    if projection.settled {
        // The rename already happened, so this is verification, not planning.
        //
        // Generic and recognized-ref destinations are both verified by whether
        // they resolve unambiguously to their intended target, not by spelling.
        // Ordinary global relink retains normalization authority for recognized
        // refs. Slopid normalizes generic destinations in no mode, so this branch
        // must accept ordinary resolving forms such as `./x.md` and `dir/`.
        //
        // The `current_valid` check above already established that this
        // destination resolves. A spelling that genuinely breaks because the move
        // changes the owner's depth is caught *before* the move by this same
        // function's non-settled path, which compares the authored-after reading
        // against the projected target and plans or blocks accordingly.
        // (`move_cannot_change_destination` performs the identity-backed check for
        // recognized refs before this generic resolver is selected.)
        //
        // This branch is deliberately explicit rather than a fall-through. It is
        // documentation of a decided contract, not load-bearing control flow: with
        // `from_owner == to_owner` the projection is the identity, so the code
        // below would return `Unchanged` anyway. Deleting it changes no behaviour;
        // it exists so the decision is visible where the rule lives.
        return ProjectedOutcome::Unchanged;
    }

    let projected_target = project_path(&generic.current_candidate, projection);
    if generic.authored_after == projected_target {
        return ProjectedOutcome::Unchanged;
    }
    let semantic = canonical_destination_text(
        &future_source_parent,
        &projected_target,
        parsed.locator,
        parsed.fragment,
    );
    let rendered = match render_destination_text(&semantic, destination.form) {
        Ok(rendered) => rendered,
        Err(err) => {
            findings.push(render_failure_finding(&err, source, destination.line, None));
            return ProjectedOutcome::Blocked;
        }
    };
    ProjectedOutcome::Planned(Replacement {
        span: destination.span,
        semantic,
        change: RelinkChange {
            path: source.to_path_buf(),
            line: destination.line,
            column: destination.column,
            id: None,
            from: destination.original,
            to: rendered,
        },
    })
}

fn push_generic_projection_finding(
    findings: &mut Vec<Finding>,
    message: String,
    source: &Path,
    line: usize,
) {
    let mut finding = Finding::error(
        FindingCode::RelinkProjectionDrift,
        message,
        Some(source.to_path_buf()),
    );
    finding.line = Some(line);
    findings.push(finding);
}

fn push_unresolved_local_warning(
    findings: &mut Vec<Finding>,
    message: String,
    source: &Path,
    line: usize,
) {
    let mut finding = Finding::warning(
        FindingCode::RelinkUnresolvedLocalDestination,
        message,
        Some(source.to_path_buf()),
    );
    finding.line = Some(line);
    findings.push(finding);
}

/// Rewrite one path as it will read after the owner move. Paths outside the
/// moving owner are unchanged, which is what keeps other tasks' destinations at
/// their current canonical locations.
///
/// `Path::strip_prefix` matches whole components, so a sibling directory whose
/// name merely starts with the owner's name is correctly left alone.
fn project_path(path: &Path, projection: &RelinkProjection) -> PathBuf {
    match path.strip_prefix(&projection.from_owner) {
        Ok(inner) => projection.to_owner.join(inner),
        Err(_) => path.to_path_buf(),
    }
}

fn unproject_path(path: &Path, projection: &RelinkProjection) -> PathBuf {
    match path.strip_prefix(&projection.to_owner) {
        Ok(inner) => projection.from_owner.join(inner),
        Err(_) => path.to_path_buf(),
    }
}

/// Can this move leave the authored destination alone?
///
/// Two conditions, and both are load-bearing:
///
/// 1. The authored path must resolve to the canonical target *today*. A
///    destination we cannot account for is drift, and drift fails closed rather
///    than being waved through as "unaffected" — a caller must not move a task
///    while a relevant link is in an unexplained state, even though the move
///    itself would not change that link.
/// 2. Interpreted from the source's future parent, the authored path must still
///    land on the post-move location of what it points at today.
///
/// Condition 2 must test the authored bytes rather than canonical text. A move
/// changes the *depth* of every file inside the owner, so an authored path that
/// walks above the owner boundary and re-descends breaks even when its current
/// and projected canonical spellings are byte-identical.
///
/// Resolution is lexical. An authored absolute path replaces the base outright,
/// which is the correct reading and correctly reports that an absolute link into
/// the moving owner does not survive.
fn move_cannot_change_destination(
    source_parent: &Path,
    resolution_path_text: &str,
    target: &Path,
    projection: &RelinkProjection,
) -> bool {
    let authored = Path::new(resolution_path_text);
    let now = normalize(&source_parent.join(authored));
    if now != normalize(target) {
        return false;
    }
    let after = normalize(&project_path(source_parent, projection).join(authored));
    after == project_path(&now, projection)
}

fn plan_digest(authority: &MoveRelinkAuthority) -> Result<String> {
    let bytes =
        serde_json::to_vec(authority).context("serialize projected relink plan authority")?;
    Ok(hex_sha256(&bytes))
}

fn hex_sha256(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut text = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn plan_changed_finding() -> Finding {
    Finding::error(
        FindingCode::RelinkPlanChanged,
        "projected relink plan no longer matches the expected digest; take a fresh preview and approve the current plan",
        None,
    )
}

fn preview_result(authority: MoveRelinkAuthority, digest: String) -> RelinkResult {
    RelinkResult {
        complete: authority.complete,
        applied: false,
        changes: authority.changes,
        findings: authority.findings,
        projection: Some(authority.projection),
        plan_sha256: Some(digest),
    }
}

/// A pre-write refusal. Nothing was applied, so it reports no changes and the
/// digest the caller must approve next.
fn refused_result(
    authority: MoveRelinkAuthority,
    digest: String,
    transient: Option<Finding>,
) -> RelinkResult {
    let mut findings = authority.findings;
    if let Some(finding) = transient {
        findings.push(finding);
    }
    findings.sort_by(compare_findings);
    RelinkResult {
        complete: false,
        applied: false,
        changes: Vec::new(),
        findings,
        projection: Some(authority.projection),
        plan_sha256: Some(digest),
    }
}

/// Hand the approved plan to the existing per-file writer.
///
/// Two checks narrow the race and neither closes it. The whole-plan digest
/// refuses an approval that no longer matches the corpus. The per-file byte
/// comparison then skips any file whose bytes already changed before that
/// comparison, so one raced file cannot roll back an independent successful
/// replacement. Both are early-race checks only: an edit landing after the
/// comparison and before atomic replacement is overwritten, which is why the
/// caller owns authored-writer quiescence across this whole interval.
///
/// The returned digest stays the approved pre-write digest — the next state comes
/// from a fresh preview, not from hashing after a partial write.
fn apply_move_plan_with<F>(plan: MoveRelinkPlan, digest: String, replace: F) -> RelinkResult
where
    F: FnMut(&Path, &[u8], Permissions) -> std::io::Result<()>,
{
    let MoveRelinkPlan {
        authority,
        files,
        usable,
    } = plan;
    let MoveRelinkAuthority {
        projection,
        complete,
        findings,
        ..
    } = authority;
    let mut result = apply_plan_with(
        RelinkPlan {
            files,
            findings,
            complete,
            usable,
        },
        replace,
    );
    result.projection = Some(projection);
    result.plan_sha256 = Some(digest);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projection(from: &Path, to: &Path) -> RelinkProjection {
        RelinkProjection {
            id: "sa2a7".into(),
            from_owner: from.to_path_buf(),
            to_owner: to.to_path_buf(),
            settled: false,
        }
    }

    fn replacement(path: &Path, from: &str, to: &str) -> Replacement {
        Replacement {
            span: 0..from.len(),
            semantic: to.into(),
            change: RelinkChange {
                path: path.to_path_buf(),
                line: 1,
                column: 1,
                id: Some("sa2a7".into()),
                from: from.into(),
                to: to.into(),
            },
        }
    }

    fn plan(files: Vec<PlannedFile>, projection: RelinkProjection) -> MoveRelinkPlan {
        MoveRelinkPlan {
            authority: MoveRelinkAuthority {
                contract: MOVE_CONTRACT,
                projection,
                complete: true,
                effect_sources: Vec::new(),
                changes: Vec::new(),
                findings: Vec::new(),
            },
            files,
            usable: true,
        }
    }

    #[test]
    fn raced_effect_source_is_skipped_while_an_independent_file_applies() {
        let tmp = tempfile::tempdir().unwrap();
        let raced = tmp.path().join("raced.md");
        let unchanged = tmp.path().join("unchanged.md");
        fs::write(&raced, "old").unwrap();
        fs::write(&unchanged, "old").unwrap();
        let approved = "a".repeat(64);
        let plan = plan(
            vec![
                PlannedFile {
                    path: raced.clone(),
                    scanned: b"old".to_vec(),
                    replacements: vec![replacement(&raced, "old", "new")],
                },
                PlannedFile {
                    path: unchanged.clone(),
                    scanned: b"old".to_vec(),
                    replacements: vec![replacement(&unchanged, "old", "new")],
                },
            ],
            projection(&tmp.path().join("from"), &tmp.path().join("to")),
        );

        // Race exactly one approved file after the whole-plan digest matched.
        fs::write(&raced, "concurrent").unwrap();
        let result = apply_move_plan_with(plan, approved.clone(), atomic_replace);

        assert!(result.applied);
        assert!(!result.complete);
        assert_eq!(
            result.plan_sha256.as_deref(),
            Some(approved.as_str()),
            "a projected write keeps the approved pre-write digest"
        );
        assert!(result.projection.is_some());
        assert_eq!(fs::read_to_string(raced).unwrap(), "concurrent");
        assert_eq!(fs::read_to_string(unchanged).unwrap(), "new");
        assert_eq!(result.changes.len(), 1);
        assert!(
            result
                .findings
                .iter()
                .any(|finding| finding.code == FindingCode::RelinkConcurrentChange)
        );
    }

    #[test]
    fn plan_digest_is_stable_and_binds_the_contract_marker() {
        let projection = projection(Path::new("/from"), Path::new("/to"));
        let authority = || MoveRelinkAuthority {
            contract: MOVE_CONTRACT,
            projection: projection.clone(),
            complete: true,
            effect_sources: vec![EffectSourceAuthority {
                path: PathBuf::from("/a.md"),
                sha256: hex_sha256(b"a"),
            }],
            changes: Vec::new(),
            findings: Vec::new(),
        };
        let digest = plan_digest(&authority()).unwrap();
        assert_eq!(digest.len(), 64);
        assert_eq!(digest, plan_digest(&authority()).unwrap());

        let mut other = authority();
        assert_eq!(MOVE_CONTRACT, "sid-relink-move-v2");
        other.contract = "sid-relink-move-v1";
        assert_ne!(digest, plan_digest(&other).unwrap());

        let mut changed_source = authority();
        changed_source.effect_sources[0].sha256 = hex_sha256(b"b");
        assert_ne!(digest, plan_digest(&changed_source).unwrap());

        let mut warned = authority();
        let mut warning = Finding::warning(
            FindingCode::RelinkUnresolvedLocalDestination,
            "local destination does not resolve: /missing.md",
            Some(PathBuf::from("/a.md")),
        );
        warning.line = Some(7);
        warned.findings.push(warning);
        let warning_digest = plan_digest(&warned).unwrap();
        assert_ne!(digest, warning_digest);

        let mut changed_warning = warned;
        changed_warning.findings[0].line = Some(8);
        assert_ne!(warning_digest, plan_digest(&changed_warning).unwrap());
    }

    #[test]
    fn only_not_found_proves_the_projected_destination_absent() {
        use std::io::{Error, ErrorKind};

        assert_eq!(
            classify_projected_destination(&Ok::<(), Error>(())),
            ProjectedDestinationState::Occupied
        );
        assert_eq!(
            classify_projected_destination(&Err::<(), Error>(Error::from(ErrorKind::NotFound))),
            ProjectedDestinationState::Absent
        );
        // Everything else leaves the destination's state unknown. Reading these
        // as absence let a scoped write rewrite inbound links before a rename
        // that could not succeed.
        for kind in [
            ErrorKind::PermissionDenied,
            ErrorKind::NotADirectory,
            ErrorKind::InvalidInput,
        ] {
            assert_eq!(
                classify_projected_destination(&Err::<(), Error>(Error::from(kind))),
                ProjectedDestinationState::Unknown(kind),
                "{kind:?} must not be read as proof of absence"
            );
        }
        assert!(matches!(
            classify_projected_destination(&Err::<(), Error>(Error::other("opaque"))),
            ProjectedDestinationState::Unknown(_)
        ));
    }

    #[test]
    fn hex_sha256_is_lowercase_and_matches_a_known_vector() {
        assert_eq!(
            hex_sha256(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn project_path_rewrites_only_paths_inside_the_moving_owner() {
        let projection = projection(
            Path::new("/stm/202607_sa2a7_x"),
            Path::new("/stm/.archive/202607_sa2a7_x"),
        );
        assert_eq!(
            project_path(Path::new("/stm/202607_sa2a7_x/notes.md"), &projection),
            Path::new("/stm/.archive/202607_sa2a7_x/notes.md")
        );
        assert_eq!(
            project_path(Path::new("/stm/202607_sa2a7_x"), &projection),
            Path::new("/stm/.archive/202607_sa2a7_x")
        );
        assert_eq!(
            project_path(
                Path::new("/stm/202607_sd5d2_y/CURRENT_STATE.md"),
                &projection
            ),
            Path::new("/stm/202607_sd5d2_y/CURRENT_STATE.md")
        );
        assert_eq!(
            unproject_path(
                Path::new("/stm/.archive/202607_sa2a7_x/notes.md"),
                &projection
            ),
            Path::new("/stm/202607_sa2a7_x/notes.md")
        );
        assert_eq!(
            unproject_path(
                Path::new("/stm/.archive/202607_sa2a7_x-notes/notes.md"),
                &projection
            ),
            Path::new("/stm/.archive/202607_sa2a7_x-notes/notes.md")
        );
    }
}
