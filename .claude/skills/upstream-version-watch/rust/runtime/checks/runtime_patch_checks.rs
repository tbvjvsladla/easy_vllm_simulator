//! Native checks of `validate_runtime_patch` (the public command line of `validate_runtime_patch.py`), run against the real built executable.
//!
//! Two kinds of check:
//! - **fresh-input regressions** (`fresh_*`): small roots built here, the product run with explicit environments, the raw streams, first exit
//!   status and the whole file system read back. They never read the saved captures and always run.
//! - **saved-capture replay** (`replay_saved_captures`): the 80 saved observations of the original (8 stamps, 72 follow-ups) are rebuilt as
//!   FRESH roots from each case's `prepared.json` (bytes, modes, link targets), the executable is installed at the owned logical script path
//!   (direct execution, no interpreter), and its raw stdout, raw stderr, first exit status and whole file system are compared with the saved
//!   `stdout.bin` / `stderr.bin` / `execution.json` / `after.json` after one literal projection: the saved root prefix becomes the fresh root. Device
//!   and inode numbers are never compared across runs; only whether a path kept or replaced its inode. The only random text (the temporary file
//!   name of case B059) is compared as the exact owned `parent/prefix` followed by eight characters of `[a-z0-9_]`, and only there.
//!
//! The replay needs `UPSTREAM_R36_REFERENCE` (the capture directory) and `UPSTREAM_R36_REPLAY_OUT` (an empty directory for the fresh roots);
//! without them it reports NOT_RUN and passes nothing.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::PyValue;
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{symlink, MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, PoisonError};

/// Orders the installation of the executable under test and its launch across the tests of this file (`ETXTBSY` otherwise: see `run_in`).
static LAUNCH: Mutex<()> = Mutex::new(());

const ELF: &str = env!("CARGO_BIN_EXE_validate_runtime_patch");
const SCRIPT_RELATIVE: &str =
    "node/.claude/skills/upstream-version-watch/scripts/validate_runtime_patch.sh";
const RANDOM_ALPHABET: &str = "abcdefghijklmnopqrstuvwxyz0123456789_";

struct Observation {
    pid: u32,
    exit: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn install(root: &Path) -> PathBuf {
    let script = root.join(SCRIPT_RELATIVE);
    fs::create_dir_all(script.parent().unwrap()).unwrap();
    let _ = fs::remove_file(&script);
    fs::copy(ELF, &script).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn run_in(
    root: &Path,
    arguments: &[OsString],
    extra_env: &[(&str, &str)],
    closed_stdout: bool,
) -> Observation {
    // The copy's write descriptor must not exist in any process that forks while another test executes its own copy: held from the installation
    // until `spawn` returned (the child has then executed), so the tests still run in parallel. A poisoned lock guards no data, only the order.
    let launch = LAUNCH.lock().unwrap_or_else(PoisonError::into_inner);
    let script = install(root);
    fs::create_dir_all(root.join("private-tmp")).unwrap();
    let mut command = Command::new(&script);
    command
        .args(arguments)
        .current_dir(root)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8")
        .env("COLUMNS", "80")
        .env("TZ", "UTC")
        .env("TMPDIR", root.join("private-tmp"))
        .env("PYTHONHASHSEED", "0")
        .stderr(Stdio::piped());
    for (key, value) in extra_env {
        command.env(key, value);
    }
    if closed_stdout {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        command.stdout(Stdio::from(writer));
    } else {
        command.stdout(Stdio::piped());
    }
    // a freshly copied executable can be 'busy' (ETXTBSY) while another test thread forks; retry only that error
    let child = (0..50)
        .find_map(|_| match command.spawn() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                None
            }
            other => Some(other.unwrap()),
        })
        .expect("the program started");
    drop(launch);
    let pid = child.id();
    let output = child.wait_with_output().unwrap();
    Observation {
        pid,
        exit: output.status.code().unwrap_or(-1),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}

// ---------- fresh-input regressions ----------

fn scratch(name: &str) -> PathBuf {
    let base = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("runtime_patch_checks")
        .join(name);
    let _ = fs::remove_dir_all(&base);
    fs::create_dir_all(&base).unwrap();
    base
}

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
}

