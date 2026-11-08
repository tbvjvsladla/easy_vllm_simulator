//! GuideLLM release resolution: a Rust port of `adversarial-benchmark/scripts/resolve_guidellm.py`, the *staging*
//! plane of the measurement tool (it may use the network; the execution plane, `bench_tool_pin`, never does).
//!
//! * [`version_from_tag`], [`image_ref`] and [`resolution`] are pure and take the values the Python functions take,
//!   including the ones they were never meant for (`image_ref(None)` is `ghcr.io/...:vNone`).
//! * [`fetch_latest`] asks the GitHub Releases API for the latest release through a [`ReleaseTransport`]; the
//!   production transport is [`crate::guidellm_release_http`], tests and the API program inject their own. The body
//!   is raw bytes decoded by `json.loads(bytes)` rules ([`crate::json_bytes`]). Nothing here caches a response or falls
//!   back to a remembered version: the latest release is whatever the upstream answered.
//! * [`merge_record`] merges a resolution into the pin record file: it reads the record, adds or replaces one version
//!   row, makes it the default, truncates the file and writes the document again, chunk by chunk. A failure in the
//!   middle leaves exactly what had been written (see [`crate::json_stream`]).
//!
//! Wherever the original raises, [`Failure`] carries the same exception class and text.

use crate::guidellm::Failure;
use crate::json_bytes::loads_bytes;
use crate::json_stream::{dump_indented, open_for_writing, ChunkSink, FileSink};
use crate::pydyn::{as_get_target, subscript, Text};
use crate::pyfs::{os_error_class, read_bytes};
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::{decimal_digit_value, PyStr};
use crate::pyvalue::{PyDict, PyValue};
use std::io::Read;
use std::time::Duration;

/// The default GitHub repository whose releases are the GuideLLM versions.
pub const REPO_DEFAULT: &str = "vllm-project/guidellm";
/// The registry host of the GuideLLM container images.
pub const IMAGE_HOST: &str = "ghcr.io";
/// Origin of the releases API; the request path is `/repos/<repo>/releases/latest`.
pub const API_ORIGIN: &str = "https://api.github.com";
/// `Accept` header of the release request.
pub const REQUEST_ACCEPT: &str = "application/vnd.github+json";
/// `User-Agent` header of the release request.
pub const REQUEST_USER_AGENT: &str = "adversarial-benchmark";
/// The timeout `urlopen` is given: a limit on each socket operation, not on the whole request.
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Exit status: resolved.
pub const RC_OK: i32 = 0;
/// Exit status: argument or response error.
pub const RC_ARGS: i32 = 2;
/// Exit status: the latest release could not be resolved over the network (declare the version).
pub const RC_UNRESOLVED: i32 = 3;
/// Environment variable naming the default pin record. The launcher sets it to the record that ships beside the
/// scripts; a direct run of the binary falls back to the record of the crate's parent folder in the node's tree.
pub const RECORD_PATH_ENV: &str = "GUIDELLM_RECORD_PATH";
/// `digest_source` written next to a recorded digest.
pub const DIGEST_SOURCE: &str = "measured(docker image inspect .RepoDigests)";
/// Where `json.dump` of the merged record starts, as interpreter frames: module, `main`, `merge_record`, `dump`,
/// `_iterencode`, `_iterencode_dict`.
const MERGE_TOP_FRAME: usize = 6;

/// Where the pin record is when the caller names none (`RECORD_PATH` of the original).
pub fn default_record_path() -> PyStr {
    match std::env::var_os(RECORD_PATH_ENV) {
        Some(path) if !path.is_empty() => {
            PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str()))
        }
        _ => PyStr::new(&format!(
            "{}/../bench_tool_pin.json",
            env!("CARGO_MANIFEST_DIR")
        )),
    }
}

/// `except (URLError, OSError, TimeoutError)`: the classes this port can raise that are `OSError`s.
pub fn is_os_error(class: &str) -> bool {
    os_error_class_named(class).is_some()
}

/// The `OSError` class a name stands for, if it is one this port can raise.
pub fn os_error_class_named(name: &str) -> Option<&'static str> {
    const CLASSES: &[&str] = &[
        "OSError",
        "URLError",
        "HTTPError",
        "TimeoutError",
        "ConnectionError",
        "ConnectionRefusedError",
        "ConnectionResetError",
        "ConnectionAbortedError",
        "BrokenPipeError",
        "FileNotFoundError",
        "FileExistsError",
        "PermissionError",
        "IsADirectoryError",
        "NotADirectoryError",
        "InterruptedError",
        "BlockingIOError",
        "ChildProcessError",
        "ProcessLookupError",
    ];
    CLASSES.iter().copied().find(|class| *class == name)
}

