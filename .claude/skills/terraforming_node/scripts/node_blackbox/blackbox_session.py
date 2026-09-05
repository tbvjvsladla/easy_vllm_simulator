#!/usr/bin/env python3
"""서빙 세션 사이드카 — `docs/logs/<node_id>/sessions/<session_id>.json` + serve_start/stop 이벤트.

근거: plan_26073109 §2.4(`sessions/` = 모델·레시피·토폴로지 태그 · events 의 serve_start/stop).
필요성: plan_26073109_12_47 합격기준 2 — "모델별 rollup + 태그가 존재하고 **조건별 비교가 가능**하다".
사이드카가 없으면 samples/ 는 모델 구분이 없는 한 덩어리라 조건별 비교가 성립하지 않는다.

설계 원칙(기존 평면과 동형):
- **줄마다 반복하지 않는다**: samples CSV 는 1초마다 쓰이므로 모델명을 넣으면 용량이 폭증한다.
  태그는 사이드카에 1회 쓰고, 구간은 [started_utc, stopped_utc] 로 samples 를 자른다.
- **시각은 `--now` 주입만**(벽시계 금지 — docs.md §기계판독 데이터 평면 · staleness_gate 선례 정합).
- **값을 만들어내지 않는다**: 판정(verdict)은 호출자가 준 것만 적는다. 미종료 세션은 stopped_utc=null
  로 남으며 그것이 "아직 안 끝났다"와 "기록이 없다"를 구분한다.
"""
import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone

# 2026-09-03(plan_26090317 P3): 프로젝트 경로를 **root 소유로 굳히지 않기 위해** 조상 소유자를
#   물려주는 디렉터리 생성기를 공유 sibling 모듈에서 가져온다(설치기가 blackbox_eta.py 를
#   /usr/local/sbin 에 sibling 으로 배치한다). 임포트 불가는 치명이 아니다 — 그 경우
#   os.makedirs 로 떨어지되 **그 사실을 숨기지 않는다**(아래 폴백은 loud 하다).
try:
    from blackbox_eta import makedirs_as_ancestor_owner as _mk_owned
except ImportError:  # pragma: no cover - 설치 배선이 깨진 경우
    import os as _os_fb, sys as _sys_fb
    def _mk_owned(path, mode=0o775):
        print("[blackbox] 경고: blackbox_eta sibling 임포트 실패 — 소유권 정렬 없이 디렉터리를 만든다",
              file=_sys_fb.stderr)
        _os_fb.makedirs(path, exist_ok=True)
        return []


ISO_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
SAFE_ID_RE = re.compile(r"^[A-Za-z0-9._-]+$")


def _parse_now(value):
    """ISO-Z 문자열만 받는다. 형식이 어긋나면 조용히 넘기지 않고 죽는다."""
    if not isinstance(value, str) or not ISO_RE.match(value):
        raise SystemExit("--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: %r" % (value,))
    return value


def _epoch(iso):
    return int(datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ")
               .replace(tzinfo=timezone.utc).timestamp())


def _session_path(node_dir, session_id):
    if not SAFE_ID_RE.match(session_id or ""):
        raise SystemExit("session-id 는 [A-Za-z0-9._-]+ 여야 한다: %r" % (session_id,))
    return os.path.join(node_dir, "sessions", session_id + ".json")


def _append_event(node_dir, rec):
    """events/<YYYY-MM>.jsonl 에 append. 기존 평면과 같은 파일·같은 키 규약."""
    month = rec["ts"][:7]
    path = os.path.join(node_dir, "events", month + ".jsonl")
    _mk_owned(os.path.dirname(path))
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    return path


def cmd_start(args):
    now = _parse_now(args.now)
    path = _session_path(args.node_dir, args.session_id)
    if os.path.exists(path) and not args.force:
        raise SystemExit("이미 존재하는 세션이다(덮어쓰기는 --force): %s" % path)
    _mk_owned(os.path.dirname(path))
    doc = {
        "session_id": args.session_id,
        "model": args.model,
        "model_path": args.model_path,
        "recipe": args.recipe,
        "topology": args.topology,
        "tensor_parallel_size": args.tp,
        "config_file": args.config_file,
        "image": args.image,
        "started_utc": now,
        "started_epoch": _epoch(now),
        "stopped_utc": None,
        "stopped_epoch": None,
        "verdict": None,
        "notes": args.note or [],
    }
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    ev = _append_event(args.node_dir, {
        "kind": "serve_start", "ts": now, "source": "blackbox_session",
        "session_id": args.session_id, "model": args.model,
        "topology": args.topology, "tensor_parallel_size": args.tp,
    })
    print("세션 시작: %s" % path)
    print("이벤트   : %s" % ev)
    return 0


def cmd_stop(args):
    now = _parse_now(args.now)
    path = _session_path(args.node_dir, args.session_id)
    if not os.path.isfile(path):
        raise SystemExit("세션 파일이 없다: %s" % path)
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    doc["stopped_utc"] = now
    doc["stopped_epoch"] = _epoch(now)
    doc["verdict"] = args.verdict
    if args.note:
        doc["notes"] = (doc.get("notes") or []) + args.note
    dur = doc["stopped_epoch"] - doc.get("started_epoch", doc["stopped_epoch"])
    doc["duration_s"] = dur
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    ev = _append_event(args.node_dir, {
        "kind": "serve_stop", "ts": now, "source": "blackbox_session",
        "session_id": args.session_id, "model": doc.get("model"),
        "verdict": args.verdict, "duration_s": dur,
    })
    print("세션 종료: %s (%ds · verdict=%s)" % (path, dur, args.verdict))
    print("이벤트   : %s" % ev)
    return 0


