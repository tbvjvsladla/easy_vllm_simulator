#!/usr/bin/env python3
"""scan_node.py — terraforming_node진입 루틴의 **결정론 스캔 코어** (판단 X, 사실만).

근거: docs/plan/plan_26062311(진입 루틴 §2.3 스캔 인벤토리 · §2.4 성능게이트 · §2.5 α/γ 종료),
      plan_26062312(output/ 통로), CLAUDE.md(결정론 스크립트 원칙 · 산출물 통로 불변식).

위치: 사용자 승인(HITL) 직후 호출되는 **deterministic** 단계. 5-전제조건 인터뷰·승인 게이트는
      terraforming_node스킬(판단계층)이 담당하고, 이 스크립트는 그 뒤 "스캔→파싱→게이트 판정"만 한다.

핵심 설계:
  - **ibstat 비의존**: 일부 환경(GB10)엔 ibstat 미설치 → `/sys/class/infiniband`(HCA) + `show_gids`(RoCE v2 GID/IP/iface)로 탐지.
  - **환경정체성 필드만**: type/hca_devices/gid_index/socket_iface/bandwidth_gbps. 튜닝상수(NCCL GDR/QPS)는 manifest 아님(렌더 프리셋).
  - **α/γ 토폴로지 키잉(fail-closed)**: --topology single → α 정상 skip / multi → RoCE+peer 필수, 미충족 시 γ blocked(비0 종료).
  - **교차검증**: 스캔값 ↔ 현 docker-compose NCCL env(NCCL_IB_HCA/GID_INDEX/SOCKET_IFNAME) 일치 확인.
  - bandwidth_gbps 는 cross-node ib_write_bw(별도 오케스트레이션) 측정 전까지 null. 성능 게이트 최종판정은 측정 후.

stdlib 만 사용. 출력 = JSON(stdout). 종료코드: 0=정상(α 또는 multi-ready 후보), 2=γ blocked, 3=스캔/입력 오류.
"""
from __future__ import annotations
import argparse
import ipaddress
import getpass
import json
import math
import os
import platform
import re
import shlex
import shutil
import subprocess
import sys

