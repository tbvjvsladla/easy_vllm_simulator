//! `urllib.request.urlretrieve(url, filename)` as `wheel_metadata` calls it (CPython 3.12.3 `urllib/request.py`): the URL is opened WITHOUT a timeout
//! (callers pass [`crate::http_direct::UreqDialer::unbounded`]; `fetch_json`'s 20 seconds do not apply here), the headers are read, the destination
//! file is created only after that, the body is copied in blocks of 8192 bytes, and a body shorter than a declared `Content-Length` is reported after
//! the copy as `ContentTooShortError` whose `content` is `(destination path, headers)`.
//!
//! Leftover state is part of the behaviour: the destination exists (possibly partial) when a read fails or the body is short. The request is the
//! default `urlopen(url)` request (`Python-urllib/3.12`, no `Accept`); redirects, `CONNECT` tunnelling and TLS are the shared engines'. `file:` URLs go
//! through the shared file handler, whose bytes are written to the destination whole. Out of scope as in `fetch_json`: `ftp:`/`data:`, a `file:` URL
//! with a host, other proxy forms.

use crate::fetch_https::{exchange_with, FetchEnvironment};
use crate::http_direct::failure::{ErrorValue, ScopeRefusal};
use crate::http_direct::opener::{open_response, PlainHttp};
use crate::http_direct::request::{RequestHeaders, SurrogateCarrier};
use crate::http_direct::url::RequestUrl;
use crate::http_direct::{Dial, PyException};
use crate::py_errors::{builtin, os_error_with_filename, os_error_without_filename};
use crate::wheel_fetch::{open_url, FetchError, Origin};
use agent_runtime_support::pynumber::int_from_str;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use num_bigint::BigInt;
use std::io::Write;
use std::path::Path;

/// `bs = 1024*8`.
const BLOCK_BYTES: usize = 8192;

fn raised(error: PyException) -> FetchError {
    FetchError::Raised {
        error: Box::new(error),
        origin: Origin::UrlOpen,
    }
}

fn refusal(boundary: &'static str, detail: &str) -> FetchError {
    FetchError::OutOfScope(ScopeRefusal {
        boundary,
        detail: detail.to_string(),
    })
}

/// `_splittype(url)`: a regular expression matched against the URL, so only text is accepted.
fn url_text(url: &PyValue) -> Result<PyStr, FetchError> {
    match url.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        PyValue::Bytes(_) => Err(raised(builtin(
            "TypeError",
            "cannot use a string pattern on a bytes-like object",
        ))),
        other => Err(raised(builtin(
            "TypeError",
            &format!(
                "expected string or bytes-like object, got '{}'",
                other.type_name()
            ),
        ))),
    }
}

fn create_destination(destination: &Path) -> Result<std::fs::File, FetchError> {
    std::fs::File::create(destination).map_err(|error| {
        let shown = PyStr::new(&destination.to_string_lossy());
        match os_error_with_filename(&error, &shown) {
            Some(error) => raised(error),
            None => refusal("destination", &error.to_string()),
        }
    })
}

fn write_failed(error: &std::io::Error) -> FetchError {
    match os_error_without_filename(error) {
        Some(error) => raised(error),
        None => refusal("write", &error.to_string()),
    }
}

/// `ContentTooShortError(message, (filename, headers))`.
fn content_too_short(
    read: usize,
    size: &BigInt,
    destination: &Path,
    headers: &[(String, String)],
) -> PyException {
    let reason = format!("retrieval incomplete: got only {read} out of {size} bytes");
    PyException {
        class: "ContentTooShortError",
        module: "urllib.error",
        message: PyStr::new(&format!("<urlopen error {reason}>")),
        args: vec![ErrorValue::Plain(PyValue::str(&reason))],
        attributes: vec![
            ("errno", PyValue::None.into()),
            ("strerror", PyValue::None.into()),
            ("filename", PyValue::None.into()),
            ("filename2", PyValue::None.into()),
            ("reason", PyValue::str(&reason).into()),
            (
                "content",
                ErrorValue::Tuple(vec![
                    PyValue::str(&destination.to_string_lossy()).into(),
                    ErrorValue::HttpMessage(headers.to_vec()),
                ]),
            ),
        ],
        http_headers: None,
        link: None,
    }
}

