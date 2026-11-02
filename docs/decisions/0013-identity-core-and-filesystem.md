# 13. Identity Core, Filesystem Workflows and CLI Composition

Date: 2026-11-01
Status: Accepted

## Context

Native/shared gates preceded code movement, but the runtime remained one mixed
package with an effectful-to-app data dependency and temporary clock/shuffle
placement. Folder identity is one responsibility; subcommands/config do not
justify more features. Meaningful failure distinctions already exist.

## Decision

Use a pure identity core and concrete filesystem workflow crate, retaining the
existing root package/bin as the CLI app and policy as tooling. Core owns shared
outcome data; filesystem operations own sequencing. App supplies selected period,
lazy shuffled tails after a successful scan and local conversion at each retained
list timestamp. Keep exact CLI contracts/error precedence and best-effort race
semantics. No custom storage/clock traits, shared Edge or public-library product
promise is introduced. New internal packages are unpublished at 0.0.0.

Remove mixed-root/reverse-data exceptions, classify actual sources and the
second proof root explicitly, and retain all native compiler controls. The
current rule inventory, allowed features and limits live in
[architecture](../architecture.md), with responsibilities/proof in the
[feature README](../../crates/identity/README.md).

## Consequences

Build dependencies now express the core/effects/app boundary. Fixed configuration
and workflow inputs permit a second real-resource caller without importing the
CLI. That proof does not replace production entrypoint tests. Per-date local
conversion preserves daylight offsets; one current offset would not. Atomic
mkdir remains exact-path-only; config creation is not transactional.

Revisit when a real new feature/protocol or a non-filesystem library-mode contract
is selected, not merely to allocate a new folder or add a CLI operation.
