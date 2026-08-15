#!/usr/bin/env python3
"""regen_envelope.py — rollup+1Hz samples 로 `envelope.json` 을 재생성하고 **키 정합**을 강제한다.

근거: plan_26081415 §3(C2). 진단 §1.2 가 특정한 결함은 두 겹이다.

  ① **동결** — envelope 쓰기 지점이 `seed_from_journal.py` 단 하나뿐이라 2026-07-31 이후 갱신이 없다.
     rollup 은 매일 생기는데 envelope 로 흘러가는 경로가 **없다**.
  ② **키 불일치** — 그 envelope 의 `eta_params` 가 담은 5키 중 트립 판정에 쓰이는 키는 **0개**다.
     `daemon_kill_s`·`eta_floor_s` 는 `blackbox_eta.DEFAULTS` 에 없어 `main()` 의 `if k in DEFAULTS`
     필터가 **조용히 버린다**. 즉 ①만 고치면(재생성만 하면) 위양성은 100% 재발한다.

  ⇒ 이 스크립트는 ①을 고치고, `check-keys` 가 ②를 **fail-loud** 로 바꾼다. 둘 다 있어야 의미가 있다.

설계 규율(헌법 §결정론 규율 — 출처 표시):
  - 값 옆에 출처를 둔다. `state`·`provenance.source`·`eta_params_source` 로 **측정분과 기본값분을
    데이터에서 가른다.** 구분이 없으면 하류가 무엇을 근거로 삼았는지 알 수 없다.
  - `max` 는 일별 최대의 최대라 **정확**하지만, 백분위수는 일별 통계에서 정확히 합쳐지지 않는다.
    그래서 원시 samples 가 남아 있는 날은 원시를 파싱해 **정확히** 계산하고, 원시가 수명으로 사라진
    날만 rollup 으로 대체하며 그 사실을 `provenance.percentiles_exact` 로 밝힌다.
  - 시각은 `--now` 주입만(벽시계 금지 — docs.md §기계판독 데이터 평면).
  - **원시는 편집하지 않는다**(plan §3 성공기준 5). 읽기 전용이다.

종료코드: 0=성공 · 1=인자/입력 오류 · 2=자체시험 실패 · 3=키 정합 위반(check-keys).
"""
from __future__ import annotations

import argparse
import glob
import json
import os
import re
import sys
from datetime import datetime, timedelta, timezone

SCHEMA_VERSION = 2
ISO_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
# node_id 스킴 R (plan_26081514 §3.1 · terraforming_node SKILL.md §2.7.6).
# 셸측 정본은 node_blackbox/node_identity.sh 의 NI_SLUG_RE — **같은 정규식**이며, 갈라지면
# 한쪽만 통과하는 값이 생긴다. 둘 다 고칠 때만 어휘가 바뀐다(정적 파일 간 교차검증 규율).
NODE_ID_RE = re.compile(r"^[a-z][a-z0-9-]{0,31}$")

# blackbox_eta.DEFAULTS 를 정본으로 읽는다. **여기에 키 목록을 복사하지 않는다** —
# 복사하면 4종 안티패턴의 `하드코딩·결함`(파생 가능한데 손으로 적은 것)이 되고,
# 정본이 바뀔 때 이 파일만 옛 목록으로 남아 검증기가 거짓 판정을 낸다.
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
try:
    from blackbox_eta import (DEFAULTS as ETA_DEFAULTS,  # noqa: E402
                              ENVELOPE_META_KEYS, max_rate_mib_s, read_csv_lines,
                              read_mem_total_mib, runway_s, unknown_envelope_keys)
except ImportError as e:  # pragma: no cover - 배치 오류는 조용히 넘기지 않는다
    raise SystemExit("blackbox_eta.py 를 같은 디렉터리에서 찾지 못했다: %r" % (e,))

# 기본값 그대로인 키도 envelope 에 실어 **하류가 무엇을 봤는지** 남긴다(출처는 default 로 표기).
_PASSTHROUGH_DEFAULT_KEYS = ("agent_act_s", "agent_notify_s")


# ──────────────────────────────────────────────────────────────────────────
# 입력 읽기
# ──────────────────────────────────────────────────────────────────────────
def _parse_now(value):
    if not isinstance(value, str) or not ISO_RE.match(value):
        raise SystemExit("--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: %r" % (value,))
    return value


def _epoch(iso):
    return int(datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ")
               .replace(tzinfo=timezone.utc).timestamp())


def _num(s):
    try:
        return float(s)
    except (TypeError, ValueError):
        return None


def _pct(sorted_vals, q):
    if not sorted_vals:
        return None
    i = min(len(sorted_vals) - 1, max(0, int(round(q * (len(sorted_vals) - 1)))))
    return round(sorted_vals[i], 3)


