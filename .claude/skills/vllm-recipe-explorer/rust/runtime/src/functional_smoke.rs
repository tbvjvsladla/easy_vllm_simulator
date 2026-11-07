//! `functional_smoke.py`: the functional smoke of a served candidate, six public operations.
//!
//! * `_post_chat(base_url, payload, timeout=60.0)`: one `POST` of the JSON payload to `<base_url>/v1/chat/completions`; every outcome of the
//!   exchange is a value (a parsed JSON answer, or `{"_error": ...}`); only what happens while the request is being built escapes
//!   (`AttributeError`, `ValueError`).
//! * `_first_choice(resp)`, `_check_completion(choice)`, `_check_reasoning(choice)`, `_check_tool_call(choice)`: the checks, with Python's
//!   dynamic typing, truthiness and `.get` semantics (nothing is validated beyond what the original asks).
//! * `smoke(base_url, served_model_name, candidate, timeout=60.0, mock_responses=None)`: completion always, then reasoning if enabled, then
//!   tool call if enabled (each is made even when an earlier one failed). `mock_responses is None` means live HTTP; every other value,
//!   falsy ones included, is the mock path (`mock_responses.get(...)`), which a mock-pass must never be mistaken for a serving pass.
//!
//! The four module constants are values given to each call ([`Constants`]), never read from the environment. Observation: the HTTP
//! steps are told to an [`Observe`] (the checks' control record); production tells nothing.

use crate::escalation::Raised;
use crate::functional_http::{open, parse_url, OpenError};
use crate::pyexpr::{get, get_or};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pycall::{optional, required, Param, Signature};
use agent_runtime_support::pyjson::{decode_utf8_strict, loads_text};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{DumpError, PyDict, PyValue};

pub const MODULE: &str = "functional_smoke";

/// What the checks observe of the HTTP steps; production observes nothing.
pub trait Observe {
    fn enabled(&self) -> bool {
        false
    }
    fn record(&mut self, _name: &str, _fields: Vec<(&str, PyValue)>) {}
}

/// An observer that observes nothing.
pub struct Plain;

impl Observe for Plain {}

fn note(observer: &mut dyn Observe, name: &str, fields: Vec<(&str, PyValue)>) {
    if observer.enabled() {
        observer.record(name, fields);
    }
}

// ── the module constants ─────────────────────────────────────────────────────────────────────────

/// The four module constants of the original, replaceable as values for one call.
#[derive(Clone, Debug)]
pub struct Constants {
    pub completion_prompt: PyValue,
    pub reasoning_prompt: PyValue,
    pub tool_prompt: PyValue,
    pub weather_tool: PyValue,
}

impl Constants {
    pub fn canonical() -> Constants {
        let weather = r#"{"type": "function", "function": {"name": "get_weather", "description": "Get the current weather for a given city.", "parameters": {"type": "object", "properties": {"city": {"type": "string", "description": "City name."}}, "required": ["city"]}}}"#;
        Constants {
            completion_prompt: PyValue::str("Say hello in one short sentence."),
            reasoning_prompt: PyValue::str(
                "What is 17 plus 26? Think step by step, then give the answer.",
            ),
            tool_prompt: PyValue::str(
                "What is the weather in Seoul right now? Use the available tool.",
            ),
            weather_tool: loads_text(&PyStr::new(weather)).expect("a fixed JSON text"),
        }
    }

    /// The constants as the module holds them, in the source's order.
    pub fn to_value(&self) -> PyValue {
        let mut all = PyDict::new();
        all.set("_COMPLETION_PROMPT", self.completion_prompt.clone());
        all.set("_REASONING_PROMPT", self.reasoning_prompt.clone());
        all.set("_TOOL_PROMPT", self.tool_prompt.clone());
        all.set("_WEATHER_TOOL", self.weather_tool.clone());
        PyValue::Dict(all)
    }

