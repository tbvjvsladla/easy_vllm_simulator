#!/bin/bash
# 메인노드 → 서브노드 **브랜치-aware 하향 싱크 오케스트레이터** (D12).
#
# 멀티노드 확장의 [하향 배달] 단계. 메인 검증 런타임을 서브로 직접 전송(GitHub 경유 X)하고,
# 서브 **로컬 git**(single·multi 두 브랜치, origin 영구 없음)에 **스크립트저작 `[sync]` 커밋**으로 박는다.
# 상향(서브 자기개선 회수)은 별도·문서기반: fetch_sub_docs.sh. (plan_2026062411_1 · workflow.md §양방향 브랜치싱크)
#
# 흐름:
#   B0 멱등 self-bootstrap — 서브 .git 부재 시: git init + base(.gitignore) 커밋 + multi·single 브랜치 생성,
#                            그리고 multi 브랜치 초기 전체 배달(+커밋). single=base(dormant). **첫 init=HITL(--apply)**.
#   B1 per-branch 증분(--branch) — dirty 체크(fail-closed) → checkout → render → rsync(빌드+오버레이) → [sync] 커밋.
#        겹침 = main-canonical(sub-yields). 서브 [improve] history 는 git 에 잔존.
#   single 확장 게이트(D12 Gap B) — output/single/manifest.yaml nodes[] 에 sub 있으면 활성, 없으면 **dormant(배달 skip)**.
#
# HITL 안전장치: 기본 DRY-RUN(미리보기). 실제 변경은 --apply. **스크립트 auto-stash 금지**(서브가 스스로 clean 화).
# 빌드 배달(S4 Band2-only · plan_2026062417_1): rsync 소스 = output/<t>/ 서브트리만 → 루트 Band1(템플릿·scripts·resolved.json) 구조적 배제.
#   output/<t>/ 내 keying: Band2(configs/{serve_runner,debug-init}.sh · envs/.env.interconnect · Dockerfile·compose·requirements·.dockerignore·.gitkeep) 전파 ·
#   Band3(모델 트리플렛 configs/<m>.{sh,yaml}·envs/.env.<m>) keying 배제 · manifest.yaml(D10)·sub_provision(overlay) 배제 · 미분류=fail-loud(assert_band_classification).
#   에이전트 환경(CLAUDE.md·.claude·docs·.gitignore)은 deliver_overlay/bootstrap 소관(빌드 배달과 분리).
#
# 사용:
#   bash sync_to_sub.sh                          # DRY-RUN (bootstrap/증분 계획 + rsync 미리보기)
#   bash sync_to_sub.sh --apply                  # 실행 (기본 --branch multi)
#   bash sync_to_sub.sh --apply --provision      # 서브 work_dir 부재 시 신설(HITL)
#   bash sync_to_sub.sh --apply --branch single  # single 브랜치 타겟(확장 dormant 면 skip)
#   bash sync_to_sub.sh --apply --branch both    # multi 후 single
# 환경변수 override: SUB_HOST(<ssh_user>@<host>) · SRC · SUB_WORK_DIR · SYNC_GIT_NAME · SYNC_GIT_EMAIL.
# SUB_HOST·SUB_WORK_DIR 미지정 시 output/multi/manifest.yaml nodes[](role:sub)에서 해소(서브는 multi manifest 에만 정의).
set -euo pipefail

SRC="${SRC:-$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)/}"
SSH_OPTS="ssh -o BatchMode=yes -o ConnectTimeout=8"
GIT_NAME="${SYNC_GIT_NAME:-easy-vllm sync (main)}"      # [sync] 커밋 = 스크립트저작 표식
GIT_EMAIL="${SYNC_GIT_EMAIL:-sync@easy-vllm.local}"
MAX_DELETE="${MAX_DELETE:-50}"     # (레거시) --delete 안전캡. S4 는 아래 ALLOW_DELETE 삭제brake 가 1차 게이트.
ALLOW_DELETE="${ALLOW_DELETE:-0}"  # (S4 d-rsync-2) 삭제 前 brake: 삭제예정 > 이 값이면 *삭제 前* fail-closed. 의도된 정리만 명시 override.
RENDER="$SRC.claude/skills/terraforming_subnode/scripts/render_sub_env.py"

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

