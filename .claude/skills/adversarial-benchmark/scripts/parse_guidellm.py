#!/usr/bin/env python3
# parse_guidellm.py — GuideLLM `benchmarks.json` → 측정 M (결정론) (plan_26090415 §3.1 · CP4)
#
# CONTRACT: 출력 키는 `parse_bench.py` 와 **동일**하다 — 소비자(`verdict_rule.py`)가 도구를 몰라도
#   되게 하기 위해서다. 도구 축은 값이 아니라 **지문**으로만 실린다(bench_tool·bench_tool_version).
#   정본 디코드 지표도 그대로다: `1000 / median(time_per_output_token_ms)`.
#
# ★ spec(투기적 디코딩) 축은 GuideLLM 이 보고하지 않는다.
#   `vllm bench serve` 는 `spec_decode_acceptance_length` 를 싣지만 GuideLLM 0.7.3 의 산출물에는
#   그 축이 없다(실측: metrics 키 21종에 수용길이 없음). 그런데 `verdict_rule` 은 `spec_on` 으로
#   **물리 상한을 R_token 과 R_fp 사이에서 고른다** — 여기서 조용히 False 를 적으면 spec 이 켜진
#   모델의 상한이 낮게 잡히고, 그 결과는 "측정치가 물리 상한을 넘는" 거짓 판정이다.
#   그래서 이 파서는 spec 축을 **추측하지 않고 요구**한다. 둘 중 하나가 반드시 있어야 한다:
#     --accept-len-src <vllm bench serve JSON>  : 같은 스윕의 lite 레그에서 **승계**(측정 > 공식)
#     --spec-axis-absent                        : 축 부재를 **명시 선언**(그 사실이 출력에 남는다)
#   부재와 결측을 가르는 것이 이 요구의 전부다. `full = lite ∪ GuideLLM` 이므로 정상 경로에서는
#   lite 레그가 늘 있고, 승계가 기본 경로다.
#
# 사용: parse_guidellm.py --benchmarks-json PATH
#         (--accept-len-src PATH | --spec-axis-absent)
#         [--engine-log PATH] [--benchmark-index N] [--json]
# 종료: 0=산출 · 2=인자/스키마 오류
import argparse
import json
import re
import sys

# recipe parse_vllm_log · parse_bench 와 **동일 관례**(세 번째 자리이지만 값이 아니라 정규식이며,
# 세 파서가 같은 엔진 로그 줄을 읽는다 — 갈라지면 교차검증이 조용히 무력해진다).
_GEN_TPS_RE = re.compile(r"generation throughput:\s*([\d,]+(?:\.\d+)?)\s*tokens/s", re.IGNORECASE)

STAT_SET = "successful"   # errored/incomplete 는 지표에서 제외한다(집계 오염 방지). 개수는 따로 싣는다.


def _stat(metrics, path, field, stat_set=STAT_SET):
    """metrics[path][stat_set][field] 를 float 로. 없으면 None(호출부가 fail-closed 처리)."""
    node = metrics.get(path)
    if not isinstance(node, dict):
        return None
    bucket = node.get(stat_set)
    if not isinstance(bucket, dict):
        return None
    value = bucket.get(field)
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        return None
    return float(value)


def _count(metrics, key):
    totals = metrics.get("request_totals")
    if not isinstance(totals, dict):
        return None
    value = totals.get(key)
    return value if isinstance(value, int) and not isinstance(value, bool) else None


def _inv(ms):
    return round(1000.0 / ms, 2) if isinstance(ms, float) and ms > 0 else None


