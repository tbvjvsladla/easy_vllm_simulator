//! Uncaught exceptions of the original programs. CPython prints a traceback and exits 1; the port prints the same header
//! and the same final `Class: message` line (the Python source frames in between are not reproduced — documented residual).

use agent_runtime_support::pyyaml::YamlError;

/// An exception the original lets escape: its final traceback line (`Class: message`).
#[derive(Debug, Clone, PartialEq)]
pub struct Raised(pub String);

impl Raised {
    pub fn new(class: &str, message: &str) -> Self {
        Raised(format!("{class}: {message}"))
    }
    pub fn os(e: &std::io::Error, path: &str) -> Self {
        Raised(crate::pathfmt::os_error(e, path))
    }
    pub fn yaml(e: &YamlError) -> Self {
        let module = match e.class {
            "ScannerError" => "yaml.scanner.",
            "ParserError" => "yaml.parser.",
            "ComposerError" => "yaml.composer.",
            "ConstructorError" => "yaml.constructor.",
            "ReaderError" => "yaml.reader.",
            _ => "",
        };
        Raised(format!(
            "{module}{}: {}",
            e.class,
            crate::py::text(&e.message)
        ))
    }
}

/// Print the traceback of `r` on standard error; returns exit status 1.
pub fn exit(r: &Raised) -> i32 {
    eprintln!("Traceback (most recent call last):\n  (Python source frames are not reproduced by the Rust port)\n{}", r.0);
    1
}
