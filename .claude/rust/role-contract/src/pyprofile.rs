//! The text profile a Python 3.12 process gets at startup, derived from the process environment (no global setting is
//! touched): which codec stdout/stderr use and with which error handler, the file-system encoding of paths and argv, and the
//! preferred encoding of `subprocess.run(text=True)` and of `open()` without an `encoding`.
//!
//! Rules (CPython 3.12 startup, checked against six interpreter metadata rows, PIDs 1425187..1425192, see `PORTING.md`):
//! * the effective `LC_CTYPE` is `LC_ALL`, else `LC_CTYPE`, else `LANG`, else `C`;
//! * UTF-8 mode is on when `PYTHONUTF8=1`, off when `PYTHONUTF8=0`, and otherwise on exactly when that locale is `C` or `POSIX`;
//! * outside UTF-8 mode a `C`/`POSIX` locale is coerced to `C.UTF-8` unless `LC_ALL` is set or `PYTHONCOERCECLOCALE=0`;
//! * the file-system and preferred encodings are UTF-8 (UTF-8 mode, a UTF-8 locale, or a coerced locale) or ASCII (the
//!   `C`/`POSIX` locale that stayed uncoerced); a locale whose charset is neither is a concrete unsupported condition;
//! * stdout/stdin use `PYTHONIOENCODING`'s encoding (`utf-8` or `ascii` modelled) and error handler when given; otherwise the
//!   locale's encoding with `surrogateescape` in UTF-8 mode and for the `C`, `POSIX`, `C.UTF-8`, `C.utf8` and `UTF-8`
//!   locales and `strict` for other UTF-8 locales; stderr always uses `backslashreplace`.
//!
//! The profile is a thread-local scope ([`with_profile`]) so that the path, Git and report code reads one answer.

use agent_runtime_support::pystr::PyStr;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Utf8,
    Ascii,
}

