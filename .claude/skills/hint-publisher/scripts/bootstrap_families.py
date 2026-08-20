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

# 수동 선언 — 자동 판별자가 닿지 않는 것만. 근거를 반드시 병기한다.
MANUAL_FAMILIES: dict[str, dict] = {
    # 사용자 D1(2026-08-20): "살짝만 교정한 변종모델까지는 같은 모델".
    #   양쪽 카드 모두 base_model 미선언이라 자동 판별 불가 → 수동.
    "deepseek-v4-flash": {
        "members": {"deepseek-v4-flash": "revision", "deepseek-v4-flash-0731": "revision"},
        "evidence": "사용자 D1(2026-08-20) · 0731=정식판, 프리뷰와 가중치 상이하나 동일 모델계열",
    },
}

# 이 호스트 NAS 에 카드가 없는 발행처(이기종 rtxpro6000/rtx5090). 단독 family 로 둔다.
KNOWN_CARDLESS = {
    "hyperclovax-think-32b", "minicpm5-1b", "minicpm5-1b-base",
    "mistral-small-4-119b", "nemotron-3-super-120b-a12b-nvfp4",
}


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
        return slug, "root", "manual"
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
    return slug, "root", "manual"


def _norm_id(repo_name: str) -> str:
    return repo_name.lower()


# 슬러그 토큰이 이 집합에 걸리면 양자화 파생으로 본다. **닫힌 목록 tripwire** — 새 양자화
# 포맷이 나오면 여기 추가가 강제되고, 그때 사람이 한 번 본다(헌법 §4종 안티패턴 판정표
# "하드코딩 = tripwire 는 정당").
QUANT_TOKENS = frozenset({
    "fp8", "fp4", "nvfp4", "mxfp4", "int4", "int8", "w4a16", "w8a8", "w4a8",
    "awq", "gptq", "autoround", "bnb", "gguf", "quantized", "compressed",
})
# SD 외장 초안 모델을 가리키는 토큰(plan_26082008 §3.3 D7).
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
_RE_MTP = re.compile(r'\bMTP\b|num_nextn_predict_layers', re.I)
_RE_DRAFT = re.compile(r'DFlash|eagle-?3?\b|draft\s*model|초안\s*모델', re.I)
_RE_OFF = re.compile(r'spec(?:ulative)?[\s-]*(?:decoding)?\s*(?:off|해제|미사용|비활성|없음|없이|미적용)|spec\s*off|no\s+spec', re.I)


