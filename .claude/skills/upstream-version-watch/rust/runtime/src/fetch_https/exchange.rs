//! One `https` request over one connection: `AbstractHTTPHandler.do_open` of `urllib.request` with `HTTPSConnection`.
//!
//! The order is the original's: validate the target, choose the route from the environment, connect (the proxy for a tunnel, else the host),
//! open the tunnel, run the TLS handshake for the logical host, send the `GET` head (the same head `http` sends: `Host` is the URL's host as
//! written, never the proxy), read the status line and headers. Failures that are `OSError`s in the original and happen while the request
//! is being sent (a refused tunnel, a closed tunnel reply, a refused certificate) are wrapped in `URLError` as `do_open` does; the others
//! (`BadStatusLine`, header limits) pass through. Before any connection is made the request line is checked as `putrequest` checks it
//! (control characters, ASCII encoding). A redirect is followed by the shared opener loop exactly as over plain TCP: `HTTPSHandler` and
//! `HTTPHandler` share `AbstractHTTPHandler.do_open`, and each hop is one new `Connection: close` exchange (tunnel, TLS, `GET`).

use super::proxy_route::{self, Route};
use super::{tls_session, tunnel, FetchEnvironment};
use crate::http_direct::failure::{out_of_scope, url_error, FetchFailure, Fetched};
use crate::http_direct::opener::Exchange;
use crate::http_direct::request::{
    check_request_line, get_request_bytes_with, RequestHeaders, SurrogateCarrier,
};
use crate::http_direct::response::HttpResponse;
use crate::http_direct::url::{HttpTarget, RequestUrl, HTTPS_DEFAULT_PORT};
use crate::http_direct::wire::{BufferedReader, Dial, Wire};

pub struct HttpsExchange<'a> {
    environment: &'a FetchEnvironment,
    dialer: &'a dyn Dial,
    /// Which stand-in characters of the URL text are lone surrogates of a Python text (empty for genuine Unicode text).
    carrier: &'a SurrogateCarrier,
    /// The headers of the `Request` (the default `urlopen(url)` headers when empty).
    headers: &'a RequestHeaders,
}

impl<'a> HttpsExchange<'a> {
    pub fn new(
        environment: &'a FetchEnvironment,
        dialer: &'a dyn Dial,
        carrier: &'a SurrogateCarrier,
        headers: &'a RequestHeaders,
    ) -> HttpsExchange<'a> {
        HttpsExchange {
            environment,
            dialer,
            carrier,
            headers,
        }
    }

    /// The encrypted connection to `target`, directly or through the route's proxy.
    fn connect(&self, target: &HttpTarget, route: &Route) -> Fetched<Box<dyn Wire>> {
        let plain = match route {
            Route::Direct => self.dialer.dial(&target.connect_host, target.port)?,
            Route::Tunnel(proxy) => {
                let to_proxy = self.dialer.dial(&proxy.host, proxy.port)?;
                tunnel::establish(to_proxy, &target.connect_host, target.port)
                    .map_err(wrapped_by_do_open)?
            }
        };
        tls_session::start(plain, &target.connect_host, &self.environment.trust)
            .map_err(wrapped_by_do_open)
    }
}

/// `except OSError as err: raise URLError(err)` of `do_open`.
fn wrapped_by_do_open(failure: FetchFailure) -> FetchFailure {
    match failure {
        FetchFailure::Raised(error) if error.is_os_error() => url_error(*error).into(),
        other => other,
    }
}

impl Exchange for HttpsExchange<'_> {
    fn open(&self, request: &RequestUrl) -> Fetched<HttpResponse> {
        if request.scheme != "https" {
            return Err(out_of_scope(
                "scheme",
                format!(
                    "scheme '{}' is not https; this component handles https URLs only",
                    request.scheme
                ),
            ));
        }
        // `putrequest` validates the request line before any connection exists, ahead of every other check of the target
        check_request_line(&request.selector, self.carrier)
            .map_err(|error| FetchFailure::from(*error))?;
        let target = HttpTarget::with_default_port(request, HTTPS_DEFAULT_PORT)?;
        let route = proxy_route::choose(&self.environment.proxies)?;
        let wire = self.connect(&target, &route)?;
        let mut reader = BufferedReader::new(wire);
        reader.send_all(&get_request_bytes_with(&target, self.headers))?;
        HttpResponse::begin(reader)
    }

    fn follows_redirects(&self) -> bool {
        true
    }
}
