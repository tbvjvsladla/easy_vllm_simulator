//! The header section of `email.parser.Parser().parsestr(text)` with the default `compat32` policy, and the `importlib.metadata` message adapter's
//! re-indentation of folded values (`textwrap.dedent`).
//!
//! Ported from CPython 3.12.3 `email/feedparser.py` (`BufferedSubFile`, `_parsegen`, `_parse_headers`), `email/_policybase.py` (`Compat32`) and
//! `importlib/metadata/_adapters.py`. Nothing is unfolded, decoded or normalized: a header value is the first line's text after its colon with leading
//! blanks removed, followed by the raw continuation lines (their line ends and leading blanks included), with trailing line ends removed. Only the
//! header section is read; the body does not influence any header.
//!
//! * Lines end at `\n`, `\r` or `\r\n` and keep their ends (`StringIO(newline='')`).
//! * A line belongs to the header section when it starts with `From `, with a space or tab, or has zero or more ASCII characters other than space,
//!   control characters, DEL and `:` before a `:`. The first line that does not (a blank line included) ends the section; a blank line is dropped,
//!   anything else is body. A non-ASCII first character (a byte-order mark, for example) therefore ends the section at once.
//! * `From ` on the first line is the envelope line and no header; as the last line of the section it ends parsing; elsewhere it is skipped. A
//!   continuation before any header, and a line starting with `:`, are skipped.

use agent_runtime_support::pystr::PyStr;

fn code(ch: char) -> u32 {
    u32::from(ch)
}

/// Lines of `points` with their line ends: `\r\n`, `\r` and `\n` end a line.
pub fn split_lines_keeping_ends(points: &[u32]) -> Vec<&[u32]> {
    let (cr, lf) = (code('\r'), code('\n'));
    let mut lines = Vec::new();
    let mut start = 0;
    let mut at = 0;
    while at < points.len() {
        let end = if points[at] == lf {
            Some(at + 1)
        } else if points[at] == cr {
            Some(if points.get(at + 1) == Some(&lf) {
                at + 2
            } else {
                at + 1
            })
        } else {
            None
        };
        match end {
            Some(end) => {
                lines.push(&points[start..end]);
                start = end;
                at = end;
            }
            None => at += 1,
        }
    }
    if start < points.len() {
        lines.push(&points[start..]);
    }
    lines
}

/// `headerRE.match(line)`.
fn is_header_section_line(line: &[u32]) -> bool {
    let from: Vec<u32> = "From ".chars().map(code).collect();
    if line.starts_with(&from)
        || matches!(line.first(), Some(&first) if first == code(' ') || first == code('\t'))
    {
        return true;
    }
    let in_name = |point: u32| (0x21..=0x39).contains(&point) || (0x3B..=0x7E).contains(&point);
    let name_length = line.iter().take_while(|&&point| in_name(point)).count();
    line.get(name_length) == Some(&code(':'))
}

/// The parsed headers of one message, in input order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HeaderSection {
    headers: Vec<(PyStr, PyStr)>,
}

impl HeaderSection {
    /// `message.get_all(name, [])`: every value whose name equals `name` ignoring case, in input order.
    pub fn get_all(&self, name: &str) -> Vec<PyStr> {
        self.headers
            .iter()
            .filter(|(key, _)| {
                key.code_points().len() == name.chars().count()
                    && key
                        .code_points()
                        .iter()
                        .zip(name.chars())
                        .all(|(&left, right)| {
                            char::from_u32(left)
                                .is_some_and(|left| left.eq_ignore_ascii_case(&right))
                        })
            })
            .map(|(_, value)| value.clone())
            .collect()
    }

    /// The header names and values in order (the checks use this to show the section).
    pub fn entries(&self) -> &[(PyStr, PyStr)] {
        &self.headers
    }

    /// The same section with `adapter` applied to every value (the `importlib.metadata` adapter re-indents folded values).
    pub fn map_values(&self, adapter: impl Fn(&PyStr) -> PyStr) -> HeaderSection {
        HeaderSection {
            headers: self
                .headers
                .iter()
                .map(|(key, value)| (key.clone(), adapter(value)))
                .collect(),
        }
    }
}

