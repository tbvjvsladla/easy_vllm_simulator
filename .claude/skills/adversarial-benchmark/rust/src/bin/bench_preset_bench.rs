//! Generic timing harness of the preset operations. It prepares its inputs outside the timer and times the whole
//! operation (the function body, every file read, the hash and the owner query) inside it; every result is passed
//! through `black_box` and dropped inside the timed region. It knows no case: the cases are files given on the
//! command line, and nothing is cached between iterations.
//!
//! ```text
//! bench_preset_bench pure    CASES.jsonl ROUNDS ITERATIONS
//! bench_preset_bench resolve CASES.json  SCRATCH_DIR ROUNDS ITERATIONS
//! bench_preset_bench hash    SCRATCH_DIR ROUNDS BYTES...
//! ```
//!
//! `pure` reads rows `{id, operation, arguments}` (`validate_preset`, `_parse_cli_value`, ...); `resolve` reads the
//! `{id, operation, arguments, files, owner_present}` cases, stages each case's tree under `SCRATCH_DIR` first (the
//! actual `campaign_init.py` of `BENCH_PRESET_REPO_ROOT` is copied where a case has an owner) and sets the tree's root
//! as the case's `repo_root`; `hash` writes one deterministic file per size (bytes `0..255` repeated) first. The output
//! is one JSON object: the per-round nanoseconds of the timed region.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;
use sweep_stop::bench_preset::{check_dataset_runtime, parse_cli_value, validate_preset};
use sweep_stop::bench_preset_api_cli::evaluate_operation;
use sweep_stop::bench_preset_cli::Runtime;
use sweep_stop::pyjson::{decode_file_text, loads, loads_text};
use sweep_stop::pyvalue::{PyDict, PyValue};

const OWNER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py";

fn fail(message: &str) -> ExitCode {
    eprintln!("[bench_preset_bench] ERROR {message}");
    ExitCode::from(2)
}

fn read_json(path: &Path) -> Result<PyValue, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = decode_file_text(&bytes).map_err(|e| e.to_string())?;
    loads_text(&text).map_err(|e| e.to_string())
}

fn dict_of(value: &PyValue) -> Result<&PyDict, String> {
    match value.peel() {
        PyValue::Dict(map) => Ok(map),
        other => Err(format!("expected an object, got {}", other.type_name())),
    }
}

fn argument(map: &PyDict, name: &str) -> PyValue {
    map.get(name).cloned().unwrap_or(PyValue::None)
}

/// A prepared call: the arguments are already values; the timed region runs the operation.
enum Prepared {
    Validate(PyValue, PyValue),
    ParseValue(PyValue, PyValue),
    Other(String, PyDict),
}

fn prepare_row(row: &PyValue) -> Result<Prepared, String> {
    let map = dict_of(row)?;
    let operation = match map.get("operation") {
        Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
        _ => return Err("row without an operation".to_string()),
    };
    let arguments = dict_of(map.get("arguments").ok_or("row without arguments")?)?.clone();
    Ok(match operation.as_str() {
        "validate_preset" => Prepared::Validate(
            argument(&arguments, "doc"),
            argument(&arguments, "bindings"),
        ),
        "_parse_cli_value" => {
            Prepared::ParseValue(argument(&arguments, "name"), argument(&arguments, "raw"))
        }
        _ => Prepared::Other(operation, arguments),
    })
}

fn run_prepared(prepared: &Prepared, runtime: &Runtime) {
    match prepared {
        Prepared::Validate(doc, bindings) => {
            black_box(validate_preset(black_box(doc), black_box(bindings)).ok());
        }
        Prepared::ParseValue(name, raw) => {
            black_box(parse_cli_value(black_box(name), black_box(raw)).ok());
        }
        Prepared::Other(operation, arguments) => {
            black_box(evaluate_operation(black_box(operation), black_box(arguments), runtime).ok());
        }
    }
}

fn time_rounds(rounds: usize, mut body: impl FnMut()) -> Vec<u128> {
    (0..rounds)
        .map(|_| {
            let started = Instant::now();
            body();
            started.elapsed().as_nanos()
        })
        .collect()
}

fn report(mode: &str, cases: usize, iterations: usize, rounds: &[u128]) {
    let list: Vec<String> = rounds.iter().map(u128::to_string).collect();
    println!(
        "{{\"mode\":\"{mode}\",\"cases\":{cases},\"iterations\":{iterations},\"round_ns\":[{}]}}",
        list.join(",")
    );
}

fn number(text: Option<&String>, what: &str) -> Result<usize, String> {
    text.ok_or_else(|| format!("missing {what}"))?
        .parse::<usize>()
        .map_err(|_| format!("{what} is not a number"))
}

fn pure(args: &[String]) -> Result<(), String> {
    let path = PathBuf::from(args.first().ok_or("missing CASES.jsonl")?);
    let (rounds, iterations) = (
        number(args.get(1), "ROUNDS")?,
        number(args.get(2), "ITERATIONS")?,
    );
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut prepared = Vec::new();
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        prepared.push(prepare_row(&loads(line).map_err(|e| e.to_string())?)?);
    }
    let runtime = Runtime::from_environment();
    // Match the original-function harness: one untimed call per prepared operation.
    for call in &prepared {
        run_prepared(call, &runtime);
    }
    let timings = time_rounds(rounds, || {
        for _ in 0..iterations {
            for call in &prepared {
                run_prepared(call, &runtime);
            }
        }
    });
    report("pure", prepared.len(), iterations, &timings);
    Ok(())
}

