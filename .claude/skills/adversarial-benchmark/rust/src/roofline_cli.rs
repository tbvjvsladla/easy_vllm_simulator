//! Command line of `roofline` (the original argparse interface, [`crate::argv::ROOFLINE`]).
//!
//! Exit codes follow the original: `0` result printed, `2` a `_die` (bad input or contradiction), `1` an uncaught
//! exception. The metadata-mismatch warning is written to stderr as the computation reaches it. Rust-only word:
//! `native-help`.

use crate::argv::{self, roofline_args as arg, Outcome, ROOFLINE};
use crate::cli::{on_large_stack, print_failed, raise, write_stderr, Streams};
use crate::pystr::PyStr;
use crate::roofline::{run as compute, RooflineOptions, Stop};
use crate::terminal;
use num_bigint::BigInt;

const NATIVE_HELP: &str = "roofline Rust-only commands (the legacy options are unchanged; see -h):\n  roofline self-test               native checks on synthetic header-only fixtures (not a measurement)\n  roofline aggregate-moe --model-dir D --num-experts N --experts-per-token K\n                                   _active_bytes_moe as JSON {result: [active, dense, routed, k, n, lookup], kinds: {...}}\n  roofline yaml-load PATH          the manifest reader as a transport: repr of yaml.safe_load(open(PATH)) or Class: message\n  roofline yaml-depth PATH         the same read without the recursion limit: JSON {max, calls: [[depth, function], ...], result}\n  roofline native-help             this text\n";

/// Runs the program and returns its exit status. `args` excludes the program name.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let status = dispatch(args, io);
    io.finish(status)
}

fn dispatch(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    if args.len() == 1 && args[0].equals("self-test") {
        let report = crate::roofline_selftest::run_self_test();
        if let Err(failure) = io.print_lines(&PyStr::new(&report.stdout)) {
            return print_failed(io, &failure, "");
        }
        write_stderr(io, &PyStr::new(&report.stderr));
        return report.exit_code;
    }
    if args.len() == 2 && args[0].equals("yaml-load") {
        return run_yaml_load(&args[1], io);
    }
    if args.len() == 2 && args[0].equals("yaml-depth") {
        return run_yaml_depth(&args[1], io);
    }
    if args
        .first()
        .is_some_and(|word| word.equals("aggregate-moe"))
    {
        return run_aggregate(&args[1..], io);
    }
    if args.len() == 1 && args[0].equals("native-help") {
        return match io.print_lines(&PyStr::new(NATIVE_HELP)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        };
    }
    let matches = match argv::parse_with(&ROOFLINE, args) {
        Outcome::Run(matches) => matches,
        Outcome::Help => {
            io.print_help(ROOFLINE.help(terminal::columns()));
            return 0;
        }
        Outcome::Error(message) => {
            let mut text = PyStr::new(&format!(
                "{}{}: error: ",
                ROOFLINE.usage(terminal::columns()),
                ROOFLINE.prog
            ));
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            return 2;
        }
    };
    let mut options =
        RooflineOptions::new(matches.value(arg::MODEL_PATH).cloned().expect("required"));
    if let Some(root) = matches.value(arg::NAS_ROOT) {
        options.nas_root = root.clone();
    }
    options.quant_root = matches.value(arg::QUANT_ROOT).cloned();
    options.manifest = matches.value(arg::MANIFEST).cloned();
    options.tp = matches.int(arg::TP).cloned();
    options.accept_len = matches.float(arg::ACCEPT_LEN);
    options.accept_len_source = matches
        .value(arg::ACCEPT_LEN_SOURCE)
        .and_then(PyStr::to_utf8);
    options.accept_len_evidence = matches.value(arg::ACCEPT_LEN_EVIDENCE).cloned();
    options.bandwidth_gbps = matches.float(arg::BANDWIDTH_GBPS);
    options.interconnect_gbps = matches.float(arg::INTERCONNECT_GBPS);
    options.act_dtype_bytes = matches
        .int(arg::ACT_DTYPE_BYTES)
        .cloned()
        .unwrap_or_else(|| BigInt::from(2));
    options.comm_latency_us_per_layer =
        matches.float(arg::COMM_LATENCY_US_PER_LAYER).unwrap_or(0.0);
    options.realistic_fraction = matches.float(arg::REALISTIC_FRACTION).unwrap_or(0.35);

    let finished = on_large_stack(move || {
        let run = compute(&options);
        let rendered = run
            .result
            .map(|result| crate::pyvalue::PyValue::Dict(result).dumps_pretty());
        (run.stderr, rendered)
    });
    let (stderr, rendered) = finished;
    write_stderr(io, &PyStr::new(&stderr));
    match rendered {
        Err(Stop::Die(message)) => {
            let mut text = PyStr::new("[roofline] ERROR: ");
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            2
        }
        Err(Stop::Raise(error)) => raise(io, "", &error),
        Ok(Err(error)) => {
            let text = error.to_string();
            let (class, message) = text.split_once(": ").unwrap_or(("ValueError", &text));
            raise(
                io,
                "",
                &crate::pynumber::PyException::new(
                    if class == "RecursionError" {
                        "RecursionError"
                    } else {
                        "ValueError"
                    },
                    message,
                ),
            )
        }
        Ok(Ok(json)) => match io.print(&json) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
    }
}

const AGGREGATE_USAGE: &str =
    "usage: roofline aggregate-moe --model-dir DIR --num-experts N --experts-per-token K\n";

