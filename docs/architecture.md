# Current architecture and compiler gate

Date: 2026-10-30
Status: Active for Sid 0.5.0

This document owns Slopid's current compiler rules, recorded direct-use
exceptions, enforcement classes, and the limits of the gate. The accepted
longer-term package shape is recorded in
[ADR 0010](decisions/0010-compiler-backed-native-architecture-gate.md); this
gate is its first bounded step, not the completed package split.

## Current boundary

Slopid is one Rust package with the `sid` CLI as its application. `just check`
is the local complete gate. CI calls that same recipe after installing the
stable `rustfmt` and Clippy components, Python 3, and just 1.57.0.

The recipe runs, in order:

1. `cargo fmt --all -- --check`
2. `cargo clippy --locked --all-targets -- -D warnings`
3. `cargo test --locked`
4. the focused Python adapter tests under `scripts/tests/`
5. `python3 scripts/check_clippy_policy.py`

Named individual recipes are available with `just --list`. The release
workflow is separate and does not define or replace this development gate.

## Native bans and recorded sites

Root `clippy.toml` names direct calls to:

- `std::env::{args,args_os,var,var_os,current_dir,set_current_dir}`
- `std::time::{SystemTime::now,Instant::now}` and
  `chrono::{Local::now,Utc::now}`
- `fastrand::shuffle`
- `std::process::{exit,abort,Command::new}`
- `std::io::{stdin,stdout,stderr}`
- `std::{print,println,eprint,eprintln,dbg}` macros

`Cargo.toml` denies `clippy::disallowed_methods`,
`clippy::disallowed_macros`, and Rust's
`unfulfilled_lint_expectations`. The usual Clippy warning policy remains
warnings-denied through the gate command.

The production source has eight current method call sites covered by seven
function-local `#[expect]` attributes:

| File | Function | Banned item | Count |
| --- | --- | --- | ---: |
| `src/lib.rs` | `main_entry` | `std::process::exit` | 1 |
| `src/lib.rs` | `current_dir` | `std::env::current_dir` | 1 |
| `src/failure.rs` | `report_failure` | `std::io::stderr`, `std::process::exit` | 2 |
| `src/failure.rs` | `write_json` | `std::io::stdout` | 1 |
| `src/failure.rs` | `write_text` | `std::io::stdout` | 1 |
| `src/commands/new.rs` | `current_period` | `chrono::Local::now` | 1 |
| `src/commands/new.rs` | `randomized_tail_space` | `fastrand::shuffle` | 1 |

`current_period` and `randomized_tail_space` keep the existing clock and random
operations in the allocation workflow temporarily. Moving those inputs to the
CLI composition root belongs to a later bucket. The other production
expectations mark current CLI input, output, process exit, and working-directory
ownership. These annotations do not authorize additional call sites.

Clippy also compiles the integration-test target. Four narrowly scoped test
expectations cover five existing test-only sites: `run_bounded` calls
`std::process::Command::new` once, `wait_bounded` calls `std::time::Instant::now`
twice, `make_fifo` calls `std::process::Command::new` once, and
`recency_uses_direct_child_metadata_without_following_child_symlinks` calls
`std::time::SystemTime::now` once. The latter two functions are `cfg(unix)`
test support. Their function bodies and the seven production function bodies
remain unchanged; the reasons are documented at the attributes.

The checked inventory in
[`scripts/clippy_policy_expectations.json`](../scripts/clippy_policy_expectations.json)
keeps production and test sites in separate groups by relative file, lint code,
canonical banned item, and count. On the supported macOS and Linux targets, it
expects eight production method sites and five test method sites. Both macro
site counts are zero. The Unix-only test entries mean that another target needs
its own reviewed inventory expectations before using this control.

## What the gate proves

| Class | Rule | Evidence |
| --- | --- | --- |
| Gated | New direct use of a named banned method or macro fails normal all-target Clippy unless narrowly expected. | Denied Cargo lint settings plus `-D warnings` in `just clippy`. |
| Gated | A stale method expectation fails compilation. | `unfulfilled_lint_expectations = "deny"` and the stale-expectation fixture. |
| Gated | The named compiler lints remain enabled and the normal command still detects them. | An unexempted aliased method fixture and a `println!` fixture compile without force flags and must fail with their exact lint identities. |
| Gated | The recorded compiled call sites remain at the expected files, identities, and counts. | Forced-warning Cargo JSON inventory, deduplicated only for identical primary span plus lint and item identity across targets. |
| Gated | CI invokes the complete local gate. | The CI workflow runs the literal `just check`. |
| Checkable | Each production expectation still protects the function and effect named in its reason. | Inspect the function body and the exact attribute; the inventory alone does not prove its semantic purpose. |
| Judgment | Calls belong in the current function, and composition owns the ambient inputs. | Review the caller, function responsibility, and later composition work. |

The Python adapter reads Cargo JSON compiler messages; it does not parse Rust
source. Its focused tests cover duplicate target reports, distinct sites,
missing and extra inventory entries, malformed JSON, unrelated compiler
errors, and unexpected warning identities. Compile-only fixtures are generated
in a fresh temporary directory from the repository's `Cargo.toml`, lockfile,
and Clippy config. They do not execute fixture functions or alter tracked
source.

## Limits

This gate checks only the named direct compiled calls and macros. It does not
prove transitive purity, dependency internals, file-I/O policy, function
ownership, runtime call frequency, business-free dispatch, or resistance to a
deliberate bypass. A method ban cannot prove that permitted helpers have the
right responsibility. The expected counts do not prove that an exemption's
reason is still sound. The composition of time and randomness in
`commands/new.rs` remains temporary debt.

The forced-warning inventory is local evidence for the current host/target; it
does not establish every platform. The integration-test `make_fifo` and
recency sites are Unix-only, and current CI targets Ubuntu while the supported
local target is macOS. There is no remote CI result until a separately
authorized push. This gate does not claim package separation or full
architecture adoption.
