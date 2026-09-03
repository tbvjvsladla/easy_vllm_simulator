#!/usr/bin/env python3
"""library_relay.py — 메인↔서브 **도서관 교환의 물리 채널**(§2.7.8 의 실행자).

왜 이 파일이 생겼나(2026-09-03 · P4 · plan_26090317):
    `library_exchange.py` 는 request/export/attestation **세 파일이 이미 로컬에 있다**는 전제로
    판정만 한다. 그런데 그 파일들이 서브에서 메인으로 **어떻게 오는지**는 어디에도 배선돼 있지
    않았다 — 계약과 판정기는 있고 통로가 없는, 이 저장소가 여러 번 만난 형태다.

통로 설계(새 경로를 만들지 않는다):
    · 서브 → 메인 : 서브가 **자기 `docs/` 아래**에 교환 메시지를 발행한다. 상향 회수는 이미
      `fetch_sub_docs.sh` 가 docs 만 미러하는 것으로 정해져 있으므로(헌법 §서브 docs 계약),
      그 통로를 그대로 쓴다. 코드·설정을 회수하는 것이 아니라 **서브가 저작한 문서**를 읽는다.
    · 메인 → 서브 : 반출(export)은 **다음 릴레이 턴의 Task 본문에 실어** 보낸다. 역방향 접속도,
      서브 워크스페이스로의 별도 쓰기도 만들지 않는다(A2A 평면이 곧 채널이다).

    서브 경로 규약:  docs/library_exchange/<exchange_id>/{request,attestation}.json
    메인 미러 경로:  sync_staging/sub_docs/library_exchange/<exchange_id>/…

사서(wiki-desk)는 메인 단독이다 — 서브는 도서관을 뒤지지 않고 **무엇을 찾는지만** 말한다.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", "..", ".."))
MIRROR = os.path.join(REPO, "sync_staging", "sub_docs", "library_exchange")
FETCH = os.path.join(REPO, ".claude/skills/upstream-version-watch/scripts/fetch_sub_docs.sh")
SMOKE_QUERY = os.path.join(REPO, ".claude/skills/wiki-desk/scripts/smoke_query.py")
WIKI_ROOT = os.path.join(REPO, "__llm-wiki")
GATE = os.path.join(HERE, "library_exchange.py")
EXPORT_DIR = os.path.join(REPO, "docs", "_evidence", "library_exchange")

SCHEMA_VERSION = 1
MAX_REFS = 6
EXCERPT_CHARS = 6000
# 2026-09-03(P4 실측) 1200 → 6000. 발췌는 **읽기 보조**가 아니라 서브 결정의 유일한 근거다.
# 실측 인과: 발췌가 잘려 서브가 KV 관련 결정을 정직하게 유보(`accepted:false`) → KV 절대 클램프
# (`policy:KV_ABSOLUTE_CLAMP_PORTABILITY`)를 채택하지 못함 → gmu 0.90 만으로 서빙 → KV 94 GiB 할당 →
# mem watchdog 이 `mem_avail 4.3 GiB < 10 GiB` 에서 컨테이너 사살(호스트는 지켜졌다).
# 즉 **발췌 예산이 그라운딩의 성패를 결정한다** — 유보 자체는 옳은 행동이었고, 부족했던 것은 근거다.
# 상한을 두는 이유는 여전히 있다(서브 컨텍스트 비용) — 없애지 않고 근거 있는 값으로 올린다.


def pull(apply=True) -> list:
    """서브 docs 를 미러하고 교환 디렉터리를 나열한다. 미러 자체는 기존 통로가 소유한다."""
    if apply:
        out = subprocess.run(["bash", FETCH, "--apply"], capture_output=True, text=True)
        if out.returncode != 0:
            raise SystemExit(f"[library-relay] FAIL: 서브 docs 미러 실패\n{out.stderr[-800:]}")
    if not os.path.isdir(MIRROR):
        return []
    return sorted(d for d in os.listdir(MIRROR) if os.path.isdir(os.path.join(MIRROR, d)))


def _digest_of(path: str, anchor: str | None) -> tuple[str, str]:
    """(digest, excerpt). 앵커가 있으면 그 절만, 없으면 문서 전체를 대상으로 한다.

    **digest 는 반출 시점의 내용에 대한 것**이다 — 서브의 인용이 낡았는지(CITATION_STALE)를
    가르는 유일한 근거이므로, excerpt(읽기 보조)가 아니라 대상 바이트에서 계산한다.
    """
    with open(path, encoding="utf-8") as f:
        text = f.read()
    if anchor:
        m = re.search(r"^#{1,6}\s*" + re.escape(anchor) + r"\s*$", text, re.M)
        if m:
            nxt = re.search(r"^#{1,6}\s", text[m.end():], re.M)
            text = text[m.start(): m.end() + (nxt.start() if nxt else len(text))]
    return hashlib.sha256(text.encode("utf-8")).hexdigest(), text[:EXCERPT_CHARS]


def resolve(request: dict, top: int = MAX_REFS) -> dict:
    """사서에게 질의해 `library.resolution.export` 를 만든다.

    해소 실패는 **정직한 공백**이다 — `unresolved` + 사유를 낸다. 없는 근거를 지어내지 않는다.
    """
    terms = ((request.get("query") or {}).get("terms")) or []
    if not terms:
        raise SystemExit("[library-relay] FAIL: request.query.terms 가 비었다 — 무엇을 찾는지 모른다.")
    if not os.path.isdir(WIKI_ROOT):
        return {"schema_version": SCHEMA_VERSION, "kind": "library.resolution.export",
                "exchange_id": request["exchange_id"], "node_id": "main",
                "topology": request.get("topology", "single"),
                "resolution": {"status": "unresolved", "librarian": "wiki-desk",
                               "reason": f"도서관 미초기화({WIKI_ROOT}) — init_wiki_desk.py 선행 필요"},
                "references": []}
    out = subprocess.run([sys.executable, SMOKE_QUERY, "--wiki-root", WIKI_ROOT,
                          "--query", " ".join(terms), "--top", str(top)],
                         capture_output=True, text=True, timeout=300)
    if out.returncode != 0:
        return {"schema_version": SCHEMA_VERSION, "kind": "library.resolution.export",
                "exchange_id": request["exchange_id"], "node_id": "main",
                "topology": request.get("topology", "single"),
                "resolution": {"status": "unresolved", "librarian": "wiki-desk",
                               "reason": f"사서 질의 실패(rc={out.returncode}): {out.stderr[-200:]}"},
                "references": []}
    refs = []
    for i, m in enumerate(re.finditer(r"^- path: `([^`]+)`", out.stdout, re.M), start=1):
        rel = m.group(1)
        full = os.path.join(REPO, rel)
        if not os.path.isfile(full):
            continue                       # 디렉터리 노드(simlog run) 등은 반출 대상이 아니다
        digest, excerpt = _digest_of(full, None)
        refs.append({"ref_id": f"R{i}", "path": rel, "digest": digest, "excerpt": excerpt})
        if len(refs) >= top:
            break
    status = "resolved" if refs else "unresolved"
    doc = {"schema_version": SCHEMA_VERSION, "kind": "library.resolution.export",
           "exchange_id": request["exchange_id"], "node_id": "main",
           "topology": request.get("topology", "single"),
           "resolution": {"status": status, "librarian": "wiki-desk"},
           "references": refs}
    if status != "resolved":
        doc["resolution"]["reason"] = "질의어를 덮는 문서를 도서관에서 찾지 못했다(정직한 공백)."
    return doc


def _schema_ok(doc: dict) -> list:
    out = subprocess.run([sys.executable, GATE, "validate", "--file", "-"],
                         input=json.dumps(doc, ensure_ascii=False), capture_output=True, text=True)
    return [] if out.returncode == 0 else [out.stdout.strip() or out.stderr.strip()]


def _self_test() -> int:
    import tempfile
    ok = True

    def chk(cond, label):
        nonlocal ok
        print("  [%s] %s" % ("PASS" if cond else "FAIL", label))
        ok = ok and bool(cond)

    with tempfile.TemporaryDirectory() as d:
        doc = os.path.join(d, "a.md")
        with open(doc, "w", encoding="utf-8") as f:
            f.write("# T\n\n## 앵커A\n본문A\n\n## 앵커B\n본문B\n")
        d_all, ex_all = _digest_of(doc, None)
        d_a, ex_a = _digest_of(doc, "앵커A")
        chk(d_all != d_a, "앵커 지정 시 digest 가 그 절에 대한 값이다(문서 전체와 다름)")
        chk("본문A" in ex_a and "본문B" not in ex_a, "앵커 절만 발췌된다")
        d_a2, _ = _digest_of(doc, "앵커A")
        chk(d_a == d_a2, "digest 결정론(같은 입력 → 같은 값)")
        with open(doc, "a", encoding="utf-8") as f:
            f.write("추가\n")
        d_all2, _ = _digest_of(doc, None)
        chk(d_all != d_all2, "내용이 바뀌면 digest 가 바뀐다(CITATION_STALE 의 근거)")

    req = {"schema_version": 1, "kind": "library.citation.request", "exchange_id": "x1",
           "node_id": "sub", "topology": "single", "query": {"terms": []}}
    try:
        resolve(req)
        chk(False, "빈 terms → fail-loud")
    except SystemExit:
        chk(True, "빈 terms → fail-loud")

    saved = globals()["WIKI_ROOT"]
    globals()["WIKI_ROOT"] = "/nonexistent-wiki"
    try:
        r = resolve({**req, "query": {"terms": ["x"]}})
        chk(r["resolution"]["status"] == "unresolved" and r["resolution"].get("reason"),
            "도서관 부재 → unresolved + 사유(없는 근거를 지어내지 않는다)")
        chk(r["references"] == [], "unresolved 면 references 는 빈 배열")
    finally:
        globals()["WIKI_ROOT"] = saved
    print("self-test: %s" % ("PASS" if ok else "FAIL"))
    return 0 if ok else 2


def main() -> int:
    ap = argparse.ArgumentParser(description="메인↔서브 도서관 교환 채널")
    sub = ap.add_subparsers(dest="cmd")
    p = sub.add_parser("pull", help="서브 docs 미러 후 교환 디렉터리 나열")
    p.add_argument("--no-apply", action="store_true")
    e = sub.add_parser("export", help="request → 사서 해소 → export.json")
    e.add_argument("--exchange-id", required=True)
    e.add_argument("--top", type=int, default=MAX_REFS)
    v = sub.add_parser("verify", help="request/export/attestation 3종 판정(library_exchange gate)")
    v.add_argument("--exchange-id", required=True)
    ap.add_argument("--self-test", action="store_true")
    a = ap.parse_args()
    if a.self_test:
        return _self_test()
    if a.cmd == "pull":
        ids = pull(apply=not a.no_apply)
        json.dump({"exchanges": ids, "mirror": MIRROR}, sys.stdout, ensure_ascii=False, indent=2)
        sys.stdout.write("\n")
        return 0
    if a.cmd == "export":
        rp = os.path.join(MIRROR, a.exchange_id, "request.json")
        if not os.path.isfile(rp):
            raise SystemExit(f"[library-relay] FAIL: 서브 요청 부재 — {rp} (pull 선행)")
        with open(rp, encoding="utf-8") as f:
            req = json.load(f)
        doc = resolve(req, top=a.top)
        os.makedirs(os.path.join(EXPORT_DIR, a.exchange_id), exist_ok=True)
        op = os.path.join(EXPORT_DIR, a.exchange_id, "export.json")
        with open(op, "w", encoding="utf-8") as f:
            json.dump(doc, f, ensure_ascii=False, indent=2)
            f.write("\n")
        print(f"[library-relay] export → {op} · status={doc['resolution']['status']} "
              f"· refs={len(doc['references'])}")
        return 0
    if a.cmd == "verify":
        req = os.path.join(MIRROR, a.exchange_id, "request.json")
        att = os.path.join(MIRROR, a.exchange_id, "attestation.json")
        exp = os.path.join(EXPORT_DIR, a.exchange_id, "export.json")
        missing = [p for p in (req, exp, att) if not os.path.isfile(p)]
        if missing:
            raise SystemExit("[library-relay] FAIL: 판정에 필요한 메시지 부재 — %s" % missing)
        return subprocess.run([sys.executable, GATE, "gate", "--request", req,
                               "--export", exp, "--attestation", att]).returncode
    ap.print_help()
    return 64


if __name__ == "__main__":
    sys.exit(main())
