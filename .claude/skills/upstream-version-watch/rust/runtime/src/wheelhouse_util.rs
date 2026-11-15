//! Support pieces of `native_wheelhouse` (the Rust port of `native_wheelhouse.py`'s `build`): an order-preserving JSON value with the
//! `json.dumps(indent=1, ensure_ascii=False)` layout of the original, URL-safe base64 digests (SHA-1/256/384/512 through `ring`, MD5 here),
//! the POSIX `os.path` pieces the original relies on, `csv`, `fnmatch`, an ustar/pax/GNU tar stream reader (for `docker export`) and a
//! streaming zip writer (zip64 local headers like `ZipFile.open(..., force_zip64=True)`, raw DEFLATE level 1 through `miniz_oxide`).

use miniz_oxide::deflate::core::{compress, create_comp_flags_from_zip_params, CompressorOxide, TDEFLFlush, TDEFLStatus};
use ring::digest;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Failure of the original: `Wh` is a `WheelhouseError` (status 2), `Crash` an unhandled exception (status 1).
#[derive(Debug)]
pub enum Err {
    Wh(String),
    Crash(String),
}
impl From<std::io::Error> for Err {
    fn from(e: std::io::Error) -> Err {
        Err::Crash(format!("OSError: {e}"))
    }
}
pub fn wh<T>(text: impl Into<String>) -> Result<T, Err> {
    Err(Err::Wh(text.into()))
}

// ───────────────────────────── JSON ─────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum J {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

impl J {
    pub fn s(t: &str) -> J {
        J::Str(t.to_string())
    }
    pub fn opt(t: &Option<String>) -> J {
        match t {
            Some(x) => J::Str(x.clone()),
            None => J::Null,
        }
    }
    pub fn strs<I: IntoIterator<Item = S>, S: AsRef<str>>(it: I) -> J {
        J::Arr(it.into_iter().map(|x| J::s(x.as_ref())).collect())
    }
    pub fn get(&self, k: &str) -> Option<&J> {
        match self {
            J::Obj(o) => o.iter().find(|(a, _)| a == k).map(|(_, v)| v),
            _ => None,
        }
    }
    pub fn str(&self) -> Option<&str> {
        match self {
            J::Str(s) => Some(s),
            _ => None,
        }
    }
    pub fn arr(&self) -> &[J] {
        match self {
            J::Arr(a) => a,
            _ => &[],
        }
    }
    /// Python truthiness.
    pub fn truthy(&self) -> bool {
        match self {
            J::Null => false,
            J::Bool(b) => *b,
            J::Int(i) => *i != 0,
            J::Float(f) => *f != 0.0,
            J::Str(s) => !s.is_empty(),
            J::Arr(a) => !a.is_empty(),
            J::Obj(o) => !o.is_empty(),
        }
    }
}

