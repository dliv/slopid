//! Consumer-owned metadata observation; engine calls belong in integration tests.
use std::path::Path;

/// Observe the requested consumer workspace. Failure never becomes an empty graph.
#[expect(
    clippy::disallowed_methods,
    reason = "Tooling owns Cargo observation; never linked into sid"
)]
pub fn cargo_metadata(root: &Path) -> Result<String, String> {
    let output = std::process::Command::new(env!("CARGO"))
        .current_dir(root)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--offline",
            "--locked",
            "--manifest-path",
        ])
        .arg(root.join("Cargo.toml"))
        .output()
        .map_err(|error| format!("cannot run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    String::from_utf8(output.stdout).map_err(|error| format!("non-UTF-8 metadata: {error}"))
}
