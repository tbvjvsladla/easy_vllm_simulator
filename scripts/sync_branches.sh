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
# 사용:
#   bash scripts/sync_branches.sh          # DRY-RUN (main→multi-node 무엇이 바뀔지 미리보기)
#   bash scripts/sync_branches.sh --apply  # 실제 복사(working-dir 변경) — 커밋은 사람이 직접
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
    scripts/sync_branches.sh
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
)
#   hint_tag.py·templates = hint 배포 레이어 엔진(빌딩블럭 · 브랜치 동일). hints/index.json·README 부록은
#   생성-데이터(태그에서 재생성 가능)라 여기 미포함 — P4서 재생성/수동. plan_2026070222_1 §4.
#   build_patches/ 는 여기 없다 — output/<topology>/build_patches/ 통로에 격리(산출물 통로 불변식, single/multi 혼재 차단).
#   토폴로지별 독립이라 cross-branch 동기 대상 아님(서브 전달은 sync_to_sub 가 output/<t>/ 로 함). §4.7 · 3+1+1.
#   docs 스켈레톤은 각 폴더 example.md 만(작업 문서 본체는 제외).
DOCS_GLOB="docs/*/example.md"

MODE="dryrun"
[ "${1:-}" = "--apply" ] && MODE="apply"

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
# docs example.md 스켈레톤(글롭 확장은 git 트리 기준)
while IFS= read -r f; do
    [ -n "$f" ] && PATHS+=("$f")
done < <(git ls-tree -r --name-only "$SRC_BRANCH" -- "$DOCS_GLOB" 2>/dev/null || true)

if [ "${#PATHS[@]}" -eq 0 ]; then
    echo "[sync-branches] FAIL: 복사할 allowlist 경로가 정본에 하나도 없습니다."; exit 2
fi

if [ "$MODE" = "dryrun" ]; then
    echo "[sync-branches] DRY-RUN  $SRC_BRANCH → $DST_BRANCH (working-dir 변경 안 함 — --apply 로 실행)"
    echo "[sync-branches] 동기화 대상(allowlist):"
    printf '  - %s\n' "${PATHS[@]}"
    echo "[sync-branches] 정본과 현재 working-dir 의 차이(없으면 이미 동일):"
    git diff --stat "$SRC_BRANCH" -- "${PATHS[@]}" || true
    echo "[sync-branches] (위는 미리보기. 변경 사항을 사람이 확인 후 --apply)"
    exit 0
fi

# ── apply: 정본 콘텐츠를 working-dir 로 가져온다(커밋은 사람이) ──
echo "[sync-branches] APPLY  $SRC_BRANCH → $DST_BRANCH (working-dir 갱신)"
git checkout "$SRC_BRANCH" -- "${PATHS[@]}"
echo "[sync-branches] 완료 — 공유 빌딩블럭을 working-dir 에 반영했습니다(스테이징됨)."
echo "[sync-branches] 다음(사람): 변경 검토 후 직접 커밋하세요. 예:"
echo "[sync-branches]   git status && git diff --cached"
echo "[sync-branches]   git commit -m 'sync: 공유 빌딩블럭 main→$DST_BRANCH'"
echo "[sync-branches] (이 스크립트는 자동 커밋하지 않습니다 — HITL.)"
