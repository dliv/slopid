# ADR Index

Date: 2026-05-31
Status: Active

Durable decisions for `slop-id` live here. Contracts in `docs/contracts/` define
implementation slices; ADRs capture project decisions that should survive beyond
one slice.

## Accepted

- [1. Start From 1134 Ref Design](0001-start-from-1134-ref-design.md)
- [2. Selectively Borrow Workforest Patterns](0002-selectively-borrow-workforest-patterns.md)
- [3. Best-Effort Ref Uniqueness](0003-best-effort-ref-uniqueness.md)
- [4. JSON Success Output By Default](0004-json-success-output.md)
  (amended by ADR 0006)
- [6. Opt-In Human Output For Discovery Commands](0006-opt-in-human-output-for-discovery.md)
- [7. Tolerant Reader Results](0007-tolerant-reader-results.md)
- [8. Deterministic Protocol And Controlled Mutations](0008-deterministic-protocol-and-controlled-mutations.md)

## Notes

- These ADRs intentionally do not import all workforest decisions.
- Reopen an ADR with a new superseding record rather than editing history after
  implementation depends on it.