[ -z "${SUB_HOST:-}" ]     && SUB_HOST="$(_resolve_sub_host_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="$(_resolve_sub_work_dir_from_manifest || true)"
[ -z "${SUB_WORK_DIR:-}" ] && SUB_WORK_DIR="${SRC%/}"
if [ -z "${SUB_HOST:-}" ]; then
    echo "[sync] FAIL: 서브노드 주소 미해소 — SUB_HOST(<ssh_user>@<host>) 지정 또는 manifest nodes[](role:sub) 채우기." >&2
    exit 4
fi
DEST="${SUB_WORK_DIR}/"

MODE="dryrun"; PROVISION=0; BRANCH="multi"
while [ $# -gt 0 ]; do
    case "$1" in
        --apply)     MODE="apply" ;;
        --provision) PROVISION=1 ;;
        --branch)    shift; BRANCH="${1:-multi}" ;;
        --branch=*)  BRANCH="${1#*=}" ;;
        *) echo "[sync] (warn) 미지 인자: $1" >&2 ;;
    esac
    shift
done
case "$BRANCH" in multi|single|both) ;; *) echo "[sync] FAIL: --branch 는 multi|single|both" >&2; exit 6 ;; esac

# 타겟 브랜치 목록
TARGETS=(); case "$BRANCH" in multi) TARGETS=(multi);; single) TARGETS=(single);; both) TARGETS=(multi single);; esac

# ── S4 Band2 빌드킷 keying(전파 = output/<t>/ 의 Band2 만 · plan_2026062417_1 rev3 R1) ──
# Band1(루트 템플릿·scripts·resolved.json)은 rsync 소스가 output/<t>/ 라 구조적으로 빠지고,
# Band3(모델 recipe = <model>.{sh,yaml}·모델 env)는 아래 keying 으로 빠진다. 미분류는 assert_band_classification 가 fail-loud.
# ⚠ 정본 주의(d12-1): 서브 gitignore.template 의 `!configs/serve_runner.sh` 는 *루트* configs/ 대상이라 이 allowlist 와
#   *동치 아님*. output/<t>/ Band2 추적은 gitignore.template 의 output/ 예외(!output/<t>/configs/serve_runner.sh 등)가 관할.
BAND2_CONFIGS=(serve_runner.sh debug-init.sh)        # topology-keyed 분산서빙 인프라(Band2, 멀티)
BAND2_ENVS=(.env.interconnect .env.cluster)          # topology/network-keyed env(Band2): NCCL(.interconnect) + 클러스터배포(.cluster=S6 materialize)
BAND2_TOP=(Dockerfile Dockerfile.source-build docker-compose.yaml requirements.txt .gitkeep)  # 최상위 빌드킷(Band2)

_band2_filters() {  # rsync include/exclude(첫매치우선). 소스 루트 = output/<t>/.
    FILT=(--exclude='/manifest.yaml' --exclude='/sub_provision' --exclude='/.env')   # D10 manifest·serve-time .env(node-local host config·PII, render --materialize-env 산출) 미전달 · 에이전트환경=overlay
    local f
    FILT+=(--include='/configs/')
    for f in "${BAND2_CONFIGS[@]}"; do FILT+=(--include="/configs/$f"); done
    FILT+=(--exclude='/configs/*')                # 나머지 configs(모델 트리플렛 Band3) 배제
    FILT+=(--include='/envs/')
    for f in "${BAND2_ENVS[@]}"; do FILT+=(--include="/envs/$f"); done
    FILT+=(--exclude='/envs/*')                   # 나머지 envs(모델 env Band3) 배제
    # (d-rsync-3) 최상위는 default-include 가 아니라 명시 allowlist + terminal exclude → stray Band1/secret/log·.dockerignore(§1.3 불요) 누출 차단
    for f in "${BAND2_TOP[@]}"; do FILT+=(--include="/$f"); done
    FILT+=(--exclude='/*')
}