fn resolve(args: &[String]) -> Result<(), String> {
    let cases = read_json(Path::new(args.first().ok_or("missing CASES.json")?))?;
    let scratch = PathBuf::from(args.get(1).ok_or("missing SCRATCH_DIR")?);
    let (rounds, iterations) = (
        number(args.get(2), "ROUNDS")?,
        number(args.get(3), "ITERATIONS")?,
    );
    let repo = PathBuf::from(
        std::env::var_os("BENCH_PRESET_REPO_ROOT")
            .ok_or("BENCH_PRESET_REPO_ROOT is not set (the installed owner is copied from it)")?,
    );
    let PyValue::List(list) = cases.peel() else {
        return Err("cases are a list".to_string());
    };
    let mut prepared = Vec::new();
    for case in list {
        let map = dict_of(case)?;
        let id = match map.get("id") {
            Some(PyValue::Str(id)) => id.to_utf8().unwrap_or_default(),
            _ => return Err("case without an id".to_string()),
        };
        let root = scratch.join(&id);
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        if let Some(PyValue::Dict(files)) = map.get("files").map(PyValue::peel) {
            for (rel, encoded) in files.iter() {
                let PyValue::Str(encoded) = encoded else {
                    return Err("file entry is a string".to_string());
                };
                let target = root.join(rel.to_utf8().unwrap_or_default());
                std::fs::create_dir_all(target.parent().ok_or("file has a parent")?)
                    .map_err(|e| e.to_string())?;
                let raw = base64_decode(&encoded.to_utf8().unwrap_or_default())?;
                let staged = replace(&raw, b"@ROOT@", root.to_string_lossy().as_bytes());
                std::fs::write(&target, staged).map_err(|e| e.to_string())?;
            }
        }
        if matches!(
            map.get("owner_present").map(PyValue::peel),
            Some(PyValue::Bool(true))
        ) {
            let owner = root.join(OWNER_REL);
            std::fs::create_dir_all(owner.parent().ok_or("owner has a parent")?)
                .map_err(|e| e.to_string())?;
            std::fs::copy(repo.join(OWNER_REL), &owner).map_err(|e| e.to_string())?;
        }
        let arguments_json = argument(map, "arguments")
            .dumps_ascii()
            .map_err(|e| e.to_string())?
            .replace("@ROOT@", &root.to_string_lossy());
        let operation = argument(map, "operation");
        let row = format!(
            "{{\"operation\":{},\"arguments\":{arguments_json}}}",
            operation.dumps_ascii().map_err(|e| e.to_string())?
        );
        prepared.push(prepare_row(&loads(&row).map_err(|e| e.to_string())?)?);
    }
    let runtime = Runtime::from_environment();
    // Match the original-function harness: one untimed call per prepared operation.
    for call in &prepared {
        run_prepared(call, &runtime);
    }
    let timings = time_rounds(rounds, || {
        for _ in 0..iterations {
            for call in &prepared {
                run_prepared(call, &runtime);
            }
        }
    });
    report("resolve", prepared.len(), iterations, &timings);
    Ok(())
}

fn hash(args: &[String]) -> Result<(), String> {
    let scratch = PathBuf::from(args.first().ok_or("missing SCRATCH_DIR")?);
    let rounds = number(args.get(1), "ROUNDS")?;
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let mut calls = Vec::new();
    for size in &args[2..] {
        let size: usize = size.parse().map_err(|_| "BYTES is a number".to_string())?;
        let file = scratch.join(format!("hash-{size}.bin"));
        let block: Vec<u8> = (0..=255u8).collect();
        let mut data = Vec::with_capacity(size);
        while data.len() < size {
            let take = (size - data.len()).min(block.len());
            data.extend_from_slice(&block[..take]);
        }
        std::fs::write(&file, &data).map_err(|e| e.to_string())?;
        // the expected digest is the one the call itself measures once, outside the timer: a check of the declaration
        let probe = format!(
            "{{\"dataset_kind\":\"json_file\",\"dataset_source\":\"custom\",\"dataset_path\":{:?},\"source_sha256\":\"{}\"}}",
            file.to_string_lossy(),
            "0".repeat(64)
        );
        let values = loads(&probe).map_err(|e| e.to_string())?;
        let measured = check_dataset_runtime(&values).err().and_then(|failure| {
            let text = failure.message.to_utf8()?;
            text.split("실측 ")
                .nth(1)
                .map(|rest| rest.chars().take(64).collect::<String>())
        });
        let declared = measured.ok_or("could not measure the fixture once")?;
        let values =
            loads(&probe.replace(&"0".repeat(64), &declared)).map_err(|e| e.to_string())?;
        calls.push(values);
    }
    // The Python harness also performs one whole successful call before its timer.
    for values in &calls {
        drop(black_box(check_dataset_runtime(black_box(values))));
    }
    let timings = time_rounds(rounds, || {
        for values in &calls {
            black_box(check_dataset_runtime(black_box(values)).ok());
        }
    });
    report("hash", calls.len(), 1, &timings);
    Ok(())
}

fn replace(data: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut at = 0;
    while at < data.len() {
        if data[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(data[at]);
            at += 1;
        }
    }
    out
}

fn base64_decode(encoded: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);
    for c in encoded.bytes().filter(|&c| c != b'=') {
        let value = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return Err("bad base64".to_string()),
        };
        acc = (acc << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xFF) as u8);
        }
    }
    Ok(out)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("pure") => pure(&args[1..]),
        Some("resolve") => resolve(&args[1..]),
        Some("hash") => hash(&args[1..]),
        _ => {
            Err("usage: bench_preset_bench pure|resolve|hash ... (see the module docs)".to_string())
        }
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => fail(&message),
    }
}
