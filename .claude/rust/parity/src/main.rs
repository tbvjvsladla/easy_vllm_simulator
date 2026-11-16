//! parity — golden compare for the Rust repack (std only, no external crates).
//! Modes: compare [--stage s1..s4|all] [--case ID]... [--repo W] [--raw-dir DIR] [--na-list FILE], na-list --tree T --out F, --self-check.
//! The comparison basis is the committed goldens in `data/` only: this tool never runs an original Python program. The former
//! `capture`/`orig` modes (which re-ran the originals through `cases/shims/*.py`) are retired and refused with status 2.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

const KNOWN_IDS: [&str; 10] = ["N01", "N02", "N03", "N04", "N06", "N08", "N09", "N11", "N13", "N14"];
// s5: terraforming_node programs. ta/tb/tc/td: terraforming A2A control plane (agent_control+providers / render_sub_env / canary, probe, gates + core contracts campaign_init·validator·manifest_contract (TC-CI/CV/MC) / relay). pg/pe/pa/ps/pd/pf: policies bundle (completion gate, doc naming, evidence publisher / registry audit / runtime selftest / distribution / claim predicates).
const STAGES: [&str; 15] = ["s1", "s2", "s3", "s4", "s5", "pg", "pe", "pa", "ps", "pd", "pf", "ta", "tb", "tc", "td"];
/// hint-publisher port stages (`cases/hp_<module>.json`): selectable with `--stage`, not part of `all` (the Sub N/A lists
/// cover `all`; a plane without hint-publisher is not affected).
const HP_STAGES: [&str; 21] = ["hp_core", "hp_naming", "hp_pii", "hp_lineage", "hp_catalog", "hp_branch", "hp_tag", "hp_template", "hp_template_t1", "hp_template_t2", "hp_artifacts", "hp_artifacts_a1", "hp_artifacts_a2", "hp_artifacts_a3", "hp_evidence", "hp_evidence_e1", "hp_evidence_e2", "hp_evidence_e3", "hp_evidence_e4", "hp_rbs", "hp_cli"];

// ---------------------------------------------------------------- JSON
#[derive(Debug, Clone, PartialEq)]
enum J { Null, Bool(bool), Num(f64), Str(String), Arr(Vec<J>), Obj(Vec<(String, J)>) }

impl J {
    fn get(&self, k: &str) -> Option<&J> {
        if let J::Obj(v) = self { v.iter().find(|(a, _)| a == k).map(|(_, b)| b) } else { None }
    }
    fn str(&self) -> Option<&str> { if let J::Str(s) = self { Some(s) } else { None } }
    fn arr(&self) -> &[J] { if let J::Arr(a) = self { a } else { &[] } }
}

struct P<'a> { s: &'a [u8], i: usize }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.i < self.s.len() && (self.s[self.i] as char).is_ascii_whitespace() { self.i += 1; } }
    fn val(&mut self) -> Result<J, String> {
        self.ws();
        match self.s.get(self.i).copied() {
            None => Err("unexpected end".into()),
            Some(b'{') => {
                self.i += 1; let mut v = vec![];
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b'}') { self.i += 1; break; }
                    let k = match self.val()? { J::Str(s) => s, _ => return Err("object key".into()) };
                    self.ws();
                    if self.s.get(self.i) != Some(&b':') { return Err("expected ':'".into()); }
                    self.i += 1;
                    let x = self.val()?; v.push((k, x)); self.ws();
                    match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b'}') => { self.i += 1; break; } _ => return Err("expected ',' or '}'".into()) }
                }
                Ok(J::Obj(v))
            }
            Some(b'[') => {
                self.i += 1; let mut v = vec![];
                loop {
                    self.ws();
                    if self.s.get(self.i) == Some(&b']') { self.i += 1; break; }
                    v.push(self.val()?); self.ws();
                    match self.s.get(self.i) { Some(b',') => self.i += 1, Some(b']') => { self.i += 1; break; } _ => return Err("expected ',' or ']'".into()) }
                }
                Ok(J::Arr(v))
            }
            Some(b'"') => {
                self.i += 1; let mut out: Vec<u8> = vec![];
                loop {
                    let c = *self.s.get(self.i).ok_or("unterminated string")?; self.i += 1;
                    match c {
                        b'"' => break,
                        b'\\' => {
                            let e = *self.s.get(self.i).ok_or("bad escape")?; self.i += 1;
                            match e {
                                b'n' => out.push(b'\n'), b't' => out.push(b'\t'), b'r' => out.push(b'\r'),
                                b'b' => out.push(8), b'f' => out.push(12),
                                b'"' | b'\\' | b'/' => out.push(e),
                                b'u' => {
                                    let mut cp = self.hex4()?;
                                    if (0xD800..0xDC00).contains(&cp) && self.s.get(self.i) == Some(&b'\\') && self.s.get(self.i + 1) == Some(&b'u') {
                                        self.i += 2; let lo = self.hex4()?;
                                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo.wrapping_sub(0xDC00) & 0x3FF);
                                    }
                                    let ch = char::from_u32(cp).unwrap_or('\u{FFFD}');
                                    let mut b = [0u8; 4]; out.extend_from_slice(ch.encode_utf8(&mut b).as_bytes());
                                }
                                _ => return Err("bad escape".into()),
                            }
                        }
                        _ => out.push(c),
                    }
                }
                Ok(J::Str(String::from_utf8_lossy(&out).into_owned()))
            }
            Some(b't') if self.s[self.i..].starts_with(b"true") => { self.i += 4; Ok(J::Bool(true)) }
            Some(b'f') if self.s[self.i..].starts_with(b"false") => { self.i += 5; Ok(J::Bool(false)) }
            Some(b'n') if self.s[self.i..].starts_with(b"null") => { self.i += 4; Ok(J::Null) }
            Some(_) => {
                let st = self.i;
                while self.i < self.s.len() && matches!(self.s[self.i], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E') { self.i += 1; }
                std::str::from_utf8(&self.s[st..self.i]).ok().and_then(|t| t.parse::<f64>().ok()).map(J::Num).ok_or_else(|| "bad value".to_string())
            }
        }
    }
    fn hex4(&mut self) -> Result<u32, String> {
        let h = self.s.get(self.i..self.i + 4).ok_or("bad \\u")?;
        self.i += 4;
        u32::from_str_radix(std::str::from_utf8(h).map_err(|_| "bad \\u")?, 16).map_err(|_| "bad \\u".to_string())
    }
}
fn parse_json(t: &str) -> Result<J, String> {
    let mut p = P { s: t.as_bytes(), i: 0 };
    let v = p.val()?; p.ws();
    if p.i != p.s.len() { return Err("trailing data".into()); }
    Ok(v)
}
// ---------------------------------------------------------------- normalization (closed list)
#[derive(Clone, Copy, PartialEq)]
enum Ch { Stdout, Stderr, File }

