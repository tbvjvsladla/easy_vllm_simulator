//! One line of shell → its clock reads. Used for tracked shell files, for the lines of Rust string literals
//! (remote command lines, unit texts, fixture scripts) and for the argv of a Rust `Command` chain.
//!
//! A read is
//! - `date` in command position that prints the clock: with options, in `$(…)`/backticks, or piped/redirected;
//!   `date -d`/`--date`/`-f`/`--file` convert a given time and are reads only when that value is relative to now
//!   (`now`, `yesterday`, `+90 days`, `"$t + 90 days"`); `@epoch` and a lone variable are given times. `date -r FILE`
//!   reads a mtime and is one;
//! - `$EPOCHSECONDS`, `$EPOCHREALTIME`, `$SECONDS`, and those names bare inside `(( … ))`/`$(( … ))`;
//! - `printf '%(…)T'`;
//! - `stat` with a `%X`/`%Y`/`%Z`/`%W` (or lowercase) format;
//! - `find -mtime/-mmin/-ctime/-cmin/-atime/-amin/-newer…`;
//! - `-nt`/`-ot` file tests;
//! - `git … --since/--until/--before/--after` with a date relative to now (`2.weeks`, `"2 weeks ago"`);
//! - an assignment to a calendar-period name (`…_DAYS=`, `…_WEEKS=`, `…_MONTHS=`);
//! - any of these inside the command text of `bash -c`/`sh -c`, `ssh <host>` or `eval`, which is read again as shell.
//!
//! A line whose first word starts with `#` is a comment, and so is a `#` after whitespace outside quotes.

use super::is_period_name;

const COMMAND_PREFIX_WORDS: [&str; 15] = [
    "then", "do", "else", "elif", "if", "while", "until", "exec", "env", "command", "builtin", "nice", "nohup", "sudo",
    "time",
];
const EPOCH_VARIABLES: [&str; 3] = ["EPOCHSECONDS", "EPOCHREALTIME", "SECONDS"];
const FIND_TIME_PREDICATES: [&str; 6] = ["-mtime", "-mmin", "-ctime", "-cmin", "-atime", "-amin"];
const GIT_TIME_FILTERS: [&str; 4] = ["--since", "--until", "--before", "--after"];
const RELATIVE_DATE_WORDS: &[&str] = &[
    "ago", "now", "today", "tomorrow", "yesterday", "midnight", "noon", "tea", "this", "last", "next", "sec", "secs",
    "second", "seconds", "min", "mins", "minute", "minutes", "hour", "hours", "day", "days", "week", "weeks",
    "fortnight", "fortnights", "month", "months", "year", "years", "monday", "tuesday", "wednesday", "thursday",
    "friday", "saturday", "sunday",
];
const SHELLS: [&str; 7] = ["sh", "bash", "dash", "zsh", "ksh", "mksh", "ash"];
const SSH_OPTIONS_WITH_VALUE: &str = "BbcDEeFIiJLlmOoPpQRSWw";
const UNQUOTED_ARGUMENT: &str = "%+-./:=@_,";

pub fn line_reads(line: &str) -> Vec<String> {
    if line.trim_start().starts_with('#') {
        return Vec::new();
    }
    let code = strip_trailing_comment(line);
    let bodies = wrapped_commands(code);
    let mut reads: Vec<String> = bodies.iter().flat_map(|body| text_reads(&body.text)).collect();
    let code = blank(code, &bodies);
    let code = code.as_str();
    date_reads(code, &mut reads);
    epoch_reads(code, &mut reads);
    arithmetic_reads(code, &mut reads);
    let words: Vec<&str> = code.split_whitespace().collect();
    if has_word(code, "printf") && code.find("%(").is_some_and(|at| code[at..].contains(")T")) {
        reads.push(format!("printf {}", "%(…)T"));
    }
    if has_word(code, "stat") {
        if let Some(spec) = stat_time_format(code) {
            reads.push(format!("stat %{spec}"));
        }
    }
    if has_word(code, "find") {
        for word in &words {
            if FIND_TIME_PREDICATES.contains(word) || word.starts_with("-newer") {
                reads.push(format!("find {word}"));
            }
        }
    }
    if (code.contains('[') || has_word(code, "test")) && words.iter().any(|w| *w == "-nt" || *w == "-ot") {
        for word in words.iter().filter(|w| **w == "-nt" || **w == "-ot") {
            reads.push(format!("test {word}"));
        }
    }
    if has_word(code, "git") {
        git_reads(code, &mut reads);
    }
    period_assignments(code, &mut reads);
    reads
}

