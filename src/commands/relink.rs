use crate::documents::{self, CanonicalSourceKind, Finding, FindingCode, compare_findings};
use crate::{markdown, refcode, scan, walk};
use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs::{self, Permissions};
use std::io::{ErrorKind, Write};
use std::ops::Range;
use std::path::{Component, Path, PathBuf};

use super::{RelinkDestinationExtension, load_project_config};

mod projected;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RelinkChange {
    pub path: PathBuf,
    pub line: usize,
    pub column: usize,
    pub id: Option<String>,
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug)]
struct Replacement {
    span: Range<usize>,
    change: RelinkChange,
}

#[derive(Clone, Debug)]
struct PlannedFile {
    path: PathBuf,
    scanned: Vec<u8>,
    replacements: Vec<Replacement>,
}

#[derive(Clone, Debug)]
struct RelinkPlan {
    files: Vec<PlannedFile>,
    findings: Vec<Finding>,
    complete: bool,
    usable: bool,
}

/// One exact owner move, projected but not yet performed. Present only on
/// projected results; `settled` means the owner already lives at `to_owner`,
/// so the command verifies scoped destinations instead of planning a move.
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct RelinkProjection {
    pub id: String,
    pub from_owner: PathBuf,
    pub to_owner: PathBuf,
    pub settled: bool,
}

#[derive(Debug, Serialize)]
pub struct RelinkResult {
    pub complete: bool,
    pub applied: bool,
    pub changes: Vec<RelinkChange>,
    pub findings: Vec<Finding>,

    // Projected-only fields. Global relink leaves both `None` so its wire
    // contract stays exactly `complete`, `applied`, `changes`, `findings`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projection: Option<RelinkProjection>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_sha256: Option<String>,
}

pub struct RelinkInputs {
    pub write: bool,
    pub move_id: Option<String>,
    pub into: Option<String>,
    pub expected_plan_sha256: Option<String>,
}

pub fn cmd_relink(inputs: RelinkInputs, cwd: &Path) -> Result<RelinkResult> {
    let config = load_project_config(cwd)?;
    // The CLI layer already enforces these relationships; the library API
    // repeats them so a future caller cannot request an unscoped digest.
    match (inputs.move_id.as_deref(), inputs.into.as_deref()) {
        (Some(move_id), Some(into)) => projected::cmd_relink_move(
            &config,
            move_id,
            into,
            inputs.write,
            inputs.expected_plan_sha256.as_deref(),
        ),
        (None, None) => {
            if inputs.expected_plan_sha256.is_some() {
                bail!("--expected-plan-sha256 requires --move and --into");
            }
            cmd_relink_global(&config, inputs.write)
        }
        _ => bail!("--move and --into must be supplied together"),
    }
}

fn cmd_relink_global(config: &super::ProjectConfig, write: bool) -> Result<RelinkResult> {
    let plan = plan_relink(config);
    if !plan.usable {
        bail!("relink could not obtain any usable authored Markdown source");
    }
    if write {
        Ok(apply_plan_with(plan, atomic_replace))
    } else {
        let mut changes = plan
            .files
            .iter()
            .flat_map(|file| {
                file.replacements
                    .iter()
                    .map(|replacement| replacement.change.clone())
            })
            .collect::<Vec<_>>();
        changes.sort_by(compare_changes);
        Ok(RelinkResult {
            complete: plan.complete,
            applied: false,
            changes,
            findings: plan.findings,
            projection: None,
            plan_sha256: None,
        })
    }
}

/// Authored Markdown that relink is allowed to read, plus the coverage state of
/// collecting it. Global and projected planning share this so neither can scan a
/// different corpus than the other.
struct SourceScan {
    paths: Vec<PathBuf>,
    findings: Vec<Finding>,
    complete: bool,
    usable: bool,
}

