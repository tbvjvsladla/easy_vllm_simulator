#!/bin/bash
# budget_renew_loop.sh — **상주 서빙의 예산 선언을 살아 있게 유지하는 사이드카** (W-7).
#
# 왜 있나(2026-08-22 실측 · testlog_26082215 §4.0): `--keep-up` 상주 서빙의 예산 선언이 만료된 채
#   서빙만 18시간째 돌고 있었다(`expires_epoch` 대비 **-43,231s = 12시간 전 만료**). `renew-budget`
#   서브커맨드는 2026-08-14 에 이미 있었지만 **호출하는 상주 루프가 없었다** — 안내문이
#   "연장은 이 명령으로" 라고 사람을 가리켰고, 사람은 18시간 동안 그 명령을 치지 않았다.
#   `workflow.md` §막힘 3분류의 **침묵 누락**이며, 처방 주체가 아키텍처에 없던 전형이다
#   (declare-budget 이 "호출자 0개" 였던 2026-08-14 결함과 같은 부류의 재발).
#
# 선언이 만료되면 워치독은 **선언된 바닥을 모르는 옛 규칙**으로 돌아간다. 상주 서빙은 잔량이 이미
#   낮으므로, 그 구간에서 무거운 요청 하나가 들어오면 정상 동작이 트립으로 읽힌다.
#   즉 만료 구간 = **무보호 구간**이고, 이 루프가 없애려는 것이 정확히 그 구간이다.
#
# ★ 컨테이너가 사라지면 **갱신을 멈추고 즉시 종료한다.** 서빙이 없는데 선언만 살아 있으면
#   다음 로드가 **남의 바닥**으로 무장한다 — teardown 의 `clear-budget` 이 막는 바로 그 상태를
#   이 루프가 되살리면 안 된다. 그래서 루프는 자기종료형이고, 종료 사유를 반드시 남긴다.
#
# ★ 만료된 선언은 **되살리지 않는다.** `renew-budget` 이 fail-closed 로 거부하며(그 판정의 소유자는
#   blackbox_session.py 다), 이 루프는 그 거부를 삼키지 않고 크게 실패한다 — 되살리려면 사람이
#   `declare-budget` 으로 바닥을 다시 산출해야 한다.
#
# 사용:
#   bash budget_renew_loop.sh --node-dir <docs/logs/<node_id>> --container <name> \
#        [--ttl-s <초>] [--interval-s <ttl/4>] [--once] [--self-test]
#
# 종료코드: 0=컨테이너 소멸로 정상 종료(또는 --once 성공) · 2=사용오류 · 5=갱신 실패(fail-loud)
set -uo pipefail

RL_TAG="[budget-renew]"

# 시각은 주입만 쓴다(blackbox_session 의 `--now` 규약). 여기서 `date -u` 를 읽는 것이 그 주입점이다 —
# 파이썬 판정기 쪽에 벽시계를 두지 않는 것이 규약의 요점이고, 셸 사이드카가 그 경계다.
rl_now(){ date -u +%FT%TZ; }

# 컨테이너 생존 프로브. **self-test 가 주입할 수 있게** 한 겹 감싼다 — 감싸지 않으면 이 루프는
# docker 없이는 한 줄도 검증할 수 없고, 검증되지 않은 사이드카는 없는 것과 같다.
rl_container_running(){   # $1=container name → 0=살아있음
    if [ -n "${BUDGET_RENEW_PROBE:-}" ]; then
        BUDGET_RENEW_TARGET="$1" bash -c "$BUDGET_RENEW_PROBE"
        return $?
    fi
    # ★ 2026-09-01 (audit_26090109 ⑧): 종전 형태는 `2>/dev/null` 로 docker 조회 실패를
    #   삼켜 **빈 출력 = 컨테이너 부재**로 접었다. 그러면 docker 데몬이 잠깐 흔들리기만 해도
    #   루프가 "서빙 끝났다"며 exit 0 하고, 예산 선언이 갱신되지 않아 TTL 만료와 함께
    #   워치독이 **옛 무조건-트립 규칙으로 복귀**한다 — 살아 있는 서빙 위에서.
    #   부재(1)와 판정 불가(2)를 가른다.
    local _out _rc
    _out="$(docker ps --filter "name=^${1}$" --filter status=running -q 2>&1)"; _rc=$?
    if [ "$_rc" -ne 0 ]; then
        echo "$RL_TAG READ-FAIL docker 조회 실패(rc=$_rc): $_out" >&2
        return 2                       # 판정 불가 — 부재와 다르다
    fi
    [ -n "$_out" ]
}