    /// Replaces what the object names (only the four constants); anything else is the error text.
    pub fn replace_from(&mut self, given: &PyValue) -> Result<(), String> {
        let PyValue::Dict(members) = given else {
            return Err("globals must be an object".to_string());
        };
        for (name, value) in members.iter() {
            match name.to_utf8().as_deref() {
                Some("_COMPLETION_PROMPT") => self.completion_prompt = value.clone(),
                Some("_REASONING_PROMPT") => self.reasoning_prompt = value.clone(),
                Some("_TOOL_PROMPT") => self.tool_prompt = value.clone(),
                Some("_WEATHER_TOOL") => self.weather_tool = value.clone(),
                _ => return Err(format!("globals member {} is not usable", name.repr())),
            }
        }
        Ok(())
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────────────────────────

fn failure(error: Failure) -> Raised {
    Raised::from(error)
}

fn dump_error(error: DumpError) -> Raised {
    let class = error.python_type();
    let shown = error.to_string();
    let message = shown
        .strip_prefix(&format!("{class}: "))
        .unwrap_or(&shown)
        .to_string();
    Raised::plain(class, &message)
}

fn dict_of(pairs: Vec<(&str, PyValue)>) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in pairs {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn error_value(text: &str) -> PyValue {
    dict_of(vec![("_error", PyValue::str(text))])
}

/// `f"{type(e).__name__}: {e}"` of an exception.
fn shown(raised: &Raised) -> String {
    format!(
        "{}: {}",
        raised.class,
        raised.message.to_utf8().unwrap_or_default()
    )
}

fn read_events(observer: &mut dyn Observe, label: &str, outcome: &Result<Vec<u8>, Raised>) {
    if !observer.enabled() {
        return;
    }
    observer.record(
        &format!("{label}_read"),
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(PyDict::new())),
        ],
    );
    match outcome {
        Ok(bytes) => observer.record(
            &format!("{label}_read_return"),
            vec![("bytes_hex", PyValue::str(&hex(bytes)))],
        ),
        Err(raised) => observer.record(
            &format!("{label}_read_error"),
            vec![
                ("type", PyValue::str(&raised.class)),
                ("message", PyValue::Str(raised.message.clone())),
            ],
        ),
    }
}

// ── _post_chat ───────────────────────────────────────────────────────────────────────────────────

/// `_post_chat(base_url, payload, timeout)`: the value of the exchange, or the exception of building the request.
pub fn post_chat(
    observer: &mut dyn Observe,
    base_url: &PyValue,
    payload: &PyValue,
    timeout: &PyValue,
) -> Result<PyValue, Raised> {
    let PyValue::Str(base) = base_url.peel() else {
        return Err(Raised::plain(
            "AttributeError",
            &format!(
                "'{}' object has no attribute 'rstrip'",
                base_url.type_name()
            ),
        ));
    };
    let mut points = base.code_points().to_vec();
    while points.last() == Some(&u32::from('/')) {
        points.pop();
    }
    let mut url = PyStr::from_code_points(points);
    url.push_str("/v1/chat/completions");
    let data = payload.dumps_ascii().map_err(dump_error)?.into_bytes();
    note(
        observer,
        "request_new",
        vec![
            ("url", PyValue::Str(url.clone())),
            ("data_hex", PyValue::str(&hex(&data))),
            (
                "headers",
                dict_of(vec![("Content-Type", PyValue::str("application/json"))]),
            ),
            ("method", PyValue::str("POST")),
        ],
    );
    let request = match parse_url(&url) {
        Ok(request) => request,
        Err(raised) => {
            note(
                observer,
                "request_new_error",
                vec![
                    ("type", PyValue::str(&raised.class)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            );
            return Err(raised);
        }
    };
    note(
        observer,
        "urlopen",
        vec![
            ("url", PyValue::Str(request.full_url.clone())),
            ("timeout", timeout.clone()),
        ],
    );
    match open(&request, &data, timeout) {
        Ok(mut reply) => {
            note(
                observer,
                "http_open",
                vec![("status", PyValue::int(reply.status))],
            );
            let read = reply.read_all();
            read_events(observer, "http", &read);
            let decoded = read.and_then(|bytes| {
                decode_utf8_strict(&bytes)
                    .map_err(|error| failure(crate::model_files::load_failure(error)))
            });
            note(observer, "http_close", Vec::new());
            let text = match decoded {
                Ok(text) => text,
                Err(raised) => return Ok(error_value(&format!("exception: {}", shown(&raised)))),
            };
            match loads_text(&text) {
                Ok(value) => Ok(value),
                Err(error) => {
                    let raised = failure(crate::model_files::load_failure(error));
                    Ok(error_value(&format!("exception: {}", shown(&raised))))
                }
            }
        }
        Err(OpenError::Url { reason }) => {
            let mut message = PyStr::new("<urlopen error ");
            message.push_py(&reason);
            message.push_str(">");
            note(
                observer,
                "urlopen_error",
                vec![
                    ("type", PyValue::str("URLError")),
                    ("module", PyValue::str("urllib.error")),
                    ("message", PyValue::Str(message)),
                ],
            );
            let mut text = PyStr::new("urlerror: ");
            text.push_py(&reason);
            Ok(dict_of(vec![("_error", PyValue::Str(text))]))
        }
        Err(OpenError::Status(mut reply)) => {
            let code = reply.status;
            let mut message = PyStr::new(&format!("HTTP Error {code}: "));
            message.push_py(&reply.reason);
            note(
                observer,
                "urlopen_error",
                vec![
                    ("type", PyValue::str("HTTPError")),
                    ("module", PyValue::str("urllib.error")),
                    ("message", PyValue::Str(message)),
                    ("code", PyValue::int(code)),
                    ("reason", PyValue::Str(reply.reason.clone())),
                ],
            );
            let read = reply.read_all();
            read_events(observer, "http_error", &read);
            let detail = read
                .ok()
                .and_then(|bytes| decode_utf8_strict(&bytes).ok())
                .unwrap_or_default();
            let mut text = PyStr::new(&format!("http_{code}: "));
            text.push_py(&PyStr::from_code_points(
                detail.code_points().iter().copied().take(300).collect(),
            ));
            Ok(dict_of(vec![("_error", PyValue::Str(text))]))
        }
        Err(OpenError::Other(raised)) => {
            note(
                observer,
                "urlopen_error",
                vec![
                    ("type", PyValue::str(&raised.class)),
                    ("module", PyValue::str(&raised.module)),
                    ("message", PyValue::Str(raised.message.clone())),
                ],
            );
            Ok(error_value(&format!("exception: {}", shown(&raised))))
        }
    }
}

// ── the checks ───────────────────────────────────────────────────────────────────────────────────

fn is_dict(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Dict(_))
}

/// `_first_choice(resp)`.
pub fn first_choice(resp: &PyValue) -> PyValue {
    let empty = || PyValue::Dict(PyDict::new());
    let PyValue::Dict(map) = resp.peel() else {
        return empty();
    };
    let Some(choices) = map.get("choices") else {
        return empty();
    };
    let PyValue::List(items) = choices.peel() else {
        return empty();
    };
    match items.first() {
        Some(first) if is_dict(first) => first.clone(),
        _ => empty(),
    }
}

/// `msg = choice.get("message") if isinstance(choice, dict) else None`, as a dictionary or nothing.
fn message_of(choice: &PyValue) -> Option<PyValue> {
    let PyValue::Dict(map) = choice.peel() else {
        return None;
    };
    let message = map.get("message")?;
    is_dict(message).then(|| message.clone())
}

fn non_blank_text(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::Str(text) if !text.strip().is_empty())
}

fn member(map: &PyValue, key: &str) -> PyValue {
    match map.peel() {
        PyValue::Dict(entries) => entries.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

/// `_check_completion(choice)`.
pub fn check_completion(choice: &PyValue) -> bool {
    message_of(choice).is_some_and(|message| non_blank_text(&member(&message, "content")))
}

/// `_check_reasoning(choice)`: `reasoning_content`, else (when it is falsy) `reasoning`; the value that was picked must be non-blank text.
pub fn check_reasoning(choice: &PyValue) -> bool {
    let Some(message) = message_of(choice) else {
        return false;
    };
    let old = member(&message, "reasoning_content");
    let picked = if old.is_truthy() {
        old
    } else {
        member(&message, "reasoning")
    };
    non_blank_text(&picked)
}

/// `_check_tool_call(choice)`.
pub fn check_tool_call(choice: &PyValue) -> bool {
    if !is_dict(choice) {
        return false;
    }
    if matches!(member(choice, "finish_reason").peel(), PyValue::Str(text) if text.equals("tool_calls"))
    {
        return true;
    }
    if let Some(message) = choice_message(choice) {
        if let PyValue::List(items) = member(&message, "tool_calls").peel() {
            return !items.is_empty();
        }
    }
    false
}

fn choice_message(choice: &PyValue) -> Option<PyValue> {
    message_of(choice)
}

// ── smoke ────────────────────────────────────────────────────────────────────────────────────────

fn messages(prompt: &PyValue) -> PyValue {
    PyValue::List(vec![dict_of(vec![
        ("role", PyValue::str("user")),
        ("content", prompt.clone()),
    ])])
}

/// `smoke(base_url, served_model_name, candidate, timeout, mock_responses)`.
pub fn smoke(
    observer: &mut dyn Observe,
    constants: &Constants,
    base_url: &PyValue,
    served_model_name: &PyValue,
    candidate: &PyValue,
    timeout: &PyValue,
    mock_responses: &PyValue,
) -> Result<PyValue, Raised> {
    let found = get(candidate, "model_capabilities").map_err(failure)?;
    let caps = if found.is_truthy() {
        found
    } else {
        PyValue::Dict(PyDict::new())
    };
    let want_tool = get(&caps, "tool_call").map_err(failure)?.is_truthy();
    let want_reasoning = get(&caps, "reasoning").map_err(failure)?.is_truthy();
    let live = matches!(mock_responses.peel(), PyValue::None);
    let empty = || PyValue::Dict(PyDict::new());
    let mut evidence = PyDict::new();

    let completion = if live {
        post_chat(
            observer,
            base_url,
            &dict_of(vec![
                ("model", served_model_name.clone()),
                ("messages", messages(&constants.completion_prompt)),
                (
                    "max_tokens",
                    PyValue::int(if want_reasoning { 4096 } else { 256 }),
                ),
                ("temperature", PyValue::Float(0.0)),
            ]),
            timeout,
        )?
    } else {
        get_or(mock_responses, "completion", empty()).map_err(failure)?
    };
    let completion_ok = check_completion(&first_choice(&completion));
    evidence.set("completion", completion);

    let reasoning_ok = if want_reasoning {
        let response = if live {
            post_chat(
                observer,
                base_url,
                &dict_of(vec![
                    ("model", served_model_name.clone()),
                    ("messages", messages(&constants.reasoning_prompt)),
                    ("max_tokens", PyValue::int(1024)),
                    ("temperature", PyValue::Float(0.0)),
                    (
                        "chat_template_kwargs",
                        dict_of(vec![("enable_thinking", PyValue::Bool(true))]),
                    ),
                ]),
                timeout,
            )?
        } else {
            get_or(mock_responses, "reasoning", empty()).map_err(failure)?
        };
        let verdict = check_reasoning(&first_choice(&response));
        evidence.set("reasoning", response);
        Some(verdict)
    } else {
        None
    };

    let tool_call_ok = if want_tool {
        let response = if live {
            post_chat(
                observer,
                base_url,
                &dict_of(vec![
                    ("model", served_model_name.clone()),
                    ("messages", messages(&constants.tool_prompt)),
                    ("tools", PyValue::List(vec![constants.weather_tool.clone()])),
                    ("tool_choice", PyValue::str("auto")),
                    ("max_tokens", PyValue::int(256)),
                    ("temperature", PyValue::Float(0.0)),
                ]),
                timeout,
            )?
        } else {
            get_or(mock_responses, "tool_call", empty()).map_err(failure)?
        };
        let verdict = check_tool_call(&first_choice(&response));
        evidence.set("tool_call", response);
        Some(verdict)
    } else {
        None
    };

    let passed = completion_ok && tool_call_ok != Some(false) && reasoning_ok != Some(false);
    let flag = |value: Option<bool>| value.map_or(PyValue::None, PyValue::Bool);
    Ok(dict_of(vec![
        ("completion_ok", PyValue::Bool(completion_ok)),
        ("tool_call_ok", flag(tool_call_ok)),
        ("reasoning_ok", flag(reasoning_ok)),
        ("passed", PyValue::Bool(passed)),
        ("evidence", PyValue::Dict(evidence)),
    ]))
}

// ── binding ──────────────────────────────────────────────────────────────────────────────────────

/// The six operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Function {
    PostChat,
    FirstChoice,
    CheckCompletion,
    CheckReasoning,
    CheckToolCall,
    Smoke,
}

fn sixty() -> PyValue {
    PyValue::Float(60.0)
}

fn none_value() -> PyValue {
    PyValue::None
}

const POST_CHAT: &[Param] = &[
    required("base_url"),
    required("payload"),
    optional("timeout", sixty),
];
const FIRST_CHOICE: &[Param] = &[required("resp")];
const CHOICE: &[Param] = &[required("choice")];
const SMOKE: &[Param] = &[
    required("base_url"),
    required("served_model_name"),
    required("candidate"),
    optional("timeout", sixty),
    optional("mock_responses", none_value),
];

impl Function {
    pub fn named(name: &str) -> Option<Function> {
        Some(match name {
            "_post_chat" => Function::PostChat,
            "_first_choice" => Function::FirstChoice,
            "_check_completion" => Function::CheckCompletion,
            "_check_reasoning" => Function::CheckReasoning,
            "_check_tool_call" => Function::CheckToolCall,
            "smoke" => Function::Smoke,
            _ => return None,
        })
    }

    fn signature(self) -> Signature {
        let (name, params) = match self {
            Function::PostChat => ("_post_chat", POST_CHAT),
            Function::FirstChoice => ("_first_choice", FIRST_CHOICE),
            Function::CheckCompletion => ("_check_completion", CHOICE),
            Function::CheckReasoning => ("_check_reasoning", CHOICE),
            Function::CheckToolCall => ("_check_tool_call", CHOICE),
            Function::Smoke => ("smoke", SMOKE),
        };
        Signature {
            name,
            params,
            rest: false,
        }
    }
}

/// Binds the arguments as Python does and runs the function; the exception is kept whole (class, module, message).
pub fn call(
    function: Function,
    observer: &mut dyn Observe,
    constants: &Constants,
    args: &[PyValue],
    kwargs: &PyDict,
) -> Result<PyValue, Raised> {
    let signature = function.signature();
    let bound = signature.bind(args, kwargs).map_err(failure)?;
    match function {
        Function::PostChat => post_chat(observer, &bound[0], &bound[1], &bound[2]),
        Function::FirstChoice => Ok(first_choice(&bound[0])),
        Function::CheckCompletion => Ok(PyValue::Bool(check_completion(&bound[0]))),
        Function::CheckReasoning => Ok(PyValue::Bool(check_reasoning(&bound[0]))),
        Function::CheckToolCall => Ok(PyValue::Bool(check_tool_call(&bound[0]))),
        Function::Smoke => smoke(
            observer, constants, &bound[0], &bound[1], &bound[2], &bound[3], &bound[4],
        ),
    }
}
