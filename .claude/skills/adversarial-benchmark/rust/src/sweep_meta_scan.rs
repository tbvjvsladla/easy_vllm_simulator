//! The naive text scanners of the sweep-index producer: the "YAML" and env-file readers, the engine-log patterns and
//! the speculative-decoding declaration reader.
//!
//! These are **not** parsers of YAML, env files or logs. The original reads the serving files with a handful of
//! regular expressions, and the exact language of each expression is the contract (a key may be preceded by blank
//! lines, `\s*` swallows newlines, a `#` ends a value, the engine log is one line of `key=value` pairs). Each function
//! here is a hand-written matcher of one original expression, for `str` patterns (Unicode `\s`, `\w`, `\d`). No
//! expression needs a general backtracking engine: after the literal each one is deterministic, except the ones whose
//! few backtracking steps are written out in the matching function.
//!
//! The texts are decoded file contents (valid characters), so everything works on `&[char]`.

use crate::pystr::{is_decimal_digit, is_python_space};

fn is_space(character: char) -> bool {
    is_python_space(u32::from(character))
}

/// `\w` of a `str` pattern (`\b` and `\W` follow it): CPython's `str.isalnum()` or the underscore, from the checked-in
/// Unicode 15.0.0 table. Rust's own alphanumeric class is not used: it counts combining marks such as U+0345 that
/// CPython does not.
fn is_word(character: char) -> bool {
    crate::pystr::is_python_word(u32::from(character))
}

fn is_ascii_digit(character: char) -> bool {
    character.is_ascii_digit()
}

/// The first index at or after `at` that is not whitespace (`\s*`).
fn skip_space(text: &[char], mut at: usize) -> usize {
    while text.get(at).is_some_and(|&c| is_space(c)) {
        at += 1;
    }
    at
}

/// The end of the literal at `at`, when the text has it there.
fn literal_at(text: &[char], at: usize, literal: &str) -> Option<usize> {
    let mut index = at;
    for expected in literal.chars() {
        if *text.get(index)? != expected {
            return None;
        }
        index += 1;
    }
    Some(index)
}

/// Every index where the literal starts, left to right (overlapping starts included, as `re.search` retries them).
fn occurrences<'a>(text: &'a [char], literal: &'a str) -> impl Iterator<Item = usize> + 'a {
    (0..text.len()).filter(move |&at| literal_at(text, at, literal).is_some())
}

/// `(?m)^` : the start of the text or the position after a `\n`.
fn at_line_start(text: &[char], at: usize) -> bool {
    at == 0 || text.get(at - 1) == Some(&'\n')
}

pub fn to_string(slice: &[char]) -> String {
    slice.iter().collect()
}

/// `str.strip()`.
fn strip_space(slice: &[char]) -> &[char] {
    let mut start = 0;
    let mut end = slice.len();
    while start < end && is_space(slice[start]) {
        start += 1;
    }
    while end > start && is_space(slice[end - 1]) {
        end -= 1;
    }
    &slice[start..end]
}

/// `str.strip(chars)`.
fn strip_set<'a>(mut slice: &'a [char], set: &[char]) -> &'a [char] {
    while slice.first().is_some_and(|c| set.contains(c)) {
        slice = &slice[1..];
    }
    while slice.last().is_some_and(|c| set.contains(c)) {
        slice = &slice[..slice.len() - 1];
    }
    slice
}

/// `m.group(1).strip().strip('"').strip("'")`.
fn clean_value(group: &[char]) -> String {
    let stripped = strip_space(group);
    to_string(strip_set(strip_set(stripped, &['"']), &['\'']))
}

