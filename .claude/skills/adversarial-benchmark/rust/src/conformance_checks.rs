//! The check list of the `full ⊇ lite` conformance checker (`main` of the original).
//!
//! Why the invariant exists: lite alone measures cold TTFT, system RAM and per-node GPU use; full alone sweeps,
//! draws the roofline and decides a verdict. If the two were intersecting sets instead of full containing lite,
//! the metric columns of a model measured in lite and one measured in full could not be compared. The checks
//! below pin the containment: static reads of the sources (the structure carries the containment), and the real
//! installed owners (a report with and without a lite block, a certificate, the publication gates, the lite
//! verdict rule on the D1 connection-refused fixture).
//!
//! Output order is the order of the checks; the report is printed once, at the end ([`crate::conformance::emit`]).
//! Everything that can raise in the original raises here with the same class and message.

use crate::conformance::{
    dict_get, emit, equals_int, equals_text, equals_zero, find_position, join_value, py_equals,
    read, read_file_bytes, read_optional_strict, subscript, Context, Raised, CERT_LITE_FIELDS,
    LITE_METRIC_KEYS,
};
use crate::conformance_native::{native_lite_certificate_gates, native_value, LiteMetrics};
use crate::dataset_prep::Console;
use crate::pyjson::{decode_file_text, loads_text};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::sweep_meta_deployed::{caller_binding, DeployedProducer, ProducerRun};
use crate::verify_fixture::TemporaryDirectory;

/// The rows of the report: `(name, ok, detail)`, in check order.
#[derive(Default)]
struct Report {
    rows: Vec<PyValue>,
}

impl Report {
    /// `check(name, ok, detail)` with a text detail (shown only when the check fails).
    fn check(&mut self, name: &str, ok: bool, detail: &str) {
        self.check_with(name, ok, PyValue::str(detail));
    }

    /// `check(name, ok, detail)` with any value as the detail; the report printer accepts only text for a failed row.
    fn check_with(&mut self, name: &str, ok: bool, detail: PyValue) {
        self.rows.push(PyValue::Tuple(vec![
            PyValue::str(name),
            PyValue::Bool(ok),
            detail,
        ]));
    }

    fn value(&self) -> PyValue {
        PyValue::List(self.rows.clone())
    }
}

fn object<const N: usize>(items: [(&str, PyValue); N]) -> PyValue {
    let mut map = PyDict::new();
    for (key, value) in items {
        map.set(key, value);
    }
    PyValue::Dict(map)
}

fn empty_object() -> PyValue {
    PyValue::Dict(PyDict::new())
}

fn verdict_pass() -> PyValue {
    object([("verdict", PyValue::str("PASS"))])
}

/// The texts the native output checks produced; empty until their step has run (a step that failed leaves the
/// later ones empty, and the checks that read them fail too).
#[derive(Default)]
struct Rendered {
    lite: String,
    missing: String,
    certificate: String,
}

fn source(context: &Context, name: &str) -> Result<Option<String>, Raised> {
    read(context, &PyStr::new(name))
}

