# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-27T07:49:00Z | sweep_index.generated_utc(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 34.36 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 32.19 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-27T07:49:00Z | — |
| `lite.cold_ttft_ms` | 89.9 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.engine_kv_gib` | 37.9 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) — 서빙 yaml `kv-cache-memory-bytes: 40694815129`(37.9 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/lfm25-26b-v031.yaml) |
| `lite.engine_kv_tokens` | 2476552 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.engine_reserved_total_gib` | 42.95 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.engine_weights_gib` | 5.05 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.gen_tps` | 34.95 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) |
| `lite.kv_gib` | 37.9 | sweep_index.lite(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02) — 서빙 yaml `kv-cache-memory-bytes: 40694815129`(37.9 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/lfm25-26b-v031.yaml) |
| `primary_source` | E(external_reference) | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 37.87 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.907 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 34.36 | 34.33 | 0 | 28.763399872125365 | 1 | true | 34.36551734212728 | 29.10511940717697 | 116.34373664855957 |
| 2 | ok | 미기재 | 16 | 40.74 | 40.75 | 0 | 24.320491154988606 | 2 | true | 81.50988612849358 | 24.54325556755066 | 81.16841316223145 |
| 4 | ok | 미기재 | 18 | 40.18 | 40.04 | 0 | 24.678255529964673 | 4 | true | 144.504737256873 | 24.889908730983734 | 78.66644859313965 |
| 8 | ok | 미기재 | 18 | 39.65 | 39.72 | 0 | 24.997990739111806 | 8 | true | 240.37298500237756 | 25.21989680826664 | 82.17740058898926 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 34.36 | 34.37 | 173.17 | 116.34 | 28.76 | 16/0 |
| 2 | 40.74 | 81.51 | 410.73 | 81.17 | 24.32 | 16/0 |
| 4 | 40.18 | 144.50 | 728.17 | 78.67 | 24.68 | 18/0 |
| 8 | 39.65 | 240.37 | 1211.25 | 82.18 | 25.00 | 18/0 |

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
| `source` | bench_report(bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | b799890f7e49..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `1629609b34df` 2026-10-16T21:44:18Z | b799890f7e49..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@1629609b34df` | `git show 1629609b34dfe6851338eb7b8353411a1016924d:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `a7083a8f933d` 2026-10-17T13:17:27Z | b799890f7e49..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@a7083a8f933d` | `git show a7083a8f933d98cd6d739fdac838ebbe3e3b7d3f:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | b799890f7e49..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_lfm25-26b-v031_camp-26102705_a02/sweep_index.json · generated_utc = measured_utc 조인)
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

GuideLLM0.7.3 default full: 입력1024/출력256, 동시성1/2/4/8, 선언 측정16+warmup2, 각 레벨3회 warm-rerun, node sub, synthetic, 측정 색인UTC2026-10-27T07:49:00Z다. GuideLLM 명령은 ignore_eos=true와 warmup 비율/총 요청 constraint를 넘긴다. 위 GuideLLM 레그는 temperature/seed를 명시하지 않았으므로 vLLM 레그의 temperature0을 여기로 옮기지 않는다. lite 레그는 별도로 cold1/warm3, random512/128·range-ratio0·ignore-eos를 쓴다. lite temperature/seed 명시는 위 스냅샷에 없으므로 미명시로 남긴다. spec off이고 acceptance length 미측정을 별도spec 관측으로 채우지 않았다. 01의 명령 복원은 실행 원문이 아니며 위 스냅샷은 측정 도구의 해당 커밋 원문이다. OS 수준 실행 바이트의 독립 증명은 없다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md` 원문 그대로):

> 미관측

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 미관측 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102716_lfm2.5-2.6b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 미관측 | .claude/rules/docs.md 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이전 같은 모델의 CNN Full은 다음과 같다. 비는 이번 대표값÷이전 대표값 한 번의 나눗셈을 소수4자리 반올림했다. 데이터셋·토큰 구성·요청 완료 집계와 드라이버 선언이 달라 성능 재현/우열의 근거로 사용하지 않는다.

| 동시성 | 이 측정 | 이전 CNN 측정 | 비 |
|---|---|---|---|
| 1 | 34.36 | 35.32 | 0.9728 |
| 2 | 40.74 | 39.9 | 1.0211 |
| 4 | 40.18 | 39.07 | 1.0284 |
| 8 | 39.65 | 36.67 | 1.0813 |

- 이전 CNN Full — 모든 레벨은 위 표 — 비교 불가: parquet CNN/seed42/전처리·이전 드라이버580.173.02와 이번 synthetic/드라이버 선언580.178.04가 다르다. 인증서 소프트 지문 전체의 일치를 확인하지 않았으며 유일한 차이라고 주장하지 않는다(bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0 §측정 구성·측정 환경 스냅샷).
- 이전 lite36.22 및36.25tok/s — 비교 불가: cold/warm vllm-bench-serve 스냅샷이고 GuideLLM 반복 Full이 아니다. 해당 lite 보고서에는 상위 동시성 곡선이 없다(bench_report_26101411_lfm2.5-2.6b_GB10_0.31.0, bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0).
- 같은 모델의 과거 KV10GiB 형상 — 기능/용량 검증은 있지만 동일 부하의 Full 속도 곡선은 없어 속도 우열은 판정 불가다(testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §2. 트라이얼 이력).
- 외부 E37.87tok/s — 비교 불가: 같은 BF16/GB10이지만 vLLM0.26.0·llama-benchy·출력1024·c10 per-user이며 이번0.31.0 build-input·GuideLLM·출력256·c1/2/4/8과 조건이 다르다. E는 측정 전 선택한 cross-version rubric 상수이며 동일 프로토콜 baseline이라고 부르지 않는다(testlog_26102716_lfm25_default_full_신규판정 §Protocol and external comparison).

이번은 full=lite와 각 유효 레벨 반복3을 충족하고 weak PASS로 표시한다. explicit 인증서는 없으며 판정은 태그 표시이고 발행 자격과 별개다. expected17.71/floor32.19와 E37.87의 출처를 보존했고, spec off의 accept_len1.0은 미선언 기본값이지 과거 측정에서 손으로 빌린 값이 아니다. 조건·이미지·핀·HW가 달라지면 새 레시피와 측정이 필요하며 이 수치를 현행 baseline으로 자동 승격하지 않는다.
