#!/bin/bash
# 멀티노드 2노드 Ray 분산 서빙 + multi-smoke (검증된 오케스트레이션, 근거: docs/testlog/testlog_260607_7).
#
# master = 메인(로컬, Ray head + vllm serve), slave = 서브(SSH, Ray worker). 둘 다 동일 코드, IP로 역할 자동.
# 준비 판정은 **엔드포인트 health(http 200)** 폴링 — master 로그의 "Application startup complete" 는
# 조기 컴포넌트에서도 떠 거짓양성이 나므로 쓰지 않는다(testlog_260607_7 §3 학습).
#
# 사용: bash multinode_serve_smoke.sh <config_name> [--build] [--build-only] [--keep-up] [--down] [--no-watchdog] [--no-budget]
#   --build   : 서빙 전 양 노드 이미지 빌드(병렬, 최소병렬 원칙)
#   --build-only : 양 노드 빌드만 하고 **종료**(서빙·스모크 없음). 로드-전 RAM 게이트를 타지 않는다.
#                  ★ 2026-08-15 신설(R3 포크 핀이 노출). 그 게이트는 **가중치 로드**의 전제조건인데
#                  (`ckpt÷tp + floor`) 빌드는 가중치를 로드하지 않는다 — 즉 빌드 앞에 놓인 그 게이트는
#                  "정상 차단"이 아니라 **주체를 잘못 겨눈 게이트**다(workflow.md 막힘 3분류).
#                  실증: R2 상주(잔여 14.5GiB) 상태에서 R3 를 `--build` 하면 required=89,818MiB 를
#                  못 넘겨 **빌드에 도달하기 전에** exit 3 로 죽는다. 빌드는 그 메모리가 불요하다.
#                  ⚠ 대신 빌드가 쓰는 것은 **컴파일러 메모리**다 — 그건 BUILD_JOBS 가 다스린다(아래).
#   --keep-up : 스모크 후 컨테이너 유지(기본은 정리/down — 워치독도 함께 유지)
#   --down    : **상주 서빙을 내리기만 한다**(기동·스모크 없음). `--keep-up` 으로 남긴 서빙의 회수 경로.
#               ★ 2026-08-16 신설. 그전까지 teardown 은 이 스크립트 **자기 실행 안에서만** 도달 가능했고
#               (`KEEP != 1` 분기), `--keep-up` 상주분에 대한 안내는 워치독 `kill <pid>` 뿐이었다. 그래서
#               사람도 에이전트도 `docker compose down` 을 직접 치게 되는데, 그러면 5단계(master·slave
#               down / 워치독 정지 / drop-caches / **예산 회수**) 중 뒤 셋을 조용히 빠뜨린다.
#               실증(2026-08-15): 그렇게 내린 뒤 재기동에서 stale 선언 때문에 `clear`+`declare` 가
#               같은 초에 실행돼 1초 주기 워치독이 `none` 을 관측 못 함 → `declare_not_honored` 로
#               진입 차단. 즉 **회수 경로의 부재가 다음 기동을 막았다**(workflow.md 막힘 3분류 = 침묵 누락).
#               워치독 PID 는 다른 프로세스가 띄웠으므로 모른다 → `reap_stale_watchdogs`(argv 위치 대조)로 회수한다.
#   --no-watchdog : 협역 워치독 사이드 기동 생략(plan_26071019 §2.3 — 진단 시)
#   --no-budget   : 서빙 예산 **선언 생략**(무보호 진입 — plan_26081415 C3 탈출구).
#                   선언 없이 로드하면 ETA 워치독이 현행 규칙 그대로 돌아 대형 로드를 사살할 수 있다.
#                   생략 사실은 로그와 `budget_skipped` 이벤트로 **반드시** 남는다(침묵 금지).
# 종료코드: 0=스모크 통과, 2=미준비/스모크 실패, 3=NAS/설정 실패(7=RAM 게이트 거부 포함 시 3으로 수렴),
#           4=예산 선언 실패(진입 차단 — plan_26081415 C3 실패정책 ㄴ. 로드는 0초도 시작하지 않았다).
set -uo pipefail

CONFIG="${1:?사용: multinode_serve_smoke.sh <config_name> [--build] [--keep-up] [--down] [--no-watchdog] [--no-budget]}"; shift || true
BUILD=0; KEEP=0; WATCHDOG=1; BUDGET=1; BUILD_ONLY=0; DOWN=0
for a in "$@"; do [ "$a" = "--build" ] && BUILD=1; [ "$a" = "--keep-up" ] && KEEP=1; [ "$a" = "--down" ] && DOWN=1; [ "$a" = "--no-watchdog" ] && WATCHDOG=0; [ "$a" = "--no-budget" ] && BUDGET=0; [ "$a" = "--build-only" ] && { BUILD=1; BUILD_ONLY=1; }; done
# --down 은 "내리기만" 이므로 기동 계열 플래그와 동시에 오면 의도가 모순이다. 조용히 한쪽을 이기게
# 두지 않고 fail-closed 한다 — 어느 쪽이 이겼는지 모르는 채 컨테이너가 뜨거나 내려가면 안 된다.
if [ "$DOWN" = "1" ]; then
  [ "$KEEP" = "1" ]       && { echo "[mn] FAIL: --down 과 --keep-up 은 함께 쓸 수 없다(내리기 vs 유지)."; exit 3; }
  # BUILD_ONLY 를 **먼저** 본다: --build-only 는 BUILD=1 도 세우므로, 순서를 뒤집으면 주지도 않은
  # --build 를 지목해 사용자가 자기 명령줄에 없는 플래그를 찾게 된다(진단은 원인을 정확히 가리켜야 한다).
  [ "$BUILD_ONLY" = "1" ] && { echo "[mn] FAIL: --down 과 --build-only 는 함께 쓸 수 없다(내리기 vs 빌드)."; exit 3; }
  [ "$BUILD" = "1" ]      && { echo "[mn] FAIL: --down 과 --build 는 함께 쓸 수 없다(내리기 vs 빌드)."; exit 3; }
  WATCHDOG=0; BUDGET=0     # 내리는 경로에서는 새 워치독·새 선언을 만들지 않는다(회수만 한다).
fi

# ── READY_MAX 단일 정의 (2026-08-16) ────────────────────────────────────────────────────────
#   기본값 180 이 파생·안내·루프 4곳에 `${READY_MAX:-180}` 로 손으로 적혀 있었다. 같은 개념이 두 곳
#   이상에 적힌 값 = 4종 안티패턴의 **매직넘버 결함**(workflow.md §결정론 규율 판정표). 여기서 한 번
#   해소하고 이후로는 `$READY_MAX` 만 참조한다 — 한 곳만 고치면 나머지가 갈리는 상태를 없앤다.
#   2026-09-05(G-B6): 기본값 180 **삭제**. 로드 시간은 모델·HW 의 함수이지 스크립트 상수가
#   아니다 — 180 은 이 캠페인의 작은 모델에서 나온 수이고, hy3 는 600 이 필요했다(실측).
#   기본값이 남아 있으면 큰 모델이 "타임아웃"으로 오판되고 그 오판이 서빙 실패로 기록된다.
#   선언 경로: `READY_MAX=<폴링 횟수>`(env · ×5s) 또는 활성 캠페인 `budgets.ready_max_seconds`(초). 모르면 직전 런의 engine 로그에서 READY 까지 걸린 초를
#   재고 여유를 얹어라 — 단일노드 정본(single_serve_up.sh)은 실측 근거로 600 을 쓴다.
# ── 캠페인 예산 선언 읽기 (2026-09-29 · plan_26092919 P3) ─────────────────────────────────────
#   우선순위는 공통층 규칙 그대로 **환경 주입 > 선언 > (리터럴 없음 → fail-loud)**. 종전에는 활성 캠페인이
#   `budgets.ready_max_seconds`·`smoke_budget_overhead_mib` 를 선언해도 이 스크립트가 읽지 않아, 선언과 같은
#   값을 env 로 손으로 다시 넘겨야 했다(2026-09-29 camp-26092919: rc 2·rc 4 로 두 번 막힘 — 같은 개념이
#   두 자리). 활성 캠페인이 없거나(_bootstrap) 값이 0(뼈대 미기입)이면 아무것도 내지 않는다 — 그러면
#   아래의 '선언되지 않았다' fail-loud 가 그대로 선다(기본값을 만들지 않는다).
_campaign_budget() {   # $1 = budgets 키 → 활성 캠페인 선언의 양의 정수만 stdout
  python3 - "$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.." && pwd)" "$1" <<'PY'
import json, os, sys
repo, key = sys.argv[1], sys.argv[2]
try:
    with open(os.path.join(repo, "campaigns", "ACTIVE"), encoding="utf-8") as fh:
        camp = fh.read().strip()
except OSError:
    sys.exit(0)
if not camp or camp == "_bootstrap":
    sys.exit(0)
path = os.path.join(repo, "campaigns", camp, "campaign.yaml")
try:
    with open(path, encoding="utf-8") as fh:
        val = (json.load(fh).get("budgets") or {}).get(key)
except (OSError, ValueError) as exc:
    print(f"[mn] ⚠ 활성 캠페인 선언을 읽지 못했다({path}: {exc}) — 선언값 없이 진행(아래 게이트가 판정)",
          file=sys.stderr)
    sys.exit(0)
if isinstance(val, int) and not isinstance(val, bool) and val > 0:
    print(val)
PY
}

READY_MAX="${READY_MAX:-}"
READY_MAX_SOURCE="env READY_MAX"
if [ -z "$READY_MAX" ] && [ "$DOWN" != "1" ]; then
  _rms="$(_campaign_budget ready_max_seconds)"
  if [ -n "$_rms" ]; then
    READY_MAX=$(( (_rms + 4) / 5 ))      # READY_MAX 는 폴링 횟수(×5s) — 선언은 초다
    READY_MAX_SOURCE="캠페인 budgets.ready_max_seconds=${_rms}s → ${READY_MAX}회×5s"
  fi
fi
# 내리기 경로(--down)는 로드를 기다리지 않으므로 선언을 요구하지 않는다 — 게이트는 그것이 지키는
# 일이 실제로 일어나는 경로에만 선다(무관한 경로를 막으면 사람이 게이트를 우회하는 법을 배운다).
if [ "$DOWN" = "1" ] && [ -z "$READY_MAX" ]; then READY_MAX=0; fi
case "$READY_MAX" in
  ''|*[!0-9]*)
    echo "[mn] FAIL: READY_MAX 가 선언되지 않았다(기본값 없음 · 2026-09-05 G-B6)." >&2
    echo "     왜: 로드 시간은 모델·HW 의 함수다. 옛 기본 180 은 큰 모델을 타임아웃으로 오판했다." >&2
    echo "     어떻게: READY_MAX=<폴링 횟수(×5s)> env 또는 활성 캠페인 budgets.ready_max_seconds(초)를 선언하라(직전 런의 READY 도달 시간 + 여유)." >&2
    exit 2;;
esac
READY_WINDOW_S=$(( READY_MAX * 5 ))
[ "$DOWN" = "1" ] || echo "[mn] READY_MAX 출처: $READY_MAX_SOURCE"

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO"
EF="output/multi/envs/.env.${CONFIG}"   # 산출물 통로 분리(plan_26062312): compose·env 는 output/multi/ 아래
[ -f "$EF" ] || { echo "[mn] FAIL: $EF 없음"; exit 3; }
EFC="output/multi/envs/.env.cluster"    # Ray 클러스터-배포 env(Band2, S6 env-split). compose 보간(${MASTER_HOST_IP}·${SLAVE_HOST_IP}·${RAY_PORT})에 필요.
[ -f "$EFC" ] || { echo "[mn] FAIL: $EFC 없음 — 'render_dockerfile.py --cluster-envfile --topology multi --manifest output/multi/manifest.yaml -o $EFC' 선행(S6)"; exit 3; }
# 통로 self-containment 전제(plan_26062321 I1/I2): 러너 스크립트가 통로에 materialize 됐는지 fail-loud.
for s in serve_runner.sh debug-init.sh; do
  [ -f "output/multi/configs/$s" ] || { echo "[mn] FAIL: 통로 미완결 — output/multi/configs/$s 부재. 먼저 'render_dockerfile.py --materialize-configs --topology multi' 실행(후 sync_to_sub.sh --apply)"; exit 3; }
done
val(){ grep -E "^$1=" "$EF" | head -1 | cut -d= -f2-; }
MC=$(val MASTER_CONTAINER_NAME); PORT=$(val SERVING_PORT)
MODEL=$(val SERVING_MODEL_NAME); SLAVE_IP=$(val SLAVE_HOST_IP)

# ── slave 전달 집합 — 실제 compose 참조에서 **파생**한다(2026-09-21 · plan_26092119 §4.6) ──────────
#   slave 는 `--env-file $EFC`(Band2)만 받으므로 셀 env(Band3)의 값은 ssh 명령 앞 env prefix 로만 닿는다.
#   그 prefix 가 compose build-arg·보간 변수와 **따로 손으로** 자라는 동안 같은 부류의 침묵 누락이 이어졌다:
#     BUILD_DOCKERFILE(07-24 Solar-Open2) → VLLM_PRETEND_VERSION(08-02) → SM12X_PORT·RAY_PORT·
#     SLAVE_CONTAINER_NAME(08-14) → SRC_DEPS_AUTHORITY·BUILD_JOBS(08-15) → VLLM_VERSION(09-05) → PLE(09-09).
#   빠질 때마다 "마스터만 변종 · slave 는 compose 기본값" 이 됐다(같은 IMAGE_TAG, 다른 내용).
#   이제 목록은 없다: slave_forward.py 가 **이 스크립트가 실제로 쓰는** compose 의 ${VAR} 참조에서 파생하고
#   (새 build-arg 는 코드 수정 없이 전달), 각 키의 날짜 박힌 사고 이력은 slave_forward.WHY 가 데이터로 든다
#   (hint sub_recipe 가 같은 함수·같은 이력을 싣는다 — 두 벌 ✗).
#   의미(X11): 첫 일치 · 빈 값은 키째 생략(부재 = stock — 빈 대입은 compose 기본값을 빈 문자열로 덮는다) ·
#   인용 없는 원격 셸 prefix 에 안전하지 않은 값은 **fail-closed**(exit 3).
#   CONFIG_FILE 은 영구 제외 — slave Band2-only(plan_2026062811_2 · policy:MODEL_TRIPLET_NO_SUB_PROPAGATION).
#   ⚠ 파생이 목록 실패를 줄여도 **결과 대조는 계속한다**(2026-09-05 "리스트를 늘리는 대신 결과를 대조한다") —
#     빌드 후 이미지 안 대조와 serve 시점 attestation v2 가 남은 갈림을 잡는다.
SF="$SDIR/slave_forward.py"
[ -f "$SF" ] || { echo "[mn] FAIL: $SF 없음 — slave 전달 집합의 정본 부재(손목록으로 되돌리지 않는다)"; exit 3; }
PENV_FILE="output/multi/.env"
# 결함#2b 배선의 부재는 **소리 낸다**(audit_26090115: `--materialize-env` 를 빠뜨리면 옛 스모크는 MOUNTVARS="" 로 조용히
#   진행했고 compose 기본 마운트가 모델 부재로 이어졌다). 차단은 새 게이트라 사람 결정 몫 — 여기서는 경고만 한다.
[ -f "$PENV_FILE" ] || echo "[mn] ⚠ $PENV_FILE 부재 — 마운트 경로(NAS/QUANT/TIKTOKEN/PLE)가 셀 env 또는 compose 기본값으로 폴백한다(render_dockerfile.py --materialize-env --topology multi 로 생성)." >&2
COMPOSE_MULTI="output/multi/docker-compose.yaml"
SFA=(--cell-env "$EF" --project-env "$PENV_FILE" --compose "$COMPOSE_MULTI" --cluster-env "$EFC")
IMG=$(val IMAGE_TAG); BDF=$(val BUILD_DOCKERFILE)
# 이미지 정체성 + 클러스터 랑데부(RAY_PORT·SLAVE_CONTAINER_NAME 등 Band2 키의 셀 덮어쓰기)를 한 prefix 로 — 옛
#   `SLAVE_IMGVARS="$SLAVE_IMGVARS $SLAVE_CLUSTERVARS"` 합성과 같은 구성이다(build·up·down 세 호출부가 쓴다).
SLAVE_IMGVARS="$(python3 "$SF" prefix --group image_identity,cluster "${SFA[@]}")" || { echo "[mn] FAIL: slave_forward(image_identity,cluster) 파생 실패 — 위 사유를 고쳐라"; exit 3; }
SLAVE_BUILDVARS="$(python3 "$SF" prefix --group build_tuning "${SFA[@]}")" || { echo "[mn] FAIL: slave_forward(build_tuning) 파생 실패"; exit 3; }
MOUNTVARS="$(python3 "$SF" prefix --group mount "${SFA[@]}")" || { echo "[mn] FAIL: slave_forward(mount) 파생 실패"; exit 3; }
PLEVARS="$(python3 "$SF" prefix --group serve_env "${SFA[@]}")" || { echo "[mn] FAIL: slave_forward(serve_env) 파생 실패"; exit 3; }
BJOBS=$(val BUILD_JOBS)                 # 로컬 표시용(빌드 병렬도 안내) — 전달은 위 SLAVE_BUILDVARS 가 한다
SLVC=$(val SLAVE_CONTAINER_NAME)        # slave 워치독 필터·로그 회수·갱신 루프가 쓰는 실제 이름