/// `main()`: runs every check and prints the report; the status is 0 when all passed, 1 otherwise. An exception
/// raised outside the guarded steps ends the run as in the original (the report is not printed then).
pub fn main(context: &Context, console: &mut dyn Console) -> Result<i32, Raised> {
    let mut report = Report::default();

    let sweep = source(context, "sweep_bench.sh")?;
    let render = source(context, "render_report.sh")?;
    let certificate = source(context, "publish_benchmark_record.sh")?;
    let lite_adapter = source(context, "lite_metrics_native.py")?;
    for (label, text) in [
        ("sweep_bench.sh", &sweep),
        ("render_report.sh", &render),
        ("publish_benchmark_record.sh", &certificate),
        ("lite_metrics_native.py", &lite_adapter),
    ] {
        report.check(&format!("파일 존재: {label}"), text.is_some(), "");
    }
    let (Some(sweep), Some(_), Some(_), Some(_)) = (sweep, render, certificate, lite_adapter)
    else {
        emit(&report.value(), console)?;
        return Ok(1);
    };

    // D. full runs lite (the structural carrier of the containment)
    report.check(
        "sweep_bench 가 lite_bench.sh 를 호출한다",
        sweep.contains("lite_bench.sh"),
        "호출이 없으면 full 은 lite 를 포함할 수 없다(병렬 목록 유지로 퇴행)",
    );

    // E. gate containment: a failed lite verdict ends the run before any GuideLLM level is called
    let gate_at = find_position(&sweep, "exit \"$GATE_RC\"");
    let mut level_at = find_position(&sweep, "run_bench.sh\" \"${RB_ARGS[@]}\"");
    if level_at < 0 {
        level_at = find_position(&sweep, "RB_ARGS=(");
    }
    report.check(
        "E sweep_bench 의 lite 게이트 종료가 GuideLLM 레벨 호출보다 **앞**에 있다",
        0 <= gate_at && gate_at < level_at,
        &format!("gate@{gate_at} level@{level_at} — 게이트가 레벨 뒤면 교집합 상태가 다시 생긴다"),
    );
    report.check(
        "E sweep_bench 가 lite 실패를 fail-soft 로 접지 않는다(옛 'lite truncated:' 경로 부재)",
        !sweep.contains("echo \"lite truncated:"),
        "",
    );
    report.check(
        "E sweep_bench 가 판정을 raw 단일 권위(read_lite_verdict)에서 읽고 게이트 불통과를 절삭 로그에 남긴다",
        sweep.contains("read_lite_verdict") && sweep.contains("lite gate failed"),
        "",
    );
    let lite_bench = source(context, "lite_bench.sh")?.unwrap_or_default();
    report.check(
        "E lite_bench 가 판정기(--judge)를 부르고 종료코드를 raw 판정에서 파생한다",
        lite_bench.contains("--judge")
            && lite_bench.contains("exit \"$LITE_RC\"")
            && lite_bench.contains("server_failed) LITE_RC=7"),
        "",
    );

    // The installed report, certificate and publication owners, through their launchers.
    let mut rendered = Rendered::default();
    if let Some(exception) =
        caught_as_exception(native_output_checks(context, &mut report, &mut rendered))?
    {
        report.check(
            "native report/certificate output and gate checks",
            false,
            exception.repr(),
        );
    }

    // E② wiring: every hop from the ② signal to the cell state and the explorer cap exists, and each hop reads the
    // verdict through the owner's rule (`read_lite_verdict`) instead of judging the value again.
    let broad_search = source(context, "broad_search.sh")?.unwrap_or_default();
    let mut classifier = source(context, "classify_cell_native.py")?.unwrap_or_default();
    report.check(
        "E② native 분류기 Python 호출 경로가 존재한다",
        !classifier.is_empty(),
        "",
    );
    // The classifier rule and its command line now live in the installed Rust owner.
    let owner_sources = crate::ospath::join_all(&context.skill_dir(), &["rust", "src"]);
    for name in ["classify_cell.rs", "classify_cell_cli.rs", "argv.rs"] {
        let owner = read_optional_strict(&crate::ospath::join_all(&owner_sources, &[name]))?;
        report.check(
            &format!("E② native 분류기 소유 소스 존재: {name}"),
            !owner.is_empty(),
            "",
        );
        classifier.push('\n');
        classifier.push_str(&owner);
    }
    let skills = context.skills_dir();
    let skill_source =
        |relative: &str| read_optional_strict(&crate::ospath::join_all(&skills, &[relative]));
    let campaign_init = skill_source("terraforming_node/scripts/campaign_init.py")?;
    let recipe = skill_source("vllm-recipe-explorer/recipe.py")?;
    report.check(
        "E② sweep_bench 가 native 서버 로그를 lite 레그에 넘긴다(없으면 native lite 는 늘 ①)",
        sweep.contains("LITE_PLANE_ARGS+=(--engine-log \"$NATIVE_ELOG\")"),
        "",
    );
    report.check(
        "E② classify_cell 이 이번 셀 lite raw 를 소유자 규칙으로 읽어 사유(lite_server_failed·lite_measurement_path_failed)로 옮긴다",
        classifier.contains("read_lite_verdict")
            && classifier.contains("\"lite_server_failed\"")
            && classifier.contains("--lite-raw"),
        "",
    );
    // The cell record is written by the native `bench_shell broad-cell`; the carried `lite_verdict` field is bound in
    // that owner's source (the caller only has to reach it through the launcher).
    let cell_writer = read_optional_strict(&crate::ospath::join_all(
        &owner_sources,
        &["bench_shell", "broad.rs"],
    ))?;
    report.check(
        "E② broad_search 가 lite raw 를 분류기·셀 기록·writer 에 넘긴다",
        broad_search.contains("_CLS_ARGS+=(--lite-raw \"$LITE_RAW_CELL\")")
            && broad_search.contains("_WARGS+=(--lite-raw \"$LITE_RAW_CELL\")")
            && broad_search.contains("bash \"$SDIR/bench_shell.sh\" broad-cell")
            && cell_writer
                .contains("cell.set(\"lite_verdict\", get(&classification, \"lite_verdict\")?)"),
        "",
    );
    report.check(
        "E② campaign_init writer 가 ② 에서 cap 차감(charges)·재발동 제안을 적고 결정 기록 문을 갖는다(cap 값은 적지 않는다)",
        campaign_init.contains("read_lite_verdict")
            && campaign_init.contains("\"charges\": charges")
            && campaign_init.contains("\"status\": \"proposed\"")
            && campaign_init.contains("--reentry-decide"),
        "",
    );
    report.check(
        "E② explorer 가 셀 상태 차감만큼 cap 을 줄이고 소진이면 트라이얼 없이 Model-C",
        recipe.contains("reconciliation")
            && recipe.contains("cap = cap_declared - _lite_spent")
            && recipe.contains("reconciliation_cap 소진"),
        "",
    );
    report.check(
        "E② lite_bench 는 native 에 --engine-log 를 요구하고 β raw 를 lite_publish/ 에 따로 둔다",
        lite_bench.contains("lite_publish/${CONFIG}_")
            && lite_bench.contains("native에는 --engine-log"),
        "",
    );

    // The lite verdict rule, through the installed lite metrics owner. Anything raised while loading it or judging
    // the fixture is a failed check (a verdict that cannot be given is not a pass).
    if let Some(exception) = caught_as_exception(lite_verdict_checks(context, &mut report))? {
        report.check("E 판정기 적재·픽스처 판정", false, exception.repr());
    }

    // A. sweep_bench hands the index assembly to the native producer, and the lite metric keys travel into the lite
    //    block that producer writes: the caller and the installed owner are read, and one real run is looked at.
    let producer_source = read_optional_strict(&crate::ospath::join_all(
        &owner_sources,
        &["assemble_sweep_meta.rs"],
    ))?;
    let binding = caller_binding(&sweep);
    report.check(
        "sweep_bench 가 index·meta 조립을 native 생성부 런처(assemble_sweep_meta.sh)로 호출하고 옛 python 생성부를 남기지 않는다",
        binding.is_ok(),
        binding.as_ref().err().map_or("", String::as_str),
    );
    let launcher_present = source(context, "assemble_sweep_meta.sh")?.is_some();
    report.check(
        "native index 생성부 런처·소유 소스가 설치돼 있다",
        launcher_present && !producer_source.is_empty(),
        "생성부가 없으면 full 산출물의 lite 블록을 만드는 주체가 없다",
    );
    let mut produced_lite = PyValue::None;
    if let Some(exception) = caught_as_exception(produced_lite_block(context, &mut produced_lite))?
    {
        report.check(
            "native index 생성부 실제 실행(lite 블록 포함)",
            false,
            exception.repr(),
        );
    }
    for key in LITE_METRIC_KEYS {
        report.check(
            &format!("sweep_index.lite 가 '{key}' 를 싣는다"),
            producer_source.contains(&format!("\"{key}\"")) && has_member(&produced_lite, key),
            "lite_metrics 산출 키가 full 산출물에 전달되지 않으면 열 결손",
        );
    }
    report.check(
        "native index 생성부가 lite 산정을 재구현하지 않고 소유자 함수를 직접 호출하며 raw 판정을 그대로 옮긴다",
        producer_source.contains("crate::lite_metrics::build(")
            && producer_source.contains("crate::lite_metrics::read_lite_verdict(")
            && !producer_source.contains("fn gen_tps_from_bench")
            && member_is_text(&produced_lite, "lite_verdict", PRODUCED_VERDICT),
        "산정식을 복제하면 두 벌이 되어 어긋난다(D4↔D6 계열)",
    );

    // B. the report renders the lite block, and warns when it is missing
    report.check(
        "render_report 가 lite 블록을 렌더한다",
        rendered.lite.contains("native-conformance-lite-table")
            && rendered.lite.contains("full ⊇ lite"),
        "",
    );
    report.check(
        "render_report 가 lite 결손을 경고한다",
        rendered.missing.contains("lite 미포함"),
        "결손을 조용히 넘기면 비교 불가 사실이 숨는다",
    );

    // C. the certificate carries the lite fields
    for field in CERT_LITE_FIELDS {
        report.check(
            &format!("인증서가 '{field}' 를 emit 한다"),
            rendered.certificate.contains(&format!("\n{field}: ")),
            "",
        );
    }

    let all_passed = emit(&report.value(), console)?;
    Ok(if all_passed { 0 } else { 1 })
}

