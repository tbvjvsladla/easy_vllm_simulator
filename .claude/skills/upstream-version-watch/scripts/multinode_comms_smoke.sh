#!/bin/bash
# 모델리스 2노드 통신 스모크 (R1·D3, plan_2026062320_1 S3) — 서빙 대상 모델 없이 main↔sub NCCL/RDMA 검증.
#
# Plan 2(plan_2026062316_1) S4 의 "올바른 형태": 모델·트리플릿·NAS 의존 0 으로
#   debug 프로파일을 `compose run --rm`(모델리스)로 양 노드에서 직접 실행해 torch.distributed(NCCL) all-reduce 로
#   ① 클러스터(2-rank) 형성 ② 노드간 집합통신 정확성 ③ RDMA 경로(NET/IB·GDR) 사용 ④ 대역폭(증거) 검증.
# NCCL/RDMA env 는 compose env_file(envs/.env.interconnect — manifest-driven, Plan 2)로 주입된다 →
#   이 스모크가 그 env_file 전달까지 함께 검증한다(Plan 2 done-gate).
#
# 왜 ray 가 아니라 torch.distributed 인가: ray + serve_runner.sh 는 모델 sh/yaml 을 요구한다(서빙 계층).
#   순수 통신 검증은 모델 없이 NCCL 을 직접 때리는 torch.distributed all-reduce 가 가장 직접적·모델리스(R1).
#
# 거짓양성 회피: master 로그 grep("startup complete") 안 씀. all-reduce **수치 정확성 + NET/IB 로그 실측**이 게이트.
#   socket fallback(NET/Socket only)으로 통과하면 FAIL(RDMA 미사용). NET/IB 존재를 명시 요구.
#
# 사용: bash multinode_comms_smoke.sh [--build] [--keep-up]
#   --build   : 양 노드 debug 이미지 빌드(병렬)
#   --keep-up : 스모크 후 컨테이너 유지(기본 down)
# 종료코드: 0=PASS / 2=통신 실패·미준비 / 3=설정/해소 실패.
# 환경 override: MASTER_IP · SUB_HOST(<user>@<host>) · SUB_WORK_DIR · MASTER_PORT · EXEC_TMO(워치독 초, 기본 240) · CTR_M·CTR_S(run 컨테이너명) (미지정 시 manifest 해소).
set -uo pipefail

BUILD=0; KEEP=0
for a in "$@"; do [ "$a" = "--build" ] && BUILD=1; [ "$a" = "--keep-up" ] && KEEP=1; done

SDIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$SDIR/../../../.." && pwd)"
cd "$REPO"
MANIFEST="output/multi/manifest.yaml"
COMPOSE="output/multi/docker-compose.yaml"
ENVI="output/multi/envs/.env.interconnect"
CTR_M="${CTR_M:-vllm_comms_m}"; CTR_S="${CTR_S:-vllm_comms_s}"   # compose run --name (1회성 --rm; compose container_name 비의존 — up -d 대화형 즉시종료 회피)
MASTER_PORT="${MASTER_PORT:-29500}"     # torch.distributed rendezvous 포트
EXEC_TMO="${EXEC_TMO:-240}"             # exec 워치독 초 — 한쪽-join/rendezvous 멈춤 시 유한시간 FAIL(NCCL 120s + 외곽)
[ -f "$MANIFEST" ] || { echo "[cs] FAIL: $MANIFEST 없음 — terraforming 으로 채우세요"; exit 3; }
[ -f "$COMPOSE" ]  || { echo "[cs] FAIL: $COMPOSE 없음 — render 산출물 통로 확인"; exit 3; }
[ -f "$ENVI" ]     || { echo "[cs] FAIL: $ENVI 없음 — render_dockerfile.py --nccl-envfile 로 먼저 생성(Plan 2)"; exit 3; }
# 통로 self-containment 전제(plan_2026062321_1 I1/I2): 러너 스크립트가 통로에 materialize 됐는지 fail-loud.
for s in serve_runner.sh debug-init.sh; do
  [ -f "output/multi/configs/$s" ] || { echo "[cs] FAIL: 통로 미완결 — output/multi/configs/$s 부재. 먼저 'render_dockerfile.py --materialize-configs --topology multi' 실행(후 sync_to_sub.sh --apply)"; exit 3; }
done