pub fn obj(items: Vec<(&str, J)>) -> J {
    J::Obj(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn dump_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn dump_into(v: &J, level: usize, out: &mut String) {
    let pad = |n: usize| " ".repeat(n);
    match v {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Int(i) => out.push_str(&i.to_string()),
        J::Float(f) => out.push_str(&format!("{f:?}")),
        J::Str(s) => dump_str(s, out),
        J::Arr(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            for (i, x) in a.iter().enumerate() {
                out.push_str(&pad(level + 1));
                dump_into(x, level + 1, out);
                if i + 1 < a.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(level));
            out.push(']');
        }
        J::Obj(o) => {
            if o.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            for (i, (k, x)) in o.iter().enumerate() {
                out.push_str(&pad(level + 1));
                dump_str(k, out);
                out.push_str(": ");
                dump_into(x, level + 1, out);
                if i + 1 < o.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str(&pad(level));
            out.push('}');
        }
    }
}

/// `json.dumps(v, ensure_ascii=False, indent=1)`.
pub fn dumps(v: &J) -> String {
    let mut s = String::new();
    dump_into(v, 0, &mut s);
    s
}

struct Parser<'a> {
    b: &'a [u8],
    i: usize,
}
impl<'a> Parser<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn lit(&mut self, t: &str, v: J) -> Result<J, ()> {
        if self.b[self.i..].starts_with(t.as_bytes()) {
            self.i += t.len();
            Ok(v)
        } else {
            Err(())
        }
    }
    fn hex4(&mut self) -> Result<u32, ()> {
        let h = self.b.get(self.i..self.i + 4).ok_or(())?;
        let t = std::str::from_utf8(h).map_err(|_| ())?;
        self.i += 4;
        u32::from_str_radix(t, 16).map_err(|_| ())
    }
    fn string(&mut self) -> Result<String, ()> {
        self.i += 1;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = *self.b.get(self.i).ok_or(())?;
            self.i += 1;
            match c {
                b'"' => return String::from_utf8(out).map_err(|_| ()),
                b'\\' => {
                    let e = *self.b.get(self.i).ok_or(())?;
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
                            let mut cp = self.hex4()?;
                            if (0xD800..0xDC00).contains(&cp) && self.b[self.i..].starts_with(b"\\u") {
                                self.i += 2;
                                let lo = self.hex4()?;
                                cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                            }
                            char::from_u32(cp).ok_or(())?
                        }
                        _ => return Err(()),
                    };
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                c => out.push(c),
            }
        }
    }
    fn val(&mut self) -> Result<J, ()> {
        self.ws();
        match *self.b.get(self.i).ok_or(())? {
            b'{' => {
                self.i += 1;
                let mut o: Vec<(String, J)> = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b'}') {
                    self.i += 1;
                    return Ok(J::Obj(o));
                }
                loop {
                    self.ws();
                    if self.b.get(self.i) != Some(&b'"') {
                        return Err(());
                    }
                    let k = self.string()?;
                    self.ws();
                    if self.b.get(self.i) != Some(&b':') {
                        return Err(());
                    }
                    self.i += 1;
                    let v = self.val()?;
                    if let Some(slot) = o.iter_mut().find(|(a, _)| *a == k) {
                        slot.1 = v;
                    } else {
                        o.push((k, v));
                    }
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b'}') => {
                            self.i += 1;
                            return Ok(J::Obj(o));
                        }
                        _ => return Err(()),
                    }
                }
            }
            b'[' => {
                self.i += 1;
                let mut a = Vec::new();
                self.ws();
                if self.b.get(self.i) == Some(&b']') {
                    self.i += 1;
                    return Ok(J::Arr(a));
                }
                loop {
                    a.push(self.val()?);
                    self.ws();
                    match self.b.get(self.i) {
                        Some(b',') => self.i += 1,
                        Some(b']') => {
                            self.i += 1;
                            return Ok(J::Arr(a));
                        }
                        _ => return Err(()),
                    }
                }
            }
            b'"' => Ok(J::Str(self.string()?)),
            b't' => self.lit("true", J::Bool(true)),
            b'f' => self.lit("false", J::Bool(false)),
            b'n' => self.lit("null", J::Null),
            _ => {
                let st = self.i;
                while self.i < self.b.len() && matches!(self.b[self.i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    self.i += 1;
                }
                let t = std::str::from_utf8(&self.b[st..self.i]).map_err(|_| ())?;
                if let Ok(i) = t.parse::<i64>() {
                    Ok(J::Int(i))
                } else {
                    t.parse::<f64>().map(J::Float).map_err(|_| ())
                }
            }
        }
    }
}

/// `json.loads`; `Err(())` on any syntax error.
pub fn loads(bytes: &[u8]) -> Result<J, ()> {
    let mut p = Parser { b: bytes, i: 0 };
    let v = p.val()?;
    p.ws();
    if p.i != bytes.len() {
        return Err(());
    }
    Ok(v)
}

// ───────────────────────────── digests ─────────────────────────────

pub fn b64url(d: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for c in d.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        if c.len() > 1 {
            out.push(T[(n >> 6) as usize & 63] as char);
        }
        if c.len() > 2 {
            out.push(T[n as usize & 63] as char);
        }
    }
    out
}

