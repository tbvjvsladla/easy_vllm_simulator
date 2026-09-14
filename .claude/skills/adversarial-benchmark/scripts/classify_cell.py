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
# ★ 2026-09-14(plan_26091407 §4.4 · 사용자 결정 Q3·Q7): **bench_mode 확정**도 여기서 한다.
#   full bench 의 정의는 `lite ∪ GuideLLM × 반복 ≥3` 이고(`repeat_axis.FULL_REPEATS_MIN`), 반복이 성립하지
#   않은 셀은 lite 로 **강등**된다. 강등 트리거는 **기계 이벤트만**이다 — run 실패(measurement_ok=false)와
#   노드 블랙박스 kill 이벤트. 분산(재현 밴드 폭)은 강등 사유가 아니다(기재일 뿐).
#   `sweep_bench.sh` 는 raw `runs[]` 와 스윕이 멈춘 자리(**즉시 신호** `repetition.stop`)만 쓰고, 종료부에서
#   이 파일을 **판정 소유자로 부른다**(bench-mode 모드 · 판정 기록 `bench_mode.json`). 판정은 post-hoc 이며
#   `void_reason` 과 같은 모양이다 — 사인은 자기추론이 아니라 멈춘 자리의 시각 창과 블랙박스 이벤트의
#   **시각 대조**가 답한다. 규칙(2026-09-14 리뷰 정정 — 반복 조건의 범위는 `repeat_axis.py` 헤더 ★):
#     · 멈춘 자리(repeat-break · clamp)의 창 안에 **집행된** 사살(KILL_EVENT_KINDS) → lite · `blackbox_kill`
#       (레벨·run 순번과 무관 — kill 이벤트는 기계 이벤트 트리거다)
#     · 그 밖에 판정점 반복이 섰다 → full (경계 레벨의 반복 중단·첫 run 실패는 적응 상한 클램프)
#     · 판정점 반복이 끊겼다 → lite · `run_failed`(트립 단독·시각 불일치·이벤트 미관측 포함 — 관측된 신호 그대로)
#   대조를 했는지·못 했는지는 사유 문자열이 아니라 구조 필드 `downgrade_correlation` 이 든다.
#   ⚠ E2E 정상 경로(판정점 반복 완주 · 사살 없음)는 이 강등 경로를 밟지 않는다. 포화 경계에서 스윕이 멈추는
#     것은 정상 경로다(클램프). 강등 경로는 실패주입 자체검사(`selftest_sweep_repeats.py`)가 지킨다.
#
# 사용: classify_cell.py --serve-rc N --measure-rc N --started-utc T --ended-utc T
#         [--events PATH]... [--events-from-repo REPO] [--tolerance-s N] [--json]            (셀 종결 모드)
#       classify_cell.py --sweep-index PATH [--events PATH]... [--events-from-repo REPO]
#         [--tolerance-s N] [--write-bench-mode]                                              (bench-mode 모드)
#   두 모드는 섞지 않는다 — 셀 인자와 --sweep-index 를 함께 주면 exit 2.
# 종료: 0=분류 산출 · 2=인자 오류
import argparse
import datetime as _dt
import glob
import json
import os
import sys
import tempfile

OUTCOME_SERVE_FAILED = "serve_failed"
OUTCOME_MEASUREMENT_VOID = "measurement_void"
OUTCOME_MEASURED = "measured"
# 2026-09-05(G-B13): "측정하지 않았다" 는 "측정에 성공했다" 와 **다른 사실**이다. 종전에는
#   broad_search 가 `MEASURE_RC="${MEASURE_RC:-0}"` 로 둘을 같은 0 에 접었고, serve 가 성립한
#   재조립 경로에서 아무것도 재지 않고도 셀이 `measured` 로 종결될 수 있었다.
OUTCOME_NOT_MEASURED = "not_measured"

# 사살을 **실제로 수행한** 이벤트만 사인 후보다. `*_trip` 은 판정이고 `*_kill_ack` 이 집행이며,
# `*_trip_dryrun` 은 관측 전용이라 여기 없다(닫힌 목록 = tripwire).
KILL_EVENT_KINDS = ("watchdog_kill_ack", "thermal_kill_ack", "earlyoom_kill")
# 집행 직전의 판정. kill_ack 이 없을 때에만 보조 근거로 쓴다(예: 이벤트가 잘려 나간 경우).
TRIP_EVENT_KINDS = ("watchdog_trip", "thermal_trip")

