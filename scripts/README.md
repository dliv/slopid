# Development policy controls

[`check_clippy_policy.py`](check_clippy_policy.py) is the local adapter between
Cargo's JSON diagnostics and Slopid's checked inventory. Run it with
`python3 scripts/check_clippy_policy.py`, or as the last step of `just check`.
The focused adapter tests live in [`tests/`](tests/).

The adapter runs all-target Clippy against this repository with the root
`clippy.toml`, force-warnings only for the two named ban lints, and compiler
override variables cleared in its child process. It requires Cargo success,
valid JSON records, no compiler errors, and no unexpected warning identity.
It normalizes primary source paths under the repository and counts a site by
relative file, lint code, canonical banned item, and source span. Identical
diagnostics repeated for the same compiled source span are counted once;
distinct spans remain distinct. Expected production and test sites are kept in
separate groups in `clippy_policy_expectations.json`.

The control also creates five compile-only libraries in a fresh temporary
directory. Each copies the actual `Cargo.toml`, `Cargo.lock`, and `clippy.toml`,
uses an explicit scratch Cargo target directory, and runs normal warnings-
denied Clippy without force flags. An effect-free library and a fulfilled
method expectation must pass. An aliased banned method, `println!`, and a stale
expectation must fail with respectively
`clippy::disallowed_methods`, `clippy::disallowed_macros`, and
`unfulfilled_lint_expectations`. No fixture function is run.

The script prints a concise success count. On failure it includes the command,
working directory, exit status, and captured stdout/stderr. Each Cargo process
has a timeout; on POSIX the script owns and terminates its process group before
the temporary fixture directory is cleaned. The tool uses only the Python
standard library and does not parse Rust or modify product source.