pub fn hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

struct Md5 {
    s: [u32; 4],
    buf: Vec<u8>,
    len: u64,
}
impl Md5 {
    fn new() -> Md5 {
        Md5 { s: [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476], buf: Vec::new(), len: 0 }
    }
    fn block(&mut self, b: &[u8]) {
        const K: [u32; 64] = [
            0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501, 0x698098d8, 0x8b44f7af, 0xffff5bb1,
            0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821, 0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453,
            0xd8a1e681, 0xe7d3fbc8, 0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a, 0xfffa3942,
            0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70, 0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
            0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665, 0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d,
            0x85845dd1, 0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
        ];
        const R: [u32; 64] = [
            7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4,
            11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
        ];
        let m: Vec<u32> = b.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let [mut a, mut bb, mut c, mut d] = self.s;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((bb & c) | (!bb & d), i),
                1 => ((d & bb) | (!d & c), (5 * i + 1) % 16),
                2 => (bb ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (bb | !d), (7 * i) % 16),
            };
            let t = d;
            d = c;
            c = bb;
            bb = bb.wrapping_add(a.wrapping_add(f).wrapping_add(K[i]).wrapping_add(m[g]).rotate_left(R[i]));
            a = t;
        }
        self.s[0] = self.s[0].wrapping_add(a);
        self.s[1] = self.s[1].wrapping_add(bb);
        self.s[2] = self.s[2].wrapping_add(c);
        self.s[3] = self.s[3].wrapping_add(d);
    }
    fn update(&mut self, data: &[u8]) {
        self.len += data.len() as u64;
        self.buf.extend_from_slice(data);
        let n = self.buf.len() / 64 * 64;
        let chunk: Vec<u8> = self.buf.drain(..n).collect();
        for b in chunk.chunks(64) {
            self.block(b);
        }
    }
    fn finish(mut self) -> Vec<u8> {
        let bits = self.len.wrapping_mul(8);
        let mut pad = vec![0x80u8];
        while (self.buf.len() + pad.len()) % 64 != 56 {
            pad.push(0);
        }
        pad.extend_from_slice(&bits.to_le_bytes());
        self.buf.extend_from_slice(&pad);
        let all = std::mem::take(&mut self.buf);
        for b in all.chunks(64) {
            self.block(b);
        }
        self.s.iter().flat_map(|w| w.to_le_bytes()).collect()
    }
}

pub fn md5(data: &[u8]) -> Vec<u8> {
    let mut m = Md5::new();
    m.update(data);
    m.finish()
}

enum Hasher {
    Ring(digest::Context),
    Md5(Md5),
}
impl Hasher {
    fn new(algo: &str) -> Result<Hasher, Err> {
        Ok(match algo {
            "sha256" => Hasher::Ring(digest::Context::new(&digest::SHA256)),
            "sha384" => Hasher::Ring(digest::Context::new(&digest::SHA384)),
            "sha512" => Hasher::Ring(digest::Context::new(&digest::SHA512)),
            "sha1" => Hasher::Ring(digest::Context::new(&digest::SHA1_FOR_LEGACY_USE_ONLY)),
            "md5" => Hasher::Md5(Md5::new()),
            other => return Err(Err::Crash(format!("ValueError: unsupported hash type {other}"))),
        })
    }
    fn update(&mut self, d: &[u8]) {
        match self {
            Hasher::Ring(c) => c.update(d),
            Hasher::Md5(m) => m.update(d),
        }
    }
    fn finish(self) -> Vec<u8> {
        match self {
            Hasher::Ring(c) => c.finish().as_ref().to_vec(),
            Hasher::Md5(m) => m.finish(),
        }
    }
}

pub const CHUNK: usize = 1 << 20;

/// `hash_file(path, algo)`: (URL-safe base64 digest without padding, byte count).
pub fn hash_file(path: &Path, algo: &str) -> Result<(String, u64), Err> {
    let mut h = Hasher::new(algo)?;
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; CHUNK];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((b64url(&h.finish()), n))
}

