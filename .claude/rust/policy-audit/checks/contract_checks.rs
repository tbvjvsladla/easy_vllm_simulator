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
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let p = std::env::temp_dir().join(format!(
        "policy-audit-contract-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

const REGISTRY_REL: &str = ".claude/policies/runtime/policy_registry.sh";
const HARNESS_REL: &str = ".claude/policies/runtime/harness_verify.sh";

/// `bash .claude/policies/runtime/policy_registry.sh <args>` with `POLICY_REGISTRY_BIN` = this build.
fn registry(cwd: &Path, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(root().join(REGISTRY_REL))
        .args(args)
        .env("POLICY_REGISTRY_BIN", env!("CARGO_BIN_EXE_policy_registry"))
        .current_dir(cwd)
        .output()
        .unwrap()
}

/// The registry envelope: one JSON object `{exit_code, schema_version, violation_count, violations[]}` whose
/// `exit_code` equals the process exit code and whose violations carry a string `reason_code`.
fn envelope(out: &Output) -> Json {
    let so = text(&out.stdout);
    let doc = Json::parse(&so).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}",
            text(&out.stderr)
        )
    });
    let rc = out.status.code().expect("exit code");
    assert_eq!(doc.at("exit_code"), &Json::Num(rc.to_string()), "{so}");
    assert_eq!(doc.at("schema_version").kind(), "number", "{so}");
    let v = doc.at("violations").arr().expect("violations is an array");
    assert_eq!(
        doc.at("violation_count"),
        &Json::Num(v.len().to_string()),
        "{so}"
    );
    for x in v {
        assert_eq!(x.at("reason_code").kind(), "string", "{x:?}");
        assert_eq!(x.at("message").kind(), "string", "{x:?}");
        for k in ["policy_id", "clause_id", "path"] {
            assert!(["string", "null"].contains(&x.at(k).kind()), "{k}: {x:?}");
        }
    }
    doc
}

fn codes(doc: &Json) -> Vec<String> {
    doc.at("violations")
        .arr()
        .unwrap()
        .iter()
        .map(|v| v.at("reason_code").str().unwrap().to_string())
        .collect()
}

/// Both launchers are the files the documents, the ledger and sync_branches.sh name; `POLICY_REGISTRY_BIN` and
/// `HARNESS_VERIFY_BIN` select the executables; an absent one is exit 2 with nothing on stdout; `--help` is exit 0.
#[test]
fn launchers_override_and_absent_executable() {
    for (rel, var, bin) in [
        (
            REGISTRY_REL,
            "POLICY_REGISTRY_BIN",
            env!("CARGO_BIN_EXE_policy_registry"),
        ),
        (
            HARNESS_REL,
            "HARNESS_VERIFY_BIN",
            env!("CARGO_BIN_EXE_harness_verify"),
        ),
    ] {
        let launcher = root().join(rel);
        assert!(launcher.is_file(), "{rel}");
        let out = Command::new("bash")
            .arg(&launcher)
            .arg("--help")
            .env(var, "/nonexistent/bin")
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(2), "{rel}");
        assert_eq!(text(&out.stdout), "", "{rel}");
        let out = Command::new("bash")
            .arg(&launcher)
            .arg("--help")
            .env(var, bin)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(0), "{rel}");
    }
}

/// policy-selftest `suite.rs`: `bash policy_registry.sh --self-test` is exit 0. The self-test reports no count of its
/// checks (one stdout line); the lower bound here is that line, `[policy_registry] --self-test PASS` (its PASS marker,
/// review r2 P-T5), so an exit 0 without the report fails. The wording around the marker is L3.
#[test]
fn registry_self_test_passes() {
    let s = scratch("selftest");
    let out = registry(&s.0, &["--self-test"]);
    let so = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout:\n{so}\nstderr:\n{}",
        text(&out.stderr)
    );
    let last = so.lines().last().unwrap_or_default();
    assert!(
        last.starts_with("[policy_registry]") && last.split_whitespace().any(|w| w == "PASS"),
        "the self-test's report line: {so:?}"
    );
}

