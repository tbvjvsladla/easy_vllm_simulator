//! One Rust source file → its clock reads, read token by token (proc-macro2), so comments never count and macro
//! bodies always do.
//!
//! A read is
//! - a path ending in `now`: a call `<Type>::now()` (`SystemTime::now()`, `Instant::now()`, an alias, a wrapper type,
//!   a qualified `<T>::now()` or `<T as U>::now()`) or the function taken as a value (`unwrap_or_else(Instant::now)`);
//!   a `use …::now;` item declares and does not read;
//! - a zero-argument `.elapsed()`, `.modified()`, `.accessed()`, `.created()`, or a `MetadataExt` file time
//!   `.mtime()`/`.atime()`/`.ctime()` (and their `_nsec`);
//! - a foreign `fn time`/`clock_gettime`/`gettimeofday`/`ftime`/`timespec_get` declared in an `extern` block, or a
//!   `libc::` call of one;
//! - `Command::new("date")` (or an alias of `Command`);
//! - the argv of any other `Command::new("<program>")`: the string literals of its `.arg(…)`/`.args([…])` in the same
//!   expression, read as one shell command line (`stat -c %Y`, `find -mmin`, `git --since=…`, `ssh <host> date`);
//! - a calendar period: a `const`, `static` or `let` named `…_DAYS`/`…_WEEKS`/`…_MONTHS`, a `.add_days(…)` or
//!   `.days_since(…)` call;
//! - a shell clock read inside a string literal (`shell_reads`); doc attributes (`///`, `//!`) are skipped.

use std::collections::{BTreeMap, HashSet};

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};

use super::{is_period_name, shell_reads, Finding};

const ZERO_ARGUMENT_CLOCK_METHODS: [&str; 10] = [
    "elapsed", "modified", "accessed", "created", "mtime", "mtime_nsec", "atime", "atime_nsec", "ctime", "ctime_nsec",
];
const FOREIGN_CLOCKS: [&str; 5] = ["time", "clock_gettime", "gettimeofday", "ftime", "timespec_get"];
const PERIOD_METHODS: [&str; 2] = ["add_days", "days_since"];

pub fn find(source: &str) -> Result<Vec<Finding>, String> {
    let tokens: TokenStream = source
        .parse()
        .map_err(|e: proc_macro2::LexError| format!("line {}: {e}", e.span().start().line))?;
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    let mut walker = Walker {
        command_names: command_aliases(&tokens),
        found: Vec::new(),
        argv_literal_starts: HashSet::new(),
    };
    walker.walk(&tokens, false);
    walker.found.sort_by(|a, b| (a.line, &a.text).cmp(&(b.line, &b.text)));
    Ok(walker.found)
}

struct Walker {
    command_names: Vec<String>,
    found: Vec<Finding>,
    argv_literal_starts: HashSet<(usize, usize)>,
}

enum ArgvWord {
    OneLineLiteral(proc_macro2::Literal, String),
    Opaque,
}

fn ident(token: Option<&TokenTree>) -> Option<String> {
    match token {
        Some(TokenTree::Ident(ident)) => Some(ident.to_string()),
        _ => None,
    }
}

fn is_punct(token: Option<&TokenTree>, c: char) -> bool {
    matches!(token, Some(TokenTree::Punct(p)) if p.as_char() == c)
}

fn is_path_separator(tokens: &[TokenTree], at: usize) -> bool {
    at >= 1
        && matches!(tokens.get(at - 1), Some(TokenTree::Punct(p)) if p.as_char() == ':' && p.spacing() == Spacing::Joint)
        && is_punct(tokens.get(at), ':')
}

fn group(token: Option<&TokenTree>, delimiter: Delimiter) -> Option<TokenStream> {
    match token {
        Some(TokenTree::Group(g)) if g.delimiter() == delimiter => Some(g.stream()),
        _ => None,
    }
}

fn is_empty_call(token: Option<&TokenTree>) -> bool {
    group(token, Delimiter::Parenthesis).is_some_and(|args| args.is_empty())
}

fn line(token: &TokenTree) -> usize {
    token.span().start().line
}

fn command_aliases(tokens: &[TokenTree]) -> Vec<String> {
    let mut names = vec!["Command".to_string()];
    collect_aliases(tokens, &mut names);
    names
}

