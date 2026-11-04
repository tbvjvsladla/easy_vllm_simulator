//! The original program's command line, parsed the way CPython 3.12.3 `argparse` parses it.
//!
//! Ported from `Lib/argparse.py` (<https://github.com/python/cpython/blob/v3.12.3/Lib/argparse.py>, PSF License;
//! see `notices/NOTICE.md`) for parsers made of `-h/--help`, value options and flags only, no positionals,
//! abbreviations allowed. Each original program contributes a [`Spec`] table: `sweep_stop.py` ([`SWEEP_STOP`]:
//! `--state STATE`, `--now-utc NOW_UTC`, `--self-test`) and `parse_bench.py` ([`PARSE_BENCH`]: required
//! `--bench-json`, `--engine-log`, `--json`) and `parse_guidellm.py` ([`PARSE_GUIDELLM`]: spec-axis, health and
//! prefix-cache inputs, `--benchmark-index`, `--max-error-rate`). Option strings are tried in the order argparse
//! registered them, and arguments are code points (Python decodes argv with `surrogateescape`).
//!
//! Usage and help text are laid out by argparse from the terminal width. For a fixed parser that layout is a
//! step function of `COLUMNS`, captured from the original program into `help_layouts*` and looked up by
//! [`Spec::usage`] / [`Spec::help`]; widths beyond the last captured step share its layout. How the width is found is in
//! [`crate::terminal`].

use crate::help_layouts::Layout;
use crate::pynumber::{float_from_str, int_from_str};
use crate::pystr::{is_decimal_digit, PyStr};
use num_bigint::BigInt;

/// The program name argparse derives from `sys.argv[0]` of the original `sweep_stop.py`.
pub const PROG: &str = "sweep_stop.py";

/// How an argparse action consumes arguments (`store` with one value, `store_true`, or `help`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Help,
    Value,
    /// `action="append"`: a value option whose every occurrence is kept, in order.
    Append,
    Flag,
}

/// How `argparse` converts the text of a value option (`type=`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Conversion {
    Text,
    Float,
    Int,
}

/// One registered action. `name` is argparse's `_get_action_name`: the option strings joined with `/`.
#[derive(Clone, Copy, Debug)]
pub struct ActionDef {
    pub name: &'static str,
    pub kind: Kind,
    pub required: bool,
    pub conversion: Conversion,
    /// `choices=`: the allowed texts (empty when unrestricted).
    pub choices: &'static [&'static str],
}

/// An action that takes no restrictions; the base of the tables below.
const PLAIN: ActionDef = ActionDef {
    name: "",
    kind: Kind::Flag,
    required: false,
    conversion: Conversion::Text,
    choices: &[],
};

const fn flag(name: &'static str) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Flag,
        ..PLAIN
    }
}

const fn text_option(name: &'static str, required: bool) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        required,
        ..PLAIN
    }
}

const fn append_option(name: &'static str) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Append,
        ..PLAIN
    }
}

const fn number(name: &'static str, conversion: Conversion) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        conversion,
        ..PLAIN
    }
}

const fn choice(name: &'static str, choices: &'static [&'static str]) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        choices,
        ..PLAIN
    }
}

/// A fixed parser: its program name, actions, option strings in registration order and captured help layouts.
/// The generic part of the port is shared; each original program contributes only this table.
pub struct Spec {
    pub prog: &'static str,
    pub actions: &'static [ActionDef],
    /// Option strings in the order argparse registered them, each with the index of its action.
    pub options: &'static [(&'static str, usize)],
    layouts: &'static [Layout],
}

impl Spec {
    fn layout_for(&self, columns: u32) -> &'static Layout {
        self.layouts
            .iter()
            .rev()
            .find(|layout| layout.from_columns <= columns)
            .unwrap_or(&self.layouts[0])
    }

    /// `parser.format_usage()` at the given terminal width (in columns).
    pub fn usage(&self, columns: u32) -> &'static str {
        self.layout_for(columns).usage
    }

    /// `parser.format_help()` at the given terminal width (in columns).
    pub fn help(&self, columns: u32) -> &'static str {
        self.layout_for(columns).help
    }

    fn lookup(&self, option_string: &[u32]) -> Option<usize> {
        self.options
            .iter()
            .find(|(name, _)| equals(option_string, name))
            .map(|&(_, action)| action)
    }
}

const HELP_ACTION: usize = 0;
const STATE: usize = 1;
const NOW_UTC: usize = 2;
const SELF_TEST: usize = 3;

/// The original `sweep_stop.py` parser (`-h/--help` is added first by `ArgumentParser`).
pub const SWEEP_STOP: Spec = Spec {
    prog: PROG,
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--state", false),
        text_option("--now-utc", false),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--state", STATE),
        ("--now-utc", NOW_UTC),
        ("--self-test", SELF_TEST),
    ],
    layouts: crate::help_layouts::LAYOUTS,
};

/// Index of the `--bench-json` action in [`PARSE_BENCH`].
pub const BENCH_JSON: usize = 1;
/// Index of the `--engine-log` action in [`PARSE_BENCH`].
pub const ENGINE_LOG: usize = 2;
/// Index of the (accepted, unused) `--json` flag in [`PARSE_BENCH`].
pub const JSON_FLAG: usize = 3;

/// The original `parse_bench.py` parser.
pub const PARSE_BENCH: Spec = Spec {
    prog: "parse_bench.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--bench-json", true),
        text_option("--engine-log", false),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--bench-json", BENCH_JSON),
        ("--engine-log", ENGINE_LOG),
        ("--json", JSON_FLAG),
    ],
    layouts: crate::help_layouts_parse_bench::LAYOUTS,
};

