#!/bin/bash
# 60-qwen4exp-nvfp4-mixed.sh — **소스 이식 패치**(build_patches_src 슬롯 · pre-compile)
#
# what : stock vLLM(0.29.0rc7.dev0+74c96922e)의 qwen4_exp 에서 **ModelOpt MIXED_PRECISION
#        (NVFP4) 체크포인트**가 로드되게 한다. 체크포인트 선언(실측, config.json
#        quantized_layers 50건): experts 48층=NVFP4 gs16 · PLE ngram_embedding=FP8 ·
#        MTP experts=FP8_PB_WO gs128. stock 은 이 셋 중 **두 자리**에서 죽는다:
#          · PLE  — `_get_ple_embedding_quant_method` 가 native Fp8Config 만 받아 mixed
#            config(ModelOptMixedPrecisionConfig)를 떨굼 → PLE 가 unquantized 로 지어져
#            F8_E4M3 shard 128개가 실릴 곳이 없다(커뮤니티 동일 장애 보고).
#          · MTP  — (B1) mixed 의 RoutedExperts 디스패치에 FP8_PB_WO 라우트가 없어
#            `weight_scale_inv`(2D 블록 스케일)를 받을 파라미터가 등록되지 않고,
#            (B2) draft 모델은 standalone 이라 레이어 인덱스가 mtp_start_layer_idx 만큼
#            오프셋인데 `quantized_layers` 에만 리맵이 적용되지 않아 전부 lookup 미스.
#
# why  : camp-26090918(qwen38fn-multi)의 variant=nvfp4 셀 전제. 사다리의 **자체 이식** 칸 —
#        포크/변종이미지 핀 없이 stock 소스트리에 얹는다(사용자 결정 2026-09-09: +1/+1 을
#        변종 태그 없이 자체 수행, 불가 확인 시에만 변종이미지 트랙).
#
# reference : github.com/dolf3131/qwen3.8-flash-next-dgx-spark `scripts/patch-nv-mixed.py`
#        (hunk A 자체 제작 + hunk B1/B2 = 미머지 upstream PR #55513 의 이식, 5파일 중 생산코드
#        3파일). 우리 베이스로의 어댑테이션: 모델 디렉터리 `qwen3_8_flash_next`→`qwen4_exp`,
#        클래스 `Qwen3_8FlashNextPLEFp8EmbeddingMethod`→`Qwen4ExpPLEFp8EmbeddingMethod`.
#        세 앵커는 우리 베이스에서 count==1 실측(2026-09-09, 컨테이너 난독).
#
# gate : 없음(ungated) — **불활성 백포트**이기 때문이다. 세 변경은
#        ModelOptMixedPrecisionConfig 로드 경로에서만 살아나며, 현재 그 경로는 *실패*이지
#        *다른 동작*이 아니다. FP8(native) 체크포인트·타 아치의 동작은 바이트 단위로 동일.
#        대신 **머지 트립와이어**를 둔다: 상류가 #55513/동등 수정을 머지한 뒤의 빌드는 이
#        스크립트가 fail-loud 로 죽여 패치 제거를 강제한다(닫힌 목록 리뷰 강제 — workflow.md
#        §4종 판정표의 하드코딩=트립와이어).
#
# probe: 최종 중재는 serve 스모크다. 빌드타임 검증(앵커 유일성·compileall·마커 grep)은
#        빌드 횟수를 줄일 뿐 가능성을 판정하지 못한다(testlog_26080223 §3).
#
# plan : plan_26090918 §Phase 0 P0-1.
set -euo pipefail

TAG="[60-qwen4exp-nvfp4-mixed]"
DST="/workspace/vllm-src"

[ -d "$DST/vllm/models/qwen4_exp" ] || { echo "$TAG FAIL: $DST 에 qwen4_exp 모델 트리 부재 — 베이스가 아니다" >&2; exit 1; }
echo "$TAG base HEAD: $(git -C "$DST" rev-parse HEAD 2>/dev/null || echo unknown)"

