//! One TypeScript source → its clock reads: `Date.now()`, `new Date()` with no argument, `performance.now()`,
//! `process.hrtime`, and the file-time properties `.mtime`/`.atime`/`.ctime`/`.birthtime` (and their `…Ms`).
//! `//` and `/* … */` comments are skipped; text inside quotes is not code.

use super::Finding;

const CALLS: [(&str, &str); 3] = [
    ("Date.now(", "Date.now()"),
    ("performance.now(", "performance.now()"),
    ("process.hrtime", "process.hrtime"),
];
const FILE_TIME_PROPERTIES: [&str; 8] = [
    ".mtime", ".atime", ".ctime", ".birthtime", ".mtimeMs", ".atimeMs", ".ctimeMs", ".birthtimeMs",
];

pub fn find(source: &str) -> Vec<Finding> {
    let mut found = Vec::new();
    let mut in_block_comment = false;
    for (index, raw) in source.lines().enumerate() {
        let code = code_of(raw, &mut in_block_comment);
        let line = index + 1;
        for (needle, text) in CALLS {
            for _ in code.match_indices(needle) {
                found.push(Finding { line, text: text.to_string() });
            }
        }
        for (at, _) in code.match_indices("new Date") {
            let rest = code[at + "new Date".len()..].trim_start();
            if rest.strip_prefix('(').is_some_and(|args| args.trim_start().starts_with(')')) {
                found.push(Finding { line, text: "new Date()".to_string() });
            }
        }
        for property in FILE_TIME_PROPERTIES {
            for (at, _) in code.match_indices(property) {
                let next = code[at + property.len()..].chars().next();
                if next.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_')) {
                    found.push(Finding { line, text: property.to_string() });
                }
            }
        }
    }
    found
}

fn code_of(line: &str, in_block_comment: &mut bool) -> String {
    let mut code = String::new();
    let mut quote: Option<char> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        if *in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                chars.next();
                *in_block_comment = false;
            }
            continue;
        }
        if let Some(q) = quote {
            if c == '\\' {
                chars.next();
            } else if c == q {
                quote = None;
                code.push(c);
            }
            continue;
        }
        match c {
            '/' if chars.peek() == Some(&'/') => break,
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                *in_block_comment = true;
            }
            '\'' | '"' | '`' => {
                quote = Some(c);
                code.push(c);
            }
            _ => code.push(c),
        }
    }
    code
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
    }
}
