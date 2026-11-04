//! The measurement parser: a Rust port of `.claude/skills/adversarial-benchmark/scripts/parse_bench.py`.
//!
//! [`measure`] turns a `vllm bench serve --save-result` document (already loaded as a [`PyValue`]) and an optional
//! engine log into a [`Measurement`]. Numeric conversions follow the Python program line by line, including the
//! order in which they run, because the order decides which failure is reported and which warnings were already
//! written when an uncaught exception ends the program:
//!
//! 1. `median_tpot_ms`, `mean_tpot_ms`, `spec_decode_acceptance_length` (`float`, `TypeError`/`ValueError` give
//!    `None`, `OverflowError` escapes), `completed`, `failed` (`int(x or 0)`, every error escapes);
//! 2. the engine log, whose problems are only warnings;
//! 3. the remaining `float` fields of the result.

use crate::engine_log::{decode_replacing, generation_throughputs};
use crate::pynumber::{float_call, float_from_str, int_call, round_to_two_places, PyException};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// Python truthiness of an optional float (`None` and `0.0` are false; NaN is true).
fn truthy(number: Option<f64>) -> bool {
    matches!(number, Some(value) if value != 0.0)
}

/// `d.get(key)`; any value that is not a dict has no `get`.
fn lookup<'a>(document: &'a PyValue, key: &str) -> Result<&'a PyValue, PyException> {
    match document {
        PyValue::Dict(map) => Ok(map.get(key).unwrap_or(&PyValue::None)),
        other => Err(PyException::new(
            "AttributeError",
            format!("'{}' object has no attribute 'get'", other.type_name()),
        )),
    }
}

/// `_f(d, k)`: a float, or `None` when the value is missing, `null` or not convertible.
fn float_field(document: &PyValue, key: &str) -> Result<Option<f64>, PyException> {
    let value = lookup(document, key)?;
    if matches!(value, PyValue::None) {
        return Ok(None);
    }
    match float_call(value) {
        Ok(number) => Ok(Some(number)),
        Err(error) if error.is_type_or_value_error() => Ok(None),
        Err(error) => Err(error),
    }
}

/// `int(d.get(key) or 0)`.
fn count_field(document: &PyValue, key: &str) -> Result<BigInt, PyException> {
    let value = lookup(document, key)?;
    if value.is_truthy() {
        int_call(value)
    } else {
        Ok(BigInt::zero())
    }
}

/// `round(1000.0 / tpot, 2)` when the time per token is a positive number.
fn decode_rate(tpot: Option<f64>) -> Option<f64> {
    match tpot {
        Some(value) if truthy(tpot) && value > 0.0 => Some(round_to_two_places(1000.0 / value)),
        _ => None,
    }
}

/// Engine throughput against the client's decode rate (`client_engine_agreement`).
#[derive(Clone, Debug, PartialEq)]
pub struct Agreement {
    pub engine_max: f64,
    pub client_decode_tps: f64,
    /// `round(engine_max / client_decode_tps, 2)`, absent when the quotient is falsy.
    pub ratio: Option<f64>,
}

/// The parsed measurement; field order is the order of the JSON the program prints.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    pub decode_tps: Option<f64>,
    pub decode_tps_mean: Option<f64>,
    pub output_throughput: Option<f64>,
    pub total_token_throughput: Option<f64>,
    pub ttft_ms_median: Option<f64>,
    pub ttft_ms_mean: Option<f64>,
    pub itl_ms_median: Option<f64>,
    pub tpot_ms_median: Option<f64>,
    pub accept_len: Option<f64>,
    pub spec_on: bool,
    pub completed: BigInt,
    pub failed: BigInt,
    pub max_concurrency: PyValue,
    pub num_prompts: PyValue,
    pub engine_gen_throughput_max: Option<f64>,
    pub client_engine_agreement: Option<Agreement>,
    pub measurement_ok: bool,
}

fn optional_float(number: Option<f64>) -> PyValue {
    number.map_or(PyValue::None, PyValue::Float)
}

