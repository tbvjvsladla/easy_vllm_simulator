//! `upstream_api evaluate-events`: one call whose console and `import yaml` are the CALLER's, asked synchronously (see `ABI.md`).
//!
//! Every real console write of the business code (`print` is two) is announced as a `write` event and acknowledged before the call goes on, so
//! a failure of the caller's real stream is the exception of that write; the loader's `import yaml` is announced as a `yaml_import` event at
//! the point the original makes it and answered by the caller's real import. A missing or malformed acknowledgement abandons the run: no result
//! and no later effect. Descriptor 0 is never touched.

use crate::api::{answer_on, Defaults};
use crate::host::{parser_write_failure, Host, Sink};
use crate::thrown::{Ctor, Done, Thrown};
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;
use std::io::{BufRead, Write};

/// The console and the importer of an event run.
pub struct EventHost<'a> {
    events: &'a mut dyn Write,
    acks: &'a mut dyn BufRead,
    sequence: i64,
    /// Why the run must be abandoned (set once; the business code is stopped by the error returned with it).
    pub fatal: Option<String>,
    parser_failure: Option<Box<Thrown>>,
}

impl<'a> EventHost<'a> {
    pub fn new(events: &'a mut dyn Write, acks: &'a mut dyn BufRead) -> EventHost<'a> {
        EventHost {
            events,
            acks,
            sequence: 0,
            fatal: None,
            parser_failure: None,
        }
    }

    fn abandon(&mut self, why: &str) -> Box<Thrown> {
        if self.fatal.is_none() {
            self.fatal = Some(why.to_string());
        }
        Thrown::builtin("ProtocolError", why)
    }

    /// Sends one event of the shared sequence and reads its acknowledgement: the object it is and the sequence it answers.
    fn exchange(&mut self, name: &str, fields: Vec<(&str, PyValue)>) -> Done<PyDict> {
        if let Some(why) = self.fatal.clone() {
            return Err(Thrown::builtin("ProtocolError", &why));
        }
        let mut event = PyDict::new();
        event.set("event", PyValue::str(name));
        event.set("sequence", PyValue::int(self.sequence));
        for (key, value) in fields {
            event.set(key, value);
        }
        let line = match PyValue::Dict(event).dumps_ascii() {
            Ok(line) => line,
            Err(error) => return Err(self.abandon(&format!("cannot encode an event: {error}"))),
        };
        if let Err(error) = writeln!(self.events, "{line}").and_then(|()| self.events.flush()) {
            return Err(self.abandon(&format!("cannot send an event: {error}")));
        }
        let mut raw = Vec::new();
        match self.acks.read_until(b'\n', &mut raw) {
            Ok(0) => return Err(self.abandon("the acknowledgement of an event never came")),
            Ok(_) => {}
            Err(error) => {
                return Err(self.abandon(&format!("cannot read an acknowledgement: {error}")))
            }
        }
        let sequence = self.sequence;
        self.sequence += 1;
        let line = PyStr::from_surrogateescape(&raw);
        match loads_text_objects(&line) {
            Ok(PyValue::Dict(ack)) if int_member(&ack, "sequence") == Some(sequence) => Ok(ack),
            Ok(PyValue::Dict(_)) => Err(self.abandon("invalid acknowledgement: out of sequence")),
            _ => Err(self.abandon("invalid acknowledgement: not a JSON object")),
        }
    }

    /// Announces one write and waits for its acknowledgement.
    fn write(&mut self, stream: &str, text: &PyStr) -> Done<()> {
        let ack = self.exchange(
            "write",
            vec![
                ("stream", PyValue::str(stream)),
                ("text", PyValue::Str(text.clone())),
            ],
        )?;
        match parse_write_ack(&ack) {
            Ok(None) => Ok(()),
            Ok(Some(thrown)) => Err(thrown),
            Err(why) => Err(self.abandon(&why)),
        }
    }
}

fn int_member(map: &PyDict, key: &str) -> Option<i64> {
    match map.get(key) {
        Some(PyValue::Int(number)) => i64::try_from(number.clone()).ok(),
        _ => None,
    }
}

