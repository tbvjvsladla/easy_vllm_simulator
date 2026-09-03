#!/usr/bin/env python3
"""blackbox_eta.py -- ETA 산출 엔진 + 포락선 갱신 (plan_26073109 §2.2).

임계를 절대 GiB 가 아니라 **ETA(0 까지 남은 시간)** 로 정의한다. 근거(testlog_26073109):
하강률의 동적 범위가 p50 0.13 ~ max 6,241 MiB/s = **약 48,000배** 라, 단일 절대 임계로는
한쪽에서 늦고 한쪽에서 오발하는 것이 원리적으로 불가피하다.

산식 (사용자 승인 2026-07-31 -- kill 지연 중 추가 소비 항 포함):

    eta_zero_s       = mem_avail_mib / rate_mib_s          # 현 하강률로 0 도달까지
    eta_actionable_s = eta_zero_s - kill_latency_s         # **킬 완료 시점** 기준 실여유
    TRIP  <=>  eta_actionable_s <= detect_margin_s

  kill 지연 항이 필요한 이유: testlog_26073109 §4-3 실측에서 MemAvail 최저값(main 7,472 MiB)이
  임계(10,240)보다 **낮았다** -- 트립 이후 kill 이 완료되기까지 메모리는 계속 떨어진다.
  검출 시점 잔량이 아니라 킬 완료 시점 잔량을 기준으로 여유를 잡아야 한다.

★ 이중규칙 (plan_26081415 C1-A · 2026-08-14) -- 하강률 상한 위에서는 ETA 를 쓰지 않는다.

    max_rate_mib_s = mem_total_mib / runway_s          # 파생. 손으로 적지 않는다.
    rate >= max_rate  ==>  TRIP <=> mem_avail <= abs_band_mib      # 절대 잔량 밴드
    rate <  max_rate  ==>  TRIP <=> mem_avail <= rate * runway     # 기존 ETA 규칙

  근거: ETA 부등식 `mem <= rate x runway` 는 `rate >= mem_total/runway` 구간에서 **잔량 축을
  통째로 삼켜 항상 참**이 된다(plan_26081415 §1.1). 이 노드에선 12,461 MiB/s(runway 10s 기준)이며,
  1 Hz 실샘플 전수 조사 결과 그 구간은 **모델 로드일에만** 출현했다(유휴일 0폴). 즉 그 구간에서
  ETA 는 "대형 모델 로드 = 트립" 과 동치라 판정이 아니라 상수다. R0 에서 정상 로드를 2회 사살했다.

  ⚠ **음성정직 -- 이 규칙이 무엇을 포기하는가**(plan_26081415 §2 설계제약):
    상한 위에서 잔량 밴드로 내려앉으면 **그 구간의 진성도 사실상 포기**한다. 유일한 진성 사례
    (KV 벌룬 74,181@27,893 -> 51,848@22,333 -> 28,686@23,162)는 밴드(10,240)를 한 번도 밟지
    않고 통과하며, 23 GiB/s 에서 10 GiB 구간의 체류시간은 **0.44 초**라 1 Hz 폴링 x 디바운스 3
    으로는 원리적으로 못 잡는다. hard_floor(5,120) 역시 같은 이유로 늦다 -- 검출해도 kill 완료에
    4~6 초가 걸린다(실측). 따라서 **상한 위 진성 방어는 선언 필수화(C1-B/C3)와 earlyoom(4% 최후선)
    에 위임된다.** 이 사실을 헤더에 적어 두라는 것이 plan §2 의 요구였다.
    진성/위양성은 (잔량, 하강률) 평면에서 분리되지 않는다 -- 빠진 정보는 임계값이 아니라
    *"이 하강이 유계인가"* 이고, 그 유일한 공급원이 선언이다(testlog_26073123).

역할 분리(헌법 결정론 기조):
  - 이 스크립트 = **오프라인·주기 실행**. 로그를 읽어 포락선을 갱신하고 상수를 emit 한다.
  - 1 초 핫루프(mem_watchdog.sh) = emit 된 상수를 source 해 **정수 산술만** 수행(파이썬 비의존).
  학습(추론)은 여기서, 실행(판정)은 저기서 -- 섞지 않는다.

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

# 하한 가드: 물리적으로 불가능한 설정을 사람도 에이전트도 넣지 못하게 막는 결정론 바닥.
# 근거: kill 지연보다 짧은 여유를 요구하는 임계는 "발동해도 늦는" 설정이다.
ETA_FLOOR_MULTIPLIER = 1.5

def makedirs_as_ancestor_owner(path, mode=0o775):
    """디렉터리 체인을 만들되 **가장 가까운 기존 조상의 소유자**를 물려준다.

    왜 이 함수가 있나(2026-09-03 · plan_26090317 P3 실측):
        블랙박스 데몬은 root 로 돌고, `--node-dir <project>/docs/logs/<node>` 아래에 쓴다.
        사용자가 프로젝트 경로를 **완전삭제**하면(이 프로젝트의 CI/CD 대리 실험이 정확히 그
        시나리오다) 다음 폴에서 데몬의 `os.makedirs` 가 그 체인을 **root:root 로 재생성**한다.
        그 순간부터 위임 사용자는 프로젝트 경로에 아무것도 쓸 수 없고, 메인의 정착(rsync/git init)이
        구조적으로 막힌다 — 실측에서 wipe 20초 만에 재발했고, 증상은 배달 중간의 rsync 실패라
        원인(소유권)에 도달하기 어렵다. 설치기의 소유권 정렬은 **설치 시점**만 고치므로
        이미 도는 데몬에는 닿지 않는다. 그래서 만드는 자리에서 고친다.

    규칙은 결정론이다: 프로젝트 경로는 **그 부모를 소유한 사람의 것**이지 데몬의 것이 아니다.
    root 가 아니면 chown 을 시도하지 않는다(권한 없음이 정상이며 조용히 넘어간다).
    """
    import os as _os
    path = _os.path.abspath(path)
    missing = []
    probe = path
    while not _os.path.exists(probe):
        missing.append(probe)
        parent = _os.path.dirname(probe)
        if parent == probe:
            break
        probe = parent
    _os.makedirs(path, mode=mode, exist_ok=True)
    if not missing:
        return []
    try:
        st = _os.stat(probe)
        uid, gid = st.st_uid, st.st_gid
    except OSError:
        return missing
    if _os.geteuid() != 0:
        return missing                      # 비-root 는 이미 자기 소유로 만든다
    for d in reversed(missing):
        try:
            if _os.stat(d).st_uid != uid:
                _os.chown(d, uid, gid)
        except OSError:
            pass                            # 소유권 정렬 실패가 수집을 막지는 않는다(로그 우선)
    return missing


def inherit_dir_owner(path, parent):
    """root 데몬이 만든 **파일**의 소유를 그 디렉터리 소유자에게 넘긴다(디렉터리 정렬의 파일판).

    2026-09-03 체크포인트 실측(plan_26090317 P4): `makedirs_as_ancestor_owner` 가 디렉터리 체인은
    위임 사용자 소유로 만들었지만, 그 안에 root 데몬이 `open(path, "a")` 로 만든
    `samples/<날짜>.csv`·`events/.cursors.json` 은 root:root 로 남았다. 부모가 사용자 소유라
    삭제(wipe)는 막지 않으나, 프로젝트 경로 안의 root 소유물은 정확히 그 소유권 트랩의
    잔여 형태다. `blackbox_events._inherit_dir_owner` 가 같은 일을 `events/*.jsonl` 에만 하고
    있었다 — 공유 형제 모듈로 올려 세 파일이 한 규칙을 쓴다.
    비-root 는 no-op(권한 문제가 애초에 없다). 실패는 기록을 막지 않는다.
    """
    import os as _os
    try:
        if _os.geteuid() != 0:
            return False
        st = _os.stat(parent)
        if _os.stat(path).st_uid != st.st_uid:
            _os.chown(path, st.st_uid, st.st_gid)
            return True
    except OSError:
        pass
    return False


DEFAULTS = {
    "kill_latency_s": 4.0,      # testlog_26073109 관측 상한 하단(3~15s, HB 15s 격자로 과대) -- 실측 대체 대상
    "detect_margin_s": 2.0,     # 폴링 간격(1s) + 여유(1s)
    # 연속 N 폴 지속해야 실제 TRIP (사용자 승인 2026-07-31, 옵션 '가').
    # ⚠ 2026-08-18: hy3 위양성(§testlog_26081811 §5.3) 대응으로 3.0 → 8.0 을 **시도했다가 되돌렸다**.
    #   동기는 타당했다 — hy3 의 KV 절대클램프 할당 12 GiB 가 1,741 MiB/s 로 약 7.0s 하강했고
    #   3 폴(=3s) 디바운스가 그것을 "지속"으로 읽어 정상 로드를 사살했다(streak=3 에서 트립).
    #   그러나 **두 요구가 정면 충돌한다**:
    #     (A) 그 KV 할당을 거르려면            → debounce > 7 폴
    #     (B) 진성 폭주(밴드 관통)를 잡으려면  → debounce ≤ 4 폴   ← 자기시험 회귀 픽스처가 강제
    #   교집합이 없다. 그리고 (B) 는 양보 불가다 — **디바운스는 hard_floor_mib 트립에도 적용**되므로
    #   8 폴(8s)이면 20,000 MiB/s 폭주가 노드 전체를 6.2s 에 소진해 최후 방어선을 통과한다.
    #   ∴ 디바운스로는 이 문제를 못 고친다. **처방은 선언된 바닥**이며(아래 decl_* 두 상수)
    #     그것만으로 hy3 는 해소된다. 이 주석은 같은 시도의 재발을 막기 위해 남긴다.
    "debounce_polls": 3.0,
    "agent_act_s": 300.0,       # 에이전트 개입 구간 (ETA 5분)
    "agent_notify_s": 900.0,    # 에이전트 알림 구간 (ETA 15분)
    "min_rate_mib_s": 1.0,      # 이보다 느린 하강은 '정지'로 간주(0 나눗셈·잡음 방지)
    "poll_interval_s": 1.0,
    # ── 선언된 바닥(testlog_26073123 · 2026-08-01) ────────────────────────
    # ETA 규칙은 `잔량÷하강률` 선형 외삽이라 **유계**인 모델 로드 하강을 무계로 읽어
    # 58 GiB 급 모델을 3/3 사살했다. 서빙이 예상 바닥을 선언하면 규칙은 그 아래에서만 무장한다.
    # 두 상수는 워치독에도 같은 기본값이 있으나 **정본은 여기**다 — 나머지 상수와 같은
    # 파이프라인(emit_params)으로 조정 가능해야 캠페인 루프튜닝의 대상이 된다.
    # arm 상한 = 선언바닥 - 이 값. 선언 오차·정상 변동 흡수분.
    #   8192 → 3072 (2026-08-18 · testlog_26081811 §5.3.2, 사용자 승인). 8 GiB 는 실측 대비 과잉이다:
    #   hy3 선언바닥 13,801 vs 실측바닥 13,699 = **오차 102 MiB**. 과잉 여유는 arm 상한을 끌어내려
    #   선언을 **거부당하게** 만들고, 그 결과 무선언(상한=무한대)으로 내몰아 위양성을 부른다.
    "decl_margin_mib": 3072,
    # arm 상한이 이 밑이 되는 선언은 거부(게이트 실명 방지). 16384 → 8192 (동상).
    #   ★ 값 선택 근거 — **층이 겹치게** 둔다. margin 3072 에서 hy3 arm 상한 = 10,729 MiB 이고
    #     이는 협역 워치독 절대임계 10,240(abs_band_mib 와 동일 개념)의 **바로 위**다. 즉 ETA 는
    #     절대층이 손대기 직전 구간까지만 무장하고, 그 아래는 절대층이 받는다.
    #   ⚠ validate_params 가 `decl_min_ceiling_mib > hard_floor_mib(5120)` 를 강제한다 — 그 이하로
    #     내리면 어떤 선언도 거부되지 않아 가드가 가드가 아니게 된다. 8192 는 그 위이면서
    #     바닥 < 11,264 인 선언은 여전히 거부하므로 **거부 권능을 유지**한다.
    #   근거 사건: hy3(예상바닥 13,801 → 옛 상한 5,609 < 16,384)가 거부당해 --no-budget 으로 밀렸고,
    #     무선언 상태의 ETA 가 KV 할당을 외삽해 정상 로드를 사살했다(2026-08-18T02:34:43Z).
    "decl_min_ceiling_mib": 8192,
    # ★ 최후 절대 바닥. min_rate_mib_s 가 만든 구멍을 막는다 -- 0.5 MiB/s 로 천천히 새면
    #   ETA 는 영원히 'green'(rate_below_min)이라 절대 트립하지 않는다. 그 상태로 0 에 도달하면
    #   호스트가 죽는다. 따라서 "느리든 빠르든 이 밑이면 죽인다"는 무조건 바닥이 필요하다.
    #   5 GiB 로 잡은 근거: 실측 MemAvail 최저가 main 7,472 / sub 8,840 MiB 였고(그때 호스트는
    #   살았다), 7/22 오발 지점(10,186)보다는 충분히 낮아 그 오발을 되살리지 않는다.
    "hard_floor_mib": 5120.0,
    # ── 이중규칙의 절대 잔량 밴드(plan_26081415 C1-A) ─────────────────────
    # `rate >= max_rate_mib_s` 구간에서 ETA 대신 쓰는 판정선. **새 숫자를 만들지 않았다** --
    # 이 프로젝트가 이미 갖고 있던 절대 임계(레거시 `host_safety/mem_watchdog.sh` 의
    # `THRESH_MIB="${2:-10240}"`, `agent_guard` 의 `kill.threshold_mib`, `preload_ram_gate`
    # 의 `--floor-mib`)를 그대로 재사용한다. 같은 개념이 여러 파일에 손으로 적혀 있으므로
    # (4종 안티패턴 `매직넘버·결함`) **여기를 이 평면의 정본**으로 두고, self-test 가 레거시
    # 워치독의 리터럴과 교차검증한다(단일 소유가 불가능하면 교차검증이 차선 -- workflow.md).
    "abs_band_mib": 10240,
}

# 이중규칙이 꺼졌음을 뜻하는 셸 표기. 0 = mem_total 미상 = **기존 ETA 규칙 그대로**(fail-safe
# 방향 = 더 죽이는 쪽). 값이 아니라 상태라서 이름을 준다.
MAX_RATE_DISABLED = 0

# ⚠ 이 값들은 **초기 임의값**이다(plan_26073109 §2.2). Plan B 7종 E2E 캠페인이 모델별 실측으로
#   교체한다 -- 여기 박힌 숫자를 확정 사실로 인용하지 말 것.


def runway_s(p):
    """TRIP 판정에 쓸 총 활주로.

    디바운스는 **kill 시작을 (N-1)x폴링 만큼 늦춘다** -- 그 지연을 활주로에 더하지 않으면
    디바운스를 켠 만큼 실효 여유가 줄어든다(늦은 킬). 그래서 활주로에 포함한다.
    """
    return (float(p["kill_latency_s"]) + float(p["detect_margin_s"])
            + (float(p["debounce_polls"]) - 1.0) * float(p["poll_interval_s"]))


def max_rate_mib_s(mem_total_mib, params=None):
    """이중규칙 전환 하강률(**파생값**) = mem_total / runway. 미상이면 None.

    이 값 이상에서 ETA 부등식 `mem <= rate x runway` 는 `mem <= mem_total` 과 같아져 **항상 참**
    이다 -- 잔량이 판정에 기여하지 않는다. 그래서 여기서부터는 규칙을 바꾼다.

    ★ **정수로 내림**한다. 핫루프(셸)는 정수 산술만 쓰므로 파이썬이 실수 경계를 쓰면 둘이
      경계 부근에서 갈린다. 판정을 두 평면에서 동일하게 만들려면 경계 자체가 정수여야 한다.
      내림 방향은 전환이 **조금 일찍** 일어나는 쪽 = ETA 가 이미 상수에 수렴한 구간이다.
    """
    if not mem_total_mib or mem_total_mib <= 0:
        return None
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    return int(float(mem_total_mib) // runway_s(p))


def compute_eta(mem_avail_mib, rate_mib_s, params=None, mem_total_mib=None):
    """순수 함수. rate_mib_s 는 **양수 = 하강**(소비 속도). 반환 dict.

    rate 가 min_rate 미만이면 하강이 아니라고 보고 ETA=None(무한) 으로 음성정직 표기한다.
    `mem_total_mib` 를 주면 이중규칙(C1-A)이 켜진다 -- 주지 않으면 **기존 ETA 규칙 그대로**다
    (fail-safe: 파생 실패가 규칙 완화로 이어지지 않는다).

    반환의 `rule` 이 **어느 규칙이 판정했는지**를 밝힌다(헌법 §결정론 규율 -- 값 옆에 출처).
    """
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    if mem_avail_mib is None or mem_avail_mib < 0:
        raise ValueError("mem_avail_mib must be >= 0")
    ceiling = max_rate_mib_s(mem_total_mib, p)
    base = {"max_rate_mib_s": ceiling, "abs_band_mib": p["abs_band_mib"]}
    # ★ 최후 바닥 -- 하강률과 **무관하게** 발동. 느린 누수(rate < min_rate)로 0 에 도달하는
    #   경로를 ETA 규칙만으로는 막을 수 없기 때문이다(구조적 구멍의 백스톱).
    if mem_avail_mib <= p["hard_floor_mib"]:
        return dict(base, **{
            "eta_zero_s": (round(mem_avail_mib / float(rate_mib_s), 3)
                           if rate_mib_s and rate_mib_s >= p["min_rate_mib_s"] else None),
            "eta_actionable_s": None, "band": "trip", "trip": True,
            "reason": "hard_floor", "rule": "hard_floor"})
    if rate_mib_s is None or rate_mib_s < p["min_rate_mib_s"]:
        return dict(base, **{"eta_zero_s": None, "eta_actionable_s": None, "band": "green",
                             "trip": False, "reason": "rate_below_min", "rule": "min_rate"})
    eta_zero = mem_avail_mib / float(rate_mib_s)
    # ── 이중규칙 전환(C1-A): ETA 가 상수가 되는 구간은 절대 잔량 밴드로 판정한다 ──────
    if ceiling is not None and rate_mib_s >= ceiling:
        trip = mem_avail_mib <= p["abs_band_mib"]
        return dict(base, **{
            "eta_zero_s": round(eta_zero, 3),
            # ETA 는 이 구간에서 판정 근거가 아니다 -- 숫자를 내면 근거로 오인된다.
            "eta_actionable_s": None,
            # 밴드를 밟지 않아도 'green' 이 아니다: 상한 초과 하강은 그 자체로 경보 상태이며,
            # 진성 방어가 선언·earlyoom 에 위임된 구간이다(헤더 §음성정직).
            "band": "trip" if trip else "red", "trip": trip,
            "reason": "abs_band" if trip else "abs_band_hold", "rule": "abs_band"})
    # 실여유 = 0 도달까지 - (kill 지연 + 디바운스 지연). detect_margin 과 비교한다.
    eta_act = eta_zero - (runway_s(p) - p["detect_margin_s"])
    trip = eta_act <= p["detect_margin_s"]
    if trip:
        band = "trip"
    elif eta_act <= p["agent_act_s"]:
        band = "red"
    elif eta_act <= p["agent_notify_s"]:
        band = "amber"
    else:
        band = "green"
    return dict(base, **{"eta_zero_s": round(eta_zero, 3),
                         "eta_actionable_s": round(eta_act, 3),
                         "band": band, "trip": trip, "reason": "ok", "rule": "eta"})


def trip_threshold_mib(rate_mib_s, params=None, mem_total_mib=None):
    """주어진 하강률에서 TRIP 이 걸리는 MemAvailable 값(설명·검증용 역산).

    이중규칙이 켜져 있고(mem_total 기지) 상한 위 하강이면 역산값은 **절대 밴드**다 --
    이 구간에서 `rate x runway` 를 돌려주면 실제로 쓰이지 않는 선을 보고하게 된다.
    """
    p = dict(DEFAULTS)
    if params:
        p.update({k: v for k, v in params.items() if k in DEFAULTS})
    if rate_mib_s is None or rate_mib_s < p["min_rate_mib_s"]:
        return None
    ceiling = max_rate_mib_s(mem_total_mib, p)
    if ceiling is not None and rate_mib_s >= ceiling:
        return float(p["abs_band_mib"])
    return round(rate_mib_s * runway_s(p), 1)


def validate_params(params):
    """하한 가드. 위반 시 (False, [사유...]) -- 조용한 보정 금지, 거부한다."""
    errs = []
    p = dict(DEFAULTS)
    p.update({k: v for k, v in (params or {}).items() if k in DEFAULTS})
    kl = float(p["kill_latency_s"])
    dm = float(p["detect_margin_s"])
    db = float(p["debounce_polls"])
    floor = kl * ETA_FLOOR_MULTIPLIER
    runway = runway_s(p)
    if kl <= 0:
        errs.append("kill_latency_s must be > 0")
    if dm <= 0:
        errs.append("detect_margin_s must be > 0")
    if db < 1 or db != int(db):
        errs.append("debounce_polls must be an integer >= 1 (got %r)" % db)
    if float(p["poll_interval_s"]) <= 0:
        errs.append("poll_interval_s must be > 0")
    if runway < floor:
        errs.append("runway(kill_latency+detect_margin)=%.2fs < floor(kill_latency*%.1f)=%.2fs"
                    % (runway, ETA_FLOOR_MULTIPLIER, floor))
    # 선언된 바닥 가드: 여유가 음수면 선언이 상한을 **올려** 규칙을 더 민감하게 만들고,
    # 최소상한이 절대바닥 이하면 "거부"가 아무것도 거부하지 못한다(가드가 가드가 아니게 된다).
    if float(p["decl_margin_mib"]) < 0:
        errs.append("decl_margin_mib must be >= 0")
    if float(p["decl_min_ceiling_mib"]) <= float(p["hard_floor_mib"]):
        errs.append("decl_min_ceiling_mib(%s) must exceed hard_floor_mib(%s) — 최소상한이 절대바닥 "
                    "이하면 어떤 선언도 거부되지 않아 가드가 무력해진다"
                    % (p["decl_min_ceiling_mib"], p["hard_floor_mib"]))
    # 이중규칙 가드: 밴드가 절대바닥 이하면 상한 위 구간에서 밴드 규칙이 hard_floor 에 흡수돼
    # **아무것도 판정하지 않는다**. 그건 "완화"가 아니라 그 구간의 조용한 무장해제다.
    if float(p["abs_band_mib"]) <= float(p["hard_floor_mib"]):
        errs.append("abs_band_mib(%s) must exceed hard_floor_mib(%s) — 밴드가 절대바닥 이하면 "
                    "이중규칙이 상한 위 구간을 조용히 무장해제한다"
                    % (p["abs_band_mib"], p["hard_floor_mib"]))
    for k in ("agent_act_s", "agent_notify_s"):
        if float(params.get(k, DEFAULTS[k])) <= runway:
            errs.append("%s must exceed runway %.2fs (에이전트 구간이 데몬 구간보다 안쪽일 수 없음)" % (k, runway))
    if float(params.get("agent_notify_s", DEFAULTS["agent_notify_s"])) <= \
       float(params.get("agent_act_s", DEFAULTS["agent_act_s"])):
        errs.append("agent_notify_s must exceed agent_act_s")
    return (not errs), errs


def emit_shell_params(params, path, mem_total_mib=None, mem_total_source=None):
    """1초 핫루프가 source 할 셸 상수. 부동소수 나눗셈을 피하려고 **밀리초 정수**도 함께 낸다.

    `mem_total_mib` 를 주면 이중규칙 상한(`BB_MAX_RATE_MIB_S`)을 **파생해** 함께 싣는다.
    주지 않으면 0 을 실어 이중규칙을 끈다 = 기존 ETA 규칙(더 죽이는 쪽)이 그대로 남는다.
    """
    p = dict(DEFAULTS)
    p.update({k: v for k, v in (params or {}).items() if k in DEFAULTS})
    lines = [
        "# generated by blackbox_eta.py -- 편집 금지(재생성으로 갱신)",
        "# 산식: eta_actionable = mem_avail/rate - kill_latency ; TRIP <=> eta_actionable <= detect_margin",
        "# 핫루프는 등가 형태를 정수로 쓴다: TRIP <=> mem_avail_mib*1000 <= rate_mib_s*RUNWAY_MS",
        "# 이중규칙(C1-A): rate >= BB_MAX_RATE_MIB_S 이면 위 식 대신 TRIP <=> mem <= BB_ABS_BAND_MIB",
    ]
    runway_ms = int(round(runway_s(p) * 1000))
    max_rate = max_rate_mib_s(mem_total_mib, p)
    for k, v in sorted(p.items()):
        lines.append("BB_%s=%s" % (k.upper(), v))
    lines.append("BB_RUNWAY_MS=%d" % runway_ms)
    lines.append("BB_DEBOUNCE_N=%d" % int(p["debounce_polls"]))
    # 출처 표시(헌법 §결정론 규율) -- 값 옆에 어디서 왔는지를 남긴다. 미상이면 미상이라 적는다.
    lines.append("# mem_total 출처: %s" % (mem_total_source or "unknown"))
    lines.append("BB_MEM_TOTAL_MIB=%d" % int(mem_total_mib or 0))
    if max_rate is None:
        lines.append("# ⚠ mem_total 미상 → 이중규칙 OFF. 상한 위 구간은 옛 규칙(무조건 트립)이다.")
    lines.append("BB_MAX_RATE_MIB_S=%d" % (MAX_RATE_DISABLED if max_rate is None else max_rate))
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write("\n".join(lines) + "\n")
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, path)
    return runway_ms


def load_envelope(node_dir):
    path = os.path.join(node_dir, "envelope.json")
    if not os.path.exists(path):
        return {}
    with open(path, encoding="utf-8") as fh:
        return json.load(fh)


# ── 노드 사실 읽기 ────────────────────────────────────────────────────────
# ★ 이 두 함수의 **정본은 여기**다. regen_envelope.py 가 import 해 쓴다 -- 같은 파싱을 두 파일에
#   손으로 적으면 4종 안티패턴의 `하드코딩·결함`이고, 한쪽만 고쳐지면 두 산출물이 갈린다.
def read_mem_total_mib():
    """/proc/meminfo MemTotal (MiB). 실패는 None -- 호출부가 fail-closed 로 처리한다."""
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for ln in fh:
                if ln.startswith("MemTotal:"):
                    return int(int(ln.split()[1]) / 1024)
    except (OSError, ValueError, IndexError):
        return None
    return None


def read_csv_lines(path):
    """원시/압축 samples 를 읽는다. 실패는 None(호출부가 fail-closed 로 처리)."""
    if path.endswith(".zst"):
        try:
            out = subprocess.run(["zstd", "-dc", path], capture_output=True, timeout=300)
        except (OSError, subprocess.SubprocessError):
            return None
        if out.returncode != 0:
            return None
        return out.stdout.decode("utf-8", "replace").splitlines()
    opener = gzip.open if path.endswith(".gz") else open
    try:
        with opener(path, "rt", encoding="utf-8", errors="replace") as fh:
            return fh.read().splitlines()
    except OSError:
        return None


def resolve_mem_total(node_dir=None, explicit=None):
    """(mem_total_mib, source) -- 우선순위: 명시 > /proc/meminfo > envelope.derived.

    /proc/meminfo 가 envelope 보다 앞서는 이유: 데몬은 **자기가 도는 노드**에서 상수를 emit 하고,
    envelope 은 원격 노드분이거나 동결본일 수 있다. 과소평가는 이중규칙을 일찍 켜(=완화) 위험하고
    과대평가는 늦게 켜(=위양성) 안전하므로, 현재 커널이 보고하는 값이 가장 낫다.
    """
    if explicit:
        return int(explicit), "explicit:--mem-total-mib"
    mt = read_mem_total_mib()
    if mt:
        return mt, "measured:/proc/meminfo MemTotal"
    if node_dir:
        d = (load_envelope(node_dir) or {}).get("derived") or {}
        if d.get("mem_total_mib"):
            return int(d["mem_total_mib"]), "envelope:%s" % (d.get("mem_total_source") or "derived")
    return None, "unknown:/proc/meminfo 판독 실패 · envelope.derived 부재"


# ── 재생기 (C1 성공기준 2) ────────────────────────────────────────────────
def replay_samples(path, params=None, mem_total_mib=None, arm_ceiling_mib=None):
    """1 Hz samples CSV 를 워치독과 **같은 디바운스 상태기계**로 재생해 kill 을 센다.

    plan_26081415 C1 성공기준 2 의 증거 생성기다. 두 규칙(기존 ETA 전용 / 신규 이중규칙)을 같은
    입력에 동시에 돌려 **차분**을 낸다 -- 한쪽만 돌리면 "0회"가 규칙 덕인지 데이터 덕인지 모른다.

    ⚠ 한계(음성정직): 첫 kill 이후의 궤적은 **반사실**이다. 실제로 kill 이 일어났다면 컨테이너가
      죽어 메모리가 회복됐을 것이므로 그 뒤 샘플은 존재하지 않았을 것이다. 그래서 `kills` 는
      "이 궤적을 끝까지 재생했을 때의 발동 횟수"이고, 판정에 쓰는 값은 `first_kill` 이다.
      워치독의 kill 후 `prev_mem=""` 리셋은 재생에서 **다음 1폴의 rate 를 0 으로** 두어 흉내낸다.
    """
    lines = read_csv_lines(path)
    if not lines:
        return None
    header = lines[0].split(",")
    idx = {k: i for i, k in enumerate(header)}
    if "mem_avail" not in idx or "mem_rate" not in idx:
        return None
    p = dict(DEFAULTS)
    p.update({k: v for k, v in (params or {}).items() if k in DEFAULTS})
    debounce = int(p["debounce_polls"])
    ceil_arm = float(arm_ceiling_mib) if arm_ceiling_mib else float("inf")

    rules = {"eta_only": None, "dual": mem_total_mib}
    state = {k: {"streak": 0, "kills": [], "skip_rate": False,
                 "trip_polls": 0, "max_streak": 0} for k in rules}
    rows = 0
    max_rate = 0.0
    min_mem = None
    for line in lines[1:]:
        parts = line.split(",")
        if len(parts) < len(header):
            continue
        try:
            mem = float(parts[idx["mem_avail"]])
        except ValueError:
            continue
        try:
            rate = float(parts[idx["mem_rate"]])
        except ValueError:
            rate = 0.0
        rows += 1
        max_rate = max(max_rate, rate)
        min_mem = mem if min_mem is None else min(min_mem, mem)
        ts = parts[idx["ts"]] if "ts" in idx else ""
        for name, mt in rules.items():
            st = state[name]
            r = 0.0 if st["skip_rate"] else rate
            st["skip_rate"] = False
            # 워치독 순서와 동일: hard_floor → min_rate → arm_ceiling → (이중규칙|ETA)
            if mem > p["hard_floor_mib"] and mem > ceil_arm:
                trip = False
            else:
                trip = compute_eta(mem, r, p, mem_total_mib=mt)["trip"]
            if trip:
                st["streak"] += 1
                st["trip_polls"] += 1
                st["max_streak"] = max(st["max_streak"], st["streak"])
                if st["streak"] >= debounce:
                    st["kills"].append({"ts": ts, "mem_avail_mib": mem, "rate_mib_s": rate})
                    st["streak"] = 0
                    st["skip_rate"] = True
            else:
                st["streak"] = 0
    out = {"path": path, "rows": rows, "max_rate_mib_s": max_rate, "min_mem_avail_mib": min_mem,
           "max_rate_ceiling_mib_s": max_rate_mib_s(mem_total_mib, p),
           "abs_band_mib": p["abs_band_mib"], "runway_s": runway_s(p),
           "arm_ceiling_mib": (None if ceil_arm == float("inf") else ceil_arm)}
    for name in rules:
        st = state[name]
        # `trip_polls`·`max_streak` = **얼마나 아슬아슬했는가**. kill=0 만 보면 "규칙이 여유롭게
        # 통과시켰다"와 "디바운스 한 폴 차이로 살았다"가 구별되지 않는다.
        out[name] = {"kills": len(st["kills"]), "first_kill": st["kills"][0] if st["kills"] else None,
                     "trip_polls": st["trip_polls"], "max_streak": st["max_streak"],
                     "debounce_polls": debounce}
    return out


# eta_params 안에서 값이 아니라 메타인 키. 정본 위치는 envelope.eta_params_source 이며
# 여기서는 **미지 키로 세지 않는다**(seed 시절 관습의 하위호환 표시).
ENVELOPE_META_KEYS = ("provenance",)


def unknown_envelope_keys(envelope):
    """envelope.eta_params 중 DEFAULTS 에 없는 키 = **조용히 버려지는 키**.

    ★ 이 함수가 정본이다(regen_envelope.check_keys 가 이걸 호출한다). DEFAULTS 키 목록을
      두 파일에 손으로 적으면 4종 안티패턴의 `하드코딩·결함`이고, 정본이 바뀔 때 한쪽만
      옛 목록으로 남아 검증기가 거짓 판정을 낸다.

    근거 plan_26081415 §1.2: seed envelope 의 `daemon_kill_s`·`eta_floor_s` 가 정확히 이
    경로로 증발했고, 그래서 "envelope 재생성만 하면 위양성은 100% 재발"한다.
    """
    params = (envelope or {}).get("eta_params") or {}
    if not isinstance(params, dict):
        return []
    return sorted(k for k in params
                  if k not in DEFAULTS and k not in ENVELOPE_META_KEYS)


def _self_test():
    checks = []

    # 1) 정지 상태 -- 하강 아님
    r = compute_eta(50000, 0.0)
    checks.append(("정지 -> ETA None·green·무트립",
                   r["eta_zero_s"] is None and r["band"] == "green" and not r["trip"]))

    # 2) 실측 최대 하강률(6241 MiB/s)에서의 트립 지점 역산
    #    runway = kill 4s + detect 2s + 디바운스 (3-1)x1s = 8s -> 6241.07*8 = 49,928.6 MiB
    th = trip_threshold_mib(6241.07)
    checks.append(("6.1GiB/s 트립 지점 ~49.9GiB(디바운스 포함)", abs(th - 49928.6) < 1.0))
    # 2b) 디바운스를 끄면 활주로가 줄어 트립 지점도 낮아진다(등가성 확인)
    th1 = trip_threshold_mib(6241.07, {"debounce_polls": 1.0})
    checks.append(("디바운스 1폴 -> 활주로 6s -> ~37.4GiB", abs(th1 - 37446.4) < 1.0))
    checks.append(("디바운스↑ -> 트립 지점↑(지연분 보상)", th > th1))

    # 3) 완만 구간(44 MiB/s) -- 7/22 오발 재현 검사.
    #    당시 10,186 MiB 에서 kill 했으나 ETA 기준으로는 트립 대상이 아니어야 한다.
    r = compute_eta(10186, 44.0)
    checks.append(("44MiB/s·10.2GiB -> 무트립(오발 제거)", not r["trip"]))
    checks.append(("같은 지점 eta_zero ~231s", abs(r["eta_zero_s"] - 10186 / 44.0) < 0.01))

    # 4) kill 지연 항이 실제로 보수적으로 작동하는가 (동일 조건에서 지연을 늘리면 더 일찍 트립)
    slow = trip_threshold_mib(1000.0, {"kill_latency_s": 4.0})
    fast = trip_threshold_mib(1000.0, {"kill_latency_s": 10.0})
    checks.append(("kill 지연↑ -> 트립 지점↑(보수)", fast > slow))

    # 5) 급락에서 트립
    r = compute_eta(5000, 2000.0)
    checks.append(("2GiB/s·5GiB -> TRIP", r["trip"] and r["band"] == "trip"))

    # 6) 밴드 경계
    r = compute_eta(100000, 200.0)      # eta_zero=500s, actionable=496s -> red(<=300? no) -> amber
    checks.append(("500s -> amber", r["band"] == "amber"))
    r = compute_eta(100000, 500.0)      # eta_zero=200s, actionable=196s -> red
    checks.append(("196s -> red", r["band"] == "red"))
    r = compute_eta(100000, 50.0)       # eta_zero=2000s -> green
    checks.append(("2000s -> green", r["band"] == "green"))

    # 7) 하한 가드 -- 물리 불가 설정 거부
    ok, errs = validate_params({"kill_latency_s": 4.0, "detect_margin_s": 2.0})
    checks.append(("정상 파라미터 통과", ok))
    ok2, errs2 = validate_params({"kill_latency_s": 10.0, "detect_margin_s": 0.5})
    checks.append(("runway < floor 거부", (not ok2) and any("floor" in e for e in errs2)))
    ok3, _ = validate_params({"kill_latency_s": 4.0, "detect_margin_s": 2.0, "agent_act_s": 3.0})
    checks.append(("에이전트 구간이 데몬 안쪽이면 거부", not ok3))
    ok4, _ = validate_params({"agent_act_s": 900.0, "agent_notify_s": 300.0})
    checks.append(("notify <= act 거부", not ok4))
    ok5, errs5 = validate_params({"debounce_polls": 0.0})
    checks.append(("디바운스 0폴 거부", (not ok5) and any("debounce" in e for e in errs5)))
    ok6, errs6 = validate_params({"debounce_polls": 2.5})
    checks.append(("디바운스 비정수 거부", (not ok6) and any("debounce" in e for e in errs6)))

    # 9) 최후 절대 바닥 -- min_rate 구멍(느린 누수)의 백스톱
    r = compute_eta(4000, 0.2)      # 0.2 MiB/s = min_rate 미만 = ETA 규칙상 'green'
    checks.append(("느린 누수라도 바닥 밑이면 TRIP", r["trip"] and r["reason"] == "hard_floor"))
    r = compute_eta(6000, 0.2)      # 바닥 위 + 느린 하강 -> 트립 안 함(기존 동작 보존)
    checks.append(("바닥 위 느린 하강은 무트립", not r["trip"] and r["reason"] == "rate_below_min"))
    r = compute_eta(10186, 44.0)    # 7/22 오발 지점이 바닥에 걸리지 않아야 한다
    checks.append(("7/22 오발 지점(10,186)은 바닥 밖", not r["trip"]))
    r = compute_eta(5120, 0.0)      # 경계 포함(<=)
    checks.append(("바닥 경계값 포함", r["trip"] and r["reason"] == "hard_floor"))

    # 8) 셸 상수 emit -- 핫루프 등가식이 파이썬 판정과 일치하는가
    import tempfile
    with tempfile.TemporaryDirectory() as td:
        p = os.path.join(td, "eta_params.env")
        runway_ms = emit_shell_params({}, p)
        txt = open(p, encoding="utf-8").read()
        # 핫루프(mem_watchdog.sh)가 쓰는 **완전한** 등가식: 최후 바닥 OR ETA 활주로.
        # 바닥 항을 빼면 느린 누수 케이스에서 셸과 파이썬이 갈린다 -- 그 갈림을 여기서 고정한다.
        floor = DEFAULTS["hard_floor_mib"]
        agree, disagreements = True, []
        for mem, rate in ((5000, 2000.0), (10186, 44.0), (37000, 6241.07), (120000, 1.0),
                          (4000, 0.2), (6000, 0.2), (5120, 0.0), (49000, 6241.07)):
            py = compute_eta(mem, rate)["trip"]
            sh = (mem <= floor) or ((mem * 1000) <= (rate * runway_ms))
            if py != sh:
                agree = False
                disagreements.append((mem, rate, py, sh))
        checks.append(("셸 등가식 == 파이썬 판정 (8 케이스)%s"
                       % ("" if agree else " 불일치=%r" % disagreements), agree))

        # 8b) ★ 이중규칙까지 포함한 등가식 (plan_26081415 C1 성공기준 3).
        #     상한 경계 ±1 MiB/s 를 반드시 넣는다 -- 파이썬이 실수, 셸이 정수라 경계에서 갈리기
        #     가장 쉽고, 갈리면 "시험은 통과인데 현장은 다르게 죽인다"가 된다.
        mt = 124610                       # GB10 실측 총량(self-test 고정 입력)
        emit2 = os.path.join(td, "eta_params_dual.env")
        runway_ms2 = emit_shell_params({}, emit2, mem_total_mib=mt,
                                       mem_total_source="explicit:self-test")
        txt2 = open(emit2, encoding="utf-8").read()
        sh_max = max_rate_mib_s(mt)       # 셸이 읽을 정수 상한
        band = DEFAULTS["abs_band_mib"]
        checks.append(("emit 에 BB_MAX_RATE_MIB_S 파생값", "BB_MAX_RATE_MIB_S=%d\n" % sh_max in txt2))
        checks.append(("emit 에 BB_ABS_BAND_MIB=10240(정수)", "BB_ABS_BAND_MIB=10240\n" in txt2))
        checks.append(("emit 에 BB_MEM_TOTAL_MIB 실값", "BB_MEM_TOTAL_MIB=%d\n" % mt in txt2))
        agree2, dis2 = True, []
        dual_cases = ((50772, 21943.0), (35158, 22799.0), (46641, 23313.5),   # R0 실측 사살·최대
                      (74181, 27893.0), (28686, 23162.0),                     # 진성 KV 벌룬
                      (9000, 20000.0), (10240, 20000.0), (10241, 20000.0),    # 밴드 경계
                      (60000, float(sh_max)), (60000, float(sh_max - 1)),     # ★ 상한 경계
                      (60000, float(sh_max + 1)), (5000, 30000.0),            # 상한 위 바닥
                      (37000, 6241.07), (120000, 1.0), (10186, 44.0))         # 상한 아래 회귀
        for mem, rate in dual_cases:
            py = compute_eta(mem, rate, mem_total_mib=mt)["trip"]
            sh = (mem <= floor) or (
                rate >= DEFAULTS["min_rate_mib_s"] and (
                    (mem <= band) if (sh_max and rate >= sh_max)
                    else ((mem * 1000) <= (rate * runway_ms2))))
            if py != sh:
                agree2 = False
                dis2.append((mem, rate, py, sh))
        checks.append(("★ 이중규칙 셸 등가식 == 파이썬 (%d 케이스)%s"
                       % (len(dual_cases), "" if agree2 else " 불일치=%r" % dis2), agree2))
        # mem_total 미상이면 상한 0 = 이중규칙 OFF = 옛 규칙(fail-safe 방향)
        emit3 = os.path.join(td, "eta_params_nomt.env")
        emit_shell_params({}, emit3, mem_total_mib=None, mem_total_source="unknown:self-test")
        txt3 = open(emit3, encoding="utf-8").read()
        checks.append(("mem_total 미상 → BB_MAX_RATE_MIB_S=0(이중규칙 OFF)",
                       "BB_MAX_RATE_MIB_S=0\n" in txt3 and "이중규칙 OFF" in txt3))
        checks.append(("mem_total 미상 → 판정이 옛 규칙과 동일(더 죽이는 쪽)",
                       compute_eta(50772, 21943.0, mem_total_mib=None)["trip"] is True))
        checks.append(("emit 파일에 RUNWAY_MS=8000", "BB_RUNWAY_MS=8000" in txt))
        checks.append(("emit 파일에 DEBOUNCE_N=3", "BB_DEBOUNCE_N=3" in txt))
        # 선언된 바닥 상수가 셸로 흘러가는가 — 워치독이 읽는 **정확한 변수명**이어야 한다.
        # 이름이 어긋나면 워치독은 조용히 자기 하드코딩 기본값으로 돌고, 여기서 조정한 값은 증발한다.
        checks.append(("emit 에 DECL_MARGIN_MIB=3072", "BB_DECL_MARGIN_MIB=3072\n" in txt))
        checks.append(("emit 에 DECL_MIN_CEILING_MIB=8192", "BB_DECL_MIN_CEILING_MIB=8192\n" in txt))
        # 정수로 나가야 한다 — 셸 산술은 정수 전용이고 '8192.0' 은 워치독 비교에서 터진다.
        checks.append(("선언 상수가 정수 표기",
                       "BB_DECL_MARGIN_MIB=3072.0" not in txt
                       and "BB_DECL_MIN_CEILING_MIB=8192.0" not in txt))
        # 가드가 가드로 작동하는가
        bad, _ = validate_params({"decl_min_ceiling_mib": 5120})   # == hard_floor
        checks.append(("최소상한 <= 절대바닥 거부", not bad))
        bad2, _ = validate_params({"decl_margin_mib": -1})
        checks.append(("음수 여유 거부", not bad2))

    # 10) envelope 키 정합 (plan_26081415 C2-3) — **조용한 버림**의 회귀 고정.
    #     seed envelope(2026-07-31 동결본)이 실제로 담고 있던 키를 그대로 재현한다.
    seed_env = {"eta_params": {"agent_act_s": 300, "agent_notify_s": 900,
                               "daemon_kill_s": 6, "eta_floor_s": 6.0,
                               "provenance": "plan_26073109 §2.2 초기 임의값"}}
    checks.append(("★ seed envelope 미지 키 = daemon_kill_s·eta_floor_s 2건",
                   unknown_envelope_keys(seed_env) == ["daemon_kill_s", "eta_floor_s"]))
    checks.append(("eta_params.provenance 는 미지 키로 세지 않는다(메타)",
                   "provenance" not in unknown_envelope_keys(seed_env)))
    checks.append(("정본 키만 있으면 미지 키 0",
                   unknown_envelope_keys({"eta_params": {"kill_latency_s": 6.0}}) == []))
    checks.append(("envelope 부재/빈 dict 도 안전",
                   unknown_envelope_keys({}) == [] and unknown_envelope_keys(None) == []))
    # 그 2건이 정확히 "트립 판정에 닿지 않는" 이유: 교집합이 알림 밴드뿐이다.
    _seed_keys = set(seed_env["eta_params"]) - {"provenance"}
    checks.append(("seed 키의 정본 교집합은 알림 밴드 2종뿐",
                   sorted(_seed_keys & set(DEFAULTS)) == ["agent_act_s", "agent_notify_s"]))
    _trip_keys = {"kill_latency_s", "detect_margin_s", "debounce_polls",
                  "poll_interval_s", "min_rate_mib_s", "hard_floor_mib"}
    checks.append(("★ seed 키 중 트립 판정에 쓰이는 키는 0개",
                   not (_seed_keys & _trip_keys)))

    # 11) ★ 이중규칙 (plan_26081415 C1-A) ─────────────────────────────────
    MT = 124610                       # GB10 실측 총량
    CEIL = max_rate_mib_s(MT)         # runway 8s 기준 15,576 MiB/s
    checks.append(("상한이 mem_total/runway 에서 파생(15,576)", CEIL == 15576))
    checks.append(("상한이 mem_total 변경을 따라간다(하드코딩 아님)",
                   max_rate_mib_s(MT // 2) == 7788))
    checks.append(("kill 지연이 늘면 상한이 내려간다(runway 종속)",
                   max_rate_mib_s(MT, {"kill_latency_s": 6.0}) == 12461))
    checks.append(("mem_total 미상 → 상한 None(이중규칙 OFF)",
                   max_rate_mib_s(None) is None and max_rate_mib_s(0) is None))

    # (a) R0 실측 사살 2건 — 위양성. 이중규칙에서 미발동해야 한다.
    for mem, rate, tag in ((50772, 21943.0, "R0 시도①"), (35158, 22799.0, "R0 시도②")):
        r = compute_eta(mem, rate, mem_total_mib=MT)
        checks.append(("★ %s 사살점(%d@%d) → 이중규칙 미발동" % (tag, mem, rate),
                       (not r["trip"]) and r["rule"] == "abs_band"))
        checks.append(("  같은 점이 옛 규칙에서는 발동했다(대조)",
                       compute_eta(mem, rate)["trip"]))
    # (b) R0 시도③ 성공 궤적의 최저점 — 선언 없이도 살아야 한다(§1.1 반사실 재생)
    r = compute_eta(46641, 23313.5, mem_total_mib=MT)
    checks.append(("★ R0 시도③ 반사실 지점(46,641@23,313) → 이중규칙 미발동", not r["trip"]))
    # (c) 밴드 경계
    checks.append(("상한 위·밴드 경계값(10,240) 포함 → 발동",
                   compute_eta(10240, 20000.0, mem_total_mib=MT)["trip"]))
    checks.append(("상한 위·밴드 위 1MiB → 미발동",
                   not compute_eta(10241, 20000.0, mem_total_mib=MT)["trip"]))
    checks.append(("상한 위·바닥 밑 → hard_floor 로 발동(밴드와 무관하게 항상 무장)",
                   compute_eta(5000, 30000.0, mem_total_mib=MT)["rule"] == "hard_floor"))
    # (d) 상한 아래는 기존 ETA 규칙이 **그대로**여야 한다(회귀)
    for mem, rate in ((37000, 6241.07), (10186, 44.0), (120000, 1.0), (5000, 2000.0)):
        checks.append(("상한 아래 회귀(%d@%g) — 옛 판정과 동일" % (mem, rate),
                       compute_eta(mem, rate, mem_total_mib=MT)["trip"]
                       == compute_eta(mem, rate)["trip"]))
    # (e) ⚠ 음성정직 — 이 규칙이 포기하는 것을 **시험으로 고정**한다.
    #     진성 KV 벌룬은 상한 위에서 밴드를 밟지 않아 미발동한다. 이건 결함이 아니라
    #     plan_26081415 §2 가 명시한 설계 트레이드오프이며, 방어는 선언 필수화(C1-B/C3)와
    #     earlyoom 으로 이동한다. 나중에 누가 "진성도 잡히네" 라고 오해하지 않도록 못을 박는다.
    balloon = ((74181, 27893.0), (51848, 22333.0), (28686, 23162.0))
    checks.append(("⚠ 진성 KV벌룬은 이중규칙에서 **미발동**(설계상 포기 · 방어는 선언/earlyoom)",
                   all(not compute_eta(m, r, mem_total_mib=MT)["trip"] for m, r in balloon)))
    checks.append(("  같은 궤적이 옛 규칙에서는 전부 발동했다(무엇을 포기했는지 대조)",
                   all(compute_eta(m, r)["trip"] for m, r in balloon)))
    checks.append(("  벌룬이 밴드까지 내려오면 되찾는다(10,000@23,162 → 발동)",
                   compute_eta(10000, 23162.0, mem_total_mib=MT)["trip"]))
    # (f) 역산이 실제 쓰이는 선을 보고하는가
    checks.append(("상한 위 역산 = 절대 밴드",
                   trip_threshold_mib(20000.0, mem_total_mib=MT) == 10240.0))
    checks.append(("상한 아래 역산 = rate x runway(기존)",
                   trip_threshold_mib(1000.0, mem_total_mib=MT) == 8000.0))
    # (g) 가드
    bad_band, errs_b = validate_params({"abs_band_mib": 5120})     # == hard_floor
    checks.append(("밴드 <= 절대바닥 거부(조용한 무장해제 방지)",
                   (not bad_band) and any("abs_band_mib" in e for e in errs_b)))
    checks.append(("정상 밴드 통과", validate_params({"abs_band_mib": 10240})[0]))

    # 12) 레거시 절대임계와의 교차검증 — 단일 소유가 불가능하면 교차검증이 차선(workflow.md).
    #     밴드는 새 숫자가 아니라 레거시 워치독이 이미 쓰던 값이다. 둘이 갈라지면 "두 워치독이
    #     서로 다른 선에서 죽인다"가 되고, 그 갈림은 값 스캔이 아니라 이 술어가 잡는다.
    _legacy = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                           "..", "host_safety", "mem_watchdog.sh")
    if os.path.isfile(_legacy):
        _txt = open(_legacy, encoding="utf-8", errors="replace").read()
        checks.append(("★ 레거시 워치독 절대임계와 abs_band_mib 일치(교차검증)",
                       'THRESH_MIB="${2:-%d}"' % int(DEFAULTS["abs_band_mib"]) in _txt))
    else:
        # 부재를 조용히 넘기지 않는다 -- 배포 배치가 바뀌었거나 레거시가 제거된 것이며,
        # 후자라면 이 평면이 밴드의 유일 소유자가 됐다는 뜻이라 사람이 알아야 한다.
        print("  [INFO] 레거시 워치독 부재(%s) — 교차검증 생략. abs_band_mib 의 정본이 "
              "이 파일 하나뿐인지 확인하라." % _legacy)

    # 13) 재생기 (C1 성공기준 2 의 증거 생성기 자체를 시험한다) ────────────
    with tempfile.TemporaryDirectory() as td2:
        hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n"

        def _write(name, rows):
            p = os.path.join(td2, name)
            with open(p, "w", encoding="utf-8") as fh:
                fh.write(hdr)
                for i, (m, r) in enumerate(rows):
                    fh.write("%d,%g,%g,50,10,1,0,,0.5,1\n" % (1000 + i, m, r))
            return p

        # 대형 로드(위양성): 상한 위 급락이 5폴 지속되지만 밴드는 밟지 않는다
        load = _write("load.csv", [(112195, 0), (90000, 22195.0), (68000, 22000.0),
                                   (46000, 22000.0), (24000, 22000.0), (24100, -100.0),
                                   (24100, 0)])
        r = replay_samples(load, mem_total_mib=MT)
        checks.append(("★ 재생: 대형 로드 — 옛 규칙 kill>=1 · 이중규칙 kill=0",
                       r["eta_only"]["kills"] >= 1 and r["dual"]["kills"] == 0))
        checks.append(("  재생 요약이 상한·밴드를 함께 보고(출처 표시)",
                       r["max_rate_ceiling_mib_s"] == CEIL and r["abs_band_mib"] == 10240))
        # 밴드 아래로 관통하면 이중규칙도 발동한다
        thru = _write("through.csv", [(30000, 0), (20000, 20000.0), (10000, 20000.0),
                                      (9000, 20000.0), (8000, 20000.0)])
        r2 = replay_samples(thru, mem_total_mib=MT)
        checks.append(("재생: 밴드 관통 → 이중규칙도 kill>=1", r2["dual"]["kills"] >= 1))
        # 유휴일(느린 변동)은 두 규칙 모두 0
        idle = _write("idle.csv", [(100000, 0), (99990, 10.0), (99980, 10.0), (99970, 10.0)])
        r3 = replay_samples(idle, mem_total_mib=MT)
        checks.append(("재생: 유휴 궤적 — 두 규칙 모두 kill=0",
                       r3["eta_only"]["kills"] == 0 and r3["dual"]["kills"] == 0))
        # 디바운스가 재생에서도 산다(2폴만 지속 → 미발동)
        two = _write("two.csv", [(30000, 0), (18000, 12000.0), (30000, -12000.0), (30000, 0)])
        r4 = replay_samples(two, mem_total_mib=MT)
        checks.append(("재생: 2폴만 지속 → 미발동(디바운스 보존)",
                       r4["eta_only"]["kills"] == 0 and r4["dual"]["kills"] == 0))
        # 선언 arm 상한을 주면 그 위는 재생에서도 미발동
        r5 = replay_samples(load, mem_total_mib=MT, arm_ceiling_mib=32768)
        checks.append(("재생: arm 상한 적용 시 옛 규칙도 kill=0(선언 효과 재현)",
                       r5["eta_only"]["kills"] == 0))
    # ── 조상 소유자 상속 디렉터리 생성 (2026-09-03 신설 · plan_26090317 P3 실측) ──
    #   root 데몬이 프로젝트 경로를 만들면 root:root 로 굳어 위임 사용자가 정착을 못 한다.
    #   wipe 20초 만에 재발함을 실측했다. 규칙: **프로젝트 경로는 부모를 소유한 사람의 것**.
    import tempfile as _tf, os as _os
    with _tf.TemporaryDirectory() as _d:
        _target = _os.path.join(_d, "proj", "docs", "logs", "sub", "samples")
        _made = makedirs_as_ancestor_owner(_target)
        checks.append(("없는 체인을 전부 만든다", len(_made) == 5 and _os.path.isdir(_target)))
        checks.append(("만든 경로가 조상 소유자를 물려받는다",
                       _os.stat(_os.path.join(_d, "proj")).st_uid == _os.stat(_d).st_uid))
        checks.append(("멱등 — 이미 있으면 신규 0건",
                       makedirs_as_ancestor_owner(_target) == []))
        # 음성대조: 기존 디렉터리의 소유권은 건드리지 않는다(우리가 만든 것만 정렬한다).
        _pre = _os.path.join(_d, "preexisting")
        _os.makedirs(_pre)
        _before = _os.stat(_pre).st_uid
        makedirs_as_ancestor_owner(_os.path.join(_pre, "child"))
        checks.append(("기존 경로의 소유권은 변경하지 않는다",
                       _os.stat(_pre).st_uid == _before))
        # 비-root 에서는 chown 을 시도조차 하지 않는다(권한 오류로 죽지 않는다).
        checks.append(("비-root 에서도 예외 없이 동작", _os.path.isdir(_os.path.join(_pre, "child"))))
        # 파일판: 없는 파일·비-root 모두 예외 없이 False, 있는 파일은 그대로 남는다.
        _f = _os.path.join(_pre, "child", "x.csv"); open(_f, "w").close()
        _r = inherit_dir_owner(_f, _os.path.dirname(_f))
        checks.append(("파일 소유 상속: 비-root 는 no-op(False) · 파일 보존",
                       _r is False and _os.path.isfile(_f)))
        checks.append(("파일 소유 상속: 없는 파일도 예외 없이 False",
                       inherit_dir_owner(_os.path.join(_pre, "none.csv"), _pre) is False))

        checks.append(("재생: 판독 불가 경로는 None(조용한 0 아님)",
                       replay_samples(os.path.join(td2, "없다.csv")) is None))

    ok_all = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok_all = ok_all and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok_all else "FAIL", len(checks)))
    return 0 if ok_all else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="노드블랙박스 ETA 엔진")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--emit-params", help="셸 상수 출력 경로 (eta_params.env)")
    ap.add_argument("--set", action="append", default=[], metavar="K=V",
                    help="파라미터 override (예: --set kill_latency_s=5)")
    ap.add_argument("--allow-unknown-envelope-keys", action="store_true",
                    help="envelope.eta_params 의 미지 키를 거부하지 않고 무시(임시 탈출구)")
    ap.add_argument("--explain", nargs=2, type=float, metavar=("MEM_MIB", "RATE_MIB_S"),
                    help="주어진 잔량·하강률의 ETA/밴드/트립 판정을 설명")
    ap.add_argument("--mem-total-mib", type=int,
                    help="이중규칙 상한 파생용 총량. 기본 = /proc/meminfo → envelope.derived")
    ap.add_argument("--replay", nargs="+", metavar="CSV",
                    help="samples CSV 를 재생해 기존 ETA 규칙 대 이중규칙의 kill 횟수를 비교")
    ap.add_argument("--replay-arm-ceiling", type=int,
                    help="재생 시 선언된 바닥의 arm 상한(기본=선언 없음 가정)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    params = {}
    if args.node_dir:
        env = load_envelope(args.node_dir)
        # ★ 조용한 버림 금지(plan_26081415 C2-3). 예전엔 `if k in DEFAULTS` 필터가 미지 키를
        #   말없이 삼켰다 — 그래서 14일 동결된 envelope 이 "반영되고 있다"고 오인됐고, 실제로는
        #   트립 파라미터에 **하나도 닿지 않았다**. 이제는 큰 소리로 실패한다.
        unknown = unknown_envelope_keys(env)
        if unknown and not args.allow_unknown_envelope_keys:
            print("[eta] envelope.eta_params 에 정본에 없는 키가 있다 — 거부(조용히 버리지 않는다):",
                  file=sys.stderr)
            for k in unknown:
                print("  - %s (가능: %s)" % (k, ", ".join(sorted(DEFAULTS))), file=sys.stderr)
            print("  해소: regen_envelope.py regen 으로 재생성하거나, 정본 키로 이관하라. "
                  "(임시 통과는 --allow-unknown-envelope-keys)", file=sys.stderr)
            return 1
        if unknown:
            print("[eta] ⚠ 미지 키 %s 를 무시하고 진행(--allow-unknown-envelope-keys)"
                  % ", ".join(unknown), file=sys.stderr)
        params.update({k: v for k, v in (env.get("eta_params") or {}).items() if k in DEFAULTS})
    for kv in args.set:
        if "=" not in kv:
            ap.error("--set 는 K=V 형식: %r" % kv)
        k, v = kv.split("=", 1)
        if k not in DEFAULTS:
            ap.error("알 수 없는 파라미터 %r (가능: %s)" % (k, ", ".join(sorted(DEFAULTS))))
        params[k] = float(v)

    ok, errs = validate_params(params)
    if not ok:
        print("[eta] 하한 가드 위반 — 거부:", file=sys.stderr)
        for e in errs:
            print("  - " + e, file=sys.stderr)
        return 1

    mem_total, mt_src = resolve_mem_total(args.node_dir, args.mem_total_mib)

    if args.explain:
        mem, rate = args.explain
        r = compute_eta(mem, rate, params, mem_total_mib=mem_total)
        eff = dict(DEFAULTS); eff.update(params)
        print(json.dumps({"input": {"mem_avail_mib": mem, "rate_mib_s": rate},
                          "params": eff, "result": r,
                          "mem_total_mib": mem_total, "mem_total_source": mt_src,
                          "max_rate_mib_s": max_rate_mib_s(mem_total, params),
                          "trip_threshold_mib_at_this_rate":
                              trip_threshold_mib(rate, params, mem_total_mib=mem_total)},
                         ensure_ascii=False, indent=2))
        return 0

    if args.replay:
        out = []
        for path in args.replay:
            r = replay_samples(path, params, mem_total_mib=mem_total,
                               arm_ceiling_mib=args.replay_arm_ceiling)
            if r is None:
                print("[eta] ⚠ 재생 불가(판독 실패 또는 컬럼 부재): %s" % path, file=sys.stderr)
                out.append({"path": path, "error": "unreadable"})
                continue
            out.append(r)
            fk = r["dual"]["first_kill"]
            print("[eta] %s rows=%d maxrate=%.0f minmem=%.0f | ETA전용 kill=%d · 이중규칙 kill=%d%s"
                  % (os.path.basename(path), r["rows"], r["max_rate_mib_s"],
                     r["min_mem_avail_mib"] or 0, r["eta_only"]["kills"], r["dual"]["kills"],
                     "" if not fk else "  ← 최초 %s mem=%s rate=%s"
                     % (fk["ts"], fk["mem_avail_mib"], fk["rate_mib_s"])))
        tot_e = sum(x.get("eta_only", {}).get("kills", 0) for x in out)
        tot_d = sum(x.get("dual", {}).get("kills", 0) for x in out)
        print("[eta] 합계: ETA전용 kill=%d · 이중규칙 kill=%d (mem_total=%s · 상한=%s MiB/s · %s)"
              % (tot_e, tot_d, mem_total, max_rate_mib_s(mem_total, params), mt_src))
        print(json.dumps({"files": out, "total_eta_only_kills": tot_e,
                          "total_dual_kills": tot_d, "mem_total_mib": mem_total,
                          "mem_total_source": mt_src,
                          "max_rate_mib_s": max_rate_mib_s(mem_total, params)},
                         ensure_ascii=False, indent=2))
        return 0

    if args.emit_params:
        runway_ms = emit_shell_params(params, args.emit_params, mem_total_mib=mem_total,
                                      mem_total_source=mt_src)
        mr = max_rate_mib_s(mem_total, params)
        print("[eta] emit %s (runway=%dms · mem_total=%s[%s] · 이중규칙 상한=%s)"
              % (args.emit_params, runway_ms, mem_total, mt_src,
                 "%d MiB/s" % mr if mr else "OFF(미상)"))
        if mr is None:
            print("[eta] ⚠ mem_total 미상 → 이중규칙 OFF. 상한 위 구간은 옛 규칙(무조건 트립)이다.",
                  file=sys.stderr)
        return 0

    ap.error("--explain / --emit-params / --replay / --self-test 중 하나가 필요합니다")


if __name__ == "__main__":
    sys.exit(main())
