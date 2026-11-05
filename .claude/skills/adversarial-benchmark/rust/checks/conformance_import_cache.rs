//! The module state of a checker run: the original imports `lite_metrics_native` once per process, so a second
//! `main()` in the same process (same context) obtains the metrics constants once, while every operation, report,
//! publication and source read still happens each time. A failed import is not remembered. The launcher of the metrics
//! owner is wrapped by a counter that records every call and delegates it to the real installed owner binary
//! (no decision is faked).

#[path = "conformance_common.rs"]
mod common;

use common::*;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use sweep_stop::conformance::Context;
use sweep_stop::conformance_native::LiteMetrics;
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::{PyDict, PyValue};

const API: &str = env!("CARGO_BIN_EXE_conformance_full_superset_lite_api");
const CLI: &str = env!("CARGO_BIN_EXE_conformance_full_superset_lite");
const OWNER: &str = env!("CARGO_BIN_EXE_lite_metrics_api");

/// A launcher that logs the first argument of every call to `events.txt` beside it and delegates to the owner; with
/// `fail_constants_once` the first `constants` call fails (status 3, text on standard error) after being logged.
fn install_counter(scripts: &Path, fail_constants_once: bool) {
    let failing = if fail_constants_once {
        "if [ \"$1\" = constants ] && [ ! -e \"$D/failed-once\" ]; then : > \"$D/failed-once\"; echo 'constants unavailable' >&2; exit 3; fi\n"
    } else {
        ""
    };
    let script = format!(
        "#!/bin/bash\nD=\"$(dirname \"$0\")\"\nprintf '%s\\n' \"$1\" >> \"$D/events.txt\"\n{failing}exec '{OWNER}' \"$@\"\n"
    );
    std::fs::write(scripts.join("lite_metrics_api.sh"), script).expect("counter launcher");
}

fn events(scripts: &Path) -> Vec<String> {
    std::fs::read_to_string(scripts.join("events.txt"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

fn count(events: &[String], name: &str) -> usize {
    events.iter().filter(|event| *event == name).count()
}

fn overrides(command: &mut Command) {
    with_overrides(
        command,
        env!("CARGO_BIN_EXE_bench_reporting_api"),
        env!("CARGO_BIN_EXE_publish_benchmark_record"),
        env!("CARGO_BIN_EXE_lite_metrics_api"),
    );
}

/// One API process with `mains` consecutive `main` rows: the same process and the same context.
fn api_mains(scripts: &Path, mains: usize) -> Vec<PyDict> {
    let mut command = Command::new(API);
    overrides(&mut command);
    let mut child = command
        .arg("evaluate-jsonl")
        .env("CONFORMANCE_SCRIPTS_DIR", scripts)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let rows: String = (0..mains)
        .map(|n| format!("{{\"id\": \"m{n}\", \"operation\": \"main\", \"arguments\": {{}}}}\n"))
        .collect();
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(rows.as_bytes())
        .expect("write");
    let output = child.wait_with_output().expect("wait");
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| match loads(line).expect("JSON line") {
            PyValue::Dict(map) => map,
            other => panic!("not an object: {}", other.repr()),
        })
        .collect()
}

fn assert_whole_pass(record: &PyDict) {
    assert_eq!(field(record, "value"), &PyValue::int(0));
    assert_eq!(
        text(field(record, "stdout")),
        read_text("normal_stdout.txt")
    );
}

#[test]
fn a_second_main_in_the_same_process_does_not_import_the_metrics_again() {
    let single = Tree::new("cache-single");
    let single_scripts = install_copy(&single);
    install_counter(&single_scripts, false);
    api_mains(&single_scripts, 1);
    let per_main = events(&single_scripts);
    assert_eq!(count(&per_main, "constants"), 1, "one main imports once");
    let operations = count(&per_main, "evaluate-jsonl");
    assert!(operations > 0, "the checks call the owner");

    let twice = Tree::new("cache-twice");
    let twice_scripts = install_copy(&twice);
    install_counter(&twice_scripts, false);
    let records = api_mains(&twice_scripts, 2);
    for record in &records {
        assert_whole_pass(record);
    }
    let seen = events(&twice_scripts);
    assert_eq!(
        count(&seen, "constants"),
        1,
        "the constants are obtained once per process"
    );
    assert_eq!(
        count(&seen, "evaluate-jsonl"),
        2 * operations,
        "every build/judge/read operation still asks the owner each time"
    );
}

#[test]
fn a_failed_import_is_not_remembered_and_the_next_main_imports_again() {
    let tree = Tree::new("cache-retry");
    let scripts = install_copy(&tree);
    install_counter(&scripts, true);
    let records = api_mains(&scripts, 3);
    // the first main: the import failed, a caught check, status 1, no further owner call
    assert_eq!(field(&records[0], "value"), &PyValue::int(1));
    let first = text(field(&records[0], "stdout"));
    assert!(
        first.contains("FAIL E 판정기 적재·픽스처 판정  — ImportError('lite metrics native transport failed (3): constants unavailable')\n"),
        "{first}"
    );
    // the second main imports again (and succeeds), the third reuses it
    assert_whole_pass(&records[1]);
    assert_whole_pass(&records[2]);
    let seen = events(&scripts);
    assert_eq!(
        count(&seen, "constants"),
        2,
        "the failed and the successful import, nothing more"
    );
    let single = Tree::new("cache-retry-baseline");
    let baseline = install_copy(&single);
    install_counter(&baseline, false);
    api_mains(&baseline, 1);
    assert_eq!(
        count(&seen, "evaluate-jsonl"),
        2 * count(&events(&baseline), "evaluate-jsonl"),
        "the failed main made no owner call; the two passing mains made all of theirs"
    );
}

#[test]
fn the_command_line_main_imports_the_metrics_once() {
    let tree = Tree::new("cache-cli");
    let scripts = install_copy(&tree);
    install_counter(&scripts, false);
    let mut command = Command::new(CLI);
    overrides(&mut command);
    let output = command
        .env("CONFORMANCE_SCRIPTS_DIR", &scripts)
        .output()
        .expect("run");
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        read_text("normal_stdout.txt")
    );
    assert_eq!(count(&events(&scripts), "constants"), 1);
}

/// The module state belongs to the context: a clone shares it, a separately built context starts empty (the
/// interpreter's cache is per process; identity across different scripts directories is not modelled).
#[test]
fn the_module_state_is_shared_by_clones_and_not_by_separate_contexts() {
    let tree = Tree::new("cache-scope");
    tree.write("lite_metrics_native.py", b"");
    tree.write(
        "lite_metrics_api.sh",
        b"printf '%s\\n' \"$1\" >> \"$(dirname \"$0\")/events.txt\"\nprintf '%s\\n' '{\"LITE_VERDICTS\": [\"pass\"], \"LITE_EXIT\": {\"pass\": 0}}'\n",
    );
    let context: Context = tree.context();
    let first = LiteMetrics::beside_adapter(&context)
        .load(context.modules())
        .expect("import");
    let clone = context.clone();
    let second = LiteMetrics::beside_adapter(&clone)
        .load(clone.modules())
        .expect("cached");
    assert_eq!(first.verdicts.len(), 1);
    assert_eq!(second.verdicts, first.verdicts);
    assert_eq!(count(&events(&tree.root), "constants"), 1);
    let separate = tree.context();
    LiteMetrics::beside_adapter(&separate)
        .load(separate.modules())
        .expect("import");
    assert_eq!(count(&events(&tree.root), "constants"), 2);
}