/// `except ValueError`: `ValueError` itself and its subclasses this port can raise.
pub fn is_value_error(class: &str) -> bool {
    matches!(
        class,
        "ValueError"
            | "JSONDecodeError"
            | "UnicodeDecodeError"
            | "UnicodeEncodeError"
            | "UnicodeError"
    )
}

// ── tag → version, image reference, resolution ──────────────────────────────────

fn missing_tag() -> Failure {
    Failure::new("ValueError", "릴리즈 응답에 tag_name 이 없다")
}

/// Whether `text` matches `^\d+(?:\.\d+){1,3}(?:[-.a-zA-Z0-9]*)$` of the original (`\d` is any Unicode decimal digit; the
/// tail is ASCII). `text` holds no trailing newline here (it was stripped), so `$` is the end of the text.
fn is_release_version(text: &[u32]) -> bool {
    let is_digit = |point: u32| decimal_digit_value(point).is_some();
    let is_tail = |point: u32| {
        point == u32::from('-')
            || point == u32::from('.')
            || char::from_u32(point).is_some_and(|c| c.is_ascii_alphanumeric())
    };
    let length = text.len();
    let digits_from = |from: usize| text[from..].iter().take_while(|&&p| is_digit(p)).count();
    // The first `\d+` must take the whole digit run: a shorter run is followed by a digit, not by the `.` that must come
    // next. Each `.\d+` group may stop inside its digit run (the rest is then the tail), so the positions reachable
    // after one, two and three groups are tracked as sets.
    let first = digits_from(0);
    if first == 0 {
        return false;
    }
    let mut tail_from = vec![false; length + 1];
    tail_from[length] = true;
    for at in (0..length).rev() {
        tail_from[at] = tail_from[at + 1] && is_tail(text[at]);
    }
    let mut reachable = vec![false; length + 1];
    reachable[first] = true;
    for _group in 1..=3 {
        let mut after = vec![false; length + 1];
        for at in (0..=length).filter(|&at| reachable[at]) {
            if text.get(at) == Some(&u32::from('.')) {
                let run = digits_from(at + 1);
                for taken in 1..=run {
                    after[at + 1 + taken] = true;
                }
            }
        }
        if (0..=length).any(|at| after[at] && tail_from[at]) {
            return true;
        }
        reachable = after;
    }
    false
}

/// `version_from_tag(tag)`: a release tag as a version string. It never guesses - anything that is not a
/// non-empty string, or whose form is not `v?<digits>.<digits>...`, is a `ValueError`.
pub fn version_from_tag(tag: &PyValue) -> Result<PyStr, Failure> {
    let PyValue::Str(text) = tag.peel() else {
        return Err(missing_tag());
    };
    let stripped = text.strip();
    if stripped.is_empty() {
        return Err(missing_tag());
    }
    let points = stripped.code_points();
    let version = match points.split_first() {
        Some((first, rest)) if *first == u32::from('v') => rest,
        _ => points,
    };
    if !is_release_version(version) {
        let message = format!(
            "태그 {} 에서 버전을 읽지 못했다(명명 스킴 변경 가능성 — 사람이 확인)",
            text.repr()
        );
        return Err(Failure::new("ValueError", &message));
    }
    Ok(PyStr::from_code_points(version.to_vec()))
}

/// `image_ref(version, repo)`: `ghcr.io/<repo>:v<version>`, each part rendered with `%s`.
pub fn image_ref(version: &PyValue, repo: &PyValue) -> PyStr {
    Text::new()
        .lit(IMAGE_HOST)
        .lit("/")
        .val(repo)
        .lit(":v")
        .val(version)
        .done()
}

/// `resolution(version, source, repo, published_utc)`: the record of one resolution. `published_utc` is what the
/// upstream said; no clock is read.
pub fn resolution(
    version: &PyValue,
    source: &PyValue,
    repo: &PyValue,
    published_utc: &PyValue,
) -> PyDict {
    let reference = image_ref(version, repo);
    let mut map = PyDict::new();
    map.set("tool", PyValue::str("guidellm"));
    map.set("repo", repo.clone());
    map.set("version", version.clone());
    map.set("version_source", source.clone());
    map.set("published_utc", published_utc.clone());
    map.set("image_ref", PyValue::Str(reference.clone()));
    map.set(
        "pull_command",
        PyValue::Str(Text::new().lit("docker pull ").str(&reference).done()),
    );
    map
}

// ── the release request ─────────────────────────────────────────────────────────

