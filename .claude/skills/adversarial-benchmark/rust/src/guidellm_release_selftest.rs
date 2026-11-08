//! The original's `--self-test` (T1..T9): the tag reader, the image reference, a latest-release resolution through a
//! built-in stand-in for the network, a record merge on a scratch file, and the shape of the shipped pin record, each
//! printed as `  [PASS] name` or `  [FAIL] name detail`.

use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::guidellm_release::{
    default_record_path, fetch_latest, image_ref, is_os_error, is_value_error, load_record,
    merge_record, resolution, version_from_tag, ReleaseRequest, ReleaseTransport, REPO_DEFAULT,
};
use crate::pydyn::{as_get_target, subscript};
use crate::pyfs;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use std::io::{Cursor, Read};

fn text(value: &str) -> PyValue {
    PyValue::str(value)
}

fn object(entries: &[(&str, PyValue)]) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in entries {
        map.set(key, value.clone());
    }
    PyValue::Dict(map)
}

fn is(value: &PyValue, expected: &str) -> bool {
    matches!(value.peel(), PyValue::Str(found) if found.equals(expected))
}

/// The stand-in for the opener of the original's T5: answers one fixed document.
struct FixedRelease(String);

impl ReleaseTransport for FixedRelease {
    fn open(&self, _request: &ReleaseRequest) -> Result<Box<dyn Read>, Failure> {
        Ok(Box::new(Cursor::new(self.0.clone().into_bytes())))
    }
}

struct Report<'a> {
    console: &'a mut dyn Console,
    failures: Vec<String>,
}

impl Report<'_> {
    /// `check(name, condition, detail="")`.
    fn check(&mut self, name: &str, condition: bool, detail: &str) -> Result<(), Failure> {
        if condition {
            self.console.print(&PyStr::new(&format!("  [PASS] {name}")))
        } else {
            self.failures.push(name.to_string());
            self.console
                .print(&PyStr::new(&format!("  [FAIL] {name} {detail}")))
        }
    }
}

/// A scratch folder under `$TMPDIR` (or `/tmp`), removed with everything in it when dropped
/// (`tempfile.TemporaryDirectory`).
struct ScratchFolder(std::path::PathBuf);

impl ScratchFolder {
    fn create() -> Result<ScratchFolder, Failure> {
        static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let base = std::env::temp_dir();
        loop {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.subsec_nanos());
            let serial = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate = base.join(format!("tmp{:x}{:x}{serial:x}", std::process::id(), nanos));
            match std::fs::create_dir(&candidate) {
                Ok(()) => return Ok(ScratchFolder(candidate)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => {
                    return Err(Failure::new(
                        crate::pyfs::os_error_class(&error),
                        &error.to_string(),
                    ))
                }
            }
        }
    }

    fn file(&self, name: &str) -> PyStr {
        PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(
            self.0.join(name).as_os_str(),
        ))
    }
}

impl Drop for ScratchFolder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// T9: the shipped record has the shape the merge relies on. Exceptions the original catches (`OSError`, `ValueError`,
/// `KeyError`, `TypeError`) fail the check with their text; others end the self-test.
fn shipped_record_shape() -> Result<(bool, String), Failure> {
    let outcome = (|| -> Result<bool, Failure> {
        let record = load_record(&default_record_path())?;
        let tools = subscript(&record, "tools")?;
        let guidellm = subscript(tools, "guidellm")?;
        let versions = as_get_target(guidellm)?.get("versions");
        let Some(PyValue::Dict(versions)) = versions.map(PyValue::peel) else {
            return Ok(false);
        };
        let default = as_get_target(guidellm)?
            .get("default_version")
            .cloned()
            .unwrap_or(PyValue::None);
        crate::pydyn::ensure_hashable(&default)?;
        Ok(match default.peel() {
            PyValue::Str(name) => versions.iter().any(|(key, _)| key == name),
            _ => false,
        })
    })();
    match outcome {
        Ok(found) => Ok((found, String::new())),
        Err(failure)
            if is_os_error(failure.class)
                || is_value_error(failure.class)
                || matches!(failure.class, "KeyError" | "TypeError") =>
        {
            Ok((
                false,
                format!("({})", failure.message.to_utf8().unwrap_or_default()),
            ))
        }
        Err(failure) => Err(failure),
    }
}

