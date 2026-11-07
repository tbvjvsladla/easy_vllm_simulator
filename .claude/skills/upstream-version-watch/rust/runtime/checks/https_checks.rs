//! Checks of the HTTPS plain-str URL fetch component (R35, TLS/CONNECT episode).
//!
//! * `saved_original_cases_through_the_json_service`: the 13 saved original records (typed value or exception chain with its constructor
//!   arguments, captured attributes including the three SSL ones, arguments after the call, streams) and the saved plaintext wire of the owned
//!   peers (the `CONNECT` request and its reply, the SNI name, whether TLS became ready or failed, the `GET` request, the response sent), driven
//!   through the production JSON-lines service over real sockets and a real Rust TLS handshake against an owned loopback TLS server. Only the
//!   saved loopback endpoint (host and port) is rebased to the new peer's; frames and the fp state of `HTTPError` are interpreter observations
//!   a native component does not have, and the encrypted bytes, cipher and the server's view of the alert are auxiliary and not compared.
//! * the same cases, a selected few, through the normal program as a subprocess with an explicit child environment (owned CA file, empty CA
//!   directory, `https_proxy` for the owned proxy only), keeping the pid, first exit status and raw streams.
//! * the typed API, the scope refusals, the transport-failure boundary, the unmodified proxy decision and a scan that the production
//!   sources hold no baseline data.
//!
//! The saved reference is read through `R35_FETCH_TLS_REFERENCE` (the directory with `cases.json`, `functional-contract.json`,
//! `certificates/` and `fetch-tls-001` ... `fetch-tls-013`); the optional `R35_TLS_LOG_DIR` names a directory that keeps the artifacts of
//! each subprocess. Production code contains no path, label or expected value. Every socket is on `127.0.0.1` and owned by the test; the
//! test server's certificates and keys are the saved test data only; no external endpoint, Python, benchmark or original is started, and
//! the client's certificate and host name verification is on in every run.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::tls_trust;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::{ServerConfig, ServerConnection};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use upstream_runtime::fetch_https::jsonl::{answer_with, OPERATION};
use upstream_runtime::fetch_https::{
    fetch_https_text_of, fetch_https_text_with, FetchEnvironment, SCOPE,
};
use upstream_runtime::http_direct::{
    Dial, ErrorValue, FetchFailure, ProxyEnvironment, TransportFailure, UreqDialer, Wire,
};

const IO_TIMEOUT: Duration = Duration::from_secs(5);
const DEFAULT_CONNECT_REPLY: &[u8] = b"HTTP/1.1 200 Connection Established\r\n\r\n";

fn reference() -> PathBuf {
    PathBuf::from(std::env::var("R35_FETCH_TLS_REFERENCE").expect(
        "R35_FETCH_TLS_REFERENCE must name the saved reference directory (cases.json, certificates, fetch-tls-001 ...)",
    ))
}

fn read_json(path: PathBuf) -> PyValue {
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    loads(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn member<'a>(value: &'a PyValue, key: &str) -> &'a PyValue {
    match value {
        PyValue::Dict(map) => map
            .get(key)
            .unwrap_or_else(|| panic!("no member {key} in {}", value.repr())),
        other => panic!("not an object: {}", other.repr()),
    }
}

fn has_member(value: &PyValue, key: &str) -> bool {
    matches!(value, PyValue::Dict(map) if map.get(key).is_some())
}

fn text_of(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.to_utf8().expect("text"),
        other => panic!("not text: {}", other.repr()),
    }
}

fn list(value: &PyValue) -> &[PyValue] {
    match value {
        PyValue::List(items) => items,
        other => panic!("not an array: {}", other.repr()),
    }
}

fn hex_decode(hex: &str) -> Vec<u8> {
    (0..hex.len() / 2)
        .map(|at| u8::from_str_radix(&hex[2 * at..2 * at + 2], 16).expect("hex"))
        .collect()
}

