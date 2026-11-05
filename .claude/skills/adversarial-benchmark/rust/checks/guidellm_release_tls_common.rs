//! Runtime TLS fixtures of the GuideLLM release checks (included with `#[path]`; not a test target): a temporary CA and a
//! leaf certificate for `api.github.com` made with the `openssl` command (`NOT_RUN` panic when it is absent), and a
//! loopback server that speaks `CONNECT` then TLS (rustls) and answers with a chosen reply. Nothing here is global: the
//! files live in a scratch folder, and the server never resolves or forwards the host.

#![allow(dead_code)]

use rustls::pki_types::{pem::PemObject, CertificateDer, PrivateKeyDer};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub const SERVER_PATIENCE: Duration = Duration::from_secs(25);
pub const CLIENT_PATIENCE: Duration = Duration::from_secs(40);

pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new(label: &str) -> Scratch {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let root = std::env::temp_dir().join(format!(
            "guidellm-tls-check-{}-{id}-{label}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("scratch");
        Scratch(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct Authority {
    pub ca_pem: PathBuf,
    pub leaf_pem: PathBuf,
    pub leaf_key: PathBuf,
}

fn openssl(folder: &Path, arguments: &[&str]) {
    let output = Command::new("openssl")
        .args(arguments)
        .current_dir(folder)
        .output()
        .unwrap_or_else(|error| {
            panic!("NOT_RUN: the openssl command is needed to create a temporary CA: {error}")
        });
    assert!(
        output.status.success(),
        "openssl {arguments:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A temporary CA and a leaf certificate for `api.github.com` signed by it, in `folder`.
pub fn authority(folder: &Path, name: &str) -> Authority {
    std::fs::create_dir_all(folder).expect("folder");
    let subject = format!("/CN={name}-CA");
    openssl(
        folder,
        &[
            "req", "-x509", "-newkey", "rsa:2048", "-sha256", "-days", "2", "-nodes", "-keyout",
            "ca.key", "-out", "ca.pem", "-subj", &subject,
        ],
    );
    openssl(
        folder,
        &[
            "req",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-keyout",
            "leaf.key",
            "-out",
            "leaf.csr",
            "-subj",
            "/CN=api.github.com",
        ],
    );
    std::fs::write(
        folder.join("extensions.cnf"),
        "subjectAltName=DNS:api.github.com\nextendedKeyUsage=serverAuth\n",
    )
    .expect("extensions");
    openssl(
        folder,
        &[
            "x509",
            "-req",
            "-in",
            "leaf.csr",
            "-CA",
            "ca.pem",
            "-CAkey",
            "ca.key",
            "-CAcreateserial",
            "-out",
            "leaf.pem",
            "-days",
            "2",
            "-sha256",
            "-extfile",
            "extensions.cnf",
        ],
    );
    Authority {
        ca_pem: folder.join("ca.pem"),
        leaf_pem: folder.join("leaf.pem"),
        leaf_key: folder.join("leaf.key"),
    }
}

pub fn server_config(authority: &Authority) -> Arc<rustls::ServerConfig> {
    let certificates: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(&authority.leaf_pem)
            .expect("leaf certificate")
            .collect::<Result<_, _>>()
            .expect("certificate");
    let key = PrivateKeyDer::from_pem_file(&authority.leaf_key).expect("leaf key");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("protocol versions")
            .with_no_client_auth()
            .with_single_cert(certificates, key)
            .expect("server certificate"),
    )
}

/// What the loopback server saw: the tunnel request and the request inside the tunnel.
#[derive(Debug, Default)]
pub struct Seen {
    pub connect: String,
    pub request_line: String,
    pub headers: Vec<(String, String)>,
}

fn read_head(stream: &mut dyn Read) -> Result<String, String> {
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            Ok(_) => return Err("closed before the end of the headers".into()),
            Err(error) => return Err(error.to_string()),
        }
        if head.len() > 65536 {
            return Err("header limit".into());
        }
    }
    Ok(String::from_utf8_lossy(&head).into_owned())
}

/// What the server answers inside the tunnel.
pub enum Reply {
    /// A complete answer with a correct `Content-Length`.
    Complete {
        status_line: &'static str,
        body: Vec<u8>,
    },
    /// A `200` whose `Content-Length` is `extra` bytes more than the body it sends before closing.
    Short { body: Vec<u8>, extra: usize },
    /// A chunked `200` that sends one chunk and closes the connection before the next chunk header.
    ChunkedThenClose { chunk: Vec<u8> },
}

pub fn serve_once(
    listener: TcpListener,
    config: Arc<rustls::ServerConfig>,
    reply: Reply,
) -> std::thread::JoinHandle<Result<Seen, String>> {
    listener.set_nonblocking(true).expect("nonblocking");
    std::thread::spawn(move || {
        let started = Instant::now();
        let mut tcp: TcpStream = loop {
            match listener.accept() {
                Ok((tcp, _)) => break tcp,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if started.elapsed() > SERVER_PATIENCE {
                        return Err("no client connected".into());
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
                Err(error) => return Err(error.to_string()),
            }
        };
        tcp.set_nonblocking(false).expect("blocking");
        tcp.set_read_timeout(Some(SERVER_PATIENCE))
            .expect("timeout");
        tcp.set_write_timeout(Some(SERVER_PATIENCE))
            .expect("timeout");
        let connect = read_head(&mut tcp)?;
        let connect = connect.lines().next().unwrap_or("").to_string();
        if connect != "CONNECT api.github.com:443 HTTP/1.1"
            && connect != "CONNECT api.github.com:443 HTTP/1.0"
        {
            return Err(format!("not the fixed upstream host: {connect}"));
        }
        tcp.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n")
            .map_err(|e| e.to_string())?;
        let mut connection = rustls::ServerConnection::new(config).map_err(|e| e.to_string())?;
        let mut tls = rustls::Stream::new(&mut connection, &mut tcp);
        let head = read_head(&mut tls)?;
        let mut lines = head.split("\r\n");
        let request_line = lines.next().unwrap_or("").to_string();
        let headers = lines
            .filter_map(|line| line.split_once(':'))
            .map(|(name, value)| (name.to_lowercase(), value.trim().to_string()))
            .collect();
        match reply {
            Reply::Complete { status_line, body } => {
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                tls.write_all(response.as_bytes())
                    .map_err(|e| e.to_string())?;
                tls.write_all(&body).map_err(|e| e.to_string())?;
            }
            Reply::Short { body, extra } => {
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len() + extra
                );
                tls.write_all(response.as_bytes())
                    .map_err(|e| e.to_string())?;
                tls.write_all(&body).map_err(|e| e.to_string())?;
            }
            Reply::ChunkedThenClose { chunk } => {
                tls.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n")
                    .map_err(|e| e.to_string())?;
                tls.write_all(format!("{:x}\r\n", chunk.len()).as_bytes())
                    .map_err(|e| e.to_string())?;
                tls.write_all(&chunk).map_err(|e| e.to_string())?;
                tls.write_all(b"\r\n").map_err(|e| e.to_string())?;
            }
        }
        tls.flush().map_err(|e| e.to_string())?;
        connection.send_close_notify();
        let _ = connection.write_tls(&mut tcp);
        Ok(Seen {
            connect,
            request_line,
            headers,
        })
    })
}

pub fn header<'a>(seen: &'a Seen, name: &str) -> Option<&'a str> {
    seen.headers
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.as_str())
}

