#!/usr/bin/env python3
"""bootstrap_families.py — hint family 색인 부트스트랩/감사 (plan_26082008 R2·R7 · plan_26082009 D9).

**왜 별도 파일인가**: `hint_tag reindex` 는 "태그 = 진실원천 → index.json 재생성"이므로 index 에
family 를 두면 **재생성이 그것을 파괴한다**. family 는 사람이 승인한 정보라 태그에서 재도출되지
않으므로 `hints/families.json`(추적·reindex 불가침)에 둔다.

**파생 권위**(plan_26082008 §3): 모델 디렉터리에 번들된 HF 원본 카드(README.md) 의 `base_model` /
`base_model_relation`. `crosscheck_model_card.py` 와 같은 권위를 쓴다(폐쇄망·네트워크 호출 없음).

**판별자 우선순위**: base_model_relation > 이름 접두 > 수동. 2026-08-20 교차검증에서 앞의 둘은
충돌 0건이었다(quantized 3/3 접두O · finetune 3/3 접두X). 단 접두는 변종 토큰이 **중간 삽입**되면
위음성을 낸다(`…-Instruct-2512 ← …-Base-2512` · `…-DFlash-NVFP4 ← …-NVFP4`) → 그래서 자동은
**후보 제안까지**이고 색인에 남는 값은 사람 승인분이며 `source` 로 출처를 표시한다(헌법 §결정론 규율).

NAS 부재 환경(배포처)에서도 동작한다 — 카드를 못 읽으면 해당 항목을 `manual` 로 남기고 계속한다.
"""
from __future__ import annotations
import argparse, glob, json, os, re, subprocess, sys

REPO = os.path.dirname(os.path.dirname(os.path.dirname(os.path.dirname(
    os.path.dirname(os.path.abspath(__file__))))))
FAMILIES_FILE = os.path.join(REPO, "hints", "families.json")


def _norm(s: str) -> str:
    return re.sub(r"[^a-z0-9]", "", s.lower())


def parse_frontmatter(txt: str) -> dict[str, list[str]]:
    """최소 YAML 프론트매터 파서(스칼라 + 리스트만). 리스트형 `base_model:\\n  - repo` 를 놓치면
    데이터 부재로 오독하므로 두 형태를 모두 받는다(2026-08-20 실제 오독 발생)."""
    if not txt.startswith("---"):
        return {}
    end = txt.find("\n---", 3)
    if end < 0:
        return {}
    out: dict[str, list[str]] = {}
    key = None
    for line in txt[3:end].split("\n"):
        if re.match(r"^\S+:", line):
            k, _, v = line.partition(":")
            key = k.strip()
            v = v.strip()
            out[key] = [v] if v else []
        elif key and re.match(r"^\s*-\s+", line):
            out.setdefault(key, []).append(re.sub(r"^\s*-\s+", "", line).strip())
    return out


def scan_cards(roots: list[str]) -> dict[str, dict]:
    cards: dict[str, dict] = {}
    for r in roots:
        if not os.path.isdir(r):
            continue
        pats = [os.path.join(r, *(["*"] * d), "README.md") for d in (2, 3)]
        for rd in sorted(sum((glob.glob(p) for p in pats), [])):
            d = os.path.basename(os.path.dirname(rd))
            try:
                fm = parse_frontmatter(open(rd, encoding="utf-8", errors="replace").read(8000))
            except OSError:
                continue
            if not fm:
                continue
            cards[d.lower()] = {
                "base": (fm.get("base_model") or [None])[0],
                "rel": (fm.get("base_model_relation") or [None])[0],
            }
    return cards


