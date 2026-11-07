//! `recipe_cleanup_api evaluate-events`: one `cleanup_docker` call whose console is the CALLER's, written synchronously (see `ABI.md`, section B).
//!
//! Every real console write of the business code (`print` is two) is announced as a `write` event and acknowledged before the call goes
//! on, so a failure of the caller's real stream is the exception of that write and stops the business code where the original's `print`
//! raised. The program decides nothing about the streams: it announces, waits, and either continues or raises what it was told.
//! A missing or malformed acknowledgement aborts the run: no result and no later effect.
//!
//! Descriptor 0 is never touched; the requests and acknowledgements come from the descriptor the caller passed, the events and the result go
//! to standard output.

use crate::cleanup_api::{answer_on, Defaults};
use crate::cleanup_docker::{parser_write_failure, CleanupConsole, Ctor, Done, Thrown};
use crate::escalation::Raised;
use crate::model_dyn::is_os_error;
use crate::pyexpr::Res;
use crate::transform_cli::Console;
use agent_runtime_support::pyjson::loads_text_objects;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{BufRead, Write};

/// The console of an event run.
pub struct EventConsole<'a> {
    events: &'a mut dyn Write,
    acks: &'a mut dyn BufRead,
    sequence: i64,
    /// Why the run must be abandoned (set once; the business code is stopped by the error returned with it).
    pub fatal: Option<String>,
    /// The first argparse output write that failed with an exception argparse does not swallow.
    parser_failure: Option<Box<Thrown>>,
}

impl<'a> EventConsole<'a> {
    pub fn new(events: &'a mut dyn Write, acks: &'a mut dyn BufRead) -> EventConsole<'a> {
        EventConsole {
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
        Thrown::plain(Raised::plain("ProtocolError", why))
    }