fn replace_bytes(haystack: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8_lossy(haystack)
        .replace(from, to)
        .into_bytes()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The saved test data: the CA files named by the saved contract, the owned leaf certificates and key.
// ---------------------------------------------------------------------------------------------------------------------------

struct TestData {
    root: PathBuf,
    trusted_ca: PathBuf,
    wrong_ca: PathBuf,
    empty_ca_dir: PathBuf,
}

fn test_data() -> TestData {
    let root = reference();
    let contract = read_json(root.join("functional-contract.json"));
    let path_of = |key: &str| PathBuf::from(text_of(member(&contract, key)));
    TestData {
        trusted_ca: path_of("trusted_CA"),
        wrong_ca: path_of("wrong_CA"),
        empty_ca_dir: path_of("SSL_CERT_DIR"),
        root,
    }
}

impl TestData {
    fn server_config(&self, leaf: &str) -> Arc<ServerConfig> {
        let certificates = self.root.join("certificates");
        let certificate_file = match leaf {
            "normal" => "leaf.pem",
            "dns-only" => "leaf-dns-only.pem",
            other => panic!("unknown saved leaf {other}"),
        };
        let chain: Vec<CertificateDer<'static>> =
            CertificateDer::pem_file_iter(certificates.join(certificate_file))
                .expect("leaf certificate file")
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

    /// The trust of a saved case: the CA file of `client_trust` and the empty CA directory, nothing from the host.
    fn trust(&self, client_trust: &str) -> tls_trust::Environment {
        let file = match client_trust {
            "trusted" => &self.trusted_ca,
            "wrong-root" => &self.wrong_ca,
            other => panic!("unknown saved trust {other}"),
        };
        tls_trust::Environment {
            ssl_cert_file: Some(file.clone().into_os_string()),
            ssl_cert_dir: Some(self.empty_ca_dir.clone().into_os_string()),
            default_file: None,
            default_directories: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The owned loopback peer: a TLS server (direct), or an HTTP proxy that tunnels to the same TLS server (CONNECT). It records the plaintext
// wire it can observe, in the saved peer's event vocabulary.
// ---------------------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq)]
enum Event {
    TcpAccept,
    ConnectRequest(Vec<u8>),
    ConnectResponse(Vec<u8>),
    TlsSni(Option<String>),
    TlsReady,
    /// The server's own error text is auxiliary (it describes the client's alert), so only the event is compared.
    TlsError,
    HttpRequest(Vec<u8>),
    HttpResponse(Vec<u8>),
}

#[derive(Clone)]
enum Behavior {
    /// Speak TLS with this configuration and answer the one request with `response`.
    Tls {
        config: Arc<ServerConfig>,
        response: Vec<u8>,
    },
    /// Do not speak TLS: read what the client sends first, answer with these bytes, close.
    NotTls(Vec<u8>),
}

#[derive(Clone)]
struct Plan {
    /// Behave as an HTTP proxy first: read a `CONNECT` head, answer `connect_reply`.
    proxy: bool,
    connect_reply: Option<Vec<u8>>,
    stop_after_connect: bool,
    behavior: Behavior,
}

struct Peer {
    address: String,
    events: Arc<Mutex<Vec<Event>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
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

fn serve(mut stream: TcpStream, plan: &Plan, events: &Mutex<Vec<Event>>) {
    let push = |event: Event| events.lock().unwrap().push(event);
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    push(Event::TcpAccept);
    if plan.proxy {
        let Some(head) = read_head(&mut stream) else {
            return;
        };
        push(Event::ConnectRequest(head));
        let reply = plan
            .connect_reply
            .clone()
            .unwrap_or_else(|| DEFAULT_CONNECT_REPLY.to_vec());
        let _ = stream.write_all(&reply);
        push(Event::ConnectResponse(reply));
        if plan.stop_after_connect {
            return;
        }
    }
    match &plan.behavior {
        Behavior::NotTls(bytes) => {
            let mut first = [0u8; 1024];
            let _ = stream.read(&mut first);
            let _ = stream.write_all(bytes);
        }
        Behavior::Tls { config, response } => {
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
            push(Event::HttpResponse(response.clone()));
            connection.send_close_notify();
            let _ = connection.complete_io(&mut stream);
        }
    }
}

impl Peer {
    fn start(plan: Plan) -> Peer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned loopback listener");
        let address = listener.local_addr().expect("address").to_string();
        let events = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (events, stop) = (events.clone(), stop.clone());
            std::thread::spawn(move || {
                for incoming in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    if let Ok(stream) = incoming {
                        serve(stream, &plan, &events);
                    }
                }
            })
        };
        Peer {
            address,
            events,
            stop,
            thread: Some(thread),
        }
    }

    fn finish(mut self) -> Vec<Event> {
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("peer thread");
        }
        let events = self.events.lock().unwrap().clone();
        events
    }
}

fn tls_plan(data: &TestData, leaf: &str, response: &[u8]) -> Plan {
    Plan {
        proxy: false,
        connect_reply: None,
        stop_after_connect: false,
        behavior: Behavior::Tls {
            config: data.server_config(leaf),
            response: response.to_vec(),
        },
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Driving the production JSON service, in process or as the normal program.
// ---------------------------------------------------------------------------------------------------------------------------

fn request_line(id: &str, operation: &str, url: &str) -> String {
    let args = vec![PyValue::List(vec![PyValue::str("str"), PyValue::str(url)])];
    let mut row = PyDict::new();
    row.set("id", PyValue::str(id));
    row.set("operation", PyValue::str(operation));
    row.set(
        "args_dto",
        PyValue::List(vec![PyValue::str("list"), PyValue::List(args)]),
    );
    PyValue::Dict(row).dumps_ascii().expect("request")
}

fn kind_of(record: &PyValue) -> String {
    text_of(member(record, "kind"))
}

/// What one run of a saved case is given: its URL, the proxy variable (the owned proxy) and the CA sources.
struct Run<'a> {
    id: String,
    url: String,
    proxy: Option<String>,
    data: &'a TestData,
    client_trust: String,
}

impl Run<'_> {
    fn environment(&self) -> FetchEnvironment {
        FetchEnvironment {
            proxies: ProxyEnvironment::from_pairs(
                self.proxy
                    .iter()
                    .map(|proxy| ("https_proxy".to_string(), proxy.clone())),
            ),
            trust: self.data.trust(&self.client_trust),
        }
    }

    fn in_process(&self) -> PyValue {
        answer_with(
            &PyStr::new(&request_line(&self.id, OPERATION, &self.url)),
            &self.environment(),
            &UreqDialer::new(),
        )
    }

    /// The normal program with exactly the environment of the saved owner child: the CA file, the empty CA directory and the owned proxy.
    fn as_program(&self) -> PyValue {
        let mut environment = vec![
            (
                b"SSL_CERT_FILE".to_vec(),
                path_bytes(&match self.client_trust.as_str() {
                    "trusted" => self.data.trusted_ca.clone(),
                    _ => self.data.wrong_ca.clone(),
                }),
            ),
            (
                b"SSL_CERT_DIR".to_vec(),
                path_bytes(&self.data.empty_ca_dir),
            ),
        ];
        if let Some(proxy) = &self.proxy {
            environment.push((b"https_proxy".to_vec(), proxy.clone().into_bytes()));
        }
        let run = run_program(
            &format!("saved-{}", self.id),
            &[request_line(&self.id, OPERATION, &self.url)],
            &environment,
        );
        assert!(
            run.status.success(),
            "{}: {:?}",
            self.id,
            String::from_utf8_lossy(&run.stderr)
        );
        assert!(
            run.stderr.is_empty(),
            "{}: the program wrote to stderr",
            self.id
        );
        assert_eq!(run.records.len(), 1);
        run.records.into_iter().next().unwrap()
    }
}

fn path_bytes(path: &Path) -> Vec<u8> {
    std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()).to_vec()
}

struct ProgramRun {
    records: Vec<PyValue>,
    status: std::process::ExitStatus,
    stderr: Vec<u8>,
}

/// The normal program as a subprocess with exactly `environment`; its pid, first exit status, raw streams and environment are kept under
/// `R35_TLS_LOG_DIR` when that (test-only) variable names a directory, as `<tag>.{pid,exit,stdout.bin,stderr.bin,environment}`.
fn run_program(tag: &str, lines: &[String], environment: &[(Vec<u8>, Vec<u8>)]) -> ProgramRun {
    use std::os::unix::ffi::OsStringExt;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_upstream_https_api"));
    command
        .arg("evaluate-jsonl")
        .env_clear()
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    for (name, value) in environment {
        command.env(
            std::ffi::OsString::from_vec(name.clone()),
            std::ffi::OsString::from_vec(value.clone()),
        );
    }
    let mut child = command.spawn().expect("the normal program starts");
    let pid = child.id();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(lines.join("\n").as_bytes())
        .unwrap();
    let output = child.wait_with_output().expect("program output");
    if let Ok(directory) = std::env::var("R35_TLS_LOG_DIR") {
        let base = Path::new(&directory);
        std::fs::write(base.join(format!("{tag}.pid")), format!("{pid}\n")).unwrap();
        std::fs::write(
            base.join(format!("{tag}.exit")),
            format!("{:?}\n", output.status.code()),
        )
        .unwrap();
        std::fs::write(base.join(format!("{tag}.stdout.bin")), &output.stdout).unwrap();
        std::fs::write(base.join(format!("{tag}.stderr.bin")), &output.stderr).unwrap();
        let shown: Vec<String> = environment
            .iter()
            .map(|(name, value)| {
                let hex = |bytes: &[u8]| -> String {
                    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
                };
                format!("key_hex={} value_hex={}", hex(name), hex(value))
            })
            .collect();
        std::fs::write(base.join(format!("{tag}.environment")), shown.join("\n")).unwrap();
    }
    let records = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| loads(line).unwrap())
        .collect();
    ProgramRun {
        records,
        status: output.status,
        stderr: output.stderr,
    }
}

