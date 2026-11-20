//! SPDX license expressions (the `license` field of a crate manifest) judged against the permitted set.
//!
//! Grammar: `OR` binds looser than `AND`; `X WITH E` names an exception to `X`; parentheses group; the legacy
//! separator `/` (`MIT/Apache-2.0`, older manifests) is `OR`. An expression is permitted when the licensee can
//! choose terms that use only permitted licenses: any branch of an `OR`, every operand of an `AND`.

/// Licenses a crate of the harness may be used under (plan 26111805 D2·D9, proposed by cell c20). Changing this set
/// is a change of the dependency policy: it needs the same human approval as the crate allowlist.
pub const PERMITTED_LICENSES: &[&str] = &[
    "0BSD",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CDLA-Permissive-2.0",
    "ISC",
    "MIT",
    "Unicode-3.0",
    "Unlicense",
    "Zlib",
];

/// Exceptions permitted after `WITH`, on a permitted license.
pub const PERMITTED_EXCEPTIONS: &[&str] = &["LLVM-exception"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Token<'a> {
    Open,
    Close,
    And,
    Or,
    With,
    Id(&'a str),
}

fn word(text: &str) -> Token<'_> {
    match text {
        "AND" => Token::And,
        "OR" => Token::Or,
        "WITH" => Token::With,
        id => Token::Id(id),
    }
}

fn tokenize(expr: &str) -> Vec<Token<'_>> {
    let mut tokens = Vec::new();
    let mut word_start: Option<usize> = None;
    for (i, c) in expr.char_indices() {
        let single = match c {
            '(' => Some(Token::Open),
            ')' => Some(Token::Close),
            '/' => Some(Token::Or),
            _ => None,
        };
        if c.is_whitespace() || single.is_some() {
            if let Some(start) = word_start.take() {
                tokens.push(word(&expr[start..i]));
            }
            tokens.extend(single);
        } else if word_start.is_none() {
            word_start = Some(i);
        }
    }
    if let Some(start) = word_start {
        tokens.push(word(&expr[start..]));
    }
    tokens
}

/// Recursive-descent evaluation over the token list; `pos` is the next unread token.
struct Evaluator<'a> {
    tokens: Vec<Token<'a>>,
    pos: usize,
}

impl<'a> Evaluator<'a> {
    fn peek(&self) -> Option<Token<'a>> {
        self.tokens.get(self.pos).copied()
    }

    fn any_of(&mut self) -> Result<bool, String> {
        let mut permitted = self.all_of()?;
        while self.peek() == Some(Token::Or) {
            self.pos += 1;
            permitted |= self.all_of()?;
        }
        Ok(permitted)
    }

    fn all_of(&mut self) -> Result<bool, String> {
        let mut permitted = self.term()?;
        while self.peek() == Some(Token::And) {
            self.pos += 1;
            permitted &= self.term()?;
        }
        Ok(permitted)
    }

    fn term(&mut self) -> Result<bool, String> {
        match self.peek() {
            Some(Token::Open) => {
                self.pos += 1;
                let permitted = self.any_of()?;
                if self.peek() != Some(Token::Close) {
                    return Err("missing ')'".to_string());
                }
                self.pos += 1;
                Ok(permitted)
            }
            Some(Token::Id(license)) => {
                self.pos += 1;
                let mut permitted = PERMITTED_LICENSES.contains(&license);
                if self.peek() == Some(Token::With) {
                    self.pos += 1;
                    match self.peek() {
                        Some(Token::Id(exception)) => {
                            self.pos += 1;
                            permitted &= PERMITTED_EXCEPTIONS.contains(&exception);
                        }
                        _ => return Err(format!("'{license} WITH' names no exception")),
                    }
                }
                Ok(permitted)
            }
            Some(other) => Err(format!("unexpected {other:?}")),
            None => Err("expression ends early".to_string()),
        }
    }
}

/// `Ok(true)` when the expression can be satisfied with permitted licenses only, `Ok(false)` when it cannot, and
/// `Err` when it is not a well-formed expression (an empty field included).
pub fn is_permitted(expr: &str) -> Result<bool, String> {
    let mut eval = Evaluator {
        tokens: tokenize(expr),
        pos: 0,
    };
    let permitted = eval.any_of()?;
    match eval.peek() {
        None => Ok(permitted),
        Some(extra) => Err(format!("unexpected {extra:?} after a complete expression")),
    }
}

#[cfg(test)]
mod tests {
    use super::is_permitted;

    #[test]
    fn manifest_expressions_of_the_locked_crates() {
        for expr in [
            "MIT",
            "MIT OR Apache-2.0",
            "MIT/Apache-2.0",
            "Unlicense/MIT",
            "(MIT OR Apache-2.0) AND Unicode-3.0",
            "Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT",
            "Apache-2.0 AND ISC",
            "0BSD OR MIT OR Apache-2.0",
            "CDLA-Permissive-2.0",
            "(Apache-2.0 OR MIT) AND BSD-3-Clause",
        ] {
            assert_eq!(is_permitted(expr), Ok(true), "{expr}");
        }
    }

    #[test]
    fn a_copyleft_term_is_permitted_only_as_an_alternative() {
        assert_eq!(is_permitted("GPL-3.0-only"), Ok(false));
        assert_eq!(is_permitted("MIT AND GPL-3.0-only"), Ok(false));
        assert_eq!(is_permitted("GPL-3.0-only OR MIT"), Ok(true));
        assert_eq!(is_permitted("(MPL-2.0 OR GPL-2.0-only) AND MIT"), Ok(false));
    }

    #[test]
    fn an_exception_counts_only_on_a_permitted_license() {
        assert_eq!(is_permitted("Apache-2.0 WITH LLVM-exception"), Ok(true));
        assert_eq!(
            is_permitted("Apache-2.0 WITH Classpath-exception-2.0"),
            Ok(false)
        );
        assert_eq!(is_permitted("GPL-2.0-only WITH LLVM-exception"), Ok(false));
    }

    #[test]
    fn malformed_expressions_are_errors() {
        for expr in [
            "",
            "MIT OR",
            "(MIT",
            "MIT)",
            "AND MIT",
            "MIT Apache-2.0",
            "Apache-2.0 WITH",
        ] {
            assert!(is_permitted(expr).is_err(), "{expr:?}");
        }
    }
}
