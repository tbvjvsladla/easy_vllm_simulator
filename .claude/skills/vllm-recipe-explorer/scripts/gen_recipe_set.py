#!/usr/bin/env python3
"""gen_recipe_set.py — DGX Spark 서빙용 3종 세트(.yaml + .sh + .env) 결정론 생성기.

vllm-recipe-explorer 스킬의 파이프라인 ④ generate 단계.
HITL 로 선택된 레시피(recipe dict)와 파싱 결과(parsed dict)를 받아
configs/<name>.yaml · configs/<name>.sh · envs/.env.<name> 3종을 생성한다.

스키마는 기존 산출물(configs/gpt-oss-20b-normal.{yaml,sh}, envs/.env.gpt-oss-20b-normal)을
그대로 모사한다(표류 금지). quantization 줄은 native/none 이 아닐 때만 추가.
기존 파일이 있으면 force 없이는 덮어쓰지 않고 에러(안전).

importable 함수 `generate(...)` + __main__ CLI 둘 다 제공.
stdlib 만 사용(os, sys, json, argparse).
"""

import os
import sys
import json
import argparse


# native/none 으로 간주하는 quant 값(이 경우 configs yaml 에 quantization 줄을 넣지 않음).
# vLLM 의 native dtype 서빙(=양자화 없음)을 의미한다.
_NATIVE_QUANT_VALUES = {None, "", "none", "null", "native", "bf16", "fp16",
                        "float16", "bfloat16", "float32", "fp32"}


def _is_native_or_none(quant):
    """quantization 값이 native/none 인지 판정(소문자 정규화)."""
    if quant is None:
        return True
    return str(quant).strip().lower() in _NATIVE_QUANT_VALUES


def _build_vram_breakdown_block(breakdown):
    """vram_breakdown dict 를 상단 주석 블록 라인 리스트로 변환.

    Phase 2 VRAM 분해(weights/kv/overhead/total GiB, budget, headroom)를
    yaml 상단에 사람이 읽을 수 있는 주석으로 기재한다. None 이면 빈 리스트.
    키가 없는 항목은 생략(결정론적·견고).
    """
    if not breakdown:
        return []
    lines = ["# ── VRAM 분해 (Phase 2 절대 KV 클램프 기준) ──"]
    order = [
        ("weights_gib", "weights"),
        ("kv_gib", "kv_cache"),
        ("overhead_gib", "overhead"),
        ("total_gib", "total"),
        ("budget_gib", "budget"),
        ("headroom_gib", "headroom"),
    ]
    for key, label in order:
        val = breakdown.get(key)
        if val is not None:
            lines.append("#   {}: {} GiB".format(label, val))
    return lines


