//! `lite_metrics_api self-test`: native scenarios that evaluate the core operations for real and judge the
//! owner's own D1 fixture (`fixtures/lite_raw_d1_connection_refused.json` with its two bench result files beside
//! it), so a node bootstrap proves the installed program decides the way the contract says.
//!
//! The fixture is mandatory: its path comes from `LITE_METRICS_FIXTURE`, and a missing variable, file or sibling
//! is a failure, not a skipped check. The transcript says on every line that it is a self-test; the figures are
//! fixture values and unit arithmetic, never a measurement of a server.

use crate::lite_engine_log::{
    decode_log_text, first_kv_memory_group, kv_tokens_group, nontorch_group, weights_group,
};
use crate::lite_metrics::{
    accept_len_from_bench, build, cold_ttft_ms, gen_tps_from_bench, judge_lite, load_json,
    node_capacity, read_lite_verdict, render_multi, render_single, requests_ok, GenTpsSource,
    LiteVerdict, RequestsCheck, LITE_EXIT_CODES, LITE_VERDICT_NAMES,
};
use crate::pyjson::loads;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::selftest::SelfTestReport;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

const TAG: &str = "[lite_metrics self-test]";

struct Recorder {
    stdout: String,
    failures: Vec<String>,
}

impl Recorder {
    fn check(&mut self, name: &str, condition: bool) {
        let status = if condition { "PASS" } else { "FAIL" };
        if !condition {
            self.failures.push(name.to_string());
        }
        self.stdout.push_str(&format!("{TAG} {status} {name}\n"));
    }
}

fn document(json: &str) -> PyValue {
    loads(json).expect("fixture JSON is valid")
}

fn verdict_of(raw: &PyValue) -> Result<(LiteVerdict, i64), String> {
    let built = build(raw).map_err(|failure| failure.line().to_utf8().unwrap_or_default())?;
    let judgement = judge_lite(raw, &built.to_value())
        .map_err(|failure| failure.line().to_utf8().unwrap_or_default())?;
    Ok((judgement.verdict, judgement.verdict.exit_code()))
}

fn with(raw: &PyValue, key: &str, value: PyValue) -> PyValue {
    let mut map = match raw {
        PyValue::Dict(map) => map.clone(),
        _ => PyDict::new(),
    };
    map.set(key, value);
    PyValue::Dict(map)
}

/// The owner's D1 fixture with its bench file names resolved beside it, or why it cannot be used.
fn load_fixture(path: &Path) -> Result<PyValue, String> {
    let location = path.to_string_lossy().into_owned();
    let raw = load_json(&PyValue::str(&location))
        .map_err(|failure| failure.line().to_utf8().unwrap_or_default())?
        .ok_or_else(|| format!("fixture {location} is not readable JSON"))?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut resolved = match raw {
        PyValue::Dict(map) => map,
        _ => return Err(format!("fixture {location} is not a JSON object")),
    };
    for key in ["bench_cold_json", "bench_warm_json"] {
        let name = match resolved.get(key) {
            Some(PyValue::Str(name)) => name.to_utf8().unwrap_or_default(),
            _ => return Err(format!("fixture {location} declares no {key}")),
        };
        let sibling = parent.join(name);
        if !sibling.is_file() {
            return Err(format!("fixture sibling {} is missing", sibling.display()));
        }
        resolved.set(key, PyValue::str(&sibling.to_string_lossy()));
    }
    Ok(PyValue::Dict(resolved))
}

fn fixture_checks(rec: &mut Recorder, fixture: &PyValue) {
    let cold_completed = match fixture {
        PyValue::Dict(map) => map
            .get("bench_cold_json")
            .and_then(|path| load_json(path).ok().flatten()),
        _ => None,
    }
    .and_then(|bench| match bench {
        PyValue::Dict(map) => map.get("completed").cloned(),
        _ => None,
    });
    rec.check(
        "D1 fixture has the real shape: rc 0 while the result file says nothing completed",
        matches!(fixture, PyValue::Dict(map) if map.get("bench_cold_rc") == Some(&PyValue::int(0)))
            && cold_completed == Some(PyValue::int(0)),
    );
    rec.check(
        "D1 connection refused -> measurement_path_failed, exit 6 (a harness defect, not a server failure)",
        verdict_of(fixture) == Ok((LiteVerdict::MeasurementPathFailed, 6)),
    );
    rec.check(
        "D1 negative control: a reachable client plane does not turn the connection-error text into a server failure",
        verdict_of(&with(fixture, "client_plane_health", PyValue::str("200")))
            == Ok((LiteVerdict::MeasurementPathFailed, 6)),
    );
    rec.check(
        "D1 host plane dead too (server died while measured) -> server_failed, exit 7",
        verdict_of(&with(fixture, "host_health_after", PyValue::str("000")))
            == Ok((LiteVerdict::ServerFailed, 7)),
    );
}

