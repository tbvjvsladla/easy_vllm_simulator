#!/usr/bin/env python3
"""render_report.py — full-런 사람용 성능 보고서 결정론 렌더러 (adversarial-benchmark full 모드).

편지 패턴 B: full 벤치 종결 시 판정과 **별개로** 사람이 읽는 per-metric report 를 발행한다.
  - **항상 발행**(PASS/FAIL 무관 — "왜 느렸나"를 사람이 봐야 하므로).
  - **inform-only**: verdict 를 *표시만* 한다. PASS/FAIL 판정 권한 없음(verdict_rule.py 독점).
  - **결정론**: 이 스크립트가 sweep_index.json + verdict JSON 을 읽어 md 표로 렌더. **LLM 은 표·숫자 저작 ✗**.
  - **N/A fail-soft**: 결측 필드는 "N/A" 원문 기록(대체값 날조 ✗).

입력: sweep_bench.sh 산출 sweep_index.json(meta + per-level measured) + verdict_rule.py JSON(판정점 결과).
출력: docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md (기본 · 시간토큰=doc_naming SSOT) — 부하 스윕 곡선·루프라인 컨텍스트·환경 스냅샷.

stdlib only. 종료: 0=성공 · 2=입력 오류.
"""
import argparse, json, os, sys


def load(path, label):
    try:
        with open(path, encoding="utf-8") as f:
            return json.load(f)
    except Exception as e:
        sys.stderr.write("[render_report] ERROR %s: %s\n" % (label, e))
        sys.exit(2)


def na(v, suffix=""):
    """None/NA fail-soft. 값 있으면 (문자열+suffix), 없으면 'N/A'."""
    if v is None:
        return "N/A"
    if isinstance(v, str) and v.strip().upper() in ("", "NA", "N/A"):
        return "N/A"
    return "%s%s" % (v, suffix)


def fnum(v, fmt="%.2f", suffix=""):
    try:
        return (fmt % float(v)) + suffix
    except (TypeError, ValueError):
        return "N/A"


def repo_root(start):
    d = os.path.abspath(start)
    while d != "/":
        if os.path.isdir(os.path.join(d, ".git")):
            return d
        d = os.path.dirname(d)
    return os.getcwd()


def _bench_mode_view(index, record, record_status):
    """bench_mode 판정 기록(classify_cell 사이드카 · `read_bench_mode_record` 판독 결과) → 한 줄 서술.
    기록이 없거나·판독 실패·다른 측정의 것이면 '미확정' 과 그 사유를 적는다(추측 ✗ · full 로 접지 않는다)."""
    if not isinstance(record, dict):
        return "미확정 — %s" % record_status
    mode, reason = record.get("bench_mode"), record.get("downgrade_reason")
    if mode is None:
        return "미확정 — %s" % (record.get("bench_mode_source") or "사유 N/A")
    corr = record.get("downgrade_correlation")
    if reason:
        return ("**%s** — 반복 불성립(기계 이벤트)으로 **강등** · 사유 `%s` · 사살 대조 `%s` (%s)"
                % (mode, reason, corr, record.get("downgrade_reason_source")))
    return "**%s** · 사살 대조 `%s` (%s)" % (mode, corr, record.get("bench_mode_source"))


