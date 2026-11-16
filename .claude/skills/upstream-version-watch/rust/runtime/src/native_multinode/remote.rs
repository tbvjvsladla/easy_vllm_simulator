//! The fixed sub-node runner (`native_remote`): what the `Ssh` runner of `native_multinode_serve` executes on the Sub, one
//! operation per ssh call. It replaces the `python3 -c BOOT REMOTE` program the port still sent byte for byte (plan_26111615
//! pass 3 — no Python on the Sub for harness work). Protocol unchanged: argv `-` (request on stdin) or the request itself,
//! base64 of a JSON object `{op, root, run_id, marker, created_by, …}`; one JSON line on stdout (`sort_keys`) and exit 0, or
//! one stderr line and exit 3 (`die`). Operations and refusals are the original's: `init` · `put` · `run` · `start` ·
//! `stop` · `text` · `tree` · `gpu` · `ports` · `exists` · `remove_owned_tree`; the owned-root and marker checks guard every
//! mutating operation, and every path is resolved beneath the run root.

use super::*;

fn die(m: impl std::fmt::Display) -> ! {
    let _ = writeln!(std::io::stderr(), "{m}");
    std::process::exit(3)
}

fn emit(v: &PyValue) {
    let text = sorted(v).dumps_ascii().unwrap_or_else(|_| die("unencodable result"));
    let mut o = std::io::stdout();
    let _ = writeln!(o, "{text}");
    let _ = o.flush();
}

fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if bytes.len() % 4 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for ch in bytes.chunks(4) {
        let pad = ch.iter().rev().take_while(|&&b| b == b'=').count();
        if pad > 2 || ch[..4 - pad].contains(&b'=') {
            return None;
        }
        let mut n = 0u32;
        for (i, &b) in ch.iter().enumerate() {
            n |= if i < 4 - pad { val(b)? } else { 0 } << (18 - 6 * i as u32);
        }
        out.push((n >> 16) as u8);
        if pad < 2 {
            out.push((n >> 8) as u8);
        }
        if pad < 1 {
            out.push(n as u8);
        }
    }
    Some(out)
}

struct Req {
    r: PyValue,
    root: String,
    rid: String,
    marker: String,
    created: String,
}
impl Req {
    fn s(&self, k: &str) -> String {
        as_str(dget(&self.r, k)).unwrap_or_else(|| die(format!("KeyError: '{k}'")))
    }
    fn want(&self) -> Vec<u8> {
        let d = dict(vec![("run_id", vs(&self.rid)), ("created_by", vs(&self.created))]);
        (dumps_tight(&d) + "\n").into_bytes()
    }
    fn under_rel(&self, rel: Option<&PyValue>) -> PathBuf {
        let rel = match rel.map(PyValue::peel) {
            Some(PyValue::Str(s)) => s.to_utf8().unwrap_or_default(),
            _ => die("bad-relative-path"),
        };
        if rel.is_empty() || rel.starts_with('/') {
            die("bad-relative-path");
        }
        let b = resolve_nonstrict(Path::new(&self.root), 0);
        let p = resolve_nonstrict(&Path::new(&self.root).join(&rel), 0);
        if p.strip_prefix(&b).is_err() {
            die("escape");
        }
        p
    }
    fn owned_root(&self) {
        let root = Path::new(&self.root);
        if root.file_name().and_then(|n| n.to_str()) != Some(self.rid.as_str()) || is_symlink(root) || !root.is_dir() || ismount(root) {
            die("bad-root");
        }
        if std::fs::read(root.join(&self.marker)).ok().as_deref() != Some(self.want().as_slice()) {
            die("bad-marker");
        }
    }
}

fn ints(v: Option<&PyValue>) -> Vec<i64> {
    list_items(v).into_iter().filter_map(|x| as_i64(Some(x))).collect()
}

fn ok() -> PyValue {
    dict(vec![("ok", PyValue::Bool(true))])
}

