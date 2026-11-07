//! The real byte transport: `ureq` 3.5.0's connector chain (TCP and the CONNECT-proxy pass-through) driven as a raw pipe,
//! without `ureq`'s HTTP engine (which drops the reason phrase, frames responses its own way and rewrites headers).
//!
//! Types are those proven by the stage-B0 compile-only study (`ConnectionDetails` built outside an `Agent`, the two private closure aliases
//! spelled out, `Transport`/`Buffers` as a byte pipe). What is new here is the runtime use: every `pull` asks the transport for
//! input (`await_input`), copies what is buffered and consumes exactly that (including the zero-length consume that keeps `ureq`'s
//! progress flag honest), and reports end of stream when the transport made no progress and holds nothing.
//!
//! The chain is plain TCP only. TLS is not done here: the HTTPS fetch (`crate::fetch_https`) puts a `rustls` session over the [`Wire`] this
//! dialer returns, after its own CONNECT exchange, so that the wire bytes of the tunnel and the handshake stay under that component's control.

use super::failure::TransportFailure;
use super::wire::{Dial, Wire};
use std::sync::{Arc, Weak};
use ureq::config::Config;
use ureq::http::Uri;
use ureq::unversioned::resolver::{DefaultResolver, ResolvedSocketAddrs, Resolver};
use ureq::unversioned::transport::time::{Duration, Instant};
use ureq::unversioned::transport::{
    Buffers, ConnectProxyConnector, ConnectionDetails, Connector, NextTimeout, ShutdownHandle,
    TcpConnector, Transport,
};
use ureq::{Error, Timeout};

/// `urlopen(url, timeout=20)` of the original: a per-socket-operation timeout, not a deadline for the whole fetch.
pub const ORIGINAL_SOCKET_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(20);

/// External spelling of `ureq`'s private `RunConnector` alias.
type RunConnectorFn = dyn Fn(&ConnectionDetails) -> Result<Box<dyn Transport>, Error> + Send + Sync;
/// External spelling of `ureq`'s private `RegisterShutdown` alias.
type RegisterShutdownFn = dyn Fn(Weak<dyn ShutdownHandle>) -> Result<(), Error> + Send + Sync;
/// A connector chain with its output erased.
type BoxedChain = dyn Connector<(), Out = Box<dyn Transport>>;

/// External equivalent of `ureq`'s private boxing connector.
#[derive(Debug)]
struct BoxingConnector;

impl<In: Transport> Connector<In> for BoxingConnector {
    type Out = Box<dyn Transport>;

    fn connect(
        &self,
        _: &ConnectionDetails,
        chained: Option<In>,
    ) -> Result<Option<Self::Out>, Error> {
        Ok(chained.map(|transport| transport.boxed()))
    }
}

/// The chain of `ureq`'s default connector without its TLS stage (and without SOCKS, which the locked features do not have).
fn build_chain() -> Arc<BoxedChain> {
    Arc::new(
        ().chain(ConnectProxyConnector::default())
            .chain(TcpConnector::default())
            .chain(BoxingConnector),
    )
}

/// The `Config` of a direct fetch: no proxy (not even the one `ureq` would read from its own environment variables), no HTTP-status
/// handling, and the original's 20 s per connect, read and write.
fn direct_config() -> Config {
    Config::builder()
        .proxy(None)
        .http_status_as_error(false)
        .timeout_connect(Some(ORIGINAL_SOCKET_TIMEOUT))
        .timeout_per_read(Some(ORIGINAL_SOCKET_TIMEOUT))
        .timeout_per_write(Some(ORIGINAL_SOCKET_TIMEOUT))
        .build()
}

fn connect_failure(error: &Error) -> TransportFailure {
    TransportFailure {
        phase: "connect",
        detail: error.to_string(),
    }
}

/// Dials plain TCP connections through the `ureq` chain.
pub struct UreqDialer {
    chain: Arc<BoxedChain>,
    resolver: DefaultResolver,
    config: Config,
}

impl Default for UreqDialer {
    fn default() -> Self {
        UreqDialer::new()
    }
}

