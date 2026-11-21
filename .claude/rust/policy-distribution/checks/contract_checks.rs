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
        "policy-distribution-contract-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

const LAUNCHER_REL: &str = ".claude/policies/runtime/verify_distribution.sh";

/// A scratch checkout holding a copy of the launcher and the two files the run reads before its first check
/// (`.dockerignore`, terraforming_node SKILL.md); everything else is absent, so the checks that need those files fail and
/// the verdict is FAIL.
fn checkout(tag: &str) -> Scratch {
    let s = scratch(tag);
    let l = s.0.join(LAUNCHER_REL);
    std::fs::create_dir_all(l.parent().unwrap()).unwrap();
    std::fs::copy(root().join(LAUNCHER_REL), &l).unwrap();
    std::fs::write(s.0.join(".dockerignore"), "").unwrap();
    let skill = s.0.join(".claude/skills/terraforming_node/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::fs::write(skill, "").unwrap();
    s
}

/// `bash <checkout>/.claude/policies/runtime/verify_distribution.sh <args>` with `VERIFY_DISTRIBUTION_BIN` = this build.
fn verify(s: &Scratch, args: &[&str]) -> Output {
    Command::new("bash")
        .arg(s.0.join(LAUNCHER_REL))
        .args(args)
        .env(
            "VERIFY_DISTRIBUTION_BIN",
            env!("CARGO_BIN_EXE_verify_distribution"),
        )
        .current_dir(&s.0)
        .output()
        .unwrap()
}

fn report(out: &Output) -> Json {
    let so = text(&out.stdout);
    let doc = Json::parse(&so).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}",
            text(&out.stderr)
        )
    });
    assert_eq!(doc.kind(), "object", "{so}");
    doc
}