/// `urlretrieve(url, destination)` for any typed `url`.
pub fn urlretrieve(
    url: &PyValue,
    destination: &Path,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
) -> Result<(), FetchError> {
    let text = url_text(url)?;
    let Some((carried, carrier)) = SurrogateCarrier::carry(text.code_points()) else {
        return Err(refusal(
            "url_text",
            "more lone surrogates than unused private-use characters",
        ));
    };
    let request = RequestUrl::parse(&carried)?;
    let headers = RequestHeaders::default();
    let response = match request.scheme.as_str() {
        "http" => {
            if environment.proxies.has_http_proxy() {
                return Err(refusal(
                    "proxy_environment",
                    "an http proxy variable is set; only direct connections are modelled",
                ));
            }
            open_response(
                &PlainHttp {
                    dial: dialer,
                    headers: &headers,
                    carrier: Some(&carrier),
                },
                request,
            )?
        }
        "https" => open_response(
            &exchange_with(environment, dialer, &carrier, &headers),
            request,
        )?,
        _ => {
            // `file:` and the schemes the shared opener refuses or rejects: the whole content is known at once
            let bytes = open_url(&PyValue::Str(text), &headers, environment, dialer)?;
            create_destination(destination)?
                .write_all(&bytes)
                .map_err(|error| write_failed(&error))?;
            return Ok(());
        }
    };
    let mut response = response;
    let received_headers: Vec<(String, String)> = response.headers.raw_items().to_vec();
    let mut file = create_destination(destination)?;
    let declared = if response.headers.contains("content-length") {
        let value = response
            .headers
            .get("content-length")
            .unwrap_or_default()
            .to_string();
        Some(
            int_from_str(&PyStr::new(&value))
                .map_err(|failure| raised(builtin("ValueError", &failure.message)))?,
        )
    } else {
        None
    };
    let mut read = 0usize;
    loop {
        let block = response.read_amount(BLOCK_BYTES)?;
        if block.is_empty() {
            break;
        }
        read += block.len();
        file.write_all(&block)
            .map_err(|error| write_failed(&error))?;
    }
    drop(file);
    match declared {
        Some(size) if size >= BigInt::from(0) && BigInt::from(read) < size => Err(raised(
            content_too_short(read, &size, destination, &received_headers),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::UreqDialer;
    use std::io::Read;
    use std::net::TcpListener;
    use std::sync::{Arc, Mutex};

    /// An owned loopback server that answers each connection with the next canned response and records the request head.
    struct Tape {
        address: String,
        heads: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    fn serve(responses: Vec<Vec<u8>>) -> Tape {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        let heads = Arc::new(Mutex::new(Vec::new()));
        let seen = heads.clone();
        std::thread::spawn(move || {
            for response in responses {
                let Ok((mut stream, _)) = listener.accept() else {
                    return;
                };
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n")
                    && stream.read(&mut byte).map(|n| n == 1).unwrap_or(false)
                {
                    head.push(byte[0]);
                }
                seen.lock().unwrap().push(head);
                let _ = std::io::Write::write_all(&mut stream, &response);
            }
        });
        Tape { address, heads }
    }

    fn scratch() -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "urlretrieve-checks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn retrieve(url: &str, destination: &Path) -> Result<(), FetchError> {
        urlretrieve(
            &PyValue::str(url),
            destination,
            &FetchEnvironment::default(),
            &UreqDialer::unbounded(),
        )
    }

    #[test]
    fn a_complete_body_is_copied_and_the_request_is_the_default_urlopen_head() {
        let body: Vec<u8> = (0..20_000u32).map(|n| (n % 251) as u8).collect();
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(&body);
        let tape = serve(vec![response]);
        let directory = scratch();
        retrieve(
            &format!("http://{}/pkg.whl", tape.address),
            &directory.join("pkg.whl"),
        )
        .unwrap();
        assert_eq!(std::fs::read(directory.join("pkg.whl")).unwrap(), body);
        let head = String::from_utf8(tape.heads.lock().unwrap()[0].clone()).unwrap();
        assert_eq!(
            head,
            format!("GET /pkg.whl HTTP/1.1\r\nAccept-Encoding: identity\r\nHost: {}\r\nUser-Agent: Python-urllib/3.12\r\nConnection: close\r\n\r\n", tape.address)
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_short_body_leaves_the_partial_file_and_reports_the_typed_content() {
        let tape = serve(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 20\r\nConnection: close\r\n\r\n{\"x\":1}".to_vec(),
        ]);
        let directory = scratch();
        let destination = directory.join("pkg.whl");
        let FetchError::Raised { error, .. } =
            retrieve(&format!("http://{}/", tape.address), &destination).unwrap_err()
        else {
            panic!("an exception")
        };
        assert_eq!(error.class, "ContentTooShortError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "<urlopen error retrieval incomplete: got only 7 out of 20 bytes>"
        );
        assert_eq!(std::fs::read(&destination).unwrap(), b"{\"x\":1}");
        let (_, content) = error
            .attributes
            .iter()
            .find(|(name, _)| *name == "content")
            .unwrap();
        let ErrorValue::Tuple(members) = content else {
            panic!("tuple")
        };
        assert!(
            matches!(&members[0], ErrorValue::Plain(PyValue::Str(path)) if path.to_utf8().unwrap() == destination.to_string_lossy())
        );
        assert!(
            matches!(&members[1], ErrorValue::HttpMessage(items) if items == &[("Content-Length".to_string(), "20".to_string()), ("Connection".to_string(), "close".to_string())])
        );
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn typed_urls_and_other_schemes_fail_before_any_destination_exists() {
        let directory = scratch();
        let destination = directory.join("pkg.whl");
        for (url, expected) in [
            (
                PyValue::None,
                "expected string or bytes-like object, got 'NoneType'",
            ),
            (
                PyValue::int(1),
                "expected string or bytes-like object, got 'int'",
            ),
            (
                PyValue::Bytes(b"http://x".to_vec()),
                "cannot use a string pattern on a bytes-like object",
            ),
            (PyValue::str(""), "unknown url type: ''"),
        ] {
            let outcome = urlretrieve(
                &url,
                &destination,
                &FetchEnvironment::default(),
                &UreqDialer::unbounded(),
            );
            let FetchError::Raised { error, .. } = outcome.unwrap_err() else {
                panic!("an exception")
            };
            assert_eq!(error.message.to_utf8().unwrap(), expected);
        }
        assert!(!destination.exists());
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_file_url_is_copied_whole() {
        let directory = scratch();
        let source = directory.join("in.whl");
        std::fs::write(&source, b"zipbytes").unwrap();
        retrieve(
            &format!("file://{}", source.display()),
            &directory.join("out.whl"),
        )
        .unwrap();
        assert_eq!(
            std::fs::read(directory.join("out.whl")).unwrap(),
            b"zipbytes"
        );
        std::fs::remove_dir_all(directory).unwrap();
    }
}
