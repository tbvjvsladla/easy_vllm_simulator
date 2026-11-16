//! The regular expressions of `render_sub_env.py`, hand-matched with CPython `re` semantics (leftmost match, greedy and
//! lazy quantifiers with backtracking, `re.sub` / `re.findall` scanning). Only these exact patterns are implemented.

use crate::py::is_space;

fn chars(s: &str) -> Vec<char> {
    s.chars().collect()
}

fn skip_ws(c: &[char], mut i: usize) -> usize {
    while i < c.len() && is_space(c[i]) {
        i += 1;
    }
    i
}

fn skip_blank(c: &[char], mut i: usize) -> usize {
    while i < c.len() && (c[i] == ' ' || c[i] == '\t') {
        i += 1;
    }
    i
}

fn lit(c: &[char], i: usize, s: &str) -> Option<usize> {
    let mut j = i;
    for ch in s.chars() {
        if c.get(j) != Some(&ch) {
            return None;
        }
        j += 1;
    }
    Some(j)
}

fn is_mode_char(ch: char) -> bool {
    ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-'
}

/// `MODE_BLOCK_RE` at `i`: `[ \t]*<!--\s*MODE:(?P<mode>[a-z0-9-]+)\s*-->\n(?P<body>.*?)[ \t]*<!--\s*/MODE:(?P=mode)\s*-->\n?`
/// (DOTALL). Returns (end, mode, body).
fn mode_block_at(c: &[char], i: usize) -> Option<(usize, String, String)> {
    let j = lit(c, skip_blank(c, i), "<!--")?;
    let j = lit(c, skip_ws(c, j), "MODE:")?;
    let mut mend = j;
    while mend < c.len() && is_mode_char(c[mend]) {
        mend += 1;
    }
    let mut l = mend;
    while l > j {
        let mode: String = c[j..l].iter().collect();
        if let Some(k) = lit(c, skip_ws(c, l), "-->\n") {
            // lazy body: the earliest closing tag
            let mut b = k;
            while b <= c.len() {
                if let Some(e) = close_at(c, b, &mode) {
                    let body: String = c[k..b].iter().collect();
                    return Some((e, mode, body));
                }
                b += 1;
            }
        }
        l -= 1;
    }
    None
}

fn close_at(c: &[char], b: usize, mode: &str) -> Option<usize> {
    let j = lit(c, skip_blank(c, b), "<!--")?;
    let j = lit(c, skip_ws(c, j), "/MODE:")?;
    let j = lit(c, j, mode)?;
    let j = lit(c, skip_ws(c, j), "-->")?;
    Some(if c.get(j) == Some(&'\n') { j + 1 } else { j })
}

/// `MODE_BLOCK_RE.sub(f, text)` where `f(mode, body)` returns the replacement or an error.
pub fn sub_mode_blocks<E>(
    text: &str,
    mut f: impl FnMut(&str, &str) -> Result<String, E>,
) -> Result<String, E> {
    let c = chars(text);
    let mut out = String::new();
    let mut i = 0;
    while i <= c.len() {
        if let Some((e, mode, body)) = mode_block_at(&c, i) {
            out.push_str(&f(&mode, &body)?);
            if e == i {
                if i < c.len() {
                    out.push(c[i]);
                }
                i += 1;
            } else {
                i = e;
            }
            continue;
        }
        if i < c.len() {
            out.push(c[i]);
        }
        i += 1;
    }
    Ok(out)
}

/// `TEMPLATE_HEADER_RE.sub("", text)`: `<!--\s*TEMPLATE-ONLY:START\s*-->.*?<!--\s*TEMPLATE-ONLY:END\s*-->\s*` (DOTALL).
pub fn strip_template_header(text: &str) -> String {
    let c = chars(text);
    let start_at = |i: usize| -> Option<usize> {
        let j = lit(&c, i, "<!--")?;
        let j = lit(&c, skip_ws(&c, j), "TEMPLATE-ONLY:START")?;
        lit(&c, skip_ws(&c, j), "-->")
    };
    let end_at = |i: usize| -> Option<usize> {
        let j = lit(&c, i, "<!--")?;
        let j = lit(&c, skip_ws(&c, j), "TEMPLATE-ONLY:END")?;
        let j = lit(&c, skip_ws(&c, j), "-->")?;
        Some(skip_ws(&c, j))
    };
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if let Some(k) = start_at(i) {
            let mut b = k;
            let mut found = None;
            while b < c.len() {
                if let Some(e) = end_at(b) {
                    found = Some(e);
                    break;
                }
                b += 1;
            }
            if let Some(e) = found {
                i = e;
                continue;
            }
        }
        out.push(c[i]);
        i += 1;
    }
    out
}