#[test]
fn registry_verify_envelope_and_exit_code() {
    let r = root();
    let rs = r.to_str().unwrap();
    let out = registry(&r, &["verify", "--repo-root", rs]);
    let doc = envelope(&out);
    let rc = out.status.code().unwrap();
    assert_eq!(rc, 0, "the real tree has no violation: {doc:?}");
    assert_eq!(doc.at("violation_count"), &Json::Num("0".into()));
    assert_eq!(doc.at("violations").arr().map(<[Json]>::len), Some(0));
    let reg = r.join(".claude/policies/registry.yaml");
    let schema = r.join(".claude/schemas/policy-registry.schema.json");
    let cb = r.join(".claude/policies/claim_bindings.json");
    let verify_with_inputs = |repo_root: &str| {
        registry(
            &r,
            &[
                "verify",
                "--registry",
                reg.to_str().unwrap(),
                "--schema",
                schema.to_str().unwrap(),
                "--claim-bindings",
                cb.to_str().unwrap(),
                "--repo-root",
                repo_root,
            ],
        )
    };
    let explicit = verify_with_inputs(rs);
    assert_eq!(explicit.status.code(), out.status.code());
    assert_eq!(envelope(&explicit), doc);
    let elsewhere = scratch("elsewhere");
    let away = verify_with_inputs(elsewhere.0.to_str().unwrap());
    let away_doc = envelope(&away);
    assert_eq!(away.status.code(), Some(1), "{away_doc:?}");
    let rows = away_doc.at("violations").arr().unwrap();
    assert!(!rows.is_empty());
    assert_eq!(
        away_doc.at("violation_count"),
        &Json::Num(rows.len().to_string())
    );
    for row in rows {
        assert_eq!(row.at("reason_code").kind(), "string", "{row:?}");
        assert_eq!(row.at("policy_id").kind(), "string", "{row:?}");
    }
}

