//! The two self-tests the original programs carry, ported case for case:
//!
//! * `render_sweep_map.py --self-test`: checks R1–R18 of the exploration map (execution order, no ranking field
//!   at any depth, incomplete maps kept, repetition axis shown, the negative controls that plant a ranking key).
//! * `doc_naming.py` run without a command: the naming rules (KST tokens, slot union, collisions) and the
//!   directory scan against real files.
//!
//! Both return the transcript the original prints, so a run is compared by its text and exit status, not by a
//! printed "success".

use crate::bench_naming::{
    bench_filename, gpu_key, kst_tokens, measurement_ts, scan_bench_dir,
    NAMING_COLLISION_EXHAUSTED, NAMING_SOURCE_UNREADABLE,
};
use crate::guidellm::Failure;
use crate::pyjson::loads;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::render_sweep_map::{assert_no_ranking, build, render_markdown, FORBIDDEN_HEADERS};
use crate::report_support::Res;

/// What `render_sweep_map --self-test` printed and how it ended.
pub struct SweepMapReport {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// What the naming self-test printed, and the exception that ended it (an `AssertionError` or any other).
pub struct NamingReport {
    pub stdout: String,
    pub failure: Option<Failure>,
}

fn json(text: &str) -> PyValue {
    loads(text).expect("self-test document is JSON")
}

fn dict_set(value: &mut PyValue, path: &[&str], new: PyValue) {
    let PyValue::Dict(map) = value else {
        panic!("self-test path goes through objects")
    };
    match path {
        [last] => map.set(last, new),
        [first, rest @ ..] => {
            let mut inner = map.get(first).cloned().expect("self-test path exists");
            dict_set(&mut inner, rest, new);
            map.set(first, inner);
        }
        [] => unreachable!(),
    }
}

fn cell_at(doc: &mut PyValue, index: usize, key: &str, new: PyValue) {
    let PyValue::Dict(map) = doc else {
        panic!("document is an object")
    };
    let PyValue::List(cells) = map.get("cells").cloned().expect("cells") else {
        panic!("cells is a list")
    };
    let mut cells = cells;
    if let PyValue::Dict(cell) = &mut cells[index] {
        cell.set(key, new);
    }
    map.set("cells", PyValue::List(cells));
}

fn text_of(value: &PyStr) -> String {
    value.to_utf8().unwrap_or_default()
}

fn rank_error_has(result: Res<()>, needle: &str) -> bool {
    match result {
        Err(failure) => {
            failure.class == "RenderError" && text_of(&failure.message).contains(needle)
        }
        Ok(()) => false,
    }
}

/// The R1–R18 transcript of `render_sweep_map --self-test`.
pub fn run_sweep_map_self_test() -> SweepMapReport {
    let mut stdout = String::new();
    let mut failures: Vec<String> = Vec::new();
    let mut check = |name: &str, condition: bool, detail: &str| {
        if condition {
            stdout.push_str(&format!("  [PASS] {name}\n"));
        } else {
            stdout.push_str(&format!("  [FAIL] {name} {detail}\n"));
            failures.push(name.to_string());
        }
    };

    let state = json(
        r#"{"sweep_id":"bs_2026090416","rubric_authority":"explore","control_variable":"target HW: GB10 unified 120GiB","cells":[
        {"cell_key":"len131072-kvfp8","coordinates":{"max_model_len":131072},"cell_outcome":"measured","concurrency_vector":{"1":34.4,"2":60.1},"capacity":{"kv_gib":12.0},"verdict_narrative":"explore: E 대비 서술","measurement":{"bench_tool":"guidellm","bench_tool_version":"0.7.3"},"axis_citation":"wiki: kv-clamp-portability"},
        {"cell_key":"len262144-kvfp8","coordinates":{"max_model_len":262144},"cell_outcome":"serve_failed","note":"OOM","concurrency_vector":null,"capacity":null,"measurement":null}]}"#,
    );
    let stop = json(
        r#"{"sweep_status":"incomplete","stopped_by":["consecutive_failures"],"cells_planned":4,"cells_attempted":2,"cells_remaining":["c3","c4"],"consecutive_failures":1,"elapsed_s":1800,"declared_budget":{"max_cells":4,"wall_clock_budget_s":7200,"consecutive_failure_limit":3},"budget_declared_by":"hitl:2026-09-04","budget_basis":"직전 로드 실측"}"#,
    );
    let doc = build(&state, &stop).expect("fixture builds");
    let field = |value: &PyValue, key: &str| -> PyValue {
        match value {
            PyValue::Dict(map) => map.get(key).cloned().unwrap_or(PyValue::None),
            _ => PyValue::None,
        }
    };
    let cell = |value: &PyValue, index: usize| -> PyValue {
        match field(value, "cells") {
            PyValue::List(cells) => cells[index].clone(),
            _ => PyValue::None,
        }
    };
    let orders: Vec<PyValue> = match field(&doc, "cells") {
        PyValue::List(cells) => cells.iter().map(|c| field(c, "order")).collect(),
        _ => Vec::new(),
    };
    check(
        "R1 셀은 실행 순서로만 나열된다",
        orders == vec![PyValue::int(1), PyValue::int(2)]
            && field(&doc, "ordering") == PyValue::str("execution_order"),
        "",
    );
    check(
        "R2 순위 필드가 없다(자기 단언 통과)",
        assert_no_ranking(&doc, "$").is_ok(),
        "",
    );
    check(
        "R3 미완이 산출물에 남는다",
        field(&doc, "sweep_status") == PyValue::str("incomplete")
            && field(&doc, "cells_remaining") == json(r#"["c3","c4"]"#),
        "",
    );
    check(
        "R4 인용 누락 셀이 드러난다",
        field(&doc, "axis_citation_missing") == json(r#"["len262144-kvfp8"]"#),
        "",
    );
    check(
        "R5 동시성은 벡터 전부·용량은 나란히",
        field(&cell(&doc, 0), "concurrency_vector") == json(r#"{"1":34.4,"2":60.1}"#)
            && field(&cell(&doc, 0), "capacity") == json(r#"{"kv_gib":12.0}"#),
        "",
    );
    let markdown = text_of(&render_markdown(&doc).expect("fixture renders"));
    let lowered = markdown.to_lowercase();
    let has_ranking_header = |text: &str| {
        FORBIDDEN_HEADERS
            .iter()
            .any(|header| text.contains(&format!("| {}", header.to_lowercase())))
    };
    check(
        "R6 마크다운에 순위 열이 없다",
        !has_ranking_header(&lowered),
        "",
    );
    check(
        "R7 마크다운이 비순위 계약을 명시한다",
        markdown.contains("순위를 만들지 않는다"),
        "",
    );

    let mut poisoned = doc.clone();
    cell_at(&mut poisoned, 0, "rank", PyValue::int(1));
    let verdict = assert_no_ranking(&poisoned, "$");
    let planted = verdict.is_err();
    check(
        "R8 ★음성대조: 순위 필드를 심으면 거부",
        rank_error_has(verdict, "순위 필드 금지 위반"),
        if planted {
            ""
        } else {
            "(예외가 나지 않았다)"
        },
    );
    let mut poisoned_deep = doc.clone();
    dict_set(
        &mut poisoned_deep,
        &["summary"],
        json(r#"{"nested":{"best":"c1"}}"#),
    );
    let verdict = assert_no_ranking(&poisoned_deep, "$");
    let planted = verdict.is_err();
    check(
        "R9 ★음성대조: 깊이 무제한으로 잡는다",
        rank_error_has(verdict, "summary.nested.best"),
        if planted {
            ""
        } else {
            "(예외가 나지 않았다)"
        },
    );
    let mut value_only = doc.clone();
    cell_at(
        &mut value_only,
        0,
        "cell_key",
        PyValue::str("best-effort-len131072"),
    );
    check(
        "R10 값에 들어간 단어는 위반이 아니다(키만 본다)",
        assert_no_ranking(&value_only, "$").is_ok(),
        "",
    );

    let state2 = json(
        r#"{"sweep_id":"bs_rep","rubric_authority":"explore","control_variable":"fixture","cells":[
        {"cell_key":"full-a","cell_outcome":"measured","axis_citation":"wiki: x","concurrency_vector":{"1":30.0,"2":55.0},
         "repetition":{"requested":3,"requested_source":"declared(sweep state)","kind":"warm-rerun","completed_by_level":{"1":3,"2":3},"band_pct_by_level":{"1":0.6,"2":41.5},"band_formula":"(max-min)/mean*100","completed_min":3,"runs_attempted":6,"stop":null},
         "bench_mode":"full","bench_mode_source":"runs[] 집계(레벨별 완주 min=3 ≥ full 정의 3)","downgrade_reason":null},
        {"cell_key":"lite-b","cell_outcome":"measured","axis_citation":"wiki: y","concurrency_vector":{"1":28.0},
         "repetition":{"requested":3,"requested_source":"declared(sweep state)","kind":"warm-rerun","completed_by_level":{"1":1},"band_pct_by_level":{"1":null},"band_formula":"(max-min)/mean*100","completed_min":1,"runs_attempted":1,"stop":{"kind":"repeat-break","level":1,"run":2,"signal":"run_failed"}},
         "bench_mode":"lite","bench_mode_source":"runs[] 집계(레벨별 완주 min=1 < full 정의 3)","downgrade_reason":"blackbox_kill","downgrade_reason_source":"events(e.jsonl) · thermal_kill_ack"},
        {"cell_key":"legacy-c","cell_outcome":"measured","axis_citation":"wiki: z","bench_mode":null,"bench_mode_source":"not_evaluated(runs[] 부재 레벨 [1])"}]}"#,
    );
    let mut stop2 = stop.clone();
    dict_set(&mut stop2, &["cells_remaining"], json("[]"));
    dict_set(&mut stop2, &["sweep_status"], PyValue::str("complete"));
    dict_set(&mut stop2, &["runs_attempted"], PyValue::int(7));
    dict_set(
        &mut stop2,
        &["budget_repeats_source"],
        PyValue::str("declared(campaigns/camp-x/campaign.yaml budgets.repeats)"),
    );
    dict_set(
        &mut stop2,
        &["repeats_mismatch"],
        json(r#"[{"cell_key":"lite-b","requested":5,"declared_budget_repeats":3}]"#),
    );
    dict_set(&mut stop2, &["repeats_unrecorded"], json(r#"["legacy-c"]"#));
    let doc2 = build(&state2, &stop2).expect("fixture builds");
    let markdown2 = text_of(&render_markdown(&doc2).expect("fixture renders"));
    let tail: String = {
        let chars: Vec<char> = markdown2.chars().collect();
        chars[chars.len().saturating_sub(900)..].iter().collect()
    };
    check(
        "R11 반복 축이 레벨별 완주·재현 밴드로 실린다(밴드 41.5% 도 그대로 — 분산은 평가가 아니다)",
        markdown2.contains("\"1\": 3, \"2\": 3")
            && markdown2.contains("\"2\": 41.5")
            && markdown2.contains("bench_mode=`full`"),
        &tail,
    );
    check(
        "R12 반복 축 필드가 순위 금지 단언을 통과한다(키 이름이 판단을 담지 않는다 · 표에 순위 열 없음)",
        assert_no_ranking(&doc2, "$").is_ok() && !has_ranking_header(&markdown2.to_lowercase()),
        "",
    );
    check(
        "R13 강등된 lite 는 사유와 출처가 보인다",
        markdown2.contains("**강등** 사유 `blackbox_kill`"),
        "",
    );
    check(
        "R14 미완 표시 — 스윕이 멈춘 자리(반복 중단)가 이름으로 남는다",
        markdown2.contains("**스윕이 멈춘 자리 — 반복 중단**(level 1 run 2"),
        "",
    );
    check(
        "R15 기록 없는 셀은 '기록 없음 · 미확정' 이다(빈칸 ✗)",
        markdown2.contains("반복 축: 기록 없음 · bench_mode 미확정(`not_evaluated("),
        "",
    );
    check(
        "R16 예산과 다른 반복·기록 없는 측정 셀·run 합이 헤더에 기재된다",
        markdown2.contains("선언 예산과 다른 반복으로 잰 셀")
            && markdown2.contains("`legacy-c`")
            && markdown2.contains("| 레벨 run 시도 합 | 7 |"),
        "",
    );
    let mut poisoned_repetition = doc2.clone();
    let mut first = cell(&poisoned_repetition, 0);
    dict_set(&mut first, &["repetition", "top"], PyValue::str("full-a"));
    cell_at(
        &mut poisoned_repetition,
        0,
        "repetition",
        field(&first, "repetition"),
    );
    let verdict = assert_no_ranking(&poisoned_repetition, "$");
    let planted = verdict.is_err();
    check(
        "R17 ★음성대조: 반복 축 안에 순위 필드를 심어도 거부",
        rank_error_has(verdict, "repetition.top"),
        if planted {
            ""
        } else {
            "(예외가 나지 않았다)"
        },
    );
    let mut stop3 = stop2.clone();
    let mut budget = match field(&stop2, "declared_budget") {
        PyValue::Dict(map) => map,
        _ => PyDict::new(),
    };
    budget.set("repeats", PyValue::None);
    dict_set(&mut stop3, &["declared_budget"], PyValue::Dict(budget));
    dict_set(
        &mut stop3,
        &["budget_repeats_source"],
        PyValue::str("absent(fixture)"),
    );
    dict_set(&mut stop3, &["repeats_mismatch"], PyValue::None);
    let markdown3 = text_of(
        &render_markdown(&build(&state2, &stop3).expect("fixture builds")).expect("renders"),
    );
    check(
        "R18 ★반복 수 미선언 상태 → 지도 발행 · '예산 선언에 반복 수가 없다' 경고 · 출처 absent",
        markdown3.contains("예산 선언에 반복 수가 없다")
            && markdown3.contains("absent(fixture)")
            && !markdown3.contains("선언 예산과 다른 반복"),
        "",
    );

    if failures.is_empty() {
        stdout.push_str("[render_sweep_map --self-test] OK — R1~R18 전부 통과\n");
        SweepMapReport {
            stdout,
            stderr: String::new(),
            exit_code: 0,
        }
    } else {
        let shown: Vec<String> = failures
            .iter()
            .map(|name| PyStr::new(name).repr())
            .collect();
        SweepMapReport {
            stdout,
            stderr: format!(
                "[render_sweep_map --self-test] FAIL {} 건: [{}]\n",
                failures.len(),
                shown.join(", ")
            ),
            exit_code: 1,
        }
    }
}

// ── doc_naming self-test ──────────────────────────────────────────────────────

fn require(condition: bool, message: &str) -> Res<()> {
    if condition {
        Ok(())
    } else {
        Err(Failure::new("AssertionError", message))
    }
}

fn s(text: &str) -> PyValue {
    PyValue::str(text)
}

fn existing(entries: &[(&str, Option<&str>)]) -> PyValue {
    let mut map = PyDict::new();
    for (name, stamp) in entries {
        map.set(name, stamp.map_or(PyValue::None, s));
    }
    PyValue::Dict(map)
}

fn filename(kind: &str, meta: &PyValue, utc: &PyValue, existing: &PyValue) -> Res<String> {
    bench_filename(&s(kind), meta, utc, existing, &PyValue::None).map(|name| text_of(&name))
}

fn tokens(utc: &PyValue) -> (String, String, String) {
    let (a, b, c) = kst_tokens(utc);
    (text_of(&a), text_of(&b), text_of(&c))
}

/// Expects `result` to fail with `class`; a success is the self-test's `AssertionError(message)`.
fn expect_failure(result: Res<String>, class: &str, message: &str) -> Res<()> {
    match result {
        Err(failure) if failure.class == class => Ok(()),
        Err(failure) => Err(failure),
        Ok(_) => Err(Failure::new("AssertionError", message)),
    }
}

fn random_directory() -> Res<std::path::PathBuf> {
    use std::os::unix::fs::DirBuilderExt;
    const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789_";
    let base = std::env::temp_dir();
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64)
        ^ u64::from(std::process::id()).rotate_left(32);
    for _ in 0..100 {
        let mut name = String::from("tmp");
        for _ in 0..8 {
            seed = seed
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            name.push(ALPHABET[((seed >> 33) as usize) % ALPHABET.len()] as char);
        }
        let candidate = base.join(name);
        if std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&candidate)
            .is_ok()
        {
            return Ok(candidate);
        }
    }
    Err(Failure::new("FileExistsError", "[Errno 17] File exists"))
}

fn write_file(path: &std::path::Path, text: &str) -> Res<()> {
    std::fs::write(path, text).map_err(|error| {
        Failure::new(
            crate::pyfs::os_error_class(&error),
            &crate::cli::os_error_text(&error, &PyStr::new(&path.to_string_lossy())),
        )
    })
}

/// The transcript of the naming self-test. The temporary directory it makes is left behind, as the original does.
pub fn run_naming_self_test() -> NamingReport {
    let mut stdout = String::new();
    let failure = naming_self_test(&mut stdout).err();
    NamingReport { stdout, failure }
}

fn naming_self_test(stdout: &mut String) -> Res<()> {
    let t = |a: &str, b: &str, c: &str| (a.to_string(), b.to_string(), c.to_string());
    require(
        tokens(&s("2026-07-24T16:22:29Z")) == t("26072501", "22", "29"),
        "KST conversion 1",
    )?;
    require(
        tokens(&s("2026-07-15T11:00:33Z")) == t("26071520", "00", "33"),
        "KST conversion 2",
    )?;
    require(
        tokens(&s("2026-07-15T21:21:33Z")) == t("26071606", "21", "33"),
        "KST conversion 3",
    )?;
    require(
        tokens(&PyValue::None) == t("NA", "00", "00"),
        "None timestamp fallback",
    )?;
    require(
        tokens(&s("garbage")) == t("NA", "00", "00"),
        "invalid timestamp fallback",
    )?;
    require(
        tokens(&s("2026-99-99T99:99:99Z")) == t("NA", "00", "00"),
        "invalid calendar fallback (no raise)",
    )?;
    for (given, expected) in [
        (s("NVIDIA GB10"), "GB10"),
        (s("NVIDIA H100 80GB"), "H10080GB"),
        (s(""), "NA"),
        (PyValue::None, "NA"),
        (s("GB10"), "GB10"),
    ] {
        let actual = text_of(&gpu_key(&given));
        require(
            actual == expected,
            &format!("gpu_key {} -> {}", given.repr(), PyStr::new(&actual).repr()),
        )?;
    }

    let meta = json(r#"{"model":"solar-open2-250b","gpu_key":"GB10","vllm_version":"0.22.0"}"#);
    let (t1, t2, t3) = (
        s("2026-07-24T16:22:29Z"),
        s("2026-07-24T16:45:00Z"),
        s("2026-07-24T16:50:00Z"),
    );
    let (t1_text, t2_text) = ("2026-07-24T16:22:29Z", "2026-07-24T16:45:00Z");
    let base = "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md";
    let suffixed = "bench_report_26072501_45_00_solar-open2-250b_GB10_0.22.0.md";
    let none = PyValue::None;
    let report = |utc: &PyValue, existing: &PyValue| filename("bench_report", &meta, utc, existing);
    require(
        report(&t1, &none)? == base,
        "stdout 모드(existing 없음) = base",
    )?;
    require(report(&t1, &existing(&[]))? == base, "빈 디렉터리 = base")?;
    require(
        report(&t1, &existing(&[(base, Some(t1_text))]))? == base,
        "같은 측정 재발행 = 덮어쓰기(멱등)",
    )?;
    require(
        report(&t2, &existing(&[(base, Some(t1_text))]))? == suffixed,
        "다른 측정 같은 시간대 = _MM_SS",
    )?;
    require(
        report(
            &t2,
            &existing(&[(base, Some(t1_text)), (suffixed, Some(t2_text))]),
        )? == suffixed,
        "접미 이름의 같은 측정 재발행 = 멱등",
    )?;
    require(
        report(
            &t3,
            &existing(&[(base, Some(t1_text)), (suffixed, Some(t2_text))]),
        )? == "bench_report_26072501_50_00_solar-open2-250b_GB10_0.22.0.md",
        "세 번째 측정은 자기 MM_SS 로 간다(접미는 새 측정의 시각에서 파생)",
    )?;
    expect_failure(
        report(
            &t2,
            &existing(&[
                (base, Some(t1_text)),
                (suffixed, Some("2026-01-01T00:00:00Z")),
            ]),
        ),
        NAMING_COLLISION_EXHAUSTED,
        "두 형태 모두 다른 측정에 점유 → NamingCollisionExhausted 기대",
    )?;
    expect_failure(
        report(&t2, &existing(&[(base, None)])),
        NAMING_SOURCE_UNREADABLE,
        "★음성대조 측정시각을 못 읽는 후보는 덮어쓰기/접미가 아니라 raise",
    )?;
    require(
        filename("benchmark", &meta, &t1, &none)?
            == "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml",
        "cert ext",
    )?;
    require(
        filename("max_envelope", &meta, &PyValue::None, &none)?
            == "max_envelope_NA_solar-open2-250b_GB10_0.22.0.md",
        "NA token",
    )?;
    expect_failure(
        filename("nope", &meta, &t1, &none),
        "ValueError",
        "unknown kind must raise",
    )?;

    let directory = random_directory()?;
    let directory_text = PyValue::str(&directory.to_string_lossy());
    write_file(
        &directory.join(base),
        "# report\n생성일 2026-07-24T16:22:29Z.\n",
    )?;
    write_file(
        &directory.join("benchmark_x.yaml"),
        "measured_utc: \"2026-07-24T16:22:29Z\"\n",
    )?;
    write_file(&directory.join("bench_report_no_ts.md"), "nothing here\n")?;
    let scanned = scan_bench_dir(&directory_text, &none)?;
    let lookup = |name: &str| scanned.get(name).cloned();
    require(
        lookup(base) == Some(s(t1_text)) && lookup("benchmark_x.yaml") == Some(s(t1_text)),
        "scan 이 본문에서 측정시각을 뽑는다(따옴표 포함)",
    )?;
    require(
        lookup("bench_report_no_ts.md") == Some(PyValue::None),
        "시각 없는 파일은 None 으로 **표시**",
    )?;
    let filtered = scan_bench_dir(&directory_text, &s("benchmark"))?;
    require(
        filtered.len() == 1 && filtered.get("benchmark_x.yaml").is_some(),
        "kind 필터",
    )?;
    let absent = PyValue::str(&directory.join("absent").to_string_lossy());
    require(
        scan_bench_dir(&absent, &none)?.is_empty(),
        "부재 디렉터리 = {}",
    )?;

    let cert_base = "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml";
    let cert_suffixed = "benchmark_26072501_45_00_solar-open2-250b_GB10_0.22.0.yaml";
    require(
        report(&t1, &existing(&[(cert_base, Some(t1_text))]))? == base,
        "다른 kind base 가 **같은 측정** → base 로 짝 정렬",
    )?;
    require(
        report(&t2, &existing(&[(cert_base, Some(t1_text))]))? == suffixed,
        "★STEM_MISMATCH: 다른 kind base 가 다른 측정 → report 도 _MM_SS(옛 규칙은 base 반환)",
    )?;
    let certificate =
        |utc: &PyValue, existing: &PyValue| filename("benchmark", &meta, utc, existing);
    require(
        certificate(&t2, &existing(&[(base, Some(t2_text))]))? == cert_base,
        "report base 가 같은 측정 → cert 도 base 로 짝 정렬",
    )?;
    require(
        certificate(&t2, &existing(&[(base, Some(t1_text))]))? == cert_suffixed,
        "★STEM_MISMATCH 역방향: report base 가 다른 측정 → cert 도 _MM_SS",
    )?;
    require(
        report(&t2, &existing(&[(cert_suffixed, Some(t2_text))]))? == suffixed,
        "다른 kind 접미가 같은 측정 → 같은 접미로 짝 정렬",
    )?;
    require(
        report(&t3, &existing(&[(cert_suffixed, Some(t2_text))]))? == base,
        "base 슬롯이 비고 다른 측정의 짝이 접미에 있으면 base 사용 가능(측정별 stem 은 그대로 정합)",
    )?;
    expect_failure(
        report(
            &t2,
            &existing(&[
                (base, Some(t1_text)),
                (cert_suffixed, Some("2026-07-24T16:10:00Z")),
            ]),
        ),
        NAMING_COLLISION_EXHAUSTED,
        "접미 슬롯이 다른 kind 의 다른 측정에 점유 → Exhausted 기대",
    )?;
    expect_failure(
        report(&t2, &existing(&[(cert_base, None)])),
        NAMING_SOURCE_UNREADABLE,
        "★음성대조 다른 kind 점유자의 시각 불독도 raise",
    )?;
    require(
        filename(
            "max_envelope",
            &meta,
            &t2,
            &existing(&[(cert_base, Some(t1_text))]),
        )? == "max_envelope_26072501_45_00_solar-open2-250b_GB10_0.22.0.md",
        "세 번째 kind 도 같은 합집합 규칙",
    )?;
    let _ = measurement_ts(&PyValue::None)?;
    stdout.push_str("[doc_naming] self-test PASS\n");
    Ok(())
}