# ── 서브 식별자/경로 해소(단일계약): env-file > manifest nodes[sub] > 폴백. 옛 고정 서브경로 하드코딩 제거 ──
MANIFEST_MF="$REPO/output/multi/manifest.yaml"
_mf_node() {  # $1=role $2=field → nodes[role=$1].field (role 정확매칭 — 'subordinate' 등 접두 오인 방지)
  [ -f "$MANIFEST_MF" ] || return 1
  awk -v role="$1" -v field="$2" '
    $0 ~ "^[[:space:]]*-[[:space:]]*role:[[:space:]]*" role "([[:space:]]|$|#)" { in_n=1; next }
    /^[[:space:]]*-[[:space:]]*role:/             { in_n=0 }
    in_n && $0 ~ "^[[:space:]]*" field ":" { sub("^[[:space:]]*" field ":[[:space:]]*",""); sub(/[[:space:]]*#.*/,""); gsub(/[ "\r]/,""); print; exit }
  ' "$MANIFEST_MF"
}
_mf_sub() { _mf_node sub "$1"; }   # 기존 호출부 보존(얇은 래퍼)

# ── node-identity (plan_26081514 §3 스킴 R · SKILL.md §2.7.6) ────────────────────────────────
#   로그 경로 성분은 **manifest nodes[].role** 이다. 예전엔 메인은 `$(hostname)`, 서브는
#   `ssh <sub> hostname` **4회 왕복**으로 파생했다. 그 왕복은 (a) 매 스모크마다 원격 4회이고
#   (b) **실패하면 관측이 조용히 빠졌다**(옛 문구: "차단 이벤트 미기록"). 즉 서브가 잠깐
#   도달 불가일 때 "그날 아무 일도 없었다"와 구분되지 않는 기록이 남았다. role 은 manifest 에서
#   읽으므로 왕복이 **0회**가 되고, 해소 실패는 진입 시점에 fail-loud 로 드러난다.
#   ⚠ 이 값은 manifest 에 그 role 이 **실재할 때만** 나온다 — 손으로 적은 리터럴이 아니다.
_mf_node_id() {  # $1=role → 검증된 슬러그 · 1=부재
  [ -f "$MANIFEST_MF" ] || return 1
  grep -Eq "^[[:space:]]*-[[:space:]]*role:[[:space:]]*$1([[:space:]]|#|$)" "$MANIFEST_MF" || return 1
  printf '%s' "$1"
}
MAIN_NODE_ID="${MAIN_NODE_ID:-$(_mf_node_id main)}"
SUB_NODE_ID="${SUB_NODE_ID:-$(_mf_node_id sub)}"
for _r in MAIN_NODE_ID SUB_NODE_ID; do
  eval "_v=\${$_r}"
  [ -n "$_v" ] || { echo "[mn] FAIL: $_r 해소 실패 — $MANIFEST_MF 의 nodes[].role 을 확인하라(hostname 파생 폐지)."; exit 3; }
  printf '%s' "$_v" | grep -Eq '^[a-z][a-z0-9-]{0,31}$' \
    || { echo "[mn] FAIL: $_r='$_v' 가 node_id 스킴 위반이다."; exit 3; }
done
unset _r _v
SLAVE_IP="${SLAVE_IP:-$(_mf_sub host)}"                                              # env-file > manifest
SSH_USER="${SSH_USER:-$(val SSH_USER)}"; SSH_USER="${SSH_USER:-$(_mf_sub ssh_user)}"; SSH_USER="${SSH_USER:-$(id -un)}"  # env-file > manifest > 현재 사용자
SUB_HOST="${SUB_HOST:-${SSH_USER}@${SLAVE_IP}}"
SSH="ssh -o BatchMode=yes -o ConnectTimeout=8"
SUB_WORK_DIR="${SUB_WORK_DIR:-$(_mf_sub work_dir)}"; SUB_WORK_DIR="${SUB_WORK_DIR:-$REPO}"  # env > manifest > 메인 REPO(R2 기본값=동일)
case "$SUB_WORK_DIR" in *[[:space:]]*) echo "[mn] FAIL: SUB_WORK_DIR 공백 — 원격 cd 임베드 불가: '$SUB_WORK_DIR'"; exit 3;; esac
SUB_CD="cd $SUB_WORK_DIR &&"   # bash -lc '...' 단일인용 컨텍스트 임베드 — 무공백 보장(위 가드)
echo "[mn] config=$CONFIG master=$MC port=$PORT model=$MODEL sub=$SUB_HOST sub_work_dir=$SUB_WORK_DIR"

# ── NAS pre-flight + 로드-전 RAM 게이트(메인) — 다운로드 금지 · plan_26071019 §2.6 ──
#   rc 구분(2=모델부재 / 7=RAM게이트 거부 / 3=설정) — exit 7 을 '모델 부재·다운로드 금지'로 오귀속 금지.
#   ⚠ --build-only 는 이 블록 전체를 건너뛴다: 이 게이트들은 **가중치 로드**의 전제조건이고
#     (NAS 존재·spec 레이아웃·`ckpt÷tp+floor` RAM) 빌드는 그중 무엇도 요구하지 않는다.
#     우회가 아니라 **주체 정합**이다 — 로드할 때는 그대로 전량 강제된다(아래 serve 경로 불변).
#   ⚠ --down 도 같은 이유로 건너뛴다. 오히려 더 강한 사례다: 내리려는 시점에는 그 서빙이 메모리를
#     쥐고 있으므로 RAM 게이트가 **거의 항상** 거부한다 — 즉 "내리기 위해 먼저 내려야 하는" 교착이
#     된다. 게이트가 겨눌 주체는 로드이지 회수가 아니다(workflow.md 막힘 3분류: 정상 차단 아님).
if [ "$BUILD_ONLY" != "1" ] && [ "$DOWN" != "1" ]; then
NAS_OUT=$(python3 "$SDIR/check_smoke_model.py" "$CONFIG" --repo "$REPO" --topology multi --emit-gate-params 2>&1); NAS_RC=$?
printf '%s\n' "$NAS_OUT"
case "$NAS_RC" in
  0) : ;;
  7) echo "[mn] STOP: 로드-전 RAM 게이트 거부(메인) — 모델은 실재하나 가용 RAM 부족. 잔존 컨테이너/페이지캐시 정리 후 재시도(§모델 확보 결정트리 아님 — 다운로드 불요)"; exit 3 ;;
  2) echo "[mn] STOP: 스모크 모델 부재 — 다운로드 금지, 중단"; exit 3 ;;
  *) echo "[mn] STOP: NAS/설정 확인 실패(rc=$NAS_RC)"; exit 3 ;;
esac

# ── 슬레이브 노드 동일-문턱 RAM 게이트(예방 대칭 — 하드다운 #2=DS4 serve#1 '서브'였음 · §2.6) ──
#   메인이 emit 한 required_mib(같은 NAS·같은 ckpt÷TP)를 슬레이브 /proc/meminfo 에 비교. 슬레이브는
#   config/게이트 모듈 의존 없이 순수 MemAvailable 만 필요(부족 시 drop-caches 1회 재측정 후 판정).
REQ_MIB=$(printf '%s\n' "$NAS_OUT" | grep -oE 'required_mib=[0-9]+' | head -1 | cut -d= -f2)
if [ -n "$REQ_MIB" ]; then
  _slave_avail(){ $SSH "$SUB_HOST" "awk '/MemAvailable:/{print int(\$2/1024)}' /proc/meminfo" 2>/dev/null; }
  SLAVE_AVAIL=$(_slave_avail)
  if [ -n "$SLAVE_AVAIL" ] && [ "$SLAVE_AVAIL" -lt "$REQ_MIB" ]; then
    $SSH "$SUB_HOST" "[ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches" >/dev/null 2>&1
    SLAVE_AVAIL=$(_slave_avail)   # drop 후 재측정
  fi
  if [ -z "$SLAVE_AVAIL" ]; then
    echo "[mn] ⚠ 슬레이브 MemAvailable 조회 실패 — 슬레이브 게이트 생략(상시 워치독 층만 커버)"
  elif [ "$SLAVE_AVAIL" -lt "$REQ_MIB" ]; then
    echo "[mn] STOP: 슬레이브 로드-전 RAM 게이트 거부 — MemAvailable=${SLAVE_AVAIL}MiB < required=${REQ_MIB}MiB. 슬레이브 잔존 컨테이너/페이지캐시 정리 후 재시도(하드다운 #2 서브노드 예방)"; exit 3
  else
    echo "[mn] 슬레이브 RAM-gate PASS: MemAvailable=${SLAVE_AVAIL}MiB ≥ required=${REQ_MIB}MiB"
  fi
fi
else
  # 어느 플래그가 이 생략을 일으켰는지 **그 플래그 이름으로** 말한다. 하드코딩된 `--build-only` 는
  # --down 진입 시 주지도 않은 플래그를 지목해 사용자가 자기 명령줄에 없는 것을 찾게 만든다
  # (2026-08-16 --down 첫 실행에서 실제로 관측). 생략 사유도 경로마다 다르다.
  if [ "$DOWN" = "1" ]; then
    echo "[mn] --down: 로드-전 게이트(NAS·spec·RAM) 생략 — 회수는 가중치를 로드하지 않는다(게이트가 겨눌 주체는 로드다)."
  else
    echo "[mn] --build-only: 로드-전 게이트(NAS·spec·RAM) 생략 — 빌드는 가중치를 로드하지 않는다."
  fi
fi

# ── 빌드 병렬도 전달(2026-08-15 신설) ────────────────────────────────────────
#   BUILD_JOBS 는 **이미지 정체성이 아니다**(같은 산출물, 다른 병렬도) — 그래서 SLAVE_IMGVARS 가
#   아니라 별도 그룹(slave_forward `build_tuning` · NON_IDENTITY_BUILD_ARGS)으로 넘긴다. 다만 **양 노드에
#   똑같이** 가야 한다: 슬레이브만 기본값 16 으로 컴파일하면 거기서 OOM 이 난다(하드다운 #2 가 서브였다).
#   마스터는 `--env-file $EF` 로 자동 획득하므로 명시 전달은 슬레이브 몫이다(값은 위 SLAVE_BUILDVARS).

# ── 캠페인 진행표 writer(2026-09-21 · plan_26092119 §4.8 ①② · 코드맵 K7) ──────────────────────────
#   멀티에는 build·serve 진행표를 쓰는 손이 **없었다**(싱글은 single_serve_up.sh 가 serve 를 쓴다). 그래서
#   hint 발행 자격(관측: serve proof)을 멀티 셀에서 찾을 곳이 없었다. 셀 = $CONFIG(캠페인 셀 id).
#   **이 스크립트를 돌리는 노드(= master 가 로컬인 메인)의 진행표만** 적는다(자기 저작 · single_serve_up.sh 와 같은 규칙):
#     · 멀티 셀은 클러스터 셀이고 그 진행표 노드는 셀을 배정받은 노드다(orchestration.topology.md "assignments 에는
#       메인만" · hint evidence/publish 도 배정 노드의 진행표를 읽는다).
#     · `phases/<node>/<phase>.status.json` 은 노드당 한 자리라 **마지막 쓰기가 이긴다** — 메인이 phases/<sub> 를 쓰면
#       서브가 자기 셀로 적은 진행표를 덮는다(2026-09-17 camp-26091717 에는 서브 자기저작 진행표가 실재했다).
#     · 메인이 서브의 진행표를 적는 것은 P5 가 결함으로 잡는 형태다(상향 회수는 문서기반 `--import-sub` · 노드 제어 ①).
#     slave(Ray 워커)의 빌드·기동 결과는 이 진행표의 proof 출처(양 노드 rc · attestation v2)에 실린다.
#   ACTIVE=_bootstrap(캠페인 밖)이면 writer 가 스스로 no-op 이다. writer 부재는 **소리 낸다**(single 2026-09-08 F5).
_CI="$REPO/.claude/skills/terraforming_node/scripts/campaign_init.py"
_campaign_phase() {   # $1=phase $2=state $3=proof_ok(1|0) $4=predicate $5=source $6=started_utc
  if [ ! -f "$_CI" ]; then
    echo "[mn] ⚠ campaigns writer 부재($_CI) — $1 진행표를 아무도 적지 않았다(침묵 누락 ✗)." >&2
    return 0
  fi
  local _ok=()
  [ "$3" = "1" ] && _ok=(--proof-ok)
  python3 "$_CI" --phase-set "$1" --node "$MAIN_NODE_ID" --state "$2" --cell "$CONFIG" \
    --proof-predicate "$4" "${_ok[@]}" --proof-source "$5" \
    --started-utc "$6" --ended-utc "$(date -u +%FT%TZ)" --authored-by "$MAIN_NODE_ID" \
    || echo "[mn] ⚠ campaigns writer 실패 — $1 진행표($MAIN_NODE_ID)가 기록되지 않았다" >&2
}
#   빌드 phase 는 게이트·대조의 `exit` 가 여러 갈래라 EXIT 트랩으로 닫는다: 열린 채 끝나면 failed 로 적는다
#   (부재와 실패는 다른 사실이다 — "돌지 않았다" 와 "돌다 죽었다" 를 가른다). 종료코드는 바꾸지 않는다.
_BUILD_PHASE_OPEN=0; _BUILD_T0=""
_close_open_build_phase() {
  local _rc=$?
  rm -f "${ATTEST_ROWS:-}" 2>/dev/null   # 대조 행 임시파일은 --build 에서만 생긴다 — 어느 exit 갈래로 끝나도 남기지 않는다
  if [ "${_BUILD_PHASE_OPEN:-0}" = "1" ]; then
    _BUILD_PHASE_OPEN=0
    _campaign_phase build failed 0 "양 노드 compose build rc=0 + 빌드 후 대조" \
      "multinode_serve_smoke.sh: build 단계가 rc=$_rc 로 끝났다(빌드 실패·트랙/버전/ABI 대조 거부 — 위 [mn] FAIL 줄)" "$_BUILD_T0"
  fi
}

