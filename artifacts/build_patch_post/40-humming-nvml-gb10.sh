#!/bin/bash
# 40-humming-nvml-gb10.sh — 빌드-바깥 의존 패치(humming-kernels NVML 가드) · upstream-version-watch §4.7 · 헌법 3+1+1
#
# what : humming-kernels 의 GPU 튜닝 휴리스틱이 호출하는 NVML 클록 쿼리 2개를 try/except 로 가드(폴백).
#        파일 humming/utils/device.py:
#          (1) calculate_gpu_bandwidth   → nvmlDeviceGetMaxClockInfo(NVML_CLOCK_MEM)
#          (2) estimate_tensorcore_max_tops → nvmlDeviceGetMaxClockInfo(NVML_CLOCK_SM)
# why  : GB10(Grace-Blackwell 통합 LPDDR5X)는 NVML 로 **메모리/SM 클록을 노출하지 않는다** →
#        nvmlDeviceGetMaxClockInfo 가 NVMLError_NotSupported 로 던진다. 이 호출은 HUMMING MoE 백엔드의
#        process_weights_after_loading(mxfp4.py:748 → fused_humming_moe.py:97 → humming/layer.py:375
#        → tune/base.py → device.py:33) 에서 **블록사이즈 튜닝 휴리스틱**용으로만 쓰인다(정확도 무관).
#        가드 없으면 DeepSeek-V4-Flash 서빙이 가중치 로드(844s, OOM 회피 성공) **직후** EngineCore init 에서
#        크래시(serve 측정 2026-06-28: ray RayTaskError(NVMLError_NotSupported)). bus_width 쿼리는 GB10 서
#        성공(크래시가 그 뒤 mem-clock 라인) → 클록 2개만 가드하면 충분.
# why-build-not-runtime : 런타임 .pth(arm_patch)는 vllm/ray serve 워커에 안 닿음(serve#5 측정·30- 패치 동일근거).
#        site-packages 직접 편집(빌드타임)은 전 워커 프로세스에 균일 → robust.
# scope: GB10/SM12x NVML 한계 가드(아치-enablement). 모델-키잉 ✗(humming 쓰는 모든 모델 공통). 이미지 superset 보존.
# probe: serve 측정 traceback(device.py:33 nvmlDeviceGetMaxClockInfo(NVML_CLOCK_MEM) → NotSupported). 폴백값은
#        튜닝 휴리스틱(compute/memory-bound 판정)용 — 정확도 무관, near-DGX-Spark 추정(LPDDR5X ~273 GB/s · SM ~1.5GHz).
#        실 humming 커널/서빙 = serve 스모크가 최종 중재.
# REF  : humming/utils/device.py:14-72 · plan_2026062818_1(Route B) · 헌법 3+1+1 빌드-바깥 패치 따름정리.
set -e
F=$(python3 -c "import humming.utils.device as d; print(d.__file__)" 2>/dev/null || true)
[ -n "$F" ] && [ -f "$F" ] || { echo "[40-humming-nvml-gb10] FAIL: humming/utils/device.py 미발견 — humming-kernels 설치 전제(포크 빌드)" >&2; exit 1; }

python3 - "$F" <<'PY'
import sys
f = sys.argv[1]; s = open(f).read()
n = 0

# (1) calculate_gpu_bandwidth: NVML_CLOCK_MEM 가드 → GB10 폴백 273 GB/s(반환단위 GB/s).
OLD_MEM = (
    "        mem_clock_mhz = pynvml.nvmlDeviceGetMaxClockInfo(handle, pynvml.NVML_CLOCK_MEM)\n"
    "        return (mem_clock_mhz * 2 * bus_width) / 8 / 1000"
)
NEW_MEM = (
    "        try:\n"
    "            mem_clock_mhz = pynvml.nvmlDeviceGetMaxClockInfo(handle, pynvml.NVML_CLOCK_MEM)\n"
    "        except pynvml.NVMLError:\n"
    "            return 273.0  # [GB10] NVML mem-clock NotSupported → DGX Spark LPDDR5X ~273 GB/s fallback (tuning heuristic only)\n"
    "        return (mem_clock_mhz * 2 * bus_width) / 8 / 1000"
)

# (2) estimate_tensorcore_max_tops: NVML_CLOCK_SM 가드 → GB10 폴백 1500 MHz.
OLD_SM = "        max_clock_mhz = pynvml.nvmlDeviceGetMaxClockInfo(handle, pynvml.NVML_CLOCK_SM)"
NEW_SM = (
    "        try:\n"
    "            max_clock_mhz = pynvml.nvmlDeviceGetMaxClockInfo(handle, pynvml.NVML_CLOCK_SM)\n"
    "        except pynvml.NVMLError:\n"
    "            max_clock_mhz = 1500  # [GB10] NVML SM-clock NotSupported → ~1.5GHz fallback (tuning heuristic only)"
)

already = ("[GB10] NVML mem-clock NotSupported" in s) and ("[GB10] NVML SM-clock NotSupported" in s)
if already:
    print("[40-humming-nvml-gb10] 이미 가드됨 — skip"); sys.exit(0)

if OLD_MEM in s:
    s = s.replace(OLD_MEM, NEW_MEM, 1); n += 1; print("[40-humming-nvml-gb10] OK — NVML_CLOCK_MEM 가드")
else:
    sys.stderr.write("[40-humming-nvml-gb10] FAIL: NVML_CLOCK_MEM 예상 블록 미발견 — humming 상류 변경 의심(false-determinism 차단)\n"); sys.exit(1)

if OLD_SM in s:
    s = s.replace(OLD_SM, NEW_SM, 1); n += 1; print("[40-humming-nvml-gb10] OK — NVML_CLOCK_SM 가드")
else:
    sys.stderr.write("[40-humming-nvml-gb10] FAIL: NVML_CLOCK_SM 예상 라인 미발견 — humming 상류 변경 의심\n"); sys.exit(1)

open(f, "w").write(s)
print(f"[40-humming-nvml-gb10] OK — {n} NVML 클록 쿼리 가드 적용: {f}")
PY

# stale .pyc 무효화(즉시 반영).
find "$(dirname "$F")" -name 'device*.pyc' -delete 2>/dev/null || true

# 검증(fail-loud): import 가능 + 양 가드 문자열 존재.
python3 -c "
import humming.utils.device as d
src = open(d.__file__).read()
assert '[GB10] NVML mem-clock NotSupported' in src, 'mem-clock 가드 미적용'
assert '[GB10] NVML SM-clock NotSupported' in src, 'SM-clock 가드 미적용'
print('[40-humming-nvml-gb10] 검증 OK — humming/utils/device.py NVML 클록 가드 2개 존재')
"
