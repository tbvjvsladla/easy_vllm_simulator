#!/usr/bin/env python3
"""blackbox_thermal.py -- 열·전력 포락선 트립 엔진 (plan_26082319 §5.2 · §6.3).

`blackbox_eta.py`(RAM 축)의 **자매 파일**이다. 역할 분리도 같다:
  - 이 스크립트 = **오프라인·주기 실행**. 실측 로그로 임계를 검증하고 상수를 emit 한다.
  - 1 초 핫루프(`thermal_watchdog.sh`) = emit 된 상수를 source 해 **정수 산술만** 수행.
학습(추론)은 여기서, 실행(판정)은 저기서 -- 섞지 않는다.

★ 왜 새 축이 필요한가 (2026-08-23 하드 락업)
  R6 셧다운은 RAM OOM 이 **아니었다** -- MemAvailable 은 19.3 GiB 로 평탄했고, ETA 워치독은
  정상적으로 미발동했다. 커널이 통째로 멈췄고(softlockup/hung_task 둘 다 미발동 = 인터럽트까지
  정지), 이 플랫폼은 NMI 하드워치독이 **영구 비활성**이라 커널 내부 감지기도 없다.
  즉 **RAM 축 방어는 이 사건에 대해 원리적으로 무력**하다. 축이 하나 더 필요하다.

  ⚠ 이 트립은 "가용성 대 안전"의 교환이 **아니다**. 이 하드웨어는 지속 고전력을 견디지 못하고
    (외부 보고: GB10 `Hard power-off under sustained GPU load at ~90W`, 펌웨어 갱신 후에도 잔존),
    그 부하는 어차피 죽는다. 트립은 **회복 불가능한 하드다운을 회복 가능한 컨테이너 kill 로
    바꾸는 것**이다. 같은 작업이 죽는 것은 같지만, 수동 재부팅 13 분이 사라진다.

★ 판정 규칙 -- **지속성**이 판정의 전부다 (누설 버킷)

    poll 마다:  값 >= 임계 ?  bucket = min(cap, bucket+1)
                            :  bucket = max(0,   bucket-decay)
    TRIP  <=>  bucket >= sustain_s

  왜 연속 스트릭이 아니라 누설 버킷인가: 사건 구간의 전력은 88~92 W 를 **진동**하며, 88 W
  연속 스트릭은 최장 44 초에서 끊긴다(1 초 dropout 이 상시 발생). 연속 스트릭으로 잡으려면
  임계를 44 초 밑으로 내려야 하고 그러면 평시 버스트에 닿는다. 누설 버킷은 dropout 을 흡수하면서
  **순간 버스트는 흡수하지 않는다**(버스트가 끝나면 같은 속도로 빠진다).

  `cap = 2 x sustain_s` 는 **파생값**이다(손으로 적지 않는다). 상한이 없으면 장시간 부하 뒤
  버킷이 수천까지 쌓여 부하가 끝난 뒤에도 수천 초 동안 트립 상태로 남는다 -- 회복이 유계여야 한다.

★ 임계값의 출처 (헌법 §결정론 규율 -- 값 옆에 출처를 둔다)

  `PARAM_PROVENANCE` 가 파라미터마다 출처를 명시한다. 두 종류가 섞여 있고 **강도가 다르다**:

  (a) `measured` -- GPU 전력 축. 이 노드 `docs/logs/main/samples/` **23 일 1 Hz 전수**
      (약 195 만 샘플, 2026-07-31~08-23)로 그리드 탐색해 고른 값이다.
        · >=80 W 누설버킷(decay=1) 최대치: 생존 22 일 **13** (2026-08-01) vs 사건일 **309**
        · 임계 60 초 = 두 값의 기하평균 부근 -- 평시 최악의 4.6배, 사건 최고치의 1/5.2
        · 재생 결과: 생존 22 일 kill **0회**, 사건일 18:24:36 kill(락업 **251 초 전**)
        · 사건일의 15:24 순간 100.79 W 버스트: 버킷 최고 **10/60** -> 미발동 (음성대조)

  (b) `external_report` -- 과거 SoC 열 축의 출처였다. **2026-09-09 전수 교정(measured+EC 팬 커브) 완료.**
      이 노드는 SoC 열을 **오늘부터** 수집하기 시작했으므로(`blackbox_collect.py` 확장)
      과거 분포가 없다. 값은 외부 보고(NVIDIA 개발자 포럼 GB10 하드 파워오프)에서 역산했다:
      "acpitz 88->97.8도 in 5s 로 재현성 있게 하드 파워오프", "thermal_zone0/zone5 acpitz
      최대 96.8도 (그때 GPU 는 87~88도)". 30 일치 rollup 이 쌓이면 (a)와 같은 절차로 교정한다.
      **교정 전까지 이 축의 위양성/위음성은 미지다** -- 이 사실을 숨기지 않는다.

★ 음성정직 -- 이 규칙이 무엇을 **하지 않는가**

  1. **GPU 온도는 트립 축이 아니다.** plan_26082319 §5.2 는 "지속 85도+"를 제안했으나
     **실측이 그것을 반증한다**: 사건일 GPU 온도 최대는 85도(>=85도 지속 1 초)인데, 생존일
     2026-08-05 는 >=85도가 **12 초 연속**, 2026-08-10 은 >=80도가 **70 초 연속**이었다.
     즉 사건일은 여러 생존일보다 **GPU 온도가 더 낮았다**. 이 축으로 두 집단은 분리되지 않으며,
     제안값을 그대로 넣었으면 생존일을 죽이고 사건일은 놓쳤을 것이다. 그래서 **뺐다**.
     (외부 보고와도 정합한다 -- GPU 온도는 SoC 열의 **지연 지표**다.)
  2. **순간 전력 상한이 없다.** 생존일도 98.4 W(08-21) · 99.1 W(08-22) 순간치를 찍었다.
     순간 고전력은 이 하드웨어에서 생존 가능하므로 즉시 트립선을 그을 근거가 없다.
  3. **하드 락업 자체는 여전히 못 잡는다.** 이 워치독도 userland 다. 커널이 얼면 같이 언다.
     이 파일이 담당하는 것은 **예방**(그 지점에 도달하기 전에 부하를 끊는 것)이고,
     **복구**는 SBSA 하드웨어 워치독이 담당한다(`install_host_safety.sh` §5).
     둘은 대체재가 아니라 직렬 계층이다 -- 리셋이 반복되면 그것은 예방 실패 신호다.
  4. RAM 축과 **독립**이다. 어느 한 축이 트립하면 kill 한다(양쪽 충족 요구 아님) --
     하드다운은 어느 축으로든 오기 때문이다(plan §6.3).

종료코드: 0=성공 · 1=인자/입력 오류 · 2=자체시험 실패.
"""
from __future__ import annotations

