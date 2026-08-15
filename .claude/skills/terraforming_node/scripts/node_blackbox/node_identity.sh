#!/bin/bash
# node_identity.sh — node_id **단일 해소기** (plan_26081514 §4.2 A · 스킴 R)
#   정본 선언: .claude/skills/terraforming_node/SKILL.md §2.7.6
#
#     node_id ::= manifest nodes[].role 슬러그
#     정규식  ::= ^[a-z][a-z0-9-]{0,31}$
#     경로    ::= <repo>/docs/logs/<node_id>
#
# ★ 왜 파일 하나인가 — 이전엔 5개 스크립트가 각자 `$(hostname)` 으로 파생했다. 각자 파싱하면
#   그 표(plan §2.1)가 그대로 재생산되고, 한 곳만 고치면 트리가 조용히 갈라진다. 소비자는
#   이 파일을 source 하고 `ni_resolve_node_id` 만 부른다.
#
# ★ **기본값이 없다.** 해소 실패는 `$(hostname)` 으로 떨어지지 않고 fail-loud 로 끝난다.
#   구 기본값 `NODE_ID="$(hostname)"` 은 헌법 §결정론 규율 4종 안티패턴 판정표의 **폴백·결함**
#   칸(= 결정·게이트·안전 경로에서 원인을 삼키는 침묵 폴백)이었다: 해소가 틀려도 에러 없이
#   **두 번째 로그 트리**가 생기고, 워치독은 아무도 안 보는 곳에 기록하며, 관측 공백은 사고가
#   나야 발견된다. 그리고 hostname 은 운영자 네트워크 지문이라 경로 성분이 되면 PII 다.
#
# 해소 우선순위 — **높은 쪽이 이긴다**:
#   1) 명시 `--node-id=<slug>`         — SKILL.md §2.7.6 표의 "명시 override"
#   2) `<repo>/Agent_Card.json`        — **서브측 권위**(`.node_identity.role`)
#   3) `<repo>/output/*/manifest.yaml` — **메인측 권위**(유일한 `role: main`)
#   4) fail-loud (return 1)
#
#   ⚠ plan §4.2 는 이 순서를 "manifest → Agent_Card → --node-id → fail-loud" 로 적었으나,
#     같은 plan §3.1·SKILL.md §2.7.6 표는 `--node-id` 를 **override** 로 규정한다. override 가
#     마지막 폴백이면 override 가 아니므로 명시 주입을 최우선으로 둔다(헌법 §불변식
#     "환경 주입이 manifest보다, manifest가 리터럴 기본값보다 우선"과 같은 방향).
#
#   ⚠ 2)가 3)보다 앞인 이유: 서브가 어떤 경위로든 manifest 사본을 갖게 되었을 때 자기를 main 으로
#     오인하면 **두 노드의 로그가 같은 이름으로 겹친다**(복구 불가한 혼합). 카드는 서브에만
#     배달되므로(`render_sub_env.py`) 카드의 존재 자체가 "나는 서브다"라는 신호다.
#
# 사용:
#   . "$(dirname "${BASH_SOURCE[0]}")/node_identity.sh"
#   NODE_ID="$(ni_resolve_node_id "$REPO" "$EXPLICIT_NODE_ID")" || exit 1
#
# 종료: 0=해소 성공(stdout 에 슬러그 한 줄) · 1=fail-loud(stderr 에 사유·해소 경로)

# 역할 어휘는 **닫힌 목록**이다. 공동 소유자: scripts/scan_node.py 의 manifest 게이트가
# roles.count("main")==1 · role ∈ {main,sub} 를 이미 강제한다(거기가 검증 권위).
# 여기 리터럴은 그 계약의 소비이며, 아래 ni_manifest_main_role 이 "정말 유일한가"를 다시
# 확인하므로 손으로 적은 값이 아니라 **검사되는 tripwire** 다 — 어휘를 늘리려면 두 곳이 함께 바뀐다.
NI_MAIN_ROLE="main"
NI_SLUG_RE='^[a-z][a-z0-9-]{0,31}$'