/// One request for the latest release, as `urlopen(request, timeout=20)` receives it.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseRequest {
    /// `request.full_url`, fragment included.
    pub full_url: PyStr,
    /// The URL that goes on the wire: the same without the fragment.
    pub wire_url: String,
    pub method: &'static str,
    /// Header names as `urllib` capitalises them (`Accept`, `User-agent`) with their values.
    pub headers: Vec<(&'static str, &'static str)>,
    pub timeout: Duration,
}

impl ReleaseRequest {
    /// `{url, method, headers: {lowercase name: value}, timeout}` - what a transport was asked for.
    pub fn to_value(&self) -> PyValue {
        let mut headers = PyDict::new();
        for (name, value) in &self.headers {
            headers.set(&name.to_ascii_lowercase(), PyValue::str(value));
        }
        let mut map = PyDict::new();
        map.set("url", PyValue::Str(self.full_url.clone()));
        map.set("method", PyValue::str(self.method));
        map.set("headers", PyValue::Dict(headers));
        map.set("timeout", PyValue::int(self.timeout.as_secs()));
        PyValue::Dict(map)
    }
}

/// `\x%02x`, `\u%04x` or `\U%08x`, as `UnicodeEncodeError` names a character.
fn escaped_character(point: u32) -> String {
    match point {
        0..=0xFF => format!("\\x{point:02x}"),
        0x100..=0xFFFF => format!("\\u{point:04x}"),
        _ => format!("\\U{point:08x}"),
    }
}

/// What `http.client` does with the request target before it connects: a control character or space is an
/// `InvalidURL`, and the request line must be ASCII (a `UnicodeEncodeError`, a `ValueError`).
fn check_selector(selector: &[u32]) -> Result<(), Failure> {
    if let Some(&found) = selector.iter().find(|&&p| p <= 0x20 || p == 0x7F) {
        let message = format!(
            "URL can't contain control characters. {} (found at least {})",
            PyStr::from_code_points(selector.to_vec()).repr(),
            PyStr::from_code_points(vec![found]).repr()
        );
        return Err(Failure::new("InvalidURL", &message));
    }
    if let Some(first) = selector.iter().position(|&p| p > 0x7F) {
        const REQUEST_LINE_PREFIX: usize = "GET ".len();
        let run = selector[first..].iter().take_while(|&&p| p > 0x7F).count();
        let start = REQUEST_LINE_PREFIX + first;
        let subject = if run == 1 {
            format!(
                "character '{}' in position {start}",
                escaped_character(selector[first])
            )
        } else {
            format!("characters in position {start}-{}", start + run - 1)
        };
        let message = format!("'ascii' codec can't encode {subject}: ordinal not in range(128)");
        return Err(Failure::new("UnicodeEncodeError", &message));
    }
    Ok(())
}

/// The request for the latest release of `repo` (`LATEST_API % repo`, `urllib.request.Request`). Fails the way
/// `urllib`/`http.client` do before any connection is made.
pub fn latest_request(repo: &PyValue) -> Result<ReleaseRequest, Failure> {
    let full_url = Text::new()
        .lit(API_ORIGIN)
        .lit("/repos/")
        .val(repo)
        .lit("/releases/latest")
        .done();
    let points = full_url.code_points();
    // `Request.full_url` splits the fragment off at the last `#`; it is not sent.
    let end = points
        .iter()
        .rposition(|&p| p == u32::from('#'))
        .unwrap_or(points.len());
    let selector = &points[API_ORIGIN.len()..end];
    check_selector(selector)?;
    let wire_url = points[..end]
        .iter()
        .map(|&p| char::from_u32(p).expect("checked ASCII"))
        .collect();
    Ok(ReleaseRequest {
        full_url,
        wire_url,
        method: "GET",
        headers: vec![
            ("Accept", REQUEST_ACCEPT),
            ("User-agent", REQUEST_USER_AGENT),
        ],
        timeout: REQUEST_TIMEOUT,
    })
}

/// Opens the response of a release request. A failure is the exception `urlopen` raises (`URLError`, `HTTPError`,
/// `TimeoutError`, `OSError`). The returned reader owns the response: dropping it closes it, on every path.
pub trait ReleaseTransport {
    fn open(&self, request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure>;
}

/// `http.client.IncompleteRead`: the response ended before the length it declared. Carried inside an
/// `std::io::Error` by a transport (its `Display` is Python's `str()` of the exception) and turned back into the
/// exception by [`read_failure`]. It is not an `OSError` and not a `ValueError`, so the command line does not handle it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IncompleteRead {
    /// Bytes received before the end.
    pub partial: u64,
    /// Bytes still expected, when the length was declared (`None` for a chunked body).
    pub expected: Option<u64>,
}

