//! The report of an exception `regen_requirements.py` does not handle: CPython 3.12.3's `Traceback (most recent call last):` blocks for the sites
//! a run can raise through, laid out by the shared renderer of [`crate::wheel_traceback`].
//!
//! **Provenance.** The script's lines (77, 95, 96, 101, 162, 164, 176, 178, 188) and the `zipfile` (1365, 1432) and `urllib.request` frames of a
//! missing local file (1506, 1484, 1523), a refused certificate (with its chained first traceback) and an unknown URL type (318, 344, 373, 499; only 318 carries carets) were read
//! from the saved raw tracebacks of the original capture; the script's lines 83, 99, 100, 179, 181, 183 and the frames shared with the `fetch_json`
//! owner come from the same source file and the rules the renderer already follows (DERIVED, not captured). A failure raised anywhere this table has
//! no site for (a failed tunnel, a bad CRC deep inside `zipfile`, an installed lookup error other than the identity check, ...) is reported by its
//! `Class: message` block alone, never with invented frames.

use crate::http_direct::{Link, PyException};
use crate::regen_flow::Stage;
use crate::wheel_traceback::{
    exception_block, handshake_frames, script, stdlib, traceback, Frame, Underline, CALL_CHAIN,
    DO_OPEN_RAISE, HTTPS_OPEN, OPENER_OPEN, OPENER_OPEN_UNDERSCORE, REQUEST, URLOPEN,
};
use agent_runtime_support::pystr::PyStr;

const ZIPFILE: &str = "zipfile/__init__.py";

/// The kind of URL the wheel was asked for; the urllib frames of a failure depend on its handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scheme {
    Http,
    Https,
    File,
    Other,
}

/// Where an uncaught exception was raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Site {
    /// `wheel_metadata(a.from_wheel_url)` at the stage that failed.
    Wheel { stage: Stage, scheme: Scheme },
    /// `installed_metadata()` at the stage that failed.
    Installed { stage: Stage },
    /// `open(a.out, "w", encoding="utf-8")`.
    OpenOutput,
}

/// The `print` of the run's report lines that failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrintSite {
    Written,
    Version,
    Exemption,
    Ceilings,
}

const MODULE_ENTRY: Frame = script(188, "<module>", "    main()", Underline::None);
const MAIN_WHEEL: Frame = script(
    164,
    "main",
    "        (reqs, version), src = wheel_metadata(a.from_wheel_url), a.from_wheel_url",
    Underline::Span("wheel_metadata(a.from_wheel_url)"),
);
const MAIN_INSTALLED: Frame = script(
    162,
    "main",
    "        (reqs, version), src = installed_metadata(), \"importlib.metadata.distribution('vllm').metadata (container)\"",
    Underline::Span("installed_metadata()"),
);
const OPEN_OUTPUT: Frame = script(
    176,
    "main",
    "    with open(a.out, \"w\", encoding=\"utf-8\") as f:",
    Underline::Span("open(a.out, \"w\", encoding=\"utf-8\")"),
);
const WHEEL_RETRIEVE: Frame = script(
    95,
    "wheel_metadata",
    "        urllib.request.urlretrieve(url, whl)",
    Underline::None,
);
const WHEEL_ZIPFILE: Frame = script(
    96,
    "wheel_metadata",
    "        with zipfile.ZipFile(whl) as z:",
    Underline::Span("zipfile.ZipFile(whl)"),
);
const WHEEL_UNIQUE: Frame = script(
    99,
    "wheel_metadata",
    "                raise RuntimeError(\"wheel 에 유일한 METADATA 없음\")",
    Underline::None,
);
const WHEEL_DECODE: Frame = script(
    100,
    "wheel_metadata",
    "            txt = z.read(metas[0]).decode(\"utf-8\")",
    Underline::Span("z.read(metas[0]).decode(\"utf-8\")"),
);
const WHEEL_FIELDS: Frame = script(
    101,
    "wheel_metadata",
    "    return _metadata_fields(Parser().parsestr(txt))",
    Underline::Span("_metadata_fields(Parser().parsestr(txt))"),
);
const INSTALLED_FIELDS: Frame = script(
    83,
    "installed_metadata",
    "    return _metadata_fields(m.distribution(\"vllm\").metadata)",
    Underline::Span("_metadata_fields(m.distribution(\"vllm\").metadata)"),
);
const FIELDS_RAISE: Frame = script(
    77,
    "_metadata_fields",
    "        raise RuntimeError(\"vLLM METADATA Name/Version missing, malformed or ambiguous\")",
    Underline::None,
);

