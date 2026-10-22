# Contract 0007: root discovery and identity lint

Date: 2026-10-22
Status: Implemented in v0.4.0

## Root discovery

`sid root` discovers the nearest `.sid` configuration upward from the current
directory, using the same configuration and defaults as allocation. It returns
exactly `{"root": "<absolute active task root>"}` and exits zero. Discovery does
not create a missing root. `sid root --human` prints that path and a newline.
Invalid configuration fails with exit 1, empty stdout, and a stderr diagnostic.

## Identity namespace

`sid lint` inspects direct-child names in every configured allocation root:
the active task root, additional task scan roots, and the seed root. It opens
no authored document and does not stat or follow individual entries. Files,
directories, and symlinks all reserve a recognized ref. A missing root is an
empty namespace. Nonmatching names and non-UTF-8 names are ignored.

A six-digit period followed by an unrecognized ref is an identity error.
Recognized refs are the current five-character refs accepted by the allocator;
legacy document IDs remain readable by the document commands but are not made
valid allocation refs by that tolerance. The same recognized ref at two
distinct direct-child locators is a duplicate even across periods or roots.

## JSON and exits

The report contains exactly `complete`, `healthy`, and `findings`.
`complete` means all configured allocation roots and entries were enumerated.
`healthy` means no error finding was observed. Each finding contains exactly
`code`, `severity`, `message`, nullable `ref_id`, and nullable `path`.
Paths and locators in messages are relative to the discovered project base.
Duplicate findings have a null path and list sorted reservation locators in
their message. No `id` or `line` field belongs to this report.

The complete finding vocabulary is:

- `identity-folder-ref-invalid`: a valid period prefix with an unrecognized ref.
- `identity-ref-duplicate`: a recognized ref reserved at multiple locators.
- `identity-root-unreadable`: a configured allocation root cannot be enumerated.
- `identity-entry-unreadable`: a directory entry cannot be read during enumeration.

The latter two codes make coverage incomplete. All four currently have severity
`error`; the severity vocabulary also supports `warning`. Findings sort by
severity (errors first), code, optional ref, optional path, then message;
optional null values sort first. Warnings alone do not make a report unhealthy.

Exit 0 emits a complete healthy report. Exit 1 emits a complete unhealthy
report. Exit 2 emits an incomplete report, even when errors are also present.
Configuration or invocation failures occur before a report exists and retain
exit 1 with empty stdout and stderr; callers must distinguish those failures
from a decoded report.

## Compatibility and proof

This is a deliberate pre-1 breaking replacement of v0.3.x workflow lint, not a
compatibility mode. It supersedes the lint portions of contracts 0004/0005 and
ADR 0007. Frontmatter, relationships, inboxes, notes, topics, and lifecycle
views belong to the task-memory layer above Slopid. The other commands remain
available, including unbounded-by-default `context` and its optional depth.

Consumers should validate the closed finding vocabulary, completeness,
healthiness, and exit-status agreement. Adding finding codes requires consumer
coordination. `src/commands/lint.rs` unit tests cover deterministic entry-read
failure, ordering, and status boundaries. `tests/reader_cli_test.rs` covers
healthy, invalid, duplicate, and incomplete namespaces and independence from
authored bytes. `tests/cli_test.rs` covers root discovery and no-create behavior.
`tests/protocol_cli_test.rs` proves queue defects remain visible to context
without entering identity lint.
