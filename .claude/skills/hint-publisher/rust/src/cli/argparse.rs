//! The subset of Python 3.12 `argparse` that `hint.py` uses, reproduced byte for byte: option matching (exact,
//! `--opt=value`, unambiguous prefixes, `-hX` short-flag clusters), `store` / `store_true` / `append` / `help`, `choices`,
//! `required`, mutually exclusive groups, nested sub-parsers (`add_subparsers` + `add_parser(help=...)`), and the
//! `HelpFormatter` layout (usage wrapping, sections, `textwrap` with hyphen breaking). Width = `$COLUMNS` (else 80) − 2,
//! like `shutil.get_terminal_size()` when stdout is not a terminal.
use std::collections::HashMap;

use crate::io;
use crate::value::py_repr;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum K {
    Help,
    True,
    Store,
    Append,
    Parsers,
}

/// A parsed value (`None` / `bool` / `str` / `list[str]`).
#[derive(Clone, Debug, PartialEq)]
pub enum V {
    None,
    Bool(bool),
    Str(String),
    List(Vec<String>),
}

pub type Ns = HashMap<String, V>;

pub struct Act {
    pub opts: Vec<String>,
    pub dest: String,
    pub k: K,
    pub required: bool,
    pub choices: Option<Vec<String>>,
    pub metavar: Option<String>,
    pub help: Option<String>,
    pub default: V,
}

impl Act {
    /// An optional `--name` (dest derived like argparse: first long option, dashes → underscores).
    pub fn opt(name: &str, k: K) -> Act {
        let dest = name.trim_start_matches('-').replace('-', "_");
        let default = match k {
            K::True => V::Bool(false),
            K::Append => V::List(Vec::new()),
            _ => V::None,
        };
        Act { opts: vec![name.to_string()], dest, k, required: false, choices: None, metavar: None, help: None, default }
    }
    pub fn help(mut self, h: &str) -> Act {
        self.help = Some(h.to_string());
        self
    }
    pub fn req(mut self) -> Act {
        self.required = true;
        self
    }
    pub fn choices(mut self, c: &[&str]) -> Act {
        self.choices = Some(c.iter().map(|s| s.to_string()).collect());
        self
    }
    pub fn metavar(mut self, m: &str) -> Act {
        self.metavar = Some(m.to_string());
        self
    }
    pub fn default(mut self, v: &str) -> Act {
        self.default = V::Str(v.to_string());
        self
    }
    fn nargs0(&self) -> bool {
        matches!(self.k, K::Help | K::True)
    }
    fn is_positional(&self) -> bool {
        self.opts.is_empty()
    }
    /// `_get_action_name`.
    fn name(&self) -> String {
        if !self.opts.is_empty() {
            self.opts.join("/")
        } else if let Some(m) = &self.metavar {
            m.clone()
        } else {
            self.dest.clone()
        }
    }
}

pub struct Sub {
    pub name: String,
    pub help: Option<String>,
    pub parser: Parser,
}

pub struct Parser {
    pub prog: String,
    pub desc: Option<String>,
    pub acts: Vec<Act>,
    pub mutex: Vec<Vec<usize>>,
    pub subs: Vec<Sub>,
    /// `set_defaults(**kw)` (namespace defaults that no action owns).
    pub defaults: Vec<(String, V)>,
}

/// An argparse exit: the status (help printed = 0, error printed = 2).
pub type Exit = i32;

enum Fail {
    Arg(String),
    Exit(Exit),
}

type Tuple = (Option<usize>, String, Option<String>, Option<String>);