python3 - "$DST" <<'PY'
import sys

DST = sys.argv[1]
TAG = "[60-qwen4exp-nvfp4-mixed]"

def sub(text, old, new, what):
    n = text.count(old)
    if n != 1:
        print(f"{TAG} FAILED — anchor {what!r} matched {n} times", file=sys.stderr)
        raise SystemExit(1)
    return text.replace(old, new)

def tripwire(path, needle, what):
    if needle in open(path).read():
        print(f"{TAG} FAILED — {what}: 상류가 이미 동등 수정을 머지한 것으로 보인다. "
              f"이 패치(60-qwen4exp-nvfp4-mixed)를 제거하고 stock 경로로 스모크하라.",
              file=sys.stderr)
        raise SystemExit(1)

# ── A. ple_layer.py — MIXED_PRECISION 도착 시 PLE FP8 라우트 ─────────────────
ple = f"{DST}/vllm/models/qwen4_exp/nvidia/ple_layer.py"
tripwire(ple, "ModelOptMixedPrecisionConfig", "A: PLE mixed 라우트 이미 존재")
s = open(ple).read()
s = sub(
    s,
    '''    """Select global-scale FP8 only for quantized PLE checkpoint shards."""

    if not isinstance(quant_config, Fp8Config):
        return None
''',
    '''    """Select global-scale FP8 only for quantized PLE checkpoint shards."""

    # [port:60-nvfp4-mixed] ModelOpt MIXED_PRECISION 체크포인트(예 NVFP4 빌드:
    # experts=NVFP4 · PLE=FP8 · MTP=FP8_PB_WO)는 ModelOptMixedPrecisionConfig 로
    # 도착하므로 아래 native Fp8Config 게이트를 통과하지 못한다. mixed config 은
    # 이미 이 prefix 를 quantized_layers 로 "FP8" 에 해소하므로 그 판정을 받는다.
    # 참조: dolf3131 patch-nv-mixed.py hunk A.
    try:
        from vllm.model_executor.layers.quantization.modelopt import (
            ModelOptMixedPrecisionConfig,
        )
    except ImportError:  # modelopt 미탑재 빌드
        ModelOptMixedPrecisionConfig = ()
    if ModelOptMixedPrecisionConfig and isinstance(
        quant_config, ModelOptMixedPrecisionConfig
    ):
        if quant_config.is_layer_excluded(prefix):
            return None
        if quant_config._resolve_quant_algo(prefix) != "FP8":
            return None
        return Qwen4ExpPLEFp8EmbeddingMethod()

    if not isinstance(quant_config, Fp8Config):
        return None
''',
    "A: PLE mixed-precision FP8 route",
)
open(ple, "w").write(s)
print(f"{TAG} A  patched {ple}")

