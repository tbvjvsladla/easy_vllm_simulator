//! inline_python_scan `.rs` mode (plan 26111805 c10 · request 26111719 A-4): temporary git repositories, the real
//! binary, the verdict (0/1/2) and the finding lines.
//!
//! - `mutation_*`: one launch variant written into a `checks/*.rs` of a clean tree makes the check fail.
//! - `stale_*` / `beyond_*` / `listed_*`: the allowlist is exact (path + call text + count; no per-file permission).
//! - `not_a_launch_*`: Python mentions that start nothing (request A-1 "실행이 아닌" list) are no findings.
//! - `rs_summary_*`: the `.rs` summary line is printed only when the whole check passes (divergence d1).
use std::path::{Path, PathBuf};
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_inline_python_scan")
}

struct Repo(PathBuf);
impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const SHELL_ALLOW_REL: &str = ".claude/rust/py-boundary-guard/inline_python_allowlist.tsv";
const RS_ALLOW_REL: &str = ".claude/rust/py-boundary-guard/rs_python_allowlist.tsv";
const MUTANT_REL: &str = ".claude/skills/x/rust/checks/mutant_cases.rs";

fn repo(name: &str) -> Repo {
    let dir = std::env::temp_dir().join(format!("rsscan-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q"]);
    write(&dir, SHELL_ALLOW_REL, "# none\n");
    Repo(dir)
}

fn git(dir: &Path, args: &[&str]) {
    let s = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(s.success(), "git {args:?}");
}