pub fn text_reads(text: &str) -> Vec<String> {
    text.lines().flat_map(line_reads).collect()
}

/// The reads of one program started with an argv (a Rust `Command` chain): `None` stands for an argument that is not
/// a string literal. The argv is quoted back into one shell line, so a `bash -c`/`ssh` body is read again.
pub fn argv_reads(program: &str, args: &[Option<&str>]) -> Vec<String> {
    let mut line = quote(program);
    for arg in args {
        line.push(' ');
        line.push_str(&arg.map_or_else(|| "_".to_string(), quote));
    }
    line_reads(&line)
}

fn quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || UNQUOTED_ARGUMENT.contains(c)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// True when a date value names a time relative to now; `@epoch` and a lone variable (`$t`, `${t}`) are given times.
pub fn is_relative_to_now(value: &str) -> bool {
    let value = value.trim();
    if value.starts_with('@') || is_lone_variable(value) {
        return false;
    }
    let lowered = without_variables(value).to_ascii_lowercase();
    lowered
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '_'))
        .any(|word| RELATIVE_DATE_WORDS.contains(&word))
}

fn is_lone_variable(value: &str) -> bool {
    let Some(name) = value.strip_prefix('$') else { return false };
    let name = name.strip_prefix('{').and_then(|n| n.strip_suffix('}')).unwrap_or(name);
    !name.is_empty() && name.chars().all(is_name_char) && !name.starts_with(|c: char| c.is_ascii_digit())
}

fn without_variables(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '$' {
            out.push(c);
            continue;
        }
        if chars.peek() == Some(&'{') {
            for c in chars.by_ref() {
                if c == '}' {
                    break;
                }
            }
        } else {
            while chars.peek().is_some_and(|c| is_name_char(*c)) {
                chars.next();
            }
        }
        out.push(' ');
    }
    out
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn after_last(text: &str, pred: impl Fn(char) -> bool) -> usize {
    text.char_indices()
        .rev()
        .find(|(_, c)| pred(*c))
        .map_or(0, |(at, c)| at + c.len_utf8())
}

fn word_spans<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = (usize, usize)> + 'a {
    code.match_indices(word).filter_map(move |(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + word.len()..].chars().next();
        let bounded = |c: Option<char>| c.is_none_or(|c| !(is_name_char(c) || c == '-' || c == '.'));
        (bounded(before) && bounded(after)).then_some((at, at + word.len()))
    })
}

fn has_word(code: &str, word: &str) -> bool {
    word_spans(code, word).next().is_some()
}

fn is_assignment(word: &str) -> bool {
    match word.split_once('=') {
        Some((name, _)) => {
            !name.is_empty() && name.chars().all(is_name_char) && !name.starts_with(|c: char| c.is_ascii_digit())
        }
        None => false,
    }
}

fn before_assignments(left: &str) -> &str {
    let mut rest = left.trim_end();
    loop {
        let start = after_last(rest, |c| c.is_whitespace() || "$(`;|&{".contains(c));
        let word = &rest[start..];
        if is_assignment(word) {
            rest = rest[..start].trim_end();
        } else {
            return rest;
        }
    }
}

fn opens_substitution(left: &str) -> bool {
    left.ends_with("$(") || left.ends_with('`')
}

fn is_command_position(left: &str) -> bool {
    if left.is_empty() || opens_substitution(left) || left.ends_with([';', '|', '&', '(', '{', '!']) {
        return true;
    }
    let last = left.rsplit(char::is_whitespace).next().unwrap_or(left);
    COMMAND_PREFIX_WORDS.contains(&last)
}