import argparse
import gzip
import json
import os
import subprocess
import sys

SCHEMA_VERSION = 1

# 부재/판독불가 센티넬. 0 을 쓰면 "0 W" 와 구별되지 않아 조용한 오판이 된다.
ABSENT = -1

# 센서 타당성 밴드. **정본은 `blackbox_collect.py:SOC_TEMP_PLAUSIBLE_C`** 이다 -- 센서 판독의
# 타당성은 수집기의 소관이다. 그런데 설치 시 두 파일은 서로 다른 이름(`easy-vllm-bb-collect` /
# `easy-vllm-bb-thermal`)으로 배치돼 모듈 import 가 성립하지 않으므로 단일 소유가 불가능하다.
# **단일 소유가 불가능하면 교차검증이 차선이다**(workflow.md §결정론 규율) -- `_self_test` 가
# 정본 파일의 리터럴을 파싱해 이 값과 일치하는지 확인한다. 갈라지면 자체시험이 FAIL 한다.
SOC_PLAUSIBLE_C = (-40, 150)

DEFAULTS = {
    # ── GPU 전력 축 (measured) ────────────────────────────────────────────
    "gpu_pwr_w": 80,            # 이 위를 '고부하'로 센다
    "gpu_sustain_s": 60,        # 버킷이 이만큼 차면 TRIP
    # ── SoC 열 축 (calibrated 2026-09-09 · measured 8일 682k samples + EC 팬 커브 + 장애 시그니처) ──
    "soc_warn_c": 97,           # 지속 감시선 = EC 팬 커브 100% 지점(플랫폼 자체 관리 천장) · calibrated 2026-09-09
    "soc_sustain_s": 30,        # 술어 유지: 새 warn 기준 "관리 천장 위 30초 지속" = 진성 이탈만
    "soc_hard_c": 99,           # 즉시 계층(최속 장애 스파이크 종점 97.8 + 마진 · 3-poll 디바운스) · calibrated 2026-09-09
    "soc_hard_polls": 3,        # 즉시 계층의 디바운스(ETA `debounce_polls` 와 같은 개념)
    # ── 공통 ──────────────────────────────────────────────────────────────
    "bucket_decay": 1,          # 임계 미만 poll 당 버킷 감소량
    "poll_interval_s": 1,
    # 수집기 CSV 꼬리가 이보다 오래되면 GPU 축은 **판정하지 않고 stale 을 외친다**.
    # 조용히 '안전'으로 넘기지 않는다 -- 수집기 사망이 곧 무장해제가 되면 안 된다.
    "stale_after_s": 10,
}

# ★ 값 옆의 출처(헌법 §결정론 규율). `measured` 와 `external_report` 는 **신뢰 강도가 다르다** --
#   하류(이벤트·리포트·request)가 이 차이를 그대로 표기할 수 있어야 한다.
PARAM_PROVENANCE = {
    "gpu_pwr_w": "measured:docs/logs/main/samples 23일 1Hz 전수(2026-07-31~08-23) 그리드 탐색",
    "gpu_sustain_s": "measured:생존22일 최대버킷 13 vs 사건일 309 의 기하평균 부근",
    "bucket_decay": "measured:decay=1 이 분리비 23.8배로 최대(decay=2 는 23.7, 연속스트릭은 21.8)",
    "soc_warn_c": "calibrated:2026-09-09 — EC 팬 커브 100%@97°C(포럼 377044 펌웨어 복구 · 3버전 동일) + 이 유닛 8일 682k samples(max 97°C·무사고) · testlog_26090904 후속",
    "soc_sustain_s": "calibrated:2026-09-09 — 술어(버킷 지속초)는 유지 · warn 재기준으로 진성 이탈만 걸림 · 장애 시그니처는 급상승(2°C/s)이라 hard 축이 담당",
    "soc_hard_c": "calibrated:2026-09-09 — 장애 스파이크 종점 97.8°C(acpitz) + 마진 · 포럼 하드오프 사례는 센서 결함 진단 동반(정상 관리 상승과 다른 사건)",
    "soc_hard_polls": "inherited:blackbox_eta.DEFAULTS['debounce_polls'] 와 동일 개념(1Hz x 3폴)",
    "bucket_cap_s": "derived:2 x gpu/soc sustain — 회복이 유계이도록(손으로 적지 않는다)",
    "poll_interval_s": "design:수집기와 동일 격자",
    "stale_after_s": "derived:수집기 1Hz 격자 + 여유",
}

