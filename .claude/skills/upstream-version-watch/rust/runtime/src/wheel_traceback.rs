//! The report of an exception `resolve_wheel.py` does not handle: CPython 3.12.3's `Traceback (most recent call last):` blocks, with the caret
//! layout of the interpreter, for the sites a run can raise through.
//!
//! **Provenance of the sites.** The script's own sites and the `urllib`, `http.client`, `ssl` and `json` frames of an HTTP error status, a refused
//! certificate (with its chained first traceback), a non-ASCII request line and a malformed JSON document were read from the saved raw
//! tracebacks of the original capture (`cli-015/023/024/025/026/027/029/031/032/037/038/039/040`). The `json` frames of the other scanner
//! failures (extra data, a byte decode, an error the C scanner raises itself) and the generator frame of the nearest-candidates list are DERIVED
//! from the same source files and CPython's rule for the instruction that raises (a call covers the whole call, an attribute load covers
//! `object.attribute`, a subscript is `~` under the value and `^` under the brackets, a multi-line call covers the rest of its first line, a
//! statement that is the whole line shows no carets); they were not captured. The span of each site is FOUND in its source line by the needle
//! given here, never typed as numbers, so a span cannot drift from the text. An exception raised somewhere this module has no site for (for
//! example a failed tunnel) is reported by its `Class: message` block alone, and the `AttributeError` suggestion line is not reproduced.

use crate::http_direct::PyException;
use crate::wheel_fetch::Origin;
use crate::wheel_json::JsonSite;
use agent_runtime_support::pystr::PyStr;

pub const STDLIB_PREFIX: &str = "/usr/lib/python3.12";

/// The file a frame is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameFile {
    /// `resolve_wheel.py`; its path is the caller's `argv[0]`.
    Script,
    /// A file of the reference interpreter's standard library, relative to [`STDLIB_PREFIX`].
    Stdlib(&'static str),
}

/// What the interpreter underlines in the source line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Underline {
    /// The instruction covers the whole statement or the line: no caret line.
    None,
    /// `^` under the first occurrence of this text in the line.
    Span(&'static str),
    /// `^` under the first occurrence of this text even when it is the whole statement (the interpreter shows such carets for some frames,
    /// for example the `urllib.request.Request` constructor's).
    Always(&'static str),
    /// A subscript: `~` under the first `value_width` characters of the text and `^` under the rest (the brackets).
    Subscript {
        text: &'static str,
        value_width: usize,
    },
}

/// One frame: where the interpreter says the exception passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub file: FrameFile,
    pub line: u32,
    pub function: &'static str,
    /// The source line with its indentation.
    pub text: &'static str,
    pub underline: Underline,
}

pub(crate) const fn script(
    line: u32,
    function: &'static str,
    text: &'static str,
    underline: Underline,
) -> Frame {
    Frame {
        file: FrameFile::Script,
        line,
        function,
        text,
        underline,
    }
}

pub(crate) const fn stdlib(
    file: &'static str,
    line: u32,
    function: &'static str,
    text: &'static str,
    underline: Underline,
) -> Frame {
    Frame {
        file: FrameFile::Stdlib(file),
        line,
        function,
        text,
        underline,
    }
}

pub(crate) const REQUEST: &str = "urllib/request.py";
const CLIENT: &str = "http/client.py";
const SSL: &str = "ssl.py";
const JSON_INIT: &str = "json/__init__.py";
const JSON_DECODER: &str = "json/decoder.py";
const ASSETS_LINE: &str = "    assets = [x[\"name\"] for x in rel.get(\"assets\", [])]";
const PRINT_RESULT: &str = "    print(json.dumps({";
const REQ_LINE: &str = "                req_major, req_minor = int(req[:-1]), int(req[-1])";
const SORT_LINE: &str =
    "        chosen = sorted(cands, key=lambda c: int(c[\"cuda\"]))[-1]  # 가장 높은 cuXXX";

