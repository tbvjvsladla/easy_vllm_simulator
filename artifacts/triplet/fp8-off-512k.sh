# ⚠ 형상(shape) — 운영자 절대경로는 플레이스홀더로 가렸다.
#   <repo> · <manifest.nas_model_path> · <manifest-derived VLLM_VENV> 는 각자 환경에서 온다.
#   어떤 변수가 필요한지가 재현 정보이고, 값은 배포 대상이 아니다(계약 §3).
#!/bin/bash
# fp8-off-512k 서빙 스크립트 (vllm-recipe-explorer 생성, gpt-oss-20b-normal.sh 구조 동일)
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# TIKTOKEN 자산 경로 미선언(manifest.tiktoken_host_path 비어 있음) — export 생략

# 모델구동 런타임 패치 arming (configs/${CONFIG_FILE}_patch.py 존재 시; 메인 저작 arm_patch.sh)
if [ -f <repo>/output/single/configs/arm_patch.sh ]; then source <repo>/output/single/configs/arm_patch.sh; fi

# 선언된 서빙 env (recipe.serve_env)
export VLLM_PLE_CPU_OFFLOAD=1

# vllm serve 실행
vllm serve --config "<repo>/output/single/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