def _sample_path(node_dir, day):
    for ext in (".csv", ".csv.zst", ".csv.gz"):
        p = os.path.join(node_dir, "samples", day + ext)
        if os.path.isfile(p):
            return p
    return None


def scan_samples_day(path):
    """하루치 원시 → 값 리스트. 반환 dict(mem=[], falling=[], n=int, absent=set)."""
    lines = read_csv_lines(path)
    if not lines:
        return None
    header = lines[0].split(",")
    idx = {k: i for i, k in enumerate(header)}
    if "mem_avail" not in idx or "mem_rate" not in idx:
        return None
    mem, falling, n = [], [], 0
    present = set()
    for line in lines[1:]:
        parts = line.split(",")
        if len(parts) < len(header):
            continue
        n += 1
        v = _num(parts[idx["mem_avail"]])
        if v is not None:
            mem.append(v)
        r = _num(parts[idx["mem_rate"]])
        if r is not None and r > 0:      # 양수 = 하강 (blackbox_collect 규약)
            falling.append(r)
        for k, i in idx.items():
            if i < len(parts) and parts[i] != "":
                present.add(k)
    absent = sorted(set(idx) - present)
    return {"mem": mem, "falling": falling, "n": n, "absent": absent}


def load_events(node_dir):
    """events 평면 전체를 중복 제거해 읽는다.

    같은 사건이 두 파일에 있다 — 워치독이 직접 쓰는 `watchdog.jsonl`(BB_EVENTS)과
    `blackbox_events.py` 가 저널에서 통합한 `<YYYY-MM>.jsonl`. 중복을 지우지 않으면
    kill 지연 표본이 부풀어 '측정'이 아니라 '중복 계수'가 된다.
    """
    seen, rows = set(), []
    paths = sorted(glob.glob(os.path.join(node_dir, "events", "*.jsonl")))
    for p in paths:
        try:
            fh = open(p, encoding="utf-8")
        except OSError:
            continue
        with fh:
            for ln in fh:
                ln = ln.strip()
                if not ln:
                    continue
                try:
                    r = json.loads(ln)
                except ValueError:
                    continue
                key = (r.get("ts"), r.get("kind"),
                       str(r.get("targets") or r.get("target") or ""))
                if key in seen:
                    continue
                seen.add(key)
                rows.append(r)
    rows.sort(key=lambda r: r.get("ts") or "")
    return rows


def measure_kill_latency(events, since_utc=None):
    """`watchdog_trip` → 다음 `watchdog_kill_ack` 간격(초)을 실측한다.

    ★ seed 값(max 14 s)은 **15초 해상도 저널 재구성**이라 구조적으로 과대평가다(seed envelope 의
      caveat 가 그렇게 적고 있다). events 평면은 1초 해상도이므로 여기서 나온 값이 정본이다.
      과대평가된 kill 지연은 활주로를 늘려 **위양성을 더 키운다** — 그래서 이 측정이 필요하다.
    """
    obs = []
    pending = None
    for r in events:
        ts = r.get("ts")
        if not ts or not ISO_RE.match(ts):
            continue
        if since_utc and ts < since_utc:
            continue
        kind = r.get("kind")
        if kind == "watchdog_trip":
            pending = ts
        elif kind == "watchdog_kill_ack" and pending:
            d = _epoch(ts) - _epoch(pending)
            if 0 <= d <= 120:            # 120s 초과는 짝짓기 실패로 본다(조용히 섞지 않는다)
                obs.append(float(d))
            pending = None
    return obs


