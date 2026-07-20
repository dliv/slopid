# Project handoff - slop-id restart

Date: 2026-05-31

This document is meant to stand alone in a fresh project. It captures the
problem, product direction, and owner preferences discovered during an earlier
design pass. It is not an implementation spec; use it to get oriented and to
start a focused Q&A with the owner before coding.

## The problem

Agentic work creates many small, parallel bits of work: fixes, reviews, spikes,
experiments, chores, and follow-up notes. These need durable places on disk that
humans and agents can find again without turning every task into a formal ticket.

The desired tool, tentatively named `sid`, generates and discovers folder-safe
identifiers for those work items.

The likely folder shape is:

```text
{YYYYMM}_{ref}[_{slug}]
```

Example:

```text
202605_sa2a7_fix-auth-state
```

Where:

- `YYYYMM` gives month-level time ordering.
- `ref` is a short `sXXXX` token that agents can cite and search for.
- `slug` is human context, like a branch-name style task title.

## Product intent

This is a small local CLI for short-term agentic workflow memory. It is not a
global id service, issue tracker, document database, or security boundary.

The common tree might be something like:

```text
repo/
  stm/
    202605_sa2a7_fix-auth-state/
    202605_sa3m9_review-login-flow/
    .pending/
    .archive/
```

The exact task root should be configurable. `stm/` is a common example, not a
law.

## Core goals

1. Folders should sort and scan by time.
   The filesystem tree itself is a UI. Finder, VS Code, terminal listings, and
   simple text search matter, not only the CLI.

2. Short refs should be operationally useful.
   An agent or skill may search for `sXXXX` or a prefix of it. A ref should
   usually identify one task folder, not turn into a vague slug search.

3. Full ids should be human-glanceable.
   They should look intentional and local, not like random hex, UUIDs, or git
   object names.

4. Slugs are context, not identity.
   The ref is the durable short handle. The slug helps a human scan the tree.

5. Active, pending, and archived tasks share one namespace.
   Moving a task to `.archive` or `.pending` should not silently free its ref for
   reuse in the same project/month.

6. Good-enough local behavior beats heavyweight coordination.
   Single-user local workflows are the target. Cryptographic uniqueness,
   multi-machine coordination, and strong concurrent allocation guarantees are
   out of scope unless real use proves otherwise.

## Likely CLI shape

Command names are examples, not final commitments, but these names fit the
existing mental model:

- `sid new "task title"`: allocate a new folder id and create the folder.
- `sid list` or `sid find`: discover existing ids, possibly by ref prefix.
- `sid agent-instructions`: print embedded instructions for agents. This name
  matches the `workforest` project and is probably preferred unless there is a
  strong precedent for another name.
- `sid id`: optional. If kept, be clear that it generates a syntactic id string
  without reserving a folder or preserving chronology.

Machine-readable output should be available with `--json`, especially for
agent-driven workflows.

<human>discuss "agent first", do agents like json? maybe json should be the default</human>

(Resolved by ADR 0004: JSON is `sid`'s default success output, and the
`--json` flag mentioned in this section was removed. ADR 0005 records the `stm`
compatibility binary's plain-output exception; ADR 0006 later allowed opt-in
non-contractual `--human` output on discovery commands such as `sid list`.)

## Config direction

Expect a project config file, probably `.sid`, likely TOML because this is a Rust
CLI. A minimal shape might be:

```toml
root = "stm"
additional = ["stm/.pending", "stm/.archive"]
```

(Superseded: the implemented shape is a `[task]` table with `root` and
`scan_roots` — see contract 0001's Project Config section. Unknown keys fail
closed, so this sketch is rejected by the real binary.)

The design preference is direct child scans only. Do not build a recursive
indexer unless the owner explicitly asks for that.

Questions to settle before implementation:

- Should missing optional `additional` dirs be skipped or created?
- Should unreadable configured dirs fail allocation closed?
- Should symlinked scan roots be followed, rejected, or treated as ordinary
  paths?
- Should user-level config exist in v1, or only project-local `.sid`?

(All settled: see contract 0001's Project Config and Scan Policy sections —
missing dirs are empty snapshots, unreadable dirs fail closed, symlinked roots
are followed — and the roadmap's Deferred By Default list for user-level
config.)

## Workforest influence

Use `workforest` as a style and architecture reference, not something to copy
blindly.

Likely useful patterns:

- agent-first commands such as `agent-instructions`;
- stable JSON output for automation;
- a functional core with an imperative CLI shell;
- typed result structs that serialize cleanly;
- assert-driven integration tests around observable CLI behavior;
- short decision records when a design choice is subtle.

Before importing any pattern wholesale, ask whether `slop-id` is large enough to
need it. This tool should stay small.

## Testing posture

The owner is leaning toward TDD after an initial Q&A. The ref design has enough
edge cases that tests should lead the implementation.

Useful test layers:

- pure tests for slug normalization, period parsing, ref encode/decode, and
  generated-ref validation;
- injected RNG/clock/scanner tests for allocation behavior;
- CLI integration tests with temp dirs and `--json` (note: ADR 0004 later
  removed the `--json` flag; `sid` output is JSON by default);
- focused tests for direct-child scanning across root, pending, and archive
  dirs.

Avoid tests that freeze internal retry mechanics more tightly than necessary.
Prefer observable contracts: final id shape, created path, JSON fields,
collision handling, and error behavior.

## Q&A prompts for the next agent

Start with questions like these before writing code:

(Most are now settled — do not reopen without new evidence: 1, 2, 4, and 9 by
contract 0001; 7 by ADR 0002; 10 by contract 0002 and ADR 0005; 6 shipped
minimally with the fuller text deferred to roadmap milestone 4; 3 and 5 are
tracked in contract 0001's "Requirements To Settle Soon" and roadmap
milestones 3 and 5. Only 8 remains genuinely open.)

1. Is `sid new` definitely a folder-creating command in v1, or should the first
   version only print ids?
2. What should the default task root be when there is no `.sid`?
3. Should `sid list` exist in v1, or should discovery wait for a later pass?
4. What exact JSON fields do agents need from `sid new`?
5. Should `sid id` exist, and if so how loudly should docs warn that it does not
   reserve anything?
6. Should `agent-instructions` be embedded in the binary from day one?
7. How much of the `workforest` architecture should be adapted, and what would
   be overkill here?
8. What are the owner's must-have examples for agent instructions?
9. What behavior should happen when a configured scan dir is missing or
   unreadable?
10. Are there any existing folder trees that must be migrated, or can v1 be a
    clean break?

## Ref design

The most contentious part of the earlier design work was the `ref` portion of
the id. Do not casually invent a new ref scheme before reading
`REF_DESIGN_HANDOFF.md`.

The current recommended default is the 1134 mixed-radix ref design described
there. Treat it as the starting point unless the owner explicitly reopens the
trade-off.
