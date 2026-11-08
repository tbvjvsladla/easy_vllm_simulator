//! What can end a direct-HTTP fetch, kept apart by what is claimed about it.
//!
//! * [`PyException`]: an exception the original `fetch` raises (class, module, `str()`, `args`, the captured public attributes, the
//!   headers of an `HTTPError`, and the `__cause__`/`__context__` chain). Built from the CPython `http.client` / `urllib` sources.
//! * [`ScopeRefusal`]: an input or situation this component does not implement. Nothing is claimed about what the original would do.
//! * [`TransportFailure`]: a real socket-level failure. Its Python spelling (`URLError`, `TimeoutError`, ...) was never captured, so it is
//!   reported as what ureq saw and not as a Python exception.
//!
//! The HTTPS fetch (`crate::fetch_https`) shares these types; the constructors at the end of the exceptions ([`url_error`], [`os_error`],
//! [`ssl_cert_verification_error`]) are the ones it needs for the TLS and CONNECT failures it models.

use agent_runtime_support::pyjson::utf8_error_text;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use num_bigint::BigInt;

pub const HTTP_CLIENT_MODULE: &str = "http.client";

/// How an exception links to the next exception of its chain.
#[derive(Clone, Debug)]
pub enum Link {
    /// `__context__` (raised while the other was being handled).
    Context(Box<PyException>),
    /// `__cause__` (`raise ... from other`; it also sets `__suppress_context__`).
    Cause(Box<PyException>),
}

/// An element of an exception's `args` or the value of a captured attribute: a plain value, or another exception (`URLError.args[0]` and
/// `URLError.reason` hold the exception that was wrapped).
#[derive(Clone, Debug)]
pub enum ErrorValue {
    Plain(PyValue),
    Exception(Box<PyException>),
    /// An `http.client.HTTPMessage`: the ordered raw header items (duplicates and spelling kept), as the public value
    /// `["http_message", {type, module, raw_items}]`.
    HttpMessage(Vec<(String, String)>),
    /// A tuple of public values (`ContentTooShortError.content` is `(path, HTTPMessage)`).
    Tuple(Vec<ErrorValue>),
    /// A `yaml.error.Mark` (the six public fields: name, index, line, column, buffer, pointer), as the public value
    /// `["yaml_mark", {module, type, fields, extra_attributes}]`.
    YamlMark(Box<agent_runtime_support::pyyaml::Mark6>),
}

impl From<PyValue> for ErrorValue {
    fn from(value: PyValue) -> ErrorValue {
        ErrorValue::Plain(value)
    }
}

/// The public value of a `yaml.error.Mark`: `["yaml_mark", {module: "yaml.error", type: "Mark", fields: {name, index, line, column, buffer, pointer},
/// extra_attributes: {}}]`; `buffer` and `pointer` are `None` for a mark of a file stream.
pub fn yaml_mark_dto(mark: &agent_runtime_support::pyyaml::Mark6) -> PyValue {
    use agent_runtime_support::pyvalue::PyDict;
    let none = || PyValue::List(vec![PyValue::str("none")]);
    let int =
        |value: usize| PyValue::List(vec![PyValue::str("int"), PyValue::str(&value.to_string())]);
    let mut fields = PyDict::new();
    fields.set(
        "name",
        PyValue::List(vec![PyValue::str("str"), PyValue::Str(mark.name.clone())]),
    );
    fields.set("index", int(mark.index));
    fields.set("line", int(mark.line));
    fields.set("column", int(mark.column));
    fields.set(
        "buffer",
        mark.buffer.as_ref().map_or_else(none, |b| {
            PyValue::List(vec![PyValue::str("str"), PyValue::Str(b.clone())])
        }),
    );
    fields.set("pointer", mark.pointer.map_or_else(none, int));
    let mut node = PyDict::new();
    node.set("module", PyValue::str("yaml.error"));
    node.set("type", PyValue::str("Mark"));
    node.set("fields", PyValue::Dict(fields));
    node.set("extra_attributes", PyValue::Dict(PyDict::new()));
    PyValue::List(vec![PyValue::str("yaml_mark"), PyValue::Dict(node)])
}

