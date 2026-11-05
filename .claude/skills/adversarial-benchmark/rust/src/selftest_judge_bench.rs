//! `selftest_judge_bench`: the isolated check of what `judge_bench.sh` hands to the judge. A Rust port of
//! `selftest_judge_bench.py` (the body of `judge_bench.sh --self-test`).
//!
//! The deployed shell script is run for real: accept-length inheritance, the speculative-decoding declaration, the
//! missing-value reason and the suppression of certificate publication are properties of the shell entry path, so a
//! unit test of the judge alone would not show that they stand in the actual chain.
//!
//! Isolation: a temporary git checkout holds byte copies of `judge_bench.sh`, `roofline.sh`, `verdict_rule.sh` and
//! the two release binaries behind them at the same relative paths, so the script sees only its own checkout. The
//! certificate publisher is **not** copied; a stub that only records that it was called stands in for it (the real one
//! writes `docs/benchmark/` and the campaign evidence pointers). The model is one safetensors file that is only a
//! header declaring 2.73e9 bytes (273 GB/s GB10: `R_fp` = 100 tokens/s exactly).
//!
//! Cases (a negative control must be blocked or turned over): J1 inheritance of a measured accept length · J2 explicit
//! declared value · J2b the explicit value shadows the measured one · J3 speculation off · J4 speculation on with no
//! value (no automatic 1.0) · J4b invalid measured value · J4c invalid explicit value (exit 2) · J4d the same in
//! place (verdict pair untouched, no partial file) · J5/J5b no declaration fingerprint · J6/J6b suppressed
//! publication (`--out-dir`, `--no-publish`) · J7 publication reached without suppression · J12 an unfinished sweep is
//! not judged · J8 `--dry-run` · J10 the roofline combination contract · J11/J11b declaration off against measurement.

