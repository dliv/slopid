use pulldown_cmark::{Event, LinkType, Parser, Tag};
use std::collections::BTreeSet;
use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownDestinationForm {
    Bare,
    Angle,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownDestination {
    pub span: Range<usize>,
    pub original: String,
    pub resolved: String,
    pub form: MarkdownDestinationForm,
    pub line: usize,
    pub column: usize,
}

/// A destination the parser reported but whose replaceable raw bytes could not
/// be proven. It deliberately carries no span: a caller must be able to reason
/// about the destination's identity without ever receiving mutation authority
/// over bytes nobody verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkdownDestinationIssue {
    pub resolved: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// One parse of one authored file. Located destinations and unlocatable parsed
/// destinations are separate state on purpose: collapsing them into one vector
/// is how a scan failure became silent success.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkdownDestinationScan {
    pub destinations: Vec<MarkdownDestination>,
    pub issues: Vec<MarkdownDestinationIssue>,
}

pub fn destinations(text: &str) -> MarkdownDestinationScan {
    let parser = Parser::new(text);
    let references = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| (definition.span.clone(), definition.dest.to_string()))
        .collect::<Vec<_>>();

    // Inline constructs nest, and a nested construct's own `](` marker lies
    // inside its parent's range. Collect every inline range first so a parent
    // never enumerates its child's delimiter as a candidate for itself.
    let mut inline_ranges = Vec::new();
    let mut inline_events = Vec::new();
    for (event, range) in Parser::new(text).into_offset_iter() {
        let Event::Start(tag) = event else {
            continue;
        };
        let (link_type, destination) = match tag {
            Tag::Link {
                link_type,
                dest_url,
                ..
            }
            | Tag::Image {
                link_type,
                dest_url,
                ..
            } => (link_type, dest_url.to_string()),
            _ => continue,
        };
        if link_type != LinkType::Inline {
            // A reference use carries no destination bytes of its own; its
            // definition supplies the only replaceable span.
            continue;
        }
        inline_ranges.push(range.clone());
        inline_events.push((range, destination));
    }

    let mut scan = MarkdownDestinationScan::default();
    let mut candidates = Vec::new();
    for (range, destination) in inline_events {
        match verified_span(text, &range, "](", &destination, Some(&inline_ranges)) {
            Ok((span, form)) => candidates.push((span, destination, form)),
            Err(message) => scan.issues.push(issue(text, &range, destination, message)),
        }
    }
    for (range, destination) in references {
        match verified_span(text, &range, "]:", &destination, None) {
            Ok((span, form)) => candidates.push((span, destination, form)),
            Err(message) => scan.issues.push(issue(text, &range, destination, message)),
        }
    }

    let mut seen = BTreeSet::new();
    for (span, resolved, form) in candidates {
        if !seen.insert((span.start, span.end)) {
            continue;
        }
        let original = text[span.clone()].to_string();
        let (line, column) = line_column(text, span.start);
        scan.destinations.push(MarkdownDestination {
            span,
            original,
            resolved,
            form,
            line,
            column,
        });
    }
    scan.destinations
        .sort_by_key(|destination| destination.span.start);
    scan.issues.sort_by_key(|issue| (issue.line, issue.column));
    scan
}

fn issue(
    text: &str,
    range: &Range<usize>,
    resolved: String,
    message: String,
) -> MarkdownDestinationIssue {
    let (line, column) = line_column(text, range.start);
    MarkdownDestinationIssue {
        resolved,
        line,
        column,
        message,
    }
}

