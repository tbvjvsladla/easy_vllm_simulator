//! The HTTP `CONNECT` leg of an `https` request through a proxy: `HTTPConnection._tunnel` of CPython 3.12.3.
//!
//! The request is `CONNECT <host>:<port> HTTP/1.1` and one `Host: <host>:<port>` header (`set_tunnel` adds it when the caller gave none; no
//! other header is sent without proxy credentials). The reply is read like a response head by `http.client` (`_read_status`,
//! `_read_headers`); anything but status 200 is `OSError("Tunnel connection failed: <code> <reason>")`. Bytes the proxy sent after its head are
//! dropped, as CPython drops the buffer of the file object it closes.

use crate::http_direct::failure::{os_error, out_of_scope, Fetched};
use crate::http_direct::response::{read_headers, read_status};
use crate::http_direct::text::strip;
use crate::http_direct::wire::{BufferedReader, Wire};

const MAX_LABEL_LENGTH: usize = 63;

/// `HTTPConnection._tunnel`'s `connect` and header lines for `host:port`, with `host` as `str.encode("idna")` writes an ASCII name.
pub fn connect_request_bytes(host: &str, port: u16) -> Fetched<Vec<u8>> {
    if !host.is_ascii()
        || host
            .split('.')
            .any(|label| label.is_empty() || label.len() > MAX_LABEL_LENGTH)
    {
        return Err(out_of_scope(
            "url_form",
            "a host with an empty or over-long label makes the idna codec raise UnicodeError; not captured",
        ));
    }
    Ok(format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\n\r\n").into_bytes())
}

/// Asks the proxy behind `wire` for a tunnel to `host:port`; returns the same connection, now the tunnel.
pub fn establish(wire: Box<dyn Wire>, host: &str, port: u16) -> Fetched<Box<dyn Wire>> {
    let request = connect_request_bytes(host, port)?;
    let mut reader = BufferedReader::new(wire);
    reader.send_all(&request)?;
    let (_version, status, reason) = read_status(&mut reader)?;
    read_headers(&mut reader)?;
    if status != 200 {
        return Err(os_error(&format!(
            "Tunnel connection failed: {status} {}",
            strip(&reason)
        ))
        .into());
    }
    Ok(reader.into_wire())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::failure::FetchFailure;
    use crate::http_direct::wire::testing::Scripted;

    #[test]
    fn the_connect_head_is_the_captured_plaintext() {
        assert_eq!(
            connect_request_bytes("api.github.com", 443).unwrap(),
            b"CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n\r\n"
        );
        assert!(connect_request_bytes("a..b", 443).is_err());
        assert!(connect_request_bytes(&"x".repeat(64), 443).is_err());
    }

    fn outcome(reply: &[u8]) -> Fetched<Box<dyn Wire>> {
        let scripted = Scripted::reader(&[reply]);
        establish(scripted.into_wire(), "h", 443)
    }

    fn raised(reply: &[u8]) -> crate::http_direct::PyException {
        match outcome(reply) {
            Err(FetchFailure::Raised(error)) => *error,
            Err(other) => panic!("expected an exception, got {other:?}"),
            Ok(_) => panic!("expected a failure"),
        }
    }

    #[test]
    fn only_status_200_opens_the_tunnel() {
        assert!(outcome(b"HTTP/1.1 200 Connection Established\r\n\r\n").is_ok());
        assert!(outcome(b"HTTP/1.0 200 OK\r\nVia: x\r\n\r\n").is_ok());
        let refusal = raised(b"HTTP/1.1 403 Forbidden\r\n\r\n");
        assert_eq!(refusal.class, "OSError");
        assert_eq!(
            refusal.message.to_utf8().unwrap(),
            "Tunnel connection failed: 403 Forbidden"
        );
        assert_eq!(
            raised(b"HTTP/1.1 204\r\n\r\n").message.to_utf8().unwrap(),
            "Tunnel connection failed: 204 "
        );
    }

    #[test]
    fn a_damaged_reply_raises_the_http_client_exception() {
        assert_eq!(raised(b"not HTTP\r\n\r\n").class, "BadStatusLine");
        assert_eq!(raised(b"").class, "RemoteDisconnected");
    }
}
