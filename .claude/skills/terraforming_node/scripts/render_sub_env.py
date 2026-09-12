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
  campaigns/_bootstrap/relay/.gitkeep    ← 릴레이 원장 스캐폴드(옛 tasks/ · 2026-09-06 이관)
  campaigns/_template/**                 ← 캠페인 뼈대(메인 정본의 복제 — 서브도 같은 모양을 채운다)

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
#   adversarial-benchmark 의 (b) 외부검색 arm: 결정론 백스톱(`run_bench.sh`)이 실제로 검사하는 것은
#   **A2A 위임 키 ∨ Flag** 한 축이다 — egress arm 은 코드에 없다(2026-09-04 감사 실측). 종전 주석은
#   "이중게이트" 라고 적었으나 그 두 번째 문은 존재하지 않았다. egress 는 스캔 시점 attestation 이며
#   불일치가 fail-open 경고다. 서술을 코드에 맞춘다 — 없는 게이트를 있다고 적으면 다음 사람이 그것을
#   믿고 설계한다. 대신 그 값은 이제 서브 페르소나로 **전달된다**(`EGRESS_STATE` · plan_26090412 B8).
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
# 싱글(A2A) 서브에 배달하는 캠페인 도구. **뼈대만 보내고 채우는 손을 안 보내면 그 손은 없는 것**
# 이다 — 2026-09-07 실측: 서브 오버레이에 `campaign_init.py` 가 없어서 `broad_search`·
# `single_serve_up` 의 `[ -f "$_CI" ]` 가드가 서브에서 **침묵 no-op** 이었고, 그 결과
# `phases/sub/*` 는 캠페인이 끝난 뒤 메인이 같은 초에 통째로 저작했다(plan_26090813 F5).
# 메인 전용 오케스트레이션(relay·sync·fetch)은 여기 오지 않는다 — 배달 방향이 뒤집힌다.
CAMPAIGN_TOOLS = (
    "campaign_init.py",                # 단일 writer + 읽는 눈 + 파생 개설(--from-slice)
    "campaign_template_validator.py",  # 선언·인스턴스 술어(P1~P3). 스키마 엔진은 메인 소관
)

RUNTIME_BLOCK_EXCLUDES = {
    "upstream-version-watch": (
        # ★ 2026-09-08(plan_26090813 §4.2): 메인의 **해소 결과**는 서브에 가지 않는다. 싱글의 sub 는
        #   A2A 원격 에이전트라 자기 HW 에서 스스로 해소해야 하는데, 이 자산이 딸려가면 서브가
        #   물을 이유 자체가 사라진다(2026-09-07 실측: 서브 library_request 5회 전부 null).
        #   render_dockerfile 은 `--resolved` 를 받는 정규 경로가 있고, 이 자산은 그 인자가 없을 때의
        #   **공유 폴백**이다 — 폴백이 없으면 fail-loud 하므로 침묵 누락이 되지 않는다.
        "assets/current-production-resolution.json",
        "scripts/sync_to_sub.sh",          # 메인→서브 배달(방향이 뒤집힌다)
        "scripts/sync_branches.sh",        # 공유 빌딩블럭 브랜치 동기(메인 소관)
        "scripts/fetch_sub_docs.sh",       # 서브→메인 문서 회수(메인이 당긴다)
        "scripts/smoke_clone.sh",          # 배포본 클론 검증(메인 소관 · 이미 retirement 대상)
        "scripts/multinode_comms_smoke.sh",  # 노드 간 통신 스모크(메인이 양노드를 향해 쓴다)
        "scripts/multinode_serve_smoke.sh",  # 동상
    ),
}
DOCS_RULES = os.path.join(REPO, ".claude", "rules", "docs.md")     # 문서규약(정적계약 — 서브 테라포밍, D12)
# 특화헌법(2026-09-12 · plan_26091210 A7 · policy BRANCH_CONSTITUTION_LAYERING). **명시 등록이 필요하다** —
#   서브로 가는 `.claude/rules/*` 는 이 파일이 이름 하나하나로 정하는 닫힌 목록이고(`comms.md`·`docs.md`
#   둘뿐이었다), 규약에 맞는 파일이 자동으로 따라가지 않는다. 등록하지 않으면 서브는 자기 토폴로지의
#   헌법을 **영원히 받지 못한다**.
#   `orchestration.topology.md` 는 여기 없다 — `terraforming_node` 자체가 서브로 가지 않기 때문이고,
#   그것은 결함이 아니라 의도다(오케스트레이션은 메인의 일이다).
TOPOLOGY_RULES = os.path.join(REPO, ".claude", "rules", "strategy.topology.md")
DOC_SKELETONS = os.path.join(SKILL_DIR, "templates", "document_skeletons")  # 배포 포함 docs/*/example.md 정본(D12)
RECIPE_REFERENCE = os.path.join(REPO, ".claude", "skills", "wiki-desk", "reference", "references.md")

# 토폴로지 축 계약(sub_mode·rank·정체성 권위)의 **단일 소유자**. 렌더러는 산출을 **적기만** 한다 —
# 두 번째 파생 구현을 두면 두 답이 갈리고(헌법 §4종 안티패턴 "파생 가능한데 손으로 적은 것"),
# 그 갈림이 2026-08-22 진단의 형태였다(SKILL.md §2.7.6(c) · 헌법 §불변식 A).
sys.path.insert(0, HERE)
import node_role_contract as _contract  # noqa: E402  (형제 스크립트 — 위 sys.path 선행 필요)
import agent_card_contract as _acc      # noqa: E402  Agent_Card v2 계약·JWS 서명(단일 소유 · plan_26090516 §7.2)
import manifest_contract as _mc         # noqa: E402  서브 manifest Flag 계약 리더(§7.3)

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



