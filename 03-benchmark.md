# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-16T11:25:05Z | sweep_index.generated_utc(output/single/benchlog/sweep_ornith15-9b-v031) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 12.75 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 4.32 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-16T11:25:05Z | — |
| `lite.cold_ttft_ms` | 140.8 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.engine_kv_gib` | 9.07 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.engine_kv_tokens` | 267367 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.engine_reserved_total_gib` | 25.87 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.engine_weights_gib` | 16.8 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.gen_tps` | 12.98 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `lite.kv_gib` | 9.07 | sweep_index.lite(output/single/benchlog/sweep_ornith15-9b-v031) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 5.08 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.51 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_ornith15-9b-v031 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_ornith15-9b-v031 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_ornith15-9b-v031 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 12.75 | 12.76 | 0 | 77.68626493566177 | 1 | true | 12.76858789537822 | 78.43464892357588 | 271.6481685638428 |
| 2 | ok | 미기재 | 16 | 13.6 | 13.6 | 0 | 71.95063759298885 | 2 | true | 27.261108860298208 | 73.53346794843674 | 433.3319664001465 |
| 4 | ok | 미기재 | 15 | 13.02 | 13.09 | 0 | 73.46900117163564 | 4 | true | 39.78942444939015 | 76.81289035826921 | 928.2834529876709 |
| 8 | ok | 미기재 | 15 | 12.22 | 12.34 | 0 | 76.4109078575583 | 8 | true | 64.54325017369054 | 81.80979546159506 | 1240.281343460083 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 12.75 | 12.77 | 64.34 | 271.65 | 77.69 | 16/0 |
| 2 | 13.60 | 27.26 | 137.37 | 433.33 | 71.95 | 16/0 |
| 4 | 13.02 | 39.79 | 200.50 | 928.28 | 73.47 | 15/0 |
| 8 | 12.22 | 64.54 | 325.24 | 1240.28 | 76.41 | 15/0 |

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
| `source` | bench_report(bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:dataset_provenance -->
### 데이터셋 출처

> 리포트 측정 구성 표 · 인증서 소프트 지문이 **기재한** 데이터셋 출처다(파싱만 · 합성 ✗ · `/app/` 밖 호스트 경로는 이름만).

| 키 | 값 |
|---|---|
| `dataset_kind` | parquet_file |
| `dataset_source` | managed |
| `dataset_id` | abisee/cnn_dailymail 3.0.0 test |
| `dataset_revision` | 96df5e686bee6baa90b8bee7c28b81fa3fa6223d |
| `dataset_source_sha256` | 04e322d2634a96dba76bf9a6294fbbe48e0b36abeae43f13d86ba2c3bebffe4e |
| `dataset_column_map` | {"text_column": "article"} |
| `dataset_preprocess_strategy` | concatenate |
| `dataset_seed` | 42 |
| `dataset_duplicate_sampling` | False |
| `dataset_tokenizer` | /app/models/ornith-ai/Ornith-1.5-9B |
| `dataset_provenance` | 미기재 |
| `status` | recorded |
| `source` | bench_report(bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_ornith15-9b-v031/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `cb6cc250a65d` 2026-10-16T10:43:45Z | cb6cc250a65d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@cb6cc250a65d` | `git show cb6cc250a65d5572cc33d2d0fb81717dc17e187c:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `cb6cc250a65d` 2026-10-16T10:43:45Z | cb6cc250a65d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@cb6cc250a65d` | `git show cb6cc250a65d5572cc33d2d0fb81717dc17e187c:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | cb6cc250a65d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | cb6cc250a65d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_ornith15-9b-v031/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

> [원문] run_bench.sh@cb6cc250a65d §L444-448
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \

> [원문] testlog_26101620_p10_cnn_fullbench §전제
> - 명령: `sweep_bench.sh <config> --topology single --tool guidellm --preset output/single/configs/cnn_preset.yaml --backend openai-chat --knob-reason backend=chat_template --bench-budget-mib 4096 --knob-reason bench_budget_mib=host_budget` → judge_bench `--authority weak` → render_report. (setsid nohup 래퍼 `/tmp/p10/run.sh`, 로그 `/tmp/p10/log`)

- 도구 GuideLLM 0.7.3, 반복 3, 입출력 길이 1024/256 프리셋, spec 꺼짐(03 §3.1 `spec_on` false), 백엔드 openai-chat.
- full 레그(GuideLLM) 인자는 위 run_bench.sh 원문이 정한다(concurrent 프로필, ignore_eos). 입력 길이·seed 는 프리셋이 정한다. 위 원문 이외의 인자는 도구 원문 확인 없이 적지 않는다.
- lite 레그(vllm bench serve)는 lite_bench.sh 원문이 정한다:

> [원문] lite_bench.sh@e2b67d3320fd §L172-178
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
- 실측 토큰: guidellm JSON(c=1)에서 입력 1034, 출력 256 으로 전부 동일하다(testlog_26101620_49_00_ornith15_9b_v031_CNN_hint셀 에 적힌 측정값).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md` 원문 그대로):

> 미관측

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 미관측 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101620_ornith-1.5-9b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 미관측 | .claude/rules/docs.md 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 측정 · 이 계보에 없음 — 비교 불가(기록 없음).
- 자매 셀(LFM2.5-2.6B) · 같은 노드의 다른 모델 — 모델·가중치 크기가 달라 처리량을 순위 매기지 않는다 · 비교 불가 — 강한 일치 키 `model` 이 다르다.
- 메인 노드의 Granite·MiniCPM CNN 셀 · 다른 노드 — 측정 노드·소프트 지문이 다르다 · 비교 불가(이 계보에 인증서 없음).
- 합성(random) 입력으로 잰 lite 수치 · lite 레그의 값 · 입력 분포가 다르다 · 비교 불가(lite 수치는 03 §3.1 에 따로 있다).
- 외부 레퍼런스 · 없음 — 외부검색(E) 미수행.

판정 등급: 판정 PASS(권위 weak, primary 5.08 t/s, floor 4.32 t/s, ratio 2.51). 판정은 발행 게이트가 아니라 표시다. 기대치 산정은 roofline×MBU 이고 외부 레퍼런스가 붙지 않아 기대치 자체의 검증이 없다 — 이 수치는 baseline 으로 쓰지 않는다. 현행 full 정의 충족 여부는 위 사실 블록이 미관측이라 판정하지 않는다. 재측정 조건을 정한 계보 문서는 없다(추론: 인증서 규칙상 소프트 지문 키 — 드라이버·CUDA·이미지·max_model_len·KV 바이트·gmu 등 — 가 바뀌면 stale 이다; 이 계보의 결정이 아니다). 자체 재현 밴드는 리포트 "반복 축 · 재현 밴드" 표가 레벨별로 기록한다 — 동시성 1/2/4/8 에서 0.08/0.00/0.00/0.08 %(n=3 · 이 셀 값).
