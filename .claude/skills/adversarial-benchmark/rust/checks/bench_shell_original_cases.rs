//! The 181 frozen executions of the original inline programs, replayed against the native `bench_shell` on detached
//! private copies of their fixtures: the printed lines, the exit status, the final traceback line (or the whole
//! message for an exit that is not an exception) and every file effect, including the effects of a refusal. The
//! originals are never run here; their recorded results are the data (`checks/data/bench_shell/original_cases.json`).
//!
//! The binary runs with a `PATH` that finds no program, so a hidden interpreter, `bash` or any other helper would
//! fail the case instead of answering it.

#[path = "common.rs"]
mod common;

use common::{data_path, dict, list, load_json_file, text_of};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyvalue::{PyDict, PyValue};

const TRACEBACK: &str = "Traceback (most recent call last):";

fn bytes_of(value: Option<&PyValue>) -> Vec<u8> {
    match value {
        Some(PyValue::Str(text)) => text
            .to_surrogateescape_bytes()
            .expect("fixture text is encodable"),
        other => panic!("expected a string field, got {other:?}"),
    }
}

fn string_map(value: &PyValue) -> BTreeMap<String, Vec<u8>> {
    dict(value)
        .iter()
        .map(|(key, value)| (key.to_utf8().expect("key"), bytes_of(Some(value))))
        .collect()
}

fn umask() -> u32 {
    let status = std::fs::read_to_string("/proc/self/status").expect("proc status");
    let line = status
        .lines()
        .find(|line| line.starts_with("Umask:"))
        .expect("umask line");
    u32::from_str_radix(line.trim_start_matches("Umask:").trim(), 8).expect("octal umask")
}

struct Snapshot {
    kind: &'static str,
    mode: u32,
    /// A link's target (`None` for a file or a directory).
    value: Option<String>,
    /// A file's bytes, kept whole so they are compared with the literal expectation (`None` otherwise).
    data: Option<Vec<u8>>,
}

