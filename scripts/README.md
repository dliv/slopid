# Development policy controls

[`check_clippy_policy.py`](check_clippy_policy.py) is the local adapter between
Cargo's JSON diagnostics and Slopid's checked inventory. Run it with
`python3 scripts/check_clippy_policy.py`, or as the last step of `just check`.
The focused adapter tests live in [`tests/`](tests/).

The Python 3.11+ adapter runs workspace-wide all-target Clippy against this repository with the root
`clippy.toml`, force-warnings only for the two named ban lints, and compiler
override variables cleared in its child process. It requires Cargo success,
valid JSON records, no compiler errors, and no unexpected warning identity.
It normalizes primary source paths under the repository and counts a site by
relative file, lint code, canonical banned item, and source span. Identical
diagnostics repeated for the same compiled source span are counted once;
distinct spans remain distinct. Expected production, test and tooling sites are kept in
separate groups in `clippy_policy_expectations.json`.

The control also creates five compile-only libraries in a fresh temporary
directory. Each preserves the actual root/member manifests, workspace lint inheritance,
`Cargo.lock`, and `clippy.toml`, creates minimal member target placeholders,
uses an explicit scratch Cargo target directory, selects the synthetic product
library, and runs normal warnings-
denied Clippy without force flags. Cargo runs from the repository root with an
explicit fixture `--manifest-path`, so repository `.cargo/config.toml` settings
also apply to these controls. `CLIPPY_CONF_DIR` still points to each fixture's
copied Clippy config, and `CARGO_TARGET_DIR` points to the shared scratch target.
An effect-free library and a fulfilled method expectation must pass. An aliased
banned method, `println!`, and a stale expectation must fail with respectively
`clippy::disallowed_methods`, `clippy::disallowed_macros`, and
`unfulfilled_lint_expectations`. No fixture function is run.

The script prints a concise success count. On failure it includes the command,
working directory, exit status, and captured stdout/stderr. Each Cargo process
has a timeout; on POSIX the script owns and terminates its process group before
the temporary fixture directory is cleaned. The tool uses only the Python
standard library and does not parse Rust or modify product source.

## Release identity

`python3 scripts/check_release.py v<version>` requires clean source, a tag
matching the root Cargo version, and that tag's commit at HEAD. It accepts
annotated and lightweight tags and emits release outputs only after validation.
Its real-Git fixture tests run with the adapter tests in `just check`.
The tag workflow runs that complete gate before either native macOS build;
locked builds, Mach-O architecture, executable version and archive bytes are
checked before upload. This does not itself publish or install a release.