fn write(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn run(dir: &Path) -> (i32, String, String) {
    git(dir, &["add", "-A"]);
    let out = Command::new(bin())
        .arg("check")
        .arg("--repo")
        .arg(dir)
        .output()
        .unwrap();
    (
        out.status.code().unwrap(),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn real_repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

/// Writes `source` as a `checks/*.rs` of an otherwise clean tree and expects the check to fail with one finding of
/// `kind` per entry of `lines` (1-based lines of `source`).
fn assert_mutant_fails(name: &str, source: &str, kind: &str, lines: &[usize]) {
    let r = repo(name);
    write(&r.0, MUTANT_REL, source);
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 1, "{name}: the mutant must fail\n{out}{err}");
    for line in lines {
        let want = format!("RS-VIOLATION {MUTANT_REL}:{line}: {kind} ");
        assert!(out.contains(&want), "{name}: missing `{want}`\n{out}");
    }
    let found = out
        .lines()
        .filter(|l| l.starts_with("RS-VIOLATION "))
        .count();
    assert_eq!(found, lines.len(), "{name}: unexpected findings\n{out}");
    assert!(err.contains("FAIL .rs mode"), "{name}: {err}");
}

macro_rules! mutation {
    ($test:ident, $kind:literal, $lines:expr, $source:expr) => {
        #[test]
        fn $test() {
            assert_mutant_fails(stringify!($test), $source, $kind, &$lines);
        }
    };
}

// ------------------------------------------------------------------------------------------- mutations (variants)

mutation!(
    mutation_literal_program,
    "program",
    [3],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(\"python3\");\n}\n"
);
mutation!(
    mutation_versioned_and_bare_names,
    "program",
    [3, 4],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(\"python3.12\");\n    let _ = Command::new(\"python\");\n}\n"
);
mutation!(
    mutation_path_prefix,
    "program",
    [3],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(\"/usr/bin/python3\");\n}\n"
);
mutation!(
    mutation_raw_strings,
    "program",
    [3, 4],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(r#\"python3\"#);\n    let _ = Command::new(r\"/usr/local/bin/python3.11\");\n}\n"
);
mutation!(
    mutation_byte_string,
    "program",
    [4],
    "use std::os::unix::ffi::OsStrExt;\nuse std::process::Command;\nfn m() {\n    let _ = Command::new(std::ffi::OsStr::from_bytes(b\"python3\"));\n}\n"
);
mutation!(
    mutation_std_process_path,
    "program",
    [2],
    "fn m() {\n    let _ = std::process::Command::new(\"python3\");\n}\n"
);
mutation!(
    mutation_tokio_process_path,
    "program",
    [2],
    "async fn m() {\n    let _ = tokio::process::Command::new(\"python3\").output().await;\n}\n"
);
mutation!(
    mutation_use_alias,
    "program",
    [3],
    "use std::process::Command as Cmd;\nfn m() {\n    let _ = Cmd::new(\"python3\");\n}\n"
);
mutation!(
    mutation_const_binding,
    "program",
    [4],
    "use std::process::Command;\nconst PY: &str = \"python3\";\nfn m() {\n    let _ = Command::new(PY);\n}\n"
);
mutation!(
    mutation_static_binding_through_a_module_path,
    "program",
    [4],
    "use std::process::Command;\nmod cfg { pub static INTERPRETER: &str = \"/usr/bin/python3\"; }\nfn m() {\n    let _ = Command::new(cfg::INTERPRETER);\n}\n"
);
mutation!(
    mutation_let_bindings,
    "program",
    [4, 6],
    "use std::process::Command;\nfn m() {\n    let py = \"python3\";\n    let _ = Command::new(&py);\n    let owned = String::from(\"python3\");\n    let _ = Command::new(owned.as_str());\n}\n"
);
mutation!(
    mutation_path_and_string_constructors,
    "program",
    [4, 5, 6],
    "use std::path::{Path, PathBuf};\nuse std::process::Command;\nfn m() {\n    let _ = Command::new(Path::new(\"/usr/bin/python3\"));\n    let _ = Command::new(PathBuf::from(\"python3\"));\n    let _ = Command::new(\"python3\".to_string());\n}\n"
);
mutation!(
    mutation_join_names_the_program,
    "program",
    [3],
    "use std::process::Command;\nfn m(venv: &std::path::Path) {\n    let _ = Command::new(venv.join(\"bin/python\"));\n}\n"
);
mutation!(
    mutation_format_names_the_program,
    "program",
    [3, 5],
    "use std::process::Command;\nfn m(root: &str) {\n    let _ = Command::new(format!(\"{root}/venv/bin/python3\"));\n    let py = format!(\"{}/bin/python\", root);\n    let _ = Command::new(&py);\n}\n"
);
mutation!(
    mutation_branch_values,
    "program",
    [3],
    "use std::process::Command;\nfn m(fast: bool) {\n    let _ = Command::new(if fast { \"pypy\" } else { \"python3\" });\n}\n"
);
mutation!(
    mutation_arg0_names_the_interpreter,
    "arg0",
    [4],
    "use std::os::unix::process::CommandExt;\nuse std::process::Command;\nfn m() {\n    let _ = Command::new(\"/bin/true\").arg0(\"python3\").status();\n}\n"
);
mutation!(
    mutation_shell_command_line,
    "shell",
    [3, 4],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(\"sh\").args([\"-c\", \"python3 -c 'print(1)'\"]).status();\n    let _ = Command::new(\"/bin/bash\").arg(\"-lc\").arg(\"exec python3 tool.py\").status();\n}\n"
);
mutation!(
    mutation_shell_command_line_over_statements,
    "shell",
    [3],
    "use std::process::Command;\nfn m() {\n    let mut c = Command::new(\"bash\");\n    c.arg(\"-c\");\n    let script = \"python3 - <<'PY'\\nprint(1)\\nPY\";\n    c.arg(script);\n    let _ = c.status();\n}\n"
);
mutation!(
    mutation_wrapper_programs,
    "wrapper",
    [3, 4],
    "use std::process::Command;\nfn m() {\n    let _ = Command::new(\"/usr/bin/env\").args([\"PYTHONPATH=/x\", \"python3\", \"-c\", \"1\"]).status();\n    let _ = Command::new(\"timeout\").args([\"9\", \"python3\", \"x.py\"]).status();\n}\n"
);
mutation!(
    mutation_remote_command_line,
    "remote",
    [3],
    "use std::process::Command;\nfn m(host: &str) {\n    let _ = Command::new(\"ssh\").args([host, \"cd /x && python3 run.py\"]).status();\n}\n"
);
mutation!(
    mutation_argument_vector_array,
    "argv",
    [3],
    "fn run(_argv: &[&str]) {}\nfn m(dir: &str) {\n    run(&[\"python3\", \"-m\", \"venv\", dir]);\n}\n"
);
mutation!(
    mutation_argument_vector_vec,
    "argv",
    [3],
    "fn run(_argv: Vec<String>) {}\nfn m(code: String) {\n    run(vec![\"python3\".to_string(), \"-c\".into(), code]);\n}\n"
);
mutation!(
    mutation_argument_vector_through_a_shell,
    "argv",
    [3],
    "fn trace(_argv: &[&str]) {}\nfn m() {\n    trace(&[\"/bin/sh\", \"-c\", \"python3 -c pass\"]);\n}\n"
);
mutation!(
    mutation_helper_parameter,
    "helper",
    [6],
    "use std::process::Command;\nfn spawn(program: &str, args: &[&str]) -> Command {\n    let mut c = Command::new(program); c.args(args); c\n}\nfn m() {\n    let _ = spawn(\"python3\", &[\"x\"]);\n}\n"
);
mutation!(
    mutation_inside_an_expression_macro,
    "program",
    [3],
    "use std::process::Command;\nfn m() {\n    assert!(Command::new(\"python3\").status().unwrap().success(), \"x\");\n}\n"
);
mutation!(
    mutation_inside_macro_rules,
    "program",
    [3],
    "use std::process::Command;\nmacro_rules! interpreter {\n    () => { Command::new(\"python3\") };\n}\n"
);
mutation!(
    mutation_inside_a_test_module,
    "program",
    [5],
    "#[cfg(test)]\nmod tests {\n    #[test]\n    fn t() {\n        let _ = std::process::Command::new(\"python3\");\n    }\n}\n"
);

/// The layout of a call does not change its text: a wrapped call is the same allowlist key as a one-line call.
#[test]
fn mutation_wrapped_call_has_the_one_line_call_text() {
    let r = repo("wrapped");
    write(
        &r.0,
        MUTANT_REL,
        "use std::process::Command;\nfn m() {\n    let _ = Command::new(\n        \"python3\", // the oracle\n    );\n}\n",
    );
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains(&format!(
            "RS-VIOLATION {MUTANT_REL}:3: program Command::new(\"python3\")\n"
        )),
        "{out}"
    );
}

/// A real `checks/*.rs` of this repository with one more launch: beyond its allowlist row (same call text) and in a
/// file without a row (another call text) both fail.
#[test]
fn mutation_in_real_checks_files() {
    let real = real_repo();
    let classifier = ".claude/skills/vllm-recipe-explorer/rust/runtime/checks/classifier_checks.rs";
    let guard = ".claude/rust/py-boundary-guard/checks/guard_checks.rs";
    let row = std::fs::read_to_string(real.join(RS_ALLOW_REL))
        .unwrap()
        .lines()
        .find(|l| l.starts_with(&format!("{classifier}\t")))
        .expect("the classifier row of the real allowlist")
        .to_string();
    let r = repo("real-checks");
    write(&r.0, RS_ALLOW_REL, &format!("{row}\n"));
    let classifier_text = std::fs::read_to_string(real.join(classifier)).unwrap();
    let guard_text = std::fs::read_to_string(real.join(guard)).unwrap();
    write(&r.0, classifier, &classifier_text);
    write(&r.0, guard, &guard_text);
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 0, "the copies pass as they are\n{out}{err}");
    write(
        &r.0,
        classifier,
        &format!("{classifier_text}\nfn injected() {{ let _ = Command::new(\"python3\"); }}\n"),
    );
    write(
        &r.0,
        guard,
        &format!(
            "{guard_text}\nfn injected() {{ let _ = std::process::Command::new(\"python3\"); }}\n"
        ),
    );
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("program Command::new(\"python3\") (row allows 2, found 3)"),
        "{out}"
    );
    assert!(out.contains(&format!("RS-VIOLATION {guard}:")), "{out}");
}

// --------------------------------------------------------------------------------------------------- allowlist

const TWO_LAUNCHES: &str = "use std::process::Command;\nfn a() { let _ = Command::new(\"python3\"); }\nfn b() { let _ = Command::new(\"python3\"); }\n";

#[test]
fn listed_launches_pass_with_the_exact_count() {
    let r = repo("listed");
    write(&r.0, MUTANT_REL, TWO_LAUNCHES);
    write(
        &r.0,
        RS_ALLOW_REL,
        &format!("# c\n{MUTANT_REL}\tCommand::new(\"python3\")\t2\tc34-recipe-runtime\toracle\n"),
    );
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains(".rs mode: scanned 1 files · 2 interpreter launches within rs_python_allowlist.tsv (1 rows)"), "{out}");
    assert!(
        out.lines().last().unwrap_or("").contains("] PASS scanned"),
        "{out}"
    );
}

/// Approved divergence d1 (`rs-mode-output`): the `.rs` summary line belongs to a pass of the whole check and stands
/// right before the PASS line. When the `.rs` mode passes but a non-`.rs` surface fails, the output is the base one:
/// the VIOLATION line on stdout, the FAIL line on stderr and exit 1, with no `.rs` line.
#[test]
fn rs_summary_line_only_on_a_whole_pass() {
    let r = repo("summary");
    write(&r.0, MUTANT_REL, TWO_LAUNCHES);
    write(
        &r.0,
        RS_ALLOW_REL,
        &format!("{MUTANT_REL}\tCommand::new(\"python3\")\t2\tc34-recipe-runtime\toracle\n"),
    );
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2, "{out}");
    assert!(
        lines[0].starts_with(
            "[inline_python_scan] .rs mode: scanned 1 files · 2 interpreter launches within rs_python_allowlist.tsv (1 rows)"
        ),
        "{out}"
    );
    assert!(
        lines[1].starts_with("[inline_python_scan] PASS scanned 0 files"),
        "{out}"
    );

    write(&r.0, "a/host.sh", "python3 -c 'print(1)'\n");
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    assert_eq!(out, "VIOLATION a/host.sh:1: python3 -c 'print(1)'\n");
    assert!(!err.contains(".rs mode"), "{err}");
    assert_eq!(err.lines().count(), 1, "{err}");
    assert!(
        err.starts_with(
            "[inline_python_scan] FAIL 1 line(s) start Python on the host/sub outside the allowlist."
        ),
        "{err}"
    );
}

#[test]
fn beyond_the_count_of_a_row_is_a_violation() {
    let r = repo("beyond");
    write(&r.0, MUTANT_REL, TWO_LAUNCHES);
    write(
        &r.0,
        RS_ALLOW_REL,
        &format!("{MUTANT_REL}\tCommand::new(\"python3\")\t1\tc34-recipe-runtime\toracle\n"),
    );
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains(&format!("RS-VIOLATION {MUTANT_REL}:2: program Command::new(\"python3\") (row allows 1, found 2)")), "{out}");
    assert!(
        out.contains(&format!("RS-VIOLATION {MUTANT_REL}:3: program")),
        "{out}"
    );
}

#[test]
fn stale_rows_fail() {
    let r = repo("stale");
    write(&r.0, MUTANT_REL, TWO_LAUNCHES);
    write(
        &r.0,
        RS_ALLOW_REL,
        &format!(
            "{MUTANT_REL}\tCommand::new(\"python3\")\t3\tc34-recipe-runtime\toracle\ngone/checks/x.rs\tCommand::new(\"python3\")\t1\tc43-upstream-runtime\tremoved\n"
        ),
    );
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        out.contains(&format!(
            "RS-STALE {MUTANT_REL}\tCommand::new(\"python3\"): listed 3, found 2"
        )),
        "{out}"
    );
    assert!(
        out.contains("RS-STALE gone/checks/x.rs\tCommand::new(\"python3\"): listed 1, found 0"),
        "{out}"
    );
    assert!(!out.contains("RS-VIOLATION"), "{out}");
    assert!(!out.contains("] PASS"), "{out}");
}