pub fn sha256_hex_file(path: &Path) -> Result<String, Err> {
    let mut h = Hasher::new("sha256")?;
    let mut f = File::open(path)?;
    let mut buf = vec![0u8; CHUNK];
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
    }
    Ok(hex(&h.finish()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    hex(digest::digest(&digest::SHA256, data).as_ref())
}

// ───────────────────────────── os.path ─────────────────────────────

pub fn normpath(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let initial = if p.starts_with("//") && !p.starts_with("///") {
        2
    } else if p.starts_with('/') {
        1
    } else {
        0
    };
    let mut comps: Vec<&str> = Vec::new();
    for c in p.split('/') {
        if c.is_empty() || c == "." {
            continue;
        }
        if c != ".." || (initial == 0 && comps.is_empty()) || comps.last() == Some(&"..") {
            comps.push(c);
        } else if !comps.is_empty() {
            comps.pop();
        }
    }
    let s = format!("{}{}", "/".repeat(initial), comps.join("/"));
    if s.is_empty() {
        ".".into()
    } else {
        s
    }
}

pub fn join(a: &str, b: &str) -> String {
    if b.starts_with('/') {
        b.to_string()
    } else if a.is_empty() || a.ends_with('/') {
        format!("{a}{b}")
    } else {
        format!("{a}/{b}")
    }
}

pub fn basename(p: &str) -> &str {
    p.rsplit('/').next().unwrap_or("")
}

pub fn dirname(p: &str) -> String {
    match p.rfind('/') {
        None => String::new(),
        Some(i) => {
            let head = &p[..=i];
            if head.chars().all(|c| c == '/') {
                head.to_string()
            } else {
                head.trim_end_matches('/').to_string()
            }
        }
    }
}

/// `os.path.relpath(path, start)` for absolute inputs.
pub fn relpath(path: &str, start: &str) -> String {
    let p = normpath(path);
    let s = normpath(start);
    let pc: Vec<&str> = p.split('/').filter(|x| !x.is_empty()).collect();
    let sc: Vec<&str> = s.split('/').filter(|x| !x.is_empty()).collect();
    let common = pc.iter().zip(sc.iter()).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = vec![".."; sc.len() - common];
    out.extend_from_slice(&pc[common..]);
    if out.is_empty() {
        ".".into()
    } else {
        out.join("/")
    }
}

/// `str.splitlines()`.
pub fn splitlines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut it = text.char_indices().peekable();
    while let Some((i, c)) = it.next() {
        if matches!(c, '\n' | '\r' | '\x0b' | '\x0c' | '\x1c' | '\x1d' | '\x1e' | '\u{85}' | '\u{2028}' | '\u{2029}') {
            out.push(&text[start..i]);
            let mut next = i + c.len_utf8();
            if c == '\r' {
                if let Some(&(j, '\n')) = it.peek() {
                    it.next();
                    next = j + 1;
                }
            }
            start = next;
        }
    }
    if start < text.len() {
        out.push(&text[start..]);
    }
    out
}

/// `fnmatch.fnmatch` on POSIX (`*` crosses `/`).
pub fn fnmatch(name: &str, pat: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pat.chars().collect();
    fn go(n: &[char], p: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|k| go(&n[k..], &p[1..])),
            Some('?') => !n.is_empty() && go(&n[1..], &p[1..]),
            Some('[') => {
                // find the closing bracket the way fnmatch.translate does
                let mut j = 1;
                if p.get(j) == Some(&'!') {
                    j += 1;
                }
                if p.get(j) == Some(&']') {
                    j += 1;
                }
                while j < p.len() && p[j] != ']' {
                    j += 1;
                }
                if j >= p.len() {
                    return !n.is_empty() && n[0] == '[' && go(&n[1..], &p[1..]);
                }
                let mut body = &p[1..j];
                let neg = body.first() == Some(&'!');
                if neg {
                    body = &body[1..];
                }
                if n.is_empty() {
                    return false;
                }
                let c = n[0];
                let mut hit = false;
                let mut k = 0;
                while k < body.len() {
                    if k + 2 < body.len() && body[k + 1] == '-' {
                        if body[k] <= c && c <= body[k + 2] {
                            hit = true;
                        }
                        k += 3;
                    } else {
                        if body[k] == c {
                            hit = true;
                        }
                        k += 1;
                    }
                }
                hit != neg && go(&n[1..], &p[j + 1..])
            }
            Some(&c) => !n.is_empty() && n[0] == c && go(&n[1..], &p[1..]),
        }
    }
    go(&n, &p)
}

