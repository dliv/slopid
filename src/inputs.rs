use slop_id_identity_core::refcode::{self, RefTail};
use std::time::SystemTime;
#[expect(
    clippy::disallowed_methods,
    reason = "The CLI composition root supplies the current allocation period."
)]
pub fn current_period() -> String {
    chrono::Local::now().format("%Y%m").to_string()
}

#[expect(
    clippy::disallowed_methods,
    reason = "The CLI composition root supplies allocation entropy lazily."
)]
pub fn randomized_tail_space() -> Vec<RefTail> {
    let mut tails = Vec::with_capacity(refcode::SLOP30_CHARS.len().pow(2));
    for hi in refcode::SLOP30_CHARS {
        for lo in refcode::SLOP30_CHARS {
            tails.push(RefTail { hi, lo });
        }
    }
    fastrand::shuffle(&mut tails);
    tails
}

pub fn local_timestamp(touched: SystemTime) -> String {
    use chrono::{DateTime, Local, SecondsFormat};
    DateTime::<Local>::from(touched).to_rfc3339_opts(SecondsFormat::Secs, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn randomized_tail_space_is_complete_and_without_replacement() {
        let tails = randomized_tail_space();
        let unique = tails
            .iter()
            .map(|tail| (tail.hi, tail.lo))
            .collect::<HashSet<_>>();

        assert_eq!(tails.len(), refcode::SLOP30_CHARS.len().pow(2));
        assert_eq!(unique.len(), tails.len());
    }
}