def _repetition_section(index, bench_mode_record, bench_mode_status):
    """반복 축 · 재현 밴드 절. 스윕 표(render_bench_section 이 파싱하는 형식 계약)와 **별개 표**로 둔다."""
    L = []
    A = L.append
    rep = index.get("repetition") if isinstance(index.get("repetition"), dict) else None
    A("## 반복 축 · 재현 밴드 (full 정의 = lite ∪ GuideLLM × 반복 ≥3)")
    A("")
    A("> full 정의는 레벨마다 같은 running serve 에 **반복 ≥3** 이다. 위 스윕 표와 판정점·인증서는 **대표 run(첫 완주 run) "
      "하나**의 값이다(평균·합성 ✗). 재현 밴드는 **기재**다 — 판정 게이트는 불변이고 분산은 강등 사유가 "
      "아니다. 반복 불성립(판정점 run 실패 · 스윕이 멈춘 자리의 블랙박스 kill)만 셀을 lite 로 강등하며 확정은 "
      "`classify_cell.py` 가 한다. 포화 경계에서 스윕이 멈춘 것(적응 상한 클램프)은 강등이 아니다.")
    A("")
    A("- bench_mode: %s" % _bench_mode_view(index, bench_mode_record, bench_mode_status))
    if rep is None:
        A("")
        A("> ⚠ **반복 축 기록 없음** — 반복 축 신설(2026-09-14) 전 산출물이거나 재조립 원천에 runs[] 가 없다. "
          "이 리포트의 수치는 단일 run 이며 **산포 추정치가 없다**(다른 도구·조건의 밴드를 빌려 쓰지 말 것).")
        A("")
        return L
    A("- 요청 반복: %s (%s) · 종류 `%s` · 레벨별 완주 min %s"
      % (na(rep.get("requested")), na(rep.get("requested_source")), na(rep.get("kind")),
         na(rep.get("completed_min"))))
    stop = rep.get("stop")
    if isinstance(stop, dict):
        what = "반복 중단" if stop.get("kind") == "repeat-break" else "적응 상한 클램프(레벨 첫 run 실패)"
        A("- ⚠ **스윕이 멈춘 자리 — %s**: level %s run %s — %s (남은 반복·상위 레벨은 측정하지 않았다 · "
          "조용히 자르지 않는다)" % (what, stop.get("level"), stop.get("run"), na(stop.get("detail"))))
    if rep.get("unrecorded_levels"):
        A("- ⚠ runs[] 가 없는 레벨: %s (집계 실패 또는 이전 산출물 — 완주 0 회로 읽지 말 것)"
          % rep["unrecorded_levels"])
    A("")
    A("| 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |")
    A("|---|---|---|---|---|")
    for lv in index.get("levels", []):
        m = lv.get("measured") or {}
        runs = m.get("runs") if isinstance(m.get("runs"), list) else None
        if runs is None:
            A("| %s | N/A | N/A | N/A | runs[] 없음 |" % lv.get("level"))
            continue
        seq = " / ".join((fnum(r.get("decode_tps")) if r.get("measurement_ok") else "✗(run %s)" % r.get("run"))
                         for r in runs)
        A("| %s | %s/%s | %s | %s | %s |" % (lv.get("level"), na(m.get("repeats_completed")),
                                           na(m.get("repeats_requested")), seq,
                                           fnum(m.get("repro_band_pct")), na(m.get("repro_band_source"))))
    A("")
    A("_밴드 = %s · 지표 %s · 완주 run 만 · 2회 미만이면 정의되지 않는다(N/A) · 레벨 run 시도 합 %s._"
      % (rep.get("band_formula") or "N/A", rep.get("band_metric") or "N/A", na(rep.get("runs_attempted"))))
    A("")
    return L


