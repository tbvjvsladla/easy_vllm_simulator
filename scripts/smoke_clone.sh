#!/bin/bash
# smoke_clone.sh — G1 합격 게이트(7 hermetic assertion).
#
# 순수 결정론·STAGE-FREE: claude 세션 비실행, git add/commit/rm/checkout 일절 안 함.
# 사용 도구는 읽기 전용뿐 — git check-ignore / git ls-files / grep / test.
# 7개 전부 PASS 면 exit 0(= G1 done), 하나라도 FAIL 이면 exit 1.
#
# 주의: 이 스크립트는 "G1 전체 세트가 적용된 뒤"의 추적 트리를 검증한다(plan §9, S7).
#   .gitignore 재분할·PII 제거 등이 끝나기 전에는 의도적으로 FAIL 할 수 있다(게이트이므로 정상).
#
# 사용:  bash scripts/smoke_clone.sh
set -uo pipefail

# 레포 루트로 이동(어디서 호출하든 동일 기준).
ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || { echo "FAIL: git 레포 아님"; exit 1; }
cd "$ROOT"

# ─────────────────────────────────────────────────────────────────────────────
# 데이터 주도 설정 (패턴·경로를 상단에 모음 — B2 단순성)
# ─────────────────────────────────────────────────────────────────────────────

# PII 금지 리터럴(A4). 추적/미추적-비무시 후보 전반에서 0 매치여야 한다.
PII_REGEX='(192\.168\.|coga[-_]|spark-a73e|spark-bdc9|naver\.com|/mnt/llm|/home/|tbvjvsladla)'

# 헌법·rules 가 참조하는 "문서화된 생성 산출물"(gitignore 대상이라 부재가 정상 — dangling 예외 허용).
DOC_GENERATED_ARTIFACTS=(
    manifest.yaml
    config.yaml
    Dockerfile
    Dockerfile.source-build
    docker-compose.yaml
    .claude/settings.local.json
)

# A5: 반드시 "추적 가능(미무시)"+존재 해야 하는 빌딩블럭.
MUST_TRACKED=(
    CLAUDE.md
    .claude/rules/workflow.md
    .claude/rules/docs.md
    .claude/skills/vllm-recipe-explorer/SKILL.md
    .claude/skills/upstream-version-watch/SKILL.md
    manifest.template.yaml
)
# A5: 반드시 "무시됨"이어야 하는 사용자 실값·로컬 산출물.
MUST_IGNORED=(
    .claude/settings.local.json
    manifest.yaml
    .claude/skills/vllm-recipe-explorer/config.yaml
    .claude/skills/upstream-version-watch/config.yaml
)

# A6: manifest.template.yaml 에 있어야 하는 필드명.
MANIFEST_FIELDS=(topology cpu_arch cuda_version gpus_per_node nas_model_path origin_url nodes interconnect)

# A2: 최소 기대 스킬(부재 시 FAIL). terraforming_node 등은 optional(있으면 검사, 없으면 보고만).
EXPECTED_SKILLS=(vllm-recipe-explorer upstream-version-watch)

# A7: 렌더러 후보 경로(첫 번째 존재 항목 사용). 부재 시 레포 전체 검색 폴백.
#   render 는 upstream-version-watch 소유(G2 구현). scripts/ 는 구(舊) 스텁 위치(이전됨).
RENDER_CANDIDATES=(
    .claude/skills/upstream-version-watch/scripts/render_dockerfile.py
    scripts/render_dockerfile.py
    render_dockerfile.py
)

# ─────────────────────────────────────────────────────────────────────────────
# 헬퍼
# ─────────────────────────────────────────────────────────────────────────────
PASS_N=0
FAIL_N=0
pass() { echo "PASS  $1"; PASS_N=$((PASS_N+1)); }
fail() { echo "FAIL  $1"; FAIL_N=$((FAIL_N+1)); }

# 문서화된 생성 산출물인지(dangling 예외) 판정.
is_doc_generated() {
    local cand="$1" a
    for a in "${DOC_GENERATED_ARTIFACTS[@]}"; do [ "$cand" = "$a" ] && return 0; done
    return 1
}

# ─────────────────────────────────────────────────────────────────────────────
# A1 — 헌법 파싱: CLAUDE.md 마크다운 sanity + @-include·rules 참조 경로 resolve
# ─────────────────────────────────────────────────────────────────────────────
a1() {
    local ok=1
    if [ ! -s CLAUDE.md ]; then fail "A1 헌법 파싱: CLAUDE.md 부재/빈 파일"; return; fi
    # 마크다운 sanity: 최소 1개의 H1(# ) 헤더.
    if ! grep -qE '^# ' CLAUDE.md; then echo "  - CLAUDE.md 에 H1 헤더 없음"; ok=0; fi
    # @-include(줄 시작 @경로) 전수 resolve.
    while IFS= read -r inc; do
        [ -z "$inc" ] && continue
        if [ ! -e "$inc" ]; then echo "  - @-include 미해소: $inc"; ok=0; fi
    done < <(grep -oE '^@[^[:space:]]+' CLAUDE.md | sed 's/^@//')
    # CLAUDE.md 가 언급한 .claude/rules/*.md 경로 resolve.
    while IFS= read -r rp; do
        [ -z "$rp" ] && continue
        if [ ! -e "$rp" ]; then echo "  - rules 참조 미해소: $rp"; ok=0; fi
    done < <(grep -oE '\.claude/rules/[A-Za-z0-9_./-]+\.md' CLAUDE.md | sort -u)
    [ "$ok" -eq 1 ] && pass "A1 헌법 파싱: CLAUDE.md sanity + @-include/rules 참조 전수 resolve" \
                    || fail "A1 헌법 파싱: 위 미해소 항목 존재"
}

