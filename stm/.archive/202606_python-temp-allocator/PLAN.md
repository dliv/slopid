# Temporary Python allocator

Created: 2026-06-01

## Why

This project needs a single-file Python version that can temporarily replace an
existing work script before the Rust crate is ready to publish.

## Current Intent

- Provide only folder creation.
- Exclude `list`.
- Exclude project config.
- Keep the implementation pragmatic rather than fully hardened for races and
  unusual filesystem failures.
- Preserve the important `ref` sequence behavior missing from the existing work
  script.

## Decisions

- The temporary script exposes one creation path: `python sid.py new "task
  title"` creates `./stm/{YYYYMM}_{sXXXX}_{slug}` and prints basic JSON shaped
  like Rust `sid new`.
- The single-file Python script lives at the repo root as `sid.py`.
- The script scans direct children of `./stm`, `./stm/.pending`, and
  `./stm/.archive` into one period/ref namespace. It does not support config.
- The Python deliverable remains one file, including executable tests using the
  standard library instead of a separate `tests/test_sid_py.py`.
- In-file tests run with `python sid.py --self-test`.
- Success output includes the Rust `sid new` JSON fields: `title`, `slug`,
  `period`, `sid_ref`, `id`, `path`, and `dry_run`, with `dry_run` always
  `false`.
- `SID_TEST_PERIOD=YYYYMM` overrides the current month for deterministic tests.
- Ref generation ports the Rust rules: deterministic month start, `MAX_SEQ =
  659`, recognized-ref scanning, sequence decoding, and generated-tail rule.
- Exact path collisions fail immediately instead of retrying alternate tails. A
  code comment notes this temporary simplification from Rust behavior.
- Scanned ref collisions also fail immediately instead of retrying alternate
  tails. A code comment notes this temporary simplification from Rust behavior.
- Tail randomness uses plain `random.choice`; deterministic self-tests can
  patch or seed it.
- Match Rust title parsing: the title is one CLI argument, so multi-word titles
  require shell quotes and extra positional arguments are rejected.
- Match Rust's `new` subcommand name even though the temporary script supports
  only that one user-facing command.

## Checklist

- [x] Collect and lock decisions from the grill loop.
- [x] Write executable in-file tests first.
- [x] Implement the smallest single-file Python version that satisfies the
      locked scope.
- [x] Run focused verification.

## Out Of Scope

- Publishing the Rust crate.
- Full CLI parity with Rust.
- `sid list`.
- Project-local or user-level config.
- Race-condition hardening beyond basic folder creation.
