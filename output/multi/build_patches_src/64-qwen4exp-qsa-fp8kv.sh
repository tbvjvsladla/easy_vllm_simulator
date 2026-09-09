#!/bin/bash
# 64-qwen4exp-qsa-fp8kv.sh — **소스 이식 패치**(build_patches_src 슬롯 · pre-compile)
#
# what : qwen4_exp QSA(Triton) attention 의 KV 캐시에 **fp8_e4m3** 를 허용한다.
#        stock 은 5+2 개 가드로 bf16 외를 전부 거부한다(`supported=["auto","bfloat16"]`
#        · "requires a BF16 main KV cache" 외). vLLM 코어는 이미 양자화 배선을 갖췄다
#        (kv_quant_mode 전달 · `_k_scale`/`_v_scale` 등록 · 할당/쓰기는 제네릭 경로) —
#        구멍은 **읽기**(Triton 커널이 bf16 상정 로드)뿐이다. 이 패치는 읽기에
#        디콴트를 얹고 가드를 넓힌다.
#        `--kv-cache-dtype auto/bfloat16` 이면 **완전 불활성**: FP8_KV constexpr 이
#        거짓이라 Triton 컴파일에서 분기가 소거된다(stock 과 바이트 동일 경로).
#
# reference : github.com/blazux/qwen3.8-Flash-DGX `src/patch_qsa_fp8_kv.py` (401줄 · 17편집)
#        실측 근거(그들의 싱글노드): KV 풀 ×1.9 · 디코드 −10% · 프리필 −30%.
#        디콴트는 MiaAI-Lab(SGLang)식 자체 작성이 아니라 **vLLM 정준 `_cast_kv_tile`**
#        (triton_unified_attention) 재사용 — 두 번째 진실을 만들지 않는다.
#
# 우리 베이스 어댑테이션: 가드 메시지의 아치명만 다르다(Qwen3.8-Flash-Next → Qwen4Exp).
#        20개 앵커는 전수 count==1 실측(2026-09-09 · anchor_check).
#        포함된 함정 수정 3가지(참조가 실측으로 발견 — 재현 금지):
#          · #19 인덱서 압축캐시(uint8) 미재해석 — 블록 선택이 첫 토큰부터 붕괴(11/12 오답 실측)
#          · #18 sm_121 공유메모리 99KiB 벽 — fp32 타일이 5,120B 초과 → 양자화 시 block_n 반감
#          · #15/17 가드가 5개가 아니라 7개 — QSAAttention.__init__ 의 cache_config 가드와
#            FlashAttentionImpl 부모 가드(QSA 커널과 무관한 능력을 검사) 중화
#
# gate : 없음(ungated) — bf16 경로 불활성 백포트(60·62 와 같은 판정).
#
# ★ 검증 원칙(참조의 경고를 계승): "부팅됨"은 아무것도 증명하지 않는다 — 조용히
#   그럴듯한 오답을 내는 실패 모드가 실측된 부위다. P0-3 스모크는 **bf16-KV 대비
#   토큰 동등성**(greedy·고정 프롬프트·seed 고정)을 비교한다. 정확히 일치하지 않으면
#   확률 분포 근접도가 아니라 **실패**로 분류한다.
#
# plan : plan_26090918 §Phase 0 P0-3.
set -euo pipefail

TAG="[64-qwen4exp-qsa-fp8kv]"
DST="/workspace/vllm-src"
OPS="$DST/vllm/models/qwen4_exp/nvidia/ops/qsa.py"
OWN="$DST/vllm/models/qwen4_exp/nvidia/qsa.py"

[ -f "$OPS" ] && [ -f "$OWN" ] || { echo "$TAG FAIL: qsa 트리 부재 — 베이스가 아니다" >&2; exit 1; }
echo "$TAG base HEAD: $(git -C "$DST" rev-parse HEAD 2>/dev/null || echo unknown)"

# 머지 트립와이어: 상류가 fp8 KV 를 이미 받으면 빌드를 멈춰 이 패치 제거를 강제한다.
if grep -q '"fp8_e4m3"' "$OWN" || grep -q '_cast_kv_tile' "$OPS"; then
    echo "$TAG FAIL: QSA fp8 KV 경로가 이미 존재한다 — 상류 머지로 보인다. 이 패치를 제거하고 stock 경로로 스모크하라." >&2
    exit 1
fi

python3 - "$OPS" "$OWN" <<'PY'
import ast, sys

OPS, OWN = sys.argv[1], sys.argv[2]
TAG = "[64-qwen4exp-qsa-fp8kv]"

