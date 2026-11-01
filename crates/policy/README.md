# Slopid policy tooling

[Architecture](../../docs/architecture.md) owns the rules and limits.
`src/lib.rs` observes Cargo metadata for an explicit consumer root. It returns
raw JSON/error and owns the one narrowly expected tooling spawn.
`tests/checks/pure.rs` selects Pure files from the common local map and calls arch-gates from a pinned
local Git dev-dependency. Run `just check` at the product root: it includes
workspace tests/Clippy and the native compiler controls. Python 3.11+ is needed.

`tests/checks/graph.rs` invokes shared coverage/direction over the full map in
`tests/support/mod.rs`, with consumer-owned workspace/file observation and
exact transition allowances. Root profiles and package checks remain later work. File selection is local; shared scanner fixtures are tested in
the engine, and local valid/violation controls exercise the consumer call.
The local Git pin requires this machine and must be replaced and independently
qualified after engine publication. No runtime package depends on tooling.
