#!/bin/bash
# 서브노드 → 메인노드 **문서기반 회수** 미러 (D12 상향 채널).
#
# 양방향 브랜치싱크의 [상향] 단계. 서브가 자기개선 insight 를 docs 규약으로 발행하면, 메인이
# 서브 docs/ 만 로컬 gitignored 미러로 rsync 해 **열람**한다. 그 뒤 메인이 HITL 로 자기 템플릿/헌법/스킬에
# 재저작한다(자동 머지 없음). 이것이 유일한 상향 채널 — patch/bundle/코드 추출 없음(plan_2026062411_1 D12-06).
#
# 경계(A2A): 이건 "서브 디스크 재스캔"이 아니라 **서브가 발행한 docs/ 만** 가져오는 것이다(서브가 가리킨 산출물).
#            작업코드/설정(configs/envs/.claude/CLAUDE.md)은 가져오지 않는다 — 회수는 문서기반 only.
#
# 방향: 서브 → 메인 (read-only on sub — rsync FROM sub). 서브를 변경하지 않는다.
# 산출: sync_staging/sub_docs/ (메인 로컬, gitignored — 회수 작업본). 그 뒤 사람이 읽고 재저작.
#
# 사용:
#   bash fetch_sub_docs.sh                 # DRY-RUN (무엇을 가져올지 미리보기)
#   bash fetch_sub_docs.sh --apply         # 실제 미러
#   bash fetch_sub_docs.sh --apply --with-simlog-raw   # 대용량 simlog 원시로그도 포함(기본 제외)
# 환경변수 override: SUB_HOST(<ssh_user>@<host>) · SUB_WORK_DIR · DEST(기본 sync_staging/sub_docs).
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)/}"

# ── SUB_HOST/SUB_WORK_DIR 해소: 환경변수 우선, 없으면 manifest.yaml(output/multi 통로) nodes[] (role: sub) ──
# 서브 접속정보는 항상 multi 통로 manifest 에 있다(single manifest 는 nodes:[] — 서브 미정의).
_resolve_sub_host_from_manifest() {
    local manifest="${SRC%/}/output/multi/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; host=""; user=""; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*host:/    { sub(/^[[:space:]]*host:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); host=$0 }
        in_sub && /^[[:space:]]*ssh_user:/{ sub(/^[[:space:]]*ssh_user:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); user=$0 }
        in_sub && host != "" && user != "" { print user "@" host; exit }
    ' "$manifest"
}
_resolve_sub_work_dir_from_manifest() {
    local manifest="${SRC%/}/output/multi/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*work_dir:/ { sub(/^[[:space:]]*work_dir:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); print; exit }
    ' "$manifest"
}

[ -z "${SUB_HOST:-}" ]     && SUB_HOST="$(_resolve_sub_host_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="$(_resolve_sub_work_dir_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="${SRC%/}"
if [ -z "${SUB_HOST:-}" ]; then
    echo "[fetch] FAIL: 서브노드 주소 미해소 — SUB_HOST(<ssh_user>@<host>) 지정 또는 manifest nodes[](role:sub) 채우기." >&2
    exit 4
fi

MODE="dryrun"; WITH_SIMLOG_RAW=0
for a in "$@"; do
    [ "$a" = "--apply" ]             && MODE="apply"
    [ "$a" = "--with-simlog-raw" ]   && WITH_SIMLOG_RAW=1
done

DEST="${DEST:-${SRC%/}/sync_staging/sub_docs}"
SSH_OPTS="ssh -o BatchMode=yes -o ConnectTimeout=8"
# 대용량 simlog 원시 trial 로그는 기본 제외(회수는 사람이 읽는 종합문서가 목적 — 빌드/원시증거 불요).
EXCLUDES=(--exclude '__pycache__' --exclude '*.pyc')
[ "$WITH_SIMLOG_RAW" -eq 0 ] && EXCLUDES+=(--exclude 'simlog/*/trial*' --exclude 'simlog/*/*.log')

# ── pre-flight: SSH 도달성 ──
if ! $SSH_OPTS "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    echo "[fetch] FAIL: $SUB_HOST 에 SSH 불가 (키 인증·네트워크 확인)"; exit 3
fi
# 서브 docs/ 존재 확인(없으면 회수할 것 없음 — 정상 종료)
if ! $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR/docs' ]" 2>/dev/null; then
    echo "[fetch] (info) 서브에 docs/ 없음 — 회수할 발행문서 없음. ($SUB_HOST:$SUB_WORK_DIR/docs)"; exit 0
fi

if [ "$MODE" = "dryrun" ]; then
    echo "[fetch] DRY-RUN  $SUB_HOST:$SUB_WORK_DIR/docs/ → $DEST/  (read-only on sub · 실제 미러 안 함 — --apply 로 실행)"
    rsync -an --itemize-changes "${EXCLUDES[@]}" -e "$SSH_OPTS" "$SUB_HOST:$SUB_WORK_DIR/docs/" "$DEST/" 2>&1 || true
    echo "[fetch] (위는 미리보기 — 서브 docs/ 만 가져온다. 코드/설정 미회수. 사람이 확인 후 --apply)"
    exit 0
fi

mkdir -p "$DEST"
echo "[fetch] APPLY  $SUB_HOST:$SUB_WORK_DIR/docs/ → $DEST/  (서브 read-only)"
rsync -az "${EXCLUDES[@]}" -e "$SSH_OPTS" "$SUB_HOST:$SUB_WORK_DIR/docs/" "$DEST/"
n=$(find "$DEST" -type f 2>/dev/null | wc -l | tr -d ' ')
echo "[fetch] 미러 완료 — $DEST ($n 파일)."
echo "[fetch] 다음(사람): 이 문서를 열람 → 반영할 개선을 메인 템플릿/헌법/스킬에 HITL 재저작(자동 머지 없음)."
