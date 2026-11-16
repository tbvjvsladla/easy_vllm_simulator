//! `manifest_contract` checks: the exact grammar and the routes, the owner-source binding of every frame row, and the frozen
//! ORIGINAL190 capture replayed through the native library on owned synthetic fixtures (no Git process, no Python).
//!
//! The golden rows are the literal stdout/stderr bytes and first exit status of the one actual original run (Python 3.12.3,
//! PyYAML 6.0.1); only the private fixture root and the owner clone path are tokens. The branch the original's `git` printed
//! is stood in for by a probe that reads the lane from the fixture group a repo path resolves into, so a symlinked repo
//! behaves as it does under Git. A row that needs a mode-000 entry is skipped (and said so) when the process is not
//! restricted by file modes. The native process-level capture on the real Git fixtures is a separate, later cohort.

use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use role_contract::manifest_gate::{BranchProbe, OWNER_FRAMES};
use role_contract::manifest_gate_cli::{execute, parse_exact, print_failure, MAIN_FRAMES};
use role_contract::pyprofile::{encode_stream, with_profile, Errors, TextProfile};
use role_contract::source_identity::REVIEWED_MANIFEST_CONTRACT;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use sweep_stop::manifest_contract::{choose_route, Environment, Route};

fn bytes(items: &[&str]) -> Vec<Vec<u8>> {
    items.iter().map(|item| item.as_bytes().to_vec()).collect()
}