rl_usage(){
    cat <<'EOF'
사용법: budget_renew_loop.sh --node-dir <dir> --container <name> [옵션]

  --node-dir    docs/logs/<node_id> (예산 선언·events 가 사는 곳)
  --container   감시할 컨테이너 이름(정확 일치). 사라지면 루프가 스스로 끝난다
  --ttl-s       매 갱신이 미는 만료(미지정 시 blackbox_session 의 기본값 · 상한도 그쪽이 강제)
  --interval-s  갱신 주기(기본 ttl/4 · 최소 60). 만료보다 훨씬 짧아야 한다 —
                만료된 선언은 갱신되지 않는다(fail-closed)
  --once        한 번만 갱신하고 종료(배선 점검용)
  --self-test   docker·선언 없이 도는 회귀(주입 프로브 사용)
EOF
}

RL_NODE_DIR=""; RL_CONTAINER=""; RL_TTL_S=""; RL_INTERVAL_S=""; RL_ONCE=0; RL_SELFTEST=0
while [ $# -gt 0 ]; do
    case "$1" in
        --node-dir)   RL_NODE_DIR="${2:-}"; shift 2 ;;
        --container)  RL_CONTAINER="${2:-}"; shift 2 ;;
        --ttl-s)      RL_TTL_S="${2:-}"; shift 2 ;;
        --interval-s) RL_INTERVAL_S="${2:-}"; shift 2 ;;
        --once)       RL_ONCE=1; shift ;;
        --self-test)  RL_SELFTEST=1; shift ;;
        --help|-h)    rl_usage; exit 0 ;;
        *) echo "$RL_TAG FAIL: 알 수 없는 인자: $1" >&2; rl_usage >&2; exit 2 ;;
    esac
done

RL_HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RL_SESSION_PY="${BUDGET_RENEW_SESSION_PY:-$RL_HERE/blackbox_session.py}"

rl_renew_once(){   # → 0=갱신됨 · 1=실패(사유는 stdout/stderr 에 그대로)
    local out rc
    out="$(python3 "$RL_SESSION_PY" --node-dir "$RL_NODE_DIR" renew-budget \
                   --ttl-s "$RL_TTL_S" --now "$(rl_now)" 2>&1)"; rc=$?
    if [ $rc -ne 0 ]; then
        echo "$RL_TAG FAIL: renew-budget 거부(rc=$rc) — 삼키지 않는다:" >&2
        printf '%s\n' "$out" | sed 's/^/  /' >&2
        return 1
    fi
    printf '%s\n' "$out" | sed "s|^|$RL_TAG |"
    return 0
}