impl std::fmt::Display for IncompleteRead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IncompleteRead({} bytes read", self.partial)?;
        if let Some(expected) = self.expected {
            write!(f, ", {expected} more expected")?;
        }
        f.write_str(")")
    }
}

impl std::error::Error for IncompleteRead {}

/// The exception a failed read of the response raises.
pub fn read_failure(error: &std::io::Error) -> Failure {
    if let Some(incomplete) = error
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<IncompleteRead>())
    {
        return Failure::new("IncompleteRead", &incomplete.to_string());
    }
    if matches!(
        error.kind(),
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
    ) {
        return Failure::new("TimeoutError", "The read operation timed out");
    }
    let detail = error.to_string();
    let message = detail
        .split(" (os error")
        .next()
        .unwrap_or(&detail)
        .to_string();
    match error.raw_os_error() {
        Some(errno) => Failure::new(os_error_class(error), &format!("[Errno {errno}] {message}")),
        None => Failure::new("OSError", &message),
    }
}

/// `fetch_latest(repo, opener)`: the JSON document the releases API answers with, whatever it is. The response is closed
/// when the call ends, whether it returned or raised.
pub fn fetch_latest(repo: &PyValue, transport: &dyn ReleaseTransport) -> Result<PyValue, Failure> {
    let request = latest_request(repo)?;
    fetch_request(&request, transport)
}

/// [`fetch_latest`] for a request that was already built.
pub fn fetch_request(
    request: &ReleaseRequest,
    transport: &dyn ReleaseTransport,
) -> Result<PyValue, Failure> {
    let mut response = transport.open(request)?;
    let outcome = read_document(response.as_mut());
    drop(response);
    outcome
}

fn read_document(response: &mut dyn Read) -> Result<PyValue, Failure> {
    let mut bytes = Vec::new();
    response
        .read_to_end(&mut bytes)
        .map_err(|error| read_failure(&error))?;
    loads_bytes(&bytes).map_err(|error| crate::repeat_api_cli::load_failure(&error))
}

// ── merging into the record ─────────────────────────────────────────────────────

fn text_path(path: &PyValue) -> Result<PyStr, Failure> {
    match path.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "expected str, bytes or os.PathLike object, not {}",
                other.type_name()
            ),
        )),
    }
}

/// `json.load(open(path, encoding="utf-8"))`.
pub fn load_record(path: &PyStr) -> Result<PyValue, Failure> {
    let bytes = read_bytes(path)?;
    let text =
        decode_file_text(&bytes).map_err(|error| crate::repeat_api_cli::load_failure(&error))?;
    loads_text(&text).map_err(|error| crate::repeat_api_cli::load_failure(&error))
}

fn no_attribute(value: &PyValue, name: &str) -> Failure {
    Failure::new(
        "AttributeError",
        &format!("'{}' object has no attribute '{name}'", value.type_name()),
    )
}

/// `container.setdefault(key, {})` for a string key: the stored value, an empty dict when it was absent.
fn setdefault_empty<'a>(container: &'a mut PyValue, key: &str) -> Result<&'a mut PyValue, Failure> {
    match container {
        PyValue::Dict(map) => {
            if !map.contains_key(key) {
                map.set(key, PyValue::Dict(PyDict::new()));
            }
            Ok(map.get_mut(key).expect("present or just inserted"))
        }
        other => Err(no_attribute(other, "setdefault")),
    }
}

/// `target[key] = value` for a string key.
fn assign(target: &mut PyValue, key: &str, value: PyValue) -> Result<(), Failure> {
    match target {
        PyValue::Dict(map) => {
            map.set(key, value);
            Ok(())
        }
        PyValue::List(_) => Err(Failure::new(
            "TypeError",
            "list indices must be integers or slices, not str",
        )),
        other => Err(Failure::new(
            "TypeError",
            &format!(
                "'{}' object does not support item assignment",
                other.type_name()
            ),
        )),
    }
}

