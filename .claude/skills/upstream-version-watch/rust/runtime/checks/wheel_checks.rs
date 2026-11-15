//! Checks of the `resolve_wheel` owner (R38): the raw-byte `fetch_json` seam over owned loopback sockets, and the normal program against an owned
//! `CONNECT` + TLS tape peer.
//!
//! * `plain_http_*`: the request head the original's `Request` writes (caller headers after `Host`, `Accept-Encoding` first), raw response bytes read
//!   by `json.load` in every codec, an HTTP error status with its ordered headers, a short body, a redirect, and the `file:` handler.
//! * `selected_saved_cli_cases_through_the_normal_program`: a representative subset of the saved original CLI cases (one per exit status and per
//!   traceback kind) replayed against an owned tape peer built from the saved response bytes, comparing the exit status, the raw standard streams (with
//!   the saved script path rebased to the copied executable's) and the plaintext the peer observed. The full 40-case cohort belongs to the
//!   independent normal run, not to this focused check.
//!
//! The saved reference is read through `R38_WHEEL_REFERENCE` (the directory of the saved capture: `cli-*/`, `certificates/`). Every socket is on
//! `127.0.0.1` and owned by the test; certificate and host name verification is on in every client run; the logical host `api.github.com` is only
//! a name in the `CONNECT` request and the TLS server name. No external endpoint, Python or original is started.

use agent_runtime_support::pyvalue::PyValue;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use upstream_runtime::fetch_https::FetchEnvironment;
use upstream_runtime::http_direct::{ProxyEnvironment, UreqDialer};
use upstream_runtime::wheel_fetch::{fetch_json, FetchError};

const IO_TIMEOUT: Duration = Duration::from_secs(5);

fn reference() -> PathBuf {
    PathBuf::from(
        std::env::var("R38_WHEEL_REFERENCE").expect(
            "R38_WHEEL_REFERENCE must name the saved capture directory (cli-*, certificates)",
        ),
    )
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).expect("hex"))
        .collect()
}

fn read_head(stream: &mut dyn Read) -> Option<Vec<u8>> {
    let mut head = Vec::new();
    let mut byte = [0u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return None,
        }
        assert!(head.len() <= 65536, "peer request head bound exceeded");
    }
    Some(head)
}

fn replace_bytes(haystack: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(haystack.len());
    let mut at = 0;
    while at < haystack.len() {
        if haystack[at..].starts_with(from) {
            out.extend_from_slice(to);
            at += from.len();
        } else {
            out.push(haystack[at]);
            at += 1;
        }
    }
    out
}

// ---- a plain HTTP tape: answers each connection with the next response and records the request heads ----

struct HttpTape {
    address: String,
    heads: Arc<Mutex<Vec<Vec<u8>>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HttpTape {
    fn start(responses: Vec<Vec<u8>>) -> HttpTape {
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned loopback listener");
        let address = listener.local_addr().unwrap().to_string();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, halt) = (heads.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let mut served = 0;
            for incoming in listener.incoming() {
                if halt.load(Ordering::SeqCst) {
                    return;
                }
                let Ok(mut stream) = incoming else { continue };
                stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
                if let Some(head) = read_head(&mut stream) {
                    seen.lock().unwrap().push(head);
                    let response = &responses[served.min(responses.len() - 1)];
                    served += 1;
                    let _ = stream.write_all(response);
                }
            }
        });
        HttpTape {
            address,
            heads,
            stop,
            thread: Some(thread),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("http://{}{path}", self.address)
    }

    fn finish(mut self) -> Vec<Vec<u8>> {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        self.thread.take().unwrap().join().unwrap();
        let heads = self.heads.lock().unwrap().clone();
        heads
    }
}

fn http_response(status: &str, headers: &[(&str, &str)], body: &[u8]) -> Vec<u8> {
    let mut out = format!("HTTP/1.1 {status}\r\n").into_bytes();
    for (name, value) in headers {
        out.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "Content-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .as_bytes(),
    );
    out.extend_from_slice(body);
    out
}

fn fetch_plain(url: &str) -> Result<PyValue, FetchError> {
    fetch_json(
        &PyValue::str(url),
        &FetchEnvironment::default(),
        &UreqDialer::new(),
    )
}

fn encoded(value: &PyValue) -> String {
    upstream_runtime::dto::encode(value)
        .unwrap()
        .dumps_ascii_tight()
        .unwrap()
}