fn plan_relink(config: &super::ProjectConfig) -> RelinkPlan {
    let index = documents::scan_sources(&config.document_sources());
    let scan = scan_relink_sources(config, &index);
    let mut findings = scan.findings;
    let mut complete = scan.complete;

    let mut files = Vec::new();
    for path in scan.paths {
        let scanned = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
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
        let mut replacements = Vec::new();
        for destination in markdown::destinations(text) {
            if let Some(replacement) = resolve_destination(
                &path,
                destination,
                &index,
                &config.relink_destination_extensions,
                &mut findings,
            ) {
                replacements.push(replacement);
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
        files.push(PlannedFile {
            path,
            scanned,
            replacements,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    findings.sort_by(compare_findings);
    RelinkPlan {
        files,
        findings,
        complete,
        usable: scan.usable,
    }
}

fn has_overlapping_spans(replacements: &[Replacement]) -> bool {
    replacements
        .windows(2)
        .any(|window| window[0].span.end > window[1].span.start)
}

fn scan_relink_sources(
    config: &super::ProjectConfig,
    index: &documents::DocumentIndex,
) -> SourceScan {
    let mut findings = index
        .findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.code,
                FindingCode::UnreadableEntry | FindingCode::UnreadableRoot
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut complete = findings.is_empty();
    let mut usable = config.scan_roots.iter().any(|root| root.is_dir())
        || config.topic_roots.iter().any(|root| root.is_dir())
        || config.seed_root.is_dir();
    let mut paths = BTreeSet::new();

    for record in index
        .nodes
        .values()
        .filter(|record| record.source_kind == CanonicalSourceKind::TaskOwner)
    {
        let outcome = walk::walk_files(
            &record.owner_path,
            &[config.note_root.clone(), config.seed_root.clone()],
            true,
        );
        usable |= outcome.usable;
        complete &= outcome.findings.is_empty();
        findings.extend(outcome.findings);
        paths.extend(
            outcome
                .paths
                .into_iter()
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md")),
        );
    }
    for root in &config.topic_roots {
        let mut exclusions = vec![config.note_root.clone(), config.seed_root.clone()];
        exclusions.extend(config.scan_roots.iter().cloned());
        let outcome = walk::walk_files(root, &exclusions, false);
        usable |= outcome.usable;
        complete &= outcome.findings.is_empty();
        findings.extend(outcome.findings);
        paths.extend(
            outcome
                .paths
                .into_iter()
                .filter(|path| path.extension().and_then(|value| value.to_str()) == Some("md")),
        );
    }
    match fs::read_dir(&config.seed_root) {
        Ok(entries) => {
            usable = true;
            for entry in entries {
                match entry {
                    Ok(entry)
                        if entry.file_type().is_ok_and(|kind| kind.is_file())
                            && entry.path().extension().and_then(|value| value.to_str())
                                == Some("md") =>
                    {
                        paths.insert(entry.path());
                    }
                    Ok(_) => {}
                    Err(err) => {
                        complete = false;
                        findings.push(Finding::error(
                            FindingCode::UnreadableEntry,
                            format!("cannot read seed source entry for relink: {err}"),
                            Some(config.seed_root.clone()),
                        ));
                    }
                }
            }
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => {
            complete = false;
            findings.push(Finding::error(
                FindingCode::UnreadableRoot,
                format!("cannot read seed root for relink: {err}"),
                Some(config.seed_root.clone()),
            ));
        }
    }

    SourceScan {
        paths: paths.into_iter().collect(),
        findings,
        complete,
        usable,
    }
}

/// The stronger identity-backed interpretation of a local destination: exactly
/// one embedded canonical ref, with suffix and locator semantics preserved.
struct LocalDestination<'a> {
    resolution_path_text: &'a str,
    locator: Option<&'a str>,
    fragment: &'a str,
}

fn parse_local_destination<'a>(
    resolved: &'a str,
    destination_extensions: &[RelinkDestinationExtension],
) -> Option<LocalDestination<'a>> {
    let (path_text, fragment) = resolved
        .split_once('#')
        .map_or((resolved, ""), |(path, fragment)| (path, fragment));
    if path_text.is_empty() || is_external(path_text) {
        return None;
    }
    let colon_line = destination_extensions
        .contains(&RelinkDestinationExtension::ColonLine)
        .then(|| split_colon_line(path_text))
        .flatten();
    let (resolution_path_text, locator) = colon_line
        .map(|(base, locator)| (base, Some(locator)))
        .unwrap_or((path_text, None));
    Some(LocalDestination {
        resolution_path_text,
        locator,
        fragment,
    })
}

struct DestinationRef<'a> {
    fragment: &'a str,
    locator: Option<&'a str>,
    target_id: String,
    suffix: Vec<&'a str>,
    seed_destination: bool,
    /// The authored path exactly as it resolves on disk: fragment and any
    /// `colon-line` locator removed, escapes already decoded. Projection needs
    /// this because where the *authored bytes* point can differ from where the
    /// canonical text points.
    resolution_path_text: &'a str,
}

fn parse_destination_ref<'a>(
    resolved: &'a str,
    destination_extensions: &[RelinkDestinationExtension],
) -> Option<DestinationRef<'a>> {
    let local = parse_local_destination(resolved, destination_extensions)?;
    let components = local.resolution_path_text.split('/').collect::<Vec<_>>();
    let mut embedded = Vec::new();
    for (offset, component) in components.iter().enumerate() {
        let stem = component.strip_suffix(".md").unwrap_or(component);
        if let Some((_, candidate_ref)) = scan::split_task_folder_name(stem)
            && is_supported_ref(candidate_ref)
        {
            embedded.push((
                offset,
                candidate_ref.to_string(),
                component.ends_with(".md"),
            ));
        }
    }
    let [(component_index, target_id, seed_destination)] = embedded.as_slice() else {
        return None;
    };
    Some(DestinationRef {
        fragment: local.fragment,
        locator: local.locator,
        target_id: target_id.clone(),
        suffix: components[component_index + 1..].to_vec(),
        seed_destination: *seed_destination,
        resolution_path_text: local.resolution_path_text,
    })
}

