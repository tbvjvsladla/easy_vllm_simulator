//! `parse_guidellm --self-test`: the G1..G29 scenarios of the original `_self_test`, run against the Rust library.
//!
//! The transcript lines, the order of the scenarios and the early exits are the original's: after G14, after G28
//! and after G29 any recorded failure ends the run with the failure list on stderr and status 1.
//!
//! G11 and G29 read the real GuideLLM 0.7.3 report of the owner's `fixtures/guidellm_benchmarks_sample.json` **at
//! run time** (nothing is compiled in, so deleting the sample fails G11 as it fails the original). Where that file
//! is: the launcher exports `PARSE_GUIDELLM_FIXTURE`; without it the owner layout is derived from the executable
//! (`<skill>/rust/target/release/parse_guidellm` finds `<skill>/fixtures/...`). Neither being a file is the
//! original's "fixture absent" failure.

use crate::guidellm::{build, inherit_accept_len, p99, BuildRequest, Failure, SpecAxis};
use crate::guidellm_cli::{prefix_cache_run, Termination};
use crate::pyjson::{decode_file_text, loads, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::selftest::SelfTestReport;
use num_bigint::BigInt;
use std::path::PathBuf;

const FIXTURE_ENV: &str = "PARSE_GUIDELLM_FIXTURE";
const FIXTURE_FILE: &str = "guidellm_benchmarks_sample.json";

#[derive(Default)]
struct Transcript {
    stdout: String,
    failures: Vec<String>,
}

impl Transcript {
    fn check(&mut self, name: &str, condition: bool, detail: &str) {
        if condition {
            self.stdout.push_str(&format!("  [PASS] {name}\n"));
        } else {
            self.stdout.push_str(&format!("  [FAIL] {name} {detail}\n"));
            self.failures.push(name.to_string());
        }
    }
}

/// How the scenarios ended: all passed, or a gate stopped the run with failures recorded.
type Run = Result<bool, Failure>;

// ── fixtures ────────────────────────────────────────────────────────────────────

const TOTALS_OK: &str = r#"{"successful":16,"errored":0,"incomplete":0,"total":16}"#;
const TPOT_25: &str = r#"{"successful":{"median":25.0,"mean":20.0}}"#;

/// The in-memory report of the original's `doc` with a chosen TPOT statistic, `request_totals` and an optional
/// `requests` object.
fn document(tpot: &str, totals: &str, requests: Option<&str>) -> PyValue {
    let requests = requests.map_or_else(String::new, |body| format!(",\"requests\":{body}"));
    loads(&format!(
        "{{\"metadata\":{{\"guidellm_version\":\"0.7.3\"}},\"benchmarks\":[{{\"config\":{{\"profile\":{{\"kind\":\"concurrent\",\"streams\":[4]}}}},\"metrics\":{{\"time_per_output_token_ms\":{tpot},\"time_to_first_token_ms\":{{\"successful\":{{\"median\":300.0,\"mean\":310.0}}}},\"inter_token_latency_ms\":{{\"successful\":{{\"median\":24.0,\"mean\":24.0}}}},\"output_tokens_per_second\":{{\"successful\":{{\"median\":38.0,\"mean\":39.0}}}},\"tokens_per_second\":{{\"successful\":{{\"median\":160.0,\"mean\":161.0}}}},\"request_concurrency\":{{\"successful\":{{\"median\":4.0,\"mean\":4.0}}}},\"request_totals\":{totals}}}{requests}}}]}}"
    ))
    .expect("self-test fixture JSON is valid")
}

fn report_with(tpot: &str, totals: &str) -> PyValue {
    document(tpot, totals, None)
}

const BOUNDARY_ERROR: &str = r#"{"info":{"error":"ValueError('Streaming response returned an error: Unexpected token 200002 while expecting start token 200006')"},"output_tokens":203}"#;
const SERVER_ERROR: &str =
    r#"{"info":{"error":"HTTPStatusError: 500 Internal Server Error"},"output_tokens":0}"#;
/// The pattern fits but no token was produced: a failure, not a boundary (both conditions must hold).
const FAKE_ERROR: &str =
    r#"{"info":{"error":"Unexpected token while expecting start token"},"output_tokens":0}"#;
const TOTALS_THREE_ERRORED: &str = r#"{"successful":13,"errored":3,"incomplete":0,"total":16}"#;

fn errored_report(records: [&str; 3]) -> PyValue {
    document(
        TPOT_25,
        TOTALS_THREE_ERRORED,
        Some(&format!("{{\"errored\":[{}]}}", records.join(","))),
    )
}

// ── value helpers (Python `==`, `is`, `in` on the outputs) ───────────────────────

fn field<'a>(out: &'a PyValue, key: &str) -> &'a PyValue {
    match out {
        PyValue::Dict(map) => map.get(key).unwrap_or(&PyValue::None),
        _ => &PyValue::None,
    }
}