fn snapshot(root: &Path) -> BTreeMap<String, Snapshot> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<String, Snapshot>) {
        let mut entries: Vec<_> = std::fs::read_dir(at)
            .expect("directory")
            .map(|e| e.expect("entry"))
            .collect();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            let meta = std::fs::symlink_metadata(&path).expect("metadata");
            let relative = path
                .strip_prefix(root)
                .expect("inside")
                .to_string_lossy()
                .into_owned();
            let mode = meta.permissions().mode() & 0o7777;
            if meta.file_type().is_symlink() {
                let target = std::fs::read_link(&path).expect("link");
                out.insert(
                    relative,
                    Snapshot {
                        kind: "link",
                        mode,
                        value: Some(target.to_string_lossy().into_owned()),
                        data: None,
                    },
                );
            } else if meta.is_dir() {
                out.insert(
                    relative,
                    Snapshot {
                        kind: "dir",
                        mode,
                        value: None,
                        data: None,
                    },
                );
                walk(root, &path, out);
            } else {
                let data = std::fs::read(&path).expect("file");
                out.insert(
                    relative,
                    Snapshot {
                        kind: "file",
                        mode,
                        value: None,
                        data: Some(data),
                    },
                );
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// `1970-01-01T00:00:00Z` text of a Unix time in whole seconds.
fn utc_text(seconds: i64) -> String {
    let days = seconds.div_euclid(86_400);
    let rest = seconds.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_secs() as i64
}

fn split_stderr(text: &str) -> (String, Option<String>) {
    match text.split_once(TRACEBACK) {
        Some((head, rest)) => {
            let last = rest
                .trim_end_matches('\n')
                .rsplit('\n')
                .next()
                .unwrap_or("")
                .to_string();
            (
                head.to_string(),
                Some(last.replace("json.decoder.JSONDecodeError", "JSONDecodeError")),
            )
        }
        None => (text.to_string(), None),
    }
}

fn replace_bytes(text: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut at = 0;
    while at < text.len() {
        if text[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(text[at]);
            at += 1;
        }
    }
    out
}

fn run_case(
    case: &PyDict,
    base_env: &BTreeMap<String, Vec<u8>>,
    base_files: &BTreeMap<String, Vec<u8>>,
    scratch: &Path,
) -> Vec<String> {
    let id = text_of(case.get("id"));
    let node = scratch.join(&id).join("node");
    std::fs::create_dir_all(&node).expect("node directory");
    let node_text = node.to_string_lossy().into_owned();
    let subst = |bytes: &[u8]| -> Vec<u8> {
        let rooted = replace_bytes(bytes, b"{ROOT}", node_text.as_bytes());
        replace_bytes(
            &rooted,
            b"{SCRIPTS}",
            format!("{node_text}/scripts").as_bytes(),
        )
    };
    let mut files = base_files.clone();
    for (name, content) in string_map(case.get("files").expect("files")) {
        files.insert(name, content);
    }
    for removed in list(case.get("removed").expect("removed").clone()) {
        files.remove(&text_of(Some(&removed)));
    }
    for directory in list(case.get("dirs").expect("dirs").clone()) {
        std::fs::create_dir_all(node.join(text_of(Some(&directory)))).expect("directory");
    }
    for (name, content) in &files {
        let path = node.join(name);
        if path.symlink_metadata().is_ok() || name.is_empty() {
            continue;
        }
        std::fs::create_dir_all(path.parent().expect("parent")).expect("parent directory");
        std::fs::write(&path, content).expect("fixture file");
    }
    let links = string_map(case.get("links").expect("links"));
    for (name, target) in &links {
        let path = node.join(name);
        if path.is_dir() && !path.is_symlink() {
            std::fs::remove_dir_all(&path).expect("replace directory by link");
        }
        std::os::unix::fs::symlink(OsString::from_vec(subst(target)), &path).expect("link");
    }
    let before = snapshot(&node);
    let mut env = base_env.clone();
    for (name, value) in string_map(case.get("env").expect("env")) {
        env.insert(name, value);
    }
    let mut command = Command::new(env!("CARGO_BIN_EXE_bench_shell"));
    command
        .current_dir(&node)
        .env_clear()
        .env("PATH", "/nonexistent");
    for (name, value) in &env {
        command.env(name, OsString::from_vec(subst(value)));
    }
    command.arg(text_of(case.get("op")));
    for argument in list(case.get("args").expect("args").clone()) {
        command.arg(OsString::from_vec(subst(&bytes_of(Some(&argument)))));
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let started = now_seconds();
    let mut child = command.spawn().expect("spawn bench_shell");
    let stdin = bytes_of(case.get("stdin"));
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(&stdin)
        .expect("stdin write");
    let output = child.wait_with_output().expect("wait");
    let ended = now_seconds();
    let mut problems = Vec::new();
    let expected_exit = match case.get("exit") {
        Some(PyValue::Int(number)) => number.to_string().parse::<i32>().expect("exit status"),
        other => panic!("exit: {other:?}"),
    };
    if output.status.code() != Some(expected_exit) {
        problems.push(format!(
            "exit {:?} != {expected_exit}",
            output.status.code()
        ));
    }
    let want_out = subst(&bytes_of(case.get("stdout")));
    let got_out = output.stdout.clone();
    if got_out != want_out {
        problems.push(format!(
            "stdout {:?} != {:?}",
            String::from_utf8_lossy(&got_out),
            String::from_utf8_lossy(&want_out)
        ));
    }
    let (got_head, got_last) = split_stderr(&String::from_utf8_lossy(&output.stderr));
    let want_head =
        String::from_utf8_lossy(&subst(&bytes_of(case.get("stderr_head")))).into_owned();
    let want_last = match case.get("stderr_last") {
        Some(PyValue::None) | None => None,
        other => Some(String::from_utf8_lossy(&subst(&bytes_of(other))).into_owned()),
    };
    if got_head != want_head || got_last != want_last {
        problems.push(format!(
            "stderr {got_head:?}/{got_last:?} != {want_head:?}/{want_last:?}"
        ));
    }
    let after = snapshot(&node);
    let expected = dict(case.get("after").expect("after"));
    let fresh = match case.get("fresh") {
        Some(PyValue::Str(path)) => path.to_utf8(),
        _ => None,
    };
    let mask = umask();
    for (name, entry) in &after {
        let Some(want) = expected.get(name) else {
            problems.push(format!("unexpected entry {name}"));
            continue;
        };
        let want = dict(want);
        let want_kind = text_of(want.get("kind"));
        if entry.kind != want_kind {
            problems.push(format!("{name}: kind {} != {want_kind}", entry.kind));
            continue;
        }
        let created = matches!(want.get("created"), Some(PyValue::Bool(true)));
        let expected_mode = if created {
            (if entry.kind == "dir" { 0o777 } else { 0o666 }) & !mask
        } else {
            before.get(name).map_or(entry.mode, |old| old.mode)
        };
        if entry.kind != "link" && entry.mode != expected_mode {
            problems.push(format!(
                "{name}: mode {:o} != {expected_mode:o}",
                entry.mode
            ));
        }
        let want_value = match want.get("value") {
            Some(PyValue::Str(value)) => Some(value.to_utf8().expect("value")),
            _ => None,
        };
        // A file is expected as its literal bytes (`content`), never as a stored digest.
        let want_data = match want.get("content") {
            Some(PyValue::Str(_)) => Some(bytes_of(want.get("content"))),
            _ => None,
        };
        if want_kind == "file" && (want_data.is_none() || want_value.is_some()) {
            problems.push(format!(
                "{name}: a file expectation must carry its literal `content` and no digest"
            ));
            continue;
        }
        let got_value = if entry.kind == "link" {
            entry
                .value
                .as_ref()
                .map(|value| value.replace(&node_text, "{ROOT}"))
        } else {
            entry.value.clone()
        };
        let same = if entry.kind == "file" {
            entry.data == want_data
        } else {
            got_value == want_value
        };
        if !same {
            if fresh.as_deref() == Some(name.as_str()) {
                if let Some(PyValue::Str(template)) = case.get("fresh_text") {
                    let written = String::from_utf8_lossy(
                        &std::fs::read(node.join(name)).expect("fresh file"),
                    )
                    .into_owned();
                    let marker = "\"generated_utc\": \"";
                    let start = written.find(marker).expect("generated_utc") + marker.len();
                    let stamp = &written[start..start + 20];
                    let normal = written.replacen(stamp, "@TIME@", 1);
                    let in_window =
                        (started - 1..=ended + 1).any(|second| utc_text(second) == stamp);
                    if normal != template.to_utf8().expect("template") || !in_window {
                        problems.push(format!(
                            "{name}: fresh record differs (stamp {stamp}, in window {in_window})"
                        ));
                    }
                    continue;
                }
            }
            if entry.kind == "file" {
                let shown = |bytes: &Option<Vec<u8>>| {
                    bytes
                        .as_ref()
                        .map(|data| String::from_utf8_lossy(data).into_owned())
                };
                problems.push(format!(
                    "{name}: file bytes {:?} != {:?}",
                    shown(&entry.data),
                    shown(&want_data)
                ));
            } else {
                problems.push(format!("{name}: {got_value:?} != {want_value:?}"));
            }
        }
    }
    for (name, _) in expected.iter() {
        if !after.contains_key(&name.to_utf8().unwrap_or_default()) {
            problems.push(format!(
                "missing entry {}",
                name.to_utf8().unwrap_or_default()
            ));
        }
    }
    problems
        .into_iter()
        .map(|problem| format!("{id}: {problem}"))
        .collect()
}

#[test]
fn a_wrong_expectation_is_reported() {
    let document = load_json_file(&data_path("bench_shell/original_cases.json"));
    let document = dict(&document);
    let base_env = string_map(document.get("base_env").expect("base_env"));
    let base_files = string_map(document.get("base_files").expect("base_files"));
    let mut case = dict(&list(document.get("cases").expect("cases").clone())[0]).clone();
    case.set("stdout", PyValue::str("not what the original printed\n"));
    let scratch: PathBuf =
        std::env::temp_dir().join(format!("bench-shell-negative-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let problems = run_case(&case, &base_env, &base_files, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        problems.iter().any(|problem| problem.contains("stdout")),
        "{problems:?}"
    );
}

#[test]
fn wrong_expected_file_bytes_are_reported() {
    // The same first case with one expected file given other bytes: the byte comparison must name that file.
    let document = load_json_file(&data_path("bench_shell/original_cases.json"));
    let document = dict(&document);
    let base_env = string_map(document.get("base_env").expect("base_env"));
    let base_files = string_map(document.get("base_files").expect("base_files"));
    let mut case = dict(&list(document.get("cases").expect("cases").clone())[0]).clone();
    let mut after = dict(case.get("after").expect("after")).clone();
    let mut entry = dict(after.get("config.yaml").expect("config.yaml expectation")).clone();
    assert!(matches!(entry.get("content"), Some(PyValue::Str(_))));
    entry.set("content", PyValue::str("not the bytes the original left\n"));
    after.set("config.yaml", PyValue::Dict(entry));
    case.set("after", PyValue::Dict(after));
    let scratch: PathBuf =
        std::env::temp_dir().join(format!("bench-shell-negative-file-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let problems = run_case(&case, &base_env, &base_files, &scratch);
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains("config.yaml: file bytes")),
        "{problems:?}"
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn every_frozen_original_execution_is_reproduced() {
    let document = load_json_file(&data_path("bench_shell/original_cases.json"));
    let document = dict(&document);
    let base_env = string_map(document.get("base_env").expect("base_env"));
    let base_files = string_map(document.get("base_files").expect("base_files"));
    let cases = list(document.get("cases").expect("cases").clone());
    assert_eq!(
        cases.len(),
        181,
        "the frozen reference holds 181 executions"
    );
    let scratch: PathBuf =
        std::env::temp_dir().join(format!("bench-shell-original-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    let mut problems = Vec::new();
    for case in &cases {
        problems.extend(run_case(dict(case), &base_env, &base_files, &scratch));
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(
        problems.is_empty(),
        "{} problem(s):\n{}",
        problems.len(),
        problems.join("\n")
    );
}
