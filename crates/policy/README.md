# Slopid policy tooling

[Architecture](../../docs/architecture.md) owns the rules and limits.
`src/lib.rs` observes Cargo metadata for an explicit consumer root. It returns
raw JSON/error and owns the one narrowly expected tooling spawn.
`tests/checks/pure.rs` selects Pure files from the common local map and calls arch-gates from a pinned
public Git dev-dependency. Run `just check` at the product root: it includes
workspace tests/Clippy and the native compiler controls. Python 3.11+ is needed.

`tests/checks/graph.rs` invokes shared coverage/direction over the full map in
`tests/support/mod.rs`, with consumer-owned workspace/file observation and
no transition allowances. Root and external-reference checks in `tests/checks/source.rs` use that same
map and local permissions/allowances. `tests/checks/native.rs` guards Slopid's
method/macro/lint baseline and pairs Root environment permissions with native
bans. File selection is local; shared scanner fixtures are tested in
the engine, and local valid/violation controls exercise the consumer call.
The immutable engine pin is fetched over HTTPS from the public
[arch-gates repository](https://github.com/dliv/arch-gates), with no adjacent
clone or local override. Fresh exported-checkout gates qualify this host;
remote CI and other platforms are separate evidence. No runtime package depends
on tooling.

`tests/checks/packages.rs` classifies actual Cargo members and invokes shared
package dependency checks. Support uses shared metadata/manifest parsing over
locally observed inputs, preserving unknown-member coverage and fail-closed
reads. Production cannot depend on local policy or external engine packages,
including aliases and dev/build declarations. Slopid's feature/role direction and dependency lists remain local. The engine is still a Git dev-dependency;
no shared types enter the normal observation library.

The role map includes identity core/filesystem and the explicit second-root and
disk-inspector test sources. Core is a real Pure build unit; all mixed-root and
resolve-to-failure exceptions are removed. See [feature ownership](../identity/README.md).

Source roles are checked against the actual owning member and the single local
package-class table. Exact named proof-source roles are validated for presence,
uniqueness and ownership. A source-map edit cannot quietly exempt a Pure-package
file from purity; normal policy regressions cover this agreement.