/// Indices of the `lite_metrics.py` actions.
pub mod lite_args {
    pub const RAW_JSON: usize = 1;
    pub const JSON: usize = 2;
    pub const JUDGE: usize = 3;
}

/// The original `lite_metrics.py` parser: required `--raw-json`, the flags `--json` and `--judge`.
pub const LITE_METRICS: Spec = Spec {
    prog: "lite_metrics.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--raw-json", true),
        flag("--json"),
        flag("--judge"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--raw-json", lite_args::RAW_JSON),
        ("--json", lite_args::JSON),
        ("--judge", lite_args::JUDGE),
    ],
    layouts: crate::help_layouts_lite_metrics::LAYOUTS,
};

/// Indices of the `classify_cell.py` actions.
pub mod classify_args {
    pub const SERVE_RC: usize = 1;
    pub const MEASURE_RC: usize = 2;
    pub const STARTED_UTC: usize = 3;
    pub const ENDED_UTC: usize = 4;
    pub const EVENTS: usize = 5;
    pub const EVENTS_FROM_REPO: usize = 6;
    pub const TOPOLOGY: usize = 7;
    pub const MANIFEST: usize = 8;
    pub const TOLERANCE_S: usize = 9;
    pub const CROSS_NODE_TOLERANCE_S: usize = 10;
    pub const CROSS_NODE_TOLERANCE_SOURCE: usize = 11;
    pub const SWEEP_INDEX: usize = 12;
    pub const WRITE_BENCH_MODE: usize = 13;
    pub const LITE_RAW: usize = 14;
    pub const JSON: usize = 15;
    pub const SELF_TEST: usize = 16;
}

/// The original `classify_cell.py` parser: the cell-closing and bench-mode options, every one optional (the program
/// itself decides which are required, after parsing).
pub const CLASSIFY_CELL: Spec = Spec {
    prog: "classify_cell.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        number("--serve-rc", Conversion::Int),
        text_option("--measure-rc", false),
        text_option("--started-utc", false),
        text_option("--ended-utc", false),
        append_option("--events"),
        text_option("--events-from-repo", false),
        choice("--topology", &["single", "multi"]),
        text_option("--manifest", false),
        number("--tolerance-s", Conversion::Int),
        number("--cross-node-tolerance-s", Conversion::Int),
        text_option("--cross-node-tolerance-source", false),
        text_option("--sweep-index", false),
        flag("--write-bench-mode"),
        text_option("--lite-raw", false),
        flag("--json"),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--serve-rc", classify_args::SERVE_RC),
        ("--measure-rc", classify_args::MEASURE_RC),
        ("--started-utc", classify_args::STARTED_UTC),
        ("--ended-utc", classify_args::ENDED_UTC),
        ("--events", classify_args::EVENTS),
        ("--events-from-repo", classify_args::EVENTS_FROM_REPO),
        ("--topology", classify_args::TOPOLOGY),
        ("--manifest", classify_args::MANIFEST),
        ("--tolerance-s", classify_args::TOLERANCE_S),
        (
            "--cross-node-tolerance-s",
            classify_args::CROSS_NODE_TOLERANCE_S,
        ),
        (
            "--cross-node-tolerance-source",
            classify_args::CROSS_NODE_TOLERANCE_SOURCE,
        ),
        ("--sweep-index", classify_args::SWEEP_INDEX),
        ("--write-bench-mode", classify_args::WRITE_BENCH_MODE),
        ("--lite-raw", classify_args::LITE_RAW),
        ("--json", classify_args::JSON),
        ("--self-test", classify_args::SELF_TEST),
    ],
    layouts: crate::help_layouts_classify_cell::LAYOUTS,
};

/// Indices of the `parse_guidellm.py` actions.
pub mod guidellm_args {
    pub const BENCHMARKS_JSON: usize = 1;
    pub const ENGINE_LOG: usize = 2;
    pub const BENCHMARK_INDEX: usize = 3;
    pub const ACCEPT_LEN_SRC: usize = 4;
    pub const SPEC_AXIS_ABSENT: usize = 5;
    pub const POST_HEALTH_JSON: usize = 6;
    pub const PREFIX_CACHE_JSON: usize = 7;
    pub const MAX_ERROR_RATE: usize = 8;
    pub const JSON: usize = 9;
    pub const SELF_TEST: usize = 10;
}

/// The original `parse_guidellm.py` parser.
pub const PARSE_GUIDELLM: Spec = Spec {
    prog: "parse_guidellm.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--benchmarks-json", false),
        text_option("--engine-log", false),
        number("--benchmark-index", Conversion::Int),
        text_option("--accept-len-src", false),
        flag("--spec-axis-absent"),
        text_option("--post-health-json", false),
        text_option("--prefix-cache-json", false),
        number("--max-error-rate", Conversion::Float),
        flag("--json"),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--benchmarks-json", 1),
        ("--engine-log", 2),
        ("--benchmark-index", 3),
        ("--accept-len-src", 4),
        ("--spec-axis-absent", 5),
        ("--post-health-json", 6),
        ("--prefix-cache-json", 7),
        ("--max-error-rate", 8),
        ("--json", 9),
        ("--self-test", 10),
    ],
    layouts: crate::help_layouts_parse_guidellm::LAYOUTS,
};

/// Indices of the `roofline.py` actions.
pub mod roofline_args {
    pub const MODEL_PATH: usize = 1;
    pub const NAS_ROOT: usize = 2;
    pub const QUANT_ROOT: usize = 3;
    pub const MANIFEST: usize = 4;
    pub const TP: usize = 5;
    pub const ACCEPT_LEN: usize = 6;
    pub const ACCEPT_LEN_SOURCE: usize = 7;
    pub const ACCEPT_LEN_EVIDENCE: usize = 8;
    pub const BANDWIDTH_GBPS: usize = 9;
    pub const INTERCONNECT_GBPS: usize = 10;
    pub const ACT_DTYPE_BYTES: usize = 11;
    pub const COMM_LATENCY_US_PER_LAYER: usize = 12;
    pub const REALISTIC_FRACTION: usize = 13;
    pub const JSON: usize = 14;
}

