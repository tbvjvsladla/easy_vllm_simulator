# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-29T16:43:41Z | sweep_index.generated_utc(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `accept_len` | 1.8461538461538463 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md)) |
| `decode_tps_conc1` | 24.04 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 37.17 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-29T16:43:41Z | — |
| `lite.cold_ttft_ms` | 2211.9 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.engine_kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) — 서빙 yaml `kv-cache-memory-bytes: 8589934592`(8.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/i5-032-sm12x.yaml) |
| `lite.engine_kv_tokens` | 2341696 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.engine_reserved_total_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.engine_weights_gib` | 73.96 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.gen_tps` | 21.57 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) |
| `lite.kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_i5-032-sm12x_261030A) — 서빙 yaml `kv-cache-memory-bytes: 8589934592`(8.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/i5-032-sm12x.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 43.73 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.55 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_i5-032-sm12x_261030A · output) |
| `verdict_reasons` | M(24.04) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.55) → 기대 이하 | output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_i5-032-sm12x_261030A · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_i5-032-sm12x_261030A · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 1.8461538461538463 | 16 | 24.04 | 23.72 | 0 | 38.80621685701258 | 1 | true | 23.820376422381937 | 41.59154649823904 | 753.7105083465576 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(24.04) < 43.73×(1−0.15)=37.17 [E(external_reference)] (ratio 0.55) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 24.04 | 23.82 | 121.89 | 753.71 | 38.81 | 16/0 |

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
| `source` | bench_report(bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) |
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
| `source` | bench_report(bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_i5-032-sm12x_261030A/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | 712a6a6e3956..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `29cbdbc89a3d` 2026-10-17T02:03:29Z | 712a6a6e3956..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@29cbdbc89a3d` | `git show 29cbdbc89a3d694097e94a9f9d6c56dde4bc1ee0:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | 712a6a6e3956..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | 712a6a6e3956..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_i5-032-sm12x_261030A/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

> [원문] bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0 §측정 구성 — bench_mode · 도구 · 반복 (기재 · 게이트 아님)
> | bench_tool_version | 0.7.3 |

현재GB10 TP2 cluster의동일runningserve에서lite→GuideLLM0.7.3 synthetic1024/256·n16·warmup2·C1×3,openai-chat(chat_template)를측정했다. 반복24.04/22.73/23.66,대표는첫완주24.04이며평균으로대체하지않는다. DSpark3/greedy/eager,acceptance1.8461538461538463은같은스윕litewarm승계실측이다. UTC16:32:21Z→16:43:41Z(2026-10-29),context131072는설정/KV수용이며전체길이검증이아니다.
실행전체CLI원문을기억으로재구성하지않고01§1.4JSON복원과도구원문을구분한다. GuideLLM extras는ignore_eos=true,temperature/seed는CLI미명시로기본값을추정하지않는다. warmup2는전체요청수증가/비율로변환해16완료집계를대조한다. synthetic_text길이는DATA_ARG에서결정한다.

> [원문] run_bench.sh@29cbdbc89a3d §L445-452
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \
>     --disable-progress \

lite는vLLM bench random512/128/random-range-ratio0/request-rateinf/ignore-eos, cold1요청·warmup0/warmburst3요청·warmup1이다. temperature/seed는liteCLI에도명시없고GuideLLM warmup2와혼합하지않는다.

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
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26103001_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.32.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

| 동시성 | 이0.32 측정 | 최신0.30 10-10 | 비(이측정÷이전,소수4자리) |
|---|---|---|---|
| 1 | 24.04 | 35.87 | 0.6702 |

최신전측정 bench_report_26101013_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0은전체레벨C1하나이며반복35.87/41.04/41.22,밴드13.59%다. 현재밴드5.58%를그자체로동일범위재현이라고부르지않는다. acceptance비=1.8461538461538463÷2.026315789473684=0.9111(소수4자리)와decode비0.6702는맞지않아수용길이하나로처리량격차를설명하지못한다.

| 동시성 | 이0.32 측정 | 10-06 C1전용 | 10-05 열중단구성(Full근거제외) |
|---|---|---|---|
| 1 | 24.04 | 34.41 | 39.68 |
| 2 | 미측정 | 미측정 | 31.41(반복미완) |

10-06원천bench_report_26100613은C1한레벨,10-05원천bench_report_26100516_41_31은C1/C2두레벨이며testlog_26100510_V4_1_Flash_i5_a_kern_정식레시피_판정의열kill정정때문에후자를유효Full재현기준으로쓰지않는다. N/A레벨을새측정으로채우지않는다.

- 이전모델같은pack계열측정은strongkeyvLLM0.30대0.32가달라정밀재현비교불가다. image/source/deps/driver출처,context1M→131k,seq2→1,graph→eager,optional90~92on→미적용,densegemv/moeprepon→off,Engram/V2접점등다수차이가있다. KV8GiB나같은GuideLLM0.7.3/1024·256만으로나머지소프트지문동등을주장하지않는다. 최신pack정확revision도미관측이다.
- 자매형상:역사A/Bpack·eager n0/3/5/10·graph/커널레버F2/K6는별도0.30또는다른pack/지표다(plan_26100319_V4_1_Flash_2노드_광의탐색_캠페인_재설계/devlog_26100610_V4_1_Flash_트랙K_커널레버_이식과_정식화/testlog_26100420_V4_1_Flash_정식화_F2_K6_moe_prep_판정). L.A.I.L/lite/Full을섞어순위를매기지않고역사형상선택근거로만읽는다. 현0.32의새설정스윕은0이며현재c2/graph우세구간은미관측이다.
- 외부E는plan_26102921_Solar_i5_032_성능루브릭의sfxnz round35 oldpack MCG2bpw/lmheadMXFP8·GB10TP2/diskEngram/DSpark3·prose_longC1 43.73이다. exactHF revision/가중치바이트동등미관측,외부graph+10kernel/자연종료와현재eager최소port/syntheticchat이달라동일실험은비교불가다. 사전weak앵커로조건차이를명시해채택했고쉬운LAIL43.17/short52.52/느린옛round로바꾸지않는다.
- 산정입력은E43.73/tolerance0.15/floor37.17(정확37.1705)/M/E0.55다. correctedRfp44.32/Rtoken81.83/expected28.64는물리모형컨텍스트로E를대체하지않았다. Engram행조회제외는전체테이블매토큰스캔오류를교정한것이며행조회지연/실제효율/물리상주0을보증하지않는다. acceptance는litewarm실측출력승계로1.0손승계가아니다.

현행lite∪GuideLLM반복≥3을충족한Full이며weakREFUTE/인증서없음이다. REFUTE는유효측정과고정기준미달의표시이며hint발행자격과분리된다. OBSERVATION-ONLY/production최적baseline으로바꾸지않는다. 다른노드/버전/기능/부하에는그조건의새측정이필요하다.
