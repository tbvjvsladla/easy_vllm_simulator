#!/bin/bash
# 메인노드 → 서브노드 코드 동기화 (rsync over SSH/ConnectX-7).
#
# 멀티노드 확장의 [전달] 단계. 메인에서 검증된 런타임 소스를 서브로 직접 전송한다(GitHub 경유 X).
# 메인/서브 코드는 완전 동일해야 하므로 --delete 로 정합(stray 제거). 빌딩블럭/VCS/캐시는 제외·보호.
#
# HITL 안전장치: 기본은 DRY-RUN(미리보기만). 실제 전송은 명시적으로 --apply 를 줘야 한다.
#
# 제외(전송·삭제 양쪽에서 보호): .git(서브 git 상태) · .claude(스킬=메인 전용 빌딩블럭) ·
#   seed(빌딩블럭) · docs(빌드 불필요) · __pycache__(캐시).
# 전송 대상: Dockerfile · docker-compose.yaml · requirements.txt · configs/ · envs/ · README.md.
#
# 사용:
#   bash sync_to_sub.sh            # DRY-RUN (무엇이 바뀔지 미리보기)
#   bash sync_to_sub.sh --apply    # 실제 전송 + 체크섬 검증
# 환경변수 override: SUB_HOST(<ssh_user>@<host>) · SRC(기본 레포루트) · DEST.
# SUB_HOST 미지정 시 manifest.yaml 의 nodes[] (role: sub) 에서 ssh_user/host 를 읽어 해소한다.
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)/}"

# ── SUB_HOST 해소: 환경변수 우선, 없으면 manifest.yaml nodes[] (role: sub) ──
_resolve_sub_host_from_manifest() {
    # 테라포밍이 채운 레포 루트 manifest.yaml 에서 role=sub 노드의 ssh_user@host 를 추출.
    local manifest="${SRC%/}/manifest.yaml"
    [ -f "$manifest" ] || return 1
    # nodes: 블록에서 role: sub 항목의 host/ssh_user 를 순차 파싱(외부 yq 의존 없이 awk).
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub/ { in_sub=1; host=""; user=""; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*host:/    { sub(/^[[:space:]]*host:[[:space:]]*/, ""); gsub(/[ "\r]/, ""); host=$0 }
        in_sub && /^[[:space:]]*ssh_user:/{ sub(/^[[:space:]]*ssh_user:[[:space:]]*/, ""); gsub(/[ "\r]/, ""); user=$0 }
        in_sub && host != "" && user != "" { print user "@" host; exit }
    ' "$manifest"
}

if [ -z "${SUB_HOST:-}" ]; then
    SUB_HOST="$(_resolve_sub_host_from_manifest || true)"
fi
if [ -z "${SUB_HOST:-}" ]; then
    echo "[sync] FAIL: 서브노드 주소 미해소 — 환경변수 SUB_HOST(<ssh_user>@<host>)를 지정하거나" >&2
    echo "             manifest.yaml 의 nodes[] (role: sub) 를 테라포밍으로 채우세요." >&2
    exit 4
fi
DEST="${DEST:-~/ws_docker/vllm_serving_server/}"
SSH_OPTS="ssh -o BatchMode=yes -o ConnectTimeout=8"

MODE="dryrun"
[ "${1:-}" = "--apply" ] && MODE="apply"

EXCLUDES=(--exclude '.git' --exclude '.claude' --exclude 'seed' --exclude 'docs' --exclude '__pycache__' --exclude 'CLAUDE.md' --exclude 'output/single')  # output/single = single-node 통로(서브 불필요). 멀티 빌드입력 output/multi 는 전송. plan_2026062312_1
RSYNC=(rsync -az --delete -e "$SSH_OPTS" "${EXCLUDES[@]}")

# ── pre-flight: SSH 도달성 ──
if ! $SSH_OPTS "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    echo "[sync] FAIL: $SUB_HOST 에 SSH 불가 (키 인증·네트워크 확인)"; exit 3
fi

if [ "$MODE" = "dryrun" ]; then
    echo "[sync] DRY-RUN  $SRC → $SUB_HOST:$DEST  (실제 전송 안 함 — --apply 로 실행)"
    "${RSYNC[@]}" --dry-run --itemize-changes "$SRC" "$SUB_HOST:$DEST"
    echo "[sync] (위는 미리보기. 변경 사항을 사람이 확인 후 --apply)"
    exit 0
fi

# ── apply ──
echo "[sync] APPLY  $SRC → $SUB_HOST:$DEST"
"${RSYNC[@]}" "$SRC" "$SUB_HOST:$DEST"

# ── 전송 후 체크섬 검증(핵심 빌드 입력) ──
echo "[sync] 체크섬 검증..."
fail=0
for f in output/multi/Dockerfile output/multi/Dockerfile.source-build output/multi/docker-compose.yaml output/multi/requirements.txt; do
    [ -f "${SRC}${f}" ] || { echo "  ⏭  ${f}: 로컬 부재 — 검증 생략(소스빌드/prebuilt 브랜치 차이)"; continue; }
    L=$(md5sum "${SRC}${f}" 2>/dev/null | awk '{print $1}')
    R=$($SSH_OPTS "$SUB_HOST" "md5sum ~/ws_docker/vllm_serving_server/${f} 2>/dev/null" | awk '{print $1}')
    if [ -n "$L" ] && [ "$L" = "$R" ]; then
        echo "  ✅ ${f}: $L"
    else
        echo "  ❌ ${f}: main=$L sub=$R (불일치)"; fail=1
    fi
done
[ "$fail" -eq 0 ] && echo "[sync] 완료 — 핵심 빌드 입력 체크섬 일치" || { echo "[sync] FAIL: 체크섬 불일치"; exit 2; }