def _build_yaml(parsed, recipe, served_model_name, port=8000):
    """configs/<name>.yaml 내용 문자열 생성.

    기존 configs/gpt-oss-20b-normal.yaml 스키마 준수:
      model / host / port / gpu-memory-utilization / max-model-len
    quantization 줄은 native/none 이 아닐 때만 추가.

    Phase 2 확장(recipe 에 해당 키가 있을 때만 반영):
      max-num-seqs(batch) / kv-cache-memory-bytes / kv-cache-dtype,
      gpu-memory-utilization 은 safety_margin(디바이스 풀 상한),
      상단 vram_breakdown 주석 블록.
    """
    container_path = parsed.get("model_path_container")
    model_id = parsed.get("model_id", "")

    quant = recipe.get("quantization")
    gmu = recipe.get("gpu_memory_utilization")
    max_model_len = recipe.get("max_model_len")

    # Phase 2 신규 키(없으면 None → 해당 줄 생략).
    batch = recipe.get("batch")
    kv_bytes = recipe.get("kv_cache_memory_bytes")
    kv_quant = recipe.get("kv_cache_quant")
    breakdown = recipe.get("vram_breakdown")

    lines = []
    lines.append("# {} 서빙 설정 (vllm-recipe-explorer 생성)".format(model_id))
    lines.append("# 레시피: quant={} max_model_len={} gpu_mem_util={}".format(
        quant, max_model_len, gmu))
    # VRAM 분해 주석 블록(vram_breakdown 있을 때만).
    lines.extend(_build_vram_breakdown_block(breakdown))
    # 타겟-GPU 이식 정직성 주석(recipe.target_gpu 있을 때만 — §4.9, plan_26070809_47_07).
    target_gpu = recipe.get("target_gpu")
    if target_gpu:
        lines.append("# ── 타겟-GPU 이식 클램프 (host≠target, gpu_model={}) ──".format(
            target_gpu.get("gpu_model")))
        lines.append("# host-측정 weights/overhead 이전값. target≠host arch 면 target-margin 이 쿠션(정량보증 아님).")
        lines.append("# 가능하면 타겟에서 재측정(Phase-2)을 권장. per-token-KV·weights 는 GPU-불변, overhead 는 런타임 성질 일부 포함.")
    lines.append("model: {}".format(container_path))
    lines.append("host: 0.0.0.0")
    # ★ 2026-09-06: 여기에 8000 이 **상수로** 박혀 있었다. env 의 SERVING_PORT 는 --port 를
    #   받는데 yaml 은 안 받으니, 둘이 갈리면 엔진은 8000 으로 리슨하고 스모크는 --port 를
    #   폴링한다 — 서빙이 성공했는데 health 가 영영 안 뜬다(캠페인 ⑦ b0 실측: 컨테이너는
    #   `Application startup complete` 인데 폴링은 75분 타임아웃을 향해 갔다).
    #   직전 캠페인 트리플렛은 둘 다 8080 이라 안 걸렸는데, 그건 **손저작이었기 때문**이다.
    #   같은 개념이 두 자리에 손으로 적힌 값 = 4종 안티패턴의 매직넘버 결함. 파생시킨다.
    lines.append("port: {}".format(port))
    # KV 절대클램프 따름정리(헌법, E2E 실증 corrected): gpu-memory-utilization 은 **항상 emit**.
    # clamp(kv-cache-memory-bytes)가 KV 사이징·이식성을 제어하지만, gmu 는 startup free-memory 검증(free ≥ gmu×total)
    # + 총 메모리 cap 에 여전히 쓰인다(vLLM 은 gmu 를 *KV 사이징*에만 무시 — config/cache.py). 통합메모리(GB10 free/total≈0.91)는
    # 기본 0.92 가 startup OOM → gmu ≤ 0.90 명시 필수. 이식성은 절대 clamp 가 준다(gmu-derived KV 는 호스트 VRAM 차이로 비이식).
    if kv_bytes is not None:
        lines.append("# gpu-memory-utilization = startup free-memory 게이트 + 총 cap(통합메모리 ≤0.90); 실제 KV·이식성은 kv-cache-memory-bytes 절대 클램프가 제어")
    lines.append("gpu-memory-utilization: {}".format(gmu))
    lines.append("max-model-len: {}".format(max_model_len))
    # max-num-seqs(batch) 줄: recipe 에 batch 있을 때만.
    if batch is not None:
        lines.append("max-num-seqs: {}".format(batch))
    # kv-cache-memory-bytes 절대 클램프 줄: 있을 때만.
    if kv_bytes is not None:
        lines.append("kv-cache-memory-bytes: {}".format(kv_bytes))
    # kv-cache-dtype 줄: KV quant 있을 때만.
    if kv_quant is not None and not _is_native_or_none(kv_quant):
        lines.append("kv-cache-dtype: {}".format(str(kv_quant).strip().lower()))
    # quantization 줄: native/none 이 아닐 때만.
    if not _is_native_or_none(quant):
        lines.append("quantization: {}".format(str(quant).strip().lower()))

    # ── 분산 스탠자 (2026-09-06) ────────────────────────────────────────────
    # ★ 이 생성기는 단일 토폴로지만 낼 줄 알았다. 그래서 multi 트리플렛은 매번 **손으로**
    #   `tensor-parallel-size`·`distributed-executor-backend` 두 줄을 덧붙여 만들어졌다
    #   (직전 캠페인 결함 #8 "분산 런타임 스탠자가 템플릿에 없고 손으로만 들어가 있었음" —
    #   그때 교정은 다른 템플릿에 들어갔고 이 생성기는 그대로였다).
    #   손저작은 선언에서 재현되지 않고 재생성이 덮어쓴다. 자리를 만든다.
    # TP=1 은 **적지 않는다** — vLLM 기본이고, 적으면 single 트리플렛의 기존 모양이 바뀐다
    #   (회귀 0 원칙). 즉 이 두 줄은 분산일 때만 나타난다.
    tp = recipe.get("tensor_parallel_size")
    if tp is not None and int(tp) > 1:
        lines.append("tensor-parallel-size: {}".format(int(tp)))
        # 백엔드는 **선언된 것만** 쓴다. 기본값을 여기서 지어내면 그 순간 매직넘버다 —
        # 어느 실행기를 쓰는지는 토폴로지·클러스터 형상의 함수이지 생성기 상수가 아니다.
        deb = recipe.get("distributed_executor_backend")
        if deb:
            lines.append("distributed-executor-backend: {}".format(str(deb).strip().lower()))

    # ── 트라이얼이 검증한 나머지 serve 노브 (2026-08-01 파리티 교정) ──────
    # ★ 여기가 비어 있으면 **검증된 레시피 ≠ 배포된 레시피** 가 된다.
    #   실증: gpt-oss-120b 는 `--moe-backend MARLIN` 으로 트라이얼이 수렴했는데 이 yaml 에
    #   그 줄이 없어, 같은 config 로 띄운 serve 가 auto→TRITON 커널 컴파일 실패로 즉사했다.
    #   트라이얼 통과가 배포 성공을 보장하지 못하면 S6(검증레시피) 자체가 무의미해진다.
    #   run_trial._build_serve_args 와 **같은 필드 집합**을 유지해야 하며,
    #   그 파리티는 아래 assert_serve_knob_parity() 가 집행한다.
    for key, flag, is_bool in (
        ("moe_backend", "moe-backend", False),
        ("gdn_prefill_backend", "gdn-prefill-backend", False),
        ("max_num_batched_tokens", "max-num-batched-tokens", False),
        ("enforce_eager", "enforce-eager", True),
        ("language_model_only", "language-model-only", True),
    ):
        v = recipe.get(key)
        if is_bool:
            if v:
                lines.append("{}: true".format(flag))
        elif v is not None and str(v).strip().lower() not in ("", "auto", "none"):
            lines.append("{}: {}".format(flag, v))
    return "\n".join(lines) + "\n"


