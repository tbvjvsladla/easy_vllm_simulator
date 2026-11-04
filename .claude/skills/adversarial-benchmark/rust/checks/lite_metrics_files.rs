//! `lite_metrics --judge` file boundaries: the raw reading is rewritten through `PATH + ".tmp"` with
//! `open(tmp, "w")`, an incremental `json.dump(indent=1, ensure_ascii=False)` and `os.replace(tmp, PATH)`. Every
//! scenario below is an observed behaviour of the original, kept as it is (the self-looping alias included): a
//! symlinked raw reading is replaced by a regular file, a symlinked temporary name is written through and then
//! moved, a directory in the way fails at open with nothing changed, and a lone surrogate in an unused field stops
//! the dump half way and leaves the written prefix in the temporary file.

#[path = "common.rs"]
mod common;

use common::command;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

const BENCH_COLD: &str =
    r#"{"completed": 1, "failed": 0, "total_output_tokens": 16, "median_ttft_ms": 123.5}"#;
const BENCH_WARM: &str =
    r#"{"completed": 4, "failed": 0, "total_output_tokens": 64, "median_tpot_ms": 25.0}"#;
const ENGINE: &str = "INFO Available KV cache memory: 20.5 GiB\nINFO Model weights take 12.5 GiB\n";

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let root =
            std::env::temp_dir().join(format!("lite_metrics_files_{}_{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("inputs")).expect("scratch");
        std::fs::write(root.join("inputs/bench-0.json"), BENCH_COLD).expect("cold");
        std::fs::write(root.join("inputs/bench-1.json"), BENCH_WARM).expect("warm");
        std::fs::write(root.join("inputs/engine-1.txt"), ENGINE).expect("engine");
        let raw = format!(
            r#"{{"topology": "single", "nodes": [{{"role": "main", "gpu_smi_used_mib": 8192, "gpu_smi_total_mib": 16384, "ram_total_kib": 33554432, "ram_avail_kib": 16777216}}], "bench_cold_json": "{0}/inputs/bench-0.json", "bench_warm_json": "{0}/inputs/bench-1.json", "engine_log_head": "{0}/inputs/engine-1.txt", "burst_n": 4, "host_health_after": "200", "client_plane_health": "200"{{EXTRA}}}}"#,
            root.display()
        );
        std::fs::write(root.join("raw.json"), raw.replace("{EXTRA}", "")).expect("raw");
        Scratch(root)
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    fn judge(&self) -> std::process::Output {
        command(
            env!("CARGO_BIN_EXE_lite_metrics"),
            Some("80"),
            &[
                "--raw-json".to_string(),
                "raw.json".to_string(),
                "--judge".to_string(),
            ],
        )
        .current_dir(&self.0)
        .output()
        .expect("run")
    }

    fn raw_text(&self) -> String {
        std::fs::read_to_string(self.path("raw.json")).expect("raw reading")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn last_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default()
        .to_string()
}

fn is_verdict_passing(output: &std::process::Output) -> bool {
    output.status.code() == Some(0)
        && output.stderr.is_empty()
        && String::from_utf8_lossy(&output.stdout).contains("lite_verdict: pass (exit 0)")
}

fn is_symlink(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

#[test]
fn symlinked_raw_reading_is_replaced_by_a_regular_file() {
    let s = Scratch::new("raw_symlink");
    std::fs::create_dir(s.path("source")).expect("dir");
    std::fs::rename(s.path("raw.json"), s.path("source/raw-target.json")).expect("move");
    symlink("source/raw-target.json", s.path("raw.json")).expect("link");
    std::fs::write(s.path("raw.json.tmp"), "OLD TMP CONTROL\n").expect("old tmp");
    let target_before = std::fs::read(s.path("source/raw-target.json")).expect("target");
    assert!(is_verdict_passing(&s.judge()));
    assert!(
        !is_symlink(&s.path("raw.json")),
        "raw is now a regular file"
    );
    assert!(!s.path("raw.json.tmp").exists());
    assert_eq!(
        std::fs::read(s.path("source/raw-target.json")).unwrap(),
        target_before
    );
    assert!(s.raw_text().contains("\"lite_verdict\": \"pass\""));
}

#[test]
fn symlinked_temporary_name_is_written_through_then_moved() {
    let s = Scratch::new("tmp_symlink");
    std::fs::create_dir(s.path("sink")).expect("dir");
    std::fs::write(s.path("sink/tmp-target.json"), "OLD TMP TARGET\n").expect("old");
    symlink("sink/tmp-target.json", s.path("raw.json.tmp")).expect("link");
    assert!(is_verdict_passing(&s.judge()));
    assert!(
        is_symlink(&s.path("raw.json")),
        "the symlink itself became raw"
    );
    assert!(!s.path("raw.json.tmp").exists());
    let written = std::fs::read_to_string(s.path("sink/tmp-target.json")).expect("sink");
    assert!(
        written.starts_with("{\n \"topology\": \"single\","),
        "indent=1 layout: {written}"
    );
    assert!(written.contains("\"lite_verdict_source\": \"lite_metrics.judge_lite(raw)\""));
}

#[test]
fn directory_in_the_way_fails_at_open_with_nothing_changed() {
    let s = Scratch::new("tmp_directory");
    std::fs::create_dir(s.path("raw.json.tmp")).expect("dir");
    std::fs::write(
        s.path("raw.json.tmp/keep.txt"),
        "DIRECTORY CONTENT MUST SURVIVE\n",
    )
    .expect("keep");
    let raw_before = s.raw_text();
    let output = s.judge();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        last_line(&output.stderr),
        "IsADirectoryError: [Errno 21] Is a directory: 'raw.json.tmp'"
    );
    assert_eq!(s.raw_text(), raw_before);
    assert_eq!(
        std::fs::read(s.path("raw.json.tmp/keep.txt")).unwrap(),
        b"DIRECTORY CONTENT MUST SURVIVE\n"
    );
}

#[test]
fn alias_of_the_temporary_name_to_raw_leaves_a_self_loop_as_the_original_does() {
    let s = Scratch::new("alias");
    symlink("raw.json", s.path("raw.json.tmp")).expect("link");
    assert!(
        is_verdict_passing(&s.judge()),
        "the original reports success here"
    );
    assert!(is_symlink(&s.path("raw.json")));
    assert_eq!(
        std::fs::read_link(s.path("raw.json")).unwrap(),
        Path::new("raw.json")
    );
    assert!(!s.path("raw.json.tmp").exists());
}

#[test]
fn lone_surrogate_stops_the_dump_and_keeps_the_written_prefix() {
    let s = Scratch::new("surrogate");
    let with_note = s.raw_text().trim_end().trim_end_matches('}').to_string()
        + r#", "operator_note": "operator-\udcff"}"#;
    std::fs::write(s.path("raw.json"), &with_note).expect("raw");
    std::fs::write(s.path("raw.json.tmp"), "OLD TMP SENTINEL\n").expect("sentinel");
    let output = s.judge();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        last_line(&output.stderr),
        "UnicodeEncodeError: 'utf-8' codec can't encode character '\\udcff' in position 10: surrogates not allowed"
    );
    assert_eq!(
        std::fs::read_to_string(s.path("raw.json")).unwrap(),
        with_note,
        "raw is untouched"
    );
    let partial = std::fs::read_to_string(s.path("raw.json.tmp")).expect("partial");
    assert!(partial.starts_with("{\n \"topology\": \"single\","));
    assert!(
        partial.ends_with("\"operator_note\": "),
        "stops before the failing string: {partial:?}"
    );
    assert!(
        !partial.contains("OLD TMP"),
        "the open truncated the old content"
    );
}
