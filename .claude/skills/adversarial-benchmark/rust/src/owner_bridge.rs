//! The owners the lite and sweep verifiers ask, called as Rust libraries (no interpreter, no session).
//!
//! The verifiers must see what the real owners answer, so none of their rules is copied here: every method below is one
//! call of the owner's own library function, and its value or exception is handed back in the verifiers' value model.
//!
//! | owner | library (path dependency) | original Python |
//! |---|---|---|
//! | hint reader | `hintlib::render_bench_section` (`.claude/skills/hint-publisher/rust`) | `render_bench_section.py` |
//! | evidence publisher | `policy_publisher` (`.claude/rust/policy-publisher`) | `evidence_publisher.py` |
//! | completion gate | `policy_gate::completion_gate` (`.claude/rust/policy-gate`) | `completion_gate.py` |
//!
//! This module used to start one Python interpreter (`owner_bridge.py`, a closed-operation socket session) because the
//! hint reader had not been ported. The reader is now the hint-publisher crate's own port, so the verifiers no longer
//! start Python and no longer require any `.py` file of the owners to exist (plan_26111615 pass 3). A reader exception
//! keeps its class (`BenchSectionFailure`, or the Python built-in the original lets through) and message; the class is
//! shown qualified the way a traceback would print it for the module name the original verifier imported it under.
//!
//! Every call is a real call: results are never cached or shared between callers.

use crate::conformance::Raised;
use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyjson::loads_text;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use hintlib::core::HintError;
use hintlib::render_bench_section as rbs;
use hintlib::value::J;

/// The class reported when a value of an owner cannot be carried into the verifiers' value model.
const BRIDGE_ERROR: &str = "OwnerBridgeError";

/// What the reader returned for a sweep report (`render_bench_section::parse_report`): the `level` of each parsed sweep
/// row, the `truncated` value (`None` when the parsed report has no such key) and `repr(parsed)` for a failure detail.
pub struct SweepReportParse {
    pub levels: PyValue,
    pub truncated: Option<PyValue>,
    pub shown: String,
}

fn py_text_of(value: &PyValue) -> String {
    value.display_text().to_utf8().unwrap_or_default()
}

fn bridge_error(message: &str) -> Raised {
    Raised::new(BRIDGE_ERROR, message)
}

/// The static class name of an exception an owner raised: the common names are fixed, any other is kept as it arrived
/// (an error path only; the leaked text is small and lives as long as the process).
fn exception_class(name: &str) -> &'static str {
    const KNOWN: [&str; 12] = [
        "AttributeError",
        "BenchSectionFailure",
        "IndexError",
        "KeyError",
        "OSError",
        "OverflowError",
        "RuntimeError",
        "TypeError",
        "UnicodeDecodeError",
        "ValueError",
        "ZeroDivisionError",
        "FileNotFoundError",
    ];
    KNOWN
        .iter()
        .find(|known| **known == name)
        .copied()
        .unwrap_or_else(|| Box::leak(name.to_string().into_boxed_str()))
}

/// The `Raised` of a reader failure. `module` is the name the original verifier imported the reader under: a traceback
/// prints a non-builtin class as `module.Class`.
fn raised_of(error: &HintError, module: &str) -> Raised {
    let class = error.code.strip_prefix("py:").unwrap_or(&error.code).to_string();
    let failure = Failure {
        class: exception_class(&class),
        message: PyStr::new(&error.message),
    };
    let raised = Raised::from_failure(failure);
    if class == "BenchSectionFailure" {
        raised.with_qualified_class(format!("{module}.{class}"))
    } else {
        raised
    }
}

/// A reader value in the verifiers' value model (through its JSON text: both models are JSON-shaped).
fn value_of(value: &J) -> Result<PyValue, Raised> {
    let text = hintlib::value::dumps_unicode(value);
    loads_text(&PyStr::new(&text))
        .map_err(|error| bridge_error(&format!("a reader value cannot be carried over: {error:?}")))
}

