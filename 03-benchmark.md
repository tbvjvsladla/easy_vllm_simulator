# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-16T07:54:23Z | sweep_index.generated_utc(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 38.13 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 16.13 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-16T07:54:23Z | — |
| `lite.cold_ttft_ms` | 33.8 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_kv_tokens` | 873808 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_reserved_total_gib` | 39.75 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.engine_weights_gib` | 4.75 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.gen_tps` | 38.97 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `lite.kv_gib` | 35.0 | sweep_index.lite(output/single/benchlog/sweep_minicpm5-2b-v031) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 18.98 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.009 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_minicpm5-2b-v031 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 38.13 | 38.14 | 0 | 25.913021611232384 | 1 | true | 38.17672057184363 | 26.228186674416065 | 106.70328140258789 |
| 2 | ok | 미기재 | 16 | 45.19 | 45.19 | 0 | 21.57618859234978 | 2 | true | 90.64553718231204 | 22.12646696716547 | 162.8739833831787 |
| 4 | ok | 미기재 | 16 | 43.61 | 43.72 | 0 | 21.98041747598087 | 4 | true | 141.8987013799454 | 22.932428866624832 | 226.4101505279541 |
| 8 | ok | 미기재 | 16 | 40.41 | 40.81 | 0 | 22.967735926310223 | 8 | true | 228.19757996350432 | 24.746978655457497 | 471.29130363464355 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 38.13 | 38.18 | 192.23 | 106.70 | 25.91 | 16/0 |
| 2 | 45.19 | 90.65 | 456.41 | 162.87 | 21.58 | 16/0 |
| 4 | 43.61 | 141.90 | 714.48 | 226.41 | 21.98 | 16/0 |
| 8 | 40.41 | 228.20 | 1149.01 | 471.29 | 22.97 | 16/0 |

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
| `source` | bench_report(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:dataset_provenance -->
### 데이터셋 출처

> 리포트 측정 구성 표 · 인증서 소프트 지문이 **기재한** 데이터셋 출처다(파싱만 · 합성 ✗ · `/app/` 밖 호스트 경로는 이름만).

| 키 | 값 |
|---|---|
| `dataset_kind` | parquet_file |
| `dataset_source` | managed |
| `dataset_id` | abisee/cnn_dailymail 3.0.0 test |
| `dataset_revision` | 96df5e686bee6baa90b8bee7c28b81fa3fa6223d |
| `dataset_source_sha256` | 04e322d2634a96dba76bf9a6294fbbe48e0b36abeae43f13d86ba2c3bebffe4e |
| `dataset_column_map` | {"text_column": "article"} |
| `dataset_preprocess_strategy` | concatenate |
| `dataset_seed` | 42 |
| `dataset_duplicate_sampling` | False |
| `dataset_tokenizer` | /app/models/Openbmb/MiniCPM5/MiniCPM5-2B |
| `dataset_provenance` | 미기재 |
| `status` | recorded |
| `source` | bench_report(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md) 측정 구성 표 |
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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | `7db9d67706df` 2026-10-16T07:56:04Z | `sweep_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | `7db9d67706df` 2026-10-16T07:56:04Z | `run_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `803064d23d51` 2026-10-06T06:17:10Z | 12ef50a89b86..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@803064d23d51` | `git show 803064d23d516fc72600077cf65e9f6c2a8775d5:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 12ef50a89b86..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_minicpm5-2b-v031/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**도구 원문 스냅샷과 실제로 돈 바이트가 다르다.** 위 표의 `run_bench.sh@2661d614cf52` 는 측정 시각 이전 마지막 **커밋**이고, 이 측정은 데이터셋 knob 를 넣은 커밋(7db9d67) 직전의 워킹트리로 돌았다(testlog_26101619_54_16 머리 "측정은 커밋 직전 워킹트리 · 같은 내용"). 그래서 스냅샷의 GuideLLM 호출은 아직 합성 텍스트 고정이다 — 이 줄은 이 측정의 입력이 **아니다**:

> [원문] run_bench.sh@2661d614cf52 §L398-401
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \

이 측정의 실제 GuideLLM 구성은 결과 JSON 이 스스로 적는다(레벨 1 첫 런 · 명령 줄 원문은 계보에 없다 — 01 §1.4 의 재구성 명령은 lite 레그뿐이다):

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/level_01/guidellm_minicpm5-2b-v031.json §L1-1
> "data": [{"kind": "json_file", "load_kwargs": {"split": "train"}, "path": "/out/dataset_rows.jsonl"}]

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/level_01/guidellm_minicpm5-2b-v031.json §L1-1
> "column_mappings": {"text_column": "article", "prompt_tokens_count_column": "prompt_tokens_count", "output_tokens_count_column": "output_tokens_count"}

lite 레그(같은 스윕의 진입 게이트)는 아래 원문 인자로 돌았다:

> [원문] lite_bench.sh@803064d23d51 §L172-176
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

**측정 조건(GuideLLM 레벨 측정)**

- 도구 · 버전: GuideLLM 0.7.3(`ghcr.io/vllm-project/guidellm:v0.7.3` · digest 일치 관측 — level_01 `bench_tool_*.json`).
- 백엔드: `openai_http` · `/v1/chat/completions`(openai-chat · chat 템플릿 적용) · `ignore_eos: true`(결과 JSON `extras`).
- 입력: CNN/DailyMail `article` 을 concatenate 로 1024 토큰 목표로 전처리 → 이 토크나이저에서 목표 1025(BOS 포함) · 런마다 겹치지 않는 연속 18 행 · seed `static 42`(03 §3.3 · 런 사이드카). 실측 평균 입력 1032 · 출력 256 토큰(chat 템플릿 포함 · testlog_26101619_54_16 §2).
- 출력: 256 토큰(ignore_eos 로 고정).
- 레벨 · 요청: 동시성 1 · 2 · 4 · 8(16 은 max-num-seqs 8 로 걸러짐) · 런마다 요청 16 + warmup 2(결과 JSON `max_requests` 18 · warmup 0.1111).
- 반복: 3(완주 3 — 3.3 표).
- spec: off(03 §3.1 `spec_on` false).
- 샘플링 temperature: 결과 JSON 과 스냅샷 GuideLLM 호출 원문에 temperature 인자가 없다(도구 기본값 — 값은 미기록).
- 벤치 컨테이너 예산: 4096 MiB(devlog_26101619 §5 재현 명령 `--bench-budget-mib 4096`).

**측정 조건(lite 레그)** — vllm bench serve · random 512/128 · range-ratio 0 · ignore-eos · 동시성 1 · cold 1 요청(warmup 0) + warm 3 요청(warmup 1) · temperature 인자 원문에 없음(위 원문). lite 는 판정점이 아니라 진입 게이트다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- **같은 셀의 이전 측정(2차 스윕 · bench_report_26101616_minicpm5-2b_GB10_0.31.0 · SUPERSEDED)** · c=1 decode 38.58 t/s · 조건 차이: 입력 행 반복(`dataset_duplicate_sampling` True 대 False — 두 리포트의 측정 구성 표 · 측정 환경 스냅샷에서 다른 키는 이것 하나 · 강한 일치 키 · 소프트 지문 키 동일) · **비교 불가** — 부하 조건(입력)이 다르다. 아래 표는 기록이다(값 = 두 리포트의 글자 그대로 · 비 = 이 측정 ÷ 이전 · 소수 3자리 반올림):

| 동시성 | 이 측정 decode t/s | 이전 측정 decode t/s | 비 |
|---|---|---|---|
| 1 | 38.13 | 38.58 | 0.988 |
| 2 | 45.19 | 45.98 | 0.983 |
| 4 | 43.61 | 45.16 | 0.966 |
| 8 | 40.41 | 44.0 | 0.918 |

| 동시성 | 이 측정 TTFT p50(ms) | 이전 측정 TTFT p50(ms) | 비 |
|---|---|---|---|
| 1 | 106.70328140258789 | 64.90707397460938 | 1.644 |
| 2 | 162.8739833831787 | 95.38578987121582 | 1.708 |
| 4 | 226.4101505279541 | 91.53485298156738 | 2.473 |
| 8 | 471.29130363464355 | 90.64626693725586 | 5.199 |

  이 측정 자신의 재현 밴드(bench_report_26101616_54_23 반복 축 · c=1 0.44% · c=8 0.12%)를 크게 넘는 차이라 "같은 범위 재현" 이 아니다. 원인은 설명되지 않았다 — 반복 입력의 prefix cache 효과로 **보인다**는 추론뿐이다(02 W2 · Q1). 완료 수도 레벨 4 · 8 에서 이전 18 대 이 측정 16 으로 다르다(이전 리포트 표).
- **같은 config 의 lite-only 셀(bench_report_26101619_minicpm5-2b_GB10_0.31.0)** · warm 39.22 t/s · cold TTFT 35 ms · 조건 차이: 도구 vllm bench serve · random 512/128 · 반복 1 · 판정 없음 · **비교 불가** — 도구 · 입력 · 반복이 다르고 lite 는 성능 판정이 아니다.
- **이 측정의 lite 레그** · warm 38.97 t/s · cold TTFT 33.8 ms(03 §3.1 `lite.*`) · 같은 서빙 · 같은 시각이지만 도구 · 입력(random 512/128)이 다르다 · **비교 불가**(판정점 대신 쓰지 않는다 — 진입 게이트 기록).
- **자매 셀 granite-4.2-8b CNN 셀** · c=1 12.46 t/s(plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행 §2) · 강한 일치 키 `model` 이 다르다 · **무효**(같은 프리셋이어도 다른 모델의 수치다).
- **외부 레퍼런스** · 없음 — E-search 상태 no(리포트 경고 `E-not-attempted`).
- **판정 산정 입력 중 손 승계값** · acceptance length 1.0 은 spec 미선언에 따른 선언값이다(03 §3.1 · 측정 아님). roofline R_fp 54.24 t/s 와 expected_achievable 18.98 t/s 는 결정론 산식 출력이다(리포트 루프라인 표) — 그 안의 MBU 계수 출처는 이 계보 문서가 적지 않았다.

**판정 등급** — `verdict_rule` 의 PASS(루브릭 권한 **weak** · roofline-only · floor 16.13 · ratio 2.009)이고 인증서는 explicit ∧ PASS 에서만 나오므로 발행되지 않았다. 현행 full 정의는 **충족**이다(3.5 사실 블록: 반복 3 ≥ 3 · 도구 guidellm). 판정은 발행 게이트가 아니라 표시이며, 인증서가 없는 weak 판정이라 다른 측정의 baseline 으로 쓰지 않는다 — 다른 HW · 이미지 · 서빙 설정 · 데이터셋이면 재측정하고, explicit 판정이 필요하면 외부 레퍼런스(E)를 넣어 다시 판정한다.
