#!/usr/bin/env python3
"""selftest_sweep_repeats.py — full bench 반복 축(§4.4)의 격리 실행 자체검사 (plan_26091407 §7 O4).

왜 있는가: 반복 루프·강등 신호·bench_mode 확정은 셸 진입 경로(`sweep_bench.sh` 레벨 루프 → `repeat_axis.py`
집계 → `classify_cell.py` 확정 → `broad_search.sh` 셀 기록·정지 평가)에 걸쳐 있다. 순수 함수 자체검사
(`repeat_axis --self-test`·`classify_cell --self-test`)만으로는 **그 호출들이 실제 체인에 서 있는지**를 증명하지
못한다 — 호출자 없는 자체검사는 L1(산문)이다. 그래서 여기서는 배포되는 스크립트의 **바이트 사본**을 임시 git
저장소에서 돌린다(재구현 ✗ · `selftest_judge_bench.py`·`selftest_broad_search_precheck.py` 선례).

격리 방식 — 부하 도구는 **shim** 이다(테스트 평면의 외부 의존 차단 · 정당한 모킹):
  · `run_bench.sh`·`lite_bench.sh` 는 사본이 아니라 shim 이다. GPU·docker·서빙에 닿지 않고, 레벨 run 산출물로
    **실측 GuideLLM 산출물 픽스처**(`fixtures/guidellm_benchmarks_sample.json`)를 복제해 TPOT 중앙값·미완결 수만
    바꿔 쓴다 — 파서(`parse_guidellm.py`)는 실물 그대로 돈다(픽스처가 실물보다 좁지 않게).
    shim 산출물은 `guidellm_version=0.0.0+selftest-shim` 로 **스스로 가짜임을 밝힌다**(실측인 척 ✗).
  · `curl` 은 PATH 앞단 shim 이 200 을 낸다(broad_search 의 health 프로브). 임시 저장소 밖에는 아무것도 쓰지 않는다.
  · 실패주입은 shim 계획(`SHIM_PLAN`)이 정한다: run 의 measurement_ok=false(미완결 요청) · run_bench 비0 ·
    블랙박스 사살(shim 이 events JSONL 에 `watchdog_kill_ack`/`thermal_trip` 을 **그 순간 시각**으로 적고 비0 종료).

⚠ E2E(plan §5 ⑨) 정상 경로는 판정점 반복이 전부 완주하고 사살이 없으므로 **강등 경로를 밟지 않는다** — 포화 경계에서
  스윕이 멈추는 것(적응 상한 클램프 · 경계 레벨의 첫 run 이든 둘째 run 이든)도 정상 경로다(F4·F7). 강등(run_failed·
  blackbox_kill) 경로는 GPU 에서 일부러 재현할 수 없으므로 이 실패주입 픽스처가 그 경로의 유일한 상시 증거다.

사례(★ = 음성대조 · 막혀야/뒤집혀야 하는 것):
  N1  정상 · 캠페인 budgets.repeats=3 → 레벨×3 run · runs[]·repro_band_pct·measured(n=3) · 대표 run 필드 그대로
  N2  judge_bench 승계(③ 계약) — 대표 run 의 accept_len 이 roofline accept_len_source=measured 로 선다
  N3  정규 경로 — sweep_bench 종료부가 판정 소유자(classify_cell)를 불러 bench_mode.json(full)을 index 측정시각에 묶어 쓴다
  N4  render_report — 반복 절(완주/요청·밴드) · hint 파서(render_bench_section)가 스윕 표를 그대로 읽는다
  N5  인증서 — 대표 run 1회 값·측정시각 1개(반복이 인증서 키를 늘리지 않는다)
  N8★ 판정 기록이 낡음·반쯤 쓰임 → 리포트는 '미확정 — 사유' 로 **발행**(rc 0) · 명시 경로면 exit 2 · 인증서는 미발행
  N6  --reassemble-only — 측정 0회 · 요청 반복·출처는 **측정 시점** 것을 승계(지금 선언이 5 로 바뀌어도 3) · 판정 기록 재작성
  N7★ 같은 config 재스윕 — 이전 스윕의 run_04 가 이번 runs[] 에 섞이지 않는다(--repeats 4 → 3)
  R1★ 재조립은 기존 index levels[] 만 복원 — 판정점에서 끊긴 재스윕 뒤 디스크에 남은 옛 level_02 가 부활하지 않는다
  F1★ 판정점 run 2 measurement_ok=false → 즉시 신호 · 상위 레벨 미측정 · 판정 기록 lite · run_failed · 대조 not_scanned
  F2★ 같은 끊긴 창에 집행 사살 → blackbox_kill · 트립 단독 → run_failed · 창 밖 사살 → run_failed
  F3★ 강등 셀에는 full 인증서를 내지 않는다 · 리포트에 멈춘 자리·실패 run·강등 사유가 이름으로 남는다
  F4★ 비대칭 교정 — 경계 레벨 2 의 run 2 실패(판정점 3) → full · 인증서 발행(경계 첫 run 실패 F7 과 같은 판정)
  F5★ 클램프 레벨(2) 첫 run 에서 shim 이 사살 이벤트를 남기고 죽는다 → lite · blackbox_kill · 인증서 미발행
  F6★ 같은 자리 트립 단독 → full 유지(사인 불충분)
  F7  경계 레벨 2 첫 run 실패(rc) · 사살 없음 → full(적응 상한 클램프 · 대조 miss 기재)
  F8★ 집계 실패(run 2 경계 사실 기록 불가) → runs[] 없는 레벨 · 판정 기록 null(not_evaluated) · full 인증서 미발행
  V1★ 분산만 크다(TPOT 20/30/45) → full(분산은 강등 사유 ✗)
  A1★ --repeats 2 명시 → exit 2 · 부하 도구 미호출 / A2★ 캠페인 선언 repeats=2 → exit 2 · 미호출
  A3★ --reassemble-only 에 --repeats → exit 2(조용히 버리지 않는다)
  B1  broad_search init → declared_budget.repeats=3 + 출처(캠페인 선언) · cell 이 그 값을 sweep_bench 에 넘긴다
  B2  broad_search cell 정상 → 셀 기록 bench_mode=full(판정 기록 그대로) · repetition 요약 · 정지 평가 runs_attempted
  B3★ broad_search cell 에서 shim 이 run 2 에 사살 이벤트를 남기고 죽는다 → 셀 기록 lite · blackbox_kill
  B4★ declared_budget.repeats 없는 상태 파일 → 셀 진입 exit 2(새 run 소비 경로 · 기본값 발명 ✗)
  B5★ 같은 상태 파일의 status·map(읽기 경로)은 막히지 않는다 — 반복 수는 absent 로 기재

사용: python3 selftest_sweep_repeats.py [-v]   (exit 0 = 전부 통과)
"""
from __future__ import annotations

