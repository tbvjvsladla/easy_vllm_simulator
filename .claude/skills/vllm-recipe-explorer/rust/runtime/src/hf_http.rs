//! The one network call of the model scripts: an HTTP(S) GET of the Hugging Face model metadata (`urllib.request.urlopen(
//! Request(url, headers={"Accept": "application/json"}), timeout=...)` of the original). A small synchronous client (`ureq`
//! with rustls), no model download, no cache, no global setting: the URL is built by the caller for one call.
//!
//! What it takes from `urllib`: the `Accept` header, `Accept-Encoding: identity`, the `Python-urllib` user agent, redirects
//! (at most ten), the proxy variables for the scheme (`http_proxy`/`https_proxy`, lower case first, empty means none,
//! `no_proxy` for the host), the socket timeout with CPython's own rules for what a timeout may be (see [`socket_timeout`]), any
//! status outside 200-299 as an `HTTPError`, and the host's trusted certificate authorities for HTTPS exactly as OpenSSL
//! chooses them (see [`crate::tls_trust`]).
//!
//! The text of connection and TLS failures is the one CPython 3.12 with OpenSSL 3.0.13 prints for the same cause, for the
//! causes listed in [`tls_reason`] and for the common socket errors; the trailing `(_ssl.c:1000)` is that frozen reference's
//! source line, kept so the text equals what the original reported (it is not a position in this program). A failure with
//! another cause is reported with the client's own words and is not claimed to match.

use crate::tls_trust;
use crate::tls_verify::{client_config, TrustedConnector};
use agent_runtime_support::failure::Failure;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::Read;
use std::net::ToSocketAddrs;
use std::time::Duration;
use ureq::unversioned::resolver::DefaultResolver;
use ureq::unversioned::transport::{ConnectProxyConnector, Connector, TcpConnector};
use ureq::{Agent, Error, Proxy};

/// How a request failed, in the ways the original's handlers tell apart.
#[derive(Debug)]
pub enum HttpFailure {
    /// `urllib.error.HTTPError` with its status code.
    Status(u16),
    /// `URLError`, `TimeoutError` or another `OSError`: the text `str(error)` gives.
    Network(PyStr),
    /// A `ValueError` the original's handler turns into a parse failure (an unusable timeout, a request line that cannot be
    /// encoded).
    Value(PyStr),
    /// An exception the original's handlers do not catch (it propagates out of the lookup).
    Raised(Failure),
}

fn url_error(detail: &str) -> HttpFailure {
    HttpFailure::Network(PyStr::new(&format!("<urlopen error {detail}>")))
}

/// `[Errno N] text` of an operating-system error, as `OSError` prints it.
fn errno_text(error: &std::io::Error) -> String {
    let detail = error.to_string();
    let message = detail.split(" (os error").next().unwrap_or(&detail);
    match error.raw_os_error() {
        Some(errno) => format!("[Errno {errno}] {message}"),
        None => message.to_string(),
    }
}

/// What a socket timeout may be, as CPython's `socket.settimeout` takes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketTimeout {
    /// `None`: wait without a limit.
    Blocking,
    /// Exactly zero: a non-blocking socket, whose connect cannot complete at once.
    NonBlocking,
    /// A positive limit for every operation.
    Limit(Duration),
}

const SECONDS_LIMIT: i64 = 9_223_372_036;

