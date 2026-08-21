#!/bin/bash
# 50-dsv4-sm12x-port.sh — **소스 이식 패치**(build_patches_src 슬롯 · pre-compile)
#   upstream-version-watch §4.7 확장 · 헌법 3+1+1 마지막 +1 의 **소스 위상**
#
# what : vLLM PR#41834(jasl, SM12x DeepSeek-V4 Flash enablement)의 **선별 이식본**을
#        stock v0.27.1 소스트리에 얹는다. 포크 전체를 핀하지 않고 필요한 변경만 가져온다.
#        (R2 = arch-wall 사다리의 `자체 이식` 칸. 실패 시 다음 칸이 R3 포크 핀이다.)
#
# why  : stock v0.27.x 는 DeepSeek-V4-Flash-0731 의 spec decoding 을 **양쪽 다** 막는다.
#        2026-08-14 에 두 경로를 실측했고 **서로 다른 벽**임이 확정됐다:
#          · method=dspark → 드래프트 로드는 성공하나 커널 워밍업에서 즉사.
#            flashinfer `_decode_dsv4_dispatchable` 이 page_block_size==64 를 요구하는데
#            이 체크포인트는 compress_ratio 4/128 혼재라 ratio-128 이 탈락 → paged_attention
#            폴백 → `Check failed: num_tokens > 64` (testlog_26081418).
#          · method=mtp   → 커널 이전에 로더 사망.
#            `KeyError 'model.layers.43.mtp_block.main_norm.weight'`. 원인은 이름이 아니라
#            **모듈 구조**다 — 0731 의 mtp.* 는 plain-MTP 가 아니라 DSpark 드래프터
#            (markov_head·confidence_head·hc_*·main_proj). stock 의 MTP 모듈엔 담을 슬롯이
#            없다 → 이 체크포인트에 method=mtp 는 **구조적 부적용** (testlog_26081419).
#        PR#41834 는 flashinfer sparse_mla_sm120 대신 **SM12x Triton sparse-MLA** 를 써서
#        그 디스패치 표 자체를 안 탄다. 즉 여는 것은 성능 노브가 아니라 **커널 인스턴스**다.
#
# why-src-not-post : `build_patches/`(post-compile)는 **빌드-바깥 native 의존 설치** 슬롯이다.
#        이식분에는 `csrc/` 9파일이 포함되고 `_C` 재컴파일이 필요하므로 **컴파일 전**이어야
#        한다. Python 은 editable install 이라 사후에도 먹지만, 위상을 둘로 쪼개면 순서
#        의존이 생기므로 **한 위상으로 통일**한다(v0.26.0 스크립트의 결정 승계).
#        그 csrc 는 버릴 수 없다 — 원 주석이 우리 하드웨어를 지목한다:
#         "GB10 / consumer-Blackwell parts whose opt-in smem cannot host the 128KB
#          cooperative-radix fallback … fixes the long-context oversubscribe hard-fail".
#        1M 타깃이므로 long-context 경로가 정확히 우리 것이다.
#
# model-trigger : DeepseekV4ForCausalLM(0731 정식판) + `--speculative-config method=dspark`.
#        sm_121a(GB10) 전용 경로이나 이미지-네이밍 불변식 보존(모델-키잉 ✗ — arch enablement).
#
# plan : plan_26081418(스코프 확정) · plan_26081410 §4(사다리) ·
#        testlog_26081418(R1-a) · testlog_26081419(R1-b) · testlog_26080223(v0.26.0 이식 3회 실패)
#
# ── 좌표(PROVENANCE.json 이 정본) ─────────────────────────────────────────────
#   upstream_base = v0.27.1            6e448d0ea9bf3d88d898b65449ca6dc2aec170ac
#     └ 2026-08-21 재베이스(plan_26082111 §4). 이전 base = v0.27.0 4bdc8a788d2e (롤백 앵커).
#       v0.27.1 의 유일한 런타임 델타 #50424(qwen3_dspark.py +4, quantized Markov head)를
#       0.27.0 이식본이 **조용히 되돌리는** 것이 실측돼(carry-forward·침묵 누락) 좌표를 옮겼다.
#       재파생 108파일 중 바이트가 바뀐 것은 qwen3_dspark.py 1건뿐이다(실측).
#   pr_base       = main(three-dot MB) 50ba4bc6b2cacd70c4711a1904dd7ad9740a578b
#   fork_head     = PR#41834           9ad62027bc84ca0ccbcc40853179312de770220c
#   ⚠ v0.26.0 이식 때 쓰던 `merge_base(fork_head, tag)` 는 **스코프 추출 기준이 아니다.**
#     그것으로 diff 하면 PR 브랜치가 흡수한 main 드리프트까지 딸려와 1,422파일이 된다
#     (2026-08-14 실측). GitHub 'Files changed' 와 일치하는 기준은 pr_base 다 → 221파일.
#
# ── 이식 절차(재현 가능) ─────────────────────────────────────────────────────
#   1) PR 델타 추출: diff pr_base..fork_head = 221파일 / +32,347 / −1,477 (GitHub API 일치)
#   2) 버킷 분류 → 이식 105 / 제외 116
#        제외: tests 68 · RTX PRO 6000 튜닝 18 · kv_offload 13 · entrypoints·파서류 14 ·
#              docker 2 · requirements 1
#        ★ 2026-08-14 R2 재판정: requirements/cuda.txt 를 **스코프 안으로 되돌렸다**(아래 §핀 승격).
#          제외 판정의 전제가 "45-flashinfer 가 pip 로 설치하면 끝"이었는데, 그 뒤 #20
#          `pip install -e .` 가 stock cuda.txt 핀을 만나 **python 만 0.6.16.post3 로 되돌리는**
#          것이 실측됐다(짝 어긋남 → import 크래시). 즉 이 파일은 "핀 표기"가 아니라
#          **설치 결과를 결정하는 입력**이었다.
#   3) `git apply --3way` on v0.27.1 → **충돌 8파일 / 16헝크**
#        (2026-08-21 실측 — v0.27.0 base 와 동일. 중단 임계 50헝크 대비 여유)
#        (v0.26.0 때는 14파일 / 50헝크였다. 중단 임계 50 대비 큰 여유)
#   4) 헝크 판정: theirs 가 참조하는 심볼이 이식 완료 트리에 있는가
#   5) 정적 검증 5단계(전부 통과 — 상세는 아래)
#   6) **서빙 스모크 = 최종 중재** ← 여기서 죽으면 위 전부 통과해도 실패다
#
# ── 헝크 판정 결과 8파일 ─────────────────────────────────────────────────────
#   [PR본 통째 채택] 드리프트가 PR 재작성 대비 미미한 파일(파일 단위 정합 우선):
#     · nvidia/dspark.py       드리프트 +24/−5  vs PR +1040/−362 (43배)
#     · nvidia/flashmla.py     드리프트 +24/−3  vs PR  +832/−47  (35배)
#     · layers/sparse_attn_indexer.py 드리프트 +8/−1 vs PR +218/−86
#       ↳ 단 이 파일은 드리프트 2곳을 **되돌렸다**(파일 내 [port:revert-drift] 주석):
#          (a) `indices=decode_metadata.indices` — v0.27.0 의
#              utils/deep_gemm.fp8_fp4_paged_mqa_logits 에 `indices` 파라미터가 없다 → TypeError
#          (b) `is_rdna_aiter_enabled()` — v0.27.0 에 정의 부재(ROCm 경로, 우리 미실행이나
#              미정의 심볼을 남기지 않는다)
#   [헝크 판정] 드리프트가 우세한 파일:
#     · config/speculative.py  theirs — dspark_* 필드 추가(순수 삽입). envs.env_bool 는 PR 이 공급
#     · models/registry.py     theirs **−1줄** — `BailingMoeV3MTPModel` 은 드리프트이고
#                              모듈 `bailing_moe_v3_mtp` 가 v0.27.0 에 없다 → 제외
#     · fused_moe/oracle/mxfp4.py theirs — docstring 뿐
#     · mla/indexer.py         **혼합** — require_uniform 은 ours(PR 의 `or supports_varlen` 은
#                              upstream #47808 드리프트이고 v0.27.0 에 정의 없음),
#                              treat_short_extends_as_decodes 는 theirs(PR 고유이고
#                              sparse_short_extend_tiering 이 이식본 backends/utils.py 에 존재)
#     · gpu/spec_decode/dspark/speculator.py ours — adaptive-verification 가드는 #47808
#                              드리프트이며 v0.27.0 엔 그 기능 자체가 없다
#
# ── 스코프 밖 3파일을 **추가**했다(105 → 108) ────────────────────────────────
#   · models/qwen3_dspark.py  : `DSparkConfidenceHead` **최소 추출**(+26줄). PR 델타 밖이지만
#       PR 이 재작성한 nvidia/dspark.py 가 25곳에서 참조한다. v0.26.0 1차 실패가 정확히
#       "안 필요해 보인다"로 제외한 결과였으므로 **제외가 아니라 공급**을 택했다.
#       파일의 나머지 드리프트(+101/−4)는 가져오지 않는다.
#   · v1/kv_offload/compact_geometry.py + config.py : plan_26081418 §3.3 이 명시한
#       **되돌림 조건 발동**. v1/core/kv_cache_utils.py·v1/kv_cache_interface.py 가
#       모듈 레벨에서 import 한다 → 빼면 엔진 기동 시 ImportError.
#
# ── 정적 검증 결과(빌드 전) ──────────────────────────────────────────────────
#   1 compileall               PASS
#   2 pyflakes **기준선 대비 증분**  13건 → 그중 **12건은 PR 원본에도 존재**(승계).
#     ★ 이식 고유 1건 = `kv_cache_coordinator.py undefined name 'logger'` — PR 의 로깅 헝크가
#       쓰는 logger 정의가 드리프트라 3-way 가 헝크 밖으로 조용히 통과시켰다. 2줄 추가로 해소.
#   3 부재 모듈 검사            실질 1건(compact_geometry) → 폐포 2파일 추가로 해소.
#                              잔여 4건은 `_C_stable_libtorch`·`_qutlass_C`·`third_party/
#                              triton_kernels` = 빌드 산출/vendored(위양성, base 도 동일)
#   4 AST 시그니처 대조         정의 차이 22건 · **실제 계약 위반 0건**
#                              (호출부가 PR 전용 파라미터를 쓰는 곳 0 · 위치인자 위반 0)
#   5 import 시험              ⚠ 이 relay 미실행(torch/CUDA 필요 — 컨테이너 평면)
#
# ── ⚠ 알려진 갭 ─────────────────────────────────────────────────────────────
#   · v1/kv_offload/cpu/compact_accounting.py 미이식. compact_geometry 의
#     build_compact_slice_accounting 안 **지연 import** 로만 도달하고 그 함수는
#     `enable_compact_layout`(CPU KV 오프로딩) 가드 뒤다. 우리는 켜지 않는다. **켜면 ImportError.**
#
# probe: 최종 중재는 serve 스모크다. 정적 5단계는 빌드 횟수를 줄일 뿐 이식 가능성을 판정하지
#        못한다(testlog_26080223 §3 — 5단계 전부 통과하고도 3회 서빙 실패했다).
# how  : vendored 파일 트리를 소스에 덮어쓴다(네트워크 불요 · PROVENANCE.json 해시 검증).
set -euo pipefail

