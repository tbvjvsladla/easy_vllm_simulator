//! File-system, glob, timestamp and owner-boundary behaviour of `classify_cell` that the frozen rows do not pin, each
//! expectation observed from the original program on CPython 3.12.3 (the `write_bench_mode_record` error positions,
//! the `glob.glob` expansion of `[`/`*` in repository and node names, universal newlines and `errors="replace"` on event
//! files, the original's report text for a missing, failing or misbehaving owner) and recorded here as literals.
//!
//! The owner-boundary checks use throwaway resolver scripts and query adapters written into a scratch tree: they prove
//! how the program relays what an owner printed, returned or raised - the owners' own rules are not exercised here.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use sweep_stop::classify_cell::{
    bench_mode_record_path, events_node_plan, read_bench_mode_record, scan_node_events,
    write_bench_mode_record, Diagnostics,
};
use sweep_stop::classify_cell_owner::{Owners, ADAPTER_ERROR, CONFIGURATION_ERROR};
use sweep_stop::pyjson::loads;
use sweep_stop::pystr::PyStr;
use sweep_stop::pyvalue::PyValue;

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

    fn owners(&self, adapter: Option<&Path>) -> Owners {
        Owners::new(
            Some(self.scripts()),
            "python3",
            adapter.map(|path| path.as_os_str().to_owned()),
        )
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
    let (node, source) = resolve(&layout.owners(None), &layout.root);
    assert_eq!(node, None);
    assert_eq!(
        source,
        "node_identity.sh 부재(terraforming_node/scripts/node_blackbox · .claude/runtime/node_blackbox)"
    );
    let repo_missing = layout
        .owners(None)
        .resolve_self_node(&PyValue::None)
        .expect("resolve");
    assert_eq!(repo_missing.1.to_utf8().expect("text"), "repo 미지정");
}

#[test]
fn the_resolvers_answer_failure_and_runtime_copy_are_relayed_as_the_original_does() {
    let layout = Layout::new("resolver");
    layout.resolver("#!/bin/bash\necho '  main  '\n");
    let (node, source) = resolve(&layout.owners(None), &layout.root);
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
    let (node, source) = resolve(&layout.owners(None), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=3 — boom one / two")
    );
    layout.resolver("#!/bin/bash\necho 'boom one' >&2\necho >&2\necho 'two' >&2\nexit 3\n");
    let (node, source) = resolve(&layout.owners(None), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=3 — boom one")
    );
    layout.resolver("#!/bin/bash\nexit 4\n");
    let (node, source) = resolve(&layout.owners(None), &layout.root);
    assert_eq!(
        (node, source.as_str()),
        (None, "node_identity.sh --resolve rc=4")
    );

    layout.resolver("#!/bin/bash\necho '../x'\n");
    let (node, source) = resolve(&layout.owners(None), &layout.root);
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
    let (node, _) = resolve(&layout.owners(None), &layout.root);
    assert_eq!(node.as_deref(), Some("sub"));
}

#[test]
fn a_resolver_that_does_not_answer_in_time_is_a_reported_failure_naming_the_command() {
    let layout = Layout::new("timeout");
    layout.resolver("#!/bin/bash\nsleep 30\n");
    let (node, source) = resolve(&layout.owners(None).with_timeout_seconds(1), &layout.root);
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
    let owners = Owners::new(None, "python3", None);
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

/// The query adapter under test (the program root applies as `classify_cell_role_query.py`), and a stand-in owner module
/// whose `load_yaml` / `evaluate_manifest` bodies the test writes: it returns crafted Python objects (the owner's own
/// parsing and rules are not exercised here, only how the program reads what an owner returns or raises).
const ADAPTER_SOURCE: &str = include_str!("data/classify_cell/classify_cell_role_query.py");

fn install_owner(layout: &Layout, load_yaml: &str, evaluate: &str) -> PathBuf {
    let adapter = layout.root.join("classify_cell_role_query.py");
    std::fs::write(&adapter, ADAPTER_SOURCE).expect("adapter");
    let indent = |body: &str| {
        body.lines()
            .map(|line| format!("    {line}\n"))
            .collect::<String>()
    };
    let module = format!(
        "def load_yaml(path):\n{}\n\ndef evaluate_manifest(doc, topology):\n{}\n",
        indent(load_yaml),
        indent(evaluate)
    );
    put(
        &layout.root,
        ".claude/skills/terraforming_node/scripts/node_role_contract.py",
        module.as_bytes(),
    );
    adapter
}

const MANIFEST: &str = "self_role: main\ntopology: multi\nnodes:\n  - role: main\n  - role: sub\n";
const NODES: &str = "{'nodes': [{'role': 'main'}, {'role': 'sub'}]}";
const CLEAN: &str = "{'violations': []}";

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
    let manifest = layout.root.join("manifest.yaml");
    (layout, manifest)
}

