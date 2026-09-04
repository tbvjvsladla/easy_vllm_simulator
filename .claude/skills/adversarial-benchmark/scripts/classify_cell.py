#!/usr/bin/env python3
# classify_cell.py — 광의의 탐색 셀 종결 3분류 (plan_26090415 §4.6 · CP6)
#
#   serve_failed       서빙 자체가 성립하지 않았다        verdict 없음 · 연속 실패 카운터 ↑
#   measurement_void   서빙은 됐는데 **측정이 파괴**됐다  verdict 없음 · 연속 실패 카운터 ↑
#   measured           측정이 성립했다                    verdict 있음(PASS/REFUTE/서술)
#
# ★ 왜 `measurement_void` 를 따로 두는가. 워치독이 죽인 런을 REFUTE 로 적으면 지도에
#   **"이 레시피는 느리다"** 라는 거짓 진술이 남는다. 진실은 "호스트가 죽였다"이며, 이 작업의
#   유일한 계약(진실된 결과만 보인다)을 정면으로 어긴다. 두 사건은 원인도 처방도 다르다.
#
# ★ `void_reason` 은 자기추론이 아니라 **노드 블랙박스 이벤트와의 시각 대조**로 채운다.
#   대조에 실패하면 `unknown` 이며 추측으로 메우지 않는다(부재와 결측을 가른다).
#   `mode` 를 반드시 본다 — dry-run 트립은 **아무것도 죽이지 않았다**. 그것을 사살로 읽으면
#   관측 전용 인스턴스가 남긴 기록이 거짓 사인(死因)이 된다.
#
# ★ 허용오차 기본값 0 의 근거(U7 해소): 워치독과 러너는 **같은 호스트의 같은 시계**를 쓰므로
#   스큐가 없고, 셀의 [시작,끝] 구간이 이미 로드·측정·정리를 전부 감싼다. 넓힐 이유가 생기면
#   `--tolerance-s` 로 명시한다(조용히 넓히지 않는다).
#
# 사용: classify_cell.py --serve-rc N --measure-rc N --started-utc T --ended-utc T
#         [--events PATH]... [--tolerance-s N] [--json]
# 종료: 0=분류 산출 · 2=인자 오류
import argparse
import datetime as _dt
import json
import sys

OUTCOME_SERVE_FAILED = "serve_failed"
OUTCOME_MEASUREMENT_VOID = "measurement_void"
OUTCOME_MEASURED = "measured"

# 사살을 **실제로 수행한** 이벤트만 사인 후보다. `*_trip` 은 판정이고 `*_kill_ack` 이 집행이며,
# `*_trip_dryrun` 은 관측 전용이라 여기 없다(닫힌 목록 = tripwire).
KILL_EVENT_KINDS = ("watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill")
# 집행 직전의 판정. kill_ack 이 없을 때에만 보조 근거로 쓴다(예: 이벤트가 잘려 나간 경우).
TRIP_EVENT_KINDS = ("watchdog_trip", "thermal_trip")


class ClassifyError(ValueError):
    """분류가 성립하지 않는 입력."""


def _utc(value, field):
    if not isinstance(value, str) or not value.strip():
        raise ClassifyError("%s 는 ISO-8601 UTC 문자열이어야 한다(받은 값: %r)" % (field, value))
    text = value.strip()
    if text.endswith("Z"):
        text = text[:-1] + "+00:00"
    try:
        parsed = _dt.datetime.fromisoformat(text)
    except ValueError as exc:
        raise ClassifyError("%s 를 시각으로 읽을 수 없다(%r): %s" % (field, value, exc)) from exc
    if parsed.tzinfo is None:
        raise ClassifyError("%s 에 타임존이 없다(%r)" % (field, value))
    return parsed.astimezone(_dt.timezone.utc)


