//! stdout/stderr of the ported modules. Ported code never calls `println!`/`eprintln!` directly: it writes through
//! [`out`]/[`err`] (Python `print(..., file=sys.stdout|sys.stderr)`), so that the library bridge (`hint_api`) can capture
//! what a function printed exactly like the Python capture shim does (`contextlib.redirect_stdout/stderr`).
use std::cell::RefCell;
use std::io::Write;

thread_local! {
    static CAPTURE: RefCell<Vec<(String, String)>> = const { RefCell::new(Vec::new()) };
}

/// Write text to stdout (no newline added).
pub fn out(text: &str) {
    let captured = CAPTURE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(top) = c.last_mut() {
            top.0.push_str(text);
            true
        } else {
            false
        }
    });
    if !captured {
        let mut so = std::io::stdout().lock();
        let _ = so.write_all(text.as_bytes());
    }
}

/// Write text to stderr (no newline added).
pub fn err(text: &str) {
    let captured = CAPTURE.with(|c| {
        let mut c = c.borrow_mut();
        if let Some(top) = c.last_mut() {
            top.1.push_str(text);
            true
        } else {
            false
        }
    });
    if !captured {
        let _ = std::io::stdout().flush();
        let mut se = std::io::stderr().lock();
        let _ = se.write_all(text.as_bytes());
    }
}

/// `print(text)`.
pub fn outln(text: &str) {
    out(text);
    out("\n");
}

/// `print(text, file=sys.stderr)`.
pub fn errln(text: &str) {
    err(text);
    err("\n");
}

pub fn flush() {
    let _ = std::io::stdout().flush();
}

/// Run `f` with stdout and stderr captured; returns `(result, stdout, stderr)`.
pub fn capture<T>(f: impl FnOnce() -> T) -> (T, String, String) {
    CAPTURE.with(|c| c.borrow_mut().push((String::new(), String::new())));
    let r = f();
    let (o, e) = CAPTURE.with(|c| c.borrow_mut().pop().unwrap_or_default());
    (r, o, e)
}

/// Run `f` with stderr discarded and stdout passed through (`contextlib.redirect_stderr(io.StringIO())`).
pub fn quiet_stderr<T>(f: impl FnOnce() -> T) -> T {
    let (r, o, _e) = capture(f);
    out(&o);
    r
}
