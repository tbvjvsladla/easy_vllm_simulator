//! `yaml11_oracle` — independent YAML 1.1 (PyYAML) oracle for wiki-desk `checks/frontmatter.check.ts` (plan_26111707).
//!
//! The check used to start host `python3 -c 'import yaml'` to ask "does a YAML 1.1 loader read every emitted key/value back
//! as a string?". The harness is Rust (constitution), so this binary answers the same question without Python.
//!
//! What it reproduces: PyYAML 6.0.1 `yaml/resolver.py` implicit resolvers (bool·float·int·merge·null·timestamp·value) for
//! plain scalars, plus single-/double-quoted scalars (always `str`). The regular expressions are quoted verbatim below and
//! ported to hand matchers (std only — no regex crate, so every node can build it offline).
//!
//! Limits (deliberate): input is a **flat top-level mapping**, one `key: value` per line (blank lines, `#` comments and a
//! leading `---` allowed). Anything else — indentation, flow/block collections, anchors, tags, multi-line scalars — is
//! exit 3 with the reason (fail-loud; no guessing). It reproduces PyYAML on that subset; it does not replace PyYAML.
//!
//! Output (stdout): JSON `[[key, value], …]` in first-insertion order (Python dict semantics: a repeated key keeps its
//! first position and takes the last value). A scalar that does not resolve to `str` is printed as `"!!<tag> <raw>"`.
//! `--self-test` checks the resolver table and the quoted-scalar decoder.
use std::io::Read;
use std::process::ExitCode;

/// Tag a PyYAML plain scalar resolves to (`None` = `str`). Order = resolver registration order in resolver.py.
fn resolve_plain(s: &str) -> Option<&'static str> {
    let b = s.as_bytes();
    if is_bool(s) { return Some("bool"); }
    if is_float(b) { return Some("float"); }
    if is_int(b) { return Some("int"); }
    if s == "<<" { return Some("merge"); } // ^(?:<<)$
    if is_null(s) { return Some("null"); }
    if is_timestamp(b) { return Some("timestamp"); }
    if s == "=" { return Some("value"); } // ^(?:=)$
    None
}

// ^(?:yes|Yes|YES|no|No|NO|true|True|TRUE|false|False|FALSE|on|On|ON|off|Off|OFF)$   (re.X)
fn is_bool(s: &str) -> bool {
    matches!(s, "yes" | "Yes" | "YES" | "no" | "No" | "NO" | "true" | "True" | "TRUE" | "false" | "False" | "FALSE"
        | "on" | "On" | "ON" | "off" | "Off" | "OFF")
}

// ^(?: ~ |null|Null|NULL | )$   (re.X — whitespace is dropped, so the last alternative is the empty string)
fn is_null(s: &str) -> bool { matches!(s, "~" | "null" | "Null" | "NULL" | "") }

fn sign(b: &[u8], i: usize) -> usize { if i < b.len() && (b[i] == b'-' || b[i] == b'+') { i + 1 } else { i } }
fn span(b: &[u8], mut i: usize, ok: impl Fn(u8) -> bool) -> usize { while i < b.len() && ok(b[i]) { i += 1; } i }
fn dig(c: u8) -> bool { c.is_ascii_digit() }
fn dig_(c: u8) -> bool { c.is_ascii_digit() || c == b'_' }

/// `(?:[eE][-+][0-9]+)?` then end of input.
fn opt_exp_end(b: &[u8], i: usize) -> bool {
    if i == b.len() { return true; }
    if !(b[i] == b'e' || b[i] == b'E') { return false; }
    let j = i + 1;
    if j >= b.len() || !(b[j] == b'-' || b[j] == b'+') { return false; }
    let k = span(b, j + 1, dig);
    k > j + 1 && k == b.len()
}