/// `merge_record(path, res, digest)`: merges one resolution into the record file and returns `path`. Existing versions,
/// unknown fields, digests and platforms are kept; the new version becomes `default_version`.
///
/// The order of the original is kept, because each step can fail on a record that is not the expected shape: read and
/// decode the file; `tools`, `guidellm` and `versions` are taken (or created) in turn; `res["version"]` names the row;
/// the row is filled; the file is truncated and the document written in chunks, so a failing write leaves the part
/// already written. The digest is stored only when it is truthy, `published_utc` only when it is.
pub fn merge_record(path: &PyValue, res: &PyValue, digest: &PyValue) -> Result<PyValue, Failure> {
    let record_path = text_path(path)?;
    let mut document = load_record(&record_path)?;

    let tools = setdefault_empty(&mut document, "tools")?;
    let entry = setdefault_empty(tools, "guidellm")?;
    let versions = setdefault_empty(entry, "versions")?;
    let PyValue::Dict(versions) = versions else {
        return Err(no_attribute(versions, "setdefault"));
    };
    let version = subscript(res, "version")?;
    let row = versions.setdefault_key(
        crate::bench_tool_pin::dict_key(version)?,
        PyValue::Dict(PyDict::new()),
    );

    assign(row, "image_ref", subscript(res, "image_ref")?.clone())?;
    assign(
        row,
        "version_source",
        subscript(res, "version_source")?.clone(),
    )?;
    let published = as_get_target(res)?.get("published_utc");
    if let Some(published) = published.filter(|published| published.is_truthy()) {
        assign(row, "published_utc", published.clone())?;
    }
    if digest.is_truthy() {
        assign(row, "index_digest", digest.clone())?;
        assign(row, "digest_source", PyValue::str(DIGEST_SOURCE))?;
    }
    let entry = setdefault_empty(setdefault_empty(&mut document, "tools")?, "guidellm")?;
    assign(entry, "default_version", version.clone())?;

    // `with open(path, "w", encoding="utf-8")`: the file is truncated first, the document is dumped chunk by chunk, a
    // newline follows, and closing flushes what was accepted whether or not the dump failed.
    let mut sink = FileSink::new(open_for_writing(&record_path)?);
    let dumped = dump_indented(&document, 2, MERGE_TOP_FRAME, &mut sink)
        .and_then(|()| sink.write(&PyStr::new("\n")));
    let closed = sink.close();
    closed?;
    dumped?;
    Ok(path.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tag(text: &str) -> PyValue {
        PyValue::str(text)
    }

    #[test]
    fn versions_follow_the_original_pattern() {
        for (given, version) in [
            ("v0.7.3", "0.7.3"),
            ("0.8.0", "0.8.0"),
            ("  v0.7.3  ", "0.7.3"),
            ("v1.2", "1.2"),
            ("1.2.3.4", "1.2.3.4"),
            ("1.2.3.4.5", "1.2.3.4.5"),
            ("v1.2rc1", "1.2rc1"),
            ("v1.2-alpha", "1.2-alpha"),
            ("v１.２", "１.２"),
            ("v١.٢", "١.٢"),
            ("v1.2\n", "1.2"),
        ] {
            let got = version_from_tag(&tag(given)).unwrap_or_else(|e| panic!("{given:?}: {e:?}"));
            assert!(got.equals(version), "{given:?}");
        }
        for bad in [
            "v",
            "latest",
            "release-candidate",
            "v1.2+g123",
            "v1",
            "1.",
            "1..2",
            "v1.2é",
        ] {
            assert_eq!(
                version_from_tag(&tag(bad)).unwrap_err().class,
                "ValueError",
                "{bad:?}"
            );
        }
        for bad in [
            PyValue::None,
            PyValue::Bool(true),
            PyValue::int(1),
            tag(""),
            tag("  "),
        ] {
            assert_eq!(
                version_from_tag(&bad)
                    .unwrap_err()
                    .message
                    .to_utf8()
                    .as_deref(),
                Some("릴리즈 응답에 tag_name 이 없다")
            );
        }
    }

    #[test]
    fn a_long_digit_run_is_judged_in_linear_time() {
        let long = format!("v{}.{}é", "1".repeat(200_000), "2".repeat(200_000));
        assert!(version_from_tag(&tag(&long)).is_err());
    }

    #[test]
    fn the_request_target_is_checked_like_http_client_does() {
        let request = latest_request(&PyValue::str("a/b")).expect("plain repo");
        assert_eq!(
            request.wire_url,
            "https://api.github.com/repos/a/b/releases/latest"
        );
        assert_eq!(
            latest_request(&PyValue::str("a b")).unwrap_err().class,
            "InvalidURL"
        );
        let non_ascii = latest_request(&PyValue::str("a/é")).unwrap_err();
        assert_eq!(non_ascii.class, "UnicodeEncodeError");
        assert_eq!(
            non_ascii.message.to_utf8().unwrap(),
            "'ascii' codec can't encode character '\\xe9' in position 13: ordinal not in range(128)"
        );
        // the fragment is split off at the last `#` and never sent
        let fragment = latest_request(&PyValue::str("a#b")).expect("fragment");
        assert_eq!(fragment.wire_url, "https://api.github.com/repos/a");
    }
}
