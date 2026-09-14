#!/usr/bin/env python3
"""render_report.py — full-런 사람용 성능 보고서 결정론 렌더러 (adversarial-benchmark full 모드).

편지 패턴 B: full 벤치 종결 시 판정과 **별개로** 사람이 읽는 per-metric report 를 발행한다.
  - **항상 발행**(PASS/FAIL 무관 — "왜 느렸나"를 사람이 봐야 하므로).
  - **inform-only**: verdict 를 *표시만* 한다. PASS/FAIL 판정 권한 없음(verdict_rule.py 독점).
  - **결정론**: 이 스크립트가 sweep_index.json + verdict JSON 을 읽어 md 표로 렌더. **LLM 은 표·숫자 저작 ✗**.
  - **N/A fail-soft**: 결측 필드는 "N/A" 원문 기록(대체값 날조 ✗).

입력: sweep_bench.sh 산출 sweep_index.json(meta + per-level measured) + verdict_rule.py JSON(판정점 결과).
출력: docs/benchmark/bench_report_<YYMMDDHH>[_MM_SS]_<model>_<gpu>_<vllm>.md (기본 · 시간토큰=doc_naming SSOT) — 부하 스윕 곡선·루프라인 컨텍스트·환경 스냅샷.

`--lite-only`(2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q4·Q10): lite 만 잰 셀의 **경량 리포트**. sweep_index·verdict
없이 `lite_bench.sh` raw(`--lite-raw-json`)만 읽어 같은 접두사·같은 명명 SSOT 로 발행한다 — 헤더 `mode: lite` · 측정 구성
표(bench_mode=선언된 lite) · lite 지표 5종 표 · 환경 스냅샷. 판정·루프라인·인증서는 없다(lite 는 inform-only). 이 문서가
hint `hint_map_only` 통로의 바인딩 대상이다(인증서는 PASS·full 전용이라 lite 셀에는 구조적으로 없다).
두 모드 모두 **측정 구성 표**(bench_mode · 도구 · 반복 N · downgrade_reason)를 같은 모양으로 싣는다 — hint 발행기
(`render_bench_section.parse_measurement_config`)가 이 표를 결정론으로 되읽는다(형식이 두 스크립트 사이의 계약이다).

stdlib only. 종료: 0=성공 · 2=입력 오류.
"""
import argparse, json, os, re, sys


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


# ── 측정 구성 표 (2026-09-14 · plan_26091407 §4.5 · 사용자 결정 Q10) ─────────────────────────────────────────
# 등급 표지는 태그 **이름**에 새기지 않는다(이름은 불변 · 5세그먼트). 대신 "무엇으로 · 몇 번 · 어느 모드로 쟀나" 를
# 이 표에 **기재**한다(게이트 ✗). 표의 제목·키 이름·행 모양은 hint 발행기 `render_bench_section.py` 의 파싱 계약이다
# (`MEASUREMENT_CONFIG_TITLE` · `MEASUREMENT_CONFIG_KEYS`) — 두 자리의 일치는 `selftest_lite_report.py` L10 이 대조하고,
# 계약 밖 키는 파서가 FAIL 한다(여기만 바꾸면 verify_distribution 이 잡는다).
# 값이 없으면 `N/A` 로 적는다(0·빈칸으로 접지 않는다). 셀 안의 `|` 는 이스케이프한다(표가 깨지면 파서가 행을 잃는다).
MEASUREMENT_CONFIG_TITLE = "## 측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)"
MEASUREMENT_CONFIG_KEYS = ("bench_mode", "bench_mode_kind", "bench_mode_source", "downgrade_reason",
                           "downgrade_reason_source", "bench_tool", "bench_tool_version", "bench_tool_version_source",
                           "repeats", "repeats_source", "repeats_completed")
