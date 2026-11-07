//! `installed_metadata()` / `from_installed()` of `regen_requirements.py` (lines 81-88): `importlib.metadata.distribution("vllm").metadata` on the
//! declared Python profile, without a Python runtime.
//!
//! The profile is `python3 -P -s -S -B` with an owned `PYTHONPATH`: `sys.path` is the `PYTHONPATH` entries followed by the interpreter's fixed
//! standard-library entries ([`STDLIB_SYS_PATH_SUFFIX`], a fact of the controlled CPython 3.12 layout, not a general one). The search is the one of
//! `importlib.metadata` in CPython 3.12.3: each `sys.path` entry is listed in the operating system's directory order, a child whose lower-cased name ends
//! in `.dist-info` or `.egg-info` is filed under the normalized text before its first `-` (runs of `- _ .` become one `_`), and the FIRST child filed
//! under `vllm` is the distribution: a later or better one is never looked at. Its text is `METADATA`, else `PKG-INFO`, else the entry itself
//! (empty or unreadable counts as absent; missing files, directories, a not-a-directory parent and permission errors are swallowed, any other
//! error is raised); it is decoded as strict UTF-8 text with universal newlines, its header section is read like the wheel's, and the message adapter
//! re-indents folded values. With no such text the message is empty, so the identity check fails.
//!
//! Not in scope (named): zip entries on `sys.path`, `.egg` roots, relative or empty `PYTHONPATH` entries, a directory entry that ends like a distribution directory but whose name is not UTF-8 (refused as `non_utf8_directory_entry`; other non-UTF-8 names cannot be filed and are skipped), and a
//! decoding failure in a file larger than one text chunk (8 KiB), where the exception's `object` would be a chunk the product does not reproduce.

use crate::http_direct::failure::{utf8_decode_error, ErrorValue};
use crate::http_direct::{Link, PyException};
use crate::mail_headers::{parse_header_section, redent};
use crate::py_errors::os_error_with_filename;
use crate::requirements_rules::metadata_fields;
use crate::tempdir::Provenance;
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

/// The interpreter's standard-library `sys.path` entries of the controlled profile, after the `PYTHONPATH` entries.
pub const STDLIB_SYS_PATH_SUFFIX: [&str; 3] = [
    "/usr/lib/python312.zip",
    "/usr/lib/python3.12",
    "/usr/lib/python3.12/lib-dynload",
];

/// The text decoder's chunk size: a decoding error in a larger file is outside this module's scope.
const TEXT_CHUNK_BYTES: usize = 8192;

/// What can end the lookup.
#[derive(Clone, Debug)]
pub enum InstalledError {
    Raised(Box<PyException>),
    OutOfScope {
        boundary: &'static str,
        detail: String,
    },
}

impl From<PyException> for InstalledError {
    fn from(error: PyException) -> InstalledError {
        InstalledError::Raised(Box::new(error))
    }
}

fn out_of_scope(boundary: &'static str, detail: impl Into<String>) -> InstalledError {
    InstalledError::OutOfScope {
        boundary,
        detail: detail.into(),
    }
}

/// `sys.path` as the profile builds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstalledProfile {
    pub sys_path: Vec<PathBuf>,
}

impl InstalledProfile {
    /// The profile for a `PYTHONPATH` value (`None` when the variable is unset).
    pub fn from_pythonpath(
        pythonpath: Option<&std::ffi::OsStr>,
    ) -> Result<InstalledProfile, InstalledError> {
        let mut sys_path = Vec::new();
        if let Some(value) = pythonpath {
            let text = value
                .to_str()
                .ok_or_else(|| out_of_scope("pythonpath", "PYTHONPATH is not valid UTF-8"))?;
            for entry in text.split(':').filter(|_| !text.is_empty()) {
                if entry.is_empty() || !Path::new(entry).is_absolute() {
                    return Err(out_of_scope(
                        "pythonpath_entry",
                        format!("PYTHONPATH entry {entry:?} is empty or relative; only absolute entries are modelled"),
                    ));
                }
                sys_path.push(PathBuf::from(entry));
            }
        }
        sys_path.extend(STDLIB_SYS_PATH_SUFFIX.iter().map(PathBuf::from));
        Ok(InstalledProfile { sys_path })
    }

    /// The profile of this process's environment.
    pub fn of_process() -> Result<InstalledProfile, InstalledError> {
        InstalledProfile::from_pythonpath(std::env::var_os("PYTHONPATH").as_deref())
    }
}

