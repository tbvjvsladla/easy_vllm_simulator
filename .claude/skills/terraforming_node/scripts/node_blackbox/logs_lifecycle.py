#!/usr/bin/env python3
"""logs_lifecycle.py -- docs/logs 수명 집행 (plan_26073109 §2.4).

`docs/logs` 는 1초 상시 수집이라 방치하면 무한히 자란다. **디스크가 차면 호스트 자체가
불안정해지므로**, 블랙박스가 스스로 사고 원인이 되는 자가당착을 막는 것이 이 스크립트의 목적이다.

집행 순서(★ 순서가 곧 안전장치다):
    1) rollup   -- 원시에서 **일별 포락선 통계를 먼저 뽑는다**. 압축·삭제보다 먼저여야
                   원시를 버려도 학습 입력이 남는다.
    2) compress -- raw_days 지난 samples 를 압축(zstd 있으면 zstd, 없으면 gzip -- 실제 사용
                   포맷을 events 에 기록한다).
    3) evict    -- compressed_days 지난 압축본 삭제.
    4) capacity -- 노드 총량이 상한을 넘으면 **오래된 samples 부터** 삭제(J2-(i) 사용자 결정).
    events/ 와 rollup/ 은 **영구** -- 희소하고 학습의 실제 입력이므로 수명 대상이 아니다.

음성정직: 모든 삭제·압축은 events 에 `log_evicted`/`log_compressed` 로 남긴다. 조용한 삭제는
"기록이 원래 없었던 것"과 구분되지 않으므로 금지한다.

시각은 --now 주입만 사용한다(벽시계 금지 -- staleness_gate 규약 정합).
종료코드: 0=성공 · 1=인자/입력 오류 · 2=자체시험 실패.
"""
from __future__ import annotations

import argparse
import gzip
import json
import os
import re
import shutil
import subprocess
import sys
from datetime import datetime, timedelta, timezone

SCHEMA_VERSION = 1
DEFAULT_RAW_DAYS = 7
DEFAULT_COMPRESSED_DAYS = 30
DEFAULT_CAP_MIB = 512

_DAY_RE = re.compile(r"^(\d{4}-\d{2}-\d{2})\.csv(\.(gz|zst))?$")