fn equals_number(value: &PyValue, expected: f64) -> bool {
    match value {
        PyValue::Float(number) => *number == expected,
        PyValue::Int(number) => BigInt::from(expected as i64) == *number && expected.fract() == 0.0,
        PyValue::Bool(flag) => f64::from(u8::from(*flag)) == expected,
        _ => false,
    }
}

fn is_bool(value: &PyValue, expected: bool) -> bool {
    matches!(value, PyValue::Bool(flag) if *flag == expected)
}

fn is_none(value: &PyValue) -> bool {
    matches!(value, PyValue::None)
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().unwrap_or_default(),
        _ => String::new(),
    }
}

fn starts_with(value: &PyValue, prefix: &str) -> bool {
    text_of(value).starts_with(prefix)
}

fn equals_str(value: &PyValue, expected: &str) -> bool {
    matches!(value, PyValue::Str(text) if text.equals(expected))
}

fn close_to(value: &PyValue, expected: f64, tolerance: f64) -> bool {
    matches!(value, PyValue::Float(number) if (number - expected).abs() < tolerance)
}

fn int_is(value: &PyValue, expected: i64) -> bool {
    matches!(value, PyValue::Int(number) if *number == BigInt::from(expected))
}

fn built(
    doc: &PyValue,
    spec: SpecAxis,
    max_error_rate: f64,
    alive: Option<bool>,
) -> Result<PyValue, Failure> {
    build(
        doc,
        &BuildRequest {
            spec: spec.to_value(),
            max_error_rate: PyValue::Float(max_error_rate),
            server_alive_at_bench_end: alive.map_or(PyValue::None, PyValue::Bool),
            ..BuildRequest::default()
        },
    )
}

fn absent(doc: &PyValue) -> Result<PyValue, Failure> {
    built(doc, SpecAxis::declared_absent(), 0.0, None)
}

fn fixture_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os(FIXTURE_ENV).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    let executable = std::env::current_exe().ok()?;
    Some(
        executable
            .ancestors()
            .nth(4)?
            .join("fixtures")
            .join(FIXTURE_FILE),
    )
}

/// `json.load(open(sample))`: the real report, read now.
fn load_fixture(path: &PathBuf) -> Result<PyValue, Failure> {
    let bytes = std::fs::read(path).map_err(|error| {
        Failure::new(
            "OSError",
            &crate::cli::os_error_text(&error, &PyStr::new(&path.to_string_lossy())),
        )
    })?;
    decode_file_text(&bytes)
        .and_then(|text| loads_text(&text))
        .map_err(|error| Failure::new("ValueError", &error.to_string()))
}

