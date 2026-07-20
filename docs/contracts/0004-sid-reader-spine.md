# Contract 0004: sid reader spine

Date: 2026-07-12
Status: Implemented

## Scope and source model

`sid resolve`, `sid graph`, and `sid lint` are read-only JSON commands. They
inspect direct child directories of the active and additional configured task
roots and read `CURRENT_STATE.md` only. Missing roots are empty. A task folder
is `YYYYMM_<ref>[_slug]`; legacy four-character and current `s` plus four-
character refs are accepted. A parseable entrypoint in a malformed folder is
still indexed by its canonical frontmatter `id` and linted.

Frontmatter must start at byte zero and uses one `key: <JSON value>` line
between `---` delimiters. Required nonempty strings are `type`, `id`, `title`,
and `timestamp`; supported types are `task` and `review`. Unknown keys and JSON
values are preserved. `status` is forbidden. Relationships are string arrays
under `origin`, `supersedes`, and `related`.

## Commands and JSON

`sid resolve <ID>` matches the exact, case-sensitive canonical id. Success is
`{"node":{"path":<absolute path>,"frontmatter":<complete mapping>}}`.
Missing or ambiguous identity exits 1 with empty stdout.

`sid graph <ID> [--depth N] [--direction both|outgoing|incoming]
[--edge origin|supersedes|related]...` returns exactly `anchor`, `complete`,
`nodes`, `edges`, and `findings`. Bare graph performs unbounded breadth-first
traversal in both directions. `related` is traversable both ways under every
direction setting, but its authored edge is emitted once. Nodes sort by folder
id oldest-first, then canonical id and absolute path. Edges sort by type,
source, and target. Depth is shortest-hop distance with the anchor at zero.

Graph completeness describes the scanned corpus, not the selected query. Any
unreadable/unparseable entry, missing or ambiguous id, missing entrypoint,
unsupported type, invalid relationship list, or dangling target makes every
successful query over that scan `complete: false` with the relevant findings.
Malformed but indexable folders, metadata defects, duplicate values, and
self-edges are lint findings but do not alone make graph partial. Duplicate
values collapse and self-edges are omitted.

`sid lint` returns exactly `{"findings":[...]}`. A finding contains exactly
`code`, `severity`, `message`, `id`, `path`, and `line`; unavailable locations
are null and lines are one-based. Findings sort by path, line, code, then id,
with null before non-null. Clean and warning-only scans exit 0 with JSON; a
completed scan with an error exits 1 with JSON; an unreadable root or entrypoint
exits 2 with empty stdout and human stderr.

Stable 3A codes are `missing-entrypoint`, `malformed-folder-ref`,
`frontmatter-missing`, `frontmatter-unclosed`, `frontmatter-syntax`,
`duplicate-frontmatter-key`, `missing-required-field`,
`invalid-required-field`, `unsupported-type`, `forbidden-status`,
`id-folder-mismatch`, `duplicate-id`, `invalid-edge-list`, `duplicate-edge`,
`self-edge`, `dangling-edge`, `missing-edge-reason`, `unreadable-entry`, and
`unreadable-root`. Severity wire values are `error` and `warning`.

## Test matrix

`src/documents.rs` unit tests cover parsing, discovery, identity, edge
normalization, omission classification, and finding order.
`tests/reader_cli_test.rs` covers exact envelopes, strict resolve, traversal,
filters, cycles, completeness, lint exits, help, and agent instructions.
`tests/cli_test.rs` remains the regression contract for allocation, list,
configuration, and stdout/stderr.