enum TargetResolution {
    Resolved(PathBuf),
    /// A file-backed record (seed or topic) cannot carry an internal suffix.
    FileBackedSuffix,
}

fn resolve_target_path(
    record: &documents::DocumentRecord,
    suffix: &[&str],
    seed_destination: bool,
) -> TargetResolution {
    if seed_destination || suffix.is_empty() || suffix == ["CURRENT_STATE.md"] {
        TargetResolution::Resolved(record.node.path.clone())
    } else if record.source_kind == CanonicalSourceKind::TaskOwner {
        let mut target = record.owner_path.clone();
        for component in suffix {
            target.push(component);
        }
        TargetResolution::Resolved(target)
    } else {
        TargetResolution::FileBackedSuffix
    }
}

/// The path a caller should report as missing, or `None` when the current target
/// exists. A `colon-line` locator may name a literal file, so either spelling
/// existing is enough.
fn missing_target_path(target: &Path, locator: Option<&str>) -> Option<PathBuf> {
    let literal_target = locator.and_then(|locator| append_file_name_suffix(target, locator));
    let literal_exists = literal_target
        .as_ref()
        .is_some_and(|literal_target| literal_target.exists());
    if literal_exists || target.exists() {
        None
    } else {
        Some(literal_target.unwrap_or_else(|| target.to_path_buf()))
    }
}

fn canonical_destination_text(
    source_parent: &Path,
    target: &Path,
    locator: Option<&str>,
    fragment: &str,
) -> String {
    let relative = relative_path(source_parent, target);
    let mut text = path_text_for_wire(&relative);
    if let Some(locator) = locator {
        text.push_str(locator);
    }
    if !fragment.is_empty() {
        text.push('#');
        text.push_str(fragment);
    }
    text
}