/// The saved exception record without the interpreter-only observations (frames, the fp state of `HTTPError`), at every depth.
fn without_interpreter_observations(value: &PyValue) -> PyValue {
    match value {
        PyValue::Dict(map) => {
            let mut kept = PyDict::new();
            for (key, item) in map.iter() {
                let name = key.to_utf8().expect("key");
                if name == "frames" || name == "http_fp_state" {
                    continue;
                }
                kept.set(&name, without_interpreter_observations(item));
            }
            PyValue::Dict(kept)
        }
        PyValue::List(items) => {
            PyValue::List(items.iter().map(without_interpreter_observations).collect())
        }
        other => other.clone(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The saved cases.
// ---------------------------------------------------------------------------------------------------------------------------

fn saved_events(peer: &PyValue, old_host: &str, new_host: &str) -> Vec<Event> {
    list(member(peer, "events"))
        .iter()
        .map(|event| {
            let raw = |key: &str| hex_decode(&text_of(member(event, key)));
            match text_of(member(event, "event")).as_str() {
                "tcp_accept" => Event::TcpAccept,
                "connect_request" => Event::ConnectRequest(raw("raw_hex")),
                "connect_response" => Event::ConnectResponse(raw("raw_hex")),
                "tls_sni" => Event::TlsSni(match member(event, "name") {
                    PyValue::None => None,
                    name => Some(text_of(name)),
                }),
                "tls_ready" => Event::TlsReady,
                "tls_error" => Event::TlsError,
                "http_request" => {
                    Event::HttpRequest(replace_bytes(&raw("raw_hex"), old_host, new_host))
                }
                "http_response" => Event::HttpResponse(raw("raw_hex")),
                other => panic!("unknown saved event {other}"),
            }
        })
        .collect()
}

/// What a pass over the saved cases counted.
#[derive(Default, Debug, PartialEq, Eq)]
struct Totals {
    logical_cases: usize,
    values: usize,
    exceptions: usize,
    tcp_connections: usize,
    connect_requests: usize,
    get_requests: usize,
    tls_ready: usize,
    tls_errors: usize,
}

/// Drives the saved cases named by `only` (all when empty) through `run` against a fresh owned peer and compares the whole record and the
/// peer's plaintext wire with the saved original and the saved peer record. Only the saved loopback endpoint is rebased.
fn drive_saved_cases(only: &[&str], run: &dyn Fn(&Run) -> PyValue) -> Totals {
    let data = test_data();
    let directory = read_json(data.root.join("cases.json"));
    let mut totals = Totals::default();
    for case in list(member(&directory, "cases")) {
        let id = text_of(member(case, "id"));
        if !only.is_empty() && !only.contains(&id.as_str()) {
            continue;
        }
        let folder = data.root.join(&id);
        let original = read_json(folder.join("original.json"));
        let saved_peer = read_json(folder.join("peer.json"));
        let spec = member(case, "peer");
        let proxy_mode = text_of(member(spec, "mode")) == "proxy";
        let optional_hex =
            |key: &str| has_member(spec, key).then(|| hex_decode(&text_of(member(spec, key))));
        let plan = Plan {
            proxy: proxy_mode,
            connect_reply: optional_hex("connect_reply_hex"),
            stop_after_connect: has_member(spec, "stop_after_connect"),
            behavior: Behavior::Tls {
                config: data.server_config(&text_of(member(spec, "leaf"))),
                response: hex_decode(&text_of(member(spec, "raw_response_hex"))),
            },
        };
        let peer = Peer::start(plan);
        let (url, proxy) = if proxy_mode {
            (
                text_of(member(case, "url")),
                Some(format!("http://{}", peer.address)),
            )
        } else {
            (
                text_of(member(case, "url"))
                    .replace("@BASE@", &format!("https://{}", peer.address)),
                None,
            )
        };
        let run_case = Run {
            id: id.clone(),
            url: url.clone(),
            proxy,
            data: &data,
            client_trust: text_of(member(case, "client_trust")),
        };
        let record = run(&run_case);
        let address = peer.address.clone();
        let events = peer.finish();
        totals.logical_cases += 1;

        // the whole record against the saved original (the saved endpoint is the only text rebased)
        let old_endpoint = text_of(member(&saved_peer, "endpoint"));
        let old_host = old_endpoint.trim_start_matches("http://").to_string();
        let rebase = |text: String| text.replace(&old_host, &address);
        assert_eq!(text_of(member(&record, "id")), id);
        assert_eq!(text_of(member(&record, "scope")), SCOPE, "{id}: scope");
        let saved_kind = text_of(member(&original, "kind"));
        assert_eq!(kind_of(&record), saved_kind, "{id}");
        match saved_kind.as_str() {
            "value" => {
                totals.values += 1;
                assert_eq!(
                    member(&record, "value").repr(),
                    member(&original, "value").repr(),
                    "{id}: returned text"
                );
            }
            "exception" => {
                totals.exceptions += 1;
                let expected = without_interpreter_observations(member(&original, "exception"));
                assert_eq!(
                    member(&record, "exception").repr(),
                    rebase(expected.repr()),
                    "{id}: exception chain"
                );
            }
            other => panic!("{id}: unexpected saved kind {other}"),
        }
        for stream in ["stdout", "stderr"] {
            assert_eq!(
                text_of(member(&record, stream)),
                text_of(member(&original, stream)),
                "{id}: {stream}"
            );
        }
        let saved_url = member(member(&original, "before"), "url");
        assert_eq!(
            member(&record, "args_after").repr(),
            format!("['list', [{}]]", rebase(saved_url.repr())),
            "{id}: arguments after the call"
        );
        assert_eq!(
            member(&record, "kwargs_after").repr(),
            "['dict', []]",
            "{id}"
        );

        // the plaintext wire the owned peer observed against the saved peer record
        let expected_events = saved_events(&saved_peer, &old_host, &address);
        assert_eq!(
            events, expected_events,
            "{id}: plaintext wire and TLS outcome"
        );
        totals.tcp_connections += events.iter().filter(|e| **e == Event::TcpAccept).count();
        totals.connect_requests += events
            .iter()
            .filter(|e| matches!(e, Event::ConnectRequest(_)))
            .count();
        totals.get_requests += events
            .iter()
            .filter(|e| matches!(e, Event::HttpRequest(_)))
            .count();
        totals.tls_ready += events.iter().filter(|e| **e == Event::TlsReady).count();
        totals.tls_errors += events.iter().filter(|e| **e == Event::TlsError).count();
        // the saved counters agree with the saved events they summarise
        assert_eq!(
            events.iter().filter(|e| **e == Event::TcpAccept).count() as i64,
            int_of(member(&saved_peer, "tcp_accept_count"))
        );
    }
    totals
}

fn int_of(value: &PyValue) -> i64 {
    match value {
        PyValue::Int(number) => i64::try_from(number.clone()).expect("small"),
        other => panic!("not an int: {}", other.repr()),
    }
}

#[test]
fn saved_original_cases_through_the_json_service() {
    let totals = drive_saved_cases(&[], &|case| case.in_process());
    println!("R35 fetch_https in-process: {totals:?}");
    assert_eq!(
        totals,
        Totals {
            logical_cases: 13,
            values: 2,
            exceptions: 11,
            tcp_connections: 13,
            connect_requests: 10,
            get_requests: 5,
            tls_ready: 5,
            tls_errors: 4,
        }
    );
}

#[test]
fn selected_saved_cases_through_the_normal_program() {
    // direct trusted, direct untrusted CA, direct IP mismatch, CONNECT trusted, CONNECT HTTP 404, CONNECT refused 403
    let totals = drive_saved_cases(
        &[
            "fetch-tls-001",
            "fetch-tls-002",
            "fetch-tls-003",
            "fetch-tls-004",
            "fetch-tls-005",
            "fetch-tls-010",
        ],
        &|case| case.as_program(),
    );
    println!("R35 fetch_https normal program: {totals:?}");
    assert_eq!(totals.logical_cases, 6);
    assert_eq!((totals.values, totals.exceptions), (2, 4));
    assert_eq!((totals.tls_ready, totals.tls_errors), (3, 2));
}

// ---------------------------------------------------------------------------------------------------------------------------
// The typed API, the scope refusals and the transport boundary.
// ---------------------------------------------------------------------------------------------------------------------------

fn environment_for(data: &TestData, proxy: Option<&str>) -> FetchEnvironment {
    FetchEnvironment {
        proxies: ProxyEnvironment::from_pairs(
            proxy
                .iter()
                .map(|value| ("https_proxy".to_string(), value.to_string())),
        ),
        trust: data.trust("trusted"),
    }
}

fn fetch(url: &str, environment: &FetchEnvironment) -> Result<String, FetchFailure> {
    fetch_https_text_with(url, environment, &UreqDialer::new())
}

fn refusal_boundary(outcome: Result<String, FetchFailure>) -> String {
    match outcome {
        Err(FetchFailure::OutOfScope(refusal)) => refusal.boundary.to_string(),
        other => panic!("expected a named refusal, got {other:?}"),
    }
}

#[test]
fn the_typed_api_returns_text_and_the_exception_chain() {
    let data = test_data();
    let ok = Peer::start(tls_plan(
        &data,
        "normal",
        b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\nConnection: close\r\n\r\nTLS \xed\x95\x9c\xea\xb8\x80",
    ));
    let environment = environment_for(&data, None);
    assert_eq!(
        fetch(&format!("https://{}/x", ok.address), &environment).unwrap(),
        "TLS 한글"
    );
    let events = ok.finish();
    let Event::HttpRequest(request) = &events[3] else {
        panic!("the fourth event is the request: {events:?}")
    };
    assert!(
        request.starts_with(b"GET /x HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: 127.0.0.1:")
    );

    let wrong = Peer::start(tls_plan(&data, "normal", b"unused"));
    let mut untrusted = environment_for(&data, None);
    untrusted.trust = data.trust("wrong-root");
    let Err(FetchFailure::Raised(error)) =
        fetch(&format!("https://{}/x", wrong.address), &untrusted)
    else {
        panic!("a certificate the trust does not cover raises")
    };
    assert_eq!((error.class, error.module), ("URLError", "urllib.error"));
    assert!(error
        .message
        .to_utf8()
        .unwrap()
        .starts_with("<urlopen error [SSL: CERTIFICATE_VERIFY_FAILED]"));
    assert_eq!(wrong.finish().last(), Some(&Event::TlsError));
}

#[test]
fn other_schemes_and_proxy_settings_are_refused_by_name_before_any_connection() {
    let data = test_data();
    let quiet = Peer::start(tls_plan(&data, "normal", b"unused"));
    let url = format!("https://{}/x", quiet.address);
    let proxied = |value: &str| environment_for(&data, Some(value));
    assert_eq!(
        refusal_boundary(fetch("http://127.0.0.1:9/x", &environment_for(&data, None))),
        "scheme"
    );
    assert_eq!(
        refusal_boundary(fetch("ftp://127.0.0.1:9/x", &environment_for(&data, None))),
        "scheme"
    );
    assert_eq!(
        refusal_boundary(fetch(&url, &proxied("http://user:pw@127.0.0.1:9"))),
        "proxy_credentials"
    );
    assert_eq!(
        refusal_boundary(fetch(&url, &proxied("socks5://127.0.0.1:9"))),
        "proxy_scheme"
    );
    assert_eq!(
        refusal_boundary(fetch(&url, &proxied("http://127.0.0.1"))),
        "proxy_hostport"
    );
    let mut bypass = proxied("http://127.0.0.1:9");
    bypass.proxies = ProxyEnvironment::from_pairs([
        ("https_proxy".to_string(), "http://127.0.0.1:9".to_string()),
        ("no_proxy".to_string(), "127.0.0.1".to_string()),
    ]);
    assert_eq!(
        refusal_boundary(fetch(&url, &bypass)),
        "no_proxy_environment"
    );
    assert!(
        quiet.finish().is_empty(),
        "no refusal may touch the network"
    );
}

#[test]
fn a_configured_proxy_is_never_bypassed() {
    let data = test_data();
    let target = Peer::start(tls_plan(&data, "normal", b"unused"));
    // nothing listens on the proxy address: the fetch fails to connect to the proxy and never goes to the target directly
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_address = dead.local_addr().unwrap().to_string();
    drop(dead);
    let environment = environment_for(&data, Some(&format!("http://{dead_address}")));
    let outcome = fetch(&format!("https://{}/x", target.address), &environment);
    assert!(
        matches!(&outcome, Err(FetchFailure::Transport(failure)) if failure.phase == "connect"),
        "{outcome:?}"
    );
    assert!(target.finish().is_empty());
}

#[test]
fn failures_without_a_captured_python_spelling_are_transport_failures() {
    let data = test_data();
    let environment = environment_for(&data, None);
    let phase_of = |plan: Plan| {
        let peer = Peer::start(plan);
        let outcome = fetch(&format!("https://{}/x", peer.address), &environment);
        peer.finish();
        match outcome {
            Err(FetchFailure::Transport(failure)) => failure.phase,
            other => panic!("expected a transport failure, got {other:?}"),
        }
    };
    let not_tls = |bytes: &[u8]| Plan {
        proxy: false,
        connect_reply: None,
        stop_after_connect: false,
        behavior: Behavior::NotTls(bytes.to_vec()),
    };
    // a peer that answers a ClientHello with text, and one that closes at once
    assert_eq!(
        phase_of(not_tls(b"HTTP/1.1 400 Bad Request\r\n\r\n")),
        "tls_handshake"
    );
    assert_eq!(phase_of(not_tls(b"")), "tls_handshake");
    // a port nobody listens on
    let dead = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = dead.local_addr().unwrap().to_string();
    drop(dead);
    let outcome = fetch(&format!("https://{address}/x"), &environment);
    assert!(
        matches!(&outcome, Err(FetchFailure::Transport(f)) if f.phase == "connect"),
        "{outcome:?}"
    );
}

#[test]
fn a_redirect_over_tls_is_followed_over_a_new_connection_not_refused() {
    let data = test_data();
    let peer = Peer::start(tls_plan(
        &data,
        "normal",
        b"HTTP/1.1 302 Found\r\nLocation: /elsewhere\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ));
    let outcome = fetch(
        &format!("https://{}/x", peer.address),
        &environment_for(&data, None),
    );
    // the shared opener follows the 302 over new tunnel/TLS connections; this peer answers every hop with the same redirect, so the chain ends
    // where `HTTPRedirectHandler` ends it: `max_repeats` (4) repeats of the target after the first request, five connections, then the
    // "infinite loop" `HTTPError`, never the old refusal by name
    let Err(FetchFailure::Raised(error)) = outcome else {
        panic!("a redirect loop is an HTTPError: {outcome:?}")
    };
    assert_eq!(error.class, "HTTPError");
    assert!(error
        .message
        .to_utf8()
        .unwrap()
        .contains("would lead to an infinite loop"));
    let events = peer.finish();
    assert_eq!(events.iter().filter(|e| **e == Event::TcpAccept).count(), 5);
    // a 30x without a Location is the plain HTTPError of the shared handler
    let bare = Peer::start(tls_plan(
        &data,
        "normal",
        b"HTTP/1.1 301 Moved\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
    ));
    let outcome = fetch(
        &format!("https://{}/x", bare.address),
        &environment_for(&data, None),
    );
    let Err(FetchFailure::Raised(error)) = outcome else {
        panic!("a redirect without Location is an HTTPError")
    };
    assert_eq!(error.class, "HTTPError");
    bare.finish();
}

#[test]
fn the_tunnel_carries_the_logical_host_and_never_the_proxy_address() {
    let data = test_data();
    let proxy = Peer::start(Plan {
        proxy: true,
        ..tls_plan(
            &data,
            "normal",
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
        )
    });
    let environment = environment_for(&data, Some(&format!("http://{}", proxy.address)));
    assert_eq!(
        fetch("https://api.github.com:443/x?q=1", &environment).unwrap(),
        "ok"
    );
    let events = proxy.finish();
    assert_eq!(
        events[1],
        Event::ConnectRequest(
            b"CONNECT api.github.com:443 HTTP/1.1\r\nHost: api.github.com:443\r\n\r\n".to_vec()
        )
    );
    assert_eq!(events[3], Event::TlsSni(Some("api.github.com".to_string())));
    // `Host` of the GET is the URL's host as written (with its port here), the target keeps the query
    assert_eq!(
        events[5],
        Event::HttpRequest(
            b"GET /x?q=1 HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: api.github.com:443\r\nUser-Agent: Python-urllib/3.12\r\nConnection: close\r\n\r\n"
                .to_vec()
        )
    );
}

#[test]
fn the_normal_program_ignores_unrelated_non_unicode_environment_but_not_a_proxy() {
    let data = test_data();
    let peer = Peer::start(tls_plan(
        &data,
        "normal",
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab",
    ));
    let trust = vec![
        (b"SSL_CERT_FILE".to_vec(), path_bytes(&data.trusted_ca)),
        (b"SSL_CERT_DIR".to_vec(), path_bytes(&data.empty_ca_dir)),
    ];
    let mut environment = trust.clone();
    environment.push((b"VAL_AGENT_PUBLIC_HTTP_FIXTURE".to_vec(), vec![0xff]));
    environment.push((b"UNRELATED\xffNAME".to_vec(), vec![0xfe, 0xff]));
    let line = request_line(
        "env-nonunicode",
        OPERATION,
        &format!("https://{}/x", peer.address),
    );
    let run = run_program("env-nonunicode", &[line], &environment);
    assert!(
        run.status.success(),
        "{:?}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stderr.is_empty());
    assert_eq!(kind_of(&run.records[0]), "value");
    assert_eq!(member(&run.records[0], "value").repr(), "['str', 'ab']");
    assert_eq!(
        peer.finish()
            .iter()
            .filter(|e| **e == Event::TlsReady)
            .count(),
        1
    );

    // a known proxy variable whose bytes are not UTF-8 is neither ignored nor used: the program refuses it by name and connects nowhere
    let target = Peer::start(tls_plan(&data, "normal", b"unused"));
    let mut environment = trust;
    environment.push((b"https_proxy".to_vec(), vec![0xff, b':', b'1']));
    environment.push((b"UNRELATED\xffNAME".to_vec(), vec![0xff]));
    let line = request_line(
        "env-proxy",
        OPERATION,
        &format!("https://{}/x", target.address),
    );
    let run = run_program("env-proxy", &[line], &environment);
    assert!(run.status.success());
    assert!(run.stderr.is_empty());
    assert_eq!(kind_of(&run.records[0]), "out_of_scope");
    assert_eq!(
        text_of(member(&run.records[0], "boundary")),
        "proxy_environment"
    );
    assert!(target.finish().is_empty());
}

#[test]
fn the_normal_program_answers_json_lines_and_refuses_other_operations() {
    let lines = [
        request_line("program-scheme", OPERATION, "http://127.0.0.1:9/x"),
        request_line(
            "program-op",
            "resolve_torch_pin.fetch_http_direct",
            "https://127.0.0.1:9/x",
        ),
        request_line("program-arg", OPERATION, "https://127.0.0.1:9/x")
            .replace("\"args_dto\"", "\"kwargs_dto\""),
    ];
    let run = run_program("program-empty-environment", &lines, &[]);
    assert!(
        run.status.success(),
        "{:?}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stderr.is_empty());
    assert_eq!(run.records.len(), 3);
    assert_eq!(kind_of(&run.records[0]), "out_of_scope");
    assert_eq!(text_of(member(&run.records[1], "type")), "RequestError");
    assert_eq!(text_of(member(&run.records[0], "scope")), SCOPE);
}

// ---------------------------------------------------------------------------------------------------------------------------
// The production sources hold no baseline data, no external process and no way around the verification.
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn production_sources_hold_no_saved_data_or_verification_bypass() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(source.join("fetch_https"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.push(source.join("bin/upstream_https_api.rs"));
    files.push(source.join("jsonl_program.rs"));
    assert!(files.len() >= 8);
    for file in files {
        let text = std::fs::read_to_string(&file).unwrap();
        // the unit-test modules inside a file are the test plane; only the production part is scanned
        let production = text.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "fetch-tls-0",
            "cases.json",
            "R35_FETCH",
            "R35_TLS",
            "std::process::Command",
            "python3",
            "\"openssl\"",
            "\"curl\"",
            "std::net::TcpListener",
            "get_json_bytes",
            "dangerous()",
            "danger::",
            "with_custom_certificate_verifier",
            "SSLKEYLOGFILE",
            "verify_hostname",
        ] {
            assert!(
                !production.contains(forbidden),
                "{} holds {forbidden}",
                file.display()
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The request line is checked before any connection: the typed `UnicodeEncodeError` and the provenance of surrogates.
// ---------------------------------------------------------------------------------------------------------------------------

/// A dialer that records that it was asked to connect and refuses.
struct RecordingDialer(AtomicBool);

impl Dial for RecordingDialer {
    fn dial(&self, _host: &str, _port: u16) -> Result<Box<dyn Wire>, TransportFailure> {
        self.0.store(true, Ordering::SeqCst);
        Err(TransportFailure {
            phase: "connect",
            detail: "the test dialer refuses every connection".to_string(),
        })
    }
}

/// `[tag, value]` of a typed value of the service record.
fn typed(value: &PyValue) -> (String, String) {
    let parts = list(value);
    (text_of(&parts[0]), text_of(&parts[1]))
}

fn service_exception(url: &str) -> (PyValue, bool) {
    let dialer = RecordingDialer(AtomicBool::new(false));
    let line = request_line("encode", OPERATION, url);
    let record = answer_with(&PyStr::new(&line), &FetchEnvironment::default(), &dialer);
    assert_eq!(kind_of(&record), "exception", "{}", record.repr());
    (
        member(&record, "exception").clone(),
        dialer.0.load(Ordering::SeqCst),
    )
}

fn assert_full_unicode_encode_error(exception: &PyValue, object: &str, message: &str) {
    assert_eq!(text_of(member(exception, "type")), "UnicodeEncodeError");
    assert_eq!(text_of(member(exception, "module")), "builtins");
    assert_eq!(text_of(member(exception, "message")), message);
    let args = list(member(exception, "args"));
    let shown: Vec<(String, String)> = args.iter().map(typed).collect();
    assert_eq!(
        shown,
        vec![
            ("str".to_string(), "ascii".to_string()),
            ("str".to_string(), object.to_string()),
            ("int".to_string(), "5".to_string()),
            ("int".to_string(), "6".to_string()),
            ("str".to_string(), "ordinal not in range(128)".to_string()),
        ]
    );
    let attributes = member(exception, "attributes");
    for (name, expected) in [
        ("encoding", ("str", "ascii")),
        ("object", ("str", object)),
        ("start", ("int", "5")),
        ("end", ("int", "6")),
        ("reason", ("str", "ordinal not in range(128)")),
    ] {
        assert_eq!(
            typed(member(attributes, name)),
            (expected.0.to_string(), expected.1.to_string()),
            "attribute {name}"
        );
    }
}

#[test]
fn a_non_ascii_url_is_the_typed_unicode_encode_error_before_any_dial() {
    let (exception, dialed) = service_exception("https://example.test/\u{e9}");
    assert!(
        !dialed,
        "no connection is made before the request line is encoded"
    );
    assert_full_unicode_encode_error(
        &exception,
        "GET /\u{e9} HTTP/1.1",
        "'ascii' codec can't encode character '\\xe9' in position 5: ordinal not in range(128)",
    );
}

#[test]
fn a_genuine_private_use_url_character_is_not_reinterpreted_as_a_surrogate() {
    let (exception, dialed) = service_exception("https://example.test/\u{f0000}");
    assert!(!dialed);
    assert_full_unicode_encode_error(
        &exception,
        "GET /\u{f0000} HTTP/1.1",
        "'ascii' codec can't encode character '\\U000f0000' in position 5: ordinal not in range(128)",
    );
}

#[test]
fn a_python_text_with_a_lone_surrogate_is_carried_explicitly_and_keeps_both_kinds_apart() {
    let dialer = RecordingDialer(AtomicBool::new(false));
    let environment = FetchEnvironment::default();
    let text = |points: &[u32]| PyStr::from_code_points(points.to_vec());
    let prefix: Vec<u32> = "https://example.test/".chars().map(u32::from).collect();
    let url = |tail: &[u32]| text(&[prefix.clone(), tail.to_vec()].concat());

    // a lone surrogate alone: the original's message names the surrogate
    let Err(FetchFailure::Raised(error)) =
        fetch_https_text_of(&url(&[0xDCFF]), &environment, &dialer)
    else {
        panic!("a lone surrogate cannot be encoded")
    };
    assert_eq!(
        error.message.to_utf8().unwrap(),
        "'ascii' codec can't encode character '\\udcff' in position 5: ordinal not in range(128)"
    );
    // a surrogate next to a genuine U+F0000: one run of two characters, each kept as what it is
    let Err(FetchFailure::Raised(error)) =
        fetch_https_text_of(&url(&[0xF0000, 0xDCFF]), &environment, &dialer)
    else {
        panic!("both characters cannot be encoded")
    };
    assert_eq!(
        error.message.to_utf8().unwrap(),
        "'ascii' codec can't encode characters in position 5-6: ordinal not in range(128)"
    );
    let ErrorValue::Plain(PyValue::Str(object)) = &error.args[1] else {
        panic!("the second argument is the encoded text")
    };
    let mut expected: Vec<u32> = "GET /".chars().map(u32::from).collect();
    expected.extend_from_slice(&[0xF0000, 0xDCFF]);
    expected.extend(" HTTP/1.1".chars().map(u32::from));
    assert_eq!(object.code_points(), expected.as_slice());
    assert!(!dialer.0.load(Ordering::SeqCst));
}
