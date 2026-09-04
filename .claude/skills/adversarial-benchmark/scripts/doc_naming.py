# SPDX-License-Identifier: Apache-2.0
"""doc_naming.py — 벤치 산출물(bench_report / benchmark / max_envelope) **명명 SSOT**.

왜 이 파일이 여기(런타임블럭) 있는가 — 2026-09-04 plan_26090410 P0.5:
  이 모듈은 `adversarial-benchmark` 스킬에 속해 **서브 노드에도 배달**된다. wiki-desk 는 메인 전용
  사서라 서브에서 import 할 수 없다. 그래서 벤치 명명의 단일 소유는 여기고, 메인의
  `.claude/skills/wiki-desk/scripts/doc_naming.py` 는 이 파일을 **재수출**한다(두 사본 ✗).
  종전에는 두 사본이 서로 다른 충돌 규칙(여기: measured_utc 기반 / wiki-desk: 시간대 prefix 기반)을
  가져 같은 측정이 두 이름을 갖는 것이 구조적으로 보장됐다(감사 D-1).

규약(.claude/rules/docs.md §명명 SSOT):
  시간 토큰 = YYMMDDHH(2자리 연도 · KST +0900) · `_MM_SS` 는 **같은 시간대에 다른 측정**이 이미
  base 이름을 점유했을 때만 · 같은 측정의 재발행은 덮어쓰기(멱등).
  "같은 측정" 판정은 기존 파일 **본문의 measured_utc/generated_utc** 로 한다 — 이름이 아니라 내용.
  읽을 수 없으면 같은 측정으로 **간주하지 않는다**(fail-closed · 종전 `except: None` 은 무경고
  덮어쓰기 경로였다 — 감사 A-3/D-1).

순수성: `bench_filename` 은 파일시스템을 만지지 않는다 — 기존 파일의 측정 시각 매핑을 인자로 받는다.
IO 는 `scan_bench_dir` 하나에만 있다(호출부가 명시적으로 부른다). 벽시계·난수 호출 없음.
"""
from __future__ import annotations

import datetime as _dt
import os as _os
import re as _re

BENCH_KIND_DEFAULT_EXT = {"bench_report": "md", "benchmark": "yaml", "max_envelope": "md"}

