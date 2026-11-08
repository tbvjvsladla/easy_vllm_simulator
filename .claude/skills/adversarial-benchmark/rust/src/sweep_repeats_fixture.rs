//! The isolated sandbox of `selftest_sweep_repeats`: a private git checkout that holds byte copies of the deployed
//! scripts and programs the repeat-axis chain runs through, the two load-tool shims and the `curl` shim, and the
//! campaign, manifest and model files the chain reads. A Rust port of `Sandbox` and its module constants in
//! `selftest_sweep_repeats.py`.
//!
//! **Remaining Python in this fixture (test data, not production logic):** the two shims are shell scripts that embed
//! small Python programs (`python3 - <<'PY'`, and two `python3 -c` one-liners in the lite shim). They are the
//! original's shim text, kept byte for byte and written into the sandbox; they replace `run_bench.sh` and
//! `lite_bench.sh` (no GPU, docker or serving is touched) and still need a `python3` on `PATH` when the chain calls them.
//!
//! Copies are made with [`crate::verify_fixture::IsolatedCheckout::copy_from`], which writes each file as a new inode
//! (`copy2` refuses a destination that is the source file); nothing is hard-linked or written through an alias.

use crate::conformance::{splitlines, Raised};
use crate::pystr::PyStr;
use crate::pyvalue::{PyDict, PyValue};
use crate::verify_fixture::{
    chmod, dumps, is_file, list, load_json, object, read_text, run_text_within, text,
    unlink_if_present, write_bytes, write_str, IsolatedCheckout, TextRun,
};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const SKILL: &str = ".claude/skills/adversarial-benchmark";
/// The campaign the fixture declares.
pub const CAMPAIGN: &str = "camp-fx";
/// The shim's own sentinel: the check that the load tools are shims looks for it.
pub const SHIM_SENTINEL: &str = "SELFTEST_SWEEP_REPEATS_SHIM";
/// The wall-clock limit of every sandbox call (`Sandbox.run(timeout=170)`).
const RUN_SECONDS: u64 = 170;

/// Measurement shape knob contract: the sweep flags are the agent's choice, so each carries a reason code.
pub const KNOB_REASONS: [&str; 4] = [
    "--knob-reason",
    "backend=harmony_completion",
    "--knob-reason",
    "bench_budget_mib=host_budget",
];
pub const LEVEL_REASON: [&str; 2] = ["--knob-reason", "levels=serving_concurrency"];

