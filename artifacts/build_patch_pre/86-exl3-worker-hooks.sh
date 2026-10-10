#!/bin/bash
# 86-exl3-worker-hooks.sh - **소스 이식 패치**(build_patches_src 슬롯 · pre-compile) · 워커 훅: 로드 후 page-cache 반환 + (옵트인) EXL3 pretune  [round 2: 플러그인 대응]
#
# what : vllm/v1/worker/gpu_worker.py 끝에 footer 를 덧붙여 `Worker.compile_or_warm_up_model` 을 감싼다(양자화가 exl3 일 때만):
#          (1) 로드 직후 모델 샤드 파일의 page cache 를 posix_fadvise(DONTNEED)로 반환(env DSV41_DROP_PAGE_CACHE=0 이면 생략)
#          (2) **옵트인**(env DSV41_EXL3_PRETUNE=1 일 때만 · round 2 에서 기본 off): exllamav3 GEMM 형상 pretune. 플러그인 `vllm_exl3.exl3` 가 `pretune_exl3_shapes` 를 **제공할 때만** 호출한다
#              (vcruz305/vllm-exl3 @ d3cfd394 는 이 심볼이 없다 - Mia 계보 전용). 심볼 부재 시 크래시 대신 WARNING 한 줄 후 건너뛴다. DSV41_EXL3_PRETUNE_SIZES="1,2,4,8,16"
#        이 훅 코드는 이 저장소에서 새로 쓴 것이다(Mia patch_memory_log.py 의 *설계*만 참고 · 코드 비복사). 진단 로깅/적응형 empty_cache 는 버렸다.
# why  : (2) exllamav3 는 협력형 trellis GEMM 을 (min(pow2(rows),16),k,n,K,out dtype) 형상을 처음 볼 때 후보를 시간 측정하며 cudaStreamSynchronize 를 한다.
#        CUDA graph capture 안에서는 불법("operation not permitted when stream is capturing", coop_autotune.cu:464 · Mia boot 15 사망). capture 전에 모든 형상을
#        한 번씩 건드려야 한다 -> --enforce-eager 가 아닌 그래프 부팅의 **필수 조건**이다(exl3.py pretune_exl3_shapes docstring).
#        (1) GB10 에서는 GPU 드라이버가 MemFree 에서 할당하고 page cache 는 회수하지 않는다(NVRM NO_MEMORY, MemFree~1GiB/MemAvailable~9GiB 실측) - 로드가
#        남긴 ~99GiB 샤드 page cache 를 즉시 돌려준다. 호스트 안전(UMA) 방어와 같은 방향.
# model-trigger : quant_config.get_name()=="exl3" 인 워커만. 그 외 모델은 원래 메서드를 그대로 호출.
# fail-loud : pretune 가 예외를 던지면 그대로 전파한다(Mia 는 삼키고 "never break the boot" 였으나, 삼키면 뒤 capture 에서 더 모호하게 죽는다).
#        page cache 반환은 부수 최적화라 OSError 를 로그로만 남긴다.
# (fetch 캐시 정리는 round 2 부터 마지막 PRE 스크립트인 87 이 맡는다.)
# ⚠ UNTESTED(GPU): model_runner.model / speculator(드래프트) 접근 경로가 0.30.0 V1/V2 러너에서 맞는지(훅은 model_runner.{model,speculator,drafter}.model 을 방어적으로 찾는다),
#        pretune 가 실제 형상을 찾는지(핸들 0개면 WARNING).
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
