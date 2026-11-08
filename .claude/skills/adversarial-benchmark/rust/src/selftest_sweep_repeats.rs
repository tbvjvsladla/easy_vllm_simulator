//! `selftest_sweep_repeats`: the isolated run of the repeat axis of a full benchmark sweep (§4.4). A Rust port of
//! `selftest_sweep_repeats.py` (plan_26091407 §7 O4).
//!
//! The repeat loop, the downgrade signals and the `bench_mode` decision stand across the shell entry path:
//! `sweep_bench.sh` level loop → `repeat_axis` aggregation → `classify_cell` decision → `broad_search.sh` cell record
//! and stop evaluation. Pure-function self-tests cannot show that those calls are really on the chain, so this
//! program runs **byte copies of the deployed scripts and programs** in a private git checkout
//! ([`crate::sweep_repeats_fixture::Sandbox`]); nothing is re-implemented here. The load tools are shims (a legitimate
//! test-plane mock) that declare themselves fake; the failure injection is the shim plan.
//!
//! Real owners the checks reach: the deployed shell scripts and Rust programs inside the sandbox (judge, repeat axis,
//! parse, classify, report, publish, preset, broad search, campaign writer); the actual conformance program of the
//! installed tree (case G); and, **late and once**, the hint reader `render_bench_section.py` (case N4), which is the
//! one Python owner this program still depends on besides the shim text (see [`crate::owner_bridge`]). That reader is
//! imported under its own module name at the moment of the first report check, never earlier, and an `Exception` of
//! its import or parse is a failed check whose detail is the exception text, as in the original.
//!
//! Case families (★ = negative control): N normal path and judge/classify/report/publish contracts · P measurement
//! shape preset · R reassembly · G lite gate · F failure injection · V variance · A declaration violations · B broad
//! search wiring · O explicit new sweep paths · M multi-node. The groups of this file are N P R G F V A; B O M are
//! in [`crate::sweep_repeats_broad`]. Every check is evaluated as the original evaluates it: the condition and then the
//! detail, both eagerly, a failing detail propagating as the program's exception.

