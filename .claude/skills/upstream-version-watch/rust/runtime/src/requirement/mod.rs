//! The `packaging.requirements.Requirement` constructor of packaging 24.0 and the projection `resolve_torch_pin.main` consumes from it:
//! whether the constructor accepts the value, and for an accepted one `req.name`, `str(req.specifier)` and the specifiers in the order iterating
//! `req.specifier` yields them.
//!
//! Scope (`requirement-main-projection-packaging24`): the constructor and that projection only. Not the full `Requirement` object (`__str__`, `__hash__`,
//! `__eq__`, extras and URL as values), no marker evaluation, no containment test. A rejection keeps no diagnostic: `main` catches every `Exception`
//! from the constructor and skips the entry, so only the fact of the rejection and the warnings printed before it are observable.
//!
//! The iteration order of the specifier `frozenset` depends on `hash()` of strings, which Python randomizes per process. This component supports
//! the two explicit, reproducible profiles `PYTHONHASHSEED=0` and `PYTHONHASHSEED=17` only ([`projection::SUPPORTED_HASH_SEEDS`]); the random,
//! unset and `-R` profiles are outside it.
//!
//! Layout: [`pyre`] (the `re` subset), [`grammar`] (`_tokenizer.py` and `_parser.py`), [`specifier`] (`specifiers.py`, `utils.canonicalize_version`),
//! [`string_literal`] with [`unicode_names`] (`ast.literal_eval` of a marker string), [`projection`] (the business function), [`service`] (the JSON-lines
//! program).

mod grammar;
pub mod projection;
mod pyre;
pub mod service;
mod specifier;
mod string_literal;
mod unicode_names;
#[rustfmt::skip]
mod unicode_names_data;

/// Why the constructor raises. Every variant except [`Rejection::Unsupported`] is an `Exception` that `main` skips; the variants name the
/// original raise sites for the unit tests, they are not part of the observable projection.
#[derive(Debug)]
pub(super) enum Rejection {
    /// `ParserSyntaxError` (`InvalidRequirement`).
    DependencySyntax,
    /// `ast.literal_eval` of a marker string raises (see [`string_literal::LiteralRejection`] for the cases).
    LiteralEval,
    /// `InvalidSpecifier`.
    InvalidSpecifier,
    /// `Version(...)` of a specifier fails an integer conversion (`ValueError`, over the 4300-digit limit).
    VersionIntegerLimit,
    /// A domain this component does not model; the answer is a refusal, never a rejection.
    Unsupported(UnsupportedDomain),
}

/// A named domain outside this component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsupportedDomain {
    /// A marker with more nested parentheses than the parser's recursion is certain to survive: the interpreter's `RecursionError` depends on
    /// the stack the caller already has, which is not modelled.
    MarkerNestingBeyondRecursionLimit,
    /// The thread that runs the construction could not be started with its large stack (or panicked).
    ConstructionStackUnavailable,
}

impl UnsupportedDomain {
    /// The stable name used in refusal records.
    pub fn name(self) -> &'static str {
        match self {
            UnsupportedDomain::MarkerNestingBeyondRecursionLimit => {
                "marker_nesting_beyond_recursion_limit"
            }
            UnsupportedDomain::ConstructionStackUnavailable => "construction_stack_unavailable",
        }
    }

    pub fn detail(self) -> &'static str {
        match self {
            UnsupportedDomain::MarkerNestingBeyondRecursionLimit => {
                "marker parentheses are nested deeper than 100 levels; whether the original's parser raises RecursionError there depends on the caller's stack, which this component does not model"
            }
            UnsupportedDomain::ConstructionStackUnavailable => {
                "the construction thread with its large matcher stack could not be started or panicked"
            }
        }
    }
}
