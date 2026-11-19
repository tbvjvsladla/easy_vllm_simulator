//! One Rust source file → its clock reads, read token by token (proc-macro2), so comments never count and macro
//! bodies always do.
//!
//! A read is
//! - a path call `<Type>::now()` (`SystemTime::now()`, `Instant::now()`, an alias, a wrapper type);
//! - a zero-argument `.elapsed()`, `.modified()`, `.accessed()` or `.created()`;
//! - a foreign `fn time`/`clock_gettime`/`gettimeofday`/`ftime`/`timespec_get` declared in an `extern` block, or a
//!   `libc::` call of one;
//! - `Command::new("date")` (or an alias of `Command`);
//! - a calendar period: a `const`, `static` or `let` named `…_DAYS`/`…_WEEKS`/`…_MONTHS`, a `.add_days(…)` or
//!   `.days_since(…)` call;
//! - a shell clock read inside a string literal (`shell_reads`); doc attributes (`///`, `//!`) are skipped.

use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};

use super::{is_period_name, shell_reads, Finding};

const ZERO_ARGUMENT_CLOCK_METHODS: [&str; 4] = ["elapsed", "modified", "accessed", "created"];
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
    };
    walker.walk(&tokens, false);
    walker.found.sort_by(|a, b| (a.line, &a.text).cmp(&(b.line, &b.text)));
    Ok(walker.found)
}

struct Walker {
    command_names: Vec<String>,
    found: Vec<Finding>,
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
        match ident(tokens.get(end - 3)) {
            Some(segment) => {
                segments.push(segment);
                end -= 3;
            }
            None => break,
        }
    }
    segments.reverse();
    segments.join("::")
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
        if name == "now" && after_path && is_empty_call(tokens.get(at + 1)) {
            self.push(here, format!("{}()", path_ending_at(tokens, at)));
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
            self.command_new_date(tokens, at, here);
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

    fn command_new_date(&mut self, tokens: &[TokenTree], at: usize, here: usize) {
        if !(is_path_separator(tokens, at + 2) && ident(tokens.get(at + 3)).as_deref() == Some("new")) {
            return;
        }
        let Some(args) = group(tokens.get(at + 4), Delimiter::Parenthesis) else { return };
        let args: Vec<TokenTree> = args.into_iter().collect();
        let [TokenTree::Literal(program)] = &args[..] else { return };
        if literal_text(program).is_some_and(|p| p.rsplit('/').next() == Some("date")) {
            self.push(here, format!("{}::new(\"date\")", tokens[at]));
        }
    }

    fn literal(&mut self, literal: &proc_macro2::Literal) {
        let Some(text) = literal_text(literal) else { return };
        let span = literal.span();
        let start = span.start().line;
        let spans_lines = span.end().line > start;
        for (offset, source_line) in text.lines().enumerate() {
            for read in shell_reads::line_reads(source_line) {
                self.push(if spans_lines { start + offset } else { start }, read);
            }
        }
    }
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