use crate::bench_verify_support::{
    dict_of, float, get_eq, get_eq_text, get_or_empty, int, is_none, py_len, py_str, py_strip, st,
    starts_with, tuple, BracketChecks, DetailStyle,
};
use crate::conformance::{iterate_values, py_equals, splitlines, Raised};
use crate::dataset_prep::Console;
use crate::owner_bridge::OwnerBridge;
use crate::pyjson::loads_text;
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::sweep_repeats_fixture::{Sandbox, CAMPAIGN, SKILL};
use crate::verify_fixture::{
    contains, detail_json_ascii, exists, get, is_file, or_default, read_text,
    repo_root_unknown_message, run_text_within, tail, words, write_str, TextRun,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_sweep_repeats";

/// The wall-clock limit of the conformance child (`timeout=120`).
const CONFORMANCE_SECONDS: u64 = 120;
const CONFORMANCE_SCRIPT: &str = "conformance_full_superset_lite.sh";
const HINT_READER_DIRECTORY: &str = ".claude/skills/hint-publisher/scripts";

pub(crate) type Outcome = Result<(), Raised>;

// ── Python evaluations of the conditions and details ────────────────────────────────

pub(crate) fn tup(items: Vec<PyValue>) -> PyValue {
    tuple(items)
}

pub(crate) fn strings(items: &[String]) -> PyValue {
    PyValue::List(items.iter().map(|item| st(item)).collect())
}

/// `text[-count:]` as a detail value.
pub(crate) fn tl(text: &str, count: usize) -> PyValue {
    st(&tail(text, count))
}

pub(crate) fn status(run: &TextRun) -> PyValue {
    int(run.status)
}

pub(crate) fn empty_object() -> PyValue {
    PyValue::Dict(PyDict::new())
}

/// `value.get(key, default)`: the default only for a missing key.
pub(crate) fn get_default(value: &PyValue, key: &str, default: PyValue) -> Result<PyValue, Raised> {
    let found = get(value, key)?;
    Ok(match value.peel() {
        PyValue::Dict(map) if !map.contains_key(key) => default,
        _ => found,
    })
}

/// `value.get(key) or {}`.
pub(crate) fn get_obj(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    get_or_empty(value, key)
}

/// `value.get(key) == "text"`.
pub(crate) fn is_text_at(value: &PyValue, key: &str, expected: &str) -> Result<bool, Raised> {
    get_eq_text(value, key, expected)
}

pub(crate) fn is_int_at(value: &PyValue, key: &str, expected: i64) -> Result<bool, Raised> {
    get_eq(value, key, &int(expected))
}

pub(crate) fn is_none_at(value: &PyValue, key: &str) -> Result<bool, Raised> {
    Ok(is_none(&get(value, key)?))
}

/// `str(value.get(key)).startswith(prefix)`.
pub(crate) fn str_at_starts(value: &PyValue, key: &str, prefix: &str) -> Result<bool, Raised> {
    Ok(py_str(&get(value, key)?).starts_with(prefix))
}

/// `needle in (value.get(key) or "")`.
pub(crate) fn contained_in_or_empty(
    needle: &str,
    value: &PyValue,
    key: &str,
) -> Result<bool, Raised> {
    contains(&or_default(get(value, key)?, st("")), needle)
}

/// `(value.get(key) or []) ` as a list value.
pub(crate) fn get_list(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    Ok(or_default(get(value, key)?, PyValue::List(Vec::new())))
}

/// `[lv.get("level") for lv in index.get("levels", [])]`.
pub(crate) fn level_numbers(index: &PyValue) -> Result<PyValue, Raised> {
    let levels = get_default(index, "levels", PyValue::List(Vec::new()))?;
    let mut numbers = Vec::new();
    for level in iterate_values(&levels)? {
        numbers.push(get(&level, "level")?);
    }
    Ok(PyValue::List(numbers))
}

/// `[c for c in calls if "level=" in c]`.
pub(crate) fn level_calls(calls: &[String]) -> Vec<String> {
    calls
        .iter()
        .filter(|call| call.contains("level="))
        .cloned()
        .collect()
}

fn lite_calls(calls: &[String]) -> usize {
    calls.iter().filter(|call| call.ends_with(" lite")).count()
}

pub(crate) fn int_list(numbers: &[i64]) -> PyValue {
    PyValue::List(numbers.iter().map(|number| int(*number)).collect())
}

/// `Sandbox.load(path) or {}`.
pub(crate) fn load_object(path: &Path) -> PyValue {
    or_default(Sandbox::load(path).unwrap_or(PyValue::None), empty_object())
}

/// `json.dumps(value)` as text.
pub(crate) fn json_line(value: &PyValue) -> Result<String, Raised> {
    detail_json_ascii(value)
}

/// `value > limit` for a loaded value (`int`, `float`, `bool`); anything else is the `TypeError` Python raises.
fn number_above(value: &PyValue, limit: i64) -> Result<bool, Raised> {
    use num_traits::ToPrimitive;
    let limit_value = i64_to_f64(limit);
    match value.peel() {
        PyValue::Bool(flag) => Ok(f64::from(u8::from(*flag)) > limit_value),
        PyValue::Int(number) => Ok(number.to_f64().is_none_or(|plain| plain > limit_value)),
        PyValue::Float(number) => Ok(*number > limit_value),
        other => Err(Raised::new(
            "TypeError",
            &format!(
                "'>' not supported between instances of '{}' and 'int'",
                other.type_name()
            ),
        )),
    }
}

fn i64_to_f64(number: i64) -> f64 {
    f64::from(i32::try_from(number).expect("a small literal limit"))
}

// ── the program state ─────────────────────────────────────────────────────────────────

/// The sandbox, the checks, and what one group leaves for a later one.
pub(crate) struct Program<'a, 'c> {
    pub checks: &'a mut BracketChecks<'c>,
    pub sb: &'a Sandbox,
    pub repo: &'a Path,
    pub scripts_dir: &'a Path,
    pub verbose: bool,
    /// N1: the sweep directory of `cfg-n`, its index and the first level's measurement.
    n_dir: PathBuf,
    n_index: PyValue,
    n_measured: PyValue,
    verdict_pass: PathBuf,
}

impl Program<'_, '_> {
    /// `ck(name, cond, detail)`.
    pub(crate) fn ck(&mut self, name: &str, cond: bool, detail: &PyValue) -> Outcome {
        self.checks.check(name, cond, || py_str(detail))
    }

    pub(crate) fn run(&self, command: Vec<OsString>) -> Result<TextRun, Raised> {
        self.sb.run(command, None)
    }

    pub(crate) fn sweep(
        &self,
        config: &str,
        extra: &[&str],
        plan: Option<&PyValue>,
        levels: &str,
    ) -> Result<TextRun, Raised> {
        self.sb.sweep(config, extra, plan, levels)
    }

    pub(crate) fn calls_value(&self) -> Result<PyValue, Raised> {
        Ok(strings(&self.sb.calls()?))
    }

    /// `side(cfg)`: the decision record next to the sweep (`bench_mode.json`), `{}` when unreadable.
    pub(crate) fn side(&self, config: &str) -> PyValue {
        load_object(&self.sb.sweep_dir(config, "single").join("bench_mode.json"))
    }

    pub(crate) fn index_of(&self, config: &str) -> PyValue {
        load_object(&self.sb.sweep_dir(config, "single").join("sweep_index.json"))
    }

    /// `bash <script> args...` in the sandbox.
    pub(crate) fn bash(&self, script: &str, arguments: Vec<OsString>) -> Result<TextRun, Raised> {
        let mut command = vec![OsString::from("bash"), self.sb.script(script)];
        command.extend(arguments);
        self.run(command)
    }

    fn report(&self, config: &str, extra: &[OsString]) -> Result<TextRun, Raised> {
        let mut arguments = vec![
            OsString::from("--sweep-index"),
            self.sb
                .sweep_dir(config, "single")
                .join("sweep_index.json")
                .into_os_string(),
            OsString::from("--verdict-json"),
            self.verdict_pass.clone().into_os_string(),
            OsString::from("--stdout"),
        ];
        arguments.extend(extra.iter().cloned());
        self.bash("render_report.sh", arguments)
    }

    fn publish(&self, config: &str) -> Result<TextRun, Raised> {
        self.bash(
            "publish_benchmark_record.sh",
            vec![
                OsString::from("--sweep-index"),
                self.sb
                    .sweep_dir(config, "single")
                    .join("sweep_index.json")
                    .into_os_string(),
                OsString::from("--verdict-json"),
                self.verdict_pass.clone().into_os_string(),
                OsString::from("--stdout"),
            ],
        )
    }

    /// `classify(cfg, events)`: the decision owner's command line on a finished sweep, with the given events file;
    /// the printed decision (`{}` when it is not JSON) and the run.
    pub(crate) fn classify(
        &self,
        config: &str,
        events: &[PyValue],
    ) -> Result<(PyValue, TextRun), Raised> {
        let mut arguments = vec![
            OsString::from("--sweep-index"),
            self.sb
                .sweep_dir(config, "single")
                .join("sweep_index.json")
                .into_os_string(),
        ];
        let events_file = self.sb.temporary().join(format!("events_{config}.jsonl"));
        let mut lines = String::new();
        for event in events {
            lines.push_str(&json_line(event)?);
            lines.push('\n');
        }
        crate::verify_fixture::write_str(&events_file, &lines)?;
        arguments.push(OsString::from("--events"));
        arguments.push(events_file.into_os_string());
        let run = self.bash("classify_cell.sh", arguments)?;
        let decision = loads_text(&PyStr::new(&run.stdout)).unwrap_or_else(|_| empty_object());
        Ok((decision, run))
    }
}

/// `bench_mode_line(md)`: the first line with `bench_mode: `, else `""`.
pub(crate) fn bench_mode_line(markdown: &str) -> String {
    splitlines(&PyStr::new(markdown))
        .iter()
        .map(|line| line.to_utf8().unwrap_or_default())
        .find(|line| line.contains("bench_mode: "))
        .unwrap_or_default()
}

// ── the program ──────────────────────────────────────────────────────────────────────

/// The program: `repo_root` is the installed tree the deployed files are copied from, `verbose` is `-v`.
pub fn main(
    repo_root: Option<&Path>,
    verbose: bool,
    console: &mut dyn Console,
) -> Result<i32, Raised> {
    let mut checks = BracketChecks::new(console, DetailStyle::TailedWithDashOf(700));
    if crate::verify_fixture::which("git").is_none() {
        checks.write_error(&format!(
            "[{PROGRAM}] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)\n"
        ));
        return Ok(1);
    }
    let Some(repo_root) = repo_root else {
        checks.write_error(&format!("{}\n", repo_root_unknown_message(PROGRAM)));
        return Ok(2);
    };
    let repo = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let scripts_dir = repo.join(SKILL).join("scripts");
    let sandbox = Sandbox::build(&repo)?;
    let outcome = run_groups(&mut checks, &sandbox, &repo, &scripts_dir, verbose);
    sandbox.close()?;
    outcome?;
    if !checks.failures().is_empty() {
        let line = format!(
            "[{PROGRAM}] FAIL {}건: {}\n",
            checks.failures().len(),
            checks.failure_list_repr()
        );
        checks.write_error(&line);
        return Ok(1);
    }
    checks.print(
        "[selftest_sweep_repeats] PASS — N1~N8 · P1~P3 · R1 · G(lite 게이트 5) · F1~F8 · V1 · A1~A4 · B1~B8 · O1~O6 · M1~M2 · R★(실패주입·음성대조 포함)",
    )?;
    Ok(0)
}

fn run_groups(
    checks: &mut BracketChecks<'_>,
    sandbox: &Sandbox,
    repo: &Path,
    scripts_dir: &Path,
    verbose: bool,
) -> Outcome {
    let mut program = Program {
        checks,
        sb: sandbox,
        repo,
        scripts_dir,
        verbose,
        n_dir: PathBuf::new(),
        n_index: empty_object(),
        n_measured: empty_object(),
        verdict_pass: sandbox.temporary().join("verdict_pass.json"),
    };
    program.shim_check()?;
    program.normal_path()?;
    program.reassembly_and_presets()?;
    program.lite_gate()?;
    program.failure_injection()?;
    program.declaration_violations()?;
    program.broad_search_wiring()?;
    program.new_output_paths()?;
    program.broad_cells_and_legacy_state()?;
    program.multi_node()?;
    Ok(())
}

impl Program<'_, '_> {
    fn shim_check(&mut self) -> Outcome {
        let sentinel = crate::sweep_repeats_fixture::SHIM_SENTINEL;
        let cond = read_text(&self.sb.scripts().join("run_bench.sh"))?.contains(sentinel)
            && read_text(&self.sb.scripts().join("lite_bench.sh"))?.contains(sentinel);
        self.ck(
            "사본의 부하 도구는 shim 이다(실제 run_bench/lite_bench 가 아니다)",
            cond,
            &st(""),
        )
    }

    /// N1–N8, A3 and N7.
    fn normal_path(&mut self) -> Outcome {
        let plan = dict_of(vec![(
            "tpot",
            dict_of(vec![
                (
                    "1",
                    PyValue::List(vec![float(25.0), float(24.0), float(26.0)]),
                ),
                (
                    "2",
                    PyValue::List(vec![float(30.0), float(30.0), float(30.0)]),
                ),
            ]),
        )]);
        let run = self.sweep("cfg-n", &[], Some(&plan), "1,2")?;
        let n1_stdout = run.stdout.clone();
        let directory = self.sb.sweep_dir("cfg-n", "single");
        let index = load_object(&directory.join("sweep_index.json"));
        let measured = load_object(&directory.join("level_01/measured.json"));
        if self.verbose {
            self.checks.print(&format!(
                "{} {}",
                tail(&run.stdout, 1500),
                tail(&run.stderr, 1500)
            ))?;
        }
        let calls = self.sb.calls()?;
        let cond = run.status == 0 && level_calls(&calls).len() == 6 && lite_calls(&calls) == 1;
        self.ck(
            "N1 rc=0 · 레벨 2 × 반복 3 = shim 6 호출 + lite 1 호출",
            cond,
            &tup(vec![status(&run), self.calls_value()?, st(&run.stderr)]),
        )?;
        let cond = is_file(&directory.join("level_01/measured.json"))?
            && is_file(&directory.join("level_01/run_02/measured.json"))?
            && is_file(&directory.join("level_02/run_03/measured.json"))?;
        self.ck(
            "N1 run 1 은 level_NN/ 에, run 2·3 은 level_NN/run_KK/ 에",
            cond,
            &st(""),
        )?;
        let cond = py_len(&get_list(&measured, "runs")?)? == 3
            && is_int_at(&measured, "repeats_completed", 3)?
            && is_text_at(&measured, "repro_band_source", "measured(n=3)")?
            && matches!(get(&measured, "repro_band_pct")?.peel(), PyValue::Float(_))
            && band_between(&get(&measured, "repro_band_pct")?, 7.0, 9.0)
            && is_text_at(&measured, "repeat_kind", "warm-rerun")?;
        self.ck(
            "N1 runs[] 3 · 완주 3 · 밴드 (1000/24−1000/26)/mean×100 · measured(n=3) · warm-rerun",
            cond,
            &measured,
        )?;
        let cond = get_eq(&measured, "decode_tps", &float(40.0))?
            && get_eq(&measured, "accept_len", &float(2.0))?
            && is_int_at(&measured, "representative_run", 1)?
            && is_text_at(&measured, "bench_tool_version", "0.0.0+selftest-shim")?;
        let fields = dict_of(vec![
            ("decode_tps", get(&measured, "decode_tps")?),
            ("accept_len", get(&measured, "accept_len")?),
            ("representative_run", get(&measured, "representative_run")?),
        ]);
        self.ck(
            "N1 대표 run 필드 그대로 — decode_tps=1000/25=40.0(평균 ✗) · accept_len 2.0(lite warm 승계) · 대표 run 1",
            cond,
            &fields,
        )?;
        let repetition = get_obj(&index, "repetition")?;
        let declared = format!("declared(campaigns/{CAMPAIGN}/campaign.yaml budgets.repeats)");
        let cond = is_int_at(&repetition, "requested", 3)?
            && is_text_at(&repetition, "requested_source", &declared)?
            && is_int_at(&repetition, "completed_min", 3)?
            && is_none_at(&repetition, "stop")?
            && is_int_at(&repetition, "runs_attempted", 6)?;
        self.ck(
            "N1 index.repetition — 요청 3 · 출처=캠페인 선언 · 레벨별 완주 min 3 · 중단 없음 · runs 6",
            cond,
            &repetition,
        )?;

        self.judge_inheritance()?;
        let sweep_side = self.side("cfg-n");
        let cond = is_text_at(&sweep_side, "bench_mode", "full")?
            && is_none_at(&sweep_side, "downgrade_reason")?
            && get_eq(
                &sweep_side,
                "sweep_index_generated_utc",
                &get(&index, "generated_utc")?,
            )?
            && is_text_at(&sweep_side, "downgrade_correlation", "not_applicable")?;
        self.ck(
            "N3 정규 경로 — sweep_bench 종료부가 판정 기록을 쓴다(bench_mode=full · 사유 null · index 측정시각에 묶임)",
            cond,
            &sweep_side,
        )?;
        self.ck(
            "N3 sweep_bench 로그에 판정이 보인다(부르기만 하고 판정하지 않는다)",
            n1_stdout.contains("[sweep_bench] bench_mode=full"),
            &tl(&n1_stdout, 600),
        )?;
        self.n_dir = directory;
        self.n_index = index;
        self.n_measured = measured;
        self.report_checks()?;
        self.certificate_check()?;
        self.damaged_decision_record()?;
        self.reassemble_inherits_request()?;
        self.rerun_does_not_mix_old_runs()
    }

    /// N2: the judge inherits the representative run's accept length.
    fn judge_inheritance(&mut self) -> Outcome {
        let out_dir = self.sb.temporary().join("rejudge");
        let run = self.bash(
            "judge_bench.sh",
            words(&[
                "cfg-n",
                "--topology",
                "single",
                "--authority",
                "explore",
                "--e-search",
                "empty",
                "--out-dir",
            ])
            .into_iter()
            .chain([out_dir.clone().into_os_string()])
            .collect(),
        )?;
        let roofline = load_object(&out_dir.join("roofline.json"));
        let cond = run.status == 0
            && is_text_at(&roofline, "accept_len_source", "measured")?
            && get_eq(&roofline, "accept_len", &float(2.0))?
            && starts_with(
                &or_default(get(&roofline, "accept_len_evidence")?, st("")),
                "level_01/measured.json",
            )?;
        self.ck(
            "N2 judge_bench 가 대표 run 의 accept_len 을 승계한다(accept_len_source=measured · 근거 level_01)",
            cond,
            &tup(vec![status(&run), roofline, tl(&run.stderr, 400)]),
        )
    }

    /// N4: the report's repeat section, and the hint reader's view of it.
    fn report_checks(&mut self) -> Outcome {
        let decode = get(&self.n_measured, "decode_tps")?;
        let rubric = dict_of(vec![
            ("authority", st("explicit")),
            ("source", st("c")),
            ("primary", float(30.0)),
            ("floor", float(27.0)),
            ("ratio_M_over_primary", float(1.33)),
            ("tolerance", float(0.1)),
        ]);
        let verdict = dict_of(vec![
            ("verdict", st("PASS")),
            ("measured_decode_tps", decode),
            ("rubric", rubric),
        ]);
        crate::verify_fixture::write_text(&self.verdict_pass, &PyStr::new(&json_line(&verdict)?))?;
        let run = self.report("cfg-n", &[])?;
        let markdown = run.stdout.clone();
        let cond = run.status == 0
            && markdown.contains("## 반복 축 · 재현 밴드")
            && markdown.contains("| 1 | 3/3 |")
            && markdown.contains("measured(n=3)")
            && markdown.contains("bench_mode: **full**")
            && markdown
                .contains("| 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |");
        self.ck(
            "N4 리포트 반복 절 — 완주/요청 3/3 · measured(n=3) · bench_mode full",
            cond,
            &tup(vec![status(&run), tl(&markdown, 900), tl(&run.stderr, 300)]),
        )?;
        let (reader_ok, shown) = self.hint_reader_view(&markdown)?;
        self.ck(
            "N4 hint 파서(render_bench_section.parse_report)가 스윕 표·절삭 블록을 그대로 읽는다(형식 계약 불변)",
            reader_ok,
            &st(&shown),
        )
    }

    /// The late import and parse of the hint reader: `(ok, str(parsed or exception))`. An `Exception` of the reader is
    /// the check's failure; what is not an `Exception` (`SystemExit`, the transport itself) ends the program.
    fn hint_reader_view(&mut self, markdown: &str) -> Result<(bool, String), Raised> {
        let directory = self.repo.join(HINT_READER_DIRECTORY);
        // The session starts here, at the first use (the original imported the reader at this very point).
        let mut session = OwnerBridge::start(self.scripts_dir, self.checks.console())?;
        let outcome = session.parse_sweep_report(self.checks.console(), &directory, markdown);
        drop(session);
        // A failure of the session itself is fatal; only an exception the reader raised reaches the match below.
        match outcome? {
            Ok(parsed) => {
                let levels_match = py_equals(&parsed.levels, &int_list(&[1, 2]));
                if !levels_match {
                    return Ok((false, parsed.shown));
                }
                match parsed.truncated {
                    Some(truncated) => Ok((
                        py_equals(&truncated, &PyValue::List(Vec::new())),
                        parsed.shown,
                    )),
                    None => Ok((false, "'truncated'".to_string())),
                }
            }
            // `except Exception`: what is not an `Exception` by its own hierarchy ends the run.
            Err(raised) if !raised.is_exception() => Err(raised),
            Err(raised) => Ok((
                false,
                raised.failure().message.to_utf8().unwrap_or_default(),
            )),
        }
    }

    /// N5: one certificate, bound to the representative run.
    fn certificate_check(&mut self) -> Outcome {
        let run = self.publish("cfg-n")?;
        let certificate = run.stdout.clone();
        let generated = get(&self.n_index, "generated_utc")?;
        let cond = run.status == 0
            && certificate.contains("decode_tps_conc1: 40.0")
            && certificate.matches("measured_utc:").count() == 1
            && certificate.contains(&format!("measured_utc: \"{}\"", py_str(&generated)))
            && !certificate.contains("runs")
            && certificate.contains("benchmark_mode: full");
        self.ck(
            "N5 인증서 1건 — decode_tps_conc1=대표 run 40.0 · measured_utc 1줄=index 측정시각 · runs 미포함",
            cond,
            &tup(vec![status(&run), tl(&certificate, 500), tl(&run.stderr, 300)]),
        )
    }

    /// N8: a stale or half-written decision record does not block the report and does block the certificate.
    fn damaged_decision_record(&mut self) -> Outcome {
        let record_path = self.n_dir.join("bench_mode.json");
        let good = read_text(&record_path)?;
        let loaded = loads_text(&PyStr::new(&good))
            .map_err(|error| Raised::from_failure(crate::repeat_api_cli::load_failure(&error)))?;
        let stale = with_key(
            &loaded,
            "sweep_index_generated_utc",
            st("2020-01-01T00:00:00Z"),
        )?;
        write_str(&record_path, &json_line(&stale)?)?;
        let report = self.report("cfg-n", &[])?;
        let published = self.publish("cfg-n")?;
        let cond = report.status == 0
            && report.stdout.contains("bench_mode: 미확정 — stale(")
            && published.status == 0
            && py_strip(&published.stdout).is_empty()
            && published.stderr.contains("stale(");
        self.ck(
            "N8★ 다른 측정의 판정 기록(stale) → 리포트 rc 0 · '미확정 — stale(' · 인증서 미발행",
            cond,
            &tup(vec![
                status(&report),
                tl(&report.stdout, 500),
                tl(&published.stdout, 200),
                tl(&published.stderr, 300),
            ]),
        )?;
        write_str(&record_path, "{\"bench_mode\": \"fu")?;
        let report = self.report("cfg-n", &[])?;
        let explicit = self.report(
            "cfg-n",
            &[
                OsString::from("--bench-mode-json"),
                record_path.clone().into_os_string(),
            ],
        )?;
        let cond = report.status == 0
            && report.stdout.contains("bench_mode: 미확정 — unreadable(")
            && explicit.status == 2;
        self.ck(
            "N8★ 반쯤 쓰인 판정 기록 → 관례 자리는 rc 0 · '미확정 — unreadable(' / 명시 경로는 exit 2",
            cond,
            &tup(vec![
                status(&report),
                tl(&report.stdout, 400),
                tl(&report.stderr, 300),
                status(&explicit),
            ]),
        )?;
        write_str(&record_path, &good)
    }

    /// N6 and A3: reassembly inherits the request recorded at measurement time.
    fn reassemble_inherits_request(&mut self) -> Outcome {
        self.sb.declare(Some(5))?;
        self.sb.reset_calls()?;
        crate::verify_fixture::unlink(&self.n_dir.join("bench_mode.json"))?;
        let reassemble = words(&[
            "cfg-n",
            "--topology",
            "single",
            "--backend",
            "openai",
            "--reassemble-only",
        ]);
        let run = self.bash("sweep_bench.sh", reassemble.clone())?;
        let index = self.index_of("cfg-n");
        let repetition = get_obj(&index, "repetition")?;
        let declared = format!("declared(campaigns/{CAMPAIGN}/campaign.yaml budgets.repeats)");
        let original_time = get(&self.n_index, "generated_utc")?;
        let calls = self.sb.calls()?;
        let cond = run.status == 0
            && calls.is_empty()
            && is_int_at(&repetition, "requested", 3)?
            && is_text_at(&repetition, "requested_source", &declared)?
            && is_int_at(&repetition, "completed_min", 3)?
            && get_eq(&index, "generated_utc", &original_time)?;
        self.ck(
            "N6 재조립 — shim 미호출 · 요청 3·출처 승계(선언 5 를 다시 읽지 않는다) · 완주 min 3 · 측정시각 승계",
            cond,
            &tup(vec![status(&run), self.calls_value()?, repetition, tl(&run.stderr, 400)]),
        )?;
        let rewritten = self.side("cfg-n");
        let cond = is_text_at(&self.side("cfg-n"), "bench_mode", "full")?
            && get_eq(
                &self.side("cfg-n"),
                "sweep_index_generated_utc",
                &original_time,
            )?;
        self.ck(
            "N6 재조립도 판정 기록을 다시 쓴다(같은 측정시각에 묶인 full)",
            cond,
            &rewritten,
        )?;
        self.sb.declare(Some(3))?;
        let mut with_repeats = reassemble;
        with_repeats.extend(words(&["--repeats", "4"]));
        let run = self.bash("sweep_bench.sh", with_repeats)?;
        let cond = run.status == 2 && run.stderr.contains("--reassemble-only");
        self.ck(
            "A3★ --reassemble-only 에 --repeats → exit 2(측정하지 않은 반복 수를 조용히 버리지 않는다)",
            cond,
            &tup(vec![status(&run), tl(&run.stderr, 300)]),
        )
    }

    /// N7: a re-sweep of the same config does not mix the previous sweep's runs in.
    fn rerun_does_not_mix_old_runs(&mut self) -> Outcome {
        let first = self.sweep(
            "cfg-s",
            &["--repeats", "4", "--repeats-source", "fixture declaration"],
            None,
            "1",
        )?;
        let directory = self.sb.sweep_dir("cfg-s", "single");
        let had_fourth = is_file(&directory.join("level_01/run_04/measured.json"))?;
        let second = self.sweep("cfg-s", &[], None, "1")?;
        let measured = load_object(&directory.join("level_01/measured.json"));
        let cond = first.status == 0
            && had_fourth
            && second.status == 0
            && !exists(&directory.join("level_01/run_04"))?
            && py_len(&get_list(&measured, "runs")?)? == 3
            && is_int_at(&measured, "repeats_requested", 3)?;
        let mut run_numbers = Vec::new();
        for entry in iterate_values(&get_list(&measured, "runs")?)? {
            run_numbers.push(get(&entry, "run")?);
        }
        self.ck(
            "N7★ --repeats 4 뒤 기본 3 재스윕 → run_04 제거 · runs[] 3 · 요청 3",
            cond,
            &tup(vec![
                status(&first),
                PyValue::Bool(had_fourth),
                status(&second),
                PyValue::List(run_numbers),
            ]),
        )
    }
}

/// `7.0 < value < 9.0` for a loaded number.
fn band_between(value: &PyValue, low: f64, high: f64) -> bool {
    matches!(value.peel(), PyValue::Float(number) if low < *number && *number < high)
}

/// `dict(value, key=new)`: a copy of a loaded object with `key` set (an existing key keeps its place).
fn with_key(value: &PyValue, key: &str, new: PyValue) -> Result<PyValue, Raised> {
    match value.peel() {
        PyValue::Dict(map) => {
            let mut copy = map.clone();
            copy.set(key, new);
            Ok(PyValue::Dict(copy))
        }
        other => Err(Raised::new(
            "TypeError",
            &format!("'{}' object is not iterable", other.type_name()),
        )),
    }
}

impl Program<'_, '_> {
    /// P1–P3 and R1.
    fn reassembly_and_presets(&mut self) -> Outcome {
        let index = self.index_of("cfg-s");
        let knobs_record = get_obj(&index, "bench_knobs")?;
        let knobs = get_obj(&knobs_record, "knobs")?;
        let pending = self
            .sb
            .sweep_dir("cfg-s", "single")
            .join(".bench_knobs.pending.json");
        let cond = is_text_at(&knobs_record, "preset", "default_preset")?
            && is_text_at(&get_obj(&knobs, "levels")?, "source", "agent")?
            && is_text_at(&get_obj(&knobs, "levels")?, "reason", "serving_concurrency")?
            && is_text_at(&get_obj(&knobs, "repeats")?, "source", "campaign")?
            && is_text_at(&get_obj(&knobs, "input_len")?, "source", "preset")?
            && is_int_at(&index, "input_len", 1024)?
            && !exists(&pending)?;
        let mut shown = PyDict::new();
        if let PyValue::Dict(map) = knobs.peel() {
            for (name, value) in map.iter() {
                shown.insert(
                    name.clone(),
                    tup(vec![get(value, "source")?, get(value, "reason")?]),
                );
            }
        } else {
            return Err(Raised::new(
                "AttributeError",
                &format!("'{}' object has no attribute 'items'", knobs.type_name()),
            ));
        }
        self.ck(
            "P1★ sweep_index.bench_knobs = 프리셋 이름 · levels Agent 선택+근거 코드 · 반복 수 캠페인 선언 · 기본값 출처 preset",
            cond,
            &tup(vec![get(&knobs_record, "preset")?, PyValue::Dict(shown)]),
        )?;
        self.sb.reset_calls()?;
        let run = self.sweep(
            "cfg-p",
            &[
                "--input-len",
                "32768",
                "--knob-reason",
                "input_len=serving_context",
            ],
            None,
            "1",
        )?;
        let cond =
            run.status == 2 && self.sb.calls()?.is_empty() && run.stderr.contains("escalate");
        self.ck(
            "P2★ escalate 값(input_len 32768)을 Agent 가 고름 → 부하 전 exit 2 · lite·레벨 shim 미호출",
            cond,
            &tup(vec![status(&run), self.calls_value()?, tl(&run.stderr, 300)]),
        )?;
        self.sb.reset_calls()?;
        let run = self.sweep("cfg-p", &["--input-len", "2048"], None, "1")?;
        let cond =
            run.status == 2 && self.sb.calls()?.is_empty() && run.stderr.contains("근거 코드");
        self.ck(
            "P3★ 근거 코드 없는 Agent 선택(input_len 2048) → 부하 전 exit 2",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 300),
            ]),
        )?;
        self.reassembly_keeps_index_levels()
    }

    /// R1: reassembly restores only the levels the existing index holds.
    fn reassembly_keeps_index_levels(&mut self) -> Outcome {
        let first = self.sweep("cfg-r", &[], None, "1,2")?;
        let interrupted_plan = dict_of(vec![("fail", dict_of(vec![("1:2", st("measurement"))]))]);
        let second = self.sweep("cfg-r", &[], Some(&interrupted_plan), "1,2")?;
        let middle_levels = level_numbers(&load_object(
            &self
                .sb
                .sweep_dir("cfg-r", "single")
                .join("sweep_index.json"),
        ))?;
        let stale_on_disk = is_file(
            &self
                .sb
                .sweep_dir("cfg-r", "single")
                .join("level_02/measured.json"),
        )?;
        let third = self.bash(
            "sweep_bench.sh",
            words(&[
                "cfg-r",
                "--topology",
                "single",
                "--backend",
                "openai",
                "--reassemble-only",
            ]),
        )?;
        let rebuilt = self.index_of("cfg-r");
        let cond = first.status == 0
            && second.status == 0
            && py_equals(&middle_levels, &int_list(&[1]))
            && stale_on_disk
            && third.status == 0
            && py_equals(&level_numbers(&rebuilt)?, &int_list(&[1]))
            && get_eq(
                &get_obj(&rebuilt, "repetition")?,
                "completed_by_level",
                &dict_of(vec![("1", int(1))]),
            )?
            && is_text_at(&self.side("cfg-r"), "bench_mode", "lite")?;
        self.ck(
            "R1★ 판정점에서 끊긴 재스윕 뒤 재조립 → levels [1] 유지(디스크의 옛 level_02 부활 ✗) · 판정 기록 lite 유지",
            cond,
            &tup(vec![
                status(&first),
                status(&second),
                middle_levels,
                PyValue::Bool(stale_on_disk),
                status(&third),
                level_numbers(&rebuilt)?,
                tl(&third.stderr, 300),
            ]),
        )
    }

    /// G: a lite result that does not pass keeps the run out of GuideLLM (lite ⊂ full).
    fn lite_gate(&mut self) -> Outcome {
        let cases: [(&str, &str, i64, i64); 5] = [
            ("mpf", "measurement_path_failed", 6, 6),
            ("srv", "server_failed", 7, 7),
            ("abs", "absent", 0, 6),
            ("unk", "pass", 9, 6),
            ("voc", "ok", 0, 6),
        ];
        for (tag, verdict, shim_rc, want) in cases {
            self.sb.reset_calls()?;
            let config = format!("cfg-gate-{tag}");
            let lite = dict_of(vec![("verdict", st(verdict)), ("rc", int(shim_rc))]);
            let plan = dict_of(vec![("lite", lite.clone())]);
            let run = self.sweep(&config, &[], Some(&plan), "1,2")?;
            let directory = self.sb.sweep_dir(&config, "single");
            let truncation = directory.join("truncation.log");
            let log = if is_file(&truncation)? {
                read_text(&truncation)?
            } else {
                String::new()
            };
            let calls = self.sb.calls()?;
            let cond = run.status == want
                && level_calls(&calls).is_empty()
                && !has_level_entries(&directory)
                && !exists(&directory.join("sweep_index.json"))?
                && log.contains("lite gate failed");
            let name = format!(
                "G★ lite 게이트 {tag}({}) → exit {want} · GuideLLM 레벨 호출 0 · level_* 0 · index 0 · 절삭 로그 기재",
                lite.repr()
            );
            self.ck(
                &name,
                cond,
                &tup(vec![
                    status(&run),
                    self.calls_value()?,
                    tl(&run.stderr, 400),
                ]),
            )?;
        }
        let script = self.scripts_dir.join(CONFORMANCE_SCRIPT);
        let conformance = run_text_within(
            &[OsString::from("bash"), script.into_os_string()],
            &crate::captured_process::ChildSettings::default(),
            CONFORMANCE_SECONDS,
        )?;
        self.ck(
            "G conformance(게이트 포함 단언 포함) 초록",
            conformance.status == 0,
            &tl(&conformance.stdout, 600),
        )
    }
}

