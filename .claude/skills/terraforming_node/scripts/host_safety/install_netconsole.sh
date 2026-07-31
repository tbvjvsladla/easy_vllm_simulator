#!/bin/bash
# install_netconsole.sh — netconsole 교차 스트리밍 + rsyslog UDP 수신 설치자 (관측성 계층).
#
#   목적: 2026-07-30 두 번의 하드다운(1차 쌍노드·2차 main)에서 커널의 마지막 메시지가 어디에도
#   남지 않은 관측성 공백 해소. mem-watchdog·earlyoom 은 "점진 고갈" 감시라 "24 GiB 잔존 즉사"
#   클래스에 구조적으로 무효(testlog_26073021) — 이 계층은 방어가 아니라 **사고 후 블랙박스**다.
#
#   설치물: ① netconsole 모듈 영구 로드(modules-load.d + modprobe.d, peer 직결 바인딩)
#           ② rsyslog imudp 514 수신 + 송신호스트별 분리(/var/log/remote-kmsg-<peer>.log)
#           ③ kernel.printk=7(sysctl.d — debug 까지 유출)  ④ ufw 활성 시 peer udp/514 허용.
#   환경값 = manifest(output/multi/manifest.yaml nodes[])에서 읽는다(인터페이스는 `ip route get` 으로 결정론 해소).
#   실행 주체 = 사람(HITL sudo): sudo bash install_netconsole.sh --apply
#   기본 = dry-run(무엇을 설치할지 표시만). 멱등 — 재실행 안전. 양 노드 각각 실행.
# 종료코드: 0=성공(또는 dry-run) · 1=전제 실패 · 2=설치/검증 실패.
set -uo pipefail

APPLY=0; MANIFEST=""
for a in "$@"; do
  case "$a" in
    --apply) APPLY=1 ;;
    --manifest=*) MANIFEST="${a#*=}" ;;
    *) echo "알 수 없는 인자: $a (사용: $0 [--apply] [--manifest=PATH])" >&2; exit 1 ;;
  esac
done

# ★ 고정 상대깊이 금지 — 이 파일은 메인에서 .claude/skills/terraforming_node/scripts/host_safety/
#   에 있고 서브에는 .claude/runtime/host_safety/ 로 배달된다. 깊이가 달라 ../../../../.. 는
#   서브에서 /home/cona 를 가리키고 manifest 를 못 찾는다(2026-07-31 서브 배포에서 실측).
_find_repo(){ local d="$1"; while [ "$d" != "/" ] && [ -n "$d" ]; do
    [ -d "$d/.claude" ] && [ -d "$d/output" ] && { printf '%s' "$d"; return 0; }; d="$(dirname "$d")"; done; return 1; }
REPO="$(_find_repo "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)" \
        || (cd "$(dirname "${BASH_SOURCE[0]}")/../../../../.." && pwd))"
MANIFEST="${MANIFEST:-$REPO/output/multi/manifest.yaml}"
[ -f "$MANIFEST" ] || { echo "FAIL: manifest 부재: $MANIFEST" >&2; exit 1; }

