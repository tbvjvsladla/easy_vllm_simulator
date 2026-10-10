#!/bin/bash
# ============================================================================================================
#  DRAFT(UNTESTED on GPU · 2026-10-04 · track K2): 정적 검증만 = 0.30.0 트리 사본에서 저자 스크립트 2종이 앵커 일치·
#  py_compile 통과·재실행 no-op 임을 확인(track_k2/selftest_90.log). 서빙 효과·engaged 로그는 GPU 부팅이 판정한다.
# ============================================================================================================
# 90-woa-fp8-einsum.sh - **소스 이식 패치**(build_patches_src 슬롯 · Python 편집이라 재컴파일 불요) · o_proj wo_a 를 fp8 einsum 으로
#
# what : vllm/models/deepseek_v4/nvidia/ops/o_proj.py 에 sfxnz 작성자 스크립트 2종을 **핀 커밋에서 받아 그대로 실행**한다
#        (재구현 ✗ · sources.sfxnz_p2b 와 같은 커밋 3de91468 · 트리 digest + 스크립트별 sha256 = sources.sfxnz_woa).
#          (1) docker/patch/probe_wo_a.py        - 첫 호출 1회 `[wo_a-probe] wo_a.dtype=.. use_fp8=..` (진단 · 동작 불변)
#          (2) docker/patch/fix_o_proj_woa_fp8.py - stage 1 `_woa_try_requant`: bf16 로 풀린 wo_a 를 층이 보존한 e8m0 스케일로
#              e4m3 재양자화(원 바이트의 비트-정확 왕복) → deep_gemm fp8_einsum 경로. 캡처 중 ✗(eager 패스에서 1회) · 더미 einsum
#              자가시험 실패 시 bf16 bmm 유지 + 로그. stage 2 `_woa_prepacked_scale`(DSV41_WOA_PREPACK=1): fp32 스케일을
#              UE8M0 packed 레이아웃으로 층당 1회 변환(비트 동일 자가시험) → 매 호출 transpose_and_pack 제거.
#        그 위에 우리 편집 1개: 재양자화 호출을 `_exl3port_woa_requant_gate` 로 감싼다 —
#          (a) EXL3PORT_WOA_REQUANT=1(기본) · =0 이면 stock(bf16 bmm) = 재빌드 없는 A/B 손잡이
#          (b) einsum_recipe[2] != 32 이면 재양자화하지 않고 1회 경고(저자 자가시험은 recipe (1,1,32) 고정이라 128-블록 레시피에서
#              자가시험은 통과하고 실호출이 어긋날 수 있다 — 0.30.0 attention.py:347 은 wo_a.weight_block_size==[1,32] 일 때만 32).
# why  : 0.30.0 도 0909 와 같다 — is_bmm wo_a 는 init_mxfp8_linear_kernel(bmm) → EmulationMxfp8LinearKernel(k1 로그 "Using
#        EmulationMxfp8LinearKernel") → VLLM_MXFP8_EMULATION_DEQUANT_AT_LOAD(기본 True) 로 로드 시 bf16 화 → o_proj 의
#        use_fp8=False → strided torch.bmm(sm_80 WMMA). 저자 실측(RESULTS E10/E11 · Round 7 트레이스): 288→80.6 us × 40/step
#        (−8.3 ms/step, 프로파일러 부풀림 포함) · 격리 벤치 157→45 us. prepack: 31.75→25.0 us/call · 트레이스 −0.71..−0.81 ms/step.
#        81 헤더의 "A 의 wo_a 는 FP8 -> stock FP8 경로" 전제는 로드-시 dequant 를 놓쳤다 — (1) 의 프로브 로그가 판정한다.
# toggles(serve-time env · DSV41_/EXL3PORT_ 는 .env 의 VLLM_RAY_EXTRA_ENV_VAR_PREFIXES_TO_COPY 로 Ray 워커에 간다):
#        EXL3PORT_WOA_REQUANT=1(기본)|0 · DSV41_WOA_PREPACK=0(저자 코드 기본)|1(저자 run.sh 기본 · 권장) · DSV41_PROBE_WO_A_EVERY=0|1
# engaged 증거 : `[wo_a-probe] wo_a.dtype=torch.bfloat16 ... use_fp8=False`(전제 성립) 다음 `[woa-requant] fp8 einsum engaged: (4, 1024, 4096)`
#        (층마다 · 랭크당 target+draft) · prepack 켜면 `[woa-prepack] packed ue8m0 scale engaged`.
#   반증 : `[woa-requant] fp8 path rejected` · `[woa-prepack] REJECTED` · `exl3-port: woa requant skipped` · 프로브가 use_fp8=True(이미 fp8 = 레버 무의미).
# model-trigger : deepseek_v4 o_proj 공용 경로(V4·V4.1 · flashmla/flashinfer_sparse). wo_a 가 이미 e4m3 면 stage 1 은 no-op.
# 이름이 90 인 이유 : 80~89 가 모두 점유. 정식 Dockerfile 은 *.sh 전부를 정렬 실행하지만 v-probe Dockerfile 의 PRE 루프는 8*.sh 만
#        돈다 → 파생 이미지 Dockerfile.source-build-v7probe 가 이 파일을 명시 실행한다.
# provenance : sources.sfxnz_woa(커밋·트리 digest 는 sources.sfxnz_p2b 와 동일해야 함 — 다르면 die). MIT LICENSE 를
#        /opt/exl3-port/licenses/sfxnz-woa/ 에 둔다(스크립트가 삽입한 코드가 vLLM 트리에 남는다). 받은 트리·tarball 은 실행 직후 삭제.
# idempotent : 세 마커(_PROBE_WO_A_LOGGED · _woa_try_requant · _woa_prepacked_scale) + 우리 게이트가 모두 있으면 fetch 없이 no-op.
#        일부만 있으면(부분/외래 편집) die.
set -euo pipefail

