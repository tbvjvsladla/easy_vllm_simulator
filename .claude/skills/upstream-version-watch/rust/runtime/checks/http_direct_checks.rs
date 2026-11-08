//! Checks of the direct-HTTP plain-str URL component (R35 stage B1).
//!
//! * `saved_original_cases_through_the_json_service`: the 38 saved original records (typed value, exception chain with its constructor
//!   arguments, captured attributes and headers, arguments after the call, streams) and the saved owned-peer request bytes, driven through the
//!   production JSON-lines service over real sockets to an owned `127.0.0.1` peer. The saved base `http://127.0.0.1:<port>` is the ONLY thing
//!   rebased (to the new peer's base); frames and the fp state of `HTTPError` are interpreter observations a native component does not have.
//! * the typed API, the scope refusals, buffering/progress behaviour (a response in small delayed pieces; a body that ends by its declared
//!   length while the peer keeps the connection open), the normal program as a subprocess with an empty proxy environment, and a scan that the
//!   production sources hold no baseline data.
//!
//! The saved reference is read through the environment variable `R35_FETCH_HTTP_REFERENCE` (the directory with `cases.json` and
//! `fetch-http-001` ... `fetch-http-038`), the nine supplement originals through `R35_FETCH_HTTP_SUPPLEMENT_REFERENCE`, and the optional
//! `R35_REPAIR_LOG_DIR` names a directory that keeps the pid, first exit status and raw streams of each normal-program subprocess; production
//! code contains no path, label or expected value. Every socket is on `127.0.0.1` and owned
//! by the test; no external endpoint, Python, benchmark or original is started.

use agent_runtime_support::pyjson::loads;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use upstream_runtime::http_direct::jsonl::{answer_with, OPERATION};
use upstream_runtime::http_direct::{
    fetch_http_direct_text_with, FetchFailure, ProxyEnvironment, UreqDialer,
};

fn reference() -> PathBuf {
    PathBuf::from(std::env::var("R35_FETCH_HTTP_REFERENCE").expect(
        "R35_FETCH_HTTP_REFERENCE must name the saved reference directory (cases.json, fetch-http-001 ...)",
    ))
}