MODE_PREFIX_RE = re.compile(r"^MODE:(?P<mode>[a-z0-9-]+)\s+(?P<rest>.+)$", re.DOTALL)


def apply_mode_prefixes(obj, sub_mode: str):
    """JSON 배열 원소의 `MODE:<mode> <값>` 접두어로 모드를 가른다(재귀).

    왜 블록 마커가 아니라 접두어인가(2026-09-03 · plan_26090317 P4):
        `<!-- MODE:x -->` 는 템플릿을 **유효하지 않은 JSON** 으로 만든다. 그러면 템플릿 자체를
        `json.tool` 로 검증할 수 없고, 렌더 전에는 아무도 그 파일이 깨졌는지 모른다. 값 접두어는
        템플릿을 유효 JSON 으로 유지하면서 같은 일을 하고, 목록은 여전히 **템플릿 한 곳**에만 있다
        (렌더러에 경로 목록을 다시 적으면 그 순간 두 자리가 갈라진다 — 하드코딩 결함 칸).

    왜 필요한가:
        서브의 쓰기 권한 평면이 `output/**/{Dockerfile,docker-compose.yaml,requirements.txt}` 를
        **양 모드 모두** deny 했다. 그 목록은 **멀티 상정**이다 — 거기서는 메인이 빌드킷을 배달하므로
        서브가 고치면 안 된다. 그런데 헌법은 **싱글 서브는 자기 빌드킷을 자율 저작한다**(불변식 A)고
        못박고, `sync_to_sub` 도 그래서 싱글에 빌드킷을 보내지 않는다(빌드킷 평면 dormant).
        즉 **저작하라고 해놓고 쓰기를 막았다** — 토폴로지 제1축이 권한 평면에는 아직 안 닿아 있었다.
        실측(2026-09-03 P4): 서브 build 턴 착수 직전에 발견. 서브에는 `render_dockerfile.py`·
        `regen_requirements.py`·정본 러너 assets 이 이미 다 배달돼 있어 **권한만이 유일한 벽**이었다.
    """
    if isinstance(obj, dict):
        return {k: apply_mode_prefixes(v, sub_mode) for k, v in obj.items()}
    if isinstance(obj, list):
        out = []
        for item in obj:
            if isinstance(item, str):
                m = MODE_PREFIX_RE.match(item)
                if m:
                    mode = m.group("mode")
                    if mode not in KNOWN_SUB_MODES:
                        raise SystemExit(f"[render] FAIL: 미지의 MODE 접두 '{mode}' — 알려진 모드 {KNOWN_SUB_MODES}")
                    if mode != sub_mode:
                        continue
                    out.append(m.group("rest"))
                    continue
            out.append(apply_mode_prefixes(item, sub_mode))
        return out
    return obj


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
    data: dict = {"interconnect": {}, "network": {}, "nodes": []}
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
                # `network:` 는 2026-09-04 추가(plan_26090412 B8). 종전에는 중첩 매핑 섹션이
                #   `interconnect` 하나뿐이라 `network.egress` 가 **조용히 버려졌다** — 값은
                #   manifest 에 실재했고(스캐너가 적었다) 읽는 코드가 0 이었다(감사 D2/I).
                if s.startswith("network:"):
                    section = "network"; cur = None
                    data.setdefault("network", {}); continue
                if s.startswith("nodes:"):
                    section = "nodes"; cur = None; continue
                section = None; cur = None
                k, _, v = s.partition(":")
                data[k.strip()] = _clean(v)
            elif section in ("interconnect", "network"):
                k, _, v = s.partition(":")
                data[section][k.strip()] = _clean(v)
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