# run_trial._build_serve_args 가 serve 인자로 소비하지만 **이 생성기가 의도적으로 다루지 않는**
# 필드. 여기 없는 신규 필드가 run_trial 에 생기면 파리티 검사가 실패한다(침묵 드롭 차단).
_PARITY_EXEMPT = {
    "model_path", "model_path_container", "model_capabilities", "id",
    "attention_backend",      # .sh 의 export VLLM_ATTENTION_BACKEND 로 전달
    "tool_call_parser",       # .sh 의 CLI 플래그로 전달
    "reasoning_parser",       # .sh 의 CLI 플래그로 전달
    "served_model_name",      # .env/.sh 로 전달
    "model_id",               # 주석/이름용
    "extra_env",              # 트라이얼 전용(임시 실험 env) — 배포 3종 세트로 승격하지 않는다
    # gmu 는 **config.safety_margin 이 권위**다(디바이스 풀 상한 = 배포 정책). candidate 값은
    # 트라이얼-로컬이며 배포로 승격하지 않는다 — 의도된 분기. 다만 둘이 다르면 "검증한 gmu ≠
    # 배포된 gmu" 가 되므로 recipe.py 가 불일치를 경고한다(조용한 분기 금지).
    "gpu_memory_utilization",
}


# candidate → recipe 투영의 **단일 소유자**. 이전에는 recipe.py 가 자체 dict 리터럴로
# 9개 필드만 복사해, gen_recipe_set 을 고쳐도 노브가 그 홉에서 조용히 떨어졌다
# (2026-08-01: moe_backend 를 _build_yaml 에 추가했는데도 생성물에 안 나온 원인).
# 홉이 둘이면 둘 다 고쳐야 하고, 그 사실을 잊는 것이 이 결함 계열의 본질이다 → 홉을 하나로 만든다.
SERVE_KNOB_KEYS = (
    "quantization", "max_model_len", "batch",
    "kv_cache_memory_bytes", "kv_cache_quant",
    "attention_backend", "tool_call_parser", "reasoning_parser",
    "moe_backend", "gdn_prefill_backend", "max_num_batched_tokens",
    "enforce_eager", "language_model_only",
    "serve_env",              # 선언된 커널 스위치 등 — .sh 의 export 로 승격(2026-09-06)
    "tensor_parallel_size",   # 분산 스탠자(2026-09-06) — TP>1 일 때만 yaml 에 나타난다
    "distributed_executor_backend",
)