/// The deployed files the sandbox copies, in the original's order (`COPIES`). `classify_cell` / `classify_cell_api` ask the
/// private native helper `node_identity_resolve` for the node identity (one normal sibling in the same release directory), so
/// the helper is copied with them: a mirrored caller without its helper fails loud, by design.
const COPIES: [&str; 75] = [
    ".claude/skills/adversarial-benchmark/scripts/render_report.sh",
    ".claude/skills/adversarial-benchmark/scripts/render_sweep_map.sh",
    ".claude/skills/adversarial-benchmark/scripts/render_max_report.sh",
    ".claude/skills/adversarial-benchmark/scripts/publish_benchmark_record.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_doc_naming.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_reporting_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/reporting_native.py",
    ".claude/skills/adversarial-benchmark/scripts/doc_naming_native.py",
    ".claude/skills/adversarial-benchmark/scripts/render_report_native.py",
    ".claude/skills/adversarial-benchmark/scripts/render_sweep_map_native.py",
    ".claude/skills/adversarial-benchmark/scripts/render_max_report_native.py",
    ".claude/skills/adversarial-benchmark/scripts/publish_benchmark_record_native.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/render_report",
    ".claude/skills/adversarial-benchmark/rust/target/release/render_sweep_map",
    ".claude/skills/adversarial-benchmark/rust/target/release/publish_benchmark_record",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_reporting_api",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell.sh",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_native.py",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell_role_query.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/classify_cell",
    ".claude/skills/adversarial-benchmark/rust/target/release/classify_cell_api",
    ".claude/skills/adversarial-benchmark/rust/target/release/node_identity_resolve",
    ".claude/skills/adversarial-benchmark/rust/target/release/roofline",
    ".claude/skills/adversarial-benchmark/rust/target/release/verdict_rule",
    ".claude/skills/adversarial-benchmark/scripts/sweep_bench.sh",
    ".claude/skills/adversarial-benchmark/scripts/assemble_sweep_meta.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_shell.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/assemble_sweep_meta",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_shell",
    ".claude/skills/adversarial-benchmark/rust/src/bench_shell/broad.rs",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.py",
    ".claude/skills/adversarial-benchmark/scripts/parse_guidellm.sh",
    ".claude/skills/adversarial-benchmark/scripts/parse_guidellm_native.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/parse_guidellm",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.sh",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset_api.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset",
    ".claude/skills/adversarial-benchmark/rust/target/release/bench_preset_api",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_native.py",
    ".claude/skills/adversarial-benchmark/scripts/repeat_axis_owner_query.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis",
    ".claude/skills/adversarial-benchmark/rust/target/release/repeat_axis_api",
    ".claude/skills/adversarial-benchmark/scripts/parse_bench.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/parse_bench",
    ".claude/skills/adversarial-benchmark/scripts/doc_naming.py",
    ".claude/skills/adversarial-benchmark/scripts/lite_metrics.py",
    ".claude/skills/adversarial-benchmark/scripts/lite_metrics.sh",
    ".claude/skills/adversarial-benchmark/scripts/lite_metrics_api.sh",
    ".claude/skills/adversarial-benchmark/scripts/lite_metrics_native.py",
    ".claude/skills/adversarial-benchmark/rust/target/release/lite_metrics",
    ".claude/skills/adversarial-benchmark/rust/target/release/lite_metrics_api",
    ".claude/skills/adversarial-benchmark/scripts/classify_cell.py",
    ".claude/skills/adversarial-benchmark/scripts/render_report.py",
    ".claude/skills/adversarial-benchmark/scripts/render_sweep_map.py",
    ".claude/skills/adversarial-benchmark/scripts/sweep_stop.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/sweep_stop",
    ".claude/skills/adversarial-benchmark/scripts/broad_search.sh",
    ".claude/skills/adversarial-benchmark/scripts/judge_bench.sh",
    ".claude/skills/adversarial-benchmark/scripts/roofline.sh",
    ".claude/skills/adversarial-benchmark/scripts/verdict_rule.sh",
    ".claude/skills/adversarial-benchmark/scripts/publish_benchmark_record.py",
    ".claude/skills/adversarial-benchmark/fixtures/guidellm_benchmarks_sample.json",
    ".claude/skills/adversarial-benchmark/scripts/bench_preset.py",
    ".claude/skills/adversarial-benchmark/config/bench_bindings.yaml",
    ".claude/skills/adversarial-benchmark/config/guide-llm/default_preset.yaml",
    ".claude/skills/terraforming_node/scripts/campaign_init.py",
    ".claude/skills/terraforming_node/scripts/campaign_init.sh",
    ".claude/skills/adversarial-benchmark/rust/target/release/campaign_read",
    ".claude/skills/terraforming_node/scripts/campaign_template_validator.py",
    ".claude/skills/terraforming_node/scripts/node_role_contract.py",
    ".claude/skills/terraforming_node/scripts/node_blackbox/node_identity.sh",
    "campaigns/_template/campaign.schema.json",
];

const RUN_BENCH_SHIM: &str = r##"#!/usr/bin/env bash
# @SENTINEL@ — run_bench.sh 가 아니다(테스트 평면 격리). GPU·docker·서빙에 닿지 않는다.
set -euo pipefail
CONFIG="$1"; shift
OUT=""; CONC=1
while [ $# -gt 0 ]; do case "$1" in
  --out-dir) OUT="$2"; shift 2;; --concurrency) CONC="$2"; shift 2;;
  *) shift 2 2>/dev/null || shift;;