# lite 레그의 도구는 언제나 `vllm bench serve` 다(SKILL.md 도구 구성 `full = lite ∪ GuideLLM`). 토큰은 sweep_bench 조립부가
# meta.bench_tool 에 쓰는 것과 같은 철자다(`vllm-bench-serve`) — 두 자리의 일치는 selftest_lite_report.py 가 대조한다.
LITE_BENCH_TOOL = "vllm-bench-serve"
LITE_BENCH_TOOL_VERSION_SOURCE = "declared(도구가 버전을 자기보고하지 않는다)"
LITE_DECLARED_DETAIL = "lite_bench --publish-report · lite-only 셀(full 을 시도하지 않았다)"
LITE_REPEATS_SOURCE = "lite(cold 1회 + warm burst 1회 · 반복 축 없음 — full 정의 밖)"


def _cell(v):
    if v is None or (isinstance(v, str) and v.strip().upper() in ("", "NA", "N/A")):
        return "N/A"
    return " ".join(str(v).split()).replace("|", "\\|")


def measurement_config_section(values):
    """측정 구성 값 dict → 표 절(line 목록). 키 순서는 `MEASUREMENT_CONFIG_KEYS` 고정."""
    L = [MEASUREMENT_CONFIG_TITLE, "",
         "> 같은 모델·하드웨어라도 도구·반복·모드가 다르면 수치를 나란히 놓지 않는다. `bench_mode=lite` 는 full 정의"
         "(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않은 측정이다 — 선언된 lite-only 이거나 반복 불성립으로 강등된 셀이다"
         "(강등이면 `downgrade_reason`). 이 표는 기재이지 판정이 아니다.", "",
         "| 키 | 값 |", "|---|---|"]
    for key in MEASUREMENT_CONFIG_KEYS:
        L.append("| %s | %s |" % (key, _cell(values.get(key))))
    L.append("")
    return L


