//! `tomllib.loads(text)` of CPython 3.12.3 with its default `parse_float` (`float`): the parser `resolve_torch_pin.main` calls on the text its
//! fetch returns, as a Rust library ([`loads`]) and as a typed observation ([`projection`]) served by a JSON-lines program ([`service`]).
//!
//! Scope (`tomllib-default-loads-cpython3123`): `loads` of a `str` only. Not `load`, not a custom `parse_float`, no non-`str` argument (the fetch the
//! parser follows returns text, so a request for anything else is a refusal of this component, not a `TypeError`/`AttributeError` of the original).
//! The decimal-integer digit limit is the interpreter profile's 4300; hexadecimal, octal and binary integers have none.
//!
//! Unsupported domains, refused by name and never answered with a guess (see [`UnsupportedDomain`]): nesting beyond the modelled share of the
//! interpreter's recursion limit, host resource exhaustion (memory, time), and text that is not what the typed request carries.
//!
//! Layout: [`value`] (the result types), [`scalars`] (`_re.py`: the number, time and date-time patterns), [`flags`] and [`nested`] (`Flags` and
//! `NestedDict` of `_parser.py`), [`parser`] (the statement grammar), [`failure`] (what is raised), [`repr`] (the `repr()` spellings of messages),
//! [`projection`] (the observation the saved originals record), [`service`] (the JSON-lines program).

pub mod failure;
mod flags;
mod nested;
pub mod parser;
pub mod projection;
mod repr;
mod scalars;
pub mod service;
pub mod value;

pub use failure::{ExceptionClass, LoadsFailure, Raised, UnsupportedDomain};
pub use parser::loads;
pub use value::{CivilDate, CivilDateTime, CivilTime, Table, Text, TomlValue, UtcOffset};
