//! `verdict_rule --self-test`: the T1..T23 scenarios of the original `_self_test`, run against the Rust gate.
//!
//! The scenario ids and output lines match the original so transcripts can be compared; failure details are
//! shortened to the scenario's own message.

use crate::pyjson::loads;
use crate::pyvalue::PyValue;
use crate::selftest::SelfTestReport;
use crate::verdict::{
    build_verdict, validate_cli, CliArguments, AUTHORITIES, REASON_SPEC_ACCEPT_LEN_MISSING, SRC_C,
    SRC_E, SRC_EXPECTED,
};

fn json(text: &str) -> PyValue {
    loads(text).expect("fixture JSON")
}

const FIX_MEASURED: &str = r#"{"decode_tps": 34.0, "spec_on": true, "accept_len": 1.85, "completed": 16, "failed": 0, "measurement_ok": true}"#;
const FIX_ROOFLINE: &str = r#"{"R_fp": 30.0, "R_token": 45.0, "expected_achievable": 12.6, "gpu_model": "NVIDIA GB10", "tp": 2}"#;

/// Keyword arguments of the original `_st_opts`.
#[derive(Clone)]
struct Kw {
    arguments: CliArguments,
    spec_supported: bool,
    e_search: &'static str,
}

fn kw() -> Kw {
    Kw {
        arguments: CliArguments::default(),
        spec_supported: false,
        e_search: "no",
    }
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

impl Kw {
    fn authority(mut self, v: &str) -> Self {
        self.arguments.authority = s(v);
        self
    }
    fn reference(mut self, v: f64) -> Self {
        self.arguments.reference_tps = PyValue::Float(v);
        self
    }
    fn target(mut self, v: f64) -> Self {
        self.arguments.target_tps = PyValue::Float(v);
        self
    }
    fn tolerance(mut self, v: f64) -> Self {
        self.arguments.tolerance = PyValue::Float(v);
        self
    }
    fn nodes(mut self, v: &str) -> Self {
        self.arguments.node_vram_gib = s(v);
        self
    }
    fn balance(mut self, v: f64) -> Self {
        self.arguments.balance_tol = PyValue::Float(v);
        self
    }
    fn spec(mut self, v: &str) -> Self {
        self.arguments.spec_declared = s(v);
        self
    }
    fn e_search(mut self, v: &'static str) -> Self {
        self.e_search = v;
        self
    }
}

/// `_st_opts`: options that really went through the validator (plus the two non-CLI keys).
fn st_opts(kw: &Kw) -> (PyValue, Vec<String>) {
    let (mut opts, errors) = validate_cli(&kw.arguments).expect("validation does not raise here");
    opts.set("spec_supported", PyValue::Bool(kw.spec_supported));
    opts.set("e_search", s(kw.e_search));
    (
        PyValue::Dict(opts),
        errors
            .iter()
            .map(|e| e.to_utf8().unwrap_or_default())
            .collect(),
    )
}

fn at<'a>(value: &'a PyValue, path: &[&str]) -> Option<&'a PyValue> {
    let mut current = value;
    for key in path {
        match current {
            PyValue::Dict(map) => current = map.get(key)?,
            _ => return None,
        }
    }
    Some(current)
}

fn is(value: &PyValue, path: &[&str], text: &str) -> bool {
    matches!(at(value, path), Some(PyValue::Str(own)) if own.equals(text))
}

fn num(value: &PyValue, path: &[&str]) -> Option<f64> {
    match at(value, path)? {
        PyValue::Float(n) => Some(*n),
        PyValue::Int(n) => n.to_string().parse().ok(),
        _ => None,
    }
}

fn flag(value: &PyValue, path: &[&str]) -> Option<bool> {
    match at(value, path)? {
        PyValue::Bool(b) => Some(*b),
        _ => None,
    }
}

fn is_null(value: &PyValue, path: &[&str]) -> bool {
    matches!(at(value, path), Some(PyValue::None))
}

fn list<'a>(value: &'a PyValue, path: &[&str]) -> &'a [PyValue] {
    match at(value, path) {
        Some(PyValue::List(items)) => items,
        _ => &[],
    }
}

