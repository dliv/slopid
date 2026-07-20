# sdd4c: discovery commands (roadmap milestone 3)

## Why

Owner request 2026-06-12: after dogfooding `.slow` ("allocated via sid and
moved it to .slow"), two gaps surfaced — no way to allocate directly into a
scan root, and no list/search command at all ("I've been meaning to ask, is
there a list / search type command?"). Milestone 3 planned exactly this and
the shared scanner (src/scan.rs) was extracted as its prep.

## Thread

- `sid list [TERM]...`: JSON list of every recognized-ref entry across
  the active root and configured scan roots (the allocation namespace view:
  name-based, file types never inspected, so zip reservations appear).
  Most recently touched first by default (entry + direct-child mtimes,
  `--sort id` for allocation order; owner request same day); each term
  filters by ref prefix or slug substring, case-insensitive, ANDed; a
  `modified` timestamp is included; empty list is success.
- `sid new --into <root>`: allocate directly into a configured scan root
  (`--into .slow`, `--into stm/.slow`). Validated against the configured
  list by component-wise suffix match; no match or ambiguous match fails
  closed listing the valid roots. The destination root is created on a real
  run (explicit intent), like the active root.

## Checklist

- [x] Integration tests first (TDD): list across roots, slugless + zip
      entries, prefix filter, unreadable root fails closed, upward-config
      discovery; --into shorthand + full form, dry-run creates nothing,
      invalid/ambiguous destinations fail closed.
- [x] Implement: scan.rs children carry name+path again; commands/list.rs;
      cli List variant + New --into; ProjectConfig carries the project base
      for readable error messages.
- [x] Contract 0003 (sid list); contract 0001 amendments (--into, create
      rule); ROADMAP milestone 3 Done; prune settled "sid list" question.
- [x] Gate + adversarial verification fleet; archive this folder.

## Out of scope

- `sid find` / content search; period filters (defer until needed).
- stm parity (`stm` has `--root` and no config).
- Recursive scanning (explicitly deferred by default).
