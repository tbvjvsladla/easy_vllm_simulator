//! The subset of `argparse` the three originals use: long options (exact or unique prefix, `--x=v`), store_true flags,
//! one-value options, `nargs="*"` lists, `choices`, `-h/--help`, and argparse's error text and exit code 2.
//! Usage and help texts are the originals' (80-column, non-terminal) renderings, embedded by each program.
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy)]
pub enum Kind {
    Flag,
    Value,
    Star,
    Choice(&'static [&'static str]),
}

pub struct Opt {
    pub long: &'static str,
    pub kind: Kind,
}

#[derive(Default, Debug)]
pub struct Parsed {
    pub flags: BTreeSet<&'static str>,
    pub values: BTreeMap<&'static str, String>,
    pub lists: BTreeMap<&'static str, Vec<String>>,
}

impl Parsed {
    pub fn flag(&self, long: &str) -> bool {
        self.flags.contains(long)
    }
    pub fn value(&self, long: &str) -> Option<&str> {
        self.values.get(long).map(|s| s.as_str())
    }
    pub fn list(&self, long: &str) -> Option<&Vec<String>> {
        self.lists.get(long)
    }
}

pub enum Exit {
    /// Print to stdout and exit 0 (help).
    Help(String),
    /// Print to stderr and exit 2.
    Error(String),
}

fn looks_negative_number(s: &str) -> bool {
    let t = &s[1..];
    !t.is_empty() && (t.parse::<f64>().is_ok() && t.chars().all(|c| c.is_ascii_digit() || c == '.' || c == 'e' || c == 'j'))
}

fn is_optionlike(s: &str) -> bool {
    s.starts_with('-') && s.len() > 1 && !looks_negative_number(s)
}

fn quote(s: &str) -> String {
    crate::pytext::py_repr_str(s)
}

pub fn parse(args: &[String], opts: &[Opt], prog: &str, usage: &str, help: &str) -> Result<Parsed, Exit> {
    let err = |msg: String| Exit::Error(format!("{usage}{prog}: error: {msg}\n"));
    let mut out = Parsed::default();
    let mut extras: Vec<String> = Vec::new();
    let mut i = 0;
    let mut only_positional = false;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            extras.push(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if !is_optionlike(a) {
            extras.push(a.clone());
            continue;
        }
        let (name, explicit) = if a.starts_with("--") {
            match a.find('=') {
                Some(p) => (a[..p].to_string(), Some(a[p + 1..].to_string())),
                None => (a.clone(), None),
            }
        } else {
            (a.clone(), None)
        };
        if name == "-h" {
            return Err(Exit::Help(help.to_string()));
        }
        if !name.starts_with("--") {
            extras.push(a.clone());
            continue;
        }
        // argparse matches the exact string, else every option (including --help) the text is a prefix of
        let mut all: Vec<&'static str> = vec!["--help"];
        all.extend(opts.iter().map(|o| o.long));
        let chosen: Option<&'static str> = if all.contains(&name.as_str()) {
            all.iter().copied().find(|o| *o == name)
        } else {
            let m: Vec<&'static str> = all.iter().copied().filter(|o| o.starts_with(name.as_str())).collect();
            if m.len() > 1 {
                return Err(err(format!("ambiguous option: {name} could match {}", m.join(", "))));
            }
            m.first().copied()
        };
        if chosen == Some("--help") {
            if let Some(v) = explicit {
                return Err(err(format!("argument -h/--help: ignored explicit argument {}", quote(&v))));
            }
            return Err(Exit::Help(help.to_string()));
        }
        let matched: Vec<&Opt> = opts.iter().filter(|o| Some(o.long) == chosen).collect();
        let Some(opt) = matched.first() else {
            extras.push(a.clone());
            continue;
        };
        match opt.kind {
            Kind::Flag => {
                if let Some(v) = explicit {
                    return Err(err(format!("argument {}: ignored explicit argument {}", opt.long, quote(&v))));
                }
                out.flags.insert(opt.long);
            }
            Kind::Value | Kind::Choice(_) => {
                let v = if let Some(v) = explicit {
                    v
                } else if i < args.len() && !is_optionlike(&args[i]) && args[i] != "--" {
                    i += 1;
                    args[i - 1].clone()
                } else {
                    return Err(err(format!("argument {}: expected one argument", opt.long)));
                };
                if let Kind::Choice(ch) = opt.kind {
                    if !ch.contains(&v.as_str()) {
                        return Err(err(format!(
                            "argument {}: invalid choice: {} (choose from {})",
                            opt.long,
                            quote(&v),
                            ch.iter().map(|c| quote(c)).collect::<Vec<_>>().join(", ")
                        )));
                    }
                }
                out.values.insert(opt.long, v);
            }
            Kind::Star => {
                let mut vals = Vec::new();
                if let Some(v) = explicit {
                    vals.push(v);
                } else {
                    while i < args.len() && !is_optionlike(&args[i]) && args[i] != "--" {
                        vals.push(args[i].clone());
                        i += 1;
                    }
                }
                out.lists.insert(opt.long, vals);
            }
        }
    }
    if !extras.is_empty() {
        return Err(err(format!("unrecognized arguments: {}", extras.join(" "))));
    }
    Ok(out)
}

/// Shared epilogue: write the exit and return the code.
pub fn finish(exit: Exit) -> i32 {
    use std::io::Write;
    match exit {
        Exit::Help(h) => {
            let _ = std::io::stdout().write_all(h.as_bytes());
            0
        }
        Exit::Error(e) => {
            let _ = std::io::stderr().write_all(e.as_bytes());
            2
        }
    }
}
