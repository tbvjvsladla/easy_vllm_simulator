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
    check("sweep_bench 가 lite 실패를 침묵시키지 않는다",
          "lite truncated" in sweep,
          "fail-soft 는 허용하되 절삭 로그에 남아야 한다")

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