/// `grep_yaml`: `re.search(r"(?m)^\s*KEY\s*:\s*([^\n#]+)", text)`. `None` = no match; a match whose value strips to
/// nothing is `Some("")` (the caller's `or` decides what that means).
///
/// The `\s*` after the colon is greedy and may take newlines; when the value would then be empty it gives characters
/// back one at a time, so the value can start on a whitespace character (never on `\n` or `#`).
pub fn grep_yaml(text: &[char], key: &str) -> Option<String> {
    for start in 0..=text.len() {
        if !at_line_start(text, start) {
            continue;
        }
        let key_at = skip_space(text, start);
        let Some(after_key) = literal_at(text, key_at, key) else {
            continue;
        };
        let colon_at = skip_space(text, after_key);
        if text.get(colon_at) != Some(&':') {
            continue;
        }
        let colon_end = colon_at + 1;
        let mut from = skip_space(text, colon_end);
        loop {
            if text.get(from).is_some_and(|&c| c != '\n' && c != '#') {
                let mut end = from;
                while text.get(end).is_some_and(|&c| c != '\n' && c != '#') {
                    end += 1;
                }
                return Some(clean_value(&text[from..end]));
            }
            if from == colon_end {
                break;
            }
            from -= 1;
        }
    }
    None
}

/// `grep_env`: `re.search(r"(?m)^\s*KEY\s*=\s*([^\n#]*)", text)` with the original's `... or None`: no match and an
/// empty value are both `None`. The greedy `\s*` after `=` takes newlines too, so `KEY=` at the end of a line takes the
/// next line as its value.
pub fn grep_env(text: &[char], key: &str) -> Option<String> {
    for start in 0..=text.len() {
        if !at_line_start(text, start) {
            continue;
        }
        let key_at = skip_space(text, start);
        let Some(after_key) = literal_at(text, key_at, key) else {
            continue;
        };
        let equals_at = skip_space(text, after_key);
        if text.get(equals_at) != Some(&'=') {
            continue;
        }
        let from = skip_space(text, equals_at + 1);
        let mut end = from;
        while text.get(end).is_some_and(|&c| c != '\n' && c != '#') {
            end += 1;
        }
        let value = clean_value(&text[from..end]);
        return if value.is_empty() { None } else { Some(value) };
    }
    None
}

/// `[0-9]+\.[0-9]+\.[0-9]+` at `at`: the end of the match.
fn dotted_triplet(text: &[char], at: usize) -> Option<usize> {
    let mut index = at;
    for part in 0..3 {
        let start = index;
        while text.get(index).is_some_and(|&c| is_ascii_digit(c)) {
            index += 1;
        }
        if index == start {
            return None;
        }
        if part < 2 {
            if text.get(index) != Some(&'.') {
                return None;
            }
            index += 1;
        }
    }
    Some(index)
}

/// `vllm_build`: `re.search(r"\bv([0-9]+\.[0-9]+\.[0-9]+(?:\.dev[0-9]*)?(?:\+g[0-9a-f]+)?[^\s)]*)", log)`.
///
/// The two optional groups only consume characters the final `[^\s)]*` takes anyway, so the capture is the three
/// numbers and the run of characters that are neither whitespace nor `)` after them.
pub fn engine_version(log: &[char]) -> Option<String> {
    for start in 0..log.len() {
        if log[start] != 'v' || (start > 0 && is_word(log[start - 1])) {
            continue;
        }
        let Some(mut end) = dotted_triplet(log, start + 1) else {
            continue;
        };
        while log.get(end).is_some_and(|&c| !is_space(c) && c != ')') {
            end += 1;
        }
        return Some(to_string(&log[start + 1..end]));
    }
    None
}

/// `re.search(r"easy-vllm:([0-9]+\.[0-9]+\.[0-9]+)", text)`.
pub fn image_line_version(text: &[char]) -> Option<String> {
    const PREFIX: &str = "easy-vllm:";
    occurrences(text, PREFIX).find_map(|start| {
        let from = start + PREFIX.len();
        dotted_triplet(text, from).map(|end| to_string(&text[from..end]))
    })
}

/// `re.match(r"([0-9]+\.[0-9]+\.[0-9]+)", text)`: the leading version of a string.
pub fn leading_version(text: &[char]) -> Option<String> {
    dotted_triplet(text, 0).map(|end| to_string(&text[..end]))
}