# 교정되지 않은 파라미터. `--emit-params` 가 상수 파일에 이 사실을 **적어서** 내보낸다.
UNCALIBRATED = tuple(sorted(k for k, v in PARAM_PROVENANCE.items()
                            if v.startswith("external_report:")))


def effective(params=None):
    """DEFAULTS 위에 override 를 얹은 유효 파라미터. 미지 키는 **조용히 버리지 않는다**(호출부 검증)."""
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    return p


def bucket_cap(sustain_s):
    """버킷 상한(**파생값**). 부하 종료 후 회복이 유계이도록 sustain 의 2배로 둔다."""
    return int(sustain_s) * 2


def new_state():
    """핫루프와 동형의 상태. 셸에서도 같은 세 변수를 쓴다."""
    return {"gpu_bucket": 0, "soc_bucket": 0, "soc_hard_streak": 0}


def step(state, gpu_pwr_dw, soc_temp_c, params=None, gpu_stale=False):
    """한 폴 전진 + 판정. **순수 함수**(state 는 제자리 갱신, 반환은 판정 dict).

    `gpu_pwr_dw` = 데시와트 정수(90.33 W -> 903). 핫루프가 정수 산술만 쓰므로 경계 자체를
    정수로 둔다 -- 파이썬이 실수 경계를 쓰면 두 평면이 경계 부근에서 갈린다(ETA 선례).
    부재/판독불가는 `ABSENT`(-1) 이며 **그 축은 이 폴에서 판정하지 않는다**(0 취급 금지 --
    0 W 는 '아주 안전'을 뜻해서 버킷을 빼 버린다).

    `gpu_stale=True` 면 GPU 축은 버킷을 **동결**한다. 빼면(=안전 방향) 수집기 사망이 조용한
    무장해제가 되고, 더하면(=위험 방향) 위양성이 된다. 동결이 유일하게 정직한 선택이다.
    """
    p = effective(params)
    decay = int(p["bucket_decay"])
    gcap = bucket_cap(p["gpu_sustain_s"])
    scap = bucket_cap(p["soc_sustain_s"])
    gpu_thr_dw = int(p["gpu_pwr_w"]) * 10

    if gpu_stale or gpu_pwr_dw == ABSENT:
        pass                                            # 동결 -- 판정도 갱신도 하지 않는다
    elif gpu_pwr_dw >= gpu_thr_dw:
        state["gpu_bucket"] = min(gcap, state["gpu_bucket"] + 1)
    else:
        state["gpu_bucket"] = max(0, state["gpu_bucket"] - decay)

    if soc_temp_c == ABSENT:
        pass                                            # SoC 부재 플랫폼 -- 이 축은 없는 것이다
    else:
        if soc_temp_c >= int(p["soc_warn_c"]):
            state["soc_bucket"] = min(scap, state["soc_bucket"] + 1)
        else:
            state["soc_bucket"] = max(0, state["soc_bucket"] - decay)
        if soc_temp_c >= int(p["soc_hard_c"]):
            state["soc_hard_streak"] += 1
        else:
            state["soc_hard_streak"] = 0

    # 우선순위 = 급한 것부터. `rule` 이 어느 축이 죽였는지를 이벤트·로그에 남긴다(출처 표시).
    if state["soc_hard_streak"] >= int(p["soc_hard_polls"]):
        rule, trip = "soc_hard_ceiling", True
    elif state["soc_bucket"] >= int(p["soc_sustain_s"]):
        rule, trip = "soc_temp_sustained", True
    elif state["gpu_bucket"] >= int(p["gpu_sustain_s"]):
        rule, trip = "gpu_pwr_sustained", True
    else:
        rule, trip = "none", False
    return {"trip": trip, "rule": rule, "gpu_bucket": state["gpu_bucket"],
            "soc_bucket": state["soc_bucket"], "soc_hard_streak": state["soc_hard_streak"],
            "gpu_stale": bool(gpu_stale)}


def validate_params(params):
    """하한 가드. 위반은 (False, [사유...]) -- 조용한 보정 금지, 거부한다."""
    errs = []
    p = effective(params)
    for k in ("gpu_pwr_w", "gpu_sustain_s", "soc_warn_c", "soc_sustain_s",
              "soc_hard_c", "soc_hard_polls", "bucket_decay", "poll_interval_s",
              "stale_after_s"):
        try:
            v = float(p[k])
        except (TypeError, ValueError):
            errs.append("%s must be numeric (got %r)" % (k, p[k]))
            continue
        if v != int(v):
            errs.append("%s must be an integer (핫루프가 정수 산술만 쓴다) — got %r" % (k, p[k]))
        if v <= 0:
            errs.append("%s must be > 0" % k)
    if not errs:
        if int(p["soc_hard_c"]) <= int(p["soc_warn_c"]):
            errs.append("soc_hard_c(%s) must exceed soc_warn_c(%s) — 즉시 계층이 지속 계층보다 "
                        "낮으면 지속 계층이 조용히 무의미해진다"
                        % (p["soc_hard_c"], p["soc_warn_c"]))
        if int(p["bucket_decay"]) > 1 and int(p["gpu_sustain_s"]) <= int(p["bucket_decay"]):
            errs.append("gpu_sustain_s(%s) must exceed bucket_decay(%s) — 감소량이 임계보다 크면 "
                        "버킷이 임계에 도달할 수 없다" % (p["gpu_sustain_s"], p["bucket_decay"]))
        if int(p["stale_after_s"]) <= int(p["poll_interval_s"]):
            errs.append("stale_after_s(%s) must exceed poll_interval_s(%s) — 정상 격자가 "
                        "상시 stale 로 읽히면 GPU 축이 영구 동결된다"
                        % (p["stale_after_s"], p["poll_interval_s"]))
    return (not errs), errs


