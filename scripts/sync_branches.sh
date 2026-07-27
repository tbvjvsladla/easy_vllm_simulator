#!/bin/bash
# 브랜치 동기화 — 공유 빌딩블럭을 정본(main) → multi-node 로 복사.
#
# 빌딩블럭(헌법·rules·스킬·docs 스켈레톤·생성 스크립트·매니페스트 템플릿)은 git-tracked 라서
# 브랜치 전환에 persist 되지 않는다(working-dir 가 브랜치 콘텐츠로 바뀜). 두 브랜치(main·multi-node)는
# 이 공유 콘텐츠가 항상 동일해야 하므로, 정본 = main 에서 multi-node 로 명시적으로 복사한다.
# 구현 스켈레톤 중 토폴로지 분기분(멀티 compose/serve)·사용자 실값(config.yaml·*.local.json·manifest.yaml)·
# seed·작업 문서는 동기화하지 않는다(allowlist only).
#
# 방식(SAFE): git 워크트리 인지 복사. multi-node 브랜치 체크아웃 상태에서 `git checkout main -- <경로>` 로
#   정본 콘텐츠를 working-dir 에 가져온다. 커밋은 사람이 한다(자동 커밋 안 함 — HITL).
#
# HITL 안전장치: 기본은 DRY-RUN(미리보기만). 실제 복사는 명시적으로 --apply 를 줘야 한다.
#
# side-effect 인가 게이트 (Phase 3, plan_26072506 vertical slice 2B): 이 스크립트가 무엇이든 손대기
# 전에(소스 브랜치 검증·경로 열거·git diff·특히 git checkout 이전) `scripts/completion_gate.py authorize`
# 를 동기 호출해 --mode experimental(계획+HITL 승인 스코프 manifest) 또는 --mode promotion
# (evidence-complete+PASS 벤치 재실행 검증)을 통과해야 한다. 거부/무효 manifest 는 게이트가 낸 안정
# JSON(또는 그 자체를 못 얻었을 때의 결정론 wrapper)을 그대로 출력하고 비0 종료 — 이 시점까지 git 명령을
# 전혀 실행하지 않았으므로 거부 시 side effect 는 0 이다.
#
# 사용:
#   bash scripts/sync_branches.sh --mode <experimental|promotion> --manifest <path>          # DRY-RUN
#   bash scripts/sync_branches.sh --mode <experimental|promotion> --manifest <path> --apply   # 실제 복사
#   bash scripts/sync_branches.sh --help                                                      # 도움말(레포 확인 없음)
# 환경변수 override: SRC_BRANCH(기본 main) · DST_BRANCH(기본 multi-node).
#
# 트리거 = 수동(사람이 "브랜치 동기화" 지시 / 모든 작업 종료 후 질의). 자동 훅 없음.
set -euo pipefail

SRC_BRANCH="${SRC_BRANCH:-main}"
DST_BRANCH="${DST_BRANCH:-multi-node}"

# ── 공유 allowlist (정본 main → multi-node 복사 대상만) ──
#   주의: 토폴로지 분기 구현체·사용자 실값은 여기 넣지 않는다(브랜치별 독립).
ALLOWLIST=(
    CLAUDE.md
    .claude/rules
    .claude/skills
    .claude/schemas
    .claude/policies
    .gitignore
    tests
    scripts/sync_branches.sh
    scripts/completion_gate.py
    scripts/doc_naming.py
    scripts/evidence_publisher.py
    scripts/harness_verify.py
    scripts/policy_registry.py
    scripts/agent_control.py
    scripts/providers
    scripts/smoke_clone.sh
    scripts/hint_tag.py
    scripts/templates
    scripts/mem_watchdog.sh
    scripts/engine_liveness_watchdog.sh
    scripts/install_host_safety.sh
    scripts/cleanup_docker.py
    scripts/systemd
    scripts/host
    manifest.template.yaml
    .gitattributes
    # immutable historical policy trust source; evidence input only, never an active execution plan
    docs/plan/plan_26062818_RouteB_jasl-fork_SM12x_DeepSeek-V4-Flash_2노드서빙.md
)
# These directory roots are shared control-plane mirrors, not topology-local overlays. A path
# tracked on the destination under one of these roots but absent from the source must be staged for
# deletion; `git checkout <source> -- <dir>` updates existing paths but does not remove such stale
# destination-only files.
MIRROR_DIRS=(
    .claude/rules
    .claude/skills
    .claude/schemas
    .claude/policies
    tests
    scripts/providers
    scripts/templates
    scripts/systemd
    scripts/host
    docs/report
)
#   hint_tag.py·templates = hint 배포 레이어 엔진(빌딩블럭 · 브랜치 동일). hints/index.json·HINTS.md
#   카탈로그 표(구 README 부록)는 생성-데이터(태그에서 재생성 가능)라 여기 미포함 — reindex 재생성/수동
#   git 동기(README 와 동형 취급 · 태그는 브랜치 무관 전역이라 재생성 결과 동일). plan_26070222 §4.
#   build_patches/ 는 여기 없다 — output/<topology>/build_patches/ 통로에 격리(산출물 통로 불변식, single/multi 혼재 차단).
#   토폴로지별 독립이라 cross-branch 동기 대상 아님(서브 전달은 sync_to_sub 가 output/<t>/ 로 함). §4.7 · 3+1+1.
#   docs skeletons are each `example.md`; docs/report is shared as a complete subtree. Both source
#   enumeration and destination-only deletion use NUL-delimited Git output below.