# ── manifest 해소(하드코딩 금지 — 포인터 원칙) ──
_mf_node() {  # args: role field  → nodes[role].field
  awk -v role="$1" -v field="$2" '
    $0 ~ "^[[:space:]]*-[[:space:]]*role:[[:space:]]*" role "([[:space:]]|$|#)" { in_n=1; next }
    /^[[:space:]]*-[[:space:]]*role:/ { in_n=0 }
    in_n && $0 ~ "^[[:space:]]*" field ":" { sub("^[[:space:]]*" field ":[[:space:]]*",""); sub(/[[:space:]]*#.*/,""); gsub(/[ "\r]/,""); print; exit }
  ' "$MANIFEST"
}

MASTER_IP="${MASTER_IP:-$(_mf_node main host)}"
SUB_H="$(_mf_node sub host)"; SUB_U="$(_mf_node sub ssh_user)"
[ -n "$MASTER_IP" ]                       || { echo "[cs] FAIL: nodes[main].host 해소 실패"; exit 3; }
[ -n "$SUB_H" ]                           || { echo "[cs] FAIL: nodes[sub].host 해소 실패"; exit 3; }
[ -n "${SUB_HOST:-}" ] || [ -n "$SUB_U" ] || { echo "[cs] FAIL: nodes[sub].ssh_user 해소 실패(SUB_HOST 도 미지정)"; exit 3; }
SUB_HOST="${SUB_HOST:-${SUB_U}@${SUB_H}}"
SUB_WORK_DIR="${SUB_WORK_DIR:-$(_mf_node sub work_dir)}"; SUB_WORK_DIR="${SUB_WORK_DIR:-$REPO}"
case "$SUB_WORK_DIR" in *[[:space:]]*) echo "[cs] FAIL: SUB_WORK_DIR 공백 — 원격 cd 임베드 불가: '$SUB_WORK_DIR'"; exit 3;; esac

SSH="ssh -o BatchMode=yes -o ConnectTimeout=8"
SUB_CD="cd $SUB_WORK_DIR &&"   # bash -lc '...' 단일인용 컨텍스트 임베드 — 경로 무공백 전제(manifest 통제값)
DC="docker compose -f $COMPOSE --profile debug"
echo "[cs] master_ip=$MASTER_IP sub=$SUB_HOST sub_work_dir=$SUB_WORK_DIR port=$MASTER_PORT"

# ── SSH 도달성 ──
$SSH "$SUB_HOST" 'echo ok' >/dev/null 2>&1 || { echo "[cs] FAIL: $SUB_HOST SSH 불가(키 인증·네트워크 확인)"; exit 3; }

_down() {
  [ "$KEEP" = 1 ] && return 0
  echo "[cs] 정리(1회성 run 컨테이너 제거)..."
  docker rm -f "$CTR_M" >/dev/null 2>&1
  $SSH "$SUB_HOST" "docker rm -f $CTR_S" >/dev/null 2>&1
}

# ── 빌드(옵션, 양 노드 병렬) ──
if [ "$BUILD" = 1 ]; then
  echo "[cs] 양 노드 debug 빌드(병렬)..."
  $DC build >/tmp/cs_build_m.log 2>&1 & MP=$!
  $SSH "$SUB_HOST" "bash -lc '$SUB_CD docker compose -f $COMPOSE --profile debug build'" >/tmp/cs_build_s.log 2>&1 & SP=$!
  wait $MP; MBR=$?; wait $SP; SBR=$?
  [ $MBR -eq 0 ] && [ $SBR -eq 0 ] || { echo "[cs] FAIL 빌드(master=$MBR slave=$SBR):"; tail -6 /tmp/cs_build_m.log /tmp/cs_build_s.log; exit 2; }
  echo "[cs] 빌드 OK(양 노드)"
fi

# ── 사전 점검(거짓음성 방지): GPU 점유 경합 serve 컨테이너 잔류 경고(자동 down 안 함 — 사용자 의도 보호) ──
_stale=$(docker ps --format '{{.Names}}' 2>/dev/null | grep -iE 'serve' || true)
[ -n "$_stale" ] && echo "[cs] ⚠ 경고: GPU 점유 가능 serve 컨테이너 잔류 — 통합메모리 OOM 거짓음성 위험: $_stale (필요 시 먼저 down)"
command -v nvidia-smi >/dev/null 2>&1 && echo "[cs] GPU mem(used/total): $(nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader 2>/dev/null | head -1)"

# (debug 를 up -d 로 띄우지 않는다 — `bash --rcfile` 대화형 셸은 detached 에서 stdin EOF 로 즉시 종료.
#  대신 아래 `compose run --rm` 으로 all-reduce 를 컨테이너 메인 프로세스로 직접 실행. 사람용 대화형 debug UX 는 보존.)

# ── all-reduce 페이로드(모델리스). stdin 으로 python3 - 에 주입(셸 인용 회피) ──
PYSRC=$(cat <<'PY'
import os, time, torch
from datetime import timedelta
import torch.distributed as dist
rank = int(os.environ["RANK"]); ws = int(os.environ["WORLD_SIZE"])
# timeout: 한쪽-join/rendezvous 멈춤 시 NCCL 워치독이 유한시간(120s) 내 예외 → 빠른 FAIL (TORCH_NCCL_BLOCKING_WAIT=1 동반)
dist.init_process_group(backend="nccl", init_method="env://", world_size=ws, rank=rank, timeout=timedelta(seconds=120))
torch.cuda.set_device(0)
dev = torch.device("cuda:0")
# 정확성: rank 는 (rank+1) 기여 → SUM = ws*(ws+1)/2 (예: ws=2 → 3)
small = torch.full((256, 256), float(rank + 1), device=dev)
dist.all_reduce(small, op=dist.ReduceOp.SUM)
exp = ws * (ws + 1) / 2
got = small.flatten()[0].item()
ok = abs(got - exp) < 1e-3
# 대역폭(증거 — 게이트 아님): 256MB all-reduce 타이밍 → busbw
N = 64 * 1024 * 1024
big = torch.ones(N, device=dev)
torch.cuda.synchronize(); dist.barrier()
it = 10; t0 = time.time()
for _ in range(it):
    dist.all_reduce(big)
torch.cuda.synchronize(); dt = (time.time() - t0) / it
nbytes = big.element_size() * big.nelement()
busbw = 2.0 * (ws - 1) / ws * nbytes / dt / 1e9  # GB/s
if rank == 0:
    print(f"COMMS_RESULT correct={int(ok)} expected={exp} got={got} "
          f"busbw_gbps={busbw*8:.1f} size_mb={nbytes/1e6:.0f} dt_ms={dt*1000:.1f}")
dist.barrier(); dist.destroy_process_group()
print(f"COMMS_RANK{rank}_DONE ok={int(ok)}")
PY
)

echo "[cs] all-reduce(NCCL) 실행 — compose run(모델리스): master rank0 + sub rank1(동시 join, 워치독 ${EXEC_TMO}s)..."
M_OUT=/tmp/cs_master.out; S_OUT=/tmp/cs_slave.out
# compose run --rm -T: debug 서비스의 volumes/env_file(.env.interconnect NCCL)/network_mode=host 를 그대로 쓰되 command 만 python3 - 로 override.
# stdin 으로 PYSRC 주입(-T 는 TTY 만 끄고 stdin 은 연결). NGC entrypoint 배너 후 exec python3 -.
printf '%s' "$PYSRC" | timeout "$EXEC_TMO" $DC run --rm -T --name "$CTR_M" -e RANK=0 -e WORLD_SIZE=2 -e TORCH_NCCL_BLOCKING_WAIT=1 -e MASTER_ADDR="$MASTER_IP" -e MASTER_PORT="$MASTER_PORT" vllm python3 - >"$M_OUT" 2>&1 & MPID=$!
printf '%s' "$PYSRC" | timeout "$EXEC_TMO" $SSH "$SUB_HOST" "bash -lc '$SUB_CD docker compose -f $COMPOSE --profile debug run --rm -T --name $CTR_S -e RANK=1 -e WORLD_SIZE=2 -e TORCH_NCCL_BLOCKING_WAIT=1 -e MASTER_ADDR=$MASTER_IP -e MASTER_PORT=$MASTER_PORT vllm python3 -'" >"$S_OUT" 2>&1 & SPID=$!
wait $MPID; MR=$?; wait $SPID; SR=$?
[ "$MR" = 124 ] && echo "[cs] ⚠ master run 워치독 타임아웃(${EXEC_TMO}s) — 한쪽-join/rendezvous 멈춤 의심"
[ "$SR" = 124 ] && echo "[cs] ⚠ slave run 워치독 타임아웃(${EXEC_TMO}s) — 한쪽-join/rendezvous 멈춤 의심"

# ── 판정(거짓양성 회피): 정확성 + NET/IB 경로 실측 ──
RES=$(grep -h 'COMMS_RESULT' "$M_OUT" | head -1)
CORRECT=$(printf '%s' "$RES" | grep -oP 'correct=\K[01]' || true)
BUSBW=$(printf '%s' "$RES" | grep -oP 'busbw_gbps=\K[0-9.]+' || true)
IB=$(grep -ciE 'NET/IB|GPU Direct RDMA|GDRDMA|via NET/IB|\[IB' "$M_OUT" 2>/dev/null || true)
SOCK=$(grep -ciE 'NET/Socket' "$M_OUT" 2>/dev/null || true)

RESULT=2
if [ "$MR" = 0 ] && [ "$SR" = 0 ] && [ "${CORRECT:-0}" = "1" ]; then
  if [ "${IB:-0}" -ge 1 ]; then
    [ "${SOCK:-0}" -ge 1 ] && echo "[cs] ⚠ WARN: NET/IB 와 NET/Socket 혼재(sock_hits=$SOCK) — 부분 socket 폴백 의심(RDMA 일부만). NCCL 로그 확인 권장."
    echo "[cs] PASS — all-reduce 정확(SUM 일치) + RDMA(NET/IB) 경로 사용. busbw=${BUSBW:-?} Gb/s (ib_write_bw 208.2 참고)"
    RESULT=0
  else
    echo "[cs] FAIL — all-reduce 는 정확하나 NET/IB 로그 없음(socket fallback 의심, sock_hits=${SOCK:-0})."
    echo "          RDMA 미사용 = 거짓양성으로 차단. NCCL 로그 확인:"; grep -iE 'NET/|NCCL INFO' "$M_OUT" | tail -15
  fi
else
  echo "[cs] FAIL — all-reduce 실패(master_exit=$MR slave_exit=$SR correct=${CORRECT:-?}). raw tail:"
  echo "--- master ---"; tail -20 "$M_OUT"; echo "--- slave ---"; tail -20 "$S_OUT"
fi

_down
echo "[cs] 종료코드 $RESULT"
exit $RESULT
