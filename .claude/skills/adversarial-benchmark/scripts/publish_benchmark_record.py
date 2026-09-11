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
    A("#   model = 체크포인트에서 파생한 **모델 축**(소문자). 운영 조합명은 serving_config 축에 따로 산다.")
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
    # --- 운영 조합명(정본 필드 · 2026-08-15 신설) ---
    # `model` 은 **모델 축**이다(체크포인트에서 파생). 그런데 같은 모델을 사다리 칸/이미지 변종으로
    # 가르려면 서빙 config 이름이 갈려야 하고(`ds4f0731-x2-sm12x` vs `-fork`), 종전 스키마엔 그 축을
    # 담을 칸이 없어 조합명이 `model` 로 흘러들었다 → hint 태그의 `<model>` 세그먼트와 영구 불일치
    # (`IDENTITY_MISMATCH:model`, 2026-08-15). 축이 둘이면 칸도 둘이어야 한다.
    # **강한 일치 키가 아니다** — 같은 모델의 서로 다른 칸을 carry-forward 로 갈라 세우는 것은
    # 소프트 지문의 일이고, 강한키를 늘리면 completion_gate 의 6키 계약(STRONG_IDENTITY_FIELDS)이
    # 갈라진다. 항상 발행한다(결측이면 N/A) — 있을 때만 적으면 소비자가 부재와 결측을 구분 못 한다.
    A("# --- 운영 조합명(모델 축 아님 · 정본 필드) ---")
    A("serving_config: %s" % scalar(meta.get("serving_config")))
    A("model_source: %s" % scalar(meta.get("model_source")))
    A("")
    A("# --- 소프트 지문 (불일치 시 stale 경고) ---")
    # ★ 2026-09-04(plan_26090415 §3.6): 측정 **도구** 축을 소프트 지문으로 싣는다.
    #   강한키로 올리지 않는 이유는 위 serving_config 주석과 같다 — 7번째 강한키는 부재값 None 대
    #   "N/A" 비교 때문에 레거시 인증서 45장을 같은 vLLM-bench 런에 대해서도 즉시 무효화한다.
    #   같은 범주(측정 환경 지문)인 image_digest 가 이미 소프트로 굳어 있고, runtime_selftest 가
    #   `image_digest not in CERTIFICATE_RUN_KEY_FIELDS` 를 하드 assert 해 그 처방을 tripwire 로
    #   못박아 두었다. 도구가 바뀌면 인증서가 무효가 되는 것이 아니라 **불일치가 보이는** 것이 옳다.
    for k in ("driver_version", "cuda_version", "image_tag", "image_digest", "max_model_len", "max_num_seqs",
              "kv_cache_memory_bytes", "kv_cache_dtype", "gpu_memory_utilization", "moe_backend",
              "enforce_eager", "bench_tool", "bench_tool_version", "bench_tool_version_source",
              # 커널 축은 **실측**이 정본이다(2026-09-04). 요청과 실효가 갈릴 수 있고
              # (`VLLM_ATTENTION_BACKEND=FLASHINFER` 인데 엔진은 TRITON_ATTN 을 썼다),
              # 인증서가 요청값을 실었다면 그 인증서는 쓰지 않은 커널로 잰 것처럼 읽힌다.
              "attention_backend", "attention_backend_source", "attention_backend_mismatch",
              "moe_backend_source", "moe_backend_mismatch",
              # ★ 2026-09-11(plan_26091108 R9): PLE 상주/mmap 축. 이 축이 **인증서 어디에도
              #   없어서** camp-26090918 의 res·mmp 두 인증서가 강한키·소프트키 전부 동일했고,
              #   hint 태그 이름이 충돌해 타임스탬프 접미사로 회피했다. 47.7GiB 가 상주하느냐
              #   NVMe 에서 오느냐는 예산·지연을 동시에 바꾸는 축이며, 축이 없으면 그 셀은
              #   carry-forward 대조에서 다른 셀과 구분되지 않는다.
              "ple_mode", "ple_mode_source"):
        A("%s: %s" % (k, scalar(meta.get(k))))
    A("ngc_base_tag: %s" % scalar(meta.get("ngc_base_tag")))  # 현재 resolved.json 부재 시 N/A(fail-soft)
    A("")
    A("# --- 검증 결과(인증서 본문) ---")
    A("benchmark_mode: full")
    A("decode_tps_conc1: %s" % scalar(verdict.get("measured_decode_tps")))
    # 루브릭 **권한**(weak|explicit|explore) — 어느 권한에서 잰 판정인지 인증서가 스스로 밝힌다
    # (헌법 §결정론 규율 출처 표시 · plan_26082219 A6). 승격 판정기(completion_gate)가 이 필드로
    # explore 계약을 읽는다. 결측은 scalar() 가 N/A 로 fail-soft(legacy 인증서 관용).
    A("rubric_authority: %s" % scalar(rub.get("authority")))
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
    # --- lite 지표 (full ⊇ lite 불변식) ---
    # lite 만 돈 모델과 full 을 돈 모델의 열 집합이 **중첩**되어야 조건별 비교가 성립한다.
    # 이 5행이 없으면 인증서는 lite 기록과 교집합 관계가 되어 carry-forward 비교가 반쪽이 된다.
    # 값은 lite_metrics.py 산정을 sweep_index 가 그대로 실어온 것이다(재산정 ✗ — 산정 권위 단일).
    _lite = index.get("lite") if isinstance(index.get("lite"), dict) else {}
    _cap = (_lite.get("capacity") or {}).get("main") or {}
    A("lite_included: %s" % scalar("true" if _lite.get("table") else "false"))
    A("lite_gen_tps_warm: %s" % scalar(_lite.get("gen_tps")))
    A("lite_gen_src: %s" % scalar(_lite.get("gen_src")))
    A("lite_cold_ttft_ms: %s" % scalar(_lite.get("cold_ttft_ms")))
    A("lite_kv_gib: %s" % scalar(_lite.get("kv_gib")))
    A("lite_gpu_occupancy: %s" % scalar(_cap.get("gpu")))
    A("lite_ram_occupancy: %s" % scalar(_cap.get("ram")))
    if verdict.get("balance"):
        b = verdict["balance"]
        A("node_vram_balance_dev: %s" % scalar(b.get("balance_dev")))
        A("node_vram_balance_pass: %s" % scalar(b.get("pass")))
    A("measured_utc: %s" % scalar(index.get("generated_utc")))
    # ── 측정 노드 출처 (2026-09-06 신설 · plan_26090616 ⑤) ──
    # 왜: 인증서에 **어느 노드가 쟀는지**가 없었다. 그래서 메인과 서브가 같은 모델·같은 버전을
    # 재면 파일명까지 동명이 되어 서로를 덮었고, hint 발행은 그 인증서를 메인 것으로만 읽었다 —
    # 서브가 완주했는데도 그 증거가 태그에 닿는 경로가 아예 없었다(2026-09-05 실측).
    # 강한 일치 키가 아니다(6키 계약을 늘리지 않는다) — **출처 표시**다(헌법 §결정론 규율).
    # 항상 적는다: 결측이면 N/A 여야 부재와 결측이 구분된다.
    A("measured_node: %s" % scalar(meta.get("measured_node")))
    A("measured_node_source: %s" % scalar(meta.get("measured_node_source")))
    # 벤치 종료 시 서버 생존(2026-09-07 · 유예 결함 ②) — 소프트 지문이 아니라 **측정 유효성 사실**이다.
    #   false 면 이 측정의 errored 는 도구 경계로 면제되지 않았다는 뜻이다(파서가 이미 반영했다).
    A("server_alive_at_bench_end: %s" % scalar(meta.get("server_alive_at_bench_end")))
    A("boundary_exemption: %s" % scalar(meta.get("boundary_exemption")))
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
    from doc_naming import bench_filename, scan_bench_dir
    _outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    # 같은 측정 = 덮어쓰기 · 다른 측정 = _MM_SS · 판독 불가 = fail-loud (doc_naming 규약)
    fname = bench_filename("benchmark", meta, index.get("generated_utc"),
                           (None if a.stdout else scan_bench_dir(_outdir, "benchmark")), "yaml")
    if a.stdout:
        sys.stdout.write(y)
        return
    outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    os.makedirs(outdir, exist_ok=True)
    outp = os.path.join(outdir, fname)
    with open(outp, "w", encoding="utf-8") as f:
        f.write(y)
    sys.stderr.write("[publish_record] 인증서 발행(PASS): %s\n" % outp)
    _register_campaign_evidence(a.sweep_index, outp, meta)
    print(outp)