const URLRETRIEVE: Frame = stdlib(
    REQUEST,
    240,
    "urlretrieve",
    "    with contextlib.closing(urlopen(url, data)) as fp:",
    Underline::Span("urlopen(url, data)"),
);
const REQUEST_NEW: Frame = stdlib(
    REQUEST,
    499,
    "open",
    "        req = Request(fullurl, data)",
    Underline::Span("Request(fullurl, data)"),
);
const REQUEST_INIT: Frame = stdlib(
    REQUEST,
    318,
    "__init__",
    "        self.full_url = url",
    Underline::Always("self.full_url"),
);
const REQUEST_FULL_URL: Frame = stdlib(
    REQUEST,
    344,
    "full_url",
    "        self._parse()",
    Underline::None,
);
const REQUEST_PARSE: Frame = stdlib(
    REQUEST,
    373,
    "_parse",
    "            raise ValueError(\"unknown url type: %r\" % self.full_url)",
    Underline::None,
);
const FILE_OPEN: Frame = stdlib(
    REQUEST,
    1484,
    "file_open",
    "            return self.open_local_file(req)",
    Underline::Span("self.open_local_file(req)"),
);
const OPEN_LOCAL_RAISE: Frame = stdlib(
    REQUEST,
    1523,
    "open_local_file",
    "            raise URLError(exp)",
    Underline::None,
);
const OPEN_LOCAL_STAT: Frame = stdlib(
    REQUEST,
    1506,
    "open_local_file",
    "            stats = os.stat(localfile)",
    Underline::Span("os.stat(localfile)"),
);
const ZIP_INIT: Frame = stdlib(
    ZIPFILE,
    1365,
    "__init__",
    "                self._RealGetContents()",
    Underline::None,
);
const ZIP_REAL_CONTENTS: Frame = stdlib(
    ZIPFILE,
    1432,
    "_RealGetContents",
    "            raise BadZipFile(\"File is not a zip file\")",
    Underline::None,
);

const PRINT_WRITTEN: Frame = script(
    178,
    "main",
    "    print(f\"[regen] {a.out} : {len(lines)} packages (source: {src})\")",
    Underline::None,
);
const PRINT_VERSION: Frame = script(
    179,
    "main",
    "    print(f\"[regen] distribution METADATA Version: {version}\")",
    Underline::None,
);
const PRINT_EXEMPTION: Frame = script(
    181,
    "main",
    "        print(\"[regen] KNOWN_INCOMPAT exemption: transformers<5.15.0 omitted for exact 0.32.0; compatibility and smoke unverified\")",
    Underline::None,
);
const PRINT_CEILINGS: Frame = script(
    183,
    "main",
    "        print(\"[regen] ⚠ KNOWN_INCOMPAT 천장 적용: \" + \", \".join(applied)",
    Underline::None,
);

fn is(error: &PyException, class: &str, message: Option<&str>) -> bool {
    error.class == class
        && message.is_none_or(|text| error.message.to_utf8().as_deref() == Some(text))
}

/// The urllib frames from `urlretrieve` upwards (script frames included), innermost first, below `below`.
fn through_urlretrieve(mut below: Vec<Frame>) -> Vec<Frame> {
    below.extend([URLRETRIEVE, WHEEL_RETRIEVE, MAIN_WHEEL, MODULE_ENTRY]);
    below
}