/// `socket.settimeout(value)`: `None`, or a number of seconds that reaches CPython's integer nanoseconds. A float is converted
/// to nanoseconds and rounded away from zero (`1e-12` is one nanosecond, `-5e-10` is negative); a number that does not fit the
/// clock is an `OverflowError` (`inf`, `1e300`, `9.3e9`, integers beyond ±9223372036, with the two texts CPython gives for floats
/// and for integers), `NaN` and a negative value are `ValueError`s, anything else is a `TypeError`.
pub fn socket_timeout(value: &PyValue) -> Result<SocketTimeout, HttpFailure> {
    let overflow_float = || {
        HttpFailure::Raised(Failure::new(
            "OverflowError",
            "timestamp out of range for platform time_t",
        ))
    };
    let overflow_int = || {
        HttpFailure::Raised(Failure::new(
            "OverflowError",
            "timestamp too large to convert to C _PyTime_t",
        ))
    };
    let negative = || HttpFailure::Value(PyStr::new("Timeout value out of range"));
    let whole_seconds = |seconds: i64| -> Result<SocketTimeout, HttpFailure> {
        match seconds {
            0 => Ok(SocketTimeout::NonBlocking),
            1.. => Ok(SocketTimeout::Limit(Duration::from_secs(seconds as u64))),
            _ => Err(negative()),
        }
    };
    match value.peel() {
        PyValue::None => Ok(SocketTimeout::Blocking),
        PyValue::Bool(flag) => whole_seconds(i64::from(*flag)),
        PyValue::Int(number) => {
            use num_traits::ToPrimitive;
            match number.to_i64() {
                Some(seconds) if (-SECONDS_LIMIT..=SECONDS_LIMIT).contains(&seconds) => {
                    whole_seconds(seconds)
                }
                _ => Err(overflow_int()),
            }
        }
        PyValue::Float(seconds) => {
            if seconds.is_nan() {
                return Err(HttpFailure::Value(PyStr::new(
                    "Invalid value NaN (not a number)",
                )));
            }
            let nanoseconds = seconds * 1e9;
            let rounded = if nanoseconds >= 0.0 {
                nanoseconds.ceil()
            } else {
                nanoseconds.floor()
            };
            if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&rounded) {
                return Err(overflow_float());
            }
            match rounded as i64 {
                0 => Ok(SocketTimeout::NonBlocking),
                nanos if nanos < 0 => Err(negative()),
                nanos => Ok(SocketTimeout::Limit(Duration::from_nanos(nanos as u64))),
            }
        }
        other => Err(HttpFailure::Raised(Failure::new(
            "TypeError",
            &format!(
                "'{}' object cannot be interpreted as an integer",
                other.type_name()
            ),
        ))),
    }
}

/// The reason OpenSSL gives (as CPython words the `ssl.SSLCertVerificationError`) for a certificate the client refused, for the
/// causes the original's reference observations cover; any other cause is `None` and is not claimed to match.
fn tls_reason(error: &rustls::Error, host: &str) -> Option<String> {
    use rustls::{CertificateError, Error as TlsError};
    let TlsError::InvalidCertificate(cause) = error else {
        return None;
    };
    let reason = match cause {
        CertificateError::UnknownIssuer => "unable to get local issuer certificate".to_string(),
        // The chain ended at a certificate of the store that is not a trust anchor (OpenSSL error 2): observed text. A chain
        // that ended at an untrusted self-signed certificate has no observed text and is not given one.
        CertificateError::Other(detail) => {
            match detail.0.downcast_ref::<crate::tls_verify::ChainProblem>() {
                Some(crate::tls_verify::ChainProblem::UnableToGetIssuer) => {
                    "unable to get issuer certificate".to_string()
                }
                _ => return None,
            }
        }
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => {
            "certificate has expired".to_string()
        }
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            "certificate is not yet valid".to_string()
        }
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            if host.parse::<std::net::IpAddr>().is_ok() {
                format!("IP address mismatch, certificate is not valid for '{host}'. ")
            } else {
                format!("Hostname mismatch, certificate is not valid for '{host}'. ")
            }
        }
        _ => return None,
    };
    Some(format!(
        "[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: {}(_ssl.c:1000)",
        if reason.ends_with(' ') {
            reason
        } else {
            format!("{reason} ")
        }
    ))
}

/// The `rustls` error behind a client error, however it was wrapped.
fn rustls_cause(error: &Error) -> Option<&rustls::Error> {
    match error {
        Error::Rustls(inner) => Some(inner),
        Error::Io(io) => io
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
            .or_else(|| {
                io.get_ref()
                    .and_then(|inner| inner.downcast_ref::<Error>())
                    .and_then(rustls_cause)
            }),
        _ => None,
    }
}

