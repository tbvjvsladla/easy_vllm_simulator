//! The Python call stack of the original, as a counter.
//!
//! CPython raises `RecursionError` when a call would make the stack of Python frames deeper than the recursion limit
//! (1000), and PyYAML's composer recurses once per nesting level, so *how deep a document may nest* is a question
//! about how many Python frames PyYAML keeps open at the deepest point - not about a number of levels. Every function
//! of the port that is a Python function of the original takes a [`frame`] for its duration, and the constructors of
//! the objects the original instantiates (marks, tokens, events, nodes, exceptions) take a frame for an instant
//! ([`nested`]); the first one that would exceed the limit fails with the original's `RecursionError`, at the same
//! moment the original would, so the error that wins is the same one.
//!
//! The limit: `roofline.py` calls `yaml.safe_load` from `main()`, which was called from the module, so two frames are
//! already open when `safe_load` starts; `safe_load` itself is frame 1 here. 1000 - 2 = 998 frames may be open. This
//! was measured on the original CLI over twelve document shapes (flow and block, sequence and mapping, with scalars,
//! aliases, anchors and tags at the bottom): reading stops exactly when the deepest stack reaches 999.

use super::sites::{CALLS, DEFS, RAISES};
use super::Failure;
use crate::pystr::PyStr;
use std::cell::{Cell, RefCell};

/// One entry of a Python traceback of the original, as source-mapping data: the file (relative to the site-packages directory for the `yaml`
/// package, or a frozen-module name), the function (`co_name`) and the line that frame was executing - the line of the call to the next
/// frame, or of the `raise` for the innermost one. This is a LOGICAL source-compatibility record computed from the frames the port opens and
/// the call/raise lines of PyYAML 6.0.1 (`sites`); it is not a native stack and Python never ran.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PySite {
    pub file: &'static str,
    pub function: &'static str,
    pub line: u32,
}

#[derive(Clone, Copy)]
struct Entry {
    name: &'static str,
    /// The class a method belongs to when the name alone does not say (`__init__`).
    qualifier: &'static str,
    /// Which of several call sites of the same callee in the caller this entry was called from (0-based, in source order).
    occurrence: Option<usize>,
}

/// Open Python frames `safe_load` may use under the recursion limit of the original CLI.
pub(crate) const LIMIT: usize = 998;