/// `Prepared.normalize`: runs of `-`, `_` and `.` become one `-`, lower-cased, then `-` becomes `_`.
fn normalize(name: &str) -> String {
    let mut out = String::new();
    let mut in_run = false;
    for ch in name.chars() {
        if matches!(ch, '-' | '_' | '.') {
            if !in_run {
                out.push('_');
            }
            in_run = true;
        } else {
            out.extend(ch.to_lowercase());
            in_run = false;
        }
    }
    out
}

/// The distribution name a child of a `sys.path` entry is filed under, when it is a `.dist-info` / `.egg-info` entry.
fn filed_name(child: &str) -> Option<String> {
    let low = agent_runtime_support::pylower::python_lower(child);
    if !(low.ends_with(".dist-info") || low.ends_with(".egg-info")) {
        return None;
    }
    let stem = low.rsplit_once('.').map_or(low.as_str(), |(stem, _)| stem);
    let name = stem.split_once('-').map_or(stem, |(name, _)| name);
    Some(normalize(name))
}

/// Whether raw name bytes end in `.dist-info` or `.egg-info` once ASCII letters are lower-cased (the end of the name is all the suffix test of the
/// original looks at, so the bytes before it do not matter here).
fn ends_like_distribution_directory(name: &[u8]) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(b".dist-info") || lower.ends_with(b".egg-info")
}

/// The first distribution directory named `wanted` along `profile.sys_path`.
fn find_distribution(
    profile: &InstalledProfile,
    wanted: &str,
) -> Result<Option<PathBuf>, InstalledError> {
    for root in &profile.sys_path {
        let base_is_egg = root
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.to_lowercase().ends_with(".egg"));
        if base_is_egg {
            return Err(out_of_scope(
                "egg_root",
                format!("{} is an .egg root", root.display()),
            ));
        }
        let Ok(children) = std::fs::read_dir(root) else {
            continue;
        };
        for child in children.flatten() {
            let raw_name = child.file_name();
            let Some(name) = raw_name.to_str() else {
                // `os.listdir` would hand the interpreter a surrogate-escaped name. Such a name can only be filed when its end says it is a
                // distribution directory; any other name fails the suffix test whatever its bytes are, so it is skipped as the original skips it.
                if ends_like_distribution_directory(raw_name.as_bytes()) {
                    return Err(out_of_scope(
                        "non_utf8_directory_entry",
                        format!(
                            "{} holds an entry that ends like a distribution directory but whose name is not valid UTF-8; surrogate-escaped names are not modelled",
                            root.display()
                        ),
                    ));
                }
                continue;
            };
            if filed_name(name).is_some_and(|filed| filed == wanted) {
                return Ok(Some(root.join(name)));
            }
        }
    }
    Ok(None)
}

/// `path.read_text(encoding="utf-8")` with the suppressions of `PathDistribution.read_text`: `None` when the file is absent, a directory, below a
/// non-directory, or unreadable by permission.
fn read_text(path: &Path, provenance: &mut Provenance) -> Result<Option<PyStr>, InstalledError> {
    provenance
        .metadata_read_attempts
        .push(path.to_string_lossy().into_owned());
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) if matches!(error.raw_os_error(), Some(1 | 2 | 13 | 20 | 21)) => {
            return Ok(None)
        }
        Err(error) => {
            let shown = PyStr::new(&path.to_string_lossy());
            return Err(match os_error_with_filename(&error, &shown) {
                Some(raised) => raised.into(),
                None => out_of_scope("read_error", error.to_string()),
            });
        }
    };
    let text = match std::str::from_utf8(&bytes) {
        Ok(text) => text,
        Err(error) if bytes.len() <= TEXT_CHUNK_BYTES => {
            return Err(utf8_decode_error(&bytes, &error).into());
        }
        Err(_) => {
            return Err(out_of_scope(
                "text_chunk_boundary",
                "a decoding error in a file larger than one text chunk",
            ));
        }
    };
    // universal newlines: `\r\n` and a lone `\r` become `\n`
    let mut translated = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            translated.push('\n');
        } else {
            translated.push(ch);
        }
    }
    Ok(Some(PyStr::new(&translated)))
}

/// `PackageNotFoundError("vllm")`, raised inside the `except StopIteration` of `Distribution.from_name`.
fn package_not_found(name: &str) -> PyException {
    PyException {
        class: "PackageNotFoundError",
        module: "importlib.metadata",
        message: PyStr::new(&format!("No package metadata was found for {name}")),
        args: vec![ErrorValue::Plain(PyValue::str(name))],
        attributes: vec![
            ("name", PyValue::str(name).into()),
            ("path", PyValue::None.into()),
            ("msg", PyValue::str(name).into()),
        ],
        http_headers: None,
        link: Some(Link::Context(Box::new(PyException {
            class: "StopIteration",
            module: "builtins",
            message: PyStr::default(),
            args: Vec::new(),
            attributes: Vec::new(),
            http_headers: None,
            link: None,
        }))),
    }
}