/// The guarded block of the original: the publication gates, then the report with and without a lite block, then the
/// certificate. A step that raises leaves the texts of the later steps empty.
fn native_output_checks(
    context: &Context,
    report: &mut Report,
    rendered: &mut Rendered,
) -> Result<(), Raised> {
    let gates_hold = native_lite_certificate_gates(context)?;
    let sample = object([
        ("meta", empty_object()),
        ("levels", PyValue::List(Vec::new())),
        (
            "lite",
            object([("table", PyValue::str("native-conformance-lite-table"))]),
        ),
    ]);
    let mut call = object([
        ("index", sample.clone()),
        ("verdict", verdict_pass()),
        ("roofline", PyValue::None),
    ]);
    let build_report = PyValue::str("render_report.build_md");
    rendered.lite = native_value(context, &build_report, &call)?
        .to_utf8()
        .unwrap_or_default();
    if let PyValue::Dict(map) = &mut call {
        map.set(
            "index",
            object([
                ("meta", empty_object()),
                ("levels", PyValue::List(Vec::new())),
            ]),
        );
    }
    rendered.missing = native_value(context, &build_report, &call)?
        .to_utf8()
        .unwrap_or_default();
    let certificate_call = object([("index", sample), ("verdict", verdict_pass())]);
    rendered.certificate = native_value(
        context,
        &PyValue::str("publish_benchmark_record.build_yaml"),
        &certificate_call,
    )?
    .to_utf8()
    .unwrap_or_default();
    report.check(
        "E 인증서 발행기가 실제 beta/gamma lite gate를 지킨다(양성 포함)",
        gates_hold,
        "",
    );
    Ok(())
}