esac; done
ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
echo "$CONFIG level=$CONC out=$(basename "$OUT")" >> "$ROOT/SHIM_CALLS"
SHIM_ROOT="$ROOT" SHIM_CONFIG="$CONFIG" SHIM_CONC="$CONC" SHIM_OUT="$OUT" python3 - <<'PY'
import copy, datetime, json, os, re, sys
root, cfg, conc, out = os.environ["SHIM_ROOT"], os.environ["SHIM_CONFIG"], int(os.environ["SHIM_CONC"]), os.environ["SHIM_OUT"]
m = re.match(r"run_(\d+)$", os.path.basename(out))
run = int(m.group(1)) if m else 1
plan = json.loads(os.environ.get("SHIM_PLAN") or "{}")
key = "%d:%d" % (conc, run)
act = (plan.get("fail") or {}).get(key)
now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
if act in ("kill", "trip"):
    ev = os.path.join(root, "docs", "logs", "main", "events", "2026-09.jsonl")
    os.makedirs(os.path.dirname(ev), exist_ok=True)
    kind = "watchdog_kill_ack" if act == "kill" else "thermal_trip"
    with open(ev, "a", encoding="utf-8") as f:
        f.write(json.dumps({"ts": now, "kind": kind, "source": "selftest-shim", "mode": "armed"}) + "\n")
    sys.exit(3)
if act == "rc":
    sys.exit(3)
if act == "norecord":
    # run 경계 사실 기록 불가 주입 — record-run 이 쓸 자리를 디렉터리로 막는다(측정 자체는 정상).
    os.makedirs(os.path.join(out, "repeat_run.json"), exist_ok=True)
doc = json.load(open(os.path.join(root, ".claude/skills/adversarial-benchmark/fixtures/guidellm_benchmarks_sample.json")))
doc["metadata"]["guidellm_version"] = "0.0.0+selftest-shim"
b = doc["benchmarks"][0]
tpots = (plan.get("tpot") or {}).get(str(conc)) or [25.0, 24.0, 26.0]
b["metrics"]["time_per_output_token_ms"]["successful"]["median"] = float(tpots[(run - 1) % len(tpots)])
b["config"]["profile"]["streams"] = [conc]
if act == "measurement":
    b["metrics"]["request_totals"]["incomplete"] = 2
with open(os.path.join(out, "guidellm_%s.json" % cfg), "w") as f:
    json.dump(doc, f)
with open(os.path.join(out, "engine_%s.log" % cfg), "w") as f:
    f.write("INFO generation throughput: 40.0 tokens/s\n")
with open(os.path.join(out, "post_health_%s.json" % cfg), "w") as f:
    json.dump({"server_alive_at_bench_end": True, "provenance": "selftest-shim"}, f)
PY
"##;

