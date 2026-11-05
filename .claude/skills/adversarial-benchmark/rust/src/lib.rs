//! Broad Search stop-condition evaluator: a Rust port of
//! `.claude/skills/adversarial-benchmark/scripts/sweep_stop.py`.
//!
//! [`evaluate`] is the pure decision; [`cli::run`] is the program's command line. State files follow Python's
//! JSON and `datetime.fromisoformat` rules exactly (see [`pyjson`] and [`utc`]), because the files are written by
//! Python tools and the original's observable behaviour on odd input is part of the contract.

pub mod argv;
pub mod bench_cli;
pub mod bench_naming;
pub mod bench_preset;
pub mod bench_preset_api_cli;
pub mod bench_preset_cli;
mod bench_preset_selftest;
pub mod bench_reporting_api;
mod bench_selftest;
pub mod bench_tool_api;
pub mod bench_tool_cli;
pub mod bench_tool_pin;
mod bench_tool_selftest;
pub mod classify_cell;
pub mod classify_cell_api_cli;
pub mod classify_cell_cli;
pub mod classify_cell_owner;
mod classify_cell_selftest;
mod classify_cell_selftest_names;
pub mod cli;
pub mod dataset_commands;
pub mod dataset_prep;
pub mod dataset_prep_api;
pub mod dataset_prep_cli;
mod dataset_prep_selftest;
pub mod dataset_random;
pub mod engine_log;
pub mod error;
pub mod evaluate;
pub mod guidellm;
pub mod guidellm_cli;
mod guidellm_selftest;
mod help_layouts;
mod help_layouts_bench_preset;
mod help_layouts_classify_cell;
mod help_layouts_dataset_prep;
mod help_layouts_lite_metrics;
mod help_layouts_parse_bench;
mod help_layouts_parse_guidellm;
mod help_layouts_publish_benchmark_record;
mod help_layouts_render_max_report;
mod help_layouts_render_report;
mod help_layouts_render_sweep_map;
mod help_layouts_repeat_axis;
mod help_layouts_resolve_bench_tool;
mod help_layouts_roofline;
mod help_layouts_verdict_rule;
pub mod json_stream;
pub mod jsonl;
pub mod lite_engine_log;
pub mod lite_metrics;
pub mod lite_metrics_api_cli;
pub mod lite_metrics_cli;
mod lite_metrics_selftest;
pub mod ospath;
pub mod parse_bench;
pub mod publish_record;
pub mod pydyn;
pub mod pyformat;
pub mod pyfs;
pub mod pyjson;
pub mod pynumber;
pub mod pyops;
pub mod pysort;
pub mod pystr;
pub mod pyvalue;
pub mod pyyaml;
pub mod render_max_report;
pub mod render_report;
pub mod render_sweep_map;
pub mod repeat_api_cli;
pub mod repeat_axis;
pub mod repeat_cli;
pub mod repeat_fs;
pub mod repeat_owner;
mod repeat_selftest;
mod repeat_text;
pub mod report;
pub mod report_cli;
pub mod report_support;
pub mod reporting_cli;
mod reporting_selftest;
pub mod roofline;
pub mod roofline_cli;
mod roofline_selftest;
pub mod selftest;
pub mod stdout;
pub mod terminal;
mod unicode_digit_tables;
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