def recipe_from_candidate(candidate: dict, **extra) -> dict:
    """수렴 candidate → gen_recipe_set 이 읽는 recipe dict. 추가 키는 extra 로 덮어쓴다.

    extra 용례: gpu_memory_utilization(=safety_margin) · target_gpu · vram_breakdown.
    """
    r = {k: candidate.get(k) for k in SERVE_KNOB_KEYS}
    r["id"] = candidate.get("id")
    r.update(extra)
    return r


# 파리티 검사용 대표값(타입별). 값이 yaml 에 그대로 나타나는지로 전달 여부를 판정한다.
def serve_env_pairs(value):
    """`serve_env` 를 (이름, 값) 목록으로 정규화한다. **이 모양의 단일 소유자**.

    받는 모양: dict{NAME: VALUE}(정본) · ["NAME=VALUE", ...] · "NAME=VALUE" · None.
    여러 모양을 받는 이유는 관대함이 아니라 **파리티 프로브 때문**이다 — 프로브는 모든
    노브에 스칼라 sentinel 을 넣어 산출물에 나타나는지 본다. 여기서 dict 만 받으면 프로브가
    TypeError 로 죽고, 그러면 노브 파리티라는 가드 전체가 이 필드에서 무력해진다.
    run_trial 도 **이 함수를 import 해서** 쓴다(두 자리에 적으면 갈린다).
    """
    if not value:
        return []
    if isinstance(value, dict):
        return [(str(k), str(v)) for k, v in sorted(value.items())]
    items = value if isinstance(value, (list, tuple)) else [value]
    pairs = []
    for it in items:
        text = str(it)
        name, sep, val = text.partition("=")
        pairs.append((name.strip(), val if sep else ""))
    return pairs


_PROBE = {
    "enforce_eager": True, "language_model_only": True,
    "max_num_batched_tokens": 4242, "batch": 4242,
    "max_model_len": 4242, "kv_cache_memory_bytes": 4242,
    "gpu_memory_utilization": 0.77,
}


def assert_serve_knob_parity(run_trial_source: str, raise_on_gap: bool = True):
    """run_trial 이 소비하는 serve 노브가 **실제로 3종 세트까지 도달**하는지 행위로 검사.

    소스 문자열 존재 확인은 부족하다 — 필드명이 파일에 있어도 중간 홉(recipe 투영)에서
    떨어지면 배포물엔 안 나온다. 그래서 candidate → recipe_from_candidate → _build_yaml/_build_sh
    를 실제로 통과시켜 대표값이 산출물에 나타나는지 본다.
    반환: 누락 필드 리스트(빈 리스트면 파리티 OK).
    """
    import re as _re
    consumed = set(_re.findall(r'candidate\.get\("([a-z_]+)"\)', run_trial_source))
    parsed = {"model_id": "probe/model", "container_path": "/app/models/probe/model"}
    gaps = []
    for f in sorted(consumed - _PARITY_EXEMPT):
        probe = _PROBE.get(f, "PROBE%sVALUE" % f.upper().replace("_", ""))
        cand = {"id": "probe", f: probe}
        # 시험 대상이 gmu 자신이면 덮어쓰지 않는다(덮어쓰면 프로브가 무효가 돼 오탐).
        extra = {} if f == "gpu_memory_utilization" else {"gpu_memory_utilization": 0.9}
        rec = recipe_from_candidate(cand, **extra)
        blob = _build_yaml(parsed, rec, "probe") + _build_sh("probe", "probe", rec)
        needle = "true" if probe is True else str(probe)
        # 생성기가 값을 정규화(소문자화)하는 필드가 있으므로 대소문자 무시로 비교한다.
        if needle.lower() not in blob.lower():
            gaps.append(f)
    if gaps and raise_on_gap:
        raise AssertionError(
            "serve 노브 파리티 위반 — run_trial 이 쓰는데 3종 세트에 도달하지 않는 필드: %s. "
            "검증된 레시피와 배포된 레시피가 갈린다. SERVE_KNOB_KEYS/_build_yaml 에 추가하거나 "
            "_PARITY_EXEMPT 에 근거와 함께 등록하라." % ", ".join(gaps))
    return gaps


