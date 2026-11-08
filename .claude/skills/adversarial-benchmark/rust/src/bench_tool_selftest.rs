//! The original's `--self-test` (P1..P9): the classification rules on a fixed record, the refusal of malformed pin
//! records (written to a scratch file and read back) and the shipped pin file, each printed as `  [PASS] name` or
//! `  [FAIL] name detail`.

use crate::bench_tool_pin::{
    classify, default_pin_path, is_pin_error, load_pin, Classification, DEFAULT_TOOL, RC_ABSENT,
    RC_OK,
};
use crate::dataset_prep::Console;
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};

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

fn digests(items: &[&str]) -> PyValue {
    PyValue::List(items.iter().map(|item| text(item)).collect())
}

fn is_digest(classification: &Classification, expected: &str) -> bool {
    classification
        .observed_digest
        .as_ref()
        .is_some_and(|seen| seen.equals(expected))
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

fn classified(entry: &PyValue, observed: &PyValue) -> Result<Classification, Failure> {
    classify(entry, observed)
}

/// The scratch file of the P7 checks: `$TMPDIR` (or `/tmp`) and `_pin_selftest.json`.
fn scratch_pin_path() -> PyStr {
    let folder = std::env::var_os("TMPDIR").unwrap_or_else(|| "/tmp".into());
    let folder =
        PyStr::from_surrogateescape(std::os::unix::ffi::OsStrExt::as_bytes(folder.as_os_str()));
    // `os.path.join`: an empty folder leaves the bare name; nothing is normalised.
    let mut path = folder.clone();
    if !path.is_empty() && !path.ends_with_code_point(u32::from('/')) {
        path.push_str("/");
    }
    path.push_str("_pin_selftest.json");
    path
}

/// `_self_test()`: the exit status, 0 when every check passed.
pub fn self_test(console: &mut dyn Console) -> Result<i32, Failure> {
    let mut report = Report {
        console,
        failures: Vec::new(),
    };
    let entry = object(&[
        ("version", text("0.7.3")),
        ("image_ref", text("example/tool:v1")),
        ("digest_source", text("measured(x)")),
        ("index_digest", text("sha256:aaa")),
        (
            "platform_digests",
            object(&[
                ("linux/arm64", text("sha256:bbb")),
                ("linux/amd64", text("sha256:ccc")),
            ]),
        ),
    ]);

    let result = classified(&entry, &PyValue::None)?;
    report.check(
        "P1 이미지 부재 → RC_ABSENT(자동 pull ✗)",
        result.resolution.code() == RC_ABSENT && result.observed_digest.is_none(),
        "",
    )?;

    let result = classified(&entry, &digests(&["example/tool@sha256:aaa"]))?;
    report.check(
        "P2 인덱스 digest 일치 → OK·기록일치",
        result.resolution.code() == RC_OK
            && is_digest(&result, "sha256:aaa")
            && result.matches_record == Some(true),
        "",
    )?;

    let result = classified(&entry, &digests(&["example/tool@sha256:bbb"]))?;
    report.check(
        "P3 플랫폼(arm64) digest 일치 → OK",
        result.resolution.code() == RC_OK && is_digest(&result, "sha256:bbb"),
        "",
    )?;

    let result = classified(&entry, &digests(&["example/tool@sha256:ccc"]))?;
    report.check(
        "P4 다른 플랫폼(amd64) digest 도 기록 안이면 일치(아키텍처는 docker 가 강제한다)",
        result.resolution.code() == RC_OK && result.matches_record == Some(true),
        "",
    )?;

    let result = classified(&entry, &digests(&["example/tool@sha256:dead"]))?;
    report.check(
        "P5 ★기록 밖 digest 도 **막지 않는다** — 관측을 그대로 싣고 불일치를 표시한다(버전 갱신이 하네스 수정을 부르지 않는다)",
        result.resolution.code() == RC_OK
            && is_digest(&result, "sha256:dead")
            && result.matches_record == Some(false),
        "",
    )?;

    let result = classified(&entry, &digests(&[]))?;
    report.check(
        "P6 RepoDigests 가 비면 관측 digest 는 없음(로컬 빌드 등) — 그 사실만 남는다",
        result.resolution.code() == RC_OK
            && result.observed_digest.is_none()
            && result.matches_record == Some(false),
        "",
    )?;

    let bare = object(&[("image_ref", text("x")), ("version", text("9"))]);
    let result = classified(&bare, &digests(&["x@sha256:zzz"]))?;
    report.check(
        "P6b 기록에 digest 가 아예 없으면 대조 자체가 성립하지 않는다(None)",
        result.matches_record.is_none(),
        "",
    )?;

    let broken_records: [(PyValue, &str); 3] = [
        (object(&[("tools", object(&[]))]), "도구"),
        (
            object(&[(
                "tools",
                object(&[("t", object(&[("versions", object(&[]))]))]),
            )]),
            "비었다",
        ),
        (
            object(&[(
                "tools",
                object(&[(
                    "t",
                    object(&[
                        ("default_version", text("1")),
                        (
                            "versions",
                            object(&[(
                                "1",
                                object(&[("image_ref", text("a")), ("index_digest", text("x"))]),
                            )]),
                        ),
                    ]),
                )]),
            )]),
            "sha256:",
        ),
    ];
    for (broken, fragment) in &broken_records {
        let path = scratch_pin_path();
        let name = format!("P7 불량 핀 거부({fragment})");
        let document = broken
            .dumps_ascii()
            .map_err(|error| Failure::new(error.python_type(), &error.to_string()))?;
        pyfs::write_text(&path, &PyStr::new(&document))?;
        match load_pin(&PyValue::Str(path.clone()), &text("t"), &PyValue::None) {
            Err(failure) if is_pin_error(&failure) => {
                let shown = failure.message.to_utf8().unwrap_or_default();
                report.check(
                    &name,
                    failure
                        .message
                        .to_utf8()
                        .is_some_and(|m| m.contains(fragment)),
                    &format!("(실제 {})", PyStr::new(&shown).repr()),
                )?;
            }
            Err(failure) => return Err(failure),
            Ok(_) => report.check(&name, false, "(예외가 나지 않았다)")?,
        }
        pyfs::unlink(&path)?;
    }

    let shipped = PyValue::Str(default_pin_path());
    match load_pin(&shipped, &text(DEFAULT_TOOL), &PyValue::None) {
        Ok(real) => {
            let image_is_ghcr = matches!(
                real.get("image_ref"),
                Some(PyValue::Str(image)) if image.code_points().starts_with(PyStr::static_points("ghcr.io/"))
            );
            let versioned = real.get("version").is_some_and(PyValue::is_truthy);
            report.check(
                "P8 배포 기록이 스키마를 만족한다(default_version 항목)",
                image_is_ghcr && versioned,
                "",
            )?;
        }
        Err(failure) if is_pin_error(&failure) => report.check(
            "P8 배포 기록이 스키마를 만족한다",
            false,
            &format!("({})", failure.message.to_utf8().unwrap_or_default()),
        )?,
        Err(failure) => return Err(failure),
    }
    match load_pin(&shipped, &text(DEFAULT_TOOL), &text("99.99.99")) {
        Ok(_) => report.check(
            "P9 스테이징하지 않은 버전 요청은 fail-loud",
            false,
            "(예외가 나지 않았다)",
        )?,
        Err(failure) if is_pin_error(&failure) => report.check(
            "P9 스테이징하지 않은 버전 요청은 fail-loud",
            failure
                .message
                .to_utf8()
                .is_some_and(|m| m.contains("스테이징하라")),
            "",
        )?,
        Err(failure) => return Err(failure),
    }

    if !report.failures.is_empty() {
        let names: Vec<PyValue> = report.failures.iter().map(|name| text(name)).collect();
        let mut line = PyStr::new(&format!(
            "[resolve_bench_tool --self-test] FAIL {} 건: ",
            names.len()
        ));
        line.push_str(&PyValue::List(names).repr());
        line.push_str("\n");
        report.console.error(&line);
        return Ok(1);
    }
    report.console.print(&PyStr::new(
        "[resolve_bench_tool --self-test] OK — P1~P9 전부 통과",
    ))?;
    Ok(0)
}
