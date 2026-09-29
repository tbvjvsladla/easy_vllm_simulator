#!/usr/bin/env python3
"""conformance_full_superset_lite.py — **full ⊇ lite** 불변식 결정론 검사.

근거: 2026-07-31 사용자 지시. 종전 두 모드는 상위집합이 아니라 **교집합**이었다 —
lite 만 cold TTFT·시스템 RAM·per-node nvidia-smi 를 재고, full 만 스윕·루프라인·verdict 를 냈다.
그러면 lite 로 잰 모델과 full 로 잰 모델의 지표 **열(column) 집합이 달라져 조건별 비교가 깨진다**.

이 검사가 존재하는 이유: 불변식을 산문으로만 적어두면 지켜지지 않는다. 같은 날 이 프로젝트에서
파서가 두 벌로 갈라져 어긋난 사건(D4↔D6)이 났고, 그 전날엔 `pipefail` 교훈이 한 파일에만 남아
결함 계열을 못 막았다. **불변식은 시험으로 고정한다.**

검사 대상(라이브 하드웨어 불요 — 정적/픽스처):
  A. lite_metrics 가 산출하는 지표 키 ⊆ sweep_index 의 lite 블록이 싣는 키
  B. render_report 가 lite 표를 렌더하는 경로를 갖는다
  C. publish_benchmark_record 가 lite 5종을 인증서에 emit 한다
  D. sweep_bench 가 lite_bench 를 실제로 호출한다(포함관계의 담지체)
  E. **게이트 포함**(2026-09-29 · plan_26092923 · 인터뷰 interview_20260929_132122) — 열 포함만으로는 부족했다.
     lite 레그가 실패해도 `lite truncated` 로 적고 GuideLLM 으로 넘어가 "full 은 됐는데 lite 는 불성립" 인 교집합이
     존재할 수 있었다(D1). 이제 lite 판정(raw `lite_verdict` 단일 권위)이 pass 가 아니면 GuideLLM 에 들어가지 않는다.
     E 는 그 선행관계를 정적 순서·판정 어휘·D1 연결거부 픽스처로 고정한다.

종료: 0=PASS · 1=FAIL
"""
import os
import re
import sys

SDIR = os.path.dirname(os.path.abspath(__file__))

# lite 가 사람에게 보여주는 5종(references/lite-and-publication.md §1 "5종 메트릭").
# 이 목록이 늘면 아래 검사가 자동으로 full 쪽 결손을 잡는다.
LITE_METRIC_KEYS = ["gen_tps", "cold_ttft_ms", "kv_gib", "capacity"]
CERT_LITE_FIELDS = [
    "lite_included", "lite_gen_tps_warm", "lite_cold_ttft_ms",
    "lite_kv_gib", "lite_gpu_occupancy", "lite_ram_occupancy",
]


def read(name):
    p = os.path.join(SDIR, name)
    if not os.path.isfile(p):
        return None
    with open(p, encoding="utf-8", errors="replace") as f:
        return f.read()


