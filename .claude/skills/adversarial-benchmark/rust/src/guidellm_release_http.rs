//! The production transport of the release lookup: an HTTPS GET with `ureq` (rustls with `ring`).
//!
//! What it takes over from `urllib.request.urlopen(request, timeout=20)`:
//!
//! * **Trust** follows OpenSSL's two independent sources, which Python's `ssl` uses: a CA *file* (`SSL_CERT_FILE`, else
//!   the platform default file) and CA *directories* (`SSL_CERT_DIR`, a `:`-separated list, else the platform default
//!   directories). An override replaces only its own side; the other side keeps its platform default. A variable that is
//!   set but empty switches its side off (it does not bring the default back). When both are set only they are
//!   trusted; when neither is, the host store is. Certificates are read from the chosen locations by
//!   `rustls-native-certs` (PEM) and handed to the verifier; certificate and host-name verification are never
//!   disabled, the process environment is never changed, and no external program is run. A CA *file* is a PEM bundle (any
//!   name); a CA *directory* is OpenSSL's hash index (only `<hash>.<n>` entries whose certificate hashes to the name are
//!   trusted - see [`crate::openssl_ca_directory`]). See [`PlatformTrust`] for what "platform default" means.
//! * **Proxy** is what `urllib` takes from the environment for an `https` URL: `https_proxy` (else `HTTPS_PROXY`; a
//!   variable that is set but empty switches the proxy off), unless `no_proxy` names the host. The proxy is asked for a
//!   `CONNECT` tunnel and the TLS session runs through it.
//! * **Timeout**: every network operation is limited to 20 seconds (per read, per write, and connection setup). The
//!   request as a whole has no deadline - `urlopen`'s timeout is a socket timeout, not a total one.
//! * **Body**: the raw response bytes, no decompression (`Accept-Encoding: identity` is sent as `urllib` does), no
//!   charset conversion and no size limit; any status outside 200-299 is an `HTTPError`. A body that ends before the
//!   length it declared is `http.client.IncompleteRead` (see [`IncompleteRead`]).
//!
//! What it cannot reproduce exactly: the text of connection and TLS errors (OpenSSL's against rustls'), the reason phrase
//! of a non-standard status (the standard one is used), the phase a timeout is assigned to, redirect-loop diagnostics
//! (at most 10 redirects are followed, then the last redirect response is an `HTTPError`), a proxy scheme other than
//! `http`, a `no_proxy` entry that applies to a host reached by a redirect, and the byte count of an `IncompleteRead` of a
//! *chunked* body (the bytes received, where `http.client` counts only the complete chunks).

use crate::guidellm::Failure;
use crate::guidellm_release::{IncompleteRead, ReleaseRequest, ReleaseTransport};
use crate::openssl_ca_directory::load_directory_roots;
use rustls_native_certs::load_certs_from_paths;
use std::ffi::OsString;
use std::io::Read;
use std::path::PathBuf;
use ureq::tls::{Certificate, RootCerts, TlsConfig, TlsProvider};
use ureq::{Agent, Error, Proxy, Timeout};

/// The `urllib` host the proxy rules are evaluated for: the releases API.
const API_HOST: &str = "api.github.com";
const CA_FILE_VARIABLE: &str = "SSL_CERT_FILE";
const CA_DIRECTORY_VARIABLE: &str = "SSL_CERT_DIR";

/// Where the host keeps its CA certificates when no variable says otherwise.
///
/// On Linux these are the locations `openssl-probe` knows for the common distributions (Debian/Ubuntu, RHEL/Fedora,
/// SUSE, Alpine, Arch, ...) - not the paths compiled into a particular OpenSSL build. A host that keeps its store
/// somewhere else has to name it with `SSL_CERT_FILE`/`SSL_CERT_DIR`. Only Linux is supported.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlatformTrust {
    pub file: Option<PathBuf>,
    pub directories: Vec<PathBuf>,
}

impl PlatformTrust {
    /// The default store of this host. Each side is discovered only when no variable overrides it, because an override
    /// replaces it, so the answer never depends on a variable that is set.
    fn discover(file_overridden: bool, directories_overridden: bool) -> PlatformTrust {
        PlatformTrust {
            file: if file_overridden {
                None
            } else {
                openssl_probe::probe().cert_file
            },
            directories: if directories_overridden {
                Vec::new()
            } else {
                openssl_probe::candidate_cert_dirs()
                    .map(PathBuf::from)
                    .collect()
            },
        }
    }
}

