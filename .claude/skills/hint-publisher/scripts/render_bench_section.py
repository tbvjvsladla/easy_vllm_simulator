#!/usr/bin/env python3
"""render_bench_section.py — hint 태그 03-benchmark 절의 **동시성별 표**를 결정론으로 렌더한다.

왜 있는가(plan_26090616 H · 사용자 지시 2026-09-06):
    full 벤치는 동시성 레벨마다 측정한다. 그런데 인증서는 판정점 하나(`decode_tps_conc1`)만 싣고,
    hint 의 벤치 절은 인증서만 읽었다 — 그래서 **레벨별 곡선이 hint 에 한 번도 실린 적이 없다**.
    수신자는 "동시성 1에서 50 t/s" 만 보고 그 레시피가 부하에서 어떻게 되는지 알 수 없었다.

    수치는 **LLM 이 옮기지 않는다.** 이 스크립트가 벤치 산출물에서 파싱해 렌더하고, `--verify` 가
    발행된 절을 다시 렌더해 **diff 0** 을 요구한다. 손이 닿으면 그 자리에서 걸린다.

출처 우선순위(둘 다 결정론 · 어느 쪽을 썼는지 산출에 적는다 — 헌법 §결정론 규율):
    ① `--sweep-index <sweep_index.json>` — 가장 풍부(꼬리 p99·산포 std 포함). `output/` 평면이라
       캠페인이 끝나면 사라질 수 있다.
    ② `--report <bench_report_*.md>` — `docs/benchmark/` 에서 태어난 **바인딩된 증거**라 살아남는다.
       `render_report.py` 가 결정론으로 낸 표를 되읽는다(두 스크립트 사이의 형식이 계약이다).

    ③ 경량 리포트(`mode: lite` · 2026-09-14 · plan_26091407 §4.5) — 스윕 표가 **없다**(동시성 곡선을 재지 않았다).
       이때는 `## lite 지표` 표를 그대로 옮긴 절을 렌더한다(lite_metrics 가 이미 결정론으로 쓴 표 · 재산정 ✗).

  측정 구성 표(두 리포트 공통 · `render_report.py MEASUREMENT_CONFIG_TITLE`)는 `parse_measurement_config` 가 되읽는다 —
  bench_mode · 도구 · 반복 N · downgrade_reason 을 hint 동봉 문서·PAYLOAD·카탈로그로 나르는 **유일한 파서**다.
  lite 지표 존재 판정(`lite_metrics_present`)도 여기 하나다 — hint_tag(`HINT_MISSING_LITE` 요구)와 hint_collect(그 코드를
  적는 손)가 같은 술어를 쓴다(두 판정기가 갈리면 "적으라고 요구하는데 적는 손이 다른 기준을 쓴다" 가 된다).

  렌더: render_bench_section.py --report docs/benchmark/bench_report_....md
  대조: render_bench_section.py --verify --section <03-benchmark.md> --report <...>
  전수: render_bench_section.py --verify --all --hints-dir <dir>
  단위: render_bench_section.py --selftest
"""
from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path

SECTION_TITLE = "## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)"
# `render_report.py` 가 내는 표의 머리행. 이 문자열이 두 스크립트 사이의 **계약**이다 —
# 바뀌면 여기서 fail-loud 하고, 조용히 빈 표를 내지 않는다.
REPORT_HEADER = "| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |"
REPORT_TRUNC_MARK = "**⚠ 절삭된 부하 레벨"
REPORT_NO_TRUNC = "_절삭된 레벨 없음"
VERDICT_MARK = "★판정점"
# ── 경량 리포트 · 측정 구성 표 계약(2026-09-14 · plan_26091407 §4.5) ─────────────────────────────────────────────
#   writer = adversarial-benchmark `render_report.py`(`MEASUREMENT_CONFIG_TITLE`·`MEASUREMENT_CONFIG_KEYS` · `mode: lite` 헤더
#   · `## lite 지표` 절). 이 문자열들이 두 스크립트 사이의 계약이다 — 어긋나면 selftest_lite_report.py 가 실물 렌더로 잡는다.
MEASUREMENT_CONFIG_TITLE = "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)"
MEASUREMENT_CONFIG_KEYS = ("bench_mode", "bench_mode_kind", "bench_mode_source", "downgrade_reason",
                           "downgrade_reason_source", "bench_tool", "bench_tool_version", "bench_tool_version_source",
                           "repeats", "repeats_source", "repeats_completed")