fn base_root(name: &str) -> PathBuf {
    let root = scratch(name);
    write(&root, "configs/m1_patch.py", b"print('patch one')\n");
    write(&root, "configs/m1.yaml", b"model: one\n");
    write(
        &root,
        "state/resolution.json",
        b"{\"vllm\": \"0.31.0\", \"unicode\": \"\xc3\xa9\"}\r\n",
    );
    fs::create_dir_all(root.join("envs")).unwrap();
    root
}

fn arguments(root: &Path, action: &str, topology: &str) -> Vec<OsString> {
    let join = |relative: &str| root.join(relative).into_os_string();
    vec![
        action.into(),
        "--patch".into(),
        join("configs/m1_patch.py"),
        "--topology".into(),
        topology.into(),
        "--config-dir".into(),
        join("configs"),
        "--env-dir".into(),
        join("envs"),
        "--resolution".into(),
        join("state/resolution.json"),
    ]
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn fresh_stamp_writes_the_sorted_sidecar_then_verify_passes_and_notices_every_bound_input() {
    let root = base_root("stamp_verify");
    let stamped = run_in(&root, &arguments(&root, "stamp", "multi"), &[], false);
    assert_eq!(stamped.exit, 0);
    let sidecar = root.join("configs/m1_patch.provenance.json");
    assert_eq!(text(&stamped.stdout), format!("{}\n", sidecar.display()));
    assert!(stamped.stderr.is_empty());
    let written = fs::read_to_string(&sidecar).unwrap();
    assert!(written.starts_with("{\n  \"canonical_resolution_sha256\": \""));
    assert!(written.ends_with("\n  \"topology\": \"multi\"\n}\n"));
    assert!(written.contains("\n  \"schema_version\": 1,\n"));
    assert!(written.contains("\"m1.yaml\": \""));
    assert!(!written.contains("m1.sh"));
    assert_eq!(fs::metadata(&sidecar).unwrap().mode() & 0o7777, 0o600);
    let leftovers: Vec<_> = fs::read_dir(root.join("configs"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers.len(), 3, "{leftovers:?}");
    let verified = run_in(&root, &arguments(&root, "verify", "multi"), &[], false);
    assert_eq!(
        (verified.exit, text(&verified.stdout)),
        (0, format!("PASS {}\n", sidecar.display()))
    );
    for (relative, bytes) in [
        ("configs/m1_patch.py", &b"print('patch two')\n"[..]),
        ("configs/m1.yaml", b"model: two\n"),
        ("state/resolution.json", b"{\"vllm\": \"0.31.1\"}\n"),
    ] {
        let original = fs::read(root.join(relative)).unwrap();
        fs::write(root.join(relative), bytes).unwrap();
        let stale = run_in(&root, &arguments(&root, "verify", "multi"), &[], false);
        assert_eq!(stale.exit, 2, "{relative}");
        assert_eq!(
            text(&stale.stderr),
            "runtime-patch-provenance FAIL: runtime patch provenance is stale or does not bind current topology/model inputs\n"
        );
        fs::write(root.join(relative), original).unwrap();
    }
    write(&root, "configs/m1.sh", b"MODEL=one\n");
    assert_eq!(
        run_in(&root, &arguments(&root, "verify", "multi"), &[], false).exit,
        2
    );
    fs::remove_file(root.join("configs/m1.sh")).unwrap();
    let other_topology = run_in(&root, &arguments(&root, "verify", "single"), &[], false);
    assert_eq!(other_topology.exit, 2);
}

#[test]
fn fresh_verify_compares_values_not_text_so_equal_numbers_and_a_duplicate_key_pass() {
    let root = base_root("equality");
    assert_eq!(
        run_in(&root, &arguments(&root, "stamp", "single"), &[], false).exit,
        0
    );
    let sidecar = root.join("configs/m1_patch.provenance.json");
    let original = fs::read_to_string(&sidecar).unwrap();
    let as_float = original.replace("\"schema_version\": 1", "\"schema_version\": 1.0");
    let as_bool = original.replace("\"schema_version\": 1", "\"schema_version\": true");
    // a duplicate key: the last one wins, as in the standard decoder
    let duplicated = original.replacen("\n}\n", ",\n  \"schema_version\": 1\n}\n", 1);
    for body in [as_float, as_bool, duplicated] {
        fs::write(&sidecar, body).unwrap();
        assert_eq!(
            run_in(&root, &arguments(&root, "verify", "single"), &[], false).exit,
            0
        );
    }
    fs::write(
        &sidecar,
        original.replace("\"schema_version\": 1", "\"schema_version\": 2"),
    )
    .unwrap();
    assert_eq!(
        run_in(&root, &arguments(&root, "verify", "single"), &[], false).exit,
        2
    );
}

#[test]
fn fresh_a_closed_standard_output_ends_in_status_120_after_the_sidecar_was_written() {
    let root = base_root("closed_stdout");
    let observed = run_in(&root, &arguments(&root, "stamp", "single"), &[], true);
    assert_eq!(observed.exit, 120);
    assert!(text(&observed.stderr).ends_with("BrokenPipeError: [Errno 32] Broken pipe\n"));
    assert!(root.join("configs/m1_patch.provenance.json").is_file());
}

#[test]
fn fresh_a_strict_ascii_stream_names_the_unencodable_run_and_other_profiles_are_refused() {
    let root = base_root("ascii");
    let accented = root.join("configs");
    let renamed = root.join("cafés");
    fs::rename(&accented, &renamed).unwrap();
    let mut arguments = arguments(&root, "stamp", "single");
    arguments[2] = renamed.join("m1_patch.py").into_os_string();
    arguments[6] = renamed.clone().into_os_string();
    let observed = run_in(
        &root,
        &arguments,
        &[("PYTHONIOENCODING", "ascii:strict")],
        false,
    );
    let shown = renamed.to_string_lossy().into_owned();
    let at = shown.find('\u{e9}').unwrap();
    assert_eq!(observed.exit, 2);
    assert_eq!(
        text(&observed.stderr),
        format!("runtime-patch-provenance FAIL: 'ascii' codec can't encode character '\\xe9' in position {at}: ordinal not in range(128)\n")
    );
    assert!(observed.stdout.is_empty());
    assert!(renamed.join("m1_patch.provenance.json").is_file());
    let refused = run_in(&root, &arguments, &[("PYTHONIOENCODING", "latin-1")], false);
    assert_eq!(refused.exit, 70);
    assert!(text(&refused.stderr)
        .starts_with("validate_runtime_patch.sh: not decided by this port (PYTHONIOENCODING)"));
}

#[test]
fn fresh_a_document_nested_beyond_the_recursion_limit_is_named_and_not_disguised() {
    let root = base_root("deep");
    write(
        &root,
        "state/resolution.json",
        "[".repeat(10_050).as_bytes(),
    );
    let observed = run_in(&root, &arguments(&root, "verify", "single"), &[], false);
    assert_eq!(observed.exit, 70);
    assert!(text(&observed.stderr).contains("not decided by this port (json_recursion)"));
}

#[test]
fn fresh_a_sidecar_directory_keeps_the_random_temporary_name_out_of_the_leftovers() {
    let root = base_root("sidecar_dir");
    fs::create_dir(root.join("configs/m1_patch.provenance.json")).unwrap();
    let observed = run_in(&root, &arguments(&root, "stamp", "single"), &[], false);
    assert_eq!(observed.exit, 2);
    let message = text(&observed.stderr);
    let prefix = format!(
        "runtime-patch-provenance FAIL: [Errno 21] Is a directory: '{}/configs/m1_patch.provenance.json.",
        root.display()
    );
    let tail = message.strip_prefix(&prefix).expect("operand prefix");
    assert!(tail[..8].chars().all(|ch| RANDOM_ALPHABET.contains(ch)));
    assert!(tail[8..].starts_with(&format!(
        "' -> '{}/configs/m1_patch.provenance.json'\n",
        root.display()
    )));
    let names: Vec<_> = fs::read_dir(root.join("configs"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 3, "no temporary file may remain: {names:?}");
}

#[test]
fn fresh_the_patch_must_resolve_under_the_configuration_directory_and_a_loop_is_the_uncaught_runtime_error(
) {
    let root = base_root("escape_loop");
    write(&root, "outside/other_patch.py", b"x\n");
    fs::remove_file(root.join("configs/m1_patch.py")).unwrap();
    symlink(
        "../outside/other_patch.py",
        root.join("configs/m1_patch.py"),
    )
    .unwrap();
    let escaped = run_in(&root, &arguments(&root, "verify", "single"), &[], false);
    assert_eq!(escaped.exit, 2);
    assert!(text(&escaped.stderr).starts_with(&format!(
        "runtime-patch-provenance FAIL: path escapes authoritative directory: {}/configs/m1_patch.py\n",
        root.display()
    )));
    fs::remove_file(root.join("configs/m1_patch.py")).unwrap();
    symlink("m1_patch.py", root.join("configs/m1_patch.py")).unwrap();
    let looped = run_in(&root, &arguments(&root, "verify", "single"), &[], false);
    assert_eq!(looped.exit, 1);
    let report = text(&looped.stderr);
    assert!(report.contains("OSError: [Errno 40] Too many levels of symbolic links:"));
    assert!(report
        .contains("\n\nDuring handling of the above exception, another exception occurred:\n\n"));
    assert!(report.contains(&format!(
        "File \"{}/{SCRIPT_RELATIVE}\", line 107, in <module>",
        root.display()
    )));
    assert!(report.ends_with(&format!(
        "RuntimeError: Symlink loop from '{}/configs/m1_patch.py'\n",
        root.display()
    )));
}

// ---------- saved-capture replay ----------

fn json(path: &Path) -> PyValue {
    let bytes = fs::read(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    loads(std::str::from_utf8(&bytes).unwrap())
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn field<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(dict) => dict.get(key).unwrap_or_else(|| panic!("missing {key}")),
        other => panic!("not an object: {other:?}"),
    }
}

fn string(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().expect("utf8 text"),
        other => panic!("not a string: {other:?}"),
    }
}

fn number(value: &PyValue) -> i64 {
    match value.peel() {
        PyValue::Int(number) => number.to_string().parse().expect("small integer"),
        other => panic!("not an integer: {other:?}"),
    }
}

fn items(value: &PyValue) -> &[PyValue] {
    match value.peel() {
        PyValue::List(items) => items,
        other => panic!("not a list: {other:?}"),
    }
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&text[at..at + 2], 16).unwrap())
        .collect()
}

