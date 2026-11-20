//! Contract checks of `parity` (plan 26111805 「보존 계약」, cell c21): the surface the W gate of request 26111719 §3
//! (`target/release/parity compare --stage all --repo "$PWD"`, consumers.tsv `parity` row) reads — the exit code and
//! the `MISMATCH <case-id> …` lines on stdout — and the pass marker line. The binary is started for real, inside a
//! scratch directory (TMPDIR) and against throwaway repositories; no case program of the real repository runs.
//!
//! Fixed here: `compare` with `--stage`, `--case`, `--repo`, `--raw-dir`, `--na-list`; `na-list --tree --out`;
//! exit 0 = no mismatch (last stdout line `PARITY-<STAGE>-PASS python_exec=0`), 1 = mismatch (`MISMATCH <id> …`
//! lines, last line `compare: <bad>/<n> cases mismatch python_exec=<n>`), 2 = usage or input error (a message on
//! stderr, stdout empty).
//!
//! Limit: the pass marker is fixed on a run where every case is not applicable (no case program runs). A pass after
//! real case runs needs `parity`'s own `strace -f`, which cannot attach under the campaign's M fence (safe_run traces
//! the whole test with `strace -f`; a nested tracer gets `PTRACE_TRACEME: Operation not permitted`), so it is not
//! checked here; the W gate `parity compare --stage all --repo "$PWD"` runs it for real.
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use json::Json;

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
    let dir =
        std::env::temp_dir().join(format!("parity-contract-{tag}-{}-{n}", std::process::id()));
    std::fs::create_dir_all(dir.join("tmp")).unwrap();
    Scratch(dir)
}

/// `parity <args>` with its temporary sandboxes under the scratch directory.
fn parity(s: &Scratch, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_parity"))
        .args(args)
        .env("TMPDIR", s.0.join("tmp"))
        .output()
        .unwrap()
}

fn text(b: &[u8]) -> String {
    String::from_utf8(b.to_vec()).unwrap()
}

/// An empty git repository: no case entry point exists in it.
fn empty_repo(s: &Scratch) -> PathBuf {
    let r = s.0.join("repo");
    std::fs::create_dir_all(&r).unwrap();
    let st = Command::new("git")
        .arg("-C")
        .arg(&r)
        .args(["init", "-q"])
        .status()
        .unwrap();
    assert!(st.success());
    r
}

fn first_case_id(stage: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("cases")
        .join(format!("{stage}.json"));
    let doc = Json::parse(&std::fs::read_to_string(p).unwrap()).unwrap();
    doc.arr().unwrap()[0].at("id").str().unwrap().to_string()
}

/// The pass path of `compare --stage all --repo`: every case is listed as not applicable to the tree (`na-list`
/// generates the list from the empty tree), so no case runs; exit 0 and the marker is the last line (see the module
/// notes for the limit).
#[test]
fn compare_all_pass_marker_and_exit_0() {
    let s = scratch("pass");
    let repo = empty_repo(&s);
    std::fs::create_dir_all(s.0.join("tree/.claude")).unwrap();
    let list = s.0.join("na.txt");
    let tree = s.0.join("tree");
    let out = parity(
        &s,
        &[
            "na-list",
            "--tree",
            tree.to_str().unwrap(),
            "--out",
            list.to_str().unwrap(),
        ],
    );
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let out = parity(
        &s,
        &[
            "compare",
            "--stage",
            "all",
            "--repo",
            repo.to_str().unwrap(),
            "--na-list",
            list.to_str().unwrap(),
        ],
    );
    let so = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {so}\nstderr: {}",
        text(&out.stderr)
    );
    assert_eq!(
        so.lines().last(),
        Some("PARITY-ALL-PASS python_exec=0"),
        "{so}"
    );
    assert!(!so.lines().any(|l| l.starts_with("MISMATCH ")), "{so}");
}

/// The fail path: one real case run against an empty repository cannot match its golden; exit 1, `MISMATCH <id>`
/// lines on stdout and the count line last. The raw outcome goes to `--raw-dir`, not into the tool's directory.
#[test]
fn compare_mismatch_lines_and_exit_1() {
    let s = scratch("mismatch");
    let repo = empty_repo(&s);
    let raw = s.0.join("raw");
    let id = first_case_id("pg");
    let out = parity(
        &s,
        &[
            "compare",
            "--stage",
            "pg",
            "--case",
            &id,
            "--repo",
            repo.to_str().unwrap(),
            "--raw-dir",
            raw.to_str().unwrap(),
        ],
    );
    let so = text(&out.stdout);
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdout: {so}\nstderr: {}",
        text(&out.stderr)
    );
    let mismatches: Vec<&str> = so.lines().filter(|l| l.starts_with("MISMATCH ")).collect();
    assert!(!mismatches.is_empty(), "{so}");
    for l in &mismatches {
        assert!(l.starts_with(&format!("MISMATCH {id} ")), "{l}");
    }
    assert_eq!(
        so.lines().last(),
        Some("compare: 1/1 cases mismatch python_exec=0"),
        "{so}"
    );
    assert!(!so.contains("-PASS"), "{so}");
    assert!(
        raw.join(&id).join("exit_code").is_file(),
        "the raw outcome is written under --raw-dir"
    );
}

/// Usage and input errors: exit 2, a reason on stderr (its wording is free), nothing on stdout.
#[test]
fn usage_errors_exit_2_on_stderr() {
    let s = scratch("usage");
    for args in [
        &[][..],
        &["compare", "--stage", "nope"],
        &["compare", "--bogus"],
        &["compare", "--repo"],
        &["capture"],
        &["na-list", "--tree", "x"],
    ] {
        let out = parity(&s, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stdout), "", "{args:?}");
        assert!(
            !text(&out.stderr).trim().is_empty(),
            "{args:?}: the reason is on stderr"
        );
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
