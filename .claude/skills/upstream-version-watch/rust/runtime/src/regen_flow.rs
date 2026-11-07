//! The four metadata-reading functions of `regen_requirements.py` (lines 81-106) as one flow over the native pieces:
//!
//! * `wheel_metadata(url)`: a temporary directory, `urlretrieve(url, <dir>/pkg.whl)`, `ZipFile(<dir>/pkg.whl)`, the single member whose name ends in
//!   `.dist-info/METADATA` (any other count is `RuntimeError`), its bytes decoded as strict UTF-8, the header section of the text, and the identity
//!   check; the directory is removed on every way out.
//! * `from_wheel_url(url)`: the `Requires-Dist` list of that.
//! * `installed_metadata()` / `from_installed()`: the same over [`crate::installed_dist`].
//!
//! Each failure carries the [`Stage`] it came from, which decides the frames of an uncaught report.

use crate::fetch_https::FetchEnvironment;
use crate::http_direct::failure::{utf8_decode_error, TransportFailure};
use crate::http_direct::{Dial, PyException};
use crate::installed_dist::{
    installed_metadata as installed_lookup, InstalledError, InstalledProfile,
};
use crate::mail_headers::parse_header_section;
use crate::py_errors::{builtin, os_error_with_filename};
use crate::requirements_rules::metadata_fields;
use crate::tempdir::{with_temporary_directory, Provenance, TempBase};
use crate::urlretrieve::urlretrieve;
use crate::wheel_fetch::FetchError;
use crate::zip_archive::{ZipArchive, ZipError};
use agent_runtime_support::pystr::PyStr;
use agent_runtime_support::pyvalue::PyValue;
use std::path::Path;

/// The `RuntimeError` text of a wheel without exactly one METADATA member.
pub const NO_UNIQUE_METADATA: &str = "wheel 에 유일한 METADATA 없음";

/// Where in the original a failure was raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// `urllib.request.urlretrieve(url, whl)` (line 95).
    Urlretrieve,
    /// `zipfile.ZipFile(whl)` (line 96).
    ZipOpen,
    /// `z.read(metas[0])` / `.decode("utf-8")` / the unique-member check (lines 97-100).
    ZipMember,
    /// `_metadata_fields(...)` of the wheel path (line 101).
    WheelIdentity,
    /// `importlib.metadata.distribution("vllm")` and `.metadata` (line 83).
    InstalledLookup,
    /// `_metadata_fields(...)` of the installed path (line 83).
    InstalledIdentity,
    /// Anything else (temporary directory handling, file reads of the archive).
    Other,
}

/// What can end a metadata read.
#[derive(Clone, Debug)]
pub enum FlowError {
    Raised {
        error: Box<PyException>,
        stage: Stage,
    },
    OutOfScope {
        boundary: String,
        detail: String,
    },
    Transport(TransportFailure),
}

impl From<Box<PyException>> for FlowError {
    fn from(error: Box<PyException>) -> FlowError {
        FlowError::Raised {
            error,
            stage: Stage::Other,
        }
    }
}

impl From<PyException> for FlowError {
    fn from(error: PyException) -> FlowError {
        FlowError::Raised {
            error: Box::new(error),
            stage: Stage::Other,
        }
    }
}

fn raised(error: PyException, stage: Stage) -> FlowError {
    FlowError::Raised {
        error: Box::new(error),
        stage,
    }
}

fn from_fetch(error: FetchError) -> FlowError {
    match error {
        FetchError::Raised { error, .. } => FlowError::Raised {
            error,
            stage: Stage::Urlretrieve,
        },
        FetchError::OutOfScope(refusal) => FlowError::OutOfScope {
            boundary: refusal.boundary.to_string(),
            detail: refusal.detail,
        },
        FetchError::Transport(transport) => FlowError::Transport(transport),
    }
}

fn from_zip(error: ZipError, stage: Stage) -> FlowError {
    match error {
        ZipError::Raised(error) => FlowError::Raised { error, stage },
        ZipError::OutOfScope { boundary, detail } => FlowError::OutOfScope {
            boundary: boundary.to_string(),
            detail,
        },
    }
}

/// The three header lists of a METADATA text and the identity check, as `_metadata_fields(Parser().parsestr(text))` does.
pub fn identity_of(text: &PyStr) -> Result<(Vec<PyStr>, PyStr), Box<PyException>> {
    let section = parse_header_section(text);
    metadata_fields(
        &section.get_all("Name"),
        &section.get_all("Version"),
        section.get_all("Requires-Dist"),
    )
}