/// Recover the one raw byte range that decodes to `destination`.
///
/// Delimiter position alone is not mutation authority: `pulldown-cmark` reports
/// the whole construct, so a legal title or link label may contain the same
/// delimiter the destination follows. Every `marker` occurrence inside the
/// construct is therefore treated as a candidate, and each candidate is decoded
/// independently through the parser *in the grammar it was authored in*. Exactly
/// one candidate that the parser reads as a whole destination — same decoded
/// value, no title, nothing left over — is accepted. Zero or several verified
/// candidates fail closed rather than guessing.
///
/// `nested` excludes markers belonging to a nested inline construct, which the
/// parser already reports separately with its own destination.
fn verified_span(
    text: &str,
    range: &Range<usize>,
    marker: &str,
    destination: &str,
    nested: Option<&[Range<usize>]>,
) -> Result<(Range<usize>, MarkdownDestinationForm), String> {
    let Some(slice) = text.get(range.clone()) else {
        return Err("construct range is not a character boundary".to_string());
    };
    let slot = if marker == "]:" {
        DestinationSlot::Definition
    } else {
        DestinationSlot::Inline
    };
    let mut verified: Vec<(Range<usize>, MarkdownDestinationForm)> = Vec::new();
    let mut offset = 0;
    while let Some(found) = slice[offset..].find(marker) {
        let position = range.start + offset + found;
        offset += found + marker.len();
        if nested.is_some_and(|ranges| encloses_any_nested(ranges, range, position)) {
            continue;
        }
        let Some((span, form)) = destination_after(text, position + marker.len(), range.end) else {
            continue;
        };
        if verified.iter().any(|(candidate, _)| *candidate == span) {
            continue;
        }
        if decode_in_slot(&text[span.clone()], form, slot).as_deref() == Some(destination) {
            verified.push((span, form));
        }
    }
    match verified.len() {
        1 => Ok(verified.remove(0)),
        0 => Err(format!(
            "no raw destination span decodes to the parsed destination {destination}"
        )),
        found => Err(format!(
            "{found} indistinguishable raw destination spans decode to the parsed destination {destination}"
        )),
    }
}

/// Is `position` inside some *other* inline construct nested in `outer`?
fn encloses_any_nested(ranges: &[Range<usize>], outer: &Range<usize>, position: usize) -> bool {
    ranges.iter().any(|nested| {
        nested != outer
            && nested.start >= outer.start
            && nested.end <= outer.end
            && nested.contains(&position)
    })
}

/// Which grammar a raw destination came from.
///
/// Verifying a candidate in the slot it was authored in is defence in depth
/// rather than a behavioural requirement: the whitespace-delimited inline wrapper
/// below already expresses every destination a definition can hold, including one
/// ending in a literal backslash. The definition slot is retained because it is
/// the more faithful question to ask and is measurably stricter around
/// whitespace, not because inline verification is known to be insufficient.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DestinationSlot {
    Inline,
    Definition,
}

/// Decode `raw` as the destination of a minimal synthetic inline link in `form`.
///
/// This is a semantic test oracle, never mutation input. It answers only "if
/// CommonMark read exactly these bytes as an inline destination of this form,
/// what destination would it produce?".
pub fn decode_destination(raw: &str, form: MarkdownDestinationForm) -> Option<String> {
    decode_in_slot(raw, form, DestinationSlot::Inline)
}

/// The oracle behind both span verification and render verification.
///
/// The wrapper must read as *exactly one* construct whose destination is the
/// whole of `raw` — nothing else. Matching only the decoded destination is not
/// enough: for `raw` of `DEST "TITLE"`, the wrapper is a single legal link whose
/// destination really is `DEST`, so comparing destinations alone would accept a
/// range that also covers the title and would splice over it. Requiring an empty
/// title and full consumption of the wrapper closes that.
fn decode_in_slot(
    raw: &str,
    form: MarkdownDestinationForm,
    slot: DestinationSlot,
) -> Option<String> {
    // A bare destination cannot contain whitespace anywhere, and the wrapper
    // below cannot tell the difference. Whitespace in `raw` would be absorbed by
    // the wrapper's own delimiter, or turn the remainder into a title, and the
    // parser would then report a destination *shorter* than the bytes asked
    // about — the exact false claim this oracle exists to prevent. Checking only
    // the edges was not enough: `x.md ()` reads as an empty title, which the
    // title check cannot reject.
    if form == MarkdownDestinationForm::Bare && raw.bytes().any(is_commonmark_whitespace) {
        return None;
    }
    match slot {
        DestinationSlot::Inline => {
            // The bare wrapper ends the destination with whitespace rather than
            // the closing parenthesis. A destination legitimately ending in a
            // literal backslash is otherwise inexpressible: in `[x](note\)` the
            // backslash escapes the parenthesis and nothing parses. CommonMark
            // permits whitespace between a destination and `)`, and a title
            // would still be reported separately, so this delimits without
            // loosening the check.
            //
            let wrapper = match form {
                MarkdownDestinationForm::Bare => format!("[x]({raw} )"),
                MarkdownDestinationForm::Angle => format!("[x](<{raw}>)"),
            };
            let mut decoded = None;
            for (event, range) in Parser::new(&wrapper).into_offset_iter() {
                match event {
                    Event::Start(Tag::Link {
                        link_type,
                        dest_url,
                        title,
                        ..
                    }) => {
                        if link_type != LinkType::Inline || decoded.is_some() {
                            return None;
                        }
                        // A title means `raw` held more than a destination.
                        if !title.is_empty() {
                            return None;
                        }
                        // Leftover text means the construct ended early, so the
                        // trailing bytes of `raw` are not part of the
                        // destination either.
                        if range != (0..wrapper.len()) {
                            return None;
                        }
                        decoded = Some(dest_url.to_string());
                    }
                    // Any other link-like construct means the wrapper did not
                    // read as the single inline link this oracle requires.
                    Event::Start(Tag::Image { .. }) => return None,
                    _ => {}
                }
            }
            decoded
        }
        DestinationSlot::Definition => {
            let wrapper = match form {
                MarkdownDestinationForm::Bare => format!("[x]: {raw}\n"),
                MarkdownDestinationForm::Angle => format!("[x]: <{raw}>\n"),
            };
            let parser = Parser::new(&wrapper);
            let definitions = parser.reference_definitions();
            let mut found = None;
            for (_, definition) in definitions.iter() {
                if found.is_some() || definition.title.is_some() {
                    return None;
                }
                // The definition must consume the wrapper up to its newline.
                if definition.span != (0..wrapper.len() - 1) {
                    return None;
                }
                found = Some(definition.dest.to_string());
            }
            found
        }
    }
}