impl Parser {
    pub fn new(prog: &str, desc: Option<&str>) -> Parser {
        let mut p = Parser { prog: prog.to_string(), desc: desc.map(str::to_string), acts: Vec::new(), mutex: Vec::new(), subs: Vec::new(), defaults: Vec::new() };
        let mut h = Act::opt("--help", K::Help).help("show this help message and exit");
        h.opts = vec!["-h".into(), "--help".into()];
        h.dest = "help".into();
        p.acts.push(h);
        p
    }
    pub fn add(&mut self, a: Act) -> usize {
        self.acts.push(a);
        self.acts.len() - 1
    }
    /// `set_defaults(dest=value)`.
    pub fn set_default(&mut self, dest: &str, v: V) {
        self.defaults.push((dest.to_string(), v));
    }
    pub fn add_mutex(&mut self, acts: Vec<Act>) {
        let ids: Vec<usize> = acts.into_iter().map(|a| self.add(a)).collect();
        self.mutex.push(ids);
    }
    /// `add_subparsers(dest=..., metavar=..., required=...)`.
    pub fn add_subparsers(&mut self, dest: &str, metavar: &str, required: bool) {
        self.acts.push(Act {
            opts: Vec::new(),
            dest: dest.to_string(),
            k: K::Parsers,
            required,
            choices: None,
            metavar: Some(metavar.to_string()),
            help: None,
            default: V::None,
        });
    }
    /// `add_parser(name, help=...)` → the new parser (prog = `<parent prog> <name>`).
    pub fn add_parser(&mut self, name: &str, help: Option<&str>) -> &mut Parser {
        let prog = format!("{} {}", self.prog, name);
        self.subs.push(Sub { name: name.to_string(), help: help.map(str::to_string), parser: Parser::new(&prog, None) });
        &mut self.subs.last_mut().unwrap().parser
    }

