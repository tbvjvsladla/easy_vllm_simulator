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
import sys

import yaml

# --- scripts/ 를 import 경로에 추가 후 함수 import 조립 (CONTRACT) ---
SKILL_ROOT = os.path.dirname(os.path.abspath(__file__))
SCRIPTS_DIR = os.path.join(SKILL_ROOT, "scripts")
sys.path.insert(0, SCRIPTS_DIR)

from parse_model_config import parse  # noqa: E402
from rank_recipes import auto_candidates, rank, render_report  # noqa: E402
from gen_recipe_set import generate  # noqa: E402
from feedback_log import append as feedback_append  # noqa: E402

# Phase 2 시뮬레이터(통합 trial-loop) 조립용 import.
from run_trial import run_trial  # noqa: E402
from sim_classify import classify as sim_classify  # noqa: E402
from estimate_vram import (  # noqa: E402
    GIB,
    required_kv_bytes,
    max_safe_kv_bytes,
)
import simlog_writer  # noqa: E402

# --- 영속 경로 (SKILL_ROOT 기준 동적 도출 — 이식 가능, 하드코딩 금지) ---
REPO_ROOT = os.path.abspath(os.path.join(SKILL_ROOT, os.pardir, os.pardir, os.pardir))
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
    """tp 자동결정.

    CONTRACT: config 의 tensor_parallel_size 가 있으면 우선. 없으면
    git -C <repo> rev-parse --abbrev-ref HEAD → single-node→1, multi-node→2, 기타→1.
    """
    explicit = cfg.get("tensor_parallel_size")
    if explicit is not None:
        return int(explicit)
    branch = None
    try:
        import subprocess

        out = subprocess.run(
            ["git", "-C", repo_root, "rev-parse", "--abbrev-ref", "HEAD"],
            capture_output=True,
            text=True,
        )
        if out.returncode == 0:
            branch = out.stdout.strip()
    except Exception:
        branch = None
    if branch == "single-node":
        return 1
    if branch == "multi-node":
        return 2
    return 1


def _cfg_common(cfg):
    """estimate/generate 공통 입력값 추출(스키마 결함은 즉시 중단)."""
    target = cfg.get("target_model") or {}
    model_path = target.get("path")
    if not model_path:
        _die("config.target_model.path 누락")
    nas_root = cfg.get("nas_host_root", DEFAULT_NAS_HOST_ROOT)
    budget = cfg.get("vram_budget_gb")
    if budget is None:
        _die("config.vram_budget_gb 누락")
    budget = float(budget)
    margin = float(cfg.get("safety_margin", DEFAULT_SAFETY_MARGIN))
    kv_bytes = int(cfg.get("kv_cache_dtype_bytes", DEFAULT_KV_BYTES))
    return model_path, nas_root, budget, margin, kv_bytes