/// The `accept_len` provenance vocabulary (`roofline.ACCEPT_LEN_SOURCES`); one list for the parser and the library.
pub const ACCEPT_LEN_SOURCE_NAMES: &[&str] = &["measured", "declared", "declared-absent", "absent"];

/// The original `roofline.py` parser.
pub const ROOFLINE: Spec = Spec {
    prog: "roofline.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--model-path", true),
        text_option("--nas-root", false),
        text_option("--quant-root", false),
        text_option("--manifest", false),
        number("--tp", Conversion::Int),
        number("--accept-len", Conversion::Float),
        choice("--accept-len-source", ACCEPT_LEN_SOURCE_NAMES),
        text_option("--accept-len-evidence", false),
        number("--bandwidth-gbps", Conversion::Float),
        number("--interconnect-gbps", Conversion::Float),
        number("--act-dtype-bytes", Conversion::Int),
        number("--comm-latency-us-per-layer", Conversion::Float),
        number("--realistic-fraction", Conversion::Float),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--model-path", 1),
        ("--nas-root", 2),
        ("--quant-root", 3),
        ("--manifest", 4),
        ("--tp", 5),
        ("--accept-len", 6),
        ("--accept-len-source", 7),
        ("--accept-len-evidence", 8),
        ("--bandwidth-gbps", 9),
        ("--interconnect-gbps", 10),
        ("--act-dtype-bytes", 11),
        ("--comm-latency-us-per-layer", 12),
        ("--realistic-fraction", 13),
        ("--json", 14),
    ],
    layouts: crate::help_layouts_roofline::LAYOUTS,
};

/// Indices of the `verdict_rule.py` actions.
pub mod verdict_args {
    pub const MEASURED: usize = 1;
    pub const ROOFLINE: usize = 2;
    pub const REFERENCE_TPS: usize = 3;
    pub const TARGET_TPS: usize = 4;
    pub const AUTHORITY: usize = 5;
    pub const TOLERANCE: usize = 6;
    pub const SPEC_SUPPORTED: usize = 7;
    pub const SPEC_DECLARED: usize = 8;
    pub const E_SEARCH: usize = 9;
    pub const NODE_VRAM_GIB: usize = 10;
    pub const BALANCE_TOL: usize = 11;
    pub const SELF_TEST: usize = 12;
    pub const JSON: usize = 13;
}

pub const AUTHORITY_NAMES: &[&str] = &["weak", "explicit", "explore"];
pub const SPEC_DECLARED_NAMES: &[&str] = &["on", "off", "unknown"];
pub const E_SEARCH_NAMES: &[&str] = &["hit", "empty", "no"];

/// The original `verdict_rule.py` parser.
pub const VERDICT_RULE: Spec = Spec {
    prog: "verdict_rule.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--measured", false),
        text_option("--roofline", false),
        number("--reference-tps", Conversion::Float),
        number("--target-tps", Conversion::Float),
        choice("--authority", AUTHORITY_NAMES),
        number("--tolerance", Conversion::Float),
        flag("--spec-supported"),
        choice("--spec-declared", SPEC_DECLARED_NAMES),
        choice("--e-search", E_SEARCH_NAMES),
        text_option("--node-vram-gib", false),
        number("--balance-tol", Conversion::Float),
        flag("--self-test"),
        flag("--json"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--measured", 1),
        ("--roofline", 2),
        ("--reference-tps", 3),
        ("--target-tps", 4),
        ("--authority", 5),
        ("--tolerance", 6),
        ("--spec-supported", 7),
        ("--spec-declared", 8),
        ("--e-search", 9),
        ("--node-vram-gib", 10),
        ("--balance-tol", 11),
        ("--self-test", 12),
        ("--json", 13),
    ],
    layouts: crate::help_layouts_verdict_rule::LAYOUTS,
};

/// Indices of the actions of the four `repeat_axis.py` parsers (the top parser and its three subparsers).
pub mod repeat_axis_args {
    pub const SELF_TEST: usize = 1;

    pub mod resolve {
        pub const REPEATS: usize = 1;
        pub const REPEATS_SOURCE: usize = 2;
        pub const CAMPAIGN_ID: usize = 3;
        pub const PRESET_DEFAULT: usize = 4;
        pub const PRESET_SOURCE: usize = 5;
    }

    pub mod record_run {
        pub const RUN_DIR: usize = 1;
        pub const LEVEL: usize = 2;
        pub const RUN: usize = 3;
        pub const STARTED_UTC: usize = 4;
        pub const ENDED_UTC: usize = 5;
        pub const RUN_BENCH_RC: usize = 6;
        pub const PARSE_RC: usize = 7;
    }

    pub mod aggregate_level {
        pub const LEVEL_DIR: usize = 1;
        pub const REQUESTED: usize = 2;
        pub const REQUESTED_SOURCE: usize = 3;
        pub const KIND: usize = 4;
    }
}

const fn required_option(name: &'static str, conversion: Conversion) -> ActionDef {
    ActionDef {
        name,
        kind: Kind::Value,
        required: true,
        conversion,
        choices: &[],
    }
}

const HELP_DEFINITION: ActionDef = ActionDef {
    name: "-h/--help",
    kind: Kind::Help,
    required: false,
    conversion: Conversion::Text,
    choices: &[],
};