def _build_sh(name, served_model_name, recipe=None):
    """configs/<name>.sh 내용 문자열 생성.

    기존 gpt-oss-20b-normal.sh 구조 그대로:
      TIKTOKEN 가드 + vllm serve --config ... --served-model-name ...

    Phase 2 확장(recipe 에 해당 키가 있을 때만 반영):
      attention_backend → export VLLM_ATTENTION_BACKEND=...,
      tool_call_parser → --enable-auto-tool-choice --tool-call-parser X,
      reasoning_parser → --reasoning-parser Y.
    """
    if recipe is None:
        recipe = {}
    attn_backend = recipe.get("attention_backend")
    tool_parser = recipe.get("tool_call_parser")
    reasoning_parser = recipe.get("reasoning_parser")

    lines = []
    lines.append("#!/bin/bash")
    lines.append("# {} 서빙 스크립트 (vllm-recipe-explorer 생성, "
                 "gpt-oss-20b-normal.sh 구조 동일)".format(name))
    lines.append("# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, "
                 "TIKTOKEN_ENABLED")
    lines.append("")
    lines.append("# TIKTOKEN 환경변수 설정")
    lines.append('if [ "$TIKTOKEN_ENABLED" = "true" ]; then')
    lines.append("    export TIKTOKEN_ENCODINGS_BASE=/encodings")
    lines.append("    export TIKTOKEN_RS_CACHE_DIR=/encodings")
    lines.append("fi")
    lines.append("")
    # 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh).
    # 단일노드/모드2 의 serve 진입은 이 .sh 이므로 여기서 arm 한다(멀티는 serve_runner 가 양노드 arm).
    # arm_patch.sh 가 .pth 를 써 engine+로컬 TP worker 전체에 패치 적용. 헌법 모델구동 런타임 패치 따름정리.
    lines.append("# 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh)")
    lines.append('if [ -f /app/configs/arm_patch.sh ]; then source /app/configs/arm_patch.sh; fi')
    lines.append("")
    # attention backend export: recipe 에 attention_backend 있을 때만.
    if attn_backend is not None:
        lines.append("# attention backend 고정")
        lines.append("export VLLM_ATTENTION_BACKEND={}".format(attn_backend))
        lines.append("")
    # 선언된 서빙 env(커널 스위치 등). 종전에는 자리가 없어 생성된 .sh 를 손으로 고쳤고,
    # 그 손질은 재생성이 덮어쓰며 선언에서 재현되지 않았다(그림자 배달 경로).
    env_pairs = serve_env_pairs(recipe.get("serve_env"))
    if env_pairs:
        lines.append("# 선언된 서빙 env (recipe.serve_env)")
        for _k, _v in env_pairs:
            lines.append("export {}={}".format(_k, _v))
        lines.append("")
    lines.append("# vllm serve 실행")
    # tool/reasoning parser 플래그: recipe 에 있을 때만 줄을 추가.
    serve_lines = ['vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \\',
                   '    --served-model-name "$SERVING_MODEL_NAME"']
    if tool_parser is not None:
        serve_lines[-1] = serve_lines[-1] + " \\"
        serve_lines.append("    --enable-auto-tool-choice \\")
        serve_lines.append("    --tool-call-parser {}".format(tool_parser))
    if reasoning_parser is not None:
        serve_lines[-1] = serve_lines[-1] + " \\"
        serve_lines.append("    --reasoning-parser {}".format(reasoning_parser))
    lines.extend(serve_lines)
    return "\n".join(lines) + "\n"