/// `wheel_metadata(url)`: `(Requires-Dist values, METADATA Version)`.
pub fn wheel_metadata(
    url: &PyValue,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
    base: &TempBase,
    provenance: &mut Provenance,
) -> Result<(Vec<PyStr>, PyStr), FlowError> {
    wheel_metadata_via(base, provenance, |wheel| {
        urlretrieve(url, wheel, environment, dialer).map_err(from_fetch)
    })
}

/// `wheel_metadata` with the retrieval step supplied by the caller (the source's self-test replaces `urlretrieve` by a file copy); everything
/// around it, the temporary directory, the archive, the member, the decoding and the identity check, is the same code.
pub fn wheel_metadata_via(
    base: &TempBase,
    provenance: &mut Provenance,
    retrieve: impl FnOnce(&Path) -> Result<(), FlowError>,
) -> Result<(Vec<PyStr>, PyStr), FlowError> {
    with_temporary_directory(base, provenance, |directory, _| {
        let wheel = directory.join("pkg.whl");
        retrieve(&wheel)?;
        let bytes = std::fs::read(&wheel).map_err(|error| {
            let shown = PyStr::new(&wheel.to_string_lossy());
            match os_error_with_filename(&error, &shown) {
                Some(error) => raised(error, Stage::ZipOpen),
                None => FlowError::OutOfScope {
                    boundary: "archive_read".to_string(),
                    detail: error.to_string(),
                },
            }
        })?;
        let archive = ZipArchive::open(&bytes).map_err(|error| from_zip(error, Stage::ZipOpen))?;
        let suffix: Vec<u32> = ".dist-info/METADATA".chars().map(u32::from).collect();
        let members: Vec<usize> = archive
            .namelist()
            .iter()
            .enumerate()
            .filter(|(_, name)| name.code_points().ends_with(&suffix))
            .map(|(index, _)| index)
            .collect();
        let [member] = members[..] else {
            return Err(raised(
                builtin("RuntimeError", NO_UNIQUE_METADATA),
                Stage::ZipMember,
            ));
        };
        let raw = archive
            .read_entry(member)
            .map_err(|error| from_zip(error, Stage::ZipMember))?;
        let text = match std::str::from_utf8(&raw) {
            Ok(text) => PyStr::new(text),
            Err(error) => return Err(raised(utf8_decode_error(&raw, &error), Stage::ZipMember)),
        };
        identity_of(&text).map_err(|error| raised(*error, Stage::WheelIdentity))
    })
}

/// `from_wheel_url(url)`: only the `Requires-Dist` values.
pub fn from_wheel_url(
    url: &PyValue,
    environment: &FetchEnvironment,
    dialer: &dyn Dial,
    base: &TempBase,
    provenance: &mut Provenance,
) -> Result<Vec<PyStr>, FlowError> {
    wheel_metadata(url, environment, dialer, base, provenance).map(|(requires, _)| requires)
}

/// `installed_metadata()`: `(Requires-Dist values, METADATA Version)` of the installed `vllm` on the declared profile.
pub fn installed_metadata(
    profile: &InstalledProfile,
    provenance: &mut Provenance,
) -> Result<(Vec<PyStr>, PyStr), FlowError> {
    installed_lookup(profile, provenance).map_err(|error| match error {
        InstalledError::Raised(error) => {
            let stage = if error.message.to_utf8().as_deref()
                == Some(crate::requirements_rules::IDENTITY_FAILURE)
            {
                Stage::InstalledIdentity
            } else {
                Stage::InstalledLookup
            };
            FlowError::Raised { error, stage }
        }
        InstalledError::OutOfScope { boundary, detail } => FlowError::OutOfScope {
            boundary: boundary.to_string(),
            detail,
        },
    })
}

