//! One TypeScript source → its clock reads:
//! - `Date.now()` and `performance.now()`, and the same functions taken as values (`Date.now`, `performance.now`;
//!   `now` destructured from `Date`, as in `const { now } = Date` or `{ now: n } = Date`, is the value `Date.now`);
//! - `new Date()` with no argument, also written `new Date`;
//! - `Date(…)` called without `new` (the current time as a string, whatever the arguments);
//! - `process.hrtime`;
//! - the file-time properties `.mtime`/`.atime`/`.ctime`/`.birthtime` (and their `…Ms`).
//!
//! `globalThis.Date` and `window.Date` are `Date`. `//` and `/* … */` comments are skipped; text inside quotes, template
//! literals and regular expression literals is not code, but a `${…}` inside a template literal is. A `/` starts a
//! regular expression where an operand is expected: at the start of a line, after `=>`, after `return`, `typeof` or
//! `case`, and after one of `( , = : [ ! & | ? { } ; ~ + - * %`; it runs to the next `/` outside a `[…]` class on the
//! same line, then its flags. A comment, string or template literal still open at the end of the file is an input
//! error.

use super::Finding;

const CLOCK_FUNCTIONS: [&str; 2] = ["Date.now", "performance.now"];
const GLOBAL_OBJECTS: [&str; 2] = ["globalThis.", "window."];
const FILE_TIME_PROPERTIES: [&str; 8] = [
    ".mtime", ".atime", ".ctime", ".birthtime", ".mtimeMs", ".atimeMs", ".ctimeMs", ".birthtimeMs",
];
const OPERATORS_BEFORE_AN_OPERAND: &str = "(,=:[!&|?{};~+-*%";
const KEYWORDS_BEFORE_AN_OPERAND: [&str; 3] = ["return", "typeof", "case"];

pub fn find(source: &str) -> Result<Vec<Finding>, String> {
    let lines: Vec<String> = code_only_lines(source)?.iter().map(|code| without_global_objects(code)).collect();
    let mut found = Vec::new();
    for (index, code) in lines.iter().enumerate() {
        for text in line_reads(code) {
            found.push(Finding { line: index + 1, text });
        }
    }
    found.extend(now_destructured_from_date(&lines.join("\n")));
    Ok(found)
}

fn now_destructured_from_date(code: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    for at in whole_name_offsets(code, "Date") {
        if code[at + "Date".len()..].trim_start().starts_with(['.', '(', '[']) {
            continue;
        }
        let Some(left) = code[..at].trim_end().strip_suffix('=') else { continue };
        if left.ends_with(['=', '!', '<', '>']) {
            continue;
        }
        let Some(pattern) = left.trim_end().strip_suffix('}') else { continue };
        let Some(open) = matching_open_brace(pattern) else { continue };
        if names_now(&pattern[open + 1..]) {
            found.push(Finding { line: code[..at].matches('\n').count() + 1, text: "Date.now".to_string() });
        }
    }
    found
}

fn matching_open_brace(text: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (at, c) in text.char_indices().rev() {
        match c {
            '}' => depth += 1,
            '{' if depth == 0 => return Some(at),
            '{' => depth -= 1,
            _ => {}
        }
    }
    None
}

fn names_now(object_pattern: &str) -> bool {
    let mut depth = 0usize;
    let mut property = String::new();
    let mut properties = Vec::new();
    for c in object_pattern.chars() {
        match c {
            '{' | '[' | '(' => depth += 1,
            '}' | ']' | ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                properties.push(std::mem::take(&mut property));
                continue;
            }
            _ => {}
        }
        property.push(c);
    }
    properties.push(property);
    properties.iter().any(|property| property.split([':', '=']).next().map(str::trim) == Some("now"))
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

fn whole_name_offsets<'a>(code: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    code.match_indices(word).filter_map(move |(at, _)| {
        let before = code[..at].chars().next_back();
        let after = code[at + word.len()..].chars().next();
        let bounded = before.is_none_or(|c| !(is_ident_char(c) || c == '.')) && after.is_none_or(|c| !is_ident_char(c));
        bounded.then_some(at)
    })
}

