#!/bin/bash
# ============================================================================================================
#  DRAFT(2026-10-04 · 커널 프로파일 후속 track K5): 정적 검증만 = v9probe 컨테이너 안에서 적용·재실행 no-op·py_compile·
#  .cu 의 sm_121a 컴파일(GPU 없이)을 확인(track_k5/selftest_92.log). 서빙 효과·engaged 는 GPU 부팅 + 프로파일이 판정한다.
# ============================================================================================================
# 92-dense-gemv.sh - **소스 이식 패치**(build_patches_src · Python 편집 + 런타임 모듈 설치 · vLLM 재컴파일 불요) · M<=8 dense GEMV
#
# what : sfxnz 저자 스크립트 docker/patch/dense_gemv.py 를 핀 커밋(3de91468)에서 받아 **그대로 실행**한다
#        (`python3 dense_gemv.py <vllm 패키지 루트>` = 저자 sitecustomize 가 기동 때마다 하던 apply() 를 빌드 때 1회).
#          (1) vllm/model_executor/kernels/linear/mxfp8/flashinfer.py — FlashInferCutlassMxfp8LinearKernel 에 훅 2개
#              (process_weights_after_loading 끝 = prepare · apply_weights 머리 = maybe_apply) + `from dense_gemv import`
#          (2) vllm/models/deepseek_v4/nvidia/ops/o_proj.py — fp8_einsum 앞에 grouped wo_a GEMV 훅(woa_apply)
#        런타임 모듈 2종(dense_gemv.py · dense_gemv_kernel.cu)은 **원문 그대로** /opt/exl3-port/dsv41-patch/ 에 설치(sha256 대조).
#        커널은 첫 armed 층에서 torch cpp_extension 이 nvcc(-gencode arch=compute_121a,code=sm_121a)로 JIT 한다(랭크당 ~1분).
#        우리 편집(유일한 이탈) — 두 파일의 저자 import 블록에 `[exl3-port:dense-gemv]` 헤더 1개씩:
#          (a) sys.path 에 /opt/exl3-port/dsv41-patch 추가(저자는 sitecustomize 가 /opt/dsv41-patch 를 넣는다 — 우리는 sitecustomize 없음)
#          (b) 저자 `except ImportError:` 의 침묵 폴백을 fail-loud 로: DSV41_DENSE_GEMV=1 일 때만 프로세스당 1줄
#              `exl3-port: dense-gemv hooks loaded (<file>, DSV41_DENSE_GEMV=1)` | `... import FAILED (...); stock path stays`.
#        저자 자가시험(층별 M=1..8 × 3분포 비트 대조 vs b12x · 그래프 재생 · wo_a 는 vs fp8_einsum + in-situ)과 폴백은 손대지 않는다.
# why  : 커널 프로파일(rank0 · decode c=1 · DSpark k3): 우리 dense MXFP8 GEMM = flashinfer cutlass 54 ms/step,
#        참조 = dsv41_gemv::gemv_kernel<...> 합계 ≈22 ms/step(qkv_a·wq_b·wo_b·shared gate_up/down·wo_a·lm_head·draft main_proj).
#        저자 flags.md:163 — GEMV 는 b12x 대비 M=4 cold +4~10%/shape(250 GB/s 의 86~94%) · serve 귀속 −1.69 ms/step.
#        (54→22 의 대부분은 91 의 cutlass→b12x 몫이고, 92 는 그 위의 b12x→GEMV 몫이다.)
# 전제 : PRE 91(mm_mxfp8 backend=_exl3port_mxfp8_backend() · 기본 auto=b12x) — 자가시험 기준이 apply_weights(=b12x)라 필수.
#        PRE 90(wo_a fp8 재양자화) — 없으면 wo_a 가 bf16 bmm 이라 woa 훅에 닿지 않는다. 둘 다 없으면 die.
# toggles(serve-time env · DSV41_ 는 .env 의 VLLM_RAY_EXTRA_ENV_VAR_PREFIXES_TO_COPY 로 Ray 워커에 간다):
#        DSV41_DENSE_GEMV=0(저자 코드 기본 · 미설정 = stock = 롤백)|1 · DSV41_DENSE_GEMV_SHAPES=KxN,...,wo_a(빈값 = 전부)
#        DSV41_DENSE_DG_SMALLM 은 0/미설정 유지(같은 호출을 다투면 저자가 거부) · EXL3PORT_MXFP8_BACKEND 는 auto 유지
#        (cutlass 로 두면 자가시험 기준이 cutlass 가 되어 비트 불일치 → 전 shape `; b12x stays`).
# engaged 증거 : `exl3-port: dense-gemv hooks loaded (flashinfer.py, DSV41_DENSE_GEMV=1)` · `... (o_proj.py, ...)`
#        → `[dense-gemv] rank<r> K<k>xN<n> (<name>) dense GEMV armed: KC.. ; self-test M=1..8 x 3 dist bitwise, graph replay ok`
#        → `[dense-gemv] rank<r> wo_a (4x1024x4096) dense GEMV armed: ...` + 프로파일의 dsv41_gemv::gemv_kernel.
#   반증 : `; b12x stays` · `; fp8_einsum stays` · `kernel build failed` · `dense-gemv import FAILED`.
# provenance : sources.sfxnz_gemv(커밋·트리 digest 는 sources.sfxnz_p2b 와 동일해야 함). MIT LICENSE → /opt/exl3-port/licenses/sfxnz-gemv/.
# idempotent : 저자 마커(_dsv41_gemv_) + 우리 헤더가 두 파일에 있고 설치 모듈 sha256 이 일치하면 fetch 없이 no-op · 일부만이면 die.
set -euo pipefail

