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
`colon-line` rules as recognized refs, including that a separator-ended base is
not a locator and the whole spelling remains literal. A valid current candidate is unchanged
when authored-after still names its projected location; otherwise Slopid plans
the canonical future-relative text. A valid projected-only candidate is an
already-forward retry and is never reversed. Two valid candidates naming
different referents are ambiguous and block. When the raw current and projected
presence states are both `Absent`, the destination instead emits
`relink-unresolved-local-destination` with `severity:warning`, proposes no
replacement, and keeps the plan complete. This decision is made from raw
presence, never from validity booleans: a projected candidate can be present but
invalid because the authored-after path does not survive projection, and that
state remains blocking drift. Unknown filesystem state also remains blocking.

## CommonMark destination representation

Comparison uses the parser's decoded semantic destination. Wire `from` values
retain the raw authored bytes, and replacement `to` values are rendered in the
same destination form the author used.

Three properties are preconditions of any destination becoming digest or write
authority, not checks applied afterwards.

**One proven raw span.** The parser reports the whole construct, and a legal
title or link label may contain the same `](` or `]:` delimiter a destination
follows. Delimiter position is therefore never mutation authority on its own.
Every delimiter occurrence inside the construct — excluding those belonging to a
nested construct the parser reports separately — yields a candidate raw range,
and byte-identical ranges are deduplicated by position.

Each candidate is decoded independently in the grammar it was authored in: an
inline candidate inside a minimal inline link, a reference-definition candidate
inside a minimal definition. The inline wrapper ends its destination with
whitespace rather than the closing parenthesis, which is what lets it express a
destination ending in a literal backslash — inside `[x](note\)` that backslash
would escape the parenthesis and nothing would parse. Because of that wrapper the
two grammars accept the same destinations in practice; verifying in the authored
grammar is defence in depth and the more faithful question to ask, not a
behavioural requirement.

Whitespace **anywhere** in a candidate is refused outright in bare form, using
CommonMark's whitespace set rather than a host language's ASCII notion of it —
notably including line tabulation (`U+000B`). Whitespace would either merge with the
wrapper's own delimiter or turn the remainder into a title, and the parser would
then report a destination *shorter* than the candidate, which is exactly the false
claim these rules exist to prevent. Checking only the candidate's edges was not
enough: a trailing `()` reads as an **empty** title, which a nonempty-title check
cannot reject. A bare destination cannot contain whitespace at all, so refusing it
loses nothing.

A candidate is accepted only when that wrapper reads as exactly one construct
whose destination equals the parser's own destination, **with no title and no
leftover bytes**. Matching the decoded destination alone is not sufficient: for
candidate bytes of `DEST "TITLE"` the wrapper is a single legal link whose
destination really is `DEST`, so a destination-only comparison would accept a
range covering the title as well and splice over authored text.

Exactly one accepted candidate is success. Zero accepted candidates, or two or
more accepted candidates at different positions, fail closed: the destination is
reported and completeness is lost rather than a span being guessed.

**One proven round trip.** Rendering starts from canonical decoded text and is a
serializer, not a punctuation escaper. Bare destinations retain balanced
parentheses, escape unmatched parentheses, escape literal backslashes and angle
delimiters, encode literal `&` as `&amp;`, and encode spaces and ASCII control
characters as numeric character references. Angle destinations treat parentheses
as normal content, retain permitted spaces, escape literal backslashes and angle
delimiters, encode literal `&` as `&amp;`, and encode ASCII control characters.
Inline links, images, and reference definitions follow the same rule. The
rendered bytes are then reparsed and must yield exactly one destination equal to
the requested semantic value.

Rendering failure is defence in depth rather than a routinely reached branch: the
encoding above covers every character class a parsed path can contain, so in
practice only a value no CommonMark spelling can express — a NUL, which the
parser would already have replaced — is refused. When it does refuse, the
destination produces no replacement at all.

