# Contract 0005: deterministic STM protocol

Date: 2026-07-13
Status: Implemented

## Compatibility and typed sources

Contract 0004 remains the historical task/review reader slice. The composed
canonical index now reads three purpose-named source kinds: direct child task
and review folders with `CURRENT_STATE.md`, direct Markdown files in the seed
root, and recursive Markdown files in explicit topic roots. Their supported
frontmatter types are respectively `task|review`, `seed`, and `topic`.
Canonical envelopes remain exactly absolute `path` plus the complete
JSON-compatible `frontmatter` mapping.

The project config accepts these tables and defaults:

    [task]
    root = "stm"
    scan_roots = ["stm/.pending", "stm/.prs", "stm/.slow", "stm/.archive"]

    [seed]
    root = "stm/.seeds"

    [note]
    root = "stm/.notes"

    [topic]
    roots = []

    [queue]
    stale_after_days = 7

When omitted, seed and note roots are `.seeds` and `.notes` beneath the
effective task root; topics are never implicit. All configured paths are
project-relative and reject absolute, parent, empty, current-directory, and
unknown settings. Missing typed roots are successful empty sources. Existing
task-only configs remain valid.

An optional `[ref].deny_prefixes` array is an exact replacement for the
built-in `prude` policy. Omitting it uses `prude`; setting it to `[]` allows
every otherwise valid generated ref. Contract 0001 owns its validation and
allocation semantics.

An optional relink destination compatibility list is:

    [relink]
    destination_extensions = ["colon-line"]

The list is a closed set of kebab-case names. Unknown names fail config parsing.
Omitting `[relink]`, omitting `destination_extensions`, or setting it to `[]`
are equivalent. `sid init` continues to omit this default-off optional table.

`sid resolve`, `sid graph`, and `sid lint` use the composed canonical index.
Task/review nodes retain Contract 0004 ordering. They sort before file-backed
nodes; seed/topic nodes sort by timestamp, id, and path. The seed filename's
embedded ref must match its canonical id. Topic identity is explicit
frontmatter only.

`sid list`, ordinary `sid new`, and `new --into` retain their existing task-root
results. Seed files share allocation ref occupancy but are neither list entries
nor allocation destinations. Ordinary task and seed allocation share the
generated-prefix policy in Contract 0001.

## Deterministic read/query commands

`sid search <TERM>... [--limit N]` requires one or more case-insensitive
literal terms and a positive limit (default 20). Terms are ANDed at whole-file
scope. Search covers UTF-8 text recursively beneath task and explicit topic
roots, direct seed files, and top-level pending notes. It honors project
`.gitignore`/`.ignore` files without global user ignores, stops inherited
rules at the nearest repository boundary, and excludes VCS metadata, `tmp`,
task `inbox`, note `quarantine`, and note `done`.

Success contains exactly `complete`, uncapped owner `total`, capped `results`,
and `findings`. A result contains exactly `owner_kind`, `path`, nullable
`node`, `rank`, `match_count`, and `excerpts`. Owner kinds are `canonical`,
`note`, and `unindexed`; ranks are `id`, `title`, `metadata`, `path`, and
`body` in that priority. Results then sort by matching-file count descending,
match count descending, and path. Match counts are uncapped non-overlapping
occurrences in matching UTF-8 text and searchable paths. Each result retains
at most three excerpts, sorted by path and line. Excerpts contain exactly
`path`, nullable `line`/`text`, and `truncated`; text is capped at 240 Unicode
scalar values, elision markers included, and windows onto the line's first
matching term: a term that fits inside the cap is shown whole, and a longer one
is shown from its start. Path-only excerpts have null line/text. Invalid UTF-8 is
non-text. Read failures produce partial exit-0 results; no usable configured
source fails with empty stdout.

`sid context <ID>` accepts exactly one folder-backed task/review id. Success
contains exactly `complete`, `node`, `graph`, and `inbox`; `graph` is the bare
graph result. Inbox contains exactly `complete`, `messages`, and `findings`.
Only top-level `inbox/*.md` is read. Each valid message contains exactly its
absolute `path` and complete `frontmatter`; bodies and `done/` are excluded.
Required nonempty strings are `from`, real `YYYY-MM-DD` `date`, and `subject`.
Messages sort oldest date then path. Malformed or unreadable individual
messages keep usable context with partial completeness. Topic/seed anchors fail
with empty stdout.

`sid captures` returns exactly `complete`, `notes`, `seeds`, and `findings`.
Notes contain exactly absolute `path`, UTC RFC3339 second-precision `modified`,
and `bytes`, sorted newest mtime then path; content is never returned. Seeds
are canonical nodes sorted newest timestamp then id/path. Missing roots are
complete and empty. Unreadable notes or malformed seeds make the usable result
partial. Direct `log.md` beneath the note root is reserved for the filing
ledger and is excluded from both capture inventory and literal search; it is
not a pending capture.

The default stale threshold is seven UTC calendar days; zero disables stale
warnings. Age six is clear and age seven warns. `stale-inbox-message` and
`stale-capture-note` are warning-only. `invalid-inbox-envelope` is a data
error. `unreadable-inbox-message` and `unreadable-capture-note` are operational
for lint: lint exits 2 with empty stdout. Context and captures keep independent
partial exit-0 envelopes for those individual failures.

## Controlled capture and seed mutations

`sid note [TEXT]` captures at most one identity-free UTF-8 note. An argument
wins; otherwise non-TTY stdin is read to EOF, while TTY stdin opens `$VISUAL`,
then `$EDITOR`, then `vi`. Empty input creates nothing and returns exactly
`{"state":"cancelled","path":null}`. Editor failure creates no note and
leaves a named recovery file. Success contains exactly `state` and absolute
`path`; states are `pending` and `quarantined`, and content is never echoed.

