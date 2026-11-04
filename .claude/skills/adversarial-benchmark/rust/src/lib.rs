//! Broad Search stop-condition evaluator: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/sweep_stop.py`.
//!
//! [`evaluate`] is the pure decision; [`cli::run`] is the program's command line. State files follow Python's
//! JSON and `datetime.fromisoformat` rules exactly (see [`pyjson`] and [`utc`]), because the files are written by
//! Python tools and the original's observable behaviour on odd input is part of the contract.

pub mod argv;
pub mod cli;
pub mod error;
pub mod evaluate;
mod help_layouts;
pub mod jsonl;
pub mod pyjson;
pub mod pystr;
pub mod pyvalue;
pub mod report;
pub mod selftest;
pub mod terminal;
mod unicode_tables;
pub mod utc;

mod repeat_floor {
    include!(concat!(env!("OUT_DIR"), "/full_repeats_min.rs"));
}

pub use error::SweepError;
pub use evaluate::{evaluate, Evaluation, StopReason};
pub use repeat_floor::FULL_REPEATS_MIN;
pub use utc::{parse_utc, UtcInstant};