fn collect_aliases(tokens: &[TokenTree], names: &mut Vec<String>) {
    for (at, token) in tokens.iter().enumerate() {
        if let TokenTree::Group(g) = token {
            collect_aliases(&g.stream().into_iter().collect::<Vec<_>>(), names);
        }
        if ident(Some(token)).as_deref() == Some("Command") && ident(tokens.get(at + 1)).as_deref() == Some("as") {
            if let Some(alias) = ident(tokens.get(at + 2)) {
                names.push(alias);
            }
        }
    }
}

fn doc_attribute_len(tokens: &[TokenTree], at: usize) -> usize {
    if !is_punct(tokens.get(at), '#') {
        return 0;
    }
    let bang = usize::from(is_punct(tokens.get(at + 1), '!'));
    let names_doc = group(tokens.get(at + 1 + bang), Delimiter::Bracket)
        .is_some_and(|body| ident(body.into_iter().next().as_ref()).as_deref() == Some("doc"));
    if names_doc {
        2 + bang
    } else {
        0
    }
}

fn path_ending_at(tokens: &[TokenTree], at: usize) -> String {
    let mut segments = vec![tokens[at].to_string()];
    let mut end = at;
    while end >= 3 && is_path_separator(tokens, end - 1) {
        if let Some(segment) = ident(tokens.get(end - 3)) {
            segments.push(segment);
            end -= 3;
        } else {
            if let Some(qualifier) = qualified_self_type_ending_at(tokens, end - 3) {
                segments.push(qualifier);
            }
            break;
        }
    }
    segments.reverse();
    segments.join("::")
}

fn qualified_self_type_ending_at(tokens: &[TokenTree], close: usize) -> Option<String> {
    if !is_punct(tokens.get(close), '>') {
        return None;
    }
    let mut depth = 0usize;
    for open in (0..=close).rev() {
        match &tokens[open] {
            TokenTree::Punct(p) if p.as_char() == '>' => depth += 1,
            TokenTree::Punct(p) if p.as_char() == '<' => {
                depth -= 1;
                if depth == 0 {
                    return Some(render(&tokens[open..=close]));
                }
            }
            _ => {}
        }
    }
    None
}

fn render(tokens: &[TokenTree]) -> String {
    let mut text = String::new();
    let mut after_word = false;
    for token in tokens {
        let word = matches!(token, TokenTree::Ident(_) | TokenTree::Literal(_));
        if word && after_word {
            text.push(' ');
        }
        text.push_str(&token.to_string());
        after_word = word;
    }
    text
}

fn ends_path_in_now(tokens: &[TokenTree], at: usize) -> bool {
    let after_segment = ident(tokens.get(at.wrapping_sub(3))).is_some() || is_punct(tokens.get(at.wrapping_sub(3)), '>');
    let continues = is_punct(tokens.get(at + 1), ':') && is_punct(tokens.get(at + 2), ':');
    at >= 3 && is_path_separator(tokens, at - 1) && after_segment && !continues
}

fn split_arguments(stream: TokenStream) -> Vec<Vec<TokenTree>> {
    let mut arguments = vec![Vec::new()];
    for token in stream {
        if is_punct(Some(&token), ',') {
            arguments.push(Vec::new());
        } else {
            arguments.last_mut().expect("one argument at least").push(token);
        }
    }
    arguments.retain(|argument| !argument.is_empty());
    arguments
}

fn argv_word(tokens: &[TokenTree]) -> ArgvWord {
    match tokens {
        [TokenTree::Literal(literal)] => match literal_text(literal) {
            Some(text) if !text.contains('\n') => ArgvWord::OneLineLiteral(literal.clone(), text),
            _ => ArgvWord::Opaque,
        },
        _ => ArgvWord::Opaque,
    }
}

fn argv_of_method_chain(tokens: &[TokenTree], mut from: usize) -> Vec<ArgvWord> {
    let mut arguments = Vec::new();
    loop {
        if is_punct(tokens.get(from), '?') {
            from += 1;
            continue;
        }
        let (true, Some(method), Some(call)) = (
            is_punct(tokens.get(from), '.'),
            ident(tokens.get(from + 1)),
            group(tokens.get(from + 2), Delimiter::Parenthesis),
        ) else {
            return arguments;
        };
        match method.as_str() {
            "arg" => arguments.push(argv_word(&call.into_iter().collect::<Vec<_>>())),
            "args" => {
                let inner: Vec<TokenTree> = call.into_iter().filter(|t| !is_punct(Some(t), '&')).collect();
                let list = match &inner[..] {
                    [TokenTree::Group(list)] if list.delimiter() == Delimiter::Bracket => Some(list.stream()),
                    [TokenTree::Ident(vec), bang, TokenTree::Group(list)] if vec == "vec" && is_punct(Some(bang), '!') => {
                        Some(list.stream())
                    }
                    _ => None,
                };
                match list {
                    Some(list) => arguments.extend(split_arguments(list).iter().map(|a| argv_word(a))),
                    None => arguments.push(ArgvWord::Opaque),
                }
            }
            _ => {}
        }
        from += 3;
    }
}

