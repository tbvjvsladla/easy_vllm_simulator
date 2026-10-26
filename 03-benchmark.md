# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-26T23:00:21Z | sweep_index.generated_utc(output/single/benchlog/sweep_granite42-8b-v031) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 12.44 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 4.62 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-26T23:00:21Z | — |
| `lite.cold_ttft_ms` | 157.4 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_kv_gib` | 23.5 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) — 서빙 yaml `kv-cache-memory-bytes: 25232932864`(23.5 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/granite42-8b-v031.yaml) |
| `lite.engine_kv_tokens` | 154000 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_reserved_total_gib` | 39.92 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_weights_gib` | 16.42 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.gen_tps` | 12.68 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.kv_gib` | 23.5 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) — 서빙 yaml `kv-cache-memory-bytes: 25232932864`(23.5 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/granite42-8b-v031.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 5.43 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.291 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 12.44 | 12.44 | 0 | 79.63657659642837 | 1 | true | 12.454604692860897 | 80.37164248526096 | 266.2777900695801 |
| 2 | ok | 미기재 | 16 | 12.84 | 12.83 | 0 | 77.27395693461101 | 2 | true | 25.66142802100746 | 77.90727354586124 | 238.97504806518555 |
| 4 | ok | 미기재 | 18 | 12.57 | 12.62 | 0 | 78.86112437528722 | 4 | true | 45.542431089812496 | 79.5425558462739 | 237.593412399292 |
| 8 | ok | 미기재 | 15 | 12.0 | 12.12 | 0 | 82.61038181828518 | 8 | true | 61.596746689485954 | 83.30955635756254 | 248.4431266784668 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 12.44 | 12.45 | 63.00 | 266.28 | 79.64 | 16/0 |
| 2 | 12.84 | 25.66 | 129.81 | 238.98 | 77.27 | 16/0 |
| 4 | 12.57 | 45.54 | 230.38 | 237.59 | 78.86 | 18/0 |
| 8 | 12 | 61.60 | 311.59 | 248.44 | 82.61 | 15/0 |

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
| `source` | bench_report(bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `ea66e41a7df4` 2026-10-25T22:23:02Z | 7caad29c102d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@ea66e41a7df4` | `git show ea66e41a7df455f1714c6da7663b52d07e2ea781:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `3fa6eac00a18` 2026-10-16T21:43:55Z | 7caad29c102d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@3fa6eac00a18` | `git show 3fa6eac00a185bb5f145fa5ecb6cae276601a3c9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | 7caad29c102d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 7caad29c102d..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

현재핵심인자요약은testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §2이며전체shell원문으로위장하지않는다. 실제GuideLLM호출경로는측정시점원문이다:

실제 broad_search 드라이버 호출은 `docs/simlog/26102706_코덱스_메인4셀/granite42-8b-v031_bench_invocation.json`의 command 배열·started_utc와 같은 vault의 bench.log, 캠페인 main-full.json 셀 기록에서 관측된다. 호출 기록 자체가 없는 것이 아니다. 이 호출 관측과 특정 스냅샷 바이트가 실제 실행됐다는 암호학적 결속은 별개다. 실행 시점 파일 해시를 별도 캡처하지 않았으므로 정확한 실행 바이트 결속까지 관측했다고 주장하지 않는다.

> [원문] run_bench.sh@3fa6eac00a18 §L445-451
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

GuideLLM0.7.3·synthetic1024/256·요청목표16/워밍업2·1/2/4/8각3회·mainTP1·specoff다. 실제집계completed는run별원문을읽으며16고정을주장하지않는다. extras의ignore_eos=true와warmup비율변환을보존한다. temperature/seed는이GuideLLM명령에서명시하지않았으므로다른레그의값을옮기지않는다. 측정완료UTC는2026-10-26T23:00:21Z다.

lite는cold1요청warmup0과warm3요청warmup1의별도512/128 random레그다:

> [원문] lite_bench.sh@90a3cdd56a94 §L177-182
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

lite의range-ratio0·ignore-eos는원문과같고temperature는이명령에없다. 반복warm-rerun에서synthetic동일프롬프트가prefixcache에재적중하는것은현재카운터로기재한다. CNN의salt행오프셋과같은형상이라고부르지않는다. specoff규칙acceptance1.0은실측이아니다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102708_granite-4.2-8b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 이전Q6 CNN Full · c1/2/4/8값은아래표 · 현재synthetic대과거CNN/parquet·단독대두모델상주·driver선언580.173.02→580.178.04 · 비교불가 — 부하와상주조건이다르므로재현/최적화효과로해석하지않는다(bench_report_26101707_granite-4.2-8b_GB10_0.31.0).

| 동시성 | 이 측정 | 이전 측정 | 비 |
|---|---|---|---|
| 1 | 12.44 | 12.36 | 1.0065 |
| 2 | 12.84 | 12.64 | 1.0158 |
| 4 | 12.57 | 12.16 | 1.0337 |
| 8 | 12.0 | 11.18 | 1.0733 |

비는현재÷이전한번의나눗셈·소수넷째자리반올림이다. 자체밴드보다큰차이가있어도동일범위재현으로부르지않고원인대조가없음을밝힌다. 더이른CNN측정은12.46/12.67/12.15/11.11이었으며c1대표TTFT가부분대체됐다(기존testlog머리). Q6의12.36/12.64/12.16/11.18을가장최근유효CNN대조로택했다.
- 같은모델의다른형상 · 과거lite12.55·512/128·vllm-bench-serve·반복축없음 · 비교불가 — 현Full과도구·부하다르다. MiniCPM은다른모델이므로같은모델자매형상성능순위에넣지않는다.
- 외부E · bounded동일GB10 BF16 decode검색결과없음 · 비교불가 — 모델카드배포설명은성능baseline이아니다(testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정 §3).
- 손승계입력 · specoff규칙acceptance1.0외실측acceptance승계없음. 현재primary5.43·floor4.62·expected_achievable(roofline×MBU)·weak·Eempty다.

현재는lite+GuideLLM레벨당3회로Full정의를충족하고실제PASS다. weak권한이라인증서없이report를바인딩했다. 인증서소프트지문전항목을검증했다고주장하지않고유일한차이를단정하지않는다. 다른HW·드라이버·부하·서빙형상은재측정대상이며판정표시가독립사실검증을대체하지않는다.
