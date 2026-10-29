# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-29T11:41:26Z | sweep_index.generated_utc(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `accept_len` | 2.284023668639053 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md)) |
| `decode_tps_conc1` | 41.11 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 45.65 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-29T11:41:26Z | — |
| `lite.cold_ttft_ms` | 185.8 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.engine_kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn032-nv4-safe-tp2.yaml) |
| `lite.engine_kv_tokens` | 1595769 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.engine_reserved_total_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.engine_weights_gib` | 63.04 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.gen_tps` | 45.92 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) |
| `lite.kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn032-nv4-safe-tp2.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 53.7 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.766 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · output) |
| `verdict_reasons` | M(41.11) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.766) → 기대 이하 | output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · output) | — |

**`levels`**(3행 · 출처 sweep_index(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.284023668639053 | 16 | 41.11 | 37.53 | 0 | 23.159212224623737 | 1 | true | 37.658953496754606 | 24.325428530573845 | 328.8431167602539 |
| 2 | ok | 2.284023668639053 | 16 | 33.68 | 31.73 | 0 | 28.420049068974514 | 2 | true | 61.07117424278657 | 29.690164141356945 | 353.56950759887695 |
| 4 | ok | 2.284023668639053 | 16 | 24.34 | 23.39 | 0 | 39.7553518706677 | 4 | true | 78.38359166900358 | 41.07917007058859 | 385.8377933502197 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(41.11) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.766) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 41.11 | 37.66 | 188.44 | 328.84 | 23.16 | 16/0 |
| 2 | 33.68 | 61.07 | 305.59 | 353.57 | 28.42 | 16/0 |
| 4 | 24.34 | 78.38 | 392.22 | 385.84 | 39.76 | 16/0 |

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
| `source` | bench_report(bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md) |
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
| `source` | bench_report(bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | dc28b0608133..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `29cbdbc89a3d` 2026-10-17T02:03:29Z | dc28b0608133..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@29cbdbc89a3d` | `git show 29cbdbc89a3d694097e94a9f9d6c56dde4bc1ee0:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | dc28b0608133..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `17959581d8ae` 2026-10-27T07:26:14Z | dc28b0608133..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@17959581d8ae` | `git show 17959581d8ae97c2e951120dbca09a5a882e3051:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_q38fn032-nv4-safe-tp2_261029B/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

선택B의Full은GuideLLM0.7.3 synthetic1024/256·요청16/워밍업2·completions/openai·client4096MiB·C1/2/4각3회다. 실제실집계건수는03표/각runJSON을읽는다. 측정은11:27:11Z lite시작→11:41:26Z sweep종료이며이후단위교정시각을측정시각으로쓰지않는다. B의수용길이2.284023668639053은같은스윕litewarm에서승계된실측이며GuideLLM자체acceptance측정이라고쓰지않는다.

> [원문] run_bench.sh@29cbdbc89a3d §L445-451
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

GuideLLM레그는ignore_eos true이고위호출에서temperature/seed는명시되지않았다. lite는별도vllm bench serve random512/128 cold1/warm3·warmup0/1이다.

> [원문] lite_bench.sh@d33b0878a6c3 §L177-182
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

lite의range-ratio0/ignore-eos와GuideLLM조건을섞지않는다. 측정tool스냅샷은커밋된소스의정적선택이며worktree실행바이트전체결속을보증하지않는다. B의qwen-full-guidellm.log와판정문이실행관측이고Marlin controller부분stdout은그후보의제한이다. 01재구성명령을전체실행argv원문으로위장하지않는다. parser8f0d009는요청지연초→ms만재파생했고원run/engine/TPS/verdict는불변이다(testlog_26103000_11_58_GuideLLM_요청지연_단위_교정).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102920_qwen3.8-flash-next-nvfp4_GB10_0.32.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

모든비는이측정÷대상한번나눗셈,소수넷째자리반올림이다. 인증서가없으므로각report/meta와declaredshape를비교했으며소프트지문전체동등/유일차이를주장하지않는다.

- 같은032셀C(Marlin) ·대표39.68/31.68/24.20 ·동일synthetic1024/256/16/2·MTP3이나backend형상다름 ·비교가능(형상비교·stale) — B의targetCUTLASS/draftTRITON 대 C전역Marlin,acceptance도실측출력으로다르다. 통계적우월성은불명이며C반복최고43.77을보존한다(testlog_26102921).

B와C의모든공통레벨

| 동시성 | 이 측정 | 이전 측정 | 비 | acceptance 비 |
|---|---|---|---|---|
| 1 | 41.11 | 39.68 | 1.0360 | 1.2312 |
| 2 | 33.68 | 31.68 | 1.0631 | 1.2312 |
| 4 | 24.34 | 24.20 | 1.0058 | 1.2312 |

acceptance비는양쪽MTP가켜진실측출력의비다. decode비가레벨마다달라acceptance비하나로설명할수없고인과분리는판정불가다.

- H ·대표33.73/31.47 ·같은도구/짧은부하이지만nativeb12x/text-only/131072/seq2/chunk1024/KV4GiB와cutoverexport제거가함께다름 ·형상비교참고이며단일backend비교불가 — acceptance와메모리관측의범위도선택판정문을따른다(testlog_26103000_Qwen032_유효Full과_발행후보_선택).

B와H의공통레벨;H에는C4없음

| 동시성 | 이 측정 | 이전 측정 | 비 | acceptance 비 |
|---|---|---|---|---|
| 1 | 41.11 | 33.73 | 1.2188 | 1.1092 |
| 2 | 33.68 | 31.47 | 1.0702 | 1.1092 |

H의litewarm acceptance2.0591397849462365 대비비도실측출력의비이며decode차이를그비하나로설명하는지는판정불가다.

- 과거같은모델0.31 1M resident ·26.01(C1만) ·vLLM강한키·context·specoff/eager/seq1·chat부하가다름 ·비교불가(강한키무효/부하차이) — 이표의비는산술참고이며개선율이아니다(bench_report_26102511).

이전0.31의존재하는전레벨

| 동시성 | 이 측정 | 이전 측정 | 비 |
|---|---|---|---|
| 1 | 41.11 | 26.01 | 1.5805 |

- 과거0.30 262k resident ·37.63 ·vLLM강한키/이미지·KV8GiB/auto·eager·seq8이다름 ·비교불가 — 0.29의resident46.70/mmap대조와RTX PRO6000자료도같은HW/버전baseline으로이식하지않는다(bench_report_26101013, testlog_26091304, perf_26091322).
- 외부E53.7 ·2Spark resident/MTP3/FP8KV선례로계보가선택한값 ·전체부하/context/소프트지문동등은미확정 ·조건차이를남긴외부기대기준이다. 1Spark32.5mmap을같은기준으로바꿔쓰지않는다(testlog_26091304 §2).
- 손승계입력은E53.7/tolerance0.15다. acceptance2.284023668639053은B같은스윕litewarm실측에서승계한값이지사람이만든상수가아니다. floor45.645의도구반올림은45.65다.

현재B는lite∪GuideLLM각3회Full을충족했고weak REFUTE다. 성능판정은표시이며서빙성립/유효측정과발행승인을별도로따른다. weakREFUTE라full인증서는없고report를바인딩한다. 긴부하·다른HW/driver·소스/의존·backend/메모리형상은재측정대상이며이관측을SLA나검증된최속전략으로부르지않는다.
