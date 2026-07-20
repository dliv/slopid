# 2. Selectively Borrow Workforest Patterns

Date: 2026-05-31
Status: Accepted

## Context

`workforest` is a nearby project in a similar agentic-work context. It has been
useful for the owner through months of fairly heavy use, so its patterns are
evidence rather than decoration.

`slop-id` is smaller. It should not inherit process, configuration, locking, or
type machinery merely because workforest has it.

## Decision

Use workforest as a proven local precedent with a presumption of relevance, not
automatic authority. Borrow patterns that fit the size and risk of `sid`, and
expect deviations when this project has different constraints.

Borrow now:

- typed command results that can be serialized;
- human and JSON output backed by the same result structs;
- a plan/execute split for mutating commands;
- contract docs and test matrices for behavior-heavy slices;
- assert-driven CLI tests around observable behavior;
- an `agent-instructions` command for agent-facing guidance.

Defer unless justified:

- broad configuration machinery;
- heavy newtype layers for every concept;
- lock, reservation, and rollback systems;
- larger process or command surface copied from workforest.

## Consequences

- Future agents should check workforest before inventing a new local pattern,
  especially for CLI result shapes, dry runs, and docs.
- A workforest pattern still has to pay rent in `slop-id`; smaller direct code is
  preferred when it preserves the same observable contract.
- Contract docs and ADRs should capture durable decisions before the handoff
  notes fade from active memory.
