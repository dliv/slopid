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