IPV4_RE = re.compile(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$")
TOKEN_RE = re.compile(r"^[A-Za-z0-9_.:-]+$")
PRESET_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")
CUDA_RE = re.compile(r"^[0-9]{2,4}$")
HOST_LABEL_RE = re.compile(r"^[A-Za-z0-9](?:[A-Za-z0-9-]{0,61}[A-Za-z0-9])?$")


def valid_host(value) -> bool:
    if not isinstance(value, str) or not value or len(value) > 253:
        return False
    try:
        ipaddress.ip_address(value)
        return True
    except ValueError:
        labels = value[:-1].split(".") if value.endswith(".") else value.split(".")
        return bool(labels) and all(HOST_LABEL_RE.fullmatch(label) for label in labels)


def _run(cmd: list[str], timeout: int = 10) -> str:
    """명령 실행 → stdout(실패/미설치는 빈 문자열, graceful)."""
    exe = shutil.which(cmd[0])
    if not exe:
        return ""
    try:
        out = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        return out.stdout if out.returncode == 0 else ""
    except Exception:
        return ""


def scan_cpu_arch() -> str:
    return platform.machine()  # aarch64 | x86_64


def scan_cuda_version() -> str | None:
    """nvcc --version 의 'release 13.2' → '132'. 미설치/실패 → None(인터뷰 폴백)."""
    txt = _run(["nvcc", "--version"])
    m = re.search(r"release\s+(\d+)\.(\d+)", txt)
    return f"{m.group(1)}{m.group(2)}" if m else None


def scan_gpus_per_node() -> int | None:
    txt = _run(["nvidia-smi", "-L"])
    if not txt:
        return None
    return sum(1 for ln in txt.splitlines() if ln.strip().startswith("GPU "))


def scan_infiniband_hcas() -> list[str]:
    """/sys/class/infiniband 의 HCA 디바이스명(ibstat 비의존). 없으면 빈 리스트(→ α 후보)."""
    p = "/sys/class/infiniband"
    try:
        return sorted(os.listdir(p))
    except OSError:
        return []


def scan_roce_v2_gids() -> list[dict]:
    """show_gids 파싱 → RoCE v2 + IPv4 라우팅 GID 행만.
    반환 [{hca, port, gid_index, ipv4, iface}]. 링크로컬(v1/IPv4 없음)은 제외 — NCCL 이 쓰는 건 v2+IPv4."""
    txt = _run(["show_gids"])
    rows: list[dict] = []
    for ln in txt.splitlines():
        toks = ln.split()
        if len(toks) < 5 or toks[0] in ("DEV", "---"):
            continue
        if "v2" not in toks:
            continue
        ipv4 = next((t for t in toks if IPV4_RE.match(t)), None)
        if not ipv4:
            continue  # v2 이지만 IPv4 없는 링크로컬 → 스킵
        # 형식: DEV PORT INDEX GID [IPv4] VER IFACE  → IPv4 있는 행은 iface=마지막, index=세번째
        rows.append({
            "hca": toks[0],
            "port": toks[1],
            "gid_index": int(toks[2]) if toks[2].isdigit() else toks[2],
            "ipv4": ipv4,
            "iface": toks[-1],
        })
    return rows


def _read_iface_mtu(iface):
    """/sys/class/net/<iface>/mtu 실측. 못 읽으면 None — 9000 같은 값을 지어내지 않는다."""
    if not iface:
        return None
    try:
        with open(f"/sys/class/net/{iface}/mtu", encoding="utf-8") as f:
            return int(f.read().strip())
    except (OSError, ValueError):
        return None


def detect_interconnect() -> dict:
    """환경정체성 interconnect 블록을 결정론적으로 산출."""
    hcas_all = scan_infiniband_hcas()
    v2 = scan_roce_v2_gids()
    if not v2:
        # RoCE v2 라우팅 GID 부재 → 고속 인터커넥트 미탐지(α 후보).
        return {
            "type": "generic-ethernet",
            "hca_devices": [],
            "gid_index": None,
            "socket_iface": None,
            "mtu": None,
            "bandwidth_gbps": None,
            "platform_preset": None,
            "_sysfs_hcas": hcas_all,
            "_roce_v2_gids": [],
        }
    # 라우팅 가능한 v2 GID 가 붙은 HCA = NCCL 이 실제 쓰는 디바이스(예: rocep1s0f1, roceP2p1s0f1).
    # 결정론 tie-break = **최저 IPv4 우선**(=Domain 0/.100 먼저). 문자열 sort 는 대문자 'P'가 앞서 도메인 순서가
    # 뒤집히므로 금지(T1 학습 — compose 의 enp1s0f1np1/.100 bootstrap 의도와 일치시킨다).
    def _ipkey(r: dict):
        return tuple(int(x) for x in r["ipv4"].split("."))
    v2_sorted = sorted(v2, key=_ipkey)
    hca_devices: list[str] = []
    for r in v2_sorted:               # IPv4 오름차순 순서 보존 dedup → [rocep1s0f1, roceP2p1s0f1]
        if r["hca"] not in hca_devices:
            hca_devices.append(r["hca"])
    raw_gid_indices = [row.get("gid_index") for row in v2]
    if all(isinstance(value, int) and not isinstance(value, bool) and value >= 0
           for value in raw_gid_indices):
        gid_idx = sorted(set(raw_gid_indices))
        gid_idx = gid_idx[0] if len(gid_idx) == 1 else gid_idx
    else:
        # Preserve a structured invalid fact. The emission gate and staleness
        # consumer reject it without a traceback or fabricated replacement.
        gid_idx = "invalid"
    return {
        "type": "RoCE v2",
        "hca_devices": hca_devices,
        # GID 인덱스가 행마다 동일하면 단일값, 아니면 리스트(가드)
        "gid_index": gid_idx,
        # bootstrap iface = 최저 IPv4(Domain 0) 의 iface → enp1s0f1np1 (compose 와 정합)
        "socket_iface": v2_sorted[0]["iface"],
        # 2026-09-03(⑬ · plan_26090317 P3): mtu 를 산출하지 않아 emit 이 기존 manifest 의 실측값을
        #   덮어 지웠고, 렌더러는 `INTERCONNECT_MTU` 를 9000 으로 **침묵 폴백**했다 — 9000 이 아닌
        #   링크에서 조용히 틀린다. bootstrap iface 의 실측을 그대로 싣는다.
        "mtu": _read_iface_mtu(v2_sorted[0]["iface"]),
        "bandwidth_gbps": None,  # ib_write_bw 측정 전까지 null
        "platform_preset": None,  # 사람/플랫폼 탐지가 채움(예: dgx-spark-gb10)
        "_sysfs_hcas": hcas_all,
        "_roce_v2_gids": v2,
    }


def _parse_compose_nccl(path: str) -> dict:
    """docker-compose 에서 NCCL_IB_HCA/GID_INDEX/SOCKET_IFNAME 추출(교차검증용, flat grep)."""
    return _parse_nccl_lines(path, sep=":")


def _parse_env_file_nccl(path: str) -> dict:
    """`.env.interconnect`(KEY=VALUE) 에서 NCCL 3키 추출 — S6 env-split 이후의 제2 소스.
    compose inline 이 비어도 여기서 찾으면 교차검증이 침묵 no-op 되지 않는다."""
    return _parse_nccl_lines(path, sep="=")


def _parse_nccl_lines(path: str, sep: str) -> dict:
    """NCCL 3키 flat grep 공통부. `NCCL_IB_HCA==roce…`(env) 와 `NCCL_IB_HCA: "=roce…"`(compose)
    양쪽 다 값 앞의 잔여 '=' 는 호출부(cross_validate)의 lstrip("=")이 처리한다."""
    res: dict = {}
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                s = ln.strip()
                for key in ("NCCL_IB_HCA", "NCCL_IB_GID_INDEX", "NCCL_SOCKET_IFNAME"):
                    if s.startswith(key + sep):
                        val = s.split(sep, 1)[1].strip().strip('"').strip()
                        # 인라인 주석 제거
                        val = val.split("#", 1)[0].strip().strip('"')
                        res[key] = val
    except OSError:
        pass
    return res


def default_env_interconnect_path(compose_path: str) -> str:
    """S6 env-split 통로: compose 와 같은 디렉터리의 `envs/.env.interconnect`."""
    return os.path.join(os.path.dirname(compose_path), "envs", ".env.interconnect")


def cross_validate(ic: dict, compose_path: str, env_interconnect_path: str | None = None) -> dict:
    """스캔 interconnect ↔ 현 배포 NCCL 값 일치 확인.
    소스는 2개다 — compose inline(S6 이전 통로) + envs/.env.interconnect(S6 env-split 이후 통로).
    키별 출처를 `sources` 에 남긴다(헌법 §결정론 규율 — 값 옆에 출처). compose inline 이 같은 키를
    가지면 env 파일보다 우선한다(compose 가 최종 서비스 서술). 둘 다 비면 checked:false + fail-loud
    어휘로 **무엇을 안 봤는지**를 명시한다 — 종전 'compose 없음/NCCL 부재' 문구는 env-split 통로의
    존재 자체를 숨겨 침묵 누락을 낳았다(2026-08-24 plan_26082415 결함 1)."""
    nccl = _parse_compose_nccl(compose_path)
    env_nccl = _parse_env_file_nccl(env_interconnect_path) if env_interconnect_path else {}
    merged: dict = {}
    sources: dict = {}
    for key, val in env_nccl.items():
        merged[key] = val
        sources[key] = "env.interconnect"
    for key, val in nccl.items():
        merged[key] = val
        sources[key] = "compose"
    if not merged:
        return {"checked": False,
                "reason": "NCCL env 부재(compose+env.interconnect 둘 다): "
                          f"compose={compose_path} env={env_interconnect_path or '(미지정)'}"}
    checks = []
    # HCA: compose/env 는 "=rocep1s0f1,roceP2p1s0f1" 형식 → '=' 와 분해
    if "NCCL_IB_HCA" in merged:
        comp_hcas = sorted(x for x in merged["NCCL_IB_HCA"].lstrip("=").split(",") if x)
        checks.append({"field": "hca_devices", "scanned": ic["hca_devices"],
                       "deployed": comp_hcas, "source": sources["NCCL_IB_HCA"],
                       "match": sorted(ic["hca_devices"]) == comp_hcas})
    if "NCCL_IB_GID_INDEX" in merged:
        comp_gid = merged["NCCL_IB_GID_INDEX"]
        checks.append({"field": "gid_index", "scanned": str(ic["gid_index"]),
                       "deployed": comp_gid, "source": sources["NCCL_IB_GID_INDEX"],
                       "match": str(ic["gid_index"]) == comp_gid})
    if "NCCL_SOCKET_IFNAME" in merged:
        checks.append({"field": "socket_iface", "scanned": ic["socket_iface"],
                       "deployed": merged["NCCL_SOCKET_IFNAME"],
                       "source": sources["NCCL_SOCKET_IFNAME"],
                       "match": ic["socket_iface"] == merged["NCCL_SOCKET_IFNAME"]})
    return {"checked": True, "all_match": all(c["match"] for c in checks),
            "sources": sources, "checks": checks}


def peer_reachable(ip: str, port: int = 22, timeout: int = 5) -> bool:
    """peer 도달성(TCP connect — ib_write_bw 아님, 그건 별도 cross-node 오케스트레이션)."""
    import socket
    try:
        with socket.create_connection((ip, port), timeout=timeout):
            return True
    except OSError:
        return False


# ── 외부 egress 스캔 (축 B — plan_26070809_46_57 §4.2 · 인터커넥트 축 A 와 직교) ──
# "서브=물리 에어갭" 전역 상수를 폐기하고 manifest 사실로 대체하는 신규 축. render-on-main 동형
# (메인이 SSH 로 서브를 실측 — 서브 자가스캔 ✗). 결과는 attestation(fail-open) — model_env 만 fail-closed(§4.2).
_EGRESS_PEER_PROBE = (
    "(curl -sS -o /dev/null -w '%{{http_code}}' --max-time {timeout} https://{hf_host} 2>/dev/null || echo 000); "
    "echo; "
    "(timeout {timeout} bash -c '</dev/tcp/{gw_host}/{gw_port}' 2>/dev/null && echo OK || echo FAIL)"
)


def scan_egress(hf_host: str = "huggingface.co", gw_host: str = "8.8.8.8", gw_port: int = 443,
                timeout: int = 5) -> dict:
    """로컬(메인) 외부 egress 프로브 — (i) HF 엔드포인트 HTTPS HEAD (ii) 일반 게이트웨이 TCP connect.
    둘 다 성공 = online, 아니면 restricted(보수적). stdlib 만(urllib+socket) — 결정론 최소권한.
    반환은 attestation 용(non-blocking) — evaluate_gate 의 egress_self/egress_peer 입력."""
    import socket
    import urllib.error
    import urllib.request

    hf_ok = False
    try:
        req = urllib.request.Request(f"https://{hf_host}", method="HEAD")
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            hf_ok = 200 <= resp.status < 500
    except (urllib.error.URLError, OSError, ValueError):
        hf_ok = False

    gw_ok = False
    try:
        with socket.create_connection((gw_host, gw_port), timeout=timeout):
            gw_ok = True
    except OSError:
        gw_ok = False

    return {"egress": "online" if (hf_ok and gw_ok) else "restricted",
            "hf_reachable": hf_ok, "gateway_reachable": gw_ok}


PEER_SSH_LAST_ERROR: dict = {}


def _peer_ssh_diag(what: str, target: str, rc, stderr) -> None:
    """서브 ssh 프로브 실패의 **사유를 보존**한다(S8). stdout 은 스캔 JSON 계약이라 stderr 로만 낸다."""
    detail = (stderr or "").strip()
    if "Host key verification failed" in detail or "REMOTE HOST IDENTIFICATION" in detail:
        cause = "host-key 미등록/불일치 — 메인 known_hosts 에 서브 키를 등록하라(ssh-keyscan)"
    elif "Permission denied" in detail:
        cause = "키 인증 거부 — 서브 authorized_keys 에 메인 공개키를 넣어라"
    elif "Could not resolve hostname" in detail or "Name or service not known" in detail:
        cause = "이름 해소 실패 — 주소/DNS 확인"
    elif "Connection timed out" in detail or "No route to host" in detail or rc is None:
        cause = "네트워크 미도달/타임아웃"
    elif rc == 127 or "command not found" in detail:
        cause = "원격 도구 부재(nvidia-smi/uname 등) — PATH 또는 미설치"
    else:
        cause = "미분류(rc=%s)" % rc
    PEER_SSH_LAST_ERROR[what] = cause
    print("[scan] 서브 %s 프로브 실패: %s" % (what, cause), file=sys.stderr)
    if detail:
        print("[scan]   ssh stderr: %s" % detail[-500:], file=sys.stderr)


def collect_peer_model_env(ssh_target, nas_path, hf_token_env_file, ssh_opts=None) -> dict:
    """서브의 모델 제반환경을 **서브에서** 실측한다(S6).

    수집 실패는 None(미측정)으로 둔다 — 여기서 False 로 접으면 **전송 실패가 "NAS 없음"** 이 되고,
    그건 fail-closed 가 아니라 원인을 바꿔치기하는 것이다. fail-closed 판단은 호출부가 한다.
    """
    opts = ssh_opts or ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
    checks = []
    if nas_path:
        checks.append("[ -d %s ] && echo NAS=1 || echo NAS=0" % shlex.quote(nas_path))
    if hf_token_env_file:
        checks.append("[ -f %s ] && echo TOK=1 || echo TOK=0" % shlex.quote(hf_token_env_file))
    if not checks:
        return {"nas_reachable": None, "hf_token_present": None, "probed": False}
    try:
        out = subprocess.run(["ssh", *opts, ssh_target, "bash", "-ls"],
                             input="\n".join(checks) + "\n", capture_output=True, text=True, timeout=25)
        if out.returncode != 0:
            _peer_ssh_diag("model-env", ssh_target, out.returncode, out.stderr)
            return {"nas_reachable": None, "hf_token_present": None, "probed": False}
        kv = dict(ln.split("=", 1) for ln in out.stdout.split() if "=" in ln)
    except Exception as exc:
        _peer_ssh_diag("model-env", ssh_target, None, "%s: %s" % (type(exc).__name__, exc))
        return {"nas_reachable": None, "hf_token_present": None, "probed": False}
    return {
        "nas_reachable": (kv["NAS"] == "1") if "NAS" in kv else None,
        "hf_token_present": (kv["TOK"] == "1") if "TOK" in kv else None,
        "probed": True, "source": "measured:peer-ssh",
    }


def collect_peer_egress(ssh_target: str, hf_host: str = "huggingface.co", gw_host: str = "8.8.8.8",
                        gw_port: int = 443, timeout: int = 5,
                        ssh_opts: list[str] | None = None) -> dict:
    """서브 egress 를 SSH 로 실측(render-on-main — collect_peer_hw 와 동형: stdin bash -ls 로 인용/PATH 함정 회피).
    SSH/원격 실패 → restricted(보수적 — fail-open 이되 미검증은 restricted 로 귀속)."""
    opts = ssh_opts or ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
    probe = _EGRESS_PEER_PROBE.format(timeout=timeout, hf_host=hf_host, gw_host=gw_host, gw_port=gw_port)
    text = ""
    try:
        out = subprocess.run(["ssh", *opts, ssh_target, "bash", "-ls"],
                             input=probe, capture_output=True, text=True, timeout=timeout * 2 + 10)
        text = out.stdout if out.returncode == 0 else ""
        if out.returncode != 0:
            _peer_ssh_diag("egress", ssh_target, out.returncode, out.stderr)
    except Exception as exc:
        _peer_ssh_diag("egress", ssh_target, None, f"{type(exc).__name__}: {exc}")
        text = ""
    lines = [ln.strip() for ln in text.splitlines() if ln.strip()]
    hf_code = lines[0] if len(lines) > 0 else "000"
    hf_ok = hf_code.isdigit() and 200 <= int(hf_code) < 500
    gw_ok = lines[1] == "OK" if len(lines) > 1 else False
    return {"egress": "online" if (hf_ok and gw_ok) else "restricted",
            "hf_reachable": hf_ok, "gateway_reachable": gw_ok}


def check_model_env(model_source: str | None, egress: str | None,
                    nas_reachable: bool | None = None, hf_token_present: bool | None = None) -> dict:
    """모델 제반환경 판정(§4.2 fail-closed 축 — egress attestation 과 달리 이건 blocking).
    managed: nas_reachable 이 명시 False 일 때만 block(미측정 None 은 정보부족≠실패로 통과).
    ephemeral/custom: egress==restricted ∧ hf_token_present 가 명시 False 일 때만 block(다운로드 물리 불가)."""
    if model_source == "managed":
        if nas_reachable is False:
            return {"ok": False, "reason": "managed: 서브 모델 NAS 경로 도달 불가(fail-closed)"}
        return {"ok": True, "reason": None}
    if model_source in ("ephemeral", "custom"):
        if egress == "restricted" and hf_token_present is False:
            return {"ok": False,
                    "reason": f"{model_source}: HF_TOKEN 부재 ∧ egress-restricted → 다운로드 물리 불가(fail-closed)"}
        return {"ok": True, "reason": None}
    return {"ok": True, "reason": None}


def read_manifest_field(path: str, key: str) -> str | None:
    """manifest.yaml 의 top-level 'key: value' 플랫 grep(pyyaml 비의존 — read_manifest_topology 와 동형).
    존재하나 판독 불가면 ManifestUnreadable(F10)."""
    return _read_manifest_line(path, key + ":")


# ── 서브 HW 수집 + 메인↔서브 동질성 단언 (plan_26063021_14_37 · D2/D3) ──
# render-on-main: terraforming(메인)이 SSH로 서브 HW를 실측하고 동질성을 단언한다(서브 자가스캔 ✗·terraforming_node 영구 main-only).
# 5종 2등급: cpu_arch·gpu_model·gpus_per_node = 정확일치(하드블록) / cuda·driver = major 하드·minor/patch 경고.
# 근거: 양 노드가 *같은 컨테이너 이미지* 구동(컨테이너가 CUDA 추상화) + Ray TP 노드대칭 → 앞 3종 병렬정합성 직결,
#       뒤 2종은 호스트 드라이버 호환 floor(소분류 drift 허용). 동질 GPU only — 혼종 클러스터 범위 밖(헌법 §A2A-위임 Flag 따름정리).
def scan_gpu_model() -> str | None:
    ln = _run(["nvidia-smi", "--query-gpu=name", "--format=csv,noheader"]).strip().splitlines()
    return ln[0].strip() if ln else None


def scan_driver_version() -> str | None:
    ln = _run(["nvidia-smi", "--query-gpu=driver_version", "--format=csv,noheader"]).strip().splitlines()
    return ln[0].strip() if ln else None


def collect_local_hw() -> dict:
    """메인(로컬) HW 동질성 비교용 5종(cuda 는 raw 'X.Y' — 버전 튜플 비교용)."""
    m = re.search(r"release\s+(\d+\.\d+)", _run(["nvcc", "--version"]))
    return {
        "cpu_arch": scan_cpu_arch(),
        "gpu_model": scan_gpu_model(),
        "gpus_per_node": scan_gpus_per_node(),
        "cuda": m.group(1) if m else None,
        "driver": scan_driver_version(),
    }


# 서브 HW 를 단일 SSH 라운드로 수집(printf KEY=VAL — nvidia-smi/nvcc 실패해도 빈값 graceful, SSH 자체 실패만 {}).
_PEER_HW_PROBE = (
    "printf 'ARCH=%s\\n' \"$(uname -m)\"; "
    "printf 'GPUS=%s\\n' \"$(nvidia-smi -L 2>/dev/null | grep -c '^GPU ')\"; "
    "printf 'MODEL=%s\\n' \"$(nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null | head -1)\"; "
    "printf 'DRIVER=%s\\n' \"$(nvidia-smi --query-gpu=driver_version --format=csv,noheader 2>/dev/null | head -1)\"; "
    "printf 'CUDA=%s\\n' \"$(nvcc --version 2>/dev/null | grep -oE 'release [0-9]+\\.[0-9]+' | grep -oE '[0-9]+\\.[0-9]+')\""
)


def collect_peer_hw(ssh_target: str, ssh_opts: list[str] | None = None) -> dict:
    """SSH로 서브 HW 5종 수집(단일 라운드). SSH/원격 실패 → {} (assert_homogeneity 가 fail-closed 블록).
    프로브를 **stdin 으로 login shell(`bash -ls`) 에 투입** — `ssh host bash -lc <PROBE>` 는 ssh 가 argv 를 공백조인해
    원격 로그인셸이 `bash -lc <첫단어>` 로 첫 printf 만 먹고 나머지는 비-login 셸로 흘려(PATH 누락 — uname/nvcc 유실) 깨진다.
    stdin 투입은 인용 함정·PATH 누락 양쪽을 회피한다(라이브 E2E 발견 — plan_26063021_14_37)."""
    opts = ssh_opts or ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
    text = ""
    # 2026-09-03(S8 · plan_26090317 P1): host-key 거부 / 네트워크 불통 / 원격 nvidia-smi 부재가 모두
    #   `{}` 로 접혀 상류가 "서브 HW 수집 실패(SSH/nvidia-smi 미도달)" 한 문구만 남겼다. stderr 는
    #   capture 해 놓고 아무도 읽지 않았다 — 첫 라이브 온보딩에서 어디를 고칠지 알 수 없다.
    try:
        out = subprocess.run(["ssh", *opts, ssh_target, "bash", "-ls"],
                             input=_PEER_HW_PROBE, capture_output=True, text=True, timeout=25)
        text = out.stdout if out.returncode == 0 else ""
        if out.returncode != 0:
            _peer_ssh_diag("HW", ssh_target, out.returncode, out.stderr)
    except Exception as exc:
        _peer_ssh_diag("HW", ssh_target, None, f"{type(exc).__name__}: {exc}")
        text = ""
    if not text.strip():
        return {}
    kv: dict = {}
    for ln in text.splitlines():
        if "=" in ln:
            k, _, v = ln.partition("=")
            kv[k.strip()] = v.strip()
    g = kv.get("GPUS", "")
    return {
        "cpu_arch": kv.get("ARCH") or None,
        "gpu_model": kv.get("MODEL") or None,
        "gpus_per_node": int(g) if g.isdigit() else None,
        "cuda": kv.get("CUDA") or None,
        "driver": kv.get("DRIVER") or None,
    }


def _ver_tuple(s):
    """버전 문자열 → (major, minor) int 튜플. 미상 → None."""
    if not s:
        return None
    nums = re.findall(r"\d+", str(s))
    return tuple(int(x) for x in nums[:2]) if nums else None


def assert_homogeneity(local: dict, peer: dict) -> dict:
    """메인↔서브 HW 동질성 단언(순수함수 → --self-test 회귀). 5종 2등급.
    정확일치(하드): cpu_arch·gpu_model·gpus_per_node / 버전(major 하드·minor 경고): cuda·driver.
    반환 {verified, blocks[], warns[], detail{}}. peer 미수집 = 검증불가 = 블록(fail-closed)."""
    if not peer:
        return {"verified": False, "warns": [], "detail": {},
                "blocks": ["서브 HW 수집 실패(SSH/nvidia-smi 미도달) — 동질성 미검증(fail-closed)"]}
    blocks: list[str] = []
    warns: list[str] = []
    detail: dict = {}
    for f in ("cpu_arch", "gpu_model", "gpus_per_node"):
        lv, pv = local.get(f), peer.get(f)
        detail[f] = {"main": lv, "sub": pv, "match": (lv == pv and lv is not None)}
        if lv is None or pv is None:   # None==None 은 '동질'이 아니라 '수집 실패' — cuda/driver 와 동형 가드(WARN-4)
            blocks.append(f"{f} 미상(하드): main={lv} sub={pv} — 수집 실패(None==None 은 동질 아님)")
        elif lv != pv:
            blocks.append(f"{f} 불일치(하드): main={lv} ≠ sub={pv}")
    for f in ("cuda", "driver"):
        lv, pv = local.get(f), peer.get(f)
        lt, pt = _ver_tuple(lv), _ver_tuple(pv)
        detail[f] = {"main": lv, "sub": pv, "major_match": bool(lt and pt and lt[0] == pt[0])}
        if not lt or not pt:
            blocks.append(f"{f} 버전 미상(main={lv} sub={pv}) — 검증 불가(하드)")
        elif lt[0] != pt[0]:
            blocks.append(f"{f} major 불일치(하드): main={lv} ≠ sub={pv}")
        elif lt != pt:
            warns.append(f"{f} minor/patch drift(허용·경고): main={lv} sub={pv}")
    return {"verified": not blocks, "blocks": blocks, "warns": warns, "detail": detail}


def git_branch() -> str | None:
    """현재 git 브랜치(3자-일치 단언용). 실패 시 None."""
    out = _run(["git", "rev-parse", "--abbrev-ref", "HEAD"]).strip()
    return out or None


class ManifestUnreadable(Exception):
    """manifest 가 **존재하는데** 읽히지 않는다(권한·소유권·NFS). 2026-09-03(F10 · plan_26090317 P1):
    이전에는 이 상태가 `OSError: pass; return None` 으로 "부재" 와 같은 값이 됐다. 부재는 첫 온보딩의
    정당한 상태라 게이트가 그 축을 **건너뛰도록** 설계돼 있었고, 그래서 판독 불가가 `consistent: true`
    · `gate.status: ok` · exit 0 을 만들었다(b735880 ③ 이 닫은 3자일치 게이트가 다시 열린다).
    모르는 것은 넘기지 않는다."""


def _read_manifest_line(path: str, prefix: str) -> str | None:
    if not os.path.exists(path):
        return None                      # 부재 = 정당한 첫 온보딩 상태
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                t = ln.strip()
                if t.startswith(prefix):
                    return t.split(":", 1)[1].split("#", 1)[0].strip().strip('"')
    except OSError as exc:
        raise ManifestUnreadable(f"{path}: {exc}") from exc
    return None


def read_manifest_topology(path: str = "manifest.yaml") -> str | None:
    """manifest.yaml 의 topology 값(flat grep — pyyaml 비의존). 존재하나 판독 불가면 ManifestUnreadable."""
    return _read_manifest_line(path, "topology:")


# NCCL 프리셋 파생 — render_dockerfile.NCCL_PRESETS 의 키 집합과 1:1로 맞춘 **닫힌** 표.
# 2026-09-03(B3 · plan_26090317 P1): 이전 emit 는 platform_preset 을 항상 `null` 로 냈고,
#   render_dockerfile.build_context 는 `preset_key not in NCCL_PRESETS` 에서 KeyError 로 즉사했다 —
#   즉 emit 을 그대로 반영한 multi manifest 는 **렌더가 100% 죽는** 값이었다. 사람이 손으로 채워야
#   한다는 안내는 인라인 주석 한 줄뿐이었다. GPU 모델에서 결정론으로 파생하고 출처를 함께 적는다.
PLATFORM_PRESET_BY_GPU = (("GB10", "dgx-spark-gb10"),)
PLATFORM_PRESET_DEFAULT = "generic"


def derive_platform_preset(gpu_model) -> tuple[str, str]:
    """(preset, source) — 파생 실패는 없다(generic 이 닫힌 표의 기본값이다)."""
    if isinstance(gpu_model, str):
        for needle, preset in PLATFORM_PRESET_BY_GPU:
            if needle.lower() in gpu_model.lower():
                return preset, f"derived:gpu_model~{needle}"
    return PLATFORM_PRESET_DEFAULT, "default:no-gpu-model-match"


def _yaml_str(value) -> str:
    return json.dumps(value, ensure_ascii=False) if isinstance(value, str) and value.strip() else "null"


def emit_manifest_block(result: dict) -> str:
    """검증된 스캔 결과 → manifest.yaml 에 기입할 YAML 블록(문자열).

    2026-09-03(B2·B3·S7·⑬ · plan_26090317 P1) 재작성. 이전 판본의 산출물은 **그대로 반영하면
    하류가 죽는** 블록이었다:
      · Flag 를 valid 로 만드는 `model_source` 와 렌더 필수 `nas_model_path`·`nodes[].ssh_user`·
        `nodes[].work_dir` 가 없었다 → manifest_contract exit 5 / render exit 2.
      · `hw_verified` 는 계약 스켈레톤에도 없고 emit 도 안 했다 → A2A 위임 키 영구 미발급.
      · `platform_preset: null` → 렌더 KeyError.
      · `mtu` 를 빠뜨려 재기입 시 기존 값이 소실됐고, single 은 `nodes: []` 를 하드코딩해
        등록된 main 항목을 지웠다(§2.7.0 이 허용하는 single+sub 조합도 함께 지운다).
      · 같은 출력 안에 "이 블록으로 덮어써라" 와 "sub 항목에 이것을 추가하라"(병합) 가 공존했다.
    이제 아는 값은 채우고, 모르는 값은 **`__REQUIRED__` 센티넬**로 남겨 사람이 채우게 한다 —
    조용한 기본값을 넣지 않는다(그것이 침묵 폴백의 씨앗이다).
    """
    ic = result["interconnect"]
    hcas = "[" + ", ".join(ic["hca_devices"]) + "]"
    topo = "multi" if result["topology_declared"] == "multi" else "single"
    # None-가드(plan_26063009_44_23): 프로브 실패(예: nvidia-smi PATH 미노출)로 None 인 필드는 YAML null.
    cuda = f'"{result["cuda_version"]}"' if result["cuda_version"] is not None else "null"
    gpus = result["gpus_per_node"] if result["gpus_per_node"] is not None else "null"
    gpu_model = _yaml_str(result.get("gpu_model"))
    driver = _yaml_str(result.get("driver_version"))
    import datetime
    scanned_at = datetime.datetime.now().strftime("%Y%m%d%H")
    # 테라포밍 완수 Flag (attestation · plan_26063018 · 헌법 §테라포밍-완수 Flag 게이트):
    # 이 emit 는 evaluate_gate status==ok(§1.5 3자일치 + α/γ 통과) 시에만 호출되므로 complete:true·branch_verified:true 기입.
    lines = [
        "# 테라포밍 완수 Flag (attestation · plan_26063018) — scan §1.5 3자일치 통과 시 기입(보수적).",
        "terraforming:",
        "  complete: true",
        "  branch_verified: true   # git 브랜치 ↔ topology ↔ scan 3자일치 단언 통과",
        f"  scanned_at: \"{scanned_at}\"",
        f"topology: {topo}",
        f"cpu_arch: \"{result['cpu_arch']}\"",
        f"cuda_version: {cuda}",
        f"gpus_per_node: {gpus}",
        f"gpu_model: {gpu_model}",
        f"driver_version: {driver}",
    ]

    # ── 모델 획득(인터뷰 확정분) — 있으면 채우고, 없으면 센티넬 ──
    ms = result.get("model_source")
    lines.append(f"model_source: {ms}" if ms in ("managed", "ephemeral", "custom")
                 else "model_source: __REQUIRED__   # managed|ephemeral|custom — 인터뷰 확정값. "
                      "미기입 시 manifest_contract exit 5(3 런타임 스킬 info-only)")
    nas = result.get("nas_model_path")
    lines.append(f"nas_model_path: {_yaml_str(nas)}" if nas
                 else "nas_model_path: __REQUIRED__   # render_sub_env NAS_MOUNT 필수 — 미기입 시 렌더 exit 2")

    # ── network.egress (S7: 계약에만 있고 emit 경로가 없던 축) ──
    eg = (result.get("egress") or {}).get("self") or {}
    if eg.get("egress"):
        lines += ["network:",
                  f"  egress: {eg['egress']}          # --check-egress 실측(hf_reachable/gateway_reachable)"]

    # ── nodes[] (⑬: 덮어쓰기와 병합 지시의 공존을 없앤다 — 한 블록으로 완결) ──
    lines.append("nodes:")
    for node in result.get("nodes", []) or []:
        role = node["role"]
        lines.append(f"  - role: {role}")
        lines.append(f"    host: \"{node['host']}\"")
        lines.append(f"    hostname: {_yaml_str(node.get('hostname')) if node.get('hostname') else '__REQUIRED__'}"
                     "   # CLAUDE.template SUB_HOSTNAME 계약" if role == "sub"
                     else f"    hostname: {_yaml_str(node.get('hostname'))}")
        lines.append(f"    ssh_user: {_yaml_str(node.get('ssh_user'))}" if node.get("ssh_user")
                     else "    ssh_user: __REQUIRED__   # sync_to_sub 주소 해소·A2A 위임 계정")
        lines.append(f"    work_dir: {_yaml_str(node.get('work_dir'))}" if node.get("work_dir")
                     else "    work_dir: __REQUIRED__   # 서브 프로젝트 절대경로(--sub-work-dir 로 주입 가능)")
        if role == "sub":
            hv = result.get("homogeneity", {}).get("verified")
            if hv:
                ph = result["homogeneity"]["peer"]
                lines.append("    hw_verified: true        # --peer-ssh 동질성 단언 통과 → A2A 위임 키 발급 자격")
                lines.append(f"    gpu_model: {_yaml_str(ph.get('gpu_model'))}")
                lines.append(f"    driver_version: {_yaml_str(ph.get('driver'))}")
                lines.append(f"    cuda_version_raw: {_yaml_str(ph.get('cuda'))}")
            else:
                lines.append("    hw_verified: false       # --peer-ssh 동질성 미검증 → 위임 키 미발급(info-only)")
    if not (result.get("nodes") or []):
        lines.append("  []   # 노드 미등록. single 도 서브 등록이 가능하다(§2.7.0) — 등록 시 위 형식으로 추가")

    preset, preset_source = derive_platform_preset(result.get("gpu_model"))
    lines += [
        "interconnect:",
        f"  type: {ic['type']}",
        f"  hca_devices: {hcas}",
        f"  gid_index: {ic['gid_index'] if ic['gid_index'] is not None else 'null'}",
        f"  socket_iface: {ic['socket_iface'] if ic['socket_iface'] is not None else 'null'}",
        f"  mtu: {ic['mtu'] if ic.get('mtu') is not None else 'null'}",
        f"  bandwidth_gbps: {ic['bandwidth_gbps'] if ic['bandwidth_gbps'] is not None else 'null'}",
        f"  bandwidth_source: {'measured:ib_write_bw-injected' if ic['bandwidth_gbps'] is not None else 'null'}",
        f"  platform_preset: {preset}",
        f"  platform_preset_source: {preset_source}   # 결정론 파생(헌법 §결정론 규율 — 값 옆에 출처)",
    ]
    lines += [
        "# ⚠ `__REQUIRED__` 가 남아 있으면 그 manifest 는 아직 불완전하다 — 하류(manifest_contract·render_sub_env)가",
        "#   그 자리에서 fail-closed 한다. 값을 채우거나 해당 스캔 플래그(--model-source·--peer-ssh·--sub-work-dir)를 주고 재스캔한다.",
        "# ⚠ host_safety.installed(true|false): terraforming 세션 최종 Y/N 답변 후 기입(scan 무증거 기입 ✗ · Flag 와 독립 —",
        "#   안전체계 미설치여도 Flag valid. 헌법 §호스트 안전체계 따름정리 선택화 · plan_26071115).",
    ]
    return "\n".join(lines) + "\n"


def evaluate_gate(*, declared, ic_present, peer_given, peer_reachable, bandwidth, bw_floor,
                  per_port_gbps=None, per_port_floor=None,
                  branch=None, branch_topo=None, mani_topo=None, mani_path="manifest.yaml",
                  peer_ip=None, egress_self=None, egress_peer=None,
                  model_env_ok=None, model_env_reason=None, manifest_read_error=None):
    """결정론 게이트 판정 — **순수 함수**(I/O 없음 → --self-test 회귀 대상). 반환 (assertion, gate, exit_code).
    3자-일치 단언(branch↔manifest↔scan) + α/γ(구조)/γ(성능 fail-closed)/multi-ready 판정을 한 곳에 codify.
    egress_self/egress_peer: attestation 뿐(불일치=경고·fail-open — 서브 능력차는 정상, plan_26070809_46_57 §4.2).
    model_env_ok=False: **blocking**(모델 제반환경 불충족 — 서브=에어갭 전역상수를 대체하는 유일한 fail-closed 축)."""
    mism: list[str] = []     # blocking(혼재/위험)
    warns: list[str] = []    # 비blocking(정보)
    if declared in ("single", "multi"):
        if branch_topo and branch_topo != declared:
            mism.append(f"declared={declared} ≠ git branch({branch})⇒{branch_topo}")
        elif not branch_topo:
            # ★ 2026-09-01 (audit_26090109 ③) — 종전에는 두 대조 다리가 **모두**
            #   `branch_topo` 가 참일 때만 걸렸다. 그래서 브랜치 미해소(detached HEAD ·
            #   규약 밖 이름) 한 번이면 3자-일치 단언이 **통째로 증발**하고 Flag 가
            #   fail-open 으로 발급됐다. **판정 불가는 통과가 아니다** — 이 저장소가
            #   "부재와 판단 불가의 융합"이라 부른 결함의 교과서 사례다.
            mism.append(
                f"git branch({branch}) 로 토폴로지를 해소하지 못했다 — "
                f"3자-일치(branch↔manifest↔scan)를 판정할 수 없다(fail-closed)")
        if declared == "multi" and not ic_present:
            mism.append("declared=multi 인데 스캔: RoCE v2 미탐지")
        if declared == "single" and ic_present:
            warns.append("single 선언 + RoCE 하드웨어 존재 — 멀티 가능 머신의 단일노드 운용(정상·정보)")
        # ★ manifest 는 branch 를 **경유하지 않고** declared 와 직접 대조한다(2026-09-01 · ③).
        #   종전에는 branch 를 거쳐서만 비교해, declared=single + manifest=multi 라는
        #   **정면 모순**이 consistent=True · exit 0 으로 통과했다. 헌법은 manifest 를
        #   하드웨어·TP 의 단일 권위로 두고 브랜치 추론을 금지하는데, 실제 배선은 그
        #   권위를 브랜치에 **종속**시키고 있었다.
        if mani_topo and mani_topo != declared:
            mism.append(f"manifest({mani_path}) topology={mani_topo} ≠ declared={declared}")
        if mani_topo and branch_topo and mani_topo != branch_topo:
            mism.append(f"manifest({mani_path}) topology={mani_topo} ≠ branch⇒{branch_topo}")
    if egress_self and egress_peer and egress_self != egress_peer:
        warns.append(f"egress 능력차(정보·fail-open): main={egress_self} sub={egress_peer} — 서브는 자동 격하(루프라인-only)")
    model_block = (model_env_reason or "모델 제반환경 불충족(fail-closed)") if model_env_ok is False else None
    if manifest_read_error:
        # F10: 존재하는데 못 읽는 manifest 는 "없는 것" 이 아니다 — 3자일치를 무증거로 통과시킬 수 없다.
        mism.append(f"manifest 판독 불가(부재 아님): {manifest_read_error}")
    assertion = {
        "git_branch": branch, "branch_implies": branch_topo,
        "manifest_path": mani_path, "manifest_topology": mani_topo, "declared": declared,
        "scan_interconnect_present": ic_present,
        "egress_self": egress_self, "egress_peer": egress_peer,
        "consistent": not mism, "mismatches": mism, "warnings": warns,
        # 음성정직: 평가하지 않은 축은 조용히 통과시키지 않고 그 사실을 남긴다(헌법 §결정론 규율).
        "per_port_evaluated": bool(per_port_gbps is not None and per_port_floor is not None),
        "per_port_gbps": per_port_gbps, "per_port_floor": per_port_floor,
        "bandwidth_source": ("injected:--bandwidth-gbps" if bandwidth is not None else "unmeasured"),
    }
    exit_code = 0
    if declared == "single":
        blockers = list(mism)
        if model_block:
            blockers.append(model_block)
        gate = {"branch": "alpha", "status": "ok" if not blockers else "blocked", "mismatches": blockers,
                "note": "single-node: interconnect 스캔 skip(실패 아님)" if not blockers else "일관성 단언 실패 → blocked"}
        if blockers:
            exit_code = 2
    elif declared == "multi":
        struct_block = []
        if not ic_present:
            struct_block.append("RoCE v2 미탐지")
        if not peer_given:
            struct_block.append("peer-ip 미지정(도달성 미검증)")
        elif not peer_reachable:
            struct_block.append(f"peer {peer_ip} 미도달")
        if model_block:
            struct_block.append(model_block)
        if (isinstance(bw_floor, bool) or not isinstance(bw_floor, (int, float))
                or not math.isfinite(bw_floor) or bw_floor < 0):
            struct_block.append("대역폭 합격선이 finite non-negative number가 아님")
        if bandwidth is not None and (
                isinstance(bandwidth, bool) or not isinstance(bandwidth, (int, float))
                or not math.isfinite(bandwidth) or bandwidth < 0):
            struct_block.append("대역폭 측정값이 finite non-negative number가 아님")
        struct_block += mism
        if struct_block:                                   # γ: 구조/일관성 미충족
            gate = {"branch": "gamma", "status": "blocked", "reasons": struct_block,
                    "note": "멀티-ready manifest 미생성. 구조/일관성 확보 후 재실행."}
            exit_code = 2
        elif bandwidth is None:                            # 구조 충족, 성능 **미측정**
            # 2026-09-03(B4 · plan_26090317 P1): 이 분기는 exit 0 이었다. 차단은 emit 억제뿐이라,
            #   온보딩 러너가 `scan_node.py … || abort` 관용구를 쓰면 **성능 미검증을 성공으로 읽는다**
            #   — SKILL.md §1 머리의 "성능 미검증 멀티 진행 금지(fail-closed)" 와 종료코드 계약이
            #   정면으로 어긋나 있었다. 미측정은 통과가 아니라 **미완**이므로 비-0 으로 낸다.
            gate = {"branch": "multi-ready-candidate", "status": "pending-perf", "pending": ["bandwidth_gbps"],
                    "note": (f"구조 충족·성능 미측정(fail-closed). ib_write_bw 로 포트당 ≥{per_port_floor}Gb/s "
                             f"및 합산 ≥{bw_floor}Gb/s 를 측정해 --bandwidth-gbps(및 --per-port-gbps)로 주입하라.")}
            exit_code = 2
        elif bandwidth < bw_floor or (per_port_gbps is not None and per_port_floor is not None
                                      and per_port_gbps < per_port_floor):
            # 2026-09-03(B4): SKILL.md §1.4 합격선은 "포트당 ≥100 **&** 합산 ≥180" 인데 코드엔 합산만
            #   있었다 — 포트당 조건은 **집행 불가한 문서상의 선언**이었다. 한쪽 포트가 죽어 나머지가
            #   과대보상하는 구성이 합산만으로는 통과한다.
            _reasons = []
            if bandwidth < bw_floor:
                _reasons.append(f"합산 대역폭 {bandwidth}Gb/s < 합격선 {bw_floor}Gb/s")
            if per_port_gbps is not None and per_port_floor is not None and per_port_gbps < per_port_floor:
                _reasons.append(f"포트당 대역폭 {per_port_gbps}Gb/s < 합격선 {per_port_floor}Gb/s")
            gate = {"branch": "gamma", "status": "blocked",
                    "reasons": _reasons,
                    "note": ("성능 미달 → 멀티-ready 거부(fail-closed). 오설정 점검(케이블 수/GID/MTU/GDR). "
                             f"합격선 {bw_floor}은 200Gbps 플랫폼 파생 기본값 — 저속-그러나-정상 링크(예 100GbE)면 "
                             "불가가 아니라 `--bw-floor <합산 line-rate×0.9>` 재설정 대상(HITL·시도-우선, SKILL §1.4).")}
            exit_code = 2
        else:                                              # multi-ready: 구조+성능 통과
            gate = {"branch": "multi-ready", "status": "ok", "bandwidth_gbps": bandwidth,
                    "note": f"구조+성능 통과(대역폭 {bandwidth} ≥ {bw_floor}Gb/s). 멀티-ready."}
    else:  # auto — 보고만
        gate = {"branch": "auto", "status": "report-only",
                "note": "토폴로지 미선언 — single/multi 명시 시 게이트 판정. 스캔 사실만 보고."}
    return assertion, gate, exit_code


def emit_gate(emit_manifest: bool, topology: str) -> int:
    """fail-closed: --emit-manifest 는 **명시 토폴로지(single|multi)** 필요 — auto면 거부(비0 종료 3).
    토폴로지는 terraforming_node 진입의 **인터뷰**로 선언한다(브랜치/스캔 추론으로 manifest 기입 금지).
    순수함수 → --self-test 회귀. 근거: plan_26063009_44_23 D3(토폴로지 인터뷰 fail-closed 게이트)."""
    return 3 if (emit_manifest and topology == "auto") else 0


def emission_blockers(result: dict) -> list[str]:
    """Canonical scan facts that forbid a `complete:true` manifest emission."""
    blockers = []
    for field in ("cpu_arch", "cuda_version", "gpu_model"):
        if not isinstance(result.get(field), str) or not result[field].strip():
            blockers.append(field)
    if result.get("cpu_arch") not in ("aarch64", "x86_64", "amd64"):
        blockers.append("cpu_arch")
    if isinstance(result.get("cuda_version"), str) and not CUDA_RE.fullmatch(result["cuda_version"]):
        blockers.append("cuda_version")
    count = result.get("gpus_per_node")
    if isinstance(count, bool) or not isinstance(count, int) or count < 1:
        blockers.append("gpus_per_node")
    if result.get("topology_declared") not in ("single", "multi"):
        blockers.append("topology_declared")
    interconnect = result.get("interconnect")
    if not isinstance(interconnect, dict):
        blockers.append("interconnect")
        return sorted(blockers)
    bandwidth = interconnect.get("bandwidth_gbps")
    if ("bandwidth_gbps" not in interconnect
            or (bandwidth is not None and (
                isinstance(bandwidth, bool) or not isinstance(bandwidth, (int, float))
                or not math.isfinite(bandwidth) or bandwidth < 0))):
        blockers.append("interconnect.bandwidth_gbps")
    if interconnect.get("type") not in ("generic-ethernet", "RoCE v2"):
        blockers.append("interconnect.type")
    hcas = interconnect.get("hca_devices")
    if (not isinstance(hcas, list)
            or any(not isinstance(item, str) or not TOKEN_RE.fullmatch(item) for item in hcas)):
        blockers.append("interconnect.hca_devices")
    gid_index = interconnect.get("gid_index")
    gid_scalar_valid = (
        gid_index is None
        or (isinstance(gid_index, int) and not isinstance(gid_index, bool) and gid_index >= 0)
    )
    gid_list_valid = (
        isinstance(gid_index, list) and len(gid_index) >= 2
        and all(isinstance(item, int) and not isinstance(item, bool) and item >= 0
                for item in gid_index)
        and gid_index == sorted(set(gid_index))
    )
    if "gid_index" not in interconnect or not (gid_scalar_valid or gid_list_valid):
        blockers.append("interconnect.gid_index")
    socket_iface = interconnect.get("socket_iface")
    if ("socket_iface" not in interconnect or
            (socket_iface is not None and (
                not isinstance(socket_iface, str) or not TOKEN_RE.fullmatch(socket_iface)))):
        blockers.append("interconnect.socket_iface")
    preset = interconnect.get("platform_preset")
    if ("platform_preset" not in interconnect or
            (preset is not None and (
                not isinstance(preset, str) or not PRESET_RE.fullmatch(preset)))):
        blockers.append("interconnect.platform_preset")
    if interconnect.get("type") == "RoCE v2":
        if not isinstance(hcas, list) or not hcas:
            blockers.append("interconnect.hca_devices")
        if gid_index is None:
            blockers.append("interconnect.gid_index")
        if socket_iface is None:
            blockers.append("interconnect.socket_iface")
    if result.get("topology_declared") == "multi":
        nodes = result.get("nodes")
        roles = [node.get("role") for node in nodes if isinstance(node, dict)] \
            if isinstance(nodes, list) else []
        if roles.count("main") != 1 or roles.count("sub") < 1:
            blockers.append("nodes.roster")
        if isinstance(nodes, list):
            for index, node in enumerate(nodes):
                if not isinstance(node, dict):
                    blockers.append("nodes[%d]" % index)
                    continue
                if node.get("role") not in ("main", "sub"):
                    blockers.append("nodes[%d].role" % index)
                host = node.get("host")
                if not valid_host(host):
                    blockers.append("nodes[%d].host" % index)
    return sorted(set(blockers))


def _self_test() -> int:
    """결정론 회귀(게이트 9분기 + emit_gate 4 + emit-block None-leak 2 = 15) — 실 2노드 라이브 검증(testlog_26062314) fixture 고정(#4). 하드웨어 불요."""
    F = 180.0
    cases = [
        # name, kwargs, (expect_gate, expect_exit, expect_consistent)
        ("multi-ready(구조+성능)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("multi-ready", 0, True)),
        ("multi γ(성능 fail-closed)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=100.0, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("gamma", 2, True)),
        ("multi γ(peer 미도달)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=False, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi", peer_ip="x"), ("gamma", 2, True)),
        ("multi γ(no RoCE)", dict(declared="multi", ic_present=False, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("gamma", 2, False)),
        ("multi pending-perf(미측정=fail-closed)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=None, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("multi-ready-candidate", 2, True)),
        ("multi 포트당 미달(합산은 통과)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=200.0, bw_floor=F, per_port_gbps=60.0, per_port_floor=100.0, branch_topo="multi", mani_topo="multi"), ("gamma", 2, True)),
        ("multi 포트당·합산 모두 통과", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.0, bw_floor=F, per_port_gbps=104.0, per_port_floor=100.0, branch_topo="multi", mani_topo="multi"), ("multi-ready", 0, True)),
        ("single α", dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="single", mani_topo="single"), ("alpha", 0, True)),
        ("single α + RoCE(경고 비blocking)", dict(declared="single", ic_present=True, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="single", mani_topo="single"), ("alpha", 0, True)),
        ("single on multi-branch(혼재차단)", dict(declared="single", ic_present=True, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("alpha", 2, False)),
        ("manifest≠branch(혼재차단)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="single"), ("gamma", 2, False)),
        # ★ audit_26090109 ③ 회귀 2건 (2026-09-01). 종전에는 두 대조가 모두 branch_topo 를
        #   경유해, 아래 첫 케이스(declared 와 manifest 의 **정면 모순**)가 consistent=True ·
        #   exit 0 으로 통과했고, 둘째(브랜치 미해소)는 단언 자체가 증발해 Flag 가 fail-open
        #   으로 발급됐다. 이 두 줄이 그 두 문을 닫는다.
        ("★declared≠manifest 직접모순(③)", dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch="single-node", branch_topo="single", mani_topo="multi"), ("alpha", 2, False)),
        ("★branch 미해소 → 3자일치 판정불가(③)", dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch="feature/x", branch_topo=None, mani_topo="single"), ("alpha", 2, False)),
    ]
    passed = 0
    for name, kw, (eg, ec, econ) in cases:
        a, g, code = evaluate_gate(**kw)
        ok = (g["branch"] == eg and code == ec and a["consistent"] == econ)
        passed += ok
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}: gate={g['branch']} exit={code} consistent={a['consistent']}"
              + ("" if ok else f"  ← 기대 ({eg},{ec},{econ})"))
    n = len(cases)
    # emit_gate fail-closed 회귀(plan_26063009_44_23 D3): --emit-manifest 는 명시 토폴로지 필요.
    eg_cases = [
        ("emit+auto → 거부(fail-closed)", (True, "auto"), 3),
        ("emit+single → 통과", (True, "single"), 0),
        ("emit+multi → 통과", (True, "multi"), 0),
        ("no-emit+auto → 통과(보고만)", (False, "auto"), 0),
    ]
    for name, (em, topo), expect in eg_cases:
        got = emit_gate(em, topo)
        ok = got == expect
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}: emit_gate={got}" + ("" if ok else f"  ← 기대 {expect}"))
    # emit_manifest_block 회귀. 원래 축(None-leak · Flag attestation)은 유지하고, 2026-09-03(P1)에
    # **하류 성립성** 축을 더한다 — 이전 판본은 `nodes: []`(single 하드코딩)를 *요구*하고 있었는데
    # 그것이 바로 ⑬ 결함(등록된 main 항목 소실)이었다. 시험이 결함을 고정하고 있었던 셈이다.
    # 새 계약: 아는 값은 채우고, 모르는 값은 `__REQUIRED__` 센티넬로 남기며, preset 은 절대 null 이 아니다.
    emit_cases = [
        ("single GPU-less(전필드 None)", {"topology_declared": "single", "cpu_arch": "x86_64", "cuda_version": None,
            "gpus_per_node": None, "nodes": [{"role": "main", "host": "node-a", "hostname": "node-a",
            "ssh_user": "u", "work_dir": "/w"}],
            "interconnect": {"type": "generic-ethernet", "hca_devices": [], "gid_index": None,
            "socket_iface": None, "bandwidth_gbps": None, "platform_preset": None}}, ["main"]),
        ("multi RoCE(채워짐)", {"topology_declared": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
            "gpus_per_node": 1, "gpu_model": "NVIDIA GB10", "model_source": "managed",
            "nas_model_path": "/mnt/llm/Model/hugging_face_ver_model",
            "nodes": [{"role": "main", "host": "node-a", "hostname": "node-a", "ssh_user": "u", "work_dir": "/w"},
                      {"role": "sub", "host": "node-b", "hostname": "node-b", "ssh_user": "u", "work_dir": "/w"}],
            "homogeneity": {"verified": True, "peer": {"gpu_model": "NVIDIA GB10", "driver": "580.173.02", "cuda": "13.2"}},
            "interconnect": {"type": "RoCE v2", "hca_devices": ["mlx5_0"], "gid_index": 3, "mtu": 9000,
            "socket_iface": "enp1s0f0", "bandwidth_gbps": 208.2, "platform_preset": "dgx-spark-gb10"}}, ["main", "sub"]),
        ("multi 서브 미상(센티넬 남김)", {"topology_declared": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
            "gpus_per_node": 1, "gpu_model": "NVIDIA GB10",
            "nodes": [{"role": "main", "host": "node-a", "hostname": "node-a", "ssh_user": "u", "work_dir": "/w"},
                      {"role": "sub", "host": "node-b"}],
            "interconnect": {"type": "RoCE v2", "hca_devices": [], "gid_index": None, "socket_iface": None,
            "bandwidth_gbps": None, "platform_preset": None}}, ["main", "sub"]),
    ]
    for name, res, want_roles in emit_cases:
        blk = emit_manifest_block(res)
        no_none = "None" not in blk                                  # bare Python None 누수 0
        nodes_ok = "nodes:" in blk and all(f"role: {r}" in blk for r in want_roles)
        attest_ok = ("terraforming:" in blk) and ("complete: true" in blk) and ("branch_verified: true" in blk)
        # B3: preset 은 어떤 입력에서도 렌더러의 닫힌 키 집합 안에서 나온다(null 이면 렌더가 KeyError 로 죽는다).
        preset_ok = ("platform_preset: dgx-spark-gb10" in blk or "platform_preset: generic" in blk) \
            and "platform_preset: null" not in blk and "platform_preset_source:" in blk
        # B2: 하류 필수 키가 값이든 센티넬이든 **자리로는 반드시** 있어야 한다(없으면 사람이 존재조차 모른다).
        keys_ok = all(k in blk for k in ("model_source:", "nas_model_path:", "ssh_user:", "work_dir:"))
        # 센티넬 대조: 아는 케이스엔 __REQUIRED__ 가 없고, 모르는 케이스엔 있다(둘 다 확인 — 한쪽만 보면 위양성).
        # 말미 안내 주석에도 그 단어가 나오므로 **값 줄만** 센다(주석을 세면 항상 참이 되어 시험이 공허해진다).
        value_lines = [ln for ln in blk.splitlines() if not ln.lstrip().startswith("#")]
        has_sentinel = any("__REQUIRED__" in ln for ln in value_lines)
        if name.startswith("multi RoCE"):
            sentinel_ok = (not has_sentinel) and "hw_verified: true" in blk
        elif name.startswith("multi 서브 미상"):
            sentinel_ok = has_sentinel and "hw_verified: false" in blk
        else:
            sentinel_ok = has_sentinel                 # model_source 미상 → 센티넬
        ok = no_none and nodes_ok and "topology:" in blk and attest_ok and preset_ok and keys_ok and sentinel_ok
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] emit:{name}: no-None={no_none} nodes={nodes_ok} attest={attest_ok} "
              f"preset={preset_ok} keys={keys_ok} sentinel={sentinel_ok}")
    # 동질성 단언 회귀(plan_26063021_14_37 D3 — 5종 2등급: arch/gpu/count 하드 · cuda/driver major-하드·minor-경고).
    HL = {"cpu_arch": "aarch64", "gpu_model": "NVIDIA GB10", "gpus_per_node": 1, "cuda": "13.2", "driver": "565.57.01"}
    homo_cases = [
        ("동질 pass", HL, dict(HL), True),
        ("arch 불일치 → block", HL, {**HL, "cpu_arch": "x86_64"}, False),
        ("gpu_model 불일치 → block", HL, {**HL, "gpu_model": "NVIDIA RTX 5090"}, False),
        ("gpus_per_node 불일치 → block", HL, {**HL, "gpus_per_node": 2}, False),
        ("cuda minor drift → warn-pass", HL, {**HL, "cuda": "13.1"}, True),
        ("cuda major 불일치 → block", HL, {**HL, "cuda": "12.9"}, False),
        ("driver minor drift → warn-pass", HL, {**HL, "driver": "565.90.07"}, True),
        ("gpu_model 미상(None) → block(WARN-4)", HL, {**HL, "gpu_model": None}, False),
        ("peer 수집실패 → block", HL, {}, False),
    ]
    for name, loc, peer, want_verified in homo_cases:
        h = assert_homogeneity(loc, peer)
        ok = (h["verified"] == want_verified)
        if not want_verified:
            ok = ok and len(h["blocks"]) >= 1
        if "warn-pass" in name:
            ok = ok and not h["blocks"] and len(h["warns"]) >= 1
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] homo:{name}: verified={h['verified']} blocks={len(h['blocks'])} warns={len(h['warns'])}")
    # check_model_env 순수함수 회귀(§4.2 fail-closed 축 — managed/ephemeral/custom 3분기).
    cme_cases = [
        ("managed+NAS도달 → ok", ("managed", "online", True, None), True),
        ("managed+NAS미도달 → block", ("managed", "online", False, None), False),
        ("managed+NAS미측정(None) → ok(정보부족≠실패)", ("managed", "online", None, None), True),
        ("ephemeral+egress-restricted+토큰부재 → block", ("ephemeral", "restricted", None, False), False),
        ("ephemeral+egress-restricted+토큰존재 → ok", ("ephemeral", "restricted", None, True), True),
        ("ephemeral+egress-online+토큰부재 → ok(온라인이라 무관)", ("ephemeral", "online", None, False), True),
        ("custom+egress-restricted+토큰부재 → block", ("custom", "restricted", None, False), False),
    ]
    for name, args4, want_ok in cme_cases:
        r = check_model_env(*args4)
        ok = (r["ok"] == want_ok)
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] model_env:{name}: ok={r['ok']}" + ("" if ok else f"  ← 기대 {want_ok}"))
    # evaluate_gate egress/model-env 통합 회귀(§4.2 — egress 는 warn-only·model_env 는 block).
    gate_egress_cases = [
        ("egress-restricted+model-ok → pass(egress 는 attestation 뿐)",
         dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None,
              bw_floor=F, branch_topo="single", mani_topo="single",
              egress_self="online", egress_peer="restricted", model_env_ok=True),
         ("alpha", 0, True)),
        ("model-env-missing → block(fail-closed)",
         dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None,
              bw_floor=F, branch_topo="single", mani_topo="single",
              model_env_ok=False, model_env_reason="ephemeral: HF_TOKEN 부재 ∧ egress-restricted"),
         ("alpha", 2, True)),
        ("multi+model-env-missing → gamma block",
         dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.2,
              bw_floor=F, branch_topo="multi", mani_topo="multi", model_env_ok=False,
              model_env_reason="managed: NAS 미도달"),
         ("gamma", 2, True)),
    ]
    for name, kw, (eg, ec, econ) in gate_egress_cases:
        a, g, code = evaluate_gate(**kw)
        ok = (g["branch"] == eg and code == ec and a["consistent"] == econ)
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] gate-egress:{name}: gate={g['branch']} exit={code}"
              + ("" if ok else f"  ← 기대 ({eg},{ec},{econ})"))
    # cross_validate 소스 병합 회귀(2026-08-24 plan_26082415 결함 1 — S6 env-split 이후 compose
    # inline 이 비어 교차검증이 침묵 no-op 되던 결함). 소스 2개: compose inline + .env.interconnect.
    import tempfile
    cv_ic = {"hca_devices": ["roceP2p1s0f1", "rocep1s0f1"], "gid_index": 3,
             "socket_iface": "enp1s0f1np1"}
    with tempfile.TemporaryDirectory() as td:
        comp = os.path.join(td, "docker-compose.yaml")
        envf = os.path.join(td, ".env.interconnect")
        with open(comp, "w", encoding="utf-8") as fh:
            fh.write('services:\n  vllm:\n    environment:\n'
                     '      NCCL_IB_HCA: "=rocep1s0f1,roceP2p1s0f1"\n'
                     '      NCCL_SOCKET_IFNAME: enp1s0f1np1\n')
        with open(envf, "w", encoding="utf-8") as fh:
            fh.write("# rendered\nNCCL_IB_GID_INDEX=3\nNCCL_IB_HCA==rocep1s0f1,roceP2p1s0f1\n"
                     "NCCL_SOCKET_IFNAME=enp1s0f1np1\n")
        cv_cases = []
        # ① compose-inline-only(env 부재) — S6 이전 통로가 여전히 동작
        r = cross_validate(cv_ic, comp, os.path.join(td, "없음"))
        cv_cases.append(("compose-inline-only → checked+all_match",
                         r["checked"] and r["all_match"]
                         and r["sources"].get("NCCL_IB_HCA") == "compose"))
        # ② compose NCCL 부재 + env.interconnect 만 — S6 이후 통로(결함 재현 위치)
        with open(comp, "w", encoding="utf-8") as fh:
            fh.write("services:\n  vllm:\n    env_file:\n      - envs/.env.interconnect\n")
        r = cross_validate(cv_ic, comp, envf)
        cv_cases.append(("env.interconnect-only → checked+all_match(침묵 no-op 아님)",
                         r["checked"] and r["all_match"] and len(r["checks"]) == 3
                         and r["sources"].get("NCCL_IB_GID_INDEX") == "env.interconnect"))
        # ③ 양쪽 부재 → checked:false + fail-loud 어휘
        r = cross_validate(cv_ic, comp, os.path.join(td, "없음"))
        cv_cases.append(("양쪽 부재 → checked:false + 명시 reason",
                         (not r["checked"]) and "NCCL env 부재" in r.get("reason", "")))
        # ④ env 값 불일치 → all_match False(비교가 실제로 물린다)
        with open(envf, "w", encoding="utf-8") as fh:
            fh.write("NCCL_SOCKET_IFNAME=wrong0\n")
        r = cross_validate(cv_ic, comp, envf)
        cv_cases.append(("env 값 불일치 → all_match False",
                         r["checked"] and not r["all_match"]))
        for name, ok in cv_cases:
            passed += ok
            n += 1
            print(f"  [{'PASS' if ok else 'FAIL'}] cross-validate:{name}")
    print(f"self-test: {passed}/{n} {'PASS' if passed == n else 'FAIL'}")
    return 0 if passed == n else 1


