use crate::support;
use arch_gates::packages::{CrateClass, ExternalAllowlists, check_edges};

// The root is still a mixed application package. Pure/effectful build units are later work.
const CRATES: &[(&str, CrateClass)] = &[
    ("slop-id", CrateClass::Binary),
    ("slop-id-policy", CrateClass::Tooling),
];

fn permits(from: CrateClass, to: CrateClass) -> Result<(), &'static str> {
    use CrateClass::*;
    let allowed = match (from, to) {
        (Pure { feature: a }, Pure { feature: b }) => a == b,
        (Effectful { feature: a }, Pure { feature: b } | Effectful { feature: b }) => a == b,
        (Binary, Pure { .. } | Effectful { .. } | Edge) => true,
        _ => false,
    };
    if allowed {
        Ok(())
    } else {
        Err("Slopid package direction refuses app/tooling and cross-feature dependencies")
    }
}

#[test]
fn workspace_packages_follow_local_policy() {
    let members = support::workspace_members();
    assert!(
        members
            .iter()
            .any(|m| m.name == "slop-id" && !m.dependencies.is_empty()),
        "metadata omitted the runtime package/dependencies"
    );
    let problems = check_edges(
        &members,
        CRATES,
        ExternalAllowlists {
            pure: support::PURE_ALLOWED,
            effectful: &[],
            edge: &[],
        },
        &["arch-gates"],
        permits,
    );
    assert!(
        problems.is_empty(),
        "package dependency violations: {problems:?}"
    );
}
