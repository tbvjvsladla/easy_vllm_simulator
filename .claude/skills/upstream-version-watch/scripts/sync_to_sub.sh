#!/bin/bash
# 메인노드 → 서브노드 **브랜치-aware 하향 싱크 오케스트레이터** (D12).
#
# 멀티노드 확장의 [하향 배달] 단계. 메인 검증 런타임을 서브로 직접 전송(GitHub 경유 X)하고,
# 서브 **로컬 git**(single·multi 두 브랜치, origin 영구 없음)에 **스크립트저작 `[sync]` 커밋**으로 박는다.
# 상향(서브 자기개선 회수)은 별도·문서기반: fetch_sub_docs.sh. (plan_26062411 · workflow.md §양방향 브랜치싱크)
#
# 흐름:
#   B0 멱등 self-bootstrap — 서브 .git 부재 시: git init + base(.gitignore) 커밋 + multi·single 브랜치 생성,
#                            그리고 multi 브랜치 초기 전체 배달(+커밋). single=base(dormant). **첫 init=HITL(--apply)**.
#   B1 per-branch 증분(--branch) — dirty 체크(fail-closed) → checkout → render → rsync(빌드+오버레이) → [sync] 커밋.
#        겹침 = main-canonical(sub-yields). 서브 [improve] history 는 git 에 잔존.
#   single 확장 게이트(D12 Gap B) — 판정은 `node_role_contract.py … --field delivery_plane`(단일 소유자)가 한다.
#        single 의 sub_mode=a2a-agent → **dormant(빌드킷 배달 skip)** · multi 의 sub 만 ray-worker → active.
#        `role: sub` 의 *존재* 는 판정 입력이 아니다(헌법 §불변식 A · 2026-08-22 교체).
#
# HITL 안전장치: 기본 DRY-RUN(미리보기). 실제 변경은 --apply. **스크립트 auto-stash 금지**(서브가 스스로 clean 화).
# 빌드 배달(S4 Band2-only · plan_26062417): rsync 소스 = output/<t>/ 서브트리만 → 루트 Band1(템플릿·scripts·resolved.json) 구조적 배제.
#   output/<t>/ 내 keying: Band2(configs/{serve_runner,debug-init}.sh · envs/.env.interconnect · Dockerfile·compose·requirements·.dockerignore·.gitkeep) 전파 ·
#   Band3(모델 트리플렛 configs/<m>.{sh,yaml}·envs/.env.<m>) keying 배제 · manifest.yaml(D10)·sub_provision(overlay) 배제 · 미분류=fail-loud(assert_band_classification).
#   에이전트 환경(CLAUDE.md·.claude·docs·.gitignore)은 deliver_overlay/bootstrap 소관(빌드 배달과 분리).
#
# 사용:
#   bash sync_to_sub.sh --mode experimental --manifest <work.json>             # 승인된 실험 DRY-RUN
#   bash sync_to_sub.sh --mode promotion --manifest <work.json> --apply          # promotion-ready 실행
#   bash sync_to_sub.sh --mode promotion --manifest <work.json> --apply --provision
#   bash sync_to_sub.sh --mode promotion --manifest <work.json> --apply --branch single|both
# 환경변수 override: SUB_HOST(<ssh_user>@<host>) · SRC · SUB_WORK_DIR · SYNC_GIT_NAME · SYNC_GIT_EMAIL.
# SUB_HOST·SUB_WORK_DIR 미지정 시 output/multi/manifest.yaml nodes[](role:sub)에서 해소(서브는 multi manifest 에만 정의).
set -euo pipefail

# 2026-09-05(N3 · ②-b 라이브): 미리보기를 `head -40` 으로 **말없이** 잘랐다. 무엇이 배달되는지
# 확인하라고 만든 화면인데 꼬리가 사라지면 확인이 성립하지 않는다(가산 전용이라 파괴 위험은
# 없었지만, "보여준다"고 적힌 것이 실제로는 일부만 보여준 것은 표시 결함이다).
# 처방: 자르되 **잘랐다고 말한다**. 전량은 `SYNC_PREVIEW_LINES=0` 로 본다.
preview_lines(){   # stdin → 앞부분 + (잘렸으면) 남은 줄 수 고지
    local limit="${SYNC_PREVIEW_LINES:-40}" buf n
    buf="$(cat)"
    n="$(printf '%s\n' "$buf" | grep -c '' || true)"
    if [ "$limit" = "0" ] || [ "$n" -le "$limit" ]; then
        printf '%s\n' "$buf"
    else
        printf '%s\n' "$buf" | head -n "$limit"
        echo "      … 이하 $(( n - limit ))줄 생략(전량: SYNC_PREVIEW_LINES=0)"
    fi
}

usage() {
    cat <<'EOF'
사용법: sync_to_sub.sh --mode <experimental|promotion> --manifest <path> [--apply] [--provision] [--branch multi|single|both]

  --mode       side-effect authorization mode (required)
  --manifest   work-manifest JSON; relative paths use the caller's original CWD (required)
  --apply      execute delivery (default: dry-run)
  --provision  allow missing remote work directory creation with --apply
  --branch     multi, single, or both (default: multi)
  --help, -h   print this help without repo, manifest, host, or transport checks
EOF
}

# Parse before repo/topology/host/transport discovery.  Missing required gate flags are
# intentionally forwarded to completion_gate.py so its stable authorization JSON is the sole
# fail-closed machine contract.
ORIGINAL_CWD="$(pwd)"
GATE_MODE=""; HAVE_MODE=0
MANIFEST_ARG=""; HAVE_MANIFEST=0
MODE="dryrun"; PROVISION=0; BRANCH="multi"
while [ $# -gt 0 ]; do
    case "$1" in
        --help|-h) usage; exit 0 ;;
        --mode)
            [ $# -ge 2 ] || { echo "[sync] FAIL: --mode requires a value" >&2; exit 2; }
            GATE_MODE="$2"; HAVE_MODE=1; shift 2 ;;
        --manifest)
            [ $# -ge 2 ] || { echo "[sync] FAIL: --manifest requires a path" >&2; exit 2; }
            MANIFEST_ARG="$2"; HAVE_MANIFEST=1; shift 2 ;;
        --apply) MODE="apply"; shift ;;
        --provision) PROVISION=1; shift ;;
        --branch)
            [ $# -ge 2 ] || { echo "[sync] FAIL: --branch requires multi|single|both" >&2; exit 2; }
            BRANCH="$2"; shift 2 ;;
        --branch=*) BRANCH="${1#*=}"; shift ;;
        *) echo "[sync] FAIL: unknown argument: $1" >&2; exit 2 ;;
    esac
done
case "$BRANCH" in multi|single|both) ;; *) echo "[sync] FAIL: --branch must be multi|single|both" >&2; exit 6 ;; esac

# Gate location is immutable and derived from this script, never from caller-controlled SRC.
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)"
GATE_SCRIPT="$REPO_ROOT/.claude/policies/runtime/completion_gate.py"
RESOLVED_MANIFEST=""
if [ "$HAVE_MANIFEST" -eq 1 ]; then
    case "$MANIFEST_ARG" in
        /*) RESOLVED_MANIFEST="$MANIFEST_ARG" ;;
        *) RESOLVED_MANIFEST="$ORIGINAL_CWD/$MANIFEST_ARG" ;;
    esac
fi

gate_cmd=(python3 "$GATE_SCRIPT" authorize --action sync_to_sub --repo-root "$REPO_ROOT")
[ "$HAVE_MANIFEST" -eq 0 ] || gate_cmd+=(--manifest "$RESOLVED_MANIFEST")
[ "$HAVE_MODE" -eq 0 ] || gate_cmd+=(--mode "$GATE_MODE")
if gate_stdout="$("${gate_cmd[@]}")"; then gate_exit=0; else gate_exit=$?; fi

gate_allowed="false"
if [ -n "$gate_stdout" ]; then
    gate_allowed="$(printf '%s' "$gate_stdout" | python3 -c '
import json, sys
try:
    print("true" if json.load(sys.stdin).get("allowed") is True else "false")
except Exception:
    print("false")
' 2>/dev/null || printf false)"
fi
if [ "$gate_exit" -ne 0 ] || [ "$gate_allowed" != "true" ]; then
    if [ -n "$gate_stdout" ]; then
        printf '%s\n' "$gate_stdout"
        [ "$gate_exit" -eq 0 ] && exit 1
        exit "$gate_exit"
    fi
    python3 -c '
import json
print(json.dumps({
 "schema_version":1,"mode":None,"action":"sync_to_sub","task_class":None,
 "authorization_state":None,"allowed":False,
 "reason_codes":["SYNC_TO_SUB_GATE_OUTPUT_UNREADABLE"],
 "messages":{"SYNC_TO_SUB_GATE_OUTPUT_UNREADABLE":"completion gate produced no parseable JSON"},
 "identity":None,"exit_code":2}, ensure_ascii=False, sort_keys=True, indent=2))
'
    exit 2
fi

# Authorization passed. Existing source override and all operational discovery begin only here.
SRC="${SRC:-$REPO_ROOT/}"
CANONICAL_SRC="${SRC%/}/"
SSH_OPTS="${SYNC_SSH_COMMAND:-ssh -o BatchMode=yes -o ConnectTimeout=8}"
GIT_NAME="${SYNC_GIT_NAME:-easy-vllm sync (main)}"      # [sync] 커밋 = 스크립트저작 표식
GIT_EMAIL="${SYNC_GIT_EMAIL:-sync@easy-vllm.local}"
MAX_DELETE="${MAX_DELETE:-50}"     # (레거시) --delete 안전캡. S4 는 아래 ALLOW_DELETE 삭제brake 가 1차 게이트.
ALLOW_DELETE="${ALLOW_DELETE:-0}"  # (S4 d-rsync-2) 삭제 前 brake: 삭제예정 > 이 값이면 *삭제 前* fail-closed. 의도된 정리만 명시 override.
RENDER="$SRC.claude/skills/terraforming_node/scripts/render_sub_env.py"
# 역할 계약 판정기 — 배달 표면 검증이 tool_plane 을 물을 때 쓴다. 2026-09-05: 이 상수가
#   **정의된 적이 없었다**. 검증기 본문 주석은 "경로는 파일 상단 ROLE_CONTRACT 상수"라고
#   적고 있었으니 배선만 빠진 것이다(만든 것과 도는 것은 다르다).
ROLE_CONTRACT="$SRC.claude/skills/terraforming_node/scripts/node_role_contract.py"
PATCH_VALIDATOR="$SRC.claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
PATCH_RESOLUTION="$SRC.claude/skills/upstream-version-watch/assets/current-production-resolution.json"

# ── manifest 해소(서브 접속·work_dir = 항상 multi 통로 manifest. single 은 nodes:[] 라 서브 미정의) ──
# 2026-09-03(P3 · plan_26090317): 두 해소기가 `output/multi/manifest.yaml` 을 **하드코딩**했다.
#   §2.7.0 이 명시적으로 허용하는 single+sub(A2A 에이전트) 구성에서는 서브 주소가
#   `output/single/manifest.yaml` 에 있으므로, 등록을 해도 "서브노드 주소 미해소" 로 죽었다.
#   토폴로지는 인자로 받는다(브랜치로 추론하지 않는다 — 호출부가 --branch 로 정한 타겟을 넘긴다).
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
# single 확장 활성? — 판정 입력은 `role: sub` 의 **존재**가 아니라 **정체성 계약(sub_mode)** 이다.
#   존재는 정체성을 말하지 않는다: 두 종류의 sub(멀티=Ray 워커 · 싱글=A2A 원격 에이전트)가 같은
#   단어를 쓰기 때문이다(헌법 §불변식 A). 옛 awk 술어는 존재만 보고 배달 평면을 켰고, 그 결과
#   싱글 서브로 메인 빌드킷이 34회 흘러갔다(testlog_26082215 §4.2 ④).
#   판정의 **단일 소유자**는 node_role_contract.py 다 — 여기서는 묻기만 하고 규칙을 재저작하지 않는다.
#   fail-closed: 판정기 부재·파싱 실패·계약 위반은 전부 dormant(배달 안 열림)이며, 이유를 stderr 로
#   밝힌다(침묵 폴백 ✗ — 헌법 §4종 안티패턴 판정표의 '결함' 칸).
_single_extension_active() {
    local manifest="${SRC%/}/output/single/manifest.yaml"
    local contract="${SRC%/}/.claude/skills/terraforming_node/scripts/node_role_contract.py"
    local plane rc=0
    # 2026-09-03(P3 · plan_26090317): B0 는 서브 git 이 없으면 **무조건 multi 를 채웠다**. 그 설계는
#   멀티 온보딩만 상정한 것인데, §2.7.0 이 허용하는 single+sub(A2A 에이전트) 구성에서는 그것이
#   **헌법이 금지하는 배달**이다 — 싱글 서브는 자기 빌드킷을 자율 저작하고, 메인의 멀티 빌드킷은
#   그에게 가면 안 된다(불변식 A). 게다가 이 워크스페이스의 output/multi 는 다른 브랜치 산출물이라
#   상시 stale 이고, 그 stale 을 preflight 가 요구해 **single 배달이 multi 사유로 죽었다**.
#   → 부트스트랩이 채우는 토폴로지는 **요청된 타겟에 따른다**. multi 가 타겟이 아니면 base 만 만든다.
BOOTSTRAP_POPULATE=""
if [ $HAS_GIT = 0 ]; then
    for _t in "${TARGETS[@]}"; do [ "$_t" = "multi" ] && BOOTSTRAP_POPULATE="multi"; done
fi
SINGLE_PLANE_SOURCE="fail-closed:unevaluated"      # 판정 출처(*_source 표시 — 안내문이 인용한다)
    if [ ! -f "$manifest" ]; then
        SINGLE_PLANE_SOURCE="fail-closed:manifest-absent"
        echo "[sync] single 확장 판정: manifest 부재($manifest) → dormant(fail-closed)" >&2
        return 1
    fi
    if [ ! -f "$contract" ]; then
        SINGLE_PLANE_SOURCE="fail-closed:contract-script-absent"
        echo "[sync] single 확장 판정: 계약 판정기 부재($contract) → dormant(fail-closed)" >&2
        return 1
    fi
    # stderr 는 캡처하지 않는다 — 위반 메시지([node-role] VIOLATION …)가 사람 화면에 그대로 떠야 한다.
    plane="$(python3 "$contract" evaluate --manifest "$manifest" --topology single \
                     --field delivery_plane --format value)" || rc=$?
    if [ "$rc" -ne 0 ]; then
        SINGLE_PLANE_SOURCE="fail-closed:contract-exit-$rc"
        echo "[sync] single 확장 판정: node_role_contract exit=$rc → dormant(fail-closed)" >&2
        return 1
    fi
    # 출처는 판정기가 소유한다 — 여기서 사유를 손저작하지 않는다(sub-mode:a2a-agent · no-sub-registered …).
    SINGLE_PLANE_SOURCE="$(python3 "$contract" evaluate --manifest "$manifest" --topology single \
                                  --field delivery_plane --format json 2>/dev/null \
                           | python3 -c 'import json,sys; print((json.load(sys.stdin).get("delivery_plane") or {}).get("source") or "unknown")' \
                           2>/dev/null || echo unknown)"
    [ "$plane" = "active" ]
}

# ── A2A-위임 전파 게이트 (plan_26063021_14_37 D5 · New-2) ──
# 서브 위임 키의 *권위* = output/<t>/manifest.yaml 의 nodes[sub].hw_verified(메인이 동질성 검증 후 기입).
# render_sub_env 가 이 값으로 키를 렌더 → 게이트가 권위(소스)를 검사 = egg-free(같은 sync 가 배달하는 키를 검사하지 않음).
# 미검증 → 빌드 전파 거부 + HITL(무인 자동 서브-스캔 ✗ — "열쇠 분실=사고"). 검증 path = 운영자가 terraforming --peer-ssh 구동.
_resolve_sub_hw_verified() {  # $1=topology → nodes[sub].hw_verified 값 출력(없으면 빈줄)
    local manifest="${SRC%/}/output/$1/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '
        /^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { in_sub=1; next }
        /^[[:space:]]*-[[:space:]]*role:/ { in_sub=0 }
        in_sub && /^[[:space:]]*hw_verified:/ { sub(/^[[:space:]]*hw_verified:[[:space:]]*/, ""); sub(/[[:space:]]*#.*/, ""); gsub(/[ "\r]/, ""); print; exit }
    ' "$manifest"
}
assert_sub_delegation_authorized() {  # $1=topology → 0=인가(hw_verified:true), 1=거부(키 미발급)
    local hv; hv="$(_resolve_sub_hw_verified "$1" 2>/dev/null || true)"
    [ "$hv" = "true" ] && return 0
    echo "[sync] STOP(A2A-위임 게이트): output/$1/manifest.yaml 의 nodes[sub].hw_verified != true (got '${hv:-<none>}')" >&2
    echo "  → 서브 HW 동질성 미검증 = 위임 키 미발급 → 빌드 전파 거부(fail-closed)." >&2
    echo "  → 검증 path(HITL): python3 .claude/skills/terraforming_node/scripts/scan_node.py --topology $1 --peer-ssh <user@sub> --emit-manifest" >&2
    echo "     → 동질성 통과 시 nodes[sub].hw_verified:true 기입(사람 확인) → 재실행. (무인 자동 서브-스캔 ✗ — '열쇠 분실=사고'.)" >&2
    return 1
}

# 타겟 브랜치 목록 — 주소 해소가 이 목록을 쓰므로 먼저 정한다.
TARGETS=(); case "$BRANCH" in multi) TARGETS=(multi);; single) TARGETS=(single);; both) TARGETS=(multi single);; esac

# 주소는 **타겟 토폴로지의 manifest** 에서 온다. --branch both 처럼 후보가 둘이면 값이 일치해야 한다 —
# 갈리면 어느 노드에 배달하는지가 모호해지므로 fail-closed 한다(추측으로 고르지 않는다).
#
# ⚠ 2026-09-06: **host 가 통로마다 다른 것은 비정상이 아니다.** 같은 물리 노드라도 싱글은 관리
#   hostname(A2A 제어 평면)으로, 멀티는 RoCE IP(집단연산 평면)로 잡힌다 — `fetch_sub_docs.sh` 는
#   2026-09-05(N1)에 이 사실을 반영해 "어느 통로에서 회수할지 **선언**하면 그 통로만 본다"로 고쳤는데
#   이 스크립트는 따라오지 않았다. 그래서 `--branch both` 가 **구조적으로 불가능**하다.
#   지금 할 수 있는 정직한 안내는 아래 메시지다(상태가 틀린 게 아니라 이 스크립트가 통로 하나만
#   든다). 근본 교정은 SUB_HOST 를 타겟별로 나르는 것인데, 이 값이 원격 트랜잭션·롤백 경로까지
#   관통하므로 별도 작업으로 뺀다.
_resolve_addr_across_targets() {   # $1=host|work_dir → stdout=값 · 1=미해소 · 2=불일치
    local kind="$1" t v prev="" src=""
    for t in "${TARGETS[@]}"; do
        if [ "$kind" = "host" ]; then v="$(_resolve_sub_host_from_manifest "$t" || true)"
        else v="$(_resolve_sub_work_dir_from_manifest "$t" || true)"; fi
        [ -n "$v" ] || continue
        if [ -n "$prev" ] && [ "$v" != "$prev" ]; then
            echo "[sync] FAIL: 서브 $kind 가 토폴로지 manifest 간에 다르다 — $src=$prev vs output/$t=$v" >&2
            if [ "$kind" = "host" ]; then
                echo "  → **상태가 틀린 것이 아닐 수 있다.** 같은 노드라도 싱글은 관리 hostname(A2A 평면),"  >&2
                echo "     멀티는 RoCE IP(집단연산 평면)로 잡히는 것이 정상이다."                              >&2
                echo "  → 이 스크립트는 아직 통로를 하나만 든다. 통로별로 **나눠 실행**하라:"                  >&2
                echo "       ... --branch multi   (그 다음)  ... --branch single"                              >&2
                echo "     두 노드가 정말 서로 다른 물리 노드라면 그것도 이 방법이 맞다."                      >&2
            fi
            return 2
        fi
        prev="$v"; src="output/$t"
    done
    [ -n "$prev" ] || return 1
    printf '%s' "$prev"
}

if [ -z "${SUB_HOST:-}" ]; then
    SUB_HOST="$(_resolve_addr_across_targets host)" || { [ $? = 2 ] && exit 4; SUB_HOST=""; }
