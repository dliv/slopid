# Contract 0007 — root, resolution, errors, and identity lint

Date: 2026-10-22
Updated: 2026-10-26
Status: Active for the unreleased identity-only candidate

## Root discovery

`sid root` loads the nearest upward `.sid`; without a config it uses cwd as the
base and `stm` as the logical root. It returns exactly
`{"base": "<absolute base>", "root": "<absolute logical root>"}` without
creating the root. `sid root --human` prints only the root path and newline.
Config discovery fails closed on malformed or dangling configs and rejects
unknown keys and unsafe root paths. Help, version, `init`, and
`agent-instructions` do not load project config.

The accepted config fields are top-level optional `root` and optional
`[ref].deny_prefixes`; `sid init` writes exactly `root = "stm"`. Table-based
`[task]`, `[seed]`, `[note]`, `[topic]`, `[queue]`, and `[relink]` configs are
not accepted by this candidate.

## Folder resolution

List and resolve use the complete, no-follow directory snapshot in
[contract 0003](0003-sid-list.md). Resolve validates its selector after config
loading and before scanning. An exact recognized ref or complete identity
folder name can resolve only through enumerated direct-child names. Invalid
selectors do not become paths or prefix searches. Resolve never opens
`CURRENT_STATE.md`, Markdown, or other authored content.

## Identity lint

`sid lint` scans the names of direct children under the one configured root.
It does not stat children, follow symlinks, or open authored content. Missing
root is empty. Hidden and non-period names and non-UTF-8 names are ignored.
Files, real directories, and symlinks reserve a recognized ref. A valid
six-digit period with an unrecognized ref remains a finding even though it is
not a list/resolve identity. Duplicate recognized refs are reported across
periods and object types.

The report contains exactly `complete`, `healthy`, and `findings`. Each finding
contains exactly `code`, `severity`, `message`, nullable `ref_id`, and nullable
`path`. Paths and locators are relative to the discovered base. The closed
code set is:

- `identity-folder-ref-invalid`
- `identity-ref-duplicate`
- `identity-root-unreadable`
- `identity-entry-unreadable`

Root and entry read failures make coverage incomplete. Findings sort by
severity (errors first), code, optional ref, optional path, then message. All
current findings have severity `error`; the report retains the `warning`
severity value for wire stability.

Exit 0 emits a complete healthy report, exit 1 a complete unhealthy report,
and exit 2 an incomplete report. Config failure occurs before a report exists
and uses the generic `sid-config` stderr envelope.

## Error envelopes

Failures before a result emit one JSON object on stderr and leave stdout empty:

```json
{"error":{"code":"sid-config","message":"..."}}
```

Codes come from typed failure variants, never display-string matching:

| Code | Boundary |
| --- | --- |
| `sid-config` | Config discovery, parse, or policy validation |
| `sid-new` | Allocation scan, plan, or create after config |
| `sid-list` | Complete directory snapshot failure |
| `sid-resolve-invalid` | Invalid selector shape |
| `sid-resolve-not-found` | Valid selector with no directory match |
| `sid-resolve-ambiguous` | Short ref matches multiple directories |
| `sid-resolve-unavailable` | Resolve snapshot failure |
| `sid-init` | Exclusive config creation failure, including existing config |
| `sid-runtime` | Current-directory acquisition failure |
| `sid-output` | Result serialization or stdout write failure |
| `sid-usage` | Clap parse, removed command/option, or required-argument failure |

Resolve ambiguity alone adds `candidates`, a sorted array of objects with only
`id` and absolute `path`. Command/domain failures exit 1; CLI usage failures
exit 2. Help and version remain ordinary exit-0 text. Human flags alter
successful output only. Lint's complete/incomplete result reports remain on
stdout with their own 0/1/2 status contract.

## Compatibility boundary and proof

This is an unreleased breaking protocol change from v0.4.0. It intentionally
does not decode legacy table config, accept a `tasks` list alias, or negotiate
with old consumers. Slopdeck must be qualified as a paired consumer before
adoption. The local CLI checks prove neither external-workspace compatibility,
installed-binary safety, release embedding, nor browser behavior.

`tests/identity_cli_test.rs` covers exact channels, exits, config and selector
precedence, and no-document-read behavior. `src/commands/folders.rs` tests
deterministic iterator and matching-metadata failures without relying on
filesystem permissions. The separate Slopdeck pair harness supplies assembled
consumer and HTTP proof.