# ─────────────────────────────────────────────────────────────────────────────
# A2 — 스킬 목록: 각 SKILL.md 존재·비어있지 않음·인식가능 헤더(frontmatter name/description 또는 H1)
# ─────────────────────────────────────────────────────────────────────────────
a2() {
    local ok=1 s skill_md found_skills=()
    shopt -s nullglob
    for skill_md in .claude/skills/*/SKILL.md; do
        found_skills+=("$(basename "$(dirname "$skill_md")")")
        if [ ! -s "$skill_md" ]; then echo "  - 빈 SKILL.md: $skill_md"; ok=0; continue; fi
        # 인식가능 헤더: YAML frontmatter(name+description) 또는 H1 타이틀.
        if grep -qE '^name:' "$skill_md" && grep -qE '^description:' "$skill_md"; then
            : # frontmatter OK
        elif grep -qE '^# ' "$skill_md"; then
            : # H1 타이틀 OK
        else
            echo "  - SKILL.md 헤더 인식 불가(frontmatter name/description·H1 모두 없음): $skill_md"; ok=0
        fi
    done
    shopt -u nullglob
    # 최소 기대 스킬 존재 확인.
    local e
    for e in "${EXPECTED_SKILLS[@]}"; do
        if [ ! -s ".claude/skills/$e/SKILL.md" ]; then echo "  - 기대 스킬 누락: $e"; ok=0; fi
    done
    # terraforming_node(optional): 없으면 보고만.
    if [ ! -e ".claude/skills/terraforming_node/SKILL.md" ]; then
        echo "  - (info) optional 스킬 terraforming_node 아직 없음 — FAIL 아님"
    fi
    [ "$ok" -eq 1 ] && pass "A2 스킬 목록: SKILL.md 전수 유효(헤더+기대 스킬 존재)" \
                    || fail "A2 스킬 목록: 위 문제 존재"
}

# ─────────────────────────────────────────────────────────────────────────────
# A3 — dangling 0: 헌법·rules 가 참조한 "레포-상대 경로"(슬래시 포함) 전수 resolve
#   범위 한정(결정론·오탐 제거): 슬래시를 포함한 repo-relative 경로만 검사한다.
#     · 절대경로(/etc/pip/..., /app/...)·서사 속 단독 파일명(SKILL.md 등)은 "참조 경로"가 아니므로 제외.
#   resolve 규칙: (a) test -e 그대로 존재, (b) 문서화된 생성 산출물(allowlist),
#     (c) 동일 basename 파일이 레포 어딘가에 추적 존재(서브디렉토리 거주·브랜치별 빌딩블럭 커버) → OK.
#   그 외(어디에도 없는 슬래시 경로)만 dangling 으로 FAIL.
# ─────────────────────────────────────────────────────────────────────────────
a3() {
    local ok=1 src ref base
    local SRCS=(CLAUDE.md)
    while IFS= read -r r; do SRCS+=("$r"); done < <(ls .claude/rules/*.md 2>/dev/null)
    # 추적/비무시 트리의 basename 인덱스(규칙 c 판정용).
    local TRACKED; TRACKED="$(git ls-files --cached --others --exclude-standard 2>/dev/null)"
    for src in "${SRCS[@]}"; do
        [ -e "$src" ] || continue
        while IFS= read -r ref; do
            [ -z "$ref" ] && continue
            ref="${ref%/}"                                  # 후행 슬래시 제거
            case "$ref" in /*) continue ;; esac             # 절대경로 → 참조 아님(제외)
            case "$ref" in */*) : ;; *) continue ;; esac    # 슬래시 없는 단독 파일명 → 제외
            [ -e "$ref" ] && continue                       # (a) 존재 → OK
            is_doc_generated "$ref" && continue             # (b) 문서화된 생성 산출물 → 예외
            base="$(basename "$ref")"
            if printf '%s\n' "$TRACKED" | grep -qE "(^|/)${base//./\\.}$"; then
                continue                                    # (c) 동일 basename 추적 존재 → OK
            fi
            echo "  - dangling 참조: $ref  (in $src)"; ok=0
        done < <(grep -oE '(\.claude/[A-Za-z0-9_./-]+|/?[A-Za-z0-9_][A-Za-z0-9_./-]*/[A-Za-z0-9_./-]*\.(md|py|sh|yaml|yml|txt|json|template))' "$src" \
                  | sort -u)
    done
    [ "$ok" -eq 1 ] && pass "A3 dangling 0: repo-relative 경로 참조 전수 resolve(절대경로·단독파일명·생성물 제외)" \
                    || fail "A3 dangling 0: 위 미해소 참조 존재"
}

