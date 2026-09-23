#!/bin/bash
# native-install.sh — nv4-bf-262k-mmp-native native 설치 재현 명령(native_multinode_serve.py up 이 run n1-2609230653 에서 **각 노드마다** 실행한 순서).
#   각 노드가 자기 로컬 이미지에서 wheelhouse 를 재포장하고(이미지 전송 ✗) 오프라인으로만 설치한다(--no-index · 다운로드 ✗).
#   provenance: 원천 이미지 태그 easy-vllm:0.29.0rc6-cu133-aarch64-source · wheelhouse 도구 sha256=fcf4dca4396824d9c3bd37e837c653fcb9d339d9893fd7a712dbbda67e2fd461 · 승인 선언 sha256=9041c0def8baa3d06a3483aba1b5270e467b28acccbdfd606db1281d6f8d39c3 · generated_utc=2026-09-23T06:53:50Z
#   사용: bash native-install.sh <run-root> [image] — 저장소 루트에서 실행(도구는 저장소 상대경로).
set -euo pipefail
RUN_ROOT="${1:?사용: native-install.sh <run-root> [image]}"
IMAGE="${2:-easy-vllm:0.29.0rc6-cu133-aarch64-source}"
WH_TOOL=.claude/skills/upstream-version-watch/scripts/native_wheelhouse.py
WH_ACCEPT=.claude/skills/upstream-version-watch/references/native_wheelhouse_accept.json
# ① 재포장(docker create → 설치 트리 cp → RECORD 대조 재포장 · 시작하지 않는 컨테이너)
python3 "${WH_TOOL}" build --image "${IMAGE}" --out "${RUN_ROOT}/wh" --generated-utc 2026-09-23T06:53:50Z --accept-file "${WH_ACCEPT}"
# ② 설치 게이트(이미지 동등성) — 도구가 python3.12 -m venv → pip install --no-index --no-deps -r wh/requirements-closure.txt
#    → 설치 집합 == closure 핀 → pip check ⊆ 선언된 이미지 고유 충돌 → ldd(vllm *.so) → import vllm,torch + cuda → env 레시피
python3 "${WH_TOOL}" verify --wheelhouse-dir "${RUN_ROOT}/wh" --venv "${RUN_ROOT}/venv" --generated-utc 2026-09-23T06:53:50Z --accept-file "${WH_ACCEPT}"