# ── manifest nodes[] 에서 main/sub 호스트 해소(결정론 — sync_to_sub.sh 의 awk 패턴과 동형) ──
_main_ip=$(awk '/^[[:space:]]*-[[:space:]]*role:[[:space:]]*main([[:space:]]|$|#)/{in_m=1;next}
               /^[[:space:]]*-[[:space:]]*role:/{in_m=0}
               in_m && /^[[:space:]]*host:/{sub(/^[[:space:]]*host:[[:space:]]*/,"");sub(/[[:space:]]*#.*/,"");gsub(/[ "\r]/,"");print;exit}' "$MANIFEST")
_sub_ip=$(awk  '/^[[:space:]]*-[[:space:]]*role:[[:space:]]*sub([[:space:]]|$|#)/{in_s=1;next}
               /^[[:space:]]*-[[:space:]]*role:/{in_s=0}
               in_s && /^[[:space:]]*host:/{sub(/^[[:space:]]*host:[[:space:]]*/,"");sub(/[[:space:]]*#.*/,"");gsub(/[ "\r]/,"");print;exit}' "$MANIFEST")
[ -n "$_main_ip" ] && [ -n "$_sub_ip" ] || { echo "FAIL: manifest nodes[] 에 main/sub host 해소 불가" >&2; exit 1; }

# ── 자기 역할 판정(로컬 주소와 대조) — 어느 노드에서 실행하든 결정론 ──
_local_ips="$(ip -4 -o addr show scope global | awk '{split($4,a,"/");print a[1]}')"
case "$_local_ips" in
  *"$_main_ip"*) SELF_IP="$_main_ip"; PEER_IP="$_sub_ip" ;;
  *"$_sub_ip"*)  SELF_IP="$_sub_ip";  PEER_IP="$_main_ip" ;;
  *) echo "FAIL: 이 머신의 주소($_local_ips)가 manifest main/sub 어느 쪽과도 불일치" >&2; exit 1 ;;
esac

# ── peer 로 가는 인터페이스 해소(환경 하드코딩 금지) ──
IFACE="$(ip route get "$PEER_IP" 2>/dev/null | awk '{for(i=1;i<=NF;i++) if($i=="dev"){print $(i+1);exit}}')"
[ -n "$IFACE" ] || { echo "FAIL: $PEER_IP 행 인터페이스 해소 불가" >&2; exit 1; }

echo "[netconsole] role: self=$SELF_IP peer=$PEER_IP iface=$IFACE (manifest=$MANIFEST)"

if [ "$APPLY" -ne 1 ]; then
  cat <<EOF
[dry-run] 아래를 설치한다(적용은 --apply + sudo):
  /etc/rsyslog.d/49-netconsole-remote.conf  (imudp 514 → /var/log/remote-kmsg-<peer>.log)
  /etc/modules-load.d/netconsole.conf       (부팅 자동 로드)
  /etc/modprobe.d/netconsole.conf           (options netconsole netconsole=@${SELF_IP}/${IFACE},514@${PEER_IP}/)
  /etc/sysctl.d/90-netconsole-printk.conf   (kernel.printk = 7 4 1 7)
  ufw 활성 시: allow ${PEER_IP} udp/514
  + rsyslog 재시작, netconsole 재로드, /proc/modules 검증
EOF
  exit 0
fi

[ "$(id -u)" = 0 ] || { echo "FAIL: root 필요(sudo)" >&2; exit 1; }

# 1) rsyslog: UDP 514 수신 + 송신 호스트별 파일 분리(로컬 로그와 격리)
cat > /etc/rsyslog.d/49-netconsole-remote.conf <<'EOF'
# netconsole 원격 커널 로그 수신 (install_netconsole.sh — 하드다운 관측성 계층)
module(load="imudp")
input(type="imudp" port="514")
template(name="RemoteKmsg" type="string" string="/var/log/remote-kmsg-%fromhost-ip%.log")
if ($fromhost-ip != "127.0.0.1") then {
    action(type="omfile" dynaFile="RemoteKmsg")
    stop
}
EOF

# 2) netconsole: 부팅 자동 로드 + peer 바인딩
#    문법 주의: netconsole=[src-port]@[src-ip]/[dev],[tgt-port]@[tgt-ip]/[tgt-mac]
#    → 타겟 포트는 IP *앞* (@IP/514 는 514 를 MAC 으로 오파싱해 로드 실패 — 2026-07-30 실전 교훈)
echo netconsole > /etc/modules-load.d/netconsole.conf
cat > /etc/modprobe.d/netconsole.conf <<EOF
options netconsole netconsole=@${SELF_IP}/${IFACE},514@${PEER_IP}/
EOF

# 3) printk 콘솔 레벨 상향(7 = debug 까지 netconsole 유출)
printf 'kernel.printk = 7 4 1 7\n' > /etc/sysctl.d/90-netconsole-printk.conf
sysctl --system >/dev/null 2>&1 || sysctl -p /etc/sysctl.d/90-netconsole-printk.conf

# 4) 방화벽이 활성이면 peer 의 UDP 514 만 허용
if command -v ufw >/dev/null 2>&1; then
  ufw_status="$(ufw status 2>/dev/null || true)"
  case "$ufw_status" in
    *"Status: active"*) ufw allow from "$PEER_IP" to any port 514 proto udp comment "netconsole peer" >/dev/null
                        echo "[netconsole] ufw allow $PEER_IP udp/514" ;;
  esac
fi

# 5) 즉시 적용(기존 모듈 있으면 재로드)
systemctl restart rsyslog
modprobe -r netconsole 2>/dev/null || true
modprobe netconsole
# SIGPIPE 안전 확인: pipefail 하 `lsmod | grep -q` 는 grep -q 조기종료로 lsmod 에 SIGPIPE(141) 를 내는
# 경쟁이 있다(모듈은 로드됐는데 FAIL 오판 — 2026-07-30 실전 교훈). 파이프 없이 /proc/modules 를 본다.
grep -q '^netconsole ' /proc/modules || { echo "FAIL: netconsole not loaded" >&2; exit 2; }

echo "[netconsole] OK: $SELF_IP -> $PEER_IP:514 (수신 로그: /var/log/remote-kmsg-${PEER_IP}.log)"
echo "[netconsole] 자체시험: echo \"netconsole-selftest\" | sudo tee /dev/kmsg → 상대 노드 파일에 도착해야 함"
