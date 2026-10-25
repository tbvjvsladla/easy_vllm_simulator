# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-25T00:44:08Z | sweep_index.generated_utc(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 23.29 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 13.69 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-25T00:44:08Z | — |
| `lite.cold_ttft_ms` | 485.0 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.engine_kv_gib` | 3.14 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) — 서빙 yaml `kv-cache-memory-bytes: 3368402176`(3.1 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/gemma26b-ctx-single.yaml) |
| `lite.engine_kv_tokens` | 267398 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.engine_reserved_total_gib` | 51.68 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.engine_weights_gib` | 48.54 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.gen_tps` | 24.84 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) |
| `lite.kv_gib` | 3.14 | sweep_index.lite(output/single/benchlog/sweep_gemma26b-ctx-single) — 서빙 yaml `kv-cache-memory-bytes: 3368402176`(3.1 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/gemma26b-ctx-single.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 16.1 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.447 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_gemma26b-ctx-single · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_gemma26b-ctx-single · output) | — |

**`levels`**(5행 · 출처 sweep_index(output/single/benchlog/sweep_gemma26b-ctx-single · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 23.29 | 23.28 | 0 | 41.79201874078489 | 1 | true | 23.341729446331524 | 42.94085502624512 | 335.8941078186035 |
| 2 | ok | 미기재 | 16 | 21.03 | 21.04 | 0 | 46.88075850991642 | 2 | true | 42.08365602267448 | 47.560579143464565 | 189.64886665344238 |
| 4 | ok | 미기재 | 16 | 16.05 | 16.53 | 0 | 61.62689433378332 | 4 | true | 54.066490122601735 | 62.32407223433256 | 237.74981498718262 |
| 8 | ok | 미기재 | 16 | 12.16 | 12.8 | 0 | 81.54204592985265 | 8 | true | 75.49156670168364 | 82.25398510694504 | 264.2219066619873 |
| 16 | ok | 미기재 | 18 | 9.22 | 9.83 | 0 | 107.52059338139553 | 16 | true | 115.95927290933346 | 108.49669575691223 | 291.57018661499023 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 23.29 | 23.34 | 117.89 | 335.89 | 41.79 | 16/0 |
| 2 | 21.03 | 42.08 | 212.56 | 189.65 | 46.88 | 16/0 |
| 4 | 16.05 | 54.07 | 273.08 | 237.75 | 61.63 | 16/0 |
| 8 | 12.16 | 75.49 | 381.29 | 264.22 | 81.54 | 16/0 |
| 16 | 9.22 | 115.96 | 585.68 | 291.57 | 107.52 | 18/0 |

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
| `source` | bench_report(bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_gemma26b-ctx-single/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
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

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_gemma26b-ctx-single/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 셀의 명령 줄 원문(셸 히스토리 · 로그에 남은 호출 줄)은 계보에 없다 — 01 §1.4 의 3단계 명령은 bench JSON 에서 재구성한 lite 레그 명령이다. 판정점(full · GuideLLM)과 lite 레그는 다른 도구가 불렀으므로 도구 원문을 레그별로 인용한다. 아래는 그 판본 `run_bench.sh` 의 GuideLLM 호출 줄이다 — 스윕 색인(`bench_knobs`)이 이 셀에 넘긴 값은 동시성 `1 · 2 · 4 · 8 · 16` · 입력 `1024` · 출력 `256` · 요청 `16` · warmup `2` · 반복 `3` · 데이터셋 `synthetic` 이다.

> [원문] run_bench.sh@3fa6eac00a18 §L445-450
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

같은 판본에서 synthetic 이면 `DATA_ARG="kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN"`(L407)이고, warmup 은 요청 수가 아니라 비율로 옮겨 `TOTAL_REQ = NPROMPTS + WARMUPS` · `WARM_FRAC = WARMUPS / TOTAL_REQ` 로 넘긴다(L345 · L346). 벤치 컨테이너는 `--memory ${BENCH_BUDGET_MIB}m` 하드 제한으로 돌고(L441), 띄우기 전에 선언된 floor 에서 그 몫을 빼도 절대밴드가 남는지 `budget_preflight.py --floor-mib … --bench-budget-mib …` 로 묻는다(L378). GuideLLM 호출에는 temperature · seed 인자가 없다 — 서버 기본값(측정 기동 로그가 `generation_config.json` 에서 temperature 1.0 · top_k 64 · top_p 0.95 로 덮였다고 적는다 · lite_engine L110)이 쓰였을 것이다(추론). lite 레그는 서버 컨테이너 안에서 `vllm bench serve` 를 부른다:

> [원문] lite_bench.sh@90a3cdd56a94 §L177-181
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

cold 는 `_bench "$COLD" 1 0`(요청 1 · warmup 0) · warm 은 `_bench "$WARM" "$BURST_N" 1`(기본 `BURST_N=3` · warmup 1)이다(lite_bench.sh@90a3cdd56a94 L229 · L231 · L42). lite 호출에도 `--temperature` 가 없다. 측정 도구 4건은 측정 뒤 지금 HEAD 까지 바뀌지 않았다(위 표 `다음 변경`).

**조건 목록**
- 도구와 버전: 판정점 = GuideLLM 0.7.3(`ghcr.io/vllm-project/guidellm:v0.7.3` · digest 일치 확인 · 레벨 1 bench_tool JSON) · lite = 서빙 이미지 안 `vllm bench serve`(vLLM 0.31.1.dev0 빌드).
- 입력/출력 토큰 길이: 판정점 synthetic prompt_tokens 1024 · output_tokens 256(스윕 색인 bench_knobs · 레벨 1 dataset JSON · 도구 원문 L407) · lite random 512 / 128 · range-ratio 0(도구 원문 L180). 01 §1.4 의 재구성 명령이 적은 525 / 128 은 chat 템플릿을 포함한 실측 평균이다.
- 동시성 레벨: 1 · 2 · 4 · 8 · 16 전부 완주(03 §3.2 · 절삭 없음 · 서빙 yaml 에 max-num-seqs 선언 없음).
- 레벨당 요청 수: 16(+warmup 2 → GuideLLM max_requests 18) · 레벨 16 은 완료 18 로 적혔다(03 §3.1 levels).
- 반복 수: 3(레벨별 3/3 완주 · 판정점 run 순 decode 23.29 / 23.66 / 23.67 · 재현 밴드 1.61% — bench_report_26102509 §반복 축). 표 수치는 대표 run(첫 완주) 값이다.
- warm/cold: lite 는 cold 단일 요청(warmup 0) + warm 3요청(warmup 1) · GuideLLM 은 warmup 비율 2/18.
- ignore-eos: 두 레그 모두 켬(GuideLLM `extras={"ignore_eos":true}` · lite `--ignore-eos`).
- spec decode: 꺼짐(선언 off · 측정 `spec_on false`) · acceptance length 미기재(루브릭은 선언 부재라 1.0 을 쓴다 · 측정값 아님).
- 엔드포인트 · 백엔드: `/v1/chat/completions` · `openai-chat`(bench_tool JSON `endpoint` · 스윕 색인 `backend`).
- 측정 노드: main(스윕 색인 `measured_node` · single).
- 측정 시각(UTC): lite 레그 시작 2026-10-25T00:17:00Z → 색인 조립 00:44:08Z(01 §1.4 bench 단계 · 03 §3.1 measured_utc).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 셀에는 인증서가 없으므로(위 비발행 판정 · 권한 weak) 비교의 강한 일치 키 · 소프트 지문은 스윕 색인 meta 와 리포트 측정 환경 스냅샷에서 읽는다.

- 같은 셀의 이전 측정(캠페인 lite · 2026-10-24T03:49:19Z · 원장 시도 2) · warm gen 25.11 t/s · cold TTFT 456 ms(testlog_26102412 §3. lite) · 같은 lite_bench.sh 판본(`90a3cdd56a94` · 측정 뒤 변경 없음)과 같은 부하(random 512/128 · 동시성 1 · burst 3) · 다른 기동 회차 · 비교 가능(lite 대 lite) — 이 측정의 lite 레그와 아래 표로 나란히 둔다. 소프트 지문(이미지 digest 등)은 캠페인 lite raw 에 기록이 없어 대조하지 못했다(미확인). 이전 측정에는 full 레벨이 없다 — 레벨 곡선의 비교 대상은 없다.

| 동시성 | 이 측정(lite 레그 warm gen) | 이전 측정(캠페인 lite warm gen) | 비 |
|---|---|---|---|
| 1 | 24.84 | 25.11 | 0.989 |

  같은 행의 cold TTFT 는 485 ms 대 456 ms 다(비 1.064 · 소수 셋째 자리 반올림). warm gen 차이 −1.1% 는 이 측정의 판정점 재현 밴드(1.61% · GuideLLM 레벨 1)보다 작지만, lite 레그 자체의 반복 밴드는 측정되지 않았으므로 "같은 범위 재현" 이라고 부르지 않는다 — 차이의 원인은 설명되지 않았다.
- 이 측정의 full 판정점 23.29 t/s 대 lite 레그 24.84 t/s · 도구(GuideLLM 대 vllm bench serve) · 입출력 길이(1024/256 대 512/128) · 요청 수가 다르다 · 비교 불가 — 부하 조건이 다르다(위 조건 목록).
- 같은 S5 묶음의 다른 셀(Qwen3.8-Flash-Next · Solar-Open2 · testlog_26102504 §1. 결과표) · 16.67 ~ 26.01 t/s · 강한 일치 키 `model`(과 `quantization` · 일부 `topology` · `tensor_parallel_size`)가 다르다 · 비교 불가(무효) — 다른 모델의 수치다.
- 외부 레퍼런스 · 없음 · 판정기 E 검색이 `empty` 로 끝났다(bench_report §판정 `E-attempted-empty`) · 비교 대상 없음.
- 다른 vLLM 버전의 hint · 없음 — 코드 브랜치 카탈로그(`HINTS.md`)에 이 모델의 다른 태그가 없다(저작 시 확인).

**판정 등급** — `PASS`(verdict_rule 결정론 · 루브릭 권한 weak). 기준은 외부 레퍼런스가 아니라 roofline 기대치다: R_fp 45.99 t/s(판정기가 센 활성 가중치 5.528 GiB · `bandwidth_gbps 273.0` · `bandwidth_source lookup:gb10`) × realistic_fraction 0.35 = expected 16.1 → floor 16.1 × (1 − 0.15) = 13.69, 측정 23.29 의 비는 1.447 이다(verdict.json · roofline.json). ⚠ 그 5.528 GiB 는 토큰당 활성 가중치가 아니다 — roofline.json notes 가 `MoE active = dense 5.53GiB + (0/128)×routed 42.54GiB` 로 routed expert 를 0개로 셌다. 생산자 roofline.py 결함이다: top-k 를 `num_experts_per_tok` · `num_experts_per_token` · `moe_topk` 키에서만 찾아(roofline.py L314) 이 config 의 `top_k_experts: 8` 을 읽지 못해 k=0 이 됐다. config 대로 top-8 을 넣으면 활성은 약 5.53 + 42.54 × 8/128 ≈ 8.19 GiB · R_fp 약 31.0 · expected 약 10.9 · floor 약 9.2 t/s 다(저작자 산술 · 판정기 재실행 아님) — 어느 쪽이든 23.29 는 floor 위라 PASS 는 바뀌지 않는다. 위 판정 수치는 verdict.json 의 값 그대로다. 손으로 승계된 판정 입력은 acceptance length 1.0(spec 미선언이라 루브릭이 쓴 선언값 · 측정 아님)과 루브릭 상수 realistic_fraction 0.35 · tolerance 0.15 이다 — 이 셀에서 잰 값이 아니다. 인증서는 explicit ∧ PASS 에서만 나오므로 없다 — 이 판정은 발행 게이트가 아니라 표시다. 현행 full 정의(위 사실 블록)는 **충족**이다(반복 3 · GuideLLM). 다시 재야 하는 조건: 외부 레퍼런스(E)를 얻었을 때(02 Q4) · 장문 프롬프트 조건이 필요할 때(이 측정은 1024/256 synthetic · 02 Q1) · 이미지 · 드라이버 · KV 클램프 · gmu 가 바뀌었을 때.