fn destination_after(
    text: &str,
    mut start: usize,
    limit: usize,
) -> Option<(Range<usize>, MarkdownDestinationForm)> {
    let bytes = text.as_bytes();
    while start < limit && is_commonmark_whitespace(bytes[start]) {
        start += 1;
    }
    if start >= limit {
        return None;
    }
    if bytes[start] == b'<' {
        let destination_start = start + 1;
        let mut cursor = destination_start;
        while cursor < limit {
            if is_escape_pair(bytes, cursor, limit) {
                cursor += 2;
                continue;
            }
            match bytes[cursor] {
                b'>' => return Some((destination_start..cursor, MarkdownDestinationForm::Angle)),
                // An angle destination cannot span lines, so an unclosed one is
                // not a destination at all.
                b'\n' => return None,
                _ => {}
            }
            cursor += 1;
        }
        return None;
    }
    let destination_start = start;
    let mut cursor = start;
    let mut depth = 0_i32;
    while cursor < limit {
        if is_escape_pair(bytes, cursor, limit) {
            cursor += 2;
            continue;
        }
        match bytes[cursor] {
            // A bare destination may not contain whitespace at all, not even
            // inside balanced parentheses, so whitespace always terminates it.
            byte if is_commonmark_whitespace(byte) => break,
            b'(' => depth += 1,
            b')' if depth == 0 => break,
            b')' => depth -= 1,
            _ => {}
        }
        cursor += 1;
    }
    (cursor > destination_start)
        .then_some((destination_start..cursor, MarkdownDestinationForm::Bare))
}

/// CommonMark's whitespace set for delimiting a link destination.
///
/// Deliberately not `u8::is_ascii_whitespace`, which omits line tabulation
/// (`U+000B`). CommonMark counts it, so using Rust's predicate let a bare span
/// run past a byte the parser had already treated as the destination's end.
/// Paired with the oracle's whole-candidate whitespace refusal, and the two do
/// different jobs: this predicate is what makes a recovered span correct in the
/// first place, while the oracle's refusal is the fail-closed backstop if a span
/// ever arrives with whitespace anyway. Either alone leaves a gap — a wrong span
/// or a false refusal — so both are kept.
fn is_commonmark_whitespace(byte: u8) -> bool {
    matches!(byte, b'\t' | b'\n' | 0x0b | 0x0c | b'\r' | b' ')
}

/// Is there a CommonMark backslash escape starting at `cursor`?
///
/// CommonMark escapes only ASCII punctuation. A backslash before anything else —
/// notably a space — is literal content, so the byte after it still terminates
/// the destination normally. Treating every post-backslash byte as escaped is what
/// let the scanner consume its own terminator and keep reading into the title.
fn is_escape_pair(bytes: &[u8], cursor: usize, limit: usize) -> bool {
    bytes[cursor] == b'\\' && cursor + 1 < limit && bytes[cursor + 1].is_ascii_punctuation()
}

