//! `campaign_read`: the native `campaign_init --active|--derive` process, checked against the frozen raw bytes of the ACTUAL
//! original CLI capture (`data/campaign_read/original26.json`, only the private fixture root replaced by `<ROOT>`), the absence
//! of any interpreter route, and the frozen source lines of the traceback frames (`data/campaign_read/owner_frame_lines.json`).
//! The retired original is never run or read: only its path matters (the checkout root is derived from it).
//!
//! Counts: 24 native-equality/native-error vectors run as native processes (P08 is rendered at the function level because its
//! fixture needs a Root-allocated shallow path; P09 was the declared legacy route of a modified original and is retired),
//! 25 vectors compared byte for byte.

#[path = "common.rs"]
mod common;

use common::{dict, hex_decode, list, load_json_file, text_of};
use role_contract::campaign::module_site;
use role_contract::topology::Frame;
use std::collections::BTreeMap;
use std::fs;
use std::io::{Cursor, Write};
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::campaign_read::{
    import_traceback, parse_exact, run, Environment, OBSERVER_ENV, OWNER_ENV, OWNER_FRAMES,
};
use sweep_stop::cli::Streams;
use sweep_stop::stdout::StdoutProfile;

const BIN: &str = env!("CARGO_BIN_EXE_campaign_read");
const OWNER_REL: &str = ".claude/skills/terraforming_node/scripts/campaign_init.py"; // original capture conditions (frozen raw)
/// The bytes a fixture puts at the program path: the retired original is never read, so any content is the same program.
const PLACEHOLDER_OWNER: &[u8] = b"# retired campaign_init.py: only this path matters to the native program\n";

struct Vector {
    id: String,
    args: Vec<String>,
    env: Vec<(String, String)>,
    owner: String,
    dirs: Vec<(String, u32)>,
    files: Vec<(String, Vec<u8>, u32)>,
    opaque: Vec<(String, u32)>,
    exit: i32,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// Directories (with modes) and files (with bytes and modes) of a literal fixture.
struct TreeSpec {
    dirs: Vec<(String, u32)>,
    files: Vec<(String, Vec<u8>, u32)>,
}

struct Data {
    other: TreeSpec,
    append: Vec<u8>,
    vectors: Vec<Vector>,
}

fn mode(text: &str) -> u32 {
    u32::from_str_radix(text, 8).expect("octal mode")
}

fn strings(value: &sweep_stop::pyvalue::PyValue) -> Vec<String> {
    list(value.clone())
        .iter()
        .map(|v| text_of(Some(v)))
        .collect()
}

fn spec(d: &sweep_stop::pyvalue::PyDict) -> TreeSpec {
    let dirs = list(d.get("dirs").expect("dirs").clone())
        .iter()
        .map(|row| {
            let r = list(row.clone());
            (text_of(r.first()), mode(&text_of(r.get(1))))
        })
        .collect();
    let files = list(d.get("files").expect("files").clone())
        .iter()
        .map(|row| {
            let r = list(row.clone());
            (
                text_of(r.first()),
                hex_decode(&text_of(r.get(1))),
                mode(&text_of(r.get(2))),
            )
        })
        .collect();
    TreeSpec { dirs, files }
}

fn data() -> Data {
    let doc = load_json_file(&common::data_path("campaign_read/original26.json"));
    let root = dict(&doc);
    let vectors = list(root.get("vectors").expect("vectors").clone())
        .iter()
        .map(|item| {
            let d = dict(item);
            let TreeSpec { dirs, files } = spec(d);
            let pairs = |key: &str| -> Vec<(String, String)> {
                list(d.get(key).expect(key).clone())
                    .iter()
                    .map(|p| {
                        let r = list(p.clone());
                        (text_of(r.first()), text_of(r.get(1)))
                    })
                    .collect()
            };
            Vector {
                id: text_of(d.get("id")),
                args: strings(d.get("args").expect("args")),
                env: pairs("env"),
                owner: text_of(d.get("owner")),
                dirs,
                files,
                opaque: pairs("opaque")
                    .into_iter()
                    .map(|(p, m)| (p, mode(&m)))
                    .collect(),
                exit: text_of(d.get("exit")).parse().expect("exit"),
                stdout: hex_decode(&text_of(d.get("stdout_hex"))),
                stderr: hex_decode(&text_of(d.get("stderr_hex"))),
            }
        })
        .collect();
    Data {
        other: spec(dict(root.get("other_root").expect("other_root"))),
        append: hex_decode(&text_of(root.get("p09_append_hex"))),
        vectors,
    }
}

// ── fixtures ─────────────────────────────────────────────────────────────────────────

fn make_writable(path: &Path) {
    if let Ok(meta) = fs::symlink_metadata(path) {
        if meta.is_dir() {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    make_writable(&entry.path());
                }
            }
        } else if !meta.file_type().is_symlink() {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
        }
    }
}

fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("campaign_read")
        .join(format!("{name}-{}", std::process::id()));
    if path.exists() {
        make_writable(&path);
        fs::remove_dir_all(&path).expect("clear scratch");
    }
    fs::create_dir_all(&path).expect("scratch");
    fs::canonicalize(&path).expect("canonical scratch")
}

fn put(root: &Path, dirs: &[(String, u32)], files: &[(String, Vec<u8>, u32)]) {
    fs::create_dir_all(root).expect("root");
    for (rel, m) in dirs {
        fs::create_dir_all(root.join(rel)).expect("dir");
        fs::set_permissions(root.join(rel), fs::Permissions::from_mode(*m)).expect("dir mode");
    }
    for (rel, bytes, m) in files {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("parent");
        fs::write(&p, bytes).expect("file");
        fs::set_permissions(&p, fs::Permissions::from_mode(*m)).expect("file mode");
    }
}

fn install_owner(root: &Path, bytes: &[u8]) {
    put(root, &[], &[(OWNER_REL.to_string(), bytes.to_vec(), 0o775)]);
}

/// Builds the fixture of one vector under `base` and returns (root, other root if any).
fn build(base: &Path, v: &Vector, d: &Data) -> (PathBuf, Option<PathBuf>) {
    let root = base.join(&v.id).join("root");
    put(&root, &v.dirs, &v.files);
    let mut other = None;
    match v.owner.as_str() {
        "copy" => install_owner(&root, PLACEHOLDER_OWNER),
        "variant" => {
            let mut bytes = PLACEHOLDER_OWNER.to_vec();
            bytes.extend_from_slice(&d.append);
            install_owner(&root, &bytes);
        }
        "symlink_other" => {
            let o = base.join(&v.id).join("other-root");
            put(&o, &d.other.dirs, &d.other.files);
            install_owner(&o, PLACEHOLDER_OWNER);
            let link = root.join(OWNER_REL);
            fs::create_dir_all(link.parent().expect("parent")).expect("link dir");
            symlink(o.join(OWNER_REL), &link).expect("owner symlink");
            other = Some(o);
        }
        "symlink_shallow" => {}
        other => panic!("unknown owner kind {other}"),
    }
    let mut opaque = v.opaque.clone();
    opaque.sort_by_key(|(p, _)| std::cmp::Reverse(Path::new(p).components().count()));
    for (rel, m) in opaque {
        fs::set_permissions(root.join(rel), fs::Permissions::from_mode(m)).expect("opaque mode");
    }
    (root, other)
}

/// Metadata, links and readable bytes of a tree; an unreadable directory is recorded by its stat and not entered.
fn snapshot(root: &Path) -> BTreeMap<String, String> {
    fn visit(p: &Path, root: &Path, out: &mut BTreeMap<String, String>) {
        let rel = p.strip_prefix(root).unwrap().display().to_string();
        let meta = fs::symlink_metadata(p).expect("lstat");
        let m = meta.permissions().mode();
        let line = if meta.file_type().is_symlink() {
            format!("link {:o} -> {}", m, fs::read_link(p).unwrap().display())
        } else if meta.is_dir() {
            format!(
                "dir {m:o} mtime {}",
                meta.modified()
                    .map(|t| format!("{t:?}"))
                    .unwrap_or_default()
            )
        } else {
            match fs::read(p) {
                Ok(bytes) => format!(
                    "file {m:o} {} {}",
                    bytes.len(),
                    bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
                ),
                Err(e) => format!("file {m:o} unreadable {:?}", e.raw_os_error()),
            }
        };
        out.insert(rel, line);
        if meta.is_dir() {
            if let Ok(entries) = fs::read_dir(p) {
                let mut names: Vec<_> = entries.flatten().map(|e| e.path()).collect();
                names.sort();
                for n in names {
                    visit(&n, root, out);
                }
            }
        }
    }
    let mut out = BTreeMap::new();
    visit(root, root, &mut out);
    out
}

