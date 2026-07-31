#!/usr/bin/env python3
# roofline.py — 디코드 성능 *의심 임계* 결정론 산출 (adversarial-benchmark §8)
#
# ★ SLA 아님 · 측정>공식. R 은 100% MBU(memory-bandwidth utilization) 가정의 *낙관적 천장* +
#   "의심스럽게 느린가?" 트리거. 진짜 목표 = 외부 현실-달성치 E(검증기 (b) 외부검색). R 은 *방향*을 쓴다.
#
# spec-aware (dogfood BLOCK 교정):
#   R_fp    = forward-pass/sec 상한 = 1/(t_weight + t_comm)   (no-spec, 1 token/pass 가정)
#   R_token = mean_accept_len × R_fp                          (speculative=MTP 시 token/sec)
#   → no-MTP 서브는 R_fp 와, MTP 서브는 R_token 와 비교(like-with-like). spec on 이면 token/s 가 R_fp 초과 가능.
#   expected_achievable = realistic_fraction × R_token        (MBU 보정; batch=1 MoE 통상 0.25~0.4)
#
# 결정론 · stdlib only · 외부 네트워크 호출 없음. 입력=manifest(하드웨어)+모델 config/index(NAS 로컬).
# CONTRACT: JSON 출력 키는 verdict_rule.py 가 소비한다(고정).
import argparse, json, os, re, struct, sys

GIB = 1024 ** 3

# gpu_model(부분문자열, 대소문자무시) → peak 메모리 대역폭 GB/s(=GiBytes 아님, 10^9 bytes/s 관례).
# 참조-그라운디드 하드웨어 스펙(datasheet). 미지 모델 = fail-loud(추측 금지 — 헌법 §결정론).
GPU_PEAK_BW_GBPS = {
    "gb10": 273.0,            # DGX Spark GB10 LPDDR5X 통합메모리 (published ~273 GB/s)
    "rtx pro 6000": 1792.0,   # Blackwell GDDR7
    "rtx 6000 ada": 960.0,
    "rtx 4090": 1008.0,
    "rtx 5090": 1792.0,
    "h100": 3350.0,           # SXM5
    "h200": 4800.0,
    "a100": 2039.0,           # 80GB
    "l40": 864.0,
    "l40s": 864.0,
}


def _die(msg, code=2):
    sys.stderr.write("[roofline] ERROR: %s\n" % msg)
    sys.exit(code)


def _resolve_host_path(model_path, nas_host_root, nas_container_root="/app/models"):
    prefix = nas_container_root.rstrip("/")
    if model_path == prefix or model_path.startswith(prefix + "/"):
        rel = model_path[len(prefix):].lstrip("/")
        return os.path.join(nas_host_root, rel)
    return model_path


def _load_config(host_dir):
    cfgp = os.path.join(host_dir, "config.json")
    if not os.path.isfile(cfgp):
        _die("config.json 없음: %s (모델 부재 — 다운로드 금지)" % cfgp)
    with open(cfgp, "r", encoding="utf-8") as f:
        cfg = json.load(f)
    # text_config 중첩 우선(멀티모달 래퍼) — recipe parse_model_config 와 동일 관례.
    tc = cfg.get("text_config") if isinstance(cfg.get("text_config"), dict) else {}

    def pick(*keys, default=None):
        for k in keys:
            if k in tc and tc[k] is not None:
                return tc[k]
            if k in cfg and cfg[k] is not None:
                return cfg[k]
        return default

    return cfg, tc, pick


def _st_header(path):
    """safetensors 헤더만 읽는다(가중치 본문 아님 — 빠름). {name: {data_offsets:[s,e]}} 반환."""
    with open(path, "rb") as f:
        n = struct.unpack("<Q", f.read(8))[0]
        hdr = json.loads(f.read(n).decode("utf-8"))
    return hdr


_ROUTED_RE = re.compile(r"\.experts\.")          # routed experts (indexed or fused). shared_experts 는 '_experts' 라 미매치.
_MTP_RE = re.compile(r"(^|\.)mtp[\._]|\.mtp_block\.|model\.layers\.\d+\.mtp")


