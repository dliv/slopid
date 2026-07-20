# Implementation handoff

Ref: `seag8`
Branch: `bootstrap-tdd`

This is the concrete work order for the allocation-hardening pass. `PLAN.md`
captures the owner decisions and scope; this file tells the next coding agent
what to do first.

## Goal

Make the current `sid new` slice trustworthy before config, extra scan roots,
`sid list`, or `sid id`.

Do not broaden the product surface. Stay inside the current slice unless a small
doc/test update is directly needed to lock a decision.

## Suggested Order

### 1. Make JSON-only success output real

Files likely touched:

- `src/cli.rs`
- `src/lib.rs`
- `src/commands/new.rs`
- `src/commands/agent_instructions.rs`
- `tests/cli_test.rs`
- `docs/decisions/ADR_INDEX.md`
- new `docs/decisions/0004-json-success-output.md`

Tests to write/update first:

- `sid new "fix auth state"` prints parseable JSON by default.
- `sid new "fix auth state" --dry-run` prints parseable JSON and creates no
  `stm`.
- `sid agent-instructions` prints parseable JSON with at least
  `{ "format": "markdown", "text": "..." }`.
- `sid --help` or command help no longer advertises a global `--json`.

Implementation notes:

- Remove the user-facing `--json` switch for now.
- Keep typed result structs.
- Add `slug` to `NewResult`.
- Keep errors/diagnostics on stderr as human text.
- Do not add `--human` or `--raw` yet.

ADR 0004 should defend:

- defaults reveal priority;
- requiring `--json` means the machine contract is not the default path;
- established human/operator CLIs with opt-in JSON are useful precedent for JSON
  as a format, not strong evidence for an explicitly agent-first default;
- stdout is machine-actionable success output, stderr is diagnostics.

### 2. Tighten module boundaries

Files likely touched:

- `src/lib.rs`
- tests, only if visibility changes require moving/adjusting tests.

Desired result:

- Internal modules are private unless intentionally exported.
- `main_entry()` remains the public binary entry point.
- Unit tests can continue testing internals inside modules.
- Integration tests should prefer the `sid` binary.

Avoid creating a polished public library API in this pass.

### 3. Add a simple deterministic `sid new` seam

Files likely touched:

- `src/commands/new.rs`
- `tests/cli_test.rs`, only if the seam is exposed through a test path.

Desired result:

- Production `cmd_new` still uses local time and random tails.
- Tests can call a helper with explicit `period` and candidate tails.
- Keep this plain: structs/functions are fine; avoid traits or an effects
  framework.

Possible shape:

```rust
struct NewDeps {
    period: String,
    tails: Vec<RefTail>,
}

fn cmd_new_with_deps(inputs: NewInputs, cwd: &Path, deps: NewDeps) -> Result<NewResult>
```

This exact shape is not required; keep the smallest clear seam.

### 4. Harden slug behavior

Files likely touched:

- `src/slug.rs`
- `src/commands/new.rs`
- `tests/cli_test.rs`

Tests to write first:

- Long titles produce deterministic truncated slugs.
- Truncation trims trailing `-`.
- All-punctuation titles fail through the CLI with a clear stderr diagnostic.
- Titles containing `sXXXX` are allowed and slugified normally.

Policy:

- Truncate rather than error for long titles.
- Keep slugs relatively short and branch-name-like.
- Agents should be encouraged in docs/instructions to summarize verbose user
  input before calling `sid new`.

Pick a cap that feels conservative and document it in code/tests. The owner has
not chosen an exact number; prefer a short, practical default over maximizing
path length.

### 5. Harden scanning behavior

Files likely touched:

- `src/commands/new.rs`

Tests to write first:

- Missing root is an empty snapshot.
- If the root disappears between lookup/read, `NotFound` is treated as empty.
- Task-id-shaped direct child that is not a directory fails closed with a clear
  error.
- Non-task-shaped files remain ignored.
- Recognized-but-not-generated direct child refs are scanned and affect
  `max(seq)`.

Policy:

- Direct-child scan only.
- Task-id-shaped direct children in scan roots must be directories.
- Do not reject `sXXXX` inside slugs/titles.
- Fail closed on suspicious task-shaped filesystem state.

Symlink policy is not fully settled. If touched, prefer documenting the current
behavior clearly rather than broadening scope.

### 6. Harden allocation edge cases

Files likely touched:

- `src/commands/new.rs`
- `docs/contracts/0001-sid-new-dry-run.md`

Tests to write first:

- Monthly exhaustion when scanned max seq is `659`.
- Occupied ref blocks allocation even when the new slug differs.
- Invalid drawn tails are skipped.
- All candidate tails exhausted returns a clear error.
- `execute_new` retries the next candidate on exact-path `AlreadyExists`.

Policy:

- Do not add locks or hidden reservation files.
- Do not write tests that imply strict ref-level atomicity under concurrency.
- Document the accepted same-ref/different-slug race in code comments near
  `execute_new` or allocation docs, but keep it concise.

### 7. Update docs/contracts

Files likely touched:

- `docs/contracts/0001-sid-new-dry-run.md`
- `docs/decisions/ADR_INDEX.md`
- `stm/202605_seag8_harden-allocation-semantics/PLAN.md`

Expected updates:

- Mark newly tested contract rows as `tested`.
- Add output-default ADR to the index.
- If a checklist item is intentionally deferred, say why.

### 8. Verify

Run:

```sh
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
git status --short --branch
```

If tests fail because the JSON-output contract changed, update tests rather than
preserving the old human-output behavior.

## Out Of Scope

- `.sid` config.
- Additional scan roots such as `.pending` and `.archive`.
- `sid list` or `sid id`.
- Separate `slop30` / refcode crate.
- Lock files or strict concurrent allocation guarantees.
- Structured JSON errors.
- `--human`, `--raw`, or extra output formats.
- Broad README/AGENTS polish, unless a tiny note is required by a changed
  contract.

## Commit Guidance

This branch will likely be squashed before merge. Prefer coherent checkpoints,
but do not split work unnaturally just for commit purity.