TAG="[90-woa-fp8-einsum]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIBDIR="${EXL3_PORT_LIBDIR:-$HERE}"
export PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import pathlib
import shutil
import subprocess
import sys

import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
M = "# [exl3-port:woa-fp8-einsum]"
REL = "vllm/models/deepseek_v4/nvidia/ops/o_proj.py"
SCRIPTS = ["docker/patch/probe_wo_a.py", "docker/patch/fix_o_proj_woa_fp8.py"]   # 저자 순서(probe 먼저 · fix 헤더 "Apply AFTER probe_wo_a.py")
AUTHOR_MARKS = ["_PROBE_WO_A_LOGGED", "_woa_try_requant", "_woa_prepacked_scale"]

P = L.prov()
S = P["sources"]
if "sfxnz_woa" not in S:
    L.die(TAG, "PROVENANCE.json lacks sources.sfxnz_woa (track_k2/PROVENANCE.sfxnz_woa.fragment.json)")
W, X = S["sfxnz_woa"], S["sfxnz_p2b"]
if W.get("chain") != SCRIPTS or set(W.get("files_sha256", {})) != set(SCRIPTS):
    L.die(TAG, "sources.sfxnz_woa chain/files_sha256 disagree with the scripts this patch runs")
if W["commit"] != X["commit"] or W["tree_digest"] != X["tree_digest"]:
    L.die(TAG, "sources.sfxnz_woa must pin the same sfxnz commit/tree as sources.sfxnz_p2b (one fetch, one pin)")

target = L.VLLM_SRC / REL
if not target.is_file():
    L.die(TAG, f"target missing: {target}")
text = target.read_text()

# 0.30.0 형상 needle (앵커 정밀 일치는 저자 스크립트가 count==1 로 다시 강제한다).
for needle in ("def deep_gemm_fp8_o_proj(", "use_fp8 = wo_a.weight.dtype == torch.float8_e4m3fn",
               "    o_proj_input, o_scale = fused_inv_rope_fp8_quant(", "from vllm.utils.deep_gemm import fp8_einsum"):
    if needle not in text:
        L.die(TAG, f"{REL}: needle {needle!r} absent - 0.30.0 o_proj drifted from the audited shape")
dg = (L.VLLM_SRC / "vllm/utils/deep_gemm.py").read_text()
for needle in ("def fp8_einsum(", "def transform_sf_into_required_layout("):
    if needle not in dg:
        L.die(TAG, f"vllm/utils/deep_gemm.py: {needle!r} absent (stage 1/2 import it at runtime)")