/// `re.search(r"vLLM[\s]*([0-9]+\.[0-9]+\.[0-9]+)", text)`.
pub fn comment_version(text: &[char]) -> Option<String> {
    occurrences(text, "vLLM").find_map(|start| {
        let from = skip_space(text, start + 4);
        dotted_triplet(text, from).map(|end| to_string(&text[from..end]))
    })
}

/// `re.search(r"Initializing a V1 LLM engine[^\n]*?with config:([^\n]*)", log)`: the rest of the line after the first
/// `with config:` that follows the literal on the same line.
pub fn engine_config_line(log: &[char]) -> Option<String> {
    const OPENING: &str = "Initializing a V1 LLM engine";
    const CONFIG: &str = "with config:";
    for start in occurrences(log, OPENING) {
        let mut at = start + OPENING.len();
        loop {
            if let Some(after) = literal_at(log, at, CONFIG) {
                let mut end = after;
                while log.get(end).is_some_and(|&c| c != '\n') {
                    end += 1;
                }
                return Some(to_string(&log[after..end]));
            }
            if log.get(at).is_none_or(|&c| c == '\n') {
                break;
            }
            at += 1;
        }
    }
    None
}

/// `engine_cfg_val`: `re.search(r"(?:^|[\s,({\[])KEY=([^,\s)\]}]+)", line)` and the original's
/// `.strip().strip("'\"")`. The boundary is the start of the line or one of whitespace `,` `(` `{` `[`; the value is
/// one or more characters other than `,` whitespace `)` `]` `}`.
pub fn engine_config_value(line: &[char], key: &str) -> Option<String> {
    if line.is_empty() {
        return None;
    }
    let value_at = |at: usize| -> Option<String> {
        let after = literal_at(line, at, key)?;
        let from = literal_at(line, after, "=")?;
        let mut end = from;
        while line
            .get(end)
            .is_some_and(|&c| !matches!(c, ',' | ')' | ']' | '}') && !is_space(c))
        {
            end += 1;
        }
        (end > from).then(|| {
            let stripped = strip_space(&line[from..end]);
            to_string(strip_set(stripped, &['\'', '"']))
        })
    };
    for start in 0..=line.len() {
        if start == 0 {
            if let Some(value) = value_at(0) {
                return Some(value);
            }
        }
        if line
            .get(start)
            .is_some_and(|&c| is_space(c) || matches!(c, ',' | '(' | '{' | '['))
        {
            if let Some(value) = value_at(start + 1) {
                return Some(value);
            }
        }
    }
    None
}

/// `re.search(r"Using\s+(\S+)\s+attention backend", log)`.
pub fn attention_backend(log: &[char]) -> Option<String> {
    for start in occurrences(log, "Using") {
        let name_at = skip_space(log, start + 5);
        if name_at == start + 5 {
            continue;
        }
        let mut name_end = name_at;
        while log.get(name_end).is_some_and(|&c| !is_space(c)) {
            name_end += 1;
        }
        if name_end == name_at {
            continue;
        }
        let rest = skip_space(log, name_end);
        if rest > name_end && literal_at(log, rest, "attention backend").is_some() {
            return Some(to_string(&log[name_at..name_end]));
        }
    }
    None
}

/// `re.search(r"attention backend out of potential backends:\s*\[([^\]]*)\]", log)` and the original's list
/// comprehension over `group.split(",")`: every non-blank entry, stripped of whitespace and quotes. `None` = no match.
pub fn attention_candidates(log: &[char]) -> Option<Vec<String>> {
    const OPENING: &str = "attention backend out of potential backends:";
    for start in occurrences(log, OPENING) {
        let bracket = skip_space(log, start + OPENING.len());
        if log.get(bracket) != Some(&'[') {
            continue;
        }
        let from = bracket + 1;
        let Some(close) = (from..log.len()).find(|&at| log[at] == ']') else {
            continue;
        };
        let group = &log[from..close];
        return Some(
            group
                .split(|&c| c == ',')
                .filter(|entry| !strip_space(entry).is_empty())
                .map(|entry| to_string(strip_set(strip_space(entry), &['\'', '"'])))
                .collect(),
        );
    }
    None
}

