//! `parse_requires` and `_metadata_fields` of `regen_requirements.py` (lines 25-78): the source's OWN string rules over `Requires-Dist` lines.
//!
//! This is deliberately not a PEP 508 / `packaging.Requirement` reader: a line is cut at its first `;` (every marker that is not an `extra` marker is
//! DELETED, as the original does), the package name is the text before the first of `< > = ! [` or a space, and the literal `KNOWN_INCOMPAT` ceilings are
//! appended to the cut text with a comma when it already has a constraint character. Every oddity of that is kept: `~=` counts as a constraint, names
//! are lower-cased with Unicode rules after a Unicode `strip`, the base-provided set is matched on that name, `extra` is a lower-case substring test of
//! the text after the first `;`, and the one exemption is `transformers` for a version equal to the text `0.32.0`.
//!
//! The public function takes typed Python values because the original is called with whatever the caller passes: the iteration of `reqs` and the
//! membership and `split` calls on its elements fail with the exceptions CPython raises for that type, at the element where they happen.

use crate::http_direct::PyException;
use crate::py_errors::{attribute_error, builtin, not_a_container, not_iterable};
use agent_runtime_support::pylower::python_lower;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::{PyValue, Typed};

/// `BASE_PROVIDED`: packages the base image provides; a requirement of one of them is dropped.
pub const BASE_PROVIDED: [&str; 8] = [
    "torch",
    "torchvision",
    "torchaudio",
    "torchao",
    "setuptools",
    "numpy",
    "pip",
    "wheel",
];

/// `KNOWN_INCOMPAT`: package name to the constraint appended to its requirement line, in the source's order.
pub const KNOWN_INCOMPAT: [(&str, &str); 2] =
    [("fastapi", "<0.137.0"), ("transformers", "<5.15.0")];

/// The one metadata version for which the `transformers` ceiling is omitted.
pub const EXEMPT_VERSION: &str = "0.32.0";

/// The `RuntimeError` text of an unusable METADATA identity.
pub const IDENTITY_FAILURE: &str = "vLLM METADATA Name/Version missing, malformed or ambiguous";

/// The two lists `parse_requires` returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parsed {
    /// `sorted(set(out))`: the requirement lines.
    pub lines: Vec<PyStr>,
    /// `sorted(set(applied))`: the ceilings that were appended, as `name + ceiling`.
    pub applied: Vec<PyStr>,
}

fn point(ch: char) -> u32 {
    u32::from(ch)
}