/// `main()` at the bottom of the script.
pub const MODULE_ENTRY: Frame = script(98, "<module>", "    main()", Underline::None);
/// `rel = fetch_json(API.format(v=a.vllm_version))`.
pub const MAIN_FETCH: Frame = script(
    33,
    "main",
    "    rel = fetch_json(API.format(v=a.vllm_version))",
    Underline::Span("fetch_json(API.format(v=a.vllm_version))"),
);
/// `with urllib.request.urlopen(req, timeout=20) as r:` of `fetch_json`.
pub const FETCH_URLOPEN: Frame = script(
    21,
    "fetch_json",
    "    with urllib.request.urlopen(req, timeout=20) as r:",
    Underline::Span("urllib.request.urlopen(req, timeout=20)"),
);
/// `return json.load(r)` of `fetch_json`.
pub const FETCH_LOAD: Frame = script(
    22,
    "fetch_json",
    "        return json.load(r)",
    Underline::Span("json.load(r)"),
);
/// `rel.get`: the release document has no `get`.
pub const MAIN_GET: Frame = script(34, "main", ASSETS_LINE, Underline::Span("rel.get"));
/// The comprehension's iteration over a value that cannot be iterated (the inlined comprehension is the instruction).
pub const MAIN_ITERATE: Frame = script(
    34,
    "main",
    ASSETS_LINE,
    Underline::Span("[x[\"name\"] for x in rel.get(\"assets\", [])]"),
);
/// `x["name"]`.
pub const MAIN_NAME: Frame = script(
    34,
    "main",
    ASSETS_LINE,
    Underline::Subscript {
        text: "x[\"name\"]",
        value_width: 1,
    },
);
/// `m = pat_cu.match(name)`.
pub const MAIN_MATCH: Frame = script(
    45,
    "main",
    "        m = pat_cu.match(name)",
    Underline::Span("pat_cu.match(name)"),
);
/// `int(req[:-1])`.
pub const MAIN_REQUESTED_MAJOR: Frame =
    script(67, "main", REQ_LINE, Underline::Span("int(req[:-1])"));
/// `int(req[-1])`.
pub const MAIN_REQUESTED_MINOR: Frame =
    script(67, "main", REQ_LINE, Underline::Span("int(req[-1])"));
/// `nearest = sorted(`: the call spans five lines, so the span is the rest of the first.
pub const MAIN_NEAREST: Frame = script(
    68,
    "main",
    "                nearest = sorted(",
    Underline::Span("sorted("),
);
/// `int(c["cuda"][:-1])` inside the generator that feeds `sorted`.
pub const NEAREST_GENERATOR: Frame = script(
    71,
    "<genexpr>",
    "                     and int(c[\"cuda\"][:-1]) == req_major),",
    Underline::Span("int(c[\"cuda\"][:-1])"),
);
/// `sorted(cands, key=lambda c: int(c["cuda"]))`.
pub const MAIN_HIGHEST: Frame = script(
    83,
    "main",
    SORT_LINE,
    Underline::Span("sorted(cands, key=lambda c: int(c[\"cuda\"]))"),
);
/// `int(c["cuda"])` inside the key lambda.
pub const HIGHEST_KEY: Frame = script(
    83,
    "<lambda>",
    SORT_LINE,
    Underline::Span("int(c[\"cuda\"])"),
);
/// The `print(json.dumps({` of the result (line 89): the call spans several lines, so the span is the whole first line.
pub const MAIN_PRINT_RESULT: Frame = script(89, "main", PRINT_RESULT, Underline::None);
/// The `print(json.dumps(...), file=sys.stderr)` of the missing-assets report (line 54).
pub const MAIN_PRINT_NO_ASSETS: Frame = script(
    54,
    "main",
    "        print(json.dumps({\"error\": (\"arch 매칭 wheel 자산 없음(+cu/무접미어 모두) — \"",
    Underline::Span(
        "print(json.dumps({\"error\": (\"arch 매칭 wheel 자산 없음(+cu/무접미어 모두) — \"",
    ),
);
/// The `print(json.dumps(...), file=sys.stderr)` of the missing-exact-CUDA report (line 73).
pub const MAIN_PRINT_NO_CUDA: Frame = script(
    73,
    "main",
    "            print(json.dumps({\"error\": f\"cu{a.cuda} 정확일치 자산 없음\", \"available\": cands,",
    Underline::Span("print(json.dumps({\"error\": f\"cu{a.cuda} 정확일치 자산 없음\", \"available\": cands,"),
);

