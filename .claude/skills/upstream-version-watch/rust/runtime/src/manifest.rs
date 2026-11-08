//! `load_manifest(path)` and `_load_yaml_flat(path)`: the manifest as PyYAML reads it, or, where the original's `import yaml` raises
//! `ImportError`, as the flat mini parser reads it. The import is asked of the host at the very point the original makes it.

use crate::business::strip_inline_comment;
use crate::host::Host;
use crate::textfile::TextStream;
use crate::thrown::{encode_thrown, Ctor, Done, Thrown};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use agent_runtime_support::pyyaml::{safe_load_observed, ReadEvent, YamlDetail, YamlError};
use recipe_runtime::escalation::Raised;
use recipe_runtime::memory::open_text_file;
use recipe_runtime::model_files::{fspath, os_failure};
use std::io::Read;

/// What a call runs on, as the manifest code sees it.
pub struct World<'a> {
    pub host: &'a mut dyn Host,
    /// `sys.argv`, the program name first.
    pub argv: &'a [PyStr],
    /// The terminal width argparse formats for.
    pub columns: Option<&'a str>,
}

impl World<'_> {
    pub fn note(&mut self, name: &str, fields: Vec<(&str, PyValue)>) {
        if self.host.tracing() {
            self.host.trace(name, fields);
        }
    }

    fn event(&mut self, name: &str, path: &PyStr, extra: Vec<(&str, PyValue)>) {
        if self.host.tracing() {
            let mut fields = vec![("path", PyValue::Str(path.clone()))];
            fields.extend(extra);
            self.host.trace(name, fields);
        }
    }
}

/// `os.path.isfile(path)`: a regular file after following links; any error (an unencodable path, a NUL, a missing file) is `False`.
pub fn is_file(world: &mut World<'_>, path: &PyStr) -> bool {
    let found = path
        .to_surrogateescape_bytes()
        .ok()
        .filter(|bytes| !bytes.contains(&0))
        .and_then(|bytes| {
            use std::os::unix::ffi::OsStrExt;
            std::fs::metadata(std::ffi::OsStr::from_bytes(&bytes)).ok()
        })
        .is_some_and(|metadata| metadata.is_file());
    world.event("isfile", path, vec![("result", PyValue::Bool(found))]);
    found
}

fn failure_thrown(failure: agent_runtime_support::failure::Failure) -> Box<Thrown> {
    Thrown::plain(Raised::from(failure))
}

/// `open(path, encoding="utf-8")` and the whole content of the file, told as the collector sees it (`file_open`, `file_open_error`).
fn open_file(world: &mut World<'_>, path: &PyValue) -> Done<(PyStr, Vec<u8>)> {
    let name = fspath(path, false).map_err(failure_thrown)?.clone();
    let mut keywords = PyDict::new();
    keywords.set("encoding", PyValue::str("utf-8"));
    world.event(
        "file_open",
        &name,
        vec![
            ("args", PyValue::List(Vec::new())),
            ("kwargs", PyValue::Dict(keywords)),
        ],
    );
    let opened = match name.to_surrogateescape_bytes() {
        Err(failure) => Err(encode_thrown(&name, &failure)),
        Ok(_) => open_text_file(&name).map_err(failure_thrown),
    };
    match opened {
        Ok(mut file) => {
            let mut bytes = Vec::new();
            match file.read_to_end(&mut bytes) {
                Ok(_) => Ok((name, bytes)),
                Err(error) => Err(failure_thrown(os_failure(&error, None))),
            }
        }
        Err(thrown) => {
            world.event(
                "file_open_error",
                &name,
                vec![
                    ("type", PyValue::str(&thrown.raised.class)),
                    ("message", PyValue::Str(thrown.raised.message.clone())),
                ],
            );
            Err(thrown)
        }
    }
}

fn module_of(class: &str) -> &'static str {
    match class {
        "ReaderError" => "yaml.reader",
        "ScannerError" => "yaml.scanner",
        "ParserError" => "yaml.parser",
        "ComposerError" => "yaml.composer",
        "ConstructorError" => "yaml.constructor",
        _ => "builtins",
    }
}

fn yaml_thrown(error: YamlError) -> Box<Thrown> {
    let ctor = match error.detail {
        // the marked-error constructor state is public data of the generic parser; this loader reports the class and text only
        YamlDetail::None | YamlDetail::Marked(_) => None,
        YamlDetail::Reader {
            name,
            position,
            character,
            encoding,
            reason,
        } => Some(Ctor::Reader {
            name,
            position,
            character,
            encoding: encoding.to_string(),
            reason: reason.to_string(),
        }),
        YamlDetail::Decode {
            encoding,
            object,
            start,
            end,
            reason,
            ..
        } => Some(Ctor::Decode {
            encoding: encoding.to_string(),
            object,
            start,
            end,
            reason: reason.to_string(),
        }),
    };
    Box::new(Thrown {
        raised: Raised::new(error.class, module_of(error.class), error.message),
        ctors: vec![ctor],
        bases: None,
    })
}