fn line_column(text: &str, offset: usize) -> (usize, usize) {
    let before = &text[..offset];
    let line = before.bytes().filter(|byte| *byte == b'\n').count() + 1;
    let column = before
        .rsplit_once('\n')
        .map_or(before, |(_, suffix)| suffix)
        .chars()
        .count()
        + 1;
    (line, column)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One legal inline link whose title re-scans to the same semantic
    /// destination as the real one. No syntactic rule can prefer either
    /// candidate, so span recovery must refuse rather than guess.
    const AMBIGUOUS: &str = "[a](x.md \"t](x.md t\")\n";

    /// The destinations a caller may safely replace, without the scan's
    /// uncertainty channel.
    fn located(text: &str) -> Vec<MarkdownDestination> {
        destinations(text).destinations
    }

    #[test]
    fn a_title_delimiter_decoy_never_becomes_the_replacement_span() {
        // Legal CommonMark: the title is inside the parser's event range and
        // contains `](`, so reverse delimiter search selects title bytes while
        // keeping the real destination as semantic identity.
        let markdown = "[a](real.md \"see ](fake.md) here\")\n\
![i](img.md \"also ](fake.md) here\")\n";
        let found = located(markdown);
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(found[0].original, "real.md");
        assert_eq!(found[0].resolved, "real.md");
        assert_eq!(found[1].original, "img.md");
        assert_eq!(found[1].resolved, "img.md");
    }

    #[test]
    fn a_reference_label_delimiter_decoy_never_becomes_the_replacement_span() {
        // A link label may contain an escaped `]`, so the first `]:` in the
        // definition range can sit inside the label.
        let markdown = "[a\\]: decoy]: real.md\n";
        let found = located(markdown);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "real.md");
        assert_eq!(found[0].resolved, "real.md");
    }

    #[test]
    fn indistinguishable_delimiter_candidates_are_not_located() {
        assert!(located(AMBIGUOUS).is_empty(), "{:#?}", located(AMBIGUOUS));
    }

    #[test]
    fn indistinguishable_candidates_surface_as_a_scan_issue_with_identity() {
        let scan = destinations(AMBIGUOUS);
        assert!(scan.destinations.is_empty(), "{scan:#?}");
        assert_eq!(scan.issues.len(), 1, "{scan:#?}");
        // The caller still learns *what* it could not locate, so it can decide
        // whether the destination is inside its own authority.
        assert_eq!(scan.issues[0].resolved, "x.md");
        assert_eq!(scan.issues[0].line, 1);
        assert_eq!(scan.issues[0].column, 1);
        assert!(
            scan.issues[0].message.contains("indistinguishable"),
            "{:?}",
            scan.issues[0].message
        );
    }

    #[test]
    fn a_reference_use_alone_is_neither_located_nor_an_issue() {
        // A shortcut/collapsed/full reference use carries no destination bytes;
        // only its definition does, so it must not look like a scan failure.
        let scan = destinations("[use][missing]\n");
        assert!(scan.destinations.is_empty(), "{scan:#?}");
        assert!(scan.issues.is_empty(), "{scan:#?}");
    }

    #[test]
    fn decode_destination_requires_exactly_one_unambiguous_inline_link() {
        use MarkdownDestinationForm::{Angle, Bare};

        assert_eq!(
            decode_destination("a&#40;b.md", Bare).as_deref(),
            Some("a(b.md")
        );
        assert_eq!(
            decode_destination("a\\>b.md", Angle).as_deref(),
            Some("a>b.md")
        );
        // A raw newline cannot be a destination at all.
        assert_eq!(decode_destination("a\nb.md", Bare), None);
        // An unescaped `>` closes the angle form early, so the wrapper stops
        // being one inline link.
        assert_eq!(decode_destination("a>b.md", Angle), None);
    }

    #[test]
    fn decode_destination_rejects_bytes_that_are_more_than_a_destination() {
        use MarkdownDestinationForm::Bare;

        // Each of these decodes to a destination that *matches* what the parser
        // would report, which is exactly why comparing destinations alone is not
        // enough. They must still be refused: the extra bytes are title or
        // trailing content, and splicing over them would delete authored text.
        //
        // Caught by the *title* check: this fills the whole wrapper but carries
        // a title.
        assert_eq!(decode_destination("note\\ \"t\"", Bare), None);
        assert_eq!(decode_destination("note\\ \"t](x.md t\")", Bare), None);
        // Caught only by the *leftover bytes* check: the wrapper reports an empty
        // title, so nothing else can reject these. The construct ends early and
        // the remaining bytes of `raw` are not part of the destination at all.
        for raw in ["x.md)", "x.md)trailing", "x.md) [](y.md", "x.md)]("] {
            assert_eq!(
                decode_destination(raw, Bare),
                None,
                "{raw:?} must not be accepted: the parser ends the destination \
                 before its last byte"
            );
        }
        // A plain destination plus a title is the general shape of both.
        assert_eq!(decode_destination("x.md \"title\"", Bare), None);
        // The destinations alone still decode, including one ending in a
        // literal backslash.
        assert_eq!(
            decode_destination("note\\", Bare).as_deref(),
            Some("note\\")
        );
        assert_eq!(decode_destination("x.md", Bare).as_deref(), Some("x.md"));
        // An unbalanced parenthesis is not a bare destination at all.
        assert_eq!(decode_destination("a(b", Bare), None);
    }

    #[test]
    fn decode_destination_refuses_any_whitespace_in_a_bare_candidate() {
        use MarkdownDestinationForm::{Angle, Bare};

        // A bare destination cannot contain whitespace anywhere, and the wrapper
        // cannot tell the difference: whitespace in `raw` is absorbed by the
        // wrapper's delimiter or turns the remainder into a title, and the parser
        // then reports a destination *shorter* than the bytes asked about.
        for raw in [
            "x.md ",
            "x.md\t",
            " x.md",
            " ",
            "\t",
            // Interior whitespace: `x.md ()` reads as an empty title, which the
            // title check cannot reject because the title really is empty.
            "x.md ()",
            "x.md \"t\"",
            // Line tabulation is CommonMark whitespace but *not*
            // `u8::is_ascii_whitespace`, so an edge-only ASCII check missed it and
            // the leading byte was silently spliced away.
            "\u{0b}x.md",
            "x.md\u{0b}",
            "a\u{0b}b.md",
        ] {
            assert_eq!(
                decode_destination(raw, Bare),
                None,
                "{raw:?} is not wholly a bare destination and must not decode"
            );
        }
        // The same rule applies in definition grammar.
        assert_eq!(
            decode_in_slot(" x.md", Bare, DestinationSlot::Definition),
            None
        );
        // Angle form legitimately carries interior spaces and is unaffected.
        assert_eq!(
            decode_destination("a b.md", Angle).as_deref(),
            Some("a b.md")
        );
    }

    #[test]
    fn line_tabulation_terminates_a_bare_destination_like_the_parser() {
        // `pulldown-cmark` treats U+000B as whitespace, so it ends the
        // destination; a scanner using Rust's ASCII predicate ran past it and
        // produced a span wider than the destination.
        assert!(is_commonmark_whitespace(0x0b));
        assert!(!0x0bu8.is_ascii_whitespace());

        // In situ the construct is not a link at all, so nothing is located and
        // nothing is silently rewritten.
        let scan = destinations("[a](x.md\u{0b}y)\n");
        assert!(scan.destinations.is_empty(), "{scan:#?}");

        // A leading line tabulation is skipped as whitespace, exactly as the
        // parser skips it, so the located span starts at the real destination.
        let found = located("[a](\u{0b}x.md)\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "x.md");
        assert_eq!(found[0].resolved, "x.md");

        // The trailing side of the same boundary: the span must end at the
        // destination, not run through the line tabulation into the title.
        let found = located("[a](x.md\u{0b} \"t\")\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "x.md");
        assert_eq!(found[0].resolved, "x.md");
    }

    #[test]
    fn a_definition_destination_is_verified_in_definition_grammar() {
        use MarkdownDestinationForm::Bare;

        assert_eq!(
            decode_in_slot("note\\", Bare, DestinationSlot::Definition).as_deref(),
            Some("note\\")
        );
        // The definition oracle applies the same full-consumption rule: a title
        // means these bytes were more than a destination.
        assert_eq!(
            decode_in_slot("real.md \"t\"", Bare, DestinationSlot::Definition),
            None
        );
        // Parenthesis handling is the parser's, not ours: balanced pairs are
        // content in a definition destination and unbalanced ones are refused,
        // exactly as inline. The oracle mirrors the parser rather than deciding.
        assert_eq!(
            decode_in_slot("a(b).md", Bare, DestinationSlot::Definition).as_deref(),
            Some("a(b).md")
        );
        assert_eq!(
            decode_in_slot("a)b.md", Bare, DestinationSlot::Definition),
            None
        );

        let found = located("[r]: note\\\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "note\\");
        assert_eq!(found[0].resolved, "note\\");
    }

    #[test]
    fn a_trailing_backslash_does_not_extend_a_bare_span_into_the_title() {
        // The scanner used to treat the byte after `\` as escaped. CommonMark
        // escapes only ASCII punctuation, so `\` before a space is literal and
        // the space still ends the destination. Consuming it ran the span
        // through the title and past the link's closing parenthesis.
        let markdown = "[a](note\\ \"t](x.md t\")\n";
        let found = located(markdown);
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "note\\");
        assert_eq!(found[0].resolved, "note\\");

        // The same construct with an ordinary title.
        let found = located("[a](note\\ \"plain\")\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "note\\");

        // A punctuation escape is still an escape.
        let found = located("[a](a\\(b\\).md \"t\")\n");
        assert_eq!(found.len(), 1, "{found:#?}");
        assert_eq!(found[0].original, "a\\(b\\).md");
        assert_eq!(found[0].resolved, "a(b).md");
    }

    #[test]
    fn a_destination_with_no_verifiable_span_surfaces_as_a_scan_issue() {
        // The zero-candidate half of the fail-closed rule. An autolink-looking
        // angle destination inside a construct whose delimiters cannot be
        // located leaves the parser's destination with no provable bytes.
        let scan = destinations("[a](<x.md\n");
        assert!(scan.destinations.is_empty(), "{scan:#?}");

        // Force the zero-candidate branch directly: a construct whose only
        // candidate decodes to something other than the parsed destination.
        // The construct is the only inline range, so it is its own nesting set.
        let outer = 0..9;
        let issue = verified_span(
            "[a](x.md)",
            &outer,
            "](",
            "totally-different.md",
            Some(std::slice::from_ref(&outer)),
        );
        let message = issue.expect_err("no candidate can decode to a different destination");
        assert!(
            message.contains("no raw destination span"),
            "{message:?}, expected the zero-candidate message"
        );
    }

    #[test]
    fn an_image_nested_in_a_link_keeps_both_destinations() {
        // The image's own `](` marker lies inside the outer link's event range,
        // and both constructs may legally share one destination.
        let found = located("[![alt](x.md)](x.md)\n");
        assert_eq!(found.len(), 2, "{found:#?}");
        assert_eq!(found[0].original, "x.md");
        assert_eq!(found[1].original, "x.md");
        assert_ne!(found[0].span, found[1].span);
    }

    #[test]
    fn extracts_inline_image_angle_reference_and_unicode_columns_not_code() {
        let markdown = "é [one](../202401_sa2a7_old/file.md \"t\")\n\
![two](<../202402_sb3b8_seed.md>)\n\
[use][ref]\n\
\n\
[ref]: ../202403_sc4c9_old/CURRENT_STATE.md\n\
`code ../202404_sd5d2_old`\n\
```md\n[fenced](../202405_se6e3_old/CURRENT_STATE.md)\n```\n";
        let found = located(markdown);
        assert_eq!(found.len(), 3, "{found:#?}");
        assert_eq!(found[0].line, 1);
        assert_eq!(found[0].column, 9);
        assert_eq!(found[0].form, MarkdownDestinationForm::Bare);
        assert_eq!(found[1].original, "../202402_sb3b8_seed.md");
        assert_eq!(found[1].form, MarkdownDestinationForm::Angle);
        assert_eq!(found[2].line, 5);
        assert_eq!(found[2].form, MarkdownDestinationForm::Bare);
    }

    #[test]
    fn escaped_parentheses_stay_inside_one_destination_span() {
        let markdown = "[x](../202401_sa2a7_old/a\\(b\\).md)\n";
        let found = located(markdown);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].original, "../202401_sa2a7_old/a\\(b\\).md");
        assert_eq!(found[0].resolved, "../202401_sa2a7_old/a(b).md");
        assert_eq!(found[0].form, MarkdownDestinationForm::Bare);
    }

    #[test]
    fn destination_forms_preserve_raw_and_decoded_representations() {
        let markdown = "[close](close\\).md)\n\
[angle](<a(b.md>)\n\
[entity](a&#40;b&#41;.md)\n\
[use][ref]\n\n\
[ref]: <close).md>\n";
        let found = located(markdown);
        assert_eq!(found.len(), 4, "{found:#?}");
        assert_eq!(found[0].original, "close\\).md");
        assert_eq!(found[0].resolved, "close).md");
        assert_eq!(found[0].form, MarkdownDestinationForm::Bare);
        assert_eq!(found[1].form, MarkdownDestinationForm::Angle);
        assert_eq!(found[2].original, "a&#40;b&#41;.md");
        assert_eq!(found[2].resolved, "a(b).md");
        assert_eq!(found[3].form, MarkdownDestinationForm::Angle);
    }
}
