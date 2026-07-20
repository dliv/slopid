pub const ALPHA22: &str = "abcdefghjkmnpqrtuvwxyz";
pub const SLOP30: &str = "23456789abcdefghjkmnpqrtuvwxyz";
pub const DIGITS: &str = "23456789";
pub const START6: &str = "abcdef";
pub const MAX_SEQ: u16 = 659;

pub const ALPHA22_CHARS: [char; 22] = [
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'm', 'n', 'p', 'q', 'r', 't', 'u', 'v', 'w',
    'x', 'y', 'z',
];
pub const SLOP30_CHARS: [char; 30] = [
    '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'j', 'k', 'm',
    'n', 'p', 'q', 'r', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];

const FNV_OFFSET: u32 = 2_166_136_261;
const FNV_PRIME: u32 = 16_777_619;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefTail {
    pub hi: char,
    pub lo: char,
}

pub fn encode_seq(seq: u16) -> Option<String> {
    if seq > MAX_SEQ {
        return None;
    }

    let hi = ALPHA22_CHARS[(seq / 30) as usize];
    let lo = SLOP30_CHARS[(seq % 30) as usize];
    Some(format!("{hi}{lo}"))
}

pub fn decode_seq(sid_ref: &str) -> Option<u16> {
    if !is_recognized_ref(sid_ref) {
        return None;
    }

    let mut chars = sid_ref.chars();
    chars.next()?;
    let hi = chars.next()?;
    let lo = chars.next()?;
    let hi_index = ALPHA22.find(hi)? as u16;
    let lo_index = SLOP30.find(lo)? as u16;
    Some(hi_index * 30 + lo_index)
}

pub fn encode_ref(seq: u16, tail: RefTail) -> Option<String> {
    let seq_text = encode_seq(seq)?;
    let mut seq_chars = seq_text.chars();
    seq_chars.next()?;
    let seq_lo = seq_chars.next()?;

    if !SLOP30.contains(tail.hi) || !SLOP30.contains(tail.lo) {
        return None;
    }
    if !generated_tail_rule(seq_lo, tail) {
        return None;
    }

    Some(format!("s{seq_text}{}{}", tail.hi, tail.lo))
}

pub fn is_recognized_ref(sid_ref: &str) -> bool {
    let chars: Vec<char> = sid_ref.chars().collect();
    chars.len() == 5
        && chars[0] == 's'
        && ALPHA22.contains(chars[1])
        && SLOP30.contains(chars[2])
        && SLOP30.contains(chars[3])
        && SLOP30.contains(chars[4])
}

#[cfg(test)]
pub fn is_generated_ref(sid_ref: &str) -> bool {
    if !is_recognized_ref(sid_ref) {
        return false;
    }

    let chars: Vec<char> = sid_ref.chars().collect();
    let seq_lo = chars[2];
    let tail = RefTail {
        hi: chars[3],
        lo: chars[4],
    };
    generated_tail_rule(seq_lo, tail)
}

pub fn deterministic_seq_start(period: &str) -> Option<u16> {
    if period.len() != 6 || !period.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }

    let mut hash = FNV_OFFSET;
    for byte in period.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }

    let slot = hash % 36;
    let seq_hi = START6.as_bytes()[(slot / 6) as usize] as char;
    let seq_lo = START6.as_bytes()[(slot % 6) as usize] as char;
    let hi_index = ALPHA22.find(seq_hi)? as u16;
    let lo_index = SLOP30.find(seq_lo)? as u16;
    Some(hi_index * 30 + lo_index)
}

fn generated_tail_rule(seq_lo: char, tail: RefTail) -> bool {
    if DIGITS.contains(seq_lo) {
        ALPHA22.contains(tail.hi)
    } else {
        DIGITS.contains(tail.hi) || DIGITS.contains(tail.lo)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alphabets_match_handoff_order() {
        assert_eq!(ALPHA22, "abcdefghjkmnpqrtuvwxyz");
        assert_eq!(SLOP30, "23456789abcdefghjkmnpqrtuvwxyz");
        assert_eq!(DIGITS, "23456789");
        assert_eq!(START6, "abcdef");
    }

    #[test]
    fn alphabet_arrays_are_locked_to_alphabet_strings() {
        // encode uses the arrays while decode/recognition use the strings; a
        // mid-array swap would otherwise pass every example-based test.
        assert!(ALPHA22.chars().eq(ALPHA22_CHARS));
        assert!(SLOP30.chars().eq(SLOP30_CHARS));
    }

    #[test]
    fn encode_decode_round_trips_every_sequence() {
        for seq in 0..=MAX_SEQ {
            let seq_text = encode_seq(seq).unwrap();
            let sid_ref = format!("s{seq_text}a2");
            assert_eq!(decode_seq(&sid_ref), Some(seq));
        }
    }

    #[test]
    fn encode_seq_matches_handoff_examples() {
        assert_eq!(encode_seq(0).as_deref(), Some("a2"));
        assert_eq!(encode_seq(1).as_deref(), Some("a3"));
        assert_eq!(encode_seq(7).as_deref(), Some("a9"));
        assert_eq!(encode_seq(8).as_deref(), Some("aa"));
        assert_eq!(encode_seq(29).as_deref(), Some("az"));
        assert_eq!(encode_seq(30).as_deref(), Some("b2"));
        assert_eq!(encode_seq(659).as_deref(), Some("zz"));
        assert_eq!(encode_seq(660), None);
    }

    #[test]
    fn decode_seq_round_trips_recognized_refs() {
        assert_eq!(decode_seq("sa2a2"), Some(0));
        assert_eq!(decode_seq("sa3a2"), Some(1));
        assert_eq!(decode_seq("sa9a2"), Some(7));
        assert_eq!(decode_seq("saaa2"), Some(8));
        assert_eq!(decode_seq("saza2"), Some(29));
        assert_eq!(decode_seq("sb2a2"), Some(30));
        assert_eq!(decode_seq("szza2"), Some(659));
    }

    #[test]
    fn recognized_refs_are_broader_than_generated_refs() {
        assert!(is_generated_ref("sa2a2"));
        assert!(is_recognized_ref("sa222"));
        assert!(!is_generated_ref("sa222"));
        assert!(is_recognized_ref("saabc"));
        assert!(!is_generated_ref("saabc"));
        assert!(!is_recognized_ref("s2a22"));
    }

    #[test]
    fn encode_ref_applies_digit_run_rule() {
        assert_eq!(
            encode_ref(0, RefTail { hi: 'a', lo: '2' }).as_deref(),
            Some("sa2a2")
        );
        assert_eq!(encode_ref(0, RefTail { hi: '2', lo: 'a' }), None);
        assert_eq!(
            encode_ref(8, RefTail { hi: '2', lo: 'a' }).as_deref(),
            Some("saa2a")
        );
        assert_eq!(encode_ref(8, RefTail { hi: 'a', lo: 'b' }), None);
    }

    #[test]
    fn deterministic_seq_start_uses_start6_space() {
        assert_eq!(deterministic_seq_start("202605"), Some(128));
        assert_eq!(deterministic_seq_start("202606"), Some(101));
        assert_eq!(deterministic_seq_start("202607"), Some(102));
        assert_eq!(deterministic_seq_start("2026-05"), None);
    }
}
