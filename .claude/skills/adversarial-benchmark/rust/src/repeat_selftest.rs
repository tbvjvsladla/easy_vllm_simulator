//! The program's own self-test: the original `--self-test` scenarios A1 / B / P / R / G / H, run against this
//! implementation. Nothing here replays the original's transcript.
//!
//! * **A1** reads the runtime schema `campaigns/_template/campaign.schema.json` under the repository root and
//!   compares its `budgets.repeats.minimum` with the floor compiled in from that same schema at build time. A
//!   missing or reshaped schema is a failed check.
//! * **R** copies the actual campaign owner launcher (`campaign_init.sh`; the retired `campaign_init.py` may be absent) from the repository root into a fresh temporary tree and
//!   resolves repeat counts through it (the query adapter, see [`crate::repeat_owner`]); a missing owner copy source
//!   ends the self-test with the original's `FileNotFoundError`.
//! * **G** writes and rereads real files (run boundaries, `measured.json` aggregation, idempotence, failure
//!   injection). **H** and **B/P** are pure.

use crate::cli::TRACEBACK_HEADER;
use crate::guidellm::Failure;
use crate::pyfs;
use crate::pyjson::loads;
use crate::pynumber::round_to_places;
use crate::pysort::py_eq;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::repeat_axis::{
    band_pct, parse_repeats, repetition_established, summarize, SummarizeRequest,
    REPEAT_KIND_COLD, REPEAT_KIND_WARM, SCHEMA_REL, SIGNAL_RUN_FAILED,
    STOP_KIND_CLAMP, STOP_KIND_REPEAT_BREAK,
};
use crate::repeat_fs::{aggregate_level, record_run, AggregateRequest, RunBoundary};
use crate::repeat_owner::{resolve_repeats, CampaignOwner, RepoRoot, ResolveRequest};
use crate::FULL_REPEATS_MIN;
use num_bigint::BigInt;