TAG="[50-dsv4-sm12x-port]"

# ══ G-4 변종 게이트 ═══════════════════════════════════════════════════════════
#   이 게이트가 없으면 스크립트를 디렉터리에 놓는 순간 **이후 모든 multi 빌드가 이식본**이
#   된다 — stock 재빌드가 조용히 변종이 되는데 이미지 태그는 그대로일 수 있다.
#   workflow.md 막힘 3분류의 **침묵 누락**이며 plan_26081410 §10.3.2.1 의
#   *"배선이 다른 이미지가 같은 이름을 갖는 것은 불변식이 아니라 결함"* 과 동일 클래스다.
#   변종 좌표의 거처는 `.claude/policies/arch_variant_ledger.json`(Band2).
: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip — SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    exit 0
fi
# ══════════════════════════════════════════════════════════════════════════════

SRC_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/files"
DST="/workspace/vllm-src"
MAN="$(dirname "$SRC_DIR")/PROVENANCE.json"

[ -d "$SRC_DIR" ] || { echo "$TAG FAIL: vendored files/ 부재: $SRC_DIR" >&2; exit 1; }
[ -f "$MAN" ]     || { echo "$TAG FAIL: PROVENANCE.json 부재" >&2; exit 1; }

# 이 이식본은 PROVENANCE 가 선언한 base ref 에만 유효하다 — 다른 ref 위에 얹으면 조용히 틀린다(fail-loud).
# 태그·SHA 둘 다 PROVENANCE 에서 읽는다(손으로 적지 않는다 — 안내문이 틀리면 가드가 있어도 사고가 난다).
BASE_SHA="$(python3 -c "import json;print(json.load(open('$MAN'))['source']['upstream_base_sha'])")"
BASE_TAG="$(python3 -c "import json;print(json.load(open('$MAN'))['source']['upstream_base_tag'])")"
HEAD_SHA="$(git -C "$DST" rev-parse HEAD 2>/dev/null || echo unknown)"
if [ "$HEAD_SHA" != "$BASE_SHA" ]; then
    echo "$TAG FAIL: 소스 ref 불일치 — 이 이식본은 ${BASE_TAG}(${BASE_SHA}) 전용이다." >&2
    echo "$TAG       현재 HEAD=$HEAD_SHA. VLLM_REF 를 ${BASE_TAG} 로 두거나 이식본을 재생성하라." >&2
    exit 1