The exclusive filename is UTC
`YYYYMMDDTHHMMSS.ffffffZ_<8-lowercase-hex>.md`; collisions retry up to 100
times. Suspected secrets place the complete note beneath `quarantine/`.
The fixed floor covers PEM private-key headers, AWS `AKIA` ids, GitHub-style
tokens, `sk-` tokens, and recognized JSON/YAML/shell credential assignments
after `_`/`-`/`.` key normalization. Empty, null, redacted, changeme, example,
environment-placeholder, and angle-placeholder values do not trigger.

`sid seed <TITLE> [--origin ID]... [--edit] [--dry-run]` creates a flat seed
file in the shared allocation namespace and returns the unchanged allocation
fields `title`, `slug`, `period`, `sid_ref`, `id`, `path`, and `dry_run`.
Title-only birth is valid. Non-TTY stdin supplies an optional body unless
`--edit` explicitly selects the editor. Dry-run validates title and exact
origins without consuming input, opening an editor, creating roots, or writing.
Duplicate origins fail. A real write uses same-directory no-clobber persistence.
Seed ref generation uses the same resolved deny-prefix list as ordinary
`sid new`.

The seed filename is `{YYYYMM}_{ref}_{slug}.md`; canonical `id` is the short
ref. JSON-compatible frontmatter contains `type`, `id`, `title`, current local
date `timestamp`, and ordered `origin` only when present. Origins also emit one
ID-led reason line each under `## Related Work`; arbitrary body bytes follow.

`sid new --from-seed <ID> [--into ROOT] [--dry-run]` is mutually exclusive
with an ordinary title. It resolves one exact valid seed, preserves period,
ref, slug, and title, and targets only an existing configured task destination
root. Dry-run proves the source and collision-free destination without mutation.
Real graduation creates the exact task directory and atomically renames the
seed file to `napkin.md`; it never creates `CURRENT_STATE.md`. Rename failure
removes only the newly created empty directory and leaves the seed in place.
There is no cross-filesystem copy fallback. Ordinary `new` and `new --into`
retain their prior JSON/filesystem behavior.

## Safe Markdown relink repair

`sid relink [--write]` scans authored UTF-8 Markdown beneath discovered
task/review owners and explicit topic roots plus direct seed files. It honors
project ignore boundaries and excludes VCS metadata, `tmp`, every task
`inbox` subtree, and the entire note root including quarantine and done.
CommonMark link/image destinations and reference definitions are candidates;
code spans/fences, autolinks, external/schemed URLs, fragment-only links,
labels, and ordinary prose are not.

A Markdown destination remains a literal path by default; Markdown does not
assign line-navigation meaning to a suffix such as `:33`. When the project
enables the `colon-line` destination extension, relink recognizes exactly a
terminal `:[1-9][0-9]*` on an otherwise local destination. It first checks the
canonical literal path including that suffix. Only when the literal target does
not exist does it check the canonical base path without the suffix. A proven
replacement preserves the exact locator before any following fragment. Relink
does not count target lines or validate editor navigation. External
destinations, including URLs with ports, never enter extension handling. If
neither literal nor base target exists, the result retains a
`relink-missing-internal-target` finding.

A local destination is eligible only when exactly one path component embeds a
recognized task-folder or seed-file ref that resolves uniquely. Task moves
preserve an existing within-owner suffix; an empty suffix or
`CURRENT_STATE.md` targets the current entrypoint. Seed-file destinations
target the current seed file or a graduated task entrypoint. Fragments are
preserved. Proven replacements are normalized relative to the source file.
Missing, ambiguous, and vanished internal targets produce respectively
`relink-unresolved-ref`, `relink-ambiguous-ref`, and
`relink-missing-internal-target`; candidate findings do not make an otherwise
complete scan partial.

The result contains exactly `complete`, `applied`, `changes`, and `findings`.
Each change contains exactly absolute source `path`, one-based destination
`line`/Unicode-scalar `column`, canonical target `id`, original `from`, and
relative `to`, sorted by path, line, column, and id. Preview sets
`applied:false`, reports every proven change, and changes no bytes or mtimes.

Write never rescans the universe. Each planned file retains its exact bytes;
before replacement the current bytes must match. Replacements apply back to
front and a same-directory atomic replacement preserves permissions. A raced
file is skipped as a whole with `relink-concurrent-change`; an atomic failure
adds `relink-write-failed`. Either makes `complete:false` without blocking
other files, and only successfully applied changes are returned. Any usable
write result sets `applied:true`, including zero-change convergence. No usable
source fails with empty stdout. Invalid-UTF-8 Markdown is partial unreadable
coverage and is never rewritten.

## Finding and failure compatibility

Every finding contains exactly `code`, `severity`, `message`, nullable `id`,
nullable absolute `path`, and nullable one-based `line`, sorted by path, line,
code, then id with null first. Contract 0004 codes remain stable. Contract
0005 adds `stale-inbox-message`, `stale-capture-note`,
`invalid-inbox-envelope`, `unreadable-inbox-message`,
`unreadable-capture-note`, `relink-unresolved-ref`,
`relink-ambiguous-ref`, `relink-missing-internal-target`,
`relink-concurrent-change`, and `relink-write-failed`.

Configuration, validation, missing/ambiguous anchor, invalid limit, editor,
allocation, and no-usable-source failures are nonzero with human stderr and
empty stdout. Tolerant search/context/captures/relink scans return exit 0 JSON
when any contracted usable envelope exists, with `complete:false` for omitted
source coverage. Candidate-only relink findings do not make coverage partial.
Lint retains its distinct 0 clean/warning-only, 1 completed-data-error, and
2 operationally-incomplete empty-stdout exits.
