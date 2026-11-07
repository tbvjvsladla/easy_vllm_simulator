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

use super::Failure;
use crate::pystr::PyStr;
use std::cell::{Cell, RefCell};

/// Open Python frames `safe_load` may use under the recursion limit of the original CLI.
pub(crate) const LIMIT: usize = 998;

struct State {
    depth: Cell<usize>,
    limit: Cell<usize>,
    peak: Cell<usize>,
    trace: RefCell<Option<Vec<(usize, &'static str)>>>,
}

thread_local! {
    static STATE: State = const {
        State {
            depth: Cell::new(0),
            limit: Cell::new(LIMIT),
            peak: Cell::new(0),
            trace: RefCell::new(None),
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
    STATE.with(|s| s.depth.set(s.depth.get() - count));
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
