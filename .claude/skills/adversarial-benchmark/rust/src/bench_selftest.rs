//! `parse_bench self-test`: native scenarios for node bootstrap.
//!
//! The scenarios run the parser on in-memory fixtures only. The transcript says on every line that it is a
//! self-test; it never prints a measurement object, reads no bench file and measures no server, so it cannot be
//! mistaken for (or substituted by) measured serving evidence.

use crate::argv::{self, Outcome as Parse, BENCH_JSON, ENGINE_LOG, PARSE_BENCH};
use crate::parse_bench::{measure, Measurement};
use crate::pyjson::loads;
use crate::pystr::PyStr;
use crate::pyvalue::PyValue;
use crate::selftest::SelfTestReport;
use num_bigint::BigInt;

const TAG: &str = "[parse_bench self-test]";

struct Recorder {
    stdout: String,
    failures: Vec<String>,
    passed: usize,
}

impl Recorder {
    fn check(&mut self, name: &str, condition: bool) {
        if condition {
            self.passed += 1;
            self.stdout.push_str(&format!("{TAG} PASS {name}\n"));
        } else {
            self.failures.push(name.to_string());
            self.stdout.push_str(&format!("{TAG} FAIL {name}\n"));
        }
    }
}

fn document(json: &str) -> PyValue {
    loads(json).expect("fixture JSON is valid")
}

fn measured(json: &str, log: Option<&'static [u8]>) -> Option<Measurement> {
    let document = document(json);
    match log {
        None => measure(&document, None).result.ok(),
        Some(bytes) => {
            let source = move || Ok(bytes.to_vec());
            measure(&document, Some(&source)).result.ok()
        }
    }
}

fn error_text(json: &str) -> String {
    match measure(&document(json), None).result {
        Ok(_) => "no error".to_string(),
        Err(error) => error.to_string(),
    }
}

fn args(items: &[&str]) -> Vec<PyStr> {
    items.iter().map(|item| PyStr::new(item)).collect()
}

fn parse_error(items: &[&str]) -> String {
    match argv::parse_with(&PARSE_BENCH, &args(items)) {
        Parse::Error(message) => message.to_utf8().unwrap_or_default(),
        Parse::Help => "help".to_string(),
        Parse::Run(_) => "run".to_string(),
    }
}