/// `(?::[0-5]?[0-9])+` — returns the index after the last group (None if zero groups).
fn sexagesimal(b: &[u8], mut i: usize) -> Option<usize> {
    let mut groups = 0;
    while i < b.len() && b[i] == b':' {
        let j = i + 1;
        if j + 1 < b.len() && (b'0'..=b'5').contains(&b[j]) && dig(b[j + 1]) { i = j + 2; }
        else if j < b.len() && dig(b[j]) { i = j + 1; }
        else { break; }
        groups += 1;
    }
    if groups > 0 { Some(i) } else { None }
}

// ^(?:[-+]?(?:[0-9][0-9_]*)\.[0-9_]*(?:[eE][-+][0-9]+)?
//    |\.[0-9][0-9_]*(?:[eE][-+][0-9]+)?
//    |[-+]?[0-9][0-9_]*(?::[0-5]?[0-9])+\.[0-9_]*
//    |[-+]?\.(?:inf|Inf|INF)
//    |\.(?:nan|NaN|NAN))$   (re.X)
fn is_float(b: &[u8]) -> bool {
    // alt 1
    let i = sign(b, 0);
    if i < b.len() && dig(b[i]) {
        let j = span(b, i + 1, dig_);
        if j < b.len() && b[j] == b'.' && opt_exp_end(b, span(b, j + 1, dig_)) { return true; }
        // alt 3
        if let Some(k) = sexagesimal(b, j) {
            if k < b.len() && b[k] == b'.' && span(b, k + 1, dig_) == b.len() { return true; }
        }
    }
    // alt 2
    if !b.is_empty() && b[0] == b'.' && b.len() > 1 && dig(b[1]) && opt_exp_end(b, span(b, 2, dig_)) { return true; }
    // alt 4
    if i < b.len() && b[i] == b'.' && matches!(&b[i + 1..], b"inf" | b"Inf" | b"INF") { return true; }
    // alt 5
    matches!(b, b".nan" | b".NaN" | b".NAN")
}

// ^(?:[-+]?0b[0-1_]+
//    |[-+]?0[0-7_]+
//    |[-+]?(?:0|[1-9][0-9_]*)
//    |[-+]?0x[0-9a-fA-F_]+
//    |[-+]?[1-9][0-9_]*(?::[0-5]?[0-9])+)$   (re.X)
fn is_int(b: &[u8]) -> bool {
    let i = sign(b, 0);
    let r = &b[i..];
    if r.is_empty() { return false; }
    let all = |s: &[u8], ok: fn(u8) -> bool| !s.is_empty() && s.iter().all(|&c| ok(c));
    if r.starts_with(b"0b") && all(&r[2..], |c| c == b'0' || c == b'1' || c == b'_') { return true; }
    if r[0] == b'0' && all(&r[1..], |c| (b'0'..=b'7').contains(&c) || c == b'_') { return true; }
    if r == b"0" { return true; }
    if (b'1'..=b'9').contains(&r[0]) && r[1..].iter().all(|&c| dig_(c)) { return true; }
    if r.starts_with(b"0x") && all(&r[2..], |c| c.is_ascii_hexdigit() || c == b'_') { return true; }
    if (b'1'..=b'9').contains(&r[0]) {
        let j = span(r, 1, dig_);
        if sexagesimal(r, j) == Some(r.len()) { return true; }
    }
    false
}