def _active_bytes_moe(host_dir, num_experts, num_experts_per_tok):
    """safetensors index/단일파일 헤더를 categorize → per-token 활성 가중치 바이트(MoE)."""
    idx = os.path.join(host_dir, "model.safetensors.index.json")
    shards = set()
    if os.path.isfile(idx):
        with open(idx, "r", encoding="utf-8") as f:
            wm = json.load(f).get("weight_map", {})
        shards = set(wm.values())
    else:
        single = os.path.join(host_dir, "model.safetensors")
        if os.path.isfile(single):
            shards = {"model.safetensors"}
        else:
            _die("safetensors index/단일파일 없음: %s" % host_dir)

    dense = 0       # 항상 읽힘: attention·shared_experts·embed·norm·lm_head
    routed = 0      # routed experts 전체 합(이후 k/n 스케일)
    for sh in sorted(shards):
        hdr = _st_header(os.path.join(host_dir, sh))
        for name, meta in hdr.items():
            if name == "__metadata__" or not isinstance(meta, dict):
                continue
            off = meta.get("data_offsets")
            if not off or len(off) != 2:
                continue
            nbytes = int(off[1]) - int(off[0])
            if _MTP_RE.search(name):
                continue  # MTP draft 모듈 = R_fp(메인 forward) 제외(accept_len 으로 모델링)
            if _ROUTED_RE.search(name) and "shared_expert" not in name:
                routed += nbytes
            else:
                dense += nbytes
    if not num_experts or num_experts <= 0:
        _die("MoE 인데 num_experts 불명 — config 확인")
    k = int(num_experts_per_tok or 0)
    n = int(num_experts)
    active_routed = int(routed * (k / n)) if k > 0 else 0
    return dense + active_routed, dense, routed, k, n


def _total_bytes_dense(host_dir):
    """비-MoE: 전 가중치(=항상 읽힘). **index weight_map 참조 샤드의 헤더 data_offsets 합**이 권위.

    ★ `metadata.total_size` 를 쓰면 안 된다 — 디스크 바이트가 아니라 **발행자가 계산해 적은 값**이라
      틀릴 수 있다. Olmo-3.1-32B-Instruct 는 32B 파라미터를 fp32(4 B) 기준으로 적어 실제 bf16
      가중치의 **정확히 2배**(120.08 GiB vs 60.04)를 신고하고, 그대로 쓰면 R_fp 가 2.12 t/s 로
      절반이 되어 **판정 기준 자체가 무너진다**(2026-08-01 실측).
    ★ `os.listdir` 글롭도 안 된다 — 동거 포맷 세트를 함께 센다(LFM2 의 F32 ↔ bf16).
    MoE 경로(_active_bytes_moe)는 이미 weight_map 을 쓴다. dense 경로만 어긋나 있었다.
    (parse_model_config._native_weight_bytes 와 같은 원칙 — 다만 여기선 루프라인이 요구하는
     **텐서 바이트**가 필요하므로 파일 크기가 아니라 헤더 data_offsets 를 합산한다.)
    """
    idx = os.path.join(host_dir, "model.safetensors.index.json")
    shard_names = None
    if os.path.isfile(idx):
        with open(idx, "r", encoding="utf-8") as f:
            blob = json.load(f)
        wm = blob.get("weight_map")
        if isinstance(wm, dict) and wm:
            cand = sorted(set(wm.values()))
            if all(os.path.isfile(os.path.join(host_dir, n)) for n in cand):
                shard_names = cand
        ts = blob.get("metadata", {}).get("total_size")
    else:
        ts = None
    total = 0
    for fn in (shard_names if shard_names is not None else sorted(os.listdir(host_dir))):
        if fn.endswith(".safetensors"):
            hdr = _st_header(os.path.join(host_dir, fn))
            for name, m in hdr.items():
                if name == "__metadata__" or not isinstance(m, dict):
                    continue
                off = m.get("data_offsets")
                if off and len(off) == 2:
                    total += int(off[1]) - int(off[0])
    if total <= 0:
        _die("가중치 바이트 산출 실패: %s" % host_dir)
    # total_size 는 교차검증용. 어긋나면 조용히 넘기지 않는다 — 그래야 다음 사람이 재조사하지 않는다.
    if ts and total > 0:
        ratio = float(ts) / total
        if not (0.9 <= ratio <= 1.1):
            sys.stderr.write(
                "[roofline] ⚠ index metadata.total_size=%.2f GiB 가 실제 텐서 합 %.2f GiB 의 "
                "%.2f배 — 발행자 오기재. 실측 헤더 합을 채택한다.\n"
                % (ts / 1024 ** 3, total / 1024 ** 3, ratio))
    return total


