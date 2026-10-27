# 9. Identity-Only Lint

Date: 2026-10-22
Status: Accepted

## Context

Folder allocation and task-memory document validation are separate contracts.
Consumers such as Slopdeck already own document metadata, relationships,
inboxes, notes, and views. Repeating those rules in Slopid couples its identity
namespace to one workflow and prevents a consumer from composing an honest
coverage report when Slopid cannot inspect part of that namespace.

## Decision

Slopid lint audits direct-child allocation identity only. It emits a typed
report with independent `complete` and `healthy` verdicts, including JSON on
operationally incomplete exit 2. The four identity finding codes and status
rules are defined in contract 0007. Root discovery exposes the same active
root that allocation uses without creating it.

This identity lint boundary shipped in v0.4.0. The unreleased identity-only
candidate removes the remaining document-reader and workflow commands; it has
no legacy lint mode or config/list compatibility aliases. Slopdeck must be
qualified as its paired consumer before any later adoption or release.

## Consequences

Consumers of v0.3.x lint must update their report decoding and use their own
task-memory validation layer. Adding another finding code requires coordinating
closed-enum decoders. The identity-only candidate leaves document and workflow
validation to consumers.