/// A row is for its own file: the same call text in another file is not covered (no per-file or global permission).
#[test]
fn listed_text_in_another_file_is_a_violation() {
    let r = repo("other-file");
    write(&r.0, MUTANT_REL, TWO_LAUNCHES);
    write(&r.0, "b/checks/copy.rs", TWO_LAUNCHES);
    write(
        &r.0,
        RS_ALLOW_REL,
        &format!("{MUTANT_REL}\tCommand::new(\"python3\")\t2\tc34-recipe-runtime\toracle\n"),
    );
    let (code, out, _) = run(&r.0);
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("RS-VIOLATION b/checks/copy.rs:2: program"),
        "{out}"
    );
    assert!(
        !out.contains(&format!("RS-VIOLATION {MUTANT_REL}")),
        "{out}"
    );
}

#[test]
fn bad_allowlists_and_unparsable_sources_fail_closed() {
    for (name, body) in [
        (
            "fields",
            format!("{MUTANT_REL}\tCommand::new(\"python3\")\t2\tc34\n"),
        ),
        (
            "count",
            format!("{MUTANT_REL}\tCommand::new(\"python3\")\t0\tc34\twhy\n"),
        ),
        (
            "cell",
            format!("{MUTANT_REL}\tCommand::new(\"python3\")\t2\tC 34\twhy\n"),
        ),
        (
            "not-rs",
            "a/x.sh\tCommand::new(\"python3\")\t2\tc34\twhy\n".to_string(),
        ),
        (
            "dup",
            format!("{MUTANT_REL}\tc\t1\tc34\twhy\n{MUTANT_REL}\tc\t1\tc34\twhy\n"),
        ),
    ] {
        let r = repo(&format!("bad-{name}"));
        write(&r.0, RS_ALLOW_REL, &body);
        assert_eq!(run(&r.0).0, 2, "{name}");
    }
    let r = repo("bad-source");
    write(&r.0, MUTANT_REL, "fn broken( {\n");
    let (code, _, err) = run(&r.0);
    assert_eq!(code, 2);
    assert!(
        err.contains(&format!("cannot read {MUTANT_REL} as Rust tokens")),
        "{err}"
    );
    let r = repo("bad-explicit");
    git(&r.0, &["add", "-A"]);
    let out = Command::new(bin())
        .arg("check")
        .arg("--repo")
        .arg(&r.0)
        .arg("--rs-allowlist")
        .arg(r.0.join("missing.tsv"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

/// A tracked `.rs` that is no Rust file (a fragment) is still read, token by token.
#[test]
fn mutation_in_a_fragment_that_is_no_rust_file() {
    let r = repo("fragment");
    write(
        &r.0,
        MUTANT_REL,
        "let a = \"python3 -c\";\nlet b = std::process::Command::new(\"python3\");\n",
    );
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 1, "{out}{err}");
    assert!(
        out.contains(&format!(
            "RS-VIOLATION {MUTANT_REL}:2: program Command::new(\"python3\")"
        )),
        "{out}"
    );
    assert!(
        err.contains(&format!(
            "note .rs mode: {MUTANT_REL} does not parse as a Rust file"
        )),
        "{err}"
    );
}

#[test]
fn help_names_the_rs_mode() {
    let out = Command::new(bin()).arg("--help").output().unwrap();
    assert_eq!(out.status.code(), Some(0));
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(
        help.contains("--rs-allowlist") && help.contains("tracked .rs sources"),
        "{help}"
    );
}

// ------------------------------------------------------------------------------------------------- not a launch

/// Request 26111719 A-1, "실행이 아닌 python3 언급": the lines, in the shape they have in the tree, start nothing.
#[test]
fn not_a_launch_request_a1_lines() {
    let r = repo("not-a-launch");
    // adversarial-benchmark poisoned fake interpreters (negative tests: the stub fails if it is ever started)
    write(
        &r.0,
        ".claude/skills/adversarial-benchmark/rust/checks/bench_verifier_cases.rs",
        "fn t(bin: &std::path::Path, marker: &std::path::Path) {\n    for name in [\"python3\", \"python\"] {\n        let stub = bin.join(name);\n        std::fs::write(&stub, format!(\"#!/bin/sh\\necho \\\"$0 $*\\\" >> '{}'\\nexit 97\\n\", marker.display())).expect(\"stub\");\n    }\n}\n",
    );
    write(
        &r.0,
        ".claude/skills/adversarial-benchmark/rust/checks/broad_search_selftest_cases.rs",
        "fn t(scratch: &S, marker: &std::path::Path) {\n    fake_tool(\n        scratch,\n        \"python3\",\n        &format!(\"printf called >> {}\\nexit 1\", marker.display()),\n    );\n}\nfn fake_tool(s: &S, name: &str, body: &str) { std::fs::write(s.bin().join(name), body).unwrap(); }\n",
    );
    // recipe/upstream forbidden-word lists (assert that production sources do not name python3)
    write(
        &r.0,
        ".claude/skills/vllm-recipe-explorer/rust/runtime/checks/cleanup_checks.rs",
        "fn t(source: &str) {\n    for forbidden in [\n        \"BoundaryViolation\",\n        \"/usr/bin/python\",\n        \"python3\",\n        \"/usr/bin/docker\",\n    ] {\n        assert!(!source.contains(forbidden));\n    }\n    for forbidden in [\"python3\", \"std::process::Command\"] { assert!(!source.contains(forbidden)); }\n}\n",
    );
    // the retired-route constant of the resolver API surface
    write(
        &r.0,
        ".claude/skills/adversarial-benchmark/rust/src/resolve_topology.rs",
        "/// The interpreter text of the retired route.\npub const DEFAULT_PYTHON: &str = \"python3\";\npub const PYTHON_ENV: &str = \"RESOLVE_TOPOLOGY_PYTHON\";\nfn api(map: &mut M) { map.set(\"DEFAULT_PYTHON\", PyValue::str(DEFAULT_PYTHON)); }\n",
    );
    // hint-publisher fixture strings and an observed-command list
    write(
        &r.0,
        ".claude/skills/hint-publisher/rust/src/artifacts/selftest.rs",
        "fn f() -> Vec<(&'static str, &'static str)> {\n    vec![(\"probe.sh\", \"#!/bin/bash\\nset -e\\nLOC=$(python3 -c \\\"print('')\\\" 2>/dev/null || true)\\n\"),\n         (\"x\", \"python3 -c \\\"import fxmod.dev as d\\\"\")]\n}\nfn g() { for (name, argv) in [(\"module attribute\", jl![\"python3\", \"-c\", \"import vllm; print(vllm.__version__)\"])] { let _ = (name, argv); } }\n",
    );
    write(
        &r.0,
        ".claude/skills/hint-publisher/rust/src/template/selftest.rs",
        "fn f() -> &'static str { r#\"{\"command\": \"python3 render.py --materialize-configs\\npython3 render.py --cluster-envfile\"}\"# }\n",
    );
    // a fake /proc/<pid>/cmdline (data shaped like an argument vector) and other data
    write(
        &r.0,
        ".claude/skills/upstream-version-watch/rust/runtime/src/native_multinode/self_test.rs",
        "fn cmdline(root: &str) -> String {\n    let parts = [format!(\"{}/venv/bin/python3\", root), format!(\"{}/venv/bin/vllm\", root), \"serve\".into()];\n    parts.join(\"\\0\")\n}\n",
    );
    write(
        &r.0,
        ".claude/skills/x/rust/src/misc.rs",
        "// Command::new(\"python3\") was the original\n/// `Command::new(\"python3\")` in a doc comment\nfn f() {\n    let a = \"python3 -c\";\n    let code = \"Command::new(\\\"python3\\\")\";\n    let _ = std::process::Command::new(\"bash\").args([\"-c\", \"echo python3 is gone\"]).status();\n    let _ = std::process::Command::new(\"docker\").args([\"exec\", \"c\", \"python3\", \"-c\", \"1\"]).status();\n    let _ = std::process::Command::new(env!(\"CARGO_BIN_EXE_py_exec_trap\")).status();\n    let _ = (a, code);\n}\n",
    );
    let (code, out, err) = run(&r.0);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains(".rs mode: scanned 8 files · 0 interpreter launches"),
        "{out}"
    );
}

/// The real tree: the files of the A-1 "실행이 아닌" list hold no row of the allowlist, and the real tree passes — so
/// none of their lines is found as a launch (and none is hidden by the allowlist).
#[test]
fn not_a_launch_request_a1_files_in_this_repository() {
    let real = real_repo();
    let allowlist = std::fs::read_to_string(real.join(RS_ALLOW_REL)).unwrap();
    let listed: Vec<&str> = allowlist
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| l.split('\t').next().unwrap())
        .collect();
    let mut present = 0;
    for rel in [
        ".claude/skills/adversarial-benchmark/rust/checks/bench_verifier_cases.rs",
        ".claude/skills/adversarial-benchmark/rust/checks/sweep_repeats_cases.rs",
        ".claude/skills/adversarial-benchmark/rust/checks/broad_search_selftest_cases.rs",
        ".claude/skills/vllm-recipe-explorer/rust/runtime/checks/cleanup_checks.rs",
        ".claude/skills/vllm-recipe-explorer/rust/runtime/checks/functional_checks.rs",
        ".claude/skills/vllm-recipe-explorer/rust/runtime/checks/simlog_checks.rs",
        ".claude/skills/upstream-version-watch/rust/runtime/checks/upstream_checks.rs",
        ".claude/skills/upstream-version-watch/rust/runtime/checks/layout_checks.rs",
        ".claude/skills/adversarial-benchmark/rust/src/resolve_topology.rs",
        ".claude/skills/hint-publisher/rust/src/artifacts/selftest.rs",
        ".claude/skills/hint-publisher/rust/src/template/selftest.rs",
    ] {
        assert!(
            !listed.contains(&rel),
            "{rel} must not be covered by the allowlist"
        );
        if let Ok(text) = std::fs::read_to_string(real.join(rel)) {
            assert!(
                text.contains("python3"),
                "{rel} no longer mentions python3: update this list"
            );
            present += 1;
        }
    }
    assert!(present > 0, "none of the A-1 files is in the tree");
    let out = Command::new(bin())
        .arg("check")
        .arg("--repo")
        .arg(&real)
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
}