// ───────────────────────────── csv ─────────────────────────────

/// `csv.reader` rows (default dialect, `newline=""` input).
pub fn csv_rows(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_q = false;
    let mut started = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_q {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_q = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => {
                in_q = true;
                started = true;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                started = true;
            }
            '\r' | '\n' => {
                if c == '\r' && chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if started || !field.is_empty() {
                    row.push(std::mem::take(&mut field));
                }
                rows.push(std::mem::take(&mut row));
                started = false;
            }
            c => {
                field.push(c);
                started = true;
            }
        }
    }
    if started || !field.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// `csv.writer(buf, lineterminator="\n").writerows(rows)` (QUOTE_MINIMAL).
pub fn csv_write(rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    for r in rows {
        let cells: Vec<String> = r
            .iter()
            .map(|f| {
                if f.contains([',', '"', '\n', '\r']) {
                    format!("\"{}\"", f.replace('"', "\"\""))
                } else {
                    f.clone()
                }
            })
            .collect();
        out.push_str(&cells.join(","));
        out.push('\n');
    }
    out
}

// ───────────────────────────── tar stream ─────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    Reg,
    Hard,
    Sym,
    Dir,
    Other,
}

#[derive(Debug, Clone)]
pub struct TarMember {
    pub name: String,
    pub kind: Kind,
    pub linkname: String,
    pub mode: u32,
    pub size: u64,
}

fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&x| x == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

fn tar_num(b: &[u8]) -> u64 {
    if b.first().map_or(false, |x| *x & 0x80 != 0) {
        let mut n = (b[0] & 0x7f) as u64;
        for x in &b[1..] {
            n = n << 8 | *x as u64;
        }
        return n;
    }
    let t = cstr(b);
    u64::from_str_radix(t.trim(), 8).unwrap_or(0)
}

fn read_full<R: Read>(r: &mut R, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let k = r.read(&mut buf[n..])?;
        if k == 0 {
            break;
        }
        n += k;
    }
    Ok(n)
}

fn skip<R: Read>(r: &mut R, n: u64) -> std::io::Result<()> {
    std::io::copy(&mut r.take(n), &mut std::io::sink())?;
    Ok(())
}

fn pax_records(data: &[u8]) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < data.len() {
        let Some(sp) = data[i..].iter().position(|&x| x == b' ') else { break };
        let Ok(len) = std::str::from_utf8(&data[i..i + sp]).unwrap_or("").parse::<usize>() else { break };
        if len == 0 || i + len > data.len() {
            break;
        }
        let rec = &data[i + sp + 1..i + len];
        let rec = rec.strip_suffix(b"\n").unwrap_or(rec);
        if let Some(eq) = rec.iter().position(|&x| x == b'=') {
            out.push((String::from_utf8_lossy(&rec[..eq]).into_owned(), String::from_utf8_lossy(&rec[eq + 1..]).into_owned()));
        }
        i += len;
    }
    out
}

