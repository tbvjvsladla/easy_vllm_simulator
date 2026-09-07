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


def _pct(metrics, path, pct="p99", stat_set=STAT_SET):
    """metrics[path][stat_set]["percentiles"][pct] → float. 없으면 None(호출부가 fail-closed).

    ★ 2026-09-06 신설(plan_26090616 H): 종전 산출에는 중앙값·평균만 있었다. 그런데 hint 를 읽는
    쪽이 알아야 하는 것은 "이 레시피가 얼마나 빠른가" 만이 아니라 **얼마나 고르게 빠른가** 다 —
    꼬리(p99)와 산포(std)가 없으면 같은 중앙값을 가진 두 레시피가 구분되지 않는다. 값은 이미
    GuideLLM 산출에 들어 있었고 읽는 코드만 없었다.
    """
    node = metrics.get(path)
    bucket = node.get(stat_set) if isinstance(node, dict) else None
    pcts = bucket.get("percentiles") if isinstance(bucket, dict) else None
    value = pcts.get(pct) if isinstance(pcts, dict) else None
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



# ── 오류 분류: 도구 경계 대 서버 오류 (2026-09-06) ───────────────────────────
# 왜: GuideLLM 의 `errored` 는 서버가 낸 오류만이 아니다. 클라이언트 스트림 파서가 거부한
#   요청도 같은 통에 들어간다. 실측(2026-09-06 캠페인 ⑦ a0): gpt-oss harmony 가 자연 종료
#   토큰을 내보내자 파서가
#     ValueError('Streaming response returned an error: Unexpected token 200002 while
#                 expecting start token 200006')
#   로 4건을 버렸다 — 그 요청들은 **출력 토큰을 195~210개 정상 생성**했고, 성공 14건의 측정은
#   엔진 로그와 ratio 1.02 로 일치했다. 서버는 멀쩡했다.
#   종전 처방은 `--max-error-rate` 를 올리는 것뿐이었는데, 그러면 **진짜 서버 실패도 같이
#   통과한다**. 허용치를 올리는 것은 경계를 지우는 것이지 가르는 것이 아니다.
# 처방: 도구 경계를 **양성으로 지목**하고, 나머지는 전부 서버 오류로 센다(fail-closed).
#   판정은 **두 조건 동시 충족**이다 — ① 닫힌 패턴 목록에 걸리고 ② 그 요청이 실제로 출력
#   토큰을 냈다. 패턴만 보면 "Streaming response" 를 품은 진짜 장애가 면제되고, 토큰 수만
#   보면 아무 오류나 면제된다.
_TOOL_BOUNDARY_ERROR_PATTERNS = (
    "while expecting start token",   # harmony 채널 토큰 경계(gpt-oss 계열)
    "unexpected token",              # 동상 — 표현 변화 대비
    "jsondecodeerror",               # 잘린 SSE 청크
    "expecting value",               # 동상
)


def classify_errors(run, benchmark_index=0, server_alive_at_bench_end=None):
    """errored[] 를 (도구경계 n, 서버 n, 미분류 사유 표본) 으로 가른다.

    `requests.errored` 가 없는 산출물(옛 포맷·요약본)에서는 **가르지 않는다** — 모르는 것을
    면제로 접으면 그 순간 게이트가 fail-open 이 된다. 그 경우 전량을 서버 오류로 돌린다.

    `server_alive_at_bench_end` (2026-09-07 · 유예 결함 ②): 벤치가 끝난 시점의 서버 생존 관측.
    **False 면 도구경계 면제를 통째로 무효화**한다 — 엔진이 죽어 잘린 스트림과 도구가 끊은
    스트림은 같은 예외(`RemoteProtocolError`)로 나오고, 죽은 서버 쪽에서는 그 예외가 도구 탓이
    아니다. None(관측 없음)이면 종전대로 분류한다: 모르는 것을 차단으로도 면제로도 접지 않고,
    대신 산출물이 그 사실을 `error_split_source` 로 밝힌다.
    """
    try:
        errored = run["benchmarks"][benchmark_index]["requests"]["errored"]
    except (KeyError, IndexError, TypeError):
        return None
    boundary, server, samples = 0, 0, []
    for rec in errored if isinstance(errored, list) else []:
        info = rec.get("info") or {}
        msg = str(info.get("error") or "")
        produced = rec.get("output_tokens") or (rec.get("output_metrics") or {}).get("text_tokens")
        low = msg.lower()
        hit = any(pat in low for pat in _TOOL_BOUNDARY_ERROR_PATTERNS)
        if hit and produced and server_alive_at_bench_end is not False:
            boundary += 1
        else:
            server += 1
            if msg and len(samples) < 3:
                samples.append(msg[:200])
    return {"tool_boundary": boundary, "server": server, "unclassified_samples": samples,
            "server_alive_at_bench_end": server_alive_at_bench_end,
            "boundary_exemption": ("disabled(server dead at bench end)"
                                   if server_alive_at_bench_end is False else
                                   ("enabled(server alive)" if server_alive_at_bench_end is True
                                    else "enabled(liveness unobserved)"))}