/// One snapshot entry: kind, permission bits, bytes of a file or target of a link.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Entry {
    kind: String,
    mode: u32,
    content: Vec<u8>,
}

fn substitute(bytes: &[u8], from: &str, to: &str) -> Vec<u8> {
    text(bytes).replace(from, to).into_bytes()
}

/// Rebuilds a prepared snapshot in `root`: entries in saved order, directory modes applied last.
fn rebuild(root: &Path, prepared: &PyValue, from: &str) -> BTreeMap<String, u64> {
    let mut modes: Vec<(PathBuf, u32)> = Vec::new();
    for entry in items(prepared) {
        let relative = string(field(entry, "path"));
        let path = if relative == "." {
            root.to_path_buf()
        } else {
            root.join(&relative)
        };
        let mode = number(field(entry, "mode")) as u32;
        match string(field(entry, "kind")).as_str() {
            "dir" => {
                fs::create_dir_all(&path).unwrap();
                modes.push((path, mode));
            }
            "file" => {
                fs::write(&path, unhex(&string(field(entry, "hex")))).unwrap();
                fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
            }
            "symlink" => {
                let target = string(field(entry, "target")).replace(from, &root.to_string_lossy());
                symlink(target, &path).unwrap();
            }
            other => panic!("kind {other}"),
        }
    }
    for (path, mode) in modes.into_iter().rev() {
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
    }
    inodes(root)
}