/// `re.search(r"^\s*export\s+VLLM_ATTENTION_BACKEND=(\S+)", sh, re.M)`.
pub fn declared_attention_backend(shell: &[char]) -> Option<String> {
    const NAME: &str = "VLLM_ATTENTION_BACKEND=";
    for start in 0..=shell.len() {
        if !at_line_start(shell, start) {
            continue;
        }
        let export_at = skip_space(shell, start);
        let Some(after_export) = literal_at(shell, export_at, "export") else {
            continue;
        };
        let name_at = skip_space(shell, after_export);
        if name_at == after_export {
            continue;
        }
        let Some(from) = literal_at(shell, name_at, NAME) else {
            continue;
        };
        let mut end = from;
        while shell.get(end).is_some_and(|&c| !is_space(c)) {
            end += 1;
        }
        if end > from {
            return Some(to_string(&shell[from..end]));
        }
    }
    None
}

/// `[A-Za-z0-9_]+` at `at`: the end of the run, or `at` itself when empty.
fn ascii_name_end(text: &[char], mut at: usize) -> usize {
    while text
        .get(at)
        .is_some_and(|&c| c.is_ascii_alphanumeric() || c == '_')
    {
        at += 1;
    }
    at
}

/// `'?([A-Za-z0-9_]+)'?\s+` at `at`: the name, and the position after the whitespace.
fn quoted_name_then_space(text: &[char], at: usize) -> Option<(String, usize)> {
    let name_at = if text.get(at) == Some(&'\'') {
        at + 1
    } else {
        at
    };
    let name_end = ascii_name_end(text, name_at);
    if name_end == name_at {
        return None;
    }
    let after_quote = if text.get(name_end) == Some(&'\'') {
        name_end + 1
    } else {
        name_end
    };
    let rest = skip_space(text, after_quote);
    (rest > after_quote).then(|| (to_string(&text[name_at..name_end]), rest))
}

/// The MoE backend of the engine log: `Using\s+'?(NAME)'?\s+(?:\w+\s+)*?MoE backend`, else
/// `mxfp4\.py[^\]]*\]\s*Using\s+'?(NAME)'?\s+backend`.
pub fn moe_backend(log: &[char]) -> Option<String> {
    first_moe_pattern(log).or_else(|| mxfp4_pattern(log))
}

fn first_moe_pattern(log: &[char]) -> Option<String> {
    for start in occurrences(log, "Using") {
        let from = skip_space(log, start + 5);
        if from == start + 5 {
            continue;
        }
        let Some((name, mut at)) = quoted_name_then_space(log, from) else {
            continue;
        };
        // `(?:\w+\s+)*?` is lazy: the literal is tried before each further word.
        loop {
            if literal_at(log, at, "MoE backend").is_some() {
                return Some(name);
            }
            let mut word_end = at;
            while log.get(word_end).is_some_and(|&c| is_word(c)) {
                word_end += 1;
            }
            if word_end == at {
                break;
            }
            let next = skip_space(log, word_end);
            if next == word_end {
                break;
            }
            at = next;
        }
    }
    None
}

fn mxfp4_pattern(log: &[char]) -> Option<String> {
    for start in occurrences(log, "mxfp4.py") {
        let from = start + "mxfp4.py".len();
        let Some(close) = (from..log.len()).find(|&at| log[at] == ']') else {
            continue;
        };
        let using_at = skip_space(log, close + 1);
        let Some(after_using) = literal_at(log, using_at, "Using") else {
            continue;
        };
        let name_from = skip_space(log, after_using);
        if name_from == after_using {
            continue;
        }
        if let Some((name, at)) = quoted_name_then_space(log, name_from) {
            if literal_at(log, at, "backend").is_some() {
                return Some(name);
            }
        }
    }
    None
}