/// The frames of an exception raised at `site`, or `None` when this module has no site for it.
fn frames_for(error: &PyException, site: &Site) -> Option<Vec<Frame>> {
    match *site {
        Site::OpenOutput => Some(vec![OPEN_OUTPUT, MODULE_ENTRY]),
        Site::Installed {
            stage: Stage::InstalledIdentity,
        } => Some(vec![
            FIELDS_RAISE,
            INSTALLED_FIELDS,
            MAIN_INSTALLED,
            MODULE_ENTRY,
        ]),
        Site::Installed { .. } => None,
        Site::Wheel { stage, scheme } => match stage {
            Stage::Urlretrieve => {
                if is(error, "ValueError", None)
                    && error
                        .message
                        .to_utf8()
                        .is_some_and(|text| text.starts_with("unknown url type"))
                {
                    return Some(through_urlretrieve(vec![
                        REQUEST_PARSE,
                        REQUEST_FULL_URL,
                        REQUEST_INIT,
                        REQUEST_NEW,
                        URLOPEN,
                    ]));
                }
                match (scheme, &error.link) {
                    (Scheme::File, Some(Link::Context(_))) if error.class == "URLError" => {
                        Some(through_urlretrieve(vec![
                            OPEN_LOCAL_RAISE,
                            FILE_OPEN,
                            CALL_CHAIN,
                            OPENER_OPEN_UNDERSCORE,
                            OPENER_OPEN,
                            URLOPEN,
                        ]))
                    }
                    (Scheme::Https, Some(Link::Context(reason)))
                        if error.class == "URLError"
                            && reason.class == "SSLCertVerificationError" =>
                    {
                        Some(through_urlretrieve(vec![
                            DO_OPEN_RAISE,
                            HTTPS_OPEN,
                            CALL_CHAIN,
                            OPENER_OPEN_UNDERSCORE,
                            OPENER_OPEN,
                            URLOPEN,
                        ]))
                    }
                    _ => None,
                }
            }
            Stage::ZipOpen if is(error, "BadZipFile", Some("File is not a zip file")) => {
                Some(vec![
                    ZIP_REAL_CONTENTS,
                    ZIP_INIT,
                    WHEEL_ZIPFILE,
                    MAIN_WHEEL,
                    MODULE_ENTRY,
                ])
            }
            Stage::ZipMember
                if is(
                    error,
                    "RuntimeError",
                    Some(crate::regen_flow::NO_UNIQUE_METADATA),
                ) =>
            {
                Some(vec![WHEEL_UNIQUE, MAIN_WHEEL, MODULE_ENTRY])
            }
            Stage::ZipMember if error.class == "UnicodeDecodeError" => {
                Some(vec![WHEEL_DECODE, MAIN_WHEEL, MODULE_ENTRY])
            }
            Stage::WheelIdentity => {
                Some(vec![FIELDS_RAISE, WHEEL_FIELDS, MAIN_WHEEL, MODULE_ENTRY])
            }
            _ => None,
        },
    }
}

/// The chained first traceback of a failure raised while another was handled, with the interpreter's linking sentence, when that applies.
fn chained_first(error: &PyException, site: &Site, script: &PyStr, out: &mut PyStr) {
    let Site::Wheel {
        stage: Stage::Urlretrieve,
        scheme,
    } = *site
    else {
        return;
    };
    let Some(Link::Context(reason)) = &error.link else {
        return;
    };
    let first = match (scheme, error.class) {
        (Scheme::File, "URLError") => vec![OPEN_LOCAL_STAT],
        (Scheme::Https, "URLError") if reason.class == "SSLCertVerificationError" => {
            handshake_frames()
        }
        _ => return,
    };
    traceback(&first, script, &exception_block(reason), out);
    out.push_str("\nDuring handling of the above exception, another exception occurred:\n\n");
}

/// The whole report of an uncaught exception.
pub fn report(error: &PyException, site: &Site, script: &PyStr) -> PyStr {
    let mut out = PyStr::default();
    let Some(frames) = frames_for(error, site) else {
        out.push_py(&exception_block(error));
        return out;
    };
    chained_first(error, site, script, &mut out);
    traceback(&frames, script, &exception_block(error), &mut out);
    out
}

