# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-29T16:08:59Z | sweep_index.generated_utc(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `accept_len` | 2.086021505376344 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md)) |
| `decode_tps_conc1` | 36.32 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 39.52 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-29T16:08:59Z | — |
| `lite.cold_ttft_ms` | 2887.4 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.engine_kv_gib` | 10.0 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) — 서빙 yaml `kv-cache-memory-bytes: 10737418240`(10.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/ds4f0731-032-spec7-roce.yaml) |
| `lite.engine_kv_tokens` | 1034763 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.engine_reserved_total_gib` | 10.0 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.engine_weights_gib` | 79.02 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.gen_tps` | 32.8 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) |
| `lite.kv_gib` | 10.0 | sweep_index.lite(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A) — 서빙 yaml `kv-cache-memory-bytes: 10737418240`(10.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/ds4f0731-032-spec7-roce.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 46.5 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.781 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A · output) |
| `verdict_reasons` | M(36.32) < 46.5×(1−0.15)=39.52 [E(external_reference)] (ratio 0.781) → 기대 이하 | output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.086021505376344 | 16 | 36.32 | 34.83 | 0 | 25.189944809558344 | 1 | true | 35.01524318897278 | 27.53225713968277 | 627.0756721496582 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(36.32) < 46.5×(1−0.15)=39.52 [E(external_reference)] (ratio 0.781) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 36.32 | 35.02 | 186.43 | 627.08 | 25.19 | 16/0 |

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
| `source` | bench_report(bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md) |
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
| `source` | bench_report(bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | 2c2640e74a00..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `29cbdbc89a3d` 2026-10-17T02:03:29Z | 2c2640e74a00..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@29cbdbc89a3d` | `git show 29cbdbc89a3d694097e94a9f9d6c56dde4bc1ee0:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | 2c2640e74a00..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | 2c2640e74a00..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_ds4f0731-032-spec7-roce_261030A/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

> [원문] bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0 §측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)
> | bench_tool_version | 0.7.3 |

이번 cluster측정은GB10 TP2의동일runningserve에서lite후GuideLLM0.7.3 synthetic1024/256,n16,warmup2,C1×3이다. 첫완주36.32와후속37.30/37.75를평균으로대체하지않는다. openai-chat(chat_template),DSpark5/graph,실측acceptance2.086021505376344는litewarm계승이다. UTC창16:01:20Z→16:08:59Z(2026-10-29),최대길이262144는설정/KV수용이며Full전체길이실험이아니다.
전체실행CLI원문은여기새로재구성해원문이라부르지않는다.01§1.4의JSON복원명령과아래측정도구원문을구분한다. GuideLLM은extras ignore_eos=true,temperature/seed CLI미명시다. 그기본값은추정하지않는다. warmup요청2는전체요청을늘린제약과비율로넘기므로완료집계16을원시와대조한다. synthetic_text길이는DATA_ARG에서정한다.

> [원문] run_bench.sh@29cbdbc89a3d §L445-452
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \
>     --disable-progress \

lite는vLLM bench의512/128/random-range-ratio0/request-rateinf/ignore-eos로cold1요청·warmup0,warmburst3요청·warmup1이다. liteCLI도temperature/seed를명시하지않았고GuideLLM warmup2와혼합하지않는다.

> [원문] lite_bench.sh@d33b0878a6c3 §L177-183
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
>       && docker exec "$CTR" cat /tmp/lite_tmp.json > "$1"

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26103001_deepseek-v4-flash-0731_GB10_0.32.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

| 동시성 | 이0.32 측정 | 역사09-28 0.29 측정 | 비(이측정÷이전,소수4자리) |
|---|---|---|---|
| 1 | 36.32 | 32.2 | 1.1280 |
| 2 | 미측정 | 16.2 | N/A |
| 4 | 미측정 | 8.53 | N/A |
| 8 | 미측정 | 4.35 | N/A |
| 16 | 미측정 | 3.69 | N/A |

- bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0의전레벨을싣고현재없는레벨을채우지않았다. strongkey vLLM0.29.0대0.32.0가달라정밀재현비교불가다. driver580.173.02→580.178.04,image tag/digest,parent/source의존,context1048576→262144,Humming→B12X,DSpark7→5,tool-auto플래그등이달랐고모든소프트지문동등을주장하지않는다. 부하GuideLLM0.7.3/synthetic1024·256이겹쳐도한변경만의인과가아니다. acceptance비=2.086021505376344÷2.238372093023256=0.9319(소수4자리)로decode비1.1280을설명하지못하며원인은미분해다.
- 후속09-29 testlog_26092914_54_41_hint_v7_라이브E2E_DS4F0731_판정은tool플래그true형상에서C1=30.95,accept_len2.016,레벨1/2/4/8/16×3완주를보고했다. 그문서에는C2이상개별decode값이없어표에새로합성하지않는다. 현재와는버전/context/K/backend/driver/image가달라비교불가이며30.95대36.32로tool플래그효과나최적성을주장하지않는다.
- 자매형상은역사768K/1M의base/graph/spec/combo다(testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정). 당시0.29/다른context의형상선택근거이지현0.32의새대조가아니다. 현재다른C/graph/K/backend의새실측은없어구간별우세는비교불가다.
- 외부E는testlog_26102919_DS4F0731_외부E_제한검색의동일공식0731/GB10TP2/RoCE직접operator C1short46.5다. 상대는B12X1.2.3/1M/seq8/짧은자연프롬프트,현재1.5.1/262k/seq1/GuideLLM syntheticchat1024·256이므로동일실험은비교불가다. 조건차이를명시한사전weak앵커이며측정후33.4나Abliterated/V4.1로교체하지않는다.
- 판정입력은사전E46.5/tolerance0.15,도구floor39.52(정확산술39.525),M/primary0.781이다. rooflineRfp44.38/Rtoken92.58/expected32.4는컨텍스트이며더낮은primary로대체하지않았다. acceptance는litewarm실측출력계승이고1.0손승계가아니다.

현행lite∪GuideLLM반복≥3을충족한Full,weak REFUTE이며explicit인증서는없다. REFUTE는유효측정과기준미달의표시로hint발행을막지않는다. OBSERVATION-ONLY나production최적baseline으로바꾸지않는다. 다른HW/버전/부하/기능변경에는그조건의새측정이필요하다.