def build(doc, benchmark_index=0, engine_max=None, spec=None, max_error_rate=0.0):
    """GuideLLM report 문서 → parse_bench 와 동일한 측정 M. 순수 함수."""
    if not isinstance(doc, dict):
        raise ValueError("benchmarks.json 은 JSON 객체여야 한다")
    runs = doc.get("benchmarks")
    if not isinstance(runs, list) or not runs:
        raise ValueError("benchmarks[] 가 비었다 — 측정이 없다")
    if not 0 <= benchmark_index < len(runs):
        raise ValueError("--benchmark-index %d 가 범위 밖이다(benchmarks %d건)"
                         % (benchmark_index, len(runs)))
    run = runs[benchmark_index]
    metrics = run.get("metrics")
    if not isinstance(metrics, dict):
        raise ValueError("benchmarks[%d].metrics 가 없다" % benchmark_index)

    tpot_median = _stat(metrics, "time_per_output_token_ms", "median")
    tpot_mean = _stat(metrics, "time_per_output_token_ms", "mean")
    decode_tps = _inv(tpot_median)

    successful = _count(metrics, "successful")
    errored = _count(metrics, "errored")
    incomplete = _count(metrics, "incomplete")
    total = _count(metrics, "total")
    # incomplete 는 성공도 실패도 아닌 **미완결**이다. 측정 유효성 관점에서는 실패와 같이 취급하되
    # (완결되지 않은 요청이 있으면 그 런은 잘렸다) 개수는 따로 남긴다 — 접으면 원인을 잃는다.
    failed = None if errored is None or incomplete is None else errored + incomplete

    streams = (((run.get("config") or {}).get("profile") or {}).get("streams") or [])
    max_conc = streams[benchmark_index] if benchmark_index < len(streams) else (
        streams[0] if streams else None)
    if max_conc is None:
        max_conc = _stat(metrics, "request_concurrency", "median")

    agreement = None
    if engine_max and decode_tps:
        agreement = {"engine_max": engine_max, "client_decode_tps": decode_tps,
                     "ratio": round(engine_max / decode_tps, 2)}

    error_rate = (float(failed) / float(total)) if (failed is not None and total) else 0.0
    spec = spec or {}
    accept_len = spec.get("accept_len")
    meta = doc.get("metadata") if isinstance(doc.get("metadata"), dict) else {}
    tool_version = meta.get("guidellm_version")

    return {
        "decode_tps": decode_tps,
        "decode_tps_mean": _inv(tpot_mean),
        "output_throughput": _stat(metrics, "output_tokens_per_second", "mean"),
        "total_token_throughput": _stat(metrics, "tokens_per_second", "mean"),
        "ttft_ms_median": _stat(metrics, "time_to_first_token_ms", "median"),
        "ttft_ms_mean": _stat(metrics, "time_to_first_token_ms", "mean"),
        "itl_ms_median": _stat(metrics, "inter_token_latency_ms", "median"),
        "tpot_ms_median": tpot_median,
        "accept_len": accept_len,
        "spec_on": bool(accept_len and accept_len > 1.05),
        "completed": successful,
        "failed": failed,
        "max_concurrency": max_conc,
        "num_prompts": total,
        "engine_gen_throughput_max": engine_max,
        "client_engine_agreement": agreement,
        # ★ 오류 허용치는 **선언에서만** 온다(기본 0 = 엄격 · plan_26090419 §8.9).
        #   왜 0 이 아닌 값을 받을 수 있게 했나: GuideLLM 의 `errored` 는 서버 오류만이 아니라
        #   **클라이언트측 스트림 파싱 실패**도 담는다(실측: JSONDecodeError, 잘린 SSE 청크 1건).
        #   그런 1건 때문에 통계가 건전한 측정(성공 17건 · 엔진로그와 ratio 1.01)을 통째로 버리면
        #   지도에 구멍이 생기고, 그 구멍은 "이 레시피는 측정 불가"로 읽힌다 — 거짓이다.
        #   그렇다고 조용히 삼키면 진짜 서버 오류가 묻힌다. 그래서 **삼키지 않고 선언하게** 한다:
        #   허용치는 호출자가 근거와 함께 넘기고, 실제 오류율은 산출물이 항상 싣는다.
        "error_rate": error_rate,
        "max_error_rate_declared": max_error_rate,
        "measurement_ok": bool(successful and decode_tps is not None
                               and error_rate <= max_error_rate),
        # ── 출처 표시(헌법 §결정론 규율). 값 옆에 어디서 왔는지를 둔다. ──
        "bench_tool": "guidellm",
        "bench_tool_version": tool_version,
        "bench_tool_version_source": ("measured(benchmarks.json metadata.guidellm_version)"
                                      if tool_version else "unavailable"),
        "spec_axis_source": spec.get("source"),
        # 측정 조건 자체보고 — 같은 열 이름 아래 다른 산정식이 숨지 않게 한다.
        "stat_set": STAT_SET,
        "errored": errored,
        "incomplete": incomplete,
    }


def _load(path, label):
    try:
        with open(path, encoding="utf-8") as handle:
            return json.load(handle)
    except (OSError, ValueError) as exc:
        sys.stderr.write("[parse_guidellm] ERROR %s: %s\n" % (label, exc))
        sys.exit(2)


