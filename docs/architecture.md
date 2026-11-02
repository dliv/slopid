# Architecture and compiler gate

Date: 2026-11-01. Sid 0.5.0 retains its CLI contracts while separating one
folder-identity feature into build-enforced roles. [Feature entrypoint](../crates/identity/README.md),
[app](../src/README.md), [ADR 0013](decisions/0013-identity-core-and-filesystem.md).

## Build and ownership rules

| Package | Role | Allowed production dependencies |
| --- | --- | --- |
| slop-id | Binary/app | Identity core/filesystem and its concrete CLI, time, random and serialization libraries; never tooling or another app. |
| slop-id-identity-core | Pure, feature identity | Own core and approved anyhow, serde derive, TOML parse with default features disabled. No filesystem/app dependency. |
| slop-id-identity-fs | Effectful, feature identity | Own feature core and anyhow. No app/tooling or ambient time/random library dependency. |
| slop-id-policy | Tooling | Published immutable arch-gates Git dev-dependency and local check dependencies. No production unit depends on tooling. |

All packages inherit workspace lint settings. Each internal member is
unpublished at 0.0.0; root remains slop-id 0.5.0 with bin sid and root version
embedding. Member addition requires explicit classification. Path dependencies
must remain within listed members. Cross-feature and app/tooling edges are
rejected. Normal/build/dev declarations and ordinary aliases cannot smuggle local
policy or external arch-gates into production. Pure declared/forwarded dependency
features are checked; resolved feature unification remains a stated limit.

Core owns values/plans/outcomes, config parsing, name/selector interpretation,
filter/order/render decisions and lint interpretation. Filesystem owns business
sequencing and operation-local I/O. App owns process inputs, period, lazy entropy,
per-date local-time conversion, output and exit. Standard concrete callbacks
supply the last two capabilities without custom interface hierarchies. See the
[app dispatch contract](../src/README.md) for semantic ownership review.

No mixed-root allowance or reverse resolve-to-failure exception remains. Candidate
data belongs to core; its typed resolve outcomes and lint verdicts map exhaustively
at the root. Terminal diagnostics remain coarse where no caller action changes.
Existing public root refcode/slug/scan paths are re-export facades, not a new
public-library-mode contract. No Slopid trait restriction is adopted.

## Normal gate

`just check`, also invoked literally by CI after tool installation, runs:

1. cargo fmt --all -- --check
2. cargo clippy --workspace --locked --all-targets -- -D warnings
3. cargo test --workspace --locked
4. Python adapter tests
5. python3 scripts/check_clippy_policy.py

The release workflow validates exact tag/Cargo/commit identity and runs the same
complete `just check` before its two macOS builds; packaging additionally
checks architecture, executable version and archive content. [Policy tooling](../crates/policy/README.md)
owns source/package check navigation; [scripts](../scripts/README.md) owns native
compiler-control mechanics. Python 3.11+ is required. CI pins just 1.57.0.

## Native controls and sanctioned sites

Root clippy.toml bans args/args_os/var/var_os/current_dir/set_current_dir;
SystemTime/Instant now, chrono Local/Utc now, fastrand shuffle; process exit/abort/
Command::new; stdio stdin/stdout/stderr and print/println/eprint/eprintln/dbg.
Cargo denies disallowed_methods/disallowed_macros and unfulfilled expectations.
Function-local expectations name their boundary and reason. Workspace/all-target
Clippy and forced-warning inventory cover all compiled members and the second
root. The expected production call sites are:

| File/function | Banned item(s) | Count |
| --- | --- | ---: |
| src/lib.rs main_entry/current_dir | process::exit; env::current_dir | 2 |
| src/failure.rs report_failure | io::stderr; process::exit | 2 |
| src/failure.rs write_json/write_text | io::stdout | 2 |
| src/inputs.rs current_period/randomized_tail_space | chrono::Local::now; fastrand::shuffle | 2 |