def parse_now(text):
    try:
        return datetime.strptime(text, "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=timezone.utc)
    except ValueError:
        raise SystemExit("logs_lifecycle: --now must be 'YYYY-MM-DDTHH:MM:SSZ', got %r" % text)


def list_sample_files(samples_dir):
    """[(day_str, path, is_compressed)] -- 날짜 오름차순. 규칙 밖 파일은 건드리지 않는다."""
    out = []
    if not os.path.isdir(samples_dir):
        return out
    for name in sorted(os.listdir(samples_dir)):
        m = _DAY_RE.match(name)
        if m:
            out.append((m.group(1), os.path.join(samples_dir, name), bool(m.group(2))))
    return out


def dir_size_bytes(path):
    total = 0
    for root, _dirs, files in os.walk(path):
        for f in files:
            try:
                total += os.path.getsize(os.path.join(root, f))
            except OSError:
                pass
    return total


def _num(text):
    try:
        return float(text)
    except (TypeError, ValueError):
        return None


def _pct(vals, q):
    if not vals:
        return None
    s = sorted(vals)
    i = min(len(s) - 1, max(0, int(round(q * (len(s) - 1)))))
    return round(s[i], 3)


def build_rollup(csv_path, day):
    """원시 CSV -> 일별 포락선 통계. 압축본도 읽는다(재생성 가능하게)."""
    opener = gzip.open if csv_path.endswith(".gz") else open
    if csv_path.endswith(".zst"):
        try:
            raw = subprocess.run(["zstd", "-dc", csv_path], capture_output=True,
                                 timeout=120).stdout.decode("utf-8", "replace")
            lines = raw.splitlines()
        except (OSError, subprocess.SubprocessError):
            return None
    else:
        with opener(csv_path, "rt", encoding="utf-8", errors="replace") as fh:
            lines = fh.read().splitlines()
    if not lines:
        return None
    header = lines[0].split(",")
    idx = {k: i for i, k in enumerate(header)}
    need = ("mem_avail", "mem_rate", "gpu_temp", "gpu_pwr")
    if not all(k in idx for k in need):
        return None

    mem, falling, temp, pwr, n = [], [], [], [], 0
    for line in lines[1:]:
        parts = line.split(",")
        if len(parts) < len(header):
            continue
        n += 1
        v = _num(parts[idx["mem_avail"]])
        if v is not None:
            mem.append(v)
        r = _num(parts[idx["mem_rate"]])
        if r is not None and r > 0:      # 양수 = 하강
            falling.append(r)
        t = _num(parts[idx["gpu_temp"]])
        if t is not None:
            temp.append(t)
        w = _num(parts[idx["gpu_pwr"]])
        if w is not None:
            pwr.append(w)

    return {
        "schema_version": SCHEMA_VERSION, "day": day, "samples": n,
        "mem_avail_mib": {"min": min(mem) if mem else None, "p01": _pct(mem, 0.01),
                          "p50": _pct(mem, 0.50), "max": max(mem) if mem else None},
        "descent_rate_mib_s": {"falling_samples": len(falling), "p50": _pct(falling, 0.50),
                               "p90": _pct(falling, 0.90), "p99": _pct(falling, 0.99),
                               "max": max(falling) if falling else None},
        "gpu_temp_c": {"p50": _pct(temp, 0.50), "max": max(temp) if temp else None},
        "gpu_power_w": {"p50": _pct(pwr, 0.50), "max": max(pwr) if pwr else None},
    }


def append_event(node_dir, now, kind, extra):
    ev_dir = os.path.join(node_dir, "events")
    os.makedirs(ev_dir, exist_ok=True)
    rec = {"ts": now.strftime("%Y-%m-%dT%H:%M:%SZ"), "kind": kind,
           "source": "logs_lifecycle"}
    rec.update(extra)
    with open(os.path.join(ev_dir, now.strftime("%Y-%m") + ".jsonl"), "a",
              encoding="utf-8") as fh:
        fh.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")


def compress_file(path):
    """(new_path, method). zstd 우선, 없으면 gzip(파이썬 stdlib -- 외부 의존 0)."""
    if shutil.which("zstd"):
        new = path + ".zst"
        try:
            subprocess.run(["zstd", "-q", "-19", "--rm", path, "-o", new],
                           check=True, timeout=600)
            return new, "zstd"
        except (OSError, subprocess.SubprocessError):
            if os.path.exists(new):
                os.remove(new)
    new = path + ".gz"
    with open(path, "rb") as src, gzip.open(new, "wb") as dst:
        shutil.copyfileobj(src, dst)
    os.remove(path)
    return new, "gzip"


def enforce(node_dir, now, raw_days=DEFAULT_RAW_DAYS,
            compressed_days=DEFAULT_COMPRESSED_DAYS, cap_mib=DEFAULT_CAP_MIB,
            apply=False):
    samples_dir = os.path.join(node_dir, "samples")
    rollup_dir = os.path.join(node_dir, "rollup")
    actions = {"rolled_up": [], "compressed": [], "evicted_age": [], "evicted_capacity": []}
    today = now.date()

    # ── 1) rollup 먼저 (원시를 버려도 학습 입력이 남게) ──────────────────
    if apply:
        os.makedirs(rollup_dir, exist_ok=True)
    for day, path, _comp in list_sample_files(samples_dir):
        target = os.path.join(rollup_dir, "%s.json" % day)
        if os.path.exists(target):
            continue
        if day == today.isoformat():
            continue                       # 진행 중인 오늘은 아직 확정하지 않는다
        stats = build_rollup(path, day)
        if stats is None:
            continue
        actions["rolled_up"].append(day)
        if apply:
            with open(target, "w", encoding="utf-8") as fh:
                json.dump(stats, fh, ensure_ascii=False, indent=2, sort_keys=True)
                fh.write("\n")

    # ── 2) compress: raw_days 초과 원시 ──────────────────────────────────
    for day, path, is_comp in list_sample_files(samples_dir):
        if is_comp:
            continue
        try:
            age = (today - datetime.strptime(day, "%Y-%m-%d").date()).days
        except ValueError:
            continue
        if age > raw_days:
            actions["compressed"].append({"day": day, "age_days": age})
            if apply:
                _new, method = compress_file(path)
                append_event(node_dir, now, "log_compressed",
                             {"day": day, "age_days": age, "method": method})

    # ── 3) evict: compressed_days 초과 압축본 ────────────────────────────
    for day, path, is_comp in list_sample_files(samples_dir):
        if not is_comp:
            continue
        try:
            age = (today - datetime.strptime(day, "%Y-%m-%d").date()).days
        except ValueError:
            continue
        if age > compressed_days:
            size = os.path.getsize(path) if os.path.exists(path) else 0
            actions["evicted_age"].append({"day": day, "age_days": age, "bytes": size})
            if apply:
                os.remove(path)
                append_event(node_dir, now, "log_evicted",
                             {"day": day, "reason": "age", "age_days": age, "bytes": size})

    # ── 4) capacity: 상한 초과 시 오래된 samples 부터 (J2-(i)) ───────────
    cap_bytes = cap_mib * 1024 * 1024
    total = dir_size_bytes(node_dir)
    if total > cap_bytes:
        for day, path, _c in list_sample_files(samples_dir):   # 날짜 오름차순 = 오래된 것부터
            if total <= cap_bytes:
                break
            if day == today.isoformat():
                continue                   # 오늘 것은 마지막까지 지키다 -- 최근이 가장 중요
            size = os.path.getsize(path) if os.path.exists(path) else 0
            actions["evicted_capacity"].append({"day": day, "bytes": size})
            total -= size
            if apply:
                os.remove(path)
                append_event(node_dir, now, "log_evicted",
                             {"day": day, "reason": "capacity", "bytes": size,
                              "cap_mib": cap_mib})

    actions["total_bytes_after"] = dir_size_bytes(node_dir)
    actions["cap_bytes"] = cap_bytes
    actions["over_cap"] = actions["total_bytes_after"] > cap_bytes
    return actions


def _self_test():
    import tempfile
    checks = []
    now = parse_now("2026-07-31T00:00:00Z")

    def mkday(sdir, day, rows=5, mem0=117000):
        os.makedirs(sdir, exist_ok=True)
        p = os.path.join(sdir, "%s.csv" % day)
        with open(p, "w", encoding="utf-8") as fh:
            fh.write("ts,mem_avail,mem_rate,gpu_temp,gpu_pwr,gpu_sm,gpu_util,gpu_mem,load1,ctr_n\n")
            for i in range(rows):
                fh.write("%d,%d,%d,49,4.8,208,0,,0.3,1\n" % (1785 + i, mem0 - i * 100, 100))
        return p

    with tempfile.TemporaryDirectory() as td:
        nd = os.path.join(td, "spark-x")
        sd = os.path.join(nd, "samples")
        mkday(sd, "2026-07-30")                       # 1일 전 -- 유지
        mkday(sd, "2026-07-20")                       # 11일 전 -- 압축 대상
        mkday(sd, "2026-06-01")                       # 60일 전 -- 압축→나이 삭제 대상
        mkday(sd, "2026-07-31")                       # 오늘 -- rollup 유보

        plan = enforce(nd, now, apply=False)
        checks.append(("dry-run 은 파일을 안 바꾼다",
                       os.path.exists(os.path.join(sd, "2026-07-20.csv"))))
        checks.append(("오늘은 rollup 유보", "2026-07-31" not in plan["rolled_up"]))
        checks.append(("과거일은 rollup 대상", "2026-07-30" in plan["rolled_up"]))
        checks.append(("7일 초과는 압축 대상",
                       any(c["day"] == "2026-07-20" for c in plan["compressed"])))
        checks.append(("1일 전은 압축 아님",
                       not any(c["day"] == "2026-07-30" for c in plan["compressed"])))

        res = enforce(nd, now, apply=True)
        checks.append(("rollup 파일 생성",
                       os.path.exists(os.path.join(nd, "rollup", "2026-07-30.json"))))
        r = json.load(open(os.path.join(nd, "rollup", "2026-07-30.json")))
        checks.append(("rollup 통계 내용", r["samples"] == 5 and r["descent_rate_mib_s"]["max"] == 100.0))
        checks.append(("rollup 이 압축보다 먼저(원시 잃어도 통계 남음)",
                       os.path.exists(os.path.join(nd, "rollup", "2026-07-20.json"))))
        left = {os.path.basename(p) for _d, p, _c in list_sample_files(sd)}
        checks.append(("압축 수행", any(n.startswith("2026-07-20.csv.") for n in left)))
        checks.append(("60일 전은 삭제(같은 실행에서 압축→나이삭제)",
                       not any(n.startswith("2026-06-01") for n in left)))
        ev = open(os.path.join(nd, "events", "2026-07.jsonl"), encoding="utf-8").read()
        checks.append(("압축이 events 에 기록", '"log_compressed"' in ev))
        checks.append(("삭제가 events 에 기록(침묵 삭제 금지)", '"log_evicted"' in ev))

        # 멱등성 -- 재실행이 같은 일을 두 번 하지 않는가
        res2 = enforce(nd, now, apply=True)
        checks.append(("멱등: 재실행 시 압축 0", res2["compressed"] == []))
        checks.append(("멱등: 재실행 시 rollup 0", res2["rolled_up"] == []))

        # 용량 상한 -- 오래된 것부터, 오늘은 보존
        nd2 = os.path.join(td, "spark-y")
        sd2 = os.path.join(nd2, "samples")
        # 상한 단위가 MiB(최소 1)이므로 fixture 총량이 1 MiB 를 확실히 넘어야 시험이 성립한다
        for d in ("2026-07-28", "2026-07-29", "2026-07-30", "2026-07-31"):
            mkday(sd2, d, rows=8000)                          # 일당 약 0.35 MiB × 4 ≈ 1.4 MiB
        before = dir_size_bytes(nd2)
        checks.append(("용량 fixture 가 상한을 초과", before > 1024 * 1024))
        res3 = enforce(nd2, now, cap_mib=1, apply=True)
        checks.append(("용량 초과 시 삭제 발생", len(res3["evicted_capacity"]) > 0))
        checks.append(("가장 오래된 것부터 삭제",
                       res3["evicted_capacity"][0]["day"] == "2026-07-28"))
        checks.append(("오늘 것은 보존",
                       os.path.exists(os.path.join(sd2, "2026-07-31.csv"))))
        checks.append(("상한 이하로 수렴", not res3["over_cap"]))
        ev2 = open(os.path.join(nd2, "events", "2026-07.jsonl"), encoding="utf-8").read()
        checks.append(("용량 삭제도 events 기록", '"capacity"' in ev2))

        # events/rollup 은 수명 대상이 아니다
        checks.append(("events 는 삭제되지 않음",
                       os.path.exists(os.path.join(nd2, "events", "2026-07.jsonl"))))
        checks.append(("rollup 은 삭제되지 않음",
                       os.path.isdir(os.path.join(nd2, "rollup"))))

        # 규칙 밖 파일은 건드리지 않는다
        stray = os.path.join(sd2, "README.txt")
        open(stray, "w").write("x")
        enforce(nd2, now, cap_mib=1, apply=True)
        checks.append(("규칙 밖 파일 불가침", os.path.exists(stray)))

    ok = True
    for name, passed in checks:
        print("  [%s] %s" % ("PASS" if passed else "FAIL", name))
        ok = ok and passed
    print("self-test: %s (%d 케이스)" % ("PASS" if ok else "FAIL", len(checks)))
    return 0 if ok else 2


def main(argv=None):
    ap = argparse.ArgumentParser(description="docs/logs 수명 집행")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    ap.add_argument("--now", help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    ap.add_argument("--raw-days", type=int, default=DEFAULT_RAW_DAYS)
    ap.add_argument("--compressed-days", type=int, default=DEFAULT_COMPRESSED_DAYS)
    ap.add_argument("--cap-mib", type=int, default=DEFAULT_CAP_MIB)
    ap.add_argument("--apply", action="store_true", help="기본은 dry-run")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    missing = [n for n, v in (("--node-dir", args.node_dir), ("--now", args.now)) if not v]
    if missing:
        ap.error("required: %s" % ", ".join(missing))

    res = enforce(args.node_dir, parse_now(args.now), raw_days=args.raw_days,
                  compressed_days=args.compressed_days, cap_mib=args.cap_mib,
                  apply=args.apply)
    mode = "APPLY" if args.apply else "DRY-RUN"
    print("[lifecycle] %s node=%s" % (mode, args.node_dir))
    print("[lifecycle] rollup %d · 압축 %d · 나이삭제 %d · 용량삭제 %d" % (
        len(res["rolled_up"]), len(res["compressed"]),
        len(res["evicted_age"]), len(res["evicted_capacity"])))
    print("[lifecycle] 총량 %.1f MiB / 상한 %.0f MiB%s" % (
        res["total_bytes_after"] / 1048576.0, res["cap_bytes"] / 1048576.0,
        "  ★상한 초과 지속" if res["over_cap"] else ""))
    return 0


if __name__ == "__main__":
    sys.exit(main())