/// `from_installed()`.
pub fn from_installed(
    profile: &InstalledProfile,
    provenance: &mut Provenance,
) -> Result<Vec<PyStr>, FlowError> {
    installed_metadata(profile, provenance).map(|(requires, _)| requires)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::http_direct::UreqDialer;
    use crate::zip_archive::write_stored_archive;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn serve_once(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        std::thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n")
                    && stream.read(&mut byte).map(|n| n == 1).unwrap_or(false)
                {
                    head.push(byte[0]);
                }
                let _ = stream.write_all(&response);
            }
        });
        format!("http://{address}/pkg.whl")
    }

    fn http_ok(body: &[u8]) -> Vec<u8> {
        let mut out = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        out.extend_from_slice(body);
        out
    }

    fn owned_base() -> (TempBase, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!(
            "flow-checks-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        (TempBase::fixed(root.clone()), root)
    }

    fn fetch(url: &str, base: &TempBase) -> (Result<(Vec<PyStr>, PyStr), FlowError>, Provenance) {
        let mut provenance = Provenance::default();
        let outcome = wheel_metadata(
            &PyValue::str(url),
            &FetchEnvironment::default(),
            &UreqDialer::unbounded(),
            base,
            &mut provenance,
        );
        (outcome, provenance)
    }

    #[test]
    fn a_wheel_over_http_yields_its_requires_and_version_and_leaves_no_temporary_directory() {
        let metadata = b"Metadata-Version: 2.3\nName: vllm\nVersion: 0.32.0\nRequires-Dist: transformers>=5.16.1,<5.20.0\nRequires-Dist: fastapi>=0.133.0\n\n";
        let archive = write_stored_archive(&[
            ("vllm/__init__.py", b""),
            ("vllm-0.32.0.dist-info/METADATA", metadata),
        ]);
        let url = serve_once(http_ok(&archive));
        let (base, root) = owned_base();
        let (outcome, provenance) = fetch(&url, &base);
        let (requires, version) = outcome.unwrap();
        assert_eq!(version.to_utf8().unwrap(), "0.32.0");
        assert_eq!(
            requires
                .iter()
                .map(|r| r.to_utf8().unwrap())
                .collect::<Vec<_>>(),
            ["transformers>=5.16.1,<5.20.0", "fastapi>=0.133.0"]
        );
        assert_eq!(provenance.temp_producers.len(), 1);
        assert!(
            provenance.temp_producers[0].created && provenance.temp_producers[0].cleanup_complete
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_wheel_without_one_metadata_member_or_with_a_bad_identity_is_the_runtime_error_of_the_original(
    ) {
        let (base, root) = owned_base();
        for archive in [
            write_stored_archive(&[("a.txt", b"x")]),
            write_stored_archive(&[
                ("a.dist-info/METADATA", b"Name: vllm\nVersion: 1\n"),
                ("b.dist-info/METADATA", b"Name: vllm\nVersion: 1\n"),
            ]),
        ] {
            let (outcome, _) = fetch(&serve_once(http_ok(&archive)), &base);
            let Err(FlowError::Raised { error, stage }) = outcome else {
                panic!("RuntimeError")
            };
            assert_eq!(
                (error.message.to_utf8().unwrap().as_str(), stage),
                (NO_UNIQUE_METADATA, Stage::ZipMember)
            );
        }
        let bad = write_stored_archive(&[("a.dist-info/METADATA", b"Name: other\nVersion: 1\n")]);
        let (outcome, _) = fetch(&serve_once(http_ok(&bad)), &base);
        let Err(FlowError::Raised { error, stage }) = outcome else {
            panic!("RuntimeError")
        };
        assert_eq!(
            (error.message.to_utf8().unwrap().as_str(), stage),
            (
                crate::requirements_rules::IDENTITY_FAILURE,
                Stage::WheelIdentity
            )
        );
        assert_eq!(
            std::fs::read_dir(&root).unwrap().count(),
            0,
            "every failing run still removed its directory"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_non_archive_a_failed_retrieval_and_a_non_utf8_member_each_fail_at_their_stage_with_cleanup(
    ) {
        let (base, root) = owned_base();
        let (outcome, _) = fetch(&serve_once(http_ok(b"not a zip")), &base);
        let Err(FlowError::Raised { error, stage }) = outcome else {
            panic!("BadZipFile")
        };
        assert_eq!(
            (
                error.class,
                error.message.to_utf8().unwrap().as_str(),
                stage
            ),
            ("BadZipFile", "File is not a zip file", Stage::ZipOpen)
        );
        let (outcome, provenance) = fetch("file:///definitely/absent.whl", &base);
        let Err(FlowError::Raised { error, stage }) = outcome else {
            panic!("URLError")
        };
        assert_eq!((error.class, stage), ("URLError", Stage::Urlretrieve));
        assert!(provenance.temp_producers[0].cleanup_complete);
        let invalid =
            write_stored_archive(&[("v.dist-info/METADATA", b"Name: vllm\nRequires-Dist: \xff\n")]);
        let (outcome, _) = fetch(&serve_once(http_ok(&invalid)), &base);
        let Err(FlowError::Raised { error, stage }) = outcome else {
            panic!("UnicodeDecodeError")
        };
        assert_eq!(
            (error.class, stage),
            ("UnicodeDecodeError", Stage::ZipMember)
        );
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), 0);
        std::fs::remove_dir_all(root).unwrap();
    }
}
