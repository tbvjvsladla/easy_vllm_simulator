#!/usr/bin/env python3
"""selftest_sweep_meta.py — `sweep_bench.sh` meta 추출의 결정론 회귀시험.

근거: `docs/plan/plan_26082322_…_quantization_kv_cache_dtype_실측누락_수정.md` §3.3.
2026-08-23 R7(1M·fp8 KV·fp8 가중치) 리포트가 `quantization: N/A`·`kv_cache_dtype: N/A` 로
발행됐다 — 두 값을 **config yaml 에서만** 읽었기 때문이고, 축 F/H 는 serve-plane CLI 로 들어와서
yaml 에 흔적이 없다. 엔진은 그 순간 `quantization=fp8, kv_cache_dtype=fp8` 을 자기보고 중이었다.

**이 시험은 파서를 복제하지 않는다.** `sweep_bench.sh` 안의 python heredoc 을 그대로 뽑아
실행한다 — 로직을 여기에 다시 적으면 두 벌이 되어 갈라지고, 그게 바로 이 프로젝트가 D4↔D6 파서
사건으로 이미 겪은 실패다. 따라서 이 파일이 검사하는 것은 **배포되는 그 코드**다.

2026-09-14(plan_26091407 §4.1 · 항목 1): spec 선언 지문(`spec_declared`·`_k`·`_source`)을 같은 방식으로
친다. 판정기는 accept_len 결손을 이 선언과 대조해서만 가르므로(SPEC_ACCEPT_LEN_MISSING), 주석 줄을
선언으로 읽거나 config yaml 부재를 `off` 로 접으면 결손이 "정상 1.0" 으로 위장한다.

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
               engine_log_text=None, cfg_extra=None, sh_text=None, no_cfg=False, cfg_text=None,
               prior_meta=None):
    """픽스처 한 벌을 깔고 **실제 heredoc** 을 실행해 meta 를 돌려준다.

    yaml_quant/yaml_kvdt : config yaml 선언(None 이면 줄 자체가 없음 = 축 F/H 경로)
    engine               : (quantization, kv_cache_dtype) 엔진 자기보고. None 이면 엔진 로그 부재
    engine_log_text      : 통짜 로그를 직접 주입(실 로그 스냅샷 회귀용)
    cfg_extra            : config yaml 에 덧붙일 원문(spec 선언 지문 케이스)
    sh_text              : 러너 `<cfg>.sh` 원문(None 이면 파일 없음)
    no_cfg               : config yaml 자체를 만들지 않는다(부재 ≠ off 음성대조)
    cfg_text             : config yaml 원문 통째 주입(실 config 스냅샷 회귀용)
    prior_meta           : 주면 `--reassemble-only` 경로 — 기존 sweep_index.json(meta=이 값)을 깔고 REASSEMBLE=1
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
        if cfg_extra is not None:
            cfg_lines.append(cfg_extra)
        if cfg_text is not None:
            (root / "cfg.yaml").write_text(cfg_text, encoding="utf-8")
        elif not no_cfg:
            (root / "cfg.yaml").write_text("\n".join(cfg_lines) + "\n", encoding="utf-8")
        if sh_text is not None:
            # 러너는 config yaml 과 같은 디렉터리의 `<cfg>.sh` 다(heredoc 의 _shtext 규약).
            (root / "x.sh").write_text(sh_text, encoding="utf-8")
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
        if prior_meta is not None:
            (sweepdir / "sweep_index.json").write_text(json.dumps(
                {"generated_utc": "2026-01-01T00:00:00Z", "meta": prior_meta}), encoding="utf-8")

        env = {
            "CONFIG": "x", "TOPO": "single", "CFGYAML": str(root / "cfg.yaml"),
            "EF": str(root / ".env.x"), "MANIFEST": str(root / "manifest.yaml"),
            "AGENT_CARD": str(root / "nope.json"), "SWEEPDIR": str(sweepdir),
            "VLLM_VER": "", "COMPLETED": "1", "ILEN": "1024",
            "IMAGE_TAG_ACTUAL": "NA", "REASSEMBLE": "1" if prior_meta is not None else "0",
            "LITE_RAW": "", "SDIR": str(SDIR),
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

# ── spec 선언 지문 케이스 (2026-09-14 · plan_26091407 §4.1) ─────────────────────────────────
_MTP3 = """speculative-config: '{"method":"mtp","num_speculative_tokens":3}'   # 주석: MTP k=3"""
SPEC_CASES = [
    ("S1 JSON 선언(+행말 주석) → on · k=3", dict(cfg_extra=_MTP3),
     {"spec_declared": "on", "spec_declared_k": 3, "spec_declared_source": "declared(config yaml speculative-config)"}),
    ("S2 ★함정 주석 줄(#speculative-config: MTP OFF)은 선언이 아니다 → off",
     dict(cfg_extra="#speculative-config: MTP OFF (결정론 분리 시험)"),
     {"spec_declared": "off", "spec_declared_k": None}),
    ("S3 줄 부재 → off", dict(), {"spec_declared": "off", "spec_declared_k": None}),
    ("S4 블록 매핑 형태 → on · k=7",
     dict(cfg_extra="speculative-config:\n  method: dspark\n  num_speculative_tokens: 7\nenforce-eager: true"),
     {"spec_declared": "on", "spec_declared_k": 7}),
    ("S5 명시 null → off(선언된 부재)", dict(cfg_extra="speculative-config: null"),
     {"spec_declared": "off"}),
    ("S6 ★음성대조 config yaml 부재 → unknown(off 로 접지 않는다)", dict(no_cfg=True),
     {"spec_declared": "unknown", "spec_declared_k": None}),
    ("S7 러너 sh 가 인자로 넣는 선언 → on",
     dict(sh_text="#!/bin/bash\n# --speculative-config 주석은 무시\n"
                  "EXTRA_ARGS+=(--speculative-config '{\"method\":\"mtp\",\"num_speculative_tokens\":2}')\n"),
     {"spec_declared": "on", "spec_declared_k": 2, "spec_declared_source": "declared(runner sh --speculative-config)"}),
    ("S8 러너 sh 의 주석 줄만 있으면 off", dict(sh_text="# --speculative-config '{}'\n"),
     {"spec_declared": "off"}),
    # 재조립(--reassemble-only)은 측정 시점의 선언을 승계한다 — 측정 뒤에 편집된 yaml 을 다시 읽지 않는다.
    ("S9 ★재조립 · 기존 index 에 지문 on 이 있으면 승계(현재 yaml 에서 spec 줄이 사라져도 off 로 바꾸지 않는다)",
     dict(cfg_extra="#speculative-config: 측정 뒤 주석 처리",
          prior_meta={"spec_declared": "on", "spec_declared_k": 3,
                      "spec_declared_source": "declared(config yaml speculative-config)"}),
     {"spec_declared": "on", "spec_declared_k": 3,
      "spec_declared_source": "declared(config yaml speculative-config)"}),
    ("S10 재조립 · 기존 index 에 지문이 없으면 현재 파일로 계산하되 retro 표지",
     dict(cfg_extra=_MTP3, prior_meta={"model": "x"}),
     {"spec_declared": "on", "spec_declared_k": 3,
      "spec_declared_source": "retro(reassemble · 현재 config 기준 · 측정 시점 보장 없음) "
                              "declared(config yaml speculative-config)"}),
]

# 실 config 스냅샷(있으면) — 캠페인 셀 yaml 의 실제 두 형태(JSON 선언 · 주석 처리된 OFF)를 친다.
REAL_CFG = [
    ("RC1 승자 셀 MTP k=3", "output/multi/configs/nv4-bf-262k-res-kv8g-gmu80.yaml",
     {"spec_declared": "on", "spec_declared_k": 3}),
    ("RC2 dspark k=7(행말 긴 주석)", "output/multi/configs/b-768k-kvfp8-l1spec.yaml",
     {"spec_declared": "on", "spec_declared_k": 7}),
    ("RC3 MTP OFF 주석 줄", "output/multi/configs/q38fn-nvfp4.yaml",
     {"spec_declared": "off"}),
]

# ── GuideLLM spec 축 승계원 해석 `_spec_axis_args` (2026-09-14 리뷰 정정 · plan_26091407 §4.1) ──────────
#   뒤집힌 판정 4건의 뿌리(lite raw 포인터 문서를 승계원으로 넘김)를 고친 **셸 호출부**를 바이트 그대로 뽑아
#   `set -euo pipefail` 아래에서 실행한다. 파서 순수함수 시험(parse_guidellm G23~G25)만으로는 호출부가 무엇을
#   넘기는지를 증명하지 못한다.
def extract_spec_axis_fn():
    text = SWEEP.read_text(encoding="utf-8")
    m = re.search(r"# >>> spec-axis-args[^\n]*\n(.*?)# <<< spec-axis-args\n", text, re.S)
    if not m or "_spec_axis_args()" not in m.group(1):
        raise SystemExit("[selftest_sweep_meta] sweep_bench.sh 의 _spec_axis_args 구간을 찾지 못했다 — 중단")
    return m.group(1)


def run_spec_axis(fn_src, root, raw_doc=None, warm_text=None, raw_path=None):
    """(args, truncation_log_text). raw_doc=None ∧ raw_path=None 이면 lite 절삭(빈 인자)."""
    import subprocess
    log = root / "truncation.log"
    log.write_text("", encoding="utf-8")
    warm = root / "lite_warm_x.json"
    if warm_text is not None:
        warm.write_text(warm_text, encoding="utf-8")
    if raw_doc is not None:
        doc = dict(raw_doc)
        if doc.get("bench_warm_json") == "<WARM>":
            doc["bench_warm_json"] = str(warm)
        raw_path = root / "lite_raw_x.json"
        raw_path.write_text(json.dumps(doc), encoding="utf-8")
    argsf = root / "spec_axis_args.txt"    # 함수의 경고 에코(stdout)와 결과 배열을 섞지 않는다
    script = fn_src + '\n_spec_axis_args "$1" "$2" "$3"\nprintf "%s\\n" "${SPEC_AXIS_ARGS[@]}" > "$4"\n'
    cp = subprocess.run(["bash", "-c", "set -euo pipefail\n" + script, "_",
                         str(raw_path) if raw_path is not None else "", str(log), str(SDIR), str(argsf)],
                        capture_output=True, text=True, timeout=60)
    if cp.returncode != 0:
        raise RuntimeError("함수 실행이 set -e 아래에서 죽었다(rc=%d): %s" % (cp.returncode, cp.stderr[-300:]))
    return (argsf.read_text(encoding="utf-8").splitlines(), log.read_text(encoding="utf-8"),
            str(warm), str(raw_path))


_WARM_SPEC = json.dumps({"completed": 3, "failed": 0, "median_tpot_ms": 21.4,
                         "spec_decode_acceptance_length": 2.2})
_WARM_NOSPEC = json.dumps({"completed": 3, "failed": 0, "median_tpot_ms": 30.0})
_POINTER = {"topology": "multi", "burst_n": 3, "bench_warm_json": "<WARM>", "nodes": []}
AXIS_CASES = [
    # (이름, kwargs, 기대 모드 'warm'|'absent', 절삭 로그에 있어야 할 문구(None=로그 비어 있어야 함))
    ("W1 ★포인터 → warm JSON 경로를 넘긴다(포인터 문서 자체 ✗)", dict(raw_doc=_POINTER, warm_text=_WARM_SPEC),
     "warm", None),
    ("W2 spec off warm(수용길이 키 없음 · completed 있음)도 승계원이다", dict(raw_doc=_POINTER, warm_text=_WARM_NOSPEC),
     "warm", None),
    ("W3 ★잘린 warm JSON → 부재 명시로 강등 · 사유 기재(스윕을 죽이지 않는다)",
     dict(raw_doc=_POINTER, warm_text='{"completed":3,"spec_decode_acceptance_len'), "absent", "JSONDecodeError"),
    ("W4 ★bench 결과 모양이 아닌 warm → 강등 · 파서 술어의 사유 기재",
     dict(raw_doc=_POINTER, warm_text='{"error":"x"}'), "absent", "vllm bench serve"),
    ("W5 포인터에 bench_warm_json 없음 → 강등 · 사유 기재", dict(raw_doc={"topology": "multi", "nodes": []}),
     "absent", "bench_warm_json 이 없다"),
    ("W6 포인터가 가리키는 warm 부재 → 강등 · 사유 기재", dict(raw_doc=_POINTER), "absent", "부재/빈 파일"),
    ("W7 lite 절삭(빈 인자) → 부재 명시 · 로그 추가 없음(이미 기재됨)", dict(), "absent", None),
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

    for name, kwargs, expect in SPEC_CASES:
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
            print("  ..   %s → %s/%s" % (name, meta.get("spec_declared"), meta.get("spec_declared_k")))

    fn_src = extract_spec_axis_fn()
    for name, kwargs, mode, log_phrase in AXIS_CASES:
        with tempfile.TemporaryDirectory() as td:
            try:
                args, log, warm, raw = run_spec_axis(fn_src, pathlib.Path(td), **kwargs)
            except Exception as exc:                                # noqa: BLE001 — 시험 러너
                failures.append("%s: 실행 예외 %s: %s" % (name, type(exc).__name__, exc))
                continue
            if mode == "warm":
                ok = args == ["--accept-len-src", warm] and raw not in args
            else:
                ok = args == ["--spec-axis-absent"]
            if not ok:
                failures.append("%s: SPEC_AXIS_ARGS=%r (기대 %s)" % (name, args, mode))
            if log_phrase is None and log.strip():
                failures.append("%s: 절삭 로그가 비어 있어야 한다: %r" % (name, log[-200:]))
            if log_phrase is not None and log_phrase not in log:
                failures.append("%s: 절삭 로그에 %r 없음: %r" % (name, log_phrase, log[-300:]))
            if verbose:
                print("  ..   %s → %s" % (name, args))
    # 음성대조: W1 이 넘긴 경로가 아니라 **포인터 문서**를 파서가 받으면 거부된다(호출부 교정이 필요했던 이유).
    sys.path.insert(0, str(SDIR))
    from parse_guidellm import inherit_accept_len          # noqa: E402 — 배포되는 술어 그대로
    try:
        inherit_accept_len(dict(_POINTER, bench_warm_json="lite_warm_x.json"), "lite_raw_x.json")
        failures.append("W1 음성대조: 포인터 문서가 승계원으로 수용됐다(조용한 None 승계)")
    except ValueError:
        pass

    repo = SDIR.parents[3]
    for name, rel, expect in REAL_CFG:
        p = repo / rel
        if not p.is_file():
            print("[selftest_sweep_meta] SKIP 실 config 부재 — %s" % rel)
            continue
        try:
            meta = build_meta(cfg_text=p.read_text(encoding="utf-8", errors="replace"))
        except Exception as exc:                                    # noqa: BLE001
            failures.append("실 config %s: 실행 예외 %s: %s" % (name, type(exc).__name__, exc))
            continue
        for key, want in expect.items():
            got = meta.get(key)
            if got != want:
                failures.append("실 config %s: meta[%r] = %r (기대 %r)" % (name, key, got, want))
            elif verbose:
                print("  ok   실 config %s · %s=%s" % (name, key, got))

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
    print("[selftest_sweep_meta] OK — C1~C10 + S1~S10(spec 선언 지문·재조립 승계) + W1~W7(spec 축 승계원 호출부) "
          "+ 실 config·실로그 회귀 전부 통과 (측정>선언 · 기본값≠미상 · 함정 3종 차단 · 주석≠선언 · 부재≠off)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