fi

# vendored 무결성(공급망 가드): 배달된 파일이 발행 시점 해시와 같은가.
python3 - "$MAN" "$SRC_DIR" <<'PY'
import hashlib, json, pathlib, sys
man = json.load(open(sys.argv[1])); root = pathlib.Path(sys.argv[2])
bad = miss = 0
for rel, want in man["files"].items():
    p = root / rel
    if not p.is_file():
        print(f"[50-dsv4-sm12x-port] MISSING {rel}"); miss += 1; continue
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
echo "$TAG 적용 $COUNT 파일 → $DST"

# 검증(fail-loud): 핵심 신규 모듈이 실제로 자리에 있는가. import 는 torch/CUDA 가 필요해
#   빌드타임에 못 하므로(core 빌드검증 불변식과 동일) 존재+구문만 본다. 최종중재 = serve 스모크.
for must in \
    vllm/v1/attention/backends/mla/sparse_mla_kernels.py \
    vllm/v1/attention/backends/mla/sparse_mla_env.py \
    vllm/models/deepseek_v4/nvidia/dspark_triton.py \
    vllm/models/deepseek_v4/nvidia/flashinfer_sm120_decode.py \
    vllm/models/deepseek_v4/nvidia/ops/sm12x_mqa.py \
    vllm/models/deepseek_v4/nvidia/ops/sm12x_deep_gemm_fallbacks.py \
    vllm/models/deepseek_v4/nvidia/ops/fp8_einsum.py \
    vllm/model_executor/warmup/deepseek_v4_sm12x_warmup.py \
    vllm/v1/spec_decode/dspark.py \
    vllm/v1/spec_decode/dspark_sampling.py \
    vllm/v1/kv_offload/compact_geometry.py \
    csrc/libtorch_stable/topk.cu \
    csrc/libtorch_stable/persistent_topk.cuh ; do
    [ -f "$DST/$must" ] || { echo "$TAG FAIL: 적용 후에도 부재: $must" >&2; exit 1; }