fn render_destination_text(semantic: &str, form: markdown::MarkdownDestinationForm) -> String {
    match form {
        markdown::MarkdownDestinationForm::Bare => {
            let mut stack = Vec::new();
            let mut matched = BTreeSet::new();
            for (offset, ch) in semantic.char_indices() {
                match ch {
                    '(' => stack.push(offset),
                    ')' => {
                        if let Some(open) = stack.pop() {
                            matched.insert(open);
                            matched.insert(offset);
                        }
                    }
                    _ => {}
                }
            }
            let mut rendered = String::with_capacity(semantic.len());
            for (offset, ch) in semantic.char_indices() {
                if ch == '\\' || (matches!(ch, '(' | ')') && !matched.contains(&offset)) {
                    rendered.push('\\');
                }
                rendered.push(ch);
            }
            rendered
        }
        markdown::MarkdownDestinationForm::Angle => {
            let mut rendered = String::with_capacity(semantic.len());
            for ch in semantic.chars() {
                if matches!(ch, '\\' | '<' | '>') {
                    rendered.push('\\');
                }
                rendered.push(ch);
            }
            rendered
        }
    }
}

fn source_parent_of(source: &Path) -> &Path {
    source.parent().unwrap_or_else(|| Path::new("/"))
}

fn unresolved_ref_finding_code(index: &documents::DocumentIndex, target_id: &str) -> FindingCode {
    let ambiguous = index.findings.iter().any(|finding| {
        finding.code == FindingCode::DuplicateId && finding.id.as_deref() == Some(target_id)
    });
    if ambiguous {
        FindingCode::RelinkAmbiguousRef
    } else {
        FindingCode::RelinkUnresolvedRef
    }
}

fn unresolved_ref_message(code: FindingCode, target_id: &str) -> String {
    if code == FindingCode::RelinkAmbiguousRef {
        format!("embedded ref {target_id} resolves ambiguously")
    } else {
        format!("embedded ref {target_id} does not resolve")
    }
}

fn resolve_destination(
    source: &Path,
    destination: markdown::MarkdownDestination,
    index: &documents::DocumentIndex,
    destination_extensions: &[RelinkDestinationExtension],
    findings: &mut Vec<Finding>,
) -> Option<Replacement> {
    let resolved = destination.resolved;
    let parsed = parse_destination_ref(&resolved, destination_extensions)?;
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
        return None;
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
            return None;
        }
    };
    if let Some(missing_target) = missing_target_path(&target, parsed.locator) {
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
        return None;
    }
    let canonical = canonical_destination_text(
        source_parent_of(source),
        &target,
        parsed.locator,
        parsed.fragment,
    );
    let replacement_text = render_destination_text(&canonical, destination.form);
    if resolved == canonical && destination.original == replacement_text {
        return None;
    }
    Some(Replacement {
        span: destination.span,
        change: RelinkChange {
            path: source.to_path_buf(),
            line: destination.line,
            column: destination.column,
            id: Some(target_id),
            from: destination.original,
            to: replacement_text,
        },
    })
}

fn split_colon_line(destination: &str) -> Option<(&str, &str)> {
    let offset = destination.rfind(':')?;
    let (base, locator) = destination.split_at(offset);
    let digits = locator.strip_prefix(':')?;
    if base.is_empty()
        || digits.is_empty()
        || digits.starts_with('0')
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    Some((base, locator))
}

fn append_file_name_suffix(path: &Path, suffix: &str) -> Option<PathBuf> {
    let mut file_name = path.file_name()?.to_os_string();
    file_name.push(suffix);
    let mut suffixed = path.to_path_buf();
    suffixed.set_file_name(file_name);
    Some(suffixed)
}

fn push_candidate_finding(
    findings: &mut Vec<Finding>,
    code: FindingCode,
    message: String,
    source: &Path,
    id: &str,
    line: usize,
) {
    let mut finding = Finding::error(code, message, Some(source.to_path_buf()));
    finding.id = Some(id.to_string());
    finding.line = Some(line);
    findings.push(finding);
}

fn is_external(destination: &str) -> bool {
    if destination.starts_with("//") {
        return true;
    }
    let Some(colon) = destination.find(':') else {
        return false;
    };
    let scheme = &destination[..colon];
    let mut chars = scheme.chars();
    chars.next().is_some_and(|ch| ch.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '-' | '.'))
}

