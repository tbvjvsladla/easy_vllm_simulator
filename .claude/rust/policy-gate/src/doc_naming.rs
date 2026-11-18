//! Rust port of `.claude/policies/runtime/doc_naming.py` — the naming SSOT shim (implementation in neither place
//! here). Prose names: `node .claude/skills/wiki-desk/src/doc_naming.ts <fn> '<JSON args>'` and `--patterns`, exactly
//! as the Python shim called them. Bench names: the adversarial-benchmark Rust owner through its
//! `bench_reporting_api.sh evaluate-jsonl` transport (the same owner `doc_naming_native.py` reached). Owner absent =
//! fail loud; there is no local fallback implementation.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

pub const NODE_MAJOR_MIN: u32 = 24;

/// The Python exception a naming call raised.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NamingError {
    /// `NamingCollisionExhausted` (a `ValueError`).
    CollisionExhausted(String),
    /// `NamingSourceUnreadable` (a `ValueError`).
    SourceUnreadable(String),
    /// Any other `ValueError` (TS `ok:false` that is not a collision).
    Value(String),
    /// Transport/owner failure (`RuntimeError` / `ImportError` / other class), class name + message.
    Other(String, String),
}

impl NamingError {
    pub fn class(&self) -> &str {
        match self {
            NamingError::CollisionExhausted(_) => "NamingCollisionExhausted",
            NamingError::SourceUnreadable(_) => "NamingSourceUnreadable",
            NamingError::Value(_) => "ValueError",
            NamingError::Other(c, _) => c,
        }
    }
    pub fn message(&self) -> &str {
        match self {
            NamingError::CollisionExhausted(m) | NamingError::SourceUnreadable(m) | NamingError::Value(m) => m,
            NamingError::Other(_, m) => m,
        }
    }
    /// `isinstance(e, ValueError)`.
    pub fn is_value_error(&self) -> bool {
        !matches!(self, NamingError::Other(_, _))
    }
}

impl std::fmt::Display for NamingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message())
    }
}

/// The loaded shim (what importing `doc_naming` produced): owners located, patterns read.
#[derive(Clone)]
pub struct DocNaming {
    pub repo: PathBuf,
    pub ts: PathBuf,
    pub node: PathBuf,
    pub bench_launcher: PathBuf,
    /// `DATED_DOC_TYPES`.
    pub dated_doc_types: Vec<String>,
    /// Python spellings of the TS patterns (`(?<x>` -> `(?P<x>`): topic, dated_doc, report.
    pub topic_pattern: String,
    pub dated_doc_pattern: String,
    pub report_pattern: String,
    /// `BENCH_KIND_DEFAULT_EXT`.
    pub bench_kind_default_ext: PyValue,
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = if dir.as_os_str().is_empty() { PathBuf::from(name) } else { dir.join(name) };
        if let Ok(md) = std::fs::metadata(&cand) {
            use std::os::unix::fs::PermissionsExt;
            if md.is_file() && md.permissions().mode() & 0o111 != 0 {
                return Some(cand);
            }
        }
    }
    None
}

fn get<'a>(v: &'a PyValue, k: &str) -> Option<&'a PyValue> {
    match v.peel() {
        PyValue::Dict(d) => d.get(k),
        _ => None,
    }
}

fn text(v: Option<&PyValue>) -> Option<String> {
    match v.map(|x| x.peel()) {
        Some(PyValue::Str(s)) => s.to_utf8(),
        _ => None,
    }
}

