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
#   single 확장 게이트(D12 Gap B) — output/single/manifest.yaml nodes[] 에 sub 있으면 활성, 없으면 **dormant(배달 skip)**.
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
PATCH_VALIDATOR="$SRC.claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
PATCH_RESOLUTION="$SRC.claude/skills/upstream-version-watch/assets/current-production-resolution.json"

# ── manifest 해소(서브 접속·work_dir = 항상 multi 통로 manifest. single 은 nodes:[] 라 서브 미정의) ──
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
# single 확장 활성? = output/single/manifest.yaml nodes[] 에 role:sub 가 있나(D12 Gap B 결정론 게이트).
_single_extension_active() {
    local manifest="${SRC%/}/output/single/manifest.yaml"
    [ -f "$manifest" ] || return 1
    awk '/^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/ { found=1 } END { exit(found?0:1) }' "$manifest"
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

[ -z "${SUB_HOST:-}" ]     && SUB_HOST="$(_resolve_sub_host_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="$(_resolve_sub_work_dir_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="${SRC%/}"
if [ -z "${SUB_HOST:-}" ]; then
    echo "[sync] FAIL: 서브노드 주소 미해소 — SUB_HOST(<ssh_user>@<host>) 지정 또는 manifest nodes[](role:sub) 채우기." >&2
    exit 4
fi
DEST="${SUB_WORK_DIR}/"

# 타겟 브랜치 목록
TARGETS=(); case "$BRANCH" in multi) TARGETS=(multi);; single) TARGETS=(single);; both) TARGETS=(multi single);; esac

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
BAND2_PATCH_DIRS=(build_patches build_patches_src)
# output/<t>/ 최상위 **비전송** 경로의 단일 소유. 세 소비자가 전부 여기서 파생한다:
#   _band2_filters(rsync --exclude) · validate_remote_deletion_tree(서브 walk) · validate_inventory_tree(로컬 walk).
# ★ 목록을 두 벌 두면 갈라진다 — 이 프로젝트는 파서 두 벌(D4↔D6)·TP 오카운트 7사이트로 같은 계열
#   사고를 이미 겪었다. 갈라졌을 때의 실해악이 2026-08-01 에 현실화됐다: rsync 는 cache/ 를 제외하는데
#   검증기는 그걸 걷어서, 컨테이너가 root 로 만든 **빈 캐시 디렉터리** 하나가 모든 후속 배달을 영구
#   차단했다(비-root 서브 계정은 자력 해소 불가 → 매번 사람 sudo). 검증기의 시야는 전송 범위를
#   넘지 않는다 — 롤백은 rsync 가 바꿀 수 있는 것만 덮으면 되기 때문이다.
# 근거: manifest.yaml=D10 · .env=serve-time node-local host config(PII, render --materialize-env 산출) ·
#   sub_provision=overlay 소관 · benchlog=adversarial-benchmark 생성 증거(빌드입력 ✗, plan_26063014) ·
#   cache·tiktoken_cache=노드-로컬 JIT·tokenizer 캐시(빌드입력 ✗·전파 ✗, plan_26072217).
BAND2_EXCLUDED_TOP=(manifest.yaml sub_provision .env benchlog cache tiktoken_cache)
BAND2_RUNTIME_PATCH_STEMS=(exaone45-33b hy3)          # owner-local provenance-bound runtime patches; wildcard authority 금지
# ↑ Dockerfile.source-build-upstage = Solar-Open2 변종 트랙(UpstageAI 포크 @ v0.22.0-solar-open2).
#   Band2 편입 근거 = **빌드-평면**: 멀티는 클러스터-와이드 이미지라 슬레이브도 동일 이미지를 빌드해야 한다
#   (헌법 변종이미지 build-plane ≠ serve-plane 따름정리 · workflow.md S2.5). 모델-키잉 ✗ — track 은 포크
#   **벤더**명이며 stock 0.22.0 의 superset. Band3 모델 트리플렛은 계속 배제.