fn environment(owner: Option<&Path>, variables: &[(&str, &str)]) -> Environment {
    Environment {
        owner: owner.map(|path| path.as_os_str().as_bytes().to_vec()),
        observer: None,
        text_variables: variables
            .iter()
            .map(|(name, value)| (name.to_string(), value.as_bytes().to_vec()))
            .collect(),
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("manifest-contract-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

// ── grammar and routes ──────────────────────────────────────────────────────────────────

#[test]
fn exact_grammar_accepts_only_the_handled_shapes() {
    let handled: [&[&str]; 6] = [
        &["--topology", "single", "--repo", "/r", "--require-flag"],
        &[
            "--require-flag",
            "--repo",
            "/r",
            "--topology",
            "multi",
            "--json",
        ],
        &["--topology", "single"],
        &["--repo", ""],
        &["--repo", "relative dir/x"],
        &[],
    ];
    for shape in handled {
        assert!(
            parse_exact(&bytes(shape)).is_some(),
            "native shape {shape:?}"
        );
    }
    let legacy: [&[&str]; 16] = [
        &["-h"],
        &["--help"],
        &["--self-test"],
        &["--topology=single"],
        &["--topo", "single"],
        &["--topology", "bogus"],
        &["--topology"],
        &["--repo"],
        &["--topology", "single", "--topology", "single"],
        &["--json", "--json"],
        &["--require-flag", "--require-flag"],
        &["--repo", "-x"],
        &["--repo", "-5"],
        &["--repo", "a", "--repo", "b"],
        &["extra"],
        &["--bogus"],
    ];
    for shape in legacy {
        assert!(
            parse_exact(&bytes(shape)).is_none(),
            "legacy shape {shape:?}"
        );
    }
    let request =
        parse_exact(&bytes(&["--json", "--repo", "r", "--topology", "multi"])).expect("shape");
    assert_eq!(request.topology, Some("multi"));
    assert_eq!(request.repo, b"r".to_vec());
    assert!(request.json && !request.require_flag);
    assert_eq!(parse_exact(&[]).expect("empty").repo, b".".to_vec());
}

#[test]
fn every_shape_and_every_source_takes_the_native_route() {
    let dir = scratch("routes");
    let reviewed = dir.join("manifest_contract.py");
    std::fs::write(&reviewed, REVIEWED_MANIFEST_CONTRACT).unwrap();
    let owner = |path: &Path| path.as_os_str().as_bytes().to_vec();
    for (shape, env) in [
        (bytes(&["--topology", "single", "--repo", "/r", "--require-flag"]), environment(Some(&reviewed), &[("LC_ALL", "C.UTF-8")])),
        (bytes(&["-h"]), environment(Some(&reviewed), &[])),
        (bytes(&["--self-test"]), environment(Some(&reviewed), &[])),
        (bytes(&["--topology", "single"]), environment(Some(&reviewed), &[("PYTHONIOENCODING", "latin-1")])),
    ] {
        assert_eq!(choose_route(&shape, &env, &owner(&reviewed)), Route::NativeCurrentSource);
        assert_eq!(choose_route(&shape, &env, &owner(&dir.join("missing.py"))), Route::NativeCurrentSource);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

// ── every frame row is a line of the reviewed owner ─────────────────────────────────────

#[test]
fn owner_frame_rows_are_bound_to_the_reviewed_source() {
    let source = std::str::from_utf8(REVIEWED_MANIFEST_CONTRACT).expect("owner is UTF-8");
    let lines: Vec<&str> = source.split('\n').collect();
    let mut rows: Vec<(u32, &str, &str)> = OWNER_FRAMES
        .iter()
        .map(|row| (row.line, row.code, row.expr))
        .collect();
    rows.extend(MAIN_FRAMES.iter().map(|row| (row.line, row.code, row.expr)));
    for site in [
        role_contract::manifest_gate_cli::PrintSite::Conflict,
        role_contract::manifest_gate_cli::PrintSite::Missing,
        role_contract::manifest_gate_cli::PrintSite::Json,
        role_contract::manifest_gate_cli::PrintSite::Plain,
    ] {
        let frame = site.frame();
        rows.push((frame.line, frame.code, frame.expr));
    }
    for (line, code, expr) in rows {
        let at = lines
            .get(line as usize - 1)
            .unwrap_or_else(|| panic!("owner line {line} exists"));
        assert_eq!(at.trim(), code, "line {line} of the reviewed owner");
        assert!(
            expr.is_empty() || code.contains(expr),
            "line {line}: expression {expr:?} is part of {code:?}"
        );
    }
}

// ── the frozen ORIGINAL190 capture ──────────────────────────────────────────────────────

fn field<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value.peel() {
        PyValue::Dict(map) => map.get(key).unwrap_or_else(|| panic!("field {key}")),
        _ => panic!("object expected for {key}"),
    }
}
fn text(value: &PyValue) -> String {
    match value.peel() {
        PyValue::Str(text) => text.to_utf8().expect("UTF-8 text"),
        other => panic!("string expected, got {}", other.type_name()),
    }
}
fn maybe_field<'a>(value: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match value.peel() {
        PyValue::Dict(map) => map.get(key),
        _ => None,
    }
}
fn items(value: &PyValue) -> &Vec<PyValue> {
    match value.peel() {
        PyValue::List(items) => items,
        _ => panic!("array expected"),
    }
}
fn opt_text(value: &PyValue) -> Option<String> {
    match value.peel() {
        PyValue::None => None,
        _ => Some(text(value)),
    }
}
/// The golden strings carry one byte per code point (latin-1).
fn raw_bytes(text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| u8::try_from(u32::from(c)).expect("latin-1 code point"))
        .collect()
}

/// The lane a repo path resolves into, standing in for `git rev-parse --abbrev-ref HEAD` on the fixture groups.
struct LaneProbe {
    root: PathBuf,
    absent: bool,
}

impl BranchProbe for LaneProbe {
    fn branch(&mut self, repo: &[u8]) -> Option<PyStr> {
        if self.absent {
            return None;
        }
        let real = std::fs::canonicalize(OsStr::from_bytes(repo)).ok()?;
        let relative = real
            .strip_prefix(std::fs::canonicalize(&self.root).ok()?)
            .ok()?;
        let group = relative
            .components()
            .next()?
            .as_os_str()
            .to_str()?
            .to_string();
        let branch = match group.as_str() {
            "single" => "single-node",
            "multi" => "multi-node",
            "other" => "fixture-other",
            "detached" => "HEAD",
            _ => return None,
        };
        Some(PyStr::new(branch))
    }
}

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/manifest_contract")
}

