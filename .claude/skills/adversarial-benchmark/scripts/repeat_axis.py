#!/usr/bin/env python3
# repeat_axis.py — full bench 반복 축의 단일 소유자 (plan_26091407 §4.4 · §5 ④ · §7 O4)
#
# ★ full bench 의 정의(사용자 결정 Q3 · 2026-09-13 인터뷰): `full = lite ∪ GuideLLM × 반복 ≥3`.
#   반복은 분산·신뢰성의 최소조건이다. 종전 full 은 동시성 스윕의 레벨마다 GuideLLM 1회였고
#   (audit_26091323 §2 B4), 셀당 1회라 **산포 추정치 자체가 없었다** — 그 공백을 다른 도구·다른 조건의
#   밴드로 메우다가 리포트의 추론이 틀렸다(같은 감사 §2.4b).
#
# 이 파일이 소유하는 것(한 곳에만 적는다 — 소비자는 import 한다):
#   · FULL_REPEATS_MIN(=3) — full 정의의 반복 하한. `campaigns/_template/campaign.schema.json` 의
#     `budgets.repeats.minimum` 과 **같은 개념**이며 정적 파일끼리는 한쪽이 다른 쪽을 생성할 수 없으므로
#     자체검사가 둘을 **교차검증**한다(workflow.md §결정론 규율 — 단일 소유 불가 시 교차검증이 차선).
#   · 반복 수 해소(resolve): argv `--repeats` > 활성 캠페인 `campaign.yaml budgets.repeats` > full 정의값.
#     활성 캠페인 해소는 `terraforming_node/scripts/campaign_init.py` 가 소유하므로 **import 해서** 쓴다.
#     출처는 값 옆에 스스로 밝힌다(`repeats_source`).
#   · run 집계(aggregate-level): 레벨의 대표 run(= 첫 완주 run) `measured.json` 에 `runs[]` ·
#     `repro_band_pct` · `repro_band_source=measured(n=N)` · `repeat_kind` 를 덧붙인다. 대표 run 의 파서
#     필드는 **그대로** 둔다 — judge_bench 승계(accept_len 등)·verdict·인증서가 종전처럼 그 값을 읽는다.
#   · 반복 요약(summarize): 레벨별 완주 수·스윕이 멈춘 자리 **즉시 신호**(`stop` — 끊긴 run 과 시각 대조 창).
#   · 반복 조건(repetition_established): full 정의의 반복 조건이 섰는가(아래 ★ 반복 조건의 범위).
#
# 이 파일이 소유하지 **않는** 것:
#   · bench_mode(full|lite)·downgrade_reason 의 **확정** — `classify_cell.py`(post-hoc · void_reason 과 동형).
#     여기서 내는 것은 raw 요약과 즉시 신호뿐이다. 블랙박스 kill 이벤트를 읽지 않는다.
#   · 판정 게이트 — 불변이다. 밴드는 **기재**이며 분산은 강등 사유가 아니다.
#
# 반복 종류(repeat_kind): `warm-rerun`(같은 running serve 에 연속 재측정 — 이번 구현) ·
#   `cold-restart`(재기동 후 재측정 — **선언 슬롯만**. 기동은 이 스킬의 소유가 아니라 explorer 소관이며,
#   구현 전까지 이 값을 내는 실행자는 없다).
#
# ★ 반복 조건의 범위(2026-09-14 리뷰 정정) — **판정점**(sweep_index `verdict_point_level` · sweep_bench 가 강제
#   포함하는 동시성 1)이 반복 ≥3 을 채웠는가가 full 정의의 반복 조건이다. 근거는 셋이다:
#   ① 적응 상한 클램프는 스윕의 **정상 산출**이다 — 상위 레벨이 무너지는 자리가 곧 포화 경계이고, 그 경계에서
#     첫 run 이 무너지든 둘째 run 이 무너지든 같은 사실(그 동시성을 반복해 버티지 못했다)이다. 종전 규칙(측정
#     레벨 전체의 완주 min)은 첫 run 실패면 full·인증서, 둘째 run 실패면 lite·미발행으로 **같은 경계를 반대로**
#     판정했다(판정점이 3회 모두 완주했어도).
#   ② 판정·인증서가 묶이는 것은 판정점 대표 run 하나다 — 발행 여부를 비판정점 경계 레벨의 반복 수가 정하면
#     증거와 판정이 다른 레벨을 본다.
#   ③ 스윕은 끊긴 자리에서 멈추므로(아래 stop) 경계 **아래** 레벨은 구조적으로 요청 반복을 다 채운다 — 경계
#     밖에서 정의 미만 레벨이 보이면 그것은 기계 이벤트가 아니라 규칙 밖 산출물이다(판정 불가).
#   스윕이 멈춘 자리(`stop`)는 두 종류다: `repeat-break`(대표 run 이 선 레벨의 run k≥2 실패) · `clamp`(레벨
#   첫 run 실패 — 그 레벨은 측정 레벨이 아니다). 둘 다 시각 대조 창을 가지며, 그 창 안의 **집행된** 블랙박스
#   사살은 레벨·run 순번과 무관하게 강등 사유다(사용자 결정 — kill 이벤트는 기계 이벤트 트리거다). 사인 대조는
#   분류기(`classify_cell.py`)가 하고, 여기서는 창과 반복 조건만 계산한다.
#
# 사용: repeat_axis.py resolve [--repeats N [--repeats-source TEXT]] [--campaign-id ID]
#       repeat_axis.py record-run --run-dir D --level L --run K --started-utc T --ended-utc T
#                                 --run-bench-rc N --parse-rc N
#       repeat_axis.py aggregate-level --level-dir D --requested N --requested-source TEXT [--kind K]
#       repeat_axis.py --self-test
# 종료: 0=성공 · 2=인자/선언 오류(fail-closed) · 1=자체검사 실패
from __future__ import annotations