fn positioned_command_start(code: &str, start: usize) -> Option<usize> {
    let command_start = after_last(&code[..start], |c| c != '/' && !is_name_char(c) && c != '.');
    let path = &code[command_start..start];
    let positioned = (path.is_empty() || path.starts_with('/'))
        && is_command_position(before_assignments(&code[..command_start]));
    positioned.then_some(command_start)
}

struct Word {
    start: usize,
    end: usize,
    value: String,
}

fn is_separator(c: char) -> bool {
    ";|&)<>`\n(".contains(c)
}

fn words(code: &str, from: usize, stop_at_separator: bool) -> Vec<Word> {
    let chars: Vec<(usize, char)> = code[from..].char_indices().map(|(at, c)| (at + from, c)).collect();
    let end_of = |i: usize| chars.get(i).map_or(code.len(), |(at, _)| *at);
    let mut found = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i].1;
        if c.is_whitespace() && c != '\n' {
            i += 1;
            continue;
        }
        if is_separator(c) {
            if stop_at_separator {
                break;
            }
            i += 1;
            continue;
        }
        let start = chars[i].0;
        let mut value = String::new();
        while i < chars.len() {
            let c = chars[i].1;
            if (c.is_whitespace() || is_separator(c)) && !(c == '(' && value.ends_with('$')) {
                break;
            }
            i += 1;
            match c {
                '\'' => {
                    while i < chars.len() && chars[i].1 != '\'' {
                        value.push(chars[i].1);
                        i += 1;
                    }
                    i += 1;
                }
                '"' => {
                    while i < chars.len() && chars[i].1 != '"' {
                        let c = chars[i].1;
                        if c == '\\' && chars.get(i + 1).is_some_and(|(_, n)| "$`\"\\\n".contains(*n)) {
                            value.push(chars[i + 1].1);
                            i += 2;
                        } else {
                            value.push(c);
                            i += 1;
                        }
                    }
                    i += 1;
                }
                '\\' => {
                    if let Some((_, next)) = chars.get(i) {
                        value.push(*next);
                        i += 1;
                    }
                }
                '(' => {
                    value.push('(');
                    let mut depth = 1;
                    while i < chars.len() && depth > 0 {
                        let c = chars[i].1;
                        depth += match c {
                            '(' => 1,
                            ')' => -1,
                            _ => 0,
                        };
                        value.push(c);
                        i += 1;
                    }
                }
                _ => value.push(c),
            }
        }
        found.push(Word { start, end: end_of(i.min(chars.len())), value });
    }
    found
}

struct WrappedCommand {
    start: usize,
    end: usize,
    text: String,
}

fn wrapped_commands(code: &str) -> Vec<WrappedCommand> {
    let mut candidates: Vec<(usize, usize, &str)> = SHELLS
        .iter()
        .chain(["eval", "ssh"].iter())
        .flat_map(|name| word_spans(code, name).map(move |(start, end)| (start, end, *name)))
        .collect();
    candidates.sort();
    let mut bodies: Vec<WrappedCommand> = Vec::new();
    for (start, end, name) in candidates {
        if bodies.iter().any(|b| start >= b.start && start < b.end) || positioned_command_start(code, start).is_none() {
            continue;
        }
        let after = words(code, end, true);
        let body = match name {
            "eval" => Some(&after[..]),
            "ssh" => ssh_command(&after),
            _ => shell_command_string(&after),
        };
        if let Some(body @ [first, ..]) = body {
            let text = body.iter().map(|w| w.value.as_str()).collect::<Vec<_>>().join(" ");
            bodies.push(WrappedCommand { start: first.start, end: body[body.len() - 1].end, text });
        }
    }
    bodies
}

