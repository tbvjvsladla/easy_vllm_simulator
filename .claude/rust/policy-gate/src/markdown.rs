//! The deterministic Markdown local-link scanner of completion_gate.py (`_scan_markdown_links`,
//! `classify_link_target` and helpers), over code points exactly as the Python `str` indexing does.

use crate::pytext::is_space;

type T = [char];

fn s(text: &T, a: usize, b: usize) -> String {
    text[a..b].iter().collect()
}

/// `_blank_inline_code_spans`.
fn blank_inline_code_spans(text: &T) -> Vec<char> {
    let n = text.len();
    let mut result = text.to_vec();
    let mut i = 0;
    while i < n {
        if text[i] == '`' {
            let mut j = i;
            while j < n && text[j] == '`' {
                j += 1;
            }
            let run_len = j - i;
            let mut k = j;
            let mut close: Option<(usize, usize)> = None;
            while k < n {
                if text[k] == '`' {
                    let mut m = k;
                    while m < n && text[m] == '`' {
                        m += 1;
                    }
                    if m - k == run_len {
                        close = Some((k, m));
                        break;
                    }
                    k = m;
                } else {
                    k += 1;
                }
            }
            if let Some((_cs, ce)) = close {
                for p in i..ce {
                    if text[p] != '\n' {
                        result[p] = ' ';
                    }
                }
                i = ce;
                continue;
            }
            i = j;
            continue;
        }
        i += 1;
    }
    result
}

/// `_FENCE_OPEN_RE` match: (fence char, run length, rest-of-line).
fn fence_open(line: &T) -> Option<(char, usize, Vec<char>)> {
    let mut i = 0;
    while i < line.len() && i < 3 && line[i] == ' ' {
        i += 1;
    }
    // `( {0,3})` is greedy but may backtrack; a fourth space cannot start a fence run anyway.
    let c = *line.get(i)?;
    if c != '`' && c != '~' {
        return None;
    }
    let mut j = i;
    while j < line.len() && line[j] == c {
        j += 1;
    }
    if j - i < 3 {
        return None;
    }
    Some((c, j - i, line[j..].to_vec()))
}

/// `_closing_fence_re(c, len).match(line)`.
fn fence_close(line: &T, c: char, len: usize) -> bool {
    let mut i = 0;
    while i < line.len() && i < 3 && line[i] == ' ' {
        i += 1;
    }
    let mut j = i;
    while j < line.len() && line[j] == c {
        j += 1;
    }
    if j - i < len {
        return false;
    }
    line[j..].iter().all(|&x| x == ' ' || x == '\t')
}

/// `_blank_code_regions`.
fn blank_code_regions(text: &T) -> Vec<char> {
    let mut out: Vec<char> = Vec::with_capacity(text.len());
    let mut fence: Option<(char, usize)> = None;
    let lines: Vec<&T> = text.split(|&c| c == '\n').collect();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if let Some((fc, fl)) = fence {
            out.extend(std::iter::repeat(' ').take(line.len()));
            if fence_close(line, fc, fl) {
                fence = None;
            }
            continue;
        }
        if let Some((c, len, rest)) = fence_open(line) {
            if !(c == '`' && rest.contains(&'`')) {
                fence = Some((c, len));
                out.extend(std::iter::repeat(' ').take(line.len()));
                continue;
            }
        }
        out.extend_from_slice(line);
    }
    blank_inline_code_spans(&out)
}

/// `_scan_bracket`.
fn scan_bracket(text: &T, open_idx: usize) -> Option<usize> {
    let n = text.len();
    let mut depth = 1;
    let mut j = open_idx + 1;
    while j < n {
        let c = text[j];
        if c == '\\' && j + 1 < n {
            j += 2;
            continue;
        }
        if c == '[' {
            depth += 1;
        } else if c == ']' {
            depth -= 1;
            if depth == 0 {
                return Some(j + 1);
            }
        }
        j += 1;
    }
    None
}