impl UreqDialer {
    pub fn new() -> UreqDialer {
        UreqDialer {
            chain: build_chain(),
            resolver: DefaultResolver::default(),
            config: direct_config(),
        }
    }

    fn resolve(&self, uri: &Uri) -> Result<ResolvedSocketAddrs, Error> {
        let timeout = NextTimeout {
            after: self
                .config
                .timeouts()
                .resolve
                .map(Duration::Exact)
                .unwrap_or(Duration::NotHappening),
            reason: Timeout::Resolve,
            per_read: None,
            per_write: None,
        };
        self.resolver.resolve(uri, &self.config, timeout)
    }

    fn connect_timeout(&self) -> NextTimeout {
        NextTimeout {
            after: self
                .config
                .timeouts()
                .connect
                .map(Duration::Exact)
                .unwrap_or(Duration::NotHappening),
            reason: Timeout::Connect,
            per_read: self.config.timeout_per_read().map(Duration::Exact),
            per_write: self.config.timeout_per_write().map(Duration::Exact),
        }
    }

    fn open(&self, uri: &Uri) -> Result<Box<dyn Transport>, Error> {
        let chain = self.chain.clone();
        let run_connector: Arc<RunConnectorFn> = Arc::new(move |details: &ConnectionDetails| {
            chain.connect(details, None)?.ok_or(Error::ConnectionFailed)
        });
        // Nothing cancels a fetch (the original has no cancellation either), so the handle registration is accepted and dropped.
        let register_shutdown: Arc<RegisterShutdownFn> = Arc::new(|_handle| Ok(()));
        let details = ConnectionDetails {
            uri,
            addrs: self.resolve(uri)?,
            config: &self.config,
            request_level: false,
            resolver: &self.resolver,
            now: Instant::now(),
            timeout: self.connect_timeout(),
            current_time: Arc::new(Instant::now),
            run_connector,
            register_shutdown,
        };
        self.chain
            .connect(&details, None)?
            .ok_or(Error::ConnectionFailed)
    }
}

impl Dial for UreqDialer {
    fn dial(&self, host: &str, port: u16) -> Result<Box<dyn Wire>, TransportFailure> {
        let authority = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.to_string()
        };
        let uri: Uri = format!("http://{authority}:{port}/").parse().map_err(
            |error: ureq::http::uri::InvalidUri| TransportFailure {
                phase: "connect",
                detail: error.to_string(),
            },
        )?;
        let transport = self.open(&uri).map_err(|error| connect_failure(&error))?;
        Ok(Box::new(UreqWire {
            transport,
            timeout: NextTimeout {
                after: Duration::NotHappening,
                reason: Timeout::Global,
                per_read: self.config.timeout_per_read().map(Duration::Exact),
                per_write: self.config.timeout_per_write().map(Duration::Exact),
            },
        }))
    }
}

/// An open transport used as a raw pipe.
struct UreqWire {
    transport: Box<dyn Transport>,
    timeout: NextTimeout,
}

impl Wire for UreqWire {
    fn send_all(&mut self, bytes: &[u8]) -> Result<(), TransportFailure> {
        let mut rest = bytes;
        while !rest.is_empty() {
            let output = self.transport.buffers().output();
            let chunk = rest.len().min(output.len());
            output[..chunk].copy_from_slice(&rest[..chunk]);
            self.transport
                .transmit_output(chunk, self.timeout)
                .map_err(|error| TransportFailure {
                    phase: "send",
                    detail: error.to_string(),
                })?;
            rest = &rest[chunk..];
        }
        Ok(())
    }

    fn pull(&mut self, sink: &mut Vec<u8>) -> Result<bool, TransportFailure> {
        self.transport
            .await_input(self.timeout)
            .map_err(|error| TransportFailure {
                phase: "receive",
                detail: error.to_string(),
            })?;
        let buffers: &mut dyn Buffers = self.transport.buffers();
        let available = buffers.input().len();
        sink.extend_from_slice(buffers.input());
        buffers.input_consume(available);
        Ok(available > 0)
    }
}
