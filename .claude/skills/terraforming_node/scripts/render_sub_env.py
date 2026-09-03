#!/usr/bin/env python3
"""render_sub_env.py — terraforming_node 의 **결정론 렌더러** (판단 X, 치환만).

역할(plan_26062408 G1·D10=render-on-main): 메인에서 `output/<topology>/manifest.yaml` 의
노드정체성(nodes[]·interconnect·hw)을 PII-free 템플릿 `{{ ... }}` 에 치환해 **서브노드 에이전트 환경**을
gitignored 스테이징 트리 `output/<topology>/sub_provision/` 로 렌더한다(서브 워크스페이스 루트 미러).

산출 스테이징 레이아웃(= 서브 워크스페이스에 오버레이될 아티팩트):
  CLAUDE.md                              ← CLAUDE.template.md         (렌더)
  Agent_Card.json                        ← Agent_Card.template.json   (렌더)
  .claude/settings.local.json            ← settings.local.template.json (렌더, 스코프드)
  .claude/rules/comms.md                 ← comms.md                   (복제·정적계약)
  .claude/rules/docs.md                  ← .claude/rules/docs.md      (복제·문서규약 테라포밍, D12)
  .claude/schemas/task-report.schema.json← task-report.schema.json    (복제·정적계약)
  .claude/schemas/library-exchange.schema.json ← library-exchange.schema.json (복제·정적계약, §2.7.8 그라운딩 교환)
  .claude/skills/{vllm-recipe-explorer,adversarial-benchmark}/ ← 런타임블럭(git-tracked만 복제 — config.yaml/feedback/lockset 제외)
  .claude/skills/wiki-desk/reference/references.md ← recipe의 on-demand 정적 reference dependency
  .gitignore                             ← gitignore.template         (복제·서브 로컬git 추적규칙, D12)
  docs/{plan,devlog,testlog,simlog,benchmark}/example.md ← terraforming owner templates (복제·발행 스켈레톤, D12)
  tasks/.gitkeep                         ← 런타임 상태 스캐폴드(빈 디렉토리)

D12: --topology {single|multi} 로 양 토폴로지 렌더(서브 로컬 git 양 브랜치). {{ TOPOLOGY }} 치환으로 페르소나가 브랜치 맥락 인지.

전달은 sync_to_sub.sh --provision(별도). 이 스크립트는 렌더까지만(결정론).
stdlib 만. PII(실 manifest)는 읽되 gitignored 스테이징으로만 쓴다(추적물엔 안 씀).
종료코드: 0=성공, 2=필수 manifest 필드 누락, 3=입력/IO 오류.
"""
from __future__ import annotations
import argparse
import glob
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys

# docs.md 가 규정하는 발행 문서 5종(서브 docs 스켈레톤 계약 — self-test 가 강제).
#   benchmark = full-런 계측 vault(서브도 adversarial-benchmark 런타임블럭 실행 → report/인증서 발행 가능, plan_26071510).
DOC_TYPES = ("plan", "devlog", "testlog", "simlog", "benchmark")

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.dirname(HERE)                       # .claude/skills/terraforming_node
SUBNODE_DIR = os.path.join(SKILL_DIR, "sub_node")       # 템플릿·정적자산 보관
REPO = os.path.abspath(os.path.join(SKILL_DIR, "..", "..", ".."))  # repo root
# 런타임블럭(서브 복제) — git-tracked 만 복제. 다중(plan_26063014: adversarial-benchmark 추가 = 2번째 런타임블럭).
#   adversarial-benchmark 의 (b) 외부검색 arm = 이중게이트(A2A 위임 키 ∧ egress-online) 통과 시 서브 자율,
#   미통과 시 미수행+증상 상향 — 서브는 (a) 루프라인-only 판정(SKILL.md §7). 렌더 시 서브 env 에 egress
#   attestation 을 반영해 서브 페르소나가 자기 능력을 정확히 로드한다(plan_26070809_46_57).
# 2026-09-03(P2 · plan_26090317): 이 리스트는 **토폴로지와 무관한 닫힌 목록**이라 멀티 서브(Ray
#   워커)에게도 서빙전략·벤치 스킬이 배달됐다. 불변식 A 가 말하는 멀티 sub 는 정본을 재현하는
#   워커이지 전략을 세우는 주체가 아니다 — 스킬을 들려주면 그 스킬이 시키는 자율 판단(트리플렛
#   자작·루브릭 판정)을 하게 되고, 그건 head 종속 제어평면과 충돌한다. 반대로 싱글 sub 는 빌드→
#   서빙→벤치를 자율 완주해야 하므로 3종이 필요하다.
#   판정은 `node_role_contract.tool_plane` 이 소유한다 — 여기서 두 번째 답을 만들지 않는다.
#   이 상수는 **경로 해소표**로만 남는다(어떤 스킬을 줄지는 계약이 정한다).
RUNTIME_BLOCK_PATHS = {
    "vllm-recipe-explorer": os.path.join(REPO, ".claude", "skills", "vllm-recipe-explorer"),
    "adversarial-benchmark": os.path.join(REPO, ".claude", "skills", "adversarial-benchmark"),
    "upstream-version-watch": os.path.join(REPO, ".claude", "skills", "upstream-version-watch"),
}

# 서브에 배달할 때 **제외**하는 경로(스킬 안의 메인 전용 오케스트레이션).
#
# 2026-09-03(P2 · plan_26090317): 사용자 범위 선언은 싱글 서브가 "타겟 모델 + 엔진 버전을 받으면
# 자율적으로 빌드→서빙→벤치" 를 하려면 **upstream 런타임 스킬이 활성화돼야 한다** 고 못박는다.
# 그런데 SKILL.md §2 는 upstream 을 "메인 전용, 서브 전달 ✗" 로 분류해 왔다. 둘 다 옳다 —
# **한 스킬 안에 두 성질이 섞여 있는 것**이 문제였다:
#
#   · 서브가 필요로 하는 것 = **해소·렌더**(resolve_*·render_dockerfile·regen_requirements·
#     classify_failure·check_smoke_model) — 자기 노드에서 자기 이미지를 만드는 능력.
#   · 서브에 가면 안 되는 것 = **노드 간 오케스트레이션**(sync_to_sub·sync_branches·fetch_sub_docs·
#     smoke_clone·multinode_*) — 이것은 메인이 서브를 향해 쓰는 도구다. 서브가 들면 배달 방향이
#     뒤집히고(§2.7.1 권한 평면 위반), 서브가 다른 노드를 향해 쓰는 경로가 생긴다.
#
# 그래서 "스킬 전체를 주느냐 마느냐" 가 아니라 **경로 단위로 가른다**. 목록은 닫혀 있고(tripwire),
# 새 오케스트레이션 스크립트가 생기면 아래 자체검사가 분류를 강제한다.
RUNTIME_BLOCK_EXCLUDES = {
    "upstream-version-watch": (
        "scripts/sync_to_sub.sh",          # 메인→서브 배달(방향이 뒤집힌다)
        "scripts/sync_branches.sh",        # 공유 빌딩블럭 브랜치 동기(메인 소관)
        "scripts/fetch_sub_docs.sh",       # 서브→메인 문서 회수(메인이 당긴다)
        "scripts/smoke_clone.sh",          # 배포본 클론 검증(메인 소관 · 이미 retirement 대상)
        "scripts/multinode_comms_smoke.sh",  # 노드 간 통신 스모크(메인이 양노드를 향해 쓴다)
        "scripts/multinode_serve_smoke.sh",  # 동상
    ),
}
DOCS_RULES = os.path.join(REPO, ".claude", "rules", "docs.md")     # 문서규약(정적계약 — 서브 테라포밍, D12)
DOC_SKELETONS = os.path.join(SKILL_DIR, "templates", "document_skeletons")  # 배포 포함 docs/*/example.md 정본(D12)
RECIPE_REFERENCE = os.path.join(REPO, ".claude", "skills", "wiki-desk", "reference", "references.md")

# 토폴로지 축 계약(sub_mode·rank·정체성 권위)의 **단일 소유자**. 렌더러는 산출을 **적기만** 한다 —
# 두 번째 파생 구현을 두면 두 답이 갈리고(헌법 §4종 안티패턴 "파생 가능한데 손으로 적은 것"),
# 그 갈림이 2026-08-22 진단의 형태였다(SKILL.md §2.7.6(c) · 헌법 §불변식 A).
sys.path.insert(0, HERE)
import node_role_contract as _contract  # noqa: E402  (형제 스크립트 — 위 sys.path 선행 필요)

