#!/usr/bin/env python3
"""recipe.py — vllm-recipe-explorer 오케스트레이터 CLI.

CONTRACT(FROZEN): 호스트 python3 단독 실행. whichllm 패키지 import 금지.
scripts/ 의 importable 함수를 조립한다(parse / auto_candidates / rank / render_report
/ generate / append). 결정/게이트/랭킹은 scripts 의 결정론 함수가 책임지며, 이 파일은
입력 로드 · tp 자동결정 · estimate→generate id 연결(.last_ranking.json)만 담당한다.

서브커맨드:
  estimate  : parse → 후보(--candidates json 또는 --auto) → rank → report 출력 →
              ranked 결과를 feedback/.last_ranking.json 저장(generate 가 id 로 참조).
  generate  : .last_ranking.json 에서 --recipe-id rN 찾기 → gen_recipe_set →
              feedback_log.append(추정 필드). config.yaml 의 serving.{...} 사용.

CLI 예:
  python3 recipe.py estimate --config config.yaml --auto
  python3 recipe.py generate --config config.yaml --recipe-id r3
"""

import argparse
import copy
import json
import os
import re
import sys

import yaml

# --- scripts/ 를 import 경로에 추가 후 함수 import 조립 (CONTRACT) ---
SKILL_ROOT = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_DIR = os.path.join(SKILL_ROOT, "scripts")
sys.path.insert(0, SCRIPTS_DIR)

from parse_model_config import parse  # noqa: E402
from rank_recipes import auto_candidates, rank, render_report  # noqa: E402
from gen_recipe_set import generate, recipe_from_candidate  # noqa: E402
from feedback_log import append as feedback_append  # noqa: E402

# Phase 2 시뮬레이터(통합 trial-loop) 조립용 import.
from run_trial import run_trial, PROVENANCE_VALUES, PROVENANCE_MEASURED  # noqa: E402
from sim_classify import classify as sim_classify  # noqa: E402
from preload_ram_gate import gate as preload_ram_gate  # noqa: E402
from estimate_vram import (  # noqa: E402
    GIB,
    KV_BLOCK_ALIGN_BUFFER,
    required_kv_bytes,
    max_safe_kv_bytes,
)
import simlog_writer  # noqa: E402

# --- 영속 경로 (SKILL_ROOT 기준 동적 도출 — 이식 가능, 하드코딩 금지) ---
def _repo_root(skill_root):
    """git toplevel 로 repo 루트 해소(스테이징 하위서도 실 repo 루트로 escape — run_bench.sh 와 동형, 레이아웃 비의존).
    실패(비-git) 시 abspath 3-up 폴백. WARN-3(adversarial-verify): 스테이징 사본 실행 시 abspath 가 스테이징 dir 를
    repo_root 로 오인 → 그 안의 위임 키로 메인 게이트 우회 가능했음. git toplevel 은 그 경계를 넘어 실 repo 로 해소."""
    try:
        import subprocess
        out = subprocess.run(["git", "-C", skill_root, "rev-parse", "--show-toplevel"],
                             capture_output=True, text=True)
        if out.returncode == 0 and out.stdout.strip():
            return out.stdout.strip()
    except Exception:
        pass
    return os.path.abspath(os.path.join(skill_root, os.pardir, os.pardir, os.pardir))


REPO_ROOT = _repo_root(SKILL_ROOT)
FEEDBACK_DIR = os.path.join(SKILL_ROOT, "feedback")
LAST_RANKING_PATH = os.path.join(FEEDBACK_DIR, ".last_ranking.json")
FEEDBACK_JSONL_PATH = os.path.join(FEEDBACK_DIR, "recipe_feedback.jsonl")

# --- 기본값 (config.example.yaml 스키마) ---
DEFAULT_NAS_HOST_ROOT = "/mnt/models"
DEFAULT_SAFETY_MARGIN = 0.90
DEFAULT_KV_BYTES = 2

# --- Phase 2 시뮬레이터 기본값 ---
DEFAULT_TRIAL_CAP = 3          # reconciliation_cap (workflow S3 와 동일 기본 3)
DEFAULT_TRIAL_IMAGE = "vllm-src-022:clean"


def _die(msg, code=1):
    """비0 종료 + 명확한 중단·보고 메시지(stderr)."""
    print(f"[recipe] 중단: {msg}", file=sys.stderr)
    sys.exit(code)


# docker compose 프로젝트명 규칙: [a-z0-9][a-z0-9_-]* — **점(.) 불가**.
# compose 는 COMPOSE_PROJECT_NAME 을 config_name 에서 만들기 때문에, 점이 든 이름으로 3종 세트를
# 생성하면 렌더는 성공하고 **기동에서만** 터진다:
#   invalid project name "vllm_glm-4.7-flash-e2e_project": must consist only of lowercase
#   alphanumeric characters, hyphens, and underscores as well as start with a letter or number
# 2026-07-31 GLM-4.7-Flash 에서 실제 발생 — 모델명에 점이 흔하므로(4.7 · 3.1 · 2.1) 재발한다.
# 생성 시점에 fail-loud 하는 것이 기동 시점에 터지는 것보다 낫다.
_COMPOSE_NAME_RE = re.compile(r"^[a-z0-9][a-z0-9_-]*$")


def _validate_config_name(name):
    if not _COMPOSE_NAME_RE.match(name or ""):
        _die("config.serving.config_name '%s' 은 docker compose 프로젝트명 규칙 위반 "
             "([a-z0-9][a-z0-9_-]* · 점/대문자/공백 불가). 모델명의 점을 빼라(예: glm-4.7 → glm-47). "
             "지금 막지 않으면 3종 세트는 생성되고 compose up 에서만 터진다." % name)


def load_config(config_path):
    """config.yaml 로드. 부재 시 명확히 중단."""
    if not os.path.isfile(config_path):
        _die(f"config 파일 없음: {config_path}")
    with open(config_path, "r", encoding="utf-8") as f:
        cfg = yaml.safe_load(f) or {}
    if not isinstance(cfg, dict):
        _die(f"config 형식 오류(매핑 아님): {config_path}")
    return cfg


def resolve_tp(cfg, repo_root):
    """tp 결정 (헌법 §manifest→서빙전략 배선 불변식 · roofline.py:196-209 패턴).

    config.tensor_parallel_size(명시 override) > manifest > 최종폴백 1.
    **git 브랜치 폴백 없음** — 브랜치⇒TP 추론이 보고된 버그였음(1-GPU 머신이 multi-node 브랜치면 TP=2).
    manifest = HW사실 단일 권위.

    manifest 파생: **topology=single → gpus_per_node(노드 배수 1 고정)** — single 의 nodes[role=sub]
    는 sub-control 피어이지 텐서 워커가 아니다. 이 배수를 안 걷으면 서브가 등록된 single manifest 가
    1-GPU 노드에 TP=2 를 요구한다(δ1-1 라이브 E2E 실버그 — 아래 `_target_cards` 는 이미 이 계약을
    쓰는데 serve TP 를 정하는 여기가 누락돼 있었다). 그 외 nodes 있으면 len(nodes)×gpus_per_node.
    """
    explicit = cfg.get("tensor_parallel_size")
    if explicit is not None:
        return int(explicit)
    man, _mpath, topo = _read_manifest(repo_root)
    gpus = _manifest_gpus(man, _mpath)
    mtopo = man.get("topology") or topo
    if (mtopo or "").startswith("single"):
        return gpus
    nodes = man.get("nodes") or []
    if nodes:
        return max(1, len(nodes)) * gpus
    return 1  # 최종폴백(안전·과대구독 ✗; multi 인데 nodes 비면 manifest 미완 신호)


