//! The slice of CPython 3.12 `argparse` the two CLIs use: long/short options with one value or none, `--opt=value`,
//! unique-prefix abbreviations (and the ambiguity error), required options, one optional sub-command positional
//! (`add_subparsers(dest="command")`, PARSER nargs), unrecognized-argument collection, `-h/--help`, and the
//! `HelpFormatter`/`RawDescriptionHelpFormatter` output (usage wrapping, `textwrap` filling with the hyphen-aware word
//! splitter, `COLUMNS`-driven width). Messages are argparse's own text.

use fancy_regex::Regex;
use std::sync::OnceLock;

#[derive(Clone, Debug)]
pub struct Opt {
    pub strings: Vec<&'static str>,
    /// `None` = a flag (store_true / help); `Some(dest)` = one value, metavar = dest upper-cased.
    pub dest: Option<&'static str>,
    pub required: bool,
    pub help: Option<&'static str>,
    pub is_help: bool,
}

impl Opt {
    pub fn help_flag() -> Opt {
        Opt { strings: vec!["-h", "--help"], dest: None, required: false, help: Some("show this help message and exit"), is_help: true }
    }
    pub fn flag(s: &'static str, help: Option<&'static str>) -> Opt {
        Opt { strings: vec![s], dest: None, required: false, help, is_help: false }
    }
    pub fn value(s: &'static str, dest: &'static str, required: bool, help: Option<&'static str>) -> Opt {
        Opt { strings: vec![s], dest: Some(dest), required, help, is_help: false }
    }
    fn name(&self) -> String {
        self.strings.join("/")
    }
    fn metavar(&self) -> String {
        self.dest.unwrap_or("").to_uppercase()
    }
    fn invocation(&self) -> String {
        match self.dest {
            None => self.strings.join(", "),
            Some(_) => self.strings.iter().map(|s| format!("{s} {}", self.metavar())).collect::<Vec<_>>().join(", "),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Parser {
    pub prog: String,
    pub description: Option<&'static str>,
    pub raw_description: bool,
    pub options: Vec<Opt>,
    /// sub-command name -> parser (prog = "<prog> <name>")
    pub subcommands: Vec<(&'static str, Parser)>,
}

#[derive(Debug, Default, Clone)]
pub struct Parsed {
    pub flags: Vec<&'static str>,
    pub values: Vec<(&'static str, String)>,
    pub command: Option<&'static str>,
    pub sub: Option<Box<Parsed>>,
}

impl Parsed {
    pub fn flag(&self, s: &str) -> bool {
        self.flags.iter().any(|f| *f == s)
    }
    pub fn value(&self, dest: &str) -> Option<&str> {
        self.values.iter().rev().find(|(d, _)| *d == dest).map(|(_, v)| v.as_str())
    }
}

/// What argparse would do instead of returning.
pub enum Exit {
    /// print to stdout, exit 0
    Help(String),
    /// `parser.error(message)`; `usage` is that parser's usage text (for the default handler)
    Error { message: String, usage: String, prog: String },
}

fn negative_number(s: &str) -> bool {
    // '^-\d+$|^-\d*\.\d+$'
    let Some(rest) = s.strip_prefix('-') else { return false };
    if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    if let Some((a, b)) = rest.split_once('.') {
        return a.chars().all(|c| c.is_ascii_digit()) && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit());
    }
    false
}

enum Class {
    Arg,
    Opt(usize, Option<String>),
    Unknown,
}

impl Parser {
    fn err(&self, message: String) -> Exit {
        Exit::Error { message, usage: self.format_usage(), prog: self.prog.clone() }
    }

    fn classify(&self, a: &str) -> Result<Class, Exit> {
        if a.is_empty() || !a.starts_with('-') {
            return Ok(Class::Arg);
        }
        if let Some(i) = self.options.iter().position(|o| o.strings.contains(&a)) {
            return Ok(Class::Opt(i, None));
        }
        if a.len() == 1 {
            return Ok(Class::Arg);
        }
        if let Some((head, tail)) = a.split_once('=') {
            if let Some(i) = self.options.iter().position(|o| o.strings.contains(&head)) {
                return Ok(Class::Opt(i, Some(tail.to_string())));
            }
        }
        // abbreviations (only `--` long options here)
        if a.starts_with("--") {
            let (prefix, explicit) = match a.split_once('=') {
                Some((h, t)) => (h, Some(t.to_string())),
                None => (a, None),
            };
            let mut matches: Vec<(usize, &str)> = Vec::new();
            for (i, o) in self.options.iter().enumerate() {
                for s in &o.strings {
                    if s.starts_with(prefix) {
                        matches.push((i, s));
                    }
                }
            }
            if matches.len() > 1 {
                let names: Vec<&str> = matches.iter().map(|(_, s)| *s).collect();
                return Err(self.err(format!("ambiguous option: {a} could match {}", names.join(", "))));
            }
            if matches.len() == 1 {
                return Ok(Class::Opt(matches[0].0, explicit));
            }
        } else if a.starts_with('-') && !a.starts_with("--") {
            // single-dash prefix: `-hx` style is not used by these parsers; an exact `-h=...` handled above
        }
        if negative_number(a) {
            return Ok(Class::Arg);
        }
        if a.contains(' ') {
            return Ok(Class::Arg);
        }
        Ok(Class::Unknown)
    }

    /// `parse_known_args` + the parent's handling. Returns the parse and the unrecognized arguments.
    fn parse_known(&self, args: &[String]) -> Result<(Parsed, Vec<String>), Exit> {
        // classify everything first (ambiguity errors fire here, before any consumption)
        let mut classes = Vec::with_capacity(args.len());
        let mut after_dd = false;
        for a in args {
            if after_dd {
                classes.push(Class::Arg);
                continue;
            }
            if a == "--" {
                after_dd = true;
                classes.push(Class::Arg);
                continue;
            }
            classes.push(self.classify(a)?);
        }
        let mut parsed = Parsed::default();
        let mut extras: Vec<String> = Vec::new();
        let mut seen: Vec<usize> = Vec::new();
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--" && !self.subcommands.is_empty() && parsed.command.is_none() {
                // the separator itself is dropped; everything after is positional
                i += 1;
                if i < args.len() {
                    return self.dispatch_sub(args, i, parsed, extras, &seen);
                }
                break;
            }
            match &classes[i] {
                Class::Arg => {
                    if !self.subcommands.is_empty() && parsed.command.is_none() {
                        return self.dispatch_sub(args, i, parsed, extras, &seen);
                    }
                    extras.push(args[i].clone());
                    i += 1;
                }
                Class::Unknown => {
                    extras.push(args[i].clone());
                    i += 1;
                }
                Class::Opt(k, explicit) => {
                    let o = &self.options[*k];
                    if o.is_help {
                        if explicit.is_some() {
                            return Err(self.err(format!("argument {}: ignored explicit argument {}", o.name(), crate::value::r(explicit.as_deref().unwrap()))));
                        }
                        return Err(Exit::Help(self.format_help()));
                    }
                    match o.dest {
                        None => {
                            if let Some(x) = explicit {
                                return Err(self.err(format!("argument {}: ignored explicit argument {}", o.name(), crate::value::r(x))));
                            }
                            parsed.flags.push(o.strings[0]);
                            i += 1;
                        }
                        Some(dest) => {
                            if let Some(x) = explicit {
                                parsed.values.push((dest, x.clone()));
                                i += 1;
                            } else if i + 1 < args.len() && matches!(classes[i + 1], Class::Arg) && !(args[i + 1] == "--") {
                                parsed.values.push((dest, args[i + 1].clone()));
                                i += 2;
                            } else {
                                return Err(self.err(format!("argument {}: expected one argument", o.name())));
                            }
                        }
                    }
                    seen.push(*k);
                }
            }
        }
        self.check_required(&seen)?;
        Ok((parsed, extras))
    }

    fn check_required(&self, seen: &[usize]) -> Result<(), Exit> {
        let missing: Vec<String> = self.options.iter().enumerate().filter(|(k, o)| o.required && !seen.contains(k)).map(|(_, o)| o.name()).collect();
        if !missing.is_empty() {
            return Err(self.err(format!("the following arguments are required: {}", missing.join(", "))));
        }
        Ok(())
    }

    fn dispatch_sub(&self, args: &[String], i: usize, mut parsed: Parsed, mut extras: Vec<String>, seen: &[usize]) -> Result<(Parsed, Vec<String>), Exit> {
        let name = &args[i];
        let Some((cmd, sub)) = self.subcommands.iter().find(|(n, _)| *n == name.as_str()) else {
            let choices: Vec<String> = self.subcommands.iter().map(|(n, _)| crate::value::r(n)).collect();
            return Err(self.err(format!("argument command: invalid choice: {} (choose from {})", crate::value::r(name), choices.join(", "))));
        };
        parsed.command = Some(cmd);
        let (sp, sub_extras) = sub.parse_known(&args[i + 1..])?;
        parsed.sub = Some(Box::new(sp));
        self.check_required(seen)?;
        extras.extend(sub_extras);
        Ok((parsed, extras))
    }

    pub fn parse(&self, args: &[String]) -> Result<Parsed, Exit> {
        let (parsed, extras) = self.parse_known(args)?;
        if !extras.is_empty() {
            return Err(self.err(format!("unrecognized arguments: {}", extras.join(" "))));
        }
        Ok(parsed)
    }

    // ── help ──────────────────────────────────────────────────────────────────────────────────

    fn usage_parts(&self) -> (Vec<String>, Vec<String>) {
        let mut opt = Vec::new();
        for o in &self.options {
            let inner = match o.dest {
                None => o.strings[0].to_string(),
                Some(_) => format!("{} {}", o.strings[0], o.metavar()),
            };
            opt.push(if o.required { inner } else { format!("[{inner}]") });
        }
        let mut pos = Vec::new();
        if !self.subcommands.is_empty() {
            let names: Vec<&str> = self.subcommands.iter().map(|(n, _)| *n).collect();
            pos.push(format!("{{{}}} ...", names.join(",")));
        }
        (opt, pos)
    }

    pub fn format_usage(&self) -> String {
        let width = help_width();
        let prefix = "usage: ";
        let (opt, pos) = self.usage_parts();
        let action_usage = opt.iter().chain(pos.iter()).cloned().collect::<Vec<_>>().join(" ");
        let usage = [self.prog.clone(), action_usage.clone()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" ");
        let text_width = width;
        let body = if prefix.len() + usage.chars().count() > text_width {
            let split = |s: &str| -> Vec<String> { usage_part_split(s) };
            let opt_parts = split(&opt.join(" "));
            let pos_parts = split(&pos.join(" "));
            let get_lines = |parts: &[String], indent: &str, prefix: Option<&str>| -> Vec<String> {
                let mut lines = Vec::new();
                let mut line: Vec<String> = Vec::new();
                let indent_length = indent.chars().count();
                let mut line_len = match prefix {
                    Some(p) => p.chars().count() as isize - 1,
                    None => indent_length as isize - 1,
                };
                for part in parts {
                    let plen = part.chars().count() as isize;
                    if line_len + 1 + plen > text_width as isize && !line.is_empty() {
                        lines.push(format!("{indent}{}", line.join(" ")));
                        line.clear();
                        line_len = indent_length as isize - 1;
                    }
                    line.push(part.clone());
                    line_len += plen + 1;
                }
                if !line.is_empty() {
                    lines.push(format!("{indent}{}", line.join(" ")));
                }
                if prefix.is_some() && !lines.is_empty() {
                    lines[0] = lines[0][indent.len()..].to_string();
                }
                lines
            };
            let prog_len = self.prog.chars().count();
            let lines: Vec<String> = if (prefix.len() + prog_len) as f64 <= 0.75 * text_width as f64 {
                let indent = " ".repeat(prefix.len() + prog_len + 1);
                let mut first = vec![self.prog.clone()];
                if !opt_parts.is_empty() {
                    first.extend(opt_parts.iter().cloned());
                    let mut l = get_lines(&first, &indent, Some(prefix));
                    l.extend(get_lines(&pos_parts, &indent, None));
                    l
                } else if !pos_parts.is_empty() {
                    first.extend(pos_parts.iter().cloned());
                    get_lines(&first, &indent, Some(prefix))
                } else {
                    vec![self.prog.clone()]
                }
            } else {
                let indent = " ".repeat(prefix.len());
                let parts: Vec<String> = opt_parts.iter().chain(pos_parts.iter()).cloned().collect();
                let mut l = get_lines(&parts, &indent, None);
                if l.len() > 1 {
                    l = get_lines(&opt_parts, &indent, None);
                    l.extend(get_lines(&pos_parts, &indent, None));
                }
                let mut out = vec![self.prog.clone()];
                out.extend(l);
                out
            };
            lines.join("\n")
        } else {
            usage
        };
        format!("{prefix}{body}\n\n")
    }

    pub fn format_help(&self) -> String {
        let width = help_width();
        let max_help_position = 24usize.min((width as isize - 20).max(4) as usize);
        let mut out = self.format_usage();
        if let Some(desc) = self.description {
            if self.raw_description {
                out.push_str(desc);
                out.push_str("\n\n");
            } else {
                let text = collapse_ws(desc);
                let lines = textwrap(&text, width.max(11));
                out.push_str(&lines.join("\n"));
                out.push_str("\n\n");
            }
        }
        // action max length over every invocation (indent 2)
        let mut invocations: Vec<String> = self.options.iter().map(|o| o.invocation()).collect();
        let pos_inv = if self.subcommands.is_empty() {
            None
        } else {
            Some(format!("{{{}}}", self.subcommands.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(",")))
        };
        if let Some(p) = &pos_inv {
            invocations.push(p.clone());
        }
        let action_max = invocations.iter().map(|s| s.chars().count() + 2).max().unwrap_or(0);
        let help_position = (action_max + 2).min(max_help_position);
        let help_w = (width as isize - help_position as isize).max(11) as usize;
        let action_width = help_position - 2 - 2;
        let fmt_action = |inv: &str, help: Option<&str>| -> String {
            let mut s = String::new();
            match help {
                None => {
                    s.push_str(&format!("  {inv}\n"));
                }
                Some(h) => {
                    let lines = textwrap(&collapse_ws(h), help_w);
                    if inv.chars().count() <= action_width {
                        s.push_str(&format!("  {inv:<action_width$}  "));
                        s.push_str(lines.first().map(|x| x.as_str()).unwrap_or(""));
                        s.push('\n');
                    } else {
                        s.push_str(&format!("  {inv}\n"));
                        s.push_str(&format!("{}{}\n", " ".repeat(help_position), lines.first().map(|x| x.as_str()).unwrap_or("")));
                    }
                    for l in lines.iter().skip(1) {
                        s.push_str(&format!("{}{l}\n", " ".repeat(help_position)));
                    }
                }
            }
            s
        };
        if let Some(p) = &pos_inv {
            out.push_str("positional arguments:\n");
            out.push_str(&fmt_action(p, None));
            out.push('\n');
        }
        out.push_str("options:\n");
        for o in &self.options {
            out.push_str(&fmt_action(&o.invocation(), o.help));
        }
        // _long_break_matcher.sub('\n\n', help); help.strip('\n') + '\n'
        let mut collapsed = String::with_capacity(out.len());
        let mut run = 0usize;
        for c in out.chars() {
            if c == '\n' {
                run += 1;
                if run <= 2 {
                    collapsed.push(c);
                }
            } else {
                run = 0;
                collapsed.push(c);
            }
        }
        let mut t = collapsed.trim_matches('\n').to_string();
        t.push('\n');
        t
    }
}

/// `part_regexp = r'\(.*?\)+(?=\s|$)|\[.*?\]+(?=\s|$)|\S+'` findall.
fn usage_part_split(s: &str) -> Vec<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"\(.*?\)+(?=\s|$)|\[.*?\]+(?=\s|$)|\S+").unwrap());
    re.find_iter(s).filter_map(|m| m.ok()).map(|m| m.as_str().to_string()).collect()
}