/// `with open(path, encoding="utf-8") as f: return yaml.safe_load(f) or {}`.
fn yaml_branch(world: &mut World<'_>, path: &PyValue) -> Done<PyValue> {
    let (name, bytes) = open_file(world, path)?;
    let (result, reads) = safe_load_observed(&bytes, &name);
    for read in reads {
        match read {
            ReadEvent::Read { size } => world.event(
                "file_read",
                &name,
                vec![
                    ("args", PyValue::List(vec![PyValue::int(size)])),
                    ("kwargs", PyValue::Dict(PyDict::new())),
                ],
            ),
            ReadEvent::Return { text } => world.event(
                "file_read_return",
                &name,
                vec![("text", PyValue::Str(text))],
            ),
            ReadEvent::Error { class, message } => world.event(
                "file_read_error",
                &name,
                vec![
                    ("type", PyValue::str(class)),
                    ("message", PyValue::Str(message)),
                ],
            ),
        }
    }
    world.event("file_close", &name, Vec::new());
    let value = result.map_err(yaml_thrown)?;
    Ok(if value.is_truthy() {
        value
    } else {
        PyValue::Dict(PyDict::new())
    })
}

/// `load_manifest(path)`: PyYAML when `import yaml` succeeds, the flat parser when it raises `ImportError` (an exception raised there has the
/// import error as its context), `{}` for a missing file; every other exception is the original's.
pub fn load_manifest(world: &mut World<'_>, path: &PyValue) -> Done<PyValue> {
    match world.host.import_yaml() {
        Ok(()) => match yaml_branch(world, path) {
            Err(thrown) if thrown.is_a("FileNotFoundError") => Ok(PyValue::Dict(PyDict::new())),
            other => other,
        },
        Err(imported) if imported.is_a("ImportError") => {
            load_yaml_flat(world, path).map_err(|thrown| thrown.with_context(&imported))
        }
        Err(imported) if imported.is_a("FileNotFoundError") => Ok(PyValue::Dict(PyDict::new())),
        Err(imported) => Err(imported),
    }
}

/// `_load_yaml_flat(path)`: the first colon of each top-level line splits key and value; indented, `-` and comment lines are skipped.
pub fn load_yaml_flat(world: &mut World<'_>, path: &PyValue) -> Done<PyValue> {
    let (name, bytes) = match open_file(world, path) {
        Ok(opened) => opened,
        Err(thrown) if thrown.is_a("FileNotFoundError") => return Ok(PyValue::Dict(PyDict::new())),
        Err(thrown) => return Err(thrown),
    };
    let outcome = flat_lines(world, &name, bytes);
    world.event("file_close", &name, Vec::new());
    outcome
}

fn flat_lines(world: &mut World<'_>, name: &PyStr, bytes: Vec<u8>) -> Done<PyValue> {
    let mut out = PyDict::new();
    let mut stream = TextStream::new(bytes);
    loop {
        let line = match stream.next_line() {
            Ok(Some(line)) => line,
            Ok(None) => {
                world.event("file_iter_end", name, Vec::new());
                return Ok(PyValue::Dict(out));
            }
            Err(failure) => {
                let message = failure.message();
                world.event(
                    "file_iter_error",
                    name,
                    vec![
                        ("type", PyValue::str("UnicodeDecodeError")),
                        ("message", PyValue::str(&message)),
                    ],
                );
                return Err(Box::new(Thrown {
                    raised: Raised::plain("UnicodeDecodeError", &message),
                    ctors: vec![Some(Ctor::Decode {
                        encoding: "utf-8".to_string(),
                        object: failure.object,
                        start: failure.start,
                        end: failure.end,
                        reason: failure.reason.to_string(),
                    })],
                    bases: None,
                }));
            }
        };
        world.event("file_iter_line", name, vec![("text", PyValue::str(&line))]);
        flat_line(&mut out, &line)?;
    }
}

/// One iteration of the flat parser's loop body.
fn flat_line(out: &mut PyDict, line: &str) -> Done<()> {
    let s = PyStr::new(line.trim_end_matches('\n'));
    if s.strip().is_empty() {
        return Ok(());
    }
    let points = s.code_points();
    let leading = points
        .iter()
        .position(|&c| !agent_runtime_support::pystr::is_python_space(c))
        .unwrap_or(points.len());
    // `s.lstrip().startswith("#")`
    if points.get(leading) == Some(&u32::from('#')) {
        return Ok(());
    }
    // `s[0] in (" ", "\t", "-")`
    if matches!(points[0], 0x20 | 0x09 | 0x2D) {
        return Ok(());
    }
    let Some(colon) = points.iter().position(|&c| c == u32::from(':')) else {
        return Ok(());
    };
    let key = PyStr::from_code_points(points[..colon].to_vec()).strip();
    let rest = PyValue::Str(PyStr::from_code_points(points[colon + 1..].to_vec()));
    let PyValue::Str(value) = strip_inline_comment(&rest)? else {
        unreachable!("a str stays a str")
    };
    let value = value.strip();
    if value.equals("[]") {
        out.insert(key, PyValue::List(Vec::new()));
        return Ok(());
    }
    if value.is_empty() {
        out.insert(key, PyValue::str(""));
        return Ok(());
    }
    let chars = value.code_points();
    let (first, last) = (chars[0], chars[chars.len() - 1]);
    let value = if first == last && (first == u32::from('\'') || first == u32::from('"')) {
        // `val[1:-1]`: empty for a single quote character
        PyStr::from_code_points(chars[1..chars.len().saturating_sub(1).max(1)].to_vec())
    } else {
        value
    };
    out.insert(key, PyValue::Str(value));
    Ok(())
}