struct Norm { ids: Vec<String>, n13_entries: Vec<String> }
impl Norm {
    fn load(path: &Path) -> Result<Norm, String> {
        let t = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let j = parse_json(&t).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut ids = vec![]; let mut n13_entries = vec![];
        for n in j.get("normalizations").map(|a| a.arr()).unwrap_or(&[]) {
            let id = n.get("id").and_then(|x| x.str()).ok_or("normalization without id")?;
            if !KNOWN_IDS.contains(&id) { return Err(format!("normalization {id} is not implemented (closed list)")); }
            if id == "N13" { n13_entries = n.get("applies_to").map(|a| a.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default(); }
            ids.push(id.to_string());
        }
        Ok(Norm { ids, n13_entries })
    }
    fn on(&self, id: &str) -> bool { self.ids.iter().any(|x| x == id) }
    /// N13 applies to stdout and .json files of the entries named in `applies_to` (matched on the program's file stem).
    fn n13_for(&self, prog: &str) -> bool {
        let stem = prog.rsplit('/').next().unwrap_or("").split('.').next().unwrap_or("");
        self.on("N13") && self.n13_entries.iter().any(|e| e == stem)
    }
    fn text(&self, ch: Ch, raw: &[u8], repo: &str, tmp: &str) -> String {
        let mut s = String::from_utf8_lossy(raw).into_owned();
        if self.on("N01") && !repo.is_empty() { s = s.replace(repo, "<REPO>"); }
        if self.on("N02") && !tmp.is_empty() { s = s.replace(tmp, "<TMP>"); }
        if self.on("N03") { s = scrub_ts(&s); }
        if self.on("N04") && ch == Ch::Stderr { s = scrub_pid(&s); }
        if self.on("N11") { s = scrub_mark(&s); }
        if self.on("N06") && ch == Ch::Stderr { s = reduce_traceback(&s); }
        s
    }
}

fn scrub_ts(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let d = |i: usize| c.get(i).map_or(false, |x| x.is_ascii_digit());
    let is = |i: usize, ch: char| c.get(i) == Some(&ch);
    let mut o = String::new(); let mut i = 0;
    while i < c.len() {
        // ISO-8601: YYYY-MM-DDTHH:MM:SS[.frac][Z|+00:00]
        if d(i) && (0..4).all(|k| d(i + k)) && is(i + 4, '-') && d(i + 5) && d(i + 6) && is(i + 7, '-') && d(i + 8) && d(i + 9)
            && (is(i + 10, 'T') || is(i + 10, ' ')) && d(i + 11) && d(i + 12) && is(i + 13, ':') && d(i + 14) && d(i + 15) && is(i + 16, ':') && d(i + 17) && d(i + 18) {
            let mut j = i + 19;
            if is(j, '.') && d(j + 1) { j += 1; while d(j) { j += 1; } }
            if is(j, 'Z') { j += 1; } else if is(j, '+') && is(j + 3, ':') && d(j + 1) && d(j + 2) && d(j + 4) && d(j + 5) { j += 6; }
            o.push_str("<TS>"); i = j; continue;
        }
        if d(i) && (i == 0 || !d(i - 1)) && (0..10).all(|k| d(i + k)) && !d(i + 10) { o.push_str("<TS>"); i += 10; continue; }
        o.push(c[i]); i += 1;
    }
    o
}
/// N11: `<yaml.error.Mark object at 0x[0-9a-f]+>` -> `<MARK@>`; only that exact literal, nothing around it changes.
fn scrub_mark(s: &str) -> String {
    const HEAD: &str = "<yaml.error.Mark object at 0x";
    let mut o = String::new(); let mut rest = s;
    while let Some(i) = rest.find(HEAD) {
        let after = &rest[i + HEAD.len()..];
        let n = after.bytes().take_while(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)).count();
        if n > 0 && after.as_bytes().get(n) == Some(&b'>') {
            o.push_str(&rest[..i]); o.push_str("<MARK@>"); rest = &after[n + 1..];
        } else {
            o.push_str(&rest[..i + 1]); rest = &rest[i + 1..];
        }
    }
    o.push_str(rest); o
}
fn scrub_pid(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut o = String::new(); let mut i = 0;
    while i < c.len() {
        if c[i..].starts_with(&['p', 'i', 'd', '=']) && c.get(i + 4).map_or(false, |x| x.is_ascii_digit()) {
            let mut j = i + 4; while c.get(j).map_or(false, |x| x.is_ascii_digit()) { j += 1; }
            o.push_str("pid=<PID>"); i = j; continue;
        }
        o.push(c[i]); i += 1;
    }
    o
}
fn reduce_traceback(s: &str) -> String {
    if !s.contains("Traceback (most recent call last):") { return s.to_string(); }
    let last = s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("");
    format!("{last}\n")
}

/// N13: sort the members of every JSON object by key, keeping value text, number lexemes and the layout (the separators
/// between members stay at their slots). Input that is not one JSON document is returned unchanged.
fn sort_keys(t: &str) -> String {
    let b = t.as_bytes();
    let ws = |mut i: usize| { while i < b.len() && (b[i] as char).is_ascii_whitespace() { i += 1; } i };
    fn val(b: &[u8], i: usize) -> Option<(String, usize)> {
        let ws = |mut i: usize| { while i < b.len() && (b[i] as char).is_ascii_whitespace() { i += 1; } i };
        let i = ws(i);
        match *b.get(i)? {
            b'{' => {
                let mut j = ws(i + 1);
                let lead = String::from_utf8_lossy(&b[i + 1..j]).into_owned();
                if b.get(j) == Some(&b'}') { return Some((format!("{{{lead}}}"), j + 1)); }
                let mut members: Vec<(String, String)> = vec![]; let mut seps: Vec<String> = vec![];
                let trail;
                loop {
                    let (k, e) = string(b, j)?; let c = ws(e);
                    if b.get(c) != Some(&b':') { return None; }
                    let vs = c + 1; let (v, ve) = val(b, vs)?;
                    let kv = format!("{}{}{}", String::from_utf8_lossy(&b[j..e]), String::from_utf8_lossy(&b[e..ws(vs)]), v);
                    members.push((k, kv));
                    let n = ws(ve);
                    match b.get(n) {
                        Some(&b',') => { let nx = ws(n + 1); seps.push(String::from_utf8_lossy(&b[ve..nx]).into_owned()); j = nx; }
                        Some(&b'}') => { trail = String::from_utf8_lossy(&b[ve..n]).into_owned(); j = n + 1; break; }
                        _ => return None,
                    }
                }
                members.sort_by(|x, y| x.0.cmp(&y.0));
                let mut o = format!("{{{lead}");
                for (idx, (_, kv)) in members.iter().enumerate() { o += kv; if idx < seps.len() { o += &seps[idx]; } }
                o += &trail; o.push('}');
                Some((o, j))
            }
            b'[' => {
                let mut j = ws(i + 1);
                let mut o = String::from_utf8_lossy(&b[i..j]).into_owned();
                if b.get(j) == Some(&b']') { o.push(']'); return Some((o, j + 1)); }
                loop {
                    let (v, ve) = val(b, j)?; o += &String::from_utf8_lossy(&b[j..ws(j)]); o += &v;
                    let n = ws(ve);
                    match b.get(n) {
                        Some(&b',') => { let nx = ws(n + 1); o += &String::from_utf8_lossy(&b[ve..nx]); j = nx; }
                        Some(&b']') => { o += &String::from_utf8_lossy(&b[ve..n]); o.push(']'); return Some((o, n + 1)); }
                        _ => return None,
                    }
                }
            }
            b'"' => { let (_, e) = string(b, i)?; Some((String::from_utf8_lossy(&b[i..e]).into_owned(), e)) }
            _ => {
                let mut e = i;
                while e < b.len() && !matches!(b[e], b',' | b'}' | b']') && !(b[e] as char).is_ascii_whitespace() { e += 1; }
                if e == i { None } else { Some((String::from_utf8_lossy(&b[i..e]).into_owned(), e)) }
            }
        }
    }
    fn string(b: &[u8], i: usize) -> Option<(String, usize)> {
        if b.get(i) != Some(&b'"') { return None; }
        let mut j = i + 1;
        while j < b.len() { match b[j] { b'\\' => j += 2, b'"' => return Some((String::from_utf8_lossy(&b[i + 1..j]).into_owned(), j + 1)), _ => j += 1 } }
        None
    }
    let start = ws(0);
    match val(b, 0) { Some((v, e)) if ws(e) == b.len() => format!("{}{}{}", &t[..start], v, &t[e..]), _ => t.to_string() }
}

