//! `recipe_memory_api`: the five functions of `preload_ram_gate.py` as a JSON-lines service, wire-compatible with the other
//! recipe services. `constants` writes one flat object (`MIB`, `DROP_HELPER`); `evaluate-jsonl` reads one request per line,
//! `{id, operation: "preload_ram_gate.<function>", args, kwargs}`, and writes exactly one record per request, in order:
//! `{id, kind: "value", value, stdout, stderr, args_after, kwargs_after}` or `{id, kind: "exception", type, message, ...}`.
//!
//! `stdout` and `stderr` are what the function wrote (the gate's default log is standard error). The arguments are bound before
//! any body runs, with CPython's order and messages; a warnings list the caller passed is written to in place, so its
//! additions are in `args_after`/`kwargs_after` even when the call then failed.
//!
//! The service has no way to substitute the machine: a request carries no members beyond the four above, a JSON `log` is data
//! (never a callable), and the provider is the real [`System`](crate::memory_system::System). A request that names anything else is
//! answered by a `RequestError` record, never executed.

use crate::memory::{
    checkpoint_bytes_for, gate, offloadable_bytes_for, try_drop_caches, DropSystem, GateArguments,
    Host, LogChoice, MeminfoSource, Provider, DEFAULT_FLOOR_MIB, DROP_HELPER, MIB,
};
use crate::model_api::encode;
use crate::pyexpr::Res;
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};

/// The module `preload_ram_gate`.
pub const MODULE: &str = "preload_ram_gate";

/// The machine a call runs against, as the lowest-level functions see it, together with the gate's own sinks.
pub trait Environment: Host {
    fn meminfo(&mut self) -> &mut dyn MeminfoSource;
    fn drops(&mut self) -> &mut dyn DropSystem;
}

/// A real or controlled machine: the pieces [`Environment`] is made of.
pub trait Machine: Provider + MeminfoSource + DropSystem {}
impl<T: Provider + MeminfoSource + DropSystem> Machine for T {}

#[derive(Clone, Copy)]
enum Operation {
    MemAvailable,
    CheckpointBytes,
    OffloadableBytes,
    TryDrop,
    Gate,
}

/// A function of `preload_ram_gate`.
pub struct Function {
    pub name: &'static str,
    params: &'static [Param],
    operation: Operation,
}

fn none() -> PyValue {
    PyValue::None
}

fn one() -> PyValue {
    PyValue::int(1)
}

fn default_floor() -> PyValue {
    PyValue::int(DEFAULT_FLOOR_MIB)
}

fn truth() -> PyValue {
    PyValue::Bool(true)
}

fn default_mechanism() -> PyValue {
    PyValue::str("ple_mmap")
}

const GATE_LOG: usize = 4;

pub const FUNCTIONS: &[Function] = &[
    Function {
        name: "mem_available_mib",
        params: &[],
        operation: Operation::MemAvailable,
    },
    Function {
        name: "checkpoint_bytes_for",
        params: &[required("host_path"), optional("warnings", none)],
        operation: Operation::CheckpointBytes,
    },
    Function {
        name: "offloadable_bytes_for",
        params: &[
            required("host_path"),
            optional("mechanism", default_mechanism),
            optional("warnings", none),
        ],
        operation: Operation::OffloadableBytes,
    },
    Function {
        name: "try_drop_caches",
        params: &[],
        operation: Operation::TryDrop,
    },
    Function {
        name: "gate",
        params: &[
            required("checkpoint_bytes"),
            optional("tp", one),
            optional("floor_mib", default_floor),
            optional("auto_drop", truth),
            optional("log", none),
        ],
        operation: Operation::Gate,
    },
];

/// Resolves `preload_ram_gate.function`; the failure is the `AttributeError`/`ModuleNotFoundError` of the lookup.
pub fn resolve(operation: &str) -> Result<&'static Function, Failure> {
    let (module, name) = operation.split_once('.').unwrap_or((operation, ""));
    if module != MODULE {
        return Err(Failure::new(
            "ModuleNotFoundError",
            &format!("No module named '{module}'"),
        ));
    }
    FUNCTIONS
        .iter()
        .find(|function| function.name == name)
        .ok_or_else(|| {
            Failure::new(
                "AttributeError",
                &format!("module '{module}' has no attribute '{name}'"),
            )
        })
}