def sub(text, old, new, what):
    n = text.count(old)
    if n != 1:
        print(f"{TAG} FAILED — anchor {what!r} matched {n} times", file=sys.stderr)
        raise SystemExit(1)
    return text.replace(old, new)

# ================================================================ ops/qsa.py
ops = open(OPS).read()

# 0. vLLM 정준 디콴트 헬퍼 재사용
ops = sub(ops,
    "from vllm.triton_utils import HAS_TRITON, tl, triton",
    "from vllm.triton_utils import HAS_TRITON, tl, triton\n"
    "from vllm.v1.attention.ops.triton_unified_attention import _cast_kv_tile",
    "import canonical cast helper")

# 1. DECODE 커널 시그니처
ops = sub(ops,
    "    num_requests,\n    TOPK: tl.constexpr,",
    "    num_requests,\n"
    "    k_scale_ptr,\n"
    "    v_scale_ptr,\n"
    "    KV_QUANT_MODE: tl.constexpr,\n"
    "    TOPK: tl.constexpr,",
    "decode kernel signature")

# 2. DECODE 커널 디콴트
ops = sub(ops,
    "        scores = tl.dot(query, keys)\n"
    "        # Scaling scores avoids re-quantizing a scaled query to BF16.",
    "        keys = _cast_kv_tile(keys, query, k_scale_ptr, KV_QUANT_MODE)\n"
    "        values = _cast_kv_tile(values, query, v_scale_ptr, KV_QUANT_MODE)\n"
    "        scores = tl.dot(query, keys)\n"
    "        # Scaling scores avoids re-quantizing a scaled query to BF16.",
    "decode kernel dequant")

# 3. MQA(압축캐시) 커널 시그니처
ops = sub(ops,
    "    score_divisor,\n    PAGE_SIZE: tl.constexpr,",
    "    score_divisor,\n"
    "    kc_scale_ptr,\n"
    "    KC_QUANT_MODE: tl.constexpr,\n"
    "    PAGE_SIZE: tl.constexpr,",
    "mqa kernel signature")

# 4. MQA 커널 디콴트
ops = sub(ops,
    "        scores = tl.dot(keys, query, out_dtype=tl.float32)",
    "        keys = _cast_kv_tile(keys, query, kc_scale_ptr, KC_QUANT_MODE)\n"
    "        scores = tl.dot(keys, query, out_dtype=tl.float32)",
    "mqa kernel dequant")

# 5. 래퍼 dtype 가드 — 캐시 fp8 + 쿼리 bf16 허용
ops = sub(ops,
    "    assert q.dtype == k_cache.dtype == v_cache.dtype == torch.bfloat16",
    "    assert q.dtype == torch.bfloat16\n"
    "    assert k_cache.dtype == v_cache.dtype\n"
    "    _fp8_kv = k_cache.dtype == torch.float8_e4m3fn  # e5m2: the mqa launch has no quant mode for it\n"
    "    # 1 = FP8_PER_TENSOR in KVQuantMode. The per-token-head modes (2, 3)\n"
    "    # apply their scales elsewhere in the loop and are not wired here.\n"
    "    _kv_mode = 1 if _fp8_kv else 0\n"
    "    assert k_cache.dtype == torch.bfloat16 or _fp8_kv, (\n"
    '        f"QSA: KV cache is {k_cache.dtype}, expected bf16 or fp8_e4m3"\n'
    "    )",
    "wrapper dtype guard")

# 6. DECODE 커널 런치 — 스케일 전달
ops = sub(ops,
    "        block_table.shape[0],\n        TOPK=logical_indices.shape[1],",
    "        block_table.shape[0],\n"
    "        _k_scale_t,\n"
    "        _v_scale_t,\n"
    "        KV_QUANT_MODE=_kv_mode,\n"
    "        TOPK=logical_indices.shape[1],",
    "decode kernel launch")

# 7. 스케일 텐서 실체화(부재 시 1.0 — 수치 no-op)
ops = sub(ops,
    "    _qsa_sparse_paged_gqa_splitk_kernel[partial_grid](",
    "    _one = torch.ones((), dtype=torch.float32, device=q.device)\n"
    "    _k_scale_t = k_scale if k_scale is not None else _one\n"
    "    _v_scale_t = v_scale if v_scale is not None else _one\n"
    "    _qsa_sparse_paged_gqa_splitk_kernel[partial_grid](",
    "decode scales materialize")

