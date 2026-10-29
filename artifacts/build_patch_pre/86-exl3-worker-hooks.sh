#!/bin/bash
# Official vLLM v0.32.0 SM12x port candidate.
# Source pins: sibling build_patches_src/PROVENANCE.json; no previous GPU validation inherited.
# CPU source checks only so far; native ABI/import/GPU health/inference remain required.
# Historical development explanation is retained in pre-v032-buildkit, not current success evidence.
set -euo pipefail

TAG="[86-exl3-worker-hooks]"

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

TAG = sys.argv[1]
MARK = "# [exl3-port:worker-hooks]"
FOOTER = f'''
{MARK}
def _exl3port_install_worker_hooks() -> None:
    import functools
    import glob
    import os
    import time

    orig = getattr(Worker, "compile_or_warm_up_model", None)
    if orig is None:
        raise RuntimeError("Worker.compile_or_warm_up_model missing: vLLM drifted from the audited v0.30.0 layout")
    if getattr(orig, "_exl3port_wrapped", False):
        return

    def _is_exl3(worker) -> bool:
        qc = getattr(worker.vllm_config, "quant_config", None)
        return getattr(qc, "get_name", lambda: "")() == "exl3"

    def _drop_page_cache(worker) -> None:
        if os.environ.get("DSV41_DROP_PAGE_CACHE", "1") == "0":
            return
        model_dir = str(worker.vllm_config.model_config.model)
        n = 0
        for path in glob.glob(os.path.join(model_dir, "*.safetensors")):
            try:
                fd = os.open(path, os.O_RDONLY)
                try:
                    os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
                finally:
                    os.close(fd)
                n += 1
            except OSError as exc:
                logger.warning("exl3-port: fadvise %s failed: %r", path, exc)
        logger.info("exl3-port: dropped page cache of %d shard files", n)

    def _pretune(worker) -> None:
        if os.environ.get("DSV41_EXL3_PRETUNE", "0") != "1":  # opt-in
            return
        import vllm_exl3.exl3 as _exl3  # plugin module (73); the in-tree exl3.py of round 1 no longer exists

        fn = getattr(_exl3, "pretune_exl3_shapes", None)
        if fn is None:
            logger.warning("exl3-port: DSV41_EXL3_PRETUNE=1 but vllm_exl3.exl3 has no pretune_exl3_shapes (plugin is not the Mia lineage); skipped")
            return

        runner = worker.model_runner
        mods = []
        for holder in (runner, getattr(runner, "speculator", None), getattr(runner, "drafter", None)):
            mod = getattr(holder, "model", None) if holder is not None else None
            if mod is not None and all(mod is not seen for seen in mods):
                mods.append(mod)
        sizes = tuple(
            int(s) for s in os.environ.get("DSV41_EXL3_PRETUNE_SIZES", "1,2,4,8,16").split(",") if s
        )
        t0 = time.perf_counter()
        shapes, launches = fn(mods, sizes=sizes)
        if shapes == 0:
            logger.warning("exl3-port: pretune found 0 EXL3 shapes in %d module(s)", len(mods))
        logger.info(
            "exl3-port: EXL3 pretune: %d shapes x %d row buckets = %d launches in %.1fs",
            shapes, len(sizes), launches, time.perf_counter() - t0,
        )

    @functools.wraps(orig)
    def wrapped(self, *args, **kwargs):
        if _is_exl3(self):
            _drop_page_cache(self)
            _pretune(self)
        return orig(self, *args, **kwargs)

    wrapped._exl3port_wrapped = True
    Worker.compile_or_warm_up_model = wrapped


_exl3port_install_worker_hooks()
'''
rel = "vllm/v1/worker/gpu_worker.py"
text = (L.VLLM_SRC / rel).read_text()
if "class Worker(" not in text or "def compile_or_warm_up_model" not in text or "\nlogger = " not in text:
    L.die(TAG, f"{rel}: expected Worker class / compile_or_warm_up_model / module logger not found")
out = L.apply_edits(TAG, rel, [], footer=FOOTER, footer_marker=MARK)
print(f"{TAG} {rel}: {out}")
L.py_compile(TAG, [L.VLLM_SRC / rel])
if (L.VLLM_SRC / rel).read_text().count(MARK) != 1:
    L.die(TAG, "post-assert: footer count != 1")
applied = any(o.endswith(":applied") for o in out)
L.status("applied", "worker hooks footer installed" if applied else "already present (no-op rerun)")
print(f"{TAG} OK")
PY