fn observer_lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .map(|t| t.lines().map(str::to_string).collect())
        .unwrap_or_default()
}

struct Ran {
    status: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    pid: u32,
}

fn spawn(
    program: &str,
    args: &[String],
    env: &[(String, String)],
    cwd: &Path,
    stdin: &[u8],
) -> Ran {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let pid = child.id();
    child.stdin.take().unwrap().write_all(stdin).ok();
    let out = child.wait_with_output().expect("wait");
    Ran {
        status: out.status.code(),
        stdout: out.stdout,
        stderr: out.stderr,
        pid,
    }
}

fn with_root(bytes: &[u8], root: &Path) -> Vec<u8> {
    let token = b"<ROOT>";
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i..].starts_with(token) {
            out.extend_from_slice(root.as_os_str().as_encoded_bytes());
            i += token.len();
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

fn expected_outcome(id: &str) -> &'static str {
    match id {
        "P01" => "encode",
        "P04" | "P05" | "P06" => "refusal",
        _ if id.starts_with('N') => "success",
        _ => "crash",
    }
}

// ── the frozen original26 comparison ─────────────────────────────────────────────────

#[test]
fn native_cases_match_the_frozen_original_raw_bytes_and_leave_the_fixture_unchanged() {
    let d = data();
    assert_eq!(d.vectors.len(), 26, "the declared 26 controls");
    let base = scratch("original26");
    let mut compared = 0;
    for v in d.vectors.iter().filter(|v| v.id != "P08" && v.id != "P09") {
        let (root, other) = build(&base, v, &d);
        let owner = root.join(OWNER_REL);
        let observer = base.join(format!("{}.jsonl", v.id));
        let mut env = v.env.clone();
        env.push((OWNER_ENV.into(), owner.display().to_string()));
        env.push((OBSERVER_ENV.into(), observer.display().to_string()));
        let (before, other_before) = (snapshot(&root), other.as_ref().map(|o| snapshot(o)));
        let ran = spawn(BIN, &v.args, &env, &root, b"");
        assert_eq!(ran.status, Some(v.exit), "{} exit", v.id);
        assert_eq!(
            String::from_utf8_lossy(&ran.stdout),
            String::from_utf8_lossy(&with_root(&v.stdout, &root)),
            "{} stdout",
            v.id
        );
        assert_eq!(
            String::from_utf8_lossy(&ran.stderr),
            String::from_utf8_lossy(&with_root(&v.stderr, &root)),
            "{} stderr",
            v.id
        );
        assert_eq!(
            ran.stdout,
            with_root(&v.stdout, &root),
            "{} stdout bytes",
            v.id
        );
        assert_eq!(
            ran.stderr,
            with_root(&v.stderr, &root),
            "{} stderr bytes",
            v.id
        );
        assert_eq!(snapshot(&root), before, "{} fixture changed", v.id);
        assert_eq!(
            other.as_ref().map(|o| snapshot(o)),
            other_before,
            "{} other root changed",
            v.id
        );
        let lines = observer_lines(&observer);
        assert_eq!(lines.len(), 1, "{} one observer line", v.id);
        for needle in [
            "\"route\": \"NativeCurrentSource\"".to_string(),
            format!("\"native_outcome\": \"{}\"", expected_outcome(&v.id)),
            format!("\"pid\": {}", ran.pid),
        ] {
            assert!(
                lines[0].contains(&needle),
                "{} observer {needle}: {}",
                v.id,
                lines[0]
            );
        }
        compared += 1;
    }
    assert_eq!(
        compared, 24,
        "24 native processes (P08 function-level, P09 legacy route)"
    );
    make_writable(&base);
}

