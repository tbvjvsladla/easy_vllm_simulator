//! The dependency-specifier grammar of packaging 24.0: the context-sensitive tokenizer (`packaging/_tokenizer.py`) and the recursive-descent
//! parser (`packaging/_parser.py`, `_parse_requirement`), ported function by function.
//!
//! The parse result the requirement projection needs is the package name and the concatenated specifier text. The extras, the URL and the marker
//! are parsed (their syntax decides acceptance, and a marker's quoted strings are compiled, which may print a `SyntaxWarning`) but not kept.
//!
//! Adapted from packaging 24.0 (dual licensed Apache-2.0 / BSD-2-Clause, see `notices/`).

use super::pyre::Pattern;
use super::specifier::{OPERATOR_REGEX, VERSION_REGEX};
use super::string_literal::compile_quoted_string;
use super::{Rejection, UnsupportedDomain};
use std::sync::LazyLock;

/// The token names of `DEFAULT_RULES`.
#[derive(Clone, Copy)]
enum Rule {
    LeftParenthesis,
    RightParenthesis,
    LeftBracket,
    RightBracket,
    Semicolon,
    Comma,
    QuotedString,
    Op,
    BoolOp,
    In,
    Not,
    Variable,
    Specifier,
    At,
    Url,
    Identifier,
    VersionPrefixTrail,
    VersionLocalLabelTrail,
    Ws,
    End,
}

/// How a rule of `DEFAULT_RULES` is compiled: a plain `str` pattern has no flags; the three `re.compile(...)` objects carry their own.
struct RuleSource {
    pattern: String,
    verbose: bool,
    ignore_case: bool,
}

fn plain(pattern: &str) -> RuleSource {
    RuleSource {
        pattern: pattern.to_string(),
        verbose: false,
        ignore_case: false,
    }
}

fn verbose(pattern: &str) -> RuleSource {
    RuleSource {
        pattern: pattern.to_string(),
        verbose: true,
        ignore_case: false,
    }
}

/// `DEFAULT_RULES` in the order of [`Rule`] (patterns copied from `_tokenizer.py`).
fn default_rules() -> Vec<RuleSource> {
    vec![
        plain(r"\("),
        plain(r"\)"),
        plain(r"\["),
        plain(r"\]"),
        plain(r";"),
        plain(r","),
        verbose(
            r#"
            (
                ('[^']*')
                |
                ("[^"]*")
            )
        "#,
        ),
        plain(r"(===|==|~=|!=|<=|>=|<|>)"),
        plain(r"\b(or|and)\b"),
        plain(r"\bin\b"),
        plain(r"\bnot\b"),
        verbose(
            r"
            \b(
                python_version
                |python_full_version
                |os[._]name
                |sys[._]platform
                |platform_(release|system)
                |platform[._](version|machine|python_implementation)
                |python_implementation
                |implementation_(name|version)
                |extra
            )\b
        ",
        ),
        RuleSource {
            pattern: format!("{OPERATOR_REGEX}{VERSION_REGEX}"),
            verbose: true,
            ignore_case: true,
        },
        plain(r"\@"),
        plain(r"[^ \t]+"),
        plain(r"\b[a-zA-Z0-9][a-zA-Z0-9._-]*\b"),
        plain(r"\.\*"),
        plain(r"\+[a-z0-9]+(?:[-_\.][a-z0-9]+)*"),
        plain(r"[ \t]+"),
        plain(r"$"),
    ]
}

static COMPILED_RULES: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    default_rules()
        .iter()
        .map(|rule| {
            Pattern::compile(&rule.pattern, rule.verbose, rule.ignore_case)
                .unwrap_or_else(|error| panic!("packaging grammar rule: {error}"))
        })
        .collect()
});

/// A token: where it starts and how long its text is.
#[derive(Clone, Copy)]
struct Token {
    start: usize,
    length: usize,
}

/// `Tokenizer`: the source text and the position of the next token.
struct Tokenizer<'s> {
    source: &'s [u32],
    position: usize,
}

impl Tokenizer<'_> {
    /// `check(name, peek=True)`: the length of the token at the position, `None` when the rule does not match.
    fn check(&self, rule: Rule) -> Option<usize> {
        let found = COMPILED_RULES[rule as usize].match_at(self.source, self.position)?;
        let (start, end) = found.whole();
        Some(end - start)
    }

    /// `check(name)` followed by `read()`.
    fn read_if(&mut self, rule: Rule) -> Option<Token> {
        let length = self.check(rule)?;
        let token = Token {
            start: self.position,
            length,
        };
        self.position += length;
        Some(token)
    }

    fn consume(&mut self, rule: Rule) {
        self.read_if(rule);
    }

    /// `expect(name)`: the token, or a `ParserSyntaxError`.
    fn expect(&mut self, rule: Rule) -> Result<Token, Rejection> {
        self.read_if(rule).ok_or(Rejection::DependencySyntax)
    }

    fn text(&self, token: Token) -> &[u32] {
        &self.source[token.start..token.start + token.length]
    }
}

