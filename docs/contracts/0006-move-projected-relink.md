# 0006 — Move-projected relink

Status: accepted.

Global `sid relink` answers "which Markdown destinations are wrong now?" and
repairs all of them. This contract adds a strictly narrower authority that
answers "which destinations would one exact planned owner move make wrong?", so
a lifecycle caller can approve and apply a scoped repair without also accepting
a corpus-wide rewrite.

This contract is self-contained. Contract
[0005](0005-deterministic-stm-protocol.md) keeps the global relink behavior
unchanged, including its exact four-key result.

## Command surface

    sid relink --move <ID> --into <CONFIGURED_ROOT>
    sid relink --move <ID> --into <CONFIGURED_ROOT> \
      --write --expected-plan-sha256 <64-lowercase-hex>

`--move` and `--into` must be supplied together. `--expected-plan-sha256`
requires both `--write` and `--move`, and a projected `--write` requires it. The
digest argument is exactly 64 lowercase ASCII hexadecimal characters; any other
length or case is an argument error, so an approval can never be silently
truncated or case-folded into a match. No other combination is valid. Plain
`sid relink` and `sid relink --write` are unaffected and still require no digest.

Projected mode is preview-first: without `--write` it never changes bytes or
mtimes.

## Move identity and destination root

`<ID>` is one exact, case-sensitive canonical frontmatter id that must resolve
uniquely to a folder-backed task or review owner whose entrypoint is
`CURRENT_STATE.md`. A seed, a topic, a duplicate id, and an unknown id are all
refused.

`<ROOT>` is resolved with the same component-aware, ambiguity-safe resolver as
`sid new --into`: it must match exactly one configured task root. A caller may
never supply an arbitrary future filesystem path. The projected owner is always
that configured root joined with the current owner directory's unchanged
basename.

If the projected owner differs from the current owner it must not already exist.
If it is identical to the current owner, the command is settled verification
(below).

## Authored-source coverage and move effect set

The effect set is not the global repair set. Projected mode classifies every
nonempty, nonexternal local CommonMark destination produced by Slopid's authored
source scanner. That scanner covers readable UTF-8 Markdown beneath canonical
task/review owners and configured topic roots plus direct seed files. It honors
project ignore rules and excludes VCS metadata, `tmp`, task `inbox` subtrees,
note roots, and their quarantine/done areas.

A destination is in the move effect set when its source base moves, its current
lexical target is under the moving owner, or its future-authored target is under
the projected owner. Destinations containing exactly one recognized ref retain
the stronger identity-backed resolution from contract 0005. Ref-less and
multi-ref local destinations use lexical filesystem authority instead of being
silently omitted.

Both sides of a link are projected. A source inside the moving owner is measured
from its future parent directory. A target whose id is the moving id resolves
under the future owner. Targets with other ids stay at their current canonical
locations.

For a recognized-ref destination, resolve the authored path lexically from the
source's current parent and from its future parent, and compute the canonical
current and projected semantic texts. Then, in order:

- the move cannot change this destination when **both** the authored path
  resolves to the canonical target today **and**, read from the source's future
  parent, it still lands on the post-move location of what it points at today.
  Such a destination is outside the change set — a link whose source and target
  move together with an unchanged relative path is therefore absent, and any
  ordinary normalization it still needs remains global relink's job;
- an authored destination equal to the current canonical text is planned for
  replacement with the projected canonical text;
- an authored destination already equal to the projected canonical text is
  settled and is never reversed merely because the folder has not moved yet;
- anything else yields `relink-projection-drift`, makes the result incomplete,
  and is not opportunistically normalized.

The first rule tests the **authored path**, not canonical-text equality, and both
of its conditions are load-bearing. A move changes the depth of every file inside
the owner, so an authored spelling that walks above the owner boundary and
re-descends breaks even when its current and projected canonical texts are
byte-identical; comparing only canonical text reports such a destination as
unaffected and leaves it broken after the move. Requiring that the authored path
resolve to the canonical target keeps drift failing closed: a relevant
destination nobody can account for must block the move even though the move
itself would not change it.

For any other local destination, projected mode computes:

1. the current candidate from the source's current parent;
2. the authored-after path from the source's future parent; and
3. the projected candidate by mapping authored-after back into the current
   namespace when it lies beneath the projected owner.

The current and projected candidates use the same existence and opt-in
`colon-line` rules as recognized refs. A valid current candidate is unchanged
when authored-after still names its projected location; otherwise Slopid plans
the canonical future-relative text. A valid projected-only candidate is an
already-forward retry and is never reversed. Two valid candidates naming
different referents are ambiguous and block. No valid candidate also blocks.

## CommonMark destination representation

Comparison uses the parser's decoded semantic destination. Wire `from` values
retain the raw authored bytes, and replacement `to` values are rendered in the
same destination form the author used.

Bare destinations retain balanced parentheses, escape unmatched parentheses,
and escape literal backslashes. Angle destinations treat parentheses as normal
content and escape literal backslashes and angle delimiters. Inline links,
images, and reference definitions follow the same rule. Relink never splices
decoded path text directly when CommonMark would reparse it as a different
destination.

