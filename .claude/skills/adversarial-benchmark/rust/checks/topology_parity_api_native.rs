//! The typed API bridge `topology_parity_api` (actual native functions of `role_contract::{topology, campaign}`) on W-owned
//! literal fixtures built here: request/answer protocol, DTO typing (str, bigint, tuple, list, dict order, path), exceptions
//! with logical frames, and the explicit bridge errors. Nothing is read from a saved original root and nothing is expected
//! from a case id; each assertion is a fact of the original sources (return types, pointer rules, raise sites).

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sweep_stop::pyjson::loads;
use sweep_stop::pyvalue::PyValue;

const API: &str = env!("CARGO_BIN_EXE_topology_parity_api");

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("topology_api")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    dir
}

fn put(root: &Path, rel: &str, bytes: &[u8]) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
    std::fs::write(&path, bytes).expect("write");
    path
}

fn git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .expect("git")
        .status;
    assert!(status.success(), "git {args:?}");
}

/// A checkout: git on `single-node`, one tracked layer file, a `single` manifest, the campaign module file and an active campaign.
fn checkout(label: &str) -> PathBuf {
    let root = scratch(label);
    git(&root, &["init", "-q"]);
    git(&root, &["symbolic-ref", "HEAD", "refs/heads/single-node"]);
    put(
        &root,
        ".claude/rules/strategy.topology.md",
        "# t\n\n**topology: single** · layer\n".as_bytes(),
    );
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        role_contract::source_identity::REVIEWED_CAMPAIGN,
    );
    put(
        &root,
        "output/single/manifest.yaml",
        b"topology: single\nterraforming:\n  complete: true\n",
    );
    git(&root, &["add", "-A"]);
    std::fs::create_dir_all(root.join("campaigns/camp-x")).expect("campaign");
    put(&root, "campaigns/ACTIVE", b"camp-x\n");
    put(&root, "campaigns/camp-x/campaign.yaml", br#"{"control_variables": {"topology": "single"}, "big": 123456789012345678901234567890, "nodes": [{"node_id": "n1", "topology": "single"}]}"#);
    root
}

fn call(rows: &[String]) -> Vec<PyValue> {
    let mut child = Command::new(API)
        .arg("evaluate-jsonl")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", env!("CARGO_TARGET_TMPDIR"))
        .env("LC_ALL", "C.UTF-8")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("api");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all((rows.join("\n") + "\n").as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("output");
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| loads(line).expect("answer JSON"))
        .collect()
}