# ── self-test (docker·실 선언 불요 — 주입 프로브로 분기만 고정) ─────────────────────────
if [ "$RL_SELFTEST" = "1" ]; then
    rl_fail=0
    rl_chk(){ if [ "$2" = "$3" ]; then echo "PASS $1"; else echo "FAIL $1 — got=$2 want=$3"; rl_fail=1; fi; }

    BUDGET_RENEW_PROBE='exit 0' rl_container_running x && rl_r=0 || rl_r=1
    rl_chk "프로브 주입: 살아있음 → 0" "$rl_r" "0"
    BUDGET_RENEW_PROBE='exit 1' rl_container_running x && rl_r=0 || rl_r=1
    rl_chk "프로브 주입: 사라짐 → 1" "$rl_r" "1"

    # ★ 컨테이너가 없으면 **갱신을 시도조차 하지 않고** 종료한다(선언이 서빙보다 오래 살면 안 된다).
    rl_tmp="$(mktemp -d)"; : >"$rl_tmp/renew_called"
    # 픽스처 TTL — 이 자리들은 TTL 축을 시험하지 않으므로 값을 **명시 선언**한다(국소 상수 · 정당).
    RL_FIXTURE_TTL_S=600
    rl_out="$(BUDGET_RENEW_PROBE='exit 1' \
              BUDGET_RENEW_SESSION_PY="$rl_tmp/should_not_run.py" \
              bash "${BASH_SOURCE[0]}" --node-dir "$rl_tmp" --container ghost \
                   --ttl-s "$RL_FIXTURE_TTL_S" --interval-s 60 2>&1)"; rl_r=$?
    rl_chk "컨테이너 부재 → 갱신 없이 exit 0" "$rl_r" "0"
    case "$rl_out" in *"컨테이너 부재"*) rl_r=0 ;; *) rl_r=1 ;; esac
    rl_chk "종료 사유를 남긴다(침묵 종료 ✗)" "$rl_r" "0"

    # ★ audit ⑧ 회귀: docker 조회 실패(판정 불가)를 **부재로 접지 않는다**.
    #   probe 가 2 를 내면 루프는 종료하지 않아야 한다(--once 아님 · 짧게 돌려 확인).
    BUDGET_RENEW_PROBE='exit 2' rl_container_running x; rl_r=$?
    rl_chk "프로브 주입: 판정 불가 → 2(부재 1 과 구별)" "$rl_r" "2"
    printf '%s\n' 'print("ok")' >"$rl_tmp/ok2.py"
    rl_out="$(BUDGET_RENEW_PROBE='exit 2' BUDGET_RENEW_SESSION_PY="$rl_tmp/ok2.py" \
              timeout -s KILL 3 bash "${BASH_SOURCE[0]}" --node-dir "$rl_tmp" --container c \
                   --ttl-s "$RL_FIXTURE_TTL_S" --interval-s 1 2>&1)"; rl_r=$?
    case "$rl_out" in *"판정 불가"*) rl_r2=0 ;; *) rl_r2=1 ;; esac
    rl_chk "판정 불가 → 종료하지 않고 갱신 계속(사유 기록)" "$rl_r2" "0"
    case "$rl_out" in *"컨테이너 부재"*) rl_r2=1 ;; *) rl_r2=0 ;; esac
    rl_chk "판정 불가를 '컨테이너 부재'로 적지 않는다" "$rl_r2" "0"

    # 갱신 실패는 크게 죽는다(만료된 선언을 조용히 되살리지 않는다).
    printf '%s\n' 'import sys; sys.exit(1)' >"$rl_tmp/fail.py"
    rl_out="$(BUDGET_RENEW_PROBE='exit 0' BUDGET_RENEW_SESSION_PY="$rl_tmp/fail.py" \
              bash "${BASH_SOURCE[0]}" --node-dir "$rl_tmp" --container c \
                   --ttl-s "$RL_FIXTURE_TTL_S" --once 2>&1)"; rl_r=$?
    rl_chk "갱신 거부 → exit 5(fail-loud)" "$rl_r" "5"

    printf '%s\n' 'print("ok")' >"$rl_tmp/ok.py"
    BUDGET_RENEW_PROBE='exit 0' BUDGET_RENEW_SESSION_PY="$rl_tmp/ok.py" \
        bash "${BASH_SOURCE[0]}" --node-dir "$rl_tmp" --container c --ttl-s "$RL_FIXTURE_TTL_S" \
        --once >/dev/null 2>&1 && rl_r=0 || rl_r=1
    rl_chk "--once 성공 → exit 0" "$rl_r" "0"

    # ★ TTL 기본값 파생(2026-09-05 · G-B2): --ttl-s 를 주지 않으면 **단일 소유자**에게 묻는다.
    #   여기서 기대값도 손으로 적지 않고 같은 소유자에게 물어 대조한다(두 자리가 갈라지지 않게).
    rl_owner_ttl="$(python3 "$RL_HERE/blackbox_session.py" --node-dir "$rl_tmp" \
                    budget-defaults --field ttl_s 2>/dev/null)"
    rl_out="$(BUDGET_RENEW_PROBE='exit 1' bash "${BASH_SOURCE[0]}" \
              --node-dir "$rl_tmp" --container ghost 2>&1)"
    case "$rl_out" in *"ttl=${rl_owner_ttl}s"*) rl_r=0 ;; *) rl_r=1 ;; esac
    rl_chk "★ --ttl-s 미지정 → 단일 소유자(blackbox_session)의 기본값을 쓴다" "$rl_r" "0"

    rl_out="$(bash "${BASH_SOURCE[0]}" --container c 2>&1)"; rl_r=$?
    rl_chk "--node-dir 누락 → 사용오류 2" "$rl_r" "2"
    # 주기 클램프는 순수 산술이라 시작줄의 해소값으로 고정한다(루프를 돌리지 않는다).
    rl_out="$(BUDGET_RENEW_PROBE='exit 1' bash "${BASH_SOURCE[0]}" \
              --node-dir "$rl_tmp" --container ghost --ttl-s 7200 --interval-s 1 2>&1)"
    case "$rl_out" in *"interval=60s"*) rl_r=0 ;; *) rl_r=1 ;; esac
    rl_chk "interval 하한 60s 로 클램프" "$rl_r" "0"
    rl_out="$(BUDGET_RENEW_PROBE='exit 1' bash "${BASH_SOURCE[0]}" \
              --node-dir "$rl_tmp" --container ghost --ttl-s 7200 --interval-s 99999 2>&1)"
    case "$rl_out" in *"interval=3600s"*) rl_r=0 ;; *) rl_r=1 ;; esac
    rl_chk "★ interval 상한 = TTL/2 (만료 후 갱신은 거부되므로 여유 필수)" "$rl_r" "0"
    rl_out="$(BUDGET_RENEW_PROBE='exit 1' bash "${BASH_SOURCE[0]}" \
              --node-dir "$rl_tmp" --container ghost --ttl-s 7200 2>&1)"
    case "$rl_out" in *"interval=1800s"*) rl_r=0 ;; *) rl_r=1 ;; esac
    rl_chk "기본 interval = TTL/4" "$rl_r" "0"

    rm -rf "$rl_tmp"
    echo "self-test: $([ $rl_fail = 0 ] && echo PASS || echo FAIL)"
    exit $rl_fail
