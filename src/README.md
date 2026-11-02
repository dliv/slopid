# Sid CLI composition root

`bin/sid.rs` invokes `lib.rs`. The app parses Clap inputs, obtains cwd and the
default allocation period, supplies lazy shuffled tails and per-timestamp local
time conversion, dispatches filesystem operations and presents results/errors.
`failure.rs` owns stable stderr envelopes, stdout writes and process exit;
`cli.rs` maps its local ValueEnum exhaustively to core ListSort. Agent instructions
are CLI documentation. The public refcode/slug/scan modules are narrow compatibility
re-exports; libraries never import the app.

`inputs.rs` owns clock/shuffle. Default period is sampled after config load and
before scanning; tails are requested once after scan success and before planner
validation, including dry-run. Local timestamps are converted for each date,
not with one present-day offset. No background runtime is started.

Checkable dispatch contract: does `run` only parse/map inputs, acquire ambient
capabilities, call feature operations and map/render outcomes? Does every business
predicate and filesystem observation remain in identity core/filesystem? Are
period/shuffle ordering, config-versus-selector precedence and help bypasses
preserved? Source bans and call counts do not answer those questions.

[Architecture](../docs/architecture.md) owns the rules;
[process tests](../tests/README.md) prove this app's contracts. The filesystem
second root establishes a different, explicitly bounded composition proof.
