//! One line of shell → its clock reads. Used for tracked shell files and for the lines of Rust string literals
//! (remote command lines, unit texts, fixture scripts).
//!
//! A read is
//! - `date` in command position that prints the clock: with options, in `$(…)`/backticks, or piped/redirected;
//!   `date -d`/`--date`/`-f` convert a given time and are not reads, `date -r FILE` reads a mtime and is one;
//! - `$EPOCHSECONDS`, `$EPOCHREALTIME`, `$SECONDS`;
//! - `printf '%(…)T'`;
//! - `stat` with a `%X`/`%Y`/`%Z`/`%W` (or lowercase) format;
//! - `find -mtime/-mmin/-ctime/-cmin/-atime/-amin/-newer…`;
//! - `-nt`/`-ot` file tests;
//! - `git … --since/--until/--before/--after` with a date relative to now (`2.weeks`, `yesterday`);
//! - an assignment to a calendar-period name (`…_DAYS=`, `…_WEEKS=`, `…_MONTHS=`).
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
const RELATIVE_DATE_WORDS: [&str; 12] = [
    "ago", "now", "today", "yesterday", "midnight", "noon", "second", "minute", "hour", "day", "week", "month",
];

pub fn line_reads(line: &str) -> Vec<String> {
    if line.trim_start().starts_with('#') {
        return Vec::new();
    }
    let code = strip_trailing_comment(line);
    let mut reads = Vec::new();
    date_reads(code, &mut reads);
    epoch_reads(code, &mut reads);
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
        for (at, word) in words.iter().enumerate() {
            let (option, attached) = word.split_once('=').map_or((*word, None), |(o, v)| (o, Some(v)));
            let value = attached.map_or_else(
                || words[at + 1..].iter().take_while(|w| !w.starts_with('-')).copied().collect::<Vec<_>>().join(" "),
                str::to_string,
            );
            if GIT_TIME_FILTERS.contains(&option) && is_relative_to_now(&value) {
                reads.push(format!("git {option}"));
            }
        }
    }
    period_assignments(code, &mut reads);
    reads
}

/// A git date that git resolves against the current time (`2.weeks`, `3 days ago`, `yesterday`); an absolute stamp or an
/// interpolated value is a given time.
fn is_relative_to_now(value: &str) -> bool {
    let value = value.to_ascii_lowercase();
    RELATIVE_DATE_WORDS.iter().any(|word| value.contains(word))
}

fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The byte index just after the last character of `text` that matches `pred` (0 when none does).
fn after_last(text: &str, pred: impl Fn(char) -> bool) -> usize {
    text.char_indices()
        .rev()
        .find(|(_, c)| pred(*c))
        .map_or(0, |(at, c)| at + c.len_utf8())
}

/// The byte ranges of `word` in `code` bounded by non-name characters (and not by `-` or `.`).
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

/// The text before a command word with leading assignments (`TZ=UTC LC_ALL=C date`) removed.
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

/// The arguments of a command: the text up to the first unquoted `)`, backtick, `;`, `|`, `&`, `<` or `>`.
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

/// `date -d`, `--date`, `-f`, `--file` (short clusters too, `-ud`) convert a given time instead of reading the clock.
fn converts_given_time(arguments: &str) -> bool {
    arguments.split_whitespace().any(|word| {
        let word = word.trim_start_matches(['\'', '"']);
        if let Some(long) = word.strip_prefix("--") {
            let name = long.split('=').next().unwrap_or(long);
            return name == "date" || name == "file";
        }
        match word.strip_prefix('-') {
            Some(cluster) if cluster.starts_with(|c: char| c.is_ascii_alphabetic()) => {
                for flag in cluster.chars() {
                    match flag {
                        'd' | 'f' => return true,
                        'I' => return false,
                        c if c.is_ascii_alphabetic() => continue,
                        _ => return false,
                    }
                }
                false
            }
            _ => false,
        }
    })
}

fn date_reads(code: &str, reads: &mut Vec<String>) {
    for (start, end) in word_spans(code, "date") {
        let command_start = after_last(&code[..start], |c| c != '/' && !is_name_char(c) && c != '.');
        let path = &code[command_start..start];
        if !(path.is_empty() || path.starts_with('/')) {
            continue;
        }
        let left = before_assignments(&code[..command_start]);
        if !is_command_position(left) {
            continue;
        }
        let (arguments, after) = argument_text(&code[end..]);
        let arguments = arguments.split_whitespace().collect::<Vec<_>>().join(" ");
        if arguments.is_empty() {
            if !opens_substitution(left) && after.is_empty() {
                continue;
            }
        } else if !arguments.trim_start_matches(['\'', '"']).starts_with(['-', '+']) {
            continue;
        }
        if converts_given_time(&arguments) {
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

/// The line without a trailing shell comment: a `#` after whitespace, outside single/double quotes.
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
    use super::line_reads;

    /// One case per row: a shell line, then the reads it must give (none = no read). The cases live in the data plane
    /// so that this source holds no shell clock read of its own.
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
        assert!(checked > 20, "{checked}");
    }
}