/// `_metadata_fields(importlib.metadata.distribution("vllm").metadata)`: the `Requires-Dist` values and the version.
pub fn installed_metadata(
    profile: &InstalledProfile,
    provenance: &mut Provenance,
) -> Result<(Vec<PyStr>, PyStr), InstalledError> {
    let Some(distribution) = find_distribution(profile, "vllm")? else {
        return Err(package_not_found("vllm").into());
    };
    let mut text = None;
    for candidate in [
        distribution.join("METADATA"),
        distribution.join("PKG-INFO"),
        distribution.clone(),
    ] {
        match read_text(&candidate, provenance)? {
            Some(found) if !found.is_empty() => {
                text = Some(found);
                break;
            }
            _ => {}
        }
    }
    let section = text
        .map(|text| parse_header_section(&text).map_values(redent))
        .unwrap_or_default();
    metadata_fields(
        &section.get_all("Name"),
        &section.get_all("Version"),
        section.get_all("Requires-Dist"),
    )
    .map_err(InstalledError::Raised)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "installed-checks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        root
    }

    fn profile_of(root: &Path) -> InstalledProfile {
        InstalledProfile {
            sys_path: vec![root.to_path_buf()],
        }
    }

    fn run(root: &Path) -> (Result<(Vec<PyStr>, PyStr), InstalledError>, Provenance) {
        let mut provenance = Provenance::default();
        let outcome = installed_metadata(&profile_of(root), &mut provenance);
        (outcome, provenance)
    }

    fn shown(items: &[PyStr]) -> Vec<String> {
        items.iter().map(|item| item.to_utf8().unwrap()).collect()
    }

    #[test]
    fn the_first_matching_dist_info_is_read_as_metadata_with_its_requires_in_order() {
        let root = owned_root();
        let dist = root.join("vllm-0.32.0.dist-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(
            dist.join("METADATA"),
            "Name: vllm\nVersion: 0.32.0\nRequires-Dist: b\nRequires-Dist: a>=1\n\nbody\n",
        )
        .unwrap();
        let (outcome, provenance) = run(&root);
        let (requires, version) = outcome.unwrap();
        assert_eq!(
            (shown(&requires), version.to_utf8().unwrap()),
            (
                vec!["b".to_string(), "a>=1".to_string()],
                "0.32.0".to_string()
            )
        );
        assert_eq!(
            provenance.metadata_read_attempts,
            vec![dist.join("METADATA").to_string_lossy().into_owned()]
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn names_match_ignoring_case_and_pkg_info_is_the_fallback_while_a_longer_name_is_another_distribution(
    ) {
        let root = owned_root();
        let dist = root.join("VLLM-1.egg-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(
            dist.join("PKG-INFO"),
            "Name: vLLM\nVersion: 1\r\nRequires-Dist: z\r\n",
        )
        .unwrap();
        let (outcome, provenance) = run(&root);
        let (requires, version) = outcome.unwrap();
        // the CRLF text is translated: no `\r` reaches a value
        assert_eq!(
            (shown(&requires), version.to_utf8().unwrap()),
            (vec!["z".to_string()], "1".to_string())
        );
        assert_eq!(provenance.metadata_read_attempts.len(), 2);
        assert!(
            provenance.metadata_read_attempts[0].ends_with("/METADATA")
                && provenance.metadata_read_attempts[1].ends_with("/PKG-INFO")
        );
        // the name is the text before the first `-` (`vllm_x` is another distribution, `vllm.x` and `vllm-x` normalize differently from `vllm`)
        assert_eq!(filed_name("VLLM_x-1.egg-info").as_deref(), Some("vllm_x"));
        assert_eq!(filed_name("V.L__L-M-1.dist-info").as_deref(), Some("v_l_l"));
        assert_eq!(filed_name("vllm.dist-info").as_deref(), Some("vllm"));
        assert_eq!(filed_name("vllm"), None);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_empty_metadata_falls_through_and_no_text_at_all_fails_the_identity_check() {
        let root = owned_root();
        let dist = root.join("vllm-1.dist-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(dist.join("METADATA"), "").unwrap();
        match run(&root).0 {
            Err(InstalledError::Raised(error)) => assert_eq!(
                error.message.to_utf8().unwrap(),
                crate::requirements_rules::IDENTITY_FAILURE
            ),
            other => panic!("RuntimeError expected: {other:?}"),
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_missing_distribution_is_package_not_found_raised_while_handling_stop_iteration() {
        let root = owned_root();
        std::fs::create_dir(root.join("other-1.dist-info")).unwrap();
        match run(&root).0 {
            Err(InstalledError::Raised(error)) => {
                assert_eq!(
                    (error.class, error.module),
                    ("PackageNotFoundError", "importlib.metadata")
                );
                assert_eq!(
                    error.message.to_utf8().unwrap(),
                    "No package metadata was found for vllm"
                );
                let Some(Link::Context(context)) = &error.link else {
                    panic!("context")
                };
                assert_eq!(context.class, "StopIteration");
            }
            other => panic!("PackageNotFoundError expected: {other:?}"),
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn folded_values_are_reindented_and_invalid_utf8_is_the_decode_error_of_the_whole_text() {
        let root = owned_root();
        let dist = root.join("vllm-1.dist-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(
            dist.join("METADATA"),
            "Name: vllm\nVersion: 1\nRequires-Dist: a\n        b\n",
        )
        .unwrap();
        let (requires, _) = run(&root).0.unwrap();
        assert_eq!(shown(&requires), ["a\nb"]);
        std::fs::write(
            dist.join("METADATA"),
            b"Name: vllm\nRequires-Dist: \xff\n\n",
        )
        .unwrap();
        match run(&root).0 {
            Err(InstalledError::Raised(error)) => {
                assert_eq!(
                    error.message.to_utf8().unwrap(),
                    "'utf-8' codec can't decode byte 0xff in position 26: invalid start byte"
                );
            }
            other => panic!("UnicodeDecodeError expected: {other:?}"),
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_profile_is_pythonpath_then_the_stdlib_entries_and_refuses_relative_or_empty_entries() {
        let profile =
            InstalledProfile::from_pythonpath(Some(std::ffi::OsStr::new("/a:/b"))).unwrap();
        assert_eq!(profile.sys_path.len(), 5);
        assert_eq!(profile.sys_path[0], PathBuf::from("/a"));
        assert_eq!(
            InstalledProfile::from_pythonpath(None)
                .unwrap()
                .sys_path
                .len(),
            3
        );
        for bad in ["rel", "/a::/b", "/a:"] {
            assert!(matches!(
                InstalledProfile::from_pythonpath(Some(std::ffi::OsStr::new(bad))),
                Err(InstalledError::OutOfScope {
                    boundary: "pythonpath_entry",
                    ..
                })
            ));
        }
    }

    #[test]
    fn a_non_utf8_entry_that_ends_like_a_distribution_directory_is_refused_by_name_while_other_odd_names_are_skipped(
    ) {
        use std::os::unix::ffi::OsStringExt;
        let root = owned_root();
        let odd = std::ffi::OsString::from_vec(b"vllm-\xff.dist-info".to_vec());
        std::fs::create_dir(root.join(&odd)).unwrap();
        match run(&root).0 {
            Err(InstalledError::OutOfScope { boundary, .. }) => {
                assert_eq!(boundary, "non_utf8_directory_entry")
            }
            other => panic!("a named boundary was expected, not a skipped candidate: {other:?}"),
        }
        std::fs::remove_dir(root.join(&odd)).unwrap();
        // a non-UTF-8 name that cannot be filed fails the suffix test whatever its bytes are: skipped, the real distribution is found
        std::fs::write(
            root.join(std::ffi::OsString::from_vec(b"notes-\xfe.txt".to_vec())),
            b"x",
        )
        .unwrap();
        match run(&root).0 {
            Err(InstalledError::Raised(error)) => assert_eq!(error.class, "PackageNotFoundError"),
            other => panic!("PackageNotFoundError expected without a distribution: {other:?}"),
        }
        let dist = root.join("vllm-1.dist-info");
        std::fs::create_dir(&dist).unwrap();
        std::fs::write(
            dist.join("METADATA"),
            "Name: vllm\nVersion: 1\nRequires-Dist: z\n",
        )
        .unwrap();
        let (requires, version) = run(&root).0.unwrap();
        assert_eq!(
            (shown(&requires), version.to_utf8().unwrap()),
            (vec!["z".to_string()], "1".to_string())
        );
        assert!(ends_like_distribution_directory(b"X\xff.DIST-INFO"));
        assert!(ends_like_distribution_directory(b"\xff.egg-info"));
        assert!(!ends_like_distribution_directory(b"\xff.dist-infox"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