# ── B1. modelopt.py — block-FP8(FP8_PB_WO) MoE 디스패치 (PR #55513) ──────────
mo = f"{DST}/vllm/model_executor/layers/quantization/modelopt.py"
tripwire(mo, "FP8_BLOCK_SCALES", "B1: block-FP8 MoE 라우트 이미 존재")
s = open(mo).read()
s = sub(
    s,
    """        if isinstance(layer, RoutedExperts):
            if quant_algo == "FP8":
                return ModelOptFp8MoEMethod(""",
    """        if isinstance(layer, RoutedExperts):
            # [port:60-nvfp4-mixed] PR #55513: block-scaled FP8 experts. ModelOpt 의
            # 정식 명칭은 FP8_PB_WO(초기 composed 체크포인트는 FP8_BLOCK_SCALES).
            # 아래 ModelOptFp8MoEMethod 는 per-tensor 스케일만 등록해
            # `weight_scale_inv`(2D 블록) 를 받을 수 없다. native Fp8MoEMethod 는
            # weight_block_size 설정 시 weight_scale_name="weight_scale_inv" — 정확히
            # 이 레이아웃이다.
            if quant_algo in ("FP8_PB_WO", "FP8_BLOCK_SCALES"):
                from vllm.model_executor.layers.quantization.fp8 import (
                    Fp8Config as _NativeFp8Config,
                )
                from vllm.model_executor.layers.quantization.fp8 import (
                    Fp8MoEMethod as _NativeFp8MoEMethod,
                )

                _sizes = {
                    int(info.get("group_size", 128))
                    for info in self.quantized_layers.values()
                    if info.get("quant_algo", "").upper()
                    in ("FP8_PB_WO", "FP8_BLOCK_SCALES")
                }
                if len(_sizes) > 1:
                    raise ValueError(
                        "MIXED_PRECISION currently requires all block-FP8 MoE "
                        f"layers to use one group_size, got {sorted(_sizes)}."
                    )
                _bs = next(iter(_sizes), 128)
                return _NativeFp8MoEMethod(
                    _NativeFp8Config(
                        is_checkpoint_fp8_serialized=True,
                        activation_scheme="dynamic",
                        weight_block_size=[_bs, _bs],
                    ),
                    layer,
                )
            if quant_algo == "FP8":
                return ModelOptFp8MoEMethod(""",
    "B1: block-FP8 MoE dispatch",
)
open(mo, "w").write(s)
print(f"{TAG} B1 patched {mo}")

# ── B2. mtp.py — draft 의 quantized_layers 리맵 (PR #55513) ──────────────────
mtp = f"{DST}/vllm/models/qwen4_exp/nvidia/mtp.py"
tripwire(mtp, '"quantized_layers",', "B2: quantized_layers 리맵 이미 존재")
s = open(mtp).read()
s = sub(
    s,
    """        exclude_modules = getattr(draft_quant_config, "exclude_modules", None)
        if exclude_modules:""",
    """        # [port:60-nvfp4-mixed] PR #55513: quantized_layers 는 CHECKPOINT 인덱스
        # (mtp.layers.0) 로 키잉되는데 draft 모델은 standalone 이라 레이어가
        # mtp_start_layer_idx 만큼 오프셋이다. ignored_layers/exclude_modules 에는
        # 이미 같은 리맵이 있으나 이것만 빠져 있어 MTP 엔트리가 전부 lookup 미스.
        quantized_layers = getattr(draft_quant_config, "quantized_layers", None)
        if quantized_layers:
            setattr(  # noqa: B010
                draft_quant_config,
                "quantized_layers",
                {
                    _remap_ignored_layers([name], mtp_start_layer_idx)[0]: info
                    for name, info in quantized_layers.items()
                },
            )
        exclude_modules = getattr(draft_quant_config, "exclude_modules", None)
        if exclude_modules:""",
    "B2: remap quantized_layers",
)
open(mtp, "w").write(s)
print(f"{TAG} B2 patched {mtp}")
print(f"{TAG} apply OK")
PY

# ── 빌드타임 검증(fail-loud): 구문 + 마커 존재. import 는 torch/CUDA 가 필요해 빌드에서 못 한다 ──
python3 -m compileall -q \
    "$DST/vllm/models/qwen4_exp/nvidia/ple_layer.py" \
    "$DST/vllm/model_executor/layers/quantization/modelopt.py" \
    "$DST/vllm/models/qwen4_exp/nvidia/mtp.py" \
    || { echo "$TAG FAIL: compileall" >&2; exit 1; }
grep -q 'port:60-nvfp4-mixed' "$DST/vllm/models/qwen4_exp/nvidia/ple_layer.py" \
  && grep -q 'port:60-nvfp4-mixed' "$DST/vllm/model_executor/layers/quantization/modelopt.py" \
  && grep -q 'port:60-nvfp4-mixed' "$DST/vllm/models/qwen4_exp/nvidia/mtp.py" \
  || { echo "$TAG FAIL: 적용 마커 부재" >&2; exit 1; }
echo "$TAG OK — 3파일 이식·검증 통과(최종 중재 = serve 스모크)"
