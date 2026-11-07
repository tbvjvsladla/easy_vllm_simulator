//! The `urllib.request` control flow behind `urlopen(url, timeout=20).read()` for an `http` URL and a direct connection: one connection
//! per request (`Connection: close`), the `HTTPErrorProcessor` (anything outside 2xx goes to the error handlers), the
//! `HTTPRedirectHandler` for 301/302/303/307/308 and the `HTTPDefaultErrorHandler` that turns every other status into `HTTPError`.
//!
//! Ported from CPython 3.12.3 `Lib/urllib/request.py` (`OpenerDirector.open/_open/error`, `AbstractHTTPHandler.do_open`,
//! `HTTPErrorProcessor.http_response`, `HTTPRedirectHandler.http_error_302/redirect_request`, `HTTPDefaultErrorHandler`).
//! Exceptions raised by `http.client` while the response is read are not wrapped (`do_open` only wraps connect failures, which here are
//! [`TransportFailure`]s). The redirect handler's shared `redirect_dict` is an insertion-ordered visit count.
//!
//! The connection layer is the [`Exchange`] the caller supplies (plain TCP here, TLS in `crate::fetch_https`); everything above it, the
//! status handling and the redirect rules, is this module's single implementation.

use super::failure::{http_error, out_of_scope, FetchFailure, Fetched};
use super::proxy_scope::ProxyEnvironment;
use super::request::{
    check_request_line, get_request_bytes_with, RequestHeaders, SurrogateCarrier,
};
use super::response::HttpResponse;
use super::url::{
    quote_latin1_keeping_punctuation, urljoin, urlparse, urlunparse, HttpTarget, RequestUrl,
};
use super::wire::{BufferedReader, Dial};

/// `HTTPRedirectHandler.max_repeats`.
const MAX_REPEATS: u32 = 4;
/// `HTTPRedirectHandler.max_redirections`.
const MAX_REDIRECTIONS: usize = 10;
/// `HTTPRedirectHandler.inf_msg`.
const INFINITE_LOOP_MESSAGE: &str = "The HTTP server returned a redirect error that would lead to an infinite loop.\nThe last 30x error message was:\n";
const REDIRECT_CODES: [u16; 5] = [301, 302, 303, 307, 308];

/// `redirect_dict`: the URLs this chain of redirects visited and how often, in the order first visited.
type Visited = Vec<(String, u32)>;

/// One request over one connection: connect, send the `GET` head, read the status line and headers.
pub trait Exchange {
    fn open(&self, request: &RequestUrl) -> Fetched<HttpResponse>;

    /// Whether a 30x response with a `Location` may be followed by this exchange. When it may not, such a response is refused by name
    /// instead of being followed over a connection kind that was never captured.
    fn follows_redirects(&self) -> bool;
}

/// `urlopen(url, timeout=20).read()` over plain TCP: the body of the final 2xx response.
pub fn fetch_body(dial: &dyn Dial, proxies: &ProxyEnvironment, url: &str) -> Fetched<Vec<u8>> {
    fetch_body_with_headers(dial, proxies, url, &RequestHeaders::default())
}

/// [`fetch_body`] for a `Request` the caller built with `headers`.
pub fn fetch_body_with_headers(
    dial: &dyn Dial,
    proxies: &ProxyEnvironment,
    url: &str,
    headers: &RequestHeaders,
) -> Fetched<Vec<u8>> {
    let request = RequestUrl::parse(url)?;
    if proxies.has_http_proxy() {
        return Err(out_of_scope(
            "proxy_environment",
            "an http proxy variable is set; urllib would route through it and this component only connects directly",
        ));
    }
    drive(
        &PlainHttp {
            dial,
            headers,
            carrier: None,
        },
        request,
    )
}

/// The opener loop for `request` over `exchange`: the body of the final 2xx response.
pub fn drive(exchange: &dyn Exchange, mut request: RequestUrl) -> Fetched<Vec<u8>> {
    let mut visited: Option<Visited> = None;
    loop {
        let mut response = exchange.open(&request)?;
        if (200..300).contains(&response.status) {
            return response.read_body();
        }
        if REDIRECT_CODES.contains(&response.status) {
            if let Some(next) = follow_redirect(exchange, &request, &mut response, &mut visited)? {
                request = next;
                continue;
            }
        }
        return Err(http_error(
            &request.full_url(),
            response.status,
            &response.reason,
            response.headers.raw_items(),
        )
        .into());
    }
}

