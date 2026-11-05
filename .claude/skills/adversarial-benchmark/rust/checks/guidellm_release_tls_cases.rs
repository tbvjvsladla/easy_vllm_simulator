//! The live HTTPS client of `resolve_guidellm`, exercised for real: the actual binary talks to a loopback TLS server of
//! this check through the proxy variable (`HTTPS_PROXY` -> a `CONNECT api.github.com:443` tunnel -> TLS), trusting only the
//! temporary CA this check creates (`SSL_CERT_FILE`/`SSL_CERT_DIR` of the child alone; nothing global is touched, no
//! certificate check is bypassed). The fixed real URL of the original is kept: the proxy never resolves or forwards the
//! host, so no request leaves this machine.
//!
//! Scenarios: a normal answer, a UTF-16 answer with a byte-order mark, an answer past ten million bytes (there is no
//! default body cap), an `HTTPError` (404), and a server whose certificate the child does not trust (the lookup must
//! fail, never fall back and never skip verification). The certificates are generated at run time with the `openssl`
//! command (`NOT_RUN` if it is absent); the server side is `rustls`.

#[path = "guidellm_release_tls_common.rs"]
mod common;

use common::*;
use std::io::Read;
use std::net::TcpListener;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const BINARY: &str = env!("CARGO_BIN_EXE_resolve_guidellm");

struct Outcome {
    status: Option<i32>,
    stdout: String,
    stderr: String,
    seen: Result<Seen, String>,
}

/// Runs the binary against one complete server answer. The child trusts `trusted_ca` only.
fn run_scenario(
    server: &Authority,
    trusted_ca: &Path,
    status_line: &'static str,
    body: Vec<u8>,
    args: &[&str],
) -> Outcome {
    run_reply(
        server,
        trusted_ca,
        Reply::Complete { status_line, body },
        args,
    )
}

