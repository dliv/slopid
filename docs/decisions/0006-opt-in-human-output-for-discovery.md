# 6. Opt-In Human Output For Discovery Commands

Date: 2026-06-12
Status: Accepted

## Context

ADR 0004 made JSON the only `sid` success output and deferred human-oriented
modes "until real use shows they are needed", naming `--human` as the
anticipated shape and requiring a deliberate contract update.

Real use arrived with `sid list` (contract 0003): the owner runs it directly
to find recently touched tasks, and pretty-printed JSON is poor for eyeball
scanning. This is the deliberate amendment ADR 0004 required for human-oriented
output rather than a supersession, since the JSON-by-default decision stands.

## Decision

Success stdout stays JSON by default for every `sid` command; agents keep
the machine contract with no flags involved.

Discovery commands may add an opt-in `--human` switch that replaces the JSON
success output with column-aligned plain text. `sid list --human` is the
first and currently only one.

Human output is explicitly not a machine interface: its formatting may
change without a contract update, and agents must not parse it. The JSON
output remains the contracted surface and is unaffected by the flag's
existence.

Mutating commands (`sid new`, `sid init`) stay JSON-only until real use
shows otherwise; each addition needs its own contract update and an
amendment here.

## Consequences

- The owner can scan `sid list --human` directly; agents are unaffected.
- Human modes are per-command, per-contract decisions — there is still no
  global output toggle, and the `--json` switch stays removed.
- ADR 0004's "JSON is the only supported success output format" is now
  scoped: JSON-only by default, with explicitly non-contractual opt-in
  human renderings where a contract records them.
