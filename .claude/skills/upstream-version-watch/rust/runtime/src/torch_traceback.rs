//! The uncaught-exception report of `resolve_torch_pin.py`: CPython 3.12.3's `Traceback (most recent call last):` block for the sites the command
//! can raise through, as COMPATIBILITY frames.
//!
//! The source locations below (line, function, source text, caret span) were read from the seven raw tracebacks of the saved original captures
//! (`main-038/039/040/044/045/047/049`, the pinned 103-line script and the reference interpreter's `json/__init__.py` and `json/encoder.py`); nothing
//! here reads a Python file, runs Python or claims that it ran. An exception that escaped through a site this table does not hold (for example a
//! `ValueError` raised inside `packaging.version`) is reported by its `Class: message` block alone.

use agent_runtime_support::pystr::PyStr;

pub const STDLIB_PREFIX: &str = "/usr/lib/python3.12";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteFile {
    /// The script itself; its path is the caller's `argv[0]`.
    Script,
    /// A file of the reference interpreter's standard library, relative to [`STDLIB_PREFIX`].
    Stdlib(&'static str),
}

/// One frame: where the interpreter says the exception passed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub file: SiteFile,
    pub line: u32,
    pub function: &'static str,
    /// The source line with its indentation.
    pub text: &'static str,
    /// The instruction's span as UTF-8 byte offsets in `text`; `None` when the interpreter prints no caret line.
    pub columns: Option<(u32, u32)>,
}

const JSON_INIT: SiteFile = SiteFile::Stdlib("json/__init__.py");
const JSON_ENCODER: SiteFile = SiteFile::Stdlib("json/encoder.py");

const NO_TORCH_DUMPS_TEXT: &str =
    "        print(json.dumps({\"error\": \"build-system.requires 에 torch 없음\", \"requires\": requires},";

/// `main()` at the bottom of the script.
pub const MODULE_ENTRY: Site = Site {
    file: SiteFile::Script,
    line: 103,
    function: "<module>",
    text: "    main()",
    columns: None,
};
/// `data.get("build-system", {}).get(...)`: the attribute load on a value that is not a table.
pub const MAIN_BUILD_SYSTEM_GET: Site = Site {
    file: SiteFile::Script,
    line: 64,
    function: "main",
    text: "    requires = data.get(\"build-system\", {}).get(\"requires\", [])",
    columns: Some((15, 47)),
};
/// `for r in requires:` over a value that cannot be iterated.
pub const MAIN_REQUIRES_LOOP: Site = Site {
    file: SiteFile::Script,
    line: 66,
    function: "main",
    text: "    for r in requires:",
    columns: None,
};
/// The `json.dumps` of the "no torch" report, the call that spans two lines (the span ends with the first).
pub const MAIN_NO_TORCH_DUMPS: Site = Site {
    file: SiteFile::Script,
    line: 75,
    function: "main",
    text: NO_TORCH_DUMPS_TEXT,
    columns: Some((14, NO_TORCH_DUMPS_TEXT.len() as u32)),
};
pub const JSON_DUMPS_ENCODE: Site = Site {
    file: JSON_INIT,
    line: 238,
    function: "dumps",
    text: "        **kw).encode(obj)",
    columns: Some((14, 25)),
};
pub const JSON_ENCODE_ITERENCODE: Site = Site {
    file: JSON_ENCODER,
    line: 200,
    function: "encode",
    text: "        chunks = self.iterencode(o, _one_shot=True)",
    columns: Some((17, 51)),
};
pub const JSON_ITERENCODE_ONE_SHOT: Site = Site {
    file: JSON_ENCODER,
    line: 258,
    function: "iterencode",
    text: "        return _iterencode(o, 0)",
    columns: Some((15, 32)),
};
pub const JSON_DEFAULT_RAISE: Site = Site {
    file: JSON_ENCODER,
    line: 180,
    function: "default",
    text: "        raise TypeError(f'Object of type {o.__class__.__name__} '",
    columns: None,
};

