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
#   bash sync_to_sub.sh                       # DRY-RUN (무엇이 바뀔지 미리보기)
#   bash sync_to_sub.sh --apply               # 실제 전송 + 체크섬 검증 (work_dir 부재 시 R2 게이트로 정지)
#   bash sync_to_sub.sh --apply --provision   # 사람 승인: 서브 work_dir 부재 시 신설(mkdir) 후 전송
# 환경변수 override: SUB_HOST(<ssh_user>@<host>) · SRC(기본 레포루트) · DEST · SUB_WORK_DIR.
# SUB_HOST·SUB_WORK_DIR 미지정 시 manifest.yaml 의 nodes[] (role: sub) 에서 ssh_user/host·work_dir 를 읽어 해소한다.
# DEST 미지정 시 = 서브 work_dir(기본값=메인 레포 경로와 동일, R2/plan_2026062320_1).
# ⚠ R2 불변식: 서브 work_dir 신설은 반드시 HITL(--provision) — HITL 없는 자동 경로 신설 금지.
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)/}"

# ── SUB_HOST 해소: 환경변수 우선, 없으면 manifest.yaml nodes[] (role: sub) ──
_resolve_sub_host_from_manifest() {
    # 테라포밍이 채운 manifest 실값(output/multi 통로)에서 role=sub 노드의 ssh_user@host 를 추출(plan_2026062315_1).
    local manifest="${SRC%/}/output/multi/manifest.yaml"
    [ -f "$manifest" ] || return 1
    # nodes: 블록에서 role: sub 항목의 host/ssh_user 를 순차 파싱(외부 yq 의존 없이 awk).
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; host=""; user=""; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*host:/    { sub(/^[[:space:]]*host:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); host=$0 }
        in_sub && /^[[:space:]]*ssh_user:/{ sub(/^[[:space:]]*ssh_user:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); user=$0 }
        in_sub && host != "" && user != "" { print user "@" host; exit }
    ' "$manifest"
}

# ── SUB_WORK_DIR 해소: 환경변수 우선, 없으면 manifest nodes[sub].work_dir, 그래도 없으면 메인 레포 경로(R2 기본값=동일) ──
_resolve_sub_work_dir_from_manifest() {
    local manifest="${SRC%/}/output/multi/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*work_dir:/ { sub(/^[[:space:]]*work_dir:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); print; exit }
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
if [ -z "${SUB_WORK_DIR:-}" ]; then
    SUB_WORK_DIR="$(_resolve_sub_work_dir_from_manifest || true)"
fi
# 폴백: manifest 미지정 → 메인 레포 경로와 동일(R2 기본값). 옛 고정 서브경로 하드코딩 제거.
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="${SRC%/}"
DEST="${DEST:-${SUB_WORK_DIR}/}"
SSH_OPTS="ssh -o BatchMode=yes -o ConnectTimeout=8"

MODE="dryrun"; PROVISION=0
for a in "$@"; do
    [ "$a" = "--apply" ]     && MODE="apply"
    [ "$a" = "--provision" ] && PROVISION=1
done

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
# R2 HITL 게이트: 서브 work_dir 부재 시 자동신설 금지 — 사람 승인(--provision) 전 정지.
if ! $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR' ]" 2>/dev/null; then
    if [ "$PROVISION" != "1" ]; then
        echo "[sync] STOP(R2): 서브 작업경로 부재 — $SUB_HOST:$SUB_WORK_DIR" >&2
        echo "       ❓ 서브노드에 이 경로를 신설할까요? 사람 승인 시 아래로 재실행:" >&2
        echo "          bash sync_to_sub.sh --apply --provision" >&2
        echo "       (HITL 없는 자동 경로 신설 금지 — 헌법 R2/plan_2026062320_1)" >&2
        exit 5
    fi
    echo "[sync] PROVISION(사람 승인됨): mkdir -p $SUB_HOST:$SUB_WORK_DIR"
    $SSH_OPTS "$SUB_HOST" "mkdir -p '$SUB_WORK_DIR'" || { echo "[sync] FAIL: 서브 work_dir 신설 실패"; exit 5; }
fi

echo "[sync] APPLY  $SRC → $SUB_HOST:$DEST"
"${RSYNC[@]}" "$SRC" "$SUB_HOST:$DEST"

# ── 전송 후 체크섬 검증(핵심 빌드 입력) ──
echo "[sync] 체크섬 검증..."
fail=0
for f in output/multi/Dockerfile output/multi/Dockerfile.source-build output/multi/docker-compose.yaml output/multi/requirements.txt; do
    [ -f "${SRC}${f}" ] || { echo "  ⏭  ${f}: 로컬 부재 — 검증 생략(소스빌드/prebuilt 브랜치 차이)"; continue; }
    L=$(md5sum "${SRC}${f}" 2>/dev/null | awk '{print $1}')
    R=$($SSH_OPTS "$SUB_HOST" "md5sum '${SUB_WORK_DIR}/${f}' 2>/dev/null" | awk '{print $1}')
    if [ -n "$L" ] && [ "$L" = "$R" ]; then
        echo "  ✅ ${f}: $L"
    else
        echo "  ❌ ${f}: main=$L sub=$R (불일치)"; fail=1
    fi
done
[ "$fail" -eq 0 ] && echo "[sync] 완료 — 핵심 빌드 입력 체크섬 일치" || { echo "[sync] FAIL: 체크섬 불일치"; exit 2; }