/// `_self_test()`: the exit status, 0 when every check passed.
pub fn self_test(console: &mut dyn Console) -> Result<i32, Failure> {
    let mut report = Report {
        console,
        failures: Vec::new(),
    };
    let default_repo = text(REPO_DEFAULT);

    report.check(
        "T1 태그 v0.7.3 → 0.7.3",
        version_from_tag(&text("v0.7.3")).is_ok_and(|v| v.equals("0.7.3")),
        "",
    )?;
    report.check(
        "T2 v 없는 태그도 읽는다",
        version_from_tag(&text("0.8.0")).is_ok_and(|v| v.equals("0.8.0")),
        "",
    )?;
    for bad in [
        text(""),
        PyValue::None,
        text("release-candidate"),
        text("v"),
        text("latest"),
    ] {
        let name = format!("T3 형태가 아니면 거부({})", bad.repr());
        match version_from_tag(&bad) {
            Ok(_) => report.check(&name, false, "(예외가 나지 않았다)")?,
            Err(failure) if is_value_error(failure.class) || failure.class == "TypeError" => {
                report.check(&name, true, "")?;
            }
            Err(failure) => return Err(failure),
        }
    }
    report.check(
        "T4 이미지 참조 조립",
        image_ref(&text("0.7.3"), &default_repo).equals("ghcr.io/vllm-project/guidellm:v0.7.3"),
        "",
    )?;

    // The latest release is read from the answer only (never guessed); the network is replaced by a fixed answer.
    let release = fetch_latest(
        &default_repo,
        &FixedRelease(
            "{\"tag_name\": \"v0.9.1\", \"published_at\": \"2026-08-30T12:00:00Z\"}".to_string(),
        ),
    )?;
    let tag = subscript(&release, "tag_name")?;
    let published = as_get_target(&release)?
        .get("published_at")
        .cloned()
        .unwrap_or(PyValue::None);
    let res = resolution(
        &PyValue::Str(version_from_tag(tag)?),
        &text("measured(GitHub Releases latest)"),
        &default_repo,
        &published,
    );
    let field = |map: &PyDict, key: &str| map.get(key).cloned().unwrap_or(PyValue::None);
    report.check(
        "T5 최신 릴리즈 해소 → 버전·이미지·출처",
        is(&field(&res, "version"), "0.9.1")
            && matches!(field(&res, "image_ref").peel(), PyValue::Str(r) if r.to_utf8().is_some_and(|r| r.ends_with(":v0.9.1")))
            && is(&field(&res, "published_utc"), "2026-08-30T12:00:00Z")
            && matches!(field(&res, "version_source").peel(), PyValue::Str(s) if s.to_utf8().is_some_and(|s| s.starts_with("measured("))),
        "",
    )?;
    let declared = resolution(
        &text("0.7.3"),
        &text("declared(--version)"),
        &default_repo,
        &PyValue::None,
    );
    report.check(
        "T6 선언 override 는 출처가 declared 다",
        is(&field(&declared, "version_source"), "declared(--version)")
            && matches!(field(&declared, "published_utc"), PyValue::None),
        "",
    )?;

    // The merge keeps what the record already holds.
    {
        let folder = ScratchFolder::create()?;
        let path = folder.file("rec.json");
        let start = object(&[
            ("schema_version", PyValue::int(1)),
            (
                "tools",
                object(&[(
                    "guidellm",
                    object(&[(
                        "versions",
                        object(&[(
                            "0.7.3",
                            object(&[
                                ("image_ref", text("ghcr.io/x:v0.7.3")),
                                ("index_digest", text("sha256:old")),
                            ]),
                        )]),
                    )]),
                )]),
            ),
        ]);
        let initial = start
            .dumps_ascii()
            .map_err(crate::dataset_prep::dump_failure)?;
        pyfs::write_bytes(&path, initial.as_bytes())?;
        merge_record(
            &PyValue::Str(path.clone()),
            &PyValue::Dict(res.clone()),
            &text("sha256:new"),
        )?;
        let document = load_record(&path)?;
        let versions = subscript(
            subscript(subscript(&document, "tools")?, "guidellm")?,
            "versions",
        )?;
        let versions_map = as_get_target(versions)?;
        report.check(
            "T7 기록 병합이 옛 버전을 지우지 않는다",
            versions_map.contains_key("0.7.3") && versions_map.contains_key("0.9.1"),
            "",
        )?;
        let default_version = subscript(
            subscript(subscript(&document, "tools")?, "guidellm")?,
            "default_version",
        )?;
        let digest = subscript(subscript(versions, "0.9.1")?, "index_digest")?;
        report.check(
            "T8 새 기본 버전과 digest 가 기록된다",
            is(default_version, "0.9.1") && is(digest, "sha256:new"),
            "",
        )?;
    }

    let (shaped, detail) = shipped_record_shape()?;
    report.check(
        "T9 배포 기록이 versions/default_version 형태다",
        shaped,
        &detail,
    )?;

    if report.failures.is_empty() {
        report.console.print(&PyStr::new(
            "[resolve_guidellm --self-test] OK — T1~T9 전부 통과",
        ))?;
        Ok(0)
    } else {
        let listed = PyValue::List(report.failures.iter().map(|name| text(name)).collect());
        let message = format!(
            "[resolve_guidellm --self-test] FAIL {} 건: {}\n",
            report.failures.len(),
            listed.repr()
        );
        report.console.error(&PyStr::new(&message));
        Ok(1)
    }
}
