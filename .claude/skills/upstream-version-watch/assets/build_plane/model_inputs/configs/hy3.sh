#!/bin/bash
# hy3.sh — 모델별 사전처리 + vllm serve — Tencent Hy3-NVFP4-W4A16 × stock vLLM 0.24.0 (이미지 …-source) × 2x GB10 TP=2 Ray.
# serve_runner.sh(master)에서 source 호출. 모델: HYV3ForCausalLM native (0.24.0 레지스트리 확인). plan_2026071119_1.
# experts=NVFP4(W4A16) + 비-expert=BF16. MoE backend = MARLIN(auto — W4A16 유일 수용 커널, hy3.yaml 주석 참조).
# NO tiktoken(Hy3 자체 tokenizer) · NO trust-remote-code(native arch).
# 에이전트-레디: MTP spec-1 + reasoning/tool 파서(hy_v3) + :opensource 런타임 패치(hy3_patch.py, arm_patch.sh 가 arm).

# (TIKTOKEN_ENABLED=false → no-op. 러너 계약 정합용 보존.)
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[hy3] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

echo "[hy3] Starting vLLM serve (stock 0.24.0, TP=2, Ray, kv fp8_e4m3, moe=MARLIN[auto/W4A16], MTP spec=1, enforce-eager, agent-ready)..."
# MTP(hy_v3) spec-1 — 83.4% accept, lossless(카드). spec-2 는 pos-2 acceptance ~18-21% → net loss(tonyd2wild #5).
#   JSON 은 이 .sh 파일 안이라 SSH command-substitution mangling 무관(tonyd2wild #1 우회 — 파일 경유).
# --no-enable-flashinfer-autotune: 통합메모리 빡빡 fit 보호(serve#3 실증) — flashinfer 오토튜너가 KV 할당 직후
#   max-config 어텐션 workspace 를 probe 하며 slave 메모리를 >4.6GiB 스파이크시켜 10.24GiB memwatch 층 아래로
#   떨어뜨려 컨테이너 kill(exit137, 12:13:26 autotune→crash). 오토튜닝 생략 = 스타트업 스파이크 제거(DS4 선례 동형).
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}" \
    --speculative-config '{"method":"mtp","num_speculative_tokens":1}' \
    --no-enable-flashinfer-autotune