/// The public value of an `http.client.HTTPMessage`: `["http_message", {type, module, raw_items}]`, `raw_items` being the ordered
/// `[name, value]` pairs as received.
pub fn http_message_dto(raw_items: &[(String, String)]) -> PyValue {
    let mut node = agent_runtime_support::pyvalue::PyDict::new();
    node.set("type", PyValue::str("HTTPMessage"));
    node.set("module", PyValue::str(HTTP_CLIENT_MODULE));
    node.set(
        "raw_items",
        PyValue::List(
            raw_items
                .iter()
                .map(|(name, value)| PyValue::List(vec![PyValue::str(name), PyValue::str(value)]))
                .collect(),
        ),
    );
    PyValue::List(vec![PyValue::str("http_message"), PyValue::Dict(node)])
}

/// An exception of the original, with what the selected contract captures of it.
#[derive(Clone, Debug)]
pub struct PyException {
    pub class: &'static str,
    pub module: &'static str,
    pub message: PyStr,
    pub args: Vec<ErrorValue>,
    /// Public attributes the contract captures, in any order (the record writes them in the contract's order).
    pub attributes: Vec<(&'static str, ErrorValue)>,
    /// `HTTPError.headers.raw_items()`.
    pub http_headers: Option<Vec<(String, String)>>,
    pub link: Option<Link>,
}

impl PyException {
    fn plain(class: &'static str, module: &'static str, message: &str) -> PyException {
        PyException {
            class,
            module,
            message: PyStr::new(message),
            args: vec![PyValue::str(message).into()],
            attributes: Vec::new(),
            http_headers: None,
            link: None,
        }
    }

    /// Raised while `context` was being handled.
    pub fn during(mut self, context: PyException) -> PyException {
        self.link = Some(Link::Context(Box::new(context)));
        self
    }

    /// `raise self from cause`.
    pub fn caused_by(mut self, cause: PyException) -> PyException {
        self.link = Some(Link::Cause(Box::new(cause)));
        self
    }

    pub fn is_class(&self, class: &str) -> bool {
        self.class == class
    }

    /// `isinstance(exception, OSError)` for the classes this component can raise: the ones `urllib`'s `do_open` wraps in `URLError`
    /// (`BadStatusLine` and the other `HTTPException`s are not `OSError`s and pass through unwrapped; `RemoteDisconnected` is both).
    pub fn is_os_error(&self) -> bool {
        ["OSError", "RemoteDisconnected", "SSLCertVerificationError"].contains(&self.class)
    }