fn inodes(root: &Path) -> BTreeMap<String, u64> {
    walk(root)
        .into_iter()
        .map(|(path, _, inode)| (path, inode))
        .collect::<Vec<_>>()
        .into_iter()
        .collect()
}

fn walk(root: &Path) -> Vec<(String, Entry, u64)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let relative = if relative.is_empty() {
            ".".to_string()
        } else {
            relative
        };
        let kind = metadata.file_type();
        let (name, content) = if kind.is_symlink() {
            (
                "symlink",
                fs::read_link(&path).unwrap().into_os_string().into_vec(),
            )
        } else if kind.is_dir() {
            ("dir", Vec::new())
        } else {
            ("file", fs::read(&path).unwrap())
        };
        found.push((
            relative,
            Entry {
                kind: name.to_string(),
                mode: metadata.mode() & 0o7777,
                content,
            },
            metadata.ino(),
        ));
        if kind.is_dir() {
            let mut children: Vec<_> = fs::read_dir(&path)
                .unwrap()
                .map(|e| e.unwrap().path())
                .collect();
            children.sort();
            pending.extend(children);
        }
    }
    found.sort_by(|left, right| left.0.cmp(&right.0));
    found
}

fn saved_entries(after: &PyValue, from: &str, to: &str) -> BTreeMap<String, Entry> {
    items(after)
        .iter()
        .map(|entry| {
            let kind = string(field(entry, "kind"));
            let content = match kind.as_str() {
                "file" => unhex(&string(field(entry, "hex"))),
                "symlink" => string(field(entry, "target"))
                    .replace(from, to)
                    .into_bytes(),
                _ => Vec::new(),
            };
            (
                string(field(entry, "path")),
                Entry {
                    kind,
                    mode: number(field(entry, "mode")) as u32,
                    content,
                },
            )
        })
        .collect()
}