import argparse
import importlib.util
import json
import math
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[4]
CAMPAIGN_INIT_REL = ".claude/skills/terraforming_node/scripts/campaign_init.py"
SCHEMA_REL = "campaigns/_template/campaign.schema.json"

# full 정의의 반복 하한(사용자 결정 Q3). schema minimum 과 교차검증된다(아래 자체검사 A1).
FULL_REPEATS_MIN = 3

REPEAT_KIND_WARM = "warm-rerun"
REPEAT_KIND_COLD = "cold-restart"
REPEAT_KINDS = (REPEAT_KIND_WARM, REPEAT_KIND_COLD)
# 실행자가 실제로 내는 종류. cold-restart 는 선언 슬롯만이다(위 헤더).
IMPLEMENTED_REPEAT_KINDS = (REPEAT_KIND_WARM,)

RUN_META_NAME = "repeat_run.json"
RUN_DIR_RE = re.compile(r"^run_(\d{2,})$")
# 즉시 신호의 이름. 확정 어휘(`classify_cell.DOWNGRADE_REASONS`)와 **같은 토큰**이지만 뜻의 층이 다르다 —
# 여기는 "run 이 무너졌다" 는 관측이고, 강등 사유로 확정하는 것(블랙박스 kill 이면 blackbox_kill)은 분류기다.
SIGNAL_RUN_FAILED = "run_failed"
# 스윕이 멈춘 자리의 종류(위 헤더 ★). sweep_bench 의 절삭 로그 문구(`truncated` · `truncated-above … repeat-break`)와
# 같은 사실을 구조로 든다.
STOP_KIND_REPEAT_BREAK = "repeat-break"
STOP_KIND_CLAMP = "clamp"
# runs[] 에 옮기는 지표. 대표 run 의 전체 파서 출력은 measured.json 최상위가 이미 들고 있으므로 복제하지
# 않고, 산포·완주 판독에 필요한 열만 싣는다(나머지 run 의 전체 출력은 run_KK/measured.json 에 있다).
RUN_METRIC_KEYS = ("decode_tps", "output_throughput", "ttft_ms_median", "itl_ms_median",
                   "tpot_ms_median", "completed", "failed", "server_error_rate", "accept_len")
BAND_METRIC = "decode_tps"
BAND_FORMULA = "(max-min)/mean*100"


class RepeatAxisError(ValueError):
    """반복 축 입력이 성립하지 않는다. 기본값으로 메우지 않고 호출자에게 돌려준다(fail-closed)."""


def _is_int(v) -> bool:
    return isinstance(v, int) and not isinstance(v, bool)


def _load_json(path: Path):
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError):
        return None


def _campaign_init_module(repo_root: Path):
    """활성 캠페인 해소의 단일 소유자를 적재한다. 없으면 None — 호출자가 부재를 **이름으로** 남긴다."""
    path = Path(repo_root) / CAMPAIGN_INIT_REL
    if not path.is_file():
        return None
    spec = importlib.util.spec_from_file_location("_repeat_axis_campaign_init", path)
    if spec is None or spec.loader is None:
        return None
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def parse_repeats(raw, where: str) -> int:
    """반복 수 값역: 정수 ∧ ≥ FULL_REPEATS_MIN. 3 미만은 full 의 정의 위반이라 **거부**한다 —
    lite 로 표시해 통과시키면 "반복이 성립하지 않았다"(기계 이벤트)와 "처음부터 적게 돌렸다"(선언)가
    한 칸에 섞인다. lite 만 원하면 `lite_bench.sh` 가 그 통로다."""
    if isinstance(raw, str):
        text = raw.strip()
        if not re.fullmatch(r"[0-9]+", text):
            raise RepeatAxisError("%s=%r 는 정수가 아니다" % (where, raw))
        value = int(text)
    elif _is_int(raw):
        value = raw
    else:
        raise RepeatAxisError("%s=%r 는 정수가 아니다" % (where, raw))
    if value < FULL_REPEATS_MIN:
        raise RepeatAxisError(
            "%s=%d 는 full 정의(반복 ≥%d) 위반이다 — 반복 수를 낮춰 full 을 선언하지 않는다. "
            "반복이 성립하지 않으면 선언이 아니라 기계 이벤트(run 실패·블랙박스 kill)가 셀을 lite 로 "
            "강등한다(classify_cell). lite 만 재려면 lite_bench.sh 를 쓰라."
            % (where, value, FULL_REPEATS_MIN))
    return value