# ── bench_mode 어휘 (2026-09-14 · plan_26091407 §4.4) — 닫힌 목록 · 소유는 이 파일 ─────────────────
#   소비자(render_report · render_sweep_map · 단계 ⑤ lite hint 통로)는 import 하거나 이 토큰을 읽는다.
#   ⚠ 이름을 `OUTCOME_` 로 시작하지 않는다 — campaign_init 자체검사가 그 접두사를 셀 결과 어휘로 긁는다.
BENCH_MODE_FULL = "full"
BENCH_MODE_LITE = "lite"
BENCH_MODES = (BENCH_MODE_FULL, BENCH_MODE_LITE)
DOWNGRADE_RUN_FAILED = "run_failed"
DOWNGRADE_BLACKBOX_KILL = "blackbox_kill"
DOWNGRADE_REASONS = (DOWNGRADE_RUN_FAILED, DOWNGRADE_BLACKBOX_KILL)
# **선언된 lite-only**(강등 아님)와 **강등된 lite** 를 가르는 규칙(단계 ⑤ 가 결정론으로 읽는다):
#   bench_mode=lite ∧ downgrade_reason ∈ DOWNGRADE_REASONS            → 강등된 lite(full 을 시도했다)
#   bench_mode=lite ∧ downgrade_reason=null ∧ source 가 "declared(" 로 시작 → 선언된 lite-only
#   그 밖의 lite ∧ null                                              → 판정 불가(규칙 밖 기록 — 추측 ✗)
# 이 파일이 내는 lite 는 **언제나 강등**이다(사유 필수). 선언된 lite-only 기록은 lite 통로(단계 ⑤)가
# 같은 모양(`bench_mode_source="declared(...)"`)으로 적는다.
BENCH_MODE_SOURCE_DECLARED_PREFIX = "declared("
BENCH_MODE_RECORD_NAME = "bench_mode.json"   # 스윕 디렉터리 사이드카 — 판정 기록의 지속 자리
# 사살 대조를 했는가(구조 필드 · 단계 ⑤ 가 "보고도 없었다" 와 "보지 않았다" 를 문자열 파싱 없이 가른다):
#   matched=창 안 집행 사살 있음 · miss=이벤트를 봤고 창 안 집행 사살 없음 · not_scanned=판독한 이벤트 파일 0
#   unavailable=창 시각을 읽지 못했다 · not_applicable=스윕이 멈춘 자리가 없다(대조할 창이 없다)
CORRELATION_MATCHED = "matched"
CORRELATION_MISS = "miss"
CORRELATION_NOT_SCANNED = "not_scanned"
CORRELATION_UNAVAILABLE = "unavailable"
CORRELATION_NOT_APPLICABLE = "not_applicable"
DOWNGRADE_CORRELATIONS = (CORRELATION_MATCHED, CORRELATION_MISS, CORRELATION_NOT_SCANNED,
                          CORRELATION_UNAVAILABLE, CORRELATION_NOT_APPLICABLE)
# 노드 블랙박스 events 의 자리(docs.md §기계판독 데이터 평면 `docs/logs/<node_id>/events/<YYYY-MM>.jsonl`).
# 발견 규칙을 호출부(broad_search · sweep_bench)마다 다시 적지 않게 여기 한 곳에 둔다.
EVENTS_REPO_GLOB = os.path.join("docs", "logs", "*", "events", "*.jsonl")


def events_from_repo(repo):
    """저장소의 블랙박스 events 파일 목록(정렬). 없으면 빈 목록 — 부재는 호출부가 not_scanned 로 남긴다."""
    return sorted(path for path in glob.glob(os.path.join(repo, EVENTS_REPO_GLOB)) if os.path.isfile(path))


def bench_mode_record_path(sweep_index_path):
    """판정 기록의 관례 자리 — sweep_index 옆 사이드카."""
    return os.path.join(os.path.dirname(os.path.abspath(sweep_index_path)), BENCH_MODE_RECORD_NAME)


def read_bench_mode_record(sweep_index_path, index_doc, record_path=None):
    """판정 기록 판독(소비자 공용 — render_report · publish_benchmark_record · broad_search).

    → (record | None, status). status: `ok` · `absent(…)` · `unreadable(…)` · `stale(…)`.
    stale = 기록의 `sweep_index_generated_utc` 가 지금 index 의 측정시각과 다르다(같은 스윕 디렉터리에 나중 측정이
    덮였다) — 낡은 분류를 싣지 않는다. 부재·판독 실패·낡음을 full 로도 lite 로도 접지 않는다."""
    path = record_path or bench_mode_record_path(sweep_index_path)
    name = os.path.basename(path)
    if not os.path.isfile(path):
        return None, "absent(%s 부재 — 이 스윕을 분류한 기록이 없다)" % name
    try:
        with open(path, encoding="utf-8") as handle:
            record = json.load(handle)
    except (OSError, ValueError) as exc:
        return None, "unreadable(%s 판독 실패: %s)" % (name, exc)
    if not isinstance(record, dict):
        return None, "unreadable(%s 가 객체가 아니다)" % name
    index_utc = index_doc.get("generated_utc") if isinstance(index_doc, dict) else None
    if record.get("sweep_index_generated_utc") != index_utc:
        return None, ("stale(%s 는 다른 측정(generated_utc=%s)의 분류다 · 이 스윕 %s)"
                      % (name, record.get("sweep_index_generated_utc"), index_utc))
    return record, "ok"