def _build_env(name, served_model_name, port, image=None, topology="single",
               vllm_version=None, build_dockerfile=None):
    """envs/.env.<name> 내용 문자열 생성.

    기존 .env.gpt-oss-20b-normal 스키마 준수:
      COMPOSE_PROJECT_NAME, CONTAINER_NAME, VERSION, NVIDIA_VISIBLE_DEVICES,
      SERVING_IP, SERVING_PORT, TIKTOKEN_ENABLED, SERVING_MODEL_NAME, CONFIG_FILE
      (+ IMAGE_TAG — 아래 참조)

    ★ IMAGE_TAG 를 반드시 emit 한다. docker-compose.yaml 은
      `image: ${IMAGE_TAG:-easy-vllm:0.24.0-cu132-aarch64-source}` 라서 이 변수가 없으면
      **조용히 낡은 기본 이미지로 폴백**한다. 그 결과 의도한 vLLM 이 아닌 버전을 서빙·측정하게
      되고, 로그·벤치 리포트에는 그 사실이 드러나지 않는다(D8 버전 치환 — testlog_26073117).
      2026-07-31~08-01 캠페인에서 `simulate --force` 재생성 때마다 3회 재발했고 그때마다
      사람이 수동 재주입했다 — 계획서에 경고를 세 번 적는 대신 생성부를 고친다.
    """
    lines = []
    sep = "# " + "═" * 69
    lines.append(sep)
    lines.append("# vLLM 서버 환경 설정 — {} (vllm-recipe-explorer 생성)".format(name))
    lines.append(sep)
    lines.append("")
    lines.append("# ─────────────── 0) 프로젝트, 컨테이너, env파일 이름 ─────────────────────")
    lines.append("COMPOSE_PROJECT_NAME=vllm_{}_project".format(name))
    # ★ 컨테이너 이름은 **토폴로지의 함수**다(2026-09-06). single 은 하나, multi 는 master/slave
    #   쌍이며 multinode_serve_smoke 가 그 두 이름으로 워치독 킬 필터를 만든다. 종전에는 single
    #   형태만 낼 줄 알아서 multi env 를 매번 손으로 고쳐 왔고, 이번에 그 손질을 빠뜨리자 스모크가
    #   "워치독 필터가 비었다(MASTER_CONTAINER_NAME 미설정)"로 **기동 전에** 멈췄다 — 가드가 옳게
    #   울었지만, 울릴 필요가 없는 울음이었다(자리가 없어서 난 결손).
    if topology == "multi":
        lines.append("MASTER_CONTAINER_NAME={}-master-container".format(name))
        lines.append("SLAVE_CONTAINER_NAME={}-slave-container".format(name))
    else:
        lines.append("CONTAINER_NAME={}-serving-container".format(name))
    lines.append("VERSION=1.0.0")
    lines.append("NVIDIA_VISIBLE_DEVICES=all")
    lines.append("")
    lines.append("# ─────────────── 1) 외부 노출 설정 (docker-compose 포트매핑) ─────────────")
    lines.append("SERVING_IP=0.0.0.0")
    lines.append("SERVING_PORT={}".format(port))
    lines.append("")
    lines.append("# ─────────────── 2) LLM 서빙 설정 ─────────────────────────────────────")
    lines.append("TIKTOKEN_ENABLED=true")
    lines.append("SERVING_MODEL_NAME={}".format(served_model_name))
    lines.append("CONFIG_FILE={}".format(name))
    lines.append("")
    lines.append("# ─────────────── 3) 컨테이너 이미지 (필수) ───────────────────────────")
    lines.append("# 비우면 compose 가 낡은 기본값으로 조용히 폴백해 **다른 vLLM 버전을 측정**한다.")
    if image:
        lines.append("IMAGE_TAG={}".format(image))
    else:
        # 조용한 부재를 만들지 않는다 — 주석으로 자리를 남겨 "안 적혀 있음"이 눈에 보이게 한다.
        lines.append("# IMAGE_TAG=<미지정 — 반드시 채울 것>")
        sys.stderr.write(
            "[gen_recipe_set] WARN: image 미지정 → env 에 IMAGE_TAG 를 쓰지 못했다. "
            "compose 가 낡은 기본 이미지로 폴백하므로 서빙 전에 직접 채워라.\n")
    # multi compose·스모크가 추가로 요구하는 키. **선언된 것만** 쓴다 — 지어내면 매직넘버이고,
    # 특히 VLLM_VERSION 은 이미지 정체성 변수라 틀리면 다른 버전을 측정하고도 모른다.
    if topology == "multi":
        lines.append("")
        lines.append("# ─────────────── 4) 분산(멀티노드) 전용 ──────────────────────────────")
        if build_dockerfile:
            lines.append("BUILD_DOCKERFILE={}".format(build_dockerfile))
        else:
            lines.append("# BUILD_DOCKERFILE=<미지정 — 반드시 채울 것>")
            sys.stderr.write("[gen_recipe_set] WARN: topology=multi 인데 build_dockerfile 미지정.\n")
        if vllm_version:
            lines.append("VLLM_VERSION={}".format(vllm_version))
        else:
            lines.append("# VLLM_VERSION=<미지정 — 반드시 채울 것>")
            sys.stderr.write("[gen_recipe_set] WARN: topology=multi 인데 vllm_version 미지정 — "
                             "이미지 정체성 변수다(틀리면 다른 버전을 측정하고도 모른다).\n")
    return "\n".join(lines) + "\n"


