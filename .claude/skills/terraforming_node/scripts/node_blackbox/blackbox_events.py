#!/usr/bin/env python3
"""blackbox_events.py -- 흩어진 킬 경로를 하나의 이벤트 평면으로 통합 (plan_26073109 §Phase 3).

문제(인터뷰 E1 진단): 이 노드에는 **무인 강제종료 경로가 4개**(+커널 OOM)인데 서로를 모르고
로그도 각자 다른 곳에 남는다 -- 사후에 "누가 죽였는지" 알려면 4곳을 따로 뒤져야 한다.
2026-07-30 두 번의 하드다운에서 이 산재가 그대로 비용이 됐다.

통합 대상:
  ① mem_watchdog 상시(레거시 `[mem-watchdog]`)      ② 노드블랙박스 ETA 워치독(`[bb-watchdog]`)
  ③ 협역 워치독(같은 접두어, 필터만 다름)            ④ engine_liveness_watchdog(`[liveness]`)
  ⑤ earlyoom(프로세스 레벨)                          ⑥ 커널 OOM killer
  ⑦ **열·전력 포락선 워치독**(`[tp-watchdog]` · plan_26082319 §6.4 · 2026-08-23 신설)

★ ⑦ 이 왜 추가됐나: 2026-08-23 R6 는 RAM OOM 이 아니라 **하드 락업**이었다. RAM 축 6 경로는
  전부 정상적으로 미발동했고 그것이 옳았다 -- 방어 표면 자체에 축이 없었다. 열·전력 축이
  생겼으므로 그 킬 경로도 같은 이벤트 평면으로 들어와야 한다. **경로가 늘었는데 통합이 안 되면
  사후분석은 다시 여러 곳을 뒤지게 된다** -- 이 파일의 존재 이유가 바로 그것이다.

전부 journald 에 있으므로 **journald 커서**로 증분 수집한다(멱등 -- 재실행이 중복을 만들지 않는다).
추가로 **부정 클린 부팅**(정상 종료 흔적 없이 끊긴 부팅 = 하드다운 후보)을 판정해 남긴다 --
다음 세션이 사후분석에 자동 진입하는 근거다(plan §Phase 4).

종료코드: 0=성공 · 1=인자/입력 오류 · 2=자체시험 실패.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import sys
from datetime import datetime, timezone

SCHEMA_VERSION = 1
CURSOR_FILE = ".cursors.json"


def _num_or_none(text):
    """'90' -> 90 · '90.5' -> 90.5 · 'na'(부재) -> None.

    ★ 'na' 를 0 으로 접지 않는다 -- 0 W/0 C 는 '아주 안전'을 뜻해서, 부재를 0 으로 적으면
      사후분석이 **센서가 죽어 있던 구간을 한산한 구간으로** 읽는다(수집기의 '부재는 빈 칸'
      규율과 같은 이유).
    """
    if text is None or text == "na":
        return None
    try:
        return int(text) if "." not in text else float(text)
    except ValueError:
        return None


# ── 메시지 파서 (본문만 받는다 -- 유닛/시각은 journald 메타에서 온다) ─────
_P = [
    # ① ② ③ 워치독 TRIP: 레거시 '[mem-watchdog]' 와 신규 '[bb-watchdog]' 양쪽
    (re.compile(r"\[(?:mem|bb)-watchdog\]\s+TRIP\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB"
                r".*?docker kill\s+(\S+)"),
     lambda m: {"kind": "watchdog_trip", "mem_avail_mib": int(m.group(1)),
                "threshold_mib": int(m.group(2)), "target": m.group(3),
                "action": "docker_kill", "rule": "absolute"}),
    # 신규 ETA 워치독 TRIP (rate/streak 포함 -- 판정 근거가 로그에 남는다)
    (re.compile(r"\[bb-watchdog\]\s+TRIP\s+mem=(\d+)MiB\s+rate=(-?\d+)MiB/s\s+streak=(\d+)"
                r"\s+→\s+docker kill\s+(.+?)\s+\d{4}-"),
     lambda m: {"kind": "watchdog_trip", "mem_avail_mib": int(m.group(1)),
                "rate_mib_s": int(m.group(2)), "streak": int(m.group(3)),
                "target": m.group(4).strip(), "action": "docker_kill", "rule": "eta"}),
    (re.compile(r"\[(?:mem|bb)-watchdog\]\s+TRIP-nomatch\s+(?:MemAvailable=|mem=)(\d+)MiB"),
     lambda m: {"kind": "watchdog_trip_nomatch", "mem_avail_mib": int(m.group(1)),
                "action": "none"}),
    (re.compile(r"\[bb-watchdog\]\s+TRIP-ARM\s+mem=(\d+)MiB\s+rate=(-?\d+)MiB/s"),
     lambda m: {"kind": "watchdog_trip_arm", "mem_avail_mib": int(m.group(1)),
                "rate_mib_s": int(m.group(2))}),
    # ⑦ 열·전력 포락선 워치독 -- RAM 과 무관한 **또 하나의 별개 축**(하드 락업 예방).
    #    `rule` 이 어느 축이 죽였는지를 남긴다(gpu_pwr_sustained · soc_temp_sustained ·
    #    soc_hard_ceiling). 사후분석에서 "무엇이 이 kill 을 만들었나"가 이벤트만으로 서야 한다.
    (re.compile(r"\[tp-watchdog\]\s+TRIP\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C\s+"
                r"buckets=(\d+)/(\d+)\s+streak=(\d+)\s+→\s+docker kill\s+(.+?)\s+\d{4}-"),
     lambda m: {"kind": "thermal_trip", "rule": m.group(1),
                "gpu_pwr_w": _num_or_none(m.group(2)), "soc_temp_c": _num_or_none(m.group(3)),
                "gpu_bucket": int(m.group(4)), "soc_bucket": int(m.group(5)),
                "soc_hard_streak": int(m.group(6)), "target": m.group(7).strip(),
                "action": "docker_kill"}),
    # dry-run(관측 전용)은 **별도 kind** 다 -- 같은 kind 로 내면 하류가 kill 로 오독한다.
    (re.compile(r"\[tp-watchdog\]\s+TRIP-DRYRUN\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C\s+"
                r"buckets=(\d+)/(\d+)"),
     lambda m: {"kind": "thermal_trip_dryrun", "rule": m.group(1),
                "gpu_pwr_w": _num_or_none(m.group(2)), "soc_temp_c": _num_or_none(m.group(3)),
                "gpu_bucket": int(m.group(4)), "soc_bucket": int(m.group(5)),
                "action": "none"}),
    (re.compile(r"\[tp-watchdog\]\s+TRIP-nomatch\s+rule=(\S+)\s+gpu=(\S+?)W\s+soc=(\S+?)C"),
     lambda m: {"kind": "thermal_trip_nomatch", "rule": m.group(1),
                "gpu_pwr_w": _num_or_none(m.group(2)), "soc_temp_c": _num_or_none(m.group(3)),
                "action": "none"}),
    # 수집기 꼬리가 뒤처져 전력 축이 **동결**된 구간. '조용한 무장해제'를 사후에 볼 수 있어야 한다.
    (re.compile(r"\[tp-watchdog\]\s+GPU-STALE\s+수집기 꼬리가\s+(\d+)s\s+뒤짐"),
     lambda m: {"kind": "thermal_gpu_stale", "lag_s": int(m.group(1)), "axis": "frozen"}),
    (re.compile(r"\[tp-watchdog\]\s+GPU-STALE\s+해제"),
     lambda m: {"kind": "thermal_gpu_stale_clear", "axis": "resumed"}),
    (re.compile(r"\[tp-watchdog\]\s+start\s+mode=(\S+)\s+filter='([^']*)'"),
     lambda m: {"kind": "thermal_watchdog_start", "mode": m.group(1), "filter": m.group(2)}),
    # ④ 엔진 교착(liveness) -- 메모리와 무관한 별개 축
    (re.compile(r"\[liveness\]\s+TRIP\s+엔진교착.*?정체\s+(\d+)s.*?docker kill\s+(\S+)"),
     lambda m: {"kind": "liveness_trip", "stalled_s": int(m.group(1)),
                "target": m.group(2), "action": "docker_kill"}),
    # ⑤ earlyoom -- 프로세스 레벨(컨테이너가 아니라 프로세스를 죽인다 = 빌드 평면도 사정권)
    (re.compile(r"sending SIG(TERM|KILL) to process (\d+)\s+uid\s+\d+\s+\"([^\"]*)\""),
     lambda m: {"kind": "earlyoom_kill", "signal": "SIG" + m.group(1),
                "pid": int(m.group(2)), "process": m.group(3), "action": "signal"}),
    # ⑥ 커널 OOM killer -- 우리 방어가 전부 늦었을 때만 도달하는 최종 지점
    (re.compile(r"Out of memory: Killed process (\d+) \(([^)]*)\)"),
     lambda m: {"kind": "kernel_oom_kill", "pid": int(m.group(1)),
                "process": m.group(2), "action": "kernel_kill"}),
    (re.compile(r"\[(?:mem|bb)-watchdog\]\s+start\s+filter='([^']*)'"),
     lambda m: {"kind": "watchdog_start", "filter": m.group(1)}),
]

# docker kill 이 표준출력으로 되돌린 컨테이너 ID = 킬 명령이 반환됐다는 증거
_ACK = re.compile(r"^\[(?:mem|bb)-watchdog\]\s+([0-9a-f]{8,})\s*$")
# 열·전력 워치독의 같은 증거. **kind 를 따로 둔다** -- 어느 축의 kill 이 실제로 반환됐는지가
# 갈려야 "트립은 났는데 kill 이 안 돌아왔다"를 축별로 판정할 수 있다.
_TP_ACK = re.compile(r"^\[tp-watchdog\]\s+([0-9a-f]{8,})\s*$")

# 정상 종료의 흔적. 이 중 어느 것도 없이 끊긴 부팅 = 부정 클린(하드다운 후보).
_CLEAN_SHUTDOWN = re.compile(
    r"(systemd-shutdown|Reached target\s+(Power-Off|Reboot|Shutdown|Halt)"
    r"|Shutting down|Powering off|Rebooting)", re.IGNORECASE)


def parse_message(msg):
    """journald MESSAGE 본문 -> 정규화 이벤트(dict) 또는 None."""
    if not msg:
        return None
    for rx, build in _P:
        m = rx.search(msg)
        if m:
            return build(m)
    m = _ACK.match(msg.strip())
    if m:
        return {"kind": "watchdog_kill_ack", "target": m.group(1)}
    m = _TP_ACK.match(msg.strip())
    if m:
        return {"kind": "thermal_kill_ack", "target": m.group(1)}
    return None


def journal_read(unit=None, kernel=False, after_cursor=None, extra_args=None):
    """journalctl -o json 증분 읽기 -> (events_raw, last_cursor). 실패는 ([], after_cursor)."""
    cmd = ["journalctl", "-o", "json", "--no-pager"]
    if kernel:
        cmd.append("-k")
    if unit:
        cmd += ["-u", unit]
    if after_cursor:
        cmd += ["--after-cursor", after_cursor]
    else:
        cmd += ["-n", "20000"]          # 최초 실행은 최근분만(전 기간은 seed_from_journal 소관)
    if extra_args:
        cmd += list(extra_args)
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=120).stdout
    except (OSError, subprocess.SubprocessError):
        return [], after_cursor
    rows, cursor = [], after_cursor
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            rec = json.loads(line)
        except ValueError:
            continue
        rows.append(rec)
        cursor = rec.get("__CURSOR", cursor)
    return rows, cursor


def _us_to_iso(value):
    """journald 는 시각을 **정수 마이크로초**로 준다(--list-boots 의 first_entry/last_entry 포함).
    문자열로 가정하면 하류에서 슬라이싱이 터진다 -- 실기 실행이 잡은 결함."""
    if value is None:
        return None
    if isinstance(value, str):
        return value
    try:
        return datetime.fromtimestamp(int(value) / 1e6,
                                      tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    except (ValueError, OSError, OverflowError, TypeError):
        return None


def _ts_from_record(rec):
    us = rec.get("__REALTIME_TIMESTAMP")
    if us:
        try:
            return datetime.fromtimestamp(int(us) / 1e6, tz=timezone.utc)
        except (ValueError, OSError, OverflowError):
            return None
    return None


def normalize(rec, source):
    """journald 레코드 -> 이벤트. MESSAGE 가 리스트(바이트열)인 경우도 처리."""
    msg = rec.get("MESSAGE")
    if isinstance(msg, list):
        try:
            msg = bytes(msg).decode("utf-8", "replace")
        except (TypeError, ValueError):
            msg = ""
    ev = parse_message(msg or "")
    if ev is None:
        return None
    ts = _ts_from_record(rec)
    ev["ts"] = ts.strftime("%Y-%m-%dT%H:%M:%SZ") if ts else None
    ev["source"] = source
    if rec.get("_BOOT_ID"):
        ev["boot_id"] = rec["_BOOT_ID"]
    return ev


def detect_unclean_boots(boot_summaries):
    """[{boot_id, first_ts, last_ts, tail_messages, is_current}] -> 부정 클린 부팅 목록.

    판정: 현재 부팅이 아니면서 꼬리 메시지에 정상 종료 흔적이 하나도 없으면 부정 클린.
    (하드다운은 정의상 종료 절차를 남기지 못한다.)"""
    out = []
    for b in boot_summaries:
        if b.get("is_current"):
            continue
        tail = "\n".join(b.get("tail_messages") or [])
        if not _CLEAN_SHUTDOWN.search(tail):
            out.append({"kind": "unclean_boot", "boot_id": b.get("boot_id"),
                        "first_ts": b.get("first_ts"), "last_ts": b.get("last_ts"),
                        "reason": "no_clean_shutdown_marker"})
    return out


def collect_boot_summaries(tail_n=40):
    """journalctl --list-boots 로 부팅 목록을 얻고 각 부팅의 꼬리 메시지를 수집."""
    try:
        raw = subprocess.run(["journalctl", "--list-boots", "-o", "json", "--no-pager"],
                             capture_output=True, text=True, timeout=60).stdout
        boots = json.loads(raw) if raw.strip() else []
    except (OSError, subprocess.SubprocessError, ValueError):
        return []
    out = []
    for b in boots:
        idx = b.get("index")
        bid = b.get("boot_id")
        try:
            tail = subprocess.run(["journalctl", "-b", str(idx), "-n", str(tail_n),
                                   "-o", "cat", "--no-pager"],
                                  capture_output=True, text=True, timeout=60).stdout
        except (OSError, subprocess.SubprocessError):
            tail = ""
        out.append({"boot_id": bid, "is_current": idx == 0,
                    "first_ts": _us_to_iso(b.get("first_entry")),
                    "last_ts": _us_to_iso(b.get("last_entry")),
                    "tail_messages": tail.splitlines()})
    return out


def load_cursors(events_dir):
    p = os.path.join(events_dir, CURSOR_FILE)
    if os.path.exists(p):
        try:
            with open(p, encoding="utf-8") as fh:
                return json.load(fh)
        except (OSError, ValueError):
            return {}
    return {}


def save_cursors(events_dir, cursors):
    os.makedirs(events_dir, exist_ok=True)
    p = os.path.join(events_dir, CURSOR_FILE)
    tmp = p + ".tmp"
    with open(tmp, "w", encoding="utf-8") as fh:
        json.dump(cursors, fh, ensure_ascii=False, indent=2, sort_keys=True)
        fh.write("\n")
        fh.flush()
        os.fsync(fh.fileno())
    os.replace(tmp, p)


def _inherit_dir_owner(path, parent):
    """root 로 만든 파일의 소유를 **디렉터리 소유자에게 넘긴다**.

    ★ 왜 필요한가(2026-08-01 실화): `events/<월>.jsonl` 은 root 데몬(이 파일·워치독·수명집행)과
      비-root 사용자 도구(`blackbox_session.py declare-budget`)가 **함께 append** 한다. install 은
      디렉터리만 위임 사용자 소유(0775)로 만들고 파일은 안 만들므로, 그 달 **먼저 쓴 쪽이 소유자**가
      된다. root 가 이기면 사용자 도구가 EACCES 로 죽는다 — 서브에서 실제로 발생해
      `declare-budget` 이 불가능해졌고, 그러면 ETA 워치독 위양성 방어가 통째로 못 선다.
      메인이 멀쩡했던 건 우연히 사용자 도구가 먼저 썼기 때문이다.
      **우연한 성공을 설계로 착각하지 않는다** — 디렉터리 소유자를 정본으로 삼아 상속시킨다.
    """
    try:
        if os.geteuid() != 0:
            return                      # root 가 아니면 애초에 소유 문제가 없다
        st = os.stat(parent)
        if os.stat(path).st_uid != st.st_uid:
            os.chown(path, st.st_uid, st.st_gid)
    except OSError:
        pass                            # 소유 정렬 실패가 이벤트 기록을 막아서는 안 된다


def append_events(events_dir, events):
    os.makedirs(events_dir, exist_ok=True)
    by_month = {}
    for e in events:
        month = (e.get("ts") or "0000-00")[:7]
        by_month.setdefault(month, []).append(e)
    for month, evs in sorted(by_month.items()):
        path = os.path.join(events_dir, "%s.jsonl" % month)
        with open(path, "a", encoding="utf-8") as fh:
            for e in evs:
                fh.write(json.dumps(e, ensure_ascii=False, sort_keys=True) + "\n")
        _inherit_dir_owner(path, events_dir)


DEFAULT_SOURCES = [
    ("easy-vllm-blackbox-watchdog", False),   # 신규 상시 ETA 워치독(RAM 축)
    ("easy-vllm-blackbox-thermal", False),    # 열·전력 포락선 워치독(하드락업 예방 축)
    ("easy-vllm-memwatch", False),            # 레거시(제거 전/이행기)
    ("earlyoom", False),
    (None, True),                             # 커널(-k) : OOM killer
]


def collect(node_dir, sources=None, detect_boots=True):
    events_dir = os.path.join(node_dir, "events")
    cursors = load_cursors(events_dir)
    found = []
    for unit, kernel in (sources or DEFAULT_SOURCES):
        key = "kernel" if kernel else ("unit:%s" % unit)
        rows, cursor = journal_read(unit=unit, kernel=kernel,
                                    after_cursor=cursors.get(key))
        for rec in rows:
            ev = normalize(rec, key)
            if ev:
                found.append(ev)
        if cursor:
            cursors[key] = cursor
    if detect_boots:
        seen = set(cursors.get("_unclean_boots", []))
        for ub in detect_unclean_boots(collect_boot_summaries()):
            if ub["boot_id"] in seen:
                continue
            seen.add(ub["boot_id"])
            ub["source"] = "boot_scan"
            ub["ts"] = ub.get("last_ts")
            found.append(ub)
        cursors["_unclean_boots"] = sorted(seen)
    if found:
        append_events(events_dir, found)
    save_cursors(events_dir, cursors)
    return found


def _self_test():
    import tempfile
    checks = []

    def kind_of(msg):
        e = parse_message(msg)
        return e["kind"] if e else None

    # ① 레거시 워치독
    e = parse_message("[mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB → docker kill 249d24630725 2026-07-22T08:48:59Z")
    checks.append(("레거시 TRIP 파싱", e and e["kind"] == "watchdog_trip"
                   and e["mem_avail_mib"] == 10186 and e["target"] == "249d24630725"
                   and e["rule"] == "absolute"))
    # ② 신규 ETA 워치독
    e = parse_message("[bb-watchdog] TRIP mem=37000MiB rate=6241MiB/s streak=3 → docker kill abc123 def456 2026-07-31T00:00:00Z")
    checks.append(("ETA TRIP 파싱(rate·streak 보존)",
                   e and e["kind"] == "watchdog_trip" and e["rule"] == "eta"
                   and e["rate_mib_s"] == 6241 and e["streak"] == 3
                   and e["target"] == "abc123 def456"))
    e = parse_message("[bb-watchdog] TRIP-ARM mem=40000MiB rate=6000MiB/s (디바운스 1/3) 2026-07-31T00:00:00Z")
    checks.append(("TRIP-ARM(디바운스 진행) 파싱", e and e["kind"] == "watchdog_trip_arm"))
    checks.append(("TRIP-nomatch 파싱",
                   kind_of("[bb-watchdog] TRIP-nomatch mem=5000MiB rate=10MiB/s (filter='@vllm' 매칭 0)")
                   == "watchdog_trip_nomatch"))
    checks.append(("kill-ack 파싱", kind_of("[mem-watchdog] 249d24630725") == "watchdog_kill_ack"))
    checks.append(("start 파싱", kind_of("[bb-watchdog] start filter='@vllm' interval=1s") == "watchdog_start"))
    # ⑦ 열·전력 포락선 워치독 (plan_26082319 §6.4)
    #    ★ 픽스처는 thermal_watchdog.sh 가 **실제로 찍는 문자열**이어야 한다. 손으로 지어낸
    #      문자열로 시험하면 로그 포맷이 바뀌어도 PASS 가 나고, 그 사이 이벤트는 조용히 사라진다.
    e = parse_message("[tp-watchdog] TRIP rule=gpu_pwr_sustained gpu=90W soc=47C "
                      "buckets=60/0 streak=0 → docker kill cafe1234 2026-08-23T09:24:36Z")
    checks.append(("열·전력 TRIP 파싱(축·버킷 보존)",
                   e and e["kind"] == "thermal_trip" and e["rule"] == "gpu_pwr_sustained"
                   and e["gpu_pwr_w"] == 90 and e["soc_temp_c"] == 47
                   and e["gpu_bucket"] == 60 and e["target"] == "cafe1234"
                   and e["action"] == "docker_kill"))
    e = parse_message("[tp-watchdog] TRIP rule=soc_hard_ceiling gpu=45W soc=97C "
                      "buckets=0/12 streak=3 → docker kill a1b2c3 d4e5f6 2026-08-23T09:24:36Z")
    checks.append(("열·전력 TRIP: SoC 즉시계층 규칙 라벨·다중 타깃",
                   e and e["rule"] == "soc_hard_ceiling" and e["soc_hard_streak"] == 3
                   and e["target"] == "a1b2c3 d4e5f6"))
    # 부재('na')는 0 이 아니다 -- 센서 사망 구간을 '한산'으로 읽지 않게
    e = parse_message("[tp-watchdog] TRIP rule=gpu_pwr_sustained gpu=90W soc=naC "
                      "buckets=60/0 streak=0 → docker kill z9 2026-08-23T09:24:36Z")
    checks.append(("열·전력 TRIP: 부재(na)는 None(0 으로 접지 않는다)",
                   e and e["soc_temp_c"] is None and e["gpu_pwr_w"] == 90))
    e = parse_message("[tp-watchdog] TRIP-DRYRUN rule=gpu_pwr_sustained gpu=90W soc=47C "
                      "buckets=60/0 streak=0 → 실무장이었으면 docker kill cafe1234 2026-08-23T09:24:36Z")
    checks.append(("열·전력 dry-run 은 별도 kind(하류 오독 방지)",
                   e and e["kind"] == "thermal_trip_dryrun" and e["action"] == "none"))
    checks.append(("★dry-run 이 실무장 kind 로 새지 않는다",
                   parse_message("[tp-watchdog] TRIP-DRYRUN rule=gpu_pwr_sustained gpu=90W "
                                 "soc=47C buckets=60/0 streak=0 → 실무장이었으면 docker kill x "
                                 "2026-08-23T09:24:36Z")["kind"] != "thermal_trip"))
    checks.append(("열·전력 TRIP-nomatch 파싱",
                   kind_of("[tp-watchdog] TRIP-nomatch rule=gpu_pwr_sustained gpu=90W soc=47C "
                           "(filter='@vllm' 매칭 0) 2026-08-23T09:24:36Z")
                   == "thermal_trip_nomatch"))
    e = parse_message("[tp-watchdog] GPU-STALE 수집기 꼬리가 37s 뒤짐(>10s) — 전력 축 동결. "
                      "수집기 확인: systemctl status easy-vllm-blackbox-collect 2026-08-23T09:24:36Z")
    checks.append(("전력 축 동결(stale) 파싱 — 조용한 무장해제를 사후에 본다",
                   e and e["kind"] == "thermal_gpu_stale" and e["lag_s"] == 37
                   and e["axis"] == "frozen"))
    checks.append(("stale 해제 파싱",
                   kind_of("[tp-watchdog] GPU-STALE 해제 — 전력 축 재개 2026-08-23T09:24:36Z")
                   == "thermal_gpu_stale_clear"))
    e = parse_message("[tp-watchdog] start mode=armed filter='@vllm' interval=1s "
                      "params=/etc/easy-vllm/thermal_params.env node=/x/y")
    checks.append(("열·전력 워치독 start 파싱(mode 보존)",
                   e and e["kind"] == "thermal_watchdog_start" and e["mode"] == "armed"
                   and e["filter"] == "@vllm"))
    checks.append(("열·전력 kill-ack 파싱(축별로 갈린다)",
                   kind_of("[tp-watchdog] cafe1234deadbeef") == "thermal_kill_ack"))
    checks.append(("RAM 축 kill-ack 은 여전히 RAM kind(축 혼선 없음)",
                   kind_of("[bb-watchdog] cafe1234deadbeef") == "watchdog_kill_ack"))
    # 새 유닛이 수집 소스에 배선됐는가 — "만든 것과 도는 것은 다르다"
    checks.append(("★열·전력 유닛이 수집 소스에 배선됨(배선 실증)",
                   ("easy-vllm-blackbox-thermal", False) in DEFAULT_SOURCES))

    # ④ liveness
    e = parse_message("[liveness] TRIP 엔진교착: running=1 reachable=1 progress 정체 600s(>=600) → docker kill mn-x 2026-07-31T00:00:00Z")
    checks.append(("liveness TRIP 파싱", e and e["kind"] == "liveness_trip"
                   and e["stalled_s"] == 600 and e["target"] == "mn-x"))
    # ⑤ earlyoom
    e = parse_message('earlyoom[123]: sending SIGTERM to process 4567 uid 0 "VLLM::EngineCore": badness 100, VmRSS 90000 MiB')
    checks.append(("earlyoom 파싱", e and e["kind"] == "earlyoom_kill"
                   and e["pid"] == 4567 and e["process"] == "VLLM::EngineCore"))
    # ⑥ 커널 OOM
    e = parse_message("Out of memory: Killed process 8901 (python3) total-vm:100000kB, anon-rss:50000kB")
    checks.append(("커널 OOM 파싱", e and e["kind"] == "kernel_oom_kill" and e["pid"] == 8901))
    # 무관 라인
    checks.append(("무관 라인 -> None", parse_message("Started something.service") is None))
    checks.append(("빈 입력 -> None", parse_message("") is None))

    # 부정 클린 부팅 판정
    boots = [
        {"boot_id": "b1", "is_current": False, "first_ts": 1, "last_ts": 2,
         "tail_messages": ["foo", "systemd-shutdown[1]: Powering off.", "bar"]},
        {"boot_id": "b2", "is_current": False, "first_ts": 3, "last_ts": 4,
         "tail_messages": ["[mem-watchdog] HB MemAvailable=24000MiB", "kernel: something"]},
        {"boot_id": "b3", "is_current": True, "first_ts": 5, "last_ts": 6,
         "tail_messages": ["still running"]},
    ]
    ub = detect_unclean_boots(boots)
    checks.append(("정상 종료 부팅은 제외", all(x["boot_id"] != "b1" for x in ub)))
    checks.append(("종료 흔적 없는 부팅 = 부정클린", any(x["boot_id"] == "b2" for x in ub)))
    checks.append(("현재 부팅은 판정 제외", all(x["boot_id"] != "b3" for x in ub)))
    checks.append(("'Reached target Reboot' 도 정상으로 인정",
                   not detect_unclean_boots([{"boot_id": "b4", "is_current": False,
                                              "tail_messages": ["Reached target Reboot."]}])))
    # journald 정수 마이크로초 -> ISO (실기 실행이 잡은 결함의 회귀 고정).
    # 손으로 적은 기대문자열은 그 자체가 오류원이므로(첫 시도에서 실제로 틀렸다) 명백한 기준점과
    # 독립 계산 왕복으로 고정한다.
    import calendar
    checks.append(("epoch 0 -> 1970-01-01T00:00:00Z", _us_to_iso(0) == "1970-01-01T00:00:00Z"))
    _probe = 1785464193
    checks.append(("정수 μs 변환 왕복 일치",
                   calendar.timegm(datetime.strptime(_us_to_iso(_probe * 1000000),
                                                     "%Y-%m-%dT%H:%M:%SZ").timetuple()) == _probe))
    checks.append(("문자열은 그대로 통과", _us_to_iso("2026-07-31T00:00:00Z") == "2026-07-31T00:00:00Z"))
    checks.append(("None 은 None", _us_to_iso(None) is None))
    ub_int = detect_unclean_boots([{"boot_id": "b5", "is_current": False,
                                    "last_ts": _us_to_iso(1785464193000000),
                                    "tail_messages": ["nothing clean here"]}])
    checks.append(("부정클린 이벤트의 ts 가 문자열",
                   bool(ub_int) and isinstance(ub_int[0]["last_ts"], str)))

    # journald 레코드 정규화 + 저장/커서
    with tempfile.TemporaryDirectory() as td:
        nd = os.path.join(td, "spark-x")
        ed = os.path.join(nd, "events")
        rec = {"MESSAGE": "[bb-watchdog] TRIP mem=5000MiB rate=2000MiB/s streak=3 → docker kill zz 2026-07-31T00:00:00Z",
               "__REALTIME_TIMESTAMP": "1785464193000000", "_BOOT_ID": "bootA",
               "__CURSOR": "s=1;i=2"}
        ev = normalize(rec, "unit:test")
        checks.append(("정규화: ts UTC 변환", ev and ev["ts"].endswith("Z") and ev["ts"].startswith("2026-")))
        checks.append(("정규화: boot_id 보존", ev and ev["boot_id"] == "bootA"))
        checks.append(("정규화: source 부착", ev and ev["source"] == "unit:test"))
        rec_bytes = {"MESSAGE": list(b"Out of memory: Killed process 1 (x)"),
                     "__REALTIME_TIMESTAMP": "1785464193000000"}
        checks.append(("MESSAGE 바이트열 처리",
                       (normalize(rec_bytes, "k") or {}).get("kind") == "kernel_oom_kill"))

        append_events(ed, [ev])
        append_events(ed, [ev])
        n = len(open(os.path.join(ed, "2026-07.jsonl"), encoding="utf-8").read().strip().split("\n"))
        checks.append(("append 는 월별 파일에 누적", n == 2))
        save_cursors(ed, {"unit:test": "s=1;i=2"})
        checks.append(("커서 저장/복원", load_cursors(ed).get("unit:test") == "s=1;i=2"))
        checks.append(("커서 파일 부재 시 빈 dict",
                       load_cursors(os.path.join(td, "nope")) == {}))

    ok = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok = ok and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok else "FAIL", len(checks)))
    return 0 if ok else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="4개 킬 경로 -> 단일 이벤트 평면")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--no-boot-scan", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        return _self_test()
    if not args.node_dir:
        ap.error("--node-dir 가 필요합니다")
    found = collect(args.node_dir, detect_boots=not args.no_boot_scan)
    by_kind = {}
    for e in found:
        by_kind[e["kind"]] = by_kind.get(e["kind"], 0) + 1
    print("[events] 신규 %d건 -> %s/events" % (len(found), args.node_dir))
    for k, v in sorted(by_kind.items()):
        print("           %-24s %d" % (k, v))
    return 0


if __name__ == "__main__":
    sys.exit(main())