/// `MODE_PREFIX_RE.match(item)`: `^MODE:(?P<mode>[a-z0-9-]+)\s+(?P<rest>.+)$` (DOTALL) → (mode, rest).
pub fn mode_prefix(item: &str) -> Option<(String, String)> {
    let c = chars(item);
    let j = lit(&c, 0, "MODE:")?;
    let mut m = j;
    while m < c.len() && is_mode_char(c[m]) {
        m += 1;
    }
    if m == j {
        return None;
    }
    let mut w = m;
    while w < c.len() && is_space(c[w]) {
        w += 1;
    }
    if w == m {
        return None;
    }
    // `.+` needs one character: the greedy `\s+` gives one back when only whitespace remains
    let rest_start = if w < c.len() {
        w
    } else if w - m >= 2 {
        w - 1
    } else {
        return None;
    };
    // `$` (DOTALL `.+` greedy to the end) always succeeds at the end
    Some((c[j..m].iter().collect(), c[rest_start..].iter().collect()))
}

/// `PLACEHOLDER_RE` = `\{\{\s*([A-Z_]+)\s*\}\}`: every match as (start, end, key), left to right, non-overlapping.
pub fn placeholders(text: &str) -> Vec<(usize, usize, String)> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        let m = (|| {
            let j = lit(&c, i, "{{")?;
            let k0 = skip_ws(&c, j);
            let mut k = k0;
            while k < c.len() && (c[k].is_ascii_uppercase() || c[k] == '_') {
                k += 1;
            }
            if k == k0 {
                return None;
            }
            let e = lit(&c, skip_ws(&c, k), "}}")?;
            Some((e, c[k0..k].iter().collect::<String>()))
        })();
        if let Some((e, key)) = m {
            out.push((i, e, key));
            i = e;
        } else {
            i += 1;
        }
    }
    out
}

/// `PLACEHOLDER_RE.sub(lambda m: str(ph.get(key, m.group(0))), text)`.
pub fn render_text(text: &str, get: impl Fn(&str) -> Option<String>) -> String {
    let c = chars(text);
    let mut out = String::new();
    let mut last = 0;
    for (s, e, key) in placeholders(text) {
        out.extend(c[last..s].iter());
        match get(&key) {
            Some(v) => out.push_str(&v),
            None => out.extend(c[s..e].iter()),
        }
        last = e;
    }
    out.extend(c[last..].iter());
    out
}

fn class_a(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '.' || ch == '/' || ch == '-'
}

