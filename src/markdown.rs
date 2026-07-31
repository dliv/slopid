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

pub fn destinations(text: &str) -> Vec<MarkdownDestination> {
    let parser = Parser::new(text);
    let references = parser
        .reference_definitions()
        .iter()
        .map(|(_, definition)| (definition.span.clone(), definition.dest.to_string()))
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for (event, range) in parser.into_offset_iter() {
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
        if link_type == LinkType::Inline
            && let Some((span, form)) = inline_span(text, range)
        {
            candidates.push((span, destination, form));
        }
    }
    for (range, destination) in references {
        if let Some((span, form)) = reference_span(text, range) {
            candidates.push((span, destination, form));
        }
    }
    let mut seen = BTreeSet::new();
    let mut output = Vec::new();
    for (span, resolved, form) in candidates {
        if !seen.insert((span.start, span.end)) {
            continue;
        }
        let original = text[span.clone()].to_string();
        let (line, column) = line_column(text, span.start);
        output.push(MarkdownDestination {
            span,
            original,
            resolved,
            form,
            line,
            column,
        });
    }
    output.sort_by_key(|destination| destination.span.start);
    output
}

fn inline_span(text: &str, range: Range<usize>) -> Option<(Range<usize>, MarkdownDestinationForm)> {
    let slice = text.get(range.clone())?;
    let marker = slice.rfind("](")?;
    destination_after(text, range.start + marker + 2, range.end)
}

fn reference_span(
    text: &str,
    range: Range<usize>,
) -> Option<(Range<usize>, MarkdownDestinationForm)> {
    let slice = text.get(range.clone())?;
    let marker = slice.find("]:")?;
    destination_after(text, range.start + marker + 2, range.end)
}

fn destination_after(
    text: &str,
    mut start: usize,
    limit: usize,
) -> Option<(Range<usize>, MarkdownDestinationForm)> {
    let bytes = text.as_bytes();
    while start < limit && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    if start >= limit {
        return None;
    }
    if bytes[start] == b'<' {
        let destination_start = start + 1;
        let mut cursor = destination_start;
        let mut escaped = false;
        while cursor < limit {
            let byte = bytes[cursor];
            if byte == b'>' && !escaped {
                return Some((destination_start..cursor, MarkdownDestinationForm::Angle));
            }
            escaped = byte == b'\\' && !escaped;
            if byte != b'\\' {
                escaped = false;
            }
            cursor += 1;
        }
        return None;
    }
    let destination_start = start;
    let mut cursor = start;
    let mut depth = 0_i32;
    let mut escaped = false;
    while cursor < limit {
        let byte = bytes[cursor];
        if escaped {
            escaped = false;
            cursor += 1;
            continue;
        }
        match byte {
            b'\\' => escaped = true,
            b'(' => depth += 1,
            b')' if depth == 0 => break,
            b')' => depth -= 1,
            byte if byte.is_ascii_whitespace() && depth == 0 => break,
            _ => {}
        }
        cursor += 1;
    }
    (cursor > destination_start)
        .then_some((destination_start..cursor, MarkdownDestinationForm::Bare))
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

    #[test]
    fn extracts_inline_image_angle_reference_and_unicode_columns_not_code() {
        let markdown = "é [one](../202401_sa2a7_old/file.md \"t\")\n\
![two](<../202402_sb3b8_seed.md>)\n\
[use][ref]\n\
\n\
[ref]: ../202403_sc4c9_old/CURRENT_STATE.md\n\
`code ../202404_sd5d2_old`\n\
```md\n[fenced](../202405_se6e3_old/CURRENT_STATE.md)\n```\n";
        let found = destinations(markdown);
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
        let found = destinations(markdown);
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
        let found = destinations(markdown);
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