ni_valid_slug() {  # $1=후보 → 0=적합
  [ -n "${1:-}" ] || return 1
  printf '%s' "$1" | grep -Eq "$NI_SLUG_RE"
}

# Agent_Card.json → .node_identity.role (서브측 권위).
# JSON 을 셸로 긁지 않는다 — python3 는 이 패키지의 이미 확정된 의존이다(blackbox_*.py).
# python3 가 없으면 **모른다고 말하고 실패**한다(hostname 으로 떨어지지 않는다).
ni_agent_card_role() {  # $1=repo → stdout=슬러그 · 1=부재/해소불가
  local card="$1/Agent_Card.json"
  [ -f "$card" ] || return 1
  command -v python3 >/dev/null 2>&1 || {
    echo "[node-identity] FAIL: $card 는 있으나 python3 가 없어 role 을 읽지 못한다." >&2
    echo "[node-identity]       python3 설치 후 재시도하거나 --node-id=<slug> 로 명시 주입하라." >&2
    return 2
  }
  python3 - "$card" <<'PY' 2>/dev/null
import json, sys
try:
    doc = json.load(open(sys.argv[1], encoding="utf-8"))
except Exception:
    sys.exit(1)
role = (doc.get("node_identity") or {}).get("role")
if not isinstance(role, str) or not role:
    sys.exit(1)
print(role.strip())
PY
}

# output/*/manifest.yaml → 유일한 `- role: main` 슬러그 (메인측 권위).
# 여러 토폴로지 manifest 가 공존하면 값이 **일치해야** 한다 — 갈리면 fail-loud(추측 금지).
ni_manifest_main_role() {  # $1=repo → stdout=슬러그 · 1=부재 · 2=계약 위반
  local repo="$1" mf found="" n=0 hits
  for mf in "$repo"/output/*/manifest.yaml; do
    [ -f "$mf" ] || continue
    # `- role: main` 정확매칭(주석·후행공백 허용). 접두 오인 방지는 _mf_sub 선례와 동일.
    hits=$(grep -Ec "^[[:space:]]*-[[:space:]]*role:[[:space:]]*${NI_MAIN_ROLE}([[:space:]]|#|$)" "$mf" || true)
    [ "$hits" -eq 0 ] && continue
    if [ "$hits" -ne 1 ]; then
      echo "[node-identity] FAIL: $mf 에 role: ${NI_MAIN_ROLE} 항목이 ${hits}개다(계약은 정확히 1개)." >&2
      echo "[node-identity]       scan_node.py manifest 게이트를 다시 통과시켜라." >&2
      return 2
    fi
    n=$((n + 1)); found="$NI_MAIN_ROLE"
  done
  [ "$n" -gt 0 ] || return 1
  printf '%s' "$found"
}

# 단일 진입점.
ni_resolve_node_id() {  # $1=repo  $2=명시 --node-id(없으면 빈 문자열) → stdout=슬러그 · 1=fail-loud
  local repo="${1:-}" explicit="${2:-}" v rc
  if [ -n "$explicit" ]; then
    if ni_valid_slug "$explicit"; then printf '%s' "$explicit"; return 0; fi
    echo "[node-identity] FAIL: --node-id='$explicit' 가 스킴 위반이다(정규식 $NI_SLUG_RE)." >&2
    return 1
  fi
  if [ -z "$repo" ] || [ ! -d "$repo" ]; then
    echo "[node-identity] FAIL: repo 루트를 못 찾았다(node_id 해소 불가)." >&2
    return 1
  fi

  v="$(ni_agent_card_role "$repo")"; rc=$?
  if [ "$rc" -eq 2 ]; then return 1; fi
  if [ "$rc" -eq 0 ] && [ -n "$v" ]; then
    if ni_valid_slug "$v"; then printf '%s' "$v"; return 0; fi
    echo "[node-identity] FAIL: Agent_Card.json 의 node_identity.role='$v' 가 스킴 위반이다($NI_SLUG_RE)." >&2
    return 1
  fi

  v="$(ni_manifest_main_role "$repo")"; rc=$?
  if [ "$rc" -eq 2 ]; then return 1; fi
  if [ "$rc" -eq 0 ] && [ -n "$v" ]; then
    if ni_valid_slug "$v"; then printf '%s' "$v"; return 0; fi
    echo "[node-identity] FAIL: manifest role='$v' 가 스킴 위반이다($NI_SLUG_RE)." >&2
    return 1
  fi

  # 여기가 옛 `$(hostname)` 자리다. 조용히 만들지 않는다.
  echo "[node-identity] FAIL: node_id 를 해소하지 못했다 — hostname 으로 대체하지 않는다." >&2
  echo "[node-identity]   찾은 곳: (a) $repo/Agent_Card.json  (b) $repo/output/*/manifest.yaml 의 role: ${NI_MAIN_ROLE}" >&2
  echo "[node-identity]   해소: 서브면 render_sub_env.py 로 Agent_Card.json 재배달 · 메인이면 manifest 를" >&2
  echo "[node-identity]         terraforming_node 로 채워라. 급하면 --node-id=<slug> 로 명시 주입." >&2
  return 1
}

