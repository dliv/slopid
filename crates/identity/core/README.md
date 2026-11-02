# Identity core

Pure folder-identity values and decisions. `allocation::plan_new` builds a
nonempty plan from an observed snapshot, period, prefix policy and supplied tails.
Config parsing consumes supplied text; name/selector parsing, resolve selection,
list filtering/order/rendering and lint interpretation return data.
`LintVerdict` preserves Healthy/Unhealthy/Incomplete while the CLI owns exit codes.

Plans keep their fields private; accessors expose execution data without allowing
empty candidate lists. RefPrefixPolicy validates its private prefix set at
construction. Six-digit periods remain shape-validated, with calendar validity
deferred. Generic ordered list keys let the filesystem caller retain SystemTime
ordering without importing the ambient time namespace into core.

External dependencies and permitted features are in the local
[architecture policy](../../../docs/architecture.md). Unit tests here own pure
boundaries; filesystem behavior belongs to [the sibling workflow](../fs/README.md).
See [the feature entrypoint](../README.md) for contracts and proof limits.
