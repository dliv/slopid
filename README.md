# slopid

`slopid` provides `sid`, a local CLI for allocating and identifying task
folders. Slopid owns folder identity; a task-memory consumer such as Slopdeck
owns documents, relationships, notes, seeds, and lifecycle state.

> **Current source version: Sid 0.5.0.** This is a breaking, identity-only
> release paired with Slopdeck 0.1.1-alpha.19. The previous v0.4.0 protocol used
> table-based `.sid` config and the `tasks` list key; the two protocols are
> incompatible. Migrate callers and config, then adopt the matching pair.

## Install

```sh
brew tap dliv/tools
brew install sid
```

Or download a binary from [GitHub Releases](https://github.com/dliv/slopid/releases).
The v0.5.0 release for these installation paths pairs Sid with Slopdeck
0.1.1-alpha.19. Preserve the previous v0.4.0 installation until the workspace
is ready to switch both products together.

## Quick Start

Initialize a project-local `.sid`, then allocate and inspect folders:

```sh
sid init
sid new "investigate flaky deployment"
sid list --human
sid resolve sa2a7
sid lint
```

`sid` discovers the nearest `.sid` file by walking upward from the current
directory. Without one, the current directory is the project base and `stm` is
the identity root. The root is not created until `sid new` needs it.

The supported config shape is one relative root and an optional replacement for
the generated-ref deny-prefix policy:

```toml
root = "stm"

[ref]
deny_prefixes = []
```

Omitting `root` uses `stm`. Root paths reject absolute paths, `..`, empty
strings, and `.`. Generated refs use the built-in policy unless
`[ref].deny_prefixes` is present; a present array replaces the preset exactly,
and an empty array disables prefix filtering.

## Commands

```text
sid init                    Write the default .sid file
sid root                    Print the absolute project base and root
sid new "task title"        Allocate one folder identity
sid new --dry-run "title"   Preview allocation without writing
sid list                    List real direct-child identity folders
sid resolve sa2a7           Resolve one exact short ref
sid resolve 202610_sa2a7_fix-login  Resolve one exact folder name
sid lint                    Audit names and ref reservations
sid agent-instructions      Print machine-readable usage guidance
```

Successful commands emit JSON by default. `root --human` prints only the root;
`list --human` prints plain columns. List terms are case-insensitive AND
filters: each must match the ref prefix or a slug substring. The default list
sort is most recently touched first; `--sort id` orders by full folder name.

List returns `{"folders": [...]}`. Each row contains exactly `id`, `period`,
`sid_ref`, `slug`, `modified`, `root`, and `path`. Only direct real directories
appear. Slugless identities are supported. A file, directory symlink, broken
symlink, or other matching child remains an allocation/lint reservation but is
not a navigable folder. Folder-list and resolve results are snapshots, not
leases against concurrent filesystem changes.

Resolve accepts an exact recognized short ref or complete identity folder
name. It reads only direct-child names and metadata; it does not open
`CURRENT_STATE.md` or other authored content. A short ref with multiple real
directories returns sorted candidates; the full folder name remains an exact
recovery selector.

`sid lint` scans direct-child names only. It reports invalid refs under a
six-digit period, duplicate recognized refs, and incomplete directory scans.
It never opens Markdown. The report contains `complete`, `healthy`, and
`findings`, with locators relative to the project base. Exit 0 is complete and
healthy, exit 1 is complete with findings, and exit 2 is incomplete.

Pre-result errors emit one JSON `error` object on stderr with empty stdout.
Typed codes distinguish config, allocation, list, resolve, init, runtime,
output, and CLI usage failures. CLI usage errors exit 2; other command failures
exit 1. Lint's complete or incomplete reports remain on stdout at their
documented exit status.

## AI Agent Integration

Run `sid agent-instructions` for the machine-readable usage guide. Parse JSON
results and keep Slopid's folder identity separate from task-memory metadata.
Do not invent refs by hand for durable task folders.

## Development

```sh
cargo build --locked
cargo test --locked
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Durable behavior is documented in [`docs/contracts/`](docs/contracts/), and
architecture decisions live in [`docs/decisions/`](docs/decisions/).

## License

[MIT](LICENSE)
