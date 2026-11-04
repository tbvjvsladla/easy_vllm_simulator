//! Broad Search stop-condition evaluator: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/sweep_stop.py`.
//!
//! [`evaluate`] is the pure decision; [`cli::run`] is the program's command line. State files follow Python's
//! JSON and `datetime.fromisoformat` rules exactly (see [`pyjson`] and [`utc`]), because the files are written by
//! Python tools and the original's observable behaviour on odd input is part of the contract.

pub mod argv;
pub mod bench_cli;
mod bench_selftest;
pub mod cli;
pub mod engine_log;
pub mod error;
pub mod evaluate;
mod help_layouts;
mod help_layouts_parse_bench;
mod help_layouts_roofline;
mod help_layouts_verdict_rule;
pub mod jsonl;
pub mod parse_bench;
pub mod pyjson;
pub mod pynumber;
pub mod pyops;
pub mod pystr;
pub mod pyvalue;
pub mod pyyaml;
pub mod report;
pub mod roofline;
pub mod roofline_cli;
mod roofline_selftest;
pub mod selftest;
pub mod stdout;
pub mod terminal;
mod unicode_tables;
pub mod utc;
pub mod verdict;
pub mod verdict_cli;
mod verdict_selftest;

mod repeat_floor {
    include!(concat!(env!("OUT_DIR"), "/full_repeats_min.rs"));
}

pub use error::SweepError;
pub use evaluate::{evaluate, Evaluation, StopReason};
pub use repeat_floor::FULL_REPEATS_MIN;
pub use utc::{parse_utc, UtcInstant};