def kill_events_in_window(event_lines, started, ended, tolerance_s=0):
    """구간 안의 **집행된** 사살 이벤트. dry-run 은 제외한다(아무것도 죽이지 않았다)."""
    lo = started - _dt.timedelta(seconds=tolerance_s)
    hi = ended + _dt.timedelta(seconds=tolerance_s)
    hits = []
    for line in event_lines:
        line = line.strip()
        if not line:
            continue
        try:
            event = json.loads(line)
        except ValueError:
            continue                      # 손상 줄은 건너뛴다 — 없는 것으로 읽지 않고 아래 참조
        if not isinstance(event, dict):
            continue
        kind = event.get("kind")
        if kind not in KILL_EVENT_KINDS and kind not in TRIP_EVENT_KINDS:
            continue
        if event.get("mode") == "dry-run":
            continue                      # 관측 전용 — 사인이 될 수 없다
        try:
            ts = _utc(event.get("ts"), "event.ts")
        except ClassifyError:
            continue
        if lo <= ts <= hi:
            hits.append({"ts": event.get("ts"), "kind": kind,
                         "source": event.get("source"), "mode": event.get("mode"),
                         "targets": event.get("targets")})
    hits.sort(key=lambda h: h["ts"])
    return hits


def classify(serve_rc, measure_rc, kill_hits, events_scanned):
    """rc 두 개 + 이벤트 대조 → 종결 분류. 순수 함수."""
    if serve_rc != 0:
        return {
            "cell_outcome": OUTCOME_SERVE_FAILED,
            "void_reason": None,
            "void_reason_source": None,
            "kill_events": kill_hits,
            "note": "서빙이 성립하지 않아 측정 단계에 도달하지 않았다(rc=%s)." % serve_rc,
        }

    executed = [h for h in kill_hits if h["kind"] in KILL_EVENT_KINDS]
    if measure_rc != 0:
        if executed:
            first = executed[0]
            return {
                "cell_outcome": OUTCOME_MEASUREMENT_VOID,
                "void_reason": first["kind"],
                "void_reason_source": "events(%s)" % events_scanned,
                "kill_events": kill_hits,
                "note": "서빙은 성립했으나 %s 가 %s 에 집행되어 측정이 파괴됐다."
                        % (first["kind"], first["ts"]),
            }
        return {
            "cell_outcome": OUTCOME_MEASUREMENT_VOID,
            "void_reason": "unknown",
            # 대조를 **했는데** 못 찾은 것과 대조를 안 한 것은 다르다. 후자를 unknown 으로
            # 적으면 나중에 "이벤트를 봤나"를 알 수 없다.
            "void_reason_source": "correlation-miss(%s)" % events_scanned,
            "kill_events": kill_hits,
            "note": "서빙은 성립했으나 측정이 실패했고(rc=%s) 구간 안에 집행된 사살 이벤트가 "
                    "없다 — 사인을 추측하지 않는다." % measure_rc,
        }

    result = {
        "cell_outcome": OUTCOME_MEASURED,
        "void_reason": None,
        "void_reason_source": None,
        "kill_events": kill_hits,
        "note": "측정 성립.",
    }
    if executed:
        # 측정은 성공했는데 같은 구간에 사살이 있었다 — 다른 컨테이너였을 수 있으므로 분류를
        # 바꾸지 않는다. 그러나 **보이게** 남긴다(침묵 금지). 사람이 판단할 사실이다.
        result["note"] = ("측정 성립. 다만 같은 구간에 집행된 사살 이벤트가 %d건 있다 — "
                          "대상이 이 셀이 아니었을 수 있으나 기록으로 남긴다." % len(executed))
    return result