/// `shutil.get_terminal_size().columns - 2`
pub fn help_width() -> usize {
    let cols = std::env::var("COLUMNS").ok().and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0);
    let cols = if cols > 0 {
        cols as usize
    } else {
        terminal_columns().unwrap_or(80)
    };
    cols.saturating_sub(2)
}

fn terminal_columns() -> Option<usize> {
    // SAFETY: TIOCGWINSZ on fd 1 writes into the winsize struct we own.
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        if libc::ioctl(1, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
            Some(ws.ws_col as usize)
        } else {
            None
        }
    }
}

/// `re.sub(r'\s+', ' ', text, flags=re.ASCII).strip()`
pub fn collapse_ws(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut ws = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') {
            ws = true;
        } else {
            if ws && !out.is_empty() {
                out.push(' ');
            }
            ws = false;
            out.push(c);
        }
    }
    out
}

fn wordsep() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        let wp = r#"[\w!"'&.,?]"#;
        let lt = r"[^\d\W]";
        let ws = r"[\t\n\x0b\x0c\r ]";
        let nws = r"[^\t\n\x0b\x0c\r ]";
        let pat = format!(
            r"({ws}+|(?<={wp})-{{2,}}(?=\w)|{nws}+?(?:-(?:(?<={lt}{lt}-)|(?<={lt}-{lt}-))(?={lt}-?{lt})|(?={ws}|\z)|(?<={wp})(?=-{{2,}}\w)))"
        );
        Regex::new(&pat).unwrap()
    })
}