/// The top parser of `repeat_axis.py`: `-h`, `--self-test` and the subcommand word, which [`crate::repeat_cli`]
/// dispatches (this table covers the options before it).
pub const REPEAT_AXIS: Spec = Spec {
    prog: "repeat_axis.py",
    actions: &[HELP_DEFINITION, flag("--self-test")],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--self-test", 1),
    ],
    layouts: crate::help_layouts_repeat_axis::TOP,
};

/// The `resolve` subparser.
pub const REPEAT_AXIS_RESOLVE: Spec = Spec {
    prog: "repeat_axis.py resolve",
    actions: &[
        HELP_DEFINITION,
        text_option("--repeats", false),
        text_option("--repeats-source", false),
        text_option("--campaign-id", false),
        text_option("--preset-default", false),
        text_option("--preset-source", false),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--repeats", 1),
        ("--repeats-source", 2),
        ("--campaign-id", 3),
        ("--preset-default", 4),
        ("--preset-source", 5),
    ],
    layouts: crate::help_layouts_repeat_axis::RESOLVE,
};

/// The `record-run` subparser.
pub const REPEAT_AXIS_RECORD_RUN: Spec = Spec {
    prog: "repeat_axis.py record-run",
    actions: &[
        HELP_DEFINITION,
        text_option("--run-dir", true),
        required_option("--level", Conversion::Int),
        required_option("--run", Conversion::Int),
        text_option("--started-utc", true),
        text_option("--ended-utc", true),
        required_option("--run-bench-rc", Conversion::Int),
        required_option("--parse-rc", Conversion::Int),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--run-dir", 1),
        ("--level", 2),
        ("--run", 3),
        ("--started-utc", 4),
        ("--ended-utc", 5),
        ("--run-bench-rc", 6),
        ("--parse-rc", 7),
    ],
    layouts: crate::help_layouts_repeat_axis::RECORD_RUN,
};

/// The `aggregate-level` subparser (`--kind` defaults to `warm-rerun` in the program, not here).
pub const REPEAT_AXIS_AGGREGATE_LEVEL: Spec = Spec {
    prog: "repeat_axis.py aggregate-level",
    actions: &[
        HELP_DEFINITION,
        text_option("--level-dir", true),
        required_option("--requested", Conversion::Int),
        text_option("--requested-source", true),
        text_option("--kind", false),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--level-dir", 1),
        ("--requested", 2),
        ("--requested-source", 3),
        ("--kind", 4),
    ],
    layouts: crate::help_layouts_repeat_axis::AGGREGATE_LEVEL,
};

/// Indices of the actions of the four `bench_preset.py` parsers (the top parser and its three subparsers).
pub mod bench_preset_args {
    pub const SELF_TEST: usize = 1;

    pub mod validate {
        pub const PRESET: usize = 1;
    }

    pub mod default {
        pub const KNOB: usize = 1;
        pub const PRESET: usize = 2;
    }

    pub mod resolve {
        pub const TOOL: usize = 1;
        pub const PRESET: usize = 2;
        pub const SERVING_YAML: usize = 3;
        pub const DRY_RUN: usize = 4;
        pub const RECORD_OUT: usize = 5;
        pub const LEVELS: usize = 6;
        pub const INPUT_LEN: usize = 7;
        pub const OUTPUT_LEN: usize = 8;
        pub const NUM_PROMPTS: usize = 9;
        pub const WARMUPS: usize = 10;
        pub const REPEATS: usize = 11;
        pub const REPEATS_SOURCE: usize = 12;
        pub const BACKEND: usize = 13;
        pub const BENCH_BUDGET_MIB: usize = 14;
        pub const PIN: usize = 15;
        pub const PINNED_BY: usize = 16;
        pub const KNOB_REASON: usize = 17;
        pub const APPROVED_BY: usize = 18;
        pub const CAMPAIGN_ID: usize = 19;
    }
}

/// The top parser of `bench_preset.py`: `-h`, `--self-test` and the subcommand word, which
/// [`crate::bench_preset_cli`] dispatches.
pub const BENCH_PRESET: Spec = Spec {
    prog: "bench_preset.py",
    actions: &[HELP_DEFINITION, flag("--self-test")],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--self-test", 1),
    ],
    layouts: crate::help_layouts_bench_preset::TOP,
};

/// The `validate` subparser.
pub const BENCH_PRESET_VALIDATE: Spec = Spec {
    prog: "bench_preset.py validate",
    actions: &[HELP_DEFINITION, text_option("--preset", false)],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--preset", 1),
    ],
    layouts: crate::help_layouts_bench_preset::VALIDATE,
};

/// The `default` subparser.
pub const BENCH_PRESET_DEFAULT: Spec = Spec {
    prog: "bench_preset.py default",
    actions: &[
        HELP_DEFINITION,
        required_option("--knob", Conversion::Text),
        text_option("--preset", false),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--knob", 1),
        ("--preset", 2),
    ],
    layouts: crate::help_layouts_bench_preset::DEFAULT,
};

/// The `resolve` subparser.
pub const BENCH_PRESET_RESOLVE: Spec = Spec {
    prog: "bench_preset.py resolve",
    actions: &[
        HELP_DEFINITION,
        required_option("--tool", Conversion::Text),
        text_option("--preset", false),
        text_option("--serving-yaml", false),
        flag("--dry-run"),
        required_option("--record-out", Conversion::Text),
        text_option("--levels", false),
        text_option("--input-len", false),
        text_option("--output-len", false),
        text_option("--num-prompts", false),
        text_option("--warmups", false),
        text_option("--repeats", false),
        text_option("--repeats-source", false),
        text_option("--backend", false),
        text_option("--bench-budget-mib", false),
        append_option("--pin"),
        text_option("--pinned-by", false),
        append_option("--knob-reason"),
        text_option("--approved-by", false),
        text_option("--campaign-id", false),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--tool", 1),
        ("--preset", 2),
        ("--serving-yaml", 3),
        ("--dry-run", 4),
        ("--record-out", 5),
        ("--levels", 6),
        ("--input-len", 7),
        ("--output-len", 8),
        ("--num-prompts", 9),
        ("--warmups", 10),
        ("--repeats", 11),
        ("--repeats-source", 12),
        ("--backend", 13),
        ("--bench-budget-mib", 14),
        ("--pin", 15),
        ("--pinned-by", 16),
        ("--knob-reason", 17),
        ("--approved-by", 18),
        ("--campaign-id", 19),
    ],
    layouts: crate::help_layouts_bench_preset::RESOLVE,
};

