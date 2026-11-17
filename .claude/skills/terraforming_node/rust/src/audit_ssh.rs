//! `audit_session_ssh.py` — audit of a session transcript for ssh opened outside the sanctioned channels (predicate A1).
//!
//! Same contract as the original: exit 0 = no violation, 1 = violations (or an unreadable transcript), 2 = input error.

use crate::argp::{self, flag, store, Outcome, Spec, HELP};
use crate::py;
use agent_runtime_support::pyjson;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;
use std::path::Path;

/// Sanctioned channels — a closed list (tripwire): a new orchestration script must be classified here.
/// `relay.sh`/`agent_control.sh` are the Rust launchers of the two A2A programs (the `.py` names stay for older transcripts).
pub const SANCTIONED: [&str; 12] = [
    "relay.py",
    "agent_control.py",
    "relay.sh",
    "agent_control.sh",
    "sync_to_sub.sh",
    "fetch_sub_docs.sh",
    "library_access.py",
    "library_access.sh",
    "multinode_comms_smoke.sh",
    "multinode_serve_smoke.sh",
    "scan_node.py",
    "scan_node.sh",
];
const SHELL_SINKS: [&str; 6] = ["bash", "sh ", "zsh", "ssh", "dash", "/bin/sh"];

fn ident_end(c: &[char], i: usize) -> Option<usize> {
    let first = *c.get(i)?;
    if !(first.is_ascii_alphabetic() || first == '_') {
        return None;
    }
    let mut j = i + 1;
    while j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '_') {
        j += 1;
    }
    Some(j)
}

fn spaces(c: &[char], mut i: usize) -> usize {
    while i < c.len() && py::is_space(c[i]) {
        i += 1;
    }
    i
}

/// `\s+` at i: the end, or None when there is no whitespace.
fn spaces1(c: &[char], i: usize) -> Option<usize> {
    let j = spaces(c, i);
    if j > i {
        Some(j)
    } else {
        None
    }
}

