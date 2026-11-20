//! Contract checks of `claim_predicates`, `companion_predicate` and `runtime_patch_predicate` (plan 26111805 「보존 계약」,
//! cell c21c-contract-tests): the surface their consumers read (runtime-support/contract/consumers.tsv rows of these
//! bins) —
//! - harness_verify (stage 3) and policy-distribution run `bash .claude/policies/predicates/claim_predicates.sh` without
//!   arguments, then every `*_predicate.sh` companion launcher: exit 0 passes, non-zero fails (harness exit 3); the
//!   claim run prints its JSON verdict on stdout (`verdict` PASS|FAIL, `predicate_count`, `failed_count`, `failures[]`
//!   with `clause_id` and `error`);
//! - the ledger, the README and the documents name the launchers; `CLAIM_PREDICATES_BIN`, `COMPANION_PREDICATE_BIN`,
//!   `RUNTIME_PATCH_PREDICATE_BIN` override the executables; `--clause ID` and `--list` are the launcher's documented
//!   additions.
//!
//! The bins run for real through their launchers (argv[0] = the launcher, so the repository root is the checkout).
//! The full predicate run reaches the network-facing sync scripts and is a W gate; here the claim launcher runs two pure
//! clauses on the checkout (PASS, exit 0) and the whole suite against an empty scratch root (`POLICY_PREDICATES_REPO`,
//! FAIL, exit 1), and the companion suites run on the checkout (exit 0) and on the empty root (exit 1). Exit codes and
//! key names are L1, the verdict document L2, error texts and the unittest-style stderr summary L3. A non-UTF-8
//! argument panics `claim_predicates` today (`src/bin/claim_predicates.rs:5` `std::env::args()`); recorded as a defect,
//! not pinned.
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
        "policy-predicates-contract-{tag}-{}-{n}",
        std::process::id()
    ));
    std::fs::create_dir_all(&p).unwrap();
    Scratch(std::fs::canonicalize(&p).unwrap())
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

const CLAIM: (&str, &str, &str) = (
    ".claude/policies/predicates/claim_predicates.sh",
    "CLAIM_PREDICATES_BIN",
    env!("CARGO_BIN_EXE_claim_predicates"),
);
const COMPANION: (&str, &str, &str) = (
    ".claude/policies/predicates/companion_predicate.sh",
    "COMPANION_PREDICATE_BIN",
    env!("CARGO_BIN_EXE_companion_predicate"),
);
const PATCH: (&str, &str, &str) = (
    ".claude/policies/predicates/runtime_patch_predicate.sh",
    "RUNTIME_PATCH_PREDICATE_BIN",
    env!("CARGO_BIN_EXE_runtime_patch_predicate"),
);

/// `bash <launcher> <args>` with the override = this build, cwd = the checkout; `repo` = `POLICY_PREDICATES_REPO`
/// (None: unset, the launcher's checkout is judged).
fn launch(l: (&str, &str, &str), args: &[&str], repo: Option<&Path>) -> Output {
    let mut c = Command::new("bash");
    c.arg(root().join(l.0))
        .args(args)
        .env(l.1, l.2)
        .env_remove("POLICY_PREDICATES_REPO")
        .current_dir(root());
    if let Some(r) = repo {
        c.env("POLICY_PREDICATES_REPO", r);
    }
    c.output().unwrap()
}

/// The claim verdict: one JSON object on stdout, `failed_count` = the number of `failures`, each with a string
/// `clause_id` and `error` (and string tails when the clause ran a child).
fn verdict(out: &Output) -> Json {
    let so = text(&out.stdout);
    let doc = Json::parse(&so).unwrap_or_else(|e| {
        panic!(
            "stdout is not one JSON document ({e}):\n{so}\nstderr:\n{}",
            text(&out.stderr)
        )
    });
    assert_eq!(doc.at("schema_version").kind(), "number", "{so}");
    assert_eq!(doc.at("predicate_count").kind(), "number", "{so}");
    let failures = doc.at("failures").arr().expect("failures is an array");
    assert_eq!(
        doc.at("failed_count"),
        &Json::Num(failures.len().to_string()),
        "{so}"
    );
    for f in failures {
        assert_eq!(f.at("clause_id").kind(), "string", "{f:?}");
        assert_eq!(f.at("error").kind(), "string", "{f:?}");
        for k in ["stderr_tail", "stdout_tail"] {
            if let Some(v) = f.get(k) {
                assert_eq!(v.kind(), "string", "{k}: {f:?}");
            }
        }
    }
    let rc = out.status.code().expect("exit code");
    let pass = failures.is_empty();
    assert_eq!(
        doc.at("verdict").str(),
        Some(if pass { "PASS" } else { "FAIL" }),
        "{so}"
    );
    assert_eq!(rc, if pass { 0 } else { 1 }, "exit 0 PASS / 1 FAIL: {so}");
    doc
}