/// N14: a `.whl` (zip) file is compared by its members, not by its compressed bytes: for every central-directory entry the name,
/// CRC-32, uncompressed size, external attributes and DOS date/time, in archive order (the CRC and size pin the member content; the
/// DEFLATE stream and the local-header layout are free). `None` when the bytes are not a zip with a readable central directory.
fn zip_members(b: &[u8]) -> Option<String> {
    let u16 = |o: usize| b.get(o..o + 2).map(|x| u16::from_le_bytes([x[0], x[1]]) as u64);
    let u32 = |o: usize| b.get(o..o + 4).map(|x| u32::from_le_bytes([x[0], x[1], x[2], x[3]]) as u64);
    let u64v = |o: usize| b.get(o..o + 8).map(|x| u64::from_le_bytes(x.try_into().unwrap()));
    let eocd = (0..=b.len().checked_sub(22)?).rev().find(|&i| b[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))?;
    let (mut n, mut cd) = (u16(eocd + 10)?, u32(eocd + 16)?);
    if n == 0xFFFF || cd == 0xFFFF_FFFF {
        let loc = eocd.checked_sub(20)?;
        if !b[loc..].starts_with(&[0x50, 0x4b, 0x06, 0x07]) { return None; }
        let rec = u64v(loc + 8)? as usize;
        if !b.get(rec..)?.starts_with(&[0x50, 0x4b, 0x06, 0x06]) { return None; }
        n = u64v(rec + 32)?; cd = u64v(rec + 48)?;
    }
    let mut o = cd as usize; let mut out = String::new();
    for _ in 0..n {
        if u32(o)? != 0x02014b50 { return None; }
        let (time, date, crc, mut usz) = (u16(o + 12)?, u16(o + 14)?, u32(o + 16)?, u32(o + 24)?);
        let (nl, el, cl, attr) = (u16(o + 28)? as usize, u16(o + 30)? as usize, u16(o + 32)? as usize, u32(o + 38)?);
        let name = String::from_utf8_lossy(b.get(o + 46..o + 46 + nl)?).into_owned();
        if usz == 0xFFFF_FFFF {
            let ex = b.get(o + 46 + nl..o + 46 + nl + el)?; let mut p = 0;
            while p + 4 <= ex.len() {
                let (id, sz) = (u16::from_le_bytes([ex[p], ex[p + 1]]), u16::from_le_bytes([ex[p + 2], ex[p + 3]]) as usize);
                if id == 1 { usz = u64::from_le_bytes(ex.get(p + 4..p + 12)?.try_into().ok()?); break; }
                p += 4 + sz;
            }
        }
        out += &format!("{name}\t{crc:08x}\t{usz}\t{attr:08x}\t{date:04x}{time:04x}\n");
        o += 46 + nl + el + cl;
    }
    Some(out)
}

// ---------------------------------------------------------------- execution
struct Outcome { stdout: Vec<u8>, stderr: Vec<u8>, exit: i32, files: BTreeMap<String, Vec<u8>>, python_exec: usize, repo: String, tmp: String }

/// `orig` is metadata only (which original the golden was recorded from; it selects normalizations, it is never run).
struct Case { id: String, entry: String, orig: String, args: Vec<String>, stdin: Option<String>, setup: Vec<(String, String)>, env: Vec<(String, String)>, outputs: Vec<String> }

fn load_cases(dir: &Path, stage: &str) -> Result<Vec<Case>, String> {
    let p = dir.join("cases").join(format!("{stage}.json"));
    let t = fs::read_to_string(&p).map_err(|e| format!("{}: {e}", p.display()))?;
    let j = parse_json(&t).map_err(|e| format!("{}: {e}", p.display()))?;
    let strs = |v: Option<&J>| -> Vec<String> { v.map(|a| a.arr().iter().filter_map(|x| x.str().map(String::from)).collect()).unwrap_or_default() };
    let kv = |v: Option<&J>| -> Vec<(String, String)> { if let Some(J::Obj(o)) = v { o.iter().filter_map(|(k, x)| x.str().map(|s| (k.clone(), s.to_string()))).collect() } else { vec![] } };
    let mut out = vec![];
    for c in j.arr() {
        let id = c.get("id").and_then(|x| x.str()).ok_or("case without id")?.to_string();
        if id.contains('/') || id.contains("..") || id.is_empty() { return Err(format!("bad case id {id}")); }
        out.push(Case {
            id,
            entry: c.get("entry").and_then(|x| x.str()).unwrap_or("").to_string(),
            orig: c.get("orig").and_then(|x| x.str()).unwrap_or("").to_string(),
            args: strs(c.get("args")), stdin: c.get("stdin").and_then(|x| x.str()).map(String::from),
            setup: kv(c.get("setup")), env: kv(c.get("env")), outputs: strs(c.get("outputs")),
        });
    }
    Ok(out)
}

fn count_python_exec(strace_log: &str) -> usize {
    strace_log.lines().filter(|l| {
        let l = l.trim_start_matches(|c: char| c.is_ascii_digit() || c == ' ');
        if !l.starts_with("execve(\"") || l.contains("= -1") { return false; }
        let path = &l["execve(\"".len()..];
        let path = path.split('"').next().unwrap_or("");
        let base = path.rsplit('/').next().unwrap_or("");
        base == "python" || base.starts_with("python3") || base.starts_with("python2")
    }).count()
}

fn scratch(id: &str, tag: &str) -> PathBuf {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    std::env::temp_dir().join(format!("parity-{}-{}-{}-{}", tag, id, std::process::id(), n))
}

fn collect(dir: &Path, base: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() { collect(&p, base, out); } else if let Ok(b) = fs::read(&p) {
                out.insert(p.strip_prefix(base).unwrap().to_string_lossy().into_owned(), b);
            }
        }
    }
}

