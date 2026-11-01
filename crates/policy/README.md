# Slopid policy tooling

[Architecture](../../docs/architecture.md) owns the rules and limits.
`src/lib.rs` observes Cargo metadata for an explicit consumer root. It returns
raw JSON/error and owns the one narrowly expected tooling spawn.
`tests/pure.rs` selects three Pure files and calls arch-gates from a pinned
local Git dev-dependency. Run `just check` at the product root: it includes
workspace tests/Clippy and the native compiler controls. Python 3.11+ is needed.

This is the first Pure-source slice, not the later coverage, graph, Root or
package checks. File selection is local; shared scanner fixtures are tested in
the engine, and local valid/violation controls exercise the consumer call.
The local Git pin requires this machine and must be replaced and independently
qualified after engine publication. No runtime package depends on tooling.
