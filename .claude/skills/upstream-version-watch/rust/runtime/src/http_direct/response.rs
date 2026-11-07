//! `http.client.HTTPResponse` for one `GET` answered over a connection that is dropped afterwards (`Connection: close`): the status line,
//! the interim `100` responses, the header block, and the body by its framing (`Content-Length`, chunked, or until the peer closes).
//!
//! Ported from CPython 3.12.3 `Lib/http/client.py` (`_read_status`, `begin`, `read`, `_read_chunked`, `_get_chunk_left`,
//! `_read_next_chunk_size`, `_read_and_discard_trailer`, `_safe_read`, `_read_headers`). The connection is never reused, so the
//! keep-alive bookkeeping of the original (`will_close`) is not carried; a response that was read to its end is closed exactly as the
//! original's `_close_conn` does, and a closed response reads as empty.

use super::failure::{
    bad_status_line, http_exception, incomplete_read, line_too_long, remote_disconnected,
    unknown_protocol, value_error, FetchFailure, Fetched,
};
use super::headers::{parse_header_lines, HeaderBlock};
use super::py_int::{parse_int, IntOperand};
use super::text::{latin1, split_whitespace_max, strip};
use super::wire::BufferedReader;
use num_bigint::BigInt;
use num_traits::{Signed, Zero};

/// `http.client._MAXLINE`.
const MAX_LINE: usize = 65536;
/// `http.client._MAXHEADERS`.
const MAX_HEADERS: usize = 100;
/// `http.client._MIN_READ_BUF_SIZE`.
const MIN_READ_BUF_SIZE: i64 = 1 << 20;
const CONTINUE: u16 = 100;
const NO_CONTENT: u16 = 204;
const NOT_MODIFIED: u16 = 304;

/// `HTTPResponse.chunk_left`: `None` before the first chunk, `Remaining(0)` at the end of a chunk.
enum ChunkLeft {
    Unset,
    Remaining(BigInt),
}

/// A response whose status line and headers were read.
pub struct HttpResponse {
    reader: BufferedReader,
    pub status: u16,
    /// `HTTPResponse.reason` (stripped); `msg` of urllib is the same text.
    pub reason: String,
    pub headers: HeaderBlock,
    chunked: bool,
    chunk_left: ChunkLeft,
    /// Bytes left of a `Content-Length` body; `None` when the body ends with the connection.
    length: Option<BigInt>,
    /// `fp is not None`.
    open: bool,
}

impl HttpResponse {
    /// `HTTPResponse(sock).begin()` over the reader of an already sent request.
    pub fn begin(mut reader: BufferedReader) -> Fetched<HttpResponse> {
        let (version, status, reason) = loop {
            let (version, status, reason) = read_status(&mut reader)?;
            if status != CONTINUE {
                break (version, status, reason);
            }
            read_headers(&mut reader)?;
        };
        if version != "HTTP/1.0" && version != "HTTP/0.9" && !version.starts_with("HTTP/1.") {
            return Err(unknown_protocol(&version).into());
        }
        let headers = parse_header_lines(&read_headers(&mut reader)?);
        let chunked = headers
            .get("transfer-encoding")
            .is_some_and(|value| !value.is_empty() && value.eq_ignore_ascii_case("chunked"));
        let mut length = None;
        if let Some(text) = headers.get("content-length") {
            if !text.is_empty() && !chunked {
                if let Ok(number) = parse_int(IntOperand::Text(text), 10) {
                    if !number.is_negative() {
                        length = Some(number);
                    }
                }
            }
        }
        if status == NO_CONTENT || status == NOT_MODIFIED || (100..200).contains(&status) {
            length = Some(BigInt::zero());
        }
        Ok(HttpResponse {
            reader,
            status,
            reason: strip(&reason).to_string(),
            headers,
            chunked,
            chunk_left: if chunked {
                ChunkLeft::Unset
            } else {
                ChunkLeft::Remaining(BigInt::zero())
            },
            length,
            open: true,
        })
    }

    /// `HTTPResponse.read()`: the whole body.
    pub fn read_body(&mut self) -> Fetched<Vec<u8>> {
        if !self.open {
            return Ok(Vec::new());
        }
        if self.chunked {
            return self.read_chunked();
        }
        match self.length.clone() {
            None => {
                let body = self.reader.read(-1)?;
                self.open = false;
                Ok(body)
            }
            Some(length) => {
                let body = self.safe_read(&length);
                if matches!(&body, Err(FetchFailure::Raised(error)) if error.is_class("IncompleteRead"))
                {
                    self.open = false;
                }
                let body = body?;
                self.length = Some(BigInt::zero());
                self.open = false;
                Ok(body)
            }
        }
    }