BUDGET_FILE = "serve_budget.env"
# 워치독 가드와 같은 값. 여기서는 **경고용 예측**에만 쓴다 — 실제 수락/거부의 권위는 워치독이다.
# ★ 2026-08-18: 옛 판본은 이 두 값을 **손으로 적어** 두고 바로 위 주석에 "두 곳이 판정하면 반드시
#   갈라진다"고 스스로 경고했다. 그리고 갈라졌다 — 정본(blackbox_eta.DEFAULTS)이 3072/8192 로
#   바뀐 뒤 이 파일만 8192/16384 로 남아, 워치독이 **수락한**(arm_ceiling=10729) 선언을 두고
#   "거부된다"고 출력했다(2026-08-18T03:10:50Z 실측: 경고 직후 같은 노드에서 budget_honored ✓).
#   경고가 사실과 반대면 운영자는 정상을 사고로 읽는다 — 4종 안티패턴 `매직넘버·결함`
#   (같은 개념이 두 곳 이상에 손으로 적힌 값)의 교과서 사례다.
# ∴ 사본을 없애고 정본에서 파생한다. 같은 디렉터리에 배달되므로(메인 scripts/node_blackbox,
#   서브 .claude/runtime/node_blackbox) sys.path[0] 로 import 가능하다.
# 2026-09-05(G-B3): 리터럴 폴백을 **삭제**했다. 그 사본은 2026-09-01 감사에서 이미 한 번
#   정본과 갈라진 채 발견됐고(8192/16384 vs 3072/8192), 값을 맞춰 두는 것으로는 재발을 막지
#   못한다 — 두 자리가 있는 한 언젠가 갈라진다. 정본을 못 읽으면 **모르는 것이므로 죽는다**:
#   틀린 상한으로 계산한 예측 경고는 없는 것보다 나쁘다(데몬과 다른 상한을 쓰는 맹점).
try:
    from blackbox_eta import DEFAULTS as _ETA_DEFAULTS
    _WD_MARGIN_MIB = int(_ETA_DEFAULTS["decl_margin_mib"])
    _WD_MIN_CEILING_MIB = int(_ETA_DEFAULTS["decl_min_ceiling_mib"])
    _WD_CONST_SOURCE = "derived:blackbox_eta.DEFAULTS"
except Exception as _exc:
    raise SystemExit(
        "[blackbox_session] FAIL: blackbox_eta.DEFAULTS 에서 선언 상수를 읽지 못했다 — "
        "여기에 사본을 두지 않는다(%s: %s).\n"
        "  → 두 자리에 적힌 상수는 갈라진다(2026-09-01 감사: 사본이 옛 8192/16384 로 남아 있었다).\n"
        "  → 같은 디렉터리의 blackbox_eta.py 가 배달됐는지 확인하라." % (type(_exc).__name__, _exc))
MAX_TTL_S = 86400
# 예산 선언의 기본 TTL. **이 파일이 단일 소유자다**(2026-09-05 · G-B2). 종전에는 같은 7200 이
# run_trial · single_serve_up · budget_renew_loop · multinode_serve_smoke 에 각각 손으로 적혀
# 다섯 자리였다 — 개념이 다섯 곳에 있으면 하나를 고쳐도 나머지가 옛값을 쓴다(margin 거울이
# 실제로 그렇게 갈라져 "설치된 데몬이 옛값" 사고를 냈다). 다른 자리는 `budget-defaults` 로 읽는다.
DEFAULT_TTL_S = 7200

# ── TTL 정합(plan_26081415 C4) ──────────────────────────────────────────────
# TTL 파생 배수. 근거: R0 실측 READY 소요 665 s. **로드 도중 만료**는 그 폴부터 옛 규칙
# (무조건-트립)으로 되돌리므로 사고와 동치다 — 선언이 지켜야 할 최소 수명은 "로드가 끝날
# 때까지"이며, 3배는 로드 변동(NAS 지연·재시도·재컴파일)의 여유다.
# 이 상수는 이 파일에서만 쓰는 국소 상수이고 값이 아니라 **정책**이라 파생 대상이 아니다
# (4종 안티패턴 판정표의 `매직넘버·정당` 칸).
TTL_SAFETY_MULT = 3


def ttl_floor_s(expected_load_s):
    """예상 로드 소요에서 파생한 TTL 하한. 손으로 적지 않는다."""
    return int(expected_load_s) * TTL_SAFETY_MULT


def validate_ttl(ttl_s, expected_load_s=0):
    """declare 와 renew 가 **같은 규칙**을 쓰도록 한 곳에 둔다.

    두 곳에 같은 판정을 적으면 한쪽만 고쳐져 갈라진다 — 갱신 경로가 느슨하면 "선언은 엄격한데
    연장은 아무 값이나"가 되어 검증이 통째로 무의미해진다(4종 안티패턴 `매직넘버·결함`).
    """
    if ttl_s <= 0 or ttl_s > MAX_TTL_S:
        raise SystemExit("--ttl-s 는 1..%d 여야 한다(무기한 선언 금지): %r" % (MAX_TTL_S, ttl_s))
    # ★ TTL 파생 검증(plan_26081415 C4-3). 예상 로드 소요를 알면서 그보다 짧은 TTL 을 받는 것은
    #   "로드 도중 만료" 를 예약하는 것이다 — 만료 순간부터 옛 규칙(무조건-트립)이 재적용되고,
    #   하필 그 지점이 하강 골짜기면 그대로 사살된다. 조용히 늘리지 않고 **거부**한다.
    if expected_load_s:
        floor_ttl = ttl_floor_s(expected_load_s)
        if ttl_s < floor_ttl:
            raise SystemExit(
                "--ttl-s %d 는 예상 로드 %ds 의 %d배(=%ds)보다 짧다 — 로드 도중 만료가 예약된다. "
                "거부한다(조용한 보정 금지)." % (ttl_s, expected_load_s, TTL_SAFETY_MULT, floor_ttl))


def _budget_path(node_dir):
    return os.path.join(node_dir, BUDGET_FILE)


def _read_budget(node_dir):
    """현행 선언을 파싱한다. 워치독과 **같은 문자셋 규약**으로 읽는다(source 하지 않는다).

    순수 파서다 — 없거나 깨졌으면 None 을 돌려주고, 호출부가 fail-closed 로 처리한다
    (4종 안티패턴 판정표의 `폴백·정당` 칸: 순수 파서의 None).
    """
    path = _budget_path(node_dir)
    if not os.path.isfile(path):
        return None
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"^\s*([A-Za-z_]+)\s*=\s*([A-Za-z0-9._-]{1,64})\s*$", line)
            if m:
                out[m.group(1)] = m.group(2)
    if "floor_mib" not in out or "expires_epoch" not in out:
        return None
    try:
        out["floor_mib"] = int(out["floor_mib"])
        out["expires_epoch"] = int(out["expires_epoch"])
    except ValueError:
        return None
    return out


