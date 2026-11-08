//! The one HTTP exchange of `functional_smoke._post_chat`: what `urllib.request.urlopen` does for a `POST` of a JSON body over `http://`,
//! with the exceptions `urllib` and `http.client` raise (class, module and `str()` of each) so that the caller can report them as Python does.
//!
//! The request is built the way `urllib.request.Request` and `AbstractHTTPHandler.do_open` build it: the URL is unwrapped and split
//! (`unwrap`, `_splittag`, `_splittype`, `_splithost`), the handler adds `Content-length`, `Host` and `User-agent`, `do_open` adds
//! `Connection: close` and title-cases the names, and `http.client` writes `Accept-Encoding: identity` first; the request line and
//! the headers are written, then the body, as two sends. The response is read as `http.client.HTTPResponse` reads it: the status line,
//! the headers, then the body by `Content-Length`, by chunks, or to the end of the stream; every failure of that reading is the
//! exception `http.client` raises (`RemoteDisconnected`, `BadStatusLine`, `IncompleteRead`, `LineTooLong`, `TimeoutError`, ...).
//!
//! Supported: `http://`, with an IPv4/IPv6 literal or a name, any port, no proxy, no redirect, no authentication, no keep-alive.
//! Outside this profile (documented, never silently emulated): `https://` and the other `urllib` schemes `file:`, `ftp:` and `data:`
//! (a `NotImplementedError`), the environment's proxy variables, following a redirect (a 3xx is an HTTP status like any other),
//! non-ASCII host names, a timeout of zero or one that is not a number.

use crate::escalation::Raised;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

const MAX_LINE: usize = 65536;
const MAX_HEADERS: usize = 100;

fn plain(class: &str, message: &str) -> Raised {
    Raised::plain(class, message)
}

fn module_error(class: &str, module: &str, message: &str) -> Raised {
    Raised::new(class, module, PyStr::new(message))
}

// ── the URL, as `urllib.request.Request` takes it apart ──────────────────────────────────────────