# ── S4 fail-loud band 분류 단언(plan ⊕rev3 keying linter) ──
# output/<t>/ 의 (a) 최상위 (b) configs/ (c) envs/ 모든 항목이 Band2(allowlist) 또는 Band3(완전 모델 트리플렛
# .sh+.yaml / .env.<model>)로 명확 분류되는지 + (멀티) Band2 인프라가 소스에 실재하는지 검증. 미분류/누락 → 비-0.
assert_band_classification() {  # $1=topology → 0=ok, 1=미분류·누락
    local odir="${SRC%/}/output/$1" cdir="${SRC%/}/output/$1/configs" edir="${SRC%/}/output/$1/envs" bad=0 f b ok stem
    local nullsave; nullsave="$(shopt -p nullglob dotglob || true)"; shopt -s nullglob dotglob
    local -A _b2c _b2e _b2top
    for b in "${BAND2_CONFIGS[@]}"; do _b2c["$b"]=1; done
    for b in "${BAND2_ENVS[@]}"; do _b2e["$b"]=1; done
    for b in "${BAND2_TOP[@]}" configs envs manifest.yaml sub_provision .env; do _b2top["$b"]=1; done

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
        ok=0
        case "$b" in                                                     # (d-band-2) 짝의 .sh 가 Band2 면 Band3 로 green-light 안 함(stem 충돌 차단)
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

    # (d-rsync-1) 멀티: 소스 Band2 인프라 부재 시 fail-closed(빈 소스가 --delete 로 서브 Band2 인프라 wipe 방지)
    if [ "$1" = "multi" ]; then
        for b in "${BAND2_CONFIGS[@]}"; do [ -f "$cdir/$b" ] || { echo "[sync] FAIL(S4 통로미완결): output/$1/configs/$b 부재 — 'render_dockerfile.py --materialize-configs --topology $1' 선행." >&2; bad=1; }; done
        for b in "${BAND2_ENVS[@]}"; do [ -f "$edir/$b" ] || { echo "[sync] FAIL(S4 통로미완결): output/$1/envs/$b 부재 — render(NCCL/cluster envfile, S6) 선행." >&2; bad=1; }; done
    fi

    eval "$nullsave"; return $bad
}
OVERLAY_EXCLUDES=(--exclude '__pycache__' --exclude '*.pyc')

