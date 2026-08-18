#!/bin/bash
# hy3.sh — 모델별 사전처리 + vllm serve — Tencent Hy3-NVFP4-W4A16 × stock vLLM 0.27.0 (이미지 …-source) × 2x GB10 TP=2 Ray.
# serve_runner.sh(master)에서 source 호출. 모델: HYV3ForCausalLM native (0.27.0 레지스트리 확인 — 2026-08-18 이미지 내부 직독). plan_2026071119_1.
# experts=NVFP4(W4A16) + 비-expert=BF16. MoE backend = MARLIN(auto — W4A16 유일 수용 커널, hy3.yaml 주석 참조).
# NO tiktoken(Hy3 자체 tokenizer) · NO trust-remote-code(native arch).
# 에이전트-레디: MTP spec-1 + reasoning/tool 파서(hy_v3). :opensource 는 0.27.0 이 tokenizer token_suffix 로
#   네이티브 처리 → **런타임 패치 불요**(2026-08-18 재유도 · policy:RUNTIME_PATCH_NO_CARRY_FORWARD).

# (TIKTOKEN_ENABLED=false → no-op. 러너 계약 정합용 보존.)
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

if [ "${SKIP_SERVE:-0}" = "1" ]; then
    echo "[hy3] Env vars loaded, skipping serve (slave mode)"
    return 0
fi

echo "[hy3] Starting vLLM serve (stock 0.27.0, TP=2, Ray, kv fp8_e4m3, moe=MARLIN[auto/W4A16], MTP spec=2, enforce-eager, agent-ready)..."
# MTP(hy_v3) spec-2 — 2026-08-18 OFAT 캠페인 실측 승자(21.46 t/s · R0 spec-1 20.05 대비 +7.0%).
#   ⚠ 이 자리에 있던 주석은 "spec-2 는 pos-2 acceptance ~18-21% → net loss(tonyd2wild #5)" 였고
#   그것이 기준선을 spec-1 로 고정시킨 근거였다. 캠페인이 **관측은 맞고 추론은 틀렸음**을 보였다:
#   둘째 위치 수용률 0.208(=20.8%, 주장과 일치)인데도 순이득 +7.0% 다. 순손실 경계는 **3 에 있다**
#   (spec-3 = 18.42 t/s, -14.2% — 위치2 기여가 연쇄조건부라 0.602x0.306x0.143≈0.026 토큰뿐).
#   근거: docs/testlog/testlog_26081814 §8-9 · docs/simlog/26081814_hy3_OFAT_캠페인/R{0,4,5}.
#   carry-forward 금지는 주석에도 적용된다 — 외부 이슈번호가 달려도 우리 버전에서 재측정 전엔 가설이다.
#   JSON 은 이 .sh 파일 안이라 SSH command-substitution mangling 무관(tonyd2wild #1 우회 — 파일 경유).
# --no-enable-flashinfer-autotune: 통합메모리 빡빡 fit 보호(serve#3 실증) — flashinfer 오토튜너가 KV 할당 직후
#   max-config 어텐션 workspace 를 probe 하며 slave 메모리를 >4.6GiB 스파이크시켜 10.24GiB memwatch 층 아래로
#   떨어뜨려 컨테이너 kill(exit137, 12:13:26 autotune→crash). 오토튜닝 생략 = 스타트업 스파이크 제거(DS4 선례 동형).
exec vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}" \
    --host "${SERVING_IP:-0.0.0.0}" \
    --port "${SERVING_PORT}" \
    --speculative-config '{"method":"mtp","num_speculative_tokens":2}' \
    --no-enable-flashinfer-autotune
