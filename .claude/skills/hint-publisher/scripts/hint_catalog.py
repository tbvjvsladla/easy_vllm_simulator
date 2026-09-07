#!/usr/bin/env python3
"""hint_catalog.py — 카탈로그를 **원격 발행 태그에서 파생**한다 (plan_26090107 D1.1 · Phase 4).

무엇이 바뀌나
    이전에는 `index`/`reindex` 가 **로컬 상태를 신뢰해** `index.json` 과 `HINTS.md` 에 행을 썼다.
    그래서 감사 `audit_26090106` 이 잡은 두 결함이 가능했다:
      ④ `cmd_index` 가 증거 검증 0회로 `status: "active"` 를 박는다 — 격리된 태그를 다시 넣으면
         격리가 **세탁**된다.
      ⑤ 손저작 brief 가 오염되면 카탈로그가 깨진다(실측: 59항목 중 11 brief 손상 ·
         `<!--` 4개가 렌더에서 18행을 삼킴).

    이제 **진실원천은 `git ls-remote` 다.** 원격에 있는 것만 카탈로그에 들어간다.
      - "발행됐다"의 증거가 카탈로그 **바깥**에 생긴다 → ④ 가 구조적으로 소멸한다.
      - 손저작 평면이 사라진다 → ⑤ 가 재발 불가가 된다(사람이 brief 를 타이핑할 자리가 없다).
      - 로컬에만 있는 미발행 태그는 **들어갈 수 없다**.

fail-closed
    원격 조회에 실패하면 **캐시로 대체하지 않고 중단한다.** 침묵 폴백은 카탈로그를 조용히 낡게
    만들고, 낡은 카탈로그는 "기록이 원래 없었던 것"과 구분되지 않는다(docs.md 부재≠결측).
    원격에 있는데 로컬에 태그 오브젝트가 없으면 **brief 를 지어낼 수 없으므로** 역시 중단한다
    (`git fetch --tags` 를 하라고 말한다). 합성 금지가 여기서도 그대로다.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HINTS_MARKER = "<!-- hint-index:rows -->"
CENTRAL_FLAG = "hints/.central_authority"
# ★ 2026-09-04(CP7 · plan_26090415 §7.5 M1): 태그가 5세그먼트가 됐다 —
#   `hint/<vllm>/<model>/<arch>/<recipe>`. 이 파일은 `hint_tag.py` 의 9곳을 세던 개정
#   표면에 **들어 있지 않았다**(계획의 실측 누락). 카탈로그 파생은 원격 태그 이름을
#   직접 파싱하므로, 여기를 안 고치면 새 이름이 전부 "규약 밖"으로 거부되어 발행은
#   되는데 카탈로그에 못 들어가는 상태가 된다.
TAG_SHAPE = re.compile(
    r"^hint/(?P<vllm>[^/]+)/(?P<model>[^/]+)/(?P<arch>[^/]+)/(?P<recipe>[^/]+)$")


def die(msg: str, code: int = 2) -> None:
    print(f"[hint_catalog] {msg}", file=sys.stderr)
    raise SystemExit(code)


def git(*args: str, cwd: Path | None = None, check: bool = True) -> subprocess.CompletedProcess:
    p = subprocess.run(["git", *args], cwd=str(cwd) if cwd else None,
                       capture_output=True, text=True)
    if check and p.returncode != 0:
        die(f"git {' '.join(args)} 실패(rc={p.returncode}): {p.stderr.strip()}")
    return p


# ---------------------------------------------------------------- 진실원천

def remote_hint_tags(repo: Path, remote: str, *, allow_empty: bool = False) -> list[str]:
    """**카탈로그의 진실원천.** 실패하면 캐시로 대체하지 않고 죽는다.

    `allow_empty` 는 **0건을 정상으로 선언**하는 스위치다(기본 False = fail-closed). 태그 전량을
    폐기한 직후처럼 '0건이 사실'인 상태가 실재하며, 그때 게이트가 열리지 않으면 카탈로그가
    낡은 1건을 영원히 들고 있게 된다. 조회 **실패**(returncode != 0)는 이 스위치로도 열리지
    않는다 — 그것은 '0건'이 아니라 '모른다'이고, 모르는 것을 0으로 적으면 침묵 폴백이 된다."""
    p = git("ls-remote", "--tags", remote, "refs/tags/hint/*", cwd=repo, check=False)
    if p.returncode != 0:
        die(f"원격 태그 조회 실패({remote}) — **캐시로 대체하지 않는다**(D1.1 fail-closed).\n"
            f"  {p.stderr.strip()}\n"
            "  낡은 카탈로그는 '기록이 원래 없었던 것'과 구분되지 않는다.")
    tags = sorted({line.split("\t", 1)[1].removeprefix("refs/tags/")
                   for line in p.stdout.splitlines()
                   if "\t" in line and not line.rstrip().endswith("^{}")})
    if not tags and not allow_empty:
        die(f"원격 {remote} 에 hint 태그가 0건이다 — 빈 카탈로그를 발행하지 않는다. "
            "정말 0건이 맞다면 --allow-empty 로 명시하라.")
    return tags


# ---------------------------------------------------------------- brief 추출

_HTML_COMMENT = re.compile(r"<!--.*?-->", re.S)


def extract_brief(body: str) -> str:
    """태그 본문에서 brief 한 줄.

    ⑤ 의 기전을 여기서 닫는다 — 이전 `_brief_of` 는 *"첫 비어있지 않은 줄"* 이라는 **위치
    규약**만 봤고, 템플릿 맨 위에 경고 HTML 주석이 추가되자 그 경고문이 brief 가 됐다
    (실측 4건). 위치가 아니라 **모양**으로 거른다: HTML 주석·인용부호·헤딩은 brief 가 아니다.
    """
    text = _HTML_COMMENT.sub("", body)
    for raw in text.split("\n"):
        line = raw.strip()
        if not line:
            continue
        if line.startswith(("<!--", ">", "#", "|", "-", "*", "```")):
            continue
        if line.startswith("object ") or line.startswith("type ") or line.startswith("tag "):
            continue  # `cat-file -p` 헤더가 섞여 들어오던 경로(실측 7건 오염)
        return line
    return ""


def md_cell(s: str) -> str:
    """마크다운 표 셀로 안전하게. 이스케이프가 없어 카탈로그가 깨졌다(⑤ 사슬 4단계)."""
    s = s.replace("|", "\\|").replace("\n", " ").replace("\r", " ")
    s = _HTML_COMMENT.sub("", s).replace("<!--", "&lt;!--").replace("-->", "--&gt;")
    return " ".join(s.split())


# ---------------------------------------------------------------- 파생

def payload_missing(repo: Path, tag: str) -> list:
    """태그 페이로드가 **스스로 선언한** 결손 사유코드. 읽을 수 없으면 빈 목록이다.

    부재(페이로드가 없거나 낡은 태그)와 "결손 0"을 같은 값으로 접는다는 점은 알고 쓴다 —
    카탈로그는 **비권위 캐시**이므로 여기서 fail-closed 하면 발행되지 않은 사실이 아니라
    카탈로그 갱신이 막힌다. 판정의 권위는 `hint_tag verify` 이고 여기는 표시다.
    """
    out = subprocess.run(["git", "-C", str(repo), "cat-file", "-p", f"{tag}^{{}}:PAYLOAD.json"],
                         capture_output=True, text=True)
    if out.returncode != 0:
        return []
    try:
        doc = json.loads(out.stdout)
    except ValueError:
        return []
    missing = doc.get("missing") if isinstance(doc, dict) else None
    return sorted(m for m in (missing or []) if isinstance(m, str))


def derive_entries(repo: Path, tags: list[str]) -> list[dict]:
    entries: list[dict] = []
    missing_local: list[str] = []
    for tag in tags:
        m = TAG_SHAPE.match(tag)
        if not m:
            die(f"원격 태그 이름이 규약 밖이다: {tag!r} — 카탈로그에 넣지 않는다")
        obj = git("rev-parse", "--verify", "--quiet", tag, cwd=repo, check=False).stdout.strip()
        if not obj:
            missing_local.append(tag)
            continue
        otype = git("cat-file", "-t", obj, cwd=repo).stdout.strip()
        if otype != "tag":
            die(f"{tag} 가 annotated 태그가 아니다({otype}) — 본문이 없으면 brief 를 지어낼 수 없다")
        raw = git("cat-file", "tag", obj, cwd=repo).stdout
        body = raw.split("\n\n", 1)[1] if "\n\n" in raw else ""
        anchor = git("rev-list", "-n1", tag, cwd=repo).stdout.strip()
        entries.append({
            "tag": tag, "vllm": m.group("vllm"), "model": m.group("model"),
            "arch": m.group("arch"), "recipe": m.group("recipe"),
            "brief": extract_brief(body),
            "anchor": anchor,
            "object": obj,
            # 결손은 **파생 컬럼**이다(2026-09-07 · 인터뷰 Q5). 태그 이름에 등급을 새기지 않는다 —
            # 이름은 불변인데 결손은 재발행으로 바뀔 수 있고, 이름에 새기면 그 순간 이름이 거짓이 된다.
            # 출처는 페이로드가 스스로 선언한 PAYLOAD.json.missing[] 하나다(사람 판단 ✗).
            "missing": payload_missing(repo, tag),
            "published": True,          # 원격에 있다 = 발행됐다. 이것이 유일한 근거다.
            "source": "remote-derived",  # 출처 표시(헌법 §결정론 규율)
        })
    if missing_local:
        die("원격에 있으나 **로컬에 태그 오브젝트가 없다** — brief 를 지어낼 수 없다(합성 금지):\n  "
            + "\n  ".join(missing_local[:10])
            + f"\n  (총 {len(missing_local)}건)  →  `git fetch --tags` 후 다시 실행하라.")
    return entries


def render_rows(entries: list[dict]) -> str:
    # `결손` 열(2026-09-07): "무엇을 모른 채 발행됐는가" 가 카탈로그에서 보여야 한다. 본문 슬롯과
    # PAYLOAD.missing[] 에만 있으면 사람이 태그를 열어야 알 수 있고, 그러면 비교가 불가능하다.
    rows = ["| 태그 | vLLM | 모델 | arch | 결손 | brief |", "|---|---|---|---|---|---|"]
    for e in entries:
        miss = e.get("missing") or []
        cell = "—" if not miss else md_cell(" · ".join(miss))
        rows.append(f"| `{e['tag']}` | {md_cell(e['vllm'])} | {md_cell(e['model'])} | "
                    f"{md_cell(e['arch'])} | {cell} | {md_cell(e['brief'])} |")
    return "\n".join(rows)


def splice_hints_md(text: str, rows: str) -> str:
    """마커 **쌍** 사이만 갈아끼운다. 마커가 없으면 쓰지 않는다(⑬ 침묵 파괴 방지)."""
    if text.count(HINTS_MARKER) != 2:
        die(f"HINTS.md 의 마커 `{HINTS_MARKER}` 가 정확히 2개가 아니다"
            f"(발견 {text.count(HINTS_MARKER)}개) — 기존 내용을 지우지 않고 중단한다.")
    head, _mid, tail = text.split(HINTS_MARKER, 2)
    return f"{head}{HINTS_MARKER}\n{rows}\n{HINTS_MARKER}{tail}"


def cmd_derive(a) -> int:
    repo = Path(a.repo).resolve()
    if not (repo / CENTRAL_FLAG).exists():
        die(f"{CENTRAL_FLAG} 부재 — 카탈로그는 중앙 저장소만 갱신한다(권한 비대칭).")
    tags = remote_hint_tags(repo, a.remote, allow_empty=a.allow_empty)
    entries = derive_entries(repo, tags)
    rows = render_rows(entries)

    idx_p = repo / "hints" / "index.json"
    md_p = repo / "HINTS.md"
    doc = {
        "_note": ("카탈로그는 **원격 발행 태그에서 파생**한다(plan_26090107 D1.1). 손저작 금지 — "
                  "이 파일을 직접 편집하면 다음 derive 에서 사라진다. 진실원천은 "
                  "`git ls-remote --tags <remote> 'refs/tags/hint/*'` 이고, brief 는 로컬 태그 "
                  "오브젝트에서 파싱한다(합성 금지)."),
        "schema": 2,
        "source": "remote-derived",
        "remote": a.remote,
        "generated_kst": a.generated_kst,
        "count": len(entries),
        "hints": entries,
    }
    if a.dry_run:
        print(f"[hint_catalog] DRY-RUN — 원격 {len(tags)}건 · 파생 {len(entries)}건")
        print(rows[:400] + ("…" if len(rows) > 400 else ""))
        return 0
    idx_p.write_text(json.dumps(doc, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    md_p.write_text(splice_hints_md(md_p.read_text(encoding="utf-8"), rows), encoding="utf-8")
    print(f"[hint_catalog] 파생 완료 — 원격 {len(tags)}건 → index.json {len(entries)}항목 · "
          f"HINTS.md {len(entries)}행")

    local_only = sorted(set(git("tag", "-l", "hint/*", cwd=repo).stdout.split()) - set(tags))
    if local_only:
        print(f"[hint_catalog] 고지: 로컬에만 있는 미발행 태그 {len(local_only)}건 — "
              "카탈로그에 **들어가지 않았다**(발행 사실이 없다):", file=sys.stderr)
        for t in local_only[:10]:
            print(f"  {t}", file=sys.stderr)
    return 0


# ---------------------------------------------------------------- self-test

def _run_self_test() -> int:
    import tempfile
    checks: list[tuple[str, bool]] = []

    def ck(n: str, c: bool) -> None:
        checks.append((n, bool(c)))

    def expect_die(fn):
        try:
            fn()
        except SystemExit:
            return "raised"
        return None

    # brief 추출 — ⑤ 의 두 오염 경로
    warn = ("<!-- ⚠ 절 헤딩(`## 1.` ~ `## 7.`)을 지우지 마라 — seal 의 린터 L1 이 "
            "fail-closed 로 -->\n\n진짜 brief 한 줄\n\n## 1. 벽 지도\n")
    ck("★⑤회귀 HTML 주석을 brief 로 캐지 않는다", extract_brief(warn) == "진짜 brief 한 줄")
    ck("★⑤회귀 cat-file 헤더를 brief 로 캐지 않는다",
       extract_brief("object abc\ntype commit\ntag hint/a/b/c/d\n\n실제 brief\n") == "실제 brief")
    ck("인용부호 줄은 brief 아님",
       extract_brief("> ⚠ 유효맥락 …\n\nbrief 다\n") == "brief 다")
    ck("헤딩은 brief 아님", extract_brief("# 제목\n\nbrief\n") == "brief")
    ck("본문이 비면 빈 문자열", extract_brief("") == "")

    # 표 셀 이스케이프 — ⑤ 사슬 4단계
    ck("★파이프 이스케이프", md_cell("a | b") == "a \\| b")
    ck("★미닫힘 주석 무해화", "<!--" not in md_cell("깨진 <!-- 주석"))
    ck("줄바꿈 평탄화", "\n" not in md_cell("a\nb"))

    # HINTS.md 스플라이스 — ⑬ 침묵 파괴 방지
    good = f"머리\n{HINTS_MARKER}\n옛 행\n{HINTS_MARKER}\n꼬리\n"
    out = splice_hints_md(good, "| 새 | 행 |")
    ck("마커 사이만 교체", "새" in out and "옛 행" not in out and "머리" in out and "꼬리" in out)
    ck("★음성대조 마커 1개면 거부",
       expect_die(lambda: splice_hints_md(f"머리\n{HINTS_MARKER}\n꼬리\n", "x")) == "raised")
    ck("★음성대조 마커 0개면 거부",
       expect_die(lambda: splice_hints_md("머리만\n", "x")) == "raised")

    # 파생 — 원격에 있는데 로컬에 없으면 합성하지 않고 죽는다
    with tempfile.TemporaryDirectory() as tmp:
        repo = Path(tmp) / "r"
        repo.mkdir()
        git("init", "-q", "-b", "main", str(repo))
        git("config", "user.email", "selftest@example.invalid", cwd=repo)
        git("config", "user.name", "selftest", cwd=repo)
        (repo / "f.txt").write_text("x\n", encoding="utf-8")
        git("add", "-A", cwd=repo)
        git("commit", "-qm", "c", cwd=repo)
        git("tag", "-a", "hint/1.0/m/a/qx-len1-kvfp8", "-m", "brief 줄\n\n## 1. 벽\n본문", cwd=repo)
        ents = derive_entries(repo, ["hint/1.0/m/a/qx-len1-kvfp8"])
        ck("파생 항목 필드", ents[0]["vllm"] == "1.0" and ents[0]["model"] == "m"
           and ents[0]["brief"] == "brief 줄" and ents[0]["published"] is True)
        ck("출처 표시", ents[0]["source"] == "remote-derived")
        ck("★음성대조 로컬 부재 태그는 합성하지 않고 죽는다",
           expect_die(lambda: derive_entries(repo, ["hint/1.0/m/a/qx-len1-kvfp8", "hint/9.9/none/x/q"])) == "raised")
        # 4세그먼트(구세대) 이름은 이제 규약 밖이다 — CP7 에서 레시피 칸이 생겼다.
        ck("★음성대조 규약 밖 태그명 거부(4세그먼트 구세대)",
           expect_die(lambda: derive_entries(repo, ["hint/0.1/m/a"])) == "raised")
        git("tag", "-f", "lw", cwd=repo)  # lightweight
        git("tag", "-a", "-f", "hint/1.0/m/b/qy", "-m", "b", cwd=repo)
        git("tag", "-d", "hint/1.0/m/b/qy", cwd=repo)
        git("tag", "hint/1.0/m/b/qy", cwd=repo)  # lightweight hint 태그
        ck("★음성대조 lightweight 태그 거부",
           expect_die(lambda: derive_entries(repo, ["hint/1.0/m/b/qy"])) == "raised")

        # ── 원격 조회 실패는 **캐시로 대체하지 않고 죽는다**(D1.1 fail-closed).
        # 2026-09-01 변이시험이 이 공백을 찾았다: E2E 프로브는 원격이 항상 성공하므로
        # "실패 시 조용히 빈 목록 반환"이라는 변이를 **초록으로 통과시켰다**. 침묵 폴백은
        # 카탈로그를 조용히 낡게 만들고, 낡은 카탈로그는 부재와 구분되지 않는다.
        nope = str(Path(tmp) / "does-not-exist.git")
        ck("★음성대조 원격 조회 실패 → 중단(캐시 폴백 금지)",
           expect_die(lambda: remote_hint_tags(repo, nope)) == "raised")

        # 원격은 살아 있는데 hint 태그가 0건 → 빈 카탈로그를 발행하지 않는다
        bare = Path(tmp) / "empty.git"
        git("init", "--bare", "-q", str(bare))
        ck("★음성대조 원격 hint 태그 0건 → 빈 카탈로그 거부",
           expect_die(lambda: remote_hint_tags(repo, str(bare))) == "raised")

        # --allow-empty: 0건을 **선언하면** 통과한다(빈 목록). 태그 전량 폐기 직후의 재파생 경로.
        ck("allow_empty=True → 0건 정상 통과(빈 목록)",
           remote_hint_tags(repo, str(bare), allow_empty=True) == [])
        # 단 조회 **실패**는 allow_empty 로도 열리지 않는다 — '0건'과 '모른다'는 다르다.
        ck("★음성대조 allow_empty 여도 원격 조회 실패는 중단",
           expect_die(lambda: remote_hint_tags(repo, nope, allow_empty=True)) == "raised")

        # 살아 있는 원격 + 태그 1건 → 정상 조회
        git("push", "-q", str(bare),
            "refs/tags/hint/1.0/m/a/qx-len1-kvfp8:refs/tags/hint/1.0/m/a/qx-len1-kvfp8", cwd=repo)
        ck("살아 있는 원격에서 태그 조회", remote_hint_tags(repo, str(bare)) == ["hint/1.0/m/a/qx-len1-kvfp8"])

    failed = [n for n, ok in checks if not ok]
    for n, ok in checks:
        print(f"  {'ok  ' if ok else 'FAIL'} {n}")
    print(f"[hint_catalog] self-test {len(checks) - len(failed)}/{len(checks)} "
          + ("PASS" if not failed else "FAIL"))
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description="hint 카탈로그를 원격 발행 태그에서 파생")
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--repo", default=".")
    sub = ap.add_subparsers(dest="cmd")
    d = sub.add_parser("derive", help="원격 태그 → index.json + HINTS.md 재생성")
    d.add_argument("--remote", default="origin")
    d.add_argument("--generated-kst", required=True, help="시각은 주입만 받는다(벽시계 금지)")
    d.add_argument("--dry-run", action="store_true")
    d.add_argument("--allow-empty", action="store_true",
                   help="원격 hint 태그 0건을 **정상**으로 선언하고 빈 카탈로그를 파생한다"
                        "(태그 전량 폐기 직후용 · 기본은 fail-closed). 조회 실패는 이 플래그로도 열리지 않는다.")
    d.set_defaults(fn=cmd_derive)
    a = ap.parse_args()
    if a.self_test:
        return _run_self_test()
    if not getattr(a, "fn", None):
        ap.print_help()
        return 2
    return a.fn(a)


if __name__ == "__main__":
    raise SystemExit(main())