impl Measurement {
    /// The result object in the program's key order.
    pub fn to_value(&self) -> PyValue {
        let mut out = PyDict::new();
        out.set("decode_tps", optional_float(self.decode_tps));
        out.set("decode_tps_mean", optional_float(self.decode_tps_mean));
        out.set("output_throughput", optional_float(self.output_throughput));
        out.set(
            "total_token_throughput",
            optional_float(self.total_token_throughput),
        );
        out.set("ttft_ms_median", optional_float(self.ttft_ms_median));
        out.set("ttft_ms_mean", optional_float(self.ttft_ms_mean));
        out.set("itl_ms_median", optional_float(self.itl_ms_median));
        out.set("tpot_ms_median", optional_float(self.tpot_ms_median));
        out.set("accept_len", optional_float(self.accept_len));
        out.set("spec_on", PyValue::Bool(self.spec_on));
        out.set("completed", PyValue::Int(self.completed.clone()));
        out.set("failed", PyValue::Int(self.failed.clone()));
        out.set("max_concurrency", self.max_concurrency.clone());
        out.set("num_prompts", self.num_prompts.clone());
        out.set(
            "engine_gen_throughput_max",
            optional_float(self.engine_gen_throughput_max),
        );
        out.set(
            "client_engine_agreement",
            match &self.client_engine_agreement {
                None => PyValue::None,
                Some(agreement) => {
                    let mut map = PyDict::new();
                    map.set("engine_max", PyValue::Float(agreement.engine_max));
                    map.set(
                        "client_decode_tps",
                        PyValue::Float(agreement.client_decode_tps),
                    );
                    map.set("ratio", optional_float(agreement.ratio));
                    PyValue::Dict(map)
                }
            },
        );
        out.set("measurement_ok", PyValue::Bool(self.measurement_ok));
        PyValue::Dict(out)
    }
}

/// Reads the engine log: its bytes, or the text of the `OSError` that stopped the read.
pub type EngineLogSource<'a> = &'a dyn Fn() -> Result<Vec<u8>, String>;

/// What a run produced: warnings in the order they were written, then the measurement or the exception the
/// Python program would have died with.
#[derive(Debug)]
pub struct Outcome {
    pub warnings: Vec<String>,
    pub result: Result<Measurement, PyException>,
}

/// The maximum throughput the log reports, `round(max(vals), 2)`; `Err` is the warning text.
fn engine_throughput_max(source: EngineLogSource<'_>) -> Result<Option<f64>, String> {
    let bytes = source()?;
    let text = decode_replacing(&bytes);
    let mut values = Vec::new();
    for found in generation_throughputs(&text) {
        let digits = PyStr::new(&found.replace(',', ""));
        values.push(float_from_str(&digits).map_err(|error| error.message)?);
    }
    let mut values = values.into_iter();
    let Some(mut best) = values.next() else {
        return Ok(None);
    };
    for value in values {
        if value > best {
            best = value;
        }
    }
    Ok(Some(round_to_two_places(best)))
}

fn agreement(engine_max: Option<f64>, decode_tps: Option<f64>) -> Option<Agreement> {
    let (Some(engine), Some(client)) = (engine_max, decode_tps) else {
        return None;
    };
    if !truthy(engine_max) || !truthy(decode_tps) {
        return None;
    }
    let ratio = engine / client;
    Some(Agreement {
        engine_max: engine,
        client_decode_tps: client,
        ratio: truthy(Some(ratio)).then(|| round_to_two_places(ratio)),
    })
}

/// Parses a loaded bench document. `engine_log` is consulted only after the first group of conversions has
/// succeeded, as the original opens the log at that point.
pub fn measure(document: &PyValue, engine_log: Option<EngineLogSource<'_>>) -> Outcome {
    let mut warnings = Vec::new();
    let result = measure_into(document, engine_log, &mut warnings);
    Outcome { warnings, result }
}

