//! Contract checks of `completion_gate` and `doc_naming` (plan 26111805 「보존 계약」, cell c21): the surface their
//! consumers read (consumers.md 「고정 표면」 first row and consumers.tsv) — `sync_branches.sh`·`sync_to_sub.sh`
//! (`authorize --action …`, `allowed`, `propagation_scope.*`), hint-publisher `evidence/e1.rs` (`authorize --mode
//! promotion`, `allowed`, the exit code), policy-selftest `suite.rs` (`verify`, `state`, `eligible_for_promotion`,
//! `reason_codes`, `rubric`, `certificate`, `checked_evidence`), policy-publisher (the verify exit code and stdout
//! relayed), policy-distribution (`doc_naming --self-test`).
//!
//! The programs run for real through their launchers with the consumers' override variables
//! (`COMPLETION_GATE_BIN`, `DOC_NAMING_BIN`), on copies of the committed input fixtures `checks/data/cg/fx` in a
//! scratch directory. The checks read stdout as JSON (L2: keys, types, values) and fix exit codes and reason code
//! strings (L1); message texts, key order and layout are free (L3).
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use json::Json;

fn root() -> PathBuf {
    std::fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")).unwrap()
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(tag: &str) -> Scratch {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    let p = std::env::temp_dir().join(format!("policy-gate-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for e in std::fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let d = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_tree(&e.path(), &d);
        } else {
            std::fs::copy(e.path(), &d).unwrap();
        }
    }
}

/// A scratch directory holding `fx/` (the input fixtures and their symlinks), the cwd of every gate run.
fn fixtures(tag: &str) -> Scratch {
    let s = scratch(tag);
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/cg");
    copy_tree(&data.join("fx"), &s.0.join("fx"));
    for line in std::fs::read_to_string(data.join("symlinks.tsv")).unwrap().lines() {
        if let Some((link, target)) = line.split_once('\t') {
            std::os::unix::fs::symlink(target, s.0.join(link)).unwrap();
        }
    }
    s
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

/// `bash .claude/policies/runtime/completion_gate.sh <args>` with `COMPLETION_GATE_BIN` = this build.
fn gate(cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(root().join(".claude/policies/runtime/completion_gate.sh"))
        .args(args)
        .env("COMPLETION_GATE_BIN", env!("CARGO_BIN_EXE_completion_gate"))
        .current_dir(cwd)
        .output()
        .unwrap()
}

/// The whole stdout is one JSON object whose `exit_code` equals the process exit code.
fn gate_json(out: &Output) -> Json {
    let so = text(&out.stdout);
    let doc = Json::parse(&so).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}", text(&out.stderr)));
    assert_eq!(doc.kind(), "object", "{so}");
    let rc = out.status.code().expect("exit code");
    assert_eq!(doc.at("exit_code"), &Json::Num(rc.to_string()), "exit_code field = process exit code: {so}");
    doc
}

/// Keys and JSON types of the `verify` document (completion manifest) every consumer may read.
fn assert_verify_shape(doc: &Json) {
    let types: [(&str, &[&str]); 10] = [
        ("schema_version", &["number"]),
        ("state", &["string", "null"]),
        ("eligible_for_promotion", &["bool"]),
        ("reason_codes", &["array"]),
        ("messages", &["object"]),
        ("exit_code", &["number"]),
        ("identity", &["object", "null"]),
        ("task_class", &["string", "null"]),
        ("certificate", &["object", "null"]),
        ("checked_evidence", &["object"]),
    ];
    for (k, kinds) in types {
        assert!(kinds.contains(&doc.at(k).kind()), "{k}: {:?} not in {kinds:?}", doc.at(k));
    }
    doc.at("reason_codes").strs();
}

/// Keys and JSON types of the `authorize` document (side-effect authorization).
fn assert_authorize_shape(doc: &Json) {
    let types: [(&str, &[&str]); 11] = [
        ("schema_version", &["number"]),
        ("action", &["string", "null"]),
        ("mode", &["string", "null"]),
        ("allowed", &["bool"]),
        ("authorization_state", &["string", "null"]),
        ("reason_codes", &["array"]),
        ("messages", &["object"]),
        ("exit_code", &["number"]),
        ("identity", &["object", "null"]),
        ("task_class", &["string", "null"]),
        ("propagation_scope", &["object", "null"]),
    ];
    for (k, kinds) in types {
        assert!(kinds.contains(&doc.at(k).kind()), "{k}: {:?} not in {kinds:?}", doc.at(k));
    }
    doc.at("reason_codes").strs();
}

fn opt_str(v: &Json) -> Option<&str> {
    v.str()
}

/// `verify --manifest <fx> --repo-root fx`: exit code, `state`, `eligible_for_promotion` and the exact reason codes
/// (the codes policy-selftest and the hint promotion path read are among them).
#[test]
fn verify_exit_state_and_reason_codes() {
    type Row = (&'static str, i32, Option<&'static str>, bool, &'static [&'static str]);
    let rows: [Row; 8] = [
        ("m_full_pass", 0, Some("promotion-ready"), true, &[]),
        ("m_map", 0, Some("promotion-ready"), true, &["HINT_MAP_ONLY_PROMOTION"]),
        ("m_map_nocert", 1, Some("evidence-complete"), false, &["HINT_MAP_REQUIRES_LITE_PASS_CERTIFICATE"]),
        (
            "m_lite_in_full",
            1,
            Some("runtime-ready"),
            false,
            &[
                "CERTIFICATE_GRADE_NOT_FULL",
                "PII_SCAN_COVERAGE_INCOMPLETE:bench_report",
                "PII_SCAN_COVERAGE_INCOMPLETE:certificate",
                "PII_SCAN_COVERAGE_INCOMPLETE:devlog",
                "PII_SCAN_COVERAGE_INCOMPLETE:plan",
                "PII_SCAN_COVERAGE_INCOMPLETE:simlog",
                "PII_SCAN_COVERAGE_INCOMPLETE:testlog",
            ],
        ),
        ("m_rt_smoke", 1, None, false, &["RUNTIME_FUNCTIONAL_SMOKE_NOT_PASSED"]),
        ("m_harness", 1, Some("evidence-complete"), false, &["PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS"]),
        ("none", 2, None, false, &["MANIFEST_FILE_NOT_FOUND"]),
        ("m_badjson", 2, None, false, &["MANIFEST_INVALID_JSON"]),
    ];
    let s = fixtures("verify");
    for (mf, rc, state, eligible, codes) in rows {
        let manifest = format!("fx/{mf}.mf");
        let out = gate(&s.0, &["verify", "--manifest", &manifest, "--repo-root", "fx"]);
        assert_eq!(out.status.code(), Some(rc), "{mf}: {}", text(&out.stdout));
        let doc = gate_json(&out);
        assert_verify_shape(&doc);
        assert_eq!(opt_str(doc.at("state")), state, "{mf}");
        assert_eq!(doc.at("eligible_for_promotion"), &Json::Bool(eligible), "{mf}");
        assert_eq!(doc.at("reason_codes").strs(), codes, "{mf}");
    }
    // the promotion-ready document carries the rubric, the certificate and the checked evidence it judged
    let out = gate(&s.0, &["verify", "--manifest", "fx/m_full_pass.mf", "--repo-root", "fx"]);
    let doc = gate_json(&out);
    assert_eq!(doc.at("rubric").kind(), "object");
    assert_eq!(doc.at("certificate").kind(), "object");
    assert_eq!(doc.at("certificate").at("verdict").str(), Some("PASS"));
    assert_eq!(doc.at("checked_evidence").at("plan").at("exists"), &Json::Bool(true));
}

