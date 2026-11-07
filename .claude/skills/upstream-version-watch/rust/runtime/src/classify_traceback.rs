//! The reports of an exception `classify_failure.py` does not handle: CPython 3.12.3 `Traceback (most recent call last):` blocks for the sites a run
//! can raise through, laid out by the shared renderer of [`crate::wheel_traceback`].
//!
//! **Provenance.** CAPTURED (read from the saved raw tracebacks of the original capture): script lines 91, 71 (open of `--log`), 72 -> 24 (open of
//! the registry), 75 and the `re` frames of an unterminated character set (`__init__.py` 177/307, `_compiler.py` 745, `_parser.py` 979/460/568), 77
//! (`KeyError: 'class'`) and 83 (the unknown-class print). DERIVED from the source file and the rules the renderer already follows: 72 -> 30 (the
//! `unicode_escape` decode), 80 (the matched-class print) and the second `unterminated character set` raise (`_parser.py` 591). A failure raised
//! anywhere this table has no site for is *refused by name* by the caller, never printed with invented frames.

use crate::http_direct::PyException;
use crate::wheel_traceback::{exception_block, script, stdlib, traceback, Frame, Underline};
use agent_runtime_support::pystr::PyStr;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// `open(a.log, encoding="utf-8", errors="replace")` (line 71).
    OpenLog,
    /// `open(path, encoding="utf-8")` inside `load_patterns` (lines 72 -> 24).
    OpenPatterns,
    /// `.decode("unicode_escape")` inside `load_patterns` (lines 72 -> 30).
    EscapeDecode,
    /// `re.search(p["signature"], text)` failing in the pattern parser at the given `_parser.py` raise line (lines 75 -> `re`).
    Parse { raise_line: u32 },
    /// `p["class"]` (line 77).
    ClassKey,
    /// `print(json.dumps(out, ...))` of a matched class (line 80).
    PrintMatched,
    /// `print(json.dumps({"class": "unknown", ...}))` (line 83).
    PrintUnknown,
}

const MODULE_ENTRY: Frame = script(91, "<module>", "    main()", Underline::None);
const MAIN_LOG: Frame = script(
    71,
    "main",
    "    text = open(a.log, encoding=\"utf-8\", errors=\"replace\").read() if a.log else sys.stdin.read()",
    Underline::Span("open(a.log, encoding=\"utf-8\", errors=\"replace\")"),
);
const MAIN_LOAD: Frame = script(
    72,
    "main",
    "    pats = load_patterns(a.patterns)",
    Underline::Span("load_patterns(a.patterns)"),
);
const LOAD_OPEN: Frame = script(
    24,
    "load_patterns",
    "    with open(path, encoding=\"utf-8\") as f:",
    Underline::Span("open(path, encoding=\"utf-8\")"),
);
const LOAD_ESCAPE: Frame = script(
    30,
    "load_patterns",
    "                cur = {\"signature\": m.group(1).encode().decode(\"unicode_escape\")}",
    Underline::Span("m.group(1).encode().decode(\"unicode_escape\")"),
);
const MAIN_SEARCH: Frame = script(
    75,
    "main",
    "        m = re.search(p[\"signature\"], text)",
    Underline::Span("re.search(p[\"signature\"], text)"),
);
const MAIN_CLASS: Frame = script(
    77,
    "main",
    "            out = {\"class\": p[\"class\"], \"matched_signature\": p[\"signature\"],",
    Underline::Subscript {
        text: "p[\"class\"]",
        value_width: 1,
    },
);
const PRINT_MATCHED: Frame = script(
    80,
    "main",
    "            print(json.dumps(out, ensure_ascii=False, indent=2))",
    Underline::None,
);
const PRINT_UNKNOWN: Frame = script(
    83,
    "main",
    "    print(json.dumps({\"class\": \"unknown\",",
    Underline::None,
);

fn parse_frames(raise_line: u32) -> Vec<Frame> {
    let raise = match raise_line {
        568 => stdlib(
            "re/_parser.py",
            568,
            "_parse",
            "                raise source.error(\"unterminated character set\",",
            Underline::None,
        ),
        _ => stdlib(
            "re/_parser.py",
            591,
            "_parse",
            "                        raise source.error(\"unterminated character set\",",
            Underline::None,
        ),
    };
    vec![
        raise,
        stdlib(
            "re/_parser.py",
            460,
            "_parse_sub",
            "            itemsappend(_parse(source, state, verbose, nested + 1,",
            Underline::Span("_parse(source, state, verbose, nested + 1,"),
        ),
        stdlib(
            "re/_parser.py",
            979,
            "parse",
            "        p = _parse_sub(source, state, flags & SRE_FLAG_VERBOSE, 0)",
            Underline::Span("_parse_sub(source, state, flags & SRE_FLAG_VERBOSE, 0)"),
        ),
        stdlib(
            "re/_compiler.py",
            745,
            "compile",
            "        p = _parser.parse(p, flags)",
            Underline::Span("_parser.parse(p, flags)"),
        ),
        stdlib(
            "re/__init__.py",
            307,
            "_compile",
            "        p = _compiler.compile(pattern, flags)",
            Underline::Span("_compiler.compile(pattern, flags)"),
        ),
        stdlib(
            "re/__init__.py",
            177,
            "search",
            "    return _compile(pattern, flags).search(string)",
            Underline::Span("_compile(pattern, flags)"),
        ),
        MAIN_SEARCH,
    ]
}

/// The frames of `site`, innermost first, the module entry last.
fn frames_of(site: Site) -> Vec<Frame> {
    let mut frames = match site {
        Site::OpenLog => vec![MAIN_LOG],
        Site::OpenPatterns => vec![LOAD_OPEN, MAIN_LOAD],
        Site::EscapeDecode => vec![LOAD_ESCAPE, MAIN_LOAD],
        Site::Parse { raise_line } => parse_frames(raise_line),
        Site::ClassKey => vec![MAIN_CLASS],
        Site::PrintMatched => vec![PRINT_MATCHED],
        Site::PrintUnknown => vec![PRINT_UNKNOWN],
    };
    frames.push(MODULE_ENTRY);
    frames
}

/// The report of `error` raised at `site`.
pub fn report(error: &PyException, site: Site, script_path: &PyStr) -> PyStr {
    block_report(&exception_block(error), site, script_path)
}

/// The report for an exception block already rendered (a failed `print`).
pub fn block_report(block: &PyStr, site: Site, script_path: &PyStr) -> PyStr {
    let mut out = PyStr::default();
    traceback(&frames_of(site), script_path, block, &mut out);
    out
}
