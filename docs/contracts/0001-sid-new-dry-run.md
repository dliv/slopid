# Contract 0001 - `sid new` dry-run slice

Date: 2026-05-31
Updated: 2026-06-12
Status: Active

## Milestone

Build the smallest end-to-end slice that proves the shape of the project:

```text
sid new "task title"
sid new "task title" --dry-run
```

The command allocates ids shaped like:

```text
{YYYYMM}_{ref}_{slug}
```

where `ref` uses the recommended 1134 design from
`handoff-docs/REF_DESIGN_HANDOFF.md`.

## Architecture Commitments

- Use a Rust package named `slop-id` with a `sid` binary.
- Keep the `workforest` pattern: typed command results and a thin CLI.
- Successful command stdout is JSON by default. Human-readable diagnostics and
  errors use stderr.
- Use a plan/execute split for mutating commands.
- `sid new --dry-run` returns the same planned folder information without
  creating the task root or task folder.
- Keep `slop30`/1134 logic internal for now; do not extract a crate until a real
  reuse boundary appears.

## First Defaults

- With no config, the task root is `stm` under the current directory.
- With no config, the additional scan roots are `stm/.pending`, `stm/.prs`, `stm/.slow`,
  and `stm/.archive`.
- `sid new` creates the task root if it is missing.
- `sid new --dry-run` does not create the task root.
- Scan direct children only.
- The scan targets are the active task root plus configured additional
  direct-child scan roots.
- Periods are six ASCII digits. Calendar-valid month enforcement, such as
  rejecting `202600` or `202613`, is deferred.
- `sid new` accepts a hidden `--period YYYYMM` override as a deterministic
  test seam (mirroring `stm --month`). The override is shape-checked to six
  ASCII digits; calendar validity stays deferred. It is excluded from help
  output and is not part of the supported user surface.

## Project Config

`sid new` discovers an optional project-local `.sid` TOML file by walking from
the current working directory up to the filesystem root and using the nearest
one (owner decision 2026-06-12, replacing the earlier cwd-only rule).
Configured paths resolve against the directory containing the discovered
`.sid` — the project base — so `sid` allocates into the same tree from any
subdirectory of a configured project. With no `.sid` anywhere, defaults
resolve against the current working directory. `sid init` always writes to
`.sid` in the current working directory. The v1 shape is:

```toml
[task]
root = "stm"
scan_roots = ["stm/.pending", "stm/.prs", "stm/.slow", "stm/.archive"]
```

- `[task].root` is the active task root used for newly allocated folders. It
  defaults to `stm`.
- `[task].scan_roots` is the list of additional direct-child scan roots. It
  defaults to `["stm/.pending", "stm/.prs", "stm/.slow", "stm/.archive"]` when `[task].root`
  is omitted. In a hand-written config with `[task].root` but no
  `[task].scan_roots`, the additional scan roots default to `.pending`, `.prs`, `.slow`,
  and `.archive` under the configured active root.
- Unknown `.sid` keys fail closed instead of being ignored.
- Configured paths must be relative, must not contain `..`, must name at least
  one real path segment (empty strings and `.` fail closed), and are resolved
  against the project base (the directory containing the discovered `.sid`,
  or the current working directory when none exists).
- The active root and additional scan roots share one period/ref namespace.
- `sid new --into <root>` allocates into a configured root instead of the
  active root (owner request 2026-06-12, e.g. `--into .slow`). The argument
  resolves by component-wise suffix match against the configured list
  (active root included; a leading `./` is ignored). Empty and unknown
  values fail closed naming the configured roots; ambiguous values fail
  closed naming the matching roots. Scanning is unaffected — the namespace
  stays shared.
- `sid new` creates only the destination root (the active root by default,
  or the `--into` root) and the chosen task folder. It does not create the
  other configured scan roots.
- `sid new --dry-run` creates nothing.
- `sid init` creates `.sid` with the default TOML and fails rather than
  overwriting an existing config.
- Running `sid init` in a subdirectory of an already-configured project
  writes a nested config that shadows the parent for that subtree (nearest
  wins). It does not warn; check for an ancestor `.sid` first when one
  namespace is intended.

## Scan Policy

- A missing active root or additional configured scan root is an empty snapshot.
- If reading a scan root reports `NotFound`, treat it as an empty snapshot.
- Other scan-root read errors fail closed.
- Symlinked active or scan roots are followed like ordinary directories. A
  dangling symlinked scan root therefore reads as `NotFound` and becomes an
  empty snapshot — unlike a dangling `.sid` config symlink, which fails
  closed. A dangling symlinked active root also scans as empty, but a real
  (non-dry-run) `sid new` then fails at the create-root step, since a
  directory cannot be created through the dangling link.
