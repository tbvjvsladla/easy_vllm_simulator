//! `selftest_lite_gate`: the isolated check of the lite verdict (the entry gate of a full run) and of the lite-grade
//! certificate. A Rust port of `selftest_lite_gate.py` (plan_26092923; the trigger was D1 of 2026-09-29: a single-node
//! lite gave the in-container client a host port, measured nothing, returned rc 0 and printed an N/A table — a
//! measurement that did not stand was folded into a success).
//!
//! The deployed `lite_bench.sh`, `lite_metrics.py`, `render_report.py` and `publish_benchmark_record.py` run as
//! byte-for-byte copies in the same sandbox the lite report check uses ([`crate::selftest_lite_report::Sandbox`];
//! docker, curl and nvidia-smi are shims). The verdict vocabulary and the exit-code table are this crate's own
//! `lite_metrics` library (the original asked the native adapter module for them); the completion gate and the
//! campaign writer are the real current owners (the gate through [`crate::owner_bridge`], the writer as the deployed
//! `campaign_init.py` copy).
//!
//! Cases: K1 D1 connection refused (measurement path failed, exit 6) · K1b a reachable client plane does not turn a
//! connection error into a server failure · K2 α (no flag = automatic handoff after serving): nothing published, the
//! verdict reported · K3 server answered with failures (exit 7, no certificate) · K4 β pass + `--publish-report`: a
//! report and a lite-grade certificate with the same stem · K5 the publisher refuses a missing, out-of-vocabulary or
//! non-pass verdict (exit 3) · K6 the exit status is derived from the raw verdict · K7 native serving requires
//! `--engine-log` · K8 the β raw is kept apart from α · K9 campaign cell bench phase.