fn shell_command_string(after: &[Word]) -> Option<&[Word]> {
    let mut has_command_string = false;
    let mut at = 0;
    while let Some(word) = after.get(at) {
        let option = word.value.as_str();
        if !(option.starts_with('-') || option.starts_with('+')) || option == "--" || option == "-" {
            break;
        }
        has_command_string |= option.starts_with('-') && !option.starts_with("--") && option.contains('c');
        at += if option == "-o" || option == "+o" { 2 } else { 1 };
    }
    let at = at + usize::from(after.get(at).is_some_and(|w| w.value == "--"));
    has_command_string.then(|| after.get(at..at + 1)).flatten()
}

fn ssh_command(after: &[Word]) -> Option<&[Word]> {
    let mut at = 0;
    while let Some(word) = after.get(at) {
        let Some(cluster) = word.value.strip_prefix('-') else { break };
        at += 1;
        if cluster == "-" {
            break;
        }
        for (k, flag) in cluster.char_indices() {
            if SSH_OPTIONS_WITH_VALUE.contains(flag) {
                at += usize::from(k + flag.len_utf8() == cluster.len());
                break;
            }
        }
    }
    let command = after.get(at + 1..)?;
    (!command.is_empty()).then_some(command)
}

fn blank(code: &str, bodies: &[WrappedCommand]) -> String {
    code.char_indices()
        .map(|(at, c)| if bodies.iter().any(|b| at >= b.start && at < b.end) { ' ' } else { c })
        .collect()
}

fn argument_text(rest: &str) -> (&str, &str) {
    let (mut single, mut double) = (false, false);
    for (at, c) in rest.char_indices() {
        match c {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            ')' | '`' | ';' | '|' | '&' | '<' | '>' if !single && !double => return (&rest[..at], &rest[at..]),
            _ => {}
        }
    }
    (rest, "")
}

fn date_input_value(arguments: &str) -> Option<String> {
    let words = words(arguments, 0, true);
    let value_after = |at: usize| words.get(at + 1).map_or_else(String::new, |w| w.value.clone());
    for (at, word) in words.iter().enumerate() {
        if let Some(long) = word.value.strip_prefix("--") {
            let (name, attached) = long.split_once('=').map_or((long, None), |(n, v)| (n, Some(v)));
            if name == "date" || name == "file" {
                return Some(attached.map_or_else(|| value_after(at), str::to_string));
            }
            continue;
        }
        let Some(cluster) = word.value.strip_prefix('-') else { continue };
        for (k, flag) in cluster.char_indices() {
            match flag {
                'd' | 'f' => {
                    let attached = &cluster[k + 1..];
                    return Some(if attached.is_empty() { value_after(at) } else { attached.to_string() });
                }
                c if c.is_ascii_alphabetic() && c != 'I' => continue,
                _ => break,
            }
        }
    }
    None
}

fn date_reads(code: &str, reads: &mut Vec<String>) {
    for (start, end) in word_spans(code, "date") {
        let Some(command_start) = positioned_command_start(code, start) else { continue };
        let left = before_assignments(&code[..command_start]);
        let (arguments, after) = argument_text(&code[end..]);
        let arguments = arguments.split_whitespace().collect::<Vec<_>>().join(" ");
        if arguments.is_empty() {
            if !opens_substitution(left) && after.is_empty() {
                continue;
            }
        } else if !arguments.trim_start_matches(['\'', '"']).starts_with(['-', '+']) {
            continue;
        }
        if date_input_value(&arguments).is_some_and(|value| !is_relative_to_now(&value)) {
            continue;
        }
        reads.push(if arguments.is_empty() { "date".to_string() } else { format!("date {arguments}") });
    }
}

fn epoch_reads(code: &str, reads: &mut Vec<String>) {
    for variable in EPOCH_VARIABLES {
        for form in [format!("${variable}"), format!("${{{variable}")] {
            for (at, _) in code.match_indices(&form) {
                let next = code[at + form.len()..].chars().next();
                if next.is_none_or(|c| !is_name_char(c)) {
                    reads.push(format!("${variable}"));
                }
            }
        }
    }
}