/// `textwrap.wrap(text, width)` with the defaults argparse uses.
pub fn textwrap(text: &str, width: usize) -> Vec<String> {
    // split into chunks (re.split with a capturing group, empty strings dropped)
    let mut chunks: Vec<String> = Vec::new();
    let mut last = 0;
    for m in wordsep().find_iter(text).filter_map(|m| m.ok()) {
        if m.start() > last {
            chunks.push(text[last..m.start()].to_string());
        }
        if !m.as_str().is_empty() {
            chunks.push(m.as_str().to_string());
        }
        last = m.end();
    }
    if last < text.len() {
        chunks.push(text[last..].to_string());
    }
    let clen = |s: &str| s.chars().count();
    chunks.reverse();
    let mut lines: Vec<String> = Vec::new();
    while !chunks.is_empty() {
        let mut cur: Vec<String> = Vec::new();
        let mut cur_len = 0usize;
        let w = width;
        if chunks.last().map_or(false, |c| c.trim().is_empty()) && !lines.is_empty() {
            chunks.pop();
        }
        while let Some(c) = chunks.last() {
            let l = clen(c);
            if cur_len + l <= w {
                cur_len += l;
                cur.push(chunks.pop().unwrap());
            } else {
                break;
            }
        }
        if chunks.last().map_or(false, |c| clen(c) > w) {
            let space_left = if w < 1 { 1 } else { w - cur_len };
            let chunk: Vec<char> = chunks.last().unwrap().chars().collect();
            let mut end = space_left;
            if chunk.len() > space_left {
                let hyphen = chunk[..space_left.min(chunk.len())].iter().rposition(|&c| c == '-');
                if let Some(h) = hyphen {
                    if h > 0 && chunk[..h].iter().any(|&c| c != '-') {
                        end = h + 1;
                    }
                }
            }
            let head: String = chunk[..end.min(chunk.len())].iter().collect();
            let tail: String = chunk[end.min(chunk.len())..].iter().collect();
            cur.push(head);
            *chunks.last_mut().unwrap() = tail;
            cur_len = cur.iter().map(|c| clen(c)).sum();
        }
        if cur.last().map_or(false, |c| c.trim().is_empty()) {
            cur_len -= clen(cur.last().unwrap());
            cur.pop();
        }
        let _ = cur_len;
        if !cur.is_empty() {
            lines.push(cur.concat());
        }
    }
    lines
}
