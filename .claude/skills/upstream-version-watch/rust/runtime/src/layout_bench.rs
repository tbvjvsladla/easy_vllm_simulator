//! The frozen measurement of the layout functions: the fixture rows of the resolver's measurement ([`crate::bench`]: one real preflight per row with
//! the whole DTO compared before any timer, then whole public calls plus the creation and drop of the same DTO inside each timer, the report only
//! after every series passed), with the arguments as DTOs.
//!
//! A row is `{metric, operation, args_dto, kwargs_dto, globals?, iterations?, expected}` with `operation` `check_spec_layout.<function>` and
//! `expected` the whole DTO of a saved original result. The DTO of the result (every dict, tuple, list and set) is built inside the timed call.

use crate::api::Capture;
use crate::bench::{benchmark_error, iterations_of, text_of, Series};
use crate::dto::encode_owned;
use crate::dto_decode::{decode_args, decode_kwargs};
use crate::host::YamlProfile;
use crate::layout_api::{call, resolve_operation, Globals};
use crate::layout_files::LayoutWorld;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::bench::Case;
use recipe_runtime::pyexpr::Res;

/// Reads the fixture: a JSON array of rows, parsed once. `columns` is the process's terminal width.
pub fn load(source: &str, columns: Option<String>) -> Res<Vec<Series>> {
    let PyValue::List(rows) =
        loads(source).map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
    else {
        return Err(benchmark_error("the fixture is not an array".to_string()));
    };
    let mut series = Vec::new();
    for row in rows {
        let PyValue::Dict(members) = row else {
            return Err(benchmark_error(
                "a fixture row is not an object".to_string(),
            ));
        };
        let metric = text_of(&members, "metric")?;
        let function = resolve_operation(&text_of(&members, "operation")?).map_err(|raised| {
            benchmark_error(format!(
                "{metric}: {}",
                raised.report().to_utf8().unwrap_or_default().trim_end()
            ))
        })?;
        let args = match members.get("args_dto") {
            None => Vec::new(),
            Some(dto) => decode_args(dto)
                .map_err(|problem| benchmark_error(format!("{metric}: args_dto: {problem}")))?,
        };
        let kwargs = match members.get("kwargs_dto") {
            None => PyDict::new(),
            Some(dto) => decode_kwargs(dto)
                .map_err(|problem| benchmark_error(format!("{metric}: kwargs_dto: {problem}")))?,
        };
        let mut globals = Globals::defaults();
        if let Some(given) = members.get("globals") {
            globals
                .replace_from(given)
                .map_err(|problem| benchmark_error(format!("{metric}: {problem}")))?;
        }
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let iterations = iterations_of(&members, &metric)?;
        let columns = columns.clone();
        let case = Case {
            metric,
            call: Box::new(move || {
                let mut capture = Capture::new(YamlProfile::Available, false);
                let value = call(
                    function,
                    &mut LayoutWorld::new(&mut capture, &globals.argv, columns.as_deref()),
                    &args,
                    &kwargs,
                )
                .map_err(|thrown| {
                    Failure::with_message("BenchmarkError", thrown.raised.report())
                })?;
                encode_owned(value).map_err(benchmark_error)
            }),
            expected,
        };
        series.push(Series { case, iterations });
    }
    Ok(series)
}