/// The module name the lite verifiers imported the reader under.
const LITE_MODULE: &str = "_lite_rbs";
/// The module name the sweep verifier's late import used.
const SWEEP_MODULE: &str = "render_bench_section";

/// The owners of one verification run. Holds no state: every method is a direct library call.
pub struct OwnerBridge {
    _private: (),
}

impl OwnerBridge {
    /// Binds the owners (nothing is started; binding a library cannot fail).
    pub fn start() -> Self {
        OwnerBridge { _private: () }
    }

    /// `rbs.parse_measurement_config(text)`.
    pub fn parse_measurement_config(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        match rbs::parse_measurement_config(text).map_err(|e| raised_of(&e, LITE_MODULE))? {
            None => Ok(PyValue::None),
            Some(table) => value_of(&table),
        }
    }

    /// `rbs.is_lite_report(text)`.
    pub fn is_lite_report(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<bool, Raised> {
        rbs::is_lite_report(text).map_err(|e| raised_of(&e, LITE_MODULE))
    }

    /// `rbs.lite_metrics_present(text)`.
    pub fn lite_metrics_present(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<bool, Raised> {
        rbs::lite_metrics_present(text).map_err(|e| raised_of(&e, LITE_MODULE))
    }

    /// `rbs.render_lite(rbs.parse_lite_report(text), name)`.
    pub fn render_lite(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
        name: &str,
    ) -> Result<PyValue, Raised> {
        let parsed = rbs::parse_lite_report(text).map_err(|e| raised_of(&e, LITE_MODULE))?;
        let rendered = rbs::render_lite(&parsed, name).map_err(|e| raised_of(&e, LITE_MODULE))?;
        Ok(PyValue::str(&rendered))
    }

    /// `len(rbs.parse_report(text)["levels"])`.
    pub fn parse_report_level_count(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        let parsed = rbs::parse_report(text).map_err(|e| raised_of(&e, LITE_MODULE))?;
        match &parsed {
            J::Dict(map) => match map.get("levels") {
                Some(J::List(levels)) => Ok(PyValue::int(levels.len() as i64)),
                _ => Err(bridge_error("the reader's parsed report has no `levels` list")),
            },
            _ => Err(bridge_error("the reader's parsed report is not a record")),
        }
    }

    /// The reader check of `selftest_sweep_repeats` (N4): parses the report. The reader's exception comes back as the
    /// inner `Err` (an `Exception` by its hierarchy, so the check fails and the run continues, as in the original); the
    /// outer `Err` is a value that cannot be carried over.
    pub fn parse_sweep_report(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<Result<SweepReportParse, Raised>, Raised> {
        let parsed = match rbs::parse_report(text) {
            Ok(parsed) => parsed,
            Err(error) => return Ok(Err(raised_of(&error, SWEEP_MODULE))),
        };
        let value = value_of(&parsed)?;
        let PyValue::Dict(map) = value.peel() else {
            return Err(bridge_error("the reader's parsed report is not a record"));
        };
        let levels = match map.get("levels").map(PyValue::peel) {
            Some(PyValue::List(rows)) => PyValue::List(
                rows.iter()
                    .map(|row| match row.peel() {
                        PyValue::Dict(row) => row.get("level").cloned().unwrap_or(PyValue::None),
                        _ => PyValue::None,
                    })
                    .collect(),
            ),
            _ => return Err(bridge_error("the reader's parsed report has no `levels` list")),
        };
        Ok(Ok(SweepReportParse {
            levels,
            truncated: map.get("truncated").cloned(),
            shown: value.repr(),
        }))
    }

    /// A contract constant of the reader (`MEASUREMENT_CONFIG_TITLE`, `MEASUREMENT_CONFIG_KEYS`, `LITE_SECTION_TITLE`,
    /// `REPORT_HEADER`); any other name is refused, as the closed operation list of the former session refused it.
    pub fn reader_constant(
        &mut self,
        _console: &mut dyn Console,
        name: &str,
    ) -> Result<PyValue, Raised> {
        match name {
            "MEASUREMENT_CONFIG_TITLE" => Ok(PyValue::str(rbs::MEASUREMENT_CONFIG_TITLE)),
            "MEASUREMENT_CONFIG_KEYS" => Ok(PyValue::Tuple(
                rbs::MEASUREMENT_CONFIG_KEYS.iter().map(|key| PyValue::str(key)).collect(),
            )),
            "LITE_SECTION_TITLE" => Ok(PyValue::str(rbs::LITE_SECTION_TITLE)),
            "REPORT_HEADER" => Ok(PyValue::str(rbs::REPORT_HEADER)),
            other => Err(Raised::new(
                "ValueError",
                &format!("not an owner constant of the reader: {}", PyStr::new(other).repr()),
            )),
        }
    }

    /// `ep._BENCH_SRC_NAME_RE[kind].match(name) is not None` — the Rust evidence publisher library.
    pub fn publisher_name_matches(
        &mut self,
        _console: &mut dyn Console,
        kind: &str,
        name: &str,
    ) -> Result<bool, Raised> {
        Ok(policy_publisher::bench_src_name_matches(kind, name))
    }

    /// `ep.report_measurement_config(text)[0]` — the Rust evidence publisher library. The table (str / int / None cells)
    /// crosses the two value models through its JSON text.
    pub fn publisher_measurement_config(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<PyValue, Raised> {
        match policy_publisher::report_measurement_config(text).0 {
            None => Ok(PyValue::None),
            Some(table) => {
                let json = table.dumps_unicode().map_err(|e| {
                    bridge_error(&format!("measurement table is not JSON-safe: {e:?}"))
                })?;
                loads_text(&PyStr::new(&json.to_utf8().unwrap_or_default())).map_err(|e| {
                    bridge_error(&format!("measurement table JSON reload failed: {e:?}"))
                })
            }
        }
    }

    /// A contract constant of the publisher (`LITE_BENCH_MODE`) — the Rust library.
    pub fn publisher_constant(
        &mut self,
        _console: &mut dyn Console,
        name: &str,
    ) -> Result<PyValue, Raised> {
        match name {
            "LITE_BENCH_MODE" => Ok(PyValue::str(policy_publisher::LITE_BENCH_MODE)),
            other => Err(bridge_error(&format!("unknown publisher constant `{other}`"))),
        }
    }

    /// `gate.parse_flat_certificate(text)` as `(fields, ok)` — the Rust completion gate library.
    pub fn parse_flat_certificate(
        &mut self,
        _console: &mut dyn Console,
        text: &str,
    ) -> Result<(PyValue, bool), Raised> {
        let (fields, ok) = policy_gate::completion_gate::parse_flat_certificate(text);
        let mut dict = PyDict::new();
        for (key, value) in &fields.0 {
            dict.set(key, PyValue::str(value));
        }
        Ok((PyValue::Dict(dict), ok))
    }

    /// `gate.certificate_run_key(fields) is not None` — the Rust completion gate library.
    pub fn certificate_run_key_present(
        &mut self,
        _console: &mut dyn Console,
        fields: &PyValue,
    ) -> Result<bool, Raised> {
        let mut flat = policy_gate::completion_gate::Fields::default();
        if let PyValue::Dict(dict) = fields.peel() {
            for (key, value) in dict.iter() {
                if !matches!(value.peel(), PyValue::None) {
                    flat.0.push((py_text_of(&PyValue::Str(key.clone())), py_text_of(value)));
                }
            }
        }
        Ok(policy_gate::completion_gate::certificate_run_key(&flat).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_failure_keeps_its_class_and_the_module_qualified_name() {
        let raised = raised_of(&HintError::py("BenchSectionFailure", "boom"), SWEEP_MODULE);
        assert_eq!(raised.failure().class, "BenchSectionFailure");
        assert!(raised.is_exception());
        let builtin = raised_of(&HintError::py("ValueError", "v"), LITE_MODULE);
        assert_eq!(builtin.failure().class, "ValueError");
    }

    #[test]
    fn a_report_without_the_measurement_section_is_none() {
        let parsed = rbs::parse_measurement_config("no table").expect("parse");
        assert!(parsed.is_none());
    }
}
