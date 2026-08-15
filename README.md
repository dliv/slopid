# slopid

Deterministic local task memory for humans and agents.

The repository is named `slopid`; the command is `sid`. It allocates durable
task folders with short references, reads their relationships and captures,
and performs a small set of controlled mutations. Machine-readable JSON is the
default interface.

The code and documentation in this repository were at least 99% LLM generated.

## Install

```sh
brew tap dliv/tools
brew install sid
```

Or download a binary from [GitHub Releases](https://github.com/dliv/slopid/releases).

## Quick Start

Initialize a project-local `.sid` configuration:

```sh
sid init
```

Then allocate and inspect task memory:

```sh
sid new "investigate flaky deployment"
sid list --human
sid lint
sid search deployment
```

`sid` discovers the nearest `.sid` file by walking upward from the current
directory. Without one, it uses `stm/` beneath the current directory. Paths in
`.sid` are project-relative.

Generated refs default to the built-in `prude` prefix policy. A project can
replace it exactly—including disabling it with an empty array:

```toml
[ref]
deny_prefixes = []
```

Markdown destinations are literal by default, including filenames that end in
text such as `:33`. Projects that intentionally author terminal editor-style
line locators can opt into relink compatibility:

```toml
[relink]
destination_extensions = ["colon-line"]
```

With `colon-line`, `sid relink` recognizes only a terminal positive decimal
locator such as `:33` whose base does not end in `/`. A separator-ended spelling
such as `assets/:33` remains an ordinary literal path. For a recognized locator,
Slopid still gives an existing literal colon-suffixed file precedence; otherwise
it resolves and checks the base file while preserving the locator and any
following `#fragment`. It does not validate line counts.
Omitting `[relink]` or using an empty list preserves the default literal-path
behavior. Unknown extension names fail config parsing.

## Commands

```text
sid new                 Allocate or graduate a task folder
sid list                List task folders and reservations
sid resolve             Resolve one canonical id
sid graph               Read a relationship component
sid lint                Audit direct-child folder identity
sid search              Search authored text and captures
sid context             Read one task graph and pending inbox
sid captures            Inventory pending notes and seeds
sid note                Capture an identity-free note
sid seed                Create or preview a parked seed
sid relink              Preview or apply safe Markdown link repairs
sid agent-instructions  Print agent usage guidance
sid init                Write the default .sid configuration
```

Read/query commands emit JSON by default. `sid list --human` is an explicitly
non-contractual display mode for direct use. Mutations expose dry-run or preview
paths where applicable; `sid relink` does not write unless passed `--write`.

### Identity lint

`sid lint` audits folder identity and nothing else. It scans the direct
children of every configured allocation root — the task scan roots plus the
seed root, which all reserve one ref namespace — and reports four things:

```text
identity-folder-ref-invalid   valid six-digit period, unrecognized ref
identity-ref-duplicate        one ref reserved by more than one direct child
identity-root-unreadable      a configured root could not be enumerated
identity-entry-unreadable     a directory entry could not be read
```

The report is `{"complete": bool, "healthy": bool, "findings": [...]}`, with
locators relative to the project base. `complete` describes coverage and
`healthy` describes validity, so the process exits `0` for a complete healthy
report, `1` for a complete report containing errors, and `2` for an incomplete
one. Warnings alone stay successful.

Lint never opens `CURRENT_STATE.md`, an inbox message, a capture note, or a
topic document, and it never follows an entry: a period-and-ref-shaped file or
symlink is a valid reservation, because allocation must not reuse its ref. A
configured root that does not exist yet is an empty namespace. Entrypoint
frontmatter, relationship, inbox, and lifecycle-view health belong to the
task-memory layer above Slopid, not to this command.

### Move-projected relink

`sid relink` repairs every destination it can prove. To instead repair only what
one planned lifecycle move would change, scope it to a stable id and a configured
destination root:

```bash
sid relink --move sa2a7 --into .archive
sid relink --move sa2a7 --into .archive \
  --write --expected-plan-sha256 <plan_sha256-from-the-preview>
```

The preview reports the projected `from_owner`/`to_owner`, the scoped changes, and
a `plan_sha256` bound to the move's effect set: inbound links to the moving owner
plus links authored inside it whose source directory moves. Writing requires that
exact digest from a *fresh* preview and refuses before touching any file if the
plan changed or is incomplete. A partial write is forward-recoverable rather than
rolled back, so take another preview and apply the remainder. If the owner already
sits under the destination root, the same command verifies its scoped links
instead of planning a move.

`complete:true` means every move-caused repair in Slopid's declared authored
source coverage was classified and this scoped plan is safe to apply. It is not
a certificate that every local destination resolves. A generic destination
proven absent under both its current and projected readings yields
`relink-unresolved-local-destination` with `severity:warning`. The warning and
the source's complete bytes bind `plan_sha256`, but no replacement is proposed
and completeness stays true; an exact-digest warning-only `--write` is valid
zero-change convergence. Ref-less and multi-ref projected changes still report
`id:null` rather than inventing target identity. Excluded task `inbox`, notes,
`tmp`, VCS metadata, and ignored paths remain outside the proof boundary. See
[contract 0006](docs/contracts/0006-move-projected-relink.md) for the exact
effect set, digest authority, and refusal rules.

Every in-scope destination is matched to one raw byte range independently proven
to decode to the destination the parser reported, and every replacement is
proven to reparse to the intended destination before it becomes write authority.
A destination whose span or representation cannot be proven yields
`unreadable-entry` and makes the result incomplete instead of being guessed or
quietly dropped. Settled verification judges recognized-ref and generic
destinations by preserved, unambiguous resolution to their intended targets;
ordinary global relink still owns canonical spelling. Recognized missing-target
and ambiguity failures remain errors, while generic proven absence remains a
warning. Unknown inspection state, unsafe representation, and actual move-caused
drift also remain errors. Only `NotFound` proves absence; any other inspection
error refuses. Slopid reports observed link and filesystem state and does not
model cleanup-resource lifecycle.

### Authored-writer quiescence

Keep authored Markdown writers quiescent from an approved projected `--write`
through the owner rename you perform afterwards. This is the caller's duty:
Slopid does not detect or lease writers, and has no lock, session registry, or
compare-and-swap.

The per-file byte check is an early-race check only. It skips a file whose bytes
already changed before the comparison, but an edit landing after that check and
before the atomic replacement is overwritten. Atomic replacement prevents a torn
file, not a lost concurrent edit. That narrow window is an accepted limitation of
this version; if an unexpected writer may have run, take a fresh preview rather
than trusting the previous comparison.

Slopid only repairs Markdown destinations. It does not move folders, file tasks,
or run any other part of a lifecycle operation.

## AI Agent Integration

Run `sid agent-instructions` for the full machine-readable usage guide.

For a project's `AGENTS.md` or `CLAUDE.md`, a minimal integration is:

```markdown
## sid

This project uses `sid` for deterministic local task memory. Run
`sid agent-instructions` before creating, querying, or modifying task memory.
Parse JSON output and inspect completeness and findings before treating partial
query results as authoritative.
```

## Development

```sh
cargo build
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Durable behavior is documented in [`docs/contracts/`](docs/contracts/), and
architecture decisions live in [`docs/decisions/`](docs/decisions/).

## License

[MIT](LICENSE)
