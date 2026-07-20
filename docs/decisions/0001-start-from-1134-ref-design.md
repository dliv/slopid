# 1. Start From 1134 Ref Design

Date: 2026-05-31
Status: Accepted

## Context

`sid` creates local task folder ids for agent and human work. The folder tree is
part of the user interface: Finder, editors, terminal listings, and text search
should all remain useful without a separate database.

The ref portion of the id was explored in detail before the bootstrap slice.
That exploration is recorded in `handoff-docs/REF_DESIGN_HANDOFF.md`. It settled
on a short `sXXXX` ref that combines chronological sorting with a small random
tail and a local, folder-safe visual shape.

## Decision

Treat the previous ref-design spike and `handoff-docs/REF_DESIGN_HANDOFF.md` as
the implementation starting point.

Folder ids are shaped like:

```text
{YYYYMM}_{ref}_{slug}
```

where `ref` is a short `sXXXX` token and the recommended default is the 1134
mixed-radix design from the handoff. The slug gives human context, but the ref
is the durable short handle. Slugs must not become an identity fallback.

The folder tree should sort by period and, within a month, usually by allocation
order. A sortable folder-tree UI is a core goal, not cosmetic polish. Agents and
humans should be able to cite and search short refs such as `sXXXX`.

Once active, pending, and archive scan roots exist, they share one namespace.
Moving a task between those roots must not casually free its ref for reuse in the
same period.

Do not redesign refs casually. Reopen the scheme only with evidence from real
use or an owner decision.

## Consequences

- The 1134 handoff is the source of truth for generated and recognized ref
  shapes until a later ADR supersedes it.
- The default id shape remains period first, then ref, then slug.
- Implementation and tests should preserve the distinction between ref identity
  and slug context.
- Simpler fallback designs such as SEQ22 remain known options, but are not the
  default path.