// ---- urllib / http.client / ssl (all read from the saved raw tracebacks) ----

pub(crate) const URLOPEN: Frame = stdlib(
    REQUEST,
    215,
    "urlopen",
    "    return opener.open(url, data, timeout)",
    Underline::Span("opener.open(url, data, timeout)"),
);
pub(crate) const OPENER_OPEN: Frame = stdlib(
    REQUEST,
    515,
    "open",
    "        response = self._open(req, data)",
    Underline::Span("self._open(req, data)"),
);
const OPENER_OPEN_RESPONSE: Frame = stdlib(
    REQUEST,
    521,
    "open",
    "        response = meth(req, response)",
    Underline::Span("meth(req, response)"),
);
pub(crate) const OPENER_OPEN_UNDERSCORE: Frame = stdlib(
    REQUEST,
    532,
    "_open",
    "        result = self._call_chain(self.handle_open, protocol, protocol +",
    Underline::Span("self._call_chain(self.handle_open, protocol, protocol +"),
);
pub(crate) const CALL_CHAIN: Frame = stdlib(
    REQUEST,
    492,
    "_call_chain",
    "            result = func(*args)",
    Underline::Span("func(*args)"),
);
pub(crate) const HTTPS_OPEN: Frame = stdlib(
    REQUEST,
    1392,
    "https_open",
    "        return self.do_open(http.client.HTTPSConnection, req,",
    Underline::Span("self.do_open(http.client.HTTPSConnection, req,"),
);
const DO_OPEN_REQUEST: Frame = stdlib(
    REQUEST,
    1344,
    "do_open",
    "                h.request(req.get_method(), req.selector, req.data, headers,",
    Underline::None,
);
pub(crate) const DO_OPEN_RAISE: Frame = stdlib(
    REQUEST,
    1347,
    "do_open",
    "                raise URLError(err)",
    Underline::None,
);
const HTTP_RESPONSE: Frame = stdlib(
    REQUEST,
    630,
    "http_response",
    "            response = self.parent.error(",
    Underline::Span("self.parent.error("),
);
const PARENT_ERROR: Frame = stdlib(
    REQUEST,
    559,
    "error",
    "        return self._call_chain(*args)",
    Underline::Span("self._call_chain(*args)"),
);
const HTTP_ERROR_DEFAULT: Frame = stdlib(
    REQUEST,
    639,
    "http_error_default",
    "        raise HTTPError(req.full_url, code, msg, hdrs, fp)",
    Underline::None,
);
const CLIENT_REQUEST: Frame = stdlib(
    CLIENT,
    1365,
    "request",
    "        self._send_request(method, url, body, headers, encode_chunked)",
    Underline::None,
);
const CLIENT_SEND_REQUEST_PUT: Frame = stdlib(
    CLIENT,
    1376,
    "_send_request",
    "        self.putrequest(method, url, **skips)",
    Underline::None,
);
const CLIENT_SEND_REQUEST_END: Frame = stdlib(
    CLIENT,
    1411,
    "_send_request",
    "        self.endheaders(body, encode_chunked=encode_chunked)",
    Underline::None,
);
const CLIENT_ENDHEADERS: Frame = stdlib(
    CLIENT,
    1360,
    "endheaders",
    "        self._send_output(message_body, encode_chunked=encode_chunked)",
    Underline::None,
);
const CLIENT_SEND_OUTPUT: Frame = stdlib(
    CLIENT,
    1120,
    "_send_output",
    "        self.send(msg)",
    Underline::None,
);
const CLIENT_SEND: Frame = stdlib(
    CLIENT,
    1064,
    "send",
    "            self.connect()",
    Underline::None,
);
const CLIENT_CONNECT: Frame = stdlib(
    CLIENT,
    1506,
    "connect",
    "            self.sock = self._context.wrap_socket(self.sock,",
    Underline::Span("self._context.wrap_socket(self.sock,"),
);
const PUTREQUEST: Frame = stdlib(
    CLIENT,
    1214,
    "putrequest",
    "        self._output(self._encode_request(request))",
    Underline::Span("self._encode_request(request)"),
);
const ENCODE_REQUEST: Frame = stdlib(
    CLIENT,
    1294,
    "_encode_request",
    "        return request.encode('ascii')",
    Underline::Span("request.encode('ascii')"),
);
const WRAP_SOCKET: Frame = stdlib(
    SSL,
    455,
    "wrap_socket",
    "        return self.sslsocket_class._create(",
    Underline::Span("self.sslsocket_class._create("),
);
const SSL_CREATE: Frame = stdlib(
    SSL,
    1042,
    "_create",
    "                    self.do_handshake()",
    Underline::None,
);
const SSL_HANDSHAKE: Frame = stdlib(
    SSL,
    1320,
    "do_handshake",
    "            self._sslobj.do_handshake()",
    Underline::None,
);