/// What the requirement projection keeps of a parsed requirement.
pub(super) struct ParsedRequirement {
    /// `parsed.name`: the identifier token as written.
    pub(super) name: Vec<u32>,
    /// `parsed.specifier`: the specifier tokens and commas joined without the blanks between them.
    pub(super) specifier: Vec<u32>,
}

/// The nesting of marker parentheses up to which the recursion of the original's parser is certain to stay below the interpreter's recursion
/// limit (two frames per level). A deeper marker is outside the modelled domain, not a guessed `RecursionError`.
const MAX_MARKER_NESTING: usize = 100;

/// What parsing a requirement printed to standard error, with the result.
pub(super) struct ParseOutcome {
    /// One `SyntaxWarning` line per marker string literal that has an invalid escape, in parse order.
    pub(super) warnings: Vec<String>,
    pub(super) result: Result<ParsedRequirement, Rejection>,
}

/// `parse_requirement(source)`: the parse of the whole dependency specifier, the warnings of the marker literals included.
pub(super) fn parse_requirement(source: &[u32]) -> ParseOutcome {
    let mut tokenizer = Tokenizer {
        source,
        position: 0,
    };
    let mut warnings = Vec::new();
    let result = requirement(&mut tokenizer, &mut warnings);
    ParseOutcome { warnings, result }
}

/// `requirement = WS? IDENTIFIER WS? extras WS? requirement_details`
fn requirement(
    tokenizer: &mut Tokenizer<'_>,
    warnings: &mut Vec<String>,
) -> Result<ParsedRequirement, Rejection> {
    tokenizer.consume(Rule::Ws);
    let name_token = tokenizer.expect(Rule::Identifier)?;
    let name = tokenizer.text(name_token).to_vec();
    tokenizer.consume(Rule::Ws);
    extras(tokenizer)?;
    tokenizer.consume(Rule::Ws);
    let specifier = requirement_details(tokenizer, warnings)?;
    tokenizer.expect(Rule::End)?;
    Ok(ParsedRequirement { name, specifier })
}

/// `requirement_details = AT URL (WS requirement_marker?)? | specifier WS? (requirement_marker)?`; the result is the specifier text.
fn requirement_details(
    tokenizer: &mut Tokenizer<'_>,
    warnings: &mut Vec<String>,
) -> Result<Vec<u32>, Rejection> {
    if tokenizer.read_if(Rule::At).is_some() {
        tokenizer.consume(Rule::Ws);
        tokenizer.expect(Rule::Url)?;
        if tokenizer.check(Rule::End).is_some() {
            return Ok(Vec::new());
        }
        tokenizer.expect(Rule::Ws)?;
        if tokenizer.check(Rule::End).is_some() {
            return Ok(Vec::new());
        }
        requirement_marker(tokenizer, warnings)?;
        return Ok(Vec::new());
    }
    let specifier = specifier(tokenizer)?;
    tokenizer.consume(Rule::Ws);
    if tokenizer.check(Rule::End).is_some() {
        return Ok(specifier);
    }
    requirement_marker(tokenizer, warnings)?;
    Ok(specifier)
}

/// `requirement_marker = SEMICOLON marker WS?`
fn requirement_marker(
    tokenizer: &mut Tokenizer<'_>,
    warnings: &mut Vec<String>,
) -> Result<(), Rejection> {
    if tokenizer.read_if(Rule::Semicolon).is_none() {
        return Err(Rejection::DependencySyntax);
    }
    marker(tokenizer, warnings, 0)?;
    tokenizer.consume(Rule::Ws);
    Ok(())
}

/// `extras = (LEFT_BRACKET wsp* extras_list? wsp* RIGHT_BRACKET)?`
fn extras(tokenizer: &mut Tokenizer<'_>) -> Result<(), Rejection> {
    if tokenizer.read_if(Rule::LeftBracket).is_none() {
        return Ok(());
    }
    tokenizer.consume(Rule::Ws);
    extras_list(tokenizer)?;
    tokenizer.consume(Rule::Ws);
    if tokenizer.read_if(Rule::RightBracket).is_none() {
        return Err(Rejection::DependencySyntax);
    }
    Ok(())
}

/// `extras_list = identifier (wsp* ',' wsp* identifier)*`
fn extras_list(tokenizer: &mut Tokenizer<'_>) -> Result<(), Rejection> {
    if tokenizer.read_if(Rule::Identifier).is_none() {
        return Ok(());
    }
    loop {
        tokenizer.consume(Rule::Ws);
        if tokenizer.check(Rule::Identifier).is_some() {
            return Err(Rejection::DependencySyntax);
        }
        if tokenizer.read_if(Rule::Comma).is_none() {
            return Ok(());
        }
        tokenizer.consume(Rule::Ws);
        tokenizer.expect(Rule::Identifier)?;
    }
}