/// `authorize --manifest --mode --action --repo-root`: `allowed`, the exit code and the reason codes; `sync_to_sub`
/// carries the approved `propagation_scope` (status, topology, node_id, ssh_host, work_dir, planes).
#[test]
fn authorize_allowed_exit_and_reason_codes() {
    type Row = (&'static str, &'static str, &'static str, i32, bool, &'static [&'static str]);
    let rows: [Row; 9] = [
        ("a_ok", "experimental", "sync_branches", 0, true, &[]),
        ("a_ok", "experimental", "hint_push", 1, false, &["EXECUTION_APPROVAL_ACTION_NOT_ALLOWED"]),
        ("a_noapproval", "experimental", "sync_branches", 1, false, &["EXECUTION_APPROVAL_ABSENT"]),
        ("a_prop_ok", "experimental", "sync_to_sub", 0, true, &[]),
        ("a_prop_badhost", "experimental", "sync_to_sub", 2, false, &["PROPAGATION_AUTHORIZATION_SSH_HOST_INVALID"]),
        ("m_full_pass", "promotion", "hint_finalize", 0, true, &[]),
        ("m_explore", "promotion", "hint_verify", 0, true, &[]),
        ("m_harness", "promotion", "sync_branches", 1, false, &["PROMOTION_GATE_NOT_ELIGIBLE", "PROMOTION_REQUIRES_FULL_BENCHMARK_CLASS"]),
        ("m_badjson", "experimental", "sync_branches", 2, false, &["MANIFEST_INVALID_JSON"]),
    ];
    let s = fixtures("authorize");
    for (mf, mode, action, rc, allowed, codes) in rows {
        let manifest = format!("fx/{mf}.mf");
        let out = gate(&s.0, &["authorize", "--manifest", &manifest, "--mode", mode, "--action", action, "--repo-root", "fx"]);
        assert_eq!(out.status.code(), Some(rc), "{mf} {mode} {action}: {}", text(&out.stdout));
        let doc = gate_json(&out);
        assert_authorize_shape(&doc);
        assert_eq!(doc.at("allowed"), &Json::Bool(allowed), "{mf} {action}");
        assert_eq!(doc.at("action").str(), Some(action));
        assert_eq!(doc.at("mode").str(), Some(mode));
        assert_eq!(doc.at("reason_codes").strs(), codes, "{mf} {action}");
        if mode == "promotion" && allowed {
            assert_eq!(doc.at("authorization_state").str(), Some("promotion-ready"));
        }
    }
    let out = gate(&s.0, &["authorize", "--manifest", "fx/a_prop_ok.mf", "--mode", "experimental", "--action", "sync_to_sub", "--repo-root", "fx"]);
    let scope = gate_json(&out).at("propagation_scope").clone();
    assert_eq!(scope.at("status").str(), Some("approved"));
    assert_eq!(scope.at("topology").str(), Some("single"));
    assert_eq!(scope.at("node_id").str(), Some("sub1"));
    assert_eq!(scope.at("ssh_host").str(), Some("u@h1"));
    assert_eq!(scope.at("work_dir").str(), Some("/srv/work"));
    assert_eq!(scope.at("planes").strs(), ["overlay"]);
    let out = gate(&s.0, &["authorize", "--manifest", "fx/a_ok.mf", "--mode", "experimental", "--action", "sync_branches", "--repo-root", "fx"]);
    assert_eq!(gate_json(&out).at("propagation_scope"), &Json::Null, "no scope outside sync_to_sub");
}

/// Usage errors are exit 2 with the gate-shaped JSON on stdout (`CLI_USAGE_ERROR`), never an empty stdout; every
/// action name the consumers pass and both modes are accepted.
#[test]
fn cli_usage_errors_and_accepted_names() {
    let s = fixtures("cli");
    let bad: [&[&str]; 5] = [
        &[],
        &["frob"],
        &["verify", "--manifest", "fx/m_harness.mf", "--bogus", "x"],
        &["authorize", "--manifest", "fx/a_ok.mf", "--mode", "experimental", "--action", "push"],
        &["authorize", "--manifest", "fx/a_ok.mf", "--mode", "bad", "--action", "sync_to_sub"],
    ];
    for args in bad {
        let out = gate(&s.0, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        let doc = gate_json(&out);
        assert_eq!(doc.at("reason_codes").strs(), ["CLI_USAGE_ERROR"], "{args:?}");
        if args.first() == Some(&"authorize") {
            assert_authorize_shape(&doc);
            assert_eq!(doc.at("allowed"), &Json::Bool(false));
        } else {
            assert_verify_shape(&doc);
        }
    }
    for action in ["sync_to_sub", "sync_branches", "hint_finalize", "hint_verify", "hint_push"] {
        for mode in ["experimental", "promotion"] {
            let out = gate(&s.0, &["authorize", "--manifest", "fx/a_ok.mf", "--mode", mode, "--action", action, "--repo-root", "fx"]);
            let doc = gate_json(&out);
            assert!(!doc.at("reason_codes").strs().contains(&"CLI_USAGE_ERROR".to_string()), "{mode} {action} rejected");
        }
    }
    let out = gate(&s.0, &["--help"]);
    assert_eq!(out.status.code(), Some(0));
}

/// The launcher: `COMPLETION_GATE_BIN` selects the executable; an absent one is exit 2 with an empty stdout (the sync
/// scripts and hint read an empty or non-JSON stdout as "gate unreadable"). The launcher is the file the ledgers and
/// documents name.
#[test]
fn launcher_override_and_absent_executable() {
    let launcher = root().join(".claude/policies/runtime/completion_gate.sh");
    assert!(launcher.is_file());
    let out = Command::new("bash").arg(&launcher).arg("verify").env("COMPLETION_GATE_BIN", "/nonexistent/completion_gate").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    let dn = root().join(".claude/policies/runtime/doc_naming.sh");
    assert!(dn.is_file());
    let out = Command::new("bash").arg(&dn).arg("--self-test").env("DOC_NAMING_BIN", "/nonexistent/doc_naming").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
}

/// `authorize` runs the pre-side-effect tripwires (`runtime_selftest.sh --tripwires-only` beside the program) when
/// `--repo-root` is the program's own repository: a failing tripwire is `TRIPWIRE_FAILED`, `allowed: false`, exit 2.
#[test]
fn authorize_tripwire_failure_is_a_refusal() {
    for (stub, rc, allowed, codes) in [
        ("exit 0\n", 0, true, &[][..]),
        ("echo '[tripwire] FAIL contract stub' >&2\nexit 1\n", 2, false, &["TRIPWIRE_FAILED"][..]),
    ] {
        let s = scratch("tripwire");
        let repo = &s.0;
        copy_tree(&Path::new(env!("CARGO_MANIFEST_DIR")).join("checks/data/cg/fx"), repo);
        copy_tree(&root().join(".claude/schemas"), &repo.join(".claude/schemas"));
        let rt = repo.join(".claude/policies/runtime");
        std::fs::create_dir_all(&rt).unwrap();
        std::fs::write(rt.join("runtime_selftest.sh"), stub).unwrap();
        let out = Command::new("bash")
            .arg("-c")
            .arg("exec -a \"$0\" \"$GATE_BIN\" \"$@\"")
            .arg(rt.join("completion_gate.sh"))
            .args(["authorize", "--manifest", "a_ok.mf", "--mode", "experimental", "--action", "sync_branches", "--repo-root"])
            .arg(repo)
            .env("GATE_BIN", env!("CARGO_BIN_EXE_completion_gate"))
            .current_dir(repo)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(rc), "{}{}", text(&out.stdout), text(&out.stderr));
        let doc = gate_json(&out);
        assert_eq!(doc.at("allowed"), &Json::Bool(allowed));
        assert_eq!(doc.at("reason_codes").strs(), codes);
    }
}

/// A text argument that is not UTF-8 is an input error, exit 2, never a crash (types.md 2절): `--action <bytes>` is
/// the gate's usage error (`CLI_USAGE_ERROR`), an unknown non-UTF-8 argument of `doc_naming` is its usage error.
#[test]
fn non_utf8_text_argument_is_exit_2() {
    use std::os::unix::ffi::OsStrExt;
    let bad = std::ffi::OsStr::from_bytes(b"sync_\xffbranches");
    let s = fixtures("nonutf8");
    let out = Command::new("bash")
        .arg(root().join(".claude/policies/runtime/completion_gate.sh"))
        .args(["authorize", "--manifest", "fx/a_ok.mf", "--mode", "experimental", "--action"])
        .arg(bad)
        .args(["--repo-root", "fx"])
        .env("COMPLETION_GATE_BIN", env!("CARGO_BIN_EXE_completion_gate"))
        .current_dir(&s.0)
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(gate_json(&out).at("reason_codes").strs(), ["CLI_USAGE_ERROR"]);
    let out = Command::new("bash")
        .arg(root().join(".claude/policies/runtime/doc_naming.sh"))
        .arg(bad)
        .env("DOC_NAMING_BIN", env!("CARGO_BIN_EXE_doc_naming"))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
}

/// policy-distribution: `doc_naming.sh --self-test` is exit 0; usage errors are exit 2 with nothing on stdout.
#[test]
fn doc_naming_self_test_and_usage() {
    let dn = |args: &[&str]| {
        Command::new("bash")
            .arg(root().join(".claude/policies/runtime/doc_naming.sh"))
            .args(args)
            .env("DOC_NAMING_BIN", env!("CARGO_BIN_EXE_doc_naming"))
            .output()
            .unwrap()
    };
    let out = dn(&["--self-test"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    for args in [&["--bogus"][..], &["extra"], &[]] {
        let out = dn(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stdout), "", "{args:?}");
    }
}

/// A small RFC 8259 reader for the contract checks: the fixed surface is the parsed value (L2), so the checks read the
/// output as JSON instead of comparing bytes. Numbers keep their source text; no dependency of the crate is used.
mod json {
    // Each crate's checks use a different part of this shared reader.
    #![allow(dead_code)]

    #[derive(Debug, Clone, PartialEq)]
    pub enum Json {
        Null,
        Bool(bool),
        Num(String),
        Str(String),
        Arr(Vec<Json>),
        Obj(Vec<(String, Json)>),
    }

    impl Json {
        pub fn parse(text: &str) -> Result<Json, String> {
            let mut r = Reader { b: text.as_bytes(), i: 0 };
            let v = r.value()?;
            r.ws();
            if r.i != r.b.len() {
                return Err(format!("text after the JSON value at byte {}", r.i));
            }
            Ok(v)
        }
        pub fn get(&self, key: &str) -> Option<&Json> {
            match self {
                Json::Obj(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
                _ => None,
            }
        }
        /// `self[key]`, failing the check when the key is absent.
        pub fn at(&self, key: &str) -> &Json {
            self.get(key).unwrap_or_else(|| panic!("key {key:?} missing in {self:?}"))
        }
        pub fn keys(&self) -> Vec<&str> {
            match self {
                Json::Obj(m) => m.iter().map(|(k, _)| k.as_str()).collect(),
                _ => Vec::new(),
            }
        }
        pub fn str(&self) -> Option<&str> {
            match self {
                Json::Str(s) => Some(s),
                _ => None,
            }
        }
        pub fn arr(&self) -> Option<&[Json]> {
            match self {
                Json::Arr(a) => Some(a),
                _ => None,
            }
        }
        /// The JSON type name: null, bool, number, string, array, object.
        pub fn kind(&self) -> &'static str {
            match self {
                Json::Null => "null",
                Json::Bool(_) => "bool",
                Json::Num(_) => "number",
                Json::Str(_) => "string",
                Json::Arr(_) => "array",
                Json::Obj(_) => "object",
            }
        }
        /// A list of strings (e.g. `reason_codes`), failing the check on any other shape.
        pub fn strs(&self) -> Vec<String> {
            let items = self.arr().unwrap_or_else(|| panic!("not an array: {self:?}"));
            items.iter().map(|v| v.str().unwrap_or_else(|| panic!("not a string: {v:?}")).to_string()).collect()
        }
    }

    struct Reader<'a> {
        b: &'a [u8],
        i: usize,
    }

    impl Reader<'_> {
        fn ws(&mut self) {
            while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
                self.i += 1;
            }
        }
        fn eat(&mut self, lit: &str) -> Result<(), String> {
            if self.b[self.i..].starts_with(lit.as_bytes()) {
                self.i += lit.len();
                Ok(())
            } else {
                Err(format!("expected {lit:?} at byte {}", self.i))
            }
        }
        fn value(&mut self) -> Result<Json, String> {
            self.ws();
            match self.b.get(self.i) {
                None => Err("unexpected end of input".into()),
                Some(b'{') => {
                    self.i += 1;
                    let mut m = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b'}') {
                        self.i += 1;
                        return Ok(Json::Obj(m));
                    }
                    loop {
                        self.ws();
                        let k = self.string()?;
                        self.ws();
                        self.eat(":")?;
                        let v = self.value()?;
                        m.push((k, v));
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b'}') => {
                                self.i += 1;
                                return Ok(Json::Obj(m));
                            }
                            _ => return Err(format!("expected ',' or '}}' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'[') => {
                    self.i += 1;
                    let mut a = Vec::new();
                    self.ws();
                    if self.b.get(self.i) == Some(&b']') {
                        self.i += 1;
                        return Ok(Json::Arr(a));
                    }
                    loop {
                        a.push(self.value()?);
                        self.ws();
                        match self.b.get(self.i) {
                            Some(b',') => self.i += 1,
                            Some(b']') => {
                                self.i += 1;
                                return Ok(Json::Arr(a));
                            }
                            _ => return Err(format!("expected ',' or ']' at byte {}", self.i)),
                        }
                    }
                }
                Some(b'"') => Ok(Json::Str(self.string()?)),
                Some(b't') => self.eat("true").map(|_| Json::Bool(true)),
                Some(b'f') => self.eat("false").map(|_| Json::Bool(false)),
                Some(b'n') => self.eat("null").map(|_| Json::Null),
                Some(c) if *c == b'-' || c.is_ascii_digit() => {
                    let start = self.i;
                    while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                        self.i += 1;
                    }
                    let t = std::str::from_utf8(&self.b[start..self.i]).unwrap().to_string();
                    t.parse::<f64>().map_err(|e| format!("bad number {t:?}: {e}"))?;
                    Ok(Json::Num(t))
                }
                Some(c) => Err(format!("unexpected byte {c:#x} at {}", self.i)),
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            let v = u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16).map_err(|e| e.to_string())?;
            self.i += 4;
            Ok(v)
        }
        fn string(&mut self) -> Result<String, String> {
            self.eat("\"")?;
            let mut out = Vec::new();
            loop {
                let c = *self.b.get(self.i).ok_or("unterminated string")?;
                self.i += 1;
                match c {
                    b'"' => return String::from_utf8(out).map_err(|e| e.to_string()),
                    b'\\' => {
                        let e = *self.b.get(self.i).ok_or("unterminated escape")?;
                        self.i += 1;
                        let ch = match e {
                            b'"' => '"',
                            b'\\' => '\\',
                            b'/' => '/',
                            b'b' => '\u{8}',
                            b'f' => '\u{c}',
                            b'n' => '\n',
                            b'r' => '\r',
                            b't' => '\t',
                            b'u' => {
                                let hi = self.hex4()?;
                                let code = if (0xD800..0xDC00).contains(&hi) {
                                    self.eat("\\u")?;
                                    let lo = self.hex4()?;
                                    0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00)
                                } else {
                                    hi
                                };
                                char::from_u32(code).ok_or("lone surrogate")?
                            }
                            _ => return Err(format!("bad escape at byte {}", self.i)),
                        };
                        let mut buf = [0u8; 4];
                        out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                    }
                    c if c < 0x20 => return Err(format!("control byte in string at {}", self.i)),
                    c => out.push(c),
                }
            }
        }
    }
}
