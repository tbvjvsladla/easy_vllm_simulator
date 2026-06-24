#!/usr/bin/env python3
"""render_sub_env.py — terraforming_subnode 의 **결정론 렌더러** (판단 X, 치환만).

역할(plan_2026062408_1 G1·D10=render-on-main): 메인에서 `output/<topology>/manifest.yaml` 의
노드정체성(nodes[]·interconnect·hw)을 PII-free 템플릿 `{{ ... }}` 에 치환해 **서브노드 에이전트 환경**을
gitignored 스테이징 트리 `output/<topology>/sub_provision/` 로 렌더한다(서브 워크스페이스 루트 미러).

산출 스테이징 레이아웃(= 서브 워크스페이스에 오버레이될 7-아티팩트):
  CLAUDE.md                              ← CLAUDE.template.md         (렌더)
  Agent_Card.json                        ← Agent_Card.template.json   (렌더)
  .claude/settings.local.json            ← settings.local.template.json (렌더, 스코프드)
  .claude/rules/comms.md                 ← comms.md                   (복제·정적계약)
  .claude/schemas/task-report.schema.json← task-report.schema.json    (복제·정적계약)
  .claude/skills/vllm-recipe-explorer/   ← 런타임블럭(git-tracked만 복제 — config.yaml/feedback/lockset 제외)
  tasks/.gitkeep                         ← 런타임 상태 스캐폴드(빈 디렉토리)

전달은 sync_to_sub.sh --provision(별도). 이 스크립트는 렌더까지만(결정론).
stdlib 만. PII(실 manifest)는 읽되 gitignored 스테이징으로만 쓴다(추적물엔 안 씀).
종료코드: 0=성공, 2=필수 manifest 필드 누락, 3=입력/IO 오류.
"""
from __future__ import annotations
import argparse
import json
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.dirname(HERE)                       # .claude/skills/terraforming_subnode
SUBNODE_DIR = os.path.join(SKILL_DIR, "sub_node")       # 템플릿·정적자산 보관
REPO = os.path.abspath(os.path.join(SKILL_DIR, "..", "..", ".."))  # repo root
RUNTIME_BLOCK = os.path.join(REPO, ".claude", "skills", "vllm-recipe-explorer")

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
        "INTERCONNECT": ic.get("type", ""),
        "INTERCONNECT_IFACE": ic.get("socket_iface", ""),
        # ── 폴백 있는 항목(이식성) ──
        "INTERCONNECT_MTU": ic.get("mtu") or "9000",
        "GID_INDEX": ic.get("gid_index") or "null",
        "HCA_DEVICES": ic.get("hca_devices") or "[]",
        "PLATFORM_PRESET": ic.get("platform_preset") or "",
        "RAY_PORT": data.get("ray_port") or "6379",
        "GPU_MODEL": data.get("gpu_model") or (f"{gpus}x-{cpu_arch}" if gpus and cpu_arch else cpu_arch or "unknown-gpu"),
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

    # 2) 복제 정적계약 2종
    shutil.copyfile(os.path.join(SUBNODE_DIR, "comms.md"), os.path.join(claude, "rules", "comms.md"))
    schema_dst = os.path.join(claude, "schemas", "task-report.schema.json")
    shutil.copyfile(os.path.join(SUBNODE_DIR, "task-report.schema.json"), schema_dst)
    with open(schema_dst, encoding="utf-8") as f:
        json.load(f)
    produced += [".claude/rules/comms.md", ".claude/schemas/task-report.schema.json"]

    # 3) 런타임블럭 복제(git-tracked 만 — config.yaml/feedback/lockset/__pycache__ 제외)
    if copy_runtime_block:
        dst_skill = os.path.join(claude, "skills", "vllm-recipe-explorer")
        n = _copy_tracked(RUNTIME_BLOCK, dst_skill)
        produced.append(f".claude/skills/vllm-recipe-explorer/ ({n} tracked files)")

    # 4) tasks/ 스캐폴드(빈 디렉토리 — git keep)
    with open(os.path.join(out_dir, "tasks", ".gitkeep"), "w") as f:
        f.write("")
    produced.append("tasks/.gitkeep")

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
        and ph["GPU_MODEL"] == "TEST-GPU" and ph["NAS_MOUNT"] == "/srv/test-models"
    print(f"  [{'PASS' if c1 else 'FAIL'}] manifest 파싱 + placeholders (missing={missing})")
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

    # (4) 전체 렌더 → 미치환 0 + JSON 유효 + 7 아티팩트 구조
    out = os.path.join(tmp, "sub_provision")
    try:
        res = render_tree(ph, out, copy_runtime_block=False)  # 런타임블럭 복제는 git 의존 → self-test 제외
        expect = ["CLAUDE.md", "Agent_Card.json", ".claude/settings.local.json",
                  ".claude/rules/comms.md", ".claude/schemas/task-report.schema.json", "tasks/.gitkeep"]
        have = all(os.path.exists(os.path.join(out, p)) for p in expect)
        # 렌더 산출물에 미치환 placeholder 0
        leftover = []
        for p in ("CLAUDE.md", "Agent_Card.json", ".claude/settings.local.json"):
            with open(os.path.join(out, p), encoding="utf-8") as f:
                leftover += _unrendered(f.read())
        # JSON 유효
        for p in ("Agent_Card.json", ".claude/settings.local.json", ".claude/schemas/task-report.schema.json"):
            with open(os.path.join(out, p), encoding="utf-8") as f:
                json.load(f)
        c4 = have and not leftover
        print(f"  [{'PASS' if c4 else 'FAIL'}] 전체 렌더(미치환={leftover}, 구조완비={have})")
        ok &= c4
    except SystemExit as e:
        print(f"  [FAIL] 렌더 예외: {e}")
        ok = False

    # (템플릿 PII-free 는 scripts/smoke_clone.sh A4 가 추적물 전반에서 단일 게이트로 검사 — 여기 중복/리터럴 미보유)
    shutil.rmtree(tmp, ignore_errors=True)
    print(f"self-test: {'PASS' if ok else 'FAIL'}")
    return 0 if ok else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="terraforming_subnode 서브 에이전트 환경 렌더러 (결정론)")
    ap.add_argument("--topology", choices=["single", "multi"], default="multi",
                    help="산출물 통로(output/<topology>/). 서브 에이전트 환경은 multi 전용.")
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
        print(f"[render] FAIL: manifest 없음 — {manifest} (terraforming_subnode 스캔/인터뷰로 먼저 채우세요)", file=sys.stderr)
        return 3
    data = parse_manifest(manifest)
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
