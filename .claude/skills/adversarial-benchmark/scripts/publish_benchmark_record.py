#!/usr/bin/env python3
"""publish_benchmark_record.py — full-런 인증서(flat 계약) 발행 (adversarial-benchmark full 모드).

편지 패턴 A: "이 모델을 이 HW/config 에서 테스트했고 통과했다"는 **기계 소비용 flat 계약 기록**.
  - **PASS 일 때만 발행**(report md 는 항상이지만 인증서는 "검증된 한계" 의미 → PASS 전용).
  - **flat 스키마**(중첩 ✗) — 미래 소비 도구가 stdlib/awk 한 줄로 독해(파싱편의 > 미학).
  - **carry-forward 재검증 헤더** 필수: 강한 일치 키(정확일치 실패=무효) + 소프트 지문(불일치=stale 경고).
  - **N/A fail-soft**: 결측은 N/A 원문(날조 ✗). 판정 권한 없음 — verdict 결과를 *기록*할 뿐.

입력: sweep_index.json(meta) + verdict_rule.py JSON. 출력: docs/benchmark/benchmark_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.yaml (시간토큰=doc_naming SSOT).
verdict != PASS 면 **미발행**(exit 0, 메시지만 — report 는 render_report.py 가 별도로 항상 발행).

stdlib only(yaml 라이브러리 비의존 — flat 보장 위해 직접 emit). 종료: 0=성공/미발행 · 2=입력 오류.
"""
import argparse, json, os, sys


def load(path, label):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        sys.stderr.write("[publish_record] ERROR %s: %s\n" % (label, e))
        sys.exit(2)


def repo_root(start):
    d = os.path.abspath(start)
    while d != "/":
        if os.path.isdir(os.path.join(d, ".git")):
            return d
        d = os.path.dirname(d)
    return os.getcwd()


def scalar(v):
    """flat yaml 스칼라: None/NA→N/A · bool→소문자 · 공백·특수문자 포함 문자열은 인용."""
    if v is None:
        return "N/A"
    if isinstance(v, bool):
        return "true" if v else "false"
    s = str(v)
    if s.strip() == "" or s.strip().upper() == "NA":
        return "N/A"
    if any(c in s for c in (":", "#", " ", "'", '"')) and not s.replace(".", "").replace("-", "").isdigit():
        return '"%s"' % s.replace('"', '\\"')
    return s


def build_yaml(index, verdict):
    meta = index.get("meta", {})
    rub = verdict.get("rubric") or {}
    completed = [lv.get("level") for lv in index.get("levels", []) if lv.get("status") == "ok"]
    trunc = index.get("truncated", [])
    L = []
    A = L.append

    # ── carry-forward 재검증 헤더 (문서화된 배너) ──
    A("# ⚠ CARRY-FORWARD 재검증 필수 — 이건 '그때-그 환경' 한정 측정이다(지도≠정답). 소비 전 재확인하라.")
    A("#   강한 일치 키(model/gpu/vllm/quant/topology/tp): 정확일치 실패 시 이 인증서 무효.")
    A("#   소프트 지문(driver/cuda/image/max-len/kv-bytes/gmu/moe): 불일치 시 stale — 재측정 권고.")
    A("#   발행 = full 모드 verdict==PASS 시만(결정론 publish_benchmark_record.py · inform-record).")
    A("schema_version: 1")
    A("record_type: benchmark_certificate")
    A("verdict: PASS")
    A("")
    A("# --- 강한 일치 키 (정확일치 필요) ---")
    for k in ("model", "gpu_model", "vllm_version", "quantization", "topology", "tensor_parallel_size"):
        A("%s: %s" % (k, scalar(meta.get(k))))
    A("")
    A("# --- 소프트 지문 (불일치 시 stale 경고) ---")
    for k in ("driver_version", "cuda_version", "image_tag", "max_model_len", "max_num_seqs",
              "kv_cache_memory_bytes", "kv_cache_dtype", "gpu_memory_utilization", "moe_backend",
              "enforce_eager"):
        A("%s: %s" % (k, scalar(meta.get(k))))
    A("ngc_base_tag: %s" % scalar(meta.get("ngc_base_tag")))  # 현재 resolved.json 부재 시 N/A(fail-soft)
    A("")
    A("# --- 검증 결과(인증서 본문) ---")
    A("benchmark_mode: full")
    A("decode_tps_conc1: %s" % scalar(verdict.get("measured_decode_tps")))
    A("primary_source: %s" % scalar(rub.get("source")))
    A("primary_tps: %s" % scalar(rub.get("primary")))
    A("floor_tps: %s" % scalar(rub.get("floor")))
    A("tolerance: %s" % scalar(rub.get("tolerance")))
    A("ratio_M_over_primary: %s" % scalar(rub.get("ratio_M_over_primary")))
    A("e_search: %s" % scalar(verdict.get("e_search")))
    A("spec_on: %s" % scalar(verdict.get("measured_spec_on")))
    A("accept_len: %s" % scalar(verdict.get("measured_accept_len")))
    A("sweep_levels: %s" % scalar(",".join(str(x) for x in completed) if completed else None))
    A("sweep_truncated: %s" % scalar("; ".join(trunc) if trunc else None))
    if verdict.get("balance"):
        b = verdict["balance"]
        A("node_vram_balance_dev: %s" % scalar(b.get("balance_dev")))
        A("node_vram_balance_pass: %s" % scalar(b.get("pass")))
    A("measured_utc: %s" % scalar(index.get("generated_utc")))
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser(description="full-런 인증서(flat 계약) 발행 — PASS시만")
    ap.add_argument("--sweep-index", required=True)
    ap.add_argument("--verdict-json", required=True)
    ap.add_argument("--out-dir", help="출력 디렉토리(기본 <repo>/docs/benchmark)")
    ap.add_argument("--stdout", action="store_true", help="파일 기록 대신 표준출력(테스트)")
    a = ap.parse_args()

    index = load(a.sweep_index, "sweep-index")
    verdict = load(a.verdict_json, "verdict-json")

    if verdict.get("verdict") != "PASS":
        sys.stderr.write("[publish_record] verdict=%s ≠ PASS → 인증서 미발행(report 는 render_report 가 항상 발행)\n"
                         % verdict.get("verdict"))
        return  # exit 0 — 정상(발행 조건 미충족)

    y = build_yaml(index, verdict)
    meta = index.get("meta", {})
    import sys as _s, os as _o; _s.path.insert(0, _o.path.dirname(_o.path.abspath(__file__)))
    from doc_naming import bench_filename
    _outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    fname = bench_filename("benchmark", meta, index.get("generated_utc"), (None if a.stdout else _outdir), "yaml")
    if a.stdout:
        sys.stdout.write(y)
        return
    outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    os.makedirs(outdir, exist_ok=True)
    outp = os.path.join(outdir, fname)
    with open(outp, "w", encoding="utf-8") as f:
        f.write(y)
    sys.stderr.write("[publish_record] 인증서 발행(PASS): %s\n" % outp)
    print(outp)


if __name__ == "__main__":
    main()
