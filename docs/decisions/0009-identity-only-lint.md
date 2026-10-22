# 9. Identity-Only Lint

Date: 2026-10-22
Status: Accepted; supersedes the lint portion of ADR 0007

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

This accepted identity boundary ships as the pre-1 breaking v0.4.0 contract.
There is no legacy workflow-lint mode. Other existing commands remain in this
release; deciding whether to remove them is separate work.

## Consequences

Consumers of v0.3.x lint must update their report decoding and use their
own task-memory validation layer. Consumers of the existing identity report
can now obtain it from a tagged release. Adding another finding code requires
coordinating closed-enum decoders. Document readers retain tolerant partial
results and corpus-level findings; context depth affects selection only.
