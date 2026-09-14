#!/usr/bin/env python3
"""selftest_lite_report.py — lite hint 통로의 **리포트 평면** 격리 자체검사 (plan_26091407 §4.5 · §7 O5).

왜 있는가: lite 만 잰 셀도 hint 를 낸다(사용자 결정 Q4·Q10). 그러려면 lite 가 바인딩할 문서를 **실제로** 만들어야 하는데,
종전 `lite_bench.sh` 는 raw JSON 과 채팅 표만 남겼다(`docs/benchmark` 참조 0건 · audit_26091323 §1.2 A2). 이 파일은
배포되는 `lite_bench.sh` 를 **바이트 사본 그대로** 격리 저장소에서 돌려, 종결부가 경량 리포트를 발행하는지와 그 리포트가
발행기 계약(측정 구성 표 · lite 지표 표 · `mode: lite`)을 지키는지를 친다. 단위 함수 시험으로는 셸 종결부의 배선이
실제 경로에 서 있는지 증명하지 못한다.

격리 방식(테스트 평면의 외부 의존 차단 — 정당한 모킹 · 산출물이 실측처럼 보이지 않는다):
  · 임시 git 저장소에 lite_bench.sh · lite_metrics.py · render_report.py · classify_cell.py · doc_naming.py · repeat_axis.py
    의 **사본**을 같은 상대경로로 둔다(재구현 ✗).
  · `docker`·`curl`·`nvidia-smi` 는 PATH 앞단 shim 이다 — vllm bench serve 를 돌리지 않고 픽스처 결과 JSON 을 돌려준다.
  · 테라포밍 Flag 게이트(`manifest_contract.py --require-flag`)는 사본 자리의 **stub** 이 통과시킨다. 게이트 자체는 이
    시험의 대상이 아니다(게이트 회귀는 benchmark_info_only_gate 가 지킨다). stub 은 SENTINEL 로 표시한다.
  · GPU·서빙·NAS·네트워크 불요 · 실 저장소의 docs/benchmark·campaigns·태그에 닿지 않는다.

사례:
  L1 기본(플래그 없음) = 자동 핸드오프 경로 → exit 0 · 리포트 **미발행** · raw 에 측정시각·명명 입력
  L2 --publish-report → exit 0 · 리포트 1건 · 이름 = 명명 SSOT = publisher 정규식 · mode: lite · 측정 구성 표(선언 lite)
     · lite 지표 술어 참 · publish-lite-report 표지 행 · 본문 측정시각 = raw 측정시각(같은 측정 판정 근거)
  L3 같은 raw 재렌더 → 같은 이름 덮어쓰기(멱등 · 파일 1건)
  L4 ★manifest gpu_model 부재 + --publish-report → exit 5 · 리포트 0 · raw 는 남는다(측정은 잃지 않는다)
  L5 ★raw 측정시각 부재 → render_report exit 2 · 리포트 0(시각을 날조하지 않는다)
  L6 ★모드 혼합(--lite-only + --sweep-index) · full 인자 누락 → exit 2
  L7 ★full 스윕의 lite 레그는 --publish-report 를 넘기지 않는다(같은 시간대 full 리포트의 stem 을 밀지 않는다)
  L8 명명 키 교차검증 — 같은 입력에서 sweep_bench 조립 heredoc(실행)과 render_report.lite_identity 가 같은 model·gpu_key·vllm
  L9 full 리포트 측정 구성 표 — 판정 기록 full/강등(lite·run_failed)/부재 → 표 값 · 강등만 lite 표지 행 · 스윕 표 형식 계약 불변
  L10 두 스킬 사이의 계약 상수(측정 구성 제목·키 · lite 도구 토큰)가 같다

사용: python3 selftest_lite_report.py [-v]   (exit 0 = 전부 통과)
"""
from __future__ import annotations

import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

SDIR = Path(__file__).resolve().parent
REPO = Path(__file__).resolve().parents[4]
AB = ".claude/skills/adversarial-benchmark/scripts"
COPIES = ("lite_bench.sh", "lite_metrics.py", "render_report.py", "classify_cell.py", "doc_naming.py", "repeat_axis.py")
SHIM_SENTINEL = "selftest-lite-report-shim"
CFG = "fx-lite"
MEASURED_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")