fi
if [ -z "${SUB_WORK_DIR:-}" ]; then
    SUB_WORK_DIR="$(_resolve_addr_across_targets work_dir)" || { [ $? = 2 ] && exit 4; SUB_WORK_DIR=""; }
fi
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="${SRC%/}"
if [ -z "${SUB_HOST:-}" ]; then
    echo "[sync] FAIL: 서브노드 주소 미해소 — SUB_HOST(<ssh_user>@<host>) 지정 또는" >&2
    echo "       output/{${TARGETS[*]}}/manifest.yaml 의 nodes[](role:sub) 에 host·ssh_user 채우기." >&2
    exit 4
fi
DEST="${SUB_WORK_DIR}/"

# ── S4 Band2 빌드킷 keying(전파 = output/<t>/ 의 Band2 만 · plan_26062417 rev3 R1) ──
# Band1(루트 템플릿·scripts·resolved.json)은 rsync 소스가 output/<t>/ 라 구조적으로 빠지고,
# Band3(모델 recipe = <model>.{sh,yaml}·모델 env)는 아래 keying 으로 빠진다. 미분류는 assert_band_classification 가 fail-loud.
# ⚠ 정본 주의(d12-1): 서브 gitignore.template 의 `!configs/serve_runner.sh` 는 *루트* configs/ 대상이라 이 allowlist 와
#   *동치 아님*. output/<t>/ Band2 추적은 gitignore.template 의 output/ 예외(!output/<t>/configs/serve_runner.sh 등)가 관할.
BAND2_CONFIGS=(serve_runner.sh debug-init.sh arm_patch.sh)   # topology-keyed 분산서빙 인프라(Band2, 멀티). arm_patch.sh=모델구동 패치 arming(제네릭 결정론·양노드)
BAND2_ENVS=(.env.interconnect .env.cluster)          # topology/network-keyed env(Band2): NCCL(.interconnect) + 클러스터배포(.cluster=S6 materialize)
BAND2_TOP=(Dockerfile Dockerfile.source-build Dockerfile.source-build-upstage docker-compose.yaml requirements.txt .gitkeep)  # 최상위 빌드킷(Band2)
# 빌드 패치 디렉토리의 **단일 소유**(2026-08-13 신설). 두 소비자가 여기서 파생한다:
#   _band2_filters(rsync --include) · assert_band_classification(_b2top 등록).
# ★ 왜 신설했나: `build_patches_src/`(빌드패치 **pre** 위상 = 자체 이식 = 포크 사다리 3번째 칸)가
#   .gitignore 예외·rsync include·린터 목록 **3곳 모두**에서 누락돼 서브 배달 경로가 아예 없었다.
#   더 나쁜 건 침묵이다 — 린터는 prepare_transactional_source 이후의 **index 스냅샷**을 도는데,
#   비추적 항목은 스냅샷에 없으므로 "미분류 fail-loud" 가 발화하지 못한다(gitignore 가 게이트의 눈을
#   가린다). 목록을 세 벌 두면 또 갈라지므로 여기 한 곳에서만 선언한다.
#   ⚠ pre 슬롯은 `workflow.md:77` 기준 **미검증 슬롯**(배관은 동작 확인, 그 위 서빙 성공 사례 없음).
# ⛔ 비움(2026-09-10 · 사용자 결정): build_patches · build_patches_src 의 추적 예외가 철회되면서
#   이 배달 표면도 함께 비운다. 두 목록은 쌍이다 — `.gitignore` 에서 예외를 걷고 여기를 그대로 두면
#   parity 검사가 정확히 "보내지 않는 것을 지우게 된다" 로 막는다(2026-09-10 실측: --apply 배달 거부).
#   ⇒ 슬롯 산출물의 서브 배달은 index 권위 스냅샷이 아니라 `regen_build_patches_src.py`(pre 위상
#     생성엔진)와 hint 페이로드가 맡는다. post 위상(build_patches/)의 배달 경로는 **아직 비어 있다** —
#     그 공백은 이 비움이 만든 것이 아니라 드러낸 것이다(후속: plan_26091021 §4 후속).
BAND2_PATCH_DIRS=()
# 패치 디렉토리 **안쪽**의 배달 범위(payload glob)의 단일 소유(2026-08-14 신설 · R0 사전점검에서 발견).
# ★ 무엇이 틀려 있었나: 배달 스코프와 삭제 스코프가 **서로 다른 정의**를 쓰고 있었다.
#     배달 = git index(prepare_transactional_source) → `files/` 는 .gitignore:156 으로 비추적이라 **부재**
#     삭제 = rsync `--include=/<d>/**` → `files/` 가 **삭제 대상에 포함**
#   즉 "보내지는 않는데 지우기는 한다". 2026-08-13 에 gitignore 만 좁히고(92파일 비추적화) rsync 는
#   `/**` 로 남겨둔 결과이며, 실측 dry-run 이 서브의 92파일 + 부모 3디렉토리를 삭제예정으로 세웠다.
#   삭제brake(ALLOW_DELETE)가 1차로 막지만, 브레이크는 **개수만** 말하므로 운영자가 안내대로
#   `ALLOW_DELETE=95` 를 주면 그대로 파괴된다 — 2026-08-01 브랜치 불일치 사고와 **동형**이다.
# ★ 처방(D3): 우회(예외 하드코딩)가 아니라 **두 스코프를 한 정의로 묶는다**. rsync 의 include 집합은
#   전송 대상이자 삭제 대상이므로, include 를 추적 allowlist 와 일치시키면 두 스코프가 **구성적으로**
#   같아진다. 값은 여기가 소유하고 .gitignore 두 벌은 assert_band2_top_gitignore_parity 가 대조한다.
# 값 근거 = 밴드 규정: build_patches 는 전부 손작성(post 위상 .sh) · build_patches_src 는 손작성분만
#   (`*.sh`+`PROVENANCE.json`)이고 `files/` 는 업스트림 벤더링 = **파생 산출물**이라 비추적이다
#   (.gitignore:140-156 · PROVENANCE.json 유도식 + 파일별 sha256 으로 재생성한다).
# ★ 2026-08-14 교정(B-5): 비추적이 **비배달**을 함의한다고 적었던 것은 틀렸다. 서브는 egress-restricted 라
#   상류를 clone 할 수 없다 → 재생성을 서브에서 할 수 없고, rsync 도 안 보내면 payload 를 **영원히 얻지
#   못한다**. 그래서 배달 경로가 없는 채로 "재생성하면 된다"고 적혀 있었다(전제 없는 규정 = 구멍).
#   처방은 우회(추적 승격·수작업 scp)가 아니라 **평면 신설**이다 — 아래 deliver_source_port_payload 가
#   `메인 파생 → 결정론 번들 → 배달 → 서브 해체·검증` 을 소유한다. rsync 는 여전히 손작성분만 나른다.
declare -A BAND2_PATCH_DIR_PAYLOAD=(
    [build_patches]='*'                          # post 위상: 디렉토리 전체가 손작성 정본
    [build_patches_src]='*.sh PROVENANCE.json'   # pre 위상: 손작성분만. `files/` = 파생 → rsync 비대상·**비삭제**(번들 평면 소관)
)
# 파생 payload 의 배달 평면(2026-08-14 신설 · B-5). rsync 평면과 **의도적으로 분리**한다:
#   rsync   = 인덱스 권위 + 손작성 정본 → 추적 allowlist 와 1:1(parity 단언이 그 정합을 지킨다)
#   번들     = 파일시스템 payload + **인덱스 PROVENANCE 로 검증** → 판정 권위는 추적물에 남는다
#   두 평면을 섞으면(= payload glob 에 files/ 추가) parity 단언이 61,846줄 벤더링을 추적물로 끌어올린다.
SOURCE_PORT_DIR="build_patches_src"
SOURCE_PORT_PAYLOAD="files"
SOURCE_PORT_BUNDLE_NAME=".files.bundle.tar.gz"     # 서브 임시 수신물. 해체 후 즉시 제거(이미지 COPY 오염 차단)
SOURCE_PORT_BUNDLE_SHA=""                          # materialize 단계에서 채운다(토폴로지별 마지막 값)
# output/<t>/ 최상위 **비전송** 경로의 단일 소유. 세 소비자가 전부 여기서 파생한다:
#   _band2_filters(rsync --exclude) · validate_remote_deletion_tree(서브 walk) · validate_inventory_tree(로컬 walk).
# ★ 목록을 두 벌 두면 갈라진다 — 이 프로젝트는 파서 두 벌(D4↔D6)·TP 오카운트 7사이트로 같은 계열
#   사고를 이미 겪었다. 갈라졌을 때의 실해악이 2026-08-01 에 현실화됐다: rsync 는 cache/ 를 제외하는데
#   검증기는 그걸 걷어서, 컨테이너가 root 로 만든 **빈 캐시 디렉터리** 하나가 모든 후속 배달을 영구
#   차단했다(비-root 서브 계정은 자력 해소 불가 → 매번 사람 sudo). 검증기의 시야는 전송 범위를
#   넘지 않는다 — 롤백은 rsync 가 바꿀 수 있는 것만 덮으면 되기 때문이다.
# 근거: manifest.yaml=D10 · .env=serve-time node-local host config(PII, render --materialize-env 산출) ·
#   sub_provision=overlay 소관 · benchlog=adversarial-benchmark 생성 증거(빌드입력 ✗, plan_26063014) ·
#   cache=노드-로컬 JIT 캐시(자동 생성 — 전파가 무의미. 빌드입력 ✗·전파 ✗, plan_26072217).
#   tiktoken_cache=**폴백 기본값 디렉터리일 뿐 정본이 아니다**(2026-09-03 분류 교정).
#     ⚠ 이전 주석은 이 둘을 "노드-로컬 캐시" 한 이름으로 묶었고, 그 분류가 서브를 막다른 길에 넣었다:
#       harmony/tiktoken vocab 은 **자동 생성되는 캐시가 아니라 사전적재가 필요한 입력**이고, 서브는
#       curl/wget/hf 가 전부 deny 라 스스로 얻을 수 없다. 전파도 다운로드도 막히면 영원히 못 얻는다
#       (2026-09-03 실증: 서브 serve 가 `HarmonyError: invalid tiktoken vocab file` 로 차단).
#     처방은 전파 경로 신설이 아니라 **공유 스토리지 + manifest 포인터**다(사용자 결정 — NAS 배치).
#       자산 정본 = `manifest.tiktoken_host_path` 가 가리키는 공유 경로. 그러면 전파가 불필요해지므로
#       이 제외 목록은 그대로 옳다. 되돌아감 방지는 `manifest_contract.py` 의 노드-로컬 경고가 맡는다.
BAND2_EXCLUDED_TOP=(manifest.yaml sub_provision .env benchlog cache tiktoken_cache a2a_signing sub_manifest.yaml
                    build_patches build_patches_src)
#   build_patches·build_patches_src=3+1+1 빌드 패치 슬롯의 **산출물**(2026-09-10 추적 예외 철회의 짝).
#   BAND2_PATCH_DIRS 를 비우면 이 둘은 배달 목록에서 빠지지만 **분류에서도 빠져** assert_band_classification
#   이 '미분류 top-level' 로 정당하게 막는다(실측: --apply 배달 거부). 분류는 파티션이라 배달을 그만두는
#   것과 제외로 선언하는 것이 **한 쌍**이다 — 한쪽만 하면 침묵이 아니라 교착이 된다.
#   여기 등재가 주는 것: 배달 제외 + 서브측 삭제 보호(rsync 는 exclude 된 수신측 항목을 지우지 않는다)
#   + 밴드 분류. 서브가 자기 슬롯 산출물을 자율 저작해도 메인이 지우지 않는다.
#   a2a_signing=메인 A2A **개인 서명키**(서브는 공개 JWK 만 받는다 — 오버레이의 .claude/a2a/trusted_keys.json) ·
#   sub_manifest.yaml=서브 manifest 의 **발급 원본**(서브 사본은 오버레이가 output/<t>/manifest.yaml 로 나른다).
#   둘 다 render 입력이면서 비추적이라 prepare_transactional_source 가 파일시스템 예외로 스냅샷에 넣는다
#   (2026-09-05 ②-b · plan_26090516 §7.6). 여기 등재는 배달 제외 + 삭제 보호 + 밴드 분류를 동시에 준다.
BAND2_RUNTIME_PATCH_STEMS=(exaone45-33b hy3)          # owner-local provenance-bound runtime patches; wildcard authority 금지
# ↑ Dockerfile.source-build-upstage = Solar-Open2 변종 트랙(UpstageAI 포크 @ v0.22.0-solar-open2).
#   Band2 편입 근거 = **빌드-평면**: 멀티는 클러스터-와이드 이미지라 슬레이브도 동일 이미지를 빌드해야 한다
#   (헌법 변종이미지 build-plane ≠ serve-plane 따름정리 · workflow.md S2.5). 모델-키잉 ✗ — track 은 포크
#   **벤더**명이며 stock 0.22.0 의 superset. Band3 모델 트리플렛은 계속 배제.

_band2_filters() {  # rsync include/exclude(첫매치우선). 소스 루트 = output/<t>/.
    FILT=()
    local f g; local -a payload
    for f in "${BAND2_EXCLUDED_TOP[@]}"; do FILT+=(--exclude="/$f"); done   # 단일 소유 = BAND2_EXCLUDED_TOP
    FILT+=(--include='/configs/')
    for f in "${BAND2_CONFIGS[@]}"; do FILT+=(--include="/configs/$f"); done
    for f in "${BAND2_RUNTIME_PATCH_STEMS[@]}"; do
        FILT+=(--include="/configs/${f}_patch.py" --include="/configs/${f}_patch.provenance.json")
    done
    FILT+=(--exclude='/configs/*')                # 나머지 configs(모델 트리플렛 Band3) 배제
    FILT+=(--include='/envs/')
    for f in "${BAND2_ENVS[@]}"; do FILT+=(--include="/envs/$f"); done
    FILT+=(--exclude='/envs/*')                   # 나머지 envs(모델 env Band3) 배제
    # (d-rsync-3) 최상위는 default-include 가 아니라 명시 allowlist + terminal exclude → stray Band1/secret/log·.dockerignore(§1.3 불요) 누출 차단
    for f in "${BAND2_TOP[@]}"; do FILT+=(--include="/$f"); done
    # 빌드 패치 모듈 디렉토리(Band2 빌드입력·서브 빌드가 COPY — §4.7·3+1+1).
    #   build_patches=post(컴파일 후) · build_patches_src=pre(컴파일 전, 소스 이식)
    # 배달=삭제 스코프 일치는 BAND2_PATCH_DIR_PAYLOAD 에서 파생한다(위 스탠자 근거). 규칙 3종:
    #   (a) `P /<d>/`  = **수신측 전용** protect. 정본에 <d> 자체가 없을 때(예: 손작성분 0 인
    #       build_patches_src) rsync 가 서브의 <d> 를 지우려다 `cannot delete non-empty directory`
    #       를 뱉는 것을 막는다. P 는 삭제 판정에만 걸리므로 **전송은 그대로** 된다(실측 확인).
    #       내용물이 전부 없어지면 빈 디렉토리가 되고 그건 prune_remote_empty_dirs 가 정비한다.
    #   (b) payload glob include = 전송 + 삭제 대상(stale 손작성분은 계속 회수된다)
    #   (c) terminal `--exclude=/<d>/*` = 나머지(파생 payload)는 전송 ✗ **삭제 ✗**(rsync 는 exclude 된
    #       수신측 항목을 보호한다 — configs/*·envs/* Band3 보호와 동일 메커니즘)
    for f in "${BAND2_PATCH_DIRS[@]}"; do
        [ -n "${BAND2_PATCH_DIR_PAYLOAD[$f]+set}" ] || {
            echo "[sync] FAIL(payload 미선언): BAND2_PATCH_DIRS 의 '$f' 가 BAND2_PATCH_DIR_PAYLOAD 에 없다." >&2
            echo "[sync]   → 선언 없이 두면 terminal exclude 만 남아 **아무것도 배달되지 않는다**(침묵 누락)." >&2
            return 1
        }
        FILT+=(--filter="P /$f/" --include="/$f/")
        # ⚠ `read -a` 로 쪼갠다 — 비인용 확장은 payload 의 `*` 가 **CWD 에 대해 경로확장**된다
        #   (실측: `[build_patches]='*'` 가 리포 최상위 15개 이름으로 터졌다). read 는 glob 을 하지 않는다.
        payload=(); IFS=' ' read -r -a payload <<<"${BAND2_PATCH_DIR_PAYLOAD[$f]}"
        for g in "${payload[@]}"; do FILT+=(--include="/$f/$g"); done
        FILT+=(--exclude="/$f/*")
    done
    FILT+=(--exclude='/*')
}