/// `openssl x509 -hash -noout`: the subject hash OpenSSL names a directory entry with (test set-up only; the production
/// code computes the hash itself, which the checks compare with this).
pub fn openssl_subject_hash(certificate_pem: &Path) -> String {
    let output = Command::new("openssl")
        .args(["x509", "-hash", "-noout", "-in"])
        .arg(certificate_pem)
        .output()
        .unwrap_or_else(|error| panic!("NOT_RUN: the openssl command is needed: {error}"));
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("hash text")
        .trim()
        .to_string()
}

/// A throw-away self-signed certificate for `subject` (an `openssl req -subj` string) in `folder`.
pub fn self_signed(folder: &Path, name: &str, subject: &str) -> PathBuf {
    std::fs::create_dir_all(folder).expect("folder");
    let pem = folder.join(format!("{name}.pem"));
    let output = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "ec",
            "-pkeyopt",
            "ec_paramgen_curve:prime256v1",
            "-sha256",
            "-days",
            "2",
            "-nodes",
            "-utf8",
            "-keyout",
        ])
        .arg(folder.join(format!("{name}.key")))
        .arg("-out")
        .arg(&pem)
        .args(["-subj", subject])
        .output()
        .unwrap_or_else(|error| panic!("NOT_RUN: the openssl command is needed: {error}"));
    assert!(
        output.status.success(),
        "{subject}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    pem
}
