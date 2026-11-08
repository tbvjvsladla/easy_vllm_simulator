//! The URL handling of `urllib.request.Request` and of the redirect handler, ported from CPython 3.12 `urllib/parse.py` and
//! `urllib/request.py` for the plain-str, HTTP-direct scope.
//!
//! Forms whose Python behaviour depends on code this port does not carry (IPv6 bracket validation, NFKC normalisation of a
//! non-ASCII network location, percent-decoding of the host, user information, characters `http.client` rejects) are refused as
//! out of scope, never answered with a guess.

use super::failure::{out_of_scope, value_error, Fetched, PyException};
use super::py_int::{parse_int, IntOperand};
use super::text::{is_space, strip};
use agent_runtime_support::pystr::PyStr;

const USES_PARAMS: [&str; 4] = ["", "ftp", "http", "https"];
const USES_NETLOC: [&str; 4] = ["", "ftp", "http", "https"];
const USES_RELATIVE: [&str; 4] = ["", "ftp", "http", "https"];

/// `HTTPConnection.default_port` and `HTTPSConnection.default_port`.
pub const HTTP_DEFAULT_PORT: u16 = 80;
pub const HTTPS_DEFAULT_PORT: u16 = 443;

/// `urllib.parse.ParseResult` fields.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedUrl {
    pub scheme: String,
    pub netloc: String,
    pub path: String,
    pub params: String,
    pub query: String,
    pub fragment: String,
}

fn is_c0_control_or_space(character: char) -> bool {
    character <= ' '
}

/// `urlsplit` followed by the parameter split of `urlparse`.
pub fn urlparse(url: &str, default_scheme: &str) -> Fetched<ParsedUrl> {
    let mut rest: String = url
        .trim_start_matches(is_c0_control_or_space)
        .chars()
        .filter(|character| !matches!(character, '\t' | '\r' | '\n'))
        .collect();
    let mut scheme = default_scheme
        .trim_matches(is_c0_control_or_space)
        .replace(['\t', '\r', '\n'], "");
    if let Some(colon) = rest.find(':') {
        let candidate = &rest[..colon];
        let first_is_letter = candidate
            .chars()
            .next()
            .is_some_and(|first| first.is_ascii_alphabetic());
        if colon > 0
            && first_is_letter
            && candidate
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
        {
            scheme = candidate.to_ascii_lowercase();
            rest = rest[colon + 1..].to_string();
        }
    }
    let mut netloc = String::new();
    if let Some(after_slashes) = rest.strip_prefix("//") {
        let end = after_slashes
            .find(['/', '?', '#'])
            .unwrap_or(after_slashes.len());
        netloc = after_slashes[..end].to_string();
        let tail = after_slashes[end..].to_string();
        if netloc.contains(['[', ']']) {
            return Err(out_of_scope(
                "url_form",
                "a bracketed network location needs the IPv6 validation of urllib.parse, which is not ported",
            ));
        }
        if !netloc.is_ascii() {
            return Err(out_of_scope(
                "url_form",
                "a non-ASCII network location needs the NFKC check of urllib.parse, which is not ported",
            ));
        }
        rest = tail;
    }
    let mut fragment = String::new();
    if let Some((before, after)) = rest.split_once('#') {
        fragment = after.to_string();
        rest = before.to_string();
    }
    let mut query = String::new();
    if let Some((before, after)) = rest.split_once('?') {
        query = after.to_string();
        rest = before.to_string();
    }
    let mut params = String::new();
    if USES_PARAMS.contains(&scheme.as_str()) && rest.contains(';') {
        let search_from = rest.rfind('/').unwrap_or(0);
        let semicolon = if rest.contains('/') {
            rest[search_from..].find(';').map(|at| at + search_from)
        } else {
            rest.find(';')
        };
        if let Some(at) = semicolon {
            params = rest[at + 1..].to_string();
            rest.truncate(at);
        }
    }
    Ok(ParsedUrl {
        scheme,
        netloc,
        path: rest,
        params,
        query,
        fragment,
    })
}