/// The saved supplement reference (nine more originals: N1/N2/N3 evidence), `cases.json` and `fetch-http-supplement-001` ...
fn supplement_reference() -> PathBuf {
    PathBuf::from(std::env::var("R35_FETCH_HTTP_SUPPLEMENT_REFERENCE").expect(
        "R35_FETCH_HTTP_SUPPLEMENT_REFERENCE must name the saved supplement directory (cases.json, fetch-http-supplement-001 ...)",
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

// ---------------------------------------------------------------------------------------------------------------------------
// The owned loopback peer: it reads one request head per connection, records it, sends the planned response, then closes (or lingers).
// ---------------------------------------------------------------------------------------------------------------------------

/// What the peer sends for one request.
#[derive(Clone)]
struct Reply {
    pieces: Vec<Vec<u8>>,
    pause_between_pieces: Duration,
    /// Keep the connection open after the last piece until the test releases it (the peer never closes first).
    linger: bool,
}

impl Reply {
    fn raw(bytes: &[u8]) -> Reply {
        Reply {
            pieces: if bytes.is_empty() {
                Vec::new()
            } else {
                vec![bytes.to_vec()]
            },
            pause_between_pieces: Duration::ZERO,
            linger: false,
        }
    }
}

struct Peer {
    base: String,
    /// Connections that carried no complete request head (the test's own wake-up connection at `finish`, an aborted client).
    cleanup_connections: Arc<AtomicUsize>,
    requests: Arc<Mutex<Vec<Vec<u8>>>>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    address: String,
    thread: Option<std::thread::JoinHandle<()>>,
}

fn read_request_head(stream: &mut TcpStream) -> Option<Vec<u8>> {
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let mut head = Vec::new();
    let mut chunk = [0u8; 4096];
    while !head.windows(4).any(|window| window == b"\r\n\r\n") {
        let count = stream.read(&mut chunk).ok()?;
        if count == 0 {
            return None;
        }
        head.extend_from_slice(&chunk[..count]);
        assert!(head.len() <= 65536, "peer request head bound exceeded");
    }
    Some(head)
}

impl Peer {
    fn start(replies: Vec<Reply>) -> Peer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("owned loopback listener");
        let address = listener.local_addr().expect("address").to_string();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let release = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let cleanup_connections = Arc::new(AtomicUsize::new(0));
        let thread = {
            let (requests, release, stop, cleanup) = (
                requests.clone(),
                release.clone(),
                stop.clone(),
                cleanup_connections.clone(),
            );
            std::thread::spawn(move || {
                for incoming in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        cleanup.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                    let Ok(mut stream) = incoming else { continue };
                    let Some(head) = read_request_head(&mut stream) else {
                        cleanup.fetch_add(1, Ordering::SeqCst);
                        continue;
                    };
                    let index = {
                        let mut seen = requests.lock().unwrap();
                        seen.push(head);
                        seen.len() - 1
                    };
                    let reply = &replies[index.min(replies.len() - 1)];
                    for (at, piece) in reply.pieces.iter().enumerate() {
                        if at > 0 {
                            std::thread::sleep(reply.pause_between_pieces);
                        }
                        let _ = stream.write_all(piece);
                        let _ = stream.flush();
                    }
                    if reply.linger {
                        for _ in 0..3000 {
                            if release.load(Ordering::SeqCst) {
                                break;
                            }
                            std::thread::sleep(Duration::from_millis(10));
                        }
                    }
                }
            })
        };
        Peer {
            base: format!("http://{address}"),
            cleanup_connections,
            address,
            requests,
            release,
            stop,
            thread: Some(thread),
        }
    }

    fn raw(responses: &[&[u8]]) -> Peer {
        Peer::start(responses.iter().map(|bytes| Reply::raw(bytes)).collect())
    }

    fn requests(&self) -> Vec<Vec<u8>> {
        self.requests.lock().unwrap().clone()
    }

    fn finish(self) -> Vec<Vec<u8>> {
        self.finish_counting_cleanup().0
    }

    /// The request-bearing requests, and how many connections carried none (distinct from the request count).
    fn finish_counting_cleanup(mut self) -> (Vec<Vec<u8>>, usize) {
        self.release.store(true, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(&self.address);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("peer thread");
        }
        (
            self.requests(),
            self.cleanup_connections.load(Ordering::SeqCst),
        )
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// Driving the production JSON service.
// ---------------------------------------------------------------------------------------------------------------------------

fn no_proxies() -> ProxyEnvironment {
    ProxyEnvironment::from_pairs(Vec::new())
}

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

fn ask(line: &str) -> PyValue {
    answer_with(&PyStr::new(line), &no_proxies(), &UreqDialer::new())
}

fn ask_url(id: &str, url: &str) -> PyValue {
    ask(&request_line(id, OPERATION, url))
}

/// The saved exception record without the interpreter-only observations (frames, the fp state of `HTTPError`), recursively.
fn without_interpreter_observations(value: &PyValue) -> PyValue {
    let PyValue::Dict(map) = value else {
        return value.clone();
    };
    let mut kept = PyDict::new();
    for (key, item) in map.iter() {
        let name = key.to_utf8().expect("key");
        if name == "frames" || name == "http_fp_state" {
            continue;
        }
        kept.set(
            &name,
            if name == "next" {
                without_interpreter_observations(item)
            } else {
                item.clone()
            },
        );
    }
    PyValue::Dict(kept)
}

/// The saved facts of one case, rebased to the new peer.
struct Saved {
    id: String,
    original: PyValue,
    peer_requests: Vec<Vec<u8>>,
    old_base: String,
}

fn load_case(root: &Path, case: &PyValue) -> (Saved, Vec<Vec<u8>>) {
    let id = text_of(member(case, "id"));
    let folder = root.join(&id);
    let original = read_json(folder.join("original.json"));
    let peer = read_json(folder.join("peer.json"));
    let responses = list(member(member(case, "peer"), "responses"))
        .iter()
        .map(|response| hex_decode(&text_of(member(response, "raw_hex"))))
        .collect();
    let peer_requests = list(member(&peer, "events"))
        .iter()
        .map(|event| hex_decode(&text_of(member(event, "request_hex"))))
        .collect();
    let old_base = text_of(member(&peer, "base"));
    (
        Saved {
            id,
            original,
            peer_requests,
            old_base,
        },
        responses,
    )
}

fn replace_bytes(haystack: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8_lossy(haystack)
        .replace(from, to)
        .into_bytes()
}

/// What a pass over one saved reference counted.
#[derive(Default)]
struct Totals {
    logical_cases: usize,
    values: usize,
    exceptions: usize,
    /// Request-bearing HTTP connections the owned peers recorded.
    connections: usize,
    /// GETs the saved peers recorded for the same cases.
    saved_requests: usize,
    /// Connections that carried no request (the test's wake-up connection at each peer's end).
    cleanup_connections: usize,
}

/// Drives every saved case of `root` through `run` (which returns the service record for the case, given its URL) against a fresh owned peer
/// and compares the whole record and the peer's request bytes with the saved original. Only the saved peer base is rebased.
fn drive_saved_cases(root: &Path, run: &dyn Fn(&PyValue, &str) -> PyValue) -> Totals {
    let directory = read_json(root.join("cases.json"));
    let mut totals = Totals::default();
    for case in list(member(&directory, "cases")) {
        let (saved, responses) = load_case(root, case);
        let peer = Peer::start(responses.iter().map(|bytes| Reply::raw(bytes)).collect());
        let url = format!("{}/x", peer.base);
        let record = run(case, &url);
        let (wire, cleanup) = peer.finish_counting_cleanup();
        totals.logical_cases += 1;
        totals.cleanup_connections += cleanup;

        // whole record against the saved original
        let rebase = |text: String| text.replace(&saved.old_base, &peer_base_of(&url));
        assert_eq!(text_of(member(&record, "id")), saved.id);
        let saved_kind = text_of(member(&saved.original, "kind"));
        assert_eq!(text_of(member(&record, "kind")), saved_kind, "{}", saved.id);
        match saved_kind.as_str() {
            "value" => {
                totals.values += 1;
                assert_eq!(
                    member(&record, "value").repr(),
                    member(&saved.original, "value").repr(),
                    "{}: returned text",
                    saved.id
                );
            }
            "exception" => {
                totals.exceptions += 1;
                let expected =
                    without_interpreter_observations(member(&saved.original, "exception"));
                assert_eq!(
                    member(&record, "exception").repr(),
                    rebase(expected.repr()),
                    "{}: exception chain",
                    saved.id
                );
            }
            other => panic!("{}: unexpected saved kind {other}", saved.id),
        }
        assert_eq!(
            text_of(member(&record, "stdout")),
            text_of(member(&saved.original, "stdout"))
        );
        assert_eq!(
            text_of(member(&record, "stderr")),
            text_of(member(&saved.original, "stderr"))
        );
        // the saved `before.url` is the argument; the argument is unchanged after the call
        let saved_url = member(member(&saved.original, "before"), "url");
        assert_eq!(
            member(&record, "args_after").repr(),
            format!("['list', [{}]]", rebase(saved_url.repr())),
            "{}: arguments after the call",
            saved.id
        );

        // the exact GET bytes the owned peer received, after rebasing the base only
        assert_eq!(
            wire.len(),
            saved.peer_requests.len(),
            "{}: request count",
            saved.id
        );
        totals.connections += wire.len();
        totals.saved_requests += saved.peer_requests.len();
        let (old_host, new_host) = (
            saved.old_base.trim_start_matches("http://").to_string(),
            peer_base_of(&url).trim_start_matches("http://").to_string(),
        );
        for (got, expected) in wire.iter().zip(&saved.peer_requests) {
            assert_eq!(
                got,
                &replace_bytes(expected, &old_host, &new_host),
                "{}: request bytes",
                saved.id
            );
        }
    }
    totals
}

#[test]
fn saved_original_cases_through_the_json_service() {
    let totals = drive_saved_cases(&reference(), &|case, url| {
        ask_url(&text_of(member(case, "id")), url)
    });
    println!(
        "R35 http_direct: {} logical cases ({} values, {} exceptions), {} request-bearing HTTP connections, {} saved owned GETs, {} cleanup connections without a request",
        totals.logical_cases,
        totals.values,
        totals.exceptions,
        totals.connections,
        totals.saved_requests,
        totals.cleanup_connections
    );
    assert_eq!(
        totals.logical_cases, 38,
        "the frozen reference holds 38 logical cases"
    );
    assert_eq!((totals.values, totals.exceptions), (19, 19));
    assert_eq!(totals.connections, totals.saved_requests);
    assert_eq!(totals.connections, 44);
    assert_eq!(totals.cleanup_connections, 38);
}

/// The normal program as a subprocess: its pid, first exit status and raw streams are kept under `R35_REPAIR_LOG_DIR` when that
/// (test-only) variable names a directory, as `<tag>.{pid,exit,stdout.bin,stderr.bin,environment}`.
struct ProgramRun {
    records: Vec<PyValue>,
    status: std::process::ExitStatus,
    stderr: Vec<u8>,
}

fn run_program(tag: &str, lines: &[String], environment: &[(Vec<u8>, Vec<u8>)]) -> ProgramRun {
    use std::os::unix::ffi::OsStringExt;
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_upstream_http_direct_api"));
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
    if let Ok(directory) = std::env::var("R35_REPAIR_LOG_DIR") {
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

/// A supplement case runs through the production program when it names an actor environment (the saved original ran with it), else in process.
fn run_supplement_case(case: &PyValue, url: &str) -> PyValue {
    let id = text_of(member(case, "id"));
    let PyValue::Dict(members) = case else {
        panic!("case is an object")
    };
    let Some(environment) = members.get("actor_environment_hex") else {
        return ask_url(&id, url);
    };
    let pairs: Vec<(Vec<u8>, Vec<u8>)> = list(environment)
        .iter()
        .map(|entry| {
            (
                hex_decode(&text_of(member(entry, "key_hex"))),
                hex_decode(&text_of(member(entry, "value_hex"))),
            )
        })
        .collect();
    let mut run = run_program(&id, &[request_line(&id, OPERATION, url)], &pairs);
    assert!(run.status.success(), "{id}: {:?}", run.stderr);
    assert!(run.stderr.is_empty(), "{id}: program stderr");
    assert_eq!(run.records.len(), 1);
    run.records.remove(0)
}

#[test]
fn saved_supplement_cases_through_the_json_service_and_the_program() {
    let totals = drive_saved_cases(&supplement_reference(), &run_supplement_case);
    println!(
        "R35 http_direct supplement: {} logical cases ({} values, {} exceptions), {} request-bearing HTTP connections, {} saved owned GETs, {} cleanup connections without a request",
        totals.logical_cases,
        totals.values,
        totals.exceptions,
        totals.connections,
        totals.saved_requests,
        totals.cleanup_connections
    );
    assert_eq!(totals.logical_cases, 9);
    assert_eq!((totals.values, totals.exceptions), (7, 2));
    assert_eq!(totals.connections, totals.saved_requests);
    assert_eq!(totals.connections, 9);
}

#[test]
fn the_normal_program_ignores_unrelated_non_unicode_environment_but_not_a_proxy() {
    // N3: unrelated entries whose name and value are not valid UTF-8 must not abort the process.
    let peer = Peer::raw(&[b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nab"]);
    let line = request_line("env-nonunicode", OPERATION, &format!("{}/x", peer.base));
    let environment = vec![
        (b"VAL_AGENT_PUBLIC_HTTP_FIXTURE".to_vec(), vec![0xff]),
        (b"UNRELATED\xffNAME".to_vec(), vec![0xfe, 0xff]),
    ];
    let run = run_program("env-nonunicode", &[line], &environment);
    assert!(
        run.status.success(),
        "{:?}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stderr.is_empty());
    assert_eq!(kind_of(&run.records[0]), "value");
    assert_eq!(member(&run.records[0], "value").repr(), "['str', 'ab']");
    assert_eq!(peer.finish().len(), 1);

    // an http proxy variable is read from the real environment too, whatever bytes its value holds
    let line = request_line("env-proxy", OPERATION, "http://127.0.0.1:9/x");
    let environment = vec![
        (b"UNRELATED\xffNAME".to_vec(), vec![0xff]),
        (b"http_proxy".to_vec(), vec![0xff, 0xfe]),
    ];
    let run = run_program("env-proxy", &[line], &environment);
    assert!(run.status.success());
    assert!(run.stderr.is_empty());
    assert_eq!(kind_of(&run.records[0]), "out_of_scope");
    assert_eq!(
        text_of(member(&run.records[0], "boundary")),
        "proxy_environment"
    );
}

fn peer_base_of(url: &str) -> String {
    url.trim_end_matches("/x").to_string()
}

// ---------------------------------------------------------------------------------------------------------------------------
// The typed API.
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn the_typed_api_returns_text_and_the_exception_chain() {
    let peer = Peer::raw(&[b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\n\r\na\r\nb\rc\n"]);
    let text = fetch_http_direct_text_with(
        &format!("{}/x", peer.base),
        &no_proxies(),
        &UreqDialer::new(),
    )
    .expect("text");
    assert_eq!(text, "a\r\nb\rc\n");
    peer.finish();

    let peer = Peer::raw(&[
        b"HTTP/1.1 404 Caf\xe9\r\nContent-Length: 10\r\nConnection: close\r\n\r\nerror-body",
    ]);
    let url = format!("{}/x", peer.base);
    let Err(FetchFailure::Raised(error)) =
        fetch_http_direct_text_with(&url, &no_proxies(), &UreqDialer::new())
    else {
        panic!("expected HTTPError");
    };
    assert_eq!((error.class, error.module), ("HTTPError", "urllib.error"));
    assert_eq!(
        error.message.to_utf8().unwrap(),
        "HTTP Error 404: Caf\u{e9}"
    );
    assert_eq!(
        error.http_headers.as_deref(),
        Some(
            &[
                ("Content-Length".to_string(), "10".to_string()),
                ("Connection".to_string(), "close".to_string())
            ][..]
        )
    );
    peer.finish();
}

#[test]
fn a_redirect_chain_follows_location_and_keeps_the_wire_shape() {
    let peer = Peer::raw(&[
        b"HTTP/1.1 301 Moved\r\nURI: /by-uri\r\nContent-Length: 0\r\n\r\n",
        b"HTTP/1.1 307 Again\r\nLocation: /a b/\xe9?q=1\r\nContent-Length: 0\r\n\r\n",
        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok",
    ]);
    let url = format!("{}/start", peer.base);
    let text = fetch_http_direct_text_with(&url, &no_proxies(), &UreqDialer::new()).expect("text");
    assert_eq!(text, "ok");
    let wire = peer.finish();
    let lines: Vec<String> = wire
        .iter()
        .map(|head| {
            String::from_utf8_lossy(head)
                .lines()
                .next()
                .unwrap()
                .to_string()
        })
        .collect();
    assert_eq!(
        lines,
        [
            "GET /start HTTP/1.1",
            "GET /by-uri HTTP/1.1",
            "GET /a%20b/%E9?q=1 HTTP/1.1"
        ]
    );
}

// ---------------------------------------------------------------------------------------------------------------------------
// Scope boundaries: named refusals, never guesses and never Python.
// ---------------------------------------------------------------------------------------------------------------------------

fn kind_of(record: &PyValue) -> String {
    text_of(member(record, "kind"))
}

#[test]
fn other_schemes_urls_and_proxies_are_refused_by_name() {
    for url in [
        "https://127.0.0.1:9/x",
        "file:///etc/hostname",
        "ftp://127.0.0.1/x",
        "data:text/plain,hi",
    ] {
        let record = ask_url("scheme", url);
        assert_eq!(kind_of(&record), "out_of_scope", "{url}");
        assert_eq!(text_of(member(&record, "boundary")), "scheme");
    }
    // a URL without a scheme is a ValueError of urllib's Request, as in the original
    let record = ask_url("noscheme", "no-scheme/x");
    assert_eq!(kind_of(&record), "exception");
    assert_eq!(
        text_of(member(member(&record, "exception"), "message")),
        "unknown url type: 'no-scheme/x'"
    );
    // a non-str URL
    let mut row = PyDict::new();
    row.set("id", PyValue::str("nonstr"));
    row.set("operation", PyValue::str(OPERATION));
    row.set(
        "args_dto",
        PyValue::List(vec![
            PyValue::str("list"),
            PyValue::List(vec![PyValue::List(vec![
                PyValue::str("bytes"),
                PyValue::str("6162"),
            ])]),
        ]),
    );
    let record = ask(&PyValue::Dict(row).dumps_ascii().unwrap());
    assert_eq!(kind_of(&record), "out_of_scope");
    assert_eq!(text_of(member(&record, "boundary")), "non_str_url");
    // an http proxy in the environment is refused, not bypassed
    let proxied = ProxyEnvironment::from_pairs([(
        "http_proxy".to_string(),
        "http://127.0.0.1:9".to_string(),
    )]);
    let line = request_line("proxy", OPERATION, "http://127.0.0.1:9/x");
    let record = answer_with(&PyStr::new(&line), &proxied, &UreqDialer::new());
    assert_eq!(kind_of(&record), "out_of_scope");
    assert_eq!(text_of(member(&record, "boundary")), "proxy_environment");
}

#[test]
fn a_redirect_out_of_scope_is_refused_after_the_first_connection() {
    let peer = Peer::raw(&[
        b"HTTP/1.1 302 Found\r\nLocation: https://127.0.0.1:9/z\r\nContent-Length: 0\r\n\r\n",
    ]);
    let record = ask_url("to-https", &format!("{}/x", peer.base));
    assert_eq!(kind_of(&record), "out_of_scope");
    assert_eq!(text_of(member(&record, "boundary")), "scheme");
    assert_eq!(peer.finish().len(), 1);

    let peer = Peer::raw(&[
        b"HTTP/1.1 302 Found\r\nLocation: file:///etc/hostname\r\nContent-Length: 0\r\n\r\n",
    ]);
    let record = ask_url("to-file", &format!("{}/x", peer.base));
    assert_eq!(kind_of(&record), "exception");
    let error = member(&record, "exception");
    assert_eq!(text_of(member(error, "type")), "HTTPError");
    assert_eq!(
        text_of(member(error, "message")),
        "HTTP Error 302: Found - Redirection to url 'file:///etc/hostname' is not allowed"
    );
    peer.finish();
}

#[test]
fn bad_requests_fail_explicitly() {
    let record = ask("not json");
    assert_eq!(text_of(member(&record, "type")), "RequestError");
    let record = ask(&request_line(
        "op",
        "resolve_torch_pin.fetch",
        "http://127.0.0.1:9/x",
    ));
    assert_eq!(text_of(member(&record, "type")), "RequestError");
    assert!(text_of(member(&record, "message")).contains("operation must be"));
    let record = ask(&format!(
        r#"{{"id":"none","operation":"{OPERATION}","args_dto":["list",[]]}}"#
    ));
    assert_eq!(text_of(member(&record, "type")), "RequestError");
    assert!(text_of(member(&record, "message")).contains("exactly one argument"));
}

#[test]
fn a_refused_connection_is_a_transport_failure_not_a_python_exception() {
    let address = {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.local_addr().unwrap()
    };
    let record = ask_url("refused", &format!("http://{address}/x"));
    assert_eq!(kind_of(&record), "transport_failure");
    assert_eq!(text_of(member(&record, "phase")), "connect");
}

// ---------------------------------------------------------------------------------------------------------------------------
// Buffering, progress and end of stream on the real transport.
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn a_response_in_small_delayed_pieces_is_assembled() {
    let whole: Vec<u8> =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Split: a\r\n b\r\n\r\n5;ext=1\r\nhel"
            .to_vec();
    let mut pieces: Vec<Vec<u8>> = whole.chunks(7).map(<[u8]>::to_vec).collect();
    pieces.push(b"lo\r\n4\r\n \xe2\x82\xac\r\n0\r\nT: v\r\n\r\n".to_vec());
    let peer = Peer::start(vec![Reply {
        pieces,
        pause_between_pieces: Duration::from_millis(5),
        linger: false,
    }]);
    let text = fetch_http_direct_text_with(
        &format!("{}/x", peer.base),
        &no_proxies(),
        &UreqDialer::new(),
    )
    .expect("text");
    assert_eq!(text, "hello \u{20ac}");
    peer.finish();
}

#[test]
fn a_declared_length_ends_the_body_without_waiting_for_the_peer_to_close() {
    // The peer keeps the connection open until released: a reader that read to end of stream would not return until the 20 s socket timeout.
    for response in [
        &b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello"[..],
        &b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n"[..],
    ] {
        let peer = Peer::start(vec![Reply {
            pieces: vec![response.to_vec()],
            pause_between_pieces: Duration::ZERO,
            linger: true,
        }]);
        let outcome = fetch_http_direct_text_with(
            &format!("{}/x", peer.base),
            &no_proxies(),
            &UreqDialer::new(),
        );
        assert_eq!(outcome.expect("text"), "hello");
        peer.finish();
    }
}

// ---------------------------------------------------------------------------------------------------------------------------
// The normal program, as a subprocess with an empty environment.
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn the_normal_program_answers_json_lines_with_an_empty_environment() {
    let peer = Peer::raw(&[b"HTTP/1.1 200 OK\r\nContent-Length: 14\r\nConnection: close\r\n\r\n\xed\x95\x9c\xea\xb8\x80 \xc3\xa9 \xf0\x9f\x98\x80"]);
    let lines = [
        request_line("program-value", OPERATION, &format!("{}/x", peer.base)),
        request_line("program-scheme", OPERATION, "https://127.0.0.1:9/x"),
        request_line(
            "program-op",
            "resolve_torch_pin.fetch",
            "http://127.0.0.1:9/x",
        ),
    ];
    let run = run_program("program-empty-environment", &lines, &[]);
    assert!(
        run.status.success(),
        "{:?}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert!(run.stderr.is_empty());
    let lines = run.records;
    assert_eq!(lines.len(), 3);
    assert_eq!(kind_of(&lines[0]), "value");
    assert_eq!(
        member(&lines[0], "value").repr(),
        "['str', '\u{d55c}\u{ae00} \u{e9} \u{1f600}']"
    );
    assert_eq!(kind_of(&lines[1]), "out_of_scope");
    assert_eq!(text_of(member(&lines[2], "type")), "RequestError");
    assert_eq!(peer.finish().len(), 1);
}

// ---------------------------------------------------------------------------------------------------------------------------
// The production sources hold no baseline data and no way around the component.
// ---------------------------------------------------------------------------------------------------------------------------

#[test]
fn production_sources_hold_no_saved_data_or_python_route() {
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory.join("http_direct"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    files.push(directory.join("bin/upstream_http_direct_api.rs"));
    assert!(files.len() >= 13);
    for file in files {
        let source = std::fs::read_to_string(&file).unwrap();
        // the unit-test modules inside a file are the test plane; only the production part is scanned
        let production = source.split("#[cfg(test)]").next().unwrap();
        for forbidden in [
            "fetch-http-0",
            "cases.json",
            "R35_FETCH_HTTP",
            "std::process::Command",
            "python3",
            "canonical_reason",
            "std::net::TcpListener",
            "get_json_bytes",
        ] {
            assert!(
                !production.contains(forbidden),
                "{} holds {forbidden}",
                file.display()
            );
        }
    }
}