# 8. 래퍼 공개 시그니처 — 스케일 선택 인자
ops = sub(ops,
    "    token_to_req: torch.Tensor,\n"
    "    out: torch.Tensor | None = None,\n"
    ") -> torch.Tensor:",
    "    token_to_req: torch.Tensor,\n"
    "    out: torch.Tensor | None = None,\n"
    "    k_scale: torch.Tensor | None = None,\n"
    "    v_scale: torch.Tensor | None = None,\n"
    ") -> torch.Tensor:",
    "wrapper public signature")

# 19. 인덱서 압축캐시 재해석(미재해석 시 블록 선택 붕괴 — 참조 실측 11/12 오답)
ops = sub(ops,
    "    _validate_mqa(q)",
    "    # The block selector reads the indexer cache, a SEPARATE tensor from the\n"
    "    # main KV cache that follows the same dtype. Left as uint8 the kernel\n"
    "    # would read integers and pick arbitrary blocks.\n"
    "    if k_cache.dtype == torch.uint8:\n"
    "        k_cache = k_cache.view(torch.float8_e4m3fn)\n"
    "    _validate_mqa(q)",
    "indexer cache reinterpretation")

# 9. MQA 런치 — 중성 스케일
ops = sub(ops,
    "        float(score_divisor),\n        PAGE_SIZE=k_cache.shape[1],",
    "        float(score_divisor),\n"
    "        torch.ones((), dtype=torch.float32, device=q.device),\n"
    "        1 if k_cache.dtype == torch.float8_e4m3fn else 0,\n"
    "        PAGE_SIZE=k_cache.shape[1],",
    "mqa kernel launch")

# 18. sm_121 공유메모리 99KiB — 양자화 시 block_n 반감
ops = sub(ops,
    "    num_tiles = triton.cdiv(logical_indices.shape[1], block_n)",
    "    if _kv_mode != 0:\n"
    "        # _cast_kv_tile materialises an fp32 tile, doubling shared memory for\n"
    "        # K and V. sm_121 caps at 101376 bytes and the kernel asked for\n"
    "        # 106496; halving the N block fits.\n"
    "        block_n = max(16, block_n // 2)\n"
    "    num_tiles = triton.cdiv(logical_indices.shape[1], block_n)",
    "halve block_n under quant")

ast.parse(ops)
open(OPS, "w").write(ops)
print(f"{TAG} ops/qsa.py patched (decode+mqa kernels dequant)")

# ==================================================================== qsa.py
owner = open(OWN).read()

# 10. 지원 선언
owner = sub(owner,
    '    supported_kv_cache_dtypes: ClassVar[list[CacheDType]] = ["auto", "bfloat16"]',
    '    supported_kv_cache_dtypes: ClassVar[list[CacheDType]] = [\n'
    '        "auto",\n'
    '        "bfloat16",\n'
    '        "fp8",\n'
    '        "fp8_e4m3",\n'
    '    ]',
    "supported dtypes")

# 11. init 가드 (우리 트리는 한 줄 raise 형식)
owner = sub(owner,
    '        if self.kv_cache_dtype not in ("auto", "bfloat16"):\n'
    '            raise NotImplementedError("Qwen4Exp QSA requires a BF16 main KV cache")',
    '        if self.kv_cache_dtype not in ("auto", "bfloat16", "fp8", "fp8_e4m3"):\n'
    "            raise NotImplementedError(\n"
    '                f"Qwen4Exp QSA: {self.kv_cache_dtype} is not supported "\n'
    '                "(bf16 and fp8_e4m3 are)"\n'
    "            )",
    "init guard")

# 12. 커널 진입 가드
owner = sub(owner,
    "        if key_cache.dtype != torch.bfloat16 or query.dtype != torch.bfloat16:\n"
    '            raise NotImplementedError("Qwen4Exp QSA requires BF16 Q/K/V")',
    "        if query.dtype != torch.bfloat16:\n"
    '            raise NotImplementedError("Qwen4Exp QSA requires a BF16 query")\n'
    "        if key_cache.dtype not in (\n"
    "            torch.bfloat16,\n"
    "            torch.float8_e4m3fn,\n"
    "            torch.uint8,\n"
    "        ):\n"
    "            raise NotImplementedError(\n"
    '                f"Qwen4Exp QSA: cache dtype {key_cache.dtype} is not supported"\n'
    "            )",
    "kernel entry guard")