/// Reads members in stream order (`tarfile.open(mode="r|")`). `want(member)` decides whether the regular file's bytes are handed to
/// `take`; every other member's data is skipped.
pub fn tar_walk<R: Read>(
    r: &mut R,
    mut each: impl FnMut(&TarMember, &mut dyn FnMut(&mut dyn Write) -> Result<(), Err>) -> Result<(), Err>,
    mut wants: impl FnMut(&TarMember) -> bool,
) -> Result<(), Err> {
    let mut pending: Vec<(String, String)> = Vec::new();
    let mut global: Vec<(String, String)> = Vec::new();
    let mut long_name: Option<String> = None;
    let mut long_link: Option<String> = None;
    let mut hdr = [0u8; 512];
    loop {
        let n = read_full(r, &mut hdr)?;
        if n < 512 || hdr.iter().all(|&x| x == 0) {
            return Ok(());
        }
        let ty = hdr[156];
        let size = tar_num(&hdr[124..136]);
        let padded = size.div_ceil(512) * 512;
        match ty {
            b'L' | b'K' | b'x' | b'g' => {
                let mut data = vec![0u8; size as usize];
                r.read_exact(&mut data)?;
                skip(r, padded - size)?;
                match ty {
                    b'L' => long_name = Some(cstr(&data)),
                    b'K' => long_link = Some(cstr(&data)),
                    b'x' => pending.extend(pax_records(&data)),
                    _ => global.extend(pax_records(&data)),
                }
                continue;
            }
            _ => {}
        }
        let mut name = cstr(&hdr[0..100]);
        let magic = &hdr[257..263];
        let prefix = cstr(&hdr[345..500]);
        let mut kind = match ty {
            b'0' | 0 | b'7' => Kind::Reg,
            b'1' => Kind::Hard,
            b'2' => Kind::Sym,
            b'5' => Kind::Dir,
            _ => Kind::Other,
        };
        if ty == 0 && name.ends_with('/') {
            kind = Kind::Dir;
        }
        if kind == Kind::Dir {
            name = name.trim_end_matches('/').to_string();
        }
        if !prefix.is_empty() && magic.starts_with(b"ustar") {
            name = format!("{prefix}/{name}");
        }
        let mut linkname = cstr(&hdr[157..257]);
        let mut size = size;
        if let Some(l) = long_name.take() {
            name = l;
        }
        if let Some(l) = long_link.take() {
            linkname = l;
        }
        for (k, v) in global.iter().chain(pending.iter()) {
            match k.as_str() {
                "path" => name = v.clone(),
                "linkpath" => linkname = v.clone(),
                "size" => size = v.parse().unwrap_or(size),
                _ => {}
            }
        }
        pending.clear();
        let mode = tar_num(&hdr[100..108]) as u32;
        let m = TarMember { name, kind: kind.clone(), linkname, mode, size };
        let has_data = matches!(kind, Kind::Reg | Kind::Other);
        let padded = if has_data { size.div_ceil(512) * 512 } else { 0 };
        if kind == Kind::Reg && wants(&m) {
            let mut consumed = false;
            {
                let rr = &mut *r;
                let mut take = |w: &mut dyn Write| -> Result<(), Err> {
                    let mut lim = rr.take(size);
                    std::io::copy(&mut lim, w)?;
                    consumed = true;
                    Ok(())
                };
                each(&m, &mut take)?;
            }
            if consumed {
                skip(r, padded - size)?;
            } else {
                skip(r, padded)?;
            }
        } else {
            let mut noop = |_w: &mut dyn Write| -> Result<(), Err> { Ok(()) };
            each(&m, &mut noop)?;
            skip(r, padded)?;
        }
    }
}

// ───────────────────────────── zip writer ─────────────────────────────

pub const ZIP_LIMIT: u64 = 0xFFFF_FFFF;

pub struct CentralEntry {
    name: String,
    crc: u32,
    csize: u64,
    usize_: u64,
    offset: u64,
    external_attr: u32,
    zip64_local: bool,
}

pub struct ZipOut {
    f: File,
    pos: u64,
    entries: Vec<CentralEntry>,
}

fn le16(v: &mut Vec<u8>, x: u16) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le32(v: &mut Vec<u8>, x: u32) {
    v.extend_from_slice(&x.to_le_bytes());
}
fn le64(v: &mut Vec<u8>, x: u64) {
    v.extend_from_slice(&x.to_le_bytes());
}