def write_bench_mode_record(sweep_index_path, record):
    """판정 기록을 **원자적으로** 쓴다(임시 파일 → os.replace). 반쯤 쓰인 기록을 소비자가 읽지 않게 한다."""
    out = bench_mode_record_path(sweep_index_path)
    fd, tmp = tempfile.mkstemp(prefix=".%s." % BENCH_MODE_RECORD_NAME, dir=os.path.dirname(out))
    try:
        with os.fdopen(fd, "w", encoding="utf-8") as handle:
            json.dump(record, handle, ensure_ascii=False, indent=2)
            handle.write("\n")
        os.replace(tmp, out)
    except BaseException:
        if os.path.exists(tmp):
            os.unlink(tmp)
        raise
    return out


def bench_mode_kind(record):
    """기록 → 'full' | 'declared-lite' | 'downgraded-lite' | 'undetermined'. 순수 함수(단계 ⑤ 판독 규칙)."""
    if not isinstance(record, dict):
        return "undetermined"
    mode, reason = record.get("bench_mode"), record.get("downgrade_reason")
    source = record.get("bench_mode_source") or ""
    if mode == BENCH_MODE_FULL and reason is None:
        return "full"
    if mode == BENCH_MODE_LITE and reason in DOWNGRADE_REASONS:
        return "downgraded-lite"
    if (mode == BENCH_MODE_LITE and reason is None
            and isinstance(source, str) and source.startswith(BENCH_MODE_SOURCE_DECLARED_PREFIX)):
        return "declared-lite"
    return "undetermined"


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
    """rc 두 개 + 이벤트 대조 → 종결 분류. 순수 함수.

    `measure_rc is None` = **측정 단계에 들어가지 않았다**(부재). 성공(0)과 구분한다.
    """
    executed = [h for h in kill_hits if h["kind"] in KILL_EVENT_KINDS]
    if serve_rc != 0:
        # ★ 2026-09-11(plan_26091108 R4): 이 분기가 **사살을 무시했다**. 종전에는 구간 안에
        #   집행된 사살이 있어도 `void_reason: None` 을 내고 사유 칸을 비웠고, 그 빈자리를
        #   사람의 산문("fused_moe FP8 config 부재 추정 hang")이 메웠다 — camp-26090918 의
        #   void 3건이 그렇게 기록됐고, 그 서술이 근거 없는 체크포인트 재다운로드로 이어졌다.
        #   사인은 추정이 아니라 **시각 대조**가 답한다. 분류(serve_failed)는 바꾸지 않는다 —
        #   서빙은 실제로 성립하지 않았다. 바뀌는 것은 **왜** 다.
        if executed:
            first = executed[0]
            return {
                "cell_outcome": OUTCOME_SERVE_FAILED,
                "void_reason": first["kind"],
                "void_reason_source": "events(%s)" % events_scanned,
                "kill_events": kill_hits,
                "note": "서빙이 성립하지 않았다(rc=%s). 구간 안에서 %s 가 %s 에 **집행**됐다 — "
                        "이 죽음은 모델·빌드 평면의 구동불가가 아니라 호스트 평면의 사살이다."
                        % (serve_rc, first["kind"], first["ts"]),
            }
        return {
            "cell_outcome": OUTCOME_SERVE_FAILED,
            "void_reason": None,
            "void_reason_source": "correlation-miss(%s)" % events_scanned,
            "kill_events": kill_hits,
            "note": "서빙이 성립하지 않아 측정 단계에 도달하지 않았다(rc=%s). 구간 안에 집행된 "
                    "사살 이벤트는 없다 — 사인을 추측하지 않는다." % serve_rc,
        }

    if measure_rc is None:
        return {
            "cell_outcome": OUTCOME_NOT_MEASURED,
            "void_reason": None,
            "void_reason_source": None,
            "kill_events": kill_hits,
            "note": "서빙은 성립했으나 측정 단계에 들어가지 않았다(rc 부재) — 성공으로 집계하지 않는다.",
        }
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


def _bench_mode_record(mode, source, reason=None, reason_source=None, repetition=None, kill_hits=None,
                       correlation=CORRELATION_NOT_APPLICABLE):
    return {"bench_mode": mode, "bench_mode_source": source,
            "downgrade_reason": reason, "downgrade_reason_source": reason_source,
            "downgrade_correlation": correlation,
            "repetition": repetition, "repetition_kill_events": kill_hits or []}