use crate::bench_verify_support::{
    dict_of, eq, int, py_len, py_str, st, tuple_text, BracketChecks, DetailStyle,
};
use crate::conformance::{py_equals, Raised};
use crate::dataset_prep::Console;
use crate::lite_metrics::read_lite_verdict;
use crate::owner_bridge::OwnerBridge;
use crate::pyvalue::{PyDict, PyValue};
use crate::selftest_lite_report::{Sandbox, AB, CFG};
use crate::verify_fixture::{
    chmod, contains, copy2, get, is_file, load_json, or_default, read_bytes, read_text,
    repo_root_unknown_message, which, write_str,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_lite_gate";

const WRITER_DIRECTORY: &str = ".claude/skills/terraforming_node/scripts";

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// `raw()` of the original: the last run's raw, or `None` when it cannot be read or parsed.
fn raw_of(sandbox: &Sandbox) -> PyValue {
    load_json(&sandbox.raw_path()).unwrap_or(PyValue::None)
}

fn clean_docs(sandbox: &Sandbox) {
    let _ = std::fs::remove_dir_all(sandbox.root().join("docs").join("benchmark"));
}

fn get_text(value: &PyValue, key: &str, text: &str) -> Result<bool, Raised> {
    Ok(py_equals(&get(value, key)?, &st(text)))
}

fn get_int(value: &PyValue, key: &str, number: i64) -> Result<bool, Raised> {
    Ok(py_equals(&get(value, key)?, &int(number)))
}

fn posix_repr(path: &Path) -> String {
    format!(
        "PosixPath({})",
        crate::pystr::PyStr::new(&path_str(path)).repr()
    )
}

fn paths_repr(paths: &[PathBuf]) -> String {
    format!(
        "[{}]",
        paths
            .iter()
            .map(|path| posix_repr(path))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn names_value(names: &[String]) -> PyValue {
    PyValue::List(names.iter().map(|name| st(name)).collect())
}

/// `x.get("key") or {}` applied to a state file that may not exist.
fn state_of(path: &Path) -> Result<PyValue, Raised> {
    if is_file(path)? {
        load_json(path)
    } else {
        Ok(dict_of(vec![]))
    }
}

/// The program. `args` are not read: the original ignores its arguments entirely.
#[allow(clippy::too_many_lines)]
pub fn main(repo_root: Option<&Path>, console: &mut dyn Console) -> Result<i32, Raised> {
    let mut checks = BracketChecks::new(console, DetailStyle::TailedWithDash);
    if which("git").is_none() {
        checks.write_error(&format!(
            "[{PROGRAM}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다\n"
        ));
        return Ok(1);
    }
    let Some(repo_root) = repo_root else {
        checks.write_error(&format!("{}\n", repo_root_unknown_message(PROGRAM)));
        return Ok(2);
    };
    let canonical = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let scripts_dir = canonical.join(AB);
    let mut bridge = OwnerBridge::start(&scripts_dir, checks.console())?;

    let mut sandbox = Sandbox::build(&canonical, &scripts_dir, "lite-gate-selftest.")?;
    let outcome = run_cases(
        &mut checks,
        &mut bridge,
        &mut sandbox,
        &canonical,
        &scripts_dir,
    );
    sandbox.close()?;
    outcome?;
    if !checks.failures().is_empty() {
        let line = format!(
            "[selftest_lite_gate] FAIL {} 건: {}\n",
            checks.failures().len(),
            checks.failure_list_repr()
        );
        checks.write_error(&line);
        return Ok(1);
    }
    checks.print(
        "[selftest_lite_gate] PASS — K1~K9(D1 연결거부 · ② · α 관측 · β lite 인증서 · 발행기 음성대조 · 종료코드 파생 · native 엔진 로그 · β/α raw 분리 · 캠페인 셀 phase)",
    )?;
    Ok(0)
}

#[allow(clippy::too_many_lines)]
fn run_cases(
    checks: &mut BracketChecks<'_>,
    bridge: &mut OwnerBridge,
    sandbox: &mut Sandbox,
    repo: &Path,
    scripts_dir: &Path,
) -> Result<(), Raised> {
    let docs = sandbox.root().join("docs/benchmark");
    let or_empty = |value: PyValue| or_default(value, PyValue::Dict(PyDict::new()));

    // ── K1 D1 connection refused ────────────────────────────────────────────────────────
    sandbox.set_env("SHIM_BENCH_MODE", "refused");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "unreachable");
    let run = sandbox.lite(&[])?;
    let raw = or_empty(raw_of(sandbox));
    let mut plane_named = false;
    for reason in crate::verify_fixture::items_or_empty(get(&raw, "lite_verdict_reasons")?)? {
        if contains(&reason, "클라이언트가 도는 평면")? {
            plane_named = true;
            break;
        }
    }
    let cond = run.status == 6
        && get_text(&raw, "lite_verdict", "measurement_path_failed")?
        && get_int(&raw, "lite_exit", 6)?
        && plane_named;
    checks.check(
        "K1★ D1 연결거부 → exit 6 · raw lite_verdict=measurement_path_failed · 사유에 클라이언트 평면 미도달",
        cond,
        || -> Result<String, Raised> { Ok(tuple_text(vec![
                int(run.status),
                get(&raw, "lite_verdict")?,
                get(&raw, "lite_verdict_reasons")?,
                st(&crate::verify_fixture::tail(&run.stderr, 400)),
            ])) },
    )?;
    let cond = get_int(&raw, "bench_cold_rc", 0)?
        && get_text(&raw, "host_health_after", "200")?
        && get_text(&raw, "client_plane_health", "unreachable")?
        && contains(
            &or_default(get(&raw, "bench_stderr_tail")?, st("")),
            "Cannot connect",
        )?;
    // Real shape: a connection refusal still exits 0 (the shape of D1).
    checks.check(
        "K1 판정 입력이 raw 에 실린다(요청 rc · 두 평면 health · bench 출력 끝)",
        cond,
        || raw.repr(),
    )?;
    checks.check(
        "K2★ α 불통과(플래그 없음) → 리포트·인증서 0(기록·보고만)",
        sandbox.reports().is_empty() && sandbox.certs().is_empty(),
        || {
            tuple_text(vec![
                names_value(&sandbox.reports()),
                names_value(&sandbox.certs()),
            ])
        },
    )?;
    let run = sandbox.lite(&["--publish-report"])?;
    let reports = sandbox.reports();
    let cond = run.status == 6
        && reports.len() == 1
        && sandbox.certs().is_empty()
        && read_text(&docs.join(&reports[0]))?
            .contains("| lite_verdict | measurement_path_failed |");
    checks.check(
        "K1★ D1 + --publish-report → exit 6 · 리포트 1(판정 절에 measurement_path_failed) · 인증서 0",
        cond,
        || {
            tuple_text(vec![
                int(run.status),
                names_value(&sandbox.reports()),
                names_value(&sandbox.certs()),
                st(&crate::verify_fixture::tail(&run.stderr, 400)),
            ])
        },
    )?;
    clean_docs(sandbox);

    // ── K1b a reachable client plane does not turn a connection error into a server failure ──
    sandbox.set_env("SHIM_BENCH_MODE", "refused");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&[])?;
    let raw = or_empty(raw_of(sandbox));
    let cond = run.status == 6 && get_text(&raw, "lite_verdict", "measurement_path_failed")?;
    checks.check(
        "K1b★ 연결 오류 실패 + 클라이언트 평면 200 → 여전히 ① · exit 6(② 로 접지 않는다)",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                get(&raw, "lite_verdict_reasons")?,
            ]))
        },
    )?;

    // ── K3 the server answered with failures ────────────────────────────────────────────
    sandbox.set_env("SHIM_BENCH_MODE", "http500");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&["--publish-report"])?;
    let raw = or_empty(raw_of(sandbox));
    let cond = run.status == 7
        && get_text(&raw, "lite_verdict", "server_failed")?
        && sandbox.certs().is_empty();
    checks.check(
        "K3★ 서버가 실패로 응답(failed>0 · 출력 0) → exit 7 · server_failed · 인증서 0",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                get(&raw, "lite_verdict")?,
                get(&raw, "lite_verdict_reasons")?,
                names_value(&sandbox.certs()),
            ]))
        },
    )?;
    clean_docs(sandbox);

    // ── K2 · K4 pass ────────────────────────────────────────────────────────────────────
    sandbox.set_env("SHIM_BENCH_MODE", "ok");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&[])?;
    let raw = or_empty(raw_of(sandbox));
    let cond = run.status == 0
        && get_text(&raw, "lite_verdict", "pass")?
        && sandbox.reports().is_empty()
        && sandbox.certs().is_empty()
        && run.stdout.contains("lite_verdict: pass");
    checks.check(
        "K2★ α 통과(플래그 없음) → exit 0 · raw pass · 리포트·인증서 0 · 판정을 출력으로 보고",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                get(&raw, "lite_verdict_reasons")?,
                st(&crate::verify_fixture::tail(&run.stderr, 400)),
            ]))
        },
    )?;
    // K6: the exit status is derived from the raw verdict; reading the same raw twice does not split.
    let derived = match read_lite_verdict(&raw) {
        Some(verdict) => verdict.exit_code(),
        None => return Err(Raised::new("KeyError", "None")),
    };
    let cond = derived == run.status && py_equals(&int(run.status), &get(&raw, "lite_exit")?);
    checks.check(
        "K6 종료코드 = raw 판정에서 파생(LITE_EXIT) · 두 번 읽어도 같다",
        cond,
        || raw.repr(),
    )?;
    let run = sandbox.lite(&["--publish-report"])?;
    let certs = sandbox.certs();
    let reps = sandbox.reports();
    let certificate_text = if certs.len() == 1 {
        read_text(&docs.join(&certs[0]))?
    } else {
        String::new()
    };
    let (fields, parsed_ok) = if certificate_text.is_empty() {
        (dict_of(vec![]), false)
    } else {
        bridge.parse_flat_certificate(checks.console(), &certificate_text)?
    };
    checks.check(
        "K4 β 통과 + --publish-report → exit 0 · 리포트 1 · lite 등급 인증서 1",
        run.status == 0 && reps.len() == 1 && certs.len() == 1,
        || {
            tuple_text(vec![
                int(run.status),
                names_value(&reps),
                names_value(&certs),
                st(&crate::verify_fixture::tail(&run.stderr, 500)),
            ])
        },
    )?;
    let cond = parsed_ok
        && get_text(&fields, "benchmark_mode", "lite")?
        && get_text(&fields, "verdict", "not_applicable")?
        && get_text(&fields, "lite_verdict", "pass")?
        && get_text(&fields, "entry_path", "beta_lite_only_cell")?
        && get_text(&fields, "record_type", "benchmark_certificate")?;
    checks.check(
        "K4 인증서 = benchmark_mode lite · verdict not_applicable · lite_verdict pass · entry β · 게이트 파서 ok",
        cond,
        || tuple_text(vec![PyValue::Bool(parsed_ok), fields.clone()]),
    )?;
    let mut strong_keys_filled = true;
    for key in [
        "model",
        "gpu_model",
        "vllm_version",
        "topology",
        "tensor_parallel_size",
    ] {
        let value = get(&fields, key)?;
        let blank = matches!(value.peel(), PyValue::None)
            || py_equals(&value, &st(""))
            || py_equals(&value, &st("N/A"));
        if blank {
            strong_keys_filled = false;
            break;
        }
    }
    let cond = strong_keys_filled
        && contains(&fields, "quantization")?
        && get_text(&fields, "measured_node", "main")?
        && py_equals(
            &get(&fields, "measured_utc")?,
            &get(&or_empty(raw_of(sandbox)), "measured_utc")?,
        );
    checks.check(
        "K4 강한 6키가 채워진다(model·gpu_model·vllm_version·quantization·topology·tp) · 측정 노드 · 측정시각=raw",
        cond,
        || fields.repr(),
    )?;
    let cond = !certs.is_empty()
        && !reps.is_empty()
        && char_slice(
            &certs[0],
            "benchmark_".chars().count(),
            ".yaml".chars().count(),
        ) == char_slice(
            &reps[0],
            "bench_report_".chars().count(),
            ".md".chars().count(),
        );
    checks.check(
        "K4 인증서와 리포트가 같은 stem(발행기 바인딩 전제)",
        cond,
        || tuple_text(vec![names_value(&certs), names_value(&reps)]),
    )?;
    let cond = parsed_ok && bridge.certificate_run_key_present(checks.console(), &fields)?;
    checks.check(
        "K4 lite 인증서의 run key 가 풀린다(중복 판정·바인딩이 같은 키로 찾는다)",
        cond,
        || fields.repr(),
    )?;

    // ── K8 the β raw is not overwritten by α ────────────────────────────────────────────
    let publish_directory = sandbox.root().join("output/single/benchlog/lite_publish");
    let mut published: Vec<PathBuf> = Vec::new();
    if let Ok(listing) = std::fs::read_dir(&publish_directory) {
        for entry in listing.filter_map(Result::ok) {
            if let Ok(inner) = std::fs::read_dir(entry.path()) {
                for candidate in inner.filter_map(Result::ok) {
                    let name = candidate.file_name().to_string_lossy().into_owned();
                    if name.starts_with("lite_raw_") && name.ends_with(".json") {
                        published.push(candidate.path());
                    }
                }
            }
        }
    }
    published.sort();
    let beta_raw = published.last().cloned();
    let beta_bytes = match &beta_raw {
        Some(path) => read_bytes(path)?,
        None => Vec::new(),
    };
    let cond = match &beta_raw {
        None => false,
        Some(path) => {
            let named_publish = path
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .is_some_and(|name| name == "lite_publish");
            let document_bytes: &[u8] = if beta_bytes.is_empty() {
                b"{}"
            } else {
                &beta_bytes
            };
            let document = crate::pyjson::loads_text(&crate::pystr::PyStr::new(
                &String::from_utf8_lossy(document_bytes),
            ))
            .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
            named_publish
                && py_equals(
                    &get(&document, "measured_utc")?,
                    &get(&fields, "measured_utc")?,
                )
        }
    };
    checks.check(
        "K8 β(--publish-report) raw 는 lite_publish/<config>_<UTC>/ 에 있다(α 기본 자리와 다르다)",
        cond,
        || format!("({}, {})", paths_repr(&published), fields.repr()),
    )?;
    let run = sandbox.lite(&[])?;
    let after = sandbox.raw_path();
    let cond = run.status == 0
        && match &beta_raw {
            Some(path) => read_bytes(path)? == beta_bytes,
            None => false,
        }
        && after
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|name| name == "benchlog");
    checks.check(
        "K8★ 이어서 α(플래그 없음)를 돌려도 β raw 바이트가 그대로다(리포트·인증서의 입력 raw 보존)",
        cond,
        || {
            format!(
                "({}, {}, {})",
                run.status,
                beta_raw
                    .as_ref()
                    .map_or("None".to_string(), |path| posix_repr(path)),
                posix_repr(&after)
            )
        },
    )?;

    // ── K7 native serving requires the server log declaration ───────────────────────────
    let fake_vllm = sandbox.root().join("fake_vllm");
    write_str(&fake_vllm, "#!/bin/sh\nexit 0\n")?;
    chmod(&fake_vllm, 0o755)?;
    let native_arguments = |script: &str, extra: &[&str]| -> Vec<OsString> {
        let mut argv = vec![
            OsString::from("bash"),
            sandbox.script(script),
            OsString::from(CFG),
            OsString::from("--topology"),
            OsString::from("single"),
        ];
        argv.extend(extra.iter().map(OsString::from));
        argv
    };
    let vllm_text = path_str(&fake_vllm);
    let run = sandbox.run(native_arguments(
        "lite_bench.sh",
        &[
            "--backend",
            "openai",
            "--serve-plane",
            "native",
            "--host-endpoint",
            "http://127.0.0.1:9",
            "--client-vllm",
            &vllm_text,
        ],
    ))?;
    checks.check(
        "K7★ native · --engine-log 부재 → exit 2(인자 단계 · 조용한 ① ✗) · 사유를 말한다",
        run.status == 2 && run.stderr.contains("--engine-log"),
        || {
            tuple_text(vec![
                int(run.status),
                st(&crate::verify_fixture::tail(&run.stderr, 300)),
            ])
        },
    )?;
    // The deployed bytes of sweep_bench.sh (only its argument check is exercised).
    copy2(
        &scripts_dir.join("sweep_bench.sh"),
        &sandbox.root().join(AB).join("sweep_bench.sh"),
    )?;
    let run = sandbox.run(native_arguments(
        "sweep_bench.sh",
        &[
            "--serve-plane",
            "native",
            "--host-endpoint",
            "http://127.0.0.1:9",
            "--client-vllm",
            &vllm_text,
            "--backend",
            "openai",
            "--bench-budget-mib",
            "1024",
        ],
    ))?;
    checks.check(
        "K7★ sweep_bench native · --engine-log 부재 → exit 2(lite 레그 판정 입력 선언)",
        run.status == 2 && run.stderr.contains("--engine-log"),
        || {
            tuple_text(vec![
                int(run.status),
                st(&crate::verify_fixture::tail(&run.stderr, 300)),
            ])
        },
    )?;

    // ── K9 campaign cell bench phase (2026-10-17, plan_26101722 K5) ─────────────────────
    // A finished lite was read as phase=running and the forced transition dropped it (plan_26101719 E1). With
    // `--campaign-cell` the run records the phase itself; the writer is a deployed copy, the node comes from the
    // assignments.
    let writer_directory = sandbox.root().join(WRITER_DIRECTORY);
    for name in [
        "campaign_init.py",
        "campaign_template_validator.py",
        "campaign_init.sh",
        "campaign_template_validator.sh",
    ] {
        copy2(
            &repo.join(WRITER_DIRECTORY).join(name),
            &writer_directory.join(name),
        )?;
    }
    for (variable, binary) in [
        ("CAMPAIGN_READ_BIN", "campaign_init"),
        ("CAMPAIGN_VALIDATOR_BIN", "campaign_template_validator"),
    ] {
        let built = repo.join(".claude/skills/adversarial-benchmark/rust/target/release").join(binary);
        sandbox.set_env(variable, &built.to_string_lossy());
    }
    let campaign = sandbox.root().join("campaigns").join("camp-k9");
    std::fs::create_dir_all(campaign.join("cells"))
        .map_err(|error| Raised::os_error(&error, &crate::verify_fixture::path_text(&campaign)))?;
    let declaration = dict_of(vec![
        ("schema_version", int(1)),
        ("id", st("camp-k9")),
        ("plan_ref", st("docs/plan/p.md")),
        (
            "nodes",
            PyValue::List(vec![dict_of(vec![
                ("node_id", st("main")),
                ("role", st("main")),
                ("topology", st("single")),
            ])]),
        ),
        (
            "assignments",
            dict_of(vec![(
                "main",
                PyValue::List(vec![dict_of(vec![("cell", st("k9"))])]),
            )]),
        ),
        (
            "control_variables",
            dict_of(vec![
                ("model", st("m")),
                ("vllm_version", st("9.8.7")),
                ("topology", st("single")),
            ]),
        ),
    ]);
    write_str(
        &campaign.join("campaign.yaml"),
        &crate::verify_fixture::dumps(&declaration)?
            .to_utf8()
            .unwrap_or_default(),
    )?;
    write_str(
        &campaign.join("evidence_pointers.json"),
        &crate::verify_fixture::dumps(&dict_of(vec![("pointers", PyValue::List(vec![]))]))?
            .to_utf8()
            .unwrap_or_default(),
    )?;
    let active = sandbox.root().join("campaigns").join("ACTIVE");
    write_str(&active, "camp-k9\n")?;
    let phase = campaign
        .join("phases")
        .join("main")
        .join("bench.status.json");
    sandbox.set_env("SHIM_BENCH_MODE", "ok");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&["--campaign-cell", "k9"])?;
    let state = state_of(&phase)?;
    let proof = or_empty(get(&state, "proof")?);
    let cond = run.status == 0
        && get_text(&state, "state", "done")?
        && eq(&get(&proof, "ok")?, &PyValue::Bool(true))
        && py_str(&get(&or_empty(get(&state, "proof")?), "source")?).contains("lite_raw_")
        && get_text(&state, "cell_id", "k9")?;
    checks.check(
        "K9★ --campaign-cell + 통과 → bench phase done · proof ok · 출처 = raw · 노드 = 배정 파생 · exit 0",
        cond,
        || {
            tuple_text(vec![
                int(run.status),
                state.clone(),
                st(&crate::verify_fixture::tail(&run.stderr, 500)),
            ])
        },
    )?;
    sandbox.set_env("SHIM_BENCH_MODE", "http500");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&["--campaign-cell", "k9"])?;
    let state = state_of(&phase)?;
    let cell_status = campaign.join("cells").join("k9").join("cell.status.json");
    let cell_state = state_of(&cell_status)?;
    let charges = or_default(
        get(&or_empty(get(&cell_state, "reconciliation")?), "charges")?,
        PyValue::List(vec![]),
    );
    let cond = run.status == 7 && get_text(&state, "state", "failed")? && py_len(&charges)? == 1;
    checks.check(
        "K9★ --campaign-cell + ② → exit 7 그대로 · bench phase failed · 셀 상태 writer 가 cap 차감 1",
        cond,
        || -> Result<String, Raised> { Ok(tuple_text(vec![
                int(run.status),
                get(&state, "state")?,
                get(&cell_state, "reconciliation")?,
                st(&crate::verify_fixture::tail(&run.stderr, 500)),
            ])) },
    )?;
    write_str(&active, "_bootstrap\n")?;
    sandbox.set_env("SHIM_BENCH_MODE", "ok");
    sandbox.set_env("SHIM_CLIENT_HEALTH", "200");
    let run = sandbox.lite(&["--campaign-cell", "k9"])?;
    checks.check(
        "K9 ACTIVE=_bootstrap → writer no-op · 측정 종료코드 불변(0)",
        run.status == 0,
        || crate::verify_fixture::tail(&run.stderr, 300),
    )?;
    let run = sandbox.lite(&[])?;
    checks.check(
        "K9 플래그 없음 → phase 기록 시도 0(명시로만 켠다)",
        run.status == 0 && !run.stderr.contains("bench phase"),
        || crate::verify_fixture::tail(&run.stderr, 300),
    )?;

    // ── K5 negative controls of the publisher ───────────────────────────────────────────
    let publisher = sandbox.script("publish_benchmark_record.sh");
    let good = or_empty(raw_of(sandbox));
    type Mutation = fn(&PyValue) -> PyValue;
    let mutations: [(&str, Mutation); 3] = [
        ("판정 부재", |document| {
            crate::verify_fixture::without_key(document.clone(), "lite_verdict")
                .unwrap_or(PyValue::None)
        }),
        ("어휘 밖", |document| with_lite_verdict(document, "ok")),
        ("불통과", |document| {
            with_lite_verdict(document, "server_failed")
        }),
    ];
    for (label, mutate) in mutations {
        let bad = mutate(&good);
        let bad_path = sandbox.root().join("bad_raw.json");
        write_str(
            &bad_path,
            &crate::verify_fixture::dumps(&bad)?
                .to_utf8()
                .unwrap_or_default(),
        )?;
        clean_docs(sandbox);
        let run = sandbox.run(vec![
            OsString::from("bash"),
            publisher.clone(),
            OsString::from("--lite-raw-json"),
            bad_path.into_os_string(),
        ])?;
        let cond = run.status == 3 && sandbox.certs().is_empty();
        checks.check(
            &format!("K5★ {label} raw → 인증서 발행기 exit 3 · 인증서 0"),
            cond,
            || {
                tuple_text(vec![
                    int(run.status),
                    names_value(&sandbox.certs()),
                    st(&crate::verify_fixture::tail(&run.stderr, 300)),
                ])
            },
        )?;
    }
    let run = sandbox.run(vec![
        OsString::from("bash"),
        publisher,
        OsString::from("--lite-raw-json"),
        sandbox.raw_path().into_os_string(),
        OsString::from("--sweep-index"),
        OsString::from("x.json"),
    ])?;
    checks.check(
        "K5 --lite-raw-json 과 full 인자 혼합 → exit 2",
        run.status == 2,
        || crate::verify_fixture::tail(&run.stderr, 200),
    )?;
    Ok(())
}

/// `dict(good, lite_verdict=value)`: an existing key keeps its place.
fn with_lite_verdict(document: &PyValue, value: &str) -> PyValue {
    match document.peel() {
        PyValue::Dict(map) => {
            let mut copy = map.clone();
            copy.set("lite_verdict", st(value));
            PyValue::Dict(copy)
        }
        other => other.clone(),
    }
}

/// `text[start:-end]` by characters.
fn char_slice(text: &str, start: usize, end: usize) -> String {
    let characters: Vec<char> = text.chars().collect();
    if characters.len() < start + end {
        return String::new();
    }
    characters[start..characters.len() - end].iter().collect()
}