import json
import os
import shutil
import struct
import subprocess
import sys
import tempfile
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]
AB = ".claude/skills/adversarial-benchmark"
TN = ".claude/skills/terraforming_node/scripts"
COPIES = (
    f"{AB}/scripts/sweep_bench.sh", f"{AB}/scripts/repeat_axis.py", f"{AB}/scripts/parse_guidellm.py",
    f"{AB}/scripts/parse_bench.py", f"{AB}/scripts/doc_naming.py", f"{AB}/scripts/lite_metrics.py",
    f"{AB}/scripts/classify_cell.py", f"{AB}/scripts/render_report.py", f"{AB}/scripts/render_sweep_map.py",
    f"{AB}/scripts/sweep_stop.py", f"{AB}/scripts/broad_search.sh", f"{AB}/scripts/judge_bench.sh",
    f"{AB}/scripts/roofline.py", f"{AB}/scripts/verdict_rule.py", f"{AB}/scripts/publish_benchmark_record.py",
    f"{AB}/fixtures/guidellm_benchmarks_sample.json",
    f"{TN}/campaign_init.py", f"{TN}/campaign_template_validator.py",
    "campaigns/_template/campaign.schema.json",
)
SHIM_SENTINEL = "SELFTEST_SWEEP_REPEATS_SHIM"
CAMP = "camp-fx"

RUN_BENCH_SHIM = r'''#!/usr/bin/env bash
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
'''

LITE_BENCH_SHIM = r'''#!/usr/bin/env bash
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
cat > "$OUT/lite_raw_$CONFIG.json" <<JSON
{"topology": "single", "burst_n": 3, "bench_warm_json": "$OUT/lite_warm_$CONFIG.json", "nodes": [{"role": "main"}]}
JSON
'''