_MEASUREMENT_INT_KEYS = ("repeats", "repeats_completed")
REPORT_LITE_MODE_LINE = "mode: lite"
REPORT_LITE_SECTION_PREFIX = "## lite 지표"
LITE_SECTION_TITLE = "## lite 지표 — 서빙 성공 직후 스냅샷 (결정론 파싱 · 손저작 ✗)"
# lite 지표 존재 술어의 두 조건 — 절이 있고 · gen tokens/sec 행에 **수치**가 있다(N/A 는 결손이다).
_LITE_GEN_TPS_ROW = re.compile(r"gen tokens/sec[^|]*\|\s*[0-9]")


class BenchSectionFailure(Exception):
    """파싱·대조 실패. 삼키지 않는다 — 빈 표는 '측정하지 않았다' 와 구분되지 않는다."""


def _num(text: str):
    """표 칸 하나 → 수 또는 None. `N/A`·빈칸은 **결측**이지 0 이 아니다."""
    text = (text or "").strip()
    if not text or text.upper() in ("N/A", "-", "—"):
        return None
    try:
        return float(text)
    except ValueError:
        return None


def _fmt(value, digits: int = 2) -> str:
    """표시 반올림. 원본을 바꾸지 않고 **표시만** 줄인다 — 자릿수는 아래 절 머리말에 밝힌다."""
    if value is None:
        return "N/A"
    if isinstance(value, (int,)) or (isinstance(value, float) and value.is_integer()):
        return str(int(value))
    return f"{round(float(value), digits):.{digits}f}"


def parse_report(text: str) -> dict:
    """`bench_report_*.md` 의 스윕 표 → 구조화. 머리행이 없으면 **FAIL**(빈 결과 ✗)."""
    lines = text.splitlines()
    try:
        head = next(i for i, ln in enumerate(lines) if ln.strip() == REPORT_HEADER)
    except StopIteration:
        raise BenchSectionFailure(
            "리포트에서 스윕 표 머리행을 찾지 못했다 — render_report.py 의 형식이 바뀌었거나 "
            "이 문서가 full 리포트가 아니다. 빈 표를 내지 않는다(형식 계약 위반은 fail-loud).")
    levels = []
    for ln in lines[head + 2:]:
        if not ln.strip().startswith("|"):
            break
        cells = [c.strip() for c in ln.strip().strip("|").split("|")]
        if len(cells) != 7:
            break
        conc_raw = cells[0]
        verdict_point = VERDICT_MARK in conc_raw
        conc = _num(conc_raw.replace(VERDICT_MARK, ""))
        done, _, failed = cells[6].partition("/")
        levels.append({
            "level": int(conc) if conc is not None else None,
            "verdict_point": verdict_point,
            "decode_tps": _num(cells[1]),
            "output_throughput": _num(cells[2]),
            "total_token_throughput": _num(cells[3]),
            "ttft_ms_median": _num(cells[4]),
            "itl_ms_median": _num(cells[5]),
            "completed": _num(done),
            "failed": _num(failed),
        })
    if not levels:
        raise BenchSectionFailure("스윕 표에 행이 하나도 없다 — 측정이 없었는지 파싱이 깨졌는지 "
                                  "구분되지 않으므로 통과시키지 않는다.")
    truncated = []
    in_trunc = False
    for ln in lines[head:]:
        if ln.startswith(REPORT_TRUNC_MARK):
            in_trunc = True
            continue
        if in_trunc:
            if ln.startswith("- "):
                truncated.append(ln[2:].strip())
            elif ln.strip():
                break
    return {"levels": levels, "truncated": truncated, "source_kind": "bench_report"}


def is_lite_report(text: str) -> bool:
    """경량 리포트인가 — 헤더 줄 `mode: lite` 가 **한 줄로** 서 있을 때만(본문 서술 속 문자열은 아니다)."""
    return any(ln.strip() == REPORT_LITE_MODE_LINE for ln in (text or "").splitlines())


def lite_metrics_present(text: str) -> bool:
    """리포트에 lite 정량지표가 **실측값으로** 실렸는가(`HINT_MISSING_LITE` 의 단일 술어).

    종전 hint_tag 의 두 인라인 검사(`'lite 지표' in text` · gen tokens/sec 행 수치)를 그대로 옮겼다 — 판정 기준은
    바뀌지 않았고 자리만 하나가 됐다. full 리포트(`## lite 지표 (full ⊇ lite …)`)와 경량 리포트 모두 같은 표를 싣는다."""
    text = text or ""
    return "lite 지표" in text and _LITE_GEN_TPS_ROW.search(text) is not None


