# 7. Tolerant Reader Results

Date: 2026-07-12
Status: Accepted

## Context

STM is an evolving file corpus. One malformed entrypoint must not prevent an
agent from resolving a valid task or reading the valid portion of its graph.
At the same time, silently omitting possible nodes or edges would make partial
context look authoritative. ADR 0004's general rule that failures have empty
stdout is too coarse for a completed audit whose result is “data defects were
found.”

## Decision

Reader discovery is tolerant. `sid resolve` ignores unrelated invalid entries.
`sid graph` returns valid reachable context and sets `complete: false` with
fixed-shape findings whenever the corpus scan may have omitted distinct
context. Query filters do not affect this corpus-level completeness signal.

`sid lint` distinguishes data from operations. A completed scan exits 0 when
clean (or warning-only) and exits 1 with complete JSON when error findings
exist. An unreadable configured root or entrypoint is an operationally
incomplete scan: lint exits 2, writes no stdout, and explains the failure on
stderr. Other command failures retain exit 1 with empty stdout.

## Consequences

Agents can use healthy context in an imperfect corpus without mistaking it for
a complete view. Stable codes and locations make lint automation possible.
Exit 1 no longer universally means “no usable JSON” for `sid lint`; callers
must follow the command-specific contract.