fn measure_into(
    document: &PyValue,
    engine_log: Option<EngineLogSource<'_>>,
    warnings: &mut Vec<String>,
) -> Result<Measurement, PyException> {
    let median_tpot = float_field(document, "median_tpot_ms")?;
    let mean_tpot = float_field(document, "mean_tpot_ms")?;
    let decode_tps = decode_rate(median_tpot);
    let decode_tps_mean = decode_rate(mean_tpot);
    let accept_len = float_field(document, "spec_decode_acceptance_length")?;
    let completed = count_field(document, "completed")?;
    let failed = count_field(document, "failed")?;
    let max_concurrency = lookup(document, "max_concurrency")?.clone();

    let mut engine_max = None;
    if let Some(source) = engine_log {
        match engine_throughput_max(source) {
            Ok(found) => engine_max = found,
            Err(message) => warnings.push(message),
        }
    }
    let client_engine_agreement = agreement(engine_max, decode_tps);

    let output_throughput = float_field(document, "output_throughput")?;
    let total_token_throughput = float_field(document, "total_token_throughput")?;
    let ttft_ms_median = float_field(document, "median_ttft_ms")?;
    let ttft_ms_mean = float_field(document, "mean_ttft_ms")?;
    let itl_ms_median = float_field(document, "median_itl_ms")?;
    let num_prompts = lookup(document, "num_prompts")?.clone();
    let measurement_ok = completed.is_positive() && failed.is_zero() && decode_tps.is_some();
    Ok(Measurement {
        decode_tps,
        decode_tps_mean,
        output_throughput,
        total_token_throughput,
        ttft_ms_median,
        ttft_ms_mean,
        itl_ms_median,
        tpot_ms_median: median_tpot,
        accept_len,
        spec_on: truthy(accept_len) && accept_len.is_some_and(|value| value > 1.05),
        completed,
        failed,
        max_concurrency,
        num_prompts,
        engine_gen_throughput_max: engine_max,
        client_engine_agreement,
        measurement_ok,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pyjson::loads;

    fn run(json: &str, log: Option<&'static [u8]>) -> Outcome {
        let document = loads(json).expect("fixture JSON");
        match log {
            None => measure(&document, None),
            Some(bytes) => {
                let source = move || Ok(bytes.to_vec());
                measure(&document, Some(&source))
            }
        }
    }

    const BASE: &str = r#"{"median_tpot_ms": 20.0, "mean_tpot_ms": 25.0, "output_throughput": 42.25, "total_token_throughput": 120.0, "median_ttft_ms": 5.0, "mean_ttft_ms": 7.0, "median_itl_ms": 21.0, "spec_decode_acceptance_length": 1.1, "completed": 4, "failed": 0, "max_concurrency": 1, "num_prompts": 4}"#;

    #[test]
    fn normal_document() {
        let outcome = run(BASE, None);
        let measurement = outcome.result.expect("measures");
        assert_eq!(measurement.decode_tps, Some(50.0));
        assert_eq!(measurement.decode_tps_mean, Some(40.0));
        assert!(measurement.spec_on && measurement.measurement_ok);
        assert!(outcome.warnings.is_empty());
        let printed = measurement
            .to_value()
            .dumps_pretty()
            .unwrap()
            .to_utf8()
            .unwrap();
        assert!(printed.starts_with("{\n  \"decode_tps\": 50.0,\n  \"decode_tps_mean\": 40.0,"));
        assert!(
            printed.ends_with("\"client_engine_agreement\": null,\n  \"measurement_ok\": true\n}")
        );
    }

    #[test]
    fn rounding_is_python_rounding() {
        let outcome = run(r#"{"median_tpot_ms": 8000, "completed": 1}"#, None);
        assert_eq!(outcome.result.unwrap().decode_tps, Some(0.12));
    }

    #[test]
    fn missing_is_not_zero() {
        let measurement = run("{}", None).result.unwrap();
        assert_eq!(measurement.decode_tps, None);
        assert_eq!(measurement.output_throughput, None);
        assert_eq!(measurement.completed, BigInt::zero());
        assert_eq!(measurement.num_prompts, PyValue::None);
        assert!(!measurement.measurement_ok);
    }

    #[test]
    fn a_failed_conversion_is_none_but_overflow_escapes() {
        let measurement = run(r#"{"median_tpot_ms": "bad", "mean_ttft_ms": [1]}"#, None)
            .result
            .unwrap();
        assert_eq!(measurement.tpot_ms_median, None);
        assert_eq!(measurement.ttft_ms_mean, None);
        let huge = format!("{{\"output_throughput\": 1{}}}", "0".repeat(400));
        let error = run(&huge, None).result.unwrap_err();
        assert_eq!(
            (error.class, error.message.as_str()),
            ("OverflowError", "int too large to convert to float")
        );
    }

    #[test]
    fn warnings_precede_a_later_escape() {
        let document =
            loads(r#"{"total_token_throughput": 1e999999999, "output_throughput": 1}"#).unwrap();
        let source = || Err("[Errno 2] No such file or directory: 'x'".to_string());
        let outcome = measure(&document, Some(&source));
        assert_eq!(
            outcome.warnings,
            ["[Errno 2] No such file or directory: 'x'"]
        );
        assert!(outcome.result.is_ok());
        let overflowing = loads(&format!(
            "{{\"total_token_throughput\": 1{}}}",
            "0".repeat(400)
        ))
        .unwrap();
        let outcome = measure(&overflowing, Some(&source));
        assert_eq!(outcome.warnings.len(), 1);
        assert!(outcome.result.is_err());
    }

    #[test]
    fn non_dict_documents_have_no_get() {
        let error = run("[1]", None).result.unwrap_err();
        assert_eq!(
            error.to_string(),
            "AttributeError: 'list' object has no attribute 'get'"
        );
        let error = run("null", None).result.unwrap_err();
        assert_eq!(error.message, "'NoneType' object has no attribute 'get'");
    }

    #[test]
    fn engine_log_maximum_and_agreement() {
        let outcome = run(
            BASE,
            Some(
                b"generation throughput: 10 tokens/s\ngeneration throughput: 1,200.125 tokens/s\n",
            ),
        );
        let measurement = outcome.result.unwrap();
        assert_eq!(measurement.engine_gen_throughput_max, Some(1200.12));
        let agreement = measurement.client_engine_agreement.unwrap();
        assert_eq!(
            (agreement.client_decode_tps, agreement.ratio),
            (50.0, Some(24.0))
        );
    }

    #[test]
    fn engine_conversion_failure_warns_and_keeps_the_main_result() {
        let outcome = run(
            BASE,
            Some(b"generation throughput: 10 tokens/s\ngeneration throughput: , tokens/s\n"),
        );
        assert_eq!(outcome.warnings, ["could not convert string to float: ''"]);
        let measurement = outcome.result.unwrap();
        assert_eq!(measurement.engine_gen_throughput_max, None);
        assert_eq!(measurement.decode_tps, Some(50.0));
    }

    #[test]
    fn count_conversions() {
        assert_eq!(
            run(r#"{"completed": true}"#, None)
                .result
                .unwrap()
                .completed,
            BigInt::from(1)
        );
        assert_eq!(
            run(r#"{"completed": 2.8, "failed": -0.8}"#, None)
                .result
                .unwrap()
                .completed,
            BigInt::from(2)
        );
        let error = run(r#"{"completed": "2.0"}"#, None).result.unwrap_err();
        assert_eq!(
            error.message,
            "invalid literal for int() with base 10: '2.0'"
        );
        let error = run(r#"{"failed": NaN}"#, None).result.unwrap_err();
        assert_eq!(error.message, "cannot convert float NaN to integer");
        assert_eq!(
            run(r#"{"completed": []}"#, None).result.unwrap().completed,
            BigInt::zero()
        );
        let error = run(r#"{"completed": [1]}"#, None).result.unwrap_err();
        assert_eq!(error.class, "TypeError");
    }
}
