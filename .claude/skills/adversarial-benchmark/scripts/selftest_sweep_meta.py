#!/usr/bin/env python3
"""selftest_sweep_meta.py — `sweep_bench.sh` meta 추출의 결정론 회귀시험.

근거: `docs/plan/plan_26082322_…_quantization_kv_cache_dtype_실측누락_수정.md` §3.3.
2026-08-23 R7(1M·fp8 KV·fp8 가중치) 리포트가 `quantization: N/A`·`kv_cache_dtype: N/A` 로
발행됐다 — 두 값을 **config yaml 에서만** 읽었기 때문이고, 축 F/H 는 serve-plane CLI 로 들어와서
yaml 에 흔적이 없다. 엔진은 그 순간 `quantization=fp8, kv_cache_dtype=fp8` 을 자기보고 중이었다.

**이 시험은 파서를 복제하지 않는다.** `sweep_bench.sh` 안의 python heredoc 을 그대로 뽑아
실행한다 — 로직을 여기에 다시 적으면 두 벌이 되어 갈라지고, 그게 바로 이 프로젝트가 D4↔D6 파서
사건으로 이미 겪은 실패다. 따라서 이 파일이 검사하는 것은 **배포되는 그 코드**다.

라이브 하드웨어·serve·docker 불요(정적 픽스처 + 실 로그 스냅샷).
사용: python3 selftest_sweep_meta.py [-v]
종료: 0=PASS · 1=FAIL
"""
import json
import os
import pathlib
import re
import sys
import tempfile

SDIR = pathlib.Path(__file__).resolve().parent
SWEEP = SDIR / "sweep_bench.sh"

# 실제 캠페인 로그의 엔진 config 라인(2026-08-23 R7, 발췌·축약). 함정이 두 개 심겨 있다:
#   ① `quantization_config=None` 이 `quantization=fp8` 바로 뒤에 온다 → 경계 없는 검색은 헷갈린다
#   ② 같은 로그 다른 줄에 러너 에코와 FlashInfer 해석값이 있다 → 전역 검색은 `torch` 를 집는다
ENGINE_LINE = (
    "INFO 08-23 12:20:10 [core.py:99] Initializing a V1 LLM engine (v0.27.1.dev0+g4bdc8a788."
    "d20260814) with config: model='/app/models/Qwen/Qwen3.8/Qwen3.8-27B', "
    "speculative_config=SpeculativeConfig(method='mtp', num_spec_tokens=3), "
    "dtype=torch.bfloat16, max_seq_len=1000000, tensor_parallel_size=1, "
    "quantization={quant}, quantization_config=None, enforce_eager=False, "
    "kv_cache_dtype={kvdt}, device_config=cuda, seed=None\n"
)
DECOY_LINES = (
    "[runner] KV_CACHE_DTYPE=DECOY_A\n"
    "[runner] QUANTIZATION=DECOY_B\n"
    "INFO 08-23 12:20:30 [flashinfer.py:1] FlashInfer resolved backend, "
    "kv_cache_dtype=torch.float8_e4m3fn, arch=sm121\n"
)


def extract_embedded_python():
    """`sweep_bench.sh` 의 조립 heredoc 을 뽑는다. 파일이 없거나 heredoc 이 사라졌으면 조용히
    통과시키지 않고 즉시 실패한다 — 검사 대상이 사라진 것을 PASS 로 읽으면 시험이 무의미해진다."""
    text = SWEEP.read_text(encoding="utf-8")
    m = re.search(r"python3 - <<'PY'\n(.*?)\nPY\n", text, re.S)
    if not m:
        raise SystemExit("[selftest_sweep_meta] sweep_bench.sh 의 python heredoc 을 찾지 못했다 — 중단")
    return m.group(1)


SRC = extract_embedded_python()