def main():
    results = []

    def check(name, ok, detail=""):
        results.append((name, ok, detail))

    sweep = read("sweep_bench.sh")
    report = read("render_report.py")
    cert = read("publish_benchmark_record.py")
    lite_m = read("lite_metrics.py")

    for label, src in (("sweep_bench.sh", sweep), ("render_report.py", report),
                       ("publish_benchmark_record.py", cert), ("lite_metrics.py", lite_m)):
        check("파일 존재: %s" % label, src is not None)
    if any(s is None for s in (sweep, report, cert, lite_m)):
        _emit(results)
        return 1

    # D. full 이 lite 를 **실행**하는가 (포함관계의 구조적 담지체)
    check("sweep_bench 가 lite_bench.sh 를 호출한다",
          "lite_bench.sh" in sweep,
          "호출이 없으면 full 은 lite 를 포함할 수 없다(병렬 목록 유지로 퇴행)")
    # E. 게이트 포함 — lite 불통과면 GuideLLM 레벨 호출 전에 끝난다
    gate_at = sweep.find('exit "$GATE_RC"')
    level_at = sweep.find('run_bench.sh" "${RB_ARGS[@]}"')
    if level_at < 0:
        level_at = sweep.find("RB_ARGS=(")
    check("E sweep_bench 의 lite 게이트 종료가 GuideLLM 레벨 호출보다 **앞**에 있다",
          0 <= gate_at < level_at, "gate@%d level@%d — 게이트가 레벨 뒤면 교집합 상태가 다시 생긴다" % (gate_at, level_at))
    check("E sweep_bench 가 lite 실패를 fail-soft 로 접지 않는다(옛 'lite truncated:' 경로 부재)",
          'echo "lite truncated:' not in sweep)
    check("E sweep_bench 가 판정을 raw 단일 권위(read_lite_verdict)에서 읽고 게이트 불통과를 절삭 로그에 남긴다",
          "read_lite_verdict" in sweep and "lite gate failed" in sweep)
    lite_sh = read("lite_bench.sh") or ""
    check("E lite_bench 가 판정기(--judge)를 부르고 종료코드를 raw 판정에서 파생한다",
          "--judge" in lite_sh and 'exit "$LITE_RC"' in lite_sh and "server_failed) LITE_RC=7" in lite_sh)
    check("E 인증서 발행기가 lite 판정 pass 에서만 lite 등급 인증서를 낸다(β read_lite_verdict · γ index.lite)",
          'read_lite_verdict(raw)' in cert and 'lite.get("lite_verdict") != "pass"' in cert
          and "benchmark_mode: lite" in cert and "verdict: not_applicable" in cert)
    # E② 배선(2026-09-29 · plan_26092923_58_27) — ② 신호가 셀 상태·explorer cap 까지 닿는 사슬의 각 홉이 실재한다.
    #   판정 읽기는 모든 홉이 소유자 규칙(read_lite_verdict) 하나를 쓴다(값을 다시 판정하지 않는다).
    bsearch = read("broad_search.sh") or ""
    ccell = read("classify_cell.py") or ""
    skills = os.path.dirname(os.path.dirname(SDIR))

    def _sk(rel):
        try:
            with open(os.path.join(skills, rel), encoding="utf-8") as f:
                return f.read()
        except OSError:
            return ""
    cinit = _sk("terraforming_node/scripts/campaign_init.py")
    recipe = _sk("vllm-recipe-explorer/recipe.py")
    check("E② sweep_bench 가 native 서버 로그를 lite 레그에 넘긴다(없으면 native lite 는 늘 ①)",
          'LITE_PLANE_ARGS+=(--engine-log "$NATIVE_ELOG")' in sweep)
    check("E② classify_cell 이 이번 셀 lite raw 를 소유자 규칙으로 읽어 사유(lite_server_failed·lite_measurement_path_failed)로 옮긴다",
          "read_lite_verdict" in ccell and '"lite_server_failed"' in ccell and "--lite-raw" in ccell)
    check("E② broad_search 가 lite raw 를 분류기·셀 기록·writer 에 넘긴다",
          '_CLS_ARGS+=(--lite-raw "$LITE_RAW_CELL")' in bsearch and '_WARGS+=(--lite-raw "$LITE_RAW_CELL")' in bsearch
          and '"lite_verdict": cls.get("lite_verdict")' in bsearch)
    check("E② campaign_init writer 가 ② 에서 cap 차감(charges)·재발동 제안을 적고 결정 기록 문을 갖는다(cap 값은 적지 않는다)",
          "read_lite_verdict" in cinit and '"charges": charges' in cinit and '"status": "proposed"' in cinit
          and "--reentry-decide" in cinit)
    check("E② explorer 가 셀 상태 차감만큼 cap 을 줄이고 소진이면 트라이얼 없이 Model-C",
          "reconciliation" in recipe and "cap = cap_declared - _lite_spent" in recipe and "reconciliation_cap 소진" in recipe)
    check("E② lite_bench 는 native 에 --engine-log 를 요구하고 β raw 를 lite_publish/ 에 따로 둔다",
          'lite_publish/${CONFIG}_' in lite_sh and "native에는 --engine-log" in lite_sh)
    sys.path.insert(0, SDIR)
    try:
        import json as _json
        import lite_metrics as _lm
        check("E 판정 어휘·종료코드 = pass 0 · measurement_path_failed 6 · server_failed 7",
              tuple(_lm.LITE_VERDICTS) == ("pass", "measurement_path_failed", "server_failed")
              and _lm.LITE_EXIT == {"pass": 0, "measurement_path_failed": 6, "server_failed": 7})
        check("E 판정 읽기 규칙: 부재·어휘 밖 = None(호출부는 불통과로 읽는다)",
              _lm.read_lite_verdict(None) is None and _lm.read_lite_verdict({}) is None
              and _lm.read_lite_verdict({"lite_verdict": "ok"}) is None
              and _lm.read_lite_verdict({"lite_verdict": "pass"}) == "pass")
        fx = os.path.join(os.path.dirname(SDIR), "fixtures", "lite_raw_d1_connection_refused.json")
        with open(fx, encoding="utf-8") as f:
            d1 = _json.load(f)
        # 결과 파일 경로는 픽스처 디렉터리 상대(실측 발췌 — 연결 거부에도 rc 0 · completed 0 · failed N)
        for _k in ("bench_cold_json", "bench_warm_json"):
            d1[_k] = os.path.join(os.path.dirname(fx), d1[_k])
        check("E D1 픽스처는 실물 모양이다(rc 0 인데 결과 파일이 전부 실패를 말한다 — rc 로는 못 가른다)",
              d1.get("bench_cold_rc") == 0 and (_lm._load_json(d1["bench_cold_json"]) or {}).get("completed") == 0)
        v = _lm.judge_lite(d1, _lm.build(d1))
        check("E D1 연결거부 픽스처 → measurement_path_failed · exit 6(하네스 결함 · 서버 실패로 접지 않는다)",
              v["lite_verdict"] == "measurement_path_failed" and v["lite_exit"] == 6, v)
        d1b = dict(d1, client_plane_health="200")
        v2 = _lm.judge_lite(d1b, _lm.build(d1b))
        check("E 음성대조: 같은 출력이라도 클라이언트 평면이 닿으면 연결 오류 문구가 ② 로 접히지 않는다(① 유지)",
              v2["lite_verdict"] == "measurement_path_failed", v2)
        d1c = dict(d1, host_health_after="000")
        v3 = _lm.judge_lite(d1c, _lm.build(d1c))
        check("E 호스트 평면도 죽었으면(측정 중 서버 사망) → server_failed · exit 7", v3["lite_verdict"] == "server_failed"
              and v3["lite_exit"] == 7, v3)
    except Exception as e:   # noqa: BLE001 — 판정 불가는 FAIL 이다(통과로 접지 않는다)
        check("E 판정기 적재·픽스처 판정", False, repr(e))

    # A. lite 지표 키가 sweep_index 의 lite 블록에 실리는가
    for k in LITE_METRIC_KEYS:
        check("sweep_index.lite 가 '%s' 를 싣는다" % k,
              re.search(r'"%s":\s*_built\.get\("%s"\)' % (re.escape(k), re.escape(k)), sweep) is not None
              or re.search(r'"%s":\s*_built\.get' % re.escape(k), sweep) is not None,
              "lite_metrics 산출 키가 full 산출물에 전달되지 않으면 열 결손")
    check("sweep_bench 가 lite_metrics 를 재구현하지 않고 import 한다",
          "lite_metrics" in sweep and "spec_from_file_location" in sweep,
          "산정식을 복제하면 두 벌이 되어 어긋난다(D4↔D6 계열)")

    # B. report 가 lite 를 렌더하는가
    check("render_report 가 lite 블록을 렌더한다",
          'index.get("lite")' in report and "full ⊇ lite" in report)
    check("render_report 가 lite 결손을 경고한다",
          "lite 미포함" in report,
          "결손을 조용히 넘기면 비교 불가 사실이 숨는다")

    # C. 인증서가 lite 5종을 emit 하는가
    for f in CERT_LITE_FIELDS:
        check("인증서가 '%s' 를 emit 한다" % f, f in cert)

    ok = _emit(results)
    return 0 if ok else 1


def _emit(results):
    allok = True
    for name, good, detail in results:
        print("%s %s%s" % ("PASS" if good else "FAIL", name,
                           ("  — " + detail) if (detail and not good) else ""))
        allok = allok and bool(good)
    n = sum(1 for _, g, _ in results if g)
    print("\nfull ⊇ lite conformance: %d/%d %s" % (n, len(results), "PASS" if allok else "FAIL"))
    return allok


if __name__ == "__main__":
    sys.exit(main())