# ── 노드 정합 attestation v2 (2026-09-21 · plan_26092119 §4.6 · X12 · 코드맵 K1/K2) ─────────────────
#   v1 은 `--build` 성공 분기 **안에서만** 쓰였고 두 대조가 **wheel 트랙 전용**이라 source-build 이미지는
#   `checks: []` 였다(K1). 태그2 셀은 빌드를 다른 config 이름으로 했기 때문에 셀 이름의 파일 자체가 없었다(K2).
#   v2: 셀 이름($CONFIG)으로 **serve 시점**에 두 노드의 **실행 중 컨테이너**에서 캡처한다 —
#     build_ledger.json(없으면 옛 이미지: unobservable) · vllm git sha · vllm/torch 배포판 버전 · 호스트 driver.
#   축별 판정 equal|differ|unobservable 은 **기록만** 한다 — differ 를 차단할지는 사람 결정(D-5)이다.
#   ⚠ image digest 는 판정 축이 아니다(strategy.topology.md: "동일 이미지가 아니라 동일 ABI" — 2026-08-22 성공한
#     두 컨테이너의 digest 가 달랐다). 기록만 한다.
#   policy:GIT_SINGLE_AUTHORITY: git 이 들지 않는 실행 시점 사실(맹점층)이다. 파일은 output/multi/benchlog(비추적).
ATTEST_DIR="$REPO/output/multi/benchlog"   # $TOPO 아님 — 이 스크립트는 멀티 전용
ATTEST_JSON="$ATTEST_DIR/attestation_${CONFIG}.json"
ATTEST_ROWS="/tmp/mn_attest_rows.${CONFIG}.$$"   # 이번 실행의 빌드 후 대조 행(필요할 때만 생긴다 · serve 기록 후 지운다)
_attest_add() {   # $1=축 $2=노드 $3=값 $4=기대(선택) — TSV 로 모으고 JSON 조립은 파이썬이 한다(따옴표 깨짐 ✗)
  printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "${4:-}" >> "$ATTEST_ROWS"
}
_attest_run() {   # $1=main|sub · 나머지 = 명령 argv. sub 는 argv 를 %q 로 인용해 원격 셸에 그대로 넘긴다
  local n="$1"; shift
  if [ "$n" = "sub" ]; then timeout 30 $SSH -n "$SUB_HOST" "$(printf '%q ' "$@")"; else "$@"; fi
}
_attest_capture_node() {   # $1=main|sub $2=컨테이너 $3=캡처 디렉터리 — 실패는 파일 부재로 남는다(기록 전용)
  local n="$1" c="$2" d="$3"
  # 버전은 importlib.metadata 로만 읽는다 — `import vllm` 은 엔진을 적재한다(관측이 관측 대상을 건드리면 안 된다).
  _attest_run "$n" docker exec "$c" cat /opt/easy-vllm/build_ledger.json > "$d/$n.ledger.json" 2>/dev/null || rm -f "$d/$n.ledger.json"
  _attest_run "$n" docker exec "$c" git -C /workspace/vllm-src rev-parse HEAD > "$d/$n.vllm_sha" 2>/dev/null || rm -f "$d/$n.vllm_sha"
  _attest_run "$n" docker exec "$c" python3 -c "import importlib.metadata as m;print(m.version('vllm'))" > "$d/$n.vllm_dist" 2>/dev/null || rm -f "$d/$n.vllm_dist"
  _attest_run "$n" docker exec "$c" python3 -c "import importlib.metadata as m;print(m.version('torch'))" > "$d/$n.torch" 2>/dev/null || rm -f "$d/$n.torch"
  _attest_run "$n" docker inspect -f '{{.Image}}' "$c" > "$d/$n.image_id" 2>/dev/null || rm -f "$d/$n.image_id"
  _attest_run "$n" nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null | head -1 > "$d/$n.driver"
  [ -s "$d/$n.driver" ] || rm -f "$d/$n.driver"
  printf '%s' "$c" > "$d/$n.container"
}
_attest_write() {   # $1=phase(build|serve) $2=캡처 디렉터리(serve 만) — 실패해도 진행은 막지 않는다
  mkdir -p "$ATTEST_DIR" 2>/dev/null || { echo "[mn] ⚠ $ATTEST_DIR 생성 실패 — attestation 미기록"; return 0; }
  if python3 - "$ATTEST_JSON" "$CONFIG" "$IMG" "$1" "${2:-}" "$ATTEST_ROWS" "$SF" "$MAIN_NODE_ID" "$SUB_NODE_ID" <<'PY'
import hashlib, json, os, re, sys
out, config, image, phase, cap, rows_path, sf, main_id, sub_id = sys.argv[1:10]
sys.path.insert(0, os.path.dirname(sf))
import slave_forward
def read(p):
    try:
        with open(p, encoding="utf-8") as fh:
            return fh.read().strip() or None
    except OSError:
        return None
checks = []
for line in (read(rows_path) or "").splitlines():
    axis, node, observed, expected = (line.split("\t") + ["", "", "", ""])[:4]
    checks.append({"axis": axis, "node": node, "observed": observed, "expected": expected, "phase": "build"})
doc = {"schema_version": 2, "kind": "multinode_node_parity_attestation", "config": config, "topology": "multi",
       "image_tag": image, "phase": phase, "checks": checks,
       "parity_blocking": False,
       "parity_note": "기록 전용 — differ 를 차단할지는 사람 결정(D-5). image digest 는 판정 축이 아니다(동일 ABI ≠ 동일 이미지)."}
if phase == "build":
    doc["provenance"] = "measured(docker run in each node image · 이번 실행 --build 대조)"
else:
    doc["provenance"] = "measured(docker exec in each running container · host nvidia-smi)"
    exempt = sorted(slave_forward.NON_IDENTITY_BUILD_ARGS)
    nodes = {}
    for role, node_id in (("main", main_id), ("sub", sub_id)):
        raw = read(os.path.join(cap, role + ".ledger.json"))
        ledger, status = None, "unobservable(capture failed or legacy image without /opt/easy-vllm/build_ledger.json)"
        if raw:
            try:
                ledger, status = json.loads(raw), "observed"
            except ValueError:
                status = "unobservable(ledger unparsable)"
        if status == "observed" and not isinstance(ledger, dict):
            ledger, status = None, "unobservable(ledger is not a JSON object)"
        sha = (ledger.get("vllm") or {}).get("git_sha") if isinstance(ledger, dict) else None
        sha_src = "build_ledger.vllm.git_sha" if sha else None
        if not sha:
            cand = read(os.path.join(cap, role + ".vllm_sha"))
            if cand and re.fullmatch(r"[0-9a-f]{40}", cand):
                sha, sha_src = cand, "docker exec git -C /workspace/vllm-src rev-parse HEAD"
        nodes[role] = {"node_id": node_id, "container": read(os.path.join(cap, role + ".container")),
                       "image_id": read(os.path.join(cap, role + ".image_id")),
                       "build_ledger": ledger, "build_ledger_status": status,
                       "vllm_sha": sha, "vllm_sha_source": sha_src,
                       "vllm_dist_version": read(os.path.join(cap, role + ".vllm_dist")),
                       "torch": read(os.path.join(cap, role + ".torch")),
                       "driver": read(os.path.join(cap, role + ".driver"))}
    def identity(led):
        if not isinstance(led, dict):
            return None
        args = {k: v for k, v in (led.get("build_args") or {}).items() if k not in exempt}
        pats = sorted((p.get("phase"), p.get("file"), p.get("script_sha256"), p.get("result"))
                      for p in (led.get("patches") or []) if isinstance(p, dict))
        body = {"dockerfile": led.get("dockerfile"), "build_args": args, "patches": pats,
                "vllm_git_sha": (led.get("vllm") or {}).get("git_sha")}
        return hashlib.sha256(json.dumps(body, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()
    def verdict(a, b):
        return "unobservable" if a is None or b is None else ("equal" if a == b else "differ")
    parity = {}
    for axis in ("vllm_sha", "torch", "driver"):
        a, b = nodes["main"][axis], nodes["sub"][axis]
        parity[axis] = {"verdict": verdict(a, b), "main": a, "sub": b}
    la, lb = identity(nodes["main"]["build_ledger"]), identity(nodes["sub"]["build_ledger"])
    parity["build_ledger"] = {"verdict": verdict(la, lb), "main_identity_sha256": la, "sub_identity_sha256": lb,
                              "identity_exempt_build_args": exempt,
                              "compared": "dockerfile · build_args(− exempt) · patches(phase,file,script_sha256,result) · vllm git sha"}
    # serve 시점 관측도 **대조 행**(`checks` 의 {axis,node,observed,expected})으로 싣는다 — v1 부터 행을 읽는 소비자
    #   (hint evidence 의 결손 판정: 행 0 = HINT_MISSING_ATTESTATION_PARITY_ROWS)가 `--build` 없이 서빙만 한 셀
    #   (태그2 가 그랬다)을 "대조 없음" 으로 적지 않게. expected = 상대 노드의 관측값(정합 = 두 노드가 같다).
    #   **양 노드를 다 관측한 축만** 행이 된다 — 한쪽만 본 값(예: ssh 불통으로 서브 캡처 실패)을 행으로 만들면 소비자가
    #   "대조가 있었다" 로 읽는다(행 ≥ 1 = 결손 아님). 그 값은 nodes/parity(unobservable)에 그대로 남는다 — 전부 못 봤으면
    #   행 0 이 결손을 말한다(관측 불가를 대조로 지어내지 않는다).
    ident = {"main": la, "sub": lb}
    for axis in ("vllm_sha", "torch", "driver", "build_ledger_identity"):
        key = "build_ledger" if axis == "build_ledger_identity" else axis
        if parity[key]["verdict"] not in ("equal", "differ"):
            continue
        for role, other in (("main", "sub"), ("sub", "main")):
            got = ident[role] if axis == "build_ledger_identity" else nodes[role][axis]
            want = ident[other] if axis == "build_ledger_identity" else nodes[other][axis]
            checks.append({"axis": axis, "node": role, "observed": got, "expected": want,
                           "expected_source": f"parity:{other}", "phase": "serve", "verdict": parity[key]["verdict"]})
    doc["nodes"] = nodes
    doc["parity"] = parity
with open(out, "w", encoding="utf-8") as fh:
    json.dump(doc, fh, ensure_ascii=False, indent=2, sort_keys=True)
    fh.write("\n")
summary = " ".join(f"{k}={v['verdict']}" for k, v in sorted((doc.get("parity") or {}).items()))
print(f"[mn] 노드 정합 attestation v2({phase}) → {os.path.relpath(out)} {summary}".rstrip())
PY
  then :; else echo "[mn] ⚠ attestation 기록 실패(기록 전용 — 진행은 막지 않는다)"; fi
}

# ── 빌드(옵션, 양 노드 병렬) ──
if [ "$BUILD" = "1" ]; then
_BUILD_T0="$(date -u +%FT%TZ)"; _BUILD_PHASE_OPEN=1
trap _close_open_build_phase EXIT     # 아래 어느 exit 로 끝나도 build 진행표가 failed 로 닫힌다
# ── 빌드 트랙 ↔ 이미지 태그 정합 게이트 (2026-09-04 실측) ─────────────────────────
#
# 이미지 네이밍 불변식은 `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}` 이라 **track 이 태그 안에 있다**.
# 그런데 실제 빌드 트랙은 `BUILD_DOCKERFILE` 이 따로 고르고 compose 기본값은 `Dockerfile.source-build`
# 다. 즉 태그와 내용이 **독립적으로** 정해질 수 있고, 갈려도 아무도 울지 않는다.
#
# 실측: `.env` 에 `IMAGE_TAG=…-wheel` 만 적고 `BUILD_DOCKERFILE` 을 빠뜨렸더니 0.27.1 **소스**가
# 빌드돼 그 결과물이 `…-0.18.0-…-wheel` 태그로 붙었다(엔진 로그 `v0.27.2.dev0+g6e448d0ea`).
# 양 노드의 진짜 0.18.0 wheel 이미지가 그것으로 덮였고, 인증서는 태그만 적으므로 **그 사실을 말할
# 수단이 없었다**. 태그는 가변 포인터다 — 같은 문자열이 다른 내용을 가리킬 수 있다.
#
# 여기서 fail-loud 한다. 주입하지 않는다 — 어느 쪽이 옳은지는 사람이 정할 일이고, 조용히 맞추면
# 그 선택이 기록되지 않는다.
# ⚠ 이 스크립트는 env 를 셸에 source 하지 않는다 — `val` 로 뽑아 $IMG/$BDF 에 담는다(위 참조).
#   셸 변수 $IMAGE_TAG/$BUILD_DOCKERFILE 을 보면 항상 비어 있어 **게이트가 조용히 무력해진다**
#   (있는 척하지만 울지 않는 가드 — 없는 것보다 나쁘다).
_it="$IMG"
_bd="${BDF:-Dockerfile.source-build}"
[ -n "$_it" ] || { echo "[mn] FAIL: IMAGE_TAG 미해소 — 빌드 트랙 정합을 판정할 수 없다(fail-closed)." >&2; exit 3; }
case "$_it" in
  *-wheel)
    [ "$_bd" = "Dockerfile" ] || {
      echo "[mn] FAIL(트랙 불일치): IMAGE_TAG='$_it' 는 wheel 트랙인데 BUILD_DOCKERFILE='$_bd' 다." >&2
      echo "[mn]   → 이대로 빌드하면 소스빌드 결과물이 wheel 태그로 붙는다(태그≠내용)." >&2
      echo "[mn]   → $EF 에 'BUILD_DOCKERFILE=Dockerfile' 을 명시하라." >&2
      exit 3; } ;;
  *-source|*-source-*)
    case "$_bd" in Dockerfile.source-build*) ;; *)
      echo "[mn] FAIL(트랙 불일치): IMAGE_TAG='$_it' 는 source 트랙인데 BUILD_DOCKERFILE='$_bd' 다." >&2
      echo "[mn]   → $EF 에 'BUILD_DOCKERFILE=Dockerfile.source-build' 을 명시하라." >&2
      exit 3 ;; esac ;;
esac
# ── 버전 정합 — 트랙만 맞아도 **버전이 갈릴 수 있다** (2026-09-04) ──
#   태그의 버전 조각과 실제 빌드가 쓰는 버전을 대조한다. wheel 트랙은 VLLM_VERSION(env 또는
#   Dockerfile ARG 기본값), source 트랙은 VLLM_REF 가 그 값이다. 트랙 게이트만 있던 초판은
#   `IMAGE_TAG=…0.19.1…-wheel` + Dockerfile 기본값 0.18.0 조합을 그대로 통과시켰다.
_tag_ver="$(printf '%s' "$_it" | sed -n 's/^easy-vllm:\([0-9][0-9.]*\)-.*/\1/p')"
if [ -n "$_tag_ver" ]; then
  case "$_bd" in
    Dockerfile)
      # ⚠ 이 스크립트에 $TOPO 는 없다(멀티 전용 진입점 — EF 도 output/multi 로 고정이다).
      #   초판이 `output/$TOPO/Dockerfile` 을 읽어 빈 경로가 됐고, 그 결과 `_bld_ver` 가 비어
      #   **대조를 건너뛰었다**(로그: `빌드 버전 정합: 태그 0.18.0 ↔ 빌드  `). 판정 불가를 통과로
      #   읽은 것 — 바로 아래 ABI 게이트에서는 지킨 원칙을 여기서 어겼다. 경로 고정 + fail-loud.
      _bld_ver="$(val VLLM_VERSION)"
      [ -n "$_bld_ver" ] || _bld_ver="$(sed -n 's/^ARG VLLM_VERSION=\(.*\)$/\1/p' "output/multi/Dockerfile" 2>/dev/null | head -1)"
      _src="env/Dockerfile-ARG" ;;
    # VLLM_REF 가 셀 env 에 없으면 compose 기본값이 쓰인다 — 그 값을 **compose 에서 읽는다**. 옛 하드 폴백
    #   "0.27.1" 은 compose 기본값이 v0.29.0rc6 로 바뀐 뒤에도 남아 대조를 거짓으로 만들었다(코드맵 K10 · 매직넘버 결함).
    *) _bld_ver="$(val VLLM_REF)"; _bld_ver="${_bld_ver#v}"; _src="VLLM_REF"
       [ -n "$_bld_ver" ] || { _bld_ver="$(python3 "$SF" default --compose "$COMPOSE_MULTI" --key VLLM_REF 2>/dev/null)"; _bld_ver="${_bld_ver#v}"; _src="compose 기본값(slave_forward default)"; } ;;
  esac
  if [ -z "$_bld_ver" ]; then
    echo "[mn] FAIL(버전 판정 불가): 태그 버전 '$_tag_ver' 에 대응하는 빌드 버전을 읽지 못했다($_src)." >&2
    echo "[mn]   → 판정 불가는 통과가 아니다. wheel 트랙이면 output/multi/Dockerfile 의 ARG VLLM_VERSION" >&2
    echo "[mn]     또는 $EF 의 VLLM_VERSION 을, source 트랙이면 VLLM_REF 를 확인하라." >&2
    exit 3
  fi
  if [ "$_tag_ver" != "$_bld_ver" ]; then
    echo "[mn] FAIL(버전 불일치): IMAGE_TAG 의 버전 '$_tag_ver' ≠ 실제 빌드 버전 '$_bld_ver' ($_src)." >&2
    echo "[mn]   → 이대로 빌드하면 태그가 거짓말을 한다(다른 버전이 그 이름으로 붙는다)." >&2
    echo "[mn]   → wheel 트랙이면 $EF 에 'VLLM_VERSION=$_tag_ver' 를, source 트랙이면 'VLLM_REF=v$_tag_ver' 를 명시하라." >&2
    exit 3
  fi
  echo "[mn] 빌드 버전 정합: 태그 $_tag_ver ↔ 빌드 $_bld_ver ($_src)"
else
  echo "[mn] ⚠ IMAGE_TAG 에서 버전 조각을 못 읽었다('$_it') — 버전 정합 판정 생략(트랙 정합은 통과)."
fi
echo "[mn] 빌드 트랙 정합: IMAGE_TAG=$_it ↔ BUILD_DOCKERFILE=$_bd"
  echo "[mn] 양 노드 빌드(병렬)... build_jobs=${BJOBS:-<Dockerfile 기본 16>}"
  docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master build >/tmp/mn_build_master.log 2>&1 & BPID=$!
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS $SLAVE_BUILDVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave build'" >/tmp/mn_build_slave.log 2>&1 & SPID=$!
  wait $BPID; MR=$?; wait $SPID; SR=$?
  if [ $MR -eq 0 ] && [ $SR -eq 0 ]; then echo "[mn] 빌드 OK(양 노드)";