PLACEHOLDER_RE = re.compile(r"\{\{\s*([A-Z_]+)\s*\}\}")
# 템플릿 전용 머리말(렌더 산출물에서 제거) — md 템플릿의 "이건 템플릿이다" 메타 블록.
TEMPLATE_HEADER_RE = re.compile(r"<!--\s*TEMPLATE-ONLY:START\s*-->.*?<!--\s*TEMPLATE-ONLY:END\s*-->\s*", re.DOTALL)

# ── 모드 게이트 블록 (2026-09-03 · S2 · plan_26090317 P1) ──────────────────────────────────────
# 헌법 불변식 A: **한 스킴의 `role: sub` 로 멀티 Ray 워커와 싱글 A2A 에이전트 양자를 덮지 않는다.**
# 그런데 CLAUDE.template.md 에는 `{{SUB_MODE}}` 계열 placeholder 가 하나도 없어서, single 렌더에도
# "너 = slave(Ray worker)" 가 그대로 배달됐다(실측 확증). 같은 워크스페이스에 Agent_Card 는
# `sub_mode: a2a-agent`, 페르소나는 Ray 워커 — **서로 모순되는 두 정체성**이고 자동 로드되는 쪽
# (페르소나)이 틀린 쪽이었다.
#
# 파일을 둘로 쪼개지 않고(90% 중복은 곧 드리프트다) 마커로 가른다. 렌더러가 SUB_MODE 와 일치하는
# 블록만 남기고 나머지는 **바이트째 제거**한다 — 서브는 자기 모드의 문장만 본다.
MODE_BLOCK_RE = re.compile(
    r"[ \t]*<!--\s*MODE:(?P<mode>[a-z0-9-]+)\s*-->\n(?P<body>.*?)[ \t]*<!--\s*/MODE:(?P=mode)\s*-->\n?",
    re.DOTALL)
KNOWN_SUB_MODES = ("ray-worker", "a2a-agent")


def apply_mode_blocks(text: str, sub_mode: str) -> str:
    """SUB_MODE 와 일치하는 MODE 블록만 남긴다. 미지 모드는 fail-loud(조용한 전량 삭제 금지)."""
    seen = set()

    def _keep(m):
        mode = m.group("mode")
        seen.add(mode)
        if mode not in KNOWN_SUB_MODES:
            raise SystemExit(f"[render] FAIL: 미지의 MODE 블록 '{mode}' — 알려진 모드 {KNOWN_SUB_MODES}")
        return m.group("body") if mode == sub_mode else ""

    out = MODE_BLOCK_RE.sub(_keep, text)
    if seen and sub_mode not in seen:
        # 블록이 있는데 내 모드용 블록이 하나도 없다 = 그 모드의 페르소나가 통째로 비어 배달된다.
        raise SystemExit(f"[render] FAIL: sub_mode='{sub_mode}' 용 MODE 블록이 템플릿에 없다(있는 것: {sorted(seen)})")
    return out



def strip_template_header(text: str) -> str:
    return TEMPLATE_HEADER_RE.sub("", text)


# ── manifest 파싱 (pyyaml 비의존 — scan_node.py 와 동일 원칙, 이 구조 전용) ──
# 계약: 리스트는 **inline-flow `[a, b]` 만** 지원(producer scan_node.py 가 항상 inline-flow emit — matched-pair).
#       block-style(`key:`\n`  - a`)는 설계상 미지원(HCA_DEVICES 는 식별 prose-only 라 영향 없음). renderer-1.
# _clean 은 '#' 분할→따옴표 strip 순서(manifest 값 도메인 IP/경로/iface 엔 '#' 없음 — scan_node.py 와 정합). renderer-2.
def _clean(v: str) -> str:
    v = v.split("#", 1)[0].strip()        # 인라인 주석 제거
    return v.strip().strip('"').strip("'").strip()


def parse_manifest(path: str) -> dict:
    data: dict = {"interconnect": {}, "nodes": []}
    section = None
    cur = None
    with open(path, encoding="utf-8") as f:
        for raw in f:
            line = raw.rstrip("\n")
            if not line.strip() or line.lstrip().startswith("#"):
                continue
            indent = len(line) - len(line.lstrip())
            s = line.strip()
            if indent == 0:
                if s.startswith("interconnect:"):
                    section = "interconnect"; cur = None; continue
                if s.startswith("nodes:"):
                    section = "nodes"; cur = None; continue
                section = None; cur = None
                k, _, v = s.partition(":")
                data[k.strip()] = _clean(v)
            elif section == "interconnect":
                k, _, v = s.partition(":")
                data["interconnect"][k.strip()] = _clean(v)
            elif section == "nodes":
                if s.startswith("- role:"):
                    cur = {"role": _clean(s.split(":", 1)[1])}
                    data["nodes"].append(cur)
                elif cur is not None and ":" in s:
                    k, _, v = s.partition(":")
                    cur[k.strip()] = _clean(v)
    return data


def _node(data: dict, role: str) -> dict:
    for n in data.get("nodes", []):
        if n.get("role") == role:
            return n
    return {}


def build_placeholders(data: dict) -> tuple[dict, list[str]]:
    """manifest → {{KEY}} 치환 dict. 반환 (placeholders, missing_required)."""
    ic = data.get("interconnect", {})
    sub = _node(data, "sub")
    main = _node(data, "main")
    sub_host = sub.get("host", "")
    work_dir = sub.get("work_dir") or main.get("work_dir", "")
    ssh_user = sub.get("ssh_user") or main.get("ssh_user", "")
    gpus = data.get("gpus_per_node", "")
    cpu_arch = data.get("cpu_arch", "")

    ph = {
        "SUB_HOST": sub_host,
        "SUB_HOSTNAME": sub.get("hostname") or sub_host,
        "MASTER_HOST": main.get("host", ""),
        "SSH_USER": ssh_user,
        "WORKSPACE_PATH": work_dir,
        "NAS_MOUNT": data.get("nas_model_path", ""),
        "CPU_ARCH": cpu_arch,
        "TOPOLOGY": data.get("topology", ""),   # D12: 서브 브랜치 맥락(single|multi) — main() 이 manifest 누락 시 --topology 로 채움

        "INTERCONNECT": ic.get("type", ""),
        "INTERCONNECT_IFACE": ic.get("socket_iface", ""),
        # ── 폴백 있는 항목(이식성) ──
        "INTERCONNECT_MTU": ic.get("mtu") or "9000",
        "GID_INDEX": ic.get("gid_index") or "null",
        "HCA_DEVICES": ic.get("hca_devices") or "[]",
        "PLATFORM_PRESET": ic.get("platform_preset") or "",
        "RAY_PORT": data.get("ray_port") or "6379",
        "GPU_MODEL": data.get("gpu_model") or (f"{gpus}x-{cpu_arch}" if gpus and cpu_arch else cpu_arch or "unknown-gpu"),
        # A2A 위임 키 발급 판정용(plan_26063021_14_37 D5/D7) — nodes[sub].hw_verified(동질성 검증 통과 표식). 템플릿 치환엔 미사용.
        "SUB_HW_VERIFIED": (sub.get("hw_verified") or ""),
    }
    ph.update(_contract_placeholders(data))
    # 필수(누락 시 fail-loud — 무증거/빈 정체성 렌더 금지)
    #   SUB_MODE·*_SOURCE 는 계약이 해소하는 파생값이다 — 빈 값 = 계약 미해소(위반 또는 서브 미등록)이며
    #   그대로 렌더하면 서브의 정체성 권위(AgentCard)가 거짓을 싣는다. 그래서 같은 fail-loud 통로에 둔다.
    #   ⚠ SUB_RANK 는 여기 넣지 않는다 — single 의 정답이 리터럴 `null` 이라 "빈 값"과 구분돼야 한다(음성정직).
    required = ["SUB_HOST", "MASTER_HOST", "SSH_USER", "WORKSPACE_PATH", "NAS_MOUNT",
               "CPU_ARCH", "INTERCONNECT", "INTERCONNECT_IFACE",
               "SUB_MODE", "SUB_MODE_SOURCE", "SUB_RANK_SOURCE"]
    missing = [k for k in required if not ph.get(k)]
    return ph, missing