#[test]
fn p08_import_traceback_matches_the_frozen_original_raw_bytes() {
    let d = data();
    let v = d.vectors.iter().find(|v| v.id == "P08").expect("P08");
    let crash =
        module_site(b"/o/campaign_init.py").expect_err("a path with fewer than five components");
    assert_eq!(
        (crash.class, crash.message.to_utf8().as_deref()),
        ("IndexError", Some("4"))
    );
    let root = Path::new("/private/fixture/P08/root");
    let owner = root.join(OWNER_REL);
    let text = import_traceback(owner.as_os_str().as_encoded_bytes(), &crash);
    let got = text.to_surrogateescape_bytes().expect("encodable");
    assert_eq!(
        String::from_utf8_lossy(&got),
        String::from_utf8_lossy(&with_root(&v.stderr, root))
    );
    assert_eq!(got, with_root(&v.stderr, root));
    assert!(
        module_site(b"/a/b/c/d/e/f.py").is_ok(),
        "six components resolve"
    );
}

// ── no interpreter route ──────────────────────────────────────────────────────────────

fn stub(dir: &Path) -> PathBuf {
    let path = dir.join("stub.sh");
    fs::write(&path, "#!/bin/sh\nprintf '%s' \"$$\" > \"$STUB_OUT.pid\"\nexit 7\n").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn fixture_env(base: &Path, root: &Path, extra: &[(&str, String)]) -> Vec<(String, String)> {
    let mut env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("LC_ALL".to_string(), "C.UTF-8".to_string()),
        ("STUB_OUT".to_string(), base.join("stub").display().to_string()),
        (OWNER_ENV.to_string(), root.join(OWNER_REL).display().to_string()),
    ];
    env.extend(extra.iter().map(|(k, v)| (k.to_string(), v.clone())));
    env
}

#[test]
fn no_shape_and_no_variable_starts_an_interpreter() {
    let base = scratch("no-interpreter");
    let root = base.join("root");
    put(&root, &[], &[("campaigns/ACTIVE".to_string(), b"camp-x\n".to_vec(), 0o664)]);
    fs::create_dir_all(root.join("campaigns/camp-x")).unwrap();
    let stub = stub(&base).display().to_string();
    // the program file itself is absent: the native answer does not need it
    for extra in [
        vec![],
        vec![("CAMPAIGN_READ_PYTHON", stub.clone())],
        vec![("CAMPAIGN_READ_COMPAT_PYTHON", stub.clone())],
        vec![("LC_ALL", "en_US.ISO-8859-1".to_string())],
    ] {
        let ran = spawn(BIN, &["--active".to_string()], &fixture_env(&base, &root, &extra), &root, b"");
        assert_eq!(ran.status, Some(0), "{extra:?} {}", String::from_utf8_lossy(&ran.stderr));
        assert_eq!(ran.stdout, b"camp-x\n", "{extra:?}");
        assert!(!base.join("stub.pid").exists(), "no interpreter is started ({extra:?})");
    }
    // a vector that is not a read shape is a control failure of this program (the full command line is `campaign_init`)
    let ran = spawn(BIN, &["--resume-brief".to_string()], &fixture_env(&base, &root, &[]), &root, b"");
    assert_eq!(ran.status, Some(2));
    assert!(ran.stdout.is_empty());
    assert!(String::from_utf8_lossy(&ran.stderr).starts_with("[campaign_read] ERROR not a read shape"));
    assert!(!base.join("stub.pid").exists());
    make_writable(&base);
}

