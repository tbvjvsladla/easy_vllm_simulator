#!/usr/bin/env python3
"""서빙 세션 사이드카 — `docs/logs/<node_id>/sessions/<session_id>.json` + serve_start/stop 이벤트.

근거: plan_26073109 §2.4(`sessions/` = 모델·레시피·토폴로지 태그 · events 의 serve_start/stop).
필요성: plan_26073109_12_47 합격기준 2 — "모델별 rollup + 태그가 존재하고 **조건별 비교가 가능**하다".
사이드카가 없으면 samples/ 는 모델 구분이 없는 한 덩어리라 조건별 비교가 성립하지 않는다.

설계 원칙(기존 평면과 동형):
- **줄마다 반복하지 않는다**: samples CSV 는 1초마다 쓰이므로 모델명을 넣으면 용량이 폭증한다.
  태그는 사이드카에 1회 쓰고, 구간은 [started_utc, stopped_utc] 로 samples 를 자른다.
- **시각은 `--now` 주입만**(벽시계 금지 — docs.md §기계판독 데이터 평면 · staleness_gate 선례 정합).
- **값을 만들어내지 않는다**: 판정(verdict)은 호출자가 준 것만 적는다. 미종료 세션은 stopped_utc=null
  로 남으며 그것이 "아직 안 끝났다"와 "기록이 없다"를 구분한다.
"""
import argparse
import json
import os
import re
import sys
from datetime import datetime, timezone

ISO_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
SAFE_ID_RE = re.compile(r"^[A-Za-z0-9._-]+$")


def _parse_now(value):
    """ISO-Z 문자열만 받는다. 형식이 어긋나면 조용히 넘기지 않고 죽는다."""
    if not isinstance(value, str) or not ISO_RE.match(value):
        raise SystemExit("--now 는 YYYY-MM-DDTHH:MM:SSZ 형식이어야 한다: %r" % (value,))
    return value


def _epoch(iso):
    return int(datetime.strptime(iso, "%Y-%m-%dT%H:%M:%SZ")
               .replace(tzinfo=timezone.utc).timestamp())


def _session_path(node_dir, session_id):
    if not SAFE_ID_RE.match(session_id or ""):
        raise SystemExit("session-id 는 [A-Za-z0-9._-]+ 여야 한다: %r" % (session_id,))
    return os.path.join(node_dir, "sessions", session_id + ".json")


def _append_event(node_dir, rec):
    """events/<YYYY-MM>.jsonl 에 append. 기존 평면과 같은 파일·같은 키 규약."""
    month = rec["ts"][:7]
    path = os.path.join(node_dir, "events", month + ".jsonl")
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False, sort_keys=True) + "\n")
    return path


def cmd_start(args):
    now = _parse_now(args.now)
    path = _session_path(args.node_dir, args.session_id)
    if os.path.exists(path) and not args.force:
        raise SystemExit("이미 존재하는 세션이다(덮어쓰기는 --force): %s" % path)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    doc = {
        "session_id": args.session_id,
        "model": args.model,
        "model_path": args.model_path,
        "recipe": args.recipe,
        "topology": args.topology,
        "tensor_parallel_size": args.tp,
        "config_file": args.config_file,
        "image": args.image,
        "started_utc": now,
        "started_epoch": _epoch(now),
        "stopped_utc": None,
        "stopped_epoch": None,
        "verdict": None,
        "notes": args.note or [],
    }
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    ev = _append_event(args.node_dir, {
        "kind": "serve_start", "ts": now, "source": "blackbox_session",
        "session_id": args.session_id, "model": args.model,
        "topology": args.topology, "tensor_parallel_size": args.tp,
    })
    print("세션 시작: %s" % path)
    print("이벤트   : %s" % ev)
    return 0


