//! The bytes `urllib.request` + `http.client` put on the wire for `urlopen(url)` of a plain `GET`.
//!
//! `AbstractHTTPHandler.do_open` hands `http.client` the headers `Host` and `User-agent` (title-cased to `User-Agent`) plus
//! `Connection: close`; because `Host` is supplied, `putrequest` only adds `Accept-Encoding: identity` (before them). No `Accept`
//! header exists. The request line is `GET <selector> HTTP/1.1`, the head is one write, and there is no body.

use super::failure::{ascii_encode_error, invalid_url, PyException};
use super::url::HttpTarget;
use agent_runtime_support::pystr::PyStr;
#[cfg(test)]
use agent_runtime_support::pyvalue::PyValue;

/// `urllib.request.OpenerDirector.addheaders`: `User-agent: Python-urllib/<major>.<minor>` of the pinned interpreter.
pub const USER_AGENT: &str = "Python-urllib/3.12";

/// The headers a caller passes to `urllib.request.Request(url, headers=...)`, in the order given.
///
/// `Request.add_header` stores each name as `name.capitalize()`, so a later entry of the same capitalized name replaces the value and keeps the
/// position of the first. `do_open` then sends `Host` (the unredirected header), the default `User-agent` unless the caller supplied one, the
/// caller's headers, and `Connection: close` (in place of the caller's own `Connection`, at its position); `http.client` title-cases every name
/// and adds `Accept-Encoding: identity` first unless the caller named `Accept-Encoding`.
///
/// Scope: names are ASCII tokens and values are ASCII without line breaks, as `putheader` accepts them; anything else is refused by name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RequestHeaders {
    /// `(name.capitalize(), value)` in the order of first insertion.
    entries: Vec<(String, String)>,
}

fn is_header_token(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_graphic()
                && !matches!(
                    byte,
                    b':' | b'('
                        | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                        | b'{'
                        | b'}'
                )
        })
}

fn is_header_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || (0x20..0x7F).contains(&byte))
}

/// `str.capitalize()` of an ASCII name.
fn capitalize(name: &str) -> String {
    let mut letters = name.chars();
    match letters.next() {
        Some(first) => {
            first.to_ascii_uppercase().to_string() + &letters.as_str().to_ascii_lowercase()
        }
        None => String::new(),
    }
}

/// `str.title()` of an ASCII name: a letter after a non-letter is upper-case, every other letter lower-case.
fn title(name: &str) -> String {
    let mut previous_is_letter = false;
    name.chars()
        .map(|ch| {
            let shown = if previous_is_letter {
                ch.to_ascii_lowercase()
            } else {
                ch.to_ascii_uppercase()
            };
            previous_is_letter = ch.is_ascii_alphabetic();
            shown
        })
        .collect()
}

impl RequestHeaders {
    /// `Request(url, headers=dict(entries))`; `Err` names the first entry outside the supported scope.
    pub fn new(entries: &[(&str, &str)]) -> Result<RequestHeaders, String> {
        let mut stored: Vec<(String, String)> = Vec::new();
        for (name, value) in entries {
            if !is_header_token(name) {
                return Err(format!("header name {name:?} is not an ASCII token"));
            }
            if !is_header_value(value) {
                return Err(format!(
                    "the value of header {name:?} is not ASCII text without line breaks"
                ));
            }
            let key = capitalize(name);
            match stored.iter_mut().find(|(known, _)| *known == key) {
                Some((_, held)) => *held = (*value).to_string(),
                None => stored.push((key, (*value).to_string())),
            }
        }
        Ok(RequestHeaders { entries: stored })
    }

    fn has(&self, capitalized: &str) -> bool {
        self.entries.iter().any(|(name, _)| name == capitalized)
    }

