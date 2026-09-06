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
# 전에(소스 브랜치 검증·경로 열거·git diff·특히 git checkout 이전) constitution completion gate `authorize`
# 를 동기 호출해 --mode experimental(계획+HITL 승인 스코프 manifest) 또는 --mode promotion
# (evidence-complete+PASS 벤치 재실행 검증)을 통과해야 한다. 거부/무효 manifest 는 게이트가 낸 안정
# JSON(또는 그 자체를 못 얻었을 때의 결정론 wrapper)을 그대로 출력하고 비0 종료 — 이 시점까지 git 명령을
# 전혀 실행하지 않았으므로 거부 시 side effect 는 0 이다.
#
# 사용:
#   bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --mode <experimental|promotion> --manifest <path>
#   bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --mode <experimental|promotion> --manifest <path> --apply
#   bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --help
# 환경변수 override: SRC_BRANCH(기본 main) · DST_BRANCH(기본 multi-node).
#
# 트리거 = 수동(사람이 "브랜치 동기화" 지시 / 모든 작업 종료 후 질의). 자동 훅 없음.
set -euo pipefail

SRC_BRANCH="${SRC_BRANCH:-main}"
DST_BRANCH="${DST_BRANCH:-multi-node}"

# ── hint 브랜치는 sync 대상이 아니다 (plan_26090107 R7 · 2026-09-01) ────────────────
#   hint 는 **빌딩블럭이 아니라 산출물**이다. 그 브랜치의 트리는 `hint_branch.py` 가 allowlist 로
#   매번 새로 짓는 페이로드이며, 코드·스킬이 들어가면 그 순간 합격기준 A1(archive 에 `.claude/`
#   엔트리 0)이 깨진다. 위 ALLOWLIST 는 `.claude/skills` 를 통째로 복사하므로 방향을 착각하면
#   **산출물 브랜치를 빌딩블럭으로 덮어쓴다** — 되돌리려면 페이로드를 다시 지어야 한다.
#   기본값으로는 닿지 않지만, 환경변수 override 가 있으므로 **명시적으로 막는다**.
for _b in "$SRC_BRANCH" "$DST_BRANCH"; do
    case "$_b" in
        hint|refs/heads/hint)
            echo "[sync_branches] 거부: hint 브랜치는 sync 대상이 아니다(산출물 · plan R7)." >&2
            echo "  hint 페이로드는 hint_branch.py publish 가 allowlist 로 새로 짓는다." >&2
            exit 2 ;;
    esac
done

