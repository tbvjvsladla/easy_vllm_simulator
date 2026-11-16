//! Small Python-semantics helpers private to the CLI port (equality, strip, int(), basename, tree copy, diff).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::core::{self, HintError, HR};
use crate::value::J;

/// `str.isspace()` for one character (Python's whitespace set: Rust's plus `\x1c`..`\x1f`).
pub fn py_space(c: char) -> bool {
    c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c)
}

/// `str.strip()`.
pub fn py_strip(s: &str) -> &str {
    s.trim_matches(py_space)
}

/// `str.rstrip()`.
pub fn py_rstrip(s: &str) -> &str {
    s.trim_end_matches(py_space)
}

/// `str.casefold()` (lowercase + the common full folds).
pub fn casefold(s: &str) -> String {
    s.to_lowercase().replace('ß', "ss").replace('ς', "σ")
}

/// Python `==` between two JSON values (dict order-insensitive; `1 == 1.0 == True`).
pub fn py_eq(a: &J, b: &J) -> bool {
    fn num(v: &J) -> Option<f64> {
        match v {
            J::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            J::Int(i) => Some(*i as f64),
            J::Float(f) => Some(*f),
            _ => None,
        }
    }
    match (a, b) {
        (J::Null, J::Null) => true,
        (J::Str(x), J::Str(y)) => x == y,
        (J::List(x), J::List(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| py_eq(p, q)),
        (J::Dict(x), J::Dict(y)) => x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).map(|w| py_eq(v, w)).unwrap_or(false)),
        (J::Int(x), J::Int(y)) => x == y,
        _ => match (num(a), num(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        },
    }
}

/// `v == n` for an int constant.
pub fn eq_int(v: Option<&J>, n: i64) -> bool {
    v.map(|v| py_eq(v, &J::Int(n))).unwrap_or(false)
}

/// `isinstance(v, int) and not isinstance(v, bool)`.
pub fn is_int(v: &J) -> Option<i64> {
    match v {
        J::Int(i) => Some(*i),
        _ => None,
    }
}

/// `int(x or 0)` for the JSON shapes the payload writers use.
pub fn py_int(v: &J) -> i64 {
    match v {
        J::Int(i) => *i,
        J::Bool(b) => *b as i64,
        J::Float(f) => f.trunc() as i64,
        J::Str(s) => py_strip(s).parse().unwrap_or(0),
        _ => 0,
    }
}

/// `str(x or "")`.
pub fn disp_or_empty(v: &J) -> String {
    if v.truthy() {
        v.display()
    } else {
        String::new()
    }
}

/// `repr(tuple_of_str)`.
pub fn tuple_repr(items: &[&str]) -> String {
    let parts: Vec<String> = items.iter().map(|s| crate::value::py_repr(s)).collect();
    if parts.len() == 1 {
        format!("({},)", parts[0])
    } else {
        format!("({})", parts.join(", "))
    }
}

/// `repr(list_of_str)`.
pub fn list_repr<S: AsRef<str>>(items: &[S]) -> String {
    let parts: Vec<String> = items.iter().map(|s| crate::value::py_repr(s.as_ref())).collect();
    format!("[{}]", parts.join(", "))
}

/// `Path(s).name`.
pub fn py_basename(s: &str) -> String {
    s.split('/').filter(|p| !p.is_empty() && *p != ".").last().unwrap_or("").to_string()
}

/// `f"{n:,}"`.
pub fn thousands(n: i64) -> String {
    let s = n.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    if n < 0 {
        format!("-{out}")
    } else {
        out
    }
}

/// Python's exception type name for a read failure (`Path.read_text`).
pub fn read_text_typed(path: &Path) -> Result<String, (String, String)> {
    match std::fs::read(path) {
        Err(e) => {
            let ty = match e.raw_os_error() {
                Some(2) => "FileNotFoundError",
                Some(13) | Some(1) => "PermissionError",
                Some(21) => "IsADirectoryError",
                Some(20) => "NotADirectoryError",
                _ => "OSError",
            };
            Err((ty.to_string(), core::io_err_text(&e, path)))
        }
        Ok(bytes) => String::from_utf8(bytes).map_err(|e| {
            let pos = e.utf8_error().valid_up_to();
            let b = e.as_bytes().get(pos).copied().unwrap_or(0);
            ("UnicodeDecodeError".to_string(), format!("'utf-8' codec can't decode byte 0x{b:02x} in position {pos}: invalid start byte"))
        }),
    }
}