enum Step {
    Key(&'static str),
    Index(usize),
}

/// A copy of `value` with the entry at `path` replaced (`doc["a"][0]["b"] = new` on a deep copy).
fn replaced(value: &PyValue, path: &[Step], new: PyValue) -> PyValue {
    let Some((step, rest)) = path.split_first() else {
        return new;
    };
    match (step, value) {
        (Step::Key(key), PyValue::Dict(map)) => {
            let mut copy = map.clone();
            let child = map.get(key).cloned().unwrap_or(PyValue::None);
            copy.set(key, replaced(&child, rest, new));
            PyValue::Dict(copy)
        }
        (Step::Index(index), PyValue::List(items)) => {
            let mut copy = items.clone();
            copy[*index] = replaced(&items[*index], rest, new);
            PyValue::List(copy)
        }
        _ => value.clone(),
    }
}

// ── scenarios ───────────────────────────────────────────────────────────────────

fn scenarios(t: &mut Transcript) -> Run {
    let doc = report_with(TPOT_25, TOTALS_OK);

    let out = absent(&doc)?;
    t.check(
        "G1 정본 지표 = 1000/median(TPOT)",
        equals_number(field(&out, "decode_tps"), 40.0)
            && equals_number(field(&out, "tpot_ms_median"), 25.0),
        "",
    );
    t.check(
        "G2 mean 도 median 과 같은 산정식",
        equals_number(field(&out, "decode_tps_mean"), 50.0),
        "",
    );
    let contract_keys = [
        "decode_tps",
        "ttft_ms_median",
        "itl_ms_median",
        "completed",
        "failed",
        "max_concurrency",
        "num_prompts",
        "measurement_ok",
        "spec_on",
        "accept_len",
    ];
    t.check(
        "G3 parse_bench 계약 키를 전부 낸다",
        matches!(&out, PyValue::Dict(map) if contract_keys.iter().all(|key| map.contains_key(key))),
        "",
    );
    t.check(
        "G4 도구 지문은 산출물에서 실측한다",
        equals_str(field(&out, "bench_tool"), "guidellm")
            && equals_str(field(&out, "bench_tool_version"), "0.7.3")
            && starts_with(field(&out, "bench_tool_version_source"), "measured("),
        "",
    );
    t.check(
        "G5 동시성은 profile.streams 에서 읽는다",
        equals_number(field(&out, "max_concurrency"), 4.0),
        "",
    );
    t.check(
        "G6 spec 축 부재는 출처로 남는다",
        is_none(field(&out, "accept_len"))
            && is_bool(field(&out, "spec_on"), false)
            && equals_str(field(&out, "spec_axis_source"), "declared-absent"),
        "",
    );

    let inherited = SpecAxis {
        accept_len: PyValue::Float(1.85),
        source: PyValue::str("inherited(lite vllm bench serve)"),
    };
    let out2 = built(&doc, inherited, 0.0, None)?;
    t.check(
        "G7 lite 승계 accept_len 이 spec_on 을 켠다",
        is_bool(field(&out2, "spec_on"), true)
            && equals_number(field(&out2, "accept_len"), 1.85)
            && starts_with(field(&out2, "spec_axis_source"), "inherited("),
        "",
    );

    let incomplete = report_with(
        TPOT_25,
        r#"{"successful":16,"errored":0,"incomplete":2,"total":16}"#,
    );
    let out3 = absent(&incomplete)?;
    t.check(
        "G8 incomplete 는 실패로 세되 개수를 따로 남긴다",
        int_is(field(&out3, "failed"), 2)
            && int_is(field(&out3, "incomplete"), 2)
            && int_is(field(&out3, "errored"), 0)
            && is_bool(field(&out3, "measurement_ok"), false),
        "",
    );

    let zero = report_with(r#"{"successful":{"median":0.0,"mean":0.0}}"#, TOTALS_OK);
    let out4 = absent(&zero)?;
    t.check(
        "G9 TPOT 0 은 무한대가 아니라 None(침묵 나눗셈 ✗)",
        is_none(field(&out4, "decode_tps")) && is_bool(field(&out4, "measurement_ok"), false),
        "",
    );

    for (broken, fragment) in [
        (loads(r#"{"benchmarks": []}"#).expect("literal"), "비었다"),
        (
            loads(r#"{"benchmarks": [{}]}"#).expect("literal"),
            "metrics",
        ),
    ] {
        let name = format!("G10 불량 문서 거부({fragment})");
        match build(
            &broken,
            &BuildRequest {
                spec: SpecAxis::declared_absent().to_value(),
                ..BuildRequest::default()
            },
        ) {
            Err(failure) if failure.is_value_error() => {
                let message = failure.message.to_utf8().unwrap_or_default();
                t.check(
                    &name,
                    message.contains(fragment),
                    &format!("(실제 {})", failure.message.repr()),
                );
            }
            Err(other) => return Err(other),
            Ok(_) => t.check(&name, false, "(예외가 나지 않았다)"),
        }
    }

    // The real report exists, so the contract also holds for it: a fixture must not be narrower than reality.
    let sample = fixture_path().filter(|path| path.is_file());
    let mut real = PyValue::None;
    match &sample {
        Some(path) => {
            real = absent(&load_fixture(path)?)?;
            t.check(
                "G11 실측 산출물 픽스처가 계약을 만족한다",
                !is_none(field(&real, "decode_tps"))
                    && is_bool(field(&real, "measurement_ok"), true)
                    && starts_with(field(&real, "bench_tool_version_source"), "measured("),
                "",
            );
        }
        None => t.check(
            "G11 실측 산출물 픽스처 존재",
            false,
            "(fixtures/guidellm_benchmarks_sample.json 부재)",
        ),
    }

    let one_error = report_with(
        TPOT_25,
        r#"{"successful":15,"errored":1,"incomplete":0,"total":16}"#,
    );
    let strict = absent(&one_error)?;
    let declared = built(&one_error, SpecAxis::declared_absent(), 0.0625, None)?;
    t.check(
        "G12 기본은 엄격 — 오류 1건이면 measurement_ok=False",
        is_bool(field(&strict, "measurement_ok"), false)
            && close_to(field(&strict, "error_rate"), 0.0625, 1e-9),
        "",
    );
    t.check(
        "G13 선언한 허용치 안이면 통과하되 오류율을 **항상 싣는다**",
        is_bool(field(&declared, "measurement_ok"), true)
            && matches!(field(&declared, "error_rate"), PyValue::Float(rate) if *rate > 0.0)
            && equals_number(field(&declared, "max_error_rate_declared"), 0.0625),
        "",
    );
    let over = built(&one_error, SpecAxis::declared_absent(), 0.01, None)?;
    t.check(
        "G14 ★음성대조 선언 허용치를 넘으면 여전히 False",
        is_bool(field(&over, "measurement_ok"), false),
        "",
    );

    if !t.failures.is_empty() {
        return Ok(false);
    }
    // ── G15~G19 오류 분류 ───────────────────────────────────────────────────────
    let boundary_only = errored_report([BOUNDARY_ERROR; 3]);
    let o = absent(&boundary_only)?;
    t.check(
        "G15 ★도구 경계 3건은 엄격 허용치에서도 measurement_ok 를 깨지 않는다",
        is_bool(field(&o, "measurement_ok"), true)
            && int_is(field(&o, "tool_boundary_errors"), 3)
            && int_is(field(&o, "server_errors"), 0)
            && close_to(field(&o, "error_rate"), 0.1875, 1e-9)
            && equals_number(field(&o, "server_error_rate"), 0.0),
        "",
    );
    let mixed = errored_report([BOUNDARY_ERROR, SERVER_ERROR, BOUNDARY_ERROR]);
    let o2 = absent(&mixed)?;
    t.check(
        "G16 ★음성대조 서버 오류가 섞이면 엄격 허용치에서 False",
        is_bool(field(&o2, "measurement_ok"), false)
            && int_is(field(&o2, "server_errors"), 1)
            && int_is(field(&o2, "tool_boundary_errors"), 2),
        "",
    );
    let fakes = errored_report([FAKE_ERROR; 3]);
    let o3 = absent(&fakes)?;
    t.check(
        "G17 ★패턴만 맞고 토큰 0 이면 경계가 아니다(두 조건 동시 충족)",
        is_bool(field(&o3, "measurement_ok"), false)
            && int_is(field(&o3, "server_errors"), 3)
            && int_is(field(&o3, "tool_boundary_errors"), 0),
        "",
    );
    let without_requests = document(TPOT_25, TOTALS_THREE_ERRORED, None);
    let o4 = absent(&without_requests)?;
    t.check(
        "G18 ★requests 부재면 가르지 않고 전량 서버 오류(fail-closed)",
        is_bool(field(&o4, "measurement_ok"), false)
            && is_none(field(&o4, "tool_boundary_errors"))
            && text_of(field(&o4, "error_split_source")).contains("unavailable"),
        "",
    );
    t.check(
        "G19 미분류 사유를 표본으로 남긴다(삼키지 않는다)",
        matches!(field(&o2, "unclassified_error_samples"), PyValue::List(samples)
            if samples.first().is_some_and(|first| text_of(first).contains("500"))),
        "",
    );

    // ── G20~G22 벤치 종료 시 서버 생존 ─────────────────────────────────────────
    let alive = built(&boundary_only, SpecAxis::declared_absent(), 0.0, Some(true))?;
    t.check(
        "G20 서버 생존이면 도구 경계 면제가 유지된다",
        int_is(field(&alive, "tool_boundary_errors"), 3)
            && starts_with(field(&alive, "boundary_exemption"), "enabled"),
        "",
    );
    let dead = built(
        &boundary_only,
        SpecAxis::declared_absent(),
        0.0,
        Some(false),
    )?;
    t.check(
        "G21 ★음성대조 서버가 죽었으면 면제 무효 — 전량 서버 오류로 센다",
        int_is(field(&dead, "tool_boundary_errors"), 0)
            && matches!((field(&dead, "server_error_rate"), field(&alive, "server_error_rate")),
                (PyValue::Float(dead_rate), PyValue::Float(alive_rate)) if dead_rate > alive_rate)
            && starts_with(field(&dead, "boundary_exemption"), "disabled"),
        "",
    );
    let unknown = absent(&boundary_only)?;
    t.check(
        "G22 관측이 없으면 모름으로 남긴다(False 로 접지 않는다)",
        is_none(field(&unknown, "server_alive_at_bench_end"))
            && int_is(field(&unknown, "tool_boundary_errors"), 3)
            && text_of(field(&unknown, "boundary_exemption")).contains("unobserved"),
        "",
    );

    // ── G23~G25 spec 승계원의 모양 ──────────────────────────────────────────────
    let warm = loads(r#"{"completed": 3, "failed": 0, "median_tpot_ms": 21.4, "spec_decode_acceptance_length": 2.2069}"#)
        .expect("literal");
    let g23 = inherit_accept_len(&warm, &PyValue::str("lite_warm_x.json"))?;
    t.check(
        "G23 bench 결과의 수용길이를 승계한다",
        equals_number(&g23.accept_len, 2.2069)
            && equals_str(&g23.source, "inherited(lite_warm_x.json)"),
        "",
    );
    let spec_off =
        loads(r#"{"completed": 3, "failed": 0, "median_tpot_ms": 30.0}"#).expect("literal");
    let g24 = inherit_accept_len(&spec_off, &PyValue::str("lite_warm_y.json"))?;
    t.check(
        "G24 spec 키 없는 정상 bench 결과(spec off)는 None 승계가 맞다",
        is_none(&g24.accept_len) && starts_with(&g24.source, "inherited("),
        "",
    );
    let pointer = loads(r#"{"topology": "multi", "burst_n": 3, "bench_warm_json": "lite_warm_x.json", "bench_cold_json": "lite_cold_x.json", "nodes": []}"#)
        .expect("literal");
    let name = "G25 ★음성대조 lite raw 포인터 문서는 거부한다(조용한 None 승계 ✗)";
    match inherit_accept_len(&pointer, &PyValue::str("lite_raw_x.json")) {
        Err(failure) if failure.is_value_error() => {
            let message = failure.message.to_utf8().unwrap_or_default();
            t.check(
                name,
                message.contains("bench_warm_json") && message.contains("vllm bench serve"),
                &format!("(실제 {})", failure.message.repr()),
            );
        }
        Err(other) => return Err(other),
        Ok(_) => t.check(name, false, "(예외가 나지 않았다)"),
    }

    // ── G26~G28 prefix cache 실행 차분 운반 ─────────────────────────────────────
    let mut ignored = PyStr::default();
    t.check(
        "G26 관측 파일 없음 = None(0 으로 접지 않는다)",
        matches!(prefix_cache_run(None, &mut ignored), Ok(PyValue::None)),
        "",
    );
    let directory = ScratchDirectory::create()?;
    let observed = directory.path("pc.json");
    directory.write(
        &observed,
        r#"{"provenance": "measured", "queries_delta": 100, "hits_delta": 90, "requests_delta": 18, "hit_rate_run": 0.9, "junk": 1}"#,
    )?;
    let mut warnings = PyStr::default();
    let record = carried(prefix_cache_run(
        Some(&PyStr::new(&observed)),
        &mut warnings,
    ))?;
    t.check(
        "G27 measured 운반 · 키 닫힌 목록 · 높은 적중률 경고",
        equals_number(field(&record, "hit_rate_run"), 0.9)
            && matches!(&record, PyValue::Dict(map) if !map.contains_key("junk"))
            && warnings.to_utf8().unwrap_or_default().contains("WARN"),
        &format!("({})", record.repr()),
    );
    directory.write(&observed, r#"{"provenance": "unobserved", "reason": "x"}"#)?;
    let mut warnings = PyStr::default();
    let record = carried(prefix_cache_run(
        Some(&PyStr::new(&observed)),
        &mut warnings,
    ))?;
    t.check(
        "G28 unobserved 는 값 없이 사유째 운반",
        matches!(&record, PyValue::Dict(map) if map.len() == 2
            && matches!(map.get("provenance"), Some(PyValue::Str(text)) if text.equals("unobserved"))
            && matches!(map.get("reason"), Some(PyValue::Str(text)) if text.equals("x"))),
        &format!("({})", record.repr()),
    );
    drop(directory);
    if !t.failures.is_empty() {
        return Ok(false);
    }

    // G29: the real GuideLLM 0.7.3 report proves seconds from the request timestamps.
    let path = sample.ok_or_else(|| {
        Failure::new(
            "TypeError",
            "expected str, bytes or os.PathLike object, not NoneType",
        )
    })?;
    let latency_doc = load_fixture(&path)?;
    let metrics_of = |doc: &PyValue| -> Option<crate::pyvalue::PyDict> {
        let PyValue::Dict(top) = doc else { return None };
        let PyValue::List(runs) = top.get("benchmarks")? else {
            return None;
        };
        let PyValue::Dict(run) = runs.first()? else {
            return None;
        };
        match run.get("metrics")? {
            PyValue::Dict(metrics) => Some(metrics.clone()),
            _ => None,
        }
    };
    let metrics = metrics_of(&latency_doc).ok_or_else(|| Failure::new("KeyError", "'metrics'"))?;
    let p99_s = p99(&metrics, "request_latency")?;
    let requests = successful_requests(&latency_doc)?;
    let matching = requests
        .iter()
        .find(|request| {
            matches!((field(request, "request_latency"), p99_s),
            (PyValue::Float(latency), Some(expected)) if *latency == expected)
        })
        .ok_or_else(|| Failure::new("StopIteration", ""))?;
    let elapsed_s = number(field(matching, "request_end_time"))?
        - number(field(matching, "request_start_time"))?;
    let latency_out = absent(&latency_doc)?;
    let mut cases_ok = true;
    let expected_cases: [(PyValue, Option<f64>); 6] = [
        (PyValue::None, None),
        (PyValue::Bool(true), None),
        (PyValue::Bool(false), None),
        (PyValue::str("1.25"), None),
        (PyValue::int(2), Some(2000.0)),
        (PyValue::Float(1.25), Some(1250.0)),
    ];
    for (value, expected) in expected_cases {
        let changed = replaced(
            &latency_doc,
            &[
                Step::Key("benchmarks"),
                Step::Index(0),
                Step::Key("metrics"),
                Step::Key("request_latency"),
                Step::Key("successful"),
                Step::Key("percentiles"),
                Step::Key("p99"),
            ],
            value,
        );
        let output = absent(&changed)?;
        let actual = field(&output, "request_latency_ms_p99");
        cases_ok = cases_ok
            && match expected {
                None => is_none(actual),
                Some(number) => equals_number(actual, number),
            };
    }
    let reported = field(&latency_out, "request_latency_ms_p99");
    t.check(
        "G29 request latency seconds→ms · real timestamps · None/numeric/bool contract",
        p99_s.is_some_and(|seconds| (elapsed_s - seconds).abs() < 1e-9)
            && matches!(reported, PyValue::Float(ms) if (ms - elapsed_s * 1000.0).abs() < 1e-6)
            && field(&latency_out, "decode_tps") == field(&real, "decode_tps")
            && cases_ok,
        "",
    );
    Ok(t.failures.is_empty())
}

fn number(value: &PyValue) -> Result<f64, Failure> {
    match value {
        PyValue::Float(number) => Ok(*number),
        PyValue::Int(number) => Ok(crate::pynumber::float_from_int(number)?),
        _ => Err(Failure::new(
            "TypeError",
            "unsupported operand type(s) for -",
        )),
    }
}

/// `latency_run["requests"]["successful"]`.
fn successful_requests(doc: &PyValue) -> Result<Vec<PyValue>, Failure> {
    let missing = |key: &str| Failure::new("KeyError", &PyStr::new(key).repr());
    let PyValue::Dict(top) = doc else {
        return Err(missing("benchmarks"));
    };
    let Some(PyValue::List(runs)) = top.get("benchmarks") else {
        return Err(missing("benchmarks"));
    };
    let Some(PyValue::Dict(run)) = runs.first() else {
        return Err(missing("benchmarks"));
    };
    let Some(PyValue::Dict(requests)) = run.get("requests") else {
        return Err(missing("requests"));
    };
    match requests.get("successful") {
        Some(PyValue::List(items)) => Ok(items.clone()),
        _ => Err(missing("successful")),
    }
}

fn carried(outcome: Result<PyValue, Termination>) -> Result<PyValue, Failure> {
    match outcome {
        Ok(value) => Ok(value),
        Err(Termination::Uncaught(failure)) => Err(failure),
        Err(Termination::Handled(line)) => Err(Failure::with_message("SystemExit", line)),
    }
}

/// `tempfile.TemporaryDirectory()` for G27/G28: removed when dropped.
struct ScratchDirectory(PathBuf);

impl ScratchDirectory {
    fn create() -> Result<Self, Failure> {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "parse_guidellm-selftest-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir(&path).map_err(|error| Failure::new("OSError", &error.to_string()))?;
        Ok(ScratchDirectory(path))
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_string_lossy().into_owned()
    }

    fn write(&self, path: &str, text: &str) -> Result<(), Failure> {
        std::fs::write(path, text).map_err(|error| Failure::new("OSError", &error.to_string()))
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Runs the self-test: the transcript, the failure list (stderr) and the status.
pub fn run_self_test() -> SelfTestReport {
    let mut transcript = Transcript::default();
    let outcome = scenarios(&mut transcript);
    let mut stderr = String::new();
    let exit_code = match outcome {
        Ok(true) => {
            transcript
                .stdout
                .push_str("[parse_guidellm --self-test] OK — G1~G29 전부 통과\n");
            0
        }
        Ok(false) => {
            let names: Vec<String> = transcript
                .failures
                .iter()
                .map(|name| PyStr::new(name).repr())
                .collect();
            stderr = format!(
                "[parse_guidellm --self-test] FAIL {} 건: [{}]\n",
                transcript.failures.len(),
                names.join(", ")
            );
            1
        }
        Err(failure) => {
            stderr = format!(
                "{}{}\n",
                crate::cli::TRACEBACK_HEADER,
                failure.line().to_utf8().unwrap_or_default()
            );
            1
        }
    };
    SelfTestReport {
        stdout: transcript.stdout,
        stderr,
        exit_code,
    }
}