def build_local_scan_result(ic: dict, compose: str, topology: str,
                            env_interconnect: str | None = None) -> dict:
    """Build the canonical local scan envelope consumed by staleness_gate.py."""
    return {
        "cpu_arch": scan_cpu_arch(),
        "cuda_version": scan_cuda_version(),
        "gpus_per_node": scan_gpus_per_node(),
        "gpu_model": scan_gpu_model(),
        # 메인 자신의 드라이버. 종전엔 collect_local_hw() 가 서브 **동질성 비교용**으로만 수집해
        # emit 경로가 없었고, 그 결과 single manifest 는 낡고 multi 는 driver_version:null 로 남았다.
        # emission_blockers 에는 넣지 않는다 — GPU-less 그라운딩 세션에서 발행을 막으면 안 되므로
        # 부재는 null 로 정직하게 흘린다(gpu_model 과 동일한 None-가드 계열).
        "driver_version": scan_driver_version(),
        "interconnect": {k: v for k, v in ic.items() if not k.startswith("_")},
        "scan_detail": {k: v for k, v in ic.items() if k.startswith("_")},
        "cross_validation": cross_validate(ic, compose, env_interconnect),
        "topology_declared": topology,
        "interconnect_present": ic["type"] != "generic-ethernet",
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="terraforming_node결정론 스캔 코어 (사실만)")
    ap.add_argument("--topology", choices=["single", "multi", "auto"], default="auto",
                    help="선언 토폴로지(인터뷰 결정). auto=스캔으로 추정(보고만, 게이트는 single/multi 명시 시)")
    ap.add_argument("--peer-ip", help="multi: 서브노드 IP(도달성 체크). 예: 203.0.113.11")
    ap.add_argument("--peer-port", type=int, default=22, help="도달성 체크 포트(기본 22=SSH)")
    ap.add_argument("--peer-ssh", help="서브 SSH 타겟(user@host) — 서브 HW 실측. "
                    "multi 는 동질성 하드 단언(D2/D3), single 은 관측 단언(GPU≥1)으로 hw_verified 를 낸다. "
                                       "미지정 시 동질성 미검증 → nodes[sub].hw_verified 미발급 → 서브 위임 키 미발급(fail-closed).")
    ap.add_argument("--compose", default="output/multi/docker-compose.yaml",
                    help="교차검증할 docker-compose 경로")
    ap.add_argument("--env-interconnect", default=None,
                    help="교차검증할 envs/.env.interconnect 경로(S6 env-split 제2 소스). "
                         "기본 = --compose 와 같은 디렉터리의 envs/.env.interconnect")
    ap.add_argument("--bandwidth-gbps", type=float, default=None,
                    help="cross-node ib_write_bw 합산 실측(Gb/s). 성능게이트 입력. 미지정 시 perf pending.")
    ap.add_argument("--per-port-gbps", type=float, default=None,
                    help="ib_write_bw 포트당 실측(Gb/s) — SKILL §1.4 합격선의 나머지 절반. 미지정 시 포트당 조건은 미평가(음성정직 표기).")
    ap.add_argument("--per-port-floor", type=float, default=100.0,
                    help="포트당 합격선(기본 100.0 — 200Gbps RoCE 플랫폼 파생). --bw-floor 와 함께 재설정한다.")
    ap.add_argument("--bw-floor", type=float, default=180.0,
                    help="성능게이트 합산 합격선(Gb/s). 기본 180(=200Gbps 풀대역폭의 ~90%%, devlog 218 기준).")
    ap.add_argument("--emit-manifest", action="store_true",
                    help="검증 통과 시 manifest topology+interconnect 블록을 stdout 끝에 출력. "
                         "--topology single|multi 명시 필수(auto면 fail-closed 거부 — plan_26063009_44_23 D3)")
    ap.add_argument("--manifest", default=None,
                    help="manifest 실값 경로(기본=브랜치 파생 output/<topology>/manifest.yaml). plan_26062315")
    ap.add_argument("--check-egress", action="store_true",
                    help="외부 egress 스캔(축 B — plan_26070809_46_57 §4.2). 로컬(메인) HF/게이트웨이 도달성 프로브 + "
                         "(--peer-ssh 동반 시) 서브 egress 실측 + manifest model_source 기반 모델 제반환경 판정.")
    ap.add_argument("--sub-work-dir", default=None,
                    help="multi: 서브 프로젝트 절대경로 → emit nodes[sub].work_dir. 미지정 시 emit 이 "
                         "__REQUIRED__ 센티넬을 남긴다(추측 금지).")
    ap.add_argument("--model-source", choices=["managed", "ephemeral", "custom"], default=None,
                    help="모델 제반환경 판정용 override(기본=manifest model_source 읽음).")
    ap.add_argument("--self-test", action="store_true",
                    help="결정론 회귀(게이트+emit_gate+emit-block YAML, 하드웨어 불요 — 라이브 검증 고정, #4)")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()

    # ── fail-closed: 토폴로지 인터뷰 게이트(plan_26063009_44_23 D3) — emit 전 명시 선언 필수 ──
    eg = emit_gate(args.emit_manifest, args.topology)
    if eg:
        print("[scan] FAIL: --emit-manifest 에는 --topology single|multi 명시 필요 — "
              "토폴로지는 terraforming_node 진입의 인터뷰로 선언한다(브랜치/스캔 추론으로 manifest 기입 금지).",
              file=sys.stderr)
        return eg

    ic = detect_interconnect()
    if args.bandwidth_gbps is not None:
        ic["bandwidth_gbps"] = args.bandwidth_gbps   # cross-node ib_write_bw 실측 주입
    result = build_local_scan_result(ic, args.compose, args.topology,
                                     args.env_interconnect
                                     or default_env_interconnect_path(args.compose))
    # 2026-09-03(B2 · plan_26090317 P1): nodes[] 가 {role,host} 2필드뿐이라 하류 소비자 4곳이
    #   전부 막혔다(render NAS/ssh_user/work_dir 필수 · sync 주소해소 · A2A 위임 키). 스캔이 알 수 있는
    #   것은 채우고(호스트명·ssh 계정·메인 work_dir), 모르는 것은 emit 이 센티넬로 남긴다.
    _main_node = {
        "role": "main",
        "host": platform.node() or "localhost",
        "hostname": platform.node() or None,
        "ssh_user": getpass.getuser(),
        "work_dir": os.getcwd(),
    }
    if args.topology == "multi" and args.peer_ip:
        _sub = {"role": "sub", "host": args.peer_ip}
        if args.peer_ssh and "@" in args.peer_ssh:
            _sub["ssh_user"], _, _sub["hostname"] = args.peer_ssh.partition("@")
        elif args.peer_ssh:
            _sub["hostname"] = args.peer_ssh
        if args.sub_work_dir:
            _sub["work_dir"] = args.sub_work_dir
        result["nodes"] = [_main_node, _sub]
    elif args.topology == "single":
        # ⑬: 이전에는 single 이 `nodes: []` 를 하드코딩해 등록된 main 항목을 지웠다.
        #    §2.7.0 은 single+sub(A2A) 조합을 명시적으로 허용하므로 통째로 비우면 안 된다.
        result["nodes"] = [_main_node]
        # single 서브 등록: `--peer-ssh` 가 주어졌다는 것이 곧 "이 서브를 관측 대상으로 삼는다" 는
        #   선언이다(무단 프로빙 금지 원칙상 이 플래그 없이는 서브를 만지지 않는다).
        if args.peer_ssh:
            _u, _, _h = args.peer_ssh.partition("@")
            _sub = {"role": "sub", "host": args.peer_ip or _h or args.peer_ssh,
                    "hostname": _h or None, "ssh_user": _u or None}
            if args.sub_work_dir:
                _sub["work_dir"] = args.sub_work_dir
            result["nodes"].append(_sub)
    if args.peer_ip:
        result["peer_check"] = {"ip": args.peer_ip, "port": args.peer_port,
                                "reachable": peer_reachable(args.peer_ip, args.peer_port)}

    # ── 3자-일치 단언 + α/γ 게이트 (결정론 — evaluate_gate 순수함수, --self-test 회귀) ──
    manifest_read_error = None
    branch = git_branch()
    branch_topo = {"single-node": "single", "multi-node": "multi"}.get(branch or "")
    # manifest 실값은 브랜치 파생 통로 output/<topology>/manifest.yaml (plan_26062315).
    mani_path = args.manifest or (f"output/{branch_topo}/manifest.yaml" if branch_topo else "manifest.yaml")
    try:
        mani_topo = read_manifest_topology(mani_path)
        manifest_read_error = None
    except ManifestUnreadable as exc:
        mani_topo, manifest_read_error = None, str(exc)
    peer_reach = result.get("peer_check", {}).get("reachable")

    # ── 외부 egress 스캔 (축 B — plan_26070809_46_57 §4.2, 인터커넥트 축 A 와 병렬·독립) ──
    egress_self = egress_peer = None
    model_env_result = None
    # 모델 획득 축은 egress 스캔과 독립이다 — --check-egress 없이도 인터뷰 확정값은 emit 에 실린다.
    if args.model_source:
        result.setdefault("model_source", args.model_source)
    try:
        _nas = read_manifest_field(mani_path, "nas_model_path")
    except ManifestUnreadable as exc:
        _nas, manifest_read_error = None, str(exc)
    if _nas:
        result.setdefault("nas_model_path", _nas)

    if args.check_egress:
        es = scan_egress()
        egress_self = es["egress"]
        result["egress"] = {"self": es}
        if args.peer_ssh and args.topology in ("single", "multi"):
            ep = collect_peer_egress(args.peer_ssh)
            egress_peer = ep["egress"]
            result["egress"]["peer"] = ep
        try:
            model_source = args.model_source or read_manifest_field(mani_path, "model_source")
        except ManifestUnreadable as exc:
            model_source, manifest_read_error = args.model_source, str(exc)
        if model_source:
            result["model_source"] = model_source
            nas_path = result.get("nas_model_path")
            if nas_path:
                result["nas_model_path"] = nas_path
            nas_reachable = os.path.isdir(nas_path) if (model_source == "managed" and nas_path) else None
            nas_source = "measured:main-fs"
            try:
                hf_token_env_file = read_manifest_field(mani_path, "hf_token_env_file")
            except ManifestUnreadable:
                hf_token_env_file = None
            hf_token_present = (bool(hf_token_env_file) and os.path.isfile(hf_token_env_file)) \
                if model_source in ("ephemeral", "custom") else None
            token_source = "measured:main-fs"
            # 2026-09-03(S6 · plan_26090317 P1): 서브가 등록된 multi 에서는 **서브의** 사실이 서브 판정의
            #   권위다. 이전에는 nas_reachable/hf_token_present 를 **메인 파일시스템**에서 판정하고 그 값을
            #   서브 판정에 썼다 — docstring 은 "서브 모델 NAS 경로 도달 불가" 라고 말하는데 서브를 보지
            #   않았다. 메인에만 NAS 가 있으면 게이트는 통과하고 **서빙에서** 죽는다(가장 비싼 자리).
            if args.peer_ssh and args.topology in ("single", "multi"):
                peer_env = collect_peer_model_env(args.peer_ssh, nas_path, hf_token_env_file)
                result["model_env_peer"] = peer_env
                if peer_env["probed"]:
                    if model_source == "managed" and peer_env["nas_reachable"] is not None:
                        nas_reachable, nas_source = peer_env["nas_reachable"], "measured:peer-ssh"
                    if model_source in ("ephemeral", "custom") and peer_env["hf_token_present"] is not None:
                        hf_token_present, token_source = peer_env["hf_token_present"], "measured:peer-ssh"
                egress_for_model = egress_peer or egress_self
            else:
                egress_for_model = egress_self
            model_env_result = check_model_env(model_source, egress_for_model, nas_reachable, hf_token_present)
            model_env_result["nas_source"] = nas_source
            model_env_result["hf_token_source"] = token_source
            model_env_result["egress_source"] = ("peer" if (args.peer_ssh and egress_peer) else "self")
            result["model_env"] = model_env_result


    if manifest_read_error:
        print(f"[scan] FAIL(F10): manifest 가 존재하나 판독 불가 — {manifest_read_error}", file=sys.stderr)
        print("[scan]        판독 불가를 '부재' 로 접으면 3자일치 게이트가 통째로 skip 된다(무증거 통과).",
              file=sys.stderr)
    assertion, gate, exit_code = evaluate_gate(
        declared=args.topology, ic_present=result["interconnect_present"],
        peer_given=bool(args.peer_ip), peer_reachable=peer_reach,
        bandwidth=ic["bandwidth_gbps"], bw_floor=args.bw_floor,
        per_port_gbps=args.per_port_gbps, per_port_floor=args.per_port_floor,
        branch=branch, branch_topo=branch_topo, mani_topo=mani_topo,
        mani_path=mani_path, peer_ip=args.peer_ip,
        egress_self=egress_self, egress_peer=egress_peer,
        model_env_ok=(model_env_result["ok"] if model_env_result else None),
        model_env_reason=(model_env_result["reason"] if model_env_result else None),
        manifest_read_error=manifest_read_error)
    result["consistency_assertion"] = assertion
    result["gate"] = gate

    # ── 서브 HW 동질성 단언 (multi · --peer-ssh — plan_26063021_14_37 D2/D3) ──
    # 통과 시 nodes[role=sub].hw_verified 발급(서브 위임 키의 전제) · 하드 블록 시 게이트 blocked(fail-closed).
    # 2026-09-03(P3 · plan_26090317): `--peer-ssh` 가 **multi 전용**이라 single+sub 조합
    #   (§2.7.0 이 명시적으로 허용하는 A2A 에이전트 구성)은 `hw_verified` 를 얻을 경로가 아예
    #   없었다 → A2A 위임 키 영구 미발급 → 서브가 영구 info-only. 게이트가 아니라 **배선 공백**이다.
    #
    #   두 토폴로지에서 이 단언의 **의미가 다르다**:
    #     · multi  = 집단 연산(NCCL/Ray)이 ABI·GPU 동질성을 요구한다 → 불일치는 **하드 블록**.
    #     · single = 집단 연산이 없다. 서브는 독립 서빙하는 A2A 에이전트이므로 동질성은 **정보**이고,
    #                검증해야 할 것은 "메인이 이 서브를 실제로 관측했고 서빙 가능한 노드다" 이다.
    #                따라서 술어 = 서브 HW 수집 성공 ∧ GPU ≥ 1. 불일치는 warns 로 남긴다(침묵 ✗).
    if args.peer_ssh and args.topology in ("single", "multi"):
        local_hw = collect_local_hw()
        peer_hw = collect_peer_hw(args.peer_ssh)
        homo = assert_homogeneity(local_hw, peer_hw)
        if args.topology == "single":
            _peer_gpus = peer_hw.get("gpus_per_node")
            _observed = bool(peer_hw) and isinstance(_peer_gpus, int) and _peer_gpus >= 1
            homo = {
                "verified": _observed,
                "blocks": [] if _observed else
                          ["서브 HW 수집 실패 또는 GPU 0 — 관측되지 않은 노드에 위임하지 않는다(fail-closed)"],
                # 동질성 불일치는 single 에서 블록 사유가 아니다. 사라지게 두지도 않는다.
                "warns": list(homo.get("warns") or []) + [
                    "single 동질성 정보(블록 아님): " + b for b in (homo.get("blocks") or [])],
                "detail": homo.get("detail") or {},
                "verified_rule": "single:peer-observed(gpus>=1)",
            }
        else:
            homo["verified_rule"] = "multi:homogeneity-hard"
        result["homogeneity"] = {"local": local_hw, "peer": peer_hw, **homo}
        if homo["blocks"]:
            result["gate"]["status"] = "blocked"
            result["gate"].setdefault("reasons", []).extend(homo["blocks"])
            result["gate"]["note"] = ("서브 HW 동질성 미충족(fail-closed) — 표기 드리프트(동일 GPU 다른 문자열) 등 "
                                      "오탐 판단 시 HITL 우회 = manifest nodes[sub].hw_verified 수동 기입이 권위"
                                      "(plan_26063021_14_37 D3) — ") + result["gate"].get("note", "")
            exit_code = 2

    if args.emit_manifest and result["gate"]["status"] == "ok":
        incomplete = emission_blockers(result)
        if incomplete:
            result["gate"]["status"] = "blocked"
            result["gate"].setdefault("reasons", []).append(
                "완수 manifest 발행 금지: scan facts 불완전(%s)" % ",".join(incomplete))
            exit_code = 2

    json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    if args.emit_manifest and result["gate"]["status"] == "ok" and args.topology in ("single", "multi"):
        sys.stdout.write("\n# ===== manifest 기입 블록 (검증 통과 — 사람 확인 후 manifest.yaml 반영) =====\n")
        sys.stdout.write(emit_manifest_block(result))
        # 2026-09-03(⑬): 동질성 결과는 위 emit 블록의 nodes[sub] 안에 이미 들어간다.
        #   "이 블록으로 덮어써라" 와 "sub 항목에 추가하라" 가 한 출력에 공존하던 모호함을 없앴다.
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