/// What the self-test printed and how it ended.
#[derive(Debug)]
pub struct SelfTestReport {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

#[derive(Default)]
struct Recorder {
    stdout: String,
    failures: Vec<String>,
}

impl Recorder {
    /// `check(name, cond, detail)`: one `[PASS]`/`[FAIL]` line; a failure also names the detail.
    fn check(&mut self, name: &str, condition: bool, detail: impl FnOnce() -> String) {
        if condition {
            self.stdout.push_str(&format!("  [PASS] {name}\n"));
        } else {
            self.stdout
                .push_str(&format!("  [FAIL] {name} {}\n", detail()));
            self.failures.push(name.to_string());
        }
    }
}

fn no_detail() -> String {
    String::new()
}

/// The temporary directory of the R and G scenarios, removed when it goes out of scope (as Python's
/// `TemporaryDirectory` does, including when the self-test ends in an exception).
struct TempRoot {
    path: std::path::PathBuf,
}

impl TempRoot {
    fn create() -> Result<Self, Failure> {
        let base = std::env::temp_dir();
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        for attempt in 0..100u32 {
            let path = base.join(format!(
                "repeat-axis.{}-{stamp:x}-{attempt}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => return Ok(TempRoot { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(Failure::new(
                        crate::pyfs::os_error_class(&error),
                        &crate::cli::os_error_text(&error, &PyStr::new(&path.to_string_lossy())),
                    ))
                }
            }
        }
        Err(Failure::new(
            "FileExistsError",
            "no unused temporary directory name",
        ))
    }

    fn text(&self) -> PyStr {
        PyStr::new(&self.path.to_string_lossy())
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn json(text: &str) -> PyValue {
    loads(text).expect("self-test literal is valid JSON")
}

fn get(value: &PyValue, key: &str) -> PyValue {
    match value {
        PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    }
}

fn int(number: i64) -> PyValue {
    PyValue::int(number)
}

fn equals_text(value: &PyValue, text: &str) -> bool {
    matches!(value, PyValue::Str(own) if own.equals(text))
}

fn contains(text: &PyStr, needle: &str) -> bool {
    let haystack = text.to_utf8().unwrap_or_default();
    haystack.contains(needle)
}

fn is_none(value: &PyValue) -> bool {
    matches!(value, PyValue::None)
}

fn into_exit(result: Result<(), Failure>, mut recorder: Recorder) -> SelfTestReport {
    match result {
        Err(failure) => {
            // An exception the original does not catch: what was printed stays, then the traceback.
            let mut stderr = String::from(TRACEBACK_HEADER);
            stderr.push_str(&failure.line().to_utf8().unwrap_or_default());
            stderr.push('\n');
            SelfTestReport {
                stdout: std::mem::take(&mut recorder.stdout),
                stderr,
                exit_code: 1,
            }
        }
        Ok(()) if !recorder.failures.is_empty() => {
            let names: Vec<String> = recorder
                .failures
                .iter()
                .map(|name| PyStr::new(name).repr())
                .collect();
            SelfTestReport {
                stdout: std::mem::take(&mut recorder.stdout),
                stderr: format!(
                    "[repeat_axis --self-test] FAIL {} 건: [{}]\n",
                    recorder.failures.len(),
                    names.join(", ")
                ),
                exit_code: 1,
            }
        }
        Ok(()) => {
            recorder.stdout.push_str("[repeat_axis --self-test] OK — A1(schema 교차검증) · B1~B3 · P1~P2 · R1~R11 · G1~G9 · H1~H11 전부 통과\n");
            SelfTestReport {
                stdout: std::mem::take(&mut recorder.stdout),
                stderr: String::new(),
                exit_code: 0,
            }
        }
    }
}

/// Runs the self-test against the repository root's schema and owner.
pub fn run_self_test(repo_root: &RepoRoot, owner: &dyn CampaignOwner) -> SelfTestReport {
    let mut recorder = Recorder::default();
    let result = scenarios(&mut recorder, repo_root, owner);
    into_exit(result, recorder)
}

/// `expected RepeatAxisError`: `Ok(true)` when it was raised, `Ok(false)` when the call succeeded, and any other
/// exception propagates (the original's `except RepeatAxisError` does not catch it).
fn refused<T>(outcome: Result<T, Failure>) -> Result<bool, Failure> {
    match outcome {
        Ok(_) => Ok(false),
        Err(failure) if failure.class == "RepeatAxisError" => Ok(true),
        Err(failure) => Err(failure),
    }
}

fn scenarios(
    rec: &mut Recorder,
    repo_root: &RepoRoot,
    owner: &dyn CampaignOwner,
) -> Result<(), Failure> {
    let repo = repo_root.get()?.clone();

    // A1 — the schema minimum and the compiled-in floor are one concept.
    let schema = pyfs::load_json_or_none(&pyfs::join_text(&repo, &PyStr::new(SCHEMA_REL)))?;
    let minimum = ["definitions", "budgets", "properties", "repeats", "minimum"]
        .iter()
        .try_fold(schema.as_ref(), |node, key| match node {
            Some(PyValue::Dict(map)) => map.get(key).map(Some),
            _ => None,
        })
        .flatten()
        .cloned();
    let schema_floor = minimum.clone().unwrap_or(PyValue::None);
    rec.check(
        "A1 ★교차검증 campaign.schema.json budgets.repeats.minimum == FULL_REPEATS_MIN",
        minimum
            .as_ref()
            .is_some_and(|value| value.equals_int(&BigInt::from(FULL_REPEATS_MIN))),
        || {
            format!(
                "(schema={} · 상수={})",
                schema_floor.repr(),
                FULL_REPEATS_MIN
            )
        },
    );

    pure_band_and_value_domain(rec)?;
    let temp = TempRoot::create()?;
    resolution_through_owner(rec, &temp, &repo, owner)?;
    aggregation_and_summary(rec, &temp)?;
    stop_and_repetition_condition(rec)?;
    Ok(())
}

// ── B and P ─────────────────────────────────────────────────────────────────────

fn pure_band_and_value_domain(rec: &mut Recorder) -> Result<(), Failure> {
    let band = band_pct(&json("[11.759, 11.971, 11.301]"))?;
    rec.check(
        "B1 역채점: 11.759/11.971/11.301 → 5.74% (audit_26091323 §2.3 원장)",
        band.percent.is_some_and(|p| round_to_places(p, 2) == 5.74)
            && band.source.equals("measured(n=3)"),
        || band.to_value().repr(),
    );
    let band = band_pct(&json("[30.0]"))?;
    rec.check(
        "B2 완주 1회 → 밴드 없음(0 으로 적지 않는다)",
        band.percent.is_none()
            && contains(&band.source, "unavailable(n=1")
            && band
                .source
                .to_utf8()
                .is_some_and(|s| s.starts_with("unavailable(n=1")),
        no_detail,
    );
    let band = band_pct(&json("[30.0, null, NaN, 30.0]"))?;
    rec.check(
        "B3 결측·NaN 은 n 에서 빠진다",
        band.percent == Some(0.0) && band.source.equals("measured(n=2)"),
        || band.to_value().repr(),
    );

    let where_x = PyValue::str("x");
    for bad in [
        PyValue::str("2"),
        int(1),
        PyValue::str("abc"),
        PyValue::Bool(true),
        PyValue::Float(2.5),
        PyValue::str(""),
    ] {
        let rejected = refused(parse_repeats(&bad, &where_x))?;
        rec.check(
            &format!("P1 ★반복 수 {} 거부(full 정의 위반·비정수)", bad.repr()),
            rejected,
            no_detail,
        );
    }
    let accepted = parse_repeats(&int(3), &where_x)? == BigInt::from(3)
        && parse_repeats(&PyValue::str("5"), &where_x)? == BigInt::from(5);
    rec.check("P2 3·'5' 수용", accepted, no_detail);
    Ok(())
}

// ── R ───────────────────────────────────────────────────────────────────────────

fn write_declaration(camps: &PyStr, campaign: &str, document: &str) -> Result<(), Failure> {
    let path = pyfs::join(&pyfs::join(camps, campaign), "campaign.yaml");
    pyfs::write_text(&path, &PyStr::new(document))
}

fn resolution_through_owner(
    rec: &mut Recorder,
    temp: &TempRoot,
    repo: &PyStr,
    owner: &dyn CampaignOwner,
) -> Result<(), Failure> {
    let root = temp.text();
    let root_ref = RepoRoot::given(root.clone());
    let owner_copy = pyfs::join_text(&root, &PyStr::new(crate::repeat_owner::CAMPAIGN_LAUNCHER_REL));
    pyfs::mkdir_parents_exist_ok(&pyfs::parent(&owner_copy))?;
    // `ci_src.read_bytes()`: a missing owner in the repository is the original's FileNotFoundError.
    let owner_source = pyfs::read_bytes(&pyfs::join_text(repo, &PyStr::new(crate::repeat_owner::CAMPAIGN_LAUNCHER_REL)))?;
    pyfs::write_bytes(&owner_copy, &owner_source)?;
    let camps = pyfs::join(&root, "campaigns");
    pyfs::mkdir_parents_exist_ok(&pyfs::join(&camps, "camp-a"))?;

    // The lowest default is the preset's; the self-test asks through that path.
    let preset = |count: i64, name: &str| ResolveRequest {
        preset_default: int(count),
        preset_source: PyValue::str(name),
        ..ResolveRequest::default()
    };
    let bare = ResolveRequest::default();
    let declare = |budgets: Option<&str>| -> Result<(), Failure> {
        let document = match budgets {
            Some(budgets) => format!("{{\"id\": \"camp-a\", \"budgets\": {budgets}}}"),
            None => "{\"id\": \"camp-a\"}".to_string(),
        };
        write_declaration(&camps, "camp-a", &document)
    };

    let resolved = resolve_repeats(&preset(3, "default_preset"), &root_ref, owner)?;
    rec.check(
        "R1 ACTIVE 부재 → 프리셋 3 · defaulted(preset · _bootstrap)",
        resolved.count == BigInt::from(3)
            && contains(&resolved.source, "_bootstrap")
            && resolved
                .source
                .to_utf8()
                .is_some_and(|s| s.starts_with("defaulted(preset default_preset")),
        || resolved.source.repr(),
    );
    pyfs::write_text(&pyfs::join(&camps, "ACTIVE"), &PyStr::new("camp-a\n"))?;
    declare(Some("{\"smoke_budget_overhead_mib\": 1}"))?;
    let resolved = resolve_repeats(&preset(3, "default_preset"), &root_ref, owner)?;
    rec.check(
        "R2 선언 budgets.repeats 없음 → 3 · defaulted(미선언)",
        resolved.count == BigInt::from(3) && contains(&resolved.source, "미선언"),
        || resolved.source.to_utf8().unwrap_or_default(),
    );
    declare(Some("{\"repeats\": 5}"))?;
    let resolved = resolve_repeats(&preset(3, "default_preset"), &root_ref, owner)?;
    rec.check(
        "R3 선언 5 → 5 · declared(campaign.yaml)",
        resolved.count == BigInt::from(5)
            && resolved
                .source
                .equals("declared(campaigns/camp-a/campaign.yaml budgets.repeats)"),
        || resolved.source.to_utf8().unwrap_or_default(),
    );
    let argv_request = ResolveRequest {
        argv_value: PyValue::str("4"),
        argv_source: PyValue::str("sweep state"),
        ..ResolveRequest::default()
    };
    let resolved = resolve_repeats(&argv_request, &root_ref, owner)?;
    rec.check(
        "R4 argv 가 선언을 이긴다 · 출처에 argv",
        resolved.count == BigInt::from(4)
            && resolved
                .source
                .to_utf8()
                .is_some_and(|s| s.starts_with("declared(argv --repeats · sweep state")),
        || resolved.source.to_utf8().unwrap_or_default(),
    );
    declare(Some("{\"repeats\": 2}"))?;
    let outcome = refused(resolve_repeats(&bare, &root_ref, owner))?;
    rec.check(
        "R5 ★선언 2(검증기가 막았어야 할 값) → 거부(기본값 대체 ✗)",
        outcome,
        no_detail,
    );
    pyfs::write_text(
        &pyfs::join(&pyfs::join(&camps, "camp-a"), "campaign.yaml"),
        &PyStr::new("{broken"),
    )?;
    let outcome = refused(resolve_repeats(&bare, &root_ref, owner))?;
    rec.check(
        "R6 ★선언 파손 → 거부('미선언' 으로 접지 않는다)",
        outcome,
        no_detail,
    );
    declare(Some("{\"repeats\": 6}"))?;
    pyfs::mkdir_plain(&pyfs::join(&camps, "camp-b"))?;
    write_declaration(&camps, "camp-b", "{\"budgets\": {\"repeats\": 7}}")?;
    let by_id = ResolveRequest {
        campaign_id: PyValue::str("camp-b"),
        ..ResolveRequest::default()
    };
    let resolved = resolve_repeats(&by_id, &root_ref, owner)?;
    rec.check(
        "R7 --campaign-id 가 ACTIVE 보다 우선(상태 파일의 캠페인)",
        resolved.count == BigInt::from(7) && contains(&resolved.source, "camp-b"),
        || resolved.source.to_utf8().unwrap_or_default(),
    );
    let unbound_source = ResolveRequest {
        argv_source: PyValue::str("x"),
        ..ResolveRequest::default()
    };
    let outcome = refused(resolve_repeats(&unbound_source, &root_ref, owner))?;
    rec.check("R8 ★값 없는 --repeats-source 거부", outcome, no_detail);
    pyfs::unlink(&owner_copy)?;
    let resolved = resolve_repeats(&preset(3, "default_preset"), &root_ref, owner)?;
    rec.check(
        "R9 해소자 부재 → 프리셋 3 · 출처에 부재를 이름으로 남긴다(fail-loud 대체)",
        resolved.count == BigInt::from(3)
            && contains(&resolved.source, "해소자 부재")
            && contains(&resolved.source, "preset default_preset"),
        || resolved.source.to_utf8().unwrap_or_default(),
    );
    let outcome = refused(resolve_repeats(&bare, &root_ref, owner))?;
    rec.check(
        "R10 ★선언 없음 ∧ 프리셋 기본값 미전달 → 거부(코드 안 숫자로 메우지 않는다)",
        outcome,
        no_detail,
    );
    let outcome = refused(resolve_repeats(&preset(2, "bad"), &root_ref, owner))?;
    rec.check(
        "R11 ★프리셋 기본값 2(full 정의 미만) → 거부",
        outcome,
        no_detail,
    );
    Ok(())
}

// ── G ───────────────────────────────────────────────────────────────────────────

struct LevelDir {
    path: PyStr,
}

impl LevelDir {
    fn run_dir(&self, run: u32) -> PyStr {
        pyfs::join(&self.path, &format!("run_{run:02}"))
    }
}

fn boundary(
    level: i64,
    run: i64,
    started: &str,
    ended: &str,
    bench_rc: i64,
    parse_rc: i64,
) -> RunBoundary {
    RunBoundary {
        level: int(level),
        run: int(run),
        started_utc: PyValue::str(started),
        ended_utc: PyValue::str(ended),
        run_bench_rc: int(bench_rc),
        parse_rc: int(parse_rc),
    }
}

/// `json.dumps(document)` of the self-test's own dict.
fn dumps(document: &PyValue) -> PyStr {
    PyStr::new(
        &document
            .dumps_ascii()
            .expect("self-test documents are small"),
    )
}

fn aggregate(
    level: &LevelDir,
    requested: i64,
    source: &str,
    kind: &str,
) -> Result<PyValue, Failure> {
    aggregate_level(
        &PyValue::Str(level.path.clone()),
        &AggregateRequest {
            requested: &int(requested),
            requested_source: &PyValue::str(source),
            kind: &PyValue::str(kind),
        },
    )
}

fn summary_value(
    levels: &PyValue,
    requested: &PyValue,
    source: &PyValue,
    kind: &PyValue,
) -> Result<PyValue, Failure> {
    Ok(summarize(&SummarizeRequest {
        levels,
        requested,
        requested_source: source,
        kind,
        clamp_run: &PyValue::None,
    })?
    .to_value())
}

fn aggregation_and_summary(rec: &mut Recorder, temp: &TempRoot) -> Result<(), Failure> {
    let root = temp.text();
    let level = LevelDir {
        path: pyfs::join(&pyfs::join(&root, "sweep"), "level_01"),
    };
    pyfs::mkdir_parents_exist_ok(&level.path)?;
    let representative = json(
        "{\"decode_tps\": 30.0, \"accept_len\": 2.0, \"spec_axis_source\": \"inherited(w)\", \"measurement_ok\": true, \"completed\": 16, \"failed\": 0}",
    );
    let with = |key: &str, value: PyValue| -> PyValue {
        let PyValue::Dict(base) = &representative else {
            unreachable!("literal object")
        };
        let mut copy: PyDict = base.clone();
        copy.set(key, value);
        PyValue::Dict(copy)
    };
    pyfs::write_text(
        &pyfs::join(&level.path, "measured.json"),
        &dumps(&representative),
    )?;
    record_run(
        &PyValue::Str(level.path.clone()),
        &boundary(1, 1, "2026-01-01T00:00:00Z", "2026-01-01T00:01:00Z", 0, 0),
    )?;
    for (run, throughput) in [(2u32, 31.0), (3, 29.0)] {
        let dir = level.run_dir(run);
        pyfs::mkdir_plain(&dir)?;
        pyfs::write_text(
            &pyfs::join(&dir, "measured.json"),
            &dumps(&with("decode_tps", PyValue::Float(throughput))),
        )?;
        record_run(
            &PyValue::Str(dir),
            &boundary(
                1,
                i64::from(run),
                &format!("2026-01-01T00:0{run}:00Z"),
                &format!("2026-01-01T00:0{run}:30Z"),
                0,
                0,
            ),
        )?;
    }
    let doc = aggregate(&level, 3, "declared(x)", REPEAT_KIND_WARM)?;
    rec.check(
        "G1 대표 run 필드 그대로(decode_tps 30.0 · accept_len 2.0 · 평균으로 바꾸지 않는다)",
        py_eq(&get(&doc, "decode_tps"), &PyValue::Float(30.0))
            && py_eq(&get(&doc, "accept_len"), &PyValue::Float(2.0))
            && py_eq(&get(&doc, "representative_run"), &int(1)),
        no_detail,
    );
    let runs_len = match get(&doc, "runs") {
        PyValue::List(runs) => runs.len(),
        _ => 0,
    };
    rec.check(
        "G2 runs[] 3 · 완주 3 · 밴드 (31−29)/30×100 = 6.6667 · measured(n=3)",
        runs_len == 3
            && py_eq(&get(&doc, "repeats_completed"), &int(3))
            && py_eq(&get(&doc, "repro_band_pct"), &PyValue::Float(6.6667))
            && equals_text(&get(&doc, "repro_band_source"), "measured(n=3)"),
        || doc.repr(),
    );
    let doc2 = aggregate(&level, 3, "declared(x)", REPEAT_KIND_WARM)?;
    rec.check(
        "G3 재집계는 멱등(runs[] 가 중첩되지 않는다)",
        py_eq(&doc2, &doc),
        no_detail,
    );

    let level_one = |measured: &PyValue| -> PyValue {
        let mut entry = PyDict::new();
        entry.set("level", int(1));
        entry.set("measured", measured.clone());
        PyValue::List(vec![PyValue::Dict(entry)])
    };
    let s = summary_value(
        &level_one(&doc),
        &int(3),
        &PyValue::str("declared(x)"),
        &PyValue::str(REPEAT_KIND_WARM),
    )?;
    rec.check(
        "G4 요약: 완주 min 3 · 멈춘 자리 없음 · runs 3",
        py_eq(&get(&s, "completed_min"), &int(3))
            && is_none(&get(&s, "stop"))
            && py_eq(&get(&s, "runs_attempted"), &int(3))
            && !get(&s, "unrecorded_levels").is_truthy(),
        || s.repr(),
    );
    // Failure injection: run 3 broke (parsed fine, measurement_ok false).
    let run_three = level.run_dir(3);
    pyfs::write_text(
        &pyfs::join(&run_three, "measured.json"),
        &dumps(
            &with("measurement_ok", PyValue::Bool(false))
                .tap_set("decode_tps", PyValue::Float(5.0)),
        ),
    )?;
    let doc3 = aggregate(&level, 3, "declared(x)", REPEAT_KIND_WARM)?;
    let s3 = summary_value(
        &level_one(&doc3),
        &PyValue::None,
        &PyValue::None,
        &PyValue::None,
    )?;
    let stop3 = get(&s3, "stop");
    rec.check(
        "G5 ★run 3 실패 → 완주 2 · 밴드는 완주분만(n=2) · 멈춘 자리 repeat-break run 3",
        py_eq(&get(&doc3, "repeats_completed"), &int(2))
            && equals_text(&get(&doc3, "repro_band_source"), "measured(n=2)")
            && stop3.is_truthy()
            && py_eq(&get(&stop3, "run"), &int(3))
            && equals_text(&get(&stop3, "kind"), STOP_KIND_REPEAT_BREAK)
            && equals_text(&get(&stop3, "signal"), SIGNAL_RUN_FAILED),
        || PyValue::Tuple(vec![doc3.clone(), s3.clone()]).repr(),
    );
    rec.check(
        "G6 시각 대조 창 = 직전 run 시작 ~ 끊긴 run 끝",
        equals_text(&get(&stop3, "window_start_utc"), "2026-01-01T00:02:00Z")
            && equals_text(&get(&stop3, "window_end_utc"), "2026-01-01T00:03:30Z"),
        || stop3.repr(),
    );
    // A run_bench failure (no measurement file) gives the same signal.
    pyfs::unlink(&pyfs::join(&run_three, "measured.json"))?;
    record_run(
        &PyValue::Str(run_three),
        &boundary(1, 3, "2026-01-01T00:03:00Z", "2026-01-01T00:03:30Z", 3, 0),
    )?;
    let doc4 = aggregate(&level, 3, "declared(x)", REPEAT_KIND_WARM)?;
    let third = match get(&doc4, "runs") {
        PyValue::List(runs) => runs.get(2).cloned().unwrap_or(PyValue::None),
        _ => PyValue::None,
    };
    rec.check(
        "G7 ★run_bench rc=3(측정 파일 없음) → measurement_ok=false 로 기록 · 지표 None",
        matches!(get(&third, "measurement_ok"), PyValue::Bool(false))
            && is_none(&get(&third, "decode_tps"))
            && py_eq(&get(&third, "run_bench_rc"), &int(3)),
        no_detail,
    );
    let unrecorded = summary_value(
        &json("[{\"level\": 1, \"measured\": {\"measurement_ok\": true}}]"),
        &PyValue::None,
        &PyValue::None,
        &PyValue::None,
    )?;
    rec.check(
        "G8 runs[] 없는 레벨(반복 축 이전 산출물) → unrecorded · 완주 min None(추측 ✗)",
        py_eq(&get(&unrecorded, "unrecorded_levels"), &json("[1]"))
            && is_none(&get(&unrecorded, "completed_min")),
        no_detail,
    );
    let outcome = refused(aggregate(&level, 3, "x", REPEAT_KIND_COLD))?;
    rec.check(
        "G9 ★cold-restart 는 선언 슬롯만 — 집계 거부",
        outcome,
        no_detail,
    );
    Ok(())
}

/// A dict copy with one more assignment, for building the self-test's measurement variants.
trait TapSet {
    fn tap_set(self, key: &str, value: PyValue) -> PyValue;
}

impl TapSet for PyValue {
    fn tap_set(self, key: &str, value: PyValue) -> PyValue {
        match self {
            PyValue::Dict(mut map) => {
                map.set(key, value);
                PyValue::Dict(map)
            }
            other => other,
        }
    }
}

// ── H ───────────────────────────────────────────────────────────────────────────

/// `runs_(n_ok, n_fail, minute)` of the original: `n_ok + n_fail` runs of which the first `n_ok` completed.
fn runs_(completed: u32, broken: u32, minute: u32) -> String {
    let runs: Vec<String> = (1..=completed + broken)
        .map(|k| {
            format!(
                "{{\"run\": {k}, \"started_utc\": \"2026-01-01T01:{:02}:00Z\", \"ended_utc\": \"2026-01-01T01:{:02}:40Z\", \"run_bench_rc\": 0, \"parse_rc\": 0, \"measurement_ok\": {}, \"decode_tps\": 30.0}}",
                minute + k,
                minute + k,
                k <= completed
            )
        })
        .collect();
    format!("[{}]", runs.join(", "))
}

fn level_with(level: i64, runs: &str) -> String {
    format!("{{\"level\": {level}, \"measured\": {{\"measurement_ok\": true, \"runs\": {runs}}}}}")
}

fn summary_of(levels: &[String], clamp_run: Option<&PyValue>) -> Result<PyValue, Failure> {
    let levels = json(&format!("[{}]", levels.join(", ")));
    Ok(summarize(&SummarizeRequest {
        levels: &levels,
        requested: &PyValue::None,
        requested_source: &PyValue::None,
        kind: &PyValue::None,
        clamp_run: clamp_run.unwrap_or(&PyValue::None),
    })?
    .to_value())
}

fn stop_and_repetition_condition(rec: &mut Recorder) -> Result<(), Failure> {
    let clamp = json(
        "{\"level\": 2, \"run\": 1, \"started_utc\": \"2026-01-01T01:10:00Z\", \"ended_utc\": \"2026-01-01T01:10:20Z\", \"run_bench_rc\": 3, \"parse_rc\": 0}",
    );
    let s = summary_of(&[level_with(1, &runs_(3, 0, 0))], Some(&clamp))?;
    let stop = get(&s, "stop");
    rec.check(
        "H1 클램프(레벨 2 첫 run 실패) → stop.kind=clamp · 창 = 아래 레벨 마지막 run 시작 ~ 클램프 run 끝 · runs_attempted 에 클램프 run 1 포함",
        stop.is_truthy()
            && equals_text(&get(&stop, "kind"), STOP_KIND_CLAMP)
            && py_eq(&get(&stop, "level"), &int(2))
            && equals_text(&get(&stop, "window_start_utc"), "2026-01-01T01:03:00Z")
            && equals_text(&get(&stop, "window_end_utc"), "2026-01-01T01:10:20Z")
            && py_eq(&get(&s, "runs_attempted"), &int(4))
            && py_eq(&get(&s, "clamp_run"), &clamp),
        || s.repr(),
    );
    let unreadable = json("{\"level\": 2, \"run\": 1, \"unreadable\": \"repeat_run.json 부재\"}");
    let s = summary_of(&[level_with(1, &runs_(3, 0, 0))], Some(&unreadable))?;
    let stop = get(&s, "stop");
    rec.check(
        "H2 클램프 경계 사실 판독 불가 → 창 None(대조 불가를 분류기가 이름으로 남긴다 · 추측 ✗)",
        is_none(&get(&stop, "window_start_utc"))
            && matches!(get(&stop, "detail"), PyValue::Str(ref text) if contains(text, "판독 불가")),
        || stop.repr(),
    );

    let judged = |levels: &[String],
                  clamp: Option<&PyValue>,
                  judgment: PyValue|
     -> Result<(Option<bool>, PyStr), Failure> {
        let summary = summary_of(levels, clamp)?;
        let verdict = repetition_established(&summary, &judgment)?;
        Ok((verdict.established, verdict.reason))
    };

    let (established, why) = judged(
        &[
            level_with(1, &runs_(3, 0, 0)),
            level_with(2, &runs_(3, 0, 0)),
        ],
        None,
        int(1),
    )?;
    rec.check(
        "H3 판정점 3 · 경계 없음 → 반복 조건 성립",
        established == Some(true) && contains(&why, "완주 3"),
        || why.repr(),
    );
    // 2026-09-23 of the original: the source text must not look like a Markdown inline link.
    rec.check(
        "H3b 출처 문구가 마크다운 인라인 링크 모양(`](`)이 아니다",
        !contains(&why, "]("),
        || why.repr(),
    );
    let (established, why) = judged(
        &[
            level_with(1, &runs_(3, 0, 0)),
            level_with(2, &runs_(1, 1, 10)),
        ],
        None,
        int(1),
    )?;
    rec.check(
        "H4 ★비대칭 교정: 경계 레벨 2 의 run 2 실패(판정점 3) → 성립(적응 상한 클램프 · 첫 run 실패와 같은 판정)",
        established == Some(true) && contains(&why, "적응 상한 클램프"),
        || why.repr(),
    );
    let (established, why) = judged(&[level_with(1, &runs_(3, 0, 0))], Some(&clamp), int(1))?;
    rec.check(
        "H5 경계 레벨 2 의 첫 run 실패(판정점 3) → 성립(H4 와 같은 판정)",
        established == Some(true),
        || why.repr(),
    );
    let (established, why) = judged(&[level_with(1, &runs_(1, 1, 0))], None, int(1))?;
    rec.check(
        "H6 ★판정점 run 2 실패 → 불성립(False · 기계 이벤트)",
        established == Some(false) && contains(&why, "반복 중단"),
        || why.repr(),
    );
    let (established, why) = judged(&[level_with(1, &runs_(4, 1, 0))], None, int(1))?;
    rec.check(
        "H7 판정점 요청 5 · run 5 실패(완주 4) → 성립(정의 충족 · 상위 레벨 미측정 기재)",
        established == Some(true) && contains(&why, "판정점 level 1 run 5"),
        || why.repr(),
    );
    let (established, why) = judged(&[level_with(1, &runs_(2, 0, 0))], None, int(1))?;
    rec.check(
        "H8 ★끊김 없이 판정점 2 → 판정 불가(요청 자체가 정의 미만 · 사유 발명 ✗)",
        established.is_none()
            && why
                .to_utf8()
                .is_some_and(|s| s.starts_with("unclassifiable(")),
        || why.repr(),
    );
    let (established, why) = judged(
        &[
            level_with(1, &runs_(3, 0, 0)),
            level_with(2, &runs_(2, 0, 0)),
            level_with(4, &runs_(3, 0, 0)),
        ],
        None,
        int(1),
    )?;
    rec.check(
        "H9 ★경계가 아닌 레벨 2 가 정의 미만(스윕은 멈추지 않았다) → 판정 불가(규칙 밖 산출물)",
        established.is_none() && contains(&why, "레벨 [2]"),
        || why.repr(),
    );
    let (established, why) = judged(
        &["{\"level\": 1, \"measured\": {\"measurement_ok\": true}}".to_string()],
        None,
        int(1),
    )?;
    rec.check(
        "H10 runs[] 부재 → 판정 불가(not_evaluated)",
        established.is_none()
            && why
                .to_utf8()
                .is_some_and(|s| s.starts_with("not_evaluated(")),
        || why.repr(),
    );
    let (established, why) = judged(&[level_with(1, &runs_(3, 0, 0))], None, PyValue::None)?;
    rec.check(
        "H11 ★판정점 레벨 부재(verdict_point_level 없음) → 판정 불가(1 을 가정하지 않는다)",
        established.is_none() && contains(&why, "verdict_point_level"),
        || why.repr(),
    );
    Ok(())
}