/// `_parse_destination` -> (raw, end, ok).
fn parse_destination(text: &T, i: usize) -> (Option<String>, usize, bool) {
    let n = text.len();
    if i < n && text[i] == '<' {
        let mut j = i + 1;
        while j < n {
            let c = text[j];
            if c == '\\' && j + 1 < n {
                j += 2;
                continue;
            }
            if c == '>' {
                return (Some(s(text, i, j + 1)), j + 1, true);
            }
            if c == '<' || c == '\n' {
                return (None, i, false);
            }
            j += 1;
        }
        return (None, i, false);
    }
    let mut j = i;
    let mut depth = 0usize;
    while j < n {
        let c = text[j];
        if c == '\\' && j + 1 < n {
            j += 2;
            continue;
        }
        if c == '(' {
            depth += 1;
            j += 1;
            continue;
        }
        if c == ')' {
            if depth == 0 {
                break;
            }
            depth -= 1;
            j += 1;
            continue;
        }
        if is_space(c) {
            break;
        }
        j += 1;
    }
    let j = j.min(n);
    (Some(s(text, i.min(n), j)), j, true)
}

/// `_parse_title` -> (present, end).
fn parse_title(text: &T, i: usize) -> (bool, usize) {
    let n = text.len();
    if i >= n || !matches!(text[i], '"' | '\'' | '(') {
        return (false, i);
    }
    let open_c = text[i];
    let close_c = if open_c == '(' { ')' } else { open_c };
    let mut j = i + 1;
    while j < n {
        let c = text[j];
        if c == '\\' && j + 1 < n {
            j += 2;
            continue;
        }
        if c == close_c {
            return (true, j + 1);
        }
        j += 1;
    }
    (false, i)
}

/// `_skip_ws`.
fn skip_ws(text: &T, i: usize) -> usize {
    let n = text.len();
    let mut j = i;
    while j < n && (text[j] == ' ' || text[j] == '\t') {
        j += 1;
    }
    if j < n && text[j] == '\n' {
        j += 1;
        while j < n && (text[j] == ' ' || text[j] == '\t') {
            j += 1;
        }
    }
    j
}

/// `_parse_inline_link_tail`.
fn parse_inline_link_tail(text: &T, i: usize) -> (Option<String>, usize, bool) {
    let j = skip_ws(text, i);
    let (dest, j, ok) = parse_destination(text, j);
    if !ok {
        return (None, i, false);
    }
    let k = skip_ws(text, j);
    if k < text.len() && text[k] == ')' {
        return (dest, k + 1, true);
    }
    let (_present, k2) = parse_title(text, k);
    if k2 == k {
        return (None, i, false);
    }
    let k = skip_ws(text, k2);
    if k < text.len() && text[k] == ')' {
        return (dest, k + 1, true);
    }
    (None, i, false)
}

/// `str.casefold()` over the Python lowercase mapping plus the full case foldings that differ from `lower()`
/// for common scripts (ß/ẞ, final sigma, long s, micro sign and the Latin ligatures).
pub fn casefold(text: &str) -> String {
    let lowered = agent_runtime_support::pylower::python_lower(text);
    let mut out = String::with_capacity(lowered.len());
    for c in lowered.chars() {
        match c {
            'ß' => out.push_str("ss"),
            'ς' => out.push('σ'),
            'ſ' => out.push('s'),
            'µ' => out.push('μ'),
            'ﬀ' => out.push_str("ff"),
            'ﬁ' => out.push_str("fi"),
            'ﬂ' => out.push_str("fl"),
            'ﬃ' => out.push_str("ffi"),
            'ﬄ' => out.push_str("ffl"),
            'ﬅ' | 'ﬆ' => out.push_str("st"),
            other => out.push(other),
        }
    }
    out
}

/// `_normalize_label`: `re.sub(r'\s+', ' ', label.strip()).casefold()`.
fn normalize_label(label: &str) -> String {
    let stripped = crate::pytext::strip(label);
    let mut collapsed = String::new();
    let mut in_ws = false;
    for c in stripped.chars() {
        if is_space(c) {
            if !in_ws {
                collapsed.push(' ');
            }
            in_ws = true;
        } else {
            collapsed.push(c);
            in_ws = false;
        }
    }
    casefold(&collapsed)
}