def _self_test():
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    started = _utc("2026-09-04T10:00:00Z", "s")
    ended = _utc("2026-09-04T10:30:00Z", "e")
    ev = lambda kind, ts, mode="armed": json.dumps(
        {"ts": ts, "kind": kind, "source": "mem_watchdog_eta", "mode": mode, "targets": "abc"})

    lines = [ev("watchdog_trip", "2026-09-04T10:10:00Z"),
             ev("watchdog_kill_ack", "2026-09-04T10:10:01Z"),
             ev("watchdog_kill_ack", "2026-09-03T10:10:01Z"),          # 구간 밖
             ev("watchdog_trip_dryrun", "2026-09-04T10:12:00Z", "dry-run"),
             ev("watchdog_kill_ack", "2026-09-04T10:13:00Z", "dry-run"),  # 관측 전용
             "not json at all"]
    hits = kill_events_in_window(lines, started, ended)
    check("K1 구간 밖 이벤트는 제외", all(h["ts"].startswith("2026-09-04") for h in hits))
    check("K2 dry-run 은 사인 후보가 아니다", all(h["mode"] != "dry-run" for h in hits))
    check("K3 손상 줄이 분류를 죽이지 않는다", len(hits) == 2)

    out = classify(3, 0, [], "none")
    check("C1 서빙 실패 → serve_failed",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED and out["void_reason"] is None)

    out = classify(0, 4, hits, "docs/logs/main/events/2026-09.jsonl")
    check("C2 측정 실패 + 집행된 사살 → measurement_void(사인 명시)",
          out["cell_outcome"] == OUTCOME_MEASUREMENT_VOID
          and out["void_reason"] == "watchdog_kill_ack"
          and out["void_reason_source"].startswith("events("))

    out = classify(0, 4, [], "docs/logs/main/events/2026-09.jsonl")
    check("C3 측정 실패 + 대조 실패 → unknown(추측 ✗ · 대조했음이 남는다)",
          out["cell_outcome"] == OUTCOME_MEASUREMENT_VOID
          and out["void_reason"] == "unknown"
          and out["void_reason_source"].startswith("correlation-miss("))

    out = classify(0, 0, [], "none")
    check("C4 정상 → measured", out["cell_outcome"] == OUTCOME_MEASURED)

    out = classify(0, 0, hits, "x")
    check("C5 측정 성공인데 사살이 있었다 → 분류 유지·기록 노출",
          out["cell_outcome"] == OUTCOME_MEASURED and "사살 이벤트가" in out["note"])

    trips_only = kill_events_in_window([ev("watchdog_trip", "2026-09-04T10:10:00Z")], started, ended)
    out = classify(0, 4, trips_only, "x")
    check("C6 트립만 있고 집행이 없으면 사인으로 단정하지 않는다",
          out["void_reason"] == "unknown" and len(out["kill_events"]) == 1)

    if failures:
        sys.stderr.write("[classify_cell --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[classify_cell --self-test] OK — K1~K3 · C1~C6 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="광의의 탐색 셀 종결 3분류(결정론)")
    ap.add_argument("--serve-rc", type=int)
    ap.add_argument("--measure-rc", type=int)
    ap.add_argument("--started-utc")
    ap.add_argument("--ended-utc")
    ap.add_argument("--events", action="append", default=[],
                    help="노드 블랙박스 events JSONL(반복 가능)")
    ap.add_argument("--tolerance-s", type=int, default=0,
                    help="시각 대조 허용오차(기본 0 — 같은 호스트 같은 시계라 스큐가 없다)")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    missing = [n for n, v in (("--serve-rc", args.serve_rc), ("--measure-rc", args.measure_rc),
                              ("--started-utc", args.started_utc), ("--ended-utc", args.ended_utc))
               if v is None]
    if missing:
        sys.stderr.write("[classify_cell] ERROR 필수 인자 부재: %s\n" % ", ".join(missing))
        return 2

    lines = []
    scanned = []
    for path in args.events:
        try:
            with open(path, encoding="utf-8", errors="replace") as handle:
                lines.extend(handle.readlines())
            scanned.append(path)
        except OSError as exc:
            # 이벤트 파일을 못 읽은 것을 "사건이 없었다"로 접지 않는다 — 큰 소리로 남긴다.
            sys.stderr.write("[classify_cell] WARN events 판독 실패(%s): %s\n" % (path, exc))
    try:
        started = _utc(args.started_utc, "--started-utc")
        ended = _utc(args.ended_utc, "--ended-utc")
        hits = kill_events_in_window(lines, started, ended, args.tolerance_s)
    except ClassifyError as exc:
        sys.stderr.write("[classify_cell] ERROR %s\n" % exc)
        return 2

    out = classify(args.serve_rc, args.measure_rc, hits,
                   ",".join(scanned) if scanned else "none")
    out["events_scanned"] = scanned
    out["tolerance_s"] = args.tolerance_s
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