    fn with_attribute(mut self, name: &'static str, value: impl Into<ErrorValue>) -> PyException {
        self.attributes.push((name, value.into()));
        self
    }
}

pub fn http_exception(message: &str) -> PyException {
    PyException::plain("HTTPException", HTTP_CLIENT_MODULE, message)
}

/// `InvalidURL(message)` of `http.client`.
pub fn invalid_url(message: &str) -> PyException {
    PyException::plain("InvalidURL", HTTP_CLIENT_MODULE, message)
}

/// `UnicodeEncodeError('ascii', object, start, end, 'ordinal not in range(128)')` as `request.encode('ascii')` raises it in
/// `http.client.HTTPConnection._encode_request`: `object` is the whole text that was encoded (as code points, a lone surrogate included),
/// `start..end` the first run of characters above U+007F. `str()` is built from those fields the way `UnicodeEncodeError.__str__` builds it
/// (one character: its escape and position; a run: the position range) and the constructor arguments and attributes carry the same fields.
pub fn ascii_encode_error(object: &[u32], start: usize, end: usize) -> PyException {
    const REASON: &str = "ordinal not in range(128)";
    let escaped = |point: u32| match point {
        0..=0xFF => format!("\\x{point:02x}"),
        0x100..=0xFFFF => format!("\\u{point:04x}"),
        _ => format!("\\U{point:08x}"),
    };
    let message = if end - start == 1 {
        format!(
            "'ascii' codec can't encode character '{}' in position {start}: {REASON}",
            escaped(object[start])
        )
    } else {
        format!(
            "'ascii' codec can't encode characters in position {start}-{}: {REASON}",
            end - 1
        )
    };
    let text = || PyValue::Str(PyStr::from_code_points(object.to_vec()));
    PyException {
        class: "UnicodeEncodeError",
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![
            PyValue::str("ascii").into(),
            text().into(),
            PyValue::int(start).into(),
            PyValue::int(end).into(),
            PyValue::str(REASON).into(),
        ],
        attributes: vec![
            ("reason", PyValue::str(REASON).into()),
            ("encoding", PyValue::str("ascii").into()),
            ("object", text().into()),
            ("start", PyValue::int(start).into()),
            ("end", PyValue::int(end).into()),
        ],
        http_headers: None,
        link: None,
    }
}

/// `LineTooLong(line_type)`.
pub fn line_too_long(line_type: &str, limit: usize) -> PyException {
    PyException::plain(
        "LineTooLong",
        HTTP_CLIENT_MODULE,
        &format!("got more than {limit} bytes when reading {line_type}"),
    )
}

/// `UnknownProtocol(version)`.
pub fn unknown_protocol(version: &str) -> PyException {
    PyException::plain("UnknownProtocol", HTTP_CLIENT_MODULE, version)
}

/// `BadStatusLine(line)`; an empty line is shown as its repr, as `BadStatusLine.__init__` does.
pub fn bad_status_line(line: &str) -> PyException {
    let shown = if line.is_empty() { "''" } else { line };
    PyException::plain("BadStatusLine", HTTP_CLIENT_MODULE, shown)
        .with_attribute("line", PyValue::str(shown))
}

/// `RemoteDisconnected("Remote end closed connection without response")`: a `ConnectionResetError` that is also a `BadStatusLine`.
pub fn remote_disconnected() -> PyException {
    let mut error = PyException::plain(
        "RemoteDisconnected",
        HTTP_CLIENT_MODULE,
        "Remote end closed connection without response",
    );
    error = without_os_error_detail(error);
    error.with_attribute("line", PyValue::str("''"))
}

/// The four `OSError` attributes of an exception raised with a message only (`errno`, `strerror`, `filename`, `filename2` are all `None`).
fn without_os_error_detail(error: PyException) -> PyException {
    ["errno", "strerror", "filename", "filename2"]
        .into_iter()
        .fold(error, |error, name| {
            error.with_attribute(name, PyValue::None)
        })
}

/// `IncompleteRead(partial, expected)`.
pub fn incomplete_read(partial: Vec<u8>, expected: Option<BigInt>) -> PyException {
    let detail = match &expected {
        Some(more) => format!(", {more} more expected"),
        None => String::new(),
    };
    let message = format!("IncompleteRead({} bytes read{detail})", partial.len());
    PyException {
        class: "IncompleteRead",
        module: HTTP_CLIENT_MODULE,
        message: PyStr::new(&message),
        args: vec![PyValue::Bytes(partial.clone()).into()],
        attributes: vec![
            ("partial", PyValue::Bytes(partial).into()),
            (
                "expected",
                expected.map_or(PyValue::None, PyValue::int).into(),
            ),
        ],
        http_headers: None,
        link: None,
    }
}

pub fn value_error(message: &str) -> PyException {
    PyException::plain("ValueError", "builtins", message)
}

/// The `UnicodeDecodeError` of `bytes.decode("utf-8")` for the failure `std::str::from_utf8` reported on `object`.
pub fn utf8_decode_error(object: &[u8], failure: &std::str::Utf8Error) -> PyException {
    let (start, length, reason) = agent_runtime_support::pyjson::utf8_error_parts(object, failure);
    PyException {
        class: "UnicodeDecodeError",
        module: "builtins",
        message: PyStr::new(&utf8_error_text(object, failure)),
        args: vec![
            PyValue::str("utf-8").into(),
            PyValue::Bytes(object.to_vec()).into(),
            PyValue::int(start).into(),
            PyValue::int(start + length).into(),
            PyValue::str(reason).into(),
        ],
        attributes: vec![
            ("reason", PyValue::str(reason).into()),
            ("encoding", PyValue::str("utf-8").into()),
            ("object", PyValue::Bytes(object.to_vec()).into()),
            ("start", PyValue::int(start).into()),
            ("end", PyValue::int(start + length).into()),
        ],
        http_headers: None,
        link: None,
    }
}

/// `urllib.error.HTTPError(url, code, msg, hdrs, fp)` as the selected contract captures it (the live `fp` object is not modelled).
pub fn http_error(url: &str, code: u16, reason: &str, headers: &[(String, String)]) -> PyException {
    let attributes = vec![
        ("errno", PyValue::None.into()),
        ("strerror", PyValue::None.into()),
        ("filename", PyValue::str(url).into()),
        ("filename2", PyValue::None.into()),
        ("code", PyValue::int(code).into()),
        ("url", PyValue::str(url).into()),
        ("reason", PyValue::str(reason).into()),
    ];
    PyException {
        class: "HTTPError",
        module: "urllib.error",
        message: PyStr::new(&format!("HTTP Error {code}: {reason}")),
        args: Vec::new(),
        attributes,
        http_headers: Some(headers.to_vec()),
        link: None,
    }
}

/// The `[SSL: CERTIFICATE_VERIFY_FAILED]` text CPython 3.12.3's `_ssl.c` builds, with the source line number of that interpreter's
/// build (the frozen profile of the saved originals; this port never runs `_ssl.c`).
const SSL_SITE: &str = "(_ssl.c:1000)";

/// `ssl.SSLCertVerificationError` as CPython 3.12.3 reports a failed certificate check: `verify_code` is the X.509 verify result number and
/// `verify_message` its text, which the caller derives from the TLS error and the real host.
pub fn ssl_cert_verification_error(verify_code: i64, verify_message: &str) -> PyException {
    const REASON: &str = "CERTIFICATE_VERIFY_FAILED";
    let text = format!("[SSL: {REASON}] certificate verify failed: {verify_message} {SSL_SITE}");
    PyException {
        class: "SSLCertVerificationError",
        module: "ssl",
        message: PyStr::new(&text),
        args: vec![PyValue::int(1).into(), PyValue::str(&text).into()],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    }
    .with_attribute("errno", PyValue::int(1))
    .with_attribute("strerror", PyValue::str(&text))
    .with_attribute("filename", PyValue::None)
    .with_attribute("filename2", PyValue::None)
    .with_attribute("reason", PyValue::str(REASON))
    .with_attribute("verify_code", PyValue::int(verify_code))
    .with_attribute("verify_message", PyValue::str(verify_message))
    .with_attribute("library", PyValue::str("SSL"))
}

/// `OSError(message)`: the exception `http.client` raises for a failed CONNECT tunnel.
pub fn os_error(message: &str) -> PyException {
    without_os_error_detail(PyException::plain("OSError", "builtins", message))
}

/// `urllib.error.URLError(reason)` raised inside the `except OSError` of `do_open`: its only argument and its `reason` are the wrapped
/// exception, which is also its `__context__`.
pub fn url_error(reason: PyException) -> PyException {
    let shown = reason
        .message
        .to_utf8()
        .expect("the messages this component builds are valid UTF-8");
    let message = format!("<urlopen error {shown}>");
    let mut error = PyException {
        class: "URLError",
        module: "urllib.error",
        message: PyStr::new(&message),
        args: vec![ErrorValue::Exception(Box::new(reason.clone()))],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    };
    for name in ["errno", "strerror", "filename", "filename2"] {
        error = error.with_attribute(name, PyValue::None);
    }
    error
        .with_attribute("reason", ErrorValue::Exception(Box::new(reason.clone())))
        .during(reason)
}

/// An input form or situation outside the implemented direct-HTTP scope.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeRefusal {
    /// Stable machine name of the boundary.
    pub boundary: &'static str,
    pub detail: String,
}

/// A real I/O failure of the transport.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransportFailure {
    /// `connect`, `send` or `receive`.
    pub phase: &'static str,
    pub detail: String,
}

/// The ways a fetch ends without a text.
#[derive(Clone, Debug)]
pub enum FetchFailure {
    /// Boxed: an exception with its chain is large next to the other two variants.
    Raised(Box<PyException>),
    OutOfScope(ScopeRefusal),
    Transport(TransportFailure),
}

pub type Fetched<T> = Result<T, FetchFailure>;

impl From<PyException> for FetchFailure {
    fn from(error: PyException) -> FetchFailure {
        FetchFailure::Raised(Box::new(error))
    }
}

impl From<TransportFailure> for FetchFailure {
    fn from(failure: TransportFailure) -> FetchFailure {
        FetchFailure::Transport(failure)
    }
}

/// A refusal of the scope boundary `boundary`.
pub fn out_of_scope(boundary: &'static str, detail: impl Into<String>) -> FetchFailure {
    FetchFailure::OutOfScope(ScopeRefusal {
        boundary,
        detail: detail.into(),
    })
}
