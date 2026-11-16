//! The downgrade-vocabulary owner `adversarial-benchmark` `classify_cell` (Rust), reached exactly as the original's
//! `classify_cell_native.py` transport reaches it: `bash classify_cell_api.sh constants` and one `evaluate-jsonl`
//! request per call. No rule of the owner is copied here.

use crate::json::{dict, dumps_ascii, get_str, obj, s, text, V};
use agent_runtime_support::pyjson;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const NATIVE_REL: &str = ".claude/skills/adversarial-benchmark/scripts/classify_cell_native.py";
const LAUNCHER_REL: &str = ".claude/skills/adversarial-benchmark/scripts/classify_cell_api.sh";

fn run(repo: &Path, argv: &[&str], payload: Option<&str>) -> Result<String, String> {
    let mut cmd = Command::new("bash");
    cmd.arg(repo.join(LAUNCHER_REL)).args(argv).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| format!("classifier native transport failed (-1): {e}"))?;
    if let Some(mut stdin) = child.stdin.take() {
        if let Some(p) = payload {
            let _ = stdin.write_all(p.as_bytes());
        }
    }
    let out = child.wait_with_output().map_err(|e| format!("classifier native transport failed (-1): {e}"))?;
    let code = out.status.code().unwrap_or(-1);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if code != 0 || !stderr.is_empty() {
        return Err(format!("classifier native transport failed ({code}): {}", stderr.trim_end()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

pub struct Owner {
    pub downgrade_reasons: Vec<String>,
    repo: std::path::PathBuf,
}

/// Loads the owner (the original's import of `classify_cell_native.py`, which reads the constants at import).
pub fn load(repo: &Path) -> Result<Owner, String> {
    let raw = run(repo, &["constants"], None)?;
    let doc = pyjson::loads(&raw).map_err(|e| e.to_string())?;
    let reasons = dict(&doc)
        .and_then(|d| d.get("DOWNGRADE_REASONS"))
        .ok_or_else(|| "module has no attribute 'DOWNGRADE_REASONS'".to_string())?;
    let mut out = Vec::new();
    if let PyValue::List(items) = reasons.peel() {
        for item in items {
            out.push(text(item).unwrap_or_default());
        }
    }
    for need in ["BENCH_MODE_LITE"] {
        if dict(&doc).and_then(|d| d.get(need)).is_none() {
            return Err(format!("module '_evidence_publisher_classify_cell' has no attribute '{need}'"));
        }
    }
    Ok(Owner { downgrade_reasons: out, repo: repo.to_path_buf() })
}

impl Owner {
    /// `bench_mode_kind(record)`.
    pub fn bench_mode_kind(&self, record: &V) -> Result<V, String> {
        let request = obj(vec![
            ("id", s("python-consumer")),
            ("operation", s("bench_mode_kind")),
            ("arguments_json", obj(vec![("record", s(&dumps_ascii(record)))])),
            ("result_json", PyValue::Bool(true)),
        ]);
        let line = format!("{}\n", dumps_ascii(&request));
        let text = run(&self.repo, &["evaluate-jsonl"], Some(&line))?;
        let lines: Vec<&str> = text.lines().collect();
        if lines.len() != 1 {
            return Err("classifier native transport did not return one record".to_string());
        }
        let record = pyjson::loads(lines[0]).map_err(|e| e.to_string())?;
        if get_str(&record, "id").as_deref() != Some("python-consumer")
            || get_str(&record, "encoding").as_deref() != Some("json-values-v1")
        {
            return Err("classifier native binary lacks the required JSON transport; rebuild it".to_string());
        }
        if let Some(err) = get_str(&record, "stderr") {
            eprint!("{err}");
        }
        match get_str(&record, "kind").as_deref() {
            Some("value") => pyjson::loads(&get_str(&record, "value_json").unwrap_or_default()).map_err(|e| e.to_string()),
            Some("exception") => Err(format!(
                "classifier native owner reported {}: {}",
                get_str(&record, "type").unwrap_or_default(),
                get_str(&record, "message").unwrap_or_default()
            )),
            _ => Err("classifier native transport returned an unknown record kind".to_string()),
        }
    }
}
