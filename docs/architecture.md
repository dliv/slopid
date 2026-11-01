# Current architecture and compiler gate

Date: 2026-11-01
Status: Active for Sid 0.5.0

This document owns Slopid's current compiler rules, recorded direct-use
exceptions, enforcement classes, and the limits of the gate. The accepted
longer-term package shape is recorded in
[ADR 0010](decisions/0010-compiler-backed-native-architecture-gate.md); this
gate is its first bounded step, not the completed package split.

## Current boundary

Slopid has the `slop-id` runtime package and a test-only `slop-id-policy`
workspace member. The `sid` CLI remains its application. `just check`
is the local complete gate. CI calls that same recipe after installing the
stable `rustfmt` and Clippy components, Python 3, and just 1.57.0.

The recipe runs, in order:

1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --locked --all-targets -- -D warnings`
3. `cargo test --workspace --locked`
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
keeps production, test and tooling sites in separate groups by relative file, lint code,
canonical banned item, and count. On the supported macOS and Linux targets, it
expects eight production method sites, five test method sites and one tooling
method site. All macro site counts are zero. The tooling site is Cargo
observation in `crates/policy/src/lib.rs`, narrowly expected at that function. The Unix-only test entries mean that another target needs
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

The Python 3.11+ adapter reads Cargo JSON compiler messages; it does not parse Rust
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

## First shared Pure-source check

The policy test in `crates/policy/tests/checks/pure.rs` reads the three currently
selected Pure files: `src/refcode.rs`, `src/slug.rs`, and
`src/commands/agent_instructions.rs`. It passes their source to arch-gates'
`check_pure_source`, failing on named ambient source forms. The local caller
owns reads and file selection; missing files fail. A valid/planted control
and a fail-closed Cargo observation test run in the same workspace gate.

The engine is a dev-dependency of policy only. Engine calls stay in integration
tests; the normal policy library returns raw metadata and never imports engine
types. The runtime package does not depend on policy or engine. Both consumers
currently pin an immutable **local Git** revision: this checkpoint needs this
machine's engine checkout and has no portable/remote-CI qualification yet.
Publication and replacement with a published revision remain separate work.

Source-level external Pure dependency restrictions and Root profiles now run
through the shared scanner, as described below. Trait acceptance has not been
adopted for Slopid; Workforest's no-traits rule is not imported. Package dependency
evaluation is described below. Source coverage and import direction are now
covered by the shared checks described below. Pure source scans
skip recognized test modules and do not resolve types, expand macros or inspect
dependency internals. Fixed build inputs remain permitted. The selected files
passing this scan do not establish transitive purity or completed package split.
The remaining mixed modules and root-owned time/random inputs stay explicit
follow-up work. See [policy tooling](../crates/policy/README.md).

## Source coverage and import direction

`crates/policy/tests/support/mod.rs` owns Slopid's full 16-file map and the
Root/Pure/Effectful role matrix. Policy tests obtain all workspace members from
local Cargo metadata and walk every non-policy member's `src` directory,
including newly added product members. Missing reads/metadata are failures.
The shared coverage evaluator rejects missing, stale, duplicate or malformed
classification entries. Pure selection reads that same local map.

Root may reference production roles; Pure references Pure; Effectful references
Pure or Effectful. The shared direction evaluator applies this local predicate
and exact exceptions. `commands::resolve -> failure` temporarily permits use of
AmbiguousCandidate data colocated with root output/exit. Remove it when outcome
types move; stale/duplicate exceptions fail. Nine named Effectful files are
explicit temporary allowances in the mixed root package; the local test rejects
stale or added allowances. This is not the completed package split.

These checks run in the single `policy` integration test target in normal
`just check`. The engine follows crate/self/super paths and the donor's tested
inline-module grammar; it ignores bare child names and child edges and does not
expand macros. Coverage proves conventional Rust-file inventory, not complete
Rust module resolution. Import discovery cannot establish a complete semantic
graph or absence of cycles. Native compiler gates retain their separate claims.

## Shared metadata and package dependencies

Local support reads raw Cargo metadata through the existing normal-library
adapter and calls `arch_gates::metadata::parse_members`. Only explicit workspace
IDs are selected; absent/duplicate IDs or malformed selected records fail.
Support reads manifests and supplies text for shared lint-inheritance facts.
Coverage now uses this same member model; source reads and subprocess work
remain consumer-owned and the engine stays a policy dev-dependency.

`tests/checks/packages.rs` classifies the current runtime package Binary and
policy package Tooling. It retains the mixed-root transition rather than claiming
Pure/effectful build units exist already. The shared evaluator rejects missing,
duplicate or stale package classes, missing workspace lint inheritance, outside
member path dependencies, and forbidden local/external Tooling targets. Production
cannot depend on `arch-gates` under an alias or in normal/build/dev tables;
Tooling may use it only as a dev-dependency. Slopid owns internal direction/reason
policy; the engine does not prescribe its layout. Declared/forwarded Pure feature
checks apply once Pure build units are classified; this does not newly restrict
all dependencies of today's mixed runtime package.

Normal `just check` runs the package test alongside coverage/direction/Pure tests.
The product runtime, version and release workflow remain unchanged. Local Git
pins still require publication and independent portable qualification before
consumer pushes. See ADR 0011 and the policy README.

## Shared Root and external-source checks

The local `tests/checks/source.rs` scans all four Root files selected by the
same role map. Root forbids filesystem, network, thread, path-query and escape
forms. Its only permitted environment names are `args_os`, `ArgsOs`, `var`,
`var_os` and `current_dir`; namespace imports are checked at their uses.
`current_exe` remains forbidden. Process, time and stream forms are left to the
existing compiler controls, which continue to inventory their sanctioned sites.
The shared engine discovers forms from supplied source; this consumer reads
files and chooses permissions. Missing reads and empty selections fail.

Pure files may refer only to `anyhow` and `serde` among declared external
normal dependencies. The source checker uses dependency names from each file's
own package manifest, including ordinary renamed import names. Unused Pure
allowances fail. This list is shared with local package policy; it does not
claim dependency internals are pure or that a mixed package is a Pure build unit.

`tests/checks/native.rs` parses the real Clippy and root Cargo configuration.
It requires Slopid's adopted method/macro baseline and denied workspace lint
levels, and pairs each permitted Root environment function with its native ban.
It accepts stronger lint levels. The `ArgsOs` type needs no method ban. These
are Slopid's controls, not the union of consumer bans. The existing Python
compiler adapter, five compile-only controls and site inventory remain the
assembled compiler proof. Configuration checks alone do not prove a lint fires.

Shared Root/external/trait/token fixtures run in the engine's own suite;
Slopid invokes its actual-source checks from normal `just check`. Trait
discovery is available to consumers, but this slice adds no Slopid trait
restriction. Source grammar, macro/type-resolution and transitive-purity limits
remain as above. See ADR 0012.
