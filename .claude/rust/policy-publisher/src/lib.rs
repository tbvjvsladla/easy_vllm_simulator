//! `policy_publisher`: the Rust port of `.claude/policies/runtime/evidence_publisher.py` (plan_26072506 Phase 2
//! hybrid evidence publisher). The launcher `.claude/policies/runtime/evidence_publisher.sh` starts the
//! `evidence_publisher` binary as `evidence_publisher.py` (argv[0]), so the program locates its repository exactly as
//! `Path(__file__)` did. The completion gate and the naming shim are the `policy-gate` crate; the downgrade vocabulary
//! is the adversarial-benchmark classifier (through its native launcher); nothing of theirs is copied.

pub mod bench_section;
pub mod classify;
pub mod cli;
pub mod fsx;
pub mod gate;
pub mod json;
pub mod publisher;
pub mod selftest;

use publisher::{bare_error, emit, Ctx, Exit};

/// Library surface for the verifiers that used to import the publisher (`selftest_lite_report`):
/// `report_measurement_config(text)` -> (table | None, state), `_BENCH_SRC_NAME_RE[kind].match(name) is not None`,
/// and the `LITE_BENCH_MODE` contract constant.
pub use publisher::{bench_src_matches as bench_src_name_matches, measurement_config as report_measurement_config, LITE_BENCH_MODE};
use std::path::{Path, PathBuf};

pub const SCRIPT_NAME: &str = "evidence_publisher.py";

/// `Path(__file__).resolve()` of the original: argv[0] when the launcher named it, else derived from the binary location
/// (`<repo>/.claude/rust/policy-publisher/target/<profile>/evidence_publisher`).
pub fn context(argv0: &str) -> Ctx {
    let script = if argv0.ends_with(&format!("/{SCRIPT_NAME}")) && Path::new(argv0).is_absolute() {
        PathBuf::from(argv0)
    } else {
        let exe = std::env::current_exe().unwrap_or_default();
        let mut repo = exe.clone();
        for _ in 0..5 {
            repo.pop();
        }
        repo.join(".claude/policies/runtime").join(SCRIPT_NAME)
    };
    let script = match std::fs::canonicalize(script.parent().unwrap_or(Path::new("/"))) {
        Ok(d) => d.join(SCRIPT_NAME),
        Err(_) => script,
    };
    let mut repo = script.clone();
    for _ in 0..4 {
        repo.pop();
    }
    Ctx { script, repo }
}

fn help_text(sub: Option<&str>) -> &'static str {
    match sub {
        None => include_str!("help/root.txt"),
        Some("init") => include_str!("help/init.txt"),
        Some("append-raw") => include_str!("help/append-raw.txt"),
        Some("set-narrative") => include_str!("help/set-narrative.txt"),
        Some("record-capacity-rejection") => include_str!("help/record-capacity-rejection.txt"),
        Some("publish-benchmark") => include_str!("help/publish-benchmark.txt"),
        Some("publish-lite-report") => include_str!("help/publish-lite-report.txt"),
        Some("set-promotion-target") => include_str!("help/set-promotion-target.txt"),
        _ => include_str!("help/finalize.txt"),
    }
}

