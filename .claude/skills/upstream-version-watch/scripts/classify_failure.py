#!/usr/bin/env python3
"""A3 — 실패 로그 결정론 분류기.

failure_patterns.yaml 의 signature(정규식)로 빌드/스모크 실패 로그를 1차 분류한다.
  requirements-fixable → 범위 내(Loop-Until-Done 조정)
  source-build-class   → 범위 밖(propose Y/N 에스컬레이트)
  unknown              → Model-C(LLM 제안 + 사람 승인)

출력(JSON): {class, matched_signature, note, evidence}
사용: docker logs <c> 2>&1 | python3 classify_failure.py
      python3 classify_failure.py --log /path/to/build.log
종료코드: 0=requirements-fixable, 1=source-build-class, 2=unknown(=Model-C 필요).
"""
import sys, re, json, argparse, os

PATTERNS_FILE = os.path.join(os.path.dirname(__file__), "..", "failure_patterns.yaml")


def load_patterns(path):
    """failure_patterns.yaml 의 - signature/class/note 블록을 정규식으로 파싱(yaml 의존 없음)."""
    pats, cur = [], {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            m = re.match(r'\s*-\s*signature:\s*"(.*)"\s*$', line)
            if m:
                if cur:
                    pats.append(cur)
                cur = {"signature": m.group(1).encode().decode("unicode_escape")}
                continue
            m = re.match(r'\s*class:\s*(\S+)', line)
            if m and cur:
                cur["class"] = m.group(1)
                continue
            m = re.match(r'\s*note:\s*"(.*)"\s*$', line)
            if m and cur:
                cur["note"] = m.group(1)
    if cur:
        pats.append(cur)
    return pats


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", help="로그 파일(미지정 시 stdin)")
    ap.add_argument("--patterns", default=PATTERNS_FILE)
    a = ap.parse_args()

    text = open(a.log, encoding="utf-8", errors="replace").read() if a.log else sys.stdin.read()
    pats = load_patterns(a.patterns)

    for p in pats:
        m = re.search(p["signature"], text)
        if m:
            out = {"class": p["class"], "matched_signature": p["signature"],
                   "note": p.get("note", ""), "evidence": m.group(0)[:200]}
            print(json.dumps(out, ensure_ascii=False, indent=2))
            sys.exit(0 if p["class"] == "requirements-fixable" else 1)

    print(json.dumps({"class": "unknown",
                      "note": "알려진 패턴 미매칭 → Model-C: LLM 이 {proposed_class, evidence} 제시 후 사람 승인.",
                      "evidence": text.strip().splitlines()[-1][:200] if text.strip() else ""},
                     ensure_ascii=False, indent=2))
    sys.exit(2)


if __name__ == "__main__":
    main()
