# Slopid policy tooling

[Architecture](../../docs/architecture.md) owns the rules and limits.
`src/lib.rs` observes Cargo metadata for an explicit consumer root. It returns
raw JSON/error and owns the one narrowly expected tooling spawn.
`tests/checks/pure.rs` selects Pure files from the common local map and calls arch-gates from a pinned
local Git dev-dependency. Run `just check` at the product root: it includes
workspace tests/Clippy and the native compiler controls. Python 3.11+ is needed.

`tests/checks/graph.rs` invokes shared coverage/direction over the full map in
`tests/support/mod.rs`, with consumer-owned workspace/file observation and
exact transition allowances. Root and external-reference checks in `tests/checks/source.rs` use that same
map and local permissions/allowances. `tests/checks/native.rs` guards Slopid's
method/macro/lint baseline and pairs Root environment permissions with native
bans. File selection is local; shared scanner fixtures are tested in
the engine, and local valid/violation controls exercise the consumer call.
The local Git pin requires this machine and must be replaced and independently
qualified after engine publication. No runtime package depends on tooling.

`tests/checks/packages.rs` classifies actual Cargo members and invokes shared
package dependency checks. Support uses shared metadata/manifest parsing over
locally observed inputs, preserving unknown-member coverage and fail-closed
reads. Production cannot depend on local policy or external engine packages,
including aliases and dev/build declarations. Slopid's direction policy and
mixed-root transition remain local. The engine is still a Git dev-dependency;
no shared types enter the normal observation library.