impl Walker {
    fn push(&mut self, line: usize, text: String) {
        self.found.push(Finding { line, text });
    }

    fn walk(&mut self, tokens: &[TokenTree], in_extern: bool) {
        let mut at = 0;
        while at < tokens.len() {
            let skip = doc_attribute_len(tokens, at);
            if skip > 0 {
                at += skip;
                continue;
            }
            if ident(tokens.get(at)).as_deref() == Some("use") {
                while at < tokens.len() && !is_punct(tokens.get(at), ';') {
                    at += 1;
                }
                continue;
            }
            match &tokens[at] {
                TokenTree::Group(g) => {
                    let inner: Vec<TokenTree> = g.stream().into_iter().collect();
                    let extern_block = g.delimiter() == Delimiter::Brace && self.opens_extern_block(tokens, at);
                    self.walk(&inner, extern_block);
                }
                TokenTree::Literal(literal) => self.literal(literal),
                TokenTree::Ident(name) => self.ident(tokens, at, &name.to_string(), in_extern),
                TokenTree::Punct(_) => {}
            }
            at += 1;
        }
    }

    fn opens_extern_block(&self, tokens: &[TokenTree], at: usize) -> bool {
        match at.checked_sub(1).map(|i| &tokens[i]) {
            Some(TokenTree::Ident(i)) => i == "extern",
            Some(TokenTree::Literal(_)) => at >= 2 && ident(tokens.get(at - 2)).as_deref() == Some("extern"),
            _ => false,
        }
    }

    fn ident(&mut self, tokens: &[TokenTree], at: usize, name: &str, in_extern: bool) {
        let here = line(&tokens[at]);
        let after_dot = at >= 1 && is_punct(tokens.get(at - 1), '.');
        let after_path = at >= 3 && is_path_separator(tokens, at - 1) && ident(tokens.get(at - 3)).is_some();
        if name == "now" && ends_path_in_now(tokens, at) {
            let path = path_ending_at(tokens, at);
            let text = match group(tokens.get(at + 1), Delimiter::Parenthesis) {
                Some(args) if args.is_empty() => format!("{path}()"),
                Some(_) => format!("{path}(…)"),
                None => path,
            };
            self.push(here, text);
        } else if after_dot
            && ((ZERO_ARGUMENT_CLOCK_METHODS.contains(&name) && is_empty_call(tokens.get(at + 1)))
                || (PERIOD_METHODS.contains(&name) && group(tokens.get(at + 1), Delimiter::Parenthesis).is_some()))
        {
            self.push(here, format!(".{name}()"));
        } else if FOREIGN_CLOCKS.contains(&name) && after_path && ident(tokens.get(at - 3)).as_deref() == Some("libc") {
            self.push(here, format!("libc::{name}"));
        } else if in_extern && name == "fn" {
            if let Some(foreign) = ident(tokens.get(at + 1)).filter(|f| FOREIGN_CLOCKS.contains(&f.as_str())) {
                self.push(here, format!("extern fn {foreign}"));
            }
        } else if self.command_names.iter().any(|c| c == name) {
            self.command(tokens, at, here);
        } else if matches!(name, "const" | "static" | "let") {
            let mut next = at + 1;
            if ident(tokens.get(next)).as_deref() == Some("mut") {
                next += 1;
            }
            if let Some(binding) = ident(tokens.get(next)).filter(|b| is_period_name(b)) {
                if is_punct(tokens.get(next + 1), ':') || is_punct(tokens.get(next + 1), '=') {
                    self.push(here, format!("{name} {binding}"));
                }
            }
        }
    }