    fn option_index(&self, s: &str) -> Option<usize> {
        self.acts.iter().position(|a| a.opts.iter().any(|o| o == s))
    }
    /// `_option_string_actions` in insertion order.
    fn option_strings(&self) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        for (i, a) in self.acts.iter().enumerate() {
            for o in &a.opts {
                out.push((o.clone(), i));
            }
        }
        out
    }

    // ── parsing ──────────────────────────────────────────────────────────────────────────────────
    /// `parse_args(argv)`: the namespace, or the exit status (output already written).
    pub fn parse_args(&self, argv: &[String]) -> Result<Ns, Exit> {
        let mut ns = Ns::new();
        let extras = self.parse_known_args(argv, &mut ns)?;
        if !extras.is_empty() {
            return Err(self.error(&format!("unrecognized arguments: {}", extras.join(" "))));
        }
        Ok(ns)
    }

    fn parse_known_args(&self, argv: &[String], ns: &mut Ns) -> Result<Vec<String>, Exit> {
        for a in &self.acts {
            if a.k != K::Help && !ns.contains_key(&a.dest) {
                ns.insert(a.dest.clone(), a.default.clone());
            }
        }
        for (dest, v) in &self.defaults {
            if !ns.contains_key(dest) {
                ns.insert(dest.clone(), v.clone());
            }
        }
        let mut unrec = Vec::new();
        match self.parse_inner(argv, ns, &mut unrec) {
            Ok(mut extras) => {
                extras.extend(unrec);
                Ok(extras)
            }
            Err(Fail::Arg(m)) => Err(self.error(&m)),
            Err(Fail::Exit(c)) => Err(c),
        }
    }

    fn parse_optional(&self, arg: &str) -> Result<Option<Tuple>, Fail> {
        if arg.is_empty() || !arg.starts_with('-') {
            return Ok(None);
        }
        if let Some(i) = self.option_index(arg) {
            return Ok(Some((Some(i), arg.to_string(), None, None)));
        }
        if arg.chars().count() == 1 {
            return Ok(None);
        }
        if let Some((o, e)) = arg.split_once('=') {
            if let Some(i) = self.option_index(o) {
                return Ok(Some((Some(i), o.to_string(), Some("=".into()), Some(e.to_string()))));
            }
        }
        let tuples = self.option_tuples(arg);
        if tuples.len() > 1 {
            let names: Vec<&str> = tuples.iter().map(|t| t.1.as_str()).collect();
            return Err(Fail::Exit(self.error(&format!("ambiguous option: {arg} could match {}", names.join(", ")))));
        }
        if let Some(t) = tuples.into_iter().next() {
            return Ok(Some(t));
        }
        if negative_number(arg) {
            return Ok(None);
        }
        if arg.contains(' ') {
            return Ok(None);
        }
        Ok(Some((None, arg.to_string(), None, None)))
    }

    fn option_tuples(&self, arg: &str) -> Vec<Tuple> {
        let mut out = Vec::new();
        let second_dash = arg.chars().nth(1) == Some('-');
        if second_dash {
            let (prefix, sep, explicit) = match arg.split_once('=') {
                Some((p, e)) => (p, Some("=".to_string()), Some(e.to_string())),
                None => (arg, None, None),
            };
            for (o, i) in self.option_strings() {
                if o.starts_with(prefix) {
                    out.push((Some(i), o, sep.clone(), explicit.clone()));
                }
            }
        } else {
            let short: String = arg.chars().take(2).collect();
            let short_explicit: String = arg.chars().skip(2).collect();
            for (o, i) in self.option_strings() {
                if o == short {
                    out.push((Some(i), o, Some(String::new()), Some(short_explicit.clone())));
                } else if o.starts_with(arg) {
                    out.push((Some(i), o, None, None));
                }
            }
        }
        out
    }

    fn parse_inner(&self, argv: &[String], ns: &mut Ns, unrec: &mut Vec<String>) -> Result<Vec<String>, Fail> {
        // mutual exclusion conflicts
        let mut conflicts: HashMap<usize, Vec<usize>> = HashMap::new();
        for g in &self.mutex {
            for (i, a) in g.iter().enumerate() {
                let c = conflicts.entry(*a).or_default();
                c.extend(g[..i].iter().copied());
                c.extend(g[i + 1..].iter().copied());
            }
        }
        let mut idx: HashMap<usize, Tuple> = HashMap::new();
        let mut pat = String::new();
        let mut it = argv.iter().enumerate();
        while let Some((i, a)) = it.next() {
            if a == "--" {
                pat.push('-');
                for _ in it.by_ref() {
                    pat.push('A');
                }
                break;
            }
            match self.parse_optional(a)? {
                None => pat.push('A'),
                Some(t) => {
                    idx.insert(i, t);
                    pat.push('O');
                }
            }
        }
        let pat: Vec<char> = pat.chars().collect();
        let mut seen: Vec<usize> = Vec::new();
        let mut seen_non_default: Vec<usize> = Vec::new();
        let mut extras: Vec<String> = Vec::new();
        let mut positionals: Vec<usize> = (0..self.acts.len()).filter(|i| self.acts[*i].is_positional()).collect();

        let max_opt = idx.keys().copied().max();
        let mut start = 0usize;
        if let Some(max_opt) = max_opt {
            while start <= max_opt {
                let next = *idx.keys().filter(|k| **k >= start).min().unwrap();
                if start != next {
                    let end = self.consume_positionals(start, argv, &pat, &mut positionals, ns, unrec, &mut seen, &mut seen_non_default, &conflicts)?;
                    if end > start {
                        start = end;
                        continue;
                    }
                    start = end;
                }
                if !idx.contains_key(&start) {
                    extras.extend(argv[start..next].iter().cloned());
                    start = next;
                }
                start = self.consume_optional(start, argv, &pat, &idx, ns, unrec, &mut seen, &mut seen_non_default, &conflicts, &mut extras)?;
            }
        }
        let stop = self.consume_positionals(start, argv, &pat, &mut positionals, ns, unrec, &mut seen, &mut seen_non_default, &conflicts)?;
        extras.extend(argv[stop.min(argv.len())..].iter().cloned());

        let required: Vec<String> =
            self.acts.iter().enumerate().filter(|(i, a)| !seen.contains(i) && a.required).map(|(_, a)| a.name()).collect();
        if !required.is_empty() {
            return Err(Fail::Arg(format!("the following arguments are required: {}", required.join(", "))));
        }
        Ok(extras)
    }

    #[allow(clippy::too_many_arguments)]
    fn consume_optional(
        &self,
        start: usize,
        argv: &[String],
        pat: &[char],
        idx: &HashMap<usize, Tuple>,
        ns: &mut Ns,
        unrec: &mut Vec<String>,
        seen: &mut Vec<usize>,
        snd: &mut Vec<usize>,
        conflicts: &HashMap<usize, Vec<usize>>,
        extras: &mut Vec<String>,
    ) -> Result<usize, Fail> {
        let (mut action, mut option_string, mut sep, mut explicit) = idx[&start].clone();
        let mut tuples: Vec<(usize, Vec<String>, String)> = Vec::new();
        let stop;
        loop {
            let Some(ai) = action else {
                extras.push(argv[start].clone());
                return Ok(start + 1);
            };
            let a = &self.acts[ai];
            if let Some(ex) = explicit.clone() {
                let arg_count = if a.nargs0() { 0 } else { 1 };
                let second = option_string.chars().nth(1);
                if arg_count == 0 && second != Some('-') && !ex.is_empty() {
                    if sep.as_deref().map(|s| !s.is_empty()).unwrap_or(false) || ex.starts_with('-') {
                        return Err(Fail::Arg(format!("argument {}: ignored explicit argument {}", a.name(), py_repr(&ex))));
                    }
                    tuples.push((ai, Vec::new(), option_string.clone()));
                    let first: String = ex.chars().take(1).collect();
                    let os = format!("-{first}");
                    if let Some(ni) = self.option_index(&os) {
                        action = Some(ni);
                        option_string = os;
                        let rest: String = ex.chars().skip(1).collect();
                        if rest.is_empty() {
                            sep = None;
                            explicit = None;
                        } else if let Some(r) = rest.strip_prefix('=') {
                            sep = Some("=".into());
                            explicit = Some(r.to_string());
                        } else {
                            sep = Some(String::new());
                            explicit = Some(rest);
                        }
                    } else {
                        extras.push(format!("-{ex}"));
                        stop = start + 1;
                        break;
                    }
                } else if arg_count == 1 {
                    stop = start + 1;
                    tuples.push((ai, vec![ex], option_string.clone()));
                    break;
                } else {
                    return Err(Fail::Arg(format!("argument {}: ignored explicit argument {}", a.name(), py_repr(&ex))));
                }
            } else {
                let s = start + 1;
                let rest: String = pat[s.min(pat.len())..].iter().collect();
                let n = if a.nargs0() {
                    0
                } else if rest.starts_with('A') {
                    1
                } else {
                    return Err(Fail::Arg(format!("argument {}: expected one argument", a.name())));
                };
                stop = s + n;
                tuples.push((ai, argv[s..stop].to_vec(), option_string.clone()));
                break;
            }
        }
        for (ai, args, _os) in tuples {
            self.take_action(ai, &args, ns, unrec, seen, snd, conflicts)?;
        }
        Ok(stop)
    }

    #[allow(clippy::too_many_arguments)]
    fn consume_positionals(
        &self,
        start: usize,
        argv: &[String],
        pat: &[char],
        positionals: &mut Vec<usize>,
        ns: &mut Ns,
        unrec: &mut Vec<String>,
        seen: &mut Vec<usize>,
        snd: &mut Vec<usize>,
        conflicts: &HashMap<usize, Vec<usize>>,
    ) -> Result<usize, Fail> {
        // The only positional this CLI has is a sub-parser action: pattern `-*A[-AO]*` (everything from the first `A`).
        let Some(&ai) = positionals.first() else { return Ok(start) };
        let sel = &pat[start.min(pat.len())..];
        let lead = sel.iter().take_while(|c| **c == '-').count();
        if sel.get(lead) != Some(&'A') {
            return Ok(start);
        }
        let n = sel.len();
        let args = argv[start..start + n].to_vec();
        positionals.remove(0);
        self.take_action(ai, &args, ns, unrec, seen, snd, conflicts)?;
        Ok(start + n)
    }

    #[allow(clippy::too_many_arguments)]
    fn take_action(
        &self,
        ai: usize,
        args: &[String],
        ns: &mut Ns,
        unrec: &mut Vec<String>,
        seen: &mut Vec<usize>,
        snd: &mut Vec<usize>,
        conflicts: &HashMap<usize, Vec<usize>>,
    ) -> Result<(), Fail> {
        seen.push(ai);
        let a = &self.acts[ai];
        // _get_values + _check_value
        let value: V = match a.k {
            K::Help => V::None,
            K::True => V::Bool(true),
            K::Store | K::Append => {
                let v = args.first().cloned().unwrap_or_default();
                if let Some(ch) = &a.choices {
                    if !ch.contains(&v) {
                        return Err(Fail::Arg(format!(
                            "argument {}: invalid choice: {} (choose from {})",
                            a.name(),
                            py_repr(&v),
                            ch.iter().map(|c| py_repr(c)).collect::<Vec<_>>().join(", ")
                        )));
                    }
                }
                V::Str(v)
            }
            K::Parsers => {
                let names: Vec<&str> = self.subs.iter().map(|s| s.name.as_str()).collect();
                if !names.contains(&args[0].as_str()) {
                    return Err(Fail::Arg(format!(
                        "argument {}: invalid choice: {} (choose from {})",
                        a.name(),
                        py_repr(&args[0]),
                        names.iter().map(|c| py_repr(c)).collect::<Vec<_>>().join(", ")
                    )));
                }
                V::None
            }
        };
        if a.k != K::Help {
            snd.push(ai);
            for c in conflicts.get(&ai).map(Vec::as_slice).unwrap_or(&[]) {
                if snd.contains(c) {
                    return Err(Fail::Arg(format!("argument {}: not allowed with argument {}", a.name(), self.acts[*c].name())));
                }
            }
        }
        match a.k {
            K::Help => {
                io::out(&self.format_help());
                return Err(Fail::Exit(0));
            }
            K::True | K::Store => {
                ns.insert(a.dest.clone(), value);
            }
            K::Append => {
                let mut cur = match ns.get(&a.dest) {
                    Some(V::List(l)) => l.clone(),
                    _ => Vec::new(),
                };
                if let V::Str(s) = value {
                    cur.push(s);
                }
                ns.insert(a.dest.clone(), V::List(cur));
            }
            K::Parsers => {
                ns.insert(a.dest.clone(), V::Str(args[0].clone()));
                let sub = self.subs.iter().find(|s| s.name == args[0]).unwrap();
                let mut subns = Ns::new();
                let rest = sub.parser.parse_known_args(&args[1..], &mut subns).map_err(Fail::Exit)?;
                for (k, v) in subns {
                    ns.insert(k, v);
                }
                unrec.extend(rest);
            }
        }
        Ok(())
    }

    /// `parser.error(message)`: usage + `prog: error: message` on stderr, exit 2.
    pub fn error(&self, msg: &str) -> Exit {
        io::err(&self.format_usage());
        io::err(&format!("{}: error: {msg}\n", self.prog));
        2
    }

    // ── help ─────────────────────────────────────────────────────────────────────────────────────
    pub fn format_usage(&self) -> String {
        let f = Fmt::new(width());
        let u = f.usage(self, 0);
        format!("{}\n", u.trim_matches('\n'))
    }

    pub fn format_help(&self) -> String {
        let mut f = Fmt::new(width());
        // add_argument: the longest invocation (with indented sub-actions) + the section indent (2)
        for a in &self.acts {
            let mut l = self.invocation(a).chars().count();
            if a.k == K::Parsers {
                for s in self.subs.iter().filter(|s| s.help.is_some()) {
                    l = l.max(s.name.chars().count());
                }
            }
            f.max_len = f.max_len.max(l + 2);
        }
        let mut s = f.usage(self, 0);
        if let Some(d) = &self.desc {
            let w = f.width.max(11);
            s.push_str(&fill(&collapse(d), w, ""));
            s.push_str("\n\n");
        }
        let pos: Vec<&Act> = self.acts.iter().filter(|a| a.is_positional()).collect();
        let opt: Vec<&Act> = self.acts.iter().filter(|a| !a.is_positional()).collect();
        for (title, group) in [("positional arguments", pos), ("options", opt)] {
            let items: String = group.iter().map(|a| self.format_action(&f, a, 2)).collect();
            if !items.is_empty() {
                s.push_str(&format!("\n{title}:\n{items}\n"));
            }
        }
        let mut s = long_break().sub_literal(&s, "\n\n");
        s = s.trim_matches('\n').to_string();
        s.push('\n');
        s
    }

    fn invocation(&self, a: &Act) -> String {
        if a.is_positional() {
            return a.metavar.clone().unwrap_or_else(|| a.dest.clone());
        }
        if a.nargs0() {
            return a.opts.join(", ");
        }
        let m = self.metavar_of(a);
        a.opts.iter().map(|o| format!("{o} {m}")).collect::<Vec<_>>().join(", ")
    }

    fn metavar_of(&self, a: &Act) -> String {
        if let Some(m) = &a.metavar {
            m.clone()
        } else if let Some(c) = &a.choices {
            format!("{{{}}}", c.join(","))
        } else {
            a.dest.to_uppercase()
        }
    }

    fn format_action(&self, f: &Fmt, a: &Act, indent: usize) -> String {
        let mut out = f.action(&self.invocation(a), a.help.as_deref(), indent);
        if a.k == K::Parsers {
            for s in self.subs.iter().filter(|s| s.help.is_some()) {
                out.push_str(&f.action(&s.name, s.help.as_deref(), indent + 2));
            }
        }
        out
    }

    /// `_format_actions_usage(actions, groups)` for the given action indexes.
    fn actions_usage(&self, actions: &[usize]) -> String {
        let mut group_actions: Vec<usize> = Vec::new();
        let mut inserts: HashMap<usize, String> = HashMap::new();
        for g in &self.mutex {
            let Some(start) = actions.iter().position(|x| *x == g[0]) else { continue };
            let end = start + g.len();
            if end <= actions.len() && actions[start..end] == g[..] {
                group_actions.extend(g.iter().copied());
                match inserts.get_mut(&start) {
                    Some(s) => s.push_str(" ["),
                    None => {
                        inserts.insert(start, "[".into());
                    }
                }
                match inserts.get_mut(&end) {
                    Some(s) => s.push(']'),
                    None => {
                        inserts.insert(end, "]".into());
                    }
                }
                for i in start + 1..end {
                    inserts.insert(i, "|".into());
                }
            }
        }
        let mut parts: Vec<String> = Vec::new();
        for ai in actions {
            let a = &self.acts[*ai];
            if a.is_positional() {
                let m = a.metavar.clone().unwrap_or_else(|| a.dest.clone());
                parts.push(if a.k == K::Parsers { format!("{m} ...") } else { m });
            } else {
                let mut part = if a.nargs0() { a.opts[0].clone() } else { format!("{} {}", a.opts[0], self.metavar_of(a)) };
                if !a.required && !group_actions.contains(ai) {
                    part = format!("[{part}]");
                }
                parts.push(part);
            }
        }
        let mut keys: Vec<usize> = inserts.keys().copied().collect();
        keys.sort_unstable_by(|a, b| b.cmp(a));
        for i in keys {
            parts.insert(i.min(parts.len()), inserts[&i].clone());
        }
        let text = parts.join(" ");
        let text = re_open().sub_fn(&text, |c| c.get(1).unwrap().as_str().to_string());
        let text = re_close().sub_fn(&text, |c| c.get(1).unwrap().as_str().to_string());
        let text = re_empty().sub_literal(&text, "");
        text.trim().to_string()
    }
}

