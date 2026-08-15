#!/usr/bin/env python3
"""seed_from_journal.py -- Phase 0 증거 수확기 (plan_26073109 §Phase 0).

레거시 mem_watchdog systemd 저널을 노드블랙박스 `docs/logs/` 평면의 **시드**로 변환한다.
이 저널은 하강률(1.0~1.9 GiB/s)·kill 지연(<=4s)의 유일한 원천이자 포락선 학습의 초기
데이터이며, 저널은 vacuum 대상이므로 안전체계 제거(Phase 1) **전에** 반드시 수확해야 한다.

음성정직 원칙: 시드는 15초 해상도 HB 재구성이지 1초 실샘플이 아니다. GPU/load/컨테이너수
필드는 저널에 존재하지 않는다. 따라서 canonical `samples/` 가 아니라 `seed/` 하위에 별도
스키마로 적재하고, 부재 필드를 `seed/README.json` 에 명시한다(조용한 통과 금지).

입력  : journalctl -u easy-vllm-memwatch --no-pager -o short-iso 의 출력 (파일 또는 stdin)
출력  : <logs_root>/<node_id>/seed/{memwatch-journal.txt, samples_15s.csv, README.json}
        <logs_root>/<node_id>/events/<YYYY-MM>.jsonl      (canonical -- 실제 이벤트)
        <logs_root>/<node_id>/envelope.json               (초기 포락선)

시각은 --generated-utc 주입만 사용한다(벽시계 금지 -- staleness_gate/publisher 규약 정합).
종료코드: 0=성공 · 1=입력/인자 오류 · 2=게이트 실패(보존율 미달 등).
"""
from __future__ import annotations

import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone

SCHEMA_VERSION = 1