_UTC_RE = _re.compile(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$")
# 기존 파일에서 "이 파일이 어느 측정인가" 를 읽는 패턴 — 인증서 `measured_utc:` · sweep_index/report 의
# `generated_utc` · 사람용 report 본문의 `생성일`. 따옴표는 `\D*` 가 삼킨다.
_MEASUREMENT_TS_RE = _re.compile(
    r"(?:measured_utc|generated_utc|생성일)\D*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)")


class NamingCollisionExhausted(ValueError):
    """base 와 단일 `_MM_SS` 접미 후보가 **둘 다 다른 측정**에 점유됐다. 규약은 이 두 형태만 허용하므로
    호출부는 새 접미를 발명하지 말고 fail-closed 해야 한다."""


class NamingSourceUnreadable(ValueError):
    """충돌 후보 파일의 측정 시각을 읽지 못했다 — "같은 측정인가" 를 판정할 수 없으므로 덮어쓰지도
    접미를 붙이지도 않는다(침묵 덮어쓰기 금지)."""


def kst_tokens(generated_utc):
    """UTC 'YYYY-MM-DDTHH:MM:SSZ' -> (YYMMDDHH, MM, SS) KST(+0900). None · 형태 불일치 · 달력에 없는
    날짜('2026-99-99T…')는 전부 ('NA','00','00') 센티널 — 어떤 문자열 입력에도 raise 하지 않는다."""
    if not generated_utc:
        return ("NA", "00", "00")
    m = _UTC_RE.match(str(generated_utc))
    if not m:
        return ("NA", "00", "00")
    try:
        d = _dt.datetime(*map(int, m.groups())) + _dt.timedelta(hours=9)
    except (ValueError, OverflowError):
        return ("NA", "00", "00")
    return (d.strftime("%y%m%d%H"), d.strftime("%M"), d.strftime("%S"))


def gpu_key(gpu_model):
    """자유텍스트 GPU 모델명 → 파일명 안전 키. "NVIDIA GB10" → "GB10". 부재 → "NA".

    호출부마다 정규화하면 같은 측정이 두 이름을 갖는다(2026-08-24 · 09-04 재발: 인증서 파일명에
    공백). 규칙은 여기 한 곳이다."""
    if not gpu_model:
        return "NA"
    return _re.sub(r"[^A-Za-z0-9]", "", str(gpu_model).replace("NVIDIA", "")) or "NA"


def measurement_ts(text):
    """파일 본문에서 측정 시각 문자열을 뽑는다. 없으면 None."""
    m = _MEASUREMENT_TS_RE.search(text or "")
    return m.group(1) if m else None


def scan_bench_dir(out_dir, kind=None):
    """{basename: 측정시각 | None} — 이 모듈의 **유일한 IO**. `kind` 를 주면 그 접두 파일만.
    읽기 실패·시각 부재는 None 으로 **표시**한다(삼키지 않는다) — 그 이름이 충돌 후보가 되는 순간
    `bench_filename` 이 NamingSourceUnreadable 로 죽는다."""
    out: dict = {}
    if not out_dir or not _os.path.isdir(out_dir):
        return out
    for name in _os.listdir(out_dir):
        if kind and not name.startswith(kind + "_"):
            continue
        p = _os.path.join(out_dir, name)
        if not _os.path.isfile(p):
            continue
        try:
            with open(p, encoding="utf-8", errors="ignore") as f:
                out[name] = measurement_ts(f.read())
        except OSError:
            out[name] = None
    return out


def bench_filename(kind, meta, generated_utc, existing=None, ext=None):
    """kind ∈ {bench_report, benchmark, max_envelope}. `meta` = model/gpu_key/vllm_version(부재는
    리터럴 'NA' — 합성하지 않는다). `existing` = `scan_bench_dir()` 결과(또는 None = stdout 모드).

    규칙: base 가 비어 있으면 base · base 가 **같은 측정**이면 base(멱등 덮어쓰기) · 다른 측정이면
    `_MM_SS` · 그 접미 이름도 다른 측정이면 NamingCollisionExhausted · 후보의 측정 시각을 못 읽으면
    NamingSourceUnreadable."""
    if kind not in BENCH_KIND_DEFAULT_EXT:
        raise ValueError("bench_filename: kind must be one of %r, got %r"
                         % (tuple(BENCH_KIND_DEFAULT_EXT), kind))
    ext = ext or BENCH_KIND_DEFAULT_EXT[kind]
    meta = meta or {}
    yymmddhh, mm, ss = kst_tokens(generated_utc)
    combo = "%s_%s_%s" % (meta.get("model", "NA"), meta.get("gpu_key", "NA"), meta.get("vllm_version", "NA"))
    base = "%s_%s_%s.%s" % (kind, yymmddhh, combo, ext)
    if not existing or base not in existing:
        return base
    ts = existing[base]
    if ts is None:
        raise NamingSourceUnreadable(
            "bench_filename: %r exists but its measurement timestamp is unreadable — cannot decide "
            "same-vs-different measurement, refusing to overwrite or suffix" % base)
    if ts == str(generated_utc):
        return base
    suffixed = "%s_%s_%s_%s_%s.%s" % (kind, yymmddhh, mm, ss, combo, ext)
    if suffixed not in existing:
        return suffixed
    ts2 = existing[suffixed]
    if ts2 is None:
        raise NamingSourceUnreadable(
            "bench_filename: %r exists but its measurement timestamp is unreadable" % suffixed)
    if ts2 == str(generated_utc):
        return suffixed
    raise NamingCollisionExhausted(
        "bench_filename: both %r and %r are occupied by OTHER measurements (%s, %s) — canon permits "
        "only unsuffixed or a single _MM_SS suffix" % (base, suffixed, ts, ts2))


def _require(condition, message):
    if not condition:
        raise AssertionError(message)


def _self_test():
    import tempfile
    _require(kst_tokens("2026-07-24T16:22:29Z") == ("26072501", "22", "29"), "KST conversion 1")
    _require(kst_tokens("2026-07-15T11:00:33Z") == ("26071520", "00", "33"), "KST conversion 2")
    _require(kst_tokens("2026-07-15T21:21:33Z") == ("26071606", "21", "33"), "KST conversion 3")
    _require(kst_tokens(None) == ("NA", "00", "00"), "None timestamp fallback")
    _require(kst_tokens("garbage") == ("NA", "00", "00"), "invalid timestamp fallback")
    _require(kst_tokens("2026-99-99T99:99:99Z") == ("NA", "00", "00"), "invalid calendar fallback (no raise)")
    for a, b in (("NVIDIA GB10", "GB10"), ("NVIDIA H100 80GB", "H10080GB"), ("", "NA"), (None, "NA"), ("GB10", "GB10")):
        _require(gpu_key(a) == b, "gpu_key %r -> %r" % (a, gpu_key(a)))

    meta = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}
    T1, T2, T3 = "2026-07-24T16:22:29Z", "2026-07-24T16:45:00Z", "2026-07-24T16:50:00Z"
    base = "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md"
    sfx2 = "bench_report_26072501_45_00_solar-open2-250b_GB10_0.22.0.md"
    _require(bench_filename("bench_report", meta, T1) == base, "stdout 모드(existing 없음) = base")
    _require(bench_filename("bench_report", meta, T1, {}) == base, "빈 디렉터리 = base")
    _require(bench_filename("bench_report", meta, T1, {base: T1}) == base, "같은 측정 재발행 = 덮어쓰기(멱등)")
    _require(bench_filename("bench_report", meta, T2, {base: T1}) == sfx2, "다른 측정 같은 시간대 = _MM_SS")
    _require(bench_filename("bench_report", meta, T2, {base: T1, sfx2: T2}) == sfx2, "접미 이름의 같은 측정 재발행 = 멱등")
    _require(bench_filename("bench_report", meta, T3, {base: T1, sfx2: T2})
             == "bench_report_26072501_50_00_solar-open2-250b_GB10_0.22.0.md",
             "세 번째 측정은 자기 MM_SS 로 간다(접미는 새 측정의 시각에서 파생)")
    try:
        # 접미 이름이 **자기 이름과 다른 측정**을 담고 있는 비정합 상태 — 규약의 두 형태가 모두 막힘
        bench_filename("bench_report", meta, T2, {base: T1, sfx2: "2026-01-01T00:00:00Z"})
        raise AssertionError("두 형태 모두 다른 측정에 점유 → NamingCollisionExhausted 기대")
    except NamingCollisionExhausted:
        pass
    try:
        bench_filename("bench_report", meta, T2, {base: None})
        raise AssertionError("★음성대조 측정시각을 못 읽는 후보는 덮어쓰기/접미가 아니라 raise")
    except NamingSourceUnreadable:
        pass
    _require(bench_filename("benchmark", meta, T1) == "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml", "cert ext")
    _require(bench_filename("max_envelope", meta, None) == "max_envelope_NA_solar-open2-250b_GB10_0.22.0.md", "NA token")
    try:
        bench_filename("nope", meta, T1)
        raise AssertionError("unknown kind must raise")
    except ValueError:
        pass

    d = tempfile.mkdtemp()
    with open(_os.path.join(d, base), "w", encoding="utf-8") as f:
        f.write("# report\n생성일 2026-07-24T16:22:29Z.\n")
    with open(_os.path.join(d, "benchmark_x.yaml"), "w", encoding="utf-8") as f:
        f.write('measured_utc: "2026-07-24T16:22:29Z"\n')
    with open(_os.path.join(d, "bench_report_no_ts.md"), "w", encoding="utf-8") as f:
        f.write("nothing here\n")
    scanned = scan_bench_dir(d)
    _require(scanned[base] == T1 and scanned["benchmark_x.yaml"] == T1, "scan 이 본문에서 측정시각을 뽑는다(따옴표 포함)")
    _require(scanned["bench_report_no_ts.md"] is None, "시각 없는 파일은 None 으로 **표시**")
    _require(set(scan_bench_dir(d, "benchmark")) == {"benchmark_x.yaml"}, "kind 필터")
    _require(scan_bench_dir(_os.path.join(d, "absent")) == {}, "부재 디렉터리 = {}")
    print("[doc_naming] self-test PASS")


if __name__ == "__main__":
    _self_test()