def _unescape_cell(cell: str) -> str:
    return cell.replace("\\|", "|").strip()


def _split_row(line: str) -> list:
    """`| a | b\\|c |` → ['a', 'b|c'] — 이스케이프된 `|` 는 칸 구분자가 아니다."""
    body = line.strip()
    if body.startswith("|"):
        body = body[1:]
    if body.endswith("|") and not body.endswith("\\|"):
        body = body[:-1]
    return [_unescape_cell(c) for c in re.split(r"(?<!\\)\|", body)]


def parse_measurement_config(text: str) -> "dict | None":
    """리포트의 측정 구성 표 → {키: 값}. 절이 **없으면 None**(측정 구성 표 신설 전 리포트 — 부재는 미기재다).

    절은 있는데 표가 깨졌거나 모르는 키가 섞였으면 FAIL 이다(빈 dict 로 접으면 "기재 없음" 과 "파싱이 깨졌다" 가
    구분되지 않는다). 값 `N/A` 는 None · 반복 수는 정수로 읽는다."""
    lines = (text or "").splitlines()
    try:
        start = next(i for i, ln in enumerate(lines) if ln.strip() == MEASUREMENT_CONFIG_TITLE)
    except StopIteration:
        return None
    out: dict = {}
    seen_table = False
    for ln in lines[start + 1:]:
        stripped = ln.strip()
        if stripped.startswith("## "):
            break
        if not stripped.startswith("|"):
            if seen_table and not stripped:
                break
            continue
        cells = _split_row(stripped)
        if len(cells) != 2:
            raise BenchSectionFailure(f"측정 구성 표 행이 2칸이 아니다: {stripped!r}")
        key, value = cells
        if key in ("키", "---") or set(key) <= {"-"}:
            seen_table = True
            continue
        if key not in MEASUREMENT_CONFIG_KEYS:
            raise BenchSectionFailure(f"측정 구성 표에 계약 밖 키가 있다: {key!r} — render_report 와 계약이 갈라졌다")
        seen_table = True
        if value.upper() == "N/A" or value == "":
            out[key] = None
        elif key in _MEASUREMENT_INT_KEYS and re.fullmatch(r"-?[0-9]+", value):
            out[key] = int(value)
        else:
            out[key] = value
    if not out:
        raise BenchSectionFailure("측정 구성 절은 있는데 표 행이 하나도 없다 — 빈 기재로 접지 않는다")
    return out


def parse_lite_report(text: str) -> dict:
    """경량 리포트의 `## lite 지표` 표 → 구조화(행을 **그대로** 보존 · 수치 재가공 ✗). 표가 없으면 FAIL."""
    lines = (text or "").splitlines()
    try:
        start = next(i for i, ln in enumerate(lines) if ln.strip().startswith(REPORT_LITE_SECTION_PREFIX))
    except StopIteration:
        raise BenchSectionFailure("리포트에 `## lite 지표` 절이 없다 — 빈 lite 절을 내지 않는다.")
    table: list = []
    for ln in lines[start + 1:]:
        stripped = ln.strip()
        if stripped.startswith("## "):
            break
        if stripped.startswith("|"):
            table.append(stripped)
        elif table:
            break
    if len(table) < 3:
        raise BenchSectionFailure("`## lite 지표` 절에 표(머리행·구분행·값행)가 없다 — lite 산정 실패를 빈 표로 접지 않는다.")
    return {"table": table, "source_kind": "bench_report(lite)"}


def render_lite(parsed: dict, source_name: str) -> str:
    """경량 리포트 lite 표 → hint 절. `render` 와 같이 발행기·검증기가 공유하는 커널이다."""
    out = [LITE_SECTION_TITLE, "",
           "> 서빙 성공 직후의 **lite 스냅샷**이다 — cold 1회 + warm burst 1회. 동시성 곡선·반복·판정·인증서가 없다.",
           "> 성능 baseline 이 아니며(OBSERVATION-ONLY) 수신자는 자기 환경에서 재측정한다.",
           f"> 출처: `{source_name}` ({parsed['source_kind']}) · 표는 리포트의 행을 그대로 옮긴다(재산정 ✗ · `--verify` diff 0).",
           ""]
    out += list(parsed["table"])
    out.append("")
    return "\n".join(out)


