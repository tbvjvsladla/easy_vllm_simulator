//! The command line of `assemble_sweep_meta`.
//!
//! Without arguments the program is the producer the deployed `sweep_bench.sh` calls (through its launcher): the
//! environment is the input, the index is written beside the sweep, one line goes to standard output. Warnings and
//! refusals go to standard error as the original's `print(..., file=sys.stderr)` and `raise SystemExit` did; an
//! exception nothing catches ends with the traceback marker and `Class: message`, status 1.
//!
//! `--self-test` is the native start-up check of the producer itself: it runs the real producer on a private cell of
//! its own and checks what came out. The scenario suite of the metadata contract is not repeated here; it is the
//! existing `selftest_sweep_meta` verifier, which drives the deployed producer.

use crate::assemble_sweep_meta::{
    assemble, current_utc_text, Ending, EnvSource, MapEnv, ProcessEnv,
};
use crate::cli::{write_stderr, Streams, TRACEBACK_HEADER};
use crate::guidellm::Failure;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::repeat_text::cat;
use std::path::Path;

const PROGRAM: &str = crate::assemble_sweep_meta::PROGRAM;
const USAGE: &str = "usage: assemble_sweep_meta [--self-test]\n";

/// The name a traceback prints for the class of a failure: an exception of the owners the original imported carries
/// its module (`repeat_axis_native.RepeatAxisError`); the builtins are bare.
fn traceback_class(class: &str) -> String {
    match class {
        "RepeatAxisError" => "repeat_axis_native.RepeatAxisError".to_string(),
        "JSONDecodeError" => "json.decoder.JSONDecodeError".to_string(),
        other => other.to_string(),
    }
}

fn traceback_line(failure: &Failure) -> PyStr {
    let class = traceback_class(failure.class);
    if failure.message.is_empty() {
        PyStr::new(&class)
    } else {
        cat!(class, ": ", failure.message)
    }
}

/// Runs the program and returns its exit status.
pub fn run(args: &[PyStr], io: &mut Streams<'_>) -> i32 {
    match args {
        [] => produce(&ProcessEnv, io),
        [flag] if flag.equals("--self-test") => self_test(io),
        _ => {
            let shown: Vec<String> = args
                .iter()
                .map(|argument| argument.to_utf8().unwrap_or_default())
                .collect();
            let text = format!(
                "{USAGE}{PROGRAM}: error: unrecognized arguments: {}\n",
                shown.join(" ")
            );
            write_stderr(io, &PyStr::new(&text));
            2
        }
    }
}

fn produce(env: &dyn EnvSource, io: &mut Streams<'_>) -> i32 {
    let produced = assemble(env, &current_utc_text);
    write_stderr(io, &produced.stderr);
    let status = match produced.ending {
        Ending::Done(line) => match io.print(&line) {
            Ok(()) => 0,
            Err(failure) => crate::cli::print_failed(io, &failure, ""),
        },
        Ending::Status(status) => status,
        Ending::Reason(reason) => {
            write_stderr(io, &cat!(reason, "\n"));
            1
        }
        Ending::Uncaught(failure) => {
            write_stderr(io, &cat!(TRACEBACK_HEADER, traceback_line(&failure), "\n"));
            1
        }
    };
    io.finish(status)
}

// ── self-test ─────────────────────────────────────────────────────────────────────