/// The launcher is the file the documents name; `VERIFY_DISTRIBUTION_BIN` selects the executable; an absent one is
/// exit 2 with nothing on stdout; `--help` is exit 0.
#[test]
fn launcher_override_and_absent_executable() {
    let launcher = root().join(LAUNCHER_REL);
    assert!(launcher.is_file());
    let out = Command::new("bash")
        .arg(&launcher)
        .arg("--help")
        .env(
            "VERIFY_DISTRIBUTION_BIN",
            "/nonexistent/verify_distribution",
        )
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(text(&out.stdout), "");
    let s = checkout("launcher");
    let out = verify(&s, &["--help"]);
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn failing_checkout_report_shape_exit_1_and_json_out() {
    let s = checkout("fail");
    let out_file = s.0.join("report.json");
    let out = verify(&s, &["--json-out", out_file.to_str().unwrap()]);
    assert_eq!(out.status.code(), Some(1), "stderr:\n{}", text(&out.stderr));
    let doc = report(&out);
    assert_eq!(doc.at("schema_version").kind(), "number");
    assert_eq!(doc.at("repo").str(), Some(s.0.to_str().unwrap()));
    assert_eq!(doc.at("verdict").str(), Some("FAIL"));
    let checks = doc.at("checks").arr().expect("checks is an array");
    assert!(!checks.is_empty());
    for c in checks {
        assert_eq!(c.at("name").kind(), "string", "{c:?}");
        assert_eq!(c.at("ok").kind(), "bool", "{c:?}");
    }
    assert!(checks.iter().any(|c| c.at("ok") == &Json::Bool(false)));
    let names: Vec<&str> = checks.iter().filter_map(|c| c.at("name").str()).collect();
    for name in [
        "policy_registry_verify",
        "production_claim_predicates",
        "runtime_regression_selftest",
        "antipattern_scan_selftest",
    ] {
        assert!(
            names.contains(&name),
            "check {name} is part of the report: {names:?}"
        );
    }
    let written = std::fs::read_to_string(&out_file).unwrap();
    assert_eq!(
        Json::parse(&written).unwrap(),
        doc,
        "--json-out holds the same report"
    );
}

/// The sub's retirement scan (`sync_to_sub.sh` → `sub_runner retirement-scan`) is audited through the source text of
/// `sub_runner.rs`: with this checkout's `sync_to_sub.sh` and `sub_runner.rs` the check
/// `sub_retirement_consumer_audit_before_tombstone` is ok; without one of the tokens `let owner = format!(`,
/// `while lo > 0 && is_chars(cs[lo - 1])`, `if token != owner` it is not (and the run is exit 1).
#[test]
fn sub_runner_source_tokens_are_audited() {
    let sync = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";
    let runner = ".claude/skills/upstream-version-watch/rust/runtime/src/bin/sub_runner.rs";
    let audit = |s: &Scratch| -> Json {
        let out = verify(s, &[]);
        assert_eq!(out.status.code(), Some(1), "stderr:\n{}", text(&out.stderr));
        let doc = report(&out);
        let checks = doc.at("checks").arr().expect("checks is an array").to_vec();
        let found = checks
            .into_iter()
            .find(|c| c.at("name").str() == Some("sub_retirement_consumer_audit_before_tombstone"));
        found.unwrap_or_else(|| panic!("the check is part of the report: {doc:?}"))
    };
    let s = checkout("sub-runner");
    for rel in [sync, runner] {
        let dst = s.0.join(rel);
        std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
        std::fs::copy(root().join(rel), &dst).unwrap();
    }
    assert_eq!(audit(&s).at("ok"), &Json::Bool(true));
    let source = std::fs::read_to_string(root().join(runner)).unwrap();
    for token in [
        "let owner = format!(",
        "while lo > 0 && is_chars(cs[lo - 1])",
        "if token != owner",
    ] {
        assert!(source.contains(token), "{token}");
        std::fs::write(
            s.0.join(runner),
            source.replace(token, "/* contract: token removed */"),
        )
        .unwrap();
        assert_eq!(audit(&s).at("ok"), &Json::Bool(false), "without {token:?}");
    }
}

/// sync_to_sub.sh's bootstrap path (B0) delivers, verifies and lays its tombstones before it attests the sub's zero
/// remotes (`sub_bootstrap_replacement_before_tombstone`). plan_26112012 M1-① moved that read into the function
/// `assert_sub_origin_zero`, called by B0 and by each B1 target, so the order ends at B0's call; the inline read of the
/// older tree (the frozen goldens' b1f3563e) still ends it; with neither after the bootstrap delivery the check fails.
#[test]
fn sub_bootstrap_order_ends_at_the_origin_attestation() {
    let sync = ".claude/skills/upstream-version-watch/scripts/sync_to_sub.sh";
    let check = |s: &Scratch| -> Json {
        let out = verify(s, &[]);
        assert_eq!(out.status.code(), Some(1), "stderr:\n{}", text(&out.stderr));
        let doc = report(&out);
        let checks = doc.at("checks").arr().expect("checks is an array").to_vec();
        let found = checks
            .into_iter()
            .find(|c| c.at("name").str() == Some("sub_bootstrap_replacement_before_tombstone"));
        found.unwrap_or_else(|| panic!("the check is part of the report: {doc:?}"))
    };
    let s = checkout("b0-attest");
    let dst = s.0.join(sync);
    std::fs::create_dir_all(dst.parent().unwrap()).unwrap();
    let source = std::fs::read_to_string(root().join(sync)).unwrap();
    std::fs::write(&dst, &source).unwrap();
    assert_eq!(check(&s).at("ok"), &Json::Bool(true), "the checkout's sync_to_sub.sh");
    let b0_call = "\n    assert_sub_origin_zero\n";
    let b1_call = "\n    assert_sub_origin_zero \"$t\"\n";
    assert!(source.contains(b0_call) && source.contains(b1_call), "the B0 and B1 calls");
    let inline = "\n    REMOTES=\"$(sub_run 'git remote')\" || exit 7\n";
    std::fs::write(&dst, source.replacen(b0_call, inline, 1)).unwrap();
    assert_eq!(check(&s).at("ok"), &Json::Bool(true), "the inline read of the older tree");
    std::fs::write(&dst, source.replacen(b0_call, "\n", 1).replacen(b1_call, "\n", 1)).unwrap();
    let got = check(&s);
    assert_eq!(got.at("ok"), &Json::Bool(false), "no attestation after the bootstrap delivery: {got:?}");
}

#[test]
fn usage_errors_are_exit_2() {
    let s = checkout("usage");
    for args in [&["--bogus"][..], &["--json-out"]] {
        let out = verify(&s, args);
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