impl Encoding {
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Utf8 => "utf-8",
            Encoding::Ascii => "ascii",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Errors {
    Strict,
    SurrogateEscape,
    BackslashReplace,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextProfile {
    pub utf8_mode: bool,
    pub stdout: Encoding,
    pub stdout_errors: Errors,
    pub filesystem: Encoding,
    pub preferred: Encoding,
}

fn nonempty(value: Option<Vec<u8>>) -> Option<Vec<u8>> {
    value.filter(|v| !v.is_empty())
}

fn has_utf8_charset(name: &[u8]) -> bool {
    let lower: Vec<u8> = name.iter().map(u8::to_ascii_lowercase).collect();
    lower == b"utf-8"
        || lower.windows(5).any(|w| w == b".utf8")
        || lower.windows(6).any(|w| w == b".utf-8")
}

impl TextProfile {
    /// The profile of `Python 3.12` started with this environment (`get` reads one variable).
    pub fn from_environment(get: &dyn Fn(&str) -> Option<Vec<u8>>) -> Result<TextProfile, String> {
        let lc_all = nonempty(get("LC_ALL"));
        let ctype = lc_all
            .clone()
            .or_else(|| nonempty(get("LC_CTYPE")))
            .or_else(|| nonempty(get("LANG")));
        let name = ctype.unwrap_or_else(|| b"C".to_vec());
        let is_c = name == b"C" || name == b"POSIX";
        let utf8_mode = match get("PYTHONUTF8").as_deref() {
            Some(b"1") => true,
            Some(b"0") => false,
            None | Some(b"") => is_c,
            Some(other) => {
                return Err(format!(
                    "PYTHONUTF8={:?} is invalid (Python 3.12 accepts 0 or 1)",
                    String::from_utf8_lossy(other)
                ))
            }
        };
        let coerce_allowed = get("PYTHONCOERCECLOCALE").as_deref() != Some(b"0");
        let coerced = !utf8_mode && is_c && lc_all.is_none() && coerce_allowed;
        let utf8_locale = has_utf8_charset(&name) || coerced;
        let locale_encoding = if utf8_mode || utf8_locale {
            Encoding::Utf8
        } else if is_c {
            Encoding::Ascii
        } else {
            return Err(format!(
                "locale {:?} has a charset that is neither UTF-8 nor the C locale's ASCII (not modelled)",
                String::from_utf8_lossy(&name)
            ));
        };
        let legacy_surrogate_locale = matches!(name.as_slice(), b"C.UTF-8" | b"C.utf8" | b"UTF-8");
        let mut stdout = locale_encoding;
        let mut errors = if utf8_mode || is_c || coerced || legacy_surrogate_locale {
            Errors::SurrogateEscape
        } else {
            Errors::Strict
        };
        if let Some(io) = nonempty(get("PYTHONIOENCODING")) {
            let text = String::from_utf8_lossy(&io).to_string();
            let (encoding, handler) = match text.split_once(':') {
                Some((e, h)) => (e.to_string(), Some(h.to_string())),
                None => (text.clone(), None),
            };
            if !encoding.is_empty() {
                stdout = match encoding.to_ascii_lowercase().replace('_', "-").as_str() {
                    "utf-8" | "utf8" => Encoding::Utf8,
                    "ascii" | "us-ascii" | "ansi-x3.4-1968" | "646" => Encoding::Ascii,
                    other => {
                        return Err(format!(
                        "PYTHONIOENCODING encoding {other:?} is not modelled (utf-8 and ascii are)"
                    ))
                    }
                };
            }
            if let Some(handler) = handler.filter(|h| !h.is_empty()) {
                errors = match handler.as_str() {
                    "strict" => Errors::Strict,
                    "surrogateescape" => Errors::SurrogateEscape,
                    "backslashreplace" => Errors::BackslashReplace,
                    other => {
                        return Err(format!(
                            "PYTHONIOENCODING error handler {other:?} is not modelled"
                        ))
                    }
                };
            }
        }
        Ok(TextProfile {
            utf8_mode,
            stdout,
            stdout_errors: errors,
            filesystem: locale_encoding,
            preferred: locale_encoding,
        })
    }

    /// UTF-8 everywhere with `surrogateescape` (the profile of an unset or `C.UTF-8` environment).
    pub fn utf8() -> TextProfile {
        TextProfile {
            utf8_mode: false,
            stdout: Encoding::Utf8,
            stdout_errors: Errors::SurrogateEscape,
            filesystem: Encoding::Utf8,
            preferred: Encoding::Utf8,
        }
    }
}

thread_local! {
    static CURRENT: RefCell<TextProfile> = RefCell::new(TextProfile::utf8());
}

/// Runs `body` with `profile` as the current text profile of this thread.
pub fn with_profile<R>(profile: &TextProfile, body: impl FnOnce() -> R) -> R {
    let previous = CURRENT.with(|c| c.replace(profile.clone()));
    let result = body();
    CURRENT.with(|c| *c.borrow_mut() = previous);
    result
}

pub fn current() -> TextProfile {
    CURRENT.with(|c| c.borrow().clone())
}

/// `os.fsdecode(bytes)`: UTF-8 (invalid bytes as lone surrogates) or ASCII (every byte >= 0x80 as a lone surrogate).
pub fn fs_decode(bytes: &[u8]) -> PyStr {
    match current().filesystem {
        Encoding::Utf8 => PyStr::from_surrogateescape(bytes),
        Encoding::Ascii => {
            let points: Vec<u32> = bytes
                .iter()
                .map(|b| {
                    if *b < 0x80 {
                        u32::from(*b)
                    } else {
                        0xDC00 + u32::from(*b)
                    }
                })
                .collect();
            PyStr::from_code_points(points)
        }
    }
}

/// A `UnicodeEncodeError` of the profile's codec: its constructor parts (the object is the encoded text) and `str()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EncodeFailure {
    pub encoding: &'static str,
    pub start: usize,
    /// Exclusive end, as `UnicodeEncodeError.end`.
    pub end: usize,
    pub reason: String,
    pub message: String,
}

impl std::fmt::Display for EncodeFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

fn encode_failure(encoding: Encoding, points: &[u32], first: usize, last: usize) -> EncodeFailure {
    let reason = match encoding {
        Encoding::Utf8 => "surrogates not allowed",
        Encoding::Ascii => "ordinal not in range(128)",
    };
    EncodeFailure {
        encoding: encoding.name(),
        start: first,
        end: last + 1,
        reason: reason.to_string(),
        message: encode_error_message(encoding, points, first, last),
    }
}

/// The `UnicodeEncodeError` text of an unencodable run `[first, last]` of `points` under `encoding`.
pub fn encode_error_message(
    encoding: Encoding,
    points: &[u32],
    first: usize,
    last: usize,
) -> String {
    let why = match encoding {
        Encoding::Utf8 => "surrogates not allowed",
        Encoding::Ascii => "ordinal not in range(128)",
    };
    if first == last {
        let c = points[first];
        let shown = if c < 0x100 {
            format!("\\x{c:02x}")
        } else if c < 0x10000 {
            format!("\\u{c:04x}")
        } else {
            format!("\\U{c:08x}")
        };
        format!(
            "'{}' codec can't encode character '{shown}' in position {first}: {why}",
            encoding.name()
        )
    } else {
        format!(
            "'{}' codec can't encode characters in position {first}-{last}: {why}",
            encoding.name()
        )
    }
}

/// `os.fsencode(text)`: the bytes, or the `UnicodeEncodeError` text (`surrogateescape` handler).
pub fn fs_encode(text: &PyStr) -> Result<Vec<u8>, EncodeFailure> {
    let points = text.code_points();
    let encoding = current().filesystem;
    let bad = |c: u32| match encoding {
        Encoding::Ascii => c >= 128 && !(0xDC80..0xDD00).contains(&c),
        Encoding::Utf8 => (0xD800..0xE000).contains(&c) && !(0xDC80..0xDD00).contains(&c),
    };
    if let Some(first) = points.iter().position(|c| bad(*c)) {
        let mut last = first;
        while last + 1 < points.len() && bad(points[last + 1]) {
            last += 1;
        }
        return Err(encode_failure(encoding, points, first, last));
    }
    Ok(match encoding {
        Encoding::Utf8 => text.to_surrogateescape_bytes().map_err(|_| EncodeFailure {
            encoding: "utf-8",
            start: 0,
            end: 0,
            reason: "cannot encode".to_string(),
            message: "cannot encode".to_string(),
        })?,
        Encoding::Ascii => points
            .iter()
            .map(|c| {
                if *c < 128 {
                    *c as u8
                } else {
                    (*c - 0xDC00) as u8
                }
            })
            .collect(),
    })
}

/// The parts of a `UnicodeDecodeError` (its constructor arguments besides the object) and its `str()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeFailure {
    pub encoding: &'static str,
    pub start: usize,
    pub end: usize,
    pub reason: String,
    pub message: String,
}