    /// `HTTPResponse._safe_read(amt)`: exactly `amt` bytes or an `IncompleteRead` that carries what arrived.
    fn safe_read(&mut self, amount: &BigInt) -> Fetched<Vec<u8>> {
        let step_limit = BigInt::from(MIN_READ_BUF_SIZE);
        let mut current_size = amount.clone().min(step_limit);
        let mut data = self.reader.read(read_size(&current_size)?)?;
        if BigInt::from(data.len()) >= *amount {
            return Ok(data);
        }
        if BigInt::from(data.len()) < current_size {
            let missing = amount - BigInt::from(data.len());
            return Err(incomplete_read(data, Some(missing)).into());
        }
        loop {
            let delta = current_size.clone().min(amount - &current_size);
            data.extend(self.reader.read(read_size(&delta)?)?);
            if BigInt::from(data.len()) >= *amount {
                return Ok(data);
            }
            current_size += delta;
            if BigInt::from(data.len()) < current_size {
                let missing = amount - BigInt::from(data.len());
                return Err(incomplete_read(data, Some(missing)).into());
            }
        }
    }

    /// `HTTPResponse._read_chunked()` without a size limit.
    fn read_chunked(&mut self) -> Fetched<Vec<u8>> {
        let mut value = Vec::new();
        loop {
            let step = self
                .get_chunk_left()
                .and_then(|chunk_left| match chunk_left {
                    None => Ok(None),
                    Some(size) => {
                        let part = self.safe_read(&size)?;
                        Ok(Some(part))
                    }
                });
            match step {
                Ok(None) => return Ok(value),
                Ok(Some(part)) => {
                    value.extend(part);
                    self.chunk_left = ChunkLeft::Remaining(BigInt::zero());
                }
                Err(FetchFailure::Raised(error)) if error.is_class("IncompleteRead") => {
                    return Err(incomplete_read(value, None).caused_by(*error).into());
                }
                Err(other) => return Err(other),
            }
        }
    }

    /// `HTTPResponse._get_chunk_left()`: the bytes left in the current chunk, reading the next chunk header when one ended; `None` after the
    /// last chunk.
    fn get_chunk_left(&mut self) -> Fetched<Option<BigInt>> {
        let at_boundary = match &self.chunk_left {
            ChunkLeft::Unset => true,
            ChunkLeft::Remaining(size) => size.is_zero(),
        };
        if !at_boundary {
            return Ok(match &self.chunk_left {
                ChunkLeft::Remaining(size) => Some(size.clone()),
                ChunkLeft::Unset => None,
            });
        }
        if matches!(self.chunk_left, ChunkLeft::Remaining(_)) {
            self.safe_read(&BigInt::from(2))?;
        }
        let size = match self.read_next_chunk_size() {
            Ok(size) => size,
            Err(FetchFailure::Raised(error)) if error.is_class("ValueError") => {
                return Err(incomplete_read(Vec::new(), None).during(*error).into());
            }
            Err(other) => return Err(other),
        };
        if size.is_zero() {
            self.read_and_discard_trailer()?;
            self.open = false;
            self.chunk_left = ChunkLeft::Unset;
            return Ok(None);
        }
        self.chunk_left = ChunkLeft::Remaining(size.clone());
        Ok(Some(size))
    }

    /// `HTTPResponse._read_next_chunk_size()`: `int(line_without_extension, 16)`; the connection is closed when that fails.
    fn read_next_chunk_size(&mut self) -> Fetched<BigInt> {
        let line = self.reader.read_line(MAX_LINE + 1)?;
        if line.len() > MAX_LINE {
            return Err(line_too_long("chunk size", MAX_LINE).into());
        }
        let size_text = match line.iter().position(|&byte| byte == b';') {
            Some(at) => &line[..at],
            None => &line[..],
        };
        parse_int(IntOperand::Bytes(size_text), 16).map_err(|message| {
            self.open = false;
            FetchFailure::from(value_error(&message))
        })
    }

    /// `HTTPResponse._read_and_discard_trailer()`.
    fn read_and_discard_trailer(&mut self) -> Fetched<()> {
        loop {
            let line = self.reader.read_line(MAX_LINE + 1)?;
            if line.len() > MAX_LINE {
                return Err(line_too_long("trailer line", MAX_LINE).into());
            }
            if line.is_empty() || line == b"\r\n" || line == b"\n" {
                return Ok(());
            }
        }
    }
}

/// The size argument of `fp.read` for a value `_safe_read` computed (which is at most `_MIN_READ_BUF_SIZE`).
fn read_size(size: &BigInt) -> Fetched<i64> {
    i64::try_from(size.clone()).map_err(|_| {
        super::failure::out_of_scope(
            "framing",
            "a negative chunk size below the C ssize_t range raises OverflowError in the original; not modelled",
        )
    })
}