// ^(?:[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]
//    |[0-9][0-9][0-9][0-9] -[0-9][0-9]? -[0-9][0-9]?
//     (?:[Tt]|[ \t]+)[0-9][0-9]?
//     :[0-9][0-9] :[0-9][0-9] (?:\.[0-9]*)?
//     (?:[ \t]*(?:Z|[-+][0-9][0-9]?(?::[0-9][0-9])?))?)$   (re.X — spaces outside [] are dropped)
fn is_timestamp(b: &[u8]) -> bool {
    let d = |i: usize| i < b.len() && dig(b[i]);
    if b.len() == 10 && (0..4).all(d) && b[4] == b'-' && d(5) && d(6) && b[7] == b'-' && d(8) && d(9) { return true; }
    // alt 2
    if !(0..4).all(d) || b.len() < 5 || b[4] != b'-' { return false; }
    let one_two = |i: usize| -> Option<usize> { if !d(i) { None } else if d(i + 1) { Some(i + 2) } else { Some(i + 1) } };
    let Some(mut i) = one_two(5) else { return false };
    if i >= b.len() || b[i] != b'-' { return false; }
    let Some(j) = one_two(i + 1) else { return false };
    i = j;
    if i < b.len() && (b[i] == b'T' || b[i] == b't') { i += 1; }
    else {
        let k = span(b, i, |c| c == b' ' || c == b'\t');
        if k == i { return false; }
        i = k;
    }
    let Some(j) = one_two(i) else { return false };
    i = j;
    for _ in 0..2 {
        if !(i < b.len() && b[i] == b':' && d(i + 1) && d(i + 2)) { return false; }
        i += 3;
    }
    if i < b.len() && b[i] == b'.' { i = span(b, i + 1, dig); }
    if i == b.len() { return true; }
    let k = span(b, i, |c| c == b' ' || c == b'\t');
    if k < b.len() && b[k] == b'Z' { return k + 1 == b.len(); }
    if k < b.len() && (b[k] == b'-' || b[k] == b'+') {
        let Some(mut m) = one_two(k + 1) else { return false };
        if m < b.len() && b[m] == b':' && d(m + 1) && d(m + 2) { m += 3; }
        return m == b.len();
    }
    false
}

/// A decoded scalar: `(text, quoted)`.
type Scalar = (String, bool);

fn err(line: usize, msg: &str) -> String { format!("line {line}: {msg}") }

/// Double-quoted scalar starting at `s[0] == '"'`; returns (text, rest-after-closing-quote).
fn double_quoted(s: &str, line: usize) -> Result<(String, &str), String> {
    let mut out = String::new();
    let mut it = s[1..].char_indices();
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Ok((out, &s[1 + i + 1..])),
            '\\' => {
                let (_, e) = it.next().ok_or_else(|| err(line, "multi-line double-quoted scalar is unsupported"))?;
                let hex = |n: usize, it: &mut std::str::CharIndices| -> Result<char, String> {
                    let h: String = (0..n).filter_map(|_| it.next().map(|x| x.1)).collect();
                    u32::from_str_radix(&h, 16).ok().filter(|_| h.len() == n).and_then(char::from_u32)
                        .ok_or_else(|| err(line, &format!("bad \\ escape hex {h:?}")))
                };
                out.push(match e {
                    '0' => '\0', 'a' => '\x07', 'b' => '\x08', 't' | '\t' => '\t', 'n' => '\n', 'v' => '\x0b',
                    'f' => '\x0c', 'r' => '\r', 'e' => '\x1b', ' ' => ' ', '"' => '"', '/' => '/', '\\' => '\\',
                    'N' => '\u{85}', '_' => '\u{a0}', 'L' => '\u{2028}', 'P' => '\u{2029}',
                    'x' => hex(2, &mut it)?, 'u' => hex(4, &mut it)?, 'U' => hex(8, &mut it)?,
                    other => return Err(err(line, &format!("unknown escape \\{other}"))),
                });
            }
            _ => out.push(c),
        }
    }
    Err(err(line, "unterminated double-quoted scalar (multi-line is unsupported)"))
}

