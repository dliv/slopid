# Shared source scanners and native-policy consistency

Date: 2026-11-01
Status: Accepted; published Git dependency

## Context

The shared engine already checks Pure forms, coverage and import direction.
Remaining scanner duplication must be removed without importing Workforest's
Root permissions or trait acceptance criteria into Slopid.

## Decision

Invoke shared Root and external-reference discovery from policy integration
tests over Slopid's existing map. Keep Root environment names and Pure external
allowances local, with stale external-allowance detection. Root does not permit
current_exe because Slopid's native policy does not control it. Retain the
existing native compiler adapter, fixtures, inventory and exact call-site
exemptions. Add a local configuration baseline check and pair permitted Root
environment functions with their native bans. Use a policy dev-only TOML
reader; no engine types or parsing enter the normal observation library.

## Consequences

No runtime, CLI behavior, version or release change. Trait discovery is shared
machinery; Slopid adopts no new trait restriction in this slice. Source checks
retain bounded grammar and cannot prove transitive purity, semantic ownership
or runtime frequency. Native config consistency supplements real compiler
controls rather than replacing them. Engine Git pins name the reviewed public revision over HTTPS and require no
adjacent clone. Independent exported-checkout gates qualify the current host;
remote CI and other platforms remain separate evidence.