def build_meta(*, yaml_quant=None, yaml_kvdt=None, engine=None, with_decoys=True,
               engine_log_text=None):
    """픽스처 한 벌을 깔고 **실제 heredoc** 을 실행해 meta 를 돌려준다.

    yaml_quant/yaml_kvdt : config yaml 선언(None 이면 줄 자체가 없음 = 축 F/H 경로)
    engine               : (quantization, kv_cache_dtype) 엔진 자기보고. None 이면 엔진 로그 부재
    engine_log_text      : 통짜 로그를 직접 주입(실 로그 스냅샷 회귀용)
    """
    with tempfile.TemporaryDirectory() as td:
        root = pathlib.Path(td)
        sweepdir = root / "sweep_x"
        (sweepdir / "level_01").mkdir(parents=True)

        cfg_lines = ["model: /models/acme/Acme-7B", "max-model-len: 262144",
                     "gpu-memory-utilization: 0.9",
                     "# 레시피 주석: quant=DECOY_C kv-cache-dtype: DECOY_D"]
        if yaml_quant is not None:
            cfg_lines.append("quantization: %s" % yaml_quant)
        if yaml_kvdt is not None:
            cfg_lines.append("kv-cache-dtype: %s" % yaml_kvdt)
        (root / "cfg.yaml").write_text("\n".join(cfg_lines) + "\n", encoding="utf-8")
        (root / ".env.x").write_text("IMAGE_TAG=easy-vllm:9.9.9-test\nSERVING_MODEL_NAME=Acme-7B\n"
                                     "QUANTIZATION=DECOY_E\nKV_CACHE_DTYPE=DECOY_F\n", encoding="utf-8")
        (root / "manifest.yaml").write_text("gpu_model: NVIDIA GB10\ngpus_per_node: 1\n"
                                            "topology: single\ndriver_version: 1.2.3\n"
                                            "cuda_version: 132\n", encoding="utf-8")

        if engine_log_text is not None:
            log = engine_log_text
        elif engine is not None:
            log = ENGINE_LINE.format(quant=engine[0], kvdt=engine[1])
            if with_decoys:
                log = DECOY_LINES + log + DECOY_LINES
        else:
            log = DECOY_LINES if with_decoys else ""
        (sweepdir / "level_01" / "engine_x.log").write_text(log, encoding="utf-8")
        (sweepdir / "level_01" / "measured.json").write_text(
            json.dumps({"measurement_ok": True}), encoding="utf-8")
        (sweepdir / "truncation.log").write_text("", encoding="utf-8")

        env = {
            "CONFIG": "x", "TOPO": "single", "CFGYAML": str(root / "cfg.yaml"),
            "EF": str(root / ".env.x"), "MANIFEST": str(root / "manifest.yaml"),
            "AGENT_CARD": str(root / "nope.json"), "SWEEPDIR": str(sweepdir),
            "VLLM_VER": "", "COMPLETED": "1", "ILEN": "1024",
            "IMAGE_TAG_ACTUAL": "NA", "REASSEMBLE": "0", "LITE_RAW": "", "SDIR": str(SDIR),
        }
        saved = {k: os.environ.get(k) for k in env}
        os.environ.update(env)
        try:
            exec(compile(SRC, str(SWEEP) + "::heredoc", "exec"), {"__name__": "__sweep_meta__"})
        finally:
            for k, v in saved.items():
                if v is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = v
        return json.loads((sweepdir / "sweep_index.json").read_text(encoding="utf-8"))["meta"]


# ── 케이스 표 (plan §3.3: yaml-only · argv-only · 둘 다 · 양쪽 부재 · 불일치) ──────────────────
# 기대값은 4-필드 계약: 값 · _source · _declared · _mismatch.
CASES = [
    # (이름, kwargs, {필드: 기대값})
    ("C1 yaml-only — 선언만 있으면 선언을 싣고 출처를 밝힌다", dict(yaml_quant="fp8", yaml_kvdt="fp8"), {
        "quantization": "fp8", "quantization_source": "declared(config yaml)",
        "quantization_declared": "fp8", "quantization_mismatch": "unknown",
        "kv_cache_dtype": "fp8", "kv_cache_dtype_source": "declared(config yaml)",
        "kv_cache_dtype_declared": "fp8", "kv_cache_dtype_mismatch": "unknown"}),

    ("C2 argv-only — ★R7 회귀: yaml 무언·엔진 fp8 → NA 가 아니라 fp8", dict(engine=("fp8", "fp8")), {
        "quantization": "fp8", "quantization_source": "measured(engine log)",
        "quantization_declared": "NA", "quantization_mismatch": "unknown",
        "kv_cache_dtype": "fp8", "kv_cache_dtype_source": "measured(engine log)",
        "kv_cache_dtype_declared": "NA", "kv_cache_dtype_mismatch": "unknown"}),

    ("C3 둘 다 — 측정이 이기고 선언은 병기, 일치하면 mismatch=no",
     dict(yaml_quant="fp8", yaml_kvdt="fp8", engine=("fp8", "fp8")), {
        "quantization": "fp8", "quantization_source": "measured(engine log)",
        "quantization_declared": "fp8", "quantization_mismatch": "no",
        "kv_cache_dtype": "fp8", "kv_cache_dtype_source": "measured(engine log)",
        "kv_cache_dtype_declared": "fp8", "kv_cache_dtype_mismatch": "no"}),

    ("C4 양쪽 부재 — 여기서만 NA 다", dict(), {
        "quantization": "NA", "quantization_source": "absent(both)",
        "quantization_declared": "NA", "quantization_mismatch": "unknown",
        "kv_cache_dtype": "NA", "kv_cache_dtype_source": "absent(both)",
        "kv_cache_dtype_declared": "NA", "kv_cache_dtype_mismatch": "unknown"}),

    ("C5 불일치 — 선언 fp8·실측 none/auto → 시끄럽게 YES",
     dict(yaml_quant="fp8", yaml_kvdt="fp8", engine=("None", "auto")), {
        "quantization": "none", "quantization_source": "measured(engine log)",
        "quantization_declared": "fp8", "quantization_mismatch": "YES(measured=none declared=fp8)",
        "kv_cache_dtype": "auto", "kv_cache_dtype_source": "measured(engine log)",
        "kv_cache_dtype_declared": "fp8", "kv_cache_dtype_mismatch": "YES(measured=auto declared=fp8)"}),

    # ── 음성정직: "기본값"과 "측정 못 함"은 다른 사실이다 ─────────────────────────────────
    ("C6 음성정직 — 엔진이 None 을 보고하면 그것은 측정된 기본값(none) 이지 NA 가 아니다",
     dict(engine=("None", "auto")), {
        "quantization": "none", "quantization_source": "measured(engine log)",
        "kv_cache_dtype": "auto", "kv_cache_dtype_source": "measured(engine log)"}),

    # ── 함정 대조: 이 두 건이 실패하면 값은 맞아도 출처가 틀린 것이다 ─────────────────────
    ("C7 함정 — quantization_config=None 이 quantization=fp8 을 가리지 않는다",
     dict(engine=("fp8", "fp8")), {"quantization": "fp8"}),

    ("C8 함정 — 러너 에코·FlashInfer 해석값을 집지 않는다(2026-08-23 러너 6g 실증 결함)",
     dict(engine=("fp8", "fp8"), with_decoys=True),
     {"kv_cache_dtype": "fp8", "quantization": "fp8"}),

    ("C9 함정 — 엔진 라인이 없으면 decoy 만 있어도 measured 로 승격하지 않는다",
     dict(with_decoys=True), {"quantization_source": "absent(both)",
                              "kv_cache_dtype_source": "absent(both)"}),

    ("C10 대소문자 정규화 — FP8 도 fp8 로 눕는다(정확일치 게이트 보호)",
     dict(engine=("FP8", "FP8")), {"quantization": "fp8", "kv_cache_dtype": "fp8"}),
]