/// `urlunparse`.
pub fn urlunparse(parts: &ParsedUrl) -> String {
    let mut path = parts.path.clone();
    if !parts.params.is_empty() {
        path = format!("{path};{}", parts.params);
    }
    if !parts.netloc.is_empty()
        || (!parts.scheme.is_empty()
            && USES_NETLOC.contains(&parts.scheme.as_str())
            && !path.starts_with("//"))
    {
        if !path.is_empty() && !path.starts_with('/') {
            path.insert(0, '/');
        }
        path = format!("//{}{path}", parts.netloc);
    }
    let mut url = path;
    if !parts.scheme.is_empty() {
        url = format!("{}:{url}", parts.scheme);
    }
    if !parts.query.is_empty() {
        url = format!("{url}?{}", parts.query);
    }
    if !parts.fragment.is_empty() {
        url = format!("{url}#{}", parts.fragment);
    }
    url
}

/// `urljoin(base, url)`.
pub fn urljoin(base: &str, url: &str) -> Fetched<String> {
    if base.is_empty() {
        return Ok(url.to_string());
    }
    if url.is_empty() {
        return Ok(base.to_string());
    }
    let base_parts = urlparse(base, "")?;
    let mut target = urlparse(url, &base_parts.scheme)?;
    if target.scheme != base_parts.scheme || !USES_RELATIVE.contains(&target.scheme.as_str()) {
        return Ok(url.to_string());
    }
    if USES_NETLOC.contains(&target.scheme.as_str()) {
        if !target.netloc.is_empty() {
            return Ok(urlunparse(&target));
        }
        target.netloc = base_parts.netloc.clone();
    }
    if target.path.is_empty() && target.params.is_empty() {
        target.path = base_parts.path.clone();
        target.params = base_parts.params.clone();
        if target.query.is_empty() {
            target.query = base_parts.query.clone();
        }
        return Ok(urlunparse(&target));
    }
    let mut base_segments: Vec<&str> = base_parts.path.split('/').collect();
    if base_segments.last() != Some(&"") {
        base_segments.pop();
    }
    let mut segments: Vec<&str> = if target.path.starts_with('/') {
        target.path.split('/').collect()
    } else {
        let mut joined = base_segments;
        joined.extend(target.path.split('/'));
        // `segments[1:-1] = filter(None, segments[1:-1])`
        if joined.len() > 2 {
            let last = joined.len() - 1;
            let middle: Vec<&str> = joined[1..last]
                .iter()
                .copied()
                .filter(|segment| !segment.is_empty())
                .collect();
            let mut rebuilt = vec![joined[0]];
            rebuilt.extend(middle);
            rebuilt.push(joined[last]);
            joined = rebuilt;
        }
        joined
    };
    let mut resolved: Vec<&str> = Vec::new();
    for segment in &segments {
        match *segment {
            ".." => {
                resolved.pop();
            }
            "." => {}
            other => resolved.push(other),
        }
    }
    if matches!(segments.last(), Some(&".") | Some(&"..")) {
        resolved.push("");
    }
    segments.clear();
    let joined_path = resolved.join("/");
    target.path = if joined_path.is_empty() {
        "/".to_string()
    } else {
        joined_path
    };
    Ok(urlunparse(&target))
}

/// `quote(url, encoding="iso-8859-1", safe=string.punctuation)`: only controls, space and non-ASCII bytes are escaped.
pub fn quote_latin1_keeping_punctuation(url: &str) -> Fetched<String> {
    let mut quoted = String::with_capacity(url.len());
    for character in url.chars() {
        let point = u32::from(character);
        if point > 0xff {
            return Err(out_of_scope(
                "url_form",
                "a redirect target above U+00FF cannot be encoded as ISO-8859-1 (UnicodeEncodeError is not modelled)",
            ));
        }
        if character.is_ascii_graphic() {
            quoted.push(character);
        } else {
            quoted.push_str(&format!("%{point:02X}"));
        }
    }
    Ok(quoted)
}

/// `urllib.request.Request(url)`: the part of the constructor the fetch depends on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestUrl {
    /// `_full_url` (without the fragment).
    without_fragment: String,
    fragment: Option<String>,
    /// `Request.type`, lower-case.
    pub scheme: String,
    /// `Request.host` (before `unquote`; a `%` is out of scope).
    pub host: String,
    /// `Request.selector`.
    pub selector: String,
}