fn build_fixture(root: &Path, row: &PyValue, opaque: &mut Vec<PathBuf>) {
    let group = root.join(text(field(row, "repo_group")));
    let case_dir = group.join(text(field(row, "case_dir")));
    let places = items(field(row, "place"));
    let linked = places
        .iter()
        .any(|p| text(field(p, "op")) == "symlink_group");
    std::fs::create_dir_all(group.join("cases")).unwrap();
    if !linked {
        std::fs::create_dir_all(&case_dir).unwrap();
    }
    for place in places {
        let op = text(field(place, "op"));
        let relative = text(field(place, "path"));
        let target_path = if op == "symlink_group" {
            group.join(&relative)
        } else {
            case_dir.join(&relative)
        };
        if let Some(parent) = target_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        match op.as_str() {
            "dir" => std::fs::create_dir_all(&target_path).unwrap(),
            "file" => {
                let name = text(field(place, "data"));
                std::fs::write(
                    &target_path,
                    std::fs::read(data_dir().join("manifests").join(format!("{name}.yaml")))
                        .unwrap(),
                )
                .unwrap();
            }
            "symlink" => {
                std::os::unix::fs::symlink(text(field(place, "target")), &target_path).unwrap()
            }
            "symlink_group" => {
                let target =
                    text(field(place, "target")).replace("<FIXROOT>", root.to_str().unwrap());
                std::os::unix::fs::symlink(target, &target_path).unwrap();
            }
            "opaque" => opaque.push(target_path),
            other => panic!("unknown placement {other}"),
        }
    }
}

fn restricted_by_modes(path: &Path) -> bool {
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o0)).unwrap();
    let restricted = std::fs::read(path).is_err() && std::fs::read_dir(path).is_err();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    restricted
}

/// The replay changes the process working directory (relative `--repo` rows): one replay at a time.
static CWD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn original190_golden_rows_replay_natively() {
    replay_golden("original190.json", 190);
}

#[test]
fn extra14_golden_rows_replay_natively() {
    replay_golden("extra14.json", 14);
}

#[test]
fn extra9_golden_rows_replay_natively() {
    replay_golden("extra9.json", 9);
}

#[test]
fn key4_golden_rows_replay_natively() {
    // The true huge-integer key branch: frames 432, 406, 373 `key = _intstr(key)` (the explicit block key of R48K04).
    replay_golden("key4.json", 1);
}

#[test]
fn bytedepth_golden_row_replays_natively() {
    // The bytes leaf under 994 enclosing dicts: the `_default(o)` CALL of the leaf's generator frame hits the limit (frame 439).
    replay_golden("bytedepth.json", 1);
}

#[test]
fn deep2_golden_rows_replay_natively() {
    // R48K06: a float value under 995 containers fails at the container's own `floatstr` call (encoder line 398); R48K07: the repeated
    // visit of a cyclic dict at 995 containers fails at the call that creates its child generator (line 403) before the circular check.
    replay_golden("deep2.json", 2);
}

#[test]
fn key8_golden_row_replays_natively() {
    // R48K08: a float dict KEY in the 995th (innermost) container is spelled by `floatstr` in the dict's own generator frame: encoder line 364
    // `key = _floatstr(key)`, after 406 x3 and the repeated-line summary (991), RecursionError, stdout empty. Literal copy of the saved raw.
    replay_golden("key8.json", 1);
}

#[test]
fn gap3_golden_rows_replay_natively() {
    // R48K01 keeps the observed parser error (its flow-style key exceeded the YAML simple-key limit), R48K02 the recursion
    // limit through deferred anchors, R48K03 the deleted working directory.
    replay_golden("gap3.json", 3);
}

fn replay_golden(file: &str, expected_rows: usize) {
    let file = file.to_string();
    on_big_stack(move || replay_golden_body(&file, expected_rows));
}