def resolve_repeats(argv_value=None, *, argv_source=None, campaign_id=None,
                    repo_root: Path = REPO_ROOT) -> "tuple[int, str]":
    """(반복 수, 출처). 우선순위 argv > 활성 캠페인 선언 > full 정의값.

    · 선언을 **읽지 못한** 것(파손·모양 불량)은 미선언으로 접지 않는다 — RepeatAxisError.
    · 해소자(campaign_init) 부재는 fail-loud 대체다: 반복 수는 게이트가 아니라 측정 구성이고 정의값 3 은
      정의를 충족하므로 측정을 막지 않되, 출처에 그 사실을 적는다(부재를 파생이라 적지 않는다).
    """
    if argv_value is not None and str(argv_value).strip() != "":
        n = parse_repeats(argv_value, "--repeats")
        src = "declared(argv --repeats%s)" % ((" · %s" % argv_source) if argv_source else "")
        return n, src
    if argv_source:
        raise RepeatAxisError("--repeats-source 는 --repeats 와 함께만 쓴다(값 없는 출처는 거짓 표시다)")
    mod = _campaign_init_module(repo_root)
    if mod is None:
        return FULL_REPEATS_MIN, ("defaulted(full 정의 최소 %d · 캠페인 해소자 부재: %s — 선언을 읽지 못했다)"
                                  % (FULL_REPEATS_MIN, CAMPAIGN_INIT_REL))
    camp = campaign_id or mod.active_campaign_id()
    if camp == mod.BOOTSTRAP:
        return FULL_REPEATS_MIN, ("defaulted(full 정의 최소 %d · 활성 캠페인 없음(%s))"
                                  % (FULL_REPEATS_MIN, mod.BOOTSTRAP))
    if camp in getattr(mod, "RESERVED_IDS", ()):
        raise RepeatAxisError("--campaign-id %r 는 예약 id 다 — 선언이 아니다" % camp)
    base = mod.derive_path("root", camp_id=camp)
    decl = base / "campaign.yaml"
    if not base.is_dir():
        raise RepeatAxisError("campaigns/%s 인스턴스가 없다 — 반복 수를 물을 선언이 없다" % camp)
    doc = _load_json(decl)
    if not isinstance(doc, dict):
        raise RepeatAxisError("campaigns/%s/campaign.yaml 을 읽지 못했다(부재·파손) — 읽지 못한 선언을 "
                              "'미선언' 으로 접지 않는다" % camp)
    budgets = doc.get("budgets", {})
    if not isinstance(budgets, dict):
        raise RepeatAxisError("campaigns/%s/campaign.yaml budgets 가 객체가 아니다" % camp)
    if "repeats" not in budgets:
        return FULL_REPEATS_MIN, ("defaulted(full 정의 최소 %d · campaigns/%s/campaign.yaml budgets.repeats 미선언)"
                                  % (FULL_REPEATS_MIN, camp))
    n = parse_repeats(budgets["repeats"], "campaigns/%s/campaign.yaml budgets.repeats" % camp)
    return n, "declared(campaigns/%s/campaign.yaml budgets.repeats)" % camp


def band_pct(values) -> "tuple[float | None, str]":
    """재현 밴드 = (max−min)/mean×100 (완주 run 의 decode_tps). 2회 미만이면 정의되지 않는다.

    식의 근거: 캠페인이 셀마다 3회 돌려 적은 "산포" 가 이 식이다(audit_26091323 §2.3 원장:
    11.759/11.971/11.301 → 5.74%). 같은 이름 아래 다른 식을 쓰면 옛 기록과 비교가 조용히 깨진다."""
    vals = [float(v) for v in values
            if isinstance(v, (int, float)) and not isinstance(v, bool) and math.isfinite(float(v))]
    n = len(vals)
    if n < 2:
        return None, "unavailable(n=%d — 산포는 완주 2회 이상에서만 정의된다)" % n
    mean = sum(vals) / n
    if mean <= 0:
        return None, "unavailable(n=%d — 평균이 0 이하라 상대 산포가 정의되지 않는다)" % n
    return round((max(vals) - min(vals)) / mean * 100.0, 4), "measured(n=%d)" % n


def record_run(run_dir: Path, *, level: int, run: int, started_utc: str, ended_utc: str,
               run_bench_rc: int, parse_rc: int) -> Path:
    """run 하나의 raw 사실(시각·rc)을 그 run 디렉터리에 남긴다. 측정값은 파서 출력이 따로 든다."""
    run_dir = Path(run_dir)
    run_dir.mkdir(parents=True, exist_ok=True)
    doc = {"schema_version": 1, "level": level, "run": run,
           "started_utc": started_utc, "ended_utc": ended_utc,
           "run_bench_rc": run_bench_rc, "parse_rc": parse_rc,
           "provenance": "measured(sweep_bench 반복 루프 · 시각은 run 경계에서 잰 벽시계)"}
    out = run_dir / RUN_META_NAME
    out.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return out


def _run_dirs(level_dir: Path) -> "list[tuple[int, Path]]":
    """(run 번호, 디렉터리). run 1 은 레벨 디렉터리 자체(종전 배치 그대로 — 기존 소비자 호환)."""
    out = [(1, level_dir)]
    for d in level_dir.iterdir() if level_dir.is_dir() else ():
        m = RUN_DIR_RE.match(d.name)
        if d.is_dir() and m and int(m.group(1)) >= 2:
            out.append((int(m.group(1)), d))
    return sorted(out, key=lambda x: x[0])