/// `urllib.request.Request(url, ...)` after construction: the parts the exchange needs.
#[derive(Clone, Debug)]
pub struct UrlRequest {
    /// `req.type`: the scheme, lower-cased.
    pub scheme: PyStr,
    /// `req.host`: the authority after `unquote`, `None` when the URL has none.
    pub host: Option<PyStr>,
    pub selector: PyStr,
    /// `req.full_url` (the URL with its fragment).
    pub full_url: PyStr,
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn starts(haystack: &[u32], needle: &str) -> bool {
    haystack.starts_with(&points(needle))
}

/// `urllib.parse.unwrap`.
fn unwrap(url: &PyStr) -> PyStr {
    let mut text = url.strip();
    let all = text.code_points().to_vec();
    if all.first() == Some(&u32::from('<')) && all.last() == Some(&u32::from('>')) {
        text = PyStr::from_code_points(all[1..all.len() - 1].to_vec()).strip();
    }
    let all = text.code_points().to_vec();
    if starts(&all, "URL:") {
        text = PyStr::from_code_points(all[4..].to_vec()).strip();
    }
    text
}

/// `urllib.parse.unquote` of a host: percent escapes decoded as UTF-8 with replacement characters.
fn unquote(text: &PyStr) -> PyStr {
    let source = text.code_points();
    if !source.contains(&u32::from('%')) {
        return text.clone();
    }
    let mut out: Vec<u32> = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    let flush = |bytes: &mut Vec<u8>, out: &mut Vec<u32>| {
        if !bytes.is_empty() {
            for ch in String::from_utf8_lossy(bytes).chars() {
                out.push(u32::from(ch));
            }
            bytes.clear();
        }
    };
    let hex = |value: u32| char::from_u32(value).and_then(|c| c.to_digit(16));
    let mut at = 0;
    while at < source.len() {
        if source[at] == u32::from('%') {
            if let (Some(high), Some(low)) = (
                source.get(at + 1).copied().and_then(hex),
                source.get(at + 2).copied().and_then(hex),
            ) {
                bytes.push((high * 16 + low) as u8);
                at += 3;
                continue;
            }
        }
        flush(&mut bytes, &mut out);
        out.push(source[at]);
        at += 1;
    }
    flush(&mut bytes, &mut out);
    PyStr::from_code_points(out)
}

/// `_splittype`: `([^/:]+):(.*)` with the scheme lower-cased, else no scheme.
fn split_type(url: &[u32]) -> (Option<Vec<u32>>, Vec<u32>) {
    let slash = u32::from('/');
    let colon = u32::from(':');
    let mut at = 0;
    while at < url.len() && url[at] != slash && url[at] != colon {
        at += 1;
    }
    if at > 0 && at < url.len() && url[at] == colon {
        let scheme: Vec<u32> = url[..at]
            .iter()
            .map(|&p| {
                char::from_u32(p)
                    .map(|c| c.to_lowercase().collect::<Vec<char>>())
                    .filter(|lowered| lowered.len() == 1)
                    .map_or(p, |lowered| u32::from(lowered[0]))
            })
            .collect();
        return (Some(scheme), url[at + 1..].to_vec());
    }
    (None, url.to_vec())
}

/// `_splithost`: `//([^/#?]*)(.*)`, the path getting a leading `/` when it has none.
fn split_host(rest: &[u32]) -> (Option<Vec<u32>>, Vec<u32>) {
    if !starts(rest, "//") {
        return (None, rest.to_vec());
    }
    let stops = [u32::from('/'), u32::from('#'), u32::from('?')];
    let body = &rest[2..];
    let end = body
        .iter()
        .position(|p| stops.contains(p))
        .unwrap_or(body.len());
    let host = body[..end].to_vec();
    let mut path = body[end..].to_vec();
    if !path.is_empty() && path[0] != u32::from('/') {
        path.insert(0, u32::from('/'));
    }
    (Some(host), path)
}

/// `urllib.request.Request(url)`: `ValueError("unknown url type: ...")` for a URL without a scheme.
pub fn parse_url(url: &PyStr) -> Result<UrlRequest, Raised> {
    let unwrapped = unwrap(url);
    let all = unwrapped.code_points().to_vec();
    let (without_fragment, fragment) = match all.iter().rposition(|&p| p == u32::from('#')) {
        Some(at) => (all[..at].to_vec(), Some(all[at + 1..].to_vec())),
        None => (all.clone(), None),
    };
    let mut full = without_fragment.clone();
    if let Some(tag) = fragment.as_ref().filter(|tag| !tag.is_empty()) {
        full.push(u32::from('#'));
        full.extend_from_slice(tag);
    }
    let full_url = PyStr::from_code_points(full);
    let (scheme, rest) = split_type(&without_fragment);
    let Some(scheme) = scheme else {
        return Err(plain(
            "ValueError",
            &format!("unknown url type: {}", full_url.repr()),
        ));
    };
    let (host, selector) = split_host(&rest);
    Ok(UrlRequest {
        scheme: PyStr::from_code_points(scheme),
        host: host.map(|h| unquote(&PyStr::from_code_points(h))),
        selector: PyStr::from_code_points(selector),
        full_url,
    })
}

// ── errors ───────────────────────────────────────────────────────────────────────────────────────

/// What `urlopen` raised.
#[derive(Debug)]
pub enum OpenError {
    /// `urllib.error.URLError(reason)`: `str(error)` is `<urlopen error REASON>`.
    Url { reason: PyStr },
    /// `urllib.error.HTTPError`: a non-2xx answer, whose body is still to be read.
    Status(Box<Reply>),
    /// Any other exception.
    Other(Raised),
}

fn url_error(reason: &str) -> OpenError {
    OpenError::Url {
        reason: PyStr::new(reason),
    }
}

/// An I/O error as the Python exception of the same operation: a timeout is `TimeoutError("timed out")`, the others keep the system's
/// text (`[Errno N] message`) and the class of their errno.
fn io_exception(error: &std::io::Error) -> Raised {
    use std::io::ErrorKind;
    match (error.raw_os_error(), error.kind()) {
        (Some(110), _) | (Some(111), _) => {
            Raised::from(crate::model_files::os_failure(error, None))
        }
        (None, ErrorKind::TimedOut | ErrorKind::WouldBlock) | (Some(11), _) => {
            plain("TimeoutError", "timed out")
        }
        _ => {
            let text = error.to_string();
            if text.contains("Name or service not known") {
                return module_error("gaierror", "socket", "[Errno -2] Name or service not known");
            }
            if text.contains("Temporary failure in name resolution") {
                return module_error(
                    "gaierror",
                    "socket",
                    "[Errno -3] Temporary failure in name resolution",
                );
            }
            Raised::from(crate::model_files::os_failure(error, None))
        }
    }
}

// ── the exchange ─────────────────────────────────────────────────────────────────────────────────

/// `timeout` of `urlopen`: no limit, or seconds per socket operation.
#[derive(Clone, Copy, Debug)]
pub enum Timeout {
    None,
    Seconds(f64),
}

/// The timeout argument as the socket layer takes it: `None` or a real number above zero are supported; a negative number or `NaN`
/// have the interpreter's own messages; zero and other types are outside the profile.
pub fn timeout_of(value: &PyValue) -> Result<Timeout, Raised> {
    match value.peel() {
        PyValue::None => Ok(Timeout::None),
        PyValue::Bool(flag) => Ok(Timeout::Seconds(f64::from(u8::from(*flag)))).and_then(positive),
        PyValue::Int(number) => {
            let seconds =
                agent_runtime_support::pynumber::float_from_int(number).map_err(|_| {
                    plain(
                        "OverflowError",
                        "timestamp too large to convert to C _PyTime_t",
                    )
                })?;
            positive(Timeout::Seconds(seconds))
        }
        PyValue::Float(number) => {
            if number.is_nan() {
                return Err(plain("ValueError", "Invalid value NaN (not a number)"));
            }
            positive(Timeout::Seconds(*number))
        }
        other => Err(plain(
            "NotImplementedError",
            &format!(
                "a timeout of type {} is outside the native HTTP client's profile",
                other.type_name()
            ),
        )),
    }
}

fn positive(timeout: Timeout) -> Result<Timeout, Raised> {
    match timeout {
        Timeout::Seconds(seconds) if seconds < 0.0 => {
            Err(plain("ValueError", "Timeout value out of range"))
        }
        Timeout::Seconds(0.0) => Err(plain(
            "NotImplementedError",
            "a timeout of zero (non-blocking sockets) is outside the native HTTP client's profile",
        )),
        other => Ok(other),
    }
}

fn duration(timeout: Timeout) -> Option<Duration> {
    match timeout {
        Timeout::None => None,
        Timeout::Seconds(seconds) => Some(Duration::from_secs_f64(seconds.min(1.0e9))),
    }
}

/// The escape `UnicodeEncodeError.__str__` shows for one character: `\\xNN` below 0x100, `\\uNNNN` in the BMP, `\\UNNNNNNNN` above.
fn escaped_character(point: u32) -> String {
    if point <= 0xFF {
        format!("\\x{point:02x}")
    } else if point <= 0xFFFF {
        format!("\\u{point:04x}")
    } else {
        format!("\\U{point:08x}")
    }
}

/// `HTTPConnection._get_hostport`: the host and the port of the authority (`InvalidURL` for a port that is not a number).
fn host_port(authority: &PyStr) -> Result<(PyStr, PyValue), Raised> {
    let all = authority.code_points().to_vec();
    let colon = all.iter().rposition(|&p| p == u32::from(':'));
    let bracket = all.iter().rposition(|&p| p == u32::from(']'));
    let (mut host, mut port) = (all.clone(), PyValue::int(80));
    if let Some(i) = colon {
        if bracket.is_none_or(|j| i > j) {
            let digits = PyStr::from_code_points(all[i + 1..].to_vec());
            if digits.is_empty() {
                host = all[..i].to_vec();
            } else {
                match crate::pyexpr::int_of(&PyValue::Str(digits.clone())) {
                    Ok(number) => {
                        port = PyValue::Int(number);
                        host = all[..i].to_vec();
                    }
                    Err(_) => {
                        return Err(module_error(
                            "InvalidURL",
                            "http.client",
                            &format!(
                                "nonnumeric port: '{}'",
                                digits.to_utf8().unwrap_or_default()
                            ),
                        ))
                    }
                }
            }
        }
    }
    if host.first() == Some(&u32::from('[')) && host.last() == Some(&u32::from(']')) {
        host = host[1..host.len() - 1].to_vec();
    }
    Ok((PyStr::from_code_points(host), port))
}

/// The control-character test of `http.client` (`[\x00-\x20\x7f]`): the first offending character.
fn disallowed(text: &PyStr) -> Option<u32> {
    text.code_points()
        .iter()
        .copied()
        .find(|&p| p <= 0x20 || p == 0x7F)
}

fn control_characters(text: &PyStr, found: u32) -> Raised {
    module_error(
        "InvalidURL",
        "http.client",
        &format!(
            "URL can't contain control characters. {} (found at least {})",
            text.repr(),
            PyStr::from_code_points(vec![found]).repr()
        ),
    )
}

/// A buffered reader over the socket (`sock.makefile("rb")`): lines up to a limit and exact counts, every failure an exception.
struct Source {
    stream: TcpStream,
    buffer: Vec<u8>,
    position: usize,
    ended: bool,
}

impl Source {
    fn fill(&mut self) -> Result<bool, Raised> {
        if self.ended {
            return Ok(false);
        }
        if self.position > 0 && self.position == self.buffer.len() {
            self.buffer.clear();
            self.position = 0;
        }
        let mut chunk = [0u8; 8192];
        loop {
            match self.stream.read(&mut chunk) {
                Ok(0) => {
                    self.ended = true;
                    return Ok(false);
                }
                Ok(count) => {
                    self.buffer.extend_from_slice(&chunk[..count]);
                    return Ok(true);
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(error) => return Err(io_exception(&error)),
            }
        }
    }

    /// `fp.readline(limit)`: up to and including the newline, or `limit` bytes, or the end of the stream.
    fn read_line(&mut self, limit: usize) -> Result<Vec<u8>, Raised> {
        let mut line = Vec::new();
        loop {
            while self.position < self.buffer.len() {
                let byte = self.buffer[self.position];
                self.position += 1;
                line.push(byte);
                if byte == b'\n' || line.len() >= limit {
                    return Ok(line);
                }
            }
            if !self.fill()? {
                return Ok(line);
            }
        }
    }

    /// `fp.read(count)`: exactly `count` bytes unless the stream ends first.
    fn read_exact_or_end(&mut self, count: usize) -> Result<Vec<u8>, Raised> {
        let mut out = Vec::with_capacity(count.min(1 << 20));
        while out.len() < count {
            if self.position < self.buffer.len() {
                let take = (self.buffer.len() - self.position).min(count - out.len());
                out.extend_from_slice(&self.buffer[self.position..self.position + take]);
                self.position += take;
                continue;
            }
            if !self.fill()? {
                break;
            }
        }
        Ok(out)
    }

    fn read_to_end(&mut self) -> Result<Vec<u8>, Raised> {
        let mut out = Vec::new();
        loop {
            out.extend_from_slice(&self.buffer[self.position..]);
            self.position = self.buffer.len();
            if !self.fill()? {
                return Ok(out);
            }
        }
    }
}

/// How the body of an answer is delimited.
#[derive(Debug, Clone, Copy)]
enum Body {
    Length(usize),
    Chunked,
    UntilEnd,
}

/// An answer whose status line and headers were read (`http.client.HTTPResponse` after `begin`).
pub struct Reply {
    pub status: u32,
    /// `response.reason`, stripped (the `msg` of an `HTTPError`).
    pub reason: PyStr,
    source: Option<Source>,
    body: Body,
}

impl std::fmt::Debug for Reply {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Reply({} {})", self.status, self.reason.repr())
    }
}

fn incomplete(partial: usize, expected: Option<usize>) -> Raised {
    let expected = expected.map_or(String::new(), |count| format!(", {count} more expected"));
    module_error(
        "IncompleteRead",
        "http.client",
        &format!("IncompleteRead({partial} bytes read{expected})"),
    )
}

impl Reply {
    /// `response.read()`: the whole body, or the exception `http.client` raises.
    pub fn read_all(&mut self) -> Result<Vec<u8>, Raised> {
        let Some(source) = self.source.as_mut() else {
            return Ok(Vec::new());
        };
        let outcome = match self.body {
            Body::Length(length) => {
                let data = source.read_exact_or_end(length);
                match data {
                    Ok(data) if data.len() < length => {
                        Err(incomplete(data.len(), Some(length - data.len())))
                    }
                    other => other,
                }
            }
            Body::UntilEnd => source.read_to_end(),
            Body::Chunked => read_chunked(source),
        };
        self.source = None;
        outcome
    }
}

/// `int(line, 16)` of a chunk-size line as CPython parses it: surrounding ASCII whitespace, an optional sign, an optional `0x`, hex digits with
/// single underscores between them. `None` is the `ValueError`; the magnitude saturates (a size no stream can hold is read to the end).
fn python_hex(text: &[u8]) -> Option<(bool, u128)> {
    let space = |byte: u8| matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c);
    let mut rest = text;
    while let Some((&first, tail)) = rest.split_first() {
        if !space(first) {
            break;
        }
        rest = tail;
    }
    while let Some((&last, head)) = rest.split_last() {
        if !space(last) {
            break;
        }
        rest = head;
    }
    let mut negative = false;
    if let Some((&sign, tail)) = rest.split_first() {
        if sign == b'+' || sign == b'-' {
            negative = sign == b'-';
            rest = tail;
        }
    }
    if rest.len() >= 2 && rest[0] == b'0' && (rest[1] == b'x' || rest[1] == b'X') {
        rest = &rest[2..];
        if rest.first() == Some(&b'_') {
            rest = &rest[1..];
        }
    }
    let (mut value, mut seen_digit, mut after_underscore) = (0u128, false, false);
    for &byte in rest {
        if byte == b'_' {
            if !seen_digit || after_underscore {
                return None;
            }
            after_underscore = true;
        } else {
            let digit = char::from(byte).to_digit(16)?;
            seen_digit = true;
            after_underscore = false;
            value = value.saturating_mul(16).saturating_add(u128::from(digit));
        }
    }
    (seen_digit && !after_underscore).then_some((negative, value))
}