#[test]
fn control_failures_are_private_status_2_and_produce_no_business_output() {
    let base = scratch("observer-bad");
    let root = base.join("root");
    install_owner(&root, PLACEHOLDER_OWNER);
    let ran = spawn(
        BIN,
        &["--active".to_string()],
        &fixture_env(&base, &root, &[(OBSERVER_ENV, "/nonexistent-dir/obs.jsonl".to_string())]),
        &root,
        b"",
    );
    assert_eq!(ran.status, Some(2));
    assert!(ran.stdout.is_empty());
    assert!(String::from_utf8_lossy(&ran.stderr)
        .starts_with("[campaign_read] ERROR the route observation cannot be written"));
    // no program path configured
    let base = scratch("no-owner");
    let env = vec![("PATH".to_string(), "/usr/bin:/bin".to_string())];
    let ran = spawn(BIN, &["--active".to_string()], &env, &base, b"");
    assert_eq!(ran.status, Some(2));
    assert!(String::from_utf8_lossy(&ran.stderr).contains("CAMPAIGN_READ_COMPAT_OWNER is not set"));
}

// ── grammar ──────────────────────────────────────────────────────────────────────────

fn bytes_of(args: &[&str]) -> Vec<Vec<u8>> {
    args.iter().map(|a| a.as_bytes().to_vec()).collect()
}

#[test]
fn the_grammar_handles_exactly_the_declared_shapes() {
    let native: [&[&str]; 5] = [
        &["--active"],
        &["--derive", "root"],
        &["--derive", "cell-status", "--cell", "k1"],
        &[
            "--derive",
            "config",
            "--cell",
            "c1",
            "--campaign-id",
            "camp-y",
        ],
        &[
            "--derive",
            "relay",
            "--context",
            "ctx",
            "--sweep",
            "s",
            "--cell",
            "c",
            "--campaign-id",
            "y",
        ],
    ];
    for args in native {
        assert!(parse_exact(&bytes_of(args)).is_some(), "{args:?}");
    }
    let python: [&[&str]; 19] = [
        &["--active", "--derive", "root"],
        &["--derive=root"],
        &["--deriv", "root"],
        &["--derive"],
        &["--derive", ""],
        &["--derive", "root", "--cell", ""],
        &["--derive", "root", "--cell", "a", "--cell", "b"],
        &["--derive", "root", "--cell", "-x"],
        &["--derive", "root", "--cell", "a b"],
        &["--derive", "root", "--cell", "é"],
        &["--active", "--apply"],
        &["--act"],
        &[],
        &["--resume-brief"],
        &["--derive", "root", "--unknown"],
        &["--derive", "root", "extra"],
        &["--derive", "-x"],
        &["--active", "--active"],
        &["--derive", "root", "--campaign-id"],
    ];
    for args in python {
        assert!(parse_exact(&bytes_of(args)).is_none(), "{args:?}");
    }
    assert!(parse_exact(&[
        b"--derive".to_vec(),
        b"root".to_vec(),
        b"--cell".to_vec(),
        vec![b'a', 0xff]
    ])
    .is_none());
}

// ── write failure is modelled natively, never retried in Python ─────────────────────

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::from_raw_os_error(32))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::from_raw_os_error(32))
    }
}

#[test]
fn a_failing_stdout_is_a_native_status_and_no_python_retry() {
    let base = scratch("broken-stdout");
    let root = base.join("root");
    install_owner(&root, PLACEHOLDER_OWNER);
    let environment = Environment {
        owner: Some(root.join(OWNER_REL).as_os_str().as_encoded_bytes().to_vec()),
        text_variables: vec![("LC_ALL".into(), b"C.UTF-8".to_vec())],
        ..Environment::default()
    };
    let (mut stdin, mut stdout, mut stderr) = (Cursor::new(Vec::new()), Broken, Vec::new());
    let status = run(
        &bytes_of(&["--active"]),
        &environment,
        &mut Streams::new(
            &mut stdin,
            &mut stdout,
            &mut stderr,
            StdoutProfile::default(),
        ),
    );
    assert_eq!(
        status, 120,
        "the buffered flush at exit fails: CPython's status 120"
    );
    assert!(
        String::from_utf8_lossy(&stderr).starts_with("Exception ignored in:"),
        "{}",
        String::from_utf8_lossy(&stderr)
    );
}

// ── source bindings of the frame tables ──────────────────────────────────────────────