def _write_budget(node_dir, body):
    """★ 원자적 교체 필수. 워치독은 이 파일을 **1초마다** 읽는다. 제자리 쓰기(open 'w')는
    내용이 비거나 잘린 순간을 만들고, 그 폴에서 선언이 거부돼 arm 상한이 무한대로 돌아간다.
    하필 그 순간이 모델 로드 골짜기면 옛 규칙 그대로 사살된다 — 갱신 행위가 사고를 만든다.
    rename 은 같은 파일시스템에서 원자적이므로 워치독은 옛 선언 아니면 새 선언만 본다.
    """
    path = _budget_path(node_dir)
    _mk_owned(node_dir)
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(body)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    return path


def cmd_declare_budget(args):
    """서빙이 남길 **예상 최소 MemAvailable** 을 선언한다 → 워치독 ETA 규칙의 arm 상한이 된다.

    근거 testlog_26073123: ETA 규칙은 `잔량÷하강률` 선형 외삽이라 **유계**인 모델 로드 하강을
    무계로 읽어 58 GiB 급 모델을 3/3 사살했다. 빠진 것은 임계값이 아니라 "바닥이 어디인가"다.

    ★ KV 를 명시하지 않으면 선언할 수 없다(--kv-mib 필수). 이것이 설계의 핵심이다 —
      유일한 진성 트립(KV 벌룬 97 GiB)은 `kv_cache_memory_bytes: null` 이라 선언 불가한
      사건이었고, 선언이 없으면 워치독은 현행 규칙 그대로 동작해 그 벌룬을 잡는다.
      즉 이 선언은 헌법 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY` 를 집행 가능한 형태로 바꾼다.

    ★ **거부도 기록한다**(plan_26081415 C3 기준3 · 침묵 금지). 아래 모든 거부 경로는 stdout 에만
      남아 있었다 — 그러면 사후 분석에서 "선언을 시도조차 안 했다"와 "시도했는데 규칙이 막았다"가
      구분되지 않는다. 두 사실의 처방은 정반대다(배선 추가 vs 입력 교정).
    """
    # ★ 시각만 이 밖에서 판정한다 — ts 없이는 이벤트를 쓸 수 없고(벽시계 금지 · docs.md §기계판독
    #   데이터 평면), 시각을 지어내 기록하면 그 기록 자체가 §결정론 규율의 출처 위조가 된다.
    #   `--now` 형식 위반은 예산 거부가 아니라 호출자 버그이므로 이벤트 없이 죽는 것이 맞다.
    now = _parse_now(args.now)
    try:
        return _declare_budget(args, now)
    except SystemExit as exc:
        _record_declare_rejection(args, now, exc)
        raise


def _safe_label_for_event(label):
    """이벤트에 남길 라벨. 거부 사유가 **라벨 자체**일 수 있으므로 원문을 자르기만 한다 —
    무엇이 거부됐는지가 증거다. events 는 JSON 이라 워치독 파서 문자셋 제약을 받지 않는다."""
    if not label:
        return "unlabeled"
    return str(label)[:64]


def _record_declare_rejection(args, now, exc):
    """`declare-budget` 거부를 events 에 남긴다(kind=`budget_declare_rejected`)."""
    code = getattr(exc, "code", None)
    if code is None or code == 0:
        return  # 정상 종료는 거부가 아니다
    rec = {
        "kind": "budget_declare_rejected", "ts": now, "source": "blackbox_session",
        "reason": str(code),
        "label": _safe_label_for_event(getattr(args, "label", None)),
        "mem_total_mib": getattr(args, "mem_total_mib", None),
        "weights_mib": getattr(args, "weights_mib", None),
        "kv_mib": getattr(args, "kv_mib", None),
        "overhead_mib": getattr(args, "overhead_mib", None),
        "ttl_s": getattr(args, "ttl_s", None),
        "expected_load_s": getattr(args, "expected_load_s", 0) or 0,
    }
    try:
        _append_event(args.node_dir, rec)
    except OSError as e:
        # 기록 실패를 조용히 넘기면 침묵 금지가 한 겹 더 깨진다. 크게 말하되 **원래 거부를 가리지
        # 않는다** — 호출부는 여전히 원 SystemExit 을 받는다.
        print("⚠ 거부 이벤트 기록 실패(%s): %s" % (e, rec["reason"]), file=sys.stderr)