# ── 실 로그 회귀(있으면) — 픽스처가 현실을 닮았다는 것을 실제 스냅샷으로 못박는다 ────────────
REAL = [
    ("R6a 1M·fp8 KV", "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r6a_1m_fp8/"
     "benchlog_sweep/level_01/engine_qwen38-27b.log", {"quantization": "none", "kv_cache_dtype": "fp8"}),
    ("R6b 512k·bf16 KV", "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r6b_512k_bf16/"
     "benchlog_sweep/level_01/engine_qwen38-27b.log", {"quantization": "none", "kv_cache_dtype": "auto"}),
    ("R7 1M·fp8 KV·fp8 가중치", "docs/simlog/26082317_qwen38_27b_R6ab_tiers/r7_quant_fp8/"
     "benchlog_sweep/level_01/engine_qwen38-27b.log", {"quantization": "fp8", "kv_cache_dtype": "fp8"}),
]


def main():
    verbose = "-v" in sys.argv
    failures = []

    for name, kwargs, expect in CASES:
        try:
            meta = build_meta(**kwargs)
        except Exception as exc:                                    # noqa: BLE001 — 시험 러너
            failures.append("%s: 실행 예외 %s: %s" % (name, type(exc).__name__, exc))
            continue
        for key, want in expect.items():
            got = meta.get(key)
            if got != want:
                failures.append("%s: meta[%r] = %r (기대 %r)" % (name, key, got, want))
        if verbose:
            print("  %-4s %s" % ("ok" if not failures else "..", name))

    repo = SDIR.parents[3]
    for name, rel, expect in REAL:
        p = repo / rel
        if not p.is_file():
            print("[selftest_sweep_meta] SKIP 실로그 부재 — %s" % rel)
            continue
        try:
            meta = build_meta(engine_log_text=p.read_text(encoding="utf-8", errors="replace"))
        except Exception as exc:                                    # noqa: BLE001
            failures.append("실로그 %s: 실행 예외 %s: %s" % (name, type(exc).__name__, exc))
            continue
        for key, want in expect.items():
            got = meta.get(key)
            if got != want:
                failures.append("실로그 %s: meta[%r] = %r (기대 %r)" % (name, key, got, want))
            elif verbose:
                print("  ok   실로그 %s · %s=%s" % (name, key, got))

    if failures:
        sys.stderr.write("[selftest_sweep_meta] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    print("[selftest_sweep_meta] OK — C1~C10 + 실로그 회귀 전부 통과 "
          "(측정>선언 · 기본값≠미상 · 함정 3종 차단)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
