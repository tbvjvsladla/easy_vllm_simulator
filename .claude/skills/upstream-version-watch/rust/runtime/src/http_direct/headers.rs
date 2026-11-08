//! `http.client.parse_headers`: the response header block as `email.parser.Parser` (compat32 policy) sees it.
//!
//! The raw lines were already cut by the caller; here they are decoded as ISO-8859-1 and split again the way `email.feedparser` does
//! (`\r\n`, `\r` or `\n` end a line). Lines are taken as headers until one does not look like a header or continuation line (a malformed
//! line without a colon ends the block silently, as in `feedparser._parsegen`); continuation lines fold into the previous value; a value
//! keeps its internal line breaks and loses only trailing `\r`/`\n`. Defects are recorded by the original but never raised, so none is.

use super::text::latin1;

/// The parsed block: `(name, value)` pairs in the order of the wire (`Message.raw_items()`).
#[derive(Clone, Debug, Default)]
pub struct HeaderBlock {
    items: Vec<(String, String)>,
}

impl HeaderBlock {
    /// `Message.get(name)`: the first header whose name matches case-insensitively.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// `name in Message`.
    pub fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub fn raw_items(&self) -> &[(String, String)] {
        &self.items
    }
}

/// `email.feedparser.NLCRE`-style line cut, keeping the line ends.
fn cut_lines(text: &str) -> Vec<&str> {
    let bytes = text.as_bytes();
    let mut lines = Vec::new();
    let mut begin = 0;
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'\n' => {
                at += 1;
                lines.push(&text[begin..at]);
                begin = at;
            }
            b'\r' => {
                at += 1;
                if bytes.get(at) == Some(&b'\n') {
                    at += 1;
                }
                lines.push(&text[begin..at]);
                begin = at;
            }
            _ => at += 1,
        }
    }
    if begin < bytes.len() {
        lines.push(&text[begin..]);
    }
    lines
}

/// `feedparser.headerRE`: `^(From |[\041-\071\073-\176]*:|[\t ])`.
fn starts_like_header(line: &str) -> bool {
    if line.starts_with("From ") || line.starts_with([' ', '\t']) {
        return true;
    }
    let name_length = line
        .bytes()
        .take_while(|byte| matches!(byte, 0x21..=0x39 | 0x3b..=0x7e))
        .count();
    line.as_bytes().get(name_length) == Some(&b':')
}

/// `email.message.Message.set_raw` input of `Compat32.header_source_parse`.
fn header_of(source_lines: &[&str]) -> (String, String) {
    let (name, first_value) = source_lines[0]
        .split_once(':')
        .expect("a header line has a colon");
    let mut value = first_value.trim_start_matches([' ', '\t']).to_string();
    for continuation in &source_lines[1..] {
        value.push_str(continuation);
    }
    (
        name.to_string(),
        value.trim_end_matches(['\r', '\n']).to_string(),
    )
}

/// `parse_headers` over the raw lines `_read_headers` collected (including the blank terminator line when there was one).
pub fn parse_header_lines(raw_lines: &[Vec<u8>]) -> HeaderBlock {
    let text = latin1(&raw_lines.concat());
    let mut candidates = Vec::new();
    for line in cut_lines(&text) {
        if !starts_like_header(line) {
            break;
        }
        candidates.push(line);
    }
    let mut items = Vec::new();
    let mut pending: Option<Vec<&str>> = None;
    for (number, line) in candidates.iter().enumerate() {
        if line.starts_with([' ', '\t']) {
            if let Some(lines) = pending.as_mut() {
                lines.push(line);
            }
            continue;
        }
        if let Some(lines) = pending.take() {
            items.push(header_of(&lines));
        }
        if line.starts_with("From ") {
            if number + 1 == candidates.len() && number != 0 {
                // A unix-from line at the end is pushed back as body and the scan stops (the earlier headers were already stored).
                return HeaderBlock { items };
            }
            continue;
        }
        if line.starts_with(':') {
            continue;
        }
        pending = Some(vec![line]);
    }
    if let Some(lines) = pending {
        items.push(header_of(&lines));
    }
    HeaderBlock { items }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(text: &str) -> HeaderBlock {
        let lines: Vec<Vec<u8>> = text
            .split_inclusive('\n')
            .map(|line| line.as_bytes().to_vec())
            .collect();
        parse_header_lines(&lines)
    }

    #[test]
    fn headers_keep_order_case_and_folding() {
        let parsed = block("Content-Length: 5\r\nConnection: close\r\nX-Fold: a\r\n b\r\n\r\n");
        assert_eq!(
            parsed.raw_items(),
            [
                ("Content-Length".to_string(), "5".to_string()),
                ("Connection".to_string(), "close".to_string()),
                ("X-Fold".to_string(), "a\r\n b".to_string()),
            ]
        );
        assert_eq!(parsed.get("content-length"), Some("5"));
        assert!(parsed.contains("CONNECTION"));
    }

    #[test]
    fn a_line_without_colon_ends_the_block() {
        let parsed = block("no-colon\r\nContent-Length: 5\r\n\r\n");
        assert!(parsed.raw_items().is_empty());
        let after_header = block("A: 1\r\nno-colon\r\nB: 2\r\n\r\n");
        assert_eq!(after_header.raw_items().len(), 1);
    }

    #[test]
    fn leading_continuation_and_empty_name_lines_are_ignored() {
        let parsed = block(" folded first\r\n: nameless\r\nA: 1\r\n\r\n");
        assert_eq!(parsed.raw_items(), [("A".to_string(), "1".to_string())]);
    }

    #[test]
    fn duplicate_names_answer_with_the_first() {
        let parsed = block("Location: /first\r\nLocation: /second\r\n\r\n");
        assert_eq!(parsed.get("location"), Some("/first"));
        assert_eq!(parsed.raw_items().len(), 2);
    }
}
