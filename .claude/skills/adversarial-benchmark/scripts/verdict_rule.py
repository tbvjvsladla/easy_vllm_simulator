#!/usr/bin/env python3
# verdict_rule.py — 결정론 PASS/REFUTE 게이트 (adversarial-benchmark §6·§9)
#
# ★ 게이트는 *규칙*이 결정한다 — LLM 다수결 아님(§6). LLM Devil's Advocate 는 외부검색(E)·
#   진단·재탐색힌트(증거/판정)만 생산해 이 규칙에 투입. 여기서 PASS/REFUTE 가 결정론으로 닫힌다.
#
# 루브릭 우선순위(3중 방어막) — **트리거 기반 authority**(SKILL.md §2 · plan_26081514 Q4/Step 5):
#   --authority weak    (기본) : primary = E(외부 현실-달성치) > c(사용자 목표) > expected_achievable(루프라인×MBU)
#   --authority explicit       : primary = c(사용자 목표) > E > expected_achievable
#   --authority explore        : primary = E > expected_achievable  (**c 칸 부재** — 목표를 세우지 않는다)
#   ★ explicit 은 **사용자가 HITL 로 목표를 명시했을 때만** 켠다(에이전트 자기선언 ✗). 남용 방어의 전부가
#     이 조건이다 — c 가 정본이 되면 낮은 목표로 검증을 우회할 수 있기 때문.
#   ★ explore 는 사용자가 HITL 로 *"목표 0/NULL/광범위 탐색"* 을 지시했을 때만 켠다. 게이트를 세우지
#     않는 상태이므로 `--target-tps` 는 **금지**다(존재하면 모순 → exit 2). loop-until-done 해제는
#     판정 규칙이 아니라 오케스트레이션 계약이라, 산출물이 `rubric.loop_until_done: false` 로 밝힌다.
#   ★ E 는 어느 authority 에서도 **삭제되지 않는다** — rubric.reference_E 로 칸 비교용 상수 보존
#     (R0~R3 사다리 비교의 성립 조건 = 모든 칸이 같은 E 로 재어졌다는 것. 우선순위만 바뀌고 상수는 불변).
#   ★ 출력의 rubric.authority 로 **어느 권한에서 잰 판정인지 산출물이 스스로 밝힌다**(헌법 §결정론 규율
#     — 출처 표시). 표시 없는 권한 전환 금지.
#   PASS  : M ≥ primary × (1 − tol)
#   REFUTE: M <  primary × (1 − tol)
#   establish 실패(E·target·expected 모두 부재/불가) → failure_axis=establish → (c) 사용자 백스톱.
#
# ★★ 공허 PASS 원천 차단 (plan_26082219 · D1) — 이 파일의 **본체 불변식**:
#   이전 구현은 두 개의 **파이썬 falsy 우연**에 판정이 얹혀 있었다 — `0.0 is not None`(True) 로
#   `--target-tps 0` 이 정본으로 낙찰되고, `if primary`(False) 로 ratio 가 사라졌다. 그 결과
#   floor=0 · ratio=null 인 **어떤 측정치도 통과하는 PASS**(=공허 PASS)가 만들어졌고, 그것이
#   completion_gate 의 promotion-ready(=hint 발행 자격)를 열었다(2026-08-22 실측).
#   처방은 `target_tps` 특례가 아니라 **사다리 후보 유효성의 단일 술어**(rubric_candidate)다 —
#   E·c·expected **세 칸 모두** 같은 술어를 통과해야 사다리에 오른다. 특례로 고치면 나머지 두 칸이
#   침묵 누락으로 남는다(헌법 §결정론 규율 4종 안티패턴 — '폴백'의 결함 칸).
#   ⇒ primary 는 항상 유한 양수이며, 따라서 `floor > 0` 과 `ratio ≠ null` 이 **구조적으로 보장**된다.
#
# like-with-like: spec on 서브는 R_token, off 서브는 R_fp 기준(루프라인이 이미 accept_len 반영해 산출).
# ★ spec 축의 **선언**을 입력으로 받는다(`--spec-declared on|off|unknown` · 2026-09-14 · plan_26091407 §4.1):
#   accept_len 은 루프라인에서 R_token = accept_len × R_fp 와 expected_achievable = realistic_fraction ×
#   accept_len × R_fp **양쪽에** 곱해진다 — E·c 가 없으면 expected 가 primary 이므로 accept_len 은
#   물리 상한만이 아니라 **합격선 자체**를 움직인다. 그래서 그 값의 출처(roofline.accept_len_source)를
#   선언과 대조해 분기한다:
#     off                         → R_fp(결손 아님 — spec 이 없으니 1.0 이 정상이다)
#     on ∧ accept_len 실측/명시   → R_token (명시는 accept_len_source=declared 로 드러난다)
#     on ∧ accept_len 결손/무효   → NEEDS_RUBRIC · reason_code=SPEC_ACCEPT_LEN_MISSING
#                                   (failure_axis=establish · **자동 대체 ✗** — 1.0 으로 메우면 상한과
#                                    합격선이 함께 내려가 판정이 조용히 틀린다)
#     unknown(지문 없는 과거 sweep) → 종전 동작 보존(measured.spec_on 이 고른다) · 출처만 표시
#   off ∧ 실측 spec_on 은 선언↔실측 불일치다 — 측정 > 선언(sweep_bench `_measured_first` 와 같은 규율)으로
#   R_token 을 쓰고 불일치를 **기재**한다(게이트로 격상하지 않는다).
#   off ∧ 실측 spec_off 인데 accept_len > 1(대개 사람 명시)이 곱해졌으면 상한은 R_fp 이되 그 사실을
#   `spec_axis.mismatch` 에 기재한다("1.0 이 정상" 이라고 서술하지 않는다 · 게이트 ✗).
#   SPEC_ACCEPT_LEN_MISSING 의 **소유는 이 파일**이다 — 다른 곳은 이 상수를 인용만 한다.
# E-search 상태 표면(--e-search hit|empty|no): 외부검색(E) 시도 여부를 출력에 *기록*한다 —
#   roofline-only 강등(reference/target 부재)이 침묵으로 지나가지 않게 warning 필드로 표면화(self-preference 차단).
#   verdict 자체는 불변(warning-only — 결정론 게이트 보존). egress-restricted 서브 = --e-search empty 로
#   음성정직 기록(egress-online+A2A 위임 서브는 --e-search hit 자율 시도 — plan_26070809_46_57 이중게이트).
# 노드간 VRAM 밸런스 축(--node-vram-gib, plan_26070809_47_07 §4.8): decode-tps 축과 **직교** — 미지정 시 비활성
#   (기존 판정 완전 보존). balance_dev=(max-min)/max > --balance-tol(기본 0.10) → REFUTE(failure_axis="balance").
#   ★ explore 에서 이 축은 **게이트가 아니라 서술**이다(SKILL.md §2.1 · plan_26082219 U1) — 값은 종전대로
#     전부 기록하되(balance.pass=false · gates_verdict=false) verdict 를 뒤집지 않는다. weak/explicit 은
#     종전 그대로 REFUTE(failure_axis="balance"). 출력의 balance.gates_verdict 가 어느 쪽인지 스스로 밝힌다.
# CONTRACT: 출력 verdict JSON. stdlib only. `--self-test` = 결정론 단위 자체검사(픽스처 파일 불요).
import argparse, json, math, os, sys

# accept_len 출처 어휘·유효성 술어의 **소유는 roofline.py**(그 필드를 내는 곳)다 — 여기서 목록을 다시
# 적지 않고 import 한다. 두 벌이 되면 한쪽만 늘어나는 순간 판정기가 새 출처를 모르는 값으로 읽는다.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from roofline import (ACCEPT_LEN_SOURCES, ACCEPT_LEN_VALUE_SOURCES,  # noqa: E402
                      accept_len_state)

# 사다리 칸 라벨 — **한 곳에만** 적는다(같은 개념이 두 곳에 손으로 적히면 매직넘버 결함 칸).
SRC_E = "E(external_reference)"
SRC_C = "c(user_target)"
SRC_EXPECTED = "expected_achievable(roofline×MBU)"

AUTHORITIES = ("weak", "explicit", "explore")

# spec 축 선언의 닫힌 값역 — sweep_index.meta.spec_declared 의 값이 그대로 들어온다(부재 = unknown).
SPEC_DECLARED = ("on", "off", "unknown")
# 사유 코드(기계 분기용) — **소유는 이 파일**. judge_bench·testlog·E2E 술어는 인용만 한다.
REASON_SPEC_ACCEPT_LEN_MISSING = "SPEC_ACCEPT_LEN_MISSING"

