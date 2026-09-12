#!/usr/bin/env bash
# closing_sequence.sh — 종료 명령("커밋 → 브랜치 동기화 → 서브 전파")의 **결정론 실행자**
#
# policy:BRANCH_CONSTITUTION_LAYERING · 근거 docs/plan/plan_26091210 §3.6
#
# 이 스크립트가 하는 것과 하지 않는 것
# ------------------------------------
# 이 시퀀스에는 확률론 한 걸음과 사람 한 걸음이 섞여 있다. 이 파일은 **그 둘을 하지 않는다**:
#
#   ⓪ 전제 확인      ← 여기(preflight)
#   ① 공통층 변경 열거 ← 여기(preflight) — 분류표의 **빈칸**을 만든다
#   ② 분류            ← Agent. 파일마다 common_promote / branch_only 를 판정하고 근거를 적는다
#   ③ 승인            ← 사람. 분류표를 보고 1회 승인한다. **이 시퀀스의 유일한 사람 게이트**
#   ④~⑦ 집행         ← 여기(execute) — 원장·커밋·동기화·서브 전파·push. 추가 프롬프트 없음
#
# 절차(②③)의 정본은 `upstream-version-watch` SKILL.md §종료 시퀀스다. 여기서 그 판정을 흉내내지
# 않는 이유: 확률론 판단을 결정론 스크립트에 넣으면 **판정의 출처가 사라진다**. 분류표는 항상
# `provenance: agent-judged` 를 달고 원장에 남아야 하고, 그래야 나중에 누가 무엇을 승인했는지 읽힌다.
#
# 사용법
#   closing_sequence.sh preflight --repo <경로>
#   closing_sequence.sh execute --repo <경로> --entry-file <분류표.json> \
#                       --mode <experimental|promotion> --manifest <work.json> \
#                       [--remote <원격>] [--no-push] [--no-sub]
#
# 종료코드: 0 정상 · 2 사용법 · 3 전제 실패 · 5 집행 실패
set -euo pipefail

SELF_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" >/dev/null 2>&1 && pwd -P)"
REPO=""
ACTION=""
ENTRY_FILE=""
GATE_MODE=""
WORK_MANIFEST=""
REMOTE="github-ssh"
DO_PUSH=1
DO_SUB=1

usage() { sed -n '2,28p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//'; }

[ $# -gt 0 ] || { usage; exit 2; }
ACTION="$1"; shift
case "$ACTION" in preflight|execute) ;; --help|-h) usage; exit 0 ;; *) echo "[closing] FAIL: 알 수 없는 동작: $ACTION" >&2; exit 2 ;; esac

while [ $# -gt 0 ]; do
    case "$1" in
        --repo)       REPO="$2"; shift 2 ;;
        --entry-file) ENTRY_FILE="$2"; shift 2 ;;
        --mode)       GATE_MODE="$2"; shift 2 ;;
        --manifest)   WORK_MANIFEST="$2"; shift 2 ;;
        --remote)     REMOTE="$2"; shift 2 ;;
        --no-push)    DO_PUSH=0; shift ;;
        --no-sub)     DO_SUB=0; shift ;;
        --help|-h)    usage; exit 0 ;;
        *) echo "[closing] FAIL: 알 수 없는 인자: $1" >&2; exit 2 ;;
    esac
done

REPO="$(cd -- "${REPO:-$SELF_DIR/../../../..}" >/dev/null 2>&1 && pwd -P)"
PARITY="$REPO/.claude/skills/terraforming_node/scripts/topology_parity.py"
LEDGER="$REPO/.claude/skills/upstream-version-watch/scripts/layer_ledger.py"
SYNC="$REPO/.claude/skills/upstream-version-watch/scripts/sync_branches.sh"
SUBSYNC="$REPO/.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh"
PUSHER="$REPO/.claude/skills/upstream-version-watch/scripts/push_branches.py"
CATALOG="$REPO/.claude/skills/hint-publisher/scripts/hint_catalog.py"

CUR="$(git -C "$REPO" symbolic-ref --quiet --short HEAD || true)"
case "$CUR" in
    single-node) OTHER="multi-node" ;;
    multi-node)  OTHER="single-node" ;;
    *) echo "[closing] FAIL: 출발 브랜치가 운영 브랜치가 아니다: '${CUR:-detached}'" >&2; exit 3 ;;
esac