fn core_checks(rec: &mut Recorder) {
    rec.check(
        "verdict vocabulary and exit codes: pass 0, measurement_path_failed 6, server_failed 7",
        LITE_VERDICT_NAMES == ["pass", "measurement_path_failed", "server_failed"]
            && LITE_EXIT_CODES
                == [
                    ("pass", 0),
                    ("measurement_path_failed", 6),
                    ("server_failed", 7),
                ],
    );
    rec.check(
        "read_lite_verdict: absent or out-of-vocabulary is None, a stored verdict is read back",
        read_lite_verdict(&PyValue::None).is_none()
            && read_lite_verdict(&document("{}")).is_none()
            && read_lite_verdict(&document(r#"{"lite_verdict": "ok"}"#)).is_none()
            && read_lite_verdict(&document(r#"{"lite_verdict": true}"#)).is_none()
            && read_lite_verdict(&document(r#"{"lite_verdict": "pass"}"#))
                == Some(LiteVerdict::Pass),
    );
    let tpot = gen_tps_from_bench(&document(
        r#"{"median_tpot_ms": 25.0, "output_throughput": 39.5}"#,
    ));
    let fallback = gen_tps_from_bench(&document(
        r#"{"median_tpot_ms": 0, "output_throughput": 39.555}"#,
    ));
    rec.check(
        "gen tokens/sec: 1000 / median_tpot_ms, else output_throughput rounded to two places",
        matches!(tpot, Ok(Some(gen)) if gen.tokens_per_second == 40.0 && gen.source == GenTpsSource::MedianTpot)
            && matches!(fallback, Ok(Some(gen)) if gen.tokens_per_second == 39.55
                && gen.source == GenTpsSource::OutputThroughput),
    );
    rec.check(
        "cold TTFT: median (else mean) when positive, else the first of ttfts",
        cold_ttft_ms(&document(
            r#"{"median_ttft_ms": 0, "mean_ttft_ms": 124.55}"#,
        )) == Ok(Some(124.5))
            && cold_ttft_ms(&document(r#"{"ttfts": ["122.55"]}"#)) == Ok(Some(122.5)),
    );
    rec.check(
        "accept length is the bench's own number: no rounding, a bool is not a number",
        matches!(
            accept_len_from_bench(&document(r#"{"spec_decode_acceptance_length": 1.2345}"#), &PyValue::str("warm")),
            Ok(Some(accept)) if accept.value == PyValue::Float(1.2345)
                && accept.source.equals("warm.spec_decode_acceptance_length")
        ) && matches!(
            accept_len_from_bench(&document(r#"{"spec_decode_acceptance_length": true}"#), &PyValue::str("warm")),
            Ok(None)
        ),
    );
    let node = document(
        r#"{"role": "main", "gpu_smi_used_mib": 8192, "gpu_smi_total_mib": 16384, "ram_total_kib": 33554432, "ram_avail_kib": 16777216}"#,
    );
    let engine = document(r#"{"reserved_total_gib": 34.25}"#);
    let unified = document("{}");
    rec.check(
        "node capacity: nvidia-smi figure, else the master's serve-log total, else N/A (never invented)",
        node_capacity(&node, &engine, &PyValue::Bool(false)).is_ok_and(|cap| {
            cap.gpu.equals("8.0 GiB (50%)") && cap.ram.equals("16.0 GiB (50%)")
        }) && node_capacity(&unified, &engine, &PyValue::Bool(true))
            .is_ok_and(|cap| cap.gpu.equals("~34.2 GiB (serve-log 분해; nvidia-smi N/A)"))
            && node_capacity(&unified, &engine, &PyValue::Bool(false)).is_ok_and(|cap| {
                cap.gpu.equals("N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음)")
                    && cap.ram.equals("N/A")
            }),
    );
    let cap = document(r#"{"gpu": "8 GiB", "ram": "16 GiB"}"#);
    let single = render_single(
        &PyValue::Float(40.0),
        &cap,
        &PyValue::Float(125.5),
        &PyValue::Float(20.0),
        &PyValue::int(16),
    );
    rec.check(
        "single table: five rows, KV share of the GPU total",
        single.is_ok_and(|table| {
            table.equals(
                "| 메트릭 | 값 |\n|---|---|\n| gen tokens/sec (warm) | 40.00 t/s |\n| cold-start TTFT | 126 ms |\n| GPU VRAM 점유 | 8 GiB |\n| KV cache 점유 | 20.0 GiB (125%) |\n| 시스템 RAM 점유 | 16 GiB |",
            )
        }),
    );
    let multi = render_multi(
        &PyValue::None,
        &PyValue::None,
        &document(r#"{"main": {"gpu": "8 GiB", "ram": "16 GiB"}, "sub": null}"#),
        &PyValue::None,
    );
    rec.check(
        "multi table: N/A where there is no value, the sub column marks the failed probe",
        multi.is_ok_and(|table| {
            let text = table.to_utf8().unwrap_or_default();
            text.contains("| gen tokens/sec (warm) [master] | N/A | — |")
                && text.contains("| GPU VRAM 점유 | 8 GiB | — (probe 실패) |")
        }),
    );
    rec.check(
        "request counts: all completed with tokens passes; a missing file, a count or a failure is rejected with its reason",
        requests_ok(&document(r#"{"completed": 1, "failed": 0, "total_output_tokens": 10}"#), &PyValue::int(1))
            == Ok(RequestsCheck::AllSucceeded)
            && requests_ok(&PyValue::None, &PyValue::int(1))
                == Ok(RequestsCheck::Rejected(PyStr::new("결과 파일 없음")))
            && requests_ok(&document(r#"{"completed": 2, "failed": 0, "total_output_tokens": 10}"#), &PyValue::int(1))
                == Ok(RequestsCheck::Rejected(PyStr::new("completed=2 ≠ 요청 1"))),
    );
    let log = decode_log_text(
        b"Available KV cache memory: 20.5 GiB\nGPU KV cache size: 154,192 tokens\nModel weights take 12.5 GiB\nnon-torch memory: 1.25 GiB\n",
    );
    rec.check(
        "engine log patterns: KV GiB, KV tokens, weights and non-torch figures",
        first_kv_memory_group(&log).as_deref() == Some("20.5")
            && kv_tokens_group(&log).as_deref() == Some("154,192")
            && weights_group(&log).as_deref() == Some("12.5")
            && nontorch_group(&log).as_deref() == Some("1.25"),
    );
    rec.check(
        "build refuses a missing topology or role instead of defaulting",
        build(&document("{}")).is_err_and(|failure| failure.class == "SystemExit")
            && build(&document(r#"{"topology": "single", "nodes": [{}]}"#))
                .is_err_and(|failure| failure.class == "SystemExit"),
    );
}

/// Runs every scenario; `fixture` is the value of `LITE_METRICS_FIXTURE`.
pub fn run_self_test(fixture: Option<OsString>) -> SelfTestReport {
    let mut rec = Recorder {
        stdout: String::new(),
        failures: Vec::new(),
    };
    core_checks(&mut rec);
    let loaded = match fixture {
        None => Err("LITE_METRICS_FIXTURE is not set (the D1 fixture is required)".to_string()),
        Some(path) => load_fixture(&PathBuf::from(path)),
    };
    match loaded {
        Ok(fixture) => fixture_checks(&mut rec, &fixture),
        Err(reason) => rec.check(&format!("D1 fixture available: {reason}"), false),
    }
    let failed = rec.failures.len();
    let mut stdout = rec.stdout;
    stdout.push_str(&format!(
        "{TAG} {} (this is a self-test of fixture values, not a measurement)\n",
        if failed == 0 { "ALL PASS" } else { "FAILED" }
    ));
    SelfTestReport {
        stdout,
        stderr: if failed == 0 {
            String::new()
        } else {
            format!(
                "{TAG} {failed} check(s) failed: {}\n",
                rec.failures.join("; ")
            )
        },
        exit_code: i32::from(failed != 0),
    }
}
