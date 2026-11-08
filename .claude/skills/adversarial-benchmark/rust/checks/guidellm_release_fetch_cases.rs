//! The frozen original `fetch_latest` criteria, run through the injected transport of the API program: 30 calls (18
//! values, 12 exceptions) whose answer is a recorded body (BOM, UTF-16, NaN, malformed, invalid UTF-8, `null`, an array)
//! or a recorded opener failure. The request the transport receives (URL, method, headers, timeout 20) is compared with
//! the recorded one, and the response resource must be closed whether the call returned or raised.
//!
//! This is the production `fetch_latest` over a stand-in transport. It is not a proof of the live HTTPS client: see
//! `guidellm_release_tls_cases` for that.

#[path = "guidellm_release_common.rs"]
mod common;

use common::*;
use std::cell::Cell;
use std::io::{Cursor, Read};
use std::rc::Rc;
use sweep_stop::dataset_prep::Recorder;
use sweep_stop::guidellm::Failure;
use sweep_stop::guidellm_release::{fetch_latest, ReleaseRequest, ReleaseTransport};
use sweep_stop::guidellm_release_api::{evaluate_operation, parse_fixture};
use sweep_stop::pyvalue::PyValue;

#[test]
fn every_recorded_fetch_matches_the_original_value_exception_and_request() {
    let rows = list(read_json("fetch_cases.json"));
    assert_eq!(rows.len(), 30);
    let (mut values, mut exceptions) = (0, 0);
    let mut failures = Vec::new();
    for row in &rows {
        let row = dict(row);
        let id = text(field(row, "id"));
        let transport = parse_fixture(field(row, "fetch_fixture")).expect("fixture");
        let arguments = dict(field(row, "arguments")).clone();
        let want = dict(field(row, "expected"));
        let mut streams = Recorder::default();
        let got = evaluate_operation("fetch_latest", &arguments, &mut streams, &transport);
        let agrees = match (got, text(field(want, "kind")).as_str()) {
            (Ok(value), "value") => {
                values += 1;
                key(&value) == key(field(want, "value"))
            }
            (Err(failure), "exception") => {
                exceptions += 1;
                failure.class == text(field(want, "type"))
                    && failure.message.to_utf8() == Some(text(field(want, "message")))
            }
            _ => false,
        };
        let asked = transport.requests();
        let request_ok =
            asked.len() == 1 && key(&asked[0].to_value()) == key(field(row, "expected_request"));
        if !(agrees && request_ok && streams.stderr.is_empty() && streams.stdout.is_empty()) {
            failures.push(id);
        }
    }
    assert!(failures.is_empty(), "differing calls {failures:?}");
    assert_eq!((values, exceptions), (18, 12));
}

/// A response whose drop is observable.
struct Probe {
    body: Cursor<Vec<u8>>,
    failing_read: bool,
    dropped: Rc<Cell<usize>>,
}

impl Read for Probe {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.failing_read {
            return Err(std::io::Error::from(std::io::ErrorKind::TimedOut));
        }
        self.body.read(buffer)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        self.dropped.set(self.dropped.get() + 1);
    }
}

struct Opens {
    body: Vec<u8>,
    failing_read: bool,
    dropped: Rc<Cell<usize>>,
}

impl ReleaseTransport for Opens {
    fn open(&self, _request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure> {
        Ok(Box::new(Probe {
            body: Cursor::new(self.body.clone()),
            failing_read: self.failing_read,
            dropped: Rc::clone(&self.dropped),
        }))
    }
}

#[test]
fn the_response_is_closed_after_a_value_a_decode_failure_and_a_read_failure() {
    let repo = PyValue::str("vllm-project/guidellm");
    for (label, body, failing_read, class) in [
        ("value", b"{\"tag_name\": \"v1.2.3\"}".to_vec(), false, None),
        (
            "json failure",
            b"{\"tag_name\":".to_vec(),
            false,
            Some("JSONDecodeError"),
        ),
        (
            "decode failure",
            vec![0x7B, 0xFF],
            false,
            Some("UnicodeDecodeError"),
        ),
        ("read failure", Vec::new(), true, Some("TimeoutError")),
    ] {
        let dropped = Rc::new(Cell::new(0));
        let transport = Opens {
            body,
            failing_read,
            dropped: Rc::clone(&dropped),
        };
        let outcome = fetch_latest(&repo, &transport);
        assert_eq!(outcome.err().map(|failure| failure.class), class, "{label}");
        assert_eq!(
            dropped.get(),
            1,
            "{label}: the response must be closed exactly once"
        );
    }
}

#[test]
fn a_request_the_original_would_refuse_never_reaches_the_transport() {
    let dropped = Rc::new(Cell::new(0));
    let transport = Opens {
        body: Vec::new(),
        failing_read: false,
        dropped: Rc::clone(&dropped),
    };
    for (repo, class) in [("a b", "InvalidURL"), ("a/\u{ed}", "UnicodeEncodeError")] {
        let failure = fetch_latest(&PyValue::str(repo), &transport).unwrap_err();
        assert_eq!(failure.class, class);
    }
    assert_eq!(dropped.get(), 0, "no response was ever opened");
}
