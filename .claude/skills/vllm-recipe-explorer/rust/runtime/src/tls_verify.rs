//! The TLS side of the HTTPS lookup: a certificate verifier and a connector that keep CPython 3.12's default chain rules.
//!
//! CPython's default context (`ssl.create_default_context()`, OpenSSL 3.0) sets `VERIFY_X509_TRUSTED_FIRST` and **not**
//! `VERIFY_X509_PARTIAL_CHAIN`. A certificate of the CA file or of a hashed directory is therefore a *candidate* while a chain is
//! built, but it may *end* the chain only when it is a trust anchor: here, self-signed in OpenSSL's sense
//! ([`crate::tls_trust::is_self_signed`]). An intermediate CA that is the only certificate configured is not enough: the chain
//! must still reach an anchor, or the connection is refused (`unable to get issuer certificate`).
//!
//! What this module does on every connection (there is no preflight and no cache of a verdict; session resumption is off, so
//! the verifier runs for each connection the HTTP client opens, redirects included):
//!
//! * the chain is built and verified by `rustls`/WebPKI (`WebPkiServerVerifier`: signatures, validity times, key usage, name
//!   constraints, host name or IP address of the certificate), with the store's self-signed certificates as the only trust
//!   anchors and the server's intermediates plus the store's other certificates as the candidate intermediates;
//! * the validity time of the trust anchor itself is checked too, as OpenSSL does (WebPKI does not check an anchor's dates): a
//!   path that ends at an expired or not yet valid anchor is refused with that cause;
//! * the TLS handshake signatures are verified by the same `rustls` provider.
//!
//! A refusal for an incomplete chain carries OpenSSL's own distinction (`ChainProblem`): the chain ended at a certificate that
//! came from the store (error 2, `unable to get issuer certificate`), at a self-signed certificate that is not trusted, or at a
//! certificate from the peer (error 20, `unable to get local issuer certificate`); it is decided by walking the chain the way
//! `build_chain` of `crypto/x509/x509_vfy.c` (OpenSSL 3.0.13) builds it, trusted store first. The verdict itself always comes
//! from the real verification above; the walk only names the cause of a refusal.
//!
//! The connector is adapted from `ureq` 3.5.0's `RustlsConnector` (`src/tls/rustls.rs`, MIT OR Apache-2.0; see
//! `notices/NOTICE.md`): `ureq` cannot be given a verifier of its own, so the same wrapping of a connected transport is done
//! here with a `rustls` client configuration that holds this module's verifier.
//!
//! Not implemented: explicit trust attributes of OpenSSL's `TRUSTED CERTIFICATE` format (a certificate marked as trusted or
//! rejected for a purpose), revocation (CPython's default does not check it), and OpenSSL's own `check_extensions` rules for
//! unusual extension combinations (the WebPKI rules apply instead). These are recorded in `PORTING.md`.

use crate::tls_trust::{
    is_self_signed, likely_issued, split_store, Facts, Store, StoreCertificate,
};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::client::{Resumption, WebPkiServerVerifier};
use rustls::crypto::CryptoProvider;
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{
    CertificateError, ClientConfig, ClientConnection, DigitallySignedStruct, Error as TlsError,
    OtherError, RootCertStore, SignatureScheme, StreamOwned, ALL_VERSIONS,
};
use std::fmt;
use std::io::{Read, Write};
use std::sync::Arc;
use ureq::unversioned::transport::{
    Buffers, ConnectionDetails, Connector, Either, LazyBuffers, NextTimeout, Transport,
    TransportAdapter,
};
use ureq::Error;

/// OpenSSL's verdict on a chain that could not be completed, for the causes that have an observed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChainProblem {
    /// Error 2: the chain ends at a certificate taken from the store, whose issuer is not there and which is not a trust
    /// anchor (`unable to get issuer certificate`).
    UnableToGetIssuer,
    /// The chain ends at a self-signed certificate that is not in the store (OpenSSL 18 or 19). Its text is not claimed.
    SelfSignedUntrusted,
}

impl fmt::Display for ChainProblem {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChainProblem::UnableToGetIssuer => {
                formatter.write_str("unable to get issuer certificate")
            }
            ChainProblem::SelfSignedUntrusted => {
                formatter.write_str("self-signed certificate not in the store")
            }
        }
    }
}

impl std::error::Error for ChainProblem {}

fn other(problem: ChainProblem) -> TlsError {
    TlsError::InvalidCertificate(CertificateError::Other(OtherError(Arc::new(problem))))
}

