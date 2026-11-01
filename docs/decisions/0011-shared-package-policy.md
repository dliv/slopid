# Shared package policy checks

Date: 2026-11-01
Status: Accepted locally; portable qualification pending

## Context

Slopid has a mixed runtime package and a Tooling policy member. Shared checks
must protect their boundary without pretending the later production split is
complete or importing another product's layout/feature policy.

## Decision

Parse locally observed Cargo JSON and manifest text through arch-gates from
policy integration tests. Keep Cargo invocation in the normal policy library,
which returns raw JSON/errors and remains under native compiler control.
Classify runtime Binary and policy Tooling; keep direction, Pure allowances
and future feature membership local. Reject production dependencies on policy
or external engine identities across normal/build/dev kinds, including aliases;
Tooling uses the engine only as a dev-dependency. Validate package classification,
workspace lint inheritance and dependency constraints in normal `just check`.

## Consequences

The runtime package still contains its recorded mixed modules. No runtime source,
CLI behavior, version or release workflow changes. Shared package checks are not
source-level or transitive purity proofs; declared feature checks do not prove
resolved/default capability safety. Local Git pins remain a nonportable
checkpoint until engine publication and independent qualification. Reconsider
local classifications/feature pins during the separately scoped production split.
