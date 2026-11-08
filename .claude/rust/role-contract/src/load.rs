//! `load_yaml(path)`: `yaml.safe_load(open(path, "r", encoding="utf-8"))` over the common YAML loader ([`agent_runtime_support::pyyaml`], a port of the
//! PyYAML `SafeLoader` pipeline that reads the file like a text file: UTF-8, universal newlines, 8192-byte chunks).
//!
//! Failures are returned as data ([`LoadFailure`]) so a shell can build the Python exception it needs; nothing is turned into a string here. The
//! original's `import yaml` fallback (a `ModuleNotFoundError` retried with the active venv's `site-packages`) belongs to Python's import system; a
//! native process always has its loader, so that branch does not exist here and is not emulated.

use crate::{FrameRoot, SourceFrame, OWNER_FILE};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use agent_runtime_support::pyyaml::{safe_load, YamlError};

#[derive(Debug)]
pub enum LoadFailure {
    /// `open()` of a value that is not a path: `expected str, bytes or os.PathLike object, not 'T'` (the `T` is the type name).
    NotPathLike(&'static str),
    /// `ValueError: embedded null byte`.
    EmbeddedNull,
    /// An operating-system failure of `open`, with the path text Python names in the error.
    Os {
        error: std::io::Error,
        filename: PyStr,
    },
    /// A `yaml` or decoding error the loader raised.
    Yaml(Box<YamlError>),
    /// A situation outside the named profile.
    Unsupported {
        boundary: &'static str,
        detail: String,
    },
}

impl LoadFailure {
    /// The logical traceback of the original owner and the YAML package for this failure (outermost first): `load_yaml` at the `open` line (105) or
    /// at the `yaml.safe_load` line (106) followed by the loader's own mapped frames. `None` when the loader has no mapped chain for the failure
    /// (a scanner or constructor error, a late reader chunk): the chain is then not claimed.
    pub fn frames(&self) -> Option<Vec<SourceFrame>> {
        let owner = |line| SourceFrame {
            root: FrameRoot::Owner,
            file: OWNER_FILE,
            line,
            function: "load_yaml",
        };
        match self {
            LoadFailure::NotPathLike(_) | LoadFailure::EmbeddedNull | LoadFailure::Os { .. } => {
                Some(vec![owner(105)])
            }
            LoadFailure::Unsupported { .. } => None,
            LoadFailure::Yaml(error) => {
                let mut frames = vec![owner(106)];
                for site in error.frames()? {
                    frames.push(SourceFrame {
                        root: if site.file.starts_with("yaml/") {
                            FrameRoot::Yaml
                        } else {
                            FrameRoot::Frozen
                        },
                        file: site.file,
                        line: site.line,
                        function: site.function,
                    });
                }
                Some(frames)
            }
        }
    }
}

/// `load_yaml(path)`.
pub fn load_yaml(path: &PyValue) -> Result<PyValue, LoadFailure> {
    let name = match path.peel() {
        PyValue::Str(text) => text.clone(),
        PyValue::Bool(_) | PyValue::Int(_) => {
            return Err(LoadFailure::Unsupported {
                boundary: "open_file_descriptor",
                detail: "an integer path is a file descriptor for open(); not modelled".to_string(),
            })
        }
        PyValue::Bytes(_) => {
            return Err(LoadFailure::Unsupported {
                boundary: "open_bytes_path",
                detail: "a bytes path names its file in bytes in the errors; not modelled"
                    .to_string(),
            })
        }
        other => return Err(LoadFailure::NotPathLike(other.type_name())),
    };
    let bytes = name
        .to_surrogateescape_bytes()
        .map_err(|_| LoadFailure::Unsupported {
            boundary: "path_encoding",
            detail: "the path cannot be encoded for the file system".to_string(),
        })?;
    if bytes.contains(&0) {
        return Err(LoadFailure::EmbeddedNull);
    }
    use std::os::unix::ffi::OsStringExt;
    let os_path = std::path::PathBuf::from(std::ffi::OsString::from_vec(bytes));
    let failed = |error: std::io::Error| LoadFailure::Os {
        error,
        filename: name.clone(),
    };
    let mut file = std::fs::File::open(&os_path).map_err(failed)?;
    // `open()` refuses a directory at the call (EISDIR); `File::open` would succeed and fail at the first read
    if file.metadata().map(|m| m.is_dir()).unwrap_or(false) {
        return Err(failed(std::io::Error::from_raw_os_error(21)));
    }
    let mut content = Vec::new();
    {
        use std::io::Read;
        file.read_to_end(&mut content).map_err(failed)?;
    }
    safe_load(&content, &name).map_err(|error| LoadFailure::Yaml(Box::new(error)))
}
