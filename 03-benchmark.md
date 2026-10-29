# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-29T09:43:49Z | sweep_index.generated_utc(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 33.74 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 32.19 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-29T09:43:49Z | — |
| `lite.cold_ttft_ms` | 88.7 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.engine_kv_gib` | 37.9 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) — 서빙 yaml `kv-cache-memory-bytes: 40694815129`(37.9 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/lfm25-26b-v031.yaml) |
| `lite.engine_kv_tokens` | 2476552 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.engine_reserved_total_gib` | 42.95 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.engine_weights_gib` | 5.05 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.gen_tps` | 34.31 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| `lite.kv_gib` | 37.9 | sweep_index.lite(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) — 서빙 yaml `kv-cache-memory-bytes: 40694815129`(37.9 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/lfm25-26b-v031.yaml) |
| `primary_source` | E(external_reference) | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 37.87 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.891 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 33.74 | 33.75 | 0 | 29.29353433496812 | 1 | true | 33.78244336842981 | 29.63504008948803 | 114.88628387451172 |
| 2 | ok | 미기재 | 16 | 40.45 | 40.46 | 0 | 24.48576104407217 | 2 | true | 80.93850896355585 | 24.720323272049427 | 82.3216438293457 |
| 4 | ok | 미기재 | 18 | 40.08 | 40.15 | 0 | 24.72551663716634 | 4 | true | 144.8195824391235 | 24.947467260062695 | 81.04395866394043 |
| 8 | ok | 미기재 | 18 | 39.4 | 39.51 | 0 | 25.163772059421913 | 8 | true | 239.3828851359653 | 25.377916172146797 | 77.66532897949219 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 33.74 | 33.78 | 170.23 | 114.89 | 29.29 | 16/0 |
| 2 | 40.45 | 80.94 | 407.85 | 82.32 | 24.49 | 16/0 |
| 4 | 40.08 | 144.82 | 729.75 | 81.04 | 24.73 | 18/0 |
| 8 | 39.40 | 239.38 | 1206.27 | 77.67 | 25.16 | 18/0 |

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
| `source` | bench_report(bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:dataset_provenance -->
### 데이터셋 출처

> 리포트 측정 구성 표 · 인증서 소프트 지문이 **기재한** 데이터셋 출처다(파싱만 · 합성 ✗ · `/app/` 밖 호스트 경로는 이름만).

| 키 | 값 |
|---|---|
| `dataset_kind` | synthetic |
| `dataset_source` | 미기재 |
| `dataset_id` | 미기재 |
| `dataset_revision` | 미기재 |
| `dataset_source_sha256` | 미기재 |
| `dataset_column_map` | 미기재 |
| `dataset_preprocess_strategy` | 미기재 |
| `dataset_seed` | 미기재 |
| `dataset_duplicate_sampling` | 미기재 |
| `dataset_tokenizer` | 미기재 |
| `dataset_provenance` | 미기재 |
| `status` | recorded |
| `source` | bench_report(bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 6ea16a845769..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `1629609b34df` 2026-10-16T21:44:18Z | 6ea16a845769..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@1629609b34df` | `git show 1629609b34dfe6851338eb7b8353411a1016924d:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `a7083a8f933d` 2026-10-17T13:17:27Z | 6ea16a845769..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@a7083a8f933d` | `git show a7083a8f933d98cd6d739fdac838ebbe3e3b7d3f:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 6ea16a845769..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

> [원문] run_bench.sh@1629609b34df §L438-451
>   #   teardown 의 "캐시 소유권 정렬" 단계와도 어긋나지 않는다.
>   docker run --rm --name "$GLNAME" --network host \
>     --user "$(id -u):$(id -g)" \
>     --memory "${BENCH_BUDGET_MIB}m" --memory-swap "${BENCH_BUDGET_MIB}m" \
>     -v "$TOKSTAGE:/tok:ro" -v "$ABS_OUTDIR:/out" \
>     -e HOME=/tmp -e XDG_CACHE_HOME=/tmp/.cache \
>     -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 \
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

> [원문] lite_bench.sh@a7083a8f933d §L177-183
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
>       && docker exec "$CTR" cat /tmp/lite_tmp.json > "$1"

GuideLLM 0.7.3, default synthetic, 입력 1024·출력 256, 동시성 1/2/4/8, 각 3회다. lite는 별도 vllm bench serve 레그다. 위 발췌는 측정 전 마지막 커밋의 소스 스냅샷이다. 실제 실행 워킹트리 바이트의 결속은 관측하지 못했으며 재구성 명령과 구별한다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/skills/adversarial-benchmark/SKILL.md:33` 원문 그대로):

> `full = lite ∪ GuideLLM × 반복 ≥3`

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102918_lfm2.5-2.6b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm · sweep_index lite_verdict=pass · 측정 레벨 4개 모두 완주 반복 ≥3 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이번 판정점은 33.74 tok/s, weak PASS다. 기존 E 37.87은 같은 BF16/GB10 모델이지만 다른 vLLM, 도구, 출력 길이와 동시성에서 얻어 직접 동등 비교가 아니다. 이전 CNN 및 lite 측정은 데이터셋 또는 도구가 다르며 이번 Full 결과가 아니다. 이번 측정은 lite와 각 유효 레벨 3회로 구성된다. explicit 인증서는 없다. 다른 하드웨어나 버전에서는 재측정이 필요하다(testlog_26102918_LFM_새_Full_실측).
