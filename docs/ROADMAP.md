# slop-id roadmap

Date: 2026-05-31
Updated: 2026-07-20
Status: Working draft

This is the high-level project plan. Detailed execution notes live in `stm/`
task folders; durable behavioral contracts live in `docs/contracts/`.

## Current Direction

Build `sid` as a small local Rust CLI implementing a deterministic STM
protocol around durable task folders, parked seeds, explicit topics, and
verbatim capture queues:

```text
{YYYYMM}_{ref}_{slug}
```

The folder tree is part of the UI. Keep the tool small, local, testable, and
agent-drivable.

## Milestones

### 0. Bootstrap TDD Slice - Done

Commit: `66d3b75 Bootstrap sid TDD slice`

- Rust package `slop-id` with `sid` binary.
- `sid new "title"` creates a folder under `stm/`.
- `sid new "title" --dry-run` previews without creating folders.
- Initial machine-readable output support.
- Internal 1134 ref implementation.
- Initial contract/test matrix.

### 1. Allocation Hardening - Done

Task: `stm/.archive/202605_seag8_harden-allocation-semantics`

Goal: make the current allocation behavior trustworthy before adding config or
more commands.

Key themes:

- close pending allocation tests;
- tighten accidental public API boundaries;
- add small test seams for period/tails if useful;
- document accepted concurrency limits;
- settle small edge policies such as long slugs and malformed injected periods.

Durable behavior is recorded in `docs/contracts/0001-sid-new-dry-run.md` and
ADRs 0003-0004.

### 2. Project Config And Scan Roots - Done

Commit: `30774ef Add project config and scan roots`
Task: `stm/.archive/202605_sea99_project-config-and-scan-roots`

Goal: introduce project-local `.sid` config without making scanning recursive or
heavy.

Durable behavior is recorded in `docs/contracts/0001-sid-new-dry-run.md`
(Project Config and Scan Policy sections).

Scope was:

- TOML `.sid`;
- configurable root, defaulting to `stm`;
- additional direct-child scan dirs such as `.pending` and `.archive`;
- clear behavior for missing and unreadable scan dirs;
- tests that active, pending, and archived tasks share the same ref namespace.

### 3. Discovery Commands - Done

Task: `stm/.archive/202606_sdd4c_discovery-commands`

Goal: make existing ids easy for humans and agents to find.

Shipped 2026-06-12: `sid list [TERM]...` (contract 0003) — JSON listing of
the allocation-namespace view across all configured roots, most recently
touched first (`--sort id` for allocation order), where each term filters
by ref prefix or slug substring, case-insensitively — plus
`sid new --into <root>` for allocating directly into a configured scan
root (contract 0001). `sid find`/content search was not needed;
direct-child scanning only, as planned.

### 4. Agent Documentation And Dogfooding - Done

Goal: make future agents effective without rereading every handoff note.

Shipped in `v0.1.0`:

- comprehensive JSON-wrapped `sid agent-instructions` guidance;
- a public README with installation, command, and agent-integration guidance;
- examples for allocating, resolving, searching, citing, capturing, and
  repairing task memory;
- sustained dogfooding as the task-memory substrate that motivated the
  protocol.

### 3A. Deterministic STM Reader Spine - Done

Task: `se2vv`, bucket 3A

`sid resolve`, `sid graph`, and `sid lint` form a read-only reader spine over
configured STM task roots. They scan files and emit typed JSON; they do not add
issue-tracker semantics or mutate STM content. Resolve exposes canonical
frontmatter identity, graph derives incoming and outgoing relationships, and
lint audits the same substrate with stable finding codes. Durable behavior is
recorded in `docs/contracts/0004-sid-reader-spine.md` and ADR 0007.

This extends discovery into deterministic read/query peers suited to agents
and shell tooling while preserving task-only list/allocation behavior.

### 3B. Complete Deterministic STM Protocol - Done

Task: `se2vv`, bucket 3B

Read/query peers now include `search`, `context`, and `captures` over typed
task, seed, note, topic, and queue configuration. Controlled mutations include
identity-free `note`, minimum-valid `seed`, identity-preserving
`new --from-seed`, and preview-first per-file-atomic `relink --write`.
Contract 0005 and ADR 0008 define exact JSON, tolerant completeness,
verbatim-queue boundaries, and compatibility with ordinary `new` and `list`.

Slopid supplies deterministic substrate and bounded mutation primitives. It
does not choose relevance, drain inboxes, file notes, assign lifecycle status,
or implement workspace rituals; those judgment-bearing consumers live outside
this project.

### 5. Packaging And Release - Done

`v0.1.0` ships checksum-verified macOS archives for Apple Silicon and Intel
through GitHub Releases. The `dliv/tools` Homebrew tap installs the `sid` binary,
and the release workflow updates the formula version and checksums after each
`v*` tag.

Public-release verification covered CI, both release artifacts, the formula
update, a fresh Homebrew installation, and `sid --version`.

### 6. Evidence-Led Follow-Ups

There is no active feature milestone. Add only commands that prove useful in
practice.

Possible future scope:

- decide whether `sid id` should exist (use `sid new --dry-run` meanwhile);
- update GitHub Actions when upstream Node 24-native releases are proven;
- add platform artifacts when real users require them;
- revisit a separate `slop30`/refcode crate only if another consumer appears.

## Deferred By Default

- Generated static indexes.
- Global id service.
- Strong multi-process allocation locking.
- Calendar-valid month enforcement for injected periods.
- User-level config.
- Separate `slop30` crate.
- Issue-tracker semantics.

## Review Gate

Before moving from one milestone to the next:

- run `cargo fmt`;
- run `cargo test`;
- run `cargo clippy --all-targets -- -D warnings`;
- update the relevant contract/task docs;
- commit a coherent checkpoint.
