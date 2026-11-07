//! `fetch_json(url)` of `resolve_wheel.py` (lines 18-22):
//!
//! ```python
//! def fetch_json(url: str):
//!     req = urllib.request.Request(
//!         url, headers={"Accept": "application/vnd.github+json", "User-Agent": "upstream-version-watch"})
//!     with urllib.request.urlopen(req, timeout=20) as r:
//!         return json.load(r)
//! ```
//!
//! The call is one `urllib.request.Request` with the two caller headers, opened by the handler of its scheme (`http` direct, `https` directly or
//! through a `CONNECT` tunnel, `file` for the local host), and the body read as RAW BYTES by `json.load`, never decoded as UTF-8 text first. The
//! transports are the shared [`crate::http_direct`] and [`crate::fetch_https`] engines with the caller-supplied header seam
//! ([`crate::http_direct::request::RequestHeaders`]); the byte decoding and scanning is [`crate::wheel_json`].
//!
//! The URL argument is any Python value: `Request` applies `str()` to it, so `None`, `False` or a `bytes` object become URL text and fail as the
//! original does (`unknown url type`). Outside this component's scope, named and never guessed: the `ftp` and `data` handlers, a `file` URL with a host,
//! proxies other than the captured `https` tunnel form, and socket failures whose Python spelling was not captured. The one socket failure that
//! was captured is the 20-second read timeout while the response head is awaited (`TimeoutError: timed out`).

use crate::fetch_https::exchange_with;
use crate::fetch_https::FetchEnvironment;
use crate::http_direct::failure::{ErrorValue, FetchFailure, ScopeRefusal, TransportFailure};
use crate::http_direct::opener::{drive, PlainHttp};
use crate::http_direct::request::{RequestHeaders, SurrogateCarrier};
use crate::http_direct::url::RequestUrl;
use crate::http_direct::{Dial, PyException};
use crate::wheel_json::{loads_bytes, JsonFailure, JsonSite};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

/// The headers of the `Request`, in the original's order.
pub const API_HEADERS: [(&str, &str); 2] = [
    ("Accept", "application/vnd.github+json"),
    ("User-Agent", "upstream-version-watch"),
];

/// Where a raised exception came from when that decides what a traceback shows beyond the exception itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// `urlopen` and the stack below it.
    UrlOpen,
    /// `json.load` (the site of the `json` package and whether the exception was raised `from None`).
    Json { site: JsonSite, from_none: bool },
}

impl Origin {
    /// Whether the exception was raised `from None`, so that its `__suppress_context__` is true and it has no linked exception.
    pub fn suppresses_context(self) -> bool {
        matches!(
            self,
            Origin::Json {
                from_none: true,
                ..
            }
        )
    }
}

/// The ways `fetch_json` ends without a value.
#[derive(Clone, Debug)]
pub enum FetchError {
    Raised {
        error: Box<PyException>,
        origin: Origin,
    },
    OutOfScope(ScopeRefusal),
    Transport(TransportFailure),
}

impl From<FetchFailure> for FetchError {
    fn from(failure: FetchFailure) -> FetchError {
        match failure {
            FetchFailure::Raised(error) => FetchError::Raised {
                error,
                origin: Origin::UrlOpen,
            },
            FetchFailure::OutOfScope(refusal) => FetchError::OutOfScope(refusal),
            FetchFailure::Transport(transport) => read_timeout_or_transport(transport),
        }
    }
}

impl From<JsonFailure> for FetchError {
    fn from(failure: JsonFailure) -> FetchError {
        FetchError::Raised {
            error: failure.error,
            origin: Origin::Json {
                site: failure.site,
                from_none: failure.from_none,
            },
        }
    }
}

/// `OSError` raised with a message only (`errno`, `strerror`, `filename`, `filename2` are all `None`), here `TimeoutError("timed out")`.
fn timeout_error() -> PyException {
    PyException {
        class: "TimeoutError",
        module: "builtins",
        message: PyStr::new("timed out"),
        args: vec![PyValue::str("timed out").into()],
        attributes: ["errno", "strerror", "filename", "filename2"]
            .into_iter()
            .map(|name| (name, PyValue::None.into()))
            .collect(),
        http_headers: None,
        link: None,
    }
}

/// The socket timeout while the response head is awaited is `TimeoutError: timed out`, raised by `getresponse` outside the `except OSError` of
/// `do_open`, so it is not wrapped. Every other transport failure keeps its unclaimed form.
fn read_timeout_or_transport(transport: TransportFailure) -> FetchError {
    if transport.phase == "receive" && transport.detail.starts_with("timeout") {
        return FetchError::Raised {
            error: Box::new(timeout_error()),
            origin: Origin::UrlOpen,
        };
    }
    FetchError::Transport(transport)
}

