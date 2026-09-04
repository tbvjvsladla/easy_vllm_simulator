#!/usr/bin/env python3
"""doc_naming.py -- SSOT for docs/ publication naming (plan_26072506 Phase 2, canon = 2026-07-25).

This is the naming helper `.claude/rules/docs.md` §1 declares as the naming SSOT. Consumed by
`.claude/policies/runtime/evidence_publisher.py` so no second, hand-duplicated naming implementation exists inside
the publisher.

Canon (see .claude/rules/docs.md):
  - time token = YYMMDDHH, 2-digit year, KST (+0900). Computed from an explicit UTC
    'YYYY-MM-DDTHH:MM:SSZ' timestamp the CALLER supplies -- this module makes no wall-clock or
    randomness call anywhere, so every function here is a pure, deterministic mapping from its
    arguments to a result (safe for hermetic, flake-free tests and for exact reproducibility).
  - `_MM_SS` suffix is added ONLY when the computed base name already collides with one already
    claimed by a DIFFERENT publication in the same directory (same YYMMDDHH, same type) -- the
    caller supplies that existing-name set explicitly (`existing_basenames`/`existing_dirnames`);
    this module never touches the filesystem itself to discover collisions, keeping it pure.
    Republishing the SAME unit is an idempotency concern the caller (evidence_publisher.py) owns
    by persisting the name it already computed -- not by re-deriving a fresh, possibly different
    _MM_SS-suffixed name on every rerun.
  - plan/devlog/testlog/request: `docs/<type>/<type>_<YYMMDDHH>[_MM_SS]_<topic>.md`
    (`request` = 사람 수행지시서. 발행 시점이 고정된 작업지시라 report 와 달리 날짜 토큰을 쓴다.
    evidence chain 밖이므로 evidence_publisher 의 scaffold/completion 게이트는 타지 않는다.)
  - simlog: `docs/simlog/<YYMMDDHH>[_MM_SS]_<topic>/` -- a RUN DIRECTORY, not a file, so no
    `<type>_` prefix.
  - benchmark (**재수출** — 정본은 adversarial-benchmark/scripts/doc_naming.py):
    `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md` (human,
    always published) + `docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml`
    (certificate, PASS-only) + `docs/benchmark/max_envelope_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`.
  - report/ is the ONE exception: no date token at all -- `docs/report/<kebab-slug>.<ext>`,
    updated in place (README-like; the file identity IS the slug, there is no "same slug, new
    publication" concept for this doc type).

stdlib only, no third-party dependencies.
"""
from __future__ import annotations

import argparse
import importlib.util
import re
import sys
from pathlib import Path

DATED_DOC_TYPES = ("plan", "devlog", "testlog", "request", "checklist")

_TOPIC_RE = re.compile(r"^[A-Za-z0-9_.가-힣]+$")
_KEBAB_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")

# ── 벤치 명명(kst_tokens · gpu_key · bench_filename · scan_bench_dir · 예외 2종)은 **재수출**이다 ──
# 정본은 `.claude/skills/adversarial-benchmark/scripts/doc_naming.py` (plan_26090410 P0.5b).
# 왜 거기인가: 그 모듈은 런타임블럭이라 **서브 노드에도 배달**되고, 이 파일(wiki-desk)은 메인 전용이라
# 서브에서 import 할 수 없다. 두 사본이 서로 다른 충돌 규칙을 가져 같은 측정이 두 이름을 갖던 결함
# (감사 D-1)을 "한 파일 + 재수출" 로 끊는다. 이 파일은 산문 문서(plan/devlog/testlog/request/checklist ·
# simlog · report) 명명만 직접 소유한다. 재수출 대상이 없으면 **fail-loud** — 조용한 대체 구현 금지.
_BENCH_NAMING_PATH = (Path(__file__).resolve().parents[2] / "adversarial-benchmark" / "scripts" / "doc_naming.py")


def _load_bench_naming():
    if not _BENCH_NAMING_PATH.is_file():
        raise ImportError(
            "doc_naming: bench naming SSOT missing at %s -- wiki-desk re-exports it and must not "
            "substitute its own copy (plan_26090410 P0.5b)" % _BENCH_NAMING_PATH)
    spec = importlib.util.spec_from_file_location("_bench_doc_naming", _BENCH_NAMING_PATH)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_bench = _load_bench_naming()
kst_tokens = _bench.kst_tokens
gpu_key = _bench.gpu_key
bench_filename = _bench.bench_filename
scan_bench_dir = _bench.scan_bench_dir
BENCH_KIND_DEFAULT_EXT = _bench.BENCH_KIND_DEFAULT_EXT
NamingCollisionExhausted = _bench.NamingCollisionExhausted   # 산문 명명도 같은 예외 클래스를 쓴다(두 클래스 ✗)
NamingSourceUnreadable = _bench.NamingSourceUnreadable