# [mem-watchdog] HB MemAvailable=24926MiB min=20165MiB 2026-07-22T08:44:56Z
_HB = re.compile(
    r"\[mem-watchdog\]\s+HB\s+MemAvailable=(\d+)MiB(?:\s+min=(\d+)MiB)?\s+"
    r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")
# [mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB -> docker kill 249d24630725 2026-07-22T08:48:59Z
_TRIP = re.compile(
    r"\[mem-watchdog\]\s+TRIP\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB\s+\S+\s+docker kill\s+(\S+)\s+"
    r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")
# [mem-watchdog] TRIP-nomatch MemAvailable=NMiB < NMiB (filter='...' 매칭 0) <ts>
_TRIP_NOMATCH = re.compile(
    r"\[mem-watchdog\]\s+TRIP-nomatch\s+MemAvailable=(\d+)MiB\s+<\s+(\d+)MiB.*?"
    r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")
# [mem-watchdog] start filter='@vllm' threshold=10240MiB interval=1s heartbeat=15s pid=13211 <ts>
_START = re.compile(
    r"\[mem-watchdog\]\s+start\s+filter='([^']*)'\s+threshold=(\d+)MiB\s+interval=(\d+)s\s+"
    r"heartbeat=(\d+)s\s+pid=(\d+)\s+(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")
# journalctl -o short-iso 접두: '<ts±offset> <host> <unit[pid]>: <message>'
_PREFIX = re.compile(
    r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2})\s+(\S+)\s+([^:]+):\s*(.*)$")
_UNIT = re.compile(r"^(Started|Stopped|Stopping)\b")
_UNIT_STATE = re.compile(r"^\S+\.service:\s+(Deactivated successfully|Scheduled restart)")
# '-- Boot <128bit-id> --' -- 저널 자체 마커. 자기 타임스탬프가 없어 직후 라인 시각을 근사로 쓴다.
_BOOT = re.compile(r"^--\s+Boot\s+([0-9a-f]+)\s+--\s*$")
# docker kill 이 표준출력으로 되돌려준 컨테이너 ID = kill 확인응답(명령이 반환됐다는 증거)
_KILL_ACK = re.compile(r"^\[mem-watchdog\]\s+([0-9a-f]{8,})\s*$")
# systemd 자원회계 -- 알려진 형태이나 블랙박스 가치 없음(ignored_known 으로 분류: 미인식과 구분)
_ACCOUNTING = re.compile(r"^\S+\.service:\s+Consumed\b")

_CSV_HEADER = "ts_utc,mem_avail_mib,mem_min_mib,dt_s,mem_rate_mib_s\n"


def _parse_iso_z(text: str) -> datetime:
    return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)


def _validate_generated_utc(text: str) -> str:
    try:
        _parse_iso_z(text)
    except ValueError:
        raise SystemExit("seed_from_journal: --generated-utc must be 'YYYY-MM-DDTHH:MM:SSZ', got %r" % text)
    return text


def parse_journal(lines):
    """Return (samples, events, counters). Deterministic, order-preserving."""
    samples, events = [], []
    counters = {"total": 0, "hb": 0, "trip": 0, "trip_nomatch": 0, "start": 0,
                "unit": 0, "boot": 0, "kill_ack": 0, "ignored_known": 0, "unmatched": 0}
    pending_boots = []          # 자기 시각이 없는 boot 마커 -- 직후 라인 시각으로 확정
    for raw in lines:
        line = raw.rstrip("\n")
        if not line.strip():
            continue
        counters["total"] += 1

        m = _BOOT.match(line)
        if m:
            counters["boot"] += 1
            pending_boots.append(m.group(1))
            continue

        pm = _PREFIX.match(line)
        jts = None
        if pm:
            try:
                jts = datetime.fromisoformat(pm.group(1)).astimezone(timezone.utc)
            except ValueError:
                jts = None
        # 보류 중인 boot 마커를 이 라인의 시각으로 확정(근사 표기 -- 조용한 정밀도 위장 금지)
        if jts is not None and pending_boots:
            for boot_id in pending_boots:
                events.append({"ts": jts, "kind": "boot_marker", "boot_id": boot_id,
                               "ts_approx": True, "source": "seed:memwatch_journal"})
            pending_boots = []

        body = pm.group(4) if pm else line

        m = _KILL_ACK.match(body)
        if m and jts is not None:
            counters["kill_ack"] += 1
            events.append({"ts": jts, "kind": "watchdog_kill_ack", "target": m.group(1),
                           "source": "seed:memwatch_journal"})
            continue

        if _ACCOUNTING.match(body):
            counters["ignored_known"] += 1
            continue

        m = _UNIT_STATE.match(body)
        if m and jts is not None:
            counters["unit"] += 1
            events.append({"ts": jts,
                           "kind": "watchdog_unit_" + m.group(1).split()[0].lower(),
                           "source": "seed:memwatch_journal"})
            continue

        m = _HB.search(line)
        if m:
            counters["hb"] += 1
            samples.append({
                "ts": _parse_iso_z(m.group(3)),
                "mem_avail": int(m.group(1)),
                # 구버전 HB 에는 min= 이 없다(mem_watchdog P2 이전) -- 부재는 빈 칸으로 정직 표기
                "mem_min": int(m.group(2)) if m.group(2) else None,
            })
            continue

        m = _TRIP.search(line)
        if m:
            counters["trip"] += 1
            events.append({
                "ts": _parse_iso_z(m.group(4)),
                "kind": "watchdog_trip",
                "mem_avail_mib": int(m.group(1)),
                "threshold_mib": int(m.group(2)),
                "target": m.group(3),
                "action": "docker_kill",
                "source": "seed:memwatch_journal",
            })
            continue

        m = _TRIP_NOMATCH.search(line)
        if m:
            counters["trip_nomatch"] += 1
            events.append({
                "ts": _parse_iso_z(m.group(3)),
                "kind": "watchdog_trip_nomatch",
                "mem_avail_mib": int(m.group(1)),
                "threshold_mib": int(m.group(2)),
                "action": "none",
                "source": "seed:memwatch_journal",
            })
            continue

        m = _START.search(line)
        if m:
            counters["start"] += 1
            events.append({
                "ts": _parse_iso_z(m.group(6)),
                "kind": "watchdog_start",
                "filter": m.group(1),
                "threshold_mib": int(m.group(2)),
                "interval_s": int(m.group(3)),
                "heartbeat_s": int(m.group(4)),
                "pid": int(m.group(5)),
                "source": "seed:memwatch_journal",
            })
            continue

        m = _UNIT.match(body)
        if m and jts is not None:
            # systemd 라인은 자체 UTC 가 없다 -- 저널 타임스탬프(로컬 오프셋)를 UTC 로 정규화
            counters["unit"] += 1
            events.append({
                "ts": jts,
                "kind": "watchdog_unit_" + m.group(1).lower(),
                "source": "seed:memwatch_journal",
            })
            continue

        counters["unmatched"] += 1

    samples.sort(key=lambda s: s["ts"])
    events.sort(key=lambda e: e["ts"])
    return samples, events, counters


def compute_rates(samples):
    """Attach dt_s and signed mem_rate_mib_s (negative = falling). Gaps > 120s are not rated."""
    prev = None
    for s in samples:
        if prev is None:
            s["dt_s"], s["mem_rate"] = None, None
        else:
            dt = (s["ts"] - prev["ts"]).total_seconds()
            if dt <= 0 or dt > 120:
                s["dt_s"], s["mem_rate"] = (dt if dt > 0 else None), None
            else:
                s["dt_s"] = dt
                s["mem_rate"] = round((s["mem_avail"] - prev["mem_avail"]) / dt, 2)
        prev = s
    return samples


def _pct(sorted_vals, q):
    if not sorted_vals:
        return None
    idx = min(len(sorted_vals) - 1, max(0, int(round(q * (len(sorted_vals) - 1)))))
    return sorted_vals[idx]


def kill_latency_bounds(samples, events, recovery_mib=20000):
    """각 trip 이후 '큰 회복'을 보인 첫 HB 까지의 시간 = kill 지연의 **상한**(HB 15s 해상도).
    실제 지연은 이보다 짧다 -- 과대평가임을 명시해 사용한다."""
    out = []
    for ev in events:
        if ev["kind"] != "watchdog_trip":
            continue
        base = ev["mem_avail_mib"]
        for s in samples:
            if s["ts"] <= ev["ts"]:
                continue
            if (s["ts"] - ev["ts"]).total_seconds() > 300:
                break
            if s["mem_avail"] - base >= recovery_mib:
                out.append({
                    "trip_utc": ev["ts"].strftime("%Y-%m-%dT%H:%M:%SZ"),
                    "target": ev.get("target"),
                    "trip_mem_mib": base,
                    "recovered_mem_mib": s["mem_avail"],
                    "latency_upper_bound_s": (s["ts"] - ev["ts"]).total_seconds(),
                })
                break
    return out


def build_envelope(node_id, generated_utc, samples, events, counters, latencies):
    falling = sorted(-s["mem_rate"] for s in samples
                     if s["mem_rate"] is not None and s["mem_rate"] < 0)
    mem_vals = sorted(s["mem_avail"] for s in samples)
    lat = [x["latency_upper_bound_s"] for x in latencies]
    trips = [e for e in events if e["kind"] == "watchdog_trip"]
    return {
        "schema_version": SCHEMA_VERSION,
        "node_id": node_id,
        "generated_utc": generated_utc,
        "state": "seed",
        "provenance": {
            "source": "mem_watchdog systemd journal (legacy)",
            "resolution_s": 15,
            "absent_fields": ["gpu_temp", "gpu_pwr", "gpu_sm", "gpu_util",
                              "gpu_mem", "load1", "ctr_n"],
            "caveat": ("15s 해상도 재구성 -- 순간 최대 하강률은 과소평가되고 kill 지연은 "
                       "과대평가된다. 1초 실샘플로 대체될 때까지 보수적으로만 사용한다."),
        },
        "samples": {
            "count": len(samples),
            "first_utc": samples[0]["ts"].strftime("%Y-%m-%dT%H:%M:%SZ") if samples else None,
            "last_utc": samples[-1]["ts"].strftime("%Y-%m-%dT%H:%M:%SZ") if samples else None,
            "rated": len(falling) + sum(1 for s in samples
                                        if s["mem_rate"] is not None and s["mem_rate"] >= 0),
        },
        "descent_rate_mib_s": {
            "falling_samples": len(falling),
            "p50": _pct(falling, 0.50), "p90": _pct(falling, 0.90),
            "p99": _pct(falling, 0.99), "max": falling[-1] if falling else None,
        },
        "mem_avail_mib": {
            "min": mem_vals[0] if mem_vals else None,
            "p01": _pct(mem_vals, 0.01), "p50": _pct(mem_vals, 0.50),
            "max": mem_vals[-1] if mem_vals else None,
        },
        "kill": {
            "trips": len(trips),
            "threshold_mib": trips[0]["threshold_mib"] if trips else None,
            "trip_mem_mib": [t["mem_avail_mib"] for t in trips],
            "latency_upper_bound_s": {
                "observed": lat,
                "max": max(lat) if lat else None,
            },
        },
        "eta_params": {
            "daemon_kill_s": 6, "agent_act_s": 300, "agent_notify_s": 900,
            "eta_floor_s": 6.0,
            "provenance": "plan_26073109 §2.2 초기 임의값 -- Plan B 캠페인에서 실측 근거로 교체",
        },
        "counters": counters,
    }


def write_outputs(root, node_id, journal_text, samples, events, envelope, readme):
    node_dir = os.path.join(root, node_id)
    seed_dir = os.path.join(node_dir, "seed")
    events_dir = os.path.join(node_dir, "events")
    for d in (seed_dir, events_dir):
        os.makedirs(d, exist_ok=True)

    with open(os.path.join(seed_dir, "memwatch-journal.txt"), "w", encoding="utf-8") as fh:
        fh.write(journal_text)

    with open(os.path.join(seed_dir, "samples_15s.csv"), "w", encoding="utf-8") as fh:
        fh.write(_CSV_HEADER)
        for s in samples:
            fh.write("%s,%d,%s,%s,%s\n" % (
                s["ts"].strftime("%Y-%m-%dT%H:%M:%SZ"),
                s["mem_avail"],
                "" if s["mem_min"] is None else s["mem_min"],
                "" if s["dt_s"] is None else ("%g" % s["dt_s"]),
                "" if s["mem_rate"] is None else ("%g" % s["mem_rate"]),
            ))

    by_month = {}
    for e in events:
        by_month.setdefault(e["ts"].strftime("%Y-%m"), []).append(e)
    for month, evs in sorted(by_month.items()):
        with open(os.path.join(events_dir, "%s.jsonl" % month), "a", encoding="utf-8") as fh:
            for e in evs:
                rec = dict(e)
                rec["ts"] = e["ts"].strftime("%Y-%m-%dT%H:%M:%SZ")
                fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")

    with open(os.path.join(seed_dir, "README.json"), "w", encoding="utf-8") as fh:
        json.dump(readme, fh, ensure_ascii=False, indent=2, sort_keys=True)
        fh.write("\n")
    with open(os.path.join(node_dir, "envelope.json"), "w", encoding="utf-8") as fh:
        json.dump(envelope, fh, ensure_ascii=False, indent=2, sort_keys=True)
        fh.write("\n")
    return node_dir


def main(argv=None):
    ap = argparse.ArgumentParser(description="mem_watchdog 저널 -> 노드블랙박스 시드")
    # --self-test 는 하드웨어·인자 불요(회귀 고정용) -- required 는 main 에서 조건부 강제한다.
    # A7: 호출자 공급 인자다(파생 없음). 예시는 **role 슬러그** — 예전엔 hostname 을 적어
    # 뒀는데, 이 파일은 tracked 배포 산출물이라 4종 PII 전부가 강제되고 `spark-host` 에
    # 걸렸다. 스킴 정본은 terraforming_node SKILL.md §2.7.6 (node_id ::= manifest role).
    ap.add_argument("--node-id", help="manifest nodes[].role 슬러그 (예: main · sub)")
    ap.add_argument("--journal", default="-", help="저널 덤프 파일 (기본 '-' = stdin)")
    ap.add_argument("--logs-root", default="docs/logs")
    ap.add_argument("--generated-utc", help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    ap.add_argument("--min-retention", type=float, default=0.99,
                    help="게이트: (인식 라인 / 전체 라인) 최소 보존율")
    ap.add_argument("--expect-trips", type=int, default=None,
                    help="게이트: 기대 TRIP 건수 (실측으로 주입)")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()

    missing = [n for n, v in (("--node-id", args.node_id),
                              ("--generated-utc", args.generated_utc)) if not v]
    if missing:
        ap.error("the following arguments are required: %s" % ", ".join(missing))
    generated_utc = _validate_generated_utc(args.generated_utc)
    if args.journal == "-":
        journal_text = sys.stdin.read()
    else:
        with open(args.journal, encoding="utf-8", errors="replace") as fh:
            journal_text = fh.read()

    samples, events, counters = parse_journal(journal_text.splitlines())
    compute_rates(samples)
    latencies = kill_latency_bounds(samples, events)
    envelope = build_envelope(args.node_id, generated_utc, samples, events, counters, latencies)

    recognized = counters["total"] - counters["unmatched"]
    retention = (recognized / counters["total"]) if counters["total"] else 0.0
    readme = {
        "schema_version": SCHEMA_VERSION,
        "node_id": args.node_id,
        "generated_utc": generated_utc,
        "purpose": "plan_26073109 Phase 0 -- 안전체계 제거 전 포락선 초기 데이터 보존",
        "source": "journalctl -u easy-vllm-memwatch --no-pager -o short-iso",
        "resolution_s": 15,
        "absent_fields": envelope["provenance"]["absent_fields"],
        "counters": counters,
        "retention": round(retention, 6),
        "kill_latency_upper_bounds": latencies,
        "note": ("samples_15s.csv 는 canonical samples/ 가 아니다 -- 1초 실샘플과 혼동 금지. "
                 "events/ 는 canonical 로 적재된다(실제 발생 이벤트이므로)."),
    }
    node_dir = write_outputs(args.logs_root, args.node_id, journal_text,
                             samples, events, envelope, readme)

    trips = counters["trip"]
    print("[seed] node=%s -> %s" % (args.node_id, node_dir))
    print("[seed] 라인 %d (인식 %d · 미인식 %d) · 보존율 %.4f" % (
        counters["total"], recognized, counters["unmatched"], retention))
    print("[seed] HB %d · TRIP %d · kill_ack %d · boot %d · start %d · unit %d "
          "· TRIP-nomatch %d · 알려진무시 %d" % (
              counters["hb"], trips, counters["kill_ack"], counters["boot"],
              counters["start"], counters["unit"], counters["trip_nomatch"],
              counters["ignored_known"]))
    d = envelope["descent_rate_mib_s"]
    print("[seed] 하강률 MiB/s: p50=%s p90=%s p99=%s max=%s (falling %d)" % (
        d["p50"], d["p90"], d["p99"], d["max"], d["falling_samples"]))
    print("[seed] kill 지연 상한(s): %s" % envelope["kill"]["latency_upper_bound_s"]["observed"])

    failed = []
    if retention < args.min_retention:
        failed.append("보존율 %.4f < %.4f" % (retention, args.min_retention))
    if args.expect_trips is not None and trips != args.expect_trips:
        failed.append("TRIP %d != 기대 %d" % (trips, args.expect_trips))
    if failed:
        print("[seed] GATE FAIL: " + " · ".join(failed), file=sys.stderr)
        return 2
    print("[seed] GATE PASS")
    return 0


def _self_test():
    fixture = "\n".join([
        "2026-07-22T17:44:56+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=40104MiB min=39537MiB 2026-07-22T08:44:56Z",
        "2026-07-22T17:45:11+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=11419MiB min=11419MiB 2026-07-22T08:45:11Z",
        "2026-07-22T17:45:14+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] TRIP MemAvailable=10186MiB < 10240MiB → docker kill deadbeef 2026-07-22T08:45:14Z",
        "2026-07-22T17:45:15+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] deadbeef",
        "2026-07-22T17:45:26+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=117283MiB min=10186MiB 2026-07-22T08:45:26Z",
        "2026-07-22T17:45:41+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] HB MemAvailable=117300MiB 2026-07-22T08:45:41Z",
        "2026-07-22T17:45:42+09:00 h easy-vllm-memwatch[1]: [mem-watchdog] start filter='@vllm' threshold=10240MiB interval=1s heartbeat=15s pid=99 2026-07-22T08:45:42Z",
        "2026-07-22T17:45:43+09:00 h systemd[1]: Stopped easy-vllm-memwatch.service - x.",
        "2026-07-22T17:45:44+09:00 h systemd[1]: easy-vllm-memwatch.service: Consumed 1.2s CPU time.",
        "-- Boot abc123def456 --",
        "2026-07-22T17:50:00+09:00 h easy-vllm-memwatch[2]: [mem-watchdog] HB MemAvailable=118000MiB min=118000MiB 2026-07-22T08:50:00Z",
    ])
    samples, events, counters = parse_journal(fixture.splitlines())
    compute_rates(samples)
    lat = kill_latency_bounds(samples, events)
    boots = [e for e in events if e["kind"] == "boot_marker"]
    checks = [
        ("HB 5건", counters["hb"] == 5),
        ("TRIP 1건", counters["trip"] == 1),
        ("start 1건", counters["start"] == 1),
        ("unit 1건", counters["unit"] == 1),
        ("kill_ack 1건", counters["kill_ack"] == 1),
        ("boot 마커 1건", counters["boot"] == 1 and len(boots) == 1),
        ("boot 시각=직후 라인, 근사표기", boots and boots[0]["ts_approx"] is True
         and boots[0]["ts"].strftime("%H:%M:%SZ") == "08:50:00Z"),
        ("자원회계는 ignored_known(미인식 아님)", counters["ignored_known"] == 1),
        ("미인식 0", counters["unmatched"] == 0),
        ("min 부재 정직 표기", samples[3]["mem_min"] is None),
        ("하강률 = (11419-40104)/15", samples[1]["mem_rate"] == round((11419 - 40104) / 15, 2)),
        # TRIP 08:45:14Z -> 회복 HB 08:45:26Z = 12s (HB 15s 격자에 걸리는 상한값)
        ("kill 지연 상한 12s", bool(lat) and lat[0]["latency_upper_bound_s"] == 12.0),
        ("이벤트 시각 정렬", [e["ts"] for e in events] == sorted(e["ts"] for e in events)),
    ]
    ok = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok = ok and passed
    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


if __name__ == "__main__":
    sys.exit(main())