/// `URLError(reason)` for a reason that is text: no exception was being handled, so there is no context.
fn url_error_of_text(reason: &str) -> PyException {
    PyException {
        class: "URLError",
        module: "urllib.error",
        message: PyStr::new(&format!("<urlopen error {reason}>")),
        args: vec![ErrorValue::Plain(PyValue::str(reason))],
        attributes: vec![
            ("errno", PyValue::None.into()),
            ("strerror", PyValue::None.into()),
            ("filename", PyValue::None.into()),
            ("filename2", PyValue::None.into()),
            ("reason", PyValue::str(reason).into()),
        ],
        http_headers: None,
        link: None,
    }
}

/// `urllib.parse.unquote`: `%XX` sequences become bytes, the whole read as UTF-8 with replacement characters.
fn unquote(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let pair = bytes
            .get(at + 1..at + 3)
            .and_then(|hex| std::str::from_utf8(hex).ok())
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match (bytes[at], pair) {
            (b'%', Some(byte))
                if bytes[at + 1].is_ascii_hexdigit() && bytes[at + 2].is_ascii_hexdigit() =>
            {
                decoded.push(byte);
                at += 3;
            }
            (byte, _) => {
                decoded.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// `OSError` of a file as the interpreter spells it: the subclass for the `errno`, `strerror`, and the `filename`.
fn file_error(errno: i32, path: &str) -> Option<PyException> {
    let (class, strerror) = match errno {
        2 => ("FileNotFoundError", "No such file or directory"),
        13 => ("PermissionError", "Permission denied"),
        20 => ("NotADirectoryError", "Not a directory"),
        21 => ("IsADirectoryError", "Is a directory"),
        _ => return None,
    };
    let message = format!("[Errno {errno}] {strerror}: {}", PyStr::new(path).repr());
    Some(PyException {
        class,
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![PyValue::int(errno).into(), PyValue::str(strerror).into()],
        attributes: vec![
            ("errno", PyValue::int(errno).into()),
            ("strerror", PyValue::str(strerror).into()),
            ("filename", PyValue::str(path).into()),
            ("filename2", PyValue::None.into()),
        ],
        http_headers: None,
        link: None,
    })
}

/// `FileHandler.file_open` → `open_local_file` for a URL without a host: `os.stat` then `open(path, "rb").read()`; an `OSError` is wrapped in
/// `URLError`, which keeps it as its context.
fn open_file(request: &RequestUrl) -> Result<Vec<u8>, FetchError> {
    if !request.host.is_empty() {
        return Err(FetchError::OutOfScope(refusal(
            "file_host",
            "a file URL that names a host is resolved against the machine's names by urllib; only the empty host is in scope",
        )));
    }
    let path = unquote(&request.selector);
    if path.contains('\0') {
        return Err(FetchError::OutOfScope(refusal(
            "file_path",
            "a path with an embedded NUL raises ValueError in os.stat; not ported",
        )));
    }
    let read = std::fs::metadata(&path).and_then(|_| std::fs::read(&path));
    read.map_err(|failure| match failure.raw_os_error().and_then(|errno| file_error(errno, &path)) {
        Some(reason) => {
            let shown = reason.message.to_utf8().unwrap_or_default();
            let mut wrapped = url_error_of_text(&shown);
            wrapped.args = vec![ErrorValue::Exception(Box::new(reason.clone()))];
            wrapped.attributes.pop();
            wrapped.attributes.push(("reason", ErrorValue::Exception(Box::new(reason.clone()))));
            wrapped.link = Some(crate::http_direct::Link::Context(Box::new(reason)));
            FetchError::Raised { error: Box::new(wrapped), origin: Origin::UrlOpen }
        }
        None => FetchError::OutOfScope(refusal("file_error", format!("the operating system reported {failure} for the file; its Python spelling was not captured"))),
    })
}

fn refusal(boundary: &'static str, detail: impl Into<String>) -> ScopeRefusal {
    ScopeRefusal {
        boundary,
        detail: detail.into(),
    }
}

/// `str(url)` as `Request.full_url` takes it.
fn url_text(url: &PyValue) -> PyStr {
    match url {
        PyValue::Str(text) => text.clone(),
        other => other.display_text(),
    }
}

/// `urllib.request.urlopen(Request(url, headers=...), timeout=20)` up to the response body: the raw bytes of a 2xx answer.
pub fn open_url(
    url: &PyValue,
    headers: &RequestHeaders,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<Vec<u8>, FetchError> {
    let text = url_text(url);
    let Some((carried, carrier)) = SurrogateCarrier::carry(text.code_points()) else {
        return Err(FetchError::OutOfScope(refusal(
            "url_text",
            "the URL holds more lone surrogates than there are unused private-use characters to carry them",
        )));
    };
    let request = RequestUrl::parse(&carried)?;
    match request.scheme.as_str() {
        "http" => {
            if environment.proxies.has_http_proxy() {
                return Err(FetchError::OutOfScope(refusal(
                    "proxy_environment",
                    "an http proxy variable is set; urllib would route through it and this component only connects directly",
                )));
            }
            let exchange = PlainHttp { dial: dialer, headers, carrier: Some(&carrier) };
            Ok(drive(&exchange, request)?)
        }
        "https" => Ok(drive(&exchange_with(environment, dialer, &carrier, headers), request)?),
        "file" => open_file(&request),
        "ftp" | "data" => Err(FetchError::OutOfScope(refusal(
            "scheme",
            format!("scheme '{}' is handled by a urllib handler this component does not port (ftp and data)", request.scheme),
        ))),
        other => Err(FetchError::Raised { error: Box::new(url_error_of_text(&format!("unknown url type: {other}"))), origin: Origin::UrlOpen }),
    }
}

/// `fetch_json(url)` with the original's two headers.
pub fn fetch_json(
    url: &PyValue,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<PyValue, FetchError> {
    let headers =
        RequestHeaders::new(&API_HEADERS).expect("the two headers are plain ASCII tokens");
    let body = open_url(url, &headers, environment, dialer)?;
    loads_bytes(&body).map_err(FetchError::from)
}

/// [`fetch_json`] for a URL that is already text, over this process's environment and real connections.
pub fn fetch_json_process(url: &PyValue) -> Result<PyValue, FetchError> {
    fetch_json(
        url,
        &FetchEnvironment::from_process(),
        &crate::http_direct::UreqDialer::new(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unquote_decodes_percent_bytes_and_replaces_invalid_sequences() {
        assert_eq!(unquote("/a%20b%2Fc"), "/a b/c");
        assert_eq!(unquote("/%e2%82%ac"), "/€");
        assert_eq!(unquote("/%ff"), "/\u{fffd}");
        assert_eq!(unquote("/100%/x%zz"), "/100%/x%zz");
    }

    #[test]
    fn a_missing_file_is_a_url_error_holding_the_file_not_found_error_as_reason_and_context() {
        let request = RequestUrl::parse("file:///definitely/absent/doc.json").unwrap();
        let Err(FetchError::Raised { error, .. }) = open_file(&request) else {
            panic!("raised")
        };
        assert_eq!(error.class, "URLError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "<urlopen error [Errno 2] No such file or directory: '/definitely/absent/doc.json'>"
        );
        let Some(crate::http_direct::Link::Context(reason)) = &error.link else {
            panic!("context")
        };
        assert_eq!(reason.class, "FileNotFoundError");
        assert_eq!(reason.args.len(), 2);
    }

    #[test]
    fn a_non_text_url_becomes_text_and_an_unknown_scheme_is_a_url_error_without_context() {
        let environment = FetchEnvironment::default();
        let dialer = crate::http_direct::UreqDialer::new();
        let headers = RequestHeaders::default();
        let url = PyValue::Bytes(b"http://127.0.0.1:1/json".to_vec());
        let Err(FetchError::Raised { error, .. }) = open_url(&url, &headers, &environment, &dialer)
        else {
            panic!("raised")
        };
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "<urlopen error unknown url type: b'http>"
        );
        assert!(error.link.is_none());
        let Err(FetchError::Raised { error, .. }) =
            open_url(&PyValue::None, &headers, &environment, &dialer)
        else {
            panic!("raised")
        };
        assert_eq!(
            (error.class, error.message.to_utf8().unwrap().as_str()),
            ("ValueError", "unknown url type: 'None'")
        );
    }

    #[test]
    fn a_read_timeout_is_the_timeout_error_and_other_socket_failures_stay_unclaimed() {
        let timeout = TransportFailure {
            phase: "receive",
            detail: "timeout: per-read".to_string(),
        };
        assert!(matches!(
            FetchError::from(FetchFailure::Transport(timeout)),
            FetchError::Raised { .. }
        ));
        let refused = TransportFailure {
            phase: "connect",
            detail: "Connection refused".to_string(),
        };
        assert!(matches!(
            FetchError::from(FetchFailure::Transport(refused)),
            FetchError::Transport(_)
        ));
    }
}
