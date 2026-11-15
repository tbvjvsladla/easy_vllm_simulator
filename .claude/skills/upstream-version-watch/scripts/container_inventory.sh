#!/bin/bash
# container_inventory.sh — 빌드된 이미지 · 서빙 중 컨테이너 인벤토리 (2026-09-04 · 사용자 결정)
#
# 왜 이 스킬인가: `upstream-version-watch` 는 **컨테이너 관리**를 소유한다 — 이미지를 렌더·빌드하고
#   (`render_dockerfile.py`), 서빙을 올리고(`single_serve_up.sh`) 내린다(`single_serve_down.sh`).
#   "무엇이 빌드돼 있나 · 무엇이 지금 돌고 있나" 는 그 관리의 **읽기 면**이며, 같은 자리에 둔다.
#   이 스크립트는 **read-only 다** — 아무것도 지우거나 내리지 않는다(정리는 down 진입점이 소유).
#
# ★ 크기로 노드를 비교하지 마라. 메인은 containerd snapshotter, 서브는 overlay2 라 **같은 내용이
#   45.5GB 대 30.8GB** 로 보인 실측이 있다(2026-09-04 이전). 신원은 **digest** 가 답한다 —
#   그래서 크기는 참고로만 싣고 판정 필드로 쓰지 않는다.
#
# ★ 고아 판정은 "참조하는 envfile 이 있는가" 로만 한다. 여기서 삭제 후보를 **고르지 않는다** —
#   그 판단은 `cleanup_docker.py`(explorer 소유 · dry-run 후 사람이 --apply)의 일이다.
#
# 사용: container_inventory.sh [--json] [--topology single|multi|both] [--filter <repo 부분일치>]
# 종료: 0=산출 · 2=인자 오류
set -uo pipefail

TAG="[inventory]"
FMT=text; TOPO=both; FILTER=easy-vllm
while [ $# -gt 0 ]; do case "$1" in
  --json) FMT=json; shift ;;
  --topology) TOPO="${2:-}"; shift 2 ;;
  --filter) FILTER="${2:-}"; shift 2 ;;
  *) echo "$TAG FAIL: 알 수 없는 인자: $1" >&2; exit 2 ;;
esac; done
case "$TOPO" in single|multi|both) ;; *) echo "$TAG FAIL: --topology 는 single|multi|both" >&2; exit 2 ;; esac

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"

# docker 를 못 물어본 것을 "없다"로 접지 않는다 — rc 를 본다(single_serve_down 의 규율과 동일).
if ! IMG_RAW="$(docker images --digests --format '{{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Digest}}\t{{.CreatedAt}}\t{{.Size}}' 2>&1)"; then
  echo "$TAG FAIL: docker 이미지 조회 불가(rc≠0) — 부재가 아니라 **판정 불가**다." >&2
  echo "$TAG   docker: $IMG_RAW" >&2; exit 2
fi
if ! PS_RAW="$(docker ps --format '{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}' 2>&1)"; then
  echo "$TAG FAIL: docker 컨테이너 조회 불가(rc≠0)" >&2; exit 2
fi

REPO="$REPO" TOPO="$TOPO" FILTER="$FILTER" FMT="$FMT" IMG_RAW="$IMG_RAW" PS_RAW="$PS_RAW" \
"$SDIR/inline_json.sh" inventory
