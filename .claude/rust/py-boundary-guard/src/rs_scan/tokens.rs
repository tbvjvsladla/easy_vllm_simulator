//! The normalized call text: the allowlist key of a finding, independent of layout and comments.

use proc_macro2::{Delimiter, TokenStream, TokenTree};

/// The tokens joined without spaces, except one space between two words (identifiers, keywords, literals), and
/// without a trailing comma before a closing delimiter, so the same call reads the same however it is wrapped or
/// indented (rustfmt adds the comma when it breaks a list over lines). Literals keep their source spelling (`r#"…"#`);
/// a tab, line feed or carriage return inside one is written as `\t`, `\n`, `\r` so that the text fits one TSV cell.
pub fn compact(tokens: TokenStream) -> String {
    let mut out = String::new();
    let mut after_word = false;
    push_compact(tokens, &mut out, &mut after_word);
    out.replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn push_compact(tokens: TokenStream, out: &mut String, after_word: &mut bool) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::None => ("", ""),
                };
                if !open.is_empty() {
                    out.push_str(open);
                    *after_word = false;
                }
                push_compact(group.stream(), out, after_word);
                if !close.is_empty() {
                    if out.ends_with(',') {
                        out.pop();
                    }
                    out.push_str(close);
                    *after_word = false;
                }
            }
            TokenTree::Ident(ident) => push_word(&ident.to_string(), out, after_word),
            TokenTree::Literal(literal) => push_word(&literal.to_string(), out, after_word),
            TokenTree::Punct(punct) => {
                out.push(punct.as_char());
                *after_word = false;
            }
        }
    }
}

fn push_word(word: &str, out: &mut String, after_word: &mut bool) {
    if *after_word {
        out.push(' ');
    }
    out.push_str(word);
    *after_word = true;
}

#[cfg(test)]
mod tests {
    use super::compact;

    fn text(source: &str) -> String {
        compact(source.parse().expect("tokens"))
    }

    #[test]
    fn layout_does_not_change_the_text() {
        assert_eq!(
            text("Command::new(\"python3\")"),
            "Command::new(\"python3\")"
        );
        assert_eq!(
            text("std :: process :: Command\n    ::new( \"python3\" )"),
            "std::process::Command::new(\"python3\")"
        );
        assert_eq!(
            text("Command::new(venv.join(\"bin/python\"))"),
            "Command::new(venv.join(\"bin/python\"))"
        );
        assert_eq!(text("[&py, \"-m\", \"pip\"]"), "[&py,\"-m\",\"pip\"]");
        assert_eq!(text("x as u8"), "x as u8");
        assert_eq!(
            text("Command::new(r#\"python3\"#)"),
            "Command::new(r#\"python3\"#)"
        );
        assert_eq!(text("f(\"a\tb\")"), "f(\"a\\tb\")");
        assert_eq!(
            text("[\n    \"python3\",\n    \"-m\",\n]"),
            "[\"python3\",\"-m\"]"
        );
    }
}