def _declare_budget(args, now):
    validate_ttl(args.ttl_s, getattr(args, "expected_load_s", 0))
    for name, v in (("--mem-total-mib", args.mem_total_mib), ("--weights-mib", args.weights_mib),
                    ("--kv-mib", args.kv_mib), ("--overhead-mib", args.overhead_mib)):
        if v < 0:
            raise SystemExit("%s 는 0 이상이어야 한다: %r" % (name, v))
    resident = args.weights_mib + args.kv_mib + args.overhead_mib
    floor = args.mem_total_mib - resident
    if floor <= 0:
        raise SystemExit(
            "예상 상주 %d MiB 가 총량 %d MiB 이상이다 — 이 구성은 애초에 못 올린다."
            % (resident, args.mem_total_mib))
    expires = _epoch(now) + args.ttl_s
    path = _budget_path(args.node_dir)
    _mk_owned(args.node_dir)
    label = args.label or "unlabeled"
    if not SAFE_ID_RE.match(label):
        raise SystemExit("--label 은 [A-Za-z0-9._-]+ 여야 한다(워치독 파서 문자셋): %r" % (label,))
    body = (
        "# easy-vllm serve budget declaration — 워치독이 sed 로 읽는다(source 하지 않는다).\n"
        "# 산출: floor_mib = mem_total(%d) - [weights(%d) + kv(%d) + overhead(%d)]\n"
        "# 발행 %s · TTL %ds · 근거 testlog_26073123\n"
        "floor_mib=%d\nexpires_epoch=%d\nlabel=%s\n"
        % (args.mem_total_mib, args.weights_mib, args.kv_mib, args.overhead_mib,
           now, args.ttl_s, floor, expires, label))
    # ★ 원자적 교체 필수. 워치독은 이 파일을 **1초마다** 읽는다. 제자리 쓰기(open 'w')는
    #   내용이 비거나 잘린 순간을 만들고, 그 폴에서 선언이 거부돼 arm 상한이 무한대로 돌아간다.
    #   하필 그 순간이 모델 로드 골짜기면 옛 규칙 그대로 사살된다 — 갱신 행위가 사고를 만든다.
    #   rename 은 같은 파일시스템에서 원자적이므로 워치독은 옛 선언 아니면 새 선언만 본다.
    tmp = path + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        f.write(body)
        f.flush()
        os.fsync(f.fileno())
    os.replace(tmp, path)
    ceiling = floor - _WD_MARGIN_MIB
    _append_event(args.node_dir, {
        "kind": "budget_declare", "ts": now, "source": "blackbox_session",
        "label": label, "floor_mib": floor, "expires_epoch": expires,
        "weights_mib": args.weights_mib, "kv_mib": args.kv_mib,
        "overhead_mib": args.overhead_mib, "mem_total_mib": args.mem_total_mib,
    })
    print("선언 발행: %s" % path)
    print("  예상 상주 %d MiB → 예상 바닥 %d MiB · 만료 epoch %d (TTL %ds)"
          % (resident, floor, expires, args.ttl_s))
    print("  워치독 예상 arm 상한 = %d - %d = %d MiB" % (floor, _WD_MARGIN_MIB, ceiling))
    if ceiling < _WD_MIN_CEILING_MIB:
        # 워치독이 거부할 선언이다. 죽이지는 않는다 — 판정 권위는 워치독이고, 여기서 죽이면
        # 두 곳이 같은 규칙을 갖게 되어 반드시 갈라진다. 대신 결과를 정직하게 예고한다.
        print("  ⚠ 상한이 최소 %d MiB 미만 → **워치독이 이 선언을 거부**하고 현행 규칙으로 돈다."
              % _WD_MIN_CEILING_MIB)
    return 0


def cmd_renew_budget(args):
    """현행 선언의 **만료만** 연장한다 — 상주 서빙(`--keep-up`)의 TTL 결속(plan_26081415 C4-3).

    왜 재선언이 아니라 갱신인가: 재선언은 `weights/kv/overhead` 를 다시 받아 **바닥을 다시 계산**한다.
    상주 중에는 그 입력을 다시 구할 경로가 없어(모델은 이미 올라가 있고 스모크는 끝났다) 사람이
    손으로 적게 되고, 그 순간 선언이 실제 상주와 갈린다. 갱신은 **원 산출을 그대로 두고 시계만**
    민다 — 산출 provenance(헤더의 mem_total/weights/kv/overhead 줄)를 파괴하지 않는 것이 요점이다.

    ★ 만료된 선언은 갱신하지 않는다(fail-closed). 만료 = 그 사이 규칙이 이미 옛것으로 돌아갔고
      상주 구성이 그대로라는 보장이 없다는 뜻이다. 되살리려면 `declare-budget` 으로 다시 산출하라.
    """
    now = _parse_now(args.now)
    validate_ttl(args.ttl_s, getattr(args, "expected_load_s", 0))
    path = _budget_path(args.node_dir)
    cur = _read_budget(args.node_dir)
    if cur is None:
        raise SystemExit(
            "갱신할 선언이 없다(또는 파싱 불가): %s — 먼저 declare-budget 하라." % path)
    now_epoch = _epoch(now)
    if cur["expires_epoch"] <= now_epoch:
        raise SystemExit(
            "이미 만료된 선언은 갱신하지 않는다(expires_epoch=%d <= now=%d). "
            "만료 구간 동안 워치독은 옛 규칙으로 돌았고 상주 구성이 그대로라는 보장이 없다 — "
            "declare-budget 으로 바닥을 다시 산출하라." % (cur["expires_epoch"], now_epoch))
    with open(path, encoding="utf-8") as f:
        raw = f.read()
    new_exp = now_epoch + args.ttl_s
    body, n = re.subn(r"(?m)^([ \t]*expires_epoch[ \t]*=[ \t]*)\d+[ \t]*$",
                      r"\g<1>%d" % new_exp, raw)
    if n != 1:
        raise SystemExit(
            "expires_epoch 줄을 정확히 1개 찾지 못했다(%d개) — 손상된 선언이다: %s" % (n, path))
    body += ("# 갱신 %s · TTL %ds → expires_epoch=%d (원 산출·floor 유지)\n"
             % (now, args.ttl_s, new_exp))
    _write_budget(args.node_dir, body)
    _append_event(args.node_dir, {
        "kind": "budget_renew", "ts": now, "source": "blackbox_session",
        "label": cur.get("label", "unlabeled"), "floor_mib": cur["floor_mib"],
        "expires_epoch": new_exp, "prev_expires_epoch": cur["expires_epoch"],
        "remaining_before_s": cur["expires_epoch"] - now_epoch, "ttl_s": args.ttl_s,
    })
    print("선언 갱신: %s" % path)
    print("  바닥 %d MiB 유지 · 만료 %d → %d (갱신 전 잔여 %ds · 새 TTL %ds)"
          % (cur["floor_mib"], cur["expires_epoch"], new_exp,
             cur["expires_epoch"] - now_epoch, args.ttl_s))
    return 0


def cmd_budget_skip(args):
    """**무보호 진입**을 기록한다 — plan_26081415 C3(실패정책 ㄴ + `--no-budget` 탈출구).

    탈출구가 조용하면 탈출구가 아니라 구멍이다. 나중에 사살이 나도 "선언이 없었다"와
    "선언을 일부러 건너뛰었다"를 구분할 수 없으면 사후 분석이 불가능하다.
    """
    now = _parse_now(args.now)
    _append_event(args.node_dir, {
        "kind": "budget_skipped", "ts": now, "source": "blackbox_session",
        "reason": args.reason, "label": args.label or "unlabeled",
    })
    print("무보호 진입 기록: reason=%s (선언 없음 = 워치독 현행 규칙 그대로)" % args.reason)
    return 0