struct Recorder {
    failures: Vec<String>,
}

impl Recorder {
    fn check(&mut self, id: &str, condition: bool, detail: &str) {
        if !condition {
            self.failures.push(format!("{id}: {detail}"));
        }
    }
}

fn run(measured: &str, roofline: &str, kw: &Kw) -> (Option<PyValue>, Vec<String>) {
    let (opts, errors) = st_opts(kw);
    if !errors.is_empty() {
        return (None, errors);
    }
    (
        Some(
            build_verdict(&json(measured), &json(roofline), &opts)
                .expect("self-test verdicts do not raise"),
        ),
        vec![],
    )
}

fn roof(accept: f64, source: Option<&str>, evidence: Option<&str>) -> String {
    let r_fp = 45.03;
    let round2 = |x: f64| (x * 100.0).round() / 100.0;
    let mut text = format!(
        r#"{{"R_fp": {r_fp}, "R_token": {}, "expected_achievable": {}, "accept_len": {accept:?}, "gpu_model": "NVIDIA GB10", "tp": 2, "bandwidth_gbps": 273.0"#,
        round2(accept * r_fp),
        round2(0.35 * accept * r_fp)
    );
    if let Some(source) = source {
        text.push_str(&format!(
            r#", "accept_len_source": "{source}", "accept_len_evidence": {}"#,
            evidence.map_or("null".to_string(), |e| format!("\"{e}\""))
        ));
    }
    text.push('}');
    text
}

const ACC: f64 = 2.481818181818182;
const M_ON: &str = r#"{"decode_tps": 46.7, "spec_on": true, "accept_len": 2.481818181818182, "completed": 16, "failed": 0, "measurement_ok": true}"#;
const M_ABSENT: &str = r#"{"decode_tps": 46.7, "spec_on": false, "accept_len": null, "completed": 16, "failed": 0, "measurement_ok": true}"#;
const M_OFF: &str = r#"{"decode_tps": 20.0, "spec_on": false, "accept_len": null, "completed": 16, "failed": 0, "measurement_ok": true}"#;

fn nr_spec(v: &Option<PyValue>) -> bool {
    v.as_ref().is_some_and(|v| {
        is(v, &["verdict"], "NEEDS_RUBRIC")
            && is(v, &["failure_axis"], "establish")
            && is(v, &["reason_code"], REASON_SPEC_ACCEPT_LEN_MISSING)
            && is_null(v, &["rubric", "floor"])
            && is_null(v, &["rubric", "primary"])
            && flag(v, &["rubric", "spec_axis", "missing"]) == Some(true)
    })
}

