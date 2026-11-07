//! The TLS client session over a connected byte pipe, and the translation of its failures.
//!
//! The session is `rustls` with P's `tls_verify` verifier: certificate chain and host name verification are always on, the trusted
//! certificates are the CA sources the environment selects (`SSL_CERT_FILE`, `SSL_CERT_DIR`, else the host defaults), read when a connection
//! needs them. The logical server name is the host of the URL (or of the tunnel target), never the proxy.
//!
//! A refused certificate becomes the `SSLCertVerificationError` of CPython 3.12.3 for the three causes that were captured against the original:
//!
//! | `rustls` error | `verify_code` | `verify_message` |
//! |---|---|---|
//! | `UnknownIssuer` | 20 | `unable to get local issuer certificate` |
//! | name mismatch, host is an IP address | 64 | `IP address mismatch, certificate is not valid for '<host>'.` |
//! | name mismatch, host is a DNS name | 62 | `Hostname mismatch, certificate is not valid for '<host>'.` |
//!
//! The code and text come from the real TLS error and the real host; nothing is looked up by case. The sentence around the message is the text of
//! `_ssl.c` in the frozen interpreter profile (see `ssl_cert_verification_error`). Every other TLS failure (another certificate problem, an alert,
//! a protocol error, a close during the handshake) has no captured Python spelling and is returned as a [`TransportFailure`] of phase
//! `tls_handshake` (or `send`/`receive` once the session is up), so it is not mistaken for something else.

use crate::http_direct::failure::{
    out_of_scope, ssl_cert_verification_error, FetchFailure, Fetched, TransportFailure,
};
use crate::http_direct::wire::Wire;
use recipe_runtime::tls_trust;
use recipe_runtime::tls_verify::client_config;
use rustls::pki_types::ServerName;
use rustls::{CertificateError, ClientConnection, Error as TlsError, StreamOwned};
use std::fmt;
use std::io::{self, Read, Write};

const READ_CHUNK: usize = 16 * 1024;

/// `X509_V_ERR_UNABLE_TO_GET_ISSUER_CERT_LOCALLY` and the text of `X509_verify_cert_error_string`.
const UNABLE_TO_GET_LOCAL_ISSUER: (i64, &str) = (20, "unable to get local issuer certificate");
/// `X509_V_ERR_HOSTNAME_MISMATCH` and `X509_V_ERR_IP_ADDRESS_MISMATCH`.
const HOSTNAME_MISMATCH_CODE: i64 = 62;
const IP_ADDRESS_MISMATCH_CODE: i64 = 64;

/// A transport failure travelling through `rustls`' `io` plumbing, so the caller can tell it from a TLS error.
#[derive(Debug)]
struct WireError(TransportFailure);

impl fmt::Display for WireError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.0.phase, self.0.detail)
    }
}

impl std::error::Error for WireError {}

/// `Read`/`Write` over a [`Wire`]; bytes the wire delivered beyond the caller's buffer wait in `unread`.
struct WireIo {
    wire: Box<dyn Wire>,
    unread: Vec<u8>,
    consumed: usize,
}

impl Read for WireIo {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if self.consumed == self.unread.len() {
            self.unread.clear();
            self.consumed = 0;
            match self.wire.pull(&mut self.unread) {
                Ok(true) => {}
                Ok(false) => return Ok(0),
                Err(failure) => return Err(io::Error::other(WireError(failure))),
            }
        }
        let count = out.len().min(self.unread.len() - self.consumed);
        out[..count].copy_from_slice(&self.unread[self.consumed..self.consumed + count]);
        self.consumed += count;
        Ok(count)
    }
}

impl Write for WireIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.wire
            .send_all(bytes)
            .map_err(|failure| io::Error::other(WireError(failure)))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// A TLS session as the byte pipe the HTTP parser reads.
struct TlsWire {
    stream: StreamOwned<ClientConnection, WireIo>,
}

/// The transport failure inside an `io::Error`, or one of `phase` that carries the error's text.
fn transport_failure(error: &io::Error, phase: &'static str) -> TransportFailure {
    match error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<WireError>())
    {
        Some(WireError(failure)) => failure.clone(),
        None => TransportFailure {
            phase,
            detail: error.to_string(),
        },
    }
}

impl Wire for TlsWire {
    fn send_all(&mut self, bytes: &[u8]) -> Result<(), TransportFailure> {
        self.stream
            .write_all(bytes)
            .and_then(|()| self.stream.flush())
            .map_err(|error| transport_failure(&error, "send"))
    }

    fn pull(&mut self, sink: &mut Vec<u8>) -> Result<bool, TransportFailure> {
        let mut chunk = [0u8; READ_CHUNK];
        match self.stream.read(&mut chunk) {
            Ok(0) => Ok(false),
            Ok(count) => {
                sink.extend_from_slice(&chunk[..count]);
                Ok(true)
            }
            // A peer that closes without `close_notify` ends the stream, as `ssl.SSLSocket` does with `suppress_ragged_eofs`.
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(false),
            Err(error) => Err(transport_failure(&error, "receive")),
        }
    }
}

/// The certificate failure CPython reports for this `rustls` error, if it is one of the captured causes.
fn certificate_failure(error: &TlsError, host: &str, host_is_ip: bool) -> Option<FetchFailure> {
    let (code, message) = match error {
        TlsError::InvalidCertificate(CertificateError::UnknownIssuer) => (
            UNABLE_TO_GET_LOCAL_ISSUER.0,
            UNABLE_TO_GET_LOCAL_ISSUER.1.to_string(),
        ),
        TlsError::InvalidCertificate(
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
        ) if host_is_ip => (
            IP_ADDRESS_MISMATCH_CODE,
            format!("IP address mismatch, certificate is not valid for '{host}'."),
        ),
        TlsError::InvalidCertificate(
            CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. },
        ) => (
            HOSTNAME_MISMATCH_CODE,
            format!("Hostname mismatch, certificate is not valid for '{host}'."),
        ),
        _ => return None,
    };
    Some(ssl_cert_verification_error(code, &message).into())
}