fn single_quoted(s: &str, line: usize) -> Result<(String, &str), String> {
    let mut out = String::new();
    let b = s.as_bytes();
    let mut i = 1;
    while i < b.len() {
        if b[i] == b'\'' {
            if i + 1 < b.len() && b[i + 1] == b'\'' { out.push('\''); i += 2; continue; }
            return Ok((out, &s[i + 1..]));
        }
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Err(err(line, "unterminated single-quoted scalar (multi-line is unsupported)"))
}

/// After a quoted scalar only spaces and an optional ` #comment` may follow.
fn rest_ok(rest: &str) -> bool { let t = rest.trim_start_matches([' ', '\t']); t.is_empty() || t.starts_with('#') }

const UNSUPPORTED_START: &[char] = &['[', '{', '|', '>', '&', '*', '!', '%', '@', '`', '?'];

fn parse_value(v: &str, line: usize) -> Result<Scalar, String> {
    let v = v.trim_start_matches([' ', '\t']);
    if v.starts_with('"') {
        let (t, rest) = double_quoted(v, line)?;
        return if rest_ok(rest) { Ok((t, true)) } else { Err(err(line, "text after double-quoted value")) };
    }
    if v.starts_with('\'') {
        let (t, rest) = single_quoted(v, line)?;
        return if rest_ok(rest) { Ok((t, true)) } else { Err(err(line, "text after single-quoted value")) };
    }
    if v.starts_with('#') { return Ok((String::new(), false)); }
    if v.starts_with(UNSUPPORTED_START) || v.starts_with("- ") || v == "-" {
        return Err(err(line, "collection/anchor/tag/block value is unsupported (flat mapping only)"));
    }
    // plain: ends at " #" or end of line; trailing blanks are not part of it
    let end = v.find(" #").or_else(|| v.find("\t#")).unwrap_or(v.len());
    Ok((v[..end].trim_end_matches([' ', '\t']).to_string(), false))
}

fn parse_line(l: &str, line: usize) -> Result<(Scalar, Scalar), String> {
    if l.starts_with([' ', '\t']) { return Err(err(line, "indented line is unsupported (flat mapping only)")); }
    let (key, rest) = if l.starts_with('"') {
        let (t, r) = double_quoted(l, line)?; ((t, true), r)
    } else if l.starts_with('\'') {
        let (t, r) = single_quoted(l, line)?; ((t, true), r)
    } else {
        if l.starts_with(UNSUPPORTED_START) || l.starts_with("- ") {
            return Err(err(line, "non-mapping line is unsupported (flat mapping only)"));
        }
        let b = l.as_bytes();
        let pos = (0..b.len()).find(|&i| b[i] == b':' && (i + 1 == b.len() || b[i + 1] == b' ' || b[i + 1] == b'\t'))
            .ok_or_else(|| err(line, "no `key: value` separator"))?;
        ((l[..pos].trim_end_matches([' ', '\t']).to_string(), false), &l[pos..])
    };
    let rest = rest.trim_start_matches([' ', '\t']);
    let after = rest.strip_prefix(':').ok_or_else(|| err(line, "missing ':' after key"))?;
    if !(after.is_empty() || after.starts_with([' ', '\t'])) { return Err(err(line, "':' must be followed by a space")); }
    Ok((key, parse_value(after, line)?))
}

fn parse(text: &str) -> Result<Vec<(Scalar, Scalar)>, String> {
    let mut rows: Vec<(Scalar, Scalar)> = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let l = raw.strip_suffix('\r').unwrap_or(raw);
        let t = l.trim();
        if t.is_empty() || t.starts_with('#') || (n == 0 && t == "---") { continue; }
        if t == "..." || t == "---" { return Err(err(n + 1, "multiple documents are unsupported")); }
        let (k, v) = parse_line(l, n + 1)?;
        // Python dict: a repeated key keeps its first position and takes the last value (PyYAML builds a dict).
        let shown = shown(&k);
        if let Some(row) = rows.iter_mut().find(|(rk, _)| self::shown(rk) == shown) { row.1 = v; } else { rows.push((k, v)); }
    }
    Ok(rows)
}

/// `str` scalars as themselves; resolved plain scalars as `!!<tag> <raw>`.
fn shown(s: &Scalar) -> String {
    if s.1 { return s.0.clone(); }
    match resolve_plain(&s.0) { None => s.0.clone(), Some(tag) => format!("!!{tag} {}", s.0) }
}

