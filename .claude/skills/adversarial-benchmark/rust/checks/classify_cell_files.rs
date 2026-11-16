//! File-system, glob, timestamp and owner-boundary behaviour of `classify_cell` that the frozen rows do not pin, each
//! expectation observed from the original program on CPython 3.12.3 (the `write_bench_mode_record` error positions,
//! the `glob.glob` expansion of `[`/`*` in repository and node names, universal newlines and `errors="replace"` on event
//! files, the original's report text for a missing, failing or misbehaving owner) and recorded here as literals.
//!
//! The owner-boundary checks use throwaway resolver scripts and, for the node-role contract, a CHECK-ONLY native role
//! source (`Owners::with_role_source`) that hands the classifier crafted typed values or raised exceptions: they prove how
//! the program relays what an owner printed, returned or raised - the owners' own rules are not exercised there. The rows
//! that DO exercise the role library (the unused 617-digit key and cycle, the 5000-digit value, a violation) run it for
//! real. No Python runs anywhere in this file.

use agent_runtime_support::pyvalue::{
    DictKey as QKey, NonStringKey as QNonStringKey, Opaque as QOpaque, PyDict as QDict,
    PyValue as Q, Typed,
};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use sweep_stop::classify_cell::{
    bench_mode_record_path, events_node_plan, read_bench_mode_record, scan_node_events,
    write_bench_mode_record, Diagnostics,
};
use sweep_stop::classify_cell_owner::{Owners, BRIDGE_ERROR, CONFIGURATION_ERROR};
use sweep_stop::pyjson::loads;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;
use sweep_stop::role_bridge::{RoleFailure, RoleSource};

