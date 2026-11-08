//! Standard output on a terminal: Python's `sys.stdout` is line-buffered there, so the banner `prepare` prints comes out
//! before anything the container command writes, while into a pipe the banner is block-buffered and comes out after
//! the child's output (checked in `dataset_cli_cases`). One test function: it owns the terminal for its whole run.

#[path = "common.rs"]
mod common;
#[path = "dataset_common.rs"]
mod dataset;

use common::{drain, open_pty};
use dataset::*;
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::PyValue;

#[test]
fn on_a_terminal_the_banner_precedes_the_child_output() {
    let recorded = list(read_json("cli_cases.json"));
    let process = list(read_json("process_cases.json"));
    let case = dict(&process[0]);
    assert_eq!(text(field(case, "id")), "process-stdout");
    let base = recorded
        .iter()
        .map(dict)
        .find(|candidate| text(field(candidate, "id")) == "066-prepare-fresh")
        .expect("fresh prepare");
    let scratch = Scratch::new("terminal");
    scratch.populate(field(base, "before"));
    std::fs::write(
        scratch.root.join(".fixture-bin/docker"),
        base64_decode(&text(field(case, "fixture_docker_base64"))),
    )
    .expect("fixture script");
    let PyValue::List(argv) = field(case, "argv") else {
        panic!("argv is a list")
    };
    let pty = open_pty(24, 120);
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut search = std::ffi::OsString::from(scratch.root.join(".fixture-bin"));
    search.push(":");
    search.push(path);
    let mut child = Command::new(env!("CARGO_BIN_EXE_dataset_prep"))
        .args(
            argv.iter()
                .map(|a| text(a).replace("@ROOT@", &scratch.text())),
        )
        .current_dir(&scratch.root)
        .env("PATH", search)
        .env("DATASET_FIXTURE_DOCKER_MODE", "success")
        .stdout(Stdio::from(pty.slave.try_clone().expect("slave")))
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    drop(pty.slave);
    let status = child.wait().expect("wait");
    let screen = drain(pty.master).replace(&scratch.text(), "@ROOT@");
    assert_eq!(status.code(), Some(0));
    assert_eq!(
        screen,
        "[dataset_prep] 전처리(오프라인 · fixture-image · budget 512MiB) → @ROOT@/cache/prepared.jsonl\nfixture Docker stdout\n[dataset_prep] 전처리 완료 — 행 3 (input_len 정확 2)\n"
    );
}