def build(doc, benchmark_index=0, engine_max=None, spec=None, max_error_rate=0.0,
          server_alive_at_bench_end=None):
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
    # 도구 경계는 서버 오류가 아니다 — 가르되 **삼키지 않는다**(둘 다 싣는다).
    cls = classify_errors(doc, benchmark_index,
                          server_alive_at_bench_end=server_alive_at_bench_end)
    if cls is None:
        server_failed = failed
        error_split_source = "unavailable(requests.errored 부재 — 전량을 서버 오류로 센다)"
    else:
        # ⚠ 목록(requests.errored[])이 집계(request_totals.errored)보다 길 수 있다 — warmup 구간
        #   요청이 집계에서만 빠지기 때문이다(실측: 목록 4 대 집계 3). 그래서 이 합은 집계보다
        #   **크게 나올 수 있고**, 그 방향은 안전하다(서버 오류를 더 세는 쪽). 반대로 맞추려고
        #   집계 값으로 자르면 진짜 서버 오류 1건이 잘려 나갈 수 있다.
        server_failed = cls["server"] + (incomplete or 0)
        error_split_source = "measured(requests.errored[].info.error + output_tokens)"
    server_error_rate = ((float(server_failed) / float(total))
                         if (server_failed is not None and total) else 0.0)
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
        # ── 꼬리·산포 축(2026-09-06 · plan_26090616 H) ──
        "ttft_ms_p99": _pct(metrics, "time_to_first_token_ms"),
        "ttft_ms_std": _stat(metrics, "time_to_first_token_ms", "std_dev"),
        "itl_ms_p99": _pct(metrics, "inter_token_latency_ms"),
        "itl_ms_std": _stat(metrics, "inter_token_latency_ms", "std_dev"),
        "tpot_ms_p99": _pct(metrics, "time_per_output_token_ms"),
        "tpot_ms_std": _stat(metrics, "time_per_output_token_ms", "std_dev"),
        "request_latency_ms_p99": _pct(metrics, "request_latency"),
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
        # ★ 판정은 **서버 오류율**로 한다(2026-09-06). 전체 오류율은 그대로 싣되 게이트를
        #   흐리지 않는다 — 도구 경계 때문에 허용치를 올리면 진짜 장애까지 통과한다.
        "server_error_rate": server_error_rate,
        "tool_boundary_errors": None if cls is None else cls["tool_boundary"],
        "server_errors": None if cls is None else cls["server"],
        "unclassified_error_samples": None if cls is None else cls["unclassified_samples"],
        "error_split_source": error_split_source,
        # 벤치 종료 시 서버 생존 관측(2026-09-07 · 유예 결함 ②). None = 관측 없음 — 그 사실도 싣는다.
        "server_alive_at_bench_end": (cls or {}).get("server_alive_at_bench_end"),
        "boundary_exemption": (cls or {}).get("boundary_exemption", "n/a(분류 불가)"),
        "max_error_rate_declared": max_error_rate,
        "measurement_ok": bool(successful and decode_tps is not None
                               and server_error_rate <= max_error_rate),
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
    # ── G15~G19 오류 분류(2026-09-06) ───────────────────────────────────
    _boundary = {"info": {"error": "ValueError('Streaming response returned an error: "
                           "Unexpected token 200002 while expecting start token 200006')"},
                 "output_tokens": 203}
    _server = {"info": {"error": "HTTPStatusError: 500 Internal Server Error"},
               "output_tokens": 0}
    # 패턴은 맞지만 토큰을 하나도 못 낸 것 — 경계가 아니라 실패다(두 조건 동시 충족).
    _fake = {"info": {"error": "Unexpected token while expecting start token"},
             "output_tokens": 0}
    import copy as _copy
    _mk = lambda errs: _copy.deepcopy(err1)
    _d = _copy.deepcopy(err1)
    _d["benchmarks"][0]["requests"] = {"errored": [_boundary, _boundary, _boundary]}
    _d["benchmarks"][0]["metrics"]["request_totals"] = {"successful": 13, "errored": 3,
                                                        "incomplete": 0, "total": 16}
    _o = build(_d, spec={"source": "declared-absent"}, max_error_rate=0.0)
    check("G15 ★도구 경계 3건은 엄격 허용치에서도 measurement_ok 를 깨지 않는다",
        _o["measurement_ok"] is True and _o["tool_boundary_errors"] == 3
        and _o["server_errors"] == 0 and abs(_o["error_rate"] - 0.1875) < 1e-9
        and _o["server_error_rate"] == 0.0)
    _d2 = _copy.deepcopy(_d)
    _d2["benchmarks"][0]["requests"] = {"errored": [_boundary, _server, _boundary]}
    _o2 = build(_d2, spec={"source": "declared-absent"}, max_error_rate=0.0)
    check("G16 ★음성대조 서버 오류가 섞이면 엄격 허용치에서 False",
        _o2["measurement_ok"] is False and _o2["server_errors"] == 1
        and _o2["tool_boundary_errors"] == 2)
    _d3 = _copy.deepcopy(_d)
    _d3["benchmarks"][0]["requests"] = {"errored": [_fake, _fake, _fake]}
    _o3 = build(_d3, spec={"source": "declared-absent"}, max_error_rate=0.0)
    check("G17 ★패턴만 맞고 토큰 0 이면 경계가 아니다(두 조건 동시 충족)",
        _o3["measurement_ok"] is False and _o3["server_errors"] == 3
        and _o3["tool_boundary_errors"] == 0)
    _d4 = _copy.deepcopy(_d)
    _d4["benchmarks"][0].pop("requests", None)
    _o4 = build(_d4, spec={"source": "declared-absent"}, max_error_rate=0.0)
    check("G18 ★requests 부재면 가르지 않고 전량 서버 오류(fail-closed)",
        _o4["measurement_ok"] is False and _o4["tool_boundary_errors"] is None
        and "unavailable" in _o4["error_split_source"])
    check("G19 미분류 사유를 표본으로 남긴다(삼키지 않는다)",
        _o2["unclassified_error_samples"] and "500" in _o2["unclassified_error_samples"][0])

    # ── G20~G22 벤치 종료 시 서버 생존(2026-09-07 · 유예 결함 ②) ──────────────────────────
    #   같은 예외가 두 원인에서 나온다: 도구가 스트림을 끊었다 vs 엔진이 죽어 잘렸다.
    #   절단선은 "벤치가 끝난 시점에 서버가 살아 있었는가" 이고, 그 관측이 없으면 역방향
    #   fail-open 이 열린다(엔진 사망 중 잘린 SSE 가 도구 경계로 면제된다).
    _g = build(_d, spec={"source": "declared-absent"}, max_error_rate=0.0,
               server_alive_at_bench_end=True)
    check("G20 서버 생존이면 도구 경계 면제가 유지된다",
        _g["tool_boundary_errors"] == 3 and _g["boundary_exemption"].startswith("enabled"))
    _gd = build(_d, spec={"source": "declared-absent"}, max_error_rate=0.0,
                server_alive_at_bench_end=False)
    check("G21 ★음성대조 서버가 죽었으면 면제 무효 — 전량 서버 오류로 센다",
        _gd["tool_boundary_errors"] == 0
        and _gd["server_error_rate"] > _g["server_error_rate"]
        and _gd["boundary_exemption"].startswith("disabled"))
    _gn = build(_d, spec={"source": "declared-absent"}, max_error_rate=0.0)
    check("G22 관측이 없으면 모름으로 남긴다(False 로 접지 않는다)",
        _gn["server_alive_at_bench_end"] is None
        and _gn["tool_boundary_errors"] == 3
        and "unobserved" in _gn["boundary_exemption"])

    print("[parse_guidellm --self-test] OK — G1~G22 전부 통과")
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
    ap.add_argument("--post-health-json",
                    help="run_bench 가 벤치 직후 남긴 post_health_<cfg>.json. "
                         "server_alive_at_bench_end=false 면 도구경계 면제가 무효화된다(유예 결함 ②).")
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
    # 벤치 종료 시 서버 생존 — **관측 파일이 있을 때만** 읽는다. 파일이 없으면 None(모름)이고,
    # 모름을 False 로도 True 로도 접지 않는다(부재와 결측을 가른다).
    alive = None
    if args.post_health_json:
        ph = _load(args.post_health_json, "--post-health-json")
        v = ph.get("server_alive_at_bench_end") if isinstance(ph, dict) else None
        if not isinstance(v, bool):
            sys.stderr.write("[parse_guidellm] ERROR --post-health-json 에 "
                             "server_alive_at_bench_end(boolean) 가 없다: %r\n" % v)
            return 2
        alive = v
    doc = _load(args.benchmarks_json, "--benchmarks-json")
    try:
        out = build(doc, args.benchmark_index, engine_max, spec, args.max_error_rate,
                    server_alive_at_bench_end=alive)
    except ValueError as exc:
        sys.stderr.write("[parse_guidellm] ERROR %s\n" % exc)
        return 2
    print(json.dumps(out, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
