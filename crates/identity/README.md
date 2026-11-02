# Folder identity

Consumers are people and tools such as Slopdeck. This feature allocates local
folder identities, navigates actual identity directories and reports namespace
defects or incomplete observation. Configuration supports that responsibility;
commands are operations within it. Task-document meaning and lifecycle belong
to consumers, so there is no neighboring Slopid feature to own them.

- [Core](core/README.md): pure names, config validation, decisions, plans and outcomes.
- [Filesystem](fs/README.md): concrete observation, creation and operation sequencing.
- [CLI](../../src/README.md): process inputs, ambient capabilities and presentation.

The core has no app/filesystem dependency. Filesystem takes explicit config,
period, lazy tails and a timestamp formatter. There is no shared Edge or abstract
storage interface. Public crate entrypoints describe the small callable surface;
this internal split makes no non-filesystem public-library product promise.

The folder tree is canonical state. Names reserve refs even for files/symlinks;
navigation includes only real direct directories. Exact-path mkdir is no-clobber;
same-ref/different-slug concurrency remains best effort. Contracts
[0001](../../docs/contracts/0001-sid-new-dry-run.md),
[0003](../../docs/contracts/0003-sid-list.md) and
[0007](../../docs/contracts/0007-root-and-identity-lint.md) own behavior.

Core unit tests prove deterministic decisions. Filesystem tests use real
resources and deterministic error seams; its second-root target demonstrates
composition without the app. [CLI tests](../../tests/README.md) separately prove
the shipped entrypoint. None promises leases, strict ref uniqueness or effects
inside dependency implementations. Reconsider the package boundary when another
actual domain responsibility or protocol adapter appears.