def _contract_placeholders(data: dict) -> dict:
    """토폴로지 축 계약 산출 → {{SUB_MODE}}·{{SUB_MODE_SOURCE}}·{{SUB_RANK}}·{{SUB_RANK_SOURCE}}.

    **여기서 규칙을 재저작하지 않는다** — `node_role_contract.evaluate_manifest` 를 부르고 그 답을
    옮겨 적을 뿐이다(SKILL.md §2.7.6(c) "렌더러는 적기만 한다"). 값 옆에 출처를 함께 실어야
    하류가 측정·선언·파생을 구분할 수 있다(헌법 §결정론 규율 출처 표시).

    위반(예: single 에 sub_mode: ray-worker 선언)이면 값 자리를 비워 둔다 — 호출부의 `missing`
    fail-loud 로 흘러가 렌더가 멈춘다(침묵 폴백 ✗).
    """
    res = _contract.evaluate_manifest(data, topology=data.get("topology") or None, role="sub")
    sub_mode = res.get("sub_mode") or {}
    rank = res.get("rank") or {}
    rank_value = rank.get("value")
    tp = res.get("tool_plane") or {}
    return {
        "SUB_MODE": sub_mode.get("value") or "",
        "SUB_MODE_SOURCE": sub_mode.get("source") or "",
        # 2026-09-03(P2): 어떤 런타임 스킬을 배달할지도 **계약 산출**이다. 렌더러의 닫힌 리스트가
        #   토폴로지를 무시하던 자리를 이 값이 대체한다. 빈 문자열 = 0종(정상 상태일 수 있다).
        "TOOL_PLANE": ",".join(tp.get("value") or []),
        "TOOL_PLANE_SOURCE": tp.get("source") or "",
        # JSON 템플릿에서 **따옴표 없이** 놓인다 — single 은 리터럴 null, multi 는 정수.
        # 문자열 "null" 로 싣으면 하류가 rank 를 문자열로 읽어 TP/NCCL 입력으로 오해할 수 있다.
        "SUB_RANK": "null" if rank_value is None else str(int(rank_value)),
        "SUB_RANK_SOURCE": rank.get("source") or "",
        # 위반 코드(렌더 실패 시 사람이 읽을 진단 — 템플릿 치환엔 미사용).
        "SUB_CONTRACT_VIOLATIONS": ",".join(v.get("code", "?") for v in res.get("violations", [])),
    }


def render_text(text: str, ph: dict) -> str:
    return PLACEHOLDER_RE.sub(lambda m: str(ph.get(m.group(1), m.group(0))), text)


def _unrendered(text: str) -> list[str]:
    return sorted(set(PLACEHOLDER_RE.findall(text)))