fn without_global_objects(code: &str) -> String {
    let mut code = code.to_string();
    for global in GLOBAL_OBJECTS {
        let qualified = format!("{global}Date");
        loop {
            let Some(at) = whole_name_offsets(&code, &qualified).next() else { break };
            code.replace_range(at..at + global.len(), "");
        }
    }
    code
}

fn line_reads(code: &str) -> Vec<String> {
    let mut reads = Vec::new();
    for function in CLOCK_FUNCTIONS {
        for at in whole_name_offsets(code, function) {
            let called = code[at + function.len()..].trim_start().starts_with('(');
            reads.push(if called { format!("{function}()") } else { function.to_string() });
        }
    }
    for at in whole_name_offsets(code, "Date") {
        let rest = code[at + "Date".len()..].trim_start();
        let constructed = code[..at].trim_end().strip_suffix("new").is_some_and(|left| {
            left.chars().next_back().is_none_or(|c| !is_ident_char(c))
        });
        if constructed {
            let no_argument = rest.strip_prefix('(').is_none_or(|args| args.trim_start().starts_with(')'));
            if no_argument {
                reads.push("new Date()".to_string());
            }
        } else if rest.starts_with('(') {
            reads.push("Date()".to_string());
        }
    }
    for _ in whole_name_offsets(code, "process.hrtime") {
        reads.push("process.hrtime".to_string());
    }
    for property in FILE_TIME_PROPERTIES {
        for (at, _) in code.match_indices(property) {
            let next = code[at + property.len()..].chars().next();
            if next.is_none_or(|c| !is_ident_char(c)) {
                reads.push(property.to_string());
            }
        }
    }
    reads
}

enum Mode {
    Code { open_braces_in_template: Option<usize> },
    Quoted(char),
    Template,
    BlockComment,
}

impl Mode {
    fn name(&self) -> &'static str {
        match self {
            Mode::Code { .. } => "`${…}` of a template literal",
            Mode::Quoted(_) => "string literal",
            Mode::Template => "template literal",
            Mode::BlockComment => "block comment",
        }
    }
}

enum Before {
    LineStart,
    Operator { char: char, after_equals: bool },
    Word(String),
    Operand,
}

impl Before {
    fn expects_an_operand(&self) -> bool {
        match self {
            Before::LineStart => true,
            Before::Operator { char: '>', after_equals } => *after_equals,
            Before::Operator { char, .. } => OPERATORS_BEFORE_AN_OPERAND.contains(*char),
            Before::Word(word) => KEYWORDS_BEFORE_AN_OPERAND.contains(&word.as_str()),
            Before::Operand => false,
        }
    }

    fn then(self, c: char, previous: Option<char>) -> Before {
        match self {
            Before::Word(mut word) if is_ident_char(c) && previous.is_some_and(is_ident_char) => {
                word.push(c);
                Before::Word(word)
            }
            _ if is_ident_char(c) => Before::Word(c.to_string()),
            _ => Before::Operator { char: c, after_equals: previous == Some('=') },
        }
    }
}

fn regex_literal_end(chars: &[char], slash: usize) -> Option<usize> {
    let mut at = slash + 1;
    let mut in_class = false;
    while let Some(&c) = chars.get(at) {
        match c {
            '\n' => return None,
            '\\' => at += 1,
            '[' => in_class = true,
            ']' => in_class = false,
            '/' if !in_class => {
                at += 1;
                while chars.get(at).is_some_and(|c| c.is_ascii_alphabetic()) {
                    at += 1;
                }
                return Some(at);
            }
            _ => {}
        }
        at += 1;
    }
    None
}

