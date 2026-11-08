//! Boundaries and failures of the native `bench_shell` that the frozen originals do not reach: the arithmetic against
//! CPython's own results (`checks/data/bench_shell/numeric_vectors.json`, produced from the interpreter's `round`,
//! `sum`, `float`, `re` and integer division), the files an interrupted `json.dump` leaves behind, the missing
//! argument, variable and operation, the path normalisation of a claimed directory and the program's own start-up
//! check. Every run uses a `PATH` that finds no program.

#[path = "common.rs"]
mod common;

use common::{data_path, dict, list, load_json_file, text_of};
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::PyValue;

struct Ran {
    status: Option<i32>,
    stdout: Vec<u8>,
    stderr: String,
}

impl Ran {
    fn last_line(&self) -> String {
        self.stderr
            .trim_end_matches('\n')
            .rsplit('\n')
            .next()
            .unwrap_or("")
            .to_string()
    }
}

fn run(args: &[&str], env: &[(&str, Vec<u8>)], stdin: &[u8], cwd: Option<&Path>) -> Ran {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench_shell"));
    command.args(args).env_clear().env("PATH", "/nonexistent");
    for (name, value) in env {
        command.env(name, OsString::from_vec(value.clone()));
    }
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn bench_shell");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(stdin)
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait");
    Ran {
        status: output.status.code(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn scratch(label: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("bench-shell-edges-{label}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("scratch directory");
    path
}

fn vectors() -> PyValue {
    load_json_file(&data_path("bench_shell/numeric_vectors.json"))
}

fn text(value: &str) -> Vec<u8> {
    value.as_bytes().to_vec()
}

#[test]
fn warm_fraction_is_cpythons_rounded_true_division() {
    let document = vectors();
    for case in list(
        dict(&document)
            .get("warm_fraction")
            .expect("vectors")
            .clone(),
    ) {
        let case = dict(&case);
        let args: Vec<String> = list(case.get("args").expect("args").clone())
            .iter()
            .map(|value| text_of(Some(value)))
            .collect();
        let ran = run(&["warm-fraction", &args[0], &args[1]], &[], b"", None);
        match case.get("out") {
            Some(PyValue::Str(out)) => {
                assert_eq!(
                    ran.stdout,
                    format!("{}\n", out.to_utf8().unwrap()).into_bytes(),
                    "{args:?}"
                );
                assert_eq!(ran.status, Some(0));
            }
            _ => {
                assert_eq!(ran.status, Some(1), "{args:?} does not fit a float");
                assert_eq!(
                    ran.last_line(),
                    "OverflowError: integer division result too large for a float"
                );
            }
        }
    }
}

fn contract() -> PyValue {
    load_json_file(&data_path("bench_shell/value_contract.json"))
}

#[test]
fn plain_decimal_counts_are_values_under_the_recorded_contract() {
    // Leading zeros and an explicit plus sign are decimal values now (the original source refused `01`).
    let document = contract();
    let rows = list(
        dict(&document)
            .get("warm_fraction_values")
            .expect("rows")
            .clone(),
    );
    assert!(rows.len() >= 10);
    for row in rows {
        let row = dict(&row);
        let args: Vec<String> = list(row.get("args").expect("args").clone())
            .iter()
            .map(|value| text_of(Some(value)))
            .collect();
        let want = text_of(row.get("out"));
        let ran = run(&["warm-fraction", &args[0], &args[1]], &[], b"", None);
        assert_eq!(ran.status, Some(0), "{args:?}");
        assert_eq!(ran.stdout, format!("{want}\n").into_bytes(), "{args:?}");
        assert!(ran.stderr.is_empty(), "{args:?}");
    }
    // a result that does not fit a float keeps the interpreter's own error, also behind a plus sign
    let huge = format!("+1{}", "0".repeat(400));
    let ran = run(&["warm-fraction", &huge, "1"], &[], b"", None);
    assert_eq!(ran.status, Some(1));
    assert_eq!(
        ran.last_line(),
        "OverflowError: integer division result too large for a float"
    );
}

#[test]
fn all_zero_runs_are_values_and_the_first_bad_operand_decides() {
    for (warmups, total, want) in [
        ("00", "11", "0.0\n"),
        ("0", "000", "0.0\n"),
        ("3", "00", "3.0\n"),
    ] {
        let ran = run(&["warm-fraction", warmups, total], &[], b"", None);
        assert_eq!(
            (ran.status, String::from_utf8(ran.stdout).unwrap()),
            (Some(0), want.to_string()),
            "{warmups} {total}"
        );
    }
}

#[test]
fn warm_fraction_refuses_everything_outside_the_declared_grammar_without_evaluating_it() {
    let document = contract();
    let refusal = text_of(dict(&document).get("refusal_stderr"));
    assert!(refusal.starts_with("[bench_shell] ERROR warm-fraction"));
    let rows = list(
        dict(&document)
            .get("warm_fraction_refused")
            .expect("rows")
            .clone(),
    );
    assert!(rows.len() >= 20);
    for row in rows {
        let args: Vec<String> = list(dict(&row).get("args").expect("args").clone())
            .iter()
            .map(|value| text_of(Some(value)))
            .collect();
        let mut argv = vec!["warm-fraction"];
        argv.extend(args.iter().map(String::as_str));
        let ran = run(&argv, &[], b"", None);
        assert_eq!(ran.status, Some(2), "{args:?}");
        assert!(ran.stdout.is_empty(), "{args:?}");
        assert_eq!(ran.stderr, refusal, "{args:?}");
    }
}

fn measured_file(directory: &Path, name: &str, document: &str) -> String {
    let level = directory.join(name).join("sweep/level_01");
    std::fs::create_dir_all(&level).expect("level directory");
    let file = level.join("measured.json");
    std::fs::write(&file, document).expect("measured.json");
    file.to_str().unwrap().to_string()
}

#[test]
fn the_broad_site_reads_its_path_literally_through_the_same_loader_as_the_argv_operation() {
    let directory = scratch("broad-measure");
    let truthy = "{\"measurement_ok\": true}";
    let plain = measured_file(&directory, "plain", truthy);
    let ran = run(&["broad-measurement-ok", &plain], &[], b"", None);
    assert_eq!(
        (ran.status, ran.stdout.len(), ran.stderr.len()),
        (Some(0), 0, 0)
    );
    let falsy = measured_file(&directory, "falsy", "{\"measurement_ok\": false}");
    assert_eq!(
        run(&["broad-measurement-ok", &falsy], &[], b"", None).status,
        Some(1)
    );
    // Quotes, backslashes, a line end, spaces and Unicode are path characters: the file is read, and the answer is the
    // answer of `measurement-ok` for the very same argument (one loader, no second evaluator).
    for odd in [
        "quote'path",
        "two''quotes",
        "a'b'c",
        "it's 2x",
        "dq\"name",
        "back\\slash",
        "line\nbreak",
        "space \u{c720}\u{b2c8}\u{cf54}\u{b4dc}",
    ] {
        let path = measured_file(&directory, odd, truthy);
        let broad = run(&["broad-measurement-ok", &path], &[], b"", None);
        let argv = run(&["measurement-ok", &path], &[], b"", None);
        assert_eq!(
            (broad.status, broad.stdout.len(), broad.stderr.len()),
            (Some(0), 0, 0),
            "{odd:?}"
        );
        assert_eq!(
            (broad.status, &broad.stdout, &broad.stderr),
            (argv.status, &argv.stdout, &argv.stderr),
            "{odd:?}"
        );
    }
    // A backslash-n written in the path is two characters: the original source turned it into a line end and opened a
    // DIFFERENT path. Here the file named with the two characters is the one read.
    let literal = measured_file(&directory, "e\\nd", truthy);
    let newline = measured_file(&directory, "e\nd", "{\"measurement_ok\": false}");
    assert_ne!(literal, newline);
    assert_eq!(
        run(&["broad-measurement-ok", &literal], &[], b"", None).status,
        Some(0)
    );
    assert_eq!(
        run(&["broad-measurement-ok", &newline], &[], b"", None).status,
        Some(1)
    );
    // Load, JSON and type failures are the loader's, identical for both operations.
    let missing = directory
        .join("absent'file.json")
        .to_str()
        .unwrap()
        .to_string();
    let broken = directory.join("broken'.json");
    std::fs::write(&broken, "{").unwrap();
    for target in [
        missing.as_str(),
        broken.to_str().unwrap(),
        directory.to_str().unwrap(),
    ] {
        let broad = run(&["broad-measurement-ok", target], &[], b"", None);
        let argv = run(&["measurement-ok", target], &[], b"", None);
        assert_eq!(broad.status, Some(1), "{target}");
        assert!(
            broad.last_line().contains("Error"),
            "{target}: {}",
            broad.stderr
        );
        assert_eq!(
            (broad.status, &broad.stdout, &broad.stderr),
            (argv.status, &argv.stdout, &argv.stderr),
            "{target}"
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_nul_in_the_path_is_the_loaders_error_for_both_operations_at_library_level() {
    // The operating system cannot hand a NUL in argv, so this is a library context, not a fabricated CLI call.
    use sweep_stop::assemble_sweep_meta::MapEnv;
    use sweep_stop::bench_shell::{execute, Console, Context, Ending, Operation};
    use sweep_stop::pystr::PyStr;
    let args = [PyStr::new("a\0b")];
    let env = MapEnv::new();
    let ended = |operation| {
        let context = Context {
            args: &args,
            env: &env,
            stdin: PyStr::new(""),
            clock: &|| "2030-01-02T03:04:05Z".to_string(),
        };
        execute(operation, &context, &mut Console::default())
    };
    let broad = ended(Operation::BroadMeasurementOk);
    assert!(matches!(broad, Ending::Raised(_)), "{broad:?}");
    assert_eq!(broad, ended(Operation::MeasurementOk));
}

#[test]
fn prefix_snapshot_sums_counters_like_cpython() {
    let document = vectors();
    for case in list(
        dict(&document)
            .get("prefix_snapshot")
            .expect("vectors")
            .clone(),
    ) {
        let case = dict(&case);
        let input = text_of(case.get("stdin"));
        let want = text_of(case.get("out"));
        let ran = run(&["prefix-snapshot"], &[], input.as_bytes(), None);
        assert_eq!(ran.status, Some(0), "{input:?}");
        assert_eq!(
            String::from_utf8(ran.stdout).unwrap(),
            format!("{want}\n"),
            "{input:?}"
        );
    }
}

#[test]
fn a_malformed_counter_is_the_valueerror_of_float() {
    let metrics = "vllm:prefix_cache_queries_total{} 1e+\nvllm:prefix_cache_hits_total{} 1\nvllm:request_success_total{} 2\n";
    let ran = run(&["prefix-snapshot"], &[], metrics.as_bytes(), None);
    assert_eq!(ran.status, Some(1));
    assert!(ran.stdout.is_empty());
    assert_eq!(
        ran.last_line(),
        "ValueError: could not convert string to float: '1e+'"
    );
}

#[test]
fn prefix_record_computes_the_rate_with_exact_integer_division() {
    let document = vectors();
    let directory = scratch("record");
    for case in list(
        dict(&document)
            .get("prefix_record")
            .expect("vectors")
            .clone(),
    ) {
        let case = dict(&case);
        let out = directory.join("record.json");
        let before = text_of(case.get("before"));
        let after = text_of(case.get("after"));
        let ran = run(
            &[
                "prefix-record",
                out.to_str().unwrap(),
                "cfg",
                &before,
                &after,
                "2030-01-01T00:00:00Z",
            ],
            &[],
            b"",
            None,
        );
        assert_eq!(ran.status, Some(0), "{before} -> {after}");
        let written = std::fs::read_to_string(&out).expect("record");
        let parsed = sweep_stop::pyjson::loads(&written).expect("record is JSON");
        let rate = dict(&parsed)
            .get("hit_rate_run")
            .cloned()
            .expect("rate member");
        match (rate, case.get("hit_rate_run")) {
            (PyValue::None, Some(PyValue::None)) => {}
            (PyValue::Float(got), Some(PyValue::Float(want))) => {
                assert_eq!(got, *want, "{before} -> {after}")
            }
            (got, want) => panic!("{before} -> {after}: {got:?} vs {want:?}"),
        }
        assert!(
            !written.ends_with('\n'),
            "json.dump leaves no final newline"
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn prefix_record_unpacking_errors_are_the_interpreters() {
    let directory = scratch("unpack");
    let out = directory.join("r.json");
    let out = out.to_str().unwrap();
    let ran = run(
        &["prefix-record", out, "c", "1 2", "3 4 5", "t"],
        &[],
        b"",
        None,
    );
    assert_eq!(
        ran.last_line(),
        "ValueError: not enough values to unpack (expected 3, got 2)"
    );
    let ran = run(
        &["prefix-record", out, "c", "1 2 3 4", "3 4 5", "t"],
        &[],
        b"",
        None,
    );
    assert_eq!(
        ran.last_line(),
        "ValueError: too many values to unpack (expected 3)"
    );
    let ran = run(
        &["prefix-record", out, "c", "1 x 3", "3 4 5", "t"],
        &[],
        b"",
        None,
    );
    assert_eq!(
        ran.last_line(),
        "ValueError: invalid literal for int() with base 10: 'x'"
    );
    let ran = run(&["prefix-record", out, "c", "1 2 3"], &[], b"", None);
    assert_eq!(
        ran.last_line(),
        "ValueError: not enough values to unpack (expected 5, got 3)"
    );
    assert!(
        !Path::new(out).exists(),
        "no record is written when the counters cannot be read"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

fn init_env(sweep_id: Vec<u8>) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("SWEEP_ID", sweep_id),
        ("CELLS", text("a,b")),
        ("CTRL", text("quantization")),
        ("AUTHORITY", text("explore")),
        ("MAX_CELLS", text("3")),
        ("WALL", text("600")),
        ("FAILLIMIT", text("2")),
        ("DECLARED_BY", text("operator")),
        ("BASIS", text("edge check")),
        ("NOW", text("2030-01-01T00:00:00Z")),
        ("TOPO", text("single")),
        ("REP_N", text("3")),
        ("REP_SRC", text("preset")),
    ]
}

#[test]
fn an_unencodable_value_leaves_the_chunks_before_it_in_the_file() {
    let directory = scratch("partial-init");
    let state = directory.join("state.json");
    let ran = run(
        &["broad-init", state.to_str().unwrap()],
        &init_env(b"s\xff".to_vec()),
        b"",
        None,
    );
    assert_eq!(ran.status, Some(1));
    assert_eq!(
        ran.last_line(),
        "UnicodeEncodeError: 'utf-8' codec can't encode character '\\udcff' in position 2: surrogates not allowed"
    );
    assert_eq!(
        std::fs::read(&state).expect("the file was opened before the dump"),
        b"{\n  \"schema_version\": 1,\n  \"sweep_id\": ".to_vec()
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_dataset_with_a_lone_surrogate_is_cut_at_the_encoder_chunk() {
    let directory = scratch("partial-extract");
    let record = directory.join("record.json");
    let out = directory.join("dataset.json");
    for (dataset, kept, message) in [
        (
            "[\"a\", \"\\ud800\", \"b\"]",
            "[\"a\"",
            "'\\ud800' in position 3",
        ),
        (
            "{\"k\": [\"a\", {\"\\ud800\": 1}]}",
            "{\"k\": [\"a\", {",
            "'\\ud800' in position 1",
        ),
        (
            "{\"a\": \"ok\", \"b\": [\"x\", [\"\\udc80\"]]}",
            "{\"a\": \"ok\", \"b\": [\"x\", ",
            "'\\udc80' in position 2",
        ),
        ("\"\\ud800\"", "", "'\\ud800' in position 1"),
    ] {
        std::fs::write(&record, format!("{{\"dataset\": {dataset}}}")).expect("record");
        let ran = run(
            &[
                "extract-dataset",
                record.to_str().unwrap(),
                out.to_str().unwrap(),
            ],
            &[],
            b"",
            None,
        );
        assert_eq!(ran.status, Some(1), "{dataset}");
        assert_eq!(
            ran.last_line(),
            format!("UnicodeEncodeError: 'utf-8' codec can't encode character {message}: surrogates not allowed"),
            "{dataset}"
        );
        assert_eq!(
            std::fs::read_to_string(&out).expect("opened first"),
            kept,
            "{dataset}"
        );
    }
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_missing_dataset_member_leaves_the_output_alone() {
    let directory = scratch("extract-missing");
    let record = directory.join("record.json");
    let out = directory.join("dataset.json");
    std::fs::write(&record, "{}").expect("record");
    std::fs::write(&out, "keep").expect("existing output");
    let ran = run(
        &[
            "extract-dataset",
            record.to_str().unwrap(),
            out.to_str().unwrap(),
        ],
        &[],
        b"",
        None,
    );
    assert_eq!(ran.last_line(), "KeyError: 'dataset'");
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "keep",
        "the member is read before the output is opened"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn missing_inputs_fail_like_the_interpreter() {
    let ran = run(&["max-level"], &[], b"", None);
    assert_eq!(ran.status, Some(1));
    assert_eq!(ran.last_line(), "KeyError: 'ML'");
    let ran = run(&["verdict"], &[], b"", None);
    assert_eq!(ran.last_line(), "IndexError: list index out of range");
    let ran = run(&["verdict", "/nonexistent/none.json"], &[], b"", None);
    assert_eq!(
        ran.last_line(),
        "FileNotFoundError: [Errno 2] No such file or directory: '/nonexistent/none.json'"
    );
    let ran = run(&["sweep-path", "a", "b"], &[], b"", None);
    assert_eq!(
        ran.last_line(),
        "ValueError: not enough values to unpack (expected 5, got 2)"
    );
    let ran = run(&["image-ref"], &[], b"[1]", None);
    assert_eq!(
        ran.last_line(),
        "TypeError: list indices must be integers or slices, not str"
    );
}

#[test]
fn the_program_names_its_usage_and_refuses_an_unknown_operation() {
    let ran = run(&[], &[], b"", None);
    assert_eq!(ran.status, Some(2));
    assert!(ran.stderr.starts_with("usage: bench_shell"));
    let ran = run(&["no-such-operation"], &[], b"", None);
    assert_eq!(ran.status, Some(2));
    assert!(ran.stderr.contains("알 수 없는 연산: no-such-operation"));
}

#[test]
fn the_start_up_check_runs_the_real_operations() {
    let ran = run(&["--self-test"], &[], b"", None);
    assert_eq!(ran.status, Some(0), "{}", ran.stderr);
    let out = String::from_utf8(ran.stdout).unwrap();
    assert!(out.starts_with("[bench_shell] self-test OK — "), "{out}");
}

#[test]
fn judge_model_tp_lets_only_type_and_value_errors_become_a_dash() {
    let directory = scratch("judge-tp");
    let file = directory.join("index.json");
    for (meta, want) in [
        (
            "{\"model_path\": \"m\", \"tensor_parallel_size\": \"4\"}",
            "m 4\n",
        ),
        (
            "{\"model_path\": \"m\", \"tensor_parallel_size\": 0}",
            "m -\n",
        ),
        (
            "{\"model_path\": \"m\", \"tensor_parallel_size\": -2}",
            "m -\n",
        ),
        ("{\"tensor_parallel_size\": null, \"tp\": 3}", "NA -\n"),
        ("{\"tp\": 3}", "NA 3\n"),
        (
            "{\"model_path\": \"m\", \"tensor_parallel_size\": [1]}",
            "m -\n",
        ),
        (
            "{\"model_path\": \"m\", \"tensor_parallel_size\": \"٣\"}",
            "m 3\n",
        ),
    ] {
        std::fs::write(&file, format!("{{\"meta\": {meta}}}")).expect("index");
        let ran = run(&["judge-model-tp", file.to_str().unwrap()], &[], b"", None);
        assert_eq!(String::from_utf8(ran.stdout).unwrap(), want, "{meta}");
    }
    std::fs::write(&file, "{\"meta\": {\"tensor_parallel_size\": Infinity}}").expect("index");
    let ran = run(&["judge-model-tp", file.to_str().unwrap()], &[], b"", None);
    assert_eq!(ran.status, Some(1));
    assert_eq!(
        ran.last_line(),
        "OverflowError: cannot convert float infinity to integer"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn a_claimed_sweep_path_is_normalised_and_claimed_once() {
    let repo = scratch("sweep-path");
    let repo_text = repo.to_str().unwrap().to_string();
    let canonical = std::fs::canonicalize(&repo).expect("canonical repo");
    let want = format!(
        "{}/output/multi/benchlog/sweep_a1.b-2\n",
        canonical.display()
    );
    let relative = run(
        &[
            "sweep-path",
            &repo_text,
            "multi",
            "./output//multi/benchlog/sweep_a1.b-2/",
            "0",
            "check",
        ],
        &[],
        b"",
        None,
    );
    assert_eq!(String::from_utf8(relative.stdout).unwrap(), want);
    assert!(!canonical.join("output").exists(), "a check claims nothing");
    let absolute = format!(
        "{}//output/multi/benchlog/sweep_a1.b-2",
        canonical.display()
    );
    let claimed = run(
        &["sweep-path", &repo_text, "multi", &absolute, "0", "claim"],
        &[],
        b"",
        None,
    );
    assert_eq!(String::from_utf8(claimed.stdout).unwrap(), want);
    assert!(canonical
        .join("output/multi/benchlog/sweep_a1.b-2")
        .is_dir());
    let again = run(
        &["sweep-path", &repo_text, "multi", &absolute, "0", "claim"],
        &[],
        b"",
        None,
    );
    assert_eq!(again.status, Some(2));
    assert_eq!(
        again.stderr,
        "[sweep_bench] ERROR --sweep-dir: fresh measurement requires an absent directory; existing evidence is preserved\n"
    );
    let outside = run(
        &[
            "sweep-path",
            &repo_text,
            "multi",
            "output/multi/benchlog/sweep_.bad",
            "0",
            "check",
        ],
        &[],
        b"",
        None,
    );
    assert_eq!(
        outside.status,
        Some(2),
        "a name must start with an alphanumeric after the prefix"
    );
    let _ = std::fs::remove_dir_all(&repo);
}

/// Standard input that is an open directory: the first `read` fails with `EISDIR`. This is a deliberate fault
/// injection on the real descriptor of the operation, not an ordinary pipeline input.
fn run_with_directory_stdin(args: &[&str], env: &[(&str, Vec<u8>)], directory: &Path) -> Ran {
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench_shell"));
    command.args(args).env_clear().env("PATH", "/nonexistent");
    for (name, value) in env {
        command.env(name, OsString::from_vec(value.clone()));
    }
    command
        .stdin(Stdio::from(
            std::fs::File::open(directory).expect("open the directory"),
        ))
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let output = command
        .spawn()
        .expect("spawn bench_shell")
        .wait_with_output()
        .expect("wait");
    Ran {
        status: output.status.code(),
        stdout: output.stdout,
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn a_standard_input_that_cannot_be_read_ends_every_stdin_operation_with_the_python_error() {
    let directory = scratch("stdin-error");
    let sidecar = directory.join("tool.json");
    let env = [
        ("GL_ENDPOINT", text("/v1/completions")),
        ("GL_OUT", sidecar.to_str().unwrap().as_bytes().to_vec()),
    ];
    for operation in [
        "provenance-status",
        "event-reason",
        "lite-raw-status",
        "void-reason-source",
        "events-scanned",
        "json-escape-stdin",
        "image-ref",
        "tool-sidecar",
        "floor-mib",
        "prefix-snapshot",
        "bench-mode-log",
    ] {
        let ran = run_with_directory_stdin(&[operation], &env, &directory);
        assert_eq!(ran.status, Some(1), "{operation}");
        assert!(
            ran.stdout.is_empty(),
            "{operation}: nothing is computed from a failed read"
        );
        assert_eq!(
            ran.last_line(),
            "IsADirectoryError: [Errno 21] Is a directory",
            "{operation}"
        );
        assert!(
            ran.stderr.starts_with("Traceback (most recent call last):"),
            "{operation}"
        );
        assert!(
            !ran.stderr.contains("os error"),
            "{operation}: Python's wording, not the host library's"
        );
    }
    assert!(
        !sidecar.exists(),
        "the sidecar is not written when its input could not be read"
    );
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn an_operation_that_reads_no_standard_input_ignores_an_unreadable_one() {
    let directory = scratch("stdin-unused");
    let ran = run_with_directory_stdin(&["warm-fraction", "1", "4"], &[], &directory);
    assert_eq!((ran.status, ran.stdout), (Some(0), b"0.25\n".to_vec()));
    let _ = std::fs::remove_dir_all(&directory);
}
