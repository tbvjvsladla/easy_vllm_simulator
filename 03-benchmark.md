# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-25T01:22:24Z | sweep_index.generated_utc(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 16.72 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 6.7 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-25T01:22:24Z | — |
| `lite.cold_ttft_ms` | 5493.4 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.engine_kv_gib` | 15.5 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) — 서빙 yaml `kv-cache-memory-bytes: 16642998272`(15.5 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml) |
| `lite.engine_kv_tokens` | 1237563 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.engine_reserved_total_gib` | 89.64 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.engine_weights_gib` | 74.14 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.gen_tps` | 16.96 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| `lite.kv_gib` | 15.5 | sweep_index.lite(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) — 서빙 yaml `kv-cache-memory-bytes: 16642998272`(15.5 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 7.88 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.122 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1 · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 16.72 | 16.69 | 0 | 57.67828997443704 | 1 | true | 16.74486741806696 | 59.81433019042015 | 634.577751159668 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 16.72 | 16.74 | 87.13 | 634.58 | 57.68 | 16/0 |

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
| `source` | bench_report(bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | 20c7690b560e..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `3fa6eac00a18` 2026-10-16T21:43:55Z | 20c7690b560e..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@3fa6eac00a18` | `git show 3fa6eac00a185bb5f145fa5ecb6cae276601a3c9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | 20c7690b560e..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 20c7690b560e..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

명령 줄 실행 원문(셸 히스토리 · 드라이버 로그)은 계보에 없다. 아래는 측정 시각 이전 마지막 커밋의 **도구 원문**(위 스냅샷)이고, 레그마다 부른 도구가 다르다.

**full 판정점 레그(동시성 1 × 반복 3) — `run_bench.sh@3fa6eac00a18`.** sweep_bench 가 레벨마다 `--tool guidellm` 으로 부른다. GuideLLM 컨테이너 호출 원문:

> [원문] run_bench.sh@3fa6eac00a18 §L439-452
> docker run --rm --name "$GLNAME" --network host \
> --user "$(id -u):$(id -g)" \
> --memory "${BENCH_BUDGET_MIB}m" --memory-swap "${BENCH_BUDGET_MIB}m" \
> -v "$TOKSTAGE:/tok:ro" -v "$ABS_OUTDIR:/out" \
> -e HOME=/tmp -e XDG_CACHE_HOME=/tmp/.cache \
> -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 \
> --entrypoint guidellm "$IMAGE" run \
> --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
> --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
> --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
> --tokenizer "kind=huggingface_auto,model=/tok" \
> --constraint "kind=max_requests,count=$TOTAL_REQ" \
> --output "kind=json,path=/out/$(basename "$BJSON")" \
> --disable-progress \

합성 데이터 인자는 같은 판본 L407 의 `DATA_ARG="kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN"` 이고, warmup 은 요청 수가 아니라 비율로 바꿔 넘긴다(L345-346 `TOTAL_REQ = NPROMPTS + WARMUPS` · `WARM_FRAC = WARMUPS / TOTAL_REQ`). 이 측정의 값은 `sweep_index.json` `bench_knobs` 가 적는다 — input_len 1024 · output_len 256 · num_prompts 16 · warmups 2 · repeats 3 · bench_budget_mib 2048 · backend openai-chat(엔드포인트 `/v1/chat/completions`). GuideLLM 산출 json 은 `max_requests count 18` · `warmup percent 0.1111` · 집계 요청 16 · 요청당 프롬프트 1076 토큰 · 출력 256 토큰(전부 같은 값 — 1024 합성 프롬프트에 chat 템플릿이 붙은 실측)을 적는다(`level_01/guidellm_q38fn31-nv4-mmap-1m-tp1.json`). 도구 원문의 GuideLLM 인자에는 temperature · seed 가 없다 — 샘플링 값을 도구가 정하지 않았고, 서버는 모델 `generation_config.json` 의 `temperature 1.0 · top_k 20 · top_p 0.95` 로 기본값을 덮었다고 적는다(lite_engine L157). `ignore_eos` 는 backend extras 로 true 다.

**lite 레그(cold 1 · warm 3) — `lite_bench.sh@90a3cdd56a94`.** 같은 스윕이 lite 레그로 부른다. 서빙 컨테이너 안에서 `vllm bench serve` 를 돌린다:

> [원문] lite_bench.sh@90a3cdd56a94 §L177-182
> docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
> --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
> --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
> --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
> --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
> --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

lite 는 cold(무-warmup 단일 요청)와 warm burst(N=3 · conc 1 · warmup 1) 두 번이다(같은 판본 L7 머리 주석 · `lite_leg.log`). 01 §1.4 의 재구성 명령이 적은 입력 566 · 565 토큰은 bench JSON 의 total ÷ completed 평균(chat 템플릿 포함)이고 도구 원문 인자는 512 다. lite 레그의 temperature 인자도 원문에 없다(`--ignore-eos` 는 있다).

**조건 목록**
- 도구와 버전: 판정점 GuideLLM 0.7.3(측정 `metadata.guidellm_version` · 이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` digest `sha256:e3ad2371…`) · lite 는 이미지 안 `vllm bench serve`.
- 입력/출력 길이: 판정점 1024 / 256(합성 · 실측 프롬프트 1076) · lite 512 / 128(도구 원문).
- 동시성 레벨: 1 하나 — 2 · 4 · 8 · 16 은 `max-num-seqs 1` 로 걸러졌다(`bench_knobs.knobs.levels.filtered_out`).
- 레벨당 요청 수: 집계 16(+ warmup 비율 0.1111 · 총 18).
- 반복 수: 3 · 완주 3(03 §3.3) · decode 16.72 / 16.87 / 16.89 t/s · 재현 밴드 1.01%(`bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0` §반복 축 — 표와 판정점은 대표 run 첫 완주 하나의 값).
- warm/cold: 판정점은 warmup 비율을 둔 warm 측정 · cold TTFT 5493.4 ms 는 lite cold 레그 값(03 §3.1).
- spec decode: 꺼짐(`speculative_config=None` · lite_engine L76 · `measured_spec_on false`) · acceptance length 미기재(spec 없음 — 루브릭은 1.0 선언값을 쓴다).
- 측정 노드: main(스윕 meta `measured_node`) · 측정 시각 2026-10-25T01:07:04Z → 01:22:24Z(01 §1.4).

> [원문] bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0 §반복 축 · 재현 밴드 (full 정의 = lite ∪ GuideLLM × 반복 ≥3)
> | 동시성 | 완주/요청 | decode t/s (run 순) | 재현 밴드 % | 밴드 출처 |
> |---|---|---|---|---|
> | 1 | 3/3 | 16.72 / 16.87 / 16.89 | 1.01 | measured(n=3) |

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

인증서가 없다(weak 권한 · 03 §3.3 비발행 판정). 그래서 비교 규칙은 인증서 블록 대신 각 리포트의 '측정 환경 스냅샷' 표 · 측정 구성 표에서 같은 키를 대조했다. 이 측정의 강한 키는 model `qwen3.8-flash-next-nvfp4` · gpu `NVIDIA GB10` · vllm_version `0.31.0` · quantization `N/A` · topology `single` · TP `1` 이다.

- **같은 셀의 이전 full 측정** · 없음 — 이 셀의 full 측정은 이번 한 번뿐이다. 레벨 표를 나란히 둘 상대가 없다.
- **같은 셀 · 같은 측정의 lite 레그** · warm 16.96 t/s(median TPOT 기반) · 도구(`vllm bench serve` 512/128 대 GuideLLM 1024/256) · 반복 · 지표 정의가 다름 · **비교 불가** — 판정점 16.72 와 나란히 놓지 않는다(`bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0` §lite 지표).
- **같은 셀의 S3 lite(2026-10-24 · 시도 1)** · 계보 경유 문서 `testlog_26102500_…`(읽기 목록 밖)가 16.84 t/s 로 적는다 · 같은 셀 · 같은 이미지 · 같은 lite 도구(저작 때 `git log -- .claude/skills/adversarial-benchmark/scripts/lite_bench.sh` 로 보니 2026-10-16 뒤 커밋은 90a3cdd 하나라 S3 시각에도 같은 판본이다) · 비교 가능(lite 끼리) — 이번 lite 16.96 과의 비는 1.007(소수 3자리 · 두 값의 나눗셈 한 번)이다. 그 값의 원천 raw 는 스윕 폴더 밖이고 16.84 를 담은 문서는 이 페이로드의 근거로 쓸 수 없어 참고로만 둔다.
- **자매 셀 · 멀티 TP=2 NVFP4 PLE 상주(`q38fn31-nv4-1m-tp2`)** · 26.01 t/s · topology `multi` · TP `2` 로 강한 키 2개가 다름 · **무효** — 이 수치로 순위를 매기지 않는다(`bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0`). 참고로 그 셀은 같은 GuideLLM 0.7.3 · 반복 3 · 입력 1024 · max_num_seqs 1 · gmu 0.80 · eager 이고 KV 클램프는 15032385536 로 다르다.
- **자매 셀 · 멀티 TP=2 FP8 PLE mmap(`q38fn31-fp8-1m-tp2`)** · 22.78 t/s · model(FP8) · topology · TP 가 다름 · **무효**(`bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0`).
- **이전 vLLM 멀티 1M mmap 셀(`nv4-f8-1m-mmp` 33.40 t/s 등)** · vllm_version 0.29.0rc6 · topology multi · TP 2 · 도구 `vllm bench serve`(`/v1/completions`) · **무효**(`testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정` §판정 결과).
- **외부 레퍼런스 E — 32.5 t/s(판정 문서는 "이 세션이 고른 외부 레퍼런스" 라고만 적는다 · NVIDIA 1 Spark 사례와 수치가 같아 그 사례로 보인다 — 추론)** · 그 사례에 대해 계보는 "PLE mmap 경로(빌드 패치) 전제" 라고만 적고 spec decode · 입출력 길이 · 도구 · 컨텍스트를 적지 않는다 · 부하 조건 미상 · **비교 불가**. 이 값은 이 셀 R_fp 22.52 t/s 를 넘어 판정기가 NEEDS_RUBRIC 를 냈다(02 M2 · R5).
- **커뮤니티 기준선(dolf 33.0 t/s MTP k=3 · blazux ~37 t/s)** · 둘 다 싱글 NVFP4 이지만 MTP · 다른 패치 · 다른 이미지 · 측정 조건 미상 · **비교 불가**(`plan_26090918_qwen38_flashnext_multi_딥캠페인` §2.3).

**손으로 승계된 판정 입력.** 루브릭의 acceptance length 1.0 은 측정값이 아니라 "spec 미선언" 선언값이다(03 §3.1 · `accept_len_source declared-absent`) — 이 셀은 spec off 라 1.0 이 정상이다. MBU 비율 0.35(`realistic_fraction` · roofline.json)와 대역폭 273 GB/s(`bandwidth_source lookup:gb10`)는 루프라인 도구의 조회 상수이고 이 셀에서 잰 값이 아니다. PLE 47.684 GiB 는 active 바이트에서 뺐다(roofline.json notes — 행 gather 라서).

**판정 등급.**
- 판정 경위: 첫 판정은 weak 권한의 외부 기준 E=32.5 를 쓰려 했으나 E 가 이 형상의 물리상한 R_fp 22.52 를 넘어 NEEDS_RUBRIC 였다. `--e-search empty` 로 다시 판정해 roofline×MBU expected 7.88 → floor 6.7(× 0.85)이 primary 가 됐고, M 16.72 ≥ 6.7 로 **PASS** 다(ratio 2.122 · `verdict.json` `e_search: empty` · `warning: E-attempted-empty … roofline-only 판정(음성정직)`). 첫 판정의 산출물은 스윕 폴더에 남아 있지 않다(02 M2). 이 PASS 는 "루프라인 하한보다 빠르다" 는 뜻이고 외부 기준을 만족했다는 뜻이 아니다.
- 권위: 루브릭 권한 weak · 인증서 없음 · 관측 게재가 아니라 판정이 있는 측정이다(PASS 는 표시이고 발행 게이트가 아니다).
- 현행 full 정의: 위 사실 블록대로 **충족 — 예**(GuideLLM · 반복 3 ≥ 3). 단 레벨이 동시성 1 하나뿐이라 부하 거동은 이 측정으로 알 수 없다(결손 `HINT_MISSING_SWEEP_LEVELS` · 원인 `max-num-seqs 1`).
- 재측정이 필요한 조건: 다른 HW · 다른 vLLM · graph 호환 플러그인 · `max-num-seqs` 변경 · 같은 조건의 외부 기준을 얻었을 때(E 로 다시 판정).