#[test]
fn registry_check_removal_of_an_active_policy_is_exit_1() {
    let r = root();
    let out = registry(
        &r,
        &[
            "check-removal",
            "--policy-id",
            "GIT_SINGLE_AUTHORITY",
            "--repo-root",
            r.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(1));
    let doc = envelope(&out);
    assert_eq!(codes(&doc), ["REMOVAL_NOT_ELIGIBLE"]);
    assert_eq!(
        doc.at("violations").arr().unwrap()[0].at("policy_id").str(),
        Some("GIT_SINGLE_AUTHORITY")
    );
}

#[test]
fn registry_usage_and_input_errors_are_exit_2() {
    let s = scratch("usage");
    write(&s.0.join("bad.json"), "{\"policies\": ");
    write(&s.0.join("array.json"), "[]");
    let rows: [(&[&str], &str); 6] = [
        (&[], "CLI_USAGE_ERROR"),
        (&["bogus"], "CLI_USAGE_ERROR"),
        (&["check-removal", "--repo-root", "."], "CLI_USAGE_ERROR"),
        (
            &["verify", "--registry", "missing.json"],
            "REGISTRY_LOAD_ERROR",
        ),
        (
            &["verify", "--registry", "bad.json"],
            "REGISTRY_INVALID_JSON",
        ),
        (
            &["verify", "--registry", "array.json"],
            "REGISTRY_ROOT_NOT_OBJECT",
        ),
    ];
    for (args, code) in rows {
        let out = registry(&s.0, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(codes(&envelope(&out)), [code], "{args:?}");
    }
}

/// A scratch tree holding a copy of the harness launcher and stub stage launchers that log `<name> <argv>` to `$HV_LOG`
/// and exit `$RC_<NAME>` (default 0).
fn harness_tree(tag: &str) -> Scratch {
    let s = scratch(tag);
    let l = s.0.join(HARNESS_REL);
    std::fs::create_dir_all(l.parent().unwrap()).unwrap();
    std::fs::copy(root().join(HARNESS_REL), &l).unwrap();
    for (rel, name) in [
        (".claude/policies/runtime/policy_registry.sh", "registry"),
        (".claude/policies/runtime/runtime_selftest.sh", "runtime"),
        (".claude/policies/predicates/claim_predicates.sh", "claim"),
        (
            ".claude/policies/predicates/companion_predicate.sh",
            "companion",
        ),
        (
            ".claude/policies/predicates/runtime_patch_predicate.sh",
            "patch",
        ),
    ] {
        write(
            &s.0.join(rel),
            &format!(
                "#!/bin/bash\necho \"{name} $* @$PWD\" >>\"$HV_LOG\"\nexit \"${{RC_{}:-0}}\"\n",
                name.to_uppercase()
            ),
        );
    }
    s
}

/// `bash <tree>/.claude/policies/runtime/harness_verify.sh <args>` with `HARNESS_VERIFY_BIN` = this build; returns the
/// output and the stage log.
fn harness(t: &Scratch, args: &[&str], env: &[(&str, &str)]) -> (Output, String) {
    let log = t.0.join("stages.log");
    let _ = std::fs::remove_file(&log);
    let mut c = Command::new("bash");
    c.arg(t.0.join(HARNESS_REL))
        .args(args)
        .env("HARNESS_VERIFY_BIN", env!("CARGO_BIN_EXE_harness_verify"))
        .env("HV_LOG", &log);
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.current_dir(&t.0).output().unwrap();
    // No log = no stage ran; any other read failure is a failure of the check itself.
    let stages = match std::fs::read_to_string(&log) {
        Ok(stages) => stages,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => panic!("{}: {e}", log.display()),
    };
    (out, stages)
}

#[test]
fn harness_stage_argv_and_order() {
    let t = harness_tree("green");
    let (out, log) = harness(
        &t,
        &[
            "--registry",
            "r.json",
            "--schema",
            "s.json",
            "--claim-bindings",
            "c.json",
            "--repo-root",
            "x",
        ],
        &[],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let at = format!("@{}", t.0.display());
    assert_eq!(
        log,
        format!(
            "registry verify --registry r.json --schema s.json --claim-bindings c.json --repo-root x {at}\n\
             runtime  {at}\nclaim  {at}\ncompanion  {at}\npatch  {at}\n"
        )
    );
    let (out, log) = harness(&t, &[], &[]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        log.lines().next(),
        Some(format!("registry verify {at}").as_str())
    );
}

/// Exit codes the callers act on: stage 1's own code (1, 2), the runtime probe → 4, the predicate suite (claim or a
/// companion) → 3; later stages do not run after a failing one; a missing stage launcher fails its stage closed.
#[test]
fn harness_stage_failures_map_exit_codes() {
    let t = harness_tree("fail");
    let rows: [(&str, &str, i32, usize); 6] = [
        ("RC_REGISTRY", "1", 1, 1),
        ("RC_REGISTRY", "2", 2, 1),
        ("RC_RUNTIME", "5", 4, 2),
        ("RC_CLAIM", "1", 3, 3),
        ("RC_COMPANION", "1", 3, 5),
        ("RC_PATCH", "7", 3, 5),
    ];
    for (var, rc, want, stages) in rows {
        let (out, log) = harness(&t, &[], &[(var, rc)]);
        assert_eq!(out.status.code(), Some(want), "{var}={rc}");
        assert_eq!(log.lines().count(), stages, "{var}={rc}: {log}");
    }
    std::fs::remove_file(t.0.join(".claude/policies/predicates/runtime_patch_predicate.sh"))
        .unwrap();
    let (out, _) = harness(&t, &[], &[]);
    assert_eq!(out.status.code(), Some(3));
    std::fs::remove_file(t.0.join(".claude/policies/runtime/policy_registry.sh")).unwrap();
    let (out, log) = harness(&t, &[], &[]);
    assert_eq!((out.status.code(), log.as_str()), (Some(2), ""));
}

#[test]
fn harness_usage_errors_are_exit_2() {
    let t = harness_tree("usage");
    let (out, log) = harness(&t, &["--bogus"], &[]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(log, "");
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
            let mut r = Reader {
                b: text.as_bytes(),
                i: 0,
            };
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
            self.get(key)
                .unwrap_or_else(|| panic!("key {key:?} missing in {self:?}"))
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
            let items = self
                .arr()
                .unwrap_or_else(|| panic!("not an array: {self:?}"));
            items
                .iter()
                .map(|v| {
                    v.str()
                        .unwrap_or_else(|| panic!("not a string: {v:?}"))
                        .to_string()
                })
                .collect()
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
                    while self.i < self.b.len()
                        && matches!(
                            self.b[self.i],
                            b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'
                        )
                    {
                        self.i += 1;
                    }
                    let t = std::str::from_utf8(&self.b[start..self.i])
                        .unwrap()
                        .to_string();
                    t.parse::<f64>()
                        .map_err(|e| format!("bad number {t:?}: {e}"))?;
                    Ok(Json::Num(t))
                }
                Some(c) => Err(format!("unexpected byte {c:#x} at {}", self.i)),
            }
        }
        fn hex4(&mut self) -> Result<u32, String> {
            let h = self.b.get(self.i..self.i + 4).ok_or("short \\u escape")?;
            let v = u32::from_str_radix(std::str::from_utf8(h).map_err(|e| e.to_string())?, 16)
                .map_err(|e| e.to_string())?;
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