fn subs() -> Vec<cli::Sub> {
    use cli::{Opt, Sub};
    let o = |flag: &'static str, required: bool| Opt { flag, required, choices: None };
    let c = |flag: &'static str, required: bool, choices: Vec<String>| Opt { flag, required, choices: Some(choices) };
    let task_classes = gate::schema_enum(&["properties", "task_class", "enum"], false);
    let modes = gate::schema_enum(&["properties", "benchmark", "properties", "mode", "enum"], true);
    let verdicts = gate::schema_enum(&["properties", "benchmark", "properties", "verdict", "enum"], true);
    let strs = |x: &[&str]| x.iter().map(|y| y.to_string()).collect::<Vec<_>>();
    vec![
        Sub {
            name: "init",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                c("--task-class", true, task_classes),
                o("--generated-utc", true),
                o("--identity-json", true),
                o("--conditions-json", false),
                c("--benchmark-mode", false, modes),
                c("--benchmark-verdict", false, verdicts.clone()),
                o("--report-slug", false),
                c("--downgrade-from", false, strs(&publisher::DOWNGRADE_FROM_CLASSES)),
                o("--downgrade-reason", false),
            ],
        },
        Sub {
            name: "append-raw",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                c("--kind", true, gate::evidence_kinds()),
                o("--src", true),
                o("--recorded-utc", true),
                o("--dest-name", false),
            ],
        },
        Sub {
            name: "set-narrative",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                c("--kind", true, strs(&publisher::NARRATIVE_KINDS)),
                o("--narrative-file", true),
                o("--author", true),
                o("--generated-utc", true),
            ],
        },
        Sub {
            name: "record-capacity-rejection",
            opts: vec![o("--repo-root", false), o("--topic", true), o("--gate-evidence-src", true), o("--reason", true), o("--recorded-utc", true)],
        },
        Sub {
            name: "publish-benchmark",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                c("--verdict", true, verdicts),
                o("--generated-utc", true),
                o("--bench-report-src", true),
                o("--certificate-src", false),
                o("--verdict-json-src", false),
            ],
        },
        Sub {
            name: "publish-lite-report",
            opts: vec![o("--repo-root", false), o("--topic", true), o("--generated-utc", true), o("--bench-report-src", true), o("--certificate-src", false)],
        },
        Sub {
            name: "set-promotion-target",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                o("--tag", true),
                o("--topology", true),
                o("--anchor", true),
                o("--generated-utc", true),
            ],
        },
        Sub {
            name: "finalize",
            opts: vec![
                o("--repo-root", false),
                o("--topic", true),
                o("--pii-scan-json", false),
                o("--runtime-json", false),
                o("--capacity-rejection-json", false),
            ],
        },
    ]
}

/// `main(argv)`: one invocation, its stdout text and exit code (stderr is written as the original wrote it).
pub fn run(ctx: &Ctx, argv: &[String]) -> Exit {
    let parsed = match cli::parse(argv, &subs()) {
        Ok(p) => p,
        Err(m) => return emit(&bare_error("CLI_USAGE_ERROR", &format!("invalid command-line invocation: {m}")), 2),
    };
    let (self_test, cmd, values) = match parsed {
        cli::Parsed::Help(sub) => return Exit { code: 0, stdout: help_text(sub).to_string() },
        cli::Parsed::Ok { self_test, cmd, values } => (self_test, cmd, values),
    };
    if self_test {
        return selftest::self_test(ctx);
    }
    let cmd = match cmd {
        Some(c) => c,
        None => {
            return emit(
                &bare_error(
                    "CLI_USAGE_ERROR",
                    "no subcommand given (init|append-raw|set-narrative|publish-benchmark|publish-lite-report|set-promotion-target|finalize)",
                ),
                2,
            )
        }
    };
    let r = match cmd {
        "init" => publisher::cmd_init(ctx, &values),
        "append-raw" => publisher::cmd_append_raw(ctx, &values),
        "set-narrative" => publisher::cmd_set_narrative(ctx, &values),
        "record-capacity-rejection" => publisher::cmd_record_capacity_rejection(ctx, &values),
        "publish-benchmark" => publisher::cmd_publish_benchmark(ctx, &values),
        "publish-lite-report" => publisher::cmd_publish_lite_report(ctx, &values),
        "set-promotion-target" => publisher::cmd_set_promotion_target(ctx, &values),
        _ => publisher::cmd_finalize(ctx, &values),
    };
    match r {
        Ok(e) | Err(e) => e,
    }
}

/// The process entry: load the owners the original imported, run, print, return the exit code.
pub fn main_entry() -> i32 {
    let args: Vec<String> = std::env::args().collect();
    let argv0 = args.first().cloned().unwrap_or_default();
    let ctx = context(&argv0);
    if let Err(e) = gate::load(&ctx.repo) {
        eprintln!("{e}");
        return 1;
    }
    let out = run(&ctx, &args[1..]);
    use std::io::Write;
    let mut so = std::io::stdout();
    let _ = so.write_all(out.stdout.as_bytes());
    let _ = so.flush();
    out.code
}
