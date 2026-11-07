//! Small constructors for the Python exceptions the `regen_requirements` owner raises or lets through, in the public shape of the saved originals
//! ([`PyException`]: class, module, `str()`, `args`, captured attributes, link), and the filename-aware mapping of an operating-system error to the
//! `OSError` subclass CPython picks for its `errno`.
//!
//! The errno table is the one of `OSError.__new__` on Linux (`errnomap`): the subclass depends on the number only, `strerror` is the C library's text,
//! and the file name travels as the `filename` attribute (it is not an element of `args`), as the saved `FileNotFoundError`/`IsADirectoryError` show.

use crate::http_direct::failure::ErrorValue;
use crate::http_direct::{Link, PyException};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;

/// An exception of `module` raised with only a message: `args == (message,)`, no captured attributes.
pub fn with_message(class: &'static str, module: &'static str, message: &str) -> PyException {
    PyException {
        class,
        module,
        message: PyStr::new(message),
        args: vec![ErrorValue::Plain(PyValue::str(message))],
        attributes: Vec::new(),
        http_headers: None,
        link: None,
    }
}

/// A builtin exception raised with only a message.
pub fn builtin(class: &'static str, message: &str) -> PyException {
    with_message(class, "builtins", message)
}

/// `AttributeError("'T' object has no attribute 'name'")` with its `name` and `obj` attributes.
pub fn attribute_error(owner: &PyValue, name: &str) -> PyException {
    let mut error = builtin(
        "AttributeError",
        &format!(
            "'{}' object has no attribute '{name}'",
            owner.peel().type_name()
        ),
    );
    error.attributes = vec![
        ("name", PyValue::str(name).into()),
        ("obj", ErrorValue::Plain(owner.clone())),
    ];
    error
}

/// `TypeError("argument of type 'T' is not iterable")`: `"x" in value` for a value without `__contains__` or `__iter__`.
pub fn not_a_container(value: &PyValue) -> PyException {
    builtin(
        "TypeError",
        &format!(
            "argument of type '{}' is not iterable",
            value.peel().type_name()
        ),
    )
}

/// `TypeError("'T' object is not iterable")`: `for x in value`.
pub fn not_iterable(value: &PyValue) -> PyException {
    builtin(
        "TypeError",
        &format!("'{}' object is not iterable", value.peel().type_name()),
    )
}

/// The `OSError` subclass and spelling CPython gives an operating-system error number (Linux).
fn os_class(errno: i32) -> &'static str {
    match errno {
        1 | 13 => "PermissionError",
        2 => "FileNotFoundError",
        3 => "ProcessLookupError",
        4 => "InterruptedError",
        10 => "ChildProcessError",
        11 | 114 | 115 => "BlockingIOError",
        17 => "FileExistsError",
        20 => "NotADirectoryError",
        21 => "IsADirectoryError",
        32 | 108 => "BrokenPipeError",
        103 => "ConnectionAbortedError",
        104 => "ConnectionResetError",
        110 => "TimeoutError",
        111 => "ConnectionRefusedError",
        _ => "OSError",
    }
}

/// The C library's text for `errno` (`strerror`), as `std` reports it without its own ` (os error N)` suffix.
pub fn strerror(errno: i32) -> String {
    let shown = std::io::Error::from_raw_os_error(errno).to_string();
    shown
        .split(" (os error")
        .next()
        .unwrap_or(&shown)
        .to_string()
}

/// The `OSError` of an operating-system failure on the file `filename`: subclass by errno, `args == (errno, strerror)`, `filename` attribute, message
/// `[Errno N] strerror: 'filename'`. `None` when the failure carries no errno (this component then refuses by name instead of inventing one).
pub fn os_error_with_filename(error: &std::io::Error, filename: &PyStr) -> Option<PyException> {
    let errno = error.raw_os_error()?;
    let text = strerror(errno);
    let message = format!("[Errno {errno}] {text}: {}", filename.repr());
    Some(PyException {
        class: os_class(errno),
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![PyValue::int(errno).into(), PyValue::str(&text).into()],
        attributes: vec![
            ("errno", PyValue::int(errno).into()),
            ("strerror", PyValue::str(&text).into()),
            ("filename", PyValue::Str(filename.clone()).into()),
            ("filename2", PyValue::None.into()),
        ],
        http_headers: None,
        link: None,
    })
}

/// The same exception for a failure that names no file (a stream or a directory handle): no `filename`, message `[Errno N] strerror`.
pub fn os_error_without_filename(error: &std::io::Error) -> Option<PyException> {
    let errno = error.raw_os_error()?;
    let text = strerror(errno);
    let message = format!("[Errno {errno}] {text}");
    Some(PyException {
        class: os_class(errno),
        module: "builtins",
        message: PyStr::new(&message),
        args: vec![PyValue::int(errno).into(), PyValue::str(&text).into()],
        attributes: vec![
            ("errno", PyValue::int(errno).into()),
            ("strerror", PyValue::str(&text).into()),
            ("filename", PyValue::None.into()),
            ("filename2", PyValue::None.into()),
        ],
        http_headers: None,
        link: None,
    })
}

/// `exception` raised while `context` was being handled (its `__context__`).
pub fn during(exception: PyException, context: PyException) -> PyException {
    PyException {
        link: Some(Link::Context(Box::new(context))),
        ..exception
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_subclass_follows_the_errno_and_the_file_name_is_an_attribute_not_an_argument() {
        let missing = std::io::Error::from_raw_os_error(2);
        let error = os_error_with_filename(&missing, &PyStr::new("a/b.txt")).unwrap();
        assert_eq!(error.class, "FileNotFoundError");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "[Errno 2] No such file or directory: 'a/b.txt'"
        );
        assert_eq!(error.args.len(), 2);
        let directory = std::io::Error::from_raw_os_error(21);
        assert_eq!(
            os_error_with_filename(&directory, &PyStr::new("d"))
                .unwrap()
                .class,
            "IsADirectoryError"
        );
        let denied = std::io::Error::from_raw_os_error(13);
        assert_eq!(
            os_error_with_filename(&denied, &PyStr::new("d"))
                .unwrap()
                .class,
            "PermissionError"
        );
        let loop_ = std::io::Error::from_raw_os_error(40);
        assert_eq!(
            os_error_with_filename(&loop_, &PyStr::new("d"))
                .unwrap()
                .class,
            "OSError"
        );
        assert!(
            os_error_with_filename(&std::io::Error::other("no errno"), &PyStr::new("d")).is_none()
        );
    }

    #[test]
    fn container_and_iteration_errors_name_the_python_type() {
        assert_eq!(
            not_a_container(&PyValue::int(3)).message.to_utf8().unwrap(),
            "argument of type 'int' is not iterable"
        );
        assert_eq!(
            not_iterable(&PyValue::None).message.to_utf8().unwrap(),
            "'NoneType' object is not iterable"
        );
        let error = attribute_error(&PyValue::List(Vec::new()), "split");
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "'list' object has no attribute 'split'"
        );
        assert_eq!(error.attributes.len(), 2);
    }
}