def parse_sweep_index(doc: dict) -> dict:
    """`sweep_index.json` → 구조화(꼬리·산포 포함)."""
    levels = []
    vp = doc.get("verdict_point_level")
    for lv in doc.get("levels") or []:
        m = lv.get("measured") or {}
        levels.append({
            "level": lv.get("level"),
            "verdict_point": lv.get("level") == vp,
            "status": lv.get("status"),
            "decode_tps": m.get("decode_tps"),
            "output_throughput": m.get("output_throughput"),
            "total_token_throughput": m.get("total_token_throughput"),
            "ttft_ms_median": m.get("ttft_ms_median"),
            "ttft_ms_p99": m.get("ttft_ms_p99"),
            "itl_ms_median": m.get("itl_ms_median"),
            "itl_ms_p99": m.get("itl_ms_p99"),
            "tpot_ms_median": m.get("tpot_ms_median"),
            "completed": m.get("completed"),
            "failed": m.get("failed"),
            "error_rate": m.get("error_rate"),
            "measurement_ok": m.get("measurement_ok"),
        })
    if not levels:
        raise BenchSectionFailure("sweep_index 에 레벨이 없다 — 빈 표를 내지 않는다.")
    return {"levels": levels, "truncated": list(doc.get("truncated") or []),
            "source_kind": "sweep_index"}


def render(parsed: dict, source_name: str) -> str:
    """구조화 → 마크다운 절. **이 함수만이 표를 만든다**(발행기·검증기가 같은 커널을 쓴다)."""
    levels = parsed["levels"]
    has_tail = any(lv.get("ttft_ms_p99") is not None or lv.get("itl_ms_p99") is not None
                   for lv in levels)
    out = [SECTION_TITLE, "",
           "> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).",
           "> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.",
           f"> 출처: `{source_name}` ({parsed['source_kind']}) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).",
           "> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.",
           ""]
    if has_tail:
        out += ["| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50 | TTFT p99 | ITL p50 | ITL p99 | 완료/실패 |",
                "|---|---|---|---|---|---|---|---|---|"]
        for lv in levels:
            mark = " ★판정점" if lv.get("verdict_point") else ""
            out.append("| %s%s | %s | %s | %s | %s | %s | %s | %s | %s/%s |" % (
                lv.get("level"), mark, _fmt(lv.get("decode_tps")), _fmt(lv.get("output_throughput")),
                _fmt(lv.get("total_token_throughput")), _fmt(lv.get("ttft_ms_median")),
                _fmt(lv.get("ttft_ms_p99")), _fmt(lv.get("itl_ms_median")),
                _fmt(lv.get("itl_ms_p99")), _fmt(lv.get("completed")), _fmt(lv.get("failed"))))
    else:
        out += ["| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |",
                "|---|---|---|---|---|---|---|"]
        for lv in levels:
            mark = " ★판정점" if lv.get("verdict_point") else ""
            out.append("| %s%s | %s | %s | %s | %s | %s | %s/%s |" % (
                lv.get("level"), mark, _fmt(lv.get("decode_tps")), _fmt(lv.get("output_throughput")),
                _fmt(lv.get("total_token_throughput")), _fmt(lv.get("ttft_ms_median")),
                _fmt(lv.get("itl_ms_median")), _fmt(lv.get("completed")), _fmt(lv.get("failed"))))
    out.append("")
    if len(levels) == 1:
        out += ["> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. "
                "곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것).", ""]
    if parsed["truncated"]:
        out.append("**⚠ 절삭된 부하 레벨(조용히 자르지 않는다):**")
        out += [f"- {t}" for t in parsed["truncated"]]
        out.append("")
    else:
        out += ["_절삭된 레벨 없음(요청 전 레벨 완주)._", ""]
    return "\n".join(out)


def build_from_args(a) -> tuple[str, str]:
    if a.sweep_index:
        p = Path(a.sweep_index)
        return render(parse_sweep_index(json.loads(p.read_text(encoding="utf-8"))), p.name), p.name
    if a.report:
        p = Path(a.report)
        text = p.read_text(encoding="utf-8")
        if is_lite_report(text):
            return render_lite(parse_lite_report(text), p.name), p.name
        return render(parse_report(text), p.name), p.name
    raise BenchSectionFailure("--sweep-index 또는 --report 가 필요하다(출처 없이 표를 만들지 않는다)")