impl RequestUrl {
    /// `Request.__init__` with the unmodified original's argument: unwrap, split the fragment, the scheme and the host.
    pub fn parse(url: &str) -> Fetched<RequestUrl> {
        let unwrapped = unwrap(url);
        let (without_fragment, fragment) = match unwrapped.rsplit_once('#') {
            Some((before, tag)) => (before.to_string(), Some(tag.to_string())),
            None => (unwrapped, None),
        };
        let full_url = full_url_of(&without_fragment, fragment.as_deref());
        let Some((scheme, rest)) = split_type(&without_fragment) else {
            return Err(unknown_url_type(&full_url).into());
        };
        let (host, selector) = split_host(rest);
        if host.contains('%') {
            return Err(out_of_scope(
                "url_form",
                "a percent-encoded host would be unquoted by urllib; not ported",
            ));
        }
        Ok(RequestUrl {
            scheme,
            host,
            selector,
            without_fragment,
            fragment,
        })
    }

    /// `Request.full_url`.
    pub fn full_url(&self) -> String {
        full_url_of(&self.without_fragment, self.fragment.as_deref())
    }
}

fn full_url_of(without_fragment: &str, fragment: Option<&str>) -> String {
    match fragment {
        Some(tag) if !tag.is_empty() => format!("{without_fragment}#{tag}"),
        _ => without_fragment.to_string(),
    }
}

fn unknown_url_type(full_url: &str) -> PyException {
    value_error(&format!(
        "unknown url type: {}",
        PyStr::new(full_url).repr()
    ))
}

/// `urllib.request.unwrap`.
fn unwrap(url: &str) -> String {
    let mut text = strip(url).to_string();
    if text.starts_with('<') && text.ends_with('>') {
        text = strip(&text[1..text.len() - 1]).to_string();
    }
    if let Some(after) = text.strip_prefix("URL:") {
        text = strip(after).to_string();
    }
    text
}

/// `urllib.parse._splittype`: `([^/:]+):(.*)`.
fn split_type(url: &str) -> Option<(String, &str)> {
    let colon = url.find([':', '/'])?;
    if colon == 0 || !url[colon..].starts_with(':') {
        return None;
    }
    Some((url[..colon].to_ascii_lowercase(), &url[colon + 1..]))
}

/// `urllib.parse._splithost`: `//([^/#?]*)(.*)`.
fn split_host(rest: &str) -> (String, String) {
    let Some(after) = rest.strip_prefix("//") else {
        return (String::new(), rest.to_string());
    };
    let end = after.find(['/', '#', '?']).unwrap_or(after.len());
    let mut path = after[end..].to_string();
    if !path.is_empty() && !path.starts_with('/') {
        path.insert(0, '/');
    }
    (after[..end].to_string(), path)
}

/// Where the connection goes and what the request line says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpTarget {
    /// The `Host` header value (`Request.host`).
    pub host_header: String,
    /// The host name or address handed to the resolver.
    pub connect_host: String,
    pub port: u16,
    /// The request target of the request line (`/` when empty).
    pub selector: String,
}

impl HttpTarget {
    /// `HTTPConnection._get_hostport`, `_validate_host`, `_validate_path` for the plain case; anything those functions would reject or that
    /// this port does not carry is out of scope.
    pub fn of(request: &RequestUrl) -> Fetched<HttpTarget> {
        HttpTarget::with_default_port(request, HTTP_DEFAULT_PORT)
    }

    /// [`HttpTarget::of`] for a connection class whose `default_port` is `default_port` (an empty or missing port in the host).
    pub fn with_default_port(request: &RequestUrl, default_port: u16) -> Fetched<HttpTarget> {
        if request.host.is_empty() {
            return Err(out_of_scope(
                "url_form",
                "an empty host raises URLError('no host given') in urllib; its exception shape was not captured",
            ));
        }
        if request.host.contains('@') {
            return Err(out_of_scope(
                "url_form",
                "user information in the network location is not modelled",
            ));
        }
        if request.host.contains(['[', ']']) {
            return Err(out_of_scope(
                "url_form",
                "an IPv6 literal host is not part of the direct-HTTP scope",
            ));
        }
        let host_port = request.host.as_str();
        let colon = host_port.rfind(':');
        let (host, port) = match colon {
            Some(at) => {
                let digits = &host_port[at + 1..];
                let port = if digits.is_empty() {
                    default_port
                } else {
                    let number = parse_int(IntOperand::Text(digits), 10).map_err(|_| {
                        out_of_scope(
                            "url_form",
                            "a non-numeric port raises InvalidURL; not captured",
                        )
                    })?;
                    u16::try_from(number).map_err(|_| {
                        out_of_scope(
                            "url_form",
                            "a port outside 0..=65535 raises OverflowError; not captured",
                        )
                    })?
                };
                (&host_port[..at], port)
            }
            None => (host_port, default_port),
        };
        let disallowed = |text: &str| text.chars().any(|c| c <= ' ' || c == '\u{7f}');
        if host.is_empty() || disallowed(host) || !host.is_ascii() {
            return Err(out_of_scope(
                "url_form",
                "a host with control, space or non-ASCII characters raises InvalidURL or an IDNA error; not captured",
            ));
        }
        let selector = if request.selector.is_empty() {
            "/".to_string()
        } else {
            request.selector.clone()
        };
        if disallowed(&selector) || !selector.is_ascii() || is_space_only(&selector) {
            return Err(out_of_scope(
                "url_form",
                "a request target with control, space or non-ASCII characters raises InvalidURL or UnicodeEncodeError; not captured",
            ));
        }
        Ok(HttpTarget {
            host_header: request.host.clone(),
            connect_host: host.to_string(),
            port,
            selector,
        })
    }
}

