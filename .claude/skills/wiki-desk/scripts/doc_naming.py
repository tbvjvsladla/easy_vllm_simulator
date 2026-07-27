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
  - plan/devlog/testlog: `docs/<type>/<type>_<YYMMDDHH>[_MM_SS]_<topic>.md`
  - simlog: `docs/simlog/<YYMMDDHH>[_MM_SS]_<topic>/` -- a RUN DIRECTORY, not a file, so no
    `<type>_` prefix.
  - benchmark: `docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md` (human,
    always published) + `docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml`
    (certificate, PASS-only) + `docs/benchmark/max_envelope_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md`.
  - report/ is the ONE exception: no date token at all -- `docs/report/<kebab-slug>.<ext>`,
    updated in place (README-like; the file identity IS the slug, there is no "same slug, new
    publication" concept for this doc type).

stdlib only, no third-party dependencies.
"""
from __future__ import annotations

import argparse
import datetime
import re
import sys

DATED_DOC_TYPES = ("plan", "devlog", "testlog")
BENCH_KIND_DEFAULT_EXT = {"bench_report": "md", "benchmark": "yaml", "max_envelope": "md"}

_UTC_RE = re.compile(r"^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2}):(\d{2})Z$")
_TOPIC_RE = re.compile(r"^[A-Za-z0-9_.가-힣]+$")
_KEBAB_RE = re.compile(r"^[a-z0-9]+(-[a-z0-9]+)*$")


class NamingCollisionExhausted(ValueError):
    """Raised when BOTH the unsuffixed and the single _MM_SS-suffixed candidate name/dirname are
    already occupied by a DIFFERENT publication. Canon (.claude/rules/docs.md §1) permits only
    these two forms -- there is no third disambiguator -- so a caller hitting this must fail
    closed (surface a stable error) rather than have this module invent a new suffix scheme or,
    worse, silently return an already-occupied name that the caller then overwrites."""


def kst_tokens(generated_utc):
    """UTC 'YYYY-MM-DDTHH:MM:SSZ' -> (YYMMDDHH, MM, SS) KST(+0900). None, shape-mismatched, AND
    shape-matched-but-not-a-real-calendar-date (e.g. '2026-99-99T99:99:99Z') all fall back to the
    same ('NA','00','00') sentinel -- this function never raises for any string input, matching
    its documented 'unparseable -> NA' contract (N/A fail-soft, docs.md's own convention for
    optional/best-effort timestamp consumers such as bench certificate meta). Callers that treat a
    REQUIRED timestamp argument as invalid user input when this sentinel comes back (e.g.
    evidence_publisher.py's CLI --generated-utc/--recorded-utc) are responsible for that rejection
    themselves -- it is not this pure library's job to decide what's "required" for any given
    caller."""
    if not generated_utc:
        return ("NA", "00", "00")
    m = _UTC_RE.match(str(generated_utc))
    if not m:
        return ("NA", "00", "00")
    try:
        dt = datetime.datetime(*map(int, m.groups())) + datetime.timedelta(hours=9)
    except (ValueError, OverflowError):
        return ("NA", "00", "00")
    return (dt.strftime("%y%m%d%H"), dt.strftime("%M"), dt.strftime("%S"))


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


def bench_filename(kind, meta, generated_utc, existing_basenames=(), ext=None):
    """kind in {'bench_report','benchmark','max_envelope'}. `meta` needs model/gpu_key/vllm_version
    (N/A fail-soft: missing keys render as the literal 'NA' combo segment, never fabricated).
    Collision is keyed on (kind, hour) ALONE (P2-A04) -- docs.md: "동일 측정 재발행=덮어쓰기 ·
    동일 YYMMDDHH 다른 측정=_MM_SS" -- ANY existing artifact of this kind in this hour forces the
    new, distinct-combo publication onto its own _MM_SS form too. Raises NamingCollisionExhausted
    if both forms for THIS combo are already occupied (see dated_doc_basename)."""
    if kind not in BENCH_KIND_DEFAULT_EXT:
        raise ValueError(
            "bench_filename: kind must be one of %r, got %r" % (tuple(BENCH_KIND_DEFAULT_EXT), kind)
        )
    resolved_ext = ext or BENCH_KIND_DEFAULT_EXT[kind]
    yymmddhh, mm, ss = kst_tokens(generated_utc)
    meta = meta or {}
    existing = set(existing_basenames)
    combo = "%s_%s_%s" % (meta.get("model", "NA"), meta.get("gpu_key", "NA"), meta.get("vllm_version", "NA"))
    prefix = "%s_%s_" % (kind, yymmddhh)
    base = "%s_%s_%s.%s" % (kind, yymmddhh, combo, resolved_ext)
    if not any(name.startswith(prefix) for name in existing):
        return base
    suffixed = "%s_%s_%s_%s_%s.%s" % (kind, yymmddhh, mm, ss, combo, resolved_ext)
    if suffixed in existing:
        raise NamingCollisionExhausted(
            "bench_filename: both %r and %r are already occupied by another publication -- "
            "canon permits only unsuffixed or a single _MM_SS suffix" % (base, suffixed))
    return suffixed


def report_basename(slug, ext="html"):
    """docs/report/ is the ONE undated exception -- kebab-case slug only, no time token, no
    collision suffix (the slug itself IS the file identity; re-publishing overwrites in place,
    README-style)."""
    if not slug or not _KEBAB_RE.match(slug):
        raise ValueError("report_basename: slug must be non-empty kebab-case ([a-z0-9]+(-[a-z0-9]+)*), got %r" % (slug,))
    return "%s.%s" % (slug, ext)


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

    d1 = simlog_dirname("2026-06-21T12:21:00Z", "vLLM0.22.1_KV클램프_시뮬")
    _require(d1 == "26062121_vLLM0.22.1_KV클램프_시뮬", d1)

    meta = {"model": "solar-open2-250b", "gpu_key": "GB10", "vllm_version": "0.22.0"}
    b1 = bench_filename("bench_report", meta, "2026-07-24T16:22:29Z")
    _require(b1 == "bench_report_26072501_solar-open2-250b_GB10_0.22.0.md", b1)
    c1 = bench_filename("benchmark", meta, "2026-07-24T16:22:29Z")
    _require(c1 == "benchmark_26072501_solar-open2-250b_GB10_0.22.0.yaml", c1)

    r1 = report_basename("rtxpro6000-benchmark-explorer")
    _require(r1 == "rtxpro6000-benchmark-explorer.html", r1)
    try:
        report_basename("has_underscore")
        raise AssertionError("expected ValueError for non-kebab slug")
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