/// Projects the one random operand of case B059 (exact owned parent and prefix, then eight characters of the alphabet).
fn project_random(stderr: &str, root: &str) -> String {
    let marker = format!("{root}/configs/model_patch.provenance.json.");
    let at = stderr
        .find(&marker)
        .expect("B059 holds its temporary operand")
        + marker.len();
    let name: String = stderr[at..at + 8].to_string();
    assert!(
        name.chars().all(|ch| RANDOM_ALPHABET.contains(ch)),
        "{name:?}"
    );
    format!(
        "{}<8 random characters>{}",
        &stderr[..at],
        &stderr[at + 8..]
    )
}

#[test]
fn replay_saved_captures() {
    let (Some(reference), Some(output)) = (
        std::env::var_os("UPSTREAM_R36_REFERENCE"),
        std::env::var_os("UPSTREAM_R36_REPLAY_OUT"),
    ) else {
        eprintln!("NOT_RUN: UPSTREAM_R36_REFERENCE and UPSTREAM_R36_REPLAY_OUT are not set");
        return;
    };
    let (reference, output) = (PathBuf::from(reference), PathBuf::from(output));
    let mut ids: Vec<String> = fs::read_dir(&reference)
        .unwrap()
        .filter_map(|entry| entry.unwrap().file_name().into_string().ok())
        .filter(|name| {
            name.len() == 4
                && (name.starts_with('A') || name.starts_with('B'))
                && name[1..].chars().all(|c| c.is_ascii_digit())
        })
        .collect();
    ids.sort();
    assert_eq!(ids.len(), 80, "the capture holds 80 observations");
    let mut exits: BTreeMap<i32, usize> = BTreeMap::new();
    let mut pids = Vec::new();
    let mut failures = Vec::new();
    for id in &ids {
        let case = reference.join(id);
        let execution = json(&case.join("execution.json"));
        let saved_root = string(field(&execution, "cwd"));
        let fresh = output.join(id).join("root");
        fs::create_dir_all(&fresh).unwrap();
        let fresh_text = fresh.to_string_lossy().into_owned();
        let before = rebuild(&fresh, &json(&case.join("prepared.json")), &saved_root);
        let argv: Vec<OsString> = items(field(&execution, "argv"))
            .iter()
            .skip(7)
            .map(|argument| OsString::from(string(argument).replace(&saved_root, &fresh_text)))
            .collect();
        let saved_env = match field(&execution, "env").peel() {
            PyValue::Dict(dict) => dict
                .iter()
                .map(|(k, v)| (k.to_utf8().unwrap(), string(v)))
                .collect::<Vec<_>>(),
            other => panic!("{other:?}"),
        };
        let extra: Vec<(&str, &str)> = saved_env
            .iter()
            .filter(|(key, _)| key == "PYTHONIOENCODING")
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let observed = run_in(&fresh, &argv, &extra, id == "B060");
        *exits.entry(observed.exit).or_default() += 1;
        pids.push(observed.pid);
        let mut problems = Vec::new();
        let saved_exit = number(field(&execution, "exit_code")) as i32;
        if observed.exit != saved_exit {
            problems.push(format!("exit {} != saved {saved_exit}", observed.exit));
        }
        let saved_stdout = substitute(
            &fs::read(case.join("stdout.bin")).unwrap(),
            &saved_root,
            &fresh_text,
        );
        if observed.stdout != saved_stdout {
            problems.push(format!(
                "stdout {:?} != saved {:?}",
                text(&observed.stdout),
                text(&saved_stdout)
            ));
        }
        let saved_stderr = substitute(
            &fs::read(case.join("stderr.bin")).unwrap(),
            &saved_root,
            &fresh_text,
        );
        let (got, want) = if id == "B059" {
            (
                project_random(&text(&observed.stderr), &fresh_text),
                project_random(&text(&saved_stderr), &fresh_text),
            )
        } else {
            (text(&observed.stderr), text(&saved_stderr))
        };
        if got != want {
            problems.push(format!("stderr {got:?} != saved {want:?}"));
        }
        let script = fresh.join(SCRIPT_RELATIVE);
        let _ = script;
        let mut expected = saved_entries(&json(&case.join("after.json")), &saved_root, &fresh_text);
        let mut actual: BTreeMap<String, Entry> = BTreeMap::new();
        let mut after_inodes = BTreeMap::new();
        for (path, entry, inode) in walk(&fresh) {
            after_inodes.insert(path.clone(), inode);
            actual.insert(path, entry);
        }
        // the installed executable stands in the saved interpreter script: its bytes and mode are the one projection
        expected.remove(SCRIPT_RELATIVE);
        actual.remove(SCRIPT_RELATIVE);
        if actual != expected {
            let paths: Vec<_> = actual.keys().chain(expected.keys()).collect();
            for path in paths {
                if actual.get(path) != expected.get(path) {
                    problems.push(format!(
                        "fs {path}: {:?} != saved {:?}",
                        actual
                            .get(path)
                            .map(|e| (&e.kind, e.mode, text(&e.content))),
                        expected
                            .get(path)
                            .map(|e| (&e.kind, e.mode, text(&e.content)))
                    ));
                    break;
                }
            }
        }
        // identity relation: which paths kept their inode; compared with the saved before/after records
        let saved_before: BTreeMap<String, i64> = items(&json(&case.join("prepared.json")))
            .iter()
            .map(|e| (string(field(e, "path")), number(field(e, "inode"))))
            .collect();
        let saved_after: BTreeMap<String, i64> = items(&json(&case.join("after.json")))
            .iter()
            .map(|e| (string(field(e, "path")), number(field(e, "inode"))))
            .collect();
        for (path, inode_after) in &after_inodes {
            if path == SCRIPT_RELATIVE {
                continue;
            }
            let fresh_kept = before.get(path).map(|inode| inode == inode_after);
            let saved_kept = saved_before
                .get(path)
                .zip(saved_after.get(path))
                .map(|(a, b)| a == b);
            if fresh_kept != saved_kept {
                problems.push(format!(
                    "inode relation of {path}: fresh {fresh_kept:?} saved {saved_kept:?}"
                ));
            }
        }
        if !problems.is_empty() {
            failures.push(format!("{id}: {}", problems.join(" | ")));
        }
    }
    pids.sort_unstable();
    pids.dedup();
    let summary = format!(
        "cases={} exits={exits:?} unique_pids={} failures={}",
        ids.len(),
        pids.len(),
        failures.len()
    );
    fs::write(
        output.join("summary.txt"),
        format!("{summary}\n{}\n", failures.join("\n")),
    )
    .unwrap();
    eprintln!("{summary}");
    assert!(failures.is_empty(), "{summary}\n{}", failures.join("\n"));
    assert_eq!(pids.len(), 80);
    assert_eq!(exits, BTreeMap::from([(0, 24), (1, 2), (2, 53), (120, 1)]));
}
