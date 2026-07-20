# Agent Notes

This repo uses test-driven development. When changing behavior, write or update
the executable tests first, then make the smallest implementation change that
satisfies them.

## Testing

For numeric bounds and other thresholds, test both sides of the boundary. For
example, if a rule changes at `n > limit`, include one test proving `limit` is
still accepted and another proving `limit + 1` is rejected.

Prefer deterministic tests over clock, random, or race-dependent assertions. If a
race matters, add a small seam that makes the raced observation injectable rather
than relying on timing.

Separate test-contract work from implementation work when running TDD
experiments. A branch can be successful at making a strong test suite pass while
still needing follow-up for non-test plan items such as docs, ADRs, or cleanup.

## Planning Notes

`stm/` is short-term memory for active feature work. It is appropriate to collect
decisions there while a feature is open, but durable decisions should move to the
proper long-lived docs before the feature closes.

Use `tmp/` for review prompts, scratch notes, and transient agent responses.

When using a grill/review loop to make a patch plan, collect the answers first
and record the locked decisions in the active `stm/` task doc before editing
tests or implementation. Do not let `tmp/` become the source of truth for
requirements.
