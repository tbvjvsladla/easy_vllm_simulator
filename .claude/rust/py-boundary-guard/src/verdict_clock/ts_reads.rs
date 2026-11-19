//! One TypeScript source → its clock reads:
//! - `Date.now()` and `performance.now()`, and the same functions taken as values (`Date.now`, `performance.now`);
//! - `new Date()` with no argument, also written `new Date`;
//! - `Date(…)` called without `new` (the current time as a string, whatever the arguments);
//! - `process.hrtime`;
//! - the file-time properties `.mtime`/`.atime`/`.ctime`/`.birthtime` (and their `…Ms`).
//!
//! `globalThis.Date` and `window.Date` are `Date`. `//` and `/* … */` comments are skipped; text inside quotes is not
//! code, but a `${…}` inside a template literal is.

use super::Finding;

const CLOCK_FUNCTIONS: [&str; 2] = ["Date.now", "performance.now"];
const GLOBAL_OBJECTS: [&str; 2] = ["globalThis.", "window."];
const FILE_TIME_PROPERTIES: [&str; 8] = [
    ".mtime", ".atime", ".ctime", ".birthtime", ".mtimeMs", ".atimeMs", ".ctimeMs", ".birthtimeMs",
];

pub fn find(source: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    for (index, code) in code_only_lines(source).iter().enumerate() {
        let line = index + 1;
        let code = without_global_objects(code);
        for text in line_reads(&code) {
            found.push(Finding { line, text });
        }
    }
    found
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

fn code_only_lines(source: &str) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut modes = vec![Mode::Code { open_braces_in_template: None }];
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\n' {
            lines.push(String::new());
            if matches!(modes.last(), Some(Mode::Quoted(_))) {
                modes.pop();
            }
            continue;
        }
        let code = lines.last_mut().expect("one line at least");
        match modes.last_mut().expect("the code mode at the bottom") {
            Mode::BlockComment => {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    modes.pop();
                }
            }
            Mode::Quoted(q) => {
                if c == '\\' {
                    chars.next_if(|n| *n != '\n');
                } else if c == *q {
                    code.push(c);
                    modes.pop();
                }
            }
            Mode::Template => match c {
                '\\' => {
                    chars.next_if(|n| *n != '\n');
                }
                '`' => {
                    code.push(c);
                    modes.pop();
                }
                '$' if chars.peek() == Some(&'{') => {
                    chars.next();
                    code.push(' ');
                    modes.push(Mode::Code { open_braces_in_template: Some(0) });
                }
                _ => {}
            },
            Mode::Code { open_braces_in_template } => match c {
                '/' if chars.peek() == Some(&'/') => {
                    while chars.next_if(|n| *n != '\n').is_some() {}
                }
                '/' if chars.peek() == Some(&'*') => {
                    chars.next();
                    modes.push(Mode::BlockComment);
                }
                '\'' | '"' => {
                    code.push(c);
                    modes.push(Mode::Quoted(c));
                }
                '`' => {
                    code.push(c);
                    modes.push(Mode::Template);
                }
                '{' => {
                    if let Some(depth) = open_braces_in_template {
                        *depth += 1;
                    }
                    code.push(c);
                }
                '}' if *open_braces_in_template == Some(0) => {
                    code.push(' ');
                    modes.pop();
                }
                '}' => {
                    if let Some(depth) = open_braces_in_template {
                        *depth -= 1;
                    }
                    code.push(c);
                }
                _ => code.push(c),
            },
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::find;

    fn texts(source: &str) -> Vec<String> {
        find(source).into_iter().map(|f| f.text).collect()
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
}
