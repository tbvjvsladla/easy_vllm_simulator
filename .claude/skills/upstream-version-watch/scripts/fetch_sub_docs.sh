#!/bin/bash
# 서브노드 → 메인노드 **문서기반 회수** 미러 (D12 상향 채널).
#
# 양방향 브랜치싱크의 [상향] 단계. 서브가 자기개선 insight 를 docs 규약으로 발행하면, 메인이
# 서브 docs/ 만 로컬 gitignored 미러로 rsync 해 **열람**한다. 그 뒤 메인이 HITL 로 자기 템플릿/헌법/스킬에
# 재저작한다(자동 머지 없음). 이것이 유일한 상향 채널 — patch/bundle/코드 추출 없음(plan_26062411 D12-06).
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

# ── SUB_HOST/SUB_WORK_DIR 해소: 환경변수 우선, 없으면 manifest nodes[] (role: sub) ──
# 2026-09-03(P3 · plan_26090317): 옛 주석은 "서브 접속정보는 **항상** multi 통로 manifest 에 있다
#   (single manifest 는 nodes:[] — 서브 미정의)" 였다. 그 전제가 §2.7.0(single+sub = A2A 에이전트)
#   신설로 깨졌는데 이 파일은 따라오지 않아, 싱글 구성에서 상향 회수 채널이 통째로 죽어 있었다
#   (그것도 조용히 — `exit 0` "회수할 것 없음" 으로). 두 통로를 모두 보고, 값이 갈리면 fail-closed.
_resolve_sub_host_from_manifest() {   # $1=topology
    local manifest="${SRC%/}/output/${1:-multi}/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; host=""; user=""; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*host:/    { sub(/^[[:space:]]*host:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); host=$0 }
        in_sub && /^[[:space:]]*ssh_user:/{ sub(/^[[:space:]]*ssh_user:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); user=$0 }
        in_sub && host != "" && user != "" { print user "@" host; exit }
    ' "$manifest"
}
_resolve_sub_work_dir_from_manifest() {   # $1=topology
    local manifest="${SRC%/}/output/${1:-multi}/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; next }
        /^[[:space:]]*-[[:space:]]*role:/             { in_sub=0 }
        in_sub && /^[[:space:]]*work_dir:/ { sub(/^[[:space:]]*work_dir:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); print; exit }
    ' "$manifest"
}

# 2026-09-05(N1 · ②-b 라이브): 종전 규칙은 "두 통로의 값이 다르면 상태가 잘못됐다" 였다. 그런데
#   **같은 노드가 통로마다 다른 주소로 잡히는 것은 정상**이다 — 싱글은 관리 hostname(A2A 평면),
#   멀티는 RoCE IP(집단연산 평면). 재설치 회수 때 실제로 그 상태를 만나 회수가 막혔고, 나는
#   `SUB_HOST` override 로 우회했다(스크립트가 문서화한 통로였지만, 정상 상태에서 override 를
#   요구하는 게이트는 사람에게 우회를 가르친다 · D3).
#   교정: **어느 통로에서 회수할지 선언**하면 그 통로만 본다. 선언이 없고 두 통로가 갈릴 때만
#   멈추되, 그때도 "상태가 틀렸다"가 아니라 "무엇을 쓸지 정하라"고 말한다.
FETCH_TOPOLOGY="${FETCH_TOPOLOGY:-}"
for _a in "$@"; do
    case "$_a" in --topology=*) FETCH_TOPOLOGY="${_a#*=}" ;; esac
done
case "$FETCH_TOPOLOGY" in
    ""|single|multi) ;;
    *) echo "[fetch] FAIL: --topology 는 single|multi (받은 값: $FETCH_TOPOLOGY)" >&2; exit 4 ;;
esac

_fetch_resolve() {   # $1=host|work_dir → 선언된 통로, 없으면 두 통로(갈리면 선언 요구)
    local kind="$1" t v prev="" src=""
    for t in ${FETCH_TOPOLOGY:-single multi}; do
        if [ "$kind" = "host" ]; then v="$(_resolve_sub_host_from_manifest "$t" || true)"
        else v="$(_resolve_sub_work_dir_from_manifest "$t" || true)"; fi
        [ -n "$v" ] || continue
        if [ -n "$prev" ] && [ "$v" != "$prev" ]; then
            echo "[fetch] FAIL: 두 통로가 서로 다른 서브 $kind 를 가리킨다 — $src=$prev vs output/$t=$v" >&2
            echo "        같은 노드의 다른 평면일 수 있다(싱글=관리 hostname · 멀티=RoCE IP)." >&2
            echo "        어느 통로에서 회수할지 **선언**하라: --topology=single|multi" >&2
            echo "        (또는 SUB_HOST/SUB_WORK_DIR 를 직접 지정한다)" >&2
            return 2
        fi
        prev="$v"; src="output/$t"
    done
    [ -n "$prev" ] || return 1
    printf '%s' "$prev"
}
if [ -z "${SUB_HOST:-}" ]; then
    SUB_HOST="$(_fetch_resolve host)" || { [ $? = 2 ] && exit 4; SUB_HOST=""; }
