#!/bin/bash
# 50-dsv4-sm12x-port.sh — **소스 이식 패치**(build_patches_src 슬롯 · pre-compile)
#   upstream-version-watch §4.7 확장 · 헌법 3+1+1 마지막 +1 의 **소스 위상**
#
# what : vLLM PR#41834(jasl, SM12x DeepSeek-V4 Flash enablement)의 **선별 이식본**을
#        stock v0.26.0 소스트리에 얹는다. 포크 전체를 핀하지 않고 필요한 변경만 가져온다.
#
# why  : stock v0.26.0 은 DeepSeek-V4-Flash-0731 의 spec decoding 경로 **둘 다** 막는다
#        (원인이 서로 다름 — testlog_26080215):
#          · dspark → sm_120 sparse-MLA 커널 갭.
#            flashinfer `_decode_dsv4_dispatchable` 이 `page_block_size == 64` 를 요구하는데
#            이 체크포인트는 compress_ratio 4/128 혼재 → ratio-128 이 256/128=2 로 탈락 →
#            paged_attention 폴백 → `sparse_mla_sm120.cu:261 Check failed: num_tokens > 64`.
#            block-size 한 값으로 양쪽을 동시에 디스패치 표에 넣을 수 없다(구조적).
#          · mtp    → 커널 이전에 로더 사망:
#            `KeyError 'model.layers.43.mtp_block.main_norm.weight'` (0731 은 `mtp.N.*` 신형 레이아웃).
#        PR#41834 는 flashinfer sparse_mla_sm120 대신 **SM12x Triton sparse-MLA** 를 써서
#        그 디스패치 표 자체를 안 탄다. 즉 여는 것은 성능 노브가 아니라 **커널 인스턴스**다.
#
# why-src-not-deps : 기존 `build_patches/`(post-compile)는 **빌드-바깥 native 의존 설치** 슬롯이다.
#        여기 이식분에는 `csrc/libtorch_stable/{topk.cu,persistent_topk.cuh}` 가 포함되는데
#        `_C` 재컴파일이 필요하므로 **컴파일 전**이어야 한다. Python 은 editable install 이라
#        사후에도 먹지만, 위상을 둘로 쪼개면 순서 의존이 생기므로 **한 위상으로 통일**한다.
#        (그 csrc 는 버릴 수 없다 — 원 주석이 우리 하드웨어를 지목한다:
#         "GB10 / consumer-Blackwell parts whose opt-in smem cannot host the 128KB
#          cooperative-radix fallback … fixes the long-context oversubscribe hard-fail".
#         1M 타깃이므로 long-context 경로가 정확히 우리 것이다.)
#
# model-trigger : DeepseekV4ForCausalLM(0731 정식판) + `--speculative-config method=dspark`.
#        sm_121a(GB10) 전용 경로이나 이미지-네이밍 불변식 보존(모델-키잉 ✗ — arch enablement).
#
# plan : plan_26080218(이식) · 선행 plan_26080213(사다리) · testlog_26080215(구조적 불가 확증) ·
#        testlog_26080217(포크핀 31.01 t/s = 이식본이 넘어야 할 기준선)
#
# ── 이식 절차(재현 가능 · PROVENANCE.json 이 정본) ─────────────────────────────
#   1) merge-base 분리: 포크가 `main` 기반이라 단순 diff 의 대부분이 업스트림 드리프트다.
#        merge-base 38a466e7 · PR 델타 191파일/+29,488  vs  드리프트 1,626파일/+135,207
#   2) 버킷 분류: 테스트·kv_offload·v0 레거시 proposer·타 플랫폼(rocm/xpu/amd)·타 모델
#        (kimi_k3/minimax)·파서/엔트리(stock 실증 통과) 제외 → 이식 후보
#   3) `git apply --3way` on v0.26.0 → 충돌 50헝크(14파일)
#   4) 헝크 판정: theirs 가 참조하는 심볼이 **이식 완료 트리**에 존재하는가.
#        없으면 드리프트 의존 → ours 유지. 결과 theirs 46 · ours 4
#   5) pyflakes **기준선 대비 diff** → 미정의 이름 14건 검출 → 전수 해소(현재 0)
#   6) 실제 import 시험 29/29 통과 → 그 다음에야 빌드
#
# ── ours 로 남긴 4헝크(전부 드리프트 의존 · PR 내용 아님) ─────────────────────
#   · attention.py#1 · model.py#4 : `_C.…_insert_out`(out-variant op) 바인딩이 v0.26.0 에
#       없고 `eager_scratch_pool` 도 드리프트. → v0.26.0 시그니처 유지
#   · model.py#6·#7 : `sp_all_gather` = `models/common/ops/sequence_parallel.py`(모듈 자체 부재)
#
# ── 되돌린 8파일(PR 변경이 드리프트 위에 얹힘) ───────────────────────────────
#   · v1/core/{kv_cache_utils,kv_cache_manager,kv_cache_coordinator,single_type_kv_cache_manager}.py
#     + v1/worker/utils.py  → `kv_offload.compact_geometry` 의존. 우리 경로 미참조 확인 후 제외
#   · v1/worker/gpu/spec_decode/rejection_sampler.py · v1/worker/gpu/model_runner.py
#   · models/deepseek_v4/common/ops/cache_utils.py → JIT-warmup 래퍼(드리프트) 의존.
#     **PR 이 추가한 최상위 심볼 4개만 선별 이식**(파일 하단 [port] 블록)
#
# ── 3way 가 놓친 것(충돌 표시 없이 조용히 섞인 자리 — pyflakes 가 잡음) ──────
#   · rejection_sampler.py `import numpy as np` / dspark/utils.py `get_draft_quant_config`
#       → PR 이 추가한 import 인데 헝크 밖이라 미반영
#   · attention.py : `q_out` 인자 + 죽은 `return q_out` 혼입 → v0.26.0 시그니처 복원
#   · sparse_mla.py : `active_topk_width`(드리프트 지역변수) → `self.c128a_max_compressed`
#   · model.py : `use_sequence_parallel` → False 고정(v0.26.0 동작)
#
# probe: stock 컨테이너 오버레이서 pyflakes 증분 0 · import 29/29 (2026-08-02).
#        실 커널 JIT(sm_121a)/서빙/spec accept = **serve 스모크가 최종 중재**.
# how  : vendored 파일 트리를 소스에 덮어쓴다(네트워크 불요 · PROVENANCE.json 해시 검증).
set -e

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/files"
DST="/workspace/vllm-src"
MAN="$(dirname "$SRC_DIR")/PROVENANCE.json"