fn plan_with(
    label: &str,
    load_yaml: &str,
    evaluate: &str,
) -> Result<PyValue, sweep_stop::guidellm::Failure> {
    let (layout, manifest) = owned_layout(label);
    let adapter = install_owner(&layout, load_yaml, evaluate);
    multi_plan(&layout.owners(Some(&adapter)), &layout.root, &manifest)
}

#[test]
fn a_role_contract_owner_that_exists_but_cannot_be_queried_is_an_error_never_a_fallback() {
    let layout = Layout::new("adapterless");
    layout.resolver("#!/bin/bash\necho main\n");
    put(
        &layout.root,
        ".claude/skills/terraforming_node/scripts/node_role_contract.py",
        b"# owner\n",
    );
    put(&layout.root, "manifest.yaml", MANIFEST.as_bytes());
    let failure = multi_plan(
        &layout.owners(None),
        &layout.root,
        &layout.root.join("manifest.yaml"),
    )
    .expect_err("no adapter configured");
    assert_eq!(failure.class, ADAPTER_ERROR);
    assert!(failure
        .message
        .to_utf8()
        .expect("text")
        .contains("does not fall back"));
    // No owner file at all is the original's "owner absent" report, not an error.
    let bare = Layout::new("ownerless");
    bare.resolver("#!/bin/bash\necho main\n");
    put(&bare.root, "manifest.yaml", MANIFEST.as_bytes());
    let plan = multi_plan(
        &bare.owners(None),
        &bare.root,
        &bare.root.join("manifest.yaml"),
    )
    .expect("plan");
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

#[test]
fn owner_answers_and_exceptions_become_the_originals_plan_reports() {
    let ok = plan_with("ok", &format!("return {NODES}"), &format!("return {CLEAN}")).expect("plan");
    assert_eq!(field(&ok, "nodes"), "['main', 'sub']");
    assert_eq!(field(&ok, "undeterminable"), "None");
    assert_eq!(field(&ok, "self_node"), "main");

    // The owner's top level raised: the original read that as "no owner".
    let (layout, manifest) = owned_layout("badtop");
    let adapter = install_owner(&layout, "return {}", "return {}");
    let owner = layout
        .root
        .join(".claude/skills/terraforming_node/scripts/node_role_contract.py");
    std::fs::write(&owner, "raise ImportError('x')\n").expect("owner");
    let plan = multi_plan(&layout.owners(Some(&adapter)), &layout.root, &manifest).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 부재 — multi 참여 노드를 판정할 소유자가 없다"
    );

    // load_yaml raised: the plan says so with the exception text.
    let (layout, manifest) = owned_layout("yamlraise");
    let adapter = install_owner(
        &layout,
        "raise ValueError('bad yaml')",
        &format!("return {CLEAN}"),
    );
    let plan = multi_plan(&layout.owners(Some(&adapter)), &layout.root, &manifest).expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        format!("manifest 판독 실패({}): bad yaml", manifest.display())
    );

    // A document that is not an object.
    let plan = plan_with("notdict", "return []", &format!("return {CLEAN}")).expect("plan");
    assert!(field(&plan, "undeterminable").starts_with("manifest 가 객체가 아니다("));

    // Violations: their codes, joined (a violation without a code is `?`).
    let plan = plan_with(
        "violations",
        &format!("return {NODES}"),
        "return {'violations': [{'code': 'V1'}, {}]}",
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 위반: V1; ?"
    );
    let failure = plan_with(
        "badcode",
        &format!("return {NODES}"),
        "return {'violations': [{'code': 5}]}",
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
    let failure = plan_with(
        "badviolation",
        &format!("return {NODES}"),
        "return {'violations': [5]}",
    )
    .expect_err("a non-dict violation");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "'int' object has no attribute 'get'"
    );

    // A duplicate role, an unusable role, this node not among the roles.
    let twice = "{'nodes': [{'role': 'a'}, {'role': 'a'}]}";
    let plan = plan_with(
        "twice",
        &format!("return {twice}"),
        &format!("return {CLEAN}"),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다(['a', 'a'] — node_id = role 슬러그 · 비었거나 중복)"
    );
    let others = "{'nodes': [{'role': 'a'}, {'role': 'b'}, 'skipped', {'name': 'no role'}]}";
    let plan = plan_with(
        "others",
        &format!("return {others}"),
        &format!("return {CLEAN}"),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다(['a', 'b', None] — node_id = role 슬러그 · 비었거나 중복)"
    );
    let plan = plan_with(
        "absent",
        "return {'nodes': [{'role': 'a'}, {'role': 'b'}]}",
        &format!("return {CLEAN}"),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "이 노드('main' ← node_identity.sh --resolve(self_role → 유일한 role: main))가 manifest nodes[].role(a,b)에 없다 — 어느 노드의 events 가 이 저장소의 것인지 모른다"
    );

    // Unhashable roles are the original's `TypeError` from `set(roles)`; a tuple role is judged by its content.
    let failure = plan_with(
        "listrole",
        "return {'nodes': [{'role': ['main']}]}",
        &format!("return {CLEAN}"),
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
        "return {'nodes': [{'role': ('a', ['b'])}]}",
        &format!("return {CLEAN}"),
    )
    .expect_err("unhashable");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "unhashable type: 'list'"
    );
    let plan = plan_with(
        "tuplerole",
        "return {'nodes': [{'role': ('a', 'b')}]}",
        &format!("return {CLEAN}"),
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "manifest nodes[].role 로 참여 노드를 정할 수 없다([('a', 'b')] — node_id = role 슬러그 · 비었거나 중복)"
    );

    // An exception of evaluate_manifest is not caught by the original: it propagates with its class and text.
    let failure = plan_with(
        "evalraise",
        &format!("return {NODES}"),
        "raise KeyError('x')",
    )
    .expect_err("propagates");
    assert_eq!(
        (
            failure.class,
            failure.message.to_utf8().expect("text").as_str()
        ),
        ("KeyError", "'x'")
    );
    // So does a result that has no `.get`.
    let failure =
        plan_with("evalnotdict", &format!("return {NODES}"), "return 5").expect_err("propagates");
    assert_eq!(
        failure.message.to_utf8().expect("text"),
        "'int' object has no attribute 'get'"
    );
}

