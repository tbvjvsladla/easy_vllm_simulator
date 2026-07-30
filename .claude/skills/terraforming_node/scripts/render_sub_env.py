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
RUNTIME_BLOCKS = [
    os.path.join(REPO, ".claude", "skills", "vllm-recipe-explorer"),
    os.path.join(REPO, ".claude", "skills", "adversarial-benchmark"),
]
DOCS_RULES = os.path.join(REPO, ".claude", "rules", "docs.md")     # 문서규약(정적계약 — 서브 테라포밍, D12)
DOC_SKELETONS = os.path.join(SKILL_DIR, "templates", "document_skeletons")  # 배포 포함 docs/*/example.md 정본(D12)
RECIPE_REFERENCE = os.path.join(REPO, ".claude", "skills", "wiki-desk", "reference", "references.md")

PLACEHOLDER_RE = re.compile(r"\{\{\s*([A-Z_]+)\s*\}\}")
# 템플릿 전용 머리말(렌더 산출물에서 제거) — md 템플릿의 "이건 템플릿이다" 메타 블록.
TEMPLATE_HEADER_RE = re.compile(r"<!--\s*TEMPLATE-ONLY:START\s*-->.*?<!--\s*TEMPLATE-ONLY:END\s*-->\s*", re.DOTALL)


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
    # 필수(누락 시 fail-loud — 무증거/빈 정체성 렌더 금지)
    required = ["SUB_HOST", "MASTER_HOST", "SSH_USER", "WORKSPACE_PATH", "NAS_MOUNT",
               "CPU_ARCH", "INTERCONNECT", "INTERCONNECT_IFACE"]
    missing = [k for k in required if not ph.get(k)]
    return ph, missing


def render_text(text: str, ph: dict) -> str:
    return PLACEHOLDER_RE.sub(lambda m: str(ph.get(m.group(1), m.group(0))), text)


def _unrendered(text: str) -> list[str]:
    return sorted(set(PLACEHOLDER_RE.findall(text)))


# ── 렌더 한 판 (스테이징 트리 산출) ──
def render_tree(ph: dict, out_dir: str, copy_runtime_block: bool = True) -> dict:
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
    schema_dst = os.path.join(claude, "schemas", "task-report.schema.json")
    shutil.copyfile(os.path.join(SUBNODE_DIR, "task-report.schema.json"), schema_dst)
    with open(schema_dst, encoding="utf-8") as f:
        json.load(f)
    produced += [".claude/rules/comms.md", ".claude/schemas/task-report.schema.json"]
    # D12: 문서규약 테라포밍 — 서브가 동일 발행규약(docs.md)으로 insight 문서 발행 → 상향 문서기반 회수
    if os.path.isfile(DOCS_RULES):
        shutil.copyfile(DOCS_RULES, os.path.join(claude, "rules", "docs.md"))
        produced.append(".claude/rules/docs.md")

    # recipe.py resolves GPU/source-verification facts from this on-demand dependency.  Copy only
    # the dependency, not the main-only wiki-desk capability, so sub runtime closure stays minimal.
    if not os.path.isfile(RECIPE_REFERENCE):
        raise SystemExit(f"[render] FAIL: recipe reference dependency missing: {RECIPE_REFERENCE}")
    recipe_ref_rel = ".claude/skills/wiki-desk/reference/references.md"
    recipe_ref_dst = os.path.join(out_dir, recipe_ref_rel)
    os.makedirs(os.path.dirname(recipe_ref_dst), exist_ok=True)
    shutil.copyfile(RECIPE_REFERENCE, recipe_ref_dst)
    produced.append(recipe_ref_rel)

    # 3) 런타임블럭 복제(git-tracked 만 — config.yaml/feedback/lockset/__pycache__ 제외)
    if copy_runtime_block:
        for rb in RUNTIME_BLOCKS:
            name = os.path.basename(rb)
            dst_skill = os.path.join(claude, "skills", name)
            n = _copy_tracked(rb, dst_skill)
            produced.append(f".claude/skills/{name}/ ({n} tracked files)")

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


def _copy_tracked(src_skill: str, dst: str) -> int:
    """git-tracked 파일만 복제(런타임블럭 = vllm-recipe-explorer). 사적/생성물 자동 제외."""
    rel = os.path.relpath(src_skill, REPO)
    try:
        out = subprocess.run(["git", "-C", REPO, "ls-files", rel],
                             capture_output=True, text=True, timeout=20)
        files = [ln for ln in out.stdout.splitlines() if ln.strip()]
    except Exception:
        files = []
    if not files:
        # 폴백: tracked 목록 못 얻으면 알려진 비추적만 제외하고 복제
        files = []
        for root, _, fns in os.walk(src_skill):
            if "__pycache__" in root:
                continue
            for fn in fns:
                if fn in ("config.yaml", "lockset.json", "candidates.json", "parsed.json"):
                    continue
                full = os.path.join(root, fn)
                if os.sep + "feedback" + os.sep in full:
                    continue
                files.append(os.path.relpath(full, REPO))
    cnt = 0
    for rf in files:
        srcf = os.path.join(REPO, rf)
        sub_rel = os.path.relpath(srcf, src_skill)
        destf = os.path.join(dst, sub_rel)
        os.makedirs(os.path.dirname(destf), exist_ok=True)
        shutil.copyfile(srcf, destf)
        cnt += 1
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
                       ".claude/rules/comms.md", ".claude/schemas/task-report.schema.json", "tasks/.gitkeep",
                       ".claude/rules/docs.md", ".claude/skills/wiki-desk/reference/references.md", ".gitignore",
                       # 호스트 안전체계: canonical terraforming source → constitution runtime delivery
                       ".claude/runtime/host_safety/mem_watchdog.sh",
                       ".claude/runtime/host_safety/install_host_safety.sh",
                       ".claude/runtime/host_safety/install_netconsole.sh",
                       ".claude/runtime/host_safety/systemd/easy-vllm-memwatch.service",
                       ".claude/runtime/host_safety/host/vllm-drop-caches.sh"]
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
        for p in ("Agent_Card.json", ".claude/settings.local.json", ".claude/schemas/task-report.schema.json"):
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
        c5 = ph5["TOPOLOGY"] == "single" and "single" in claude_single and not _unrendered(claude_single)
        print(f"  [{'PASS' if c5 else 'FAIL'}] dual-topology single 렌더(TOPOLOGY={ph5['TOPOLOGY']}, 페르소나 반영={'single' in claude_single})")
        ok &= c5
    except SystemExit as e:
        print(f"  [FAIL] single 렌더 예외: {e}")
        ok = False

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
        return 2
    res = render_tree(ph, out_dir, copy_runtime_block=not args.no_runtime_block)
    print(f"[render] OK → {res['out_dir']}")
    for p in res["produced"]:
        print(f"   + {p}")
    print(f"[render] (다음: bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh --apply --provision — HITL)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