crate::re_static!(re_open, r"([\[(]) ");
crate::re_static!(re_close, r" ([\])])");
crate::re_static!(re_empty, r"[\[(] *[\])]");
crate::re_static!(long_break, r"\n\n\n+");
crate::re_static!(part_re, r"\(.*?\)+(?=\s|$)|\[.*?\]+(?=\s|$)|\S+");

fn negative_number(s: &str) -> bool {
    let b = &s[1..];
    (!b.is_empty() && b.chars().all(|c| c.is_ascii_digit()))
        || match b.split_once('.') {
            Some((i, f)) => i.chars().all(|c| c.is_ascii_digit()) && !f.is_empty() && f.chars().all(|c| c.is_ascii_digit()),
            None => false,
        }
}

fn width() -> usize {
    let cols = std::env::var("COLUMNS").ok().and_then(|v| v.trim().parse::<i64>().ok()).filter(|c| *c > 0).unwrap_or(80);
    (cols - 2).max(0) as usize
}

fn clen(s: &str) -> usize {
    s.chars().count()
}

struct Fmt {
    width: usize,
    max_help: usize,
    max_len: usize,
}

impl Fmt {
    fn new(width: usize) -> Fmt {
        let max_help = 24usize.min((width as i64 - 20).max(4) as usize);
        Fmt { width, max_help, max_len: 0 }
    }