use crate::bench_verify_support::{
    and_then_value, chain, dict_of, eq, float, get_eq, get_eq_text, get_or_empty,
    greater_than_zero, int, is_none, st, starts_with, tuple_text, BracketChecks, DetailStyle,
};
use crate::captured_process::ChildSettings;
use crate::conformance::{py_equals, Raised};
use crate::dataset_prep::Console;
use crate::pyvalue::PyValue;
use crate::verify_fixture::{
    contains, exists, get, head, is_file, load_json, or_default, read_bytes, read_text,
    repo_root_unknown_message, run_checked_inherited, run_text_within, tail, words, write_bytes,
    write_str, write_text, IsolatedCheckout, TextRun, BENCHMARK_SCRIPTS,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The program's name in its messages.
pub const PROGRAM: &str = "selftest_judge_bench";

const COPIED_SCRIPTS: [&str; 3] = ["judge_bench.sh", "roofline.sh", "verdict_rule.sh"];
const COPIED_BINARIES: [&str; 2] = ["roofline", "verdict_rule"];
const RELEASE_DIRECTORY: &str = ".claude/skills/adversarial-benchmark/rust/target/release";
/// The publisher the sandbox replaces with a stub.
const PUBLISHER: &str = "publish_benchmark_record.py";
const STUB_SENTINEL: &str = "SELFTEST_JUDGE_BENCH_PUBLISH_STUB";
/// `timeout=` of the judge calls and of the roofline calls.
const JUDGE_SECONDS: u64 = 180;
const ROOFLINE_SECONDS: u64 = 120;

/// The publisher stub: not the real publisher; it touches neither `docs/` nor `campaigns/` and leaves a mark.
fn publisher_stub() -> String {
    format!(
        "#!/usr/bin/env python3\n\
# {STUB_SENTINEL} — 실제 발행기가 아니다(테스트 평면 격리). docs/·campaigns/ 에 닿지 않고 호출 사실만 남긴다.\n\
import os, sys\n\
root = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), \"..\", \"..\", \"..\", \"..\"))\n\
with open(os.path.join(root, \"PUBLISH_CALLED\"), \"a\", encoding=\"utf-8\") as f:\n\
\x20   f.write(\" \".join(sys.argv[1:]) + \"\\n\")\n"
    )
}

/// What the sweep fixture declares about speculative decoding.
#[derive(Clone, Copy)]
enum SpecDeclaration {
    /// No fingerprint at all (a sweep from before declarations were recorded).
    Missing,
    Declared {
        state: &'static str,
        k: Option<i64>,
    },
}

struct Sandbox {
    checkout: IsolatedCheckout,
}

impl Sandbox {
    fn build(repo_root: &Path) -> Result<Sandbox, Raised> {
        let checkout = IsolatedCheckout::create("selftest-judge-bench.")?;
        let root = checkout.root().to_path_buf();
        let scripts: Vec<String> = COPIED_SCRIPTS
            .iter()
            .map(|name| format!("{BENCHMARK_SCRIPTS}/{name}"))
            .collect();
        let binaries: Vec<String> = COPIED_BINARIES
            .iter()
            .map(|name| format!("{RELEASE_DIRECTORY}/{name}"))
            .collect();
        let relative: Vec<&str> = scripts
            .iter()
            .chain(binaries.iter())
            .map(String::as_str)
            .collect();
        checkout.copy_from(repo_root, &relative)?;
        let scripts_dir = root.join(BENCHMARK_SCRIPTS);
        write_str(&scripts_dir.join(PUBLISHER), &publisher_stub())?;
        write_str(
            &scripts_dir.join("publish_benchmark_record.sh"),
            "#!/bin/bash\nexec python3 \"$(dirname \"$0\")/publish_benchmark_record.py\" \"$@\"\n",
        )?;
        run_checked_inherited(&[
            OsString::from("git"),
            OsString::from("init"),
            OsString::from("-q"),
            root.clone().into_os_string(),
        ])?;
        write_str(
            &root.join("output/single/manifest.yaml"),
            "topology: single\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\n",
        )?;
        let model = root.join("models/fx-dense");
        write_text(
            &model.join("config.json"),
            &crate::verify_fixture::dumps(&dict_of(vec![
                ("hidden_size", int(64)),
                ("num_hidden_layers", int(2)),
            ]))?,
        )?;
        // Header only: 2.73e9 bytes declared, so t_weight = 2.73e9 / 273e9 = 10 ms and R_fp = 100.
        let header =
            r#"{"w": {"dtype": "F16", "shape": [1365000000], "data_offsets": [0, 2730000000]}}"#;
        let mut weights = u64::try_from(header.len())
            .expect("a header length fits 64 bits")
            .to_le_bytes()
            .to_vec();
        weights.extend_from_slice(header.as_bytes());
        write_bytes(&model.join("model.safetensors"), &weights)?;
        Ok(Sandbox { checkout })
    }

    fn root(&self) -> &Path {
        self.checkout.root()
    }

    fn scripts_dir(&self) -> PathBuf {
        self.root().join(BENCHMARK_SCRIPTS)
    }

    fn model(&self) -> PathBuf {
        self.root().join("models/fx-dense")
    }

    fn manifest(&self) -> PathBuf {
        self.root().join("output/single/manifest.yaml")
    }

    fn marker(&self) -> PathBuf {
        self.root().join("PUBLISH_CALLED")
    }

    fn sweep_dir(&self, config: &str) -> PathBuf {
        self.root()
            .join("output/single/benchlog")
            .join(format!("sweep_{config}"))
    }

    fn make_sweep(
        &self,
        config: &str,
        spec: SpecDeclaration,
        accept_len: Option<f64>,
        decode_tps: f64,
    ) -> Result<PathBuf, Raised> {
        let directory = self.sweep_dir(config);
        let mut meta = vec![
            ("model_path", st(&self.model().to_string_lossy())),
            ("tensor_parallel_size", st("1")),
            ("config_name", st(config)),
            ("model", st("fx-dense")),
            ("gpu_model", st("NVIDIA GB10")),
        ];
        if let SpecDeclaration::Declared { state, k } = spec {
            meta.push(("spec_declared", st(state)));
            meta.push(("spec_declared_k", k.map_or(PyValue::None, int)));
            meta.push(("spec_declared_source", st("fixture")));
        }
        let index = dict_of(vec![
            ("config", st(config)),
            ("meta", dict_of(meta)),
            ("generated_utc", st("2026-01-01T00:00:00Z")),
            (
                "levels",
                PyValue::List(vec![dict_of(vec![("level", int(1)), ("status", st("ok"))])]),
            ),
        ]);
        write_text(
            &directory.join("sweep_index.json"),
            &crate::verify_fixture::dumps(&index)?,
        )?;
        let speculative = accept_len.is_some_and(|value| value > 1.05);
        let measured = dict_of(vec![
            ("decode_tps", float(decode_tps)),
            ("spec_on", PyValue::Bool(speculative)),
            ("accept_len", accept_len.map_or(PyValue::None, float)),
            ("completed", int(16)),
            ("failed", int(0)),
            ("measurement_ok", PyValue::Bool(true)),
        ]);
        write_text(
            &directory.join("level_01/measured.json"),
            &crate::verify_fixture::dumps(&measured)?,
        )?;
        Ok(directory)
    }

    /// `bash judge_bench.sh <config> --topology single <args>` in the checkout.
    fn judge(&self, config: &str, args: &[&str]) -> Result<TextRun, Raised> {
        let mut command: Vec<OsString> = vec![
            OsString::from("bash"),
            self.scripts_dir().join("judge_bench.sh").into_os_string(),
            OsString::from(config),
            OsString::from("--topology"),
            OsString::from("single"),
        ];
        command.extend(words(args));
        run_text_within(&command, &self.child_settings(), JUDGE_SECONDS)
    }

    fn child_settings(&self) -> ChildSettings {
        ChildSettings {
            current_dir: Some(self.root().to_path_buf()),
            ..ChildSettings::default()
        }
    }

    /// The loader of the sandbox: any unreadable or unparsable file is absence.
    fn load(path: &Path) -> PyValue {
        load_json(path).unwrap_or(PyValue::None)
    }
}

fn status_is(run: &TextRun, expected: i64) -> bool {
    run.status == expected
}

fn marker_text(sandbox: &Sandbox) -> Result<String, Raised> {
    if exists(&sandbox.marker())? {
        read_text(&sandbox.marker())
    } else {
        Ok(String::new())
    }
}

/// The program. Returns the exit status; `repo_root` is the repository the deployed files are copied from.
pub fn main(repo_root: Option<&Path>, console: &mut dyn Console) -> Result<i32, Raised> {
    let mut checks = BracketChecks::new(console, DetailStyle::Plain);
    let Some(repo_root) = repo_root else {
        checks.write_error(&format!("{}\n", repo_root_unknown_message(PROGRAM)));
        return Ok(2);
    };
    let canonical = std::fs::canonicalize(repo_root).unwrap_or_else(|_| repo_root.to_path_buf());
    let sandbox = Sandbox::build(repo_root)?;
    let outcome = run_cases(&mut checks, &sandbox, &canonical);
    // Leaving the `with` block removes the directory first; a cleanup failure replaces the original exception.
    let Sandbox { checkout, .. } = sandbox;
    checkout.close()?;
    outcome?;
    if !checks.failures().is_empty() {
        let line = format!(
            "[judge_bench --self-test] FAIL {} 건: {}\n",
            checks.failures().len(),
            checks.failure_list_repr()
        );
        checks.write_error(&line);
        return Ok(1);
    }
    checks.print(
        "[judge_bench --self-test] OK — J1~J12 전부 통과(승계·명시·spec off·결손·원자적 교체·조합 계약·발행 억제 음성대조 포함)",
    )?;
    Ok(0)
}

const EXPLORE: [&str; 6] = [
    "--authority",
    "explore",
    "--reference-tps",
    "110",
    "--e-search",
    "hit",
];
const EXPLICIT_PASS: [&str; 4] = ["--authority", "explicit", "--target-tps", "90"];

fn with(base: &[&str], extra: &[&str]) -> Vec<String> {
    base.iter()
        .chain(extra.iter())
        .map(|word| (*word).to_string())
        .collect()
}

fn borrowed(items: &[String]) -> Vec<&str> {
    items.iter().map(String::as_str).collect()
}

fn files_equal_or_absent(directory: &Path, names: &[&str]) -> Result<Vec<Option<Vec<u8>>>, Raised> {
    names
        .iter()
        .map(|name| {
            let path = directory.join(name);
            if exists(&path)? {
                read_bytes(&path).map(Some)
            } else {
                Ok(None)
            }
        })
        .collect()
}

#[allow(clippy::too_many_lines)]
fn run_cases(checks: &mut BracketChecks<'_>, sandbox: &Sandbox, repo: &Path) -> Result<(), Raised> {
    let on_k3 = SpecDeclaration::Declared {
        state: "on",
        k: Some(3),
    };
    let off = SpecDeclaration::Declared {
        state: "off",
        k: None,
    };

    // ── J1 inheritance of the measured accept length ─────────────────────────────────────
    let directory = sandbox.make_sweep("j1", on_k3, Some(2.0), 105.0)?;
    let run = sandbox.judge("j1", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    checks.check("J1 승계 rc=0", status_is(&run, 0), || {
        tail(&run.stderr, 400)
    })?;
    let cond = roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "measured")?
        && get_eq(&roofline, "accept_len", &float(2.0))?
        && starts_with(
            &or_default(get(&roofline, "accept_len_evidence")?, st("")),
            "level_01/measured.json",
        )?;
    checks.check(
        "J1 roofline accept_len_source=measured · 근거=판정 레벨 measured.json",
        cond,
        || roofline.repr(),
    )?;
    let cond = roofline.is_truthy()
        && get_eq(&roofline, "R_fp", &float(100.0))?
        && get_eq(&roofline, "R_token", &float(200.0))?
        && get_eq(&roofline, "expected_achievable", &float(70.0))?;
    checks.check(
        "J1 R_fp 100 · R_token 200 · expected_achievable 70 (accept_len 이 합격선에도 곱해진다)",
        cond,
        || -> Result<String, Raised> {
            Ok(and_then_value(&roofline, || {
                Ok(dict_of(
                    ["R_fp", "R_token", "expected_achievable"]
                        .iter()
                        .map(|key| Ok((*key, get(&roofline, key)?)))
                        .collect::<Result<Vec<_>, Raised>>()?,
                ))
            })?
            .repr())
        },
    )?;
    let cond = verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "PASS")?
        && is_none(&get(&verdict, "reason_code")?)
        && eq(
            &chain(&verdict, &["rubric", "physical", "limit_tps"])?,
            &float(200.0),
        )
        && eq(
            &chain(&verdict, &["rubric", "spec_axis", "declared"])?,
            &st("on"),
        )
        && greater_than_zero(&chain(&verdict, &["rubric", "floor"])?)?
        && !is_none(&chain(&verdict, &["rubric", "ratio_M_over_primary"])?);
    checks.check(
        "J1 verdict PASS · R_token 상한 · spec_declared=on 승계",
        cond,
        || -> Result<String, Raised> {
            Ok(and_then_value(&verdict, || {
                Ok(dict_of(vec![
                    ("verdict", get(&verdict, "verdict")?),
                    (
                        "physical",
                        get(&get_or_empty(&verdict, "rubric")?, "physical")?,
                    ),
                ]))
            })?
            .repr())
        },
    )?;
    checks.check(
        "J1 explore PASS 는 발행 분기에 들지 않는다",
        !exists(&sandbox.marker())?,
        String::new,
    )?;
    // J9 isolation: the copy sees only its own checkout.
    let manifest_text = sandbox.manifest().to_string_lossy().into_owned();
    let cond = run.stdout.contains(&manifest_text)
        && !run
            .stdout
            .contains(&format!("{}/output", repo.to_string_lossy()));
    checks.check(
        "J9 argv 의 manifest 는 임시 저장소 안이다",
        cond,
        || head(&run.stdout, 300),
    )?;

    // ── J2 explicit (declared) value ─────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j2", on_k3, None, 105.0)?;
    let run = sandbox.judge("j2", &borrowed(&with(&EXPLORE, &["--accept-len", "1.5"])))?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "declared")?
        && get_eq_text(&roofline, "accept_len_evidence", "argv(--accept-len)")?
        && get_eq(&roofline, "R_token", &float(150.0))?
        && verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "PASS")?
        && eq(
            &chain(&verdict, &["rubric", "spec_axis", "accept_len_source"])?,
            &st("declared"),
        );
    checks.check(
        "J2 declared — 출처가 사람 명시로 드러나고 R_token 150 으로 PASS",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_source"))?,
                and_then_value(&verdict, || get(&verdict, "verdict"))?,
            ]))
        },
    )?;

    // ── J2b the explicit value shadows the measured one ──────────────────────────────────
    let directory = sandbox.make_sweep("j2b", on_k3, Some(2.0), 105.0)?;
    let run = sandbox.judge("j2b", &borrowed(&with(&EXPLORE, &["--accept-len", "1.5"])))?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let evidence = or_default(get(&roofline, "accept_len_evidence")?, st(""));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "declared")?
        && get_eq(&roofline, "R_token", &float(150.0))?
        && contains(&evidence, "shadowed measured=2.0(level_01/measured.json)")?;
    checks.check(
        "J2b 명시 1.5 가 실측 2.0 을 덮으면 source=declared · R_token 150 · 근거에 shadowed measured=2.0",
        cond,
        || -> Result<String, Raised> { Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_evidence"))?,
                st(&tail(&run.stderr, 300)),
            ])) },
    )?;

    // ── J3 speculation off ───────────────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j3", off, None, 60.0)?;
    let run = sandbox.judge("j3", &["--authority", "explore", "--e-search", "empty"])?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "declared-absent")?
        && py_equals(&get(&roofline, "R_token")?, &get(&roofline, "R_fp")?)
        && py_equals(&get(&roofline, "R_fp")?, &float(100.0))
        && get_eq(&roofline, "expected_achievable", &float(35.0))?
        && verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "PASS")?
        && is_none(&get(&verdict, "reason_code")?)
        && starts_with(
            &chain(&verdict, &["rubric", "physical", "limit_source"])?,
            "R_fp",
        )?
        && eq(
            &chain(&verdict, &["rubric", "spec_axis", "missing"])?,
            &PyValue::Bool(false),
        );
    checks.check(
        "J3 spec off — declared-absent · R_token==R_fp · R_fp 상한 PASS(결손 아님)",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_source"))?,
                and_then_value(&verdict, || get(&verdict, "verdict"))?,
                st(&tail(&run.stderr, 300)),
            ]))
        },
    )?;

    // ── J4 speculation on with no value ──────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j4", on_k3, None, 105.0)?;
    let run = sandbox.judge("j4", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "absent")?
        && verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "NEEDS_RUBRIC")?
        && get_eq_text(&verdict, "failure_axis", "establish")?
        && get_eq_text(&verdict, "reason_code", "SPEC_ACCEPT_LEN_MISSING")?
        && is_none(&chain(&verdict, &["rubric", "floor"])?)
        && run.stdout.contains("reason_code=SPEC_ACCEPT_LEN_MISSING");
    checks.check(
        "J4★ spec on ∧ 결손 → NEEDS_RUBRIC · SPEC_ACCEPT_LEN_MISSING (1.0 자동 대체 ✗)",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_source"))?,
                and_then_value(&verdict, || get(&verdict, "verdict"))?,
                and_then_value(&verdict, || get(&verdict, "reason_code"))?,
            ]))
        },
    )?;

    // ── J4b invalid measured value ───────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j4b", on_k3, Some(0.5), 105.0)?;
    let run = sandbox.judge("j4b", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let evidence = or_default(get(&roofline, "accept_len_evidence")?, st(""));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "absent")?
        && contains(&evidence, "invalid")?
        && verdict.is_truthy()
        && get_eq_text(&verdict, "reason_code", "SPEC_ACCEPT_LEN_MISSING")?;
    checks.check(
        "J4b★ 무효 실측 0.5 → absent(근거에 invalid) · SPEC_ACCEPT_LEN_MISSING",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_evidence"))?,
                and_then_value(&verdict, || get(&verdict, "reason_code"))?,
            ]))
        },
    )?;

    // ── J4c invalid explicit value: exit 2 ───────────────────────────────────────────────
    let out_4c = sandbox.root().join("rejudge/j4c");
    let out_4c_text = out_4c.to_string_lossy().into_owned();
    let run = sandbox.judge(
        "j4",
        &borrowed(&with(
            &EXPLORE,
            &["--accept-len", "0.5", "--out-dir", &out_4c_text],
        )),
    )?;
    let cond = status_is(&run, 2) && !exists(&out_4c.join("verdict.json"))?;
    checks.check(
        "J4c★ 무효 명시 --accept-len 0.5 → exit 2 · 판정 산출 없음",
        cond,
        || tuple_text(vec![int(run.status), st(&tail(&run.stderr, 300))]),
    )?;

    // ── J4d the same, in place: the canonical pair is not torn ───────────────────────────
    let directory = sandbox.sweep_dir("j4");
    let pair = ["roofline.json", "verdict.json"];
    let before: Vec<Vec<u8>> = pair
        .iter()
        .map(|name| read_bytes(&directory.join(name)))
        .collect::<Result<_, _>>()?;
    let run = sandbox.judge("j4", &borrowed(&with(&EXPLORE, &["--accept-len", "0.5"])))?;
    let after = files_equal_or_absent(&directory, &pair)?;
    let mut leftovers: Vec<String> = Vec::new();
    let listing = std::fs::read_dir(&directory)
        .map_err(|error| Raised::os_error(&error, &crate::verify_fixture::path_text(&directory)))?;
    for entry in listing {
        let entry = entry.map_err(|error| {
            Raised::os_error(&error, &crate::verify_fixture::path_text(&directory))
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.contains(".partial.") {
            leftovers.push(name);
        }
    }
    leftovers.sort();
    let unchanged = after
        .iter()
        .zip(&before)
        .all(|(now, was)| now.as_ref() == Some(was));
    let cond = status_is(&run, 2)
        && before.iter().all(|bytes| !bytes.is_empty())
        && unchanged
        && leftovers.is_empty();
    checks.check(
        "J4d★ 제자리 무효 명시 → exit 2 · 기존 roofline/verdict 바이트 불변 · 임시 파일 잔재 없음",
        cond,
        || {
            let sizes = PyValue::Dict({
                let mut map = crate::pyvalue::PyDict::new();
                for (index, name) in pair.iter().enumerate() {
                    let after_size = after[index].as_ref().map_or(0, Vec::len);
                    map.set(
                        name,
                        tuple(vec![
                            int(before[index].len() as i64),
                            int(after_size as i64),
                        ]),
                    );
                }
                map
            });
            format!(
                "({}, {}, {})",
                run.status,
                sizes.repr(),
                PyValue::List(leftovers.iter().map(|name| st(name)).collect()).repr()
            )
        },
    )?;

    // ── J5 no declaration fingerprint ────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j5", SpecDeclaration::Missing, Some(2.0), 105.0)?;
    let run = sandbox.judge("j5", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "measured")?
        && verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "PASS")?
        && eq(
            &chain(&verdict, &["rubric", "spec_axis", "declared"])?,
            &st("unknown"),
        );
    checks.check(
        "J5 unknown — 실측은 여전히 승계 · 종전 규율(measured.spec_on) PASS",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_source"))?,
                and_then_value(&verdict, || get(&verdict, "verdict"))?,
            ]))
        },
    )?;

    // ── J5b no fingerprint and no measured value ─────────────────────────────────────────
    let directory = sandbox.make_sweep("j5b", SpecDeclaration::Missing, None, 105.0)?;
    let run = sandbox.judge("j5b", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && verdict.is_truthy()
        && get_eq_text(&verdict, "verdict", "NEEDS_RUBRIC")?
        && is_none(&get(&verdict, "reason_code")?)
        && eq(
            &chain(&verdict, &["rubric", "physical", "exceeds"])?,
            &PyValue::Bool(true),
        );
    checks.check(
        "J5b★ unknown ∧ 실측 없음 → 사유코드 없이 물리 초과 NEEDS_RUBRIC(지문 없으면 결손 식별 불가)",
        cond,
        || -> Result<String, Raised> { Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&verdict, || get(&verdict, "verdict"))?,
                and_then_value(&verdict, || get(&verdict, "reason_code"))?,
            ])) },
    )?;
    // An unknown declaration's 1.0 is a placeholder: written as declared-absent it would let a missing value pass.
    let cond = roofline.is_truthy() && get_eq_text(&roofline, "accept_len_source", "absent")?;
    checks.check(
        "J5b★ unknown ∧ 실측 없음의 roofline 출처는 absent 다(declared-absent ✗)",
        cond,
        || -> Result<String, Raised> {
            Ok(and_then_value(&roofline, || get(&roofline, "accept_len_source"))?.repr())
        },
    )?;

    // ── J6 --out-dir: the original place untouched, publication suppressed ───────────────
    let directory = sandbox.make_sweep("j6", on_k3, Some(2.0), 105.0)?;
    let out_6 = sandbox.root().join("rejudge/j6");
    let out_6_text = out_6.to_string_lossy().into_owned();
    let run = sandbox.judge(
        "j6",
        &borrowed(&with(&EXPLICIT_PASS, &["--out-dir", &out_6_text])),
    )?;
    let verdict_6 = Sandbox::load(&out_6.join("verdict.json"));
    let cond = status_is(&run, 0)
        && verdict_6.is_truthy()
        && get_eq_text(&verdict_6, "verdict", "PASS")?
        && is_file(&out_6.join("roofline.json"))?
        && !exists(&directory.join("roofline.json"))?
        && !exists(&directory.join("verdict.json"))?
        && !exists(&sandbox.marker())?
        && run.stdout.contains("인증서 발행 억제");
    checks.check(
        "J6 --out-dir — 판정은 out-dir 에만 · sweep 디렉터리에 roofline/verdict 없음 · 발행 억제",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&verdict_6, || get(&verdict_6, "verdict"))?,
                PyValue::Bool(sandbox.marker().exists()),
                st(&tail(&run.stdout, 300)),
            ]))
        },
    )?;

    // ── J6b --no-publish (in place) ──────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j6b", on_k3, Some(2.0), 105.0)?;
    let run = sandbox.judge("j6b", &borrowed(&with(&EXPLICIT_PASS, &["--no-publish"])))?;
    let in_place = Sandbox::load(&directory.join("verdict.json"));
    let cond = status_is(&run, 0)
        && get_eq_text(
            &or_default(in_place.clone(), dict_of(vec![])),
            "verdict",
            "PASS",
        )?
        && !exists(&sandbox.marker())?
        && run.stdout.contains("인증서 발행 억제");
    checks.check(
        "J6b --no-publish — 제자리 판정 · 발행 억제",
        cond,
        || {
            tuple_text(vec![
                int(run.status),
                PyValue::Bool(sandbox.marker().exists()),
            ])
        },
    )?;

    // ── J7 no suppression: the publication branch is really entered (stub) ───────────────
    let directory = sandbox.make_sweep("j7", on_k3, Some(2.0), 105.0)?;
    let run = sandbox.judge("j7", &EXPLICIT_PASS)?;
    let called = marker_text(sandbox)?;
    let stub_in_place = read_text(&sandbox.scripts_dir().join(PUBLISHER))?;
    let cond = status_is(&run, 0)
        && called.contains(
            &directory
                .join("verdict.json")
                .to_string_lossy()
                .into_owned(),
        )
        && stub_in_place.contains(STUB_SENTINEL);
    checks.check(
        "J7★ 억제 없는 explicit PASS 는 발행기를 부른다(J6·J6b 억제가 공허하지 않다)",
        cond,
        || tuple_text(vec![int(run.status), st(&head(&called, 200))]),
    )?;

    // ── J12 an unfinished last sweep is not judged ───────────────────────────────────────
    let directory = sandbox.make_sweep("j12", on_k3, Some(2.0), 105.0)?;
    write_text(
        &directory.join("sweep_incomplete.json"),
        &crate::verify_fixture::dumps(&dict_of(vec![
            ("started_utc", st("2026-01-02T00:00:00Z")),
            ("pid", int(1)),
            ("config", st("j12")),
        ]))?,
    )?;
    let run = sandbox.judge("j12", &EXPLICIT_PASS)?;
    let called = marker_text(sandbox)?;
    let cond = status_is(&run, 3)
        && run.stderr.contains("정상 종료하지 않았다")
        && !exists(&directory.join("roofline.json"))?
        && !exists(&directory.join("verdict.json"))?
        && !called.contains(
            &directory
                .join("verdict.json")
                .to_string_lossy()
                .into_owned(),
        );
    checks.check(
        "J12★ sweep_incomplete.json 이 있으면 exit 3 · roofline/verdict 미산출 · 발행기 미호출",
        cond,
        || tuple_text(vec![int(run.status), st(&tail(&run.stderr, 300))]),
    )?;

    // ── J8 --dry-run ─────────────────────────────────────────────────────────────────────
    let directory = sandbox.make_sweep("j8", on_k3, Some(2.0), 105.0)?;
    let out_8 = sandbox.root().join("rejudge/j8");
    let out_8_text = out_8.to_string_lossy().into_owned();
    let run = sandbox.judge(
        "j8",
        &borrowed(&with(&EXPLORE, &["--dry-run", "--out-dir", &out_8_text])),
    )?;
    let cond = status_is(&run, 0)
        && run.stdout.contains("--accept-len-source measured")
        && run.stdout.contains("--spec-declared on")
        && !exists(&directory.join("roofline.json"))?
        && !exists(&out_8)?;
    checks.check(
        "J8 --dry-run — 승계 인자가 argv 에 드러나고 판정 산출물도 out-dir 자리도 만들지 않는다",
        cond,
        || {
            tuple_text(vec![
                int(run.status),
                PyValue::Bool(out_8.exists()),
                st(&tail(&run.stdout, 300)),
            ])
        },
    )?;

    // ── J10 the roofline combination contract: the copied launcher is called directly ────
    let roofline_script = sandbox.scripts_dir().join("roofline.sh");
    let model = sandbox.model().to_string_lossy().into_owned();
    let manifest = sandbox.manifest().to_string_lossy().into_owned();
    let roofline_base = [
        "bash",
        &roofline_script.to_string_lossy(),
        "--model-path",
        &model,
        "--manifest",
        &manifest,
        "--tp",
        "1",
        "--json",
    ]
    .map(str::to_string);
    let roof = |extra: &[&str]| -> Result<TextRun, Raised> {
        let mut command: Vec<OsString> = roofline_base.iter().map(OsString::from).collect();
        command.extend(words(extra));
        run_text_within(&command, &sandbox.child_settings(), ROOFLINE_SECONDS)
    };
    let contradictions: [(&str, [&str; 4]); 5] = [
        (
            "값 2 ∧ 출처 absent",
            ["--accept-len", "2", "--accept-len-source", "absent"],
        ),
        (
            "값 2 ∧ 출처 declared-absent",
            [
                "--accept-len",
                "2",
                "--accept-len-source",
                "declared-absent",
            ],
        ),
        (
            "출처 measured ∧ 값 없음",
            ["--accept-len-source", "measured", "", ""],
        ),
        (
            "출처 declared ∧ 값 없음",
            ["--accept-len-source", "declared", "", ""],
        ),
        (
            "값 nan ∧ 출처 measured",
            ["--accept-len", "nan", "--accept-len-source", "measured"],
        ),
    ];
    for (label, extra) in contradictions {
        let arguments: Vec<&str> = extra
            .iter()
            .copied()
            .filter(|word| !word.is_empty())
            .collect();
        let run = roof(&arguments)?;
        let cond = status_is(&run, 2) && run.stderr.contains("[roofline] ERROR");
        checks.check(&format!("J10★ roofline {label} → exit 2"), cond, || {
            tuple_text(vec![
                int(run.status),
                st(&head(&run.stdout, 120)),
                st(&tail(&run.stderr, 200)),
            ])
        })?;
    }
    let document = |run: &TextRun| -> Result<PyValue, Raised> {
        if run.status == 0 {
            let loaded = crate::pyjson::loads_text(&crate::pystr::PyStr::new(&run.stdout))
                .map_err(|error| {
                    Raised::from_failure(crate::repeat_api_cli::load_failure(&error))
                })?;
            Ok(loaded)
        } else {
            Ok(dict_of(vec![]))
        }
    };
    let run = roof(&["--accept-len", "2"])?;
    let parsed = document(&run)?;
    let cond = status_is(&run, 0)
        && get_eq_text(&parsed, "accept_len_source", "declared")?
        && get_eq(&parsed, "R_token", &float(200.0))?;
    checks.check(
        "J10 roofline 값만(--accept-len 2) → declared · R_token 200 (종전 호출 호환)",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                get(&parsed, "accept_len_source")?,
                st(&tail(&run.stderr, 200)),
            ]))
        },
    )?;
    let run = roof(&[])?;
    let parsed = document(&run)?;
    let cond = status_is(&run, 0)
        && get_eq_text(&parsed, "accept_len_source", "absent")?
        && get_eq(&parsed, "accept_len", &float(1.0))?;
    checks.check(
        "J10 roofline 값·출처 모두 없음 → absent · accept_len 1.0(자리표시자)",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                get(&parsed, "accept_len_source")?,
                st(&tail(&run.stderr, 200)),
            ]))
        },
    )?;

    // ── J11 declaration off against a measured speculative run: measurement first ────────
    let directory = sandbox.make_sweep("j11", off, Some(2.0), 105.0)?;
    let run = sandbox.judge("j11", &EXPLORE)?;
    let roofline = Sandbox::load(&directory.join("roofline.json"));
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let axis = get_or_empty(&get_or_empty(&verdict, "rubric")?, "spec_axis")?;
    let mismatch = or_default(get(&axis, "mismatch")?, st(""));
    let cond = status_is(&run, 0)
        && roofline.is_truthy()
        && get_eq_text(&roofline, "accept_len_source", "measured")?
        && get_eq(&roofline, "R_token", &float(200.0))?
        && get_eq_text(&axis, "limit", "R_token")?
        && starts_with(&mismatch, "YES")?
        && eq(
            &chain(&verdict, &["rubric", "physical", "limit_tps"])?,
            &float(200.0),
        );
    checks.check(
        "J11 선언 off ∧ 실측 2.0 → source=measured · R_token 200 · 상한 R_token · mismatch=YES",
        cond,
        || -> Result<String, Raised> {
            Ok(tuple_text(vec![
                int(run.status),
                and_then_value(&roofline, || get(&roofline, "accept_len_source"))?,
                axis.clone(),
                st(&tail(&run.stderr, 300)),
            ]))
        },
    )?;

    // ── J11b declaration off with an explicit 1.5: bound R_fp, the explicit value is named
    let directory = sandbox.make_sweep("j11b", off, None, 60.0)?;
    let run = sandbox.judge(
        "j11b",
        &[
            "--authority",
            "explore",
            "--e-search",
            "empty",
            "--accept-len",
            "1.5",
        ],
    )?;
    let verdict = Sandbox::load(&directory.join("verdict.json"));
    let axis = get_or_empty(&get_or_empty(&verdict, "rubric")?, "spec_axis")?;
    let basis = or_default(get(&axis, "basis")?, st(""));
    let mismatch = or_default(get(&axis, "mismatch")?, st(""));
    let cond = status_is(&run, 0)
        && get_eq_text(&axis, "limit", "R_fp")?
        && !contains(&basis, "정상")?
        && contains(&mismatch, "accept_len_source=declared")?;
    checks.check(
        "J11b 선언 off ∧ 명시 1.5 → 상한 R_fp · mismatch 에 accept_len_source=declared · '정상' 서술 ✗",
        cond,
        || {
            tuple_text(vec![
                int(run.status),
                axis.clone(),
                st(&tail(&run.stderr, 300)),
            ])
        },
    )?;

    Ok(())
}

fn tuple(items: Vec<PyValue>) -> PyValue {
    PyValue::Tuple(items)
}
