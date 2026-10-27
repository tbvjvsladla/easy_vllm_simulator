# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-27T01:17:15Z | sweep_index.generated_utc(output/single/benchlog/sweep_granite42-30b-v030) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md)) |
| `decode_tps_conc1` | 3.9 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 1.39 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-27T01:17:15Z | — |
| `lite.cold_ttft_ms` | 568.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_kv_gib` | 33.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) — 서빙 yaml `kv-cache-memory-bytes: 35433480192`(33.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/granite42-30b-v030.yaml) |
| `lite.engine_kv_tokens` | 135168 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_reserved_total_gib` | 87.58 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_weights_gib` | 54.58 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.gen_tps` | 3.97 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.kv_gib` | 33.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) — 서빙 yaml `kv-cache-memory-bytes: 35433480192`(33.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/granite42-30b-v030.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 1.63 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.393 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 3.9 | 3.9 | 0 | 253.45267688526826 | 1 | true | 3.9040991810980072 | 256.16236217319965 | 948.9233493804932 |
| 2 | ok | 미기재 | 16 | 3.89 | 3.89 | 0 | 254.8808733622233 | 2 | true | 7.790013704520459 | 256.8801697343588 | 771.5609073638916 |
| 4 | ok | 미기재 | 18 | 3.85 | 3.86 | 0 | 257.7515592762068 | 4 | true | 13.906252639123213 | 259.95116867125034 | 775.3479480743408 |
| 8 | ok | 미기재 | 18 | 3.76 | 3.77 | 0 | 264.2248864267387 | 8 | true | 22.84517719767458 | 265.64835477620363 | 630.0859451293945 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 3.90 | 3.90 | 19.75 | 948.92 | 253.45 | 16/0 |
| 2 | 3.89 | 7.79 | 39.41 | 771.56 | 254.88 | 16/0 |
| 4 | 3.85 | 13.91 | 70.35 | 775.35 | 257.75 | 18/0 |
| 8 | 3.76 | 22.85 | 115.56 | 630.09 | 264.22 | 18/0 |

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
| `source` | bench_report(bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md) |
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
| `source` | bench_report(bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `ea66e41a7df4` 2026-10-25T22:23:02Z | a85af01ac8f4..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@ea66e41a7df4` | `git show ea66e41a7df455f1714c6da7663b52d07e2ea781:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `3fa6eac00a18` 2026-10-16T21:43:55Z | a85af01ac8f4..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@3fa6eac00a18` | `git show 3fa6eac00a185bb5f145fa5ecb6cae276601a3c9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | a85af01ac8f4..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | a85af01ac8f4..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_granite42-30b-v030/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

현재실제진입호출은보존된granite42-30b-v030_bench_invocation.json과bench.log의broad_search weak권한표지, main-full상태의이셀attempt에서관측된다(testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §2). 위FACT의driver-candidate는정적선택기탐색범위이며실제호출관측부재를뜻하지않는다. 측정시점이전마지막커밋의도구스냅샷과정확한실행당시바이트를결속한증명은구분하며후자는별도로수집하지않았다.

> [원문] run_bench.sh@3fa6eac00a18 §L445-450
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \


GuideLLM0.7.3·synthetic본문1024/출력256·요청목표16/워밍업2·1/2/4/8각3회·warm-rerun·mainTP1·openai-chat·bench4096·specoff다. 실제완료수는각runJSON을따르며선언16과같다고가정하지않는다. ignore_eos=true, 요청수제약과워밍업비율변환은위원문이다. 이명령은temperature/seed를명시하지않으며다른레그값을옮기지않는다. 새로운측정완료키는2026-10-27T01:17:15Z다.

> [원문] lite_bench.sh@90a3cdd56a94 §L177-181
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \


lite는512/128 random·range-ratio0·ignore-eos·cold1/warmup0와warm3/warmup1이다. chat템플릿포함실측평균을도구인자512로오인하지않는다. 일반completion reasoning_tokens0관측은전용reasoning시험N/A와별개다. 현재prefixcache카운터의실행차분은testlog§4와raw를따르며다른세션값을옮기지않는다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 이전10월10일동일셀GuideLLM Full · c1/2/4/8 3.93/3.96/3.92/3.83 · 같은1024/256 synthetic·GuideLLM0.7.3·요청16/warm2/반복3·BF16/KVauto/len131072/maxseq8·기존0.30이미지형상이나, driver선언580.173.02→580.178.04와도구커밋·실행시점차이가있다. 전체소프트지문일치를증명하지않았으므로비교불가이며재현/최적화효과로단정하지않는다(bench_report_26101016_granite-4.2-30b_GB10_0.30.0).

| 동시성 | 이번대표decode | 이전대표decode | 이번÷이전 |
|---|---|---|---|
| 1 | 3.9 | 3.93 | 0.9924 |
| 2 | 3.89 | 3.96 | 0.9823 |
| 4 | 3.85 | 3.92 | 0.9821 |
| 8 | 3.76 | 3.83 | 0.9817 |

비는각대표값한번의나눗셈·소수넷째자리반올림이다. 현재표의대표는첫유효run이며평균값이아니다. 반복별값·band는현재FACT/리포트를따른다.
- 이전lite3.89/3.90은별도512/128도구레그이고한경우멀티재사용이미지cdb1a38b…이므로현재Full과비교불가다. 다른모델·vLLM·TP의E2E자매셀도강한키가달라성능비교무효다.
- 외부E는이번bounded검색empty다. 대역폭추정4.5/5tps는실측baseline이아니며현재primary1.63/floor1.39/sourceexpected_achievable(roofline×MBU)는도구의의심기준이다.
- 이번새Full은lite→GuideLLM각3회·fresh측정바인딩·실제PASS로정의를충족한다. weak권한이라인증서없이report를바인딩했고사용자숫자목표나임계값을새로만들지않았다. specoff acceptance1.0은규칙입력이며실측acceptance승계없음이다. 다른HW·형상은재측정대상이며유일한차이를주장하지않는다.