/// The API binary copied where no ancestor directory is named `rust`: the approved generic bridge is then genuinely not
/// installed beside it, so the "bridge not installed" unmet condition is deterministic (not a function of the checkout layout).
fn call_without_bridge(label: &str, rows: &[String]) -> Vec<PyValue> {
    let dir = std::env::temp_dir().join(format!("topology_nobridge_{}_{label}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("dir");
    let copy = dir.join("topology_parity_api_copy");
    std::fs::copy(API, &copy).expect("copy api");
    // A sibling test thread forking while our copy's write fd is open makes exec fail with ETXTBSY (the fd lives in that
    // forked child until its exec). Retry only that error, briefly; any other spawn error still fails at once.
    let mut attempt = 0;
    let mut child = loop {
        match Command::new(&copy)
            .arg("evaluate-jsonl")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("TMPDIR", env!("CARGO_TARGET_TMPDIR"))
            .env("LC_ALL", "C.UTF-8")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => break c,
            Err(e) if e.raw_os_error() == Some(26) && attempt < 50 => {
                attempt += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            Err(e) => panic!("api: {e}"),
        }
    };
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all((rows.join("\n") + "\n").as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("output");
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| loads(line).expect("answer JSON"))
        .collect()
}

fn row(id: &str, operation: &str, arguments: &str) -> String {
    format!(r#"{{"id":"{id}","operation":"{operation}","arguments":{arguments}}}"#)
}

fn field(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(d) => d.get(key).cloned().unwrap_or_else(|| {
            panic!(
                "missing {key} in {}",
                value.dumps_ascii().unwrap_or_default()
            )
        }),
        other => panic!("not a dict: {other:?}"),
    }
}

fn json(value: &PyValue) -> String {
    value.dumps_ascii().expect("JSON")
}

fn js(text: &str) -> String {
    PyValue::str(text).dumps_ascii().expect("JSON")
}

fn module_of(root: &Path) -> String {
    js(root
        .join(".claude/skills/terraforming_node/scripts/campaign_init.py")
        .to_str()
        .expect("UTF-8"))
}

#[test]
fn layer_header_and_files_answer_typed_values() {
    let root = checkout("layer");
    let header = js(root
        .join(".claude/rules/strategy.topology.md")
        .to_str()
        .expect("UTF-8"));
    let missing = js(root.join("nope.topology.md").to_str().expect("UTF-8"));
    let repo = js(root.to_str().expect("UTF-8"));
    let nongit = scratch("nongit");
    let answers = call(&[
        row("a", "read_layer_header", &format!(r#"{{"path":{header}}}"#)),
        row(
            "b",
            "read_layer_header",
            &format!(r#"{{"path":{missing}}}"#),
        ),
        row("c", "layer_files", &format!(r#"{{"repo":{repo}}}"#)),
        row(
            "d",
            "layer_files",
            &format!(r#"{{"repo":{}}}"#, js(nongit.to_str().expect("UTF-8"))),
        ),
    ]);
    assert_eq!(
        json(&field(&answers[0], "value")),
        r#"["tuple", [["str", "single"], ["str", "ok"]]]"#
    );
    let why = json(&field(&answers[1], "value"));
    let expected = js("읽기 실패: [Errno 2] No such file or directory: '");
    let expected = format!(
        "[\"tuple\", [[\"none\"], [\"str\", {}",
        &expected[..expected.len() - 1]
    );
    assert!(why.starts_with(&expected), "{why}");
    let files = json(&field(&answers[2], "value"));
    assert!(
        files.starts_with(
            r#"["list", [["path", {"module": "pathlib", "type": "PosixPath", "value": ""#
        ) && files.ends_with(r#"/.claude/rules/strategy.topology.md"}]]]"#),
        "{files}"
    );
    assert_eq!(json(&field(&answers[3], "value")), r#"["list", []]"#);
}

#[test]
fn campaign_read_side_answers_typed_values_with_the_module_root_fallback() {
    let root = checkout("campaign");
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    let base = js(root.join("campaigns/camp-x").to_str().expect("UTF-8"));
    let answers = call(&[
        row(
            "a",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        ),
        row(
            "b",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":null}}"#),
        ),
        row(
            "c",
            "read_declaration",
            &format!(r#"{{"module":{module},"base":{base}}}"#),
        ),
        row(
            "d",
            "campaigns_dir",
            &format!(
                r#"{{"module":{module},"repo_root":{{"type":"Path","value":{}}}}}"#,
                repo
            ),
        ),
        row(
            "e",
            "derive_path",
            &format!(r#"{{"module":{module},"kind":"config","cell":"c1","repo_root":{repo}}}"#),
        ),
    ]);
    assert_eq!(json(&field(&answers[0], "value")), r#"["str", "camp-x"]"#);
    assert_eq!(
        json(&field(&answers[1], "value")),
        r#"["str", "camp-x"]"#,
        "repo_root null falls back to the module's own root"
    );
    let declaration = json(&field(&answers[2], "value"));
    assert!(
        declaration.contains(r#"[["str", "big"], ["int", "123456789012345678901234567890"]]"#),
        "{declaration}"
    );
    assert!(declaration.starts_with(r#"["dict", [[["str", "control_variables"], ["dict", [[["str", "topology"], ["str", "single"]]]]]"#), "insertion order: {declaration}");
    assert!(json(&field(&answers[3], "value"))
        .contains(r#"["path", {"module": "pathlib", "type": "PosixPath", "value": ""#));
    let derived = json(&field(&answers[4], "value"));
    assert!(
        derived.ends_with(r#"/campaigns/camp-x/cells/c1/config.yaml"}]"#),
        "{derived}"
    );
}

#[test]
fn a_refusal_and_a_crash_are_typed_exceptions_with_logical_frames() {
    let root = checkout("exceptions");
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    put(&root, "campaigns/ACTIVE", b"\xff");
    let answers = call(&[
        row(
            "refuse",
            "derive_path",
            &format!(r#"{{"module":{module},"kind":"bogus","repo_root":{repo},"camp_id":"zz"}}"#),
        ),
        row(
            "invalid",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        ),
    ]);
    assert_eq!(json(&field(&answers[0], "kind")), r#""exception""#);
    let refusal = field(&answers[0], "exception");
    assert_eq!(json(&field(&refusal, "type")), r#""PurgeGateRefusal""#);
    assert_eq!(
        json(&field(&refusal, "message")),
        js("알 수 없는 파생 종류: 'bogus'")
    );
    let frames = json(&field(&refusal, "frames"));
    assert!(
        frames.contains(r#""line": 147, "function": "derive_path""#),
        "{frames}"
    );
    let crash = field(&answers[1], "exception");
    assert_eq!(json(&field(&crash, "type")), r#""UnicodeDecodeError""#);
    assert_eq!(
        json(&field(&crash, "message")),
        r#""'utf-8' codec can't decode byte 0xff in position 0: invalid start byte""#
    );
    let frames = json(&field(&crash, "frames"));
    assert!(
        frames.contains(r#""line": 113, "function": "active_campaign_id""#)
            && frames.contains(r#""line": 1030, "function": "read_text""#)
            && frames.contains(r#""line": 322, "function": "decode""#),
        "{frames}"
    );
    assert_eq!(
        json(&field(&field(&answers[1], "bridge"), "frames_complete")),
        "false"
    );
}

#[test]
fn evaluate_answers_the_verdict_dict_in_construction_order_and_the_bridge_names_its_own_errors() {
    let root = checkout("evaluate");
    let repo = js(root.to_str().expect("UTF-8"));
    let answers = call(&[
        row(
            "a",
            "evaluate",
            &format!(r#"{{"repo":{{"type":"Path","value":{repo}}},"expect":"single"}}"#),
        ),
        row(
            "b",
            "evaluate",
            &format!(r#"{{"repo":{repo},"expect":"both"}}"#),
        ),
        row("c", "no_such_operation", "{}"),
        row("d", "layer_files", "{}"),
    ]);
    let verdict = json(&field(&answers[0], "value"));
    assert!(verdict.starts_with(r#"["dict", [[["str", "schema_version"], ["int", "1"]], [["str", "predicate"], ["str", "topology_parity"]], [["str", "policy"], ["str", "BRANCH_CONSTITUTION_LAYERING.C5"]], [["str", "verdict"], ["str", "PASS"]], [["str", "topology"], ["str", "single"]]"#), "{verdict}");
    let invalid = json(&field(&answers[1], "value"));
    assert!(
        invalid.contains("EXPECT_INVALID") && invalid.contains(r#"["str", "RED"]"#),
        "{invalid}"
    );
    for index in [2, 3] {
        let error = field(&answers[index], "exception");
        assert_eq!(
            json(&field(&field(&answers[index], "bridge"), "bridge_error")),
            "true"
        );
        assert_eq!(json(&field(&error, "type")), r#""TypeError""#);
    }
}

fn api_call_with(rows: &[String], env: &[(&str, &str)]) -> Vec<PyValue> {
    let mut command = Command::new(API);
    command
        .arg("evaluate-jsonl")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", env!("CARGO_TARGET_TMPDIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("api");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all((rows.join("\n") + "\n").as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("output");
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| loads(line).expect("answer JSON"))
        .collect()
}

#[test]
fn exceptions_carry_typed_args_attributes_module_alias_and_keep_bridge_metadata_apart() {
    let root = checkout("typed_exception");
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    put(&root, "campaigns/ACTIVE", b"\xff");
    let refuse = format!(
        r#"{{"id":"refuse","operation":"derive_path","arguments":{{"module":{module},"kind":"bogus","repo_root":{repo},"camp_id":"zz"}},"module_alias":"r43_alias"}}"#
    );
    let answers = call(&[
        refuse,
        row(
            "decode",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        ),
    ]);
    let refusal = field(&answers[0], "exception");
    assert_eq!(
        json(&field(&refusal, "module")),
        r#""r43_alias""#,
        "classes the original module defines take the declared loader alias"
    );
    assert_eq!(json(&field(&refusal, "suppress_context")), "false");
    assert_eq!(json(&field(&refusal, "relation")), "null");
    assert_eq!(json(&field(&refusal, "attributes")), "{}");
    let frames = json(&field(&refusal, "frames"));
    assert!(
        !frames.contains("\"code\""),
        "the business frames carry file, line and function only: {frames}"
    );
    let bridge = field(&answers[0], "bridge");
    assert_eq!(json(&field(&bridge, "module_alias_missing")), "false");
    assert!(json(&field(&bridge, "frame_code")).contains("raise PurgeGateRefusal"));
    let decode = field(&answers[1], "exception");
    assert_eq!(json(&field(&decode, "module")), r#""builtins""#);
    assert_eq!(
        json(&field(&decode, "args")),
        r#"[["str", "utf-8"], ["bytes", "ff"], ["int", "0"], ["int", "1"], ["str", "invalid start byte"]]"#
    );
    assert_eq!(
        json(&field(&decode, "attributes")),
        r#"{"encoding": ["str", "utf-8"], "object": ["bytes", "ff"], "start": ["int", "0"], "end": ["int", "1"], "reason": ["str", "invalid start byte"]}"#
    );
    // an OSError family member: typed (errno, strerror) args and the four public attributes
    put(&root, "campaigns/ACTIVE", b"camp-x\n");
    std::fs::set_permissions(
        root.join("campaigns/ACTIVE"),
        std::os::unix::fs::PermissionsExt::from_mode(0o000),
    )
    .expect("chmod own fixture");
    if std::fs::File::open(root.join("campaigns/ACTIVE")).is_err() {
        let denied = call(&[row(
            "denied",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        )]);
        let exception = field(&denied[0], "exception");
        assert_eq!(json(&field(&exception, "type")), r#""PermissionError""#);
        assert_eq!(
            json(&field(&exception, "args")),
            r#"[["int", "13"], ["str", "Permission denied"]]"#
        );
        assert!(json(&field(&exception, "attributes")).starts_with(r#"{"errno": ["int", "13"], "strerror": ["str", "Permission denied"], "filename": ["str", ""#));
    }
    // IndexError(4): an int argument, not a string
    let short = call(&[row(
        "short",
        "active_campaign_id",
        r#"{"module":"/a/b/campaign_init.py","repo_root":null}"#,
    )]);
    assert_eq!(
        json(&field(&field(&short[0], "exception"), "args")),
        r#"[["int", "4"]]"#
    );
}

#[test]
fn a_different_campaign_module_is_a_concrete_unmet_condition_not_a_current_source_answer() {
    let root = checkout("different_module");
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        b"raise SystemExit(7)\n",
    );
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    let answers = call_without_bridge("different_module", &[
        row(
            "a",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        ),
        row("b", "evaluate", &format!(r#"{{"repo":{repo}}}"#)),
    ]);
    assert_eq!(json(&field(&answers[0], "kind")), r#""unmet_condition""#);
    assert_eq!(
        json(&field(&answers[0], "route")),
        r#""LegacySourceVariant""#
    );
    // evaluate over a checkout with a non-reviewed campaign module is not answered natively either
    let evaluated = json(&field(&answers[1], "kind"));
    assert!(
        evaluated == r#""exception""# || evaluated == r#""unmet_condition""#,
        "{evaluated}"
    );
}

#[test]
fn the_api_uses_the_file_system_profile_not_the_cli_output_codec() {
    let root = checkout("profile");
    put(&root, "campaigns/ACTIVE", "é\n".as_bytes());
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    // strict C: an ASCII file-system encoding cannot encode the non-ASCII pointer name (pathlib's is_dir swallows it): bootstrap
    let strict = api_call_with(
        &[row(
            "a",
            "active_campaign_id",
            &format!(r#"{{"module":{module},"repo_root":{repo}}}"#),
        )],
        &[
            ("LC_ALL", "C"),
            ("PYTHONUTF8", "0"),
            ("PYTHONCOERCECLOCALE", "0"),
        ],
    );
    assert_eq!(
        json(&field(&strict[0], "value")),
        r#"["str", "_bootstrap"]"#
    );
    // an unsupported profile is a named private error, not a guess
    let child = Command::new(API)
        .arg("evaluate-jsonl")
        .env_clear()
        .env("LC_ALL", "de_DE.ISO-8859-1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("api");
    let out = child.wait_with_output().expect("output");
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("unsupported text profile"));
}

fn bridge_script() -> String {
    js(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../scripts/topology_legacy_api.py"
    ))
}

#[test]
fn a_changed_campaign_source_runs_the_original_function_through_the_approved_generic_child() {
    let root = checkout("legacy_api");
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        b"def active_campaign_id(repo_root=None):\n    return 'from-the-changed-source'\n",
    );
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    let compat = format!(
        r#""compat":{{"python":"python3","bridge_script":{}}},"module_alias":"r43_alias""#,
        bridge_script()
    );
    let rows = vec![
        format!(
            r#"{{"id":"legacy","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},{compat}}}"#
        ),
        format!(
            r#"{{"id":"missing_bridge","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":"/nonexistent/topology_legacy_api.py"}}}}"#
        ),
    ];
    let answers = call(&rows);
    assert_eq!(
        json(&field(&answers[0], "kind")),
        r#""legacy_observation""#,
        "{}",
        json(&answers[0])
    );
    assert_eq!(
        json(&field(&answers[0], "route")),
        r#""LegacySourceVariant""#
    );
    let observation = field(&answers[0], "observation");
    assert_eq!(json(&field(&observation, "exit_code")), "0");
    let pid: i64 = json(&field(&observation, "child_pid"))
        .parse()
        .expect("child pid");
    assert!(pid > 0);
    let sidecar = json(&field(&observation, "sidecar"));
    assert!(
        sidecar.contains("from-the-changed-source") && sidecar.contains("LegacySourceVariant"),
        "{sidecar}"
    );
    assert!(sidecar.contains("\"native_execution\": false"), "{sidecar}");
    // a bridge script that is not there: the interpreter runs and fails; the failure is kept (no sidecar is invented)
    let failed = field(&answers[1], "observation");
    assert!(
        json(&field(&failed, "sidecar_error")).contains("no sidecar"),
        "{}",
        json(&failed)
    );
}

fn api_in(cwd: &Path, tmpdir: &Path, rows: &[String], env: &[(&str, &str)]) -> Vec<PyValue> {
    let mut command = Command::new(API);
    command
        .arg("evaluate-jsonl")
        .current_dir(cwd)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("TMPDIR", tmpdir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (name, value) in env {
        command.env(name, value);
    }
    let mut child = command.spawn().expect("api");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all((rows.join("\n") + "\n").as_bytes())
        .expect("write");
    let out = child.wait_with_output().expect("output");
    assert!(
        out.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout)
        .expect("UTF-8")
        .lines()
        .map(|line| loads(line).expect("answer JSON"))
        .collect()
}

fn private_tmp(label: &str) -> PathBuf {
    scratch(&format!("tmp_{label}"))
}

#[test]
fn a_relative_real_module_and_a_typed_id_go_through_and_the_private_directory_is_gone() {
    let root = checkout("h3_relative");
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        b"def active_campaign_id(repo_root=None):\n    return 'relative-module'\n",
    );
    let tmp = private_tmp("relative");
    let rows = vec![format!(
        r#"{{"id":7,"operation":"active_campaign_id","arguments":{{"module":".claude/skills/terraforming_node/scripts/campaign_init.py","repo_root":null}},"compat":{{"python":"python3","bridge_script":{}}},"module_alias":"r43_alias"}}"#,
        bridge_script()
    )];
    let answers = api_in(&root, &tmp, &rows, &[]);
    assert_eq!(
        json(&field(&answers[0], "kind")),
        r#""legacy_observation""#,
        "{}",
        json(&answers[0])
    );
    let observation = field(&answers[0], "observation");
    let request = field(&observation, "request");
    assert_eq!(
        json(&field(&request, "id")),
        "7",
        "the request id is the typed JSON value, not its display text"
    );
    assert!(
        json(&field(&request, "module_path")).starts_with(&format!("\"{}", root.display())),
        "{}",
        json(&request)
    );
    assert_eq!(
        std::fs::read_dir(&tmp).expect("tmp").count(),
        0,
        "the private directory is removed after the call"
    );
    // a request without any real module path is rejected before dispatch, with no business answer
    let none = api_in(
        &root,
        &tmp,
        &[format!(
            r#"{{"id":8,"operation":"evaluate","arguments":{{"repo":{}}},"compat":{{"python":"python3"}}}}"#,
            js(root.to_str().expect("UTF-8"))
        )],
        &[],
    );
    assert_ne!(json(&field(&none[0], "kind")), r#""value""#);
}

#[test]
fn a_hanging_changed_module_is_a_control_incomplete_observation_not_a_business_result() {
    let root = checkout("h3_hang");
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        b"import time\ntime.sleep(60)\n",
    );
    let tmp = private_tmp("hang");
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    let rows = vec![format!(
        r#"{{"id":"hang","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":{},"deadline_ms":800}}}}"#,
        bridge_script()
    )];
    let started = std::time::Instant::now();
    let answers = api_in(&root, &tmp, &rows, &[]);
    assert!(started.elapsed() < std::time::Duration::from_secs(20));
    assert_eq!(
        json(&field(&answers[0], "kind")),
        r#""legacy_control_incomplete""#,
        "{}",
        json(&answers[0])
    );
    let observation = field(&answers[0], "observation");
    assert_eq!(
        json(&field(&observation, "control_failure")),
        r#""TimeoutExpired""#
    );
    assert_eq!(json(&field(&observation, "complete")), "false");
    assert_ne!(json(&field(&observation, "child_pid")), "null");
    assert_eq!(
        std::fs::read_dir(&tmp).expect("tmp").count(),
        0,
        "the private directory is removed on the timeout path too"
    );
}

#[test]
fn encode_failures_and_a_symlink_loop_carry_typed_args_and_the_implicit_context() {
    let root = checkout("h3_typed");
    let tmp = private_tmp("typed");
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    std::os::unix::fs::symlink("loop2", root.join("loop1")).expect("loop");
    std::os::unix::fs::symlink("loop1", root.join("loop2")).expect("loop");
    let looped = js(root.join("loop1").to_str().expect("UTF-8"));
    let rows = vec![
        row(
            "enc",
            "derive_path",
            &format!(r#"{{"module":{module},"kind":"root","camp_id":"é","repo_root":{repo}}}"#),
        ),
        row("loop", "evaluate", &format!(r#"{{"repo":{looped}}}"#)),
    ];
    let answers = api_in(
        &root,
        &tmp,
        &rows,
        &[
            ("LC_ALL", "C"),
            ("PYTHONUTF8", "0"),
            ("PYTHONCOERCECLOCALE", "0"),
        ],
    );
    let encode = field(&answers[0], "exception");
    assert_eq!(json(&field(&encode, "type")), r#""UnicodeEncodeError""#);
    assert_eq!(
        json(&field(&encode, "args")),
        format!(
            r#"[["str", "ascii"], ["str", {}], ["int", "0"], ["int", "1"], ["str", "ordinal not in range(128)"]]"#,
            js("é")
        )
    );
    let loop_error = field(&answers[1], "exception");
    assert_eq!(json(&field(&loop_error, "type")), r#""RuntimeError""#);
    let relation = field(&loop_error, "relation");
    assert_eq!(json(&field(&relation, "kind")), r#""context""#);
    let cause = field(&relation, "exception");
    assert_eq!(json(&field(&cause, "type")), r#""OSError""#);
    assert_eq!(
        json(&field(&cause, "args")),
        r#"[["int", "40"], ["str", "Too many levels of symbolic links"]]"#
    );
    assert_eq!(json(&field(&cause, "suppress_context")), "false");
}

#[test]
fn a_named_real_owner_that_does_not_exist_is_unmet_for_the_api_too() {
    let root = checkout("h3_owner");
    let repo = js(root.to_str().expect("UTF-8"));
    let answers = call(&[format!(
        r#"{{"id":"o","operation":"evaluate","arguments":{{"repo":{repo}}},"compat_owner":"/nonexistent/topology_parity.py"}}"#
    )]);
    assert_eq!(
        json(&field(&answers[0], "kind")),
        r#""unmet_condition""#,
        "{}",
        json(&answers[0])
    );
}

// ---- H4: N11 deadline validation · N12 bridge_script echo · N13 zero-execution unmet routes ------------------------------

fn changed_module(label: &str, body: &[u8]) -> (PathBuf, String, String) {
    let root = checkout(label);
    put(
        &root,
        ".claude/skills/terraforming_node/scripts/campaign_init.py",
        body,
    );
    let module = module_of(&root);
    let repo = js(root.to_str().expect("UTF-8"));
    (root, module, repo)
}

#[test]
fn n11_an_invalid_declared_deadline_is_an_explicit_unmet_condition_never_a_default() {
    let (_root, module, repo) = changed_module(
        "n11_bad",
        b"def active_campaign_id(repo_root=None):\n    return 'ran'\n",
    );
    let invalid = [
        ("string", r#""5000""#),
        ("negative", "-1"),
        ("float", "1.5"),
        ("bool", "true"),
        ("null", "null"),
        ("list", "[1]"),
        ("beyond_u64", "99999999999999999999999"),
    ];
    let rows: Vec<String> = invalid
        .iter()
        .map(|(label, value)| {
            format!(
                r#"{{"id":"{label}","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":{},"deadline_ms":{value}}}}}"#,
                bridge_script()
            )
        })
        .collect();
    let answers = call(&rows);
    for ((label, _), answer) in invalid.iter().zip(&answers) {
        assert_eq!(
            json(&field(answer, "kind")),
            r#""unmet_condition""#,
            "{label}: {}",
            json(answer)
        );
        assert_eq!(json(&field(answer, "route")), r#""NotExecuted""#, "{label}");
        assert_eq!(json(&field(answer, "executed")), "false", "{label}");
        assert!(
            json(&field(answer, "condition")).contains("InvalidCompatDeadline"),
            "{label}: {}",
            json(answer)
        );
        assert!(
            matches!(answer, PyValue::Dict(d) if d.get("observation").is_none()),
            "{label}: nothing ran, so there is no observation"
        );
    }
}

#[test]
fn n11_valid_positive_and_zero_deadlines_keep_their_declared_meaning() {
    let (_root, module, repo) = changed_module(
        "n11_ok",
        b"import time\ndef active_campaign_id(repo_root=None):\n    time.sleep(30)\n    return 'late'\n",
    );
    let row = |id: &str, deadline: &str| {
        format!(
            r#"{{"id":"{id}","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":{},"deadline_ms":{deadline}}}}}"#,
            bridge_script()
        )
    };
    let answers = call(&[row("zero", "0"), row("short", "700")]);
    for (answer, want) in answers.iter().zip(["0", "700"]) {
        assert_eq!(
            json(&field(answer, "kind")),
            r#""legacy_control_incomplete""#,
            "{}",
            json(answer)
        );
        let observation = field(answer, "observation");
        assert_eq!(json(&field(&observation, "limit_ms")), want);
        assert_eq!(
            json(&field(&observation, "control_failure")),
            r#""TimeoutExpired""#
        );
    }
    // a generous positive deadline runs to the end (and the default 60 s applies only when the key is absent)
    let (_r, module2, repo2) = changed_module(
        "n11_ok2",
        b"def active_campaign_id(repo_root=None):\n    return 'ran'\n",
    );
    let answers = call(&[
        format!(
            r#"{{"id":"p","operation":"active_campaign_id","arguments":{{"module":{module2},"repo_root":{repo2}}},"compat":{{"python":"python3","bridge_script":{},"deadline_ms":20000}}}}"#,
            bridge_script()
        ),
        format!(
            r#"{{"id":"absent","operation":"active_campaign_id","arguments":{{"module":{module2},"repo_root":{repo2}}},"compat":{{"python":"python3","bridge_script":{}}}}}"#,
            bridge_script()
        ),
    ]);
    let limits: Vec<String> = answers
        .iter()
        .map(|a| json(&field(&field(a, "observation"), "limit_ms")))
        .collect();
    assert_eq!(limits, ["20000", "60000"], "{}", json(&answers[0]));
}

#[test]
fn n12_the_observation_echoes_the_bridge_script_that_actually_ran() {
    let (_root, module, repo) = changed_module(
        "n12",
        b"def active_campaign_id(repo_root=None):\n    return 'ran'\n",
    );
    let named = bridge_script();
    let answers = call(&[
        format!(
            r#"{{"id":"named","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":{named}}}}}"#
        ),
        format!(
            r#"{{"id":"hang","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}},"compat":{{"python":"python3","bridge_script":{named},"deadline_ms":0}}}}"#
        ),
    ]);
    assert_eq!(
        json(&field(&field(&answers[0], "observation"), "bridge_script")),
        named
    );
    // the control-incomplete observation carries it too
    assert_eq!(
        json(&field(&answers[1], "kind")),
        r#""legacy_control_incomplete""#
    );
    assert_eq!(
        json(&field(&field(&answers[1], "observation"), "bridge_script")),
        named
    );
}

#[test]
fn n13_zero_execution_unmet_answers_never_claim_the_native_current_source_route() {
    let root = checkout("n13");
    let repo = js(root.to_str().expect("UTF-8"));
    let answers = call(&[
        // an owner gate that is not met: nothing ran
        format!(
            r#"{{"id":"owner","operation":"evaluate","arguments":{{"repo":{repo}}},"compat_owner":"/nonexistent/topology_parity.py"}}"#
        ),
        // the same gate on another operation: nothing ran
        r#"{"id":"header","operation":"read_layer_header","arguments":{"path":"x.md"},"compat_owner":"/nonexistent/topology_parity.py"}"#.to_string(),
    ]);
    for answer in &answers {
        assert_eq!(
            json(&field(answer, "kind")),
            r#""unmet_condition""#,
            "{}",
            json(answer)
        );
        assert_eq!(
            json(&field(answer, "route")),
            r#""NotExecuted""#,
            "{}",
            json(answer)
        );
        assert_eq!(json(&field(answer, "executed")), "false");
    }
    // a source that calls for the compatibility route it cannot get keeps naming that route - and says nothing ran
    let (_r, module, repo) = changed_module("n13_legacy", b"raise SystemExit(7)\n");
    let answers = call_without_bridge("n13_legacy", &[format!(
        r#"{{"id":"legacy","operation":"active_campaign_id","arguments":{{"module":{module},"repo_root":{repo}}}}}"#
    )]);
    assert_eq!(json(&field(&answers[0], "kind")), r#""unmet_condition""#);
    assert_eq!(
        json(&field(&answers[0], "route")),
        r#""LegacySourceVariant""#
    );
    assert_eq!(json(&field(&answers[0], "executed")), "false");
}