# ── ⓪ 전제 ────────────────────────────────────────────────────────────────────────
preflight() {
    local rc=0
    echo "[closing] ⓪ 전제 — 출발=$CUR · 반대=$OTHER"

    if ! python3 "$PARITY" evaluate --repo "$REPO" >/dev/null 2>&1; then
        echo "[closing] FAIL: 4자일치가 RED 다. 이 상태로는 무엇을 어느 층에 넣을지 판정할 수 없다." >&2
        python3 "$PARITY" evaluate --repo "$REPO" 2>&1 | head -40 >&2 || true
        rc=3
    else
        echo "[closing]   4자일치 PASS"
    fi

    # 잔존 워크트리 = 중단된 시퀀스의 흔적. 이어서 돌면 그 위에 덮어쓴다.
    local stale
    stale="$(git -C "$REPO" worktree list --porcelain | awk '/^worktree /{print $2}' | grep -v "^$REPO$" || true)"
    if [ -n "$stale" ]; then
        echo "[closing] FAIL: 잔존 워크트리가 있다 — 중단된 시퀀스일 수 있다:" >&2
        printf '  %s\n' "$stale" >&2
        echo "[closing]   확인 후 `git worktree remove` 로 정리하고 다시 실행하라." >&2
        rc=3
    else
        echo "[closing]   잔존 워크트리 없음"
    fi

    echo "[closing] ① 공통층 변경 열거"
    local sp
    sp="$(python3 "$LEDGER" sync-point --repo "$REPO" --branch "$CUR" 2>/dev/null || echo '{}')"
    printf '%s\n' "$sp" | sed 's/^/  /'
    return $rc
}

emit_skeleton() {
    # 분류표의 **빈칸**을 만든다. 판정(verdict)·근거(reason)는 비워 둔다 — 채우는 것은 Agent 의 일이고,
    # 여기서 기본값을 넣으면 그 기본값이 판정인 척하게 된다(침묵 폴백 금지).
    python3 - "$REPO" "$CUR" "$LEDGER" "$OTHER" <<'PY'
import json, subprocess, sys
repo, cur, ledger, other = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]

def git(*a):
    p = subprocess.run(["git", "-C", repo, *a], capture_output=True, text=True)
    return p.stdout if p.returncode == 0 else ""

sp = {}
try:
    sp = json.loads(subprocess.run([sys.executable, ledger, "sync-point", "--repo", repo,
                                    "--branch", cur], capture_output=True, text=True).stdout or "{}")
except Exception:
    pass
base = sp.get("commit")
spec = json.loads(subprocess.run([sys.executable, ledger, "common-layer", "--repo", repo],
                                 capture_output=True, text=True).stdout or "{}")
# 도달범위는 "동기 지점 이후 내가 무엇을 바꿨나"가 아니라 **"이 동기화가 무엇을 바꾸나"**다.
# 둘은 같지 않다 -- 부트스트랩(동기 지점 없음)에서 전자는 계산조차 불가능하고, 종전 구현은
# `HEAD~1..HEAD` 로 물러서서 **마지막 커밋 한 건만** 열거했다(2026-09-12 실측: 실제 이동 20파일 중
# 1건만 나왔다). 분류표가 이동분을 덜 보여 주면 ③ 사람 게이트는 보지 못한 것을 승인하게 되고,
# 그러면 게이트가 이름만 남는다.
#
# 반대 브랜치와의 **트리 차이**는 두 경우 모두에서 정확히 이동분이다 -- 바뀌었다 되돌아온 파일을
# 자동으로 빼고, 대상 브랜치만 움직인 파일(= 이 동기화가 되돌려 버릴 것)을 드러낸다. 동기 지점은
# 여전히 조회해 `_reach` 에 적는다(사람이 "어디서부터인가"를 읽을 수 있어야 한다).
paths = ((spec.get("include") or []) + ["docs/benchmark"]
         + (spec.get("exclude") or []) + [":(exclude)docs/report"])
files = [f for f in git("diff", "--name-only", "-z", other, "HEAD", "--", *paths).split("\0") if f]
reach = (f"{other} vs HEAD 트리차이 · 동기 지점 {base[:12]}" if base
         else f"{other} vs HEAD 트리차이 (원장 비어 있음 — 부트스트랩)")

print(json.dumps({
    "entry_id": "<FILL: 예 sync-YYYYMMDDHHMM>",
    "utc": "<FILL: 시각은 주입만 받는다>",
    "departure": cur,
    "departure_commit": "<FILL: 분류한 시점의 출발 브랜치 HEAD — 원장 파일은 동기화 제외라 ④ 커밋과 공통층이 같다>",
    "approved_by": "<FILL: 사람>",
    "approved_utc": "<FILL>",
    "_reach": reach,
    "classification": [
        {"file": f, "verdict": "<common_promote|branch_only>", "reason": "<FILL>",
         "provenance": "agent-judged"} for f in files
    ],
}, ensure_ascii=False, indent=2))
PY
}

if [ "$ACTION" = "preflight" ]; then
    preflight || exit $?
    echo "[closing] ② 분류표 빈칸(Agent 가 채우고 ③ 사람이 승인한다):"
    emit_skeleton
    python3 "$REPO/.claude/policies/runtime/constitution_lint.py" --repo "$REPO" 2>/dev/null || true
    exit 0
fi