fn is_space_only(text: &str) -> bool {
    text.chars().all(is_space)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_url_splits_like_urllib_request() {
        let request = RequestUrl::parse("  http://127.0.0.1:8080/x?y=1#frag ").unwrap();
        assert_eq!(request.scheme, "http");
        assert_eq!(request.host, "127.0.0.1:8080");
        assert_eq!(request.selector, "/x?y=1");
        assert_eq!(request.full_url(), "http://127.0.0.1:8080/x?y=1#frag");
        let bare = RequestUrl::parse("http://h?q").unwrap();
        assert_eq!(bare.selector, "/?q");
        let target = HttpTarget::of(&RequestUrl::parse("http://h:/x").unwrap()).unwrap();
        assert_eq!((target.port, target.connect_host.as_str()), (80, "h"));
    }

    #[test]
    fn a_url_without_a_scheme_is_the_value_error_of_request() {
        let Err(super::super::failure::FetchFailure::Raised(error)) = RequestUrl::parse("x/y")
        else {
            panic!("expected the ValueError");
        };
        assert_eq!(error.class, "ValueError");
        assert_eq!(error.message.to_utf8().unwrap(), "unknown url type: 'x/y'");
    }

    #[test]
    fn urljoin_resolves_the_cases_redirects_use() {
        let join = |base: &str, url: &str| urljoin(base, url).unwrap();
        assert_eq!(join("http://h:1/x", "/next"), "http://h:1/next");
        assert_eq!(join("http://h:1/a/b", "c"), "http://h:1/a/c");
        assert_eq!(join("http://h:1/a/b", "../c"), "http://h:1/c");
        assert_eq!(join("http://h:1/a/b?q#f", ""), "http://h:1/a/b?q#f");
        assert_eq!(join("http://h:1/a", "https://o/z"), "https://o/z");
        assert_eq!(join("http://h:1/a", "?k=v"), "http://h:1/a?k=v");
        assert_eq!(join("http://h:1/a/", "./"), "http://h:1/a/");
    }

    #[test]
    fn urlparse_and_unparse_round_trip_the_redirect_fix_up() {
        let parsed = urlparse("http://h:1", "").unwrap();
        assert_eq!(parsed.netloc, "h:1");
        assert_eq!(parsed.path, "");
        assert_eq!(
            urlunparse(&ParsedUrl {
                path: "/".to_string(),
                ..parsed
            }),
            "http://h:1/"
        );
        assert_eq!(urlparse("/n;p?q#f", "").unwrap().params, "p");
        assert_eq!(urlparse("HTTP:x", "").unwrap().scheme, "http");
    }

    #[test]
    fn quote_escapes_only_controls_space_and_high_bytes() {
        assert_eq!(
            quote_latin1_keeping_punctuation("/a b%41\u{e9}").unwrap(),
            "/a%20b%41%E9"
        );
    }

    #[test]
    fn forms_outside_the_ported_subset_are_refused_by_name() {
        for url in [
            "http://[::1]/x",
            "http://h\u{e9}/x",
            "http:///x",
            "http://u@h/x",
            "http://h:99999/x",
            "http://h/a b",
        ] {
            let outcome = RequestUrl::parse(url).and_then(|request| HttpTarget::of(&request));
            assert!(
                matches!(
                    outcome,
                    Err(super::super::failure::FetchFailure::OutOfScope(_))
                ),
                "{url}: {outcome:?}"
            );
        }
    }
}
