//! parity — golden capture/compare for the Rust repack (std only, no external crates).
//! Modes: capture | orig | compare [--stage s1..s4|all] [--case ID]... [--repo W] [--raw-dir DIR] [--force], --self-check.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::Write;

const ORIG_COMMIT: &str = "a6769be";
const KNOWN_IDS: [&str; 7] = ["N01", "N02", "N03", "N04", "N06", "N08", "N09"];
const STAGES: [&str; 4] = ["s1", "s2", "s3", "s4"];

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
fn jstr(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() { match c { '"' => o += "\\\"", '\\' => o += "\\\\", '\n' => o += "\\n", '\r' => o += "\\r", '\t' => o += "\\t", c if (c as u32) < 0x20 => o += &format!("\\u{:04x}", c as u32), c => o.push(c) } }
    o.push('"'); o
}

// ---------------------------------------------------------------- normalization (closed list)
#[derive(Clone, Copy, PartialEq)]
enum Ch { Stdout, Stderr, File }

struct Norm { ids: Vec<String> }
impl Norm {
    fn load(path: &Path) -> Result<Norm, String> {
        let t = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let j = parse_json(&t).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut ids = vec![];
        for n in j.get("normalizations").map(|a| a.arr()).unwrap_or(&[]) {
            let id = n.get("id").and_then(|x| x.str()).ok_or("normalization without id")?;
            if !KNOWN_IDS.contains(&id) { return Err(format!("normalization {id} is not implemented (closed list)")); }
            ids.push(id.to_string());
        }
        Ok(Norm { ids })
    }
    fn on(&self, id: &str) -> bool { self.ids.iter().any(|x| x == id) }
    fn text(&self, ch: Ch, raw: &[u8], repo: &str, tmp: &str) -> String {
        let mut s = String::from_utf8_lossy(raw).into_owned();
        if self.on("N01") && !repo.is_empty() { s = s.replace(repo, "<REPO>"); }
        if self.on("N02") && !tmp.is_empty() { s = s.replace(tmp, "<TMP>"); }
        if self.on("N03") { s = scrub_ts(&s); }
        if self.on("N04") && ch == Ch::Stderr { s = scrub_pid(&s); }
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

// ---------------------------------------------------------------- execution
struct Outcome { stdout: Vec<u8>, stderr: Vec<u8>, exit: i32, files: BTreeMap<String, Vec<u8>>, python_exec: usize, repo: String, tmp: String }

struct Case { id: String, stage: String, entry: String, orig: String, args: Vec<String>, stdin: Option<String>, setup: Vec<(String, String)>, env: Vec<(String, String)>, outputs: Vec<String> }

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
            id, stage: stage.to_string(),
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

/// Run one program for a case in a fresh scratch workspace. `strace` = wrap in execve tracing.
fn run(case: &Case, program: &str, repo: &Path, strace: bool, interp_py: bool) -> Result<Outcome, String> {
    let tmp = scratch(&case.id, if strace { "c" } else { "o" });
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    for (rel, content) in &case.setup {
        if rel.starts_with('/') || rel.contains("..") { return Err(format!("{}: unsafe setup path {rel}", case.id)); }
        let p = tmp.join(rel);
        if let Some(d) = p.parent() { fs::create_dir_all(d).map_err(|e| e.to_string())?; }
        fs::write(&p, content).map_err(|e| e.to_string())?;
    }
    let prog = if Path::new(program).is_absolute() { PathBuf::from(program) } else { repo.join(program) };
    let log = tmp.parent().unwrap().join(format!("{}.strace", tmp.file_name().unwrap().to_string_lossy()));
    let mut cmd;
    if strace {
        cmd = Command::new("strace");
        cmd.args(["-f", "-e", "trace=execve", "-o"]).arg(&log).arg(&prog);
    } else if interp_py {
        cmd = Command::new("python3"); cmd.arg("-B").arg(&prog);
    } else { cmd = Command::new(&prog); }
    cmd.args(&case.args).current_dir(&tmp).env_remove("EASY_VLLM_SOURCE_ROOT")
        .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    for (k, v) in &case.env { cmd.env(k, v); }
    let mut child = cmd.spawn().map_err(|e| format!("{}: spawn {}: {e}", case.id, prog.display()))?;
    if let Some(mut si) = child.stdin.take() { if let Some(t) = &case.stdin { let _ = si.write_all(t.as_bytes()); } }
    let o = child.wait_with_output().map_err(|e| e.to_string())?;
    let python_exec = if strace { count_python_exec(&fs::read_to_string(&log).unwrap_or_default()) } else { 0 };
    if strace { let _ = fs::remove_file(&log); }
    let mut files = BTreeMap::new();
    for rel in &case.outputs {
        let p = tmp.join(rel);
        if p.is_dir() { collect(&p, &tmp, &mut files); } else if let Ok(b) = fs::read(&p) { files.insert(rel.clone(), b); }
    }
    let tmps = tmp.to_string_lossy().into_owned();
    let _ = fs::remove_dir_all(&tmp);
    Ok(Outcome { stdout: o.stdout, stderr: o.stderr, exit: o.status.code().unwrap_or(-1), files, python_exec, repo: repo.to_string_lossy().into_owned(), tmp: tmps })
}

// ---------------------------------------------------------------- golden I/O
fn save_golden(dir: &Path, case: &Case, o: &Outcome) -> Result<(), String> {
    let g = dir.join("data").join(&case.id);
    fs::create_dir_all(g.join("files")).map_err(|e| e.to_string())?;
    let w = |n: &str, b: &[u8]| fs::write(g.join(n), b).map_err(|e| e.to_string());
    w("stdout", &o.stdout)?; w("stderr", &o.stderr)?; w("exit_code", format!("{}\n", o.exit).as_bytes())?;
    let mut idx = String::from("[");
    for (i, (rel, b)) in o.files.iter().enumerate() {
        let p = g.join("files").join(rel);
        if let Some(d) = p.parent() { fs::create_dir_all(d).map_err(|e| e.to_string())?; }
        fs::write(&p, b).map_err(|e| e.to_string())?;
        if i > 0 { idx.push(','); } idx += &jstr(rel);
    }
    idx.push(']');
    w("files.index.json", idx.as_bytes())?;
    let meta = format!("{{\"case\":{{\"id\":{},\"stage\":{},\"entry\":{},\"orig\":{},\"args\":[{}]}},\"orig_commit\":\"{}\",\"tool\":\"parity {}\",\"provenance\":\"measured\",\"repo_root\":{},\"tmp_root\":{}}}\n",
        jstr(&case.id), jstr(&case.stage), jstr(&case.entry), jstr(&case.orig),
        case.args.iter().map(|a| jstr(a)).collect::<Vec<_>>().join(","), ORIG_COMMIT, env!("CARGO_PKG_VERSION"), jstr(&o.repo), jstr(&o.tmp));
    w("meta.json", meta.as_bytes())
}

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
fn diff_outcomes(n: &Norm, g: &Outcome, a: &Outcome) -> Vec<String> {
    let mut m = vec![];
    for (name, ch, gb, ab) in [("stdout", Ch::Stdout, &g.stdout, &a.stdout), ("stderr", Ch::Stderr, &g.stderr, &a.stderr)] {
        let (x, y) = (n.text(ch, gb, &g.repo, &g.tmp), n.text(ch, ab, &a.repo, &a.tmp));
        if x != y { m.push(format!("{name}: {}", first_diff(&x, &y))); }
    }
    if g.exit != a.exit { m.push(format!("exit_code: golden={} actual={}", g.exit, a.exit)); }
    let gk: Vec<&String> = g.files.keys().collect(); let ak: Vec<&String> = a.files.keys().collect();
    if gk != ak { m.push(format!("files: path set golden={gk:?} actual={ak:?}")); }
    else { for (k, gb) in &g.files { let (x, y) = (n.text(Ch::File, gb, &g.repo, &g.tmp), n.text(Ch::File, &a.files[k], &a.repo, &a.tmp)); if x != y { m.push(format!("files/{k}: {}", first_diff(&x, &y))); } } }
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
struct Args { mode: String, stage: String, cases: Vec<String>, repo: PathBuf, raw: PathBuf, force: bool }

fn parse_args() -> Result<Args, String> {
    let v: Vec<String> = std::env::args().skip(1).collect();
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = tool.parent().and_then(|p| p.parent()).and_then(|p| p.parent()).map(PathBuf::from).unwrap_or_default();
    let mut a = Args { mode: String::new(), stage: "all".into(), cases: vec![], repo, raw: tool.join("raw"), force: false };
    let mut i = 0;
    while i < v.len() {
        match v[i].as_str() {
            "--self-check" => a.mode = "self-check".into(),
            "capture" | "orig" | "compare" => a.mode = v[i].clone(),
            "--force" => a.force = true,
            f @ ("--stage" | "--case" | "--repo" | "--raw-dir") => {
                i += 1; let val = v.get(i).ok_or(format!("{f} needs a value"))?.clone();
                match f { "--stage" => a.stage = val, "--case" => a.cases.push(val), "--repo" => a.repo = PathBuf::from(val), _ => a.raw = PathBuf::from(val) }
            }
            x => return Err(format!("unknown argument {x}")),
        }
        i += 1;
    }
    if a.mode.is_empty() { return Err("mode required: capture|orig|compare|--self-check".into()); }
    if a.stage != "all" && !STAGES.contains(&a.stage.as_str()) { return Err(format!("bad stage {}", a.stage)); }
    Ok(a)
}

fn real_main() -> Result<i32, String> {
    let a = parse_args()?;
    let tool = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if a.mode == "self-check" { return self_check(); }
    let norm = Norm::load(&tool.join("normalize.json"))?;
    let stages: Vec<&str> = if a.stage == "all" { STAGES.to_vec() } else { vec![a.stage.as_str()] };
    let mut cases = vec![];
    for s in &stages { cases.extend(load_cases(&tool, s)?); }
    if !a.cases.is_empty() { cases.retain(|c| a.cases.contains(&c.id)); }
    if cases.is_empty() { return Err("no cases selected (golden capture of cases is c03's job)".into()); }
    if a.mode == "compare" && Command::new("strace").arg("-V").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() { return Err("strace is not available".into()); }
    let mut bad = 0; let mut py = 0;
    for c in &cases {
        match a.mode.as_str() {
            "capture" => {
                if tool.join("data").join(&c.id).exists() && !a.force { return Err(format!("golden for {} exists (use --force)", c.id)); }
                if c.orig.is_empty() { return Err(format!("{}: case has no orig", c.id)); }
                let o = run(c, &c.orig, &a.repo, false, c.orig.ends_with(".py"))?;
                save_golden(&tool, c, &o)?;
            }
            _ => {
                let g = load_golden(&tool, &c.id)?;
                let o = if a.mode == "orig" { run(c, &c.orig, &a.repo, false, c.orig.ends_with(".py"))? } else { run(c, &c.entry, &a.repo, true, false)? };
                py += o.python_exec;
                let mut m = diff_outcomes(&norm, &g, &o);
                if a.mode == "compare" && o.python_exec > 0 { m.push(format!("python_exec={}", o.python_exec)); }
                if !m.is_empty() { bad += 1; write_raw(&a.raw, &c.id, &o); for x in m { println!("MISMATCH {} {}", c.id, x); } }
            }
        }
    }
    let n = cases.len();
    match a.mode.as_str() {
        "capture" => println!("PARITY-CAPTURE-OK cases={n}"),
        "orig" => if bad == 0 { println!("PARITY-ORIG-PASS cases={n}") } else { println!("orig: {bad}/{n} cases mismatch") },
        _ => if bad == 0 && py == 0 { println!("PARITY-{}-PASS python_exec=0", a.stage.to_uppercase()) } else { println!("compare: {bad}/{n} cases mismatch python_exec={py}") },
    }
    Ok(if bad == 0 { 0 } else { 1 })
}

// ---------------------------------------------------------------- self-check (no goldens)
fn self_check() -> Result<i32, String> {
    let mut fails: Vec<String> = vec![];
    let mut ck = |name: &str, ok: bool| { if !ok { fails.push(name.to_string()); } };
    let n = Norm { ids: KNOWN_IDS.iter().map(|s| s.to_string()).collect() };
    let t = |ch, s: &str| n.text(ch, s.as_bytes(), "/r/w", "/tmp/x1");
    ck("N01", t(Ch::Stdout, "a /r/w/b") == "a <REPO>/b");
    ck("N02", t(Ch::Stdout, "a /tmp/x1/f") == "a <TMP>/f");
    ck("N03-iso", t(Ch::Stdout, "at 2026-11-15T08:01:53.123Z ok") == "at <TS> ok");
    ck("N03-epoch", t(Ch::Stdout, "t=1763161313;") == "t=<TS>;" && t(Ch::Stdout, "n=12345678901") == "n=12345678901");
    ck("N04-stderr", t(Ch::Stderr, "pid=4242 x") == "pid=<PID> x");
    ck("N04-stdout-untouched", t(Ch::Stdout, "pid=4242 x") == "pid=4242 x");
    ck("N06", t(Ch::Stderr, "Traceback (most recent call last):\n  File \"/r/w/x.py\", line 3\n    boom()\nValueError: bad\n") == "ValueError: bad\n");
    ck("N06-plain-stderr-untouched", t(Ch::Stderr, "warn: x\n") == "warn: x\n");
    ck("closed-list", { let p = scratch("n", "self"); fs::create_dir_all(&p).ok(); let f = p.join("n.json"); fs::write(&f, "{\"normalizations\":[{\"id\":\"N99\"}]}").ok(); let r = Norm::load(&f).is_err(); let _ = fs::remove_dir_all(&p); r });
    // comparison: equal, then each channel differing
    let mk = |so: &str, se: &str, ex: i32, fv: &str| { let mut f = BTreeMap::new(); f.insert("o.txt".to_string(), fv.as_bytes().to_vec()); Outcome { stdout: so.into(), stderr: se.into(), exit: ex, files: f, python_exec: 0, repo: "/r/w".into(), tmp: "/tmp/x1".into() } };
    let g = mk("a\n", "", 0, "f");
    ck("cmp-equal", diff_outcomes(&n, &g, &mk("a\n", "", 0, "f")).is_empty());
    ck("cmp-stdout", !diff_outcomes(&n, &g, &mk("b\n", "", 0, "f")).is_empty());
    ck("cmp-stderr", !diff_outcomes(&n, &g, &mk("a\n", "e", 0, "f")).is_empty());
    ck("cmp-exit", !diff_outcomes(&n, &g, &mk("a\n", "", 1, "f")).is_empty());
    ck("cmp-file", !diff_outcomes(&n, &g, &mk("a\n", "", 0, "g")).is_empty());
    let mut other = mk("a\n", "", 0, "f"); other.repo = "/q/z".into(); other.tmp = "/tmp/y9".into(); other.stdout = b"a\n".to_vec();
    let mut g2 = mk("p /r/w/a\n", "", 0, "f"); g2.stdout = b"p /r/w/a\n".to_vec(); other.stdout = b"p /q/z/a\n".to_vec();
    ck("cmp-roots-normalized", diff_outcomes(&n, &g2, &other).is_empty());
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
    // end-to-end: synthetic case run through the same machinery
    let case = Case { id: "synth".into(), stage: "s1".into(), entry: "/bin/sh".into(), orig: "/bin/sh".into(), args: vec!["-c".into(), "echo out; echo err >&2; echo data > o.txt; exit 3".into()], stdin: None, setup: vec![], env: vec![], outputs: vec!["o.txt".into()] };
    match (run(&case, "/bin/sh", Path::new("/"), false, false), run(&case, "/bin/sh", Path::new("/"), true, false)) {
        (Ok(a), Ok(b)) => { ck("e2e-exit", a.exit == 3 && b.exit == 3); ck("e2e-equal", diff_outcomes(&n, &a, &b).is_empty() && b.python_exec == 0); ck("e2e-file", a.files.get("o.txt").map(|x| x.as_slice()) == Some(b"data\n".as_slice())); }
        _ => ck("e2e-run", false),
    }
    if fails.is_empty() { println!("PARITY-SELFCHECK-OK"); Ok(0) } else { for f in &fails { println!("SELFCHECK-FAIL {f}"); } Ok(1) }
}

fn main() {
    match real_main() { Ok(c) => std::process::exit(c), Err(e) => { eprintln!("parity: {e}"); std::process::exit(2) } }
}