/// Content of every file under the declared output paths (a file or a directory tree), keyed by path relative to `base`.
fn snapshot(base: &Path, outputs: &[String]) -> BTreeMap<String, Vec<u8>> {
    let mut m = BTreeMap::new();
    for rel in outputs {
        let p = base.join(rel);
        if p.is_dir() { collect(&p, base, &mut m); } else if let Ok(b) = fs::read(&p) { m.insert(rel.clone(), b); }
    }
    m
}

/// The file channel = what the entry itself did: files that are new or whose content changed (their content), and files that
/// existed before and are gone (`<path>` + DELETED_SUFFIX, empty content). Untouched pre-existing files are outside the channel.
const DELETED_SUFFIX: &str = ".parity-deleted";
fn file_delta(before: &BTreeMap<String, Vec<u8>>, after: &BTreeMap<String, Vec<u8>>) -> BTreeMap<String, Vec<u8>> {
    let mut d = BTreeMap::new();
    for (k, v) in after { if before.get(k) != Some(v) { d.insert(k.clone(), v.clone()); } }
    for k in before.keys() { if !after.contains_key(k) { d.insert(format!("{k}{DELETED_SUFFIX}"), vec![]); } }
    d
}

/// Throwaway copy of the repository. `rev` = Some(commit): `git archive` of that commit (no build output; used by the
/// self-check). None = the working tree (tracked + untracked, not ignored) with the
/// ignored `target/` build-output directories symlinked in so launchers find their binaries.
fn make_sandbox(repo: &Path, id: &str, tag: &str, rev: Option<&str>) -> Result<PathBuf, String> {
    let sb = scratch(id, tag);
    fs::create_dir_all(&sb).map_err(|e| e.to_string())?;
    let (script, args): (&str, Vec<String>) = match rev {
        Some(r) => ("git -C \"$1\" archive \"$3\" | tar -x -C \"$2\"", vec![r.to_string()]),
        None => ("git -C \"$1\" ls-files -z --cached --others --exclude-standard | tar -C \"$1\" --null -T - --ignore-failed-read -cf - 2>/dev/null | tar -x -C \"$2\"", vec![]),
    };
    let st = Command::new("sh").arg("-c").arg(script).arg("sh").arg(repo).arg(&sb).args(&args).stderr(Stdio::null()).status().map_err(|e| format!("sandbox: {e}"))?;
    if !st.success() && rev.is_some() { return Err(format!("sandbox: git archive {} failed", rev.unwrap())); }
    if rev.is_none() {
        let o = Command::new("git").arg("-C").arg(repo).args(["ls-files", "-z", "--others", "--ignored", "--exclude-standard", "--directory"]).output().map_err(|e| e.to_string())?;
        for d in String::from_utf8_lossy(&o.stdout).split('\0') {
            let d = d.trim_end_matches('/');
            if d.ends_with("/target") && !d.starts_with(".claude/runtime/") {
                let dst = sb.join(d);
                if let Some(pp) = dst.parent() { fs::create_dir_all(pp).map_err(|e| e.to_string())?; }
                std::os::unix::fs::symlink(repo.join(d), &dst).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(sb)
}

fn remove_sandbox(sb: &Path) {
    if sb.starts_with(std::env::temp_dir()) && sb.file_name().map_or(false, |n| n.to_string_lossy().starts_with("parity-")) { let _ = fs::remove_dir_all(sb); }
}

/// `name` on this tool's PATH (not the case's), or `name` itself when it is not found there.
fn tool_path_lookup(name: &str) -> PathBuf {
    std::env::var_os("PATH")
        .and_then(|p| std::env::split_paths(&p).map(|d| d.join(name)).find(|c| c.is_file()))
        .unwrap_or_else(|| PathBuf::from(name))
}

/// Run one program for a case inside its own repository copy (cwd = the copy; setup and outputs live there too), so the
/// entry runs against a copy and cannot touch the real repository. `rev`: a committed tree instead of the working tree.
fn run(case: &Case, program: &str, repo: &Path, strace: bool, rev: Option<&str>) -> Result<Outcome, String> {
    let tmp = make_sandbox(repo, &case.id, if strace { "c" } else { "o" }, rev)?;
    for (rel, content) in &case.setup {
        if rel.starts_with('/') || rel.contains("..") { return Err(format!("{}: unsafe setup path {rel}", case.id)); }
        let p = tmp.join(rel);
        if let Some(d) = p.parent() { fs::create_dir_all(d).map_err(|e| e.to_string())?; }
        fs::write(&p, content).map_err(|e| e.to_string())?;
    }
    let before = snapshot(&tmp, &case.outputs);
    let prog = if Path::new(program).is_absolute() { PathBuf::from(program) } else { tmp.join(program) };
    let log = tmp.parent().unwrap().join(format!("{}.strace", tmp.file_name().unwrap().to_string_lossy()));
    let mut cmd;
    if strace {
        // strace itself is found on the tool's own PATH: a case may set its own PATH (e.g. one without host tools).
        cmd = Command::new(tool_path_lookup("strace"));
        cmd.args(["-f", "-e", "trace=execve", "-o"]).arg(&log).arg(&prog);
    } else { cmd = Command::new(&prog); }
    cmd.args(&case.args).current_dir(&tmp).env_remove("EASY_VLLM_SOURCE_ROOT").env("PARITY_GIT_DIR", repo.join(".git"))
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in &case.env { cmd.env(k, v); }
    let mut child = cmd.spawn().map_err(|e| format!("{}: spawn {}: {e}", case.id, prog.display()))?;
    if let Some(mut si) = child.stdin.take() { if let Some(t) = &case.stdin { let _ = si.write_all(t.as_bytes()); } }
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    let python_exec = if strace { count_python_exec(&fs::read_to_string(&log).unwrap_or_default()) } else { 0 };
    if strace { let _ = fs::remove_file(&log); }
    let files = file_delta(&before, &snapshot(&tmp, &case.outputs));
    let tmps = tmp.to_string_lossy().into_owned();
    remove_sandbox(&tmp);
    Ok(Outcome { stdout: o.stdout, stderr: o.stderr, exit: o.status.code().unwrap_or(-1), files, python_exec, repo: tmps.clone(), tmp: tmps })
}

// ---------------------------------------------------------------- golden I/O
// Goldens are read-only data committed under `data/<case-id>/` (stdout, stderr, exit_code, files.index.json + files/, meta.json);
// this tool has no writer for them.
fn load_golden(dir: &Path, id: &str) -> Result<Outcome, String> {
    let g = dir.join("data").join(id);
    let rd = |n: &str| fs::read(g.join(n)).map_err(|e| format!("golden {id}/{n}: {e}"));
    let meta = parse_json(&String::from_utf8_lossy(&rd("meta.json")?)).map_err(|e| format!("golden {id}/meta.json: {e}"))?;
    let exit = String::from_utf8_lossy(&rd("exit_code")?).trim().parse::<i32>().map_err(|e| format!("golden {id}/exit_code: {e}"))?;
    let idx = parse_json(&String::from_utf8_lossy(&rd("files.index.json")?)).map_err(|e| e.to_string())?;
    let mut files = BTreeMap::new();
    for r in idx.arr() { if let Some(s) = r.str() { files.insert(s.to_string(), fs::read(g.join("files").join(s)).map_err(|e| format!("golden {id}/files/{s}: {e}"))?); } }
    Ok(Outcome { stdout: rd("stdout")?, stderr: rd("stderr")?, exit, files, python_exec: 0,
        repo: meta.get("repo_root").and_then(|x| x.str()).unwrap_or("").into(), tmp: meta.get("tmp_root").and_then(|x| x.str()).unwrap_or("").into() })
}

// ---------------------------------------------------------------- comparison
fn first_diff(a: &str, b: &str) -> String {
    for (i, (x, y)) in a.lines().zip(b.lines()).enumerate() { if x != y { return format!("line {}: golden={:?} actual={:?}", i + 1, x, y); } }
    format!("line count golden={} actual={}", a.lines().count(), b.lines().count())
}

/// Returns a list of "channel: detail" mismatches (empty = equal).
fn diff_outcomes(n: &Norm, g: &Outcome, a: &Outcome, n13: bool) -> Vec<String> {
    let mut m = vec![];
    for (name, ch, gb, ab) in [("stdout", Ch::Stdout, &g.stdout, &a.stdout), ("stderr", Ch::Stderr, &g.stderr, &a.stderr)] {
        let (mut x, mut y) = (n.text(ch, gb, &g.repo, &g.tmp), n.text(ch, ab, &a.repo, &a.tmp));
        if n13 && ch == Ch::Stdout { x = sort_keys(&x); y = sort_keys(&y); }
        if x != y { m.push(format!("{name}: {}", first_diff(&x, &y))); }
    }
    if g.exit != a.exit { m.push(format!("exit_code: golden={} actual={}", g.exit, a.exit)); }
    let gk: Vec<&String> = g.files.keys().collect(); let ak: Vec<&String> = a.files.keys().collect();
    if gk != ak { m.push(format!("files: path set golden={gk:?} actual={ak:?}")); }
    else { for (k, gb) in &g.files { if n.on("N14") && k.ends_with(".whl") { if let (Some(x), Some(y)) = (zip_members(gb), zip_members(&a.files[k])) { if x != y { m.push(format!("files/{k}: {}", first_diff(&x, &y))); } continue; } } let (mut x, mut y) = (n.text(Ch::File, gb, &g.repo, &g.tmp), n.text(Ch::File, &a.files[k], &a.repo, &a.tmp)); if n13 && k.ends_with(".json") { x = sort_keys(&x); y = sort_keys(&y); } if x != y { m.push(format!("files/{k}: {}", first_diff(&x, &y))); } } }
    m
}

fn write_raw(raw: &Path, id: &str, o: &Outcome) {
    let d = raw.join(id);
    if fs::create_dir_all(d.join("files")).is_err() { return; }
    let _ = fs::write(d.join("stdout"), &o.stdout); let _ = fs::write(d.join("stderr"), &o.stderr);
    let _ = fs::write(d.join("exit_code"), format!("{}\n", o.exit));
    for (k, b) in &o.files { let p = d.join("files").join(k); if let Some(pp) = p.parent() { let _ = fs::create_dir_all(pp); } let _ = fs::write(p, b); }
}

// ---------------------------------------------------------------- CLI
struct Args { mode: String, stage: String, cases: Vec<String>, repo: PathBuf, raw: PathBuf, na_list: Option<PathBuf>, tree: Option<PathBuf>, out: Option<PathBuf> }

fn parse_args(v: &[String]) -> Result<Args, String> {
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = tool.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).map(PathBuf::from).unwrap_or_default();
    let mut a = Args { mode: String::new(), stage: "all".into(), cases: vec![], repo, raw: tool.join("raw"), na_list: None, tree: None, out: None };
    let mut i = 0;
    while i < v.len() {
        match v[i].as_str() {
            "--self-check" => a.mode = "self-check".into(),
            "compare" | "na-list" => a.mode = v[i].clone(),
            m @ ("capture" | "orig") => return Err(format!(
                "mode {m} is retired: the comparison basis is the committed goldens in data/ and this tool never runs an original \
                 Python program (use `compare`)")),
            f @ ("--stage" | "--case" | "--repo" | "--raw-dir" | "--na-list" | "--tree" | "--out") => {
                i += 1; let val = v.get(i).ok_or(format!("{f} needs a value"))?.clone();
                match f {
                    "--stage" => a.stage = val, "--case" => a.cases.push(val), "--repo" => a.repo = PathBuf::from(val),
                    "--na-list" => a.na_list = Some(PathBuf::from(val)), "--tree" => a.tree = Some(PathBuf::from(val)),
                    "--out" => a.out = Some(PathBuf::from(val)), _ => a.raw = PathBuf::from(val),
                }
            }
            x => return Err(format!("unknown argument {x}")),
        }
        i += 1;
    }
    if a.mode.is_empty() { return Err("mode required: compare|na-list|--self-check".into()); }
    if a.stage != "all" && !STAGES.contains(&a.stage.as_str()) && !HP_STAGES.contains(&a.stage.as_str()) { return Err(format!("bad stage {}", a.stage)); }
    if a.mode == "na-list" && (a.tree.is_none() || a.out.is_none()) { return Err("na-list needs --tree DIR and --out FILE".into()); }
    if a.mode != "na-list" && (a.tree.is_some() || a.out.is_some()) { return Err("--tree/--out apply to na-list only".into()); }
    Ok(a)
}

/// Header line of a generated Sub N/A list.
const NA_HEADER: &str = "# Sub N/A list — cases whose entry point is not in the rendered Sub tree. Generated by `parity na-list`; do not hand-edit.";

/// Is `entry` in the rendered Sub `tree`? An entry under `<crate>/target/release/` is judged by the crate's Cargo.toml (the ELF
/// only exists after the Sub builds it); any other entry by its path.
fn present_in_tree(tree: &Path, entry: &str) -> bool {
    const MARKER: &str = "/target/release/";
    match entry.find(MARKER) {
        Some(at) => tree.join(&entry[..at]).join("Cargo.toml").is_file(),
        None => tree.join(entry).exists(),
    }
}

/// `na-list --tree T --out F`: the mechanical Sub N/A list (cases of every stage, in stage and case order, whose entry point is
/// not in the rendered Sub tree T). Source of truth = the render result, not a hand list. Writes F and reports the count.
fn na_list(tool: &Path, tree: &Path, out: &Path) -> Result<i32, String> {
    match na_list_text(tool, tree)? {
        None => { eprintln!("tree has no .claude — not a rendered Sub tree"); Ok(2) }
        Some((text, n)) => {
            fs::write(out, text).map_err(|e| format!("{}: {e}", out.display()))?;
            println!("{}: {n} N/A cases", out.display());
            Ok(0)
        }
    }
}

/// The text of the N/A list for `tree` and its row count; None when `tree` is not a rendered Sub tree (no `.claude`).
fn na_list_text(tool: &Path, tree: &Path) -> Result<Option<(String, usize)>, String> {
    if !tree.join(".claude").is_dir() { return Ok(None); }
    let mut text = format!("{NA_HEADER}\n");
    let mut n = 0usize;
    for s in STAGES {
        for c in load_cases(tool, s)? {
            if !present_in_tree(tree, &c.entry) { text.push_str(&format!("{} {}\n", c.id, c.entry)); n += 1; }
        }
    }
    Ok(Some((text, n)))
}

fn orphan_ids(map: &BTreeMap<String, String>, known: &[String]) -> Vec<String> {
    map.keys().filter(|k| !known.iter().any(|x| x == *k)).cloned().collect()
}

/// `--na-list FILE`: lines `<case-id> <entry path>`; `#` comments and blank lines ignored; duplicate ids are an error.
fn parse_na_list(p: &Path) -> Result<BTreeMap<String, String>, String> {
    let t = fs::read_to_string(p).map_err(|e| format!("na-list {}: {e}", p.display()))?;
    let mut m = BTreeMap::new();
    for (i, l) in t.lines().enumerate() {
        let l = l.trim(); if l.is_empty() || l.starts_with('#') { continue; }
        let mut it = l.split_whitespace();
        let (id, e) = (it.next().unwrap_or(""), it.next().ok_or(format!("na-list line {}: need `<case-id> <entry>`", i + 1))?);
        if it.next().is_some() { return Err(format!("na-list line {}: extra fields", i + 1)); }
        if m.insert(id.to_string(), e.to_string()).is_some() { return Err(format!("na-list line {}: duplicate id {id}", i + 1)); }
    }
    Ok(m)
}

fn real_main() -> Result<i32, String> {
    let v: Vec<String> = std::env::args().skip(1).collect();
    let a = parse_args(&v)?;
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if a.mode == "self-check" { return self_check(); }
    if a.mode == "na-list" { return na_list(&tool, a.tree.as_deref().unwrap_or(Path::new("")), a.out.as_deref().unwrap_or(Path::new(""))); }
    let norm = Norm::load(&tool.join("normalize.json"))?;
    let stages: Vec<&str> = if a.stage == "all" { STAGES.to_vec() } else { vec![a.stage.as_str()] };
    let mut cases = vec![];
    for s in &stages { cases.extend(load_cases(&tool, s)?); }
    if !a.cases.is_empty() { cases.retain(|c| a.cases.contains(&c.id)); }
    if cases.is_empty() { return Err("no cases selected".into()); }
    if Command::new("strace").arg("-V").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() { return Err("strace is not available".into()); }
    let na = match &a.na_list { Some(p) => Some(parse_na_list(p)?), None => None };
    let mut bad = 0; let mut py = 0; let mut na_n = 0usize;
    if let Some(map) = &na {
        // Orphan rows (id in no stage's cases) fail: a renamed/removed case must not leave a silent hole in the list.
        let mut known = vec![]; for s in STAGES { known.extend(load_cases(&tool, s)?.into_iter().map(|c| c.id)); }
        for id in orphan_ids(map, &known) { bad += 1; println!("ORPHAN {id} na-list row matches no case"); }
    }
    for c in &cases {
        if let Some(map) = &na {
            if let Some(entry) = map.get(&c.id) {
                // Case the Sub is not designed to receive: not run, only counted. Stale = listed entry differs from the case or exists in the tree.
                if *entry != c.entry { bad += 1; println!("STALE {} na-list entry {} != case entry {}", c.id, entry, c.entry); }
                else if a.repo.join(entry).exists() { bad += 1; println!("STALE {} entry present in tree ({})", c.id, entry); }
                else { na_n += 1; }
                continue;
            }
        }
        if c.entry.starts_with("@pending:") {
            bad += 1; println!("MISMATCH {} entry not converted yet ({})", c.id, c.entry); continue;
        }
        let g = load_golden(&tool, &c.id)?;
        let o = run(c, &c.entry, &a.repo, true, None)?;
        py += o.python_exec;
        let n13 = norm.n13_for(&c.entry) || norm.n13_for(&c.orig);
        let mut m = diff_outcomes(&norm, &g, &o, n13);
        if o.python_exec > 0 { m.push(format!("python_exec={}", o.python_exec)); }
        if !m.is_empty() { bad += 1; write_raw(&a.raw, &c.id, &o); for x in m { println!("MISMATCH {} {}", c.id, x); } }
    }
    let n = cases.len();
    if bad == 0 && py == 0 {
        if na.is_some() { println!("na={na_n}"); }
        println!("PARITY-{}-PASS python_exec=0", a.stage.to_uppercase())
    } else if na.is_some() { println!("compare: {bad}/{n} cases mismatch na={na_n} python_exec={py}") }
    else { println!("compare: {bad}/{n} cases mismatch python_exec={py}") }
    Ok(if bad == 0 { 0 } else { 1 })
}

// ---------------------------------------------------------------- self-check (no goldens)
fn self_check() -> Result<i32, String> {
    let mut fails: Vec<String> = vec![];
    let mut ck = |name: &str, ok: bool| { if !ok { fails.push(name.to_string()); } };
    let n = Norm { ids: KNOWN_IDS.iter().map(|s| s.to_string()).collect(), n13_entries: vec!["lite_metrics".to_string()] };
    let t = |ch, s: &str| n.text(ch, s.as_bytes(), "/r/w", "/tmp/x1");
    ck("N01", t(Ch::Stdout, "a /r/w/b") == "a <REPO>/b");
    ck("N02", t(Ch::Stdout, "a /tmp/x1/f") == "a <TMP>/f");
    ck("N03-iso", t(Ch::Stdout, "at 2026-11-15T08:01:53.123Z ok") == "at <TS> ok");
    ck("N03-epoch", t(Ch::Stdout, "t=1763161313;") == "t=<TS>;" && t(Ch::Stdout, "n=12345678901") == "n=12345678901");
    ck("N04-stderr", t(Ch::Stderr, "pid=4242 x") == "pid=<PID> x");
    ck("N04-stdout-untouched", t(Ch::Stdout, "pid=4242 x") == "pid=4242 x");
    ck("N11-addr", t(Ch::Stdout, "x <yaml.error.Mark object at 0x7f1a2b3c4d50> y") == t(Ch::Stdout, "x <yaml.error.Mark object at 0xdeadbeef> y"));
    ck("N11-context-strict", t(Ch::Stdout, "x <yaml.error.Mark object at 0x1> y") != t(Ch::Stdout, "z <yaml.error.Mark object at 0x1> y"));
    ck("N11-no-0x-untouched", t(Ch::Stdout, "<yaml.error.Mark object>") == "<yaml.error.Mark object>" && t(Ch::Stdout, "<yaml.error.Mark object at 0xZZ>") == "<yaml.error.Mark object at 0xZZ>");
    ck("N11-off", { let off = Norm { ids: vec![], n13_entries: vec![] }; off.text(Ch::Stdout, b"<yaml.error.Mark object at 0x1>", "", "") != off.text(Ch::Stdout, b"<yaml.error.Mark object at 0x2>", "", "") });
    ck("N06", t(Ch::Stderr, "Traceback (most recent call last):\n  File \"/r/w/x.py\", line 3\n    boom()\nValueError: bad\n") == "ValueError: bad\n");
    ck("N06-plain-stderr-untouched", t(Ch::Stderr, "warn: x\n") == "warn: x\n");
    ck("closed-list", { let p = scratch("n", "self"); fs::create_dir_all(&p).ok(); let f = p.join("n.json"); fs::write(&f, "{\"normalizations\":[{\"id\":\"N99\"}]}").ok(); let r = Norm::load(&f).is_err(); let _ = fs::remove_dir_all(&p); r });
    // comparison: equal, then each channel differing
    let mk = |so: &str, se: &str, ex: i32, fv: &str| { let mut f = BTreeMap::new(); f.insert("o.txt".to_string(), fv.as_bytes().to_vec()); Outcome { stdout: so.into(), stderr: se.into(), exit: ex, files: f, python_exec: 0, repo: "/r/w".into(), tmp: "/tmp/x1".into() } };
    let g = mk("a\n", "", 0, "f");
    ck("cmp-equal", diff_outcomes(&n, &g, &mk("a\n", "", 0, "f"), false).is_empty());
    ck("cmp-stdout", !diff_outcomes(&n, &g, &mk("b\n", "", 0, "f"), false).is_empty());
    ck("cmp-stderr", !diff_outcomes(&n, &g, &mk("a\n", "e", 0, "f"), false).is_empty());
    ck("cmp-exit", !diff_outcomes(&n, &g, &mk("a\n", "", 1, "f"), false).is_empty());
    ck("cmp-file", !diff_outcomes(&n, &g, &mk("a\n", "", 0, "g"), false).is_empty());
    let mut other = mk("a\n", "", 0, "f"); other.repo = "/q/z".into(); other.tmp = "/tmp/y9".into(); other.stdout = b"a\n".to_vec();
    let mut g2 = mk("p /r/w/a\n", "", 0, "f"); g2.stdout = b"p /r/w/a\n".to_vec(); other.stdout = b"p /q/z/a\n".to_vec();
    ck("cmp-roots-normalized", diff_outcomes(&n, &g2, &other, false).is_empty());
    // python execve counting
    ck("py-parse", count_python_exec("123 execve(\"/usr/bin/python3.12\", [\"python3\"], 0x0 /* 3 vars */) = 0\nexecve(\"/bin/sh\", [], 0) = 0\nexecve(\"/usr/bin/python3\", [], 0) = -1 ENOENT\n") == 1);
    // real strace runs
    let run_trace = |prog: &[&str]| -> Option<usize> {
        let log = scratch("st", "self");
        let ok = Command::new("strace").args(["-f", "-e", "trace=execve", "-o"]).arg(&log).args(prog).stdout(Stdio::null()).stderr(Stdio::null()).status().ok()?.success();
        let r = count_python_exec(&fs::read_to_string(&log).unwrap_or_default()); let _ = fs::remove_file(&log); if ok { Some(r) } else { None }
    };
    ck("strace-zero", run_trace(&["/bin/sh", "-c", "echo hi"]) == Some(0));
    ck("strace-sees-python", run_trace(&["/bin/sh", "-c", "python3 -c pass"]).map_or(false, |x| x >= 1));
    // N13: key order is normalized, values/lexemes/layout are not
    ck("N13-sort", sort_keys("{\"b\": 1.0, \"a\": {\"z\": [1, {\"y\": 2, \"x\": 3}], \"c\": \"s\"}}\n") == "{\"a\": {\"c\": \"s\", \"z\": [1, {\"x\": 3, \"y\": 2}]}, \"b\": 1.0}\n");
    ck("N13-nonjson-untouched", sort_keys("not json {") == "not json {");
    ck("N13-value-diff-kept", sort_keys("{\"b\": 1, \"a\": 2}") != sort_keys("{\"b\": 2, \"a\": 1}"));
    ck("N13-applies", n.n13_for("a/lite_metrics.py") && n.n13_for("lite_metrics") && !n.n13_for("a/other.py"));
    {
        let mut f = BTreeMap::new(); f.insert("o.json".to_string(), b"{\"b\":1,\"a\":2}".to_vec());
        let g = Outcome { stdout: b"{\"b\": 1, \"a\": 2}".to_vec(), stderr: vec![], exit: 0, files: f.clone(), python_exec: 0, repo: "/r".into(), tmp: "/t".into() };
        let mut f2 = BTreeMap::new(); f2.insert("o.json".to_string(), b"{\"a\":2,\"b\":1}".to_vec());
        let a = Outcome { stdout: b"{\"a\": 2, \"b\": 1}".to_vec(), stderr: vec![], exit: 0, files: f2, python_exec: 0, repo: "/r".into(), tmp: "/t".into() };
        ck("N13-off-mismatch", !diff_outcomes(&n, &g, &a, false).is_empty());
        ck("N13-on-equal", diff_outcomes(&n, &g, &a, true).is_empty());
    // N14: wheel members, not compressed bytes
    {
        fn mini_zip(crc: u32, payload: &[u8], usz: u32) -> Vec<u8> {
            let mut z = Vec::new(); let name = b"a/x.py";
            z.extend_from_slice(&[0x50, 0x4b, 3, 4]); z.extend_from_slice(&[20, 0, 0, 0, 8, 0, 0, 0, 33, 0]);
            z.extend_from_slice(&crc.to_le_bytes()); z.extend_from_slice(&(payload.len() as u32).to_le_bytes()); z.extend_from_slice(&usz.to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes()); z.extend_from_slice(&[0, 0]); z.extend_from_slice(name); z.extend_from_slice(payload);
            let cd = z.len() as u32;
            z.extend_from_slice(&[0x50, 0x4b, 1, 2, 20, 3, 20, 0, 0, 0, 8, 0, 0, 0, 33, 0]);
            z.extend_from_slice(&crc.to_le_bytes()); z.extend_from_slice(&(payload.len() as u32).to_le_bytes()); z.extend_from_slice(&usz.to_le_bytes());
            z.extend_from_slice(&(name.len() as u16).to_le_bytes()); z.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]);
            z.extend_from_slice(&(0o100644u32 << 16).to_le_bytes()); z.extend_from_slice(&[0, 0, 0, 0]); z.extend_from_slice(name);
            let cds = z.len() as u32 - cd;
            z.extend_from_slice(&[0x50, 0x4b, 5, 6, 0, 0, 0, 0, 1, 0, 1, 0]); z.extend_from_slice(&cds.to_le_bytes()); z.extend_from_slice(&cd.to_le_bytes()); z.extend_from_slice(&[0, 0]);
            z
        }
        let (a, b, c) = (mini_zip(7, b"AAAA", 10), mini_zip(7, b"BBBBBB", 10), mini_zip(8, b"AAAA", 10));
        ck("N14-parse", zip_members(&a).map_or(false, |t| t.starts_with("a/x.py\t00000007\t10\t")));
        ck("N14-compressed-bytes-free", a != b && zip_members(&a) == zip_members(&b));
        ck("N14-crc-diff-kept", zip_members(&a) != zip_members(&c));
        ck("N14-nonzip-none", zip_members(b"not a zip").is_none());
    }
    }
    // file channel = before/after delta of the entry's own writes
    {
        let m = |v: &[(&str, &str)]| -> BTreeMap<String, Vec<u8>> { v.iter().map(|(k, c)| (k.to_string(), c.as_bytes().to_vec())).collect() };
        let before = m(&[("d/keep", "k"), ("d/mod", "1"), ("d/gone", "g")]);
        let after = m(&[("d/keep", "k"), ("d/mod", "2"), ("d/new", "n")]);
        let d = file_delta(&before, &after);
        ck("delta-new", d.get("d/new").map(|x| x.as_slice()) == Some(b"n".as_slice()));
        ck("delta-modified", d.get("d/mod").map(|x| x.as_slice()) == Some(b"2".as_slice()));
        ck("delta-deleted", d.get(&format!("d/gone{DELETED_SUFFIX}")).map_or(false, |x| x.is_empty()));
        ck("delta-untouched-outside", !d.contains_key("d/keep") && d.len() == 3);
        ck("delta-none", file_delta(&before, &before).is_empty());
    }
    // end-to-end: synthetic case run through the same machinery inside a throwaway git repo (sandbox: working tree and a commit)
    let repo = scratch("repo", "self"); fs::create_dir_all(&repo).ok();
    let git = |a: &[&str]| Command::new("git").arg("-C").arg(&repo).args(["-c", "user.email=a@b", "-c", "user.name=n"]).args(a).stdout(Stdio::null()).stderr(Stdio::null()).status().map_or(false, |s| s.success());
    fs::write(repo.join("tracked.txt"), "t\n").ok();
    let setup_ok = git(&["init", "-q"]) && git(&["add", "tracked.txt"]) && git(&["commit", "-qm", "x"]);
    fs::write(repo.join("untracked.txt"), "u\n").ok();
    ck("e2e-setup", setup_ok);
    let case = Case { id: "synth".into(), entry: "/bin/sh".into(), orig: "/bin/sh".into(), args: vec!["-c".into(), "echo out; echo err >&2; echo data > o.txt; echo \"$(cat tracked.txt)$(cat untracked.txt 2>/dev/null)\" > seen.txt; pwd; exit 3".into()], stdin: None, setup: vec![], env: vec![], outputs: vec!["o.txt".into(), "seen.txt".into()] };
    match (run(&case, "/bin/sh", &repo, false, Some("HEAD")), run(&case, "/bin/sh", &repo, true, None)) {
        (Ok(a), Ok(b)) => {
            ck("e2e-exit", a.exit == 3 && b.exit == 3);
            ck("e2e-equal-modulo-seen", a.files.get("o.txt") == b.files.get("o.txt") && b.python_exec == 0);
            ck("e2e-file", a.files.get("o.txt").map(|x| x.as_slice()) == Some(b"data\n".as_slice()));
            ck("e2e-rev-sandbox-is-committed-tree", a.files.get("seen.txt").map(|x| x.as_slice()) == Some(b"t\n".as_slice()));
            ck("e2e-worktree-sandbox-has-untracked", b.files.get("seen.txt").map(|x| x.as_slice()) == Some(b"tu\n".as_slice()));
            ck("e2e-cwd-is-sandbox", String::from_utf8_lossy(&a.stdout).contains(&a.repo) && !String::from_utf8_lossy(&a.stdout).contains(repo.to_string_lossy().as_ref()));
            ck("e2e-sandbox-removed", !Path::new(&a.repo).exists() && !Path::new(&b.repo).exists());
        }
        _ => ck("e2e-run", false),
    }
    ck("e2e-real-repo-untouched", !repo.join("o.txt").exists() && !repo.join("seen.txt").exists());
    let _ = fs::remove_dir_all(&repo);
    {
        let d = scratch("nalist", "self"); let _ = fs::create_dir_all(&d);
        let f = d.join("na.txt");
        let _ = fs::write(&f, "# c\nA-1 bin/x\n\nA-2 bin/y\n");
        let m = parse_na_list(&f);
        ck("na-parse", m.as_ref().map_or(false, |m| m.len() == 2 && m.get("A-1").map(String::as_str) == Some("bin/x")));
        let _ = fs::write(&f, "A-1 bin/x\nA-1 bin/z\n");
        ck("na-dup-rejected", parse_na_list(&f).is_err());
        let _ = fs::write(&f, "A-1\n");
        ck("na-missing-entry-rejected", parse_na_list(&f).is_err());
        ck("na-missing-file-rejected", parse_na_list(&d.join("none.txt")).is_err());
        let _ = fs::remove_dir_all(&d);
        let mut om = BTreeMap::new(); om.insert("A-1".to_string(), "x".to_string()); om.insert("Z-9".to_string(), "y".to_string());
        ck("na-orphan-detected", orphan_ids(&om, &["A-1".to_string(), "B-2".to_string()]) == vec!["Z-9".to_string()]);
        ck("na-no-orphan-when-all-known", orphan_ids(&om, &["A-1".to_string(), "Z-9".to_string()]).is_empty());
    }
    // the retired capture/orig modes are refused (no path of this tool runs an original Python program)
    {
        let args = |v: &[&str]| parse_args(&v.iter().map(|s| s.to_string()).collect::<Vec<_>>());
        ck("retired-capture", args(&["capture"]).err().map_or(false, |e| e.contains("retired")));
        ck("retired-orig", args(&["orig", "--stage", "s2"]).err().map_or(false, |e| e.contains("retired")));
        ck("na-list-needs-tree-out", args(&["na-list", "--tree", "x"]).is_err());
        ck("tree-only-for-na-list", args(&["compare", "--tree", "x"]).is_err());
    }
    // na-list: the entry judgement (crate target by Cargo.toml, anything else by path) and the generated file
    {
        let d = scratch("natree", "self"); let t = d.join("tree");
        let _ = fs::create_dir_all(t.join(".claude/k/crate")); let _ = fs::create_dir_all(t.join(".claude/s"));
        let _ = fs::write(t.join(".claude/k/crate/Cargo.toml"), ""); let _ = fs::write(t.join(".claude/s/x.sh"), "");
        ck("na-present-crate", present_in_tree(&t, ".claude/k/crate/target/release/bin"));
        ck("na-absent-crate", !present_in_tree(&t, ".claude/k/other/target/release/bin"));
        ck("na-present-path", present_in_tree(&t, ".claude/s/x.sh") && !present_in_tree(&t, ".claude/s/y.sh"));
        let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let out = d.join("na.txt");
        ck("na-not-a-tree", na_list_text(&tool, &d.join("none")) == Ok(None));
        let total: usize = STAGES.iter().map(|s| load_cases(&tool, s).map_or(0, |c| c.len())).sum();
        let wrote = match na_list_text(&tool, &t) { Ok(Some((text, n))) => n == total && fs::write(&out, text).is_ok(), _ => false };
        ck("na-list-roundtrip", wrote && fs::read_to_string(&out).map_or(false, |x| x.starts_with(NA_HEADER))
            && parse_na_list(&out).map_or(false, |m| m.len() == total));
        let _ = fs::remove_dir_all(&d);
    }
    if fails.is_empty() { println!("PARITY-SELFCHECK-OK"); Ok(0) } else { for f in &fails { println!("SELFCHECK-FAIL {f}"); } Ok(1) }
}

fn main() {
    match real_main() { Ok(c) => std::process::exit(c), Err(e) => { eprintln!("parity: {e}"); std::process::exit(2) } }
}