# 차단 단계의 **닫힌 목록**. 새 차단 경로를 만들면 여기 한 줄을 더해야 한다(tripwire — 4종
# 안티패턴 판정표의 `하드코딩·정당` 칸). 자유문자열로 두면 호출처마다 다른 이름을 써서 집계가
# 불가능해지고, 그러면 "차단이 어디서 몇 번 일어났나"를 데이터로 물을 수 없다.
BLOCK_STAGES = ("derive", "preflight_ceiling", "declare")


def cmd_budget_block(args):
    """**진입 차단**을 기록한다 — 선언이 성립하지 않아 로드를 0초도 시작하지 않은 사건.

    `budget_skipped`(무보호로 **진입했다**)와 방향이 반대인 사실이라 kind 를 나눈다. 뭉치면
    사후 분석이 "보호 없이 돌았다"(사살 위험의 기록)와 "아예 안 돌았다"(사살이 원천적으로
    불가능한 기록)를 구분하지 못한다.

    ★ `--reason` 은 자유문장이 아니라 slug 다. ① `docs/logs` 는 기계판독 평면이고(docs.md),
      ② 이 명령은 ssh 를 건너 서브에서도 실행되므로 공백/따옴표가 섞이면 인용 지옥이 된다.
      사람이 읽을 서사는 stdout 과 testlog 가 소유한다.
    """
    now = _parse_now(args.now)
    if args.stage not in BLOCK_STAGES:
        raise SystemExit("--stage 는 %s 중 하나여야 한다: %r"
                         % ("|".join(BLOCK_STAGES), args.stage))
    if not SAFE_ID_RE.match(args.reason or ""):
        raise SystemExit("--reason 은 [A-Za-z0-9._-]+ slug 여야 한다: %r" % (args.reason,))
    rec = {
        "kind": "budget_blocked", "ts": now, "source": "blackbox_session",
        "stage": args.stage, "reason": args.reason,
        "label": _safe_label_for_event(args.label),
    }
    for item in args.detail or []:
        k, sep, v = item.partition("=")
        if not sep or not SAFE_ID_RE.match(k) or k in rec:
            raise SystemExit(
                "--detail 은 예약키가 아닌 key=value 여야 한다(key 문자셋 [A-Za-z0-9._-]): %r"
                % (item,))
        rec[k] = int(v) if re.match(r"^-?\d+$", v) else v[:200]
    ev = _append_event(args.node_dir, rec)
    print("진입 차단 기록: stage=%s reason=%s → %s" % (args.stage, args.reason, ev))
    return 0