fn json_str(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"), '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' || c == '\u{85}' => {
                out.push_str(&format!("\\u{:04x}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

fn render(rows: &[(Scalar, Scalar)]) -> String {
    let mut out = String::from("[");
    for (i, (k, v)) in rows.iter().enumerate() {
        if i > 0 { out.push_str(", "); }
        out.push('[');
        json_str(&shown(k), &mut out);
        out.push_str(", ");
        json_str(&shown(v), &mut out);
        out.push(']');
    }
    out.push(']');
    out
}

fn self_test() -> Result<(), String> {
    // (plain scalar, PyYAML 6.0.1 tag or None = str) — derived from the regular expressions quoted above.
    let cases: &[(&str, Option<&str>)] = &[
        ("yes", Some("bool")), ("OFF", Some("bool")), ("y", None), ("n", None), ("tRUE", None), ("True", Some("bool")),
        ("3.14", Some("float")), ("1.", Some("float")), (".5", Some("float")), ("+.5", None), ("1e3", None),
        ("1.0e+3", Some("float")), ("1.0e3", None), ("1:20.5", Some("float")), ("-.Inf", Some("float")),
        ("+.INF", Some("float")), (".NaN", Some("float")), ("-.nan", None),
        ("0x1F", Some("int")), ("0o17", None), ("017", Some("int")), ("019", None), ("0b101", Some("int")),
        ("1_000", Some("int")), ("+1", Some("int")), ("-1", Some("int")), ("0", Some("int")), ("1:20", Some("int")),
        ("12:30:45", Some("int")), ("0:30", None), ("<<", Some("merge")), ("=", Some("value")),
        ("~", Some("null")), ("null", Some("null")), ("nULL", None), ("", Some("null")),
        ("2026-10-14", Some("timestamp")), ("2026-10-14T09:00:00+09:00", Some("timestamp")),
        ("2026-10-14 09:00:00", Some("timestamp")), ("2026-10-14t09:00:00z", None), ("2026-1-4 9:00:00.5 Z", Some("timestamp")),
        ("2026-10-14T09:00", None), ("007_agent는", None), ("plain", None),
    ];
    for (s, want) in cases {
        let got = resolve_plain(s);
        if got != *want { return Err(format!("resolve_plain({s:?}) = {got:?}, want {want:?}")); }
    }
    let rows = parse("---\n\"a\\tb\": 'it''s'  # c\nk: v # comment\nempty:\n\"\\u00e9\": \"\\x41\\N\"\nk: w\n")?;
    let got = render(&rows);
    let want = "[[\"a\\tb\", \"it's\"], [\"k\", \"w\"], [\"empty\", \"!!null \"], [\"é\", \"A\\u0085\"]]";
    if got != want { return Err(format!("parse/render:\n got  {got}\n want {want}")); }
    for bad in ["k:\n  - x\n", "[a, b]\n", "\"open\n", "k: [1]\n", "  k: v\n"] {
        if parse(bad).is_ok() { return Err(format!("unsupported input accepted: {bad:?}")); }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--self-test") => match self_test() {
            Ok(()) => { println!("[yaml11_oracle] self-test PASS"); ExitCode::SUCCESS }
            Err(e) => { eprintln!("[yaml11_oracle] self-test FAIL: {e}"); ExitCode::from(1) }
        },
        Some("-h" | "--help") => {
            println!("usage: yaml11_oracle < flat-mapping.yaml  → JSON [[k, v], …] (non-str = \"!!tag raw\") · --self-test");
            ExitCode::SUCCESS
        }
        Some(other) => { eprintln!("[yaml11_oracle] unknown argument: {other}"); ExitCode::from(2) }
        None => {
            let mut text = String::new();
            if let Err(e) = std::io::stdin().read_to_string(&mut text) {
                eprintln!("[yaml11_oracle] stdin read failed: {e}");
                return ExitCode::from(3);
            }
            match parse(&text) {
                Ok(rows) => { println!("{}", render(&rows)); ExitCode::SUCCESS }
                Err(e) => { eprintln!("[yaml11_oracle] unsupported input (fail-loud): {e}"); ExitCode::from(3) }
            }
        }
    }
}