    /// The `(name, value)` lines `http.client` writes after the request line, in order, with `Accept-Encoding` first when it is added.
    fn wire_lines(&self, host_header: &str) -> Vec<(String, String)> {
        let mut lines: Vec<(String, String)> = vec![("Host".to_string(), host_header.to_string())];
        if !self.has("User-agent") {
            lines.push(("User-agent".to_string(), USER_AGENT.to_string()));
        }
        for (name, value) in &self.entries {
            if !lines.iter().any(|(known, _)| known == name) {
                lines.push((name.clone(), value.clone()));
            }
        }
        match lines.iter_mut().find(|(name, _)| name == "Connection") {
            Some((_, value)) => *value = "close".to_string(),
            None => lines.push(("Connection".to_string(), "close".to_string())),
        }
        let skips_accept_encoding = lines
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case("accept-encoding"));
        let mut shown: Vec<(String, String)> = Vec::new();
        if !skips_accept_encoding {
            shown.push(("Accept-Encoding".to_string(), "identity".to_string()));
        }
        shown.extend(lines.into_iter().map(|(name, value)| (title(&name), value)));
        shown
    }
}

/// The request head of a `GET` to `target` with the caller's `headers`: one write, no body.
pub fn get_request_bytes_with(target: &HttpTarget, headers: &RequestHeaders) -> Vec<u8> {
    let mut head = format!("GET {} HTTP/1.1\r\n", target.selector);
    for (name, value) in headers.wire_lines(&target.host_header) {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    head.into_bytes()
}

/// Lone surrogates of a Python text (a command-line argument decoded with `surrogateescape`) carried through the `String`-based URL code.
///
/// A Rust `String` cannot hold a surrogate, so each distinct surrogate of ONE text is written as a stand-in character chosen for that text: a
/// private-use character the text does not itself contain. The carrier lists exactly which stand-ins it introduced; only those are read back as
/// surrogates, by the code that builds a message or an exception object ([`check_request_line`]). Text that did not pass through
/// [`SurrogateCarrier::carry`] has an empty carrier, so a genuine private-use character (U+F0000 included) always stays itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurrogateCarrier {
    stand_ins: Vec<(char, u32)>,
}

impl SurrogateCarrier {
    /// The carrier of genuine Unicode text: no character is read back as a surrogate.
    pub fn none() -> SurrogateCarrier {
        SurrogateCarrier::default()
    }

    /// The `String` for `points` with its carrier. `None` only when no unused private-use character is left for a surrogate (the caller refuses
    /// by name).
    pub fn carry(points: &[u32]) -> Option<(String, SurrogateCarrier)> {
        let present: std::collections::HashSet<u32> = points.iter().copied().collect();
        let mut free = (0xF0000..=0x10FFFD).filter(|point| !present.contains(point));
        let mut carrier = SurrogateCarrier::none();
        let mut text = String::with_capacity(points.len());
        for &point in points {
            let ch = match point {
                0xD800..=0xDFFF => match carrier.stand_ins.iter().find(|(_, own)| *own == point) {
                    Some((stand_in, _)) => *stand_in,
                    None => {
                        let stand_in = char::from_u32(free.next()?)?;
                        carrier.stand_ins.push((stand_in, point));
                        stand_in
                    }
                },
                _ => char::from_u32(point)?,
            };
            text.push(ch);
        }
        Some((text, carrier))
    }

    /// The code point `ch` stands for: the surrogate it carries, else its own.
    fn code_point_of(&self, ch: char) -> u32 {
        self.stand_ins
            .iter()
            .find(|(stand_in, _)| *stand_in == ch)
            .map_or(ch as u32, |(_, surrogate)| *surrogate)
    }
}

/// `repr()` of a text of code points (a surrogate is written `\udcff`).
fn repr_of(points: Vec<u32>) -> String {
    PyStr::from_code_points(points).repr()
}