def classify(slug: str, card: dict | None) -> tuple[str, str, str]:
    """→ (family_id, relation, source). 판별자 우선순위는 모듈 docstring 참조."""
    if card is None or not card.get("base"):
        # ★ 'manual' 로 라벨하지 않는다(2026-09-01). 'manual' 은 **사람이 선언했다**는 뜻인데
        # 여기는 **판정할 근거가 없다**는 뜻이다. 둘을 같은 값으로 적으면 출처 표시가 거짓을
        # 말한다(헌법 §결정론 규율). 실측: 카드가 없는 이 호스트에서 36/36 이 'manual' 로 찍혀
        # 마치 전부 사람이 승인한 것처럼 보였다.
        return slug, "root", "unresolved"
    base = card["base"]
    base_name = base.split("/")[-1]
    rel_decl = card.get("rel")
    if rel_decl == "finetune":                      # 명시 선언 — 별 family
        return slug, "root", "base_model_relation"
    if rel_decl == "quantized":
        return _norm_id(base_name), "quantized", "base_model_relation"
    if rel_decl == "draft":
        return _norm_id(base_name), "draft", "base_model_relation"
    # 접두 규칙(대체 신호). 위음성 있음 — 걸리지 않으면 단독 family 로 두고 사람이 본다.
    if _norm(slug).startswith(_norm(base_name)) or _norm(base_name).startswith(_norm(slug)):
        return _norm_id(base_name), relation_of(slug, base_name), "name_prefix"
    return slug, "root", "unresolved"


def _norm_id(repo_name: str) -> str:
    return repo_name.lower()


# 슬러그 토큰이 이 집합에 걸리면 양자화 파생으로 본다. **닫힌 목록 tripwire** — 새 양자화
# 포맷이 나오면 여기 추가가 강제되고, 그때 사람이 한 번 본다(헌법 §4종 안티패턴 판정표
# "하드코딩 = tripwire 는 정당").
QUANT_TOKENS = frozenset({
    "fp8", "fp4", "nvfp4", "mxfp4", "int4", "int8", "w4a16", "w8a8", "w4a8",
    "awq", "gptq", "autoround", "bnb", "gguf", "quantized", "compressed",
})
# SD 외장 초안 모델을 가리키는 **슬러그 토큰**. relation_of 의 후보 생성에만 쓴다 —
# SD *능력* 판정에는 더 이상 쓰지 않는다(아래 extract_sd 참조).
DRAFT_TOKENS = frozenset({"dflash", "draft", "eagle", "eagle3", "mtp"})


def _tokens(name: str) -> set[str]:
    return {t for t in re.split(r"[^A-Za-z0-9]+", name.lower()) if t}


def relation_of(slug: str, base_name: str) -> str:
    """파생 슬러그가 base 에 대해 갖는 관계. 이름만으로 판정하므로 **후보**이며,
    확정은 사람 승인분(`source: manual`)이 덮는다."""
    if _norm(slug) == _norm(base_name):
        return "alias"
    extra = _tokens(slug) - _tokens(base_name)
    if extra & DRAFT_TOKENS:
        return "draft"
    if extra & QUANT_TOKENS:
        return "quantized"
    return "variant"


# ── SD(speculative decoding) capability 추출 ────────────────────────────────
# plan_26082008 §3.3 R8. 수신자의 1순위 질의("그래서 MTP(SD) 있음?")를 **태그 본문을 열지 않고**
# 답하기 위한 색인 필드다. 실측 2026-08-20: 본문에서 SD 를 논하는 태그 34/49 인데 index 가 드러낼
# 수 있는 것은 9/48(brief 자유문에 우연히 섞인 것)이었다.
#
# **합성 금지**: 수치(`accept_len`·`spec_tokens`)는 본문에 실제로 있을 때만 채우고 `measured` 로
# 표시한다. 없으면 비운다 — 추정치를 넣으면 하류가 실측으로 오독한다(헌법 §측정 > 공식).
_RE_NUM_SPEC = re.compile(r'num_speculative_tokens["\s:·]*(\d+)')
_RE_ACCEPT = re.compile(r'accept_len["\s:·=]*([0-9]+\.[0-9]+)')


