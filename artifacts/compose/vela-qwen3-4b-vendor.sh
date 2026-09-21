#!/usr/bin/env bash
# vela-qwen3-4b-vendor — 네이티브 평면 러너 (기동 방법)
#
# 평면: 네이티브 venv(컨테이너 아님). 그래서 이 저장소의 compose 평면 산출물을 쓰지 않는다.
# 포트·인증키는 **envfile 이 단일 권위**다 — 이 파일에 값을 적으면 선언과 실물이 갈린다.
#
# ★ 커널 모드 고정이 필수다. 배포본의 포장 형태에 따라 커널 실행 경로가 갈릴 수 있어,
#   명시하지 않으면 같은 설정이 다른 경로로 돌고 수치가 비교 불가가 된다.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CFG=vela-qwen3-4b-vendor

VENV="${VELA_VENV:?VELA_VENV 미정 — VELA 가 설치된 venv 경로를 지정하라}"
BUNDLE="${VELA_BUNDLE:?VELA_BUNDLE 미정 — 배포본 디렉터리(artifacts/ 포함)를 지정하라}"
MODEL="${MODEL_PATH:?MODEL_PATH 미정 — Qwen3-4B 체크포인트 경로를 지정하라}"

EF="$HERE/../envs/.env.$CFG"
[ -f "$EF" ] || { echo "envfile 없음: $EF" >&2; exit 2; }
# shellcheck disable=SC1090
set -a; . "$EF"; set +a
PORT="${SERVING_PORT:?envfile 에 SERVING_PORT 미정}"
HOST="${SERVING_IP:-0.0.0.0}"

# 외부 바인딩 서빙에서 키의 부재는 곧 공개다 — 침묵 폴백을 두지 않는다.
[ -n "${SERVING_API_KEY:-}" ] || { echo "SERVING_API_KEY 가 비었다 — 무인증 공개를 의도하면 NONE 으로 명시하라" >&2; exit 2; }
[ "${SERVING_API_KEY}" = "NONE" ] || export VLLM_API_KEY="${SERVING_API_KEY}"

export PATH="${VENV}/bin:${PATH}"     # 엔진 컴파일 warmup 이 PATH 에서 빌드도구를 찾는다
export VELA_KERNEL_MODE=aot           # ★ 위 주석
[ -f "$BUNDLE"/license/*.vlic ] 2>/dev/null && export VELA_LICENSE="$(ls "$BUNDLE"/license/*.vlic | head -1)"

exec "${VENV}/bin/vela" serve compress \
  --model "$MODEL" \
  --artifact "$BUNDLE/artifacts/qwen3_4b/vela_k4v4t_r8" \
  --max-model-len 32768 \
  --max-concurrency 8 \
  --max-batched-tokens 16384 \
  --gpu-mem-fraction 0.28 \
  --host "$HOST" --port "$PORT"
