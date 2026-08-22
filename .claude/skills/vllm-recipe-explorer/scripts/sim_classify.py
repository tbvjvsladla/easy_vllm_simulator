#!/usr/bin/env python3
"""sim_classify.py — 트라이얼 결과 결정론 분류기 (vllm-recipe-explorer Phase 2).

CONTRACT(FROZEN) sim_classify.py 절 준수. classify(trial_result, budget_gib, safety_margin)->dict.

run_trial.py 의 trial_result(load_ok·vllm_profile·functional·error_excerpt)를 입력으로 받아
실측값 + 알려진 OOM 정규식으로 실패를 결정론 분류하고 다음 조정 대상을 정한다.
  vram_oom        → load 실패 + OOM 시그니처(adjust kv_cache_memory_bytes)
  vram_infeasible → load OK 인데 weights+overhead 만으로 천장 초과(adjust null, HITL)
  functional      → load OK·VRAM OK·functional.passed False(adjust= 실패한 soft 변수)
  none            → 모두 OK
  unknown         → 그 외(Model-C, adjust null)

classify_failure.py 의 정규식 1차 분류 + estimate_vram.py 의 GIB/판정 스타일을 따른다.
stdlib(json,sys,argparse,re) 만 사용. 외부 네트워크 호출 없음.
"""

from __future__ import annotations

import argparse
import json
import re
import sys

GIB = 1024 ** 3  # 단위 통일: GiB = 1024**3 (estimate_vram.py 와 동일)

# 알려진 OOM 시그니처(CONTRACT sim_classify 절 명시). load 실패 시 error_excerpt 에서 탐색.
OOM_SIGNATURES = [
    r"CUDA out of memory",
    r"No available memory for the cache blocks",
    r"ValueError: To serve at least one request",
]


def _profile_gib(profile: dict, key: str):
    """vllm_profile 에서 GiB 실측값을 float 로 반환(없으면 None)."""
    if not profile:
        return None
    v = profile.get(key)
    return float(v) if v is not None else None


