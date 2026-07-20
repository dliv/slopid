# Contract 0003 - `sid list` discovery slice

Date: 2026-06-12
Status: Active

## Purpose

Make existing ids easy for humans and agents to find (roadmap milestone 3).

```text
sid list
sid list <term>...
sid list --sort id
```

## Semantics

- `sid list` reports the same namespace view allocation uses: every direct
  child with a recognized five-character ref across the active root and the
  configured scan roots, using the same `.sid` discovery as `sid new`
  (upward walk, nearest config, paths resolved at the project base).
- Name-based only. File types are never inspected, so non-directory
  reservations (zipped task-folder exports) are listed alongside folders.
- Slugless entries report an empty `slug`.
- Period-prefixed names with non-recognized ref segments are not listed
  (mirrors the scan policy in contract 0001).
- Positional arguments are search terms (owner request 2026-06-12). Each
  term matches an entry when the `sid_ref` starts with it or the slug
  contains it, case-insensitively; multiple terms AND together. A bare ref
  prefix therefore behaves as before. No matches is success with an empty
  list, not an error.
- Default sort is most recently touched first (owner request 2026-06-12).
  An entry's touch time is the later of its own mtime and its direct
  children's mtimes, so editing a file inside a task folder surfaces the
  folder; deeper edits do not count (no recursion, by design). Metadata
  errors degrade the touch time toward the epoch — they never fail the
  listing. Ties break ascending by `id`, then `path`.
- `--sort id` gives the stable order: ascending by `id`, then by `path` for
  identical ids in different roots. Within a period this is allocation
  order.
- A missing root contributes nothing; an unreadable root fails closed with
  the shared "scan task root" diagnostic (same policy as `sid new`).

## Output Contract

- Success stdout is JSON (ADR 0004) with exactly one key, `tasks`: an array
  of entries each having exactly `id`, `period`, `sid_ref`, `slug`,
  `modified`, `root`, and `path` (`root` is the absolute path of the
  containing root; `path` is the absolute entry path; `modified` is the
  touch time as RFC 3339 local time with seconds precision).
- Failure stdout is empty; diagnostics are human-readable stderr, exit 1.
- `--human` (ADR 0006) replaces the JSON success output with one
  column-aligned plain line per task — local touch time (minutes), id,
  project-relative root — newest first by default; no matches prints
  nothing. The plain formatting is explicitly not a machine interface and
  may change without a contract update; agents must parse the default JSON,
  which is unaffected by the flag's existence.

## Test Matrix

| Constraint | Status |
| --- | --- |
| Empty project lists `{"tasks": []}` | tested |
| Entries across active and all scan roots; `--sort id` order | tested |
| Default recency order, including direct-child touch propagation | tested |
| Non-directory reservations sort by their own mtime | tested |
| Exact per-entry and top-level JSON keys | tested |
| Slugless entries report empty slug | tested |
| Non-directory reservations (zip exports) are listed | tested |
| Non-recognized ref segments excluded | tested |
| Ref-prefix terms, including no-match success | tested |
| Slug-word terms: AND semantics, case-insensitive, partial words | tested |
| Unreadable scan root fails closed | tested |
| Upward `.sid` discovery applies to list | tested |
| `--human` plain lines; default output stays exact JSON | tested |
| `--human` with no matches prints nothing, exit 0 | tested |