/// `re.findall(r'\$REPO/(\.claude/[A-Za-z0-9_./-]+)', text)`.
pub fn find_repo_refs(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some(j) = lit(&c, i, "$REPO/.claude/") {
            let mut k = j;
            while k < c.len() && class_a(c[k]) {
                k += 1;
            }
            if k > j {
                let start = i + "$REPO/".len();
                out.push(c[start..k].iter().collect());
                i = k;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `re.findall(r'\$SDIR/\.\./([A-Za-z0-9_-][A-Za-z0-9_./-]*\.[A-Za-z0-9]+)', text)`.
pub fn find_sdir_parent_refs(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some(j) = lit(&c, i, "$SDIR/../") {
            if j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '_' || c[j] == '-') {
                let mut run = j + 1;
                while run < c.len() && class_a(c[run]) {
                    run += 1;
                }
                // backtrack: the last '.' (at index > j) followed by at least one alnum
                let mut d = run;
                let mut hit = None;
                while d > j + 1 {
                    d -= 1;
                    if c[d] == '.' && d + 1 < c.len() && c[d + 1].is_ascii_alphanumeric() {
                        let mut e = d + 1;
                        while e < c.len() && c[e].is_ascii_alphanumeric() {
                            e += 1;
                        }
                        hit = Some(e);
                        break;
                    }
                }
                if let Some(e) = hit {
                    out.push(c[j..e].iter().collect());
                    i = e;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// `re.findall(r'\$SDIR/([A-Za-z0-9_-]+\.(?:sh|py))', text)`.
pub fn find_sdir_scripts(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some(j) = lit(&c, i, "$SDIR/") {
            let mut k = j;
            while k < c.len() && (c[k].is_ascii_alphanumeric() || c[k] == '_' || c[k] == '-') {
                k += 1;
            }
            if k > j {
                if let Some(e) = lit(&c, k, ".sh").or_else(|| lit(&c, k, ".py")) {
                    out.push(c[j..e].iter().collect());
                    i = e;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// `re.findall(r'skill_source\("([^"]+)"\)', text)`.
pub fn find_skill_sources(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some(j) = lit(&c, i, "skill_source(\"") {
            let mut k = j;
            while k < c.len() && c[k] != '"' {
                k += 1;
            }
            if k > j {
                if let Some(e) = lit(&c, k, "\")") {
                    out.push(c[j..k].iter().collect());
                    i = e;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// `re.compile(r'^REL_[A-Z0-9_]+\s*=\s*"([^"]+)"', re.M).finditer(text)` group 1.
pub fn find_rel_constants(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        let bol = i == 0 || c[i - 1] == '\n';
        if bol {
            let m = (|| {
                let j = lit(&c, i, "REL_")?;
                let mut k = j;
                while k < c.len() && (c[k].is_ascii_uppercase() || c[k].is_ascii_digit() || c[k] == '_') {
                    k += 1;
                }
                if k == j {
                    return None;
                }
                let k = lit(&c, skip_ws(&c, k), "=")?;
                let k = lit(&c, skip_ws(&c, k), "\"")?;
                let mut e = k;
                while e < c.len() && c[e] != '"' {
                    e += 1;
                }
                if e == k || e >= c.len() {
                    return None;
                }
                Some((e + 1, c[k..e].iter().collect::<String>()))
            })();
            if let Some((e, v)) = m {
                out.push(v);
                i = e;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// `re.findall(r'_load_schema\("([^"]+)"\)', text)`.
pub fn find_load_schema(text: &str) -> Vec<String> {
    let c = chars(text);
    let mut out = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some(j) = lit(&c, i, "_load_schema(\"") {
            let mut k = j;
            while k < c.len() && c[k] != '"' {
                k += 1;
            }
            if k > j {
                if let Some(e) = lit(&c, k, "\")") {
                    out.push(c[j..k].iter().collect());
                    i = e;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// The values of the line-initial Rust string constants `[pub ]const REL_<NAME>: &str = "<value>";` (the Rust form of the
/// `REL_*` path constants [`find_rel_constants`] reads from the retired Python sources).
pub fn find_rust_rel_constants(text: &str) -> Vec<String> {
    let mut out = vec![];
    for line in text.lines() {
        let rest = line.strip_prefix("pub const REL_").or_else(|| line.strip_prefix("const REL_"));
        let Some(rest) = rest else { continue };
        let name_len = rest.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_').count();
        if name_len == 0 {
            continue;
        }
        let Some(rest) = rest[name_len..].trim_start().strip_prefix(':') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix("&str") else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('=') else { continue };
        let Some(rest) = rest.trim_start().strip_prefix('"') else { continue };
        if let Some(end) = rest.find('"') {
            if end > 0 {
                out.push(rest[..end].to_string());
            }
        }
    }
    out
}

/// The schema names a Rust owner loads: the first string literal inside each `load_schema(` call (`load_schema(&dir,
/// "work-manifest.schema.json")`).
pub fn find_rust_load_schema(text: &str) -> Vec<String> {
    let mut out = vec![];
    let mut rest = text;
    while let Some(at) = rest.find("load_schema(") {
        let after = &rest[at + "load_schema(".len()..];
        let call = &after[..after.find(')').unwrap_or(after.len())];
        if let Some(q) = call.find('"') {
            if let Some(e) = call[q + 1..].find('"') {
                let name = &call[q + 1..q + 1 + e];
                if name.ends_with(".json") {
                    out.push(name.to_string());
                }
            }
        }
        rest = after;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_blocks() {
        let t = "a\n  <!-- MODE:x -->\nX\n  <!-- /MODE:x -->\nb<!--MODE:y-->\nY\n<!-- /MODE:y -->c";
        let r = sub_mode_blocks::<()>(t, |m, b| Ok(if m == "x" { b.to_string() } else { String::new() })).unwrap();
        assert_eq!(r, "a\nX\nbc");
    }

    #[test]
    fn prefix() {
        assert_eq!(mode_prefix("MODE:a2a-agent Edit(x)"), Some(("a2a-agent".into(), "Edit(x)".into())));
        assert_eq!(mode_prefix("MODE:x"), None);
        assert_eq!(mode_prefix("MODE:x  "), Some(("x".into(), " ".into())));
    }

    #[test]
    fn regex_scans() {
        assert_eq!(find_sdir_parent_refs("$SDIR/../rust/a.b.rs-x"), vec!["rust/a.b.rs"]);
        assert_eq!(find_sdir_scripts("$SDIR/run.shx $SDIR/a.txt $SDIR/b.py"), vec!["run.sh", "b.py"]);
        assert_eq!(find_rel_constants("REL_A = \"x/y\"\n  REL_B = \"z\"\nREL_C=\"w\""), vec!["x/y", "w"]);
        assert_eq!(placeholders("{{ A_B }} {{x}} {{C}}").len(), 2);
    }
}