const DOS_DATE: u16 = (1 << 5) | 1; // 1980-01-01
const DOS_TIME: u16 = 0;

fn flags_for(name: &str) -> u16 {
    if name.is_ascii() {
        0
    } else {
        0x800
    }
}

impl ZipOut {
    pub fn create(path: &Path) -> std::io::Result<ZipOut> {
        Ok(ZipOut { f: File::create(path)?, pos: 0, entries: Vec::new() })
    }

    fn put(&mut self, b: &[u8]) -> std::io::Result<()> {
        self.f.write_all(b)?;
        self.pos += b.len() as u64;
        Ok(())
    }

    /// One member from a reader, compressed on the fly (`zf.open(zi, "w", force_zip64=True)`). Returns (sha256 digest bytes, uncompressed size).
    pub fn add_stream<R: Read>(&mut self, name: &str, mode: u32, src: &mut R, level: u8) -> Result<(Vec<u8>, u64), Err> {
        let offset = self.pos;
        let mut h = Vec::new();
        le32(&mut h, 0x04034b50);
        le16(&mut h, 45);
        le16(&mut h, flags_for(name));
        le16(&mut h, 8);
        le16(&mut h, DOS_TIME);
        le16(&mut h, DOS_DATE);
        le32(&mut h, 0);
        le32(&mut h, 0xFFFF_FFFF);
        le32(&mut h, 0xFFFF_FFFF);
        le16(&mut h, name.len() as u16);
        le16(&mut h, 20);
        h.extend_from_slice(name.as_bytes());
        le16(&mut h, 1);
        le16(&mut h, 16);
        le64(&mut h, 0);
        le64(&mut h, 0);
        self.put(&h)?;
        let flags = create_comp_flags_from_zip_params(level as i32, 0, 0);
        let mut comp = CompressorOxide::new(flags);
        let mut crc = crc32fast::Hasher::new();
        let mut sha = digest::Context::new(&digest::SHA256);
        let mut inb = vec![0u8; CHUNK];
        let mut outb = vec![0u8; CHUNK];
        let (mut usz, mut csz) = (0u64, 0u64);
        loop {
            let k = src.read(&mut inb)?;
            let fin = k == 0;
            crc.update(&inb[..k]);
            sha.update(&inb[..k]);
            usz += k as u64;
            let mut off = 0;
            loop {
                let (st, used, written) = compress(&mut comp, &inb[off..k], &mut outb, if fin { TDEFLFlush::Finish } else { TDEFLFlush::None });
                off += used;
                if written > 0 {
                    self.put(&outb[..written])?;
                    csz += written as u64;
                }
                match st {
                    TDEFLStatus::Done => break,
                    TDEFLStatus::Okay => {
                        if !fin && off >= k {
                            break;
                        }
                    }
                    _ => return Err(Err::Crash("zlib: deflate error".into())),
                }
            }
            if fin {
                break;
            }
        }
        let crc = crc.finalize();
        let end = self.pos;
        self.f.seek(SeekFrom::Start(offset + 14))?;
        self.f.write_all(&crc.to_le_bytes())?;
        self.f.seek(SeekFrom::Start(offset + 30 + name.len() as u64 + 4))?;
        let mut ex = Vec::new();
        le64(&mut ex, usz);
        le64(&mut ex, csz);
        self.f.write_all(&ex)?;
        self.f.seek(SeekFrom::Start(end))?;
        self.entries.push(CentralEntry {
            name: name.to_string(),
            crc,
            csize: csz,
            usize_: usz,
            offset,
            external_attr: ((0o100000 | (mode & 0o777)) & 0xFFFF) << 16,
            zip64_local: true,
        });
        Ok((sha.finish().as_ref().to_vec(), usz))
    }