/// Walks the chain from `leaf` the way OpenSSL's `build_chain` does (trusted store first; once a store certificate has been
/// taken the peer's certificates are no longer used) and names the end it reaches. Only called for a chain that did not verify.
fn incomplete_chain(
    leaf: &Facts,
    peer: &[Facts],
    store: &Store,
    time: i64,
) -> Option<ChainProblem> {
    const MAX_DEPTH: usize = 100;
    let from_store: Vec<&Facts> = store
        .anchors
        .iter()
        .chain(store.others.iter())
        .map(|certificate| &certificate.facts)
        .collect();
    let mut chain: Vec<&Facts> = vec![leaf];
    let mut untrusted = 1usize;
    let mut store_only = false;
    let mut used_peer = vec![false; peer.len()];
    loop {
        let current = *chain.last()?;
        if chain.len() > MAX_DEPTH {
            break;
        }
        let in_chain = |candidate: &Facts| chain.iter().any(|held| std::ptr::eq(*held, candidate));
        // `find_issuer`: the first issuer that is not expired, else the one that expires last.
        let pick = |candidates: Vec<&'_ Facts>| -> Option<usize> {
            let mut fallback: Option<usize> = None;
            for (index, candidate) in candidates.iter().enumerate() {
                if !likely_issued(candidate, current) {
                    continue;
                }
                if candidate.not_before <= time && time <= candidate.not_after {
                    return Some(index);
                }
                if fallback.is_none_or(|held| candidates[held].not_after < candidate.not_after) {
                    fallback = Some(index);
                }
            }
            fallback
        };
        let trusted: Vec<&Facts> = from_store
            .iter()
            .copied()
            .filter(|candidate| !in_chain(candidate))
            .collect();
        if let Some(found) = pick(trusted.clone()) {
            let issuer = trusted[found];
            chain.push(issuer);
            store_only = true;
            if is_self_signed(issuer) {
                // The chain reached an anchor: nothing is incomplete (another check refused it).
                return None;
            }
            continue;
        }
        if !store_only {
            let candidates: Vec<&Facts> = peer
                .iter()
                .enumerate()
                .filter(|(index, _)| !used_peer[*index])
                .map(|(_, facts)| facts)
                .collect();
            let indexes: Vec<usize> = (0..peer.len()).filter(|index| !used_peer[*index]).collect();
            if !is_self_signed(current) {
                if let Some(found) = pick(candidates) {
                    used_peer[indexes[found]] = true;
                    chain.push(&peer[indexes[found]]);
                    untrusted += 1;
                    continue;
                }
            }
        }
        break;
    }
    let top = *chain.last()?;
    if is_self_signed(top) {
        Some(ChainProblem::SelfSignedUntrusted)
    } else if untrusted < chain.len() {
        Some(ChainProblem::UnableToGetIssuer)
    } else {
        None
    }
}

/// The verifier: [`WebPkiServerVerifier`] under the rules of the module description.
#[derive(Debug)]
pub struct StoreVerifier {
    provider: Arc<CryptoProvider>,
    store: Store,
}

impl StoreVerifier {
    pub fn new(provider: Arc<CryptoProvider>, store: Store) -> StoreVerifier {
        StoreVerifier { provider, store }
    }

    /// One real verification with the given anchors (and the candidate intermediates).
    fn verify_with(
        &self,
        anchors: &[&StoreCertificate],
        end_entity: &CertificateDer<'_>,
        candidates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let mut roots = RootCertStore::empty();
        for anchor in anchors {
            // A certificate WebPKI cannot use as an anchor is simply not one.
            let _ = roots.add(anchor.der.clone());
        }
        if roots.is_empty() {
            return Err(TlsError::InvalidCertificate(
                CertificateError::UnknownIssuer,
            ));
        }
        let verifier = WebPkiServerVerifier::builder_with_provider(
            Arc::new(roots),
            Arc::clone(&self.provider),
        )
        .build()
        .map_err(|error| TlsError::General(error.to_string()))?;
        verifier.verify_server_cert(end_entity, candidates, server_name, ocsp_response, now)
    }
}

