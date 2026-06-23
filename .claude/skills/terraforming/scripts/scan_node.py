#!/usr/bin/env python3
"""scan_node.py — terraforming 진입 루틴의 **결정론 스캔 코어** (판단 X, 사실만).

근거: docs/plan/plan_2026062311_1(진입 루틴 §2.3 스캔 인벤토리 · §2.4 성능게이트 · §2.5 α/γ 종료),
      plan_2026062312_1(output/ 통로), CLAUDE.md(결정론 스크립트 원칙 · 산출물 통로 불변식).

위치: 사용자 승인(HITL) 직후 호출되는 **deterministic** 단계. 5-전제조건 인터뷰·승인 게이트는
      terraforming 스킬(판단계층)이 담당하고, 이 스크립트는 그 뒤 "스캔→파싱→게이트 판정"만 한다.

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
    lines = [
        f"topology: {'multi' if result['topology_declared']=='multi' else 'single'}",
        f"cpu_arch: \"{result['cpu_arch']}\"",
        f"cuda_version: \"{result['cuda_version']}\"",
        f"gpus_per_node: {result['gpus_per_node']}",
        "interconnect:",
        f"  type: {ic['type']}",
        f"  hca_devices: {hcas}",
        f"  gid_index: {ic['gid_index']}",
        f"  socket_iface: {ic['socket_iface']}",
        f"  bandwidth_gbps: {ic['bandwidth_gbps'] if ic['bandwidth_gbps'] is not None else 'null'}",
        f"  platform_preset: {ic['platform_preset'] or '<set: e.g. dgx-spark-gb10>'}",
    ]
    return "\n".join(lines) + "\n"


def main() -> int:
    ap = argparse.ArgumentParser(description="terraforming 결정론 스캔 코어 (사실만)")
    ap.add_argument("--topology", choices=["single", "multi", "auto"], default="auto",
                    help="선언 토폴로지(인터뷰 결정). auto=스캔으로 추정(보고만, 게이트는 single/multi 명시 시)")
    ap.add_argument("--peer-ip", help="multi: 서브노드 IP(도달성 체크). 예: 192.168.100.11")
    ap.add_argument("--peer-port", type=int, default=22, help="도달성 체크 포트(기본 22=SSH)")
    ap.add_argument("--compose", default="output/multi/docker-compose.yaml",
                    help="교차검증할 docker-compose 경로")
    ap.add_argument("--bandwidth-gbps", type=float, default=None,
                    help="cross-node ib_write_bw 합산 실측(Gb/s). 성능게이트 입력. 미지정 시 perf pending.")
    ap.add_argument("--bw-floor", type=float, default=180.0,
                    help="성능게이트 합산 합격선(Gb/s). 기본 180(=200Gbps 풀대역폭의 ~90%, devlog 218 기준).")
    ap.add_argument("--emit-manifest", action="store_true",
                    help="검증 통과 시 manifest topology+interconnect 블록을 stdout 끝에 출력")
    ap.add_argument("--manifest", default=None,
                    help="manifest 실값 경로(기본=브랜치 파생 output/<topology>/manifest.yaml). plan_2026062315_1")
    args = ap.parse_args()

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

    # ── 3자-일치 단언 (branch ↔ manifest.topology ↔ scan) — 혼재 차단 ──
    branch = git_branch()
    branch_topo = {"single-node": "single", "multi-node": "multi"}.get(branch or "")
    # manifest 실값은 브랜치 파생 통로 output/<topology>/manifest.yaml (plan_2026062315_1).
    mani_path = args.manifest or (f"output/{branch_topo}/manifest.yaml" if branch_topo else "manifest.yaml")
    mani_topo = read_manifest_topology(mani_path)
    mism: list[str] = []     # blocking(혼재/위험)
    warns: list[str] = []    # 비blocking(정보)
    if args.topology in ("single", "multi"):
        if branch_topo and branch_topo != args.topology:
            mism.append(f"declared={args.topology} ≠ git branch({branch})⇒{branch_topo}")
        if args.topology == "multi" and not result["interconnect_present"]:
            mism.append("declared=multi 인데 스캔: RoCE v2 미탐지")
        if args.topology == "single" and result["interconnect_present"]:
            # 멀티 가능 머신을 단일노드로 운용 = 정상(시나리오: 메인/서브 각자 단일 서빙). blocking 아님.
            warns.append("single 선언 + RoCE 하드웨어 존재 — 멀티 가능 머신의 단일노드 운용(정상·정보)")
        if mani_topo and branch_topo and mani_topo != branch_topo:
            mism.append(f"manifest({mani_path}) topology={mani_topo} ≠ branch⇒{branch_topo}")
    result["consistency_assertion"] = {
        "git_branch": branch, "branch_implies": branch_topo,
        "manifest_path": mani_path, "manifest_topology": mani_topo, "declared": args.topology,
        "scan_interconnect_present": result["interconnect_present"],
        "consistent": not mism, "mismatches": mism, "warnings": warns,
    }

    # ── α/γ 토폴로지 게이트 (fail-closed) ───────────────────────────────
    exit_code = 0
    if args.topology == "single":
        result["gate"] = {"branch": "alpha", "status": "ok" if not mism else "blocked",
                          "mismatches": mism,
                          "note": "single-node: interconnect 스캔 skip(실패 아님)" if not mism
                                  else "일관성 단언 실패 → blocked"}
        if mism:
            exit_code = 2
    elif args.topology == "multi":
        bw = ic["bandwidth_gbps"]
        struct_block = []
        if not result["interconnect_present"]:
            struct_block.append("RoCE v2 미탐지")
        if args.peer_ip and not result["peer_check"]["reachable"]:
            struct_block.append(f"peer {args.peer_ip} 미도달")
        struct_block += mism
        if struct_block:                                   # γ: 구조/일관성 미충족
            result["gate"] = {"branch": "gamma", "status": "blocked", "reasons": struct_block,
                              "note": "멀티-ready manifest 미생성. 구조/일관성 확보 후 재실행."}
            exit_code = 2
        elif bw is None:                                   # 구조 충족, 성능 측정 대기
            pend = [] if args.peer_ip else ["peer-ip 미지정(도달성 미검증)"]
            result["gate"] = {"branch": "multi-ready-candidate", "status": "pending-perf", "pending": pend,
                              "note": f"구조 충족. ib_write_bw 합산 ≥{args.bw_floor}Gb/s 측정(--bandwidth-gbps) 후 ready."}
        elif bw < args.bw_floor:                           # γ: 성능 미달(fail-closed)
            result["gate"] = {"branch": "gamma", "status": "blocked",
                              "reasons": [f"대역폭 {bw}Gb/s < 합격선 {args.bw_floor}Gb/s"],
                              "note": "성능 미달 → 멀티-ready 거부(fail-closed). 오설정 점검(케이블 수/GID/MTU/GDR)."}
            exit_code = 2
        else:                                              # multi-ready: 구조+성능 통과
            result["gate"] = {"branch": "multi-ready", "status": "ok", "bandwidth_gbps": bw,
                              "note": f"구조+성능 통과(대역폭 {bw} ≥ {args.bw_floor}Gb/s). 멀티-ready."}
    else:  # auto — 보고만(게이트 판정 안 함)
        result["gate"] = {"branch": "auto", "status": "report-only",
                          "note": "토폴로지 미선언 — single/multi 명시 시 게이트 판정. 스캔 사실만 보고."}

    json.dump(result, sys.stdout, ensure_ascii=False, indent=2)
    sys.stdout.write("\n")
    if args.emit_manifest and result["gate"]["status"] == "ok" and args.topology in ("single", "multi"):
        sys.stdout.write("\n# ===== manifest 기입 블록 (검증 통과 — 사람 확인 후 manifest.yaml 반영) =====\n")
        sys.stdout.write(emit_manifest_block(result))
    return exit_code


if __name__ == "__main__":
    sys.exit(main())
