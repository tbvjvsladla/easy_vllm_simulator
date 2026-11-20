//! Contract checks of `yaml11_oracle` (plan 26111805 「보존 계약」, cell c21): the fixed surface its one consumer,
//! wiki-desk `checks/frontmatter.check.ts` (`execFileSync(ORACLE, [], { input })`), reads — consumers.tsv
//! `yaml11_oracle` row. The binary is started for real; the checks read values (L2) and exit codes (L1), not texts.
//!
//! Fixed here: no argument and the flat mapping on stdin; stdout = one JSON array of `[key, value]` string pairs in
//! document order; a scalar that resolves to a string is the string itself, any other resolution is `"!!<tag> <raw>"`
//! (fixed only for scalars YAML 1.1 and 1.2 resolve alike — the oracle's PyYAML 1.1 rules change with c45); exit 0 with
//! nothing on stderr; an unsupported or unreadable input is a non-zero exit with nothing on stdout (the consumer throws
//! on a non-zero exit).
use std::io::Write;
use std::process::{Command, Stdio};

use json::Json;

fn oracle(stdin: &[u8]) -> (Option<i32>, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_yaml11_oracle"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin).unwrap();
    let out = child.wait_with_output().unwrap();
    (out.status.code(), String::from_utf8(out.stdout).unwrap(), String::from_utf8(out.stderr).unwrap())
}

fn pairs(stdout: &str) -> Vec<(String, String)> {
    let doc = Json::parse(stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {stdout:?}"));
    doc.arr()
        .unwrap_or_else(|| panic!("stdout is not an array: {stdout:?}"))
        .iter()
        .map(|row| {
            let r = row.strs();
            assert_eq!(r.len(), 2, "a row is [key, value]: {row:?}");
            (r[0].clone(), r[1].clone())
        })
        .collect()
}

/// The consumer's own probe: every string that YAML 1.1 or 1.2 could read as a non-string, emitted double-quoted
/// (`emitYaml`), must come back as the same string — key and value — in emission order.
#[test]
fn quoted_ambiguous_scalars_come_back_as_the_same_strings() {
    let ambiguous = [
        "0x1F", "0o17", "017", "0b101", "1_000", "+1", "-1", "0", "3.14", "1.", ".5", "+.5", "1e3", "1E-3", ".inf", "-.Inf",
        "+.INF", ".nan", ".NaN", "1:20", "12:30:45", "y", "Y", "n", "N", "yes", "Yes", "YES", "no", "No", "NO", "on", "On",
        "ON", "off", "Off", "OFF", "true", "True", "TRUE", "tRUE", "false", "fAlse", "null", "Null", "NULL", "nULL", "~",
        "2026-10-14", "2026-10-14T09:00:00+09:00", "2026-10-14 09:00:00", "2026-10-14t09:00:00z", "=", "<<",
    ];
    let mut text = String::new();
    for s in ambiguous {
        text.push_str(&format!("\"{s}\": \"{s}\"\n"));
    }
    let (code, out, err) = oracle(text.as_bytes());
    assert_eq!(code, Some(0), "stderr: {err}");
    assert_eq!(err, "", "a pass writes nothing on stderr");
    let want: Vec<(String, String)> = ambiguous.iter().map(|s| (s.to_string(), s.to_string())).collect();
    assert_eq!(pairs(&out), want);
}

/// Plain scalars: a `str` resolution is the text itself, any other is `!!<tag> <raw>`; rows keep the document order;
/// comments, blank lines and a leading `---` are not rows. Only scalars that YAML 1.1 and the 1.2 core schema resolve
/// alike are fixed here; the 1.1-only resolutions (`yes`, leading-zero octal, timestamps, a repeated key) are not a
/// consumer surface (types.md 5절, c45).
#[test]
fn plain_scalars_show_their_resolution() {
    let text = "---\n# c\nplain: word\nflag: true\nnum: 42\nratio: 2.5\nempty:\n\ntilde: ~\nquoted: 'yes'\n";
    let (code, out, err) = oracle(text.as_bytes());
    assert_eq!(code, Some(0), "stderr: {err}");
    let got = pairs(&out);
    let want = [
        ("plain", "word"),
        ("flag", "!!bool true"),
        ("num", "!!int 42"),
        ("ratio", "!!float 2.5"),
        ("empty", "!!null "),
        ("tilde", "!!null ~"),
        ("quoted", "yes"),
    ];
    assert_eq!(got, want.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect::<Vec<_>>());
    assert_eq!(oracle(b"").1.trim_end(), "[]", "an empty mapping is an empty array");
}

/// Fail-loud: input outside the flat-mapping subset, or stdin that is not UTF-8, is a non-zero exit and no stdout.
#[test]
fn unsupported_or_unreadable_input_is_a_nonzero_exit_without_stdout() {
    for bad in [&b"k:\n  - x\n"[..], b"[a, b]\n", b"  k: v\n", b"k: v\n---\nj: w\n", b"k: \xff\n"] {
        let (code, out, err) = oracle(bad);
        assert!(matches!(code, Some(c) if c != 0), "{bad:?}: exit {code:?}");
        assert_eq!(out, "", "{bad:?}: nothing on stdout");
        assert!(!err.is_empty(), "{bad:?}: the reason is on stderr");
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