/// The report of a failed `print` of one of the run's report lines.
pub fn print_report(print: PrintSite, block: &PyStr, script: &PyStr) -> PyStr {
    let frame = match print {
        PrintSite::Written => PRINT_WRITTEN,
        PrintSite::Version => PRINT_VERSION,
        PrintSite::Exemption => PRINT_EXEMPTION,
        PrintSite::Ceilings => PRINT_CEILINGS,
    };
    let mut out = PyStr::default();
    traceback(&[frame, MODULE_ENTRY], script, block, &mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::py_errors::{builtin, with_message};

    const SCRIPT: &str = "/x/regen_requirements.py";

    fn text(report: PyStr) -> String {
        report.to_utf8().unwrap()
    }

    #[test]
    fn the_identity_failure_of_a_wheel_shows_the_three_script_frames_with_carets_on_the_call() {
        let error = builtin("RuntimeError", crate::requirements_rules::IDENTITY_FAILURE);
        let site = Site::Wheel {
            stage: Stage::WheelIdentity,
            scheme: Scheme::File,
        };
        let shown = text(report(&error, &site, &PyStr::new(SCRIPT)));
        let expected = "Traceback (most recent call last):\n  File \"/x/regen_requirements.py\", line 188, in <module>\n    main()\n  File \"/x/regen_requirements.py\", line 164, in main\n    (reqs, version), src = wheel_metadata(a.from_wheel_url), a.from_wheel_url\n                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"/x/regen_requirements.py\", line 101, in wheel_metadata\n    return _metadata_fields(Parser().parsestr(txt))\n           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n  File \"/x/regen_requirements.py\", line 77, in _metadata_fields\n    raise RuntimeError(\"vLLM METADATA Name/Version missing, malformed or ambiguous\")\nRuntimeError: vLLM METADATA Name/Version missing, malformed or ambiguous\n";
        assert_eq!(shown, expected);
    }

    #[test]
    fn a_non_zip_file_is_reported_through_zipfile_without_carets_on_whole_line_statements() {
        let error = with_message("BadZipFile", "zipfile", "File is not a zip file");
        let site = Site::Wheel {
            stage: Stage::ZipOpen,
            scheme: Scheme::File,
        };
        let shown = text(report(&error, &site, &PyStr::new(SCRIPT)));
        assert!(shown.contains("  File \"/usr/lib/python3.12/zipfile/__init__.py\", line 1432, in _RealGetContents\n    raise BadZipFile(\"File is not a zip file\")\nzipfile.BadZipFile: File is not a zip file\n"));
        assert!(shown.contains("line 1365, in __init__\n    self._RealGetContents()\n  File"));
    }

    #[test]
    fn an_unknown_url_type_shows_the_request_constructor_frames_with_forced_carets() {
        let error = builtin("ValueError", "unknown url type: ''");
        let site = Site::Wheel {
            stage: Stage::Urlretrieve,
            scheme: Scheme::Other,
        };
        let shown = text(report(&error, &site, &PyStr::new(SCRIPT)));
        assert!(
            shown.contains("line 318, in __init__\n    self.full_url = url\n    ^^^^^^^^^^^^^\n")
        );
        assert!(shown.contains("line 344, in full_url\n    self._parse()\n  File"));
        assert!(shown.ends_with("ValueError: unknown url type: ''\n"));
    }

    #[test]
    fn a_missing_local_file_chains_the_file_not_found_traceback_before_the_url_error() {
        let reason = with_message(
            "FileNotFoundError",
            "builtins",
            "[Errno 2] No such file or directory: 'm.whl'",
        );
        let error = PyException {
            link: Some(Link::Context(Box::new(reason))),
            ..with_message(
                "URLError",
                "urllib.error",
                "<urlopen error [Errno 2] No such file or directory: 'm.whl'>",
            )
        };
        let site = Site::Wheel {
            stage: Stage::Urlretrieve,
            scheme: Scheme::File,
        };
        let shown = text(report(&error, &site, &PyStr::new(SCRIPT)));
        let first = shown.find("line 1506, in open_local_file").unwrap();
        let link = shown
            .find("During handling of the above exception, another exception occurred:")
            .unwrap();
        let second = shown.find("line 1523, in open_local_file").unwrap();
        assert!(first < link && link < second);
        assert!(shown.ends_with(
            "urllib.error.URLError: <urlopen error [Errno 2] No such file or directory: 'm.whl'>\n"
        ));
    }

    #[test]
    fn sites_without_a_table_entry_print_the_block_alone() {
        let error = builtin("OSError", "Tunnel connection failed: 403 Forbidden");
        let site = Site::Wheel {
            stage: Stage::Urlretrieve,
            scheme: Scheme::Https,
        };
        assert_eq!(
            text(report(&error, &site, &PyStr::new(SCRIPT))),
            "OSError: Tunnel connection failed: 403 Forbidden\n"
        );
    }

    #[test]
    fn output_and_print_failures_have_their_own_frames() {
        let error = with_message(
            "IsADirectoryError",
            "builtins",
            "[Errno 21] Is a directory: 'output-dir'",
        );
        let shown = text(report(&error, &Site::OpenOutput, &PyStr::new(SCRIPT)));
        assert!(shown.contains("line 176, in main\n    with open(a.out, \"w\", encoding=\"utf-8\") as f:\n         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^\n"));
        let block = PyStr::new("BrokenPipeError: [Errno 32] Broken pipe\n");
        let shown = text(print_report(
            PrintSite::Written,
            &block,
            &PyStr::new(SCRIPT),
        ));
        assert!(shown.contains("line 178, in main\n    print(f\"[regen] {a.out} : {len(lines)} packages (source: {src})\")\nBrokenPipeError"));
    }
}
