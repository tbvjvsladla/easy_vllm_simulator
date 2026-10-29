# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-29T09:39:38Z | sweep_index.generated_utc(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102918_minicpm5-2b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 36.98 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 16.13 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-29T09:39:38Z | — |
| `lite.cold_ttft_ms` | 58.0 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.engine_kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) — 서빙 yaml `kv-cache-memory-bytes: 37580963840`(35.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/minicpm5-2b-v031.yaml) |
| `lite.engine_kv_tokens` | 873808 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.engine_reserved_total_gib` | 39.75 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.engine_weights_gib` | 4.75 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.gen_tps` | 37.52 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| `lite.kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) — 서빙 yaml `kv-cache-memory-bytes: 37580963840`(35.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/minicpm5-2b-v031.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 18.98 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.948 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 36.98 | 36.96 | 0 | 26.719328001433727 | 1 | true | 36.98860381051059 | 27.04205922782421 | 109.29036140441895 |
| 2 | ok | 미기재 | 16 | 45.79 | 45.82 | 0 | 21.522870718264112 | 2 | true | 91.71251771745325 | 21.841157227754593 | 95.66044807434082 |
| 4 | ok | 미기재 | 14 | 45.07 | 45.18 | 0 | 21.886538524253695 | 4 | true | 158.94234844069382 | 22.185778245329857 | 96.8787670135498 |
| 8 | ok | 미기재 | 18 | 43.12 | 43.34 | 0 | 22.90058416478774 | 8 | true | 264.79476700451596 | 23.19287694990635 | 96.85111045837402 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 동시 요청 수를 부하 축으로 표시한다. 값이 있는 행은 원천에 기록된 해당 레벨의 대표 run(첫 완주 run)을 나타낸다.
> 반복 run의 warmup·요청 수·완료 결과는 서로 다를 수 있으므로 원천 기록을 함께 확인한다.
> `★판정점`은 원천이 표시한 판정 지점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다. 인증서 발행 여부는 판정과 루브릭 권한에 따른다.
> 출처: `bench_report_26102918_minicpm5-2b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 36.98 | 36.99 | 186.10 | 109.29 | 26.72 | 16/0 |
| 2 | 45.79 | 91.71 | 461.43 | 95.66 | 21.52 | 16/0 |
| 4 | 45.07 | 158.94 | 799.68 | 96.88 | 21.89 | 14/0 |
| 8 | 43.12 | 264.79 | 1332.25 | 96.85 | 22.90 | 18/0 |

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
| `source` | bench_report(bench_report_26102918_minicpm5-2b_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102918_minicpm5-2b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `317bae8db936` 2026-10-27T07:18:59Z | 94ee7eee7562..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@317bae8db936` | `git show 317bae8db93631f8e472743c6b8075dffe114605:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `3fa6eac00a18` 2026-10-16T21:43:55Z | 94ee7eee7562..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@3fa6eac00a18` | `git show 3fa6eac00a185bb5f145fa5ecb6cae276601a3c9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | 94ee7eee7562..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `317bae8db936` 2026-10-27T07:18:59Z | 94ee7eee7562..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@317bae8db936` | `git show 317bae8db93631f8e472743c6b8075dffe114605:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이번 실제 broad_search 전체 호출은 main-bench-command.txt에 보존하고 testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정에서 연결한다. 01 §1.4의 bench JSON 재구성 명령과 이 실제호출기록은 다른 표면이다. GuideLLM 각 레그는 아래 측정시점 도구스냅샷을 참고한다.

> [원문] run_bench.sh@3fa6eac00a18 §L445-451
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

GuideLLM0.7.3·synthetic입력1024/출력256·목표요청16/워밍업2·1/2/4/8각3회·main·TP1·spec off다. 시각은2026-10-29T09:26:11Z~09:39:38Z, 대표값은첫완주run이다. 실제집계요청은대표c1/c2=16,c4=14,c8=18이며warmup비율변환의근사와구분한다. GuideLLM extras는ignore_eos true이며temperature·seed를이명령에서명시하지않았다. 이를vllm레그의temperature0과섞지않는다.

lite는cold1요청warmup0과warm3요청warmup1의512/128 random측정이고별도도구vllm bench serve다:

> [원문] lite_bench.sh@90a3cdd56a94 §L177-182
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

lite의range-ratio0·ignore-eos와warmup원문은위와같으며temperature는이레그원문에없다. 본Full의acceptance는N/A이며specoff규칙의1.0을실측이라고부르지않는다.

호출 관측의 범위: 새 vault의bench.log와 main-bench-command.txt 및 새 캠페인 main-full.json이 broad_search 호출을 기록한다. FACT의 정적 선택기는 호출기록을 조회하지 않으므로 driver-candidate라고 표시한다. 별도 실제호출 기록의 존재와 측정시점 선택된 정적스냅샷 바이트가 실제로 실행됐는지의 별도 결속은 구분한다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102918_minicpm5-2b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102918_minicpm5-2b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 이전10월27일 synthetic Full · 같은모델·GB10·TP1·이미지/서빙노브·GuideLLM0.7.3 default 형상이며 다른 실행시각 · 형상 비교 가능, 실행당 실집계요청과 상태 차이는 남는다 — 인증서가 없어 소프트지문 전수 동등성이나 단일원인을 단정하지 않는다(testlog_26102707_minicpm5_2b_v031_코덱스_메인_Full_판정 §2·§4).

| 동시성 | 이 측정 | 이전 측정 | 비 |
|---|---|---|---|
| 1 | 36.98 | 37.34 | 0.9904 |
| 2 | 45.79 | 45.77 | 1.0004 |
| 4 | 45.07 | 44.92 | 1.0033 |
| 8 | 43.12 | 43.15 | 0.9993 |

비는 현재÷이전 한 번의 나눗셈, 소수넷째자리 반올림이다. 이번 자체 레벨별 밴드는03의 사실표이고, 두 실행의 차이를 자체밴드 안 재현이나 성능개선으로 부르지 않는다. 원인은 대조하지 않았다. 특히 대표c4의 실집계요청은 이번14·이전18로 같지 않다.

- 이전CNN Full · c1 38.13,c2 45.19,c4 43.61,c8 40.41 · 기사본문/템플릿·동시 두모델상주·과거driver선언580.173.02 대 이번synthetic·단독서빙·driver선언580.178.04 · 비교 불가 — 부하와 동시상주조건이 다르다(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0).
- 같은모델 별도lite ·39.20t/s · random512/128·vllm bench serve·1회 · 비교 불가 — Full곡선과 도구·부하·반복이 다르다(bench_report_26101621_minicpm5-2b_GB10_0.31.0). 다른모델 LFM·Granite는 같은모델 대조가 아니다.
- 외부E ·10월27일 bounded검색의 동일GB10 BF16 decode값 공백 판단을 재사용 · 비교 불가 — 모델카드는 배포근거이며 성능baseline이 아니다. 새 검색을 수행했다고 주장하지 않는다(testlog_26102918_minicpm5_2b_E2E_Full_판정 §새 Full 측정).
- 판정입력 ·expected18.98,floor16.13·weak·Eempty·사람숫자목표없음. specoff 규칙acceptance1.0은 미선언에 따른 규칙값이며 실측acceptance 승계가 아니다. measured acceptance는N/A다.

현재 측정은lite∪GuideLLM 각레벨3회로 현행Full정의를 충족하고 PASS다. weak권한이므로 인증서 없이report를 바인딩했다. 판정은표시이며 hint독립사실검증을 대체하지 않는다. 다른HW·드라이버·부하·서빙형상은 재측정대상이다.
