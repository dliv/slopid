# 3. Best-Effort Ref Uniqueness

Date: 2026-05-31
Status: Accepted

## Context

The short `sXXXX` ref is meant to be operationally useful inside one local
project. In ordinary use, a ref should usually point to one task folder for a
period, even if the task has moved between active, pending, and archive roots.

`sid` is a slop id tool, not a global identity service or a security boundary.
The v1 target is local, single-user workflow memory with good-enough behavior.

## Decision

Accept best-effort ref uniqueness in v1.

Scanning should protect the ordinary namespace in non-concurrent use: if a
period/ref already exists in a configured scan root, a new slug must not make it
available again. However, v1 will not add a lock file, hidden per-ref
reservation, or other strict coordination mechanism.

Atomic `mkdir` only protects exact full-path collisions. It does not prevent two
concurrent processes from scanning the same state, drawing the same ref, and
creating different slugged paths such as:

```text
202605_seaa2_fix-auth
202605_seaa2_review-login
```

This race is undesirable but not catastrophic. The short format and random tail
mean collision risk exists by design.

When ambiguity appears, agents and users can disambiguate by period, slug,
surrounding task context, a future `sid list`, or plain text search such as
`rg sXXXX`.

## Consequences

- Do not write tests that imply strict ref-level atomicity under concurrency.
- Tests should still verify scan-time occupancy in ordinary, non-concurrent use.
- A future lock or reservation mechanism needs its own decision record because it
  would change the simplicity and failure modes of v1.
- Documentation should describe refs as usually unique local handles, not
  globally guaranteed identifiers.