Test support retains two Command::new, two Instant::now and one SystemTime::now
in tests/identity_cli_test.rs, including Unix-only FIFO/recency cases. Tooling has
one Command::new in crates/policy/src/lib.rs. All macro inventories are zero.
The exact machine-readable inventory is scripts/clippy_policy_expectations.json.
All five compile-only controls preserve actual workspace member manifests,
lockfile and lint context; missing members/broken resolution are not successful
policy rejections.

## Source and package enforcement

The local support map classifies every conventional production Rust source and
explicitly includes both files under identity/fs/test-support/src. The second
root is Root and its disk inspector is Effectful; test-only placement supplies
no automatic app exemption. Core is Pure, filesystem Effectful and root source
Root. Missing/stale/duplicate classifications fail. Source roles must also match the
actual owning workspace package class from the same local package table used
for dependency checks: Pure -> Pure, Effectful -> Effectful, Binary -> Root.
Only the two exact named test-support sources have explicit target roles;
unknown owners and missing/duplicate proof-role entries fail. Relabeling a
Pure-package file Effectful cannot remove its profile obligation. Policy tooling observes
explicit workspace metadata, including unknown members, reads sources/manifests
locally and passes data to the shared evaluator. Failures to read/parse/spawn
fail validation. Cargo observation stays local and inventoried; engine types and
calls stay in integration tests.

Root may reference production roles. Pure references Pure; Effectful references
Pure/Effectful. Coverage/direction have no temporary exceptions. Pure source
checks reject named effects and external references outside the local anyhow/
serde/TOML list; unused external allowances fail. Root forbids filesystem,
network, thread and path-query forms. Its allowed environment names are args_os,
ArgsOs, var, var_os and current_dir; current_exe is forbidden. Callable names pair
with the actual native ban list. Time/process/streams retain native controls.
Native consistency also requires the local adopted method/macro/lint baseline.

The shared engine is pinned over public HTTPS in policy Cargo.toml/Cargo.lock.
No adjacent clone or path override is required. Earlier fresh exported-checkout
qualification proves public dependency resolution on this host; a later product
split still needs its own local proof. Engine dependency tests do not run simply
because it is a dependency; consumer actual-source tests run in the normal gate.

## Enforcement kinds and limits

Gated: Cargo package direction, declared Pure features, member lint inheritance,
source coverage and role direction, Pure/Root named forms and external references,
native baseline consistency, sanctioned compiled locations/counts and five
compiler controls. Checkable: reasons match function responsibility, dispatch is
business-free, read/entropy/conversion ordering and callable public surfaces fit
ownership. Judgment: feature placement and whether a new abstraction earns its cost.

Scans target ordinary accidental drift, not malicious syntax. They follow the
bounded crate/self/super/inline-module grammar, omit bare child and some child
edges, skip recognized test modules and do not expand macros or resolve types.
Coverage is conventional file inventory, not complete Rust module semantics.
No transitive purity, cycle-freedom, dependency-internal effect or adversarial
security claim follows. A generic ordered list key is inert data; the filesystem
caller supplies original SystemTime ordering while app owns timezone conversion.

Native inventories prove direct compiled locations/counts, not call frequency,
resource ownership or reason quality. Root can call allowed helpers; semantic
review still checks their responsibilities. Compiler configuration checks alone
do not prove a lint fires. Current native inventory supports macOS/Linux; Unix
support needs a reviewed inventory on another target. Rust is not pinned, so
future stable lints can change warnings-denied results.

Core deterministic tests, real-resource workflows and the second caller prove
their own seams. Production CLI tests separately prove channels/exits/effects.
Neither snapshot navigation nor name allocation leases the filesystem. Same-ref/
different-slug concurrency and non-transactional config init retain their stated
limits. Consumer compatibility, installation, remote CI and another platform
need separate evidence. No release/deployment is implied by a local gate.