def extract_sd(body: str) -> dict:
    """태그 본문 → (a) 모델의 SD **능력**(mode) 과 (b) 이 레시피가 SD 를 **썼는지**(enabled).
    둘은 다르다 — hy3 `-nospec` 은 능력이 있으나 쓰지 않은 경우다.

    **신호 우선순위**(2026-08-20 교정): 수치 > 산문. `accept_len > 1.0` 은 초안 토큰이 실제로
    수용됐다는 뜻이라 spec 가동의 **직접 증거**다. 산문 패턴('no spec'·'없이')을 먼저 보면
    다른 맥락의 부정문에 걸려 오판한다 — 실제로 R3/R2/R6 3건이 그렇게 뒤집혔다.

    **합성 금지**: 수치는 본문에 실제로 있을 때만 채우고 `measured` 로 표시한다.
    능력을 단정할 근거가 없으면 `unknown` 으로 둔다 — `none`(SD 없는 모델)과 혼동하지 않는다."""
    ns = _RE_NUM_SPEC.search(body)
    acc = _RE_ACCEPT.search(body)
    spec_tokens = int(ns.group(1)) if ns else None
    accept_len = float(acc.group(1)) if acc else None
    has_draft, has_mtp = bool(_RE_DRAFT.search(body)), bool(_RE_MTP.search(body))

    if spec_tokens is not None:
        enabled = spec_tokens > 0
    elif accept_len is not None and accept_len > 1.0:
        enabled = True                      # 초안 수용이 실측됐다 = 켜져 있었다
    elif _RE_OFF.search(body):
        enabled = False
    elif has_draft or has_mtp:
        enabled = None                      # 언급은 있으나 사용 여부 불명
    else:
        enabled = None
    mode = "draft-external" if has_draft else ("mtp-internal" if has_mtp else "unknown")
    return {"enabled": enabled, "mode": mode, "spec_tokens": spec_tokens,
            "accept_len": accept_len, "measured": accept_len is not None,
            "source": "tag_body_scan"}


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
        if slug in KNOWN_CARDLESS:
            card = None
        fid, relation, source = classify(slug, card)
        ev = ""
        if source != "manual" and card:
            ev = f"card base_model={card['base']}" + (f" relation={card['rel']}" if card.get("rel") else "")
        elif slug in KNOWN_CARDLESS:
            ev = "이기종 발행처(rtxpro6000/rtx5090) — 이 호스트 NAS 에 카드 없음"
        f = fams.setdefault(fid, {"root_repo": None, "members": [], "sd_capability":
                                  {"mode": "unknown", "draft_slug": None}})
        # `repo` = 실제로 서빙한 모델의 HF repo 이름(NAS 카드 디렉터리명). 슬러그가 이것과 다를 수
        # 있으므로(예: 태그 `hy3` ↔ repo `Hy3-NVFP4-W4A16`) 별도로 적는다 — R1 슬러그 파생 검사가
        # family 연속성을 판정할 때 이 필드로 이어붙인다.
        f["members"].append({"slug": slug, "relation": relation, "source": source,
                             "evidence": ev, "repo": (cand and sorted(cand, key=lambda k: abs(len(_norm(k)) - len(ns)))[0]) or None})
        if card and card.get("base") and source != "manual":
            f["root_repo"] = card["base"]
    # 수동 선언을 덮어씌운다(자동보다 우선 — 사람 승인이 최종 권위).
    for fid, spec in MANUAL_FAMILIES.items():
        merged = fams.setdefault(fid, {"root_repo": None, "members": [],
                                       "sd_capability": {"mode": "unknown", "draft_slug": None}})
        keep = [m for m in merged["members"] if m["slug"] not in spec["members"]]
        for slug, relation in spec["members"].items():
            keep.append({"slug": slug, "relation": relation, "source": "manual",
                         "evidence": spec["evidence"]})
            for other in list(fams):
                if other == fid:
                    continue
                fams[other]["members"] = [m for m in fams[other]["members"] if m["slug"] != slug]
        merged["members"] = sorted(keep, key=lambda m: m["slug"])
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
        # family 층 = 소속 태그들의 mode 합집합. unknown 은 다른 신호가 있으면 밀려난다.
        by_slug = {m["slug"]: fid for fid, f in data["families"].items() for m in f["members"]}
        agg: dict[str, set] = {}
        for tag, sd in data["tag_sd"].items():
            fid = by_slug.get(tag.split("/")[2])
            if fid and sd["mode"] != "unknown":
                agg.setdefault(fid, set()).add(sd["mode"])
        for fid, modes in agg.items():
            m = ("draft-external" if "draft-external" in modes else
                 "mtp-internal" if "mtp-internal" in modes else "none")
            data["families"][fid]["sd_capability"]["mode"] = m
            # 출처 표시(헌법 §결정론 규율) — 이 값은 모델 카드 선언이 아니라 **태그 본문 스캔에서
            # 파생**됐다. 사람이 확인해 고치면 source 를 manual 로 바꾼다.
            data["families"][fid]["sd_capability"]["source"] = "tag_body_scan"
        c = {}
        for sd in data["tag_sd"].values():
            c[sd["mode"]] = c.get(sd["mode"], 0) + 1
        print(f"[sd] 태그 {len(data['tag_sd'])}종 스캔 · mode 분포 {c} · "
              f"measured {sum(1 for v in data['tag_sd'].values() if v['measured'])}종")
    n_auto = sum(1 for f in data["families"].values() for m in f["members"] if m["source"] != "manual")
    n_man = sum(1 for f in data["families"].values() for m in f["members"] if m["source"] == "manual")
    multi = {k: v for k, v in data["families"].items() if len(v["members"]) > 1}
    print(f"[families] family {len(data['families'])}종 · 멤버 {n_auto + n_man}종 "
          f"(자동 {n_auto} · 수동 {n_man})")
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
        with open(FAMILIES_FILE, "w", encoding="utf-8") as fh:
            json.dump(data, fh, ensure_ascii=False, indent=2, sort_keys=True)
            fh.write("\n")
        print(f"[families] 기록: {os.path.relpath(FAMILIES_FILE, REPO)}")
    else:
        print("[families] dry-run (--write 로 기록)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
