//! Behaviour of the three modules that the saved original executions do not pin down by themselves: the formulas and
//! their fallbacks by independent arithmetic, Python's call binding, the API's record protocol (tuples, request
//! errors, one record per request), `argparse` rules checked against what CPython's own `argparse` does for the same
//! option set, the hybrid-layer fallback order, stable ranking of equal results, and the benchmark program's shape.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use recipe_runtime::api::{answer, render};
use std::process::Command;

fn json(text: &str) -> PyValue {
    loads(text).expect("test JSON")
}

fn call(operation: &str, args: &str, kwargs: &str) -> PyValue {
    let line =
        format!(r#"{{"id": "t", "operation": "{operation}", "args": {args}, "kwargs": {kwargs}}}"#);
    let row = answer(&PyStr::new(&line));
    json(&render(&row).expect("renders"))
}

fn member(row: &PyValue, key: &str) -> PyValue {
    match row {
        PyValue::Dict(dict) => dict.get(key).cloned().unwrap_or(PyValue::None),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn value_of(row: &PyValue) -> PyValue {
    assert_eq!(member(row, "kind"), PyValue::str("value"), "{}", row.repr());
    member(row, "value")
}

fn exception_of(row: &PyValue) -> (String, String) {
    assert_eq!(
        member(row, "kind"),
        PyValue::str("exception"),
        "{}",
        row.repr()
    );
    (
        member(row, "type").display_text().to_utf8().unwrap(),
        member(row, "message").display_text().to_utf8().unwrap(),
    )
}

const DENSE: &str = r#"{"num_hidden_layers": 32, "num_key_value_heads": 8, "head_dim": 128, "max_position_embeddings": 131072, "num_params": 8000000000, "prequantized": false, "serve_bpw": 2.0}"#;

#[test]
fn the_kv_term_follows_the_formula_and_the_hard_gate_the_plan_literal() {
    // 2 (K and V) * 32 layers * 8 kv heads * 128 head dim * 2 bytes * 32768 tokens = 4 GiB exactly.
    let candidate = r#"{"id": "c", "quantization": "none", "max_model_len": 32768, "gpu_memory_utilization": 0.5}"#;
    let row = call(
        "estimate_vram.estimate",
        &format!("[{DENSE}, {candidate}, 2, 80.0, 0.9]"),
        "{}",
    );
    let result = value_of(&row);
    assert_eq!(member(&result, "kv_gib"), PyValue::Float(4.0));
    assert_eq!(member(&result, "kv_layers"), PyValue::int(32));
    assert_eq!(
        member(&result, "kv_layers_source"),
        PyValue::str("num_hidden_layers")
    );
    // weight 8e9 * 2.0 bytes, divided by tp=2 only; kv and overhead are not divided; all divided by the utilization.
    let overhead = 400_000_000.0
        + (8e9_f64 * 0.08).trunc()
        + ((32768.0_f64 / 4096.0) * 150_000_000.0).trunc()
        + 500_000_000.0;
    let total = (16e9_f64 / 2.0 + 4294967296.0 + overhead) / 0.5;
    let gib = 1073741824.0;
    assert_eq!(
        member(&result, "estimated_total_gib"),
        PyValue::Float(total / gib)
    );
    assert_eq!(
        member(&result, "headroom_gib"),
        PyValue::Float(80.0 - total / gib)
    );
    assert_eq!(
        member(&result, "gate_pass"),
        PyValue::Bool(total / gib <= 80.0 * 0.9)
    );
    // Key order: the skeleton, then the new keys at the end.
    let keys: Vec<String> = match &result {
        PyValue::Dict(dict) => dict.iter().map(|(key, _)| key.to_utf8().unwrap()).collect(),
        _ => unreachable!(),
    };
    assert_eq!(keys.last().map(String::as_str), Some("kv_layers_source"));
    assert_eq!(
        &keys[..4],
        [
            "id",
            "quantization",
            "max_model_len",
            "gpu_memory_utilization"
        ]
    );
}

#[test]
fn hybrid_layer_count_falls_back_to_every_layer_unless_it_is_plausible() {
    let layers = |extra: &str| {
        let parsed =
            format!(r#"{{"num_hidden_layers": 64, "num_full_attention_layers": {extra}}}"#);
        value_of(&call(
            "estimate_vram.kv_bearing_layers",
            &format!("[{parsed}]"),
            "{}",
        ))
    };
    assert_eq!(layers("16"), PyValue::int(16));
    assert_eq!(layers("64"), PyValue::int(64));
    for implausible in ["0", "-3", "65", r#""x""#, "[]", "null"] {
        assert_eq!(layers(implausible), PyValue::int(64), "{implausible}");
    }
    // An infinite count is not caught by the original's `except (TypeError, ValueError)`.
    let row = call(
        "estimate_vram.kv_bearing_layers",
        r#"[{"num_hidden_layers": 64, "num_full_attention_layers": Infinity}]"#,
        "{}",
    );
    assert_eq!(exception_of(&row).0, "OverflowError");
    // Unknown total: a positive count stands on its own, anything else is unknown.
    let alone = |count: &str| {
        value_of(&call(
            "estimate_vram.kv_bearing_layers",
            &format!(r#"[{{"num_full_attention_layers": {count}}}]"#),
            "{}",
        ))
    };
    assert_eq!(alone("12"), PyValue::int(12));
    assert_eq!(alone("0"), PyValue::None);
}

#[test]
fn the_feasible_limits_floor_to_a_power_of_two_and_a_batch_and_stop_at_zero() {
    let weights = "51.1e9";
    let args = |extra: &str| format!("[{DENSE}, 115.0, 0.9, {extra}, null, 2, {weights}, 2e9]");
    let max_len = value_of(&call(
        "estimate_vram.max_feasible_max_len",
        &args("1"),
        "{}",
    ));
    let PyValue::Int(length) = &max_len else {
        panic!("{}", max_len.repr())
    };
    assert!(
        length.to_string().parse::<u64>().unwrap().is_power_of_two()
            || length.to_string() == "131072"
    );
    let none_fit = value_of(&call(
        "estimate_vram.max_feasible_max_len",
        &format!("[{DENSE}, 1.0, 0.9, 1, null, 2, 51.1e9, 2e9]"),
        "{}",
    ));
    assert_eq!(none_fit, PyValue::int(0));
    let batch = value_of(&call(
        "estimate_vram.max_feasible_batch",
        &args("32768"),
        "{}",
    ));
    // floor division of the safe KV bytes by (bytes per token * context)
    let safe = (115.0_f64 * 0.9 * 1073741824.0).trunc() as i128 - 51_100_000_000 - 2_000_000_000;
    assert_eq!(batch, PyValue::int((safe / (131072 * 32768)) as i64));
}

#[test]
fn python_call_binding_errors_are_cpythons() {
    let (class, message) = exception_of(&call("estimate_vram.estimate", "[1, 2, 3, 4]", "{}"));
    assert_eq!(
        (class.as_str(), message.as_str()),
        (
            "TypeError",
            "estimate() missing 1 required positional argument: 'safety_margin'"
        )
    );
    let (_, message) = exception_of(&call(
        "estimate_vram.estimate",
        "[1, 2, 3, 4, 5, 6, 7]",
        "{}",
    ));
    assert_eq!(
        message,
        "estimate() takes from 5 to 6 positional arguments but 7 were given"
    );
    let (_, message) = exception_of(&call(
        "estimate_vram.estimate",
        "[1, 2, 3, 4, 5]",
        r#"{"x": 1}"#,
    ));
    assert_eq!(message, "estimate() got an unexpected keyword argument 'x'");
    let (_, message) = exception_of(&call(
        "estimate_vram.estimate",
        "[1, 2, 3, 4, 5]",
        r#"{"parsed": 1}"#,
    ));
    assert_eq!(
        message,
        "estimate() got multiple values for argument 'parsed'"
    );
    let (_, message) = exception_of(&call("quant_table.vllm_quant_bpw", "[]", "{}"));
    assert_eq!(
        message,
        "vllm_quant_bpw() missing 2 required positional arguments: 'quant' and 'serve_bpw'"
    );
    let (_, message) = exception_of(&call("estimate_vram.estimate", "[]", "{}"));
    assert_eq!(message, "estimate() missing 5 required positional arguments: 'parsed', 'candidate', 'tp', 'budget_gib', and 'safety_margin'");
    // Keywords bind by the original names and defaults apply.
    let by_name = value_of(&call(
        "quant_table.vllm_quant_bpw",
        "[]",
        r#"{"serve_bpw": 1.5, "quant": null}"#,
    ));
    assert_eq!(by_name, PyValue::Float(1.5));
}

#[test]
fn the_record_protocol_marks_tuples_and_answers_every_request_even_a_bad_one() {
    let pair = value_of(&call("quant_table.dtype_bpw", r#"["  BF16 "]"#, "{}"));
    assert_eq!(pair.repr(), "{'__tuple__': [2.0, None]}");
    let nested = value_of(&call(
        "quant_table.vllm_quant_bpw_or_none",
        r#"["nope", 1.0]"#,
        "{}",
    ));
    assert!(
        nested
            .repr()
            .starts_with("{'__tuple__': [None, 'unknown quant_method 'nope'")
            || nested.repr().contains("unknown quant_method"),
        "{}",
        nested.repr()
    );
    for (line, fragment) in [
        ("not json", "not JSON"),
        ("[1]", "not an object"),
        (r#"{"operation": "quant_table.dtype_bpw"}"#, "no id"),
        (r#"{"id": 1, "operation": 5}"#, "operation must be a string"),
        (
            r#"{"id": 1, "operation": "quant_table.dtype_bpw", "args": 3}"#,
            "args must be an array",
        ),
    ] {
        let row = json(&render(&answer(&PyStr::new(line))).unwrap());
        let (class, message) = exception_of(&row);
        assert_eq!(class, "RequestError", "{line}");
        assert!(message.contains(fragment), "{line}: {message}");
    }
    let (class, message) = exception_of(&call("quant_table.nope", "[]", "{}"));
    assert_eq!(
        (class.as_str(), message.as_str()),
        (
            "AttributeError",
            "module 'quant_table' has no attribute 'nope'"
        )
    );
    let (class, message) = exception_of(&call("other.f", "[]", "{}"));
    assert_eq!(
        (class.as_str(), message.as_str()),
        ("ModuleNotFoundError", "No module named 'other'")
    );
}

#[test]
fn equal_headroom_results_keep_their_candidate_order() {
    let candidate = |id: &str| {
        format!(
            r#"{{"id": "{id}", "quantization": "fp8", "max_model_len": 4096, "gpu_memory_utilization": 0.9}}"#
        )
    };
    let candidates = format!(
        "[{}, {}, {}]",
        candidate("b"),
        candidate("a"),
        candidate("c")
    );
    let row = call(
        "rank_recipes.rank",
        &format!("[{DENSE}, {candidates}, 1, 80.0, 0.9]"),
        "{}",
    );
    let result = value_of(&row);
    let ids = |key: &str| match member(&result, key) {
        PyValue::List(items) => items
            .iter()
            .map(|item| member(item, "id").display_text().to_utf8().unwrap())
            .collect::<Vec<_>>(),
        other => panic!("{}", other.repr()),
    };
    assert_eq!(ids("ranked"), ["b", "a", "c"]);
    assert_eq!(ids("all"), ["b", "a", "c"]);
}

fn run(program: &str, args: &[&str]) -> (Option<i32>, String, String) {
    let output = Command::new(program)
        .args(args)
        .env_clear()
        .output()
        .expect("run");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn argparse_rules_match_cpythons_for_the_same_option_set() {
    // Expected texts are what CPython 3.12's argparse printed for the same options (checked while writing these).
    let estimate = env!("CARGO_BIN_EXE_estimate_vram");
    for (args, last) in [
        (
            vec!["--m", "1"],
            "estimate_vram.sh: error: ambiguous option: --m could match --max-model-len, --margin",
        ),
        (
            vec!["--ma", "1"],
            "estimate_vram.sh: error: ambiguous option: --ma could match --max-model-len, --margin",
        ),
        (
            vec!["--tp"],
            "estimate_vram.sh: error: argument --tp: expected one argument",
        ),
        (
            vec!["--tp", "x", "--zzz"],
            "estimate_vram.sh: error: argument --tp: invalid int value: 'x'",
        ),
        (
            vec!["stray"],
            "estimate_vram.sh: error: unrecognized arguments: stray",
        ),
        (
            vec!["--self-test=1"],
            "estimate_vram.sh: error: argument --self-test: ignored explicit argument '1'",
        ),
    ] {
        let (code, out, err) = run(estimate, &args);
        assert_eq!(code, Some(2), "{args:?}");
        assert!(out.is_empty());
        assert!(err.starts_with("usage: estimate_vram.sh [-h]"), "{err}");
        assert_eq!(err.lines().last(), Some(last), "{args:?}");
    }
    // An abbreviation, `--opt=value`, a negative number as a value and `-hx` (help runs first) are all accepted.
    let (code, out, _) = run(estimate, &["-hx"]);
    assert_eq!(code, Some(0));
    assert!(out.starts_with("usage: estimate_vram.sh"));
    let quant = env!("CARGO_BIN_EXE_quant_table");
    let (code, out, _) = run(quant, &["--serve-b=3.5", "--quant=NONE"]);
    assert_eq!(
        (code, out.as_str()),
        (
            Some(0),
            "quant: NONE\nserve_bpw: 3.5\nvllm_quant_bpw: 3.5\n"
        )
    );
    let (_, _, err) = run(quant, &["--serve-bpw", "-1.5", "--quant", "awq"]);
    assert!(
        err.is_empty(),
        "a negative number is a value, not an option: {err}"
    );
}

#[test]
fn help_follows_the_terminal_width_the_environment_names() {
    // At COLUMNS=40 CPython's argparse (checked while writing this) wraps the usage one option per line and the
    // description after "→"; the table below it keeps the 24-column help position cut back to 16.
    let output = Command::new(env!("CARGO_BIN_EXE_quant_table"))
        .arg("--help")
        .env_clear()
        .env("COLUMNS", "40")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&output.stdout).into_owned();
    let head = "usage: quant_table.sh [-h]\n                      [--quant QUANT]\n                      [--serve-bpw SERVE_BPW]\n                      [--dtype DTYPE]\n                      [--json]\n\nquant 바이트 테이블 해소(벤더링) — quant/dtype →\nbpw.\n\noptions:\n  -h, --help      show this help\n                  message and exit\n";
    assert!(text.starts_with(head), "{text}");
}

#[test]
fn the_benchmark_program_measures_the_library_and_refuses_a_failing_request() {
    // The fixture rows carry the expected whole value; the failing request is refused before any loop.
    // A directory this test creates for itself (exclusively; a name that exists is never reused) and removes at the end.
    let private = recipe_runtime::model_files::PrivateDirectory::create("recipe-bench-").unwrap();
    let directory = private.path().to_path_buf();
    let fixture = directory.join("fixture.json");
    std::fs::write(&fixture, r#"[{"metric": "quant_table.vllm_quant_bpw", "operation": "quant_table.vllm_quant_bpw", "args": ["AWQ", 2.0], "kwargs": {}, "expected": 0.5}]"#).unwrap();
    let bench = env!("CARGO_BIN_EXE_recipe_numeric_bench");
    let (code, out, err) = run(
        bench,
        &[
            "--fixture",
            fixture.to_str().unwrap(),
            "--rounds",
            "3",
            "--iterations",
            "5",
        ],
    );
    assert_eq!(code, Some(0), "{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    let metric = json(lines[1]);
    assert_eq!(
        member(&metric, "metric"),
        PyValue::str("quant_table.vllm_quant_bpw")
    );
    assert_eq!(member(&metric, "unit"), PyValue::str("ns_per_call"));
    let PyValue::List(rounds) = member(&metric, "rounds") else {
        panic!()
    };
    assert_eq!(rounds.len(), 3);
    std::fs::write(&fixture, r#"[{"metric": "m", "operation": "quant_table.vllm_quant_bpw", "args": ["unknown", 2.0], "kwargs": {}, "expected": 0.5}]"#).unwrap();
    let (code, out, err) = run(bench, &["--fixture", fixture.to_str().unwrap()]);
    assert_eq!(code, Some(1));
    assert!(
        out.is_empty() && err.contains("ValueError") && err.contains("fails before measurement"),
        "{err}"
    );
}

#[test]
fn the_constants_object_carries_every_original_constant_in_order() {
    let (code, out, _) = run(env!("CARGO_BIN_EXE_recipe_numeric_api"), &["constants"]);
    assert_eq!(code, Some(0));
    let constants = json(out.trim());
    let keys: Vec<String> = match &constants {
        PyValue::Dict(dict) => dict.iter().map(|(key, _)| key.to_utf8().unwrap()).collect(),
        _ => panic!(),
    };
    assert_eq!(
        keys,
        [
            "QUANT_BYTES_PER_WEIGHT",
            "VLLM_QUANT_BYTES",
            "DTYPE_BPW",
            "FRAMEWORK_OVERHEAD_BYTES",
            "OFFLINE_ONLY_QUANT",
            "GIB",
            "KV_BLOCK_ALIGN_BUFFER"
        ]
    );
    assert_eq!(member(&constants, "GIB"), PyValue::int(1073741824));
    assert_eq!(
        member(&constants, "KV_BLOCK_ALIGN_BUFFER"),
        PyValue::Float(1.02)
    );
}