fn replay_golden_body(file: &str, expected_rows: usize) {
    let _cwd = CWD_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let golden = pyjson::loads(&std::fs::read_to_string(data_dir().join(file)).unwrap())
        .expect("golden JSON");
    let rows = items(field(&golden, "rows"));
    let base: Vec<(String, String)> = match field(&golden, "base_profile_env").peel() {
        PyValue::Dict(map) => map
            .iter()
            .map(|(k, v)| (k.to_utf8().unwrap(), text(v)))
            .collect(),
        _ => panic!("base env"),
    };
    let root = scratch(&format!("fixtures-{}", file.trim_end_matches(".json")));
    let owner_path = root.join("_owner/manifest_contract.py");
    std::fs::create_dir_all(owner_path.parent().unwrap()).unwrap();
    std::fs::write(&owner_path, REVIEWED_MANIFEST_CONTRACT).unwrap();
    std::fs::create_dir_all(root.join("_nogit_bin")).unwrap();
    let owner = owner_path.as_os_str().as_bytes().to_vec();
    let (fixroot, owner_text) = (
        root.to_str().unwrap().to_string(),
        owner_path.to_str().unwrap().to_string(),
    );
    let mut opaque_by_row: Vec<(String, Vec<PathBuf>)> = Vec::new();
    for row in rows {
        let mut opaque = Vec::new();
        build_fixture(&root, row, &mut opaque);
        opaque_by_row.push((text(field(row, "id")), opaque));
    }
    let cwd_before = std::env::current_dir().unwrap();
    let (mut replayed, mut skipped, mut routed) = (0usize, Vec::new(), 0usize);
    let mut failures: Vec<String> = Vec::new();
    for row in rows {
        if !matches!(field(row, "run").peel(), PyValue::Bool(true)) {
            continue;
        }
        let id = text(field(row, "id"));
        let group = root.join(text(field(row, "repo_group")));
        let case_dir = group.join(text(field(row, "case_dir")));
        let cwd = if text(field(row, "cwd")) == "group" {
            group.clone()
        } else {
            case_dir.clone()
        };
        let relative = text(field(row, "case_dir"));
        let token = |arg: &str| {
            arg.replace("<CASE>", case_dir.to_str().unwrap())
                .replace("<GROUP>", group.to_str().unwrap())
                .replace("<FIXROOT>", &fixroot)
                .replace("<REL>", &relative)
        };
        let args: Vec<Vec<u8>> = items(field(row, "args"))
            .iter()
            .map(|a| token(&text(a)).into_bytes())
            .collect();
        let env_extra: Vec<(String, String)> = match field(row, "env_extra").peel() {
            PyValue::Dict(map) => map
                .iter()
                .map(|(k, v)| (k.to_utf8().unwrap(), text(v)))
                .collect(),
            _ => Vec::new(),
        };
        let lookup = |name: &str| -> Option<Vec<u8>> {
            env_extra
                .iter()
                .chain(base.iter())
                .find(|(k, _)| k == name)
                .map(|(_, v)| v.clone().into_bytes())
        };
        let route = opt_text(field(row, "route"));
        let Some(request) = parse_exact(&args) else {
            assert_eq!(
                route.as_deref(),
                Some("LegacyShape"),
                "{id}: a shape native code does not answer is a LegacyShape row"
            );
            routed += 1;
            continue;
        };
        let Ok(profile) = TextProfile::from_environment(&lookup) else {
            assert_eq!(
                route.as_deref(),
                Some("LegacyProfile"),
                "{id}: an unmodelled profile is a LegacyProfile row"
            );
            routed += 1;
            continue;
        };
        if id == "V01" {
            routed += 1; // declared legacy owner variant of the retired original (no interpreter route any more; the native answer covers every source)
            continue;
        }
        let places = items(field(row, "place"));
        if places.iter().any(|p| text(field(p, "op")) == "opaque") {
            let paths = &opaque_by_row
                .iter()
                .find(|(name, _)| *name == id)
                .unwrap()
                .1;
            if paths.iter().any(|p| !restricted_by_modes(p)) {
                skipped.push(id);
                continue;
            }
        }
        // Mode-000 entries are applied last and lifted again after the row.
        let paths = opaque_by_row
            .iter()
            .find(|(name, _)| *name == id)
            .unwrap()
            .1
            .clone();
        let mut ordered = paths.clone();
        ordered.sort_by_key(|p| std::cmp::Reverse(p.components().count()));
        for p in &ordered {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o0)).unwrap();
        }
        let mut probe = LaneProbe {
            root: root.clone(),
            absent: env_extra.iter().any(|(k, _)| k == "PATH"),
        };
        std::env::set_current_dir(&cwd).unwrap();
        if matches!(
            maybe_field(row, "deleted_cwd").map(PyValue::peel),
            Some(PyValue::Bool(true))
        ) {
            // the original run had its working directory removed after the interpreter started: replay with an own scratch one
            let gone = root.join("deleted-cwd");
            std::fs::create_dir(&gone).unwrap();
            std::env::set_current_dir(&gone).unwrap();
            std::fs::remove_dir(&gone).unwrap();
        }
        let answered = with_profile(&profile, || execute(&request, &owner, &mut probe));
        std::env::set_current_dir(&cwd_before).unwrap();
        ordered.sort_by_key(|p| p.components().count());
        for p in &ordered {
            let mode = if p.is_dir() { 0o755 } else { 0o644 };
            let _ = std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode));
        }
        let answer = match answered {
            Ok(answer) => answer,
            Err(unsupported) => {
                failures.push(format!(
                    "{id}: unsupported {} — {}",
                    unsupported.boundary, unsupported.detail
                ));
                continue;
            }
        };
        let (mut out, mut err, mut status) = (Vec::new(), Vec::new(), answer.status);
        let mut stderr_text = answer.stderr.clone();
        if let Some(printed) = &answer.stdout {
            match encode_stream(&printed.text, profile.stdout, profile.stdout_errors) {
                Ok(encoded) => {
                    out = encoded;
                    out.push(b'\n');
                }
                Err(failure) => {
                    let crash = print_failure(&owner, printed.site, &failure.message);
                    stderr_text = crash.stderr;
                    status = crash.status;
                }
            }
        }
        if let Some(message) = &stderr_text {
            err = encode_stream(message, profile.stdout, Errors::BackslashReplace)
                .unwrap_or_default();
        }
        let expand = |golden: &str| {
            raw_bytes(
                &golden
                    .replace("<OWNER>", &owner_text)
                    .replace("<FIXROOT>", &fixroot)
                    .replace("<OWNER>", &owner_text),
            )
        };
        let want_out = expand(&text(field(row, "stdout")));
        let want_err = expand(&text(field(row, "stderr")));
        let want_status = text_status(field(row, "exit"));
        if status != want_status || out != want_out || err != want_err {
            failures.push(format!(
                "{id}: status {status} want {want_status}; stdout: {}; stderr: {}",
                first_difference(&out, &want_out),
                first_difference(&err, &want_err)
            ));
        }
        replayed += 1;
    }
    let _ = std::fs::remove_dir_all(&root);
    eprintln!("replayed {replayed} rows natively, {routed} rows are legacy-route rows, skipped (file modes not restrictive): {skipped:?}");
    assert!(
        failures.is_empty(),
        "{} golden rows differ:\n{}",
        failures.len(),
        failures.join("\n")
    );
    assert!(
        replayed + routed + skipped.len() == expected_rows,
        "all {expected_rows} original rows are accounted for"
    );
}