    /// Announces one write and waits for its acknowledgement: `Ok` when the caller's stream took it, the exception it reported otherwise.
    fn write(&mut self, stream: &str, text: &PyStr) -> Done<()> {
        if let Some(why) = self.fatal.clone() {
            return Err(Thrown::plain(Raised::plain("ProtocolError", &why)));
        }
        let mut event = PyDict::new();
        event.set("event", PyValue::str("write"));
        event.set("sequence", PyValue::int(self.sequence));
        event.set("stream", PyValue::str(stream));
        event.set("text", PyValue::Str(text.clone()));
        let line = match PyValue::Dict(event).dumps_ascii() {
            Ok(line) => line,
            Err(error) => {
                return Err(self.abandon(&format!("cannot encode a write event: {error}")))
            }
        };
        if let Err(error) = writeln!(self.events, "{line}").and_then(|()| self.events.flush()) {
            return Err(self.abandon(&format!("cannot send a write event: {error}")));
        }
        let mut raw = Vec::new();
        match self.acks.read_until(b'\n', &mut raw) {
            Ok(0) => return Err(self.abandon("the acknowledgement of a write never came")),
            Ok(_) => {}
            Err(error) => {
                return Err(self.abandon(&format!("cannot read an acknowledgement: {error}")))
            }
        }
        let outcome = parse_ack(&raw, self.sequence);
        self.sequence += 1;
        match outcome {
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

/// The constructor DTO of an acknowledged exception (`unicode_encode` / `unicode_decode`).
fn constructor_of(value: &PyValue) -> Result<Ctor, String> {
    let bad = || "an acknowledgement has an invalid constructor".to_string();
    let PyValue::Dict(map) = value else {
        return Err(bad());
    };
    let utf8 = |key: &str| text_member(map, key).and_then(|text| text.to_utf8());
    let (kind, encoding, reason) = (utf8("kind"), utf8("encoding"), utf8("reason"));
    let (Some(kind), Some(encoding), Some(reason)) = (kind, encoding, reason) else {
        return Err(bad());
    };
    let (Some(start), Some(end)) = (index_member(map, "start"), index_member(map, "end")) else {
        return Err(bad());
    };
    let names = |expected: &[&str]| {
        map.len() == expected.len() && expected.iter().all(|name| map.contains_key(name))
    };
    match kind.as_str() {
        "unicode_encode" if names(&["kind", "encoding", "object", "start", "end", "reason"]) => {
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
            if names(&["kind", "encoding", "object_hex", "start", "end", "reason"]) =>
        {
            let hex = utf8("object_hex").ok_or_else(bad)?;
            if hex.len() % 2 != 0 || !hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(bad());
            }
            let object = (0..hex.len() / 2)
                .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).map_err(|_| bad()))
                .collect::<Result<Vec<u8>, String>>()?;
            Ok(Ctor::Unicode {
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

/// One acknowledgement line: `Ok(None)` for success, `Ok(Some(error))` for the exception it reports, `Err` for an acknowledgement that
/// is not valid (the run is abandoned).
fn parse_ack(raw: &[u8], sequence: i64) -> Result<Option<Box<Thrown>>, String> {
    let invalid = |why: &str| format!("invalid acknowledgement: {why}");
    let line = PyStr::from_surrogateescape(raw);
    let PyValue::Dict(ack) = loads_text_objects(&line).map_err(|_| invalid("not JSON"))? else {
        return Err(invalid("not an object"));
    };
    if text_member(&ack, "event").is_none_or(|event| !event.equals("write_ack")) {
        return Err(invalid("not a write_ack"));
    }
    if int_member(&ack, "sequence") != Some(sequence) {
        return Err(invalid("out of sequence"));
    }
    let Some(kind) = text_member(&ack, "kind") else {
        return Err(invalid("no kind"));
    };
    let names = |expected: &[&str]| {
        ack.len() == expected.len() && expected.iter().all(|name| ack.contains_key(name))
    };
    if kind.equals("ok") {
        return if names(&["event", "sequence", "kind"]) {
            Ok(None)
        } else {
            Err(invalid("members of an ok acknowledgement"))
        };
    }
    if !kind.equals("exception") {
        return Err(invalid("unknown kind"));
    }
    let with_constructor = ack.contains_key("constructor");
    let expected: &[&str] = if with_constructor {
        &[
            "event",
            "sequence",
            "kind",
            "type",
            "module",
            "message",
            "builtin_bases",
            "constructor",
        ]
    } else {
        &[
            "event",
            "sequence",
            "kind",
            "type",
            "module",
            "message",
            "builtin_bases",
        ]
    };
    if !names(expected) {
        return Err(invalid("members of an exception acknowledgement"));
    }
    let (Some(class), Some(module), Some(message)) = (
        text_member(&ack, "type").and_then(|text| text.to_utf8()),
        text_member(&ack, "module").and_then(|text| text.to_utf8()),
        text_member(&ack, "message"),
    ) else {
        return Err(invalid("type, module and message are text"));
    };
    let bases = bases_of(ack.get("builtin_bases"))?;
    // a constructor belongs to exactly the exceptions that ARE a `UnicodeEncodeError` or a `UnicodeDecodeError` (by their real ancestry)
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

/// The `builtin_bases` descriptor of an exception acknowledgement: the canonical builtin classes of the real MRO, in order. It must be a
/// non-empty list of distinct non-empty names that ends with `BaseException` (or `BaseException`, `object`: the builtin `object` every MRO
/// ends in, which has no exception meaning). The immediate `type` and `module` are descriptive strings, never proof of a canonical identity
/// (`type("OSError", (Exception,), {"__module__": "builtins"})` is a valid class whose canonical ancestry is `Exception`): the constraints on
/// the hierarchy are read from the canonical names PRESENT in the list, an `OSError`-family builtin has `OSError`, the Unicode classes have
/// `UnicodeError` and `ValueError`.
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
    if bases
        .iter()
        .any(|base| is_os_error(base) || base == "ConnectionError")
        && !has("OSError")
    {
        return Err(bad("names an OSError-family class without OSError"));
    }
    if (has("UnicodeEncodeError") || has("UnicodeDecodeError"))
        && !(has("UnicodeError") && has("ValueError"))
    {
        return Err(bad("lacks UnicodeError and ValueError for a Unicode class"));
    }
    Ok(bases)
}

impl CleanupConsole for EventConsole<'_> {
    fn out(&mut self, text: &PyStr) -> Done<()> {
        self.write("stdout", text)
    }

    fn err(&mut self, text: &PyStr) -> Done<()> {
        self.write("stderr", text)
    }

    fn take_parser_failure(&mut self) -> Option<Box<Thrown>> {
        self.parser_failure.take()
    }
}

impl Console for EventConsole<'_> {
    fn print_stderr(&mut self, text: &PyStr) -> Res<()> {
        let _ = self.write("stderr", text);
        Ok(())
    }

    fn write_stdout(&mut self, text: &PyStr) {
        let _ = self.write("stdout", text);
    }

    /// argparse's usage and error writes (`_print_message`): announced one after the other; an `AttributeError` or `OSError` of the caller's
    /// stream is swallowed and the next write follows, any other exception stops the parser at that write.
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

    fn write_uncaught(&mut self, _text: &PyStr) {}

    fn read_stdin(&mut self) -> Result<PyStr, Raised> {
        Ok(PyStr::default())
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

/// One event run: the request, the call with its synchronous writes, the result event. `Err` is a transport failure (no result was sent).
pub fn run_events(
    request: &PyStr,
    defaults: &Defaults,
    events: &mut dyn Write,
    acks: &mut dyn BufRead,
) -> Result<(), String> {
    let mut console = EventConsole::new(&mut *events, &mut *acks);
    let record = answer_on(request, defaults, &mut console);
    if let Some(why) = console.fatal.take() {
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