fn text_member(map: &PyDict, key: &str) -> Option<PyStr> {
    match map.get(key) {
        Some(PyValue::Str(text)) => Some(text.clone()),
        _ => None,
    }
}

fn index_member(map: &PyDict, key: &str) -> Option<usize> {
    int_member(map, key).and_then(|number| usize::try_from(number).ok())
}

fn has_exactly(map: &PyDict, names: &[&str]) -> bool {
    map.len() == names.len() && names.iter().all(|name| map.contains_key(name))
}

/// The constructor DTO of an acknowledged exception (`unicode_encode` / `unicode_decode`).
fn constructor_of(value: &PyValue) -> Result<Ctor, String> {
    let bad = || "invalid acknowledgement: invalid constructor".to_string();
    let PyValue::Dict(map) = value else {
        return Err(bad());
    };
    let utf8 = |key: &str| text_member(map, key).and_then(|text| text.to_utf8());
    let (Some(kind), Some(encoding), Some(reason)) =
        (utf8("kind"), utf8("encoding"), utf8("reason"))
    else {
        return Err(bad());
    };
    let (Some(start), Some(end)) = (index_member(map, "start"), index_member(map, "end")) else {
        return Err(bad());
    };
    match kind.as_str() {
        "unicode_encode"
            if has_exactly(
                map,
                &["kind", "encoding", "object", "start", "end", "reason"],
            ) =>
        {
            let object = text_member(map, "object").ok_or_else(bad)?;
            Ok(Ctor::Encode {
                encoding,
                object,
                start,
                end,
                reason,
            })
        }
        "unicode_decode"
            if has_exactly(
                map,
                &["kind", "encoding", "object_hex", "start", "end", "reason"],
            ) =>
        {
            let hex = utf8("object_hex").ok_or_else(bad)?;
            if hex.len() % 2 != 0 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(bad());
            }
            let object = (0..hex.len() / 2)
                .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).map_err(|_| bad()))
                .collect::<Result<Vec<u8>, String>>()?;
            Ok(Ctor::Decode {
                encoding,
                object,
                start,
                end,
                reason,
            })
        }
        _ => Err(bad()),
    }
}

/// The `builtin_bases` descriptor of an exception acknowledgement: the canonical builtin classes of the real MRO, in order. A non-empty list of
/// distinct non-empty names ending `BaseException` (optionally followed by `object`). The immediate class and module are descriptive strings,
/// never proof of identity: the constraints on the hierarchy are read from the canonical names PRESENT in the list.
fn bases_of(value: Option<&PyValue>) -> Result<Vec<String>, String> {
    let bad = |why: &str| format!("invalid acknowledgement: builtin_bases {why}");
    let Some(PyValue::List(rows)) = value else {
        return Err(bad("is not a list"));
    };
    let mut bases: Vec<String> = Vec::new();
    for row in rows {
        let Some(name) = (match row {
            PyValue::Str(text) => text.to_utf8().filter(|name| !name.is_empty()),
            _ => None,
        }) else {
            return Err(bad("holds a name that is not non-empty text"));
        };
        if bases.contains(&name) {
            return Err(bad("holds a duplicate"));
        }
        bases.push(name);
    }
    let terminal_ok = match bases.as_slice() {
        [.., last] if last == "BaseException" => true,
        [.., last, object] => last == "BaseException" && object == "object",
        _ => false,
    };
    if !terminal_ok
        || bases
            .iter()
            .take(bases.len().saturating_sub(1))
            .any(|base| base == "object")
    {
        return Err(bad("does not end with BaseException"));
    }
    let has = |name: &str| bases.iter().any(|base| base == name);
    const OS_FAMILY: [&str; 15] = [
        "FileNotFoundError",
        "PermissionError",
        "IsADirectoryError",
        "NotADirectoryError",
        "FileExistsError",
        "InterruptedError",
        "ChildProcessError",
        "ProcessLookupError",
        "BrokenPipeError",
        "BlockingIOError",
        "ConnectionError",
        "ConnectionAbortedError",
        "ConnectionResetError",
        "ConnectionRefusedError",
        "TimeoutError",
    ];
    if OS_FAMILY.iter().any(|name| has(name)) && !has("OSError") {
        return Err(bad("names an OSError-family class without OSError"));
    }
    if (has("UnicodeEncodeError") || has("UnicodeDecodeError"))
        && !(has("UnicodeError") && has("ValueError"))
    {
        return Err(bad("lacks UnicodeError and ValueError for a Unicode class"));
    }
    if has("ModuleNotFoundError") && !has("ImportError") {
        return Err(bad("names ModuleNotFoundError without ImportError"));
    }
    Ok(bases)
}