#[test]
fn every_owner_frame_row_is_bound_to_the_frozen_original_lines() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/campaign_read/owner_frame_lines.json");
    let frozen = load_json_file(&path);
    let frozen = dict(&frozen);
    assert!(frozen.get("line_count").is_some(), "the frozen data records the original's length");
    let lines = dict(frozen.get("lines").expect("lines"));
    for frame in OWNER_FRAMES {
        assert!(lines.get(&frame.line.to_string()).is_some(), "a frozen line for frame {}", frame.line);
        let line = text_of(lines.get(&frame.line.to_string()));
        assert_eq!(
            line.trim(),
            frame.code,
            "line {}",
            frame.line
        );
        assert!(
            frame.expr.is_empty() || frame.code.contains(frame.expr),
            "{}",
            frame.expr
        );
    }
}

#[test]
fn stdlib_frame_constants_match_the_python_3_12_pathlib_source() {
    let path = "/usr/lib/python3.12/pathlib.py";
    let text =
        fs::read_to_string(path).unwrap_or_else(|e| panic!("NOT_RUN: {path} unreadable: {e}"));
    let lines: Vec<&str> = text.lines().collect();
    let groups = [
        Frame::pathlib_open(),
        Frame::pathlib_read(),
        Frame::pathlib_is_file(),
        Frame::pathlib_is_dir(),
        Frame::pathlib_parents_index(),
    ];
    let mut checked = 0;
    for frame in groups
        .iter()
        .flatten()
        .filter(|f| f.file == path.as_bytes())
    {
        assert_eq!(
            lines[frame.line as usize - 1].trim(),
            frame.code,
            "pathlib.py line {}",
            frame.line
        );
        assert!(frame.expr.is_empty() || frame.code.contains(frame.expr));
        checked += 1;
    }
    assert_eq!(
        checked, 8,
        "open 2 + read 1 (the codecs frame is frozen) + is_file 2 + is_dir 2 + parents 1"
    );
}

// ── launcher ─────────────────────────────────────────────────────────────────────────

fn launcher() -> PathBuf {
    fs::canonicalize(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../terraforming_node/scripts/campaign_init.sh"),
    )
    .expect("launcher")
}

fn run_launcher(name: &str, bin: Option<&Path>, python: Option<&str>) -> (Ran, PathBuf) {
    let base = scratch(name);
    let mut env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("STUB_OUT".into(), base.join("stub").display().to_string()),
        ("MARKER".into(), "x".into()),
    ];
    if let Some(b) = bin {
        env.push(("CAMPAIGN_READ_BIN".into(), b.display().to_string()));
    }
    if let Some(p) = python {
        env.push((
            "CAMPAIGN_READ_PYTHON".into(),
            p.replace("<STUB>", &stub(&base).display().to_string()),
        ));
    }
    let args = vec![
        launcher().display().to_string(),
        "--derive".into(),
        "root".into(),
        "--campaign-id".into(),
        "camp-y".into(),
    ];
    (spawn("bash", &args, &env, &base, b""), base)
}