def _bandwidth_gbps(gpu_model, override):
    if override:
        return float(override), "override"
    gm = (gpu_model or "").lower()
    for key, bw in GPU_PEAK_BW_GBPS.items():
        if key in gm:
            return bw, "lookup:%s" % key
    _die("gpu_model '%s' 대역폭 미상 — --bandwidth-gbps 로 명시(추측 금지)" % gpu_model)


def main():
    ap = argparse.ArgumentParser(description="디코드 성능 의심 임계 루프라인 (spec-aware R_fp/R_token)")
    ap.add_argument("--model-path", required=True, help="컨테이너 경로(/app/models/...) 또는 호스트 경로")
    ap.add_argument("--nas-root", default="/mnt/models", help="/app/models 매핑 호스트 NAS 루트")
    ap.add_argument("--manifest", help="manifest.yaml (gpu_model·interconnect·gpus_per_node)")
    ap.add_argument("--tp", type=int, help="tensor-parallel-size (미지정 시 manifest nodes×gpus 또는 1)")
    ap.add_argument("--accept-len", type=float, default=1.0, help="mean acceptance length(speculative). spec off=1.0")
    ap.add_argument("--bandwidth-gbps", type=float, help="노드당 peak 메모리 대역폭 GB/s override")
    ap.add_argument("--interconnect-gbps", type=float, help="노드간 대역폭 Gb/s(bits) override")
    ap.add_argument("--act-dtype-bytes", type=int, default=2, help="활성 dtype 바이트(all-reduce 메시지, 기본 bf16=2)")
    ap.add_argument("--comm-latency-us-per-layer", type=float, default=0.0, help="레이어당 all-reduce 지연(µs). 기본 0=대역폭만(보수)")
    ap.add_argument("--realistic-fraction", type=float, default=0.35, help="MBU 보정(batch=1 MoE 통상 0.25~0.4). expected=fraction×R")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    host_dir = _resolve_host_path(args.model_path, args.nas_root)
    if not os.path.isdir(host_dir):
        _die("모델 디렉토리 없음: %s (다운로드 금지·중단)" % host_dir)

    cfg, tc, pick = _load_config(host_dir)
    hidden = pick("hidden_size")
    n_layers = pick("num_hidden_layers")
    num_experts = pick("num_experts", "n_routed_experts", "num_local_experts")
    n_per_tok = pick("num_experts_per_tok", "num_experts_per_token", "moe_topk")
    is_moe = bool(num_experts)
    notes = []

    # --- 하드웨어 (manifest) ---
    gpu_model, gpus_per_node, ic_gbps, manifest_topology = None, 1, None, None
    if args.manifest and os.path.isfile(args.manifest):
        try:
            import yaml  # PyYAML 6 (호스트)
            with open(args.manifest, "r", encoding="utf-8") as f:
                man = yaml.safe_load(f) or {}
            gpu_model = man.get("gpu_model")
            gpus_per_node = int(man.get("gpus_per_node", 1) or 1)
            manifest_topology = man.get("topology")
            ic = man.get("interconnect") or {}
            ic_gbps = ic.get("bandwidth_gbps")
        except Exception as e:
            notes.append("manifest 파싱 경고: %s" % e)

    # tp 결정: 명시 > manifest(nodes 수 × gpus_per_node) > 1
    tp = args.tp
    if tp is None and args.manifest and os.path.isfile(args.manifest):
        try:
            import yaml
            with open(args.manifest, "r", encoding="utf-8") as f:
                man = yaml.safe_load(f) or {}
            # topology=single → 노드 배수 1 고정(nodes[role=sub] 는 sub-control 피어이지 텐서
            # 워커가 아님 — δ1-1 실버그 · manifest_contract.manifest_tp 와 동일 계약).
            if (manifest_topology or "").startswith("single"):
                tp = max(1, gpus_per_node)
            else:
                nodes = man.get("nodes") or []
                tp = max(1, len(nodes)) * max(1, gpus_per_node) if nodes else None
        except Exception:
            tp = None
    if tp is None:
        tp = 1
        notes.append("tp 미지정 → 1 가정")

    bw_gbps, bw_src = _bandwidth_gbps(gpu_model, args.bandwidth_gbps)
    ic_gbps = args.interconnect_gbps if args.interconnect_gbps else ic_gbps

    # --- 활성 바이트 (per forward pass) ---
    if is_moe:
        active_bytes, dense_b, routed_b, k, n = _active_bytes_moe(host_dir, num_experts, n_per_tok)
        notes.append("MoE active = dense %.2fGiB + (%d/%d)×routed %.2fGiB" % (dense_b/GIB, k, n, routed_b/GIB))
    else:
        active_bytes = _total_bytes_dense(host_dir)
        notes.append("dense 모델 — active = 전 가중치")

    per_node_read = active_bytes / tp
    bw_bytes = bw_gbps * 1e9                      # GB/s → bytes/s
    t_weight = per_node_read / bw_bytes           # sec

    # --- comm (멀티노드 TP all-reduce; = "RDMA였나?"의 결정론 답) ---
    if tp > 1 and hidden and n_layers and ic_gbps:
        comm_bytes_tok = 2.0 * (tp - 1) / tp * float(hidden) * float(args.act_dtype_bytes) * float(n_layers)
        ic_bytes = float(ic_gbps) * 1e9 / 8.0     # Gb/s(bits) → bytes/s
        t_comm = comm_bytes_tok / ic_bytes + (args.comm_latency_us_per_layer * float(n_layers) * 1e-6)
    else:
        comm_bytes_tok, t_comm = 0.0, 0.0
        if tp > 1 and not ic_gbps:
            notes.append("tp>1 인데 interconnect 대역폭 불명 → comm 무시(과소추정 주의)")

    denom = t_weight + t_comm
    if denom <= 0:
        _die("루프라인 분모 0 — 입력 확인")
    r_fp = 1.0 / denom
    accept = max(1.0, float(args.accept_len))
    r_token = accept * r_fp
    expected = args.realistic_fraction * r_token
    comm_bound = t_comm > t_weight

    out = {
        "model_path": args.model_path,
        "is_moe": is_moe,
        "tp": tp,
        "gpu_model": gpu_model,
        "bandwidth_gbps": bw_gbps,
        "bandwidth_source": bw_src,
        "interconnect_gbps": ic_gbps,
        "active_bytes": int(active_bytes),
        "active_gib": round(active_bytes / GIB, 3),
        "per_node_read_gib": round(per_node_read / GIB, 3),
        "t_weight_ms": round(t_weight * 1e3, 4),
        "t_comm_ms": round(t_comm * 1e3, 4),
        "comm_bound": comm_bound,
        "accept_len": accept,
        "R_fp": round(r_fp, 2),
        "R_token": round(r_token, 2),
        "realistic_fraction": args.realistic_fraction,
        "expected_achievable": round(expected, 2),
        "notes": notes,
        "caveat": "R 은 의심 임계(낙관적 천장). 측정 M·외부 레퍼런스 E 가 진실. spec on 이면 R_token 과, off 면 R_fp 와 비교.",
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