def cmd_stop(args):
    now = _parse_now(args.now)
    path = _session_path(args.node_dir, args.session_id)
    if not os.path.isfile(path):
        raise SystemExit("세션 파일이 없다: %s" % path)
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    doc["stopped_utc"] = now
    doc["stopped_epoch"] = _epoch(now)
    doc["verdict"] = args.verdict
    if args.note:
        doc["notes"] = (doc.get("notes") or []) + args.note
    dur = doc["stopped_epoch"] - doc.get("started_epoch", doc["stopped_epoch"])
    doc["duration_s"] = dur
    with open(path, "w", encoding="utf-8") as f:
        f.write(json.dumps(doc, ensure_ascii=False, indent=2, sort_keys=True) + "\n")
    ev = _append_event(args.node_dir, {
        "kind": "serve_stop", "ts": now, "source": "blackbox_session",
        "session_id": args.session_id, "model": doc.get("model"),
        "verdict": args.verdict, "duration_s": dur,
    })
    print("세션 종료: %s (%ds · verdict=%s)" % (path, dur, args.verdict))
    print("이벤트   : %s" % ev)
    return 0


def cmd_list(args):
    d = os.path.join(args.node_dir, "sessions")
    if not os.path.isdir(d):
        print("(세션 없음)")
        return 0
    rows = []
    for fn in sorted(os.listdir(d)):
        if not fn.endswith(".json"):
            continue
        with open(os.path.join(d, fn), encoding="utf-8") as f:
            doc = json.load(f)
        rows.append(doc)
    if not rows:
        print("(세션 없음)")
        return 0
    print("%-34s %-22s %-7s %-3s %-20s %-9s %s" %
          ("session_id", "model", "topo", "tp", "started_utc", "dur_s", "verdict"))
    for d_ in rows:
        print("%-34s %-22s %-7s %-3s %-20s %-9s %s" % (
            d_.get("session_id", "?"), (d_.get("model") or "?")[:22],
            d_.get("topology") or "?", d_.get("tensor_parallel_size") or "?",
            d_.get("started_utc") or "?",
            d_.get("duration_s") if d_.get("duration_s") is not None else "(진행중)",
            d_.get("verdict") or "-"))
    return 0


def cmd_slice(args):
    """세션 구간으로 samples 를 자른다 — 조건별 비교의 실제 소비 경로."""
    path = _session_path(args.node_dir, args.session_id)
    if not os.path.isfile(path):
        raise SystemExit("세션 파일이 없다: %s" % path)
    with open(path, encoding="utf-8") as f:
        doc = json.load(f)
    lo = doc.get("started_epoch")
    hi = doc.get("stopped_epoch")
    if lo is None:
        raise SystemExit("started_epoch 이 없다(손상된 세션): %s" % path)
    sdir = os.path.join(args.node_dir, "samples")
    if not os.path.isdir(sdir):
        raise SystemExit("samples 디렉터리가 없다: %s" % sdir)
    header, out = None, []
    for fn in sorted(os.listdir(sdir)):
        if not fn.endswith(".csv"):
            continue
        with open(os.path.join(sdir, fn), encoding="utf-8") as f:
            for i, line in enumerate(f):
                line = line.rstrip("\n")
                if i == 0:
                    if header is None:
                        header = line
                    continue
                ts = line.split(",", 1)[0]
                if not ts.isdigit():
                    continue
                t = int(ts)
                if t >= lo and (hi is None or t <= hi):
                    out.append(line)
    if header:
        print(header)
    for line in out:
        print(line)
    print("# %d 행 · 구간 [%s, %s]" % (len(out), doc.get("started_utc"),
                                       doc.get("stopped_utc") or "진행중"), file=sys.stderr)
    return 0