validate_runtime_patches() {  # $1=topology; every patch must bind current patch/model/resolution bytes
    local topology="$1" cdir="${SRC%/}/output/$1/configs" edir="${SRC%/}/output/$1/envs"
    local patch sidecar found=0
    for patch in "$cdir"/*_patch.py; do
        [ -f "$patch" ] || continue
        if [ "$found" = 0 ]; then
            [ -f "$PATCH_VALIDATOR" ] || { echo "[sync] FAIL(runtime patch): validator missing: $PATCH_VALIDATOR" >&2; return 1; }
            [ -f "$PATCH_RESOLUTION" ] || { echo "[sync] FAIL(runtime patch): canonical resolution missing" >&2; return 1; }
            found=1
        fi
        python3 "$PATCH_VALIDATOR" verify --patch "$patch" --topology "$topology" \
            --config-dir "$cdir" --env-dir "$edir" --resolution "$PATCH_RESOLUTION" || return 1
    done
    # Orphan sidecars are stale authority too: never deliver one without its exact patch.
    for sidecar in "$cdir"/*_patch.provenance.json; do
        [ -f "$sidecar" ] || continue
        patch="${sidecar%.provenance.json}.py"
        [ -f "$patch" ] || { echo "[sync] FAIL(runtime patch): orphan provenance $sidecar" >&2; return 1; }
    done
    return 0
}

# ── S4 fail-loud band 분류 단언(plan ⊕rev3 keying linter) ──
# output/<t>/ 의 (a) 최상위 (b) configs/ (c) envs/ 모든 항목이 Band2(allowlist) 또는 Band3(완전 모델 트리플렛
# .sh+.yaml / .env.<model>)로 명확 분류되는지 + (멀티) Band2 인프라가 소스에 실재하는지 검증. 미분류/누락 → 비-0.
# ── BAND2_TOP ↔ .gitignore 재포함 목록 교차검증 (2026-08-13 · plan_26081314 D4) ──────────────
# 두 목록은 **같은 개념**(어떤 최상위 파일이 Band2 빌드킷인가)을 서로 다른 평면에 적는다:
#   - BAND2_TOP        → rsync 전송 allowlist(무엇을 서브로 보내는가)
#   - .gitignore `!`   → git 추적 allowlist(무엇이 정본 스냅샷에 들어가는가)
# 정적 파일이라 한쪽에서 다른 쪽을 **생성할 수 없으므로**(단일 소유 불가) 차선으로 **정합을 검증**한다.
# 갈라지면 어떤 일이 벌어지는지는 실증됐다: 8/11 에 gitignore 만 3종→11종으로 늘고 BAND2_TOP 은
# 그대로여서, 추적은 되는데 전송되지 않거나 그 반대인 상태가 생겼다(plan_26081310 §배경 #1·#6).
# 전송 대상인데 추적되지 않으면 index-권위 스냅샷에서 **조용히 빠진다** — 침묵 누락의 전형이다.
# ★ 검증 대상은 **두 벌**이다(2026-08-13 확장). 메인 `.gitignore` 만 보던 초판은 서브측 사본을 놓쳤다:
#   서브의 추적규칙은 메인 `.gitignore` 가 아니라 `sub_node/gitignore.template` 이 정한다(render_sub_env.py
#   가 sub_provision/.gitignore 로 복제 → deliver_overlay 가 배달). 그 템플릿은 `!output/` 예외가 **4개**뿐
#   (multi Dockerfile 3종 + .gitkeep)인 채 멈춰 있었고 메인은 **19개**였다 → 서브 git 은 자기가 받은
#   빌드킷을 **하나도 추적하지 못했다**. 서브 git 은 메인의 **관측 장치**인데(CLAUDE.md 권한평면 B),
#   그 눈이 가려져 `[sync]` 커밋이 배달분을 과소기록했다 — 2026-08-13 requirements.txt 오배달이
#   3주간 침묵한 배경이 이것이다. 세 벌(BAND2_* ↔ 메인 gitignore ↔ 서브 template)을 손으로 유지하면
#   반드시 갈라지므로, **배열이 소유자**이고 두 정적 파일은 여기서 대조된다.
assert_band2_top_gitignore_parity() {  # 0=ok, 1=drift
    local gi="${SRC%/}/.gitignore"
    local sub_gi="${SRC%/}/.claude/skills/terraforming_node/sub_node/gitignore.template"
    local f t d g bad=0 label path; local -a payload
    [ -f "$gi" ] || { echo "[sync] FAIL(parity): .gitignore 부재 — 교차검증 불가" >&2; return 1; }
    [ -f "$sub_gi" ] || { echo "[sync] FAIL(parity): sub_node/gitignore.template 부재 — 교차검증 불가" >&2; return 1; }
    for label in "메인:$gi" "서브템플릿:$sub_gi"; do
        path="${label#*:}"; label="${label%%:*}"
        for t in multi single; do
            for f in "${BAND2_TOP[@]}"; do
                [ "$f" = ".gitkeep" ] && continue      # .gitkeep 은 `!output/*/.gitkeep` 와일드카드가 커버
                grep -qxF "!output/$t/$f" "$path" || {
                    echo "[sync] FAIL(parity/$label): BAND2_TOP 에 '$f' 가 있으나 '!output/$t/$f' 예외가 없다." >&2
                    echo "[sync]   → 전송 대상인데 추적되지 않으면 index-권위 스냅샷에서 침묵 누락된다(plan_26081314 D4)." >&2
                    bad=1
                }
            done
            for d in "${BAND2_PATCH_DIRS[@]}"; do      # 디렉토리는 자기 줄 + 내용 줄(`/*` 또는 `/**`) 둘 다 필요
                grep -qxF "!output/$t/$d/" "$path" || {
                    echo "[sync] FAIL(parity/$label): BAND2_PATCH_DIRS 의 '$d' 에 '!output/$t/$d/' 예외가 없다." >&2; bad=1; }
                # 내용 예외는 **선언된 payload glob 과 1:1 대조**한다(2026-08-14 강화).
                #   초판은 "최소 1줄 존재"만 봤다 — 범위를 정규식으로 못박으면 밴드 규정이 바뀔 때마다
                #   정규식을 고쳐야 한다는 이유였다. 그 이유는 이제 성립하지 않는다: 범위의 소유자가
                #   손으로 적은 정규식이 아니라 **BAND2_PATCH_DIR_PAYLOAD 선언**이므로, 규정이 바뀌면
                #   배열만 고치면 되고 이 대조는 자동으로 따라온다.
                #   느슨함의 실해악은 실측됐다: 2026-08-13 에 gitignore 를 `/**`→`*.sh`+`PROVENANCE.json`
                #   으로 좁혔을 때 "1줄은 남아 있어서" 게이트가 통과했고, 그 결과 추적(배달)에서 빠진
                #   92파일이 rsync 삭제 스코프에는 그대로 남아 **배달 ✗ / 삭제 ✓** 상태가 침묵으로 유지됐다.
                payload=(); IFS=' ' read -r -a payload <<<"${BAND2_PATCH_DIR_PAYLOAD[$d]:-}"  # 비인용 확장 금지(경로확장 위험)
                for g in "${payload[@]}"; do
                    grep -qxF "!output/$t/$d/$g" "$path" || {
                        echo "[sync] FAIL(parity/$label): '$d' 의 payload glob '$g' 에 '!output/$t/$d/$g' 예외가 없다." >&2
                        echo "[sync]   → 배달(index 권위)과 삭제(rsync include)가 갈라진다 — 보내지 않는 것을 지우게 된다." >&2
                        bad=1; }
                done
            done
        done
    done
    return $bad
}

assert_band_classification() {  # $1=topology → 0=ok, 1=미분류·누락
    local odir="${SRC%/}/output/$1" cdir="${SRC%/}/output/$1/configs" edir="${SRC%/}/output/$1/envs" bad=0 f b ok stem
    local nullsave; nullsave="$(shopt -p nullglob dotglob || true)"; shopt -s nullglob dotglob
    local -A _b2c _b2e _b2top _b2p
    for b in "${BAND2_CONFIGS[@]}"; do _b2c["$b"]=1; done
    for b in "${BAND2_ENVS[@]}"; do _b2e["$b"]=1; done
    for b in "${BAND2_RUNTIME_PATCH_STEMS[@]}"; do
        _b2p["${b}_patch.py"]=1
        _b2p["${b}_patch.provenance.json"]=1
    done
    # 제외 목록은 손으로 적지 않는다 — BAND2_EXCLUDED_TOP 단일 소유에서 파생한다(네 번째 소비자).
    for b in "${BAND2_TOP[@]}" configs envs "${BAND2_PATCH_DIRS[@]}" "${BAND2_EXCLUDED_TOP[@]}"; do _b2top["$b"]=1; done

    # (a) (d-cg-4) 최상위 — 빌드킷·서브디렉토리·의도적 제외(manifest/sub_provision) 외 미지 항목 fail-loud
    for f in "$odir"/*; do
        b="$(basename "$f")"
        [ -n "${_b2top[$b]:-}" ] && continue
        echo "[sync] FAIL(S4 미분류 top-level): output/$1/$b — Band2 빌드킷이면 BAND2_TOP 추가 · Band3/생성물이면 _band2_filters 에 --exclude 추가." >&2; bad=1
    done

    # (b) configs/
    for f in "$cdir"/*; do
        [ -f "$f" ] || continue                                          # (d-band-1) 디렉토리/비정규 skip
        b="$(basename "$f")"; [ "$b" = ".gitkeep" ] && continue
        [ -n "${_b2c[$b]:-}" ] && continue                              # Band2 인프라(allowlist)
        [ -n "${_b2p[$b]:-}" ] && continue                              # owner-local provenance-bound runtime patch
        ok=0
        case "$b" in                                                     # (d-band-2) 짝의 .sh 가 Band2 면 Band3 로 green-light 안 함(stem 충돌 차단)
            *_patch.py) stem="${b%_patch.py}"; { [ -f "$cdir/$stem.sh" ] || [ -f "$cdir/$stem.yaml" ]; } && [ -f "$cdir/${stem}_patch.provenance.json" ] && ok=1 || true ;;
            *_patch.provenance.json) stem="${b%_patch.provenance.json}"; [ -f "$cdir/${stem}_patch.py" ] && ok=1 || true ;;
            *.sh)   stem="${b%.sh}";   [ -f "$cdir/$stem.yaml" ] && ok=1 || true ;;
            *.yaml) stem="${b%.yaml}"; { [ -f "$cdir/$stem.sh" ] && [ -z "${_b2c[$stem.sh]:-}" ]; } && ok=1 || true ;;
        esac
        [ "$ok" = 1 ] && continue
        echo "[sync] FAIL(S4 미분류): output/$1/configs/$b — Band2 인프라면 BAND2_CONFIGS 추가 · Band3 모델이면 .sh+.yaml 짝 확인." >&2; bad=1
    done

    # (c) envs/
    for f in "$edir"/*; do
        [ -f "$f" ] || continue
        b="$(basename "$f")"; [ "$b" = ".gitkeep" ] && continue
        [ -n "${_b2e[$b]:-}" ] && continue                              # Band2 네트워크 env(allowlist)
        ok=0
        case "$b" in
            .env.*) stem="${b#.env.}"; { { [ -f "$cdir/$stem.sh" ] || [ -f "$cdir/$stem.yaml" ]; } && [ -z "${_b2c[$stem.sh]:-}" ]; } && ok=1 || true ;;  # 모델 env → Band3
        esac
        [ "$ok" = 1 ] && continue
        echo "[sync] FAIL(S4 미분류): output/$1/envs/$b — Band2면 BAND2_ENVS 추가 · Band3 모델 env 면 configs 짝 확인." >&2; bad=1
    done

    # (d-bcf-4) 양방향 enforcement: Band2 config 와 짝맞는 .env.<base> 가 있으면 BAND2_ENVS 등록 필수(아니면 모델로 오분류·drop)
    for b in "${BAND2_CONFIGS[@]}"; do
        stem="${b%.*}"
        if [ -f "$edir/.env.$stem" ] && [ -z "${_b2e[.env.$stem]:-}" ]; then
            echo "[sync] FAIL(S4 양방향): output/$1/envs/.env.$stem 가 Band2 config '$b' 와 짝이나 BAND2_ENVS 미등록." >&2; bad=1
        fi
    done

    # (d-rsync-1) every active topology requires the canonical Band2 runner set.  Dormant
    # single is skipped by the caller before render/classification.  Multi additionally
    # requires its network env set.
    for b in "${BAND2_CONFIGS[@]}"; do [ -f "$cdir/$b" ] || { echo "[sync] FAIL(S4 통로미완결): output/$1/configs/$b 부재 — canonical materialize 선행." >&2; bad=1; }; done
    if [ "$1" = "multi" ]; then
        for b in "${BAND2_ENVS[@]}"; do [ -f "$edir/$b" ] || { echo "[sync] FAIL(S4 통로미완결): output/$1/envs/$b 부재 — render(NCCL/cluster envfile, S6) 선행." >&2; bad=1; }; done
    fi

    eval "$nullsave"; return $bad
}

# Executable runner integrity is metadata, not merely content.  rsync -a preserves the
# source mode, so reject a stale/materialized 0644/0664 source before transport and verify
# the remote destination after delivery.  Active single and multi both require all runners;
# dormant single is skipped before materialization.
normalize_canonical_runner_modes() {
    local canonical="${SRC%/}/.claude/skills/upstream-version-watch/assets/configs" runner mode bad=0
    for runner in "${BAND2_CONFIGS[@]}"; do
        if [ ! -f "$canonical/$runner" ]; then
            echo "[sync] FAIL(canonical runner mode): missing $canonical/$runner" >&2
            bad=1
            continue
        fi
        chmod 0755 "$canonical/$runner" || { echo "[sync] FAIL(canonical runner mode): chmod 0755 failed: $runner" >&2; bad=1; continue; }
        mode="$(stat -c '%a' "$canonical/$runner" 2>/dev/null || true)"
        if [ "$mode" != "755" ]; then
            echo "[sync] FAIL(canonical runner mode): $runner mode=${mode:-missing} after normalization" >&2
            bad=1
        fi
    done
    return $bad
}

assert_source_runner_modes() {  # $1=topology -- exact canonical bytes + exact 0755
    local topology="$1" cdir="${SRC%/}/output/$1/configs" canonical="${SRC%/}/.claude/skills/upstream-version-watch/assets/configs" runner mode canonical_mode bad=0
    for runner in "${BAND2_CONFIGS[@]}"; do
        if [ ! -f "$canonical/$runner" ]; then
            echo "[sync] FAIL(source runner bytes): canonical asset missing: .claude/skills/upstream-version-watch/assets/configs/$runner" >&2
            bad=1
        else
            canonical_mode="$(stat -c '%a' "$canonical/$runner" 2>/dev/null || true)"
            if [ "$canonical_mode" != "755" ]; then
                echo "[sync] FAIL(canonical runner mode): .claude/skills/upstream-version-watch/assets/configs/$runner mode=${canonical_mode:-missing}, expected=755" >&2
                bad=1
            else
                echo "[sync] canonical runner mode: .claude/skills/upstream-version-watch/assets/configs/$runner=755"
            fi
        fi
        if [ ! -f "$cdir/$runner" ]; then
            echo "[sync] FAIL(source runner bytes): materialized runner missing: output/$topology/configs/$runner" >&2
            bad=1
        elif ! cmp -s "$canonical/$runner" "$cdir/$runner"; then
            echo "[sync] FAIL(source runner bytes): output/$topology/configs/$runner differs from canonical asset" >&2
            bad=1
        else
            echo "[sync] source runner bytes: output/$topology/configs/$runner=canonical"
        fi
        mode="$(stat -c '%a' "$cdir/$runner" 2>/dev/null || true)"
        if [ "$mode" != "755" ]; then
            echo "[sync] FAIL(source runner mode): output/$topology/configs/$runner mode=${mode:-missing}, expected=755" >&2
            bad=1
        else
            echo "[sync] source runner mode: output/$topology/configs/$runner=755"
        fi
    done
    return $bad
}

verify_destination_runner_modes() {  # $1=topology -- exact canonical bytes + exact 0755
    local topology="$1" cdir="${SRC%/}/output/$1/configs" canonical="${SRC%/}/.claude/skills/upstream-version-watch/assets/configs" runner mode expected actual bad=0
    for runner in "${BAND2_CONFIGS[@]}"; do
        if [ ! -f "$cdir/$runner" ]; then
            echo "[sync] FAIL(destination runner bytes): source materialized runner missing: output/$topology/configs/$runner" >&2
            bad=1
            continue
        fi
        expected="$(sha256sum "$canonical/$runner" 2>/dev/null | awk '{print $1}')"
        actual="$(sub_run "sha256sum 'output/$topology/configs/$runner' 2>/dev/null" | awk '{print $1}' || true)"
        if [ -z "$expected" ] || [ "$actual" != "$expected" ]; then
            echo "[sync] FAIL(destination runner bytes): output/$topology/configs/$runner sha256=${actual:-missing}, canonical=${expected:-missing}" >&2
            bad=1
        else
            echo "  ✅ destination runner bytes output/$topology/configs/$runner=canonical"
        fi
        mode="$(sub_run "stat -c '%a' 'output/$topology/configs/$runner' 2>/dev/null" || true)"
        if [ "$mode" != "755" ]; then
            echo "[sync] FAIL(destination runner mode): output/$topology/configs/$runner mode=${mode:-missing}, expected=755" >&2
            bad=1
        else
            echo "  ✅ destination runner mode output/$topology/configs/$runner=755"
        fi
    done
    return $bad
}
OVERLAY_EXCLUDES=(--exclude '__pycache__' --exclude '*.pyc')
# Explicit migration tombstones only. Overlay remains additive for every other path; this narrow
# list closes known source relocations after the replacement has been delivered and verified.
OVERLAY_STALE_PATHS=(
    .claude/rules/references.md
    scripts/install_host_safety.sh
    scripts/mem_watchdog.sh
    scripts/host/vllm-drop-caches.sh
    scripts/systemd/easy-vllm-memwatch.service
    scripts/smoke_clone.sh
    scripts/sync_branches.sh
    # 2026-09-06(plan_26090616 ②): 릴레이 원장이 `campaigns/<camp-id>/relay/` 로 이관되면서
    #   루트 `tasks/` 는 폐지됐다(root_registry tombstone). 오버레이는 **가산**이라 지운 자리가
    #   서브에 그대로 남는다 — 남으면 서브가 옛 자리에 계속 쓰고, 그 원장은 어느 캠페인의
    #   왕복인지 알 수 없는 채로 다음 캠페인 입력과 섞인다(이관의 목적이 무효가 된다).
    #   ★ 지우는 것은 **스캔폴드 마커뿐**이다. `tasks/` 디렉터리 자체는 건드리지 않는다 —
    #     거기엔 서브가 저작한 살아 있는 원장이 들어 있고, 그것은 서브 소유 평면이다(무단 교정
    #     금지). 게다가 `apply_overlay_tombstones` 는 `rm -f` 라 디렉터리를 지우지도 못한다 —
    #     목록에 디렉터리를 적으면 **조용히 아무 일도 일어나지 않는다**(침묵 no-op).
    #     서브의 새 작업은 CLAUDE.template/comms.md 가 가리키는 새 자리로 간다.
    tasks/.gitkeep
    # 2026-09-08(plan_26090813 §4.2): 메인의 **해소 결과**는 싱글 서브에 가지 않는다. 오버레이는
    #   가산이라 이미 배달된 사본은 스스로 사라지지 않고, 남으면 서브가 자기 HW 로 해소할 이유가
    #   없어진다(2026-09-07 실측: 서브 library_request 5회 전부 null — 물을 이유가 없었다).
    .claude/skills/upstream-version-watch/assets/current-production-resolution.json
)
OVERLAY_RELOCATION_STALE_PATHS=(
    .claude/rules/references.md
    scripts/install_host_safety.sh
    scripts/mem_watchdog.sh
    scripts/host/vllm-drop-caches.sh
    scripts/systemd/easy-vllm-memwatch.service
    # 2026-09-06: 릴레이 원장의 **정본 자리**가 campaigns/<camp-id>/relay/ 로 옮겨졌다.
    #   은퇴가 아니라 이관인 이유: 서브에 대체 자리가 실재한다(CLAUDE.template/comms.md 가
    #   가리킨다). 은퇴로 분류하면 활성 소비자 감사가 서는데, 서브의 `tasks/` 에는 **서브가
    #   저작한 살아 있는 원장**이 남아 있어(의도적으로 건드리지 않는다) 그 감사는 정당하게
    #   막는다 — 잘못된 분류가 배달을 교착시킨다.
    #   ★ 2026-09-06 커밋 3263339 는 상위 배열에만 넣고 여기를 빠뜨려 파티션 단언을 깼다.
    tasks/.gitkeep
)
# Main-only orchestration has no sub runtime replacement.  Deletion is retirement and therefore
# requires an explicit active-consumer audit before these exact paths are removed.
OVERLAY_RETIREMENT_STALE_PATHS=(
    scripts/smoke_clone.sh
    scripts/sync_branches.sh
)

# ── 서브 git 헬퍼 ──
# 2026-09-03(F1·F6·F7·F9 · plan_26090317 P1): 이 헬퍼들은 원격 판독 실패(ssh rc255 · cd 실패 · .git 권한/인덱스
#   손상 rc128)를 **정상 상태**로 접고 있었다 — `2>/dev/null` 로 사유까지 지운 채. "모르는 것" 과 "없는 것" 이
#   같은 값이 되면 그 위의 게이트는 전부 fail-open 이다(dirty 미보존 배달 · B0 오발동 · origin-0 거짓확증).
#   처방: 원인을 살리고(stderr 유지) rc 를 전파하며, `[ -d ]` 는 rc 1(부재)만 부재로 읽는다.
sub_run()  { $SSH_OPTS "$SUB_HOST" "cd '$SUB_WORK_DIR' && $1"; }
# 0=존재 · 1=부재(확정) · 2=판독불가(트랜스포트/권한) — 호출부는 2 를 fail-closed 로 다뤄야 한다.
sub_has_git() {
    local rc=0
    $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR/.git' ]" || rc=$?
    case "$rc" in 0) return 0 ;; 1) return 1 ;; *) return 2 ;; esac
}
sub_dirty() { sub_run "git status --porcelain"; }
sub_commit() { sub_run "git -c user.name='$GIT_NAME' -c user.email='$GIT_EMAIL' commit -q -m \"$1\""; }
sub_branch_current() { sub_run "git rev-parse --abbrev-ref HEAD"; }

render_topology() {
    normalize_canonical_runner_modes || return 9
    local output_dir="${SRC%/}/output/$1"
    local build_assets="${SRC%/}/.claude/skills/upstream-version-watch/assets/build_plane"
    mkdir -p "$output_dir/configs" "$output_dir/envs" || return 9
    # ⚠ requirements.txt 를 여기서 install 하지 않는다(2026-08-13 제거). 그 줄은 checkout-index 가 방금
    #   가져온 **인덱스 정본**(output/<t>/requirements.txt)을 Band1 정적 사본으로 덮어썼고, 그 사본에는
    #   갱신 소유자가 없었다 — `regen_requirements.py -o` 는 기본값이 cwd 의 `requirements.txt` 라 bump 는
    #   늘 output/<t>/ 만 갱신한다. 결과: 사본은 7/28(vLLM 0.18.0 METADATA)에 얼어붙고 서브는 0.19 이후
    #   모든 배달에서 **0.18.0 핀**을 받았다. 2026-08-13 서브 0.27.0 소스빌드가 여기서 죽었다 —
    #   constraint `transformers<5,>=4.56.0` 대 vLLM 요구 `transformers>=5.5.3` → ResolutionImpossible.
    #   침묵한 이유: verify_distribution 의 단언은 존재·비어있지않음·비실행뿐이라 **내용 신선도**를 못 본다.
    #   ∴ 소유자는 per-topology `output/<t>/requirements.txt` 하나다(핀은 브랜치별로 독립 — CLAUDE.md).
    #   토폴로지 통로 완결성은 assert_buildkit_completeness 가 이미 존재로 단언한다.
    install -m 0644 "$build_assets/Dockerfile.source-build-upstage" "$output_dir/Dockerfile.source-build-upstage" || return 9
    if [ "$1" = "multi" ]; then
        # 패치 0건은 **정상 상태**다: policy:RUNTIME_PATCH_NO_CARRY_FORWARD.C2 가 버전 bump 마다 재유도를
        # 요구하므로, bump 직후엔 정본 runtime_patches/ 가 비어 있는 게 규정된 결과다(또한 fresh clone 도 동일).
        # 옛 무조건 glob 는 이 정상 상태에서 `install: cannot stat …/*` 로 죽었다 → 존재할 때만 install.
        if compgen -G "$build_assets/runtime_patches/*" >/dev/null; then
            install -m 0644 "$build_assets"/runtime_patches/* "$output_dir/configs/" || return 9
        else
            echo "[sync] info: 정본 runtime patch 0건 — policy:RUNTIME_PATCH_NO_CARRY_FORWARD(버전 bump 시 재유도) 정합 상태"
        fi
        install -m 0644 "$build_assets"/model_inputs/configs/* "$output_dir/configs/" || return 9
        install -m 0644 "$build_assets"/model_inputs/envs/.env.* "$output_dir/envs/" || return 9
    fi
    python3 "${SRC%/}/.claude/skills/upstream-version-watch/scripts/render_dockerfile.py" \
        --materialize-configs --repo "${SRC%/}" --topology "$1" >/dev/null || return 9
    if [ "$1" = "multi" ]; then
        python3 "${SRC%/}/.claude/skills/upstream-version-watch/scripts/render_dockerfile.py" \
            --nccl-envfile --manifest "$output_dir/manifest.yaml" --out "$output_dir/envs/.env.interconnect" >/dev/null || return 9
        python3 "${SRC%/}/.claude/skills/upstream-version-watch/scripts/render_dockerfile.py" \
            --cluster-envfile --manifest "$output_dir/manifest.yaml" --out "$output_dir/envs/.env.cluster" >/dev/null || return 9
    fi
    # 2026-09-03(S10/F3 · plan_26090317 P1): 트랜잭션 소스는 `checkout-index --prefix` 트리라 `.git` 이
    #   없다 — 거기서 render 가 `git ls-files` 를 부르면 rc=128 이고, 옛 판본은 그걸 조용히 os.walk 폴백으로
    #   대체하면서 화면엔 "(N tracked files)" 라고 찍었다(거짓 표기). 이제 render 는 목록 없이 복제하지
    #   않으므로, **정본 레포에서 뽑은 tracked 목록을 명시 주입**한다.
    _tracked_list="$(mktemp)"
    git -C "$REPO_ROOT" ls-files -z > "$_tracked_list" || {
        rm -f "$_tracked_list"; echo "[sync] FAIL: tracked 목록 생성 실패 — 런타임블럭 배달 중단" >&2; return 9; }
    python3 "$RENDER" --topology "$1" --tracked-list "$_tracked_list" >/dev/null || {
        rm -f "$_tracked_list"; return 9; }
    rm -f "$_tracked_list"
    if [ "$1" != "multi" ] && [ -d "$output_dir/envs" ] \
        && [ -z "$(find "$output_dir/envs" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
        rmdir "$output_dir/envs" || return 9
    fi
}
staging_dir()     { echo "${SRC%/}/output/$1/sub_provision"; }

TRANSACTIONAL_SRC=""
REMOTE_TX_DIRS=()
REMOTE_TX_BRANCHES=()
REMOTE_TX_HEADS=()
REMOTE_TX_BOOTSTRAPS=()
REMOTE_TX_ACTIVE=0
REMOTE_ORIGINAL_BRANCH=""
PROVISIONED_BY_SYNC=0
BOOTSTRAP_TX_PREPARED=0
cleanup_transactional_source() {
    if [ -n "$TRANSACTIONAL_SRC" ] && [ -d "$TRANSACTIONAL_SRC" ]; then
        rm -rf -- "$TRANSACTIONAL_SRC"
    fi
}

validate_inventory_relative_path() { # $1=repo-relative path
    local rel="$1"
    if [[ "$rel" =~ [[:space:][:cntrl:]] ]]; then
        echo "[sync] FAIL: rollback line protocol forbids whitespace/control path: $rel" >&2
        return 9
    fi
    case "$rel" in ''|/*|../*|*/../*|*/..)
        echo "[sync] FAIL: unsafe rollback path: $rel" >&2; return 9;;
    esac
}

