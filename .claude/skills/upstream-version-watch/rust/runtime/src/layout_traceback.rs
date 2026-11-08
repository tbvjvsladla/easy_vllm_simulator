//! The uncaught-exception report of the public command line: CPython 3.12.3's `Traceback (most recent call last):` block for the sites the command
//! can raise through, rendered from the generated source-location metadata ([`crate::layout_sites`]) and the caller's `argv[0]`.
//!
//! These are COMPATIBILITY frames: the locations of the original script's code, stated as metadata. Nothing here reads a Python file, runs Python
//! or claims that it ran; an exception that escaped through a site the table does not hold is reported without frames (see `ABI-r2.md`).

use crate::layout_sites::{Site, SiteFile, SiteId, SITES, STDLIB_PREFIX};
use agent_runtime_support::pystr::PyStr;

/// The site record of an id.
pub fn site(id: SiteId) -> &'static Site {
    SITES
        .iter()
        .find(|site| site.id == id)
        .expect("every site id is in the generated table")
}

/// The number of characters in the first `bytes` bytes of `text` (the interpreter stores byte offsets and prints character columns).
fn characters_before(text: &str, bytes: u32) -> usize {
    let end = (bytes as usize).min(text.len());
    text.get(..end).map_or_else(
        || {
            String::from_utf8_lossy(&text.as_bytes()[..end])
                .chars()
                .count()
        },
        |head| head.chars().count(),
    )
}

/// `    <source line stripped of leading whitespace>` and, when the instruction does not cover the whole stripped line, the caret line under it.
fn source_lines(site: &Site, text: &str, out: &mut PyStr) {
    let stripped = text.trim_start_matches([' ', '\t', '\u{c}']);
    out.push_str("    ");
    out.push_str(stripped);
    out.push_str("\n");
    let Some((start_byte, end_byte)) = site.columns else {
        return;
    };
    let indent = characters_before(text, (text.len() - stripped.len()) as u32);
    let start = characters_before(text, start_byte);
    let end = characters_before(text, end_byte);
    let anchors = site.anchors.map(|(left, right)| {
        (
            characters_before(text, left),
            characters_before(text, right),
        )
    });
    let shown_whole_line = end - start == stripped.chars().count();
    let secondary_gap = anchors.is_some_and(|(left, right)| right > left);
    if shown_whole_line && !secondary_gap {
        return;
    }
    out.push_str("    ");
    out.push_str(&" ".repeat(start.saturating_sub(indent)));
    for column in start..end {
        let mark = match anchors {
            Some((left, right)) if column < left || column >= right => '~',
            _ => '^',
        };
        out.push_str(&mark.to_string());
    }
    out.push_str("\n");
}

/// The file name a frame shows: the script is the caller's `script`, the standard library the reference interpreter's prefix, a frozen module its
/// `<frozen name>`.
fn file_name(site: &Site, script: &PyStr) -> PyStr {
    match site.file {
        SiteFile::Owner => script.clone(),
        SiteFile::Stdlib(relative) => PyStr::new(&format!("{STDLIB_PREFIX}/{relative}")),
        SiteFile::Frozen(name) => PyStr::new(name),
    }
}

/// The report: the header, the frames outermost first (`stamps` are innermost first, as they were stamped while the exception propagated) and the
/// exception's own text (`final_text`, the `Class: message` block with its line end).
pub fn render(stamps: &[SiteId], script: &PyStr, final_text: &PyStr) -> PyStr {
    let mut out = PyStr::new("Traceback (most recent call last):\n");
    for id in stamps.iter().rev() {
        let site = site(*id);
        out.push_str("  File \"");
        out.push_py(&file_name(site, script));
        out.push_str(&format!("\", line {}, in {}\n", site.line, site.function));
        if let Some(text) = site.text {
            source_lines(site, text, &mut out);
        }
    }
    out.push_py(final_text);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered(stamps: &[SiteId], last: &str) -> String {
        render(
            stamps,
            &PyStr::new("/x/check_spec_layout.py"),
            &PyStr::new(last),
        )
        .to_utf8()
        .expect("text")
    }

    #[test]
    fn the_entry_frame_has_no_caret_line_because_its_span_is_the_whole_line() {
        let text = rendered(&[SiteId::ModuleEntry], "E: m\n");
        assert_eq!(
            text,
            "Traceback (most recent call last):\n  File \"/x/check_spec_layout.py\", line 196, in <module>\n    main()\nE: m\n"
        );
    }

    #[test]
    fn a_binary_operation_shows_its_operator_anchors_and_an_attribute_its_whole_span() {
        let text = rendered(
            &[SiteId::MainNextnMinusOne, SiteId::ModuleEntry],
            "TypeError: x\n",
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[4],
            "    visible = \"mtp.0\" if nextn in (1, None) else f\"mtp.0..{nextn - 1}\""
        );
        assert_eq!(lines[5], format!("    {}~~~~~~^~~", " ".repeat(55)));
        let text = rendered(
            &[SiteId::IndexGet, SiteId::MainLoadIndex, SiteId::ModuleEntry],
            "AttributeError: y\n",
        );
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[7], "    wm = json.load(f).get(\"weight_map\") or {}");
        assert_eq!(lines[8], format!("    {}{}", " ".repeat(5), "^".repeat(16)));
    }

    #[test]
    fn a_frozen_frame_has_no_source_and_a_stdlib_frame_is_under_the_reference_prefix() {
        let text = rendered(
            &[
                SiteId::CodecsDecode,
                SiteId::JsonLoadRead,
                SiteId::IndexJsonLoad,
            ],
            "UnicodeDecodeError: z\n",
        );
        assert!(text
            .contains("  File \"<frozen codecs>\", line 322, in decode\nUnicodeDecodeError: z\n"));
        assert!(text.contains(&format!(
            "  File \"{STDLIB_PREFIX}/json/__init__.py\", line 293, in load\n    return loads(fp.read(),\n"
        )));
    }
}