# ── 빌드 후 ABI 불변식 검증 (2026-09-04 신설) ─────────────────────────────────────
#
# 헌법 §레이어 커플링: "prebuilt wheel 트랙에서는 대상 vLLM 버전이 선언한 torch 버전과 NGC 컨테이너
# 베이스의 torch 빌드버전이 **접두어 일치**해야 한다(wheel `_C` 의 ABI 요건)".
# 그 불변식은 문서에만 있었고 **코드가 집행한 적이 없었다**. 실측(2026-09-04): 멀티 requirements 가
# 0.27.1 wheel 에서 파생된 낡은 것이라 `torchcodec>=0.14` 가 들어 있었고, 그것이 의존성으로 torch 를
# 2.14.0 으로 끌어올려 NGC 베이스의 2.10.0a0 을 덮었다. 빌드는 성공했고 서빙 기동에서야
# `ImportError: vllm/_C.abi3.so: undefined symbol: _ZN3c104cuda29c10_cuda_check_implementation…`
# 로 죽었다 — 증상이 원인에서 멀다.
#
# 그래서 **빌드 직후 이미지 안에서** 실제 torch 를 읽어 대조한다. 양 노드 모두 본다(멀티는 빌드
# 인자가 한 톨도 갈리면 안 된다 — 이 파일의 SLAVE_IMGVARS 규율과 같은 이유).
# ── 노드 정합 attestation 영속 (2026-09-06 신설 · plan_26090616 ⑤ · 결함 F6) ──
#   아래 두 대조(torch ABI · vllm 버전)는 **stdout 으로만** 말했다. 성공 경로에서는 아무것도
#   남지 않았고, 로그 보존은 실패 분기(`_save_serve_logs`)에만 걸려 있었다. 그래서 "두 노드가
#   같은 것을 돌렸다" 는 사실이 hint 태그에 실릴 근거가 없었다 — 멀티 태그에 메인 산출물만
#   실린 이유 중 하나다. 성공한 대조야말로 배포될 증거이므로 **성공할 때 적는다**.
#   (2026-09-21) 기록기는 최상위 `_attest_add`/`_attest_write`(v2)로 올라갔다 — 이 분기는 행만 더한다.

if [ "$_bd" = "Dockerfile" ]; then   # wheel 트랙에만 적용(source-build 는 베이스 torch 를 그대로 쓴다)
  _want="$(python3 "$REPO/.claude/skills/upstream-version-watch/scripts/resolve_torch_pin.py" \
             "$_tag_ver" 2>/dev/null | python3 -c \
             'import sys,json;d=json.load(sys.stdin);print(d.get("torch_prefix") or "")' 2>/dev/null || echo "")"
  if [ -z "$_want" ]; then
    echo "[mn] ⚠ torch 기대 접두어 미해소(vLLM $_tag_ver) — ABI 검증 생략(판정 불가를 통과로 읽지 않되 차단도 않는다)."
  else
    for _n in main sub; do
      if [ "$_n" = "main" ]; then
        _got="$(docker run --rm "$IMG" python3 -c 'import torch;print(torch.__version__)' 2>/dev/null | tail -1)"
      else
        _got="$(ssh -o BatchMode=yes -n "$SUB_HOST" "docker run --rm '$IMG' python3 -c 'import torch;print(torch.__version__)'" 2>/dev/null | tail -1)"
      fi
      case "$_got" in
        "$_want"*) echo "[mn] ABI 정합($_n): torch $_got ← 기대 접두어 $_want"
                   _attest_add "torch_abi" "$_n" "$_got" "$_want*" ;;
        "") echo "[mn] FAIL(ABI): $_n 이미지에서 torch 버전을 읽지 못했다 — 검증 불가는 통과가 아니다." >&2; exit 3 ;;
        *)  echo "[mn] FAIL(ABI 불일치): $_n 이미지 torch=$_got 인데 vLLM $_tag_ver 는 $_want* 를 요구한다." >&2
            echo "[mn]   → wheel 의 _C 확장이 베이스 torch 와 ABI 가 갈린다(기동 시 undefined symbol)." >&2
            echo "[mn]   → requirements 가 torch 를 끌어올렸는지 확인하라: 'regen_requirements.py --from-wheel-url <이 버전의 wheel> -o output/multi/requirements.txt'" >&2
            exit 3 ;;
      esac
    done
  fi
fi

# ── 빌드 결과 버전 대조 — **양 노드의 이미지 안에서 실제 vLLM 을 읽는다** (2026-09-05 신설) ──
#   위 §태그↔빌드 대조는 *선언* 을 본다(EF 의 VLLM_VERSION / Dockerfile ARG). 그것은 마스터의
#   의도가 태그와 맞는지만 말하고, **슬레이브가 그 의도대로 빌드됐는지는 말하지 못한다**.
#   슬레이브는 EFC(Band2)만 받으므로 콤보 EF 의 키는 SLAVE_IMGVARS 에 담겨야 도달한다(2026-09-21 부터
#   slave_forward 가 compose 에서 파생 — 그 전엔 손목록이었다). 목록이 build-arg 와 함께 자라지 않으면
#   그 순간 두 노드가 갈린다. 실제로 이 저장소는
#   같은 형태를 다섯 번 겪었다(BUILD_DOCKERFILE · VLLM_PRETEND_VERSION · SM12X_PORT ·
#   SRC_DEPS_AUTHORITY · VLLM_VERSION). **리스트를 늘리는 대신 결과를 대조한다** —
#   무엇이 빠졌든 결과가 갈리면 여기서 걸린다.
#   ⚠ torch 대조로는 이 사고를 못 잡는다: 0.18.0 과 0.19.0 은 둘 다 torch 2.10.0 을 핀하므로
#     버전이 갈려도 ABI 게이트는 통과한다.
if [ "$_bd" = "Dockerfile" ] && [ -n "$_tag_ver" ]; then
  for _n in main sub; do
    if [ "$_n" = "main" ]; then
      _vv="$(docker run --rm --entrypoint python3 "$IMG" -c 'import vllm;print(vllm.__version__)' 2>/dev/null | tail -1)"
    else
      _vv="$(ssh -o BatchMode=yes -n "$SUB_HOST" "docker run --rm --entrypoint python3 '$IMG' -c 'import vllm;print(vllm.__version__)'" 2>/dev/null | tail -1)"
    fi
    case "$_vv" in
      "") echo "[mn] FAIL(버전 대조): $_n 이미지에서 vllm 버전을 읽지 못했다 — 검증 불가는 통과가 아니다." >&2; exit 3 ;;
      "$_tag_ver"|"$_tag_ver"+*) echo "[mn] 빌드 버전 정합($_n): vllm $_vv ← 태그 $_tag_ver"
                                 _attest_add "vllm_version" "$_n" "$_vv" "$_tag_ver" ;;
      *)  echo "[mn] FAIL(빌드 버전 불일치): $_n 이미지의 vllm=$_vv 인데 IMAGE_TAG 는 $_tag_ver 다." >&2
          echo "[mn]   → 두 노드가 같은 태그로 **다른 엔진**을 돌게 된다(태그가 노드 경계에서 거짓말한다)." >&2
          echo "[mn]   → 콤보 EF 의 빌드 키가 SLAVE_IMGVARS 로 전달되는지 확인하라(현재: $SLAVE_IMGVARS)." >&2
          exit 3 ;;
    esac
  done
fi
_attest_write build
_BUILD_PHASE_OPEN=0
_campaign_phase build done 1 "양 노드 compose build rc=0 + 빌드 후 대조(wheel: torch ABI·vllm 버전)" \
  "multinode_serve_smoke.sh: compose build master rc=$MR · slave rc=$SR (image=$IMG) · ${ATTEST_JSON#$REPO/}" "$_BUILD_T0"
  else echo "[mn] FAIL: 빌드(master=$MR slave=$SR). tail:"; tail -6 /tmp/mn_build_master.log /tmp/mn_build_slave.log; exit 2; fi
fi

if [ "$BUILD_ONLY" = "1" ]; then
  rm -f "$ATTEST_ROWS"
  echo "[mn] --build-only 종료(서빙·스모크 미수행). 이미지: $IMG"
  echo "[mn] 다음 단계는 로드-전 게이트를 **정상적으로** 타야 한다 — 가중치 RAM 이 실제로 필요하다."
  exit 0
fi

# ── 협역 워치독(계층 2층 — plan_26071019 §2.3): 컨테이너 기동 *전* 폴링 개시(로드 구간 커버) ──
#   필터 = 컨테이너명 공통 접두(mn-<config> — master/slave 양쪽 부분일치). 정지는 PID 기반만
#   (**pkill -f 금지** — 자기참조 부모셸 사망 exit144 선례, devlog_26062718).
# ── 잔존 워치독 회수 계약(2026-08-02 신설) ──────────────────────────────────────
#   `--keep-up` 은 워치독을 **의도적으로** 살려두는데, 회수 책임자가 없어 실행마다 하나씩 쌓였다
#   (2026-08-02 실측: 메인 8 · 서브 9). 잔존분은 각자 thresh 10240 에서 SIGKILL 할 수 있고
#   **선언된 바닥을 모른다** — 즉 위양성 사살기가 백그라운드에 누적된다. 기동 전에 회수한다.
#   멀티는 클러스터 전체가 한 서빙에 전용되므로 "동시 서빙 보호"를 깨뜨리지 않는다.
#
#   ★ 판정은 argv **위치**로만 한다: argv[1]=bash · argv[2]=…/mem_watchdog.sh(끝 앵커).
#     부분문자열 매칭(`pkill -f`·`pgrep -fc`·`case *…*`)은 **이름을 언급만 한 명령까지** 잡아
#     자기 부모셸을 죽이거나(exit 144, devlog_26062718) 자기 진단 명령을 좀비로 센다
#     (testlog_26080207 §7). 2026-08-02 이 트랩을 또 밟았다 — 위치 대조가 유일한 정답이다.
reap_stale_watchdogs() {   # $1 = "master" | "slave"
  if [ "$1" = "master" ]; then
    ps -eo pid= -o args= | awk -v self="$$" \
      '$1 != self && $2 ~ /(^|\/)bash$/ && $3 ~ /mem_watchdog\.sh$/ {print $1}' \
      | while read -r p; do kill "$p" 2>/dev/null && echo "[mn] 잔존 워치독 회수(master pid=$p)"; done
  else
    timeout 20 $SSH -n "$SUB_HOST" "ps -eo pid= -o args= | awk '\$2 ~ /(^|\/)bash\$/ && \$3 ~ /mem_watchdog\.sh\$/ {print \$1}' | while read -r p; do kill \$p 2>/dev/null && echo \$p; done" 2>/dev/null \
      | while read -r p; do [ -n "$p" ] && echo "[mn] 잔존 워치독 회수(slave pid=$p)"; done
  fi
}

# ── 예산 갱신 루프 회수 (2026-08-22 신설 · W-7) ──────────────────────────────────────
#   워치독과 **같은 부류의 상주 사이드카**이므로 같은 규율을 쓴다: 판정은 argv **위치**로만
#   (부분문자열 매칭 금지 — 자기 부모셸 사망 exit144 선례). 루프는 컨테이너가 사라지면 스스로
#   끝나지만, teardown 이 컨테이너를 내린 **직후**에는 아직 다음 tick 전이라 살아 있을 수 있다.
#   남겨 두면 이미 회수된 선언을 되살리려다 fail-loud 로 죽으며 로그를 오염시킨다.
reap_renew_loops() {   # $1 = "master" | "slave"
  if [ "$1" = "master" ]; then
    ps -eo pid= -o args= | awk -v self="$$" \
      '$1 != self && $2 ~ /(^|\/)bash$/ && $3 ~ /budget_renew_loop\.sh$/ {print $1}' \
      | while read -r p; do kill "$p" 2>/dev/null && echo "[mn] 예산갱신 루프 회수(master pid=$p)"; done
  else
    timeout 20 $SSH -n "$SUB_HOST" "ps -eo pid= -o args= | awk '\$2 ~ /(^|\/)bash\$/ && \$3 ~ /budget_renew_loop\\.sh\$/ {print \$1}' | while read -r p; do kill \$p 2>/dev/null && echo \$p; done" 2>/dev/null \
      | while read -r p; do [ -n "$p" ] && echo "[mn] 예산갱신 루프 회수(slave pid=$p)"; done
  fi
}