# ---- 우리 편집: 재양자화 게이트(저자 스크립트 실행 뒤 텍스트에 앵커) ----
GATE_OLD = """    if not use_fp8:
        use_fp8 = _woa_try_requant(wo_a, n_groups, o_lora_rank)
"""
GATE_NEW = f"""    if not use_fp8:
        {M} env A/B + 32-block recipe guard around the author's requant
        use_fp8 = _exl3port_woa_requant_gate(wo_a, n_groups, o_lora_rank, einsum_recipe)
"""
E_GATE = Edit("requant-gate", GATE_OLD, GATE_NEW)
FOOT_MARK = f"{M} helper"
FOOTER = f'''

{FOOT_MARK}
_EXL3PORT_WOA_WARNED = False


def _exl3port_woa_requant_gate(wo_a, n_groups, o_lora_rank, einsum_recipe):
    """EXL3PORT_WOA_REQUANT=0 keeps stock bf16 bmm; requant only on the (1, 1, 32) recipe its scales match."""
    import os

    global _EXL3PORT_WOA_WARNED
    if os.environ.get("EXL3PORT_WOA_REQUANT", "1") != "1":
        return False
    if int(einsum_recipe[2]) != 32:
        if not _EXL3PORT_WOA_WARNED:
            _EXL3PORT_WOA_WARNED = True
            print(f"exl3-port: woa requant skipped (einsum_recipe={{tuple(einsum_recipe)}} != (1, 1, 32)); "
                  "bf16 bmm stays", flush=True)
        return False
    return _woa_try_requant(wo_a, n_groups, o_lora_rank)
'''

present = [m for m in AUTHOR_MARKS if m in text]
if len(present) == len(AUTHOR_MARKS) and GATE_NEW in text and FOOT_MARK in text:
    print(f"{TAG} already applied - no-op")
    L.status("applied", "already present (no-op rerun)")
    raise SystemExit(0)
if present:
    L.die(TAG, f"{REL}: partial/foreign woa edit present ({present}) - rebuild from a stock o_proj.py")

# ---- 받기(핀 트리 digest + p2b 8종 sha 는 fetch_pinned_tree 가, woa 2종 sha 는 여기서) + 실행 ----
cache = pathlib.Path(P["install"]["fetch_cache"])
root = cache / f"sfxnz_woa-{W['commit']}"
L.fetch_pinned_tree(TAG, "sfxnz_p2b", root)
try:
    for rel, want in W["files_sha256"].items():
        if L.sha256_file(root / rel) != want:
            L.die(TAG, f"sfxnz {rel} sha256 mismatch")
    for f, want in W["license"]["license_sha256"].items():
        if L.sha256_file(root / f) != want:
            L.die(TAG, f"sfxnz {f} sha256 mismatch")
    for rel in SCRIPTS:
        before = L.sha256_file(target)
        r = subprocess.run([sys.executable, str(root / rel), str(target)], capture_output=True, text=True)
        sys.stdout.write(r.stdout)
        if r.returncode != 0:
            L.die(TAG, f"sfxnz {rel} failed rc={r.returncode}: {(r.stderr or r.stdout).strip()[-600:]}")
        if L.sha256_file(target) == before:
            L.die(TAG, f"sfxnz {rel} was a no-op on a stock o_proj.py (its idempotency short-circuit fired)")
        print(f"{TAG} {rel} applied")
    lic = pathlib.Path(P["install"]["licenses_dir"]) / "sfxnz-woa"
    shutil.rmtree(lic, ignore_errors=True)
    lic.mkdir(parents=True)
    for f in W["license"]["license_sha256"]:
        (lic / f).write_bytes((root / f).read_bytes())
    (lic / "NOTICE").write_text(
        f"sfxnz/{W['repo'].split('/')[-1]} @ {W['commit']}: MIT (LICENSE) - probe_wo_a.py + fix_o_proj_woa_fp8.py "
        f"inserted code into vllm/{REL.split('vllm/', 1)[1]}.\n")
finally:
    shutil.rmtree(root, ignore_errors=True)
    (cache / f"sfxnz_p2b-{W['commit']}.tar.gz").unlink(missing_ok=True)

report = L.apply_edits(TAG, REL, [E_GATE], FOOTER, FOOT_MARK)
print(f"{TAG} {REL}: {', '.join(report)}")
L.py_compile(TAG, [target])
t = target.read_text()
for m in AUTHOR_MARKS:
    if m not in t:
        L.die(TAG, f"post-assert: author marker {m} missing")
if t.count("use_fp8 = _exl3port_woa_requant_gate(") != 1 or t.count(FOOT_MARK) != 1:
    L.die(TAG, "post-assert: gate call site or helper count != 1")
if "use_fp8 = _woa_try_requant(" in t or t.count("return _woa_try_requant(wo_a, n_groups, o_lora_rank)") != 1:
    L.die(TAG, "post-assert: an ungated _woa_try_requant call site remains (or the gate lost its call)")
L.status("applied", f"sfxnz woa probe+requant+prepack @ {W['commit'][:12]} + exl3-port gate")
print(f"{TAG} OK")
PY
