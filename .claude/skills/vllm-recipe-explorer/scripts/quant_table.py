"""quant_table.py — quant 바이트 테이블 (whichllm 벤더링) + 해소 함수.

이 모듈은 whichllm 패키지를 import 하지 않는다(폐쇄망·결정론 원칙).
대신 whichllm 소스에서 값/공식을 **복사(벤더링)** 하고 출처 주석을 단다.

벤더링 출처:
- QUANT_BYTES_PER_WEIGHT  : seed/whichllm/src/whichllm/data/quantization.py (verbatim)
- VLLM_QUANT_BYTES        : seed/whichllm/src/whichllm/engine/quantization.py
                            (_NON_GGUF_BYTES_PER_WEIGHT) + vLLM 플래그 별칭 확장
- FRAMEWORK_OVERHEAD_BYTES: seed/whichllm/src/whichllm/data/framework.py (verbatim)
- DTYPE_BPW               : CONTRACT 명세(온디스크/serve dtype → bytes-per-weight)
"""

from __future__ import annotations

# ---------------------------------------------------------------------------
# QUANT_BYTES_PER_WEIGHT — GGUF 전체 테이블 (참조용).
# 출처: seed/whichllm/src/whichllm/data/quantization.py 의 QUANT_BYTES_PER_WEIGHT
#       (verbatim 복사 — 키·값을 그대로 옮김).
# ---------------------------------------------------------------------------
QUANT_BYTES_PER_WEIGHT: dict[str, float] = {
    "F32": 4.0,
    "F16": 2.0,
    "BF16": 2.0,
    "Q8_0": 1.0625,
    "Q6_K": 0.8125,
    "Q5_K_M": 0.6875,
    "Q5_K_S": 0.6875,
    "Q5_0": 0.625,
    "Q4_K_M": 0.5625,
    "Q4_K_S": 0.5625,
    "Q4_0": 0.5,
    "Q3_K_M": 0.4375,
    "Q3_K_S": 0.4375,
    "Q3_K_L": 0.4375,
    "Q2_K": 0.3125,
    "IQ4_XS": 0.5,
    "IQ3_XXS": 0.375,
    "IQ2_XXS": 0.25,
    # Sub-2-bit / ternary tiers (extremely lossy)
    "Q1_0": 0.28,
    "Q2_0": 0.28,
    "TQ1_0": 0.21,
    "TQ2_0": 0.28,
    "IQ1_S": 0.21,
    "IQ1_M": 0.22,
    "IQ2_S": 0.275,
    "IQ2_M": 0.30,
    "IQ3_S": 0.40,
    "IQ3_M": 0.42,
    "IQ3_XS": 0.41,
    "IQ4_NL": 0.5,
}

