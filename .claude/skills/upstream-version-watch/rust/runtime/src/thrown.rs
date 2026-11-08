//! An exception of a call with the constructor fields of each row of its chain, and the ancestry the caller reported for it.

use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyDict, PyValue};
use recipe_runtime::escalation::Raised;

/// The fields the classes whose constructor needs more than the message are rebuilt from (the ACTUAL ones: nothing is guessed from a message).
#[derive(Clone, Debug)]
pub enum Ctor {
    /// `UnicodeDecodeError(encoding, object, start, end, reason)`.
    Decode {
        encoding: String,
        object: Vec<u8>,
        start: usize,
        end: usize,
        reason: String,
    },
    /// `UnicodeEncodeError(encoding, object, start, end, reason)`.
    Encode {
        encoding: String,
        object: PyStr,
        start: usize,
        end: usize,
        reason: String,
    },
    /// `json.decoder.JSONDecodeError(msg, doc, pos)`.
    Json { msg: String, doc: PyStr, pos: usize },
    /// `yaml.reader.ReaderError(name, position, character, encoding, reason)`.
    Reader {
        name: PyStr,
        position: usize,
        character: u32,
        encoding: String,
        reason: String,
    },
}

/// An exception with the constructor fields of each row of its chain (the top row first).
#[derive(Clone, Debug)]
pub struct Thrown {
    pub raised: Raised,
    pub ctors: Vec<Option<Ctor>>,
    /// The canonical builtin classes in the MRO of an exception the CALLER reported, in MRO order; `None` for one raised here. It classifies
    /// `except` clauses (argparse's, the loader's) and is not part of the result chain.
    pub bases: Option<Vec<String>>,
}

pub type Done<T> = Result<T, Box<Thrown>>;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

impl Thrown {
    pub fn plain(raised: Raised) -> Box<Thrown> {
        Box::new(Thrown {
            raised,
            ctors: Vec::new(),
            bases: None,
        })
    }

    /// An exception of the builtin module raised here.
    pub fn builtin(class: &str, message: &str) -> Box<Thrown> {
        Thrown::plain(Raised::plain(class, message))
    }

    /// Raised while `context` was being handled: it is the implicit context (`relation: "context"`) unless the exception already has a cause.
    pub fn with_context(mut self: Box<Thrown>, context: &Thrown) -> Box<Thrown> {
        if self.raised.cause.is_none() {
            self.raised.cause = Some(Box::new(context.raised.clone()));
            self.raised.via_context = true;
            self.ctors.resize(1, None);
            self.ctors.extend(context.ctors.iter().cloned());
        }
        self
    }

    /// The chain rows of `raised`, each with its `constructor` (the saved shape, no `kind`) when it has one.
    pub fn chain(&self) -> PyValue {
        let mut chain = self.raised.chain_value();
        if let PyValue::List(rows) = &mut chain {
            for (index, row) in rows.iter_mut().enumerate() {
                let Some(Some(ctor)) = self.ctors.get(index) else {
                    continue;
                };
                let PyValue::Dict(row) = row else { continue };
                let mut fields = PyDict::new();
                match ctor {
                    Ctor::Decode {
                        encoding,
                        object,
                        start,
                        end,
                        reason,
                    } => {
                        fields.set("encoding", PyValue::str(encoding));
                        fields.set("object_hex", PyValue::str(&hex(object)));
                        fields.set("start", PyValue::int(*start));
                        fields.set("end", PyValue::int(*end));
                        fields.set("reason", PyValue::str(reason));
                    }
                    Ctor::Encode {
                        encoding,
                        object,
                        start,
                        end,
                        reason,
                    } => {
                        fields.set("encoding", PyValue::str(encoding));
                        fields.set("object", PyValue::Str(object.clone()));
                        fields.set("start", PyValue::int(*start));
                        fields.set("end", PyValue::int(*end));
                        fields.set("reason", PyValue::str(reason));
                    }
                    Ctor::Json { msg, doc, pos } => {
                        fields.set("msg", PyValue::str(msg));
                        fields.set("doc", PyValue::Str(doc.clone()));
                        fields.set("pos", PyValue::int(*pos));
                    }
                    Ctor::Reader {
                        name,
                        position,
                        character,
                        encoding,
                        reason,
                    } => {
                        fields.set("name", PyValue::Str(name.clone()));
                        fields.set("position", PyValue::int(*position));
                        fields.set("character", PyValue::int(*character));
                        fields.set("encoding", PyValue::str(encoding));
                        fields.set("reason", PyValue::str(reason));
                    }
                }
                row.set("constructor", PyValue::Dict(fields));
            }
        }
        chain
    }

    /// Whether a canonical builtin class is among the ancestors: by the reported descriptor, else (raised here) by the builtin class name.
    pub fn is_a(&self, builtin: &str) -> bool {
        match &self.bases {
            Some(bases) => bases.iter().any(|base| base == builtin),
            None => {
                self.raised.module == "builtins"
                    && (self.raised.class == builtin
                        || (builtin == "ImportError" && self.raised.class == "ModuleNotFoundError")
                        || (builtin == "OSError" && is_os_error_name(&self.raised.class)))
            }
        }
    }
}

fn is_os_error_name(class: &str) -> bool {
    matches!(
        class,
        "OSError"
            | "FileNotFoundError"
            | "PermissionError"
            | "IsADirectoryError"
            | "NotADirectoryError"
            | "FileExistsError"
            | "InterruptedError"
            | "ChildProcessError"
            | "ProcessLookupError"
            | "BrokenPipeError"
            | "BlockingIOError"
            | "ConnectionError"
            | "ConnectionAbortedError"
            | "ConnectionResetError"
            | "ConnectionRefusedError"
            | "TimeoutError"
    )
}

/// The `UnicodeEncodeError` of encoding `object` as UTF-8 (`surrogateescape`): the class and message of the common encoder's failure and
/// its five constructor fields, taken from the failure itself.
pub fn encode_thrown(
    object: &PyStr,
    failure: &agent_runtime_support::pystr::EncodeFailure,
) -> Box<Thrown> {
    Box::new(Thrown {
        raised: Raised::plain("UnicodeEncodeError", &failure.message()),
        ctors: vec![Some(Ctor::Encode {
            encoding: "utf-8".to_string(),
            object: object.clone(),
            start: failure.start,
            end: failure.end,
            reason: "surrogates not allowed".to_string(),
        })],
        bases: None,
    })
}

/// The exception of a failed write to a real stream (`BrokenPipeError`, `OSError`, ...).
pub fn write_thrown(error: &std::io::Error) -> Box<Thrown> {
    Thrown::plain(Raised::from(recipe_runtime::stdio::os_failure(error)))
}
