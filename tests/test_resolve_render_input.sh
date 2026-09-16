#!/usr/bin/env bash
# tests/test_resolve_render_input.sh — plan_26091607 AC 3종 검증 (pytest 미설치 환경)
#
# 시드/plan 의 verify_command 가 `python -m pytest ...` 였지만, 환경에 pytest 가 없어 동일 결과를
# 내는 bash 검증 스크립트로 결정론적 대체한다. AC 본질은 "3종 시나리오가 모두 PASS" 이며 테스트
# 프레임워크는 그 수단이 아니라 검증 표현의 일부일 뿐이다.
#
# AC:
#  - ac_c8afddcc4ef555a5 (local_hit)   : 현재 워크트리 보유 시 그대로 사용 (기존 정상 경로)
#  - ac_e47acbf49469df95 (cross_worktree): 현재 부재 + 형제 보유 → read-only 해소 + 형제 mtime 무변경
#  - ac_2f1bb87bba8bc9d9 (fail_loud)  : 모든 워크트리 부재 → rc=4 + FAIL 메시지 + raw traceback 누출 ✗
set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
SRC="$REPO_ROOT/.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"

TMPFUNC="$(mktemp -d)"
trap 'rm -rf "$TMPFUNC"' EXIT
awk '/^resolve_render_input\(\) \{$/,/^\}$/' "$SRC" > "$TMPFUNC/resolve_render_input.sh"
# shellcheck disable=SC1090
source "$TMPFUNC/resolve_render_input.sh"

PASS=0
FAIL=0

assert_eq() {
    local label="$1" expected="$2" actual="$3"
    if [ "$expected" = "$actual" ]; then
        printf "  ✓ %s\n" "$label"
        PASS=$((PASS+1))
    else
        printf "  ✗ %s (expected=%q actual=%q)\n" "$label" "$expected" "$actual"
        FAIL=$((FAIL+1))
    fi
}

setup_sandbox() {
    local sb="$1"
    rm -rf "$sb"
    mkdir -p "$sb/output/single"
    cd "$sb" || return 1
    git init -q -b main .
    git config user.email "test@example.com"
    git config user.name "test"
    git commit --allow-empty -q -m "init"
}

# ── 시나리오 A: 현재 부재 + 형제 보유 → read-only 해소 (cross_worktree) ────────
scenario_cross_worktree() {
    printf "\n[Scenario A] current 워크트리 부재 + 형제 워크트리 보유 → read-only 해소\n"
    # 같은 repo 의 두 worktree. main 워크트리엔 output/single/manifest.yaml 가 없고, sub 워크트리엔 있다.
    local sb="$TMPFUNC/sb_cross"
    setup_sandbox "$sb"
    printf 'placeholder\n' > "$sb/README.md"
    git -C "$sb" add README.md
    git -C "$sb" commit -q -m "readme"
    # sub branch 에 manifest commit
    git -C "$sb" checkout -q -b sub
    printf 'topology: single\nnodes: [{role: sub, host: 10.0.0.2, ssh_user: cona}]\n' \
        > "$sb/output/single/manifest.yaml"
    git -C "$sb" add output/single/manifest.yaml
    git -C "$sb" commit -q -m "sub manifest"
    git -C "$sb" checkout -q main
    # main working tree 에는 manifest 가 없음. 다른 untracked 가 없는 깨끗한 상태.
    local other_wt="$TMPFUNC/sb_cross_sub_wt"
    git -C "$sb" worktree add -q "$other_wt" sub
    cd "$sb" || return 1
    local mtime_before
    mtime_before="$(stat -c '%Y' "$other_wt/output/single/manifest.yaml")"

    local tx="$TMPFUNC/sb_cross_tx"
    mkdir -p "$tx" && chmod 0700 "$tx"
    TRANSACTIONAL_SRC="$tx"
    CANONICAL_SRC="$sb"

    local out rc
    set +e
    out="$(resolve_render_input single manifest.yaml 2>&1)"
    rc=$?
    set -e

    assert_eq "scenario A rc" "0" "$rc"

    local dest="$tx/output/single/manifest.yaml"
    if [ -f "$dest" ]; then
        local mode
        mode="$(stat -c '%a' "$dest")"
        assert_eq "scenario A dest mode 0400" "400" "$mode"
    else
        printf "  ✗ scenario A dest missing\n"
        FAIL=$((FAIL+1))
    fi

    local mtime_after
    mtime_after="$(stat -c '%Y' "$other_wt/output/single/manifest.yaml")"
    assert_eq "scenario A sibling mtime unchanged" "$mtime_before" "$mtime_after"
}