_band2_filters() {  # rsync include/exclude(첫매치우선). 소스 루트 = output/<t>/.
    FILT=()
    local f
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
    for f in "${BAND2_PATCH_DIRS[@]}"; do                               # 빌드 패치 모듈 디렉토리(Band2 빌드입력·서브 빌드가 COPY — §4.7·3+1+1)
        FILT+=(--include="/$f/" --include="/$f/**")                     #   build_patches=post(컴파일 후) · build_patches_src=pre(컴파일 전, 소스 이식)
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
    local f t d bad=0 label path
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
                # 내용 예외는 **최소 1줄** 있으면 된다 — 어떤 범위인지는 밴드 규정이 정한다.
                #   `build_patches` 는 `/*`(전부 손작성 .sh), `build_patches_src` 는 `/*.sh`+`/PROVENANCE.json`
                #   (payload `files/` 는 업스트림 벤더링 = 파생 산출물이라 비추적). 여기서 범위를 못박으면
                #   밴드 규정이 바뀔 때마다 정규식을 고쳐야 하므로, **존재**만 단언하고 범위는 규정에 맡긴다.
                grep -qE "^!output/$t/$d/.+" "$path" || {
                    echo "[sync] FAIL(parity/$label): '$d' 의 내용 예외가 한 줄도 없다 — 디렉토리만 재포함하면 파일은 여전히 무시된다." >&2; bad=1; }
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
)
OVERLAY_RELOCATION_STALE_PATHS=(
    .claude/rules/references.md
    scripts/install_host_safety.sh
    scripts/mem_watchdog.sh
    scripts/host/vllm-drop-caches.sh
    scripts/systemd/easy-vllm-memwatch.service
)
# Main-only orchestration has no sub runtime replacement.  Deletion is retirement and therefore
# requires an explicit active-consumer audit before these exact paths are removed.
OVERLAY_RETIREMENT_STALE_PATHS=(
    scripts/smoke_clone.sh
    scripts/sync_branches.sh
)

# ── 서브 git 헬퍼 ──
sub_run()  { $SSH_OPTS "$SUB_HOST" "cd '$SUB_WORK_DIR' && $1"; }
sub_has_git() { $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR/.git' ]" 2>/dev/null; }
sub_dirty() { sub_run "git status --porcelain 2>/dev/null"; }
sub_commit() { sub_run "git -c user.name='$GIT_NAME' -c user.email='$GIT_EMAIL' commit -q -m \"$1\""; }
sub_branch_current() { sub_run "git rev-parse --abbrev-ref HEAD 2>/dev/null"; }

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
    python3 "$RENDER" --topology "$1" >/dev/null || return 9
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
        _band2_filters
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
    else head="$(sub_run "git rev-parse '$t'")"; branch="$t"; fi
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
    # output files never enter the snapshot. The sole filesystem exception is manifest.yaml: it is
    # topology input (possibly PII), is explicitly copied mode 0600, deterministically renders the
    # transaction, and is excluded from remote delivery by _band2_filters.
    if ! git -C "$CANONICAL_SRC" diff --quiet -- .claude CLAUDE.md .gitignore output/multi output/single; then
        echo "[sync] info: canonical worktree drift detected; filesystem bytes are excluded in favor of index authority"
    fi
    git -C "$CANONICAL_SRC" ls-files -z -- .claude CLAUDE.md .gitignore output/multi output/single \
        | git -C "$CANONICAL_SRC" checkout-index -z --stdin --prefix="$TRANSACTIONAL_SRC/"
    local topology
    for topology in multi single; do
        [ -f "${CANONICAL_SRC}output/$topology/manifest.yaml" ] || continue
        mkdir -p "$TRANSACTIONAL_SRC/output/$topology"
        install -m 0600 "${CANONICAL_SRC}output/$topology/manifest.yaml" \
            "$TRANSACTIONAL_SRC/output/$topology/manifest.yaml"
    done
    SRC="$TRANSACTIONAL_SRC/"
    RENDER="$SRC.claude/skills/terraforming_node/scripts/render_sub_env.py"
    PATCH_VALIDATOR="$SRC.claude/skills/upstream-version-watch/scripts/validate_runtime_patch.py"
    PATCH_RESOLUTION="$SRC.claude/skills/upstream-version-watch/assets/current-production-resolution.json"
    echo "[sync] transactional source prepared (canonical tree remains read-only): $TRANSACTIONAL_SRC"
}