def build_md(index, verdict, roofline, bench_mode_record=None, bench_mode_status="absent(판정 기록을 넘기지 않았다)"):
    meta = index.get("meta", {})
    model = meta.get("model", "NA")
    gpu = meta.get("gpu_model", "NA")
    vllm = meta.get("vllm_version", "NA")
    lines = []
    A = lines.append

    A("# 성능 보고서 — `%s` @ %s · vLLM %s" % (model, gpu, vllm))
    A("")
    # --- carry-forward 재검증 배너 (앵커링 방지 — 지도≠정답) ---
    A("> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정 계측이다(그때-그 HW/config). "
      "다른 세션·하드웨어·드라이버·이미지에서 이 수치를 그대로 신뢰하지 말 것 — 소비 전 재측정. "
      "이 문서는 **inform-only**(판정 권한 없음 · verdict_rule 독점). 생성일 %s." % index.get("generated_utc", "N/A"))
    A("")

    # --- 판정 표시(inform-only) ---
    v = verdict.get("verdict", "N/A")
    rub = verdict.get("rubric") or {}
    A("## 판정 (표시만 — verdict_rule.py 결과)")
    A("")
    A("| 항목 | 값 |")
    A("|---|---|")
    A("| verdict | **%s** |" % v)
    A("| 측정 decode t/s (동시성1) | %s |" % na(verdict.get("measured_decode_tps"), " t/s"))
    A("| 루브릭 권한 | %s |" % na(rub.get("authority")))  # weak|explicit|explore (표시만 — plan_26082219 A7)
    A("| 루브릭 primary | %s (%s) |" % (na(rub.get("primary"), " t/s"), na(rub.get("source"))))
    A("| floor (primary×(1−tol)) | %s |" % na(rub.get("floor"), " t/s"))
    A("| ratio (M/primary) | %s |" % na(rub.get("ratio_M_over_primary")))
    A("| tolerance | %s |" % na(rub.get("tolerance")))
    A("| E-search 상태 | %s |" % na(verdict.get("e_search")))
    if verdict.get("warning"):
        A("| ⚠ warning | %s |" % verdict["warning"])
    if verdict.get("balance"):
        b = verdict["balance"]
        # ★ 라벨은 이 축이 **게이트인지 서술인지**에 따라 갈린다(SKILL.md §2.1 · U1 후속).
        #   explore 에서는 편차 초과가 verdict 를 뒤집지 않으므로 "REFUTE" 로 적으면 전체 판정(PASS)과
        #   모순돼 보인다 — 리포트가 판정과 서술을 뒤섞지 않도록 gates_verdict 로 표기를 나눈다.
        gates = b.get("gates_verdict")
        if b.get("pass"):
            state = "PASS" if gates else "정상(서술)"
        else:
            state = "REFUTE" if gates else "편차 초과(서술 — explore 에서 게이트 아님)"
        A("| 노드간 VRAM 밸런스 | dev=%s (tol=%s) → %s |"
          % (na(b.get("balance_dev")), na(b.get("tolerance")), state))
    A("")
    if v == "REFUTE" and verdict.get("refuted_claims"):
        A("**기각 사유(사람 참고):**")
        for rc in verdict["refuted_claims"]:
            A("- %s — %s" % (rc.get("claim", ""), rc.get("reason", "")))
        for h in verdict.get("diagnosis_hint", []):
            A("  - 진단 힌트: %s" % h)
        A("")

    # --- lite 지표 (full ⊇ lite 불변식) ---
    # full 은 lite 의 상위집합이어야 한다 — 그래야 lite 만 돈 모델과 full 을 돈 모델의
    # 지표 열(column)이 **중첩**되어 조건별 비교가 성립한다. 교집합이면 비교가 깨진다.
    _lite = index.get("lite")
    A("## lite 지표 (full ⊇ lite — 열 집합 중첩 보장)")
    A("")
    if isinstance(_lite, dict) and _lite.get("table"):
        A("> full 런이 lite 를 **포함해서 실행**한 결과다(별도 재측정 아님). cold-start TTFT 는 "
          "warmup 0 단일요청 측정이라 스윕(warmup 有)이 잴 수 없는 축이다.")
        A("")
        A(_lite["table"])
    elif isinstance(_lite, dict) and _lite.get("error"):
        A("> ⚠ lite 산정 실패: %s — 이 report 는 lite 열이 **결손**이다." % _lite["error"])
    else:
        A("> ⚠ **lite 미포함** — 이 full 런은 lite 를 수집하지 못했다(열 집합 결손). "
          "lite 만 돈 다른 모델과 직접 비교하지 마라.")
    A("")

    # --- 부하 스윕 곡선 (client-load · reload 0) ---
    A("## 부하 스윕 곡선 (client-load · reload 없음)")
    A("")
    A("> 단일 running serve 에 동시 요청 수만 변화(동시성 축). **동시성=1 행이 판정점**(verdict 재사용). "
      "request-rate=inf(각 레벨 포화 측정). config-space(batch×maxlen)는 이 스윕 밖(Max 모드/explorer 소관).")
    A("")
    A("| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |")
    A("|---|---|---|---|---|---|---|")
    for lv in index.get("levels", []):
        m = lv.get("measured") or {}
        marker = " ★판정점" if lv.get("level") == index.get("verdict_point_level") else ""
        A("| %s%s | %s | %s | %s | %s | %s | %s/%s |" % (
            lv.get("level"), marker,
            na(m.get("decode_tps")), na(m.get("output_throughput")),
            na(m.get("total_token_throughput")), na(m.get("ttft_ms_median")),
            na(m.get("itl_ms_median")),
            na(m.get("completed")), na(m.get("failed"))))
    A("")
    # --- 절삭 로그 (silent truncation 금지) ---
    trunc = index.get("truncated", [])
    if trunc:
        A("**⚠ 절삭된 부하 레벨(적응 상한 클램프 — 조용히 자르지 않음):**")
        for t in trunc:
            A("- %s" % t)
        A("")
    else:
        A("_절삭된 레벨 없음(요청 전 레벨 완주)._")
        A("")

    # --- 반복 축 · 재현 밴드 (2026-09-14 · plan_26091407 §4.4) ---
    # 스윕 표·절삭 블록 **뒤**에 둔다 — 그 둘의 형식은 hint 발행기(render_bench_section)의 파싱 계약이다.
    lines.extend(_repetition_section(index, bench_mode_record, bench_mode_status))

    # --- 루프라인 컨텍스트 ---
    A("## 루프라인 컨텍스트 (결정론 상한 — 의심 임계, SLA 아님)")
    A("")
    rl = roofline or {}
    A("| 지표 | 값 |")
    A("|---|---|")
    A("| R_fp (forward-pass/s 상한, 100%% MBU) | %s |" % na(rub.get("R_fp") or rl.get("R_fp"), " t/s"))
    A("| R_token (spec, accept_len×R_fp) | %s |" % na(rub.get("R_token") or rl.get("R_token"), " t/s"))
    A("| expected_achievable (루프라인×MBU) | %s |" % na(rub.get("expected_achievable") or rl.get("expected_achievable"), " t/s"))
    A("| spec(MTP) on (측정) | %s (accept_len=%s) |"
      % (na(verdict.get("measured_spec_on")), na(verdict.get("measured_accept_len"))))
    A("")

    # --- 측정 환경 스냅샷 (재현·재검증 근거) ---
    A("## 측정 환경 스냅샷")
    A("")
    A("| 키 | 값 |")
    A("|---|---|")
    for label, key in [
        ("model", "model"), ("model_source", "model_source"), ("serving_config", "serving_config"),
        ("gpu_model", "gpu_model"), ("vllm_version", "vllm_version"),
        # `*_source` 를 값 바로 옆에 렌더한다(2026-08-23 · plan_26082322). 이 두 값은 config yaml
        # 에도 serve-plane CLI 에도 살 수 있어서, 값만 보면 "N/A = 양자화 없음"으로 오독된다 —
        # 2026-08-23 R7 이 실제로 그렇게 읽혔다. 출처가 보이면 `measured(engine log)` 인지
        # `absent(both)` 인지가 표에서 바로 갈린다(헌법 §결정론 규율 "값 옆에 출처 필드").
        ("quantization", "quantization"), ("quantization_source", "quantization_source"),
        ("topology", "topology"), ("tensor_parallel_size", "tensor_parallel_size"),
        ("driver_version", "driver_version"), ("cuda_version", "cuda_version"), ("image_tag", "image_tag"),
        ("max_model_len", "max_model_len"), ("max_num_seqs", "max_num_seqs"),
        ("kv_cache_memory_bytes", "kv_cache_memory_bytes"),
        ("kv_cache_dtype", "kv_cache_dtype"), ("kv_cache_dtype_source", "kv_cache_dtype_source"),
        ("gpu_memory_utilization", "gpu_memory_utilization"), ("moe_backend", "moe_backend"),
        ("enforce_eager", "enforce_eager"), ("model_path", "model_path"),
    ]:
        A("| %s | %s |" % (label, na(meta.get(key))))
    A("| 입력 길이(sweep) | %s |" % na(index.get("input_len")))
    A("")
    A("---")
    A("_결정론 렌더 = `render_report.py`(LLM 표저작 ✗) · 참조 체인: bench JSON → 이 report → testlog(판정) → devlog(서사)._")
    return "\n".join(lines) + "\n"