/// `Compat32.header_source_parse`: the name before the first colon, the value as described in the module text.
fn source_parse(group: &[&[u32]]) -> (PyStr, PyStr) {
    let first = group[0];
    let colon = first
        .iter()
        .position(|&point| point == code(':'))
        .expect("a header line holds a colon");
    let name = PyStr::from_code_points(first[..colon].to_vec());
    let blanks = first[colon + 1..]
        .iter()
        .take_while(|&&point| point == code(' ') || point == code('\t'))
        .count();
    let mut value: Vec<u32> = first[colon + 1 + blanks..].to_vec();
    for continuation in &group[1..] {
        value.extend_from_slice(continuation);
    }
    while matches!(value.last(), Some(&last) if last == code('\r') || last == code('\n')) {
        value.pop();
    }
    (name, PyStr::from_code_points(value))
}

/// The headers `Parser().parsestr(text)` finds.
pub fn parse_header_section(text: &PyStr) -> HeaderSection {
    let lines = split_lines_keeping_ends(text.code_points());
    let mut section_lines: Vec<&[u32]> = Vec::new();
    for line in lines {
        if !is_header_section_line(line) {
            break;
        }
        section_lines.push(line);
    }
    let mut headers = Vec::new();
    let mut current: Option<Vec<&[u32]>> = None;
    let last_index = section_lines.len().saturating_sub(1);
    for (index, line) in section_lines.iter().copied().enumerate() {
        if matches!(line.first(), Some(&first) if first == code(' ') || first == code('\t')) {
            if let Some(group) = current.as_mut() {
                group.push(line);
            }
            continue;
        }
        if let Some(group) = current.take() {
            headers.push(source_parse(&group));
        }
        let from: Vec<u32> = "From ".chars().map(code).collect();
        if line.starts_with(&from) {
            if index == 0 {
                continue;
            }
            if index == last_index {
                return HeaderSection { headers };
            }
            continue;
        }
        let colon = line.iter().position(|&point| point == code(':'));
        if colon == Some(0) {
            continue;
        }
        current = Some(vec![line]);
    }
    if let Some(group) = current.take() {
        headers.push(source_parse(&group));
    }
    HeaderSection { headers }
}

/// `textwrap.dedent(text)`: removes the longest leading run of blanks common to every line that has other text; whitespace-only lines become empty.
pub fn dedent(text: &PyStr) -> PyStr {
    let lf = code('\n');
    let blank = |point: u32| point == code(' ') || point == code('\t');
    let mut lines: Vec<Vec<u32>> = Vec::new();
    let mut line = Vec::new();
    for &point in text.code_points() {
        if point == lf {
            lines.push(std::mem::take(&mut line));
            lines.push(vec![lf]);
        } else {
            line.push(point);
        }
    }
    lines.push(line);
    // text lines sit at even positions, the separators between them at odd positions
    for (position, content) in lines.iter_mut().enumerate() {
        if position % 2 == 0 && !content.is_empty() && content.iter().all(|&point| blank(point)) {
            content.clear();
        }
    }
    let mut margin: Option<Vec<u32>> = None;
    for (position, content) in lines.iter().enumerate() {
        if position % 2 == 1 {
            continue;
        }
        let indent_length = content.iter().take_while(|&&point| blank(point)).count();
        if indent_length == content.len() {
            continue;
        }
        let indent = &content[..indent_length];
        margin = Some(match margin.take() {
            None => indent.to_vec(),
            Some(current) if indent.starts_with(&current) => current,
            Some(current) if current.starts_with(indent) => indent.to_vec(),
            Some(current) => {
                let common = current
                    .iter()
                    .zip(indent)
                    .take_while(|(left, right)| left == right)
                    .count();
                current[..common].to_vec()
            }
        });
    }
    let mut out = Vec::with_capacity(text.len());
    for (position, content) in lines.iter().enumerate() {
        match (&margin, position % 2) {
            (Some(margin), 0) if !margin.is_empty() && content.starts_with(margin) => {
                out.extend_from_slice(&content[margin.len()..]);
            }
            _ => out.extend_from_slice(content),
        }
    }
    PyStr::from_code_points(out)
}