/// `HTTPResponse._read_next_chunk_size`: `Ok(None)` is the `ValueError` of an invalid size (the connection is dropped by the caller).
fn read_next_chunk_size(source: &mut Source) -> Result<Option<(bool, u128)>, Raised> {
    let line = source.read_line(MAX_LINE + 1)?;
    if line.len() > MAX_LINE {
        return Err(module_error(
            "LineTooLong",
            "http.client",
            &format!("got more than {MAX_LINE} bytes when reading chunk size"),
        ));
    }
    let size_text = match line.iter().position(|&b| b == b';') {
        Some(at) => &line[..at],
        None => &line[..],
    };
    Ok(python_hex(size_text))
}

/// `HTTPResponse._read_and_discard_trailer`.
fn discard_trailer(source: &mut Source) -> Result<(), Raised> {
    loop {
        let trailer = source.read_line(MAX_LINE + 1)?;
        if trailer.len() > MAX_LINE {
            return Err(module_error(
                "LineTooLong",
                "http.client",
                &format!("got more than {MAX_LINE} bytes when reading trailer line"),
            ));
        }
        if trailer.is_empty() || trailer == b"\r\n" || trailer == b"\n" {
            return Ok(());
        }
    }
}

/// `HTTPResponse._read_chunked(None)` with `_get_chunk_left` and `_safe_read`. An `IncompleteRead` raised while reading (a short chunk,
/// a short chunk terminator, an invalid size: the `ValueError` becomes `IncompleteRead(b'')` first) is re-raised as an `IncompleteRead`
/// of the COMPLETED chunks only, the bytes of a current short chunk being discarded; every other exception (a timeout, a reset, a line
/// that is too long) passes through unchanged.
fn read_chunked(source: &mut Source) -> Result<Vec<u8>, Raised> {
    let mut value: Vec<u8> = Vec::new();
    let mut at_chunk_end = false;
    loop {
        if at_chunk_end {
            // the CRLF after a chunk is thrown away (two bytes, not looked at)
            let end = source.read_exact_or_end(2)?;
            if end.len() < 2 {
                return Err(incomplete(value.len(), None));
            }
        }
        let size = match read_next_chunk_size(source)? {
            Some(size) => size,
            None => return Err(incomplete(value.len(), None)),
        };
        match size {
            (false, 0) | (true, 0) => {
                discard_trailer(source)?;
                return Ok(value);
            }
            (true, _) => {
                // a negative size reads to the end of the stream (`read(-n)`), then the next terminator is missing
                let data = source.read_to_end()?;
                value.extend_from_slice(&data);
            }
            (false, wanted) => {
                let wanted = usize::try_from(wanted).unwrap_or(usize::MAX);
                let data = source.read_exact_or_end(wanted)?;
                if data.len() < wanted {
                    return Err(incomplete(value.len(), None));
                }
                value.extend_from_slice(&data);
            }
        }
        at_chunk_end = true;
    }
}