/// The three launchers are the files the documents, the ledger and the harness name; the override variable selects the
/// executable; an absent or non-ELF one is exit 2 with nothing on stdout.
#[test]
fn launchers_override_and_absent_executable() {
    let s = scratch("launchers");
    let fake = s.0.join("not-elf");
    std::fs::write(&fake, "#!/bin/bash\nexit 0\n").unwrap();
    std::fs::set_permissions(&fake, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    for (rel, var, _) in [CLAIM, COMPANION, PATCH] {
        let launcher = root().join(rel);
        assert!(launcher.is_file(), "{rel}");
        for bin in [Path::new("/nonexistent/bin"), fake.as_path()] {
            let out = Command::new("bash")
                .arg(&launcher)
                .env(var, bin)
                .output()
                .unwrap();
            assert_eq!(out.status.code(), Some(2), "{rel} {}", bin.display());
            assert_eq!(text(&out.stdout), "", "{rel} {}", bin.display());
        }
    }
}

/// A passing run: `--clause` limits the run to pure clauses of the checkout; exit 0, `verdict` PASS, no failure.
#[test]
fn claim_predicates_pass_is_exit_0_with_the_verdict() {
    let out = launch(
        CLAIM,
        &[
            "--clause",
            "KV_ABSOLUTE_CLAMP_PORTABILITY.C1",
            "--clause",
            "KV_ABSOLUTE_CLAMP_PORTABILITY.C2",
        ],
        None,
    );
    let doc = verdict(&out);
    assert_eq!(
        doc.at("verdict").str(),
        Some("PASS"),
        "{}",
        text(&out.stdout)
    );
    assert_eq!(doc.at("failed_count"), &Json::Num("0".into()));
}

/// The consumers' form (no arguments) against a root without the registry and the owners: exit 1, `verdict` FAIL, the
/// registry mapping failure (`__mapping__`) and the failing clauses listed; `--clause ID` fails that clause the same way.
#[test]
fn claim_predicates_fail_is_exit_1_with_the_failures() {
    let s = scratch("empty");
    let out = launch(CLAIM, &[], Some(&s.0));
    let doc = verdict(&out);
    let ids: Vec<&str> = doc
        .at("failures")
        .arr()
        .unwrap()
        .iter()
        .filter_map(|f| f.at("clause_id").str())
        .collect();
    assert!(ids.contains(&"__mapping__"), "{ids:?}");
    assert!(ids.len() > 1, "{ids:?}");
    let out = launch(
        CLAIM,
        &["--clause", "KV_ABSOLUTE_CLAMP_PORTABILITY.C1"],
        Some(&s.0),
    );
    let doc = verdict(&out);
    let ids: Vec<&str> = doc
        .at("failures")
        .arr()
        .unwrap()
        .iter()
        .filter_map(|f| f.at("clause_id").str())
        .collect();
    assert_eq!(ids, ["__mapping__", "KV_ABSOLUTE_CLAMP_PORTABILITY.C1"]);
}

/// `--list`: exit 0, one JSON object per clause line with `clause_id`, `function` and `status`; an unknown argument or a
/// `--clause` without its value is exit 2 with nothing on stdout.
#[test]
fn claim_predicates_list_and_usage_errors() {
    let out = launch(CLAIM, &["--list"], None);
    assert_eq!(out.status.code(), Some(0));
    let so = text(&out.stdout);
    assert!(so.lines().count() > 1);
    for line in so.lines() {
        let row = Json::parse(line).unwrap_or_else(|e| panic!("{e}: {line}"));
        for k in ["clause_id", "function", "status"] {
            assert_eq!(row.at(k).kind(), "string", "{line}");
        }
    }
    for args in [&["--bogus"][..], &["--clause"]] {
        let out = launch(CLAIM, args, None);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stdout), "", "{args:?}");
    }
}

/// harness_verify stage 3 / policy-distribution: each companion launcher, without arguments, is exit 0 on the checkout
/// and non-zero (1) when the owners it reads are absent.
#[test]
fn companion_suites_pass_on_the_checkout_and_fail_closed() {
    let s = scratch("companion");
    for l in [COMPANION, PATCH] {
        let out = launch(l, &[], None);
        assert_eq!(
            out.status.code(),
            Some(0),
            "{}\nstderr:\n{}",
            l.0,
            text(&out.stderr)
        );
        let out = launch(l, &[], Some(&s.0));
        assert_eq!(out.status.code(), Some(1), "{}", l.0);
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