fn is_supported_ref(value: &str) -> bool {
    refcode::is_recognized_ref(value)
        || (value.len() == 4
            && value
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()))
}

fn relative_path(from: &Path, to: &Path) -> PathBuf {
    let from = normalize(from);
    let to = normalize(to);
    let from_components = from.components().collect::<Vec<_>>();
    let to_components = to.components().collect::<Vec<_>>();
    let shared = from_components
        .iter()
        .zip(&to_components)
        .take_while(|(left, right)| left == right)
        .count();
    let mut relative = PathBuf::new();
    for _ in shared..from_components.len() {
        relative.push("..");
    }
    for component in &to_components[shared..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    relative
}

fn normalize(path: &Path) -> PathBuf {
    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                output.pop();
            }
            other => output.push(other.as_os_str()),
        }
    }
    output
}

fn path_text_for_wire(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn apply_plan_with<F>(plan: RelinkPlan, mut replace: F) -> RelinkResult
where
    F: FnMut(&Path, &[u8], Permissions) -> std::io::Result<()>,
{
    let mut complete = plan.complete;
    let mut findings = plan.findings;
    let mut changes = Vec::new();
    for file in plan.files {
        if file.replacements.is_empty() {
            continue;
        }
        let current = match fs::read(&file.path) {
            Ok(bytes) => bytes,
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::RelinkConcurrentChange,
                    format!("cannot re-read planned source: {err}"),
                    Some(file.path),
                ));
                continue;
            }
        };
        if current != file.scanned {
            complete = false;
            findings.push(Finding::error(
                FindingCode::RelinkConcurrentChange,
                "source changed after relink planning; whole file skipped",
                Some(file.path),
            ));
            continue;
        }
        let permissions = match fs::metadata(&file.path) {
            Ok(metadata) => metadata.permissions(),
            Err(err) => {
                complete = false;
                findings.push(Finding::error(
                    FindingCode::RelinkConcurrentChange,
                    format!("cannot inspect planned source permissions: {err}"),
                    Some(file.path),
                ));
                continue;
            }
        };
        let mut updated = file.scanned;
        for replacement in file.replacements.iter().rev() {
            updated.splice(
                replacement.span.clone(),
                replacement.change.to.as_bytes().iter().copied(),
            );
        }
        if let Err(err) = replace(&file.path, &updated, permissions) {
            complete = false;
            findings.push(Finding::error(
                FindingCode::RelinkWriteFailed,
                format!("atomic relink replacement failed: {err}"),
                Some(file.path),
            ));
            continue;
        }
        changes.extend(
            file.replacements
                .into_iter()
                .map(|replacement| replacement.change),
        );
    }
    changes.sort_by(compare_changes);
    findings.sort_by(compare_findings);
    RelinkResult {
        complete,
        applied: true,
        changes,
        findings,
        projection: None,
        plan_sha256: None,
    }
}

fn atomic_replace(path: &Path, bytes: &[u8], permissions: Permissions) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("source path has no parent"))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.as_file().set_permissions(permissions)?;
    temporary.write_all(bytes)?;
    temporary.flush()?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|err| err.error)?;
    Ok(())
}