/// `list(directory.glob("level_*"))` is not empty (a missing directory has none).
fn has_level_entries(directory: &Path) -> bool {
    std::fs::read_dir(directory).is_ok_and(|entries| {
        entries
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().starts_with("level_"))
    })
}

impl Program<'_, '_> {
    /// F1–F8 and V1 with the negative controls R★.
    fn failure_injection(&mut self) -> Outcome {
        self.stop_at_decision_point()?;
        self.asymmetric_correction()?;
        self.clamp_level_failures()?;
        self.aggregation_failure()?;
        self.variance_alone()
    }

    fn stop_at_decision_point(&mut self) -> Outcome {
        self.sb.clear_events();
        self.sb.reset_calls()?;
        let plan = fail_plan("1:2", "measurement");
        let run = self.sweep("cfg-f", &[], Some(&plan), "1,2")?;
        let directory = self.sb.sweep_dir("cfg-f", "single");
        let index = load_object(&directory.join("sweep_index.json"));
        let measured = load_object(&directory.join("level_01/measured.json"));
        let truncation_path = directory.join("truncation.log");
        let truncation = if is_file(&truncation_path)? {
            read_text(&truncation_path)?
        } else {
            String::new()
        };
        let calls = self.sb.calls()?;
        let cond = run.status == 0
            && level_calls(&calls).len() == 2
            && !exists(&directory.join("level_02/measured.json"))?;
        self.ck(
            "F1★ 대표 run 은 섰으므로 rc=0 · run 2 에서 멈춘다(shim 2 호출 · level 2 미측정)",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 400),
            ]),
        )?;
        let stop = or_default(
            get(&get_obj(&index, "repetition")?, "stop")?,
            empty_object(),
        );
        let cond = truncation.contains("repeat-break")
            && is_text_at(&stop, "kind", "repeat-break")?
            && is_int_at(&stop, "run", 2)?
            && py_len(&get_list(&measured, "runs")?)? == 2
            && is_int_at(&measured, "repeats_completed", 1)?
            && is_none_at(&measured, "repro_band_pct")?
            && str_at_starts(&measured, "repro_band_source", "unavailable(n=1")?;
        self.ck(
            "F1★ 즉시 신호 — truncation.log · index.repetition.stop(repeat-break · level 1 run 2) · runs[] 2 · 완주 1 · 밴드 없음",
            cond,
            &tup(vec![st(&truncation), get(&index, "repetition")?, get(&measured, "runs")?]),
        )?;
        let record = self.side("cfg-f");
        let cond = is_text_at(&record, "bench_mode", "lite")?
            && is_text_at(&record, "downgrade_reason", "run_failed")?
            && is_text_at(&record, "downgrade_correlation", "not_scanned")?;
        self.ck(
            "F1★ 판정 기록(정규 경로) → lite · run_failed · 사살 대조 not_scanned(블랙박스 events 0)",
            cond,
            &record,
        )?;
        self.classification_controls(&stop)?;
        self.demoted_cell_outputs()
    }

    /// F2: the same stopped window with a kill, a trip alone, and a kill outside the window.
    fn classification_controls(&mut self, stop: &PyValue) -> Outcome {
        let window_end = get(stop, "window_end_utc")?;
        let event = |ts: PyValue, kind: &str| {
            dict_of(vec![("ts", ts), ("kind", st(kind)), ("mode", st("armed"))])
        };
        let (decision, _) =
            self.classify("cfg-f", &[event(window_end.clone(), "watchdog_kill_ack")])?;
        let cond = is_text_at(&decision, "bench_mode", "lite")?
            && is_text_at(&decision, "downgrade_reason", "blackbox_kill")?
            && is_text_at(&decision, "downgrade_correlation", "matched")?;
        self.ck(
            "F2★ 끊긴 창 안 집행 사살(시각 일치) → lite · blackbox_kill · matched",
            cond,
            &decision,
        )?;
        let (decision, _) = self.classify("cfg-f", &[event(window_end, "thermal_trip")])?;
        let cond = is_text_at(&decision, "downgrade_reason", "run_failed")?
            && is_text_at(&decision, "downgrade_correlation", "miss")?;
        self.ck(
            "F2★ 음성대조: 같은 시각 트립 단독 → blackbox_kill ✗ · run_failed · miss",
            cond,
            &decision,
        )?;
        let (decision, _) = self.classify(
            "cfg-f",
            &[event(st("2020-01-01T00:00:00Z"), "watchdog_kill_ack")],
        )?;
        let cond = is_text_at(&decision, "downgrade_reason", "run_failed")?;
        self.ck(
            "F2★ 음성대조: 사살 시각 불일치(창 밖) → run_failed",
            cond,
            &decision,
        )
    }

    /// F3 and the first R★: a demoted cell gets the lite-grade certificate, and the report names why.
    fn demoted_cell_outputs(&mut self) -> Outcome {
        let published = self.publish("cfg-f")?;
        let out = &published.stdout;
        let cond = published.status == 0
            && !out.contains("benchmark_mode: full")
            && out.contains("benchmark_mode: lite")
            && out.contains("verdict: not_applicable")
            && out.contains("lite_verdict: pass")
            && out.contains("entry_path: gamma_demoted_lite");
        self.ck(
            "F3★ 강등 셀(판정 기록 lite)에는 PASS 여도 full 인증서를 내지 않고 lite 등급 인증서(γ)를 낸다",
            cond,
            &tup(vec![tl(out, 300), st(&published.stderr)]),
        )?;
        let report = self.report("cfg-f", &[])?;
        let cond = report
            .stdout
            .contains("**스윕이 멈춘 자리 — 반복 중단**: level 1 run 2")
            && report.stdout.contains("✗(run 2)")
            && report.stdout.contains("**강등** · 사유 `run_failed`");
        self.ck(
            "F3★ 리포트에 멈춘 자리·실패 run·강등 사유가 이름으로 남는다",
            cond,
            &tl(&report.stdout, 900),
        )?;
        let line = bench_mode_line(&report.stdout);
        let cond = line.contains("⚠ **대조 불가**") && line.matches("대조 노드[").count() == 1;
        self.ck(
            "R★ 강등 행(lite · run_failed)도 대조 not_scanned 이면 '⚠ 대조 불가' 표지 · 노드 요약은 한 번만",
            cond,
            &st(&line),
        )
    }

    /// F4: a failure at the boundary level (decision point 3, level 2 has 1) stays full.
    fn asymmetric_correction(&mut self) -> Outcome {
        self.sb.clear_events();
        let plan = fail_plan("2:2", "measurement");
        let run = self.sweep("cfg-g", &[], Some(&plan), "1,2")?;
        let repetition = get_obj(&self.index_of("cfg-g"), "repetition")?;
        let record = self.side("cfg-g");
        let published = self.publish("cfg-g")?;
        let cond = run.status == 0
            && get_eq(
                &repetition,
                "completed_by_level",
                &dict_of(vec![("1", int(3)), ("2", int(1))]),
            )?
            && is_text_at(&get_obj(&repetition, "stop")?, "kind", "repeat-break")?
            && is_text_at(&record, "bench_mode", "full")?
            && contained_in_or_empty("적응 상한 클램프", &record, "bench_mode_source")?
            && published.stdout.contains("benchmark_mode: full");
        self.ck(
            "F4★ 경계 레벨 2 run 2 실패(판정점 3 · 레벨2 1) → full(적응 상한 클램프) · 인증서 발행",
            cond,
            &tup(vec![repetition, record, tl(&published.stderr, 300)]),
        )
    }

    /// F5–F7: the first run of the clamp level fails (kill, trip alone, plain exit code).
    fn clamp_level_failures(&mut self) -> Outcome {
        self.sb.clear_events();
        let run = self.sweep("cfg-k", &[], Some(&fail_plan("2:1", "kill")), "1,2")?;
        let index = self.index_of("cfg-k");
        let repetition = get_obj(&index, "repetition")?;
        let record = self.side("cfg-k");
        let published = self.publish("cfg-k")?;
        let cond = run.status == 0
            && py_equals(&level_numbers(&index)?, &int_list(&[1]))
            && is_text_at(&get_obj(&repetition, "stop")?, "kind", "clamp")?
            && is_int_at(&repetition, "runs_attempted", 4)?
            && is_text_at(&record, "bench_mode", "lite")?
            && is_text_at(&record, "downgrade_reason", "blackbox_kill")?
            && !published.stdout.contains("benchmark_mode: full")
            && published.stdout.contains("benchmark_mode: lite");
        self.ck(
            "F5★ 클램프 레벨 2 첫 run 사살 → stop.kind=clamp · 판정 기록 lite · blackbox_kill · 인증서 미발행",
            cond,
            &tup(vec![repetition, record, tl(&run.stderr, 400), tl(&published.stderr, 300)]),
        )?;
        self.sb.clear_events();
        let run = self.sweep("cfg-t", &[], Some(&fail_plan("2:1", "trip")), "1,2")?;
        let record = self.side("cfg-t");
        let cond = run.status == 0
            && is_text_at(&record, "bench_mode", "full")?
            && is_text_at(&record, "downgrade_correlation", "miss")?
            && contained_in_or_empty("사인 불충분", &record, "bench_mode_source")?;
        self.ck(
            "F6★ 같은 자리 트립 단독 → full 유지(사인 불충분 · 대조 miss)",
            cond,
            &record,
        )?;
        let line = bench_mode_line(&self.report("cfg-t", &[])?.stdout);
        let cond = !line.is_empty()
            && !line.contains("대조 불가")
            && line.matches("대조 노드[").count() == 1;
        self.ck(
            "R★ 음성대조: 대조 miss(이 노드 기록을 봤다) 행에는 '대조 불가' 표지가 없다 · 노드 요약은 한 번",
            cond,
            &st(&line),
        )?;
        self.sb.clear_events();
        let run = self.sweep("cfg-c", &[], Some(&fail_plan("2:1", "rc")), "1,2")?;
        let record = self.side("cfg-c");
        let cond = run.status == 0
            && is_text_at(&record, "bench_mode", "full")?
            && is_text_at(&record, "downgrade_correlation", "not_scanned")?;
        self.ck(
            "F7 경계 레벨 2 첫 run 실패(rc) · events 0 → full · 대조 not_scanned(F4 와 같은 판정)",
            cond,
            &record,
        )?;
        let line = bench_mode_line(&self.report("cfg-c", &[])?.stdout);
        let cond = line.contains("**full**")
            && line.contains("⚠ **대조 불가**")
            && line.matches("대조 노드[").count() == 1;
        self.ck(
            "R★ full 행 · 대조 not_scanned → '⚠ 대조 불가' 표지 · 노드 요약은 한 번만(분류기 출처와 겹쳐 두 번 찍지 않는다)",
            cond,
            &st(&line),
        )
    }

    /// F8: the run boundary fact cannot be recorded.
    fn aggregation_failure(&mut self) -> Outcome {
        let run = self.sweep("cfg-u", &[], Some(&fail_plan("1:2", "norecord")), "1")?;
        let repetition = get_obj(&self.index_of("cfg-u"), "repetition")?;
        let record = self.side("cfg-u");
        let published = self.publish("cfg-u")?;
        let cond = run.status == 0
            && get_eq(&repetition, "unrecorded_levels", &int_list(&[1]))?
            && is_int_at(&repetition, "requested", 3)?
            && is_none_at(&record, "bench_mode")?
            && str_at_starts(&record, "bench_mode_source", "not_evaluated(")?
            && published.status == 0
            && py_strip(&published.stdout).is_empty()
            && published.stderr.contains("판정 기록");
        self.ck(
            "F8★ 경계 사실 기록 실패 → runs[] 없는 레벨 [1] · 판정 기록 null(not_evaluated) · full 인증서 미발행",
            cond,
            &tup(vec![repetition, record, tl(&published.stdout, 200), tl(&published.stderr, 300)]),
        )
    }

    /// V1: variance alone is not a downgrade reason.
    fn variance_alone(&mut self) -> Outcome {
        let plan = dict_of(vec![(
            "tpot",
            dict_of(vec![(
                "1",
                PyValue::List(vec![float(20.0), float(30.0), float(45.0)]),
            )]),
        )]);
        let run = self.sweep("cfg-v", &[], Some(&plan), "1")?;
        let measured = load_object(
            &self
                .sb
                .sweep_dir("cfg-v", "single")
                .join("level_01/measured.json"),
        );
        let record = self.side("cfg-v");
        let band = or_default(get(&measured, "repro_band_pct")?, int(0));
        let cond = run.status == 0
            && number_above(&band, 60)?
            && is_text_at(&record, "bench_mode", "full")?
            && is_none_at(&record, "downgrade_reason")?;
        self.ck(
            "V1★ 밴드 > 60% 인데도 full · 사유 null(분산은 강등 사유가 아니다 · 기재만)",
            cond,
            &tup(vec![get(&measured, "repro_band_pct")?, record]),
        )
    }

    /// A1, A2 and A4: declaration violations stop before the load tool.
    fn declaration_violations(&mut self) -> Outcome {
        self.sb.reset_calls()?;
        let run = self.sweep("cfg-a", &["--repeats", "2"], None, "1,2")?;
        let cond =
            run.status == 2 && self.sb.calls()?.is_empty() && run.stderr.contains("full 정의");
        self.ck(
            "A1★ --repeats 2 → exit 2 · lite·레벨 shim 미호출(부하 전 거부)",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 300),
            ]),
        )?;
        self.sb.declare(Some(2))?;
        let run = self.sweep("cfg-a", &[], None, "1,2")?;
        let cond = run.status == 2 && self.sb.calls()?.is_empty();
        self.ck(
            "A2★ 캠페인 선언 budgets.repeats=2 → exit 2 · 미호출(기본값 대체 ✗)",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 300),
            ]),
        )?;
        self.sb.declare(Some(3))?;
        self.sb.reset_calls()?;
        let without_source =
            self.sweep("cfg-a", &["--cross-node-tolerance-s", "2"], None, "1,2")?;
        let not_a_number = self.sweep(
            "cfg-a",
            &[
                "--cross-node-tolerance-s",
                "x",
                "--cross-node-tolerance-source",
                "fixture",
            ],
            None,
            "1,2",
        )?;
        let cond = without_source.status == 2
            && not_a_number.status == 2
            && self.sb.calls()?.is_empty()
            && without_source.stderr.contains("함께 준다");
        self.ck(
            "A4★ 출처 없는 노드 간 허용오차 · 정수 아닌 값 → exit 2 · 부하 도구 미호출",
            cond,
            &tup(vec![
                status(&without_source),
                status(&not_a_number),
                self.calls_value()?,
                tl(&without_source.stderr, 300),
            ]),
        )
    }
}

/// `{"fail": {key: action}}`: the shim plan that fails one run.
fn fail_plan(key: &str, action: &str) -> PyValue {
    dict_of(vec![("fail", dict_of(vec![(key, st(action))]))])
}