/// Runs every scenario and returns the transcript.
pub fn run_self_test() -> SelfTestReport {
    let mut rec = Recorder {
        stdout: String::new(),
        failures: Vec::new(),
        passed: 0,
    };
    let base = r#"{"median_tpot_ms": 20.0, "mean_tpot_ms": 25.0, "completed": 4, "failed": 0, "spec_decode_acceptance_length": 1.1, "max_concurrency": 1}"#;

    let normal = measured(base, None);
    rec.check(
        "T1 1000/median_tpot -> decode_tps 50.0 · spec_on · measurement_ok",
        normal.as_ref().is_some_and(|m| {
            m.decode_tps == Some(50.0)
                && m.decode_tps_mean == Some(40.0)
                && m.spec_on
                && m.measurement_ok
        }),
    );
    rec.check(
        "T2 round(x, 2) is half-even on the exact double (0.125 -> 0.12, 2.675 -> 2.67)",
        measured(r#"{"median_tpot_ms": 8000, "completed": 1}"#, None)
            .is_some_and(|m| m.decode_tps == Some(0.12))
            && measured(base, Some(b"generation throughput: 2.675 tokens/s"))
                .is_some_and(|m| m.engine_gen_throughput_max == Some(2.67)),
    );
    rec.check(
        "T3 부재 ≠ 0 — 키가 없으면 null 이고 measurement_ok 는 거짓",
        measured("{}", None).is_some_and(|m| {
            m.decode_tps.is_none()
                && m.output_throughput.is_none()
                && m.completed == BigInt::from(0)
                && !m.measurement_ok
        }),
    );
    rec.check(
        "T4 float(): bool · 유니코드 숫자 · 밑줄 · 비수 문자열은 None",
        measured(r#"{"median_tpot_ms": "２０", "mean_tpot_ms": " 2_5 ", "mean_ttft_ms": true, "median_ttft_ms": "bad", "completed": 1}"#, None)
            .is_some_and(|m| {
                m.tpot_ms_median == Some(20.0)
                    && m.decode_tps_mean == Some(40.0)
                    && m.ttft_ms_mean == Some(1.0)
                    && m.ttft_ms_median.is_none()
            }),
    );
    rec.check(
        "T5 int(x or 0): 실수는 버림, 문자열 '2.0' 은 ValueError, 목록은 TypeError",
        measured(r#"{"completed": 2.8, "failed": "٣"}"#, None)
            .is_some_and(|m| m.completed == BigInt::from(2) && m.failed == BigInt::from(3))
            && error_text(r#"{"completed": "2.0"}"#)
                == "ValueError: invalid literal for int() with base 10: '2.0'"
            && error_text(r#"{"failed": [1]}"#).starts_with("TypeError: int() argument must be"),
    );
    let huge = format!("{{\"output_throughput\": 1{}}}", "0".repeat(400));
    rec.check(
        "T6 OverflowError 는 흡수되지 않는다 · 최상위가 객체가 아니면 AttributeError",
        error_text(&huge) == "OverflowError: int too large to convert to float"
            && error_text("[1]") == "AttributeError: 'list' object has no attribute 'get'",
    );
    rec.check(
        "T7 engine-log: 최댓값 · 쉼표 · 대소문자 무시(İ) · 유니코드 공백",
        measured(
            base,
            Some("GENERAT\u{130}ON THROUGHPUT:\u{a0}1,200.125\u{3000}TOKENS/S\ngeneration throughput: 10 tokens/s\n".as_bytes()),
        )
        .is_some_and(|m| m.engine_gen_throughput_max == Some(1200.12)),
    );
    let failing = measure(
        &document(base),
        Some(&|| Ok(b"generation throughput: , tokens/s\n".to_vec())),
    );
    rec.check(
        "T8 engine-log 변환 실패는 경고만 — 본 결과는 그대로 나온다",
        failing.warnings == ["could not convert string to float: ''"]
            && failing
                .result
                .as_ref()
                .is_ok_and(|m| m.decode_tps == Some(50.0) && m.engine_gen_throughput_max.is_none()),
    );
    let ordered = normal
        .map(|m| {
            m.to_value()
                .dumps_pretty()
                .map(|text| text.to_utf8().unwrap_or_default())
        })
        .and_then(Result::ok)
        .unwrap_or_default();
    rec.check(
        "T9 출력은 고정 키 순서의 pretty JSON (decode_tps 로 시작 · measurement_ok 로 끝남)",
        ordered.starts_with("{\n  \"decode_tps\": 50.0,")
            && ordered.ends_with("\"measurement_ok\": true\n}"),
    );
    rec.check(
        "T10 argparse: --bench-json 필수 · 약어 · 설명된 인자 오류",
        parse_error(&[]) == "the following arguments are required: --bench-json"
            && parse_error(&["--bench-json"]) == "argument --bench-json: expected one argument"
            && parse_error(&["--bench-json", "x", "--json=true"]) == "argument --json: ignored explicit argument 'true'"
            && matches!(
                argv::parse_with(&PARSE_BENCH, &args(&["--bench", "a.json", "--e", "b.log", "--j"])),
                Parse::Run(matches) if matches.value(BENCH_JSON).is_some_and(|v| v.equals("a.json"))
                    && matches.value(ENGINE_LOG).is_some_and(|v| v.equals("b.log")) && matches.flag(argv::JSON_FLAG)
            ),
    );
    rec.check(
        "T11 도움말은 터미널 폭에 따라 달라지고 프로그램 이름은 parse_bench.sh",
        PARSE_BENCH
            .usage(200)
            .starts_with("usage: parse_bench.sh [-h] --bench-json BENCH_JSON")
            && PARSE_BENCH.usage(30) != PARSE_BENCH.usage(200)
            && PARSE_BENCH.help(80).contains("--engine-log ENGINE_LOG"),
    );

    let Recorder {
        mut stdout,
        failures,
        passed,
    } = rec;
    if failures.is_empty() {
        stdout.push_str(&format!(
            "{TAG} OK — {passed} 시나리오 통과 (native fixture 점검일 뿐 · 서빙 측정 증거가 아니다)\n"
        ));
        return SelfTestReport {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        };
    }
    let names: Vec<String> = failures
        .iter()
        .map(|name| PyStr::new(name).repr())
        .collect();
    SelfTestReport {
        stdout,
        stderr: format!("{TAG} FAIL {} 건: [{}]\n", failures.len(), names.join(", ")),
        exit_code: 1,
    }
}