fn code_only_lines(source: &str) -> Result<Vec<String>, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut lines = vec![String::new()];
    let mut modes = vec![(Mode::Code { open_braces_in_template: None }, 1)];
    let mut before = Before::LineStart;
    let mut previous: Option<char> = None;
    let mut at = 0;
    while at < chars.len() {
        let c = chars[at];
        let next = chars.get(at + 1).copied();
        at += 1;
        if c == '\n' {
            lines.push(String::new());
            if matches!(modes.last(), Some((Mode::Quoted(_), _))) {
                modes.pop();
            }
            if matches!(modes.last(), Some((Mode::Code { .. }, _))) {
                before = Before::LineStart;
            }
            previous = None;
            continue;
        }
        let line = lines.len();
        let code = lines.last_mut().expect("one line at least");
        let (mode, _) = modes.last_mut().expect("the code mode at the bottom");
        match mode {
            Mode::BlockComment => {
                if c == '*' && next == Some('/') {
                    at += 1;
                    modes.pop();
                }
            }
            Mode::Quoted(q) => {
                if c == '\\' {
                    at += usize::from(next.is_some_and(|n| n != '\n'));
                } else if c == *q {
                    code.push(c);
                    modes.pop();
                    before = Before::Operand;
                }
            }
            Mode::Template => match c {
                '\\' => at += usize::from(next.is_some_and(|n| n != '\n')),
                '`' => {
                    code.push(c);
                    modes.pop();
                    before = Before::Operand;
                }
                '$' if next == Some('{') => {
                    at += 1;
                    code.push(' ');
                    modes.push((Mode::Code { open_braces_in_template: Some(0) }, line));
                    before = Before::Operator { char: '{', after_equals: false };
                }
                _ => {}
            },
            Mode::Code { open_braces_in_template } => match c {
                '/' if next == Some('/') => {
                    while chars.get(at).is_some_and(|n| *n != '\n') {
                        at += 1;
                    }
                }
                '/' if next == Some('*') => {
                    at += 1;
                    modes.push((Mode::BlockComment, line));
                }
                '/' if before.expects_an_operand() && regex_literal_end(&chars, at - 1).is_some() => {
                    at = regex_literal_end(&chars, at - 1).expect("checked");
                    code.push(' ');
                    before = Before::Operand;
                }
                '\'' | '"' => {
                    code.push(c);
                    modes.push((Mode::Quoted(c), line));
                }
                '`' => {
                    code.push(c);
                    modes.push((Mode::Template, line));
                }
                '}' if *open_braces_in_template == Some(0) => {
                    code.push(' ');
                    modes.pop();
                }
                _ => {
                    match (c, open_braces_in_template.as_mut()) {
                        ('{', Some(depth)) => *depth += 1,
                        ('}', Some(depth)) => *depth -= 1,
                        _ => {}
                    }
                    code.push(c);
                    if !c.is_whitespace() {
                        before = std::mem::replace(&mut before, Before::LineStart).then(c, previous);
                    }
                    previous = Some(c);
                    continue;
                }
            },
        }
        previous = None;
    }
    match &modes[..] {
        [(Mode::Code { open_braces_in_template: None }, _)] => Ok(lines),
        [.., (mode, line)] => Err(format!("line {line}: a {} is not closed at the end of the file", mode.name())),
        [] => unreachable!("the code mode at the bottom is never popped"),
    }
}

#[cfg(test)]
mod tests {
    use super::find;

    fn texts(source: &str) -> Vec<String> {
        find(source).expect("a closed source").into_iter().map(|f| f.text).collect()
    }

    #[test]
    fn reads_and_non_reads() {
        assert_eq!(texts("const now = new Date().toISOString();\n"), ["new Date()"]);
        assert_eq!(texts("const t = performance.now(); x = Date.now();\n"), ["Date.now()", "performance.now()"]);
        assert!(texts("const d = new Date(t);\nconst e = new Date(Date.UTC(y, m, d));\n").is_empty());
        assert!(texts("// Date.now()\n/* new Date()\n performance.now() */ const s = \"Date.now()\";\n").is_empty());
        assert_eq!(texts("if (st.mtimeMs > 0) {}\n"), [".mtimeMs"]);
        assert!(texts("const myDate = toDate(x); let d: Date = y; z = x instanceof Date;\n").is_empty());
    }

