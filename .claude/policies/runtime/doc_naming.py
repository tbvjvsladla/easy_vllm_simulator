#!/usr/bin/env python3
"""doc_naming.py -- 파이썬 호출부를 위한 명명 SSOT **shim**(구현 아님 · plan_26101912 D6).

정본은 둘이며 이 파일은 어느 것도 다시 적지 않는다:
  - 산문 문서(plan/devlog/testlog/request/checklist · simlog · report) 명명 = `.claude/skills/wiki-desk/src/doc_naming.ts`
    (TypeScript · wiki-desk 안 Python 0). 함수는 `node doc_naming.ts <함수> '<JSON 인자>'` 로 부르고, 정규식은
    `--patterns` 원문을 받아 같은 문자열로 컴파일한다(이름 그룹 `(?<x>` → `(?P<x>` 만 바꾼다).
  - 벤치 명명(kst_tokens · gpu_key · bench_filename · scan_bench_dir · 예외 2종) = `adversarial-benchmark/scripts/doc_naming.py`
    — 재수출한다(사본 ✗). TS 의 KST 변환과 이 벤치 kst_tokens 의 일치는 `--self-test` 가 교차검증한다.

호출자: evidence_publisher.py · hint-publisher hintlib/lineage.py · upstream-version-watch native_multinode_serve.py.
정본 부재는 **fail-loud**(ImportError) — 조용한 대체 구현 금지. stdlib only.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

_REPO = Path(__file__).resolve().parents[3]
_TS = _REPO / ".claude" / "skills" / "wiki-desk" / "src" / "doc_naming.ts"
_BENCH_NAMING_PATH = _REPO / ".claude" / "skills" / "adversarial-benchmark" / "scripts" / "doc_naming.py"
_NODE_MAJOR_MIN = 24


def _load_bench_naming():
    if not _BENCH_NAMING_PATH.is_file():
        raise ImportError("doc_naming: bench naming SSOT missing at %s -- this shim re-exports it and must not "
                          "substitute its own copy (plan_26090410 P0.5b)" % _BENCH_NAMING_PATH)
    spec = importlib.util.spec_from_file_location("_bench_doc_naming", _BENCH_NAMING_PATH)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def _node():
    node = shutil.which("node")
    if node is None or not _TS.is_file():
        raise ImportError("doc_naming: prose naming SSOT needs node >= %d and %s (wiki-desk TS) -- "
                          "no Python fallback by design (plan_26101912 D6)" % (_NODE_MAJOR_MIN, _TS))
    return node


_bench = _load_bench_naming()
kst_tokens = _bench.kst_tokens
gpu_key = _bench.gpu_key
bench_filename = _bench.bench_filename
scan_bench_dir = _bench.scan_bench_dir
BENCH_KIND_DEFAULT_EXT = _bench.BENCH_KIND_DEFAULT_EXT
NamingCollisionExhausted = _bench.NamingCollisionExhausted   # 산문 명명도 같은 예외 클래스를 쓴다(두 클래스 ✗)
NamingSourceUnreadable = _bench.NamingSourceUnreadable

_NODE = _node()


def _run(*args):
    r = subprocess.run([_NODE, str(_TS), *args], capture_output=True, text=True)
    if r.returncode not in (0, 3) or not r.stdout.strip():
        raise RuntimeError("doc_naming.ts failed (exit %s): %s" % (r.returncode, r.stderr.strip()))
    return json.loads(r.stdout)


def _py_pattern(js):
    return re.compile(js.replace("(?<", "(?P<"))


_P = _run("--patterns")
DATED_DOC_TYPES = tuple(_P["dated_doc_types"])
_TOPIC_RE = _py_pattern(_P["topic"])
_DATED_DOC_RE = _py_pattern(_P["dated_doc"])
_REPORT_RE = _py_pattern(_P["report"])


def _call(fn, *args):
    out = _run(fn, json.dumps(list(args), ensure_ascii=False))
    if out.get("ok"):
        return out.get("value")
    if out.get("error") == "NamingCollisionExhausted":
        raise NamingCollisionExhausted(out.get("message"))
    raise ValueError(out.get("message"))


def validate_topic(topic):
    _call("validate_topic", topic)


def is_dated_doc_basename(doc_type, basename):
    return bool(_call("is_dated_doc_basename", doc_type, basename))


def dated_doc_basename(doc_type, generated_utc, topic, existing_basenames=(), ext="md"):
    return _call("dated_doc_basename", doc_type, generated_utc, topic, sorted(existing_basenames), ext)


def simlog_dirname(generated_utc, topic, existing_dirnames=()):
    return _call("simlog_dirname", generated_utc, topic, sorted(existing_dirnames))


def report_basename(name, ext="html"):
    return _call("report_basename", name, ext)


def _require(condition, message):
    if not condition:
        raise AssertionError(message)


def _self_test():
    # 교차검증: TS 의 KST 변환(산문 명명)과 벤치 정본의 kst_tokens 가 같은 토큰을 낸다
    for utc in ("2026-07-24T16:22:29Z", "2026-07-15T21:21:33Z", "2026-12-31T15:00:00Z", "2026-02-29T00:00:00Z", None, "garbage"):
        _require(tuple(_call("kst_tokens", utc)) == tuple(kst_tokens(utc)), "KST 교차검증 불일치: %r" % (utc,))
    _require(dated_doc_basename("plan", "2026-07-25T06:58:00Z", "하네스_루프_구조개선") == "plan_26072515_하네스_루프_구조개선.md", "plan")
    _require(dated_doc_basename("testlog", "2026-07-25T06:58:29Z", "구조개선", {"testlog_26072515_구조개선.md"})
             == "testlog_26072515_58_29_구조개선.md", "collision")
    try:
        dated_doc_basename("testlog", "2026-07-25T06:58:29Z", "구조개선",
                           {"testlog_26072515_구조개선.md", "testlog_26072515_58_29_구조개선.md"})
        raise AssertionError("expected NamingCollisionExhausted")
    except NamingCollisionExhausted:
        pass
    _require({"hour", "topic"} <= set(_DATED_DOC_RE.groupindex), "lineage 가 읽는 그룹 이름")
    _require(is_dated_doc_basename("testlog", "testlog_26072515_58_29_구조개선.md")
             and not is_dated_doc_basename("plan", "testlog_26072515_구조개선.md"), "is_dated")
    _require(simlog_dirname("2026-06-21T12:21:00Z", "vLLM0.22.1_KV클램프_시뮬") == "26062121_vLLM0.22.1_KV클램프_시뮬", "simlog")
    _require(report_basename("perf_26082421_qwen3.8-27b_레시피별_성능") == "perf_26082421_qwen3.8-27b_레시피별_성능.html", "report")
    try:
        report_basename("rtxpro6000-benchmark-explorer")
        raise AssertionError("expected ValueError")
    except ValueError:
        pass
    _require(bench_filename is _bench.bench_filename and NamingCollisionExhausted is _bench.NamingCollisionExhausted,
             "bench naming must be re-exported, not copied")
    print("[doc_naming shim] self-test PASS")


def main(argv=None):
    ap = argparse.ArgumentParser(description="doc_naming.py -- Python shim over the naming SSOTs")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)
    if args.self_test:
        _self_test()
        return
    ap.print_help(sys.stderr)
    raise SystemExit(2)


if __name__ == "__main__":
    main()
