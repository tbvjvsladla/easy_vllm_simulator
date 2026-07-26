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


def _build_yaml(parsed, recipe, served_model_name):
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
    lines.append("port: 8000")
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
    return "\n".join(lines) + "\n"


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


def _build_env(name, served_model_name, port):
    """envs/.env.<name> 내용 문자열 생성.

    기존 .env.gpt-oss-20b-normal 스키마 준수:
      COMPOSE_PROJECT_NAME, CONTAINER_NAME, VERSION, NVIDIA_VISIBLE_DEVICES,
      SERVING_IP, SERVING_PORT, TIKTOKEN_ENABLED, SERVING_MODEL_NAME, CONFIG_FILE
    """
    lines = []
    sep = "# " + "═" * 69
    lines.append(sep)
    lines.append("# vLLM 서버 환경 설정 — {} (vllm-recipe-explorer 생성)".format(name))
    lines.append(sep)
    lines.append("")
    lines.append("# ─────────────── 0) 프로젝트, 컨테이너, env파일 이름 ─────────────────────")
    lines.append("COMPOSE_PROJECT_NAME=vllm_{}_project".format(name))
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
    return "\n".join(lines) + "\n"


def generate(parsed, recipe, name, repo_root, port, served_model_name, force=False):
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

    yaml_text = _build_yaml(parsed, recipe, served_model_name)
    sh_text = _build_sh(name, served_model_name, recipe)
    env_text = _build_env(name, served_model_name, port)

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
    parser.add_argument("--force", action="store_true",
                        help="기존 파일 덮어쓰기 허용")
    args = parser.parse_args(argv)

    parsed = _load_json(args.parsed)
    recipe = _load_json(args.recipe)

    try:
        paths = generate(parsed, recipe, args.name, args.repo, args.port,
                         args.served_model_name, force=args.force)
    except FileExistsError as e:
        print("[gen_recipe_set] 중단: {}".format(e), file=sys.stderr)
        return 2

    for p in paths:
        print("생성됨: {}".format(p))
    return 0


if __name__ == "__main__":
    sys.exit(main())