done

# v0.26.0 1차 실패 재발 방지: DSparkProposer 를 import 하는 러너가 있는데 그 모듈이 없으면
#   엔진 초기화에서 ModuleNotFoundError 로 죽는다. 존재 자체를 명시 검사한다.
grep -rq "class DSparkProposer" "$DST/vllm/v1/spec_decode/dspark.py" \
    || { echo "$TAG FAIL: v1/spec_decode/dspark.py 에 DSparkProposer 부재" >&2; exit 1; }

# 드리프트 최소추출이 실제로 자리에 있는가(없으면 dspark.py 가 import 단계에서 죽는다).
grep -rq "class DSparkConfidenceHead" "$DST/vllm/model_executor/models/qwen3_dspark.py" \
    || { echo "$TAG FAIL: DSparkConfidenceHead 최소추출 미적용 — nvidia/dspark.py 가 import 실패한다" >&2; exit 1; }

python3 -m compileall -q \
    "$DST/vllm/models/deepseek_v4" \
    "$DST/vllm/v1/attention/backends/mla" \
    "$DST/vllm/v1/spec_decode" \
    "$DST/vllm/v1/kv_offload" >/dev/null \
    || { echo "$TAG FAIL: 이식본 구문 오류" >&2; exit 1; }

# csrc 가 실제로 바뀌었는지(= _C 재컴파일이 이 이식을 반영하는지) 앵커 확인.
grep -q "VLLM_TOPK_DISABLE_NONCOOP" "$DST/csrc/libtorch_stable/topk.cu" \
    || { echo "$TAG FAIL: topk.cu 에 GB10 non-cooperative 게이트 앵커 부재" >&2; exit 1; }