/// `_active_bytes_moe(dir, n, k, kinds)` as a command: the traffic accounting a Python test can call without Python.
fn run_aggregate(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    let mut values: [Option<PyStr>; 3] = [None, None, None];
    let names = ["--model-dir", "--num-experts", "--experts-per-token"];
    let mut at = 0;
    while at < args.len() {
        let slot = args[at]
            .to_utf8()
            .and_then(|name| names.iter().position(|n| *n == name));
        match (slot, args.get(at + 1)) {
            (Some(slot), Some(value)) if values[slot].is_none() => {
                values[slot] = Some(value.clone());
                at += 2;
            }
            _ => {
                write_stderr(io, &PyStr::new(AGGREGATE_USAGE));
                return 2;
            }
        }
    }
    let [Some(dir), Some(experts), Some(per_token)] = values else {
        write_stderr(io, &PyStr::new(AGGREGATE_USAGE));
        return 2;
    };
    let (Ok(experts), Ok(per_token)) = (
        crate::pynumber::int_from_str(&experts),
        crate::pynumber::int_from_str(&per_token),
    ) else {
        write_stderr(io, &PyStr::new(AGGREGATE_USAGE));
        return 2;
    };
    let computed = on_large_stack(move || {
        crate::roofline::active_bytes_moe(
            &dir,
            &crate::pyvalue::PyValue::Int(experts),
            &crate::pyvalue::PyValue::Int(per_token),
        )
        .map(|moe| {
            let mut kinds = crate::pyvalue::PyDict::new();
            for (kind, bytes) in &moe.lookup_by_kind {
                kinds.set(kind, crate::pyvalue::PyValue::Int(bytes.clone()));
            }
            let mut out = crate::pyvalue::PyDict::new();
            out.set(
                "result",
                crate::pyvalue::PyValue::List(
                    [
                        &moe.active,
                        &moe.dense,
                        &moe.routed,
                        &moe.experts_per_token,
                        &moe.experts,
                        &moe.lookup,
                    ]
                    .into_iter()
                    .map(|n| crate::pyvalue::PyValue::Int(n.clone()))
                    .collect(),
                ),
            );
            out.set("kinds", crate::pyvalue::PyValue::Dict(kinds));
            crate::pyvalue::PyValue::Dict(out).dumps_pretty()
        })
    });
    match computed {
        Err(Stop::Die(message)) => {
            let mut text = PyStr::new("[roofline] ERROR: ");
            text.push_py(&message);
            text.push_str("\n");
            write_stderr(io, &text);
            2
        }
        Err(Stop::Raise(error)) => raise(io, "", &error),
        Ok(Err(error)) => raise(
            io,
            "",
            &crate::pynumber::PyException::new("ValueError", error.to_string()),
        ),
        Ok(Ok(json)) => match io.print(&json) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
    }
}

/// `yaml-load PATH`: what `yaml.safe_load(open(PATH, encoding="utf-8"))` returns, as its `repr`, or the exception as
/// `Class: message` (exit 1). A transport for differential tests of the YAML reader; not used by the programs.
fn run_yaml_load(path: &PyStr, io: &mut Streams<'_>) -> i32 {
    let path = path.clone();
    let outcome = on_large_stack(move || -> Result<String, String> {
        let bytes = crate::cli::read_path(&path)?;
        match crate::pyyaml::safe_load(&bytes, &path) {
            Ok(value) => Ok(value.repr()),
            Err(error) => Err(format!(
                "{}: {}",
                error.class,
                error.message.to_utf8().unwrap_or_default()
            )),
        }
    });
    match outcome {
        Ok(text) => match io.print(&PyStr::new(&text)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
        Err(text) => {
            let _ = io.print(&PyStr::new(&text));
            1
        }
    }
}

/// `yaml-depth PATH`: the deepest Python stack the original's `yaml.safe_load` reaches for the document (`safe_load` = 1),
/// the calls it makes while composing, and the result as `yaml-load` prints it - read without any recursion limit. A
/// transport for comparing the port's call structure with the original's; not used by the programs.
fn run_yaml_depth(path: &PyStr, io: &mut Streams<'_>) -> i32 {
    let path = path.clone();
    let outcome = on_large_stack(move || -> Result<String, String> {
        let bytes = crate::cli::read_path(&path)?;
        let probe = crate::pyyaml::probe(&bytes, &path);
        let result = match &probe.result {
            Ok(value) => value.repr(),
            Err(error) => format!(
                "{}: {}",
                error.class,
                error.message.to_utf8().unwrap_or_default()
            ),
        };
        let calls: Vec<String> = probe
            .trace
            .iter()
            .map(|(depth, name)| format!("[{depth},\"{name}\"]"))
            .collect();
        Ok(format!(
            "{{\"max\": {}, \"calls\": [{}], \"result\": {}}}",
            probe.peak,
            calls.join(","),
            quoted(&result)
        ))
    });
    match outcome {
        Ok(text) | Err(text) => match io.print(&PyStr::new(&text)) {
            Ok(()) => 0,
            Err(failure) => print_failed(io, &failure, ""),
        },
    }
}

/// `result` as a JSON string literal with ASCII-only text.
fn quoted(text: &str) -> String {
    let mut out = PyStr::default();
    PyStr::new(text).write_json(&mut out, true);
    out.to_utf8().unwrap_or_default()
}
