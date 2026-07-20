# 8. Deterministic Protocol And Controlled Mutations

Date: 2026-07-13
Status: Accepted

## Context

Tasks, parked seeds, explicit topics, raw notes, and task inboxes have distinct
identity, ownership, traversal, and mutation rules. Treating them as one
generic root registry would hide those differences and make compatibility
with the existing task allocator difficult to verify.

## Decision

Configuration uses purpose-named task, seed, note, topic, and queue tables.
Canonical discovery composes typed sources while keeping allocation and list
semantics explicit. Read/query commands return deterministic JSON and preserve
usable partial results with completeness findings. Mutation commands expose
bounded, inspectable state transitions; relink is preview-first and applies
independently verified files atomically. Raw capture and inbox queues remain
verbatim and outside relink.

Note capture writes at most one exclusive file and quarantines conservative
credential-pattern matches without echoing content. Seeds share allocation
identity but remain file-backed until graduation. Graduation preserves the
authored seed bytes through one atomic rename to `napkin.md`; it does not
fabricate a canonical task entrypoint or copy across filesystems.

Relink separates planning from execution. A plan retains scanned bytes and
non-overlapping destination spans. Write compares each file independently,
skips a whole raced file, applies offsets back-to-front, and atomically replaces
only that file while preserving permissions. This intentionally permits other
independently proven files to succeed. Queue artifacts remain verbatim because
their transient authored content is outside structural repair.

Workspace rituals and relevance or filing judgment remain outside Slopid.

## Consequences

Callers can distinguish canonical owners from queue artifacts without path
guessing. Existing task-only projects continue to work. Each new source or
mutation needs an explicit contract and synthetic standalone-project proof;
there is no generic source plugin mechanism or lifecycle/status model.
