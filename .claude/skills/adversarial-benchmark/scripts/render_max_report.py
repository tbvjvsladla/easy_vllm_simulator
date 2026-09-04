#!/usr/bin/env python3
"""render_max_report.py — Max 모드 HW 안전-최대 컨텍스트 envelope 보고서 결정론 렌더러.

Max(별도 오퍼레이션 · plan_26071510 §Max)의 max_envelope.sh 산출 max_index.json 을 사람이 읽는
envelope 보고서로 렌더한다. full-모드 render_report.py 와 동형 규율:
  - **inform-only**: 안전상한을 *특성화 표시*만(판정 게이트 아님). - **결정론**(LLM 표저작 ✗). - **N/A fail-soft**.
  - **carry-forward 재검증 배너**(지도≠정답 — 드라이버/HW 바뀌면 봉투 재측정).

출력: docs/benchmark/max_envelope_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md (시간토큰=doc_naming SSOT).
stdlib only. 종료: 0=성공 · 2=입력 오류.
"""
import argparse, json, os, sys


def load(path, label):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        sys.stderr.write("[render_max] ERROR %s: %s\n" % (label, e))
        sys.exit(2)


def na(v):
    if v is None:
        return "N/A"
    if isinstance(v, str) and v.strip().upper() in ("", "NA", "N/A"):
        return "N/A"
    return str(v)


def repo_root(start):
    d = os.path.abspath(start)
    while d != "/":
        if os.path.isdir(os.path.join(d, ".git")):
            return d
        d = os.path.dirname(d)
    return os.getcwd()


def build_md(idx):
    meta = idx.get("meta", {})
    model = meta.get("model", "NA"); gpu = meta.get("gpu_model", "NA"); vllm = meta.get("vllm_version", "NA")
    ceil = idx.get("safe_ceiling_max_model_len")
    L = []; A = L.append
    A("# Max 모드 — HW 안전-최대 컨텍스트 envelope: `%s` @ %s · vLLM %s" % (model, gpu, vllm))
    A("")
    A("> ⚠ **CARRY-FORWARD 재검증**: 이 envelope 은 아래 환경 한정 특성화다(그때-그 HW·드라이버·이미지). "
      "다른 세션·하드웨어·**드라이버 버전**에서 이 안전상한을 그대로 신뢰하지 말 것 — 재측정. **inform-only**"
      "(안전상한 특성화 표시 · 판정 게이트 아님). 별도 오퍼레이션(벤치마커 인프라 공유). 생성일 %s."
      % idx.get("generated_utc", "N/A"))
    A("")
    A("## 안전-최대 컨텍스트 (핵심 결과)")
    A("")
    A("| 항목 | 값 |")
    A("|---|---|")
    A("| **안전상한 max-model-len** | **%s%s** |" % (na(ceil), " tokens" if ceil else ""))
    A("| 스텝업 축 | %s (안전측 오름차순) |" % na(idx.get("axis")))
    A("| 원 config max-model-len | %s |" % na(idx.get("original_max_model_len")))
    A("")
    A("> 안전상한 = **serve+smoke 를 통과한 최대 컨텍스트**(그 위 레벨은 실패/트립/미시도). 각 레벨은 재서빙(reload)+"
      "기능 스모크 + 협역 워치독/로드-전 RAM 게이트 관측으로 판정. **선-기록 후-위험**(레벨 결과는 다음 시도 전 기록).")
    A("")
    A("## 레벨별 결과 (컨텍스트 스텝업)")
    A("")
    A("| max-model-len | serve+smoke | 판정 |")
    A("|---|---|---|")
    for lv in idx.get("safe_levels", []):
        ex = lv.get("serve_smoke_exit")
        A("| %s | exit %s | ✅ 안전 |" % (na(lv.get("max_model_len")), na(ex)))
    A("")
    trunc = idx.get("truncated", [])
    if trunc:
        A("**⚠ 안전상한 위 / 미시도 (silent truncation 금지):**")
        for t in trunc:
            A("- %s" % t)
        A("")
    else:
        A("_요청 전 레벨이 안전(안전상한을 넘는 실패 없음 — 상한 위를 더 probe하려면 --levels 확장)._")
        A("")
    A("## 측정 환경 스냅샷")
    A("")
    A("| 키 | 값 |")
    A("|---|---|")
    for label, key in [("model", "model"), ("gpu_model", "gpu_model"), ("vllm_version", "vllm_version"),
                       ("topology", "topology"), ("tensor_parallel_size", "tensor_parallel_size"),
                       ("driver_version", "driver_version"), ("quantization", "quantization"),
                       ("moe_backend", "moe_backend"), ("image_tag", "image_tag")]:
        A("| %s | %s |" % (label, na(meta.get(key))))
    A("")
    A("---")
    A("_Max = 별도 오퍼레이션(벤치마커 측정 인프라만 공유 · native 모드 아님) · 결정론 렌더 `render_max_report.py`(LLM 표저작 ✗) · "
      "안전 = 헌법 §호스트 안전체계 따름정리 · plan_26071510._")
    return "\n".join(L) + "\n"


def main():
    ap = argparse.ArgumentParser(description="Max envelope 보고서 결정론 렌더러(inform-only)")
    ap.add_argument("--max-index", required=True, help="max_envelope.sh 산출 max_index.json")
    ap.add_argument("--out-dir", help="출력 디렉토리(기본 <repo>/docs/benchmark)")
    ap.add_argument("--stdout", action="store_true")
    a = ap.parse_args()
    idx = load(a.max_index, "max-index")
    md = build_md(idx)
    meta = idx.get("meta", {})
    import sys as _s, os as _o; _s.path.insert(0, _o.path.dirname(_o.path.abspath(__file__)))
    from doc_naming import bench_filename, scan_bench_dir
    _outdir = a.out_dir or os.path.join(repo_root(a.max_index), "docs", "benchmark")
    fname = bench_filename("max_envelope", meta, idx.get("generated_utc"),
                           (None if a.stdout else scan_bench_dir(_outdir, "max_envelope")), "md")
    if a.stdout:
        sys.stdout.write(md); return
    outdir = a.out_dir or os.path.join(repo_root(a.max_index), "docs", "benchmark")
    os.makedirs(outdir, exist_ok=True)
    outp = os.path.join(outdir, fname)
    with open(outp, "w", encoding="utf-8") as f:
        f.write(md)
    sys.stderr.write("[render_max] 발행: %s (안전상한=%s)\n" % (outp, idx.get("safe_ceiling_max_model_len")))
    print(outp)


if __name__ == "__main__":
    main()