fn scratch(label: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("classify_cell_files")
        .join(format!("{}_{label}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch directory");
    dir
}

fn put(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
    std::fs::write(path, bytes).expect("write");
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn json(text: &str) -> PyValue {
    loads(text).expect("fixture JSON")
}

fn leftovers(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(directory)
        .expect("list")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.starts_with(".bench_mode.json."))
        .collect();
    names.sort();
    names
}

// ── write_bench_mode_record ──────────────────────────────────────────────────────

#[test]
fn record_is_written_atomically_with_the_original_bytes_and_mode() {
    let root = scratch("write");
    let sweep = s(root.join("sweep_index.json").to_str().expect("UTF-8"));
    let written = write_bench_mode_record(
        &sweep,
        &json(r#"{"a": [1, {"b": null}], "c": {}, "d": [], "é": "ü "}"#),
    )
    .expect("written");
    assert_eq!(written, bench_mode_record_path(&sweep).expect("path"));
    let bytes = std::fs::read(root.join("bench_mode.json")).expect("record");
    assert_eq!(
        String::from_utf8(bytes).expect("UTF-8"),
        "{\n  \"a\": [\n    1,\n    {\n      \"b\": null\n    }\n  ],\n  \"c\": {},\n  \"d\": [],\n  \"é\": \"ü\u{2028}\"\n}\n"
    );
    let mode = std::fs::metadata(root.join("bench_mode.json"))
        .expect("stat")
        .permissions()
        .mode();
    assert_eq!(
        mode & 0o777,
        0o600,
        "mkstemp's owner-only mode survives the replace"
    );
    assert!(leftovers(&root).is_empty());
}

#[test]
fn a_record_that_cannot_be_encoded_leaves_the_old_record_and_no_temporary_file() {
    let root = scratch("surrogate");
    put(&root, "bench_mode.json", b"old\n");
    let sweep = s(root.join("sweep_index.json").to_str().expect("UTF-8"));
    // (record, position within the chunk json.dump hands the file when it first meets the surrogate)
    for (record, position) in [
        (r#"{"l": ["\udcff"]}"#, "character '\\udcff' in position 7"),
        (
            r#"{"l": [1, ["\udcff"]]}"#,
            "character '\\udcff' in position 9",
        ),
        (
            r#"{"a": {"b": "\udcff"}}"#,
            "character '\\udcff' in position 1",
        ),
        (r#"{"k\udcff": 1}"#, "character '\\udcff' in position 2"),
        (
            r#"{"l": ["x", "\ud800\ud801"]}"#,
            "characters in position 7-8",
        ),
        (r#"["\udcff"]"#, "character '\\udcff' in position 5"),
    ] {
        let failure = write_bench_mode_record(&sweep, &json(record)).expect_err(record);
        assert_eq!(failure.class, "UnicodeEncodeError", "{record}");
        assert_eq!(
            failure.message.to_utf8().expect("ASCII message"),
            format!("'utf-8' codec can't encode {position}: surrogates not allowed"),
            "{record}"
        );
        assert_eq!(
            std::fs::read(root.join("bench_mode.json")).expect("old"),
            b"old\n"
        );
        assert!(leftovers(&root).is_empty(), "{record}");
    }
}

#[test]
fn a_record_slot_that_is_a_directory_fails_in_replace_and_is_cleaned_up() {
    let root = scratch("dirslot");
    put(&root, "bench_mode.json/x", b"x");
    let sweep = s(root.join("sweep_index.json").to_str().expect("UTF-8"));
    let failure =
        write_bench_mode_record(&sweep, &json("{}")).expect_err("a directory is not replaced");
    assert_eq!(failure.class, "IsADirectoryError");
    let message = failure.message.to_utf8().expect("message");
    assert!(
        message.starts_with("[Errno 21] Is a directory: '"),
        "{message}"
    );
    assert!(message.contains("/.bench_mode.json."), "{message}");
    assert!(
        message.ends_with(&format!(
            "' -> '{}'",
            root.join("bench_mode.json").display()
        )),
        "{message}"
    );
    assert!(root.join("bench_mode.json/x").is_file());
    assert!(leftovers(&root).is_empty());
}

#[test]
fn a_record_slot_that_is_a_symlink_is_replaced_as_a_link() {
    let root = scratch("linkslot");
    put(&root, "real.json", b"old");
    std::os::unix::fs::symlink("real.json", root.join("bench_mode.json")).expect("link");
    let sweep = s(root.join("sweep_index.json").to_str().expect("UTF-8"));
    write_bench_mode_record(&sweep, &json("{}")).expect("written");
    assert!(
        !root.join("bench_mode.json").is_symlink(),
        "rename replaces the link itself"
    );
    assert_eq!(
        std::fs::read(root.join("real.json")).expect("target untouched"),
        b"old"
    );
    assert_eq!(
        std::fs::read(root.join("bench_mode.json")).expect("record"),
        b"{}\n"
    );
}

#[test]
fn a_missing_directory_is_the_open_error_of_the_temporary_name() {
    let root = scratch("nodir");
    let sweep = s(root
        .join("missing/sweep_index.json")
        .to_str()
        .expect("UTF-8"));
    let failure = write_bench_mode_record(&sweep, &json("{}")).expect_err("no directory");
    assert_eq!(failure.class, "FileNotFoundError");
    let message = failure.message.to_utf8().expect("message");
    assert!(
        message.starts_with("[Errno 2] No such file or directory: '"),
        "{message}"
    );
    assert!(message.contains("/missing/.bench_mode.json."), "{message}");
}

#[test]
fn reading_a_record_never_folds_absence_damage_or_staleness_into_a_mode() {
    let root = scratch("read");
    let sweep = s(root.join("sweep_index.json").to_str().expect("UTF-8"));
    let read =
        |index: &PyValue| read_bench_mode_record(&sweep, index, &PyValue::None).expect("read");
    let status = |pair: &PyValue| match pair {
        PyValue::Tuple(items) => items[1].display_text().to_utf8().expect("text"),
        other => panic!("not a pair: {}", other.repr()),
    };
    assert!(status(&read(&json("{}"))).starts_with("absent(bench_mode.json 부재"));
    put(&root, "bench_mode.json", b"\xff");
    assert_eq!(
        status(&read(&json("{}"))),
        "unreadable(bench_mode.json 판독 실패: 'utf-8' codec can't decode byte 0xff in position 0: invalid start byte)"
    );
    put(&root, "bench_mode.json", b"\xef\xbb\xbf{}");
    assert_eq!(
        status(&read(&json("{}"))),
        "unreadable(bench_mode.json 판독 실패: Unexpected UTF-8 BOM (decode using utf-8-sig): line 1 column 1 (char 0))"
    );
    put(
        &root,
        "bench_mode.json",
        br#"{"sweep_index_generated_utc": 1}"#,
    );
    // 1 == 1.0 == True in Python: the same measurement time however the writer typed it.
    for same in ["1", "1.0", "true"] {
        let pair = read(&json(&format!(r#"{{"generated_utc": {same}}}"#)));
        assert_eq!(status(&pair), "ok", "{same}");
    }
    assert!(status(&read(&json(r#"{"generated_utc": "n"}"#)))
        .starts_with("stale(bench_mode.json 는 다른 측정(generated_utc=1)의 분류다 · 이 스윕 n)"));
}

// ── scan_node_events: glob and event-file reading ────────────────────────────────

fn scan(repo: &Path, plan: &str) -> (PyValue, Diagnostics) {
    let mut diagnostics = Diagnostics::default();
    let scans = scan_node_events(
        &s(repo.to_str().expect("UTF-8")),
        &json(plan),
        &mut diagnostics,
    )
    .expect("scan");
    (scans, diagnostics)
}

fn files_of(scans: &PyValue, repo: &Path) -> Vec<String> {
    let PyValue::List(entries) = scans else {
        panic!("scans")
    };
    let mut found = Vec::new();
    for entry in entries {
        let PyValue::Dict(entry) = entry else {
            panic!("entry")
        };
        let Some(PyValue::List(files)) = entry.get("files") else {
            panic!("files")
        };
        for file in files {
            let text = file.display_text().to_utf8().expect("text");
            found.push(text.replace(repo.to_str().expect("UTF-8"), "@REPO@"));
        }
    }
    found
}

#[test]
fn glob_characters_in_a_repository_or_node_name_are_patterns_as_in_the_original() {
    let root = scratch("glob");
    for relative in [
        "m[1]/docs/logs/main/events/a.jsonl",
        "m1/docs/logs/main/events/a.jsonl",
        "r/docs/logs/we[ir]d/events/a.jsonl",
        "r/docs/logs/we*/events/a.jsonl",
        "r/docs/logs/main/events/a.jsonl",
    ] {
        put(&root, relative, b"{}\n");
    }
    // `m[1]` is the pattern for `m1`; the literal directory `m[1]` is not matched by it.
    let (scans, _) = scan(&root.join("m[1]"), r#"{"nodes": ["main"]}"#);
    assert_eq!(
        files_of(&scans, &root),
        ["@REPO@/m1/docs/logs/main/events/a.jsonl"]
    );
    // a node name is a pattern as well: `we*` matches both directories, `we[ir]d` matches neither (`weid`/`werd`).
    let (scans, _) = scan(&root.join("r"), r#"{"nodes": ["we*"]}"#);
    assert_eq!(
        files_of(&scans, &root),
        [
            "@REPO@/r/docs/logs/we*/events/a.jsonl",
            "@REPO@/r/docs/logs/we[ir]d/events/a.jsonl"
        ]
    );
    let (scans, _) = scan(&root.join("r"), r#"{"nodes": ["we[ir]d"]}"#);
    assert!(files_of(&scans, &root).is_empty());
}

#[test]
fn event_files_are_read_with_universal_newlines_and_replacement_decoding() {
    let root = scratch("events");
    put(&root, "docs/logs/main/events/b.jsonl", b"one\r\ntwo\rthree");
    put(&root, "docs/logs/main/events/a.jsonl", b"\xffx\n\xc3(\n");
    put(&root, "docs/logs/main/events/.hidden.jsonl", b"hidden\n");
    put(&root, "docs/logs/main/events/notes.txt", b"text\n");
    put(
        &root,
        "docs/logs/main/events/dir.jsonl/inner.jsonl",
        b"inner\n",
    );
    let (scans, diagnostics) = scan(&root, r#"{"nodes": ["main"], "self_node": "main"}"#);
    assert!(diagnostics.stderr.is_empty());
    assert_eq!(
        files_of(&scans, &root),
        [
            "@REPO@/docs/logs/main/events/a.jsonl",
            "@REPO@/docs/logs/main/events/b.jsonl"
        ],
        "sorted, regular files only, no dot-file, no directory named *.jsonl"
    );
    let PyValue::List(entries) = &scans else {
        panic!()
    };
    let PyValue::Dict(entry) = &entries[0] else {
        panic!()
    };
    let Some(PyValue::List(lines)) = entry.get("lines") else {
        panic!()
    };
    let lines: Vec<String> = lines
        .iter()
        .map(|line| line.display_text().to_utf8().expect("text"))
        .collect();
    assert_eq!(
        lines,
        ["\u{fffd}x\n", "\u{fffd}(\n", "one\n", "two\n", "three"]
    );
}

#[test]
fn an_unreadable_event_file_is_warned_about_and_left_out() {
    let root = scratch("unreadable");
    put(&root, "docs/logs/main/events/a.jsonl", b"{}\n");
    put(&root, "docs/logs/main/events/b.jsonl", b"{}\n");
    std::fs::set_permissions(
        root.join("docs/logs/main/events/a.jsonl"),
        std::fs::Permissions::from_mode(0o000),
    )
    .expect("chmod");
    let readable_anyway = std::fs::read(root.join("docs/logs/main/events/a.jsonl")).is_ok();
    let (scans, diagnostics) = scan(&root, r#"{"nodes": ["main"], "self_node": "main"}"#);
    if readable_anyway {
        // Running as a user the permission bits do not bind: nothing to observe, and nothing silently skipped.
        assert!(diagnostics.stderr.is_empty());
        return;
    }
    let warning = diagnostics.stderr.to_utf8().expect("text");
    assert!(
        warning.starts_with("[classify_cell] WARN events 판독 실패("),
        "{warning}"
    );
    assert!(
        warning.contains("[Errno 13] Permission denied: '"),
        "{warning}"
    );
    assert_eq!(
        files_of(&scans, &root),
        ["@REPO@/docs/logs/main/events/b.jsonl"]
    );
}

// ── the owners ───────────────────────────────────────────────────────────────────

/// A scripts directory whose siblings are where the original looked for its owners.
struct Layout {
    root: PathBuf,
}

impl Layout {
    fn new(label: &str) -> Self {
        let root = scratch(label);
        std::fs::create_dir_all(root.join(".claude/skills/adversarial-benchmark/scripts"))
            .expect("scripts");
        Layout { root }
    }

    fn scripts(&self) -> PyStr {
        PyStr::new(
            self.root
                .join(".claude/skills/adversarial-benchmark/scripts")
                .to_str()
                .expect("UTF-8"),
        )
    }

    /// The stand-in resolver seam: the throwaway Bash scripts below model any resolver behavior (exit codes, stderr,
    /// slowness) through the same pipes, deadline and decoding as production. The native resolver has its own checks
    /// (`node_identity_native`).
    fn owners(&self) -> Owners {
        Owners::new(Some(self.scripts())).with_stand_in_resolver_for_checks()
    }

    fn resolver(&self, body: &str) {
        put(
            &self.root,
            ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
            body.as_bytes(),
        );
    }

    fn runtime_resolver(&self, body: &str) {
        put(
            &self.root,
            ".claude/runtime/node_blackbox/node_identity.sh",
            body.as_bytes(),
        );
    }
}

fn resolve(owners: &Owners, repo: &Path) -> (Option<String>, String) {
    let (node, source) = owners
        .resolve_self_node(&s(repo.to_str().expect("UTF-8")))
        .expect("resolve");
    (
        node.map(|n| n.to_utf8().expect("text")),
        source.to_utf8().expect("text"),
    )
}

#[test]
fn a_missing_resolver_is_reported_with_the_originals_text_and_never_guessed() {
    let layout = Layout::new("no_resolver");
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(node, None);
    assert_eq!(
        source,
        "node_identity.sh 부재(terraforming_node/scripts/node_blackbox · .claude/runtime/node_blackbox)"
    );
    let repo_missing = layout
        .owners()
        .resolve_self_node(&PyValue::None)
        .expect("resolve");
    assert_eq!(repo_missing.1.to_utf8().expect("text"), "repo 미지정");
}

#[test]
fn the_resolvers_answer_failure_and_runtime_copy_are_relayed_as_the_original_does() {
    let layout = Layout::new("resolver");
    layout.resolver("#!/bin/bash\necho '  main  '\n");
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(
        (node.as_deref(), source.as_str()),
        (
            Some("main"),
            "node_identity.sh --resolve(self_role → 유일한 role: main)"
        )
    );

    // Only the first two lines of stderr are looked at (blank ones count), each stripped; the blank ones are then dropped.
    layout.resolver(
        "#!/bin/bash\necho '  boom one  ' >&2\necho 'two' >&2\necho 'three' >&2\nexit 3\n",
    );
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=3 — boom one / two")
    );
    layout.resolver("#!/bin/bash\necho 'boom one' >&2\necho >&2\necho 'two' >&2\nexit 3\n");
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=3 — boom one")
    );
    layout.resolver("#!/bin/bash\nexit 4\n");
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=4")
    );

    layout.resolver("#!/bin/bash\necho '../x'\n");
    let (node, source) = resolve(&layout.owners(), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=0"),
        "a traversal answer is refused"
    );

    // The owner copy wins; only when it is absent is the delivered runtime copy asked.
    std::fs::remove_file(
        layout
            .root
            .join(".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh"),
    )
    .expect("remove");
    layout.runtime_resolver("#!/bin/bash\necho sub\n");
    let (node, _) = resolve(&layout.owners(), &layout.root);
    assert_eq!(node.as_deref(), Some("sub"));
}

#[test]
fn a_resolver_that_does_not_answer_in_time_is_a_reported_failure_naming_the_command() {
    let layout = Layout::new("timeout");
    layout.resolver("#!/bin/bash\nsleep 30\n");
    let (node, source) = resolve(&layout.owners().with_timeout_seconds(1), &layout.root);
    assert_eq!(node, None);
    let resolver = layout
        .root
        .join(".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh");
    let expected = format!(
        "node_identity.sh 실행 실패: Command '['bash', '{}', '--resolve', '--repo', '{}']' timed out after 1 seconds",
        resolver.display(),
        layout.root.display()
    );
    assert_eq!(source, expected);
}

#[test]
fn an_unconfigured_scripts_directory_is_a_named_error_not_an_absent_owner() {
    let owners = Owners::new(None);
    let failure = owners
        .resolve_self_node(&s("/anywhere"))
        .expect_err("needs the scripts directory");
    assert_eq!(failure.class, CONFIGURATION_ERROR);
    let failure = events_node_plan(
        &s("single"),
        &PyValue::None,
        &PyValue::None,
        &PyValue::None,
        &s("/anywhere"),
        &owners,
    )
    .expect_err("a plan that asks the resolver needs it too");
    assert_eq!(failure.class, CONFIGURATION_ERROR);
}

/// A CHECK-ONLY role source: what the stand-in owners of the original's checks returned or raised, as typed values of the
/// role library's own value family (Q). It holds no rule; the test writes the answers.
struct StandIn {
    load: Result<Q, RoleFailure>,
    evaluate: Result<Q, RoleFailure>,
}

impl std::fmt::Debug for StandIn {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("StandIn")
    }
}

impl RoleSource for StandIn {
    fn load_yaml(&self, _path: &sweep_stop::pystr::PyStr) -> Result<Q, RoleFailure> {
        self.load.clone()
    }

    fn evaluate_manifest(&self, _document: &Q, _topology: &str) -> Result<Q, RoleFailure> {
        self.evaluate.clone()
    }
}

fn qs(text: &str) -> Q {
    Q::str(text)
}

fn qi(number: i64) -> Q {
    Q::int(number)
}

fn qd(pairs: Vec<(&str, Q)>) -> Q {
    let mut dict = QDict::new();
    for (key, value) in pairs {
        dict.set(key, value);
    }
    Q::Dict(dict)
}

fn ql(items: Vec<Q>) -> Q {
    Q::List(items)
}

fn qt(items: Vec<Q>) -> Q {
    Q::Tuple(items)
}

/// The inner reference of a self-containing value, as the YAML loader writes it (`[...]` / `{...}`).
fn marker(type_name: &'static str, repr: &str) -> Q {
    Q::Opaque(QOpaque {
        type_name,
        repr: repr.to_string(),
        truthy: true,
        typed: Typed::Plain,
        identity: None,
    })
}

fn other_key(dict: &mut QDict, identity: &str, repr: &str, key: Q, value: Q) {
    dict.insert_key(
        QKey::Other(QNonStringKey {
            identity: identity.to_string(),
            repr: repr.to_string(),
            value: Some(Box::new(key)),
        }),
        value,
    );
}

fn raising(class: &'static str, message: &str) -> Result<Q, RoleFailure> {
    Err(RoleFailure::new(class, message))
}

/// `{'nodes': [{'role': r}, ...]}`
fn nodes_of(roles: Vec<Q>) -> Q {
    qd(vec![(
        "nodes",
        ql(roles
            .into_iter()
            .map(|role| qd(vec![("role", role)]))
            .collect()),
    )])
}

fn nodes() -> Q {
    nodes_of(vec![qs("main"), qs("sub")])
}

fn clean() -> Q {
    qd(vec![("violations", ql(vec![]))])
}

/// The presence of the owner FILE is all the program looks up; its content is never read or run.
const OWNER_FILE: &str = ".claude/skills/terraforming_node/scripts/node_role_contract.sh";
const MANIFEST: &str = "self_role: main\ntopology: multi\nnodes:\n  - role: main\n  - role: sub\n";

fn multi_plan(
    owners: &Owners,
    repo: &Path,
    manifest: &Path,
) -> Result<PyValue, sweep_stop::guidellm::Failure> {
    events_node_plan(
        &s("multi"),
        &PyValue::None,
        &PyValue::None,
        &s(manifest.to_str().expect("UTF-8")),
        &s(repo.to_str().expect("UTF-8")),
        owners,
    )
}

fn field(plan: &PyValue, key: &str) -> String {
    let PyValue::Dict(map) = plan else {
        panic!("plan")
    };
    map.get(key).map_or("<absent>".to_string(), |value| {
        value.display_text().to_utf8().expect("text")
    })
}

fn owned_layout(label: &str) -> (Layout, PathBuf) {
    let layout = Layout::new(label);
    layout.resolver("#!/bin/bash\necho main\n");
    put(&layout.root, "manifest.yaml", MANIFEST.as_bytes());
    put(&layout.root, OWNER_FILE, b"# the file is only looked up\n");
    let manifest = layout.root.join("manifest.yaml");
    (layout, manifest)
}

/// The plan for a multi manifest when the role source answers `load` and `evaluate`.
fn plan_with(
    label: &str,
    load: Result<Q, RoleFailure>,
    evaluate: Result<Q, RoleFailure>,
) -> Result<PyValue, sweep_stop::guidellm::Failure> {
    let (layout, manifest) = owned_layout(label);
    let owners = layout
        .owners()
        .with_role_source(Arc::new(StandIn { load, evaluate }));
    multi_plan(&owners, &layout.root, &manifest)
}

/// RETIRED ROW `a_role_contract_owner_that_exists_but_cannot_be_queried_is_an_error_never_a_fallback`: it pinned the
/// failure of the removed Python transport (an owner present but no query adapter configured, the named
/// `RoleContractAdapterError`). That transport no longer exists, so the condition cannot arise. Its intent - an owner that
/// exists is NEVER answered by a fallback and never needs Python - is carried by this row: an owner file whose content
/// is not Python at all is answered natively by the `role-contract` library, no adapter is configured anywhere, the
/// bridge starts no process, and the owner-absent report is still the original's.
#[test]
fn a_present_owner_file_is_answered_natively_with_no_adapter_and_no_python_path() {
    let (layout, manifest) = owned_layout("native");
    // no role source installed: the production library answers
    let plan = multi_plan(&layout.owners(), &layout.root, &manifest).expect("plan");
    assert_eq!(field(&plan, "nodes"), "['main', 'sub']");
    assert_eq!(field(&plan, "undeterminable"), "None");
    assert_eq!(
        field(&plan, "source"),
        "multi: 서빙 참여 노드 전부 — manifest nodes[].role(main,sub) · node_role_contract 위반 0"
    );
    // structural: no adapter/interpreter selection and no process start exist in the role path
    let owner_source = include_str!("../src/classify_cell_owner.rs");
    let bridge_source = include_str!("../src/role_bridge.rs");
    for forbidden in ["ROLE_ADAPTER", "PYTHON_ENV", "RoleSession", "ADAPTER_ERROR"] {
        assert!(!owner_source.contains(forbidden), "{forbidden}");
    }
    assert!(
        !bridge_source.contains("Command"),
        "the bridge starts no process"
    );
    assert!(!bridge_source.to_lowercase().contains("python3"));
    // the owner-absent report is the original's, not an error
    let bare = Layout::new("ownerless");
    bare.resolver("#!/bin/bash\necho main\n");
    put(&bare.root, "manifest.yaml", MANIFEST.as_bytes());
    let plan =
        multi_plan(&bare.owners(), &bare.root, &bare.root.join("manifest.yaml")).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 부재 — multi 참여 노드를 판정할 소유자가 없다"
    );
    assert_eq!(
        field(&plan, "self_node"),
        "main",
        "this node's records are still read"
    );
}

/// `_node_role_contract` of the API returns whether the owner file exists, nothing more.
#[test]
fn the_role_contract_operation_answers_whether_the_owner_file_exists() {
    let (layout, _) = owned_layout("opbool");
    let bare = Layout::new("opbool_absent");
    let ask = |owners: &Owners| {
        let mut diagnostics = Diagnostics::default();
        sweep_stop::classify_cell_api_cli::evaluate_operation(
            "_node_role_contract",
            &sweep_stop::pyvalue::PyDict::new(),
            owners,
            &mut diagnostics,
        )
        .expect("answers")
    };
    assert_eq!(ask(&layout.owners()).repr(), "True");
    assert_eq!(ask(&bare.owners()).repr(), "False");
    assert!(layout
        .owners()
        .open_role_contract()
        .expect("config")
        .is_some());
    assert!(bare
        .owners()
        .open_role_contract()
        .expect("config")
        .is_none());
}

/// STAND-IN ROWS of the original's owner checks (every expectation unchanged), re-expressed through the check-only native
/// role source with typed values: each `plan_with(label, load, evaluate)` below is the row of the same label that used a
/// Python stand-in owner returning/raising the same object. The one row that needs a Python top level is
/// `an_owner_whose_top_level_raises_is_read_as_no_owner` (NOT_RUN, D1).
#[test]
fn owner_answers_and_exceptions_become_the_originals_plan_reports() {
    // row `ok`
    let ok = plan_with("ok", Ok(nodes()), Ok(clean())).expect("plan");
    assert_eq!(field(&ok, "nodes"), "['main', 'sub']");
    assert_eq!(field(&ok, "undeterminable"), "None");
    assert_eq!(field(&ok, "self_node"), "main");

    // row `yamlraise`: load_yaml raised: the plan says so with the exception text.
    let (layout, manifest) = owned_layout("yamlraise");
    let owners = layout.owners().with_role_source(Arc::new(StandIn {
        load: raising("ValueError", "bad yaml"),
        evaluate: Ok(clean()),
    }));
    let plan = multi_plan(&owners, &layout.root, &manifest).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        format!("manifest 판독 실패({}): bad yaml", manifest.display())
    );

    // row `notdict`: a document that is not an object.
    let plan = plan_with("notdict", Ok(ql(vec![])), Ok(clean())).expect("plan");
    assert!(field(&plan, "undeterminable").starts_with("manifest 가 객체가 아니다("));

    // row `violations`: their codes, joined (a violation without a code is `?`).
    let plan = plan_with(
        "violations",
        Ok(nodes()),
        Ok(qd(vec![(
            "violations",
            ql(vec![qd(vec![("code", qs("V1"))]), qd(vec![])]),
        )])),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 위반: V1; ?"
    );
    // row `badcode`: a non-str code
    let failure = plan_with(
        "badcode",
        Ok(nodes()),
        Ok(qd(vec![(
            "violations",
            ql(vec![qd(vec![("code", qi(5))])]),
        )])),
    )
    .expect_err("a non-str code");
    assert_eq!(
        (
            failure.class,
            failure.message.to_utf8().expect("text").as_str()
        ),
        (
            "TypeError",
            "sequence item 0: expected str instance, int found"
        )
    );
    // row `badviolation`: a non-dict violation
    let failure = plan_with(
        "badviolation",
        Ok(nodes()),
        Ok(qd(vec![("violations", ql(vec![qi(5)]))])),
    )
    .expect_err("a non-dict violation");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "'int' object has no attribute 'get'"
    );

    // rows `twice`, `others`, `absent`: a duplicate role, an unusable role, this node not among the roles.
    let plan = plan_with("twice", Ok(nodes_of(vec![qs("a"), qs("a")])), Ok(clean())).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다(['a', 'a'] — node_id = role 슬러그 · 비었거나 중복)"
    );
    let others = qd(vec![(
        "nodes",
        ql(vec![
            qd(vec![("role", qs("a"))]),
            qd(vec![("role", qs("b"))]),
            qs("skipped"),
            qd(vec![("name", qs("no role"))]),
        ]),
    )]);
    let plan = plan_with("others", Ok(others), Ok(clean())).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다(['a', 'b', None] — node_id = role 슬러그 · 비었거나 중복)"
    );
    let plan =
        plan_with("absent", Ok(nodes_of(vec![qs("a"), qs("b")])), Ok(clean())).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "이 노드('main' ← node_identity.sh --resolve(self_role → 유일한 role: main))가 manifest nodes[].role(a,b)에 없다 — 어느 노드의 events 가 이 저장소의 것인지 모른다"
    );

    // rows `listrole`, `tuplelist`, `tuplerole`: unhashable roles are the original's `TypeError` from `set(roles)`; a tuple
    // role is judged by its content.
    let failure = plan_with(
        "listrole",
        Ok(nodes_of(vec![ql(vec![qs("main")])])),
        Ok(clean()),
    )
    .expect_err("unhashable");
    assert_eq!(
        (
            failure.class,
            failure.message.to_utf8().expect("text").as_str()
        ),
        ("TypeError", "unhashable type: 'list'")
    );
    let failure = plan_with(
        "tuplelist",
        Ok(nodes_of(vec![qt(vec![qs("a"), ql(vec![qs("b")])])])),
        Ok(clean()),
    )
    .expect_err("unhashable");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "unhashable type: 'list'"
    );
    let plan = plan_with(
        "tuplerole",
        Ok(nodes_of(vec![qt(vec![qs("a"), qs("b")])])),
        Ok(clean()),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다([('a', 'b')] — node_id = role 슬러그 · 비었거나 중복)"
    );

    // row `evalraise`: an exception of evaluate_manifest is not caught by the original: it propagates with its class and text.
    let failure =
        plan_with("evalraise", Ok(nodes()), raising("KeyError", "'x'")).expect_err("propagates");
    assert_eq!(
        (
            failure.class,
            failure.message.to_utf8().expect("text").as_str()
        ),
        ("KeyError", "'x'")
    );
    // row `evalnotdict`: so does a result that has no `.get`.
    let failure = plan_with("evalnotdict", Ok(nodes()), Ok(qi(5))).expect_err("propagates");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "'int' object has no attribute 'get'"
    );
}

/// NOT_RUN row `badtop` of the original's owner check (D1 of `PORTING.md`): an owner file whose top level raises on import
/// was read as "no owner" by the original, which executed the file. The native owner never executes the file (the
/// `role-contract` library is the single definition of the policy; the file is only looked up), so this one case cannot be
/// observed. It is not waived: it stays listed here as an ignored row with its expectation.
#[test]
#[ignore = "NOT_RUN D1: an owner file whose top level raises is not executed natively; expectation: undeterminable == 'node_role_contract 부재 — multi 참여 노드를 판정할 소유자가 없다'"]
fn an_owner_whose_top_level_raises_is_read_as_no_owner() {
    panic!("NOT_RUN D1: unrepresentable by a native owner; see PORTING.md");
}

/// Two valid manifests the first transport refused because of metadata the classifier never reads (the original returns a
/// valid multi plan for both): an unused integer key too large for a float, and an unused self-referencing anchor. The
/// document stays one shared value and only what the classifier touches is converted. Through the check-only seam with typed
/// values (rows `bigkey`, `cycle`) and, for the same condition, through the REAL role library on YAML text.
#[test]
fn metadata_the_classifier_never_reads_cannot_change_a_valid_plan() {
    let big_two_2048 = Q::int(num_bigint::BigInt::from(1u8) << 2048);
    let big_meta = Q::int(num_bigint::BigInt::from(1u8) << 4096);
    // row `bigkey`
    let mut with_keys = QDict::new();
    for (key, value) in match nodes() {
        Q::Dict(dict) => dict.iter().cloned().collect::<Vec<_>>(),
        _ => unreachable!(),
    } {
        with_keys.insert(key, value);
    }
    other_key(
        &mut with_keys,
        "int:2048",
        "<2**2048>",
        big_two_2048,
        qs("unused"),
    );
    other_key(&mut with_keys, "float:1.0", "1.0", Q::Float(1.0), qs("a"));
    other_key(
        &mut with_keys,
        "float:nan",
        "nan",
        Q::Float(f64::NAN),
        qs("c"),
    );
    other_key(
        &mut with_keys,
        "int:2**70",
        "1180591620717411303424",
        Q::int(1i128 << 70),
        qs("d"),
    );
    // row `cycle`: the inner references of a self-containing list and mapping
    let looped = qd(vec![
        (
            "nodes",
            match nodes() {
                Q::Dict(d) => d.get("nodes").cloned().expect("nodes"),
                _ => unreachable!(),
            },
        ),
        ("unused", ql(vec![marker("list", "[...]")])),
        (
            "also",
            qd(vec![
                ("x", ql(vec![marker("list", "[...]")])),
                ("self", marker("dict", "{...}")),
            ]),
        ),
    ]);
    let meta_result = qd(vec![("violations", ql(vec![])), ("meta", big_meta)]);
    for (label, load) in [("bigkey", Q::Dict(with_keys)), ("cycle", looped)] {
        let plan = plan_with(label, Ok(load), Ok(meta_result.clone())).expect(label);
        assert_eq!(field(&plan, "nodes"), "['main', 'sub']", "{label}");
        assert_eq!(field(&plan, "undeterminable"), "None", "{label}");
        assert_eq!(field(&plan, "self_node"), "main", "{label}");
    }
    // a self-referencing value that IS read fails on demand - as a failure of the bridge, not as a plan (row `readcycle`)
    let cyclic_role = qt(vec![ql(vec![qs("x"), marker("list", "[...]")])]);
    let failure = plan_with("readcycle", Ok(nodes_of(vec![cyclic_role])), Ok(clean()))
        .expect_err("a tuple role that contains itself cannot be materialised");
    assert_eq!(
        failure.class,
        BRIDGE_ERROR,
        "{}",
        failure.line().to_utf8().expect("text")
    );

    // the REAL library on YAML text: a 617-digit unused key and an unused cycle are never read
    for (label, extra) in [
        ("realkey", format!("{}: unused\n", "9".repeat(617))),
        ("realcycle", "unused: &loop [*loop]\n".to_string()),
    ] {
        let (layout, manifest) = owned_layout(label);
        std::fs::write(
            &manifest,
            format!("topology: multi\nnodes:\n  - role: main\n  - role: sub\n{extra}"),
        )
        .expect("manifest");
        let plan = multi_plan(&layout.owners(), &layout.root, &manifest).expect(label);
        assert_eq!(field(&plan, "nodes"), "['main', 'sub']", "{label}");
        assert_eq!(field(&plan, "undeterminable"), "None", "{label}");
    }
}

/// Integers the classifier reads cross exactly (BigInt, no float rounding), and the fixed 4300-digit limit of the owner's
/// YAML loader is reproduced by the real library for a 5000-digit VALUE.
#[test]
fn a_huge_integer_the_classifier_reads_is_exact_and_the_digit_limit_is_the_owners() {
    // row `hugecode`
    let huge = Q::int(num_bigint::BigInt::from(1u8) << 20000);
    let failure = plan_with(
        "hugecode",
        Ok(nodes()),
        Ok(qd(vec![("violations", ql(vec![qd(vec![("code", huge)])]))])),
    )
    .expect_err("an integer code is not a string");
    assert_eq!(
        (
            failure.class,
            failure.message.to_utf8().expect("text").as_str()
        ),
        (
            "TypeError",
            "sequence item 0: expected str instance, int found"
        )
    );
    // row `ownerlimit`: a code that is the text of the limit (the stand-in printed `sys.get_int_max_str_digits()`)
    let plan = plan_with(
        "ownerlimit",
        Ok(nodes()),
        Ok(qd(vec![(
            "violations",
            ql(vec![qd(vec![("code", qs("4300"))])]),
        )])),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 위반: 4300"
    );
    // the real library: a 5000-digit decimal VALUE is refused at the owner's fixed limit
    let (layout, manifest) = owned_layout("realdigits");
    std::fs::write(
        &manifest,
        format!(
            "topology: multi\nnodes:\n  - role: main\n  - role: sub\nbig: {}\n",
            "9".repeat(5000)
        ),
    )
    .expect("manifest");
    let plan = multi_plan(&layout.owners(), &layout.root, &manifest).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        format!(
            "manifest 판독 실패({}): Exceeds the limit (4300 digits) for integer string conversion: value has 5000 digits; use sys.set_int_max_str_digits() to increase the limit",
            manifest.display()
        )
    );
}

/// A violation produced by the REAL library (no stand-in): the manifest declares `a2a-agent` for the sub of a multi
/// topology, which the owner's rule `SUB_MODE_TOPOLOGY_CONFLICT` refuses.
#[test]
fn a_violation_of_the_real_role_library_is_reported_with_its_code() {
    let (layout, manifest) = owned_layout("realviolation");
    std::fs::write(
        &manifest,
        "topology: multi\nnodes:\n  - role: main\n  - role: sub\n    sub_mode: a2a-agent\n",
    )
    .expect("manifest");
    let plan = multi_plan(&layout.owners(), &layout.root, &manifest).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 위반: SUB_MODE_TOPOLOGY_CONFLICT"
    );
}