# ──────────────────────────────────────────────────────────────────────────
# 포락선 조립
# ──────────────────────────────────────────────────────────────────────────
def build_envelope(node_id, node_dir, now_utc, window_days, mem_total_mib,
                   mem_total_source):
    day0 = datetime.strptime(now_utc, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    days = [(day0 - timedelta(days=i)).strftime("%Y-%m-%d") for i in range(window_days)]
    days.reverse()

    mem_all, falling_all = [], []
    from_samples, from_rollup, missing = [], [], []
    rollup_only_stats = []
    absent_union = set()
    n_samples = 0
    first_day = last_day = None

    for day in days:
        sp = _sample_path(node_dir, day)
        if sp:
            d = scan_samples_day(sp)
            if d and d["n"]:
                mem_all.extend(d["mem"])
                falling_all.extend(d["falling"])
                n_samples += d["n"]
                absent_union |= set(d["absent"])
                from_samples.append(day)
                first_day = first_day or day
                last_day = day
                continue
        rp = os.path.join(node_dir, "rollup", day + ".json")
        if os.path.isfile(rp):
            try:
                with open(rp, encoding="utf-8") as fh:
                    rollup_only_stats.append(json.load(fh))
                from_rollup.append(day)
                first_day = first_day or day
                last_day = day
                continue
            except (OSError, ValueError):
                pass
        missing.append(day)

    mem_all.sort()
    falling_all.sort()

    # rollup 대체분의 max 는 정확히 합쳐진다(최대의 최대). 백분위수는 그렇지 않다.
    r_desc_max = [s.get("descent_rate_mib_s", {}).get("max") for s in rollup_only_stats]
    r_desc_max = [x for x in r_desc_max if x is not None]
    r_mem_min = [s.get("mem_avail_mib", {}).get("min") for s in rollup_only_stats]
    r_mem_min = [x for x in r_mem_min if x is not None]
    r_mem_max = [s.get("mem_avail_mib", {}).get("max") for s in rollup_only_stats]
    r_mem_max = [x for x in r_mem_max if x is not None]
    n_samples += sum(int(s.get("samples") or 0) for s in rollup_only_stats)

    desc_max = max([falling_all[-1]] if falling_all else [], default=None)
    desc_max = max([x for x in (desc_max, max(r_desc_max, default=None)) if x is not None],
                   default=None)
    mem_min = min([x for x in ((mem_all[0] if mem_all else None),
                               min(r_mem_min, default=None)) if x is not None], default=None)
    mem_max = max([x for x in ((mem_all[-1] if mem_all else None),
                               max(r_mem_max, default=None)) if x is not None], default=None)

    percentiles_exact = not from_rollup

    events = load_events(node_dir)
    since = days[0] + "T00:00:00Z"
    lat = measure_kill_latency(events, since_utc=since)
    lat_all = measure_kill_latency(events)          # 전 기간(표본 보강용)
    lat_used = lat if lat else lat_all
    lat_window = "window" if lat else "all-time"
    trips = [r for r in events if r.get("kind") == "watchdog_trip" and r.get("ts", "") >= since]

    # ── eta_params: 측정된 것만 측정으로, 나머지는 기본값으로 **표시**한다 ──────────
    eta_params, eta_src = {}, {}
    if lat_used:
        eta_params["kill_latency_s"] = float(max(lat_used))
        eta_src["kill_latency_s"] = (
            "measured:events watchdog_trip→watchdog_kill_ack max (n=%d · 1s 해상도 · %s)"
            % (len(lat_used), lat_window))
    else:
        eta_params["kill_latency_s"] = float(ETA_DEFAULTS["kill_latency_s"])
        eta_src["kill_latency_s"] = "default:blackbox_eta.DEFAULTS (실측 표본 0)"
    for k in _PASSTHROUGH_DEFAULT_KEYS:
        eta_params[k] = float(ETA_DEFAULTS[k])
        eta_src[k] = "default:blackbox_eta.DEFAULTS"

    # ── 파생 좌표: C1(하강률 상한)이 근거로 삼을 값. **손으로 적지 않는다** ────────
    #   무조건-트립 경계 = mem_total / runway. 이 값을 넘는 하강률에서는 ETA 부등식이
    #   잔량 축을 통째로 삼켜 항상 참이 된다(plan_26081415 §1.1).
    derived = {}
    if mem_total_mib:
        p = dict(ETA_DEFAULTS)
        p.update(eta_params)
        # 산식의 정본은 blackbox_eta 다 — 여기서 다시 적으면 두 파일이 갈린다(실제로 갈렸었다).
        runway = runway_s(p)
        derived = {
            "mem_total_mib": mem_total_mib,
            "mem_total_source": mem_total_source,
            "runway_s": round(runway, 3),
            "runway_source": "derived:blackbox_eta.runway_s(envelope.eta_params + DEFAULTS)",
            "unconditional_trip_rate_mib_s": round(mem_total_mib / runway, 1),
            "unconditional_trip_rate_source":
                "derived:mem_total/runway — 이 하강률 이상에서 ETA 는 잔량과 무관하게 항상 TRIP",
            "observed_max_rate_exceeds_unconditional":
                bool(desc_max is not None and desc_max >= mem_total_mib / runway),
            # ── 이중규칙 좌표(plan_26081415 C1-A) — 핫루프가 실제로 쓰는 **정수** 상한 ────
            #   위 `unconditional_trip_rate_mib_s` 는 설명용 실수, 아래가 판정에 쓰이는 값이다.
            #   둘을 함께 실어야 "왜 12,461 이 아니라 12,461 인가"를 나중에 재구성할 수 있다.
            "max_rate_mib_s": max_rate_mib_s(mem_total_mib, p),
            "max_rate_source": "derived:int(mem_total // runway) — 셸 정수 산술과 동일 경계",
            "abs_band_mib": ETA_DEFAULTS["abs_band_mib"],
            "abs_band_source": "default:blackbox_eta.DEFAULTS.abs_band_mib (레거시 절대임계 재사용)",
        }

    return {
        "schema_version": SCHEMA_VERSION,
        "node_id": node_id,
        "generated_utc": now_utc,
        "state": "measured" if (from_samples or from_rollup) else "empty",
        "provenance": {
            "source": "rollup+1Hz samples",
            "resolution_s": 1,
            "window_days": window_days,
            "days_from_samples": from_samples,
            "days_from_rollup": from_rollup,
            "days_missing": missing,
            "percentiles_exact": percentiles_exact,
            "absent_fields": sorted(absent_union),
            "generator": "regen_envelope.py",
            "caveat": ("백분위수는 원시 samples 가 남은 날에서만 정확하다. rollup 대체분이 있으면 "
                       "`percentiles_exact=false` 이며 max/min 만 정확하다(최대의 최대·최소의 최소). "
                       "kill 지연은 events 평면 1초 해상도 실측이라 seed 의 15초 재구성값을 대체한다."),
        },
        "samples": {
            "count": n_samples,
            "first_day": first_day,
            "last_day": last_day,
            "source": "measured:samples/*.csv[.zst|.gz] + rollup/*.json",
        },
        "descent_rate_mib_s": {
            "falling_samples": len(falling_all),
            "p50": _pct(falling_all, 0.50), "p90": _pct(falling_all, 0.90),
            "p99": _pct(falling_all, 0.99), "max": desc_max,
            "max_source": "measured:max-of-daily-max (정확)",
            "percentile_source": ("measured:1Hz 원시 전수" if percentiles_exact
                                  else "partial:원시 결손일은 백분위수 미반영"),
        },
        "mem_avail_mib": {
            "min": mem_min, "p01": _pct(mem_all, 0.01), "p50": _pct(mem_all, 0.50),
            "max": mem_max,
            "min_source": "measured:min-of-daily-min (정확)",
        },
        "kill": {
            "trips": len(trips),
            "trip_mem_mib": [r.get("mem_avail_mib") for r in trips],
            "latency_s": {
                "observed": lat_used,
                "max": max(lat_used) if lat_used else None,
                "p50": _pct(sorted(lat_used), 0.50) if lat_used else None,
                "n": len(lat_used),
                "scope": lat_window,
                "source": "measured:events watchdog_trip→watchdog_kill_ack (1s 해상도)",
            },
        },
        "eta_params": eta_params,
        "eta_params_source": eta_src,
        "derived": derived,
    }


# ──────────────────────────────────────────────────────────────────────────
# 키 정합 검증기 (C2-3) — 현행의 **조용한 버림**을 fail-loud 로 바꾼다
# ──────────────────────────────────────────────────────────────────────────
def check_keys(envelope):
    """(ok, errors, unknown_keys). blackbox_eta.DEFAULTS 가 유일한 정본 키 목록이다."""
    errs = []
    params = envelope.get("eta_params")
    if params is None:
        return False, ["envelope 에 eta_params 가 없다"], []
    if not isinstance(params, dict):
        return False, ["eta_params 가 dict 가 아니다: %r" % type(params).__name__], []
    # 미지 키 판정의 정본은 blackbox_eta 다(DEFAULTS 소유자). 여기서 다시 세지 않는다.
    unknown = unknown_envelope_keys(envelope)
    for k in unknown:
        errs.append("eta_params.%s 가 blackbox_eta.DEFAULTS 에 없다 — "
                    "현행 `if k in DEFAULTS` 필터가 **조용히 버린다**(가능: %s)"
                    % (k, ", ".join(sorted(ETA_DEFAULTS))))
    src = envelope.get("eta_params_source") or {}
    for k in sorted(params):
        if k in ENVELOPE_META_KEYS:
            # seed 시절 관습: eta_params 안에 산문 provenance 를 넣었다. 값이 아니라 메타이므로
            # 미지 키로 세지는 않지만, **정본 위치는 eta_params_source** 이므로 이관을 요구한다.
            errs.append("eta_params.%s 는 값이 아니라 메타다 — eta_params_source 로 이관하라" % k)
            continue
        if k in unknown:
            continue
        if not isinstance(params[k], (int, float)) or isinstance(params[k], bool):
            errs.append("eta_params.%s 는 수치여야 한다: %r" % (k, params[k]))
        if k not in src:
            errs.append("eta_params.%s 에 출처(eta_params_source)가 없다 — "
                        "헌법 §결정론 규율: 값 옆에 출처를 둔다" % k)
    return (not errs), errs, unknown


def cmd_check_keys(args):
    path = args.envelope or os.path.join(args.node_dir or "", "envelope.json")
    if not os.path.isfile(path):
        print("[regen] FAIL: envelope 이 없다: %s" % path, file=sys.stderr)
        return 1
    with open(path, encoding="utf-8") as fh:
        env = json.load(fh)
    ok, errs, unknown = check_keys(env)
    print("[regen] 키 정합 검증 → %s" % path)
    print("        정본 키(blackbox_eta.DEFAULTS) %d 종" % len(ETA_DEFAULTS))
    print("        envelope.eta_params 키: %s"
          % ", ".join(sorted((env.get("eta_params") or {}))))
    if ok:
        print("[regen] PASS — 불일치 0")
        return 0
    for e in errs:
        print("  ✗ " + e, file=sys.stderr)
    print("[regen] FAIL — 불일치 %d 건(그중 미지 키 %d 건)" % (len(errs), len(unknown)),
          file=sys.stderr)
    return 3


# ──────────────────────────────────────────────────────────────────────────
# staleness 게이트 (C2-4) — **차단이 아니라 경고**. 시각은 --now 주입만.
# ──────────────────────────────────────────────────────────────────────────
def envelope_age_days(envelope, now_utc):
    g = envelope.get("generated_utc")
    if not g or not ISO_RE.match(g):
        return None
    return (_epoch(now_utc) - _epoch(g)) / 86400.0


def cmd_staleness(args):
    path = args.envelope or os.path.join(args.node_dir or "", "envelope.json")
    now = _parse_now(args.now)
    if not os.path.isfile(path):
        print("[regen] ⚠ envelope 부재 — stale 로 간주: %s" % path)
        _emit_event(args.node_dir, now, "envelope_stale",
                    {"reason": "absent", "path": path})
        return 0
    with open(path, encoding="utf-8") as fh:
        env = json.load(fh)
    age = envelope_age_days(env, now)
    state = env.get("state")
    if age is None:
        print("[regen] ⚠ generated_utc 파싱 불가 — stale 로 간주")
        _emit_event(args.node_dir, now, "envelope_stale", {"reason": "unparsable_generated_utc"})
        return 0
    stale = age > args.max_age_days or state != "measured"
    line = ("[regen] envelope age=%.2f일 state=%s (임계 %g일)"
            % (age, state, args.max_age_days))
    if stale:
        print(line + " → ⚠ STALE (경고 · 차단 아님)")
        _emit_event(args.node_dir, now, "envelope_stale",
                    {"age_days": round(age, 3), "state": state,
                     "max_age_days": args.max_age_days})
    else:
        print(line + " → OK")
    return 0


def _emit_event(node_dir, now_utc, kind, extra):
    if not node_dir:
        return
    d = os.path.join(node_dir, "events")
    try:
        os.makedirs(d, exist_ok=True)
        rec = {"ts": now_utc, "kind": kind, "source": "regen_envelope"}
        rec.update(extra)
        with open(os.path.join(d, now_utc[:7] + ".jsonl"), "a", encoding="utf-8") as fh:
            fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    except OSError as e:
        # fail-loud 폴백: 못 남겼다는 사실을 삼키지 않는다.
        print("[regen] ⚠ 이벤트 기록 실패(%s): %r" % (kind, e), file=sys.stderr)


# ──────────────────────────────────────────────────────────────────────────
def cmd_regen(args):
    now = _parse_now(args.now)
    node_dir = args.node_dir
    if not node_dir or not os.path.isdir(node_dir):
        print("[regen] FAIL: --node-dir 이 없다: %r" % node_dir, file=sys.stderr)
        return 1
    node_id = args.node_id or os.path.basename(os.path.normpath(node_dir))
    # node_id 스킴 fail-closed (plan_26081514 §4.2 A6 · R4). basename 파생은 디렉터리명을
    # 추종하므로 스킴이 확정되면 자동 정합하지만, **검증이 없으면** 대문자·공백·hostname
    # 잔재가 그대로 envelope 에 박혀 `docs/logs/<node_id>` 규약이 조용히 깨진다.
    # 값 하나가 들어가는 문이 여기 하나뿐이므로 여기서 막는다.
    if not NODE_ID_RE.match(node_id):
        print("[regen] FAIL: node_id %r 가 스킴 위반이다 — %s" % (node_id, NODE_ID_RE.pattern),
              file=sys.stderr)
        print("[regen]       node_id ::= manifest nodes[].role 슬러그"
              "(terraforming_node SKILL.md §2.7.6). hostname 파생은 폐지됐다.", file=sys.stderr)
        print("[regen]       경로를 docs/logs/<role> 로 마이그레이션하거나 --node-id=<slug> 로 명시하라.",
              file=sys.stderr)
        return 1
    if args.mem_total_mib:
        mt, mts = args.mem_total_mib, "explicit:--mem-total-mib"
    else:
        mt, mts = read_mem_total_mib(), "measured:/proc/meminfo MemTotal"
        if mt is None:
            mts = "unknown:/proc/meminfo 판독 실패"
    env = build_envelope(node_id, node_dir, now, args.window_days, mt, mts)

    ok, errs, _ = check_keys(env)
    if not ok:
        # 생성기가 자기 산출물을 검증한다 — 검증기가 나중에 잡을 것을 지금 잡는다(fail-loud).
        print("[regen] FAIL: 생성한 envelope 이 키 정합을 위반했다(생성기 결함):", file=sys.stderr)
        for e in errs:
            print("  ✗ " + e, file=sys.stderr)
        return 3

    out = args.output or os.path.join(node_dir, "envelope.json")
    body = json.dumps(env, ensure_ascii=False, indent=2, sort_keys=True) + "\n"
    if args.dry_run:
        print(body)
        print("[regen] (dry-run) 쓰지 않았다 → %s" % out, file=sys.stderr)
        return 0
    tmp = out + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        fh.write(body)
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, out)      # 원자적 교체 — 소비자가 잘린 JSON 을 보지 않도록
    d = env["descent_rate_mib_s"]
    k = env["kill"]["latency_s"]
    print("[regen] %s → %s" % (node_id, out))
    print("        state=%s · samples=%d · 원시일=%d · rollup일=%d · 결손일=%d"
          % (env["state"], env["samples"]["count"],
             len(env["provenance"]["days_from_samples"]),
             len(env["provenance"]["days_from_rollup"]),
             len(env["provenance"]["days_missing"])))
    print("        하강률 max=%s p99=%s p50=%s (정확백분위=%s)"
          % (d["max"], d["p99"], d["p50"], env["provenance"]["percentiles_exact"]))
    print("        kill 지연 실측 max=%s p50=%s n=%d (%s)"
          % (k["max"], k["p50"], k["n"], k["scope"]))
    if env["derived"]:
        dv = env["derived"]
        print("        파생: runway=%ss · 무조건-트립 경계=%s MiB/s%s"
              % (dv["runway_s"], dv["unconditional_trip_rate_mib_s"],
                 "  ⚠ 관측 최대 하강률이 이 경계를 넘는다"
                 if dv["observed_max_rate_exceeds_unconditional"] else ""))
    _emit_event(node_dir, now, "envelope_regenerated",
                {"state": env["state"], "descent_max_mib_s": d["max"],
                 "kill_latency_max_s": k["max"], "window_days": args.window_days})
    return 0


