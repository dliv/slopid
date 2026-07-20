# slop-id: goals vs implementation review

Date: 2026-06-10
Method: multi-agent review (9 dimensions, adversarial verification) + full manual
read of src/ by the orchestrator. The cloud fan-out was interrupted partway by a
monthly spend limit: the **refcode, config, output-slug, test-matrix, and
docs-drift** dimensions completed with verification; **allocation, stm-compat,
product-goals, and improvements** are covered by the orchestrator's own line-by-line
read of `src/commands/new.rs`, `src/bin/stm.rs`, and the rest of src/ instead.

> **Update 2026-06-11**: the interrupted portion was re-verified by a 40-agent
> fan-out — one empirical verifier plus one adversarial skeptic per item,
> covering findings 4–13, every orchestrator observation, and the three
> dimensions that never got agent coverage (allocation, stm-compat,
> product-goals). **All 20 items confirmed at high confidence; no verdict was
> overturned.** Items 8 and 10 were confirmed by actual mutation testing in a
> repo copy; behavioral claims (11, 12, O1–O3, the allocation and stm-compat
> bullets) were reproduced against the built binaries. Material refinements
> are folded in below, marked *[2026-06-11]*; three small gaps the original
> review missed are appended after the observations section.

## Verdict

**The implementation is correct against the documented goals.** No bugs and no
contract violations were found in the code. Everything actionable is either
documentation drift or small hardening/test-coverage gaps.

- Review gate (ROADMAP): `cargo fmt --check` clean, `cargo clippy --all-targets
  -- -D warnings` clean, all 61 tests pass (35 unit + 26 integration).
- `src/refcode.rs` was verified empirically against REF_DESIGN_HANDOFF.md:
  alphabets match in content and order; all encode_seq spec examples plus the
  660→None bound hold; a full 0..=659 encode/decode roundtrip is clean; the
  generated-tail rule is **exhaustively equivalent** (0 mismatches over the whole
  5-char space) to the union of the spec's two generated-ref regexes;
  `is_recognized_ref` exactly matches the recognized regex; FNV-1a was
  independently re-derived in python for 6 periods (202605→128, 202606→101,
  202607→102, 202512→161, 202601→132, 209912→10) and matches the code and the
  unit-test vectors; start range is exactly aa=8..ff=163.
- `src/commands/config.rs` was verified empirically against contract 0001 and
  the sea99 PLAN: defaults, root-only derivation, deny_unknown_fields,
  absolute/parent-dir/dangling-symlink fail-closed, `sid init` exact TOML and
  overwrite refusal, `{path, created}` JSON. Every config row in the contract's
  test matrix has a matching integration test.
- Allocation (`src/commands/new.rs`, manual read): faithful to the handoff
  algorithm — scan-then-create; direct-child scan; non-NotFound read errors fail
  closed; task-shaped non-dirs **and symlinks** fail closed (read_dir file_type
  does not follow symlinks); seq = max+1 with checked_add capped at 659 →
  "monthly sequence exhausted"; empty month → deterministic start; 100-tail
  attempt budget; digit rule enforced in `encode_ref`; occupancy keyed by ref
  only, so slug never disambiguates; `create_dir` AlreadyExists → next candidate,
  other errors fail; dry-run calls nothing that creates (not even the root);
  period-partitioned occupancy and max-seq. Matches contract 0001 scan policy
  line by line.
- Output/slug: `sid new` JSON has exactly title/slug/period/sid_ref/id/path/
  dry_run; agent-instructions exactly {format, text} with format from a named
  constant (ADR 0004); init exactly {path, created}; no `--json` on sid; errors
  to stderr with empty stdout, exit 1. Slug: ASCII, empty fails with a clear
  error, truncate-to-48-then-trim-dashes implemented and unit-tested (including
  multibyte input).
- stm compat (`src/bin/stm.rs`, manual read vs contract 0002): all commands and
  options present including hidden `--locked-root` overriding `--root`
  (`cli.locked_root.or(args.root)`); multi-word titles via `num_args 1..`; plain
  output name-vs-path; `--json` exactly {name, path, root, created}; scans root
  + .pending + .archive, skips missing roots; new creates root+folder, dry-run
  creates neither; default entropy 4 → 5-char refs that sid recognizes, and
  stm's digit-rule-by-construction tails are valid generated 1134 refs, so the
  two binaries interoperate cleanly on one tree at defaults.

## Confirmed findings (survived adversarial verification)