def main():
    ap = argparse.ArgumentParser(description="full-런 사람용 보고서 결정론 렌더러(inform-only)")
    ap.add_argument("--sweep-index", required=True, help="sweep_bench.sh 산출 sweep_index.json")
    ap.add_argument("--verdict-json", required=True, help="verdict_rule.py 출력 JSON(판정점)")
    ap.add_argument("--roofline-json", help="roofline.py 출력 JSON(선택 — rubric 에 없으면 보강)")
    ap.add_argument("--bench-mode-json",
                    help="classify_cell 의 bench_mode 판정 기록(생략 시 sweep_index 옆 사이드카 — 부재·판독 실패·다른 "
                         "측정의 기록이면 '미확정' 으로 적고 발행한다 · 명시 경로가 그러면 exit 2)")
    ap.add_argument("--out-dir", help="출력 디렉토리(기본 <repo>/docs/benchmark)")
    ap.add_argument("--stdout", action="store_true", help="파일 기록 대신 표준출력(테스트)")
    a = ap.parse_args()

    index = load(a.sweep_index, "sweep-index")
    verdict = load(a.verdict_json, "verdict-json")
    roofline = load(a.roofline_json, "roofline-json") if a.roofline_json else None
    # bench_mode 판정 기록: 판독(자리·신선도)의 소유는 classify_cell 이다. 관례 자리(스윕 디렉터리 사이드카)의
    # 부재·판독 실패·낡음은 리포트 발행을 막지 않고 '미확정 — 사유' 로 적는다 — 리포트는 PASS/FAIL 무관 **항상**
    # 발행이고, 파생 기록의 결손을 발행 실패로 격상하지 않는다(리뷰 정정). 사람이 **명시**한 경로가 그러면 인자
    # 오류라 exit 2 다.
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from classify_cell import read_bench_mode_record
    bm_record, bm_status = read_bench_mode_record(a.sweep_index, index, a.bench_mode_json)
    if a.bench_mode_json and bm_record is None:
        sys.stderr.write("[render_report] ERROR --bench-mode-json 을 쓸 수 없다: %s\n" % bm_status)
        sys.exit(2)

    md = build_md(index, verdict, roofline, bm_record, bm_status)
    meta = index.get("meta", {})
    import sys as _s, os as _o; _s.path.insert(0, _o.path.dirname(_o.path.abspath(__file__)))
    from doc_naming import bench_filename, scan_bench_dir
    _outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    fname = bench_filename("bench_report", meta, index.get("generated_utc"),
                           (None if a.stdout else scan_bench_dir(_outdir, "bench_report")), "md")

    if a.stdout:
        sys.stdout.write(md)
        return
    outdir = a.out_dir or os.path.join(repo_root(a.sweep_index), "docs", "benchmark")
    os.makedirs(outdir, exist_ok=True)
    outp = os.path.join(outdir, fname)
    with open(outp, "w", encoding="utf-8") as f:
        f.write(md)
    sys.stderr.write("[render_report] 발행(항상): %s (verdict=%s)\n" % (outp, verdict.get("verdict")))
    print(outp)


if __name__ == "__main__":
    main()