# ── 공유 allowlist (정본 main → multi-node 복사 대상만) ──
#   주의: 토폴로지 분기 구현체·사용자 실값은 여기 넣지 않는다(브랜치별 독립).
ALLOWLIST=(
    CLAUDE.md
    .claude/rules
    .claude/skills
    .claude/schemas
    .claude/policies
    # 추적 git 훅(2026-09-03 추가). `core.hooksPath` 설정은 `.git/config` 라 브랜치마다 갈리지
    #   않지만 **훅 파일 자체는 추적물**이라 갈린다 — 한쪽 브랜치에서만 tripwire 가 도는 상태가
    #   된다. ★ MIRROR_DIRS 에도 함께 넣는다(한쪽만 넣어 침묵 누락된 선례 3건: families.json ·
    #   HINT_ISSUANCE_CONTRACT.md · assets).
    .claude/hooks
    .gitignore
    manifest.template.yaml
    .gitattributes
    # ── 배포 문서·hint 카탈로그 (2026-08-14 추가) ────────────────────────────────
    # hint 태그는 **git 태그**라 브랜치와 무관하게 저장소 전체에 존재하는데, 그 인덱스
    # (hints/index.json)와 그것으로 자동 재생성되는 카탈로그(HINTS.md)는 **추적 파일**이라
    # 브랜치별로 갈린다. allowlist 에 없어서 single-node 에서 발행한 태그가 multi-node 로
    # 전파되지 않았고, 2026-08-14 실측에서 **실태그 40건 : multi-node 인덱스 35건**으로 벌어져
    # 있었다 — 배포받은 사람이 어느 브랜치를 체크아웃했느냐에 따라 카탈로그가 달라지는 상태.
    # README 도 같은 성격(배포 서사)이라 함께 묶어 브랜치 간 동일성을 보장한다.
    # (docs/report 가 tracked 예외로 승격되며 겪은 것과 같은 계열의 침묵 누락이다.)
    HINTS.md
    README.md
    # ★ 파일 열거가 아니라 **디렉터리 단위**다(2026-08-20 교정). 개별 열거는 같은 사고를
    #   **세 번** 냈다: ① families.json 신설 커밋에서 곧바로 미배선 ② HINT_ISSUANCE_CONTRACT.md
    #   가 처음부터 빠져 single-node 계약서가 **존재하지 않는 경로**를 가리키고 있었다
    #   ③ legacy_v1_pins.json 은 아직 안 바뀌어 우연히 같았을 뿐인 잠복 결함.
    #   hints/ 아래 추적 파일은 전부 hint 배포 레이어의 빌딩블럭이므로 브랜치 간 동일해야 한다.
    #   비추적 `hints/.central_authority`(중앙권위 마커)는 checkout 대상이 아니라 영향 없다.
    #   ★ MIRROR_DIRS 에도 함께 넣는다 — 안 넣으면 정본에서 지운 파일이 대상 브랜치에 유령으로 남는다.
    hints
    # ── README 삽화 (2026-08-18 추가) ──────────────────────────────────────────
    # README.md 가 `<img src="./assets/...">` 로 참조하므로 **README 와 한 벌**이다. README 만
    # 전파하고 assets 를 빼면 반대 브랜치에서 이미지가 404 로 깨진다 — 위 HINTS.md 주석이 적은
    # "배포받은 사람이 어느 브랜치를 체크아웃했느냐에 따라 달라지는 상태"의 같은 계열이고,
    # 실제로 2026-08-18 시점 single-node 의 assets 파일 수는 0 이었다.
    # ★ MIRROR_DIRS 에도 함께 넣는다 — 여기에만 넣으면 정본에서 지운 삽화가 대상 브랜치에
    #   유령으로 남는다(추가는 전파되고 삭제는 안 되는 비대칭).
    assets
    # ── 캠페인 아티팩트 체인 뼈대 (2026-09-06 추가 · plan_26090616 ②) ──────────────
    # `campaigns/README.md` 와 `campaigns/_template/**` 는 **추적 빌딩블럭**이고 인스턴스
    # `campaigns/<camp-id>/**` 는 gitignore 로 비추적이다. 그래서 디렉터리째 넣어도 휘발
    # 인스턴스는 따라가지 않는다(checkout 은 인덱스만 옮긴다).
    # ★ 이 행이 없어서 첫 싱크가 뼈대를 **조용히 빼먹었다**(2026-09-06 실측) — docs/report ·
    #   families.json · HINT_ISSUANCE_CONTRACT.md · assets 에 이은 같은 계열의 네 번째다.
    #   새 추적 루트를 만들면 여기와 MIRROR_DIRS 두 곳을 함께 갱신해야 한다.
    campaigns
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
    .claude/hooks
    docs/report
    assets
    hints
    # 뼈대에서 지운 틀이 대상 브랜치에 유령으로 남지 않게 한다(추가만 전파되고 삭제는 안 되는
    # 비대칭을 막는 것이 이 목록의 존재 이유다). 인스턴스는 비추적이라 이 판정 밖이다.
    campaigns
)
# Exact historical root paths removed by the self-contained owner relocation. These paths sit
# outside MIRROR_DIRS, so checkout alone cannot remove them from an older destination branch.
# Never widen this to `scripts/**`: only these reviewed tombstones may be staged for deletion.
ROOT_RELOCATION_TOMBSTONES=(
    scripts/agent_control.py
    scripts/cleanup_docker.py
    scripts/completion_gate.py
    scripts/doc_naming.py
    scripts/engine_liveness_watchdog.sh
    scripts/evidence_publisher.py
    scripts/harness_verify.py
    scripts/hint_tag.py
    scripts/host/vllm-drop-caches.sh
    scripts/install_host_safety.sh
    scripts/mem_watchdog.sh
    scripts/policy_registry.py
    scripts/providers/claude_code.py
    scripts/smoke_clone.sh
    scripts/sync_branches.sh
    scripts/systemd/easy-vllm-memwatch.service
    scripts/templates/hint_recipe.template.md
)
# One-to-one owner replacements, in the same order as ROOT_RELOCATION_TOMBSTONES.  Apply validates
# canonical Git object -> destination index -> materialized bytes/mode for every entry before the
# first tombstone deletion.  This is deliberately explicit: widening/globbing would weaken review.
ROOT_RELOCATION_REPLACEMENTS=(
    .claude/policies/runtime/agent_control.py
    .claude/skills/vllm-recipe-explorer/scripts/cleanup_docker.py
    .claude/policies/runtime/completion_gate.py
    .claude/skills/wiki-desk/scripts/doc_naming.py
    .claude/skills/vllm-recipe-explorer/scripts/engine_liveness_watchdog.sh
    .claude/policies/runtime/evidence_publisher.py
    .claude/policies/runtime/harness_verify.py
    .claude/skills/hint-publisher/scripts/hint_tag.py
    .claude/skills/terraforming_node/scripts/host_safety/host/vllm-drop-caches.sh
    .claude/skills/terraforming_node/scripts/host_safety/install_host_safety.sh
    .claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh
    .claude/policies/runtime/policy_registry.py
    .claude/policies/runtime/providers/claude_code.py
    .claude/skills/upstream-version-watch/scripts/smoke_clone.sh
    .claude/skills/upstream-version-watch/scripts/sync_branches.sh
    .claude/skills/terraforming_node/scripts/host_safety/systemd/easy-vllm-memwatch.service
    .claude/skills/hint-publisher/templates/hint_recipe.template.md
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
  bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --mode promotion --manifest manifests/sync.json
  bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --mode experimental --manifest manifests/sync.json --apply
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
GATE_SCRIPT="$REPO_ROOT/.claude/policies/runtime/completion_gate.py"
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

# ── pre-flight: 이 스크립트 자신이 정본과 동일 버전인가 (self-overwrite 위험 차단) ──
#
# apply 의 `git checkout <SRC> -- PATHS` 는 **이 파일 자신도 덮는다**(sync_branches.sh 가 PATHS 에
# 있다). 그런데 bash 는 배열·함수를 **시작 시 읽어둔 내용**으로 갖고 있고 본문은 파일에서 이어 읽는다
# — 즉 checkout 이후로는 *메모리는 구버전 · 디스크는 신버전* 이 되어 동작이 정의되지 않는다.
#
# 2026-08-20 실제 발생: hint_tag.py 를 hint-publisher 로 이관해 relocation 표가 바뀐 회차에서,
# checkout 뒤의 replacement 검증이 **메모리에 남은 구경로**를 찾다 죽었다:
#     FAIL: source replacement missing before tombstone scripts/hint_tag.py: <구경로>
# 디스크의 표에는 신경로가 이미 들어와 있었으므로, 사람이 파일을 열어보면 "왜 죽었는지" 알 수 없다.
#
# 처방: **불일치면 아예 시작하지 않는다.** 스크립트만 먼저 당겨오게 해서 메모리와 디스크를 같은
# 버전으로 맞춘 뒤 실행시킨다. 검증 자체를 checkout 앞으로 옮기지 않는 이유는, 그 검증이 의도적으로
# *materialized* 바이트(체크아웃 결과물)를 보기 때문이다 — 앞으로 옮기면 그 보장을 잃는다.
SELF_BASENAME="$(basename -- "${BASH_SOURCE[0]}")"
SELF_REL="${SCRIPT_DIR#"$REPO_ROOT"/}/$SELF_BASENAME"
if [ "$SELF_REL" != "$SCRIPT_DIR/$SELF_BASENAME" ] && git cat-file -e "$SRC_BRANCH:$SELF_REL" 2>/dev/null; then
    SELF_SRC_BLOB="$(git rev-parse "$SRC_BRANCH:$SELF_REL")"
    SELF_DISK_BLOB="$(git hash-object -- "$REPO_ROOT/$SELF_REL")"
    if [ "$SELF_SRC_BLOB" != "$SELF_DISK_BLOB" ]; then
        echo "[sync-branches] FAIL: 이 스크립트가 정본($SRC_BRANCH)과 다릅니다." >&2
        echo "[sync-branches]   disk=${SELF_DISK_BLOB:0:12}  $SRC_BRANCH=${SELF_SRC_BLOB:0:12}" >&2
        echo "[sync-branches]   apply 는 이 파일 자신도 덮으므로, 그대로 진행하면 메모리(구버전)와" >&2
        echo "[sync-branches]   디스크(신버전)가 갈려 relocation 검증이 엉뚱한 경로를 찾다 죽습니다." >&2
        echo "[sync-branches]   먼저 스크립트만 당겨온 뒤 다시 실행하세요:" >&2
        echo "[sync-branches]     git checkout $SRC_BRANCH -- $SELF_REL" >&2
        exit 3
    fi
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
# docs skeletons + report artifacts + 재현성 인증서 (canonical tree enumeration, NUL-safe).
#
# ★ `docs/benchmark/benchmark_*.yaml` (2026-08-20 추가 · plan_26082017 W2-b): 인증서가 tracked
#   예외로 승격되면서 이 열거에 빠지면 **같은 침묵 누락이 네 번째**가 된다(선례: families.json ·
#   HINT_ISSUANCE_CONTRACT.md · legacy_v1_pins.json · docs/report). hint 태그는 git **태그**라
#   브랜치와 무관하게 존재하는데, 그 footer 의 `certificate_ref` 가 가리키는 인증서가 한쪽
#   브랜치에만 있으면 **수신자가 어느 브랜치를 체크아웃했느냐에 따라 증거 도달이 갈린다.**
#   (2026-09-03 F-6a: footer 의 digest 3필드는 삭제됐고 참조 경로는 남았다 — 이 열거의 사유는
#   digest 대조가 아니라 참조 대상의 존재 자체다.)
#
# ⚠ 의도적 비대칭: 인증서는 MIRROR_DIRS 에 **넣지 않는다**. hints/·assets/ 와 달리 인증서는
#   측정 시점에 고정되는 **append-only 증거**다. 미러로 만들면 정본에 없는 = 반대 브랜치가
#   자기 환경에서 발행한 인증서가 동기화 때마다 삭제된다. 양방향 동기화를 거치며 두 브랜치가
#   **합집합**으로 수렴하는 것이 옳다(유령 파일이 아니라 보존이다).
while IFS= read -r -d '' f; do
    case "$f" in
        docs/report/*|docs/*/example.md|docs/benchmark/benchmark_*.yaml) PATHS+=("$f") ;;
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

