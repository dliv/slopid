# Identity filesystem workflows

The public operations in `src/lib.rs` discover/init config, report roots,
allocate, list, resolve and lint. `scan` also retains the existing raw
period-prefixed scan API through the root compatibility facade. Other observation
and execution helpers stay private to this crate.

Operations use real filesystem resources and the sibling core. The app supplies
cwd/config, a selected period, lazy shuffled tails and per-timestamp local
formatting. Allocation scans before consuming tails, then plans before creating;
dry-run never creates root/folder. List takes a complete navigation snapshot,
filters, observes recency and formats retained rows, then sorts using raw times.
Config init remains exclusive but is not a transactional write.

Unit tests preserve no-follow snapshots, deterministic iterator/metadata errors,
name-only reservations, exact-path collision retry and recency fallback. The
`second_root` integration target at `test-support/src/second_root.rs` calls only
production core/filesystem APIs with disposable resources. Its companion
`inspect.rs` owns effectful disk assertions; policy separately classifies both
sources. This proves library composition, while the
[CLI tests](../../../tests/README.md) prove production app wiring.

See [feature ownership and limits](../README.md) and
[architecture](../../../docs/architecture.md).
