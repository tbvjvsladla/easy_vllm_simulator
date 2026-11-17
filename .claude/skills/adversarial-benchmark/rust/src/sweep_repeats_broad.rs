//! The Broad Search, explicit-output-path and multi-node groups of `selftest_sweep_repeats` (B, O, M), as methods
//! of the same [`Program`] the normal-path groups run in. See [`crate::selftest_sweep_repeats`] for the shape of the
//! whole program; this file keeps the original's order: B1 init, B6, B1/B2 cell (from the broad wiring), O1–O6
//! (new output paths, including the concurrent claim), then B3, B8, B4, B5, B7 and finally M1, M2.

use crate::bench_verify_support::{dict_of, float, int, st};
use crate::conformance::{iterate_values, Raised};
use crate::pyvalue::{PyDict, PyValue};
use crate::selftest_sweep_repeats::{
    contained_in_or_empty, empty_object, get_default, get_obj, is_int_at, is_none_at, is_text_at,
    json_line, level_calls, load_object, status, str_at_starts, tl, tup, Outcome, Program,
};
use crate::sweep_repeats_clock::strptime;
use crate::sweep_repeats_fixture::{Sandbox, CAMPAIGN, KNOB_REASONS, LEVEL_REASON};
use crate::verify_fixture::{
    contains, exists, get, is_file, or_default, read_bytes, read_text, write_str, TextRun,
};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// The injected clock of every broad search call.
const NOW: &str = "2026-01-01T00:00:00Z";
const FIRST_EVENTS: &str = "docs/logs/main/events/2026-09.jsonl";

fn arguments(items: &[&str]) -> Vec<OsString> {
    items.iter().map(OsString::from).collect()
}

fn with_path(mut head: Vec<OsString>, path: &Path, tail: &[&str]) -> Vec<OsString> {
    head.push(path.as_os_str().to_os_string());
    head.extend(arguments(tail));
    head
}

/// The arguments of `broad_search.sh cell` up to the options that differ: state, cell key, config, citation, intent
/// and budget.
fn cell_head(state: &Path, key: &str, config: &str) -> Vec<OsString> {
    let mut words = arguments(&["cell", "--state"]);
    words.push(state.as_os_str().to_os_string());
    words.extend(arguments(&[
        "--cell-key",
        key,
        "--config",
        config,
        "--axis-citation",
        "fixture",
        "--next-intent",
        "fixture",
        "--bench-budget-mib",
        "4096",
    ]));
    words
}

/// The options of a measuring cell: backend, shape reasons, one level, clock, topology and the risk confirmation.
fn measuring_tail(extra: &[OsString]) -> Vec<OsString> {
    let mut words = arguments(&["--backend", "openai"]);
    words.extend(KNOB_REASONS.map(OsString::from));
    words.extend(arguments(&["--levels", "1"]));
    words.extend(LEVEL_REASON.map(OsString::from));
    words.extend(arguments(&[
        "--now-utc",
        NOW,
        "--topology",
        "single",
        "--confirm-risk",
    ]));
    words.extend(extra.iter().cloned());
    words
}

fn init_command(sweep_id: &str, state: &Path, cells: &str, topology: &str) -> Vec<OsString> {
    let mut words = arguments(&["init", "--sweep-id", sweep_id, "--state"]);
    words.push(state.as_os_str().to_os_string());
    words.extend(arguments(&[
        "--cells",
        cells,
        "--control-variable",
        "fixture",
        "--max-cells",
        "10",
        "--wall-clock-budget-s",
        "999999999",
        "--consecutive-failure-limit",
        "5",
        "--declared-by",
        "selftest",
        "--basis",
        "fixture",
        "--authority",
        "explore",
        "--now-utc",
        NOW,
        "--topology",
        topology,
    ]));
    words
}

/// The bytes and modification time (ns) of every regular file under a directory, by relative path
/// (`{str(p.relative_to(path)): (p.read_bytes(), p.stat().st_mtime_ns) for p in path.rglob("*") if p.is_file()}`).
fn tree_bytes(root: &Path) -> Result<BTreeMap<String, (Vec<u8>, i128)>, Raised> {
    let mut files = BTreeMap::new();
    collect_files(root, root, &mut files)?;
    Ok(files)
}

fn collect_files(
    root: &Path,
    directory: &Path,
    files: &mut BTreeMap<String, (Vec<u8>, i128)>,
) -> Result<(), Raised> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Ok(());
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            collect_files(root, &path, files)?;
        } else if let Ok(metadata) = std::fs::metadata(&path) {
            if metadata.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let modified = i128::from(metadata.mtime()) * 1_000_000_000
                    + i128::from(metadata.mtime_nsec());
                files.insert(
                    relative.to_string_lossy().into_owned(),
                    (read_bytes(&path)?, modified),
                );
            }
        }
    }
    Ok(())
}

fn rows_text(rows: &[TextRun]) -> PyValue {
    PyValue::List(
        rows.iter()
            .map(|run| tup(vec![status(run), tl(&run.stderr, 200)]))
            .collect(),
    )
}

impl Program<'_, '_> {
    fn bs_state(&self) -> PathBuf {
        self.sb.campaign_dir().join("sweeps").join("bs.json")
    }

    fn broad(&self, words: Vec<OsString>, plan: Option<&PyValue>) -> Result<TextRun, Raised> {
        let mut command = vec![OsString::from("bash"), self.sb.script("broad_search.sh")];
        command.extend(words);
        self.sb.run(command, plan)
    }

    /// `cell(key, cfg, plan_, *extra)`: a measuring cell on the state file `bs.json`.
    fn cell(
        &self,
        key: &str,
        config: &str,
        plan: &PyValue,
        extra: &[OsString],
    ) -> Result<TextRun, Raised> {
        self.sb.config(config, "single")?;
        let mut words = cell_head(&self.bs_state(), key, config);
        words.extend(measuring_tail(extra));
        self.broad(words, Some(plan))
    }

    /// `record(key, path)`: the last cell record of that key in a state file, `{}` when there is none.
    fn record(&self, key: &str, path: &Path) -> Result<PyValue, Raised> {
        let document = load_object(path);
        let cells = or_default(get(&document, "cells")?, PyValue::List(Vec::new()));
        let mut found = empty_object();
        for row in iterate_values(&cells)? {
            if is_text_at(&row, "cell_key", key)? {
                found = row;
            }
        }
        Ok(found)
    }

