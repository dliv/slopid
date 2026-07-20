# Harden allocation semantics

Ref: `seag8`
Created: 2026-05-31
Closed: 2026-05-31
Branch: `bootstrap-tdd`
Starting point: `66d3b75 Bootstrap sid TDD slice`

## Closeout

This task is complete. The durable behavior that should outlive this STM folder
now lives in:

- `docs/contracts/0001-sid-new-dry-run.md`
- `docs/decisions/0003-best-effort-ref-uniqueness.md`
- `docs/decisions/0004-json-success-output.md`

This file remains as historical task memory and review context, not the primary
requirements source.

## Why

The first TDD slice is green: `sid new`, `sid new --dry-run`, JSON output,
internal 1134 ref logic, and a plan/execute split all exist.

Before widening into `.sid` config, `sid list`, or additional scan roots, close
the allocation invariants identified by the contract doc and the Cursor/Codex
reviews.

## Resume Thread

We had just finished the multi-agent review digression. The intended next
high-level item before that review was:

```text
Do one more allocation hardening pass before adding config.
```

The review feedback sharpened that into:

- harden the pending allocation tests from Contract 0001;
- make the command easier for future agents to test deterministically;
- tighten accidental public API boundaries;
- document the accepted v1 concurrency limitation precisely.
- settle small edge policies surfaced by AMP before they calcify: long slugs,
  malformed injected periods, and missing-root scan races.

Owner decision: explicitly accept the concurrent same-ref/different-slug race in
v1. A collision is not ideal, but it is not catastrophic; this is a slop id, and
agents/users can disambiguate multiple `sXXXX` matches by month, slug, and
surrounding context. Do not add a lock file or hidden reservation mechanism in
this hardening pass. Add a concise code comment near `execute_new` so future
agents do not mistake the accepted limitation for an unhandled bug that requires
locks or hidden reservations.

Owner decision: long titles should produce a deterministic truncated slug rather
than an error. Keep slugs relatively short and branch-name-like; agents should be
encouraged to summarize or shorten verbose user input before passing it to
`sid new`.

Owner decision: the v1 maximum slug length is exactly 48 characters. This should
be named in the implementation rather than left as an unexplained test fixture.
Slug truncation tests should keep one exact golden string for deterministic v1
behavior and also assert properties such as length, determinism, prefix
relationship to the full slug, and no trailing dash.
Tests should also prove truncation happens after slugification, including a
multibyte/non-ASCII input case that still produces deterministic ASCII slug
output without byte-boundary issues.

Owner decision: task-id-shaped direct children in scan roots must be
directories; otherwise fail closed with a clear error. Do not inspect or reject
`sXXXX` text inside user-provided titles/slugs; those refs may be useful context
for follow-up tasks.

Owner decision: fail-closed scan behavior applies only to direct children shaped
like task ids with a recognized `sXXXX` ref segment. Period-prefixed names with
non-recognized ref segments, such as `202605_notaref_x`, are benign and should be
ignored rather than rejected.

Owner decision: direct-child directories shaped like `{YYYYMM}_{sXXXX}` reserve
that period/ref namespace even though generated ids include a slug. This is
intentional tolerance for hand-created or legacy task folders. Slugless tolerance
applies only to directories; task-shaped non-directories still fail closed.

Owner decision: task-id-shaped symlinks fail closed. The scan rule is simple:
task-shaped direct children must be real directories. Do not follow symlink
targets for this hardening pass.

Owner decision: add a small deterministic dependency seam for `sid new` period
and candidate tails now. Keep it plain data/helper functions, not traits or an
effects framework.

Owner decision: injected/planned periods are validated as six ASCII digits in
this pass. Calendar-valid month enforcement, such as rejecting `202600` or
`202613`, is deferred future policy and should be reconsidered when dependency
injection or config semantics mature.
Tests should cover malformed injected periods against both empty snapshots and
snapshots with populated `max_seq` data so validation cannot live in only one
planner branch.
The populated-snapshot test may remain white-box temporarily because it exposes
the current branch-specific bypass. When the deterministic dependency seam lands,
add a seam-level malformed-period test and then reassess whether direct
`ScanSnapshot` construction is still needed.

Owner decision: empty-slug CLI coverage should include punctuation-only,
whitespace-only, and Unicode-only titles. The v1 slug rule requires at least one
ASCII letter or digit after slugification.

Owner decision: numeric allocation bounds should be tested on both sides. For
this pass, prove `seq == 659` is still allocatable and `seq > 659` is exhausted.

Owner decision: scan snapshots are period-partitioned. Refs and max sequence
values discovered under one period must not block or advance allocation for
another period.

Owner decision: the same-seq occupied-ref planner test may remain white-box with
a clearly labeled artificial snapshot. Real scanned state advances `max_seq` and
would avoid the same-seq candidate path, so the artificial snapshot is the right
targeted test for "slug must not disambiguate an occupied ref."

Owner decision: `execute_new` should explicitly cover creating the task root
when it is absent. This keeps the execute contract clear: dry-run creates
nothing, execution creates the root and chosen task folder.

Owner decision: invalid-tail tests should include a seq-independent bad alphabet
case, such as a candidate tail containing a character outside `SLOP30`, in
addition to any digit-run-rule examples.

Owner decision: a reliable test for the lookup/read disappearing-root race
belongs with the scanner implementation seam. Do not add a flaky filesystem race
test; before declaring scan hardening complete, add a seam-level test where
`read_dir` returns `NotFound` after lookup and confirm other read errors still
fail closed.