fn serve_op(q: &Req) -> PyValue {
    let root = Path::new(&q.root);
    match q.s("op").as_str() {
        "init" => {
            if lexists(root) {
                die("root-exists");
            }
            let parent = root.parent().unwrap_or(Path::new("/"));
            if truthy(dget(&q.r, "ensure_parent")) {
                let _ = mkdir_mode(parent, 0o700, true);
            }
            if !parent.is_dir() || is_symlink(parent) {
                die("bad-parent");
            }
            mkdir_mode(root, 0o700, false).unwrap_or_else(|e| die(e));
            std::fs::write(root.join(&q.marker), q.want()).unwrap_or_else(|e| die(e));
            ok()
        }
        "put" => {
            q.owned_root();
            let p = q.under_rel(dget(&q.r, "rel"));
            if lexists(&p) {
                die("put-exists");
            }
            if let Some(par) = p.parent() {
                mkdir_mode(par, 0o700, true).unwrap_or_else(|e| die(e));
            }
            let data = b64_decode(&q.s("data")).unwrap_or_else(|| die("binascii.Error: invalid base64"));
            let mode = as_i64(dget(&q.r, "mode")).unwrap_or_else(|| die("KeyError: 'mode'")) as u32;
            let mut f = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .custom_flags(libc::O_NOFOLLOW)
                .mode(mode)
                .open(&p)
                .unwrap_or_else(|e| die(e));
            f.write_all(&data).unwrap_or_else(|e| die(e));
            ok()
        }
        "run" => {
            let argv: Vec<String> = list_items(dget(&q.r, "argv")).into_iter().filter_map(|x| as_str(Some(x))).collect();
            if argv.is_empty() {
                die("empty argv");
            }
            let timeout = as_i64(dget(&q.r, "timeout")).unwrap_or(30).max(0) as u64;
            match run_cmd(&argv, timeout, None) {
                Cmd::Done(rc, out, err) => dict(vec![("rc", vi(i64::from(rc))), ("out", vs(&out)), ("err", vs(&err))]),
                Cmd::Timeout => die("bounded-timeout"),
                Cmd::NotFound => die(format!("FileNotFoundError: {}", argv[0])),
                Cmd::Other(e) => die(e),
            }
        }
        "start" => {
            q.owned_root();
            let log_rel = q.s("log_rel");
            let _ = q.under_rel(dget(&q.r, "log_rel"));
            let argv: Vec<String> = list_items(dget(&q.r, "argv")).into_iter().filter_map(|x| as_str(Some(x))).collect();
            let node = q.s("node");
            let role = q.s("role");
            match Local::new(&q.root, &node).start(&argv, &log_rel, &role) {
                Ok(i) => i.row(),
                Err(e) => die(format!("starttime(process exited at start): {}", e.msg())),
            }
        }
        "stop" => {
            let i = dget(&q.r, "identity").cloned().unwrap_or_else(|| die("KeyError: 'identity'"));
            let (pid, pgid) = (as_i64(dget(&i, "pid")).unwrap_or_else(|| die("bad pid")), as_i64(dget(&i, "pgid")).unwrap_or_else(|| die("bad pgid")));
            let start = py_str(dget(&i, "starttime"));
            let wait = as_i64(dget(&q.r, "wait")).unwrap_or(0).max(0) as u64;
            match stop_process(pid, pgid, &start, wait, truthy(dget(&q.r, "pid_only"))) {
                Ok(status) => dict(vec![
                    ("node", vs(&py_str(dget(&i, "node")))),
                    ("role", vs(&py_str(dget(&i, "role")))),
                    ("pid", vi(pid)),
                    ("term_status", vs(status)),
                    ("starttime_match", PyValue::Bool(true)),
                ]),
                Err(e) => die(e),
            }
        }
        "text" => {
            let p = q.under_rel(dget(&q.r, "rel"));
            let b = std::fs::read(&p).unwrap_or_else(|e| die(e));
            dict(vec![("text", vs(&String::from_utf8_lossy(&b)))])
        }
        "tree" => dict(vec![("processes", PyValue::List(tree(&ints(dget(&q.r, "pids")), &ints(dget(&q.r, "pgids"))).iter().map(ProcRow::value).collect()))]),
        "gpu" => match gpu_query() {
            Ok(v) => dict(vec![("processes", PyValue::List(v.iter().map(|x| vs(x)).collect()))]),
            Err(e) => die(e),
        },
        "ports" => match ports_query() {
            Ok(v) => dict(vec![("ports", PyValue::List(v.into_iter().map(vi).collect()))]),
            Err(e) => die(e),
        },
        "exists" => dict(vec![("exists", PyValue::Bool(lexists(root)))]),
        "remove_owned_tree" => {
            q.owned_root();
            if let Err(e) = safe_remove_owned_tree(root, &q.rid) {
                die(e.msg());
            }
            if lexists(root) {
                die("remaining");
            }
            ok()
        }
        _ => die("bad-op"),
    }
}

/// `native_remote -` (request on stdin) or `native_remote <request>`.
pub fn main(args: &[String]) -> i32 {
    let raw = match args.first().map(String::as_str) {
        Some("-") => {
            let mut s = String::new();
            std::io::stdin().read_to_string(&mut s).unwrap_or_else(|e| die(e));
            s
        }
        Some(x) => x.to_string(),
        None => die("usage: native_remote -|<base64 request>"),
    };
    let bytes = b64_decode(raw.trim()).unwrap_or_else(|| die("binascii.Error: invalid base64 request"));
    let text = String::from_utf8(bytes).unwrap_or_else(|_| die("request is not UTF-8"));
    let r = agent_runtime_support::pyjson::loads(&text).unwrap_or_else(|_| die("request is not JSON"));
    let s = |k: &str| as_str(dget(&r, k)).unwrap_or_else(|| die(format!("KeyError: '{k}'")));
    let q = Req { root: s("root"), rid: s("run_id"), marker: s("marker"), created: s("created_by"), r: r.clone() };
    let out = serve_op(&q);
    emit(&out);
    0
}