def full_measurement_config(index, bench_mode_record, bench_mode_status):
    """full 스윕 산출물 → 측정 구성 값. bench_mode 의 권위는 classify_cell 판정 기록이다(여기서 다시 판정 ✗)."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from classify_cell import bench_mode_kind
    meta = index.get("meta") or {}
    rep = index.get("repetition") if isinstance(index.get("repetition"), dict) else {}
    rec = bench_mode_record if isinstance(bench_mode_record, dict) else None
    vp = index.get("verdict_point_level")
    completed = (rep.get("completed_by_level") or {}).get(str(vp)) if rep else None
    return {
        "bench_mode": rec.get("bench_mode") if rec else None,
        "bench_mode_kind": bench_mode_kind(rec),
        "bench_mode_source": (rec.get("bench_mode_source") if rec else "미확정 — %s" % bench_mode_status),
        "downgrade_reason": rec.get("downgrade_reason") if rec else None,
        "downgrade_reason_source": rec.get("downgrade_reason_source") if rec else None,
        "bench_tool": meta.get("bench_tool"),
        "bench_tool_version": meta.get("bench_tool_version"),
        "bench_tool_version_source": meta.get("bench_tool_version_source"),
        "repeats": rep.get("requested") if rep else None,
        "repeats_source": (rep.get("requested_source") if rep else "absent(반복 축 기록 없음 — 신설 전 산출물)"),
        # 판정점(인증서·판정이 묶이는 레벨)의 완주 run 수 — 강등 판정의 범위와 같은 자리다(repeat_axis 헤더 ★).
        "repeats_completed": completed,
    }


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
    # 측정 구성 표 — 판정 절 **앞**에 둔다(무엇으로 쟀는지가 판정을 읽기 전의 조건이다). 스윕 표·절삭 블록의 형식
    # (hint 파서 계약)과 무관한 별개 절이다.
    lines.extend(measurement_config_section(full_measurement_config(index, bench_mode_record, bench_mode_status)))

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


# ── --lite-only: 경량 리포트 (2026-09-14 · plan_26091407 §4.5) ─────────────────────────────────────────────────
def _read_text(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as f:
            return f.read()
    except (OSError, TypeError):
        return ""


def _grep_yaml(text, key):
    m = re.search(r"(?m)^\s*%s\s*:\s*([^\n#]+)" % re.escape(key), text)
    return m.group(1).strip().strip('"').strip("'") if m else None


def _grep_env(text, key):
    m = re.search(r"(?m)^\s*%s\s*=\s*([^\n#]*)" % re.escape(key), text)
    return (m.group(1).strip().strip('"').strip("'") or None) if m else None


class LiteIdentityError(ValueError):
    """이름을 지을 강한 키를 세우지 못했다 — 리포트를 'NA' 이름으로 발행하지 않는다."""


def lite_identity(raw, environ=None):
    """lite raw(`lite_bench.sh` 가 쓴 파일 경로들) → 명명·환경 스냅샷 키.

    ★ 규칙은 `sweep_bench.sh` 조립 heredoc 과 **같다**(모델 축 = 체크포인트 basename 소문자 · gpu_model = manifest 단일
      권위 · vllm = env EASY_VLLM_VERSION > envfile IMAGE_TAG `easy-vllm:X.Y.Z` > 엔진 로그 자기보고 > config 주석). 같은
      모델·GPU·버전의 full 리포트와 **같은 이름 축**을 가져야 두 문서가 한 조합으로 모인다. heredoc 은 bash 안에 있어
      import 할 수 없으므로 단일 소유가 불가능하고, 교차검증이 차선이다 — `selftest_lite_report.py` 가 같은 입력을 두
      자리에 넣어 model/gpu_key/vllm_version 이 같은지 대조한다(workflow.md §결정론 규율).
    값마다 출처(`*_source`)를 붙인다. gpu_model 부재는 sweep_bench 와 같이 멈춘다(NA 강한 키로 이름 짓지 않는다)."""
    environ = os.environ if environ is None else environ
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from doc_naming import gpu_key as _gpu_key
    cfg = str(raw.get("config_name") or "").strip()
    cfgtext = _read_text(raw.get("config_yaml"))
    envtext = _read_text(raw.get("env_file"))
    mftext = _read_text(raw.get("manifest"))
    elog = _read_text(raw.get("engine_log"))
    gpu_model = _grep_yaml(mftext, "gpu_model")
    if not gpu_model:
        raise LiteIdentityError("manifest(%s) 에 gpu_model 이 없다 — 리포트 이름의 gpu 축을 NA 로 짓지 않는다"
                                % raw.get("manifest"))
    mpath = _grep_yaml(cfgtext, "model") or ""
    mbase = os.path.basename(mpath.rstrip("/")).strip()
    if mbase:
        model, model_source = mbase.lower(), "derived(model_path basename · lowercased)"
    elif _grep_env(envtext, "SERVING_MODEL_NAME"):
        model, model_source = _grep_env(envtext, "SERVING_MODEL_NAME"), "fallback(serving_model_name)"
    elif cfg:
        model, model_source = cfg, "fallback(config_name)"
    else:
        raise LiteIdentityError("모델 축을 세울 입력이 없다(config yaml model · SERVING_MODEL_NAME · config_name 전부 부재)")
    m = re.search(r"\bv([0-9]+\.[0-9]+\.[0-9]+(?:\.dev[0-9]*)?(?:\+g[0-9a-f]+)?[^\s)]*)", elog)
    vllm_build = m.group(1) if m else None
    vllm, vllm_source = None, None
    if environ.get("EASY_VLLM_VERSION"):
        vllm, vllm_source = environ["EASY_VLLM_VERSION"], "declared(env EASY_VLLM_VERSION)"
    if not vllm:
        mi = re.search(r"easy-vllm:([0-9]+\.[0-9]+\.[0-9]+)", _grep_env(envtext, "IMAGE_TAG") or "")
        if mi:
            vllm, vllm_source = mi.group(1), "derived(envfile IMAGE_TAG)"
    if not vllm and vllm_build:
        mb = re.match(r"([0-9]+\.[0-9]+\.[0-9]+)", vllm_build)
        if mb:
            vllm, vllm_source = mb.group(1), "measured(engine log 자기보고)"
    if not vllm:
        mc = re.search(r"vLLM[\s]*([0-9]+\.[0-9]+\.[0-9]+)", cfgtext)
        vllm, vllm_source = (mc.group(1), "declared(config yaml comment)") if mc else ("NA", "absent(env·envfile·엔진 로그·config 모두)")
    return {"model": model, "model_source": model_source,
            "serving_config": _grep_env(envtext, "SERVING_MODEL_NAME") or cfg or "NA",
            "gpu_model": gpu_model, "gpu_key": _gpu_key(gpu_model),
            "vllm_version": vllm, "vllm_version_source": vllm_source,
            "vllm_build": vllm_build or "NA", "topology": raw.get("topology")}


def lite_measurement_config():
    """선언된 lite-only 셀의 측정 구성 값. bench_mode 기록의 **모양**은 classify_cell(어휘 소유자)이 만든다."""
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    from classify_cell import bench_mode_kind, declared_lite_record
    rec = declared_lite_record(LITE_DECLARED_DETAIL)
    return {"bench_mode": rec["bench_mode"], "bench_mode_kind": bench_mode_kind(rec),
            "bench_mode_source": rec["bench_mode_source"],
            "downgrade_reason": rec["downgrade_reason"], "downgrade_reason_source": rec["downgrade_reason_source"],
            "bench_tool": LITE_BENCH_TOOL, "bench_tool_version": None,
            "bench_tool_version_source": LITE_BENCH_TOOL_VERSION_SOURCE,
            "repeats": 1, "repeats_source": LITE_REPEATS_SOURCE, "repeats_completed": 1}


def build_lite_md(raw, lite, identity):
    """lite raw + lite_metrics.build 결과 + 명명 키 → 경량 리포트 md. 수치는 lite_metrics 표를 **그대로** 싣는다(재산정 ✗)."""
    L = []
    A = L.append
    A("# 경량 성능 보고서(lite) — `%s` @ %s · vLLM %s" % (identity["model"], identity["gpu_model"], identity["vllm_version"]))
    A("")
    A("mode: lite")
    A("")
    A("> ⚠ **CARRY-FORWARD 재검증**: 이 보고서는 아래 '측정 환경' 한정의 **lite 스냅샷**이다(그때-그 HW/config). "
      "판정·루프라인·동시성 곡선·반복·인증서가 **없다** — lite 는 inform-only 이며 verdict_rule 에 투입하지 않는다. "
      "성능 baseline 으로 승격하지 말고 소비 전 재측정할 것. 생성일 %s." % raw.get("measured_utc"))
    A("")
    L.extend(measurement_config_section(lite_measurement_config()))
    A("## lite 지표 (서빙 성공 직후 스냅샷 · inform-only)")
    A("")
    A("> cold(warmup 0 단일요청 → cold TTFT) 1회 + warm burst(N=%s · conc=1 · warmup 1) 1회. 산정·표 렌더는 "
      "`lite_metrics.py`(결정론)이며 이 문서는 그 표를 옮기지 않고 그대로 싣는다. 결측은 `N/A`(대체값 ✗)."
      % na(raw.get("burst_n")))
    A("")
    A(lite.get("table") or "> ⚠ lite 표 산정 실패 — 이 리포트는 lite 지표가 **결손**이다.")
    A("")
    A("## 측정 환경 스냅샷")
    A("")
    A("| 키 | 값 |")
    A("|---|---|")
    for key in ("model", "model_source", "serving_config", "gpu_model", "vllm_version", "vllm_version_source",
                "vllm_build", "topology"):
        A("| %s | %s |" % (key, _cell(identity.get(key))))
    for key in ("burst_n", "backend", "endpoint", "gen_src"):
        src = lite if key == "gen_src" else raw
        A("| %s | %s |" % (key, _cell(src.get(key))))
    A("")
    A("---")
    A("_결정론 렌더 = `render_report.py --lite-only`(LLM 표저작 ✗) · 참조 체인: lite raw → 이 report → testlog(판정) → devlog(서사)._")
    return "\n".join(L) + "\n"


def main_lite(a):
    """`--lite-only` 진입점. 종료: 0=발행 · 2=입력 오류(raw 판독 불가 · 측정시각 부재 · 명명 키 불성립 · 명명 충돌)."""
    raw = load(a.lite_raw_json, "lite-raw-json")
    if not isinstance(raw, dict):
        sys.stderr.write("[render_report] ERROR --lite-raw-json 이 객체가 아니다\n")
        sys.exit(2)
    measured_utc = raw.get("measured_utc")
    if not (isinstance(measured_utc, str) and re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", measured_utc)):
        # 측정시각은 이름의 시간 토큰이자 '같은 측정인가' 판정의 근거다 — 날조하지 않는다.
        sys.stderr.write("[render_report] ERROR lite raw 에 measured_utc(UTC 초) 가 없다 — 측정시각을 날조하지 않는다\n")
        sys.exit(2)
    sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
    import lite_metrics
    try:
        lite = lite_metrics.build(raw)
        identity = lite_identity(raw)
    except (LiteIdentityError, SystemExit) as exc:
        sys.stderr.write("[render_report] ERROR 경량 리포트를 세울 수 없다: %s\n" % exc)
        sys.exit(2)
    md = build_lite_md(raw, lite, identity)
    from doc_naming import bench_filename, scan_bench_dir, NamingCollisionExhausted, NamingSourceUnreadable
    outdir = a.out_dir or os.path.join(repo_root(a.lite_raw_json), "docs", "benchmark")
    try:
        fname = bench_filename("bench_report", identity, measured_utc,
                               (None if a.stdout else scan_bench_dir(outdir, "bench_report")), "md")
    except (NamingCollisionExhausted, NamingSourceUnreadable) as exc:
        sys.stderr.write("[render_report] ERROR 명명 충돌 — 덮어쓰지도 새 접미를 발명하지도 않는다: %s\n" % exc)
        sys.exit(2)
    if a.stdout:
        sys.stdout.write(md)
        return
    os.makedirs(outdir, exist_ok=True)
    outp = os.path.join(outdir, fname)
    with open(outp, "w", encoding="utf-8") as f:
        f.write(md)
    sys.stderr.write("[render_report] 경량 리포트 발행(lite): %s\n" % outp)
    print(outp)


def main():
    ap = argparse.ArgumentParser(description="full-런 사람용 보고서 결정론 렌더러(inform-only)")
    ap.add_argument("--sweep-index", help="sweep_bench.sh 산출 sweep_index.json(full 모드 필수)")
    ap.add_argument("--verdict-json", help="verdict_rule.py 출력 JSON(판정점 · full 모드 필수)")
    ap.add_argument("--lite-only", action="store_true",
                    help="경량 리포트 모드 — sweep_index·verdict 없이 --lite-raw-json 만 읽는다(plan_26091407 §4.5)")
    ap.add_argument("--lite-raw-json", help="lite_bench.sh 산출 raw readings JSON(--lite-only 필수)")
    ap.add_argument("--roofline-json", help="roofline.py 출력 JSON(선택 — rubric 에 없으면 보강)")
    ap.add_argument("--bench-mode-json",
                    help="classify_cell 의 bench_mode 판정 기록(생략 시 sweep_index 옆 사이드카 — 부재·판독 실패·다른 "
                         "측정의 기록이면 '미확정' 으로 적고 발행한다 · 명시 경로가 그러면 exit 2)")
    ap.add_argument("--out-dir", help="출력 디렉토리(기본 <repo>/docs/benchmark)")
    ap.add_argument("--stdout", action="store_true", help="파일 기록 대신 표준출력(테스트)")
    a = ap.parse_args()

    if a.lite_only:
        # 두 모드를 섞지 않는다 — lite 리포트에 스윕·판정을 끼우면 inform-only 스냅샷이 판정 문서처럼 읽힌다.
        if a.sweep_index or a.verdict_json or a.roofline_json or a.bench_mode_json or not a.lite_raw_json:
            sys.stderr.write("[render_report] ERROR --lite-only 는 --lite-raw-json 만 받는다"
                             "(--sweep-index/--verdict-json/--roofline-json/--bench-mode-json 과 섞지 않는다)\n")
            sys.exit(2)
        return main_lite(a)
    if a.lite_raw_json or not (a.sweep_index and a.verdict_json):
        sys.stderr.write("[render_report] ERROR full 모드는 --sweep-index 와 --verdict-json 이 필수다"
                         "(--lite-raw-json 은 --lite-only 전용)\n")
        sys.exit(2)

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