def _engine_max(path):
    try:
        with open(path, encoding="utf-8", errors="replace") as handle:
            values = [float(m.replace(",", "")) for m in _GEN_TPS_RE.findall(handle.read())]
    except OSError as exc:
        sys.stderr.write("[parse_guidellm] WARN engine-log: %s\n" % exc)
        return None
    return round(max(values), 2) if values else None


def _self_test():
    failures = []

    def check(name, condition, detail=""):
        if condition:
            print("  [PASS] %s" % name)
        else:
            print("  [FAIL] %s %s" % (name, detail)); failures.append(name)

    def stat(median, mean=None):
        return {"successful": {"median": median, "mean": mean if mean is not None else median}}

    doc = {
        "metadata": {"guidellm_version": "0.7.3"},
        "benchmarks": [{
            "config": {"profile": {"kind": "concurrent", "streams": [4]}},
            "metrics": {
                "time_per_output_token_ms": stat(25.0, 20.0),
                "time_to_first_token_ms": stat(300.0, 310.0),
                "inter_token_latency_ms": stat(24.0),
                "output_tokens_per_second": stat(38.0, 39.0),
                "tokens_per_second": stat(160.0, 161.0),
                "request_concurrency": stat(4.0),
                "request_totals": {"successful": 16, "errored": 0, "incomplete": 0, "total": 16},
            },
        }],
    }

    out = build(doc, spec={"accept_len": None, "source": "declared-absent"})
    check("G1 정본 지표 = 1000/median(TPOT)", out["decode_tps"] == 40.0 and out["tpot_ms_median"] == 25.0)
    check("G2 mean 도 median 과 같은 산정식", out["decode_tps_mean"] == 50.0)
    check("G3 parse_bench 계약 키를 전부 낸다",
          {"decode_tps", "ttft_ms_median", "itl_ms_median", "completed", "failed",
           "max_concurrency", "num_prompts", "measurement_ok", "spec_on",
           "accept_len"} <= set(out))
    check("G4 도구 지문은 산출물에서 실측한다",
          out["bench_tool"] == "guidellm" and out["bench_tool_version"] == "0.7.3"
          and out["bench_tool_version_source"].startswith("measured("))
    check("G5 동시성은 profile.streams 에서 읽는다", out["max_concurrency"] == 4)
    check("G6 spec 축 부재는 출처로 남는다",
          out["accept_len"] is None and out["spec_on"] is False
          and out["spec_axis_source"] == "declared-absent")

    out2 = build(doc, spec={"accept_len": 1.85, "source": "inherited(lite vllm bench serve)"})
    check("G7 lite 승계 accept_len 이 spec_on 을 켠다",
          out2["spec_on"] is True and out2["accept_len"] == 1.85
          and out2["spec_axis_source"].startswith("inherited("))

    bad = json.loads(json.dumps(doc))
    bad["benchmarks"][0]["metrics"]["request_totals"]["incomplete"] = 2
    out3 = build(bad, spec={"accept_len": None, "source": "declared-absent"})
    check("G8 incomplete 는 실패로 세되 개수를 따로 남긴다",
          out3["failed"] == 2 and out3["incomplete"] == 2 and out3["errored"] == 0
          and out3["measurement_ok"] is False)

    zero = json.loads(json.dumps(doc))
    zero["benchmarks"][0]["metrics"]["time_per_output_token_ms"] = stat(0.0)
    out4 = build(zero, spec={"accept_len": None, "source": "declared-absent"})
    check("G9 TPOT 0 은 무한대가 아니라 None(침묵 나눗셈 ✗)",
          out4["decode_tps"] is None and out4["measurement_ok"] is False)

    for broken, fragment in (({"benchmarks": []}, "비었다"),
                             ({"benchmarks": [{}]}, "metrics")):
        try:
            build(broken, spec={"source": "declared-absent"})
        except ValueError as exc:
            check("G10 불량 문서 거부(%s)" % fragment, fragment in str(exc), "(실제 %r)" % str(exc))
        else:
            check("G10 불량 문서 거부(%s)" % fragment, False, "(예외가 나지 않았다)")

    # 실측 산출물이 있으면 **그것으로도** 돈다 — 픽스처가 실물보다 좁아지지 않게.
    import os
    sample = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                          "fixtures", "guidellm_benchmarks_sample.json")
    if os.path.isfile(sample):
        with open(sample, encoding="utf-8") as handle:
            real = build(json.load(handle), spec={"source": "declared-absent"})
        check("G11 실측 산출물 픽스처가 계약을 만족한다",
              real["decode_tps"] is not None and real["measurement_ok"] is True
              and real["bench_tool_version_source"].startswith("measured("))
    else:
        check("G11 실측 산출물 픽스처 존재", False, "(fixtures/guidellm_benchmarks_sample.json 부재)")

    err1 = json.loads(json.dumps(doc))
    err1["benchmarks"][0]["metrics"]["request_totals"] = {"successful": 15, "errored": 1,
                                                          "incomplete": 0, "total": 16}
    o_strict = build(err1, spec={"source": "declared-absent"})
    o_decl = build(err1, spec={"source": "declared-absent"}, max_error_rate=0.0625)
    check("G12 기본은 엄격 — 오류 1건이면 measurement_ok=False",
          o_strict["measurement_ok"] is False and abs(o_strict["error_rate"] - 0.0625) < 1e-9)
    check("G13 선언한 허용치 안이면 통과하되 오류율을 **항상 싣는다**",
          o_decl["measurement_ok"] is True and o_decl["error_rate"] > 0
          and o_decl["max_error_rate_declared"] == 0.0625)
    o_over = build(err1, spec={"source": "declared-absent"}, max_error_rate=0.01)
    check("G14 ★음성대조 선언 허용치를 넘으면 여전히 False", o_over["measurement_ok"] is False)

    if failures:
        sys.stderr.write("[parse_guidellm --self-test] FAIL %d 건: %s\n" % (len(failures), failures))
        return 1
    print("[parse_guidellm --self-test] OK — G1~G14 전부 통과")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description="GuideLLM benchmarks.json → 측정 M(parse_bench 동일 계약)")
    ap.add_argument("--benchmarks-json", help="guidellm run --output kind=json 산출물")
    ap.add_argument("--engine-log", help="docker logs 캡처(교차검증, 선택)")
    ap.add_argument("--benchmark-index", type=int, default=0)
    ap.add_argument("--accept-len-src",
                    help="같은 스윕 lite 레그의 `vllm bench serve` JSON — spec 수용길이를 승계한다")
    ap.add_argument("--spec-axis-absent", action="store_true",
                    help="spec 축 부재를 명시 선언한다(승계원이 없을 때. 그 사실이 출력에 남는다)")
    ap.add_argument("--max-error-rate", type=float, default=0.0,
                    help="허용 오류율(기본 0.0 = 엄격). 0 이 아닌 값을 쓰려면 호출자가 근거를 갖고 "
                         "넘겨야 한다 — 실제 오류율은 산출물이 항상 error_rate 로 싣는다(삼키지 않는다).")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv)

    if args.self_test:
        return _self_test()
    if not args.benchmarks_json:
        sys.stderr.write("[parse_guidellm] ERROR --benchmarks-json 은 필수다\n")
        return 2
    if bool(args.accept_len_src) == bool(args.spec_axis_absent):
        sys.stderr.write(
            "[parse_guidellm] ERROR spec 축을 정하라 — --accept-len-src(lite 레그 승계) 또는 "
            "--spec-axis-absent(부재 명시) 중 **정확히 하나**.\n"
            "  GuideLLM 은 수용길이를 보고하지 않는데 verdict_rule 은 spec_on 으로 물리 상한을\n"
            "  R_token/R_fp 중에서 고른다. 조용히 False 를 적으면 spec 이 켜진 모델의 상한이 낮게\n"
            "  잡혀 거짓 판정이 된다 — 부재와 결측을 가른다.\n")
        return 2

    spec = {"accept_len": None, "source": "declared-absent"}
    if args.accept_len_src:
        src = _load(args.accept_len_src, "--accept-len-src")
        value = src.get("spec_decode_acceptance_length") if isinstance(src, dict) else None
        if value is None and isinstance(src, dict):
            value = src.get("accept_len")
        if value is not None and not isinstance(value, (int, float)):
            sys.stderr.write("[parse_guidellm] ERROR --accept-len-src 의 수용길이가 수가 아니다: %r\n" % value)
            return 2
        spec = {"accept_len": float(value) if value is not None else None,
                "source": "inherited(%s)" % args.accept_len_src}

    engine_max = _engine_max(args.engine_log) if args.engine_log else None
    doc = _load(args.benchmarks_json, "--benchmarks-json")
    try:
        out = build(doc, args.benchmark_index, engine_max, spec, args.max_error_rate)
    except ValueError as exc:
        sys.stderr.write("[parse_guidellm] ERROR %s\n" % exc)
        return 2
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