def cmd_budget_defaults(args):
    """예산 기본값을 JSON(또는 한 필드)으로 낸다. 소비자는 이것을 읽고 자기 자리에 적지 않는다."""
    doc = {"ttl_s": DEFAULT_TTL_S, "max_ttl_s": MAX_TTL_S, "ttl_safety_mult": TTL_SAFETY_MULT,
           "source": "single-owner:blackbox_session.py"}
    if getattr(args, "field", None):
        print(doc[args.field])
    else:
        json.dump(doc, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
    return 0


def cmd_clear_budget(args):
    now = _parse_now(args.now)
    path = _budget_path(args.node_dir)
    existed = os.path.isfile(path)
    if existed:
        os.remove(path)
    _append_event(args.node_dir, {
        "kind": "budget_clear", "ts": now, "source": "blackbox_session", "existed": existed,
    })
    print("선언 해제: %s (%s)" % (path, "제거함" if existed else "원래 없음"))
    return 0


def cmd_list(args):
    d = os.path.join(args.node_dir, "sessions")
    if not os.path.isdir(d):
        print("(세션 없음)")
        return 0
    rows = []
    for fn in sorted(os.listdir(d)):
        if not fn.endswith(".json"):
            continue
        with open(os.path.join(d, fn), encoding="utf-8") as f:
            doc = json.load(f)
        rows.append(doc)
    if not rows:
        print("(세션 없음)")
        return 0
    print("%-34s %-22s %-7s %-3s %-20s %-9s %s" %
          ("session_id", "model", "topo", "tp", "started_utc", "dur_s", "verdict"))
    for d_ in rows:
        print("%-34s %-22s %-7s %-3s %-20s %-9s %s" % (
            d_.get("session_id", "?"), (d_.get("model") or "?")[:22],
            d_.get("topology") or "?", d_.get("tensor_parallel_size") or "?",
            d_.get("started_utc") or "?",
            d_.get("duration_s") if d_.get("duration_s") is not None else "(진행중)",
            d_.get("verdict") or "-"))
    return 0


def cmd_slice(args):
    """세션 구간으로 samples 를 자른다 — 조건별 비교의 실제 소비 경로."""
    path = _session_path(args.node_dir, args.session_id)
    if not os.path.isfile(path):
        raise SystemExit("세션 파일이 없다: %s" % path)
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    lo = doc.get("started_epoch")
    hi = doc.get("stopped_epoch")
    if lo is None:
        raise SystemExit("started_epoch 이 없다(손상된 세션): %s" % path)
    sdir = os.path.join(args.node_dir, "samples")
    if not os.path.isdir(sdir):
        raise SystemExit("samples 디렉터리가 없다: %s" % sdir)
    header, out = None, []
    for fn in sorted(os.listdir(sdir)):
        if not fn.endswith(".csv"):
            continue
        with open(os.path.join(sdir, fn), encoding="utf-8") as f:
            for i, line in enumerate(f):
                line = line.rstrip("\n")
                if i == 0:
                    if header is None:
                        header = line
                    continue
                ts = line.split(",", 1)[0]
                if not ts.isdigit():
                    continue
                t = int(ts)
                if t >= lo and (hi is None or t <= hi):
                    out.append(line)
    if header:
        print(header)
    for line in out:
        print(line)
    print("# %d 행 · 구간 [%s, %s]" % (len(out), doc.get("started_utc"),
                                       doc.get("stopped_utc") or "진행중"), file=sys.stderr)
    return 0


def self_test():
    import tempfile
    ok = []
    # ★ 폴백 tripwire (2026-09-01 · audit_26090109 ①) — 폴백 리터럴이 정본과 갈리면
    #   빨간불. agent_guard 에 둔 것과 **같은 술어**다: 한 파일만 고치면 결함 계열이
    #   닫히지 않는다는 것이 ① 이 가르친 전부다.
    from blackbox_eta import DEFAULTS as _CANON_S
    ok.append(("선언 상수를 정본에서 파생한다(사본 ✗ · G-B3)",
               _WD_CONST_SOURCE == "derived:blackbox_eta.DEFAULTS"
               and _WD_MARGIN_MIB == int(_CANON_S["decl_margin_mib"])
               and _WD_MIN_CEILING_MIB == int(_CANON_S["decl_min_ceiling_mib"])))
    ok.append(("리터럴 사본이 되살아나지 않았다(tripwire)",
               "_WD_FALLBACK" not in globals()))
    with tempfile.TemporaryDirectory() as td:
        node = os.path.join(td, "node-x")
        os.makedirs(os.path.join(node, "samples"))
        # 세션 구간 안 2행 · 밖 2행
        with open(os.path.join(node, "samples", "2026-07-31.csv"), "w") as f:
            f.write("ts,mem_avail\n")
            for t in (100, 150, 200, 250, 300):
                f.write("%d,%d\n" % (t, t))
        A = argparse.Namespace(
            node_dir=node, session_id="s1", model="m", model_path="/p", recipe="r",
            topology="single", tp=1, config_file="c", image="img",
            now="2026-07-31T00:02:30Z", note=None, force=False)
        # started_epoch 을 150 으로 맞추기 위해 직접 계산 대신 실제 동작만 검증
        cmd_start(A)
        p = _session_path(node, "s1")
        doc = json.load(open(p))
        ok.append(("start 사이드카 생성", os.path.isfile(p) and doc["stopped_utc"] is None))
        ok.append(("start 이벤트 append",
                   os.path.isfile(os.path.join(node, "events", "2026-07.jsonl"))))
        B = argparse.Namespace(node_dir=node, session_id="s1",
                               now="2026-07-31T00:12:30Z", verdict="PASS", note=["끝"])
        cmd_stop(B)
        doc = json.load(open(p))
        ok.append(("stop duration 600s", doc["duration_s"] == 600))
        ok.append(("stop verdict 기록", doc["verdict"] == "PASS"))
        # ── 예산 선언 (testlog_26073123 · 워치독 arm 상한) ─────────────────
        def _bud(**kw):
            base = dict(node_dir=node, mem_total_mib=124610, weights_mib=59556,
                        kv_mib=16384, overhead_mib=12288, ttl_s=7200, expected_load_s=0,  # antipattern-ok: G-B1-overhead-default — 자체검사 픽스처(선언된 값이지 기본값이 아니다)
                        label="glm-47-flash", now="2026-07-31T00:00:00Z")
            base.update(kw)
            return argparse.Namespace(**base)

        cmd_declare_budget(_bud())
        bp = _budget_path(node)
        txt = open(bp, encoding="utf-8").read()
        # floor = 124610 - (59556+16384+12288) = 36382
        ok.append(("선언 floor 산출", "floor_mib=36382" in txt))
        ok.append(("선언 만료 = now+ttl", "expires_epoch=%d\n" % (_epoch("2026-07-31T00:00:00Z") + 7200) in txt))
        ok.append(("선언 라벨 기록", "label=glm-47-flash" in txt))
        # 워치독 파서 문자셋과의 계약: 값 줄이 [A-Za-z0-9._-] 만 쓰는가
        vals = [ln.split("=", 1)[1] for ln in txt.splitlines()
                if ln and not ln.startswith("#") and "=" in ln]
        ok.append(("선언 값이 워치독 문자셋 준수",
                   all(re.match(r"^[A-Za-z0-9._-]{1,64}$", v) for v in vals)))
        # 무기한 선언 금지
        for bad_ttl in (0, -1, MAX_TTL_S + 1):
            try:
                cmd_declare_budget(_bud(ttl_s=bad_ttl))
                ok.append(("ttl %r 거부" % bad_ttl, False))
            except SystemExit:
                ok.append(("ttl %r 거부" % bad_ttl, True))
        # 상주가 총량 이상이면 거부(선언으로 불가능을 가릴 수 없다)
        try:
            cmd_declare_budget(_bud(weights_mib=124610))
            ok.append(("상주 > 총량 거부", False))
        except SystemExit:
            ok.append(("상주 > 총량 거부", True))
        # 라벨 문자셋 강제(워치독 파서가 못 읽는 값을 쓰지 않는다)
        try:
            cmd_declare_budget(_bud(label="bad label$(id)"))
            ok.append(("불량 라벨 거부", False))
        except SystemExit:
            ok.append(("불량 라벨 거부", True))
        # ── TTL 파생 검증 (C4-3) ─────────────────────────────────────────
        # 예상 로드 665s(R0 실측) → 하한 1995s. 그보다 짧은 TTL 은 "로드 도중 만료" 예약이다.
        ok.append(("TTL 하한이 예상 로드에서 파생(665s → 1995s)", ttl_floor_s(665) == 1995))
        try:
            cmd_declare_budget(_bud(ttl_s=1800, expected_load_s=665))
            ok.append(("★ 예상 로드보다 짧은 TTL 거부", False))
        except SystemExit:
            ok.append(("★ 예상 로드보다 짧은 TTL 거부", True))
        cmd_declare_budget(_bud(ttl_s=1995, expected_load_s=665))
        ok.append(("경계값 TTL(=3배)은 수락",
                   "expires_epoch=%d\n" % (_epoch("2026-07-31T00:00:00Z") + 1995)
                   in open(bp, encoding="utf-8").read()))
        # ── 갱신 (C4-3) — 원 산출을 파괴하지 않고 시계만 민다 ────────────
        cmd_declare_budget(_bud())
        pre = open(bp, encoding="utf-8").read()
        cmd_renew_budget(argparse.Namespace(node_dir=node, ttl_s=3600, expected_load_s=0,
                                            now="2026-07-31T00:10:00Z"))
        post = open(bp, encoding="utf-8").read()
        ok.append(("갱신이 만료를 새 TTL 로 민다",
                   "expires_epoch=%d\n" % (_epoch("2026-07-31T00:10:00Z") + 3600) in post))
        ok.append(("갱신이 바닥을 보존", "floor_mib=36382" in post))
        ok.append(("갱신이 원 산출 provenance 헤더를 보존",
                   all(ln in post for ln in pre.splitlines() if ln.startswith("# 산출"))))
        ok.append(("갱신 값도 워치독 문자셋 준수",
                   all(re.match(r"^[A-Za-z0-9._-]{1,64}$", ln.split("=", 1)[1])
                       for ln in post.splitlines()
                       if ln and not ln.startswith("#") and "=" in ln)))
        try:  # 갱신도 declare 와 **같은** TTL 규칙을 쓴다(느슨한 뒷문 금지)
            cmd_renew_budget(argparse.Namespace(node_dir=node, ttl_s=100,
                                                expected_load_s=665,
                                                now="2026-07-31T00:11:00Z"))
            ok.append(("갱신도 TTL 파생 검증을 받는다", False))
        except SystemExit:
            ok.append(("갱신도 TTL 파생 검증을 받는다", True))
        try:  # 만료분 갱신은 fail-closed (되살리려면 재산출)
            cmd_renew_budget(argparse.Namespace(node_dir=node, ttl_s=3600, expected_load_s=0,
                                                now="2026-07-31T09:00:00Z"))
            ok.append(("★ 만료된 선언 갱신 거부", False))
        except SystemExit:
            ok.append(("★ 만료된 선언 갱신 거부", True))
        # 해제
        cmd_clear_budget(argparse.Namespace(node_dir=node, now="2026-07-31T00:30:00Z"))
        ok.append(("선언 해제", not os.path.isfile(bp)))
        cmd_clear_budget(argparse.Namespace(node_dir=node, now="2026-07-31T00:31:00Z"))
        ok.append(("없는 선언 해제도 안전", not os.path.isfile(bp)))

        # ── 침묵 금지: 거부·차단이 events 에 남는가 (plan_26081415 C3 기준3) ──────
        #   이 자체시험이 기준3 의 재현 가능한 증거다 — 실서빙 없이 "인위 주입 → 이벤트 잔존"을
        #   전부 검사한다. 위 거부 6건(ttl 0/-1/초과 · 상주>총량 · 불량 라벨 · TTL 하한 미달)이
        #   입력이고, 아래가 판정이다.
        def _events():
            p = os.path.join(node, "events", "2026-07.jsonl")
            if not os.path.isfile(p):
                return []
            return [json.loads(ln) for ln in open(p, encoding="utf-8") if ln.strip()]

        rej = [e for e in _events() if e["kind"] == "budget_declare_rejected"]
        ok.append(("★ declare 거부가 events 에 남는다(6건 전부)", len(rej) == 6))
        ok.append(("거부 이벤트가 사유를 담는다",
                   all(e.get("reason") for e in rej)))
        ok.append(("거부 이벤트가 입력값을 담는다(재현 가능)",
                   all(e.get("mem_total_mib") == 124610 for e in rej)))
        ok.append(("TTL 하한 미달 거부가 식별 가능",
                   any("로드 도중 만료" in e["reason"] for e in rej)))
        ok.append(("불량 라벨 거부가 그 라벨 원문을 남긴다",
                   any(e.get("label") == "bad label$(id)" for e in rej)))
        ok.append(("수락된 선언은 거부로 세지 않는다",
                   len([e for e in _events() if e["kind"] == "budget_declare"]) >= 1))

        def _blk(**kw):
            base = dict(node_dir=node, stage="derive", reason="budget_params_missing",
                        label="smoke-x", detail=[], now="2026-07-31T00:40:00Z")
            base.update(kw)
            return argparse.Namespace(**base)

        for st in BLOCK_STAGES:
            cmd_budget_block(_blk(stage=st, detail=["kv_mib=16384"]))
        blk = [e for e in _events() if e["kind"] == "budget_blocked"]
        ok.append(("★ 진입 차단이 events 에 남는다(3단계)",
                   sorted(e["stage"] for e in blk) == sorted(BLOCK_STAGES)))
        ok.append(("차단 detail 이 정수로 기록", all(e.get("kv_mib") == 16384 for e in blk)))
        ok.append(("차단은 skipped 와 다른 kind",
                   all(e["kind"] != "budget_skipped" for e in blk)))
        for bad, why in ((dict(stage="whatever"), "미등록 stage"),
                         (dict(reason="공백 있는 사유"), "비-slug reason"),
                         (dict(detail=["kind=x"]), "예약키 detail"),
                         (dict(detail=["novalue"]), "형식 위반 detail")):
            try:
                cmd_budget_block(_blk(**bad))
                ok.append(("차단 기록 %s 거부" % why, False))
            except SystemExit:
                ok.append(("차단 기록 %s 거부" % why, True))

        # 미종료 구분: stopped_utc=null 이 "안 끝남", 파일 부재가 "기록 없음"
        A2 = argparse.Namespace(**{**vars(A), "session_id": "s2"})
        cmd_start(A2)
        doc2 = json.load(open(_session_path(node, "s2")))
        ok.append(("미종료 세션은 stopped_utc=None", doc2["stopped_utc"] is None))
        # 중복 start 는 거부
        try:
            cmd_start(A2)
            ok.append(("중복 start 거부", False))
        except SystemExit:
            ok.append(("중복 start 거부", True))
        # 잘못된 --now 는 거부
        try:
            _parse_now("2026-07-31 00:00:00")
            ok.append(("비ISO --now 거부", False))
        except SystemExit:
            ok.append(("비ISO --now 거부", True))
        # 경로 이스케이프 거부
        try:
            _session_path(node, "../escape")
            ok.append(("session-id 이스케이프 거부", False))
        except SystemExit:
            ok.append(("session-id 이스케이프 거부", True))
    for name, good in ok:
        print("%s %s" % ("PASS" if good else "FAIL", name))
    n = sum(1 for _, g in ok if g)
    print("\nself-test: %d/%d PASS" % (n, len(ok)))
    return 0 if n == len(ok) else 1


def main():
    ap = argparse.ArgumentParser(description="노드블랙박스 서빙 세션 사이드카")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    sub = ap.add_subparsers(dest="cmd")

    s = sub.add_parser("start", help="세션 시작 + serve_start 이벤트")
    s.add_argument("--session-id", required=True)
    s.add_argument("--model", required=True)
    s.add_argument("--model-path")
    s.add_argument("--recipe")
    s.add_argument("--topology", choices=["single", "multi"], required=True)
    s.add_argument("--tp", type=int)
    s.add_argument("--config-file")
    s.add_argument("--image")
    s.add_argument("--now", required=True, help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    s.add_argument("--note", action="append")
    s.add_argument("--force", action="store_true")
    s.set_defaults(func=cmd_start)

    t = sub.add_parser("stop", help="세션 종료 + serve_stop 이벤트")
    t.add_argument("--session-id", required=True)
    t.add_argument("--now", required=True)
    t.add_argument("--verdict", choices=["PASS", "FAIL", "REFUTE", "ABORT"])
    t.add_argument("--note", action="append")
    t.set_defaults(func=cmd_stop)

    b = sub.add_parser("declare-budget",
                       help="서빙 예산 선언 → 워치독 ETA 규칙의 arm 상한 (testlog_26073123)")
    b.add_argument("--mem-total-mib", type=int, required=True, help="/proc/meminfo MemTotal")
    b.add_argument("--weights-mib", type=int, required=True, help="가중치 실측(체크포인트 크기)")
    b.add_argument("--kv-mib", type=int, required=True,
                   help="KV 절대클램프. **미선언이면 예산 선언 자체가 불가**하다(설계 의도)")
    # 2026-09-05(G-B1): 기본값 12288 삭제 → **필수 선언**. 그 값의 주석은 "안전측" 이라 했지만
    #   방향이 반대였다 — overhead 를 낮게 잡으면 선언 바닥이 높게 나와 워치독 arm 상한이 정상
    #   서빙 위로 올라간다. gpt-oss-120b/GB10 실측 overhead 는 17,971 MiB 로 기본값보다 5,683
    #   MiB 컸고, 그 차이가 정상 로드를 사살한 실적이 있다. 노브만 추가한 2026-09-04 처방은
    #   반쪽이었다 — 기본값이 남아 있는 한 안 넘기면 틀린 값이 **조용히** 쓰인다.
    b.add_argument("--overhead-mib", type=int, required=True,
                   help="cudagraph·활성화·런타임 여유(MiB). **선언 필수** — 기본값 없음. "
                        "모르면 재보라: 로드 완료 후 MemTotal − MemAvailable − weights − kv")
    b.add_argument("--ttl-s", type=int, default=DEFAULT_TTL_S,
                   help="만료까지 초(기본 %d · 상한 %d)" % (DEFAULT_TTL_S, MAX_TTL_S))
    b.add_argument("--expected-load-s", type=int, default=0,
                   help="예상 READY 소요(초). 주면 TTL 이 그 %d배 미만일 때 **거부**한다"
                        "(로드 도중 만료 예약 방지 · plan_26081415 C4-3)" % TTL_SAFETY_MULT)
    b.add_argument("--label")
    b.add_argument("--now", required=True, help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    b.set_defaults(func=cmd_declare_budget)

    br = sub.add_parser("renew-budget",
                        help="현행 선언의 만료만 연장(상주 서빙 · plan_26081415 C4-3)")
    br.add_argument("--ttl-s", type=int, default=DEFAULT_TTL_S,
                    help="지금부터 다시 셀 초(기본 %d · 상한 %d)" % (DEFAULT_TTL_S, MAX_TTL_S))
    br.add_argument("--expected-load-s", type=int, default=0)
    br.add_argument("--now", required=True)
    br.set_defaults(func=cmd_renew_budget)

    bd = sub.add_parser("budget-defaults",
                        help="예산 선언 기본값의 **단일 소유자**가 그 값을 알려준다"
                             "(다른 스크립트가 7200 을 손으로 적지 않게)")
    bd.add_argument("--field", choices=["ttl_s", "max_ttl_s", "ttl_safety_mult"], default=None)
    bd.set_defaults(func=cmd_budget_defaults)

    bc = sub.add_parser("clear-budget", help="예산 선언 해제 → 현행 ETA 규칙 복귀")
    bc.add_argument("--now", required=True)
    bc.set_defaults(func=cmd_clear_budget)

    bs = sub.add_parser("budget-skip", help="무보호 진입 기록(--no-budget 탈출구 · 침묵 금지)")
    bs.add_argument("--reason", required=True)
    bs.add_argument("--label")
    bs.add_argument("--now", required=True)
    bs.set_defaults(func=cmd_budget_skip)

    bk = sub.add_parser("budget-block",
                        help="예산 선언 미성립으로 **진입을 차단**했음을 기록(침묵 금지 · C3 기준3)")
    bk.add_argument("--stage", required=True, choices=BLOCK_STAGES,
                    help="derive=입력 파생 실패 · preflight_ceiling=arm 상한 선판정 · declare=선언/수락 실패")
    bk.add_argument("--reason", required=True, help="slug [A-Za-z0-9._-]+ (기계판독 평면)")
    bk.add_argument("--label")
    bk.add_argument("--detail", action="append", default=[], metavar="KEY=VALUE",
                    help="부가 수치(반복 가능). 숫자면 정수로 기록한다")
    bk.add_argument("--now", required=True)
    bk.set_defaults(func=cmd_budget_block)

    l = sub.add_parser("list", help="세션 목록")
    l.set_defaults(func=cmd_list)

    c = sub.add_parser("slice", help="세션 구간의 samples 만 출력")
    c.add_argument("--session-id", required=True)
    c.set_defaults(func=cmd_slice)

    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not getattr(args, "func", None):
        ap.print_help()
        return 2
    if not args.node_dir:
        raise SystemExit("--node-dir 이 필요하다(docs/logs/<node_id>)")
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