# 자기검사: `bash node_identity.sh --self-test` (부수효과 없음 — 임시 디렉터리에서만 논다)
if [ "${BASH_SOURCE[0]}" = "${0}" ] && [ "${1:-}" = "--self-test" ]; then
  _t="$(mktemp -d)"; _fail=0
  _ck(){ if [ "$2" = "$3" ]; then echo "  ok   $1"; else echo "  FAIL $1: '$2' != '$3'"; _fail=1; fi; }

  _ck "빈 repo → fail-loud" "$(ni_resolve_node_id "$_t" "" 2>/dev/null; echo "rc=$?")" "rc=1"
  _ck "명시 override 최우선" "$(ni_resolve_node_id "$_t" "sub2" 2>/dev/null)" "sub2"
  _ck "명시 슬러그 위반 거부" "$(ni_resolve_node_id "$_t" "Spark-A73E" 2>/dev/null; echo "rc=$?")" "rc=1"

  mkdir -p "$_t/output/multi"
  printf 'nodes:\n  - role: main\n    host: 10.0.0.1\n  - role: sub\n    host: 10.0.0.2\n' \
    > "$_t/output/multi/manifest.yaml"
  _ck "manifest → main" "$(ni_resolve_node_id "$_t" "" 2>/dev/null)" "main"

  printf '{"node_identity": {"role": "sub"}}' > "$_t/Agent_Card.json"
  _ck "Agent_Card 가 manifest 를 이긴다" "$(ni_resolve_node_id "$_t" "" 2>/dev/null)" "sub"
  _ck "명시가 카드도 이긴다" "$(ni_resolve_node_id "$_t" "main" 2>/dev/null)" "main"

  printf '{"node_identity": {"role": "SUB!"}}' > "$_t/Agent_Card.json"
  _ck "카드 슬러그 위반 거부" "$(ni_resolve_node_id "$_t" "" 2>/dev/null; echo "rc=$?")" "rc=1"

  rm -f "$_t/Agent_Card.json"
  printf 'nodes:\n  - role: main\n  - role: main\n' > "$_t/output/multi/manifest.yaml"
  _ck "main 중복 → fail-loud" "$(ni_resolve_node_id "$_t" "" 2>/dev/null; echo "rc=$?")" "rc=1"

  rm -rf "$_t"
  [ "$_fail" = 0 ] && echo "[node-identity] --self-test PASS" || echo "[node-identity] --self-test FAIL"
  exit "$_fail"
fi