/// `parser.format_usage()` of `sweep_stop.py` at the given terminal width.
pub fn usage(columns: u32) -> &'static str {
    SWEEP_STOP.usage(columns)
}

/// `parser.format_help()` of `sweep_stop.py` at the given terminal width.
pub fn help(columns: u32) -> &'static str {
    SWEEP_STOP.help(columns)
}

/// The values a successful parse produced, indexed by action: the last value given, and whether it was seen.
#[derive(Debug)]
pub struct Matches {
    values: Vec<Option<PyStr>>,
    /// Every value of an `append` action, in command-line order.
    appended: Vec<Vec<PyStr>>,
    seen: Vec<bool>,
    numbers: Vec<Option<ArgNumber>>,
}

/// The converted value of a `type=float` / `type=int` option.
#[derive(Clone, Debug, PartialEq)]
pub enum ArgNumber {
    Float(f64),
    Int(BigInt),
}

impl Matches {
    fn new(spec: &Spec) -> Self {
        Matches {
            values: vec![None; spec.actions.len()],
            appended: vec![Vec::new(); spec.actions.len()],
            seen: vec![false; spec.actions.len()],
            numbers: vec![None; spec.actions.len()],
        }
    }

    /// The value of a `Kind::Value` action, if given.
    pub fn value(&self, action: usize) -> Option<&PyStr> {
        self.values[action].as_ref()
    }

    /// Every value an `append` action collected, in command-line order (empty when it was not given).
    pub fn values_of(&self, action: usize) -> &[PyStr] {
        &self.appended[action]
    }

    /// The converted value of a `type=float` option, if given.
    pub fn float(&self, action: usize) -> Option<f64> {
        match self.numbers[action] {
            Some(ArgNumber::Float(number)) => Some(number),
            _ => None,
        }
    }

    /// The converted value of a `type=int` option, if given.
    pub fn int(&self, action: usize) -> Option<&BigInt> {
        match &self.numbers[action] {
            Some(ArgNumber::Int(number)) => Some(number),
            _ => None,
        }
    }

    /// Whether a `Kind::Flag` action was given.
    pub fn flag(&self, action: usize) -> bool {
        self.seen[action]
    }
}

#[derive(Debug)]
pub enum Outcome {
    Run(Matches),
    /// `-h` was reached before any error: print help and exit 0.
    Help,
    /// `parser.error(message)`: usage and `prog: error: message` on stderr, exit 2.
    Error(PyStr),
}

#[derive(Debug, Default)]
pub struct LegacyArgs {
    pub state: Option<PyStr>,
    pub now_utc: Option<PyStr>,
    pub self_test: bool,
}

#[derive(Debug)]
pub enum Parsed {
    Run(LegacyArgs),
    /// `-h` was reached before any error: print help and exit 0.
    Help,
    /// `parser.error(message)`: usage and `prog: error: message` on stderr, exit 2.
    Error(PyStr),
}

type Points = Vec<u32>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Separator {
    Absent,
    /// A short option followed directly by more characters (`-hx`): argparse's empty string.
    Joined,
    Equals,
}

struct OptionTuple {
    action: Option<usize>,
    option_string: Points,
    separator: Separator,
    explicit: Option<Points>,
}

fn points(text: &str) -> Points {
    text.chars().map(u32::from).collect()
}

fn equals(candidate: &[u32], text: &str) -> bool {
    candidate.iter().copied().eq(text.chars().map(u32::from))
}

fn starts_with(candidate: &[u32], prefix: &[u32]) -> bool {
    candidate.starts_with(prefix)
}

fn text(parts: &[&[u32]]) -> PyStr {
    PyStr::from_code_points(parts.concat())
}

fn quoted(value: &[u32]) -> String {
    PyStr::from_code_points(value.to_vec()).repr()
}

/// `^-\d+$|^-\d*\.\d+$` with Unicode digits; `$` also matches before one trailing newline.
fn is_negative_number(arg: &[u32]) -> bool {
    let candidates = [arg, arg.strip_suffix(&[0x0A]).unwrap_or(arg)];
    candidates.iter().any(|body| {
        let Some(rest) = body.strip_prefix(&[u32::from('-')]) else {
            return false;
        };
        let digits = |slice: &[u32]| slice.iter().all(|&c| is_decimal_digit(c));
        match rest.iter().position(|&c| c == u32::from('.')) {
            None => !rest.is_empty() && digits(rest),
            Some(dot) => {
                digits(&rest[..dot]) && !rest[dot + 1..].is_empty() && digits(&rest[dot + 1..])
            }
        }
    })
}