def emit_shell_params(params, path):
    """1 초 핫루프가 source 할 셸 상수(`BB_TP_*`). 전력 경계는 **데시와트 정수**로 낸다.

    ETA 의 `eta_params.env` 와 **파일을 나눈다** -- 두 워치독은 수명·활성화·교정 주기가 다르고,
    한 파일에 섞으면 한쪽 재emit 이 다른 쪽 규칙을 조용히 갈아끼운다.
    """
    p = effective(params)
    lines = [
        "# generated by blackbox_thermal.py -- 편집 금지(재생성으로 갱신)",
        "# 규칙: 값>=임계면 bucket+1(상한 cap), 아니면 bucket-decay(하한 0). TRIP <=> bucket>=sustain",
        "# 축은 서로 독립이다 -- 어느 하나라도 TRIP 이면 kill(하드다운은 어느 축으로든 온다).",
        "# ⚠ GPU '온도'는 트립 축이 **아니다**(실측 반증 — blackbox_thermal.py 헤더 §음성정직 1).",
    ]
    for k, v in sorted(p.items()):
        lines.append("# 출처: %s" % PARAM_PROVENANCE.get(k, "unknown"))
        lines.append("BB_TP_%s=%d" % (k.upper(), int(v)))
    lines.append("# 파생값 — 손으로 적지 않는다(출처: %s)" % PARAM_PROVENANCE["bucket_cap_s"])
    lines.append("BB_TP_GPU_PWR_DW=%d" % (int(p["gpu_pwr_w"]) * 10))
    # 센서 타당성 밴드 -- 핫루프가 미초기화 센서값(-274000·2147483647)을 최대값에 섞지 않도록.
    # 정본은 blackbox_collect.py:SOC_TEMP_PLAUSIBLE_C (자체시험이 교차검증한다).
    lines.append("BB_TP_SOC_PLAUSIBLE_MIN_C=%d" % SOC_PLAUSIBLE_C[0])
    lines.append("BB_TP_SOC_PLAUSIBLE_MAX_C=%d" % SOC_PLAUSIBLE_C[1])
    lines.append("BB_TP_GPU_BUCKET_CAP=%d" % bucket_cap(p["gpu_sustain_s"]))
    lines.append("BB_TP_SOC_BUCKET_CAP=%d" % bucket_cap(p["soc_sustain_s"]))
    # 교정 상태를 **데이터에 실어** 내보낸다. 하류가 "이 판정이 무엇에 근거하는가"를 알 수 있어야 한다.
    lines.append("# ⚠ 미교정 파라미터(외부 보고 역산 — 이 노드 실측 분포 없음):")
    for k in UNCALIBRATED:
        lines.append("#     %s = %s" % (k, p[k]))
    lines.append("BB_TP_UNCALIBRATED=\"%s\"" % " ".join(UNCALIBRATED))
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)
    return path


# ── 재생기 (plan_26082319 §8 성공기준 3 = 음성대조 증거 생성기) ───────────
def read_csv_lines(path):
    """원시/압축 samples 를 읽는다. 실패는 None(호출부가 fail-closed 로 처리).

    ★ 정본은 `blackbox_eta.read_csv_lines` 이지만 **import 하지 않는다** -- 두 엔진은 설치 시
      서로 다른 이름(`easy-vllm-bb-eta` / `easy-vllm-bb-thermal`)으로 배치되므로 모듈 경로가
      성립하지 않는다. 같은 내용을 두 번 적는 대신 이 주석으로 결속을 명시한다.
    """
    if path.endswith(".zst"):
        try:
            out = subprocess.run(["zstd", "-dc", path], capture_output=True, timeout=300)
        except (OSError, subprocess.SubprocessError):
            return None
        return out.stdout.decode("utf-8", "replace").splitlines() if out.returncode == 0 else None
    opener = gzip.open if path.endswith(".gz") else open
    try:
        with opener(path, "rt", encoding="utf-8", errors="replace") as fh:
            return fh.read().splitlines()
    except OSError:
        return None


def _cell(parts, idx, key):
    """헤더 이름으로 셀을 집는다. 열이 없거나 비었으면 None."""
    i = idx.get(key)
    if i is None or i >= len(parts):
        return None
    v = parts[i].strip()
    return v or None