    /// `_format_usage` (prefix `usage: `) → `usage: ...\n\n`.
    fn usage(&self, p: &Parser, indent: usize) -> String {
        let prefix = "usage: ";
        let prog = p.prog.as_str();
        let optionals: Vec<usize> = (0..p.acts.len()).filter(|i| !p.acts[*i].is_positional()).collect();
        let positionals: Vec<usize> = (0..p.acts.len()).filter(|i| p.acts[*i].is_positional()).collect();
        let mut all = optionals.clone();
        all.extend(positionals.iter().copied());
        let action_usage = p.actions_usage(&all);
        let mut usage = [prog.to_string(), action_usage].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ");
        let text_width = self.width as i64 - indent as i64;
        if (clen(prefix) + clen(&usage)) as i64 > text_width {
            let opt_parts = part_re().findall(&p.actions_usage(&optionals));
            let pos_parts = part_re().findall(&p.actions_usage(&positionals));
            let get_lines = |parts: &[String], indent: &str, prefix: Option<&str>| -> Vec<String> {
                let mut lines: Vec<String> = Vec::new();
                let mut line: Vec<&str> = Vec::new();
                let indent_len = clen(indent) as i64;
                let mut line_len = match prefix {
                    Some(p) => clen(p) as i64 - 1,
                    None => indent_len - 1,
                };
                for part in parts {
                    if line_len + 1 + clen(part) as i64 > text_width && !line.is_empty() {
                        lines.push(format!("{indent}{}", line.join(" ")));
                        line.clear();
                        line_len = indent_len - 1;
                    }
                    line.push(part);
                    line_len += clen(part) as i64 + 1;
                }
                if !line.is_empty() {
                    lines.push(format!("{indent}{}", line.join(" ")));
                }
                if prefix.is_some() && !lines.is_empty() {
                    lines[0] = lines[0].chars().skip(clen(indent)).collect();
                }
                lines
            };
            let lines: Vec<String> = if (clen(prefix) + clen(prog)) as f64 <= 0.75 * text_width as f64 {
                let ind = " ".repeat(clen(prefix) + clen(prog) + 1);
                if !opt_parts.is_empty() {
                    let mut ps = vec![prog.to_string()];
                    ps.extend(opt_parts.iter().cloned());
                    let mut l = get_lines(&ps, &ind, Some(prefix));
                    l.extend(get_lines(&pos_parts, &ind, None));
                    l
                } else if !pos_parts.is_empty() {
                    let mut ps = vec![prog.to_string()];
                    ps.extend(pos_parts.iter().cloned());
                    get_lines(&ps, &ind, Some(prefix))
                } else {
                    vec![prog.to_string()]
                }
            } else {
                let ind = " ".repeat(clen(prefix));
                let mut parts = opt_parts.clone();
                parts.extend(pos_parts.iter().cloned());
                let mut l = get_lines(&parts, &ind, None);
                if l.len() > 1 {
                    l = get_lines(&opt_parts, &ind, None);
                    l.extend(get_lines(&pos_parts, &ind, None));
                }
                let mut out = vec![prog.to_string()];
                out.extend(l);
                out
            };
            usage = lines.join("\n");
        }
        format!("{prefix}{usage}\n\n")
    }