const LITE_BENCH_SHIM: &str = r##"#!/usr/bin/env bash
# @SENTINEL@ — lite_bench.sh 가 아니다(테스트 평면 격리).
set -euo pipefail
CONFIG="$1"; shift
OUT=""
while [ $# -gt 0 ]; do case "$1" in --out-dir) OUT="$2"; shift 2;; *) shift 2 2>/dev/null || shift;; esac; done
ROOT="$(cd "$(dirname "$0")/../../../.." && pwd)"
echo "$CONFIG lite" >> "$ROOT/SHIM_CALLS"
cat > "$OUT/lite_warm_$CONFIG.json" <<JSON
{"completed": 3, "failed": 0, "median_tpot_ms": 25.0, "spec_decode_acceptance_length": 2.0, "_shim": "selftest"}
JSON
# SHIM_PLAN.lite = {"verdict": <lite_verdict 또는 "absent">, "rc": <종료코드>} — lite 게이트 시험용(기본 pass · rc 0).
LV="$(python3 -c 'import json,os; print((json.loads(os.environ.get("SHIM_PLAN") or "{}").get("lite") or {}).get("verdict","pass"))')"
LRC="$(python3 -c 'import json,os; print((json.loads(os.environ.get("SHIM_PLAN") or "{}").get("lite") or {}).get("rc",0))')"
if [ "$LV" = "absent" ]; then
  rm -f "$OUT/lite_raw_$CONFIG.json"
else
cat > "$OUT/lite_raw_$CONFIG.json" <<JSON
{"topology": "single", "burst_n": 3, "lite_verdict": "$LV", "measured_utc": "$(date -u +%FT%TZ)", "bench_warm_json": "$OUT/lite_warm_$CONFIG.json", "nodes": [{"role": "main"}]}
JSON
fi
exit "$LRC"
"##;

/// What `json.dumps(value)` writes (ASCII-escaped, default separators), as text.
fn json_text(value: &PyValue) -> Result<String, Raised> {
    Ok(dumps(value)?.to_utf8().unwrap_or_default())
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// The sandbox: `<temporary>/repo` (a git checkout) with the shim directory `<temporary>/bin` beside it.
pub struct Sandbox {
    checkout: IsolatedCheckout,
    root: PathBuf,
    scripts: PathBuf,
    model: PathBuf,
    campaign_dir: PathBuf,
    /// `PATH` with the shim directory in front, fixed when the sandbox was built (`self.env`).
    search_path: OsString,
}

impl Sandbox {
    /// Copies the deployed files, writes the shims, initialises the checkout and lays out the manifest, the model
    /// and the campaign declaration (`repeats = 3`). A deployed file that is missing is the copy's own
    /// `FileNotFoundError`, before any check.
    pub fn build(repo: &Path) -> Result<Sandbox, Raised> {
        let checkout = IsolatedCheckout::create("sweep-repeats.")?;
        let root = checkout.root().to_path_buf();
        checkout.copy_from(repo, &COPIES)?;
        let scripts = root.join(SKILL).join("scripts");
        write_str(
            &scripts.join("run_bench.sh"),
            &RUN_BENCH_SHIM.replace("@SENTINEL@", SHIM_SENTINEL),
        )?;
        chmod(&scripts.join("run_bench.sh"), 0o755)?;
        write_str(
            &scripts.join("lite_bench.sh"),
            &LITE_BENCH_SHIM.replace("@SENTINEL@", SHIM_SENTINEL),
        )?;
        chmod(&scripts.join("lite_bench.sh"), 0o755)?;
        let curl = checkout.bin_dir().join("curl");
        write_str(&curl, "#!/bin/sh\nprintf 200\n")?;
        chmod(&curl, 0o755)?;
        checkout.git_init()?;
        // `self_role` is left out on purpose (the live shape `sweep_bench` defaults for): this node stands by the
        // owner rule (`node_identity.sh`: the only `role: main`). The roster is the real manifest shape.
        write_str(
            &root.join("output/single/manifest.yaml"),
            "topology: single\ngpus_per_node: 1\ngpu_model: \"NVIDIA GB10\"\nnodes:\n  - role: main\n",
        )?;
        let model = root.join("models/fx-dense");
        write_str(
            &model.join("config.json"),
            &json_text(&object(vec![
                ("hidden_size", PyValue::int(64)),
                ("num_hidden_layers", PyValue::int(2)),
            ]))?,
        )?;
        let header = json_text(&object(vec![(
            "w",
            object(vec![
                ("dtype", text("F16")),
                ("shape", list(vec![PyValue::int(1_365_000_000_i64)])),
                (
                    "data_offsets",
                    list(vec![PyValue::int(0), PyValue::int(2_730_000_000_i64)]),
                ),
            ]),
        )]))?;
        let mut safetensors = u64::try_from(header.len())
            .expect("a header length fits in 64 bits")
            .to_le_bytes()
            .to_vec();
        safetensors.extend_from_slice(header.as_bytes());
        write_bytes(&model.join("model.safetensors"), &safetensors)?;
        let campaign_dir = root.join("campaigns").join(CAMPAIGN);
        write_str(&root.join("campaigns/ACTIVE"), &format!("{CAMPAIGN}\n"))?;
        let mut search_path = checkout.bin_dir().as_os_str().to_os_string();
        search_path.push(":");
        search_path.push(std::env::var_os("PATH").unwrap_or_default());
        let sandbox = Sandbox {
            checkout,
            root,
            scripts,
            model,
            campaign_dir,
            search_path,
        };
        sandbox.declare(Some(3))?;
        Ok(sandbox)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The temporary directory that holds the checkout and the shim directory (`Path(td)`).
    pub fn temporary(&self) -> &Path {
        self.root
            .parent()
            .expect("the checkout sits inside the temporary directory")
    }

    pub fn scripts(&self) -> &Path {
        &self.scripts
    }

    /// `sb.sdir / name`.
    pub fn script(&self, name: &str) -> OsString {
        self.scripts.join(name).into_os_string()
    }

    pub fn campaign_dir(&self) -> &Path {
        &self.campaign_dir
    }

    /// Removes the temporary directory; a failure is the `OSError` that leaving the `with` block raises.
    pub fn close(self) -> Result<(), Raised> {
        self.checkout.close()
    }

    /// `declare(repeats=..)`: the campaign declaration (a JSON document) and one lockset per cell.
    /// `None` leaves `budgets.repeats` out.
    pub fn declare(&self, repeats: Option<i64>) -> Result<(), Raised> {
        self.declare_cells(repeats, &["cell-a", "cell-b"])
    }

    fn declare_cells(&self, repeats: Option<i64>, cells: &[&str]) -> Result<(), Raised> {
        let mut budgets = PyDict::new();
        budgets.set("smoke_budget_overhead_mib", PyValue::int(1));
        budgets.set("ready_max_seconds", PyValue::int(1));
        if let Some(repeats) = repeats {
            budgets.set("repeats", PyValue::int(repeats));
        }
        let assignments = object(vec![(
            "main",
            list(
                cells
                    .iter()
                    .map(|cell| object(vec![("cell", text(cell))]))
                    .collect(),
            ),
        )]);
        let declaration = object(vec![
            ("schema_version", PyValue::int(1)),
            ("id", text(CAMPAIGN)),
            ("plan_ref", text("docs/plan/fixture.md")),
            (
                "nodes",
                list(vec![object(vec![
                    ("node_id", text("main")),
                    ("role", text("main")),
                    ("topology", text("single")),
                ])]),
            ),
            ("assignments", assignments),
            ("budgets", PyValue::Dict(budgets)),
            (
                "control_variables",
                object(vec![
                    ("model", text("m")),
                    ("vllm_version", text("0.0.0")),
                    ("topology", text("single")),
                    ("target_gpu", text("fixture")),
                ]),
            ),
        ]);
        write_str(
            &self.campaign_dir.join("campaign.yaml"),
            &json_text(&declaration)?,
        )?;
        for cell in cells {
            write_str(
                &self
                    .campaign_dir
                    .join("cells")
                    .join(cell)
                    .join("lockset.json"),
                &json_text(&object(vec![
                    ("id", text(cell)),
                    ("provenance", text("hand-authored")),
                ]))?,
            )?;
        }
        Ok(())
    }

    /// `config(cfg, topo)`: the serving configuration and its environment file.
    pub fn config(&self, config: &str, topology: &str) -> Result<(), Raised> {
        write_str(
            &self
                .root
                .join(format!("output/{topology}/configs/{config}.yaml")),
            &format!("model: {}\nmax-model-len: 4096\n", path_string(&self.model)),
        )?;
        write_str(
            &self
                .root
                .join(format!("output/{topology}/envs/.env.{config}")),
            &format!("SERVING_PORT=18080\nSERVING_MODEL_NAME={config}\n"),
        )
    }

    /// `sweep_dir(cfg, topo)`.
    pub fn sweep_dir(&self, config: &str, topology: &str) -> PathBuf {
        self.root
            .join(format!("output/{topology}/benchlog"))
            .join(format!("sweep_{config}"))
    }

    /// The lines of `SHIM_CALLS` (empty when no shim was called).
    pub fn calls(&self) -> Result<Vec<String>, Raised> {
        let path = self.root.join("SHIM_CALLS");
        if !is_file(&path)? {
            return Ok(Vec::new());
        }
        Ok(splitlines(&PyStr::new(&read_text(&path)?))
            .iter()
            .map(|line| line.to_utf8().unwrap_or_default())
            .collect())
    }

    pub fn reset_calls(&self) -> Result<(), Raised> {
        unlink_if_present(&self.root.join("SHIM_CALLS"))
    }

    /// Empties the blackbox events, so one case's kill does not fall into the next case's window (times are seconds).
    pub fn clear_events(&self) {
        let _ = std::fs::remove_dir_all(self.root.join("docs").join("logs"));
    }

    /// `sb.run(argv, plan=..)`: the command in the checkout with the shimmed `PATH`, the shim plan as `SHIM_PLAN`
    /// (a plan that is an empty object is still set), under the 170 second limit.
    pub fn run(&self, words: Vec<OsString>, plan: Option<&PyValue>) -> Result<TextRun, Raised> {
        let mut environment = vec![
            (OsString::from("PATH"), self.search_path.clone()),
            (
                OsString::from("PYTHONDONTWRITEBYTECODE"),
                OsString::from("1"),
            ),
        ];
        if let Some(plan) = plan {
            environment.push((
                OsString::from("SHIM_PLAN"),
                OsString::from(json_text(plan)?),
            ));
        }
        let settings = crate::captured_process::ChildSettings {
            current_dir: Some(self.root.clone()),
            env_set: environment,
            ..Default::default()
        };
        run_text_within(&words, &settings, RUN_SECONDS)
    }

    /// `sb.sweep(cfg, *extra, plan=.., levels="1,2")`.
    pub fn sweep(
        &self,
        config: &str,
        extra: &[&str],
        plan: Option<&PyValue>,
        levels: &str,
    ) -> Result<TextRun, Raised> {
        self.config(config, "single")?;
        let mut words = vec![
            OsString::from("bash"),
            self.script("sweep_bench.sh"),
            OsString::from(config),
        ];
        words.extend(
            [
                "--topology",
                "single",
                "--tool",
                "guidellm",
                "--bench-budget-mib",
                "4096",
                "--backend",
                "openai",
            ]
            .map(OsString::from),
        );
        words.extend(KNOB_REASONS.map(OsString::from));
        words.push(OsString::from("--levels"));
        words.push(OsString::from(levels));
        words.extend(LEVEL_REASON.map(OsString::from));
        words.extend(extra.iter().map(OsString::from));
        self.run(words, plan)
    }

    /// `Sandbox.load(path)`: the parsed JSON file, or `None` when it cannot be read or parsed.
    pub fn load(path: &Path) -> Option<PyValue> {
        load_json(path).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::COPIES;
    use crate::classify_cell_owner::HELPER_NAME;
    use crate::verify_fixture::IsolatedCheckout;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn the_sandbox_copies_the_private_node_identity_helper_beside_the_classifier_binaries() {
        let release = |name: &str| {
            COPIES
                .iter()
                .find(|path| path.ends_with(&format!("/rust/target/release/{name}")))
                .copied()
        };
        let helper = release(HELPER_NAME).expect("the helper is in the copy list");
        let dir_of = |path: &str| path.rsplit_once('/').map(|(dir, _)| dir.to_string());
        for caller in ["classify_cell", "classify_cell_api"] {
            let caller = release(caller).expect("the classifier binary is in the copy list");
            assert_eq!(
                dir_of(caller),
                dir_of(helper),
                "same normal sibling placement"
            );
        }
        // presence and permission bits of the copy (a synthetic helper stands in for the real binary)
        let repo = std::env::temp_dir().join(format!("closure-source-{}", std::process::id()));
        let source = repo.join(helper);
        std::fs::create_dir_all(source.parent().expect("parent")).expect("source dir");
        std::fs::write(&source, b"\x7fELF synthetic helper").expect("write");
        std::fs::set_permissions(&source, std::fs::Permissions::from_mode(0o755)).expect("mode");
        let checkout = IsolatedCheckout::create("closure.").expect("checkout");
        checkout.copy_from(&repo, &[helper]).expect("copy");
        let copied = checkout.root().join(helper);
        assert_eq!(
            std::fs::read(&copied).expect("read"),
            b"\x7fELF synthetic helper"
        );
        let mode = std::fs::metadata(&copied)
            .expect("metadata")
            .permissions()
            .mode()
            & 0o7777;
        assert_eq!(mode, 0o755);
        checkout.close().expect("close");
        std::fs::remove_dir_all(&repo).expect("cleanup");
    }
}