/// One write acknowledgement: `Ok(None)` for success, `Ok(Some(error))` for the exception it reports, `Err` for an acknowledgement that is not
/// valid (the run is abandoned).
fn parse_write_ack(ack: &PyDict) -> Result<Option<Box<Thrown>>, String> {
    let invalid = |why: &str| format!("invalid acknowledgement: {why}");
    if text_member(ack, "event").is_none_or(|event| !event.equals("write_ack")) {
        return Err(invalid("not a write_ack"));
    }
    let Some(kind) = text_member(ack, "kind") else {
        return Err(invalid("no kind"));
    };
    if kind.equals("ok") {
        return if has_exactly(ack, &["event", "sequence", "kind"]) {
            Ok(None)
        } else {
            Err(invalid("members of an ok acknowledgement"))
        };
    }
    if !kind.equals("exception") {
        return Err(invalid("unknown kind"));
    }
    let with_constructor = ack.contains_key("constructor");
    let mut expected = vec![
        "event",
        "sequence",
        "kind",
        "type",
        "module",
        "message",
        "builtin_bases",
    ];
    if with_constructor {
        expected.push("constructor");
    }
    if !has_exactly(ack, &expected) {
        return Err(invalid("members of an exception acknowledgement"));
    }
    let (Some(class), Some(module), Some(message)) = (
        text_member(ack, "type").and_then(|text| text.to_utf8()),
        text_member(ack, "module").and_then(|text| text.to_utf8()),
        text_member(ack, "message"),
    ) else {
        return Err(invalid("type, module and message are text"));
    };
    let bases = bases_of(ack.get("builtin_bases"))?;
    let has = |name: &str| bases.iter().any(|base| base == name);
    let (is_encode, is_decode) = (has("UnicodeEncodeError"), has("UnicodeDecodeError"));
    if (is_encode || is_decode) != with_constructor {
        return Err(invalid(
            "a constructor belongs to exactly the two Unicode classes",
        ));
    }
    let ctor = match ack.get("constructor") {
        Some(value) => Some(constructor_of(value)?),
        None => None,
    };
    if let Some(found) = &ctor {
        if matches!(found, Ctor::Encode { .. }) != is_encode {
            return Err(invalid("the constructor does not belong to the class"));
        }
    }
    Ok(Some(Box::new(Thrown {
        raised: Raised::new(&class, &module, message),
        ctors: vec![ctor],
        bases: Some(bases),
    })))
}