def aggregate_level(level_dir: Path, *, requested: int, requested_source: str,
                    kind: str = REPEAT_KIND_WARM) -> dict:
    """대표 run 의 measured.json 에 반복 축을 덧붙여 **제자리** 쓴다. 반환 = 갱신된 문서.

    대표 run = 첫 완주 run. 레벨 첫 run 이 무너지면 그 레벨은 절삭(측정 레벨 아님)이라 이 함수의 대상이
    아니다 — 따라서 대표 run 은 언제나 run 1 이며, 판정점·인증서가 읽는 최상위 필드는 run 1 의 파서
    출력 그대로다(평균 등 합성값으로 바꾸지 않는다)."""
    level_dir = Path(level_dir)
    if kind not in IMPLEMENTED_REPEAT_KINDS:
        raise RepeatAxisError("repeat_kind=%r 는 구현되지 않았다(구현: %s · 선언 슬롯: %s)"
                              % (kind, IMPLEMENTED_REPEAT_KINDS, REPEAT_KINDS))
    rep_path = level_dir / "measured.json"
    rep = _load_json(rep_path)
    if not isinstance(rep, dict) or rep.get("measurement_ok") is not True:
        raise RepeatAxisError("%s 가 완주한 대표 run 이 아니다 — 절삭 레벨은 집계 대상이 아니다" % rep_path)
    runs = []
    for number, d in _run_dirs(level_dir):
        meta = _load_json(d / RUN_META_NAME)
        if not isinstance(meta, dict):
            raise RepeatAxisError("%s 부재·파손 — run 경계 사실이 없으면 시각 대조가 성립하지 않는다"
                                  % (d / RUN_META_NAME))
        m = rep if d == level_dir else _load_json(d / "measured.json")
        ok = bool(meta.get("run_bench_rc") == 0 and meta.get("parse_rc") == 0
                  and isinstance(m, dict) and m.get("measurement_ok") is True)
        entry = {"run": number, "dir": "." if d == level_dir else d.name,
                 "started_utc": meta.get("started_utc"), "ended_utc": meta.get("ended_utc"),
                 "run_bench_rc": meta.get("run_bench_rc"), "parse_rc": meta.get("parse_rc"),
                 "measurement_ok": ok}
        for key in RUN_METRIC_KEYS:
            entry[key] = m.get(key) if isinstance(m, dict) else None
        runs.append(entry)
    base = {k: v for k, v in rep.items() if k not in _AGG_KEYS}
    completed = [r for r in runs if r["measurement_ok"]]
    band, band_src = band_pct([r.get(BAND_METRIC) for r in completed])
    doc = dict(base)
    doc.update({
        "runs": runs,
        "repeats_requested": requested,
        "repeats_requested_source": requested_source,
        "repeats_completed": len(completed),
        "repeat_kind": kind,
        "representative_run": completed[0]["run"],
        "representative_run_rule": "첫 완주 run — 최상위 측정 필드·판정점·인증서가 이 run 하나에 묶인다(합성 ✗)",
        "repro_band_pct": band,
        "repro_band_source": band_src,
        "repro_band_metric": BAND_METRIC,
        "repro_band_formula": BAND_FORMULA,
    })
    rep_path.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return doc


_AGG_KEYS = ("runs", "repeats_requested", "repeats_requested_source", "repeats_completed",
             "repeat_kind", "representative_run", "representative_run_rule", "repro_band_pct",
             "repro_band_source", "repro_band_metric", "repro_band_formula")


def summarize(levels, *, requested=None, requested_source=None, kind=None, clamp_run=None) -> dict:
    """sweep_index `levels[]`(각 레벨의 대표 measured.json 포함) → 반복 요약 + 스윕이 멈춘 자리(즉시 신호).

    `stop` = 스윕이 멈춘 자리 하나(스윕은 끊기면 멈추므로 둘 이상은 없다):
      · `repeat-break` — 레벨 순서로 처음 무너진 run(run ≥ 2). runs[] 에서 계산한다.
      · `clamp`        — 측정 레벨 위에서 첫 run 이 무너진 레벨. 그 레벨은 index 에 실리지 않으므로 경계 사실은
                         `clamp_run`(sweep_bench 가 그 run 디렉터리의 repeat_run.json 에서 넘긴다)에서 온다.
    시각 대조 창은 **직전 run 시작 ~ 끊긴 run 끝**이다 — 직전 run 이 완주했어도 그 꼬리(측정 종료 뒤 · 다음 run
    시작 전)에 집행된 사살이 다음 run 을 무너뜨릴 수 있고, 창을 스윕 전체로 넓히면 무관한 앞선 사살이 사인으로
    붙는다. 사인 확정은 분류기다. `runs_attempted` = 측정 레벨 runs[] 의 run 수 + 클램프 레벨 첫 run(1) —
    **레벨 run 시도 합**이다(lite 선행 레그는 레벨 run 이 아니라 세지 않는다)."""
    by_level, band_by_level, unrecorded = {}, {}, []
    runs_attempted = 0
    stop = None
    last_run_by_level = {}
    for lv in sorted((x for x in levels or [] if isinstance(x, dict)), key=lambda x: x.get("level") or 0):
        level = lv.get("level")
        m = lv.get("measured") if isinstance(lv.get("measured"), dict) else {}
        runs = m.get("runs")
        if not isinstance(runs, list):
            unrecorded.append(level)
            continue
        ordered = sorted((r for r in runs if isinstance(r, dict)), key=lambda r: r.get("run") or 0)
        by_level[str(level)] = sum(1 for r in ordered if r.get("measurement_ok") is True)
        band_by_level[str(level)] = m.get("repro_band_pct")
        runs_attempted += len(ordered)
        if ordered:
            last_run_by_level[level] = ordered[-1]
        if stop is not None:
            continue
        for i, r in enumerate(ordered):
            if r.get("measurement_ok") is True or (r.get("run") or 0) < 2:
                continue
            prev = ordered[i - 1] if i > 0 else r
            stop = {"kind": STOP_KIND_REPEAT_BREAK, "level": level, "run": r.get("run"), "signal": SIGNAL_RUN_FAILED,
                    "detail": "run_bench rc=%s · parse rc=%s · measurement_ok=false"
                              % (r.get("run_bench_rc"), r.get("parse_rc")),
                    "window_start_utc": prev.get("started_utc"),
                    "window_end_utc": r.get("ended_utc"),
                    "window_rule": "직전 run 시작 ~ 끊긴 run 끝(사인 확정은 classify_cell)"}
            break
    if isinstance(clamp_run, dict):
        runs_attempted += 1
        if stop is None:
            c_level = clamp_run.get("level")
            below = [lvl for lvl in last_run_by_level if isinstance(lvl, int) and isinstance(c_level, int)
                     and lvl < c_level]
            prev = last_run_by_level[max(below)] if below else clamp_run
            unreadable = clamp_run.get("unreadable")
            stop = {"kind": STOP_KIND_CLAMP, "level": c_level, "run": clamp_run.get("run"),
                    "signal": SIGNAL_RUN_FAILED,
                    "detail": (("경계 사실 판독 불가: %s" % unreadable) if unreadable else
                               "run_bench rc=%s · parse rc=%s · measurement_ok=false(레벨 첫 run — 적응 상한 클램프)"
                               % (clamp_run.get("run_bench_rc"), clamp_run.get("parse_rc"))),
                    "window_start_utc": None if unreadable else prev.get("started_utc"),
                    "window_end_utc": None if unreadable else clamp_run.get("ended_utc"),
                    "window_rule": "직전 run(아래 측정 레벨의 마지막 run) 시작 ~ 클램프 run 끝(사인 확정은 classify_cell)"}
    short = sorted(int(k) for k, v in by_level.items() if v < FULL_REPEATS_MIN)
    return {
        "requested": requested,
        "requested_source": requested_source,
        "kind": kind,
        "full_min": FULL_REPEATS_MIN,
        "completed_by_level": by_level,
        "completed_min": min(by_level.values()) if by_level else None,
        "short_levels": short,
        "runs_attempted": runs_attempted,
        "band_pct_by_level": band_by_level,
        "band_metric": BAND_METRIC,
        "band_formula": BAND_FORMULA,
        "stop": stop,
        # 입력 그대로 — 재조립이 **측정 시점의** 클램프 경계 사실을 승계하는 자리다(디스크를 다시 읽지 않는다).
        "clamp_run": clamp_run if isinstance(clamp_run, dict) else None,
        "unrecorded_levels": unrecorded,
        "_note": "raw 요약과 즉시 신호다 — bench_mode(full|lite)·downgrade_reason 확정은 classify_cell.py(post-hoc)",
    }


