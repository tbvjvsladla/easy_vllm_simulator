# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T04:47:50Z | sweep_index.generated_utc(output/multi/benchlog/sweep_i5-a-kern) |
| `accept_len` | 2.026315789473684 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md)) |
| `decode_tps_conc1` | 35.87 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 1.53 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T04:47:50Z | — |
| `lite.cold_ttft_ms` | 2216.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.engine_kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.engine_kv_tokens` | 2571042 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.engine_reserved_total_gib` | 82.64 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.engine_weights_gib` | 74.64 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.gen_tps` | 41.88 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `lite.kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-a-kern) |
| `primary_source` | expected_achievable(roofline×MBU) | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 1.8 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 19.928 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_i5-a-kern · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_i5-a-kern · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_i5-a-kern · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.026315789473684 | 16 | 35.87 | 36.25 | 0 | 23.535848131366805 | 1 | true | 36.60346402097427 | 27.880261652171612 | 1104.388952255249 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 35.87 | 36.60 | 187.31 | 1104.39 | 23.54 | 16/0 |

> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. 곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것).

_절삭된 레벨 없음(요청 전 레벨 완주)._

## 3.3 측정 구성
<!-- FACT:measurement_config -->
| 키 | 값 |
|---|---|
| `bench_mode` | full |
| `bench_mode_kind` | full |
| `downgrade_reason` | 미기재 |
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `repeats` | 3 |
| `repeats_completed` | 3 |
| `source` | bench_report(bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/multi/benchlog/sweep_i5-a-kern/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `5d12e3a0a7bc` 2026-10-06T06:10:02Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@5d12e3a0a7bc` | `git show 5d12e3a0a7bc46cf7b4ad84790c24ba2fdd8ce0c:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_i5-a-kern/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 수치는 `sweep_bench.sh`(측정 시각 이전 마지막 커밋 `dacb77675e63`)가 프리셋 `default_preset` 의 측정 형상으로 돌린 full-bench 다 — 먼저 lite 레그(lite_bench.sh · 게이트)를 1회 돌리고, 통과하면 레벨마다 `run_bench.sh` 로 GuideLLM 을 반복 3회 부른다. 실행 명령 원문은 셀 판정 문서에 남아 있다:

> [원문] testlog_26101017_08_29_knob프리셋_E2E_셀M2_i5_a_kern_판정 §2. 측정 — 프리셋 기반 full-bench
> bash .claude/skills/adversarial-benchmark/scripts/sweep_bench.sh i5-a-kern --topology multi --tool guidellm --levels 1 --knob-reason levels=prior_cell_shape --backend openai-chat --knob-reason backend=prior_cell_shape --bench-budget-mib 8192 --knob-reason bench_budget_mib=host_budget

**GuideLLM 레그(판정점 · 레벨 1 × 반복 3)** — 그 판본 `run_bench.sh` 가 GuideLLM 컨테이너를 이렇게 부른다(입출력 길이 · 동시성 · 요청 수는 sweep_bench 가 넘긴 프리셋 해소값):

> [원문] run_bench.sh@dacb77675e63 §L398-403
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

warmup 은 요청 수가 아니라 비율로 넘어간다 — 같은 원문이 총 요청을 warmup 만큼 늘린다:

> [원문] run_bench.sh@dacb77675e63 §L327-329
>   local TOTAL_REQ WARM_FRAC
>   TOTAL_REQ=$(( NPROMPTS + WARMUPS ))
>   WARM_FRAC="$(python3 -c "print(round($WARMUPS/max($TOTAL_REQ,1), 4))")"

**lite 레그(게이트 · 판정점 아님)** — 그 판본 sweep_bench 가 `lite_bench.sh` 를 부르고(§L356), 그 판본 lite_bench(`5d12e3a0a7bc`)가 서빙 컨테이너 안에서 `vllm bench serve` 를 cold 1회 · warm burst 1회 부른다:

> [원문] lite_bench.sh@5d12e3a0a7bc §L172-177
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

**조건 목록**
- 도구 · 버전: GuideLLM `0.7.3`(03 §3.3 · `bench_tool_version_source` = benchmarks.json 메타 관측 · 컨테이너 `ghcr.io/vllm-project/guidellm:v0.7.3` — 측정 메타의 이미지 digest 관측) · 래퍼 `sweep_bench.sh@dacb77675e63` → `run_bench.sh@dacb77675e63`. lite 레그는 서빙 컨테이너 안 `vllm bench serve`(래퍼 `lite_bench.sh@5d12e3a0a7bc`).
- 측정 형상의 출처: 입력 1024 · 출력 256 · 요청 16 · warmup 2 · 반복 3 = 프리셋 기본(`preset · preset_default`) · 레벨 `[1]` · 백엔드 `openai-chat` · 벤치 예산 8192 MiB = Agent 선택(근거 코드 `prior_cell_shape` · `prior_cell_shape` · `host_budget`) — 셀 판정 문서 §2 · sweep_index `bench_knobs`. 해소 순서는 `cli_pin > campaign > agent > preset` 이고 활성 캠페인이 없었다(`_bootstrap`).
- 백엔드 · 엔드포인트: `openai-chat` · `/v1/chat/completions`(측정 메타 `bench_endpoint`).
- 입력 / 출력 길이: GuideLLM synthetic 선언 1024 → 256 토큰 · `ignore_eos` true(도구 원문 §L399 · §L401). 서버가 받은 요청당 입력은 chat 템플릿을 포함해 1054 토큰 · 출력 256 토큰이었다(m2_sweep.log Request Token Statistics 중앙값 — 실측). lite 레그는 random 512 → 128 · `--random-range-ratio 0` · `--ignore-eos`(도구 원문 §L175-176).
- 샘플링 temperature · seed: GuideLLM 호출 원문에 인자가 없다 — 도구 기본값이 쓰였다(값은 이 원문으로 정해지지 않는다 · 단정하지 않는다). lite 레그도 원문에 temperature · seed 인자가 없다.
- 동시성 · 요청 수: GuideLLM `kind=concurrent,streams=1` · 요청 = 16 + warmup 2 = 18(§L328) · 집계 완료 16/0(03 §3.2 표). lite 는 `--max-concurrency 1` · cold 요청 1 · warm 요청 3.
- warmup: GuideLLM 은 요청 2건이 아니라 비율로 넘어간다(§L329) — 실행 로그가 `'warmup': {'percent': 0.1111, … 'mode': 'prefer_duration'}` 와 run1 'Warm Sec 14.3' 을 기록했다(m2_sweep.log) · 즉 지속 시간의 11.11 % 가 warmup 이었다 · lite cold 0 · warm 1.
- 반복: 3 · 완주 3(03 §3.3) · 종류 `warm-rerun`(같은 running serve) · 판정점 = 대표 run(첫 완주 run) 35.87 · 나머지 41.04 · 41.22(리포트 반복 축 표 · 밴드 13.59 %).
- warm/cold: 판정점 run1 은 lite 레그와 GuideLLM warmup(지속 시간 11.11 %) 뒤인데도 둘째 · 셋째 run 보다 낮다(Q10). lite gen 41.88 t/s 는 warm burst 의 median TPOT 산정 · cold-start TTFT 2216 ms 는 cold 단일 요청(03 §3.1).
- spec decode: 켜짐 — DSpark k=3 greedy(서빙 yaml · V20). 판정 입력 accept_len 2.0263…(03 §3.1)은 lite warm 레그의 값을 승계했다(02 §2.5) — GuideLLM 레그 자신의 acceptance 는 색인에 없다.
- 측정 노드: cluster(측정 메타 `measured_node` — 쌍이 하나의 측정 정체성) · 클라이언트는 main 호스트의 GuideLLM 컨테이너(메모리 상한 8192 MiB · §L394 `--memory`).
- 측정 시각(UTC): lite 레그 시작 2026-10-10T04:40:43Z(lite_raw `measured_utc` · lite_bench.sh@5d12e3a0a7bc §L190 가 cold 실행 직전에 찍는다) → GuideLLM run1 시작 04:41:27Z → sweep_index 조립 04:47:50Z(03 §3.1 `measured_utc`) · 약 7분 07초. 04:41:02Z 는 lite cold 결과 JSON 의 date 다.
- 서버 상태: READY(04:39:47Z 서버 기동) 직후 약 1분 뒤 측정을 시작했다(01 §1.4) — 10-05 기동 3 처럼 긴 유휴 뒤가 아니다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 측정에는 인증서가 없다(weak PASS — 03 §3.3 비발행 판정). 그래서 강한 일치 키와 소프트 지문은 인증서 대신 같은 키 집합을 싣는 측정 메타(sweep_index `meta` · 리포트 측정 환경 스냅샷)에서 읽는다 — 강한 키: model `deepseek-v4.1-flash-exl3-2.0bpw-sfxnz` · gpu_model NVIDIA GB10 · vllm_version 0.30.0 · quantization exl3 · topology multi · tensor_parallel_size 2 · 소프트 지문: driver_version 580.173.02(선언 — M5) · cuda_version 132 · image_tag `easy-vllm:0.30.0-cu133-aarch64-source-sm12x` · image_digest `sha256:e90626f8…` · max_model_len 1048576 · max_num_seqs 2 · kv_cache_memory_bytes 8589934592 · kv_cache_dtype fp8 · gpu_memory_utilization 0.80 · moe_backend deepgemm_mxfp4(엔진 로그 파생 라벨) · enforce_eager false · attention_backend NA · ple_mode resident(기본값 파생) · bench_tool guidellm.

**같은 셀의 이전 측정 — 10-06 기동 4 Full(bench_report_26100613) · 10-05 기동 3 Full(bench_report_26100516_41_31).** 둘 다 같은 도구(GuideLLM 0.7.3) · 같은 입출력(1024/256) · 레벨당 16 요청 · 같은 이미지 태그 · 같은 서빙 설정이다. 요청당 토큰 구성도 같다 — 레벨 1 의 총 tok/s ÷ 출력 tok/s 가 이 측정 187.31 ÷ 36.60 · 기동 4 177.28 ÷ 34.64 · 기동 3 206.65 ÷ 40.38 로 셋 다 약 5.12 다. 이전 측정의 레벨 전부를 이 측정 옆에 둔다(값은 원천의 글자 그대로 · 비 = 이 측정 ÷ 이전 · 소수 셋째 자리 반올림 · 이 측정에 없는 레벨은 `N/A`):

| 동시성 | 이 측정(10-10) | 기동 4(10-06) | 비 | 기동 3(10-05 · Full 근거 제외) | 비 |
|---|---|---|---|---|---|
| 1 ★판정점(대표 run) | 35.87 | 34.41 | 1.042 | 39.68 | 0.904 |
| 1 run 순 | 35.87 / 41.04 / 41.22 | 34.41 / 40.83 / 39.80 | — | 39.68 / 40.64 / 39.94 | — |
| 1 재현 밴드 % | 13.59 | 16.74 | — | 2.39 | — |
| 1 TTFT p50(ms · 대표 run) | 1104.39 | 1134.17 | 0.974 | 318.48 | 3.468 |
| 2 | N/A(측정 안 함 · R10) | N/A(측정 안 함) | — | 31.41(2/3 · run3 뒤 열 kill) | — |
| acceptance length(판정 입력) | 2.0263… | 2.2069… | 0.918 | 2.2011… | 0.921 |

- 기동 4(10-06) · 판정점 34.41 · 소프트 지문 대조: 기동 4 리포트 스냅샷의 driver · cuda · image_tag · max_model_len · max_num_seqs · kv_cache_memory_bytes · kv_cache_dtype · gpu_memory_utilization · moe_backend · enforce_eager 가 이 측정과 같고, 리포트가 싣지 않은 image_digest · attention_backend · ple_mode 는 기동 4 의 원시 스윕 색인(sweep_run3_boot4/sweep_index.json meta)이 각각 `sha256:e90626f8…` · NA · resident 로 기록해 이 측정과 같다 — 소프트 지문 전 키가 일치하고, 양 노드 이미지 ID 도 10-05 · 10-10 attestation 에서 같다(00 §0.4 (4)) · 부하 조건 같음 · **비교 가능(형상 비교)** — 차이 +4.2 % 는 기동 4 의 자체 밴드 16.74 % 안이고 셀 판정 문서가 이를 재현 PASS 로 판정했다(testlog_26101017_08_29 머리 판정 줄). 수락 비(0.918)는 처리량 비(1.042)와 반대 방향이라 이 차이는 수락으로 **설명되지 않는다** — 밴드 안의 run 간 산포로 읽는다. 측정 형상의 출처는 달랐다(기동 4 = 캠페인 선언 반복 · 이번 = 프리셋 기본 반복) — 값은 같다.
- 기동 3(10-05) · c=1 39.68 / 40.64 / 39.94 · c=2 31.41 / 30.72 · 소프트 지문은 기동 4 와 같은 범위로 같음 · 부하 조건 같음 · **비교 가능(형상 비교)** 이지만 이 스윕은 c=2 run3 끝의 세션 열 가드 kill 로 Full 근거에서 **제외**됐다(testlog_26100510 §4) — 정본 비교 대상이 아니다. 대표 run 비 0.904 는 기동 3 의 run1 이 낮지 않았기 때문이다(왜 낮지 않았는지는 미결 — 기동 3 은 READY 뒤 약 6시간 유휴와 L.A.I.L 이 앞섰지만, 이번에도 run1 바로 앞에 lite 레그가 있었는데 run1 은 낮았다 · Q10). 둘째 · 셋째 run 끼리는 41.04 · 41.22 대 40.64 · 39.94 로 이 측정이 1~3 % 높다. TTFT 도 첫 run 에 몰려 있다 — 이 측정의 run 별 TTFT 중앙값은 1104.39 · 344.76 · 343.81 ms 로(sweep_index runs[]) run2 · run3 은 기동 3 의 run1(318.48 ms)과 같은 대역이다(관측 · 무엇이 차가운지는 미결).
- 10-05 기동 3 lite(44.74 t/s · 앞선 판 바인딩) · 기동 1 lite(41.53) · 이 측정의 lite 레그(41.88) · 도구가 다름(vllm-bench-serve 512/128 대 GuideLLM 1024/256) · **비교 불가**(판정점과) — lite 끼리의 편차는 Q8.
- 트랙 K 진단 셀 Full(K4 · K5 · K6 · F2 — bench_report_26100417_38_45 · 26100418_19_37 · 26100421 · 26100420_31_35) · 같은 도구 · 같은 부하 · 이미지 태그 `…-v9probe` · `…-v10probe` · `…-v11probe` 또는 같은 태그의 이전 빌드(F2 — 패치 74 없음 · 태그는 가변 포인터 · testlog_26100420 §1) · **비교 가능(형상 비교 · stale)** — image_tag(또는 내용) · 셀 env 레버가 다르다. 레버 누적의 방향은 02 §2.5 의 L.A.I.L 사다리가 정본이고 이 Full 수치들끼리 순위를 매기지 않는다.
- 10-03 eager 셀탐색 lite(13.89~19.35 t/s) · 도구 · 부하 · enforce_eager 가 다르다 · **비교 불가**(bench_report_26100314 · 26100318 · 26100318_44_52 · 26100319).
- 참조 런타임 R-1(저자 0909 베이스 · lite 39.74 · L.A.I.L 44.50 — devlog_26100610 §1 · §3) · 강한 키 `vllm_version` 이 다르다(저자 변종 빌드) · **무효** — 격차의 방향을 보는 참고로만 쓴다.
- 외부 보고(sfxnz prose c=1 50.41 tok/s · coolbho3k 1M 프롬프트 32.5 t/s) · 측정 도구 · 부하가 다르고 우리 측정이 아니다 · **비교 불가**(plan_26100208 §8 D7 · plan_26100319 §1).

**손 승계 입력.** 판정 입력 accept_len 2.0263…은 GuideLLM 레그가 아니라 같은 스윕의 lite warm 레그에서 승계했다(m2_judge.log `spec_axis_source=inherited`) — 판정점 레그 자신의 수락은 아니다. E 외부 레퍼런스는 `empty` 로 기록됐고(리포트 경고 E-attempted-empty) 직전 판정의 상태를 승계했다(testlog_26101017_08_29 §3). 루프라인 primary 1.8 · floor 1.53 · ratio 19.928 은 active_bytes 과대 산식 위의 값이다(M4).

**판정 등급.** 권위 = **weak 루브릭 · verdict_rule PASS** — 인증서는 explicit ∧ PASS 일 때만 나오므로 발행되지 않았고, 그 PASS 는 루프라인 결함 위라 성능 판정 정보가 없다 — 측정은 R_fp 2.54 의 약 14배 · spec 상한 R_token 5.14 의 약 7배이고 ratio 19.928 은 기대 달성치 1.8 대비다(M4). 현행 full 정의(docs.md:42 — lite 게이트 → GuideLLM × 반복 ≥3)로 이 측정의 충족은 위 사실 블록대로 **예**다(반복 3 · 도구 guidellm) — 이 셀의 full 등급 측정은 10-06 기동 4 와 이번 두 개이고 서로의 밴드 안이다. 부하 곡선은 레벨 1 하나뿐이다(결손 `HINT_MISSING_SWEEP_LEVELS`). 재측정이 필요한 조건: 네 HW 에서는 항상 · 이 HW 에서도 이미지 재빌드 · 커널 레버 env 변경 · c ≥ 2 를 잴 때(열 여유 — Q4).
