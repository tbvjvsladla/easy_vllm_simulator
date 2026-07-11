#!/usr/bin/env python3
"""scan_node.py — terraforming_node진입 루틴의 **결정론 스캔 코어** (판단 X, 사실만).

근거: docs/plan/plan_2026062311_1(진입 루틴 §2.3 스캔 인벤토리 · §2.4 성능게이트 · §2.5 α/γ 종료),
      plan_2026062312_1(output/ 통로), CLAUDE.md(결정론 스크립트 원칙 · 산출물 통로 불변식).

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
import json
import os
import platform
import re
import shutil
import subprocess
import sys

IPV4_RE = re.compile(r"^\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}$")


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
    gid_idx = sorted({r["gid_index"] for r in v2})
    return {
        "type": "RoCE v2",
        "hca_devices": hca_devices,
        # GID 인덱스가 행마다 동일하면 단일값, 아니면 리스트(가드)
        "gid_index": gid_idx[0] if len(gid_idx) == 1 else gid_idx,
        # bootstrap iface = 최저 IPv4(Domain 0) 의 iface → enp1s0f1np1 (compose 와 정합)
        "socket_iface": v2_sorted[0]["iface"],
        "bandwidth_gbps": None,  # ib_write_bw 측정 전까지 null
        "platform_preset": None,  # 사람/플랫폼 탐지가 채움(예: dgx-spark-gb10)
        "_sysfs_hcas": hcas_all,
        "_roce_v2_gids": v2,
    }


def _parse_compose_nccl(path: str) -> dict:
    """docker-compose 에서 NCCL_IB_HCA/GID_INDEX/SOCKET_IFNAME 추출(교차검증용, flat grep)."""
    res: dict = {}
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                s = ln.strip()
                for key in ("NCCL_IB_HCA", "NCCL_IB_GID_INDEX", "NCCL_SOCKET_IFNAME"):
                    if s.startswith(key + ":"):
                        val = s.split(":", 1)[1].strip().strip('"').strip()
                        # 인라인 주석 제거
                        val = val.split("#", 1)[0].strip().strip('"')
                        res[key] = val
    except OSError:
        pass
    return res


def cross_validate(ic: dict, compose_path: str) -> dict:
    """스캔 interconnect ↔ 현 docker-compose NCCL 값 일치 확인."""
    nccl = _parse_compose_nccl(compose_path)
    if not nccl:
        return {"checked": False, "reason": f"compose 없음/NCCL 부재: {compose_path}"}
    checks = []
    # HCA: compose 는 "=rocep1s0f1,roceP2p1s0f1" 형식 → '=' 와 분해
    if "NCCL_IB_HCA" in nccl:
        comp_hcas = sorted(x for x in nccl["NCCL_IB_HCA"].lstrip("=").split(",") if x)
        checks.append({"field": "hca_devices", "scanned": ic["hca_devices"],
                       "compose": comp_hcas, "match": sorted(ic["hca_devices"]) == comp_hcas})
    if "NCCL_IB_GID_INDEX" in nccl:
        comp_gid = nccl["NCCL_IB_GID_INDEX"]
        checks.append({"field": "gid_index", "scanned": str(ic["gid_index"]),
                       "compose": comp_gid, "match": str(ic["gid_index"]) == comp_gid})
    if "NCCL_SOCKET_IFNAME" in nccl:
        checks.append({"field": "socket_iface", "scanned": ic["socket_iface"],
                       "compose": nccl["NCCL_SOCKET_IFNAME"],
                       "match": ic["socket_iface"] == nccl["NCCL_SOCKET_IFNAME"]})
    return {"checked": True, "all_match": all(c["match"] for c in checks), "checks": checks}


def peer_reachable(ip: str, port: int = 22, timeout: int = 5) -> bool:
    """peer 도달성(TCP connect — ib_write_bw 아님, 그건 별도 cross-node 오케스트레이션)."""
    import socket
    try:
        with socket.create_connection((ip, port), timeout=timeout):
            return True
    except OSError:
        return False


# ── 외부 egress 스캔 (축 B — plan_2026070809_2 §4.2 · 인터커넥트 축 A 와 직교) ──
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
    except Exception:
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
    """manifest.yaml 의 top-level 'key: value' 플랫 grep(pyyaml 비의존 — read_manifest_topology 와 동형)."""
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                s = ln.strip()
                if s.startswith(key + ":"):
                    return s.split(":", 1)[1].split("#", 1)[0].strip().strip('"')
    except OSError:
        pass
    return None


# ── 서브 HW 수집 + 메인↔서브 동질성 단언 (plan_2026063021_2 · D2/D3) ──
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
    stdin 투입은 인용 함정·PATH 누락 양쪽을 회피한다(라이브 E2E 발견 — plan_2026063021_2)."""
    opts = ssh_opts or ["-o", "BatchMode=yes", "-o", "ConnectTimeout=8"]
    text = ""
    try:
        out = subprocess.run(["ssh", *opts, ssh_target, "bash", "-ls"],
                             input=_PEER_HW_PROBE, capture_output=True, text=True, timeout=25)
        text = out.stdout if out.returncode == 0 else ""
    except Exception:
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