fn text_status(value: &PyValue) -> i32 {
    match value.peel() {
        PyValue::Int(number) => number.to_string().parse().expect("exit status"),
        _ => panic!("integer exit status"),
    }
}

// ── structure checks of the encoder-source-derived failures (NOT golden: the literal raw comes with the extra capture) ──────

struct NoGit;

impl BranchProbe for NoGit {
    fn branch(&mut self, _repo: &[u8]) -> Option<PyStr> {
        None
    }
}

/// The deep-nesting rows recurse about a thousand containers deep; the executable runs on a large stack (`on_large_stack`), so do the checks that
/// call the library directly: the debug build of a 2 MiB test thread has no room for a thousand levels of the YAML loader plus the JSON model.
fn on_big_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(256 * 1024 * 1024)
        .spawn(work)
        .expect("thread")
        .join()
        .unwrap_or_else(|panic| std::panic::resume_unwind(panic))
}

fn run_manifest(yaml: &str, json: bool) -> role_contract::manifest_gate_cli::Answer {
    let yaml = yaml.to_string();
    on_big_stack(move || run_manifest_inner(&yaml, json))
}

fn run_manifest_inner(yaml: &str, json: bool) -> role_contract::manifest_gate_cli::Answer {
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = scratch(&format!("structure-{run}"));
    std::fs::create_dir_all(dir.join("output/single")).unwrap();
    std::fs::write(dir.join("output/single/manifest.yaml"), yaml).unwrap();
    let mut args = vec!["--topology", "single", "--repo", dir.to_str().unwrap()];
    if json {
        args.push("--json");
    }
    let request = parse_exact(&bytes(&args)).expect("exact shape");
    let profile =
        TextProfile::from_environment(&|name| (name == "LC_ALL").then(|| b"C.UTF-8".to_vec()))
            .expect("UTF-8 profile");
    let answer = with_profile(&profile, || {
        execute(&request, b"/owner/manifest_contract.py", &mut NoGit)
    })
    .unwrap_or_else(|unsupported| panic!("answered, got {unsupported:?}"));
    let _ = std::fs::remove_dir_all(&dir);
    answer
}