usage() {
    cat <<'EOF'
사용법: sync_branches.sh --mode <experimental|promotion> --manifest <path> [--apply] [--help]

  --mode <experimental|promotion>  completion_gate.py authorize 인가 모드(필수).
                                    실행 전 반드시 이 side-effect 인가를 통과해야 한다(HITL 게이트).
  --manifest <path>                work-manifest JSON 경로(필수). 절대경로 또는 호출 시점
                                    현재 디렉토리 기준 상대경로.
  --apply                          실제 복사 수행(기본은 DRY-RUN 미리보기만).
  --help, -h                       이 도움말을 출력하고 종료(레포/브랜치 확인을 전혀 하지 않음).

환경변수: SRC_BRANCH(기본 main) · DST_BRANCH(기본 multi-node)

예:
  bash scripts/sync_branches.sh --mode promotion --manifest manifests/sync.json
  bash scripts/sync_branches.sh --mode experimental --manifest manifests/sync.json --apply
EOF
}

# ── 인자 파싱(robust — 배열/따옴표만 사용, eval/문자열 재해석 없음) ──
ORIGINAL_CWD="$(pwd)"
GATE_MODE=""
HAVE_MODE=0
MANIFEST_ARG=""
HAVE_MANIFEST=0
SYNC_ACTION="dryrun"

while [ $# -gt 0 ]; do
    case "$1" in
        --help|-h)
            usage
            exit 0
            ;;
        --mode)
            if [ $# -lt 2 ]; then
                echo "[sync-branches] FAIL: --mode 뒤에 값이 필요합니다(experimental|promotion)." >&2
                exit 2
            fi
            GATE_MODE="$2"
            HAVE_MODE=1
            shift 2
            ;;
        --manifest)
            if [ $# -lt 2 ]; then
                echo "[sync-branches] FAIL: --manifest 뒤에 경로가 필요합니다." >&2
                exit 2
            fi
            MANIFEST_ARG="$2"
            HAVE_MANIFEST=1
            shift 2
            ;;
        --apply)
            SYNC_ACTION="apply"
            shift
            ;;
        *)
            echo "[sync-branches] FAIL: 알 수 없는 인자: $1 (도움말: --help)" >&2
            exit 2
            ;;
    esac
done

# ── 스크립트 자신의 온디스크 위치에서 repo root 를 찾는다(호출자 CWD 의존 금지) ──
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
if ! REPO_ROOT="$(git -C "$SCRIPT_DIR" rev-parse --show-toplevel 2>/dev/null)"; then
    echo "[sync-branches] FAIL: 스크립트 위치($SCRIPT_DIR)에서 git 레포를 찾을 수 없습니다." >&2
    exit 3
fi