# Relocation tombstones are source-absent by design. Preview/stage them only when the destination
# still tracks the exact historical path; unrelated destination scripts remain untouched.
for dest_path in "${ROOT_RELOCATION_TOMBSTONES[@]}"; do
    if git ls-files --error-unmatch -- "$dest_path" >/dev/null 2>&1 \
       && ! git cat-file -e "$SRC_BRANCH:$dest_path" 2>/dev/null; then
        DELETE_PATHS+=("$dest_path")
    fi
done

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
#
# ⚠ 이 checkout 은 **이 스크립트 자신도 덮는다**(sync_branches.sh 가 PATHS 에 있다). 그런데 아래
#   relocation 검증은 bash 가 **시작 시 읽어둔 배열**을 쓰므로, 정본에서 relocation 표가 바뀐 회차에는
#   *디스크는 신버전 · 메모리는 구버전* 이 되어 검증이 엉뚱한 경로를 찾다 죽는다.
#   2026-08-20 실제 발생: hint_tag.py 를 hint-publisher 로 이관한 회차에서
#   "source replacement missing before tombstone scripts/hint_tag.py: <구경로>" 로 실패.
#   **처방**: 위 §pre-flight 의 self-consistency 가드가 **불일치면 시작 자체를 막는다**(2026-08-20).
#   그래서 이 지점에 도달했다면 메모리와 디스크는 이미 같은 버전이다.
#   (검증을 checkout 앞으로 옮기지 않는 이유는 그 검증이 *materialized* 바이트를 보기 때문이다 —
#    앞으로 옮기면 "복사가 실제로 제대로 내려앉았는가"라는 보장을 잃는다.)
echo "[sync-branches] APPLY  $SRC_BRANCH → $DST_BRANCH (working-dir 갱신)"
git checkout "$SRC_BRANCH" -- "${PATHS[@]}"
# Git records only the executable bit; shared-repository umasks can materialize 0775.
# Normalize canonical runner assets so production copies and review exports are exactly 0755.
for runner in arm_patch.sh debug-init.sh serve_runner.sh; do
    runner_path=".claude/skills/upstream-version-watch/assets/configs/$runner"
    [ ! -f "$runner_path" ] || chmod 0755 "$runner_path"