# 13. 스토리지 가드 — 코어는 양자화 캐시를 uint8 로 할당한다 (한 줄 raise)
owner = sub(owner,
    "        if self.kv_cache_torch_dtype != torch.bfloat16:\n"
    '            raise NotImplementedError("Qwen4Exp QSA requires BF16 cache storage")',
    "        if self.kv_cache_torch_dtype not in (\n"
    "            torch.bfloat16,\n"
    "            torch.float8_e4m3fn,\n"
    "            # vLLM ALLOCATES the quantised cache as uint8: raw bytes,\n"
    "            # reinterpreted as fp8 right before the kernel (see the\n"
    "            # `.view()` in `forward_qsa`). Rejecting uint8 rejected the\n"
    "            # only storage the core produces.\n"
    "            torch.uint8,\n"
    "        ):\n"
    "            raise NotImplementedError(\n"
    '                f"Qwen4Exp QSA: storage dtype {self.kv_cache_torch_dtype} "\n'
    '                "is not supported"\n'
    "            )",
    "storage guard")

# 15. 두 번째 클래스(QSAAttention.__init__)의 cache_config 가드 (한 줄 raise)
owner = sub(owner,
    '        if cache_config.cache_dtype not in ("auto", "bfloat16"):\n'
    '            raise NotImplementedError("Qwen4Exp QSA requires a BF16 main KV cache")',
    '        if cache_config.cache_dtype not in ("auto", "bfloat16", "fp8", "fp8_e4m3"):\n'
    "            raise NotImplementedError(\n"
    '                f"Qwen4Exp QSA: cache_dtype {cache_config.cache_dtype} "\n'
    '                "is not supported (bf16 and fp8_e4m3 are)"\n'
    "            )",
    "cache_config guard (QSAAttention class)")

# 17. FlashAttentionImpl 부모 가드 중화 — QSA 는 부모 커널을 안 쓴다
owner = sub(owner,
    "    def __init__(self, *args, **kwargs) -> None:\n"
    "        super().__init__(*args, **kwargs)\n"
    "        if not is_flash_attn_varlen_func_available():",
    "    def __init__(self, *args, **kwargs) -> None:\n"
    "        # FlashAttentionImpl rejects a quantised cache because ITS kernels\n"
    "        # cannot read it on this device. QSA does not use them: it calls\n"
    "        # qsa_sparse_paged_attention (Triton) and only inherits the\n"
    "        # surrounding plumbing. Neutralise the dtype for the parent init,\n"
    "        # then restore it.\n"
    "        _fp8 = (\"fp8\", \"fp8_e4m3\")\n"
    "        _real_kv_dtype = None\n"
    "        if kwargs.get(\"kv_cache_dtype\") in _fp8:\n"
    "            _real_kv_dtype = kwargs[\"kv_cache_dtype\"]\n"
    "            kwargs[\"kv_cache_dtype\"] = \"auto\"\n"
    "        elif len(args) > 6 and args[6] in _fp8:\n"
    "            _real_kv_dtype = args[6]\n"
    "            args = args[:6] + (\"auto\",) + args[7:]\n"
    "        super().__init__(*args, **kwargs)\n"
    "        if _real_kv_dtype is not None:\n"
    "            self.kv_cache_dtype = _real_kv_dtype\n"
    "        if not is_flash_attn_varlen_func_available():",
    "FlashAttentionImpl parent guard neutralisation")

# 16. uint8 → fp8 재해석(커널 직전)
owner = sub(owner,
    "        key_cache = canonicalize_singleton_dim_strides(key_cache)\n"
    "        value_cache = canonicalize_singleton_dim_strides(value_cache)",
    "        key_cache = canonicalize_singleton_dim_strides(key_cache)\n"
    "        value_cache = canonicalize_singleton_dim_strides(value_cache)\n"
    "        # uint8 holds the raw bytes of an fp8 cache: REINTERPRET them, do not\n"
    "        # convert (same step triton_attn.py takes for unified attention).\n"
    "        if key_cache.dtype == torch.uint8:\n"
    "            key_cache = key_cache.view(torch.float8_e4m3fn)\n"
    "            value_cache = value_cache.view(torch.float8_e4m3fn)",
    "uint8 -> fp8 reinterpretation")

ast.parse(owner)
open(OWN, "w").write(owner)
print(f"{TAG} qsa.py patched (dtypes declared, 5 guards widened, parent guard neutralised)")
PY

# ── 빌드타임 검증(fail-loud) ──
python3 -m compileall -q "$OPS" "$OWN" || { echo "$TAG FAIL: compileall" >&2; exit 1; }
grep -q '_cast_kv_tile' "$OPS" && grep -q '"fp8_e4m3"' "$OWN" || { echo "$TAG FAIL: 적용 마커 부재" >&2; exit 1; }
echo "$TAG OK — QSA fp8 KV 이식·검증 통과(최종 중재 = bf16-KV 대비 토큰 동등성 스모크)"