# ── manifest 경로는 호출 시점 CWD 기준으로 지금(어떤 cd 도 하기 전) 단 한 번, 결정론적으로 해소한다 ──
RESOLVED_MANIFEST=""
if [ "$HAVE_MANIFEST" -eq 1 ]; then
    case "$MANIFEST_ARG" in
        /*) RESOLVED_MANIFEST="$MANIFEST_ARG" ;;
        *) RESOLVED_MANIFEST="$ORIGINAL_CWD/$MANIFEST_ARG" ;;
    esac
fi

if ! command -v python3 >/dev/null 2>&1; then
    echo "[sync-branches] FAIL: python3 를 찾을 수 없어 completion_gate.py authorize 를 실행할 수 없습니다." >&2
    exit 2
fi

cd "$REPO_ROOT"

# ── side-effect 인가 게이트: 소스 브랜치 검증·경로 열거·git diff·git checkout 이전에 동기 호출 ──
GATE_SCRIPT="$REPO_ROOT/scripts/completion_gate.py"
gate_cmd=(python3 "$GATE_SCRIPT" authorize --action sync_branches --repo-root "$REPO_ROOT")
if [ "$HAVE_MANIFEST" -eq 1 ]; then
    gate_cmd+=(--manifest "$RESOLVED_MANIFEST")
fi
if [ "$HAVE_MODE" -eq 1 ]; then
    gate_cmd+=(--mode "$GATE_MODE")
fi

gate_stdout="$("${gate_cmd[@]}")" && gate_exit=0 || gate_exit=$?

gate_allowed="false"
if [ -n "$gate_stdout" ]; then
    gate_allowed="$(printf '%s' "$gate_stdout" | python3 -c '
import json, sys
try:
    obj = json.load(sys.stdin)
    print("true" if obj.get("allowed") is True else "false")
except Exception:
    print("false")
' 2>/dev/null)"
    [ "$gate_allowed" = "true" ] || gate_allowed="false"
fi

if [ "$gate_exit" -ne 0 ] || [ "$gate_allowed" != "true" ]; then
    if [ -n "$gate_stdout" ]; then
        printf '%s\n' "$gate_stdout"
        if [ "$gate_exit" -ne 0 ]; then
            exit "$gate_exit"
        fi
        exit 1
    fi
    # completion_gate.py 자체를 실행/파싱하지 못한 경우(예: 크래시) — 결정론 wrapper 로 대체.
    python3 -c '
import json, sys
gate_mode, manifest_path, exit_code = sys.argv[1], sys.argv[2], sys.argv[3]
mode = gate_mode if gate_mode in ("experimental", "promotion") else None
obj = {
    "schema_version": 1, "mode": mode, "action": "sync_branches", "task_class": None,
    "authorization_state": None, "allowed": False,
    "reason_codes": ["SYNC_BRANCHES_GATE_OUTPUT_UNREADABLE"],
    "messages": {
        "SYNC_BRANCHES_GATE_OUTPUT_UNREADABLE":
            "completion_gate.py authorize (exit=" + exit_code + ") did not produce parseable JSON stdout",
    },
    "identity": None, "exit_code": 2,
}
print(json.dumps(obj, ensure_ascii=False, sort_keys=True, indent=2))
' "$GATE_MODE" "$RESOLVED_MANIFEST" "$gate_exit"
    exit 2
fi

# ── 인가됨 — 여기서부터 기존 로직(변경 없음) ──

# ── pre-flight: git 레포 + 현재 브랜치 = DST_BRANCH ──
if ! git rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "[sync-branches] FAIL: git 레포 안에서 실행해야 합니다."; exit 3
fi
CUR_BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [ "$CUR_BRANCH" != "$DST_BRANCH" ]; then
    echo "[sync-branches] FAIL: 현재 브랜치=$CUR_BRANCH 이지만 대상은 $DST_BRANCH 입니다."
    echo "[sync-branches]   먼저 'git checkout $DST_BRANCH' 후 다시 실행하세요(working-dir 보호)."
    exit 3
fi
if ! git rev-parse --verify "$SRC_BRANCH" >/dev/null 2>&1; then
    echo "[sync-branches] FAIL: 정본 브랜치 '$SRC_BRANCH' 가 없습니다."; exit 3
fi

# ── allowlist 경로를 실제 존재(정본 측) 기준으로 확정 ──
#   git pathspec 으로 main 트리에 실제로 있는 항목만 복사 대상으로 모은다.
PATHS=()
for p in "${ALLOWLIST[@]}"; do
    if git cat-file -e "$SRC_BRANCH:$p" 2>/dev/null \
       || git ls-tree -r --name-only "$SRC_BRANCH" -- "$p" 2>/dev/null | grep -q .; then
        PATHS+=("$p")
    else
        echo "[sync-branches] (skip) 정본 $SRC_BRANCH 에 없음: $p"
    fi
done
# docs skeletons + report artifacts (canonical tree enumeration, NUL-safe for all Git path bytes).
while IFS= read -r -d '' f; do
    case "$f" in
        docs/report/*|docs/*/example.md) PATHS+=("$f") ;;
    esac