The future owner is allowed not to exist, because relink runs before the
lifecycle rename. The current owner and the current internal target must exist.
Before the rename, projected mode never scans or writes anything under the future
owner. After it, the future owner *is* the current owner and is scanned normally;
see `Settled verification`.

If the projected owner differs from the current owner, the destination must not
already exist as any filesystem entry. This is deliberately tested with
`symlink_metadata` rather than `exists`, because `exists` follows symlinks: a
dangling symlink at the destination would otherwise pass the guard, the scoped
write would apply, and the caller's later rename would fail with every affected
link already rewritten to a location the move cannot reach.

## What `complete` does and does not promise

`complete:true` means every local destination in the declared authored-source
coverage was classified and this scoped plan is safe to apply. It covers
ref-less and multi-ref local paths as well as recognized-ref destinations.

It is not a certificate over Markdown-looking bytes outside that declared
coverage. Task `inbox`, note roots, `tmp`, VCS metadata, and ignored paths remain
outside Slopid's read and mutation authority. A caller must not describe
`complete:true` as proof over every Markdown file anywhere on disk.

## Settled verification

When the selected owner already lives under the selected root, the projection is
`settled:true` and `from_owner` equals `to_owner`. The same scoped union is
inspected: a canonical destination is settled, and a noncanonical one yields
`relink-projection-drift` with `complete:false`. Settled verification never
plans a move-caused change. This is what makes final close verification and a
retry after a lost response safe.

## Result shape

A projected result contains exactly `complete`, `applied`, `changes`,
`findings`, `projection`, and `plan_sha256`. `projection` contains exactly `id`,
absolute `from_owner`, absolute `to_owner`, and `settled`. Every change retains
exactly `path`, `line`, `column`, `id`, `from`, and `to`. Recognized-ref
projected changes carry the canonical string ID; generic path-authority changes
carry `id:null` because no stable target identity was proven. Global changes
continue to carry a string ID. The two projected-only keys are omitted entirely
from global results, which therefore still contain exactly four keys.

`complete` means the move-scoped scan and plan are safe to use. Unreadable
authored source coverage, an ambiguous or unresolved relevant target, a missing
relevant internal target, overlapping relevant spans, or projection drift make
it false. Candidate problems outside the effect set are omitted from this scoped
result, which is why a projected scan can be complete while a global scan of the
same corpus is not.

`applied:false` means preview or a pre-write refusal. `applied:true` means the
digest matched, the plan was complete, and the per-file apply phase ran.

## Plan digest

`plan_sha256` is the lowercase SHA-256 of the deterministic JSON serialization
of an internal authority object marked `sid-relink-move-v2`, containing the
contract marker, the projection, scoped completeness, the sorted effect sources,
the sorted changes, and the sorted findings. Each effect source is a readable
Markdown file holding at least one in-scope local destination — including a
generic, settled, missing, ambiguous, or drifted one — recorded with its path and
the SHA-256 of its entire scanned byte sequence. The v2 marker prevents any v1
approval from authorizing the stronger effect set.

The digest deliberately excludes the expected digest, `applied`, the transient
`relink-plan-changed` finding, timestamps, filesystem mtimes, and display prose.
So a new or changed relevant link invalidates approval, while an unrelated
authored edit outside the effect set does not create needless close friction.
Coverage findings that could conceal a relevant link stay inside the authority
and keep the plan incomplete.

Cross-machine digest equality is not promised. Preview and write must agree on
the same canonical project and platform.

## Write, refusal, and partial recovery

A projected write recomputes the whole plan once at command start. Before the
first file is opened for replacement:

- a differing expected digest returns `applied:false`, `complete:false`, no
  changes, the newly computed digest, and a transient `relink-plan-changed`
  finding;
- a recomputed plan that is incomplete returns its findings and digest with
  `applied:false` and no changes, even when the expected digest matches.

When the digest matches and the plan is complete, the approved plan goes to the
same per-file writer as global relink: each planned file's current bytes must
still equal the previewed bytes, replacements apply back to front, and a
same-directory atomic replacement preserves permissions. A raced file is skipped
whole with `relink-concurrent-change` and an atomic failure adds
`relink-write-failed`; either makes `complete:false` without rolling back an
independent successful file, and only applied changes are returned. A matching
complete zero-change plan is convergence: `applied:true`, `complete:true`, zero
changes.

Every projected write returns the approved pre-write digest, never a post-write
digest. A partial result is intentional and forward-recoverable: already-future
destinations settle on the next preview and only the remaining repair is
planned. The caller obtains the next state through a fresh preview.

## Findings and exit behavior

This contract adds exactly two stable wire values: `relink-projection-drift`, a
substantive part of the scoped plan, and `relink-plan-changed`, a transient apply
refusal describing the request rather than the corpus. Both are errors. Neither
participates in the shared `affects-completeness` or operational classifications
used by `lint`, because neither can appear in a document-index scan.

Every usable projected result exits 0 with JSON on stdout, including
`complete:false` previews, digest mismatches, incomplete-plan write refusals, and
post-digest per-file partial results. A caller must inspect both `complete` and
`applied`; process success alone is not aggregate success.

Invalid arguments or configuration, an invalid move identity, an invalid or
ambiguous destination root, an existing destination owner, and no usable authored
source remain nonzero with human stderr and empty stdout.
