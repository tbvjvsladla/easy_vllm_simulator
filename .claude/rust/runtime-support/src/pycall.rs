//! How a Python call binds its arguments to a function's parameters, as plain data: ordinary positional-or-keyword
//! parameters, trailing defaults, and an optional `**rest` collecting the keywords no parameter takes. The values the caller
//! supplied are borrowed and stay where the caller put them; a default is made for the call. The failures are CPython 3.12's
//! `TypeError`s, raised in CPython's order (an unexpected or repeated keyword before too many positionals before missing
//! ones). Nothing here knows a function of any owner: no business rule, file, network or clock.

use crate::failure::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};

/// One parameter: its name and the default (`None` = required). A default is made afresh for each call, as Python
/// evaluates a mutable default once but these defaults are immutable values.
pub struct Param {
    pub name: &'static str,
    pub default: Option<fn() -> PyValue>,
}

pub const fn required(name: &'static str) -> Param {
    Param {
        name,
        default: None,
    }
}

pub const fn optional(name: &'static str, default: fn() -> PyValue) -> Param {
    Param {
        name,
        default: Some(default),
    }
}

/// A function's name and parameters. The defaulted parameters must be the last ones (a Python signature without
/// keyword-only parameters); `rest` says whether the function takes `**rest`.
pub struct Signature {
    pub name: &'static str,
    pub params: &'static [Param],
    pub rest: bool,
}

#[derive(Clone, Copy)]
enum Source {
    Positional(usize),
    Keyword,
    Missing,
}

/// The arguments of one call as the body sees them.
pub struct Bound<'a> {
    signature: &'a Signature,
    args: &'a [PyValue],
    kwargs: &'a PyDict,
    sources: Vec<Source>,
    defaults: Vec<Option<PyValue>>,
}

impl std::ops::Index<usize> for Bound<'_> {
    type Output = PyValue;

    /// Parameter `index`: the value the caller supplied, or the default made for the call.
    fn index(&self, index: usize) -> &PyValue {
        match self.sources[index] {
            Source::Positional(at) => &self.args[at],
            Source::Keyword => self
                .kwargs
                .get(self.signature.params[index].name)
                .expect("a keyword source names a present key"),
            Source::Missing => self.defaults[index]
                .as_ref()
                .expect("binding made every default"),
        }
    }
}

impl Bound<'_> {
    /// Whether the caller supplied parameter `index` (as a positional or as a keyword) rather than the default being used: a
    /// function whose default depends on something only the call knows tells "left out" from an explicit `None` with this.
    pub fn is_supplied(&self, index: usize) -> bool {
        !matches!(self.sources[index], Source::Missing)
    }

    /// The keywords no parameter took (`**rest`), in the order the caller gave them.
    pub fn rest(&self) -> Vec<(&PyStr, &PyValue)> {
        self.kwargs
            .iter()
            .filter(|(key, _)| !self.signature.params.iter().any(|p| key.equals(p.name)))
            .map(|(key, value)| (key, value))
            .collect()
    }
}

fn join_names(names: &[&str]) -> String {
    let quoted: Vec<String> = names.iter().map(|name| format!("'{name}'")).collect();
    match quoted.len() {
        1 => quoted[0].clone(),
        2 => format!("{} and {}", quoted[0], quoted[1]),
        _ => format!(
            "{}, and {}",
            quoted[..quoted.len() - 1].join(", "),
            quoted[quoted.len() - 1]
        ),
    }
}

impl Signature {
    /// Binds positional and keyword arguments as a Python call does.
    pub fn bind<'a>(
        &'a self,
        args: &'a [PyValue],
        kwargs: &'a PyDict,
    ) -> Result<Bound<'a>, Failure> {
        let name = self.name;
        let total = self.params.len();
        let mut sources: Vec<Source> = vec![Source::Missing; total];
        let mut supplied: Vec<bool> = vec![false; total];
        for index in 0..total.min(args.len()) {
            sources[index] = Source::Positional(index);
            supplied[index] = true;
        }
        for (key, _) in kwargs.iter() {
            match self.params.iter().position(|p| key.equals(p.name)) {
                None if self.rest => {}
                None => {
                    // CPython puts the key's own text between single quotes, with no escaping (it is not the key's `repr`),
                    // so a quote, a newline or a lone surrogate in the key appears as it is.
                    let mut message =
                        PyStr::new(&format!("{name}() got an unexpected keyword argument '"));
                    message.push_py(key);
                    message.push_str("'");
                    return Err(Failure::with_message("TypeError", message));
                }
                Some(position) if supplied[position] => {
                    let key_text = key.to_utf8().unwrap_or_default();
                    return Err(Failure::new(
                        "TypeError",
                        &format!("{name}() got multiple values for argument '{key_text}'"),
                    ));
                }
                Some(position) => {
                    sources[position] = Source::Keyword;
                    supplied[position] = true;
                }
            }
        }
        if args.len() > total {
            let required_count = self.params.iter().filter(|p| p.default.is_none()).count();
            let given = args.len();
            let takes = if required_count == total {
                format!(
                    "{total} positional argument{}",
                    if total == 1 { "" } else { "s" }
                )
            } else {
                format!("from {required_count} to {total} positional arguments")
            };
            let verb = if given == 1 { "was" } else { "were" };
            return Err(Failure::new(
                "TypeError",
                &format!("{name}() takes {takes} but {given} {verb} given"),
            ));
        }
        let missing: Vec<&str> = self
            .params
            .iter()
            .zip(&supplied)
            .filter(|(p, supplied)| !**supplied && p.default.is_none())
            .map(|(p, _)| p.name)
            .collect();
        if !missing.is_empty() {
            let count = missing.len();
            return Err(Failure::new(
                "TypeError",
                &format!(
                    "{name}() missing {count} required positional argument{}: {}",
                    if count == 1 { "" } else { "s" },
                    join_names(&missing)
                ),
            ));
        }
        let defaults = self
            .params
            .iter()
            .zip(&supplied)
            .map(|(p, supplied)| {
                if *supplied {
                    None
                } else {
                    p.default.map(|make| make())
                }
            })
            .collect();
        Ok(Bound {
            signature: self,
            args,
            kwargs,
            sources,
            defaults,
        })
    }
}