    #[test]
    fn templates_values_and_bare_forms() {
        assert_eq!(texts("const a = `stamp-${Date.now()}`;\n"), ["Date.now()"]);
        assert!(texts("const a = `Date.now() ${x}`;\n").is_empty());
        assert_eq!(texts("const a = `${ {k: new Date()}.k }`;\n"), ["new Date()"]);
        assert_eq!(texts("const b = Date();\n"), ["Date()"]);
        assert_eq!(texts("const c = new Date;\n"), ["new Date()"]);
        assert_eq!(texts("const d = Date.now;\n"), ["Date.now"]);
        assert_eq!(texts("const e = new globalThis.Date();\n"), ["new Date()"]);
        assert_eq!(texts("const f = window.Date.now();\n"), ["Date.now()"]);
        assert_eq!(texts("const g = `a\n${process.hrtime.bigint()}\n`;\n"), ["process.hrtime"]);
    }

    #[test]
    fn a_regular_expression_literal_is_not_code() {
        assert_eq!(texts("const FENCE = /^ {0,3}(`{3,}|~{3,})/;\nconst t = Date.now();\n"), ["Date.now()"]);
        assert_eq!(texts("if (/^[\\[\\]{}#&*!|>%@`,?]/.test(s)) return null;\nx = Date.now();\n"), ["Date.now()"]);
        assert_eq!(texts("const esc = (s) => s.replace(/`/g, \"\\\\`\");\nexport const t = new Date();\n"), ["new Date()"]);
        assert_eq!(texts("const r = /'/.test(s); const d = Date.now();\n"), ["Date.now()"]);
        assert!(texts("const r = /Date.now()/;\nconst q = /[/]new Date()/i;\n").is_empty());
        assert_eq!(texts("function f(x) { return /`/.test(x) && Date.now(); }\n"), ["Date.now()"]);
        assert_eq!(texts("switch (k) { case /`/.source: t = Date.now(); }\n"), ["Date.now()"]);
        assert_eq!(texts("const f = (s) => /`/.test(s);\nconst g = Date.now();\n"), ["Date.now()"]);
    }

    #[test]
    fn now_destructured_from_date_is_the_value_date_now() {
        for source in [
            "const { now } = Date;\n",
            "const { now: n } = Date;\n",
            "let { parse, now } = globalThis.Date;\n",
            "var { now = f } = Date\n",
            "({ now } = Date);\n",
        ] {
            assert_eq!(texts(source), ["Date.now"], "{source:?}");
        }
        assert_eq!(find("const {\n  now,\n} = Date;\n").expect("closed").into_iter().map(|f| (f.line, f.text)).collect::<Vec<_>>(), [(3, "Date.now".to_string())]);
        for source in [
            "const { now } = clock;\n",
            "const { parse } = Date;\n",
            "const { now } = myDate;\n",
            "const { a: { now } } = Date;\n",
            "if ({ now } == Date) {}\n",
            "const s = \"const { now } = Date\";\n",
            "// const { now } = Date;\n",
            "const { now } = Date.prototype;\n",
        ] {
            assert!(texts(source).is_empty(), "{source:?}");
        }
    }

    #[test]
    fn a_slash_after_an_operand_is_division() {
        assert_eq!(texts("const h = total / 2; const t = Date.now() / 1000;\n"), ["Date.now()"]);
        assert_eq!(texts("const a = (x) / 2 / `${Date.now()}`;\n"), ["Date.now()"]);
        assert_eq!(texts("const b = n / m; // Date.now()\nconst c = new Date;\n"), ["new Date()"]);
    }

    #[test]
    fn a_comment_or_literal_open_at_the_end_is_an_input_error() {
        for (source, what) in [
            ("const a = 1;\n/* Date.now()\n", "line 2: a block comment"),
            ("const a = `x\n${b}\n", "line 1: a template literal"),
            ("const a = `${ f(\n", "line 1: a `${…}` of a template literal"),
            ("const s = 'abc", "line 1: a string literal"),
        ] {
            let error = find(source).expect_err(source);
            assert!(error.starts_with(what), "{source:?}: {error}");
        }
    }
}