const VALID: &str = "terraforming: {complete: true, branch_verified: true}\ntopology: single\ngpus_per_node: 1\nmodel_source: managed\n";

#[test]
fn a_date_role_is_valid_for_the_gate_and_refused_by_json_as_a_value() {
    // The nodes list prints the date as a VALUE before the effective_model_source keys are reached (captured X06).
    let yaml = format!("{VALID}nodes: [{{role: 2026-11-08}}]\n");
    assert_eq!(run_manifest(&yaml, false).status, 0);
    let json = run_manifest(&yaml, true);
    assert!(json.crashed && json.status == 1);
    let text = json.stderr.expect("traceback").to_utf8().unwrap();
    assert!(text.contains("line 439, in _iterencode"), "{text}");
    assert!(
        text.ends_with("TypeError: Object of type date is not JSON serializable\n"),
        "{text}"
    );
}

#[test]
fn a_date_key_is_refused_by_json_in_the_dict_encoder() {
    // Structure check only (no capture of a key failure exists): the encoder source says line 377 in `_iterencode_dict`.
    let yaml = format!("{VALID}custom_model_paths: {{2026-11-08: x}}\n");
    assert_eq!(run_manifest(&yaml, false).status, 0);
    let text = run_manifest(&yaml, true)
        .stderr
        .expect("traceback")
        .to_utf8()
        .unwrap();
    assert!(text.contains("line 377, in _iterencode_dict"), "{text}");
    assert!(
        text.ends_with("TypeError: keys must be str, int, float, bool or None, not date\n"),
        "{text}"
    );
}

#[test]
fn a_dictionary_that_contains_itself_fails_json_in_the_dict_encoder() {
    let yaml = format!("{VALID}custom_model_paths: &d {{a: *d}}\n");
    assert_eq!(run_manifest(&yaml, false).status, 0);
    let json = run_manifest(&yaml, true);
    let text = json.stderr.expect("traceback").to_utf8().unwrap();
    assert!(text.contains("line 341, in _iterencode_dict"), "{text}");
    assert!(
        text.ends_with("ValueError: Circular reference detected\n"),
        "{text}"
    );
}