fn failure_of(error: &Error, host: &str) -> HttpFailure {
    if let Some(reason) = rustls_cause(error).and_then(|cause| tls_reason(cause, host)) {
        return url_error(reason.trim_end());
    }
    match error {
        // Before the body, `urlopen` wraps every `OSError` (timeouts included) in a `URLError`.
        Error::Timeout(_) => url_error("timed out"),
        Error::HostNotFound => url_error("[Errno -2] Name or service not known"),
        Error::Io(io) => url_error(&errno_text(io)),
        Error::Tls(detail) => url_error(&format!("TLS error: {detail}")),
        Error::Rustls(detail) => url_error(&format!("TLS error: {detail}")),
        other => url_error(&other.to_string()),
    }
}

/// The value of a proxy variable as `urllib` sees it: the lower-case name wins, an empty value counts as unset.
fn proxy_variable(lower_case_name: &str) -> Option<String> {
    let value =
        |name: &str| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned());
    match value(lower_case_name) {
        Some(found) => Some(found).filter(|found| !found.is_empty()),
        None => value(&lower_case_name.to_ascii_uppercase()).filter(|found| !found.is_empty()),
    }
}

/// `proxy_bypass_environment(host)`: `no_proxy` names (a leading dot is ignored; a host matches itself and its subdomains).
fn bypasses_proxy(host: &str, no_proxy: &str) -> bool {
    if no_proxy == "*" {
        return true;
    }
    let host = host.to_lowercase();
    no_proxy
        .split(',')
        .map(|entry| entry.trim().trim_start_matches('.').to_lowercase())
        .filter(|name| !name.is_empty())
        .any(|name| host == name || host.ends_with(&format!(".{name}")))
}

/// `urllib.parse._splittype`: the scheme (lower-cased) is what stands before the first `:` when that stretch is not empty and
/// holds no `/`; the rest is what follows the colon. `None` is a string with no scheme at all.
fn split_type(url: &str) -> Option<(String, &str)> {
    let colon = url.find(':')?;
    let scheme = &url[..colon];
    if scheme.is_empty() || scheme.contains('/') {
        return None;
    }
    Some((scheme.to_lowercase(), &url[colon + 1..]))
}

/// The host and port of `//host[:port]/path` (what follows the scheme), for the proxy rule and the connection; `None` when
/// there is no `//` authority at all (the client's `_splithost` finds nothing). The real parsing is the client's.
fn host_port(rest: &str) -> Option<(&str, Option<u16>)> {
    let authority = rest
        .strip_prefix("//")?
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("");
    let host_port = authority.rsplit('@').next().unwrap_or("");
    let (host, port) = match host_port.rsplit_once(':') {
        Some((name, port)) if port.chars().all(|c| c.is_ascii_digit()) && !port.is_empty() => {
            (name, port.parse().ok())
        }
        _ => (host_port, None),
    };
    Some((host.trim_matches(['[', ']']), port))
}

/// `http.client`'s checks of the request path before anything is sent: control characters and spaces are `InvalidURL`
/// (not caught by the original's handlers), non-ASCII text cannot be encoded (a `UnicodeEncodeError`, a `ValueError` that the
/// original reports as a parse failure).
fn check_request_path(url: &PyStr) -> Result<(), HttpFailure> {
    let Some(text) = url.to_utf8() else {
        return Err(HttpFailure::Value(PyStr::new(
            "'utf-8' codec can't encode character: surrogates not allowed",
        )));
    };
    let after_scheme = split_type(&text).map_or(text.as_str(), |(_, rest)| rest);
    let rest = after_scheme.strip_prefix("//").unwrap_or(after_scheme);
    let path = rest.find(['/', '?', '#']).map_or("/", |at| &rest[at..]);
    if let Some(found) = path.chars().find(|c| *c <= ' ' || *c == '\u{7f}') {
        return Err(HttpFailure::Raised(Failure::new(
            "InvalidURL",
            &format!(
                "URL can't contain control characters. {} (found at least {})",
                PyStr::new(path).repr(),
                PyStr::new(&found.to_string()).repr()
            ),
        )));
    }
    // The request line `GET <path> HTTP/1.1` is encoded as ASCII: the first run of other characters is the failure.
    let line: Vec<char> = format!("GET {path} HTTP/1.1").chars().collect();
    if let Some(start) = line.iter().position(|c| !c.is_ascii()) {
        let end = line[start..]
            .iter()
            .position(|c| c.is_ascii())
            .map_or(line.len(), |len| start + len)
            - 1;
        let escape = |c: char| {
            let code = u32::from(c);
            if code < 0x100 {
                format!("\\x{code:02x}")
            } else if code < 0x10000 {
                format!("\\u{code:04x}")
            } else {
                format!("\\U{code:08x}")
            }
        };
        let subject = if start == end {
            format!("character '{}' in position {start}", escape(line[start]))
        } else {
            format!("characters in position {start}-{end}")
        };
        return Err(HttpFailure::Value(PyStr::new(&format!(
            "'ascii' codec can't encode {subject}: ordinal not in range(128)"
        ))));
    }
    Ok(())
}

