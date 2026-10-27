# slop-id roadmap

Date: 2026-05-31
Updated: 2026-10-26
Status: Working draft

## Current direction

Keep `sid` a small local Rust CLI for one task-folder identity root. It allocates
stable folder names, lists real direct-child identity directories, resolves an
exact folder/ref, and audits name-level reservations. A consumer such as
Slopdeck owns task documents, relationships, capture data, and lifecycle state.

The local identity-only source cut is an unreleased breaking candidate. It must
be qualified with its paired Slopdeck consumer before any installation or
release decision. The later adoption bucket still owns external-consumer
evidence, version selection, distribution proof, and recoverable cutover.

## Delivered identity behavior

- Allocation, ref generation, prefix policy, slug rules, and no-clobber retry
  behavior: [contract 0001](contracts/0001-sid-new-dry-run.md).
- Direct real-directory list and exact folder/ref resolution:
  [contract 0003](contracts/0003-sid-list.md).
- Root discovery, structured failures, and identity-only lint:
  [contract 0007](contracts/0007-root-and-identity-lint.md).
- Ref alphabet, sequence and concurrency decisions remain in
  [ADR 0001](decisions/0001-start-from-1134-ref-design.md) and
  [ADR 0003](decisions/0003-best-effort-ref-uniqueness.md).

## Historical roadmap

The earlier v0.3.x line grew into a document-reader and workflow CLI with
graph/search/context/capture/note/seed/relink commands. That runtime surface and
its contracts were removed from this candidate after the responsibility cut
identified Slopdeck as the task-memory owner. They remain visible in the
immutable v0.4.0 baseline commit, not as active implementation promises.

The package name remains `slop-id`; the local source version remains unchanged
until a later version-selection decision. The hidden `--period` override stays
a shape-checked six-digit test seam. Calendar-valid month checks, global
allocation locking, library extraction, and a separate refcode crate remain
deferred unless later evidence changes their owner or scope.
