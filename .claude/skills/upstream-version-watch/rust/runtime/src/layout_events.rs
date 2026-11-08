//! `upstream_layout_api evaluate-events`: one call whose console is the CALLER's, written synchronously (the R33 write/acknowledgement contract of
//! [`crate::events`], without an import event: this owner imports nothing).

use crate::events::EventHost;
use crate::layout_api::{answer_on, Defaults};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{BufRead, Write};

pub use crate::events::read_request;

/// One event run: the request, the call with its synchronous writes, the result event. `Err` is a transport failure (no result was sent).
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