DOCKER_SHIM = r'''#!/usr/bin/env bash
# @SENTINEL@ — docker 가 아니다(vllm bench serve 를 돌리지 않는다).
case "$1" in
  ps) echo "cafe0001" ;;
  exec)
    shift; shift
    if [ "$1" = "cat" ]; then cat "$SHIM_DIR/bench.json"; exit 0; fi
    exit 0 ;;
  logs) cat "$SHIM_DIR/engine.log" ;;
  *) exit 0 ;;
esac
'''
MC_STUB = '''#!/usr/bin/env python3
# @SENTINEL@ — 테라포밍 Flag 게이트 stub(이 시험의 대상이 아니다 · 실 게이트는 benchmark_info_only_gate 가 지킨다).
import sys
sys.exit(0)
'''


def _load(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.path.insert(0, str(path.parent))
    spec.loader.exec_module(module)
    return module


def _write(path: Path, text: str, mode: int | None = None) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    if mode is not None:
        path.chmod(mode)


class Sandbox:
    def __init__(self, tmp: Path):
        self.root = tmp / "repo"
        self.bindir = tmp / "bin"
        self.shimdir = tmp / "shimdata"
        for name in COPIES:
            _write(self.root / AB / name, (SDIR / name).read_text(encoding="utf-8"))
        _write(self.root / ".claude/skills/terraforming_node/scripts/manifest_contract.py",
               MC_STUB.replace("@SENTINEL@", SHIM_SENTINEL), 0o755)
        _write(self.bindir / "docker", DOCKER_SHIM.replace("@SENTINEL@", SHIM_SENTINEL), 0o755)
        _write(self.bindir / "curl", "#!/bin/sh\nprintf 200\n", 0o755)
        _write(self.bindir / "nvidia-smi", "#!/bin/sh\necho '[N/A], [N/A]'\n", 0o755)
        _write(self.shimdir / "bench.json", json.dumps({"completed": 3, "failed": 0, "median_tpot_ms": 25.0,
                                                        "median_ttft_ms": 118.0, "output_throughput": 39.0}))
        _write(self.shimdir / "engine.log", "INFO GPU KV cache size: 154,192 tokens\n"
                                            "INFO Available KV cache memory: 20.5 GiB\n")
        subprocess.run(["git", "init", "-q", str(self.root)], check=True, timeout=60)
        self.manifest(True)
        _write(self.root / f"output/single/configs/{CFG}.yaml", "model: /models/fx-org/FX-Lite-7B\nmax-model-len: 8192\n")
        _write(self.root / f"output/single/envs/.env.{CFG}",
               f"SERVING_PORT=18080\nSERVING_MODEL_NAME={CFG}\nCONTAINER_NAME=fx-ctr\n"
               "IMAGE_TAG=easy-vllm:9.8.7-cu130-wheel\n")
        self.env = dict(os.environ, PATH=f"{self.bindir}{os.pathsep}{os.environ.get('PATH', '')}",
                        SHIM_DIR=str(self.shimdir), PYTHONDONTWRITEBYTECODE="1")
        self.env.pop("EASY_VLLM_VERSION", None)

    def manifest(self, with_gpu: bool) -> None:
        _write(self.root / "output/single/manifest.yaml",
               "topology: single\ngpus_per_node: 1\n" + ('gpu_model: "NVIDIA GB10"\n' if with_gpu else ""))

    def run(self, argv, timeout=120):
        return subprocess.run(argv, cwd=str(self.root), env=self.env, capture_output=True, text=True, timeout=timeout)

    def lite(self, *extra):
        return self.run(["bash", str(self.root / AB / "lite_bench.sh"), CFG, "--topology", "single",
                         "--backend", "openai", *extra])

    def render(self, *argv):
        return self.run([sys.executable, str(self.root / AB / "render_report.py"), *argv])

    def reports(self) -> list:
        d = self.root / "docs" / "benchmark"
        return sorted(p.name for p in d.glob("bench_report_*.md")) if d.is_dir() else []

    def raw_path(self) -> Path:
        return self.root / "output/single/benchlog" / f"lite_raw_{CFG}.json"


def main() -> int:
    verbose = "-v" in sys.argv
    failures: list = []

    def ck(name, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " — " + str(detail)[-900:]))
        if not cond:
            failures.append(name)

    if shutil.which("git") is None:
        print("[selftest_lite_report] FAIL git 이 없다 — 격리 저장소를 만들 수 없다(부재를 통과로 접지 않는다)", file=sys.stderr)
        return 1
    rbs = _load("_lite_rbs", REPO / ".claude/skills/hint-publisher/scripts/render_bench_section.py")
    rr = _load("_lite_render_report", SDIR / "render_report.py")
    cc = _load("_lite_classify_cell", SDIR / "classify_cell.py")
    dn = _load("_lite_doc_naming", SDIR / "doc_naming.py")
    ep = _load("_lite_evidence_publisher", REPO / ".claude/policies/runtime/evidence_publisher.py")

    with tempfile.TemporaryDirectory(prefix="lite-report-selftest.") as td:
        sb = Sandbox(Path(td))

        # ── L1 기본 = 자동 핸드오프 경로(발행 ✗) ────────────────────────────────────────────────────
        cp = sb.lite()
        if verbose:
            print(cp.stdout[-1500:], cp.stderr[-1500:])
        raw = json.loads(sb.raw_path().read_text(encoding="utf-8")) if sb.raw_path().is_file() else {}
        ck("L1 기본(플래그 없음) → exit 0 · 리포트 미발행 · 미발행을 이름으로 알린다",
           cp.returncode == 0 and sb.reports() == [] and "리포트 미발행" in cp.stdout, (cp.returncode, cp.stderr))
        ck("L1 raw 에 측정시각(UTC 초)·명명 입력(config_name·config_yaml·env_file·manifest)이 실린다",
           bool(MEASURED_RE.match(str(raw.get("measured_utc", "")))) and raw.get("config_name") == CFG
           and all(Path(str(raw.get(k, ""))).is_file() for k in ("config_yaml", "env_file", "manifest")), raw)

        # ── L2 --publish-report ────────────────────────────────────────────────────────────────────
        cp = sb.lite("--publish-report")
        raw = json.loads(sb.raw_path().read_text(encoding="utf-8"))
        names = sb.reports()
        text = (sb.root / "docs/benchmark" / names[0]).read_text(encoding="utf-8") if len(names) == 1 else ""
        ck("L2 --publish-report → exit 0 · 경량 리포트 정확히 1건", cp.returncode == 0 and len(names) == 1,
           (cp.returncode, names, cp.stderr))
        identity = rr.lite_identity(raw, environ={}) if raw else {}
        ck("L2 이름 = 명명 SSOT(doc_naming.bench_filename) = publisher 정규식(변경 없음)",
           names and names[0] == dn.bench_filename("bench_report", identity, raw.get("measured_utc"))
           and ep._BENCH_SRC_NAME_RE["bench_report"].match(names[0]) is not None, (names, identity))
        ck("L2 명명 키가 체크포인트·manifest·이미지 태그에서 파생된다(출처 표시)",
           identity.get("model") == "fx-lite-7b" and identity.get("gpu_key") == "GB10"
           and identity.get("vllm_version") == "9.8.7" and identity.get("vllm_version_source") == "derived(envfile IMAGE_TAG)",
           identity)
        mc = rbs.parse_measurement_config(text) if text else None
        ck("L2 헤더 mode: lite · 측정 구성 표 = 선언된 lite(bench_mode·kind·도구·반복 1·강등 사유 없음)",
           rbs.is_lite_report(text) and isinstance(mc, dict) and mc.get("bench_mode") == "lite"
           and mc.get("bench_mode_kind") == "declared-lite" and mc.get("bench_tool") == "vllm-bench-serve"
           and mc.get("repeats") == 1 and mc.get("downgrade_reason") is None
           and str(mc.get("bench_mode_source", "")).startswith(cc.BENCH_MODE_SOURCE_DECLARED_PREFIX), mc)
        ck("L2 lite 지표 표가 실측값으로 실린다(HINT_MISSING_LITE 술어 참) · hint 파서가 lite 절을 렌더한다",
           rbs.lite_metrics_present(text) and "| gen tokens/sec (warm) | 40.00 t/s |" in text
           and rbs.render_lite(rbs.parse_lite_report(text), names[0] if names else "x").startswith(rbs.LITE_SECTION_TITLE),
           text[:1500])
        ck("L2 publish-lite-report 수용 술어가 참이다(발행기가 표를 파싱해 lite 리포트만 바인딩한다)",
           (ep.report_measurement_config(text)[0] or {}).get("bench_mode") == ep.LITE_BENCH_MODE)
        ck("L2 본문 측정시각 = raw 측정시각(같은 측정 판정 근거 · doc_naming.measurement_ts)",
           dn.measurement_ts(text) == raw.get("measured_utc"), dn.measurement_ts(text))
        ck("L2 스윕 표가 없다(동시성 곡선을 재지 않았다 — 만들어 넣지 않는다)", rbs.REPORT_HEADER not in text)

        # ── L3 멱등 재렌더 ──────────────────────────────────────────────────────────────────────────
        cp3 = sb.render("--lite-only", "--lite-raw-json", str(sb.raw_path()))
        ck("L3 같은 raw 재렌더 → 같은 이름 덮어쓰기(파일 1건 유지)",
           cp3.returncode == 0 and sb.reports() == names and cp3.stdout.strip().endswith(names[0] if names else "?"),
           (cp3.returncode, cp3.stderr, sb.reports()))

        # ── L4 발행 실패는 측정을 잃지 않는다 ─────────────────────────────────────────────────────────
        shutil.rmtree(sb.root / "docs" / "benchmark")
        sb.manifest(False)
        cp4 = sb.lite("--publish-report")
        ck("★L4 manifest gpu_model 부재 + --publish-report → exit 5 · 리포트 0 · raw 는 남는다 · 사유를 말한다",
           cp4.returncode == 5 and sb.reports() == [] and sb.raw_path().is_file() and "gpu_model" in cp4.stderr,
           (cp4.returncode, cp4.stderr[-600:]))
        cp4b = sb.lite()
        ck("★L4 같은 조건의 자동 핸드오프 경로(플래그 없음)는 exit 0 — 발행 실패가 lite 자체를 실패로 만들지 않는다",
           cp4b.returncode == 0 and sb.reports() == [], (cp4b.returncode, cp4b.stderr[-400:]))
        sb.manifest(True)

        # ── L5 측정시각 부재 ────────────────────────────────────────────────────────────────────────
        bad_raw = dict(json.loads(sb.raw_path().read_text(encoding="utf-8")))
        bad_raw.pop("measured_utc", None)
        _write(sb.root / "bad_raw.json", json.dumps(bad_raw))
        cp5 = sb.render("--lite-only", "--lite-raw-json", str(sb.root / "bad_raw.json"))
        ck("★L5 raw 측정시각 부재 → exit 2 · 리포트 0(측정시각을 날조하지 않는다)",
           cp5.returncode == 2 and sb.reports() == [] and "measured_utc" in cp5.stderr, (cp5.returncode, cp5.stderr))

        # ── L6 모드 혼합 · 인자 누락 ──────────────────────────────────────────────────────────────────
        cp6 = sb.render("--lite-only", "--lite-raw-json", str(sb.raw_path()), "--sweep-index", "x.json")
        cp6b = sb.render("--sweep-index", "x.json")
        cp6c = sb.render("--lite-raw-json", str(sb.raw_path()), "--sweep-index", "x.json", "--verdict-json", "v.json")
        ck("★L6 --lite-only 에 스윕 인자 혼합 · full 인자 누락 · full 에 lite raw 혼합 → 전부 exit 2 · 리포트 0",
           cp6.returncode == 2 and cp6b.returncode == 2 and cp6c.returncode == 2 and sb.reports() == [],
           (cp6.returncode, cp6b.returncode, cp6c.returncode))

    # ── L7 full 스윕의 lite 레그는 발행하지 않는다 ────────────────────────────────────────────────────────
    sweep_text = (SDIR / "sweep_bench.sh").read_text(encoding="utf-8")
    lite_calls = [ln for ln in sweep_text.splitlines() if "lite_bench.sh" in ln and ln.lstrip().startswith("if bash")]
    ck("★L7 sweep_bench 의 lite 레그 호출은 1줄이고 --publish-report 를 넘기지 않는다(full 리포트 stem 보호)",
       len(lite_calls) == 1 and "--publish-report" not in lite_calls[0], lite_calls)
    lite_text = (SDIR / "lite_bench.sh").read_text(encoding="utf-8")
    ck("L7 lite_bench 는 플래그가 있을 때만 render_report --lite-only 를 부른다(기본 PUBLISH_REPORT=0)",
       "PUBLISH_REPORT=0" in lite_text and 'if [ "$PUBLISH_REPORT" = "1" ]' in lite_text
       and "--lite-only --lite-raw-json" in lite_text)

    # ── L8 명명 키 교차검증(sweep_bench heredoc 실행 ↔ lite_identity) ──────────────────────────────────
    smeta = _load("_lite_sweep_meta", SDIR / "selftest_sweep_meta.py")
    for label, engine in (("엔진 로그 있음", ("fp8", "fp8")), ("엔진 로그 없음", None)):
        saved = os.environ.pop("EASY_VLLM_VERSION", None)
        try:
            meta = smeta.build_meta(engine=engine)
        finally:
            if saved is not None:
                os.environ["EASY_VLLM_VERSION"] = saved
        with tempfile.TemporaryDirectory(prefix="lite-identity.") as td:
            t = Path(td)
            _write(t / "cfg.yaml", "model: /models/acme/Acme-7B\nmax-model-len: 262144\n")
            _write(t / ".env.x", "IMAGE_TAG=easy-vllm:9.9.9-test\nSERVING_MODEL_NAME=Acme-7B\n")
            _write(t / "manifest.yaml", "gpu_model: NVIDIA GB10\ngpus_per_node: 1\ntopology: single\n")
            _write(t / "engine.log", smeta.ENGINE_LINE.format(quant="fp8", kvdt="fp8") if engine else "")
            ident = rr.lite_identity({"config_name": "x", "config_yaml": str(t / "cfg.yaml"), "env_file": str(t / ".env.x"),
                                      "manifest": str(t / "manifest.yaml"), "engine_log": str(t / "engine.log"),
                                      "topology": "single"}, environ={})
        ck(f"L8 명명 키 교차검증({label}) — sweep_bench 조립 heredoc 과 lite_identity 가 같은 model·gpu_key·vllm_version",
           all(meta.get(k) == ident.get(k) for k in ("model", "model_source", "gpu_key", "vllm_version")),
           ({k: meta.get(k) for k in ("model", "model_source", "gpu_key", "vllm_version")},
            {k: ident.get(k) for k in ("model", "model_source", "gpu_key", "vllm_version")}))

    # ── L9 full 리포트의 측정 구성 표 ───────────────────────────────────────────────────────────────────
    with tempfile.TemporaryDirectory(prefix="lite-full-mc.") as td:
        t = Path(td)
        idx = {"generated_utc": "2026-09-14T01:00:00Z", "verdict_point_level": 1,
               "meta": {"model": "m", "gpu_model": "NVIDIA GB10", "vllm_version": "0.0.1", "bench_tool": "guidellm",
                        "bench_tool_version": "0.7.3", "bench_tool_version_source": "measured(benchmarks.json)"},
               "levels": [{"level": 1, "status": "ok", "measured": {"decode_tps": 40.0, "completed": 4, "failed": 0}}],
               "repetition": {"requested": 3, "requested_source": "campaign(budgets.repeats)", "kind": "warm-rerun",
                              "completed_by_level": {"1": 2}},
               "truncated": []}
        _write(t / "sweep_index.json", json.dumps(idx))
        _write(t / "verdict.json", json.dumps({"verdict": "PASS", "rubric": {}}))

        def full_render(record):
            rec_path = t / cc.BENCH_MODE_RECORD_NAME
            if record is None:
                rec_path.unlink(missing_ok=True)
            else:
                _write(rec_path, json.dumps(dict(record, sweep_index_generated_utc=idx["generated_utc"])))
            cp = subprocess.run([sys.executable, str(SDIR / "render_report.py"), "--sweep-index", str(t / "sweep_index.json"),
                                 "--verdict-json", str(t / "verdict.json"), "--stdout"],
                                capture_output=True, text=True, timeout=60)
            return cp, (rbs.parse_measurement_config(cp.stdout) if cp.returncode == 0 else None)

        cp, mc_down = full_render({"bench_mode": "lite", "bench_mode_source": "runs[](판정점 끊김)",
                                   "downgrade_reason": "run_failed", "downgrade_reason_source": "runs[] run 3",
                                   "downgrade_correlation": "not_scanned"})
        ck("L9 강등 기록 → 표 bench_mode=lite · kind=downgraded-lite · 사유 run_failed · 요청 반복 3 · 판정점 완주 2 · 도구",
           isinstance(mc_down, dict) and mc_down.get("bench_mode") == "lite"
           and mc_down.get("bench_mode_kind") == "downgraded-lite" and mc_down.get("downgrade_reason") == "run_failed"
           and mc_down.get("repeats") == 3 and mc_down.get("repeats_completed") == 2
           and mc_down.get("bench_tool") == "guidellm" and mc_down.get("bench_tool_version") == "0.7.3", (cp.stderr, mc_down))
        ck("L9 강등 셀의 full 모양 리포트는 발행기 표 판독에서 lite · downgraded-lite 이고(같은 lite 통로) 스윕 표 형식 계약은 그대로다",
           (ep.report_measurement_config(cp.stdout)[0] or {}).get("bench_mode") == ep.LITE_BENCH_MODE
           and cc.bench_mode_kind(ep.report_measurement_config(cp.stdout)[0]) == "downgraded-lite"
           and len(rbs.parse_report(cp.stdout)["levels"]) == 1 and not rbs.is_lite_report(cp.stdout))
        cp, mc_full = full_render({"bench_mode": "full", "bench_mode_source": "runs[](판정점 완주 3)",
                                   "downgrade_reason": None, "downgrade_reason_source": None,
                                   "downgrade_correlation": "not_applicable"})
        ck("★L9 음성대조 full 기록 → bench_mode=full · 발행기 표 판독도 lite 가 아니다(full 리포트를 lite 로 바인딩할 수 없다)",
           isinstance(mc_full, dict) and mc_full.get("bench_mode") == "full" and mc_full.get("bench_mode_kind") == "full"
           and (ep.report_measurement_config(cp.stdout)[0] or {}).get("bench_mode") != ep.LITE_BENCH_MODE, mc_full)
        cp, mc_none = full_render(None)
        ck("★L9 판정 기록 부재 → 리포트는 발행 · bench_mode 미확정(N/A)으로 적는다(full·lite 로 접지 않는다)",
           cp.returncode == 0 and isinstance(mc_none, dict) and mc_none.get("bench_mode") is None
           and mc_none.get("bench_mode_kind") == "undetermined"
           and str(mc_none.get("bench_mode_source", "")).startswith("미확정"), (cp.returncode, mc_none))

    # ── L10 두 스킬 사이의 계약 상수 ────────────────────────────────────────────────────────────────────
    ck("L10 측정 구성 표 제목·키가 writer(render_report)와 reader(render_bench_section)에서 같다",
       rr.MEASUREMENT_CONFIG_TITLE == rbs.MEASUREMENT_CONFIG_TITLE
       and tuple(rr.MEASUREMENT_CONFIG_KEYS) == tuple(rbs.MEASUREMENT_CONFIG_KEYS))
    ck("L10 lite 도구 토큰이 sweep_bench 조립부의 도구 기본 토큰과 같은 철자다",
       f'_md.get("bench_tool") or "{rr.LITE_BENCH_TOOL}"' in sweep_text)
    ck("L10 publish-lite-report 가 받는 bench_mode 토큰 = classify_cell 어휘의 lite",
       ep.LITE_BENCH_MODE == cc.BENCH_MODE_LITE)

    if failures:
        print("[selftest_lite_report] FAIL %d 건: %s" % (len(failures), failures), file=sys.stderr)
        return 1
    print("[selftest_lite_report] PASS — L1~L10(경량 리포트 발행 · 측정 구성 표 · 음성대조 · 명명 교차검증)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