/// `json.loads(path.read_text(encoding="utf-8"))` with the Python exception (type name, text) on failure.
pub fn load_json_typed(path: &Path) -> Result<J, (String, String)> {
    let text = read_text_typed(path)?;
    crate::value::loads(&text).map_err(|e| ("JSONDecodeError".to_string(), e))
}

/// Files under `root` (recursive, following symlinked files like `Path.rglob("*")` + `is_file()`), as posix paths
/// relative to `base`.
pub fn files_under(root: &Path, base: &Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(d: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(d) else { return };
        for e in rd.flatten() {
            let p = e.path();
            let is_link = e.file_type().map(|t| t.is_symlink()).unwrap_or(false);
            if p.is_dir() && !is_link {
                walk(&p, base, out);
            } else if p.is_file() {
                if let Ok(r) = p.strip_prefix(base) {
                    out.push(core::posix(r));
                }
            }
        }
    }
    if root.is_dir() {
        walk(root, base, &mut out);
    }
    out
}

/// `shutil.copytree(src, dst)` (files copied by content, directories recreated).
pub fn copy_tree(src: &Path, dst: &Path) -> HR<()> {
    std::fs::create_dir_all(dst).map_err(|e| core::io_err(&e, dst))?;
    let rd = std::fs::read_dir(src).map_err(|e| core::io_err(&e, src))?;
    for e in rd.flatten() {
        let p = e.path();
        let q = dst.join(e.file_name());
        if p.is_dir() {
            copy_tree(&p, &q)?;
        } else {
            std::fs::copy(&p, &q).map_err(|e| core::io_err(&e, &p))?;
        }
    }
    Ok(())
}

/// `shutil.rmtree(p)`.
pub fn rmtree(p: &Path) -> HR<()> {
    std::fs::remove_dir_all(p).map_err(|e| core::io_err(&e, p))
}

/// `tempfile.TemporaryDirectory(prefix=..., dir=...)` inside a given directory; removed on drop.
pub struct TempIn(pub PathBuf);

impl TempIn {
    pub fn new(dir: &Path, prefix: &str) -> HR<TempIn> {
        static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let p = dir.join(format!("{prefix}{}{n}{nanos:x}", std::process::id()));
        std::fs::create_dir_all(&p).map_err(|e| core::io_err(&e, &p))?;
        Ok(TempIn(p))
    }
}

impl Drop for TempIn {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Python built-in exception escaping (`TypeError` …).
pub fn py_err(ty: &str, msg: impl Into<String>) -> HintError {
    HintError::py(ty, msg)
}

// ── difflib.unified_diff(a, b, fromfile, tofile, lineterm="", n=N) ──────────────────────────────
struct Matcher<'a> {
    a: &'a [String],
    b: &'a [String],
    b2j: HashMap<&'a str, Vec<usize>>,
}

