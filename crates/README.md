# Libraries and tooling

[Identity](identity/README.md) owns folder naming, allocation, navigation and
namespace checks. Its [core](identity/core/README.md) and
[filesystem](identity/fs/README.md) are production libraries.
[Policy](policy/README.md) is test-only tooling; production never depends on it.
The [CLI app](../src/README.md) composes these libraries.

[Architecture](../docs/architecture.md) owns allowed dependencies and proof limits.
