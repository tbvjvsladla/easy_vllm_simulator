//! The part of Python's `argparse` (3.12) the three recipe command lines use: long options with exactly one value or
//! none, `-h/--help`, abbreviations, `--option=value`, `int`/`float` value types, one required mutually exclusive group,
//! the usage/help layout (`HelpFormatter` and `textwrap`) and the error report (`usage` + `prog: error: message`, status 2).
//! Width follows `COLUMNS` (default 80) minus two, as `shutil.get_terminal_size` does without a terminal.

use agent_runtime_support::pynumber::{float_from_str, int_from_str};
use agent_runtime_support::pystr::PyStr;
use num_bigint::BigInt;
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Help,
    Flag,
    Text,
    /// A text option that may be given several times (`action="append"`): every value is kept, in order (`Value::Texts`).
    TextAppend,
    Int,
    Float,
    /// A text value that must be one of the listed strings (`choices=`): shown as `{a,b}`, refused with `invalid choice`.
    Choice(&'static [&'static str]),
}

pub struct Opt {
    pub flags: &'static [&'static str],
    pub dest: &'static str,
    pub kind: Kind,
    pub help: Option<&'static str>,
    pub required: bool,
    /// Index of the mutually exclusive group the option belongs to.
    pub group: Option<usize>,
}

/// One optional positional argument (`nargs="?"`): the first argument that does not look like an option, if any.
pub struct Positional {
    pub dest: &'static str,
    pub help: Option<&'static str>,
}

pub struct Spec {
    pub prog: &'static str,
    pub description: &'static str,
    pub options: Vec<Opt>,
    /// Whether the (single) mutually exclusive group is required.
    pub group_required: bool,
    /// The optional positional argument, when the program has one.
    pub positional: Option<Positional>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Text(PyStr),
    Texts(Vec<PyStr>),
    Int(BigInt),
    Float(f64),
    Flag,
}