/// Where two byte strings first differ, with a little context from each side (for the failure message only).
fn first_difference(got: &[u8], want: &[u8]) -> String {
    let at = got.iter().zip(want).take_while(|(a, b)| a == b).count();
    if at == got.len() && at == want.len() {
        return "equal".to_string();
    }
    let window = |bytes: &[u8]| {
        let from = at.saturating_sub(40);
        String::from_utf8_lossy(&bytes[from..bytes.len().min(at + 80)]).into_owned()
    };
    format!(
        "differ at byte {at}: got {:?} want {:?}",
        window(got),
        window(want)
    )
}

// ── structure checks of rows derived from the encoder source (NOT golden): list chains at the recursion limit; the key branch is golden since R48K04 ──

#[test]
fn repeated_frames_collapse_like_the_traceback_module() {
    use role_contract::manifest_gate_cli::render_traceback;
    use role_contract::topology::Frame;
    let frame = |line| Frame {
        file: b"/x.py".to_vec(),
        line,
        function: "f",
        code: "yield from chunks",
        expr: "",
    };
    let render = |count: usize, tail: bool| {
        let mut frames: Vec<Frame> = (0..count).map(|_| frame(7)).collect();
        if tail {
            frames.push(frame(9));
        }
        render_traceback(&frames, "E", &PyStr::new("m"))
            .to_utf8()
            .unwrap()
    };
    assert_eq!(render(3, true).matches("line 7,").count(), 3);
    assert!(!render(3, true).contains("Previous line"));
    let four = render(4, true);
    assert_eq!(four.matches("line 7,").count(), 3);
    assert!(
        four.contains("  [Previous line repeated 1 more time]\n  File \"/x.py\", line 9,"),
        "{four}"
    );
    let six = render(6, false);
    assert!(
        six.ends_with("  [Previous line repeated 3 more times]\nE: m\n"),
        "{six}"
    );
}

fn nested_manifest(open: &str, close: &str, depth: usize) -> String {
    // a flat anchor chain: x<i> wraps the previous anchor once, so YAML itself never nests deeply
    let mut text = format!("{VALID}x0: &a0 {open}1{close}\n");
    for level in 1..=depth {
        text.push_str(&format!(
            "x{level}: &a{level} {open}{}*a{}{close}\n",
            if open == "{" { "k: " } else { "" },
            level - 1
        ));
    }
    text.push_str(&format!("custom_model_paths: *a{depth}\n"));
    text
}

#[test]
fn list_nesting_beyond_the_recursion_limit_fails_at_the_list_generator() {
    // Source-derived: dict (res) at 406, then lists at 326, the failing list creating its child list at 321.
    let json = run_manifest(&nested_manifest("[", "]", 1100), true);
    assert!(json.crashed && json.status == 1);
    let text = json.stderr.expect("traceback").to_utf8().unwrap();
    assert!(
        text.contains("  [Previous line repeated 990 more times]\n"),
        "{text}"
    );
    assert!(text.contains("line 321, in _iterencode_list"), "{text}");
    assert!(
        text.ends_with("RecursionError: maximum recursion depth exceeded\n"),
        "{text}"
    );
}

/// `containers` enclosing JSON containers around ONE leaf given as YAML text (`1.5`, or a self-referencing mapping): the same flat anchor chain.
fn leaf_manifest(leaf: &str, containers: usize) -> String {
    let chain = containers - 1;
    let mut text = format!("{VALID}x0: &a0 {leaf}\n");
    for level in 1..=chain {
        text.push_str(&format!("x{level}: &a{level} {{k: *a{}}}\n", level - 1));
    }
    text.push_str(&format!("custom_model_paths: *a{chain}\n"));
    text
}