def validate_topic(topic):
    """Raises ValueError unless `topic` is a non-empty run of [A-Za-z0-9_. 한글] with no path
    separators, whitespace, or the special '.'/'..' segments. Dots are allowed (project
    convention embeds version numbers in topics, e.g. 'vLLM0.22.1_KV클램프_시뮬') but the topic
    may never itself BE exactly '.' or '..' (defense in depth -- neither can occur as a real path
    traversal here since topic is always embedded inside a longer '<prefix>_<topic>.<ext>'
    string, never used as a standalone path component, but rejecting the literal degenerate
    values costs nothing and removes any doubt)."""
    if not topic or not _TOPIC_RE.match(topic) or topic in (".", ".."):
        raise ValueError(
            "doc_naming: topic must be a non-empty run of [A-Za-z0-9_. 한글] with no path "
            "separators/whitespace, got %r" % (topic,)
        )


_DATED_DOC_RE = re.compile(r"^(?P<type>[a-z]+)_(?P<hour>\d{8})(?:_\d{2}_\d{2})?_(?P<topic>.+)\.md\Z")


def is_dated_doc_basename(doc_type, basename):
    """`<type>_<YYMMDDHH>[_MM_SS]_<topic>.md` 규약 이름인가(= 이 SSOT 가 만들었을 수 있는 이름). 발행기가
    기존 문서를 **바인딩**할지(규약 이름 · docs/<type>/ 아래) 스캐폴드에 **주입**할지(임시 위치) 가르는
    술어 — 규약을 여기 한 곳이 소유한다(plan_26090410 P5)."""
    if doc_type not in DATED_DOC_TYPES or not basename or "/" in basename:
        return False
    m = _DATED_DOC_RE.match(basename)
    return bool(m) and m.group("type") == doc_type


def dated_doc_basename(doc_type, generated_utc, topic, existing_basenames=(), ext="md"):
    """plan/devlog/testlog: '<type>_<YYMMDDHH>[_MM_SS]_<topic>.<ext>'. Collision is keyed on
    (doc_type, hour) ALONE -- .claude/rules/docs.md 's canon is "충돌 시에만 _MM_SS: 같은
    YYMMDDHH(같은 type)에 2건 이상일 때" (same type, same hour, REGARDLESS of topic), not an
    exact-basename match. So ANY existing basename sharing this doc_type+hour prefix forces the
    new, distinct-topic publication onto its own _MM_SS form too (P2-A04). Raises
    NamingCollisionExhausted if BOTH the unsuffixed and _MM_SS forms for THIS topic are already
    occupied -- canon permits no third disambiguator, so this never invents one nor returns an
    occupied name."""
    if doc_type not in DATED_DOC_TYPES:
        raise ValueError(
            "dated_doc_basename: doc_type must be one of %r, got %r" % (DATED_DOC_TYPES, doc_type)
        )
    validate_topic(topic)
    yymmddhh, mm, ss = kst_tokens(generated_utc)
    existing = set(existing_basenames)
    prefix = "%s_%s_" % (doc_type, yymmddhh)
    base = "%s_%s_%s.%s" % (doc_type, yymmddhh, topic, ext)
    if not any(name.startswith(prefix) for name in existing):
        return base
    suffixed = "%s_%s_%s_%s_%s.%s" % (doc_type, yymmddhh, mm, ss, topic, ext)
    if suffixed in existing:
        raise NamingCollisionExhausted(
            "dated_doc_basename: both %r and %r are already occupied by another publication -- "
            "canon permits only unsuffixed or a single _MM_SS suffix" % (base, suffixed))
    return suffixed


def simlog_dirname(generated_utc, topic, existing_dirnames=()):
    """simlog: '<YYMMDDHH>[_MM_SS]_<topic>' -- a run DIRECTORY name, no '<type>_' prefix.
    Collision is keyed on the hour ALONE (P2-A04, same rationale as dated_doc_basename) -- ANY
    existing run directory in this hour forces the new one onto _MM_SS too, regardless of topic.
    Raises NamingCollisionExhausted if both forms for THIS topic are already occupied (see
    dated_doc_basename)."""
    validate_topic(topic)
    yymmddhh, mm, ss = kst_tokens(generated_utc)
    existing = set(existing_dirnames)
    prefix = "%s_" % yymmddhh
    base = "%s_%s" % (yymmddhh, topic)
    if not any(name.startswith(prefix) for name in existing):
        return base
    suffixed = "%s_%s_%s_%s" % (yymmddhh, mm, ss, topic)
    if suffixed in existing:
        raise NamingCollisionExhausted(
            "simlog_dirname: both %r and %r are already occupied by another publication -- "
            "canon permits only unsuffixed or a single _MM_SS suffix" % (base, suffixed))
    return suffixed


_REPORT_RE = re.compile(r"^[a-z][a-z0-9]*_\d{8}_.+\Z")