WD_MAIN_PID=""; WD_SUB_PID=""
RENEW_MAIN_PID=""; RENEW_SUB_PID=""
if [ "$WATCHDOG" = "1" ]; then
  # ★ 필터는 **노드마다 다르다**(2026-09-09 교정). 종전에는 `WFILTER="${MC%-master}"` 하나를 만들어
  #   양 노드에 같은 값을 보냈다. 의도는 공통 접두어(`mn-<config>`)를 뽑아 docker `name=` 부분일치로
  #   master·slave 를 함께 덮는 것이었는데, 컨테이너 명명 규약이 `-master` 에서 **`-master-container`**
  #   로 바뀌면서 그 strip 이 **no-op** 이 됐다. 결과:
  #       메인 ← `<cell>-master-container` → 자기 이름이라 **우연히** 매칭 1 (그래서 아무도 몰랐다)
  #       서브 ← `<cell>-master-container` → 서브엔 그 이름이 없으므로 **매칭 0**
  #   즉 슬레이브 협역 워치독이 "무장한 척"만 하고 아무것도 감시하지 않았다. 하드다운 #2 가 **서브**
  #   였음을 상기하라 — 계층 2층이 서브에서만 조용히 걷혀 있었다. 실측 2026-09-09(camp DS4F-0731 L0):
  #   메인 pid=359982 대상 1개 · 서브 pid=3753956 대상 0개, `verify_node_blackbox` 가 `no_zombie` 로 검출.
  #   ★ 접두어를 다시 계산하는 방식(`${MC%-master-container}`)은 **쓰지 않는다** — `b-768k-kvfp8` 은
  #     `b-768k-kvfp8-l1spec` 의 접두어라 부분일치가 다음 셀 컨테이너까지 사살 대상으로 끌어들인다.
  #     각 노드의 **실제 이름**을 그대로 쓴다(둘 다 이미 env 에서 해소돼 있다 — 새 입력이 없다).
  WFILTER_MAIN="$MC"
  WFILTER_SUB="$SLVC"
  # ★ 빈 필터 = fail-closed. 미설정이면 `${1:-@vllm}` 이 조용히 **광역** 필터로 되돌아가, 무관한
  #   vllm 컨테이너까지 사살 대상이 된다(2026-08-02 잔존분 중 실제 1건). compose 의 `:-기본값` 폴백이
  #   ⑥ env 의 multi 키 누락을 숨겼던 것과 같은 부류다 — 설정 누락은 조용한 광역화가 아니라 큰 소리로
  #   실패해야 한다. 슬레이브도 같은 강도로 막는다: 종전엔 `SLAVE_CONTAINER_NAME` 미설정이 compose
  #   기본값으로 조용히 흘러 **서브에서만** 2층이 사라졌다(:129 주석이 예고한 그 경로다).
  [ -n "$WFILTER_MAIN" ] || { echo "[mn] FAIL: 워치독 필터가 비었다(MASTER_CONTAINER_NAME 미설정). env 를 고쳐라."; exit 2; }
  [ -n "$WFILTER_SUB" ]  || { echo "[mn] FAIL: 슬레이브 워치독 필터가 비었다(SLAVE_CONTAINER_NAME 미설정) — 서브 협역층이 매칭 0 으로 무장한다. env 를 고쳐라."; exit 2; }
  MAIN_WATCHDOG="$REPO/.claude/skills/terraforming_node/scripts/host_safety/mem_watchdog.sh"
  SUB_WATCHDOG_REL=".claude/runtime/host_safety/mem_watchdog.sh"
  [ -f "$MAIN_WATCHDOG" ] || { echo "[mn] FAIL: canonical host-safety watchdog absent: $MAIN_WATCHDOG"; exit 2; }
  reap_stale_watchdogs master
  bash "$MAIN_WATCHDOG" "$WFILTER_MAIN" "${WATCHDOG_THRESH_MIB:-10240}" 2 >/tmp/mn_watchdog_master.log 2>&1 & WD_MAIN_PID=$!
  echo "$WD_MAIN_PID" > /tmp/mn_watchdog_master.pid
  echo "[mn] 워치독(master) pid=$WD_MAIN_PID filter=$WFILTER_MAIN thresh=${WATCHDOG_THRESH_MIB:-10240}MiB (/tmp/mn_watchdog_master.log)"
  if $SSH "$SUB_HOST" "bash -lc '[ -f $SUB_WORK_DIR/$SUB_WATCHDOG_REL ]'" 2>/dev/null; then
    reap_stale_watchdogs slave
    # ⚠ 원격 백그라운드 detach — 3-FD 리다이렉트(</dev/null + ssh -n)만으론 여전히 hang(2026-07-11 hy3 serve#1 실증:
    #   슬레이브 워치독은 정상 기동했으나 command-substitution ssh 가 ~8분 안 끝나 서빙 전체 블록). 원인 = 원격
    #   백그라운드 프로세스가 ssh 세션 프로세스그룹에 남아 sshd 가 채널 EOF 를 안 보냄(stdin 분리만으론 부족).
    #   해소 = setsid(새 세션 완전 분리 → sshd 즉시 채널 close) + exit 0(원격 셸 즉시 종료) + timeout 20(백스톱:
    #   그래도 hang 시 20s 후 ssh 만 종료 — 워치독은 이미 기동·PID 는 이미 echo 됨). PID 캡처 동작 보존.
    WD_SUB_PID=$(timeout 20 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD setsid nohup bash $SUB_WATCHDOG_REL $WFILTER_SUB ${WATCHDOG_THRESH_MIB:-10240} 2 </dev/null >/tmp/mn_watchdog_slave.log 2>&1 & echo \$!; exit 0'" 2>/dev/null || true)
    echo "[mn] 워치독(slave) pid=${WD_SUB_PID:-?} filter=$WFILTER_SUB (원격 /tmp/mn_watchdog_slave.log)"
  else
    echo "[mn] ⚠ 서브에 $SUB_WATCHDOG_REL 부재 — 슬레이브 워치독 생략(상시 systemd 층만. render_sub_env/sync_to_sub 재배달 필요)"
  fi
fi

# ── 이 실행이 무장한 워치독만 해제한다 (2026-08-18 신설) ──────────────────────────────
# 워치독은 **예산 게이트보다 먼저** 무장한다(위). 그런데 예산 게이트의 세 STOP 경로(derive 실패 ·
# preflight_ceiling · declare_not_honored)는 전부 맨 `exit 4` 였다 — 컨테이너는 하나도 안 떴는데
# 워치독만 양 노드에 남는다. 2026-08-18 실측: `ds4f0731-x2-sm12x` 가 preflight_ceiling 에서 멈춘 뒤
# main pid=3207947 · sub pid=2614745 가 대상 0개인 채 상주했고, `verify_node_blackbox` 가
# `no_zombie` 로 이를 잡았다(28통과/1실패).
#   이 누락이 오래 살아남은 이유는 **다음 실행의 `reap_stale_watchdogs` 가 조용히 치워줬기** 때문이다 —
#   증상이 다음 실행에서 사라지므로 아무도 원인을 안 본다. workflow.md 막힘 3분류의 **침묵 누락**이고,
#   `teardown_serve`·`reap_stale_watchdogs` 라는 배선이 **이미 있는데 호출자가 없던** 경우다.
# ⚠ 여기서 teardown_serve 를 부르지 않는다 — 그건 down·drop-caches·예산회수까지 하는데, 이 시점엔
#   띄운 것도 선언된 것도 없다(각 STOP 이 "로드는 0초도 시작하지 않았다"고 말한다). 무장 해제만이 맞다.
disarm_armed_watchdogs(){
  [ -n "$WD_MAIN_PID" ] && kill "$WD_MAIN_PID" 2>/dev/null && echo "[mn] 워치독 해제(master pid=$WD_MAIN_PID) — 이 실행이 무장한 것"
  # ⚠ 슬레이브는 **PID 로 못 죽인다**. 무장이 `setsid nohup bash … & echo $!` 라 $! 는 setsid 의 PID 이고
  #   실제 워치독은 그 자식이다(2026-08-18 실측: 캡처 2626758 vs 실제 2626760). argv **위치** 대조로 회수한다 —
  #   같은 파일의 reap_stale_watchdogs 가 이미 그 방식이고, teardown_serve 의 smoke 분기도 같은 PID 결함을
  #   갖고 있었다(회수가 조용히 실패하고 다음 실행의 reap 이 치워줘 증상이 사라졌다).
  [ -n "$WD_SUB_PID" ] && reap_stale_watchdogs slave
  return 0
}

# ══ 서빙 예산 선언 — **로드 개시 전** 필수 단계 (plan_26081415 C3 · 궁극 교정) ══════════════
#
#   왜 여기 있나: 처방(선언된 바닥)은 2026-08-01 에 이미 도입됐고 설계대로 작동했다. 그런데
#   R0(2026-08-14)에서 3회차에야 적용됐다 — **사람이 손으로 쳐야만 발동하는 단계로 남아 있었기
#   때문**이다. 그 사이 정상 모델 로드가 2회 사살됐다(잔량 50,772 / 35,158 MiB). 즉 결함은
#   규칙이 아니라 **배선의 부재**였다: 전 코드베이스에서 declare-budget 을 호출하는 스크립트가 0개.
#   workflow.md 평면 B 일반명제의 관측-데이터-평면 인스턴스 — *"게이트의 처방이 가리키는 주체가
#   아키텍처에 없으면 그 게이트는 안전장치가 아니라 교착이다."* 없던 주체는 "선언을 발행하는 스모크"다.
#
#   순서가 판정 기준이다: budget_declare → budget_honored → **그 다음** 컨테이너 up.
#   사후 발행은 무의미하다(로드 골짜기는 이미 지나갔다).
#
#   ★ 입력은 전부 **파생**한다. 손으로 적으면 KV 클램프를 바꿨을 때 선언만 옛 값으로 남아
#     예상 바닥이 틀리고, 워치독이 엉뚱한 지점에서 무장한다(4종 안티패턴 `매직넘버·결함`).
#       weights_mib ← check_smoke_model 의 BUDGET_PARAMS ckpt_mib ÷ tp   (노드당 몫)
#       kv_mib      ← 같은 곳의 kv_mib (트리플렛 yaml `kv-cache-memory-bytes`)
#       mem_total   ← 각 노드의 /proc/meminfo MemTotal
#     검산(2026-08-14): ckpt 159,155 ÷ tp 2 = 79,577 MiB — R0 이 손으로 넣었던 79,578 과 일치한다.
BUDGET_LABEL="smoke-${CONFIG}"
BUDGET_DECLARED=0
budget_node_dir_main(){ printf '%s/docs/logs/%s' "$REPO" "$MAIN_NODE_ID"; }
budget_node_dir_sub(){  printf '%s/docs/logs/%s' "$SUB_WORK_DIR" "$SUB_NODE_ID"; }
SUB_SESSION_PY="$SUB_WORK_DIR/.claude/runtime/node_blackbox/blackbox_session.py"
MAIN_SESSION_PY="$REPO/.claude/skills/terraforming_node/scripts/node_blackbox/blackbox_session.py"
MAIN_RENEW_SH="$REPO/.claude/skills/terraforming_node/scripts/node_blackbox/budget_renew_loop.sh"
SUB_RENEW_SH="$SUB_WORK_DIR/.claude/runtime/node_blackbox/budget_renew_loop.sh"
MAIN_REGEN_PY="$REPO/.claude/skills/terraforming_node/scripts/node_blackbox/regen_envelope.py"
SUB_REGEN_PY="$SUB_WORK_DIR/.claude/runtime/node_blackbox/regen_envelope.py"
NOW_ISO(){ date -u +%FT%TZ; }

# ── 서빙 종료(단일 소유) ─────────────────────────────────────────────────────────────────────
#   2026-08-16 함수화. 이 5단계는 원래 `KEEP != 1` 분기 **안에만** 있었고, 그래서 `--keep-up` 상주분을
#   나중에 내리는 경로가 없었다. 두 진입점(스모크 자체 정리 · `--down`)이 **같은 로직**을 쓰게 한다 —
#   두 벌로 나누면 갈라지고, 갈라진 목록이 침묵 누락을 만든다는 것이 이 레포의 반복된 실증이다.
#
#   $1 = smoke      : 스모크가 자기 실행에서 띄운 것을 내린다(WD PID 를 안다).
#        standalone : `--down` — 다른 프로세스가 띄운 상주분을 내린다(WD PID 를 모른다 · 선언은 무조건 회수).
teardown_serve(){
  local mode="${1:?teardown_serve <smoke|standalone>}"
  echo "[mn] 정리(양 노드 down)..."
  docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master down >/dev/null 2>&1
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave down'" >/dev/null 2>&1
  # 워치독 정지 = PID 기반만(pkill -f 금지) → 잔여 페이지캐시 드랍(§4.1 ② — 헬퍼 설치 시 best-effort).
  if [ "$mode" = "standalone" ]; then
    # PID 를 모르므로 argv **위치** 대조로 회수한다(부분문자열 매칭 금지 — 위 회수 계약 주석 참조).
    reap_stale_watchdogs master
    reap_stale_watchdogs slave
  else
    [ -n "$WD_MAIN_PID" ] && kill "$WD_MAIN_PID" 2>/dev/null
    # 슬레이브만 argv 대조 — 위 disarm_armed_watchdogs 주석의 setsid PID 결함(2026-08-18)과 동일 사유.
    [ -n "$WD_SUB_PID" ] && reap_stale_watchdogs slave
  fi
  # 예산 갱신 루프(W-7)도 같은 자리에서 회수한다 — 선언을 곧 지울 것이므로 갱신자가 남으면
  # 지워진 선언을 되살리려다 fail-loud 로 죽는다(회수 순서: 루프 정지 → clear-budget).
  reap_renew_loops master
  reap_renew_loops slave
  [ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches >/dev/null 2>&1
  $SSH "$SUB_HOST" "[ -x /usr/local/sbin/vllm-drop-caches ] && sudo -n /usr/local/sbin/vllm-drop-caches" >/dev/null 2>&1
  # 예산 회수(C3-4). 서빙을 내렸으면 선언도 내린다 — 남겨 두면 다음 로드가 **남의 바닥**으로 무장한다.
  #   standalone 은 이 실행이 선언한 바가 없으므로 `BUDGET_DECLARED` 가 0 이다. 그래도 **무조건** 회수한다 —
  #   내리려는 그 서빙의 선언은 다른 실행이 남긴 것이고, 그것을 남기는 것이 정확히 이 함수가 고치는 결함이다.
  #   clear-budget 은 멱등이라 선언이 없으면 그렇게 보고하고 끝난다.
  if [ "$mode" = "standalone" ] || [ "$BUDGET_DECLARED" = "1" ]; then
    python3 "$MAIN_SESSION_PY" --node-dir "$(budget_node_dir_main)" clear-budget --now "$(NOW_ISO)" 2>&1 | sed 's/^/[mn] 예산회수(main): /'
    [ -n "$SUB_NODE_ID" ] && timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD python3 $SUB_SESSION_PY --node-dir $SUB_WORK_DIR/docs/logs/$SUB_NODE_ID clear-budget --now $(NOW_ISO)'" 2>&1 | sed 's/^/[mn] 예산회수(sub): /'
  fi
}

# ── `--down` 진입점: 여기까지가 변수 파생이고, 아래부터가 기동이다. 내리기만 할 때는 여기서 끝낸다. ──
if [ "$DOWN" = "1" ]; then
  echo "[mn] --down: 상주 서빙 회수(컨테이너 · 워치독 · 페이지캐시 · 예산선언) — 기동·스모크 없음"
  echo "[mn]   대상: $MC / ${SLVC:-slave} · compose=output/multi/docker-compose.yaml · config=$CONFIG"
  teardown_serve standalone
  echo "[mn] --down 완료. 재기동은 인자에서 --down 을 빼고 실행하라."
  echo "[mn] 종료코드 0"
  exit 0
fi

# ── 진입 차단 기록 (plan_26081415 C3 기준3 · 침묵 금지) ──────────────────────────────────────
#   아래 세 차단 경로는 stdout 에만 남아 있었다. stdout 은 이 셸이 끝나면 사라지고, 노드블랙박스
#   `events/` 는 **영구**다(docs.md §기계판독 데이터 평면). 즉 "그날 왜 서빙이 0초도 안 떴나"를
#   나중에 물을 수 있는 유일한 곳에 기록이 없었다 — `budget_skipped`(무보호로 **진입함**)만 남고
#   `budget_blocked`(**진입 못 함**)는 없는 반쪽 관측이었고, 그 둘은 처방이 정반대다.
#   ★ 양노드에 남긴다. 서브 events 만 보는 분석자에게 "그날 아무 일도 없었다"로 보이면 안 된다.
#   ★ 기록 실패도 말한다 — 조용히 못 남기면 침묵 금지가 한 겹 더 깨진다.
budget_block_event(){  # $1=stage $2=reason-slug · $3.. = key=value detail
  local stage="$1" reason="$2"; shift 2
  local dargs="" d now out sid
  for d in "$@"; do dargs="$dargs --detail $d"; done
  now="$(NOW_ISO)"
  # shellcheck disable=SC2086  # dargs 는 우리가 만든 무공백 토큰열이라 분리가 의도다
  if out=$(python3 "$MAIN_SESSION_PY" --node-dir "$(budget_node_dir_main)" budget-block \
             --stage "$stage" --reason "$reason" --label "$BUDGET_LABEL" $dargs \
             --now "$now" 2>&1); then
    printf '%s\n' "$out" | sed 's/^/[mn]   main: /'
  else
    echo "[mn]   main: ⚠ 차단 이벤트 기록 실패 — $out"
  fi
  # 서브 경로는 manifest role 에서 이미 확정돼 있다(왕복 0회). 실패는 SSH 자체 실패뿐이고,
  # 그때도 "미기록"을 **명시**한다 — 조용히 넘기면 "그날 아무 일도 없었다"와 구분되지 않는다.
  if out=$(timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD if [ -f $SUB_SESSION_PY ]; then python3 $SUB_SESSION_PY --node-dir $(budget_node_dir_sub) budget-block --stage $stage --reason $reason --label $BUDGET_LABEL$dargs --now $now; else echo \"$SUB_SESSION_PY 부재\"; exit 3; fi'" 2>&1); then
    printf '%s\n' "$out" | sed 's/^/[mn]   sub: /'
  else
    echo "[mn]   sub: ⚠ 차단 이벤트 미기록 — $out"
  fi
}

# ── 포락선 신선도 경고 (plan_26081415 C2-4) — **차단이 아니라 경고** ────────────────────────
#   왜 차단하지 않나: 포락선은 판정 파라미터의 **근거 데이터**지 판정 자체가 아니다. 낡았다고
#   서빙을 막으면 안전과 무관한 이유로 진입이 죽는다. 반대로 조용히 넘기면 §1.2 가 다시 반복된다 —
#   14일 동결된 seed 포락선이 "반영되고 있다"고 오인됐고, 아무도 그 사실을 몰랐다.
#   ★ `--no-budget` 여부와 **무관하게** 돈다. 신선도는 예산 정책이 아니라 데이터 사실이다.
#   시각은 --now 주입만(벽시계 금지 — docs.md §기계판독 데이터 평면).
ENVELOPE_MAX_AGE_DAYS="${ENVELOPE_MAX_AGE_DAYS:-7}"
if [ -f "$MAIN_REGEN_PY" ]; then
  python3 "$MAIN_REGEN_PY" --node-dir "$(budget_node_dir_main)" staleness \
    --now "$(NOW_ISO)" --max-age-days "$ENVELOPE_MAX_AGE_DAYS" 2>&1 | sed 's/^/[mn] 포락선(main): /'
  # 부재를 조용히 넘기지 않는다 — "점검했고 괜찮았다"와 "점검 자체를 못 했다"는 다른 사실이다.
  timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD if [ -f $SUB_REGEN_PY ]; then python3 $SUB_REGEN_PY --node-dir $(budget_node_dir_sub) staleness --now $(NOW_ISO) --max-age-days $ENVELOPE_MAX_AGE_DAYS; else echo \"⚠ $SUB_REGEN_PY 부재 — 미점검(render_sub_env/sync_to_sub 재배달 필요)\"; fi'" 2>&1 \
    | sed 's/^/[mn] 포락선(sub): /'
else
  echo "[mn] ⚠ $MAIN_REGEN_PY 부재 — 포락선 신선도 미점검(경고만)."
fi

# `budget_honored` 는 **상시 ETA 워치독**(systemd)이 선언 파일을 재평가해 찍는다. 그런데 그 로그는
# **상태 전이에서만** 나온다(refresh_decl: state != BB_DECL_STATE) — 이미 honored 상태에서 새 선언을
# 얹으면 전이가 없어 이벤트가 안 나온다. 그래서 clear → declare 순서로 전이를 강제한다.
wait_budget_event(){  # $1=events path $2=main|sub $3=epoch lower bound $4=kind
  local f="$1" where="$2" t0="$3" kind="$4" i line ets
  for i in $(seq 1 40); do
    if [ "$where" = "sub" ]; then
      line=$(timeout 15 $SSH -n "$SUB_HOST" "tail -30 '$f' 2>/dev/null | grep -F '\"$kind\"' | tail -1" 2>/dev/null || true)
    else
      line=$(tail -30 "$f" 2>/dev/null | grep -F "\"$kind\"" | tail -1 || true)
    fi
    if [ -n "$line" ]; then
      ets=$(printf '%s' "$line" | sed -n 's/.*"ts":"\([^"]*\)".*/\1/p')
      if [ -n "$ets" ] && [ "$(date -u -d "$ets" +%s 2>/dev/null || echo 0)" -ge "$t0" ]; then
        printf '%s' "$line"
        return 0
      fi
    fi
    sleep 1
  done
  return 1
}

# ★ 2026-09-11(plan_26091108 R1): 종전 조건은 `BUDGET=1 ∧ WATCHDOG=1` 이었다. 그래서
#   `--no-watchdog` 하나가 **예산 선판정 게이트까지 함께 껐다**. 그런데 `--no-watchdog` 가 끄는
#   것은 *이 스크립트의 arming* 이지 **사살자가 아니다** — 죽인 것은 상주 systemd 유닛
#   (`unit:easy-vllm-memwatch`·`unit:easy-vllm-blackbox-watchdog`)이고 지금도 active 다.
#   즉 종전 결합은 **위험은 그대로 둔 채 가드만 걷어내는** 방향이었다. camp-26090918 의 사살
#   3건이 정확히 그 상태에서 났다(원장에 budget_declare 도 budget_skipped 도 없다).
#   선판정(산술)은 BUDGET=1 이면 항상 돌고, 선언(arming)만 WATCHDOG 에 종속시킨다.
if [ "$BUDGET" = "1" ]; then
  CKPT_MIB=$(printf '%s\n' "$NAS_OUT" | grep -oE 'ckpt_mib=[0-9]+' | head -1 | cut -d= -f2)
  BTP=$(printf '%s\n' "$NAS_OUT"      | grep -oE 'BUDGET_PARAMS ckpt_mib=[0-9]+ tp=[0-9]+' | grep -oE 'tp=[0-9]+' | cut -d= -f2)
  KV_MIB=$(printf '%s\n' "$NAS_OUT"   | grep -oE 'kv_mib=[0-9]+' | head -1 | cut -d= -f2)
  # 2026-09-05(G-B1): 기본값 12288 삭제 → **선언 필수**. 그 값은 "안전측"이 아니었다 —
  #   낮게 잡으면 선언 바닥이 높아져 워치독 arm 상한이 정상 서빙 위로 올라간다(실측 17,971).
  OVERHEAD_MIB="${SMOKE_BUDGET_OVERHEAD_MIB:-}"
  OVERHEAD_SOURCE="env SMOKE_BUDGET_OVERHEAD_MIB"
  if [ -z "$OVERHEAD_MIB" ]; then
    OVERHEAD_MIB="$(_campaign_budget smoke_budget_overhead_mib)"
    OVERHEAD_SOURCE="캠페인 budgets.smoke_budget_overhead_mib"
  fi
  [ -n "$OVERHEAD_MIB" ] && echo "[mn] overhead 출처: $OVERHEAD_SOURCE = ${OVERHEAD_MIB}MiB"
  if [ -z "$OVERHEAD_MIB" ]; then
    echo "[mn] FAIL: SMOKE_BUDGET_OVERHEAD_MIB 가 선언되지 않았다 — 예산 overhead 에 기본값을 쓰지 않는다." >&2
    echo "     왜: overhead 를 낮게 잡으면 선언 바닥이 높아져 정상 서빙이 무장 밴드에 들어간다(사살 실적)." >&2
    echo "     어떻게: SMOKE_BUDGET_OVERHEAD_MIB=<n> 로 넘겨라. 모르면 로드 완료 후" >&2
    echo "     (MemTotal − MemAvailable) − weights − kv 를 재서 그 값을 쓴다." >&2
    exit 4
  fi
  # TTL 파생: 스모크 자신의 로드 타임아웃(READY_MAX×5s)의 3배 — 로드 도중 만료를 구조적으로 배제한다.
  #   현행 기본 7200s 는 하한으로 남긴다(둘 중 큰 값). 상한 86400 은 blackbox_session 이 강제한다.
  READY_BUDGET_S=$READY_WINDOW_S
  # TTL 하한은 **단일 소유자**(blackbox_session)에게 묻는다 — 여기에 숫자를 다시 적지 않는다(G-B2).
  _TTL_FLOOR="$(python3 "$MAIN_SESSION_PY" --node-dir . budget-defaults --field ttl_s 2>/dev/null || echo)"
  case "$_TTL_FLOOR" in ''|*[!0-9]*) echo "[mn] FAIL: 예산 TTL 기본값을 blackbox_session 에서 읽지 못했다" >&2; exit 4;; esac
  BUDGET_TTL_S=$(( READY_BUDGET_S * 3 )); [ "$BUDGET_TTL_S" -lt "$_TTL_FLOOR" ] && BUDGET_TTL_S="$_TTL_FLOOR"
  [ "$BUDGET_TTL_S" -gt 86400 ] && BUDGET_TTL_S=86400

  if [ -z "$CKPT_MIB" ] || [ -z "$BTP" ] || [ -z "$KV_MIB" ] || [ "$BTP" -eq 0 ] 2>/dev/null; then
    echo "[mn] FAIL(예산): 선언 입력 파생 실패 — BUDGET_PARAMS 미검출(ckpt_mib='$CKPT_MIB' tp='$BTP' kv_mib='$KV_MIB')."
    echo "     원인 후보: 트리플렛에 kv-cache-memory-bytes 미선언(policy:KV_ABSOLUTE_CLAMP_PORTABILITY) 또는 ckpt 산출 실패."
    echo "     ⇒ 로드는 **0초도 시작하지 않았다**. KV 절대클램프를 트리플렛에 명시하거나, 무보호를 감수하려면 --no-budget."
    budget_block_event derive budget_params_missing \
      "ckpt_mib_found=$([ -n "$CKPT_MIB" ] && echo 1 || echo 0)" \
      "tp_found=$([ -n "$BTP" ] && echo 1 || echo 0)" \
      "kv_mib_found=$([ -n "$KV_MIB" ] && echo 1 || echo 0)"
    disarm_armed_watchdogs
    exit 4
  fi
  # ── PLE mmap 보정(2026-09-11 · plan_26091108 R2) ────────────────────────────────────────
  #   weights 는 "이 노드에 **상주할** 바이트" 여야 한다. 종전 `ckpt ÷ tp` 는 체크포인트 파일
  #   크기 기준이라, PLE n-gram 47.7GiB 가 NVMe mmap 으로 빠져도 같은 값이 나왔다. 그래서 게이트는
  #   `fp8+res`(막아야 옳다)와 `fp8+mmp`(camp-26090918 에서 **실제로 서빙 성공**)를 동일 계산해
  #   둘 다 막는다 — 게이트를 그대로 켜면 뜨는 셀을 죽인다.
  #   모드의 권위는 **셀 env 의 `VLLM_PLE_MMAP`**(serve-time 사실)이고, 크기의 권위는
  #   `check_smoke_model` 이 index 에서 파생한 `ple_mib` 다. 둘 중 하나라도 없으면 **보정 0**
  #   (= 현행 동작). 부재를 추측으로 메우면 게이트가 없는 여유를 있다고 말한다.
  PLE_MIB=$(printf '%s\n' "$NAS_OUT" | grep -oE 'ple_mib=[0-9]+' | head -1 | cut -d= -f2)
  PLE_MODE_ON=0
  case "$PLEVARS" in *VLLM_PLE_MMAP=1*) PLE_MODE_ON=1;; esac
  RESIDENT_CKPT_MIB="$CKPT_MIB"; PLE_NOTE="ple=resident"
  if [ "$PLE_MODE_ON" = "1" ]; then
    if [ -n "$PLE_MIB" ] && [ "$PLE_MIB" -gt 0 ] 2>/dev/null; then
      RESIDENT_CKPT_MIB=$(( CKPT_MIB - PLE_MIB ))
      PLE_NOTE="ple=mmap(−${PLE_MIB}MiB · index 파생)"
    else
      # 선언은 켜졌는데 크기를 못 구했다 — **보정하지 않는다**. 그 사실을 침묵시키지 않는다.
      PLE_NOTE="ple=mmap선언·크기미상(보정 0 — 게이트가 과보수적으로 판정한다)"
      echo "[mn] ⚠ VLLM_PLE_MMAP=1 인데 ple_mib 를 파생하지 못했다(값='$PLE_MIB') — 보정 없이 진행한다." >&2
    fi
  fi
  WEIGHTS_MIB=$(( RESIDENT_CKPT_MIB / BTP ))
  echo "[mn] 예산 파생: weights=${WEIGHTS_MIB}MiB(상주 ckpt ${RESIDENT_CKPT_MIB}÷tp ${BTP} · ${PLE_NOTE}) kv=${KV_MIB}MiB overhead=${OVERHEAD_MIB}MiB ttl=${BUDGET_TTL_S}s"

  # ── 선판정: arm 상한을 **선언 전에** 계산해 거부를 예고한다(plan_26081415 §1.3 구조적 상한).
  #   워치독의 `decl_min_ceiling_mib` 하드가드는 `floor - 8192 < 16384` 인 선언을 거부한다 —
  #   즉 **예상 상주가 커서 바닥이 24,576 MiB 밑으로 내려가는 로드는 선언으로 보호할 수 없다.**
  #   이걸 선언 후 15초 폴링으로 알게 하면 "왜 막혔는지" 가 불투명해진다. 숫자로 미리 말한다.
  #   실측(2026-08-14 · ds4f0731-x2): 파생 기본 overhead 12,288 → 상한 16,361 = 가드에 **23 MiB 부족**.
  #   R0 이 손으로 넣은 11,264 는 통과했다(17,385). 경계가 이만큼 얇다는 사실 자체가 판정 재료다.
  #   ★ 2026-09-05(G-B3): 두 상수의 **거울을 삭제**했다. 종전 주석은 "값을 파생할 통로가 없어
  #     tripwire 로 둔다"고 했지만 통로는 있었다 — 정본 `blackbox_eta.DEFAULTS` 는 이 노드의
  #     같은 저장소 안에 있고 import 하면 된다. 거울로 둔 대가는 이미 치렀다(2026-08-18 정본이
  #     움직이자 손으로 따라가야 했고, 워치독 셸의 거울은 **따라가지 못해 옛값으로 남았다**).
  # ★ 2026-09-11(plan_26091108 R1·R2): 선판정 **산술의 단일 소유**는 `budget_preflight.py` 다.
  #   종전에는 이 셸이 floor·ceiling·overhead 상한을 직접 계산했고, 같은 산술이
  #   `single_serve_up.sh`(선판정 자체가 없었다)와 벤치 진입에도 필요했다 — 세 자리에 적으면
  #   반드시 갈린다. 상수(decl_margin·decl_min_ceiling·abs_band)도 그 안에서 정본 import 한다.
  # ★ 2026-09-14(plan_26091407 §4.3): 서빙되는 yaml 의 gpu-memory-utilization 을 기재 입력으로 넘겨
  #   예상 vLLM 몫(gmu × MemTotal)·잔차(몫 − weights − kv)를 **기재만** 받는다 — arm 산식·종료코드는 불변(게이트 ✗).
  #   yaml 에 수치가 없으면 기재 행이 없다(실패 경로를 새로 만들지 않는다). 이 스크립트는 캠페인을 모르는
  #   계층이라 target_gmu 와의 대조는 여기서 하지 않는다 — 대조는 campaign_init --cell-set 이 기재한다.
  #   ★ 2026-09-14(⑧ 분석 발견 T3): yaml → gmu 추출 규칙은 budget_preflight.py(`--declared-gmu-yaml`)가 단일 소유한다 —
  #     이 셸에 있던 추출 함수를 싱글 기동에 옮겨 적으면 같은 규칙이 두 셸에 손으로 적히므로 경로 한 줄만 넘긴다.
  _PF="$(python3 "$SDIR/budget_preflight.py" --json --declared-gmu-yaml "output/multi/configs/${CONFIG}.yaml" \
          --mem-total-mib "$(awk '/MemTotal:/{print int($2/1024)}' /proc/meminfo)" \
          --weights-mib "$WEIGHTS_MIB" --kv-mib "$KV_MIB" --overhead-mib "$OVERHEAD_MIB" 2>&1)"
  _PF_RC=$?
  if [ "$_PF_RC" != "0" ] && [ "$_PF_RC" != "4" ]; then
    echo "[mn] FAIL: 예산 선판정을 수행하지 못했다(rc=$_PF_RC) — $_PF" >&2
    exit 4
  fi
  _pf(){ printf '%s' "$_PF" | python3 -c "import json,sys;print(json.load(sys.stdin).get(sys.argv[1]))" "$1" 2>/dev/null; }
  _PRED_FLOOR="$(_pf floor_mib)"; _PRED_CEIL="$(_pf arm_ceiling_mib)"
  _OH_MAX="$(_pf overhead_max_mib)"; _WD_MIN_CEIL="$(_pf decl_min_ceiling_mib)"
  _MEMTOT_MAIN=$(awk '/MemTotal:/{print int($2/1024)}' /proc/meminfo)
  case "${_PRED_FLOOR}${_PRED_CEIL}" in ''|*None*)
    echo "[mn] FAIL: 선판정 산출을 읽지 못했다 — $_PF" >&2; exit 4;;
  esac
  echo "[mn] 예산 선판정: 예상 바닥=${_PRED_FLOOR}MiB → arm 상한=${_PRED_CEIL}MiB (가드 최소 ${_WD_MIN_CEIL}MiB)"
  echo "[mn] declared-gmu 기재(게이트 ✗): $(printf '%s' "$_PF" | python3 -c "