validate_inventory_tree() { # $1=root  $2=1 이면 BAND2_EXCLUDED_TOP 최상위 prune(output/<t> 트리 전용)
    # prune 근거는 validate_remote_deletion_tree 와 동일 — 검사 범위 = 전송 범위.
    local root="$1" prune="${2:-0}" path rel ex
    [ -d "$root" ] || return 0
    while IFS= read -r -d '' path; do
        rel="${path#"$root/"}"
        if [ "$prune" = "1" ]; then
            for ex in "${BAND2_EXCLUDED_TOP[@]}"; do
                [ "$rel" = "$ex" ] || [ "${rel#"$ex/"}" != "$rel" ] && continue 2
            done
        fi
        validate_inventory_relative_path "$rel" || return 9
        if [ -d "$path" ] && [ -z "$(find "$path" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
            echo "[sync] FAIL: empty directories are undeclared transfer artifacts: $path" >&2
            return 9
        fi
    done < <(find "$root" -mindepth 1 -print0)
}

# ── 배달 전제 복구: 빈 디렉터리 prune (2026-08-14 신설 · plan_26081409 A) ────────────────
# 왜 게이트가 아니라 정비인가:
#   롤백 스냅샷은 `find -type f -o -type l` 로 **파일·심링크만** 담는다. 대부분의 디렉터리는 그 안의
#   파일 경로로 **함의**되므로(파일을 복원하면 부모도 생김) 담을 필요가 없다. 그런데 **빈 디렉터리는
#   파일 경로로 함의되지 않는 유일한 디렉터리**라 롤백이 복원할 수 없다 — 그래서 옛 검증기는 트리에
#   빈 디렉터리가 하나라도 있으면 배달 전체를 거부했다. 논리는 일관되지만 **거부 뒤 복구 경로가 없었다**.
#   실해악: 2026-08-01 엔 매번 사람 sudo, 2026-08-14 엔 사람 ssh rmdir 이 필요했다(둘 다 실측).
#   그러나 빈 디렉터리는 **정보량이 0**이고 mkdir -p 로 완전 복원된다 — 즉 "인벤토리가 디렉터리를 못
#   담는다"는 **구현 제약**을 손실방지라는 **정책 게이트**로 표현한 범주 오류였다. 제약은 구현에서 푼다.
#   메인은 서브에 대해 이 정도의 정비 권한을 갖는다(헌법 평면 B §파이프라인 정비 — 저작도 스캔도 아니다).
# ⚠ 침묵 삭제 금지 — 제거한 경로를 항상 출력한다(docs.md 의 log_evicted 규율과 동형).
prune_remote_empty_dirs() { # $1=topology → 0=ok(제거분 로그), 9=실패
    local root="${DEST}output/$1" skip out
    skip="$(IFS=:; printf '%s' "${BAND2_EXCLUDED_TOP[*]}")"
    out="$($SSH_OPTS "$SUB_HOST" "python3 -c 'import os,sys
root,skip=sys.argv[1],set(x for x in sys.argv[2].split(\":\") if x)
if not os.path.isdir(root): raise SystemExit(0)
removed=[]
def excluded(base):
 # ★ 전송 범위 밖은 **통째로** 건너뛴다 — 최상위 디렉터리 자신뿐 아니라 그 **하위 전부**.
 #   topdown=False 라 dirs[:] prune 이 안 먹으므로 상대경로 첫 성분으로 판정한다.
 #   (2026-08-14 실측: 이 판정을 최상위 basename 으로만 했더니 cache/vllm/dummy_cache 를 지웠다 —
 #    rsync 가 건드리지 않는 영역을 파이프라인이 정비한 셈으로, 2026-08-01 사고와 동일한 형태의
 #    "검사 범위 ≠ 전송 범위" 위반이다.)
 rel=os.path.relpath(base,root)
 return rel!=\".\" and rel.split(os.sep)[0] in skip
# bottom-up: 안쪽을 지우면 바깥이 비므로 반복 없이 한 번에 수렴한다.
for base,dirs,files in os.walk(root, topdown=False):
 if base==root or excluded(base): continue
 try:
  if not os.listdir(base):
   os.rmdir(base); removed.append(os.path.relpath(base,root))
 except OSError as e:
  sys.stderr.write(\"prune-failed %s: %s\n\" % (base,e)); raise SystemExit(9)
print(\"\n\".join(removed))
' '$root' '$skip'")" || { echo "[sync] FAIL: 서브 빈 디렉터리 prune 실패($1) — 권한/경합 확인" >&2; return 9; }
    if [ -n "$out" ]; then
        echo "[sync] [$1] 배달 전제 복구: 빈 디렉터리 $(printf '%s\n' "$out" | grep -c .)건 제거(정보량 0 · 롤백 인벤토리가 담지 못하는 유일한 형태)" >&2
        printf '%s\n' "$out" | sed 's|^|    prune: output/'"$1"'/|' >&2
    fi
}

validate_remote_deletion_tree() { # $1=topology
    # ★ 검사 범위는 **전송 범위와 일치**해야 한다. rsync 가 제외하는 최상위 경로(BAND2_EXCLUDED_TOP)는
    #   이 트랜잭션이 바꿀 수 없으므로 롤백 인벤토리에 들 이유가 없고, 따라서 검사 대상도 아니다.
    #   (2026-08-01: 이 prune 이 없어서 컨테이너가 root 로 만든 빈 cache 디렉터리가 배달을 영구 차단했다.)
    # 빈 디렉터리 검사는 2026-08-14 에 **prune_remote_empty_dirs 로 이관**했다(위 스탠자 참조) —
    #   여기 남는 것은 경로 주입 방어(whitespace/control)뿐이며 그것은 그대로 fail-closed 다.
    # ⚠ 실패 시 **어느 경로인지 출력한다** — 옛 메시지는 원인 셋을 한 문장에 뭉치고 경로를 주지 않아
    #   운영자가 무엇을 고쳐야 할지 알 수 없었다(plan_26081310 D5: "안내문이 분류를 잘못 말하면
    #   가드가 있어도 사고가 난다" — 여기선 분류를 아예 하지 않았다).
    prune_remote_empty_dirs "$1" || return 9
    local root="${DEST}output/$1" skip bad
    skip="$(IFS=:; printf '%s' "${BAND2_EXCLUDED_TOP[*]}")"
    bad="$($SSH_OPTS "$SUB_HOST" "python3 -c 'import os,sys
root,skip=sys.argv[1],set(x for x in sys.argv[2].split(\":\") if x)
if not os.path.isdir(root): raise SystemExit(0)
hits=[]
for base,dirs,files in os.walk(root):
 if base==root: dirs[:]=[d for d in dirs if d not in skip]
 for name in dirs+files:
  if base==root and name in skip: continue
  raw=os.fsencode(name)
  if any(byte<=32 or byte==127 for byte in raw):
   hits.append(os.path.relpath(os.path.join(base,name),root))
print(\"\n\".join(hits))
' '$root' '$skip'")" || { echo "[sync] FAIL: 서브 삭제 인벤토리 검사 실패($1)" >&2; return 9; }
    if [ -n "$bad" ]; then
        echo "[sync] FAIL: 서브 경로에 공백/제어문자 — 경로 주입 방어로 fail-closed(대상 아래)." >&2
        printf '%s\n' "$bad" | sed 's|^|    bad-path: output/'"$1"'/|' >&2
        return 9
    fi
}

build_remote_touch_inventory() { # $1=topology $2=output file
    local t="$1" out="$2" st rel line scan
    st="$(staging_dir "$t")"
    : >"$out"
    validate_inventory_tree "$st" || return 9                    # 오버레이 staging — prune 없음(전량 전송 대상)
    validate_inventory_tree "${SRC}output/$t" 1 || return 9       # output/<t> — 전송 제외 최상위는 prune
    if [ -d "$st" ]; then
        while IFS= read -r -d '' rel; do printf '%s\n' "${rel#"$st/"}" >>"$out"; done \
            < <(find "$st" \( -type f -o -type l \) -print0)
    fi
    if [ -d "${SRC}output/$t" ]; then
        while IFS= read -r -d '' rel; do printf 'output/%s/%s\n' "$t" "${rel#"${SRC}output/$t/"}" >>"$out"; done \
            < <(find "${SRC}output/$t" \( -type f -o -type l \) -print0)
    fi
    printf '%s\n' "${OVERLAY_STALE_PATHS[@]}" >>"$out"
    # If deletion was explicitly authorized, inventory those destination-only Band2 paths too.
    if [ "${ALLOW_DELETE:-0}" -gt 0 ]; then
        validate_remote_deletion_tree "$t" || return 9
        _band2_filters || return 9
        scan="$(mktemp "${TMPDIR:-/tmp}/easy-vllm-delete-scan.XXXXXX")"
        if ! rsync -az --delete --dry-run --itemize-changes "${FILT[@]}" -e "$SSH_OPTS" \
            "${SRC}output/$t/" "$SUB_HOST:${DEST}output/$t/" >"$scan" 2>&1; then
            echo "[sync] FAIL: deletion inventory dry-run failed" >&2
            rm -f "$scan"; return 9
        fi
        while IFS= read -r line; do
            case "$line" in
                \*deleting*) rel="${line#\*deleting}"; rel="${rel#${rel%%[![:space:]]*}}";
                    [ -n "$rel" ] && printf 'output/%s/%s\n' "$t" "${rel%/}" >>"$out" ;;
            esac
        done <"$scan"
        rm -f "$scan"
    fi
    LC_ALL=C sort -u -o "$out" "$out"
    while IFS= read -r rel; do validate_inventory_relative_path "$rel" || return 9; done <"$out"
}

begin_remote_transaction() { # $1=topology $2=bootstrap(0/1)
    local t="$1" bootstrap="$2" list part tx head branch inv_t
    list="$(mktemp "${TMPDIR:-/tmp}/easy-vllm-sync-paths.XXXXXX")"
    : >"$list"
    for inv_t in "${PRECHECK_TARGETS[@]}"; do
        part="$(mktemp "${TMPDIR:-/tmp}/easy-vllm-sync-paths-part.XXXXXX")"
        build_remote_touch_inventory "$inv_t" "$part" \
            || { rm -f "$list" "$part"; return 9; }
        cat "$part" >>"$list"; rm -f "$part"
    done
    LC_ALL=C sort -u -o "$list" "$list"
    tx="/tmp/easy-vllm-sync-rollback.$$.${#REMOTE_TX_DIRS[@]}"
    if [ "$bootstrap" = "1" ]; then head=""; branch="multi";
    else
        head="$(sub_run "git rev-parse --verify '$t'")" \
            || { echo "[sync] FAIL(F8): 서브에 브랜치 '$t' 가 없거나 판독 불가 — 빈 head 로 트랜잭션을 등록하지 않는다." >&2; return 9; }
        branch="$t"
    fi
    if ! $SSH_OPTS "$SUB_HOST" "set -eu; umask 077; tx='$tx'; rm -rf -- \"\$tx\"; mkdir -p \"\$tx/backup\"; cat >\"\$tx/paths\"; : >\"\$tx/dirs\"; : >\"\$tx/existing-dirs\"; if [ -d '$SUB_WORK_DIR' ]; then : >\"\$tx/workdir-existed\"; [ '$bootstrap' != 1 ] || cp -a -- '$SUB_WORK_DIR' \"\$tx/workdir-backup\"; cd '$SUB_WORK_DIR'; while IFS= read -r p; do d=\$(dirname \"\$p\"); while [ \"\$d\" != . ]; do printf '%s\\n' \"\$d\" >>\"\$tx/dirs\"; [ ! -d \"\$d\" ] || printf '%s %s\\n' \"\$(stat -c '%a' \"\$d\")\" \"\$d\" >>\"\$tx/existing-dirs\"; d=\$(dirname \"\$d\"); done; if [ -e \"\$p\" ] || [ -L \"\$p\" ]; then mkdir -p \"\$tx/backup/\$(dirname \"\$p\")\"; cp -a -- \"\$p\" \"\$tx/backup/\$p\"; fi; done <\"\$tx/paths\"; elif [ '$bootstrap' = 1 ]; then : >\"\$tx/workdir-absent\"; else exit 9; fi; sort -u -o \"\$tx/dirs\" \"\$tx/dirs\"; sort -u -k2,2 -o \"\$tx/existing-dirs\" \"\$tx/existing-dirs\"" <"$list"; then
        rm -f "$list"
        if ! $SSH_OPTS "$SUB_HOST" "rm -rf -- '$tx'"; then
            echo "[sync] CRITICAL: failed transaction creation left recovery path $SUB_HOST:$tx" >&2
        fi
        return 9
    fi
    rm -f "$list"
    REMOTE_TX_DIRS+=("$tx"); REMOTE_TX_BRANCHES+=("$branch"); REMOTE_TX_HEADS+=("$head"); REMOTE_TX_BOOTSTRAPS+=("$bootstrap")
    REMOTE_TX_ACTIVE=1
    echo "[sync] remote rollback transaction prepared: branch=$branch paths=$($SSH_OPTS "$SUB_HOST" "wc -l < '$tx/paths'")"
}

# 원격 git 락 유계 대기. 롤백의 모든 git 조작 앞에 선다.
#
# ★ 왜 필요한가(2026-08-01 실화): 오케스트레이터가 SIGTERM 을 받으면 EXIT trap 이 **즉시** 롤백을
#   실행한다. 그런데 ssh 는 TTY 없이 돌므로 SIGHUP 이 원격에 전파되지 않아, 방금 띄운 `git add -A`
#   가 서브에서 **계속 살아있다**. 롤백은 그 고아가 쥔 index.lock 에 부딪혀 한 번 시도하고
#   CRITICAL 로 떨어졌다. 고아 프로세스는 막을 수 없지만, **기다리지 않는 것**은 고칠 수 있다.
#
# ★ 락을 **자동 삭제하지 않는다.** 같은 날 나는 이 락을 stale 로 오판했는데 실제로는 `git add -A`
#   가 살아 있었다 — 지웠다면 진행 중인 인덱스 쓰기를 깨뜨렸을 것이다. 판별만 하고 처방은 사람에게 넘긴다.
wait_for_remote_git_lock() {  # $1=최대 대기초(기본 90)
    local max="${1:-90}"
    $SSH_OPTS "$SUB_HOST" "
        lock='$SUB_WORK_DIR/.git/index.lock'
        [ -e \"\$lock\" ] || exit 0
        echo '[sync] 원격 git 락 관측 — 최대 ${max}s 대기(자동 삭제하지 않는다)' >&2
        i=0
        while [ -e \"\$lock\" ] && [ \"\$i\" -lt $max ]; do sleep 3; i=\$((i+3)); done
        if [ ! -e \"\$lock\" ]; then echo \"[sync] 원격 git 락 해제됨(대기 \${i}s)\" >&2; exit 0; fi
        if pgrep -f 'git (add|commit|checkout|reset|status)' >/dev/null 2>&1; then
            echo '[sync] 원격 git 이 여전히 실행 중 — 진행 중인 작업이다. 락을 지우지 말고 완료를 기다려라.' >&2
            exit 10
        fi
        echo '[sync] 원격 git 프로세스는 없는데 락이 남아 있다(stale 로 보인다). 사람이 확인 후 제거하면 재시도 가능:' >&2
        echo \"[sync]   ssh $SUB_HOST rm -f \$lock\" >&2
        exit 11
    "
}

rollback_remote_transactions() {
    [ ${#REMOTE_TX_DIRS[@]} -gt 0 ] || return 0
    echo "[sync] ROLLBACK: restoring ${#REMOTE_TX_DIRS[@]} remote transaction(s)" >&2
    # 락이 살아있는 채로 롤백에 들어가면 반드시 실패한다 — 먼저 기다린다. 대기가 실패해도
    # 롤백은 시도한다(백업은 어차피 보존되며, 아래 CRITICAL 메시지가 복구 경로를 남긴다).
    wait_for_remote_git_lock 90 || echo "[sync] WARN: 원격 락이 남은 채로 롤백을 시도한다 — 실패할 수 있다." >&2
    local i tx branch head fail=0 root_tx="${REMOTE_TX_DIRS[0]}"
    if [ "${REMOTE_TX_BOOTSTRAPS[0]}" = "1" ]; then
        $SSH_OPTS "$SUB_HOST" "set -eu; if [ -f '$root_tx/workdir-absent' ]; then rm -rf -- '$SUB_WORK_DIR'; elif [ -f '$root_tx/workdir-existed' ]; then rm -rf -- '$SUB_WORK_DIR'; mkdir -p -- \"\$(dirname '$SUB_WORK_DIR')\"; cp -a -- '$root_tx/workdir-backup' '$SUB_WORK_DIR'; else echo '[sync] FAIL: bootstrap transaction lacks workdir origin marker' >&2; exit 9; fi" || fail=1
    else
        for ((i=${#REMOTE_TX_DIRS[@]}-1; i>=0; i--)); do
            tx="${REMOTE_TX_DIRS[$i]}"; branch="${REMOTE_TX_BRANCHES[$i]}"; head="${REMOTE_TX_HEADS[$i]}"
            $SSH_OPTS "$SUB_HOST" "set -eu; cd '$SUB_WORK_DIR'; git reset --hard; git checkout -q '$branch'; git reset --hard '$head'; while IFS= read -r p; do rm -rf -- \"\$p\"; done <'$tx/paths'" || fail=1
        done
        if [ -z "$REMOTE_ORIGINAL_BRANCH" ]; then
            fail=1
        else
            $SSH_OPTS "$SUB_HOST" "set -eu; cd '$SUB_WORK_DIR'; git reset --hard; git checkout -q '$REMOTE_ORIGINAL_BRANCH'; git reset --hard; while IFS= read -r p; do rm -rf -- \"\$p\"; [ ! -e '$root_tx/backup/'\"\$p\" ] && [ ! -L '$root_tx/backup/'\"\$p\" ] || { mkdir -p \"\$(dirname \"\$p\")\"; cp -a -- '$root_tx/backup/'\"\$p\" \"\$p\"; }; done <'$root_tx/paths'; tac '$root_tx/dirs' | while IFS= read -r d; do cut -d' ' -f2- '$root_tx/existing-dirs' | grep -Fqx \"\$d\" || rmdir -- \"\$d\" 2>/dev/null || true; done; while read -r m d; do chmod \"\$m\" \"\$d\"; done <'$root_tx/existing-dirs'" || fail=1
        fi
    fi
    if [ "$fail" -ne 0 ]; then
        echo "[sync] CRITICAL: rollback failed; recovery backups retained:" >&2
        for tx in "${REMOTE_TX_DIRS[@]}"; do printf '  %s:%s\n' "$SUB_HOST" "$tx" >&2; done
        return 11
    fi
    for tx in "${REMOTE_TX_DIRS[@]}"; do
        $SSH_OPTS "$SUB_HOST" "rm -rf -- '$tx'" || echo "[sync] WARNING: rollback succeeded but backup cleanup failed: $SUB_HOST:$tx" >&2
    done
    REMOTE_TX_DIRS=(); REMOTE_TX_BRANCHES=(); REMOTE_TX_HEADS=(); REMOTE_TX_BOOTSTRAPS=(); REMOTE_TX_ACTIVE=0
}

finalize_remote_transactions() {
    local tx
    # Commits are already successful. Backup cleanup is best-effort and must never trigger an
    # impossible partial rollback after one transaction backup has been removed.
    REMOTE_TX_ACTIVE=0
    for tx in "${REMOTE_TX_DIRS[@]:-}"; do
        [ -z "$tx" ] || $SSH_OPTS "$SUB_HOST" "rm -rf -- '$tx'" \
            || echo "[sync] WARNING: successful sync left recovery backup: $SUB_HOST:$tx" >&2
    done
    REMOTE_TX_DIRS=(); REMOTE_TX_BRANCHES=(); REMOTE_TX_HEADS=(); REMOTE_TX_BOOTSTRAPS=()
}

transactional_exit() {
    local rc="$1"; trap - EXIT
    if [ "$rc" -ne 0 ] && [ "$REMOTE_TX_ACTIVE" = "1" ]; then
        rollback_remote_transactions || rc=11
    fi
    cleanup_transactional_source
    exit "$rc"
}
prepare_transactional_source() {
    # Rendering is intentionally destructive/idempotent inside its output root.  Never point it at
    # the caller's canonical working tree: dry-run must be byte/mode read-only, and apply preflight
    # must not overwrite local generated/dirty files before remote dirty-tree rejection.
    TRANSACTIONAL_SRC="$(mktemp -d "${TMPDIR:-/tmp}/easy-vllm-sync-source.XXXXXX")"
    chmod 0700 "$TRANSACTIONAL_SRC"
    trap 'transactional_exit $?' EXIT
    trap 'exit 130' INT TERM
    mkdir -p "$TRANSACTIONAL_SRC/output"
    if ! git -C "$CANONICAL_SRC" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
        echo "[sync] FAIL: canonical SRC must be a Git checkout; filesystem fallback is forbidden" >&2
        return 9
    fi
    # Git index bytes/modes are the accepted control/build-plane authority. Mutable or untracked
    # output files never enter the snapshot. The filesystem exceptions are the untracked *render
    # inputs*: manifest.yaml, the A2A signing key, and the issued sub manifest. Each is topology
    # input (possibly PII), is explicitly copied mode 0600, deterministically renders the
    # transaction, and is excluded from remote delivery by _band2_filters (BAND2_EXCLUDED_TOP).
    # ⚠ 2026-09-05(②-b): 이 목록이 manifest.yaml 하나였을 때, 카드 서명·서브 manifest 를 render 입력으로
    #   새로 만든 변경이 여기까지 오지 않아 **트랜잭션 안의 render 가 "서명키 없음" 으로 죽었다**.
    #   메인 워킹트리에서 돌린 render 는 성공했으므로 단위검사로는 보이지 않았고, 라이브 dry-run 이
    #   잡았다("만든 것과 도는 것은 다르다"). render 가 새 비추적 입력을 요구하면 여기도 같이 고친다.
    # ⚠ 드리프트는 **파일 이름과 함께** 말한다(2026-09-04 실측). 이전 문구는 `[sync] info:` 한 줄로
    #   "드리프트가 있다" 만 알렸고 **어느 파일인지 말하지 않았다**. 그래서 실제로 이런 일이 벌어졌다:
    #   `output/multi/requirements.txt` 를 고치고 배달했는데 스테이징을 안 해 **인덱스의 구버전이
    #   조용히 갔고**, rc=0 이라 성공으로 보였다. 가드는 울었지만 이름이 없어 사람이 자기 파일과
    #   연결하지 못했고, 로그를 FAIL/STOP 으로 좁혀 보는 습관이 그 한 줄을 잘라냈다.
    #   인덱스 권위 자체는 계약이므로 **차단하지 않는다** — 다만 무엇이 갈렸는지는 반드시 보인다.
    local _drift
    _drift="$(git -C "$CANONICAL_SRC" diff --name-only -- .claude CLAUDE.md .gitignore campaigns output/multi output/single 2>/dev/null)"
    if [ -n "$_drift" ]; then
        # 아래 영문 한 줄은 `verify_distribution` 의 `sub_transactional_source_uses_git_index` 가
        #   앵커로 쓴다 — 트랜잭션 소스가 인덱스 권위임을 코드가 스스로 말하는 자리다. 지우지 마라
        #   (2026-09-04: 파일명 출력을 더하면서 이 줄을 지웠다가 그 검사가 RED 로 잡았다).
        echo "[sync] info: canonical worktree drift detected; filesystem bytes are excluded in favor of index authority" >&2
        echo "[sync] ⚠ 워킹트리 드리프트 — 아래 파일은 **인덱스 버전이 배달된다**(git add 안 한 변경은 안 간다):" >&2
        printf '%s\n' "$_drift" | sed 's/^/[sync]     /' >&2
        echo "[sync]   → 방금 고친 파일이 이 목록에 있으면 'git add <파일>' 후 다시 실행하라." >&2
    fi
    # ★ `campaigns` 는 2026-09-06 추가(plan_26090616 ②). 이 목록에 없으면 추적 뼈대가 트랜잭션
    #   소스에 **아예 도착하지 않고**, 그 위에서 도는 render 는 소스 부재를 조용히 건너뛴다 —
    #   실측으로 서브 오버레이에 `campaigns/_template/**` 가 통째로 빠졌다. 새 추적 루트를
    #   만들면 이 경로 목록도 함께 갱신해야 한다(ALLOWLIST·MIRROR_DIRS 와 같은 계열의 표면).
    git -C "$CANONICAL_SRC" ls-files -z -- .claude CLAUDE.md .gitignore campaigns output/multi output/single \
        | git -C "$CANONICAL_SRC" checkout-index -z --stdin --prefix="$TRANSACTIONAL_SRC/"
    local topology render_input
    for topology in multi single; do
        for render_input in manifest.yaml a2a_signing/main_ed25519.pem sub_manifest.yaml; do
            [ -f "${CANONICAL_SRC}output/$topology/$render_input" ] || continue
            mkdir -p "$(dirname "$TRANSACTIONAL_SRC/output/$topology/$render_input")"
            install -m 0600 "${CANONICAL_SRC}output/$topology/$render_input" \
                "$TRANSACTIONAL_SRC/output/$topology/$render_input"
        done
    done
    SRC="$TRANSACTIONAL_SRC/"
    RENDER="$SRC.claude/skills/terraforming_node/scripts/render_sub_env.py"
    ROLE_CONTRACT="$SRC.claude/skills/terraforming_node/scripts/node_role_contract.py"
    PATCH_VALIDATOR="$SRC.claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
    PATCH_RESOLUTION="$SRC.claude/skills/upstream-version-watch/assets/current-production-resolution.json"
    REGEN_TOOL="$SRC.claude/skills/upstream-version-watch/scripts/regen_build_patches_src.py"
    echo "[sync] transactional source prepared (canonical tree remains read-only): $TRANSACTIONAL_SRC"
    # 2026-09-03(P3 · plan_26090317): 이 루프가 **두 토폴로지 전부**를 무조건 물질화해, 이번 배달과
    #   무관한 통로의 파생 payload 부재가 배달 전체를 죽였다(싱글 배달이 multi 사유로 STOP).
    #   물질화는 **이번 실행이 실제로 배달할 통로**에만 필요하다 — 그 밖은 판정 대상이 아니다.
    #   (부트스트랩이 multi 를 채우는 경우 BOOTSTRAP_POPULATE 가 TARGETS 밖의 multi 를 요구하는데,
    #    그 값은 이 시점 이후에 정해지므로 여기서는 TARGETS 에 더해 --branch both 의 양쪽을 본다.)
    for topology in "${TARGETS[@]}"; do
        materialize_source_port_payload "$topology" || return 9
    done
}

# ── 파생 payload 를 트랜잭션 소스로 편입 (2026-08-14 신설 · B-5) ───────────────────────────
# 인덱스 권위의 **두 번째 예외**다. 첫 번째(manifest.yaml)와 같은 기준을 통과해야 한다:
#   (a) 범위가 좁고 (b) 결정론이며 (c) **추적물이 판정 권위를 쥔다**.
#   여기서 (c)는 강하다 — payload 의 모든 바이트가 인덱스 스냅샷의 PROVENANCE.json 에 sha256 으로
#   못박혀 있으므로, 파일시스템이 드리프트하면 **조용히 통과할 수 없다**(fail-loud).
# 왜 굳이 트랜잭션 소스에 넣나: 그래야 build_remote_touch_inventory 가 108경로를 인벤토리에 담아
#   **롤백이 이 배달을 덮는다**. 번들만 따로 scp 하면 실패 시 서브에 반쯤 갈린 payload 가 남는다.
materialize_source_port_payload() {  # $1=topology → 0=ok(또는 해당없음), 9=실패
    local t="$1"
    local man="${SRC%/}/output/$t/$SOURCE_PORT_DIR/PROVENANCE.json"      # 인덱스 스냅샷 = 판정 권위
    local payload="${CANONICAL_SRC}output/$t/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD"   # 파일시스템 = 바이트
    local dest="${SRC%/}/output/$t/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD"
    [ -f "$man" ] || return 0                                            # 이식 변종 없음 → 해당없음
    if [ ! -f "$REGEN_TOOL" ]; then
        echo "[sync] FAIL(source-port/$t): 재생성기가 인덱스에 없다: $REGEN_TOOL" >&2
        echo "[sync]   → PROVENANCE.json 만 추적되고 생성엔진이 비추적이면 서브는 payload 를 만들 수 없다." >&2
        return 9
    fi
    if [ ! -d "$payload" ]; then
        echo "[sync] FAIL(source-port/$t): 파생 payload 부재 — $payload" >&2
        echo "[sync]   → 정상 차단이다(우회 금지). 아래로 좌표에서 재파생한 뒤 재실행하라:" >&2
        echo "[sync]     python3 .claude/skills/upstream-version-watch/scripts/regen_build_patches_src.py derive \\" >&2
        echo "[sync]       --root output/$t/$SOURCE_PORT_DIR --work <비추적 작업디렉터리>" >&2
        return 9
    fi
    # 인덱스의 PROVENANCE 로 파일시스템 payload 를 검증한다(권위/바이트 분리의 핵심).
    if ! python3 "$REGEN_TOOL" verify --root "${SRC%/}/output/$t/$SOURCE_PORT_DIR" \
            --provenance "$man" --files-root "$payload" >/dev/null; then
        echo "[sync] FAIL(source-port/$t): payload 가 인덱스 PROVENANCE 와 불일치 — 검증되지 않은 바이트는 배달하지 않는다." >&2
        python3 "$REGEN_TOOL" verify --root "${SRC%/}/output/$t/$SOURCE_PORT_DIR" \
            --provenance "$man" --files-root "$payload" 2>&1 | sed 's/^/    /' >&2 || true
        return 9
    fi
    mkdir -p "$(dirname "$dest")"
    cp -a "$payload" "$dest" || { echo "[sync] FAIL(source-port/$t): payload 복제 실패" >&2; return 9; }
    echo "[sync] [$t] source-port payload 편입: $(find "$dest" -type f | wc -l)파일(인덱스 PROVENANCE 검증 통과 · 롤백 인벤토리 대상)"
}

# 번들 발행(로컬) — 트랜잭션 소스의 검증된 payload 에서만 만든다. 같은 payload → 같은 바이트.
build_source_port_bundle() {  # $1=topology $2=출력경로 → SOURCE_PORT_BUNDLE_SHA 설정
    local t="$1" out="$2"
    SOURCE_PORT_BUNDLE_SHA=""
    SOURCE_PORT_BUNDLE_SHA="$(python3 "$REGEN_TOOL" bundle \
        --root "${SRC%/}/output/$t/$SOURCE_PORT_DIR" --out "$out" 2>/dev/null | tail -1)" || return 9
    [ -n "$SOURCE_PORT_BUNDLE_SHA" ] || { echo "[sync] FAIL(source-port/$t): 번들 해시 미획득" >&2; return 9; }
}

source_port_active() {  # $1=topology → 0=이식 변종 존재
    [ -d "${SRC%/}/output/$1/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD" ]
}

# 배달(apply) — scp 로 번들을 보내고 **서브에서** 재생성기를 stdin 으로 실행해 해체·검증한다.
#   서브에 도구를 상주시키지 않는다(오버레이는 recipe/benchmark 스킬만 렌더한다 — 그 계약을 건드리지 않는다).
deliver_source_port_payload() {  # $1=topology → 0=ok/해당없음
    local t="$1"
    source_port_active "$t" || return 0
    local local_bundle remote_dir remote_bundle rc=0
    remote_dir="${DEST}output/$t/$SOURCE_PORT_DIR"
    remote_bundle="$remote_dir/$SOURCE_PORT_BUNDLE_NAME"
    local_bundle="$(mktemp "${TMPDIR:-/tmp}/easy-vllm-source-port.XXXXXX.tar.gz")"
    build_source_port_bundle "$t" "$local_bundle" || { rm -f "$local_bundle"; return 9; }
    echo "[sync] [$t] source-port 번들 배달 — $(stat -c %s "$local_bundle") bytes sha256=$SOURCE_PORT_BUNDLE_SHA"
    if ! rsync -a -e "$SSH_OPTS" "$local_bundle" "$SUB_HOST:$remote_bundle"; then
        echo "[sync] FAIL(source-port/$t): 번들 전송 실패" >&2; rm -f "$local_bundle"; return 9
    fi
    rm -f "$local_bundle"
    # 해체·검증은 서브에서 수행한다 — 배달된 PROVENANCE.json(추적물)이 그쪽 판정 권위다.
    if ! $SSH_OPTS "$SUB_HOST" "cd '$SUB_WORK_DIR' && python3 - unbundle \
            --root 'output/$t/$SOURCE_PORT_DIR' --bundle '$remote_bundle' \
            --expect-sha256 '$SOURCE_PORT_BUNDLE_SHA'" <"$REGEN_TOOL"; then
        rc=9
        echo "[sync] FAIL(source-port/$t): 서브 해체·검증 실패 — 기존 payload 는 보존된다(교체는 검증 뒤에만)." >&2
    fi
    $SSH_OPTS "$SUB_HOST" "rm -f -- '$remote_bundle'" \
        || echo "[sync] WARNING(source-port/$t): 서브에 번들 잔존 — 이미지 COPY 오염 방지를 위해 수동 제거 필요: $remote_bundle" >&2
    return $rc
}

# 배달 후 독립 검증 — verify_checksums 와 같은 자리에서 "108/0/0" 을 사람에게 보인다.
verify_source_port_payload() {  # $1=topology
    local t="$1"
    source_port_active "$t" || return 0
    if $SSH_OPTS "$SUB_HOST" "cd '$SUB_WORK_DIR' && python3 - verify \
            --root 'output/$t/$SOURCE_PORT_DIR'" <"$REGEN_TOOL" | sed 's/^/  /'; then
        echo "  ✅ output/$t/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD (서브 무결성)"
        return 0
    fi
    echo "  ❌ output/$t/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD (서브 무결성 실패)" >&2
    return 1
}

preview_source_port_payload() {  # $1=topology (dry-run 미리보기)
    local t="$1" remote_dir n
    if ! source_port_active "$t"; then
        echo "    source-port payload: 해당없음(이식 변종 미등재)"; return 0
    fi
    remote_dir="${DEST}output/$t/$SOURCE_PORT_DIR"
    echo "    source-port payload(번들 평면 — rsync 와 별개):"
    echo "      로컬 검증분: $(find "${SRC%/}/output/$t/$SOURCE_PORT_DIR/$SOURCE_PORT_PAYLOAD" -type f | wc -l)파일(인덱스 PROVENANCE 대조 통과)"
    n="$($SSH_OPTS "$SUB_HOST" "[ -d '$remote_dir/$SOURCE_PORT_PAYLOAD' ] && find '$remote_dir/$SOURCE_PORT_PAYLOAD' -type f | wc -l || echo 0" 2>/dev/null || echo unknown)"
    echo "      서브 현재분: ${n}파일 → --apply 시 **전량 교체**(선언 밖 잔재는 제거된다 · 제거분은 로그로 남는다)"
}

# 빌드 콘텐츠 rsync(S4): 소스 = output/<t>/ 서브트리만(루트 Band1 구조적 배제) + Band2 keying. dry 면 --dry-run.
# --delete 이중 스코프(d-rsync-5): (a) dst=output/<t>/ 한정 → 서브 루트·.claude·docs 불가침(별 평면) ·
#   (b) 그 안에서도 exclude 된 Band3(configs/*·envs/* − allowlist)·중첩 dir 는 *보호*(삭제 대상 아님) → 서브 자작 트리플렛 보존
#   = full mirror 아님. stale main-origin Band3 잔재 회수는 gate③ cleanup 소관(루트 Band1 leak 과 동일 평면).
deliver_build() {  # $1=topology $2=dry(0/1)
    validate_runtime_patches "$1" || return 9
    _band2_filters || return 9
    local src="${SRC%/}/output/$1/" dst="${DEST}output/$1/"
    if [ "$2" = "1" ]; then
        rsync -az --delete --dry-run --itemize-changes "${FILT[@]}" -e "$SSH_OPTS" "$src" "$SUB_HOST:$dst"
        return
    fi
    # apply: (d-rsync-2) 삭제 前 brake — dry-run 으로 삭제예정 세고 ALLOW_DELETE 초과 시 *삭제 前* fail-closed(부분삭제 0)
    sub_run "mkdir -p 'output/$1'"
    # ★ 브레이크 **앞에서** 빈 디렉터리를 정비한다(2026-08-14 · plan_26081409 A). 뒤에 두면
    #   정보량 0 인 정리를 위해 운영자가 ALLOW_DELETE 를 줘야 하고, 그 플래그는 **파일 삭제까지 함께**
    #   뚫는다 — 손실 0 인 작업을 손실 위험이 있는 게이트로 통과시키는 셈이다(2026-08-14 실제 발생).
    #   여기서 미리 치우면 rsync 의 삭제예정이 0 이 되어 브레이크가 정상 통과한다.
    prune_remote_empty_dirs "$1" || return 9
    local ndel dry_out
    if ! dry_out="$(rsync -az --delete --dry-run --itemize-changes "${FILT[@]}" -e "$SSH_OPTS" "$src" "$SUB_HOST:$dst" 2>&1)"; then
        echo "[sync] FAIL: deletion brake dry-run failed before apply" >&2
        return 9
    fi
    ndel="$(printf '%s\n' "$dry_out" | awk '/^\*deleting/{n++} END{print n+0}')"
    # ★ 브랜치 불일치 감지 — ALLOW_DELETE 로 뚫을 수 없는 별개 게이트.
    #   `output/<t>/` 빌드킷은 **해당 토폴로지 브랜치에서만** 추적된다(single-node 인덱스엔
    #   output/multi/.gitkeep 하나뿐). 그래서 single-node 체크아웃으로 `--branch multi` 를 배달하면
    #   SRC 에 multi 빌드킷이 없고, rsync --delete 가 서브의 정상 빌드킷을 **지우려 든다**.
    #   삭제brake 는 개수만 말하므로, 운영자가 안내대로 ALLOW_DELETE=<n> 을 주면 그대로 파괴된다
    #   (2026-08-01 실제로 8건이 이 상태로 잡혔다 — Dockerfile·docker-compose·build_patches 4종).
    #   브랜치 이름으로 토폴로지를 추론하지 않는다(헌법) — **증거로** 판정한다:
    #   "정본이 이 토폴로지의 빌드킷을 갖고 있지 않은데 목적지는 갖고 있다" = 정합 실패.
    if [ "${ndel:-0}" -gt 0 ]; then
        local bk missing_bk=0
        for bk in Dockerfile docker-compose.yaml; do
            [ -f "${src%/}/$bk" ] || missing_bk=1
        done
        if [ "$missing_bk" = "1" ] \
           && printf '%s\n' "$dry_out" | grep -qE '^\*deleting +(Dockerfile|docker-compose\.yaml|build_patches/)'; then
            echo "[sync] STOP(브랜치 불일치): 정본 체크아웃에 output/$1 빌드킷이 없는데 서브에는 있다." >&2
            echo "[sync]   → 이 삭제는 정리가 아니라 **잘못된 브랜치에서의 배달**이다. ALLOW_DELETE 로 뚫지 마라." >&2
            echo "[sync]   → output/$1 빌드킷을 추적하는 브랜치로 체크아웃한 뒤 재실행하라(현재: $(git -C "$CANONICAL_SRC" rev-parse --abbrev-ref HEAD 2>/dev/null))." >&2
            return 9
        fi
    fi
    if [ "${ndel:-0}" -gt "${ALLOW_DELETE:-0}" ]; then
        echo "[sync] STOP(S4 삭제brake): $1 삭제예정 ${ndel}건 > ALLOW_DELETE=${ALLOW_DELETE:-0} — 삭제 前 fail-closed(부분삭제 없음). 의도된 정리면 ALLOW_DELETE=${ndel} 로 재실행." >&2
        return 9
    fi
    rsync -az --delete --max-delete="${ALLOW_DELETE:-0}" "${FILT[@]}" -e "$SSH_OPTS" "$src" "$SUB_HOST:$dst"
}
# dry-run 빌드 미리보기 — 삭제(--delete) 라인을 head 절단 **이전에 전량 보장 노출**(HITL 게이트 ③ '삭제 0' 신뢰성, review major).
preview_build() {  # $1=topology
    local out ndel
    out="$(deliver_build "$1" 1)"
    ndel="$(printf '%s\n' "$out" | grep -c '^\*deleting' || true)"
    echo "    ⚠ 삭제 예정(--delete): ${ndel}건  (0이어야 정상 — apply 는 ALLOW_DELETE=${ALLOW_DELETE:-0} 초과 시 *삭제 前* fail-closed. 의도된 정리면 ALLOW_DELETE=${ndel})"
    [ "${ndel:-0}" -gt 0 ] && { printf '%s\n' "$out" | grep '^\*deleting' | sed 's/^/      DEL /' || true; }
    echo "    전송/생성 미리보기(최대 40줄):"
    printf '%s\n' "$out" | grep -v '^\*deleting' | sed 's/^/      /' | preview_lines || true
}
# 에이전트환경 오버레이 rsync(가산 — --delete 없음: 서브 자작 .claude 산출물·세션상태 보호가 목적).
# 트레이드오프(review nit): 메인이 런타임블럭에서 파일을 '제거'하면 서브에 stale 잔존 가능(동명 파일은 덮어씀 → 흔치 않음).
#   런타임블럭 정리가 필요하면 별도 정합(scoped --delete)으로 다룬다 — 기본은 안전한 가산.
deliver_overlay() {  # $1=topology $2=dry
    local st; st="$(staging_dir "$1")"
    [ -d "$st" ] || { echo "[sync] (info) 스테이징 없음($st) — render 선행 필요"; return 0; }
    local dry=(); [ "$2" = "1" ] && dry=(--dry-run --itemize-changes)
    rsync -az "${dry[@]}" "${OVERLAY_EXCLUDES[@]}" -e "$SSH_OPTS" "$st/" "$SUB_HOST:$DEST"
    if [ "$2" = "1" ]; then
        local stale
        for stale in "${OVERLAY_STALE_PATHS[@]}"; do
            sub_run "[ ! -e '$stale' ] || printf '*deleting %s\\n' '$stale'"
        done
    fi
}

# Apply exact tombstones only after replacement checksums and modes have passed.  This function is
# deliberately separate from additive rsync so a failed replacement can never delete the fallback.
apply_overlay_tombstones() {
    local stale
    for stale in "${OVERLAY_STALE_PATHS[@]}"; do
        sub_run "rm -f -- '$stale'"
    done
}

# 배달 표면 **수렴 리포트**(2026-09-05 · 사용자 요구: "자주 바뀌는 서브가 과거 찌꺼기 없이 계속
# 안정화되느냐"). 오버레이는 가산 배달이라 **정본에서 사라진 파일은 서브에 그대로 남는다** —
# 비석(OVERLAY_STALE_PATHS)은 *알려진* 은퇴만 지우므로, 알려지지 않은 잔재는 아무도 보지 못했다.
# 여기서 하는 일은 삭제가 아니라 **보이게 하는 것**이다: 정본이 배달하는 디렉터리 안에서 서브에만
# 있는 파일을 세어 목록으로 낸다. 지우는 것은 여전히 명시 비석의 몫이다(자동 삭제 ✗ — 서브의
# 정당한 로컬 산출물과 잔재를 기계가 가를 수 없다).
report_overlay_convergence() {   # $1=topology → 항상 0(정보 리포트 · 게이트 아님)
    local st; st="$(staging_dir "$1")"
    [ -d "$st" ] || return 0
    local canon dirs sub_list extra n
    canon="$(cd "$st" && find . -type f -not -path '*/__pycache__/*' -not -name '*.pyc' \
             -printf '%P\n' | LC_ALL=C sort)"
    [ -n "$canon" ] || return 0
    # 비교 범위 = 정본이 **파일을 두는 그 디렉터리**뿐이며 재귀하지 않는다(-maxdepth 1).
    #   2026-09-05 첫 실행 교정: 최상위(`docs` 등)로 잡았더니 서브가 만든 블랙박스 로그
    #   (`docs/logs/sub/**` 8건)가 "정본에 없음" 으로 잡혔다 — 그건 잔재가 아니라 **그 노드의
    #   산출물**이다. 리포트가 소음을 내면 사람은 리포트를 안 보게 되고, 그러면 진짜 잔재도 못 본다.
    dirs="$(printf '%s\n' "$canon" | sed 's|/[^/]*$||' | grep -v '^\.$' | LC_ALL=C sort -u | tr '\n' ' ')"
    [ -n "$dirs" ] || return 0
    sub_list="$(sub_run "find $dirs -maxdepth 1 -type f -not -path '*/__pycache__/*' -not -name '*.pyc' 2>/dev/null | sed 's|^\./||' | LC_ALL=C sort" || true)"
    [ -n "$sub_list" ] || return 0
    extra="$(LC_ALL=C comm -13 <(printf '%s\n' "$canon") <(printf '%s\n' "$sub_list") || true)"
    # ── 서브가 **계약상 소유**하는 평면은 잔재가 아니다 (2026-09-05 · 캠페인 1 실측) ──
    #   ① campaigns/<camp-id>/** — 캠페인 인스턴스·릴레이 원장. 서브의 쓰기 권한 평면이며 메인이
    #      배달하지 않는다(뼈대 campaigns/_template 만 배달한다 · 2026-09-06 옛 tasks/ 에서 이관).
    #   ② 빌드킷 4종 — 배달 평면이 **dormant** 인 토폴로지(싱글)에서는 서브가 자율 저작한다
    #      (헌법 불변식 A). dormant 판정은 별도 변수가 아니라 **정본이 그 디렉터리에 빌드킷을
    #      하나도 두지 않았다**는 사실로 읽는다 — 배달했으면 canon 에 있고, 그러면 잔재 판정이
    #      정상 작동한다.
    #   이 제외가 없으면 싱글에서 "정본 밖 0건" 이 **정의상 달성 불가**가 된다(서브가 계약대로
    #   저작할수록 리포트가 커진다). 달성 불가한 기준은 사람이 보지 않게 되고, 그러면 진짜
    #   잔재도 못 본다 — docs.md §PII 판정 대상 축소가 같은 근거로 내린 결정이다.
    #   목록은 닫혀 있다(antipattern-ok: hardcoding — tripwire: 늘리려면 이 주석을 읽고 리뷰하라).
    if [ -n "$extra" ]; then
        local _kit_re='(Dockerfile|Dockerfile\.source-build|docker-compose\.yaml|requirements\.txt)'
        local _keep="" _f _dir
        while IFS= read -r _f; do
            [ -n "$_f" ] || continue
            # 서브 소유 평면(닫힌 목록):
            #   campaigns/ = 캠페인 인스턴스·릴레이 원장(서브 쓰기 권한 평면 — 뼈대만 배달)
            #   docs/   = 서브의 저작·데이터 평면. 상향 회수가 **문서기반 only** 라는 계약
            #             자체가 "서브는 자기 docs/ 에 쓴다"를 요구한다(헌법 §서브 docs 계약).
            #             메인이 여기 두는 것은 `example.md` 스켈레톤뿐이고 그건 canon 에 있어
            #             애초에 잔재로 잡히지 않는다. 은퇴한 스켈레톤은 비석이 담당한다.
            case "$_f" in campaigns/_template/*) ;; campaigns/*|docs/*) continue ;; esac
            # output/<t>/.env = render --materialize-env 산출물. 배달 평면이 dormant 면 서브가 만든다.
            case "$_f" in output/*/.env) [ -n "$(printf '%s\n' "$canon" | grep -x "$_f" || true)" ] || continue ;; esac
            _dir="${_f%/*}"; [ "$_dir" = "$_f" ] && _dir="."
            if printf '%s' "${_f##*/}" | grep -qE "^${_kit_re}$" \
               && ! printf '%s\n' "$canon" | grep -qE "^${_dir}/${_kit_re}$"; then
                continue   # dormant 평면의 서브 자율 저작분
            fi
            _keep="${_keep}${_f}\n"
        done <<< "$extra"
        extra="$(printf "$_keep" | grep -v '^$' || true)"
    fi
    # 노드-로컬 상태(메인 .gitignore 가 선언적으로 비추적인 경로)는 잔재가 아니다 — 파생 판정.
    #   2026-09-10 실측 위양성 3건: `skills/*/config.yaml` ×2 · `lockset.json`. 셋 다 추적
    #   스켈레톤(`config.example.yaml`)의 **로컬 인스턴스**이고 메인에도 같은 자리에 비추적으로 있다.
    #   위 닫힌 목록(campaigns/·docs/·빌드킷)은 유지한다 — 이 필터는 그것을 대체하지 않고 보탠다.
    extra="$(printf '%s\n' "$extra" | grep -v '^$' | drop_node_local_paths || true)"
    n="$(printf '%s' "$extra" | grep -c '' || true)"
    if [ "${n:-0}" -eq 0 ]; then
        # 성공 줄에 디렉터리 목록을 다 뿌리면 화면이 목록으로 덮여 정작 다른 판정이 안 보인다.
        # 세 개수만 말한다(무엇을 봤는지는 실패했을 때 목록으로 나온다).
        local _nd; _nd="$(printf '%s\n' "$dirs" | tr ' ' '\n' | grep -c '.' || true)"
        echo "  ✅ 수렴: 배달 표면 ${_nd}개 디렉터리에 정본 밖 파일 0건 — 서브가 과거 찌꺼기 없이 정본과 같다"
        return 0
    fi
    echo "  ⚠ 수렴 리포트: 정본에 없는 파일 ${n}건이 서브의 배달 표면에 있다(삭제하지 않는다 — 눈에 보이게만 한다):"
    printf '%s\n' "$extra" | sed 's/^/      /' | preview_lines
    echo "      → 은퇴가 확정된 경로는 OVERLAY_STALE_PATHS 비석에 등재하라(명시 삭제만 허용)."
    return 0
}

# 런타임블럭 **잔재 리포트**(2026-09-10 신설 · plan_26091019_2).
#
# 왜 위 수렴 리포트로 부족한가: 그쪽은 `-maxdepth 1` 로 **정본이 파일을 둔 디렉터리**만 본다.
#   그런데 토폴로지 게이팅(tool_plane)이나 RUNTIME_BLOCK_EXCLUDES 로 정본이 **그 루트에 파일을
#   하나도 두지 않게 되면**, 그 루트는 `dirs` 에 아예 오르지 못해 잔재가 **구조적으로 보이지 않는다**.
#   2026-09-10 실측: 멀티 서브에 `.claude/skills` 196 · `.claude/policies` 18 파일이 옛 배달에서
#   남아 있었고(정본 0건), 그중 7종은 메인 전용 오케스트레이션(sync_to_sub·sync_branches·
#   fetch_sub_docs·smoke_clone·multinode_*)이라 §2.7.1 권한 평면 위반이었다. 오버레이는 **가산**
#   이므로 제외표를 추가해도 **이미 간 것은 돌아오지 않는다** — 비석이 짝으로 필요하다.
#   이 잔재가 retirement 감사를 계속 FAIL 시킨 실제 원인이었다.
#
# 두 루트는 **주로** 메인이 배달하는 코드 평면이므로 "정본에 없으면 눈에 띄어야 한다" 가 참이다.
#   다만 전량이 잔재는 아니다 — 서브의 스킬이 자기 실행 중 여기에 쓰는 상태가 있다(2026-09-10 실측:
#   `vllm-recipe-explorer/feedback/*`). 그래서 이 리포트는 **분류를 요구하는 목록**이지 삭제 목록이
#   아니다. 닫힌 목록이며 tripwire 다 — 늘리려면 이 주석을 읽고 그 루트의 저작 주체부터 확인하라.
# ⚠ 삭제하지 않는다. `정본 0건` 은 **dormant(보내지 않기로 함)** 와 **렌더 실패** 를 구분하지 못한다 —
#   여기서 지우면 렌더가 한 번 비는 순간 서브가 통째로 비워진다. 삭제는 토폴로지-aware 비석의 몫이다.
RUNTIME_BLOCK_OWNED_ROOTS=(.claude/skills .claude/policies)
# 잔재 판정에서 **노드-로컬 상태**를 걷어낸다(2026-09-10 신설 · 첫 실행이 위양성을 냈다).
#
# 판정은 파생이다 — 손목록을 두지 않는다:
#   · 메인 `.gitignore` 가 **선언적으로 비추적**인 경로 → 노드-로컬 상태다. 메인에도 그 노드의
#     사본이 있고(실측: config.yaml·lockset.json·feedback/* 전부 메인에도 비추적으로 실재),
#     애초에 index 권위 배달의 대상이 아니었으므로 "정본이 보내다 말았다" 가 성립하지 않는다.
#   · 메인이 **추적하는데** 정본이 이 토폴로지에 안 보내는 경로 → 진성 잔재(옛 배달의 찌꺼기).
# 이 구분이 없으면 "정본 밖 0건" 이 **정의상 달성 불가**가 된다 — 서브가 계약대로 일할수록
# 리포트가 커지고, 사람은 그 리포트를 안 보게 되며, 그러면 진짜 잔재도 못 본다
# (report_overlay_convergence 가 같은 근거로 이미 편 논리 · docs.md §PII 판정 대상 축소 선례).
drop_node_local_paths() {   # stdin=경로 목록 → stdout=노드-로컬을 걷어낸 목록
    local all local_only
    all="$(cat)"
    [ -n "$all" ] || return 0
    # check-ignore 는 **추적물을 무시로 보고하지 않는다** — 그래서 진성 잔재(추적물)는 남고
    # 선언적 비추적만 걸러진다. 정확히 원하는 판별이다.
    local_only="$(printf '%s\n' "$all" | git -C "${SRC%/}" check-ignore --stdin 2>/dev/null | LC_ALL=C sort || true)"
    [ -n "$local_only" ] || { printf '%s\n' "$all"; return 0; }
    LC_ALL=C comm -23 <(printf '%s\n' "$all" | LC_ALL=C sort) <(printf '%s\n' "$local_only") || true
}

report_runtime_block_residue() {   # $1=topology → 항상 0(정보 리포트 · 게이트 아님)
    local st; st="$(staging_dir "$1")"
    [ -d "$st" ] || return 0
    local root canon sub_list extra n total=0
    for root in "${RUNTIME_BLOCK_OWNED_ROOTS[@]}"; do
        canon="$(cd "$st" && find "$root" -type f -not -path '*/__pycache__/*' -not -name '*.pyc' 2>/dev/null | LC_ALL=C sort || true)"
        sub_list="$(sub_run "find '$root' -type f -not -path '*/__pycache__/*' -not -name '*.pyc' 2>/dev/null | sed 's|^\./||' | LC_ALL=C sort" || true)"
        [ -n "$sub_list" ] || continue
        extra="$(LC_ALL=C comm -13 <(printf '%s\n' "$canon" | grep -v '^$') <(printf '%s\n' "$sub_list") || true)"
        extra="$(printf '%s\n' "$extra" | grep -v '^$' | drop_node_local_paths || true)"
        n="$(printf '%s' "$extra" | grep -c '' || true)"
        [ "${n:-0}" -gt 0 ] || continue
        total=$((total + n))
        echo "  ⚠ 런타임블럭 잔재: $root 에 정본 밖 파일 ${n}건 — 서브 전용 잔존(삭제하지 않는다):"
        printf '%s\n' "$extra" | sed 's/^/      /' | preview_lines
    done
    if [ "$total" -eq 0 ]; then
        echo "  ✅ 런타임블럭 잔재 0건 — 메인 소유 코드 평면이 정본과 같다"
    else
        echo "      → 분류하라: (a) 옛 배달의 잔재면 토폴로지-aware 비석으로 은퇴 — 메인 전용"
        echo "         오케스트레이션이 서브에 남으면 배달 방향이 뒤집힌다(§2.7.1). (b) 서브 스킬이"
        echo "         저작하는 런타임 상태면 그대로 둔다(잔재가 아니다)."
    fi
    return 0
}

# 활성 표면 = **정본이 배달하는 파일**이다(스테이징 트리에서 파생 — verify_checksums 와 같은 원천).
#
# 2026-09-10 교정(plan_26091019_2): 종전 스캐너는 서브의 `.claude` 를 통째로 걸어 **배달된 적 없는
#   잔재**까지 읽었다. 그래서 은퇴를 *집행하는* 선언(OVERLAY_STALE_PATHS·SUB_TOMBSTONES·
#   RUNTIME_BLOCK_EXCLUDES)과 그 `.pyc` 를 "활성 소비자" 로 셌다 — 은퇴 대상을 **이름으로 부르지
#   않고는 은퇴시킬 수 없으므로** 그 게이트는 구조적으로 열릴 수 없었다(역-오라클).
#   실측 2026-09-10: 히트 10건 전원이 선언·낡은 주석·`.pyc` 였고 호출부는 0건이었으며, 정작 은퇴
#   대상 2종(scripts/smoke_clone.sh·scripts/sync_branches.sh)은 서브에서 **이미 부재**였다.
#   ∴ 판정 범위를 "서브에 있는 모든 것" 이 아니라 "정본이 유지하는 것" 으로 좁힌다 — FAIL 메시지가
#   원래 말하던 *active sub surfaces* 가 그 뜻이다. 잔재는 활성 표면이 아니라 **수렴 대상**이고
#   그 처방은 삭제(scoped --delete)이지 배달 차단이 아니다(deliver_overlay 헤더 주석이 이미 지목).
#   `owner` 면제는 살아 있다 — `.claude/rules/workflow.md` 등 정본 산문이 정규 경로를 인용한다.
verify_destination_retirement_consumers() {  # $1=topology — 활성 표면의 도출원(스테이징 트리)
    local topology="$1" stale hits fail=0 scan_py scan_q stale_q st canon nsurf out scanned
    st="$(staging_dir "$topology")"
    [ -d "$st" ] || { echo "[sync] FAIL(retirement consumer): 스테이징 부재($st) — 활성 표면을 도출할 수 없다" >&2; return 98; }
    canon="$(cd "$st" && find . -type f -not -path '*/__pycache__/*' -not -name '*.pyc' -printf '%P\n' | LC_ALL=C sort)"
    nsurf="$(printf '%s\n' "$canon" | grep -c . || true)"
    # 목록이 비면 "소비자 0" 이 아니라 **도출 실패**다 — 둘을 뭉개면 게이트가 조용히 통과한다.
    [ "${nsurf:-0}" -gt 0 ] || { echo "[sync] FAIL(retirement consumer): 정본 파일 0건($st) — 활성 표면 도출 실패" >&2; return 98; }
    scan_py=$'# retirement_consumer_scan\nimport os,re,sys\nstale=sys.argv[1]\nowner=".claude/skills/upstream-version-watch/"+stale\nchars=set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_./-")\nneedle=re.compile(r"(?<![A-Za-z0-9_.-])"+re.escape(stale)+r"(?![A-Za-z0-9_./-])")\nhits=[]\ndef scan(path):\n if os.path.islink(path): raise RuntimeError("active scanner refuses symlink: "+path)\n data=open(path,"rb").read().decode("utf-8","replace")\n for lineno,line in enumerate(data.splitlines(),1):\n  for match in needle.finditer(line):\n   lo,hi=match.start(),match.end()\n   while lo and line[lo-1] in chars: lo-=1\n   while hi<len(line) and line[hi] in chars: hi+=1\n   token=line[lo:hi]\n   while token.startswith("./"): token=token[2:]\n   if token!=owner: hits.append(f"{path}:{lineno}:{line}")\nsurfaces=[x for x in sys.stdin.read().splitlines() if x.strip()]\nif not surfaces: raise SystemExit("active surface list empty")\nn=0\nfor rel in surfaces:\n if os.path.isfile(rel):\n  n+=1\n  scan(rel)\nprint(n)\nprint("\\n".join(hits),end="")'
    printf -v scan_q '%q' "$scan_py"
    for stale in "${OVERLAY_RETIREMENT_STALE_PATHS[@]}"; do
        printf -v stale_q '%q' "$stale"
        if out="$(printf '%s\n' "$canon" | sub_run "python3 -c $scan_q $stale_q")"; then
            :
        else
            echo "[sync] FAIL(retirement consumer): scanner/transport failed for $stale" >&2
            return 98
        fi
        scanned="$(printf '%s\n' "$out" | head -1)"
        hits="$(printf '%s\n' "$out" | tail -n +2)"
        if [ -n "$hits" ]; then
            echo "[sync] FAIL(retirement consumer): $stale is still referenced on active sub surfaces:" >&2
            printf '%s\n' "$hits" | sed 's/^/    /' >&2
            fail=1
        else
            echo "  ✅ retirement consumer audit: $stale has no active sub consumer (활성 표면 ${scanned}/${nsurf} 파일 검사)"
        fi
    done
    return $fail
}
# 체크섬 검증(빌드킷 + 오버레이 배달 표면 전수). 불일치 시 비-0.
# ★ 목록을 손저작하지 않는다 (2026-09-03). 이전 판은 21개 경로를 이 함수 안에 손으로 적어 두었고,
#   그 목록은 **배달 표면이 늘어나도 스스로 늘지 않는다** — 새 파일이 배달되면 검증 없이 통과하고
#   사람은 "체크섬 통과"를 보고 전수 검증으로 오해한다(침묵 누락). 그래서 두 목록 모두 **배달을
#   실제로 결정하는 원천**에서 파생한다:
#     (1) 빌드킷  = ${BAND2_TOP[@]}      — deliver_build 의 rsync allowlist 그 자체(300행대 _band2_filters)
#     (2) 오버레이 = 스테이징 트리       — deliver_overlay 가 `$st/` 를 통째로 미는 대상(render_sub_env 산출물)
#                                          에서 OVERLAY_EXCLUDES(__pycache__·*.pyc)만 뺀 것
#   근거 규율: workflow.md §결정론 규율 "단일 소유가 불가능하면 교차검증이 차선" — 여기서는 단일 소유가
#   가능하므로 교차검증(assert_band2_top_gitignore_parity)이 아니라 파생을 쓴다. 함수명·호출 위치는
#   그대로 둔다(verify_distribution 의 순서체크 2건이 `verify_checksums ` 토큰을 핀한다).
verify_checksums() {  # $1=topology  $2(선택)=skip_buildkit(1이면 빌드킷 대조 생략)
    local st; st="$(staging_dir "$1")"; local fail=0 L R f rels line ok_n all_n
    # (1) 빌드킷 — 원천 = rsync allowlist. 부재 항목은 건너뛴다(토폴로지별 선택 자산).
    # 2026-09-03(P3 · plan_26090317): 빌드킷 평면이 dormant 라 **보내지 않은 것**을 여기서 대조하면
    #   전건 불일치로 배달이 죽는다(오버레이는 129/129 일치인데도). 검증 범위는 배달 범위를 따른다 —
    #   "안 보냈다" 와 "보냈는데 틀렸다" 는 다른 사실이고, 후자만 실패다.
    if [ "${2:-0}" = "1" ]; then
        echo "  ⏭  빌드킷 대조 생략(평면 dormant — 이 배달의 범위 밖)"
    else
    for f in "${BAND2_TOP[@]}"; do
        [ -f "${SRC%/}/output/$1/$f" ] || continue
        L=$(md5sum "${SRC%/}/output/$1/$f" | awk '{print $1}')
        R=$($SSH_OPTS "$SUB_HOST" "md5sum '${SUB_WORK_DIR}/output/$1/$f' 2>/dev/null" | awk '{print $1}')
        [ -n "$L" ] && [ "$L" = "$R" ] && echo "  ✅ output/$1/$f" || { echo "  ❌ output/$1/$f: main=$L sub=$R"; fail=1; }
    done
    fi
    # (2) 오버레이 — 원천 = deliver_overlay 가 미는 스테이징 트리 전수.
    if [ ! -d "$st" ]; then
        echo "  ❌ 오버레이 스테이징 부재: $st — render 선행 필요" >&2
        return 1
    fi
    rels="$(cd "$st" && find . -type f -not -path '*/__pycache__/*' -not -name '*.pyc' -printf '%P\n' | LC_ALL=C sort)"
    if [ -z "$rels" ]; then
        echo "  ❌ 배달 표면이 비었다($st) — 0건 검사를 통과로 읽지 않는다" >&2
        return 1
    fi
    # 계약 대표 — 파생 목록이 조용히 줄어드는 것을 막는다.
    #   · .claude/skills/vllm-recipe-explorer/recipe.py = 런타임블럭이 실제로 복제됐다는 증거
    #   · .claude/a2a_delegation.json               = A2A 위임키(hw_verified:true 일 때만 발급)
    #
    # ⚠ 런타임블럭 대표는 **tool_plane 이 비어 있지 않을 때만** 요구한다(2026-09-03 P5 실측).
    #   결함의 형태: P2 에서 `tool_plane` 게이팅을 도입해 **ray-worker 에는 런타임 스킬을 0종 배달**
    #   하도록 렌더러를 고쳤는데, 이 검증기는 옛 가정("무조건 렌더")을 그대로 들고 있었다. single
    #   (a2a-agent=3종)에서는 단언이 참이라 드러나지 않았고, **멀티 배달이 처음 실행된 순간**
    #   `❌ 배달 표면에 런타임블럭 대표가 없다` 로 죽었다(롤백은 정상 작동). 교정이 만든 결함이 아니라
    #   교정의 배선이 한 곳 덜 간 것이다 — 계약을 바꾸면 그 계약을 읽는 **모든** 자리를 따라가야 한다.
    #   판정 정본은 계약 판정기의 tool_plane 이다(토폴로지로 추론하지 않는다 · 경로는 파일 상단
    #   ROLE_CONTRACT 상수 — 이 함수 본문은 판정기 경로 문자열을 갖지 않는다).
    local _tp _tp_n
    # 판정기 부재/미해소는 "tool_plane 이 비었다"와 **다른 사실**이다. 먼저 갈라야 한다 —
    #   갈라 두지 않으면 배선 결함이 정상 계약(ray-worker 0종)으로 위장한다(2026-09-05 실측).
    if [ ! -f "${ROLE_CONTRACT:-}" ]; then
        # 뒤따르는 검사(위임키 회수·host-safety 모드·retirement 감사)는 계속 돌려야 한다 —
        #   한 검사의 배선 결함이 나머지 검사를 침묵시키면 결함 하나가 여러 개를 가린다.
        echo "  ❌ 계약 판정기 경로 미해소(ROLE_CONTRACT='${ROLE_CONTRACT:-}') — 런타임블럭 대표 요구 여부를 정할 수 없다(fail-closed)" >&2
        fail=1
        _tp="__UNRESOLVED__"
    else
    _tp="$(python3 "$ROLE_CONTRACT" evaluate \
             --manifest "${SRC%/}/output/$1/manifest.yaml" --topology "$1" \
             --field tool_plane --format value 2>/dev/null || echo '__UNRESOLVED__')"
    fi
    if [ "$_tp" = "__UNRESOLVED__" ]; then
        echo "  ❌ tool_plane 미해소 — 런타임블럭 대표 요구 여부를 정할 수 없다(fail-closed)" >&2; fail=1
    else
        _tp_n="$(printf '%s' "$_tp" | tr -cd '[:alnum:]-' | wc -c)"
        if [ "${_tp_n:-0}" -eq 0 ]; then
            echo "  ⏭  런타임블럭 대표 검사 생략 — tool_plane 이 비었다(ray-worker: 스킬 0종이 계약)"
        else
            printf '%s\n' "$rels" | grep -qxF '.claude/skills/vllm-recipe-explorer/recipe.py' \
                || { echo "  ❌ 배달 표면에 런타임블럭 대표(.claude/skills/vllm-recipe-explorer/recipe.py)가 없다" >&2; fail=1; }
        fi
    fi
    # 2026-09-03(S4/㉕ · plan_26090317 P1): 여기서 하던 일은 **정보 한 줄**이었다 — 스테이징에 키가
    #   없으면 "미발급" 이라고만 말하고, **서브에 있으면 안 되는 키가 남아 있는지는 보지 않았다.**
    #   회수 경로가 3중으로 없었기 때문에 그 상태는 영구였다: ① deliver_overlay 는 `--delete` 없는
    #   가산 rsync ② 이 검사가 잉여 키를 안 봄 ③ gitignore.template 에 무시 규칙이 없어 서브 git 이
    #   키를 추적 → 손으로 지워도 `sub.git.unstick`(git checkout --)이 되살린다.
    #   `policy:A2A_IDENTITY_PROOF_FAIL_CLOSED`(옛 …DELEGATION_KEY…) 는 **발급 방향으로만** fail-closed 였고 회수 방향은
    #   fail-open 이었다. 오진 정정·서브 교체·HW 변경 뒤에도 서브는 계속 위임 자격을 들고 있었다.
    # 2026-09-05(③ 3-9 · G-E1): 위임 키는 **폐기됐다** — 렌더가 더는 만들지 않는다. 그러므로
    #   스테이징에 있으면 그것이 오히려 결함이고(옛 렌더러가 되살아났다), 서브에 있으면 잔재다.
    #   가산 배달만으로는 잔재가 영원히 남는다 — 사용자가 지목한 "과거 찌꺼기" 의 정확한 사례다.
    if [ -f "$st/.claude/a2a_delegation.json" ]; then
        echo "[sync] FAIL(S4): 스테이징에 폐기된 위임 키가 있다 — 렌더러가 되살아났다(G-E1)" >&2
        fail=1
    else
        if sub_run "[ -f '.claude/a2a_delegation.json' ]" 2>/dev/null; then
            echo "  ⚠ 서브에 폐기된 위임 키 잔재 발견 — 회수한다(자격증명은 이제 서명된 카드다)."
            sub_run "rm -f -- '.claude/a2a_delegation.json'" \
                || { echo "[sync] FAIL(S4): 위임 키 회수 실패 — 자격이 남은 채로 배달하지 않는다" >&2; return 9; }
            if sub_run "[ -f '.claude/a2a_delegation.json' ]" 2>/dev/null; then
                echo "[sync] FAIL(S4): 회수 후에도 위임 키가 남아 있다(서브 git 이 되살렸을 수 있다)" >&2; return 9
            fi
            echo "  ✅ 위임 키 회수 완료"
        fi
    fi
    # 원격 md5 는 SSH 1회로 몰아 받는다(파일당 1회는 수십배 느리다).
    local -A RSUM=()
    while IFS= read -r line; do
        [ -n "$line" ] || continue
        RSUM["${line#*  }"]="${line%% *}"
    done < <(printf '%s\n' "$rels" | $SSH_OPTS "$SUB_HOST" \
        "cd '$SUB_WORK_DIR' && while IFS= read -r _p; do if [ -f \"\$_p\" ]; then md5sum -- \"\$_p\"; fi; done" 2>/dev/null)
    ok_n=0; all_n=0
    while IFS= read -r f; do
        [ -n "$f" ] || continue
        all_n=$((all_n + 1))
        L=$(md5sum -- "$st/$f" | awk '{print $1}'); R="${RSUM[$f]-}"
        if [ -n "$L" ] && [ "$L" = "$R" ]; then ok_n=$((ok_n + 1))
        else echo "  ❌ $f: main=$L sub=${R:-missing}"; fail=1; fi
    done < <(printf '%s\n' "$rels")
    [ "$ok_n" = "$all_n" ] && echo "  ✅ 오버레이 배달 표면 ${ok_n}/${all_n} 일치" \
        || echo "  ❌ 오버레이 배달 표면 ${ok_n}/${all_n} 만 일치"
    return $fail
}

verify_destination_host_safety_modes() {
    local fail=0 f expected mode
    for f in \
        .claude/runtime/host_safety/mem_watchdog.sh \
        .claude/runtime/host_safety/install_host_safety.sh \
        .claude/runtime/host_safety/install_netconsole.sh \
        .claude/runtime/host_safety/host/vllm-drop-caches.sh \
        .claude/runtime/host_safety/systemd/easy-vllm-memwatch.service; do
        case "$f" in *.service) expected=644;; *) expected=755;; esac
        mode="$($SSH_OPTS "$SUB_HOST" "stat -c '%a' '$SUB_WORK_DIR/$f' 2>/dev/null" || true)"
        if [ "$mode" != "$expected" ]; then
            echo "  ❌ destination host-safety mode $f=${mode:-missing}, expected=$expected" >&2
            fail=1
        else
            echo "  ✅ destination host-safety mode $f=$expected"
        fi
    done
    return $fail
}

# Complete source-side preflight.  It is deliberately local/read-only with respect to the
# destination and must run before provision, git init, checkout, rsync, or ref movement.
preflight_topology() {  # $1=active topology
    local t="$1"
    render_topology "$t" || return 9
    assert_band2_top_gitignore_parity || return 9
    assert_band_classification "$t" || return 9
    assert_source_runner_modes "$t" || return 9
    assert_sub_delegation_authorized "$t" || return 10
    validate_runtime_patches "$t" || return 9
}

# ── pre-flight ──
if ! $SSH_OPTS "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    echo "[sync] FAIL: $SUB_HOST 에 SSH 불가 (키 인증·네트워크 확인)"; exit 3
fi
HAS_GIT=0; _hg_rc=0; sub_has_git || _hg_rc=$?
case "$_hg_rc" in
    0) HAS_GIT=1 ;;
    1) HAS_GIT=0 ;;
    *) echo "[sync] STOP(F6): 서브 .git 존재 여부 판독 불가(rc=$_hg_rc) — 모르는 상태에서 B0 를 발동하지 않는다." >&2
       echo "       확인: $SSH_OPTS '$SUB_HOST' \"ls -d '$SUB_WORK_DIR/.git'\"" >&2; exit 3 ;;
esac
if [ "$HAS_GIT" != "0" ]; then
    REMOTE_ORIGINAL_BRANCH="$(sub_branch_current)" \
        || { echo "[sync] STOP(F7): 서브 현재 브랜치 판독 실패 — 원복 지점을 모른 채 배달하지 않는다." >&2; exit 8; }
    [ "$REMOTE_ORIGINAL_BRANCH" != "HEAD" ] \
        || { echo "[sync] STOP(F7): 서브가 detached HEAD — 원복이 no-op 이 되므로 배달 거부. 서브에서 브랜치를 체크아웃하라." >&2; exit 8; }
fi
SINGLE_PLANE_SOURCE="fail-closed:unevaluated"
SINGLE_ACTIVE=0; _single_extension_active && SINGLE_ACTIVE=1
prepare_transactional_source

# ═══════════════════════ DRY-RUN(계획 미리보기) ═══════════════════════
if [ "$MODE" = "dryrun" ]; then
    echo "[sync] DRY-RUN  $SRC → $SUB_HOST:$DEST"
    echo "  서브 git: $([ $HAS_GIT = 1 ] && echo '존재(증분 싱크)' \
        || echo "부재 → B0 멱등 self-bootstrap(git init + multi·single 브랜치 + 초기 커밋$([ -n "$BOOTSTRAP_POPULATE" ] && echo ' + multi 초기 배달' || echo '; multi 는 base 로 남김'))")"
    echo "  타겟 브랜치: ${TARGETS[*]}   single 확장: $([ $SINGLE_ACTIVE = 1 ] \
        && echo "활성(delivery_plane=active · source=${SINGLE_PLANE_SOURCE})" \
        || echo "dormant(delivery_plane=dormant · source=${SINGLE_PLANE_SOURCE} → 빌드킷 배달 skip)")"
    if [ $HAS_GIT = 1 ]; then
        for t in "${TARGETS[@]}"; do
            # dry-run 은 HITL 미리보기다 — apply 가 실제로 할 일과 어긋나면 사람이 잘못된 판단을 한다.
            #   (2026-09-03 평면 분리: 빌드킷만 skip, 에이전트 환경 오버레이는 배달된다.)
            _dry_skip_buildkit=0
            if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then
                _dry_skip_buildkit=1
                echo "  --- [single] 빌드킷 평면 dormant → build/source-port skip · **오버레이는 배달** ---"
            fi
            echo "  --- [$t] dirty 체크(fail-closed) → checkout → render → band단언 → rsync(빌드+오버레이) → [sync] 커밋 ---"
            render_topology "$t"
            # 2026-08-13: parity 는 apply 경로(preflight_topology)에만 있었다 — dry-run 이 **HITL 미리보기**인데
            #   사람이 승인 판단을 내리는 화면에서 이 게이트가 보이지 않았다. 검증되지 않은 tripwire 는 없는
            #   것과 같다(같은 날 verify_distribution 의 존재-단언이 3주 부패를 은폐한 것과 동형).
            assert_band2_top_gitignore_parity || echo "  [$t] ⚠ BAND2 ↔ gitignore 정합 실패(위 FAIL) — --apply 시 배달 거부."
            assert_band_classification "$t" || echo "  [$t] ⚠ S4 미분류 파일 존재(위 FAIL) — --apply 시 배달 거부. 분류 후 재시도."
            assert_source_runner_modes "$t" || echo "  [$t] ⚠ runner source mode 오류 — --apply 시 배달 거부. canonical materialize 후 재시도."
            if [ "$(_resolve_sub_hw_verified "$t" 2>/dev/null || true)" = "true" ]; then
                echo "    A2A-위임 게이트: nodes[sub].hw_verified=true ✓ (위임 키 발급·전파 허용)"
            else
                echo "    ⚠ A2A-위임 게이트: nodes[sub].hw_verified≠true → --apply 시 빌드 전파 거부(terraforming --peer-ssh 동질성 검증 먼저)"
            fi
            if [ "$_dry_skip_buildkit" != "1" ]; then
                preview_build "$t"
                preview_source_port_payload "$t"
            fi
            echo "    오버레이 미리보기(가산 — 삭제 없음):"; deliver_overlay "$t" 1 | sed 's/^/      /' | preview_lines || true
        done
    elif [ -n "$BOOTSTRAP_POPULATE" ]; then
        echo "  --- B0 bootstrap 미리보기(multi 초기 Band2 배달) ---"
        render_topology multi
        assert_band_classification multi || echo "  ⚠ S4 미분류(multi, 위 FAIL) — --apply 시 거부."
        assert_source_runner_modes multi || echo "  ⚠ runner source integrity 오류(multi) — --apply 시 remote mutation 전에 거부."
        preview_build multi
        preview_source_port_payload multi
    else
        echo "  --- B0 bootstrap 미리보기(base 만: git init + .gitignore + multi/single 브랜치) ---"
        echo "      multi 는 채우지 않는다 — 타겟이 아니며, 싱글 서브는 자기 빌드킷을 자율 저작한다(불변식 A)."
        for t in "${TARGETS[@]}"; do
            echo "  --- [$t] 부트스트랩 직후 배달 미리보기 ---"
            render_topology "$t"
            assert_band_classification "$t" || echo "  ⚠ S4 미분류($t) — --apply 시 거부."
            assert_source_runner_modes "$t" || echo "  ⚠ runner source integrity 오류($t) — --apply 시 거부."
            if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then
                echo "    빌드킷 평면 dormant → build/source-port skip · **에이전트 환경 오버레이만 배달**"
            else
                preview_build "$t"
                preview_source_port_payload "$t"
            fi
            echo "    오버레이 미리보기(가산 — 삭제 없음):"; deliver_overlay "$t" 1 | sed 's/^/      /' | preview_lines || true
        done
    fi
    echo "[sync] (위는 미리보기 — 변경 없음. 사람 확인 후 --apply. 첫 init 도 --apply 게이트.)"
    exit 0
fi

# ═══════════════════════ APPLY ═══════════════════════
# Resolve and validate every source topology required by this invocation before any remote
# filesystem/ref/working-tree mutation. Bootstrap always creates multi, even when only single
# was requested. Dormant single remains skipped.
PRECHECK_TARGETS=()
[ -n "$BOOTSTRAP_POPULATE" ] && PRECHECK_TARGETS+=("$BOOTSTRAP_POPULATE")
for t in "${TARGETS[@]}"; do
    # 2026-09-03(P3): dormant 라도 **오버레이는 배달**하므로 렌더/소스검사를 건너뛰면 안 된다.
    #   이전에는 여기서 continue 해 스테이징 자체가 만들어지지 않았고, B0 가 `.gitignore` 를
    #   rsync 하려다 "change_dir … sub_provision failed" 로 죽었다(원인이 전혀 드러나지 않는 형태).
    seen=0
    for p in "${PRECHECK_TARGETS[@]:-}"; do [ "$p" = "$t" ] && seen=1; done
    [ $seen = 1 ] || PRECHECK_TARGETS+=("$t")
done
for t in "${PRECHECK_TARGETS[@]}"; do
    preflight_topology "$t" || {
        rc=$?
        echo "[sync] STOP(source preflight): $t failed before remote mutation" >&2
        exit "${rc:-9}"
    }
done

# ── 쓰기 권한 프리플라이트 (2026-09-03 신설 · plan_26090317 P3 실화) ──────────────────
# 서브 work_dir 이 **존재하지만 우리 계정이 쓸 수 없는** 상태가 실제로 발생한다: 사용자가 프로젝트
# 경로를 완전삭제하면 root 로 도는 블랙박스 데몬이 다음 폴에서 그 경로를 **root:root 로 재생성**한다.
# 그 상태에서 배달을 시작하면 rsync 가 중간에 죽고, 롤백의 `rm` 마저 Permission denied 로 실패해
# **CRITICAL + 반쯤 갈린 서브**로 끝난다(2026-09-03 실측). 원인은 소유권인데 증상은 rsync 오류라
# 사람이 원인에 도달하지 못한다. 그러므로 **아무것도 건드리기 전에** 여기서 확인하고 처방을 말한다.
if $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR' ]" 2>/dev/null; then
    if ! $SSH_OPTS "$SUB_HOST" "[ -w '$SUB_WORK_DIR' ]" 2>/dev/null; then
        _owner="$($SSH_OPTS "$SUB_HOST" "stat -c '%U:%G %a' '$SUB_WORK_DIR'" 2>/dev/null || echo '판독불가')"
        echo "[sync] STOP: 서브 작업경로에 쓸 수 없다 — $SUB_HOST:$SUB_WORK_DIR (소유 $_owner)" >&2
        echo "       원인 후보: 프로젝트 경로 완전삭제 후 root 로 도는 노드블랙박스 데몬이 경로를 재생성했다." >&2
        echo "       처방(서브에서 사람이 1회 · sudo):" >&2
        echo "         sudo chown -R \$(id -un):\$(id -gn) '$SUB_WORK_DIR'" >&2
        echo "       근본 교정은 최신 install_node_blackbox.sh 재설치다 — 조상 소유권을 위임 사용자로 정렬한다." >&2
        exit 6
    fi
fi

# R2 HITL 게이트: 서브 work_dir 부재 시 자동신설 금지(--provision 필요).
if ! $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR' ]" 2>/dev/null; then
    if [ "$PROVISION" != "1" ]; then
        echo "[sync] STOP(R2): 서브 작업경로 부재 — $SUB_HOST:$SUB_WORK_DIR" >&2
        echo "       ❓ 신설하려면 --provision 을 더한다. 인가 인자(--mode·--manifest)는 생략 불가:" >&2
        echo "          bash sync_to_sub.sh --mode experimental --manifest <work-manifest.json> --apply --provision" >&2
        echo "       (work-manifest 만드는 법 = terraforming_node SKILL.md §2.3 '인가 체인')" >&2
        exit 5
    fi
    begin_remote_transaction "${BOOTSTRAP_POPULATE:-${TARGETS[0]}}" 1 || { echo "[sync] FAIL: provision 전 rollback transaction 생성 실패"; exit 9; }
    BOOTSTRAP_TX_PREPARED=1
    sub_run_mk() { $SSH_OPTS "$SUB_HOST" "mkdir -p -- '$SUB_WORK_DIR' && : >'${REMOTE_TX_DIRS[0]}/provisioned'"; }
    echo "[sync] PROVISION(승인됨): mkdir -p $SUB_HOST:$SUB_WORK_DIR"; sub_run_mk || { echo "[sync] FAIL: work_dir 신설 실패"; exit 5; }
    PROVISIONED_BY_SYNC=1
fi

# ── B0 멱등 self-bootstrap (서브 .git 부재 시) ──
if [ $HAS_GIT = 0 ]; then
    echo "[sync] B0 BOOTSTRAP — 서브 git init + multi·single 브랜치 (로컬 전용·origin 없음)"
    _bs_t="${BOOTSTRAP_POPULATE:-${TARGETS[0]}}"
    st="$(staging_dir "$_bs_t")"
    [ "$BOOTSTRAP_TX_PREPARED" = "1" ] || begin_remote_transaction "$_bs_t" 1 \
        || { echo "[sync] FAIL: bootstrap rollback transaction 생성 실패"; exit 9; }
    # Complete multi source preflight already passed before optional provision and this branch.
    # base = .gitignore 만(서브 로컬 추적규칙). 이후 multi 에만 전체 배달 → single 은 base(dormant) 로 격리.
    rsync -az -e "$SSH_OPTS" "$st/.gitignore" "$SUB_HOST:$DEST.gitignore"
    sub_run "git init -q"
    sub_run "git add .gitignore"
    sub_commit "[sync] bootstrap base (.gitignore) — D12 서브 로컬 git"
    sub_run "git branch -m multi"     # 기본 브랜치명 → multi
    sub_run "git branch single"       # single = base(.gitignore) — dormant
    if [ -n "$BOOTSTRAP_POPULATE" ]; then
        sub_run "git checkout -q multi"
        # multi 초기 Band2 배달.  Source gates above already passed before remote mutation.
        deliver_build multi 0
        deliver_source_port_payload multi || { echo "[sync] FAIL: bootstrap source-port payload 배달 실패 — commit 전 중단"; exit 2; }
        deliver_overlay multi 0
        echo "[sync] 체크섬 검증(multi)..."; verify_checksums multi || { echo "[sync] FAIL: bootstrap 체크섬 불일치 — commit 전 중단"; exit 2; }
        verify_source_port_payload multi || { echo "[sync] FAIL: bootstrap source-port 무결성 불일치 — commit 전 중단"; exit 2; }
        verify_destination_runner_modes multi || { echo "[sync] FAIL: bootstrap runner destination integrity 불일치 — commit 전 중단"; exit 2; }
        verify_destination_host_safety_modes || { echo "[sync] FAIL: bootstrap host-safety mode 불일치 — tombstone 전 중단"; exit 2; }
        verify_destination_retirement_consumers multi || { echo "[sync] FAIL: bootstrap retirement consumer 존재 — tombstone 전 중단"; exit 2; }
        apply_overlay_tombstones
        sub_run "git add -A"
        sub_commit "[sync] multi initial delivery — D12 bootstrap"
    else
        # 타겟에 multi 가 없다 = 싱글 온보딩. 브랜치 골격만 만들고 배달은 아래 B1 이 한다.
        #   multi 를 비워 두는 것은 결손이 아니라 **판정 결과**다(빌드킷 배달 평면 dormant).
        sub_run "git checkout -q single"
        echo "[sync] B0: multi 는 base 로 남긴다(타겟 아님 — 싱글 서브는 자기 빌드킷을 자율 저작한다)."
    fi
    # origin 부재 불변식 확증
    REMOTES="$(sub_run 'git remote')" \
        || { echo "[sync] FAIL(F9): 서브 origin 판독 실패 — 로컬 전용(D12)을 확증할 수 없으므로 거부." >&2; exit 7; }
    [ -z "$REMOTES" ] && echo "[sync] ✅ origin 0 (로컬 전용 확증)" || { echo "[sync] FAIL: 서브에 원격 존재($REMOTES) — D12 위반"; exit 7; }
    echo "[sync] B0 완료 — multi=$([ -n "$BOOTSTRAP_POPULATE" ] && echo populated || echo 'base(타겟 아님)'), single=base. 브랜치: $(sub_run 'git branch | tr -d "\n"')"
    # bootstrap 이 multi 를 이미 채움 → TARGETS 에서 multi 제거. 남은 타겟(single, --branch both/single)이 있으면 B1 로 진행.
    NEWT=(); for x in "${TARGETS[@]}"; do
        if [ -n "$BOOTSTRAP_POPULATE" ] && [ "$x" = "multi" ]; then continue; fi
        NEWT+=("$x")
    done
    TARGETS=("${NEWT[@]:-}"); [ -z "${TARGETS[*]:-}" ] && TARGETS=()
    [ ${#TARGETS[@]} -eq 0 ] && { finalize_remote_transactions; echo "[sync] 완료."; exit 0; }
    echo "[sync] bootstrap 후 잔여 타겟 B1 진행: ${TARGETS[*]}"
fi

# ── B1 per-branch 증분 싱크 ──
for t in "${TARGETS[@]}"; do
    # 2026-09-03(P2/P3 · plan_26090317): 여기서 `continue` 로 **타겟 전체**를 건너뛰었다. 그런데
    #   `delivery_plane` 이 판정하는 것은 이름 그대로 **빌드킷 배달 평면**이고(계약 docstring:
    #   "빌드킷 배달(rsync) 평면"), 서브의 **에이전트 환경 오버레이**(CLAUDE.md·Agent_Card·
    #   settings·comms·런타임 스킬)는 다른 평면이다 — 그것이 곧 헌법이 말하는 "A2A 에이전트 제어
    #   확장기능" 자체다. 두 평면을 한 스위치로 묶어 놓아, 싱글 서브는 **자기 정체성조차 배달받지
    #   못했다**(그 상태에서 A2A 위임은 성립하지 않는다). 평면을 가른다:
    #     · 빌드킷(deliver_build·source-port) → dormant 면 skip. 싱글 서브는 자율 저작한다.
    #     · 에이전트 환경(deliver_overlay)     → 배달한다. 그것이 확장기능의 내용이다.
    SKIP_BUILDKIT=0
    if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then
        SKIP_BUILDKIT=1
        echo "[sync] [single] 빌드킷 평면 DORMANT — delivery_plane=dormant (source=${SINGLE_PLANE_SOURCE})."
        echo "           싱글의 sub 는 A2A 원격 에이전트다 — 자기 빌드킷을 자율 저작한다(헌법 §불변식 A)."
        echo "           단 **에이전트 환경 오버레이는 배달한다** — 정체성·런타임 스킬이 곧 그 확장기능이다."
    fi
    echo "[sync] [$t] 증분 싱크 시작"
    # (1) dirty 체크 → 덮어쓰기 前 보존 (policy:SUB_SYNC_DIRTY_AUTOSAVE · plan_26081313)
    #   ⚠ 2026-08-13 교정: 옛 동작은 dirty 면 배달을 **거부**하고 "서브가 스스로 clean 화 후
    #   'ready-for-sync' 어테스트" 를 요구했다. 그 요구는 **평면 A(사용자↔메인)의 승인 규칙을
    #   평면 B(메인↔서브)에 잘못 투영**한 것이다 — B 에는 승인 주체가 없다(서브는 메인이 렌더·배달해
    #   만든 작업환경이고 headless 다). 그래서 안내가 지목하는 주체가 존재하지 않아 **교착**이 됐다.
    #   게이트의 원래 목적은 **소실 방지**이지 승인 획득이 아니었으므로, 목적만 남기고 주체를 메인으로
    #   바꾼다: stash(휘발) 가 아니라 **commit(보존)** 이라 히스토리에 영구 남고, 서브 git 을 둔 목적
    #   (메인의 서브 이력 추적)에 오히려 부합한다. 보존에 실패하면 그때는 fail-closed 한다 —
    #   보존 없는 배달만이 진짜 소실 위험이기 때문이다.
    DIRT="$(sub_dirty)" \
        || { echo "[sync] STOP(F1): 서브 git status 판독 실패 — dirty 여부를 모르면 배달하지 않는다(소실 방지)." >&2; exit 8; }
    if [ -n "$DIRT" ]; then
        echo "[sync] [$t] 서브 트리 dirty — 덮어쓰기 前 [improve] 커밋으로 보존한다(소실 방지)." >&2
        echo "$DIRT" | head -20 | sed 's/^/    /' >&2
        sub_run "git add -A" || { echo "[sync] STOP: 서브 작업물 stage 실패 — 보존 없는 배달은 소실 위험이므로 거부." >&2; exit 8; }
        sub_commit "[improve] pre-sync autosave ($(date -u +%Y%m%dT%H%M%SZ)) — main-initiated 보존" \
            || { echo "[sync] STOP: 서브 작업물 보존 커밋 실패 — 보존 없는 배달은 소실 위험이므로 거부." >&2; exit 8; }
        RE_DIRT="$(sub_dirty)" \
            || { echo "[sync] STOP(F1): 보존 후 재판독 실패 — 배달 거부." >&2; exit 8; }
        [ -z "$RE_DIRT" ] || { echo "[sync] STOP: 보존 후에도 dirty 잔존 — 배달 거부." >&2; echo "$RE_DIRT" | head -10 | sed 's/^/    /' >&2; exit 8; }
        echo "[sync] [$t] 보존 완료(서브 로컬 git) — 배달 계속." >&2
    fi
    # (2) transaction before checkout — complete source preflight ran globally before mutation.
    begin_remote_transaction "$t" 0 || { echo "[sync] FAIL: $t rollback transaction 생성 실패"; exit 9; }
    sub_run "git checkout -q $t" || { echo "[sync] FAIL: 서브 checkout $t 실패"; exit 8; }
    # (3) rsync(빌드 + 오버레이) — render/band/runner/delegation/runtime-patch
    # source checks all passed before checkout; deliver_build repeats patch validation.
    if [ "$SKIP_BUILDKIT" = "1" ]; then
        echo "[sync] [$t] 빌드킷·source-port 배달 skip(평면 dormant) — 오버레이만 진행."
    else
        deliver_build "$t" 0
        # 파생 payload 는 rsync 뒤에 온다 — 판정 권위인 PROVENANCE.json 이 먼저 서브에 있어야 한다.
        deliver_source_port_payload "$t" || { echo "[sync] FAIL: source-port payload 배달 실패($t)"; exit 2; }
    fi
    deliver_overlay "$t" 0
    echo "[sync] 체크섬 검증($t)..."; verify_checksums "$t" "$SKIP_BUILDKIT" || { echo "[sync] FAIL: 체크섬 불일치($t)"; exit 2; }
    if [ "$SKIP_BUILDKIT" != "1" ]; then
        verify_source_port_payload "$t" || { echo "[sync] FAIL: source-port 무결성 불일치($t)"; exit 2; }
        verify_destination_runner_modes "$t" || { echo "[sync] FAIL: runner destination mode 불일치($t)"; exit 2; }
    fi
    verify_destination_host_safety_modes || { echo "[sync] FAIL: host-safety mode 불일치($t) — tombstone 전 중단"; exit 2; }
    verify_destination_retirement_consumers "$t" || { echo "[sync] FAIL: retirement consumer 존재($t) — tombstone 전 중단"; exit 2; }
    apply_overlay_tombstones
    report_overlay_convergence "$t"   # 비석 적용 **뒤**에 센다 — 지운 것을 잔재로 세지 않는다
    report_runtime_block_residue "$t" # 정본이 "파일 0건" 을 두는 루트는 위 리포트가 구조적으로 못 본다
    # (5) [sync] 스크립트저작 커밋 (변경분만)
    sub_run "git add -A"
    if sub_run "git diff --cached --quiet"; then
        echo "[sync] [$t] 변경 없음 — 커밋 skip."
    else
        sub_commit "[sync] $t branch update ($(date -u +%Y%m%dT%H%M%SZ)) — main-canonical"
        echo "[sync] [$t] [sync] 커밋 완료: $(sub_run 'git log -1 --oneline')"
    fi
    LAST_DELIVERED="$t"
done
# 서브가 쉬는 브랜치 = **마지막으로 배달한 토폴로지**(2026-08-13 교정). 옛 동작은 무조건 multi 로
#   복귀했고 그 근거는 "single 은 dormant/확장" 이었는데, manifest 에 서브가 등록되면 single 은
#   **활성**이 되어 전제가 깨진다. 두 토폴로지 빌드킷은 한 워킹트리에서 **상호배타**다(통로 격리가
#   설계 의도) — 그러므로 복귀 브랜치는 그 노드가 지금 무슨 일을 하는가와 같아야 한다.
#   ★ 이 결함은 2026-08-13 이전에도 있었으나 **보이지 않았다**: single 빌드킷이 서브에서 비추적이라
#   checkout 이 건드리지 않았기 때문이다. gitignore 정합을 고쳐 추적이 시작되자 곧바로 표면화했다
#   (checkout multi → output/single/ 빌드킷 전부 삭제 → compose "no configuration file provided").
#   즉 **관측을 켜면 숨어 있던 결함이 드러난다** — 교정이 만든 결함이 아니라 은폐가 풀린 것이다.
REST_BRANCH="${LAST_DELIVERED:-multi}"
sub_run "git checkout -q $REST_BRANCH" >/dev/null 2>&1 \
    || { echo "[sync] FAIL: 최종 $REST_BRANCH branch 복귀 실패" >&2; exit 8; }
finalize_remote_transactions
echo "[sync] 완료. (현재 서브 브랜치: $(sub_branch_current))"