    /// B1 init, B6, B1 cell and B2.
    pub(crate) fn broad_search_wiring(&mut self) -> Outcome {
        self.sb.clear_events();
        let state = self.bs_state();
        let run = self.broad(init_command("bs", &state, "cell-a,cell-b", "single"), None)?;
        let budget = get_obj(&load_object(&state), "declared_budget")?;
        let declared = format!("declared(campaigns/{CAMPAIGN}/campaign.yaml budgets.repeats)");
        let cond = run.status == 0
            && is_int_at(&budget, "repeats", 3)?
            && is_text_at(&budget, "repeats_source", &declared)?;
        self.ck(
            "B1 init → declared_budget.repeats=3 · 출처=상태 파일의 캠페인 선언",
            cond,
            &tup(vec![status(&run), budget, tl(&run.stderr, 300)]),
        )?;
        self.tolerance_pair_before_load(&state)?;
        self.sb.reset_calls()?;
        let plan = tpot_plan(&[25.0, 25.5, 24.5]);
        let run = self.cell(
            "cell-a",
            "cfg-ba",
            &plan,
            &arguments(&[
                "--cross-node-tolerance-s",
                "7",
                "--cross-node-tolerance-source",
                "fixture-forward",
            ]),
        )?;
        let record = self.record("cell-a", &state)?;
        let stop = load_object(&self.sb.campaign_dir().join("sweeps").join("bs.stop.json"));
        let measured = load_object(
            &self
                .sb
                .sweep_dir("cfg-ba", "single")
                .join("level_01/measured.json"),
        );
        let scan = get_obj(&self.side("cfg-ba"), "events_scan")?;
        let cond = is_int_at(&scan, "cross_node_tolerance_s", 7)?
            && is_text_at(&scan, "cross_node_tolerance_source", "fixture-forward")?;
        self.ck(
            "B1 cell 이 노드 간 허용오차 선언(값·출처)을 sweep_bench 에 넘긴다(판정 기록 events_scan 에 실린다 · 판정은 불변)",
            cond,
            &scan,
        )?;
        let calls = self.sb.calls()?;
        let cond = run.status == 0
            && level_calls(&calls).len() == 3
            && contained_in_or_empty(
                "sweep state declared_budget.repeats",
                &measured,
                "repeats_requested_source",
            )?;
        self.ck(
            "B1 cell 이 스윕 선언의 반복 수를 sweep_bench 에 넘긴다(출처에 sweep state 경유가 남는다)",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                get(&measured, "repeats_requested_source")?,
                tl(&run.stderr, 600),
            ]),
        )?;
        let record_side = self.side("cfg-ba");
        let cond = is_text_at(&record, "cell_outcome", "measured")?
            && is_text_at(&record, "bench_mode", "full")?
            && crate::conformance::py_equals(
                &get(&record, "bench_mode_source")?,
                &get(&record_side, "bench_mode_source")?,
            )
            && crate::conformance::py_equals(
                &get(&record, "downgrade_correlation")?,
                &get(&self.side("cfg-ba"), "downgrade_correlation")?,
            )
            && is_int_at(&get_obj(&record, "repetition")?, "completed_min", 3)?;
        self.ck(
            "B2 셀 기록 bench_mode=full(판정 기록 그대로 · 대조 필드 포함) · repetition(요청 3·완주 3)",
            cond,
            &tup(vec![
                get(&record, "cell_outcome")?,
                get(&record, "bench_mode")?,
                get(&record, "bench_mode_source")?,
                tl(&run.stderr, 400),
            ]),
        )?;
        let cond = is_int_at(&stop, "runs_attempted", 3)?
            && crate::conformance::py_equals(
                &get(&stop, "repeats_mismatch")?,
                &PyValue::List(Vec::new()),
            )
            && is_int_at(&get_obj(&stop, "declared_budget")?, "repeats", 3)?;
        self.ck(
            "B2 정지 평가가 반복 소비를 기재한다(runs_attempted 3 · 선언 불일치 없음)",
            cond,
            &stop,
        )
    }

    /// B6: the cross-node tolerance pair is checked before the cell starts (measuring and `--serve-failed`).
    fn tolerance_pair_before_load(&mut self, state: &Path) -> Outcome {
        self.sb.reset_calls()?;
        self.sb.config("cfg-ba", "single")?;
        let mut measuring = cell_head(state, "cell-a", "cfg-ba");
        measuring.extend(measuring_tail(&arguments(&[
            "--cross-node-tolerance-s",
            "5",
        ])));
        let first = self.broad(measuring, None)?;
        let serve_failed_head = cell_head(state, "cell-a", "cfg-ba");
        let mut source_only = serve_failed_head.clone();
        source_only.extend(arguments(&[
            "--now-utc",
            NOW,
            "--topology",
            "single",
            "--serve-failed",
            "fixture",
            "--serve-started-utc",
            NOW,
            "--cross-node-tolerance-source",
            "only-source",
        ]));
        let second = self.broad(source_only, None)?;
        let mut not_a_number = serve_failed_head;
        not_a_number.extend(arguments(&[
            "--now-utc",
            NOW,
            "--topology",
            "single",
            "--serve-failed",
            "fixture",
            "--serve-started-utc",
            NOW,
            "--cross-node-tolerance-s",
            "x",
            "--cross-node-tolerance-source",
            "s",
        ]));
        let third = self.broad(not_a_number, None)?;
        let cond = first.status == 2
            && second.status == 2
            && third.status == 2
            && self.sb.calls()?.is_empty()
            && first.stderr.contains("함께 준다")
            && second.stderr.contains("함께 준다")
            && third.stderr.contains("정수")
            && !get(&load_object(state), "cells")?.is_truthy();
        self.ck(
            "B6★ 허용오차 짝 불일치(측정 · --serve-failed)·비정수 → 셀 진입 전 exit 2 · 부하 도구 미호출 · 셀 기록 0",
            cond,
            &tup(vec![
                status(&first),
                status(&second),
                status(&third),
                self.calls_value()?,
                tl(&first.stderr, 300),
                tl(&third.stderr, 200),
            ]),
        )
    }

    /// O1–O6: an explicit new sweep path keeps the earlier measurement's bytes and paths.
    pub(crate) fn new_output_paths(&mut self) -> Outcome {
        let old_dir = self.sb.sweep_dir("cfg-ba", "single");
        let old_tree = tree_bytes(&old_dir)?;
        let fresh = old_dir.with_file_name("sweep_cfg-ba_fresh");
        let relative_fresh = relative_text(self.sb.root(), &fresh);
        let check = self.bash(
            "sweep_bench.sh",
            arguments(&[
                "cfg-ba",
                "--topology",
                "single",
                "--sweep-dir",
                &relative_fresh,
                "--check-sweep-dir",
            ]),
        )?;
        let fresh_text = fresh.to_string_lossy().into_owned();
        let dry = self.sweep(
            "cfg-ba",
            &["--sweep-dir", &fresh_text, "--dry-run"],
            None,
            "1",
        )?;
        let cond = check.status == 0
            && crate::bench_verify_support::py_strip(&check.stdout) == fresh_text
            && dry.status == 0
            && !exists(&fresh)?
            && tree_bytes(&old_dir)? == old_tree;
        self.ck(
            "O1 명시 경로 check/dry-run은 출력 디렉터리를 만들지 않고 이전 측정을 보존",
            cond,
            &tup(vec![status(&check), status(&dry), tl(&dry.stderr, 400)]),
        )?;

        self.sb.reset_calls()?;
        let plan = tpot_plan(&[25.0, 25.5, 24.5]);
        let run = self.cell(
            "cell-a",
            "cfg-ba",
            &plan,
            &arguments(&["--sweep-dir", &relative_fresh]),
        )?;
        let fresh_index = load_object(&fresh.join("sweep_index.json"));
        let fresh_mode = load_object(&fresh.join("bench_mode.json"));
        let fresh_record = self.record("cell-a", &self.bs_state())?;
        let calls = self.sb.calls()?;
        let cond = run.status == 0
            && tree_bytes(&old_dir)? == old_tree
            && level_calls(&calls).len() == 3
            && is_file(&fresh.join("verdict.json"))?
            && is_file(&fresh.join("roofline.json"))?
            && is_file(&fresh.join("level_01/run_03/measured.json"))?
            && is_text_at(&get_obj(&fresh_index, "meta")?, "config_name", "cfg-ba")?
            && crate::conformance::py_equals(
                &get(&fresh_mode, "sweep_index_generated_utc")?,
                &get(&fresh_index, "generated_utc")?,
            );
        self.ck(
            "O2 broad→sweep→judge는 새 경로만 사용하고 기존 원시 바이트·mtime을 보존",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 600),
            ]),
        )?;
        let generated = get(&fresh_index, "generated_utc")?;
        let cond =
            crate::conformance::py_equals(&get(&fresh_record, "sweep_dir")?, &st(&relative_fresh))
                && crate::conformance::py_equals(
                    &get(&fresh_record, "sweep_generated_utc")?,
                    &generated,
                )
                && generated.is_truthy();
        self.ck(
            "O2 공식 sweep 셀 기록은 새 repo-relative 경로와 실제 generated_utc를 운반",
            cond,
            &fresh_record,
        )?;
        self.explicit_path_rejections(&old_dir, &old_tree, &fresh, &generated)?;
        self.concurrent_claim(&old_dir, &old_tree, &fresh)
    }

    /// O3, O4 and O5: an existing explicit path, escapes, a symlink; and a reassembly of the new path.
    fn explicit_path_rejections(
        &mut self,
        old_dir: &Path,
        old_tree: &BTreeMap<String, (Vec<u8>, i128)>,
        fresh: &Path,
        fresh_generated: &PyValue,
    ) -> Outcome {
        let fresh_tree = tree_bytes(fresh)?;
        let state = self.bs_state();
        let state_before = read_bytes(&state)?;
        let stop_path = state.with_extension("stop.json");
        let stop_before = read_bytes(&stop_path)?;
        self.sb.reset_calls()?;
        let existing_path = with_path(arguments(&["--sweep-dir"]), fresh, &[]);
        let run = self.cell("cell-a", "cfg-ba", &empty_object(), &existing_path)?;
        let cond = run.status == 2
            && self.sb.calls()?.is_empty()
            && read_bytes(&state)? == state_before
            && read_bytes(&stop_path)? == stop_before
            && tree_bytes(fresh)? == fresh_tree
            && tree_bytes(old_dir)? == *old_tree;
        self.ck(
            "O3 기존 명시 경로는 부하·stop sidecar·셀 기록 전 거부하고 모든 원시를 보존",
            cond,
            &tup(vec![status(&run), tl(&run.stderr, 400)]),
        )?;
        let escapes: [(&str, PathBuf); 3] = [
            ("escape", self.sb.root().join("docs/sweep_bad")),
            ("nested", old_dir.join("sweep_bad")),
            (
                "parent-traversal",
                fresh
                    .parent()
                    .expect("a sweep directory has a parent")
                    .join("..")
                    .join("benchlog")
                    .join("sweep_bad"),
            ),
        ];
        for (label, bad) in escapes {
            let run = self.bash(
                "sweep_bench.sh",
                with_path(
                    arguments(&["cfg-ba", "--topology", "single", "--sweep-dir"]),
                    &bad,
                    &["--check-sweep-dir"],
                ),
            )?;
            let cond = run.status == 2 && self.sb.calls()?.is_empty();
            self.ck(
                &format!("O4 경로 {label} 거부 · 부하 없음"),
                cond,
                &tl(&run.stderr, 300),
            )?;
        }
        let link = fresh.with_file_name("sweep_link");
        std::os::unix::fs::symlink(old_dir, &link)
            .map_err(|error| Raised::os_error(&error, &crate::verify_fixture::path_text(&link)))?;
        let run = self.bash(
            "sweep_bench.sh",
            with_path(
                arguments(&["cfg-ba", "--topology", "single", "--sweep-dir"]),
                &link,
                &["--check-sweep-dir", "--reassemble-only"],
            ),
        )?;
        let cond = run.status == 2 && tree_bytes(old_dir)? == *old_tree;
        self.ck(
            "O4 재조립도 symlink 출력은 거부",
            cond,
            &tl(&run.stderr, 300),
        )?;

        self.sb.reset_calls()?;
        let run = self.bash(
            "sweep_bench.sh",
            with_path(
                arguments(&["cfg-ba", "--topology", "single", "--sweep-dir"]),
                fresh,
                &["--reassemble-only"],
            ),
        )?;
        let reassembled = get(
            &load_object(&fresh.join("sweep_index.json")),
            "generated_utc",
        )?;
        let cond = run.status == 0
            && self.sb.calls()?.is_empty()
            && tree_bytes(old_dir)? == *old_tree
            && crate::conformance::py_equals(&reassembled, fresh_generated);
        self.ck(
            "O5 명시 경로 재조립은 부하0·측정시각 승계·기존 다른 경로 불변",
            cond,
            &tup(vec![status(&run), tl(&run.stderr, 400)]),
        )
    }

    /// O6: two calls claim the same new path at once; `mkdir` lets exactly one through.
    fn concurrent_claim(
        &mut self,
        old_dir: &Path,
        old_tree: &BTreeMap<String, (Vec<u8>, i128)>,
        fresh: &Path,
    ) -> Outcome {
        let race = fresh.with_file_name("sweep_cfg-ba_race");
        let mut race_words = vec![OsString::from("bash"), self.sb.script("sweep_bench.sh")];
        race_words.extend(with_path(
            arguments(&["cfg-ba", "--topology", "single", "--sweep-dir"]),
            &race,
            &[
                "--tool",
                "guidellm",
                "--bench-budget-mib",
                "4096",
                "--backend",
                "openai",
            ],
        ));
        race_words.extend(KNOB_REASONS.map(OsString::from));
        race_words.extend(arguments(&["--levels", "1"]));
        race_words.extend(LEVEL_REASON.map(OsString::from));
        let sandbox: &Sandbox = self.sb;
        let raced = std::thread::scope(|scope| {
            let first = scope.spawn(|| sandbox.run(race_words.clone(), None));
            let second = scope.spawn(|| sandbox.run(race_words.clone(), None));
            [first.join(), second.join()]
        });
        let mut runs = Vec::new();
        for joined in raced {
            runs.push(joined.unwrap_or_else(|panic| std::panic::resume_unwind(panic))?);
        }
        let mut statuses: Vec<i64> = runs.iter().map(|run| run.status).collect();
        statuses.sort_unstable();
        let cond = statuses == [0, 2] && tree_bytes(old_dir)? == *old_tree;
        self.ck(
            "O6 같은 새 경로 경합은 mkdir로 하나만 획득 · 다른 호출 rc2 · 기존 경로 보존",
            cond,
            &rows_text(&runs),
        )
    }

    /// B3, B8, B4, B5 and B7: the cell records after the output-path group.
    pub(crate) fn broad_cells_and_legacy_state(&mut self) -> Outcome {
        let state = self.bs_state();
        let kill = dict_of(vec![("fail", dict_of(vec![("1:2", st("kill"))]))]);
        let run = self.cell("cell-b", "cfg-bb", &kill, &[])?;
        let record = self.record("cell-b", &state)?;
        let cond = run.status == 0
            && is_text_at(&record, "cell_outcome", "measured")?
            && is_text_at(&record, "bench_mode", "lite")?
            && is_text_at(&record, "downgrade_reason", "blackbox_kill")?
            && str_at_starts(&record, "downgrade_reason_source", "events(")?;
        self.ck(
            "B3★ shim 이 run 2 에 사살 이벤트를 남기고 죽는다 → 셀 기록 lite · blackbox_kill(events 출처)",
            cond,
            &tup(vec![
                get(&record, "cell_outcome")?,
                get(&record, "bench_mode")?,
                get(&record, "downgrade_reason")?,
                get(&record, "downgrade_reason_source")?,
                tl(&run.stderr, 600),
            ]),
        )?;
        self.lite_server_failed_cell(&state)?;
        self.legacy_state_without_repeats(&state)?;
        self.serve_failed_main_node(&state)
    }

    /// B8: a lite ② cell loads nothing from the earlier sweep that sits in the directory.
    fn lite_server_failed_cell(&mut self, state: &Path) -> Outcome {
        let b8 = self.sb.campaign_dir().join("sweeps").join("b8.json");
        let mut document = match crate::sweep_repeats_fixture::Sandbox::load(state) {
            Some(document) => document,
            None => {
                return Err(Raised::new(
                    "AttributeError",
                    "'NoneType' object has no attribute 'update'",
                ))
            }
        };
        update_keys(
            &mut document,
            vec![
                ("cells", PyValue::List(Vec::new())),
                ("cells_remaining", PyValue::List(vec![st("cell-a")])),
            ],
        )?;
        write_str(&b8, &json_line(&document)?)?;
        self.sb.clear_events();
        self.sb.reset_calls()?;
        self.sb.config("cfg-ba", "single")?;
        let mut words = cell_head(&b8, "cell-a", "cfg-ba");
        words.extend(measuring_tail(&[]));
        let plan = dict_of(vec![(
            "lite",
            dict_of(vec![("verdict", st("server_failed")), ("rc", int(7))]),
        )]);
        let run = self.broad(words, Some(&plan))?;
        let record = self.record("cell-a", &b8)?;
        let calls = self.sb.calls()?;
        let cond = run.status == 0
            && is_text_at(&record, "cell_outcome", "measurement_void")?
            && is_text_at(&record, "void_reason", "lite_server_failed")?
            && str_at_starts(&record, "void_reason_source", "lite_raw(")?
            && is_text_at(&record, "lite_verdict", "server_failed")?
            && is_text_at(&record, "lite_raw_status", "fresh")?
            && level_calls(&calls).is_empty();
        let picked = pick(
            &record,
            &[
                "cell_outcome",
                "void_reason",
                "lite_verdict",
                "lite_raw_status",
            ],
        )?;
        self.ck(
            "B8★ lite ② 셀 → measurement_void · 사유 lite_server_failed(lite_raw 출처) · lite_verdict 기록 · GuideLLM 레벨 0",
            cond,
            &tup(vec![status(&run), picked, self.calls_value()?, tl(&run.stderr, 500)]),
        )?;
        let cond = crate::bench_verify_support::is_none(&get(&record, "concurrency_vector")?)
            && crate::bench_verify_support::is_none(&get(
                &get_obj(&record, "measurement")?,
                "completed_requests",
            )?)
            && is_none_at(&record, "verdict_narrative")?
            && str_at_starts(&record, "sweep_artifacts_carried", "false(")?
            && is_file(
                &self
                    .sb
                    .sweep_dir("cfg-ba", "single")
                    .join("sweep_index.json"),
            )?;
        self.ck(
            "B8★ 측정 불성립 셀은 디렉터리에 남은 이전 스윕의 벡터·오류 분할·verdict 를 싣지 않는다(실측인 척 ✗)",
            cond,
            &pick(&record, &["concurrency_vector", "verdict_narrative", "sweep_artifacts_carried"])?,
        )
    }

    /// B4 and B5: a state file without a declared repeat count stops a new cell and not the read paths.
    fn legacy_state_without_repeats(&mut self, state: &Path) -> Outcome {
        let legacy = self.sb.campaign_dir().join("sweeps").join("legacy.json");
        let mut document = Sandbox::load(state)
            .ok_or_else(|| Raised::new("TypeError", "'NoneType' object is not subscriptable"))?;
        remove_from_declared_budget(&mut document, &["repeats", "repeats_source"])?;
        update_keys(
            &mut document,
            vec![("cells_remaining", PyValue::List(vec![st("cell-a")]))],
        )?;
        write_str(&legacy, &json_line(&document)?)?;
        self.sb.reset_calls()?;
        let mut words = cell_head(&legacy, "cell-a", "cfg-ba");
        words.extend(measuring_tail(&[]));
        let run = self.broad(words, None)?;
        let cond = run.status == 2
            && self.sb.calls()?.is_empty()
            && run.stderr.contains("declared_budget.repeats 미선언");
        self.ck(
            "B4★ declared_budget.repeats 없는 상태 파일 → 셀 진입 exit 2 · 부하 도구 미호출",
            cond,
            &tup(vec![
                status(&run),
                self.calls_value()?,
                tl(&run.stderr, 400),
            ]),
        )?;
        let status_run = self.broad(
            with_path(
                arguments(&["status", "--state"]),
                &legacy,
                &["--now-utc", NOW],
            ),
            None,
        )?;
        let parsed = crate::pyjson::loads_text(&crate::pystr::PyStr::new(&status_run.stdout)).ok();
        let out_md = self.sb.temporary().join("legacy_map.md");
        let mut map_words = with_path(
            arguments(&["map", "--state"]),
            &legacy,
            &["--now-utc", NOW, "--out-md"],
        );
        map_words.push(out_md.as_os_str().to_os_string());
        let map_run = self.broad(map_words, None)?;
        let parsed_status = parsed.unwrap_or_else(empty_object);
        let cond = [0, 3].contains(&status_run.status)
            && crate::bench_verify_support::is_none(&get_default(
                &get_obj(&parsed_status, "declared_budget")?,
                "repeats",
                st("x"),
            )?)
            && str_at_starts(&parsed_status, "budget_repeats_source", "absent(")?
            && map_run.status == 0
            && is_file(&out_md)?
            && read_text(&out_md)?.contains("예산 선언에 반복 수가 없다");
        self.ck(
            "B5★ 같은 상태 파일의 status(rc 0|3)·map(rc 0) 은 막히지 않는다 — 반복 수 None · 출처 absent · 지도에 경고",
            cond,
            &tup(vec![
                status(&status_run),
                tl(&status_run.stdout, 300),
                tl(&status_run.stderr, 300),
                status(&map_run),
                tl(&map_run.stderr, 300),
            ]),
        )
    }

    /// B7: a `--serve-failed` cell names this node by the owner rule (a manifest without `self_role`).
    fn serve_failed_main_node(&mut self, state: &Path) -> Outcome {
        self.sb.clear_events();
        let event = dict_of(vec![
            ("ts", st("2026-09-01T00:00:30Z")),
            ("kind", st("watchdog_kill_ack")),
            ("source", st("fixture")),
            ("mode", st("armed")),
        ]);
        write_str(
            &self.sb.root().join(FIRST_EVENTS),
            &format!("{}\n", json_line(&event)?),
        )?;
        let mut words = cell_head(state, "cell-sf", "cfg-sf");
        words.extend(arguments(&[
            "--now-utc",
            NOW,
            "--topology",
            "single",
            "--serve-failed",
            "materialize-fixture",
            "--serve-started-utc",
            "2026-09-01T00:00:00Z",
        ]));
        let run = self.broad(words, None)?;
        let record = self.record("cell-sf", state)?;
        let source = get_default(&record, "void_reason_source", st(""))?;
        let cond = run.status == 0
            && is_text_at(&record, "cell_outcome", "serve_failed")?
            && is_text_at(&record, "void_reason", "watchdog_kill_ack")?
            && str_at_starts(&record, "void_reason_source", "events(")?
            && contains(&source, "대조 노드[main=matched")?;
        self.ck(
            "B7★ --serve-failed single · self_role 없는 manifest ∧ 이 노드 구간 안 사살 → 사인 = 이벤트 · 대조 노드[main=matched]",
            cond,
            &tup(vec![
                status(&run),
                get(&record, "void_reason")?,
                get(&record, "void_reason_source")?,
                tl(&run.stderr, 500),
            ]),
        )
    }

    /// M1 and M2: the multi-node wiring from `sweep_bench` to the decision owner, and the cell closing.
    pub(crate) fn multi_node(&mut self) -> Outcome {
        self.sb.clear_events();
        write_str(
            &self.sb.root().join("output/multi/manifest.yaml"),
            "topology: multi\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nnodes:\n  - role: main\n    host: 192.0.2.10\n  - role: sub\n    host: 192.0.2.11\n",
        )?;
        self.sb.config("cfg-m", "multi")?;
        let mut words = arguments(&[
            "cfg-m",
            "--topology",
            "multi",
            "--tool",
            "guidellm",
            "--bench-budget-mib",
            "4096",
            "--backend",
            "openai",
        ]);
        words.extend(KNOB_REASONS.map(OsString::from));
        words.extend(arguments(&["--levels", "1,2"]));
        words.extend(LEVEL_REASON.map(OsString::from));
        let plan = dict_of(vec![("fail", dict_of(vec![("2:1", st("rc"))]))]);
        let mut command = vec![OsString::from("bash"), self.sb.script("sweep_bench.sh")];
        command.extend(words);
        let run = self.sb.run(command, Some(&plan))?;
        let directory = self.sb.sweep_dir("cfg-m", "multi");
        let index = load_object(&directory.join("sweep_index.json"));
        let mut record = load_object(&directory.join("bench_mode.json"));
        let scan = get_obj(&record, "events_scan")?;
        let cond = run.status == 0
            && is_text_at(&get_obj(&index, "meta")?, "measured_node", "cluster")?
            && is_text_at(&record, "bench_mode", "full")?
            && is_text_at(&record, "downgrade_correlation", "not_scanned")?
            && crate::conformance::py_equals(
                &get(&get_obj(&scan, "plan")?, "nodes")?,
                &PyValue::List(vec![st("main"), st("sub")]),
            )
            && is_text_at(&get_obj(&scan, "plan")?, "self_node", "main")?
            && crate::conformance::py_equals(
                &node_field(&record, "sub", "status")?,
                &st("not_scanned"),
            );
        self.ck(
            "M1★ multi 스윕 직후(서브 미회수) → full ∧ not_scanned · meta.measured_node=cluster · 계획 = manifest 노드 전부(self_role 없이 이 노드 main)",
            cond,
            &tup(vec![
                status(&run),
                get(&get_obj(&index, "meta")?, "measured_node")?,
                get(&record, "bench_mode")?,
                get(&record, "downgrade_correlation")?,
                get(&scan, "plan")?,
                tl(&run.stderr, 600),
            ]),
        )?;
        let stop = or_default(
            get(&get_obj(&index, "repetition")?, "stop")?,
            empty_object(),
        );
        let window_end = strptime(&or_default(
            get(&stop, "window_end_utc")?,
            st("2000-01-01T00:00:00Z"),
        ))?;
        let after = |seconds: i64| -> Result<String, Raised> {
            Ok(window_end.plus_seconds(seconds)?.format())
        };
        let mirror = self
            .sb
            .root()
            .join("sync_staging/sub_docs/logs/sub/events/2026-09.jsonl");
        let kill = dict_of(vec![
            ("ts", st(&after(10)?)),
            ("kind", st("watchdog_kill_ack")),
            ("source", st("fixture")),
            ("mode", st("armed")),
        ]);
        let stop_event = dict_of(vec![
            ("ts", st(&after(120)?)),
            ("kind", st("serve_stop")),
            ("source", st("fixture")),
        ]);
        write_str(
            &mirror,
            &format!("{}\n{}\n", json_line(&kill)?, json_line(&stop_event)?),
        )?;
        let renew = dict_of(vec![
            ("ts", st(&after(120)?)),
            ("kind", st("budget_renew")),
            ("source", st("fixture")),
        ]);
        write_str(
            &self.sb.root().join(FIRST_EVENTS),
            &format!("{}\n", json_line(&renew)?),
        )?;
        let reassemble = arguments(&[
            "cfg-m",
            "--topology",
            "multi",
            "--backend",
            "openai",
            "--reassemble-only",
        ]);
        let run = self.bash("sweep_bench.sh", reassemble.clone())?;
        record = load_object(&directory.join("bench_mode.json"));
        let cond = run.status == 0
            && is_text_at(&stop, "kind", "clamp")?
            && is_text_at(&record, "bench_mode", "full")?
            && is_text_at(&record, "downgrade_correlation", "unavailable")?
            && crate::conformance::py_equals(
                &node_field(&record, "sub", "status")?,
                &st("skew_undeclared"),
            )
            && crate::conformance::py_equals(&node_field(&record, "main", "status")?, &st("miss"));
        self.ck(
            "M1★ 회수 미러가 창을 덮고 허용오차 미선언 → full ∧ unavailable · sub=skew_undeclared(창 끝+10s 사살을 넓혀 잡지 않는다)",
            cond,
            &tup(vec![
                status(&run),
                stop.clone(),
                get(&record, "downgrade_correlation")?,
                get(&record, "events_scan")?,
                tl(&run.stderr, 400),
            ]),
        )?;
        let mut declared = reassemble;
        declared.extend(arguments(&[
            "--cross-node-tolerance-s",
            "15",
            "--cross-node-tolerance-source",
            "fixture-clock",
        ]));
        let run = self.bash("sweep_bench.sh", declared)?;
        record = load_object(&directory.join("bench_mode.json"));
        let sub = node_entry(&record, "sub")?;
        let cond = run.status == 0
            && is_int_at(
                &get_obj(&record, "events_scan")?,
                "cross_node_tolerance_s",
                15,
            )?
            && is_text_at(&sub, "tolerance_source", "declared(fixture-clock)")?
            && is_text_at(&sub, "status", "matched")?
            && is_text_at(&record, "bench_mode", "lite")?
            && is_text_at(&record, "downgrade_reason", "blackbox_kill")?;
        self.ck(
            "M1★ --reassemble-only 에 선언된 허용오차 15s·출처 → 판정 기록에 실린다 · 창 끝+10s 서브 사살 → lite · blackbox_kill",
            cond,
            &tup(vec![
                status(&run),
                get(&record, "events_scan")?,
                get(&record, "bench_mode")?,
                get(&record, "downgrade_reason")?,
                tl(&run.stderr, 400),
            ]),
        )?;
        self.serve_failed_multi(&mirror)
    }

    /// M2: a multi-node `--serve-failed` cell hands the declared tolerance to the closing classification.
    fn serve_failed_multi(&mut self, mirror: &Path) -> Outcome {
        self.sb.clear_events();
        let _ = std::fs::remove_dir_all(self.sb.root().join("sync_staging"));
        let kill = dict_of(vec![
            ("ts", st("2026-09-01T00:00:00Z")),
            ("kind", st("watchdog_kill_ack")),
            ("source", st("fixture")),
            ("mode", st("armed")),
        ]);
        write_str(mirror, &format!("{}\n", json_line(&kill)?))?;
        let state = self.sb.campaign_dir().join("sweeps").join("bs_multi.json");
        let init = self.broad(
            init_command("bsm", &state, "cell-m1,cell-m2", "multi"),
            None,
        )?;
        let closing = |key: &str, extra: &[&str]| {
            let mut words = cell_head(&state, key, "cfg-mf");
            words.extend(arguments(&[
                "--now-utc",
                NOW,
                "--topology",
                "multi",
                "--serve-failed",
                "materialize-fixture",
                "--serve-started-utc",
                "2026-09-01T00:00:10Z",
            ]));
            words.extend(arguments(extra));
            words
        };
        let run = self.broad(
            closing(
                "cell-m1",
                &[
                    "--cross-node-tolerance-s",
                    "15",
                    "--cross-node-tolerance-source",
                    "fixture-clock",
                ],
            ),
            None,
        )?;
        let record = self.record("cell-m1", &state)?;
        let cond = init.status == 0
            && run.status == 0
            && is_text_at(&record, "void_reason", "watchdog_kill_ack")?
            && crate::bench_verify_support::py_str(&get(&record, "void_reason_source")?)
                .contains("sub(원격)=matched");
        self.ck(
            "M2★ broad_search multi --serve-failed 가 선언된 허용오차를 셀 종결 분류에 넘긴다(구간 시작 10s 전 서브 사살 → 사인 = 이벤트)",
            cond,
            &tup(vec![
                status(&init),
                status(&run),
                get(&record, "void_reason")?,
                get(&record, "void_reason_source")?,
                tl(&run.stderr, 500),
            ]),
        )?;
        let run = self.broad(closing("cell-m2", &[]), None)?;
        let record = self.record("cell-m2", &state)?;
        let cond = run.status == 0
            && is_none_at(&record, "void_reason")?
            && str_at_starts(&record, "void_reason_source", "not-scanned(")?;
        self.ck(
            "M2★ 음성대조: 같은 사실에 허용오차 선언 없음 → 사인 null · not-scanned(창을 넓히지 않는다 · 낡은 미러는 not_covered)",
            cond,
            &tup(vec![
                status(&run),
                get(&record, "void_reason")?,
                get(&record, "void_reason_source")?,
                tl(&run.stderr, 500),
            ]),
        )
    }

    /// `campaign_init.sh --cell-set KEY --outcome OUTCOME --sweep-state STATE --utc NOW` in the sandbox.
    fn cell_set(&self, key: &str, outcome: &str, state: &Path) -> Result<TextRun, Raised> {
        let mut command = vec![
            OsString::from("bash"),
            self.sb
                .root()
                .join(".claude/skills/terraforming_node/scripts/campaign_init.sh")
                .into_os_string(),
        ];
        command.extend(arguments(&["--cell-set", key, "--outcome", outcome, "--sweep-state"]));
        command.push(state.as_os_str().to_os_string());
        command.extend(arguments(&["--utc", NOW]));
        self.sb.run(command, None)
    }

    /// C1–C4 (plan_26111708 B L12): a standalone `sweep_bench.sh --campaign-cell` run leaves, in the campaign's `sweeps/`
    /// (derived from `campaigns/ACTIVE`), the cell record `campaign_init --cell-set --sweep-state` reads — on both
    /// topologies; outside a campaign it writes nothing; a failed measurement is recorded as the classifier closes it.
    pub(crate) fn campaign_cell_records(&mut self) -> Outcome {
        self.sb.declare(Some(3))?;
        self.sb.clear_events();
        self.sb.reset_calls()?;
        let sweeps = self.sb.campaign_dir().join("sweeps");
        let plan = tpot_plan(&[25.0, 25.5, 24.5]);
        let run = self.sweep("cell-a", &["--campaign-cell", "cell-a"], Some(&plan), "1")?;
        let state = sweeps.join("sweep_cell-a.json");
        let record = self.record("cell-a", &state)?;
        let document = load_object(&state);
        let cond = run.status == 0
            && is_text_at(&record, "cell_outcome", "measured")?
            && is_text_at(&record, "sweep_dir", "output/single/benchlog/sweep_cell-a")?
            && is_none_at(&record, "axis_citation")?
            && str_at_starts(&record, "axis_citation_source", "absent(")?
            && is_text_at(&document, "origin", "sweep_bench --campaign-cell")?
            && is_none_at(&document, "declared_budget")?
            && str_at_starts(&document, "declared_budget_source", "absent(")?;
        self.ck(
            "C1 sweep_bench --campaign-cell → 캠페인 sweeps/ 에 셀 기록(measured · sweep_dir · 선언 없는 칸은 null 과 출처)",
            cond,
            &tup(vec![status(&run), record.clone(), tl(&run.stdout, 400), tl(&run.stderr, 400)]),
        )?;
        let set = self.cell_set("cell-a", "measured", &state)?;
        let measurement = get_obj(
            &load_object(&self.sb.campaign_dir().join("cells/cell-a/cell.status.json")),
            "measurement",
        )?;
        let source = format!("sweep:campaigns/{CAMPAIGN}/sweeps/sweep_cell-a.json#cells[cell_key=cell-a]");
        let cond = set.status == 0
            && crate::conformance::py_equals(&get(&measurement, "measurement_ok")?, &PyValue::Bool(true))
            && str_at_starts(&measurement, "source", &source)?
            && is_text_at(&measurement, "sweep_dir", "output/single/benchlog/sweep_cell-a")?;
        self.ck(
            "C1 --cell-set --sweep-state 가 그 기록을 받는다(측정값·출력 경로가 손 없이 셀 상태로)",
            cond,
            &tup(vec![status(&set), measurement, tl(&set.stderr, 400)]),
        )?;
        let active = self.sb.root().join("campaigns/ACTIVE");
        write_str(&active, "_bootstrap\n")?;
        let run = self.sweep("cell-b", &["--campaign-cell", "cell-b"], Some(&plan), "1");
        write_str(&active, &format!("{CAMPAIGN}\n"))?;
        let run = run?;
        let wrote = exists(&self.sb.root().join("campaigns/_bootstrap/sweeps/sweep_cell-b.json"))?
            || exists(&sweeps.join("sweep_cell-b.json"))?;
        let cond = run.status == 0 && !wrote && run.stdout.contains("캠페인 밖(ACTIVE=_bootstrap)");
        self.ck(
            "C2★ 음성대조: ACTIVE=_bootstrap 이면 sweep 기록을 쓰지 않는다(writer no-op · 측정은 그대로)",
            cond,
            &tup(vec![status(&run), tl(&run.stdout, 400), tl(&run.stderr, 300)]),
        )?;
        self.sb.reset_calls()?;
        let failing = dict_of(vec![("fail", dict_of(vec![("1:1", st("measurement"))]))]);
        let run = self.sweep("cell-b", &["--campaign-cell", "cell-b"], Some(&failing), "1")?;
        let record = self.record("cell-b", &sweeps.join("sweep_cell-b.json"))?;
        let phase = load_object(&self.sb.campaign_dir().join("phases/main/bench.status.json"));
        let cond = run.status != 0
            && run.status != 2
            && is_text_at(&record, "cell_outcome", "measurement_void")?
            && is_none_at(&record, "sweep_dir")?
            && is_text_at(&phase, "state", "failed")?;
        self.ck(
            "C3★ 판정점 측정 실패 rc → 셀 기록 measurement_void(분류기 판정 · 출력 경로 없음) · bench phase failed",
            cond,
            &tup(vec![status(&run), pick(&record, &["cell_outcome", "void_reason", "sweep_dir"])?, tl(&run.stderr, 400)]),
        )?;
        // the multi topology: the same record, derived from the same declaration, read for a cell of a multi node
        let declaration = self.sb.campaign_dir().join("campaign.yaml");
        write_str(&declaration, &read_text(&declaration)?.replace("\"single\"", "\"multi\""))?;
        self.sb.config("cell-b", "multi")?;
        let mut words = arguments(&[
            "cell-b",
            "--topology",
            "multi",
            "--tool",
            "guidellm",
            "--bench-budget-mib",
            "4096",
            "--backend",
            "openai",
        ]);
        words.extend(KNOB_REASONS.map(OsString::from));
        words.extend(arguments(&["--levels", "1"]));
        words.extend(LEVEL_REASON.map(OsString::from));
        words.extend(arguments(&[
            "--sweep-dir",
            "output/multi/benchlog/sweep_cell-b_multi",
            "--campaign-cell",
            "cell-b",
        ]));
        let mut command = vec![OsString::from("bash"), self.sb.script("sweep_bench.sh")];
        command.extend(words);
        let run = self.sb.run(command, Some(&plan))?;
        let state = sweeps.join("sweep_cell-b_multi.json");
        let record = self.record("cell-b", &state)?;
        let set = self.cell_set("cell-b", "measured", &state)?;
        let measurement = get_obj(
            &load_object(&self.sb.campaign_dir().join("cells/cell-b/cell.status.json")),
            "measurement",
        )?;
        self.sb.declare(Some(3))?;
        let cond = run.status == 0
            && is_text_at(&record, "cell_outcome", "measured")?
            && is_text_at(&load_object(&state), "topology", "multi")?
            && set.status == 0
            && is_text_at(&measurement, "sweep_dir", "output/multi/benchlog/sweep_cell-b_multi")?;
        self.ck(
            "C4 multi 도 같은 자리(campaigns/ACTIVE 파생)에 기록하고 --cell-set 이 받는다(--sweep-dir 명시 경로)",
            cond,
            &tup(vec![status(&run), status(&set), measurement, tl(&run.stderr, 300), tl(&set.stderr, 300)]),
        )
    }
}

