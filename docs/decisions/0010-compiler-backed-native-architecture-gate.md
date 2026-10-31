# 10. Compiler-Backed Native Architecture Gate

Date: 2026-10-30
Status: Accepted

## Context

Slopid's Sid 0.5.0 implementation is one Rust package with existing direct
uses of process inputs, output, exit, clock, and randomness. The accepted
architecture direction calls for build-enforced boundaries, but moving
functions or packages before recording the current effects would combine gate
installation with behavior and ownership changes. The shared architecture
engine is separately owned and is not required to ban these named compiler
items.

## Decision

Add a repository-local `just check` gate before moving code. Cargo denies
Clippy's named disallowed-method and disallowed-macro lints and unfulfilled
expectations. Root `clippy.toml` owns the path list; function-local expectations
record current production use and the small existing test-only subprocess,
clock, and FIFO support. A Python standard-library adapter checks exact
compiled source-site inventory and compiles fixed fixtures against the actual
manifest, lockfile, and Clippy configuration.

CI installs the stable Rust lint/format components and the pinned just command,
then invokes the same `just check` recipe. Production behavior and function
bodies stay unchanged. The `current_period` clock read and
`randomized_tail_space` shuffle remain temporary placement debt for the later
composition bucket.

## Consequences

- New direct use of a named banned operation fails the normal local and CI
  gate unless a narrow reasoned expectation is added.
- A stale expectation and removal of compiler enforcement are checked by
  executable fixtures; the exact current inventory is checked from compiler
  diagnostics rather than source parsing.
- Test-site expectations are separated from production inventory. The FIFO and
  recency test sites are Unix-only; the control's checked target set is current
  macOS and Linux.
- This gate does not prove transitive purity, helper ownership, runtime call
  frequency, file-I/O policy, or resistance to deliberate bypass. It does not
  implement the later policy crate, shared scanner, or package split.

Revisit this decision when moving the clock/random inputs to the composition
root, when the policy engine is adopted from its owning task, when source
ownership or package boundaries change, or when CI/local target coverage
changes. The active rule inventory and limits live in
[`docs/architecture.md`](../architecture.md).
