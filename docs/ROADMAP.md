# slop-id roadmap

Date: 2026-05-31
Updated: 2026-10-26
Status: Current source version 0.5.0; paired release preparation

## Current direction

Keep `sid` a small local Rust CLI for one task-folder identity root. It allocates
stable folder names, lists real direct-child identity directories, resolves an
exact folder/ref, and audits name-level reservations. A consumer such as
Slopdeck owns task documents, relationships, capture data, and lifecycle state.

Sid 0.5.0 is the breaking identity-only release and pairs with Slopdeck
0.1.1-alpha.19. Local source and package qualification establish the release
inputs; remote distribution and recoverable workspace adoption remain separate
proofs. Do not switch only one product or infer workflow parity from the
identity protocol cut.

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
its contracts were removed for the v0.5.0 responsibility cut after Slopdeck
was identified as the task-memory owner. They remain visible in the immutable
v0.4.0 baseline commit, not as active implementation promises.

The package name remains `slop-id`; the current source version is 0.5.0. The
hidden `--period` override stays
a shape-checked six-digit test seam. Calendar-valid month checks, global
allocation locking, library extraction, and a separate refcode crate remain
deferred unless later evidence changes their owner or scope.
