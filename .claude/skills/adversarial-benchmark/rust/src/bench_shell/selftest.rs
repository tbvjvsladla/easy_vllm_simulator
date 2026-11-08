//! `bench_shell --self-test`: a small start-up check of the program itself. It runs a few real operations on a private
//! directory (a sweep opened and a cell recorded, the run counters, a claimed output directory, a refusal and an
//! uncaught failure) and checks what came out. The full behaviour contract is the registered checks, not this.

use super::{execute, Console, Context, Ending, Operation, PROGRAM};
use crate::assemble_sweep_meta::MapEnv;
use crate::cli::{write_stderr, Streams};
use crate::pystr::PyStr;
use std::path::Path;

fn arguments(words: &[&str]) -> Vec<PyStr> {
    words.iter().map(|word| PyStr::new(word)).collect()
}

fn run_one(operation: Operation, args: &[&str], env: &MapEnv, stdin: &str) -> (Console, Ending) {
    let args = arguments(args);
    let context = Context {
        args: &args,
        env,
        stdin: PyStr::new(stdin),
        clock: &|| "2030-01-02T03:04:05Z".to_string(),
    };
    let mut console = Console::default();
    let ended = execute(operation, &context, &mut console);
    (console, ended)
}

fn first_line(console: &Console) -> String {
    console
        .stdout_lines()
        .first()
        .and_then(PyStr::to_utf8)
        .unwrap_or_default()
}

fn check_all(root: &Path) -> Result<usize, Vec<String>> {
    let mut problems = Vec::new();
    let mut checks = 0usize;
    let mut expect = |ok: bool, what: &str| {
        checks += 1;
        if !ok {
            problems.push(what.to_string());
        }
    };
    let state = root.join("state.json").to_string_lossy().into_owned();
    let sweep = root.join("sweep").to_string_lossy().into_owned();
    let env = MapEnv::new()
        .with("SWEEP_ID", "s")
        .with("CELLS", "a, b")
        .with("CTRL", "quantization")
        .with("AUTHORITY", "explore")
        .with("MAX_CELLS", "3")
        .with("WALL", "600")
        .with("FAILLIMIT", "2")
        .with("DECLARED_BY", "operator")
        .with("BASIS", "self-test")
        .with("NOW", "2030-01-02T03:04:05Z")
        .with("TOPO", "single")
        .with("REP_N", "3")
        .with("REP_SRC", "preset")
        .with("CELL_KEY", "a")
        .with("CONFIG", "m")
        .with("CITATION", "self-test")
        .with("STARTED", "2030-01-02T03:04:05Z")
        .with("ENDED", "2030-01-02T03:05:05Z")
        .with("SWEEPDIR", &sweep)
        .with("REPO", &root.to_string_lossy())
        .with("SDIR", &root.to_string_lossy())
        .with("CLS", "{\"cell_outcome\":\"serve_failed\",\"note\":\"n\"}");

    let (console, ended) = run_one(Operation::BroadInit, &[&state], &env, "");
    expect(
        ended == Ending::Finished
            && first_line(&console).starts_with("[broad_search] init — sweep=s cells=2 예산="),
        "broad-init writes the state and says how many cells it holds",
    );
    let (console, ended) = run_one(Operation::BroadCell, &[&state], &env, "");
    expect(
        ended == Ending::Finished && first_line(&console) == "[broad_search] cell a → serve_failed",
        "broad-cell records the cell with its outcome",
    );
    let (console, _) = run_one(Operation::CellOutcome, &[&state, "a"], &env, "");
    expect(
        first_line(&console) == "serve_failed",
        "cell-outcome reads back what broad-cell wrote",
    );
    let text = std::fs::read_to_string(&state).unwrap_or_default();
    expect(
        text.contains("\"cells_remaining\": [\n    \"b\"\n  ]"),
        "the recorded cell leaves the remaining list",
    );
    let (console, _) = run_one(Operation::WarmFraction, &["2", "10"], &env, "");
    expect(
        first_line(&console) == "0.2",
        "warm-fraction rounds the share of warm-up requests",
    );
    let metrics = "vllm:prefix_cache_queries_total{m=\"a\"} 10\nvllm:prefix_cache_hits_total{m=\"a\"} 3\nvllm:request_success_total{m=\"a\"} 8\n";
    let (console, _) = run_one(Operation::PrefixSnapshot, &[], &env, metrics);
    expect(
        first_line(&console) == "10 3 8",
        "prefix-snapshot reads the three counters",
    );
    let (console, ended) = run_one(Operation::DatasetKind, &[&state], &env, "");
    expect(
        console.stdout_lines().is_empty()
            && matches!(&ended, Ending::Raised(failure) if failure.class == "KeyError"),
        "a document without the member ends with the KeyError of the lookup",
    );
    let repo = root.to_string_lossy().into_owned();
    let (console, ended) = run_one(
        Operation::SweepPath,
        &[
            &repo,
            "single",
            "output/single/benchlog/sweep_x",
            "0",
            "claim",
        ],
        &env,
        "",
    );
    expect(
        ended == Ending::Finished
            && root.join("output/single/benchlog/sweep_x").is_dir()
            && first_line(&console).ends_with("sweep_x"),
        "sweep-path claims a fresh output directory",
    );
    let (console, ended) = run_one(
        Operation::SweepPath,
        &[
            &repo,
            "single",
            "output/single/benchlog/sweep_x",
            "0",
            "claim",
        ],
        &env,
        "",
    );
    expect(
        ended == Ending::Exit(2)
            && console
                .stderr_text()
                .to_utf8()
                .is_some_and(|text| text.contains("existing evidence is preserved")),
        "sweep-path refuses to reuse an existing output directory",
    );
    if problems.is_empty() {
        Ok(checks)
    } else {
        Err(problems)
    }
}

pub(super) fn run(io: &mut Streams<'_>) -> i32 {
    let outcome = crate::verify_fixture::TemporaryDirectory::create("bench-shell-")
        .map_err(|raised| vec![raised.repr().to_string()])
        .and_then(|directory| {
            let outcome = check_all(directory.path());
            let _ = directory.close();
            outcome
        });
    match outcome {
        Ok(count) => {
            let line = format!("[{PROGRAM}] self-test OK — {count} checks of the real operations on a private directory");
            match io.print(&PyStr::new(&line)) {
                Ok(()) => 0,
                Err(failure) => crate::cli::print_failed(io, &failure, ""),
            }
        }
        Err(problems) => {
            let mut text = format!("[{PROGRAM}] self-test FAIL {} item(s):\n", problems.len());
            for problem in &problems {
                text.push_str(&format!("  - {problem}\n"));
            }
            write_stderr(io, &PyStr::new(&text));
            1
        }
    }
}