[ -d "$SRC_DIR" ] || { echo "[50-dsv4-sm12x-port] FAIL: vendored files/ 부재: $SRC_DIR" >&2; exit 1; }
[ -f "$MAN" ]     || { echo "[50-dsv4-sm12x-port] FAIL: PROVENANCE.json 부재" >&2; exit 1; }

# 이 패치는 v0.26.0 소스에만 유효하다 — 다른 ref 위에 얹으면 조용히 틀린다(fail-loud).
BASE_SHA="$(python3 -c "import json,sys;print(json.load(open('$MAN'))['source']['upstream_base_sha'])")"
HEAD_SHA="$(git -C "$DST" rev-parse HEAD 2>/dev/null || echo unknown)"
if [ "$HEAD_SHA" != "$BASE_SHA" ]; then
    echo "[50-dsv4-sm12x-port] FAIL: 소스 ref 불일치 — 이 이식본은 v0.26.0($BASE_SHA) 전용이다." >&2
    echo "                      현재 HEAD=$HEAD_SHA. VLLM_REF 를 v0.26.0 으로 두거나 이식본을 재생성하라." >&2
    exit 1
fi

# vendored 무결성(공급망 가드): 배달된 파일이 발행 시점 해시와 같은가.
python3 - "$MAN" "$SRC_DIR" <<'PY'
import hashlib, json, pathlib, sys
man = json.load(open(sys.argv[1])); root = pathlib.Path(sys.argv[2])
bad = miss = 0
for rel, want in man["files"].items():
    p = root / rel
    if not p.is_file(): print(f"[50-dsv4-sm12x-port] MISSING {rel}"); miss += 1; continue
    if hashlib.sha256(p.read_bytes()).hexdigest() != want:
        print(f"[50-dsv4-sm12x-port] HASH MISMATCH {rel}"); bad += 1
if miss or bad:
    sys.exit(f"[50-dsv4-sm12x-port] FAIL: vendored 무결성 위반(missing={miss} mismatch={bad})")
print(f"[50-dsv4-sm12x-port] vendored 무결성 OK — {len(man['files'])} 파일")
PY

# 적용
COUNT=0
while IFS= read -r -d '' f; do
    rel="${f#"$SRC_DIR"/}"
    mkdir -p "$DST/$(dirname "$rel")"
    cp -f "$f" "$DST/$rel"
    COUNT=$((COUNT+1))
done < <(find "$SRC_DIR" -type f -print0)
echo "[50-dsv4-sm12x-port] 적용 $COUNT 파일 → $DST"

# 검증(fail-loud): 핵심 신규 모듈이 실제로 자리에 있는가. import 는 torch/CUDA 가 필요해
#   빌드타임에 못 하므로(core 빌드검증 불변식과 동일) 존재+구문만 본다. 최종중재 = serve 스모크.
for must in \
    vllm/v1/attention/backends/mla/sparse_mla_kernels.py \
    vllm/v1/attention/backends/mla/sparse_mla_env.py \
    vllm/models/deepseek_v4/nvidia/dspark_triton.py \
    vllm/models/deepseek_v4/nvidia/ops/sm12x_mqa.py \
    vllm/models/deepseek_v4/nvidia/ops/sm12x_deep_gemm_fallbacks.py \
    vllm/model_executor/warmup/deepseek_v4_sm12x_warmup.py \
    csrc/libtorch_stable/topk.cu \
    csrc/libtorch_stable/persistent_topk.cuh ; do
    [ -f "$DST/$must" ] || { echo "[50-dsv4-sm12x-port] FAIL: 적용 후에도 부재: $must" >&2; exit 1; }
done
python3 -m compileall -q "$DST/vllm/models/deepseek_v4" "$DST/vllm/v1/attention/backends/mla" >/dev/null \
    || { echo "[50-dsv4-sm12x-port] FAIL: 이식본 구문 오류" >&2; exit 1; }

# csrc 가 실제로 바뀌었는지(= _C 재컴파일이 이 이식을 반영하는지) 앵커 확인.
grep -q "VLLM_TOPK_DISABLE_NONCOOP" "$DST/csrc/libtorch_stable/topk.cu" \
    || { echo "[50-dsv4-sm12x-port] FAIL: topk.cu 에 GB10 non-cooperative 게이트 앵커 부재" >&2; exit 1; }

echo "[50-dsv4-sm12x-port] OK — SM12x Triton sparse-MLA + DSpark 이식 적용(컴파일 전). 최종중재 = serve 스모크."