/// `HTTPResponse._read_status()`: `(version, status, reason)` of the next status line.
pub(crate) fn read_status(reader: &mut BufferedReader) -> Fetched<(String, u16, String)> {
    let raw = reader.read_line(MAX_LINE + 1)?;
    let line = latin1(&raw);
    if line.chars().count() > MAX_LINE {
        return Err(line_too_long("status line", MAX_LINE).into());
    }
    if line.is_empty() {
        return Err(remote_disconnected().into());
    }
    let (version, status_text, reason): (String, String, String) =
        match split_whitespace_max(&line, 2).as_slice() {
            [version, status, reason] => ((*version).into(), (*status).into(), (*reason).into()),
            _ => match split_whitespace_max(&line, 1).as_slice() {
                [version, status] => ((*version).into(), (*status).into(), String::new()),
                _ => (String::new(), String::new(), String::new()),
            },
        };
    if !version.starts_with("HTTP/") {
        return Err(bad_status_line(&line).into());
    }
    let status = match parse_int(IntOperand::Text(&status_text), 10) {
        Ok(number) => {
            if number < BigInt::from(100) || number > BigInt::from(999) {
                return Err(bad_status_line(&line).into());
            }
            u16::try_from(number).expect("checked to be three digits")
        }
        Err(message) => {
            return Err(bad_status_line(&line).during(value_error(&message)).into());
        }
    };
    Ok((version, status, reason))
}

