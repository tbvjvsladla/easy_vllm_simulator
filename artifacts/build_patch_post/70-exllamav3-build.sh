#!/bin/bash
# 70-exllamav3-build.sh - 빌드-바깥 의존 패치(build_patches 슬롯 · post-compile) · exllamav3 1.4.5 네이티브(aarch64 + fat-kernel, sm_121a)
#
# what : turboderp-org/exllamav3 @ e648f1a131365aae15920073e761a3fa5a527654(= 1.4.5, Mia 가 쓰는 정확한 커밋)을 받아
#          (1) Mia patch_exl3_ext_aarch64.py  : AVX2/AVX-512 CPU 타깃·x86 ia32_pause 스텁(aarch64 컴파일 가능화)
#          (2) Mia patch_exl3_fat_kernel.py   : fat-GEMM/MoE v1 소스 주입 + pybind 항목
#        를 적용하고 `TORCH_CUDA_ARCH_LIST=12.1a pip install --no-deps --no-build-isolation .` 로 설치한다(exllamav3_ext + exllamav3 파이썬 패키지).
#        패치된 소스 트리는 /opt/exl3-port/exllamav3-src 에 남긴다(71 의 e3v2 빌드가 그 헤더로 컴파일한다).
# why  : EXL3 양자화 메서드(80)의 네이티브 백엔드. exl3.py 는 exllamav3_ext + exllamav3.modules.quant.exl3/.util 만 필요하다(pip --no-deps: 파이썬 의존 불요).
#        실측(delta report §1): 우리 툴체인(torch 2.13 / CUDA 13.3 / sm_121a / MAX_JOBS=8)에서 154s rc=0, .so 328MB.
# model-trigger : EXL3 체크포인트. SM12X_PORT=1 변종에서만.
# provenance : PROVENANCE.json - exllamav3 tarball 은 커밋 SHA 로 받고 **트리 전체 digest**(614 파일)를 검증한다. Mia 패치 스크립트 3종/커널 4종은 파일별 sha256 검증
#        후 실행/복사한다(공급망 가드 · 불일치 = 빌드 실패). 라이선스: exllamav3 MIT(Turboderp) · Mia 패치/커널 AGPL-3.0(provenance 참조).
# idempotent : /opt/exl3-port/.exllamav3-built 마커(트리 digest + Mia 커밋)가 일치하고 import 가 되면 재빌드 없이 no-op.
# post-assert : `import torch, exllamav3_ext` + exl3_moe / exl3_fat_gemm 심볼 + exllamav3 파이썬 패키지 위치.
# ⚠ UNTESTED(GPU): .so 의 런타임 정합(sm_121a 커널 실행)은 GPU 부팅으로만 확인된다. 여기서는 컴파일·링크·import·심볼만 증명한다.
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