def _git_branch(repo_root):
    """현재 git 브랜치명(실패 시 "")."""
    try:
        import subprocess

        out = subprocess.run(
            ["git", "-C", repo_root, "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
        )
        return out.stdout.strip() if out.returncode == 0 else ""
    except Exception:
        return ""


def _read_manifest(repo_root):
    """output/<topology>/manifest.yaml 로드 → (manifest_dict, path, topology). 부재/오류 시 ({}, path, topo).

    topology = 현재 git 브랜치가 선택하는 *통로*(multi-node→multi, 그 외→single). recipe 는 서브 복제
    런타임블럭이라 main-only `manifest_contract.py` 를 import 하지 않고 자체 리더로 동일 계약을 읽는다.
    """
    topo = "multi" if _git_branch(repo_root) == "multi-node" else "single"
    mpath = os.path.join(repo_root, "output", topo, "manifest.yaml")
    if not os.path.isfile(mpath):
        return {}, mpath, topo
    try:
        with open(mpath, "r", encoding="utf-8") as f:
            return (yaml.safe_load(f) or {}), mpath, topo
    except Exception as e:  # 손상 manifest 를 "부재"로 읽으면 하류가 기본값으로 계속 간다 — fail-loud(audit_26090515 Group D(b))
        _die("manifest 파싱 실패(%s): %s: %s — 손상된 HW 사실은 부재가 아니다. 파일을 고치거나 terraforming_node 로 재생성하세요."
             % (mpath, type(e).__name__, e), code=5)


def _manifest_gpus(man, mpath):
    """manifest 의 `gpus_per_node` — **HW 사실이지 기본값이 아니다**(2026-09-05 · audit_26090515 B4).

    부재·비정수·1 미만 → fail-loud. 종전의 `gpus_per_node`→1 침묵 폴백은 메인에서는 상위 게이트가 먼저
    죽어 닿지 않았지만 **서브(위임키 면제 경로)에서는 살아 있던 침묵 폴백**이었다 — 같은 줄이 노드에 따라
    등급이 달랐다. 값이 없으면 TP 를 1 로 *가정*하지 않고 멈춘다(헌법: manifest = HW 사실 단일 권위).
    """
    raw = man.get("gpus_per_node")
    try:
        gpus = int(raw)
    except (TypeError, ValueError):
        gpus = None
    if gpus is None or gpus < 1:
        _die("manifest(%s) 의 gpus_per_node 가 없거나 유효하지 않다(%r) — TP 는 HW 사실이지 기본값이 아니다. "
             "terraforming_node 스캔으로 채우세요(fail-loud · 침묵 폴백 ✗ · audit_26090515 B4)." % (mpath, raw), code=5)
    return gpus


def _total_gpus(man, mpath="output/<topology>/manifest.yaml"):
    """가용 GPU 합 = gpus_per_node × 노드 수.
    **topology=single → 노드 배수 1**(sub 는 control 피어이지 텐서 워커가 아님 — resolve_tp 와 동일 계약)."""
    gpus = _manifest_gpus(man, mpath)
    if (man.get("topology") or "").startswith("single"):
        return gpus
    nodes = man.get("nodes") or []
    return max(1, len(nodes)) * gpus if nodes else gpus


# ===========================================================================
# 타겟-GPU 이식형 예산 (host≠target — plan_26070809_47_07 §4). config-time 의도(manifest 와 직교).
# target_gpu 미정의 시 아래 함수들은 전부 무영향(기존 host 흐름 완전 보존 — 회귀 0).
# ===========================================================================

REFERENCES_MD_PATH = os.path.join(
    REPO_ROOT, ".claude", "skills", "wiki-desk", "reference", "references.md"
)


def _resolve_device_total_gib(cfg):
    """디바이스 메모리 total(GiB)과 그 **출처**. 코드에 하드웨어 숫자를 적지 않는다(G-B5).

    ① config 가 `test_device_total_gib` 를 선언했으면 그것(사람이 잰 값이 가장 강하다).
    ② 아니면 `/proc/meminfo MemTotal` 실측 — 통합메모리 노드(GB10 계열)에서는 디바이스 풀이
       곧 호스트 RAM 풀이라 이 값이 그대로 디바이스 total 이다.
    ③ 둘 다 없으면 **추측하지 않고 죽는다**. 이 값은 overhead 유도(=예산·gmu 판정)의 입력이라
       틀리면 조용히 틀린 상한을 만든다.
    """
    declared = cfg.get("test_device_total_gib")
    if declared:
        return float(declared), "declared(config test_device_total_gib)"
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemTotal:"):
                    return (int(line.split()[1]) / (1024.0 * 1024.0),
                            "measured(/proc/meminfo MemTotal · 통합메모리 가정)")
    except (OSError, IndexError, ValueError):
        pass
    raise SystemExit(
        "[recipe] FAIL: 디바이스 메모리 total 을 알 수 없다 — 상수를 쓰지 않는다(2026-09-05 · G-B5).\n"
        "  → config 에 `test_device_total_gib: <실측 GiB>` 를 선언하거나, 통합메모리 노드에서 "
        "/proc/meminfo 를 읽을 수 있게 하라.")


def _is_unified_memory(device_total_gib, tolerance=0.05):
    """디바이스 메모리 풀 == 호스트 RAM 풀인가(GB10 통합메모리) — **파생** 판정. 결정론.

    왜 필요한가(plan_26082223 결함 B): 트라이얼 협역 워치독은 `/proc/meminfo MemAvailable` 을
    보고 컨테이너를 죽인다. 통합메모리에서는 엔진 할당이 그 값을 직접 끌어내리므로 gmu 상한이
    곧 호스트 바닥 준수가 되지만, discrete GPU 에서는 두 풀이 무관하다 — 거기서 호스트 RAM 을
    근거로 gmu 를 깎으면 **근거 없는 축소**다(작은 RAM + 큰 VRAM 조합에서 실제로 오작동한다).

    manifest 에 새 필드를 만들지 않는다 — 이미 있는 두 사실(`test_device_total_gib` 와
    `/proc/meminfo MemTotal`)에서 파생되므로, 손으로 적으면 §4종 안티패턴의 "파생 가능한데
    손으로 적은 것"이 된다. 판정 불가(meminfo 못 읽음/디바이스 total 미상)면 None 이며,
    소비자는 None 을 "캡 걸지 않음"으로 처리한다(fail-closed 방향 = 현행 동작 유지).
    """
    if not device_total_gib:
        return None
    try:
        with open("/proc/meminfo", encoding="utf-8") as fh:
            for line in fh:
                if line.startswith("MemTotal:"):
                    mem_total_gib = int(line.split()[1]) / (1024.0 * 1024.0)
                    break
            else:
                return None
    except (OSError, IndexError, ValueError):
        return None
    if mem_total_gib <= 0:
        return None
    return abs(float(device_total_gib) - mem_total_gib) / mem_total_gib <= tolerance


def _lookup_gpu_spec(gpu_model, references_path=None):
    """references.md §4 HW-스코프에서 gpu_model 매칭 섹션의 per-card VRAM(GiB)·통합메모리 여부를 역룩업.

    관례: `### <아무 텍스트>{gpu_model}<아무 텍스트>` 헤더 섹션 안에 `per-card VRAM (GiB): <num>` 라인이
    있으면 그 값을 쓴다(값 채우기는 α 웹취득 소관 — 여기는 소비 배선만). 헤더에 '통합메모리' 포함 시 unified.
    미등재/파일부재 → (None, None)(quick-win 우회: config.target_gpu.per_card_vram_gib 명시 유도).
    """
    path = references_path or REFERENCES_MD_PATH
    if not gpu_model or not os.path.isfile(path):
        return None, None
    try:
        with open(path, "r", encoding="utf-8") as f:
            text = f.read()
    except OSError:
        return None, None
    import re as _re
    key = _re.escape(gpu_model.strip())
    m = _re.search(r"^###\s.*" + key + r".*$", text, _re.IGNORECASE | _re.MULTILINE)
    if not m:
        return None, None
    start = m.end()
    nxt = _re.search(r"^#{1,3}\s", text[start:], _re.MULTILINE)
    section = text[start: start + nxt.start()] if nxt else text[start:]
    is_unified = "통합메모리" in m.group(0)
    vm = _re.search(r"per-card VRAM\s*\(GiB\)\s*:\s*([\d.]+)", section)
    per_card = float(vm.group(1)) if vm else None
    return per_card, is_unified


def resolve_target_gpu_budget(cfg, tp):
    """target_gpu 블록(config-time 의도, §4.1) → (per_card_vram_gib, target_gmu, gpu_model).

    산정식(§4.2): kv_clamp_perGPU = per_card_VRAM×target_gmu − weights_total/TP − overhead_total/TP.
    이 함수는 (budget_gib, deploy_gmu) 만 해소한다 — `/TP` 분할은 `_resolve_clamp_kv(tp_divisor=tp)` 가 적용.
    ★ 반환 둘째 값은 **배포 gmu**(deploy_gmu = safe_gmu 의 정본)이지 예산 검증 게이트 승수가 아니다
      (2026-09-14 · plan_26091407 §4.3 · Q2·Q9). 게이트 승수는 config.safety_margin(gate_margin)이고
      둘을 가르는 자리는 호출부 `_gpu_roles` 다. 종전 호출부는 이 값을 `margin` 한 변수로 받아
      게이트 천장·클램프 산식·배포 yaml gmu 를 한꺼번에 움직였다(F4 — 셀 config 10개가 전부 (0.90, 0.85) 쌍).
    통합메모리 타겟은 target_gmu>0.90 을 0.90 으로 하드클램프+경고(§4.4). discrete 는 in-scope·하드클램프 없음.
    target_gmu 미선언은 **fail-loud** 다 — 종전 기본 0.90 은 선언처럼 보이는 상수였고, 배포 gmu 에는
    실측 통로가 없으므로 헌법 순서(선언 > 실측 > fail-loud)상 남는 것은 멈추는 것뿐이다.
    """
    tgt = cfg.get("target_gpu") or {}
    gpu_model = tgt.get("gpu_model")
    if not gpu_model:
        _die("config.target_gpu.gpu_model 누락(타겟 GPU 이식 경로엔 필수 — plan_26070809_47_07 §4.1)")
    if tgt.get("target_gmu") is None:
        _die("config.target_gpu.target_gmu 누락 — 배포 gmu(safe_gmu)의 정본이며 기본값을 두지 않는다"
             "(2026-09-14 · plan_26091407 §4.3). 타겟에서 돌 gpu-memory-utilization 을 선언하라"
             "(통합메모리 타겟은 ≤ 0.90). 예산 검증 게이트 승수는 별개 칸 config.safety_margin 이다.",
             code=5)
    target_gmu = float(tgt["target_gmu"])
    per_card_vram_gib = tgt.get("per_card_vram_gib")
    looked_up, is_unified = _lookup_gpu_spec(gpu_model)
    if per_card_vram_gib is None:
        per_card_vram_gib = looked_up
    if per_card_vram_gib is None:
        _die(
            "target_gpu.gpu_model=%r 의 per-card VRAM 을 references.md §4 에서 찾지 못함 — "
            "config.target_gpu.per_card_vram_gib 를 명시하세요(quick-win 우회, plan_26070809_47_07 §6)." % gpu_model
        )
    per_card_vram_gib = float(per_card_vram_gib)
    if is_unified and target_gmu > 0.90:
        print(
            "[recipe] 경고: 통합메모리 타겟(gpu_model=%s)은 target_gmu ≤ 0.90 하드클램프 — %.2f → 0.90 하향"
            "(HITL 재확인 요망)." % (gpu_model, target_gmu),
            file=sys.stderr,
        )
        target_gmu = 0.90
    return per_card_vram_gib, target_gmu, gpu_model


def _target_tp(cfg, repo_root):
    """타겟 TP = target_gpu.cards_per_node × node_count(§4.3).
    node_count 는 **multi 토폴로지에서만** manifest.nodes[] 길이(분산 TP 워커 수 — RoCE 로 텐서 분할).
    **single 토폴로지의 nodes[] 는 agent-plane 관리 피어(sub-control) 이지 TP 워커가 아니다**(각 노드
    완전 독립 서빙 — CLAUDE.md §single-node 확장기능 "노드 간 추론통신/텐서패브릭 없음") → node_count=1 고정.
    (δ 1-1 라이브 E2E 발견 — single 토폴로지에서 2노드가 TP=2 로 오카운트되는 실버그였음. 이 branch 는
    multi 토폴로지 전용이라 원래도 no-op 이나, 공유 빌딩블럭 정합을 위해 동일 수정 포팅.)
    """
    tgt = cfg.get("target_gpu") or {}
    cards_per_node = int(tgt.get("cards_per_node", 1))
    man, _, _ = _read_manifest(repo_root)
    if (man.get("topology") or "").startswith("single"):
        return cards_per_node
    nodes = man.get("nodes") or []
    node_count = max(1, len(nodes))
    return cards_per_node * node_count


# host 흐름 simulate: 선언 예산(vram_budget_gb)과 노드 per_card_vram 이 "같은 값" 으로 읽히는 허용오차.
#   국소 상수(이 판정에서만 쓴다). GB 표기와 GiB 실측의 반올림(예: 120 vs 121.69 = 1.4%)은 통과시키고
#   carve-out(24 vs 121.69)은 가른다.
_HOST_BUDGET_MATCH_TOLERANCE = 0.05


def _host_target_gpu(cfg, repo_root):
    """host 흐름(target_gpu 미정의) → **이 노드의 사실로** target_gpu 블록을 채운다(host == target).

    2026-09-14(plan_26091407 §4.3): 두 흐름이 같은 해소 경로(`resolve_target_gpu_budget`)를 타게 한다.
    종전 host 흐름은 `vram_budget_gb × safety_margin` 을 KV 천장으로, safety_margin 을 배포 gmu 로 썼고
    타겟 흐름은 per_card_vram × target_gmu 를 썼다 — 같은 물음(배포 gmu 와 그 예산)에 답이 둘이었다.
    이 함수는 **관측 HW 사실 평면**(manifest)을 읽는다. config-time 의도 평면인
    `resolve_target_gpu_budget` 은 여전히 그 평면을 읽지 않는다(KV_ABSOLUTE_CLAMP_PORTABILITY C4 분리 유지).

    칸별 해소(선언 > 실측·참조 > fail-loud — 추측하지 않는다):
      · gpu_model      = manifest `gpu_model`
      · per_card_vram  = 통합메모리(references.md 헤더)면 `_resolve_device_total_gib`(선언 test_device_total_gib
                         > /proc/meminfo) · 그 외는 선언 test_device_total_gib > references.md §4 역룩업
      · target_gmu     = config.safety_margin **명시값 승계** — host 흐름에는 target_gmu 칸이 없다.
                         승계 사실은 호출부가 lockset `gmu_source` 와 stderr 에 표시한다(침묵 폴백 ✗).
                         명시값이 없으면 멈춘다(기본값 0.90 을 배포 gmu 로 쓰지 않는다)
      · cards_per_node = manifest `gpus_per_node`
    반환: (block, sources) — sources 는 칸마다 값의 출처 문장이다(§결정론 규율 출처 표시).
    """
    man, mpath, _topo = _read_manifest(repo_root)
    gpu_model = str(man.get("gpu_model") or "").strip().strip('"')
    if not gpu_model:
        _die("host 흐름(target_gpu 미정의)인데 manifest(%s) 에 gpu_model 이 없다 — 이 노드 GPU 의 per-card "
             "VRAM·통합메모리 여부를 정할 근거가 없다(추측 ✗). terraforming_node 스캔으로 채우거나 "
             "config.target_gpu 블록을 선언하라(plan_26091407 §4.3)." % mpath, code=5)
    looked_up, is_unified = _lookup_gpu_spec(gpu_model)
    declared_total = cfg.get("test_device_total_gib")
    if is_unified:
        per_card, per_card_src = _resolve_device_total_gib(cfg)
        per_card_src = "통합메모리(references.md 헤더) → " + per_card_src
    elif declared_total:
        per_card, per_card_src = float(declared_total), "declared(config test_device_total_gib)"
    elif looked_up is not None:
        per_card, per_card_src = float(looked_up), "references.md §4 역룩업(gpu_model=%s)" % gpu_model
    else:
        _die("host 흐름: gpu_model=%r 의 per-card VRAM 을 정할 수 없다 — references.md §4 미등재 · 통합메모리 "
             "표지 없음 · config.test_device_total_gib 미선언. 셋 중 하나를 채워라(호스트 RAM 을 VRAM 으로 "
             "가정하지 않는다)." % gpu_model, code=5)
    if cfg.get("safety_margin") is None:
        _die("host 흐름: 배포 gmu 를 정할 선언이 없다 — config.target_gpu.target_gmu(타겟 흐름)도 "
             "config.safety_margin 명시값(host 흐름 승계)도 없다. 기본값 0.90 을 배포 gmu 로 쓰지 않는다"
             "(2026-09-14 · plan_26091407 §4.3).", code=5)
    block = {"gpu_model": gpu_model, "per_card_vram_gib": float(per_card),
             "target_gmu": float(cfg["safety_margin"]),
             "cards_per_node": _manifest_gpus(man, mpath)}
    sources = {"gpu_model": "manifest.gpu_model",
               "per_card_vram_gib": per_card_src,
               "target_gmu": ("config.safety_margin 승계(host 흐름 — target_gpu.target_gmu 칸이 없다 · "
                              "gate_margin 과 같은 값을 배포에 쓴다)"),
               "cards_per_node": "manifest.gpus_per_node"}
    return block, sources


def _gpu_roles(cfg, tp, repo_root):
    """gmu 의 두 역할을 **가른다**(2026-09-14 · plan_26091407 §4.3 · 사용자 결정 Q2·Q9).

      · gate_margin = config.safety_margin — 예산 검증 게이트 승수(`sim_classify` 천장 = budget × gate_margin ·
                      되먹임 `safety_margin_threshold`).
      · deploy_gmu  = target_gpu.target_gmu — 배포 yaml `gpu-memory-utilization` 이자 클램프 산식
                      kv_clamp = per_card_vram × deploy_gmu − weights/TP − overhead/TP 의 승수.
                      신설 필드가 아니다 — safe_gmu 는 target_gmu 그 자체다.

    종전 `margin` 변수 하나가 네 역할(클램프 산식·검증 게이트·배포 yaml·되먹임 임계)을 겸했고, 타겟 흐름은
    `resolve_target_gpu_budget` 이 target_gmu 를 그 자리에 반환해 **게이트 승수까지 target_gmu 로** 바뀌었다.
    host 흐름(target_gpu 미정의)은 `_host_target_gpu` 가 블록을 채워 같은 해소 경로를 탄다.

    tp_divisor: 타겟 흐름은 종전대로 TP(host 측정 불변량을 타겟 TP 로 이식) · host 흐름은 1(host == target 이라
    이식이 없다 — 종전 host 흐름 동작 유지). 반환 dict 의 `*_source` 는 값 옆의 출처다.
    """
    gate_declared = cfg.get("safety_margin") is not None
    gate_margin = float(cfg["safety_margin"]) if gate_declared else DEFAULT_SAFETY_MARGIN
    roles = {
        "gate_margin": gate_margin,
        "gate_margin_source": ("config.safety_margin" if gate_declared
                               else "DEFAULT_SAFETY_MARGIN(config.safety_margin 미선언 — Phase-1 과 같은 기본값)"),
        "gate_margin_role": "예산 검증 게이트 승수(sim_classify 천장 = budget × gate_margin)",
        "deploy_gmu_role": "배포 yaml gpu-memory-utilization · 클램프 산식 per_card_vram × deploy_gmu − weights/TP − overhead/TP",
    }
    if cfg.get("target_gpu"):
        ttp = _target_tp(cfg, repo_root)
        if ttp != tp:
            _die(
                "측정 TP(%d) ≠ 타겟 TP(%d, cards_per_node×node_count) — 1→N 외삽 금지(plan_26070809_47_07 §4.3). "
                "config.tensor_parallel_size 를 타겟에 맞추거나 manifest nodes[]/target_gpu.cards_per_node 를 "
                "정합시키세요." % (tp, ttp),
                code=6,
            )
        tgt = cfg.get("target_gpu") or {}
        declared_gmu = tgt.get("target_gmu")
        budget, deploy_gmu, gpu_model = resolve_target_gpu_budget(cfg, tp)
        roles.update({
            "flow": "target", "tp_divisor": tp,
            "budget_source": ("config target_gpu.per_card_vram_gib" if tgt.get("per_card_vram_gib") is not None
                              else "references.md §4 역룩업(gpu_model=%s)" % gpu_model),
            "deploy_gmu_source": "config target_gpu.target_gmu",
            # lockset 어휘(소유 = campaign_template_validator.LOCKSET_KNOB_SOURCES) — 배포값이 정본 칸에서 왔다.
            "gmu_source": "target_gmu",
        })
    else:
        block, sources = _host_target_gpu(cfg, repo_root)
        declared_gmu = block["target_gmu"]
        budget, deploy_gmu, gpu_model = resolve_target_gpu_budget({"target_gpu": block}, tp)
        roles.update({
            "flow": "host", "tp_divisor": 1,
            "budget_source": "host 흐름 · " + sources["per_card_vram_gib"],
            "deploy_gmu_source": sources["target_gmu"],
            # 정본 칸(target_gpu.target_gmu)이 아닌 선언에서 왔다 → `hand`(사람이 적은 다른 칸). 승계 사실은
            # deploy_gmu_source 가 문장으로 말한다 — `target_gmu` 로 적으면 정본 칸을 읽은 척이 된다.
            "gmu_source": "hand",
            "host_target_gpu": block,
        })
        # 선언 예산(vram_budget_gb) ↔ 이 노드 per_card_vram — 둘이 갈라지면 **멈춘다**(리뷰 교정 2026-09-14).
        #   종전 host 흐름은 vram_budget_gb 를 예산으로 썼다(carve-out 24 GiB 가 정당한 선언이었다). per_card 실측이
        #   그 선언을 조용히 덮으면 선언 > 실측 순서가 뒤집히고, 24 GiB 로 선언한 config 가 121 GiB 클램프를 rc 0 으로
        #   낸다. 반대로 선언을 이기게 두면 plan §4.3 의 "host 흐름 per_card = 이 노드 사실" 이 죽은 코드가 된다.
        #   두 권위가 어긋난 자리에서 어느 쪽을 고를지 추측하지 않는다 — 정합하면(허용오차 안) 그대로 가고,
        #   어긋나면 carve-out 은 target_gpu 블록으로 선언하게 한다(타겟 흐름이 그 선언을 정본으로 읽는다).
        _vb = cfg.get("vram_budget_gb")
        if _vb is not None and budget:
            _gap = abs(float(_vb) - float(budget)) / float(budget)
            if _gap > _HOST_BUDGET_MATCH_TOLERANCE:
                _die("host 흐름 simulate: config.vram_budget_gb=%s 가 이 노드 per_card_vram=%.2f GiB(%s)와 %.1f%% "
                     "다르다(허용 %.0f%%) — 선언 예산과 노드 사실 중 어느 쪽을 클램프 천장·검증 게이트에 쓸지 추측하지 "
                     "않는다. carve-out(가상 예산)이면 target_gpu 블록(gpu_model·per_card_vram_gib·target_gmu)으로 "
                     "선언하고, 이 노드 전체가 예산이면 vram_budget_gb 를 per_card_vram 에 맞춰라(plan_26091407 §4.3)."
                     % (_vb, float(budget), sources["per_card_vram_gib"], 100 * _gap,
                        100 * _HOST_BUDGET_MATCH_TOLERANCE), code=5)
            roles["budget_source"] += " · config.vram_budget_gb=%s 와 정합(차 %.1f%%)" % (_vb, 100 * _gap)
    roles.update({"budget_gib": float(budget), "gpu_model": gpu_model, "deploy_gmu": float(deploy_gmu),
                  "deploy_gmu_declared": float(declared_gmu)})
    if abs(float(declared_gmu) - float(deploy_gmu)) > 1e-9:
        roles["deploy_gmu_source"] += " · 통합메모리 하드클램프 %.2f→%.2f" % (float(declared_gmu), float(deploy_gmu))
    return roles


# ── lockset 출처 어휘 — **소유자는 campaign_template_validator** (2026-09-14 · plan_26091407 §4.2·§4.3) ──
#   explorer 가 lockset 에 `provenance`·`*_source` 를 기계 각인한다. 어휘 목록은 여기 두지 않는다 — 측정 진입
#   precheck·합격 술어 P6 가 읽는 바로 그 상수를 읽고, 각인하는 값 하나하나를 그 목록으로 교차검증한다
#   (_SWEEP_TO_CELL 선례: 두 자리에 적으면 한쪽이 조용히 늦는다). 싱글 서브에도 같은 경로로 배달된다
#   (render_sub_env CAMPAIGN_TOOLS). 없으면 각인하지 않고 멈춘다 — 사본으로 메우면 그것이 거울이다.
_LOCKSET_VOCAB_REL = os.path.join(".claude", "skills", "terraforming_node", "scripts",
                                  "campaign_template_validator.py")
_LOCKSET_VOCAB = None


def _lockset_vocab():
    """(LOCKSET_PROVENANCE, LOCKSET_KNOB_SOURCES) 를 소유자 모듈에서 읽는다."""
    global _LOCKSET_VOCAB
    if _LOCKSET_VOCAB is None:
        path = os.path.join(REPO_ROOT, _LOCKSET_VOCAB_REL)
        if not os.path.isfile(path):
            _die("lockset 출처 어휘의 소유자가 없다: %s — explorer 는 provenance·*_source 를 각인하므로 "
                 "어휘 없이 진행하지 않는다(사본을 두지 않는다 · 메인이면 저장소 손상, 서브면 재배달)."
                 % _LOCKSET_VOCAB_REL, code=5)
        import importlib.util
        spec = importlib.util.spec_from_file_location("_recipe_lockset_vocab", path)
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        _LOCKSET_VOCAB = (tuple(mod.LOCKSET_PROVENANCE),
                          {k: tuple(v) for k, v in mod.LOCKSET_KNOB_SOURCES.items()})
    return _LOCKSET_VOCAB


def _is_campaign_cell_lockset(path):
    """`campaigns/<camp-id>/cells/<cell>/lockset.json`(이 저장소 · 뼈대 `_template` 제외)인가 — 경로 모양만 본다."""
    if not path:
        return False
    try:
        rel = os.path.relpath(os.path.realpath(path), os.path.realpath(REPO_ROOT))
    except ValueError:
        return False
    parts = rel.split(os.sep)
    return (len(parts) == 5 and parts[0] == "campaigns" and parts[1] != "_template"
            and parts[2] == "cells" and parts[4] == "lockset.json")


def _stamp(key, value):
    """explorer 가 **각인하는** 출처 값 하나를 소유자 어휘로 교차검증해 돌려준다. None 은 '아직 정하지 않았다'.

    어휘 밖이면 멈춘다 — 여기 들어오는 값은 explorer 자신이 고른 것이라 어휘 밖은 내부 결함이다.
    입력 lockset 이 들고 온 값은 이 함수가 아니라 `_input_source` 가 읽는다(기재 항목을 게이트로 올리지 않는다).
    """
    if value is None:
        return None
    prov, knobs = _lockset_vocab()
    allowed = prov if key == "provenance" else knobs.get(key)
    if allowed is None or value not in allowed:
        _die("lockset 각인 값이 어휘 밖이다: %s=%r (허용 %s · 소유 %s) — explorer 내부 결함이다."
             % (key, value, allowed, _LOCKSET_VOCAB_REL), code=5)
    return value


def _input_source(key, value):
    """입력 lockset 이 들고 온 `*_source` 를 읽는다. 반환 (어휘 안의 값|None, 이탈 사유|None).

    어휘 밖 값은 **멈추지 않고** 사유로 돌려준다 — 검증기 P6 도 그것을 ⓘ 줄로 기재할 뿐 막지 않는다.
    호출부는 이탈을 "사람이 적은 출처 미상 값" 으로 다루고(덮어쓰지 않는다) 그 사실을 기록에 싣는다.
    """
    if value is None:
        return None, None
    _prov, knobs = _lockset_vocab()
    allowed = knobs.get(key) or ()
    if value in allowed:
        return value, None
    return None, ("입력 lockset %s=%r 는 어휘 밖이다(허용 %s · 소유 %s) — 출처 미상의 사람 값으로 본다"
                  % (key, value, allowed, _LOCKSET_VOCAB_REL))


def _reject_target_gpu_phase1(cfg, cmd_name):
    """Phase-1 라우팅 게이트(§4.1·§7 합격기준1): target_gpu 정의 시 gmu-only 종료 거부 →
    Phase-2(simulate) 측정경로 강제. target_gpu 미정의 시 무변경(기존 host 흐름 회귀 0)."""
    if cfg.get("target_gpu"):
        _die(
            "config.target_gpu 정의됨 — Phase-1(%s)은 gmu-only 라 절대 KV 클램프를 emit 하지 않는다"
            "(헌법 §KV 절대클램프 따름정리 · plan_26070809_47_07 §2). "
            "`recipe.py simulate --config <config> --candidate <lockset.json>` 로 "
            "Phase-2 측정경로를 사용하세요(타겟 예산은 config.target_gpu 가 그대로 소비됨)." % cmd_name,
            code=7,
        )


def _card_contract_path(repo_root):
    """정체성 검증기의 위치. 메인은 스킬 아래, 서브는 물질화된 런타임 아래에 있다."""
    for rel in (os.path.join(".claude", "runtime", "a2a", "agent_card_contract.py"),
                os.path.join(".claude", "skills", "terraforming_node", "scripts",
                             "agent_card_contract.py")):
        p = os.path.join(repo_root, rel)
        if os.path.isfile(p):
            return p
    return None


def _self_role(repo_root):
    """이 워크스페이스가 스스로 선언한 역할(manifest `self_role`). 없으면 None(= 메인 정본).

    hostname·브랜치로 추론하지 않는다(헌법). 이 값이 `sub` 면 **정체성 증명이 필수**다 —
    카드를 지우면 검사를 건너뛰는 형태가 되면 안 되기 때문이다(부재를 면제로 만들지 않는다).
    """
    man, _mpath, _topo = _read_manifest(repo_root)
    if not man:
        return None
    role = man.get("self_role")
    return str(role).strip().strip('"') if role else None


def _require_identity_proof(repo_root):
    """이 워크스페이스가 **메인이 프로비저닝한 노드**임을 증명한다(2026-09-05 · G-E1).

    위임 키(`.claude/a2a_delegation.json`)를 대체한다. 종전 규약은 메인이 발급한 **실행 허가**가
    없으면 서브를 info-only 로 묶었고, 감사는 그것을 R3(에이전트 자율성 부정)로 판정했다.
    서브는 이제 자기 manifest(메인이 발급한 Flag)와 **서명된 Agent Card** 를 갖는다 — 필요한 것은
    허가가 아니라 정체성이다.

    판정 규칙: `Agent_Card.json` 이 있으면(= 프로비저닝된 노드) 서명이 **반드시 검증돼야 한다**.
    카드가 없으면 메인 자신이므로 이 검사는 성립하지 않는다(no-op). 손상·위조·신뢰저장소 부재는
    거부다 — 옛 '손상 키 fail-closed' 규율을 그대로 옮긴 자리다.
    반환: 증명 요지(dict) 또는 None(카드 없음 = 메인).
    """
    card = os.path.join(repo_root, "Agent_Card.json")
    required = _self_role(repo_root) == "sub"
    if not os.path.isfile(card):
        if required:
            _die("이 노드는 manifest 가 `self_role: sub` 라고 선언하는데 **Agent_Card.json 이 없다** — "
                 "정체성 증명 부재는 면제가 아니다(fail-closed). 메인의 재배달로 해소한다.", code=6)
        return None
    verifier = _card_contract_path(repo_root)
    if verifier is None:
        _die("Agent_Card.json 은 있는데 **검증기가 없다**(.claude/runtime/a2a/agent_card_contract.py) — "
             "서명을 확인할 수 없으므로 진행하지 않는다(fail-closed). 메인의 재배달이 필요하다.", code=6)
    import subprocess          # 이 모듈은 subprocess 를 지연 import 한다(서브 런타임블럭 관행)
    proc = subprocess.run([sys.executable, verifier, "prove-identity", "--repo-root", repo_root],
                          capture_output=True, text=True)
    if proc.returncode != 0:
        _die("정체성 증명 실패 — 이 워크스페이스의 Agent_Card 서명이 검증되지 않는다(fail-closed).\n"
             "  %s\n  → 카드가 손상됐거나 메인이 발급한 것이 아니다. 메인의 재배달로 해소한다."
             % (proc.stderr or proc.stdout).strip()[:400], code=6)
    try:
        return json.loads(proc.stdout.strip() or "{}")
    except ValueError:
        return {}

def _require_terraform_flag(repo_root):
    """헌법 §테라포밍-완수/A2A-위임 Flag 게이트 (**fail-closed**) — 면제 없으면 info-only(작업 거부·비0종료). 강제 2층의 *결정론 백스톱*.

    2026-09-05(③ 3-9 · G-E1): **위임 키 면제 경로를 삭제**했다. 종전에는 메인이 발급한 실행 허가
    (`.claude/a2a_delegation.json`)나 `EASY_VLLM_A2A_DELEGATED=1` 이 Flag 검사를 면제했고, 그것이
    "서브는 허가 없이는 아무것도 못 한다" 는 R3 구조였다. 서브는 이제 ②에서 **자기 manifest**
    (메인이 발급한 `terraforming.complete/branch_verified` · `self_role: sub`)를 받으므로 면제가
    필요 없다 — 정규 경로로 통과한다. 대신 프로비저닝된 노드는 **정체성 증명**(서명된 Agent Card)을
    통과해야 한다: 옛 '손상 키 fail-closed' 규율이 옮겨 간 자리다(`_require_identity_proof`).

    HW 필수필드(topology·gpus_per_node)와 model_source 는 종전처럼 검사한다(audit_26090515 E2).
    (recipe = 서브 복제 런타임블럭 → main-only `manifest_contract.py` 미import, 자체 리더로 동일 계약.)
    """
    identity = _require_identity_proof(repo_root)
    if identity:
        print("[recipe] 정체성 증명 OK — kid=%s role=%s topology=%s"
              % (identity.get("kid"), identity.get("role"), identity.get("topology")),
              file=sys.stderr)
    man, mpath, _topo = _read_manifest(repo_root)
    if not man:
        _die(
            "manifest 부재(%s) — 테라포밍 미완(fail-closed). HW 사실 없이 서빙전략 deliverable 생성 ✗.\n"
            "  · 메인이면: terraforming_node 로 HW스캔 + 모델획득 모드(managed|ephemeral|custom)를 먼저 정한다.\n"
            "  · 서브면: 서브 manifest 는 **메인의 terraforming 이 설치 과정에서 생성·배달**한다"
            "(2026-09-05 이후 — 위임 키로 면제하던 경로는 삭제됐다)." % mpath,
            code=4,
        )
    terra = man.get("terraforming") or {}
    if terra.get("complete") is not True or terra.get("branch_verified") is not True:
        _die(
            "테라포밍 완수 Flag 미발급(terraforming.complete/branch_verified != true) — "
            "terraforming_node 로 스캔·branch↔topology 3자일치 검증 완수 먼저(info-only). "
            "서브에서는 메인이 발급한 manifest 가 이 Flag 를 싣는다(issued_by: main).",
            code=4,
        )
    # manifest_contract.evaluate_contract 와 **동일 계약**(약한 게이트 금지 — 통합검증 BLOCK):
    # Flag 켜졌어도(또는 면제됐어도) 필수 HW사실·획득모드 없으면 거부(scan 은 model_source 없이 complete 를 emit → 게이트가 집행).
    missing = [k for k in ("topology", "gpus_per_node") if not man.get(k)]
    if missing:
        _die("필수 HW필드 누락(%s) — terraforming_node 스캔 완수 먼저(info-only)." % ", ".join(missing), code=5)
    _manifest_gpus(man, mpath)  # 존재해도 비정수/0 이면 여기서 fail-loud
    ms = man.get("model_source")
    if ms not in ("managed", "ephemeral", "custom"):
        _die("model_source 미설정/오류(%r) — terraforming_node 에서 획득모드(managed|ephemeral|custom) 지정 먼저(info-only)." % ms, code=5)


def _guard_tp(tp, repo_root):
    """가드(헌법 §manifest→서빙전략 배선 불변식): tp 가 가용 GPU 합 초과 시 비0종료(서빙 전 결정론 조기탐지)."""
    man, _mp, _ = _read_manifest(repo_root)
    tot = _total_gpus(man, _mp)
    if tp > tot:
        _die(
            "tp=%d 가 가용 GPU 합 %d 초과(GPU 초과) — config.tensor_parallel_size 또는 "
            "manifest(gpus_per_node·nodes) 확인." % (tp, tot),
            code=6,
        )


def _guard_kv_heads(parsed, tp):
    """가드: KV head 가 tp 에 분배 불가능하면 비0종료(KV-head 비분할 — vLLM serve 즉사 조기탐지).

    kvh >= tp: kvh 가 tp 로 나눠떨어져야 한다(각 rank 가 kvh/tp 개씩 보유).
    kvh <  tp: **vLLM 은 GQA/MQA 에서 kvh 를 tp 로 복제한다**(kvh=1 극단GQA — 예 gemma-4 계열 —
      포함) — tp 가 kvh 로 나눠떨어지면 유효(각 rank 가 kvh 의 복제본 하나씩 보유, replication
      factor=tp/kvh). 2026-08-20 실증: gemma-4-E2B-it(kvh=1) 이 실제 TP=2 Ray 서빙 PASS
      (testlog_26081408 §1·§6, X3 4/4 성립) — 이전 버전은 kvh%tp 단방향만 검사해 이 유효 조합을
      오탐 거부했다(vLLM 실동작과 불일치).
    """
    kvh = parsed.get("num_key_value_heads")
    try:
        kvh_i, tp_i = int(kvh), int(tp)
    except (TypeError, ValueError):
        return
    if not kvh_i or not tp_i:
        return
    divisible = (kvh_i % tp_i == 0) if kvh_i >= tp_i else (tp_i % kvh_i == 0)
    if not divisible:
        _die("num_key_value_heads=%s 가 tp=%d 와 상호 분배 불가(양방향 나눗셈 모두 비정수) — "
             "tp 조정 필요." % (kvh, tp), code=6)


def output_root(repo_root):
    """3종 세트 출력 루트 = output/<topology>/ (산출물 통로 self-containment, 결함#3 — testlog_26062422).

    topology = 브랜치 파생(multi-node→multi, 그 외→single). gen_recipe_set 이 그 하위 configs/·envs/ 에 생성 →
    docker compose 가 마운트하는 통로(output/<t>/configs)와 정합. (이전엔 REPO_ROOT 직하 configs/ 로 떨어져
    통로 밖이라 수동 복사 필요했음.)"""
    topology = "multi" if _git_branch(repo_root) == "multi-node" else "single"
    return os.path.join(repo_root, "output", topology)


def _default_jit_cache_root():
    """trial 컨테이너의 JIT/컴파일 캐시 호스트 통로 = output/<topology>/cache.

    serve 평면(docker-compose.yaml 의 `./cache/vllm`·`./cache/flashinfer`)과 **같은 디렉터리**다.
    두 평면이 캐시를 공유해야 trial 이 데운 것을 serve 가 쓰고 그 반대도 성립한다 —
    통로를 나누면 서로 cold JIT 을 반복하고, cold JIT 은 시간 문제가 아니라
    uncapped nvcc 팬아웃에 의한 **호스트 하드다운 리스크**다(compose 주석 · Laguna 선례).
    """
    return os.path.join(output_root(REPO_ROOT), "cache")


def _cfg_common(cfg, repo_root):
    """estimate/generate 공통 입력값 추출(스키마 결함은 즉시 중단).

    NAS 경로 해소(헌법 §manifest→서빙전략 배선 불변식 · recipe 호스트파싱 평면):
      config.nas_host_root > env(NAS_MODEL_PATH) > manifest.nas_model_path > DEFAULT(/mnt/models).
    """
    target = cfg.get("target_model") or {}
    model_path = target.get("path")
    if not model_path:
        _die("config.target_model.path 누락")
    _man, _mpath, _topo = _read_manifest(repo_root)
    nas_root = (
        cfg.get("nas_host_root")
        or os.environ.get("NAS_MODEL_PATH")
        or _man.get("nas_model_path")
        or DEFAULT_NAS_HOST_ROOT
    )
    budget = cfg.get("vram_budget_gb")
    if budget is None:
        _die("config.vram_budget_gb 누락")
    budget = float(budget)
    margin = float(cfg.get("safety_margin", DEFAULT_SAFETY_MARGIN))
    kv_bytes = int(cfg.get("kv_cache_dtype_bytes", DEFAULT_KV_BYTES))
    # 컨테이너 마운트 prefix(기본 /app/models). quant_model 2차 마운트면 /app/quant_models 로 override.
    container_root = cfg.get("nas_container_root", "/app/models")
    return model_path, nas_root, budget, margin, kv_bytes, container_root


def cmd_estimate(args):
    cfg = load_config(args.config)
    _reject_target_gpu_phase1(cfg, "estimate")
    model_path, nas_root, budget, margin, kv_bytes, container_root = _cfg_common(cfg, REPO_ROOT)
    tp = resolve_tp(cfg, REPO_ROOT)
    _guard_tp(tp, REPO_ROOT)

    # ① parse(결정론) — NAS 부재/디렉토리·config.json 부재 → traceback 금지,
    # config 오류와 동일한 [recipe] 중단: 클린 어보트로 통일(model_config_unparseable).
    try:
        parsed = parse(model_path, nas_host_root=nas_root, nas_container_root=container_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")
    _guard_kv_heads(parsed, tp)

    # 후보: --candidates json 또는 --auto(결정론 기본 그리드).
    if args.candidates:
        if not os.path.isfile(args.candidates):
            _die(f"--candidates 파일 없음: {args.candidates}")
        with open(args.candidates, "r", encoding="utf-8") as f:
            candidates = json.load(f)
        if not isinstance(candidates, list):
            _die("--candidates JSON 은 후보 리스트여야 함")
        # id 없는 LLM 후보엔 r1..rN 부여 (estimate→generate 의 rN 연결 보장).
        # rank_recipes._main 의 동일 규약과 정합.
        for idx, cand in enumerate(candidates, start=1):
            if isinstance(cand, dict):
                cand.setdefault("id", "r%d" % idx)
    elif args.auto:
        candidates = auto_candidates(parsed, tp)
    else:
        _die("후보 소스 미지정: --auto 또는 --candidates <file> 중 하나 필요")

    # ③ rank(결정론 하드게이트+Judge 랭킹).
    result = rank(parsed, candidates, tp, budget, margin, kv_bytes=kv_bytes)

    # 리포트 출력.
    report = render_report(result, parsed, budget, margin, tp)
    print(report)

    # ranked 결과를 .last_ranking.json 에 저장(generate 가 rN 으로 참조).
    os.makedirs(FEEDBACK_DIR, exist_ok=True)
    snapshot = {
        "model_id": parsed.get("model_id"),
        "model_path_container": parsed.get("model_path_container"),
        "tp": tp,
        "vram_budget_gb": budget,
        "safety_margin": margin,
        "kv_cache_dtype_bytes": kv_bytes,
        "result": result,
    }
    with open(LAST_RANKING_PATH, "w", encoding="utf-8") as f:
        json.dump(snapshot, f, ensure_ascii=False, indent=2)
    print(f"\n[recipe] 랭킹 저장: {LAST_RANKING_PATH}", file=sys.stderr)
    print(
        "[recipe] generate 로 선택: "
        f"python3 recipe.py generate --config {args.config} --recipe-id <rN>",
        file=sys.stderr,
    )
    print(
        "[recipe] ⚠ Phase-1(estimate→generate)은 max-num-seqs·절대 KV 클램프(kv-cache-memory-bytes)를 "
        "emit하지 않는다(공식 per-token이 sliding-window/GQA에서 부정확 — 결함#4). max-num-seqs 는 simulate(Phase-2)가 "
        "min(declared_axes.concurrency_requirement, KV_fit@typical_request_tokens) 로 엔진 보고 kv_cache_tokens 에서 "
        "산정한다(references/kv-clamp.md §3 · plan_26091407 §4.2).",
        file=sys.stderr,
    )


def _find_recipe(snapshot, recipe_id):
    """저장된 ranking 에서 rN 후보를 찾는다(ranked → failed → all 순)."""
    result = snapshot.get("result") or {}
    for bucket in ("ranked", "failed", "all"):
        for cand in result.get(bucket, []) or []:
            if cand.get("id") == recipe_id:
                return cand, bucket
    return None, None


def cmd_generate(args):
    cfg = load_config(args.config)
    _reject_target_gpu_phase1(cfg, "generate")

    if not os.path.isfile(LAST_RANKING_PATH):
        _die(
            f"이전 estimate 산출물 없음: {LAST_RANKING_PATH}. "
            "먼저 `recipe.py estimate` 를 실행하라."
        )
    with open(LAST_RANKING_PATH, "r", encoding="utf-8") as f:
        snapshot = json.load(f)

    recipe, bucket = _find_recipe(snapshot, args.recipe_id)
    if recipe is None:
        _die(f"recipe-id 미발견: {args.recipe_id} (in {LAST_RANKING_PATH})")
    if bucket != "ranked":
        # gate 탈락 후보를 강제 생성하려는 경우 경고(force 로 진행은 허용).
        print(
            f"[recipe] 경고: {args.recipe_id} 는 '{bucket}' 버킷(게이트 통과 아님).",
            file=sys.stderr,
        )

    serving = cfg.get("serving") or {}
    name = serving.get("config_name")
    port = serving.get("port")
    served_model_name = serving.get("served_model_name")
    if not name:
        _die("config.serving.config_name 누락")
    _validate_config_name(name)
    if port is None:
        _die("config.serving.port 누락")
    if not served_model_name:
        _die("config.serving.served_model_name 누락")
    port = int(port)

    # parse 결과(컨테이너 경로 등)는 generate 가 사용 → 다시 parse 하여 재현성 확보.
    model_path, nas_root, budget, margin, kv_bytes, container_root = _cfg_common(cfg, REPO_ROOT)
    tp = snapshot.get("tp", resolve_tp(cfg, REPO_ROOT))
    # parse 재실행 — NAS 부재/해소 실패 시 traceback 금지(클린 어보트로 통일).
    try:
        parsed = parse(model_path, nas_host_root=nas_root, nas_container_root=container_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")

    # 결정론 계층 경고(estimate 가 단 것) 노출 — 특히 offline-quant on non-prequantized.
    recipe_warning = recipe.get("warning")
    if recipe_warning == "offline_quant_on_non_prequantized_checkpoint":
        print(
            "[recipe] 경고: %s 는 비prequantized 체크포인트에 사전양자화 전용 quant(%s) — "
            "`vllm serve` 시 사전양자화 가중치 부재로 서빙 실패 가능(네트워크 무관). "
            "(fp8·bitsandbytes 는 온라인 양자화 가능.)"
            % (args.recipe_id, recipe.get("quantization")),
            file=sys.stderr,
        )

    # ④ generate(결정론 3종 세트). 기존 파일 충돌은 명확히 중단·보고(traceback 금지).
    try:
        paths = generate(
            parsed,
            recipe,
            name,
            output_root(REPO_ROOT),
            port,
            served_model_name,
            force=args.force,
            image=getattr(args, "image", None),
        )
    except FileExistsError as e:
        _die(f"{e} (덮어쓰려면 --force)")
    print("[recipe] 생성된 3종 세트:")
    for p in paths:
        print(f"  - {p}")

    # 되먹임 로그(추정 필드 채움; 실측·서빙결과는 nullable 예약).
    from datetime import datetime

    record = {
        "timestamp": datetime.now().isoformat(),
        "model_id": parsed.get("model_id") or snapshot.get("model_id"),
        "quantization": recipe.get("quantization"),
        "max_model_len": recipe.get("max_model_len"),
        "gpu_memory_utilization": recipe.get("gpu_memory_utilization"),
        "vram_budget_gb": snapshot.get("vram_budget_gb", budget),
        "estimated_vram_gb": recipe.get("estimated_total_gib"),
        "tensor_parallel_size": tp,
        "safety_margin_threshold": snapshot.get("safety_margin", margin),
        "selection_timestamp": datetime.now().isoformat(),
        "actual_vram_gb": None,
        "serve_success": None,
        # 결정론 계층 경고를 되먹임 로그에 기록(예약 nullable 필드 재사용 — 신규 키 미추가).
        "error_type": recipe_warning,
        "tokens_per_sec": None,
    }
    feedback_append(record, FEEDBACK_JSONL_PATH)
    print(f"[recipe] 되먹임 로그 기록: {FEEDBACK_JSONL_PATH}", file=sys.stderr)


# ===========================================================================
# Phase 2 — simulate (통합 trial-loop).
#   run_trial → sim_classify → 조정(kv_bytes 결정론 / soft 변수 후보 순서 폴백) → 반복.
#   weights/overhead 는 vllm_profile 에서 실측. kv_bytes = min(required_kv, max_safe_kv).
#   required > safe → infeasible HITL. 수렴 시 gen_recipe_set 3종 세트 + simlog + feedback.
#   cap 소진/infeasible/unknown → Model-C HITL 보고(증거 simlog 경로 제시).
#   결정/게이트는 scripts 의 결정론 함수가 책임진다(이 파일은 배선만).
# ===========================================================================


def _load_candidate(path):
    """--candidate <lockset.json> 로드(부재/형식오류 → 클린 어보트)."""
    if not os.path.isfile(path):
        _die(f"--candidate 파일 없음: {path}")
    with open(path, "r", encoding="utf-8") as f:
        raw = f.read()
    try:
        cand = json.loads(raw)
    except json.JSONDecodeError as e:
        # docstring 이 "클린 어보트"를 약속하는데 실제로는 raw JSONDecodeError 트레이스백이 났다.
        # 특히 simlog 산출물이 *.yaml 로 보존되므로 그걸 그대로 입력으로 되먹이기 쉽다 —
        # 가장 흔한 오입력에 대해 원인과 해소를 함께 준다(2026-08-01 실제 발생).
        hint = ""
        if path.endswith((".yaml", ".yml")) or raw.lstrip()[:1] not in ("{", "["):
            hint = (" — YAML 로 보인다. --candidate 는 **JSON** 만 받는다"
                    "(simlog 의 trial*_candidate.yaml 은 사람이 읽으라고 만든 *산출물*이지"
                    " 입력 형식이 아니다). 변환: python3 -c \"import yaml,json,sys;"
                    "json.dump(yaml.safe_load(open(sys.argv[1])),open(sys.argv[2],'w'),"
                    "ensure_ascii=False,indent=2)\" in.yaml out.json")
        _die(f"--candidate 파싱 실패: {path} ({e}){hint}")
    if not isinstance(cand, dict):
        _die("--candidate JSON 은 lock-set dict 여야 함")
    cand.setdefault("id", "s1")
    return cand


def _profile_bytes(profile, key):
    """vllm_profile 의 GiB 실측값(<key>)을 바이트(float)로. 없으면 None."""
    if not profile:
        return None
    v = profile.get(key)
    return float(v) * GIB if v is not None else None


# vLLM 이 받는 KV dtype 철자 중 **1바이트인 것 전부**. 종전에는 `== "fp8"` 하나만 봤고,
# 그래서 `fp8_e5m2`(지수 5·가수 2 — 여전히 1바이트)가 2바이트로 계산됐다. 그러면 required_kv 가
# 2배로 부풀어 max_safe 를 넘고, 셀이 **엔진에 닿기도 전에** `vram_infeasible` 로 죽는다.
# 그 판정은 거짓이다 — 실제 사유는 KV 용량이 아니라 커널의 dtype 수용 여부이며, 그것은 서빙을
# 해 봐야 안다. 거짓 사유가 지도에 실리면 다음 사람이 잘못된 결론을 상속한다.
# (직전 캠페인에서 드러나지 않은 이유: 이 경로는 **측정 프로파일이 없는 트라이얼 1**에서만
#  쓰이고, e5m2 셀은 그 전에 다른 이유로 죽었다.)
_KV_ONE_BYTE_DTYPES = frozenset({"fp8", "fp8_e4m3", "fp8_e5m2"})


def _kv_dtype_bytes(candidate):
    """KV dtype 바이트: fp8 계열이면 1, 아니면 2(fp16/bf16)."""
    q = str(candidate.get("kv_cache_quant") or "").strip().lower()
    return 1 if q in _KV_ONE_BYTE_DTYPES else 2


def _read_log_text(log_path):
    """trial.log_path 의 원시 로그 텍스트를 읽는다. 부재(dry-run 등)면 ""."""
    if not log_path or not os.path.isfile(log_path):
        return ""
    try:
        with open(log_path, "r", encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return ""


def _next_soft_value(candidate, target):
    """soft 변수(target)의 *_candidates 리스트에서 현재값 다음 후보를 고른다.

    candidate[target] 현재값을 후보 순서에서 찾아 그 다음 항목을 반환.
    더 없거나 후보 리스트가 비면 None(폴백 소진).
    """
    cands = candidate.get(target + "_candidates") or []
    if not cands:
        return None
    cur = candidate.get(target)
    if cur in cands:
        idx = cands.index(cur)
        if idx + 1 < len(cands):
            return cands[idx + 1]
        return None
    # 현재값이 후보 목록에 없으면 첫 후보부터 시도.
    return cands[0]


def _enrich_overhead(profile, device_total_gib, gmu_fallback=None, kv_was_explicit=False):
    """consolidated 메모리 라인이 없는 vLLM 빌드 보강: non_kv_overhead 를 유도한다.

    vLLM 메모리식: gmu × device_total = weights + non_kv_overhead + kv_available.
    → non_kv_overhead = gmu_trial × device_total − weights − kv_available
       (cuda_graph 는 잔차에 포함 = 보수적). profile 에 이미 non_kv_overhead_gib 가
       있으면(consolidated 라인 보유) 건드리지 않는다. profile 을 제자리 보강해 반환.

    gmu_trial 은 우리가 --gpu-memory-utilization 으로 **설정한 알려진 입력**이다. 로그에서
    파싱(parse_vllm_log)이 vLLM 버전별 로그 포맷 차로 못 잡으면(예: 0.18.0) gmu_fallback
    (=candidate.gpu_memory_utilization)로 대체한다 — 로그 파싱에 의존하지 않는다.

    ⚠ 이 잔차식은 **kv_available 이 gmu 풀을 다 채운(자연 프로파일링) 경우에만** 성립한다.
    `kv_cache_memory_bytes` 절대클램프가 걸린 트라이얼(kv_was_explicit=True)은 클램프가
    풀보다 훨씬 작게 KV를 강제하므로, 잔차 = 진짜 overhead + **의도적으로 남겨둔 미사용 풀**
    이 되어 overhead 가 수십 GiB 로 부풀려진다(2026-08-11 gemma-4-E2B-it 실측: gmu=0.4 트라이얼
    에서 kv=1.92GiB 클램프 시 overhead=36.96GiB 오산정 → vram_infeasible 오판). 그래서
    explicit 클램프 트라이얼은 이 식을 건너뛴다(non_kv_overhead_gib 는 None 유지 — 직전
    자연-프로파일 트라이얼의 실측값이 이미 correction_history 에 있으므로 재계산 불요).
    """
    if not isinstance(profile, dict):
        return profile
    if profile.get("non_kv_overhead_gib") is not None:
        return profile
    if kv_was_explicit:
        return profile
    gmu = profile.get("gmu_trial")
    gmu_from_log = gmu is not None
    if gmu is None:
        gmu = gmu_fallback  # 알려진 입력(우리가 설정한 gmu). 버전별 로그 포맷 비의존.
    w = profile.get("weights_gib")
    kv = profile.get("kv_cache_gib")
    if None in (gmu, w, kv) or not device_total_gib:
        return profile
    overhead = float(gmu) * float(device_total_gib) - w - kv
    if overhead > 0:
        profile["non_kv_overhead_gib"] = overhead
        profile["device_total_gib"] = float(device_total_gib)
        profile["gmu_used"] = float(gmu)
        profile["gmu_source"] = "log" if gmu_from_log else "known-input(candidate.gpu_memory_utilization)"
    return profile


# ===========================================================================
# max-num-seqs 산식 (2026-09-14 · plan_26091407 §4.2 · 사용자 결정 Q5)
#   max_num_seqs = min(concurrency_requirement, KV_fit@typical_request_tokens)
#   KV_fit@L     = floor(kv_fit_tokens ÷ L)   — kv_fit_tokens 는 엔진 보고 `GPU KV cache size` 토큰 실측에서 온다
#   입력은 셀 config.yaml `declared_axes`(선언 계층 · 단계 ① 규약)이고 산출은 lockset `batch`·`batch_source`.
#   2-위상: 첫 통과 트라이얼(보통 언클램프 측정)에서 batch 를 정하고 → 그 batch 로 클램프를 산정해 →
#   다음 트라이얼이 클램프+batch 고정으로 재검증한다(sim_classify 가 엔진 보고 토큰으로 KV-fit 을 다시 잰다).
#   batch 와 클램프가 서로를 입력으로 삼는 순환을 **위상을 나눠** 끊는다.
#   종전: batch 는 lockset 의 손값이었고(`recipe.py batch = candidate.get("batch") or 1`) parse_vllm_log 가
#   파싱한 kv_cache_tokens·max_concurrency 의 소비자는 0 이었다(F2). 무릎·열벽은 여기서 재지 않는다 —
#   adversarial-benchmark 스윕·노드 블랙박스가 재서 escalation_candidates / hint input 으로 넘긴다
#   (트리플렛 직접 쓰기 ✗). max-num-batched-tokens 모델링은 범위 밖이다(plan_26091407 §9 후속).
# ===========================================================================

# 어휘 **안에서** 무엇이 explorer 파생인가 — 분류는 explorer 소관이고 어휘 자체는 검증기 소유다.
#   두 목록이 갈라지면(검증기가 값을 개명) `_batch_plan` 이 첫 호출에서 멈춘다(사본이 조용히 늦지 않게).
_BATCH_DERIVED_SOURCES = ("declared-requirement", "kv-fit-measured")
_KV_DERIVED_SOURCES = ("measured-clamp",)


def _declared_axis(cfg, key, strict=True):
    """셀 config `declared_axes.<key>` 의 양의 정수 선언. 반환 (값|None, 상태, 출처 문장).

    상태: declared(수) · null(명시적으로 "선언하지 않았다" — 단계 ① 규약) · absent(블록·키 없음 — 캠페인 밖 config)
    · malformed(strict=False 일 때만).
    strict=True(산식 입력 — concurrency_requirement·typical_request_tokens): 그 외 모양(<<FILL>>·문자열·0 이하)은
      **선언 결함**이라 멈춘다 — 부재로 접으면 거짓 선언이 산식에서 사라진다.
    strict=False(기재 전용 — declared_axes.tp): 멈추지 않고 malformed 와 사유를 돌려준다. 기재 항목의 모양 결함이
      simulate 전체를 멈추면 기재가 게이트로 격상된다(리뷰 교정 2026-09-14).
    """
    axes = cfg.get("declared_axes")
    if axes is None:
        return None, "absent", "config.declared_axes 블록 없음"
    if not isinstance(axes, dict):
        if not strict:
            return None, "malformed", "config.declared_axes 가 매핑이 아니다(%s)" % type(axes).__name__
        _die("config.declared_axes 는 매핑이어야 한다(%r)" % type(axes).__name__, code=5)
    if key not in axes:
        return None, "absent", "config.declared_axes.%s 키 없음" % key
    val = axes[key]
    if val is None:
        return None, "null", "config.declared_axes.%s=null(선언하지 않았다)" % key
    if isinstance(val, bool) or not isinstance(val, int) or val < 1:
        if not strict:
            return None, "malformed", ("config.declared_axes.%s=%r 는 양의 정수 또는 null 이 아니다(기재 · 차단 ✗)"
                                       % (key, val))
        _die("config.declared_axes.%s=%r 는 양의 정수 또는 null 이어야 한다 — 모르면 null 로 '선언하지 "
             "않았다'를 적는다(campaigns/README.md §셀 값의 두 계층)." % (key, val), code=5)
    return val, "declared", "config.declared_axes.%s" % key


def _batch_plan(candidate, cfg):
    """입력 lockset 의 explorer 파생 칸(batch·KV 클램프)을 어떻게 다룰지 정한다. 반환 plan dict —
    **candidate 를 제자리 수정할 수 있다**(파생 칸을 비운다).

    batch:
      · mode=hand-lever : lockset 이 batch 를 들고 있고 batch_source 가 null·hand-lever·어휘 밖이다(사람이 적은 값).
                          explorer 는 **덮어쓰지 않는다**(Q1 표시+대조) — 같은 산식이 냈을 값은 대조용으로만 기재한다.
      · mode=derive     : 그 외. batch_source 가 직전 explorer 산출(declared-requirement·kv-fit-measured)이면
                          그 batch 는 **승계하지 않고** 비운 뒤 다시 잰다(carry-forward ✗ · prior_batch 로 기재).
    KV 클램프:
      · kv_source=measured-clamp(직전 explorer 산출)이면 비운다 — 그 클램프는 직전 batch 로 산정됐으므로 남기면
        이번 batch 가 옛 클램프에 묶이고, trial1 이 클램프 트라이얼이 되어 2-위상(언클램프 실측 → 클램프 재검증)이
        성립하지 않는다(prior_kv 로 기재). 그 외(hand·null·어휘 밖)는 사람이 적은 클램프라 유지한다(종전 동작).
    """
    _prov, knobs = _lockset_vocab()
    for _key, _derived in (("batch_source", _BATCH_DERIVED_SOURCES), ("kv_source", _KV_DERIVED_SOURCES)):
        if not set(_derived) <= set(knobs.get(_key) or ()):
            _die("explorer 파생 분류 %s=%s 가 소유자 어휘 %s 밖이다 — 검증기 어휘가 바뀌었는데 분류가 따라가지 "
                 "않았다(%s)." % (_key, _derived, knobs.get(_key), _LOCKSET_VOCAB_REL), code=5)
    lock_batch = candidate.get("batch")
    # 모양 검사(리뷰 교정 2026-09-14): '<<FILL>>' 같은 비수치가 int() traceback 으로 죽지 않게 안내와 함께 멈춘다.
    #   정수값 실수(8.0)는 종전 `int(batch)` 가 받던 모양이라 그대로 받는다(비회귀).
    if lock_batch is not None and (isinstance(lock_batch, bool) or not isinstance(lock_batch, (int, float))
                                   or lock_batch < 1 or int(lock_batch) != lock_batch):
        _die("lockset batch=%r 는 양의 정수 또는 null 이어야 한다 — 비워 두면(null) explorer 가 "
             "min(concurrency_requirement, KV_fit@L) 로 산출하고, 사람이 적은 수는 손레버로 보존된다"
             "(references/kv-clamp.md §3)." % (lock_batch,), code=5)
    lock_src, src_anomaly = _input_source("batch_source", candidate.get("batch_source"))
    lock_kv = candidate.get("kv_cache_memory_bytes")
    kv_src, kv_anomaly = _input_source("kv_source", candidate.get("kv_source"))
    req, req_state, req_src = _declared_axis(cfg, "concurrency_requirement")
    typ, typ_state, typ_src = _declared_axis(cfg, "typical_request_tokens")
    max_len = candidate.get("max_model_len")
    if typ is not None and max_len is not None and typ > int(max_len):
        _die("config.declared_axes.typical_request_tokens=%d > max_model_len=%s — 요청 한 건은 max_model_len 을 "
             "넘을 수 없다(선언 결함)." % (typ, max_len), code=5)
    plan = {"mode": "derive", "requirement": req, "requirement_state": req_state,
            "requirement_source": req_src, "typical": typ, "typical_state": typ_state,
            "typical_source": typ_src, "prior_batch": None, "lockset_batch_source": lock_src,
            "prior_kv": None, "lockset_kv_source": kv_src,
            "input_source_anomalies": [a for a in (src_anomaly, kv_anomaly) if a]}
    if lock_batch is not None and lock_src not in _BATCH_DERIVED_SOURCES:
        plan["mode"] = "hand-lever"
        plan["hand_batch"] = int(lock_batch)
    elif lock_batch is not None:
        plan["prior_batch"] = lock_batch
        candidate["batch"] = None
    if lock_kv is not None and kv_src in _KV_DERIVED_SOURCES:
        plan["prior_kv"] = lock_kv
        candidate["kv_cache_memory_bytes"] = None
    return plan


def _kv_fit_length(plan, candidate):
    """KV-fit 대표 길이 L(= 클램프 산식의 request_tokens). derive 모드이고 batch 가 정해졌을 때만 값이 있다."""
    if plan.get("mode") != "derive" or candidate.get("batch") is None:
        return None
    return int(plan["typical"]) if plan.get("typical") is not None else int(candidate["max_model_len"])


def _kv_fit_tokens(profile, budget, deploy_gmu, tp_divisor, clamp_bytes):
    """배포 KV 가 담을 토큰 수(kv_fit_tokens)와 출처. 엔진 보고 `GPU KV cache size` 토큰이 기점이다.

      · 배포 클램프 트라이얼(clamp_bytes 설정) — 엔진이 이미 배포 KV 로 떴다 → 보고 토큰 그대로.
      · 언클램프 측정 트라이얼 — 엔진 보고 토큰은 **측정 풀**(host 캡 gmu · host 예산)의 토큰이다. 배포 KV 천장
        max_safe = budget × deploy_gmu − weights/TP − overhead/TP 로 환산한다(max_safe ÷ (측정 per-token × 블록정렬
        버퍼)). host == target 이고 캡이 없으면 보고 토큰과 거의 같다 — 환산은 캡(host_floor_gmu_cap)·타겟 이식에서
        값을 바꾸고, 버퍼는 **다음 위상의 클램프가 실제로 담을 수 있는 토큰**으로 세기 위해 곱한다: 클램프 산식이
        required = per_token × L × batch × KV_BLOCK_ALIGN_BUFFER 이므로 버퍼 없이 센 KV-fit 을 batch 로 쓰면
        required 가 max_safe 를 버퍼만큼 넘어 vram_infeasible 이 된다(경계에서 산식이 자기 산출을 거부한다).
    반환 (tokens|None, source). None 이면 산출 불가 사유가 source 에 있다(부재를 0 으로 접지 않는다).
    ★ clamp_bytes 는 **배포될 클램프**일 때만 준다(입력 lockset 의 사람 클램프). 트라이얼 루프가 batch 를 정하기
      전에 잡은 **잠정 클램프**(첫 트라이얼 vram_oom → max_model_len 1건 기준 재산정)는 배포값이 아니므로 호출부가
      None 을 넘겨 환산 경로를 탄다 — 그 작은 클램프의 엔진 토큰을 KV-fit 으로 쓰면 요구가 거짓 사유로 깎인다
      (리뷰 교정 2026-09-14: 요구 16·배포 천장 KV-fit 83 인 구성이 batch 4 kv-fit-measured 로 각인됐다).
    """
    kv_tok = (profile or {}).get("kv_cache_tokens")
    kv_gib = (profile or {}).get("kv_cache_gib")
    if not kv_tok:
        return None, "엔진 보고 kv_cache_tokens 부재(로그 키 미관측 · mock 프로파일에 없음)"
    if clamp_bytes is not None:
        return int(kv_tok), "engine kv_cache_tokens=%d (배포 클램프 트라이얼 — 환산 없음)" % int(kv_tok)
    weights_b = _profile_bytes(profile, "weights_gib")
    overhead_b = _profile_bytes(profile, "non_kv_overhead_gib")
    if not kv_gib or weights_b is None or overhead_b is None:
        return None, "언클램프 트라이얼인데 kv_cache_gib·weights·overhead 실측 중 부재 — 배포 천장으로 환산 불가"
    if tp_divisor and tp_divisor > 1:
        weights_b, overhead_b = weights_b / tp_divisor, overhead_b / tp_divisor
    max_safe = max_safe_kv_bytes(budget, deploy_gmu, weights_b, overhead_b)
    per_token = float(kv_gib) * GIB / float(kv_tok)
    if max_safe <= 0:
        return 0, "배포 KV 천장 max_safe=%d ≤ 0 (weights+overhead 가 budget×deploy_gmu 를 채운다)" % max_safe
    tokens = int(max_safe // (per_token * KV_BLOCK_ALIGN_BUFFER))
    # 부동소수 경계: 클램프 산식과 **같은 식**(int(per_token × tokens × 버퍼))으로 되짚어 천장을 넘으면 한 칸 내린다.
    while tokens > 0 and int(per_token * tokens * KV_BLOCK_ALIGN_BUFFER) > max_safe:
        tokens -= 1
    return tokens, ("언클램프 측정 환산: 배포 KV 천장 %d B ÷ (per_token %.1f B × 블록정렬 버퍼 %.2f) — per_token = 측정 KV "
                    "%d B ÷ engine kv_cache_tokens %d"
                    % (max_safe, per_token, KV_BLOCK_ALIGN_BUFFER, int(float(kv_gib) * GIB), int(kv_tok)))


def derive_batch(plan, max_model_len, kv_fit_tokens, kv_fit_source):
    """max_num_seqs = min(concurrency_requirement, KV_fit@L). 순수 함수 — 반환은 lockset `batch_derivation` 의 몸.

    선언 상태별 동작(출처는 전부 기록된다):
      · 요구 선언 ∧ KV-fit 산출 → 요구 ≤ KV-fit: batch=요구 · declared-requirement
                                   요구 > KV-fit: batch=KV-fit · kv-fit-measured (낮춘 사유 기재 — README loop-until-done:
                                   context·KV dtype 를 재조정할지는 사람이 본다)
      · 요구 null/부재 → batch **미정**(max-num-seqs 미emit · vLLM 기본 admission 상한 · 클램프는 max_model_len 1건
                         기준 — 종전 동작). KV-fit 은 산출되면 **참고값으로만** 기재한다(`kv_fit` · batch 로 쓰지 않는다).
                         리뷰 교정(2026-09-14): 종전 초안은 여기서 batch=KV-fit@max_model_len(near-max)을 냈다 —
                         그 batch 의 required 는 배포 천장 전체라 선언 없는 config(캠페인 밖 기본 경로) 모두가 천장
                         클램프를 받았고, 통합메모리에서는 run_trial 의 호스트 바닥 캡이 클램프 트라이얼에 걸리지 않아
                         워치독 사살 구성이 된다. 또 엔진 max_concurrency(최악 길이)는 plan §4.2 가 **기재**로 둔 보수
                         하한이지 산출값이 아니다. 요구가 없으면 산식이 줄일 대상도 없다 — 선언이 먼저다.
      · KV-fit 산출 불가 → 요구 선언이면 batch=요구 · declared-requirement(**검증 안 됨** 기재) · 아니면 batch 미정
      · typical_request_tokens null/부재 → L = max_model_len(최악 길이 — 보수 산정 · 대체 사실 기재)
    """
    typ = plan.get("typical")
    length = int(typ) if typ is not None else (int(max_model_len) if max_model_len is not None else None)
    rec = {
        "formula": "max_num_seqs = min(concurrency_requirement, KV_fit@L) · KV_fit@L = floor(kv_fit_tokens ÷ L)",
        "mode": plan.get("mode"), "applied": plan.get("mode") == "derive",
        "concurrency_requirement": plan.get("requirement"),
        "concurrency_requirement_state": plan.get("requirement_state"),
        "concurrency_requirement_source": plan.get("requirement_source"),
        "kv_fit_length": length,
        "kv_fit_length_source": (plan.get("typical_source") if typ is not None else
                                 "max_model_len=%s 대체 — %s · 최악 길이로 보수 산정"
                                 % (max_model_len, plan.get("typical_source"))),
        "kv_fit_tokens": kv_fit_tokens, "kv_fit_tokens_source": kv_fit_source,
        "kv_fit": None, "batch": None, "batch_source": None, "verified": None, "reason": "",
        "prior_batch": plan.get("prior_batch"),
    }
    req = plan.get("requirement")
    if kv_fit_tokens is None or not length:
        if req is not None:
            rec.update(batch=int(req), batch_source="declared-requirement", verified=False,
                       reason="KV-fit 산출 불가(%s) — 선언 요구 %d 를 **검증 없이** 채택" % (kv_fit_source, req))
        else:
            rec["reason"] = ("KV-fit 산출 불가(%s) ∧ 동시성 요구 %s — batch 미정(max-num-seqs 미emit · vLLM 기본 "
                             "admission 상한)" % (kv_fit_source, plan.get("requirement_state")))
        return rec
    fit = int(kv_fit_tokens) // int(length)
    rec["kv_fit"] = fit
    if req is None:
        # 요구가 없으면 줄일 대상도 없다 — batch 를 만들지 않고 KV-fit 은 참고값으로만 남긴다(docstring 리뷰 교정).
        rec["reason"] = ("동시성 요구 %s → batch 를 산출하지 않는다(max-num-seqs 미emit · vLLM 기본 admission 상한 · "
                         "클램프는 max_model_len 1건 기준). KV-fit %d(@L=%d)은 참고값으로만 기재한다 — 요구가 있으면 "
                         "declared_axes.concurrency_requirement 로 선언하라" % (plan.get("requirement_state"), fit, length))
        return rec
    floor_note = "" if fit >= 1 else " · KV-fit 0(대표 요청 1건도 배포 KV 에 들지 않는다 — 1 로 두고 클램프 산정이 판정한다)"
    if req <= fit:
        rec.update(batch=int(req), batch_source="declared-requirement",
                   reason="요구 %d ≤ KV-fit %d(@L=%d) → 요구 채택" % (req, fit, length))
    else:
        rec.update(batch=max(fit, 1), batch_source="kv-fit-measured",
                   reason=("요구 %d > KV-fit %d(@L=%d) → KV-fit 으로 낮춤. 요구를 지키려면 context·KV dtype·예산을 "
                           "재조정해 다시 돈다(loop-until-done)%s" % (req, fit, length, floor_note)))
    return rec


def _resolve_clamp_kv(parsed, candidate, profile, budget, deploy_gmu, kv_dtype_bytes, tp_divisor=1,
                      request_tokens=None):
    """실측 profile(weights/overhead) 로 절대 KV 클램프 = min(required, max_safe) 산정.

    OOM-critical 경로 — 순수 결정론(확률론 금지). 반환 dict:
      kv  : 산정된 클램프 바이트(성공). weights/overhead 실측 부재면 None(+fail None).
      fail: 구조적 불가(vram_infeasible) 시 final_class dict, 아니면 None.
      note: 산정 근거 문자열.

    tp_divisor(기본 1 — 회귀 0): >1 이면 타겟-GPU 이식 경로(plan_26070809_47_07 §4.2) — host 측정
    weights_total/overhead_total(GPU-불변 기하량)을 타겟 TP 로 나눠 per-GPU 클램프를 산정한다.

    deploy_gmu(2026-09-14 · plan_26091407 §4.3 · Q9): 천장 승수는 **배포 gmu**(target_gpu.target_gmu)다 —
    max_safe = budget × deploy_gmu − weights/TP − overhead/TP. 예산 검증 게이트 승수(safety_margin)는 여기
    오지 않는다(sim_classify 소관). 종전 인자명 `margin` 이 두 역할을 겸했다(위치 인자는 그대로다).

    request_tokens(2026-09-14 · §4.2): batch 를 **KV-fit 산식**으로 정했을 때의 대표 요청 길이 L.
    주면 required 토큰 = max(max_model_len, L × batch) — batch 건의 대표 요청과 최악 길이 1건(vLLM 기동 검사)을
    함께 담는 최소량이다. 안 주면(손레버 batch · 구 lockset) 종전 식 max_model_len × batch 그대로다(회귀 0).
    KV-fit 으로 정한 batch 에 최악 길이 × batch 를 요구하면 대표 길이로 센 동시성을 최악 길이로 다시 세어 거의
    항상 vram_infeasible 이 된다 — max_num_seqs 는 admission 상한이고 KV 부족은 선점·대기라 합법이다(F3).
    """
    weights_b = _profile_bytes(profile, "weights_gib")
    overhead_b = _profile_bytes(profile, "non_kv_overhead_gib")
    if weights_b is None or overhead_b is None:
        return {"kv": None, "fail": None, "note": "weights/overhead 실측 부재."}
    if tp_divisor and tp_divisor > 1:
        weights_b = weights_b / tp_divisor
        overhead_b = overhead_b / tp_divisor
    max_len = int(candidate["max_model_len"])
    # candidate.get("batch", 1) 는 키가 *존재하지만 값이 JSON null*(자유변수 표기 — lockset.json 관례)
    # 인 경우를 못 잡는다(.get 의 default 는 키 부재에만 적용) → TypeError(2026-08-11 gemma-4-e2b-it 실측).
    batch = int(candidate.get("batch") or 1)
    req_tokens = max(max_len, int(request_tokens) * batch) if request_tokens else None
    # required_kv: 측정 트라이얼이 per-token KV(kv_available/kv_tokens)를 주면 그 측정값을 쓴다.
    # 하이브리드(GDN/linear-attention) 모델은 전 레이어가 full-KV 가 아니라 dims 공식이 과대추정한다
    # (Qwen3.6: 공식 262144 vs 실측 ~70600 B/token, 3.7×). 측정 per-token 이 정확. 없으면 공식 폴백.
    kv_gib_m = profile.get("kv_cache_gib")
    kv_tok_m = profile.get("kv_cache_tokens")
    if kv_gib_m and kv_tok_m:
        per_token = (float(kv_gib_m) * (1024 ** 3)) / float(kv_tok_m)
        # +2% 라운딩버퍼: vLLM 은 KV 를 고정 블록 단위(기본 block_size=16토큰)로 할당하므로
        # 선형식(per_token×max_len×batch)이 블록 경계 반올림을 반영 못 해 vLLM 자신의
        # `_check_enough_kv_cache_memory` 문턱에 근소 미달하는 사례가 실측됐다(2026-08-11:
        # qwen3-4b batch=1 "needed 4.5GiB > available 4.5GiB" — 표시상 동률이나 내부 초과).
        # required_kv_bytes()(estimate_vram.py, 공식-only 폴백 경로)에도 동형 버퍼가 적용되나
        # 이 measured-per-token 경로가 실제로 항상 우선 실행되므로 여기가 진짜 적용점이다.
        # 상수는 estimate_vram.KV_BLOCK_ALIGN_BUFFER **단일 소유**(2026-08-13 — 매직넘버 두 벌 제거).
        if req_tokens is None:
            required = int(per_token * max_len * batch * KV_BLOCK_ALIGN_BUFFER)
        else:
            required = int(per_token * req_tokens * KV_BLOCK_ALIGN_BUFFER)
    elif req_tokens is None:
        required = required_kv_bytes(parsed, max_len, batch, kv_dtype_bytes)
    else:
        required = required_kv_bytes(parsed, req_tokens, 1, kv_dtype_bytes)
    basis = ("max_model_len=%d×batch=%d" % (max_len, batch) if req_tokens is None else
             "max(max_model_len=%d, L=%d×batch=%d)=%d tokens" % (max_len, int(request_tokens), batch, req_tokens))
    max_safe = max_safe_kv_bytes(budget, deploy_gmu, weights_b, overhead_b)
    if max_safe <= 0:
        return {"kv": None, "fail": {
            "failure_class": "vram_infeasible", "adjust_target": None,
            "note": "weights+overhead(실측) 만으로 천장 초과 → KV 조정 불가(HITL)."}, "note": ""}
    if required > max_safe:
        return {"kv": None, "fail": {
            "failure_class": "vram_infeasible", "adjust_target": None,
            "note": ("required_kv=%d > max_safe_kv=%d (실측 기반 · %s) → "
                     "max_model_len·batch 를 KV 로 만족 불가(HITL)." % (required, max_safe, basis))},
            "note": ""}
    kv = int(min(required, max_safe))
    return {"kv": kv, "fail": None,
            "note": "kv=min(required=%d, max_safe=%d)=%d. (%s)" % (required, max_safe, kv, basis)}


def cmd_simulate(args):
    cfg = load_config(args.config)
    model_path, nas_root, _vram_budget_cfg, _safety_cfg, kv_bytes_cfg, container_root = _cfg_common(cfg, REPO_ROOT)
    tp = resolve_tp(cfg, REPO_ROOT)
    _guard_tp(tp, REPO_ROOT)
    # lockset 출처 어휘의 소유자를 **트라이얼 전에** 확인한다 — 몇 시간 돈 뒤 각인 단계에서 멈추면 늦다.
    _lockset_vocab()
    _lockset_out = getattr(args, "lockset_out", None)
    if not _lockset_out and _is_campaign_cell_lockset(args.candidate):
        # 캠페인 셀 lockset 을 입력으로 받았으면 수렴 각인은 **그 자리**에 쓴다(리뷰 교정 2026-09-14). 플래그를 잊은
        # 실행이 각인을 run_summary 에만 남기면 셀 lockset 의 explorer-phase2 가 다시 절차 자기선언으로 돌아간다 —
        # 단계 ① 이 넘긴 이월 항목("기계가 자동 기입")이 호출부 기억에 달리게 된다. 캠페인 밖 lockset 은 종전대로
        # 선택이다(입력 파일을 기본으로 덮어쓰지 않는다).
        _lockset_out = args.candidate
        print("[recipe] ⓘ --candidate 가 캠페인 셀 lockset 이다 — 수렴 각인을 그 파일에 쓴다(--lockset-out 기본값): %s"
              % args.candidate, file=sys.stderr)
    if (_lockset_out and os.path.exists(_lockset_out) and not args.force
            and os.path.realpath(_lockset_out) != os.path.realpath(args.candidate)):
        _die("--lockset-out 대상이 이미 있다: %s — --candidate 와 같은 파일(셀 lockset materialize)이 아니면 "
             "--force 로만 덮어쓴다." % _lockset_out)

    # ── gmu 두 역할 + 예산 (2026-09-14 · plan_26091407 §4.3) ──────────────────────────────────
    #   gate_margin(=safety_margin · 예산 검증 게이트 승수)과 deploy_gmu(=target_gpu.target_gmu · 배포값이자
    #   클램프 산식 승수)를 가른다. host 흐름도 target_gpu 블록을 채워 같은 해소 경로를 탄다.
    roles = _gpu_roles(cfg, tp, REPO_ROOT)
    budget = roles["budget_gib"]
    gate_margin = roles["gate_margin"]
    deploy_gmu = roles["deploy_gmu"]
    tp_divisor = roles["tp_divisor"]
    print(
        "[recipe] gmu 역할(%s 흐름): deploy_gmu=%.3f (%s) · gate_margin=%.3f (%s) · budget=per_card_vram %.2f GiB "
        "(%s) · gpu_model=%s · TP=%d(÷%d) · lockset gmu_source=%s"
        % (roles["flow"], deploy_gmu, roles["deploy_gmu_source"], gate_margin, roles["gate_margin_source"],
           budget, roles["budget_source"], roles["gpu_model"], tp, tp_divisor, roles["gmu_source"]),
        file=sys.stderr,
    )
    if roles["gmu_source"] != "target_gmu":
        print("[recipe] ⚠ 배포 gmu 가 정본 칸(target_gpu.target_gmu)에서 오지 않았다 — %s. lockset gmu_source=%s 로 "
              "각인한다." % (roles["deploy_gmu_source"], roles["gmu_source"]), file=sys.stderr)
    if deploy_gmu > gate_margin + 1e-9:
        print("[recipe] ⓘ deploy_gmu %.3f > gate_margin %.3f — 클램프 산식 천장(budget×deploy_gmu)이 검증 게이트 천장"
              "(budget×gate_margin)보다 높다. 게이트는 weights+overhead 만 본다(기재 · 차단 ✗)."
              % (deploy_gmu, gate_margin), file=sys.stderr)
    # 선언 TP(declared_axes.tp) ↔ manifest 파생 TP — **기재**만 한다(TP 의 권위는 manifest · 다르면 선언이 틀린 것).
    #   모양 결함(문자열·실수 등)도 멈추지 않고 malformed 로 적는다 — 산식 입력이 아니기 때문이다(strict=False).
    _dtp, _dtp_state, _dtp_src = _declared_axis(cfg, "tp", strict=False)
    declared_axes_check = {"tp": {"declared": _dtp, "declared_state": _dtp_state, "declared_source": _dtp_src,
                                  "resolved": tp, "resolved_source": "resolve_tp(config override > manifest)",
                                  "match": (None if _dtp is None else _dtp == tp)}}
    if _dtp is not None and _dtp != tp:
        print("[recipe] ⚠ declared_axes.tp=%d ≠ 해소 TP=%d — TP 의 권위는 manifest 다. 선언을 고쳐라(기재 · 차단 ✗)."
              % (_dtp, tp), file=sys.stderr)
    elif _dtp_state == "malformed":
        print("[recipe] ⚠ %s — 해소 TP=%d 와 대조하지 못했다(기재 · 차단 ✗)." % (_dtp_src, tp), file=sys.stderr)

    # parse(결정론) — NAS 부재/해소 실패 시 traceback 금지(클린 어보트로 통일).
    try:
        parsed = parse(model_path, nas_host_root=nas_root, nas_container_root=container_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")

    candidate = _load_candidate(args.candidate)
    # 입력 lockset 원본 — 수렴 시 explorer 가 **파생 칸만** 갱신해 되쓴다. 아래 주입(model_path_container·
    # served_model_name·gpu_memory_utilization)은 트라이얼 입력이지 lockset 칸이 아니다 — 되쓰면 다음 실행에서
    # 옛 gmu 가 setdefault 로 되살아나 deploy_gmu 를 이긴다.
    raw_lockset = copy.deepcopy(candidate)
    kv_dtype_bytes = _kv_dtype_bytes(candidate)
    # max-num-seqs 산식 계획(§4.2). 직전 explorer 산출(batch·KV 클램프)은 여기서 비운다(재측정 · carry-forward ✗).
    # 입력이 들고 온 출처 칸의 어휘 이탈은 멈추지 않고 기록한다(P6 와 같은 등급 — 기재).
    batch_plan = _batch_plan(candidate, cfg)
    kv_origin = "input" if candidate.get("kv_cache_memory_bytes") is not None else None
    print("[recipe] max-num-seqs 계획: mode=%s · 요구=%s(%s) · 대표 길이=%s(%s)%s%s"
          % (batch_plan["mode"], batch_plan["requirement"], batch_plan["requirement_state"],
             batch_plan["typical"], batch_plan["typical_state"],
             (" · 손레버 batch=%d 유지(덮어쓰기 ✗ · KV-fit 은 대조 기재)" % batch_plan["hand_batch"]
              if batch_plan["mode"] == "hand-lever" else
              (" · 직전 산출 batch=%s 는 승계하지 않는다" % batch_plan["prior_batch"]
               if batch_plan["prior_batch"] is not None else "")),
             (" · 직전 산출 KV 클램프=%s 는 승계하지 않는다(trial1 을 언클램프 실측으로 되돌린다)"
              % batch_plan["prior_kv"] if batch_plan["prior_kv"] is not None else "")),
          file=sys.stderr)
    for _anom in batch_plan["input_source_anomalies"]:
        print("[recipe] ⚠ %s(기재 · 차단 ✗)" % _anom, file=sys.stderr)

    # candidate 에 parse 산출 모델 식별자 + 서빙명을 주입(실 docker 가 올바른 모델을
    # 올바른 served-model-name 으로 서빙하도록 — 스모크가 이 이름으로 호출). setdefault=lockset 우선.
    candidate.setdefault("model_path_container", parsed.get("model_path_container"))
    candidate.setdefault("model_id", parsed.get("model_id"))
    _serving0 = cfg.get("serving") or {}
    if _serving0.get("served_model_name"):
        candidate.setdefault("served_model_name", _serving0.get("served_model_name"))
    # 트라이얼 gmu 기본값 = deploy_gmu(배포될 값으로 검증한다). 통합메모리(GB10)서 vLLM 기본 0.92 는
    # 기동 전 free ≥ ceil(total×gmu) 검사에 걸리므로 명시한다(SKILL §5). 실 KV 는 절대 클램프가 제어.
    # lockset 이 gpu_memory_utilization 을 들고 있으면 그것이 **트라이얼-로컬** 값으로 이긴다(분기는 수렴 시 경고).
    candidate.setdefault("gpu_memory_utilization", deploy_gmu)

    # 측정 하드웨어 total(torch.cuda 기준; DGX Spark 는 nvidia-smi N/A). consolidated 메모리
    # 라인이 없는 vLLM 빌드에서 overhead = gmu_trial×device_total − weights − kv 로 유도하는 데 쓴다.
    # 2026-09-05(G-B5): 기본값 121.69 는 **이 캠페인 호스트(GB10)의 실측치를 손으로 적은 것**이었다.
    #   HW 사실의 권위는 manifest·실측이지 코드 상수가 아니다(헌법). 순서: 선언 > 실측 > fail-loud.
    #   통합메모리 노드에서는 디바이스 풀이 곧 호스트 RAM 풀이므로 /proc/meminfo 가 실측 통로다.
    device_total_gib, device_total_src = _resolve_device_total_gib(cfg)
    print("[recipe] device_total_gib=%.2f (%s)" % (device_total_gib, device_total_src),
          file=sys.stderr)

    # run_id: 인자(--run-id) 우선, 없으면 <YYYYMMDDHH>_<seq>_<주제> 결정론 생성.
    from datetime import datetime

    if args.run_id:
        run_id = args.run_id
    else:
        topic = args.topic or (parsed.get("model_id") or "sim").replace("/", "_")
        run_id = "%s_1_%s" % (datetime.now().strftime("%Y%m%d%H"), topic)
    run_dir = simlog_writer.new_run_dir(REPO_ROOT, run_id)
    print(f"[recipe] simlog run_dir: {run_dir}", file=sys.stderr)

    # run_trial opts: 이미지·timeout·dry-run/mock-profile·NAS 마운트값은 contract 기본 사용.
    # 실 docker 서빙 포트·서빙명은 config.serving 에서 가져온다(없으면 run_trial 기본).
    _serving = cfg.get("serving") or {}
    opts = {
        "image": args.image,
        "timeout": args.timeout,
        "dry_run": bool(args.dry_run),
        "mock_profile": args.mock_profile,
        "served_model_name": _serving.get("served_model_name"),
        "port": int(_serving["port"]) if _serving.get("port") is not None else None,
        "nas_mount": nas_root,  # config.nas_host_root → run_trial NAS 마운트(하드코딩 /mnt/models 갭 수정)
        # config.nas_container_root → 트라이얼 컨테이너 마운트 경로. 이게 없으면 quant_model 계열에서
        # 트라이얼(/app/models)과 서빙(/app/quant_models)의 경로가 갈린다(2026-08-01 실측).
        "nas_container_root": container_root,
        "tiktoken_host_path": cfg.get("tiktoken_host_path"),  # config → run_trial /encodings:ro 마운트(C8 에어갭 자산 배선)
        # JIT 캐시 통로 + 컴파일 팬아웃 캡 — serve 평면(docker-compose)과 parity.
        # 기본을 serve 와 **같은 디렉터리**로 잡아 trial 이 데운 캐시를 serve 가 그대로 쓴다
        # (반대도 성립). 통로 분리는 두 평면이 서로 cold 를 반복하게 만들 뿐이다.
        "jit_cache_root": cfg.get("jit_cache_root") or _default_jit_cache_root(),
        "max_jobs": cfg.get("max_jobs", 4),
        # ── 서빙 예산 선언 배선 (2026-08-16 · plan_26081415 C3-1) ─────────────────────────
        #   run_trial 이 로드 개시 전에 `declare-budget` 을 발행하려면 세 값이 필요하고, 셋 다
        #   **이 스코프에 이미 있다**. 넘기지 않으면 run_trial 이 `node_dir_unresolved` 로 조용히
        #   skip 해 ETA 워치독이 무제한(arm_ceiling=999999999)으로 돌아간다 — 즉 **배선 부재가
        #   곧 무보호**다. 이 결함이 생긴 방식이 정확히 "계획은 있고 배선은 없음"이었다.
        #   · manifest  : 선언 기록 위치(node_id = nodes[].role)를 파생. `_read_manifest` 재사용.
        #   · tp        : weights = ckpt ÷ tp. manifest 권위(resolve_tp) — candidate 추측 금지.
        #   · checkpoint_bytes: **로드-전 RAM 게이트가 쓰는 바로 그 값**을 그대로 넘겨 두 게이트가
        #                 같은 축을 보게 한다(아래 preload_ram_gate 호출과 동일 입력).
        "manifest": _read_manifest(REPO_ROOT)[1],
        "tp": tp,
        "checkpoint_bytes": parsed.get("native_weight_bytes"),
        # ── 측정 트라이얼 gmu 캡 게이트 (2026-08-23 · plan_26082223 결함 B) ────────────
        #   run_trial 은 통합메모리에서만 gmu 를 호스트 바닥 준수 상한으로 깎는다. 그 판정에
        #   필요한 두 사실(device_total ↔ MemTotal)이 **이 스코프에 이미 있다** — 넘기지 않으면
        #   run_trial 이 판정 근거가 없어 캡을 못 걸고, 측정 트라이얼은 계속 구조적으로 사살된다
        #   ("만든 것과 도는 것은 다르다" — 배선이 없으면 코드는 없는 것과 같다).
        "unified_memory": _is_unified_memory(device_total_gib),
        # 예산 선언 overhead — CLI > config > run_trial 기본값. 상수 하나로 두면 워크로드마다
        #   틀리고, 틀리는 방향이 **무장 밴드를 넓히는 쪽**이다(run_trial BUDGET_OVERHEAD_MIB 주석).
        "overhead_mib": getattr(args, "overhead_mib", None) or cfg.get("budget_overhead_mib"),
    }
    opts = {k: v for k, v in opts.items() if v is not None}

    cap = int(args.cap)
    correction_history = []
    converged = False
    final_trial = None
    final_class = None
    batch_decided = batch_plan["mode"] == "hand-lever"
    clamp_provisional = False   # 트라이얼 루프가 batch 산출 **전에** 잡은 클램프인가(배포값 아님 — 위상 1 에서 재산정)
    batch_derivation = None
    engine_obs = []       # 트라이얼마다 엔진 보고 KV 토큰·max_concurrency(최악 길이 — 보수 하한 기재)

    for trial_number in range(1, cap + 1):
        print(
            f"[recipe] trial {trial_number}/{cap} — candidate id={candidate.get('id')} "
            f"max_len={candidate.get('max_model_len')} batch={candidate.get('batch')} "
            f"kv_bytes={candidate.get('kv_cache_memory_bytes')}",
            file=sys.stderr,
        )

        # ── 로드-전 RAM 게이트 (plan_26071019 §2.6 — 사고 #5 교훈: ckpt 66.97GiB >
        #    가용 45.80GiB 인데 무게이트 로드 → 하드다운). 실 docker 경로에서만 발동 —
        #    dry-run/mock 은 결정론 루프 테스트 계약 보존. 매 trial 전 재측정(트라이얼 간
        #    메모리 상태 변동). 크기 미상이면 게이트 내부에서 경고 후 생략(음성정직).
        if not opts.get("dry_run") and not opts.get("mock_profile"):
            _g = preload_ram_gate(parsed.get("native_weight_bytes"), tp=tp)
            if not _g["ok"]:
                # 거부도 결정론 판정 → simlog run 계약(docs.md: 수렴 여부·trial_count·최종
                # candidate 필수) 준수: run_summary.json 을 refused-gate 로 기록 후 종료(다른
                # 종단 실패의 _simulate_hitl summary 패턴과 대칭 — 사후분석 복원성 보존).
                simlog_writer.write_summary(run_dir, {
                    "run_id": run_id,
                    "converged": False,
                    "failure_class": "preload_ram_gate_refused",
                    "note": ("MemAvailable=%sMiB < required=%sMiB (ckpt÷tp=%d+floor) "
                             "@trial %d — 잔존 컨테이너/페이지캐시 정리 후 재시도"
                             % (_g["avail_after_mib"], _g["required_mib"], tp, trial_number)),
                    "trial_count": len(correction_history),
                    "candidate": candidate,
                    "ram_gate": _g,
                    "correction_history": correction_history,
                })
                _die(
                    "로드-전 RAM 게이트 거부(trial %d): MemAvailable=%sMiB < required=%sMiB "
                    "(ckpt÷tp+floor) — 잔존 컨테이너/페이지캐시 정리 후 재시도. "
                    "헌법 호스트 안전체계 따름정리 · plan_26071019 §2.6 · "
                    "run_summary=%s"
                    % (trial_number, _g["avail_after_mib"], _g["required_mib"],
                       os.path.join(run_dir, "run_summary.json")),
                    code=7,
                )

        # ── 실서빙(또는 dry-run/mock) 트라이얼 ──────────────────────────────
        trial = run_trial(candidate, run_dir, trial_number, opts)
        # provenance 전파 게이트(2026-08-13 · plan_26081314 D1): trial 산출물은 출처를 스스로
        # 밝혀야 한다. 필드가 없으면 run_trial 이 계약을 어긴 것이므로 조용히 진행하지 않는다 —
        # 출처 미상을 실측처럼 흘려보내면 하류 판정·인증서가 무엇을 근거로 삼았는지 알 수 없게 된다.
        _prov = trial.get("provenance")
        if _prov not in PROVENANCE_VALUES:
            _die("trial provenance 미표기/미지값(%r) — run_trial 계약 위반. 출처 미상 결과는 "
                 "판정에 쓰지 않는다(plan_26081314 D1)." % (_prov,), code=7)
        # consolidated 메모리 라인이 없는 빌드 보강: overhead 유도(제자리). dry-run mock 이
        # 이미 non_kv_overhead 를 주면 건드리지 않는다.
        # gmu_fallback 은 **실제로 emit 된 값**이어야 한다(trial.effective_gmu). 결함 B 캡이
        # 걸린 트라이얼에서 요청값을 쓰면 overhead = gmu×total − w − kv 가 그대로 틀어진다.
        _enrich_overhead(trial.get("vllm_profile"), device_total_gib,
                         gmu_fallback=(trial.get("effective_gmu")
                                       if trial.get("effective_gmu") is not None
                                       else candidate.get("gpu_memory_utilization")),
                         kv_was_explicit=candidate.get("kv_cache_memory_bytes") is not None)
        final_trial = trial
        _prof = trial.get("vllm_profile") or {}
        engine_obs.append({"trial_number": trial_number,
                           "clamped": candidate.get("kv_cache_memory_bytes") is not None,
                           "batch": candidate.get("batch"),
                           "kv_cache_tokens": _prof.get("kv_cache_tokens"),
                           "max_concurrency": _prof.get("max_concurrency")})

        # ── per-trial simlog 증거 4종 기록(SKILL.md §6) ───────────────────
        #   trialNN_vllm.log / _profile.json / _candidate.yaml / _smoke.json.
        #   로그 텍스트는 run_trial 의 log_path(실 docker 가 기록) 에서 읽고,
        #   write_trial 가 동일 run_dir 의 trialNN_vllm.log 를 (없으면 생성)하여
        #   dry-run 에서도 log_path 가 dangling 되지 않도록 보장한다.
        _vllm_log_text = _read_log_text(trial.get("log_path"))
        simlog_writer.write_trial(
            run_dir,
            trial_number,
            _vllm_log_text,
            trial.get("vllm_profile"),
            candidate,
            trial.get("functional"),
        )

        # ── 결정론 분류 ────────────────────────────────────────────────────
        # 게이트 승수는 gate_margin 이다(deploy_gmu 가 아니다). batch 가 산식으로 정해진 뒤의 트라이얼은
        # 엔진 보고 토큰으로 KV-fit 을 다시 잰다(2-위상의 재검증 · adjust_target=batch).
        verdict = sim_classify(trial, budget_gib=budget, safety_margin=gate_margin,
                               typical_request_tokens=(_kv_fit_length(batch_plan, candidate)
                                                       if batch_decided else None))
        final_class = verdict
        fclass = verdict.get("failure_class")
        adjust = verdict.get("adjust_target")
        print(f"[recipe] classify → {fclass} (adjust={adjust}) :: {verdict.get('note')}",
              file=sys.stderr)

        # ── 수렴 후보 ──────────────────────────────────────────────────────
        # 절대 KV 클램프가 아직 미설정이면(=측정 전용 트라이얼) 통과해도 수렴 아님.
        # 클램프를 산정해 candidate 에 박고, 클램프 적용 검증 트라이얼을 한 번 더 돈다.
        # (이 스킬의 산출물 핵심 = config 에 박히는 --kv-cache-memory-bytes 절대값.)
        if fclass == "none":
            # ── 위상 1: batch 산출(첫 통과 트라이얼 · §4.2) ─────────────────────────────
            #   잠정 클램프(batch 산출 전 vram_oom 재산정분)는 배포값이 아니다 → 환산 경로(None)로 KV-fit 을 잰다.
            _fit_tokens, _fit_src = _kv_fit_tokens(_prof, budget, deploy_gmu, tp_divisor,
                                                   (None if clamp_provisional
                                                    else candidate.get("kv_cache_memory_bytes")))
            batch_before = candidate.get("batch")
            if not batch_decided:
                batch_derivation = derive_batch(batch_plan, candidate.get("max_model_len"), _fit_tokens, _fit_src)
                batch_decided = True
                candidate["batch"] = batch_derivation["batch"]
                print("[recipe] max-num-seqs 산출 → batch=%s (%s) :: %s"
                      % (candidate["batch"], batch_derivation["batch_source"], batch_derivation["reason"]),
                      file=sys.stderr)
                if clamp_provisional and candidate.get("batch") is None:
                    # batch 를 산출하지 않았다(요구 없음·KV-fit 산출 불가) → 잠정 클램프의 산정 기준(max_model_len 1건)이
                    # 곧 최종 기준이다. 방금 통과한 이 트라이얼이 그 클램프의 검증이므로 수렴한다 — 재산정하면 블록 반올림
                    # 만큼 바이트가 흔들려 같은 모양의 클램프로 빈 트라이얼을 한 번 더 돈다.
                    clamp_provisional = False
                    converged = True
                    break
                if clamp_provisional:
                    # 잠정 클램프는 batch 없이(max_model_len 1건) 산정됐다 → **산출 batch 로 required 를 재계산**해
                    # 클램프를 다시 잡고 재검증한다(plan §4.2 "required_kv 는 산출된 batch 로 재계산"). 같은 값이면 이
                    # 트라이얼이 곧 재검증이므로 수렴한다(빈 트라이얼을 돌지 않는다).
                    clamp_before = candidate.get("kv_cache_memory_bytes")
                    res = _resolve_clamp_kv(
                        parsed, candidate, _prof,
                        budget, deploy_gmu, kv_dtype_bytes, tp_divisor=tp_divisor,
                        request_tokens=_kv_fit_length(batch_plan, candidate),
                    )
                    record = {"trial_number": trial_number, "failure_class": "measure",
                              "adjust_target": "kv_cache_memory_bytes",
                              "note": ("잠정 클램프(batch 산출 전 vram_oom 재산정분) 트라이얼 통과 → max-num-seqs 산출 → "
                                       "산출 batch 로 클램프 재산정 후 재검증 트라이얼."),
                              "before": {"kv_cache_memory_bytes": clamp_before, "batch": batch_before}}
                    if res["fail"] is not None or res["kv"] is None:
                        final_class = res["fail"] or {
                            "failure_class": "unknown", "adjust_target": None,
                            "note": "잠정 클램프 트라이얼 통과했으나 weights/overhead 실측 부재 → 클램프 재산정 불가(HITL)."}
                        record["after_note"] = final_class["note"]
                        correction_history.append(record)
                        simlog_writer.append_correction(run_dir, record)
                        break
                    clamp_provisional = False
                    kv_origin = "trial-loop"
                    if int(res["kv"]) == int(clamp_before) and candidate.get("batch") == batch_before:
                        converged = True
                        break
                    candidate["kv_cache_memory_bytes"] = int(res["kv"])
                    record["after"] = {"kv_cache_memory_bytes": int(res["kv"]), "batch": candidate.get("batch")}
                    record["after_note"] = res["note"]
                    correction_history.append(record)
                    simlog_writer.append_correction(run_dir, record)
                    continue
                if candidate.get("kv_cache_memory_bytes") is not None:
                    # 클램프가 이미 있는 트라이얼(입력 클램프)에서 batch 가 새로 정해졌다 → 그 batch 로 재검증한다.
                    if candidate["batch"] != batch_before:
                        record = {"trial_number": trial_number, "failure_class": "measure",
                                  "adjust_target": "batch",
                                  "note": "통과 트라이얼에서 max-num-seqs 산출 → batch 고정 재검증 트라이얼.",
                                  "before": {"batch": batch_before}, "after": {"batch": candidate["batch"]},
                                  "after_note": batch_derivation["reason"]}
                        correction_history.append(record)
                        simlog_writer.append_correction(run_dir, record)
                        continue
                    converged = True
                    break
            elif batch_derivation is None and batch_plan["mode"] == "hand-lever":
                # 손레버는 덮어쓰지 않는다 — 같은 산식이 냈을 값을 **대조용으로** 기재한다(Q1 표시+대조).
                batch_derivation = derive_batch(batch_plan, candidate.get("max_model_len"), _fit_tokens, _fit_src)
                batch_derivation.update({"derived_batch_would_be": batch_derivation["batch"],
                                         "derived_batch_source_would_be": batch_derivation["batch_source"],
                                         "batch": candidate.get("batch"), "batch_source": "hand-lever",
                                         "applied": False,
                                         "reason": "손레버 batch=%s 유지(측정 산물 아님) · 산식 대조: %s"
                                                   % (candidate.get("batch"), batch_derivation["reason"])})
            if adjust == "batch" and verdict.get("kv_fit") is not None and batch_plan["mode"] == "derive":
                # ── 위상 2 재검증 실패: 클램프 트라이얼의 엔진 보고 토큰이 batch × L 을 못 담는다 → batch 하향 ──
                #   클램프는 **다시 산정하지 않는다**(위상 분리 — 낮춘 batch 는 같은 클램프 안에 들어간다는 것이 방금
                #   엔진이 보고한 사실이다. 클램프를 따라 줄이면 batch↔클램프가 서로를 입력으로 삼아 진동한다).
                new_batch = max(int(verdict["kv_fit"]), 1)
                record = {"trial_number": trial_number, "failure_class": fclass, "adjust_target": "batch",
                          "note": verdict.get("note"), "before": {"batch": batch_before},
                          "after": {"batch": new_batch},
                          "after_note": "KV-fit 재검증 → batch %s→%d 하향 후 재검증 트라이얼." % (batch_before, new_batch)}
                candidate["batch"] = new_batch
                _der = dict(batch_derivation or {})
                _der["phase2_adjustments"] = list(_der.get("phase2_adjustments") or []) + [{
                    "trial_number": trial_number, "batch_before": batch_before, "batch_after": new_batch,
                    "kv_fit_check": verdict.get("kv_fit_check")}]
                _der.update(batch=new_batch, batch_source="kv-fit-measured", kv_fit=int(verdict["kv_fit"]),
                            verified=None,
                            reason=("클램프 트라이얼 %d 의 엔진 보고 KV 토큰으로 KV-fit %d 재측정 → batch %s 에서 낮춤"
                                    "(위상 1: %s)" % (trial_number, int(verdict["kv_fit"]), batch_before,
                                                     (batch_derivation or {}).get("reason"))))
                batch_derivation = _der
                correction_history.append(record)
                simlog_writer.append_correction(run_dir, record)
                if trial_number == cap:
                    # 마지막 칸에서 낮췄다 — 트라이얼은 통과했지만 낮춘 batch 의 재검증을 돌 칸이 없다. HITL 요약이
                    # "failure_class=none" 만 보이면 왜 멈췄는지 읽히지 않으므로 사유를 note 에 싣는다(리뷰 교정 2026-09-14).
                    final_class = dict(verdict, note=(
                        "cap(%d) 소진 — 위상 2 가 batch 를 %s→%d 로 낮췄으나 재검증 트라이얼을 돌 칸이 없다(트라이얼 자체는 "
                        "통과). --cap 을 늘려 다시 돈다 — 산식 경로는 측정·클램프 검증·위상 2 재검증까지 최소 3칸이 든다. "
                        "원 분류: %s" % (cap, batch_before, new_batch, verdict.get("note"))))
                continue
            if candidate.get("kv_cache_memory_bytes") is None:
                res = _resolve_clamp_kv(
                    parsed, candidate, _prof,
                    budget, deploy_gmu, kv_dtype_bytes, tp_divisor=tp_divisor,
                    request_tokens=_kv_fit_length(batch_plan, candidate),
                )
                record = {
                    "trial_number": trial_number,
                    "failure_class": "measure",
                    "adjust_target": "kv_cache_memory_bytes",
                    "note": "측정 트라이얼 통과 → (max-num-seqs 산출) → 절대 KV 클램프 산정 후 검증 트라이얼.",
                    "before": {"kv_cache_memory_bytes": None, "batch": batch_before},
                }
                if res["fail"] is not None:
                    final_class = res["fail"]
                    record["after_note"] = res["fail"]["note"]
                    correction_history.append(record)
                    simlog_writer.append_correction(run_dir, record)
                    break
                if res["kv"] is None:
                    final_class = {
                        "failure_class": "unknown", "adjust_target": None,
                        "note": "측정 트라이얼 통과했으나 weights/overhead 실측 부재 → 클램프 산정 불가(HITL).",
                    }
                    record["after_note"] = final_class["note"]
                    correction_history.append(record)
                    simlog_writer.append_correction(run_dir, record)
                    break
                candidate["kv_cache_memory_bytes"] = res["kv"]
                kv_origin = "trial-loop"
                record["after"] = {"kv_cache_memory_bytes": res["kv"], "batch": candidate.get("batch")}
                record["after_note"] = res["note"]
                correction_history.append(record)
                simlog_writer.append_correction(run_dir, record)
                continue
            converged = True
            break

        # ── 구조적 불가(infeasible) / unknown → 즉시 HITL 중단 ──────────────
        if fclass in ("vram_infeasible", "unknown"):
            break

        # ── 조정(다음 트라이얼 입력 준비) ─────────────────────────────────
        record = {
            "trial_number": trial_number,
            "failure_class": fclass,
            "adjust_target": adjust,
            "note": verdict.get("note"),
            "before": {
                "kv_cache_memory_bytes": candidate.get("kv_cache_memory_bytes"),
                "attention_backend": candidate.get("attention_backend"),
                "tool_call_parser": candidate.get("tool_call_parser"),
                "reasoning_parser": candidate.get("reasoning_parser"),
            },
        }

        if adjust == "kv_cache_memory_bytes":
            # 결정론 KV 재산정(helper): weights/overhead 실측 → kv = min(required, max_safe).
            res = _resolve_clamp_kv(
                parsed, candidate, trial.get("vllm_profile") or {},
                budget, deploy_gmu, kv_dtype_bytes, tp_divisor=tp_divisor,
                request_tokens=_kv_fit_length(batch_plan, candidate),
            )
            if res["fail"] is not None:
                final_class = res["fail"]
                record["after_note"] = res["fail"]["note"]
                correction_history.append(record)
                simlog_writer.append_correction(run_dir, record)
                break
            if res["kv"] is None:
                # 실측 부재(OOM 으로 프로파일 라인 미출력) → 직전 KV 를 절반으로 축소(보수적).
                cur_kv = candidate.get("kv_cache_memory_bytes")
                if cur_kv is None:
                    final_class = {
                        "failure_class": "unknown",
                        "adjust_target": None,
                        "note": "OOM 인데 vllm_profile 실측 부재 + 기존 KV 미설정 → Model-C(HITL).",
                    }
                    record["after_note"] = final_class["note"]
                    correction_history.append(record)
                    simlog_writer.append_correction(run_dir, record)
                    break
                new_kv = int(cur_kv) // 2
                record["after_note"] = "실측 부재 → 직전 KV 절반 축소(보수적)."
            else:
                new_kv = res["kv"]
                record["after_note"] = res["note"]
            candidate["kv_cache_memory_bytes"] = int(new_kv)
            kv_origin = "trial-loop"
            # batch 가 아직 산출되지 않았다면(derive 모드 · 첫 통과 전 OOM) 이 클램프는 max_model_len 1건 기준의
            # **잠정값**이다 — 통과 트라이얼의 위상 1 이 배포 천장으로 KV-fit 을 환산하고 산출 batch 로 다시 잡는다.
            clamp_provisional = not batch_decided
            record["after"] = {"kv_cache_memory_bytes": int(new_kv)}
            if clamp_provisional:
                record["after_note"] = (record.get("after_note") or "") + (
                    " (잠정 클램프 — batch 산출 전이다. 통과 트라이얼에서 산출 batch 로 재산정한다)")

        elif adjust in ("attention_backend", "tool_call_parser", "reasoning_parser"):
            # soft 변수 폴백: *_candidates 순서에서 다음 후보로 교체.
            nxt = _next_soft_value(candidate, adjust)
            if nxt is None:
                final_class = {
                    "failure_class": "functional",
                    "adjust_target": adjust,
                    "note": "%s 폴백 후보 소진 → Model-C(HITL)." % adjust,
                }
                record["after_note"] = final_class["note"]
                correction_history.append(record)
                simlog_writer.append_correction(run_dir, record)
                break
            candidate[adjust] = nxt
            record["after"] = {adjust: nxt}
            record["after_note"] = "%s → '%s' 폴백 후보로 교체." % (adjust, nxt)

        else:
            # adjust 없는 실패(functional 세부 미상 등) → HITL.
            final_class = {
                "failure_class": fclass,
                "adjust_target": None,
                "note": "조정 대상 미정 → Model-C(HITL).",
            }
            correction_history.append(record)
            simlog_writer.append_correction(run_dir, record)
            break

        correction_history.append(record)
        simlog_writer.append_correction(run_dir, record)

    # 산식·재검증 기록을 한 자리로 모은다(수렴·미수렴 요약과 lockset 이 같은 몸을 싣는다).
    if batch_derivation is not None:
        # 엔진 max_concurrency 는 "max_model_len 최악 길이 요청만 온다면" 의 동시성이다 — 대표 길이로 센 KV-fit 보다
        # 항상 작거나 같으므로 **보수 하한**이다. 산식 입력으로 쓰지 않고 트라이얼마다 나란히 적는다(F2: 파싱돼 있었는데
        # 소비자가 0 이었다).
        batch_derivation["engine_max_concurrency"] = {
            "note": "엔진 보고 max_concurrency 는 max_model_len 최악 길이 기준이다 — 보수 하한으로 기재(산식 입력 ✗)",
            "trials": [{"trial_number": o["trial_number"], "clamped": o["clamped"], "batch": o["batch"],
                        "max_concurrency": o["max_concurrency"],
                        "conservative_floor": (int(o["max_concurrency"]) if o["max_concurrency"] is not None
                                               else None),
                        "kv_cache_tokens": o["kv_cache_tokens"]}
                       for o in engine_obs]}
        _chk = (final_class or {}).get("kv_fit_check")
        batch_derivation["verification"] = _chk
        if batch_plan["mode"] == "derive" and batch_derivation.get("batch") is not None:
            # 위상 2 결과를 값 옆에 적는다 — 산출(위상 1)만 적고 재검증 결과를 비워 두면 "검증했다"와 "산출만 했다"가
            # 구분되지 않는다. 재검증을 못 했으면(엔진 토큰 미관측 등) 검증 안 됨으로 적는다(일치로 접지 않는다).
            batch_derivation["verified"] = bool(_chk and _chk.get("checked") and not _chk.get("exceeds"))
            if not batch_derivation["verified"]:
                batch_derivation["verified_gap"] = ((_chk or {}).get("reason")
                                                    or "클램프 트라이얼 재검증 기록이 없다(수렴 전 종료 또는 위상 2 미도달)")
        if batch_plan["input_source_anomalies"]:
            batch_derivation["input_source_anomalies"] = list(batch_plan["input_source_anomalies"])
        if batch_plan.get("prior_kv") is not None:
            batch_derivation["prior_kv"] = batch_plan["prior_kv"]
        batch_derivation["out_of_scope"] = (
            "무릎(동시성 대비 처리량 포화)·열벽은 explorer 가 재지 않는다 — adversarial-benchmark 스윕·노드 블랙박스가 재서 "
            "escalation_candidates / hint input 으로 넘긴다(트리플렛 직접 쓰기 ✗). max-num-batched-tokens 모델링 범위 밖.")
    lockset_ctx = {"raw": raw_lockset, "plan": batch_plan, "derivation": batch_derivation,
                   "roles": roles, "kv_origin": kv_origin, "declared_axes_check": declared_axes_check,
                   "out": _lockset_out}

    # ── 수렴 처리 ──────────────────────────────────────────────────────────
    if converged:
        return _simulate_converged(
            args, cfg, parsed, candidate, final_trial, tp, roles,
            run_dir, run_id, correction_history, lockset_ctx,
        )

    # ── 미수렴(cap 소진 / infeasible / unknown) → Model-C HITL 보고 ─────────
    _simulate_hitl(
        run_dir, run_id, candidate, final_trial, final_class, correction_history, cap,
        lockset_ctx=lockset_ctx,
    )


_GMU_ROLE_KEYS = ("flow", "deploy_gmu", "deploy_gmu_source", "deploy_gmu_declared", "deploy_gmu_role",
                  "gate_margin", "gate_margin_source", "gate_margin_role", "budget_gib", "budget_source",
                  "gpu_model", "tp_divisor")


def _gmu_roles_record(roles, trial=None, candidate=None):
    """요약·lockset 에 싣는 gmu 역할 기록. 트라이얼 gmu(측정용 · host 캡이 걸릴 수 있다)는 배포 gmu 와 **다른 칸**이다."""
    rec = {k: roles.get(k) for k in _GMU_ROLE_KEYS}
    if trial is not None:
        eff = trial.get("effective_gmu")
        rec["trial_gmu"] = eff if eff is not None else (candidate or {}).get("gpu_memory_utilization")
        rec["trial_gmu_source"] = ("run_trial.effective_gmu(실제 emit 값)" if eff is not None
                                   else "candidate.gpu_memory_utilization(트라이얼 요청값)")
    return rec


def _build_lockset(candidate, ctx, trial_provenance, trial):
    """수렴 candidate → lockset. **입력 lockset 의 칸을 보존**하고 explorer 파생 칸과 출처만 갱신·각인한다.

    각인(2026-09-14 · plan_26091407 §4.2·§4.3 · 단계 ① 이월 항목): `provenance=explorer-phase2` 와
    `batch_source`·`gmu_source`·`kv_source` 를 **기계가** 적는다 — 종전 표시는 저작자의 절차 자기선언뿐이었다.
    값은 전부 소유자 어휘로 교차검증된다(`_stamp`). `*_source` 는 **어느 규칙이 값을 냈는가**이고, 그 규칙의
    입력이 실측이었는지는 `trial_provenance`(run_trial 어휘 measured|mock|dry-run)가 말한다 — mock 으로 수렴한
    lockset 이 `kv-fit-measured` 를 들고 있어도 `trial_provenance=mock`·`measured=false` 가 옆에 선다(실측인 척 ✗).
    """
    raw, roles, plan, der = ctx["raw"], ctx["roles"], ctx["plan"], ctx["derivation"]
    out = copy.deepcopy(raw)
    for key in ("batch", "kv_cache_memory_bytes", "attention_backend", "tool_call_parser", "reasoning_parser"):
        out[key] = candidate.get(key)
    if plan["mode"] == "hand-lever":
        batch_source = "hand-lever"
    else:
        batch_source = (der or {}).get("batch_source")
    # KV 클램프: 트라이얼 루프가 산정했으면 measured-clamp, 입력 클램프를 바꾸지 않고 검증만 했으면 hand
    #   (직전 explorer 산출 클램프는 `_batch_plan` 이 이미 비웠으므로 여기 "input" 으로 오는 것은 사람 값뿐이다).
    if candidate.get("kv_cache_memory_bytes") is None:
        kv_source = None
    elif ctx["kv_origin"] == "trial-loop":
        kv_source = "measured-clamp"
    else:
        kv_source = "hand"
    out.update({
        "provenance": _stamp("provenance", "explorer-phase2"),
        "batch_source": _stamp("batch_source", batch_source),
        "kv_source": _stamp("kv_source", kv_source),
        "gmu_source": _stamp("gmu_source", roles["gmu_source"]),
        "_explorer_stamp_howto": (
            "recipe.py simulate 수렴이 기계 각인한 칸(2026-09-14 · plan_26091407 §4.2·§4.3): provenance·batch·"
            "batch_source·kv_cache_memory_bytes·kv_source·gmu_source·trial_provenance·batch_derivation·gmu_roles. "
            "*_source 는 값을 낸 규칙이고, 그 규칙의 입력이 실측이었는지는 trial_provenance 가 말한다. "
            "kv_source=hand 는 입력 lockset 클램프를 트라이얼이 바꾸지 않고 검증만 했다는 뜻이다."),
        "trial_provenance": trial_provenance,
        "measured": trial_provenance == PROVENANCE_MEASURED,
        "batch_derivation": der,
        "gmu_roles": _gmu_roles_record(roles, trial, candidate),
        "declared_axes_check": ctx["declared_axes_check"],
    })
    return out


def _simulate_converged(args, cfg, parsed, candidate, trial, tp, roles,
                        run_dir, run_id, correction_history, lockset_ctx):
    """수렴 시: 실측 채운 recipe → gen_recipe_set 3종 세트 + lockset 각인 + write_summary + feedback."""
    budget = roles["budget_gib"]
    deploy_gmu = roles["deploy_gmu"]
    gate_margin = roles["gate_margin"]
    profile = trial.get("vllm_profile") or {}
    weights_gib = profile.get("weights_gib")
    overhead_gib = profile.get("non_kv_overhead_gib")
    # kv_gib·total 은 측정값(profile.kv=언클램프 시 풀-점유)이 아니라 실제 config 에 박히는
    # 절대 클램프(candidate.kv_cache_memory_bytes) 기준으로 산정한다. 이게 96GB 카드 footprint.
    _gib = 1024 ** 3
    _kv_clamp = candidate.get("kv_cache_memory_bytes")
    kv_gib = (_kv_clamp / _gib) if _kv_clamp is not None else profile.get("kv_cache_gib")
    if None not in (weights_gib, overhead_gib, kv_gib):
        total_gib = weights_gib + overhead_gib + kv_gib
    else:
        total_gib = profile.get("total_pool_gib")

    serving = cfg.get("serving") or {}
    name = serving.get("config_name")
    port = serving.get("port")
    served_model_name = serving.get("served_model_name")
    if not name:
        _die("config.serving.config_name 누락")
    _validate_config_name(name)
    if port is None:
        _die("config.serving.port 누락")
    if not served_model_name:
        _die("config.serving.served_model_name 누락")
    port = int(port)

    # recipe dict: gen_recipe_set 가 읽는 키(quantization/max_model_len/gpu_memory_utilization
    # + Phase2 batch/kv_cache_memory_bytes/kv_cache_dtype/소프트 변수/vram_breakdown).
    # ★ 투영은 gen_recipe_set.recipe_from_candidate 가 **단독 소유**한다.
    #   예전엔 여기 dict 리터럴이 9개 필드만 복사해, 생성기를 고쳐도 노브가 이 홉에서
    #   조용히 떨어졌다(2026-08-01 moe_backend 실증 — 트라이얼 통과 ↔ 배포물 즉사).
    #   홉이 둘이면 둘 다 고쳐야 하는데 그걸 잊는 것이 결함 계열의 본질이라 홉을 하나로 모았다.
    # gmu 분기 가시화(2026-09-14 새 이름): 최종 트라이얼이 **실제로 emit 한** trial_gmu 와 배포에 박히는
    # deploy_gmu(= target_gpu.target_gmu)가 다르면 알린다. 트라이얼 gmu 는 측정 평면(host 바닥 캡이 걸릴 수 있다)
    # 이고 deploy_gmu 는 배포 평면이다 — 같은 이름으로 부르면 둘이 갈린 사실이 보이지 않는다.
    _gmu_rec = _gmu_roles_record(roles, trial, candidate)
    _trial_gmu = _gmu_rec.get("trial_gmu")
    if _trial_gmu is not None and abs(float(_trial_gmu) - float(deploy_gmu)) > 1e-9:
        print("[recipe] ⚠ gmu 분기: trial_gmu %.3f (%s) ↔ deploy_gmu %.3f (%s) — 검증한 값과 배포되는 값이 다르다. "
              "의도한 것이 아니면 lockset 의 gpu_memory_utilization 을 지우거나 target_gpu.target_gmu 를 맞춰라."
              % (float(_trial_gmu), _gmu_rec.get("trial_gmu_source"), float(deploy_gmu),
                 roles["deploy_gmu_source"]), file=sys.stderr)
    _final_prov = trial.get("provenance")
    lockset = _build_lockset(candidate, lockset_ctx, _final_prov, trial)
    recipe = recipe_from_candidate(candidate, **{
        # gpu-memory-utilization = deploy_gmu(target_gpu.target_gmu · 기동 전 free 검사에 쓰인다; 실제 KV 는 절대 클램프).
        "gpu_memory_utilization": deploy_gmu,
        # 값 옆 출처(§결정론 규율) — 생성기가 yaml 주석으로 싣는다. serve 노브가 아니다.
        "batch_source": lockset.get("batch_source"),
        "gmu_source": lockset.get("gmu_source"),
        # target_gpu 활성 시 gen_recipe_set 이 트리플렛 헤더에 이식 정직성 주석을 단다(§4.9, plan_26070809_47_07).
        "target_gpu": cfg.get("target_gpu"),
        "vram_breakdown": {
            "weights_gib": weights_gib,
            "kv_gib": kv_gib,
            "overhead_gib": overhead_gib,
            "total_gib": total_gib,
            "budget_gib": budget,
            "headroom_gib": (budget - total_gib) if total_gib is not None else None,
        },
    })

    try:
        paths = generate(
            parsed, recipe, name, output_root(REPO_ROOT), port, served_model_name, force=args.force,
            # 트라이얼이 **실제로 통과시킨** 이미지를 env 에 박는다. 비우면 compose 가 낡은
            # 기본값으로 조용히 폴백해 검증한 것과 다른 vLLM 을 서빙·측정한다(D8).
            image=candidate.get("image") or args.image,
        )
    except FileExistsError as e:
        _die(f"{e} (덮어쓰려면 --force)")

    print("[recipe] 수렴 — 생성된 3종 세트:")
    for p in paths:
        print(f"  - {p}")
    _lockset_out = lockset_ctx.get("out")
    if _lockset_out:
        os.makedirs(os.path.dirname(os.path.abspath(_lockset_out)) or ".", exist_ok=True)
        with open(_lockset_out, "w", encoding="utf-8") as fh:
            json.dump(lockset, fh, ensure_ascii=False, indent=2)
            fh.write("\n")
        print(f"[recipe] lockset 각인(provenance=explorer-phase2 · batch_source={lockset['batch_source']} · "
              f"gmu_source={lockset['gmu_source']} · kv_source={lockset['kv_source']} · "
              f"trial_provenance={_final_prov}): {_lockset_out}")
    else:
        print("[recipe] ⓘ --lockset-out 미지정 ∧ --candidate 가 캠페인 셀 lockset 이 아니다 — 각인된 lockset 은 "
              "run_summary.json 의 lockset 칸에만 남는다(셀 lockset 에 남기려면 --lockset-out 으로 경로를 넘겨라).",
              file=sys.stderr)

    # simlog run_summary.
    #   provenance 각인(2026-08-13 · plan_26081314 D1): 수렴한 레시피가 **무엇을 근거로** 수렴했는지를
    #   산출물 자체가 밝힌다. mock/dry-run 으로 수렴한 레시피를 실측 레시피와 같은 얼굴로 남기면,
    #   나중에 그 파일을 읽는 사람도 인증서 발행 경로도 진위를 가릴 수 없다.
    # ⚠ 이 함수의 파라미터명은 `trial` 이다(호출부의 지역변수명이 `final_trial` 일 뿐).
    #   2026-08-13 provenance 각인 도입(413b291) 때 호출부 이름을 그대로 적어 NameError 가 됐고,
    #   **수렴 성공 경로에서만** 터지므로 오래 숨어 있었다(대부분의 실행은 Model-C HITL 로 끝난다).
    #   3종 세트는 이미 생성된 뒤 죽어서 "파일은 있는데 run_summary 가 없는" 상태가 됐다 —
    #   evidence chain 이 끊긴다. 2026-08-16 첫 수렴 실행에서 발각(testlog_26081607 §10).
    summary = {
        "run_id": run_id,
        "converged": True,
        "trial_count": len(correction_history) + 1,
        "candidate": candidate,
        "vram_breakdown": recipe["vram_breakdown"],
        "generated_paths": paths,
        "correction_history": correction_history,
        "provenance": _final_prov,
        "measured": _final_prov == PROVENANCE_MEASURED,
        # 2026-09-14(plan_26091407 §4.2·§4.3): gmu 두 역할 · max-num-seqs 산식 · 각인된 lockset.
        "gmu_roles": _gmu_rec,
        "batch_derivation": lockset.get("batch_derivation"),
        "lockset": lockset,
        "lockset_out": _lockset_out,
    }
    if _final_prov != PROVENANCE_MEASURED:
        print(f"[recipe] ⚠ 이 레시피는 실측이 아니다(provenance={_final_prov}) — "
              f"서빙 판정·성능 인증의 근거로 쓰지 마라(plan_26081314 D1).", file=sys.stderr)
    simlog_writer.write_summary(run_dir, summary)
    print(f"[recipe] simlog 요약: {os.path.join(run_dir, 'run_summary.json')}",
          file=sys.stderr)

    # feedback 로그(converged=True, 실측 채움). 기존 ESTIMATED_FIELDS + 신규 nullable 키.
    from datetime import datetime

    record = {
        "timestamp": datetime.now().isoformat(),
        "model_id": parsed.get("model_id"),
        "quantization": candidate.get("quantization"),
        "max_model_len": candidate.get("max_model_len"),
        "gpu_memory_utilization": deploy_gmu,
        "vram_budget_gb": budget,
        "estimated_vram_gb": total_gib,
        "tensor_parallel_size": tp,
        "safety_margin_threshold": gate_margin,
        "batch_source": lockset.get("batch_source"),
        "gmu_source": lockset.get("gmu_source"),
        "selection_timestamp": datetime.now().isoformat(),
        "actual_vram_gb": total_gib,
        "serve_success": True,
        "error_type": None,
        "tokens_per_sec": None,
        # Phase 2 신규 nullable 필드(실측 채움).
        "batch": candidate.get("batch"),
        "kv_cache_memory_bytes": candidate.get("kv_cache_memory_bytes"),
        "attention_backend": candidate.get("attention_backend"),
        "tool_call_parser": candidate.get("tool_call_parser"),
        "reasoning_parser": candidate.get("reasoning_parser"),
        "converged": True,
        "trial_count": len(correction_history) + 1,
        "correction_history": correction_history,
    }
    # dry-run/mock 은 실측이 아니므로 교차학습용 되먹임 로그를 오염시키지 않는다.
    if getattr(args, "dry_run", False) or getattr(args, "mock_profile", None):
        print("[recipe] (dry-run) 되먹임 로그 기록 생략 — 실측 아님.", file=sys.stderr)
    else:
        feedback_append(record, FEEDBACK_JSONL_PATH)
        print(f"[recipe] 되먹임 로그 기록: {FEEDBACK_JSONL_PATH}", file=sys.stderr)


def _simulate_hitl(run_dir, run_id, candidate, trial, final_class,
                   correction_history, cap, lockset_ctx=None):
    """미수렴 → Model-C HITL 보고(증거 simlog 경로 제시) + 비0 종료. lockset 은 각인하지 않는다(수렴 산물이 아니다)."""
    fclass = (final_class or {}).get("failure_class", "unknown")
    note = (final_class or {}).get("note", "")
    log_path = (trial or {}).get("log_path")

    # 실패 요약에도 provenance 를 각인한다(plan_26081314 D1) — "왜 실패했는가"의 해석이 출처에
    # 따라 완전히 달라지기 때문이다. mock 으로 낸 vram_infeasible 은 하드웨어 사실이 아니라
    # 주입값의 산술 결과일 뿐이므로, 그것을 실측 실패와 같은 얼굴로 남기면 오독을 부른다.
    _final_prov = (trial or {}).get("provenance")
    summary = {
        "run_id": run_id,
        "converged": False,
        "failure_class": fclass,
        "note": note,
        "trial_count": len(correction_history) + (1 if trial else 0),
        "candidate": candidate,
        "last_trial_log": log_path,
        "correction_history": correction_history,
        "provenance": _final_prov,
        "measured": _final_prov == PROVENANCE_MEASURED,
    }
    if lockset_ctx:
        summary["gmu_roles"] = _gmu_roles_record(lockset_ctx["roles"], trial, candidate)
        summary["batch_derivation"] = lockset_ctx.get("derivation")
    simlog_writer.write_summary(run_dir, summary)

    print("[recipe] ── Model-C (HITL) — 자동 수렴 실패 ──", file=sys.stderr)
    print(f"[recipe] failure_class={fclass} :: {note}", file=sys.stderr)
    if fclass == "vram_infeasible":
        print("[recipe] 구조적 VRAM 초과 — 예산 상향 / max_model_len·batch 하향 / "
              "KV quant(fp8) / weight quant 검토 필요.", file=sys.stderr)
    elif fclass == "unknown":
        print("[recipe] 알려진 시그니처 미매칭 — 로그를 사람이 분류해야 함.", file=sys.stderr)
    else:
        print(f"[recipe] cap({cap}) 소진 또는 폴백 소진 — 추가 조정은 사람이 판단.",
              file=sys.stderr)
    print(f"[recipe] 증거 simlog 경로: {run_dir}", file=sys.stderr)
    if log_path:
        print(f"[recipe]   마지막 트라이얼 로그: {log_path}", file=sys.stderr)
    print(f"[recipe]   요약: {os.path.join(run_dir, 'run_summary.json')}", file=sys.stderr)
    sys.exit(3)


def build_parser():
    p = argparse.ArgumentParser(
        prog="recipe.py",
        description="vllm-recipe-explorer 오케스트레이터 (estimate/generate/simulate)",
    )
    sub = p.add_subparsers(dest="command", required=True)

    pe = sub.add_parser("estimate", help="parse→후보→rank→report (+.last_ranking 저장)")
    pe.add_argument("--config", required=True, help="입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**(plan_26090616): 기본값이 `config.yaml` 이었기 때문에 캠페인이 셀마다 만든 변형이 저장소 루트에 쌓였고 21개가 공개 원격까지 추적 누출됐다. 자리는 `campaign_init.py --derive config --cell <cell-id>` 로 파생하라.")
    src = pe.add_mutually_exclusive_group(required=True)
    src.add_argument("--auto", action="store_true", help="결정론 기본 그리드 후보 사용")
    src.add_argument("--candidates", help="LLM 생성 후보 JSON(list) 경로")
    pe.set_defaults(func=cmd_estimate)

    pg = sub.add_parser("generate", help=".last_ranking 의 rN → 3종 세트 + 되먹임 로그")
    pg.add_argument("--config", required=True, help="입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**(plan_26090616): 기본값이 `config.yaml` 이었기 때문에 캠페인이 셀마다 만든 변형이 저장소 루트에 쌓였고 21개가 공개 원격까지 추적 누출됐다. 자리는 `campaign_init.py --derive config --cell <cell-id>` 로 파생하라.")
    pg.add_argument("--recipe-id", required=True, help="선택 레시피 id(예: r3)")
    pg.add_argument("--force", action="store_true", help="기존 파일 덮어쓰기 허용")
    pg.add_argument("--image", default=None,
                    help="컨테이너 이미지 태그 → env 의 IMAGE_TAG. 생략하면 env 에 "
                         "미지정 표시가 박히고 경고가 나간다(compose 는 비면 낡은 기본값으로 "
                         "조용히 폴백한다 — D8).")
    pg.set_defaults(func=cmd_generate)

    # Phase 2 — simulate(통합 trial-loop: run_trial→sim_classify→조정→반복).
    ps = sub.add_parser(
        "simulate",
        help="실서빙 trial-loop(절대 KV 클램프 수렴) → 3종 세트 + simlog + 되먹임",
    )
    ps.add_argument("--config", required=True, help="입력 config.yaml 경로. **필수다 — 루트 기본값은 2026-09-06 에 제거했다**(plan_26090616): 기본값이 `config.yaml` 이었기 때문에 캠페인이 셀마다 만든 변형이 저장소 루트에 쌓였고 21개가 공개 원격까지 추적 누출됐다. 자리는 `campaign_init.py --derive config --cell <cell-id>` 로 파생하라.")
    ps.add_argument("--candidate", required=True, help="lock-set 후보 JSON 경로")
    ps.add_argument("--dry-run", action="store_true", help="docker 없이 mock_profile 로 배선 검증")
    ps.add_argument("--mock-profile", default=None, help="dry-run 시 사용할 vllm_profile+functional JSON 경로")
    ps.add_argument("--run-id", default=None, help="simlog run_id(미지정 시 <YYYYMMDDHH>_1_<topic> 자동)")
    ps.add_argument("--topic", default=None, help="run_id 자동생성 시 주제(미지정 시 model_id)")
    ps.add_argument("--cap", type=int, default=DEFAULT_TRIAL_CAP, help="reconciliation_cap(기본 3)")
    ps.add_argument("--image", default=DEFAULT_TRIAL_IMAGE, help="run_trial docker 이미지")
    ps.add_argument("--timeout", type=int, default=900, help="/health 폴링 타임아웃(초)")
    ps.add_argument("--overhead-mib", type=int, default=None,
                    help="예산 선언 overhead(MiB). 미지정 시 config.budget_overhead_mib → "
                         "run_trial 기본값 순. 실측값이 있으면 넘긴다")
    ps.add_argument("--force", action="store_true", help="수렴 시 3종 세트 덮어쓰기 허용")
    ps.add_argument("--lockset-out", default=None,
                    help="수렴 시 기계 각인한 lockset(provenance=explorer-phase2 · batch_source·gmu_source·kv_source "
                         "· trial_provenance)을 쓸 경로. 캠페인 셀이면 `campaign_init.py --derive lockset --cell <id>` "
                         "로 파생한 셀 lockset(보통 --candidate 와 같은 파일)을 준다. 다른 기존 파일은 --force 로만 "
                         "덮어쓴다. 미지정이면: --candidate 가 캠페인 셀 lockset(campaigns/<id>/cells/<cell>/lockset.json)"
                         "이면 그 파일에 쓰고, 그 밖이면 run_summary.json 의 lockset 칸에만 남는다(plan_26091407 §4.2)")
    ps.set_defaults(func=cmd_simulate)

    return p


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    # 헌법 §테라포밍-완수 Flag 게이트 — deliverable 산출 서브커맨드는 Flag 전제(미발급 시 info-only·비0종료).
    # 2026-09-05(G-E1): 위임 키·env 면제 경로는 삭제됐다. 프로비저닝된 노드는 **정체성 증명**
    # (서명된 Agent Card)을 통과해야 하고, Flag 는 자기 manifest 로 정규 통과한다. fail-closed 백스톱.
    if getattr(args, "command", None) in ("estimate", "generate", "simulate"):
        _require_terraform_flag(REPO_ROOT)
    args.func(args)


if __name__ == "__main__":
    main()