/// `_get_option_tuples` for an argument that starts with `-` and is at least two characters long.
fn option_tuples(spec: &Spec, arg: &[u32]) -> Vec<OptionTuple> {
    let dash = u32::from('-');
    let mut result = Vec::new();
    if arg[1] == dash {
        let (prefix, separator, explicit) = match arg.iter().position(|&c| c == u32::from('=')) {
            Some(at) => (&arg[..at], Separator::Equals, Some(arg[at + 1..].to_vec())),
            None => (arg, Separator::Absent, None),
        };
        for &(name, action) in spec.options {
            if starts_with(&points(name), prefix) {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: points(name),
                    separator,
                    explicit: explicit.clone(),
                });
            }
        }
    } else {
        let (short_prefix, short_explicit) = arg.split_at(2);
        for &(name, action) in spec.options {
            let name_points = points(name);
            if name_points == short_prefix {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: name_points,
                    separator: Separator::Joined,
                    explicit: Some(short_explicit.to_vec()),
                });
            } else if starts_with(&name_points, arg) {
                result.push(OptionTuple {
                    action: Some(action),
                    option_string: name_points,
                    separator: Separator::Absent,
                    explicit: None,
                });
            }
        }
    }
    result
}

/// `_parse_optional`: `Ok(None)` means the argument is positional-like. An ambiguous prefix is a parser error.
fn parse_optional(spec: &Spec, arg: &[u32]) -> Result<Option<OptionTuple>, PyStr> {
    if arg.first() != Some(&u32::from('-')) {
        return Ok(None);
    }
    if let Some(action) = spec.lookup(arg) {
        return Ok(Some(OptionTuple {
            action: Some(action),
            option_string: arg.to_vec(),
            separator: Separator::Absent,
            explicit: None,
        }));
    }
    if arg.len() == 1 {
        return Ok(None);
    }
    if let Some(at) = arg.iter().position(|&c| c == u32::from('=')) {
        if let Some(action) = spec.lookup(&arg[..at]) {
            return Ok(Some(OptionTuple {
                action: Some(action),
                option_string: arg[..at].to_vec(),
                separator: Separator::Equals,
                explicit: Some(arg[at + 1..].to_vec()),
            }));
        }
    }
    let mut candidates = option_tuples(spec, arg);
    if candidates.len() > 1 {
        let names: Vec<String> = candidates
            .iter()
            .map(|tuple| {
                PyStr::from_code_points(tuple.option_string.clone())
                    .to_utf8()
                    .unwrap_or_default()
            })
            .collect();
        let joined = points(&names.join(", "));
        return Err(text(&[
            &points("ambiguous option: ")[..],
            arg,
            &points(" could match ")[..],
            &joined[..],
        ]));
    }
    if let Some(only) = candidates.pop() {
        return Ok(Some(only));
    }
    if is_negative_number(arg) || arg.contains(&u32::from(' ')) {
        return Ok(None);
    }
    Ok(Some(OptionTuple {
        action: None,
        option_string: arg.to_vec(),
        separator: Separator::Absent,
        explicit: None,
    }))
}

fn argument_error(spec: &Spec, action: usize, message: &str) -> PyStr {
    PyStr::new(&format!(
        "argument {}: {message}",
        spec.actions[action].name
    ))
}

/// What `consume_optional` decided for one option argument.
enum Consumed {
    Next(usize),
    Help,
    Error(PyStr),
}

/// Parses the arguments of the original `sweep_stop.py` (without the program name).
pub fn parse(args: &[PyStr]) -> Parsed {
    match parse_with(&SWEEP_STOP, args) {
        Outcome::Help => Parsed::Help,
        Outcome::Error(message) => Parsed::Error(message),
        Outcome::Run(matches) => Parsed::Run(LegacyArgs {
            state: matches.values[STATE].clone(),
            now_utc: matches.values[NOW_UTC].clone(),
            self_test: matches.flag(SELF_TEST),
        }),
    }
}

/// Parses `args` (without the program name) the way `ArgumentParser.parse_args` of the given parser does.
pub fn parse_with(spec: &Spec, args: &[PyStr]) -> Outcome {
    let args: Vec<&[u32]> = args.iter().map(PyStr::code_points).collect();
    let mut pattern: Vec<char> = Vec::with_capacity(args.len());
    let mut tuples: Vec<Option<OptionTuple>> = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        if equals(args[index], "--") {
            pattern.push('-');
            tuples.push(None);
            for _ in index + 1..args.len() {
                pattern.push('A');
                tuples.push(None);
            }
            break;
        }
        match parse_optional(spec, args[index]) {
            Err(message) => return Outcome::Error(message),
            Ok(None) => {
                pattern.push('A');
                tuples.push(None);
            }
            Ok(Some(tuple)) => {
                pattern.push('O');
                tuples.push(Some(tuple));
            }
        }
        index += 1;
    }

    let mut parsed = Matches::new(spec);
    let mut extras: Vec<PyStr> = Vec::new();
    let last_option = tuples.iter().rposition(Option::is_some);
    let mut start = 0;
    while last_option.is_some_and(|last| start <= last) {
        let next = (start..tuples.len())
            .find(|&i| tuples[i].is_some())
            .expect("an option remains");
        extras.extend(
            args[start..next]
                .iter()
                .map(|a| PyStr::from_code_points(a.to_vec())),
        );
        start = next;
        match consume_optional(
            spec,
            start,
            &args,
            &pattern,
            &mut tuples,
            &mut extras,
            &mut parsed,
        ) {
            Consumed::Next(stop) => start = stop,
            Consumed::Help => return Outcome::Help,
            Consumed::Error(message) => return Outcome::Error(message),
        }
    }
    extras.extend(
        args[start..]
            .iter()
            .map(|a| PyStr::from_code_points(a.to_vec())),
    );
    // argparse checks required actions before `parse_args` looks at the leftover arguments.
    let missing: Vec<&str> = spec
        .actions
        .iter()
        .enumerate()
        .filter(|(index, action)| action.required && !parsed.seen[*index])
        .map(|(_, action)| action.name)
        .collect();
    if !missing.is_empty() {
        return Outcome::Error(PyStr::new(&format!(
            "the following arguments are required: {}",
            missing.join(", ")
        )));
    }
    if extras.is_empty() {
        return Outcome::Run(parsed);
    }
    let mut message = PyStr::new("unrecognized arguments: ");
    for (position, extra) in extras.iter().enumerate() {
        if position > 0 {
            message.push_str(" ");
        }
        message.push_py(extra);
    }
    Outcome::Error(message)
}