def _write(path: Path, text: str, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    if mode is not None:
        path.chmod(mode)


class Sandbox:
    def __init__(self, tmp: Path):
        self.root = tmp / "repo"
        self.bindir = tmp / "bin"
        for rel in COPIES:
            dst = self.root / rel
            dst.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(REPO / rel, dst)
        self.sdir = self.root / AB / "scripts"
        _write(self.sdir / "run_bench.sh", RUN_BENCH_SHIM.replace("@SENTINEL@", SHIM_SENTINEL), 0o755)
        _write(self.sdir / "lite_bench.sh", LITE_BENCH_SHIM.replace("@SENTINEL@", SHIM_SENTINEL), 0o755)
        _write(self.bindir / "curl", "#!/bin/sh\nprintf 200\n", 0o755)
        subprocess.run(["git", "init", "-q", str(self.root)], check=True, timeout=60)
        _write(self.root / "output/single/manifest.yaml",
               'topology: single\ngpus_per_node: 1\ngpu_model: "NVIDIA GB10"\n')
        self.model = self.root / "models/fx-dense"
        _write(self.model / "config.json", json.dumps({"hidden_size": 64, "num_hidden_layers": 2}))
        header = json.dumps({"w": {"dtype": "F16", "shape": [1365000000],
                                   "data_offsets": [0, 2730000000]}}).encode("utf-8")
        (self.model / "model.safetensors").write_bytes(struct.pack("<Q", len(header)) + header)
        self.camp = self.root / "campaigns" / CAMP
        _write(self.root / "campaigns/ACTIVE", CAMP + "\n")
        self.declare(repeats=3)
        self.env = dict(os.environ, PATH=f"{self.bindir}{os.pathsep}{os.environ.get('PATH', '')}",
                        PYTHONDONTWRITEBYTECODE="1")

    def declare(self, repeats=None, cells=("cell-a", "cell-b")):
        budgets = {"smoke_budget_overhead_mib": 1, "ready_max_seconds": 1}
        if repeats is not None:
            budgets["repeats"] = repeats
        _write(self.camp / "campaign.yaml", json.dumps({
            "schema_version": 1, "id": CAMP, "plan_ref": "docs/plan/fixture.md",
            "nodes": [{"node_id": "main", "role": "main", "topology": "single"}],
            "assignments": {"main": [{"cell": c} for c in cells]}, "budgets": budgets,
            "control_variables": {"model": "m", "vllm_version": "0.0.0", "topology": "single",
                                  "target_gpu": "fixture"}}))
        for c in cells:
            _write(self.camp / "cells" / c / "lockset.json", json.dumps({"id": c, "provenance": "hand-authored"}))

    def config(self, cfg: str) -> None:
        _write(self.root / f"output/single/configs/{cfg}.yaml", f"model: {self.model}\nmax-model-len: 4096\n")
        _write(self.root / f"output/single/envs/.env.{cfg}", f"SERVING_PORT=18080\nSERVING_MODEL_NAME={cfg}\n")

    def sweep_dir(self, cfg: str) -> Path:
        return self.root / "output/single/benchlog" / f"sweep_{cfg}"

    def calls(self) -> list[str]:
        p = self.root / "SHIM_CALLS"
        return p.read_text(encoding="utf-8").splitlines() if p.is_file() else []

    def reset_calls(self) -> None:
        (self.root / "SHIM_CALLS").unlink(missing_ok=True)

    def clear_events(self) -> None:
        """사례마다 블랙박스 events 를 비운다 — 앞 사례의 사살이 뒤 사례의 창에 겹치지 않게(초 단위 시각)."""
        shutil.rmtree(self.root / "docs" / "logs", ignore_errors=True)

    def run(self, argv, plan=None, timeout=170):
        env = dict(self.env)
        if plan is not None:
            env["SHIM_PLAN"] = json.dumps(plan)
        return subprocess.run(argv, cwd=str(self.root), env=env, capture_output=True, text=True, timeout=timeout)

    def sweep(self, cfg, *extra, plan=None, levels="1,2"):
        self.config(cfg)
        return self.run(["bash", str(self.sdir / "sweep_bench.sh"), cfg, "--topology", "single",
                         "--tool", "guidellm", "--bench-budget-mib", "1024", "--backend", "openai",
                         "--levels", levels, "--num-prompts", "4", *extra], plan=plan)

    def py(self, script, *args):
        return self.run([sys.executable, str(self.sdir / script), *args])

    @staticmethod
    def load(path: Path):
        try:
            return json.loads(Path(path).read_text(encoding="utf-8"))
        except (OSError, ValueError):
            return None


def main() -> int:
    verbose = "-v" in sys.argv
    failures: list[str] = []

    def ck(name, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " — " + str(detail)[-700:]))
        if not cond:
            failures.append(name)

    if shutil.which("git") is None:
        print("[selftest_sweep_repeats] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)",
              file=sys.stderr)
        return 1
    with tempfile.TemporaryDirectory(prefix="sweep-repeats.") as td:
        sb = Sandbox(Path(td))
        ck("사본의 부하 도구는 shim 이다(실제 run_bench/lite_bench 가 아니다)",
           SHIM_SENTINEL in (sb.sdir / "run_bench.sh").read_text(encoding="utf-8")
           and SHIM_SENTINEL in (sb.sdir / "lite_bench.sh").read_text(encoding="utf-8"))

        # ── N1 정상 ────────────────────────────────────────────────────────────────────────
        plan = {"tpot": {"1": [25.0, 24.0, 26.0], "2": [30.0, 30.0, 30.0]}}
        cp = sb.sweep("cfg-n", plan=plan)
        cp_n1_stdout = cp.stdout
        d = sb.sweep_dir("cfg-n")
        idx = sb.load(d / "sweep_index.json") or {}
        m1 = sb.load(d / "level_01/measured.json") or {}
        if verbose:
            print(cp.stdout[-1500:], cp.stderr[-1500:])
        ck("N1 rc=0 · 레벨 2 × 반복 3 = shim 6 호출 + lite 1 호출",
           cp.returncode == 0 and len([c for c in sb.calls() if "level=" in c]) == 6
           and len([c for c in sb.calls() if c.endswith(" lite")]) == 1, (cp.returncode, sb.calls(), cp.stderr))
        ck("N1 run 1 은 level_NN/ 에, run 2·3 은 level_NN/run_KK/ 에",
           (d / "level_01/measured.json").is_file() and (d / "level_01/run_02/measured.json").is_file()
           and (d / "level_02/run_03/measured.json").is_file())
        ck("N1 runs[] 3 · 완주 3 · 밴드 (1000/24−1000/26)/mean×100 · measured(n=3) · warm-rerun",
           len(m1.get("runs") or []) == 3 and m1.get("repeats_completed") == 3
           and m1.get("repro_band_source") == "measured(n=3)" and isinstance(m1.get("repro_band_pct"), float)
           and 7.0 < m1["repro_band_pct"] < 9.0 and m1.get("repeat_kind") == "warm-rerun", m1)
        ck("N1 대표 run 필드 그대로 — decode_tps=1000/25=40.0(평균 ✗) · accept_len 2.0(lite warm 승계) · 대표 run 1",
           m1.get("decode_tps") == 40.0 and m1.get("accept_len") == 2.0 and m1.get("representative_run") == 1
           and m1.get("bench_tool_version") == "0.0.0+selftest-shim", {k: m1.get(k) for k in
                                                                     ("decode_tps", "accept_len", "representative_run")})
        rep = idx.get("repetition") or {}
        ck("N1 index.repetition — 요청 3 · 출처=캠페인 선언 · 레벨별 완주 min 3 · 중단 없음 · runs 6",
           rep.get("requested") == 3
           and rep.get("requested_source") == f"declared(campaigns/{CAMP}/campaign.yaml budgets.repeats)"
           and rep.get("completed_min") == 3 and rep.get("stop") is None and rep.get("runs_attempted") == 6, rep)

        # ── N2 judge_bench 승계(③ 계약) ──────────────────────────────────────────────────────
        out_j = Path(td) / "rejudge"
        cp = sb.run(["bash", str(sb.sdir / "judge_bench.sh"), "cfg-n", "--topology", "single",
                     "--authority", "explore", "--e-search", "empty", "--out-dir", str(out_j)])
        roof = sb.load(out_j / "roofline.json") or {}
        ck("N2 judge_bench 가 대표 run 의 accept_len 을 승계한다(accept_len_source=measured · 근거 level_01)",
           cp.returncode == 0 and roof.get("accept_len_source") == "measured" and roof.get("accept_len") == 2.0
           and (roof.get("accept_len_evidence") or "").startswith("level_01/measured.json"),
           (cp.returncode, roof, cp.stderr[-400:]))

        # ── N3 정규 경로가 판정 기록을 남긴다 ─────────────────────────────────────────────────
        def classify(cfg, events=None, *extra):
            """판정 소유자 CLI(bench-mode 모드) 를 직접 부른다 — 기록을 쓰지 않고 stdout 판정만 본다."""
            args = ["--sweep-index", str(sb.sweep_dir(cfg) / "sweep_index.json"), *extra]
            if events is not None:
                evf = Path(td) / ("events_%s.jsonl" % cfg)
                evf.write_text("".join(json.dumps(e) + "\n" for e in events), encoding="utf-8")
                args += ["--events", str(evf)]
            cp_ = sb.py("classify_cell.py", *args)
            try:
                return json.loads(cp_.stdout), cp_
            except ValueError:
                return {}, cp_

        def side(cfg):
            return sb.load(sb.sweep_dir(cfg) / "bench_mode.json") or {}

        s_n = side("cfg-n")
        ck("N3 정규 경로 — sweep_bench 종료부가 판정 기록을 쓴다(bench_mode=full · 사유 null · index 측정시각에 묶임)",
           s_n.get("bench_mode") == "full" and s_n.get("downgrade_reason") is None
           and s_n.get("sweep_index_generated_utc") == idx.get("generated_utc")
           and s_n.get("downgrade_correlation") == "not_applicable", s_n)
        ck("N3 sweep_bench 로그에 판정이 보인다(부르기만 하고 판정하지 않는다)",
           "[sweep_bench] bench_mode=full" in cp_n1_stdout, cp_n1_stdout[-600:])

        # ── N4 render_report ──────────────────────────────────────────────────────────────
        verdict_pass = Path(td) / "verdict_pass.json"
        verdict_pass.write_text(json.dumps({"verdict": "PASS", "measured_decode_tps": m1.get("decode_tps"),
                                            "rubric": {"authority": "explicit", "source": "c", "primary": 30.0,
                                                       "floor": 27.0, "ratio_M_over_primary": 1.33,
                                                       "tolerance": 0.1}}), encoding="utf-8")

        def report(cfg, *extra):
            return sb.py("render_report.py", "--sweep-index", str(sb.sweep_dir(cfg) / "sweep_index.json"),
                         "--verdict-json", str(verdict_pass), "--stdout", *extra)

        def publish(cfg):
            return sb.py("publish_benchmark_record.py", "--sweep-index", str(sb.sweep_dir(cfg) / "sweep_index.json"),
                         "--verdict-json", str(verdict_pass), "--stdout")
        cp = report("cfg-n")
        md = cp.stdout
        ck("N4 리포트 반복 절 — 완주/요청 3/3 · measured(n=3) · bench_mode full",
           cp.returncode == 0 and "## 반복 축 · 재현 밴드" in md and "| 1 | 3/3 |" in md
           and "measured(n=3)" in md and "bench_mode: **full**" in md
           and "| 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |" in md,
           (cp.returncode, md[-900:], cp.stderr[-300:]))
        sys.path.insert(0, str(REPO / ".claude/skills/hint-publisher/scripts"))
        try:
            import render_bench_section as _rbs   # 배포되는 hint 파서 그대로(읽기 전용)
            parsed = _rbs.parse_report(md)
            ok = [lv.get("level") for lv in parsed["levels"]] == [1, 2] and parsed["truncated"] == []
        except Exception as exc:   # noqa: BLE001 — 형식 계약 위반은 여기서 드러난다
            ok, parsed = False, exc
        ck("N4 hint 파서(render_bench_section.parse_report)가 스윕 표·절삭 블록을 그대로 읽는다(형식 계약 불변)",
           ok, parsed)

        # ── N5 인증서 — 대표 run 1회 바인딩 ─────────────────────────────────────────────────
        cp = publish("cfg-n")
        cert = cp.stdout
        ck("N5 인증서 1건 — decode_tps_conc1=대표 run 40.0 · measured_utc 1줄=index 측정시각 · runs 미포함",
           cp.returncode == 0 and "decode_tps_conc1: 40.0" in cert
           and cert.count("measured_utc:") == 1 and ('measured_utc: "%s"' % idx.get("generated_utc")) in cert
           and "runs" not in cert and "benchmark_mode: full" in cert, (cp.returncode, cert[-500:], cp.stderr[-300:]))

        # ── N8 판정 기록 결손은 리포트를 막지 않고 인증서는 막는다 ──────────────────────────────
        side_path = d / "bench_mode.json"
        good_side = side_path.read_text(encoding="utf-8")
        side_path.write_text(json.dumps(dict(json.loads(good_side), sweep_index_generated_utc="2020-01-01T00:00:00Z")),
                             encoding="utf-8")
        cp = report("cfg-n")
        cp_pub = publish("cfg-n")
        ck("N8★ 다른 측정의 판정 기록(stale) → 리포트 rc 0 · '미확정 — stale(' · 인증서 미발행",
           cp.returncode == 0 and "bench_mode: 미확정 — stale(" in cp.stdout
           and cp_pub.returncode == 0 and cp_pub.stdout.strip() == "" and "stale(" in cp_pub.stderr,
           (cp.returncode, cp.stdout[-500:], cp_pub.stdout[-200:], cp_pub.stderr[-300:]))
        side_path.write_text('{"bench_mode": "fu', encoding="utf-8")
        cp = report("cfg-n")
        cp_x = report("cfg-n", "--bench-mode-json", str(side_path))
        ck("N8★ 반쯤 쓰인 판정 기록 → 관례 자리는 rc 0 · '미확정 — unreadable(' / 명시 경로는 exit 2",
           cp.returncode == 0 and "bench_mode: 미확정 — unreadable(" in cp.stdout and cp_x.returncode == 2,
           (cp.returncode, cp.stdout[-400:], cp.stderr[-300:], cp_x.returncode))
        side_path.write_text(good_side, encoding="utf-8")

        # ── N6 재조립은 측정 시점의 요청 반복을 승계한다 ─────────────────────────────────────
        sb.declare(repeats=5)
        sb.reset_calls()
        side_path.unlink()
        cp = sb.run(["bash", str(sb.sdir / "sweep_bench.sh"), "cfg-n", "--topology", "single",
                     "--backend", "openai", "--reassemble-only"])
        idx_r = sb.load(sb.sweep_dir("cfg-n") / "sweep_index.json") or {}
        rep_r = idx_r.get("repetition") or {}
        ck("N6 재조립 — shim 미호출 · 요청 3·출처 승계(선언 5 를 다시 읽지 않는다) · 완주 min 3 · 측정시각 승계",
           cp.returncode == 0 and sb.calls() == [] and rep_r.get("requested") == 3
           and rep_r.get("requested_source") == f"declared(campaigns/{CAMP}/campaign.yaml budgets.repeats)"
           and rep_r.get("completed_min") == 3 and idx_r.get("generated_utc") == idx.get("generated_utc"),
           (cp.returncode, sb.calls(), rep_r, cp.stderr[-400:]))
        ck("N6 재조립도 판정 기록을 다시 쓴다(같은 측정시각에 묶인 full)",
           side("cfg-n").get("bench_mode") == "full"
           and side("cfg-n").get("sweep_index_generated_utc") == idx.get("generated_utc"), side("cfg-n"))
        sb.declare(repeats=3)
        cp = sb.run(["bash", str(sb.sdir / "sweep_bench.sh"), "cfg-n", "--topology", "single",
                     "--backend", "openai", "--reassemble-only", "--repeats", "4"])
        ck("A3★ --reassemble-only 에 --repeats → exit 2(측정하지 않은 반복 수를 조용히 버리지 않는다)",
           cp.returncode == 2 and "--reassemble-only" in cp.stderr, (cp.returncode, cp.stderr[-300:]))

        # ── N7 재스윕이 옛 run 을 섞지 않는다 ───────────────────────────────────────────────
        cp1 = sb.sweep("cfg-s", "--repeats", "4", levels="1")
        had4 = (sb.sweep_dir("cfg-s") / "level_01/run_04/measured.json").is_file()
        cp2 = sb.sweep("cfg-s", levels="1")
        ms = sb.load(sb.sweep_dir("cfg-s") / "level_01/measured.json") or {}
        ck("N7★ --repeats 4 뒤 기본 3 재스윕 → run_04 제거 · runs[] 3 · 요청 3",
           cp1.returncode == 0 and had4 and cp2.returncode == 0
           and not (sb.sweep_dir("cfg-s") / "level_01/run_04").exists()
           and len(ms.get("runs") or []) == 3 and ms.get("repeats_requested") == 3,
           (cp1.returncode, had4, cp2.returncode, [r.get("run") for r in ms.get("runs") or []]))

        # ── R1 재조립은 기존 index levels[] 만 복원한다 ─────────────────────────────────────
        cp1 = sb.sweep("cfg-r")
        cp2 = sb.sweep("cfg-r", plan={"fail": {"1:2": "measurement"}})
        lv_mid = [lv.get("level") for lv in (sb.load(sb.sweep_dir("cfg-r") / "sweep_index.json") or {}).get("levels", [])]
        stale_on_disk = (sb.sweep_dir("cfg-r") / "level_02/measured.json").is_file()
        cp3 = sb.run(["bash", str(sb.sdir / "sweep_bench.sh"), "cfg-r", "--topology", "single",
                      "--backend", "openai", "--reassemble-only"])
        idx_rr = sb.load(sb.sweep_dir("cfg-r") / "sweep_index.json") or {}
        ck("R1★ 판정점에서 끊긴 재스윕 뒤 재조립 → levels [1] 유지(디스크의 옛 level_02 부활 ✗) · 판정 기록 lite 유지",
           cp1.returncode == 0 and cp2.returncode == 0 and lv_mid == [1] and stale_on_disk and cp3.returncode == 0
           and [lv.get("level") for lv in idx_rr.get("levels", [])] == [1]
           and (idx_rr.get("repetition") or {}).get("completed_by_level") == {"1": 1}
           and side("cfg-r").get("bench_mode") == "lite",
           (cp1.returncode, cp2.returncode, lv_mid, stale_on_disk, cp3.returncode,
            [lv.get("level") for lv in idx_rr.get("levels", [])], cp3.stderr[-300:]))

        # ── F1 실패주입: 판정점 run 2 measurement_ok=false ──────────────────────────────────
        sb.clear_events()
        sb.reset_calls()
        cp = sb.sweep("cfg-f", plan={"fail": {"1:2": "measurement"}})
        d = sb.sweep_dir("cfg-f")
        idx_f = sb.load(d / "sweep_index.json") or {}
        m1 = sb.load(d / "level_01/measured.json") or {}
        trunc = (d / "truncation.log").read_text(encoding="utf-8") if (d / "truncation.log").is_file() else ""
        ck("F1★ 대표 run 은 섰으므로 rc=0 · run 2 에서 멈춘다(shim 2 호출 · level 2 미측정)",
           cp.returncode == 0 and len([c for c in sb.calls() if "level=" in c]) == 2
           and not (d / "level_02/measured.json").exists(), (cp.returncode, sb.calls(), cp.stderr[-400:]))
        stop_f = (idx_f.get("repetition") or {}).get("stop") or {}
        ck("F1★ 즉시 신호 — truncation.log · index.repetition.stop(repeat-break · level 1 run 2) · runs[] 2 · 완주 1 · 밴드 없음",
           "repeat-break" in trunc and stop_f.get("kind") == "repeat-break" and stop_f.get("run") == 2
           and len(m1.get("runs") or []) == 2 and m1.get("repeats_completed") == 1
           and m1.get("repro_band_pct") is None and str(m1.get("repro_band_source")).startswith("unavailable(n=1"),
           (trunc, idx_f.get("repetition"), m1.get("runs")))
        s_f = side("cfg-f")
        ck("F1★ 판정 기록(정규 경로) → lite · run_failed · 사살 대조 not_scanned(블랙박스 events 0)",
           s_f.get("bench_mode") == "lite" and s_f.get("downgrade_reason") == "run_failed"
           and s_f.get("downgrade_correlation") == "not_scanned", s_f)

        in_ts = stop_f.get("window_end_utc")
        cls, _ = classify("cfg-f", [{"ts": in_ts, "kind": "watchdog_kill_ack", "mode": "armed"}])
        ck("F2★ 끊긴 창 안 집행 사살(시각 일치) → lite · blackbox_kill · matched",
           cls.get("bench_mode") == "lite" and cls.get("downgrade_reason") == "blackbox_kill"
           and cls.get("downgrade_correlation") == "matched", cls)
        cls, _ = classify("cfg-f", [{"ts": in_ts, "kind": "thermal_trip", "mode": "armed"}])
        ck("F2★ 음성대조: 같은 시각 트립 단독 → blackbox_kill ✗ · run_failed · miss",
           cls.get("downgrade_reason") == "run_failed" and cls.get("downgrade_correlation") == "miss", cls)
        cls, _ = classify("cfg-f", [{"ts": "2020-01-01T00:00:00Z", "kind": "watchdog_kill_ack", "mode": "armed"}])
        ck("F2★ 음성대조: 사살 시각 불일치(창 밖) → run_failed", cls.get("downgrade_reason") == "run_failed", cls)

        cp = publish("cfg-f")
        ck("F3★ 강등 셀(판정 기록 lite)에는 PASS 여도 full 인증서를 내지 않는다(rc 0 · stdout 비어 있음)",
           cp.returncode == 0 and cp.stdout.strip() == "" and "downgraded-lite" in cp.stderr, (cp.stdout[-300:], cp.stderr))
        cp = report("cfg-f")
        ck("F3★ 리포트에 멈춘 자리·실패 run·강등 사유가 이름으로 남는다",
           "**스윕이 멈춘 자리 — 반복 중단**: level 1 run 2" in cp.stdout and "✗(run 2)" in cp.stdout
           and "**강등** · 사유 `run_failed`" in cp.stdout, cp.stdout[-900:])

        # ── F4 비대칭 교정: 경계 레벨 2 의 run 2 실패 ───────────────────────────────────────
        sb.clear_events()
        cp = sb.sweep("cfg-g", plan={"fail": {"2:2": "measurement"}})
        rep = (sb.load(sb.sweep_dir("cfg-g") / "sweep_index.json") or {}).get("repetition") or {}
        s_g = side("cfg-g")
        cp_pub = publish("cfg-g")
        ck("F4★ 경계 레벨 2 run 2 실패(판정점 3 · 레벨2 1) → full(적응 상한 클램프) · 인증서 발행",
           cp.returncode == 0 and rep.get("completed_by_level") == {"1": 3, "2": 1}
           and (rep.get("stop") or {}).get("kind") == "repeat-break"
           and s_g.get("bench_mode") == "full" and "적응 상한 클램프" in (s_g.get("bench_mode_source") or "")
           and "benchmark_mode: full" in cp_pub.stdout, (rep, s_g, cp_pub.stderr[-300:]))

        # ── F5·F6·F7 클램프 레벨(첫 run 실패) ───────────────────────────────────────────────
        sb.clear_events()
        cp = sb.sweep("cfg-k", plan={"fail": {"2:1": "kill"}})
        idx_k = sb.load(sb.sweep_dir("cfg-k") / "sweep_index.json") or {}
        rep_k = idx_k.get("repetition") or {}
        s_k = side("cfg-k")
        cp_pub = publish("cfg-k")
        ck("F5★ 클램프 레벨 2 첫 run 사살 → stop.kind=clamp · 판정 기록 lite · blackbox_kill · 인증서 미발행",
           cp.returncode == 0 and [lv.get("level") for lv in idx_k.get("levels", [])] == [1]
           and (rep_k.get("stop") or {}).get("kind") == "clamp" and rep_k.get("runs_attempted") == 4
           and s_k.get("bench_mode") == "lite" and s_k.get("downgrade_reason") == "blackbox_kill"
           and cp_pub.stdout.strip() == "", (rep_k, s_k, cp.stderr[-400:], cp_pub.stderr[-300:]))
        sb.clear_events()
        cp = sb.sweep("cfg-t", plan={"fail": {"2:1": "trip"}})
        s_t = side("cfg-t")
        ck("F6★ 같은 자리 트립 단독 → full 유지(사인 불충분 · 대조 miss)",
           cp.returncode == 0 and s_t.get("bench_mode") == "full" and s_t.get("downgrade_correlation") == "miss"
           and "사인 불충분" in (s_t.get("bench_mode_source") or ""), s_t)
        sb.clear_events()
        cp = sb.sweep("cfg-c", plan={"fail": {"2:1": "rc"}})
        s_c = side("cfg-c")
        ck("F7 경계 레벨 2 첫 run 실패(rc) · events 0 → full · 대조 not_scanned(F4 와 같은 판정)",
           cp.returncode == 0 and s_c.get("bench_mode") == "full" and s_c.get("downgrade_correlation") == "not_scanned",
           s_c)

        # ── F8 집계 실패 ──────────────────────────────────────────────────────────────────
        cp = sb.sweep("cfg-u", plan={"fail": {"1:2": "norecord"}}, levels="1")
        rep_u = (sb.load(sb.sweep_dir("cfg-u") / "sweep_index.json") or {}).get("repetition") or {}
        s_u = side("cfg-u")
        cp_pub = publish("cfg-u")
        ck("F8★ 경계 사실 기록 실패 → runs[] 없는 레벨 [1] · 판정 기록 null(not_evaluated) · full 인증서 미발행",
           cp.returncode == 0 and rep_u.get("unrecorded_levels") == [1] and rep_u.get("requested") == 3
           and s_u.get("bench_mode") is None and str(s_u.get("bench_mode_source")).startswith("not_evaluated(")
           and cp_pub.returncode == 0 and cp_pub.stdout.strip() == "" and "판정 기록" in cp_pub.stderr,
           (rep_u, s_u, cp_pub.stdout[-200:], cp_pub.stderr[-300:]))

        # ── V1 분산만 크다 ────────────────────────────────────────────────────────────────
        cp = sb.sweep("cfg-v", plan={"tpot": {"1": [20.0, 30.0, 45.0]}}, levels="1")
        mv = sb.load(sb.sweep_dir("cfg-v") / "level_01/measured.json") or {}
        s_v = side("cfg-v")
        ck("V1★ 밴드 > 60% 인데도 full · 사유 null(분산은 강등 사유가 아니다 · 기재만)",
           cp.returncode == 0 and (mv.get("repro_band_pct") or 0) > 60 and s_v.get("bench_mode") == "full"
           and s_v.get("downgrade_reason") is None, (mv.get("repro_band_pct"), s_v))

        # ── A1·A2 선언 위반은 부하 전에 멈춘다 ──────────────────────────────────────────────
        sb.reset_calls()
        cp = sb.sweep("cfg-a", "--repeats", "2")
        ck("A1★ --repeats 2 → exit 2 · lite·레벨 shim 미호출(부하 전 거부)",
           cp.returncode == 2 and sb.calls() == [] and "full 정의" in cp.stderr, (cp.returncode, sb.calls(), cp.stderr[-300:]))
        sb.declare(repeats=2)
        cp = sb.sweep("cfg-a")
        ck("A2★ 캠페인 선언 budgets.repeats=2 → exit 2 · 미호출(기본값 대체 ✗)",
           cp.returncode == 2 and sb.calls() == [], (cp.returncode, sb.calls(), cp.stderr[-300:]))
        sb.declare(repeats=3)

        # ── B broad_search 배선 ─────────────────────────────────────────────────────────────
        sb.clear_events()
        state = sb.camp / "sweeps" / "bs.json"
        bs = str(sb.sdir / "broad_search.sh")
        now = "2026-01-01T00:00:00Z"
        cp = sb.run(["bash", bs, "init", "--sweep-id", "bs", "--state", str(state), "--cells", "cell-a,cell-b",
                     "--control-variable", "fixture", "--max-cells", "10", "--wall-clock-budget-s", "999999999",
                     "--consecutive-failure-limit", "5", "--declared-by", "selftest", "--basis", "fixture",
                     "--authority", "explore", "--now-utc", now, "--topology", "single"])
        st = sb.load(state) or {}
        db = st.get("declared_budget") or {}
        ck("B1 init → declared_budget.repeats=3 · 출처=상태 파일의 캠페인 선언",
           cp.returncode == 0 and db.get("repeats") == 3
           and db.get("repeats_source") == f"declared(campaigns/{CAMP}/campaign.yaml budgets.repeats)",
           (cp.returncode, db, cp.stderr[-300:]))

        def cell(key, cfg, plan_):
            sb.config(cfg)
            return sb.run(["bash", bs, "cell", "--state", str(state), "--cell-key", key, "--config", cfg,
                           "--axis-citation", "fixture", "--next-intent", "fixture", "--bench-budget-mib", "1024",
                           "--backend", "openai", "--levels", "1", "--num-prompts", "4", "--now-utc", now,
                           "--topology", "single", "--confirm-risk"], plan=plan_)

        def record(key, path=None):
            rows = [c for c in (sb.load(path or state) or {}).get("cells") or [] if c.get("cell_key") == key]
            return rows[-1] if rows else {}

        sb.reset_calls()
        cp = cell("cell-a", "cfg-ba", {"tpot": {"1": [25.0, 25.5, 24.5]}})
        rec = record("cell-a")
        stop = sb.load(sb.camp / "sweeps" / "bs.stop.json") or {}
        ma = sb.load(sb.sweep_dir("cfg-ba") / "level_01/measured.json") or {}
        ck("B1 cell 이 스윕 선언의 반복 수를 sweep_bench 에 넘긴다(출처에 sweep state 경유가 남는다)",
           cp.returncode == 0 and len([c for c in sb.calls() if "level=" in c]) == 3
           and "sweep state declared_budget.repeats" in (ma.get("repeats_requested_source") or ""),
           (cp.returncode, sb.calls(), ma.get("repeats_requested_source"), cp.stderr[-600:]))
        ck("B2 셀 기록 bench_mode=full(판정 기록 그대로 · 대조 필드 포함) · repetition(요청 3·완주 3)",
           rec.get("cell_outcome") == "measured" and rec.get("bench_mode") == "full"
           and rec.get("bench_mode_source") == side("cfg-ba").get("bench_mode_source")
           and rec.get("downgrade_correlation") == side("cfg-ba").get("downgrade_correlation")
           and (rec.get("repetition") or {}).get("completed_min") == 3,
           (rec.get("cell_outcome"), rec.get("bench_mode"), rec.get("bench_mode_source"), cp.stderr[-400:]))
        ck("B2 정지 평가가 반복 소비를 기재한다(runs_attempted 3 · 선언 불일치 없음)",
           stop.get("runs_attempted") == 3 and stop.get("repeats_mismatch") == []
           and (stop.get("declared_budget") or {}).get("repeats") == 3, stop)

        cp = cell("cell-b", "cfg-bb", {"fail": {"1:2": "kill"}})
        rec = record("cell-b")
        ck("B3★ shim 이 run 2 에 사살 이벤트를 남기고 죽는다 → 셀 기록 lite · blackbox_kill(events 출처)",
           cp.returncode == 0 and rec.get("cell_outcome") == "measured" and rec.get("bench_mode") == "lite"
           and rec.get("downgrade_reason") == "blackbox_kill"
           and str(rec.get("downgrade_reason_source")).startswith("events("),
           (rec.get("cell_outcome"), rec.get("bench_mode"), rec.get("downgrade_reason"),
            rec.get("downgrade_reason_source"), cp.stderr[-600:]))

        legacy = sb.camp / "sweeps" / "legacy.json"
        doc = sb.load(state)
        doc["declared_budget"].pop("repeats")
        doc["declared_budget"].pop("repeats_source")
        doc["cells_remaining"] = ["cell-a"]
        legacy.write_text(json.dumps(doc), encoding="utf-8")
        sb.reset_calls()
        cp = sb.run(["bash", bs, "cell", "--state", str(legacy), "--cell-key", "cell-a", "--config", "cfg-ba",
                     "--axis-citation", "fixture", "--next-intent", "fixture", "--bench-budget-mib", "1024",
                     "--backend", "openai", "--levels", "1", "--now-utc", now, "--topology", "single",
                     "--confirm-risk"])
        ck("B4★ declared_budget.repeats 없는 상태 파일 → 셀 진입 exit 2 · 부하 도구 미호출",
           cp.returncode == 2 and sb.calls() == [] and "declared_budget.repeats 미선언" in cp.stderr,
           (cp.returncode, sb.calls(), cp.stderr[-400:]))
        cp_s = sb.run(["bash", bs, "status", "--state", str(legacy), "--now-utc", now])
        st_s = {}
        try:
            st_s = json.loads(cp_s.stdout)
        except ValueError:
            pass
        out_md = Path(td) / "legacy_map.md"
        cp_m = sb.run(["bash", bs, "map", "--state", str(legacy), "--now-utc", now, "--out-md", str(out_md)])
        ck("B5★ 같은 상태 파일의 status(rc 0|3)·map(rc 0) 은 막히지 않는다 — 반복 수 None · 출처 absent · 지도에 경고",
           cp_s.returncode in (0, 3) and (st_s.get("declared_budget") or {}).get("repeats", "x") is None
           and str(st_s.get("budget_repeats_source")).startswith("absent(")
           and cp_m.returncode == 0 and out_md.is_file()
           and "예산 선언에 반복 수가 없다" in out_md.read_text(encoding="utf-8"),
           (cp_s.returncode, cp_s.stdout[-300:], cp_s.stderr[-300:], cp_m.returncode, cp_m.stderr[-300:]))

    if failures:
        print("[selftest_sweep_repeats] FAIL %d건: %s" % (len(failures), failures), file=sys.stderr)
        return 1
    print("[selftest_sweep_repeats] PASS — N1~N8 · R1 · F1~F8 · V1 · A1~A3 · B1~B5(실패주입·음성대조 포함)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
