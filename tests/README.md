# CLI process proof

`identity_cli_test.rs` invokes the actual built sid in disposable workspaces.
It owns exact JSON keys, stderr envelopes, exits, help/version, config precedence,
no-write dry-run, no-follow navigation, name-only reservations, ambiguity, lint
coverage and FIFO no-document-read regressions. All-target Clippy includes its
narrow subprocess/clock expectations and Unix-only support sites.

Core unit tests live with pure code; real filesystem tests and the
[second root](../crates/identity/fs/README.md) live with workflows. Their wiring is
not substituted for this production CLI proof. Run `just check` for all targets
and native controls. Remote CI and other platforms are separate evidence.