/// `_get_values` for a value option: the `type=` conversion, then the `choices` check.
fn convert_value(definition: &ActionDef, text: &PyStr) -> Result<Option<ArgNumber>, PyStr> {
    let invalid = |kind: &str| {
        PyStr::new(&format!(
            "argument {}: invalid {kind} value: {}",
            definition.name,
            text.repr()
        ))
    };
    let number = match definition.conversion {
        Conversion::Text => None,
        Conversion::Float => Some(ArgNumber::Float(
            float_from_str(text).map_err(|_| invalid("float"))?,
        )),
        Conversion::Int => Some(ArgNumber::Int(
            int_from_str(text).map_err(|_| invalid("int"))?,
        )),
    };
    if !definition.choices.is_empty()
        && !definition.choices.iter().any(|choice| text.equals(choice))
    {
        let allowed: Vec<String> = definition
            .choices
            .iter()
            .map(|choice| PyStr::new(choice).repr())
            .collect();
        return Err(PyStr::new(&format!(
            "argument {}: invalid choice: {} (choose from {})",
            definition.name,
            text.repr(),
            allowed.join(", ")
        )));
    }
    Ok(number)
}

fn consume_optional(
    spec: &Spec,
    start: usize,
    args: &[&[u32]],
    pattern: &[char],
    tuples: &mut [Option<OptionTuple>],
    extras: &mut Vec<PyStr>,
    parsed: &mut Matches,
) -> Consumed {
    let tuple = tuples[start].take().expect("start indexes an option");
    let (mut action, mut option_string, mut separator, mut explicit) = (
        tuple.action,
        tuple.option_string,
        tuple.separator,
        tuple.explicit,
    );
    let mut queued: Vec<(usize, Vec<PyStr>)> = Vec::new();
    let stop;
    loop {
        let Some(current) = action else {
            extras.push(PyStr::from_code_points(args[start].to_vec()));
            return Consumed::Next(start + 1);
        };
        if let Some(value) = explicit.clone() {
            let takes = usize::from(matches!(
                spec.actions[current].kind,
                Kind::Value | Kind::Append
            ));
            if takes == 0 && option_string[1] != u32::from('-') && !value.is_empty() {
                if separator == Separator::Equals || value[0] == u32::from('-') {
                    return Consumed::Error(argument_error(
                        spec,
                        current,
                        &format!("ignored explicit argument {}", quoted(&value)),
                    ));
                }
                queued.push((current, Vec::new()));
                let dash = option_string[0];
                option_string = vec![dash, value[0]];
                if let Some(next_action) = spec.lookup(&option_string) {
                    action = Some(next_action);
                    let rest = value[1..].to_vec();
                    if rest.is_empty() {
                        separator = Separator::Absent;
                        explicit = None;
                    } else if rest[0] == u32::from('=') {
                        separator = Separator::Equals;
                        explicit = Some(rest[1..].to_vec());
                    } else {
                        separator = Separator::Joined;
                        explicit = Some(rest);
                    }
                } else {
                    extras.push(PyStr::from_code_points([&[dash][..], &value].concat()));
                    stop = start + 1;
                    break;
                }
            } else if takes == 1 {
                stop = start + 1;
                queued.push((current, vec![PyStr::from_code_points(value)]));
                break;
            } else {
                return Consumed::Error(argument_error(
                    spec,
                    current,
                    &format!("ignored explicit argument {}", quoted(&value)),
                ));
            }
        } else {
            let wanted = usize::from(matches!(
                spec.actions[current].kind,
                Kind::Value | Kind::Append
            ));
            if wanted == 1 && pattern.get(start + 1) != Some(&'A') {
                return Consumed::Error(argument_error(spec, current, "expected one argument"));
            }
            stop = start + 1 + wanted;
            let values = args[start + 1..stop]
                .iter()
                .map(|a| PyStr::from_code_points(a.to_vec()))
                .collect();
            queued.push((current, values));
            break;
        }
    }
    for (action, mut values) in queued {
        let definition = &spec.actions[action];
        match definition.kind {
            Kind::Help => return Consumed::Help,
            Kind::Value | Kind::Append => {
                let value = values.pop();
                if let Some(text) = &value {
                    match convert_value(definition, text) {
                        Ok(number) => parsed.numbers[action] = number,
                        Err(message) => return Consumed::Error(message),
                    }
                    if definition.kind == Kind::Append {
                        parsed.appended[action].push(text.clone());
                    }
                }
                parsed.values[action] = value;
            }
            Kind::Flag => {}
        }
        parsed.seen[action] = true;
    }
    Consumed::Next(stop)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_strs(args: &[&str]) -> Parsed {
        parse(&args.iter().map(|a| PyStr::new(a)).collect::<Vec<_>>())
    }

    fn error_of(args: &[&str]) -> String {
        match parse_strs(args) {
            Parsed::Error(message) => message.to_utf8().unwrap(),
            Parsed::Help => "HELP".to_string(),
            Parsed::Run(_) => "RUN".to_string(),
        }
    }

    #[test]
    fn terminator_keeps_the_dash_dash_and_everything_after_it_as_extras() {
        assert_eq!(
            error_of(&["--", "--help"]),
            "unrecognized arguments: -- --help"
        );
        assert_eq!(
            error_of(&["--state", "-", "--now-utc", "x", "--", "--self-test"]),
            "unrecognized arguments: -- --self-test"
        );
    }

    #[test]
    fn explicit_argument_rules_for_flags() {
        assert_eq!(
            error_of(&["--help=unexpected"]),
            "argument -h/--help: ignored explicit argument 'unexpected'"
        );
        assert_eq!(
            error_of(&["--self-test=true"]),
            "argument --self-test: ignored explicit argument 'true'"
        );
        assert_eq!(error_of(&["-hh"]), "HELP");
        assert_eq!(error_of(&["-hbad"]), "HELP");
        assert_eq!(
            error_of(&["--s"]),
            "ambiguous option: --s could match --state, --self-test"
        );
    }

    #[test]
    fn values_and_negative_numbers() {
        assert!(matches!(
            parse_strs(&["--state", "-5", "--now-utc", "x"]),
            Parsed::Run(_)
        ));
        assert!(
            matches!(
                parse_strs(&["--state", "-\u{661}", "--now-utc", "x"]),
                Parsed::Run(_)
            ),
            "Arabic-Indic digits are digits"
        );
        assert_eq!(
            error_of(&["--state"]),
            "argument --state: expected one argument"
        );
        assert_eq!(
            error_of(&["--state", "--", "x"]),
            "argument --state: expected one argument"
        );
    }
}

