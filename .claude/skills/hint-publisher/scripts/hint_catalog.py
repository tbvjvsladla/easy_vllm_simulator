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
    (조회한 그 원격에서 `refs/tags/hint/*` 를 fetch 하라고 말하고 전용 종료코드
    `EXIT_LOCAL_TAG_OBJECT_MISSING` 으로 끝난다 · 자동 fetch 는 하지 않는다). 합성 금지가 여기서도 그대로다.
"""
from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

HINTS_MARKER = "<!-- hint-index:rows -->"
CATALOG_COLUMNS = ("태그", "vLLM", "모델", "arch", "bench_mode", "결손", "brief")
CENTRAL_FLAG = "hints/.central_authority"
# ★ 2026-09-04(CP7 · plan_26090415 §7.5 M1): 태그가 5세그먼트가 됐다 —
#   `hint/<vllm>/<model>/<arch>/<recipe>`. 이 파일은 `hint_tag.py` 의 9곳을 세던 개정
#   표면에 **들어 있지 않았다**(계획의 실측 누락). 카탈로그 파생은 원격 태그 이름을
#   직접 파싱하므로, 여기를 안 고치면 새 이름이 전부 "규약 밖"으로 거부되어 발행은
#   되는데 카탈로그에 못 들어가는 상태가 된다.
TAG_SHAPE = re.compile(
    r"^hint/(?P<vllm>[^/]+)/(?P<model>[^/]+)/(?P<arch>[^/]+)/(?P<recipe>[^/]+)$")

# 원격에만 있는 태그의 **로컬 태그 오브젝트 부재** 전용 종료코드(2026-09-14 · ⑧-pre D2 S7). 다른 실패(원격 조회 실패·
#   규약 밖 이름·lightweight 태그 = 기본 2)와 갈라야 호출부(종료 시퀀스 ⑤)가 **원인을 말하고** 사람에게 fetch 명령을
#   건넬 수 있다 -- 종전에는 모든 실패가 2 라 "원인은 위 출력" 한 줄로 뭉개졌다. 호출부는 이 상수를 이 파일에서 읽는다.
EXIT_LOCAL_TAG_OBJECT_MISSING = 4


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

def payload_doc(repo: Path, tag: str) -> "dict | None":
    """태그 페이로드 커밋의 `PAYLOAD.json`. 없거나 못 읽으면 None(표시 평면 — 판정 권위는 `hint_tag verify`)."""
    out = subprocess.run(["git", "-C", str(repo), "cat-file", "-p", f"{tag}^{{}}:PAYLOAD.json"],
                         capture_output=True, text=True)
    if out.returncode != 0:
        return None
    try:
        doc = json.loads(out.stdout)
    except ValueError:
        return None
    return doc if isinstance(doc, dict) else None


def payload_missing(repo: Path, tag: str) -> list:
    """태그 페이로드가 **스스로 선언한** 결손 사유코드. 읽을 수 없으면 빈 목록이다.

    부재(페이로드가 없거나 낡은 태그)와 "결손 0"을 같은 값으로 접는다는 점은 알고 쓴다 —
    카탈로그는 **비권위 캐시**이므로 여기서 fail-closed 하면 발행되지 않은 사실이 아니라
    카탈로그 갱신이 막힌다. 판정의 권위는 `hint_tag verify` 이고 여기는 표시다.
    """
    doc = payload_doc(repo, tag)
    missing = doc.get("missing") if isinstance(doc, dict) else None
    return sorted(m for m in (missing or []) if isinstance(m, str))


# ── bench_mode 파생 컬럼 (2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q10) ──────────────────────────────────────
# 등급은 태그 이름에 새기지 않는다(5세그먼트 불변). 카탈로그는 페이로드가 스스로 적은 `measurement_config`(hint_collect 가
# 리포트 측정 구성 표·인증서에서 파싱한 것)에서 **결정론으로** 한 칸을 파생한다. 과거 태그에는 그 키가 없다 — 없는 것을
# full 로도 lite 로도 접지 않고 `미기재` 로 **정직하게** 표시한다. 키는 있는데 값이 없으면(판정 기록을 못 읽은 측정) `미확정`.
BENCH_MODE_ABSENT = "미기재"
BENCH_MODE_UNDETERMINED = "미확정"


def bench_mode_cell(doc: "dict | None") -> "tuple[str, str]":
    """PAYLOAD.json → (카탈로그 칸, 출처). 카탈로그 `hint_catalog` 와 로컬 색인 `hint_tag._hints_row` 가 함께 쓴다(한 곳 소유)."""
    mc = doc.get("measurement_config") if isinstance(doc, dict) else None
    if not isinstance(mc, dict):
        return BENCH_MODE_ABSENT, "absent(PAYLOAD.measurement_config 없음 — 측정 구성 기재 신설 전 페이로드)"
    mode, kind, reason = mc.get("bench_mode"), mc.get("bench_mode_kind"), mc.get("downgrade_reason")
    if mode == "full":
        cell = "full"
    elif mode == "lite" and reason:
        cell = f"lite(강등·{reason})"
    elif mode == "lite" and kind == "declared-lite":
        cell = "lite(선언)"
    elif mode == "lite":
        cell = "lite"
    else:
        cell = BENCH_MODE_UNDETERMINED
    return cell, f"payload(PAYLOAD.measurement_config · {mc.get('source') or '출처 미표시'})"


def _main_worktree(repo: Path) -> Path:
    """`repo` 가 속한 저장소의 **주 워크트리** 경로. 안내 명령에 찍을 자리다.

    왜(2026-09-14 ⑧-pre D2 리뷰): 종료 시퀀스 ⑤ 는 파생기를 **임시 워크트리**(`<repo>.wt-<반대>`)에서 돌리고 곧바로
    지운다. 안내가 `git -C <임시 워크트리>` 를 찍으면 사람이 읽는 순간 그 경로는 없다. 태그 ref 는 워크트리가 아니라
    저장소 공용이므로 주 워크트리에서 fetch 해도 같다. 찾지 못하면 `repo` 를 그대로 쓴다(안내 문자열일 뿐 판정이 아니다).
    """
    listing = git("worktree", "list", "--porcelain", cwd=repo, check=False)
    first = listing.stdout.splitlines()[0] if listing.returncode == 0 and listing.stdout else ""
    return Path(first[len("worktree "):]) if first.startswith("worktree ") else repo


def derive_entries(repo: Path, tags: list[str], remote: str | None = None) -> list[dict]:
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
        _pdoc = payload_doc(repo, tag)
        _bm, _bm_src = bench_mode_cell(_pdoc)
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
            "bench_mode": _bm,
            "bench_mode_source": _bm_src,
            "published": True,          # 원격에 있다 = 발행됐다. 이것이 유일한 근거다.
            "source": "remote-derived",  # 출처 표시(헌법 §결정론 규율)
        })
    if missing_local:
        # 해소 명령은 **조회한 그 원격**을 가리켜야 한다 -- `git fetch --tags` 는 기본 원격(origin)을 보므로
        #   `--remote` 가 다른 이름이면 엉뚱한 곳을 긁고 같은 실패를 되풀이한다(2026-09-14 실측: 원격 전용 태그 10건).
        fetch = (f"git -C {_main_worktree(repo)} fetch {remote} 'refs/tags/hint/*:refs/tags/hint/*'" if remote
                 else "git fetch <원격> 'refs/tags/hint/*:refs/tags/hint/*'")
        die("원격에 있으나 **로컬에 태그 오브젝트가 없다** — brief 를 지어낼 수 없다(합성 금지):\n  "
            + "\n  ".join(missing_local[:10])
            + f"\n  (총 {len(missing_local)}건)  →  `{fetch}` 후 다시 실행하라(자동 fetch 하지 않는다).",
            code=EXIT_LOCAL_TAG_OBJECT_MISSING)
    return entries


def render_rows(entries: list[dict]) -> str:
    # `결손` 열(2026-09-07): "무엇을 모른 채 발행됐는가" 가 카탈로그에서 보여야 한다. 본문 슬롯과
    # PAYLOAD.missing[] 에만 있으면 사람이 태그를 열어야 알 수 있고, 그러면 비교가 불가능하다.
    # `bench_mode` 열(2026-09-14): 등급은 이름이 아니라 이 파생 컬럼이 말한다. 열 순서·이름은 hint_tag.HINTS_COLUMNS 와 같다
    #   (두 렌더러가 갈라지면 HINTS.md 가 깨진다 — 2026-09-07 실측 · 두 자체검사가 각자 열 수를 대조한다).
    rows = ["| " + " | ".join(CATALOG_COLUMNS) + " |", "|" + "|".join("---" for _ in CATALOG_COLUMNS) + "|"]
    for e in entries:
        miss = e.get("missing") or []
        cell = "—" if not miss else md_cell(" · ".join(miss))
        rows.append(f"| `{e['tag']}` | {md_cell(e['vllm'])} | {md_cell(e['model'])} | "
                    f"{md_cell(e['arch'])} | {md_cell(e.get('bench_mode') or BENCH_MODE_ABSENT)} | {cell} | "
                    f"{md_cell(e['brief'])} |")
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
    entries = derive_entries(repo, tags, remote=a.remote)
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

    def die_code(fn):
        try:
            fn()
        except SystemExit as exc:
            return exc.code
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
        ck("★과거 태그(PAYLOAD 없음)의 bench_mode 는 '미기재' 로 정직하게 표시한다(full·lite 로 접지 않는다)",
           ents[0]["bench_mode"] == BENCH_MODE_ABSENT and ents[0]["bench_mode_source"].startswith("absent("))
        _rows = render_rows(ents).splitlines()
        ck("카탈로그 행은 헤더와 같은 열 수이고 bench_mode 열을 갖는다",
           _rows[0].count("|") == len(CATALOG_COLUMNS) + 1 and _rows[2].count("|") == len(CATALOG_COLUMNS) + 1
           and "bench_mode" in _rows[0] and f"| {BENCH_MODE_ABSENT} |" in _rows[2])
        # 페이로드가 측정 구성을 적은 태그 — 파생이 결정론으로 칸을 만든다
        for _doc, _want in (({"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "declared-lite",
                                                     "downgrade_reason": None, "source": "bench_report(x.md)"}}, "lite(선언)"),
                            ({"measurement_config": {"bench_mode": "lite", "bench_mode_kind": "downgraded-lite",
                                                     "downgrade_reason": "blackbox_kill"}}, "lite(강등·blackbox_kill)"),
                            ({"measurement_config": {"bench_mode": "full"}}, "full"),
                            ({"measurement_config": {"bench_mode": None}}, BENCH_MODE_UNDETERMINED),
                            ({"missing": []}, BENCH_MODE_ABSENT), (None, BENCH_MODE_ABSENT)):
            ck(f"bench_mode 파생 칸: {_want}", bench_mode_cell(_doc)[0] == _want)
        _ptree = repo / "ptree"
        _ptree.mkdir()
        (_ptree / "PAYLOAD.json").write_text(json.dumps({"missing": ["BENCH_MODE_LITE"], "measurement_config": {
            "bench_mode": "lite", "bench_mode_kind": "declared-lite", "downgrade_reason": None,
            "source": "bench_report(r.md)"}}), encoding="utf-8")
        _blob = git("hash-object", "-w", str(_ptree / "PAYLOAD.json"), cwd=repo).stdout.strip()
        _tree = subprocess.run(["git", "-C", str(repo), "mktree"], input=f"100644 blob {_blob}\tPAYLOAD.json\n",
                               capture_output=True, text=True).stdout.strip()
        _commit = git("commit-tree", _tree, "-m", "payload", cwd=repo).stdout.strip()
        git("tag", "-a", "hint/1.0/m/c/qlite", _commit, "-m", "lite brief\n\n## 1. 벽\n본문", cwd=repo)
        _lite = derive_entries(repo, ["hint/1.0/m/c/qlite"])[0]
        ck("★원격 태그 페이로드에서 bench_mode 열을 파생한다(lite 선언) · 결손 열에 BENCH_MODE_LITE",
           _lite["bench_mode"] == "lite(선언)" and _lite["missing"] == ["BENCH_MODE_LITE"]
           and _lite["bench_mode_source"].startswith("payload("))
        ck("★음성대조 로컬 부재 태그는 합성하지 않고 죽는다",
           expect_die(lambda: derive_entries(repo, ["hint/1.0/m/a/qx-len1-kvfp8", "hint/9.9/none/x/q"])) == "raised")
        # ⑧-pre D2 S7: 원인별 종료코드 -- 로컬 오브젝트 부재만 전용 코드이고 다른 실패는 기본 코드다(호출부가 원인을 말한다)
        ck("★S7 로컬 태그 오브젝트 부재 = 전용 종료코드",
           die_code(lambda: derive_entries(repo, ["hint/9.9/none/x/q"], remote="fx")) == EXIT_LOCAL_TAG_OBJECT_MISSING)
        ck("★S7 음성대조 규약 밖 이름은 전용 코드가 아니다(원인을 섞지 않는다)",
           die_code(lambda: derive_entries(repo, ["hint/0.1/m/a"])) not in (None, EXIT_LOCAL_TAG_OBJECT_MISSING))
        import io, contextlib
        _buf = io.StringIO()
        with contextlib.redirect_stderr(_buf):
            die_code(lambda: derive_entries(repo, ["hint/9.9/none/x/q"], remote="fx"))
        ck("★S7 해소 명령은 조회한 그 원격을 가리킨다(기본 원격 `--tags` 가 아니다)",
           "fetch fx 'refs/tags/hint/*:refs/tags/hint/*'" in _buf.getvalue() and "--tags" not in _buf.getvalue())
        # 임시 워크트리에서 파생해도 안내는 **주 워크트리**를 가리킨다(종료 시퀀스 ⑤ 는 파생 직후 워크트리를 지운다)
        _wt = Path(tmp) / "r.wt-other"
        git("worktree", "add", "-q", "--detach", str(_wt), cwd=repo)
        _buf = io.StringIO()
        with contextlib.redirect_stderr(_buf):
            die_code(lambda: derive_entries(_wt, ["hint/9.9/none/x/q"], remote="fx"))
        ck("★S7 임시 워크트리에서 파생해도 fetch 안내는 주 워크트리 경로다(곧 지워질 경로를 찍지 않는다)",
           f"git -C {repo.resolve()} fetch fx" in _buf.getvalue() and str(_wt) not in _buf.getvalue())
        git("worktree", "remove", "--force", str(_wt), cwd=repo)
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