/// The frames of a value the JSON encoder cannot serialize (`TypeError` from `JSONEncoder.default`), innermost first.
pub fn json_default_frames() -> Vec<Site> {
    vec![
        JSON_DEFAULT_RAISE,
        JSON_ITERENCODE_ONE_SHOT,
        JSON_ENCODE_ITERENCODE,
        JSON_DUMPS_ENCODE,
        MAIN_NO_TORCH_DUMPS,
        MODULE_ENTRY,
    ]
}

/// The frames of an exception the C encoder raises itself (an integer over the digit limit), innermost first.
pub fn json_encoder_frames() -> Vec<Site> {
    vec![
        JSON_ITERENCODE_ONE_SHOT,
        JSON_ENCODE_ITERENCODE,
        JSON_DUMPS_ENCODE,
        MAIN_NO_TORCH_DUMPS,
        MODULE_ENTRY,
    ]
}

/// The columns a character takes when the interpreter lays out a caret line: East Asian wide and fullwidth characters take two
/// (`_PyPegen_calculate_display_width` of CPython 3.12 asks `unicodedata.east_asian_width`; the ranges are those of the wide blocks).
fn display_width(ch: char) -> usize {
    match ch as u32 {
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x20000..=0x3FFFD => 2,
        _ => 1,
    }
}

fn width_of(text: &str) -> usize {
    text.chars().map(display_width).sum()
}

/// `    <source line stripped of leading whitespace>` and, when the span does not cover the whole stripped line, the caret line under it.
fn source_lines(site: &Site, out: &mut PyStr) {
    let stripped = site.text.trim_start_matches([' ', '\t', '\u{c}']);
    out.push_str("    ");
    out.push_str(stripped);
    out.push_str("\n");
    let Some((start_byte, end_byte)) = site.columns else {
        return;
    };
    let indent = site.text.len() - stripped.len();
    let (start, end) = (
        start_byte as usize,
        (end_byte as usize).min(site.text.len()),
    );
    if end - start == stripped.len() {
        return;
    }
    out.push_str("    ");
    out.push_str(&" ".repeat(width_of(&site.text[indent..start])));
    out.push_str(&"^".repeat(width_of(&site.text[start..end])));
    out.push_str("\n");
}

/// The report: the header, the frames outermost first (`stamps` are innermost first, as they were stamped while the exception propagated) and the
/// exception's own text (`final_text`, its `Class: message` block with the line end).
pub fn render(stamps: &[Site], script: &PyStr, final_text: &PyStr) -> PyStr {
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    for site in stamps.iter().rev() {
        out.push_str("  File \"");
        match site.file {
            SiteFile::Script => out.push_py(script),
            SiteFile::Stdlib(relative) => out.push_str(&format!("{STDLIB_PREFIX}/{relative}")),
        }
        out.push_str(&format!("\", line {}, in {}\n", site.line, site.function));
        source_lines(site, &mut out);
    }
    out.push_py(final_text);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_two_line_call_shows_carets_from_the_call_to_the_end_of_its_first_line() {
        let text = render(
            &json_encoder_frames()[3..],
            &PyStr::new("/x/resolve_torch_pin.py"),
            &PyStr::new("E: m\n"),
        )
        .to_utf8()
        .expect("text");
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[4],
            "    print(json.dumps({\"error\": \"build-system.requires 에 torch 없음\", \"requires\": requires},"
        );
        let carets = lines[5].trim_start();
        assert_eq!(lines[5].len() - carets.len(), 4 + 6);
        // three Hangul syllables take two columns each (the saved raw traceback of main-047/049 has 82 carets under a 79-character span)
        let first_line_chars = lines[4].trim_start().chars().count();
        assert_eq!(carets.chars().count(), first_line_chars - 6 + 3);
    }
}