/// Indices of the `render_report.py` actions.
pub mod render_report_args {
    pub const SWEEP_INDEX: usize = 1;
    pub const VERDICT_JSON: usize = 2;
    pub const LITE_ONLY: usize = 3;
    pub const LITE_RAW_JSON: usize = 4;
    pub const ROOFLINE_JSON: usize = 5;
    pub const BENCH_MODE_JSON: usize = 6;
    pub const OUT_DIR: usize = 7;
    pub const STDOUT: usize = 8;
}

/// The original `render_report.py` parser.
pub const RENDER_REPORT: Spec = Spec {
    prog: "render_report.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--sweep-index", false),
        text_option("--verdict-json", false),
        flag("--lite-only"),
        text_option("--lite-raw-json", false),
        text_option("--roofline-json", false),
        text_option("--bench-mode-json", false),
        text_option("--out-dir", false),
        flag("--stdout"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--sweep-index", render_report_args::SWEEP_INDEX),
        ("--verdict-json", render_report_args::VERDICT_JSON),
        ("--lite-only", render_report_args::LITE_ONLY),
        ("--lite-raw-json", render_report_args::LITE_RAW_JSON),
        ("--roofline-json", render_report_args::ROOFLINE_JSON),
        ("--bench-mode-json", render_report_args::BENCH_MODE_JSON),
        ("--out-dir", render_report_args::OUT_DIR),
        ("--stdout", render_report_args::STDOUT),
    ],
    layouts: crate::help_layouts_render_report::LAYOUTS,
};

/// Indices of the `render_sweep_map.py` actions.
pub mod sweep_map_args {
    pub const STATE: usize = 1;
    pub const STOP_JSON: usize = 2;
    pub const OUT_MD: usize = 3;
    pub const OUT_JSON: usize = 4;
    pub const SELF_TEST: usize = 5;
}

/// The original `render_sweep_map.py` parser.
pub const RENDER_SWEEP_MAP: Spec = Spec {
    prog: "render_sweep_map.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--state", false),
        text_option("--stop-json", false),
        text_option("--out-md", false),
        text_option("--out-json", false),
        flag("--self-test"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--state", sweep_map_args::STATE),
        ("--stop-json", sweep_map_args::STOP_JSON),
        ("--out-md", sweep_map_args::OUT_MD),
        ("--out-json", sweep_map_args::OUT_JSON),
        ("--self-test", sweep_map_args::SELF_TEST),
    ],
    layouts: crate::help_layouts_render_sweep_map::LAYOUTS,
};

/// Indices of the `render_max_report.py` actions.
pub mod max_report_args {
    pub const MAX_INDEX: usize = 1;
    pub const OUT_DIR: usize = 2;
    pub const STDOUT: usize = 3;
}

/// The original `render_max_report.py` parser (`--max-index` is required).
pub const RENDER_MAX_REPORT: Spec = Spec {
    prog: "render_max_report.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--max-index", true),
        text_option("--out-dir", false),
        flag("--stdout"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--max-index", max_report_args::MAX_INDEX),
        ("--out-dir", max_report_args::OUT_DIR),
        ("--stdout", max_report_args::STDOUT),
    ],
    layouts: crate::help_layouts_render_max_report::LAYOUTS,
};

/// Indices of the `publish_benchmark_record.py` actions.
pub mod publish_args {
    pub const SWEEP_INDEX: usize = 1;
    pub const VERDICT_JSON: usize = 2;
    pub const LITE_RAW_JSON: usize = 3;
    pub const OUT_DIR: usize = 4;
    pub const STDOUT: usize = 5;
}

/// The original `publish_benchmark_record.py` parser.
pub const PUBLISH_BENCHMARK_RECORD: Spec = Spec {
    prog: "publish_benchmark_record.py",
    actions: &[
        ActionDef {
            name: "-h/--help",
            kind: Kind::Help,
            ..PLAIN
        },
        text_option("--sweep-index", false),
        text_option("--verdict-json", false),
        text_option("--lite-raw-json", false),
        text_option("--out-dir", false),
        flag("--stdout"),
    ],
    options: &[
        ("-h", HELP_ACTION),
        ("--help", HELP_ACTION),
        ("--sweep-index", publish_args::SWEEP_INDEX),
        ("--verdict-json", publish_args::VERDICT_JSON),
        ("--lite-raw-json", publish_args::LITE_RAW_JSON),
        ("--out-dir", publish_args::OUT_DIR),
        ("--stdout", publish_args::STDOUT),
    ],
    layouts: crate::help_layouts_publish_benchmark_record::LAYOUTS,
};
