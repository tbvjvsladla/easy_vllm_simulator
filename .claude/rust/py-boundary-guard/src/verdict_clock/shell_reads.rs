//! Shell text → its clock reads. Used for tracked shell files, for the lines of Rust string literals (remote command
//! lines, unit texts, fixture scripts) and for the argv of a Rust `Command` chain. A line that ends in an unquoted `\`
//! goes on in the next line: the joined command is read and its reads are reported at the line where it starts.
//!
//! A read is
//! - `date` in command position that prints the clock: with options, or with its output taken (inside `$(…)` or
//!   backticks, piped or redirected). `date -d`/`--date` converts a given time and is a read when that value is
//!   missing, empty or relative to now (`now`, `yesterday`, `+90 days`, `"$t + 90 days"`); `@epoch` and a value made of
//!   expansions only (`"$t"`, `"$(cat stamp)"`) are given times. `date -f`/`--file` (times from a file this lint cannot
//!   see) and `date -r FILE` (a mtime) always are reads;
//! - `$EPOCHSECONDS`, `$EPOCHREALTIME`, `$SECONDS`, and those names bare in an arithmetic context: inside
//!   `(( … ))`/`$(( … ))`, in an operand of `-eq`/`-ne`/`-lt`/`-le`/`-gt`/`-ge` inside `[[ … ]]`, and in an argument of
//!   `let` (quoted or not), the `[[` or `let` standing in command position outside quotes;
//! - `printf '%(…)T'`;
//! - `stat` with a `%X`/`%Y`/`%Z`/`%W` (or lowercase) format;
//! - `find -mtime/-mmin/-ctime/-cmin/-atime/-amin/-newer…`;
//! - `-nt`/`-ot` file tests;
//! - `git … --since/--until/--before/--after` with a date relative to now (`2.weeks`, `"2 weeks ago"`);
//! - `openssl verify` without `-attime`/`-no_check_time` (it checks certificate validity against the clock);
//! - an assignment to a calendar-period name (`…_DAYS=`, `…_WEEKS=`, `…_MONTHS=`);
//! - any of these inside a command substitution (`$(…)` or backticks, outside single quotes) or inside the command text
//!   of `bash -c`/`sh -c`, `ssh <host>` or `eval`, which are read again as shell. The output of a wrapped command is
//!   taken when the wrapper's is: inside a substitution, piped, or redirected.
//!
//! A command word is in command position at the start of a command, after `;`, `|`, `&`, `(`, `{`, `!` or a backtick,
//! after `then`/`do`/`else`/`elif`/`if`/`while`/`until`, after assignments, and after the prefix commands `exec`,
//! `env`, `command`, `builtin`, `nice`, `nohup`, `sudo`, `time`, `timeout <duration>` and `xargs` (with their options;
//! `command` without options).
//!
//! A line whose first word starts with `#` is a comment, and so is a `#` after whitespace outside quotes.

use std::ops::Range;

use super::is_period_name;

const COMMAND_KEYWORDS: [&str; 8] = ["then", "do", "else", "elif", "if", "while", "until", "!"];
const COMMAND_PREFIXES: [&str; 10] = [
    "exec", "env", "command", "builtin", "nice", "nohup", "sudo", "time", "timeout", "xargs",
];
const ARITHMETIC_COMPARISONS: [&str; 6] = ["-eq", "-ne", "-lt", "-le", "-gt", "-ge"];
const EPOCH_VARIABLES: [&str; 3] = ["EPOCHSECONDS", "EPOCHREALTIME", "SECONDS"];
const FIND_TIME_PREDICATES: [&str; 6] = ["-mtime", "-mmin", "-ctime", "-cmin", "-atime", "-amin"];
const GIT_TIME_FILTERS: [&str; 4] = ["--since", "--until", "--before", "--after"];
const OPENSSL_GIVEN_TIME_OPTIONS: [&str; 2] = ["attime", "no_check_time"];
const OPENSSL_VERIFY: &str = "verify";
const RELATIVE_DATE_WORDS: &[&str] = &[
    "ago", "now", "today", "tomorrow", "yesterday", "midnight", "noon", "tea", "this", "last", "next", "sec", "secs",
    "second", "seconds", "min", "mins", "minute", "minutes", "hour", "hours", "day", "days", "week", "weeks",
    "fortnight", "fortnights", "month", "months", "year", "years", "monday", "tuesday", "wednesday", "thursday",
    "friday", "saturday", "sunday",
];
const SHELLS: [&str; 7] = ["sh", "bash", "dash", "zsh", "ksh", "mksh", "ash"];
const SSH_OPTIONS_WITH_VALUE: &str = "BbcDEeFIiJLlmOoPpQRSWw";
const UNQUOTED_ARGUMENT: &str = "%+-./:=@_,";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Output {
    Shown,
    Taken,
}