def repetition_established(summary, judgment_level) -> "tuple[bool | None, str]":
    """full 정의의 **반복 조건**(위 헤더 ★ 반복 조건의 범위). 블랙박스 사인은 보지 않는다 — 그건 분류기다.

    True  = 판정점 완주 ≥ FULL_REPEATS_MIN (경계 레벨 · 판정점 밖의 정의 미만 레벨이 없을 때)
    False = 판정점 완주 < FULL_REPEATS_MIN ∧ 판정점에서 반복이 끊겼다(기계 이벤트 — run 실패)
    None  = 판정 불가(runs[] 부재 레벨 · 판정점 부재 · 끊김 없이 정의 미만 · 경계 밖 정의 미만)
    """
    if not isinstance(summary, dict):
        return None, "not_evaluated(반복 요약 없음)"
    if summary.get("unrecorded_levels"):
        return None, ("not_evaluated(runs[] 부재 레벨 %s — 반복 축 신설 전 산출물이거나 집계 실패 · "
                      "강등 사유를 발명하지 않는다)" % summary["unrecorded_levels"])
    if not _is_int(judgment_level):
        return None, "not_evaluated(판정점 레벨(verdict_point_level) 부재 — 반복 조건을 볼 레벨이 없다)"
    by_level = summary.get("completed_by_level") or {}
    jc = by_level.get(str(judgment_level))
    if not _is_int(jc):
        return None, "not_evaluated(판정점 레벨 %d 가 측정 레벨에 없다)" % judgment_level
    stop = summary.get("stop") if isinstance(summary.get("stop"), dict) else None
    boundary = (stop.get("level") if stop and stop.get("kind") == STOP_KIND_REPEAT_BREAK
                and stop.get("level") != judgment_level else None)
    stray = [lvl for lvl in summary.get("short_levels") or [] if lvl not in (judgment_level, boundary)]
    if stray:
        return None, ("unclassifiable(레벨 %s 이 반복 정의 미만인데 그 자리에서 스윕이 멈추지 않았다 — 기계 이벤트가 "
                      "아니므로 강등 사유를 발명하지 않는다)" % stray)
    if jc >= FULL_REPEATS_MIN:
        tail = ""
        if boundary is not None:
            tail = (" · 경계 level %s run %s 반복 중단 = 적응 상한 클램프(상위 레벨 미측정 · 판정점 반복은 성립)"
                    % (boundary, stop.get("run")))
        elif stop and stop.get("kind") == STOP_KIND_REPEAT_BREAK:
            tail = " · 판정점 level %s run %s 에서 반복 중단(완주는 정의 충족 · 상위 레벨 미측정)" % (
                stop.get("level"), stop.get("run"))
        elif stop and stop.get("kind") == STOP_KIND_CLAMP:
            tail = " · level %s 첫 run 에서 적응 상한 클램프" % stop.get("level")
        return True, "runs[](판정점 level %d 완주 %d ≥ full 정의 %d)%s" % (judgment_level, jc, FULL_REPEATS_MIN, tail)
    if stop and stop.get("kind") == STOP_KIND_REPEAT_BREAK and stop.get("level") == judgment_level:
        return False, ("runs[](판정점 level %d 완주 %d < full 정의 %d · run %s 에서 반복 중단)"
                       % (judgment_level, jc, FULL_REPEATS_MIN, stop.get("run")))
    return None, ("unclassifiable(판정점 완주 %d < full 정의 %d 인데 판정점에서 끊긴 run 이 없다 — 요청 자체가 정의 미만인 "
                  "산출물 · 강등 사유를 발명하지 않는다)" % (jc, FULL_REPEATS_MIN))