/// `{"tpot": {"1": [..]}}`: the shim plan that sets the first level's per-run TPOT.
fn tpot_plan(values: &[f64]) -> PyValue {
    dict_of(vec![(
        "tpot",
        dict_of(vec![(
            "1",
            PyValue::List(values.iter().map(|value| float(*value)).collect()),
        )]),
    )])
}

fn relative_text(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// `{k: record.get(k) for k in keys}`.
fn pick(record: &PyValue, keys: &[&str]) -> Result<PyValue, Raised> {
    let mut picked = PyDict::new();
    for key in keys {
        picked.set(key, get(record, key)?);
    }
    Ok(PyValue::Dict(picked))
}

/// `document.update(key=value, ...)`: existing keys keep their place, new ones are appended.
fn update_keys(document: &mut PyValue, entries: Vec<(&str, PyValue)>) -> Result<(), Raised> {
    match document {
        PyValue::Dict(map) => {
            for (key, value) in entries {
                map.set(key, value);
            }
            Ok(())
        }
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'update'", other.type_name()),
        )),
    }
}

/// `document["declared_budget"].pop(key)` for each key: a missing key is a `KeyError`.
fn remove_from_declared_budget(document: &mut PyValue, keys: &[&str]) -> Result<(), Raised> {
    let PyValue::Dict(map) = document else {
        return Err(Raised::new(
            "TypeError",
            &format!("'{}' object is not subscriptable", document.type_name()),
        ));
    };
    let Some(budget) = map.get_mut("declared_budget") else {
        return Err(Raised::new("KeyError", "'declared_budget'"));
    };
    for key in keys {
        *budget = pop_required(budget, key)?;
    }
    Ok(())
}