/// The `ftp` scheme as far as it is ported: the host is resolved, the socket timeout applied and the connection tried, each with
/// the client's text for its failure. A connection that succeeds would need an FTP session, which is not ported and is
/// reported as such (never as a transfer that did not happen).
fn ftp_attempt(rest: &str, timeout: &PyValue) -> Result<Vec<u8>, HttpFailure> {
    let Some((host, port)) = host_port(rest).filter(|(host, _)| !host.is_empty()) else {
        return Err(url_error("ftp error: no host given"));
    };
    let port = port.unwrap_or(21);
    let address = match (host, port).to_socket_addrs() {
        Ok(mut addresses) => addresses.next(),
        Err(_) => None,
    };
    let Some(address) = address else {
        return Err(url_error("[Errno -2] Name or service not known"));
    };
    let limit = match socket_timeout(timeout)? {
        SocketTimeout::Blocking => None,
        SocketTimeout::NonBlocking => {
            return Err(url_error("[Errno 115] Operation now in progress"))
        }
        SocketTimeout::Limit(limit) => Some(limit),
    };
    let connected = match limit {
        Some(limit) => std::net::TcpStream::connect_timeout(&address, limit),
        None => std::net::TcpStream::connect(address),
    };
    match connected {
        Err(error) if error.kind() == std::io::ErrorKind::TimedOut => {
            Err(HttpFailure::Network(PyStr::new("timed out")))
        }
        Err(error) => Err(url_error(&errno_text(&error))),
        Ok(_) => Err(HttpFailure::Raised(Failure::new(
            "NotImplementedError",
            "the 'ftp' scheme handler of urllib is not ported beyond the connection",
        ))),
    }
}