def classify(trial_result: dict, budget_gib: float, safety_margin: float) -> dict:
    """단일 트라이얼 결과의 실패 결정론 분류 + 조정 대상 선정.

    trial_result = run_trial.py 출력
        {load_ok, vllm_profile, functional, error_excerpt, ...}.

    반환: {failure_class, adjust_target, note}.
      failure_class ∈ none|vram_oom|vram_infeasible|functional|unknown.
      adjust_target ∈ kv_cache_memory_bytes|attention_backend|tool_call_parser|reasoning_parser|null.
    """
    result: dict = {
        "failure_class": "unknown",
        "adjust_target": None,
        "note": "",
    }

    load_ok = bool(trial_result.get("load_ok"))
    profile = trial_result.get("vllm_profile") or {}
    functional = trial_result.get("functional") or {}
    error_excerpt = trial_result.get("error_excerpt") or ""
    # 준비 대기의 종단 사유(run_trial 이 생존검사로 남긴다 · plan_26082223 결함 C).
    #   ⚠ 이 신호는 **분류를 바꾸지 않는다** — 분류 권위는 여전히 로그(시그니처·kv_cache_gib·
    #     예외 유무)다. 컨테이너가 죽었다는 사실만으로는 *왜* 죽었는지를 모르기 때문이다
    #     (워치독 SIGKILL 인지 엔진 크래시인지). 여기서는 note 에만 실어 사후분석이 timeout
    #     소진과 사망을 구분할 수 있게 한다. 가드를 넓히려면 별도 증거가 필요하다.
    health_wait = trial_result.get("health_wait") or {}
    wait_outcome = health_wait.get("outcome")
    wait_note = ""
    if wait_outcome == "container_died":
        wait_note = (" [준비대기 종단=container_died(%s, %.0fs 경과 — 타임아웃 소진 아님)]"
                     % (health_wait.get("container_state"), health_wait.get("waited_s") or 0.0))
    elif wait_outcome == "timeout":
        wait_note = (" [준비대기 종단=timeout(%.0fs 전량 소진 — 컨테이너는 살아 있었다)]"
                     % (health_wait.get("waited_s") or 0.0))

    # ── load 실패 ──────────────────────────────────────────────────────
    # OOM 시그니처가 잡히면 vram_oom(KV 바이트 줄여 재시도). 그 외 load 실패는 unknown(HITL).
    if not load_ok:
        for sig in OOM_SIGNATURES:
            m = re.search(sig, error_excerpt)
            if m:
                result["failure_class"] = "vram_oom"
                result["adjust_target"] = "kv_cache_memory_bytes"
                result["note"] = (
                    f"OOM 시그니처 매칭('{sig}') → kv_cache_memory_bytes 축소 후 재시도."
                    + wait_note
                )
                return result

        # ── 무예외 외부종료(호스트 워치독 SIGKILL 계열) ────────────────────
        # OOM_SIGNATURES 는 전부 **예외 텍스트**를 전제한다. 그런데 통합메모리 호스트에서
        # KV 벌룬이 나면 호스트 워치독이 컨테이너를 SIGKILL 하고, 그때는 예외가 **없다**
        # (프로세스 즉사 → 로그가 중간에 끊김). 따라서 이 실패 양식은 시그니처로 잡을 수 없다.
        #
        # 판별: load 실패 + OOM 시그니처 미매칭 + **vllm_profile 에 kv_cache_gib 존재**.
        # kv_cache_gib 가 있다는 건 엔진이 KV 사이징까지 성공했다는 뜻이고, CUDA OOM 이었다면
        # 예외를 남겼을 것이므로 이 조합은 CUDA OOM 일 수 없다 → 외부 종료로 확정한다.
        #
        # failure_class 는 계약상 5종 고정이라 새 클래스를 만들지 않는다. 조정 대상은
        # kv_cache_memory_bytes 로 동일하므로 vram_oom 으로 두되 note 로 기전을 구분한다.
        # 근거: 2026-07-31 워치독 실화(KV 97.03 GiB · 하강 27.9 GiB/s · testlog_26073116).
        # ★ 선행 가드: **예외를 남기고 죽었으면 외부 종료가 아니다.**
        #   SIGKILL 은 프로세스를 즉사시키므로 로그가 중간에 끊기고 traceback 이 없다.
        #   반대로 traceback/ERROR 가 있으면 엔진이 스스로 실패를 보고한 것이며, 그것이
        #   OOM 시그니처와 안 맞는다면 **미지 실패(Model-C)** 이지 워치독 킬이 아니다.
        #   이 가드 없이는 예컨대 cudagraph 캡처 중 Triton 커널 컴파일 실패가
        #   "워치독 SIGKILL → KV 축소" 로 오진되고, 이어서 vram_infeasible 로 승격돼
        #   "예산 상향/KV quant/weight quant" 라는 **완전히 무관한 처방**이 나간다.
        #   (2026-08-01 gpt-oss-120b 실측: KV 451,428 토큰 정상 할당 · free 113.54 GiB 상태에서
        #    capture_model() 의 mxfp4 TRITON 커널이 shape 불일치로 컴파일 실패했다.)
        _EXC = (r"Traceback \(most recent call last\)",
                r"\bERROR\b.*(Error|Exception)",
                r"^\s*\w*(Error|Exception):",
                r"CompilationError",
                r"Engine core initialization failed")
        has_exception = any(re.search(p, error_excerpt, re.MULTILINE) for p in _EXC)

        kv_seen = _profile_gib(profile, "kv_cache_gib")
        if kv_seen is not None and not has_exception:
            result["failure_class"] = "vram_oom"
            result["adjust_target"] = "kv_cache_memory_bytes"
            # 위험 판정은 KV 단독이 아니라 **KV + 가중치**로 한다 — 통합메모리에서 호스트를
            # 압박하는 건 상주 총량이다(실화: KV 97.03 + weights 9.84 ≈ 107 GiB / 안전예산 109.5).
            w_seen = _profile_gib(profile, "weights_gib")
            over = ""
            if budget_gib:
                safe = float(budget_gib) * float(safety_margin or 1.0)
                resident = kv_seen + (w_seen or 0.0)
                basis = "KV+가중치" if w_seen is not None else "KV(가중치 실측 부재)"
                over = (f" 상주 {basis} {resident:.2f} GiB / 안전예산 {safe:.1f} GiB"
                        f" ({100 * resident / safe:.0f}%).")
            result["note"] = (
                f"load 실패 + OOM 예외 없음 + vllm_profile.kv_cache_gib={kv_seen:.2f} GiB 존재 "
                f"→ 엔진이 KV 를 잡은 뒤 **외부에서 종료**됨(호스트 워치독 SIGKILL 계열)."
                f"{over} CUDA OOM 은 예외를 남기므로 이 조합은 CUDA OOM 이 아니다. "
                "kv_cache_memory_bytes 축소 후 재시도." + wait_note
            )
            return result

        result["failure_class"] = "unknown"
        result["adjust_target"] = None
        log_path = trial_result.get("log_path")
        why = ""
        if has_exception:
            # 여기 왔다는 건 "예외는 있는데 OOM 이 아니다" 라는 뜻이다. 그 사실을 명시해야
            # 사람이 메모리 축을 뒤지느라 시간을 버리지 않는다.
            why = ("엔진이 **예외를 남기고** 실패했다(외부 종료 아님) — 메모리 축이 아닐 가능성이 높다. "
                   "로그의 traceback 을 직접 읽어라. ")
            if kv_seen is not None:
                why += (f"참고: KV {kv_seen:.2f} GiB 는 **정상 할당된 뒤** 실패했다 — "
                        "KV 축소는 이 실패와 무관할 수 있다. ")
        result["note"] = (
            "load 실패이나 알려진 OOM 시그니처 미매칭 → Model-C(HITL). "
            + why
            + "시그니처 미매칭 = 미지 실패이지 불가 아님"
            + (f" (raw log: {log_path})" if log_path else "")
            + wait_note
        )
        return result

    # ── load OK: VRAM 천장 점검 ────────────────────────────────────────
    # weights+overhead 만으로 디바이스 풀 상한(budget*safety_margin)을 넘으면 KV 0 으로도
    # 띄울 수 없음 → vram_infeasible(KV 조정으로 못 푸는 구조적 초과, adjust null, HITL).
    weights_gib = _profile_gib(profile, "weights_gib")
    overhead_gib = _profile_gib(profile, "non_kv_overhead_gib")
    ceiling_gib = budget_gib * safety_margin
    if weights_gib is not None and overhead_gib is not None:
        if (weights_gib + overhead_gib) > ceiling_gib:
            result["failure_class"] = "vram_infeasible"
            result["adjust_target"] = None
            result["note"] = (
                f"weights+overhead={weights_gib + overhead_gib:.2f}GiB > "
                f"ceiling={ceiling_gib:.2f}GiB(budget*margin) → KV 조정 불가 구조적 초과(HITL)."
            )
            return result

    # ── load OK·VRAM OK: 기능 점검 ─────────────────────────────────────
    # functional.passed False 면 실패한 soft 변수를 조정 대상으로 지정.
    # 우선순위: attention_backend → tool_call_parser → reasoning_parser.
    if functional and functional.get("passed") is False:
        result["failure_class"] = "functional"
        if functional.get("completion_ok") is False:
            result["adjust_target"] = "attention_backend"
            result["note"] = "completion 실패 → attention_backend 폴백 후보로 조정."
        elif functional.get("tool_call_ok") is False:
            result["adjust_target"] = "tool_call_parser"
            result["note"] = "tool_call 실패 → tool_call_parser 폴백 후보로 조정."
        elif functional.get("reasoning_ok") is False:
            result["adjust_target"] = "reasoning_parser"
            result["note"] = "reasoning 실패 → reasoning_parser 폴백 후보로 조정."
        else:
            result["adjust_target"] = "attention_backend"
            result["note"] = "functional 실패(세부 미상) → attention_backend 폴백 후보로 조정."
        return result

    # ── 모두 OK ────────────────────────────────────────────────────────
    if functional and functional.get("passed") is True:
        result["failure_class"] = "none"
        result["adjust_target"] = None
        result["note"] = "load OK·VRAM OK·functional 통과 → 수렴."
        return result

    # ── 그 외(functional 결과 없음 등) → unknown(HITL) ─────────────────
    result["failure_class"] = "unknown"
    result["adjust_target"] = None
    result["note"] = "load OK·VRAM OK 이나 functional 판정 불가 → Model-C(HITL)."
    return result


def _main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(
        description="트라이얼 결과 결정론 분류기 (vllm-recipe-explorer Phase 2)."
    )
    p.add_argument("--trial", required=True, help="run_trial 출력 JSON 경로")
    p.add_argument("--budget", type=float, required=True, help="vram_budget_gib")
    p.add_argument("--margin", type=float, required=True, help="safety_margin (디바이스 풀 상한 비율)")
    args = p.parse_args(argv)

    with open(args.trial, "r", encoding="utf-8") as f:
        trial_result = json.load(f)

    result = classify(trial_result, budget_gib=args.budget, safety_margin=args.margin)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(_main())