fn non_spaces(c: &[char], mut i: usize) -> usize {
    while i < c.len() && !py::is_space(c[i]) {
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

/// `(?:sudo\s+|timeout\s+\S+\s+|[A-Za-z_][A-Za-z0-9_]*=\S*\s+)*ssh\b` anchored at i.
fn prefixed_ssh(c: &[char], i: usize, depth: usize) -> bool {
    if depth > c.len() + 1 {
        return false;
    }
    if let Some(j) = lit(c, i, "ssh") {
        if c.get(j).is_none_or(|&x| !py::is_word(x)) {
            return true;
        }
    }
    if let Some(j) = lit(c, i, "sudo").and_then(|j| spaces1(c, j)) {
        if prefixed_ssh(c, j, depth + 1) {
            return true;
        }
    }
    if let Some(j) = lit(c, i, "timeout").and_then(|j| spaces1(c, j)) {
        let k = non_spaces(c, j);
        if k > j {
            if let Some(m) = spaces1(c, k) {
                if prefixed_ssh(c, m, depth + 1) {
                    return true;
                }
            }
        }
    }
    if let Some(j) = ident_end(c, i) {
        if c.get(j) == Some(&'=') {
            let k = non_spaces(c, j + 1);
            if let Some(m) = spaces1(c, k) {
                if prefixed_ssh(c, m, depth + 1) {
                    return true;
                }
            }
        }
    }
    false
}

/// `SSH_RE.search(cmd)`: ssh in command position (start, after `;&|(\n` or a backquote, or inside `$(`).
pub fn ssh_hit(cmd: &str) -> bool {
    let c: Vec<char> = cmd.chars().collect();
    let mut starts = vec![0usize];
    for (i, ch) in c.iter().enumerate() {
        if matches!(ch, ';' | '&' | '|' | '(' | '\n' | '`') {
            starts.push(i + 1);
        }
        if *ch == '$' && c.get(i + 1) == Some(&'(') {
            starts.push(i + 2);
        }
    }
    starts
        .into_iter()
        .any(|q| prefixed_ssh(&c, spaces(&c, q), 0))
}

/// One `_HEREDOC_RE` match at i: (end, delimiter).
fn heredoc_at(c: &[char], i: usize) -> Option<(usize, String)> {
    let mut j = lit(c, i, "<<")?;
    if c.get(j) == Some(&'-') {
        j += 1;
    }
    j = spaces(c, j);
    let quote = match c.get(j) {
        Some(&q) if q == '\'' || q == '"' => Some(q),
        _ => None,
    };
    if let Some(q) = quote {
        if let Some(e) = ident_end(c, j + 1) {
            if c.get(e) == Some(&q) {
                return Some((e + 1, c[j + 1..e].iter().collect()));
            }
        }
        return None;
    }
    let e = ident_end(c, j)?;
    Some((e, c[j..e].iter().collect()))
}

fn find_from(c: &[char], needle: &[char], from: usize) -> Option<usize> {
    if needle.len() > c.len() {
        return None;
    }
    (from..=c.len() - needle.len()).find(|&k| c[k..k + needle.len()] == *needle)
}

/// `strip_data_heredocs`: drop heredoc bodies fed to a program that is not a shell.
pub fn strip_data_heredocs(cmd: &str) -> String {
    let c: Vec<char> = cmd.chars().collect();
    let mut matches = vec![];
    let mut i = 0;
    while i < c.len() {
        if let Some((e, d)) = heredoc_at(&c, i) {
            matches.push((i, e, d));
            i = e;
        } else {
            i += 1;
        }
    }
    let mut out = String::new();
    let mut pos = 0usize;
    for (start, end_m, delim) in matches {
        if start < pos {
            continue;
        }
        let line_start = c[..start]
            .iter()
            .rposition(|&x| x == '\n')
            .map_or(0, |k| k + 1);
        let head: String = c[line_start..start].iter().collect();
        let mut needle = vec!['\n'];
        needle.extend(delim.chars());
        let Some(end) = find_from(&c, &needle, end_m) else {
            break;
        };
        if SHELL_SINKS.iter().any(|k| head.contains(k)) {
            continue;
        }
        out.extend(c[pos..end_m].iter());
        pos = end + 1 + delim.chars().count();
    }
    if pos <= c.len() {
        out.extend(c[pos..].iter());
    }
    out
}

/// The shell commands of a transcript in the original's traversal order; `Err` = none could be read.
pub enum Unread {
    /// No command could be read: the original's `SystemExit(message)` (exit 1).
    Empty(String),
    Raised(crate::raise::Raised),
}

pub fn commands(path: &Path, raw: &[u8]) -> Result<Vec<(String, PyValue)>, Unread> {
    let mut out = vec![];
    for line in py::splitlines(&py::decode_replace(raw)) {
        let line = py::strip(&line);
        if line.is_empty() {
            continue;
        }
        let row = match pyjson::loads_text(&PyStr::new(line)) {
            Ok(v) => v,
            Err(e) if e.is_value_error() => continue,
            Err(e) => {
                return Err(Unread::Raised(crate::raise::Raised::new(
                    "RecursionError",
                    &e.to_string(),
                )))
            }
        };
        let mut stack = vec![row];
        while let Some(node) = stack.pop() {
            match node.peel() {
                PyValue::Dict(d) => {
                    let is_tool = matches!(d.get("type").map(|v| v.peel()), Some(PyValue::Str(t)) if t.equals("tool_use"));
                    if is_tool {
                        if let Some(PyValue::Dict(input)) = d.get("input").map(|v| v.peel()) {
                            if let Some(PyValue::Str(cmd)) = input.get("command").map(|v| v.peel())
                            {
                                out.push((
                                    py::text(cmd),
                                    d.get("name").cloned().unwrap_or(PyValue::None),
                                ));
                            }
                        }
                    }
                    for (_, v) in d.iter() {
                        stack.push(v.clone());
                    }
                }
                PyValue::List(items) => {
                    for v in items {
                        stack.push(v.clone());
                    }
                }
                _ => {}
            }
        }
    }
    if out.is_empty() {
        return Err(Unread::Empty(format!(
            "[ssh-audit] FAIL: transcript 에서 명령을 하나도 읽지 못했다 — {}. 판독 실패를 '위반 0' 으로 접지 않는다(형식 확인 필요).",
            path.display()
        )));
    }
    Ok(out)
}

pub struct Violation {
    pub tool: String,
    pub command: String,
}

pub fn violations(cmds: &[(String, PyValue)], sub_host: &str) -> Vec<Violation> {
    let mut out = vec![];
    for (raw, tool) in cmds {
        let cmd = strip_data_heredocs(raw);
        if !ssh_hit(&cmd) {
            continue;
        }
        if !sub_host.is_empty() && !cmd.contains(sub_host) {
            continue;
        }
        if SANCTIONED.iter().any(|s| cmd.contains(s)) {
            continue;
        }
        out.push(Violation {
            tool: py::display(tool),
            command: py::head(&py::split_ws(&cmd).join(" "), 220),
        });
    }
    out
}

fn self_test() -> i32 {
    let mut ok = true;
    let mut ck = |label: &str, cond: bool| {
        println!("{}{}", if cond { "  PASS " } else { "  FAIL " }, label);
        ok = ok && cond;
    };
    let hit = ssh_hit;
    ck("문장 첫 자리의 ssh 는 실행이다", hit("ssh user@node 'ls'"));
    ck(
        "구분자 뒤의 ssh 도 실행이다",
        hit("echo x; ssh h 'ls'") && hit("a && ssh h 'ls'") && hit("a | ssh h 'ls'"),
    );
    ck(
        "timeout·env 접두가 붙어도 실행이다",
        hit("timeout 30 ssh h 'ls'") && hit("FOO=1 ssh h 'ls'"),
    );
    ck("명령치환 안의 ssh 도 실행이다", hit("X=$(ssh h 'ls')"));
    ck(
        "★음성대조: 산문·문자열 안의 ssh 는 실행이 아니다",
        !hit("print(\"=== ssh to sub ===\")")
            && !hit("grep -c 'ssh' file")
            && !hit("audit_session_ssh.sh --transcript x"),
    );
    ck(
        "★음성대조: 경로 조각의 ssh 는 실행이 아니다",
        !hit("cat ~/.ssh/config") && !hit("ls /usr/bin/sshd"),
    );
    let py_doc = "python3 - <<PYEOF\ns = echo x; ssh h\nPYEOF\n";
    ck(
        "★음성대조: 셸 아닌 sink 의 heredoc 본문은 데이터다(감사기가 자기 소스에 걸리지 않는다)",
        hit(py_doc) && !hit(&strip_data_heredocs(py_doc)),
    );
    let sh_doc = "bash <<EOF\nssh h\nEOF\n";
    ck(
        "★셸에 넘기는 heredoc 은 벗기지 않는다(진짜 우회가 숨을 자리를 만들지 않는다)",
        hit(&strip_data_heredocs(sh_doc)),
    );
    let unterminated = "python3 - <<PYEOF\nssh h\n";
    ck(
        "종료 구분자가 없으면 벗기지 않는다(판독 실패를 통과로 접지 않는다)",
        hit(&strip_data_heredocs(unterminated)),
    );
    println!("[ssh-audit] self-test {}", if ok { "PASS" } else { "FAIL" });
    if ok {
        0
    } else {
        1
    }
}

pub static SPEC: Spec = Spec {
    prog: "audit_session_ssh.sh",
    usage: include_str!("../help/audit_session_ssh.usage"),
    help: include_str!("../help/audit_session_ssh.help"),
    args: &[
        HELP,
        flag(&["--self-test"]),
        store(&["--transcript"]),
        store(&["--sub-host"]),
        store(&["--limit"]).int(),
    ],
    subs: None,
};

pub fn main(args: &[String]) -> i32 {
    let a = match argp::parse(&SPEC, args) {
        Outcome::Ok(p) => p,
        Outcome::Exit(c) => return c,
    };
    if a.flag(1) {
        return self_test();
    }
    let transcript = a.text(2).unwrap_or("");
    let sub_host = a.text(3).unwrap_or("");
    let limit = a.int(4).unwrap_or(12);
    let mut err = std::io::stderr().lock();
    if transcript.is_empty() || sub_host.is_empty() {
        let _ = writeln!(
            err,
            "[ssh-audit] FAIL: --transcript 와 --sub-host 는 필수다(--self-test 는 예외)"
        );
        return 2;
    }
    let path = Path::new(transcript);
    if !path.is_file() {
        let _ = writeln!(
            err,
            "[ssh-audit] FAIL: transcript 부재 — {}",
            crate::pathfmt::py_path(transcript)
        );
        return 2;
    }
    let raw = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            drop(err);
            return crate::raise::exit(&crate::raise::Raised::os(
                &e,
                &crate::pathfmt::py_path(transcript),
            ));
        }
    };
    let shown = crate::pathfmt::py_path(transcript);
    let cmds = match commands(Path::new(&shown), &raw) {
        Ok(c) => c,
        Err(Unread::Empty(m)) => {
            let _ = writeln!(err, "{m}");
            return 1;
        }
        Err(Unread::Raised(r)) => {
            drop(err);
            return crate::raise::exit(&r);
        }
    };
    let bad = violations(&cmds, sub_host);
    let name = crate::pathfmt::py_name(transcript);
    if bad.is_empty() {
        println!("[ssh-audit] PASS — 허가 통로 밖 ssh 0건 ({name} · sub={sub_host})");
        return 0;
    }
    let _ = writeln!(err, "[ssh-audit] FAIL — 허가 통로 밖 ssh {}건 ({name} · sub={sub_host}). 상향 회수는 문서기반이다(헌법 노드제어 ①).", bad.len());
    for row in &bad[..py::slice_end(bad.len(), limit)] {
        let _ = writeln!(err, "  - [{}] {}", row.tool, row.command);
    }
    if (bad.len() as i64) > limit {
        let _ = writeln!(err, "  … 그 외 {}건", bad.len() as i64 - limit);
    }
    let _ = writeln!(err, "  → 대신 쓸 허가 통로:");
    let _ = writeln!(err, "     · 도달·상태 사전 확인 → `fetch_sub_docs.sh --topology=<t>` (인자 없이 = DRY-RUN). SSH 도달을 스스로 검사하고 무엇을 가져올지만 보여준다(서브 변경 0).");
    let _ = writeln!(err, "     · 진행 관측 → `sub.campaign.brief` = 회수 미러의 `sync_staging/sub_docs/logs/<node>/campaign_brief.json`.");
    let _ = writeln!(
        err,
        "     · 제어·교착 해제 → `relay.sh` / `agent_control.sh` (§2.7.7 A2A 제어명령)."
    );
    let _ = writeln!(err, "  채널이 있는데도 ssh 를 여는 것은 우회다(D3) — 경로가 없으면 경로를 만들고, 있으면 그것을 쓴다.");
    1
}
