//! Python `re` on top of `fancy-regex` (backtracking, look-around, backreferences).
//!
//! Translate the Python pattern text literally and pass Python's flags separately; `Re::new` handles the few spelling
//! differences: `\Z` (end of text) becomes `\z`, `(?P=name)` stays (fancy-regex accepts it), `(?P<name>` stays.
//! `re.match` = [`Re::match_at_start`], `re.fullmatch` = [`Re::fullmatch`], `re.search` = [`Re::search`].
//! Compile once with [`re_static!`].
use fancy_regex::Regex;

/// Captures over a `str`.
pub type Captures<'t> = fancy_regex::Captures<'t, str>;

pub const S: u32 = 1; // re.S / re.DOTALL
pub const M: u32 = 2; // re.M / re.MULTILINE
pub const I: u32 = 4; // re.I / re.IGNORECASE
pub const X: u32 = 8; // re.X / re.VERBOSE

pub struct Re {
    pub pattern: String,
    search: Regex,
    start: Regex,
    full: Regex,
}

/// Python spelling -> fancy-regex spelling: `\Z` -> `\z`; outside a character class and without `re.M`, `$` (end of
/// text or before a final newline in Python) -> `(?=\n?\z)`.
fn translate(p: &str, multiline: bool) -> String {
    let mut out = String::with_capacity(p.len());
    let mut chars = p.chars().peekable();
    let mut in_class = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some('Z') if !in_class => out.push_str("\\z"),
                Some(n) => {
                    out.push('\\');
                    out.push(n);
                }
                None => out.push('\\'),
            },
            '[' if !in_class => {
                in_class = true;
                out.push('[');
                // `[]...]` and `[^]...]`: a leading `]` is literal in Python.
                if chars.peek() == Some(&'^') {
                    out.push('^');
                    chars.next();
                }
                if chars.peek() == Some(&']') {
                    out.push_str("\\]");
                    chars.next();
                }
            }
            '[' if in_class => out.push_str("\\["),
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '$' if !in_class && !multiline => out.push_str("(?=\\n?\\z)"),
            c => out.push(c),
        }
    }
    out
}

impl Re {
    /// Compile a Python pattern with Python flags. Panics on a pattern the engine rejects (patterns are source constants).
    pub fn new(pattern: &str, flags: u32) -> Re {
        Re::try_new(pattern, flags).unwrap_or_else(|e| panic!("bad regex {pattern:?}: {e}"))
    }

    pub fn try_new(pattern: &str, flags: u32) -> Result<Re, String> {
        let mut prefix = String::new();
        if flags & (S | M | I | X) != 0 {
            prefix.push_str("(?");
            if flags & I != 0 {
                prefix.push('i');
            }
            if flags & M != 0 {
                prefix.push('m');
            }
            if flags & S != 0 {
                prefix.push('s');
            }
            if flags & X != 0 {
                prefix.push('x');
            }
            prefix.push(')');
        }
        let body = translate(pattern, flags & M != 0);
        let mk = |s: String| Regex::new(&s).map_err(|e| e.to_string());
        Ok(Re {
            pattern: pattern.to_string(),
            search: mk(format!("{prefix}{body}"))?,
            start: mk(format!("{prefix}\\A(?:{body})"))?,
            full: mk(format!("{prefix}\\A(?:{body})\\z"))?,
        })
    }

    /// `re.search(p, s) is not None`.
    pub fn is_search(&self, s: &str) -> bool {
        self.search.is_match(s).unwrap_or(false)
    }
    /// `re.match(p, s) is not None`.
    pub fn is_match(&self, s: &str) -> bool {
        self.start.is_match(s).unwrap_or(false)
    }
    /// `re.fullmatch(p, s) is not None`.
    pub fn is_full(&self, s: &str) -> bool {
        self.full.is_match(s).unwrap_or(false)
    }
    pub fn search<'t>(&self, s: &'t str) -> Option<Captures<'t>> {
        self.search.captures(s).ok().flatten()
    }
    pub fn match_at_start<'t>(&self, s: &'t str) -> Option<Captures<'t>> {
        self.start.captures(s).ok().flatten()
    }
    pub fn fullmatch<'t>(&self, s: &'t str) -> Option<Captures<'t>> {
        self.full.captures(s).ok().flatten()
    }
    /// `re.finditer` captures.
    pub fn finditer<'r, 't>(&'r self, s: &'t str) -> impl Iterator<Item = Captures<'t>> + 'r
    where
        't: 'r,
    {
        self.search.captures_iter(s).filter_map(|c| c.ok())
    }
    /// `re.findall` for a pattern with 0 groups (whole match) or 1 group (that group; `""` when it did not take part).
    pub fn findall(&self, s: &str) -> Vec<String> {
        self.finditer(s)
            .map(|c| {
                if c.len() > 1 {
                    c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default()
                } else {
                    c.get(0).map(|m| m.as_str().to_string()).unwrap_or_default()
                }
            })
            .collect()
    }
    /// `re.sub(p, repl, s)` with a closure replacement.
    pub fn sub_fn(&self, s: &str, mut f: impl FnMut(&Captures<'_>) -> String) -> String {
        let mut out = String::with_capacity(s.len());
        let mut last = 0;
        for c in self.finditer(s) {
            let m = c.get(0).unwrap();
            out.push_str(&s[last..m.start()]);
            out.push_str(&f(&c));
            last = m.end();
        }
        out.push_str(&s[last..]);
        out
    }
    /// `re.sub(p, literal, s)` where `literal` has no backslash escapes or group references.
    pub fn sub_literal(&self, s: &str, repl: &str) -> String {
        self.sub_fn(s, |_| repl.to_string())
    }
    /// `re.split(p, s)` for a pattern without groups.
    pub fn split(&self, s: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut last = 0;
        for c in self.finditer(s) {
            let m = c.get(0).unwrap();
            out.push(s[last..m.start()].to_string());
            last = m.end();
        }
        out.push(s[last..].to_string());
        out
    }
    /// The underlying search regex (for APIs not wrapped here).
    pub fn raw(&self) -> &Regex {
        &self.search
    }
}

/// Group text (`m.group(i)` or `None`).
pub fn group<'t>(c: &Captures<'t>, i: usize) -> Option<&'t str> {
    c.get(i).map(|m| m.as_str())
}

/// Named group text (`m.group(name)` or `None`).
pub fn named<'t>(c: &Captures<'t>, name: &str) -> Option<&'t str> {
    c.name(name).map(|m| m.as_str())
}

/// A lazily compiled module-level pattern: `re_static!(NAME, r"pattern", flags)` defines `fn NAME() -> &'static Re`.
#[macro_export]
macro_rules! re_static {
    ($name:ident, $pat:expr) => {
        $crate::re_static!($name, $pat, 0);
    };
    ($name:ident, $pat:expr, $flags:expr) => {
        #[allow(non_snake_case)]
        pub fn $name() -> &'static $crate::pyre::Re {
            static CELL: std::sync::OnceLock<$crate::pyre::Re> = std::sync::OnceLock::new();
            CELL.get_or_init(|| $crate::pyre::Re::new($pat, $flags))
        }
    };
}