/// `_parse_reference_definitions`: ordered (normalized label, raw destination), first wins.
fn parse_reference_definitions(text: &T) -> Vec<(String, String)> {
    let mut defs: Vec<(String, String)> = Vec::new();
    for line in text.split(|&c| c == '\n') {
        let mut idx = 0;
        while idx < line.len() && line[idx] == ' ' {
            idx += 1;
        }
        if idx > 3 || idx >= line.len() || line[idx] != '[' {
            continue;
        }
        let label_end = match scan_bracket(line, idx) {
            Some(e) => e,
            None => continue,
        };
        if label_end >= line.len() || line[label_end] != ':' {
            continue;
        }
        let label_raw = s(line, idx + 1, label_end - 1);
        let j = skip_ws(line, label_end + 1);
        let (dest, _j2, ok) = parse_destination(line, j);
        if !ok {
            continue;
        }
        let norm = normalize_label(&label_raw);
        if !norm.is_empty() && !defs.iter().any(|(k, _)| *k == norm) {
            defs.push((norm, dest.unwrap_or_default()));
        }
    }
    defs
}

fn lookup<'a>(defs: &'a [(String, String)], key: &str) -> Option<&'a String> {
    defs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// `_scan_markdown_links` -> (raw targets, unresolved).
pub fn scan_markdown_links(text: &str) -> (Vec<String>, Vec<String>) {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let chars: Vec<char> = text.chars().collect();
    let sanitized = blank_code_regions(&chars);
    let defs = parse_reference_definitions(&sanitized);
    let mut raw = Vec::new();
    let mut unresolved = Vec::new();
    let n = sanitized.len();
    let mut i = 0;
    while i < n {
        let c = sanitized[i];
        if c == '\\' && i + 1 < n {
            i += 2;
            continue;
        }
        let mut is_image = false;
        let bracket_idx;
        if c == '[' {
            bracket_idx = i;
        } else if c == '!' && i + 1 < n && sanitized[i + 1] == '[' {
            is_image = true;
            bracket_idx = i + 1;
        } else {
            i += 1;
            continue;
        }
        let label_end = match scan_bracket(&sanitized, bracket_idx) {
            Some(e) => e,
            None => {
                i = bracket_idx + 1;
                continue;
            }
        };
        let label_text = s(&sanitized, bracket_idx + 1, label_end - 1);
        if label_end < n && sanitized[label_end] == '(' {
            let (dest, end_idx, ok) = parse_inline_link_tail(&sanitized, label_end + 1);
            if ok {
                raw.push(dest.unwrap_or_default());
                i = end_idx;
            } else {
                unresolved.push(format!(
                    "{}[{}](...) [malformed inline destination]",
                    if is_image { "!" } else { "" },
                    label_text
                ));
                i = label_end + 1;
            }
            continue;
        }
        if label_end < n && sanitized[label_end] == '[' {
            if let Some(label2_end) = scan_bracket(&sanitized, label_end) {
                let label2_text = s(&sanitized, label_end + 1, label2_end - 1);
                let ref_label =
                    if !crate::pytext::strip(&label2_text).is_empty() { &label2_text } else { &label_text };
                let norm = normalize_label(ref_label);
                if let Some(d) = lookup(&defs, &norm) {
                    raw.push(d.clone());
                } else {
                    unresolved.push(format!("[{label_text}][{label2_text}]"));
                }
                i = label2_end;
                continue;
            }
        }
        let norm = normalize_label(&label_text);
        if let Some(d) = lookup(&defs, &norm) {
            raw.push(d.clone());
        }
        i = label_end;
    }
    (raw, unresolved)
}

fn is_escapable(c: char) -> bool {
    "!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~".contains(c)
}

/// `_unescape_markdown`.
fn unescape_markdown(t: &str) -> String {
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '\\' && i + 1 < cs.len() && is_escapable(cs[i + 1]) {
            out.push(cs[i + 1]);
            i += 2;
        } else {
            out.push(cs[i]);
            i += 1;
        }
    }
    out
}

