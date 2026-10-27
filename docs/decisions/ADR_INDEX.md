# ADR Index

Date: 2026-10-26
Status: Active

These decisions apply to Slopid's local folder-identity and allocation
boundary. The current CLI contracts are in `docs/contracts/`.

## Accepted

- [1. Start From 1134 Ref Design](0001-start-from-1134-ref-design.md)
- [2. Selectively Borrow Workforest Patterns](0002-selectively-borrow-workforest-patterns.md)
- [3. Best-Effort Ref Uniqueness](0003-best-effort-ref-uniqueness.md)
- [4. JSON Success Output By Default](0004-json-success-output.md)
  (amended by ADR 0006)
- [6. Opt-In Human Output For Discovery Commands](0006-opt-in-human-output-for-discovery.md)
- [9. Identity-Only Lint](0009-identity-only-lint.md)

ADRs 0007 and 0008 describe the retired document-reader and workflow-mutation
surfaces. Their implementation and active instructions were removed for the
v0.5.0 identity-only release; the pinned v0.4.0 source commit preserves that
history.
