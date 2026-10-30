#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[70-exllamav3-build]"

: "${SM12X_PORT:=0}"
if [ "${SM12X_PORT}" != "1" ]; then
    echo "$TAG skip - SM12X_PORT=${SM12X_PORT} (stock 빌드 경로 불변)"
    if [ -n "${EASY_VLLM_PATCH_STATUS:-}" ]; then printf 'skipped\tSM12X_PORT!=1\n' > "$EASY_VLLM_PATCH_STATUS"; fi
    exit 0
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LIBDIR=""
for d in "${EXL3_PORT_LIBDIR:-}" "$HERE/../build_patches_src" /tmp/build_patches_src; do
    if [ -n "$d" ] && [ -f "$d/exl3_port_lib.py" ] && [ -f "$d/PROVENANCE.json" ]; then LIBDIR="$(cd "$d" && pwd)"; break; fi
done
[ -n "$LIBDIR" ] || { echo "$TAG FAIL: exl3_port_lib.py / PROVENANCE.json 를 찾지 못했다(build_patches_src 와 함께 배달돼야 한다)" >&2; exit 1; }
export PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import os, pathlib, shutil, subprocess, sys
import exl3_port_lib as L

TAG = sys.argv[1]
P = L.prov()
inst = P["install"]
S = P["sources"]
opt = pathlib.Path(inst["opt_dir"]); opt.mkdir(parents=True, exist_ok=True)
src = pathlib.Path(inst["exllamav3_patched_src"])
marker = opt / ".exllamav3-built"
want_marker = f"exllamav3={S['exllamav3']['commit']} tree={S['exllamav3']['tree_digest']['sha256']} mia={S['mia_overlay']['commit']} arch={P['build']['torch_cuda_arch_list']}\n"

def import_ok() -> bool:
    r = subprocess.run([sys.executable, "-c",
        "import torch, exllamav3_ext as e; assert hasattr(e,'exl3_moe') and hasattr(e,'exl3_fat_gemm'); print(e.__file__)"],
        capture_output=True, text=True, cwd="/")
    if r.returncode == 0:
        print(f"{TAG} exllamav3_ext import OK: {r.stdout.strip()}")
    return r.returncode == 0

if marker.is_file() and marker.read_text() == want_marker and src.is_dir() and import_ok():
    print(f"{TAG} already built for these pins - no-op")
    L.status("applied", "already built (no-op rerun)")
    raise SystemExit(0)

# 1) 패치 입력(Mia): 해시 검증 후 스테이지
stage = opt / "fatkernel"
if stage.exists():
    shutil.rmtree(stage)
stage.mkdir(parents=True)
patch_aarch64 = L.mia_file(TAG, "overlay/patch_exl3_ext_aarch64.py")
patch_fat = L.mia_file(TAG, "overlay/patch_exl3_fat_kernel.py")
for name in ("exl3_fat_gemm.cu", "exl3_fat_gemm.cuh", "exl3_fat_moe.cu", "exl3_fat_moe.cuh"):
    shutil.copyfile(L.mia_file(TAG, f"overlay/{name}"), stage / name)

# 2) exllamav3 트리(트리 digest 검증) -> 패치
L.fetch_exllamav3_tree(TAG, src)
ext = src / "exllamav3" / "exllamav3_ext"
for cmd in ([sys.executable, str(patch_aarch64), str(ext)],
            [sys.executable, str(patch_fat), str(ext), str(stage)]):
    r = subprocess.run(cmd)
    if r.returncode != 0:
        L.die(TAG, f"patch step failed rc={r.returncode}: {' '.join(cmd[1:2])}")
for must in ("quant/exl3_fat_gemm.cu", "quant/exl3_fat_moe.cu"):
    if not (ext / must).is_file():
        L.die(TAG, f"fat-kernel injection incomplete: {must}")
if "exl3_fat_gemm" not in (ext / "bindings.cpp").read_text():
    L.die(TAG, "bindings.cpp lacks the fat-gemm pybind entries after patching")

# 3) 빌드 + 설치
env = dict(os.environ)
env["TORCH_CUDA_ARCH_LIST"] = P["build"]["torch_cuda_arch_list"]
env.setdefault("MAX_JOBS", "8")
if not pathlib.Path("/usr/local/cuda/include/cuda_runtime.h").is_file():
    # vLLM 계열 이미지는 CUDA 헤더를 nvidia/cu13 wheel 아래에 둔다(Mia Dockerfile 의 CPATH 분기). NGC 이미지는 /usr/local/cuda 에 있어 해당 없음.
    import site
    cu13 = pathlib.Path(site.getsitepackages()[0]) / "nvidia" / "cu13" / "include"
    if cu13.is_dir():
        for k in ("CPATH", "CPLUS_INCLUDE_PATH", "C_INCLUDE_PATH"):
            env[k] = f"{cu13}" + (":" + env[k] if env.get(k) else "")
print(f"{TAG} building exllamav3 (arch={env['TORCH_CUDA_ARCH_LIST']} MAX_JOBS={env['MAX_JOBS']}) ...", flush=True)
r = subprocess.run([sys.executable, "-m", "pip", "install", "--no-deps", "--no-build-isolation", "--no-cache-dir", "."],
                   cwd=src, env=env)
if r.returncode != 0:
    L.die(TAG, f"pip install exllamav3 failed rc={r.returncode}")
for junk in ("build", "dist"):
    shutil.rmtree(src / junk, ignore_errors=True)
for egg in src.glob("*.egg-info"):
    shutil.rmtree(egg, ignore_errors=True)

# 4) post-assert
if not import_ok():
    L.die(TAG, "post-assert: import torch, exllamav3_ext (exl3_moe + exl3_fat_gemm) failed")
r = subprocess.run([sys.executable, "-c",
    "import importlib.util as u; s=u.find_spec('exllamav3'); assert s and s.submodule_search_locations, 'exllamav3 package missing'; "
    "import pathlib; root=pathlib.Path(list(s.submodule_search_locations)[0]); "
    "assert (root/'modules'/'quant'/'exl3.py').is_file(), 'LinearEXL3 module missing'; print('exllamav3 pkg', root)"],
    capture_output=True, text=True, cwd="/")
if r.returncode != 0:
    L.die(TAG, f"post-assert: exllamav3 package check failed: {r.stderr.strip()}")
print(f"{TAG} {r.stdout.strip()}")
marker.write_text(want_marker)
L.status("applied", f"exllamav3 {S['exllamav3']['version']} @ {S['exllamav3']['commit'][:12]} built for {P['build']['torch_cuda_arch_list']}")
print(f"{TAG} OK")
PY
