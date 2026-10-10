#!/bin/bash
# 81-exl3-model-wiring.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · 하이브리드 EXL3 팩(MXFP8 lm_head)의 모델 배선 (round 2)
#
# what : DeepSeek-V4.1 모델 코드 3곳을 고쳐 MXFP8 lm_head(`lm_head.weight` e4m3 + `lm_head.weight_scale` U8 [N, K/32])를 싣는다.
#          nvidia/model.py   : (a) lm_head ctor 에 quant_config 전달  (b) WeightsMapper regex 2개: `^lm_head\.weight$`->`head.weight`, `^lm_head\.weight_scale$`->`lm_head.weight_scale`
#          nvidia/vl_model.py: (c) 매퍼 prefix `lm_head.` -> `language_model.lm_head.` (스케일 키를 VL 루트 이름공간으로)
#        (a) 가 있어야 ParallelLMHead 가 Exl3Config.get_quant_method 를 거친다(플러그인 73 편집이 MXFP8 ModelOptLinearMethod 를 돌려준다).
# why  : stock 매퍼의 suffix 규칙 `head.weight`는 `lm_head.weight` 도 매치해(endswith) VL 매퍼에서 `lm_language_model.lm_head.weight` 로 깨지고(sfxnz lmhead_mxfp8.py Round-30 근본원인과 같은 현상),
#        `lm_head.weight_scale` 은 어느 규칙에도 안 걸려 래퍼 루트에 맨 이름으로 도달해 "no module or parameter named 'lm_head'" 로 죽는다. regex 는 suffix 보다 먼저 적용되므로
#        weight 키를 `head.weight` 로 되돌려 stock suffix 규칙이 정식 개명하게 한다. 팩 A 외 체크포인트에는 불활성(그런 키가 없다).
# ---- round 1(Mia 전-EXL3 팩용) 편집 중 **버린 것**과 증거(팩 A 헤더 188,246 텐서 전수 · fixtures/packA/headers.json) ----
#   * compressor fused_wkv_wgate quant_config / compressor_kv_score(.quant_method.apply)  : A 는 compressor.wkv/wgate 를 BF16 `.weight` 로 저장 -> stock(quant_config=None)이 정답. 적용하면 MXFP8 로 오빌드.
#   * indexer.wk quant_config                                                           : A 의 `indexer.wk.weight` 는 BF16 [128,512] -> stock 이 정답.
#   * o_proj packed wo_a(bmm 분기 · use_fp8 가드)                                         : A 의 wo_a 는 FP8 `.weight` F8_E4M3 [8192,4096] + `.scale` -> stock FP8 경로. `wo_a.slice.N.*` 키 0개.
#   * wo_a.slice.{i} 로더 분기(model.py / dspark.py)                                      : 슬라이스 키 0개(전수 확인) -> 도달 불가 코드.
#   * head.{trellis,suh,svh,mul1,mcg} suffix 매핑(model.py / vl_model.py)                 : A 의 헤드는 packed 가 아니다(`head.*` 키 0개).
# model-trigger : DeepseekV41 + MXFP8 lm_head 체크포인트 + --quantization exl3. **이 스크립트는 SM12X_PORT=1 변종에서만 적용**되므로 stock 이미지는 영향 없음.
# provenance : 우리 저장소의 자체 편집(외부 코드 사본 아님). 설계 근거 = sfxnz lmhead_mxfp8.py 의 매퍼 규칙 분석(파일은 받지 않는다).
# idempotent: 편집마다 `new` 문자열 존재로 판정(중복 적용 ✗). 앵커 불일치(count!=1) = 빌드 실패(fail-loud).
# probe: 최종중재 = GPU 서빙 스모크. 정적 보증 = py_compile + 모든 앵커 정확히 1회.
set -euo pipefail

TAG="[81-exl3-model-wiring]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export PYTHONPATH="$HERE${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import sys
import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
M = "# [exl3-port:wiring]"

LM_HEAD_OLD = """            self.lm_head = ParallelLMHead(
                config.vocab_size,
                config.hidden_size,
                prefix=maybe_prefix(prefix, "lm_head"),
            )
"""
LM_HEAD_NEW = f"""            self.lm_head = ParallelLMHead(
                config.vocab_size,
                config.hidden_size,
                quant_config=vllm_config.quant_config,  {M}
                prefix=maybe_prefix(prefix, "lm_head"),
            )
"""

REGEX_OLD = """        orig_to_new_regex=scale_regex,
"""
REGEX_NEW = f"""        orig_to_new_regex={{
            **scale_regex,
            {M} MXFP8 lm_head keys. Regexes run BEFORE the suffix pass: route the weight back to
            # `head.weight` so the stock suffix rule renames it (a suffix result ending in `head.weight` would be re-fired).
            re.compile(r"^lm_head\\.weight$"): "head.weight",
            re.compile(r"^lm_head\\.weight_scale$"): "lm_head.weight_scale",
        }},
"""

VL_PREFIX_OLD = """        "mtp.": "language_model.model.mtp.",
    }
"""
VL_PREFIX_NEW = f"""        "mtp.": "language_model.model.mtp.",
        "lm_head.": "language_model.lm_head.",  {M} MXFP8 lm_head scale key -> VL root namespace
    }}
"""

JOBS = [
    ("vllm/models/deepseek_v41/nvidia/model.py",
     [Edit("lm_head-ctor", LM_HEAD_OLD, LM_HEAD_NEW), Edit("lm_head-mapper-regex", REGEX_OLD, REGEX_NEW)]),
    ("vllm/models/deepseek_v41/nvidia/vl_model.py", [Edit("lm_head-vl-prefix", VL_PREFIX_OLD, VL_PREFIX_NEW)]),
]

results, touched = [], []
for rel, out in L.apply_edit_jobs(TAG, [(rel, edits, None, None) for rel, edits in JOBS]):  # two-phase: all anchors verified before any write
    print(f"{TAG} {rel}: {', '.join(out)}")
    results += out
    touched.append(L.VLLM_SRC / rel)

L.py_compile(TAG, touched)
# post-assert: 모든 편집 마커가 정확히 자리에 있다
for rel, edits in JOBS:
    t = (L.VLLM_SRC / rel).read_text()
    for e in edits:
        if e.new not in t:
            L.die(TAG, f"{rel}: post-assert missing edit {e.label}")
n_new = sum(1 for r in results if r.endswith(":applied"))
L.status("applied", f"{len(results)} edits ({n_new} newly applied)" if n_new else f"{len(results)} edits already present (no-op rerun)")
print(f"{TAG} OK - {len(results)} edits across {len(JOBS)} files ({n_new} new)")
PY