# ── 서브 git 헬퍼 ──
sub_run()  { $SSH_OPTS "$SUB_HOST" "cd '$SUB_WORK_DIR' && $1"; }
sub_has_git() { $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR/.git' ]" 2>/dev/null; }
sub_dirty() { sub_run "git status --porcelain 2>/dev/null"; }
sub_commit() { sub_run "git -c user.name='$GIT_NAME' -c user.email='$GIT_EMAIL' commit -q -m \"$1\""; }
sub_branch_current() { sub_run "git rev-parse --abbrev-ref HEAD 2>/dev/null"; }

render_topology() { python3 "$RENDER" --topology "$1" >/dev/null; }
staging_dir()     { echo "${SRC%/}/output/$1/sub_provision"; }

# 빌드 콘텐츠 rsync(S4): 소스 = output/<t>/ 서브트리만(루트 Band1 구조적 배제) + Band2 keying. dry 면 --dry-run.
# --delete 이중 스코프(d-rsync-5): (a) dst=output/<t>/ 한정 → 서브 루트·.claude·docs 불가침(별 평면) ·
#   (b) 그 안에서도 exclude 된 Band3(configs/*·envs/* − allowlist)·중첩 dir 는 *보호*(삭제 대상 아님) → 서브 자작 트리플렛 보존
#   = full mirror 아님. stale main-origin Band3 잔재 회수는 gate③ cleanup 소관(루트 Band1 leak 과 동일 평면).
deliver_build() {  # $1=topology $2=dry(0/1)
    _band2_filters
    local src="${SRC%/}/output/$1/" dst="${DEST}output/$1/"
    if [ "$2" = "1" ]; then
        rsync -az --delete --dry-run --itemize-changes "${FILT[@]}" -e "$SSH_OPTS" "$src" "$SUB_HOST:$dst"
        return
    fi
    # apply: (d-rsync-2) 삭제 前 brake — dry-run 으로 삭제예정 세고 ALLOW_DELETE 초과 시 *삭제 前* fail-closed(부분삭제 0)
    sub_run "mkdir -p 'output/$1'"
    local ndel
    ndel="$(rsync -az --delete --dry-run --itemize-changes "${FILT[@]}" -e "$SSH_OPTS" "$src" "$SUB_HOST:$dst" 2>/dev/null | grep -c '^\*deleting' || true)"
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
             .claude/schemas/task-report.schema.json .gitignore .claude/skills/vllm-recipe-explorer/recipe.py; do
        [ -f "$st/$f" ] || continue
        L=$(md5sum "$st/$f" | awk '{print $1}'); R=$($SSH_OPTS "$SUB_HOST" "md5sum '$SUB_WORK_DIR/$f' 2>/dev/null" | awk '{print $1}')
        [ -n "$L" ] && [ "$L" = "$R" ] && echo "  ✅ $f" || { echo "  ❌ $f: main=$L sub=$R"; fail=1; }
    done
    return $fail
}

# ── pre-flight ──
if ! $SSH_OPTS "$SUB_HOST" 'echo ok' >/dev/null 2>&1; then
    echo "[sync] FAIL: $SUB_HOST 에 SSH 불가 (키 인증·네트워크 확인)"; exit 3
fi
HAS_GIT=0; sub_has_git && HAS_GIT=1
SINGLE_ACTIVE=0; _single_extension_active && SINGLE_ACTIVE=1

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
            assert_band_classification "$t" || echo "  [$t] ⚠ S4 미분류 파일 존재(위 FAIL) — --apply 시 배달 거부. 분류 후 재시도."
            preview_build "$t"
            echo "    오버레이 미리보기(가산 — 삭제 없음):"; deliver_overlay "$t" 1 | sed 's/^/      /' | head -40 || true
        done
    else
        echo "  --- B0 bootstrap 미리보기(multi 초기 Band2 배달) ---"
        render_topology multi
        assert_band_classification multi || echo "  ⚠ S4 미분류(multi, 위 FAIL) — --apply 시 거부."
        preview_build multi
    fi
    echo "[sync] (위는 미리보기 — 변경 없음. 사람 확인 후 --apply. 첫 init 도 --apply 게이트.)"
    exit 0
fi

# ═══════════════════════ APPLY ═══════════════════════
# R2 HITL 게이트: 서브 work_dir 부재 시 자동신설 금지(--provision 필요).
if ! $SSH_OPTS "$SUB_HOST" "[ -d '$SUB_WORK_DIR' ]" 2>/dev/null; then
    if [ "$PROVISION" != "1" ]; then
        echo "[sync] STOP(R2): 서브 작업경로 부재 — $SUB_HOST:$SUB_WORK_DIR" >&2
        echo "       ❓ 신설하려면: bash sync_to_sub.sh --apply --provision (HITL — 자동 경로 신설 금지)" >&2
        exit 5
    fi
    sub_run_mk() { $SSH_OPTS "$SUB_HOST" "mkdir -p '$SUB_WORK_DIR'"; }
    echo "[sync] PROVISION(승인됨): mkdir -p $SUB_HOST:$SUB_WORK_DIR"; sub_run_mk || { echo "[sync] FAIL: work_dir 신설 실패"; exit 5; }
fi

# ── B0 멱등 self-bootstrap (서브 .git 부재 시) ──
if [ $HAS_GIT = 0 ]; then
    echo "[sync] B0 BOOTSTRAP — 서브 git init + multi·single 브랜치 (로컬 전용·origin 없음)"
    render_topology multi
    st="$(staging_dir multi)"
    # base = .gitignore 만(서브 로컬 추적규칙). 이후 multi 에만 전체 배달 → single 은 base(dormant) 로 격리.
    rsync -az -e "$SSH_OPTS" "$st/.gitignore" "$SUB_HOST:$DEST.gitignore"
    sub_run "git init -q"
    sub_run "git add .gitignore"
    sub_commit "[sync] bootstrap base (.gitignore) — D12 서브 로컬 git"
    sub_run "git branch -m multi"     # 기본 브랜치명 → multi
    sub_run "git branch single"       # single = base(.gitignore) — dormant
    sub_run "git checkout -q multi"
    # multi 초기 Band2 배달 (S4 band 단언 = 하드 게이트)
    assert_band_classification multi || { echo "[sync] STOP(S4): multi band 분류 실패 — bootstrap 중단(위 FAIL 분류 후 재시도)." >&2; exit 9; }
    deliver_build multi 0
    deliver_overlay multi 0
    sub_run "git add -A"
    sub_commit "[sync] multi initial delivery — D12 bootstrap"
    echo "[sync] 체크섬 검증(multi)..."; verify_checksums multi || { echo "[sync] FAIL: bootstrap 체크섬 불일치"; exit 2; }
    # origin 부재 불변식 확증
    REMOTES="$(sub_run 'git remote' || true)"
    [ -z "$REMOTES" ] && echo "[sync] ✅ origin 0 (로컬 전용 확증)" || { echo "[sync] FAIL: 서브에 원격 존재($REMOTES) — D12 위반"; exit 7; }
    echo "[sync] B0 완료 — multi=populated, single=base(dormant). 브랜치: $(sub_run 'git branch | tr -d "\n"')"
    # bootstrap 이 multi 를 이미 채움 → TARGETS 에서 multi 제거. 남은 타겟(single, --branch both/single)이 있으면 B1 로 진행.
    NEWT=(); for x in "${TARGETS[@]}"; do [ "$x" = "multi" ] || NEWT+=("$x"); done
    TARGETS=("${NEWT[@]:-}"); [ -z "${TARGETS[*]:-}" ] && TARGETS=()
    [ ${#TARGETS[@]} -eq 0 ] && { echo "[sync] 완료."; exit 0; }
    echo "[sync] bootstrap 후 잔여 타겟 B1 진행: ${TARGETS[*]}"
fi

# ── B1 per-branch 증분 싱크 ──
for t in "${TARGETS[@]}"; do
    if [ "$t" = "single" ] && [ $SINGLE_ACTIVE = 0 ]; then
        echo "[sync] [single] DORMANT — single manifest nodes[] 비어있음(확장 비활성). 배달 skip(브랜치는 base 유지)."
        continue
    fi
    echo "[sync] [$t] 증분 싱크 시작"
    # (1) dirty 체크 — fail-closed (스크립트 auto-stash 금지)
    DIRT="$(sub_dirty || true)"
    if [ -n "$DIRT" ]; then
        echo "[sync] STOP(fail-closed): 서브 트리 dirty — 배달 거부(클로버 방지)." >&2
        echo "$DIRT" | head -20 | sed 's/^/    /' >&2
        echo "  → 서브가 'git add -A && git commit'(또는 git stash)로 clean 화 후 'ready-for-sync' 어테스트 → 재시도." >&2
        echo "  (정본: 메인은 너 대신 stash 하지 않는다 — workflow.md §양방향 브랜치싱크 B1-1.)" >&2
        exit 8
    fi
    # (2) checkout
    sub_run "git checkout -q $t" || { echo "[sync] FAIL: 서브 checkout $t 실패"; exit 8; }
    # (3) render + band단언(S4 하드 게이트) + (4) rsync(빌드 + 오버레이)  — 겹침=main-canonical
    render_topology "$t"
    assert_band_classification "$t" || { echo "[sync] STOP(S4): $t band 분류 실패 — 배달 거부(위 FAIL 분류 후 재시도)." >&2; exit 9; }
    deliver_build "$t" 0
    deliver_overlay "$t" 0
    echo "[sync] 체크섬 검증($t)..."; verify_checksums "$t" || { echo "[sync] FAIL: 체크섬 불일치($t)"; exit 2; }
    # (5) [sync] 스크립트저작 커밋 (변경분만)
    sub_run "git add -A"
    if sub_run "git diff --cached --quiet"; then
        echo "[sync] [$t] 변경 없음 — 커밋 skip."
    else
        sub_commit "[sync] $t branch update ($(date -u +%Y%m%dT%H%M%SZ)) — main-canonical"
        echo "[sync] [$t] [sync] 커밋 완료: $(sub_run 'git log -1 --oneline')"
    fi
done
# 서브를 기본 운용 브랜치(multi)로 복귀 — 멀티노드 서브의 active role(single 은 dormant/확장).
sub_run "git checkout -q multi" >/dev/null 2>&1 || true
echo "[sync] 완료. (현재 서브 브랜치: $(sub_branch_current))"
