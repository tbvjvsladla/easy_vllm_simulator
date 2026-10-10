#!/bin/bash
# ============================================================================================================
#  DRAFT(2026-10-04 · 커널 프로파일 후속): 서빙 효과·engaged 는 GPU 부팅 + 프로파일이 판정한다.
# ============================================================================================================
# 91-mxfp8-prefer-b12x.sh - **소스 이식 패치**(build_patches_src · Python 편집 · 재컴파일 불요) · dense MXFP8 GEMM 을 b12x 로
#
# what : vllm/model_executor/kernels/linear/mxfp8/flashinfer.py 의 FlashInferCutlassMxfp8LinearKernel 이 mm_mxfp8 에
#        backend="cutlass" 를 하드코딩한다 → sfxnz 저자 스크립트 docker/patch/prefer_b12x_mxfp8.py 를 핀 커밋(3de91468)에서
#        받아 **그대로 실행**(backend="auto": SM120/121 에서 b12x small-M 타일 우선 · 요건 실패 시 cutlass 폴백).
#        그 위에 우리 편집 1개: backend 를 `_exl3port_mxfp8_backend()` 로 감싸 serve-time A/B 손잡이를 둔다 —
#          EXL3PORT_MXFP8_BACKEND=auto(기본 = 저자 동작)|cutlass(stock)|b12x(강제) · 첫 호출 1회 engaged 로그.
# why  : 커널 프로파일 p1-a-prof(K2 구성 · rank0 30스텝): flashinfer DeviceGemmMxfp8GemmSm120 128x32 cutlass 커널이
#        스텝당 54 ms(231 호출 · 평균 234 us) = 89 ms 스텝의 61%. 저자 b12x 실측(flags.md dense_gemv 행, M=4 cold median):
#        qkv_a 46 · wq_b 97 · wo_b 101 · gate_up 57 · down 30 · lm_head 1517 us — 우리 cutlass 132 · 310 · ~189 · 167 · ~189 · 3532.
#        참조 런타임 로그: `dsv41-patch ok prefer_b12x_mxfp8` · `lm_head mxfp8 enabled (b12x, ...)`.
# engaged 증거 : `exl3-port: mm_mxfp8 backend=auto` (랭크당 1회) + 프로파일에서 cutlass DeviceGemmMxfp8GemmSm120 커널 소멸.
# provenance : sources.sfxnz_b12x(커밋·트리 digest 는 sources.sfxnz_p2b 와 동일해야 함). MIT LICENSE → /opt/exl3-port/licenses/sfxnz-b12x/.
# idempotent : backend=_exl3port_mxfp8_backend() + helper 가 있으면 fetch 없이 no-op · 부분 편집이면 die.
set -euo pipefail

TAG="[91-mxfp8-prefer-b12x]"

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
M = "# [exl3-port:mxfp8-prefer-b12x]"
REL = "vllm/model_executor/kernels/linear/mxfp8/flashinfer.py"
SCRIPT = "docker/patch/prefer_b12x_mxfp8.py"

P = L.prov()
S = P["sources"]
if "sfxnz_b12x" not in S:
    L.die(TAG, "PROVENANCE.json lacks sources.sfxnz_b12x")
W, X = S["sfxnz_b12x"], S["sfxnz_p2b"]
if W.get("chain") != [SCRIPT] or set(W.get("files_sha256", {})) != {SCRIPT}:
    L.die(TAG, "sources.sfxnz_b12x chain/files_sha256 disagree with the script this patch runs")
if W["commit"] != X["commit"] or W["tree_digest"] != X["tree_digest"]:
    L.die(TAG, "sources.sfxnz_b12x must pin the same sfxnz commit/tree as sources.sfxnz_p2b")

target = L.VLLM_SRC / REL
if not target.is_file():
    L.die(TAG, f"target missing: {target}")
text = target.read_text()