Owner decision: per-entry scan races are not softened in v1. If the root is
readable but an individual task-shaped child disappears or changes before its
metadata is read, failing closed is acceptable; do not backtrack and reuse a seq
that may have been present moments earlier.

Owner decision: ADR drafting is related context, but separate from this
hardening code task. Preserve the ADR context in a handoff prompt under `tmp/`
so another agent can draft the decision records without blocking allocation
hardening.

Owner decision: successful command stdout should be JSON by default for v1, and
possibly the only supported success output format for now. Human
diagnostics/errors may still go to stderr. Defer human success output or
`--human` until real use shows it is needed. JSON is preferred over XML/HTML-ish
formats for CLI machine output; XML/Markdown may still be useful for
agent-instructions or prompts. This needs an ADR. Key rationale: if agents must
remember an extra flag to get the machine contract, the machine contract is not
the default path and the CLI is not truly agent-first. Popular CLIs with opt-in
JSON are useful precedent for JSON as a format, but they are not strong evidence
for an explicitly agent-first default because most were designed primarily for
human/operator use.

Owner decision: implement JSON-only success output for now. Remove the global
`--json` decision point from the user-facing CLI rather than keeping an unused
dual-output path. Preserve typed result structs; add `--human` later only if real
use shows a need.

Owner decision: JSON success output should assert the exact top-level key set.
For `sid new`, the v1 schema is `title`, `slug`, `period`, `sid_ref`, `id`,
`path`, and `dry_run`. For `sid agent-instructions`, the v1 schema is `format`
and `text`. New success-output fields require an intentional contract update.
CLI JSON tests should assert that `sid_ref` has a recognized `sXXXX` shape
without depending on the exact random ref value.

Owner decision: `agent-instructions` should follow the same JSON success-output
rule in v1, returning a wrapper such as `{ "format": "markdown", "text": "..." }`.
Direct raw document output can be reconsidered later with `--raw` if piping into
files becomes a real need.
The `"markdown"` format value should be backed by a named constant or enum rather
than an ad hoc producer literal.

Owner decision: errors/diagnostics stay human-readable on stderr for this
hardening pass. This matches the Unix-style channel split: stdout is composable,
pipeable, machine-actionable success output; stderr is diagnostic output.
Successful-command stderr is intentionally unspecified rather than asserted
empty. Failure stdout should be empty so agents do not consume partial success
data.
Until structured errors exist, tests may assert stable, meaningful diagnostic
substrings, but should avoid pinning whole prose or redundant wording.

Owner decision: add the output-default ADR in the same commit as the JSON-only
implementation and tests. Commit purity is not important on this branch; it will
likely be squashed before merging to main.

## Checklist

Implementation sequencing lives in `IMPLEMENTATION.md`. This file captures the
alignment decisions and constraints; the implementation file turns them into a
concrete next-agent work order.

- Tighten crate/module boundaries so internals are not public API by accident.
- Add a small period/tail dependency seam for `cmd_new`.
- Test monthly sequence boundaries on both sides: `max = 658` allocates
  `seq = 659`; `max = 659` exhausts the month.
- Test that scanned refs/max seq values are partitioned by period.
- Test occupied-ref namespace behavior: slug must not disambiguate an occupied
  ref.
- Test recognized-but-not-generated refs affect scanning and `max(seq)`.
- Test `execute_new` retries the next candidate on exact-path `AlreadyExists`.
- Test `execute_new` creates the missing task root and chosen task folder.
- Test punctuation-only, whitespace-only, and Unicode-only empty-slug errors
  through the CLI.
- Test invalid candidate tails using at least one character outside `SLOP30`.
- Implement the 48-character slug length policy for long titles.
- Test that slug truncation happens after slugification, including multibyte
  input.
- Validate injected/planned periods as six ASCII digits before producing ids.
- Test malformed injected periods against both empty and populated snapshots.
- Make non-directory direct children shaped like task ids fail closed with a
  clear error, including task-shaped symlinks.
- Ignore period-prefixed non-directories when the ref segment is not recognized.
- Treat slugless `{YYYYMM}_{sXXXX}` direct-child directories as namespace
  reservations.
- Make `scan_root` treat a concurrently missing root as empty while still
  failing closed on other scan errors; add this as a seam-level implementation
  test rather than a flaky filesystem race.
- Keep per-entry scan races fail-closed in v1.
- Document the accepted concurrent same-ref/different-slug race in a concise
  comment near `execute_new`.
- Make successful command stdout JSON by default and remove the current
  user-facing `--json` switch.
- Assert exact JSON success-output key sets for `sid new` and
  `sid agent-instructions`.
- Assert recognized `sXXXX` shape for `sid_ref` in CLI JSON without pinning the
  random value.
- Back the `agent-instructions` `"markdown"` format value with a named constant
  or enum.
- Add the output-default ADR alongside the JSON-only implementation.
- Remove the TODO-style `agent-instructions` source comment, or move the text to
  an embedded markdown file with the workforest `include_str!` pattern.
- Update `docs/contracts/0001-sid-new-dry-run.md` statuses.
- Run `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.

## Out Of Scope

- `.sid` config.
- Additional scan roots such as `.pending` and `.archive`.
- `sid list` or `sid id`.
- Separate `slop30` / refcode crate.
- Lock files or strong concurrent allocation guarantees.
- Full README / AGENTS docs, unless a tiny note becomes necessary while touching
  docs.
- Drafting ADRs, except for keeping the handoff prompt current.
