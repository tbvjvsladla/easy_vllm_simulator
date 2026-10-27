# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-26T22:13:07Z | sweep_index.generated_utc(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102707_minicpm5-2b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 37.34 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 16.13 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-26T22:13:07Z | — |
| `lite.cold_ttft_ms` | 55.5 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) — 서빙 yaml `kv-cache-memory-bytes: 37580963840`(35.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/minicpm5-2b-v031.yaml) |
| `lite.engine_kv_tokens` | 873808 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_reserved_total_gib` | 39.75 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_weights_gib` | 4.75 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.gen_tps` | 37.99 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) — 서빙 yaml `kv-cache-memory-bytes: 37580963840`(35.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/minicpm5-2b-v031.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 18.98 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.967 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 37.34 | 37.35 | 0 | 26.466566908593272 | 1 | true | 37.38263058548054 | 26.78125537931919 | 106.2173843383789 |
| 2 | ok | 미기재 | 16 | 45.77 | 45.8 | 0 | 21.544869740804035 | 2 | true | 91.656555384909 | 21.849372424185276 | 94.42734718322754 |
| 4 | ok | 미기재 | 18 | 44.92 | 44.98 | 0 | 22.02638551300647 | 4 | true | 162.50201647809493 | 22.26307336241007 | 81.10642433166504 |
| 8 | ok | 미기재 | 18 | 43.15 | 43.41 | 0 | 22.859828612383673 | 8 | true | 265.34022541769315 | 23.17754738032818 | 101.715087890625 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102707_minicpm5-2b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 37.34 | 37.38 | 188.08 | 106.22 | 26.47 | 16/0 |
| 2 | 45.77 | 91.66 | 461.15 | 94.43 | 21.54 | 16/0 |
| 4 | 44.92 | 162.50 | 817.59 | 81.11 | 22.03 | 18/0 |
| 8 | 43.15 | 265.34 | 1334.99 | 101.72 | 22.86 | 18/0 |

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
| `source` | bench_report(bench_report_26102707_minicpm5-2b_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102707_minicpm5-2b_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `ea66e41a7df4` 2026-10-25T22:23:02Z | `317bae8db936` 2026-10-27T07:18:59Z | `sweep_bench.sh@ea66e41a7df4` | `git show ea66e41a7df455f1714c6da7663b52d07e2ea781:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `3fa6eac00a18` 2026-10-16T21:43:55Z | 88e5a467c89b..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@3fa6eac00a18` | `git show 3fa6eac00a185bb5f145fa5ecb6cae276601a3c9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `90a3cdd56a94` 2026-10-17T13:16:25Z | 88e5a467c89b..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@90a3cdd56a94` | `git show 90a3cdd56a94a2c3db79c1694112d9385cc8ba5a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | `317bae8db936` 2026-10-27T07:18:59Z | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_minicpm5-2b-v031/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이번 명령의핵심인자요약은testlog_26102707_minicpm5_2b_v031_코덱스_메인_Full_판정 §2에있다. 전체shell호출원문을그요약으로위장하지않는다. GuideLLM실제실행경로는측정시점도구스냅샷이다.

> [원문] run_bench.sh@3fa6eac00a18 §L445-451
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

GuideLLM0.7.3·synthetic입력1024/출력256·목표요청16/워밍업2·1/2/4/8각3회·main·TP1·spec off다. 시각은2026-10-26T21:59:42Z~22:13:07Z, 대표값은첫완주run이다. 실제집계요청은대표c1/c2=16,c4/c8=18이며warmup비율변환의근사와구분한다. GuideLLM extras는ignore_eos true이며temperature·seed를이명령에서명시하지않았다. 이를vllm레그의temperature0과섞지않는다.

lite는cold1요청warmup0과warm3요청warmup1의512/128 random측정이고별도도구vllm bench serve다:

> [원문] lite_bench.sh@90a3cdd56a94 §L177-182
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

lite의range-ratio0·ignore-eos와warmup원문은위와같으며temperature는이레그원문에없다. 본Full의acceptance는N/A이며specoff규칙의1.0을실측이라고부르지않는다.

호출 관측의 범위: 같은 vault의 `minicpm5-2b-v031_bench.log`에 broad_search 표지가 있고 `campaigns/camp-26102705-codex-single8/sweeps/main-full.json`에 해당 셀 시도가 남아 있다. 이 첫 MiniCPM 실행에는 별도 전체 argv JSON을 보존하지 않았으며 testlog 명령은 핵심 인자 요약이다. 호출 관측이 없다는 뜻과, 선택한 정적 스냅샷 바이트의 실제 실행 결속을 별도 관측하지 않았다는 뜻을 구분한다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102707_minicpm5-2b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102707_minicpm5-2b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 이전CNN Full · c1 38.13,c2 45.19,c4 43.61,c8 40.41 · 입력기사1024본문+템플릿,이번synthetic·단독서빙대과거두모델상주·driver선언580.173.02→580.178.04 · 비교불가 — 데이터셋·실집계요청수·동시상주조건이달라재현/개선으로해석하지않는다(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0).

| 동시성 | 이 측정 | 이전 측정 | 비 |
|---|---|---|---|
| 1 | 37.34 | 38.13 | 0.9793 |
| 2 | 45.77 | 45.19 | 1.0128 |
| 4 | 44.92 | 43.61 | 1.0300 |
| 8 | 43.15 | 40.41 | 1.0678 |

비는현재÷이전한번의나눗셈·소수넷째자리반올림이며조건이다른관측의산술표다. 같은범위재현이나원인이입증된성능차이라고부르지않는다. 기존자체밴드보다큰차이가있지만인과대조는없다.

- 같은모델자매형상 · 별도lite39.20t/s · random512/128·vllm-bench-serve·반복1 · 비교불가 — Full레벨곡선과도구·부하가다르다(bench_report_26101621_minicpm5-2b_GB10_0.31.0). 다른모델Granite는자매런너이지같은모델대조가아니다.
- 외부성능E · bounded검색에서동일GB10 BF16 decode값없음 · 비교불가 — 모델카드는배포근거이며성능baseline이아니다(신규testlog §3).
- 판정입력 · expected18.98,floor16.13·weak·Eempty·사용자숫자목표없음. specoff규칙acceptance1.0은미선언에대한규칙값이고이전실측acceptance를손승계한것이아니다.

현재측정은lite+GuideLLM레벨당3회로현행Full정의를충족하며판정PASS다. weak권한이라인증서는없고report를바인딩했다. 판정은표시규칙이며hint발행의독립사실검증을대체하지않는다. 다른HW·드라이버·부하·서빙형상은재측정대상이다. 인증서가없어완전한소프트지문동등성을검증했다고주장하지않으며유일한차이를단정하지않는다.