AUTHOR_NEW = '            backend="auto",\n        )'
OURS = f'            backend=_exl3port_mxfp8_backend(),  {M}\n        )'
FOOT_MARK = f"{M} helper"
FOOTER = f'''

{FOOT_MARK}
_EXL3PORT_MXFP8_BACKEND = None


def _exl3port_mxfp8_backend():
    """EXL3PORT_MXFP8_BACKEND=auto (default, sfxnz prefer_b12x) | cutlass (stock) | b12x; logged once per process."""
    global _EXL3PORT_MXFP8_BACKEND
    if _EXL3PORT_MXFP8_BACKEND is None:
        import os

        b = os.environ.get("EXL3PORT_MXFP8_BACKEND", "auto")
        if b not in ("auto", "cutlass", "b12x"):
            raise ValueError(f"EXL3PORT_MXFP8_BACKEND={{b!r}} (expected auto|cutlass|b12x)")
        _EXL3PORT_MXFP8_BACKEND = b
        print(f"exl3-port: mm_mxfp8 backend={{b}}", flush=True)
    return _EXL3PORT_MXFP8_BACKEND
'''

if OURS in text and FOOT_MARK in text:
    print(f"{TAG} already applied - no-op")
    L.status("applied", "already present (no-op rerun)")
    raise SystemExit(0)
if OURS in text or FOOT_MARK in text or 'backend="auto",\n        )' in text:
    L.die(TAG, f"{REL}: partial/foreign b12x edit present - rebuild from a stock flashinfer.py")
if text.count('            backend="cutlass",\n        )') != 1:
    L.die(TAG, f"{REL}: stock cutlass anchor count != 1 (0.30.0 drifted from the audited shape)")

cache = pathlib.Path(P["install"]["fetch_cache"])
root = cache / f"sfxnz_b12x-{W['commit']}"
L.fetch_pinned_tree(TAG, "sfxnz_p2b", root)
try:
    for rel, want in W["files_sha256"].items():
        if L.sha256_file(root / rel) != want:
            L.die(TAG, f"sfxnz {rel} sha256 mismatch")
    for f, want in W["license"]["license_sha256"].items():
        if L.sha256_file(root / f) != want:
            L.die(TAG, f"sfxnz {f} sha256 mismatch")
    before = L.sha256_file(target)
    # 저자 스크립트는 TREE 인자로 vllm 패키지 루트를 받는다(REL = model_executor/...).
    r = subprocess.run([sys.executable, str(root / SCRIPT), str(L.VLLM_SRC / "vllm")], capture_output=True, text=True)
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        L.die(TAG, f"sfxnz {SCRIPT} failed rc={r.returncode}: {(r.stderr or r.stdout).strip()[-600:]}")
    if L.sha256_file(target) == before:
        L.die(TAG, f"sfxnz {SCRIPT} was a no-op on a stock flashinfer.py")
    print(f"{TAG} {SCRIPT} applied")
    lic = pathlib.Path(P["install"]["licenses_dir"]) / "sfxnz-b12x"
    shutil.rmtree(lic, ignore_errors=True)
    lic.mkdir(parents=True)
    for f in W["license"]["license_sha256"]:
        (lic / f).write_bytes((root / f).read_bytes())
    (lic / "NOTICE").write_text(f"sfxnz/{W['repo'].split('/')[-1]} @ {W['commit']}: MIT (LICENSE) - prefer_b12x_mxfp8.py "
                                f"rewrote {REL}.\n")
finally:
    shutil.rmtree(root, ignore_errors=True)
    (cache / f"sfxnz_p2b-{W['commit']}.tar.gz").unlink(missing_ok=True)

report = L.apply_edits(TAG, REL, [Edit("backend-gate", AUTHOR_NEW, OURS)], FOOTER, FOOT_MARK)
print(f"{TAG} {REL}: {', '.join(report)}")
L.py_compile(TAG, [target])
t = target.read_text()
if t.count(OURS) != 1 or t.count(FOOT_MARK) != 1 or 'backend="cutlass",\n        )' in t or 'backend="auto",\n        )' in t:
    L.die(TAG, "post-assert: gate call site/helper count != 1 or an ungated backend literal remains")
L.status("applied", f"sfxnz prefer_b12x_mxfp8 @ {W['commit'][:12]} + exl3-port backend gate")
print(f"{TAG} OK")
PY