    /// `_format_action` for one invocation at `indent`.
    fn action(&self, header: &str, help: Option<&str>, indent: usize) -> String {
        let help_position = (self.max_len + 2).min(self.max_help);
        let help_width = (self.width as i64 - help_position as i64).max(11) as usize;
        let action_width = help_position as i64 - indent as i64 - 2;
        let pad = " ".repeat(indent);
        let has_help = help.map(|h| !h.is_empty()).unwrap_or(false);
        let mut out;
        let mut indent_first = 0usize;
        if !has_help {
            out = format!("{pad}{header}\n");
        } else if clen(header) as i64 <= action_width {
            let w = action_width as usize;
            out = format!("{pad}{header:<w$}  ");
        } else {
            out = format!("{pad}{header}\n");
            indent_first = help_position;
        }
        if let Some(h) = help.filter(|h| !h.trim().is_empty()) {
            let lines = wrap(&collapse(h), help_width);
            if let Some(first) = lines.first() {
                out.push_str(&format!("{}{first}\n", " ".repeat(indent_first)));
                for l in &lines[1..] {
                    out.push_str(&format!("{}{l}\n", " ".repeat(help_position)));
                }
            }
        } else if !out.ends_with('\n') {
            out.push('\n');
        }
        out
    }
}

/// `re.sub(r'\s+', ' ', text, flags=re.ASCII).strip()`.
fn collapse(text: &str) -> String {
    let mut out = String::new();
    let mut ws = false;
    for c in text.chars() {
        if matches!(c, ' ' | '\t' | '\n' | '\r' | '\x0b' | '\x0c') {
            if !ws {
                out.push(' ');
            }
            ws = true;
        } else {
            out.push(c);
            ws = false;
        }
    }
    out.trim().to_string()
}

