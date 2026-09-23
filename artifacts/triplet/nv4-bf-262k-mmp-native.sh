#!/bin/bash
# nv4-bf-262k-mmp-native native 서빙 러너 — render_native_triplet.py 가 Docker 트리플렛 nv4-bf-262k-mmp 에서 렌더(손저작 ✗).
#   원본 output/multi/configs/nv4-bf-262k-mmp.sh sha256=d7c8793d237a988f 의 `vllm serve --config … --served-model-name …` 를
#   호스트 venv 로 옮긴 것이다. 컨테이너 전용 arming(arm_patch.sh·tiktoken /encodings)은 렌더가 fail-closed 로 막는다.
#   기동 주체는 native_multinode_serve.py up 이다 — 셀 env·캐시 격리 env·LD_LIBRARY_PATH·NATIVE_VENV 를 주입해 이 파일을 실행한다.
set -euo pipefail
: "${NATIVE_VENV:?NATIVE_VENV(run root 의 venv) 미주입 — native_multinode_serve.py up 이 주입한다}"
: "${CONFIG_FILE:?CONFIG_FILE 미주입(셀 env)}"
: "${SERVING_MODEL_NAME:?SERVING_MODEL_NAME 미주입(셀 env)}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec "${NATIVE_VENV}/bin/vllm" serve --config "${HERE}/${CONFIG_FILE}.yaml" \
    --served-model-name "${SERVING_MODEL_NAME}"