fi

# ── 인자 검증 (fail-closed — 빈 값으로 도는 사이드카는 아무것도 지키지 않는다) ──
[ -n "$RL_NODE_DIR" ]  || { echo "$RL_TAG FAIL: --node-dir 필수" >&2; rl_usage >&2; exit 2; }
[ -n "$RL_CONTAINER" ] || { echo "$RL_TAG FAIL: --container 필수(감시 대상이 없으면 종료 조건도 없다)" >&2; exit 2; }
# --ttl-s 미지정이면 **단일 소유자**의 기본값을 읽는다(여기에 7200 을 적지 않는다 · G-B2).
if [ -z "$RL_TTL_S" ]; then
    RL_TTL_S="$(python3 "$RL_SESSION_PY" --node-dir "${RL_NODE_DIR:-.}" budget-defaults --field ttl_s 2>/dev/null || echo)"
fi
case "$RL_TTL_S" in ''|*[!0-9]*) echo "$RL_TAG FAIL: --ttl-s 는 양의 정수(또는 blackbox_session 기본값 판독 실패)" >&2; exit 2 ;; esac
[ "$RL_TTL_S" -gt 0 ] || { echo "$RL_TAG FAIL: --ttl-s 는 양의 정수" >&2; exit 2; }
if [ -z "$RL_INTERVAL_S" ]; then RL_INTERVAL_S=$(( RL_TTL_S / 4 )); fi
case "$RL_INTERVAL_S" in ''|*[!0-9]*) echo "$RL_TAG FAIL: --interval-s 는 양의 정수" >&2; exit 2 ;; esac
# 하한 60s: 1초 주기 워치독과 달리 갱신은 파일쓰기+이벤트라 과빈도가 events 를 오염시킨다.
[ "$RL_INTERVAL_S" -ge 60 ] || RL_INTERVAL_S=60
# 상한 = TTL 의 절반. 주기가 만료에 가까우면 한 번만 늦어도 만료되고, 만료된 선언은 갱신되지 않는다.
if [ "$RL_INTERVAL_S" -gt $(( RL_TTL_S / 2 )) ]; then
    RL_INTERVAL_S=$(( RL_TTL_S / 2 ))
    echo "$RL_TAG interval 을 TTL 의 절반($RL_INTERVAL_S s)으로 낮춘다 — 만료 후 갱신은 거부되므로 여유가 필요하다."
fi

echo "$RL_TAG 시작 — node_dir=$RL_NODE_DIR container=$RL_CONTAINER ttl=${RL_TTL_S}s interval=${RL_INTERVAL_S}s"

if [ "$RL_ONCE" = "1" ]; then
    rl_renew_once || exit 5
    echo "$RL_TAG --once 완료."
    exit 0
fi

while :; do
    rl_container_running "$RL_CONTAINER"; rl_cr=$?
    if [ "$rl_cr" -eq 1 ]; then
        echo "$RL_TAG 종료 — 컨테이너 부재($RL_CONTAINER). 서빙이 없으면 선언도 유지하지 않는다."
        echo "$RL_TAG ⚠ 선언 자체의 회수(clear-budget)는 teardown 진입점이 한다 — 이 루프는 갱신만 멈춘다."
        exit 0
    fi
    if [ "$rl_cr" -ge 2 ]; then
        # 판정 불가에서의 안전 기본값은 **갱신 계속**이다. 갱신을 멈추면 선언이 만료되고
        # 워치독이 옛 규칙으로 복귀해 정상 로드를 사살할 수 있다(2026-08-01 실화 계열).
        # 절대 바닥(hard_floor)은 선언과 무관하게 항상 무장하므로 호스트는 여전히 보호된다.
        echo "$RL_TAG ⚠ 컨테이너 생사 판정 불가 — 갱신을 계속한다(선언 만료가 더 위험하다). 부재로 접지 않는다."
    fi
    rl_renew_once || exit 5
    sleep "$RL_INTERVAL_S"
done