#[test]
fn plain_http_request_head_is_the_one_the_original_request_writes() {
    let tape = HttpTape::start(vec![http_response("200 OK", &[], b"{\"x\":1}")]);
    let url = tape.url("/json?x=1#not-sent");
    let value = fetch_plain(&url).expect("a value");
    assert_eq!(encoded(&value), r#"["dict",[[["str","x"],["int","1"]]]]"#);
    let heads = tape.finish();
    let address = url
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap()
        .to_string();
    let expected = format!(
        "GET /json?x=1 HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: {address}\r\nAccept: application/vnd.github+json\r\nUser-Agent: upstream-version-watch\r\nConnection: close\r\n\r\n"
    );
    assert_eq!(heads, vec![expected.into_bytes()]);
}

#[test]
fn plain_http_body_is_read_as_raw_bytes_in_every_codec() {
    let utf8_bom: Vec<u8> = [&[0xEF, 0xBB, 0xBF][..], b"{\"x\":1}"].concat();
    let utf16_le_bom: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(b"{\"x\":1}".iter().flat_map(|byte| [*byte, 0]))
        .collect();
    let utf16_be_bom_with_latin1_charset: Vec<u8> = [0xFE, 0xFF]
        .into_iter()
        .chain(b"{\"x\":1}".iter().flat_map(|byte| [0, *byte]))
        .collect();
    let utf32_be: Vec<u8> = b"{\"x\":1}"
        .iter()
        .flat_map(|byte| [0, 0, 0, *byte])
        .collect();
    let surrogate: Vec<u8> = b"{\"x\":\"\xed\xa0\x80\"}".to_vec();
    /// A response body, the typed value it must read as, and the response headers that go with it.
    type CodecCase = (
        Vec<u8>,
        &'static str,
        &'static [(&'static str, &'static str)],
    );
    let bodies: Vec<CodecCase> = vec![
        (utf8_bom, r#"["dict",[[["str","x"],["int","1"]]]]"#, &[]),
        (utf16_le_bom, r#"["dict",[[["str","x"],["int","1"]]]]"#, &[]),
        (
            utf16_be_bom_with_latin1_charset,
            r#"["dict",[[["str","x"],["int","1"]]]]"#,
            &[("Content-Type", "text/plain; charset=latin-1")],
        ),
        (utf32_be, r#"["dict",[[["str","x"],["int","1"]]]]"#, &[]),
        (
            surrogate,
            r#"["dict",[[["str","x"],["str","\ud800"]]]]"#,
            &[],
        ),
        (b"null".to_vec(), r#"["none"]"#, &[]),
        (
            b"{\"a\":1,\"b\":2,\"a\":3}".to_vec(),
            r#"["dict",[[["str","a"],["int","3"]],[["str","b"],["int","2"]]]]"#,
            &[],
        ),
    ];
    for (body, expected, headers) in bodies {
        let tape = HttpTape::start(vec![http_response("200 OK", headers, &body)]);
        let value = fetch_plain(&tape.url("/json")).expect("a value");
        tape.finish();
        assert_eq!(encoded(&value), expected);
    }
}

fn exception_of(
    outcome: Result<PyValue, FetchError>,
) -> upstream_runtime::http_direct::PyException {
    match outcome {
        Err(FetchError::Raised { error, .. }) => *error,
        other => panic!("an exception was expected: {other:?}"),
    }
}

#[test]
fn plain_http_failures_are_the_typed_exceptions_of_the_original() {
    let status = HttpTape::start(vec![http_response(
        "404 Not Found",
        &[("X-One", "1"), ("X-Two", "2")],
        b"{\"x\":1}",
    )]);
    let error = exception_of(fetch_plain(&status.url("/json")));
    status.finish();
    assert_eq!(
        (error.class, error.message.to_utf8().unwrap().as_str()),
        ("HTTPError", "HTTP Error 404: Not Found")
    );
    let headers = error.http_headers.expect("the ordered headers are public");
    let names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["X-One", "X-Two", "Content-Length", "Connection"]);

    let short = HttpTape::start(vec![
        b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\n\r\n{\"x\":1}".to_vec(),
    ]);
    let error = exception_of(fetch_plain(&short.url("/json")));
    short.finish();
    assert_eq!(
        (error.class, error.message.to_utf8().unwrap().as_str()),
        (
            "IncompleteRead",
            "IncompleteRead(7 bytes read, 13 more expected)"
        )
    );

    let invalid = HttpTape::start(vec![http_response("200 OK", &[], b"\xff")]);
    let error = exception_of(fetch_plain(&invalid.url("/json")));
    invalid.finish();
    assert_eq!(
        error.message.to_utf8().unwrap(),
        "'utf-8' codec can't decode byte 0xff in position 0: invalid start byte"
    );

    let malformed = HttpTape::start(vec![http_response("200 OK", &[], b"{\"x\":[")]);
    let error = exception_of(fetch_plain(&malformed.url("/json")));
    malformed.finish();
    assert_eq!(
        error.message.to_utf8().unwrap(),
        "Expecting value: line 1 column 7 (char 6)"
    );
}

#[test]
fn plain_http_redirect_is_followed_and_the_final_body_is_read() {
    let tape = HttpTape::start(vec![
        http_response("302 Found", &[("Location", "/final")], b""),
        http_response("200 OK", &[], b"{\"x\":1}"),
    ]);
    let value = fetch_plain(&tape.url("/json")).expect("a value");
    let heads = tape.finish();
    assert_eq!(encoded(&value), r#"["dict",[[["str","x"],["int","1"]]]]"#);
    assert_eq!(heads.len(), 2);
    assert!(heads[1].starts_with(b"GET /final HTTP/1.1\r\n"));
}

#[test]
fn a_file_url_reads_raw_bytes_and_a_missing_file_is_the_wrapped_file_not_found_error() {
    let directory = std::env::temp_dir().join(format!("wheel-checks-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let document = directory.join("doc.json");
    let utf16: Vec<u8> = [0xFF, 0xFE]
        .into_iter()
        .chain(b"{\"x\":1}".iter().flat_map(|byte| [*byte, 0]))
        .collect();
    std::fs::write(&document, utf16).unwrap();
    let value = fetch_plain(&format!("file://{}", document.display())).expect("a value");
    assert_eq!(encoded(&value), r#"["dict",[[["str","x"],["int","1"]]]]"#);
    let absent = directory.join("absent.json");
    let error = exception_of(fetch_plain(&format!("file://{}", absent.display())));
    assert_eq!(error.class, "URLError");
    assert_eq!(
        error.message.to_utf8().unwrap(),
        format!(
            "<urlopen error [Errno 2] No such file or directory: '{}'>",
            absent.display()
        )
    );
    std::fs::remove_dir_all(&directory).unwrap();
}

// ---- the owned CONNECT + TLS tape peer and the saved CLI cases ----

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    ConnectRequest(Vec<u8>),
    TlsSni(Option<String>),
    TlsReady,
    TlsError,
    HttpRequest(Vec<u8>),
}

struct TlsPeer {
    address: String,
    events: Arc<Mutex<Vec<Event>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn server_config(certificates: &Path) -> Arc<ServerConfig> {
    let chain: Vec<CertificateDer<'static>> =
        CertificateDer::pem_file_iter(certificates.join("leaf.pem"))
            .expect("leaf certificate")
            .map(|item| item.expect("certificate"))
            .collect();
    let key = PrivateKeyDer::from_pem_file(certificates.join("leaf.key")).expect("leaf key");
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    Arc::new(
        ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(rustls::ALL_VERSIONS)
            .expect("TLS versions")
            .with_no_client_auth()
            .with_single_cert(chain, key)
            .expect("the owned leaf and key belong together"),
    )
}

fn serve(
    mut stream: TcpStream,
    config: &Arc<ServerConfig>,
    response: &[u8],
    events: &Mutex<Vec<Event>>,
) {
    let push = |event: Event| events.lock().unwrap().push(event);
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    let Some(head) = read_head(&mut stream) else {
        return;
    };
    push(Event::ConnectRequest(head));
    let _ = stream.write_all(b"HTTP/1.1 200 Connection Established\r\n\r\n");
    let mut connection = ServerConnection::new(config.clone()).expect("server session");
    let handshake: std::io::Result<()> = (|| {
        while connection.is_handshaking() {
            connection.complete_io(&mut stream)?;
        }
        Ok(())
    })();
    push(Event::TlsSni(connection.server_name().map(str::to_string)));
    if handshake.is_err() {
        push(Event::TlsError);
        return;
    }
    push(Event::TlsReady);
    let mut tls = rustls::Stream::new(&mut connection, &mut stream);
    let Some(head) = read_head(&mut tls) else {
        return;
    };
    push(Event::HttpRequest(head));
    let _ = tls.write_all(response).and_then(|()| tls.flush());
    connection.send_close_notify();
    let _ = connection.complete_io(&mut stream);
}

impl TlsPeer {
    fn start(certificates: PathBuf, responses: Vec<Vec<u8>>) -> TlsPeer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned loopback listener");
        let address = listener.local_addr().unwrap().to_string();
        let events = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (seen, halt) = (events.clone(), stop.clone());
        let thread = std::thread::spawn(move || {
            let config = server_config(&certificates);
            let mut served = 0;
            for incoming in listener.incoming() {
                if halt.load(Ordering::SeqCst) {
                    return;
                }
                if let Ok(stream) = incoming {
                    let response = responses[served.min(responses.len() - 1)].clone();
                    served += 1;
                    serve(stream, &config, &response, &seen);
                }
            }
        });
        TlsPeer {
            address,
            events,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> Vec<Event> {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        self.thread.take().unwrap().join().unwrap();
        let events = self.events.lock().unwrap().clone();
        events
    }
}

fn load(path: PathBuf) -> PyValue {
    agent_runtime_support::pyjson::loads(&std::fs::read_to_string(path).expect("saved file"))
        .expect("saved JSON")
}

fn member<'a>(value: &'a PyValue, key: &str) -> Option<&'a PyValue> {
    match value {
        PyValue::Dict(members) => members.get(key),
        _ => None,
    }
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("a string was expected: {other:?}"),
    }
}

fn items(value: Option<&PyValue>) -> Vec<&PyValue> {
    match value {
        Some(PyValue::List(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

/// The `raw_hex` of every member of `list` that has one and whose `event` (when `kinds` is not empty) is one of `kinds`, in order.
fn raw_bytes(list: Option<&PyValue>, kinds: &[&str]) -> Vec<Vec<u8>> {
    items(list)
        .into_iter()
        .filter(|entry| {
            kinds.is_empty()
                || member(entry, "event")
                    .is_some_and(|event| kinds.contains(&text_of(event).as_str()))
        })
        .filter_map(|entry| member(entry, "raw_hex").map(|hex| hex_decode(&text_of(hex))))
        .collect()
}

/// One saved CLI case replayed against the copied executable and an owned tape peer; returns the differences found.
fn replay_saved_case(case: &str, scratch: &Path) -> Vec<String> {
    let root = reference();
    let saved = root.join(case);
    let execution = load(saved.join("execution.json"));
    let description = load(saved.join("peer-description.json"));
    let observed = load(saved.join("peer-observed.json"));
    let mut differences = Vec::new();

    let saved_base = format!("{}/business", saved.display());
    let base = scratch.join(case).join("business");
    let script = base.join("node/.claude/skills/upstream-version-watch/scripts/resolve_wheel.py");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    std::fs::create_dir_all(base.join("private-tmp")).unwrap();
    std::fs::copy(env!("CARGO_BIN_EXE_resolve_wheel"), &script).unwrap();

    let responses = raw_bytes(member(&description, "responses"), &[]);
    let environment: Vec<(String, String)> = match member(&execution, "env") {
        Some(PyValue::Dict(members)) => members
            .iter()
            .map(|(name, value)| (name.to_utf8().expect("name"), text_of(value)))
            .collect(),
        _ => panic!("the saved execution has an environment"),
    };
    let uses_peer =
        environment.iter().any(|(name, _)| name == "https_proxy") && !responses.is_empty();
    let peer = uses_peer.then(|| TlsPeer::start(root.join("certificates"), responses.clone()));

    let mut command = Command::new(&script);
    command
        .env_clear()
        .current_dir(&base)
        .stdin(std::process::Stdio::null());
    let case_argv: Vec<String> = items(member(&execution, "argv"))
        .into_iter()
        .map(text_of)
        .skip_while(|item| !item.ends_with("resolve_wheel.py"))
        .skip(1)
        .collect();
    command.args(&case_argv);
    for (name, value) in &environment {
        let value = match name.as_str() {
            "https_proxy" => format!("http://{}", peer.as_ref().expect("a peer").address),
            "TMPDIR" => base.join("private-tmp").display().to_string(),
            _ => value.clone(),
        };
        command.env(name, value);
    }
    // a freshly copied executable can be 'busy' (ETXTBSY) while another test thread forks; retry only that error
    let output = (0..50)
        .find_map(|_| match command.output() {
            Err(error) if error.raw_os_error() == Some(26) => {
                std::thread::sleep(std::time::Duration::from_millis(20));
                None
            }
            other => Some(other.expect("the copied executable ran")),
        })
        .expect("the copied executable started");
    let rebased = |bytes: &[u8]| {
        replace_bytes(
            bytes,
            saved_base.as_bytes(),
            base.display().to_string().as_bytes(),
        )
    };
    let saved_stdout = rebased(&std::fs::read(saved.join("stdout.bin")).unwrap());
    let saved_stderr = rebased(&std::fs::read(saved.join("stderr.bin")).unwrap());
    if output.stdout != saved_stdout {
        differences.push(format!(
            "{case}: stdout differs\nactual:\n{}\nsaved:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&saved_stdout)
        ));
    }
    if output.stderr != saved_stderr {
        differences.push(format!(
            "{case}: stderr differs\nactual:\n{}\nsaved:\n{}",
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&saved_stderr)
        ));
    }
    let saved_exit = match member(&execution, "exit_code") {
        Some(PyValue::Int(code)) => i64::try_from(code.clone()).expect("a status"),
        other => panic!("the saved exit status: {other:?}"),
    };
    if i64::from(output.status.code().unwrap_or(-1)) != saved_exit {
        differences.push(format!(
            "{case}: exit status {:?}, saved {saved_exit}",
            output.status.code()
        ));
    }
    if let Some(peer) = peer {
        let events = peer.finish();
        let want_head = raw_bytes(
            member(&observed, "events"),
            &["connect_request", "http_request"],
        );
        let got_head: Vec<Vec<u8>> = events
            .iter()
            .filter_map(|event| match event {
                Event::ConnectRequest(head) | Event::HttpRequest(head) => Some(head.clone()),
                _ => None,
            })
            .collect();
        // the saved wire holds the heads in order (CONNECT, GET, ...); only the ones this replay observed are compared, by content
        if got_head != want_head {
            differences.push(format!("{case}: the peer observed different request heads: {got_head:?} versus saved {want_head:?}"));
        }
        let sni_ok = events
            .iter()
            .all(|event| !matches!(event, Event::TlsSni(Some(name)) if name != "api.github.com"));
        if !sni_ok {
            differences.push(format!(
                "{case}: the TLS server name was not the logical host"
            ));
        }
    }
    differences
}

#[test]
fn selected_saved_cli_cases_through_the_normal_program() {
    let scratch = std::env::temp_dir().join(format!("wheel-cli-{}", std::process::id()));
    let mut differences = Vec::new();
    // one per exit status and per traceback kind: value, equal-tie, exit 5, int ValueError, exit 4, assets null, KeyError, malformed JSON,
    // HTTP status, refused certificate (chained), help, strict ASCII, non-ASCII request line, redirect
    for case in [
        "cli-001", "cli-004", "cli-009", "cli-015", "cli-018", "cli-023", "cli-025", "cli-029",
        "cli-031", "cli-032", "cli-034", "cli-038", "cli-040", "cli-033",
    ] {
        differences.extend(replay_saved_case(case, &scratch));
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert!(differences.is_empty(), "{}", differences.join("\n---\n"));
}

#[test]
fn the_logical_host_is_in_the_connect_request_and_never_the_proxy_address() {
    let root = reference();
    let tape = http_response("200 OK", &[], b"{\"assets\":[]}");
    let peer = TlsPeer::start(root.join("certificates"), vec![tape]);
    let environment = FetchEnvironment {
        proxies: ProxyEnvironment::from_bytes([(
            b"https_proxy".to_vec(),
            format!("http://{}", peer.address).into_bytes(),
        )]),
        trust: recipe_runtime::tls_trust::Environment {
            ssl_cert_file: Some(root.join("certificates/ca.pem").into_os_string()),
            ssl_cert_dir: Some(root.join("certificates/empty-ca-dir").into_os_string()),
            default_file: None,
            default_directories: Vec::new(),
        },
    };
    let value = fetch_json(
        &PyValue::str("https://api.github.com/repos/vllm-project/vllm/releases/tags/v0.32.0"),
        &environment,
        &UreqDialer::new(),
    )
    .expect("a value");
    let events = peer.finish();
    assert_eq!(
        encoded(&value),
        r#"["dict",[[["str","assets"],["list",[]]]]]"#
    );
    assert!(
        matches!(&events[0], Event::ConnectRequest(head) if head.starts_with(b"CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n"))
    );
    assert!(events.contains(&Event::TlsSni(Some("api.github.com".to_string()))));
}