/// The failure of a handshake step: a transport failure, a captured certificate failure, or an uncaptured TLS failure.
fn handshake_failure(error: &io::Error, host: &str, host_is_ip: bool) -> FetchFailure {
    let inner = error.get_ref();
    if let Some(WireError(failure)) = inner.and_then(|inner| inner.downcast_ref::<WireError>()) {
        return FetchFailure::Transport(failure.clone());
    }
    if let Some(captured) = inner
        .and_then(|inner| inner.downcast_ref::<TlsError>())
        .and_then(|tls| certificate_failure(tls, host, host_is_ip))
    {
        return captured;
    }
    FetchFailure::Transport(TransportFailure {
        phase: "tls_handshake",
        detail: error.to_string(),
    })
}

/// Runs the client handshake over `wire` for the logical server `host` and returns the encrypted pipe.
pub fn start(
    wire: Box<dyn Wire>,
    host: &str,
    trust: &tls_trust::Environment,
) -> Fetched<Box<dyn Wire>> {
    let name = ServerName::try_from(host.to_string()).map_err(|_| {
        out_of_scope(
            "tls_server_name",
            "the host is not a valid TLS server name (DNS name or IP address); the original's behaviour was not captured",
        )
    })?;
    let host_is_ip = matches!(name, ServerName::IpAddress(_));
    let config = client_config(tls_trust::load_store(&tls_trust::select_sources(trust)));
    let mut connection = ClientConnection::new(config, name).map_err(|error| {
        FetchFailure::Transport(TransportFailure {
            phase: "tls_handshake",
            detail: error.to_string(),
        })
    })?;
    let mut pipe = WireIo {
        wire,
        unread: Vec::new(),
        consumed: 0,
    };
    while connection.is_handshaking() {
        if let Err(error) = connection.complete_io(&mut pipe) {
            // `complete_io` returns at the first refused handshake message without writing the fatal alert rustls queued for it; the original's
            // client sends that alert (`unknown_ca` for an untrusted issuer) before it closes, and the server observes it on the wire
            while connection.wants_write() {
                if connection.write_tls(&mut pipe).is_err() {
                    break;
                }
            }
            return Err(handshake_failure(&error, host, host_is_ip));
        }
    }
    Ok(Box::new(TlsWire {
        stream: StreamOwned::new(connection, pipe),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::ErrorValue;
    use agent_runtime_support::pyvalue::PyValue;

    fn failure_of(error: TlsError, host: &str, host_is_ip: bool) -> Option<(i64, String)> {
        match certificate_failure(&error, host, host_is_ip)? {
            FetchFailure::Raised(exception) => {
                assert_eq!(exception.class, "SSLCertVerificationError");
                let Some((_, ErrorValue::Plain(PyValue::Int(code)))) = exception
                    .attributes
                    .iter()
                    .find(|(name, _)| *name == "verify_code")
                else {
                    panic!("verify_code is an int");
                };
                Some((
                    i64::try_from(code.clone()).expect("small"),
                    exception.message.to_utf8().unwrap(),
                ))
            }
            other => panic!("expected an exception, got {other:?}"),
        }
    }

    #[test]
    fn the_three_captured_causes_have_their_codes_and_texts() {
        let (code, text) = failure_of(
            TlsError::InvalidCertificate(CertificateError::UnknownIssuer),
            "127.0.0.1",
            true,
        )
        .unwrap();
        assert_eq!(code, 20);
        assert_eq!(
            text,
            "[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: unable to get local issuer certificate (_ssl.c:1000)"
        );
        let (code, text) = failure_of(
            TlsError::InvalidCertificate(CertificateError::NotValidForName),
            "127.0.0.1",
            true,
        )
        .unwrap();
        assert_eq!(code, 64);
        assert!(text.contains(
            "IP address mismatch, certificate is not valid for '127.0.0.1'. (_ssl.c:1000)"
        ));
        let (code, text) = failure_of(
            TlsError::InvalidCertificate(CertificateError::NotValidForName),
            "bad.example.invalid",
            false,
        )
        .unwrap();
        assert_eq!(code, 62);
        assert!(
            text.contains("Hostname mismatch, certificate is not valid for 'bad.example.invalid'.")
        );
    }

    #[test]
    fn other_tls_errors_are_not_given_a_python_spelling() {
        for error in [
            TlsError::InvalidCertificate(CertificateError::Expired),
            TlsError::InvalidCertificate(CertificateError::BadSignature),
            TlsError::DecryptError,
        ] {
            assert!(failure_of(error, "h", false).is_none());
        }
        let wrapped = io::Error::new(io::ErrorKind::InvalidData, TlsError::DecryptError);
        assert!(matches!(
            handshake_failure(&wrapped, "h", false),
            FetchFailure::Transport(TransportFailure {
                phase: "tls_handshake",
                ..
            })
        ));
    }

    #[test]
    fn a_transport_failure_keeps_its_phase_through_the_io_plumbing() {
        let failure = TransportFailure {
            phase: "receive",
            detail: "timed out".to_string(),
        };
        let wrapped = io::Error::other(WireError(failure.clone()));
        match handshake_failure(&wrapped, "h", false) {
            FetchFailure::Transport(kept) => assert_eq!(kept, failure),
            other => panic!("expected a transport failure, got {other:?}"),
        }
        assert_eq!(transport_failure(&wrapped, "send"), failure);
    }
}