impl ServerCertVerifier for StoreVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        intermediates: &[CertificateDer<'_>],
        server_name: &ServerName<'_>,
        ocsp_response: &[u8],
        now: UnixTime,
    ) -> Result<ServerCertVerified, TlsError> {
        let time = i64::try_from(now.as_secs()).unwrap_or(i64::MAX);
        // Candidate intermediates: what the server sent and the store's certificates that may not end a chain.
        let mut candidates: Vec<CertificateDer<'_>> = intermediates.to_vec();
        candidates.extend(
            self.store
                .others
                .iter()
                .map(|certificate| certificate.der.clone()),
        );
        let (valid, invalid): (Vec<&StoreCertificate>, Vec<&StoreCertificate>) =
            self.store.anchors.iter().partition(|anchor| {
                anchor.facts.not_before <= time && time <= anchor.facts.not_after
            });
        let refusal = match self.verify_with(
            &valid,
            end_entity,
            &candidates,
            server_name,
            ocsp_response,
            now,
        ) {
            Ok(verified) => return Ok(verified),
            Err(error) => error,
        };
        if !matches!(
            refusal,
            TlsError::InvalidCertificate(CertificateError::UnknownIssuer)
        ) {
            return Err(refusal);
        }
        // The chain may end at an anchor whose own dates are not valid: OpenSSL checks them, WebPKI does not.
        for anchor in invalid {
            let attempt = self.verify_with(
                &[anchor],
                end_entity,
                &candidates,
                server_name,
                ocsp_response,
                now,
            );
            if attempt.is_ok() {
                return Err(TlsError::InvalidCertificate(
                    if time > anchor.facts.not_after {
                        CertificateError::Expired
                    } else {
                        CertificateError::NotValidYet
                    },
                ));
            }
        }
        // No valid chain: name the end OpenSSL would have reached.
        let peer: Vec<Facts> = intermediates
            .iter()
            .filter_map(|der| crate::tls_trust::facts(der.as_ref()))
            .collect();
        match crate::tls_trust::facts(end_entity.as_ref()) {
            Some(leaf) => match incomplete_chain(&leaf, &peer, &self.store, time) {
                Some(problem) => Err(other(problem)),
                None => Err(refusal),
            },
            None => Err(refusal),
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls12_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        certificate: &CertificateDer<'_>,
        signature: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, TlsError> {
        rustls::crypto::verify_tls13_signature(
            message,
            certificate,
            signature,
            &self.provider.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider
            .signature_verification_algorithms
            .supported_schemes()
    }
}

/// A `rustls` client configuration that verifies with [`StoreVerifier`] over the given certificates; no client certificate, no
/// session resumption (every connection is verified).
pub fn client_config(certificates: Vec<CertificateDer<'static>>) -> Arc<ClientConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = StoreVerifier::new(Arc::clone(&provider), split_store(certificates));
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(ALL_VERSIONS)
        .expect("all TLS versions")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    config.resumption = Resumption::disabled();
    Arc::new(config)
}

/// Makes the client configuration for one actual TLS connection (it reads the CA sources then, not before).
pub type ConfigFactory = dyn Fn() -> Arc<ClientConfig> + Send + Sync + 'static;

/// Wraps a connected transport in TLS for an `https` request with this module's verifier (adapted from `ureq`'s
/// `RustlsConnector`). The configuration, and with it the trust store, is made by the factory **each time a connection really
/// needs TLS** (after the decision that it does): an `http` request that is redirected to `https` gets the process-selected CA
/// sources like any other `https` connection, a request that never needs TLS never reads them, and nothing about a verdict is kept
/// between connections.
pub struct TrustedConnector {
    factory: Arc<ConfigFactory>,
}

impl TrustedConnector {
    pub fn new(
        factory: impl Fn() -> Arc<ClientConfig> + Send + Sync + 'static,
    ) -> TrustedConnector {
        TrustedConnector {
            factory: Arc::new(factory),
        }
    }
}

impl fmt::Debug for TrustedConnector {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("TrustedConnector").finish()
    }
}

impl<In: Transport> Connector<In> for TrustedConnector {
    type Out = Either<In, TrustedTransport>;

    fn connect(
        &self,
        details: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, Error> {
        let Some(transport) = chained else {
            panic!("TrustedConnector requires a chained transport");
        };
        // Only for an https request on a transport that is not TLS already.
        if !details.needs_tls() || transport.is_tls() {
            return Ok(Some(Either::A(transport)));
        }
        let host = details
            .uri
            .authority()
            .map(|authority| {
                authority
                    .host()
                    .trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_string()
            })
            .ok_or(Error::Tls("no host for TLS"))?;
        let name = ServerName::try_from(host).map_err(|_| Error::Tls("invalid server name"))?;
        let mut connection = ClientConnection::new((self.factory)(), name)?;
        let mut socket = TransportAdapter::new(transport.boxed(), details.current_time.clone());
        socket.set_timeout(details.timeout);
        connection.complete_io(&mut socket)?;
        let stream = StreamOwned {
            conn: connection,
            sock: socket,
        };
        let buffers = LazyBuffers::new(
            details.config.input_buffer_size(),
            details.config.output_buffer_size(),
        );
        Ok(Some(Either::B(TrustedTransport { buffers, stream })))
    }
}

pub struct TrustedTransport {
    buffers: LazyBuffers,
    stream: StreamOwned<ClientConnection, TransportAdapter>,
}

impl Transport for TrustedTransport {
    fn buffers(&mut self) -> &mut dyn Buffers {
        &mut self.buffers
    }

    fn transmit_output(&mut self, amount: usize, timeout: NextTimeout) -> Result<(), Error> {
        self.stream.get_mut().set_timeout(timeout);
        let output = &self.buffers.output()[..amount];
        self.stream.write_all(output)?;
        Ok(())
    }

    fn await_input(&mut self, timeout: NextTimeout) -> Result<bool, Error> {
        self.stream.get_mut().set_timeout(timeout);
        let input = self.buffers.input_append_buf();
        let amount = self.stream.read(input)?;
        self.buffers.input_appended(amount);
        Ok(amount > 0)
    }

    fn is_open(&mut self) -> bool {
        self.stream.get_mut().get_mut().is_open()
    }

    fn is_tls(&self) -> bool {
        true
    }
}

impl fmt::Debug for TrustedTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TrustedTransport")
            .field("chained", &self.stream.sock.inner())
            .finish()
    }
}
