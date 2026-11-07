//! `resolve_torch_pin.fetch(url)` for a plain-`str` **`https://` URL**, over a direct TLS connection or an HTTP `CONNECT` tunnel chosen by
//! the process environment, as native Rust.
//!
//! The original is
//!
//! ```python
//! def fetch(url: str) -> str:
//!     with urllib.request.urlopen(url, timeout=20) as r:
//!         return r.read().decode("utf-8")
//! ```
//!
//! The request/response engine is the one of [`crate::http_direct`] (`urllib.request` control flow, `http.client` parsing and exceptions); this
//! module adds the two connection layers that only `https` has:
//!
//! 1. an optional `CONNECT` tunnel through the proxy named by `https_proxy` ([`tunnel`]): the plaintext request head `CONNECT host:port
//!    HTTP/1.1` + `Host`, the status line and headers read by the `http.client` readers, and `OSError("Tunnel connection failed: ...")`;
//! 2. a real `rustls` TLS client session ([`tls_session`]) over the byte pipe, with certificate and host name verification always on and the
//!    trust taken from `SSL_CERT_FILE` / `SSL_CERT_DIR` (P's `tls_verify` verifier and `tls_trust` source selection). A refused certificate is
//!    reported as the `SSLCertVerificationError` CPython 3.12.3 raises (code, text built from the real TLS error and the real host name) inside
//!    the `URLError` that `do_open` wraps it in.
//!
//! **Scope** — named in [`SCOPE`]: plain-`str` `https` URLs; direct, or tunnelled through an `http://host:port` proxy without credentials; the
//! three certificate failures that were captured (issuer unknown, IP address mismatch, host name mismatch). Refused by name
//! ([`FetchFailure::OutOfScope`]) rather than guessed: other schemes, other proxy settings (credentials, a scheme other than `http`, a
//! `no_proxy` variable, an address that is not `host:port`), non-`str` URLs. Redirects are followed (the captured 301 to the same target, each hop a
//! new tunnel and TLS session) with the opener loop's rules. Reported as [`FetchFailure::Transport`], never
//! as a Python exception: connect/DNS/timeout failures and every TLS failure that was not captured (other certificate problems, alerts, protocol
//! errors). Not covered: the live file of `HTTPError`, `main`, TOML/requirement parsing, other TLS profiles. The call never falls back to Python.

mod exchange;
pub mod jsonl;
mod proxy_route;
mod tls_session;
mod tunnel;

use crate::http_direct::request::SurrogateCarrier;
use crate::http_direct::url::RequestUrl;
use crate::http_direct::{decode_text, opener, Dial, FetchFailure, ProxyEnvironment, UreqDialer};
use recipe_runtime::tls_trust;

/// The scope this component answers for; it is part of every JSON record.
pub const SCOPE: &str = "fetch-https-selected";

/// What the process environment decides: the proxy variables and the CA sources.
#[derive(Clone, Debug, Default)]
pub struct FetchEnvironment {
    pub proxies: ProxyEnvironment,
    pub trust: tls_trust::Environment,
}

impl FetchEnvironment {
    /// This process's environment. Names and values that are not valid Unicode are read as bytes and never panic.
    pub fn from_process() -> FetchEnvironment {
        FetchEnvironment {
            proxies: ProxyEnvironment::from_process(),
            trust: tls_trust::Environment::from_process(),
        }
    }
}

/// `fetch(url)` for `https://`, in this process's environment over real connections.
pub fn fetch_https_text(url: &str) -> Result<String, FetchFailure> {
    fetch_https_text_with(url, &FetchEnvironment::from_process(), &UreqDialer::new())
}

/// `fetch(url)` for a URL written as Python text: a command-line argument can hold lone surrogates (`surrogateescape`), which a Rust `String`
/// cannot. The surrogates of THIS text are carried explicitly ([`SurrogateCarrier`]) and read back where the request line is checked, so the
/// failure they cause is the original's (`'ascii' codec can't encode character '\udcff'`, raised before any connection). A genuine private-use
/// character of the text is never taken for a surrogate. Refused by name only when no unused stand-in character is left.
pub fn fetch_https_text_of(
    url: &agent_runtime_support::pystr::PyStr,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<String, FetchFailure> {
    let Some((carried, carrier)) = SurrogateCarrier::carry(url.code_points()) else {
        return Err(crate::http_direct::failure::out_of_scope(
            "url_text",
            "the URL holds more lone surrogates than there are unused private-use characters to carry them",
        ));
    };
    fetch_with_carrier(&carried, &carrier, environment, dialer)
}

/// [`fetch_https_text`] with the environment and the dialer of the plain TCP connections chosen by the caller.
pub fn fetch_https_text_with(
    url: &str,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<String, FetchFailure> {
    fetch_with_carrier(url, &SurrogateCarrier::none(), environment, dialer)
}

fn fetch_with_carrier(
    url: &str,
    carrier: &SurrogateCarrier,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<String, FetchFailure> {
    let request = RequestUrl::parse(url)?;
    let exchange = exchange::HttpsExchange::new(environment, dialer, carrier);
    decode_text(opener::drive(&exchange, request)?)
}