/// Where a bound argument lives in the caller's call, for a body that writes to it.
enum Slot {
    Positional(usize),
    Keyword(&'static str),
    Absent,
}

impl Function {
    /// Binds the arguments and runs the function against `env`. `log` is the gate's log when the caller knows better than the
    /// data (a callable the transport holds); it needs a `log` argument to be supplied, and only the gate has one.
    pub fn call(
        &self,
        args: &mut [PyValue],
        kwargs: &mut PyDict,
        env: &mut dyn Environment,
        log: Option<LogChoice>,
    ) -> Res<PyValue> {
        let signature = Signature {
            name: self.name,
            params: self.params,
            rest: false,
        };
        let bound = signature.bind(args, kwargs)?;
        if log.is_some()
            && !(matches!(self.operation, Operation::Gate) && bound.is_supplied(GATE_LOG))
        {
            return Err(Failure::new(
                "RequestError",
                "a log descriptor needs the gate's log argument to be supplied",
            ));
        }
        match self.operation {
            Operation::MemAvailable => crate::memory::mem_available_mib(env.meminfo()),
            Operation::TryDrop => Ok(PyValue::Bool(try_drop_caches(env.drops()))),
            Operation::Gate => {
                let choice = log.unwrap_or_else(|| LogChoice::of_data(&bound[GATE_LOG]));
                let row = gate(
                    env,
                    &GateArguments {
                        checkpoint_bytes: &bound[0],
                        tp: &bound[1],
                        floor_mib: &bound[2],
                        auto_drop: &bound[3],
                        log: choice,
                    },
                )?;
                Ok(PyValue::Dict(row))
            }
            Operation::CheckpointBytes | Operation::OffloadableBytes => {
                let warnings_at = self.params.len() - 1;
                let slot = if args.len() > warnings_at {
                    Slot::Positional(warnings_at)
                } else if bound.is_supplied(warnings_at) {
                    Slot::Keyword(self.params[warnings_at].name)
                } else {
                    Slot::Absent
                };
                let host_path = bound[0].clone();
                let mechanism =
                    matches!(self.operation, Operation::OffloadableBytes).then(|| bound[1].clone());
                let mut warnings = bound[warnings_at].clone();
                let outcome = match &mechanism {
                    None => checkpoint_bytes_for(&host_path, Some(&mut warnings)),
                    Some(mechanism) => {
                        offloadable_bytes_for(&host_path, mechanism, Some(&mut warnings))
                    }
                };
                match slot {
                    Slot::Positional(at) => args[at] = warnings,
                    Slot::Keyword(name) => {
                        if let Some(held) = kwargs.get_mut(name) {
                            *held = warnings;
                        }
                    }
                    Slot::Absent => {}
                }
                outcome
            }
        }
    }
}

/// The module constants as one flat object.
pub fn constants() -> PyValue {
    let mut all = PyDict::new();
    all.set("MIB", PyValue::int(MIB));
    all.set("DROP_HELPER", PyValue::str(DROP_HELPER));
    PyValue::Dict(all)
}

/// The record of one request.
pub fn record(
    id: PyValue,
    outcome: Result<PyValue, Failure>,
    streams: (PyStr, PyStr),
    arguments_after: (PyValue, PyValue),
) -> PyValue {
    let mut row = PyDict::new();
    row.set("id", id);
    match outcome {
        Ok(value) => {
            row.set("kind", PyValue::str("value"));
            row.set("value", encode(&value));
        }
        Err(failure) => {
            row.set("kind", PyValue::str("exception"));
            row.set("type", PyValue::str(failure.class));
            row.set("message", PyValue::Str(failure.message));
        }
    }
    row.set("stdout", PyValue::Str(streams.0));
    row.set("stderr", PyValue::Str(streams.1));
    row.set("args_after", arguments_after.0);
    row.set("kwargs_after", arguments_after.1);
    PyValue::Dict(row)
}

fn request_record(id: PyValue, message: &str) -> PyValue {
    record(
        id,
        Err(Failure::new("RequestError", message)),
        (PyStr::default(), PyStr::default()),
        (PyValue::List(Vec::new()), PyValue::Dict(PyDict::new())),
    )
}

/// A request that passed the shape checks: the function it names is looked up when it is answered.
pub struct Request {
    pub id: PyValue,
    operation: PyStr,
    pub args: Vec<PyValue>,
    pub kwargs: PyDict,
    pub members: PyDict,
}

/// Reads one request line. The members allowed besides `id`, `operation`, `args` and `kwargs` are named by the caller (the
/// session service allows one more); any other member is refused. `Err` is the finished record to answer with.
pub fn parse_request(line: &PyStr, extra_members: &[&str]) -> Result<Request, PyValue> {
    let request = match loads_text_objects(line) {
        Ok(request) => request,
        Err(error) => return Err(request_record(PyValue::None, &format!("not JSON: {error}"))),
    };
    let PyValue::Dict(members) = request else {
        return Err(request_record(
            PyValue::None,
            "the request is not an object",
        ));
    };
    let Some(id) = members.get("id").cloned() else {
        return Err(request_record(PyValue::None, "the request has no id"));
    };
    for (key, _) in members.iter() {
        let known = ["id", "operation", "args", "kwargs"]
            .iter()
            .chain(extra_members)
            .any(|name| key.equals(name));
        if !known {
            return Err(request_record(
                id,
                &format!(
                    "the request has a member this service does not take: {}",
                    key.repr()
                ),
            ));
        }
    }
    let Some(PyValue::Str(operation)) = members.get("operation") else {
        return Err(request_record(
            id,
            "operation must be a string 'module.function'",
        ));
    };
    let args = match members.get("args") {
        None => Vec::new(),
        Some(PyValue::List(items)) => items.clone(),
        Some(_) => return Err(request_record(id, "args must be an array")),
    };
    let kwargs = match members.get("kwargs") {
        None => PyDict::new(),
        Some(PyValue::Dict(map)) => map.clone(),
        Some(_) => return Err(request_record(id, "kwargs must be an object")),
    };
    Ok(Request {
        id,
        operation: operation.clone(),
        args,
        kwargs,
        members,
    })
}

impl Request {
    /// The function the request names.
    pub fn function(&self) -> Result<&'static Function, Failure> {
        resolve(&self.operation.to_utf8().unwrap_or_default())
    }
}