fn contains_sequence(haystack: &[u32], needle: &str) -> bool {
    let needle: Vec<u32> = needle.chars().map(point).collect();
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

/// `str.lower()` on text that may hold lone surrogates: the valid runs are lowered by the shared Unicode rules, a surrogate stays as it is.
fn lower(text: &PyStr) -> PyStr {
    let mut out = Vec::with_capacity(text.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut Vec<u32>| {
        out.extend(python_lower(run).chars().map(point));
        run.clear();
    };
    for &code in text.code_points() {
        match char::from_u32(code) {
            Some(ch) => run.push(ch),
            None => {
                flush(&mut run, &mut out);
                out.push(code);
            }
        }
    }
    flush(&mut run, &mut out);
    PyStr::from_code_points(out)
}

/// The cut-and-ceiling rule of one requirement text; `None` when the line is dropped (extra marker, empty name, base-provided name).
fn filter_one(
    requirement: &PyStr,
    exempt_transformers: bool,
    applied: &mut Vec<PyStr>,
) -> Option<PyStr> {
    let points = requirement.code_points();
    let semicolon = points.iter().position(|&code| code == point(';'));
    if let Some(at) = semicolon {
        if contains_sequence(&points[at + 1..], "extra") {
            return None;
        }
    }
    let kept = &points[..semicolon.unwrap_or(points.len())];
    let base = PyStr::from_code_points(kept.to_vec()).strip();
    let base_points = base.code_points();
    let name_end = base_points
        .iter()
        .position(|&code| "<>=![ ".chars().any(|ch| point(ch) == code))
        .unwrap_or(base_points.len());
    let name = lower(&PyStr::from_code_points(base_points[..name_end].to_vec()).strip());
    if name.is_empty() || BASE_PROVIDED.iter().any(|known| name.equals(known)) {
        return None;
    }
    let ceiling = KNOWN_INCOMPAT
        .iter()
        .find(|(known, _)| name.equals(known))
        .map(|(_, ceiling)| *ceiling);
    let mut result = base.clone();
    if let Some(ceiling) = ceiling {
        if !(name.equals("transformers") && exempt_transformers) {
            if base_points
                .iter()
                .any(|&code| "<>=!~".chars().any(|ch| point(ch) == code))
            {
                result.push_str(",");
            }
            result.push_str(ceiling);
            let mut note = name;
            note.push_str(ceiling);
            applied.push(note);
        }
    }
    Some(result)
}

fn sorted_unique(mut items: Vec<PyStr>) -> Vec<PyStr> {
    items.sort_by(|left, right| left.code_points().cmp(right.code_points()));
    items.dedup();
    items
}

/// `parse_requires` over already-text requirements; `exempt_transformers` is `name == "transformers" and version == "0.32.0"`'s second half.
pub fn parse_requirement_texts(requirements: &[PyStr], exempt_transformers: bool) -> Parsed {
    let mut applied = Vec::new();
    let lines = requirements
        .iter()
        .filter_map(|requirement| filter_one(requirement, exempt_transformers, &mut applied))
        .collect();
    Parsed {
        lines: sorted_unique(lines),
        applied: sorted_unique(applied),
    }
}

/// `version == "0.32.0"` for any Python value.
pub fn is_exempt_version(version: &PyValue) -> bool {
    matches!(version.peel(), PyValue::Str(text) if text.equals(EXEMPT_VERSION))
}

/// The elements `for r in reqs` yields. A `str` yields its characters, a `dict` its keys, a `set` its members in the order the caller observed,
/// `bytes` its integers; a value that is not iterable fails as the loop would.
fn elements(reqs: &PyValue) -> Result<Vec<PyValue>, Box<PyException>> {
    match reqs.peel() {
        PyValue::List(items) | PyValue::Tuple(items) => Ok(items.clone()),
        PyValue::Dict(members) => Ok(members
            .iter()
            .map(|(key, _)| PyValue::Str(key.clone()))
            .collect()),
        PyValue::Str(text) => Ok(text
            .code_points()
            .iter()
            .map(|&code| PyValue::Str(PyStr::from_code_points(vec![code])))
            .collect()),
        PyValue::Bytes(bytes) => Ok(bytes.iter().map(|&byte| PyValue::int(byte)).collect()),
        PyValue::Opaque(opaque) => match &opaque.typed {
            Typed::Set(members) => Ok(members.clone()),
            _ => Err(Box::new(not_iterable(reqs))),
        },
        _ => Err(Box::new(not_iterable(reqs))),
    }
}

/// The text of one element, or the exception `";" in r` / `r.split(";")` raise for it.
fn element_text(element: &PyValue) -> Result<PyStr, Box<PyException>> {
    match element.peel() {
        PyValue::Str(text) => Ok(text.clone()),
        PyValue::Bytes(_) => Err(Box::new(builtin(
            "TypeError",
            "a bytes-like object is required, not 'str'",
        ))),
        // a container answers `";" in r`, and then has no `split`
        PyValue::List(_) | PyValue::Tuple(_) | PyValue::Dict(_) => {
            Err(Box::new(attribute_error(element, "split")))
        }
        PyValue::Opaque(opaque) if matches!(opaque.typed, Typed::Set(_)) => {
            Err(Box::new(attribute_error(element, "split")))
        }
        other => Err(Box::new(not_a_container(other))),
    }
}

/// `parse_requires(reqs, version)` for any typed arguments.
pub fn parse_requires(reqs: &PyValue, version: &PyValue) -> Result<Parsed, Box<PyException>> {
    let mut texts = Vec::new();
    for element in elements(reqs)? {
        texts.push(element_text(&element)?);
    }
    Ok(parse_requirement_texts(&texts, is_exempt_version(version)))
}

/// `re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9.!+_-]*", text)`.
fn is_version_text(text: &PyStr) -> bool {
    let points = text.code_points();
    let ascii_alnum = |code: u32| char::from_u32(code).is_some_and(|ch| ch.is_ascii_alphanumeric());
    match points.split_first() {
        Some((&first, rest)) => {
            ascii_alnum(first)
                && rest.iter().all(|&code| {
                    ascii_alnum(code) || char::from_u32(code).is_some_and(|ch| ".!+_-".contains(ch))
                })
        }
        None => false,
    }
}

/// The distribution identity of `_metadata_fields`: exactly one `Name` that is `vllm` after `strip().lower()`, exactly one `Version` of the ASCII
/// grammar after `strip()`; the `Requires-Dist` values are returned in header order. Never inferred from a file or URL name.
pub fn metadata_fields(
    names: &[PyStr],
    versions: &[PyStr],
    requires: Vec<PyStr>,
) -> Result<(Vec<PyStr>, PyStr), Box<PyException>> {
    let usable = names.len() == 1
        && lower(&names[0].strip()).equals("vllm")
        && versions.len() == 1
        && is_version_text(&versions[0].strip());
    if !usable {
        return Err(Box::new(builtin("RuntimeError", IDENTITY_FAILURE)));
    }
    Ok((requires, versions[0].strip()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(items: &[&str]) -> Vec<PyStr> {
        items.iter().map(|item| PyStr::new(item)).collect()
    }

    fn shown(items: &[PyStr]) -> Vec<String> {
        items.iter().map(|item| item.to_utf8().unwrap()).collect()
    }

    fn parse(items: &[&str], exempt: bool) -> (Vec<String>, Vec<String>) {
        let parsed = parse_requirement_texts(&texts(items), exempt);
        (shown(&parsed.lines), shown(&parsed.applied))
    }

    #[test]
    fn ceilings_join_with_a_comma_only_after_a_constraint_character() {
        let (lines, applied) = parse(&["transformers>=5.16.1,<5.20.0", "fastapi>=0.133.0"], false);
        assert_eq!(
            lines,
            [
                "fastapi>=0.133.0,<0.137.0",
                "transformers>=5.16.1,<5.20.0,<5.15.0"
            ]
        );
        assert_eq!(applied, ["fastapi<0.137.0", "transformers<5.15.0"]);
        // a bare name takes the ceiling without a comma
        let (lines, _) = parse(&["fastapi", "fastapi[standard]"], false);
        assert_eq!(lines, ["fastapi<0.137.0", "fastapi[standard]<0.137.0"]);
        // quirk kept: the name ends at `< > = ! [` or a space only, so for `~=` it is `transformers~` and no ceiling is found;
        // `~` still counts as a constraint character once a ceiling does apply
        let (lines, applied) = parse(
            &["transformers~=5.0", "fastapi~=0.1", "fastapi[standard]~=1"],
            false,
        );
        assert_eq!(
            lines,
            [
                "fastapi[standard]~=1,<0.137.0",
                "fastapi~=0.1",
                "transformers~=5.0"
            ]
        );
        assert_eq!(applied, ["fastapi<0.137.0"]);
    }

    #[test]
    fn only_the_exact_version_text_exempts_transformers_and_only_transformers() {
        let (lines, applied) = parse(&["transformers>=5.16.1,<5.20.0", "fastapi>=0.133.0"], true);
        assert_eq!(
            lines,
            ["fastapi>=0.133.0,<0.137.0", "transformers>=5.16.1,<5.20.0"]
        );
        assert_eq!(applied, ["fastapi<0.137.0"]);
        assert!(is_exempt_version(&PyValue::str("0.32.0")));
        for other in [
            PyValue::str("0.32.0+local"),
            PyValue::str(" 0.32.0"),
            PyValue::None,
            PyValue::int(32),
        ] {
            assert!(!is_exempt_version(&other));
        }
    }

    #[test]
    fn markers_other_than_extra_are_deleted_and_extra_markers_drop_the_line() {
        let (lines, _) = parse(
            &[
                "numpy>=1.2; python_version<'3.9'",
                "uvloop>=0.1 ; sys_platform=='linux'",
                "Pillow; extra == 'x'",
                "pyyaml ; EXTRA == 'y'",
                "requests[socks] >= 2 ; python_version >= '3'",
                "  TORCH==2 ",
                "wheel",
                "",
                " ; marker only",
            ],
            false,
        );
        // `EXTRA` is not the lower-case substring, so that line is kept; the base-provided names and empty names are dropped
        assert_eq!(lines, ["pyyaml", "requests[socks] >= 2", "uvloop>=0.1"]);
    }

    #[test]
    fn the_result_is_sorted_by_code_point_and_deduplicated() {
        let (lines, _) = parse(&["b", "a", "b", "B", "\u{e9}", "z"], false);
        assert_eq!(lines, ["B", "a", "b", "z", "\u{e9}"]);
    }

    #[test]
    fn typed_arguments_fail_as_the_interpreter_does_at_the_element_that_fails() {
        let none = parse_requires(&PyValue::None, &PyValue::None).unwrap_err();
        assert_eq!(
            none.message.to_utf8().unwrap(),
            "'NoneType' object is not iterable"
        );
        let integer_element = PyValue::List(vec![PyValue::str("a"), PyValue::int(3)]);
        let error = parse_requires(&integer_element, &PyValue::None).unwrap_err();
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "argument of type 'int' is not iterable"
        );
        let bytes_element = PyValue::List(vec![PyValue::Bytes(b"a".to_vec())]);
        let error = parse_requires(&bytes_element, &PyValue::None).unwrap_err();
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "a bytes-like object is required, not 'str'"
        );
        let list_element = PyValue::List(vec![PyValue::List(Vec::new())]);
        let error = parse_requires(&list_element, &PyValue::None).unwrap_err();
        assert_eq!(
            (error.class, error.message.to_utf8().unwrap().as_str()),
            ("AttributeError", "'list' object has no attribute 'split'")
        );
        // a string iterates its characters; a dict its keys; `bytes` its integers
        let parsed = parse_requires(&PyValue::str("ab"), &PyValue::None).unwrap();
        assert_eq!(shown(&parsed.lines), ["a", "b"]);
        let error = parse_requires(&PyValue::Bytes(b"x".to_vec()), &PyValue::None).unwrap_err();
        assert_eq!(
            error.message.to_utf8().unwrap(),
            "argument of type 'int' is not iterable"
        );
        assert!(parse_requires(&PyValue::Bytes(Vec::new()), &PyValue::None)
            .unwrap()
            .lines
            .is_empty());
    }

    #[test]
    fn identity_needs_one_name_one_ascii_version_and_keeps_the_header_order() {
        let ok = metadata_fields(
            &texts(&[" VLLM "]),
            &texts(&[" 0.32.0+cu130 "]),
            texts(&["b", "a"]),
        )
        .unwrap();
        assert_eq!(
            (shown(&ok.0), ok.1.to_utf8().unwrap()),
            (
                vec!["b".to_string(), "a".to_string()],
                "0.32.0+cu130".to_string()
            )
        );
        for (names, versions) in [
            (vec![], vec!["0.32.0"]),
            (vec!["vllm", "vllm"], vec!["0.32.0"]),
            (vec!["other"], vec!["0.32.0"]),
            (vec!["vllm"], vec![]),
            (vec!["vllm"], vec!["0.32.0", "0.31.0"]),
            (vec!["vllm"], vec!["bad version"]),
            (vec!["vllm"], vec!["+1"]),
            (vec!["vllm"], vec![""]),
        ] {
            let error = metadata_fields(&texts(&names), &texts(&versions), Vec::new()).unwrap_err();
            assert_eq!(error.message.to_utf8().unwrap(), IDENTITY_FAILURE);
        }
    }
}