# ---------------------------------------------------------------------------
# VLLM_QUANT_BYTES — vLLM 양자화 플래그 → bytes-per-weight.
# 베이스 출처: seed/whichllm/src/whichllm/engine/quantization.py 의
#              _NON_GGUF_BYTES_PER_WEIGHT (AWQ/GPTQ/BNB_4BIT/INT8/FP8/BF16/FP16).
# 그 값을 vLLM `--quantization` 플래그 별칭으로 소문자 정규화·확장한다.
#   - awq/gptq/bitsandbytes/bnb/int4 = 0.5  (_NON_GGUF: AWQ/GPTQ/BNB_4BIT = 0.5)
#   - fp8/int8                       = 1.0  (_NON_GGUF: FP8/INT8 = 1.0)
#   - bf16/fp16/float16/bfloat16     = 2.0  (_NON_GGUF: BF16/FP16 = 2.0)
#   - mxfp4/nvfp4                    = 0.5  (4-bit 계열 — AWQ/GPTQ와 동급 0.5)
#   - compressed-tensors             = 0.5  (best-effort — 4-bit 압축 가정)
# none/null/'' 키는 dtype 기반 serve_bpw 를 런타임에 주입한다(vllm_quant_bpw 참고).
# ---------------------------------------------------------------------------
VLLM_QUANT_BYTES: dict[str, float] = {
    # 무양자화 — serve_bpw(=native dtype bpw)를 주입해 사용.
    "none": None,
    "null": None,
    "": None,
    # 4-bit 계열
    "awq": 0.5,
    "gptq": 0.5,
    "bitsandbytes": 0.5,
    "bnb": 0.5,
    "int4": 0.5,
    "mxfp4": 0.5,
    "nvfp4": 0.5,
    "compressed-tensors": 0.5,  # best-effort: 4-bit 압축 가정
    # 4-bit Marlin 커널 별칭 (HF quant_method 로 흔함; AWQ/GPTQ와 동급 0.5).
    # 출처: vLLM quantization 플래그(awq_marlin/gptq_marlin) — 4-bit best-effort.
    "awq_marlin": 0.5,
    "gptq_marlin": 0.5,
    "marlin": 0.5,
    "bitblas": 0.5,
    "gptq_bitblas": 0.5,
    # 8-bit 계열
    "fp8": 1.0,
    "int8": 1.0,
    # 8-bit prequant 별칭 (최신 HF 체크포인트에서 흔함; ≈1.0 best-effort).
    # 출처: vLLM/HF quant_method 문자열(modelopt/fbgemm_fp8/quark/experts_int8).
    "modelopt": 1.0,
    "modelopt_fp8": 1.0,
    "fbgemm_fp8": 1.0,
    "quark": 1.0,         # best-effort(quark 는 fp8/int8 다양 — 8-bit 보수 가정)
    "experts_int8": 1.0,
    # 16-bit (무양자화 dtype 명시)
    "bf16": 2.0,
    "fp16": 2.0,
    "float16": 2.0,
    "bfloat16": 2.0,
}

# ---------------------------------------------------------------------------
# OFFLINE_ONLY_QUANT — '사전양자화된 체크포인트'를 요구하는 quant 방식(온라인 양자화 불가).
# vLLM 에서 awq/gptq(및 Marlin 변종)는 런타임 온라인 양자화가 불가능하므로 bf16 native
# 체크포인트에 이 quant 를 지정하면 서빙이 실패한다(폐쇄망에선 더욱). 반대로 fp8·
# bitsandbytes 는 bf16 체크포인트를 로드 시 온라인 양자화할 수 있어 제외한다.
# 결정론 계층(estimate)이 비prequantized 체크포인트에 이 quant 를 만나면 경고를 단다.
# 출처: SKILL.md §2 ② (LLM 후보 생성 시 awq/gptq 경고 — 결정론 계층으로 끌어내림).
# ---------------------------------------------------------------------------
OFFLINE_ONLY_QUANT: frozenset[str] = frozenset({
    "awq", "gptq", "awq_marlin", "gptq_marlin", "marlin",
    "gptq_bitblas", "bitblas",
})


def is_offline_only_quant(quant: "str | None") -> bool:
    """quant 가 사전양자화 체크포인트를 요구하는 오프라인 전용 방식인지 판정."""
    if quant is None:
        return False
    return quant.strip().lower() in OFFLINE_ONLY_QUANT

# ---------------------------------------------------------------------------
# FRAMEWORK_OVERHEAD_BYTES — 프레임워크 상주 오버헤드(~500MB).
# 출처: seed/whichllm/src/whichllm/data/framework.py (verbatim).
# ---------------------------------------------------------------------------
FRAMEWORK_OVERHEAD_BYTES = 500_000_000

# ---------------------------------------------------------------------------
# DTYPE_BPW — 온디스크/serve dtype 문자열 → bytes-per-weight.
# 출처: CONTRACT 명세. (safetensors 헤더 dtype·config torch_dtype 정규화에 사용.)
# 기타 dtype 은 경고 후 2.0 으로 폴백한다(dtype_bpw 참고).
# ---------------------------------------------------------------------------
DTYPE_BPW: dict[str, float] = {
    "float32": 4.0,
    "fp32": 4.0,
    "f32": 4.0,
    "float16": 2.0,
    "fp16": 2.0,
    "f16": 2.0,
    "bfloat16": 2.0,
    "bf16": 2.0,
    "float8_e4m3fn": 1.0,
    "float8": 1.0,
    "fp8": 1.0,
    "f8": 1.0,
}