def build_placeholders(data: dict, sub_manifest: dict | None = None) -> tuple[dict, list[str]]:
    """manifest → {{KEY}} 치환 dict. 반환 (placeholders, missing_required).

    `sub_manifest`(2026-09-05 · plan_26090516 §7.3): 서브 자신의 manifest(terraforming 이 실측·생성).
    주어지면 HW·경로 placeholder(GPU_MODEL·CPU_ARCH·NAS_MOUNT·EGRESS_STATE)는 **서브 manifest** 에서
    읽는다 — 종전에는 메인 manifest 의 메인 HW 를 서브 페르소나에 적었다(메인=서브 동질성 가정).
    """
    hw = sub_manifest if isinstance(sub_manifest, dict) and sub_manifest else data
    ic = data.get("interconnect", {})
    sub = _node(data, "sub")
    main = _node(data, "main")
    sub_host = sub.get("host", "")
    work_dir = sub.get("work_dir") or main.get("work_dir", "")
    ssh_user = sub.get("ssh_user") or main.get("ssh_user", "")
    gpus = hw.get("gpus_per_node", "")
    cpu_arch = hw.get("cpu_arch", "")

    ph = {
        "SUB_HOST": sub_host,
        "SUB_HOSTNAME": sub.get("hostname") or sub_host,
        "MASTER_HOST": main.get("host", ""),
        "SSH_USER": ssh_user,
        "WORKSPACE_PATH": work_dir,
        "NAS_MOUNT": hw.get("nas_model_path", ""),
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
        # GPU_MODEL 은 폴백 없음(2026-09-05 · audit_26090515 B9): 종전 `"<n>x-<arch>"`/`"unknown-gpu"` 폴백은
        #   Agent_Card.node_identity.gpu_model → 서브 인증서 **강한 키** `gpu` 로 흘러 위조 정체성이 인증서에
        #   실렸다. HW 사실은 manifest 가 권위이며 없으면 아래 `required` fail-loud 로 렌더가 멈춘다.
        "GPU_MODEL": hw.get("gpu_model") or "",
        # A2A 위임 키 발급 판정용(plan_26063021_14_37 D5/D7) — nodes[sub].hw_verified(동질성 검증 통과 표식). 템플릿 치환엔 미사용.
        "SUB_HW_VERIFIED": (sub.get("hw_verified") or ""),
        # ── egress attestation → 서브 페르소나(2026-09-04 · plan_26090412 B8) ──
        #   이 파일의 머리말은 2026-07 부터 *"렌더 시 서브 env 에 egress attestation 을 반영해 서브
        #   페르소나가 자기 능력을 정확히 로드한다"* 고 적어 왔지만 **그 코드가 없었다**(감사 D6).
        #   B안(서브 직접 검색)을 켠 이상 egress 는 정보성 부기가 아니라 **전제**다 — 검색 권한만
        #   주고 도달 가능성을 안 알려주면 서브는 조용히 빈손이 되고, 그 빈손을 근거 부족으로
        #   구분하지 못한다. 값이 없으면 `unknown` 이다(모르는 것을 online 으로 적지 않는다).
        "EGRESS_STATE": (hw.get("network") or {}).get("egress") or "unknown",
    }
    ph.update(_contract_placeholders(data))
    # 필수(누락 시 fail-loud — 무증거/빈 정체성 렌더 금지)
    #   SUB_MODE·*_SOURCE 는 계약이 해소하는 파생값이다 — 빈 값 = 계약 미해소(위반 또는 서브 미등록)이며
    #   그대로 렌더하면 서브의 정체성 권위(AgentCard)가 거짓을 싣는다. 그래서 같은 fail-loud 통로에 둔다.
    #   ⚠ SUB_RANK 는 여기 넣지 않는다 — single 의 정답이 리터럴 `null` 이라 "빈 값"과 구분돼야 한다(음성정직).
    required = ["SUB_HOST", "MASTER_HOST", "SSH_USER", "WORKSPACE_PATH", "NAS_MOUNT",
               "CPU_ARCH", "GPU_MODEL", "INTERCONNECT", "INTERCONNECT_IFACE",
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
    ia = res.get("identity_authority") or {}
    dp = res.get("delivery_plane") or {}
    return {
        "SUB_MODE": sub_mode.get("value") or "",
        "SUB_MODE_SOURCE": sub_mode.get("source") or "",
        # 2026-09-05(plan_26090516 ② · H1): Agent_Card v2 는 A2A 1.0.1 표준 필드만 최상위에 두고 우리
        #   노드 역할 계약을 확장(capabilities.extensions[urn:easy-vllm:ext:node-role:v1].params) 하나에 싣는다.
        #   아래 셋은 그 params 의 나머지 해소값이다 — 역시 판정기 산출을 옮겨 적을 뿐이다.
        "IDENTITY_AUTHORITY": ia.get("value") or "",
        "DELIVERY_PLANE": dp.get("value") or "",
        "TOOL_PLANE_JSON": json.dumps(list(tp.get("value") or []), ensure_ascii=False),   # JSON 배열(따옴표 없이 놓인다)
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
                tracked_list: list | None = None, signing_key: str | None = None,
                sub_manifest_path: str | None = None) -> dict:
    """치환된 placeholders 로 스테이징 트리를 만든다. 반환 = 산출 매니페스트(검증용).

    `signing_key`(2026-09-05 · plan_26090516 §7.2 H1(A)): 메인 Ed25519 PEM. 렌더된 Agent_Card 를
    A2A §8.4 JWS 로 서명하고 공개키(JWK)를 `.claude/a2a/trusted_keys.json` 으로 함께 싣는다 — 서브·메인
    게이트가 이 저장소로 검증한다. **사람 개입 0**(H1 조건). None 이면 서명하지 않는다(self-test 전용 —
    main() 은 키 부재를 fail-loud 로 막는다).
    `sub_manifest_path`(§7.3): terraforming 이 실측·생성한 서브 manifest. 스테이징의
    `output/<topology>/manifest.yaml` 로 복제한다(설치 산출물 — 빌드킷 배달 평면(D10)과 무관).
    """
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    claude = os.path.join(out_dir, ".claude")
    os.makedirs(os.path.join(claude, "rules"), exist_ok=True)
    os.makedirs(os.path.join(claude, "schemas"), exist_ok=True)
    # 2026-09-06(plan_26090616 ②): 옛 릴레이 원장 루트 `tasks/` 는 폐지됐다. 빈 디렉터리를
    #   남기면 배달 게이트가 "undeclared transfer artifact" 로 막고(실측), 막지 않더라도
    #   서브에게 "여기가 원장 자리다" 라고 계속 말한다. 새 자리는 campaigns/_bootstrap/relay/ 다.
    os.makedirs(os.path.join(out_dir, "campaigns", "_bootstrap", "relay"), exist_ok=True)

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
            obj = apply_mode_prefixes(obj, ph.get("SUB_MODE", ""))   # S2: 권한 평면도 정체성으로 갈린다
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
    card_path = _render_file("Agent_Card.template.json", "Agent_Card.json", "json")
    _render_file("settings.local.template.json", ".claude/settings.local.json", "json")

    # 1.5) Agent_Card v2 계약 검증 + JWS 서명 (A2A v1.0.1 · agent_card_contract.py 단일 소유)
    with open(card_path, encoding="utf-8") as f:
        card = json.load(f)
    violations = _acc.validate_card(card) + _acc.require_skills(card)
    if violations:
        raise SystemExit("[render] FAIL: Agent_Card 계약 위반 — " + "; ".join(violations))
    if signing_key:
        signed = _acc.sign_card(card, signing_key)
        with open(card_path, "w", encoding="utf-8") as f:
            json.dump(signed, f, ensure_ascii=False, indent=2)
            f.write("\n")
        _ser, _ = _acc._crypto()
        _pub = _acc._load_private(signing_key).public_key().public_bytes(
            _ser.Encoding.Raw, _ser.PublicFormat.Raw)
        jwk = _acc.jwk_from_public(_pub)
        jwk["issued_by"] = "main"
        a2a_dir = os.path.join(claude, "a2a")
        os.makedirs(a2a_dir, exist_ok=True)
        trusted_path = os.path.join(a2a_dir, "trusted_keys.json")
        with open(trusted_path, "w", encoding="utf-8") as f:
            json.dump({"schema_version": 1, "keys": [jwk]}, f, ensure_ascii=False, indent=2)
            f.write("\n")
        _acc.verify_card(signed, trusted_path)      # 서명 직후 자기 검증 — 배달 전 RED 를 여기서 잡는다
        produced.append(".claude/a2a/trusted_keys.json")
        produced.append("Agent_Card.json (signed · kid=%s)" % jwk["kid"])

    # 1.6) 서브 manifest (설치 산출물 · terraforming 실측) → 스테이징 output/<topology>/manifest.yaml
    if sub_manifest_path:
        sub_man = _mc._load_manifest(sub_manifest_path)   # 계약 리더의 로더(pyyaml · 중첩 terraforming 블록 필요)
        if sub_man.get("self_role") != "sub":
            raise SystemExit(f"[render] FAIL: 서브 manifest 의 self_role 이 'sub' 가 아니다({sub_man.get('self_role')!r}) — {sub_manifest_path}")
        _topo = sub_man.get("topology") or ph.get("TOPOLOGY") or ""
        _res = _mc.evaluate_contract(sub_man, _topo)
        if not _res.get("flag"):
            raise SystemExit(f"[render] FAIL: 서브 manifest 가 테라포밍 계약을 통과하지 못한다 — {_res.get('reason')} ({sub_manifest_path})")
        dest_rel = os.path.join("output", _topo, "manifest.yaml")
        dest = os.path.join(out_dir, dest_rel)
        os.makedirs(os.path.dirname(dest), exist_ok=True)
        shutil.copyfile(sub_manifest_path, dest)
        produced.append(dest_rel + " (서브 manifest · terraforming 실측 · Flag issued_by=main)")

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

    # 특화헌법 — 서브는 메인과 **같은 브랜치**이므로 이 체크아웃의 특화층이 곧 서브의 것이다
    # (policy BRANCH_CONSTITUTION_LAYERING · 메인 single ⇒ 서브 single, 메인 multi ⇒ 서브 multi).
    if os.path.isfile(TOPOLOGY_RULES):
        shutil.copyfile(TOPOLOGY_RULES, os.path.join(claude, "rules", "strategy.topology.md"))
        produced.append(".claude/rules/strategy.topology.md")

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
    # 2026-09-05(③ 3-9 · G-E1): **위임 키 발급 중단**. 여기서 만들던 `.claude/a2a_delegation.json`
    #   은 "메인이 서브에게 준 실행 허가" 였고, 감사는 그것을 R3(에이전트 자율성 부정)로 판정했다.
    #   대체물은 이미 이 렌더가 만든다 — 서명된 `Agent_Card.json` + `.claude/a2a/trusted_keys.json`
    #   (정체성 증명) + 메인이 발급한 서브 manifest(완수 Flag). 게이트들은 허가가 아니라 그것을 본다.

    # 4) 캠페인 워크스페이스 스캐폴드 (2026-09-06 이관 · plan_26090616 ②)
    #    옛 `tasks/.gitkeep` 을 대신한다. 서브도 메인과 **같은 뼈대**를 받아 자기
    #    `campaigns/<camp-id>/` 를 자율 저작한다 — 메인은 서브의 인스턴스를 읽지도 고치지도
    #    않으며(무단 스캔 금지), 결과는 publish phase 가 만든 문서로 돌아온다.
    #    활성 캠페인이 없을 때의 릴레이 원장은 예약 id `_bootstrap` 아래로 간다.
    for rel in ("campaigns/_bootstrap/relay/.gitkeep",):
        _p = os.path.join(out_dir, *rel.split("/"))
        os.makedirs(os.path.dirname(_p), exist_ok=True)
        with open(_p, "w") as f:
            f.write("")
        produced.append(rel)
    _tpl_src = os.path.join(REPO, "campaigns", "_template")
    # 부재를 조용히 건너뛰지 않는다 — 2026-09-06 실측: sync_to_sub 의 트랜잭션 소스 경로 목록에
    # `campaigns` 가 없어 뼈대가 도착하지 않았는데, 옛 판본의 `if isdir(...)` 이 그것을 **정상**
    # 으로 삼켜 서브 오버레이에서 뼈대가 통째로 사라졌다. 부재는 배선 결함이므로 소리내야 한다.
    if not os.path.isdir(_tpl_src):
        raise SystemExit(
            f"[render] FAIL: 캠페인 뼈대가 없다 — {_tpl_src}\n"
            f"   서브도 메인과 같은 뼈대를 받아야 자기 campaigns/<id>/ 를 저작할 수 있다.\n"
            f"   트랜잭션 소스에서 도는 중이라면 sync_to_sub 의 checkout-index 경로 목록에\n"
            f"   `campaigns` 가 들어 있는지 확인하라(부재는 침묵 누락이 된다).")
    if True:
        for _root, _dirs, _files in os.walk(_tpl_src):
            for _f in _files:
                _abs = os.path.join(_root, _f)
                _rel = os.path.relpath(_abs, REPO)          # campaigns/_template/...
                _dst = os.path.join(out_dir, _rel)
                os.makedirs(os.path.dirname(_dst), exist_ok=True)
                shutil.copy2(_abs, _dst)
                produced.append(_rel.replace(os.sep, "/"))

    # 4.1) 캠페인 **채우는 손**(2026-09-08 · plan_26090813 §4.2). 싱글(A2A) 서브만 받는다 —
    #      멀티의 sub 는 Ray 워커라 캠페인을 저작하는 주체가 아니다(불변식 A).
    if ph.get("SUB_MODE") == "a2a-agent":
        _ci_src = os.path.join(REPO, ".claude", "skills", "terraforming_node", "scripts")
        _ci_dst = os.path.join(claude, "skills", "terraforming_node", "scripts")
        os.makedirs(_ci_dst, exist_ok=True)
        for _tool in CAMPAIGN_TOOLS:
            _abs = os.path.join(_ci_src, _tool)
            if not os.path.isfile(_abs):
                raise SystemExit(
                    f"[render] FAIL: 캠페인 도구가 없다 — {_abs}\n"
                    f"   뼈대만 보내고 채우는 손을 안 보내면 서브의 호출부 가드가 침묵 no-op 이 된다\n"
                    f"   (2026-09-07 실측: phases/sub/* 가 캠페인 종료 후 메인 손으로 나타났다).")
            shutil.copy2(_abs, os.path.join(_ci_dst, _tool))
            produced.append(f".claude/skills/terraforming_node/scripts/{_tool}")

    # ★ 2026-09-07(plan_26090715 §5 ⑤ · 유예 결함 ⑤): `campaigns/README.md` 도 함께 보낸다.
    #   뼈대 파일은 도착하는데 **읽는 법**이 안 도착했다 — README 가 Agent 읽기 순서(0번
    #   `--resume-brief` 포함)와 채우기 규칙(`proof.ok` 는 관측이지 선언이 아니다 · 실패해도
    #   status 는 쓴다 · 증거는 여기서 태어나지 않는다)을 담는 유일한 자리다. 서브는 스키마만으로
    #   완주했지만 그건 운이지 계약이 아니다. 부재는 여기서도 소리낸다(침묵 누락 금지).
    _readme_src = os.path.join(REPO, "campaigns", "README.md")
    if not os.path.isfile(_readme_src):
        raise SystemExit(
            f"[render] FAIL: 캠페인 읽기 규약이 없다 — {_readme_src}\n"
            f"   뼈대만 보내고 읽는 법을 안 보내면 서브는 빈칸의 **의미**를 모른 채 채운다.")
    _readme_dst = os.path.join(out_dir, "campaigns", "README.md")
    os.makedirs(os.path.dirname(_readme_dst), exist_ok=True)
    shutil.copy2(_readme_src, _readme_dst)
    produced.append("campaigns/README.md")

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

    # 4.7) A2A 카드 검증기(2026-09-05 ②-b · plan_26090516 §7.2). host_safety·node_blackbox 와 **동형** 배선.
    #   ★ 왜 신설했나: ②-a 는 서브에 신뢰키 저장소(.claude/a2a/trusted_keys.json)를 배달하면서
    #     **그것을 읽는 코드를 배달하지 않았다**. 서브 재설치 라이브에서 C6(서브측 서명 검증)을 하려는
    #     순간 드러났다 — 저장소는 있는데 검증기가 없다. 감사가 이름 붙인 "실행자 0"(가드를 놓고
    #     실행 주체를 안 적는 것)의 재발이며, 처방은 **검증기를 서브 런타임으로 내리는 것**이다.
    #   canonical source 는 terraforming 스킬이 소유하고(메인 전용 스킬 트리는 서브에 가지 않는다),
    #   서브에는 헌법 runtime asset 으로 materialize 한다. stdlib + cryptography 만 쓰므로 자기완결이다.
    #   정본 소스 = 위에서 카드 계약 검증·서명에 실제로 쓴 그 모듈의 파일이다(단일 소유 — 경로를
    #   다시 조립하면 두 자리가 갈린다). 부재는 이 파일 상단의 `import agent_card_contract` 가 이미
    #   fail-closed 로 잡는다(음성대조 실측: ModuleNotFoundError · rc=1). 여기에 isfile 게이트를 더
    #   두면 **도달 불가 분기**가 된다 — 가드는 도달해야 가드다.
    card_contract_src = _acc.__file__
    rel = os.path.join(".claude", "runtime", "a2a", "agent_card_contract.py")
    dst = os.path.join(out_dir, rel)
    os.makedirs(os.path.dirname(dst), exist_ok=True)
    shutil.copyfile(card_contract_src, dst)
    os.chmod(dst, 0o755)
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

    # (2) mtu 는 표시된 이식성 폴백(9000) · gpu_model 은 **폴백 없음** → 누락이 required 로 잡혀야 한다(B9)
    data2 = dict(data); data2.pop("gpu_model", None); data2["interconnect"] = dict(data["interconnect"]); data2["interconnect"].pop("mtu", None)
    ph2, missing2 = build_placeholders(data2)
    c2 = ph2["INTERCONNECT_MTU"] == "9000" and ph2["GPU_MODEL"] == "" and "GPU_MODEL" in missing2
    print(f"  [{'PASS' if c2 else 'FAIL'}] mtu 폴백 9000 유지 · gpu_model 누락은 fail-loud(missing 에 GPU_MODEL) → got mtu={ph2['INTERCONNECT_MTU']} gpu={ph2['GPU_MODEL']!r} missing={missing2}")
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
                       ".claude/schemas/library-exchange.schema.json", "campaigns/_bootstrap/relay/.gitkeep",
                       "campaigns/README.md",
                       ".claude/rules/docs.md", ".gitignore",   # ← references.md 는 tool_plane 종속(아래 c4b)
                       # 특화헌법: 서브가 자기 토폴로지의 헌법을 받는지 fail-loud 로 확인한다
                       #   (등록을 잊으면 조용히 안 가고, 서브는 그 사실을 스스로 알 수 없다)
                       ".claude/rules/strategy.topology.md",
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
                       ".claude/runtime/node_blackbox/thermal_watchdog.sh",
                       # 신뢰키 저장소를 읽는 **실행자** — 없으면 서브 서명검증이 불가능하다(②-b)
                       ".claude/runtime/a2a/agent_card_contract.py"]
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

    # (4c) 권한 평면의 정체성 분기 (2026-09-03 · P4 · plan_26090317).
    #   결함의 형태: deny 목록이 **멀티 상정**이라 빌드킷 3종을 양 모드 모두 막았다. 헌법은 싱글
    #   서브가 자기 빌드킷을 **자율 저작**한다고 못박고 sync_to_sub 도 그래서 빌드킷을 안 보낸다 —
    #   즉 저작하라면서 쓰기를 막은 상태였다(build 턴 착수 직전 발견). 양방향으로 본다:
    #   한쪽만 보면 "전부 열기"·"전부 막기" 가 통과한다.
    _KIT = "output/**/Dockerfile"
    for _topo, _want_allow, _label in (("single", True, "a2a-agent=자율저작 allow"),
                                       ("multi", False, "ray-worker=배달분 보호 deny")):
        _d = parse_manifest(mpath); _d["topology"] = _topo
        _ph, _ = build_placeholders(_d)
        _out = os.path.join(tmp, f"perm_{_topo}")
        render_tree(_ph, _out, copy_runtime_block=False)
        with open(os.path.join(_out, ".claude/settings.local.json"), encoding="utf-8") as f:
            _st = json.load(f)
        _al, _dn = _st["permissions"]["allow"], _st["permissions"]["deny"]
        _in_allow = any(x.endswith(f"/{_KIT})") and x.startswith("Write(") for x in _al)
        _in_deny = any(x.endswith(f"/{_KIT})") and x.startswith("Write(") for x in _dn)
        # 접두어가 산출물에 새면 권한 문자열 자체가 무효가 된다(조용히 아무것도 매칭 안 함).
        _no_marker = not any(x.startswith("MODE:") for x in _al + _dn)
        # 모드 무관 항목은 양쪽 모두에 그대로 남아야 한다(필터가 과잉 삭제하지 않았는가).
        _common = ("Bash(git push:*)" in _dn and f"Edit({_ph['WORKSPACE_PATH']}/.claude/**)" in _dn)
        _c = (_in_allow == _want_allow) and (_in_deny != _want_allow) and _no_marker and _common
        print(f"  [{'PASS' if _c else 'FAIL'}] 권한 평면 {_topo}({_label}): "
              f"allow={_in_allow} deny={_in_deny} 마커제거={_no_marker} 공통보존={_common}")
        ok &= _c
    # 음성대조: 미지 MODE 접두는 조용히 지우지 않고 fail-loud 한다.
    try:
        apply_mode_prefixes({"a": ["MODE:nonesuch X"]}, "a2a-agent")
        print("  [FAIL] 미지 MODE 접두가 통과했다"); ok = False
    except SystemExit as _e:
        print(f"  [PASS] 미지 MODE 접두 → fail-loud ({str(_e)[:40]}…)")

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
        # ★ 2026-09-08: `terraforming_node` 는 **스킬 전체가 아니라 캠페인 도구 2개**만 간다.
        #   싱글(A2A) 서브만 받으며(불변식 A — 멀티의 sub 는 Ray 워커라 캠페인 저작 주체가 아니다),
        #   메인 전용 오케스트레이션(relay·sync·fetch·scan)은 여기 오지 않는다. 아래는 **양방향**:
        #   ① 있어야 할 2개가 있다 ② 그 밖의 terraforming 스크립트가 새지 않았다.
        _tn = os.path.join(_out, ".claude", "skills", "terraforming_node", "scripts")
        _tn_files = sorted(os.listdir(_tn)) if os.path.isdir(_tn) else []
        _tn_want = sorted(CAMPAIGN_TOOLS) if _topo == "single" else []
        _tn_ok = _tn_files == _tn_want
        # 해소 자산은 싱글 서브에 가지 않는다 — 가면 서브가 스스로 해소할 이유가 없어진다(F4).
        _res = os.path.exists(os.path.join(
            _out, ".claude/skills/upstream-version-watch/assets/current-production-resolution.json"))
        _c = ((_got - {"wiki-desk", "terraforming_node"}) == _want and _ref_ok and _tn_ok
              and not _res)
        print(f"  [{'PASS' if _c else 'FAIL'}] tool_plane 게이팅 {_topo}({_label}): 배달={sorted(_got)} "
              f"refs={_ref} 캠페인도구={_tn_files} 해소자산누수={_res}")
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

    # (6) 2026-09-05(G-E1): 위임 키 발급 검사 → **키가 더는 만들어지지 않는지** + 정체성 증명 자산이
    #     제자리에 있는지로 바뀐다. 허가(키)가 아니라 정체성(서명 카드 + 신뢰저장소)이 계약이다.
    data6 = parse_manifest(mpath)
    _node(data6, "sub")["hw_verified"] = "true"
    ph6, _ = build_placeholders(data6)
    out6 = os.path.join(tmp, "sub_provision_identity")
    render_tree(ph6, out6, copy_runtime_block=False)
    key_gone = not os.path.exists(os.path.join(out6, ".claude", "a2a_delegation.json"))
    card_ok = os.path.exists(os.path.join(out6, "Agent_Card.json"))
    c6 = key_gone and card_ok
    print(f"  [{'PASS' if c6 else 'FAIL'}] 위임 키 폐기: 키 미생성({key_gone}) · 정체성 자산 존재({card_ok})")
    ok &= c6

    # (7) ★ 토폴로지 축 4필드 회귀핀(testlog_26082215 S-11 FAIL 재발 차단):
    #     Agent_Card.json 의 node-role 확장 params 가 rank·rank_source·sub_mode·sub_mode_source 를 싣고,
    #     그 값이 node_role_contract 산출과 **정확히 같은지**(렌더러가 두 번째 파생을 하지 않는지).
    data7 = parse_manifest(mpath)                                   # fixture = topology: multi
    ph7m, _ = build_placeholders(data7)
    out7m = os.path.join(tmp, "sub_provision_axis_multi")
    render_tree(ph7m, out7m, copy_runtime_block=False)
    def _role_params(path):   # Agent_Card v2: 노드 역할은 A2A 확장 하나의 params 에 산다
        with open(path, encoding="utf-8") as f:
            card = json.load(f)
        exts = [e for e in card["capabilities"]["extensions"] if e.get("uri") == _acc.NODE_ROLE_EXT_URI]
        if len(exts) != 1:   # bare assert 금지(최적화 시 증발 — runtime_selftest 가 배포 python 전수 검사)
            raise SystemExit(f"[render self-test] node-role 확장이 정확히 1개여야 한다: {exts}")
        return exts[0]["params"]
    card_m = _role_params(os.path.join(out7m, "Agent_Card.json"))
    data7s = parse_manifest(mpath); data7s["topology"] = "single"
    ph7s, _ = build_placeholders(data7s)
    out7s = os.path.join(tmp, "sub_provision_axis_single")
    render_tree(ph7s, out7s, copy_runtime_block=False)
    card_s = _role_params(os.path.join(out7s, "Agent_Card.json"))
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

    # (7b) Agent_Card v2 = A2A 1.0.1 표준 필드만 최상위 + 서명 왕복 + 서브 manifest 배달 (plan_26090516 §7.2/§7.3)
    with open(os.path.join(out7s, "Agent_Card.json"), encoding="utf-8") as f:
        card_full = json.load(f)
    v7b = _acc.validate_card(card_full) + _acc.require_skills(card_full)
    std_ok = not v7b and "node_identity" not in card_full and "topology_contract" not in card_full
    key_dir = os.path.join(tmp, "a2a_signing")
    kinfo = _acc.keygen(key_dir)
    sub_man_path = os.path.join(tmp, "sub_manifest.yaml")
    with open(sub_man_path, "w", encoding="utf-8") as f:
        f.write("self_role: sub\ntopology: single\ncpu_arch: \"aarch64\"\ngpus_per_node: 1\ngpu_model: \"SUB-GPU\"\n"
                "model_source: managed\nnas_model_path: /srv/test-models\nterraforming:\n  complete: true\n  branch_verified: true\n"
                "  issued_by: main\nnodes:\n  - role: main\n    host: \"203.0.113.10\"\n  - role: sub\n    host: \"203.0.113.11\"\n")
    ph7b, _ = build_placeholders(data7s, sub_manifest=parse_manifest(sub_man_path))
    out7b = os.path.join(tmp, "sub_provision_signed")
    res7b = render_tree(ph7b, out7b, copy_runtime_block=False, signing_key=kinfo["private"], sub_manifest_path=sub_man_path)
    signed_path = os.path.join(out7b, "Agent_Card.json"); trusted_path = os.path.join(out7b, ".claude", "a2a", "trusted_keys.json")
    with open(signed_path, encoding="utf-8") as f:
        signed_card = json.load(f)
    sig_ok = bool(signed_card.get("signatures")) and _acc.verify_card(signed_card, trusted_path) == kinfo["kid"]
    forged = json.loads(json.dumps(signed_card)); forged["skills"][0]["description"] = "tampered"
    try:
        _acc.verify_card(forged, trusted_path); forge_caught = False
    except _acc.ContractViolation:
        forge_caught = True
    sub_copied = os.path.isfile(os.path.join(out7b, "output", "single", "manifest.yaml"))
    hw_from_sub = ph7b["GPU_MODEL"] == "SUB-GPU"      # 서브 페르소나 HW 는 서브 manifest 에서
    # 음성대조: self_role 이 sub 가 아닌 manifest 는 배달 거부
    bad_sub = os.path.join(tmp, "sub_manifest_bad.yaml")
    with open(sub_man_path, encoding="utf-8") as f, open(bad_sub, "w", encoding="utf-8") as g:
        g.write(f.read().replace("self_role: sub", "self_role: main"))
    try:
        render_tree(ph7b, os.path.join(tmp, "sub_provision_badsub"), copy_runtime_block=False,
                    signing_key=kinfo["private"], sub_manifest_path=bad_sub); bad_caught = False
    except SystemExit as e:
        bad_caught = "self_role" in str(e)
    c7b = std_ok and sig_ok and forge_caught and sub_copied and hw_from_sub and bad_caught
    print(f"  [{'PASS' if c7b else 'FAIL'}] Agent_Card v2: 표준필드만={std_ok} 서명검증={sig_ok} 위조감지={forge_caught} "
          f"서브manifest배달={sub_copied} HW출처=서브manifest({hw_from_sub}) self_role≠sub거부={bad_caught} "
          f"(위반={v7b[:2]})")
    ok &= c7b

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
    ap.add_argument("--signing-key", default=None,
                    help="메인 Ed25519 PEM(기본 output/<topology>/a2a_signing/main_ed25519.pem). 부재 시 fail-loud — "
                         "`agent_card_contract.py keygen --out-dir output/<topology>/a2a_signing` 로 설치 때 1회 생성(사람 개입 0).")
    ap.add_argument("--sub-manifest", default=None,
                    help="서브 manifest(terraforming 실측 · scan_node.py --emit-sub-manifest 산출). 기본 "
                         "output/<topology>/sub_manifest.yaml. single 은 필수(서브도 manifest 를 갖는다 · §7.3), multi 는 선택.")
    ap.add_argument("--self-test", action="store_true", help="fixture 렌더 회귀(하드웨어/실 manifest 불요)")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()

    manifest = args.manifest or os.path.join(REPO, "output", args.topology, "manifest.yaml")
    out_dir = args.out or os.path.join(REPO, "output", args.topology, "sub_provision")
    if not os.path.isfile(manifest):
        print(f"[render] FAIL: manifest 없음 — {manifest} (terraforming_node 스캔/인터뷰로 먼저 채우세요)", file=sys.stderr)
        return 3
    signing_key = args.signing_key or os.path.join(REPO, "output", args.topology, "a2a_signing", "main_ed25519.pem")
    if not os.path.isfile(signing_key):
        print(f"[render] FAIL: Agent_Card 서명키 없음 — {signing_key}\n"
              f"        설치 때 1회: python3 .claude/skills/terraforming_node/scripts/agent_card_contract.py keygen "
              f"--out-dir output/{args.topology}/a2a_signing   (패스프레이즈 없음 · 0600 · 비추적 — 이후 사람 개입 0)", file=sys.stderr)
        return 2
    sub_manifest = args.sub_manifest or os.path.join(REPO, "output", args.topology, "sub_manifest.yaml")
    if not os.path.isfile(sub_manifest):
        if args.topology == "single":
            print(f"[render] FAIL: 서브 manifest 파일이 아직 생성되지 않았다 — {sub_manifest}\n"
                  f"        single 의 서브는 자기 manifest 를 갖는다(terraforming 실측·생성): "
                  f"python3 .claude/skills/terraforming_node/scripts/scan_node.py --topology single --peer-ssh <user@sub> "
                  f"--model-source <managed|ephemeral|custom> --emit-sub-manifest {sub_manifest}", file=sys.stderr)
            return 2
        sub_manifest = None   # multi(ray-worker): 서브 manifest 는 이 계획 범위 밖(브랜치싱크 때 재검토)
    data = parse_manifest(manifest)
    data.setdefault("topology", args.topology)   # D12: manifest 에 topology 없으면 --topology 로 채움(브랜치 맥락 보장)
    ph, missing = build_placeholders(data, sub_manifest=parse_manifest(sub_manifest) if sub_manifest else None)
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
    res = render_tree(ph, out_dir, copy_runtime_block=not args.no_runtime_block, tracked_list=tracked,
                      signing_key=signing_key, sub_manifest_path=sub_manifest)
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