/// `len(re.findall(r"(?m)^\s*-\s*role:\s*", text))`: matches are taken left to right and do not overlap; the trailing
/// `\s*` of one match can swallow the newline that lets the next one start at a line start.
pub fn role_entries(text: &[char]) -> usize {
    let mut count = 0;
    let mut from = 0;
    while from <= text.len() {
        let found = (from..=text.len()).find_map(|start| {
            if !at_line_start(text, start) {
                return None;
            }
            let dash_at = skip_space(text, start);
            if text.get(dash_at) != Some(&'-') {
                return None;
            }
            let role_at = skip_space(text, dash_at + 1);
            let after_role = literal_at(text, role_at, "role:")?;
            Some(skip_space(text, after_role))
        });
        match found {
            Some(end) => {
                count += 1;
                from = end.max(from + 1);
            }
            None => break,
        }
    }
    count
}

// ── speculative-decoding declaration ──────────────────────────────────────────────

/// `str.splitlines()` (separators not kept) on characters.
pub fn split_lines(text: &[char]) -> Vec<Vec<char>> {
    let mut lines = Vec::new();
    let mut current = Vec::new();
    let mut index = 0;
    while index < text.len() {
        let c = text[index];
        if matches!(
            c,
            '\n' | '\r'
                | '\u{b}'
                | '\u{c}'
                | '\u{1c}'
                | '\u{1d}'
                | '\u{1e}'
                | '\u{85}'
                | '\u{2028}'
                | '\u{2029}'
        ) {
            if c == '\r' && text.get(index + 1) == Some(&'\n') {
                index += 1;
            }
            lines.push(std::mem::take(&mut current));
        } else {
            current.push(c);
        }
        index += 1;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// `str.expandtabs()` (tab size 8).
fn expand_tabs(line: &[char]) -> Vec<char> {
    let mut out = Vec::with_capacity(line.len());
    let mut column = 0;
    for &c in line {
        match c {
            '\t' => {
                let fill = 8 - column % 8;
                out.extend(std::iter::repeat_n(' ', fill));
                column += fill;
            }
            '\n' | '\r' => {
                out.push(c);
                column = 0;
            }
            other => {
                out.push(other);
                column += 1;
            }
        }
    }
    out
}

/// `re.sub(r"[ \t]+#.*$", "", text)` for a text without a line break: from the start of the first space run that is
/// followed by `#`, everything goes.
fn strip_trailing_comment(text: &[char]) -> Vec<char> {
    for (at, &c) in text.iter().enumerate() {
        if c == '#' && at > 0 && matches!(text[at - 1], ' ' | '\t') {
            let mut start = at;
            while start > 0 && matches!(text[start - 1], ' ' | '\t') {
                start -= 1;
            }
            return text[..start].to_vec();
        }
    }
    text.to_vec()
}

fn count_leading(line: &[char], set: &[char]) -> usize {
    line.iter().take_while(|c| set.contains(c)).count()
}

/// `re.match(r"^([ \t]*)speculative[-_]config[ \t]*:(.*)$", line)`: the indentation text and the rest after the colon.
fn speculative_config_line(line: &[char]) -> Option<(Vec<char>, Vec<char>)> {
    let indent = count_leading(line, &[' ', '\t']);
    let after_indent = literal_at(line, indent, "speculative")?;
    if !matches!(line.get(after_indent), Some('-' | '_')) {
        return None;
    }
    let after_name = literal_at(line, after_indent + 1, "config")?;
    let colon_at = after_name + count_leading(&line[after_name..], &[' ', '\t']);
    if line.get(colon_at) != Some(&':') {
        return None;
    }
    Some((line[..indent].to_vec(), line[colon_at + 1..].to_vec()))
}

/// `re.search(r"--speculative[-_]config[ \t=]+(\S.*)$", line)`: the argument text after the flag. The run of blanks
/// and `=` is greedy and gives characters back until a non-whitespace character follows.
fn runner_flag_argument(line: &[char]) -> Option<Vec<char>> {
    const FLAG: &str = "--speculative";
    for start in occurrences(line, FLAG) {
        let after_flag = start + FLAG.len();
        if !matches!(line.get(after_flag), Some('-' | '_')) {
            continue;
        }
        let Some(after_config) = literal_at(line, after_flag + 1, "config") else {
            continue;
        };
        let run = count_leading(&line[after_config..], &[' ', '\t', '=']);
        if run == 0 {
            continue;
        }
        let mut end = after_config + run;
        while end > after_config {
            if line.get(end).is_some_and(|&c| !is_space(c)) {
                return Some(line[end..].to_vec());
            }
            end -= 1;
        }
    }
    None
}

/// What the declaration scan decided: `state` is `on`, `off` or `unknown`; `k` is the number of speculative tokens when
/// the declaration says it (an arbitrary-size integer).
#[derive(Clone, Debug, PartialEq)]
pub struct SpecDeclaration {
    pub state: &'static str,
    pub k: Option<num_bigint::BigInt>,
    pub source: String,
}

/// A failure of the declaration scan that the original does not catch: the `int()` of a digit run beyond the
/// interpreter's limit, or a JSON document nested beyond its recursion limit.
#[derive(Clone, Debug, PartialEq)]
pub struct ScanFailure {
    pub class: &'static str,
    pub message: String,
}

/// `spec_declared_fingerprint(cfg_path, cfg_text, sh_text)`: the config yaml first (a comment line is not a
/// declaration), then the runner's `--speculative-config`. `config_exists` is the caller's `os.path.isfile(cfg_path)`
/// (a missing config is `unknown`, never `off`).
pub fn spec_declaration(
    config_exists: bool,
    config_text: &[char],
    shell_text: &[char],
) -> Result<SpecDeclaration, ScanFailure> {
    if !config_exists {
        return Ok(SpecDeclaration {
            state: "unknown",
            k: None,
            source: "absent(config yaml 부재 — 선언을 읽지 못했다)".to_string(),
        });
    }
    let lines = split_lines(config_text);
    for (at, line) in lines.iter().enumerate() {
        let Some((indent_text, rest)) = speculative_config_line(line) else {
            continue;
        };
        let indent = expand_tabs(&indent_text).len();
        let mut body: Vec<char> = strip_space(&strip_trailing_comment(&rest)).to_vec();
        if body.is_empty() {
            let mut block: Vec<Vec<char>> = Vec::new();
            for next in &lines[at + 1..] {
                let blank = strip_space(next).is_empty();
                let first = next.iter().find(|&&c| !is_space(c));
                if blank || first == Some(&'#') {
                    continue;
                }
                let expanded = expand_tabs(next);
                let leading = expanded.iter().take_while(|&&c| is_space(c)).count();
                if leading <= indent {
                    break;
                }
                block.push(strip_space(&strip_trailing_comment(next)).to_vec());
            }
            body = block.join(&' ');
        }
        let raw: Vec<char> = strip_set(strip_space(&body), &['\'', '"']).to_vec();
        let lowered = crate::pylower::python_lower(&to_string(&raw));
        if matches!(
            lowered.as_str(),
            "" | "null" | "none" | "~" | "{}" | "false"
        ) {
            let shown = if raw.is_empty() {
                "빈 값".to_string()
            } else {
                to_string(&raw)
            };
            return Ok(SpecDeclaration {
                state: "off",
                k: None,
                source: format!("declared(config yaml speculative-config={shown})"),
            });
        }
        return Ok(SpecDeclaration {
            state: "on",
            k: speculative_tokens(&raw)?,
            source: "declared(config yaml speculative-config)".to_string(),
        });
    }
    for line in split_lines(shell_text) {
        if line
            .iter()
            .find(|&&c| !is_space(c))
            .is_some_and(|&c| c == '#')
        {
            continue;
        }
        if let Some(argument) = runner_flag_argument(&line) {
            return Ok(SpecDeclaration {
                state: "on",
                k: speculative_tokens(&argument)?,
                source: "declared(runner sh --speculative-config)".to_string(),
            });
        }
    }
    Ok(SpecDeclaration {
        state: "off",
        k: None,
        source: "declared(config yaml·runner sh 에 speculative-config 없음)".to_string(),
    })
}

/// `_spec_k(text)`: `num_speculative_tokens` of the declaration. JSON first (an object's own value); otherwise, and
/// also when the JSON says `null`, the first `num_speculative_tokens\W+(\d+)`. A value that is not an integer (a
/// boolean is not one) is `None` and does not fall back.
fn speculative_tokens(text: &[char]) -> Result<Option<num_bigint::BigInt>, ScanFailure> {
    use crate::pyvalue::PyValue;
    let raw = to_string(strip_set(strip_space(text), &['\'', '"']));
    let found = match crate::pyjson::loads(&raw) {
        Ok(PyValue::Dict(map)) => map.get("num_speculative_tokens").cloned(),
        Ok(_) => None,
        Err(error) if error.is_value_error() => None,
        Err(error) => {
            let shown = error.to_string();
            return Err(ScanFailure {
                class: "RecursionError",
                message: shown
                    .strip_prefix("RecursionError: ")
                    .unwrap_or(&shown)
                    .to_string(),
            });
        }
    };
    let value = match found.as_ref().map(PyValue::peel) {
        None | Some(PyValue::None) => digits_after_name(text)?.map(PyValue::Int),
        Some(other) => Some(other.clone()),
    };
    Ok(match value {
        Some(PyValue::Int(number)) => Some(number),
        _ => None,
    })
}

/// `re.search(r"num_speculative_tokens\W+(\d+)", text)` and `int(group)`.
fn digits_after_name(text: &[char]) -> Result<Option<num_bigint::BigInt>, ScanFailure> {
    const NAME: &str = "num_speculative_tokens";
    for start in occurrences(text, NAME) {
        let mut at = start + NAME.len();
        let gap = at;
        while text.get(at).is_some_and(|&c| !is_word(c)) {
            at += 1;
        }
        if at == gap {
            continue;
        }
        let from = at;
        while text
            .get(at)
            .is_some_and(|&c| is_decimal_digit(u32::from(c)))
        {
            at += 1;
        }
        if at == from {
            continue;
        }
        let group = crate::pystr::PyStr::new(&to_string(&text[from..at]));
        return match crate::pynumber::int_from_str(&group) {
            Ok(number) => Ok(Some(number)),
            Err(error) => Err(ScanFailure {
                class: error.class,
                message: error.message,
            }),
        };
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    #[test]
    fn the_yaml_reader_follows_the_regular_expression_not_a_parser() {
        let yaml = |text: &str, key: &str| grep_yaml(&chars(text), key);
        assert_eq!(yaml("model: /m/A # c\n", "model").as_deref(), Some("/m/A"));
        assert_eq!(yaml("a: 1\n  b :  \"x y\"", "b").as_deref(), Some("x y"));
        assert_eq!(yaml("#model: x\n", "model"), None);
        assert_eq!(yaml("model_x: 1\n", "model"), None);
        // the value after a colon at the end of a line is the next line (`\s*` takes the newline)
        assert_eq!(yaml("a:\n  next: 1\n", "a").as_deref(), Some("next: 1"));
        // a value that starts with a comment gives the whitespace back: the group is the blank it can start on
        assert_eq!(yaml("a: # only a comment\n", "a").as_deref(), Some(""));
        assert_eq!(yaml("a:\n# c\n", "a"), None);
    }

    #[test]
    fn the_env_reader_swallows_a_following_line_for_an_empty_value() {
        let env = |text: &str, key: &str| grep_env(&chars(text), key);
        assert_eq!(env("K=\nL=2\n", "K").as_deref(), Some("L=2"));
        assert_eq!(env("K=  # c\n", "K"), None);
        assert_eq!(
            env("IMAGE_TAG='a:b'\n", "IMAGE_TAG").as_deref(),
            Some("a:b")
        );
        assert_eq!(env("IMAGE_TAG_X=1\n", "IMAGE_TAG"), None);
    }

    #[test]
    fn engine_version_and_config_value_patterns() {
        let log = chars("Initializing a V1 LLM engine (v0.26.1.dev0+g568afb3a1.d20260730) with config: a=1, quantization=fp8, quantization_config=None, kv_cache_dtype='auto')\n");
        assert_eq!(
            engine_version(&log).as_deref(),
            Some("0.26.1.dev0+g568afb3a1.d20260730")
        );
        let line = chars(&engine_config_line(&log).unwrap());
        assert_eq!(
            engine_config_value(&line, "quantization").as_deref(),
            Some("fp8")
        );
        assert_eq!(
            engine_config_value(&line, "kv_cache_dtype").as_deref(),
            Some("auto")
        );
        assert_eq!(engine_config_value(&line, "config"), None);
        assert_eq!(engine_version(&chars("xv1.2.3")), None);
    }

    #[test]
    fn backend_patterns() {
        let log = chars("Using 'MARLIN' Mxfp4 MoE backend.\nUsing TRITON_ATTN attention backend out of potential backends: ['TRITON_ATTN']\n");
        assert_eq!(moe_backend(&log).as_deref(), Some("MARLIN"));
        assert_eq!(attention_backend(&log).as_deref(), Some("TRITON_ATTN"));
        assert_eq!(
            attention_candidates(&log),
            Some(vec!["TRITON_ATTN".to_string()])
        );
        assert_eq!(
            moe_backend(&chars("[mxfp4.py:166] Using Marlin backend")).as_deref(),
            Some("Marlin")
        );
        assert_eq!(
            declared_attention_backend(&chars("  export VLLM_ATTENTION_BACKEND=FLASHINFER\n"))
                .as_deref(),
            Some("FLASHINFER")
        );
        assert_eq!(
            declared_attention_backend(&chars("# export VLLM_ATTENTION_BACKEND=X\n")),
            None
        );
    }

    #[test]
    fn role_entries_count_non_overlapping_matches() {
        assert_eq!(
            role_entries(&chars("nodes:\n  - role: main\n  - role: sub\n")),
            2
        );
        assert_eq!(role_entries(&chars("- role:\n- role: sub\n")), 2);
        assert_eq!(role_entries(&chars("x - role: a\n")), 0);
    }

    #[test]
    fn spec_declaration_reads_yaml_blocks_comments_and_runner_flags() {
        let config = |text: &str| spec_declaration(true, &chars(text), &[]).unwrap();
        let on = config(
            "speculative-config: '{\"method\":\"mtp\",\"num_speculative_tokens\":3}'   # c\n",
        );
        assert_eq!((on.state, on.k), ("on", Some(3.into())));
        let block =
            config("speculative-config:\n  method: dspark\n  num_speculative_tokens: 7\nx: 1\n");
        assert_eq!((block.state, block.k), ("on", Some(7.into())));
        assert_eq!(config("#speculative-config: x\n").state, "off");
        assert_eq!(config("speculative-config: null\n").state, "off");
        let runner = spec_declaration(
            true,
            &[],
            &chars("# --speculative-config '{}'\nEXTRA+=(--speculative-config '{\"num_speculative_tokens\":2}')\n"),
        )
        .unwrap();
        assert_eq!((runner.state, runner.k), ("on", Some(2.into())));
        assert_eq!(spec_declaration(false, &[], &[]).unwrap().state, "unknown");
        // a JSON `null` falls back to the text pattern (which finds no digits here); text that is not JSON is
        // searched as it is; a boolean is not an integer and does not fall back
        let null = config("speculative-config: '{\"num_speculative_tokens\": null}'\n");
        assert_eq!(null.k, None);
        let text = config("speculative-config: 'num_speculative_tokens = 5 (not json)'\n");
        assert_eq!(text.k, Some(5.into()));
        let boolean = config("speculative-config: '{\"num_speculative_tokens\": true}'\n");
        assert_eq!(boolean.k, None);
    }
}