- Direct children shaped like `{YYYYMM}_{recognized sXXXX}`, slugged or
  slugless, reserve that period/ref namespace whether or not they are
  directories. Task-id-shaped files and symlinks are best-effort reservations
  (owner decision 2026-06-12: zipped task-folder exports legitimately sit
  next to their source folders, and unexpected entries must never hard-break
  allocation). Only unreadable scan roots fail allocation closed.
- Period-prefixed names with non-recognized ref segments are ignored. Known
  boundary: a zipped *slugless* folder such as `202605_sa2a2.zip` is ignored,
  not reserved, because its ref segment includes the extension; zipped
  slugged folders (`202605_sa2a2_fix-auth.zip`) reserve normally.
- Scan state is partitioned by period; refs and max sequence values discovered
  in one period do not affect another period.
- Scan races remain fail-closed in v1. If reading the directory stream fails
  partway, fail the allocation; do not backtrack and reuse a sequence that
  may have been present moments earlier.

## Ref Design Test Matrix

| Constraint | Status |
| --- | --- |
| Alphabet membership and order | tested |
| `encode_seq` examples from handoff | tested |
| `decode_seq` examples | tested |
| Generated-vs-recognized distinction | tested |
| Digit-run generation rule | tested |
| Deterministic `seq_start(period)` vectors | tested |
| Empty-month allocation uses deterministic start | tested |
| Existing max sequence allocates `max + 1` | tested |
| Monthly exhaustion at `seq > 659` | tested |
| Final monthly sequence `659` remains allocatable | tested |
| Occupied refs reserve namespace independent of slug | tested |
| Additional scan dirs reserve the same namespace | tested |
| Missing/unreadable configured dirs behavior | tested |
| Root-only config derives pending/prs/slow/archive scan roots from root | tested |
| Unknown `.sid` config keys fail closed | tested |
| Absolute `.sid` config paths fail closed | tested |
| Parent-dir `.sid` config paths fail closed | tested |
| Dangling `.sid` symlink fails closed | tested |
| Missing default root is treated as empty | tested |
| Task-shaped non-directories reserve the namespace best-effort | tested |
| Period-prefixed non-recognized files are ignored | tested |
| Recognized-but-not-generated refs affect scanning | tested |
| Slugless task-shaped directories reserve namespace | tested |
| Period partitions scan occupancy and max sequence | tested |
| `AlreadyExists` retry behavior | tested |
| Symlinked active/scan roots followed; dangling scan root scans as empty | tested |
| Dangling active root: dry-run empty snapshot, real run fails at create | tested |
| Upward `.sid` discovery: nearest config wins, paths resolve at config dir | tested |
| Slug truncation boundary at exactly 48 and 49 input chars | tested |
| Alphabet arrays locked to the alphabet strings | tested |
| Full 0..=659 encode/decode roundtrip | tested |
| Each default scan root (`.pending`/`.prs`/`.slow`/`.archive`) is decisive | tested |
| Hidden `--period` override: deterministic start, 659/660, shape check | tested |
| Empty and `.` config paths fail closed | tested |
| `sid init` names dangling `.sid` symlinks distinctly | tested |
| `--into`: shorthand/full forms, namespace shared, dry-run inert | tested |
| `--into` unknown and ambiguous destinations fail closed | tested |
| JSON output fields for `sid new` | tested |
| `sid new --dry-run` creates nothing | tested |
| `sid init` writes default config without overwriting | tested |

## Output Contracts

- `sid new` success stdout is JSON with exactly:
  `title`, `slug`, `period`, `sid_ref`, `id`, `path`, and `dry_run`.
- `sid agent-instructions` success stdout is JSON with exactly `format` and
  `text`.
- `sid init` success stdout is JSON with exactly `path` and `created`.
- The user-facing `--json` switch is not part of v1.
- Failure stdout is empty; diagnostics are human-readable stderr.

## Slug Policy

- Slugs are ASCII branch-name-style text.
- Empty slugs fail with a clear CLI error.
- Long slugs truncate deterministically to 48 characters after slugification and
  trim trailing dashes.

## Requirements To Settle Soon

- Should `sid id` exist at all? The roadmap defers the decision to milestone 5.

Settled: `sid list` shipped 2026-06-12 as roadmap milestone 3; its contract
is `docs/contracts/0003-sid-list.md`.

Settled: calendar-valid month enforcement for injected periods is deferred by
default (see First Defaults above and the roadmap's Deferred By Default list).