#[test]
fn a_float_leaf_and_a_cyclic_dict_one_container_before_the_captured_limit() {
    // The captured bounds (goldens deep2.json: 995 containers) fix the frame arithmetic; ONE container fewer is the neighbour that still fits:
    // the float is printed; the cyclic dict's child generator exists and its circular check runs (`ValueError`, line 341). Source-derived, no raw.
    on_big_stack(|| {
        let float = run_manifest(&leaf_manifest("1.5", 994), true);
        assert!(
            !float.crashed && float.status == 0,
            "a float under 994 containers prints"
        );
        assert!(float
            .stdout
            .expect("the verdict")
            .text
            .to_utf8()
            .unwrap()
            .contains("1.5"));
        // root + 992 wrappers + the cyclic dict itself = 994 enclosing containers around its repeated visit
        let cycle = run_manifest(&leaf_manifest("{k: *a0}", 993), true);
        assert!(cycle.crashed && cycle.status == 1);
        let text = cycle.stderr.expect("traceback").to_utf8().unwrap();
        assert!(text.contains("line 341, in _iterencode_dict"), "{text}");
        assert!(
            text.ends_with("ValueError: Circular reference detected\n"),
            "{text}"
        );
    });
}

fn bytes_leaf_manifest(containers: usize) -> String {
    // `containers` enclosing JSON containers = the result dictionary + (containers - 1) chained dicts around one bytes leaf; a flat
    // anchor chain, so YAML itself never nests deeply.
    let chain = containers - 1;
    let mut text = format!("{VALID}x0: &a0 !!binary eA==\n");
    for level in 1..=chain {
        text.push_str(&format!("x{level}: &a{level} {{k: *a{}}}\n", level - 1));
    }
    text.push_str(&format!("custom_model_paths: *a{chain}\n"));
    text
}

#[test]
fn a_bytes_leaf_hits_the_recursion_limit_one_container_before_a_container_does() {
    bytes_leaf_depth_cases();
}

fn bytes_leaf_depth_cases() {
    // 993 enclosing containers: `default(o)` still fits and raises the TypeError. 994 (captured raw, golden bytedepth.json): the
    // `_default(o)` call of the leaf's own generator frame is frame 1001. 995 (source-derived, no raw): the container that holds
    // the leaf cannot create the leaf's generator (encoder line 405).
    let fits = run_manifest(&bytes_leaf_manifest(993), true);
    let text = fits.stderr.expect("traceback").to_utf8().unwrap();
    assert!(
        text.ends_with("TypeError: Object of type bytes is not JSON serializable\n"),
        "{text}"
    );
    let at_limit = run_manifest(&bytes_leaf_manifest(994), true);
    assert!(at_limit.crashed && at_limit.status == 1);
    let text = at_limit.stderr.expect("traceback").to_utf8().unwrap();
    assert!(
        text.contains("  [Previous line repeated 991 more times]\n"),
        "{text}"
    );
    assert!(
        text.contains("line 439, in _iterencode\n    o = _default(o)\n"),
        "{text}"
    );
    assert!(
        text.ends_with("RecursionError: maximum recursion depth exceeded\n"),
        "{text}"
    );
    let beyond = run_manifest(&bytes_leaf_manifest(995), true);
    let text = beyond.stderr.expect("traceback").to_utf8().unwrap();
    assert!(text.contains("line 405, in _iterencode_dict"), "{text}");
    assert!(!text.contains("_default(o)"), "{text}");
    assert!(
        text.ends_with("RecursionError: maximum recursion depth exceeded\n"),
        "{text}"
    );
}

#[test]
fn a_huge_integer_key_written_as_an_explicit_block_key_fails_in_the_dict_encoder() {
    // The captured key branch (golden key4.json) written with an own manifest: encoder line 373 `key = _intstr(key)`.
    let yaml = format!(
        "{VALID}custom_model_paths:\n  ? 0x{}\n  : x\n",
        "f".repeat(3600)
    );
    assert_eq!(run_manifest(&yaml, false).status, 0);
    let json = run_manifest(&yaml, true);
    assert!(json.crashed && json.status == 1);
    let text = json.stderr.expect("traceback").to_utf8().unwrap();
    assert!(text.contains("line 373, in _iterencode_dict"), "{text}");
    assert!(text.contains("key = _intstr(key)"), "{text}");
    assert!(text.ends_with("ValueError: Exceeds the limit (4300 digits) for integer string conversion; use sys.set_int_max_str_digits() to increase the limit\n"), "{text}");
}