/// What `HTTPConnection.putrequest` checks before it sends anything (and therefore before a connection is opened): `_validate_path` on the
/// selector, then the ASCII encoding of the request line `GET <selector> HTTP/1.1` (`_encode_request`). Both raise before any I/O.
pub fn check_request_line(
    selector: &str,
    carrier: &SurrogateCarrier,
) -> Result<(), Box<PyException>> {
    let points: Vec<u32> = selector
        .chars()
        .map(|ch| carrier.code_point_of(ch))
        .collect();
    if let Some(found) = points
        .iter()
        .copied()
        .find(|point| *point <= 0x20 || *point == 0x7F)
    {
        return Err(Box::new(invalid_url(&format!(
            "URL can't contain control characters. {} (found at least {})",
            repr_of(points.clone()),
            repr_of(vec![found])
        ))));
    }
    // `'%s %s %s' % (method, url, version)`: the object and the positions are those of the whole request line
    let line: Vec<u32> = "GET "
        .chars()
        .map(u32::from)
        .chain(points)
        .chain(" HTTP/1.1".chars().map(u32::from))
        .collect();
    let Some(start) = line.iter().position(|point| *point > 0x7F) else {
        return Ok(());
    };
    let end = start
        + line[start..]
            .iter()
            .take_while(|point| **point > 0x7F)
            .count();
    Err(Box::new(ascii_encode_error(&line, start, end)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_head_has_the_header_order_of_urllib() {
        let target = HttpTarget {
            host_header: "127.0.0.1:8080".to_string(),
            connect_host: "127.0.0.1".to_string(),
            port: 8080,
            selector: "/x".to_string(),
        };
        assert_eq!(
            get_request_bytes_with(&target, &RequestHeaders::default()),
            b"GET /x HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: 127.0.0.1:8080\r\nUser-Agent: Python-urllib/3.12\r\nConnection: close\r\n\r\n"
        );
    }

    fn target() -> HttpTarget {
        HttpTarget {
            host_header: "api.github.com".to_string(),
            connect_host: "api.github.com".to_string(),
            port: 443,
            selector: "/repos/x".to_string(),
        }
    }

    #[test]
    fn caller_headers_follow_host_in_the_order_given_and_replace_the_default_user_agent() {
        let headers = RequestHeaders::new(&[
            ("Accept", "application/vnd.github+json"),
            ("User-Agent", "upstream-version-watch"),
        ])
        .unwrap();
        assert_eq!(
            get_request_bytes_with(&target(), &headers),
            b"GET /repos/x HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: api.github.com\r\nAccept: application/vnd.github+json\r\nUser-Agent: upstream-version-watch\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn a_caller_without_a_user_agent_keeps_the_default_one_before_its_own_headers() {
        let headers = RequestHeaders::new(&[("accept", "a/b")]).unwrap();
        assert_eq!(
            get_request_bytes_with(&target(), &headers),
            b"GET /repos/x HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: api.github.com\r\nUser-Agent: Python-urllib/3.12\r\nAccept: a/b\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn names_are_capitalized_then_title_cased_and_repeats_keep_the_first_position() {
        let headers = RequestHeaders::new(&[
            ("X-aBc-dEf", "1"),
            ("accept-encoding", "gzip"),
            ("x-ABC-def", "2"),
            ("Connection", "keep-alive"),
        ])
        .unwrap();
        assert_eq!(
            get_request_bytes_with(&target(), &headers),
            b"GET /repos/x HTTP/1.1\r\nHost: api.github.com\r\nUser-Agent: Python-urllib/3.12\r\nX-Abc-Def: 2\r\nAccept-Encoding: gzip\r\nConnection: close\r\n\r\n"
        );
    }

    #[test]
    fn unsupported_header_text_is_refused_by_name() {
        assert!(RequestHeaders::new(&[("Bad Name", "x")]).is_err());
        assert!(RequestHeaders::new(&[("X", "a\r\nb")]).is_err());
        assert!(RequestHeaders::new(&[("X", "\u{e9}")]).is_err());
    }
}

#[cfg(test)]
mod request_line_tests {
    use super::*;

    fn refused(selector: &str, carrier: &SurrogateCarrier) -> Box<PyException> {
        check_request_line(selector, carrier).expect_err("the request line is refused")
    }

    fn message_of(selector: &str) -> String {
        refused(selector, &SurrogateCarrier::none())
            .message
            .to_utf8()
            .expect("text")
    }

    #[test]
    fn a_clean_selector_passes() {
        let none = SurrogateCarrier::none();
        assert!(check_request_line("/vllm-project/vllm/v9.9.9/pyproject.toml", &none).is_ok());
        assert!(check_request_line("/x?y=1%20z", &none).is_ok());
    }

    #[test]
    fn a_control_or_space_character_is_invalid_url_with_the_first_one_named() {
        assert_eq!(
            message_of("/a b/c"),
            "URL can't contain control characters. '/a b/c' (found at least ' ')"
        );
        assert_eq!(
            message_of("/a\tb\nc"),
            "URL can't contain control characters. '/a\\tb\\nc' (found at least '\\t')"
        );
        assert!(message_of("/a\u{7f}").ends_with("(found at least '\\x7f')"));
    }

    #[test]
    fn non_ascii_text_is_an_ascii_encode_error_at_its_request_line_position() {
        // "GET " is four characters, the selector starts at 4
        assert_eq!(
            message_of("/vllm-project/vllm/releases/v한글/pyproject.toml"),
            "'ascii' codec can't encode characters in position 33-34: ordinal not in range(128)"
        );
        assert_eq!(
            message_of("/\u{e9}"),
            "'ascii' codec can't encode character '\\xe9' in position 5: ordinal not in range(128)"
        );
        assert_eq!(
            message_of("/\u{1f600}"),
            "'ascii' codec can't encode character '\\U0001f600' in position 5: ordinal not in range(128)"
        );
    }

    #[test]
    fn the_exception_carries_the_whole_request_line_and_the_span_as_constructor_arguments() {
        let error = refused("/\u{e9}", &SurrogateCarrier::none());
        let object = PyValue::str("GET /\u{e9} HTTP/1.1");
        assert_eq!(error.class, "UnicodeEncodeError");
        assert_eq!(error.module, "builtins");
        let args: Vec<&PyValue> = error
            .args
            .iter()
            .map(|arg| match arg {
                crate::http_direct::failure::ErrorValue::Plain(value) => value,
                other => panic!("a plain argument, not {other:?}"),
            })
            .collect();
        assert_eq!(
            args,
            vec![
                &PyValue::str("ascii"),
                &object,
                &PyValue::int(5),
                &PyValue::int(6),
                &PyValue::str("ordinal not in range(128)")
            ]
        );
        let attributes: Vec<&str> = error.attributes.iter().map(|(name, _)| *name).collect();
        assert_eq!(attributes.len(), 5);
        for name in ["encoding", "object", "start", "end", "reason"] {
            assert!(attributes.contains(&name), "{name}");
        }
    }

    #[test]
    fn a_lone_surrogate_is_read_back_only_through_its_own_carrier() {
        let (carried, carrier) =
            SurrogateCarrier::carry(&[0x2F, 0xDCFF, 0x2F, 0xDCFF]).expect("representable");
        assert_eq!(
            refused(&carried, &carrier).message.to_utf8().unwrap(),
            "'ascii' codec can't encode character '\\udcff' in position 5: ordinal not in range(128)"
        );
        // the same text read through an empty carrier is just private-use Unicode: no surrogate is invented
        let genuine = refused(&carried, &SurrogateCarrier::none());
        assert!(!genuine.message.to_utf8().unwrap().contains("udcff"));
    }

    #[test]
    fn a_genuine_private_use_character_stays_itself_and_a_carried_text_never_collides_with_it() {
        let genuine = message_of("/\u{f0000}");
        assert_eq!(
            genuine,
            "'ascii' codec can't encode character '\\U000f0000' in position 5: ordinal not in range(128)"
        );
        // a text holding both a surrogate and U+F0000 gets a stand-in that is not U+F0000
        let (carried, carrier) = SurrogateCarrier::carry(&[0x2F, 0xF0000, 0xDCFF]).unwrap();
        let points: Vec<u32> = carried
            .chars()
            .map(|ch| carrier.code_point_of(ch))
            .collect();
        assert_eq!(points, vec![0x2F, 0xF0000, 0xDCFF]);
    }
}
