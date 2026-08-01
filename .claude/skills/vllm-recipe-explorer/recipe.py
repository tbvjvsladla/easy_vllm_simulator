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
from run_trial import run_trial  # noqa: E402
from sim_classify import classify as sim_classify  # noqa: E402
from preload_ram_gate import gate as preload_ram_gate  # noqa: E402
from estimate_vram import (  # noqa: E402
    GIB,
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
    gpus = man.get("gpus_per_node") or 1
    try:
        gpus = max(1, int(gpus))
    except (TypeError, ValueError):
        gpus = 1
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
    except Exception:
        return {}, mpath, topo


def _total_gpus(man):
    """가용 GPU 합 = gpus_per_node × 노드 수.
    **topology=single → 노드 배수 1**(sub 는 control 피어이지 텐서 워커가 아님 — resolve_tp 와 동일 계약)."""
    gpus = man.get("gpus_per_node") or 1
    try:
        gpus = max(1, int(gpus))
    except (TypeError, ValueError):
        gpus = 1
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
    이 함수는 (budget_gib, margin) 만 해소한다 — `/TP` 분할은 `_resolve_clamp_kv(tp_divisor=tp)` 가 적용.
    통합메모리 타겟은 target_gmu>0.90 을 0.90 으로 하드클램프+경고(§4.4). discrete 는 in-scope·하드클램프 없음.
    """
    tgt = cfg.get("target_gpu") or {}
    gpu_model = tgt.get("gpu_model")
    if not gpu_model:
        _die("config.target_gpu.gpu_model 누락(타겟 GPU 이식 경로엔 필수 — plan_26070809_47_07 §4.1)")
    target_gmu = float(tgt.get("target_gmu", 0.90))
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


def _require_terraform_flag(repo_root):
    """헌법 §테라포밍-완수/A2A-위임 Flag 게이트 (**fail-closed**) — 면제 없으면 info-only(작업 거부·비0종료). 강제 2층의 *결정론 백스톱*.

    면제 2경로(plan_26063021_14_37 §A2A-위임 Flag 따름정리):
      (1차·결정론) 서브 A2A 위임 *양성 키* `.claude/a2a_delegation.json` 존재 — 메인이 클러스터 HW 동질성 검증 후
                   발급·전달한 증표(메인 키 `terraforming.complete` 와 **UNIQUE**). *부재로 면제하는 fail-open ✗*.
      (2차·테스트) `EASY_VLLM_A2A_DELEGATED` env — 명시 override(개명: 옛 EASY_VLLM_SKIP_FLAG_GATE).
    그 외엔 메인 manifest Flag(complete·branch_verified·HW·model_source) 검사. (recipe = 서브 복제 런타임블럭 →
    main-only `manifest_contract.py` 미import, 자체 리더로 동일 계약.)
    """
    if os.environ.get("EASY_VLLM_A2A_DELEGATED") == "1":  # (2차) 명시 테스트 override (정확히 "1" — '0'/'false' 오인 차단)
        return
    keyp = os.path.join(repo_root, ".claude", "a2a_delegation.json")  # (1차) 서브 A2A 위임 양성 키
    if os.path.isfile(keyp):                          # 존재 + 내용·역할 검증(D8: 손상/외부 파일로 메인 게이트 우회 차단)
        try:
            with open(keyp, encoding="utf-8") as kf:
                kd = json.load(kf)
            if kd.get("delegation") == "main_cluster_flag" and kd.get("issued_to") == "sub":
                return
        except Exception:
            pass  # 손상/비유효 키 → 면제 안 함(fail-closed 진행)
    man, mpath, _topo = _read_manifest(repo_root)
    if not man:
        _die(
            "manifest 부재(%s) ∧ A2A 위임 키 부재 — 테라포밍 미완(fail-closed). terraforming_node 로 HW스캔 + "
            "모델획득 모드(managed|ephemeral|custom)를 먼저 정하세요(info-only). HW 사실 없이 서빙전략 deliverable 생성 ✗. "
            "[A2A/서브: 메인이 동질성 검증 후 .claude/a2a_delegation.json 발급 — 테스트는 EASY_VLLM_A2A_DELEGATED=1]" % mpath,
            code=4,
        )
    terra = man.get("terraforming") or {}
    if terra.get("complete") is not True or terra.get("branch_verified") is not True:
        _die(
            "테라포밍 완수 Flag 미발급(terraforming.complete/branch_verified != true) — "
            "terraforming_node 로 스캔·branch↔topology 3자일치 검증 완수 먼저(info-only).",
            code=4,
        )
    # manifest_contract.evaluate_contract 와 **동일 계약**(약한 게이트 금지 — 통합검증 BLOCK):
    # Flag 켜졌어도 필수 HW사실·획득모드 없으면 거부(scan 은 model_source 없이 complete 를 emit → 게이트가 집행).
    missing = [k for k in ("topology", "gpus_per_node") if not man.get(k)]
    if missing:
        _die("Flag true 이나 필수 HW필드 누락(%s) — terraforming_node 스캔 완수 먼저(info-only)." % ", ".join(missing), code=5)
    ms = man.get("model_source")
    if ms not in ("managed", "ephemeral", "custom"):
        _die("model_source 미설정/오류(%r) — terraforming_node 에서 획득모드(managed|ephemeral|custom) 지정 먼저(info-only)." % ms, code=5)


def _guard_tp(tp, repo_root):
    """가드(헌법 §manifest→서빙전략 배선 불변식): tp 가 가용 GPU 합 초과 시 비0종료(서빙 전 결정론 조기탐지)."""
    man, _, _ = _read_manifest(repo_root)
    tot = _total_gpus(man)
    if tp > tot:
        _die(
            "tp=%d 가 가용 GPU 합 %d 초과(GPU 초과) — config.tensor_parallel_size 또는 "
            "manifest(gpus_per_node·nodes) 확인." % (tp, tot),
            code=6,
        )


def _guard_kv_heads(parsed, tp):
    """가드: num_key_value_heads 가 tp 로 나눠떨어지지 않으면 비0종료(KV-head 비분할 — vLLM serve 즉사 조기탐지)."""
    kvh = parsed.get("num_key_value_heads")
    try:
        if kvh and tp and int(kvh) % int(tp) != 0:
            _die("num_key_value_heads=%s 가 tp=%d 로 나눠떨어지지 않음(KV-head 비분할) — tp 조정 필요." % (kvh, tp), code=6)
    except (TypeError, ValueError):
        pass


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
        "[recipe] ⚠ Phase-1(estimate→generate)은 near-max batch(max-num-seqs)·절대 KV 클램프(kv-cache-memory-bytes)를 "
        "emit하지 않는다(공식 per-token이 sliding-window/GQA에서 부정확 — 결함#4). near-max batch는 simulate(Phase-2) "
        "또는 serve 로그의 kv_cache_tokens/max_concurrency 측정으로만 산정하라(SKILL §5).",
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


def _kv_dtype_bytes(candidate):
    """KV dtype 바이트: kv_cache_quant=="fp8" 이면 1, 아니면 2(fp16)."""
    return 1 if candidate.get("kv_cache_quant") == "fp8" else 2


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


def _enrich_overhead(profile, device_total_gib, gmu_fallback=None):
    """consolidated 메모리 라인이 없는 vLLM 빌드 보강: non_kv_overhead 를 유도한다.

    vLLM 메모리식: gmu × device_total = weights + non_kv_overhead + kv_available.
    → non_kv_overhead = gmu_trial × device_total − weights − kv_available
       (cuda_graph 는 잔차에 포함 = 보수적). profile 에 이미 non_kv_overhead_gib 가
       있으면(consolidated 라인 보유) 건드리지 않는다. profile 을 제자리 보강해 반환.

    gmu_trial 은 우리가 --gpu-memory-utilization 으로 **설정한 알려진 입력**이다. 로그에서
    파싱(parse_vllm_log)이 vLLM 버전별 로그 포맷 차로 못 잡으면(예: 0.18.0) gmu_fallback
    (=candidate.gpu_memory_utilization)로 대체한다 — 로그 파싱에 의존하지 않는다.
    """
    if not isinstance(profile, dict):
        return profile
    if profile.get("non_kv_overhead_gib") is not None:
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


def _resolve_clamp_kv(parsed, candidate, profile, budget, margin, kv_dtype_bytes, tp_divisor=1):
    """실측 profile(weights/overhead) 로 절대 KV 클램프 = min(required, max_safe) 산정.

    OOM-critical 경로 — 순수 결정론(확률론 금지). 반환 dict:
      kv  : 산정된 클램프 바이트(성공). weights/overhead 실측 부재면 None(+fail None).
      fail: 구조적 불가(vram_infeasible) 시 final_class dict, 아니면 None.
      note: 산정 근거 문자열.

    tp_divisor(기본 1 — 회귀 0): >1 이면 타겟-GPU 이식 경로(plan_26070809_47_07 §4.2) — host 측정
    weights_total/overhead_total(GPU-불변 기하량)을 타겟 TP 로 나눠 per-GPU 클램프를 산정한다.
    """
    weights_b = _profile_bytes(profile, "weights_gib")
    overhead_b = _profile_bytes(profile, "non_kv_overhead_gib")
    if weights_b is None or overhead_b is None:
        return {"kv": None, "fail": None, "note": "weights/overhead 실측 부재."}
    if tp_divisor and tp_divisor > 1:
        weights_b = weights_b / tp_divisor
        overhead_b = overhead_b / tp_divisor
    max_len = int(candidate["max_model_len"])
    batch = int(candidate.get("batch", 1))
    # required_kv: 측정 트라이얼이 per-token KV(kv_available/kv_tokens)를 주면 그 측정값을 쓴다.
    # 하이브리드(GDN/linear-attention) 모델은 전 레이어가 full-KV 가 아니라 dims 공식이 과대추정한다
    # (Qwen3.6: 공식 262144 vs 실측 ~70600 B/token, 3.7×). 측정 per-token 이 정확. 없으면 공식 폴백.
    kv_gib_m = profile.get("kv_cache_gib")
    kv_tok_m = profile.get("kv_cache_tokens")
    if kv_gib_m and kv_tok_m:
        per_token = (float(kv_gib_m) * (1024 ** 3)) / float(kv_tok_m)
        required = int(per_token * max_len * batch)
    else:
        required = required_kv_bytes(parsed, max_len, batch, kv_dtype_bytes)
    max_safe = max_safe_kv_bytes(budget, margin, weights_b, overhead_b)
    if max_safe <= 0:
        return {"kv": None, "fail": {
            "failure_class": "vram_infeasible", "adjust_target": None,
            "note": "weights+overhead(실측) 만으로 천장 초과 → KV 조정 불가(HITL)."}, "note": ""}
    if required > max_safe:
        return {"kv": None, "fail": {
            "failure_class": "vram_infeasible", "adjust_target": None,
            "note": ("required_kv=%d > max_safe_kv=%d (실측 기반) → "
                     "max_model_len·batch 를 KV 로 만족 불가(HITL)." % (required, max_safe))},
            "note": ""}
    kv = int(min(required, max_safe))
    return {"kv": kv, "fail": None,
            "note": "kv=min(required=%d, max_safe=%d)=%d." % (required, max_safe, kv)}


def cmd_simulate(args):
    cfg = load_config(args.config)
    model_path, nas_root, budget, margin, kv_bytes_cfg, container_root = _cfg_common(cfg, REPO_ROOT)
    tp = resolve_tp(cfg, REPO_ROOT)
    _guard_tp(tp, REPO_ROOT)

    # ── 타겟-GPU 이식형 예산 (host≠target — plan_26070809_47_07 §4) ──
    # target_gpu 미정의 시 tp_divisor=1·budget/margin 무변경(기존 host 흐름 완전 보존).
    tp_divisor = 1
    if cfg.get("target_gpu"):
        ttp = _target_tp(cfg, REPO_ROOT)
        if ttp != tp:
            _die(
                "측정 TP(%d) ≠ 타겟 TP(%d, cards_per_node×node_count) — 1→N 외삽 금지(plan_26070809_47_07 §4.3). "
                "config.tensor_parallel_size 를 타겟에 맞추거나 manifest nodes[]/target_gpu.cards_per_node 를 "
                "정합시키세요." % (tp, ttp),
                code=6,
            )
        budget, margin, target_gpu_model = resolve_target_gpu_budget(cfg, tp)
        tp_divisor = tp
        print(
            "[recipe] 타겟-GPU 이식 경로 활성: gpu_model=%s per_card_vram_gib=%.2f target_gmu=%.2f TP=%d(÷%d)"
            % (target_gpu_model, budget, margin, tp, tp_divisor),
            file=sys.stderr,
        )

    # parse(결정론) — NAS 부재/해소 실패 시 traceback 금지(클린 어보트로 통일).
    try:
        parsed = parse(model_path, nas_host_root=nas_root, nas_container_root=container_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")

    candidate = _load_candidate(args.candidate)
    kv_dtype_bytes = _kv_dtype_bytes(candidate)

    # candidate 에 parse 산출 모델 식별자 + 서빙명을 주입(실 docker 가 올바른 모델을
    # 올바른 served-model-name 으로 서빙하도록 — 스모크가 이 이름으로 호출). setdefault=lockset 우선.
    candidate.setdefault("model_path_container", parsed.get("model_path_container"))
    candidate.setdefault("model_id", parsed.get("model_id"))
    _serving0 = cfg.get("serving") or {}
    if _serving0.get("served_model_name"):
        candidate.setdefault("served_model_name", _serving0.get("served_model_name"))
    # gpu-memory-utilization = safety_margin(디바이스 풀 상한). 통합메모리(GB10)서 vLLM
    # 기본 0.92 가 free 초과 OOM → margin 으로 명시(SKILL §5). 실 KV 는 절대 클램프가 제어.
    candidate.setdefault("gpu_memory_utilization", margin)

    # 측정 하드웨어 total(torch.cuda 기준; DGX Spark 는 nvidia-smi N/A). consolidated 메모리
    # 라인이 없는 vLLM 빌드에서 overhead = gmu_trial×device_total − weights − kv 로 유도하는 데 쓴다.
    device_total_gib = float(cfg.get("test_device_total_gib", 121.69))

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
        "tiktoken_host_path": cfg.get("tiktoken_host_path"),  # config → run_trial /encodings:ro 마운트(C8 에어갭 자산 배선)
        # JIT 캐시 통로 + 컴파일 팬아웃 캡 — serve 평면(docker-compose)과 parity.
        # 기본을 serve 와 **같은 디렉터리**로 잡아 trial 이 데운 캐시를 serve 가 그대로 쓴다
        # (반대도 성립). 통로 분리는 두 평면이 서로 cold 를 반복하게 만들 뿐이다.
        "jit_cache_root": cfg.get("jit_cache_root") or _default_jit_cache_root(),
        "max_jobs": cfg.get("max_jobs", 4),
    }
    opts = {k: v for k, v in opts.items() if v is not None}

    cap = int(args.cap)
    correction_history = []
    converged = False
    final_trial = None
    final_class = None

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
        # consolidated 메모리 라인이 없는 빌드 보강: overhead 유도(제자리). dry-run mock 이
        # 이미 non_kv_overhead 를 주면 건드리지 않는다.
        _enrich_overhead(trial.get("vllm_profile"), device_total_gib,
                         gmu_fallback=candidate.get("gpu_memory_utilization"))
        final_trial = trial

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
        verdict = sim_classify(trial, budget_gib=budget, safety_margin=margin)
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
            if candidate.get("kv_cache_memory_bytes") is None:
                res = _resolve_clamp_kv(
                    parsed, candidate, trial.get("vllm_profile") or {},
                    budget, margin, kv_dtype_bytes, tp_divisor=tp_divisor,
                )
                record = {
                    "trial_number": trial_number,
                    "failure_class": "measure",
                    "adjust_target": "kv_cache_memory_bytes",
                    "note": "측정 트라이얼 통과 → 절대 KV 클램프 산정 후 검증 트라이얼.",
                    "before": {"kv_cache_memory_bytes": None},
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
                record["after"] = {"kv_cache_memory_bytes": res["kv"]}
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
                budget, margin, kv_dtype_bytes, tp_divisor=tp_divisor,
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
            record["after"] = {"kv_cache_memory_bytes": int(new_kv)}

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

    # ── 수렴 처리 ──────────────────────────────────────────────────────────
    if converged:
        return _simulate_converged(
            args, cfg, parsed, candidate, final_trial, tp, budget, margin,
            run_dir, run_id, correction_history,
        )

    # ── 미수렴(cap 소진 / infeasible / unknown) → Model-C HITL 보고 ─────────
    _simulate_hitl(
        run_dir, run_id, candidate, final_trial, final_class, correction_history, cap,
    )


def _simulate_converged(args, cfg, parsed, candidate, trial, tp, budget, margin,
                        run_dir, run_id, correction_history):
    """수렴 시: 실측 채운 recipe → gen_recipe_set 3종 세트 + write_summary + feedback."""
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
    # gmu 분기 가시화: 트라이얼이 쓴 값과 배포에 박히는 값(safety_margin)이 다르면 알린다.
    _cand_gmu = candidate.get("gpu_memory_utilization")
    if _cand_gmu is not None and abs(float(_cand_gmu) - float(margin)) > 1e-9:
        print("[recipe] ⚠ gmu 분기: 트라이얼 %.3f ↔ 배포(config.safety_margin) %.3f — "
              "검증한 값과 배포되는 값이 다르다. 의도한 것이 아니면 config.safety_margin 을 맞춰라."
              % (float(_cand_gmu), float(margin)), file=sys.stderr)
    recipe = recipe_from_candidate(candidate, **{
        # gpu-memory-utilization = safety_margin(디바이스 풀 상한; 실제 KV 는 절대 클램프가 제어).
        "gpu_memory_utilization": margin,
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

    # simlog run_summary.
    summary = {
        "run_id": run_id,
        "converged": True,
        "trial_count": len(correction_history) + 1,
        "candidate": candidate,
        "vram_breakdown": recipe["vram_breakdown"],
        "generated_paths": paths,
        "correction_history": correction_history,
    }
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
        "gpu_memory_utilization": margin,
        "vram_budget_gb": budget,
        "estimated_vram_gb": total_gib,
        "tensor_parallel_size": tp,
        "safety_margin_threshold": margin,
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
                   correction_history, cap):
    """미수렴 → Model-C HITL 보고(증거 simlog 경로 제시) + 비0 종료."""
    fclass = (final_class or {}).get("failure_class", "unknown")
    note = (final_class or {}).get("note", "")
    log_path = (trial or {}).get("log_path")

    summary = {
        "run_id": run_id,
        "converged": False,
        "failure_class": fclass,
        "note": note,
        "trial_count": len(correction_history) + (1 if trial else 0),
        "candidate": candidate,
        "last_trial_log": log_path,
        "correction_history": correction_history,
    }
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
    pe.add_argument("--config", default="config.yaml", help="입력 config.yaml 경로")
    src = pe.add_mutually_exclusive_group(required=True)
    src.add_argument("--auto", action="store_true", help="결정론 기본 그리드 후보 사용")
    src.add_argument("--candidates", help="LLM 생성 후보 JSON(list) 경로")
    pe.set_defaults(func=cmd_estimate)

    pg = sub.add_parser("generate", help=".last_ranking 의 rN → 3종 세트 + 되먹임 로그")
    pg.add_argument("--config", default="config.yaml", help="입력 config.yaml 경로")
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
    ps.add_argument("--config", default="config.yaml", help="입력 config.yaml 경로")
    ps.add_argument("--candidate", required=True, help="lock-set 후보 JSON 경로")
    ps.add_argument("--dry-run", action="store_true", help="docker 없이 mock_profile 로 배선 검증")
    ps.add_argument("--mock-profile", default=None, help="dry-run 시 사용할 vllm_profile+functional JSON 경로")
    ps.add_argument("--run-id", default=None, help="simlog run_id(미지정 시 <YYYYMMDDHH>_1_<topic> 자동)")
    ps.add_argument("--topic", default=None, help="run_id 자동생성 시 주제(미지정 시 model_id)")
    ps.add_argument("--cap", type=int, default=DEFAULT_TRIAL_CAP, help="reconciliation_cap(기본 3)")
    ps.add_argument("--image", default=DEFAULT_TRIAL_IMAGE, help="run_trial docker 이미지")
    ps.add_argument("--timeout", type=int, default=900, help="/health 폴링 타임아웃(초)")
    ps.add_argument("--force", action="store_true", help="수렴 시 3종 세트 덮어쓰기 허용")
    ps.set_defaults(func=cmd_simulate)

    return p


def main(argv=None):
    parser = build_parser()
    args = parser.parse_args(argv)
    # 헌법 §테라포밍-완수 Flag 게이트 — deliverable 산출 서브커맨드는 Flag 전제(미발급 시 info-only·비0종료).
    # 면제 2경로: .claude/a2a_delegation.json(서브 A2A 위임 양성키·1차) 또는 EASY_VLLM_A2A_DELEGATED(테스트 override·2차). fail-closed 결정론 백스톱.
    if getattr(args, "command", None) in ("estimate", "generate", "simulate"):
        _require_terraform_flag(REPO_ROOT)
    args.func(args)


if __name__ == "__main__":
    main()
