//! The report of the one exception `validate_runtime_patch.py` lets escape its handler with a source location the saved captures show:
//! `RuntimeError("Symlink loop from ...")` raised by `Path.resolve()` while handling the `ELOOP` of its own `stat` (cases B043 and B044), as CPython
//! 3.12.3 prints the two chained blocks.
//!
//! The frames (file, line, function, source text) are those of the raw tracebacks of the saved captures, the pinned 107-line script and the reference
//! interpreter's `pathlib.py`; the caret span of each frame is found in its source text from the expression the interpreter points at (the call
//! that was executing), never counted by hand. The block layout is the common [`crate::torch_traceback::render`].

use crate::torch_traceback::{render, Site, SiteFile};
use agent_runtime_support::pystr::PyStr;

const PATHLIB: SiteFile = SiteFile::Stdlib("pathlib.py");

/// Which of the two `resolve()` calls of `_inside` met the loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResolveCall {
    Patch,
    Parent,
}

/// Which action's `_context(args)` call the exception passed through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Stamp,
    Verify,
}

/// A frame whose span is the expression `needle` in `text` (`None`: the interpreter prints no caret line, the span being the whole line).
fn frame(
    file: SiteFile,
    line: u32,
    function: &'static str,
    text: &'static str,
    needle: Option<&str>,
) -> Site {
    let columns = needle.map(|needle| {
        let start = text
            .find(needle)
            .unwrap_or_else(|| panic!("the source line {line} holds {needle:?}"));
        (start as u32, (start + needle.len()) as u32)
    });
    Site {
        file,
        line,
        function,
        text,
        columns,
    }
}

/// The text from the first character of `needle` to the end of the line, for a span that runs to it.
fn tail_from<'a>(text: &'a str, needle: &str) -> &'a str {
    &text[text
        .find(needle)
        .unwrap_or_else(|| panic!("{text:?} holds {needle:?}"))..]
}

const MODULE_LINE: &str = "    raise SystemExit(main())";
const MAIN_LINE: &str = "        return stamp(args) if args.action == \"stamp\" else verify(args)";
const STAMP_LINE: &str = "    _, sidecar, record = _context(args)";
const VERIFY_LINE: &str = "    _, sidecar, expected = _context(args)";
const CONTEXT_LINE: &str = "    patch = _inside(Path(args.patch), Path(args.config_dir))";
const INSIDE_LINE: &str = "    resolved, root = path.resolve(), parent.resolve()";

/// The frames of the script, innermost first.
fn script_frames(action: Action, call: ResolveCall) -> Vec<Site> {
    let (action_needle, action_line, context_line, context_call) = match action {
        Action::Stamp => ("stamp(args)", 65, STAMP_LINE, "stamp"),
        Action::Verify => ("verify(args)", 81, VERIFY_LINE, "verify"),
    };
    let resolve_needle = match call {
        ResolveCall::Patch => "path.resolve()",
        ResolveCall::Parent => "parent.resolve()",
    };
    vec![
        frame(
            SiteFile::Script,
            20,
            "_inside",
            INSIDE_LINE,
            Some(resolve_needle),
        ),
        frame(
            SiteFile::Script,
            29,
            "_context",
            CONTEXT_LINE,
            Some(tail_from(CONTEXT_LINE, "_inside(")),
        ),
        frame(
            SiteFile::Script,
            action_line,
            context_call,
            context_line,
            Some(tail_from(context_line, "_context(")),
        ),
        frame(
            SiteFile::Script,
            101,
            "main",
            MAIN_LINE,
            Some(action_needle),
        ),
        frame(
            SiteFile::Script,
            107,
            "<module>",
            MODULE_LINE,
            Some("main()"),
        ),
    ]
}

/// The whole report of a symlink loop met by `Path.resolve()`: the `OSError` of `stat` first, then the `RuntimeError` raised while it was handled.
pub fn symlink_loop_report(
    script: &PyStr,
    action: Action,
    call: ResolveCall,
    filename: &PyStr,
) -> PyStr {
    let shown = filename.repr();
    let stat_frames = [
        frame(
            PATHLIB,
            842,
            "stat",
            "        return os.stat(self, follow_symlinks=follow_symlinks)",
            Some("os.stat(self, follow_symlinks=follow_symlinks)"),
        ),
        frame(PATHLIB, 1252, "resolve", "                p.stat()", None),
    ];
    let mut handled = vec![
        frame(
            PATHLIB,
            1239,
            "check_eloop",
            "                raise RuntimeError(\"Symlink loop from %r\" % e.filename)",
            None,
        ),
        frame(
            PATHLIB,
            1254,
            "resolve",
            "                check_eloop(e)",
            None,
        ),
    ];
    handled.extend(script_frames(action, call));
    let mut report = render(
        &stat_frames,
        script,
        &PyStr::new(&format!(
            "OSError: [Errno 40] Too many levels of symbolic links: {shown}\n"
        )),
    );
    report.push_str("\nDuring handling of the above exception, another exception occurred:\n\n");
    report.push_py(&render(
        &handled,
        script,
        &PyStr::new(&format!("RuntimeError: Symlink loop from {shown}\n")),
    ));
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(report: &PyStr) -> Vec<String> {
        report
            .to_utf8()
            .expect("text")
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn a_patch_loop_points_at_the_first_resolve_and_a_config_loop_at_the_second() {
        let script = PyStr::new("/x/validate_runtime_patch.py");
        let name = PyStr::new("/r/loop");
        let patch = lines(&symlink_loop_report(
            &script,
            Action::Verify,
            ResolveCall::Patch,
            &name,
        ));
        let config = lines(&symlink_loop_report(
            &script,
            Action::Verify,
            ResolveCall::Parent,
            &name,
        ));
        let at = |text: &[String]| {
            let line = text
                .iter()
                .position(|l| l.contains("path.resolve(), parent"))
                .unwrap();
            let carets = &text[line + 1];
            (carets.find('^').unwrap(), carets.matches('^').count())
        };
        // `    resolved, root = ` is 21 columns of source after the four of the display
        assert_eq!(at(&patch), (4 + 17, "path.resolve()".len()));
        assert_eq!(at(&config), (4 + 33, "parent.resolve()".len()));
        assert_eq!(
            patch.last().unwrap(),
            "RuntimeError: Symlink loop from '/r/loop'"
        );
    }

    #[test]
    fn the_chain_is_two_tracebacks_joined_by_the_context_sentence() {
        let report = symlink_loop_report(
            &PyStr::new("/x/validate_runtime_patch.py"),
            Action::Stamp,
            ResolveCall::Patch,
            &PyStr::new("/r/loop"),
        )
        .to_utf8()
        .expect("text");
        assert_eq!(
            report.matches("Traceback (most recent call last):").count(),
            2
        );
        assert!(report.contains(
            "'/r/loop'\n\nDuring handling of the above exception, another exception occurred:\n\nTraceback"
        ));
        assert!(report.contains("in stamp\n    _, sidecar, record = _context(args)\n"));
    }
}