fn latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

fn bad_status(line: &[u8]) -> Raised {
    let text = latin1(line);
    Raised::new(
        "BadStatusLine",
        "http.client",
        PyStr::new(if text.is_empty() { "''" } else { &text }),
    )
}

/// `HTTPResponse._read_status`.
fn read_status(source: &mut Source) -> Result<(String, u32, String), Raised> {
    let line = source.read_line(MAX_LINE + 1)?;
    if line.len() > MAX_LINE {
        return Err(module_error(
            "LineTooLong",
            "http.client",
            &format!("got more than {MAX_LINE} bytes when reading status line"),
        ));
    }
    if line.is_empty() {
        return Err(module_error(
            "RemoteDisconnected",
            "http.client",
            "Remote end closed connection without response",
        ));
    }
    let text = latin1(&line);
    let mut parts = text.split_whitespace();
    let (version, status, reason) = {
        let version = parts.next();
        let status = parts.next();
        match (version, status) {
            (Some(version), Some(status)) => {
                // the reason is the rest of the line after the second token
                let after_version = text.trim_start().strip_prefix(version).unwrap_or("");
                let after_status = after_version
                    .trim_start()
                    .strip_prefix(status)
                    .unwrap_or("");
                let reason = after_status.trim_start_matches([' ', '\t']).to_string();
                (version.to_string(), status.to_string(), reason)
            }
            (Some(version), None) => (version.to_string(), String::new(), String::new()),
            _ => (String::new(), String::new(), String::new()),
        }
    };
    if !version.starts_with("HTTP/") {
        return Err(bad_status(&line));
    }
    let Ok(code) = status.trim().parse::<i64>() else {
        return Err(bad_status(&line));
    };
    if !(100..=999).contains(&code) {
        return Err(bad_status(&line));
    }
    Ok((version, u32::try_from(code).unwrap_or(0), reason))
}