done < <(git ls-tree -r -z --name-only "$SRC_BRANCH" -- docs/ 2>/dev/null)

if [ "${#PATHS[@]}" -eq 0 ]; then
    echo "[sync-branches] FAIL: 복사할 allowlist 경로가 정본에 하나도 없습니다."; exit 2
fi

# Resolve every destination-only tracked path before the first mutation. This is an exact mirror
# only for MIRROR_DIRS; topology outputs/configs and ignored work documents are intentionally not
# included. Source object lookup prevents a stale local file from laundering itself as authority.
DELETE_PATHS=()
for mirror_dir in "${MIRROR_DIRS[@]}"; do
    while IFS= read -r -d '' dest_path; do
        [ -n "$dest_path" ] || continue
        if ! git cat-file -e "$SRC_BRANCH:$dest_path" 2>/dev/null; then
            DELETE_PATHS+=("$dest_path")
        fi
    done < <(git ls-files -z -- "$mirror_dir")
done
# docs/report is already covered above. Mirror only skeleton names elsewhere under docs/; active
# plans and other generated documentation remain branch-local and cannot enter DELETE_PATHS.
while IFS= read -r -d '' dest_path; do
    case "$dest_path" in
        docs/report/*) ;;
        docs/*/example.md)
            if ! git cat-file -e "$SRC_BRANCH:$dest_path" 2>/dev/null; then
                DELETE_PATHS+=("$dest_path")
            fi
            ;;
    esac
done < <(git ls-files -z -- docs/)

if [ "$SYNC_ACTION" = "dryrun" ]; then
    echo "[sync-branches] DRY-RUN  $SRC_BRANCH → $DST_BRANCH (working-dir 변경 안 함 — --apply 로 실행)"
    echo "[sync-branches] 동기화 대상(allowlist):"
    printf '  - %s\n' "${PATHS[@]}"
    echo "[sync-branches] 정본과 현재 working-dir 의 차이(없으면 이미 동일):"
    git diff --stat "$SRC_BRANCH" -- "${PATHS[@]}" || true
    if [ "${#DELETE_PATHS[@]}" -gt 0 ]; then
        echo "[sync-branches] source에 없는 destination tracked path (apply 시 삭제 staging):"
        printf '  - %s\n' "${DELETE_PATHS[@]}"
    fi
    echo "[sync-branches] (위는 미리보기. 변경 사항을 사람이 확인 후 --apply)"
    exit 0
fi

# ── apply: 정본 콘텐츠를 working-dir 로 가져온다(커밋은 사람이) ──
echo "[sync-branches] APPLY  $SRC_BRANCH → $DST_BRANCH (working-dir 갱신)"
git checkout "$SRC_BRANCH" -- "${PATHS[@]}"
if [ "${#DELETE_PATHS[@]}" -gt 0 ]; then
    git rm --ignore-unmatch -- "${DELETE_PATHS[@]}"
fi
# Git records only the executable bit; shared-repository umasks can materialize 0775.
# Normalize canonical runner assets so production copies and review exports are exactly 0755.
for runner in arm_patch.sh debug-init.sh serve_runner.sh; do
    runner_path=".claude/skills/upstream-version-watch/assets/configs/$runner"
    [ ! -f "$runner_path" ] || chmod 0755 "$runner_path"
done
echo "[sync-branches] 완료 — 공유 빌딩블럭을 working-dir 에 반영했습니다(스테이징됨)."
echo "[sync-branches] 다음(사람): 변경 검토 후 직접 커밋하세요. 예:"
echo "[sync-branches]   git status && git diff --cached"
echo "[sync-branches]   git commit -m 'sync: 공유 빌딩블럭 main→$DST_BRANCH'"
echo "[sync-branches] (이 스크립트는 자동 커밋하지 않습니다 — HITL.)"