// ── textwrap (break_on_hyphens=True · break_long_words=True · drop_whitespace=True) ─────────────────
crate::re_static!(
    wordsep,
    r#"([\t\n\x0b\x0c\r ]+|(?<=[\w!"'&.,?])-{2,}(?=\w)|[^\t\n\x0b\x0c\r ]+?(?:-(?:(?<=[^\d\W]{2}-)|(?<=[^\d\W]-[^\d\W]-))(?=[^\d\W]-?[^\d\W])|(?=[\t\n\x0b\x0c\r ]|\Z)|(?<=[\w!"'&.,?])(?=-{2,}\w)))"#
);

fn chunks(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut last = 0;
    for c in wordsep().finditer(text) {
        let m = c.get(0).unwrap();
        if m.start() > last {
            out.push(text[last..m.start()].to_string());
        }
        if !m.as_str().is_empty() {
            out.push(m.as_str().to_string());
        }
        last = m.end();
    }
    if last < text.len() {
        out.push(text[last..].to_string());
    }
    out
}

fn wrap_indent(text: &str, width: usize, initial: &str, subsequent: &str) -> Vec<String> {
    let mut ch: Vec<String> = chunks(text);
    ch.reverse();
    let mut lines: Vec<String> = Vec::new();
    while !ch.is_empty() {
        let mut cur: Vec<String> = Vec::new();
        let mut cur_len: i64 = 0;
        let indent = if lines.is_empty() { initial } else { subsequent };
        let w = width as i64 - clen(indent) as i64;
        if ch.last().map(|c| c.trim().is_empty()).unwrap_or(false) && !lines.is_empty() {
            ch.pop();
        }
        while let Some(c) = ch.last() {
            let l = clen(c) as i64;
            if cur_len + l <= w {
                cur_len += l;
                cur.push(ch.pop().unwrap());
            } else {
                break;
            }
        }
        if let Some(c) = ch.last() {
            if clen(c) as i64 > w {
                // _handle_long_word
                let space_left = if w < 1 { 1 } else { (w - cur_len) as usize };
                let chunk: Vec<char> = c.chars().collect();
                let mut end = space_left;
                if chunk.len() > space_left {
                    let hy = chunk[..space_left.min(chunk.len())].iter().rposition(|x| *x == '-');
                    if let Some(h) = hy {
                        if h > 0 && chunk[..h].iter().any(|x| *x != '-') {
                            end = h + 1;
                        }
                    }
                }
                let end = end.min(chunk.len());
                cur.push(chunk[..end].iter().collect());
                let rest: String = chunk[end..].iter().collect();
                *ch.last_mut().unwrap() = rest;
            }
        }
        if cur.last().map(|c| c.trim().is_empty()).unwrap_or(false) {
            cur.pop();
        }
        if !cur.is_empty() {
            lines.push(format!("{indent}{}", cur.concat()));
        }
    }
    lines
}

pub fn wrap(text: &str, width: usize) -> Vec<String> {
    wrap_indent(text, width, "", "")
}

fn fill(text: &str, width: usize, indent: &str) -> String {
    wrap_indent(text, width, indent, indent).join("\n")
}