# ─────────────────────────────────────────────────────────────────────────────
# A4 — PII 0: 비무시(추적/미추적-비무시) 후보 전반에서 PII 리터럴 0 매치
# ─────────────────────────────────────────────────────────────────────────────
a4() {
    local matches selfname
    selfname="scripts/smoke_clone.sh"   # 자기 자신은 PII 패턴 정의를 담으므로 제외.
    # 후보 = 스테이징 추적 + 미추적이지만 .gitignore 비무시(= 추적 예정 트리).
    matches="$(git ls-files --cached --others --exclude-standard \
               | grep -v -x "$selfname" \
               | while IFS= read -r f; do
                     [ -f "$f" ] || continue
                     grep -InE "$PII_REGEX" "$f" 2>/dev/null | sed "s#^#$f:#"
                 done)"
    if [ -z "$matches" ]; then
        pass "A4 PII 0: 비무시 트리 전반 금지 리터럴 0 매치"
    else
        fail "A4 PII 0: 아래 매치 존재"
        echo "$matches" | sed 's/^/  - /'
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# A5 — .gitignore 재분할: 빌딩블럭은 추적가능+존재, 사용자 실값은 무시됨
# ─────────────────────────────────────────────────────────────────────────────
a5() {
    local ok=1 f
    for f in "${MUST_TRACKED[@]}"; do
        if git check-ignore -q "$f"; then echo "  - 추적되어야 하나 무시됨: $f"; ok=0; fi
        if [ ! -e "$f" ]; then echo "  - 추적 대상 부재: $f"; ok=0; fi
    done
    for f in "${MUST_IGNORED[@]}"; do
        if ! git check-ignore -q "$f"; then echo "  - 무시되어야 하나 추적가능: $f"; ok=0; fi
    done
    [ "$ok" -eq 1 ] && pass "A5 gitignore 재분할: 빌딩블럭 추적 / 사용자 실값 무시" \
                    || fail "A5 gitignore 재분할: 위 불일치 존재"
}

# ─────────────────────────────────────────────────────────────────────────────
# A6 — manifest 스키마: manifest.template.yaml 존재 + 필수 필드명 전수 포함
# ─────────────────────────────────────────────────────────────────────────────
a6() {
    local ok=1 fld
    if [ ! -f manifest.template.yaml ]; then fail "A6 manifest 스키마: manifest.template.yaml 부재"; return; fi
    for fld in "${MANIFEST_FIELDS[@]}"; do
        if ! grep -qE "^[[:space:]]*${fld}[[:space:]]*:" manifest.template.yaml; then
            echo "  - 누락 필드: $fld"; ok=0
        fi
    done
    [ "$ok" -eq 1 ] && pass "A6 manifest 스키마: 필수 필드명 전수 포함" \
                    || fail "A6 manifest 스키마: 위 필드 누락"
}

# ─────────────────────────────────────────────────────────────────────────────
# A7 — 렌더러: 존재 + 실행가능 + --self-test 통과(G2 구현됨). (구 스텁 검사에서 격상)
# ─────────────────────────────────────────────────────────────────────────────
a7() {
    local r="" c out rc
    for c in "${RENDER_CANDIDATES[@]}"; do [ -f "$c" ] && { r="$c"; break; }; done
    if [ -z "$r" ]; then
        # 폴백: 레포 전체에서 검색(무시 경로 제외).
        r="$(git ls-files --cached --others --exclude-standard '*render_dockerfile.py' 2>/dev/null | head -1)"
    fi
    if [ -z "$r" ] || [ ! -f "$r" ]; then fail "A7 렌더러: render_dockerfile.py 부재"; return; fi
    if [ ! -x "$r" ]; then fail "A7 렌더러: $r 실행권한 없음(chmod +x 필요)"; return; fi
    out="$(python3 "$r" --self-test 2>&1)"; rc=$?
    if [ "$rc" -ne 0 ]; then
        fail "A7 렌더러: $r --self-test 실패(rc=$rc): $(echo "$out" | head -1)"; return
    fi
    if echo "$out" | grep -qiE 'self-test OK|render'; then
        pass "A7 렌더러: $r 구현됨·self-test 통과(rc=0)"
    else
        fail "A7 렌더러: $r self-test 출력 비정상 (out: $(echo "$out" | head -1))"
    fi
}

# ─────────────────────────────────────────────────────────────────────────────
# 실행
# ─────────────────────────────────────────────────────────────────────────────
echo "=== smoke_clone.sh — G1 합격 게이트 (7 hermetic assertion) ==="
a1; a2; a3; a4; a5; a6; a7
echo "------------------------------------------------------------"
TOTAL=$((PASS_N+FAIL_N))
if [ "$FAIL_N" -eq 0 ]; then
    echo "SMOKE: PASS ($PASS_N/$TOTAL)"
    exit 0
else
    echo "SMOKE: FAIL ($PASS_N/$TOTAL)"
    exit 1
fi