fi
if [ -z "${SUB_WORK_DIR:-}" ]; then
    SUB_WORK_DIR="$(_fetch_resolve work_dir)" || { [ $? = 2 ] && exit 4; SUB_WORK_DIR=""; }
fi
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
# 회수 범위 = 서브 docs/ 문서 평면 전체(2026-09-05 · plan_26090516 §7.4 노드 오케스트레이터 publish Phase):
#   plan · devlog · testlog · benchmark(인증서 yaml · bench_report · sweep map · **hint 입력 사이드카**
#   docs/benchmark/hint_inputs_<measured_utc>/) — 메인은 이 문서들로 devlog·benchmark 를 저작하고 hint 를 발행한다.
#   output/** (렌더 산출물·raw 로그·이미지)은 회수 경로가 없다 — 문서기반 불변식(헌법). simlog raw 는 --with-simlog-raw 일 때만.
EXCLUDES=(--exclude '__pycache__' --exclude '*.pyc')
[ "$WITH_SIMLOG_RAW" -eq 0 ] && EXCLUDES+=(--exclude 'simlog/*/trial*' --exclude 'simlog/*/*.log')

# ── pre-flight: SSH 도달성 ──
if ! $SSH_OPTS "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    echo "[fetch] FAIL: $SUB_HOST 에 SSH 불가 (키 인증·네트워크 확인)"; exit 3
fi
# 서브 docs/ 존재 확인(없으면 회수할 것 없음 — 정상 종료)
# 2026-09-03(F6 · plan_26090317): rc 를 구분하지 않아 **전송 실패가 "docs 없음"** 이 됐다 —
#   유일한 서브→메인 채널이 판독 실패에 대고 "회수할 것 없음(성공)" 이라고 답했다.
_docs_rc=0
$SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR/docs' ]" || _docs_rc=$?
case "$_docs_rc" in
    0) : ;;
    1) echo "[fetch] (info) 서브에 docs/ 없음 — 회수할 발행문서 없음. ($SUB_HOST:$SUB_WORK_DIR/docs)"; exit 0 ;;
    *) echo "[fetch] FAIL: 서브 docs/ 존재 여부 판독 불가(rc=$_docs_rc) — '없음' 으로 접지 않는다." >&2; exit 3 ;;
esac

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

# ── 캠페인 편입 (2026-09-08 · plan_26090813 D13) ───────────────────────────────────────────
# 왜 여기인가: 종전에는 서브가 완주해도 메인 인스턴스의 `phases/sub/*`·`cells/*`·증거 포인터를
# **사람이** 적었고, 2026-09-07 에 그 4개가 캠페인 종료 뒤 22:34:11Z 같은 초에 나타났다. 그러면
# 진행표는 관측이 아니라 회고이고, 감독자가 읽어도 아무것도 알 수 없다. 회수가 끝난 그 자리에서
# 편입하면 그 재저작이 사라지고, 회수분은 **서브 저작**(authored_by=<서브 node_id>)으로 남는다.
#
# 상향 회수는 문서기반이다(헌법 노드제어 ①) — 편입기가 읽는 것은 방금 만든 이 미러뿐이다.
# ACTIVE=_bootstrap 이면 writer 가 스스로 no-op 이므로 캠페인 밖 회수는 영향이 없다.
_CI="${SRC%/}/.claude/skills/terraforming_node/scripts/campaign_init.py"
_NOW="$(date -u +%FT%TZ)"
if [ -f "$_CI" ]; then
    if python3 "$_CI" --import-sub "$DEST" --utc "$_NOW"; then
        echo "[fetch] 캠페인 편입 완료 — 서브 진행표·셀·증거가 메인 인스턴스에 앉았다."
    else
        echo "[fetch] ⚠ 캠페인 편입 실패 — 미러는 있고 진행표는 없다(위 사유 참조)." >&2
    fi
    # 합격 술어(P4 동시 착수 타임라인 · P5 서브 자기저작)는 **회수 뒤**에 물어야 답이 있다.
    # 판정만 하고 차단하지 않는다 — 진행을 막는 자리와 판정하는 자리는 분리돼 있다(plan §9 R3).
    _CV="${SRC%/}/.claude/skills/terraforming_node/scripts/campaign_template_validator.py"
    _CAMP="$(python3 "$_CI" --active 2>/dev/null || echo _bootstrap)"
    if [ -f "$_CV" ] && [ "$_CAMP" != "_bootstrap" ]; then
        echo "[fetch] 합격 술어(P4·P5) 판정 — 차단하지 않는다(관측 결과만 남긴다):"
        python3 "$_CV" --acceptance "${SRC%/}/campaigns/$_CAMP" || true
    fi
else
    echo "[fetch] ⚠ campaigns writer 부재($_CI) — 회수는 됐지만 아무도 편입하지 않았다." >&2
fi

echo "[fetch] 다음(사람): 이 문서를 열람 → 반영할 개선을 메인 템플릿/헌법/스킬에 HITL 재저작(자동 머지 없음)."