def classify_bench_mode(index, event_lines, events_scanned, tolerance_s=0):
    """sweep_index(대표 run 의 runs[] · repetition.clamp_run 포함) + 블랙박스 이벤트 → bench_mode 확정.
    순수 함수(파일 I/O 없음). `events_scanned` = 판독한 events 파일 목록(또는 쉼표 문자열 · "none"/빈 값 = 0개).

    판정 순서(post-hoc):
      ① 반복 요약은 `repeat_axis.summarize` 가 raw `levels[].measured.runs` 에서 **다시 계산**한다(index 의 요약을
         믿지 않는다 · 클램프 경계 사실만 index `repetition.clamp_run` 에서 받는다 — 그 레벨은 levels 에 없다)
      ② 반복 조건(`repeat_axis.repetition_established` · 판정점 범위)이 판정 불가면 판정하지 않는다(null)
      ③ 스윕이 멈춘 자리가 있으면 그 창과 이벤트를 대조한다 — 집행된 사살이 있으면 lite · blackbox_kill
      ④ 그 밖: 반복 조건 성립 → full · 불성립 → lite · run_failed(대조 결과는 downgrade_correlation 에)
    """
    try:
        sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
        import repeat_axis as _ra
    except ImportError as exc:   # 배선 결함 — 부재를 full/lite 로 접지 않는다
        return _bench_mode_record(None, "undeterminable(repeat_axis 적재 실패: %s)" % exc)
    if not isinstance(index, dict) or not isinstance(index.get("levels"), list) or not index["levels"]:
        return _bench_mode_record(None, "not_evaluated(sweep_index levels 부재 — 측정 레벨이 없다)")
    declared = index.get("repetition") if isinstance(index.get("repetition"), dict) else {}
    summ = _ra.summarize(index["levels"], requested=declared.get("requested"),
                         requested_source=declared.get("requested_source"), kind=declared.get("kind"),
                         clamp_run=declared.get("clamp_run"))
    established, why = _ra.repetition_established(summ, index.get("verdict_point_level"))
    if established is None:
        return _bench_mode_record(None, why, repetition=summ)

    if isinstance(events_scanned, (list, tuple)):
        scanned_txt = ",".join(events_scanned) if events_scanned else "none"
    else:
        scanned_txt = events_scanned or "none"
    stop = summ.get("stop")
    hits, correlation, window, signal = [], CORRELATION_NOT_APPLICABLE, None, None
    if isinstance(stop, dict):
        signal = "stop[%s · level=%s · run=%s](%s)" % (stop.get("kind"), stop.get("level"), stop.get("run"),
                                                      stop.get("detail"))
        window = "[%s, %s] tol=%ss" % (stop.get("window_start_utc"), stop.get("window_end_utc"), tolerance_s)
        try:
            lo = _utc(stop.get("window_start_utc"), "stop.window_start_utc")
            hi = _utc(stop.get("window_end_utc"), "stop.window_end_utc")
        except ClassifyError as exc:
            correlation, window = CORRELATION_UNAVAILABLE, "창 시각 판독 불가: %s" % exc
        else:
            hits = kill_events_in_window(event_lines, lo, hi, tolerance_s)
            executed = [h for h in hits if h["kind"] in KILL_EVENT_KINDS]
            if executed:
                correlation = CORRELATION_MATCHED
            elif scanned_txt == "none":
                correlation = CORRELATION_NOT_SCANNED
            else:
                correlation = CORRELATION_MISS
    executed = [h for h in hits if h["kind"] in KILL_EVENT_KINDS]
    if executed:
        first = executed[0]
        return _bench_mode_record(
            BENCH_MODE_LITE,
            "%s · 멈춘 자리 창 안에 집행된 사살 — kill 이벤트는 레벨·run 순번과 무관한 강등 트리거" % why,
            DOWNGRADE_BLACKBOX_KILL,
            "events(%s) · %s @ %s ∈ %s · %s" % (scanned_txt, first["kind"], first["ts"], window, signal),
            repetition=summ, kill_hits=hits, correlation=correlation)
    note = ""
    if correlation == CORRELATION_MISS:
        trips = len(hits)
        note = "correlation-miss(%s · 창 %s%s)" % (scanned_txt, window,
                                                  (" · 트립 %d건은 집행이 아니라 사인 불충분" % trips) if trips else "")
    elif correlation == CORRELATION_NOT_SCANNED:
        note = "not-scanned(판독한 블랙박스 events 파일 0 — 사살 여부를 보지 못했다 · 창 %s)" % window
    elif correlation == CORRELATION_UNAVAILABLE:
        note = "correlation-unavailable(%s)" % window
    if established:
        return _bench_mode_record(BENCH_MODE_FULL, "%s%s" % (why, (" · " + note) if note else ""),
                                  repetition=summ, kill_hits=hits, correlation=correlation)
    return _bench_mode_record(BENCH_MODE_LITE, why, DOWNGRADE_RUN_FAILED, "%s · %s" % (signal, note),
                              repetition=summ, kill_hits=hits, correlation=correlation)


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

    # ── R4(2026-09-11): serve 실패에서도 **사인은 시각 대조가 답한다** ──────────────────
    out = classify(3, None, hits, "docs/logs/main/events/2026-09.jsonl")
    check("★C7 서빙 실패 + 집행된 사살 → 분류는 serve_failed, 사인은 이벤트에서 온다",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED
          and out["void_reason"] == "watchdog_kill_ack"
          and out["void_reason_source"].startswith("events(")
          and "호스트 평면의 사살" in out["note"])
    out = classify(3, None, [], "docs/logs/main/events/2026-09.jsonl")
    check("★C8 서빙 실패 + 대조 실패 → 사인 없음이되 **대조했음**이 남는다(추측 ✗)",
          out["cell_outcome"] == OUTCOME_SERVE_FAILED
          and out["void_reason"] is None
          and out["void_reason_source"].startswith("correlation-miss("))
    out = classify(3, None, [h for h in hits if h["kind"] in TRIP_EVENT_KINDS],
                   "docs/logs/main/events/2026-09.jsonl")
    check("★C9 음성대조: 트립만으로 서빙 실패를 사살이라 단정하지 않는다(집행만 사인이다)",
          out["void_reason"] is None)

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

    # ── D: bench_mode 확정(2026-09-14 · plan_26091407 §4.4 · §7 O4 · 리뷰 정정: 판정점 범위·클램프 사살) ──────
    #   run k 는 base 분 + k-1 분에 [:00, :50]. 판정점(level 1) 실패주입은 run 2(=창 [10:01:00, 10:02:50]).
    def run(k, ok=True, tps=30.0, base=1):
        return {"run": k, "started_utc": "2026-09-04T10:%02d:00Z" % (k - 1 + base),
                "ended_utc": "2026-09-04T10:%02d:50Z" % (k - 1 + base),
                "run_bench_rc": 0, "parse_rc": 0, "measurement_ok": ok, "decode_tps": tps}

    def level(lv, runs):
        return {"level": lv, "status": "ok", "measured": {"measurement_ok": True, "runs": runs}}

    def index(*levels, requested=3, clamp_run=None, verdict_point=1):
        rep = {"requested": requested, "requested_source": "declared(fixture)", "kind": "warm-rerun"}
        if clamp_run is not None:
            rep["clamp_run"] = clamp_run
        doc = {"levels": list(levels), "repetition": rep}
        if verdict_point is not None:
            doc["verdict_point_level"] = verdict_point
        return doc
    evf = ["docs/logs/main/events/2026-09.jsonl"]
    ok3 = [run(1), run(2, tps=31.0), run(3, tps=29.0)]
    broken = [run(1), run(2, ok=False)]

    out = classify_bench_mode(index(level(1, ok3)), [], "none")
    check("D1 레벨 전부 반복 3 완주 → full · 사유 null · 대조 대상 없음(not_applicable)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_reason"] is None
          and out["downgrade_correlation"] == CORRELATION_NOT_APPLICABLE and bench_mode_kind(out) == "full", out)

    kill_in = [ev("watchdog_kill_ack", "2026-09-04T10:02:10Z")]
    out = classify_bench_mode(index(level(1, broken)), kill_in, evf)
    check("★D2 실패주입: 판정점 run 2 무너짐 ∧ 끊긴 창 안 집행 사살 → lite · blackbox_kill · matched",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL
          and out["downgrade_reason_source"].startswith("events(") and bench_mode_kind(out) == "downgraded-lite"
          and out["downgrade_correlation"] == CORRELATION_MATCHED, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("thermal_trip", "2026-09-04T10:02:10Z")], evf)
    check("★D3 음성대조: 같은 창에 트립만 → blackbox_kill 로 단정 ✗ · run_failed · miss · 트립 사인 불충분 기재",
          out["downgrade_reason"] == DOWNGRADE_RUN_FAILED and "사인 불충분" in out["downgrade_reason_source"]
          and out["downgrade_correlation"] == CORRELATION_MISS, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:05:00Z")], evf)
    check("★D4 음성대조: 사살 시각이 창 밖(불일치 · tolerance 0) → run_failed · miss",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_RUN_FAILED
          and not out["repetition_kill_events"] and out["downgrade_correlation"] == CORRELATION_MISS, out)
    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:05:00Z")], evf,
                              tolerance_s=200)
    check("D4b 허용오차는 **명시**로만 넓어진다(--tolerance-s 200 이면 같은 사살이 창에 든다)",
          out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL, out)

    out = classify_bench_mode(index(level(1, broken)), [ev("watchdog_kill_ack", "2026-09-04T10:02:10Z", "dry-run")], evf)
    check("★D5 dry-run 사살 기록은 사인이 아니다 → run_failed", out["downgrade_reason"] == DOWNGRADE_RUN_FAILED, out)

    wide = [run(1, tps=10.0), run(2, tps=30.0), run(3, tps=50.0)]
    out = classify_bench_mode(index(level(1, wide)), [], "none")
    check("★D6 음성대조: 분산만 크다(10/30/50) → full · 강등 사유 ✗(밴드는 기재일 뿐)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_reason"] is None, out)

    out = classify_bench_mode(index(level(1, ok3)), kill_in, evf)
    check("D7 스윕이 멈추지 않았는데 같은 시간대 사살 → full(멈춘 자리가 없으면 반복을 끊지 않았다 · C5 와 동형)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_NOT_APPLICABLE, out)

    out = classify_bench_mode({"levels": [{"level": 1, "measured": {"measurement_ok": True}}],
                               "verdict_point_level": 1}, [], "none")
    check("D8 runs[] 없는 산출물(반복 축 이전) → 판정하지 않는다(null · not_evaluated)",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("not_evaluated("), out)

    five = [run(1), run(2), run(3), run(4), run(5, ok=False)]
    out = classify_bench_mode(index(level(1, five), requested=5), [], evf)
    check("D9 요청 5 · 판정점 run 5 에서 끊김 → 완주 4 ≥ 3 이라 full(중단은 출처에 기재)",
          out["bench_mode"] == BENCH_MODE_FULL and "판정점 level 1 run 5" in out["bench_mode_source"], out)

    l2_broken = [run(1, base=5), run(2, ok=False, base=5)]
    out = classify_bench_mode(index(level(1, ok3), level(2, l2_broken)), [], evf)
    check("★D10 비대칭 교정: 경계 레벨 2 의 run 2 실패(판정점 3) → full(적응 상한 클램프 · 경계 첫 run 실패 D13 과 같은 판정)",
          out["bench_mode"] == BENCH_MODE_FULL and "적응 상한 클램프" in out["bench_mode_source"]
          and out["downgrade_correlation"] == CORRELATION_MISS, out)
    out = classify_bench_mode(index(level(1, ok3), level(2, l2_broken)),
                              [ev("thermal_kill_ack", "2026-09-04T10:06:20Z")], evf)
    check("★D10b 같은 경계 창(레벨 2 run 1 시작~run 2 끝) 안 집행 사살 → lite · blackbox_kill(판정점 반복이 섰어도)",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL, out)

    out = classify_bench_mode(index(level(1, [run(1), run(2)]), requested=2), [], "none")
    check("D11 요청 자체가 정의 미만(끊김 없음) → 판정 불가(강등 사유 발명 ✗)",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("unclassifiable("), out)

    check("D12 판독 규칙: 선언된 lite-only 와 강등된 lite 는 사유·출처로 갈린다",
          bench_mode_kind({"bench_mode": "lite", "downgrade_reason": None,
                           "bench_mode_source": "declared(lite_bench · lite-only 셀)"}) == "declared-lite"
          and bench_mode_kind({"bench_mode": "lite", "downgrade_reason": None,
                               "bench_mode_source": "runs[]"}) == "undetermined"
          and bench_mode_kind({"bench_mode": "lite", "downgrade_reason": "variance"}) == "undetermined"
          and bench_mode_kind(None) == "undetermined")

    clamp = {"level": 2, "run": 1, "started_utc": "2026-09-04T10:05:00Z", "ended_utc": "2026-09-04T10:05:30Z",
             "run_bench_rc": 3, "parse_rc": 0}
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp), [], evf)
    check("D13 경계 레벨 2 첫 run 실패(클램프) · 사살 없음 → full · 대조는 miss 로 남는다",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_MISS
          and out["repetition"]["stop"]["kind"] == "clamp", out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp),
                              [ev("watchdog_kill_ack", "2026-09-04T10:05:10Z")], evf)
    check("★D14 클램프 레벨 첫 run 창 안 집행 사살 → lite · blackbox_kill(가장 흔한 실사살 형태 · 절삭으로 삼키지 않는다)",
          out["bench_mode"] == BENCH_MODE_LITE and out["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL
          and bench_mode_kind(out) == "downgraded-lite", out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp),
                              [ev("thermal_trip", "2026-09-04T10:05:10Z")], evf)
    check("★D15 음성대조: 같은 클램프 창에 트립 단독 → full 유지(사인 불충분)",
          out["bench_mode"] == BENCH_MODE_FULL and "사인 불충분" in out["bench_mode_source"], out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run=clamp), [], "none")
    check("D16 이벤트 파일 0(블랙박스 미설치) → 판정은 runs 로 full · 대조는 not_scanned(보지 못했음이 구조로 남는다)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_NOT_SCANNED, out)
    out = classify_bench_mode(index(level(1, broken)), [], [])
    check("D16b 판정점 끊김 ∧ 이벤트 파일 0 → run_failed · not_scanned",
          out["downgrade_reason"] == DOWNGRADE_RUN_FAILED
          and out["downgrade_correlation"] == CORRELATION_NOT_SCANNED, out)
    out = classify_bench_mode(index(level(1, ok3), clamp_run={"level": 2, "run": 1, "unreadable": "부재"}),
                              kill_in, evf)
    check("D17 클램프 경계 사실 판독 불가 → 창 대조 unavailable(이름으로 남긴다 · 사살을 발명하지 않는다)",
          out["bench_mode"] == BENCH_MODE_FULL and out["downgrade_correlation"] == CORRELATION_UNAVAILABLE, out)
    out = classify_bench_mode(index(level(1, ok3), level(2, [run(1, base=5), run(2, base=5)]),
                                    level(4, [run(1, base=9), run(2, base=9), run(3, base=9)])), [], evf)
    check("★D18 경계가 아닌 레벨이 정의 미만(스윕은 멈추지 않았다) → 판정 불가",
          out["bench_mode"] is None and out["bench_mode_source"].startswith("unclassifiable("), out)
    out = classify_bench_mode(index(level(1, ok3), verdict_point=None), [], evf)
    check("★D19 판정점 레벨 부재(verdict_point_level 없음) → 판정 불가(1 을 가정하지 않는다)",
          out["bench_mode"] is None and "verdict_point_level" in out["bench_mode_source"], out)

    # ── E: 판정 기록 자리 · CLI (원자적 쓰기 · 신선도 · 모드 분리 · --tolerance-s 전달) ────────────────────
    import contextlib
    import io
    import tempfile as _tf
    with _tf.TemporaryDirectory(prefix="classify-cell.") as td:
        idx_path = os.path.join(td, "sweep_index.json")
        doc = dict(index(level(1, broken)), generated_utc="2026-09-04T11:00:00Z")
        with open(idx_path, "w", encoding="utf-8") as handle:
            json.dump(doc, handle)
        rec, status = read_bench_mode_record(idx_path, doc)
        check("E1 기록 부재 → (None, absent)", rec is None and status.startswith("absent("), status)
        events_path = os.path.join(td, "events.jsonl")
        with open(events_path, "w", encoding="utf-8") as handle:
            handle.write(ev("watchdog_kill_ack", "2026-09-04T10:05:00Z") + "\n")
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--sweep-index", idx_path, "--events", events_path, "--write-bench-mode"])
        rec, status = read_bench_mode_record(idx_path, doc)
        check("E2 CLI bench-mode 모드 → 기록이 index 측정시각에 묶여 쓰인다 · tolerance 0 이면 창 밖 사살은 run_failed",
              rc == 0 and status == "ok" and rec["downgrade_reason"] == DOWNGRADE_RUN_FAILED
              and not [n for n in os.listdir(td) if n.startswith("." + BENCH_MODE_RECORD_NAME)], (rc, status, rec))
        buf = io.StringIO()
        with contextlib.redirect_stdout(buf):
            rc = main(["--sweep-index", idx_path, "--events", events_path, "--tolerance-s", "200",
                       "--write-bench-mode"])
        rec, status = read_bench_mode_record(idx_path, doc)
        check("★E3 CLI 가 --tolerance-s 를 판정에 **전달**한다(같은 사살이 창에 든다 → blackbox_kill)",
              rc == 0 and rec["downgrade_reason"] == DOWNGRADE_BLACKBOX_KILL and rec["tolerance_s"] == 200, rec)
        rec, status = read_bench_mode_record(idx_path, dict(doc, generated_utc="2026-09-04T12:00:00Z"))
        check("★E4 다른 측정의 기록 → (None, stale)", rec is None and status.startswith("stale("), status)
        with open(bench_mode_record_path(idx_path), "w", encoding="utf-8") as handle:
            handle.write('{"bench_mode": "fu')
        rec, status = read_bench_mode_record(idx_path, doc)
        check("★E5 반쯤 쓰인 기록 → (None, unreadable) · full/lite 로 접지 않는다",
              rec is None and status.startswith("unreadable("), status)
        with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
            rc_mix = main(["--sweep-index", idx_path, "--serve-rc", "0", "--measure-rc", "0",
                           "--started-utc", "2026-09-04T10:00:00Z", "--ended-utc", "2026-09-04T11:00:00Z"])
            rc_nowrite = main(["--write-bench-mode", "--serve-rc", "0", "--measure-rc", "0",
                               "--started-utc", "2026-09-04T10:00:00Z", "--ended-utc", "2026-09-04T11:00:00Z"])
        check("★E6 두 모드를 섞으면 exit 2 · --write-bench-mode 는 --sweep-index 없이 exit 2", rc_mix == 2 and rc_nowrite == 2,
              (rc_mix, rc_nowrite))
        os.makedirs(os.path.join(td, "repo", "docs", "logs", "n1", "events"))
        found = os.path.join(td, "repo", "docs", "logs", "n1", "events", "2026-09.jsonl")
        with open(found, "w", encoding="utf-8") as handle:
            handle.write("")
        check("E7 --events-from-repo 발견 규칙 = docs/logs/<node>/events/*.jsonl",
              events_from_repo(os.path.join(td, "repo")) == [found] and events_from_repo(os.path.join(td, "none")) == [])

    if failures:
        sys.stderr.write("[classify_cell --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[classify_cell --self-test] OK — K1~K3 · C1~C9 · D1~D19(bench_mode 확정 · 실패주입 · 음성대조) · E1~E7 전부 통과")
    return 0


def _read_events(paths):
    lines, scanned = [], []
    for path in paths:
        try:
            with open(path, encoding="utf-8", errors="replace") as handle:
                lines.extend(handle.readlines())
            scanned.append(path)
        except OSError as exc:
            # 이벤트 파일을 못 읽은 것을 "사건이 없었다"로 접지 않는다 — 큰 소리로 남긴다.
            sys.stderr.write("[classify_cell] WARN events 판독 실패(%s): %s\n" % (path, exc))
    return lines, scanned


def main(argv=None):
    ap = argparse.ArgumentParser(description="광의의 탐색 셀 종결 3분류 · full bench bench_mode 확정(결정론)")
    ap.add_argument("--serve-rc", type=int)
    ap.add_argument("--measure-rc", default=None,
                    help="측정 종료코드. **측정하지 않았으면 `absent`** 를 넘겨라(0 과 다른 사실이다).")
    ap.add_argument("--started-utc")
    ap.add_argument("--ended-utc")
    ap.add_argument("--events", action="append", default=[],
                    help="노드 블랙박스 events JSONL(반복 가능)")
    ap.add_argument("--events-from-repo",
                    help="저장소의 블랙박스 events 를 관례 자리(%s)에서 찾아 --events 에 더한다" % EVENTS_REPO_GLOB)
    ap.add_argument("--tolerance-s", type=int, default=0,
                    help="시각 대조 허용오차(기본 0 — 같은 호스트 같은 시계라 스큐가 없다)")
    ap.add_argument("--sweep-index",
                    help="bench-mode 모드: sweep_bench 산출 sweep_index.json 의 bench_mode(full|lite)·downgrade_reason 을 "
                         "확정한다(셀 종결 인자와 함께 쓰지 않는다)")
    ap.add_argument("--write-bench-mode", action="store_true",
                    help="bench_mode 판정 기록을 sweep_index 옆 `%s` 에 원자적으로 쓴다(자리 이름의 소유는 이 파일 · "
                         "--sweep-index 필요)" % BENCH_MODE_RECORD_NAME)
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    event_paths = list(args.events)
    if args.events_from_repo:
        event_paths += [p for p in events_from_repo(args.events_from_repo) if p not in event_paths]
    cell_args = [n for n, v in (("--serve-rc", args.serve_rc), ("--measure-rc", args.measure_rc),
                                ("--started-utc", args.started_utc), ("--ended-utc", args.ended_utc))
                 if v is not None]
    if args.write_bench_mode and not args.sweep_index:
        sys.stderr.write("[classify_cell] ERROR --write-bench-mode 는 --sweep-index 가 필요하다\n")
        return 2

    # ── bench-mode 모드(post-hoc · sweep_bench 종료부가 부른다) ─────────────────────────────────────
    if args.sweep_index:
        if cell_args:
            sys.stderr.write("[classify_cell] ERROR --sweep-index(bench-mode 모드)와 셀 종결 인자(%s)를 섞지 않는다 — "
                             "셀 기록은 판정 기록(%s)을 읽는다\n" % (", ".join(cell_args), BENCH_MODE_RECORD_NAME))
            return 2
        try:
            with open(args.sweep_index, encoding="utf-8") as handle:
                index_doc = json.load(handle)
        except (OSError, ValueError) as exc:
            # 판독하지 못한 index 는 판정 대상이 아니다 — 기록을 쓰지 않는다(낡은 기록은 소비자가 stale 로 거른다).
            sys.stderr.write("[classify_cell] ERROR sweep-index 판독 실패(%s): %s\n" % (args.sweep_index, exc))
            return 2
        if not isinstance(index_doc, dict):
            sys.stderr.write("[classify_cell] ERROR sweep-index 가 객체가 아니다(%s)\n" % args.sweep_index)
            return 2
        lines, scanned = _read_events(event_paths)
        bm = classify_bench_mode(index_doc, lines, scanned, args.tolerance_s)
        record = dict(bm, schema_version=1, kind="bench_mode_record", generated_by="classify_cell.py",
                      sweep_index=args.sweep_index, sweep_index_generated_utc=index_doc.get("generated_utc"),
                      events_scanned=scanned, tolerance_s=args.tolerance_s)
        if args.write_bench_mode:
            try:
                record["record_path"] = write_bench_mode_record(args.sweep_index, record)
            except OSError as exc:
                sys.stderr.write("[classify_cell] ERROR bench_mode 기록 실패(%s): %s\n"
                                 % (bench_mode_record_path(args.sweep_index), exc))
                print(json.dumps(record, ensure_ascii=False, indent=2))
                return 2
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 0

    # ── 셀 종결 모드 ─────────────────────────────────────────────────────────────────────────────
    if args.measure_rc in ("absent", ""):
        args.measure_rc = None
    elif args.measure_rc is not None:
        try:
            args.measure_rc = int(args.measure_rc)
        except ValueError:
            sys.stderr.write("[classify_cell] ERROR --measure-rc 는 정수 또는 `absent`\n")
            return 2
    else:
        sys.stderr.write("[classify_cell] ERROR --measure-rc 미지정 — 부재는 `absent` 로 **명시**하라"
                         "(빠뜨림과 부재를 구분한다)\n")
        return 2
    missing = [n for n, v in (("--serve-rc", args.serve_rc),
                              ("--started-utc", args.started_utc), ("--ended-utc", args.ended_utc))
               if v is None]
    if missing:
        sys.stderr.write("[classify_cell] ERROR 필수 인자 부재: %s\n" % ", ".join(missing))
        return 2

    lines, scanned = _read_events(event_paths)
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
