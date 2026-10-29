#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
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