def generate(parsed, recipe, name, repo_root, port, served_model_name, force=False, topology="single",
             vllm_version=None, build_dockerfile=None,
             image=None):
    """3종 세트(.yaml + .sh + .env)를 생성하고 생성 경로 리스트를 반환.

    Args:
        parsed: parse_model_config.parse() 결과 dict.
        recipe: 선택된 후보 dict(quantization, max_model_len, gpu_memory_utilization).
        name: 3종 세트 base 이름(config_name).
        repo_root: 레포 루트(configs/, envs/ 가 위치).
        port: SERVING_PORT.
        served_model_name: --served-model-name 값.
        force: True 면 기존 파일 덮어쓰기 허용. False(기본)면 기존 파일 존재 시 에러.

    Returns:
        생성된 파일 절대경로 리스트 [yaml_path, sh_path, env_path].
    """
    configs_dir = os.path.join(repo_root, "configs")
    envs_dir = os.path.join(repo_root, "envs")
    os.makedirs(configs_dir, exist_ok=True)
    os.makedirs(envs_dir, exist_ok=True)

    yaml_path = os.path.join(configs_dir, "{}.yaml".format(name))
    sh_path = os.path.join(configs_dir, "{}.sh".format(name))
    env_path = os.path.join(envs_dir, ".env.{}".format(name))

    targets = [yaml_path, sh_path, env_path]

    # 기존 파일 가드: force 없이는 덮어쓰지 않는다.
    if not force:
        existing = [p for p in targets if os.path.exists(p)]
        if existing:
            raise FileExistsError(
                "이미 존재하는 파일(덮어쓰려면 --force): " + ", ".join(existing))

    yaml_text = _build_yaml(parsed, recipe, served_model_name, port=port)
    sh_text = _build_sh(name, served_model_name, recipe)
    env_text = _build_env(name, served_model_name, port, image=image,
                          topology=topology, vllm_version=vllm_version,
                          build_dockerfile=build_dockerfile)

    with open(yaml_path, "w", encoding="utf-8") as f:
        f.write(yaml_text)
    with open(sh_path, "w", encoding="utf-8") as f:
        f.write(sh_text)
    with open(env_path, "w", encoding="utf-8") as f:
        f.write(env_text)

    # .sh 는 실행권한 가정.
    try:
        os.chmod(sh_path, 0o755)
    except OSError:
        pass

    return [yaml_path, sh_path, env_path]


def _load_json(path):
    with open(path, "r", encoding="utf-8") as f:
        return json.load(f)