def vllm_quant_bpw(quant: "str | None", serve_bpw: float) -> float:
    """vLLM quant 플래그 → bytes-per-weight 해소.

    - quant 가 None / 'none' / '' → serve_bpw(=native dtype bpw) 반환.
    - 그 외 → VLLM_QUANT_BYTES[quant.lower()].
      · 매핑값이 None(none/null/'') 인 경우도 serve_bpw 로 폴백.
      · 미지(unknown) 키 → ValueError(명시).
    """
    if quant is None:
        return serve_bpw
    key = quant.strip().lower()
    if key in ("", "none"):
        return serve_bpw
    if key not in VLLM_QUANT_BYTES:
        raise ValueError(
            f"unknown vLLM quantization flag: {quant!r} "
            f"(known: {sorted(k for k in VLLM_QUANT_BYTES if k)})"
        )
    bpw = VLLM_QUANT_BYTES[key]
    if bpw is None:
        # null 류 별칭 — serve_bpw 주입.
        return serve_bpw
    return bpw


def vllm_quant_bpw_or_none(
    quant: "str | None", serve_bpw: float
) -> "tuple[float | None, str | None]":
    """vllm_quant_bpw 의 graceful 폴백 변종.

    - 알려진 키 → (bpw, None).  (vllm_quant_bpw 와 동일 값.)
    - 미지(unknown) 키 → (None, 경고문).  (ValueError 를 던지지 않는다.)

    호출측(parse 의 prequantized num_params 역산)이 ValueError 로 크래시하지 않고
    명시적 경고 + 대체경로(disk_bpw 폴백)로 떨어지도록 한 비예외(non-raising) 버전.
    """
    try:
        return vllm_quant_bpw(quant, serve_bpw), None
    except ValueError:
        return None, (
            f"unknown quant_method {quant!r} (테이블 미등록) "
            f"→ vllm_quant_bpw 산출 불가, 폴백 필요"
        )


def dtype_bpw(dtype: "str | None") -> "tuple[float, str | None]":
    """dtype 문자열 → (bytes-per-weight, warning|None).

    - dtype None/'' → 기본 2.0 + 경고.
    - DTYPE_BPW 매핑 적중 → (값, None).
    - 미지 dtype → (2.0, 경고문).
    """
    if dtype is None:
        return 2.0, "dtype is None; defaulting bpw=2.0"
    key = dtype.strip().lower()
    if key == "":
        return 2.0, "dtype is empty; defaulting bpw=2.0"
    if key in DTYPE_BPW:
        return DTYPE_BPW[key], None
    return 2.0, f"unknown dtype {dtype!r}; defaulting bpw=2.0"


if __name__ == "__main__":
    import argparse
    import json

    parser = argparse.ArgumentParser(
        description="quant 바이트 테이블 해소(벤더링) — quant/dtype → bpw."
    )
    parser.add_argument(
        "--quant",
        default=None,
        help="vLLM quantization 플래그(none/fp8/awq/...). 미지정 시 serve_bpw 반환.",
    )
    parser.add_argument(
        "--serve-bpw",
        type=float,
        default=2.0,
        help="native dtype bytes-per-weight (none/null quant 주입용, 기본 2.0).",
    )
    parser.add_argument(
        "--dtype",
        default=None,
        help="dtype 문자열(bfloat16/float16/float32/...) → bpw 해소.",
    )
    parser.add_argument("--json", action="store_true", help="JSON 출력.")
    args = parser.parse_args()

    out: dict = {}
    if args.dtype is not None:
        bpw, warn = dtype_bpw(args.dtype)
        out["dtype"] = args.dtype
        out["dtype_bpw"] = bpw
        out["dtype_warning"] = warn
    if args.quant is not None or args.dtype is None:
        out["quant"] = args.quant
        out["serve_bpw"] = args.serve_bpw
        out["vllm_quant_bpw"] = vllm_quant_bpw(args.quant, args.serve_bpw)

    if args.json:
        print(json.dumps(out, ensure_ascii=False, indent=2))
    else:
        for k, v in out.items():
            print(f"{k}: {v}")