/// Everything of the process environment the transport reads, as values, so that a test can give it any environment
/// without touching the process's own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HttpEnvironment {
    /// `https_proxy`, else `HTTPS_PROXY`, as `urllib` reads them (empty counts as unset).
    pub https_proxy: Option<String>,
    /// `no_proxy`, else `NO_PROXY`.
    pub no_proxy: Option<String>,
    /// `SSL_CERT_FILE`: `None` when unset, `Some("")` when set to the empty string.
    pub ssl_cert_file: Option<OsString>,
    /// `SSL_CERT_DIR`: `None` when unset, `Some("")` when set to the empty string.
    pub ssl_cert_dir: Option<OsString>,
    /// The host's default store, used for the side no variable overrides.
    pub platform: PlatformTrust,
}

impl HttpEnvironment {
    /// The environment of this process and the default store of this host.
    pub fn from_process() -> HttpEnvironment {
        let ssl_cert_file = std::env::var_os(CA_FILE_VARIABLE);
        let ssl_cert_dir = std::env::var_os(CA_DIRECTORY_VARIABLE);
        HttpEnvironment {
            https_proxy: proxy_variable("https_proxy"),
            no_proxy: proxy_variable("no_proxy"),
            platform: PlatformTrust::discover(ssl_cert_file.is_some(), ssl_cert_dir.is_some()),
            ssl_cert_file,
            ssl_cert_dir,
        }
    }
}

/// The CA locations in force: the file (if any) and the directories, each chosen independently.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrustSources {
    pub file: Option<PathBuf>,
    pub directories: Vec<PathBuf>,
}

/// Chooses the two trust sources: each side takes its variable when that is set (an empty value means none) and the
/// platform default only when it is unset.
pub fn select_trust_sources(environment: &HttpEnvironment) -> TrustSources {
    let file = match &environment.ssl_cert_file {
        Some(path) if path.is_empty() => None,
        Some(path) => Some(PathBuf::from(path)),
        None => environment.platform.file.clone(),
    };
    let directories = match &environment.ssl_cert_dir {
        Some(list) => std::env::split_paths(list)
            .filter(|entry| !entry.as_os_str().is_empty())
            .collect(),
        None => environment.platform.directories.clone(),
    };
    TrustSources { file, directories }
}

/// Reads the certificates of the chosen sources. The *file* is a PEM bundle: any name, every certificate in it. Each
/// *directory* is read by OpenSSL's rule ([`crate::openssl_ca_directory`]): only `<hash>.<n>` entries whose certificate
/// hashes to the name count, so an arbitrary file name or a wrong hash is never a trust anchor. A file or directory that
/// cannot be read contributes nothing (as in OpenSSL), so a wrong override leaves the lookup untrusting rather than falling
/// back to anything else.
pub fn load_trust_roots(sources: &TrustSources) -> Vec<Certificate<'static>> {
    let mut certificates: Vec<Certificate<'static>> = Vec::new();
    if let Some(file) = sources.file.as_deref() {
        certificates.extend(
            load_certs_from_paths(Some(file), None)
                .certs
                .iter()
                .map(|der| Certificate::from_der(der.as_ref()).to_owned()),
        );
    }
    for directory in &sources.directories {
        certificates.extend(
            load_directory_roots(directory)
                .iter()
                .map(|der| Certificate::from_der(der).to_owned()),
        );
    }
    certificates
}

/// The production transport. It holds the environment it reads; each request builds its client from it.
pub struct HttpsTransport {
    environment: HttpEnvironment,
}

impl HttpsTransport {
    /// The transport of the real program: this process's environment and this host's default store.
    pub fn from_process() -> HttpsTransport {
        HttpsTransport::with_environment(HttpEnvironment::from_process())
    }

    /// A transport that reads the given environment instead of the process's.
    pub fn with_environment(environment: HttpEnvironment) -> HttpsTransport {
        HttpsTransport { environment }
    }
}