/// `specifier = LEFT_PARENTHESIS WS? version_many WS? RIGHT_PARENTHESIS | WS? version_many WS?`
fn specifier(tokenizer: &mut Tokenizer<'_>) -> Result<Vec<u32>, Rejection> {
    let enclosed = tokenizer.read_if(Rule::LeftParenthesis).is_some();
    tokenizer.consume(Rule::Ws);
    let parsed = version_many(tokenizer)?;
    tokenizer.consume(Rule::Ws);
    if enclosed && tokenizer.read_if(Rule::RightParenthesis).is_none() {
        return Err(Rejection::DependencySyntax);
    }
    Ok(parsed)
}

/// `version_many = (SPECIFIER (WS? COMMA WS? SPECIFIER)*)?`
fn version_many(tokenizer: &mut Tokenizer<'_>) -> Result<Vec<u32>, Rejection> {
    let mut parsed = Vec::new();
    while let Some(token) = tokenizer.read_if(Rule::Specifier) {
        parsed.extend_from_slice(tokenizer.text(token));
        if tokenizer.check(Rule::VersionPrefixTrail).is_some()
            || tokenizer.check(Rule::VersionLocalLabelTrail).is_some()
        {
            return Err(Rejection::DependencySyntax);
        }
        tokenizer.consume(Rule::Ws);
        let Some(comma) = tokenizer.read_if(Rule::Comma) else {
            break;
        };
        parsed.extend_from_slice(tokenizer.text(comma));
        tokenizer.consume(Rule::Ws);
    }
    Ok(parsed)
}

/// `marker = marker_atom (BOOLOP marker_atom)+`
fn marker(
    tokenizer: &mut Tokenizer<'_>,
    warnings: &mut Vec<String>,
    depth: usize,
) -> Result<(), Rejection> {
    marker_atom(tokenizer, warnings, depth)?;
    while tokenizer.read_if(Rule::BoolOp).is_some() {
        marker_atom(tokenizer, warnings, depth)?;
    }
    Ok(())
}

/// `marker_atom = WS? LEFT_PARENTHESIS WS? marker WS? RIGHT_PARENTHESIS WS? | WS? marker_item WS?`
fn marker_atom(
    tokenizer: &mut Tokenizer<'_>,
    warnings: &mut Vec<String>,
    depth: usize,
) -> Result<(), Rejection> {
    tokenizer.consume(Rule::Ws);
    if tokenizer.read_if(Rule::LeftParenthesis).is_some() {
        if depth >= MAX_MARKER_NESTING {
            return Err(Rejection::Unsupported(
                UnsupportedDomain::MarkerNestingBeyondRecursionLimit,
            ));
        }
        tokenizer.consume(Rule::Ws);
        marker(tokenizer, warnings, depth + 1)?;
        tokenizer.consume(Rule::Ws);
        if tokenizer.read_if(Rule::RightParenthesis).is_none() {
            return Err(Rejection::DependencySyntax);
        }
    } else {
        marker_item(tokenizer, warnings)?;
    }
    tokenizer.consume(Rule::Ws);
    Ok(())
}

/// `marker_item = WS? marker_var WS? marker_op WS? marker_var WS?`
fn marker_item(tokenizer: &mut Tokenizer<'_>, warnings: &mut Vec<String>) -> Result<(), Rejection> {
    tokenizer.consume(Rule::Ws);
    marker_var(tokenizer, warnings)?;
    tokenizer.consume(Rule::Ws);
    marker_op(tokenizer)?;
    tokenizer.consume(Rule::Ws);
    marker_var(tokenizer, warnings)?;
    tokenizer.consume(Rule::Ws);
    Ok(())
}

/// `marker_var = VARIABLE | QUOTED_STRING`; a quoted string goes through `ast.literal_eval`.
fn marker_var(tokenizer: &mut Tokenizer<'_>, warnings: &mut Vec<String>) -> Result<(), Rejection> {
    if tokenizer.read_if(Rule::Variable).is_some() {
        return Ok(());
    }
    let Some(token) = tokenizer.read_if(Rule::QuotedString) else {
        return Err(Rejection::DependencySyntax);
    };
    let accepted =
        compile_quoted_string(tokenizer.text(token)).map_err(|_| Rejection::LiteralEval)?;
    warnings.extend(accepted.warning);
    Ok(())
}

/// `marker_op = IN | NOT IN | OP`
fn marker_op(tokenizer: &mut Tokenizer<'_>) -> Result<(), Rejection> {
    if tokenizer.read_if(Rule::In).is_some() {
        return Ok(());
    }
    if tokenizer.read_if(Rule::Not).is_some() {
        tokenizer.expect(Rule::Ws)?;
        tokenizer.expect(Rule::In)?;
        return Ok(());
    }
    if tokenizer.read_if(Rule::Op).is_some() {
        return Ok(());
    }
    Err(Rejection::DependencySyntax)
}