// ---- json ----

const JSON_LOAD: Frame = stdlib(
    JSON_INIT,
    293,
    "load",
    "    return loads(fp.read(),",
    Underline::Span("loads(fp.read(),"),
);
const JSON_LOADS_DECODE: Frame = stdlib(
    JSON_INIT,
    346,
    "loads",
    "        return _default_decoder.decode(s)",
    Underline::Span("_default_decoder.decode(s)"),
);
const JSON_LOADS_BYTES: Frame = stdlib(
    JSON_INIT,
    341,
    "loads",
    "        s = s.decode(detect_encoding(s), 'surrogatepass')",
    Underline::Span("s.decode(detect_encoding(s), 'surrogatepass')"),
);
const DECODER_DECODE: Frame = stdlib(
    JSON_DECODER,
    337,
    "decode",
    "        obj, end = self.raw_decode(s, idx=_w(s, 0).end())",
    Underline::Span("self.raw_decode(s, idx=_w(s, 0).end())"),
);
const DECODER_EXTRA_DATA: Frame = stdlib(
    JSON_DECODER,
    340,
    "decode",
    "            raise JSONDecodeError(\"Extra data\", s, end)",
    Underline::None,
);
const RAW_DECODE_EXPECTING: Frame = stdlib(
    JSON_DECODER,
    355,
    "raw_decode",
    "            raise JSONDecodeError(\"Expecting value\", s, err.value) from None",
    Underline::None,
);
const RAW_DECODE_SCAN: Frame = stdlib(
    JSON_DECODER,
    353,
    "raw_decode",
    "            obj, end = self.scan_once(s, idx)",
    Underline::Span("self.scan_once(s, idx)"),
);

/// The frames below `json.load(r)` for a failure raised at `site`, innermost first.
fn json_frames(site: JsonSite) -> Vec<Frame> {
    match site {
        JsonSite::BytesDecode => vec![JSON_LOADS_BYTES, JSON_LOAD],
        JsonSite::ExpectingValue => vec![
            RAW_DECODE_EXPECTING,
            DECODER_DECODE,
            JSON_LOADS_DECODE,
            JSON_LOAD,
        ],
        JsonSite::Scanner | JsonSite::Recursion => vec![
            RAW_DECODE_SCAN,
            DECODER_DECODE,
            JSON_LOADS_DECODE,
            JSON_LOAD,
        ],
        JsonSite::ExtraData => vec![DECODER_EXTRA_DATA, JSON_LOADS_DECODE, JSON_LOAD],
    }
}