impl DocNaming {
    /// Load the shim for repository `repo` (the import-time work of the Python module).
    pub fn load(repo: &Path) -> Result<DocNaming, NamingError> {
        let bench_py = repo.join(".claude/skills/adversarial-benchmark/scripts/doc_naming_native.py");
        let bench_launcher = std::env::var_os("BENCH_REPORTING_API_LAUNCHER")
            .map(PathBuf::from)
            .unwrap_or_else(|| repo.join(".claude/skills/adversarial-benchmark/scripts/bench_reporting_api.sh"));
        if !bench_launcher.is_file() {
            return Err(NamingError::Other(
                "ImportError".into(),
                format!(
                    "doc_naming: bench naming SSOT missing at {} -- this shim re-exports it and must not substitute its own copy (plan_26090410 P0.5b)",
                    if bench_py.is_file() { bench_launcher.display().to_string() } else { bench_py.display().to_string() }
                ),
            ));
        }
        let ts = repo.join(".claude/skills/wiki-desk/src/doc_naming.ts");
        let mut me = DocNaming {
            repo: repo.to_path_buf(),
            ts: ts.clone(),
            node: PathBuf::new(),
            bench_launcher,
            dated_doc_types: vec![],
            topic_pattern: String::new(),
            dated_doc_pattern: String::new(),
            report_pattern: String::new(),
            bench_kind_default_ext: PyValue::None,
        };
        let constants = me.bench_run("constants", None)?;
        let constants = agent_runtime_support::pyjson::loads(&constants)
            .map_err(|e| NamingError::Other("JSONDecodeError".into(), e.to_string()))?;
        if !matches!(constants.peel(), PyValue::Dict(_)) {
            return Err(NamingError::Other("RuntimeError".into(), "reporting native constants are not an object".into()));
        }
        me.bench_kind_default_ext = get(&constants, "BENCH_KIND_DEFAULT_EXT").cloned().unwrap_or(PyValue::None);
        let node = which("node").filter(|_| ts.is_file()).ok_or_else(|| {
            NamingError::Other(
                "ImportError".into(),
                format!(
                    "doc_naming: prose naming SSOT needs node >= {NODE_MAJOR_MIN} and {} (wiki-desk TS) -- no Python fallback by design (plan_26101912 D6)",
                    ts.display()
                ),
            )
        })?;
        me.node = node;
        let p = me.ts_run(&["--patterns".to_string()])?;
        let py = |k: &str| text(get(&p, k)).map(|s| s.replace("(?<", "(?P<"));
        me.topic_pattern = py("topic").ok_or_else(|| NamingError::Other("KeyError".into(), "'topic'".into()))?;
        me.dated_doc_pattern = py("dated_doc").ok_or_else(|| NamingError::Other("KeyError".into(), "'dated_doc'".into()))?;
        me.report_pattern = py("report").ok_or_else(|| NamingError::Other("KeyError".into(), "'report'".into()))?;
        me.dated_doc_types = match get(&p, "dated_doc_types").map(|x| x.peel()) {
            Some(PyValue::List(items)) => items.iter().filter_map(|x| text(Some(x))).collect(),
            _ => return Err(NamingError::Other("KeyError".into(), "'dated_doc_types'".into())),
        };
        Ok(me)
    }