# ── execute ───────────────────────────────────────────────────────────────────────
[ -n "$ENTRY_FILE" ] || { echo "[closing] FAIL: execute 는 --entry-file 이 필요하다(③ 승인된 분류표)." >&2; exit 2; }
[ -n "$GATE_MODE" ] && [ -n "$WORK_MANIFEST" ] || { echo "[closing] FAIL: execute 는 --mode 와 --manifest 가 필요하다." >&2; exit 2; }
[ -f "$ENTRY_FILE" ] || { echo "[closing] FAIL: 분류표 파일이 없다: $ENTRY_FILE" >&2; exit 2; }

preflight || exit $?

WT=""
cleanup() {
    if [ -n "$WT" ] && [ -d "$WT" ]; then
        echo "[closing] 워크트리 정리: $WT"
        git -C "$REPO" worktree remove --force "$WT" >/dev/null 2>&1 || true
    fi
}
trap cleanup EXIT

echo "[closing] ④ 원장 — 분류표를 append 하고 출발 브랜치에 커밋한다"
python3 "$LEDGER" append --repo "$REPO" --entry-file "$ENTRY_FILE" || exit 5
git -C "$REPO" add .claude/policies/branch_layer_ledger.json
if ! git -C "$REPO" diff --cached --quiet; then
    ENTRY_ID="$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["entry_id"])' "$ENTRY_FILE")"
    git -C "$REPO" commit -q -m "ledger($ENTRY_ID): 분류표 승인 기록 — 출발 $CUR"
    echo "[closing]   원장 커밋: $(git -C "$REPO" rev-parse --short HEAD)"
else
    echo "[closing]   원장에 새 바이트 없음(이미 기록됨)"
fi

echo "[closing] ⑤ 동기화 — 반대 브랜치 워크트리에서 공통층만 옮긴다"
WT="$REPO/../$(basename "$REPO").wt-$OTHER"
git -C "$REPO" worktree add -q "$WT" "$OTHER"
# 자기 일관성 가드가 요구하는 순서: 스크립트만 먼저 당겨온 뒤 실행한다.
git -C "$WT" checkout "$CUR" -- .claude/skills/upstream-version-watch/scripts/sync_branches.sh
( cd "$WT" && bash "$WT/.claude/skills/upstream-version-watch/scripts/sync_branches.sh" \
      --from "$CUR" --mode "$GATE_MODE" --manifest "$WORK_MANIFEST" --apply ) || exit 5

# 카탈로그는 복사가 아니라 재파생이다. 중앙권위 마커는 비추적이라 워크트리에 없으므로,
# **같은 노드의 같은 저장소**라는 사실을 근거로 이 시퀀스가 놓는다(파생 뒤 남기지 않는다).
if [ -f "$CATALOG" ] && [ -f "$REPO/hints/.central_authority" ]; then
    cp -p "$REPO/hints/.central_authority" "$WT/hints/.central_authority"
    ( cd "$WT" && python3 "$WT/.claude/skills/hint-publisher/scripts/hint_catalog.py" derive \
          --repo "$WT" --remote "$REMOTE" ) || echo "[closing]   ⚠ 카탈로그 재파생 실패(원격 조회) — 기재하고 진행" >&2
    rm -f "$WT/hints/.central_authority"
fi

if git -C "$WT" diff --cached --quiet && git -C "$WT" diff --quiet; then
    echo "[closing]   반대 브랜치에 옮길 변경 없음"
else
    git -C "$WT" add -A
    git -C "$WT" commit -q -m "sync: 공통층 $CUR→$OTHER (closing_sequence)"
    echo "[closing]   동기화 커밋: $(git -C "$WT" rev-parse --short HEAD)"
fi
python3 "$LEDGER" merge --repo "$REPO" --from-ref "$OTHER" >/dev/null 2>&1 || true
cleanup; WT=""

if [ "$DO_SUB" = "1" ] && [ -f "$SUBSYNC" ]; then
    echo "[closing] ⑥ 서브 전파 — 공통층 + 이 브랜치의 특화층을 오버레이로"
    TOPO="$(python3 "$PARITY" evaluate --repo "$REPO" --format value 2>/dev/null || true)"
    if [ -n "$TOPO" ]; then
        bash "$SUBSYNC" --mode "$GATE_MODE" --manifest "$WORK_MANIFEST" --branch "$TOPO" --apply \
            || echo "[closing]   ⚠ 서브 전파 실패 — 기재하고 진행(서브는 다음 B1 이 수렴시킨다)" >&2
    else
        echo "[closing]   ⚠ 토폴로지를 해소하지 못해 서브 전파를 건너뛴다" >&2
    fi
else
    echo "[closing] ⑥ 서브 전파 건너뜀(--no-sub 또는 스크립트 부재)"
fi

if [ "$DO_PUSH" = "1" ] && [ -f "$PUSHER" ]; then
    echo "[closing] ⑦ 원격 반영 — 두 운영 브랜치"
    python3 "$PUSHER" --repo "$REPO" --remote "$REMOTE" --branch "$CUR" --branch "$OTHER" || exit 5
else
    echo "[closing] ⑦ push 건너뜀(--no-push)"
fi

echo "[closing] 완료 — 사람 게이트는 ③ 분류표 승인 1회였다."
