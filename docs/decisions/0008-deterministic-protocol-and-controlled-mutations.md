# 8. Deterministic Protocol And Controlled Mutations

Date: 2026-07-13
Status: Accepted

## Context

Tasks, parked seeds, explicit topics, raw notes, and task inboxes have distinct
identity, ownership, traversal, and mutation rules. Treating them as one
generic root registry would hide those differences and make compatibility
with the existing task allocator difficult to verify.

## Decision

Configuration uses purpose-named task, seed, note, topic, queue, ref, and relink
tables. Canonical discovery composes typed sources while keeping allocation and
list semantics explicit. Read/query commands return deterministic JSON and
preserve usable partial results with completeness findings. Mutation commands
expose bounded, inspectable state transitions; relink is preview-first and
applies independently verified files atomically. Raw capture and inbox queues
remain verbatim and outside relink.

Note capture writes at most one exclusive file and quarantines conservative
credential-pattern matches without echoing content. Seeds share allocation
identity but remain file-backed until graduation. Graduation preserves the
authored seed bytes through one atomic rename to `napkin.md`; it does not
fabricate a canonical task entrypoint or copy across filesystems.

Relink separates planning from execution. A plan retains scanned bytes and
non-overlapping destination spans. Write compares each file independently,
skips a whole raced file, applies offsets back-to-front, and atomically replaces
only that file while preserving permissions. This intentionally permits other
independently proven files to succeed. Queue artifacts remain verbatim because
their transient authored content is outside structural repair.

Relink carries two distinct write authorities over that one atomic boundary.
Global relink's authority is per-file: every file it can independently prove is
repaired, so a raced neighbour costs nothing. A move-projected relink adds a
whole-plan approval in front of the same per-file boundary, because its caller is
a lifecycle operation rather than a corpus janitor. Such a caller decides
"perform this move" once, and it cannot safely reason about a scoped repair by
comparing only the files that happened to survive a partial write. So a projected
write requires the exact digest of a fresh preview, refuses before opening the
first file when the plan changed or is incomplete, and only then hands the
approved plan to the existing independent per-file writer. The per-file partial
result is deliberately preserved after that point: it is forward-recoverable,
because already-future destinations settle on the next preview.

The digest is scoped to the move effect set — local links targeting the moving
owner plus local links authored inside it — rather than to the whole corpus. A
corpus-wide digest would make every unrelated authored edit invalidate a pending
close, which is friction with no safety return; an effect-set digest still
invalidates approval whenever a relevant link is added or changed, and keeps
coverage failures that could conceal a relevant link inside the approval.

Projected relink classifies every local CommonMark destination in its declared
authored-source coverage. Exactly-one-ref destinations retain identity-backed
resolution; ref-less and multi-ref destinations use lexical current/projected
candidate authority and fail closed on ambiguity or absence. Generic projected
changes carry `id:null` rather than inventing identity, and the expanded
authority is marked `sid-relink-move-v2` so a v1 digest cannot authorize it.

Markdown destination comparison is semantic, while replacement emission is
representation-aware. Raw authored text remains the `from` value. Bare
destinations preserve balanced parentheses and escape unmatched parentheses and
backslashes; angle destinations escape backslashes and angle delimiters. This
prevents valid authored escapes from looking like drift and prevents writes from
changing which destination CommonMark parses.

Projection reasons about where the authored bytes point, not about canonical
text. Comparing a destination's current and projected canonical spellings is not
sufficient to conclude a move leaves it alone: a move changes the depth of every
file inside the owner, so an authored path that walks above the owner boundary
and re-descends breaks while both canonical spellings stay byte-identical. The
unaffected test therefore requires that the authored path resolve to the
canonical target today and still land on that target's post-move location when
read from the source's future parent. The first half of that test is what keeps
drift failing closed rather than being waved through as unaffected.

Projected mode accepts a stable id plus a configured root, never an arbitrary
future filesystem path, so a caller cannot direct repairs at a location the
project does not already own. It is also why an owner already sitting at the
destination becomes settled verification instead of an error: proving scoped
destinations are canonical is exactly what a retry or a final close check needs.

Workspace rituals and relevance or filing judgment remain outside Slopid.

The ref table controls generation only. Its optional deny-prefix array replaces
the built-in `prude` policy exactly; omission uses that preset and an explicit
empty array allows all otherwise valid refs. Both task and seed allocation
resolve this boundary to one typed prefix list, while readers remain tolerant.

Relink destination extensions are a closed, default-off set in checked-in
project configuration rather than a permissive CLI mode. The initial
`colon-line` extension accommodates projects that intentionally use a terminal
positive-decimal editor locator while keeping ordinary Markdown destinations
literal by default. Resolution gives a real colon-suffixed target precedence,
falls back to the stripped base only when enabled, and preserves rather than
validates the locator. External destinations remain outside this mechanism.

## Consequences

Callers can distinguish canonical owners from queue artifacts without path
guessing. Existing task-only projects continue to work. Each new source or
mutation needs an explicit contract and synthetic standalone-project proof;
there is no generic source plugin mechanism or lifecycle/status model.
Projects using host-style line locators must opt in durably; unknown extension
names fail closed, and `sid init` does not silently enable compatibility.
