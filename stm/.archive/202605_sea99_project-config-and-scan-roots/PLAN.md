# Project config and scan roots

Ref: `sea99`
Created: 2026-05-31
Branch: `config-tdd`
Starting point: `593b2f1 Implement sid allocation hardening`

## Why

Allocation semantics are now hardened for the default `stm` root. The next v1
slice is project-local configuration and additional scan roots, so `sid new` can
respect existing active, pending, and archived task folders without broadening
into discovery commands yet.

## Resume Thread

The allocation hardening task closed with durable behavior recorded in:

- `docs/contracts/0001-sid-new-dry-run.md`
- `docs/decisions/0003-best-effort-ref-uniqueness.md`
- `docs/decisions/0004-json-success-output.md`

The roadmap's next milestone is "Project Config And Scan Roots".

## Decisions

- `.sid` is optional project-local TOML loaded from the current working
  directory.
- The v1 shape is `[task]` with `root = "stm"` and default
  `scan_roots = ["stm/.pending", "stm/.archive"]`.
- Absent `.sid` uses the same default scan roots as the default config file.
- Hand-written configs with `root` but omitted `scan_roots` derive `.pending`
  and `.archive` scan roots from the configured active root.
- Unknown `.sid` keys fail closed instead of being ignored.
- Absolute `.sid` config paths fail closed instead of escaping the project.
- Parent-dir `.sid` config paths and dangling `.sid` symlinks fail closed.
- `sid init` writes the default config to `.sid` and does not overwrite an
  existing config.
- Missing active or additional scan roots are treated as empty snapshots.
- `sid new` creates only the active root and chosen task folder.
- `sid new --dry-run` creates nothing.
- Scan-root read errors other than `NotFound` fail closed with diagnostics.

## Initial Questions

- Resolved: `.sid` TOML shape and scan-root behavior are recorded above and in
  `docs/contracts/0001-sid-new-dry-run.md`.

## Checklist

- [x] Write/settle a contract update for `.sid` config and scan-root behavior.
- [x] Add failing tests before implementation.
- [x] Add config loading without widening into `sid list` or `sid id`.
- [x] Add `sid init` for writing the default project config.
- [x] Preserve direct-child-only scanning.
- [x] Preserve shared ref namespace across active, pending, and archive roots.
- [x] Run `cargo fmt`, `cargo test`, and `cargo clippy --all-targets -- -D warnings`.

## Out Of Scope

- `sid list`, `sid find`, or `sid id`.
- Recursive indexing.
- Strong multi-process allocation locking.
- Global or user-level config.
- Packaging or release work.