/// What a parse ended with: values by destination, or the text and status of an early exit.
pub enum Outcome {
    Values(BTreeMap<&'static str, Value>),
    Exit {
        status: i32,
        stdout: PyStr,
        stderr: PyStr,
    },
}

/// The same with a message that is text of the program's own arguments (it may hold surrogate escapes).
pub fn exit_with_text(spec: &Spec, width: usize, message: &PyStr) -> Outcome {
    let usage = format_usage(spec, width);
    let mut stderr = PyStr::new(&format!("{usage}{}: error: ", spec.prog));
    stderr.push_py(message);
    stderr.push_str("\n");
    Outcome::Exit {
        status: 2,
        stdout: PyStr::default(),
        stderr,
    }
}

pub fn exit_with(spec: &Spec, width: usize, message: &str) -> Outcome {
    let usage = format_usage(spec, width);
    Outcome::Exit {
        status: 2,
        stdout: PyStr::default(),
        stderr: PyStr::new(&format!("{usage}{}: error: {message}\n", spec.prog)),
    }
}

fn argument_error(spec: &Spec, width: usize, option: &Opt, message: &str) -> Outcome {
    exit_with(
        spec,
        width,
        &format!("argument {}: {message}", option.flags.join("/")),
    )
}

/// The terminal width argparse formats for: `COLUMNS` when it is a positive integer, otherwise 80, minus two.
pub fn terminal_width(columns: Option<&str>) -> usize {
    let width = columns
        .and_then(|text| text.trim().parse::<usize>().ok())
        .filter(|&width| width > 0)
        .unwrap_or(80);
    width.saturating_sub(2)
}

/// `shutil.get_terminal_size()[0]` for a program whose standard output may be a terminal: the `COLUMNS` environment value when
/// it is a positive integer (`int()` rules); otherwise the width the terminal on descriptor 1 reports; otherwise (not a
/// terminal, or it reports 0) 80. The result is the text [`terminal_width`] and the commands take as their `COLUMNS`.
pub fn effective_columns(environment: Option<&str>) -> String {
    use agent_runtime_support::pynumber::int_from_str;
    use num_traits::ToPrimitive;
    let from_environment = environment
        .and_then(|text| int_from_str(&PyStr::new(text)).ok())
        .filter(|number| number.sign() == num_bigint::Sign::Plus)
        .map(|number| number.to_u64().unwrap_or(u64::from(u32::MAX)));
    let columns = from_environment.unwrap_or_else(|| {
        crate::terminal::stdout_columns()
            .filter(|&columns| columns > 0)
            .map_or(80, u64::from)
    });
    columns.to_string()
}

struct Hit {
    option: Option<usize>,
    text: String,
    explicit: Option<String>,
}

fn option_strings(spec: &Spec) -> Vec<(&'static str, usize)> {
    let mut all = Vec::new();
    for (index, option) in spec.options.iter().enumerate() {
        for &flag in option.flags {
            all.push((flag, index));
        }
    }
    all
}

/// How an argument reads to the classifier: every code point as it is, except a lone surrogate (a byte that is not UTF-8,
/// kept by `surrogateescape`), which becomes U+FFFD. Only the ASCII syntax of the argument (`-`, `--`, `=`, spaces, digits) is
/// ever looked at, so this view is never used as a value: values, messages and the positional come from the original text.
fn syntax_view(argument: &PyStr) -> String {
    argument
        .code_points()
        .iter()
        .map(|&point| char::from_u32(point).unwrap_or('\u{FFFD}'))
        .collect()
}

/// `_parse_optional`: how one argument reads (`Ok(None)` = a positional-looking string). `original` is the argument as given,
/// used only where its own text is written into a message.
fn classify(
    spec: &Spec,
    width: usize,
    arg: &str,
    original: &PyStr,
) -> Result<Option<Hit>, Outcome> {
    let strings = option_strings(spec);
    if arg.is_empty() || !arg.starts_with('-') {
        return Ok(None);
    }
    if let Some(&(text, index)) = strings.iter().find(|(flag, _)| *flag == arg) {
        return Ok(Some(Hit {
            option: Some(index),
            text: text.to_string(),
            explicit: None,
        }));
    }
    if arg.chars().count() == 1 {
        return Ok(None);
    }
    if let Some((head, tail)) = arg.split_once('=') {
        if let Some(&(text, index)) = strings.iter().find(|(flag, _)| *flag == head) {
            return Ok(Some(Hit {
                option: Some(index),
                text: text.to_string(),
                explicit: Some(tail.to_string()),
            }));
        }
    }
    let mut tuples: Vec<Hit> = Vec::new();
    let mut chars = arg.chars();
    let (first, second) = (chars.next(), chars.next());
    if first == Some('-') && second == Some('-') {
        let (prefix, explicit) = match arg.split_once('=') {
            Some((head, tail)) => (head, Some(tail.to_string())),
            None => (arg, None),
        };
        for &(flag, index) in &strings {
            if flag.starts_with(prefix) {
                tuples.push(Hit {
                    option: Some(index),
                    text: flag.to_string(),
                    explicit: explicit.clone(),
                });
            }
        }
    } else if first == Some('-') {
        let short: String = arg.chars().take(2).collect();
        let rest: String = arg.chars().skip(2).collect();
        for &(flag, index) in &strings {
            if flag == short {
                tuples.push(Hit {
                    option: Some(index),
                    text: flag.to_string(),
                    explicit: Some(rest.clone()),
                });
            } else if flag.starts_with(arg) {
                tuples.push(Hit {
                    option: Some(index),
                    text: flag.to_string(),
                    explicit: None,
                });
            }
        }
    }
    if tuples.len() > 1 {
        let names: Vec<&str> = tuples.iter().map(|hit| hit.text.as_str()).collect();
        let mut message = PyStr::new("ambiguous option: ");
        message.push_py(original);
        message.push_str(&format!(" could match {}", names.join(", ")));
        return Err(exit_with_text(spec, width, &message));
    }
    if let Some(hit) = tuples.pop() {
        return Ok(Some(hit));
    }
    let negative_number = {
        let body = arg.strip_prefix('-').unwrap_or("");
        let mut parts = body.splitn(2, '.');
        let integer = parts.next().unwrap_or("");
        let fraction = parts.next();
        !body.is_empty()
            && integer.chars().all(|c| c.is_ascii_digit())
            && fraction.map_or(!integer.is_empty(), |f| {
                f.chars().all(|c| c.is_ascii_digit())
            })
            && (!integer.is_empty() || fraction.is_some_and(|f| !f.is_empty()))
    };
    if negative_number || arg.contains(' ') {
        return Ok(None);
    }
    Ok(Some(Hit {
        option: None,
        text: arg.to_string(),
        explicit: None,
    }))
}

/// Why a value was refused: the name of the type it does not read as, or that it is not one of the choices.
enum Refusal {
    Type(&'static str),
    Choice,
}

/// The value of an option as it was given: text keeps every character (a byte that is not UTF-8 survives as a surrogate escape).
fn convert(option: &Opt, source: &PyStr) -> Result<Value, Refusal> {
    match option.kind {
        Kind::Int => int_from_str(source)
            .map(Value::Int)
            .map_err(|_| Refusal::Type("int")),
        Kind::Float => float_from_str(source)
            .map(Value::Float)
            .map_err(|_| Refusal::Type("float")),
        Kind::Choice(choices) if !choices.iter().any(|choice| source.equals(choice)) => {
            Err(Refusal::Choice)
        }
        _ => Ok(Value::Text(source.clone())),
    }
}

/// What follows the first `=` of an argument (or, for a combined short option, what follows its two first characters),
/// from the argument as it was given.
fn explicit_value(argument: &PyStr, joined_with_equals: bool) -> PyStr {
    let points = argument.code_points();
    let start = if joined_with_equals {
        points
            .iter()
            .position(|&point| point == u32::from('='))
            .map_or(points.len(), |at| at + 1)
    } else {
        2.min(points.len())
    };
    PyStr::from_code_points(points[start..].to_vec())
}

/// `parser.parse_args(args)` for the given spec.
pub fn parse(spec: &Spec, args: &[PyStr], columns: Option<&str>) -> Outcome {
    let width = terminal_width(columns);
    let texts: Vec<String> = args.iter().map(syntax_view).collect();
    let mut hits: Vec<Option<Hit>> = Vec::new();
    // `--` ends the options: the first one is a marker, every argument after it reads as a positional.
    let mut marker: Option<usize> = None;
    for (position, text) in texts.iter().enumerate() {
        if marker.is_some() {
            hits.push(None);
        } else if text == "--" {
            marker = Some(position);
            hits.push(None);
        } else {
            match classify(spec, width, text, &args[position]) {
                Ok(hit) => hits.push(hit),
                Err(outcome) => return outcome,
            }
        }
    }
    let mut values: BTreeMap<&'static str, Value> = BTreeMap::new();
    let mut seen: Vec<usize> = Vec::new();
    // Arguments no option took, in order, with whether each reads as a positional (and where it was).
    let mut extras: Vec<(PyStr, bool, usize)> = Vec::new();
    let mut at = 0;
    while at < texts.len() {
        let Some(hit) = &hits[at] else {
            // With a positional to take the arguments after it, the marker only separates (argparse removes it from the values).
            if Some(at) != marker || spec.positional.is_none() {
                extras.push((args[at].clone(), true, at));
            }
            at += 1;
            continue;
        };
        let Some(index) = hit.option else {
            extras.push((args[at].clone(), false, at));
            at += 1;
            continue;
        };
        let option = &spec.options[index];
        let takes_value = matches!(
            option.kind,
            Kind::Text | Kind::TextAppend | Kind::Int | Kind::Float | Kind::Choice(_)
        );
        let value_text: Option<PyStr> = if let Some(explicit) = &hit.explicit {
            if takes_value {
                let value =
                    explicit_value(&args[at], args[at].code_points().contains(&u32::from('=')));
                at += 1;
                Some(value)
            } else if hit.text.chars().nth(1) != Some('-')
                && !explicit.is_empty()
                && option.kind == Kind::Help
            {
                // `-hx`: the combined short flags run the help action first, which ends the program.
                at += 1;
                None
            } else {
                let given =
                    explicit_value(&args[at], args[at].code_points().contains(&u32::from('=')));
                return argument_error(
                    spec,
                    width,
                    option,
                    &format!("ignored explicit argument {}", given.repr()),
                );
            }
        } else if takes_value {
            match hits.get(at + 1) {
                // An option's value is an ordinary argument, never the `--` marker.
                Some(None) if at + 1 < texts.len() && Some(at + 1) != marker => {
                    at += 2;
                    Some(args[at - 1].clone())
                }
                _ => return argument_error(spec, width, option, "expected one argument"),
            }
        } else {
            at += 1;
            None
        };
        if option.kind == Kind::Help {
            return Outcome::Exit {
                status: 0,
                stdout: format_help(spec, width),
                stderr: PyStr::default(),
            };
        }
        let value = match &value_text {
            Some(text) => match convert(option, text) {
                Ok(value) => value,
                Err(Refusal::Type(name)) => {
                    return argument_error(
                        spec,
                        width,
                        option,
                        &format!("invalid {name} value: {}", text.repr()),
                    );
                }
                Err(Refusal::Choice) => {
                    let Kind::Choice(choices) = option.kind else {
                        unreachable!("only a choice option refuses a choice")
                    };
                    let shown: Vec<String> = choices
                        .iter()
                        .map(|choice| PyStr::new(choice).repr())
                        .collect();
                    return argument_error(
                        spec,
                        width,
                        option,
                        &format!(
                            "invalid choice: {} (choose from {})",
                            text.repr(),
                            shown.join(", ")
                        ),
                    );
                }
            },
            None => Value::Flag,
        };
        if let Some(group) = option.group {
            if let Some(&other) = seen
                .iter()
                .find(|&&s| spec.options[s].group == Some(group) && s != index)
            {
                return argument_error(
                    spec,
                    width,
                    option,
                    &format!(
                        "not allowed with argument {}",
                        spec.options[other].flags.join("/")
                    ),
                );
            }
        }
        seen.push(index);
        if let (Kind::TextAppend, Value::Text(given)) = (option.kind, &value) {
            match values.get_mut(option.dest) {
                Some(Value::Texts(list)) => list.push(given.clone()),
                _ => {
                    values.insert(option.dest, Value::Texts(vec![given.clone()]));
                }
            }
        } else {
            values.insert(option.dest, value);
        }
    }
    if let Some(positional) = &spec.positional {
        if let Some(found) = extras
            .iter()
            .position(|(_, looks_positional, _)| *looks_positional)
        {
            let (_, _, index) = extras.remove(found);
            values.insert(positional.dest, Value::Text(args[index].clone()));
        }
    }
    let missing: Vec<String> = spec
        .options
        .iter()
        .enumerate()
        .filter(|(index, option)| option.required && !seen.contains(index))
        .map(|(_, option)| option.flags.join("/"))
        .collect();
    if !missing.is_empty() {
        return exit_with(
            spec,
            width,
            &format!(
                "the following arguments are required: {}",
                missing.join(", ")
            ),
        );
    }
    if spec.group_required && !seen.iter().any(|&s| spec.options[s].group.is_some()) {
        let names: Vec<String> = spec
            .options
            .iter()
            .filter(|option| option.group.is_some())
            .map(|option| option.flags.join("/"))
            .collect();
        return exit_with(
            spec,
            width,
            &format!("one of the arguments {} is required", names.join(" ")),
        );
    }
    if !extras.is_empty() {
        let mut message = PyStr::new("unrecognized arguments: ");
        for (position, (text, _, _)) in extras.iter().enumerate() {
            if position > 0 {
                message.push_str(" ");
            }
            message.push_py(text);
        }
        return exit_with_text(spec, width, &message);
    }
    Outcome::Values(values)
}

// ── usage and help ───────────────────────────────────────────────────────────────

fn metavar(option: &Opt) -> String {
    match option.kind {
        Kind::Choice(choices) => format!("{{{}}}", choices.join(",")),
        _ => option.dest.to_uppercase(),
    }
}

fn invocation(option: &Opt) -> String {
    let takes_value = matches!(
        option.kind,
        Kind::Text | Kind::TextAppend | Kind::Int | Kind::Float | Kind::Choice(_)
    );
    option
        .flags
        .iter()
        .map(|flag| {
            if takes_value {
                format!("{flag} {}", metavar(option))
            } else {
                (*flag).to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn usage_item(option: &Opt) -> String {
    let flag = option.flags[0];
    let takes_value = matches!(
        option.kind,
        Kind::Text | Kind::TextAppend | Kind::Int | Kind::Float | Kind::Choice(_)
    );
    let body = if takes_value {
        format!("{flag} {}", metavar(option))
    } else {
        flag.to_string()
    };
    if option.required {
        body
    } else {
        format!("[{body}]")
    }
}

/// The optionals part of the usage line, a required exclusive group shown as `(a | b)`.
fn optionals_usage(spec: &Spec) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut group_done = false;
    for option in &spec.options {
        if option.group.is_some() {
            if !group_done {
                let members: Vec<String> = spec
                    .options
                    .iter()
                    .filter(|o| o.group.is_some())
                    .map(|o| {
                        let takes_value = matches!(
                            o.kind,
                            Kind::Text
                                | Kind::TextAppend
                                | Kind::Int
                                | Kind::Float
                                | Kind::Choice(_)
                        );
                        if takes_value {
                            format!("{} {}", o.flags[0], metavar(o))
                        } else {
                            o.flags[0].to_string()
                        }
                    })
                    .collect();
                let joined = members.join(" | ");
                parts.push(if spec.group_required {
                    format!("({joined})")
                } else {
                    format!("[{joined}]")
                });
                group_done = true;
            }
        } else {
            parts.push(usage_item(option));
        }
    }
    parts.join(" ")
}

/// `re.findall(r'\(.*?\)+(?=\s|$)|\[.*?\]+(?=\s|$)|\S+', usage)`.
fn usage_parts(usage: &str) -> Vec<String> {
    let chars: Vec<char> = usage.chars().collect();
    let mut parts = Vec::new();
    let mut at = 0;
    while at < chars.len() {
        if chars[at].is_whitespace() {
            at += 1;
            continue;
        }
        let closer = match chars[at] {
            '(' => Some(')'),
            '[' => Some(']'),
            _ => None,
        };
        let mut end = None;
        if let Some(close) = closer {
            let mut probe = at + 1;
            while probe < chars.len() {
                if chars[probe] == close {
                    let mut stop = probe + 1;
                    while stop < chars.len() && chars[stop] == close {
                        stop += 1;
                    }
                    if stop == chars.len() || chars[stop].is_whitespace() {
                        end = Some(stop);
                        break;
                    }
                }
                probe += 1;
            }
        }
        let stop = end.unwrap_or_else(|| {
            let mut stop = at;
            while stop < chars.len() && !chars[stop].is_whitespace() {
                stop += 1;
            }
            stop
        });
        parts.push(chars[at..stop].iter().collect());
        at = stop;
    }
    parts
}

fn lines_of(
    parts: &[String],
    indent: &str,
    text_width: usize,
    prefix: Option<&str>,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut line: Vec<&str> = Vec::new();
    let indent_length = indent.chars().count();
    let mut line_length = match prefix {
        Some(prefix) => prefix.chars().count() as isize - 1,
        None => indent_length as isize - 1,
    };
    for part in parts {
        let length = part.chars().count() as isize;
        if line_length + 1 + length > text_width as isize && !line.is_empty() {
            lines.push(format!("{indent}{}", line.join(" ")));
            line.clear();
            line_length = indent_length as isize - 1;
        }
        line.push(part);
        line_length += length + 1;
    }
    if !line.is_empty() {
        lines.push(format!("{indent}{}", line.join(" ")));
    }
    if prefix.is_some() {
        if let Some(first) = lines.first_mut() {
            *first = first.chars().skip(indent_length).collect();
        }
    }
    lines
}

/// `format_usage()`: `usage: ...` and a newline, wrapped under the program name when too long.
pub fn format_usage(spec: &Spec, width: usize) -> String {
    let prefix = "usage: ";
    let optionals = optionals_usage(spec);
    let positional = spec.positional.as_ref().map(|p| format!("[{}]", p.dest));
    let usage = match &positional {
        Some(positional) => format!("{} {} {}", spec.prog, optionals, positional),
        None => format!("{} {}", spec.prog, optionals),
    };
    let text_width = width.max(11);
    let text = if prefix.chars().count() + usage.chars().count() > text_width {
        let parts = usage_parts(&optionals);
        let pieces: Vec<String> = positional.iter().cloned().collect();
        if (prefix.chars().count() + spec.prog.chars().count()) as f64 <= 0.75 * text_width as f64 {
            let indent = " ".repeat(prefix.chars().count() + spec.prog.chars().count() + 1);
            let mut all = vec![spec.prog.to_string()];
            all.extend(parts);
            let mut lines = lines_of(&all, &indent, text_width, Some(prefix));
            // The positionals are wrapped on lines of their own, under the optionals.
            lines.extend(lines_of(&pieces, &indent, text_width, None));
            lines.join("\n")
        } else {
            // A program name too long for the width: it stands alone and the arguments are indented under `usage:`.
            let indent = " ".repeat(prefix.chars().count());
            let mut everything = parts.clone();
            everything.extend(pieces.clone());
            let mut lines = lines_of(&everything, &indent, text_width, None);
            if lines.len() > 1 {
                lines = lines_of(&parts, &indent, text_width, None);
                lines.extend(lines_of(&pieces, &indent, text_width, None));
            }
            lines.insert(0, spec.prog.to_string());
            lines.join("\n")
        }
    } else {
        usage
    };
    format!("{prefix}{text}\n")
}

fn is_letter(c: char) -> bool {
    c == '_' || c.is_alphabetic()
}

/// The chunks `textwrap` splits a whitespace-normalised text into (words, single spaces, and words cut after hyphens).
fn chunks_of(text: &str) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    for (index, word) in text.split(' ').enumerate() {
        if index > 0 {
            chunks.push(" ".to_string());
        }
        let chars: Vec<char> = word.chars().collect();
        let mut start = 0;
        for at in 0..chars.len() {
            if chars[at] != '-' || at < 2 || at + 1 >= chars.len() {
                continue;
            }
            let two_letters = is_letter(chars[at - 1]) && is_letter(chars[at - 2]);
            let alternating = at >= 3
                && is_letter(chars[at - 1])
                && chars[at - 2] == '-'
                && is_letter(chars[at - 3]);
            let next_ok = is_letter(chars[at + 1])
                && (chars.get(at + 2).is_some_and(|&c| is_letter(c))
                    || (chars.get(at + 2) == Some(&'-')
                        && chars.get(at + 3).is_some_and(|&c| is_letter(c))));
            if (two_letters || alternating) && next_ok {
                chunks.push(chars[start..=at].iter().collect());
                start = at + 1;
            }
        }
        if start < chars.len() || chars.is_empty() {
            chunks.push(chars[start..].iter().collect());
        }
    }
    chunks.retain(|chunk| !chunk.is_empty());
    chunks
}

/// `textwrap.wrap(text, width)` after argparse's whitespace normalisation.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let normal = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chunks: Vec<String> = chunks_of(&normal);
    chunks.reverse();
    let mut lines: Vec<String> = Vec::new();
    while !chunks.is_empty() {
        let mut current: Vec<String> = Vec::new();
        let mut length = 0usize;
        if chunks.last().is_some_and(|c| c.trim().is_empty()) && !lines.is_empty() {
            chunks.pop();
        }
        while let Some(chunk) = chunks.last() {
            let chunk_length = chunk.chars().count();
            if length + chunk_length <= width {
                length += chunk_length;
                current.push(chunks.pop().expect("a chunk"));
            } else {
                break;
            }
        }
        if let Some(chunk) = chunks.pop_if(|c| c.chars().count() > width) {
            let room = width.saturating_sub(length).max(1);
            let (head, tail): (String, String) = (
                chunk.chars().take(room).collect(),
                chunk.chars().skip(room).collect(),
            );
            current.push(head);
            if !tail.is_empty() {
                chunks.push(tail);
            }
        }
        if current.last().is_some_and(|c| c.trim().is_empty()) {
            current.pop();
        }
        if !current.is_empty() {
            lines.push(current.concat());
        }
    }
    lines
}

/// `format_help()`.
pub fn format_help(spec: &Spec, width: usize) -> PyStr {
    let text_width = width.max(11);
    let mut out = String::new();
    out.push_str(&format_usage(spec, width));
    out.push('\n');
    out.push_str(&wrap(spec.description, text_width).join("\n"));
    out.push_str("\n\n");
    let invocations: Vec<String> = spec.options.iter().map(invocation).collect();
    let positional_header = spec.positional.as_ref().map(|p| p.dest.to_string());
    let max_length = invocations
        .iter()
        .chain(positional_header.iter())
        .map(|text| text.chars().count())
        .max()
        .unwrap_or(0)
        + 2;
    let max_help_position = 24usize.min(width.saturating_sub(20).max(4));
    let help_position = (max_length + 2).min(max_help_position);
    let help_width = width.saturating_sub(help_position).max(11);
    let action_width = help_position.saturating_sub(4);
    let entry = |out: &mut String, header: &str, help: Option<&str>| {
        let mut first = true;
        match help.filter(|text| !text.trim().is_empty()) {
            None => out.push_str(&format!("  {header}\n")),
            Some(text) => {
                let lines = wrap(text, help_width);
                if header.chars().count() <= action_width {
                    out.push_str(&format!("  {header:<action_width$}  "));
                } else {
                    out.push_str(&format!("  {header}\n"));
                    out.push_str(&" ".repeat(help_position));
                }
                for line in lines {
                    if !first {
                        out.push_str(&" ".repeat(help_position));
                    }
                    out.push_str(&line);
                    out.push('\n');
                    first = false;
                }
            }
        }
    };
    if let Some(positional) = &spec.positional {
        out.push_str("positional arguments:\n");
        entry(&mut out, positional.dest, positional.help);
        out.push('\n');
    }
    out.push_str("options:\n");
    for (option, header) in spec.options.iter().zip(&invocations) {
        let help = if option.kind == Kind::Help {
            Some("show this help message and exit")
        } else {
            option.help
        };
        entry(&mut out, header, help);
    }
    PyStr::new(&out)
}
