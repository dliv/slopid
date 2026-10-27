# Contract 0003 — identity folder discovery

Date: 2026-06-12
Updated: 2026-10-26
Status: Active for the unreleased identity-only candidate

## Commands

```text
sid list [TERM]...
sid list --sort id
sid resolve <REF-OR-FOLDER-NAME>
```

The nearest upward `.sid` supplies one logical root; with no config, the root is
`stm` under cwd. The root need not exist. A missing root is an empty snapshot;
a root, iterator, or matching-entry metadata error fails closed before a result
is printed.

## Folder snapshot

List and resolve share a complete direct-child snapshot. They parse identity
names and classify matching entries with metadata that does not follow the
final symlink. Only real directories appear. Matching files, directory
symlinks, broken symlinks, and other object types remain allocation/lint
reservations but are not navigable. Nested matching directories are never
visited. This snapshot does not lease the filesystem against later replacement.

Each list row contains exactly `id`, `period`, `sid_ref`, `slug`, `modified`,
`root`, and `path`. `id` is the complete folder name; `sid_ref` is the short
recognized ref. Slugless legacy folders report an empty slug. Paths are
absolute joins beneath the discovered logical root.

`modified` is an RFC 3339 local timestamp with seconds precision, derived from
the folder and its direct children only. Deeper writes do not affect recency;
child symlink targets are not followed. Recency-only errors degrade to the
oldest timestamp without failing the otherwise complete snapshot.

Search terms are ANDed. Each term must match the ref prefix or a slug substring,
case-insensitively. The default order is recency descending, then `id` and path
ascending. `--sort id` orders by `id`, then path. `--human` replaces only
successful JSON output with non-contractual plain columns.

## Resolve

Resolve accepts an exact case-sensitive recognized short ref or exact complete
identity folder name. Paths, prefixes, case-folded refs, and path separators
are invalid. Selectors are matched against enumerated folder names, never
joined with the root as user-supplied paths. A unique match returns
`{"folder": ...}` with exactly `id`, `period`, `sid_ref`, `slug`, `root`, and
`path`; there is no recency field and no document content is opened.

No matches is `sid-resolve-not-found`. Multiple real directories for a short
ref is `sid-resolve-ambiguous`, with candidates sorted by full `id` then path.
The complete folder name still resolves one directory in an ambiguous ref
namespace. Files and symlinks sharing the ref do not add ambiguity, though
lint continues to report their duplicate reservation.

## Output and test matrix

List success is JSON with exactly one key, `folders`, containing rows with the
exact fields above. Pre-result failures use contract 0007's structured stderr
envelopes and empty stdout. List snapshot failures use `sid-list`; resolve
snapshot failures use `sid-resolve-unavailable`; selector failures retain
distinct invalid, not-found, and ambiguous codes.

Tests cover slugged and slugless real directories; files, directory symlinks,
broken symlinks, nested directories, invalid refs, missing roots, and roots
that are files; deterministic iterator and matching-metadata failures; direct
child versus deep and symlink-target recency; AND filters, sorting, exact row
keys, human output, same-ref ambiguity, full-name recovery, file reservations,
invalid selectors, and no-read FIFO entrypoints.