def replay_samples(path, params=None):
    """1 Hz samples CSV 를 핫루프와 **같은 상태기계**로 재생해 kill 을 센다.

    ⚠ 한계(음성정직): 첫 kill 이후의 궤적은 **반사실**이다 -- 실제로 kill 됐다면 부하가 끊겨
      그 뒤 샘플은 존재하지 않았을 것이다. 판정에 쓰는 값은 `first_kill` 이고, `kills` 는
      "이 궤적을 끝까지 재생했을 때의 발동 횟수"다(ETA 재생기와 같은 규약).

    ★ 헤더가 **중간에 바뀌는 파일**(스키마 드리프트 경계)도 처리한다 -- 헤더 줄을 만나면 색인을
      다시 만든다. 그러지 않으면 확장일의 재생이 조용히 틀린 열을 읽는다.
    """
    lines = read_csv_lines(path)
    if not lines:
        return None
    idx = {}
    st = new_state()
    p = effective(params)
    rows = kills = 0
    first_kill = None
    peak_gpu = peak_soc = 0
    max_pwr_dw = -1
    max_soc = -1
    have_soc = False
    for line in lines:
        parts = line.split(",")
        if parts and parts[0].strip() == "ts":
            idx = {k.strip(): i for i, k in enumerate(parts)}   # (재)헤더 -- 색인 갱신
            continue
        if not idx:
            continue
        ts = _cell(parts, idx, "ts")
        if ts is None or not ts.isdigit():
            continue
        w = _cell(parts, idx, "gpu_pwr")
        s = _cell(parts, idx, "soc_temp")
        try:
            pwr_dw = int(round(float(w) * 10)) if w is not None else ABSENT
        except ValueError:
            pwr_dw = ABSENT
        try:
            soc_c = int(float(s)) if s is not None else ABSENT
        except ValueError:
            soc_c = ABSENT
        have_soc = have_soc or soc_c != ABSENT
        rows += 1
        max_pwr_dw = max(max_pwr_dw, pwr_dw)
        max_soc = max(max_soc, soc_c)
        r = step(st, pwr_dw, soc_c, p)
        peak_gpu = max(peak_gpu, r["gpu_bucket"])
        peak_soc = max(peak_soc, r["soc_bucket"])
        if r["trip"]:
            kills += 1
            if first_kill is None:
                first_kill = {"ts": int(ts), "rule": r["rule"],
                              "gpu_pwr_w": round(pwr_dw / 10.0, 2) if pwr_dw != ABSENT else None,
                              "soc_temp_c": soc_c if soc_c != ABSENT else None}
            st = new_state()                # kill 은 부하를 끊는다 -- 버킷도 함께 리셋
    return {"path": path, "rows": rows, "kills": kills, "first_kill": first_kill,
            "peak_gpu_bucket": peak_gpu, "peak_soc_bucket": peak_soc,
            "gpu_sustain_s": int(p["gpu_sustain_s"]), "soc_sustain_s": int(p["soc_sustain_s"]),
            "max_gpu_pwr_w": round(max_pwr_dw / 10.0, 2) if max_pwr_dw != ABSENT else None,
            "max_soc_temp_c": max_soc if max_soc != ABSENT else None,
            # SoC 열이 없는 파일에서 "SoC 축 kill 0회" 는 **안전의 증거가 아니라 데이터 부재**다.
            "soc_axis_evaluated": have_soc}