import json,sys
r=json.load(sys.stdin).get('declared_gmu_row')
print('서빙 yaml 에 gpu-memory-utilization 수치 없음 — 기재 행 없음' if r is None else
      'gmu=%s 예상 vLLM 몫=%sMiB 잔차(몫−weights−kv)=%sMiB provenance=%s status=%s 전제=%s'
      % (r.get('gmu'), r.get('expected_vllm_share_mib'), r.get('residual_mib'), r.get('provenance'), r.get('status'),
         str(r.get('premise') or '').split(' — ', 1)[0]))
" 2>/dev/null || echo '기재 행을 읽지 못했다(판정 무관)')"
  if [ "$_PF_RC" = "4" ]; then
    echo "[mn] STOP(예산 게이트): 워치독이 이 선언을 **거부**한다 — 로드는 0초도 시작하지 않았다."
    printf '%s' "$_PF" | python3 -c "
import json,sys
for r in (json.load(sys.stdin).get('reasons') or []): print('     · %s' % r)
" 2>/dev/null || true
    # 숫자를 그대로 남긴다 — 경계가 23 MiB 로 얇았다는 사실(2026-08-14)이 판정 재료였고,
    # 그건 "얼마나 모자랐나"가 데이터에 있어야만 다음 사람이 다시 알 수 있다.
    budget_block_event preflight_ceiling arm_ceiling_below_min \
      "pred_floor_mib=$_PRED_FLOOR" "pred_ceiling_mib=$_PRED_CEIL" \
      "min_ceiling_mib=$_WD_MIN_CEIL" "short_by_mib=$(( _WD_MIN_CEIL - _PRED_CEIL ))" \
      "weights_mib=$WEIGHTS_MIB" "kv_mib=$KV_MIB" "overhead_mib=$OVERHEAD_MIB" \
      "ple_mib=${PLE_MIB:-unknown}" "ple_mmap=$PLE_MODE_ON" \
      "overhead_max_mib=$_OH_MAX" "mem_total_mib=$_MEMTOT_MAIN"
    disarm_armed_watchdogs
    exit 4
  fi

  declare_one(){ # $1=main|sub → 0=honored · 비0=실패(사유는 표준출력)
    local where="$1" nd py memtot t0 ev out hon had_decl
    if [ "$where" = "main" ]; then
      nd="$(budget_node_dir_main)"; py="$MAIN_SESSION_PY"
      memtot=$(awk '/MemTotal:/{print int($2/1024)}' /proc/meminfo)
    else
      nd="$(budget_node_dir_sub)"; py="$SUB_SESSION_PY"
      memtot=$(timeout 15 $SSH -n "$SUB_HOST" "awk '/MemTotal:/{print int(\$2/1024)}' /proc/meminfo" 2>/dev/null || true)
      [ -n "$memtot" ] || { echo "[mn]   sub: MemTotal 조회 실패"; return 1; }
    fi
    ev="$nd/events/watchdog.jsonl"
    local clear_t0
    clear_t0=$(date +%s)
    if [ "$where" = "main" ]; then
      [ -f "$nd/serve_budget.env" ] && had_decl=1 || had_decl=0
      python3 "$py" --node-dir "$nd" clear-budget --now "$(NOW_ISO)" >/dev/null 2>&1
      if [ "$had_decl" = 1 ]; then
        wait_budget_event "$ev" main "$clear_t0" budget_none >/dev/null || {
          echo "[mn]   main: clear-budget 상태 전이 미관측"; return 1; }
      fi
      t0=$(date +%s)
      out=$(python3 "$py" --node-dir "$nd" declare-budget --mem-total-mib "$memtot" \
              --weights-mib "$WEIGHTS_MIB" --kv-mib "$KV_MIB" --overhead-mib "$OVERHEAD_MIB" \
              --ttl-s "$BUDGET_TTL_S" --expected-load-s "$READY_BUDGET_S" \
              --label "$BUDGET_LABEL" --now "$(NOW_ISO)" 2>&1) || {
        echo "[mn]   main: declare-budget 실패 — $out"; return 1; }
    else
      had_decl=$(timeout 15 $SSH -n "$SUB_HOST" "test -f '$nd/serve_budget.env' && echo 1 || echo 0" 2>/dev/null || echo 0)
      timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD python3 $py --node-dir $nd clear-budget --now $(NOW_ISO)'" >/dev/null 2>&1 || {
        echo "[mn]   sub: clear-budget 실패"; return 1; }
      if [ "$had_decl" = 1 ]; then
        wait_budget_event "$ev" sub "$clear_t0" budget_none >/dev/null || {
          echo "[mn]   sub: clear-budget 상태 전이 미관측"; return 1; }
      fi
      t0=$(date +%s)
      out=$(timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD python3 $py --node-dir $nd declare-budget --mem-total-mib $memtot --weights-mib $WEIGHTS_MIB --kv-mib $KV_MIB --overhead-mib $OVERHEAD_MIB --ttl-s $BUDGET_TTL_S --expected-load-s $READY_BUDGET_S --label $BUDGET_LABEL --now $(NOW_ISO)'" 2>&1) || {
        echo "[mn]   sub: declare-budget 실패 — $out"; return 1; }
    fi
    printf '%s\n' "$out" | sed "s/^/[mn]   $where: /"
    if hon=$(wait_budget_event "$ev" "$([ "$where" = sub ] && echo sub || echo main)" "$t0" budget_honored); then
      echo "[mn]   $where: budget_honored ✓ $hon"
      return 0
    fi
    echo "[mn]   $where: budget_honored 미검출(40s) — 선언은 썼으나 워치독이 수락하지 않았다."
    echo "[mn]   $where: 확인 → systemctl is-active easy-vllm-blackbox-watchdog · tail $ev"
    echo "[mn]   $where: 흔한 원인 = arm 상한(floor-8192)이 최소 16384MiB 미만 → 워치독이 거부(선언으로 게이트를 실명시킬 수 없다)"
    return 1
  }

  if [ "$WATCHDOG" != "1" ]; then
    # 선언의 소비자(워치독 arming)를 끈 실행이다 — 선언은 생략하되 **그 사실을 원장에 남긴다**.
    #   종전에는 여기서 stdout 한 줄만 찍고 끝났다. 그러면 나중에 "이 서빙이 게이트를 지났나"를
    #   물을 자리가 **없다**(탈출구가 조용하면 탈출구가 아니라 구멍이다).
    echo "[mn] ⚠ --no-watchdog: 예산 **선언**을 생략한다. 선판정은 위에서 이미 통과했다."
    echo "[mn]   주의: 이 플래그는 사살자를 끄지 않는다 — 상주 유닛(easy-vllm-memwatch ·"
    echo "[mn]   easy-vllm-blackbox-watchdog)은 계속 돌고, 선언이 없으면 arm 상한이 무제한이라"
    echo "[mn]   **정상 로드가 사살 대상이 된다**(camp-26090918 실측 3건)."
    python3 "$MAIN_SESSION_PY" --node-dir "$(budget_node_dir_main)" budget-skip \
      --reason=--no-watchdog --label "$BUDGET_LABEL" --now "$(NOW_ISO)" 2>&1 | sed 's/^/[mn]   main: /'
    timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD python3 $SUB_SESSION_PY --node-dir $(budget_node_dir_sub) budget-skip --reason=--no-watchdog --label $BUDGET_LABEL --now $(NOW_ISO)'" 2>&1 | sed 's/^/[mn]   sub: /' || true
  else
  echo "[mn] 예산 선언(양노드 · 로드 개시 전)..."
  BUD_RC=0; BUD_MAIN_OK=1; BUD_SUB_OK=1; BUD_SUB_MISSING=0
  declare_one main || { BUD_RC=1; BUD_MAIN_OK=0; }
  # ★ 서브도 **대칭** 발행한다. R0 시도②에서 서브 노드도 트립했다 — 메인만 선언하면 절반만 보호된다.
  if $SSH "$SUB_HOST" "bash -lc '[ -f $SUB_SESSION_PY ]'" 2>/dev/null; then
    declare_one sub || { BUD_RC=1; BUD_SUB_OK=0; }
  else
    echo "[mn]   sub: $SUB_SESSION_PY 부재 — 서브 선언 불가(render_sub_env/sync_to_sub 재배달 필요)"
    BUD_RC=1; BUD_SUB_OK=0; BUD_SUB_MISSING=1
  fi
  if [ "$BUD_RC" != "0" ]; then
    echo "[mn] STOP(예산 게이트): 예산 선언이 양노드에서 성립하지 않았다 — **진입 차단**(plan_26081415 C3 정책 ㄴ)."
    echo "     로드는 **0초도 시작하지 않았다**. 무보호로 진행하려면 --no-budget 을 명시하라(그 사실이 이벤트로 남는다)."
    # 어느 노드가 못 섰는지를 남긴다 — "양노드 실패"와 "서브만 실패"는 다음 행동이 다르다
    # (전자는 입력·규칙 교정, 후자는 서브 재배달). 선언 자체의 거부 사유는 그 노드의
    # `budget_declare_rejected` 가 이미 갖고 있으므로 여기서 중복 서술하지 않는다.
    budget_block_event declare declare_not_honored \
      "main_ok=$BUD_MAIN_OK" "sub_ok=$BUD_SUB_OK" "sub_session_py_missing=$BUD_SUB_MISSING" \
      "weights_mib=$WEIGHTS_MIB" "kv_mib=$KV_MIB" "overhead_mib=$OVERHEAD_MIB" \
      "ttl_s=$BUDGET_TTL_S"
    disarm_armed_watchdogs
    exit 4
  fi
  BUDGET_DECLARED=1
  echo "[mn] 예산 선언 완료 — 이제 로드를 개시한다(순서: declare→honored→up)."
  fi