/// `except Exception as exc:` around a guarded step of the original: an exception raised by the step is handed to the
/// caller to be written into a check's detail, but `SystemExit` is not an `Exception` and passes through (`Err`), so it
/// ends `main` with nothing printed, as it ends the original.
fn caught_as_exception(outcome: Result<(), Raised>) -> Result<Option<Raised>, Raised> {
    match outcome {
        Ok(()) => Ok(None),
        Err(raised) if raised.is_system_exit() => Err(raised),
        Err(raised) => Ok(Some(raised)),
    }
}

/// The verdict the produced raw record carries: it must reach the index unchanged (the raw record is the one authority).
const PRODUCED_VERDICT: &str = "server_failed";

fn has_member(value: &PyValue, key: &str) -> bool {
    matches!(value.peel(), PyValue::Dict(map) if map.contains_key(key))
}

fn member_is_text(value: &PyValue, key: &str, text: &str) -> bool {
    matches!(value.peel(), PyValue::Dict(map)
        if map.get(key).is_some_and(|member| crate::pydyn::is_text(member, text)))
}

fn runtime_error(message: &str) -> Raised {
    Raised::new("RuntimeError", message)
}

/// One real run of the deployed producer on a private cell whose lite raw record is the D1 fixture (with its result
/// files made absolute and a stored verdict); `lite` is the block it wrote into the index.
fn produced_lite_block(context: &Context, lite: &mut PyValue) -> Result<(), Raised> {
    let scripts = crate::pyfs::os_string(context.scripts_dir())
        .map_err(|failure| Raised::from_failure(failure.into_failure()))?;
    let producer = DeployedProducer::bind_installed(
        std::path::Path::new(&scripts),
        "conformance_full_superset_lite",
    )
    .map_err(|raised| runtime_error(&raised.failure().message.to_utf8().unwrap_or_default()))?;
    let fixture = crate::ospath::join_all(
        &context.skill_dir(),
        &["fixtures", "lite_raw_d1_connection_refused.json"],
    );
    let bytes = read_file_bytes(&fixture)?;
    let text = decode_file_text(&bytes).map_err(|error| Raised::decoding(&bytes, &error))?;
    let mut raw = loads_text(&text)
        .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
    let fixture_dir = crate::ospath::dirname(&fixture);
    for key in ["bench_cold_json", "bench_warm_json"] {
        let joined = join_value(&fixture_dir, &subscript(&raw, key)?)?;
        if let PyValue::Dict(map) = &mut raw {
            map.set(key, PyValue::Str(joined));
        }
    }
    if let PyValue::Dict(map) = &mut raw {
        map.set("lite_verdict", PyValue::str(PRODUCED_VERDICT));
    }
    let directory = TemporaryDirectory::create("conformance-producer-")?;
    let outcome = (|| -> Result<PyValue, Raised> {
        let root = directory.path();
        let cell = crate::assemble_sweep_meta_cli::lay_out_cell(root, "none")
            .map_err(|message| runtime_error(&message))?;
        let raw_path = root.join("lite_raw.json");
        crate::verify_fixture::write_str(
            &raw_path,
            &raw.dumps_ascii()
                .map_err(|error| runtime_error(&error.to_string()))?,
        )?;
        let mut environment = cell.pairs();
        environment.push(("LITE_RAW", raw_path.to_string_lossy().into_owned()));
        let run = producer.run(&environment)?;
        if run.status != 0 {
            let shown = ProducerRun::text(&run.stderr)?
                .to_utf8()
                .unwrap_or_default();
            return Err(runtime_error(&format!(
                "생성부가 비정상 종료했다(rc={}): {}",
                run.status,
                crate::verify_fixture::tail(shown.trim(), 300)
            )));
        }
        let index =
            crate::verify_fixture::load_json(&root.join("sweep_x").join("sweep_index.json"))?;
        subscript(&index, "lite")
    })();
    directory.close()?;
    *lite = outcome?;
    Ok(())
}