/// Runs the binary against one server reply. The child trusts `trusted_ca` only.
fn run_reply(server: &Authority, trusted_ca: &Path, reply: Reply, args: &[&str]) -> Outcome {
    let work = Scratch::new("child");
    let empty_directory = work.0.join("empty-cert-directory");
    std::fs::create_dir_all(&empty_directory).expect("empty directory");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("address").port();
    let handle = serve_once(listener, server_config(server), reply);
    let mut command = Command::new(BINARY);
    command
        .args(args)
        .current_dir(&work.0)
        .env_clear()
        .env("HTTPS_PROXY", format!("http://127.0.0.1:{port}"))
        .env("SSL_CERT_FILE", trusted_ca)
        .env("SSL_CERT_DIR", &empty_directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().expect("wait") {
            break status.code();
        }
        if started.elapsed() > CLIENT_PATIENCE {
            let _ = child.kill();
            let _ = child.wait();
            panic!("the client did not finish within {CLIENT_PATIENCE:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    let (mut stdout, mut stderr) = (String::new(), String::new());
    child
        .stdout
        .take()
        .expect("stdout")
        .read_to_string(&mut stdout)
        .expect("stdout text");
    child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .expect("stderr text");
    let seen = handle.join().expect("server thread");
    Outcome {
        status,
        stdout,
        stderr,
        seen,
    }
}

const RELEASE: &str = "{\"tag_name\":\"v0.9.1\",\"published_at\":\"2026-08-30T12:00:00Z\"}";

#[test]
fn a_normal_answer_resolves_the_latest_release_over_real_tls_with_the_original_request() {
    let folder = Scratch::new("normal");
    let server = authority(&folder.0.join("ca"), "normal");
    let run = run_scenario(
        &server,
        &server.ca_pem,
        "200 OK",
        RELEASE.as_bytes().to_vec(),
        &["--json"],
    );
    assert_eq!(run.status, Some(0), "{}", run.stderr);
    assert!(
        run.stdout.contains("\"version\": \"0.9.1\""),
        "{}",
        run.stdout
    );
    assert!(run
        .stdout
        .contains("\"published_utc\": \"2026-08-30T12:00:00Z\""));
    assert!(run.stdout.contains(
        "\"version_source\": \"measured(GitHub Releases latest · vllm-project/guidellm)\""
    ));
    let seen = run.seen.expect("the server saw the request");
    assert_eq!(seen.connect, "CONNECT api.github.com:443 HTTP/1.1");
    assert_eq!(
        seen.request_line,
        "GET /repos/vllm-project/guidellm/releases/latest HTTP/1.1"
    );
    assert_eq!(header(&seen, "accept"), Some("application/vnd.github+json"));
    assert_eq!(header(&seen, "user-agent"), Some("adversarial-benchmark"));
    assert_eq!(header(&seen, "host"), Some("api.github.com"));
    assert_eq!(header(&seen, "accept-encoding"), Some("identity"));
}

#[test]
fn a_utf16_answer_with_a_byte_order_mark_and_a_body_past_ten_million_bytes_are_read_whole() {
    let folder = Scratch::new("bodies");
    let server = authority(&folder.0.join("ca"), "bodies");
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "{\"tag_name\":\"v1.2.3\"}".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    let run = run_scenario(&server, &server.ca_pem, "200 OK", utf16, &["--json"]);
    assert_eq!(run.status, Some(0), "{}", run.stderr);
    assert!(
        run.stdout.contains("\"version\": \"1.2.3\""),
        "{}",
        run.stdout
    );
    let padding = "x".repeat(11 * 1024 * 1024);
    let big = format!("{{\"tag_name\":\"v2.0.0\",\"padding\":\"{padding}\"}}");
    let run = run_scenario(
        &server,
        &server.ca_pem,
        "200 OK",
        big.into_bytes(),
        &["--json", "--repo", "fixture/big"],
    );
    assert_eq!(run.status, Some(0), "{}", run.stderr);
    assert!(
        run.stdout.contains("\"version\": \"2.0.0\""),
        "no default body cap"
    );
    assert_eq!(
        run.seen.expect("seen").request_line,
        "GET /repos/fixture/big/releases/latest HTTP/1.1"
    );
}

#[test]
fn an_http_error_status_is_unresolved_with_the_original_text_and_exit_3() {
    let folder = Scratch::new("status");
    let server = authority(&folder.0.join("ca"), "status");
    let run = run_scenario(
        &server,
        &server.ca_pem,
        "404 Not Found",
        b"{}".to_vec(),
        &["--json"],
    );
    assert_eq!(run.status, Some(3));
    assert_eq!(
        run.stderr,
        "[resolve_guidellm] UNRESOLVED 최신 릴리즈를 조회하지 못했다(HTTPError: HTTP Error 404: Not Found).\n  → 조용히 옛 버전으로 떨어지지 않는다. 쓸 버전을 선언하라: --version <x.y.z>\n"
    );
    assert!(run.stdout.is_empty());
}

#[test]
fn a_server_certificate_the_client_does_not_trust_fails_the_lookup_instead_of_skipping_verification(
) {
    let folder = Scratch::new("untrusted");
    let server = authority(&folder.0.join("server"), "server");
    let other = authority(&folder.0.join("other"), "other");
    let run = run_scenario(
        &server,
        &other.ca_pem,
        "200 OK",
        RELEASE.as_bytes().to_vec(),
        &["--json"],
    );
    assert_eq!(run.status, Some(3), "{}", run.stderr);
    assert!(
        run.stderr.starts_with(
            "[resolve_guidellm] UNRESOLVED 최신 릴리즈를 조회하지 못했다(URLError: <urlopen error "
        ),
        "{}",
        run.stderr
    );
    assert!(
        run.stdout.is_empty(),
        "nothing is resolved from an unverified peer"
    );
    assert!(
        run.seen.is_err(),
        "the server must not have received the request"
    );
}

const TRUNCATED_BODY: &[u8] = b"{\"tag_name\":\"v0.9.1\"}";

/// The original's actual failure for a body shorter than its `Content-Length` (21 bytes of 38, observed on the original
/// through a loopback fixture): `http.client.IncompleteRead`, uncaught, exit 1, and the record file untouched.
#[test]
fn a_body_shorter_than_its_declared_length_is_the_uncaught_incomplete_read_and_writes_nothing() {
    let folder = Scratch::new("truncated");
    let server = authority(&folder.0.join("ca"), "truncated");
    let record = folder.0.join("pin.json");
    let before = b"{\"schema_version\": 2, \"tools\": {}}".to_vec();
    std::fs::write(&record, &before).expect("record");
    let record_text = record.to_string_lossy().into_owned();
    let run = run_reply(
        &server,
        &server.ca_pem,
        Reply::Short {
            body: TRUNCATED_BODY.to_vec(),
            extra: 17,
        },
        &["--json", "--record", "--record-path", &record_text],
    );
    assert_eq!(run.status, Some(1), "{}", run.stderr);
    assert!(run.stdout.is_empty());
    assert!(
        run.stderr
            .starts_with("Traceback (most recent call last):\n"),
        "{}",
        run.stderr
    );
    assert_eq!(
        run.stderr.trim_end().lines().last(),
        Some("http.client.IncompleteRead: IncompleteRead(21 bytes read, 17 more expected)")
    );
    assert_eq!(
        std::fs::read(&record).expect("record"),
        before,
        "the record must not be written"
    );
    assert!(run.seen.is_ok(), "the request was served");
}

/// A chunked body that ends between chunks: no length was declared, so no count of missing bytes - and again neither a
/// JSON error nor an `OSError`. (`http.client` reports the bytes of the complete chunks, here the one chunk of 5
/// bytes; characterised from CPython's `_read_chunked`, not observed on the original.)
#[test]
fn a_chunked_body_that_ends_between_chunks_is_an_incomplete_read_without_an_expected_count() {
    let folder = Scratch::new("chunked");
    let server = authority(&folder.0.join("ca"), "chunked");
    let run = run_reply(
        &server,
        &server.ca_pem,
        Reply::ChunkedThenClose {
            chunk: b"{\"tag".to_vec(),
        },
        &["--json"],
    );
    assert_eq!(run.status, Some(1), "{}", run.stderr);
    assert!(run.stdout.is_empty());
    assert_eq!(
        run.stderr.trim_end().lines().last(),
        Some("http.client.IncompleteRead: IncompleteRead(5 bytes read)")
    );
}
