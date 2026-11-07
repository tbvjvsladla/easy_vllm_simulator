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

/// The request head of a `GET` to `target`.
pub fn get_request_bytes(target: &HttpTarget) -> Vec<u8> {
    format!(
        "GET {} HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: {}\r\nUser-Agent: {USER_AGENT}\r\nConnection: close\r\n\r\n",
        target.selector, target.host_header
    )
    .into_bytes()
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
            get_request_bytes(&target),
            b"GET /x HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: 127.0.0.1:8080\r\nUser-Agent: Python-urllib/3.12\r\nConnection: close\r\n\r\n"
        );
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