/// The body of the response to `GET url` (`Accept: application/json`), after redirects.
///
/// The order is the original's: the request line is checked, the host is resolved, the socket timeout is applied (an unusable one
/// fails here, before any connection), and only then the connection is made.
pub fn get_json_bytes(url: &PyStr, timeout: &PyValue) -> Result<Vec<u8>, HttpFailure> {
    let Some(text) = url.to_utf8() else {
        // Text that is not UTF-8 cannot be a request line: the check says so.
        check_request_path(url)?;
        return Err(HttpFailure::Network(PyStr::new(
            "URL holds characters that cannot be sent",
        )));
    };
    // `Request(url)` drops the fragment and then needs a scheme (a `ValueError` the original reports as a parse failure).
    let without_fragment = text
        .split_once('#')
        .map_or(text.as_str(), |(before, _)| before);
    let Some((scheme, rest)) = split_type(without_fragment) else {
        return Err(HttpFailure::Value(PyStr::new(&format!(
            "unknown url type: {}",
            PyStr::new(without_fragment).repr()
        ))));
    };
    let scheme = scheme.as_str();
    match scheme {
        "http" | "https" => {}
        "ftp" => return ftp_attempt(rest, timeout),
        // Handlers the client has for these schemes are not ported: say so instead of answering as if they were absent.
        "file" | "data" => {
            return Err(HttpFailure::Raised(Failure::new(
                "NotImplementedError",
                &format!("the '{scheme}' scheme handler of urllib is not ported"),
            )))
        }
        _ => return Err(url_error(&format!("unknown url type: {scheme}"))),
    }
    let Some((host, port)) = host_port(rest).filter(|(host, _)| !host.is_empty()) else {
        return Err(url_error("no host given"));
    };
    // `http.client` checks the request path before it connects (and after the handler has found a host).
    check_request_path(url)?;
    let proxy_name = if scheme == "https" {
        "https_proxy"
    } else {
        "http_proxy"
    };
    let proxy = match proxy_variable(proxy_name) {
        Some(setting)
            if !proxy_variable("no_proxy")
                .is_some_and(|no_proxy| bypasses_proxy(host, &no_proxy)) =>
        {
            Some(Proxy::new(&setting).map_err(|_| url_error("invalid proxy URL"))?)
        }
        _ => None,
    };
    if proxy.is_none() {
        let default_port = if scheme == "https" { 443 } else { 80 };
        let resolved = (host, port.unwrap_or(default_port)).to_socket_addrs();
        if resolved
            .map(|mut addresses| addresses.next().is_none())
            .unwrap_or(true)
        {
            return Err(url_error("[Errno -2] Name or service not known"));
        }
    }
    let limit = match socket_timeout(timeout)? {
        SocketTimeout::Blocking => None,
        SocketTimeout::NonBlocking => {
            return Err(url_error("[Errno 115] Operation now in progress"))
        }
        SocketTimeout::Limit(limit) => Some(limit),
    };
    let builder = Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects_will_error(false)
        .timeout_per_read(limit)
        .timeout_per_write(limit)
        .timeout_connect(limit)
        .proxy(proxy);
    // Every connection of this call that really needs TLS (the first request, or a redirect from `http` to `https`) is verified by
    // the connector's own verifier (see `crate::tls_verify`) over the host's CA sources, read when that connection is made; an
    // `http`-only call never reads them.
    let connector =
        ().chain(ConnectProxyConnector::default())
            .chain(TcpConnector::default())
            .chain(TrustedConnector::new(|| {
                client_config(tls_trust::load_store(&tls_trust::select_sources(
                    &tls_trust::Environment::from_process(),
                )))
            }));
    let agent = Agent::with_parts(builder.build(), connector, DefaultResolver::default());
    let response = agent
        .get(&text)
        .header("Accept", "application/json")
        .header("Accept-Encoding", "identity")
        .header("User-Agent", "Python-urllib/3.12")
        .call()
        .map_err(|error| failure_of(&error, host))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(HttpFailure::Status(status));
    }
    let mut body = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut body)
        .map_err(|error| {
            // ureq reports its own timeout through the reader's `io::Error`.
            let timed_out = error.kind() == std::io::ErrorKind::TimedOut
                || error.kind() == std::io::ErrorKind::WouldBlock
                || error
                    .get_ref()
                    .and_then(|inner| inner.downcast_ref::<Error>())
                    .is_some_and(|inner| matches!(inner, Error::Timeout(_)));
            if timed_out {
                // A timeout while the body is read is the bare `TimeoutError` of the socket.
                return HttpFailure::Network(PyStr::new(if scheme == "https" {
                    "The read operation timed out"
                } else {
                    "timed out"
                }));
            }
            match error.kind() {
                std::io::ErrorKind::UnexpectedEof => HttpFailure::Raised(Failure::new(
                    "IncompleteRead",
                    &format!("IncompleteRead({} bytes read)", body.len()),
                )),
                _ => HttpFailure::Network(PyStr::new(&errno_text(&error))),
            }
        })?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;

    #[test]
    fn no_proxy_entries_match_the_host_like_urllib() {
        assert!(bypasses_proxy("huggingface.co", "*"));
        assert!(bypasses_proxy("huggingface.co", "huggingface.co"));
        assert!(bypasses_proxy(
            "api.huggingface.co",
            " .HuggingFace.co , other"
        ));
        assert!(!bypasses_proxy("huggingface.co", "face.co"));
    }

    #[test]
    fn the_scheme_host_and_port_of_a_url_are_found_as_the_client_finds_them() {
        let found = |url: &str| {
            split_type(url).map(|(scheme, rest)| {
                (
                    scheme,
                    host_port(rest).map(|(host, port)| (host.to_string(), port)),
                )
            })
        };
        assert_eq!(
            found("http://127.0.0.1:30067/ok"),
            Some((
                "http".to_string(),
                Some(("127.0.0.1".to_string(), Some(30067)))
            ))
        );
        assert_eq!(
            found("HTTPS://huggingface.co/api/models/x"),
            Some((
                "https".to_string(),
                Some(("huggingface.co".to_string(), None))
            ))
        );
        assert_eq!(
            found("http://[::1]:8/x"),
            Some(("http".to_string(), Some(("::1".to_string(), Some(8)))))
        );
        assert_eq!(found("http:foo"), Some(("http".to_string(), None)));
        assert_eq!(found("foo:bar"), Some(("foo".to_string(), None)));
        assert_eq!(found("h/x"), None);
        assert_eq!(found("//h/x"), None);
        assert_eq!(found(":x"), None);
    }

    #[test]
    fn a_socket_timeout_is_what_cpython_accepts_and_refuses() {
        let float = |value: f64| socket_timeout(&PyValue::Float(value));
        let int = |value: &str| {
            socket_timeout(&PyValue::Int(value.parse::<BigInt>().expect("an integer")))
        };
        let class = |outcome: Result<SocketTimeout, HttpFailure>| match outcome {
            Err(HttpFailure::Value(message)) => {
                ("ValueError".to_string(), message.to_utf8().expect("text"))
            }
            Err(HttpFailure::Raised(failure)) => (
                failure.class.to_string(),
                failure.message.to_utf8().expect("text"),
            ),
            other => panic!("{other:?}"),
        };
        assert_eq!(
            socket_timeout(&PyValue::None).unwrap(),
            SocketTimeout::Blocking
        );
        assert_eq!(float(0.0).unwrap(), SocketTimeout::NonBlocking);
        assert_eq!(float(-0.0).unwrap(), SocketTimeout::NonBlocking);
        assert_eq!(
            float(1e-12).unwrap(),
            SocketTimeout::Limit(Duration::from_nanos(1))
        );
        assert_eq!(
            float(0.3).unwrap(),
            SocketTimeout::Limit(Duration::from_millis(300))
        );
        assert_eq!(
            float(9_223_372_036.0).unwrap(),
            SocketTimeout::Limit(Duration::from_secs(9_223_372_036))
        );
        assert_eq!(
            socket_timeout(&PyValue::Bool(true)).unwrap(),
            SocketTimeout::Limit(Duration::from_secs(1))
        );
        assert_eq!(
            class(float(-1.0)),
            ("ValueError".into(), "Timeout value out of range".into())
        );
        assert_eq!(
            class(float(-5e-10)),
            ("ValueError".into(), "Timeout value out of range".into())
        );
        assert_eq!(
            class(float(f64::NAN)),
            (
                "ValueError".into(),
                "Invalid value NaN (not a number)".into()
            )
        );
        for value in [
            f64::INFINITY,
            f64::NEG_INFINITY,
            1e300,
            9_223_372_036.9,
            -1e300,
        ] {
            assert_eq!(
                class(float(value)),
                (
                    "OverflowError".into(),
                    "timestamp out of range for platform time_t".into()
                ),
                "{value}"
            );
        }
        assert_eq!(
            int("8589934592").unwrap(),
            SocketTimeout::Limit(Duration::from_secs(8_589_934_592))
        );
        assert_eq!(
            class(int("-9223372036")),
            ("ValueError".into(), "Timeout value out of range".into())
        );
        for value in [
            "9223372037",
            "-9223372037",
            "17179869184",
            "9223372036854775808",
            "-18446744073709551616",
        ] {
            assert_eq!(
                class(int(value)),
                (
                    "OverflowError".into(),
                    "timestamp too large to convert to C _PyTime_t".into()
                ),
                "{value}"
            );
        }
        assert_eq!(
            class(socket_timeout(&PyValue::str("x"))),
            (
                "TypeError".into(),
                "'str' object cannot be interpreted as an integer".into()
            )
        );
    }
}