/// Runs T1..T23.
pub fn run_self_test() -> SelfTestReport {
    let mut rec = Recorder {
        failures: Vec::new(),
    };
    let verdict_is =
        |v: &Option<PyValue>, x: &str| v.as_ref().is_some_and(|v| is(v, &["verdict"], x));

    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().reference(41.0).target(30.0),
    );
    rec.check(
        "T1",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "REFUTE")
                    && is(v, &["rubric", "source"], SRC_E)
                    && num(v, &["rubric", "primary"]) == Some(41.0)
                    && num(v, &["rubric", "floor"]).is_some_and(|f| (f - 34.85).abs() < 1e-9)
            }),
        "weak 사다리 E 우선/REFUTE 실패",
    );
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().authority("explicit").reference(41.0).target(30.0),
    );
    rec.check(
        "T2",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "PASS")
                    && is(v, &["rubric", "source"], SRC_C)
                    && num(v, &["rubric", "floor"]).is_some_and(|f| (f - 25.5).abs() < 1e-9)
                    && num(v, &["rubric", "reference_E"]) == Some(41.0)
            }),
        "explicit c 낙찰/PASS/E 보존 실패",
    );
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().authority("explore").reference(41.0),
    );
    rec.check(
        "T3",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["rubric", "source"], SRC_E)
                    && is_null(v, &["rubric", "target_c"])
                    && is(v, &["rubric", "authority"], "explore")
                    && flag(v, &["rubric", "loop_until_done"]) == Some(false)
                    && list(v, &["rubric", "candidates"]).iter().any(|c| {
                        is(c, &["source"], SRC_C) && is(c, &["state"], "absent-by-authority")
                    })
            }),
        "explore 계약(E 낙찰·c 부재·loop 해제) 실패",
    );
    let (v, e) = run(FIX_MEASURED, FIX_ROOFLINE, &kw().authority("explore"));
    rec.check(
        "T4",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "PASS")
                    && is(v, &["rubric", "source"], SRC_EXPECTED)
                    && num(v, &["rubric", "primary"]) == Some(12.6)
            }),
        "explore expected 낙찰/PASS 실패",
    );
    rec.check(
        "T5",
        !st_opts(&kw().authority("explore").target(30.0))
            .1
            .is_empty(),
        "explore + --target-tps 가 거부되지 않았다",
    );
    rec.check(
        "T6",
        !st_opts(&kw().authority("explicit")).1.is_empty(),
        "explicit + target 부재가 거부되지 않았다(현행 fail-closed 회귀)",
    );
    for auth in AUTHORITIES {
        rec.check(
            &format!("T7[{auth}]"),
            !st_opts(&kw().authority(auth).target(0.0)).1.is_empty(),
            "--target-tps 0 이 거부되지 않았다(공허 PASS 재발)",
        );
    }
    rec.check(
        "T8",
        !st_opts(&kw().target(-5.0)).1.is_empty(),
        "--target-tps -5 가 거부되지 않았다",
    );
    rec.check(
        "T9",
        !st_opts(&kw().reference(0.0)).1.is_empty(),
        "--reference-tps 0 이 거부되지 않았다(E 칸 공허 PASS)",
    );
    let rf0 = FIX_ROOFLINE.replace("12.6", "0");
    let (v, e) = run(FIX_MEASURED, &rf0, &kw());
    rec.check(
        "T10",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "NEEDS_RUBRIC")
                    && is(v, &["failure_axis"], "establish")
                    && list(v, &["rubric", "candidates"])
                        .iter()
                        .any(|c| is(c, &["source"], SRC_EXPECTED) && is(c, &["state"], "invalid"))
            }),
        "expected=0 이 NEEDS_RUBRIC+invalid 기록으로 잡히지 않았다",
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        rec.check(
            &format!("T11[{bad}]"),
            !st_opts(&kw().target(bad)).1.is_empty(),
            "비유한 --target-tps 가 거부되지 않았다",
        );
    }
    let normals = [
        ("T12-a", kw().reference(41.0).target(30.0)),
        (
            "T12-b",
            kw().authority("explicit").reference(41.0).target(30.0),
        ),
        ("T12-c", kw().authority("explore").reference(41.0)),
        ("T12-d", kw().authority("explore")),
    ];
    for (id, k) in &normals {
        let (v, e) = run(FIX_MEASURED, FIX_ROOFLINE, k);
        rec.check(
            id,
            e.is_empty()
                && v.as_ref().is_some_and(|v| {
                    num(v, &["rubric", "floor"]).is_some_and(|f| f > 0.0)
                        && !is_null(v, &["rubric", "ratio_M_over_primary"])
                }),
            "불변식 위반(floor>0 ∧ ratio≠null)",
        );
    }
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().reference(41.0).nodes("60.1,66.8").balance(0.10),
    );
    rec.check(
        "T13",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["failure_axis"], "balance")
                    && is(v, &["verdict"], "REFUTE")
                    && flag(v, &["balance", "pass"]) == Some(false)
                    && flag(v, &["balance", "gates_verdict"]) == Some(true)
            }),
        "밸런스 축 회귀",
    );
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().reference(41.0).nodes("66.0,66.8").balance(0.10),
    );
    rec.check(
        "T13b",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                flag(v, &["balance", "pass"]) == Some(true) && !is(v, &["failure_axis"], "balance")
            }),
        "밸런스 축 정상범위 위양성",
    );
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw()
            .authority("explicit")
            .reference(41.0)
            .target(30.0)
            .nodes("60.1,66.8")
            .balance(0.10),
    );
    rec.check(
        "T13c",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "REFUTE")
                    && is(v, &["failure_axis"], "balance")
                    && flag(v, &["balance", "gates_verdict"]) == Some(true)
            }),
        "explicit 밸런스 게이트가 완화됐다(범위 밖 변경)",
    );
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().authority("explore").nodes("60.1,66.8").balance(0.10),
    );
    rec.check("T16", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && !is(v, &["failure_axis"], "balance") && flag(v, &["balance", "pass"]) == Some(false) && flag(v, &["balance", "gates_verdict"]) == Some(false) && !list(v, &["refuted_claims"]).iter().any(|c| matches!(at(c, &["claim"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.contains("VRAM")))) && list(v, &["diagnosis_hint"]).iter().any(|h| matches!(h, PyValue::Str(t) if t.to_utf8().is_some_and(|t| t.contains("서술"))))), "explore 밸런스 서술 계약 실패");
    let (v, e) = run(
        FIX_MEASURED,
        FIX_ROOFLINE,
        &kw().authority("explore").reference(41.0).nodes("60.1,66.8"),
    );
    rec.check(
        "T16b",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "REFUTE")
                    && is(v, &["failure_axis"], "meet")
                    && flag(v, &["balance", "pass"]) == Some(false)
                    && flag(v, &["balance", "gates_verdict"]) == Some(false)
            }),
        "explore 밸런스가 decode 축을 오염시켰다",
    );
    let (v, e) = run(FIX_MEASURED, FIX_ROOFLINE, &kw().authority("explore"));
    rec.check(
        "T16c",
        e.is_empty() && v.as_ref().is_some_and(|v| at(v, &["balance"]).is_none()),
        "node_vram 미지정인데 balance 키가 생겼다",
    );
    let (v, e) = run(FIX_MEASURED, FIX_ROOFLINE, &kw().reference(60.0));
    rec.check("T17", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "NEEDS_RUBRIC") && is(v, &["failure_axis"], "establish") && flag(v, &["rubric", "physical", "exceeds"]) == Some(true) && is_null(v, &["rubric", "floor"]) && is_null(v, &["rubric", "ratio_M_over_primary"]) && matches!(at(v, &["ask_user"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.contains("like-with-like")))), "물리 초과 문턱이 NEEDS_RUBRIC 이 아니다");
    let (v, e) = run(FIX_MEASURED, FIX_ROOFLINE, &kw().reference(44.0));
    rec.check(
        "T17b",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "REFUTE")
                    && is(v, &["failure_axis"], "meet")
                    && flag(v, &["rubric", "physical", "exceeds"]) == Some(false)
                    && !is_null(v, &["rubric", "floor"])
            }),
        "상한 이하 문턱이 종전 판정을 잃었다",
    );
    let md_off = FIX_MEASURED
        .replace("\"spec_on\": true", "\"spec_on\": false")
        .replace("1.85", "1.0");
    let (v, e) = run(&md_off, FIX_ROOFLINE, &kw().reference(44.0));
    rec.check("T17c", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "NEEDS_RUBRIC") && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_fp")))), "spec off 에서 R_fp 상한을 쓰지 않았다");
    for bad in ["null", "NaN", "Infinity"] {
        let md = FIX_MEASURED.replace("34.0", bad);
        let (v, e) = run(&md, FIX_ROOFLINE, &kw().reference(41.0));
        rec.check(
            &format!("T14[{bad}]"),
            e.is_empty() && verdict_is(&v, "INVALID"),
            "measured decode_tps 가 INVALID 로 가지 않았다",
        );
    }
    for bad in [1.0, 1.5, -0.1, f64::NAN] {
        rec.check(
            &format!("T15[{bad}]"),
            !st_opts(&kw().reference(41.0).tolerance(bad)).1.is_empty(),
            "--tolerance 가 거부되지 않았다(공허 PASS 두 번째 입구)",
        );
    }

    // T18..T23: spec axis
    let (v, e) = run(
        M_ABSENT,
        &roof(
            1.0,
            Some("absent"),
            Some("level_01/measured.json accept_len=None"),
        ),
        &kw()
            .authority("explore")
            .reference(53.7)
            .spec("on")
            .e_search("hit"),
    );
    rec.check("T18", e.is_empty() && nr_spec(&v) && matches!(v.as_ref().and_then(|v| at(v, &["reason"])), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.contains("expected_achievable"))), "spec on ∧ 결손이 SPEC_ACCEPT_LEN_MISSING 이 아니다");
    for bad in ["0.5", "NaN", "true"] {
        let rf = roof(1.0, Some("measured"), None)
            .replace("\"accept_len\": 1.0", &format!("\"accept_len\": {bad}"));
        let (v, e) = run(
            M_ABSENT,
            &rf,
            &kw().authority("explore").reference(53.7).spec("on"),
        );
        rec.check(
            &format!("T18b[{bad}]"),
            e.is_empty() && nr_spec(&v),
            "무효 accept_len 이 결손으로 잡히지 않았다",
        );
    }
    let (v, e) = run(
        M_ON,
        &roof(ACC, None, None),
        &kw().authority("explore").reference(53.7).spec("on"),
    );
    rec.check("T18c", e.is_empty() && nr_spec(&v) && matches!(v.as_ref().and_then(|v| at(v, &["rubric", "spec_axis", "basis"])), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.contains("부재"))), "출처 없는 accept_len 이 실측으로 승격됐다");
    let (v, e) = run(
        M_ON,
        &roof(ACC, Some("measured"), Some("level_01/measured.json")),
        &kw()
            .authority("explore")
            .reference(53.7)
            .spec("on")
            .e_search("hit"),
    );
    rec.check("T18d", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && is_null(v, &["reason_code"]) && num(v, &["rubric", "physical", "limit_tps"]).is_some_and(|x| (x - 111.75).abs() < 0.02) && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_token"))) && is(v, &["rubric", "spec_axis", "accept_len_source"], "measured") && num(v, &["rubric", "floor"]).is_some_and(|f| f > 0.0) && !is_null(v, &["rubric", "ratio_M_over_primary"])), "실측 승계가 R_token PASS 로 판정되지 않았다");
    let (v, e) = run(
        M_OFF,
        &roof(
            1.0,
            Some("declared-absent"),
            Some("sweep_index.meta.spec_declared=off"),
        ),
        &kw().authority("explore").spec("off").e_search("empty"),
    );
    rec.check("T19", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && is_null(v, &["reason_code"]) && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_fp"))) && flag(v, &["rubric", "spec_axis", "missing"]) == Some(false) && is(v, &["rubric", "spec_axis", "accept_len_source"], "declared-absent") && num(v, &["rubric", "floor"]).is_some_and(|f| f > 0.0)), "spec off 가 R_fp 정상 판정이 아니다");
    let (v, e) = run(
        M_ON,
        &roof(ACC, Some("measured"), None),
        &kw().authority("explore").reference(53.7).spec("off"),
    );
    rec.check("T19b", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_token"))) && matches!(at(v, &["rubric", "spec_axis", "mismatch"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("YES")))), "선언 off ↔ 실측 on 불일치가 측정 우선·기재로 처리되지 않았다");
    let roof_c = r#"{"R_fp": 30.0, "R_token": 90.0, "expected_achievable": 31.5, "accept_len": 3.0, "accept_len_source": "declared", "accept_len_evidence": "argv(--accept-len)"}"#;
    let (v, e) = run(M_OFF, roof_c, &kw().authority("explore").spec("off"));
    let basis = v
        .as_ref()
        .and_then(|v| at(v, &["rubric", "spec_axis", "basis"]))
        .map(|b| b.display_text().to_utf8().unwrap_or_default())
        .unwrap_or_default();
    let mismatch = v
        .as_ref()
        .and_then(|v| at(v, &["rubric", "spec_axis", "mismatch"]))
        .map(|b| b.display_text().to_utf8().unwrap_or_default())
        .unwrap_or_default();
    rec.check(
        "T19c",
        e.is_empty()
            && v.as_ref()
                .is_some_and(|v| is(v, &["rubric", "spec_axis", "limit"], "R_fp"))
            && !basis.contains("정상")
            && basis.contains("expected_achievable")
            && mismatch.starts_with("YES(declared=off · accept_len_source=declared"),
        "off ∧ 명시 accept_len>1 이 '1.0 이 정상' 으로 거짓 서술되거나 기재되지 않았다",
    );
    let (v, e) = run(
        M_OFF,
        &roof(1.0, Some("declared-absent"), None),
        &kw().authority("explore").spec("off"),
    );
    rec.check("T19d", e.is_empty() && v.as_ref().is_some_and(|v| is_null(v, &["rubric", "spec_axis", "mismatch"]) && matches!(at(v, &["rubric", "spec_axis", "basis"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.contains("정상")))), "off ∧ declared-absent 1.0 에 불일치가 잘못 기재됐다");
    let (v, e) = run(
        M_ABSENT,
        &roof(ACC, Some("declared"), Some("argv(--accept-len)")),
        &kw().authority("explore").reference(53.7).spec("on"),
    );
    rec.check("T20", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && is(v, &["rubric", "spec_axis", "accept_len_source"], "declared") && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_token")))), "declared provenance 가 R_token 판정으로 드러나지 않았다");
    let (v, e) = run(
        M_ON,
        &roof(ACC, None, None),
        &kw().authority("explore").reference(53.7),
    );
    rec.check("T21", e.is_empty() && v.as_ref().is_some_and(|v| is(v, &["verdict"], "PASS") && is(v, &["rubric", "spec_axis", "declared"], "unknown") && matches!(at(v, &["rubric", "physical", "limit_source"]), Some(PyValue::Str(t)) if t.to_utf8().is_some_and(|t| t.starts_with("R_token")))), "unknown 이 종전 동작(measured.spec_on → R_token)을 보존하지 않았다");
    let (v, e) = run(
        M_ON,
        &roof(1.0, None, None),
        &kw().authority("explore").reference(53.7),
    );
    rec.check(
        "T21b",
        e.is_empty()
            && v.as_ref().is_some_and(|v| {
                is(v, &["verdict"], "NEEDS_RUBRIC")
                    && is_null(v, &["reason_code"])
                    && flag(v, &["rubric", "physical", "exceeds"]) == Some(true)
            }),
        "조용한 1.0 의 결함 재현(물리 초과 NEEDS_RUBRIC)이 나오지 않았다",
    );
    rec.check(
        "T22",
        !st_opts(&kw().spec("maybe")).1.is_empty(),
        "--spec-declared 값역 밖이 거부되지 않았다",
    );
    for (id, md, rf, spec) in [
        ("T23-on", M_ON, roof(ACC, Some("measured"), None), "on"),
        (
            "T23-off",
            M_OFF,
            roof(1.0, Some("declared-absent"), None),
            "off",
        ),
        (
            "T23-decl",
            M_ABSENT,
            roof(ACC, Some("declared"), None),
            "on",
        ),
    ] {
        let (v, e) = run(md, &rf, &kw().authority("explore").spec(spec));
        rec.check(
            id,
            e.is_empty()
                && v.as_ref().is_some_and(|v| {
                    (is(v, &["verdict"], "PASS") || is(v, &["verdict"], "REFUTE"))
                        && num(v, &["rubric", "floor"]).is_some_and(|f| f > 0.0)
                        && !is_null(v, &["rubric", "ratio_M_over_primary"])
                }),
            "spec 축 경로에서 불변식 위반(floor>0 ∧ ratio≠null)",
        );
    }

    if rec.failures.is_empty() {
        return SelfTestReport {
            stdout: "[verdict --self-test] OK — T1~T23 전부 통과(공허 PASS 경로 부재 + explore 밸런스=서술 + spec 축 SPEC_ACCEPT_LEN_MISSING 단언 포함)\n".to_string(),
            stderr: String::new(),
            exit_code: 0,
        };
    }
    let mut stderr = format!("[verdict --self-test] FAIL {} 건:\n", rec.failures.len());
    for failure in &rec.failures {
        stderr.push_str(&format!("  - {failure}\n"));
    }
    SelfTestReport {
        stdout: String::new(),
        stderr,
        exit_code: 1,
    }
}
