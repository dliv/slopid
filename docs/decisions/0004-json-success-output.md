# 4. JSON Success Output By Default

Date: 2026-05-31
Status: Accepted
Amended by: [ADR 0006](0006-opt-in-human-output-for-discovery.md), which allows
opt-in non-contractual `--human` output on discovery commands.

See also [ADR 0007](0007-tolerant-reader-results.md), which distinguishes a
complete machine-readable lint result at exit 1 from an operational failure.

## Context

`sid` is intended to be agent-first. Its first useful command, `sid new`, returns
structured allocation data that agents need to consume reliably: title, slug,
period, ref, id, path, and dry-run status.

Many established human/operator CLIs make JSON opt-in with a flag. That is good
precedent for JSON as a machine-readable format, but it is weaker precedent for
an explicitly agent-first tool. If agents must remember an extra flag to get the
machine contract, the machine contract is not the default path.

At the same time, Unix-style stream separation remains useful: stdout is for
successful command output, while stderr is for diagnostics.

## Decision

Successful command stdout is JSON by default in v1.

For now, JSON is the only supported success output format. Remove the
user-facing `--json` switch rather than keeping an unused dual-output path.
Consider `--human`, `--raw`, or additional output modes only after real use shows
they are needed.

Success JSON fields are part of the command contract. New success-output fields
require an intentional contract update.

Human-readable errors and diagnostics remain on stderr. Successful stderr is not
promised to be empty, but failure stdout should stay empty so agents do not
consume partial success data.

`sid agent-instructions` follows the same rule by returning a JSON envelope:

```json
{
  "format": "markdown",
  "text": "..."
}
```

The `format` value should be produced from a named constant or enum, not an
incidental string literal.

## Consequences

- Agents can parse stdout without remembering an output flag.
- Shell users can pipe successful stdout directly into JSON tools.
- Human-readable diagnostic text remains out of stdout.
- Human-oriented success output is deferred until there is evidence that it is
  needed.
- Any future non-JSON or raw-document output mode needs a deliberate contract
  update.