/// `http.client._read_headers(fp)`: the raw header lines up to and including the blank line.
pub(crate) fn read_headers(reader: &mut BufferedReader) -> Fetched<Vec<Vec<u8>>> {
    let mut lines: Vec<Vec<u8>> = Vec::new();
    loop {
        let line = reader.read_line(MAX_LINE + 1)?;
        if line.len() > MAX_LINE {
            return Err(line_too_long("header line", MAX_LINE).into());
        }
        let ends_block = line.is_empty() || line == b"\r\n" || line == b"\n";
        lines.push(line);
        if lines.len() > MAX_HEADERS {
            return Err(http_exception(&format!("got more than {MAX_HEADERS} headers")).into());
        }
        if ends_block {
            return Ok(lines);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::failure::PyException;
    use super::super::wire::testing::Scripted;
    use super::*;

    fn response(raw: &[&[u8]]) -> Fetched<HttpResponse> {
        HttpResponse::begin(Scripted::reader(raw))
    }

    fn raised<T>(outcome: Fetched<T>) -> PyException {
        match outcome {
            Err(FetchFailure::Raised(error)) => *error,
            Ok(_) => panic!("expected an exception"),
            Err(other) => panic!("expected an exception, got {other:?}"),
        }
    }

    #[test]
    fn status_line_forms_follow_http_client() {
        let ok = response(&[b"HTTP/1.1 200\r\nContent-Length: 0\r\n\r\n"]).unwrap();
        assert_eq!((ok.status, ok.reason.as_str()), (200, ""));
        let plus = response(&[b"HTTP/1.1 +200 Custom \r\n\r\n"]).unwrap();
        assert_eq!((plus.status, plus.reason.as_str()), (200, "Custom"));
        assert_eq!(
            raised(response(&[b"HTTP/1.1 099 x\r\n\r\n"])).class,
            "BadStatusLine"
        );
        assert_eq!(
            raised(response(&[b"HTTP/2.0 200 x\r\n\r\n"])).class,
            "UnknownProtocol"
        );
        assert_eq!(raised(response(&[b""])).class, "RemoteDisconnected");
        let letters = raised(response(&[b"HTTP/1.1 ABC nope\r\n\r\n"]));
        assert_eq!(letters.class, "BadStatusLine");
        assert!(letters.link.is_some());
        assert!(raised(response(&[b"hello\nworld"])).link.is_none());
    }

    #[test]
    fn interim_continue_is_skipped_and_content_length_is_not_read_past() {
        let mut parsed = response(&[
            b"HTTP/1.1 100 Continue\r\nX: 1\r\n\r\nHTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello",
            b"never pulled",
        ])
        .unwrap();
        assert_eq!(parsed.status, 200);
        assert_eq!(parsed.read_body().unwrap(), b"hello");
        assert_eq!(parsed.read_body().unwrap(), b"");
    }

    #[test]
    fn body_framings() {
        let body = |raw: &[u8]| response(&[raw]).and_then(|mut parsed| parsed.read_body());
        assert_eq!(
            body(b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5;e=1\r\nhello\r\n0\r\nX-T: v\r\n\r\n").unwrap(),
            b"hello"
        );
        assert_eq!(
            body(b"HTTP/1.1 200 OK\r\n\r\nclose-delimited").unwrap(),
            b"close-delimited"
        );
        assert_eq!(
            body(b"HTTP/1.1 204 No Content\r\nContent-Length: 8\r\n\r\nignored?").unwrap(),
            b""
        );
        assert_eq!(
            body(b"HTTP/1.1 200 OK\r\nContent-Length: nope\r\n\r\nabc").unwrap(),
            b"abc"
        );
        assert_eq!(
            body(b"HTTP/1.1 200 OK\r\nContent-Length: -1\r\n\r\nabc").unwrap(),
            b"abc"
        );
        let truncated = raised(body(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\nabc"));
        assert_eq!(
            truncated.message.to_utf8().unwrap(),
            "IncompleteRead(3 bytes read, 7 more expected)"
        );
    }

    #[test]
    fn chunk_failures_chain_like_the_original() {
        let body = |raw: &[u8]| response(&[raw]).and_then(|mut parsed| parsed.read_body());
        let missing_zero = raised(body(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n",
        ));
        assert_eq!(
            missing_zero.message.to_utf8().unwrap(),
            "IncompleteRead(5 bytes read)"
        );
        assert!(matches!(
            missing_zero.link,
            Some(super::super::failure::Link::Cause(_))
        ));
        let non_hex = raised(body(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nNOPE\r\nhello\r\n0\r\n\r\n",
        ));
        assert_eq!(
            non_hex.message.to_utf8().unwrap(),
            "IncompleteRead(0 bytes read)"
        );
        let negative = raised(body(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n-5\r\nhello\r\n",
        ));
        assert_eq!(negative.class, "ValueError");
    }

    #[test]
    fn header_limits() {
        let mut many = b"HTTP/1.1 200 OK\r\n".to_vec();
        for number in 0..100 {
            many.extend(format!("X-{number}: v\r\n").as_bytes());
        }
        many.extend(b"\r\n");
        assert_eq!(
            raised(response(&[&many])).message.to_utf8().unwrap(),
            "got more than 100 headers"
        );
        let mut long = b"HTTP/1.1 200 OK\r\nX-Long: ".to_vec();
        long.extend(std::iter::repeat_n(b'a', 70000));
        long.extend(b"\r\n\r\n");
        assert_eq!(raised(response(&[&long])).class, "LineTooLong");
    }

    #[test]
    fn content_length_with_a_separator_control_is_ignored_and_the_body_ends_with_the_connection() {
        // N1: `int("\x1c1")` is a ValueError, so the header is ignored and the close-delimited body is read whole.
        for control in [0x1cu8, 0x1d, 0x1e, 0x1f] {
            let mut raw = b"HTTP/1.1 200 OK\r\nContent-Length: ".to_vec();
            raw.push(control);
            raw.extend_from_slice(b"1\r\nConnection: close\r\n\r\nab");
            let mut parsed = response(&[&raw]).unwrap();
            assert_eq!(parsed.read_body().unwrap(), b"ab", "control {control:#x}");
        }
        // a vertical tab (C whitespace) and a non-breaking space (Unicode whitespace of the text transform) are accepted: length 1
        for blank in [&b"\x0b"[..], &b"\xa0"[..]] {
            let mut raw = b"HTTP/1.1 200 OK\r\nContent-Length: ".to_vec();
            raw.extend_from_slice(blank);
            raw.extend_from_slice(b"1\r\nConnection: close\r\n\r\nab");
            let mut parsed = response(&[&raw]).unwrap();
            assert_eq!(parsed.read_body().unwrap(), b"a");
        }
    }

    #[test]
    fn long_invalid_numbers_keep_the_cut_value_error_in_the_chain() {
        // N2, chunk size: 201 bytes below `_MAXLINE`
        let mut raw = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec();
        raw.extend(std::iter::repeat_n(b'G', 201));
        raw.extend_from_slice(b"\r\n");
        let mut parsed = response(&[&raw]).unwrap();
        let top = raised(parsed.read_body());
        let Some(super::super::failure::Link::Cause(middle)) = top.link else {
            panic!("expected the cause");
        };
        let Some(super::super::failure::Link::Context(inner)) = middle.link else {
            panic!("expected the context");
        };
        assert_eq!(inner.class, "ValueError");
        assert_eq!(inner.message.len(), 240);
        assert_eq!(inner.args.len(), 1);
        // N2, status: a 200-character field
        let status = "G".repeat(219);
        let raw = format!("HTTP/1.1 {status} reason\r\n\r\n");
        let bad = raised(response(&[raw.as_bytes()]));
        assert_eq!(bad.class, "BadStatusLine");
        // the status line itself is kept whole in `BadStatusLine`: "HTTP/1.1 " + 219 + " reason\r\n"
        assert_eq!(bad.message.len(), 237);
        let Some(super::super::failure::Link::Context(value)) = bad.link else {
            panic!("expected the context");
        };
        assert_eq!(value.message.len(), 240);
    }
}