def _self_test():
    import tempfile
    checks = []

    def chk(name, cond):
        checks.append((name, bool(cond)))

    # ── 교차검증: 타당성 밴드의 정본은 수집기다 ───────────────────────────
    #   단일 소유가 불가능한 쌍(설치 시 이름이 갈려 import 불가)이므로 리터럴을 대조한다.
    _collector = os.path.join(os.path.dirname(os.path.abspath(__file__)), "blackbox_collect.py")
    _band = None
    try:
        for _ln in open(_collector, encoding="utf-8"):
            if _ln.startswith("SOC_TEMP_PLAUSIBLE_C"):
                _band = eval(_ln.split("=", 1)[1].strip())      # noqa: S307 -- 자기 소유 리터럴
                break
    except OSError:
        _band = "unreadable"
    chk("타당성 밴드가 정본(blackbox_collect.SOC_TEMP_PLAUSIBLE_C)과 일치",
        _band == SOC_PLAUSIBLE_C)

    # ── 교차검증: 핫루프의 내장 기본값이 정본과 갈라지지 않았는가 ─────────
    #   `thermal_watchdog.sh` 는 상수 파일이 없을 때 쓸 **fail-safe 기본값**을 내장한다(부재가
    #   무방비를 뜻하지 않게). 그 사본이 여기 DEFAULTS 와 갈라지면 상수 파일 유무에 따라
    #   판정이 달라진다 -- 조용히. 그래서 리터럴을 대조한다(단일 소유 불가 → 교차검증 차선).
    _sh = os.path.join(os.path.dirname(os.path.abspath(__file__)), "thermal_watchdog.sh")
    _shvals, _mismatch = {}, []
    try:
        for _ln in open(_sh, encoding="utf-8"):
            _ln = _ln.strip()
            if not _ln.startswith("BB_TP_"):
                continue
            for _asgn in _ln.split(";"):
                _asgn = _asgn.strip()
                if not _asgn.startswith("BB_TP_") or "=" not in _asgn:
                    continue
                _k, _v = _asgn.split("=", 1)
                if _k not in _shvals and _v.lstrip("-").isdigit():
                    _shvals[_k] = int(_v)       # 첫 등장 = 내장 기본값(뒤는 자체시험용 재설정)
    except OSError:
        _mismatch.append("thermal_watchdog.sh 판독 불가")
    _expect = {"BB_TP_GPU_PWR_DW": int(DEFAULTS["gpu_pwr_w"]) * 10,
               "BB_TP_GPU_BUCKET_CAP": bucket_cap(DEFAULTS["gpu_sustain_s"]),
               "BB_TP_SOC_BUCKET_CAP": bucket_cap(DEFAULTS["soc_sustain_s"]),
               "BB_TP_SOC_PLAUSIBLE_MIN_C": SOC_PLAUSIBLE_C[0],
               "BB_TP_SOC_PLAUSIBLE_MAX_C": SOC_PLAUSIBLE_C[1]}
    for _k, _v in DEFAULTS.items():
        # `gpu_pwr_w` 는 셸 평면에서 **데시와트 정수**(BB_TP_GPU_PWR_DW)로 대체된다 -- 정수
        # 산술만 쓰는 핫루프가 와트 실수를 들 수 없기 때문이다. 그 쌍은 위에서 이미 대조했다.
        if _k == "gpu_pwr_w":
            continue
        _expect.setdefault("BB_TP_" + _k.upper(), int(_v))
    for _k, _v in sorted(_expect.items()):
        if _shvals.get(_k) != _v:
            _mismatch.append("%s: 셸=%r 정본=%r" % (_k, _shvals.get(_k), _v))
    chk("핫루프 내장 기본값이 정본 DEFAULTS 와 일치%s"
        % ("" if not _mismatch else " — " + "; ".join(_mismatch)), not _mismatch)

    # ── 파생·가드 ─────────────────────────────────────────────────────────
    chk("버킷 상한은 sustain 의 2배(파생)", bucket_cap(60) == 120 and bucket_cap(30) == 60)
    ok, errs = validate_params({})
    chk("기본값은 하한 가드 통과", ok and not errs)
    ok, errs = validate_params({"soc_hard_c": 85})
    chk("즉시선 <= 지속선 -> 거부", not ok and any("soc_hard_c" in e for e in errs))
    ok, _ = validate_params({"gpu_pwr_w": 0})
    chk("0 임계 -> 거부", not ok)
    ok, _ = validate_params({"gpu_sustain_s": 60.5})
    chk("비정수 -> 거부(핫루프는 정수 산술)", not ok)
    ok, _ = validate_params({"stale_after_s": 1})
    chk("stale <= poll -> 거부(GPU 축 영구 동결 방지)", not ok)

    # ── 지속성 판정 ───────────────────────────────────────────────────────
    p = dict(DEFAULTS)
    st = new_state()
    for _ in range(59):
        r = step(st, 900, ABSENT, p)
    chk("59 폴 고전력 -> 아직 미발동(경계 아래)", not r["trip"] and r["gpu_bucket"] == 59)
    r = step(st, 900, ABSENT, p)
    chk("60 폴 고전력 -> TRIP(rule=gpu_pwr_sustained)",
        r["trip"] and r["rule"] == "gpu_pwr_sustained")

    st = new_state()
    for _ in range(10):
        r = step(st, 1008, ABSENT, p)       # 100.8 W 순간버스트 10 초 (사건일 15:24 실측)
    chk("★음성대조: 100.8W 10초 버스트 -> 미발동", not r["trip"] and r["gpu_bucket"] == 10)
    for _ in range(10):
        r = step(st, 130, ABSENT, p)        # 부하 종료(13 W)
    chk("버스트 종료 후 버킷 배출", r["gpu_bucket"] == 0)

    # 경계값: 임계 정확히 = 고부하로 센다(>=)
    st = new_state()
    r = step(st, 800, ABSENT, p)
    chk("80.0W 경계는 고부하(>=)", r["gpu_bucket"] == 1)
    st = new_state()
    r = step(st, 799, ABSENT, p)
    chk("79.9W 는 고부하 아님", r["gpu_bucket"] == 0)

    # dropout 내성: 88~92W 진동 중 1 초 dropout 이 버킷을 리셋하지 않는다(연속 스트릭과의 차이)
    st = new_state()
    for i in range(80):
        r = step(st, 300 if i % 8 == 7 else 900, ABSENT, p)     # 8 초마다 1 초 저전력
    chk("dropout 진동에도 지속으로 인식(연속 스트릭이었으면 미발동)", r["trip"])

    # 순수 저전력은 아무리 길어도 트립하지 않는다
    st = new_state()
    for _ in range(3600):
        r = step(st, 450, ABSENT, p)
    chk("45W 1시간 -> 미발동", not r["trip"] and r["gpu_bucket"] == 0)

    # ── SoC 축 ────────────────────────────────────────────────────────────
    st = new_state()
    for _ in range(29):
        r = step(st, ABSENT, 96, p)
    chk("SoC 96도(warn 미만) 29폴 -> 미발동", not r["trip"])
    r = step(st, ABSENT, 97, p)
    r2 = None
    for _ in range(29):
        r2 = step(st, ABSENT, 97, p)
    chk("SoC 97도(warn 이상) 30폴 -> TRIP(soc_temp_sustained)",
        r2["trip"] and r2["rule"] == "soc_temp_sustained")
    st = new_state()
    for _ in range(3):
        r = step(st, ABSENT, 99, p)
    chk("SoC 99도(hard 이상) 3폴 -> 즉시계층 TRIP(soc_hard_ceiling)",
        r["trip"] and r["rule"] == "soc_hard_ceiling")
    st = new_state()
    r = step(st, ABSENT, 99, p)
    r = step(st, ABSENT, 60, p)
    r = step(st, ABSENT, 99, p)
    chk("SoC 즉시계층은 연속이어야 한다(끊기면 스트릭 리셋)", not r["trip"])
    st = new_state()
    for _ in range(600):
        r = step(st, ABSENT, 47, p)
    chk("SoC 평시(47도) -> 미발동", not r["trip"] and r["soc_bucket"] == 0)

    # ── 부재·stale 정직성 ─────────────────────────────────────────────────
    st = new_state()
    for _ in range(600):
        r = step(st, ABSENT, ABSENT, p)
    chk("양축 부재 -> 버킷 불변(0 취급 금지)",
        r["gpu_bucket"] == 0 and r["soc_bucket"] == 0 and not r["trip"])
    st = new_state()
    for _ in range(30):
        step(st, 900, ABSENT, p)
    before = st["gpu_bucket"]
    for _ in range(50):
        r = step(st, 900, ABSENT, p, gpu_stale=True)
    chk("stale -> GPU 버킷 동결(더하지도 빼지도 않는다)",
        st["gpu_bucket"] == before and not r["trip"] and r["gpu_stale"])
    st = new_state()
    for _ in range(30):
        step(st, 900, ABSENT, p)
    for _ in range(40):
        r = step(st, ABSENT, ABSENT, p)
    chk("부재로 바뀌어도 축적분이 사라지지 않는다(조용한 무장해제 금지)",
        st["gpu_bucket"] == 30)

    # 축 독립: 어느 하나만 차도 kill (양쪽 충족 요구 아님)
    st = new_state()
    for _ in range(30):
        r = step(st, 100, 97, p)            # GPU 는 10W(한산), SoC 만 뜨겁다
    chk("한 축만 차도 TRIP(축 독립)", r["trip"] and r["rule"] == "soc_temp_sustained")

    # ── emit / 재생 ───────────────────────────────────────────────────────
    with tempfile.TemporaryDirectory() as td:
        env = os.path.join(td, "thermal_params.env")
        emit_shell_params({}, env)
        txt = open(env, encoding="utf-8").read()
        chk("emit: 데시와트 정수 경계", "BB_TP_GPU_PWR_DW=800" in txt)
        chk("emit: 파생 버킷 상한", "BB_TP_GPU_BUCKET_CAP=120" in txt
            and "BB_TP_SOC_BUCKET_CAP=60" in txt)
        chk("emit: 모든 파라미터에 출처 주석", txt.count("# 출처:") == len(DEFAULTS))
        chk("emit: 센서 타당성 밴드 동봉(핫루프가 잡음을 먹지 않게)",
            "BB_TP_SOC_PLAUSIBLE_MIN_C=-40" in txt and "BB_TP_SOC_PLAUSIBLE_MAX_C=150" in txt)
        chk("emit: 미교정 목록을 데이터로 실어 본다(2026-09-09 전수 교정 — 빈 목록이 정답)",
            'BB_TP_UNCALIBRATED=""' in txt)
        chk("emit: GPU 온도가 축이 아님을 상수파일이 밝힌다", "GPU '온도'는 트립 축이" in txt)
        chk("emit: 원자적 교체(임시파일 잔재 없음)",
            not os.path.exists(env + ".tmp"))

        # 재생: 사건 궤적 축약본 -- 고전력 90 폴
        csv = os.path.join(td, "incident.csv")
        with open(csv, "w", encoding="utf-8") as fh:
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,"
                     "load1,ctr_n,soc_temp,soc_zone\n")
            for i in range(90):
                fh.write("%d,19251,0,63,90.33,2457,96,,1.0,1,47,0\n" % (1787477017 + i))
        r = replay_samples(csv)
        chk("재생: 사건 축약본 -> kill 발생", r and r["kills"] >= 1)
        chk("재생: 첫 kill 이 60 폴째(트립레벨)",
            r["first_kill"]["ts"] == 1787477017 + 59)
        chk("재생: 규칙 라벨 보존", r["first_kill"]["rule"] == "gpu_pwr_sustained")
        chk("재생: SoC 축이 평가됐음을 표기", r["soc_axis_evaluated"] is True)

        # 재생: 평시 궤적 -- 10 초 버스트 x 20 회
        csv2 = os.path.join(td, "benign.csv")
        with open(csv2, "w", encoding="utf-8") as fh:
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,"
                     "load1,ctr_n,soc_temp,soc_zone\n")
            t = 1787400000
            for _ in range(20):
                for _ in range(10):
                    fh.write("%d,60000,0,70,96.5,2400,96,,1.0,1,50,0\n" % t); t += 1
                for _ in range(60):
                    fh.write("%d,60000,0,50,13.0,2400,0,,1.0,1,47,0\n" % t); t += 1
        r2 = replay_samples(csv2)
        chk("★음성대조 재생: 반복 순간버스트 -> kill 0회", r2 and r2["kills"] == 0)
        chk("음성대조 버킷 최고치가 트립레벨의 절반 이하",
            r2["peak_gpu_bucket"] * 2 <= r2["gpu_sustain_s"])

        # 옛 스키마(SoC 열 없음) -- SoC 축은 '평가되지 않음'으로 정직 표기
        csv3 = os.path.join(td, "legacy.csv")
        with open(csv3, "w", encoding="utf-8") as fh:
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,"
                     "load1,ctr_n\n")
            for i in range(30):
                fh.write("%d,60000,0,70,45.0,2400,50,,1.0,1\n" % (1787400000 + i))
        r3 = replay_samples(csv3)
        chk("옛 스키마: SoC 축 미평가를 정직 표기(안전의 증거가 아니다)",
            r3 and r3["soc_axis_evaluated"] is False and r3["rows"] == 30)

        # 스키마 드리프트 파일: 중간 헤더에서 색인을 다시 만든다
        csv4 = os.path.join(td, "drift.csv")
        with open(csv4, "w", encoding="utf-8") as fh:
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,"
                     "load1,ctr_n\n")
            fh.write("1787400000,60000,0,70,45.0,2400,50,,1.0,1\n")
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,"
                     "load1,ctr_n,soc_temp,soc_zone\n")
            fh.write("1787400001,60000,0,70,45.0,2400,50,,1.0,1,93,5\n")
        r4 = replay_samples(csv4)
        chk("드리프트 파일: 재헤더 뒤 SoC 열을 올바로 읽는다",
            r4 and r4["rows"] == 2 and r4["max_soc_temp_c"] == 93)

        chk("판독 불가 경로 -> None(조용한 빈 결과 금지)",
            replay_samples(os.path.join(td, "nope.csv")) is None)

    ok = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok = ok and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok else "FAIL", len(checks)))
    return 0 if ok else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="노드블랙박스 열·전력 포락선 트립 엔진")
    ap.add_argument("--emit-params", help="셸 상수 출력 경로 (thermal_params.env)")
    ap.add_argument("--set", action="append", default=[], metavar="K=V",
                    help="파라미터 override (예: --set gpu_sustain_s=90)")
    ap.add_argument("--explain", nargs=2, type=float, metavar=("GPU_PWR_W", "SOC_TEMP_C"),
                    help="주어진 전력·SoC열이 몇 초 지속되면 트립하는지 설명")
    ap.add_argument("--replay", nargs="+", metavar="CSV",
                    help="samples CSV 를 재생해 kill 횟수를 센다(음성대조 증거)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    params = {}
    for kv in args.set:
        if "=" not in kv:
            ap.error("--set 는 K=V 형식: %r" % kv)
        k, v = kv.split("=", 1)
        if k not in DEFAULTS:
            ap.error("알 수 없는 파라미터 %r (가능: %s)" % (k, ", ".join(sorted(DEFAULTS))))
        params[k] = int(float(v))

    ok, errs = validate_params(params)
    if not ok:
        print("[thermal] 하한 가드 위반 — 거부:", file=sys.stderr)
        for e in errs:
            print("  - " + e, file=sys.stderr)
        return 1

    p = effective(params)

    if args.explain:
        w, c = args.explain
        st = new_state()
        polls = 0
        pwr_dw = int(round(w * 10))
        soc_c = int(c)
        limit = 4 * max(int(p["gpu_sustain_s"]), int(p["soc_sustain_s"])) + 10
        res = {"trip": False, "rule": "none"}
        while polls < limit and not res["trip"]:
            res = step(st, pwr_dw, soc_c, p)
            polls += 1
        print(json.dumps({
            "input": {"gpu_pwr_w": w, "soc_temp_c": c},
            "params": p, "provenance": PARAM_PROVENANCE, "uncalibrated": list(UNCALIBRATED),
            "verdict": res,
            "polls_to_trip": polls if res["trip"] else None,
            "note": ("이 입력이 유지되면 %d 초 뒤 TRIP" % polls) if res["trip"]
                    else "이 입력이 유지돼도 TRIP 하지 않는다(%d 폴까지 확인)" % limit,
        }, ensure_ascii=False, indent=2))
        return 0

    if args.replay:
        out, total = [], 0
        for path in args.replay:
            r = replay_samples(path, params)
            if r is None:
                print("[thermal] ⚠ 재생 불가(판독 실패): %s" % path, file=sys.stderr)
                out.append({"path": path, "error": "unreadable"})
                continue
            out.append(r)
            total += r["kills"]
            fk = r["first_kill"]
            print("[thermal] %-22s rows=%-6d maxW=%-6s maxSoC=%-4s peak(gpu/soc)=%d/%d "
                  "kill=%d%s%s"
                  % (os.path.basename(path), r["rows"], r["max_gpu_pwr_w"],
                     r["max_soc_temp_c"] if r["soc_axis_evaluated"] else "n/a",
                     r["peak_gpu_bucket"], r["peak_soc_bucket"], r["kills"],
                     "" if not fk else "  ← 최초 ts=%d rule=%s" % (fk["ts"], fk["rule"]),
                     "" if r["soc_axis_evaluated"] else "  [SoC 축 미평가]"))
        print("[thermal] 합계 kill=%d (gpu>=%dW %ds 지속 · soc>=%dC %ds 지속 · soc>=%dC %d폴)"
              % (total, p["gpu_pwr_w"], p["gpu_sustain_s"], p["soc_warn_c"],
                 p["soc_sustain_s"], p["soc_hard_c"], p["soc_hard_polls"]))
        print(json.dumps({"files": out, "total_kills": total, "params": p,
                          "uncalibrated": list(UNCALIBRATED)},
                         ensure_ascii=False, indent=2))
        return 0

    if args.emit_params:
        emit_shell_params(params, args.emit_params)
        print("[thermal] emit %s (gpu>=%dW %ds 지속 · soc>=%dC %ds 지속 · soc>=%dC %d폴)"
              % (args.emit_params, p["gpu_pwr_w"], p["gpu_sustain_s"], p["soc_warn_c"],
                 p["soc_sustain_s"], p["soc_hard_c"], p["soc_hard_polls"]))
        if UNCALIBRATED:
            print("[thermal] ⚠ 미교정(외부 보고 역산 — 이 노드 실측 분포 없음): %s"
                  % ", ".join(UNCALIBRATED), file=sys.stderr)
        return 0

    ap.error("--explain / --emit-params / --replay / --self-test 중 하나가 필요합니다")


if __name__ == "__main__":
    sys.exit(main())