/// [`decode_preferred`] with the structured failure.
pub fn decode_preferred_detail(bytes: &[u8]) -> Result<String, DecodeFailure> {
    match current().preferred {
        Encoding::Utf8 => match std::str::from_utf8(bytes) {
            Ok(text) => Ok(text.to_string()),
            Err(error) => {
                let (start, end, reason) = agent_runtime_support::pyjson::utf8_error_parts(bytes, &error);
                Err(DecodeFailure { encoding: "utf-8", start, end, reason: reason.to_string(), message: agent_runtime_support::pyjson::utf8_error_text(bytes, &error) })
            }
        },
        Encoding::Ascii => match bytes.iter().position(|b| *b >= 0x80) {
            None => Ok(String::from_utf8_lossy(bytes).to_string()),
            Some(at) => Err(DecodeFailure {
                encoding: "ascii",
                start: at,
                end: at + 1,
                reason: "ordinal not in range(128)".to_string(),
                message: format!("'ascii' codec can't decode byte 0x{:02x} in position {at}: ordinal not in range(128)", bytes[at]),
            }),
        },
    }
}

/// Strict decode of a child's output with the preferred encoding: the text, or the `UnicodeDecodeError` text.
pub fn decode_preferred(bytes: &[u8]) -> Result<String, String> {
    match current().preferred {
        Encoding::Utf8 => match std::str::from_utf8(bytes) {
            Ok(text) => Ok(text.to_string()),
            Err(error) => Err(agent_runtime_support::pyjson::utf8_error_text(bytes, &error)),
        },
        Encoding::Ascii => match bytes.iter().position(|b| *b >= 0x80) {
            None => Ok(String::from_utf8_lossy(bytes).to_string()),
            Some(at) => Err(format!("'ascii' codec can't decode byte 0x{:02x} in position {at}: ordinal not in range(128)", bytes[at])),
        },
    }
}

/// The encoder of stdout (`errors` as the profile says) or stderr (`backslashreplace`): the bytes, or the crash text.
pub fn encode_stream(
    out: &PyStr,
    encoding: Encoding,
    errors: Errors,
) -> Result<Vec<u8>, EncodeFailure> {
    let points = out.code_points();
    let bad = |c: u32| match encoding {
        Encoding::Ascii => c >= 128,
        Encoding::Utf8 => (0xD800..0xE000).contains(&c),
    };
    let escape = |c: u32| -> Option<u8> {
        if (0xDC80..0xDD00).contains(&c) {
            Some((c - 0xDC00) as u8)
        } else {
            None
        }
    };
    let mut bytes = Vec::with_capacity(points.len());
    let mut index = 0;
    while index < points.len() {
        let c = points[index];
        if !bad(c) {
            match encoding {
                Encoding::Ascii => bytes.push(c as u8),
                Encoding::Utf8 => {
                    let mut buf = [0u8; 4];
                    bytes.extend_from_slice(
                        char::from_u32(c)
                            .expect("scalar")
                            .encode_utf8(&mut buf)
                            .as_bytes(),
                    );
                }
            }
            index += 1;
            continue;
        }
        match errors {
            Errors::SurrogateEscape if escape(c).is_some() => {
                bytes.push(escape(c).expect("checked"))
            }
            Errors::BackslashReplace => {
                let shown = if c < 0x100 {
                    format!("\\x{c:02x}")
                } else if c < 0x10000 {
                    format!("\\u{c:04x}")
                } else {
                    format!("\\U{c:08x}")
                };
                bytes.extend_from_slice(shown.as_bytes());
            }
            _ => {
                let mut last = index;
                while last + 1 < points.len()
                    && bad(points[last + 1])
                    && !(errors == Errors::SurrogateEscape && escape(points[last + 1]).is_some())
                {
                    last += 1;
                }
                return Err(encode_failure(encoding, points, index, last));
            }
        }
        index += 1;
    }
    Ok(bytes)
}