# 빌드 콘텐츠 rsync(S4): 소스 = output/<t>/ 서브트리만(루트 Band1 구조적 배제) + Band2 keying. dry 면 --dry-run.
# --delete 이중 스코프(d-rsync-5): (a) dst=output/<t>/ 한정 → 서브 루트·.claude·docs 불가침(별 평면) ·
#   (b) 그 안에서도 exclude 된 Band3(configs/*·envs/* − allowlist)·중첩 dir 는 *보호*(삭제 대상 아님) → 서브 자작 트리플렛 보존
#   = full mirror 아님. stale main-origin Band3 잔재 회수는 gate③ cleanup 소관(루트 Band1 leak 과 동일 평면).
deliver_build() {  # $1=topology $2=dry(0/1)
    validate_runtime_patches "$1" || return 9
    _band2_filters
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
    printf '%s\n' "$out" | grep -v '^\*deleting' | sed 's/^/      /' | head -40 || true
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

verify_destination_retirement_consumers() {
    local stale hits fail=0 scan_py scan_q stale_q
    scan_py=$'# retirement_consumer_scan\nimport os,re,sys\nstale=sys.argv[1]\nowner=".claude/skills/upstream-version-watch/"+stale\nchars=set("ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_./-")\nneedle=re.compile(r"(?<![A-Za-z0-9_.-])"+re.escape(stale)+r"(?![A-Za-z0-9_./-])")\nhits=[]\ndef scan(path):\n if os.path.islink(path): raise RuntimeError("active scanner refuses symlink: "+path)\n data=open(path,"rb").read().decode("utf-8","replace")\n for lineno,line in enumerate(data.splitlines(),1):\n  for match in needle.finditer(line):\n   lo,hi=match.start(),match.end()\n   while lo and line[lo-1] in chars: lo-=1\n   while hi<len(line) and line[hi] in chars: hi+=1\n   token=line[lo:hi]\n   while token.startswith("./"): token=token[2:]\n   if token!=owner: hits.append(f"{path}:{lineno}:{line}")\nroots=[".claude","CLAUDE.md"]\nif os.path.exists("HINTS.md"): roots.append("HINTS.md")\nfor root in roots:\n if os.path.isdir(root):\n  for base,dirs,files in os.walk(root,onerror=lambda e: (_ for _ in ()).throw(e)):\n   dirs[:]=[d for d in dirs if d!=".git"]\n   for name in files: scan(os.path.join(base,name))\n elif os.path.exists(root): scan(root)\nprint("\\n".join(hits),end="")'
    printf -v scan_q '%q' "$scan_py"
    for stale in "${OVERLAY_RETIREMENT_STALE_PATHS[@]}"; do
        printf -v stale_q '%q' "$stale"
        if hits="$(sub_run "python3 -c $scan_q $stale_q")"; then
            :
        else
            echo "[sync] FAIL(retirement consumer): scanner/transport failed for $stale" >&2
            return 98
        fi
        if [ -n "$hits" ]; then
            echo "[sync] FAIL(retirement consumer): $stale is still referenced on active sub surfaces:" >&2
            printf '%s\n' "$hits" | sed 's/^/    /' >&2
            fail=1
        else
            echo "  ✅ retirement consumer audit: $stale has no active sub consumer"
        fi
    done
    return $fail
}
# 체크섬 검증(빌드 핵심입력 + 오버레이 대표). 불일치 시 비-0.
verify_checksums() {  # $1=topology
    local st; st="$(staging_dir "$1")"; local fail=0 L R f
    for f in "output/$1/Dockerfile" "output/$1/Dockerfile.source-build" "output/$1/docker-compose.yaml" "output/$1/requirements.txt"; do
        [ -f "${SRC}${f}" ] || continue
        L=$(md5sum "${SRC}${f}" | awk '{print $1}'); R=$($SSH_OPTS "$SUB_HOST" "md5sum '${SUB_WORK_DIR}/${f}' 2>/dev/null" | awk '{print $1}')
        [ -n "$L" ] && [ "$L" = "$R" ] && echo "  ✅ ${f}" || { echo "  ❌ ${f}: main=$L sub=$R"; fail=1; }
    done
    for f in CLAUDE.md Agent_Card.json .claude/settings.local.json .claude/rules/comms.md .claude/rules/docs.md \
             .claude/schemas/task-report.schema.json .gitignore .claude/skills/vllm-recipe-explorer/recipe.py \
             .claude/skills/adversarial-benchmark/scripts/verdict_rule.py .claude/skills/wiki-desk/reference/references.md .claude/a2a_delegation.json \
             .claude/runtime/host_safety/mem_watchdog.sh \
             .claude/runtime/host_safety/install_host_safety.sh \
             .claude/runtime/host_safety/install_netconsole.sh \
             .claude/runtime/host_safety/host/vllm-drop-caches.sh \
             .claude/runtime/host_safety/systemd/easy-vllm-memwatch.service; do
        [ -f "$st/$f" ] || continue
        L=$(md5sum "$st/$f" | awk '{print $1}'); R=$($SSH_OPTS "$SUB_HOST" "md5sum '$SUB_WORK_DIR/$f' 2>/dev/null" | awk '{print $1}')
        [ -n "$L" ] && [ "$L" = "$R" ] && echo "  ✅ $f" || { echo "  ❌ $f: main=$L sub=$R"; fail=1; }
    done
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
HAS_GIT=0; sub_has_git && HAS_GIT=1
[ "$HAS_GIT" = "0" ] || REMOTE_ORIGINAL_BRANCH="$(sub_branch_current)"
SINGLE_ACTIVE=0; _single_extension_active && SINGLE_ACTIVE=1
prepare_transactional_source

# ═══════════════════════ DRY-RUN(계획 미리보기) ═══════════════════════
if [ "$MODE" = "dryrun" ]; then
    echo "[sync] DRY-RUN  $SRC → $SUB_HOST:$DEST"
    echo "  서브 git: $([ $HAS_GIT = 1 ] && echo '존재(증분 싱크)' || echo '부재 → B0 멱등 self-bootstrap(git init + multi·single 브랜치 + 초기 커밋)')"
    echo "  타겟 브랜치: ${TARGETS[*]}   single 확장: $([ $SINGLE_ACTIVE = 1 ] && echo '활성' || echo 'dormant(single manifest nodes[] 비어있음 → 배달 skip)')"
    if [ $HAS_GIT = 1 ]; then
        for t in "${TARGETS[@]}"; do
            if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then echo "  --- [single] dormant → skip ---"; continue; fi
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
            preview_build "$t"
            echo "    오버레이 미리보기(가산 — 삭제 없음):"; deliver_overlay "$t" 1 | sed 's/^/      /' | head -40 || true
        done
    else
        echo "  --- B0 bootstrap 미리보기(multi 초기 Band2 배달) ---"
        render_topology multi
        assert_band_classification multi || echo "  ⚠ S4 미분류(multi, 위 FAIL) — --apply 시 거부."
        assert_source_runner_modes multi || echo "  ⚠ runner source integrity 오류(multi) — --apply 시 remote mutation 전에 거부."
        preview_build multi
    fi
    echo "[sync] (위는 미리보기 — 변경 없음. 사람 확인 후 --apply. 첫 init 도 --apply 게이트.)"
    exit 0
fi

# ═══════════════════════ APPLY ═══════════════════════
# Resolve and validate every source topology required by this invocation before any remote
# filesystem/ref/working-tree mutation. Bootstrap always creates multi, even when only single
# was requested. Dormant single remains skipped.
PRECHECK_TARGETS=()
[ $HAS_GIT = 0 ] && PRECHECK_TARGETS+=(multi)
for t in "${TARGETS[@]}"; do
    if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then continue; fi
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

# R2 HITL 게이트: 서브 work_dir 부재 시 자동신설 금지(--provision 필요).
if ! $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR' ]" 2>/dev/null; then
    if [ "$PROVISION" != "1" ]; then
        echo "[sync] STOP(R2): 서브 작업경로 부재 — $SUB_HOST:$SUB_WORK_DIR" >&2
        echo "       ❓ 신설하려면: bash sync_to_sub.sh --apply --provision (HITL — 자동 경로 신설 금지)" >&2
        exit 5
    fi
    begin_remote_transaction multi 1 || { echo "[sync] FAIL: provision 전 rollback transaction 생성 실패"; exit 9; }
    BOOTSTRAP_TX_PREPARED=1
    sub_run_mk() { $SSH_OPTS "$SUB_HOST" "mkdir -- '$SUB_WORK_DIR' && : >'${REMOTE_TX_DIRS[0]}/provisioned'"; }
    echo "[sync] PROVISION(승인됨): mkdir -p $SUB_HOST:$SUB_WORK_DIR"; sub_run_mk || { echo "[sync] FAIL: work_dir 신설 실패"; exit 5; }
    PROVISIONED_BY_SYNC=1
fi

# ── B0 멱등 self-bootstrap (서브 .git 부재 시) ──
if [ $HAS_GIT = 0 ]; then
    echo "[sync] B0 BOOTSTRAP — 서브 git init + multi·single 브랜치 (로컬 전용·origin 없음)"
    st="$(staging_dir multi)"
    [ "$BOOTSTRAP_TX_PREPARED" = "1" ] || begin_remote_transaction multi 1 \
        || { echo "[sync] FAIL: bootstrap rollback transaction 생성 실패"; exit 9; }
    # Complete multi source preflight already passed before optional provision and this branch.
    # base = .gitignore 만(서브 로컬 추적규칙). 이후 multi 에만 전체 배달 → single 은 base(dormant) 로 격리.
    rsync -az -e "$SSH_OPTS" "$st/.gitignore" "$SUB_HOST:$DEST.gitignore"
    sub_run "git init -q"
    sub_run "git add .gitignore"
    sub_commit "[sync] bootstrap base (.gitignore) — D12 서브 로컬 git"
    sub_run "git branch -m multi"     # 기본 브랜치명 → multi
    sub_run "git branch single"       # single = base(.gitignore) — dormant
    sub_run "git checkout -q multi"
    # multi 초기 Band2 배달.  Source gates above already passed before remote mutation.
    deliver_build multi 0
    deliver_overlay multi 0
    echo "[sync] 체크섬 검증(multi)..."; verify_checksums multi || { echo "[sync] FAIL: bootstrap 체크섬 불일치 — commit 전 중단"; exit 2; }
    verify_destination_runner_modes multi || { echo "[sync] FAIL: bootstrap runner destination integrity 불일치 — commit 전 중단"; exit 2; }
    verify_destination_host_safety_modes || { echo "[sync] FAIL: bootstrap host-safety mode 불일치 — tombstone 전 중단"; exit 2; }
    verify_destination_retirement_consumers || { echo "[sync] FAIL: bootstrap retirement consumer 존재 — tombstone 전 중단"; exit 2; }
    apply_overlay_tombstones
    sub_run "git add -A"
    sub_commit "[sync] multi initial delivery — D12 bootstrap"
    # origin 부재 불변식 확증
    REMOTES="$(sub_run 'git remote' || true)"
    [ -z "$REMOTES" ] && echo "[sync] ✅ origin 0 (로컬 전용 확증)" || { echo "[sync] FAIL: 서브에 원격 존재($REMOTES) — D12 위반"; exit 7; }
    echo "[sync] B0 완료 — multi=populated, single=base(dormant). 브랜치: $(sub_run 'git branch | tr -d "\n"')"
    # bootstrap 이 multi 를 이미 채움 → TARGETS 에서 multi 제거. 남은 타겟(single, --branch both/single)이 있으면 B1 로 진행.
    NEWT=(); for x in "${TARGETS[@]}"; do [ "$x" = "multi" ] || NEWT+=("$x"); done
    TARGETS=("${NEWT[@]:-}"); [ -z "${TARGETS[*]:-}" ] && TARGETS=()
    [ ${#TARGETS[@]} -eq 0 ] && { finalize_remote_transactions; echo "[sync] 완료."; exit 0; }
    echo "[sync] bootstrap 후 잔여 타겟 B1 진행: ${TARGETS[*]}"
fi

# ── B1 per-branch 증분 싱크 ──
for t in "${TARGETS[@]}"; do
    if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then
        echo "[sync] [single] DORMANT — single manifest nodes[] 비어있음(확장 비활성). 배달 skip(브랜치는 base 유지)."
        continue
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
    DIRT="$(sub_dirty || true)"
    if [ -n "$DIRT" ]; then
        echo "[sync] [$t] 서브 트리 dirty — 덮어쓰기 前 [improve] 커밋으로 보존한다(소실 방지)." >&2
        echo "$DIRT" | head -20 | sed 's/^/    /' >&2
        sub_run "git add -A" || { echo "[sync] STOP: 서브 작업물 stage 실패 — 보존 없는 배달은 소실 위험이므로 거부." >&2; exit 8; }
        sub_commit "[improve] pre-sync autosave ($(date -u +%Y%m%dT%H%M%SZ)) — main-initiated 보존" \
            || { echo "[sync] STOP: 서브 작업물 보존 커밋 실패 — 보존 없는 배달은 소실 위험이므로 거부." >&2; exit 8; }
        RE_DIRT="$(sub_dirty || true)"
        [ -z "$RE_DIRT" ] || { echo "[sync] STOP: 보존 후에도 dirty 잔존 — 배달 거부." >&2; echo "$RE_DIRT" | head -10 | sed 's/^/    /' >&2; exit 8; }
        echo "[sync] [$t] 보존 완료(서브 로컬 git) — 배달 계속." >&2
    fi
    # (2) transaction before checkout — complete source preflight ran globally before mutation.
    begin_remote_transaction "$t" 0 || { echo "[sync] FAIL: $t rollback transaction 생성 실패"; exit 9; }
    sub_run "git checkout -q $t" || { echo "[sync] FAIL: 서브 checkout $t 실패"; exit 8; }
    # (3) rsync(빌드 + 오버레이) — render/band/runner/delegation/runtime-patch
    # source checks all passed before checkout; deliver_build repeats patch validation.
    deliver_build "$t" 0
    deliver_overlay "$t" 0
    echo "[sync] 체크섬 검증($t)..."; verify_checksums "$t" || { echo "[sync] FAIL: 체크섬 불일치($t)"; exit 2; }
    verify_destination_runner_modes "$t" || { echo "[sync] FAIL: runner destination mode 불일치($t)"; exit 2; }
    verify_destination_host_safety_modes || { echo "[sync] FAIL: host-safety mode 불일치($t) — tombstone 전 중단"; exit 2; }
    verify_destination_retirement_consumers || { echo "[sync] FAIL: retirement consumer 존재($t) — tombstone 전 중단"; exit 2; }
    apply_overlay_tombstones
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