fn write(path: &Path, text: &str) -> Result<(), String> {
    std::fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

/// A private cell: config yaml, env file, manifest, one completed level with an engine log and the measurement-shape
/// record, laid out as the sweep driver leaves them.
pub(crate) fn lay_out_cell(root: &Path, declared_quantization: &str) -> Result<MapEnv, String> {
    let sweep = root.join("sweep_x");
    std::fs::create_dir_all(sweep.join("level_01")).map_err(|error| error.to_string())?;
    write(
        &root.join("cfg.yaml"),
        &format!("model: /models/acme/Acme-7B\nmax-model-len: 4096\nquantization: {declared_quantization}\n"),
    )?;
    write(&root.join(".env.x"), "IMAGE_TAG=easy-vllm:9.9.9-cell\n")?;
    write(
        &root.join("manifest.yaml"),
        "gpu_model: NVIDIA GB10\ngpus_per_node: 1\ntopology: single\n",
    )?;
    write(
        &sweep.join("level_01").join("engine_x.log"),
        "INFO Initializing a V1 LLM engine (v0.9.9) with config: model='m', quantization=fp8, kv_cache_dtype=auto, seed=None\n",
    )?;
    write(
        &sweep.join("level_01").join("measured.json"),
        "{\"measurement_ok\": true}",
    )?;
    write(
        &sweep.join("truncation.log"),
        "level 2 truncated: run_bench exit 1\nnoise\n",
    )?;
    write(&root.join("knobs.json"), "{\"preset\": \"cell\"}")?;
    let path = |name: &str| root.join(name).to_string_lossy().into_owned();
    Ok(MapEnv::new()
        .with("CONFIG", "x")
        .with("TOPO", "single")
        .with("CFGYAML", &path("cfg.yaml"))
        .with("EF", &path(".env.x"))
        .with("MANIFEST", &path("manifest.yaml"))
        .with("SWEEPDIR", &sweep.to_string_lossy())
        .with("COMPLETED", "1")
        .with("ILEN", "1024")
        .with("KNOB_RECORD", &path("knobs.json")))
}

fn self_test(io: &mut Streams<'_>) -> i32 {
    let outcome = run_self_test();
    match outcome {
        Ok(count) => {
            let line = format!(
                "[{PROGRAM}] self-test OK — {count} checks of the real producer on a private cell"
            );
            match io.print(&PyStr::new(&line)) {
                Ok(()) => io.finish(0),
                Err(failure) => {
                    let status = crate::cli::print_failed(io, &failure, "");
                    io.finish(status)
                }
            }
        }
        Err(problems) => {
            let mut text = format!("[{PROGRAM}] self-test FAIL {} item(s):\n", problems.len());
            for problem in &problems {
                text.push_str(&format!("  - {problem}\n"));
            }
            write_stderr(io, &PyStr::new(&text));
            io.finish(1)
        }
    }
}

fn run_self_test() -> Result<usize, Vec<String>> {
    let directory = crate::verify_fixture::TemporaryDirectory::create("assemble-sweep-meta-")
        .map_err(|raised| vec![raised.repr().to_string()])?;
    let outcome = check_cell(directory.path());
    let _ = directory.close();
    outcome
}

fn index_of(root: &Path) -> Result<(String, PyValue), String> {
    let path = root.join("sweep_x").join("sweep_index.json");
    let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    let value = crate::pyjson::loads(&text).map_err(|error| error.to_string())?;
    Ok((text, value))
}

/// `value == "text"` for a JSON value.
trait SaysText {
    fn equals_text(&self, text: &str) -> bool;
}

impl SaysText for PyValue {
    fn equals_text(&self, text: &str) -> bool {
        crate::pydyn::is_text(self, text)
    }
}

fn member(value: &PyValue, key: &str) -> PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn check_cell(root: &Path) -> Result<usize, Vec<String>> {
    let mut problems: Vec<String> = Vec::new();
    let mut checks = 0usize;
    let mut expect = |ok: bool, what: &str| {
        checks += 1;
        if !ok {
            problems.push(what.to_string());
        }
    };
    let env = lay_out_cell(root, "none").map_err(|error| vec![error])?;

    // 1. A fresh assembly: the line, the file layout, the clock reading, measured-over-declared.
    let produced = assemble(&env, &|| "2030-01-02T03:04:05Z".to_string());
    let line_ok = matches!(&produced.ending, Ending::Done(line)
        if line.to_utf8().is_some_and(|own| own.starts_with("[sweep_bench] sweep_index.json → ")
            && own.ends_with("(완료 레벨 [1] · 절삭 1건)")));
    expect(
        line_ok,
        "a fresh assembly prints the index line with the level list and the truncation count",
    );
    expect(
        produced
            .stderr
            .to_utf8()
            .is_some_and(|own| own.contains("선언↔실측 불일치") && own.contains("quantization:YES(measured=fp8 declared=none)")),
        "a declaration that disagrees with the engine's own report is warned about on standard error",
    );
    match index_of(root) {
        Ok((text, index)) => {
            let keys: Vec<String> = match index.peel() {
                PyValue::Dict(map) => map
                    .iter()
                    .map(|(key, _)| key.to_utf8().unwrap_or_default())
                    .collect(),
                _ => Vec::new(),
            };
            expect(
                keys == [
                    "config",
                    "topology",
                    "generated_utc",
                    "reassembled_from_raw",
                    "verdict_point_level",
                    "meta",
                    "lite",
                    "levels",
                    "repetition",
                    "truncated",
                    "input_len",
                    "bench_knobs",
                ],
                "the index keeps the original's top-level key order",
            );
            expect(
                !text.ends_with('\n') && text.starts_with("{\n  \"config\": \"x\""),
                "indent 2 and no final newline",
            );
            expect(
                member(&index, "generated_utc").equals_text("2030-01-02T03:04:05Z"),
                "a fresh measurement carries the clock reading of the call",
            );
            let meta = member(&index, "meta");
            expect(
                member(&meta, "quantization").equals_text("fp8")
                    && member(&meta, "quantization_source").equals_text("measured(engine log)")
                    && member(&meta, "gpu_key").equals_text("GB10")
                    && member(&meta, "vllm_version").equals_text("9.9.9")
                    && member(&meta, "model").equals_text("acme-7b"),
                "measured beats declared, the GPU key and the model axis are derived, the version comes from the image line",
            );
            expect(
                member(&index, "truncated").repr() == "['level 2 truncated: run_bench exit 1']",
                "only lines that name a truncated level are kept",
            );
        }
        Err(error) => expect(
            false,
            &format!("the fresh index could not be read back: {error}"),
        ),
    }

    // 2. A reassembly inherits the measurement time and never reads the clock.
    let reassemble = lay_out_again(root, &env)?;
    let produced = assemble(&reassemble, &|| {
        panic!("a reassembly must not read the clock")
    });
    expect(
        matches!(produced.ending, Ending::Done(_)),
        "a reassembly ends with the index line",
    );
    match index_of(root) {
        Ok((_, index)) => expect(
            member(&index, "generated_utc").equals_text("2030-01-02T03:04:05Z")
                && matches!(
                    member(&index, "reassembled_from_raw").peel(),
                    PyValue::Bool(true)
                ),
            "a reassembly inherits generated_utc exactly and says it was reassembled",
        ),
        Err(error) => expect(
            false,
            &format!("the reassembled index could not be read back: {error}"),
        ),
    }

    // 3. A manifest without a GPU model stops the run with status 2 and writes nothing.
    write(&root.join("manifest.yaml"), "topology: single\n").map_err(|error| vec![error])?;
    let _ = std::fs::remove_file(root.join("sweep_x").join("sweep_index.json"));
    let produced = assemble(&env, &|| "2030-01-02T03:04:05Z".to_string());
    expect(
        produced.ending == Ending::Status(2),
        "a manifest without gpu_model ends with status 2",
    );
    expect(
        !root.join("sweep_x").join("sweep_index.json").exists(),
        "a refused run leaves no index behind",
    );
    if problems.is_empty() {
        Ok(checks)
    } else {
        Err(problems)
    }
}

/// The same cell with `REASSEMBLE=1`.
fn lay_out_again(root: &Path, fresh: &MapEnv) -> Result<MapEnv, Vec<String>> {
    let mut env = MapEnv::new();
    for name in [
        "CONFIG",
        "TOPO",
        "CFGYAML",
        "EF",
        "MANIFEST",
        "SWEEPDIR",
        "COMPLETED",
        "ILEN",
        "KNOB_RECORD",
    ] {
        let value = fresh
            .get(name)
            .and_then(|own| own.to_utf8())
            .ok_or_else(|| {
                vec![format!(
                    "the private cell lost {name} under {}",
                    root.display()
                )]
            })?;
        env = env.with(name, &value);
    }
    Ok(env.with("REASSEMBLE", "1"))
}