1. **Symlinked scan-root policy is implemented but never recorded**
   (improvement, docs). PROJECT_HANDOFF explicitly lists "Should symlinked scan
   roots be followed, rejected, or treated as ordinary paths?" as a question to
   settle; neither contract 0001 nor the sea99 PLAN records the answer. The
   implementation follows symlinked roots, and — verified empirically — a
   **dangling** symlinked scan root hits the NotFound branch
   (src/commands/new.rs:135) and silently becomes an empty snapshot, which can
   permit ref reuse and is asymmetric with the fail-closed dangling `.sid`
   symlink policy. Fix: add one line to contract 0001's Scan Policy stating the
   chosen behavior + a boundary test for the dangling-root case (or fail closed
   on symlinked roots to match the child-symlink policy).

2. **`<human>` annotation in PROJECT_HANDOFF.md:98 is now resolved but unmarked**
   (nit). The "maybe json should be the default" question was decided exactly
   that way by ADR 0004, and the surrounding text (lines 95, 148) still
   describes a `--json` flag ADR 0004 removed. A fresh agent told to "start a
   focused Q&A" may reopen the settled question. Fix: one-line "(resolved by
   ADR 0004)" marker. The REF_DESIGN_HANDOFF.md:479 annotation is completed
   process guidance and can stay.

3. **Contract 0002 header is nonstandard** (nit): no `Date:` line (would be
   2026-06-05 per git) and `Status: active dogfood contract` vs the uniform
   `Status: Active/Accepted` elsewhere. Also consider an `Updated:` line on
   contract 0001/ROADMAP (materially amended in June) or drop Date fields in
   favor of git history.

## Confirmed findings, second batch (verification was cut off by the spend limit on 2026-06-10; completed 2026-06-11 — every item below confirmed empirically and upheld against an adversarial skeptic)

Doc drift:

4. **ADR 0004 is contradicted by the stm binary with no superseding record**
   (doc-drift). ADR 0004: "JSON is the only supported success output format.
   Remove the user-facing `--json` switch." Commit 57dc1b4 shipped stm with
   plain-output default and an opt-in `--json`. Contract 0002 carves this out
   ("`sid` remains the JSON-first CLI"), but ADR_INDEX's own rule says to reopen
   decisions with a superseding record. Fix: short ADR 0005 ("plain-output stm
   compatibility binary") or an Amended-by note on ADR 0004 scoping it to sid.

5. **Roadmap doesn't reflect reality** (doc-drift). Milestone 2 (config/scan
   roots) is complete (commit 30774ef, sea99 checklist fully checked) but has no
   "Done" marker; the stm binary was milestone-4-area work shipped out of order
   with no stm/ task folder (breaking the project's own "each task gets a
   PLAN.md" convention), recorded only as a paragraph inside milestone 4. Fix:
   mark milestone 2 Done, add a one-line "done early" note for stm, and archive
   `stm/202605_sea99_...` (stm/README.md still lists it as Active).

6. **Contract 0001 "Requirements To Settle Soon" is stale** (doc-drift). The
   calendar-valid-month question is already answered "deferred" in the same
   document's First Defaults and in the roadmap's Deferred By Default list. The
   `sid id` question now has prior art in `stm id` (contract 0002) that the
   section doesn't cross-reference.

Hardening / tests (all small):

7. **Slug boundary not tested at exactly 48/49** (nit). AGENTS.md requires both
   sides of every numeric boundary; current tests use 87- and 68-char inputs and
   a 47+dash case. Add: `slugify("a"*48) == "a"*48` and `slugify("a"*49) == "a"*48`.

8. **No test ever populates `.pending`** (improvement). Verified: every
   `create_exhaustion_entries` call seeds `.archive` only (tests/cli_test.rs:261,
   283, 305, 701). Dropping `.pending` from `task_roots` (stm.rs:360) or from
   `DEFAULT_SCAN_ROOT_NAMES` handling would pass the whole suite. The test named
   `stm_scans_pending_and_archive_roots` overstates its coverage. Fix: seed the
   max-seq entry in `.pending` in at least one sid test and one stm test.
   *[2026-06-11] Confirmed by mutation testing in a repo copy: removing
   `.pending` from `task_roots` (stm) or filtering it out of
   `default_scan_roots_for` (sid) passes the entire suite. (Removing it from
   the `DEFAULT_SCAN_ROOT_NAMES` constant itself trips only
   `init_writes_default_config_to_cwd`'s TOML string assertion — config-text
   coverage, not scan coverage.) Both binaries do scan `.pending` at runtime:
   seeding a max-seq entry only there drives both to "monthly sequence
   exhausted".*

9. **sid integration tests depend on the wall clock** (improvement). sid has no
   period override (`current_period()` is unconditional in `cmd_new`), so the
   namespace tests probe the live month via `dry_run_period` and hedge rollover
   with `next_period` entries — a timing mitigation, not the seam AGENTS.md asks
   for. stm already has `--month`. Fix: hidden `--period` flag or `SID_PERIOD`
   env override on sid new, then delete the hedge machinery. This also makes the
   deterministic-start and 659/660 boundaries end-to-end testable for sid.
   *[2026-06-11] One refinement: the 660-exhaustion side is already exercised
   end-to-end (cli_test.rs:272, 294, 316), though only via the live-month
   probe+hedge; what the seam newly enables end-to-end is the exact
   deterministic-start ref value and the 659 final-slot success, plus
   determinism throughout.*

10. **ALPHA22_CHARS/SLOP30_CHARS arrays aren't test-locked to the strings**
    (nit). encode uses the arrays, decode/recognition use the strings; the
    alphabet test asserts only the strings, and example tests sample edge
    indices. A mid-array swap would pass the suite. Fix: assert
    `ALPHA22.chars().eq(ALPHA22_CHARS)` (and SLOP30), or a 0..=659 roundtrip
    test, or derive one form from the other.
    *[2026-06-11] Confirmed by mutation testing in a repo copy: swapping
    ALPHA22_CHARS[10]/[11] and (separately) SLOP30_CHARS[16]/[17] each passed
    all 61 tests while making encode and decode mutually inconsistent — a real
    latent allocation-accounting bug class. The strings are the spec-anchored
    form (REF_DESIGN_HANDOFF.md defines the alphabets as strings); the arrays
    are the unlocked duplicate.*

11. **Degenerate config roots accepted** (improvement). `root = ""`, `"."`, and
    whitespace-only pass `validate_relative_path`; `""`/`"."` make the project
    directory itself the task root (verified empirically by the config agent)
    and `"."` leaks a literal `/./` into the JSON `path`. Not dangerous, but a
    task-id-shaped file at project top level would then fail allocation closed.
    Fix: reject empty/CurDir-only paths in `validate_relative_path`.

12. **`sid init` on a dangling `.sid` symlink says "project config already
    exists"** (nit). create_new → EEXIST is the right fail-closed behavior, but
    the message misleads (`cat .sid` finds nothing). Fix: special-case the
    message via symlink_metadata.

13. **`sid new --help` calls the slug "optional"** (nit, cli.rs:20). For
    generation the slug is mandatory (empty slugs fail per contract 0001); the
    `[_{slug}]` optionality applies only to *recognition* of existing folders.
    Reword the help text.

## Orchestrator's additional observations (verified 2026-06-11; all confirmed)

- **stm vs sid scan-policy divergence**: a task-shaped non-directory makes sid
  fail closed but stm silently skips it (stm.rs:329). Contract 0002 doesn't
  specify, so it's not a violation, but if both binaries dogfood one tree the
  asymmetry is worth a line in contract 0002. *[2026-06-11] The asymmetry is
  broader than files: stm.rs:329 discards all non-directory children before
  name parsing, and `DirEntry::file_type()` doesn't follow symlinks, so stm
  also silently skips task-shaped symlinks-to-directories, which sid fails
  closed on.* *[2026-06-12] Resolved by owner decision: both binaries now
  treat task-shaped non-directories as best-effort namespace reservations —
  the owner zips task folders as exports, and unexpected entries must never
  hard-break the tool. The divergence no longer exists.*
- **Non-default `--entropy-chars`** creates refs sid's scanner ignores (it
  recognizes exactly 5 chars), so sid's max-seq won't see them; same-month sort
  interleaving can then drift, though exact ref collisions remain impossible
  (different lengths). Fine at defaults; worth a sentence in contract 0002 if
  non-default entropy is ever used against a sid-managed tree.
- **stm validates `--month` to 01–12** while sid defers calendar validation —
  intentional-looking compat behavior, just unrecorded.
- **stm tail distribution** for letter-seq_lo forces exactly one digit position
  rather than rejection-sampling SLOP30² (stm.rs:425) — a slightly different
  distribution than the handoff's generation rule. Cosmetic; refs produced are
  all valid generated 1134 refs. *[2026-06-11] Clarification: "one digit
  position" describes the mechanism, not the result — the unforced position
  draws from full SLOP30 (which contains the digits), so ~27% of letter-seq_lo
  tails contain two digits (measured 0.2665 over 2000 samples).*
