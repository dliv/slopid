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
locator such as `:33`. It still gives an existing literal colon-suffixed file
precedence; otherwise it resolves and checks the base file while preserving
the locator and any following `#fragment`. It does not validate line counts.
Omitting `[relink]` or using an empty list preserves the default literal-path
behavior. Unknown extension names fail config parsing.

## Commands

```text
sid new                 Allocate or graduate a task folder
sid list                List task folders and reservations
sid resolve             Resolve one canonical id
sid graph               Read a relationship component
sid lint                Audit frontmatter and relationships
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
sits under the destination root, the same command verifies that its scoped links
are canonical instead of planning a move.

`complete:true` means every local destination in Slopid's declared authored
source coverage was classified and this scoped plan is safe to apply. That
includes ref-less and multi-ref local paths; projected generic changes report
`id:null` rather than inventing target identity. It is not proof over excluded
sources such as task `inbox`, notes, `tmp`, VCS metadata, or ignored paths. See
[contract 0006](docs/contracts/0006-move-projected-relink.md) for the exact
effect set, digest authority, and refusal rules.

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