/// The frames from `urlopen` down to where the exception was raised, innermost first; `None` for a failure with no site here.
fn urlopen_frames(error: &PyException) -> Option<Vec<Frame>> {
    match error.class {
        "HTTPError" => Some(vec![
            HTTP_ERROR_DEFAULT,
            CALL_CHAIN,
            PARENT_ERROR,
            HTTP_RESPONSE,
            OPENER_OPEN_RESPONSE,
            URLOPEN,
        ]),
        "URLError" => Some(vec![
            DO_OPEN_RAISE,
            HTTPS_OPEN,
            CALL_CHAIN,
            OPENER_OPEN_UNDERSCORE,
            OPENER_OPEN,
            URLOPEN,
        ]),
        "UnicodeEncodeError" => Some(vec![
            ENCODE_REQUEST,
            PUTREQUEST,
            CLIENT_SEND_REQUEST_PUT,
            CLIENT_REQUEST,
            DO_OPEN_REQUEST,
            HTTPS_OPEN,
            CALL_CHAIN,
            OPENER_OPEN_UNDERSCORE,
            OPENER_OPEN,
            URLOPEN,
        ]),
        _ => None,
    }
}

/// The frames of the first traceback of a `URLError` raised while a certificate failure was being handled, innermost first.
pub(crate) fn handshake_frames() -> Vec<Frame> {
    vec![
        SSL_HANDSHAKE,
        SSL_CREATE,
        WRAP_SOCKET,
        CLIENT_CONNECT,
        CLIENT_SEND,
        CLIENT_SEND_OUTPUT,
        CLIENT_ENDHEADERS,
        CLIENT_SEND_REQUEST_END,
        CLIENT_REQUEST,
        DO_OPEN_REQUEST,
    ]
}

/// What a traceback is for: an exception and the frames it passed through, innermost first.
pub struct Raise<'a> {
    pub error: &'a PyException,
    pub frames: Vec<Frame>,
}

/// The frames of an exception raised by `fetch_json` (below the script's own `main` and `fetch_json` frames).
fn fetch_frames(error: &PyException, origin: Origin) -> Vec<Frame> {
    let mut frames = match origin {
        Origin::Json { site, .. } => {
            let mut frames = json_frames(site);
            frames.push(FETCH_LOAD);
            frames
        }
        Origin::UrlOpen => match urlopen_frames(error) {
            Some(mut frames) => {
                frames.push(FETCH_URLOPEN);
                frames
            }
            None => return Vec::new(),
        },
    };
    frames.push(MAIN_FETCH);
    frames.push(MODULE_ENTRY);
    frames
}