**One proven file.** The two properties above prove a replacement in isolation.
They cannot prove that the replacement leaves the *rest* of the file parsing as
the plan assumed. A parenthesis this renderer legitimately emits can balance an
earlier malformed construct and make it swallow the very link being repaired,
leaving a different destination behind while every local check passed. So before
a file's replacements become digest or write authority, they are spliced into a
scratch copy and the result is re-scanned: it must hold exactly the destinations
the plan intended — same count, same order, each replaced one at its new semantic
value, each untouched one unchanged, and no new scan uncertainty. A file that
fails this proof contributes `unreadable-entry`, no changes, and
`complete:false`; other files are unaffected. Planning and apply share one splice
implementation so the bytes verified are the bytes written.

These properties exist because the parser decodes character references before
Slopid sees a semantic path, so emitting decoded text is not an inverse
operation, and because a destination's bytes interact with the markup around
them. A raw space ends a bare destination and the construct stops being a link; a
raw `&` can be re-read as a named entity and retarget the link at a different
file; a parenthesis can re-delimit a neighbour. Relink never splices decoded path
text directly when CommonMark would reparse it as a different destination.

A destination inside this command's authority whose raw span or rendered form
cannot be proven yields `unreadable-entry`, makes the result incomplete, binds
its source into the plan digest, and contributes no change. A destination
outside that authority is irrelevant whether or not it could be located: global
repair covers only single-recognized-ref destinations, and projected mode covers
only the move effect set. This keeps parser uncertainty from either becoming
false success or widening authority.

The future owner is allowed not to exist, because relink runs before the
lifecycle rename. The current owner and the current internal target must exist,
and "exist" means proven: a preserved internal target that resolves is present,
one whose inspection reports `NotFound` is absent, and any other inspection
error — a permission failure, a symlink loop — leaves its state unproven. An
unproven target is a coverage failure that yields `unreadable-entry` and
`complete:false`, never the claim that the target does not exist.
Before the rename, projected mode never scans or writes anything under the future
owner. After it, the future owner *is* the current owner and is scanned normally;
see `Settled verification`.

If the projected owner differs from the current owner, the destination must not
already exist as any filesystem entry. This is deliberately tested with
`symlink_metadata` rather than `exists`, because `exists` follows symlinks: a
dangling symlink at the destination would otherwise pass the guard, the scoped
write would apply, and the caller's later rename would fail with every affected
link already rewritten to a location the move cannot reach.

That inspection is classified explicitly, because only `NotFound` proves the
projected owner absent. Any successful inspection is a collision. Every other
error — a permission failure, an unreadable parent, any other I/O error — leaves
the destination's state unknown and is a command-level refusal with nonzero
status, human stderr, and empty stdout, before any result JSON or authored
write. Treating unknown state as absence let a scoped write rewrite every
inbound link before a rename that could not succeed.

## What `complete` does and does not promise

`complete:true` means every move-caused repair in the declared authored-source
coverage was classified and this scoped plan is safe to apply. It covers
ref-less and multi-ref local paths as well as recognized-ref destinations, and
it requires that every in-scope destination had one proven raw span and that
every proposed replacement passed its round-trip proof. It is not a certificate
that every local destination resolves: a generic destination proven absent
under both readings remains an operator-visible warning without creating or
authorizing replacement bytes or making the plan incomplete.

It is not a certificate over Markdown-looking bytes outside that declared
coverage. Task `inbox`, note roots, `tmp`, VCS metadata, and ignored paths remain
outside Slopid's read and mutation authority. A caller must not describe
`complete:true` as proof over every Markdown file anywhere on disk.

`complete:true` is also not a statement about concurrency. It describes the
corpus Slopid read, not the corpus at the moment a later rename happens; see
`Authored-writer quiescence`.

## Settled verification

When the selected owner already lives under the selected root, the projection is
`settled:true` and `from_owner` equals `to_owner`. The same scoped union is
inspected under the split rule below. Settled verification never plans a
move-caused change. Recognized-ref canonical drift, ambiguity, unknown state,
or unsafe representation still yields an error with `complete:false`; proven
generic absence retains the warning and leaves completeness true. This is what
makes final close verification and a retry after a lost response safe.

