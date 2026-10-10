#!/bin/bash
# 72-exl3-row-store.sh - 빌드-바깥 의존 패치(build_patches 슬롯 · post-compile) · librow_store.so (디스크 기반 Engram 의 pread 행 저장소)
#
# what : Mia overlay/row_store.cpp 를 `g++ -O3 -shared -fPIC -pthread` 로 /opt/exl3-port/librow_store.so 에 빌드한다. 순수 C++(CUDA 불요).
#        83 이 설치하는 engram_file_backend.py 가 ctypes 로 로드한다(경로 우선순위: env EXL3_PORT_ROW_STORE_LIB -> 이 경로).
# why  : file-backed Engram 의 행 조회(safetensors 샤드 pread + 선택적 packed 샤드 + 캐시)의 네이티브 본체. GB10 UMA 에서 47GiB x 2 pinned UVA 를 대체한다.
# model-trigger : EXL3 팩 + 원본 Engram 샤드 디렉터리 서빙. SM12X_PORT=1 변종에서만.
# provenance : row_store.cpp 를 Mia 커밋에서 받아 sha256 검증(PROVENANCE.json). AGPL-3.0(provenance 참조).
# idempotent : 같은 소스 해시로 빌드된 .so 가 있고 심볼이 모두 로드되면 no-op.
# post-assert : `ldd` 로 미해결 의존 없음 + ctypes 로 5개 심볼(row_store_open/range/stats/attach_packed/lookup) 로드.
# ⚠ UNTESTED: 실제 샤드 pread 동작/성능은 GPU 서빙에서만 확인된다(심볼 로드 + 합성 파일 열기 정도까지는 여기서 가능 - REPORT.md 참조).
set -euo pipefail

TAG="[72-exl3-row-store]"

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
[ -n "$LIBDIR" ] || { echo "$TAG FAIL: exl3_port_lib.py / PROVENANCE.json 를 찾지 못했다" >&2; exit 1; }
export PYTHONPATH="$LIBDIR${PYTHONPATH:+:$PYTHONPATH}"

python3 - "$TAG" <<'PY'
import ctypes, pathlib, subprocess, sys
import exl3_port_lib as L

TAG = sys.argv[1]
P = L.prov()
lib = pathlib.Path(P["install"]["row_store_lib"])
opt = pathlib.Path(P["install"]["opt_dir"]); opt.mkdir(parents=True, exist_ok=True)
want = P["sources"]["mia_overlay"]["files_sha256"]["overlay/row_store.cpp"]
marker = opt / ".row-store-built"
SYMS = ("row_store_open", "row_store_range", "row_store_stats", "row_store_attach_packed", "row_store_lookup")

def load_ok() -> bool:
    try:
        h = ctypes.CDLL(str(lib))
        for s in SYMS:
            getattr(h, s)
        return True
    except (OSError, AttributeError) as exc:
        print(f"{TAG} load check failed: {exc}")
        return False

if lib.is_file() and marker.is_file() and marker.read_text().strip() == want and load_ok():
    print(f"{TAG} already built for this source hash - no-op")
    L.status("applied", "already built (no-op rerun)")
    raise SystemExit(0)

cpp = L.mia_file(TAG, "overlay/row_store.cpp")
cmd = ["g++", "-O3", "-shared", "-fPIC", "-pthread", "-o", str(lib), str(cpp)]
print(f"{TAG} {' '.join(cmd)}", flush=True)
r = subprocess.run(cmd)
if r.returncode != 0:
    L.die(TAG, f"g++ failed rc={r.returncode}")
r = subprocess.run(["ldd", str(lib)], capture_output=True, text=True)
print(r.stdout.strip())
if r.returncode != 0 or "not found" in r.stdout:
    L.die(TAG, "post-assert: ldd reports unresolved dependencies")
if not load_ok():
    L.die(TAG, "post-assert: ctypes load / symbol check failed")
marker.write_text(want + "\n")
L.status("applied", f"librow_store.so built ({lib})")
print(f"{TAG} OK")
PY