TAG="[92-dense-gemv]"

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
import importlib.util
import pathlib
import shutil
import subprocess
import sys

import exl3_port_lib as L
from exl3_port_lib import Edit

TAG = sys.argv[1]
M = "# [exl3-port:dense-gemv]"
AMARK = "_dsv41_gemv_"                       # 저자 dense_gemv.MARK (fetch 뒤 모듈 상수와 대조)
SCRIPT = "docker/patch/dense_gemv.py"
KERNEL = "docker/patch/dense_gemv_kernel.cu"
FI_REL = "vllm/model_executor/kernels/linear/mxfp8/flashinfer.py"
OP_REL = "vllm/models/deepseek_v4/nvidia/ops/o_proj.py"

P = L.prov()
S = P["sources"]
if "sfxnz_gemv" not in S:
    L.die(TAG, "PROVENANCE.json lacks sources.sfxnz_gemv")
W, X = S["sfxnz_gemv"], S["sfxnz_p2b"]
if W.get("chain") != [SCRIPT] or set(W.get("files_sha256", {})) != {SCRIPT, KERNEL}:
    L.die(TAG, "sources.sfxnz_gemv chain/files_sha256 disagree with the files this patch runs/installs")
if W["commit"] != X["commit"] or W["tree_digest"] != X["tree_digest"]:
    L.die(TAG, "sources.sfxnz_gemv must pin the same sfxnz commit/tree as sources.sfxnz_p2b")
RT = pathlib.Path(W["runtime_install"]["dir"])
RT_FILES = W["runtime_install"]["files"]       # {repo rel: 설치 파일명}
if set(RT_FILES) != {SCRIPT, KERNEL} or str(RT) != "/opt/exl3-port/dsv41-patch":
    L.die(TAG, "sources.sfxnz_gemv.runtime_install disagrees with the header path this patch inserts")


def shim(where: str, try_line: str) -> str:
    """저자 try 블록 앞에 넣는 헤더: 런타임 모듈 경로 + DSV41_DENSE_GEMV=1 일 때 1회 로드/실패 로그."""
    return f'''
{M} 런타임 모듈 경로(저자는 sitecustomize 가 /opt/dsv41-patch 를 sys.path 에 넣는다) + fail-loud import 로그
import sys as _exl3port_sys

if "{RT}" not in _exl3port_sys.path:
    _exl3port_sys.path.append("{RT}")


def _exl3port_gemv_report(err):
    import os

    if os.environ.get("DSV41_DENSE_GEMV", "0") != "1":
        return
    if err is None:
        print("exl3-port: dense-gemv hooks loaded ({where}, DSV41_DENSE_GEMV=1)", flush=True)
    else:
        print(f"exl3-port: dense-gemv import FAILED ({where}: {{err!r}}); stock path stays", flush=True)

{try_line}'''