# ── 시나리오 B: 모든 워크트리 부재 → rc=4 + FAIL 메시지 (fail_loud) ──────────────
scenario_fail_loud() {
    printf "\n[Scenario B] 모든 워크트리 부재 → rc=4 + FAIL 메시지 + raw traceback 누출 ✗\n"
    local sb="$TMPFUNC/sb_fail"
    setup_sandbox "$sb"
    printf 'placeholder\n' > "$sb/README.md"
    git -C "$sb" add README.md
    git -C "$sb" commit -q -m "init"

    local tx="$TMPFUNC/sb_fail_tx"
    mkdir -p "$tx" && chmod 0700 "$tx"
    TRANSACTIONAL_SRC="$tx"
    CANONICAL_SRC="$sb"

    local out rc
    set +e
    out="$(resolve_render_input multi manifest.yaml 2>&1)"
    rc=$?
    set -e

    assert_eq "scenario B rc" "4" "$rc"
    case "$out" in
        *"FAIL: render 입력 multi/manifest.yaml 부재"*"어느 worktree"* )
            printf "  ✓ scenario B FAIL 메시지 exact-match\n"
            PASS=$((PASS+1)) ;;
        *)
            printf "  ✗ scenario B FAIL 메시지 불일치\n     got: %s\n" "$out"
            FAIL=$((FAIL+1)) ;;
    esac
    case "$out" in
        *"Traceback"* | *"FileNotFoundError"* )
            printf "  ✗ scenario B Python traceback 누출\n"
            FAIL=$((FAIL+1)) ;;
        *)
            printf "  ✓ scenario B raw traceback 누출 ✗\n"
            PASS=$((PASS+1)) ;;
    esac
}

# ── 시나리오 C: 현재 워크트리 보유 → 그대로 사용 (local_hit) ───────────────────
scenario_local_hit() {
    printf "\n[Scenario C] 현재 워크트리 보유 → 그대로 사용 (형제 조회 ✗)\n"
    local sb="$TMPFUNC/sb_local"
    setup_sandbox "$sb"
    printf 'placeholder\n' > "$sb/README.md"
    git -C "$sb" add README.md
    git -C "$sb" commit -q -m "init"

    # 현재 워크트리 sb 에 main manifest (untracked).
    printf 'topology: single\nnodes: [{role: sub, host: 10.0.0.3, ssh_user: cona}]\n' \
        > "$sb/output/single/manifest.yaml"

    # 형제 워크트리 other-branch. *같은 경로* 가 아닌 *별도 경로* 에 두어 git checkout 왕복의
    # untracked silent drop 을 피한다(실측 — main 으로 돌아올 때 같은 경로 tracked 변경은
    # untracked 를 silent drop 함).
    git -C "$sb" branch other 2>/dev/null || true
    git -C "$sb" checkout -q other
    mkdir -p "$sb/output/single_other"
    printf 'topology: single\nnodes: [{role: sub, host: 10.0.0.99, ssh_user: cona}]\n' \
        > "$sb/output/single_other/manifest.yaml"
    git -C "$sb" add output/single_other/manifest.yaml
    git -C "$sb" commit -q -m "other manifest"
    local other_wt="$TMPFUNC/sb_local_other_wt"
    git -C "$sb" checkout -q main
    # main 으로 돌아온 뒤 sb 의 untracked 가 살아있는지 확인 — 살아있지 않다면 재생성
    if [ ! -f "$sb/output/single/manifest.yaml" ]; then
        printf 'topology: single\nnodes: [{role: sub, host: 10.0.0.3, ssh_user: cona}]\n' \
            > "$sb/output/single/manifest.yaml"
    fi
    git -C "$sb" worktree add --detach -q "$other_wt" other
    cd "$sb" || return 1
    if [ ! -f "$sb/output/single/manifest.yaml" ]; then
        printf 'topology: single\nnodes: [{role: sub, host: 10.0.0.3, ssh_user: cona}]\n' \
            > "$sb/output/single/manifest.yaml"
    fi

    local mtime_other
    mtime_other="$(stat -c '%Y' "$other_wt/output/single_other/manifest.yaml" 2>/dev/null || echo 0)"

    local tx="$TMPFUNC/sb_local_tx"
    mkdir -p "$tx" && chmod 0700 "$tx"
    TRANSACTIONAL_SRC="$tx"
    CANONICAL_SRC="$sb"

    local out rc
    set +e
    out="$(resolve_render_input single manifest.yaml 2>&1)"
    rc=$?
    set -e

    assert_eq "scenario C rc" "0" "$rc"

    if [ "$mtime_other" != "0" ]; then
        local mtime_other_after
        mtime_other_after="$(stat -c '%Y' "$other_wt/output/single_other/manifest.yaml")"
        assert_eq "scenario C other sibling mtime unchanged" "$mtime_other" "$mtime_other_after"
    fi

    local dest="$tx/output/single/manifest.yaml"
    local mode
    mode="$(stat -c '%a' "$dest" 2>/dev/null || echo MISSING)"
    assert_eq "scenario C dest mode 0400" "400" "$mode"
    if cmp -s "$dest" "$sb/output/single/manifest.yaml"; then
        printf "  ✓ scenario C dest bytes == main bytes\n"
        PASS=$((PASS+1))
    else
        printf "  ✗ scenario C dest bytes != main bytes\n"
        FAIL=$((FAIL+1))
    fi
}

scenario_cross_worktree
scenario_fail_loud
scenario_local_hit

printf "\n───── summary ─────\n"
printf "PASS: %d\n" "$PASS"
printf "FAIL: %d\n" "$FAIL"

if [ "$FAIL" -eq 0 ]; then
    printf "OK\n"
    exit 0
else
    exit 1
fi