else
  # 탈출구가 조용하면 탈출구가 아니라 구멍이다. 양노드에 기록을 남긴다.
  echo "[mn] ⚠ --no-budget: **무보호 진입** — ETA 워치독이 현행 규칙 그대로 돈다(대형 로드 사살 위험)."
  # `--reason=<v>` 등호형을 쓴다 — 공백형이면 argparse 가 값 `--no-budget` 을 **옵션으로** 읽어 죽는다.
  python3 "$MAIN_SESSION_PY" --node-dir "$(budget_node_dir_main)" budget-skip \
    --reason=--no-budget --label "$BUDGET_LABEL" --now "$(NOW_ISO)" 2>&1 | sed 's/^/[mn]   main: /'
  timeout 30 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD python3 $SUB_SESSION_PY --node-dir $(budget_node_dir_sub) budget-skip --reason=--no-budget --label $BUDGET_LABEL --now $(NOW_ISO)'" 2>&1 | sed 's/^/[mn]   sub: /'
fi

# ── Ray 클러스터 기동 (master 먼저=head, slave 합류) ──
_SERVE_T0="$(date -u +%FT%TZ)"   # serve 진행표 착수 시각(P4 가 읽는 first_started_utc 의 원천)
echo "[mn] master 기동(Ray head + serve)..."
env $MOUNTVARS docker compose -f output/multi/docker-compose.yaml --env-file "$EFC" --env-file "$EF" --profile master up -d >/dev/null 2>&1
echo "[mn] slave 기동(Ray worker, SSH)..."
# ★ 2026-09-30(plan_26093000 · 멀티 라이브 발견): 이 줄은 서브 `compose up` 의 출력과 종료코드를 `/dev/null` 로 버렸다 —
#   서브 컨테이너가 **생성조차 되지 않아도**(No such container) master 가 5분을 "Waiting for slave" 로 채운 뒤 `did not join`
#   으로 끝났고, 사인은 어디에도 남지 않았다(침묵 누락). 출력은 teardown 을 견디는 자리에 남기고, rc≠0 이거나 rc 0 인데 서브에
#   컨테이너가 없으면 **대기 없이** 그 출력과 함께 멈춘다(아래 폴링을 건너뛰어 기존 미준비 경로로 간다).
SLAVE_UP_LOG="$REPO/output/multi/benchlog/slave_up_${CONFIG}.log"; mkdir -p "$(dirname "$SLAVE_UP_LOG")"
$SSH "$SUB_HOST" "bash -lc '$SUB_CD $SLAVE_IMGVARS $MOUNTVARS $PLEVARS docker compose -f output/multi/docker-compose.yaml --env-file $EFC --profile slave up -d'" > "$SLAVE_UP_LOG" 2>&1
SLAVE_UP_RC=$?
SLAVE_UP_FAILED=0
if [ "$SLAVE_UP_RC" != "0" ]; then
  SLAVE_UP_FAILED=1
  echo "[mn] FAILURE(slave-up): 서브 compose up rc=$SLAVE_UP_RC — 출력($SLAVE_UP_LOG) 꼬리:"
elif ! $SSH "$SUB_HOST" "docker ps -a --filter name='^${SLVC:-vllm-slave-serve-container}\$' -q" 2>/dev/null | grep -q .; then
  SLAVE_UP_FAILED=1
  echo "[mn] FAILURE(slave-up): 서브 compose up 은 rc 0 인데 서브에 컨테이너 '${SLVC:-vllm-slave-serve-container}' 가 없다 — 출력($SLAVE_UP_LOG) 꼬리:"
fi
[ "$SLAVE_UP_FAILED" = "1" ] && { tail -30 "$SLAVE_UP_LOG" | sed 's/^/[mn]   /'; echo "[mn]   → 5분 대기 없이 멈춘다(서브 배달·compose 입력을 먼저 확인하라 — sync_to_sub.sh dry-run)"; }

# ── 준비 폴링: 엔드포인트 health(거짓양성 회피) ──
# READY_MAX(폴링 횟수×5s) = health 창. 환경변수로 조정한다. **기본 180(15분)은 작은 모델 기준이며,
#   아래 두 실측 사례는 둘 다 그 창을 넘겼다.** 두 사례가 서로 다른 값을 권하는 것은 모순이 아니라
#   모델·트랙마다 로드 시간이 다르기 때문이다 — 그래서 값을 외우지 말고 **규칙**을 쓴다:
#
#     READY_MAX ≈ (그 구성의 실측 로드 초) ÷ 5 × 1.5   ← 여유 50%. 실측이 없으면 아래 사례에서 유추한다.
#       · Qwen3-Next-80B bf16 151GB CIFS 로드 ~11분 + KV/compile   → **360**(30분)  · testlog_26062501
#       · R2 ds4f0731-x2-sm12x (최초 JIT 변종) 실측 1,055s          → **600**(50분)  · 아래 ★
#
#   ⚠ 기본값 자체를 올리지 않는 이유: 창을 늘리면 **진짜 hang 일 때 그만큼 늦게 실패한다.** 15분 기본은
#     "빨리 실패한다"는 값어치가 있다. 근본 처방은 창 확대가 아니라 **진행 감지**(엔진 로그가 전진하면
#     연장, 정체하면 기존 창에서 끊기)이며, 지금은 느린 로드와 hang 이 **둘 다 종료코드 2 로 뭉개진다**
#     — `_ordered_between` 이 앵커 부재와 순서 위반을 뭉갰던 것과 같은 형태의 결함이다(미해소 · 후속).
# ★ 실측 사례 2 (2026-08-14 R2 · ds4f0731-x2-sm12x): **최초 JIT 컴파일이 있는 변종**은 기본 15분이 모자란다.
#   내역 = 가중치 로드 723.7s(model_runner "Model loading took 79.17 GiB and 723.696045 s")
#        + init engine(profile·KV·warmup) 226.7s(core.py "init engine ... took 226.74 s") + API 기동 ~25s
#        = 컨테이너 기동 14:06:07Z → "Application startup complete" 14:23:42Z = **1,055s(17분35초)**.
#   기본 900s 창이 2.6분 모자라 종료코드 2(로드는 정상 진행 중이었다 — hang 아님). ⇒ READY_MAX=600(50분) 권장.
#   SM12x TileLang/flashinfer JIT 캐시는 **컨테이너 쓰기층에만** 남는다(/root/.cache/vllm 는 마운트 아님)
#   → 컨테이너 재생성마다 226.7s 를 다시 낸다. 즉 이 확대는 1회성이 아니라 상시 필요하다.
#   READY_MAX 확대는 `READY_BUDGET_S=$READY_WINDOW_S` 지점에서 예산 TTL(=READY_MAX×5×3)도 함께 늘려
#   로드 도중 선언 만료를 구조적으로 배제한다(라인번호 대신 심볼로 가리킨다 — 번호는 편집마다 낡는다).
echo "[mn] 엔드포인트 :$PORT health 폴링(2노드 분산 로드; READY_MAX=${READY_MAX}회×5s ≈ $(( READY_WINDOW_S / 60 ))분)..."
# ── 실패 시 엔진 로그 보존 (2026-09-06 신설) ─────────────────────────────────────
#   왜: 위 실패 판정은 `out of memory|NCCL error|did not join|RuntimeError` 라는 **닫힌 목록**으로
#   grep 하고 `tail -3` 만 보여준다. 그런데 vLLM 이 마지막에 찍는 줄은
#   `RuntimeError: Engine core initialization failed. See root cause above.` 이고 —
#   **진짜 원인은 그 "above" 에 있으며 이 목록에 걸리지 않는다.**
#   캠페인 2 실측(2026-09-06): b5(moe=triton)·b6(mxfp4 스위치)가 이렇게 죽었는데 teardown 뒤
#   컨테이너 로그가 사라져 **사인을 영영 알 수 없게 됐다** — 두 셀을 다시 돌려야 했다.
#   "실패는 숨길 것이 아니라 지도가 실어야 할 정보다"(broad_search --serve-failed 주석)를
#   이 자리도 지켜야 한다. teardown 을 견디는 곳에 양 노드 로그를 통째로 남긴다.
_save_serve_logs() {   # $1=사유 태그
    local dir="$REPO/output/multi/benchlog/serve_fail_${CONFIG}"
    mkdir -p "$dir" 2>/dev/null || return 0
    docker logs "$MC" > "$dir/master_$1.log" 2>&1 || true
    # 슬레이브 컨테이너명은 콤보 env 의 SLAVE_CONTAINER_NAME(=$SLVC)이 정본이며, 부재 시 폴백은
    #   compose 기본값과 같아야 한다 — 이름이 갈리면 로그를 엉뚱한 곳에서 찾는다(129행 주석 참조).
    $SSH "$SUB_HOST" "docker logs '${SLVC:-vllm-slave-serve-container}' 2>&1" > "$dir/slave_$1.log" 2>&1 || true
    echo "[mn] 엔진 로그 보존 → $dir (master/slave · teardown 을 견딘다)"
    # 닫힌 목록 밖의 사인을 사람이 바로 보게 한다 — 목록을 늘리는 대신 **넓게 보여준다**.
    echo "[mn] ── master 로그 꼬리 40줄(사인 후보) ──"
    docker logs "$MC" 2>&1 | grep -vE "Capturing CUDA graphs|it/s\]$" | tail -40 | sed 's/^/[mn]   /'
}

READY=0; LAST_HTTP=""
_POLL_MAX="$READY_MAX"; [ "$SLAVE_UP_FAILED" = "1" ] && _POLL_MAX=0   # 서브가 서지 않았으면 기다리지 않는다
for i in $(seq 1 "$_POLL_MAX"); do
  LAST_HTTP="$(curl -s -m 5 -o /dev/null -w '%{http_code}' http://localhost:$PORT/health 2>/dev/null)"
  [ "$LAST_HTTP" = "200" ] && { echo "[mn] READY ~$((i*5))s"; READY=1; break; }
  docker ps --filter name="$MC" --filter status=running -q | grep -q . || { echo "[mn] master EXITED"; _save_serve_logs "exited"; docker logs "$MC" 2>&1 | tail -12; break; }
  docker logs "$MC" 2>&1 | grep -qiE "CUDA out of memory|NCCL error|did not join|RuntimeError" && { echo "[mn] FAILURE(serve)"; _save_serve_logs "failure"; docker logs "$MC" 2>&1 | grep -iE "out of memory|NCCL error|did not join|RuntimeError" | tail -3; break; }
  sleep 5