fn compare_changes(left: &RelinkChange, right: &RelinkChange) -> std::cmp::Ordering {
    (&left.path, left.line, left.column, &left.id).cmp(&(
        &right.path,
        right.line,
        right.column,
        &right.id,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn replacement(path: &Path, from: &str, to: &str) -> Replacement {
        Replacement {
            span: 0..from.len(),
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

    #[test]
    fn raced_file_is_skipped_while_an_unchanged_file_applies() {
        let tmp = tempfile::tempdir().unwrap();
        let raced = tmp.path().join("raced.md");
        let unchanged = tmp.path().join("unchanged.md");
        fs::write(&raced, "old").unwrap();
        fs::write(&unchanged, "old").unwrap();
        let plan = RelinkPlan {
            files: vec![
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
            findings: Vec::new(),
            complete: true,
            usable: true,
        };
        fs::write(&raced, "concurrent").unwrap();
        let result = apply_plan_with(plan, atomic_replace);
        assert!(result.applied);
        assert!(!result.complete);
        assert_eq!(fs::read_to_string(raced).unwrap(), "concurrent");
        assert_eq!(fs::read_to_string(unchanged).unwrap(), "new");
        assert_eq!(result.changes.len(), 1);
        assert!(
            result
                .findings
                .iter()
                .any(|finding| { finding.code == FindingCode::RelinkConcurrentChange })
        );
    }

    #[test]
    fn one_injected_write_failure_does_not_block_an_independent_file() {
        let tmp = tempfile::tempdir().unwrap();
        let failed = tmp.path().join("a.md");
        let applied = tmp.path().join("b.md");
        fs::write(&failed, "old").unwrap();
        fs::write(&applied, "old").unwrap();
        let plan = RelinkPlan {
            files: vec![
                PlannedFile {
                    path: failed.clone(),
                    scanned: b"old".to_vec(),
                    replacements: vec![replacement(&failed, "old", "new")],
                },
                PlannedFile {
                    path: applied.clone(),
                    scanned: b"old".to_vec(),
                    replacements: vec![replacement(&applied, "old", "new")],
                },
            ],
            findings: Vec::new(),
            complete: true,
            usable: true,
        };
        let result = apply_plan_with(plan, |path, bytes, permissions| {
            if path == failed {
                Err(std::io::Error::other("injected write failure"))
            } else {
                atomic_replace(path, bytes, permissions)
            }
        });
        assert!(!result.complete);
        assert_eq!(fs::read_to_string(failed).unwrap(), "old");
        assert_eq!(fs::read_to_string(applied).unwrap(), "new");
        assert_eq!(result.changes.len(), 1);
        assert!(
            result
                .findings
                .iter()
                .any(|finding| { finding.code == FindingCode::RelinkWriteFailed })
        );
    }

    #[test]
    fn overlap_detection_accepts_adjacent_spans_and_rejects_one_shared_byte() {
        let path = Path::new("/a.md");
        let span = |start: usize, end: usize| Replacement {
            span: start..end,
            change: RelinkChange {
                path: path.to_path_buf(),
                line: 1,
                column: 1,
                id: Some("sa2a7".into()),
                from: "x".into(),
                to: "y".into(),
            },
        };

        assert!(!has_overlapping_spans(&[]));
        assert!(!has_overlapping_spans(&[span(0, 5)]));
        // Touching but disjoint: `end` is exclusive, so 5 abutting 5 is legal and
        // must stay plannable.
        assert!(!has_overlapping_spans(&[span(0, 5), span(5, 9)]));
        // One shared byte is the first illegal case.
        assert!(has_overlapping_spans(&[span(0, 6), span(5, 9)]));
        // Detection is pairwise over sorted neighbours, so a later overlap counts.
        assert!(has_overlapping_spans(&[span(0, 2), span(4, 8), span(7, 9)]));
    }

    #[test]
    fn destination_renderer_escapes_only_form_sensitive_content() {
        use markdown::MarkdownDestinationForm::{Angle, Bare};

        assert_eq!(render_destination_text("a(b).md", Bare), "a(b).md");
        assert_eq!(render_destination_text("close).md", Bare), "close\\).md");
        assert_eq!(render_destination_text("a(b.md", Bare), "a\\(b.md");
        assert_eq!(render_destination_text("a\\b.md", Bare), "a\\\\b.md");
        assert_eq!(render_destination_text("a(b).md", Angle), "a(b).md");
        assert_eq!(
            render_destination_text("a\\b<c>.md", Angle),
            "a\\\\b\\<c\\>.md"
        );
    }

    #[cfg(unix)]
    #[test]
    fn atomic_replacement_preserves_source_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mode.md");
        fs::write(&path, "old").unwrap();
        fs::set_permissions(&path, Permissions::from_mode(0o640)).unwrap();
        atomic_replace(&path, b"new", fs::metadata(&path).unwrap().permissions()).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }
}