def read_manifest_topology(path: str = "manifest.yaml") -> str | None:
    """manifest.yaml 의 topology 값(flat grep — pyyaml 비의존)."""
    try:
        with open(path, encoding="utf-8") as f:
            for ln in f:
                s = ln.strip()
                if s.startswith("topology:"):
                    return s.split(":", 1)[1].split("#", 1)[0].strip().strip('"')
    except OSError:
        pass
    return None


def emit_manifest_block(result: dict) -> str:
    """검증된 스캔 결과 → manifest.yaml 에 기입할 topology+interconnect YAML 블록(문자열)."""
    ic = result["interconnect"]
    hcas = "[" + ", ".join(ic["hca_devices"]) + "]"
    topo = "multi" if result["topology_declared"] == "multi" else "single"
    # None-가드(plan_2026063009_1): 프로브 실패(예: nvidia-smi PATH 미노출 — 그라운딩 세션 시나리오)로
    # None 인 필드는 YAML null 로 emit. bare 'None' 문자열 누수 차단 — Bug2 클래스 형제필드(cuda_version·gpus_per_node) 포함.
    cuda = f'"{result["cuda_version"]}"' if result["cuda_version"] is not None else "null"
    gpus = result["gpus_per_node"] if result["gpus_per_node"] is not None else "null"
    import datetime
    scanned_at = datetime.datetime.now().strftime("%Y%m%d%H")
    # 테라포밍 완수 Flag (attestation · plan_2026063018_1 · 헌법 §테라포밍-완수 Flag 게이트):
    # 이 emit 는 evaluate_gate status==ok(§1.5 3자일치 + α/γ 통과) 시에만 호출되므로 complete:true·branch_verified:true 기입.
    # 보수적 — 게이트 미통과면 emit 자체가 안 됨(미발급). model_source(인터뷰) 는 별도 — manifest_contract 가 함께 요구.
    lines = [
        "# 테라포밍 완수 Flag (attestation · plan_2026063018_1) — scan §1.5 3자일치 통과 시 기입(보수적).",
        "terraforming:",
        "  complete: true",
        "  branch_verified: true   # git 브랜치 ↔ topology ↔ scan 3자일치 단언 통과",
        f"  scanned_at: \"{scanned_at}\"",
        f"topology: {topo}",
        f"cpu_arch: \"{result['cpu_arch']}\"",
        f"cuda_version: {cuda}",
        f"gpus_per_node: {gpus}",
    ]
    if topo == "single":
        # single dormant 게이트를 결정론으로 동결(sub-control 확장기능 비활성 — 헌법 §single-node 확장기능).
        lines.append("nodes: []  # 단일노드 dormant: sub-control 확장기능 비활성")
    lines += [
        "interconnect:",
        f"  type: {ic['type']}",
        f"  hca_devices: {hcas}",
        f"  gid_index: {ic['gid_index'] if ic['gid_index'] is not None else 'null'}",
        f"  socket_iface: {ic['socket_iface'] if ic['socket_iface'] is not None else 'null'}",
        f"  bandwidth_gbps: {ic['bandwidth_gbps'] if ic['bandwidth_gbps'] is not None else 'null'}",
        f"  platform_preset: {ic['platform_preset'] or 'null  # set: e.g. dgx-spark-gb10'}",
    ]
    lines += [
        "# ⚠ 인터뷰 확정 필드(이 scan 블록엔 없음 — 별도 추가): model_source(managed|ephemeral|custom)·nas_model_path.",
        "#   manifest_contract 게이트가 valid model_source 를 요구 — 미설정 시 info-only 유지(Flag complete 만으론 불충분).",
        "# ⚠ host_safety.installed(true|false): terraforming 세션 최종 Y/N 답변 후 기입(scan 무증거 기입 ✗ · Flag 와 독립 —",
        "#   안전체계 미설치여도 Flag valid. 헌법 §호스트 안전체계 따름정리 선택화 · plan_2026071115_1).",
    ]
    return "\n".join(lines) + "\n"