def extract_section(text: str, title: str = SECTION_TITLE) -> str | None:
    """발행된 문서에서 이 절만 잘라낸다(다음 `## ` 헤딩 직전까지). 경량 리포트 절은 `title=LITE_SECTION_TITLE`."""
    lines = text.splitlines()
    try:
        start = next(i for i, ln in enumerate(lines) if ln.strip() == title)
    except StopIteration:
        return None
    end = len(lines)
    for i in range(start + 1, len(lines)):
        if lines[i].startswith("## "):
            end = i
            break
    return "\n".join(lines[start:end]).rstrip() + "\n"


def _selftest() -> int:
    ok = True

    def ck(label, cond):
        nonlocal ok
        print(("  PASS " if cond else "  FAIL ") + label)
        ok = ok and bool(cond)

    report = "\n".join([
        "# 성능 보고서", "", "## 부하 스윕 곡선 (client-load · reload 없음)", "",
        REPORT_HEADER, "|---|---|---|---|---|---|---|",
        "| 1 ★판정점 | 50.58 | 50.69957426582275 | 253.6959165410896 | 210.4933261871338 | 19.02007589153215 | 16/0 |",
        "| 2 | 61.10 | 120.5 | 400.25 | 300.1 | 32.4 | 16/0 |", "",
        "**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**",
        "- level 4 truncated: parse/measurement_ok=false", "",
        "## 다음 절", ""])
    parsed = parse_report(report)
    ck("리포트 표 2행 파싱", len(parsed["levels"]) == 2)
    ck("판정점 표시가 값을 오염시키지 않는다", parsed["levels"][0]["level"] == 1 and parsed["levels"][0]["verdict_point"])
    ck("절삭 로그 회수", parsed["truncated"] == ["level 4 truncated: parse/measurement_ok=false"])
    body = render(parsed, "bench_report_x.md")
    ck("렌더에 두 레벨이 모두 있다", "| 1 ★판정점 |" in body and "| 2 |" in body)
    ck("표시 반올림(원본 오염 ✗)", "50.70" in body and "50.69957426582275" not in body)
    ck("절삭이 절에 남는다", "level 4 truncated" in body)

    raised = None
    try:
        parse_report("# 리포트\n본문뿐\n")
    except BenchSectionFailure as exc:
        raised = exc
    ck("★음성대조: 머리행 부재는 빈 표가 아니라 FAIL", raised is not None)
    raised = None
    try:
        parse_report("\n".join(["#", REPORT_HEADER, "|---|---|---|---|---|---|---|", "", ""]))
    except BenchSectionFailure as exc:
        raised = exc
    ck("★음성대조: 행 0 은 FAIL", raised is not None)

    idx = {"verdict_point_level": 1, "levels": [
        {"level": 1, "status": "ok", "measured": {"decode_tps": 50.58, "ttft_ms_median": 210.5,
                                                  "ttft_ms_p99": 217.6, "itl_ms_median": 19.0,
                                                  "itl_ms_p99": 19.15, "completed": 16, "failed": 0}}],
        "truncated": []}
    tail_body = render(parse_sweep_index(idx), "sweep_index.json")
    ck("sweep_index 는 꼬리 열(p99)을 낸다", "TTFT p99" in tail_body and "217.60" in tail_body)
    ck("레벨 1개면 곡선 부재를 명시한다", "레벨이 **하나뿐**" in tail_body)

    doc = "머리말\n\n" + body + "\n## 그다음\n내용\n"
    ck("절 추출이 왕복한다", extract_section(doc).strip() == body.strip())
    ck("절 부재는 None", extract_section("# 아무것도 없음\n") is None)
    tampered = doc.replace("50.58", "99.99")
    ck("★음성대조: 한 숫자만 손대도 diff 가 잡는다", extract_section(tampered).strip() != body.strip())
    # ── 경량 리포트 · 측정 구성 표 (2026-09-14 · plan_26091407 §4.5) ──────────────────────────────────────────────
    lite_doc = "\n".join([
        "# 경량 성능 보고서(lite)", "", REPORT_LITE_MODE_LINE, "", "> 생성일 2026-01-01T00:00:00Z.", "",
        MEASUREMENT_CONFIG_TITLE, "", "> 기재다.", "", "| 키 | 값 |", "|---|---|",
        "| bench_mode | lite |", "| bench_mode_kind | declared-lite |",
        "| bench_mode_source | declared(a \\| b) |", "| downgrade_reason | N/A |", "| repeats | 1 |", "",
        "## lite 지표 (서빙 성공 직후 스냅샷 · inform-only)", "", "> 설명", "",
        "| 메트릭 | 값 |", "|---|---|", "| gen tokens/sec (warm) | 26.00 t/s |", "| cold-start TTFT | 120 ms |", "",
        "## 측정 환경 스냅샷", ""])
    mc = parse_measurement_config(lite_doc)
    ck("측정 구성 표를 키→값으로 읽는다(N/A=None · 반복 정수 · 이스케이프 `|` 복원)",
       mc == {"bench_mode": "lite", "bench_mode_kind": "declared-lite", "bench_mode_source": "declared(a | b)",
              "downgrade_reason": None, "repeats": 1})
    ck("★측정 구성 절 부재는 None(미기재 — 파싱 실패와 다르다)", parse_measurement_config(report) is None)
    raised = None
    try:
        parse_measurement_config(lite_doc.replace("| repeats | 1 |", "| surprise | 1 |"))
    except BenchSectionFailure as exc:
        raised = exc
    ck("★음성대조: 계약 밖 키는 FAIL(계약이 갈라진 것을 조용히 버리지 않는다)", raised is not None)
    ck("경량 리포트 판별은 헤더 줄로만", is_lite_report(lite_doc) and not is_lite_report(report)
       and not is_lite_report("본문에 mode: lite 라는 말이 섞였을 뿐\n"))
    ck("lite 지표 술어: 수치 행이 있으면 참", lite_metrics_present(lite_doc))
    ck("★음성대조 lite 지표 술어: gen tokens/sec 가 N/A 면 거짓(결손)",
       not lite_metrics_present(lite_doc.replace("26.00 t/s", "N/A")))
    ck("★음성대조 lite 지표 술어: 절이 없으면 거짓", not lite_metrics_present(report))
    lite_body = render_lite(parse_lite_report(lite_doc), "bench_report_x.md")
    ck("lite 절이 표를 행 그대로 옮긴다", "| gen tokens/sec (warm) | 26.00 t/s |" in lite_body
       and lite_body.startswith(LITE_SECTION_TITLE))
    doc2 = "머리\n\n" + lite_body + "\n## 다음\n"
    ck("lite 절 추출이 왕복한다", extract_section(doc2, LITE_SECTION_TITLE).strip() == lite_body.strip())
    ck("★음성대조: lite 절 숫자 손댐을 diff 가 잡는다",
       extract_section(doc2.replace("26.00", "99.00"), LITE_SECTION_TITLE).strip() != lite_body.strip())
    raised = None
    try:
        parse_lite_report("# r\n\nmode: lite\n\n## lite 지표\n\n> 산정 실패\n")
    except BenchSectionFailure as exc:
        raised = exc
    ck("★음성대조: 표 없는 lite 절은 빈 절이 아니라 FAIL", raised is not None)

    print("[render_bench_section] " + ("PASS" if ok else "FAIL"))
    return 0 if ok else 1


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--report"); ap.add_argument("--sweep-index")
    ap.add_argument("--section", help="--verify: 대조할 발행 문서")
    ap.add_argument("--out", help="렌더 결과를 쓸 경로(생략 시 stdout)")
    ap.add_argument("--verify", action="store_true")
    ap.add_argument("--selftest", action="store_true")
    a = ap.parse_args(argv)
    try:
        if a.selftest:
            return _selftest()
        body, src = build_from_args(a)
        if a.verify:
            if not a.section:
                raise BenchSectionFailure("--verify 는 --section 이 필요하다")
            published = extract_section(Path(a.section).read_text(encoding="utf-8"),
                                        body.splitlines()[0].strip())
            if published is None:
                raise BenchSectionFailure(f"발행 문서에 벤치 절이 없다: {a.section}")
            if published.strip() != body.strip():
                import difflib
                diff = "\n".join(difflib.unified_diff(
                    body.splitlines(), published.strip().splitlines(),
                    fromfile="rendered", tofile="published", lineterm=""))
                raise BenchSectionFailure("발행된 벤치 절이 재렌더와 다르다 — 숫자가 손을 탔다:\n" + diff[:4000])
            print(f"[render_bench_section] VERIFY PASS diff=0 ({src})")
            return 0
        if a.out:
            Path(a.out).write_text(body, encoding="utf-8")
            print(f"[render_bench_section] wrote {a.out} ({src})")
        else:
            sys.stdout.write(body)
        return 0
    except (BenchSectionFailure, OSError, ValueError) as exc:
        print(f"[render_bench_section] FAIL {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