/// `AbstractHTTPHandler.do_open` for a plain `http` request over a direct connection.
pub(crate) struct PlainHttp<'a> {
    pub(crate) dial: &'a dyn Dial,
    pub(crate) headers: &'a RequestHeaders,
    /// When present, the request line is checked as `putrequest` checks it before any connection exists (the HTTPS exchange always does).
    pub(crate) carrier: Option<&'a SurrogateCarrier>,
}

impl Exchange for PlainHttp<'_> {
    fn open(&self, request: &RequestUrl) -> Fetched<HttpResponse> {
        if request.scheme != "http" {
            return Err(out_of_scope(
                "scheme",
                format!(
                    "scheme '{}' is handled by other urllib handlers (TLS, FTP, file, data); only http is in this component's scope",
                    request.scheme
                ),
            ));
        }
        if let Some(carrier) = self.carrier {
            check_request_line(&request.selector, carrier)
                .map_err(|error| FetchFailure::from(*error))?;
        }
        let target = HttpTarget::of(request)?;
        let wire = self.dial.dial(&target.connect_host, target.port)?;
        let mut reader = BufferedReader::new(wire);
        reader.send_all(&get_request_bytes_with(&target, self.headers))?;
        HttpResponse::begin(reader)
    }

    fn follows_redirects(&self) -> bool {
        true
    }
}

/// `HTTPRedirectHandler.http_error_302` for a `GET`: the request to follow, `None` when the response carries no `Location`/`URI` (the
/// default error handler then raises `HTTPError`). Its own `HTTPError`s are returned as failures.
fn follow_redirect(
    exchange: &dyn Exchange,
    request: &RequestUrl,
    response: &mut HttpResponse,
    visited: &mut Option<Visited>,
) -> Fetched<Option<RequestUrl>> {
    let location = if response.headers.contains("location") {
        response.headers.get("location")
    } else if response.headers.contains("uri") {
        response.headers.get("uri")
    } else {
        None
    };
    let Some(location) = location else {
        return Ok(None);
    };
    if !exchange.follows_redirects() {
        return Err(out_of_scope(
            "redirect",
            "the response redirects, and following a redirect over this connection kind was not captured against the original",
        ));
    }
    let location = location.to_string();
    let mut parts = urlparse(&location, "")?;
    if !["http", "https", "ftp", ""].contains(&parts.scheme.as_str()) {
        return Err(http_error(
            &location,
            response.status,
            &format!(
                "{} - Redirection to url '{location}' is not allowed",
                response.reason
            ),
            response.headers.raw_items(),
        )
        .into());
    }
    if parts.path.is_empty() && !parts.netloc.is_empty() {
        parts.path = "/".to_string();
    }
    let quoted = quote_latin1_keeping_punctuation(&urlunparse(&parts))?;
    let new_url = urljoin(&request.full_url(), &quoted)?;
    // `redirect_request` re-encodes spaces for the new Request only; the visit key keeps the joined text.
    let next = RequestUrl::parse(&new_url.replace(' ', "%20"))?;
    match visited {
        Some(seen) => {
            let repeats = seen
                .iter()
                .find(|(url, _)| *url == new_url)
                .map_or(0, |(_, count)| *count);
            if repeats >= MAX_REPEATS || seen.len() >= MAX_REDIRECTIONS {
                return Err(http_error(
                    &request.full_url(),
                    response.status,
                    &format!("{INFINITE_LOOP_MESSAGE}{}", response.reason),
                    response.headers.raw_items(),
                )
                .into());
            }
        }
        None => *visited = Some(Vec::new()),
    }
    let seen = visited.as_mut().expect("the visit record exists now");
    match seen.iter_mut().find(|(url, _)| *url == new_url) {
        Some((_, count)) => *count += 1,
        None => seen.push((new_url, 1)),
    }
    // `fp.read(); fp.close()` before the next request: a framing error of this response is raised here.
    response.read_body()?;
    Ok(Some(next))
}