pub fn line_reads(line: &str) -> Vec<String> {
    reads_in(line, Output::Shown)
}

pub fn source_reads(source: &str) -> Vec<(usize, String)> {
    let mut reads = Vec::new();
    let mut lines = source.lines().enumerate();
    while let Some((first, line)) = lines.next() {
        let mut command = line.to_string();
        while continues_on_next_line(&command) {
            command.pop();
            let Some((_, next)) = lines.next() else { break };
            command.push_str(next);
        }
        reads.extend(line_reads(&command).into_iter().map(|read| (first, read)));
    }
    reads
}

pub fn argv_reads(program: &str, args: &[Option<&str>], output: Output) -> Vec<String> {
    let mut line = quote(program);
    for arg in args {
        line.push(' ');
        line.push_str(&arg.map_or_else(|| "_".to_string(), quote));
    }
    reads_in(&line, output)
}

fn continues_on_next_line(line: &str) -> bool {
    if line.trim_start().starts_with('#') || strip_trailing_comment(line).len() != line.len() {
        return false;
    }
    line.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

fn reads_in(line: &str, output: Output) -> Vec<String> {
    if line.trim_start().starts_with('#') {
        return Vec::new();
    }
    let code = strip_trailing_comment(line);
    let substitutions = substitution_bodies(code);
    let mut reads: Vec<String> =
        substitutions.iter().flat_map(|body| reads_in(&code[body.clone()], Output::Taken)).collect();
    let code = blank(code, &substitutions);
    let wrapped = wrapped_commands(&code, output);
    reads.extend(wrapped.iter().flat_map(|command| reads_in(&command.text, command.output)));
    let code = blank(&code, &wrapped.iter().map(|command| command.span.clone()).collect::<Vec<_>>());
    let code = code.as_str();
    date_reads(code, output, &mut reads);
    epoch_reads(code, &mut reads);
    arithmetic_reads(code, &mut reads);
    conditional_arithmetic_reads(code, &mut reads);
    let_reads(code, &mut reads);
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
    if has_word(code, "openssl") {
        openssl_reads(code, &mut reads);
    }
    period_assignments(code, &mut reads);
    reads
}

fn quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || UNQUOTED_ARGUMENT.contains(c)) {
        word.to_string()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

pub fn is_relative_to_now(value: &str) -> bool {
    let value = value.trim();
    if value.starts_with('@') {
        return false;
    }
    without_expansions(value)
        .to_ascii_lowercase()
        .split(|c: char| !(c.is_ascii_alphabetic() || c == '_'))
        .any(|word| RELATIVE_DATE_WORDS.contains(&word))
}

type Chars = [(usize, char)];

fn char_at(chars: &Chars, at: usize) -> Option<char> {
    chars.get(at).map(|(_, c)| *c)
}

fn single_quote_close(chars: &Chars, from: usize) -> Option<usize> {
    (from..chars.len()).find(|&at| chars[at].1 == '\'')
}

fn backtick_close(chars: &Chars, mut at: usize) -> Option<usize> {
    while at < chars.len() {
        match chars[at].1 {
            '\\' => at += 1,
            '`' => return Some(at),
            _ => {}
        }
        at += 1;
    }
    None
}

fn double_quote_close(chars: &Chars, mut at: usize) -> Option<usize> {
    while at < chars.len() {
        match chars[at].1 {
            '\\' => at += 1,
            '"' => return Some(at),
            '`' => at = backtick_close(chars, at + 1)?,
            '$' if char_at(chars, at + 1) == Some('(') => at = substitution_close(chars, at + 2)?,
            _ => {}
        }
        at += 1;
    }
    None
}

fn substitution_close(chars: &Chars, mut at: usize) -> Option<usize> {
    let mut depth = 0usize;
    while at < chars.len() {
        match chars[at].1 {
            '\\' => at += 1,
            '\'' => at = single_quote_close(chars, at + 1)?,
            '"' => at = double_quote_close(chars, at + 1)?,
            '`' => at = backtick_close(chars, at + 1)?,
            '$' if char_at(chars, at + 1) == Some('(') => at = substitution_close(chars, at + 2)?,
            '(' => depth += 1,
            ')' if depth == 0 => return Some(at),
            ')' => depth -= 1,
            _ => {}
        }
        at += 1;
    }
    None
}

fn brace_close(chars: &Chars, mut at: usize) -> Option<usize> {
    let mut depth = 0usize;
    while at < chars.len() {
        match chars[at].1 {
            '{' => depth += 1,
            '}' if depth == 0 => return Some(at),
            '}' => depth -= 1,
            _ => {}
        }
        at += 1;
    }
    None
}

fn substitution_bodies(code: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = code.char_indices().collect();
    let byte = |at: usize| chars.get(at).map_or(code.len(), |(offset, _)| *offset);
    let mut bodies = Vec::new();
    let mut double = false;
    let mut at = 0;
    while at < chars.len() {
        match chars[at].1 {
            '\\' => at += 1,
            '\'' if !double => at = single_quote_close(&chars, at + 1).unwrap_or(chars.len()),
            '"' => double = !double,
            '$' if char_at(&chars, at + 1) == Some('(') && char_at(&chars, at + 2) == Some('(') => at += 2,
            '$' if char_at(&chars, at + 1) == Some('(') => {
                let close = substitution_close(&chars, at + 2).unwrap_or(chars.len());
                bodies.push(byte(at + 2)..byte(close));
                at = close;
            }
            '`' => {
                let close = backtick_close(&chars, at + 1).unwrap_or(chars.len());
                bodies.push(byte(at + 1)..byte(close));
                at = close;
            }
            _ => {}
        }
        at += 1;
    }
    bodies
}

fn without_expansions(value: &str) -> String {
    let chars: Vec<(usize, char)> = value.char_indices().collect();
    let mut out = String::new();
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at].1;
        let next = char_at(&chars, at + 1);
        let resume = match (c, next) {
            ('$', Some('(')) => substitution_close(&chars, at + 2),
            ('$', Some('{')) => brace_close(&chars, at + 2),
            ('`', _) => backtick_close(&chars, at + 1),
            ('$', Some(n)) if is_name_char(n) => {
                Some((at + 1..chars.len()).take_while(|&k| is_name_char(chars[k].1)).last().unwrap_or(at + 1))
            }
            ('$', Some(n)) if "@*#?$!-".contains(n) => Some(at + 1),
            _ => {
                out.push(c);
                at += 1;
                continue;
            }
        };
        out.push(' ');
        at = resume.map_or(chars.len(), |end| end + 1);
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

fn is_duration(word: &str) -> bool {
    let number = word.strip_suffix(['s', 'm', 'h', 'd']).unwrap_or(word);
    !number.is_empty() && number.chars().all(|c| c.is_ascii_digit() || c == '.')
}

fn is_command_position(left: &str) -> bool {
    if left.trim_end().ends_with(['{', '!']) {
        return true;
    }
    let segment = &left[after_last(left, |c| ";|&(`".contains(c))..];
    let mut words = segment.split_whitespace().peekable();
    while let Some(word) = words.next() {
        if is_assignment(word) || word == "{" || COMMAND_KEYWORDS.contains(&word) {
            continue;
        }
        if !COMMAND_PREFIXES.contains(&word) {
            return false;
        }
        if word == "command" {
            continue;
        }
        while words.next_if(|w| w.len() > 1 && w.starts_with('-')).is_some() {}
        if word == "timeout" {
            words.next_if(|w| is_duration(w));
        }
    }
    true
}

fn positioned_command_start(code: &str, start: usize) -> Option<usize> {
    let command_start = after_last(&code[..start], |c| c != '/' && !is_name_char(c) && c != '.');
    let path = &code[command_start..start];
    let positioned = (path.is_empty() || path.starts_with('/')) && is_command_position(&code[..command_start]);
    positioned.then_some(command_start)
}

struct Word {
    start: usize,
    end: usize,
    value: String,
}

fn is_separator(c: char) -> bool {
    ";|&)<>\n(".contains(c)
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
                '`' => {
                    value.push('`');
                    while i < chars.len() && chars[i].1 != '`' {
                        value.push(chars[i].1);
                        i += 1;
                    }
                    if i < chars.len() {
                        value.push('`');
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
    span: Range<usize>,
    text: String,
    output: Output,
}

fn wrapped_commands(code: &str, output: Output) -> Vec<WrappedCommand> {
    let mut candidates: Vec<(usize, usize, &str)> = SHELLS
        .iter()
        .chain(["eval", "ssh"].iter())
        .flat_map(|name| word_spans(code, name).map(move |(start, end)| (start, end, *name)))
        .collect();
    candidates.sort();
    let mut bodies: Vec<WrappedCommand> = Vec::new();
    for (start, end, name) in candidates {
        if bodies.iter().any(|b| b.span.contains(&start)) || positioned_command_start(code, start).is_none() {
            continue;
        }
        let after = words(code, end, true);
        let body = match name {
            "eval" => Some(&after[..]),
            "ssh" => ssh_command(&after),
            _ => shell_command_string(&after),
        };
        if let Some(body @ [first, ..]) = body {
            let body_end = body[body.len() - 1].end;
            let text = body.iter().map(|w| w.value.as_str()).collect::<Vec<_>>().join(" ");
            let taken = output == Output::Taken || output_is_taken(&code[body_end..]);
            let output = if taken { Output::Taken } else { Output::Shown };
            bodies.push(WrappedCommand { span: first.start..body_end, text, output });
        }
    }
    bodies
}

fn output_is_taken(rest_of_command: &str) -> bool {
    let (arguments, separator) = argument_text(rest_of_command);
    let piped = separator.starts_with('|') && !separator.starts_with("||");
    let redirected = (separator.starts_with('>') && !arguments.ends_with(|c: char| c.is_ascii_digit() && c != '1'))
        || separator.starts_with("&>");
    piped || redirected
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

fn blank(code: &str, spans: &[Range<usize>]) -> String {
    code.char_indices()
        .map(|(at, c)| {
            if spans.iter().any(|span| span.contains(&at)) {
                " ".repeat(c.len_utf8())
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn argument_text(rest: &str) -> (&str, &str) {
    let chars: Vec<(usize, char)> = rest.char_indices().collect();
    let (mut single, mut double) = (false, false);
    let mut at = 0;
    while at < chars.len() {
        match chars[at].1 {
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            '$' if !single && char_at(&chars, at + 1) == Some('(') => {
                at = substitution_close(&chars, at + 2).unwrap_or(chars.len());
            }
            '`' if !single && !double => at = backtick_close(&chars, at + 1).unwrap_or(chars.len()),
            ')' | '`' | ';' | '|' | '&' | '<' | '>' if !single && !double => {
                let offset = chars[at].0;
                return (&rest[..offset], &rest[offset..]);
            }
            _ => {}
        }
        at += 1;
    }
    (rest, "")
}

enum DateInput {
    Clock,
    File,
    Given(String),
}

fn date_input(arguments: &str) -> DateInput {
    let words = words(arguments, 0, true);
    let value_after = |at: usize| words.get(at + 1).map_or_else(String::new, |w| w.value.clone());
    for (at, word) in words.iter().enumerate() {
        if let Some(long) = word.value.strip_prefix("--") {
            let (name, attached) = long.split_once('=').map_or((long, None), |(n, v)| (n, Some(v)));
            match name {
                "date" => return DateInput::Given(attached.map_or_else(|| value_after(at), str::to_string)),
                "file" => return DateInput::File,
                _ => continue,
            }
        }
        let Some(cluster) = word.value.strip_prefix('-') else { continue };
        for (k, flag) in cluster.char_indices() {
            match flag {
                'd' => {
                    let attached = &cluster[k + 1..];
                    return DateInput::Given(if attached.is_empty() { value_after(at) } else { attached.to_string() });
                }
                'f' => return DateInput::File,
                c if c.is_ascii_alphabetic() && c != 'I' => continue,
                _ => break,
            }
        }
    }
    DateInput::Clock
}

fn date_reads(code: &str, output: Output, reads: &mut Vec<String>) {
    for (start, end) in word_spans(code, "date") {
        let Some(command_start) = positioned_command_start(code, start) else { continue };
        let left = before_assignments(&code[..command_start]);
        let (arguments, after) = argument_text(&code[end..]);
        let arguments = arguments.split_whitespace().collect::<Vec<_>>().join(" ");
        if arguments.is_empty() {
            if output == Output::Shown && !opens_substitution(left) && after.is_empty() {
                continue;
            }
        } else if !arguments.trim_start_matches(['\'', '"']).starts_with(['-', '+']) {
            continue;
        }
        if let DateInput::Given(value) = date_input(&arguments) {
            if !value.trim().is_empty() && !is_relative_to_now(&value) {
                continue;
            }
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
        bare_epoch_names(&code[body_start..body_end], reads);
        from = (body_end + 2).min(code.len()).max(body_start);
    }
}

fn bare_epoch_names(expression: &str, reads: &mut Vec<String>) {
    for variable in EPOCH_VARIABLES {
        for (at, _) in expression.match_indices(variable) {
            let before = expression[..at].chars().next_back();
            let after = expression[at + variable.len()..].chars().next();
            let bare = before.is_none_or(|c| !(is_name_char(c) || c == '$' || c == '{'));
            if bare && after.is_none_or(|c| !is_name_char(c)) {
                reads.push(format!("${variable}"));
            }
        }
    }
}

fn conditional_arithmetic_reads(code: &str, reads: &mut Vec<String>) {
    let mut from = 0;
    while let Some(open) = code[from..].find("[[").map(|at| at + from) {
        let body_start = open + 2;
        let body_end = code[body_start..].find("]]").map_or(code.len(), |at| at + body_start);
        if !is_quoted_at(code, open) && is_command_position(&code[..open]) {
            let operands = words(&code[body_start..body_end], 0, false);
            for (at, word) in operands.iter().enumerate() {
                if !ARITHMETIC_COMPARISONS.contains(&word.value.as_str()) {
                    continue;
                }
                for side in [at.checked_sub(1), Some(at + 1)].into_iter().flatten().filter_map(|k| operands.get(k)) {
                    bare_epoch_names(&side.value, reads);
                }
            }
        }
        from = (body_end + 2).min(code.len());
    }
}

fn is_quoted_at(code: &str, at: usize) -> bool {
    let (mut single, mut double, mut escaped) = (false, false, false);
    for c in code[..at].chars() {
        if std::mem::take(&mut escaped) {
            continue;
        }
        match c {
            '\\' if !single => escaped = true,
            '\'' if !double => single = !single,
            '"' if !single => double = !double,
            _ => {}
        }
    }
    single || double
}

fn let_reads(code: &str, reads: &mut Vec<String>) {
    for (start, end) in word_spans(code, "let") {
        if is_quoted_at(code, start) || positioned_command_start(code, start).is_none() {
            continue;
        }
        let (arguments, _) = argument_text(&code[end..]);
        for expression in words(arguments, 0, true) {
            bare_epoch_names(&expression.value, reads);
        }
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

fn openssl_reads(code: &str, reads: &mut Vec<String>) {
    for (start, end) in word_spans(code, "openssl") {
        if positioned_command_start(code, start).is_none() {
            continue;
        }
        let (arguments, _) = argument_text(&code[end..]);
        let arguments = words(arguments, 0, true);
        if arguments.first().map(|w| w.value.as_str()) != Some(OPENSSL_VERIFY) {
            continue;
        }
        let given_time = arguments.iter().any(|w| {
            let option = w.value.trim_start_matches('-');
            w.value.starts_with('-') && OPENSSL_GIVEN_TIME_OPTIONS.contains(&option)
        });
        if !given_time {
            reads.push(format!("openssl {OPENSSL_VERIFY}"));
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
    const CASE_ROWS: usize = 173;

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
        assert_eq!(checked, CASE_ROWS);
    }

    #[test]
    fn relative_and_given_dates() {
        for relative in ["now", "yesterday", "+90 days", "1 day ago", "$created + 90 days", "2.weeks", "last friday"] {
            assert!(is_relative_to_now(relative), "{relative}");
        }
        for given in [
            "@1700000000",
            "@$ts",
            "$ets",
            "${ets}",
            "2026-01-01T19:30:00Z",
            "{created_s}",
            "$start_day 00:00",
            "$(cat \"$state/last-run\")",
            "`cat last`",
            "${last_week}",
        ] {
            assert!(!is_relative_to_now(given), "{given}");
        }
    }
}