fn is_hex(c: char) -> bool {
    c.is_ascii_hexdigit()
}

/// `_EXTERNAL_SCHEME_RE.match`.
fn has_external_scheme(t: &str) -> bool {
    let cs: Vec<char> = t.chars().collect();
    if cs.is_empty() || !cs[0].is_ascii_alphabetic() {
        return false;
    }
    let mut i = 1;
    while i < cs.len() && (cs[i].is_ascii_alphanumeric() || matches!(cs[i], '+' | '.' | '-')) {
        i += 1;
    }
    // greedy `[...]*` then `://`; backtracking cannot help because ':' is not in the class.
    cs.len() >= i + 3 && cs[i] == ':' && cs[i + 1] == '/' && cs[i + 2] == '/'
}

/// `"%" in _VALID_PERCENT_ESCAPE_RE.sub("", s)`.
fn has_malformed_percent(t: &str) -> bool {
    let cs: Vec<char> = t.chars().collect();
    let mut i = 0;
    while i < cs.len() {
        if cs[i] == '%' {
            if i + 2 < cs.len() && is_hex(cs[i + 1]) && is_hex(cs[i + 2]) {
                i += 3;
                continue;
            }
            return true;
        }
        i += 1;
    }
    false
}

/// `urllib.parse.unquote(s, errors="strict")`; Err = UnicodeDecodeError.
fn unquote_strict(t: &str) -> Result<String, ()> {
    if !t.contains('%') {
        return Ok(t.to_string());
    }
    let cs: Vec<char> = t.chars().collect();
    let mut out = String::new();
    let mut pending: Vec<u8> = Vec::new();
    let mut i = 0;
    let flush = |pending: &mut Vec<u8>, out: &mut String| -> Result<(), ()> {
        if !pending.is_empty() {
            let s = std::str::from_utf8(pending).map_err(|_| ())?;
            out.push_str(s);
            pending.clear();
        }
        Ok(())
    };
    while i < cs.len() {
        if cs[i] == '%' && i + 2 < cs.len() && is_hex(cs[i + 1]) && is_hex(cs[i + 2]) {
            let v = u8::from_str_radix(&format!("{}{}", cs[i + 1], cs[i + 2]), 16).map_err(|_| ())?;
            pending.push(v);
            i += 3;
            continue;
        }
        flush(&mut pending, &mut out)?;
        out.push(cs[i]);
        i += 1;
    }
    flush(&mut pending, &mut out)?;
    Ok(out)
}

pub enum LinkKind {
    Skip,
    Invalid(&'static str),
    Local(String),
}

/// `classify_link_target`.
pub fn classify_link_target(raw_target: &str) -> LinkKind {
    let mut target: String = crate::pytext::strip(raw_target).to_string();
    let cs: Vec<char> = target.chars().collect();
    if cs.len() >= 2 && cs[0] == '<' && cs[cs.len() - 1] == '>' {
        target = cs[1..cs.len() - 1].iter().collect();
    }
    let target = unescape_markdown(&target);
    if target.is_empty()
        || target.starts_with('#')
        || agent_runtime_support::pylower::python_lower(&target).starts_with("mailto:")
        || has_external_scheme(&target)
    {
        return LinkKind::Skip;
    }
    let path_part = target.splitn(2, '#').next().unwrap_or("");
    let path_part = path_part.splitn(2, '?').next().unwrap_or("");
    if path_part.is_empty() {
        return LinkKind::Skip;
    }
    if has_malformed_percent(path_part) {
        return LinkKind::Invalid("malformed percent-encoding");
    }
    let decoded = match unquote_strict(path_part) {
        Ok(d) => d,
        Err(()) => return LinkKind::Invalid("malformed percent-encoding"),
    };
    if crate::completion_gate::is_absolute_path_string(&decoded) {
        return LinkKind::Invalid("absolute filesystem path");
    }
    LinkKind::Local(decoded)
}