/// The acknowledgement of `import yaml`: `Ok(())` for an available PyYAML 6.0.3, the reported exception chain otherwise.
fn parse_import_ack(ack: &PyDict) -> Result<Done<()>, String> {
    let invalid = |why: &str| format!("invalid acknowledgement: {why}");
    if text_member(ack, "event").is_none_or(|event| !event.equals("yaml_import_ack")) {
        return Err(invalid("not a yaml_import_ack"));
    }
    let Some(kind) = text_member(ack, "kind") else {
        return Err(invalid("no kind"));
    };
    if kind.equals("available") {
        if !has_exactly(ack, &["event", "sequence", "kind", "version"]) {
            return Err(invalid("members of an available acknowledgement"));
        }
        return match text_member(ack, "version") {
            Some(version) if version.equals("6.0.3") => Ok(Ok(())),
            _ => Err(invalid("only PyYAML 6.0.3 is modelled")),
        };
    }
    if !kind.equals("exception")
        || !has_exactly(ack, &["event", "sequence", "kind", "exception_chain"])
    {
        return Err(invalid("members of an import exception acknowledgement"));
    }
    let Some(PyValue::List(rows)) = ack.get("exception_chain") else {
        return Err(invalid("exception_chain is not a list"));
    };
    let mut rows = rows.clone();
    let Some(PyValue::Dict(first)) = rows.first_mut() else {
        return Err(invalid("exception_chain is empty or holds a non-object"));
    };
    let bases = bases_of(first.get("builtin_bases"))?;
    let mut stripped = PyDict::new();
    for (key, value) in first.iter() {
        if !key.equals("builtin_bases") {
            stripped.insert(key.clone(), value.clone());
        }
    }
    *first = stripped;
    if rows.iter().any(|row| matches!(row, PyValue::Dict(map) if map.contains_key("constructor") || map.contains_key("builtin_bases"))) {
        return Err(invalid("a constructor or a later descriptor in an import chain"));
    }
    let Some(raised) = Raised::from_chain(&PyValue::List(rows)) else {
        return Err(invalid("exception_chain is not a valid chain"));
    };
    Ok(Err(Box::new(Thrown {
        raised,
        ctors: Vec::new(),
        bases: Some(bases),
    })))
}

impl Sink for EventHost<'_> {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.write("stdout", text)
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.write("stderr", text)
    }

    /// argparse's usage and error writes (`_print_message`): announced one after the other; an `AttributeError` or `OSError` (by the
    /// acknowledged ancestry) is swallowed and the next write follows, any other exception stops the parser at that write.
    fn parser_error(&mut self, usage: &PyStr, message: &PyStr) {
        for text in [usage, message].into_iter().filter(|text| !text.is_empty()) {
            let outcome = self.write("stderr", text);
            if let Some(failure) = parser_write_failure(outcome) {
                self.parser_failure = Some(failure);
                return;
            }
        }
    }

    fn parser_help(&mut self, help: &PyStr) {
        let outcome = self.write("stdout", help);
        if let Some(failure) = parser_write_failure(outcome) {
            self.parser_failure = Some(failure);
        }
    }

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.parser_failure.take()
    }
}

impl Host for EventHost<'_> {
    fn import_yaml(&mut self) -> Done<()> {
        let ack = self.exchange("yaml_import", vec![("module", PyValue::str("yaml"))])?;
        match parse_import_ack(&ack) {
            Ok(outcome) => outcome,
            Err(why) => Err(self.abandon(&why)),
        }
    }
}

/// Reads the one request line (without its line end) from the caller's descriptor.
pub fn read_request(acks: &mut dyn BufRead) -> Result<PyStr, String> {
    let mut raw = Vec::new();
    acks.read_until(b'\n', &mut raw)
        .map_err(|error| format!("cannot read the request: {error}"))?;
    if raw.last() == Some(&b'\n') {
        raw.pop();
    }
    Ok(PyStr::from_surrogateescape(&raw))
}

/// One event run: the request, the call with its synchronous writes and import, the result event. `Err` is a transport failure (no result was sent).
pub fn run_events(
    request: &PyStr,
    defaults: &Defaults,
    events: &mut dyn Write,
    acks: &mut dyn BufRead,
) -> Result<(), String> {
    let mut host = EventHost::new(&mut *events, &mut *acks);
    let record = answer_on(request, defaults, &mut host);
    if let Some(why) = host.fatal.take() {
        return Err(why);
    }
    let mut result = PyDict::new();
    result.set("event", PyValue::str("result"));
    result.set("record", record);
    let line = PyValue::Dict(result)
        .dumps_ascii()
        .map_err(|error| format!("cannot encode the result: {error}"))?;
    writeln!(events, "{line}")
        .and_then(|()| events.flush())
        .map_err(|error| format!("cannot send the result: {error}"))
}
