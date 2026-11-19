//! `policy-distribution`: Rust port of `.claude/policies/runtime/verify_distribution.py` (fail-closed acceptance of the
//! five-skill self-contained distribution). Check names, the JSON report (`json.dumps(indent=2, ensure_ascii=False)`
//! key order) and exit codes (0 PASS · 1 FAIL · 2 usage) follow the original; README.md lists the differences (the
//! clock-scan checks, no `policy_as_of`, every program started through its Rust launcher).
pub mod checks;
pub mod cli;
pub mod fsglob;
pub mod pyassert;
pub mod pytext;
pub mod runner;
pub mod toml;

pub use agent_runtime_support::pyvalue::{PyDict, PyValue};
pub use checks::{
    active_retirement_consumers, codex_discovery_checks, ordered_between_detail, policy_registry_check, shell_array,
    shipped_python_paths, tracked_scratch_paths, verify, verify_with, walk_strings, Hooks,
};
pub use runner::run;

/// A Python dict literal in insertion order.
pub fn dict(items: Vec<(&str, PyValue)>) -> PyValue {
    let mut d = PyDict::new();
    for (k, v) in items {
        d.set(k, v);
    }
    PyValue::Dict(d)
}

pub fn list_strs<S: AsRef<str>>(items: &[S]) -> PyValue {
    PyValue::List(items.iter().map(|s| PyValue::str(s.as_ref())).collect())
}

pub fn list_ints(items: &[i64]) -> PyValue {
    PyValue::List(items.iter().map(|&n| PyValue::int(n)).collect())
}

pub fn opt_str(v: &Option<String>) -> PyValue {
    match v {
        Some(s) => PyValue::str(s),
        None => PyValue::None,
    }
}

/// Mutable access to a dict value (for `check.update(...)`, which keeps an existing key's position).
pub fn update(target: &mut PyValue, items: Vec<(&str, PyValue)>) {
    if let PyValue::Dict(d) = target {
        for (k, v) in items {
            d.set(k, v);
        }
    }
}

pub fn get<'a>(v: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match v {
        PyValue::Dict(d) => d.get(key),
        _ => None,
    }
}