fn is_none(value: &PyValue) -> bool {
    matches!(value.peel(), PyValue::None)
}

/// The verdict vocabulary, the reading rule and the D1 fixture judged through the installed lite metrics owner.
fn lite_verdict_checks(context: &Context, report: &mut Report) -> Result<(), Raised> {
    let lite = LiteMetrics::beside_adapter(context);
    let bindings = lite.load(context.modules())?;

    let expected_vocabulary = ["pass", "measurement_path_failed", "server_failed"];
    let vocabulary_matches = bindings.verdicts.len() == expected_vocabulary.len()
        && bindings
            .verdicts
            .iter()
            .zip(expected_vocabulary)
            .all(|(own, expected)| equals_text(own, expected));
    let expected_exit = object([
        ("pass", PyValue::int(0)),
        ("measurement_path_failed", PyValue::int(6)),
        ("server_failed", PyValue::int(7)),
    ]);
    report.check(
        "E 판정 어휘·종료코드 = pass 0 · measurement_path_failed 6 · server_failed 7",
        vocabulary_matches && py_equals(&bindings.exit, &expected_exit),
        "",
    );

    let rule_holds = is_none(&lite.read_lite_verdict(&PyValue::None)?)
        && is_none(&lite.read_lite_verdict(&empty_object())?)
        && is_none(&lite.read_lite_verdict(&object([("lite_verdict", PyValue::str("ok"))]))?)
        && equals_text(
            &lite.read_lite_verdict(&object([("lite_verdict", PyValue::str("pass"))]))?,
            "pass",
        );
    report.check(
        "E 판정 읽기 규칙: 부재·어휘 밖 = None(호출부는 불통과로 읽는다)",
        rule_holds,
        "",
    );

    let fixture = crate::ospath::join_all(
        &context.skill_dir(),
        &["fixtures", "lite_raw_d1_connection_refused.json"],
    );
    let bytes = read_file_bytes(&fixture)?;
    let text = decode_file_text(&bytes).map_err(|error| Raised::decoding(&bytes, &error))?;
    let mut d1 = loads_text(&text)
        .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
    // The result files named by the fixture are relative to the fixture directory (an excerpt of a real run:
    // connection refused with exit status 0, `completed` 0, `failed` N).
    let fixture_dir = crate::ospath::dirname(&fixture);
    for key in ["bench_cold_json", "bench_warm_json"] {
        let joined = join_value(&fixture_dir, &subscript(&d1, key)?)?;
        if let PyValue::Dict(map) = &mut d1 {
            map.set(key, PyValue::Str(joined));
        }
    }
    let looks_real = if equals_zero(&dict_get(&d1, "bench_cold_rc")?) {
        let loaded = lite.load_json(&subscript(&d1, "bench_cold_json")?)?;
        let loaded = if loaded.is_truthy() {
            loaded
        } else {
            empty_object()
        };
        equals_zero(&dict_get(&loaded, "completed")?)
    } else {
        false
    };
    report.check(
        "E D1 픽스처는 실물 모양이다(rc 0 인데 결과 파일이 전부 실패를 말한다 — rc 로는 못 가른다)",
        looks_real,
        "",
    );

    let verdict_of = |raw: &PyValue| -> Result<PyValue, Raised> {
        let built = lite.build(raw)?;
        lite.judge_lite(raw, &built)
    };
    let with = |key: &str, value: &str| -> PyValue {
        let mut copy = d1.clone();
        if let PyValue::Dict(map) = &mut copy {
            map.set(key, PyValue::str(value));
        }
        copy
    };

    let refused = verdict_of(&d1)?;
    let refused_ok = equals_text(
        &subscript(&refused, "lite_verdict")?,
        "measurement_path_failed",
    ) && equals_int(&subscript(&refused, "lite_exit")?, 6);
    report.check_with(
        "E D1 연결거부 픽스처 → measurement_path_failed · exit 6(하네스 결함 · 서버 실패로 접지 않는다)",
        refused_ok,
        refused,
    );

    let client_reachable = verdict_of(&with("client_plane_health", "200"))?;
    let client_ok = equals_text(
        &subscript(&client_reachable, "lite_verdict")?,
        "measurement_path_failed",
    );
    report.check_with(
        "E 음성대조: 같은 출력이라도 클라이언트 평면이 닿으면 연결 오류 문구가 ② 로 접히지 않는다(① 유지)",
        client_ok,
        client_reachable,
    );

    let host_dead = verdict_of(&with("host_health_after", "000"))?;
    let host_ok = equals_text(&subscript(&host_dead, "lite_verdict")?, "server_failed")
        && equals_int(&subscript(&host_dead, "lite_exit")?, 7);
    report.check_with(
        "E 호스트 평면도 죽었으면(측정 중 서버 사망) → server_failed · exit 7",
        host_ok,
        host_dead,
    );
    Ok(())
}
