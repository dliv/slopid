# slop-id task memory

This folder dogfoods `sid`'s target task-folder shape. During bootstrap, treat
these files as working notes, not as proof that the CLI surface is final.

High-level project direction lives in `docs/ROADMAP.md`.

## Active

- (none)

## Completed

- `.archive/202606_sdd4c_discovery-commands` - `sid list` (contract 0003) and
  `sid new --into` for roadmap milestone 3.
- `.archive/202605_sea99_project-config-and-scan-roots` - project-local `.sid`
  config and additional direct-child scan roots. Durable behavior moved to
  `docs/contracts/0001-sid-new-dry-run.md` (Project Config and Scan Policy).
- `.archive/202606_python-temp-allocator` - temporary Python bridge explored
  and then removed once the private Rust repo became dogfoodable.
- `.archive/202605_seag8_harden-allocation-semantics` - allocation hardening
  after the bootstrap TDD slice and multi-agent review. Durable behavior moved
  to `docs/contracts/0001-sid-new-dry-run.md` and ADRs 0003-0004.

## Convention

Each active task gets a `PLAN.md` with:

- why the task exists;
- the high-level thread to resume;
- the immediate checklist;
- explicit out-of-scope items.

When `sid` grows archive support, completed folders can move to the configured
archive location.