/// Two valid manifests the first transport refused because of metadata the classifier never reads (found by the root's
/// review; the original returns a valid multi plan for both): an unused integer key too large for a float, and an unused
/// self-referencing anchor. The document stays with the owner and only what the classifier touches is fetched.
#[test]
fn metadata_the_classifier_never_reads_cannot_change_a_valid_plan() {
    let nodes = "{'nodes': [{'role': 'main'}, {'role': 'sub'}], 'unused': LOOP}";
    let big_key = format!("return {{**{NODES}, 2 ** 2048: 'unused', 1.0: 'a', True: 'b', float('nan'): 'c', 2 ** 70: 'd', float(2 ** 70): 'e'}}");
    let looped = format!("loop = []\nloop.append(loop)\nmapping = {{'x': loop}}\nmapping['self'] = mapping\nreturn {{**{NODES}, 'unused': loop, 'also': mapping}}");
    let _ = nodes;
    for (label, load_yaml) in [("bigkey", big_key), ("cycle", looped)] {
        let plan = plan_with(
            label,
            &load_yaml,
            &format!("return {{**{CLEAN}, 'meta': 2 ** 4096}}"),
        )
        .expect(label);
        assert_eq!(field(&plan, "nodes"), "['main', 'sub']", "{label}");
        assert_eq!(field(&plan, "undeterminable"), "None", "{label}");
        assert_eq!(field(&plan, "self_node"), "main", "{label}");
    }
    // A self-referencing value that IS read still fails - as a failure of the channel, not as a plan.
    let failure = plan_with(
        "readcycle",
        "role = ['x']\nrole.append(role)\nreturn {'nodes': [{'role': (role,)}]}",
        &format!("return {CLEAN}"),
    )
    .expect_err("a tuple role that contains itself cannot be materialised");
    assert!(
        failure.class == ADAPTER_ERROR || failure.class == "TypeError",
        "{}",
        failure.line().to_utf8().expect("text")
    );
}

/// Integers cross the adapter's wire in hexadecimal, so a value far beyond the decimal digit limit is carried exactly
/// without the adapter touching the interpreter's limit (the owner runs under the inherited one).
#[test]
fn a_huge_integer_the_classifier_reads_crosses_the_wire_exactly_and_the_limit_stays_inherited() {
    let failure = plan_with(
        "hugecode",
        &format!("return {NODES}"),
        "return {'violations': [{'code': 2 ** 20000}]}",
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
    // The adapter must not widen the limit for the owner: the owner's own decimal conversion still refuses.
    let plan = plan_with(
        "ownerlimit",
        &format!("return {NODES}"),
        "import sys\nreturn {'violations': [{'code': str(sys.get_int_max_str_digits())}]}",
    )
    .expect("plan");
    assert_eq!(
        field(&plan, "undeterminable"),
        "node_role_contract 위반: 4300"
    );
}