/// The standard error of one request: what the gate's default log wrote, kept for the record.
struct Captured<'a, M: Machine> {
    machine: &'a mut M,
    stderr: PyStr,
}

impl<M: Machine> Provider for Captured<'_, M> {
    fn mem_available_mib(&mut self) -> Res<PyValue> {
        self.machine.mem_available_mib()
    }

    fn try_drop_caches(&mut self) -> Res<bool> {
        self.machine.try_drop_caches()
    }
}

impl<M: Machine> Host for Captured<'_, M> {
    fn write_stderr(&mut self, text: &PyStr) -> Res<()> {
        self.stderr.push_py(text);
        Ok(())
    }

    fn call_log(&mut self, _message: &PyStr) -> Res<()> {
        Err(Failure::new(
            "RuntimeError",
            "a data request holds no callable log",
        ))
    }
}

impl<M: Machine> Environment for Captured<'_, M> {
    fn meminfo(&mut self) -> &mut dyn MeminfoSource {
        self.machine
    }

    fn drops(&mut self) -> &mut dyn DropSystem {
        self.machine
    }
}

/// Answers one request line against `machine`.
pub fn answer<M: Machine>(line: &PyStr, machine: &mut M) -> PyValue {
    let mut request = match parse_request(line, &[]) {
        Ok(request) => request,
        Err(row) => return row,
    };
    let mut env = Captured {
        machine,
        stderr: PyStr::default(),
    };
    let outcome = request
        .function()
        .and_then(|function| function.call(&mut request.args, &mut request.kwargs, &mut env, None));
    record(
        request.id,
        outcome,
        (PyStr::default(), env.stderr),
        (
            encode(&PyValue::List(request.args)),
            encode(&PyValue::Dict(request.kwargs)),
        ),
    )
}

/// One output line of a record.
pub fn render(row: &PyValue) -> Result<String, Failure> {
    crate::api::render(row)
}

/// `evaluate-jsonl`: every non-blank line of `input` answered in order, one JSON line each. `Err` is a record that could not be
/// written as JSON (nothing is returned in that case).
pub fn evaluate_jsonl<M: Machine>(input: &[u8], machine: &mut M) -> Result<Vec<u8>, Failure> {
    let text = PyStr::from_surrogateescape(input);
    let newline = u32::from('\n');
    let mut out = Vec::new();
    for piece in text.code_points().split(|&point| point == newline) {
        let line = PyStr::from_code_points(piece.to_vec());
        if line.strip().is_empty() {
            continue;
        }
        out.extend_from_slice(render(&answer(&line, machine))?.as_bytes());
        out.push(b'\n');
    }
    Ok(out)
}
