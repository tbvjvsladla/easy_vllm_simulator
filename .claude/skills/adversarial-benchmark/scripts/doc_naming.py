# SPDX-License-Identifier: Apache-2.0
"""doc_naming.py — SSOT for benchmark publish-file naming (2026-07-25 canon).

시간 토큰 = YYMMDDHH (2-digit year · KST +0900, project convention).
_MM_SS 접미어 = 동일 YYMMDDHH 에 **다른 측정**이 이미 있을 때에만(충돌 시). 동일 측정 재발행은 덮어쓰기.
human report 접두어 = bench_report_ (구 report_ 아님). 인증서=benchmark_ · Max=max_envelope_.
정본: .claude/rules/docs.md §1. 소비자: render_report.py · publish_benchmark_record.py · render_max_report.py.
"""
import os as _os, re as _re, datetime as _dt

def kst_tokens(generated_utc):
    """'2026-07-24T16:22:29Z' -> ('26072501','22','29') KST(+0900). None/parse-fail -> ('NA','00','00')."""
    if not generated_utc:
        return ("NA", "00", "00")
    m = _re.match(r'(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z', str(generated_utc))
    if not m:
        return ("NA", "00", "00")
    d = _dt.datetime(*map(int, m.groups())) + _dt.timedelta(hours=9)
    return (d.strftime("%y%m%d%H"), d.strftime("%M"), d.strftime("%S"))

def _existing_ts(path):
    try:
        t = open(path, encoding="utf-8", errors="ignore").read()
    except Exception:
        return None
    m = _re.search(r'(?:measured_utc|generated_utc|생성일)\D*(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z)', t)
    return m.group(1) if m else None

def bench_filename(kind, meta, generated_utc, out_dir, ext):
    """kind in {'bench_report','benchmark','max_envelope'}. Returns basename.
    _MM_SS ONLY when out_dir already holds a base-named file of a DIFFERENT measurement."""
    yymmddhh, mm, ss = kst_tokens(generated_utc)
    combo = "%s_%s_%s" % (meta.get("model", "NA"), meta.get("gpu_key", "NA"), meta.get("vllm_version", "NA"))
    base = "%s_%s_%s.%s" % (kind, yymmddhh, combo, ext)
    if out_dir:
        bp = _os.path.join(out_dir, base)
        if _os.path.exists(bp) and _existing_ts(bp) not in (None, str(generated_utc)):
            base = "%s_%s_%s_%s_%s.%s" % (kind, yymmddhh, mm, ss, combo, ext)
    return base

def _self_test():
    import tempfile
    assert kst_tokens("2026-07-24T16:22:29Z") == ("26072501", "22", "29")
    assert kst_tokens("2026-07-15T11:00:33Z") == ("26071520", "00", "33")
    assert kst_tokens("2026-07-15T21:21:33Z") == ("26071606", "21", "33")
    assert kst_tokens(None) == ("NA", "00", "00")
    assert kst_tokens("garbage") == ("NA", "00", "00")
    d = tempfile.mkdtemp()
    meta = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}
    n1 = bench_filename("bench_report", meta, "2026-07-24T16:22:29Z", d, "md")
    assert n1 == "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md", n1
    open(_os.path.join(d, n1), "w").write("measured_utc: 2026-07-24T16:22:29Z\n")
    assert bench_filename("bench_report", meta, "2026-07-24T16:22:29Z", d, "md") == n1  # republish -> overwrite
    n2 = bench_filename("bench_report", meta, "2026-07-24T16:45:00Z", d, "md")          # diff measurement, same hour
    assert n2 == "bench_report_26072501_45_00_solar-open2-250b_GB10_0.22.0.md", n2
    c = bench_filename("benchmark", meta, "2026-07-24T16:22:29Z", None, "yaml")
    assert c == "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml", c
    mx = bench_filename("max_envelope", meta, None, None, "md")
    assert mx == "max_envelope_NA_solar-open2-250b_GB10_0.22.0.md", mx
    print("[doc_naming] self-test PASS")

if __name__ == "__main__":
    _self_test()