    fn command(&mut self, tokens: &[TokenTree], at: usize, here: usize) {
        if !(is_path_separator(tokens, at + 2) && ident(tokens.get(at + 3)).as_deref() == Some("new")) {
            return;
        }
        let Some(args) = group(tokens.get(at + 4), Delimiter::Parenthesis) else { return };
        let args: Vec<TokenTree> = args.into_iter().collect();
        let [TokenTree::Literal(program)] = &args[..] else { return };
        let Some(program) = literal_text(program) else { return };
        let program = program.rsplit('/').next().unwrap_or(&program).to_string();
        if program == "date" {
            self.push(here, format!("{}::new(\"date\")", tokens[at]));
            return;
        }
        let argv = argv_of_method_chain(tokens, at + 5);
        let mut found_in_single_literals: Vec<String> = Vec::new();
        for word in &argv {
            if let ArgvWord::OneLineLiteral(literal, _) = word {
                for read in self.literal_reads(literal) {
                    found_in_single_literals.push(read.text.clone());
                    self.found.push(read);
                }
                let start = literal.span().start();
                self.argv_literal_starts.insert((start.line, start.column));
            }
        }
        let words: Vec<Option<&str>> = argv
            .iter()
            .map(|word| match word {
                ArgvWord::OneLineLiteral(_, text) => Some(text.as_str()),
                ArgvWord::Opaque => None,
            })
            .collect();
        for text in missing_from(&found_in_single_literals, shell_reads::argv_reads(&program, &words)) {
            self.push(here, text);
        }
    }

    fn literal(&mut self, literal: &proc_macro2::Literal) {
        let start = literal.span().start();
        if self.argv_literal_starts.contains(&(start.line, start.column)) {
            return;
        }
        let reads = self.literal_reads(literal);
        self.found.extend(reads);
    }

    fn literal_reads(&self, literal: &proc_macro2::Literal) -> Vec<Finding> {
        let mut reads = Vec::new();
        let Some(text) = literal_text(literal) else { return reads };
        let span = literal.span();
        let start = span.start().line;
        let spans_lines = span.end().line > start;
        for (offset, source_line) in text.lines().enumerate() {
            for read in shell_reads::line_reads(source_line) {
                reads.push(Finding { line: if spans_lines { start + offset } else { start }, text: read });
            }
        }
        reads
    }
}

fn missing_from(found: &[String], wanted: Vec<String>) -> Vec<String> {
    let mut have: BTreeMap<&str, usize> = BTreeMap::new();
    for text in found {
        *have.entry(text.as_str()).or_default() += 1;
    }
    let mut missing = Vec::new();
    for text in wanted {
        match have.get_mut(text.as_str()) {
            Some(left) if *left > 0 => *left -= 1,
            _ => missing.push(text),
        }
    }
    missing
}

fn literal_text(literal: &proc_macro2::Literal) -> Option<String> {
    let stream = TokenStream::from(TokenTree::Literal(literal.clone()));
    if let Ok(text) = syn::parse2::<syn::LitStr>(stream.clone()) {
        return Some(text.value());
    }
    syn::parse2::<syn::LitByteStr>(stream)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes.value()).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(source: &str) -> Vec<String> {
        find(source).expect("tokens").into_iter().map(|f| f.text).collect()
    }

    #[test]
    fn period_names_are_whole_units() {
        for yes in ["REVIEW_PERIOD_DAYS", "max_age_days", "GRACE_WEEKS"] {
            assert!(is_period_name(yes), "{yes}");
        }
        for no in ["DAYS", "days", "_DAYS", "DAYSTAMP", "holidays", "days_left", "MONTHSX"] {
            assert!(!is_period_name(no), "{no}");
        }
    }

    #[test]
    fn mentions_are_not_reads() {
        assert!(texts("/// SystemTime::now()\n//! Instant::now()\nfn f() { /* SystemTime::now() */ }\n").is_empty());
        assert!(texts("fn now() -> u64 { 0 }\nfn g(c: &C) { c.now(); }\n").is_empty());
        assert!(texts("const S: &str = \"SystemTime::now()\";\n").is_empty());
    }

    #[test]
    fn foreign_blocks_and_libc() {
        assert_eq!(texts("extern \"C\" { fn time(t: *mut i64) -> i64; fn tzset(); }\n"), ["extern fn time"]);
        assert!(texts("fn time() {}\n").is_empty());
        assert_eq!(texts("fn f() { unsafe { libc::clock_gettime(0, p) }; }\n"), ["libc::clock_gettime"]);
    }
}