- **Error-message nit** (new.rs:199): "after 100 attempts" is reported even when
  the planner was given fewer injected tails — only observable through the test
  seam.
- **Duplication**: `parse_task_folder_name` and the scan loop exist in both
  new.rs and stm.rs. Sharing the scanner would also be the natural prep for
  milestone 3 (`sid list`/`find`), which needs exactly that direct-child scan as
  a reusable read path. This is the one refactor I'd do *before* starting
  milestone 3.
- **agent-instructions text** is minimal (no `rg sXXXX` search guidance, no
  collision-disambiguation advice from the handoff's "Agent-facing model") — but
  ROADMAP milestone 4 explicitly defers the fuller embedded version, so this is
  planned, not drift.

## Added by the 2026-06-11 verification pass (gaps the original review missed)

All three surfaced by the product-goals completeness audit; all doc-class, none
a bug or contract violation, all reproduced empirically:

- **stm ignores `.sid` project config entirely** (stm.rs:193–198 — its root
  comes only from `--locked-root`/`--root`/cwd-join-`stm`). In a project with
  `.sid` root="tasks", sid and stm each allocated seq 101 for 202606 in
  disjoint namespaces under one project. Implied by contract 0002's "`sid`
  remains the project-configured … CLI" but never listed among the stm/sid
  divergences; worth one line in contract 0002.
- **No upward `.sid` discovery**: sid run from a subdirectory of a configured
  project silently starts a fresh `stm/` namespace there. Conforms to contract
  0001 ("loads … from the current working directory") so not a violation, but
  it is the most realistic agent-workflow way to defeat the one-namespace
  intent of goal 5, and no document records the decision. *[2026-06-12]
  Implemented by owner decision: discovery now walks up to the filesystem
  root, nearest `.sid` wins, and configured paths resolve against the config's
  directory (contract 0001 updated).*
- **`stm --entropy-chars 1` abandons sequence allocation entirely**
  (stm.rs:299–302 early-returns prefix+random char before any next_seq call):
  no monthly seq, no sort order, uniqueness only by exact name at the active
  root. Non-default and unspecified by contract 0002; micro.
- (Nuance, not a gap:) **`--locked-root` is a top-level argument, not
  `global`** — `stm new --locked-root …` is rejected; only
  `stm --locked-root … new …` works. Contract 0002 doesn't specify placement,
  so compliant, just invisible from the review text.

## Explicitly rejected (so future reviews don't re-raise it)

- "Rename `stm/.archive/202606_python-temp-allocator` to add a ref": refuted.
  stm/README already explains the folder as a pre-dogfood Python bridge, and a
  retroactive ref would falsify history and consume a slot in the live 202606
  namespace.

## Suggested order of attack

1. Doc sweep (done 2026-06-11, incl. ADR 0005): mark milestone 2 Done + stm note (5), archive
   sea99 + update stm/README (5), ADR 0005 or amend ADR 0004 (4), record
   symlinked-root policy in contract 0001 (1), prune stale "Requirements To
   Settle Soon" (6), resolved-marker in PROJECT_HANDOFF (2), contract 0002
   header (3), plus contract 0002 one-liners for the 2026-06-11 additions
   (stm ignores `.sid`; no upward `.sid` discovery; scan-policy/entropy
   divergences).
2. Test tightening (done 2026-06-11): 48/49 slug boundary (7), seed `.pending`
   (8), alphabet array lock + 0..=659 roundtrip (10).
3. Small code changes, TDD per AGENTS.md (done 2026-06-11): hidden `--period`
   seam for sid + hedge machinery deleted (9), reject degenerate roots (11),
   dangling-symlink init message (12) and boundary tests for dangling/followed
   symlinked scan roots (1), help-text reword (13).
4. Before milestone 3 (done 2026-06-11): shared direct-child scanner extracted
   to `src/scan.rs` and both binaries refactored onto it; `sid list` itself
   shipped 2026-06-12 (milestone 3, contract 0003, task sdd4c) together with
   `sid new --into`. One deliberate observable change: stm's
   unreadable-scan-root error context unified from "scan STM root" to the
   shared "scan task root" (diagnostic text is not contracted).

Also done outside the review scope (owner requests): `.prs` joined
`.pending`/`.archive` in the default scan roots of both binaries
(2026-06-11), and `.slow` joined them for slow-burn task folders
(2026-06-12) — each with per-root decisive tests (contracts 0001 and 0002
updated both times).