def cmd_estimate(args):
    cfg = load_config(args.config)
    model_path, nas_root, budget, margin, kv_bytes = _cfg_common(cfg)
    tp = resolve_tp(cfg, REPO_ROOT)

    # ① parse(결정론) — NAS 부재/디렉토리·config.json 부재 → traceback 금지,
    # config 오류와 동일한 [recipe] 중단: 클린 어보트로 통일(model_config_unparseable).
    try:
        parsed = parse(model_path, nas_host_root=nas_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")

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
    if port is None:
        _die("config.serving.port 누락")
    if not served_model_name:
        _die("config.serving.served_model_name 누락")
    port = int(port)

    # parse 결과(컨테이너 경로 등)는 generate 가 사용 → 다시 parse 하여 재현성 확보.
    model_path, nas_root, budget, margin, kv_bytes = _cfg_common(cfg)
    tp = snapshot.get("tp", resolve_tp(cfg, REPO_ROOT))
    # parse 재실행 — NAS 부재/해소 실패 시 traceback 금지(클린 어보트로 통일).
    try:
        parsed = parse(model_path, nas_host_root=nas_root)
    except FileNotFoundError as e:
        _die(str(e))
    except ValueError as e:
        _die(f"모델 config 해소 실패: {e}")

    # 결정론 계층 경고(estimate 가 단 것) 노출 — 특히 offline-quant on non-prequantized.
    recipe_warning = recipe.get("warning")
    if recipe_warning == "offline_quant_on_non_prequantized_checkpoint":
        print(
            "[recipe] 경고: %s 는 비prequantized 체크포인트에 오프라인 전용 quant(%s) — "
            "`vllm serve` 시 사전양자화 가중치 부재로 서빙 실패 가능. "
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
            REPO_ROOT,
            port,
            served_model_name,
            force=args.force,
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
        cand = json.load(f)
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


def _enrich_overhead(profile, device_total_gib):
    """consolidated 메모리 라인이 없는 vLLM 빌드 보강: non_kv_overhead 를 유도한다.

    vLLM 메모리식: gmu × device_total = weights + non_kv_overhead + kv_available.
    → non_kv_overhead = gmu_trial × device_total − weights − kv_available
       (cuda_graph 는 잔차에 포함 = 보수적). profile 에 이미 non_kv_overhead_gib 가
       있으면(consolidated 라인 보유) 건드리지 않는다. profile 을 제자리 보강해 반환.
    """
    if not isinstance(profile, dict):
        return profile
    if profile.get("non_kv_overhead_gib") is not None:
        return profile
    gmu = profile.get("gmu_trial")
    w = profile.get("weights_gib")
    kv = profile.get("kv_cache_gib")
    if None in (gmu, w, kv) or not device_total_gib:
        return profile
    overhead = gmu * float(device_total_gib) - w - kv
    if overhead > 0:
        profile["non_kv_overhead_gib"] = overhead
        profile["device_total_gib"] = float(device_total_gib)
    return profile


def _resolve_clamp_kv(parsed, candidate, profile, budget, margin, kv_dtype_bytes):
    """실측 profile(weights/overhead) 로 절대 KV 클램프 = min(required, max_safe) 산정.

    OOM-critical 경로 — 순수 결정론(확률론 금지). 반환 dict:
      kv  : 산정된 클램프 바이트(성공). weights/overhead 실측 부재면 None(+fail None).
      fail: 구조적 불가(vram_infeasible) 시 final_class dict, 아니면 None.
      note: 산정 근거 문자열.
    """
    weights_b = _profile_bytes(profile, "weights_gib")
    overhead_b = _profile_bytes(profile, "non_kv_overhead_gib")
    if weights_b is None or overhead_b is None:
        return {"kv": None, "fail": None, "note": "weights/overhead 실측 부재."}
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
    model_path, nas_root, budget, margin, kv_bytes_cfg = _cfg_common(cfg)
    tp = resolve_tp(cfg, REPO_ROOT)

    # parse(결정론) — NAS 부재/해소 실패 시 traceback 금지(클린 어보트로 통일).
    try:
        parsed = parse(model_path, nas_host_root=nas_root)
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

        # ── 실서빙(또는 dry-run/mock) 트라이얼 ──────────────────────────────
        trial = run_trial(candidate, run_dir, trial_number, opts)
        # consolidated 메모리 라인이 없는 빌드 보강: overhead 유도(제자리). dry-run mock 이
        # 이미 non_kv_overhead 를 주면 건드리지 않는다.
        _enrich_overhead(trial.get("vllm_profile"), device_total_gib)
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
                    budget, margin, kv_dtype_bytes,
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
                budget, margin, kv_dtype_bytes,
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
    if port is None:
        _die("config.serving.port 누락")
    if not served_model_name:
        _die("config.serving.served_model_name 누락")
    port = int(port)

    # recipe dict: gen_recipe_set 가 읽는 키(quantization/max_model_len/gpu_memory_utilization
    # + Phase2 batch/kv_cache_memory_bytes/kv_cache_dtype/소프트 변수/vram_breakdown).
    recipe = {
        "id": candidate.get("id"),
        "quantization": candidate.get("quantization"),
        "max_model_len": candidate.get("max_model_len"),
        # gpu-memory-utilization = safety_margin(디바이스 풀 상한; 실제 KV 는 절대 클램프가 제어).
        "gpu_memory_utilization": margin,
        "batch": candidate.get("batch"),
        "kv_cache_memory_bytes": candidate.get("kv_cache_memory_bytes"),
        "kv_cache_quant": candidate.get("kv_cache_quant"),
        "attention_backend": candidate.get("attention_backend"),
        "tool_call_parser": candidate.get("tool_call_parser"),
        "reasoning_parser": candidate.get("reasoning_parser"),
        "vram_breakdown": {
            "weights_gib": weights_gib,
            "kv_gib": kv_gib,
            "overhead_gib": overhead_gib,
            "total_gib": total_gib,
            "budget_gib": budget,
            "headroom_gib": (budget - total_gib) if total_gib is not None else None,
        },
    }

    try:
        paths = generate(
            parsed, recipe, name, REPO_ROOT, port, served_model_name, force=args.force,
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
    args.func(args)


if __name__ == "__main__":
    main()
