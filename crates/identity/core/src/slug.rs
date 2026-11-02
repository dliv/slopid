pub const MAX_SLUG_LEN: usize = 48;

pub fn slugify(input: &str) -> String {
    slugify_with_max(input, MAX_SLUG_LEN)
}

pub fn slugify_with_max(input: &str, max_len: usize) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in input.chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    if slug.len() > max_len {
        slug.truncate(max_len);
        while slug.ends_with('-') {
            slug.pop();
        }
    }

    slug
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugifies_branch_name_style_titles() {
        assert_eq!(slugify("Fix auth state"), "fix-auth-state");
        assert_eq!(slugify("  review: login_flow!! "), "review-login-flow");
        assert_eq!(slugify("already-ok"), "already-ok");
    }

    #[test]
    fn slug_can_be_empty() {
        assert_eq!(slugify("..."), "");
    }

    #[test]
    fn slug_truncates_long_titles_to_stable_branch_like_length() {
        let slug = slugify(
            "prepare migration runway for billing account invoices webhook retries and notifications",
        );

        assert_eq!(slug, "prepare-migration-runway-for-billing-account-inv");
        assert_eq!(slug.len(), MAX_SLUG_LEN);
    }

    #[test]
    fn slug_truncation_is_deterministic_prefix_without_trailing_dash() {
        let input = "prepare migration runway for billing account invoices webhook retries and notifications";
        let untruncated = "prepare-migration-runway-for-billing-account-invoices-webhook-retries-and-notifications";

        let first = slugify(input);
        let second = slugify(input);

        assert_eq!(first, second);
        assert!(first.len() <= MAX_SLUG_LEN);
        assert!(untruncated.starts_with(&first));
        assert!(!first.ends_with('-'));
    }

    #[test]
    fn slug_truncates_after_slugification_for_multibyte_input() {
        let input = "alpha 🚀 beta 状態 gamma delta epsilon zeta eta theta iota kappa lambda";
        let untruncated = "alpha-beta-gamma-delta-epsilon-zeta-eta-theta-iota-kappa-lambda";

        let slug = slugify(input);

        assert_eq!(slug, "alpha-beta-gamma-delta-epsilon-zeta-eta-theta-io");
        assert!(slug.is_ascii());
        assert_eq!(slug.len(), MAX_SLUG_LEN);
        assert!(untruncated.starts_with(&slug));
        assert!(!slug.ends_with('-'));
    }

    #[test]
    fn slug_truncation_trims_trailing_dash() {
        let input = format!("{} next", "a".repeat(47));

        assert_eq!(slugify(&input), "a".repeat(47));
    }

    #[test]
    fn slug_boundary_holds_on_both_sides_of_max_len() {
        assert_eq!(slugify(&"a".repeat(MAX_SLUG_LEN)), "a".repeat(MAX_SLUG_LEN));
        assert_eq!(
            slugify(&"a".repeat(MAX_SLUG_LEN + 1)),
            "a".repeat(MAX_SLUG_LEN)
        );
    }
}