def _register_campaign_evidence(sweep_index_path: str, cert_path: str, meta: dict) -> None:
    """증거가 docs 평면에서 **태어난 그 자리**에서 캠페인 포인터를 남긴다
    (2026-09-07 · plan_26090715 §4.1 호출부).

    왜 여기인가: 종전에는 인증서가 발행되고도 `evidence_pointers.json` 에 손으로 적어야 했다.
    손이 빠지면 purge 게이트가 "증거가 없다"고 하거나(닫힘) 더 나쁘게는 증거가 있는데 목록에
    없어 지워도 되는 것으로 보인다. 포맷 소유는 campaign_init writer 하나이고 여기는 호출부다.
    ACTIVE 가 `_bootstrap` 이면 writer 가 스스로 no-op 이므로 캠페인 밖 측정은 영향이 없다.
    실패는 **삼키지 않는다** — 인증서는 이미 발행됐으므로 죽이지는 않고 stderr 로 올린다.
    """
    import subprocess
    root = repo_root(sweep_index_path)
    ci = os.path.join(root, ".claude", "skills", "terraforming_node", "scripts", "campaign_init.py")
    if not os.path.isfile(ci):
        # 부재는 침묵이 아니라 배선 결함이다(2026-09-08 · plan_26090813 F5). 조용히 넘어가면
        # "캠페인 밖 측정" 과 "도구가 안 배달됐다" 가 구분되지 않는다.
        sys.stderr.write("[publish_record] campaigns writer 부재(%s) — 인증서는 발행됐지만 "
                         "증거 포인터를 아무도 적지 않았다\n" % ci)
        return
    try:
        rel = os.path.relpath(cert_path, root)
    except ValueError:
        return
    node = str(meta.get("measured_node") or "").strip()
    args = [sys.executable, ci, "--evidence-add", "--kind", "certificate", "--path", rel]
    if node:
        args += ["--node", node]
    cfg = str(meta.get("config_name") or "").strip()
    if cfg:
        args += ["--cell", cfg]
    cp = subprocess.run(args, capture_output=True, text=True, cwd=root, check=False)
    if cp.returncode != 0:
        sys.stderr.write("[publish_record] ⚠ 캠페인 증거 포인터 등재 실패 — %s\n"
                         % (cp.stderr or "").strip())


if __name__ == "__main__":
    main()