done

if [ "${#ROOT_RELOCATION_TOMBSTONES[@]}" -ne "${#ROOT_RELOCATION_REPLACEMENTS[@]}" ]; then
    echo "[sync-branches] FAIL: relocation tombstone/replacement cardinality drift." >&2
    exit 4
fi
# ── 2026-09-03 (F-1f · plan_26090222): git-대-git blob 대조 2건 제거 ──────────────
# 이 루프는 예전에 세 층을 대조했다: ⓐ 정본 tree blob ↔ 인덱스 blob ⓑ 워킹트리 blob ↔ 정본 blob
# ⓒ 파일시스템 모드 ↔ 기대 모드. 그런데 ⓐ·ⓑ 가 비교하는 네 값은 **모두 git 이 방금 스스로 쓴
# 것**이다 — 바로 위 `git checkout "$SRC_BRANCH" -- "${PATHS[@]}"` 가 인덱스와 워킹트리를 정본
# 트리에서 동시에 갱신했고, `set -euo pipefail` 이라 그 checkout 이 실패하면 여기 도달조차 못 한다.
# 즉 ⓐ·ⓑ 는 git 이 이미 든 바이트를 손으로 옮겨적어 자기 자신과 대조하는 **중복 무결성층**이었다.
#
# ★ 그러나 ⓐ 에는 대조와 무관한 **진짜 불변식**이 하나 얹혀 있었다: "checkout 이 이 replacement
#   경로를 정말 덮었는가". 경로가 allowlist 밖이면 인덱스는 옛 blob 을 든 채 남고 ⓐ 가 그것을
#   잡았다. 그 불변식은 해시 대조 없이 **직접** 말하는 편이 정확하고 싸다 — 아래 커버리지 가드가
#   그것이며, 변형 전에 판정하므로 검출 시점도 앞당겨진다.
# KEEP: 존재 검사(정본에 replacement 가 있는가) · 모드 검사(파일시스템 모드는 git 이 exec 비트만
#       들므로 중복이 아니다).
for i in "${!ROOT_RELOCATION_REPLACEMENTS[@]}"; do
    replacement="${ROOT_RELOCATION_REPLACEMENTS[$i]}"
    tombstone="${ROOT_RELOCATION_TOMBSTONES[$i]}"
    # 커버리지: replacement 는 위 checkout 의 pathspec(PATHS) 에 반드시 덮여야 한다.
    #   ALLOWLIST 가 디렉터리 단위라 접두 일치로 판정한다(파일 열거가 아니다).
    covered=0
    for covering_path in "${PATHS[@]}"; do
        case "$replacement" in
            "$covering_path"|"$covering_path"/*) covered=1; break ;;
        esac
    done
    if [ "$covered" != 1 ]; then
        echo "[sync-branches] FAIL: relocation replacement is outside the checkout allowlist — the index would keep its stale blob: $replacement" >&2
        exit 4
    fi
    source_meta="$(git ls-tree "$SRC_BRANCH" -- "$replacement")"
    if [ -z "$source_meta" ]; then
        echo "[sync-branches] FAIL: source replacement missing before tombstone $tombstone: $replacement" >&2
        exit 4
    fi
    source_mode="${source_meta%% *}"
    case "$source_mode" in
        100755) expected_fs_mode=755 ;;
        100644) expected_fs_mode=644 ;;
        *) echo "[sync-branches] FAIL: unsupported replacement mode $source_mode: $replacement" >&2; exit 4 ;;
    esac
    chmod "$expected_fs_mode" "$replacement"
    materialized_mode="$(stat -c '%a' -- "$replacement")"
    if [ "$materialized_mode" != "$expected_fs_mode" ]; then
        echo "[sync-branches] FAIL: materialized replacement mode mismatch before tombstone $tombstone: $replacement (got $materialized_mode, want $expected_fs_mode)" >&2
        exit 4
    fi
done
echo "[sync-branches] relocation replacements verified: allowlist coverage → canonical presence → materialized mode PASS"

# Deletions are deliberately last: no historical fallback disappears until every relocated owner
# replacement above has passed canonical byte and exact-mode validation.
if [ "${#DELETE_PATHS[@]}" -gt 0 ]; then
    git rm --ignore-unmatch -- "${DELETE_PATHS[@]}"
fi
echo "[sync-branches] 완료 — 공유 빌딩블럭을 working-dir 에 반영했습니다(스테이징됨)."
echo "[sync-branches] 다음(사람): 변경 검토 후 직접 커밋하세요. 예:"
echo "[sync-branches]   git status && git diff --cached"
echo "[sync-branches]   git commit -m 'sync: 공유 빌딩블럭 main→$DST_BRANCH'"
echo "[sync-branches] (이 스크립트는 자동 커밋하지 않습니다 — HITL.)"