AUTHORITY_NOTE = {
    "weak": "weak(기본): 외부 레퍼런스(E)가 정본",
    "explicit": "explicit: 사용자 HITL 목표(c)가 정본 — 달성 시 perf_waiver 불요",
    "explore": ("explore: 목표 미설정(광범위 탐색) — 문턱을 세우지 않는다(성능·밸런스 축 **모두 서술**). "
                "loop-until-done 해제. "
                "승격 자격은 성능 판정이 아니라 **서빙 성립(기능) + 유효 측정**이다(SKILL.md §2)."),
}


def _load(p, label):
    try:
        with open(p, "r", encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        sys.stderr.write("[verdict] ERROR %s: %s\n" % (label, e))
        sys.exit(2)


# =============================================================================
# D1 — 사다리 후보 유효성 술어 (단일 소유 · 순수함수)
# =============================================================================

def rubric_candidate(source_label, raw):
    """사다리 한 칸의 (state, value) 를 결정한다. 순수함수 — I/O·전역 없음.

    state:
      'unset'   : 게이트 미설정(정상 상태 — 그 칸이 그냥 없다)
      'invalid' : 값은 있으나 루브릭이 될 수 없다(≤0 · NaN · ±Inf · 비수치)
                  → **절대 조용히 넘기지 않는다**(CLI 입력이면 exit 2, 상류 산출물이면 사다리 제외+기록)
      'valid'   : 사다리 후보(유한 양수)

    `0.0 is not None` 같은 파이썬 falsy 우연이 판정에 관여할 수 없게 만드는 것이 이 술어의 전부다.
    source_label 은 호출부 가독성/후보 기록용이며 판정에 관여하지 않는다.
    """
    del source_label  # 판정 비참여 — 기록 라벨은 호출부가 붙인다
    if raw is None:
        return ("unset", None)
    if isinstance(raw, bool):  # bool 은 int 의 서브클래스 — 수치로 받아들이지 않는다
        return ("invalid", raw)
    try:
        v = float(raw)
    except (TypeError, ValueError):
        return ("invalid", raw)
    if math.isnan(v) or math.isinf(v):
        return ("invalid", raw)
    if v <= 0.0:
        return ("invalid", v)
    return ("valid", v)


def _finite_measure(raw):
    """측정치 M 유한성. (ok, value). NaN/Inf 는 비표준 JSON 유입 + `NaN >= floor`=False 로
    조용한 REFUTE 가 되므로 INVALID 경로로 보낸다."""
    if raw is None or isinstance(raw, bool):
        return (False, None)
    try:
        v = float(raw)
    except (TypeError, ValueError):
        return (False, None)
    if math.isnan(v) or math.isinf(v):
        return (False, None)
    return (True, v)


# =============================================================================
# D3 — CLI 입력 계약 (loud reject · fail-closed)
# =============================================================================

def _validate_cli(authority, reference_tps, target_tps, tolerance, node_vram_gib, balance_tol,
                  spec_declared="unknown"):
    """(opts, errors) 반환. errors 가 비지 않으면 호출부가 exit 2 한다. 순수함수.

    ★ `--target-tps 0` 을 explore 로 **자동 매핑하지 않는다**(loud reject) — 트리거는 사용자만
      당긴다(SKILL.md §2). 자동 매핑은 에이전트/스크립트가 authority 를 전환하는 것이고, 산출물에서
      사용자가 weak 를 의도했는지 explore 를 의도했는지 구분 불가하게 만든다(표시 없는 권한 전환 금지).
    """
    errors = []

    if authority not in AUTHORITIES:
        errors.append("--authority 는 %s 중 하나여야 한다(받은 값: %r)" % ("|".join(AUTHORITIES), authority))

    if authority == "explore" and target_tps is not None:
        errors.append("--authority explore 인데 --target-tps 가 주어졌다 — explore 는 정의상 "
                      "**목표를 세우지 않는 상태**다(모순). 목표를 세우려면 --authority explicit 을 쓰라.")
    if authority == "explicit" and target_tps is None:
        errors.append("--authority explicit 인데 --target-tps 부재 — 명시적 권한은 사용자 목표(c)를 "
                      "정본으로 요구한다(fail-closed).")

    for flag, raw in (("--target-tps", target_tps), ("--reference-tps", reference_tps)):
        if raw is None:
            continue
        state, _ = rubric_candidate(flag, raw)
        if state != "valid":
            errors.append("%s=%r 은 루브릭이 될 수 없다(유한 양수 아님) — 이 값이 정본으로 낙찰되면 "
                          "floor=0 인 **공허 PASS**(어떤 측정치도 통과)가 만들어진다. "
                          "목표 미설정(광범위 탐색)을 원하면 `--authority explore` 를 쓰고 "
                          "--target-tps 는 넘기지 마십시오." % (flag, raw))

    tol_ok, tol_v = _finite_measure(tolerance)
    if not tol_ok or not (0.0 <= tol_v < 1.0):
        errors.append("--tolerance=%r 는 0 ≤ tol < 1 이어야 한다 — tol=1 이면 floor=primary×0=0 이라 "
                      "루브릭 값과 무관하게 **공허 PASS** 가 된다(같은 결함의 두 번째 입구)." % (tolerance,))
        tol_v = None

    if spec_declared not in SPEC_DECLARED:
        errors.append("--spec-declared 는 %s 중 하나여야 한다(받은 값: %r)"
                      % ("|".join(SPEC_DECLARED), spec_declared))

    node_vals = None
    if node_vram_gib:
        try:
            node_vals = [float(x) for x in str(node_vram_gib).split(",") if x.strip() != ""]
        except ValueError:
            errors.append("--node-vram-gib 파싱 실패: %r" % (node_vram_gib,))
            node_vals = None
        else:
            if len(node_vals) < 2:
                errors.append("--node-vram-gib 은 2개 이상 노드값 필요(멀티노드 전용)")
                node_vals = None

    opts = {
        "authority": authority,
        "reference_tps": reference_tps,
        "target_tps": target_tps,
        "tolerance": tol_v,
        "node_vram": node_vals,
        "balance_tol": balance_tol,
        "spec_declared": spec_declared,
    }
    return opts, errors


# =============================================================================
# spec 축 — 선언 × accept_len 출처 → 물리 상한 선택 · 결손 판정 (순수함수 · 2026-09-14)
# =============================================================================

def resolve_spec_axis(spec_declared, roofline, measured):
    """spec 축을 해소한다. 순수함수 — I/O·전역 없음.

    반환 dict(판정 산출물 `rubric.spec_axis` 로 그대로 실린다 — 출처 표시):
      declared            : on|off|unknown (입력 그대로)
      accept_len          : roofline.accept_len (루프라인이 R_token·expected 에 곱한 값)
      accept_len_source   : roofline.accept_len_source (구세대 roofline 은 None)
      accept_len_evidence : roofline.accept_len_evidence
      measured_spec_on    : measured.spec_on (측정이 본 spec 축)
      limit               : 'R_token' | 'R_fp' | None(결손 — 상한을 세울 수 없다)
      basis               : 선택 근거 한 줄
      missing             : True 면 SPEC_ACCEPT_LEN_MISSING
      mismatch            : 선언↔실측 불일치 기재(없으면 None)

    ★ 출처 없는 accept_len 은 실측으로 승격하지 않는다 — `accept_len_source` 가 없는 구세대 roofline 은
      값이 2.48 이어도 "누가 적었는지" 를 말하지 못한다. 그래서 on 에서는 결손이다(unknown 은 종전 보존).
    """
    src = roofline.get("accept_len_source")
    raw = roofline.get("accept_len")
    state, al_val = accept_len_state(raw)
    measured_spec_on = bool(measured.get("spec_on"))
    has_value = src in ACCEPT_LEN_VALUE_SOURCES and state == "valid"
    axis = {"declared": spec_declared, "accept_len": raw, "accept_len_source": src,
            "accept_len_evidence": roofline.get("accept_len_evidence"),
            "measured_spec_on": measured_spec_on,
            "limit": None, "basis": None, "missing": False, "mismatch": None}
    if spec_declared == "on":
        if has_value:
            axis["limit"] = "R_token"
            axis["basis"] = "spec 선언 on ∧ accept_len 출처=%s" % src
        else:
            axis["missing"] = True
            if src not in ACCEPT_LEN_SOURCES:
                why = "roofline.accept_len_source 부재(구세대 산출물 — 값의 출처를 말하지 못한다)"
            elif src in ACCEPT_LEN_VALUE_SOURCES:
                why = "accept_len=%r 무효(유한 ∧ ≥1 아님)" % (raw,)
            else:
                why = "accept_len 출처=%s(실측·명시 없음)" % src
            axis["basis"] = "spec 선언 on 인데 %s" % why
    elif spec_declared == "off":
        if measured_spec_on:
            # 선언↔실측 불일치 — 측정 > 선언. 기재하고 게이트로 격상하지 않는다.
            axis["limit"] = "R_token"
            axis["basis"] = "선언 off ↔ 실측 spec_on — 측정 우선"
            axis["mismatch"] = ("YES(declared=off · measured spec_on accept_len=%r)"
                                % (measured.get("accept_len"),))
        elif has_value and al_val > 1.0:
            # 선언 off ∧ 실측 spec_off 인데 루프라인에는 1 을 넘는 accept_len(대개 사람 명시)이 곱해졌다 —
            #   R_token·expected_achievable(합격선)이 그 값만큼 올라가 있다. 상한은 실측대로 R_fp 로 두되
            #   "1.0 이 정상" 이라고 서술하면 거짓이므로 **그대로 기재**한다(게이트 ✗ · 2026-09-14 리뷰 정정).
            axis["limit"] = "R_fp"
            axis["basis"] = ("선언 off ∧ 실측 spec_off 인데 accept_len 출처=%s 값=%r 이 R_token·expected_achievable "
                             "에 곱해졌다 — 상한은 R_fp" % (src, raw))
            axis["mismatch"] = "YES(declared=off · accept_len_source=%s accept_len=%r)" % (src, raw)
        else:
            axis["limit"] = "R_fp"
            axis["basis"] = "spec 미선언(off) — accept_len 1.0 이 정상(결손 아님)"
    else:
        # unknown: 지문 없는 과거 sweep — 오늘의 동작(measured.spec_on)을 보존하고 출처만 표시한다.
        axis["limit"] = "R_token" if measured_spec_on else "R_fp"
        axis["basis"] = "spec 선언 지문 부재(unknown) — 종전 규율(measured.spec_on) 보존"
    return axis


# =============================================================================
# D6 — 순수 판정 본체 (파일·전역·시계 접근 없음 → 자체검사가 직접 호출한다)
# =============================================================================

def _build_ladder(authority, reference_tps, target_tps, expected):
    """(ladder, candidates) 반환.
    ladder = 실제 낙찰 후보 [(label, value)] (valid 만) · candidates = 3-state 전수 기록(표면화).
    """
    e_state, e_val = rubric_candidate(SRC_E, reference_tps)
    x_state, x_val = rubric_candidate(SRC_EXPECTED, expected)

    if authority == "explore":
        # ★ explore 에서 c 칸은 **사다리에서 제거**된다 — explore 의 정의가 "목표를 세우지 않는다"
        #   이므로 c 는 존재해서는 안 되는 값이다. 칸을 남겨두면 "explore 인데 목표가 있다"는 모순
        #   상태가 표현 가능해지고, 그 모순이 다시 침묵 폴백의 자리가 된다.
        slots = [(SRC_E, e_state, e_val),
                 (SRC_C, "absent-by-authority", None),
                 (SRC_EXPECTED, x_state, x_val)]
        order = [SRC_E, SRC_EXPECTED]
    else:
        c_state, c_val = rubric_candidate(SRC_C, target_tps)
        if authority == "explicit":
            slots = [(SRC_C, c_state, c_val), (SRC_E, e_state, e_val), (SRC_EXPECTED, x_state, x_val)]
            order = [SRC_C, SRC_E, SRC_EXPECTED]
        else:
            slots = [(SRC_E, e_state, e_val), (SRC_C, c_state, c_val), (SRC_EXPECTED, x_state, x_val)]
            order = [SRC_E, SRC_C, SRC_EXPECTED]

    by_label = {label: (state, val) for label, state, val in slots}
    ladder = []
    for label in order:
        state, val = by_label[label]
        if state == "valid":
            ladder.append((label, val))

    candidates = []
    for label, state, val in slots:
        entry = {"source": label, "state": state}
        if state in ("valid", "invalid"):
            entry["value"] = val
        candidates.append(entry)
    return ladder, candidates


def build_verdict(measured, roofline, opts):
    """순수함수 — measured/roofline dict + 검증된 opts 로 verdict 객체를 만든다.

    호출 전제: opts 는 `_validate_cli` 를 통과한 것(tolerance 유한 ∧ 0≤tol<1, E/c 는 valid or None).
    """
    authority = opts["authority"]
    tol = opts["tolerance"]
    e_search = opts.get("e_search", "no")

    M_ok, M = _finite_measure(measured.get("decode_tps"))
    spec_on = bool(measured.get("spec_on"))
    accept_M = measured.get("accept_len")
    R_fp = roofline.get("R_fp")
    R_token = roofline.get("R_token")
    expected = roofline.get("expected_achievable")

    reasons, refuted_claims, notes = [], [], []

    # --- 측정 유효성(INVALID) — NaN/Inf 도 여기서 걸린다(조용한 REFUTE 금지) ---
    if not measured.get("measurement_ok") or not M_ok:
        return {
            "verdict": "INVALID", "failure_axis": None, "structural_or_strategy": None,
            "reason": "측정 실패(completed=%s failed=%s decode_tps=%r) — 재측정 필요" % (
                measured.get("completed"), measured.get("failed"), measured.get("decode_tps")),
            "measured_decode_tps": measured.get("decode_tps"), "rubric": None,
            "reason_code": None,
            "e_search": e_search,
        }

    ladder, candidates = _build_ladder(authority, opts["reference_tps"], opts["target_tps"], expected)
    loop_until_done = authority != "explore"
    # ★ explore 는 c 칸이 구조적으로 부재하므로 target_c 는 항상 null 이다.
    target_c_out = None if authority == "explore" else opts["target_tps"]

    rubric_common = {
        "tolerance": tol,
        "R_fp": R_fp, "R_token": R_token, "expected_achievable": expected,
        # E 는 authority 와 무관하게 **항상** 실린다 — 칸 비교용 상수(SKILL.md §2).
        "reference_E": opts["reference_tps"], "target_c": target_c_out,
        "authority": authority,
        "authority_note": AUTHORITY_NOTE[authority],
        "loop_until_done": loop_until_done,
        "candidates": candidates,
    }
    spec_axis = resolve_spec_axis(opts.get("spec_declared", "unknown"), roofline, measured)
    rubric_common["spec_axis"] = spec_axis

    # --- spec 선언 ∧ accept_len 결손 → 루브릭 못 세움 (SPEC_ACCEPT_LEN_MISSING) ---
    # ★ 사다리 판정보다 **앞**이다: accept_len 이 expected_achievable 에도 곱해지므로 결손이면 expected 칸
    #   자체가 틀린 값이고, E·c 가 낙찰돼도 물리 상한(R_token)을 세울 수 없다. 1.0 으로 대체하지 않는다.
    if spec_axis["missing"]:
        rub = dict(rubric_common)
        rub.update({"primary": None, "source": None, "floor": None, "ratio_M_over_primary": None})
        return {
            "verdict": "NEEDS_RUBRIC", "failure_axis": "establish",
            "reason_code": REASON_SPEC_ACCEPT_LEN_MISSING,
            "structural_or_strategy": None,
            "reason": ("루브릭 못 세움(%s): %s. accept_len 은 R_token 과 expected_achievable"
                       "(= realistic_fraction × accept_len × R_fp) 양쪽에 곱해지므로 1.0 으로 메우면 "
                       "물리 상한과 합격선이 함께 내려간다 — 자동 대체하지 않는다."
                       % (REASON_SPEC_ACCEPT_LEN_MISSING, spec_axis["basis"])),
            "measured_decode_tps": M,
            "measured_spec_on": spec_on,
            "measured_accept_len": accept_M,
            "rubric": rub,
            "authority": authority,
            "e_search": e_search,
            "ask_user": ("spec(speculative) 이 선언된 서빙인데 accept_len 실측이 없습니다. 판정 레벨의 "
                         "측정이 수용길이를 싣도록 재측정하거나(lite 레그 `vllm bench serve` 의 "
                         "spec_decode_acceptance_length), 값을 알고 있으면 `judge_bench.sh --accept-len L` "
                         "로 명시하세요(출처는 declared 로 기록됩니다)."),
        }

    # --- establish 실패 → 사용자 백스톱 ---
    # ★ 상류 산출물(roofline.expected_achievable)의 invalid 는 exit 하지 않는다 — 우리 CLI 입력이
    #   아니기 때문. 사다리에서 제외하고 candidates[] 에 invalid 로 **기록**한 뒤, valid 후보가 하나도
    #   없으면 NEEDS_RUBRIC 이다. 이것이 fail-loud 폴백(정당 칸)이며 조용한 제외(결함 칸)와 다르다.
    if not ladder:
        rub = dict(rubric_common)
        rub.update({"primary": None, "source": None, "floor": None, "ratio_M_over_primary": None})
        return {
            "verdict": "NEEDS_RUBRIC", "failure_axis": "establish",
            "structural_or_strategy": None,
            "reason": "루브릭 못 세움: 사다리의 유효 후보가 없다(부재 또는 invalid) → (c) 사용자 백스톱 필요",
            "reason_code": None,
            "measured_decode_tps": M, "rubric": rub,
            "authority": authority,
            "e_search": e_search,
            "ask_user": "동일 HW(%s, tp=%s)에서 이 모델의 정상 디코드 t/s 레퍼런스를 제공해 주세요." % (
                roofline.get("gpu_model"), roofline.get("tp")),
        }

    primary_src, primary = ladder[0]

    # ── 루브릭의 **물리 타당성** 검사 (2026-09-03 신설 · testlog_26090117 후속 체크박스 해소) ──
    #
    # 결함의 실측(2026-09-01 · devlog_26090117): 커뮤니티 E=59 t/s 를 그대로 문턱으로 넣어 REFUTE 가
    # 났다. 그런데 그 59 t/s 는 **363.5 GB/s 대역폭을 요구**하고 GB10 스펙은 273 GB/s 다 — 이 구성에서
    # 물리적으로 도달 불가능한 목표였다. 게이트는 그것을 그대로 받았다. 사람이 알아채고 손으로
    # like-with-like E 를 정정해 PASS 로 뒤집었고, "verdict_rule 의 E 물리타당성 미검사" 를 후속으로
    # 남겼다. 2026-09-03 재발: 같은 모델의 커뮤니티 수치가 여전히 57~60 t/s 인데 그것들은 전부
    # `--mxfp4-layers moe,qkv,o,lm_head`(dense 까지 MXFP4) + FLASHINFER + CUTLASS 구성이라 active
    # 바이트가 우리 구성(MoE 만 MXFP4 · Marlin · TRITON_ATTN)의 절반이다. **같은 모델·같은 HW 라도
    # 구성이 다르면 루프라인이 다르다** — like-with-like 의 축은 모델·HW 만이 아니다.
    #
    # 처방: 정본으로 낙찰된 문턱이 그 구성의 물리 상한을 넘으면 **루브릭을 못 세운 것**이다(§3 의
    # "루브릭 못 *세움*" 축). 이는 M 이 나쁜 것이 아니므로 REFUTE 가 아니라 NEEDS_RUBRIC 이며,
    # 사용자에게 되묻는다. 판정을 조용히 뒤집지 않고 필요 대역폭을 역산해 **왜** 불가능한지 보인다.
    #   · 비교 대상 R 은 spec 축을 따른다(spec on → R_token · off → R_fp) — like-with-like 의 기존 규율.
    #     축의 선택은 `resolve_spec_axis` 한 곳이 한다(선언 × accept_len 출처 · unknown 은 종전 measured.spec_on).
    #   · R 을 산출할 수 없으면(루프라인 부재) 검사를 건너뛴다 — 없는 근거로 기각하지 않는다.
    _limit_token = spec_axis["limit"] == "R_token"
    R_phys = R_token if _limit_token else R_fp
    physical = {"limit_source": "R_token(spec on)" if _limit_token else "R_fp(spec off)",
                "limit_tps": R_phys, "exceeds": False, "required_bandwidth_gbps": None}
    if isinstance(R_phys, (int, float)) and R_phys > 0 and primary > R_phys:
        physical["exceeds"] = True
        bw = roofline.get("bandwidth_gbps")
        if isinstance(bw, (int, float)) and bw > 0:
            # R 은 대역폭에 선형이다 ⇒ 문턱을 달성하려면 required = bw × (primary / R).
            physical["required_bandwidth_gbps"] = round(bw * primary / R_phys, 1)
        rub = dict(rubric_common)
        rub.update({"primary": primary, "source": primary_src,
                    "floor": None, "ratio_M_over_primary": None, "physical": physical})
        _req = physical["required_bandwidth_gbps"]
        return {
            "verdict": "NEEDS_RUBRIC", "failure_axis": "establish",
            "reason_code": None,
            "structural_or_strategy": None,
            "reason": ("루브릭 못 세움(물리 초과): 정본 문턱 %s t/s [%s] 가 이 구성의 물리 상한 %s t/s [%s] 를 "
                       "넘는다%s. 도달 불가능한 문턱으로 낸 REFUTE 는 측정이 아니라 루브릭의 결함이다."
                       % (primary, primary_src, R_phys, physical["limit_source"],
                          " — 달성하려면 %s GB/s 가 필요하나 이 HW 는 %s GB/s 다"
                          % (_req, roofline.get("bandwidth_gbps")) if _req else "")),
            "measured_decode_tps": M, "rubric": rub,
            "authority": authority,
            "e_search": e_search,
            "ask_user": ("문턱 %s t/s 의 출처가 **이 구성과 like-with-like** 인지 확인해 주세요 "
                         "(같은 모델·HW 라도 양자화 범위·커널 백엔드가 다르면 active 바이트가 달라져 "
                         "루프라인이 달라집니다). like-with-like 수치가 없으면 --e-search empty 로 "
                         "재판정하세요." % primary),
        }

    # ★ primary 는 유한 양수임이 D1 로 보장된다 ⇒ floor > 0 이고 ratio 는 **무조건** 실수다.
    #   (이전 구현의 `if primary` falsy 의존 제거 — 지표가 사라지는 PASS 는 구조적으로 발생 불가.)
    floor = primary * (1.0 - tol)
    passed = M >= floor
    ratio = round(M / primary, 3)
    rubric_common["physical"] = physical      # 통과 경로도 검사 결과를 밝힌다(출처 표시)

    # --- spec-off 강제함수: 모델이 MTP 지원하는데 off 면, R_token 기대 대비 미달을 명시 ---
    spec_hint = None
    if opts.get("spec_supported") and not spec_on and R_token and R_fp and R_token > R_fp * 1.1:
        spec_hint = "speculative(MTP) OFF 감지(accept_len=%s). 지원 모델인데 미사용 → R_token(%s) 대비 단일패스 R_fp(%s)에 묶임. enable 권장." % (
            accept_M, R_token, R_fp)
        notes.append(spec_hint)

    # --- structural vs strategy 힌트(최종은 LLM+HITL — 여기선 결정론 힌트) ---
    if passed:
        verdict, axis, sos = "PASS", "meet", None
        reasons.append("M(%s) ≥ %s×(1−%.2f)=%.2f [%s] → 충족" % (M, primary, tol, floor, primary_src))
    else:
        verdict, axis = "REFUTE", "meet"
        reasons.append("M(%s) < %s×(1−%.2f)=%.2f [%s] (ratio %s) → 기대 이하" % (M, primary, tol, floor, primary_src, ratio))
        refuted_claims.append({
            "claim": "서빙이 충분히 빠르다(production/agent-ready)",
            "reason": "단일스트림 디코드 %s t/s 가 루브릭 %s t/s(%s)의 %s배 — floor %.2f 미달" % (M, primary, primary_src, ratio, floor),
            "evidence": {"measured": measured.get("decode_tps"), "engine_cross": measured.get("engine_gen_throughput_max"),
                         "R_fp": R_fp, "R_token": R_token, "expected": expected},
        })
        if spec_hint:
            sos = "strategy"
            notes.append("처방 후보: --speculative-config(MTP) 활성(recipe 재탐색).")
        elif expected and M < expected * (1.0 - tol) and spec_on:
            sos = "structural?"
            notes.append("spec on 인데도 expected(%s) 한참 아래 → 커널/이미지/백엔드 구조 의심(escalation 후보 — 외부 확증 필요)." % expected)
        else:
            sos = "strategy"
            notes.append("처방 후보: recipe serve-config 재탐색(백엔드·플래그·KV).")
        if not loop_until_done:
            notes.append("explore: loop-until-done 해제 상태 — 이 REFUTE 는 재탐색 강제가 아니라 "
                         "**벽 지도 데이터**다(다음 항목으로 진행 가능). reconciliation_cap 규율은 불변.")

    # --- E-search 상태 표면: roofline-only 강등 시 warning (판정 불변 — warning-only) ---
    roofline_primary = primary_src == SRC_EXPECTED
    warning = None
    if roofline_primary:
        if e_search == "no":
            warning = "E-not-attempted: 판정이 roofline-only로 강등됨 — 메인 인스턴스는 외부검색(E) 수행 후 판정(egress-restricted 서브만 예외·--e-search empty 로 기록)"
        elif e_search == "empty":
            warning = "E-attempted-empty: 외부 레퍼런스 부재 기록됨 — roofline-only 판정(음성정직)"

    rub = dict(rubric_common)
    rub.update({"primary": primary, "source": primary_src, "floor": round(floor, 2),
                "ratio_M_over_primary": ratio})

    out = {
        "verdict": verdict,
        "failure_axis": axis,
        "reason_code": None,
        "structural_or_strategy": sos if verdict == "REFUTE" else None,
        "measured_decode_tps": M,
        "measured_spec_on": spec_on,
        "measured_accept_len": accept_M,
        "rubric": rub,
        "e_search": e_search,
        "reasons": reasons,
        "refuted_claims": refuted_claims,
        "diagnosis_hint": notes,
        "note": "게이트=결정론(이 규칙). LLM Devil's Advocate 는 E 외부검색·정성 진단·재탐색힌트만 보탠다(최종 structural/strategy = LLM+HITL).",
    }
    if warning is not None:
        out["warning"] = warning

    # --- 노드간 VRAM 밸런스 축 (직교 — plan_26070809_47_07 §4.8) ---
    # ★ explore 에서 이 축은 **게이트가 아니라 서술**이다(SKILL.md §2.1 · plan_26082219 U1).
    #   explore 의 승격 자격은 *서빙 성립(기능) ∧ 유효 측정* 이고, 성능도 밸런스도 문턱이 아니라
    #   **벽 지도 데이터**다. 그러므로 값은 종전대로 전부 기록하되 verdict 는 뒤집지 않는다.
    #   ⚠ 뒤집으면 이 축이 U1 계약을 **우회해 게이트로 되살아난다**: REFUTE → 인증서 PASS-only 규칙에
    #   걸려 미발행(publish_benchmark_record) → 인증서가 없으면 completion_gate 의 explore 승격 경로
    #   (cert_rubric_authority=='explore')가 닫혀 perf_waiver(사람 서명)를 강요하게 된다.
    #   weak/explicit 은 **종전 거동 그대로**(REFUTE + failure_axis="balance") — 기존 판정 불변.
    node_vals = opts.get("node_vram")
    if node_vals:
        balance_gates = authority != "explore"
        mx, mn = max(node_vals), min(node_vals)
        balance_dev = (mx - mn) / mx if mx else 0.0
        balance_pass = balance_dev <= opts["balance_tol"]
        out["balance"] = {
            "node_vram_gib": node_vals, "max_gib": mx, "min_gib": mn,
            "balance_dev": round(balance_dev, 4), "tolerance": opts["balance_tol"],
            "pass": balance_pass,
            # 출처 표시(헌법 §결정론 규율): 이 축이 verdict 를 뒤집을 권한을 가졌는지 산출물이 스스로 밝힌다.
            # 표시가 없으면 하류(리포트·인증서·사람)가 "밸런스가 통과했다"와 "밸런스를 안 쟀다"와
            # "쟀지만 문턱이 아니었다"를 구분할 수 없다.
            "gates_verdict": balance_gates,
        }
        if not balance_pass and balance_gates:
            out["verdict"] = "REFUTE"
            out["failure_axis"] = "balance"
            out["refuted_claims"].append({
                "claim": "멀티노드 VRAM 배분이 균형적이다(≤%d%% 편차)" % int(opts["balance_tol"] * 100),
                "reason": "balance_dev=%.4f > tolerance=%.2f (max=%.2fGiB min=%.2fGiB)" % (
                    balance_dev, opts["balance_tol"], mx, mn),
                "evidence": out["balance"],
            })
            out["diagnosis_hint"].append(
                "노드간 VRAM 편차 초과 → recipe-explorer 재탐색(per-GPU 클램프 재산정 — plan_26070809_47_07 §4.8).")
        elif not balance_pass:
            # explore: 기각이 아니라 **기록**이다 — refuted_claims 에 넣지 않는다(그 배열은 REFUTE 의
            # 근거 목록이며, PASS 산출물에 기각 항목이 섞이면 판정과 서술이 다시 뒤섞인다).
            out["diagnosis_hint"].append(
                "explore: 노드간 VRAM 편차 초과(balance_dev=%.4f > tol=%.2f · max=%.2fGiB min=%.2fGiB) — "
                "**서술**이다(게이트 ✗). verdict 를 뒤집지 않으며 벽 지도 데이터로만 기록된다"
                "(SKILL.md §2.1 — explore 의 승격 자격은 서빙 성립(기능) ∧ 유효 측정). "
                "균형이 필요하면 recipe-explorer per-GPU 클램프 재산정." % (
                    balance_dev, opts["balance_tol"], mx, mn))

    return out


# =============================================================================
# 자체검사 (--self-test) — plan_26082219 §6.1 T1~T15 + T16(explore 밸런스=서술 · U1 후속)
#   + T17(물리 초과) + T18~T23(spec 축 선언 × accept_len 출처 · plan_26091407 §4.1)
# =============================================================================

_FIX_MEASURED = {"decode_tps": 34.0, "spec_on": True, "accept_len": 1.85,
                 "completed": 16, "failed": 0, "measurement_ok": True}
_FIX_ROOFLINE = {"R_fp": 30.0, "R_token": 45.0, "expected_achievable": 12.6,
                 "gpu_model": "NVIDIA GB10", "tp": 2}


def _st_opts(**kw):
    """자체검사용: _validate_cli 를 **실제로** 통과시킨 opts 를 만든다(검증기를 우회하지 않는다)."""
    base = {"authority": "weak", "reference_tps": None, "target_tps": None,
            "tolerance": 0.15, "node_vram_gib": None, "balance_tol": 0.10, "spec_declared": "unknown"}
    extra = {"spec_supported": kw.pop("spec_supported", False), "e_search": kw.pop("e_search", "no")}
    base.update(kw)
    opts, errors = _validate_cli(**base)
    opts.update(extra)
    return opts, errors


def _self_test():
    failures = []

    def check(tid, cond, detail):
        if not cond:
            failures.append("%s: %s" % (tid, detail))

    def run(tid, measured=None, roofline=None, **kw):
        """(verdict_obj, errors). errors 가 있으면 verdict_obj 는 None."""
        opts, errors = _st_opts(**kw)
        if errors:
            return None, errors
        return build_verdict(measured or _FIX_MEASURED, roofline or _FIX_ROOFLINE, opts), []

    # --- T1: weak, E=41 > c=30 > expected — E 낙찰 · REFUTE(34 < 34.85) ---
    v, e = run("T1", reference_tps=41.0, target_tps=30.0)
    check("T1", not e and v and v["verdict"] == "REFUTE" and v["rubric"]["source"] == SRC_E
          and v["rubric"]["primary"] == 41.0 and abs(v["rubric"]["floor"] - 34.85) < 1e-9,
          "weak 사다리 E 우선/REFUTE 실패: %r" % (e or v.get("rubric")))

    # --- T2: explicit --target-tps 30 — c 낙찰 · PASS · reference_E 보존 ---
    v, e = run("T2", authority="explicit", reference_tps=41.0, target_tps=30.0)
    check("T2", not e and v and v["verdict"] == "PASS" and v["rubric"]["source"] == SRC_C
          and abs(v["rubric"]["floor"] - 25.5) < 1e-9 and v["rubric"]["reference_E"] == 41.0,
          "explicit c 낙찰/PASS/E 보존 실패: %r" % (e or v.get("rubric")))

    # --- T3: explore, E=41 — E 낙찰 · target_c=null · authority/loop_until_done 표면 ---
    v, e = run("T3", authority="explore", reference_tps=41.0)
    check("T3", not e and v and v["rubric"]["source"] == SRC_E and v["rubric"]["target_c"] is None
          and v["rubric"]["authority"] == "explore" and v["rubric"]["loop_until_done"] is False
          and any(c["source"] == SRC_C and c["state"] == "absent-by-authority"
                  for c in v["rubric"]["candidates"]),
          "explore 계약(E 낙찰·c 부재·loop 해제) 실패: %r" % (e or v.get("rubric")))

    # --- T4: explore, E 부재 → expected=12.6 낙찰 · PASS ---
    v, e = run("T4", authority="explore")
    check("T4", not e and v and v["verdict"] == "PASS" and v["rubric"]["source"] == SRC_EXPECTED
          and v["rubric"]["primary"] == 12.6,
          "explore expected 낙찰/PASS 실패: %r" % (e or v.get("rubric")))

    # --- T5: explore + --target-tps 30 → exit 2 (모순) ---
    _, e = _st_opts(authority="explore", target_tps=30.0)
    check("T5", bool(e), "explore + --target-tps 가 거부되지 않았다")

    # --- T6: explicit + --target-tps 부재 → exit 2 (현행 거동 보존) ---
    _, e = _st_opts(authority="explicit")
    check("T6", bool(e), "explicit + target 부재가 거부되지 않았다(현행 fail-closed 회귀)")

    # --- T7 ★: --target-tps 0 (weak/explicit/explore 전부) → exit 2 ★공허 PASS 회귀 테스트 본체 ---
    for auth in AUTHORITIES:
        _, e = _st_opts(authority=auth, target_tps=0.0)
        check("T7[%s]" % auth, bool(e), "--target-tps 0 이 거부되지 않았다(공허 PASS 재발)")

    # --- T8: --target-tps -5 → exit 2 ---
    _, e = _st_opts(target_tps=-5.0)
    check("T8", bool(e), "--target-tps -5 가 거부되지 않았다")

    # --- T9 ★: --reference-tps 0 (weak) → exit 2 (§1.2 E 칸) ---
    _, e = _st_opts(reference_tps=0.0)
    check("T9", bool(e), "--reference-tps 0 이 거부되지 않았다(E 칸 공허 PASS)")

    # --- T10 ★: E·c 부재 ∧ expected=0 → NEEDS_RUBRIC(establish) · candidates 에 invalid 기록 ---
    rf0 = dict(_FIX_ROOFLINE, expected_achievable=0)
    v, e = run("T10", roofline=rf0)
    check("T10", not e and v and v["verdict"] == "NEEDS_RUBRIC" and v["failure_axis"] == "establish"
          and any(c["source"] == SRC_EXPECTED and c["state"] == "invalid"
                  for c in (v.get("rubric") or {}).get("candidates", [])),
          "expected=0 이 NEEDS_RUBRIC+invalid 기록으로 잡히지 않았다: %r" % (e or v))

    # --- T11: --target-tps nan / inf → exit 2 (JSON 비표준 값 유입 차단) ---
    for bad in (float("nan"), float("inf"), float("-inf")):
        _, e = _st_opts(target_tps=bad)
        check("T11[%r]" % bad, bool(e), "비유한 --target-tps 가 거부되지 않았다")

    # --- T12 ★: 정상 경로 전부에서 floor>0 ∧ ratio≠null (공허 PASS 구조적 불가) ---
    normals = [
        ("T12-a", dict(reference_tps=41.0, target_tps=30.0)),
        ("T12-b", dict(authority="explicit", reference_tps=41.0, target_tps=30.0)),
        ("T12-c", dict(authority="explore", reference_tps=41.0)),
        ("T12-d", dict(authority="explore")),
    ]
    for tid, kw in normals:
        v, e = run(tid, **kw)
        rub = (v or {}).get("rubric") or {}
        check(tid, not e and rub.get("floor") is not None and rub["floor"] > 0
              and rub.get("ratio_M_over_primary") is not None,
              "불변식 위반(floor>0 ∧ ratio≠null): %r" % (e or rub))

    # --- T13: 밸런스 축 판정 불변(직교 축 회귀 · weak 은 **게이트**로 남는다) ---
    v, e = run("T13", reference_tps=41.0, node_vram_gib="60.1,66.8", balance_tol=0.10)
    check("T13", not e and v and v["failure_axis"] == "balance" and v["verdict"] == "REFUTE"
          and v["balance"]["pass"] is False and v["balance"]["gates_verdict"] is True,
          "밸런스 축 회귀: %r" % (e or (v or {}).get("balance")))
    v, e = run("T13b", reference_tps=41.0, node_vram_gib="66.0,66.8", balance_tol=0.10)
    check("T13b", not e and v and v["balance"]["pass"] is True and v["failure_axis"] != "balance",
          "밸런스 축 정상범위 위양성: %r" % (e or (v or {}).get("balance")))
    # T13c: explicit 도 종전대로 게이트다(기존 판정 보존 — 완화는 explore 에만)
    v, e = run("T13c", authority="explicit", reference_tps=41.0, target_tps=30.0,
               node_vram_gib="60.1,66.8", balance_tol=0.10)
    check("T13c", not e and v and v["verdict"] == "REFUTE" and v["failure_axis"] == "balance"
          and v["balance"]["gates_verdict"] is True,
          "explicit 밸런스 게이트가 완화됐다(범위 밖 변경): %r" % (e or (v or {}).get("balance")))

    # --- T16 ★: explore 에서 밸런스 축은 **서술**이다(U1) — 기록은 남되 verdict 를 안 뒤집는다 ---
    #   T13 과 **동일 입력**(60.1,66.8)이며 authority 만 다르다: weak=REFUTE(게이트) vs explore=PASS(서술).
    #   이 대조가 무너지면 밸런스 축이 U1 계약을 우회해 게이트로 되살아난 것이다(인증서 미발행 →
    #   completion_gate 의 explore 승격 경로 차단 → perf_waiver 강요).
    v, e = run("T16", authority="explore", node_vram_gib="60.1,66.8", balance_tol=0.10)
    check("T16", not e and v and v["verdict"] == "PASS" and v["failure_axis"] != "balance"
          and v["balance"]["pass"] is False and v["balance"]["gates_verdict"] is False
          and not any("VRAM" in (c.get("claim") or "") for c in v["refuted_claims"])
          and any("서술" in h for h in v["diagnosis_hint"]),
          "explore 밸런스 서술 계약 실패: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "axis": (v or {}).get("failure_axis"),
                    "balance": (v or {}).get("balance")}))

    # --- T16b: explore + 디코드 미달 — 밸런스가 decode 축 판정/failure_axis 를 오염시키지 않는다 ---
    v, e = run("T16b", authority="explore", reference_tps=41.0, node_vram_gib="60.1,66.8")
    check("T16b", not e and v and v["verdict"] == "REFUTE" and v["failure_axis"] == "meet"
          and v["balance"]["pass"] is False and v["balance"]["gates_verdict"] is False,
          "explore 밸런스가 decode 축을 오염시켰다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "axis": (v or {}).get("failure_axis")}))

    # --- T16c: 미지정 시 balance 키 자체가 부재(축 비활성 — 기존 계약) ---
    v, e = run("T16c", authority="explore")
    check("T16c", not e and v and "balance" not in v,
          "node_vram 미지정인데 balance 키가 생겼다: %r" % (e or (v or {}).get("balance")))

    # --- T17 ★: 물리 초과 문턱 → NEEDS_RUBRIC(REFUTE 아님) · 픽스처 R_fp=30 / R_token=45 ---
    #   실측 근거(devlog_26090117): E=59 t/s 가 363.5 GB/s 를 요구하는데 HW 는 273 GB/s 였고 게이트는
    #   그것을 그대로 받아 REFUTE 를 냈다. 도달 불가능한 문턱의 기각은 측정이 아니라 루브릭의 결함이다.
    #   spec 축을 따른다: 픽스처는 spec_on=True 이므로 상한은 R_token=45.
    v, e = run("T17", reference_tps=60.0)          # 60 > R_token 45
    check("T17", not e and v and v["verdict"] == "NEEDS_RUBRIC"
          and v["failure_axis"] == "establish"
          and v["rubric"]["physical"]["exceeds"] is True
          and v["rubric"]["floor"] is None and v["rubric"]["ratio_M_over_primary"] is None
          and "like-with-like" in (v.get("ask_user") or ""),
          "물리 초과 문턱이 NEEDS_RUBRIC 이 아니다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "axis": (v or {}).get("failure_axis"),
                    "physical": ((v or {}).get("rubric") or {}).get("physical")}))

    # --- T17b 음성대조: 상한 **이하** 문턱은 종전대로 판정된다(검사가 정상 경로를 삼키지 않는다) ---
    #   44 < R_token 45 이고 M=34.0 < 44×0.85=37.4 이므로 REFUTE 여야 한다 — NEEDS_RUBRIC 이 아니다.
    v, e = run("T17b", reference_tps=44.0)
    check("T17b", not e and v and v["verdict"] == "REFUTE" and v["failure_axis"] == "meet"
          and v["rubric"]["physical"]["exceeds"] is False
          and v["rubric"]["floor"] is not None,
          "상한 이하 문턱이 종전 판정을 잃었다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "axis": (v or {}).get("failure_axis")}))

    # --- T17c: spec 축을 따른다 — spec off 면 상한은 R_fp(30) 이므로 44 도 초과다 ---
    #   같은 문턱 44 가 spec on 에서는 통과 판정(T17b), off 에서는 물리 초과가 된다. 축을 안 따르면
    #   두 결과가 같아진다(like-with-like 의 기존 규율과 정합).
    _md_off = dict(_FIX_MEASURED); _md_off["spec_on"] = False; _md_off["accept_len"] = 1.0
    v, e = run("T17c", measured=_md_off, reference_tps=44.0)
    check("T17c", not e and v and v["verdict"] == "NEEDS_RUBRIC"
          and v["rubric"]["physical"]["limit_source"].startswith("R_fp"),
          "spec off 에서 R_fp 상한을 쓰지 않았다: %r" % (
              e or {"verdict": (v or {}).get("verdict"),
                    "physical": ((v or {}).get("rubric") or {}).get("physical")}))

    # --- T14: measured.decode_tps = null / NaN → INVALID ---
    for bad in (None, float("nan"), float("inf")):
        md = dict(_FIX_MEASURED, decode_tps=bad)
        v, e = run("T14", measured=md, reference_tps=41.0)
        check("T14[%r]" % bad, not e and v and v["verdict"] == "INVALID",
              "measured decode_tps=%r 가 INVALID 로 가지 않았다: %r" % (bad, e or v))

    # --- T15(계획 외 추가): --tolerance 1.0 → exit 2 (floor=0 의 두 번째 입구 봉쇄) ---
    for bad in (1.0, 1.5, -0.1, float("nan")):
        _, e = _st_opts(reference_tps=41.0, tolerance=bad)
        check("T15[%r]" % bad, bool(e), "--tolerance %r 가 거부되지 않았다(공허 PASS 두 번째 입구)" % bad)

    # =========================================================================
    # T18~ spec 축 선언 × accept_len 출처 (2026-09-14 · plan_26091407 §4.1 · 항목 1)
    #   픽스처 수치는 승자 셀(nv4-bf-262k-res-kv8g-gmu80)의 **모양**을 그대로 쓴다: R_fp=45.03 ·
    #   실측 accept_len 2.4818 → R_token 111.75 · E=53.7(explore) · M=46.7. E 가 R_fp 보다 크고 R_token
    #   보다 작다 — accept_len 을 1.0 으로 떨어뜨리면 **물리 초과 NEEDS_RUBRIC 으로 뒤집히는** 구간이다.
    # =========================================================================
    _acc = 2.481818181818182
    _rfp = 45.03
    def _roof(accept, source, evidence=None):
        r = {"R_fp": _rfp, "R_token": round(accept * _rfp, 2),
             "expected_achievable": round(0.35 * accept * _rfp, 2),
             "accept_len": accept, "gpu_model": "NVIDIA GB10", "tp": 2, "bandwidth_gbps": 273.0}
        if source is not None:
            r["accept_len_source"] = source
            r["accept_len_evidence"] = evidence
        return r
    _m_on = {"decode_tps": 46.7, "spec_on": True, "accept_len": _acc,
             "completed": 16, "failed": 0, "measurement_ok": True}
    _m_absent = {"decode_tps": 46.7, "spec_on": False, "accept_len": None,
                 "completed": 16, "failed": 0, "measurement_ok": True}
    _m_off = {"decode_tps": 20.0, "spec_on": False, "accept_len": None,
              "completed": 16, "failed": 0, "measurement_ok": True}

    def _nr_spec(v):
        return (v and v["verdict"] == "NEEDS_RUBRIC" and v.get("failure_axis") == "establish"
                and v.get("reason_code") == REASON_SPEC_ACCEPT_LEN_MISSING
                and v["rubric"]["floor"] is None and v["rubric"]["primary"] is None
                and v["rubric"]["spec_axis"]["missing"] is True)

    # --- T18 ★: spec 선언 on ∧ accept_len 결손(roofline 출처=absent) → NEEDS_RUBRIC + SPEC_ACCEPT_LEN_MISSING ---
    v, e = run("T18", measured=_m_absent, roofline=_roof(1.0, "absent", "level_01/measured.json accept_len=None"),
               authority="explore", reference_tps=53.7, spec_declared="on", e_search="hit")
    check("T18", not e and _nr_spec(v) and "expected_achievable" in (v.get("reason") or ""),
          "spec on ∧ 결손이 SPEC_ACCEPT_LEN_MISSING 이 아니다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "code": (v or {}).get("reason_code")}))
    # T18b: 출처가 값을 주장해도 값이 무효(<1·NaN)면 결손이다 — 조용히 1.0 으로 눕히지 않는다
    for bad in (0.5, float("nan"), True):
        v, e = run("T18b", measured=_m_absent, roofline=_roof(1.0, "measured") | {"accept_len": bad},
                   authority="explore", reference_tps=53.7, spec_declared="on")
        check("T18b[%r]" % (bad,), not e and _nr_spec(v),
              "무효 accept_len=%r 가 결손으로 잡히지 않았다: %r" % (bad, e or (v or {}).get("verdict")))
    # T18c: 출처 필드 없는 구세대 roofline(값 2.48) — on 에서는 실측으로 승격하지 않는다
    v, e = run("T18c", measured=_m_on, roofline=_roof(_acc, None),
               authority="explore", reference_tps=53.7, spec_declared="on")
    check("T18c", not e and _nr_spec(v) and "부재" in v["rubric"]["spec_axis"]["basis"],
          "출처 없는 accept_len 이 실측으로 승격됐다: %r" % (e or (v or {}).get("rubric", {}).get("spec_axis")))
    # T18d 음성대조: 같은 입력에서 accept_len 이 **실측 승계**면 결손이 아니다 — R_token 111.75 로 PASS
    v, e = run("T18d", measured=_m_on, roofline=_roof(_acc, "measured", "level_01/measured.json"),
               authority="explore", reference_tps=53.7, spec_declared="on", e_search="hit")
    check("T18d", not e and v and v["verdict"] == "PASS" and v.get("reason_code") is None
          and abs(v["rubric"]["physical"]["limit_tps"] - 111.75) < 0.02   # 픽스처 반올림(45.03×2.4818=111.76)
          and v["rubric"]["physical"]["limit_source"].startswith("R_token")
          and v["rubric"]["spec_axis"]["accept_len_source"] == "measured"
          and v["rubric"]["floor"] > 0 and v["rubric"]["ratio_M_over_primary"] is not None,
          "실측 승계가 R_token PASS 로 판정되지 않았다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "physical": ((v or {}).get("rubric") or {}).get("physical")}))

    # --- T19 ★: spec 미선언(off) — 1.0 은 결손이 아니다 → R_fp 상한 · NEEDS_RUBRIC 아님 ---
    v, e = run("T19", measured=_m_off, roofline=_roof(1.0, "declared-absent", "sweep_index.meta.spec_declared=off"),
               authority="explore", spec_declared="off", e_search="empty")
    check("T19", not e and v and v["verdict"] == "PASS" and v.get("reason_code") is None
          and v["rubric"]["physical"]["limit_source"].startswith("R_fp")
          and v["rubric"]["spec_axis"]["missing"] is False
          and v["rubric"]["spec_axis"]["accept_len_source"] == "declared-absent"
          and v["rubric"]["floor"] > 0,
          "spec off 가 R_fp 정상 판정이 아니다: %r" % (
              e or {"verdict": (v or {}).get("verdict"), "axis": ((v or {}).get("rubric") or {}).get("spec_axis")}))
    # T19b: off 인데 실측이 spec_on — 측정 우선(R_token) · 불일치 **기재**(게이트 ✗)
    v, e = run("T19b", measured=_m_on, roofline=_roof(_acc, "measured"),
               authority="explore", reference_tps=53.7, spec_declared="off")
    check("T19b", not e and v and v["verdict"] == "PASS"
          and v["rubric"]["physical"]["limit_source"].startswith("R_token")
          and (v["rubric"]["spec_axis"]["mismatch"] or "").startswith("YES"),
          "선언 off ↔ 실측 on 불일치가 측정 우선·기재로 처리되지 않았다: %r" % (
              e or ((v or {}).get("rubric") or {}).get("spec_axis")))
    # T19c: off ∧ 실측 spec_off 인데 사람 명시 accept_len 3.0 이 곱해졌다 — 상한은 R_fp 이되 basis 가
    #   "1.0 이 정상" 이라고 거짓 서술하지 않고 mismatch 에 기재한다(게이트 ✗ · 판정 동작은 종전과 같다).
    _roof_c = {"R_fp": 30.0, "R_token": 90.0, "expected_achievable": 31.5, "accept_len": 3.0,
               "accept_len_source": "declared", "accept_len_evidence": "argv(--accept-len)"}
    v, e = run("T19c", measured=_m_off, roofline=_roof_c, authority="explore", spec_declared="off")
    _ax = ((v or {}).get("rubric") or {}).get("spec_axis") or {}
    check("T19c", not e and v and _ax.get("limit") == "R_fp"
          and "정상" not in (_ax.get("basis") or "") and "expected_achievable" in (_ax.get("basis") or "")
          and (_ax.get("mismatch") or "").startswith("YES(declared=off · accept_len_source=declared"),
          "off ∧ 명시 accept_len>1 이 '1.0 이 정상' 으로 거짓 서술되거나 기재되지 않았다: %r" % (e or _ax))
    # T19d 음성대조: 같은 off 에서 accept_len 이 declared-absent 1.0 이면 mismatch 는 없고 basis 는 '정상' 이다
    v, e = run("T19d", measured=_m_off, roofline=_roof(1.0, "declared-absent"), authority="explore",
               spec_declared="off")
    _ax = ((v or {}).get("rubric") or {}).get("spec_axis") or {}
    check("T19d", not e and v and _ax.get("mismatch") is None and "정상" in (_ax.get("basis") or ""),
          "off ∧ declared-absent 1.0 에 불일치가 잘못 기재됐다: %r" % (e or _ax))

    # --- T20: 사람 명시(declared) — 실측이 없어도 명시값은 R_token 을 세우고 출처가 드러난다 ---
    v, e = run("T20", measured=_m_absent, roofline=_roof(_acc, "declared", "argv(--accept-len)"),
               authority="explore", reference_tps=53.7, spec_declared="on")
    check("T20", not e and v and v["verdict"] == "PASS"
          and v["rubric"]["spec_axis"]["accept_len_source"] == "declared"
          and v["rubric"]["physical"]["limit_source"].startswith("R_token"),
          "declared provenance 가 R_token 판정으로 드러나지 않았다: %r" % (
              e or ((v or {}).get("rubric") or {}).get("spec_axis")))

    # --- T21: unknown(지문 없는 과거 sweep) — 종전 동작 보존(measured.spec_on 이 고른다) · 출처 표시 ---
    v, e = run("T21", measured=_m_on, roofline=_roof(_acc, None),
               authority="explore", reference_tps=53.7)            # spec_declared 기본 = unknown
    check("T21", not e and v and v["verdict"] == "PASS"
          and v["rubric"]["spec_axis"]["declared"] == "unknown"
          and v["rubric"]["physical"]["limit_source"].startswith("R_token"),
          "unknown 이 종전 동작(measured.spec_on → R_token)을 보존하지 않았다: %r" % (
              e or ((v or {}).get("rubric") or {}).get("spec_axis")))
    # T21b ★역채점(교정 전 결함의 재현): unknown ∧ 루프라인 accept_len 이 조용한 1.0(R_token=R_fp)이면
    #   같은 측정이 **물리 초과 NEEDS_RUBRIC** 으로 뒤집힌다 — 이 대조가 T18d 가 공허하지 않다는 증거다.
    v, e = run("T21b", measured=_m_on, roofline=_roof(1.0, None),
               authority="explore", reference_tps=53.7)
    check("T21b", not e and v and v["verdict"] == "NEEDS_RUBRIC" and v.get("reason_code") is None
          and v["rubric"]["physical"]["exceeds"] is True,
          "조용한 1.0 의 결함 재현(물리 초과 NEEDS_RUBRIC)이 나오지 않았다 — 픽스처가 accept_len 에 둔감하다: %r" % (
              e or (v or {}).get("verdict")))

    # --- T22: --spec-declared 값역 밖 → exit 2 ---
    _, e = _st_opts(spec_declared="maybe")
    check("T22", bool(e), "--spec-declared 값역 밖이 거부되지 않았다")

    # --- T23 ★ 공허 PASS 불변식은 spec 축 경로에서도 유지된다 ---
    for tid, md, rf, kw in (("T23-on", _m_on, _roof(_acc, "measured"), dict(spec_declared="on")),
                            ("T23-off", _m_off, _roof(1.0, "declared-absent"), dict(spec_declared="off")),
                            ("T23-decl", _m_absent, _roof(_acc, "declared"), dict(spec_declared="on"))):
        v, e = run(tid, measured=md, roofline=rf, authority="explore", **kw)
        rub = (v or {}).get("rubric") or {}
        check(tid, not e and v["verdict"] in ("PASS", "REFUTE") and rub.get("floor") is not None
              and rub["floor"] > 0 and rub.get("ratio_M_over_primary") is not None,
              "spec 축 경로에서 불변식 위반(floor>0 ∧ ratio≠null): %r" % (e or rub))

    if failures:
        sys.stderr.write("[verdict --self-test] FAIL %d 건:\n" % len(failures))
        for f in failures:
            sys.stderr.write("  - %s\n" % f)
        return 1
    sys.stdout.write("[verdict --self-test] OK — T1~T23 전부 통과"
                     "(공허 PASS 경로 부재 + explore 밸런스=서술 + spec 축 SPEC_ACCEPT_LEN_MISSING 단언 포함)\n")
    return 0


def main():
    ap = argparse.ArgumentParser(description="결정론 PASS/REFUTE 게이트")
    ap.add_argument("--measured", help="parse_bench.py JSON")
    ap.add_argument("--roofline", help="roofline.py JSON")
    ap.add_argument("--reference-tps", type=float, help="E: 외부 현실-달성치(검증기 (b) 외부검색 산물). "
                                                        "유한 양수만 — 0/음수/NaN 은 exit 2")
    ap.add_argument("--target-tps", type=float, help="c: 사용자 선언 목표(백스톱). 유한 양수만 — "
                                                     "0/NULL 로 '게이트 미설정'을 표현하지 말 것"
                                                     "(그건 --authority explore 다)")
    ap.add_argument("--authority", choices=list(AUTHORITIES), default="weak",
                    help="루브릭 정본 권한(SKILL.md §2). weak(기본)=E>c>expected · "
                         "explicit=c>E>expected · explore=E>expected(c 칸 부재·--target-tps 금지). "
                         "explicit/explore 는 **사용자 HITL 트리거** 시에만 — 에이전트 자기선언 금지. "
                         "E 는 어느 쪽이든 rubric.reference_E 로 보존.")
    ap.add_argument("--tolerance", type=float, default=0.15, help="PASS 허용오차(기본 15%%, 0≤tol<1)")
    ap.add_argument("--spec-supported", action="store_true", help="모델이 speculative(MTP) 지원 — off 면 강제함수")
    ap.add_argument("--spec-declared", choices=list(SPEC_DECLARED), default="unknown",
                    help="서빙 선언의 spec 축(sweep_index.meta.spec_declared 승계). off=R_fp(결손 아님) · "
                         "on=R_token(accept_len 실측/명시 필수 — 결손이면 NEEDS_RUBRIC+%s) · "
                         "unknown(기본 · 지문 없는 과거 sweep)=종전 measured.spec_on 보존"
                         % REASON_SPEC_ACCEPT_LEN_MISSING)
    ap.add_argument("--e-search", choices=["hit", "empty", "no"], default="no",
                    help="외부검색(E) 상태: hit=시도·발견 / empty=시도·빈손 / no=미시도(기본). 출력에 기록(판정 불변)")
    ap.add_argument("--node-vram-gib", default=None,
                    help="멀티노드 노드별 measured VRAM used(GiB), 쉼표구분(예: 60.1,66.8). "
                         "밸런스 축(plan_26070809_47_07 §4.8) — 미지정 시 비활성(기존 decode-tps 판정 완전 보존).")
    ap.add_argument("--balance-tol", type=float, default=0.10,
                    help="노드간 VRAM 밸런스 허용편차(기본 0.10=10%%)")
    ap.add_argument("--self-test", action="store_true",
                    help="결정론 자체검사(T1~T23) — 파일 입력 불요. 0=전부 통과 · 1=실패 나열")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    if args.self_test:
        sys.exit(_self_test())

    if not args.measured or not args.roofline:
        sys.stderr.write("[verdict] ERROR --measured 와 --roofline 은 필수다(--self-test 제외)\n")
        sys.exit(2)

    opts, errors = _validate_cli(args.authority, args.reference_tps, args.target_tps,
                                 args.tolerance, args.node_vram_gib, args.balance_tol,
                                 spec_declared=args.spec_declared)
    if errors:
        for msg in errors:
            sys.stderr.write("[verdict] ERROR %s\n" % msg)
        sys.exit(2)
    opts["spec_supported"] = args.spec_supported
    opts["e_search"] = args.e_search

    m = _load(args.measured, "measured")
    r = _load(args.roofline, "roofline")

    print(json.dumps(build_verdict(m, r, opts), ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