def extract_sd(body: str) -> dict:
    """태그 본문에서 SD 관련 **수치만** 수확한다. 능력(mode) 판정은 **하지 않는다**.

    ★ 2026-09-01 개정: 종전엔 산문 정규식(`DFlash|eagle-?3?|draft model|초안 모델` 및 'no spec'
    부정문)으로 SD **능력**과 사용여부를 추론했다. 정규식이 원리적으로 못 하는 일이고 위음성이
    실증됐다 — 본문에 "MTP n=3 … 2.18×" 가 있는데 `collect --sd-only` 가 그 태그를 놓쳤다.
    능력 판정은 인용과 함께 **Agent** 가 하고 `families.json` 의 `source: judgment` 항목이 담는다.

    여기 남는 것은 **본문에 실제로 적힌 숫자를 읽는 일**뿐이다 — 의미 해석이 아니라 사실 수확이며
    결정론↔Agent 경계의 결정론 쪽이다. 없으면 비운다(합성 금지 · 추정치를 넣으면 하류가 실측으로
    오독한다).
    """
    ns = _RE_NUM_SPEC.search(body)
    acc = _RE_ACCEPT.search(body)
    spec_tokens = int(ns.group(1)) if ns else None
    accept_len = float(acc.group(1)) if acc else None

    if spec_tokens is not None:
        enabled, enabled_source = spec_tokens > 0, "measured:num_speculative_tokens"
    elif accept_len is not None and accept_len > 1.0:
        enabled, enabled_source = True, "measured:accept_len"   # 초안 수용 실측 = 켜져 있었다
    else:
        enabled, enabled_source = None, "needs_judgment"        # 숫자가 없다 = 여기서는 모른다

    return {"enabled": enabled, "enabled_source": enabled_source,
            "mode": "needs_judgment",          # 능력 판정은 Agent 소관 (인용 필수)
            "spec_tokens": spec_tokens, "accept_len": accept_len,
            "measured": accept_len is not None, "source": "tag_body_numeric_scan"}


def scan_tag_sd() -> dict[str, dict]:
    tags = [t for t in subprocess.run(["git", "tag", "-l", "hint/*"], cwd=REPO,
                                      capture_output=True, text=True).stdout.split() if t]
    out = {}
    for t in tags:
        body = subprocess.run(["git", "cat-file", "-p", t], cwd=REPO,
                              capture_output=True, text=True).stdout
        out[t] = extract_sd(body)
    return out


def existing_hint_slugs() -> list[str]:
    out = subprocess.run(["git", "tag", "-l", "hint/*"], cwd=REPO,
                         capture_output=True, text=True).stdout.split()
    return sorted({t.split("/")[2] for t in out if t.count("/") == 3})