def report_basename(name, ext="html"):
    """docs/report/ — <분류>_<YYMMDDHH>[_MM_SS]_<한글제목>.<ext> (2026-08-24 개정: kebab 무날짜 규약 폐기).
    분류 = 짧은 영문 키워드(perf|harness|audit|example ...), timestamp = KST YYMMDDHH, 한글제목 = 간결한
    한국어 `_` slug(모델명 등 proper noun 은 원형 유지). 발행 시점 고정 — 같은 주제 재발행은 새 문서."""
    if not name or "/" in name or "\\" in name or not _REPORT_RE.match(name):
        raise ValueError("report_basename: name must be <분류>_<YYMMDDHH>[_MM_SS]_<한글제목> "
                         "(분류=영문 키워드, 예: perf_26082421_qwen3.8-27b_레시피별_성능), got %r" % (name,))
    return "%s.%s" % (name, ext)


def _require(condition, message):
    if not condition:
        raise AssertionError(message)


def _self_test():
    _require(kst_tokens("2026-07-24T16:22:29Z") == ("26072501", "22", "29"), "KST conversion 1")
    _require(kst_tokens("2026-07-15T21:21:33Z") == ("26071606", "21", "33"), "KST conversion 2")
    _require(kst_tokens(None) == ("NA", "00", "00"), "None timestamp fallback")
    _require(kst_tokens("garbage") == ("NA", "00", "00"), "invalid timestamp fallback")
    _require(kst_tokens("2026-99-99T99:99:99Z") == ("NA", "00", "00"), "invalid calendar fallback")

    try:
        dated_doc_basename("testlog", "2026-07-25T06:58:29Z", "구조개선",
                            existing_basenames={"testlog_26072515_구조개선.md",
                                                 "testlog_26072515_58_29_구조개선.md"})
        raise AssertionError("expected NamingCollisionExhausted for a double-occupied name")
    except NamingCollisionExhausted:
        pass

    n1 = dated_doc_basename("plan", "2026-07-25T06:58:00Z", "하네스_루프_구조개선")
    _require(n1 == "plan_26072515_하네스_루프_구조개선.md", n1)
    n2 = dated_doc_basename("testlog", "2026-07-25T06:58:29Z", "구조개선",
                            existing_basenames={"testlog_26072515_구조개선.md"})
    _require(n2 == "testlog_26072515_58_29_구조개선.md", n2)
    _require(is_dated_doc_basename("plan", "plan_26072515_하네스_루프_구조개선.md")
             and is_dated_doc_basename("testlog", "testlog_26072515_58_29_구조개선.md")
             and not is_dated_doc_basename("plan", "testlog_26072515_구조개선.md")
             and not is_dated_doc_basename("plan", "plan_2607251_x.md")
             and not is_dated_doc_basename("plan", "notes.md")
             and not is_dated_doc_basename("report", "perf_26082421_x.md"),
             "is_dated_doc_basename 규약 판정")
    ck = dated_doc_basename("checklist", "2026-07-25T06:58:00Z", "감사_마스터")
    _require(ck == "checklist_26072515_감사_마스터.md", ck)

    d1 = simlog_dirname("2026-06-21T12:21:00Z", "vLLM0.22.1_KV클램프_시뮬")
    _require(d1 == "26062121_vLLM0.22.1_KV클램프_시뮬", d1)

    # 벤치 명명은 재수출 — 정본 모듈의 객체 **그 자체**여야 한다(사본이면 다시 갈라진다)
    _require(bench_filename is _bench.bench_filename and gpu_key is _bench.gpu_key
             and kst_tokens is _bench.kst_tokens and NamingCollisionExhausted is _bench.NamingCollisionExhausted,
             "wiki-desk must re-export the bench naming SSOT, not copy it")
    _require(gpu_key("NVIDIA GB10") == "GB10", "gpu_key re-export")
    meta = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}
    _require(bench_filename("benchmark", meta, "2026-07-24T16:22:29Z")
             == "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml", "bench_filename re-export")

    r1 = report_basename("perf_26082421_qwen3.8-27b_레시피별_성능")
    _require(r1 == "perf_26082421_qwen3.8-27b_레시피별_성능.html", r1)
    r2 = report_basename("harness_26082218_08_32_노드_정체성_토폴로지_그라운딩")
    _require(r2 == "harness_26082218_08_32_노드_정체성_토폴로지_그라운딩.html", r2)
    try:
        report_basename("rtxpro6000-benchmark-explorer")  # 무날짜 kebab → 신규 규칙 위반
        raise AssertionError("expected ValueError for undated kebab slug")
    except ValueError:
        pass

    print("[doc_naming] self-test PASS")


def main(argv=None):
    ap = argparse.ArgumentParser(description="doc_naming.py -- docs/ publication naming SSOT")
    ap.add_argument("--self-test", action="store_true", help="run the built-in deterministic self-test")
    args = ap.parse_args(argv)
    if args.self_test:
        _self_test()
        return
    ap.print_help(sys.stderr)
    raise SystemExit(2)


if __name__ == "__main__":
    main()