fn arithmetic_reads(code: &str, reads: &mut Vec<String>) {
    let mut from = 0;
    while let Some(open) = code[from..].find("((").map(|at| at + from) {
        let body_start = open + 2;
        let mut depth = 0usize;
        let mut body_end = code.len();
        let bytes = code.as_bytes();
        let mut at = body_start;
        while at < bytes.len() {
            match bytes[at] {
                b'(' => depth += 1,
                b')' if depth == 0 && bytes.get(at + 1) == Some(&b')') => {
                    body_end = at;
                    break;
                }
                b')' => depth = depth.saturating_sub(1),
                _ => {}
            }
            at += 1;
        }
        let body = &code[body_start..body_end];
        for variable in EPOCH_VARIABLES {
            for (at, _) in body.match_indices(variable) {
                let before = body[..at].chars().next_back();
                let after = body[at + variable.len()..].chars().next();
                let bare = before.is_none_or(|c| !(is_name_char(c) || c == '$' || c == '{'));
                if bare && after.is_none_or(|c| !is_name_char(c)) {
                    reads.push(format!("${variable}"));
                }
            }
        }
        from = (body_end + 2).min(code.len()).max(body_start);
    }
}

fn git_reads(code: &str, reads: &mut Vec<String>) {
    let words = words(code, 0, false);
    for (at, word) in words.iter().enumerate() {
        let (option, attached) = word.value.split_once('=').map_or((word.value.as_str(), None), |(o, v)| (o, Some(v)));
        if !GIT_TIME_FILTERS.contains(&option) {
            continue;
        }
        let value = attached.map_or_else(|| words.get(at + 1).map_or_else(String::new, |w| w.value.clone()), str::to_string);
        if is_relative_to_now(&value) {
            reads.push(format!("git {option}"));
        }
    }
}

fn stat_time_format(code: &str) -> Option<char> {
    let mut chars = code.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            if let Some(&spec) = chars.peek() {
                if "XYZWxyzw".contains(spec) {
                    return Some(spec);
                }
            }
        }
    }
    None
}

fn period_assignments(code: &str, reads: &mut Vec<String>) {
    for (at, _) in code.match_indices('=') {
        let name_start = after_last(&code[..at], |c| !is_name_char(c));
        let name = &code[name_start..at];
        let before = code[..name_start].chars().next_back();
        let in_command_position = before.is_none_or(|c| c.is_whitespace() || c == ';' || c == '(');
        if !name.is_empty() && in_command_position && is_period_name(name) {
            reads.push(format!("{name}="));
        }
    }
}

fn strip_trailing_comment(line: &str) -> &str {
    let (mut single, mut double, mut after_space, mut escaped) = (false, false, true, false);
    for (at, c) in line.char_indices() {
        if escaped {
            escaped = false;
            after_space = false;
            continue;
        }
        match c {
            '\\' if !single => escaped = true,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '#' if !single && !double && after_space => return &line[..at],
            _ => {}
        }
        after_space = c.is_whitespace();
    }
    line
}

#[cfg(test)]
mod tests {
    use super::{is_relative_to_now, line_reads};

    const CASES: &str = include_str!("../../checks/data/verdict_clock/shell_lines.tsv");

    #[test]
    fn shell_lines_give_their_reads() {
        let mut checked = 0;
        for row in CASES.lines().filter(|row| !row.is_empty()) {
            let mut fields = row.split('\t');
            let line = fields.next().unwrap();
            let want: Vec<&str> = fields.collect();
            assert_eq!(line_reads(line), want, "{line:?}");
            checked += 1;
        }
        assert!(checked > 50, "{checked}");
    }

    #[test]
    fn relative_and_given_dates() {
        for relative in ["now", "yesterday", "+90 days", "1 day ago", "$created + 90 days", "2.weeks", "last friday"] {
            assert!(is_relative_to_now(relative), "{relative}");
        }
        for given in ["@1700000000", "@$ts", "$ets", "${ets}", "2026-01-01T19:30:00Z", "{created_s}", "$start_day 00:00"] {
            assert!(!is_relative_to_now(given), "{given}");
        }
    }
}