def build(roots: list[str]) -> dict:
    cards = scan_cards(roots)
    slugs = existing_hint_slugs()
    fams: dict[str, dict] = {}
    for slug in slugs:
        ns = _norm(slug)
        cand = [k for k in cards if _norm(k) == ns] or \
               [k for k in cards if _norm(k).startswith(ns) or ns.startswith(_norm(k))]
        card = cards[sorted(cand, key=lambda k: abs(len(_norm(k)) - len(ns)))[0]] if cand else None
        fid, relation, source = classify(slug, card)
        ev = ""
        if source != "unresolved" and card:
            ev = f"card base_model={card['base']}" + (f" relation={card['rel']}" if card.get("rel") else "")
        elif card is None:
            # 카드 부재는 **사실**이지 판정이 아니다. 종전엔 특정 모델 5종을 `KNOWN_CARDLESS` 에
            # 손으로 적어 "이기종 발행처"라고 단정했는데, 그건 이 스크립트가 알 수 없는 것이다
            # (2026-09-01 제거). 부재 사실만 적고 이유는 Agent 가 인용과 함께 판단한다.
            ev = "모델 카드를 찾지 못함(--root 범위 밖이거나 미보유) — 사유는 판정 대상"
        f = fams.setdefault(fid, {"root_repo": None, "members": [], "sd_capability":
                                  {"mode": "unknown", "draft_slug": None}})
        # `repo` = 실제로 서빙한 모델의 HF repo 이름(NAS 카드 디렉터리명). 슬러그가 이것과 다를 수
        # 있으므로(예: 태그 `hy3` ↔ repo `Hy3-NVFP4-W4A16`) 별도로 적는다 — R1 슬러그 파생 검사가
        # family 연속성을 판정할 때 이 필드로 이어붙인다.
        f["members"].append({"slug": slug, "relation": relation, "source": source,
                             "evidence": ev, "repo": (cand and sorted(cand, key=lambda k: abs(len(_norm(k)) - len(ns)))[0]) or None})
        if card and card.get("base") and source != "unresolved":
            f["root_repo"] = card["base"]
    # 정규화하면 같아지는 멤버 = ②철자 갈림 사고. 색인이 이를 명시해야 수신자가
    # "왜 같은 모델이 두 슬러그인가"를 안다.
    for v in fams.values():
        seen: dict[str, str] = {}
        for m in v["members"]:
            n = _norm(m["slug"])
            if n in seen:
                for x in v["members"]:
                    if _norm(x["slug"]) == n:
                        x["relation"] = "alias"
                        x["note"] = f"철자 갈림(정규화 시 동일): {seen[n]} ↔ {m['slug']}"
            seen[n] = m["slug"]
    fams = {k: v for k, v in fams.items() if v["members"]}
    for v in fams.values():
        v["members"].sort(key=lambda m: m["slug"])
    return {"schema_version": 1,
            "note": "hint family 색인. reindex 불가침(plan_26082009 D9). 출처는 members[].source.",
            "families": dict(sorted(fams.items())),
            "tag_sd": {}}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", action="append", default=[],
                    help="모델 카드를 찾을 NAS 루트(반복 가능). manifest 의 nas_model_path·quant_model_path.")
    ap.add_argument("--write", action="store_true", help="hints/families.json 에 기록(미지정 시 dry-run)")
    ap.add_argument("--scan-sd", action="store_true", help="태그 본문에서 SD capability 를 추출해 tag_sd 채움")
    a = ap.parse_args()
    data = build(a.root)
    if a.scan_sd:
        data["tag_sd"] = scan_tag_sd()
        # ★ family 층 mode 집계는 제거했다. 종전엔 태그 본문 산문에서 추론한 mode 를 family 로
        # 올려 `source: tag_body_scan` 으로 박았는데, 그 추론 자체가 위음성이 실증됐다.
        # 능력 판정은 Agent 가 인용과 함께 하고, 아래 보존 규칙이 그 판정을 덮지 않게 지킨다.
        _need = sum(1 for v in data["tag_sd"].values() if v["enabled"] is None)
        print(f"[sd] 태그 {len(data['tag_sd'])}종 스캔 · 수치 확보 "
              f"{sum(1 for v in data['tag_sd'].values() if v['measured'])}종 · "
              f"판정대기(enabled 미상) {_need}종 → Agent 가 본문을 읽고 판정해야 한다")
    _JUDGED = ("manual", "judgment")
    n_derived = sum(1 for f in data["families"].values() for m in f["members"]
                    if m["source"] not in _JUDGED and m["source"] != "unresolved")
    n_judged = sum(1 for f in data["families"].values() for m in f["members"] if m["source"] in _JUDGED)
    n_unres = sum(1 for f in data["families"].values() for m in f["members"] if m["source"] == "unresolved")
    multi = {k: v for k, v in data["families"].items() if len(v["members"]) > 1}
    print(f"[families] family {len(data['families'])}종 · 멤버 {n_derived + n_judged + n_unres}종 "
          f"(카드파생 {n_derived} · 확정판정 {n_judged} · **판정대기 {n_unres}**)")
    if n_unres:
        print(f"[families] ⚠ 판정대기 {n_unres}종 — 모델 카드가 없어 기계가 계보를 알 수 없다. "
              f"family 귀속은 Agent 가 인용과 함께 판정하고 members[].source 를 'judgment' 로 적는다.")
    print(f"[families] 복수-멤버 family {len(multi)}종:")
    for k, v in multi.items():
        print(f"   {k}")
        for m in v["members"]:
            print(f"      - {m['slug']:<34} {m['relation']:<10} [{m['source']}]")
    if a.write:
        os.makedirs(os.path.dirname(FAMILIES_FILE), exist_ok=True)
        prev = {}
        if os.path.isfile(FAMILIES_FILE):
            prev = json.loads(open(FAMILIES_FILE, encoding="utf-8").read())
        # 스캔하지 않은 실행이 기존 tag_sd 를 지우면 안 된다(사람 승인분 보존).
        # 반대로 스캔한 실행의 결과를 이전 값으로 덮어도 안 된다 — 2026-08-20 실제 발생.
        if not a.scan_sd:
            data["tag_sd"] = prev.get("tag_sd", {})
        # ★ 부분보존 결함 교정(2026-09-01 · 감사 ⑬): 종전엔 `tag_sd` 만 보존하고
        # `families[*].sd_capability` 는 보존하지 않았다. 그래서 `--scan-sd` 없이 한 번만 돌려도
        # 사람/Agent 가 확정한 능력 판정이 기계 기본값으로 **조용히 되돌아갔다**. 판정은 기계가
        # 만든 것이 아니므로 기계가 지울 수도 없다 — judgment/manual 출처는 무조건 이어받는다.
        _prev_fams = prev.get("families") or {}
        # ★ 확정 판정 멤버 보존(2026-09-01). 종전엔 스크립트 안의 `MANUAL_FAMILIES` 상수가 매
        # 실행마다 사람 선언을 다시 주입했다. 그 상수를 없앴으므로(특정 모델명 하드코딩),
        # **판정의 거처를 파일로 옮긴다** — `members[].source` 가 manual|judgment 인 항목은
        # 기계가 만든 것이 아니므로 기계가 지우지 않는다. 이 보존이 없으면 Agent 가 내린 family
        # 판정이 다음 실행에서 조용히 사라진다(= 삭제한 상수가 하던 일을 아무도 안 하게 된다).
        _judged_members: dict[str, list] = {}
        for _fid, _pf in _prev_fams.items():
            _keep = [m for m in (_pf.get("members") or []) if m.get("source") in ("manual", "judgment")]
            if _keep:
                _judged_members[_fid] = _keep
        _claimed = {m["slug"] for ms in _judged_members.values() for m in ms}
        if _claimed:
            for _f in data["families"].values():
                _f["members"] = [m for m in _f["members"] if m["slug"] not in _claimed]
            for _fid, _ms in _judged_members.items():
                _tgt = data["families"].setdefault(_fid, {"root_repo": None, "members": [],
                                                          "sd_capability": {"mode": "needs_judgment",
                                                                            "draft_slug": None}})
                _tgt["members"] = sorted(_tgt["members"] + _ms, key=lambda m: m["slug"])
            data["families"] = {k: v for k, v in sorted(data["families"].items()) if v["members"]}
            print(f"[families] 이전 판정 보존: 멤버 {len(_claimed)}건 (source=judgment|manual)")
        _kept = 0
        for _fid, _f in data["families"].items():
            _pc = (_prev_fams.get(_fid) or {}).get("sd_capability") or {}
            if _pc.get("source") in ("judgment", "manual"):
                _f["sd_capability"] = _pc
                _kept += 1
        if _kept:
            print(f"[families] 이전 판정 보존: sd_capability {_kept}건 (source=judgment|manual)")
        with open(FAMILIES_FILE, "w", encoding="utf-8") as fh:
            json.dump(data, fh, ensure_ascii=False, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"[families] 기록: {os.path.relpath(FAMILIES_FILE, REPO)}")
    else:
        print("[families] dry-run (--write 로 기록)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