/// The header lines up to the empty one (`_read_headers`): `(name, value)` in order.
fn read_headers(source: &mut Source) -> Result<Vec<(String, String)>, Raised> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    loop {
        let line = source.read_line(MAX_LINE + 1)?;
        if line.len() > MAX_LINE {
            return Err(module_error(
                "LineTooLong",
                "http.client",
                &format!("got more than {MAX_LINE} bytes when reading header line"),
            ));
        }
        lines.push(line.clone());
        if lines.len() > MAX_HEADERS {
            return Err(module_error(
                "HTTPException",
                "http.client",
                &format!("got more than {MAX_HEADERS} headers"),
            ));
        }
        if line == b"\r\n" || line == b"\n" || line.is_empty() {
            break;
        }
    }
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        let text = latin1(&line);
        let text = text.trim_end_matches(['\r', '\n']);
        if text.is_empty() {
            continue;
        }
        if text.starts_with([' ', '\t']) {
            if let Some(last) = headers.last_mut() {
                last.1.push(' ');
                last.1.push_str(text.trim());
            }
            continue;
        }
        if let Some((name, value)) = text.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    Ok(headers)
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(held, _)| held.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// `HTTPResponse.begin`.
fn begin(mut source: Source) -> Result<Reply, Raised> {
    let (version, status, reason) = loop {
        let (version, status, reason) = read_status(&mut source)?;
        if status != 100 {
            break (version, status, reason);
        }
        // skip the headers of a 100 response
        loop {
            let skip = source.read_line(MAX_LINE + 1)?;
            if skip.len() > MAX_LINE {
                return Err(module_error(
                    "LineTooLong",
                    "http.client",
                    &format!("got more than {MAX_LINE} bytes when reading header line"),
                ));
            }
            if skip == b"\r\n" || skip == b"\n" || skip.is_empty() {
                break;
            }
        }
    };
    if !(version == "HTTP/1.0" || version == "HTTP/0.9" || version.starts_with("HTTP/1.")) {
        return Err(module_error("UnknownProtocol", "http.client", &version));
    }
    let headers = read_headers(&mut source)?;
    let chunked = header(&headers, "transfer-encoding")
        .is_some_and(|value| value.eq_ignore_ascii_case("chunked"));
    let mut length = if chunked {
        None
    } else {
        header(&headers, "content-length")
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<i64>().ok())
            .and_then(|value| usize::try_from(value).ok())
    };
    if status == 204 || status == 304 || (100..200).contains(&status) {
        length = Some(0);
    }
    let body = if chunked {
        Body::Chunked
    } else {
        length.map_or(Body::UntilEnd, Body::Length)
    };
    Ok(Reply {
        status,
        reason: PyStr::new(reason.trim()),
        source: Some(source),
        body,
    })
}

