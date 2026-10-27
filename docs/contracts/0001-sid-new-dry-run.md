# Contract 0001 — identity allocation

Date: 2026-05-31
Updated: 2026-10-26
Status: Active for Sid 0.5.0

## Purpose

`sid new "task title"` allocates one folder identity. The folder name is
`{YYYYMM}_{ref}_{slug}`; existing slugless names remain recognized. The
folder tree is canonical state. Slopid owns identity and allocation, while a
consumer such as Slopdeck owns authored documents and lifecycle metadata.

This contract records the v0.5.0 breaking change from v0.4.0: config is now
flat, list and resolve navigate real directories, and no legacy config or wire
aliases are accepted. The release pairs with Slopdeck 0.1.1-alpha.19; workspace
adoption requires caller and configuration migration.

## Project config

The nearest upward `.sid` is TOML with only these fields:

```toml
root = "stm"

[ref]
deny_prefixes = ["sex", "shat"]
```

- `root` is optional and defaults to `stm`. Without `.sid`, cwd is the project
  base. Configured paths resolve against the directory containing `.sid`.
- Root paths must be relative, contain no `..`, and name at least one normal
  path component. Unknown top-level keys and unknown `[ref]` keys fail closed.
- A missing or dangling config path is not skipped. A configured root symlink
  is followed; a missing or dangling root scans empty, while creation through a
  dangling root fails.
- Omitted `deny_prefixes` uses the built-in reviewed preset. A present list
  replaces it exactly; an empty list disables filtering. Each entry must be a
  unique reachable generated prefix.
- `sid init` exclusively creates cwd `.sid` with exactly `root = "stm"` and
  returns `path` and `created`. It never overwrites an existing file or symlink.

## Allocation

Allocation and lint scan the names of every direct child under the one root.
They do not stat entries or read authored content. Hidden and non-period names
are ignored. A six-ASCII-digit period followed by a recognized ref reserves
that ref regardless of whether the child is a directory, file, or symlink.
Invalid refs do not reserve allocation refs; lint still reports them.

The sequence is partitioned by period. Periods are exactly six ASCII digits;
calendar validity is deferred. Ref recognition remains broader than ref
generation so old recognized refs continue to reserve names. Generated refs
retain the internal 1134 alphabet, reachability and tail rules documented in
[`REF_DESIGN_HANDOFF.md`](../../handoff-docs/REF_DESIGN_HANDOFF.md).

The planner starts at the next unoccupied sequence, advances past sequences
with no policy-allowed ref, and stops after sequence 659. It shuffles the
complete 30-by-30 tail space without replacement and retains at most 100
viable unoccupied candidate paths. Invalid and denied tails do not consume that
budget. The executor creates the root as needed, then uses exact-path
no-clobber directory creation and retries `AlreadyExists` against the next
candidate. Same-ref/different-slug concurrent allocation remains an accepted
best-effort race; no global lock or lease is provided.

`--dry-run` returns the same allocation plan without creating the root or
folder. The hidden six-digit `--period` option is a deterministic test seam,
not advertised help. Slugs are ASCII kebab-style, nonempty, and truncate
deterministically to 48 bytes without a trailing dash.

## Output and errors

Successful `new` JSON has exactly `title`, `slug`, `period`, `sid_ref`, `id`,
`path`, and `dry_run`. `init` has exactly `path` and `created`. Errors before a
result use the structured stderr envelope and code set in
[contract 0007](0007-root-and-identity-lint.md). Allocation failures use
`sid-new`; config failures use `sid-config`.

## Test matrix

Tests cover the six-digit boundary and non-calendar values; recognized versus
generated refs; period partitions; omitted/replaced/empty/invalid prefix
policies; denied-prefix reachability; 659 success and 660 exhaustion; the 100
candidate budget; randomized exhaustive tails; slug length and empty-slug
boundaries; name-only file and symlink reservations; missing, real, and
symlinked roots; dangling-root creation failure; no-write dry-run; and exact
path collision retry. CLI tests cover nearest config, malformed and dangling
config, safe root validation, exact JSON keys, structured error channels, and
exclusive `init`.