# ── 자체검사 ─────────────────────────────────────────────────────────────────────────────────
def _self_test() -> int:
    import tempfile
    failures = []

    def check(name, cond, detail=""):
        print("  [%s] %s%s" % ("PASS" if cond else "FAIL", name, "" if cond else " " + str(detail)))
        if not cond:
            failures.append(name)

    # A1 — 정적 파일 교차검증: schema minimum 과 여기 상수가 같은 개념이다.
    #    경로가 바뀌면(스키마 개편) 조용히 건너뛰지 않고 None 으로 떨어져 FAIL 한다.
    schema = _load_json(REPO_ROOT / SCHEMA_REL)
    try:
        minimum = schema["definitions"]["budgets"]["properties"]["repeats"]["minimum"]
    except (KeyError, TypeError):
        minimum = None
    check("A1 ★교차검증 campaign.schema.json budgets.repeats.minimum == FULL_REPEATS_MIN",
          minimum == FULL_REPEATS_MIN, "(schema=%r · 상수=%r)" % (minimum, FULL_REPEATS_MIN))

    # B — 밴드 식은 캠페인이 적은 산포와 같은 식이다(역채점).
    band, src = band_pct([11.759, 11.971, 11.301])
    check("B1 역채점: 11.759/11.971/11.301 → 5.74% (audit_26091323 §2.3 원장)",
          band is not None and round(band, 2) == 5.74 and src == "measured(n=3)", (band, src))
    band, src = band_pct([30.0])
    check("B2 완주 1회 → 밴드 없음(0 으로 적지 않는다)", band is None and src.startswith("unavailable(n=1"))
    band, src = band_pct([30.0, None, float("nan"), 30.0])
    check("B3 결측·NaN 은 n 에서 빠진다", band == 0.0 and src == "measured(n=2)", (band, src))

    # P — 값역.
    for bad in ("2", 1, "abc", True, 2.5, ""):
        try:
            parse_repeats(bad, "x")
        except RepeatAxisError:
            ok = True
        else:
            ok = False
        check("P1 ★반복 수 %r 거부(full 정의 위반·비정수)" % (bad,), ok)
    check("P2 3·'5' 수용", parse_repeats(3, "x") == 3 and parse_repeats("5", "x") == 5)

    # R — 해소. 임시 저장소에 campaign_init 사본을 두고 활성 캠페인·선언을 바꿔 가며 묻는다.
    with tempfile.TemporaryDirectory(prefix="repeat-axis.") as td:
        root = Path(td)
        ci_src = REPO_ROOT / CAMPAIGN_INIT_REL
        (root / CAMPAIGN_INIT_REL).parent.mkdir(parents=True)
        (root / CAMPAIGN_INIT_REL).write_bytes(ci_src.read_bytes())
        camps = root / "campaigns"
        (camps / "camp-a").mkdir(parents=True)

        def decl(budgets):
            doc = {"id": "camp-a"}
            if budgets is not None:
                doc["budgets"] = budgets
            (camps / "camp-a" / "campaign.yaml").write_text(json.dumps(doc), encoding="utf-8")

        n, src = resolve_repeats(repo_root=root)
        check("R1 ACTIVE 부재 → 3 · defaulted(_bootstrap)", n == 3 and "_bootstrap" in src and src.startswith("defaulted("), src)
        (camps / "ACTIVE").write_text("camp-a\n", encoding="utf-8")
        decl({"smoke_budget_overhead_mib": 1})
        n, src = resolve_repeats(repo_root=root)
        check("R2 선언 budgets.repeats 없음 → 3 · defaulted(미선언)", n == 3 and "미선언" in src, src)
        decl({"repeats": 5})
        n, src = resolve_repeats(repo_root=root)
        check("R3 선언 5 → 5 · declared(campaign.yaml)", n == 5 and src == "declared(campaigns/camp-a/campaign.yaml budgets.repeats)", src)
        n, src = resolve_repeats("4", argv_source="sweep state", repo_root=root)
        check("R4 argv 가 선언을 이긴다 · 출처에 argv", n == 4 and src.startswith("declared(argv --repeats · sweep state"), src)
        decl({"repeats": 2})
        try:
            resolve_repeats(repo_root=root)
            check("R5 ★선언 2(검증기가 막았어야 할 값) → 거부(기본값 대체 ✗)", False)
        except RepeatAxisError:
            check("R5 ★선언 2(검증기가 막았어야 할 값) → 거부(기본값 대체 ✗)", True)
        (camps / "camp-a" / "campaign.yaml").write_text("{broken", encoding="utf-8")
        try:
            resolve_repeats(repo_root=root)
            check("R6 ★선언 파손 → 거부('미선언' 으로 접지 않는다)", False)
        except RepeatAxisError:
            check("R6 ★선언 파손 → 거부('미선언' 으로 접지 않는다)", True)
        decl({"repeats": 6})
        (camps / "camp-b").mkdir()
        (camps / "camp-b" / "campaign.yaml").write_text(json.dumps({"budgets": {"repeats": 7}}), encoding="utf-8")
        n, src = resolve_repeats(campaign_id="camp-b", repo_root=root)
        check("R7 --campaign-id 가 ACTIVE 보다 우선(상태 파일의 캠페인)", n == 7 and "camp-b" in src, src)
        try:
            resolve_repeats(argv_source="x", repo_root=root)
            check("R8 ★값 없는 --repeats-source 거부", False)
        except RepeatAxisError:
            check("R8 ★값 없는 --repeats-source 거부", True)
        (root / CAMPAIGN_INIT_REL).unlink()
        n, src = resolve_repeats(repo_root=root)
        check("R9 해소자 부재 → 3 · 출처에 부재를 이름으로 남긴다(fail-loud 대체)",
              n == 3 and "해소자 부재" in src, src)

        # G — 집계와 요약.
        ldir = root / "sweep" / "level_01"
        ldir.mkdir(parents=True)
        rep = {"decode_tps": 30.0, "accept_len": 2.0, "spec_axis_source": "inherited(w)",
               "measurement_ok": True, "completed": 16, "failed": 0}
        (ldir / "measured.json").write_text(json.dumps(rep), encoding="utf-8")
        record_run(ldir, level=1, run=1, started_utc="2026-01-01T00:00:00Z", ended_utc="2026-01-01T00:01:00Z",
                   run_bench_rc=0, parse_rc=0)
        for k, tps in ((2, 31.0), (3, 29.0)):
            d = ldir / ("run_%02d" % k)
            d.mkdir()
            (d / "measured.json").write_text(json.dumps(dict(rep, decode_tps=tps)), encoding="utf-8")
            record_run(d, level=1, run=k, started_utc="2026-01-01T00:0%d:00Z" % k,
                       ended_utc="2026-01-01T00:0%d:30Z" % k, run_bench_rc=0, parse_rc=0)
        doc = aggregate_level(ldir, requested=3, requested_source="declared(x)")
        check("G1 대표 run 필드 그대로(decode_tps 30.0 · accept_len 2.0 · 평균으로 바꾸지 않는다)",
              doc["decode_tps"] == 30.0 and doc["accept_len"] == 2.0 and doc["representative_run"] == 1)
        check("G2 runs[] 3 · 완주 3 · 밴드 (31−29)/30×100 = 6.6667 · measured(n=3)",
              len(doc["runs"]) == 3 and doc["repeats_completed"] == 3
              and doc["repro_band_pct"] == 6.6667 and doc["repro_band_source"] == "measured(n=3)", doc)
        doc2 = aggregate_level(ldir, requested=3, requested_source="declared(x)")
        check("G3 재집계는 멱등(runs[] 가 중첩되지 않는다)", doc2 == doc)
        s = summarize([{"level": 1, "measured": doc}], requested=3, requested_source="declared(x)", kind=REPEAT_KIND_WARM)
        check("G4 요약: 완주 min 3 · 멈춘 자리 없음 · runs 3",
              s["completed_min"] == 3 and s["stop"] is None and s["runs_attempted"] == 3 and not s["unrecorded_levels"], s)
        # 실패주입: run 3 이 무너졌다(parse ok 이나 measurement_ok=false).
        d3 = ldir / "run_03"
        (d3 / "measured.json").write_text(json.dumps(dict(rep, decode_tps=5.0, measurement_ok=False)), encoding="utf-8")
        doc3 = aggregate_level(ldir, requested=3, requested_source="declared(x)")
        s3 = summarize([{"level": 1, "measured": doc3}])
        check("G5 ★run 3 실패 → 완주 2 · 밴드는 완주분만(n=2) · 멈춘 자리 repeat-break run 3",
              doc3["repeats_completed"] == 2 and doc3["repro_band_source"] == "measured(n=2)"
              and s3["stop"] and s3["stop"]["run"] == 3 and s3["stop"]["kind"] == STOP_KIND_REPEAT_BREAK
              and s3["stop"]["signal"] == SIGNAL_RUN_FAILED, (doc3, s3))
        check("G6 시각 대조 창 = 직전 run 시작 ~ 끊긴 run 끝",
              s3["stop"]["window_start_utc"] == "2026-01-01T00:02:00Z"
              and s3["stop"]["window_end_utc"] == "2026-01-01T00:03:30Z", s3["stop"])
        # run_bench 실패(측정 파일 없음)도 같은 신호.
        (d3 / "measured.json").unlink()
        record_run(d3, level=1, run=3, started_utc="2026-01-01T00:03:00Z", ended_utc="2026-01-01T00:03:30Z",
                   run_bench_rc=3, parse_rc=0)
        doc4 = aggregate_level(ldir, requested=3, requested_source="declared(x)")
        check("G7 ★run_bench rc=3(측정 파일 없음) → measurement_ok=false 로 기록 · 지표 None",
              doc4["runs"][2]["measurement_ok"] is False and doc4["runs"][2]["decode_tps"] is None
              and doc4["runs"][2]["run_bench_rc"] == 3)
        s5 = summarize([{"level": 1, "measured": {"measurement_ok": True}}])
        check("G8 runs[] 없는 레벨(반복 축 이전 산출물) → unrecorded · 완주 min None(추측 ✗)",
              s5["unrecorded_levels"] == [1] and s5["completed_min"] is None)
        try:
            aggregate_level(ldir, requested=3, requested_source="x", kind=REPEAT_KIND_COLD)
            check("G9 ★cold-restart 는 선언 슬롯만 — 집계 거부", False)
        except RepeatAxisError:
            check("G9 ★cold-restart 는 선언 슬롯만 — 집계 거부", True)

    # H — 멈춘 자리(clamp)와 반복 조건(판정점 범위 · 2026-09-14 리뷰 정정). 순수 입력으로 친다.
    def runs_(n_ok, n_fail=0, minute=0):
        out = []
        for k in range(1, n_ok + n_fail + 1):
            out.append({"run": k, "started_utc": "2026-01-01T01:%02d:00Z" % (minute + k),
                        "ended_utc": "2026-01-01T01:%02d:40Z" % (minute + k),
                        "run_bench_rc": 0, "parse_rc": 0, "measurement_ok": k <= n_ok, "decode_tps": 30.0})
        return out

    def lv(level, runs):
        return {"level": level, "measured": {"measurement_ok": True, "runs": runs}}
    clamp = {"level": 2, "run": 1, "started_utc": "2026-01-01T01:10:00Z", "ended_utc": "2026-01-01T01:10:20Z",
             "run_bench_rc": 3, "parse_rc": 0}
    s = summarize([lv(1, runs_(3))], clamp_run=clamp)
    check("H1 클램프(레벨 2 첫 run 실패) → stop.kind=clamp · 창 = 아래 레벨 마지막 run 시작 ~ 클램프 run 끝 · "
          "runs_attempted 에 클램프 run 1 포함",
          s["stop"] and s["stop"]["kind"] == STOP_KIND_CLAMP and s["stop"]["level"] == 2
          and s["stop"]["window_start_utc"] == "2026-01-01T01:03:00Z"
          and s["stop"]["window_end_utc"] == "2026-01-01T01:10:20Z" and s["runs_attempted"] == 4
          and s["clamp_run"] == clamp, s)
    s = summarize([lv(1, runs_(3))], clamp_run={"level": 2, "run": 1, "unreadable": "repeat_run.json 부재"})
    check("H2 클램프 경계 사실 판독 불가 → 창 None(대조 불가를 분류기가 이름으로 남긴다 · 추측 ✗)",
          s["stop"]["window_start_utc"] is None and "판독 불가" in s["stop"]["detail"], s["stop"])

    est, why = repetition_established(summarize([lv(1, runs_(3)), lv(2, runs_(3))]), 1)
    check("H3 판정점 3 · 경계 없음 → 반복 조건 성립", est is True and "완주 3" in why, why)
    est, why = repetition_established(summarize([lv(1, runs_(3)), lv(2, runs_(1, 1, 10))]), 1)
    check("H4 ★비대칭 교정: 경계 레벨 2 의 run 2 실패(판정점 3) → 성립(적응 상한 클램프 · 첫 run 실패와 같은 판정)",
          est is True and "적응 상한 클램프" in why, why)
    est, why = repetition_established(summarize([lv(1, runs_(3))], clamp_run=clamp), 1)
    check("H5 경계 레벨 2 의 첫 run 실패(판정점 3) → 성립(H4 와 같은 판정)", est is True, why)
    est, why = repetition_established(summarize([lv(1, runs_(1, 1))]), 1)
    check("H6 ★판정점 run 2 실패 → 불성립(False · 기계 이벤트)", est is False and "반복 중단" in why, why)
    est, why = repetition_established(summarize([lv(1, runs_(4, 1))]), 1)
    check("H7 판정점 요청 5 · run 5 실패(완주 4) → 성립(정의 충족 · 상위 레벨 미측정 기재)",
          est is True and "판정점 level 1 run 5" in why, why)
    est, why = repetition_established(summarize([lv(1, runs_(2))]), 1)
    check("H8 ★끊김 없이 판정점 2 → 판정 불가(요청 자체가 정의 미만 · 사유 발명 ✗)",
          est is None and why.startswith("unclassifiable("), why)
    est, why = repetition_established(summarize([lv(1, runs_(3)), lv(2, runs_(2)), lv(4, runs_(3))]), 1)
    check("H9 ★경계가 아닌 레벨 2 가 정의 미만(스윕은 멈추지 않았다) → 판정 불가(규칙 밖 산출물)",
          est is None and "레벨 [2]" in why, why)
    est, why = repetition_established(summarize([{"level": 1, "measured": {"measurement_ok": True}}]), 1)
    check("H10 runs[] 부재 → 판정 불가(not_evaluated)", est is None and why.startswith("not_evaluated("), why)
    est, why = repetition_established(summarize([lv(1, runs_(3))]), None)
    check("H11 ★판정점 레벨 부재(verdict_point_level 없음) → 판정 불가(1 을 가정하지 않는다)",
          est is None and "verdict_point_level" in why, why)

    if failures:
        sys.stderr.write("[repeat_axis --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[repeat_axis --self-test] OK — A1(schema 교차검증) · B1~B3 · P1~P2 · R1~R9 · G1~G9 · H1~H11 전부 통과")
    return 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="full bench 반복 축(해소·집계·요약)")
    ap.add_argument("--self-test", action="store_true")
    sub = ap.add_subparsers(dest="cmd")
    r = sub.add_parser("resolve", help="반복 수 해소 → 'N<US>출처<US>종류' 한 줄")
    r.add_argument("--repeats"); r.add_argument("--repeats-source"); r.add_argument("--campaign-id")
    rr = sub.add_parser("record-run", help="run 경계 사실 기록")
    rr.add_argument("--run-dir", required=True); rr.add_argument("--level", type=int, required=True)
    rr.add_argument("--run", type=int, required=True)
    rr.add_argument("--started-utc", required=True); rr.add_argument("--ended-utc", required=True)
    rr.add_argument("--run-bench-rc", type=int, required=True); rr.add_argument("--parse-rc", type=int, required=True)
    ag = sub.add_parser("aggregate-level", help="대표 run measured.json 에 runs[]·밴드를 덧붙인다")
    ag.add_argument("--level-dir", required=True); ag.add_argument("--requested", type=int, required=True)
    ag.add_argument("--requested-source", required=True); ag.add_argument("--kind", default=REPEAT_KIND_WARM)
    a = ap.parse_args(argv)
    if a.self_test:
        return _self_test()
    try:
        if a.cmd == "resolve":
            n, src = resolve_repeats(a.repeats, argv_source=a.repeats_source, campaign_id=a.campaign_id)
            print("\x1f".join((str(n), src.replace("\x1f", " "), REPEAT_KIND_WARM)))
            return 0
        if a.cmd == "record-run":
            print(record_run(Path(a.run_dir), level=a.level, run=a.run, started_utc=a.started_utc,
                             ended_utc=a.ended_utc, run_bench_rc=a.run_bench_rc, parse_rc=a.parse_rc))
            return 0
        if a.cmd == "aggregate-level":
            doc = aggregate_level(Path(a.level_dir), requested=a.requested,
                                  requested_source=a.requested_source, kind=a.kind)
            print("[repeat_axis] %s 완주 %d/%d · 밴드 %s (%s)"
                  % (a.level_dir, doc["repeats_completed"], a.requested, doc["repro_band_pct"],
                     doc["repro_band_source"]))
            return 0
    except RepeatAxisError as exc:
        sys.stderr.write("[repeat_axis] ERROR %s\n" % exc)
        return 2
    ap.print_help()
    return 2


if __name__ == "__main__":
    sys.exit(main())