done

# 미준비 진단 보강: 워치독 트립 = 마진 결함 증거(plan_26071019 §5 판정축)를 표면화.
if [ "$READY" != "1" ] && [ "$WATCHDOG" = "1" ]; then
  grep -h "TRIP" /tmp/mn_watchdog_master.log 2>/dev/null | tail -3 | sed 's/^/[mn] watchdog(master): /'
  $SSH "$SUB_HOST" "grep -h TRIP /tmp/mn_watchdog_slave.log 2>/dev/null | tail -3" 2>/dev/null | sed 's/^/[mn] watchdog(slave): /'
fi

# ── multi-smoke (master 엔드포인트, reasoning 모델 대비 max_tokens 충분히) ──
#
# ★ 2026-08-22(W-10) 교정 — **빈 content 를 통과시키던 합격 기준**.
#   실측(testlog_26082215 §4.7): `SMOKE PASS — content='' fr=length`. 판정식은 `content or reasoning`
#   이었으므로 통과를 떠받친 것은 reasoning 이었는데, **로그는 그 사실을 말하지 않았다** — 읽는 사람에겐
#   "빈 응답으로 합격" 으로 보인다. 게다가 `finish_reason=length` 는 사고 블록이 max_tokens 안에서
#   끝나지 않았다는 뜻이라, 그 응답만으로는 "쓸 수 있는 생성" 을 확증하지 못한다.
#   같은 런에서 파서를 우회한 `/v1/completions` 는 71자 평문을 돌려줬다 — 엔진은 확실히 생성 중이었다.
#
#   그래서 판정을 **증거원 표기(provenance)** 와 함께 세 갈래로 가른다:
#     ① content 비어있지 않음                → PASS (evidence=chat.content)
#     ② content 비었고 reasoning 만 있음      → **아직 판정하지 않는다.** 파서를 우회한
#                                              `/v1/completions` 로 생성 실재를 확증한 뒤 PASS
#                                              (evidence=v1.completions) · 확증 실패면 FAIL
#     ③ 둘 다 비었음                          → FAIL (종전과 동일)
#   ②의 탈출구는 `SMOKE_ALLOW_REASONING_ONLY=1` 이며, 쓰면 **크게 로그를 남긴다**(침묵 완화 ✗).
RESULT=2
SMOKE_VERDICT="not-attempted"; SMOKE_EVIDENCE=""; CH_CLEN=""; CH_RLEN=""; CH_FR=""; CO_LEN=""
if [ "$READY" = "1" ]; then
  printf '{"model":"%s","messages":[{"role":"user","content":"2+2= ? \xec\x88\xab\xec\x9e\x90\xeb\xa7\x8c \xeb\x8b\xb5\xed\x95\x98\xec\x84\xb8\xec\x9a\x94."}],"max_tokens":256}' "$MODEL" > /tmp/mn_req.json
  curl -s -m 120 "http://localhost:$PORT/v1/chat/completions" -H "Content-Type: application/json" -d @/tmp/mn_req.json -o /tmp/mn_resp.json
  # 한 파서가 세 값을 함께 낸다(두 벌로 나누면 판정과 표시가 갈린다 — 그것이 이번 결함의 형태다).
  read -r CH_VERDICT CH_CLEN CH_RLEN CH_FR <<<"$(python3 -c "
import json
try:
    d = json.load(open('/tmp/mn_resp.json'))
    c = d['choices'][0]
    m = c.get('message') or {}
    content = (m.get('content') or '').strip()
    reasoning = (m.get('reasoning') or m.get('reasoning_content') or '').strip()
    v = 'content' if content else ('reasoning-only' if reasoning else 'empty')
    print(v, len(content), len(reasoning), c.get('finish_reason'))
except Exception:
    print('unparsable 0 0 none')
" 2>/dev/null)"
  CH_VERDICT="${CH_VERDICT:-unparsable}"
  echo "[mn] smoke(chat): verdict=$CH_VERDICT content_len=${CH_CLEN:-?} reasoning_len=${CH_RLEN:-?} finish_reason=${CH_FR:-?}"

  case "$CH_VERDICT" in
    content)
      echo "[mn] SMOKE PASS — evidence=chat.content (content_len=$CH_CLEN fr=$CH_FR)"; RESULT=0
      SMOKE_VERDICT="pass"; SMOKE_EVIDENCE="chat.content" ;;
    reasoning-only)
      # 파서 우회 프로브: reasoning 파서가 content 를 비워도 /v1/completions 는 원시 텍스트를 돌려준다.
      echo "[mn] ⚠ chat content 가 비었다(reasoning_len=$CH_RLEN · fr=$CH_FR) — 생성 실재를 /v1/completions 로 확증한다."
      printf '{"model":"%s","prompt":"2+2=","max_tokens":32}' "$MODEL" > /tmp/mn_req_compl.json
      curl -s -m 120 "http://localhost:$PORT/v1/completions" -H "Content-Type: application/json" -d @/tmp/mn_req_compl.json -o /tmp/mn_resp_compl.json
      CO_LEN=$(python3 -c "
import json
try:
    d = json.load(open('/tmp/mn_resp_compl.json'))
    print(len((d['choices'][0].get('text') or '').strip()))
except Exception:
    print(0)
" 2>/dev/null)
      if [ "${CO_LEN:-0}" -gt 0 ]; then
        echo "[mn] SMOKE PASS — evidence=v1.completions (text_len=$CO_LEN · chat.reasoning_len=$CH_RLEN)"
        echo "[mn]   ⚠ 그러나 chat content 는 비어 있다(fr=$CH_FR). reasoning 파서가 활성인데 사고 블록이"
        echo "[mn]     max_tokens 안에서 끝나지 않은 형태다 — 서빙 레시피(max_tokens·reasoning-parser)를 점검하라."
        RESULT=0; SMOKE_VERDICT="pass"; SMOKE_EVIDENCE="v1.completions"
      elif [ "${SMOKE_ALLOW_REASONING_ONLY:-0}" = "1" ]; then
        echo "[mn] SMOKE PASS(완화) — evidence=chat.reasoning only · SMOKE_ALLOW_REASONING_ONLY=1 탈출구 사용"
        echo "[mn]   ⚠ /v1/completions 확증에 실패했다(text_len=${CO_LEN:-0}). 이 PASS 는 생성 실재를 증명하지 않는다."
        # 완화 PASS 는 생성 실재를 증명하지 않는다 — serve_proof 에는 pass 가 아니라 그 사실대로 적는다
        #   (hint 발행 자격 X8 은 이 값을 추론 관측으로 인정하지 않는다).
        RESULT=0; SMOKE_VERDICT="relaxed-reasoning-only"; SMOKE_EVIDENCE="chat.reasoning(relaxed)"
      else
        SMOKE_VERDICT="fail"
        echo "[mn] SMOKE FAIL — 생성 실재 미확증: chat content 비었고 /v1/completions 도 비었다(text_len=${CO_LEN:-0})."
        echo "[mn]   reasoning 만으로 통과시키려면 SMOKE_ALLOW_REASONING_ONLY=1 (완화는 로그에 남는다). raw:"
        head -c 300 /tmp/mn_resp_compl.json; echo
      fi ;;
    *)
      SMOKE_VERDICT="fail"
      echo "[mn] SMOKE FAIL — content·reasoning 모두 비었다(verdict=$CH_VERDICT). raw:"
      head -c 300 /tmp/mn_resp.json; echo ;;
  esac
fi

# ── serve 시점 관측 영속 (2026-09-21 · plan_26092119 §4.6·§4.8 · X8/X12 · 코드맵 K1/K7) ──────────────
#   ① attestation v2 — 두 노드의 **실행 중** 컨테이너에서 원장·vllm sha·torch·driver 를 캡처(기록 전용 · 차단 ✗).
#      컨테이너가 떠 있을 때(READY)만 캡처한다 — 죽은 컨테이너를 캡처하면 "관측 불가" 가 "불일치" 처럼 남는다.
#   ② serve_proof_<CONFIG>.json — health + 추론 1회의 **관측** 결과. hint 발행 자격(X8)의 1순위 입력이다.
#      실패도 적는다(최신 관측이 옛 PASS 를 덮어야 한다 — 옛 PASS 가 남으면 죽은 셀이 자격을 얻는다).
#      시각은 넣지 않는다(데이터 평면 벽시계 ✗ — 캠페인 진행표의 ended_utc 가 시각을 든다).
#   ③ 캠페인 serve 진행표 — proof 는 ② 파일을 출처로 가리킨다(선언이 관측을 대체하지 않게).
if [ "$READY" = "1" ]; then
  _CAP="$(mktemp -d -t mn_attest_cap.XXXXXX 2>/dev/null || echo "/tmp/mn_attest_cap.$$")"; mkdir -p "$_CAP"
  _attest_capture_node main "$MC" "$_CAP"
  _attest_capture_node sub "${SLVC:-vllm-slave-serve-container}" "$_CAP"
  _attest_write serve "$_CAP"
  rm -rf "$_CAP"
else
  echo "[mn] attestation v2(serve) 생략 — READY 에 도달하지 못해 실행 중 컨테이너가 없다(관측 불가를 불일치로 적지 않는다)."
fi
rm -f "$ATTEST_ROWS"
SERVE_PROOF="$REPO/output/multi/benchlog/serve_proof_${CONFIG}.json"
mkdir -p "$(dirname "$SERVE_PROOF")" 2>/dev/null
_ATT_REF=""; [ "$READY" = "1" ] && [ -f "$ATTEST_JSON" ] && _ATT_REF="${ATTEST_JSON#$REPO/}"
if python3 - "$SERVE_PROOF" "$CONFIG" "$READY" "$LAST_HTTP" "$SMOKE_VERDICT" "$SMOKE_EVIDENCE" "${CH_CLEN:-}" \
     "${CO_LEN:-}" "${CH_RLEN:-}" "${CH_FR:-}" "$IMG" "$MC" "$_ATT_REF" <<'PY'
import json, sys
(out, config, ready, http, verdict, evidence, clen, colen, rlen, fr, image, mc, att) = sys.argv[1:14]
def num(v):
    return int(v) if v.isdigit() else None
# content_len = **추론 증거가 된 생성 텍스트의 길이**(SPEC §6.1 · hint 발행 자격 X8 이 `content_len ≥ 1` 을 요구한다).
#   evidence=chat.content → chat content 길이 · evidence=v1.completions → 파서 우회 completions 텍스트 길이 ·
#   완화 PASS(reasoning 만)·실패 → 0(생성 텍스트를 관측하지 못했다) · 추론 미시도 → null.
#   ⚠ v1.completions PASS 에 chat content 길이(0)를 적으면 진짜 PASS 가 자격을 잃는다 — chat 쪽 길이는 따로 둔다.
if verdict == "not-attempted":
    content_len = None
elif verdict == "pass" and evidence == "chat.content":
    content_len = num(clen)
elif verdict == "pass" and evidence == "v1.completions":
    content_len = num(colen)
else:
    content_len = 0
doc = {"schema_version": 1, "kind": "multinode_serve_proof", "config": config, "topology": "multi",
       "ready": ready == "1", "health_http": num(http),
       "inference_verdict": verdict, "evidence": evidence or None,
       "content_len": content_len, "chat_content_len": num(clen),
       "completion_text_len": num(colen), "reasoning_len": num(rlen),
       "finish_reason": fr or None, "image_tag": image or None, "master_container": mc or None,
       "attestation_ref": att or None, "provenance": "measured(multinode_serve_smoke)"}
with open(out, "w", encoding="utf-8") as fh:
    json.dump(doc, fh, ensure_ascii=False, indent=2, sort_keys=True)
    fh.write("\n")
PY
then echo "[mn] serve proof → ${SERVE_PROOF#$REPO/} (verdict=$SMOKE_VERDICT evidence=${SMOKE_EVIDENCE:-없음})"
else echo "[mn] ⚠ serve proof 기록 실패 — hint 발행 자격을 이 셀에서 관측할 수 없다"; fi
if [ "$RESULT" = "0" ] && [ "$SMOKE_VERDICT" = "pass" ]; then
  _campaign_phase serve done 1 "health 200 + 추론 1회" \
    "multinode_serve_smoke.sh: /health=200 · 추론 evidence=$SMOKE_EVIDENCE → ${SERVE_PROOF#$REPO/}" "$_SERVE_T0"
else
  # 완화 PASS(reasoning-only)는 스모크 종료코드는 0 이지만 proof 술어(추론 1회 관측)를 만족하지 않는다 —
  #   진행표는 그 사실대로 proof.ok 없이 적는다(술어를 낮추지 않는다).
  _campaign_phase serve "$([ "$RESULT" = "0" ] && echo done || echo failed)" 0 "health 200 + 추론 1회" \
    "multinode_serve_smoke.sh: ready=$READY health=${LAST_HTTP:-none} verdict=$SMOKE_VERDICT → ${SERVE_PROOF#$REPO/}" "$_SERVE_T0"
fi

# ── 정리 ──
if [ "$KEEP" != "1" ]; then
  teardown_serve smoke
elif [ -n "$WD_MAIN_PID" ] || [ "$BUDGET_DECLARED" = "1" ]; then
  [ -n "$WD_MAIN_PID" ] && echo "[mn] --keep-up: 워치독 유지(master pid=$WD_MAIN_PID · slave pid=${WD_SUB_PID:-없음}) — 정지는 kill <pid> 로만"
  # --keep-up 은 서빙이 상주하므로 선언도 **유지**한다(회수하면 상주 서빙이 무보호가 된다).
  #   ★ 2026-08-22(W-7): 여기엔 **안내문만** 있었다 — "연장은 이 명령으로" 가 사람을 가리켰고,
  #   사람은 치지 않았다. 실측 결과 상주 18시간 중 **12시간이 만료 상태**였다(testlog_26082215 §4.0).
  #   처방이 가리키는 주체가 아키텍처에 없으면 그것은 안전장치가 아니다 — 주체를 만들어 건다.
  if [ "$BUDGET_DECLARED" = "1" ]; then
    echo "[mn] --keep-up: 예산 선언 유지 · TTL ${BUDGET_TTL_S}s → 갱신 루프를 무장한다(만료 구간 제거)."
    if [ -f "$MAIN_RENEW_SH" ]; then
      reap_renew_loops master
      setsid nohup bash "$MAIN_RENEW_SH" --node-dir "$(budget_node_dir_main)" \
             --container "$MC" --ttl-s "$BUDGET_TTL_S" \
             </dev/null >/tmp/mn_budget_renew_master.log 2>&1 & RENEW_MAIN_PID=$!
      echo "[mn]   갱신 루프(master) pid=$RENEW_MAIN_PID container=$MC (/tmp/mn_budget_renew_master.log)"
    else
      echo "[mn]   ⚠ $MAIN_RENEW_SH 부재 — master 갱신 루프 생략. 만료까지 ${BUDGET_TTL_S}s 뒤 무보호가 된다."
      echo "[mn]     수동 연장: python3 $MAIN_SESSION_PY --node-dir $(budget_node_dir_main) renew-budget --ttl-s <n> --now <ISO>"
    fi
    if [ -n "$SUB_NODE_ID" ] && $SSH "$SUB_HOST" "bash -lc '[ -f $SUB_RENEW_SH ]'" 2>/dev/null; then
      reap_renew_loops slave
      # 원격 detach 는 워치독과 같은 처방(setsid + exit 0 + timeout 백스톱) — 같은 hang 함정이다.
      RENEW_SUB_PID=$(timeout 20 $SSH -n "$SUB_HOST" "bash -lc '$SUB_CD setsid nohup bash $SUB_RENEW_SH --node-dir $SUB_WORK_DIR/docs/logs/$SUB_NODE_ID --container ${SLVC:-vllm-slave-serve-container} --ttl-s $BUDGET_TTL_S </dev/null >/tmp/mn_budget_renew_slave.log 2>&1 & echo \$!; exit 0'" 2>/dev/null || true)
      echo "[mn]   갱신 루프(slave) pid=${RENEW_SUB_PID:-?} (원격 /tmp/mn_budget_renew_slave.log)"
    else
      echo "[mn]   ⚠ 서브에 budget_renew_loop.sh 부재 — slave 갱신 루프 생략(render_sub_env/sync_to_sub 재배달 필요)."
    fi
    echo "[mn]   회수: multinode_serve_smoke.sh $CONFIG --down (루프·워치독·선언을 함께 회수한다)"
  fi
fi
echo "[mn] 종료코드 $RESULT"
exit $RESULT