impl ReleaseTransport for HttpsTransport {
    fn open(&self, request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure> {
        let agent = build_agent(request, &self.environment)?;
        let mut call = agent.get(&request.wire_url);
        for (name, value) in &request.headers {
            call = call.header(*name, *value);
        }
        let response = call
            .header("Accept-Encoding", "identity")
            .call()
            .map_err(|error| open_failure(&error))?;
        let status = response.status();
        if !(200..300).contains(&status.as_u16()) {
            let reason = status.canonical_reason().unwrap_or("");
            return Err(Failure::new(
                "HTTPError",
                &format!("HTTP Error {}: {reason}", status.as_u16()),
            ));
        }
        let body = response.into_body();
        let declared = body.content_length();
        Ok(Box::new(ResponseBody {
            inner: body.into_reader(),
            declared,
            received: 0,
        }))
    }
}

/// The response body, which turns the end of the connection before the declared length into [`IncompleteRead`] (what
/// `http.client` raises while reading) and counts what was received.
struct ResponseBody {
    inner: ureq::BodyReader<'static>,
    declared: Option<u64>,
    received: u64,
}

impl Read for ResponseBody {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        match self.inner.read(buffer) {
            Ok(count) => {
                self.received += count as u64;
                Ok(count)
            }
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                Err(std::io::Error::other(IncompleteRead {
                    partial: self.received,
                    expected: self
                        .declared
                        .map(|total| total.saturating_sub(self.received)),
                }))
            }
            Err(error) => Err(error),
        }
    }
}

fn build_agent(request: &ReleaseRequest, environment: &HttpEnvironment) -> Result<Agent, Failure> {
    let roots = load_trust_roots(&select_trust_sources(environment));
    let tls = TlsConfig::builder()
        .provider(TlsProvider::Rustls)
        .root_certs(RootCerts::new_with_certs(&roots))
        .build();
    let limit = Some(request.timeout);
    let config = Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects_will_error(false)
        .timeout_per_read(limit)
        .timeout_per_write(limit)
        .timeout_connect(limit)
        .proxy(https_proxy(environment)?)
        .tls_config(tls)
        .build();
    Ok(Agent::new_with_config(config))
}

/// The value of a proxy variable as `urllib` sees it: the lower-case name wins, an empty value counts as unset (and
/// hides the upper-case one).
fn proxy_variable(lower_case_name: &str) -> Option<String> {
    let value =
        |name: &str| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned());
    match value(lower_case_name) {
        Some(found) => Some(found).filter(|found| !found.is_empty()),
        None => value(&lower_case_name.to_ascii_uppercase()).filter(|found| !found.is_empty()),
    }
}

/// `proxy_bypass_environment(host)` of CPython 3.12 for a host without a port.
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

fn https_proxy(environment: &HttpEnvironment) -> Result<Option<Proxy>, Failure> {
    let Some(setting) = &environment.https_proxy else {
        return Ok(None);
    };
    if environment
        .no_proxy
        .as_deref()
        .is_some_and(|no_proxy| bypasses_proxy(API_HOST, no_proxy))
    {
        return Ok(None);
    }
    Proxy::new(setting)
        .map(Some)
        .map_err(|_| Failure::new("URLError", "<urlopen error invalid proxy URL>"))
}

fn url_error(detail: &str) -> Failure {
    Failure::new("URLError", &format!("<urlopen error {detail}>"))
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

/// The exception `urlopen` would raise for a failure before the response arrived. Connection, name and TLS failures are
/// `URLError`s; a timeout while waiting for the response is a bare `TimeoutError`.
fn open_failure(error: &Error) -> Failure {
    match error {
        Error::Timeout(
            Timeout::Resolve | Timeout::Connect | Timeout::SendRequest | Timeout::PerWrite,
        ) => url_error("timed out"),
        Error::Timeout(_) => Failure::new("TimeoutError", "The read operation timed out"),
        Error::HostNotFound => url_error("[Errno -2] Name or service not known"),
        Error::Io(io) => url_error(&errno_text(io)),
        Error::Tls(detail) => url_error(&format!("TLS error: {detail}")),
        Error::Rustls(detail) => url_error(&format!("TLS error: {detail}")),
        other => url_error(&other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_proxy_entries_match_the_host_like_urllib() {
        assert!(bypasses_proxy("api.github.com", "*"));
        assert!(bypasses_proxy("api.github.com", "github.com"));
        assert!(bypasses_proxy("api.github.com", " .GitHub.com , other"));
        assert!(bypasses_proxy("api.github.com", "api.github.com"));
        assert!(!bypasses_proxy("api.github.com", "hub.com"));
        assert!(!bypasses_proxy("api.github.com", "example.org,,"));
    }
}