Canonicality is required only where Slopid can produce it. A destination carrying
exactly one recognized ref must be canonical, because global relink normalizes
those. A generic ref-less or multi-ref destination is verified by whether it
**resolves**: one that resolves to an existing target under the settled owner is
clean even when its spelling is noncanonical. One proven absent yields
`relink-unresolved-local-destination` with `severity:warning`; ambiguity or
unproven inspection still blocks.

The asymmetry is deliberate and was chosen after the symmetric rule proved
unsatisfiable. Slopid normalizes generic destinations in no mode, so requiring
canonical text for them demanded a spelling the tool cannot write. Ordinary
authored forms such as `./x.md` and `dir/` therefore had no repair path, and
because settled verification runs only once the owner has moved, the refusal landed
*after* the caller's irreversible rename while the pre-move preview still reported
`complete:true`. Treating proven absence as a warning keeps verification focused
on the bound move outcome without claiming corpus health or demanding repair
authority this command does not have.

This narrows a spelling requirement, not a safety one. A spelling that genuinely
breaks because the move changes the owner's depth is still caught *before* the move
by the authored-path rule above.

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
authored source coverage, ambiguity, an unresolved recognized ref, a missing
recognized-ref internal target, overlapping relevant spans, unknown inspection,
or actual projection drift make it false. Proven both-absent generic
non-resolution is the explicit warning exception. Candidate problems outside the
effect set are omitted from this scoped result, which is why a projected scan can
be complete while a global scan of the same corpus is not.

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

An unresolved-local warning is substantive authority. Removing it or changing
any serialized field changes the digest independently of source hashes, and
changing any byte in its warning-bearing source changes the digest independently
of warning fields. If a target disappears after preview, the new warning and
source authority produce a different plan and the stale write refuses before
authored mutation.

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
changes. This includes warning-only plans, which retain their findings in the
apply result.

Every projected write returns the approved pre-write digest, never a post-write
digest. A partial result is intentional and forward-recoverable: already-future
destinations settle on the next preview and only the remaining repair is
planned. The caller obtains the next state through a fresh preview.

## Authored-writer quiescence

The caller must keep authored Markdown writers quiescent for the whole interval
from an approved projected `--write` through the terminal owner rename it
performs afterwards. This is a precondition of the operation, stated here so a
caller can meet it; Slopid does not enforce it.

Slopid does not detect, discover, or lease writers. It has no editor discovery,
no process or session registry, no lock file, and no compare-and-swap. The
per-file byte comparison is a useful early-race check and nothing more: it
detects a change that is already visible when the comparison runs, and skips that
file whole with `relink-concurrent-change`.

It is not a compare-and-swap. The comparison, the permission read, the output
construction, and the atomic replacement are separate steps, so an authored edit
that lands after the comparison and before the replacement is overwritten
without a finding. Atomic replacement prevents a torn file, not a lost
concurrent edit. That narrow window is an accepted residual of this version, and
no part of this contract may be described as closing it.

A caller that cannot guarantee quiescence should not treat a projected write and
a subsequent rename as safe. If an unexpected writer may have run, the recovery
is a fresh preview against current bytes; the previous byte comparison is not
evidence about a write that could have landed after it.

## Findings and exit behavior

This contract adds three stable wire values. `relink-projection-drift` is a
substantive scoped error, `relink-plan-changed` is a transient apply-refusal
error, and `relink-unresolved-local-destination` is a substantive
`severity:warning` finding for raw both-absent generic state. None participates
in the shared `affects-completeness` or operational classifications used by
`lint`, because none can appear in a document-index scan.

An aggregate close caller must display the warning and keep it in approval
authority. If confirmed cleanup intentionally retires the warning's exact
forest, sandbox, runtime, or other owned target, the aggregate destructive
preview binds that meaning; settled verification must not rediscover it as a
late terminal failure. Slopid itself carries no lifecycle or deletion policy.

Every usable projected result exits 0 with JSON on stdout, including
`complete:false` previews, digest mismatches, incomplete-plan write refusals, and
post-digest per-file partial results. A caller must inspect both `complete` and
`applied`; process success alone is not aggregate success.

Invalid arguments or configuration, an invalid move identity, an invalid or
ambiguous destination root, an existing destination owner, and no usable authored
source remain nonzero with human stderr and empty stdout.