/// The columns a character takes when the interpreter lays out a caret line: East Asian wide and fullwidth characters take two
/// (`_PyPegen_calculate_display_width` of CPython 3.12 asks `unicodedata.east_asian_width`; the ranges are those of the wide blocks).
fn display_width(ch: char) -> usize {
    match ch as u32 {
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

fn width_of(text: &str) -> usize {
    text.chars().map(display_width).sum()
}

/// The carets under `frame`'s statement, or nothing when the span is the whole statement.
fn caret_line(frame: &Frame, stripped: &str, indent: usize) -> Option<String> {
    let (needle, value_width) = match frame.underline {
        Underline::None => return None,
        Underline::Span(needle) | Underline::Always(needle) => (needle, None),
        Underline::Subscript { text, value_width } => (text, Some(value_width)),
    };
    let start = frame
        .text
        .find(needle)
        .expect("the needle is in the source line");
    let end = start + needle.len();
    if value_width.is_none()
        && !matches!(frame.underline, Underline::Always(_))
        && end - start == stripped.len()
    {
        return None;
    }
    let mut line = String::from("    ");
    line.push_str(&" ".repeat(width_of(&frame.text[indent..start])));
    match value_width {
        None => line.push_str(&"^".repeat(width_of(&frame.text[start..end]))),
        Some(value_width) => {
            let left_end = start
                + frame.text[start..]
                    .chars()
                    .take(value_width)
                    .map(char::len_utf8)
                    .sum::<usize>();
            line.push_str(&"~".repeat(width_of(&frame.text[start..left_end])));
            line.push_str(&"^".repeat(width_of(&frame.text[left_end..end])));
        }
    }
    Some(line)
}

fn frame_lines(frame: &Frame, script: &PyStr, out: &mut PyStr) {
    out.push_str("  File \"");
    match frame.file {
        FrameFile::Script => out.push_py(script),
        FrameFile::Stdlib(relative) => out.push_str(&format!("{STDLIB_PREFIX}/{relative}")),
    }
    out.push_str(&format!("\", line {}, in {}\n", frame.line, frame.function));
    let stripped = frame.text.trim_start_matches([' ', '\t', '\u{c}']);
    let indent = frame.text.len() - stripped.len();
    out.push_str("    ");
    out.push_str(stripped);
    out.push_str("\n");
    if let Some(carets) = caret_line(frame, stripped, indent) {
        out.push_str(&carets);
        out.push_str("\n");
    }
}

/// `Class: message` of `traceback.format_exception_only` (the module is shown for every class outside `builtins`), with its line end.
pub fn exception_block(error: &PyException) -> PyStr {
    let mut block = PyStr::new(&if error.module == "builtins" {
        error.class.to_string()
    } else {
        format!("{}.{}", error.module, error.class)
    });
    if !error.message.is_empty() {
        block.push_str(": ");
        block.push_py(&error.message);
    }
    block.push_str("\n");
    block
}

pub(crate) fn traceback(frames: &[Frame], script: &PyStr, block: &PyStr, out: &mut PyStr) {
    out.push_str("Traceback (most recent call last):\n");
    for frame in frames.iter().rev() {
        frame_lines(frame, script, out);
    }
    out.push_py(block);
}

/// The whole report of an exception raised by `fetch_json`, or only its `Class: message` block when no site is known for it. An exception that
/// was raised while a certificate failure was handled is preceded by the first traceback and the interpreter's linking sentence.
pub fn fetch_report(error: &PyException, origin: Origin, script: &PyStr) -> PyStr {
    let frames = fetch_frames(error, origin);
    let mut report = PyStr::default();
    if frames.is_empty() {
        report.push_py(&exception_block(error));
        return report;
    }
    if let (Origin::UrlOpen, "URLError", Some(crate::http_direct::Link::Context(reason))) =
        (origin, error.class, &error.link)
    {
        if reason.class == "SSLCertVerificationError" {
            traceback(
                &handshake_frames(),
                script,
                &exception_block(reason),
                &mut report,
            );
            report.push_str(
                "\nDuring handling of the above exception, another exception occurred:\n\n",
            );
        }
    }
    traceback(&frames, script, &exception_block(error), &mut report);
    report
}

/// The report of an exception raised by the script's own code: `inner` are the frames below `main` (innermost first, `main`'s own included).
pub fn script_report(error: &PyException, inner: &[Frame], script: &PyStr) -> PyStr {
    let mut frames = inner.to_vec();
    frames.push(MODULE_ENTRY);
    let mut report = PyStr::default();
    traceback(&frames, script, &exception_block(error), &mut report);
    report
}

/// The report of a failed `print` of the result or of an error report: the frame of the `print`, the entry and the exception's block.
pub fn print_report(print_frame: Frame, block: &PyStr, script: &PyStr) -> PyStr {
    let mut report = PyStr::default();
    traceback(&[print_frame, MODULE_ENTRY], script, block, &mut report);
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::failure::http_error;
    use crate::wheel_json::json_decode_error;

    const SCRIPT: &str = "/x/resolve_wheel.py";

    fn text(report: PyStr) -> String {
        report.to_utf8().unwrap()
    }

    #[test]
    fn a_subscript_is_marked_with_tilde_under_the_value_and_carets_under_the_brackets() {
        let error = crate::http_direct::failure::value_error("m");
        let report = text(script_report(&error, &[MAIN_NAME], &PyStr::new(SCRIPT)));
        let lines: Vec<&str> = report.lines().collect();
        assert_eq!(
            lines[4],
            "    assets = [x[\"name\"] for x in rel.get(\"assets\", [])]"
        );
        assert_eq!(lines[5], "              ~^^^^^^^^");
    }

    #[test]
    fn spans_follow_the_source_line_and_wide_characters_take_two_columns() {
        let error = crate::http_direct::failure::value_error("m");
        let report = text(script_report(
            &error,
            &[HIGHEST_KEY, MAIN_HIGHEST],
            &PyStr::new(SCRIPT),
        ));
        let lines: Vec<&str> = report.lines().collect();
        // under the 43-character `sorted(...)` call of line 83, then the 14-character `int(c["cuda"])` of the lambda
        assert_eq!(
            lines[4],
            "    chosen = sorted(cands, key=lambda c: int(c[\"cuda\"]))[-1]  # 가장 높은 cuXXX"
        );
        assert_eq!(lines[5], format!("    {}{}", " ".repeat(9), "^".repeat(43)));
        assert_eq!(
            lines[8],
            format!("    {}{}", " ".repeat(37), "^".repeat(14))
        );
    }

    #[test]
    fn a_whole_line_statement_and_a_bare_entry_show_no_caret_line() {
        let error = crate::http_direct::failure::value_error("m");
        let report = text(print_report(
            MAIN_PRINT_RESULT,
            &exception_block(&error),
            &PyStr::new(SCRIPT),
        ));
        assert_eq!(
            report,
            "Traceback (most recent call last):\n  File \"/x/resolve_wheel.py\", line 98, in <module>\n    main()\n  File \"/x/resolve_wheel.py\", line 89, in main\n    print(json.dumps({\nValueError: m\n"
        );
    }

    #[test]
    fn the_report_of_an_http_status_names_the_urllib_chain_of_the_saved_original() {
        let error = http_error("https://x/y", 404, "Not Found", &[]);
        let report = text(fetch_report(&error, Origin::UrlOpen, &PyStr::new(SCRIPT)));
        let frames: Vec<&str> = report
            .lines()
            .filter(|line| line.starts_with("  File"))
            .collect();
        let expected = [
            "  File \"/x/resolve_wheel.py\", line 98, in <module>",
            "  File \"/x/resolve_wheel.py\", line 33, in main",
            "  File \"/x/resolve_wheel.py\", line 21, in fetch_json",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 215, in urlopen",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 521, in open",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 630, in http_response",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 559, in error",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 492, in _call_chain",
            "  File \"/usr/lib/python3.12/urllib/request.py\", line 639, in http_error_default",
        ];
        assert_eq!(frames, expected);
        assert!(report.ends_with("urllib.error.HTTPError: HTTP Error 404: Not Found\n"));
    }

    #[test]
    fn a_json_error_raised_from_none_shows_only_its_own_traceback() {
        let doc = PyStr::new("{\"assets\":[");
        let error = json_decode_error("Expecting value", &doc, 11, 1, 12);
        let report = text(fetch_report(
            &error,
            Origin::Json {
                site: JsonSite::ExpectingValue,
                from_none: true,
            },
            &PyStr::new(SCRIPT),
        ));
        assert!(report.contains("  File \"/usr/lib/python3.12/json/decoder.py\", line 355, in raw_decode\n    raise JSONDecodeError(\"Expecting value\", s, err.value) from None\n"));
        assert!(report.ends_with(
            "json.decoder.JSONDecodeError: Expecting value: line 1 column 12 (char 11)\n"
        ));
        assert!(!report.contains("During handling"));
    }

    #[test]
    fn an_exception_without_a_known_site_is_its_block_alone() {
        let error =
            crate::http_direct::failure::os_error("Tunnel connection failed: 403 Forbidden");
        assert_eq!(
            text(fetch_report(&error, Origin::UrlOpen, &PyStr::new(SCRIPT))),
            "OSError: Tunnel connection failed: 403 Forbidden\n"
        );
    }
}