# ══ requirements/cuda.txt 핀 승격 (2026-08-14 신설 · R2 짝 어긋남 근본해소) ═════
#   why : #20 `pip install -e .` 는 **소스트리의** requirements/cuda.txt 를 읽는다. 45-flashinfer
#         가 0.6.17(python+cubin)을 미리 깔아도, 이 파일이 `==0.6.16.post3` 이면 pip 가 python 을
#         **되돌린다** → cubin 0.6.17 과 짝이 어긋나 import 크래시.
#   ★ cubin 이 함께 되돌아가지 않아 더 위험하다: setup.py 가 cubin 을 install_requires 에서
#     제외하므로(cuda.txt 주석 "flashinfer-cubin is not on PyPI since 0.6.14") 되돌림이 **한쪽만**
#     일어난다. 즉 조용한 부분 되돌림이며, 로그상 45 의 fail-loud 검증은 통과한 뒤에 벌어진다.
#   how: PR#41834 의 requirements 델타는 이 두 줄의 버전 bump 다. 파일을 통째로 vendoring 하지
#        않고 **두 줄만** 고친다 — PR base(main) 의 cuda.txt 는 v0.27.x 대비 다른 줄에도 드리프트가
#        있고(tilelang·quack 등), 통째 채택은 그 드리프트를 조용히 끌고 들어온다.
#        apache-tvm-ffi 는 0.1.11 유지(PR 주석: 0.1.13.post0 은 tilelang 빌드와 ABI 비호환).
#   45 와의 역할 분담: 45 = constraint 핀 제거 + upstream 인덱스에서 실제 설치(cubin 은 PyPI 부재라
#        --extra-index-url 필요). 50 = 그 설치가 #20 에서 되돌려지지 않도록 **입력을 고정**.
FI_VER="0.6.17"
REQ="$DST/requirements/cuda.txt"
[ -f "$REQ" ] || { echo "$TAG FAIL: $REQ 부재 — ${BASE_TAG} 소스 레이아웃이 바뀌었다" >&2; exit 1; }

for pkg in flashinfer-python flashinfer-cubin; do
    grep -qE "^${pkg}[[:space:]]*==" "$REQ" \
        || { echo "$TAG FAIL: $REQ 에 '${pkg}==' 핀 부재 — 승격 전제가 깨졌다(스코프 재판정 필요)" >&2; exit 1; }
done

echo "$TAG requirements/cuda.txt 핀 승격 전:"
grep -nE "^flashinfer-" "$REQ" | sed "s/^/$TAG   /"
sed -i -E "s/^(flashinfer-(python|cubin))[[:space:]]*==[^;[:space:]]+/\1==${FI_VER}/" "$REQ"
echo "$TAG requirements/cuda.txt 핀 승격 후:"
grep -nE "^flashinfer-" "$REQ" | sed "s/^/$TAG   /"

# fail-loud: 승격 후 flashinfer 계열 중 0.6.17 이 아닌 줄이 하나라도 남으면 중단.
STALE="$(grep -E "^flashinfer-" "$REQ" | grep -v "==${FI_VER}" || true)"
[ -z "$STALE" ] || { echo "$TAG FAIL: 승격 누락 — $STALE" >&2; exit 1; }

# 같은 핀이 다른 requirements 파일에 또 있으면 그쪽이 #20 에서 이길 수 있다 → 함께 승격.
while IFS= read -r other; do
    [ "$other" = "$REQ" ] && continue
    echo "$TAG 부수 승격: ${other#"$DST"/}"
    sed -i -E "s/^(flashinfer-(python|cubin))[[:space:]]*==[^;[:space:]]+/\1==${FI_VER}/" "$other"
done < <(grep -rlE "^flashinfer-(python|cubin)[[:space:]]*==" "$DST/requirements" 2>/dev/null || true)

echo "$TAG OK — SM12x Triton sparse-MLA + DSpark 이식 적용(컴파일 전). 최종중재 = serve 스모크."