    fn ts_run(&self, args: &[String]) -> Result<PyValue, NamingError> {
        let out = Command::new(&self.node)
            .arg(&self.ts)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| NamingError::Other("OSError".into(), e.to_string()))?;
        let code = out.status.code().unwrap_or(-1);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        if !(code == 0 || code == 3) || crate::pytext::strip(&stdout).is_empty() {
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            return Err(NamingError::Other(
                "RuntimeError".into(),
                format!("doc_naming.ts failed (exit {code}): {}", crate::pytext::strip(&stderr)),
            ));
        }
        agent_runtime_support::pyjson::loads(&stdout).map_err(|e| NamingError::Other("JSONDecodeError".into(), e.to_string()))
    }

    /// `_call(fn, *args)` of the TS owner.
    pub fn ts_call(&self, function: &str, args: Vec<PyValue>) -> Result<PyValue, NamingError> {
        let payload = PyValue::List(args).dumps_unicode().map_err(|e| NamingError::Other("TypeError".into(), e.to_string()))?;
        let out = self.ts_run(&[function.to_string(), crate::pytext::pystr_text(&payload)])?;
        if matches!(get(&out, "ok"), Some(v) if v.is_truthy()) {
            return Ok(get(&out, "value").cloned().unwrap_or(PyValue::None));
        }
        let message = get(&out, "message").map(crate::pytext::display).unwrap_or_else(|| "None".into());
        if text(get(&out, "error")).as_deref() == Some("NamingCollisionExhausted") {
            return Err(NamingError::CollisionExhausted(message));
        }
        Err(NamingError::Value(message))
    }

    fn bench_run(&self, command: &str, payload: Option<&str>) -> Result<String, NamingError> {
        let mut child = Command::new("bash")
            .arg(&self.bench_launcher)
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| NamingError::Other("OSError".into(), e.to_string()))?;
        {
            use std::io::Write;
            let mut stdin = child.stdin.take().unwrap();
            if let Some(p) = payload {
                let _ = stdin.write_all(p.as_bytes());
            }
        }
        let out = child.wait_with_output().map_err(|e| NamingError::Other("OSError".into(), e.to_string()))?;
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        if !out.status.success() || !stderr.is_empty() {
            return Err(NamingError::Other(
                "RuntimeError".into(),
                format!(
                    "reporting native transport failed ({}): {}",
                    out.status.code().unwrap_or(-1),
                    crate::pytext::strip(&stderr)
                ),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// One `doc_naming.<function>` row of the bench owner (`reporting_native._call`). Stderr the owner reports is
    /// returned alongside the value (the Python bridge wrote it to `sys.stderr`).
    pub fn bench_call(&self, function: &str, arguments: PyDict) -> Result<(PyValue, String), NamingError> {
        let mut row = PyDict::new();
        row.set("id", PyValue::Str(PyStr::new("call")));
        row.set("operation", PyValue::Str(PyStr::new(&format!("doc_naming.{function}"))));
        row.set("arguments", PyValue::Dict(arguments));
        let line = PyValue::Dict(row).dumps_ascii().map_err(|e| NamingError::Other("TypeError".into(), e.to_string()))?;
        let out = self.bench_run("evaluate-jsonl", Some(&(line + "\n")))?;
        let records: Vec<&str> = out.lines().filter(|l| !crate::pytext::strip(l).is_empty()).collect();
        if records.len() != 1 {
            return Err(NamingError::Other(
                "RuntimeError".into(),
                format!("reporting native transport returned {} rows, expected one", records.len()),
            ));
        }
        let rec = agent_runtime_support::pyjson::loads(records[0]).map_err(|e| NamingError::Other("JSONDecodeError".into(), e.to_string()))?;
        let kind = text(get(&rec, "kind"));
        if text(get(&rec, "id")).as_deref() != Some("call") || !matches!(kind.as_deref(), Some("value") | Some("exception")) {
            return Err(NamingError::Other("RuntimeError".into(), "reporting native transport returned the wrong row".into()));
        }
        let stderr = text(get(&rec, "stderr")).unwrap_or_default();
        if kind.as_deref() == Some("exception") {
            let name = text(get(&rec, "type")).unwrap_or_default();
            let message = text(get(&rec, "message")).unwrap_or_default();
            return Err(match name.as_str() {
                "NamingCollisionExhausted" => NamingError::CollisionExhausted(message),
                "NamingSourceUnreadable" => NamingError::SourceUnreadable(message),
                "ValueError" => NamingError::Value(message),
                _ => NamingError::Other(name, message),
            });
        }
        let value = get(&rec, "value").cloned().ok_or_else(|| NamingError::Other("RuntimeError".into(), "reporting native transport returned no value".into()))?;
        Ok((value, stderr))
    }

    // ------------------------------------------------------------------ prose naming (TS owner)

    pub fn validate_topic(&self, topic: &str) -> Result<(), NamingError> {
        self.ts_call("validate_topic", vec![s(topic)]).map(|_| ())
    }

    pub fn is_dated_doc_basename(&self, doc_type: &str, basename: &str) -> Result<bool, NamingError> {
        self.ts_call("is_dated_doc_basename", vec![s(doc_type), s(basename)]).map(|v| v.is_truthy())
    }

    /// `dated_doc_basename(doc_type, generated_utc, topic, existing_basenames, ext)`.
    pub fn dated_doc_basename(
        &self,
        doc_type: &str,
        generated_utc: &PyValue,
        topic: &str,
        existing: &[String],
        ext: &str,
    ) -> Result<String, NamingError> {
        let mut ex = existing.to_vec();
        ex.sort();
        let v = self.ts_call(
            "dated_doc_basename",
            vec![s(doc_type), generated_utc.clone(), s(topic), PyValue::List(ex.iter().map(|x| s(x)).collect()), s(ext)],
        )?;
        Ok(crate::pytext::display(&v))
    }

    pub fn simlog_dirname(&self, generated_utc: &PyValue, topic: &str, existing: &[String]) -> Result<String, NamingError> {
        let mut ex = existing.to_vec();
        ex.sort();
        let v = self.ts_call("simlog_dirname", vec![generated_utc.clone(), s(topic), PyValue::List(ex.iter().map(|x| s(x)).collect())])?;
        Ok(crate::pytext::display(&v))
    }

    pub fn report_basename(&self, name: &str, ext: &str) -> Result<String, NamingError> {
        let v = self.ts_call("report_basename", vec![s(name), s(ext)])?;
        Ok(crate::pytext::display(&v))
    }

    // ------------------------------------------------------------------ bench naming (Rust owner, re-exported)

    /// `kst_tokens(generated_utc)` -> (YYMMDDHH, MM, SS).
    pub fn kst_tokens(&self, generated_utc: &PyValue) -> Result<(String, String, String), NamingError> {
        let mut a = PyDict::new();
        a.set("generated_utc", generated_utc.clone());
        let (v, _err) = self.bench_call("kst_tokens", a)?;
        match v.peel() {
            PyValue::List(items) if items.len() == 3 => {
                let t: Vec<String> = items.iter().map(crate::pytext::display).collect();
                Ok((t[0].clone(), t[1].clone(), t[2].clone()))
            }
            _ => Err(NamingError::Other("ValueError".into(), "kst_tokens returned a non-triple".into())),
        }
    }

    pub fn gpu_key(&self, gpu_model: &PyValue) -> Result<String, NamingError> {
        let mut a = PyDict::new();
        a.set("gpu_model", gpu_model.clone());
        self.bench_call("gpu_key", a).map(|(v, _)| crate::pytext::display(&v))
    }

    pub fn measurement_ts(&self, text: &PyValue) -> Result<PyValue, NamingError> {
        let mut a = PyDict::new();
        a.set("text", text.clone());
        self.bench_call("measurement_ts", a).map(|(v, _)| v)
    }

    pub fn scan_bench_dir(&self, out_dir: &str, kind: Option<&str>) -> Result<(PyValue, String), NamingError> {
        let mut a = PyDict::new();
        a.set("out_dir", s(out_dir));
        if let Some(k) = kind {
            a.set("kind", s(k));
        }
        self.bench_call("scan_bench_dir", a)
    }

    /// `bench_filename(kind, meta, generated_utc, existing=?, ext=?)`.
    pub fn bench_filename(
        &self,
        kind: &str,
        meta: &PyValue,
        generated_utc: &PyValue,
        existing: Option<&PyValue>,
        ext: Option<&str>,
    ) -> Result<(PyValue, String), NamingError> {
        let mut a = PyDict::new();
        a.set("kind", s(kind));
        a.set("meta", meta.clone());
        a.set("generated_utc", generated_utc.clone());
        if let Some(e) = existing {
            a.set("existing", e.clone());
        }
        if let Some(x) = ext {
            a.set("ext", s(x));
        }
        self.bench_call("bench_filename", a)
    }
}

fn s(t: &str) -> PyValue {
    PyValue::Str(PyStr::new(t))
}

fn opt_utc(t: Option<&str>) -> PyValue {
    t.map(s).unwrap_or(PyValue::None)
}

/// `_self_test()`; Err = the AssertionError / exception text.
pub fn self_test(dn: &DocNaming) -> Result<(), String> {
    let req = |c: bool, m: &str| if c { Ok(()) } else { Err(format!("AssertionError: {m}")) };
    let e = |x: NamingError| format!("{}: {}", x.class(), x.message());
    for utc in [
        Some("2026-07-24T16:22:29Z"),
        Some("2026-07-15T21:21:33Z"),
        Some("2026-12-31T15:00:00Z"),
        Some("2026-02-29T00:00:00Z"),
        None,
        Some("garbage"),
    ] {
        let v = opt_utc(utc);
        let ts = dn.ts_call("kst_tokens", vec![v.clone()]).map_err(e)?;
        let ts: Vec<String> = match ts.peel() {
            PyValue::List(items) => items.iter().map(crate::pytext::display).collect(),
            _ => vec![],
        };
        let b = dn.kst_tokens(&v).map_err(e)?;
        req(ts == vec![b.0, b.1, b.2], &format!("KST 교차검증 불일치: {}", utc.map(crate::pytext::repr).unwrap_or_else(|| "None".into())))?;
    }
    req(
        dn.dated_doc_basename("plan", &s("2026-07-25T06:58:00Z"), "하네스_루프_구조개선", &[], "md").map_err(e)?
            == "plan_26072515_하네스_루프_구조개선.md",
        "plan",
    )?;
    req(
        dn.dated_doc_basename("testlog", &s("2026-07-25T06:58:29Z"), "구조개선", &["testlog_26072515_구조개선.md".into()], "md")
            .map_err(e)?
            == "testlog_26072515_58_29_구조개선.md",
        "collision",
    )?;
    match dn.dated_doc_basename(
        "testlog",
        &s("2026-07-25T06:58:29Z"),
        "구조개선",
        &["testlog_26072515_구조개선.md".into(), "testlog_26072515_58_29_구조개선.md".into()],
        "md",
    ) {
        Err(NamingError::CollisionExhausted(_)) => {}
        Err(x) => return Err(e(x)),
        Ok(_) => return Err("AssertionError: expected NamingCollisionExhausted".into()),
    }
    req(dn.dated_doc_pattern.contains("(?P<hour>") && dn.dated_doc_pattern.contains("(?P<topic>"), "lineage 가 읽는 그룹 이름")?;
    req(
        dn.is_dated_doc_basename("testlog", "testlog_26072515_58_29_구조개선.md").map_err(e)?
            && !dn.is_dated_doc_basename("plan", "testlog_26072515_구조개선.md").map_err(e)?,
        "is_dated",
    )?;
    req(
        dn.simlog_dirname(&s("2026-06-21T12:21:00Z"), "vLLM0.22.1_KV클램프_시뮬", &[]).map_err(e)? == "26062121_vLLM0.22.1_KV클램프_시뮬",
        "simlog",
    )?;
    req(
        dn.report_basename("perf_26082421_qwen3.8-27b_레시피별_성능", "html").map_err(e)? == "perf_26082421_qwen3.8-27b_레시피별_성능.html",
        "report",
    )?;
    match dn.report_basename("rtxpro6000-benchmark-explorer", "html") {
        Err(x) if x.is_value_error() => {}
        Err(x) => return Err(e(x)),
        Ok(_) => return Err("AssertionError: expected ValueError".into()),
    }
    // re-export, not copy: this module has no bench naming implementation of its own (every bench call above went
    // through the owner's transport).
    Ok(())
}

const HELP: &str = "usage: {prog} [-h] [--self-test]\n\n{prog} -- Rust launcher that reads the naming SSOTs (wiki-desk doc_naming.ts, bench_reporting_api.sh)\n\noptions:\n  -h, --help   show this help message and exit\n  --self-test\n";

/// The whole `doc_naming.py` command line: (stdout, stderr, exit code).
pub fn main_cli(argv: &[String]) -> (String, String, i32) {
    let argv0 = argv.first().cloned().unwrap_or_default();
    let script = crate::cli::script_path(&argv0, "doc_naming.sh");
    let mut repo = script.clone();
    for _ in 0..4 {
        repo.pop();
    }
    let dn = match DocNaming::load(&repo) {
        Ok(d) => d,
        Err(x) => return (String::new(), format!("Traceback (most recent call last):\n{}: {}\n", x.class(), x.message()), 1),
    };
    let prog = crate::argp::prog_name(&argv0, "doc_naming.sh");
    let help = HELP.replace("{prog}", &prog);
    let usage = format!("usage: {prog} [-h] [--self-test]\n");
    let opts = [crate::argp::Opt::help(), crate::argp::Opt::flag(&["--self-test"], "self_test")];
    let args: Vec<String> = argv.iter().skip(1).cloned().collect();
    let level = match crate::argp::parse_level(&prog, &opts, &args, None) {
        Ok(l) => l,
        Err(crate::argp::Stop::Help { .. }) => return (help, String::new(), 0),
        Err(crate::argp::Stop::Error { message, .. }) => return (String::new(), format!("{usage}{prog}: error: {message}\n"), 2),
    };
    if !level.extras.is_empty() {
        return (String::new(), format!("{usage}{prog}: error: unrecognized arguments: {}\n", level.extras.join(" ")), 2);
    }
    if level.values.flag("self_test") {
        return match self_test(&dn) {
            Ok(()) => ("[doc_naming shim] self-test PASS\n".into(), String::new(), 0),
            Err(t) => (String::new(), format!("Traceback (most recent call last):\n{t}\n"), 1),
        };
    }
    (String::new(), help, 2)
}