    /// A small member held in memory (`zf.writestr`), ordinary (non-zip64) local header.
    pub fn add_bytes(&mut self, name: &str, mode: u32, data: &[u8], level: u8) -> Result<(), Err> {
        let offset = self.pos;
        let body = miniz_oxide::deflate::compress_to_vec(data, level);
        let crc = crc32fast::hash(data);
        let mut h = Vec::new();
        le32(&mut h, 0x04034b50);
        le16(&mut h, 20);
        le16(&mut h, flags_for(name));
        le16(&mut h, 8);
        le16(&mut h, DOS_TIME);
        le16(&mut h, DOS_DATE);
        le32(&mut h, crc);
        le32(&mut h, body.len() as u32);
        le32(&mut h, data.len() as u32);
        le16(&mut h, name.len() as u16);
        le16(&mut h, 0);
        h.extend_from_slice(name.as_bytes());
        self.put(&h)?;
        self.put(&body)?;
        self.entries.push(CentralEntry {
            name: name.to_string(),
            crc,
            csize: body.len() as u64,
            usize_: data.len() as u64,
            offset,
            external_attr: ((0o100000 | (mode & 0o777)) & 0xFFFF) << 16,
            zip64_local: false,
        });
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        let cd_start = self.pos;
        let mut cd = Vec::new();
        for e in &self.entries {
            let mut extra = Vec::new();
            let big_sizes = e.usize_ > ZIP_LIMIT || e.csize > ZIP_LIMIT;
            if big_sizes {
                le64(&mut extra, e.usize_);
                le64(&mut extra, e.csize);
            }
            if e.offset > ZIP_LIMIT {
                le64(&mut extra, e.offset);
            }
            let zip64_central = !extra.is_empty();
            let ver: u16 = if e.zip64_local || zip64_central { 45 } else { 20 };
            le32(&mut cd, 0x02014b50);
            le16(&mut cd, ver | (3 << 8));
            le16(&mut cd, ver);
            le16(&mut cd, flags_for(&e.name));
            le16(&mut cd, 8);
            le16(&mut cd, DOS_TIME);
            le16(&mut cd, DOS_DATE);
            le32(&mut cd, e.crc);
            le32(&mut cd, if big_sizes { 0xFFFF_FFFF } else { e.csize as u32 });
            le32(&mut cd, if big_sizes { 0xFFFF_FFFF } else { e.usize_ as u32 });
            le16(&mut cd, e.name.len() as u16);
            le16(&mut cd, if zip64_central { 4 + extra.len() as u16 } else { 0 });
            le16(&mut cd, 0);
            le16(&mut cd, 0);
            le16(&mut cd, 0);
            le32(&mut cd, e.external_attr);
            le32(&mut cd, if e.offset > ZIP_LIMIT { 0xFFFF_FFFF } else { e.offset as u32 });
            cd.extend_from_slice(e.name.as_bytes());
            if zip64_central {
                le16(&mut cd, 1);
                le16(&mut cd, extra.len() as u16);
                cd.extend_from_slice(&extra);
            }
        }
        let n = self.entries.len() as u64;
        self.put(&cd)?;
        let cd_size = cd.len() as u64;
        let mut end = Vec::new();
        if n > 0xFFFF || cd_size > ZIP_LIMIT || cd_start > ZIP_LIMIT {
            let rec_pos = self.pos;
            le32(&mut end, 0x06064b50);
            le64(&mut end, 44);
            le16(&mut end, 45);
            le16(&mut end, 45);
            le32(&mut end, 0);
            le32(&mut end, 0);
            le64(&mut end, n);
            le64(&mut end, n);
            le64(&mut end, cd_size);
            le64(&mut end, cd_start);
            le32(&mut end, 0x07064b50);
            le32(&mut end, 0);
            le64(&mut end, rec_pos);
            le32(&mut end, 1);
        }
        le32(&mut end, 0x06054b50);
        le16(&mut end, 0);
        le16(&mut end, 0);
        le16(&mut end, n.min(0xFFFF) as u16);
        le16(&mut end, n.min(0xFFFF) as u16);
        le32(&mut end, cd_size.min(ZIP_LIMIT) as u32);
        le32(&mut end, cd_start.min(ZIP_LIMIT) as u32);
        le16(&mut end, 0);
        self.put(&end)?;
        self.f.flush()
    }
}