# ──────────────────────────────────────────────────────────────────────────
def _self_test():
    import tempfile
    checks = []
    with tempfile.TemporaryDirectory() as td:
        nd = os.path.join(td, "node-x")
        os.makedirs(os.path.join(nd, "samples"))
        os.makedirs(os.path.join(nd, "events"))
        os.makedirs(os.path.join(nd, "rollup"))
        hdr = "ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n"
        # 2026-07-31: 하강률 최대 23313.5 (실측 클래스 재현) · gpu_mem 은 상시 공란(GB10 통합메모리)
        with open(os.path.join(nd, "samples", "2026-07-31.csv"), "w") as fh:
            fh.write(hdr)
            for i, (m, r) in enumerate([(100000, 0), (90000, 23313.5), (80000, 5.0),
                                        (70000, -3.0), (60000, 1.0)]):
                fh.write("%d,%d,%g,50,10,1,0,,0.5,1\n" % (1000 + i, m, r))
        # 원시가 없는 날은 rollup 으로 대체된다
        with open(os.path.join(nd, "rollup", "2026-07-30.json"), "w") as fh:
            json.dump({"day": "2026-07-30", "samples": 10,
                       "mem_avail_mib": {"min": 5000.0, "max": 111111.0},
                       "descent_rate_mib_s": {"max": 99.0}}, fh)
        # events: trip → kill_ack 3초
        with open(os.path.join(nd, "events", "2026-07.jsonl"), "w") as fh:
            for r in ({"ts": "2026-07-31T00:00:10Z", "kind": "watchdog_trip",
                       "mem_avail_mib": 31487, "targets": "abc"},
                      {"ts": "2026-07-31T00:00:13Z", "kind": "watchdog_kill_ack",
                       "targets": "abc"}):
                fh.write(json.dumps(r) + "\n")
        # 중복 파일(워치독 직기록) — 중복 계수되면 안 된다
        with open(os.path.join(nd, "events", "watchdog.jsonl"), "w") as fh:
            for r in ({"ts": "2026-07-31T00:00:10Z", "kind": "watchdog_trip",
                       "mem_avail_mib": 31487, "targets": "abc"},
                      {"ts": "2026-07-31T00:00:13Z", "kind": "watchdog_kill_ack",
                       "targets": "abc"}):
                fh.write(json.dumps(r) + "\n")

        env = build_envelope("node-x", nd, "2026-07-31T12:00:00Z", 3, 124610,
                             "explicit:self-test")
        checks.append(("state=measured", env["state"] == "measured"))
        checks.append(("실측 하강률 max 보존(23313.5)",
                       env["descent_rate_mib_s"]["max"] == 23313.5))
        checks.append(("rollup 대체분의 max 도 합쳐진다(최대의 최대)",
                       env["mem_avail_mib"]["max"] == 111111.0))
        checks.append(("rollup 대체분의 min 도 합쳐진다",
                       env["mem_avail_mib"]["min"] == 5000.0))
        checks.append(("rollup 섞이면 percentiles_exact=False",
                       env["provenance"]["percentiles_exact"] is False))
        checks.append(("음수 rate(상승)는 하강 표본에서 제외",
                       env["descent_rate_mib_s"]["falling_samples"] == 3))
        checks.append(("kill 지연 실측 3초·중복 제거로 n=1",
                       env["kill"]["latency_s"]["max"] == 3.0
                       and env["kill"]["latency_s"]["n"] == 1))
        checks.append(("kill_latency_s 출처가 measured 로 표기",
                       env["eta_params_source"]["kill_latency_s"].startswith("measured:")))
        checks.append(("기본값 통과분은 default 로 표기",
                       env["eta_params_source"]["agent_act_s"].startswith("default:")))
        checks.append(("상시 공란 컬럼이 absent_fields 로 잡힌다",
                       "gpu_mem" in env["provenance"]["absent_fields"]))
        # 파생: runway = kill 3 + detect 2 + (debounce 3-1)*1 = 7 → 124610/7 = 17801.4
        checks.append(("파생 runway 가 측정 kill 지연을 반영",
                       env["derived"]["runway_s"] == 7.0))
        checks.append(("무조건-트립 경계가 mem_total 에서 파생",
                       env["derived"]["unconditional_trip_rate_mib_s"] == 17801.4))
        checks.append(("관측 최대가 경계를 넘으면 표시",
                       env["derived"]["observed_max_rate_exceeds_unconditional"] is True))
        # mem_total 이 바뀌면 경계도 따라간다(하드코딩 아님의 증명)
        env2 = build_envelope("node-x", nd, "2026-07-31T12:00:00Z", 3, 62305,
                              "explicit:self-test")
        checks.append(("경계가 mem_total 변경을 따라간다",
                       env2["derived"]["unconditional_trip_rate_mib_s"] == 8900.7))
        # ★ C1 이 실제로 쓰는 정수 상한도 envelope 에 실린다(핫루프 값의 재구성 가능성)
        checks.append(("이중규칙 정수 상한이 envelope 에 실린다(124610/7=17801)",
                       env["derived"]["max_rate_mib_s"] == 17801))
        checks.append(("정수 상한도 mem_total 을 따라간다",
                       env2["derived"]["max_rate_mib_s"] == 8900))
        checks.append(("절대 밴드가 정본(blackbox_eta.DEFAULTS)에서 온다",
                       env["derived"]["abs_band_mib"] == ETA_DEFAULTS["abs_band_mib"]))

        # ── 키 정합 검증기 ────────────────────────────────────────────────
        ok, errs, unk = check_keys(env)
        checks.append(("생성 산출물은 키 정합 통과", ok and not errs))
        legacy = {"eta_params": {"agent_act_s": 300, "agent_notify_s": 900,
                                 "daemon_kill_s": 6, "eta_floor_s": 6.0,
                                 "provenance": "초기 임의값"},
                  "eta_params_source": {}}
        ok2, errs2, unk2 = check_keys(legacy)
        checks.append(("★ seed envelope 의 미지 키 2건 검출(daemon_kill_s·eta_floor_s)",
                       (not ok2) and sorted(unk2) == ["daemon_kill_s", "eta_floor_s"]))
        checks.append(("eta_params.provenance 는 이관 요구로 잡힌다",
                       any("이관" in e for e in errs2)))
        checks.append(("출처 누락도 잡힌다",
                       any("출처" in e for e in errs2)))
        ok3, errs3, _ = check_keys({"eta_params": {"kill_latency_s": "4.0"},
                                    "eta_params_source": {"kill_latency_s": "x"}})
        checks.append(("문자열 값 거부", (not ok3) and any("수치" in e for e in errs3)))
        ok4, _, _ = check_keys({"eta_params": {"kill_latency_s": 4.0},
                                "eta_params_source": {"kill_latency_s": "measured:x"}})
        checks.append(("정상 최소 envelope 통과", ok4))

        # ── staleness ────────────────────────────────────────────────────
        checks.append(("age 계산(0.5일)",
                       abs(envelope_age_days({"generated_utc": "2026-07-31T00:00:00Z"},
                                             "2026-07-31T12:00:00Z") - 0.5) < 1e-9))
        checks.append(("generated_utc 불량 → None",
                       envelope_age_days({"generated_utc": "bad"}, "2026-07-31T12:00:00Z")
                       is None))

        # ── 쓰기: 원자적 교체 · 원시 미편집 ──────────────────────────────
        before = sorted(os.listdir(os.path.join(nd, "samples")))
        a = argparse.Namespace(node_dir=nd, node_id=None, now="2026-07-31T12:00:00Z",
                               window_days=3, mem_total_mib=124610, output=None,
                               dry_run=False)
        rc = cmd_regen(a)
        checks.append(("regen 성공", rc == 0))
        checks.append(("envelope.json 기록됨",
                       os.path.isfile(os.path.join(nd, "envelope.json"))))
        checks.append(("원시 samples 미편집", sorted(os.listdir(os.path.join(nd, "samples")))
                       == before))
        checks.append((".tmp 잔재 없음",
                       not os.path.exists(os.path.join(nd, "envelope.json.tmp"))))
        wrote = json.load(open(os.path.join(nd, "envelope.json"), encoding="utf-8"))
        checks.append(("기록본이 키 정합 통과", check_keys(wrote)[0]))
        evs = load_events(nd)
        checks.append(("재생성 이벤트 기록",
                       any(e.get("kind") == "envelope_regenerated" for e in evs)))
        # ── node_id 스킴 fail-closed (A6 · R4) ───────────────────────────
        #   ★ 위 "regen 성공" 케이스의 node_dir 은 `node-x` 라 슬러그 규칙을 통과한다 —
        #     즉 **양성만** 덮는다. 음성이 없으면 A6 게이트를 통째로 지워도 self-test 가
        #     녹색이다. 양성·음성 쌍을 둬야 다음 사람이 정규식을 넓히거나 좁힐 때
        #     리뷰가 강제된다(claim_predicates C2 픽스처 쌍과 같은 규율).
        checks.append(("node_id 정규식 양성", bool(NODE_ID_RE.match("sub2"))))
        checks.append(("node_id 정규식 음성(hostname 잔재)",
                       not NODE_ID_RE.match("spark-A73E")))
        bad_nd = os.path.join(td, "Spark-BAD")
        os.makedirs(os.path.join(bad_nd, "samples"), exist_ok=True)
        rc_bad = cmd_regen(argparse.Namespace(
            node_dir=bad_nd, node_id=None, now="2026-07-31T12:00:00Z", window_days=3,
            mem_total_mib=124610, output=None, dry_run=False))
        checks.append(("스킴 위반 node_dir 거부(rc=1)", rc_bad == 1))
        checks.append(("거부 시 envelope 미기록",
                       not os.path.exists(os.path.join(bad_nd, "envelope.json"))))
        checks.append(("--node-id 명시로 우회 가능",
                       cmd_regen(argparse.Namespace(
                           node_dir=bad_nd, node_id="sub", now="2026-07-31T12:00:00Z",
                           window_days=3, mem_total_mib=124610, output=None,
                           dry_run=False)) == 0))
        # 비ISO now 거부
        try:
            _parse_now("2026-07-31 12:00:00")
            checks.append(("비ISO --now 거부", False))
        except SystemExit:
            checks.append(("비ISO --now 거부", True))

    ok_all = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok_all = ok_all and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok_all else "FAIL", len(checks)))
    return 0 if ok_all else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="노드블랙박스 포락선 재생성 + 키 정합 검증")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")

    r = sub.add_parser("regen", help="rollup+samples → envelope.json (state=measured)")
    r.add_argument("--now", required=True, help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    r.add_argument("--node-id", help="기본 = --node-dir 의 basename")
    r.add_argument("--window-days", type=int, default=14)
    r.add_argument("--mem-total-mib", type=int,
                   help="기본 = /proc/meminfo MemTotal (원격 노드 재생성 시 명시)")
    r.add_argument("-o", "--output", help="기본 = <node-dir>/envelope.json")
    r.add_argument("--dry-run", action="store_true", help="stdout 으로만 출력")
    r.set_defaults(func=cmd_regen)

    c = sub.add_parser("check-keys", help="eta_params 키가 blackbox_eta.DEFAULTS 와 맞는가(fail-loud)")
    c.add_argument("--envelope", help="기본 = <node-dir>/envelope.json")
    c.set_defaults(func=cmd_check_keys)

    s = sub.add_parser("staleness", help="포락선 노후 경고(차단 아님) + 이벤트")
    s.add_argument("--now", required=True)
    s.add_argument("--envelope")
    s.add_argument("--max-age-days", type=float, default=7.0)
    s.set_defaults(func=cmd_staleness)

    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()
    if not getattr(args, "func", None):
        ap.print_help()
        return 1
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
