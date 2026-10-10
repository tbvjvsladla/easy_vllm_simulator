# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T04:07:13Z | sweep_index.generated_utc(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `accept_len` | 2.075268817204301 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) measured_accept_len |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md)) |
| `decode_tps_conc1` | 37.63 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 45.65 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T04:07:13Z | — |
| `lite.cold_ttft_ms` | 1030.6 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.engine_kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.engine_kv_tokens` | 537040 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.engine_reserved_total_gib` | 70.89 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.engine_weights_gib` | 62.89 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.gen_tps` | 43.27 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `lite.kv_gib` | 8.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 53.7 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.701 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | true | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · output) |
| `verdict_reasons` | M(37.63) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.701) → 기대 이하 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.075268817204301 | 16 | 37.63 | 35.68 | 0 | 25.25054987739114 | 1 | true | 35.79853482232616 | 26.576057076454163 | 354.3283939361572 |
| 2 | ok | 2.075268817204301 | 16 | 33.8 | 31.81 | 0 | 28.107544955085306 | 2 | true | 62.63020929319735 | 29.584744945168495 | 411.130428314209 |
| 4 | ok | 2.075268817204301 | 16 | 27.57 | 25.27 | 0 | 34.61063889896168 | 4 | true | 86.37089255832127 | 36.27743758261204 | 453.1443119049072 |
| 8 | ok | 2.075268817204301 | 17 | 19.38 | 19.32 | 0 | 45.570459552839694 | 8 | true | 126.93226050965745 | 51.599971018731594 | 727.224588394165 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(37.63) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.701) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 37.63 | 35.80 | 179.13 | 354.33 | 25.25 | 16/0 |
| 2 | 33.80 | 62.63 | 313.40 | 411.13 | 28.11 | 16/0 |
| 4 | 27.57 | 86.37 | 432.19 | 453.14 | 34.61 | 16/0 |
| 8 | 19.38 | 126.93 | 635.16 | 727.22 | 45.57 | 17/0 |

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
| `source` | bench_report(bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `5d12e3a0a7bc` 2026-10-06T06:10:02Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@5d12e3a0a7bc` | `git show 5d12e3a0a7bc46cf7b4ad84790c24ba2fdd8ce0c:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | dacb77675e63..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_q38fn030-nv4-res-262k/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**셀 testlog 에 남은 측정 명령 원문**(동시성 스윕 · 판정점 포함):

> [원문] testlog_26101017_knob프리셋_E2E_셀M1_q38fn030_nv4_res_262k_판정 §2
> bash .claude/skills/adversarial-benchmark/scripts/sweep_bench.sh q38fn030-nv4-res-262k --topology multi --tool guidellm --backend openai --knob-reason backend=prior_cell_shape --bench-budget-mib 8192 --knob-reason bench_budget_mib=host_budget

> [원문] testlog_26101017_knob프리셋_E2E_셀M1_q38fn030_nv4_res_262k_판정 §2
> - 레벨: 1·2·4·8(16 은 서빙 max-num-seqs 8 초과로 상한 필터 제외)
> - 판정점(레벨 1) decode t/s: **37.63 (37.63 · 39.79 · 38.66) · 밴드 5.58 %** · 나머지: L2 33.80 · L4 27.57 · L8 19.38
> - `sweep_index.bench_knobs`: input_len 1024 · output_len 256 · num_prompts 16 · warmups 2 · repeats 3 = preset · preset_default / backend·bench_budget_mib = agent(아래 근거) / 프리셋 = default_preset

**동시성 스윕 레그를 부른 도구 원문**(`run_bench.sh` · 그 판본 `sweep_bench.sh` 가 레벨마다 부른다) — GuideLLM 호출. bench JSON 에 없는 인자 중 `ignore_eos` 는 여기서 `true` 로 넘기고, warmup 은 요청 수가 아니라 비율(`warmup=$WARM_FRAC`)로, 요청 수는 `16 + warmup 2` 를 `max_requests` 로 건다. 샘플링 temperature · seed 는 이 호출에 없다 — GuideLLM 기본값이며 값은 원문 없음.

> [원문] run_bench.sh@dacb77675e63 §L398-403
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

**lite 레그(cold · warm)를 부른 도구 원문**(`lite_bench.sh` — 컨테이너 안 `vllm bench serve`). 입력 512 · 출력 128 · `--random-range-ratio 0` · `--ignore-eos` · 동시성 1 이고, cold 는 1요청 · warmup 0, warm 은 3요청 · warmup 1 이다(헤더 주석 · 01 §1.4 bench 단계). temperature 는 이 호출에 없다 — 원문 없음.

> [원문] lite_bench.sh@5d12e3a0a7bc §L172-176
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

**조건 목록**
- 도구 · 버전: 동시성 스윕 = GuideLLM 0.7.3(3.3 · `benchmarks.json metadata` 실측) · lite = 서빙 이미지 안의 `vllm bench serve`(엔진 0.30.1.dev0+gced6857af).
- 측정 형상의 출처: 프리셋 `default_preset` 기본값(입력 1024 · 출력 256 · 레벨당 16요청 · warmup 2 · 반복 3) + Agent 가 근거 코드와 함께 고른 knob 2개(`backend=openai` ← `prior_cell_shape` · `bench_budget_mib=8192` ← `host_budget`).
- 엔드포인트: `openai` 백엔드 = `/v1/completions`(과거 셀과 같게 맞춘 선택 — testlog §2 근거 표).
- 동시성 레벨: 1 · 2 · 4 · 8(16 은 서빙 `max-num-seqs 8` 초과라 프리셋 상한 필터가 제외) · request-rate inf.
- 반복: 레벨마다 3회(3.3 `repeats_completed` 3) · 판정점은 첫 완주 run 값(37.63) · 레벨 1 의 run 순 값은 37.63 · 39.79 · 38.66(밴드 5.58%).
- warm/cold: 스윕은 warmup 2 를 둔 warm 측정 · cold TTFT 1030.6ms 는 lite cold(warmup 0 단일 요청)의 값이다(3.1).
- spec decode: 켜짐(MTP k=3) · acceptance length 2.075268817204301(3.1 `accept_len`) — 이 값은 GuideLLM 레벨 런이 잰 것이 아니라 lite warm 레그(`vllm bench serve` · 512/128 · 3요청)의 `spec_decode_acceptance_length` 를 승계한 값이다(level_01/measured.json `spec_axis_source` = inherited(lite_warm…json) · 네 레벨 모두 같은 값). 1024/256 판정점 자체의 수용길이는 잰 기록이 없다.
- 측정 노드: API 서버가 뜬 메인에서 GuideLLM 이 `localhost` 로 붙는다(cluster 측정 · TP=2 양 노드가 서빙).
- 측정 시각(UTC): lite 시작 2026-10-10T03:50:30Z → 스윕 색인 2026-10-10T04:07:13Z(01 §1.4 bench 단계).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101013_qwen3.8-flash-next-nvfp4_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 측정(2026-10-01 · `bench_report_26100109_qwen3.8-flash-next-nvfp4_GB10_0.30.0`) · 레벨 1 36.54 t/s · 강한 일치 키(model · gpu_model · vllm_version 0.30.0 · quantization N/A · topology multi · TP 2) 전부 같음 · 소프트 지문 중 리포트 스냅샷 키(driver_version 580.173.02 · cuda_version 132 · image_tag · max_model_len · max_num_seqs · kv_cache_memory_bytes · kv_cache_dtype · gpu_memory_utilization · moe_backend · enforce_eager · bench_tool)는 모두 같다(리포트 스냅샷 키만 보면 다른 소프트 키는 0개다) · 리포트 스냅샷 밖에서 **image_digest 가 다르다**(이번 `sha256:123c331f…` — sweep_index.meta · 0.4 / 이전 메인 `sha256:cdb1a38b…` — `testlog_26100109` §1) — 이름만의 차이가 아니라 메인 이미지의 빌드 내용이 바뀌었다: 이전 측정의 메인 이미지는 post 패치 10 · 20 · 30 · 40 이 구워져 있었고(`testlog_26100109` §1 빌드 패치 행), 이번 메인 이미지는 패치 슬롯이 비어 있다(01 §1.1 노드별 원장) · 부하 조건: 입력 1024 · 출력 256 · 16요청 · warmup 2 · 반복 3 · GuideLLM · openai 엔드포인트 · MTP on 이 같고(같은 레벨의 총 tok/s ÷ 출력 tok/s 가 두 측정 모두 약 5.00), 이전 측정에만 레벨 16 이 있다 · **비교 가능(형상 비교 · stale)** — stale 사유는 image_digest(와 그에 따른 메인 이미지의 post 패치 유무)다(두 측정 모두 인증서가 없어 ple_mode · attention_backend · ngc_base_tag 같은 인증서 전용 소프트 키는 대조하지 못했다). 그 밖에 노드 정합도 달랐다 — 이전 측정의 serve attestation 은 전 축 equal, 이번은 빌드 원장 differ(01 §1.5). 또 두 리포트의 driver_version 은 둘 다 manifest 선언값(관측은 이번 580.178.04 — 0.4)이라 드라이버가 같았는지는 이 표가 가르지 못한다.

| 동시성 | 이 측정 | 이전 측정 | 비 |
|---|---|---|---|
| 1 | 37.63 | 36.54 | 1.030 |
| 2 | 33.8 | 30.44 | 1.110 |
| 4 | 27.57 | 26.57 | 1.038 |
| 8 | 19.38 | 21.21 | 0.914 |
| 16 | 미측정(상한 필터 제외) | 13.25 | — |

비 = 이 측정 ÷ 이전(decode t/s · 소수 셋째 자리 반올림). acceptance length 는 이 측정 2.075268817204301 · 이전 1.9641025641025642(각 리포트 루프라인 표의 값)로 비 1.057 이다 — 단 두 값 모두 GuideLLM 1024/256 판정점이 아니라 각 회차 lite warm 레그(512/128)에서 승계한 수용길이라(이번은 level_01/measured.json · 이전은 `docs/simlog/26100107_v030_e2e/m9_bench.log` 의 judge 줄 `measured.spec_axis_source=inherited(…lite_warm…json)` 로 확인), 이 비는 두 회차 lite warm 수용길이의 비다. 레벨 1 의 처리량 비 1.030 이 수용길이로 설명되는지는 **판정 불가**다(판정점 수용길이를 잰 측정이 없다). 밴드 규칙은 하나로 정한다 — 셀 testlog 가 레벨 1 에 쓴 규칙(차이를 **이전 측정 자신의 그 레벨 재현 밴드**와 비교)을 모든 레벨에 같게 적용한다. 레벨 1 +3.0% ≤ 이전 7.72% → 안(셀 testlog 의 재현 PASS 와 같다 · `testlog_26101017` 머리) · 레벨 2 +11.0% ≤ 이전 12.13% → 안 · 레벨 4 +3.8% ≤ 이전 10.45% → 안 · 레벨 8 −8.63% > 이전 8.58% → 밖(0.05%p 차 · 차이는 19.38 ÷ 21.21 − 1). 참고로 이번 측정 자신의 밴드(3.36 · 13.66 · 9.44%)로 보면 레벨 2 는 밖 · 레벨 4 · 8 은 안이다 — 어느 밴드로 보느냐에 따라 레벨 2 · 8 의 판정이 갈리므로 레벨 8 의 차이를 '같은 범위 재현' 이라 부르지 않는다. 차이의 원인은 설명되지 않았다.
- 0.29.0rc6 의 같은 형상 res 셀(`bench_report_26091223_qwen3.8-flash-next-nvfp4_GB10_0.29.0` · 46.7 t/s) · 강한 키 vllm_version 이 다르다(0.29.0 대 0.30.0) · 측정 도구도 다르다(`vllm bench serve` 대 GuideLLM — `testlog_26091304` §1) · **비교 불가** — 이 수치는 그 비교에서 무효다. 트리플렛 머리 주석도 그 셀을 "참조(승계 아님)" 로만 적는다.
- 0.29.0rc6 의 첫 res 측정(`nv4-bf-262k-res` · 32.53 t/s · GuideLLM 0.7.3) · vllm_version 다름 · KV 20GiB · gmu 0.85 · **비교 불가** — 그 인증서의 판정도 교정으로 `NEEDS_RUBRIC` 이 됐다(M2).
- 자매 형상 PLE mmap(0.29.0rc6 의 262k mmp · D1 등) · vllm_version 이 다르다 · **비교 불가** — 이 셀과 같은 vLLM 버전의 mmap 측정은 없다(R6).
- 외부 레퍼런스 E 53.7 t/s(nvidia 2 Spark SPEED median · PLE resident · MTP3 · KV fp8) · 인증서가 없어 강한 키를 대조할 수 없고 KV dtype 이 다르다(이 셀 auto) · 측정 도구 · 입출력 길이는 계보에 기록이 없다 · **비교 불가(엄밀)** — 그럼에도 weak 권한 판정의 기준으로 쓰였다는 것이 이 셀 REFUTE 의 정체다(`testlog_26100109` §2.1 — "weak 판정의 E 는 그 불일치를 안고 있다").
- 손으로 승계된 판정 입력: E 53.7(`--reference-tps 53.7` · 2026-10-01 판정 M9 에서 승계 · 원 출처는 `testlog_26091304` §2 의 외부 게시값) · 권한 weak(`--authority weak` 명시). floor 45.65 = 53.7 × (1 − 0.15) 에 accept_len 은 들어가지 않는다. 루브릭의 R_token 은 실측 accept_len 을 승계했다.

**판정 등급.** 판정 원천(`verdict.json` · verdict_rule 출력)은 weak 권한 REFUTE(ratio 0.701)이며 인증서는 explicit ∧ PASS 에서만 나오므로 발행되지 않았다 — 계측 바인딩은 bench_report 다. 위 사실 블록대로 이 측정은 현행 full 정의를 **충족**한다(반복 3 · GuideLLM). 판정은 발행 게이트가 아니라 표시다 — 이 셀은 서빙(health 200 · 추론 1회)과 full 측정이 성립했고, REFUTE 는 외부 기준 대비 기대 이하라는 표시이지 서빙 실패가 아니다. 수신자는 수치를 baseline 이나 권고로 쓰지 말고, (1) 다른 vLLM 버전 · 다른 이미지(digest) · 다른 드라이버로 옮기면 재측정하고 (2) E 와 같은 KV dtype · 같은 측정 조건의 레퍼런스가 생기면 판정을 다시 내야 한다(Q1).