#[test]
fn the_launcher_reports_a_missing_or_non_executable_binary_before_any_route() {
    let missing = scratch("launcher-missing").join("no-binary");
    for python in [None, Some("<STUB>")] {
        let (ran, base) = run_launcher("launcher-missing", Some(&missing), python);
        assert_eq!(ran.status, Some(2));
        assert!(ran.stdout.is_empty());
        let err = String::from_utf8_lossy(&ran.stderr);
        assert!(
            err.starts_with("[campaign_init] ERROR Rust 실행파일이 없다/실행 불가: "),
            "{err}"
        );
        assert!(err.contains("build_rust.sh"), "{err}");
        assert!(
            !base.join("stub.pid").exists(),
            "the interpreter is never started"
        );
    }
    let base = scratch("launcher-nonexec");
    let plain = base.join("campaign_read");
    fs::write(&plain, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
    let (ran, _) = run_launcher("launcher-nonexec2", Some(&plain), Some("<STUB>"));
    assert_eq!(ran.status, Some(2));
}

#[test]
fn the_launcher_exports_the_program_path_beside_it_and_never_an_interpreter() {
    let base = scratch("launcher-pass");
    let recorder = base.join("campaign_read");
    fs::write(&recorder, "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$STUB_OUT.argv\"\nprintf '%s|%s' \"$CAMPAIGN_READ_COMPAT_OWNER\" \"${CAMPAIGN_READ_PYTHON-unset}\" > \"$STUB_OUT.env\"\n").unwrap();
    fs::set_permissions(&recorder, fs::Permissions::from_mode(0o755)).unwrap();
    let owner = launcher()
        .with_file_name("campaign_init.sh")
        .display()
        .to_string();
    for (python, want) in [
        (None, "unset"),
        (Some(""), "unset"),
        (Some("/x/python"), "unset"),
    ] {
        let (ran, dir) = run_launcher("launcher-pass2", Some(&recorder), python);
        assert_eq!(ran.status, Some(0));
        assert_eq!(
            fs::read_to_string(dir.join("stub.argv")).unwrap(),
            "--derive\nroot\n--campaign-id\ncamp-y\n"
        );
        assert_eq!(
            fs::read_to_string(dir.join("stub.env")).unwrap(),
            format!("{owner}|{want}")
        );
    }
}

// ── availability guard (CAMPAIGN_READ_VALIDATE_ONLY) ────────────────────────────────

fn launcher_run(name: &str, bin: Option<&Path>, extra: &[(&str, &str)]) -> (Ran, PathBuf) {
    let base = scratch(name);
    let mut env = vec![
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("STUB_OUT".into(), base.join("stub").display().to_string()),
        ("MARKER".into(), "x".into()),
    ];
    if let Some(b) = bin {
        env.push(("CAMPAIGN_READ_BIN".into(), b.display().to_string()));
    }
    env.extend(extra.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    let args = vec![launcher().display().to_string(), "--active".into()];
    (spawn("bash", &args, &env, &base, b""), base)
}

fn recorder(dir: &Path) -> PathBuf {
    let path = dir.join("campaign_read");
    fs::write(
        &path,
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$STUB_OUT.argv\"\nprintf '%s' \"${CAMPAIGN_READ_VALIDATE_ONLY-unset}\" > \"$STUB_OUT.validate\"\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[test]
fn validate_only_stops_on_a_missing_or_non_executable_runtime_and_runs_no_business_on_a_valid_one()
{
    // missing and non-executable: visible guidance, status 2, nothing started (even with an explicit interpreter)
    let base = scratch("validate-missing");
    let missing = base.join("none");
    let plain = base.join("plain");
    fs::write(&plain, "#!/bin/sh\nexit 0\n").unwrap();
    fs::set_permissions(&plain, fs::Permissions::from_mode(0o644)).unwrap();
    for bin in [&missing, &plain] {
        let (ran, dir) = launcher_run(
            "validate-missing2",
            Some(bin),
            &[
                ("CAMPAIGN_READ_VALIDATE_ONLY", "1"),
                ("CAMPAIGN_READ_PYTHON", "/nonexistent/python"),
            ],
        );
        assert_eq!(ran.status, Some(2));
        assert!(ran.stdout.is_empty());
        let err = String::from_utf8_lossy(&ran.stderr);
        assert!(
            err.starts_with("[campaign_init] ERROR Rust 실행파일이 없다/실행 불가: ")
                && err.contains("build_rust.sh"),
            "{err}"
        );
        assert!(!dir.join("stub.argv").exists());
    }
    // valid runtime: 0, no output, the binary is not started
    let dir = scratch("validate-valid");
    let bin = recorder(&dir);
    let (ran, dir) = launcher_run(
        "validate-valid2",
        Some(&bin),
        &[("CAMPAIGN_READ_VALIDATE_ONLY", "1")],
    );
    assert_eq!(ran.status, Some(0));
    assert!(ran.stdout.is_empty() && ran.stderr.is_empty());
    assert!(
        !dir.join("stub.argv").exists(),
        "validate-only runs no actor"
    );
    // only the value 1 validates; anything else routes as before and the variable does not reach the binary
    for value in ["0", "", "yes"] {
        let (ran, dir) = launcher_run(
            "validate-other",
            Some(&bin),
            &[("CAMPAIGN_READ_VALIDATE_ONLY", value)],
        );
        assert_eq!(ran.status, Some(0));
        assert_eq!(
            fs::read_to_string(dir.join("stub.argv")).unwrap(),
            "--active\n",
            "value {value:?}"
        );
        assert_eq!(
            fs::read_to_string(dir.join("stub.validate")).unwrap(),
            "unset"
        );
    }
}

fn skills_dir() -> PathBuf {
    fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")).expect("skills dir")
}

#[test]
fn fetch_sub_docs_validates_the_runtime_before_any_remote_access() {
    let script = skills_dir().join("upstream-version-watch/scripts/fetch_sub_docs.sh");
    let base = scratch("fetch-guard");
    let src = base.join("src");
    let scripts = src.join(".claude/skills/terraforming_node/scripts");
    fs::create_dir_all(&scripts).unwrap();
    fs::copy(launcher(), scripts.join("campaign_init.sh")).unwrap();
    let bin_dir = base.join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    for tool in ["ssh", "rsync"] {
        let stub = bin_dir.join(tool);
        fs::write(
            &stub,
            format!(
                "#!/bin/sh\necho {tool} >> \"{}\"\n",
                base.join("remote-calls").display()
            ),
        )
        .unwrap();
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let run_fetch = |bin: Option<&Path>| {
        let mut env = vec![
            (
                "PATH".to_string(),
                format!("{}:/usr/bin:/bin", bin_dir.display()),
            ),
            ("SRC".into(), format!("{}/", src.display())),
            ("STUB_OUT".into(), base.join("stub").display().to_string()),
        ];
        if let Some(b) = bin {
            env.push(("CAMPAIGN_READ_BIN".into(), b.display().to_string()));
        }
        spawn("bash", &[script.display().to_string()], &env, &base, b"")
    };
    let ran = run_fetch(None);
    assert_eq!(ran.status, Some(2));
    let err = String::from_utf8_lossy(&ran.stderr);
    assert!(
        err.contains("[campaign_init] ERROR Rust 실행파일이 없다/실행 불가")
            && err.contains("[fetch] ERROR 캠페인 읽기 런타임"),
        "{err}"
    );
    assert!(
        !base.join("remote-calls").exists(),
        "no remote access after a failed availability check"
    );
    // a present runtime: the guard passes silently and runs no business (the script then continues on its own terms)
    let good = recorder(&base);
    let ran = run_fetch(Some(&good));
    assert!(
        !String::from_utf8_lossy(&ran.stderr).contains("캠페인 읽기 런타임"),
        "{}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(
        !base.join("stub.argv").exists(),
        "validate-only started no actor"
    );
}

#[test]
fn broad_search_validates_the_runtime_once_before_its_first_campaign_read_and_side_effect() {
    let text =
        fs::read_to_string(skills_dir().join("adversarial-benchmark/scripts/broad_search.sh"))
            .expect("broad_search.sh");
    let validate: Vec<usize> = text
        .match_indices("CAMPAIGN_READ_VALIDATE_ONLY=1 bash")
        .map(|(i, _)| i)
        .collect();
    assert_eq!(validate.len(), 1, "exactly one availability call");
    let at = validate[0];
    let line = text[at..].lines().next().unwrap();
    assert!(!line.contains("2>/dev/null"), "{line}");
    let cell = text.find("\ncell)\n").expect("cell branch");
    assert!(cell < at, "inside the cell branch");
    for later in [
        "  _resolve_topo\n",
        "\"$SDIR/sweep_bench.sh\"",
        "set +e; _stop; STOPRC=$?",
        "_CI_PC=",
        "\"$_CI_PC\" --active",
        "\"$_CI\" --derive",
        "bench_shell.sh\" broad-cell",
        "curl -s -m 5",
    ] {
        let found = text[cell..]
            .find(later)
            .map(|i| i + cell)
            .unwrap_or_else(|| panic!("{later} missing"));
        assert!(at < found, "the guard precedes {later:?}");
    }
    assert!(
        text[at..].contains("exit \"$_CIV_RC\""),
        "the failure status is propagated"
    );
}
