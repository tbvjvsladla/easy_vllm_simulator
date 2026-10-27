# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-27T10:05:40Z | sweep_index.generated_utc(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102719_qwen3-4b_GB10_0.26.0.md)) |
| `decode_tps_conc1` | 21.68 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 10.1 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-27T10:05:40Z | — |
| `lite.cold_ttft_ms` | 84.7 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.engine_kv_gib` | 11.0 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) — 서빙 yaml `kv-cache-memory-bytes: 11811160064`(11.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/c-tq4nc.yaml) |
| `lite.engine_kv_tokens` | 232992 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.engine_reserved_total_gib` | 18.56 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.engine_weights_gib` | 7.56 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.gen_tps` | 22.24 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) |
| `lite.kv_gib` | 11.0 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z) — 서빙 yaml `kv-cache-memory-bytes: 11811160064`(11.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/single/configs/c-tq4nc.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 11.88 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.825 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 21.68 | 21.57 | 0 | 45.55164785946117 | 1 | true | 21.589723901835452 | 46.12222872674465 | 192.43836402893066 |
| 2 | ok | 미기재 | 16 | 25.52 | 25.54 | 0 | 38.62453254998899 | 2 | true | 51.116862359305095 | 39.18375261127949 | 179.55565452575684 |
| 4 | ok | 미기재 | 18 | 24.3 | 24.44 | 0 | 40.540585798375744 | 4 | true | 88.52697121648535 | 41.147736832499504 | 196.1495876312256 |
| 8 | ok | 미기재 | 18 | 22.22 | 22.55 | 0 | 44.26192302329868 | 8 | true | 140.10332772130283 | 45.00435013324022 | 235.19563674926758 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102719_qwen3-4b_GB10_0.26.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 21.68 | 21.59 | 108.62 | 192.44 | 45.55 | 16/0 |
| 2 | 25.52 | 51.12 | 257.18 | 179.56 | 38.62 | 16/0 |
| 4 | 24.30 | 88.53 | 445.40 | 196.15 | 40.54 | 18/0 |
| 8 | 22.22 | 140.10 | 704.89 | 235.20 | 44.26 | 18/0 |

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
| `source` | bench_report(bench_report_26102719_qwen3-4b_GB10_0.26.0.md) |
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
| `source` | bench_report(bench_report_26102719_qwen3-4b_GB10_0.26.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 5a724c7e3045..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `1629609b34df` 2026-10-16T21:44:18Z | 5a724c7e3045..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@1629609b34df` | `git show 1629609b34dfe6851338eb7b8353411a1016924d:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `a7083a8f933d` 2026-10-17T13:17:27Z | 5a724c7e3045..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@a7083a8f933d` | `git show a7083a8f933d98cd6d739fdac838ebbe3e3b7d3f:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `b799890f7e49` 2026-10-27T07:28:50Z | 5a724c7e3045..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@b799890f7e49` | `git show b799890f7e4996fecf84b3999b9b0100909f80b6:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_c-tq4nc_camp-26102705_qwen_a01_20261027T093346Z/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 이 선택기는 호출 기록을 조회하지 않는다(정적 소스 기반 후보이며 실제 드라이버 여부는 이 선택기에서 미검증). 별도 호출 관측과 이 판본의 실제 실행 바이트 결속은 구분한다
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

GuideLLM0.7.3, sub 단일노드의 기본1024/256/16/warmup2, 레벨1/2/4/8×3회 warm-rerun이다. spec off, measured accept_len=null이며 루브릭1.0은 선언 부재 처리다. measured_utc는2026-10-27T10:05:40Z다. 실제 broad argv는 이번 simlog bench_invocation.json에 보존했고 1.4의 명령은 재구성이라고 구분한다.

> [원문] docs/testlog/testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정.md §이번 실행
> 모든 반복 failed0/error_rate0/measurement_ok=true. 선언 input1024/output256/num_prompts16/warmup2/repeats3/synthetic, backend=openai-chat, bench_budget8192MiB. max-num-seqs8 필터로16 제외. 레벨1/2 parser completed/num_prompts16, 레벨4/8은18, 서버 request_success_total 증가분은 모두18이다. 워밍업 선언2건과 양립하지만 높은 레벨의 집계18 현상 원인은 미확정이다. 선언16을 실측18로 바꾸지 않았다.

Full 레그의 도구 인자는 다음 원문이다. 샘플링 seed/temperature를 원문에 없는 값으로 단정하지 않는다.

> [원문] run_bench.sh@1629609b34df §L445-452
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \
>     --disable-progress \

Lite cold/warm은 별도 vLLM bench 레그이며 아래 리터럴의 입력512/출력128 및 random-range-ratio/ignore-eos를 쓴다. chat template 포함 실측520을 입력 인자512와 혼동하지 않는다.

> [원문] lite_bench.sh@a7083a8f933d §L180-183
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \
>       && docker exec "$CTR" cat /tmp/lite_tmp.json > "$1"

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md` 원문 그대로):

> 미관측

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102719_qwen3-4b_GB10_0.26.0.md)) |
| 현행 정의의 반복 요건 | 미관측 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102719_qwen3-4b_GB10_0.26.0.md) |
| 현행 full 정의 충족 | 미관측 | .claude/rules/docs.md 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

| 동시성 | 이 측정 | 이전10-10 | 비(이 측정/이전, 소수4자리) |
|---|---|---|---|
| 1 | 21.68 | 22.44 | 0.9661 |
| 2 | 25.52 | 26.26 | 0.9718 |
| 4 | 24.3 | 25.0 | 0.972 |
| 8 | 22.22 | 22.77 | 0.9758 |

- 같은 셀10-10 기본형상은 같은 default 부하와 도구지만 driver 선언580.173.02→580.178.04, 반복 출처 defaulted→declared, 양자화/attention 관측 표면과 E-search 상태가 다르다. 인증서가 없는 weak 기록끼리 소프트 지문 전체 일치를 증명하지 못해 통제 비교는 불가이며 형상 참고로만 둔다. 차이는 현재 밴드로 재현됐다고 부르지 않고 원인은 미확정이다(두 bench_report 및 testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §이전 기록과 비교 범위).
-9-08 자매3bit/4bit는 dtype·부하8192/1024/32와 레벨1/2/4가 다르므로 이번과 비교 불가다. 당시 각 레벨 decode(mean)는 C18.56/37.58/55.16 대 D17.58/35.32/51.21로 보고됐다. 이번 decode 통계와도 의미가 같다고 단정하지 않는다(testlog_26090807_qwen3_4b_tq_kvquant_서브 §벤치 계측).
- 외부 H20 표와 다른 Instruct-2507/NVFP4 Spark 수치는 모델·HW·runtime·부하 대응이 부족해 비교 불가/E미채택이다(testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §외부 검색).

이번은 공식 full mode/PASS, 약한 권한의 roofline-only 판정이다. primary11.88/floor10.1/ratio1.825이며 E/c는 null, tolerance0.15와 MBU0.35는 결정론 루브릭 입력이다. spec off의 accept_len1.0은 측정값이 아니다. weak라 인증서는 없고 bench_report가 운반한다. raw는 lite와 GuideLLM 각 레벨3회라는 현행 스킬 정의를 충족한다. FACT의 docs.md 정의 조회는 해당 파일 부재라는 제한된 범위를 유지하며, 현재 세션 귀속은 정규 journal/event로 재파생됐다. 모듈 버전 기록과 distribution metadata도 별도 행으로 분리했다. 변경된 FACT와 산문은 독립 검증자가 다시 대조한다. 성능 판정은 표시이며 발행은 독립 검증과 기존 승인 등 별도 게이트가 필요하다.