def report_edit(label: str, fallback_line: str) -> Edit:
    old = f"except ImportError:\n{fallback_line}"
    new = (f"except ImportError as _exl3port_gemv_err:  {M}\n{fallback_line}"
           f"    _exl3port_gemv_report(_exl3port_gemv_err)\nelse:\n    _exl3port_gemv_report(None)\n")
    return Edit(label, old, new)


FI_TRY = "try:  # dsv41 dense_gemv (DSV41_DENSE_GEMV)\n"
OP_TRY = "try:  # dsv41 dense_gemv (DSV41_DENSE_GEMV): grouped wo_a GEMV\n"
FI_EDITS = [Edit("fi-path-shim", "\n" + FI_TRY, shim("flashinfer.py", FI_TRY)),
            report_edit("fi-import-report", "    _dsv41_gemv_apply = _dsv41_gemv_prepare = lambda *a, **k: None\n")]
OP_EDITS = [Edit("op-path-shim", "\n" + OP_TRY, shim("o_proj.py", OP_TRY)),
            report_edit("op-import-report", "    _dsv41_gemv_woa = lambda *a, **k: False\n")]

fi, op = L.VLLM_SRC / FI_REL, L.VLLM_SRC / OP_REL
for t in (fi, op):
    if not t.is_file():
        L.die(TAG, f"target missing: {t}")
fi_t, op_t = fi.read_text(), op.read_text()

# ---- 선행 패치 의존(91 · 90) ----
if "backend=_exl3port_mxfp8_backend()," not in fi_t:
    L.die(TAG, f"{FI_REL}: PRE 91 (mxfp8-prefer-b12x gate) absent - the GEMV self-test reference must be b12x")
if "_woa_try_requant" not in op_t:
    L.die(TAG, f"{OP_REL}: PRE 90 (woa-fp8-einsum) absent - wo_a would stay on bf16 bmm and never reach the hook")


def runtime_ok() -> bool:
    return all((RT / dst).is_file() and L.sha256_file(RT / dst) == W["files_sha256"][src] for src, dst in RT_FILES.items())


SHIM_HEAD, REPORT_OK = f"{M} 런타임 모듈 경로", "    _exl3port_gemv_report(None)\n"
# 파일별 마커 3개(저자 MARK · 우리 헤더 · 우리 import 보고) + 런타임 설치물 — 전부면 no-op, 하나라도 섞여 있으면 die.
done = {f"{n}:{k}": (v in t) for n, t in (("fi", fi_t), ("op", op_t))
        for k, v in (("author", AMARK), ("shim", SHIM_HEAD), ("report", REPORT_OK))}
done["runtime"] = runtime_ok()
if all(done.values()):
    print(f"{TAG} already applied - no-op")
    L.status("applied", "already present (no-op rerun)")
    raise SystemExit(0)
if any(v for k, v in done.items() if k != "runtime") or M in fi_t or M in op_t:
    L.die(TAG, f"partial/foreign dense-gemv edit present ({done}) - rebuild from v9probe")