struct State {
    depth: Cell<usize>,
    limit: Cell<usize>,
    peak: Cell<usize>,
    trace: RefCell<Option<Vec<(usize, &'static str)>>>,
    stack: RefCell<Vec<Entry>>,
    pending_qualifier: Cell<&'static str>,
    pending_occurrence: Cell<Option<usize>>,
    pending_raise: Cell<Option<u32>>,
    captured: RefCell<Option<Vec<PySite>>>,
}

thread_local! {
    static STATE: State = const {
        State {
            depth: Cell::new(0),
            limit: Cell::new(LIMIT),
            peak: Cell::new(0),
            trace: RefCell::new(None),
            stack: RefCell::new(Vec::new()),
            pending_qualifier: Cell::new(""),
            pending_occurrence: Cell::new(None),
            pending_raise: Cell::new(None),
            captured: RefCell::new(None),
        }
    };
}

/// Starts a load: no frame open, the given limit, optionally recording every frame entered (name and depth).
pub(crate) fn reset(limit: usize, trace: bool) {
    STATE.with(|s| {
        s.depth.set(0);
        s.limit.set(limit);
        s.peak.set(0);
        *s.trace.borrow_mut() = if trace { Some(Vec::new()) } else { None };
        s.stack.borrow_mut().clear();
        s.pending_qualifier.set("");
        s.pending_occurrence.set(None);
        s.pending_raise.set(None);
        *s.captured.borrow_mut() = None;
    });
}

/// The deepest stack of the last load.
pub(crate) fn peak() -> usize {
    STATE.with(|s| s.peak.get())
}

/// Drops what was recorded so far (the loader's construction, which the original's trace starts after).
pub(crate) fn forget_trace() {
    STATE.with(|s| {
        if let Some(trace) = s.trace.borrow_mut().as_mut() {
            trace.clear();
        }
    });
}

pub(crate) fn take_trace() -> Vec<(usize, &'static str)> {
    STATE.with(|s| s.trace.borrow_mut().take().unwrap_or_default())
}

fn recursion_error() -> Failure {
    Failure::Plain {
        class: "RecursionError",
        message: PyStr::new("maximum recursion depth exceeded"),
    }
}

fn enter(name: &'static str) -> Result<(), Failure> {
    STATE.with(|s| {
        let depth = s.depth.get() + 1;
        if depth > s.limit.get() {
            return Err(recursion_error());
        }
        s.depth.set(depth);
        s.stack.borrow_mut().push(Entry {
            name,
            qualifier: s.pending_qualifier.replace(""),
            occurrence: s.pending_occurrence.take(),
        });
        if depth > s.peak.get() {
            s.peak.set(depth);
        }
        if let Some(trace) = s.trace.borrow_mut().as_mut() {
            trace.push((depth, name));
        }
        Ok(())
    })
}

fn leave(count: usize) {
    STATE.with(|s| {
        s.depth.set(s.depth.get() - count);
        let mut stack = s.stack.borrow_mut();
        let keep = stack.len().saturating_sub(count);
        stack.truncate(keep);
    });
}

/// The next frame entered is a method of `qualifier` (`SafeLoader`, `Reader`, `Scanner`) and, when the caller calls the same callee at several lines,
/// the `occurrence`-th of those call sites (0-based, source order).
pub(crate) fn hint(qualifier: &'static str, occurrence: Option<usize>) {
    STATE.with(|s| {
        s.pending_qualifier.set(qualifier);
        s.pending_occurrence.set(occurrence);
    });
}

/// The next frame entered is the `occurrence`-th call site of its callee in its caller.
pub(crate) fn occurrence(index: usize) {
    STATE.with(|s| s.pending_occurrence.set(Some(index)));
}

/// The line of the `raise` the failure created next mirrors (a failure with no known raise line is not mapped).
pub(crate) fn set_raise(line: u32) {
    STATE.with(|s| s.pending_raise.set(Some(line)));
}

fn simple(function: &str) -> &str {
    function.rsplit('.').next().unwrap_or(function)
}

/// The file and qualified name of the Python function a frame is, or `None` when the name is ambiguous.
fn definition(entry: &Entry) -> Option<(&'static str, &'static str)> {
    let mut found = DEFS.iter().filter(|(_, function, _)| {
        simple(function) == entry.name
            && (entry.qualifier.is_empty()
                || function
                    .split_once('.')
                    .is_some_and(|(class, _)| class == entry.qualifier))
    });
    let first = found.next()?;
    if found.next().is_some() {
        return None;
    }
    Some((first.0, first.1))
}

fn call_lines(file: &str, caller: &str, callee: &str) -> Vec<u32> {
    let mut lines: Vec<u32> = CALLS
        .iter()
        .filter(|(f, c, g, _)| *f == file && *c == caller && *g == callee)
        .map(|(_, _, _, line)| *line)
        .collect();
    lines.sort_unstable();
    lines.dedup();
    lines
}

fn is_parser_state(name: &str) -> bool {
    name.starts_with("parse_")
}

/// Resolves the open frames into traceback entries: the line of each frame is the call site of the next one (looked up in the source map; the
/// entry's occurrence picks among several), the innermost one is `raise` (or, with a `leaf` in a module outside the package, the line of the call
/// that reaches it). `None` when any frame, call or raise cannot be placed from the source map: the chain is then not claimed.
fn resolve(stack: &[Entry], raise: Option<u32>, leaf: Option<PySite>) -> Option<Vec<PySite>> {
    let mut out: Vec<PySite> = Vec::with_capacity(stack.len() + 1);
    for (index, entry) in stack.iter().enumerate() {
        let (file, qualified) = definition(entry)?;
        let line = match stack.get(index + 1) {
            Some(next) => {
                let callee = if matches!(
                    simple(qualified),
                    "check_event" | "peek_event" | "get_event"
                ) && is_parser_state(next.name)
                {
                    "state"
                } else if simple(qualified) == "load" && next.name == "__init__" {
                    "Loader"
                } else {
                    next.name
                };
                let lines = call_lines(file, qualified, callee);
                match lines.as_slice() {
                    [only] => *only,
                    [] => return None,
                    many => *many.get(next.occurrence?)?,
                }
            }
            None => match &leaf {
                Some(site) => {
                    let callee = if site.function == "decode" {
                        "read"
                    } else {
                        site.function
                    };
                    match call_lines(file, qualified, callee).as_slice() {
                        [only] => *only,
                        _ => return None,
                    }
                }
                None => {
                    // the raise line must be a `raise` of that very function in the source map
                    let line = raise?;
                    if !RAISES
                        .iter()
                        .any(|(f, q, l, _)| *f == file && *q == qualified && *l == line)
                    {
                        return None;
                    }
                    line
                }
            },
        };
        let function = simple(qualified);
        let function = DEFS
            .iter()
            .find(|(f, q, _)| *f == file && *q == qualified)
            .map(|(_, q, _)| simple(q))
            .unwrap_or(function);
        out.push(PySite {
            file: package_path(file),
            function,
            line,
        });
    }
    out.extend(leaf);
    Some(out)
}

fn package_path(file: &'static str) -> &'static str {
    match file {
        "__init__.py" => "yaml/__init__.py",
        "loader.py" => "yaml/loader.py",
        "reader.py" => "yaml/reader.py",
        "scanner.py" => "yaml/scanner.py",
        "parser.py" => "yaml/parser.py",
        "composer.py" => "yaml/composer.py",
        "constructor.py" => "yaml/constructor.py",
        "resolver.py" => "yaml/resolver.py",
        other => other,
    }
}

/// Records the traceback of the failure being created now (the open frames, the pending raise line, an optional leaf frame outside the
/// package). The record is read by [`take_captured`] when the failure becomes the error.
pub(crate) fn capture(leaf: Option<PySite>) {
    STATE.with(|s| {
        let raise = s.pending_raise.take();
        let stack = s.stack.borrow().clone();
        *s.captured.borrow_mut() = resolve(&stack, raise, leaf);
    });
}

/// The record of the last [`capture`] (read without consuming it: the reader reports a failure to the file observer first and the load
/// reports it again); `None` when the failure had no mapped traceback.
pub(crate) fn take_captured() -> Option<Vec<PySite>> {
    STATE.with(|s| {
        s.pending_raise.set(None);
        s.captured.borrow().clone()
    })
}

/// An open Python frame; closes when dropped.
#[must_use = "a frame closes when it is dropped"]
pub(crate) struct Frame;

impl Drop for Frame {
    fn drop(&mut self) {
        leave(1);
    }
}

/// Enters the Python function `name`.
pub(crate) fn frame(name: &'static str) -> Result<Frame, Failure> {
    enter(name)?;
    Ok(Frame)
}

/// Calls the Python functions `names` one inside the other and returns from all of them at once: the frames a
/// constructor (`Mark.__init__`) or a helper that only builds a value takes.
pub(crate) fn nested(names: &[&'static str]) -> Result<(), Failure> {
    let mut entered = 0;
    for name in names {
        if let Err(error) = enter(name) {
            leave(entered);
            return Err(error);
        }
        entered += 1;
    }
    leave(entered);
    Ok(())
}
