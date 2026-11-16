//! `manifest_contract --self-test`: the original's `_self_test()` (27 cases: the Flag gate, per-node `model_source`, the preloaded
//! asset path predicate and the TP derivation) translated over the native gate of this library. Each line is the original's
//! `PASS|FAIL <name> — <reason>`; the reasons are the native gate's own output, never a recorded answer.

use crate::manifest_gate::{
    evaluate_contract, manifest_tp_value, py_str, EXIT_MISSING_FIELD, EXIT_NO_FLAG, EXIT_OK,
};
use agent_runtime_support::pyvalue::{PyDict, PyValue};

fn d(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = PyDict::new();
    for (key, value) in pairs {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn i(number: i64) -> PyValue {
    PyValue::int(number)
}

fn with(base: &PyValue, pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut dict = match base {
        PyValue::Dict(dict) => dict.clone(),
        _ => PyDict::new(),
    };
    for (key, value) in pairs {
        dict.set(key, value);
    }
    PyValue::Dict(dict)
}

fn without(base: &PyValue, drop: &str) -> PyValue {
    let mut dict = PyDict::new();
    if let PyValue::Dict(source) = base {
        for (key, value) in source.iter() {
            if !key.equals(drop) {
                dict.insert(key.clone(), value.clone());
            }
        }
    }
    PyValue::Dict(dict)
}

fn terra(pairs: Vec<(&str, PyValue)>) -> PyValue {
    d(pairs)
}

fn node(role: &str) -> PyValue {
    d(vec![("role", s(role))])
}

struct Cases {
    rows: Vec<(String, bool, String)>,
}

impl Cases {
    fn push(&mut self, name: &str, ok: bool, reason: String) {
        self.rows.push((name.to_string(), ok, reason));
    }

    fn chk(&mut self, name: &str, man: &PyValue, topology: &str, flag: bool, exit: i32) {
        match evaluate_contract(man, topology) {
            Ok(r) => self.push(name, r.flag() == flag && r.exit_code() == exit, text(&r.reason())),
            Err(failure) => self.push(name, false, format!("{failure:?}")),
        }
    }

    fn eval_with(&mut self, name: &str, man: &PyValue, topology: &str, ok: impl Fn(&PyDict) -> bool) {
        match evaluate_contract(man, topology) {
            Ok(r) => {
                let reason = text(&r.reason());
                self.push(name, ok(&r.res), reason)
            }
            Err(failure) => self.push(name, false, format!("{failure:?}")),
        }
    }
}

fn text(value: &agent_runtime_support::pystr::PyStr) -> String {
    value.to_utf8().unwrap_or_else(|| value.repr())
}

fn warnings_mention(res: &PyDict, needle: &str) -> bool {
    match res.get("node_warnings").map(PyValue::peel) {
        Some(PyValue::List(items)) => items.iter().any(|w| text(&py_str(w)).contains(needle)),
        _ => false,
    }
}

fn warning_count(res: &PyDict) -> usize {
    match res.get("node_warnings").map(PyValue::peel) {
        Some(PyValue::List(items)) => items.len(),
        _ => 0,
    }
}

fn source_of(res: &PyDict, role: &str) -> Option<String> {
    match res.get("effective_model_source").map(PyValue::peel) {
        Some(PyValue::Dict(map)) => map.get(role).map(|v| text(&py_str(v))),
        _ => None,
    }
}

/// Runs the self-test: the printed text (each case line, a blank line, the count line) and the exit status.
pub fn run() -> (String, i32) {
    let mut c = Cases { rows: Vec::new() };
    let base_ok = d(vec![
        ("topology", s("single")),
        ("gpus_per_node", i(1)),
        ("model_source", s("managed")),
        ("nas_model_path", s("/mnt/models")),
        ("nodes", PyValue::List(vec![])),
        ("terraforming", terra(vec![("complete", PyValue::Bool(true)), ("branch_verified", PyValue::Bool(true))])),
    ]);
    c.chk("valid-single", &base_ok, "single", true, EXIT_OK);
    let sub_ok = with(
        &base_ok,
        vec![
            ("self_role", s("sub")),
            (
                "terraforming",
                terra(vec![
                    ("complete", PyValue::Bool(true)),
                    ("branch_verified", PyValue::Bool(true)),
                    ("issued_by", s("main")),
                ]),
            ),
        ],
    );
    c.chk("valid-sub-issued-by-main", &sub_ok, "single", true, EXIT_OK);
    for (label, issued) in [("missing", None), ("sub", Some("sub")), ("unknown", Some("unknown"))] {
        let mut t = vec![("complete", PyValue::Bool(true)), ("branch_verified", PyValue::Bool(true))];
        if let Some(by) = issued {
            t.push(("issued_by", s(by)));
        }
        let man = with(&base_ok, vec![("self_role", s("sub")), ("terraforming", terra(t))]);
        c.chk(&format!("sub-issued-by-{label}-rejected"), &man, "single", false, EXIT_NO_FLAG);
    }
    let multi_ok = d(vec![
        ("topology", s("multi")),
        ("gpus_per_node", i(1)),
        ("model_source", s("ephemeral")),
        ("nodes", PyValue::List(vec![node("main"), node("sub")])),
        ("terraforming", terra(vec![("complete", PyValue::Bool(true)), ("branch_verified", PyValue::Bool(true))])),
    ]);
    c.chk("valid-multi", &multi_ok, "multi", true, EXIT_OK);
    c.chk(
        "no-flag-complete-false",
        &with(&base_ok, vec![("terraforming", terra(vec![("complete", PyValue::Bool(false)), ("branch_verified", PyValue::Bool(true))]))]),
        "single",
        false,
        EXIT_NO_FLAG,
    );
    c.chk("no-flag-missing-block", &without(&base_ok, "terraforming"), "single", false, EXIT_NO_FLAG);
    c.chk(
        "no-flag-branch-unverified",
        &with(&base_ok, vec![("terraforming", terra(vec![("complete", PyValue::Bool(true)), ("branch_verified", PyValue::Bool(false))]))]),
        "single",
        false,
        EXIT_NO_FLAG,
    );
    c.chk("flag-but-missing-gpus", &with(&base_ok, vec![("gpus_per_node", PyValue::None)]), "single", false, EXIT_MISSING_FIELD);
    c.chk("flag-but-bad-model-source", &with(&base_ok, vec![("model_source", s("nas"))]), "single", false, EXIT_MISSING_FIELD);
    c.chk("flag-but-gpus-not-int(B4)", &with(&base_ok, vec![("gpus_per_node", s("abc"))]), "single", false, EXIT_MISSING_FIELD);
    c.chk("flag-but-gpus-zero(B4)", &with(&base_ok, vec![("gpus_per_node", i(0))]), "single", false, EXIT_MISSING_FIELD);

    let flag_terra = || terra(vec![("complete", PyValue::Bool(true)), ("branch_verified", PyValue::Bool(true))]);
    let single_sub_control = d(vec![
        ("topology", s("single")),
        ("gpus_per_node", i(1)),
        ("model_source", s("managed")),
        ("nas_model_path", s("/mnt/models")),
        ("nodes", PyValue::List(vec![node("main"), d(vec![("role", s("sub")), ("model_source", s("ephemeral"))])])),
        ("terraforming", flag_terra()),
    ]);
    c.eval_with("valid-override(single sub-control)", &single_sub_control, "single", |r| {
        matches!(r.get("flag").map(PyValue::peel), Some(PyValue::Bool(true)))
            && source_of(r, "sub").as_deref() == Some("ephemeral")
            && source_of(r, "main").as_deref() == Some("managed")
    });
    let bad_override = d(vec![
        ("topology", s("single")),
        ("gpus_per_node", i(1)),
        ("model_source", s("managed")),
        ("nas_model_path", s("/mnt/models")),
        ("nodes", PyValue::List(vec![node("main"), d(vec![("role", s("sub")), ("model_source", s("nas-bogus"))])])),
        ("terraforming", flag_terra()),
    ]);
    match evaluate_contract(&bad_override, "single") {
        Ok(r) => {
            let ok = !r.flag() && r.exit_code() == EXIT_MISSING_FIELD;
            c.push("bad-override(invalid per-node model_source)", ok, text(&r.reason()))
        }
        Err(f) => c.push("bad-override(invalid per-node model_source)", false, format!("{f:?}")),
    }

    let work = |role: &str| d(vec![("role", s(role)), ("work_dir", s("/home/u/proj"))]);
    let tik_local = d(vec![
        ("topology", s("single")),
        ("model_source", s("managed")),
        ("nas_model_path", s("/mnt/models")),
        ("gpus_per_node", i(1)),
        ("tiktoken_host_path", s("/home/u/proj/tiktoken_cache")),
        ("nodes", PyValue::List(vec![work("main"), work("sub")])),
        ("terraforming", flag_terra()),
    ]);
    c.eval_with("사전적재 자산 노드-로컬 → WARN", &tik_local, "single", |r| warnings_mention(r, "tiktoken_host_path"));
    let tik_shared = with(&tik_local, vec![("tiktoken_host_path", s("/mnt/models/OpenAI/harmony_tokenizer"))]);
    c.eval_with("공유 스토리지 → 무경고(대조군)", &tik_shared, "single", |r| !warnings_mention(r, "tiktoken_host_path"));
    let tik_solo = with(&tik_local, vec![("nodes", PyValue::List(vec![work("main")]))]);
    c.eval_with("서브 없는 단일노드 → 무경고(로컬이 정상)", &tik_solo, "single", |r| !warnings_mention(r, "tiktoken_host_path"));
    let multi_override_warn = d(vec![
        ("topology", s("multi")),
        ("gpus_per_node", i(1)),
        ("model_source", s("ephemeral")),
        ("nodes", PyValue::List(vec![node("main"), d(vec![("role", s("sub")), ("model_source", s("managed"))])])),
        ("terraforming", flag_terra()),
    ]);
    c.eval_with("multi-override(WARN, 무시 대상)", &multi_override_warn, "multi", |r| {
        matches!(r.get("flag").map(PyValue::peel), Some(PyValue::Bool(true))) && warning_count(r) >= 1
    });

    let tp_cases: Vec<(&str, PyValue, &str, PyValue)> = vec![
        ("tp-single-empty", d(vec![("gpus_per_node", i(4)), ("nodes", PyValue::List(vec![]))]), "single", i(4)),
        ("tp-single-with-sub(δ1-1)", d(vec![("gpus_per_node", i(1)), ("nodes", PyValue::List(vec![node("main"), node("sub")]))]), "single", i(1)),
        ("tp-single-with-sub-4gpu", d(vec![("gpus_per_node", i(4)), ("nodes", PyValue::List(vec![node("main"), node("sub")]))]), "single", i(4)),
        ("tp-multi-2x1", d(vec![("gpus_per_node", i(1)), ("nodes", PyValue::List(vec![i(1), i(2)]))]), "multi", i(2)),
        ("tp-multi-2x2", d(vec![("gpus_per_node", i(2)), ("nodes", PyValue::List(vec![i(1), i(2)]))]), "multi", i(4)),
        ("tp-empty-multi-none", d(vec![("gpus_per_node", i(1)), ("nodes", PyValue::List(vec![]))]), "multi", PyValue::None),
    ];
    for (name, man, topology, expect) in tp_cases {
        let PyValue::Dict(man) = man else { unreachable!() };
        match manifest_tp_value(&man, topology) {
            Ok(Ok(got)) => c.push(name, got == expect, format!("tp={} (기대 {})", got.repr(), expect.repr())),
            Ok(Err(why)) => c.push(name, false, format!("ValueError: {why}")),
            Err(f) => c.push(name, false, format!("{f:?}")),
        }
    }
    for (name, man) in [
        ("tp-missing-gpus-raises(B4)", d(vec![("nodes", PyValue::List(vec![]))])),
        ("tp-str-gpus-raises(B4)", d(vec![("gpus_per_node", s("two")), ("nodes", PyValue::List(vec![]))])),
    ] {
        let PyValue::Dict(man) = man else { unreachable!() };
        match manifest_tp_value(&man, "single") {
            Ok(Ok(got)) => c.push(name, false, format!("예외 없이 tp={} 반환(침묵 폴백 생존)", got.repr())),
            Ok(Err(why)) => c.push(name, true, format!("ValueError: {why}")),
            Err(f) => c.push(name, false, format!("{f:?}")),
        }
    }

    let passed = c.rows.iter().filter(|r| r.1).count();
    let mut out = String::new();
    for (name, ok, reason) in &c.rows {
        out.push_str(&format!("{} {} — {}\n", if *ok { "PASS" } else { "FAIL" }, name, reason));
    }
    out.push_str(&format!("\n{}/{} self-test 통과\n", passed, c.rows.len()));
    (out, if passed == c.rows.len() { 0 } else { 1 })
}
