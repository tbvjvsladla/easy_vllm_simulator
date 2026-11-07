//! `resolve_torch_pin.fetch(url)` for a plain-`str` **`http://` URL over a direct connection**, as native Rust.
//!
//! The original is
//!
//! ```python
//! def fetch(url: str) -> str:
//!     with urllib.request.urlopen(url, timeout=20) as r:
//!         return r.read().decode("utf-8")
//! ```
//!
//! This module reproduces the selected behaviour of that call for HTTP: the returned text (strict UTF-8, line breaks and BOM kept), the
//! exceptions `urllib`/`http.client` raise with their class, module, message, arguments, captured attributes and `__cause__`/`__context__`
//! chain, the redirect rules, and the exact bytes of the `GET` request. The HTTP engine is a port of CPython 3.12.3's `http.client` and
//! `urllib.request` (see `NOTICE-cpython-http.md`), speaking over a real socket through `ureq`'s transport chain used as a raw byte
//! pipe.
//!
//! **Scope** — named in [`SCOPE`] and in every API name here: HTTP only, direct only. Not covered, and refused by name rather than guessed
//! ([`FetchFailure::OutOfScope`]): TLS, HTTP proxies from the environment, other schemes (`file`, `data`, `ftp`, `https`), non-`str`
//! URLs, the live file-like object of `HTTPError`, connect/DNS/timeout failures (reported as [`FetchFailure::Transport`], not as Python
//! exceptions), and whole-script `main`. The call never falls back to Python.

pub(crate) mod failure;
mod headers;
pub(crate) mod opener;
mod proxy_scope;
mod py_int;
pub(crate) mod request;
pub(crate) mod response;
pub(crate) mod text;
mod transport;
pub(crate) mod url;
pub(crate) mod wire;

pub mod jsonl;

pub use failure::{ErrorValue, FetchFailure, Link, PyException, ScopeRefusal, TransportFailure};
pub use proxy_scope::ProxyEnvironment;
pub use transport::UreqDialer;
pub use wire::{Dial, Wire};

/// The scope this component answers for; it is part of every JSON record.
pub const SCOPE: &str = "http-direct-plain-str-url";

/// `fetch(url)` for `http://` over a direct connection, in this process's environment.
pub fn fetch_http_direct_text(url: &str) -> Result<String, FetchFailure> {
    fetch_http_direct_text_with(url, &ProxyEnvironment::from_process(), &UreqDialer::new())
}

/// [`fetch_http_direct_text`] with the proxy environment and the connection dialer chosen by the caller.
pub fn fetch_http_direct_text_with(
    url: &str,
    proxies: &ProxyEnvironment,
    dialer: &dyn Dial,
) -> Result<String, FetchFailure> {
    decode_text(opener::fetch_body(dialer, proxies, url)?)
}

/// `r.read().decode("utf-8")`: the body as text, or the `UnicodeDecodeError` the original raises.
pub(crate) fn decode_text(body: Vec<u8>) -> Result<String, FetchFailure> {
    String::from_utf8(body).map_err(|error| {
        FetchFailure::from(failure::utf8_decode_error(
            error.as_bytes(),
            &error.utf8_error(),
        ))
    })
}