/// `_adapters.Message._repair_headers.redent`: a value without a line feed is kept, a folded one is dedented behind eight spaces.
pub fn redent(value: &PyStr) -> PyStr {
    if value.is_empty() || !value.code_points().contains(&code('\n')) {
        return value.clone();
    }
    let mut padded = PyStr::new("        ");
    padded.push_py(value);
    dedent(&padded)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(text: &str) -> HeaderSection {
        parse_header_section(&PyStr::new(text))
    }

    fn values(section: &HeaderSection, name: &str) -> Vec<String> {
        section
            .get_all(name)
            .iter()
            .map(|value| value.to_utf8().unwrap())
            .collect()
    }

    #[test]
    fn values_keep_continuation_lines_raw_and_names_match_ignoring_case_in_order() {
        let parsed = section(
            "Name: vllm\nVERSION:\t 0.32.0\nrequires-dist: a\n  b\nRequires-Dist: c\n\nbody: x\n",
        );
        assert_eq!(values(&parsed, "Name"), ["vllm"]);
        assert_eq!(values(&parsed, "Version"), ["0.32.0"]);
        // the continuation keeps its leading blanks and the line end that separates it; only the final end is removed
        assert_eq!(values(&parsed, "Requires-Dist"), ["a\n  b", "c"]);
        assert!(values(&parsed, "body").is_empty());
    }

    #[test]
    fn line_ends_are_lf_cr_or_crlf_and_the_value_keeps_the_ones_inside() {
        let parsed = section("A: 1\r\n 2\rB: 3\n\r\nC: no");
        assert_eq!(values(&parsed, "A"), ["1\r\n 2"]);
        assert_eq!(values(&parsed, "B"), ["3"]);
        assert!(values(&parsed, "C").is_empty());
        let lines: Vec<usize> =
            split_lines_keeping_ends(&"a\r\nb\rc\nd".chars().map(u32::from).collect::<Vec<_>>())
                .iter()
                .map(|line| line.len())
                .collect();
        assert_eq!(lines, [3, 2, 2, 1]);
    }

    #[test]
    fn a_non_header_first_line_ends_the_section_and_a_byte_order_mark_is_such_a_line() {
        assert!(section("\u{feff}Name: vllm\nVersion: 1\n")
            .entries()
            .is_empty());
        assert!(section("not a header\nName: vllm\n").entries().is_empty());
        assert!(section("Na me: x\nName: y\n").entries().is_empty());
        let parsed = section("Name: vllm\nstray line without colon\nVersion: 1\n");
        assert_eq!(values(&parsed, "Name"), ["vllm"]);
        assert!(values(&parsed, "Version").is_empty());
        // `:` inside the name class position: a colon-first line is skipped, the section goes on
        let parsed = section(": nameless\nName: vllm\n");
        assert_eq!(values(&parsed, "Name"), ["vllm"]);
    }

    #[test]
    fn envelope_lines_and_leading_continuations_follow_the_feed_parser() {
        let parsed = section("From x@y Mon\nName: vllm\n");
        assert_eq!(parsed.entries().len(), 1);
        let parsed = section("Name: vllm\nFrom x@y Mon\nVersion: 1\n");
        assert_eq!(values(&parsed, "Version"), ["1"]);
        // a `From ` line that is the last line of the section ends parsing and is body
        let parsed = section("Name: vllm\nVersion: 1\nFrom x@y Mon\n");
        assert_eq!(parsed.entries().len(), 2);
        // a continuation before any header is ignored, and it does not attach to the header after it
        let parsed = section("  orphan\nName: vllm\n");
        assert_eq!(values(&parsed, "Name"), ["vllm"]);
        let parsed = section("Name: a\nFrom b\n c\nVersion: 1\n");
        assert_eq!(values(&parsed, "Name"), ["a"]);
    }

    #[test]
    fn dedent_follows_textwrap() {
        let dedent_text = |text: &str| dedent(&PyStr::new(text)).to_utf8().unwrap();
        assert_eq!(dedent_text("  hello\n    world\n"), "hello\n  world\n");
        assert_eq!(dedent_text("  hello\n\tworld"), "  hello\n\tworld");
        assert_eq!(dedent_text("  a\n   \n  b"), "a\n\nb");
        assert_eq!(dedent_text("\n\n"), "\n\n");
        assert_eq!(dedent_text("    a\n  b\n      c"), "  a\nb\n    c");
        assert_eq!(dedent_text(""), "");
    }

    #[test]
    fn the_installed_adapter_dedents_only_values_with_a_line_feed() {
        let redent_text = |text: &str| redent(&PyStr::new(text)).to_utf8().unwrap();
        assert_eq!(redent_text("plain value"), "plain value");
        assert_eq!(redent_text(""), "");
        // a value folded with eight spaces comes out flush
        assert_eq!(redent_text("a\n        b"), "a\nb");
        // a differently indented continuation leaves the first line indented by the difference
        assert_eq!(redent_text("a\n    b"), "    a\nb");
    }
}