cache = pathlib.Path(P["install"]["fetch_cache"])
root = cache / f"sfxnz_gemv-{W['commit']}"
L.fetch_pinned_tree(TAG, "sfxnz_p2b", root)
try:
    for rel, want in W["files_sha256"].items():
        if L.sha256_file(root / rel) != want:
            L.die(TAG, f"sfxnz {rel} sha256 mismatch")
    for f, want in W["license"]["license_sha256"].items():
        if L.sha256_file(root / f) != want:
            L.die(TAG, f"sfxnz {f} sha256 mismatch")
    # 앵커 사전 점검: 저자 모듈(최상위 stdlib 전용)의 상수에서 파생 — 손으로 다시 적지 않는다.
    spec = importlib.util.spec_from_file_location("_sfxnz_dense_gemv", root / SCRIPT)
    D = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(D)
    if D.MARK != AMARK or str(D.REL) != FI_REL.split("vllm/", 1)[1] or str(D.OPROJ_REL) != OP_REL.split("vllm/", 1)[1]:
        L.die(TAG, "author dense_gemv.py MARK/REL/OPROJ_REL disagree with this patch")
    for name, txt, old in (("import", fi_t, D.IMPORT_OLD), ("pwal", fi_t, D.PWAL_OLD), ("apply", fi_t, D.APPLY_OLD),
                           ("oproj import", op_t, D.OPROJ_IMPORT_OLD), ("oproj einsum", op_t, D.OPROJ_CALL_OLD)):
        if txt.count(old) != 1:
            L.die(TAG, f"author anchor '{name}' count={txt.count(old)} (want 1) - 0.30.0 drifted from the audited shape")
    # 저자 스크립트 실행(인자 = vllm 패키지 루트). o_proj 드리프트를 저자는 skip 로그로 넘기지만 우리는 아래에서 die.
    r = subprocess.run([sys.executable, str(root / SCRIPT), str(L.VLLM_SRC / "vllm")], capture_output=True, text=True)
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        L.die(TAG, f"sfxnz {SCRIPT} failed rc={r.returncode}: {(r.stderr or r.stdout).strip()[-600:]}")
    if AMARK not in fi.read_text() or AMARK not in op.read_text():
        L.die(TAG, f"sfxnz {SCRIPT} did not hook both flashinfer.py and o_proj.py (wo_a hook skipped?)")
    print(f"{TAG} {SCRIPT} applied")
    # 런타임 모듈 원문 설치 + 설치본 sha256 재대조.
    shutil.rmtree(RT, ignore_errors=True)
    RT.mkdir(parents=True)
    for src, dst in RT_FILES.items():
        (RT / dst).write_bytes((root / src).read_bytes())
    if not runtime_ok():
        L.die(TAG, f"installed runtime files under {RT} fail sha256")
    lic = pathlib.Path(P["install"]["licenses_dir"]) / "sfxnz-gemv"
    shutil.rmtree(lic, ignore_errors=True)
    lic.mkdir(parents=True)
    for f in W["license"]["license_sha256"]:
        (lic / f).write_bytes((root / f).read_bytes())
    (lic / "NOTICE").write_text(f"sfxnz/{W['repo'].split('/')[-1]} @ {W['commit']}: MIT (LICENSE) - dense_gemv.py hooked "
                                f"{FI_REL} and {OP_REL}; dense_gemv.py + dense_gemv_kernel.cu kept verbatim in {RT}.\n")
finally:
    shutil.rmtree(root, ignore_errors=True)
    (cache / f"sfxnz_p2b-{W['commit']}.tar.gz").unlink(missing_ok=True)

for rel, edits in ((FI_REL, FI_EDITS), (OP_REL, OP_EDITS)):
    print(f"{TAG} {rel}: {', '.join(L.apply_edits(TAG, rel, edits))}")
L.py_compile(TAG, [fi, op, RT / "dense_gemv.py"])

# ---- post-asserts ----
for path, n_hook in ((fi, ("_dsv41_gemv_prepare(self, layer, weight_scale_2d)", "_dsv41_gemv_apply(layer, x, bias)")),
                     (op, ("if not _dsv41_gemv_woa(wo_a, o_proj_input, o_scale, weight_scale, z, einsum_recipe):",))):
    t = path.read_text()
    if t.count(SHIM_HEAD) != 1 or t.count(REPORT_OK) != 1:
        L.die(TAG, f"post-assert: {path.name} path shim / import report count != 1")
    if "except ImportError:\n" in t.split(M, 1)[1].split("_exl3port_gemv_report(None)", 1)[0]:
        L.die(TAG, f"post-assert: {path.name} still has a silent ImportError fallback in the hook block")
    for h in n_hook:
        if t.count(h) != 1:
            L.die(TAG, f"post-assert: {path.name} hook {h!r} count != 1")
# 런타임 모듈은 최상위 stdlib 전용 — 설치 경로에서 import + env 파서 기본값(전 shape + wo_a)을 확인.
chk = subprocess.run([sys.executable, "-c",
                      f"import sys; sys.path.insert(0, {str(RT)!r}); import dense_gemv as d; "
                      "s = d.shapes_from_env({}); assert len(s) == len(d.CONFIGS) + 1 and 'wo_a' in s, s; "
                      "assert not d.enabled_from_env({}); print(len(s))"], capture_output=True, text=True)
if chk.returncode != 0:
    L.die(TAG, f"post-assert: installed dense_gemv import/env parse failed: {(chk.stderr or chk.stdout).strip()[-400:]}")
L.status("applied", f"sfxnz dense_gemv @ {W['commit'][:12]} (runtime {RT}) + exl3-port path/import-report shim")
print(f"{TAG} OK")
PY