/// `dict.pop(key)` on a loaded object: the object without that key (the order of the rest kept); a missing key is
/// a `KeyError`, a value that is no dict has no `pop`.
fn pop_required(value: &PyValue, key: &str) -> Result<PyValue, Raised> {
    match value.peel() {
        PyValue::Dict(map) if map.contains_key(key) => {
            crate::verify_fixture::without_key(value.clone(), key)
        }
        PyValue::Dict(_) => Err(Raised::new(
            "KeyError",
            &crate::pystr::PyStr::new(key).repr(),
        )),
        other => Err(Raised::new(
            "AttributeError",
            &format!("'{}' object has no attribute 'pop'", other.type_name()),
        )),
    }
}

/// The entry of a node in `events_scan.nodes`: `next((e for e in ... if e.get("node") == node), {})`.
fn node_entry(record: &PyValue, node: &str) -> Result<PyValue, Raised> {
    let nodes = or_default(
        get(&get_obj(record, "events_scan")?, "nodes")?,
        PyValue::List(Vec::new()),
    );
    for entry in iterate_values(&nodes)? {
        if is_text_at(&entry, "node", node)? {
            return Ok(entry);
        }
    }
    Ok(empty_object())
}

/// `mstatus(record, node)`-style lookup: the field of the node's entry, `None` when the node is not listed.
fn node_field(record: &PyValue, node: &str, field: &str) -> Result<PyValue, Raised> {
    let nodes = or_default(
        get(&get_obj(record, "events_scan")?, "nodes")?,
        PyValue::List(Vec::new()),
    );
    for entry in iterate_values(&nodes)? {
        if is_text_at(&entry, "node", node)? {
            return get(&entry, field);
        }
    }
    Ok(PyValue::None)
}