impl<'a> Matcher<'a> {
    fn new(a: &'a [String], b: &'a [String]) -> Matcher<'a> {
        let mut b2j: HashMap<&'a str, Vec<usize>> = HashMap::new();
        for (i, e) in b.iter().enumerate() {
            b2j.entry(e.as_str()).or_default().push(i);
        }
        let n = b.len();
        if n >= 200 {
            let ntest = n / 100 + 1;
            b2j.retain(|_, v| v.len() <= ntest);
        }
        Matcher { a, b, b2j }
    }

    fn find_longest_match(&self, alo: usize, ahi: usize, blo: usize, bhi: usize) -> (usize, usize, usize) {
        let (a, b) = (self.a, self.b);
        let (mut besti, mut bestj, mut bestsize) = (alo, blo, 0usize);
        let mut j2len: HashMap<usize, usize> = HashMap::new();
        for i in alo..ahi {
            let mut newj2len: HashMap<usize, usize> = HashMap::new();
            if let Some(js) = self.b2j.get(a[i].as_str()) {
                for &j in js {
                    if j < blo {
                        continue;
                    }
                    if j >= bhi {
                        break;
                    }
                    let k = j.checked_sub(1).and_then(|p| j2len.get(&p)).copied().unwrap_or(0) + 1;
                    newj2len.insert(j, k);
                    if k > bestsize {
                        besti = i + 1 - k;
                        bestj = j + 1 - k;
                        bestsize = k;
                    }
                }
            }
            j2len = newj2len;
        }
        while besti > alo && bestj > blo && a[besti - 1] == b[bestj - 1] {
            besti -= 1;
            bestj -= 1;
            bestsize += 1;
        }
        while besti + bestsize < ahi && bestj + bestsize < bhi && a[besti + bestsize] == b[bestj + bestsize] {
            bestsize += 1;
        }
        (besti, bestj, bestsize)
    }

    fn matching_blocks(&self) -> Vec<(usize, usize, usize)> {
        let (la, lb) = (self.a.len(), self.b.len());
        let mut queue = vec![(0, la, 0, lb)];
        let mut blocks = Vec::new();
        while let Some((alo, ahi, blo, bhi)) = queue.pop() {
            let (i, j, k) = self.find_longest_match(alo, ahi, blo, bhi);
            if k > 0 {
                blocks.push((i, j, k));
                if alo < i && blo < j {
                    queue.push((alo, i, blo, j));
                }
                if i + k < ahi && j + k < bhi {
                    queue.push((i + k, ahi, j + k, bhi));
                }
            }
        }
        blocks.sort();
        let (mut i1, mut j1, mut k1) = (0, 0, 0);
        let mut out = Vec::new();
        for (i2, j2, k2) in blocks {
            if i1 + k1 == i2 && j1 + k1 == j2 {
                k1 += k2;
            } else {
                if k1 > 0 {
                    out.push((i1, j1, k1));
                }
                (i1, j1, k1) = (i2, j2, k2);
            }
        }
        if k1 > 0 {
            out.push((i1, j1, k1));
        }
        out.push((la, lb, 0));
        out
    }

    fn opcodes(&self) -> Vec<(&'static str, usize, usize, usize, usize)> {
        let (mut i, mut j) = (0, 0);
        let mut ans = Vec::new();
        for (ai, bj, size) in self.matching_blocks() {
            let tag = if i < ai && j < bj {
                "replace"
            } else if i < ai {
                "delete"
            } else if j < bj {
                "insert"
            } else {
                ""
            };
            if !tag.is_empty() {
                ans.push((tag, i, ai, j, bj));
            }
            i = ai + size;
            j = bj + size;
            if size > 0 {
                ans.push(("equal", ai, i, bj, j));
            }
        }
        ans
    }

    fn grouped_opcodes(&self, n: usize) -> Vec<Vec<(&'static str, usize, usize, usize, usize)>> {
        let mut codes = self.opcodes();
        if codes.is_empty() {
            codes = vec![("equal", 0, 1, 0, 1)];
        }
        if codes[0].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[0];
            codes[0] = (t, i1.max(i2.saturating_sub(n)), i2, j1.max(j2.saturating_sub(n)), j2);
        }
        let last = codes.len() - 1;
        if codes[last].0 == "equal" {
            let (t, i1, i2, j1, j2) = codes[last];
            codes[last] = (t, i1, i2.min(i1 + n), j1, j2.min(j1 + n));
        }
        let nn = n + n;
        let mut groups = Vec::new();
        let mut group = Vec::new();
        for (tag, mut i1, i2, mut j1, j2) in codes {
            if tag == "equal" && i2 - i1 > nn {
                group.push((tag, i1, i2.min(i1 + n), j1, j2.min(j1 + n)));
                groups.push(std::mem::take(&mut group));
                i1 = i1.max(i2.saturating_sub(n));
                j1 = j1.max(j2.saturating_sub(n));
            }
            group.push((tag, i1, i2, j1, j2));
        }
        if !group.is_empty() && !(group.len() == 1 && group[0].0 == "equal") {
            groups.push(group);
        }
        groups
    }
}

fn format_range_unified(start: usize, stop: usize) -> String {
    let mut beginning = start + 1;
    let length = stop - start;
    if length == 1 {
        return beginning.to_string();
    }
    if length == 0 {
        beginning -= 1;
    }
    format!("{beginning},{length}")
}

pub fn unified_diff(a: &[String], b: &[String], fromfile: &str, tofile: &str, n: usize) -> Vec<String> {
    let m = Matcher::new(a, b);
    let mut out = Vec::new();
    let mut started = false;
    for group in m.grouped_opcodes(n) {
        if !started {
            started = true;
            out.push(format!("--- {fromfile}"));
            out.push(format!("+++ {tofile}"));
        }
        let (first, last) = (group[0], group[group.len() - 1]);
        out.push(format!("@@ -{} +{} @@", format_range_unified(first.1, last.2), format_range_unified(first.3, last.4)));
        for (tag, i1, i2, j1, j2) in group {
            if tag == "equal" {
                for l in &a[i1..i2] {
                    out.push(format!(" {l}"));
                }
                continue;
            }
            if tag == "replace" || tag == "delete" {
                for l in &a[i1..i2] {
                    out.push(format!("-{l}"));
                }
            }
            if tag == "replace" || tag == "insert" {
                for l in &b[j1..j2] {
                    out.push(format!("+{l}"));
                }
            }
        }
    }
    out
}