/// `urllib.request.urlopen(req, timeout=...)` for a `POST` of `data`: the answer of a 2xx, `OpenError::Status` for any other
/// status, or what `urllib` raised.
pub fn open(request: &UrlRequest, data: &[u8], timeout: &PyValue) -> Result<Reply, OpenError> {
    let scheme = request.scheme.to_utf8().unwrap_or_default();
    match scheme.as_str() {
        "http" => {}
        "https" | "file" | "ftp" | "data" => {
            return Err(OpenError::Other(plain(
                "NotImplementedError",
                &format!("the {scheme}: scheme is outside the native HTTP client's profile"),
            )))
        }
        other => return Err(url_error(&format!("unknown url type: {other}"))),
    }
    // `do_request_`: no host, no request.
    let Some(authority) = request.host.as_ref().filter(|host| !host.is_empty()) else {
        return Err(url_error("no host given"));
    };
    // `HTTPConnection(host, timeout=...)`: the port, the host's characters.
    let (host, port) = host_port(authority).map_err(OpenError::Other)?;
    if let Some(found) = disallowed(&host) {
        return Err(OpenError::Other(control_characters(&host, found)));
    }
    // `putrequest`: the selector is validated, then the whole request line is ASCII.
    let selector = if request.selector.is_empty() {
        PyStr::new("/")
    } else {
        request.selector.clone()
    };
    if let Some(found) = disallowed(&selector) {
        return Err(OpenError::Other(control_characters(&selector, found)));
    }
    let line_prefix = "POST ".chars().count();
    let sel_points = selector.code_points();
    if let Some(start) = sel_points.iter().position(|&p| p > 0x7F) {
        let end = start
            + sel_points[start..]
                .iter()
                .take_while(|&&p| p > 0x7F)
                .count();
        let message = if end - start == 1 {
            format!(
                "'ascii' codec can't encode character '{}' in position {}: ordinal not in range(128)",
                escaped_character(sel_points[start]),
                line_prefix + start
            )
        } else {
            format!(
                "'ascii' codec can't encode characters in position {}-{}: ordinal not in range(128)",
                line_prefix + start,
                line_prefix + end - 1
            )
        };
        return Err(OpenError::Other(plain("UnicodeEncodeError", &message)));
    }
    if authority.code_points().iter().any(|&p| p > 0x7F) {
        return Err(OpenError::Other(plain(
            "NotImplementedError",
            "a non-ASCII host name is outside the native HTTP client's profile",
        )));
    }
    let selector_text = selector.to_utf8().unwrap_or_default();
    let authority_text = authority.to_utf8().unwrap_or_default();
    let timeout = timeout_of(timeout).map_err(OpenError::Other)?;

    // connect (`socket.create_connection`): every address in turn, the last error is the error
    let port_number = match &port {
        PyValue::Int(number) => i64::try_from(number.clone())
            .ok()
            .filter(|p| (0..=65535).contains(p)),
        _ => None,
    };
    let Some(port_number) = port_number else {
        return Err(OpenError::Other(plain(
            "OverflowError",
            "getsockaddrarg: port must be 0-65535.",
        )));
    };
    let host_text = host.to_utf8().unwrap_or_default();
    let addresses: Vec<std::net::SocketAddr> =
        match (host_text.as_str(), u16::try_from(port_number).unwrap_or(0)).to_socket_addrs() {
            Ok(found) => found.collect(),
            Err(error) => {
                return Err(OpenError::Url {
                    reason: PyStr::new(&io_exception(&error).message.to_utf8().unwrap_or_default()),
                })
            }
        };
    let mut last: Option<Raised> = None;
    let mut stream: Option<TcpStream> = None;
    for address in addresses {
        let attempt = match duration(timeout) {
            Some(limit) => TcpStream::connect_timeout(&address, limit),
            None => TcpStream::connect(address),
        };
        match attempt {
            Ok(connected) => {
                stream = Some(connected);
                break;
            }
            Err(error) => last = Some(io_exception(&error)),
        }
    }
    let Some(stream) = stream else {
        let reason = last.map_or_else(
            || PyStr::new("getaddrinfo returns an empty list"),
            |raised| raised.message,
        );
        return Err(OpenError::Url { reason });
    };
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(duration(timeout));
    let _ = stream.set_write_timeout(duration(timeout));

    // the request: line, headers in `http.client`'s order, then the body
    let head = format!(
        "POST {selector_text} HTTP/1.1\r\nAccept-Encoding: identity\r\nContent-Length: {}\r\nHost: {authority_text}\r\nUser-Agent: Python-urllib/3.12\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n",
        data.len()
    );
    let mut writer = &stream;
    let sent = writer
        .write_all(head.as_bytes())
        .and_then(|()| writer.write_all(data))
        .and_then(|()| writer.flush());
    if let Err(error) = sent {
        return Err(OpenError::Url {
            reason: io_exception(&error).message,
        });
    }
    let source = Source {
        stream,
        buffer: Vec::new(),
        position: 0,
        ended: false,
    };
    let reply = begin(source).map_err(OpenError::Other)?;
    if (200..300).contains(&reply.status) {
        Ok(reply)
    } else {
        Err(OpenError::Status(Box::new(reply)))
    }
}