def self_test():
    import tempfile
    ok = []
    with tempfile.TemporaryDirectory() as td:
        node = os.path.join(td, "node-x")
        os.makedirs(os.path.join(node, "samples"))
        # 세션 구간 안 2행 · 밖 2행
        with open(os.path.join(node, "samples", "2026-07-31.csv"), "w") as f:
            f.write("ts,mem_avail\n")
            for t in (100, 150, 200, 250, 300):
                f.write("%d,%d\n" % (t, t))
        A = argparse.Namespace(
            node_dir=node, session_id="s1", model="m", model_path="/p", recipe="r",
            topology="single", tp=1, config_file="c", image="img",
            now="2026-07-31T00:02:30Z", note=None, force=False)
        # started_epoch 을 150 으로 맞추기 위해 직접 계산 대신 실제 동작만 검증
        cmd_start(A)
        p = _session_path(node, "s1")
        doc = json.load(open(p))
        ok.append(("start 사이드카 생성", os.path.isfile(p) and doc["stopped_utc"] is None))
        ok.append(("start 이벤트 append",
                   os.path.isfile(os.path.join(node, "events", "2026-07.jsonl"))))
        B = argparse.Namespace(node_dir=node, session_id="s1",
                               now="2026-07-31T00:12:30Z", verdict="PASS", note=["끝"])
        cmd_stop(B)
        doc = json.load(open(p))
        ok.append(("stop duration 600s", doc["duration_s"] == 600))
        ok.append(("stop verdict 기록", doc["verdict"] == "PASS"))
        # 미종료 구분: stopped_utc=null 이 "안 끝남", 파일 부재가 "기록 없음"
        A2 = argparse.Namespace(**{**vars(A), "session_id": "s2"})
        cmd_start(A2)
        doc2 = json.load(open(_session_path(node, "s2")))
        ok.append(("미종료 세션은 stopped_utc=None", doc2["stopped_utc"] is None))
        # 중복 start 는 거부
        try:
            cmd_start(A2)
            ok.append(("중복 start 거부", False))
        except SystemExit:
            ok.append(("중복 start 거부", True))
        # 잘못된 --now 는 거부
        try:
            _parse_now("2026-07-31 00:00:00")
            ok.append(("비ISO --now 거부", False))
        except SystemExit:
            ok.append(("비ISO --now 거부", True))
        # 경로 이스케이프 거부
        try:
            _session_path(node, "../escape")
            ok.append(("session-id 이스케이프 거부", False))
        except SystemExit:
            ok.append(("session-id 이스케이프 거부", True))
    for name, good in ok:
        print("%s %s" % ("PASS" if good else "FAIL", name))
    n = sum(1 for _, g in ok if g)
    print("\nself-test: %d/%d PASS" % (n, len(ok)))
    return 0 if n == len(ok) else 1


def main():
    ap = argparse.ArgumentParser(description="노드블랙박스 서빙 세션 사이드카")
    ap.add_argument("--node-dir", help="docs/logs/<node_id>")
    sub = ap.add_subparsers(dest="cmd")

    s = sub.add_parser("start", help="세션 시작 + serve_start 이벤트")
    s.add_argument("--session-id", required=True)
    s.add_argument("--model", required=True)
    s.add_argument("--model-path")
    s.add_argument("--recipe")
    s.add_argument("--topology", choices=["single", "multi"], required=True)
    s.add_argument("--tp", type=int)
    s.add_argument("--config-file")
    s.add_argument("--image")
    s.add_argument("--now", required=True, help="YYYY-MM-DDTHH:MM:SSZ (벽시계 금지)")
    s.add_argument("--note", action="append")
    s.add_argument("--force", action="store_true")
    s.set_defaults(func=cmd_start)

    t = sub.add_parser("stop", help="세션 종료 + serve_stop 이벤트")
    t.add_argument("--session-id", required=True)
    t.add_argument("--now", required=True)
    t.add_argument("--verdict", choices=["PASS", "FAIL", "REFUTE", "ABORT"])
    t.add_argument("--note", action="append")
    t.set_defaults(func=cmd_stop)

    l = sub.add_parser("list", help="세션 목록")
    l.set_defaults(func=cmd_list)

    c = sub.add_parser("slice", help="세션 구간의 samples 만 출력")
    c.add_argument("--session-id", required=True)
    c.set_defaults(func=cmd_slice)

    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args()
    if args.self_test:
        return self_test()
    if not getattr(args, "func", None):
        ap.print_help()
        return 2
    if not args.node_dir:
        raise SystemExit("--node-dir 이 필요하다(docs/logs/<node_id>)")
    return args.func(args)


if __name__ == "__main__":
    sys.exit(main())