# ── 렌더 한 판 (스테이징 트리 산출) ──
def render_tree(ph: dict, out_dir: str, copy_runtime_block: bool = True,
                tracked_list: list | None = None) -> dict:
    """치환된 placeholders 로 스테이징 트리를 만든다. 반환 = 산출 매니페스트(검증용)."""
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    claude = os.path.join(out_dir, ".claude")
    os.makedirs(os.path.join(claude, "rules"), exist_ok=True)
    os.makedirs(os.path.join(claude, "schemas"), exist_ok=True)
    os.makedirs(os.path.join(out_dir, "tasks"), exist_ok=True)

    produced: list[str] = []

    def _render_file(tmpl_name: str, dest_rel: str, kind: str):
        with open(os.path.join(SUBNODE_DIR, tmpl_name), encoding="utf-8") as f:
            raw = f.read()
        if kind == "md":
            raw = strip_template_header(raw)        # 템플릿 메타 머리말 제거(렌더 산출물 정결)
            raw = apply_mode_blocks(raw, ph.get("SUB_MODE", ""))   # S2: 정체성 분기(불변식 A)
        txt = render_text(raw, ph)
        left = _unrendered(txt)
        if left:
            raise SystemExit(f"[render] FAIL: 미치환 placeholder {left} in {tmpl_name}")
        if kind == "json":
            obj = json.loads(txt)                   # 렌더 후 JSON 유효성
            if isinstance(obj, dict):               # _ 접두 메타키(_template_note) 제거
                for k in [k for k in obj if k.startswith("_")]:
                    obj.pop(k)
            txt = json.dumps(obj, ensure_ascii=False, indent=2) + "\n"
        dest = os.path.join(out_dir, dest_rel)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        with open(dest, "w", encoding="utf-8") as f:
            f.write(txt)
        produced.append(dest_rel)
        return dest

    # 1) 렌더 3종 (md=머리말 strip · json=_메타키 drop+유효성)
    _render_file("CLAUDE.template.md", "CLAUDE.md", "md")
    _render_file("Agent_Card.template.json", "Agent_Card.json", "json")
    _render_file("settings.local.template.json", ".claude/settings.local.json", "json")

    # 2) 복제 정적계약 (comms·schema·docs규약)
    shutil.copyfile(os.path.join(SUBNODE_DIR, "comms.md"), os.path.join(claude, "rules", "comms.md"))
    produced.append(".claude/rules/comms.md")
    # 서브로 복제되는 정적 스키마 계약. `library-exchange` 가 빠져 있던 동안 서브는 교환 메시지의
    # 계약 없이 형태를 추측해야 했다(testlog_26082215 S-9) — SKILL.md §4 목록이 정본으로 적었는데
    # 렌더러 배선만 없던 **침묵 누락**이다(만든 것 ≠ 도는 것).
    SUB_SCHEMAS = ("task-report.schema.json", "library-exchange.schema.json")
    for schema_name in SUB_SCHEMAS:
        schema_dst = os.path.join(claude, "schemas", schema_name)
        shutil.copyfile(os.path.join(SUBNODE_DIR, schema_name), schema_dst)
        with open(schema_dst, encoding="utf-8") as f:
            json.load(f)
        produced.append(f".claude/schemas/{schema_name}")
    # D12: 문서규약 테라포밍 — 서브가 동일 발행규약(docs.md)으로 insight 문서 발행 → 상향 문서기반 회수
    if os.path.isfile(DOCS_RULES):
        shutil.copyfile(DOCS_RULES, os.path.join(claude, "rules", "docs.md"))
        produced.append(".claude/rules/docs.md")

    # recipe.py resolves GPU/source-verification facts from this on-demand dependency.  Copy only
    # the dependency, not the main-only wiki-desk capability, so sub runtime closure stays minimal.
    # 2026-09-03(P2): 이 사본은 `vllm-recipe-explorer` 의 의존이다 — 그 스킬이 가지 않는 모드
    #   (ray-worker)에 도서관 발췌만 보내는 것은 근거 없는 배달이다. tool_plane 을 따른다.
    #   (사본 자체를 걷어내는 것 = wiki-desk 사영은 이번 plan 범위 밖 — §8 후속.)
    if "vllm-recipe-explorer" in (ph.get("TOOL_PLANE") or ""):
        if not os.path.isfile(RECIPE_REFERENCE):
            raise SystemExit(f"[render] FAIL: recipe reference dependency missing: {RECIPE_REFERENCE}")
        recipe_ref_rel = ".claude/skills/wiki-desk/reference/references.md"
        recipe_ref_dst = os.path.join(out_dir, recipe_ref_rel)
        os.makedirs(os.path.dirname(recipe_ref_dst), exist_ok=True)
        shutil.copyfile(RECIPE_REFERENCE, recipe_ref_dst)
        produced.append(recipe_ref_rel)

    # 3) 런타임블럭 복제(git-tracked 만) — **무엇을 줄지는 계약이 정한다**(P2 토폴로지 게이팅).
    if copy_runtime_block:
        wanted = ph.get("TOOL_PLANE") or ""
        names = [x for x in wanted.split(",") if x.strip()]
        for name in names:
            rb = RUNTIME_BLOCK_PATHS.get(name)
            if rb is None:
                raise SystemExit(f"[render] FAIL: tool_plane 이 지목한 런타임블럭 경로를 모른다: {name}")
            dst_skill = os.path.join(claude, "skills", name)
            n = _copy_tracked(rb, dst_skill, tracked_list=tracked_list,
                              excludes=RUNTIME_BLOCK_EXCLUDES.get(name))
            ex = RUNTIME_BLOCK_EXCLUDES.get(name)
            produced.append(f".claude/skills/{name}/ ({n} tracked files"
                            + (f" · 메인전용 {len(ex)}건 제외)" if ex else ")"))
        if not names:
            # 음성정직: "0종" 은 결손이 아니라 판정 결과다. 그 사실이 산출 매니페스트에 남아야
            # 나중에 "왜 스킬이 없지?" 가 조용한 결손과 구분된다.
            produced.append(".claude/skills/ (런타임블럭 0종 — tool_plane=%s)"
                            % (ph.get("TOOL_PLANE_SOURCE") or "unknown"))

    # 3.5) A2A 위임 키 (plan_26063021_14_37 D5/D7) — 서브 HW 동질성 검증(nodes[sub].hw_verified=true) 통과 시에만 발급.
    #   메인 키(terraforming.complete@manifest)와 UNIQUE. 최소 attestation(HW사실/전체 manifest ✗ → D10 보존).
    #   recipe.py·run_bench.sh 가 이 파일 존재로 서브 게이트 면제(fail-closed 양성 키). 미검증이면 미발급 → 서브 info-only.
    if str(ph.get("SUB_HW_VERIFIED", "")).strip().lower() == "true":
        deleg = {
            "delegation": "main_cluster_flag",
            "issued_to": "sub",   # 역할 단언(D8·WARN-1): recipe.py·run_bench.sh 가 이 값으로 메인 키 오용 차단
            "topology": ph.get("TOPOLOGY", ""),
            "note": ("Sub operates under main-node terraforming Flag (A2A delegation). "
                     "HW homogeneity verified by main (plan_26063021_14_37). "
                     "Do NOT create manually on a main/standalone node."),
        }
        with open(os.path.join(claude, "a2a_delegation.json"), "w", encoding="utf-8") as f:
            json.dump(deleg, f, ensure_ascii=False, indent=2)
            f.write("\n")
        produced.append(".claude/a2a_delegation.json")

    # 4) tasks/ 스캐폴드(빈 디렉토리 — git keep)
    with open(os.path.join(out_dir, "tasks", ".gitkeep"), "w") as f:
        f.write("")
    produced.append("tasks/.gitkeep")

    # 4.5) 호스트 안전체계(plan_26071019 §2.2).
    #   canonical source는 terraforming skill이 소유하고, 서브에는 헌법 runtime asset으로
    #   materialize한다. root scripts/에 대한 숨은 source/runtime 의존성을 만들지 않는다.
    host_safety_src = os.path.join(
        REPO, ".claude", "skills", "terraforming_node", "scripts", "host_safety")
    host_safety_files = (
        ("mem_watchdog.sh", "mem_watchdog.sh", 0o755),
        ("install_host_safety.sh", "install_host_safety.sh", 0o755),
        ("install_netconsole.sh", "install_netconsole.sh", 0o755),
        ("systemd/easy-vllm-memwatch.service", "systemd/easy-vllm-memwatch.service", 0o644),
        ("host/vllm-drop-caches.sh", "host/vllm-drop-caches.sh", 0o755),
    )
    for source_rel, delivered_rel, mode in host_safety_files:
        src = os.path.join(host_safety_src, source_rel)
        if not os.path.isfile(src):
            raise SystemExit(
                "[render_sub_env] HOST_SAFETY_SOURCE_MISSING: "
                f"canonical terraforming asset absent: {source_rel}")
        rel = os.path.join(".claude", "runtime", "host_safety", delivered_rel)
        dst = os.path.join(out_dir, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(src, dst)
        os.chmod(dst, mode)
        produced.append(rel)

    # 4.6) 노드 블랙박스(plan_26073109 §Phase 3·5). host_safety 와 **동형** 배선이다.
    #   ★ 이 블록이 없던 동안 node_blackbox 는 수동 rsync 로만 서브에 갔고, 커밋되지 않은 그
    #     잔재가 2026-08-01 정식 배달을 dirty 로 막았다(policy:SUB_SYNC_DIRTY_AUTOSAVE).
    #     "배달 경로가 없으면 사람이 우회한다 — 그리고 그 우회가 다음 정식 경로를 막는다."
    #   ★ offline/*.deb 는 **배달하지 않는다**. `.gitignore:228` 이 offline/ 전체를 비추적으로 두는데,
    #     서브 배달은 git 인덱스 권위(ls-files→checkout-index)라 비추적물은 스냅샷에 아예 없다.
    #     정책을 뒤집지 않고 따른다 — 이 프로젝트는 바이너리가 아니라 스켈레톤+생성엔진을 배포한다.
    #     결과: 서브의 earlyoom 설치는 `apt-get install -y earlyoom` 폴백 경로를 탄다
    #     (install_node_blackbox.sh:278-280 이 이미 그렇게 분기한다). egress 가 막힌 서브라면
    #     운영자가 deb 를 수동으로 넣어야 하며, 그 사실이 여기 기록돼 있다.
    blackbox_src = os.path.join(
        REPO, ".claude", "skills", "terraforming_node", "scripts", "node_blackbox")
    blackbox_files = (
        ("mem_watchdog_eta.sh", "mem_watchdog_eta.sh", 0o755),
        ("blackbox_collect.py", "blackbox_collect.py", 0o755),
        ("blackbox_events.py", "blackbox_events.py", 0o755),
        ("blackbox_eta.py", "blackbox_eta.py", 0o644),
        ("blackbox_session.py", "blackbox_session.py", 0o644),
        ("logs_lifecycle.py", "logs_lifecycle.py", 0o755),
        ("seed_from_journal.py", "seed_from_journal.py", 0o755),
        ("install_node_blackbox.sh", "install_node_blackbox.sh", 0o755),
        ("verify_node_blackbox.sh", "verify_node_blackbox.sh", 0o755),
        ("purge_host_safety.sh", "purge_host_safety.sh", 0o755),
        # ★ 방어 2단(에이전트 예방)과 그 시험 도구. 2026-08-02 추가 —
        #   두 파일을 만들고 **이 목록에 넣지 않아** 서브 전파에서 조용히 빠졌다.
        #   D3(배달 배선 부재)를 같은 날 문서화해 놓고 같은 방식으로 재발시켰다:
        #   "새 도구를 만들면 배달 목록도 함께 고친다" 가 아직 절차로 굳지 않았다는 증거.
        ("agent_guard.py", "agent_guard.py", 0o755),
        ("adversarial_stress.py", "adversarial_stress.py", 0o755),
        # ★ 포락선 재생성기 + 키 정합 검증기. 2026-08-14 추가(plan_26081415 C2).
        #   위 2026-08-02 주석이 예고한 재발을 **이번엔 같은 커밋에서** 막는다.
        #   서브에 이게 없으면 서브 포락선만 seed 로 동결된 채 남아(2026-08-14 실측: 서브
        #   envelope 이 여전히 state=seed·2026-07-31), 양노드 대칭이 깨진다.
        ("regen_envelope.py", "regen_envelope.py", 0o755),
        # ★ node_id 단일 해소기. 2026-08-15 추가(plan_26081514 A1–A3).
        #   install/verify/purge 세 스크립트가 이걸 **source** 한다 — 빠지면 서브에서 설치가
        #   첫 줄에서 죽는다. 위 2026-08-02·08-14 주석이 예고한 재발을 이번에도 같은 편집에서 막는다.
        ("node_identity.sh", "node_identity.sh", 0o755),
        # ★ 상주 서빙 예산 갱신 사이드카. 2026-08-22 추가(W-7 · testlog_26082215 §4.0).
        #   멀티 `--keep-up` 은 **양 노드**에 선언을 남기므로 갱신자도 양 노드에 있어야 한다 —
        #   빠지면 슬레이브 선언만 조용히 만료돼 서브에서만 무보호 구간이 생긴다(하드다운 #2 가
        #   서브였음을 상기하라). 위 세 ★ 주석이 예고한 "만들고 목록에 안 넣기" 재발을 같은
        #   편집에서 막는다.
        ("budget_renew_loop.sh", "budget_renew_loop.sh", 0o755),
        # ★ 열·전력 포락선 축(하드락업 **예방**). 2026-08-23 추가(plan_26082319 §6.3).
        #   위 네 ★ 주석이 예고한 "만들고 목록에 안 넣기" 재발을 이번에도 같은 편집에서 막는다 —
        #   실제로 이번에도 **전파 단계에서야** 누락이 드러났다(구현 커밋 시점엔 빠져 있었다).
        #   빠지면 서브에서 install_node_blackbox.sh 가 `install $SDIR/blackbox_thermal.py` 에서
        #   죽고(전제 실패), verify 의 자체시험·배포본 신선도 검사도 통째로 FAIL 한다.
        #   두 파일은 **한 쌍**이다: 엔진이 emit 한 상수를 핫루프가 source 하므로 한쪽만 가면
        #   핫루프가 내장 기본값으로 조용히 돈다(= 상수 파일 부재와 같은 상태).
        ("blackbox_thermal.py", "blackbox_thermal.py", 0o755),
        ("thermal_watchdog.sh", "thermal_watchdog.sh", 0o755),
        # ── 의도적 **미배달** (2026-08-18 명시 · testlog_26081810 §8) ─────────────────
        #   `publish_install_request.py` 는 이 디렉터리에 있지만 **서브로 보내지 않는다**.
        #   그것은 L3 설치를 `docs/request/` 수행지시서로 발행하는 도구인데, 서브에는
        #   `docs/request/` 자체가 없다 — `.claude/rules/docs.md` §서브 docs 계약이 서브에
        #   plan·devlog·testlog·simlog·benchmark **다섯 스켈레톤만** 렌더하고 report·request 는
        #   렌더하지 않는다고 규정한다(2026-08-18 실측: 서브 docs/ = 그 5개 + logs). 보내면
        #   존재하지 않는 디렉터리에 발행을 시도하는 도구가 서브에 놓인다.
        #   ★ 이 줄을 적는 이유: 위 세 ★ 주석이 증언하듯 이 목록은 **손유지 닫힌 목록**이고
        #     누락은 침묵한다. "빠져 있다"만으로는 *판정한 제외*와 *잊은 누락*이 구분되지
        #     않는다 — 실제로 이 파일도 그 구분 없이 빠져 있었고, 결과가 옳았을 뿐이다.
        #     새 도구를 추가할 때는 배달하거나, 배달하지 않는 **사유를 여기 적는다**.
    )
    for source_rel, delivered_rel, mode in blackbox_files:
        src = os.path.join(blackbox_src, source_rel)
        if not os.path.isfile(src):
            raise SystemExit(
                "[render_sub_env] NODE_BLACKBOX_SOURCE_MISSING: "
                f"canonical terraforming asset absent: {source_rel}")
        rel = os.path.join(".claude", "runtime", "node_blackbox", delivered_rel)
        dst = os.path.join(out_dir, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(src, dst)
        os.chmod(dst, mode)
        produced.append(rel)

    # 5) 서브 로컬 git .gitignore (D12 — placeholder 없는 정적자산 그대로 복제)
    gi_src = os.path.join(SUBNODE_DIR, "gitignore.template")
    if os.path.isfile(gi_src):
        shutil.copyfile(gi_src, os.path.join(out_dir, ".gitignore"))
        produced.append(".gitignore")

    # 6) docs/ 발행 스켈레톤.  `docs/`는 release archive에서 제외되므로 canonical sources are
    #    terraforming-owned templates, not repository documentation. Every required type is explicit
    #    and missing authority fails closed; report and other main-only folders cannot be globbed in.
    for dtype in DOC_TYPES:
        ex = os.path.join(DOC_SKELETONS, dtype, "example.md")
        if not os.path.isfile(ex):
            raise SystemExit(f"[render] FAIL: docs skeleton missing: {ex}")
        dst = os.path.join(out_dir, "docs", dtype, "example.md")
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copyfile(ex, dst)
    produced.append(f"docs/*/example.md ({len(DOC_TYPES)} skeletons)")

    return {"out_dir": out_dir, "produced": produced}


def _copy_tracked(src_skill: str, dst: str, tracked_list: list | None = None,
                  excludes: tuple | None = None) -> int:
    """런타임블럭을 서브 스테이징으로 복제한다 — **git-tracked 만**이 계약이다.

    2026-09-03(S10/㉓/F3 · plan_26090317 P1) 재작성. 이전 판본은 `git ls-files` 의 returncode 를
    보지 않고 빈 목록이면 조용히 `os.walk` 폴백으로 넘어갔다. 그런데 이 함수가 실제로 도는 자리는
    `sync_to_sub.prepare_transactional_source` 가 만든 **`checkout-index --prefix` 트리**이고 거기엔
    `.git` 이 없다 — 즉 `git ls-files` 가 **매번 rc=128** 로 죽어 **폴백이 곧 프로덕션 경로**였다.
    화면에는 `+ …/vllm-recipe-explorer/ (23 tracked files)` 라고 찍히는데 그 "tracked" 는 거짓 표기였고,
    필터는 손유지 파일명 블랙리스트 4개뿐이었다(그 이름을 가진 추적 파일이 생기면 **말없이 누락**된다).

    처방은 둘이다: ① tracked 목록을 얻지 못하면 **추측하지 않는다**(fail-loud) ② 호출부가 목록을
    알고 있으면 `tracked_list` 로 주입한다(트랜잭션 트리처럼 git 이 없는 자리를 위한 정식 통로).
    모드도 보존한다(`copy2` — ㉟: 이전에는 0664 로 평탄화됐다).
    """
    rel = os.path.relpath(src_skill, REPO)
    if tracked_list is not None:
        prefix = rel.rstrip("/") + "/"
        files = [f for f in tracked_list if f == rel or f.startswith(prefix)]
        source = "injected:tracked-list"
    else:
        try:
            out = subprocess.run(["git", "-C", REPO, "ls-files", "-z", "--", rel],
                                 capture_output=True, text=True, timeout=20)
        except Exception as exc:
            raise SystemExit(f"[render] FAIL: git ls-files 실행 불가({type(exc).__name__}: {exc}) — "
                             f"tracked 목록 없이 런타임블럭을 복제하지 않는다({rel}).")
        if out.returncode != 0:
            raise SystemExit(
                f"[render] FAIL: git ls-files rc={out.returncode} — tracked 목록 없이 복제하지 않는다({rel}).\n"
                f"          stderr: {(out.stderr or '').strip()[:300]}\n"
                f"          (git 없는 트리에서 렌더한다면 호출부가 tracked_list 를 주입해야 한다.)")
        files = [f for f in out.stdout.split("\0") if f.strip()]
        source = "measured:git-ls-files"
    if not files:
        raise SystemExit(f"[render] FAIL: 런타임블럭 {rel} 의 tracked 파일이 0개 — 배달할 것이 없다(계약 위반).")
    cnt = 0
    ex = set(excludes or ())
    for rf in files:
        srcf = os.path.join(REPO, rf)
        if not os.path.isfile(srcf):
            continue                       # index 에는 있으나 워킹트리에 없는 항목(삭제 스테이징 등)
        sub_rel = os.path.relpath(srcf, src_skill)
        if sub_rel in ex:
            continue                       # 메인 전용 오케스트레이션(위 RUNTIME_BLOCK_EXCLUDES)
        destf = os.path.join(dst, sub_rel)
        os.makedirs(os.path.dirname(destf), exist_ok=True)
        shutil.copy2(srcf, destf)          # ㉟: 모드 보존
        cnt += 1
    _copy_tracked.last_source = source     # 호출부/시험이 출처를 확인할 수 있게(결정론 규율)
    return cnt


# ── self-test (하드웨어/실 manifest 불요 — fixture 렌더 회귀) ──
FIXTURE_MANIFEST = """\
topology: multi
cpu_arch: "aarch64"
cuda_version: "132"
gpus_per_node: 1
gpu_model: "TEST-GPU"
nas_model_path: /srv/test-models
model_source: nas
interconnect:
  type: RoCE v2
  hca_devices: [testhca0, testhca1]
  gid_index: 3
  socket_iface: testif0
  mtu: 9000
  bandwidth_gbps: 200
  platform_preset: test-preset
nodes:
  - role: main
    host: 203.0.113.10
    ssh_user: tester
    work_dir: /srv/tester/ws
  - role: sub
    host: 203.0.113.11
    ssh_user: tester
    work_dir: /srv/tester/ws
"""


def _self_test() -> int:
    import tempfile
    ok = True
    tmp = tempfile.mkdtemp(prefix="render_sub_env_selftest_")
    mpath = os.path.join(tmp, "manifest.yaml")
    with open(mpath, "w", encoding="utf-8") as f:
        f.write(FIXTURE_MANIFEST)

    # (1) 파싱 + placeholders
    data = parse_manifest(mpath)
    ph, missing = build_placeholders(data)
    c1 = not missing and ph["SUB_HOST"] == "203.0.113.11" and ph["MASTER_HOST"] == "203.0.113.10" \
        and ph["INTERCONNECT_IFACE"] == "testif0" and ph["INTERCONNECT_MTU"] == "9000" \
        and ph["GPU_MODEL"] == "TEST-GPU" and ph["NAS_MOUNT"] == "/srv/test-models" \
        and ph["TOPOLOGY"] == "multi"
    print(f"  [{'PASS' if c1 else 'FAIL'}] manifest 파싱 + placeholders (missing={missing}, topology={ph.get('TOPOLOGY')})")
    ok &= c1

    # (2) 폴백: gpu_model/mtu 누락 시 기본값
    data2 = dict(data); data2.pop("gpu_model", None); data2["interconnect"] = dict(data["interconnect"]); data2["interconnect"].pop("mtu", None)
    ph2, _ = build_placeholders(data2)
    c2 = ph2["INTERCONNECT_MTU"] == "9000" and ph2["GPU_MODEL"] == "1x-aarch64"
    print(f"  [{'PASS' if c2 else 'FAIL'}] 폴백 기본값(mtu=9000, gpu_model=1x-aarch64) → got mtu={ph2['INTERCONNECT_MTU']} gpu={ph2['GPU_MODEL']}")
    ok &= c2

    # (3) 필수 누락 → fail-loud
    data3 = {"interconnect": {}, "nodes": [{"role": "sub", "host": "", "work_dir": ""}]}
    _, missing3 = build_placeholders(data3)
    c3 = "SUB_HOST" in missing3 and "MASTER_HOST" in missing3
    print(f"  [{'PASS' if c3 else 'FAIL'}] 필수 누락 감지(fail-loud) → missing={missing3[:3]}...")
    ok &= c3

    # (4) 전체 렌더 → 미치환 0 + JSON 유효 + 아티팩트 구조(+ D12 신규: .gitignore·docs규약·docs스켈레톤)
    out = os.path.join(tmp, "sub_provision")
    try:
        res = render_tree(ph, out, copy_runtime_block=False)  # 런타임블럭 복제는 git 의존 → self-test 제외
        base_expect = ["CLAUDE.md", "Agent_Card.json", ".claude/settings.local.json",
                       ".claude/rules/comms.md", ".claude/schemas/task-report.schema.json",
                       ".claude/schemas/library-exchange.schema.json", "tasks/.gitkeep",
                       ".claude/rules/docs.md", ".gitignore",   # ← references.md 는 tool_plane 종속(아래 c4b)
                       # 호스트 안전체계: canonical terraforming source → constitution runtime delivery
                       ".claude/runtime/host_safety/mem_watchdog.sh",
                       ".claude/runtime/host_safety/install_host_safety.sh",
                       ".claude/runtime/host_safety/install_netconsole.sh",
                       ".claude/runtime/host_safety/systemd/easy-vllm-memwatch.service",
                       ".claude/runtime/host_safety/host/vllm-drop-caches.sh",
                       # 노드 블랙박스: 하나라도 빠지면 fail-loud (수동 rsync 우회 재발 차단)
                       ".claude/runtime/node_blackbox/mem_watchdog_eta.sh",
                       ".claude/runtime/node_blackbox/blackbox_collect.py",
                       ".claude/runtime/node_blackbox/blackbox_events.py",
                       ".claude/runtime/node_blackbox/blackbox_eta.py",
                       ".claude/runtime/node_blackbox/blackbox_session.py",
                       ".claude/runtime/node_blackbox/logs_lifecycle.py",
                       ".claude/runtime/node_blackbox/seed_from_journal.py",
                       ".claude/runtime/node_blackbox/install_node_blackbox.sh",
                       ".claude/runtime/node_blackbox/verify_node_blackbox.sh",
                       ".claude/runtime/node_blackbox/purge_host_safety.sh",
                       ".claude/runtime/node_blackbox/agent_guard.py",
                       ".claude/runtime/node_blackbox/adversarial_stress.py",
                       ".claude/runtime/node_blackbox/regen_envelope.py",
                       ".claude/runtime/node_blackbox/node_identity.sh",
                       ".claude/runtime/node_blackbox/budget_renew_loop.sh",
                       ".claude/runtime/node_blackbox/blackbox_thermal.py",
                       ".claude/runtime/node_blackbox/thermal_watchdog.sh"]
        have = all(os.path.exists(os.path.join(out, p)) for p in base_expect)
        missing_art = [p for p in base_expect if not os.path.exists(os.path.join(out, p))]
        # docs 스켈레톤: docs.md 계약 5종(DOC_TYPES) 전부 렌더됐나(simlog·benchmark 누락 회귀 차단 — review)
        rendered_doc_types = {os.path.basename(os.path.dirname(p))
                              for p in glob.glob(os.path.join(out, "docs", "*", "example.md"))}
        docs_contract_ok = set(DOC_TYPES).issubset(rendered_doc_types)
        if not docs_contract_ok:
            missing_art.append("docs/{%s}/example.md" % ",".join(sorted(set(DOC_TYPES) - rendered_doc_types)))
        # 렌더 산출물에 미치환 placeholder 0
        leftover = []
        for p in ("CLAUDE.md", "Agent_Card.json", ".claude/settings.local.json"):
            with open(os.path.join(out, p), encoding="utf-8") as f:
                leftover += _unrendered(f.read())
        # JSON 유효
        for p in ("Agent_Card.json", ".claude/settings.local.json",
                  ".claude/schemas/task-report.schema.json",
                  ".claude/schemas/library-exchange.schema.json"):
            with open(os.path.join(out, p), encoding="utf-8") as f:
                json.load(f)
        # settings: 로컬 git allow + 원격 deny 정합(D12-02)
        with open(os.path.join(out, ".claude/settings.local.json"), encoding="utf-8") as f:
            st = json.load(f)
        allow, deny = st["permissions"]["allow"], st["permissions"]["deny"]
        git_ok = ("Bash(git commit:*)" in allow and "Bash(git checkout:*)" in allow
                  and "Bash(git commit:*)" not in deny
                  and all(f"Bash(git {r}:*)" in deny for r in ("push", "pull", "fetch", "remote", "clone")))
        c4 = have and not leftover and git_ok and docs_contract_ok
        print(f"  [{'PASS' if c4 else 'FAIL'}] 전체 렌더(미치환={leftover}, 누락아티팩트={missing_art}, git권한정합={git_ok}, docs계약5종={sorted(rendered_doc_types)})")
        ok &= c4
    except SystemExit as e:
        print(f"  [FAIL] 렌더 예외: {e}")
        ok = False

    # (5) D12 dual-topology: single 토폴로지 렌더(브랜치 맥락 single) — TOPOLOGY 치환 정합
    data5 = parse_manifest(mpath); data5["topology"] = "single"
    ph5, _ = build_placeholders(data5)
    out5 = os.path.join(tmp, "sub_provision_single")
    try:
        render_tree(ph5, out5, copy_runtime_block=False)
        with open(os.path.join(out5, "CLAUDE.md"), encoding="utf-8") as f:
            claude_single = f.read()
        # 2026-09-03(S2 · plan_26090317 P1): 이전 단언은 `"single" in claude_single` 이었다.
        #   템플릿 본문에 "single" 이라는 낱말이 리터럴로 있어 **항상 참**이었고, 페르소나가
        #   "너 = slave(Ray worker)" 라고 말해도 PASS 했다(실제로 PASS 하고 있었다). 변이가 통과하는
        #   시험은 커버리지 구멍의 신호다 — 이제 **정체성 문장 자체**를 양방향으로 본다.
        ray_marks = ("slave(Ray worker)", "Ray head", "RAY_PORT", "/dev/infiniband", "--profile slave")
        a2a_marks = ("A2A 원격 에이전트", "AgentCard")
        leaked = [m for m in ray_marks if m in claude_single]
        has_a2a = all(m in claude_single for m in a2a_marks)
        no_markers = "<!-- MODE:" not in claude_single      # 마커가 산출물에 새면 안 된다
        c5 = (ph5["TOPOLOGY"] == "single" and not _unrendered(claude_single)
              and not leaked and has_a2a and no_markers)
        print(f"  [{'PASS' if c5 else 'FAIL'}] single 렌더 페르소나=a2a-agent "
              f"(ray누수={leaked or '없음'} a2a문장={has_a2a} 마커제거={no_markers})")
        ok &= c5

        # 대조군(양성): multi 렌더는 Ray 워커 정체성을 **가져야** 한다 — 한쪽만 보면 "전부 지우기" 가
        #   통과해 버린다. 두 방향을 함께 봐야 게이트가 실질이 된다.
        data5m = parse_manifest(mpath); data5m["topology"] = "multi"
        ph5m, _ = build_placeholders(data5m)
        out5m = os.path.join(tmp, "sub_provision_multi_persona")
        render_tree(ph5m, out5m, copy_runtime_block=False)
        with open(os.path.join(out5m, "CLAUDE.md"), encoding="utf-8") as f:
            claude_multi = f.read()
        c5b = ("slave(Ray worker)" in claude_multi
               and not any(m in claude_multi for m in a2a_marks)
               and "<!-- MODE:" not in claude_multi)
        print(f"  [{'PASS' if c5b else 'FAIL'}] multi 렌더 페르소나=ray-worker (대조군 — 양방향 확인)")
        ok &= c5b
    except SystemExit as e:
        print(f"  [FAIL] single 렌더 예외: {e}")
        ok = False

    # (4b) tool_plane 토폴로지 게이팅 (2026-09-03 · P2 · plan_26090317).
    #   결함의 형태: 렌더러가 **토폴로지와 무관한 닫힌 리스트**로 런타임블럭을 배달해, 멀티 서브
    #   (Ray 워커)에게도 서빙전략·벤치 스킬이 갔다. 판정을 계약(node_role_contract.tool_plane)으로
    #   옮겼으므로, 여기서는 **양방향**을 본다 — 한쪽만 보면 "전부 안 주기"가 통과한다.
    for _topo, _want, _label in (("single", {"vllm-recipe-explorer", "adversarial-benchmark",
                                             "upstream-version-watch"}, "a2a-agent=3종"),
                                 ("multi", set(), "ray-worker=0종")):
        _d = parse_manifest(mpath); _d["topology"] = _topo
        _ph, _ = build_placeholders(_d)
        _out = os.path.join(tmp, f"tp_{_topo}")
        render_tree(_ph, _out, copy_runtime_block=True)
        _got = {os.path.basename(x) for x in glob.glob(os.path.join(_out, ".claude", "skills", "*"))
                if os.path.isdir(x)}
        _ref = os.path.exists(os.path.join(_out, ".claude/skills/wiki-desk/reference/references.md"))
        # references.md 는 recipe 스킬의 의존이므로 recipe 가 갈 때만 간다.
        _ref_ok = _ref == ("vllm-recipe-explorer" in _want)
        _c = (_got - {"wiki-desk"}) == _want and _ref_ok
        print(f"  [{'PASS' if _c else 'FAIL'}] tool_plane 게이팅 {_topo}({_label}): 배달={sorted(_got)} refs={_ref}")
        ok &= _c

    # (4b-2) upstream 스킬의 **경로 단위 분할** — 서브는 해소·렌더 능력만 받고 노드 간
    #   오케스트레이션(sync_to_sub·sync_branches·fetch_sub_docs·multinode_*)은 받지 않는다.
    #   이 분류가 새 스크립트에서 조용히 새는 것을 막기 위해 **두 방향**을 본다:
    #     ① 제외 대상이 실제로 서브에 없다  ② 서브가 필요로 하는 것은 실제로 있다
    #   그리고 ③ 정본에 새 "노드 간" 스크립트가 생겼는데 분류표에 없으면 fail-loud 한다(tripwire).
    _d = parse_manifest(mpath); _d["topology"] = "single"
    _ph, _ = build_placeholders(_d)
    _out = os.path.join(tmp, "upstream_split")
    render_tree(_ph, _out, copy_runtime_block=True)
    _uw = os.path.join(_out, ".claude", "skills", "upstream-version-watch")
    _leaked = [x for x in RUNTIME_BLOCK_EXCLUDES["upstream-version-watch"]
               if os.path.exists(os.path.join(_uw, x))]
    _needed = ["scripts/resolve_wheel.py", "scripts/resolve_torch_pin.py",
               "scripts/render_dockerfile.py", "scripts/regen_requirements.py",
               "scripts/classify_failure.py", "references/resolve-and-render.md"]
    _absent = [x for x in _needed if not os.path.exists(os.path.join(_uw, x))]
    # ③ 정본의 노드 간 스크립트 전수 ⊆ 제외표. 이름에 sub/branch/multinode 가 든 셸 스크립트를
    #    "노드 간" 후보로 본다 — 새 파일이 생기면 분류를 강제한다(닫힌 목록 tripwire).
    _canon = os.path.join(REPO, ".claude", "skills", "upstream-version-watch", "scripts")
    _cands = {f"scripts/{n}" for n in os.listdir(_canon)
              if n.endswith(".sh") and any(k in n for k in ("sub", "branch", "multinode", "clone"))}
    _unclassified = sorted(_cands - set(RUNTIME_BLOCK_EXCLUDES["upstream-version-watch"]))
    _c4b2 = not _leaked and not _absent and not _unclassified
    print(f"  [{'PASS' if _c4b2 else 'FAIL'}] upstream 경로 분할: 누수={_leaked or '없음'} "
          f"필요분누락={_absent or '없음'} 미분류={_unclassified or '없음'}")
    ok &= _c4b2

    # (4c) 렌더 결정론 — 같은 입력이면 **바이트가 같아야** 한다(P3 재정착 합격 기준의 전제).
    _d1 = parse_manifest(mpath); _ph1, _ = build_placeholders(_d1)
    _o1, _o2 = os.path.join(tmp, "det_a"), os.path.join(tmp, "det_b")
    render_tree(_ph1, _o1, copy_runtime_block=True)
    render_tree(_ph1, _o2, copy_runtime_block=True)
    _digest = {}
    for _tag, _root in (("a", _o1), ("b", _o2)):
        _h = hashlib.sha256()
        for _f in sorted(glob.glob(os.path.join(_root, "**"), recursive=True)):
            if os.path.isfile(_f):
                _h.update(os.path.relpath(_f, _root).encode())
                with open(_f, "rb") as _fh:
                    _h.update(_fh.read())
        _digest[_tag] = _h.hexdigest()
    _cdet = _digest["a"] == _digest["b"]
    print(f"  [{'PASS' if _cdet else 'FAIL'}] 렌더 결정론(같은 입력 → 같은 바이트): {_digest['a'][:16]}…")
    ok &= _cdet

    # (5c) 런타임블럭 복제 경로 — 2026-09-03(S10/㉓/F3 · plan_26090317 P1) 신설.
    #   감사 지적: `_copy_tracked` 의 모든 self-test 호출이 copy_runtime_block=False 라 **이 경로를
    #   한 번도 타지 않았다**. 그 사이 프로덕션에서는 git 없는 트랜잭션 트리 때문에 폴백이 상시 경로였다.
    #   세 방향을 본다: ① 실 git 목록 ② 주입 목록 ③ git 없는 트리에서 fail-loud.
    rb_dir = os.path.join(REPO, ".claude", "skills", "vllm-recipe-explorer")
    if os.path.isdir(rb_dir):
        d5c = os.path.join(tmp, "rb_git"); os.makedirs(d5c, exist_ok=True)
        n_git = _copy_tracked(rb_dir, d5c)
        src_git = getattr(_copy_tracked, "last_source", None)
        d5d = os.path.join(tmp, "rb_inject"); os.makedirs(d5d, exist_ok=True)
        listed = subprocess.run(["git", "-C", REPO, "ls-files", "-z"],
                                capture_output=True, text=True, timeout=20)
        inject = [x for x in listed.stdout.split("\0") if x.strip()]
        n_inj = _copy_tracked(rb_dir, d5d, tracked_list=inject)
        src_inj = getattr(_copy_tracked, "last_source", None)
        # ③ git 이 없는 트리: 예전엔 조용히 os.walk 로 넘어갔다. 이제는 정지해야 한다.
        gitless = os.path.join(tmp, "gitless_repo", ".claude", "skills", "vllm-recipe-explorer")
        os.makedirs(gitless, exist_ok=True)
        with open(os.path.join(gitless, "SKILL.md"), "w", encoding="utf-8") as f:
            f.write("x\n")
        _saved_repo = globals()["REPO"]
        globals()["REPO"] = os.path.join(tmp, "gitless_repo")
        try:
            _copy_tracked(gitless, os.path.join(tmp, "rb_gitless"))
            failed_loud = False
        except SystemExit:
            failed_loud = True
        finally:
            globals()["REPO"] = _saved_repo
        c5c = (n_git > 0 and n_git == n_inj and src_git == "measured:git-ls-files"
               and src_inj == "injected:tracked-list" and failed_loud)
        print(f"  [{'PASS' if c5c else 'FAIL'}] 런타임블럭 복제: git={n_git} 주입={n_inj} "
              f"출처={src_git}/{src_inj} git없는트리→fail-loud={failed_loud}")
        ok &= c5c

    # (6) A2A 위임 키(plan_26063021_14_37 D5/D7): nodes[sub].hw_verified=true → 키 발급 / 부재 → 미발급(fail-closed).
    data6 = parse_manifest(mpath)
    ph6a, _ = build_placeholders(data6)                       # 기본 fixture(sub hw_verified 없음) → 미발급
    out6a = os.path.join(tmp, "sub_provision_nokey")
    render_tree(ph6a, out6a, copy_runtime_block=False)
    key6a_absent = not os.path.exists(os.path.join(out6a, ".claude", "a2a_delegation.json"))
    _node(data6, "sub")["hw_verified"] = "true"               # 동질성 검증 통과 주입 → 발급
    ph6b, _ = build_placeholders(data6)
    out6b = os.path.join(tmp, "sub_provision_key")
    render_tree(ph6b, out6b, copy_runtime_block=False)
    keyp = os.path.join(out6b, ".claude", "a2a_delegation.json")
    key6b_ok = False
    if os.path.exists(keyp):
        with open(keyp, encoding="utf-8") as f:
            _kd = json.load(f)
        key6b_ok = _kd.get("delegation") == "main_cluster_flag" and _kd.get("issued_to") == "sub"
    c6 = key6a_absent and key6b_ok
    print(f"  [{'PASS' if c6 else 'FAIL'}] A2A 위임 키: hw_verified 부재→미발급({key6a_absent}) · =true→발급+유효({key6b_ok})")
    ok &= c6

    # (7) ★ 토폴로지 축 4필드 회귀핀(testlog_26082215 S-11 FAIL 재발 차단):
    #     Agent_Card.json:node_identity 가 rank·rank_source·sub_mode·sub_mode_source 를 싣고,
    #     그 값이 node_role_contract 산출과 **정확히 같은지**(렌더러가 두 번째 파생을 하지 않는지).
    data7 = parse_manifest(mpath)                                   # fixture = topology: multi
    ph7m, _ = build_placeholders(data7)
    out7m = os.path.join(tmp, "sub_provision_axis_multi")
    render_tree(ph7m, out7m, copy_runtime_block=False)
    with open(os.path.join(out7m, "Agent_Card.json"), encoding="utf-8") as f:
        card_m = json.load(f)["node_identity"]
    data7s = parse_manifest(mpath); data7s["topology"] = "single"
    ph7s, _ = build_placeholders(data7s)
    out7s = os.path.join(tmp, "sub_provision_axis_single")
    render_tree(ph7s, out7s, copy_runtime_block=False)
    with open(os.path.join(out7s, "Agent_Card.json"), encoding="utf-8") as f:
        card_s = json.load(f)["node_identity"]
    axis_keys = ("rank", "rank_source", "sub_mode", "sub_mode_source")
    keys_ok = all(k in card_m for k in axis_keys) and all(k in card_s for k in axis_keys)
    # multi: rank 는 **정수 1**(nodes[main,sub] 의 인덱스)이지 문자열 "1" 이 아니다 — 하류가 TP/NCCL
    #        입력으로 읽으므로 타입이 계약이다. single: 리터럴 None + 음성정직 출처(조용한 0 ✗).
    multi_ok = (card_m["sub_mode"] == "ray-worker"
                and card_m["sub_mode_source"] == "derived-from-topology"
                and card_m["rank"] == 1 and isinstance(card_m["rank"], int)
                and card_m["rank_source"] == "manifest-nodes-index")
    single_ok = (card_s["sub_mode"] == "a2a-agent"
                 and card_s["rank"] is None
                 and card_s["rank_source"] == "not-applicable:single-a2a-agent")
    # 단일 소유 확증: 렌더 산출물이 판정기 산출과 문자 그대로 같아야 한다(재저작 ✗).
    owner_s = _contract.evaluate_manifest(data7s, topology="single", role="sub")
    owner_ok = (card_s["sub_mode"] == owner_s["sub_mode"]["value"]
                and card_s["sub_mode_source"] == owner_s["sub_mode"]["source"]
                and card_s["rank"] == owner_s["rank"]["value"]
                and card_s["rank_source"] == owner_s["rank"]["source"])
    c7 = keys_ok and multi_ok and single_ok and owner_ok
    print(f"  [{'PASS' if c7 else 'FAIL'}] 토폴로지 축 4필드 탑재(keys={keys_ok}, multi={multi_ok}, "
          f"single={single_ok}, 판정기일치={owner_ok}) → multi rank={card_m.get('rank')!r} / single rank={card_s.get('rank')!r}")
    ok &= c7

    # (8) 계약 위반(single 에 ray-worker 선언) → 렌더 fail-loud(빈 정체성 렌더 금지)
    data8 = parse_manifest(mpath); data8["topology"] = "single"
    _node(data8, "sub")["sub_mode"] = "ray-worker"
    ph8, missing8 = build_placeholders(data8)
    c8 = ("SUB_MODE" in missing8
          and ph8.get("SUB_CONTRACT_VIOLATIONS") == "SUB_MODE_TOPOLOGY_CONFLICT")
    print(f"  [{'PASS' if c8 else 'FAIL'}] 계약 위반 → fail-loud(missing={sorted(set(missing8))[:3]}, "
          f"violations={ph8.get('SUB_CONTRACT_VIOLATIONS')!r})")
    ok &= c8

    # (템플릿 PII-free 는 upstream-version-watch owner-local smoke_clone.sh A4가 단일 게이트로 검사)
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"self-test: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="terraforming_node 서브 에이전트 환경 렌더러 (결정론)")
    ap.add_argument("--topology", choices=["single", "multi"], default="multi",
                    help="산출물 통로(output/<topology>/) + 서브 브랜치 맥락. D12: single·multi 양쪽 렌더 가능(서브 로컬 git 양 브랜치).")
    ap.add_argument("--manifest", default=None, help="manifest 경로(기본 output/<topology>/manifest.yaml)")
    ap.add_argument("--out", default=None, help="스테이징 출력(기본 output/<topology>/sub_provision)")
    ap.add_argument("--no-runtime-block", action="store_true", help="런타임블럭(vllm-recipe-explorer) 복제 생략(디버그)")
    ap.add_argument("--tracked-list", default=None,
                    help="git-tracked 경로 목록 파일(NUL 또는 개행 구분). git 이 없는 트랜잭션 트리에서 "
                         "런타임블럭을 복제할 때 호출부가 주입한다(S10 — 추측 폴백 금지).")
    ap.add_argument("--self-test", action="store_true", help="fixture 렌더 회귀(하드웨어/실 manifest 불요)")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()

    manifest = args.manifest or os.path.join(REPO, "output", args.topology, "manifest.yaml")
    out_dir = args.out or os.path.join(REPO, "output", args.topology, "sub_provision")
    if not os.path.isfile(manifest):
        print(f"[render] FAIL: manifest 없음 — {manifest} (terraforming_node 스캔/인터뷰로 먼저 채우세요)", file=sys.stderr)
        return 3
    data = parse_manifest(manifest)
    data.setdefault("topology", args.topology)   # D12: manifest 에 topology 없으면 --topology 로 채움(브랜치 맥락 보장)
    ph, missing = build_placeholders(data)
    if missing:
        print(f"[render] FAIL: manifest 필수 필드 누락 {missing} — 무증거 빈 정체성 렌더 금지.", file=sys.stderr)
        if ph.get("SUB_CONTRACT_VIOLATIONS"):
            # 왜 비었는지를 말한다 — 계약 위반이 "필드 누락"으로만 보이면 사람이 manifest 를 잘못 고친다.
            print(f"[render]       ↳ 토폴로지 축 계약 위반: {ph['SUB_CONTRACT_VIOLATIONS']} "
                  f"(node_role_contract.py evaluate --manifest {manifest} 로 상세 확인)", file=sys.stderr)
        return 2
    tracked = None
    if args.tracked_list:
        with open(args.tracked_list, encoding="utf-8") as f:
            blob = f.read()
        tracked = [x for x in (blob.split("\0") if "\0" in blob else blob.splitlines()) if x.strip()]
    res = render_tree(ph, out_dir, copy_runtime_block=not args.no_runtime_block, tracked_list=tracked)
    print(f"[render] OK → {res['out_dir']}")
    for p in res["produced"]:
        print(f"   + {p}")
    # 2026-09-03(B0 · plan_26090317 P1): 여기서 찍던 명령은 필수 인가 인자(--mode·--manifest)가 없어
    #   completion_gate 가 CLI_USAGE_ERROR 로 거부했다 — 렌더 성공 화면이 실행 불가 명령을 안내하고 있었다.
    print("[render] 다음(HITL · 인가 인자 필수):")
    print("   bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh \\")
    print("        --mode experimental --manifest <work-manifest.json> --apply --provision")
    print("   ↑ work-manifest 발행 절차 = terraforming_node SKILL.md §2.3 '인가 체인'")
    return 0


if __name__ == "__main__":
    sys.exit(main())