def evaluate_gate(*, declared, ic_present, peer_given, peer_reachable, bandwidth, bw_floor,
                  branch=None, branch_topo=None, mani_topo=None, mani_path="manifest.yaml",
                  peer_ip=None, egress_self=None, egress_peer=None,
                  model_env_ok=None, model_env_reason=None):
    """결정론 게이트 판정 — **순수 함수**(I/O 없음 → --self-test 회귀 대상). 반환 (assertion, gate, exit_code).
    3자-일치 단언(branch↔manifest↔scan) + α/γ(구조)/γ(성능 fail-closed)/multi-ready 판정을 한 곳에 codify.
    egress_self/egress_peer: attestation 뿐(불일치=경고·fail-open — 서브 능력차는 정상, plan_2026070809_2 §4.2).
    model_env_ok=False: **blocking**(모델 제반환경 불충족 — 서브=에어갭 전역상수를 대체하는 유일한 fail-closed 축)."""
    mism: list[str] = []     # blocking(혼재/위험)
    warns: list[str] = []    # 비blocking(정보)
    if declared in ("single", "multi"):
        if branch_topo and branch_topo != declared:
            mism.append(f"declared={declared} ≠ git branch({branch})⇒{branch_topo}")
        if declared == "multi" and not ic_present:
            mism.append("declared=multi 인데 스캔: RoCE v2 미탐지")
        if declared == "single" and ic_present:
            warns.append("single 선언 + RoCE 하드웨어 존재 — 멀티 가능 머신의 단일노드 운용(정상·정보)")
        if mani_topo and branch_topo and mani_topo != branch_topo:
            mism.append(f"manifest({mani_path}) topology={mani_topo} ≠ branch⇒{branch_topo}")
    if egress_self and egress_peer and egress_self != egress_peer:
        warns.append(f"egress 능력차(정보·fail-open): main={egress_self} sub={egress_peer} — 서브는 자동 격하(루프라인-only)")
    model_block = (model_env_reason or "모델 제반환경 불충족(fail-closed)") if model_env_ok is False else None
    assertion = {
        "git_branch": branch, "branch_implies": branch_topo,
        "manifest_path": mani_path, "manifest_topology": mani_topo, "declared": declared,
        "scan_interconnect_present": ic_present,
        "egress_self": egress_self, "egress_peer": egress_peer,
        "consistent": not mism, "mismatches": mism, "warnings": warns,
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
        if peer_given and not peer_reachable:
            struct_block.append(f"peer {peer_ip} 미도달")
        if model_block:
            struct_block.append(model_block)
        struct_block += mism
        if struct_block:                                   # γ: 구조/일관성 미충족
            gate = {"branch": "gamma", "status": "blocked", "reasons": struct_block,
                    "note": "멀티-ready manifest 미생성. 구조/일관성 확보 후 재실행."}
            exit_code = 2
        elif bandwidth is None:                            # 구조 충족, 성능 측정 대기
            pend = [] if peer_given else ["peer-ip 미지정(도달성 미검증)"]
            gate = {"branch": "multi-ready-candidate", "status": "pending-perf", "pending": pend,
                    "note": f"구조 충족. ib_write_bw 합산 ≥{bw_floor}Gb/s 측정(--bandwidth-gbps) 후 ready."}
        elif bandwidth < bw_floor:                         # γ: 성능 미달(fail-closed)
            gate = {"branch": "gamma", "status": "blocked",
                    "reasons": [f"대역폭 {bandwidth}Gb/s < 합격선 {bw_floor}Gb/s"],
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
    순수함수 → --self-test 회귀. 근거: plan_2026063009_1 D3(토폴로지 인터뷰 fail-closed 게이트)."""
    return 3 if (emit_manifest and topology == "auto") else 0


def _self_test() -> int:
    """결정론 회귀(게이트 9분기 + emit_gate 4 + emit-block None-leak 2 = 15) — 실 2노드 라이브 검증(testlog_2026062314_1) fixture 고정(#4). 하드웨어 불요."""
    F = 180.0
    cases = [
        # name, kwargs, (expect_gate, expect_exit, expect_consistent)
        ("multi-ready(구조+성능)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("multi-ready", 0, True)),
        ("multi γ(성능 fail-closed)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=100.0, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("gamma", 2, True)),
        ("multi γ(peer 미도달)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=False, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi", peer_ip="x"), ("gamma", 2, True)),
        ("multi γ(no RoCE)", dict(declared="multi", ic_present=False, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("gamma", 2, False)),
        ("multi pending-perf", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=None, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("multi-ready-candidate", 0, True)),
        ("single α", dict(declared="single", ic_present=False, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="single", mani_topo="single"), ("alpha", 0, True)),
        ("single α + RoCE(경고 비blocking)", dict(declared="single", ic_present=True, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="single", mani_topo="single"), ("alpha", 0, True)),
        ("single on multi-branch(혼재차단)", dict(declared="single", ic_present=True, peer_given=False, peer_reachable=None, bandwidth=None, bw_floor=F, branch_topo="multi", mani_topo="multi"), ("alpha", 2, False)),
        ("manifest≠branch(혼재차단)", dict(declared="multi", ic_present=True, peer_given=True, peer_reachable=True, bandwidth=208.2, bw_floor=F, branch_topo="multi", mani_topo="single"), ("gamma", 2, False)),
    ]
    passed = 0
    for name, kw, (eg, ec, econ) in cases:
        a, g, code = evaluate_gate(**kw)
        ok = (g["branch"] == eg and code == ec and a["consistent"] == econ)
        passed += ok
        print(f"  [{'PASS' if ok else 'FAIL'}] {name}: gate={g['branch']} exit={code} consistent={a['consistent']}"
              + ("" if ok else f"  ← 기대 ({eg},{ec},{econ})"))
    n = len(cases)
    # emit_gate fail-closed 회귀(plan_2026063009_1 D3): --emit-manifest 는 명시 토폴로지 필요.
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
    # emit_manifest_block None-leak 회귀(plan_2026063009_1 — Bug2 클래스: gid/iface/preset/gpus/cuda None→null).
    # stdlib만 사용(yaml 비의존): bare 'None' 누수 0 + single 시 nodes:[] 동결 + 필수 키 존재로 검증.
    emit_cases = [
        ("single GPU-less(전필드 None)", {"topology_declared": "single", "cpu_arch": "x86_64", "cuda_version": None,
            "gpus_per_node": None, "interconnect": {"type": "generic-ethernet", "hca_devices": [], "gid_index": None,
            "socket_iface": None, "bandwidth_gbps": None, "platform_preset": None}}, True),
        ("multi RoCE(채워짐)", {"topology_declared": "multi", "cpu_arch": "aarch64", "cuda_version": "132",
            "gpus_per_node": 1, "interconnect": {"type": "roce-v2", "hca_devices": ["mlx5_0"], "gid_index": 3,
            "socket_iface": "enp1s0f0", "bandwidth_gbps": 208.2, "platform_preset": "dgx-spark-gb10"}}, False),
    ]
    for name, res, want_single in emit_cases:
        blk = emit_manifest_block(res)
        no_none = "None" not in blk                                  # bare Python None 누수 0
        nodes_ok = ("nodes: []" in blk) if want_single else ("nodes:" not in blk)
        attest_ok = ("terraforming:" in blk) and ("complete: true" in blk) and ("branch_verified: true" in blk)
        ok = no_none and nodes_ok and "topology:" in blk and attest_ok   # Flag attestation 기입 검증(plan_2026063018_1)
        passed += ok
        n += 1
        print(f"  [{'PASS' if ok else 'FAIL'}] emit:{name}: no-None={no_none} nodes-gate={nodes_ok} attest={attest_ok}")
    # 동질성 단언 회귀(plan_2026063021_2 D3 — 5종 2등급: arch/gpu/count 하드 · cuda/driver major-하드·minor-경고).
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
    print(f"self-test: {passed}/{n} {'PASS' if passed == n else 'FAIL'}")
    return 0 if passed == n else 1


def main() -> int:
    ap = argparse.ArgumentParser(description="terraforming_node결정론 스캔 코어 (사실만)")
    ap.add_argument("--topology", choices=["single", "multi", "auto"], default="auto",
                    help="선언 토폴로지(인터뷰 결정). auto=스캔으로 추정(보고만, 게이트는 single/multi 명시 시)")
    ap.add_argument("--peer-ip", help="multi: 서브노드 IP(도달성 체크). 예: 203.0.113.11")
    ap.add_argument("--peer-port", type=int, default=22, help="도달성 체크 포트(기본 22=SSH)")
    ap.add_argument("--peer-ssh", help="multi: 서브 SSH 타겟(user@host) — 서브 HW 실측 + 메인↔서브 동질성 단언(D2/D3). "
                                       "미지정 시 동질성 미검증 → nodes[sub].hw_verified 미발급 → 서브 위임 키 미발급(fail-closed).")
    ap.add_argument("--compose", default="output/multi/docker-compose.yaml",
                    help="교차검증할 docker-compose 경로")
    ap.add_argument("--bandwidth-gbps", type=float, default=None,
                    help="cross-node ib_write_bw 합산 실측(Gb/s). 성능게이트 입력. 미지정 시 perf pending.")
    ap.add_argument("--bw-floor", type=float, default=180.0,
                    help="성능게이트 합산 합격선(Gb/s). 기본 180(=200Gbps 풀대역폭의 ~90%%, devlog 218 기준).")
    ap.add_argument("--emit-manifest", action="store_true",
                    help="검증 통과 시 manifest topology+interconnect 블록을 stdout 끝에 출력. "
                         "--topology single|multi 명시 필수(auto면 fail-closed 거부 — plan_2026063009_1 D3)")
    ap.add_argument("--manifest", default=None,
                    help="manifest 실값 경로(기본=브랜치 파생 output/<topology>/manifest.yaml). plan_2026062315_1")
    ap.add_argument("--check-egress", action="store_true",
                    help="외부 egress 스캔(축 B — plan_2026070809_2 §4.2). 로컬(메인) HF/게이트웨이 도달성 프로브 + "
                         "(--peer-ssh 동반 시) 서브 egress 실측 + manifest model_source 기반 모델 제반환경 판정.")
    ap.add_argument("--model-source", choices=["managed", "ephemeral", "custom"], default=None,
                    help="모델 제반환경 판정용 override(기본=manifest model_source 읽음).")
    ap.add_argument("--self-test", action="store_true",
                    help="결정론 회귀(게이트+emit_gate+emit-block YAML, 하드웨어 불요 — 라이브 검증 고정, #4)")
    args = ap.parse_args()

    if args.self_test:
        return _self_test()

    # ── fail-closed: 토폴로지 인터뷰 게이트(plan_2026063009_1 D3) — emit 전 명시 선언 필수 ──
    eg = emit_gate(args.emit_manifest, args.topology)
    if eg:
        print("[scan] FAIL: --emit-manifest 에는 --topology single|multi 명시 필요 — "
              "토폴로지는 terraforming_node 진입의 인터뷰로 선언한다(브랜치/스캔 추론으로 manifest 기입 금지).",
              file=sys.stderr)
        return eg

    ic = detect_interconnect()
    if args.bandwidth_gbps is not None:
        ic["bandwidth_gbps"] = args.bandwidth_gbps   # cross-node ib_write_bw 실측 주입
    result = {
        "cpu_arch": scan_cpu_arch(),
        "cuda_version": scan_cuda_version(),
        "gpus_per_node": scan_gpus_per_node(),
        "interconnect": {k: v for k, v in ic.items() if not k.startswith("_")},
        "scan_detail": {k: v for k, v in ic.items() if k.startswith("_")},
        "cross_validation": cross_validate(ic, args.compose),
        "topology_declared": args.topology,
        "interconnect_present": ic["type"] != "generic-ethernet",
    }
    if args.peer_ip:
        result["peer_check"] = {"ip": args.peer_ip, "port": args.peer_port,
                                "reachable": peer_reachable(args.peer_ip, args.peer_port)}

    # ── 3자-일치 단언 + α/γ 게이트 (결정론 — evaluate_gate 순수함수, --self-test 회귀) ──
    branch = git_branch()
    branch_topo = {"single-node": "single", "multi-node": "multi"}.get(branch or "")
    # manifest 실값은 브랜치 파생 통로 output/<topology>/manifest.yaml (plan_2026062315_1).
    mani_path = args.manifest or (f"output/{branch_topo}/manifest.yaml" if branch_topo else "manifest.yaml")
    mani_topo = read_manifest_topology(mani_path)
    peer_reach = result.get("peer_check", {}).get("reachable")

    # ── 외부 egress 스캔 (축 B — plan_2026070809_2 §4.2, 인터커넥트 축 A 와 병렬·독립) ──
    egress_self = egress_peer = None
    model_env_result = None
    if args.check_egress:
        es = scan_egress()
        egress_self = es["egress"]
        result["egress"] = {"self": es}
        if args.peer_ssh and args.topology == "multi":
            ep = collect_peer_egress(args.peer_ssh)
            egress_peer = ep["egress"]
            result["egress"]["peer"] = ep
        model_source = args.model_source or read_manifest_field(mani_path, "model_source")
        if model_source:
            nas_path = read_manifest_field(mani_path, "nas_model_path")
            nas_reachable = os.path.isdir(nas_path) if (model_source == "managed" and nas_path) else None
            hf_token_env_file = read_manifest_field(mani_path, "hf_token_env_file")
            hf_token_present = (bool(hf_token_env_file) and os.path.isfile(hf_token_env_file)) \
                if model_source in ("ephemeral", "custom") else None
            model_env_result = check_model_env(model_source, egress_self, nas_reachable, hf_token_present)
            result["model_env"] = model_env_result

    assertion, gate, exit_code = evaluate_gate(
        declared=args.topology, ic_present=result["interconnect_present"],
        peer_given=bool(args.peer_ip), peer_reachable=peer_reach,
        bandwidth=ic["bandwidth_gbps"], bw_floor=args.bw_floor,
        branch=branch, branch_topo=branch_topo, mani_topo=mani_topo,
        mani_path=mani_path, peer_ip=args.peer_ip,
        egress_self=egress_self, egress_peer=egress_peer,
        model_env_ok=(model_env_result["ok"] if model_env_result else None),
        model_env_reason=(model_env_result["reason"] if model_env_result else None))
    result["consistency_assertion"] = assertion
    result["gate"] = gate

    # ── 서브 HW 동질성 단언 (multi · --peer-ssh — plan_2026063021_2 D2/D3) ──
    # 통과 시 nodes[role=sub].hw_verified 발급(서브 위임 키의 전제) · 하드 블록 시 게이트 blocked(fail-closed).
    if args.peer_ssh and args.topology == "multi":
        local_hw = collect_local_hw()
        peer_hw = collect_peer_hw(args.peer_ssh)
        homo = assert_homogeneity(local_hw, peer_hw)
        result["homogeneity"] = {"local": local_hw, "peer": peer_hw, **homo}
        if homo["blocks"]:
            result["gate"]["status"] = "blocked"
            result["gate"].setdefault("reasons", []).extend(homo["blocks"])
            result["gate"]["note"] = ("서브 HW 동질성 미충족(fail-closed) — 표기 드리프트(동일 GPU 다른 문자열) 등 "
                                      "오탐 판단 시 HITL 우회 = manifest nodes[sub].hw_verified 수동 기입이 권위"
                                      "(plan_2026063021_2 D3) — ") + result["gate"].get("note", "")
            exit_code = 2

    json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    if args.emit_manifest and result["gate"]["status"] == "ok" and args.topology in ("single", "multi"):
        sys.stdout.write("\n# ===== manifest 기입 블록 (검증 통과 — 사람 확인 후 manifest.yaml 반영) =====\n")
        sys.stdout.write(emit_manifest_block(result))
        if result.get("homogeneity", {}).get("verified"):
            ph = result["homogeneity"]["peer"]
            sys.stdout.write(
                "\n# ===== nodes[role=sub] 병합 (서브 HW 동질성 검증 통과 — HITL · plan_2026063021_2 D3) =====\n"
                "#   nodes 의 '- role: sub' 항목에 아래를 추가 → render_sub_env 가 이 hw_verified 로 서브 위임 키 발급 판정:\n"
                "#       hw_verified: true\n"
                f"#       gpu_model: \"{ph.get('gpu_model')}\"\n"
                f"#       driver_version: \"{ph.get('driver')}\"\n"
                f"#       cuda_version_raw: \"{ph.get('cuda')}\"\n"
            )
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
