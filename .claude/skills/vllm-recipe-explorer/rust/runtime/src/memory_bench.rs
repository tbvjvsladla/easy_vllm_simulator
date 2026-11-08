//! The frozen post-quality measurement of the RAM-gate functions: what the benchmark program runs, over **controlled** inputs.
//!
//! A fixture row is `{metric, operation, args, kwargs, context, expected}`: the function (`preload_ram_gate.gate` or
//! `preload_ram_gate.mem_available_mib`), its arguments, the controlled machine it runs against, and the **expected whole value**
//! taken from a saved original result. The machine is one of two things and nothing else:
//!
//! * `{"mem": N}`: a fixed memory figure that every call reads again (nothing is consumed, so a call repeated a thousand times sees
//!   what the first one saw), and no cache helper;
//! * `{"meminfo_file": PATH}`: the real file parser over that regular file, opened afresh by every call.
//!
//! The report is timing of controlled calls. It is not a RAM check of the machine it runs on and never approves a load: the
//! machine of a row is declared by the fixture, and the real `/proc/meminfo`, the real helper and the real `sudo` are not reachable
//! from here. The protocol is that of the other benchmarks: every row is called once before any timer and its complete value is
//! compared with the expected one, a failing call ends the run, each measured call's complete value is kept alive and dropped inside
//! the timer, and nothing is reported unless every series succeeded (see [`crate::bench::run_cases`]).

use crate::bench::Case;
use crate::memory::{
    mem_available_mib, DropSystem, HelperRequest, HelperRun, Host, MeminfoSource, Provider,
};
use crate::memory_api::{resolve, Environment};
use crate::memory_system::FileMeminfo;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::Read;

/// The controlled machine of one row.
enum Machine {
    Fixed(PyValue),
    File(FileMeminfo),
}

impl MeminfoSource for Machine {
    fn open(&mut self) -> Res<Box<dyn Read>> {
        match self {
            Machine::File(file) => file.open(),
            Machine::Fixed(_) => Err(Failure::new(
                "BenchmarkError",
                "a fixed-memory row has no meminfo file",
            )),
        }
    }
}

impl DropSystem for Machine {
    fn helper_exists(&mut self) -> bool {
        false
    }

    fn run_helper(&mut self, _request: &HelperRequest) -> HelperRun {
        HelperRun::Failed
    }
}

impl Provider for Machine {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        match self {
            Machine::Fixed(value) => Ok(value.clone()),
            Machine::File(_) => mem_available_mib(self),
        }
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        Ok(false)
    }
}

impl Host for Machine {
    fn write_stderr(&mut self, _text: &PyStr) -> Res<()> {
        Ok(())
    }

    fn call_log(&mut self, _message: &PyStr) -> Res<()> {
        Ok(())
    }
}

impl Environment for Machine {
    fn meminfo(&mut self) -> &mut dyn MeminfoSource {
        self
    }

    fn drops(&mut self) -> &mut dyn DropSystem {
        self
    }
}

fn benchmark_error(message: String) -> Failure {
    Failure::new("BenchmarkError", &message)
}

fn text_of(members: &PyDict, key: &str) -> Res<String> {
    match members.get(key) {
        Some(PyValue::Str(own)) => own
            .to_utf8()
            .ok_or_else(|| benchmark_error(format!("{key} is not text"))),
        _ => Err(benchmark_error(format!(
            "a fixture row has no text member {key}"
        ))),
    }
}

fn machine_of(metric: &str, members: &PyDict) -> Res<(&'static str, PyValue)> {
    let Some(PyValue::Dict(context)) = members.get("context") else {
        return Err(benchmark_error(format!("{metric}: the row has no context")));
    };
    match (
        context.get("mem"),
        context.get("meminfo_file"),
        context.len(),
    ) {
        (Some(figure), None, 1) => Ok(("mem", figure.clone())),
        (None, Some(path @ PyValue::Str(_)), 1) => Ok(("meminfo_file", path.clone())),
        _ => Err(benchmark_error(format!(
            "{metric}: the context is exactly one of mem, meminfo_file"
        ))),
    }
}

/// Reads the fixture: a JSON array of rows, parsed once.
pub fn load(source: &str) -> Res<Vec<Case>> {
    let PyValue::List(rows) =
        loads(source).map_err(|error| benchmark_error(format!("fixture is not JSON: {error}")))?
    else {
        return Err(benchmark_error("the fixture is not an array".to_string()));
    };
    let mut cases = Vec::new();
    for row in rows {
        let PyValue::Dict(members) = row else {
            return Err(benchmark_error(
                "a fixture row is not an object".to_string(),
            ));
        };
        let metric = text_of(&members, "metric")?;
        let function = resolve(&text_of(&members, "operation")?)?;
        let args = match members.get("args") {
            Some(PyValue::List(items)) => items.clone(),
            _ => Vec::new(),
        };
        let kwargs = match members.get("kwargs") {
            Some(PyValue::Dict(map)) => map.clone(),
            _ => PyDict::new(),
        };
        let Some(expected) = members.get("expected").cloned() else {
            return Err(benchmark_error(format!(
                "{metric}: the row has no expected value"
            )));
        };
        let (kind, detail) = machine_of(&metric, &members)?;
        let make_machine = move || match (kind, &detail) {
            ("mem", figure) => Machine::Fixed(figure.clone()),
            (_, PyValue::Str(path)) => Machine::File(FileMeminfo::new(path.clone())),
            _ => unreachable!("machine_of admits two shapes"),
        };
        cases.push(Case {
            metric,
            call: Box::new(move || {
                let mut machine = make_machine();
                let (mut args, mut kwargs) = (args.clone(), kwargs.clone());
                function.call(&mut args, &mut kwargs, &mut machine, None)
            }),
            expected,
        });
    }
    Ok(cases)
}