def main(argv=None):
    # ── `--check-parity`: 파리티 tripwire 의 **집행 진입점** (2026-08-16 신설) ──────────────
    #   `assert_serve_knob_parity` 는 "run_trial 이 candidate 에서 읽는 필드는 3종 세트까지
    #   도달해야 한다"는 계약을 지키려고 만들어졌는데, **호출자가 0 개였다**(정의 + 주석뿐).
    #   그 결과 2026-08-16 에 실제 위반(model_host_path·tensor_parallel_size·tp)이 커밋을 통과했고
    #   verify_distribution·policy_registry·claim_predicates 어느 것도 잡지 못했다 —
    #   **검사를 만든 것과 검사가 도는 것은 다르다**(이 레포에서 반복된 패턴: 선언 미배선·
    #   teardown 미배선·READY_MAX 권고 미반영). 여기에 진입점을 두고 verify_distribution 이 호출한다.
    if argv is None:
        argv = sys.argv[1:]
    if "--check-parity" in argv:
        rt_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "run_trial.py")
        try:
            with open(rt_path, encoding="utf-8") as fh:
                gaps = assert_serve_knob_parity(fh.read(), raise_on_gap=False)
        except OSError as exc:
            print("parity: run_trial.py 를 읽지 못했다 — %s" % exc, file=sys.stderr)
            return 2
        if gaps:
            print("parity FAIL — run_trial 이 candidate 에서 읽지만 3종 세트에 도달하지 않는 필드: %s"
                  % ", ".join(gaps), file=sys.stderr)
            print("  → SERVE_KNOB_KEYS/_build_yaml 에 추가하거나, serve 노브가 아니면 candidate 를"
                  " 읽지 말고 opts 로 받아라(평면 분리). 면제 등록은 마지막 수단이다.", file=sys.stderr)
            return 1
        print("parity ok — 3종 세트 도달 갭 0")
        return 0

    parser = argparse.ArgumentParser(
        description="DGX Spark 서빙용 3종 세트(.yaml+.sh+.env) 생성")
    parser.add_argument("--parsed", required=True,
                        help="parse_model_config 결과 JSON 경로")
    parser.add_argument("--recipe", required=True,
                        help="선택된 레시피 후보 JSON 경로")
    parser.add_argument("--name", required=True, help="3종 세트 base 이름")
    parser.add_argument("--repo", required=True, help="레포 루트(configs/, envs/)")
    parser.add_argument("--port", type=int, required=True, help="SERVING_PORT")
    parser.add_argument("--served-model-name", required=True,
                        help="--served-model-name 값")
    # ★ 2026-09-06 발견 — CLI 배선이 반쪽이었다.
    #   ① `image` 는 generate() 인자에 있는데 CLI 에 없었다. 그래서 이 스크립트를 **독립 호출**하면
    #      IMAGE_TAG 가 빠지고 compose 가 낡은 기본 이미지로 조용히 폴백한다 — 함수 본문 주석이
    #      D8 로 세 번 경고한 그 사고를 CLI 경로에서는 막을 수 없었다. 경고문이 있어도 배선이
    #      반쪽이면 사고는 난다.
    #   ② 토폴로지 인지가 없어 multi env 는 매번 손으로 고쳐졌다(master/slave 쌍 · 분산 키).
    parser.add_argument("--image", help="컨테이너 이미지 태그 → env 의 IMAGE_TAG")
    parser.add_argument("--topology", default="single", choices=("single", "multi"),
                        help="env 형태를 가른다. multi = master/slave 컨테이너 쌍 + 분산 키")
    parser.add_argument("--vllm-version", help="multi env 의 VLLM_VERSION(이미지 정체성)")
    parser.add_argument("--build-dockerfile", help="multi env 의 BUILD_DOCKERFILE")
    parser.add_argument("--force", action="store_true",
                        help="기존 파일 덮어쓰기 허용")
    args = parser.parse_args(argv)

    parsed = _load_json(args.parsed)
    recipe = _load_json(args.recipe)

    try:
        paths = generate(parsed, recipe, args.name, args.repo, args.port,
                         args.served_model_name, force=args.force, image=args.image,
                         topology=args.topology, vllm_version=args.vllm_version,
                         build_dockerfile=args.build_dockerfile)
    except FileExistsError as e:
        print("[gen_recipe_set] 중단: {}".format(e), file=sys.stderr)
        return 2

    for p in paths:
        print("생성됨: {}".format(p))
    return 0


if __name__ == "__main__":
    sys.exit(main())
