# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-16T07:25:36Z | sweep_index.generated_utc(output/single/benchlog/sweep_granite42-8b-v031) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 12.46 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 4.62 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-16T07:25:36Z | — |
| `lite.cold_ttft_ms` | 90.8 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_kv_gib` | 23.5 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_kv_tokens` | 154000 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_reserved_total_gib` | 39.92 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.engine_weights_gib` | 16.42 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.gen_tps` | 12.57 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `lite.kv_gib` | 23.5 | sweep_index.lite(output/single/benchlog/sweep_granite42-8b-v031) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 5.43 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.295 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_granite42-8b-v031 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 12.46 | 12.45 | 0 | 80.21136452169979 | 1 | true | 12.451304607296313 | 80.25986235588789 | 93.57929229736328 |
| 2 | ok | 미기재 | 16 | 12.67 | 12.66 | 0 | 77.32329088098862 | 2 | true | 25.341778202513282 | 78.95592041313648 | 409.0738296508789 |
| 4 | ok | 미기재 | 16 | 12.15 | 12.22 | 0 | 79.36744128956514 | 4 | true | 39.631215179773434 | 82.32559449970722 | 566.3533210754395 |
| 8 | ok | 미기재 | 16 | 11.11 | 11.28 | 0 | 83.92734901577819 | 8 | true | 63.197039348453714 | 89.97185807675123 | 1264.7898197174072 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 12.46 | 12.45 | 62.99 | 93.58 | 80.21 | 16/0 |
| 2 | 12.67 | 25.34 | 128.19 | 409.07 | 77.32 | 16/0 |
| 4 | 12.15 | 39.63 | 200.48 | 566.35 | 79.37 | 16/0 |
| 8 | 11.11 | 63.20 | 319.69 | 1264.79 | 83.93 | 16/0 |

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
| `source` | bench_report(bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md) |
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
| `dataset_tokenizer` | /app/models/IBM/Granite-4.2/granite-4.2-8b |
| `dataset_provenance` | 미기재 |
| `status` | recorded |
| `source` | bench_report(bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md) 측정 구성 표 |
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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | `7db9d67706df` 2026-10-16T07:56:04Z | `sweep_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | `7db9d67706df` 2026-10-16T07:56:04Z | `run_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `803064d23d51` 2026-10-06T06:17:10Z | 12ef50a89b86..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@803064d23d51` | `git show 803064d23d516fc72600077cf65e9f6c2a8775d5:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 12ef50a89b86..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**동시성 스윕 레그(GuideLLM)** — 측정 도구 스냅샷 `run_bench.sh@2661d614cf52` 의 GuideLLM 호출은 아래다. ⚠ 이 스냅샷은 데이터셋 knob 커밋(7db9d67) **이전** 판본이라 `--data` 줄이 합성 입력이다 — 이 측정은 커밋 직전 워킹트리(testlog 머리 · 같은 내용이라고 적는다)로 돌았고, 실제 데이터 인자는 아래 결과 JSON 의 config 가 기록한다(데이터 줄은 스냅샷이 아니라 config 가 정본이다).

> [원문] run_bench.sh@2661d614cf52 §L398-403
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

측정 결과 JSON(레벨 1 · 대표 run)의 config 가 기록한 프로파일 · 요청 수 · 시드:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json §L1-1
> "profile": {"kind": "concurrent", "rampup_duration": 0.0, "warmup": {"percent": 0.1111, "value": null, "mode": "prefer_duration"}, "cooldown": {"percent": null, "value": null, "mode": "prefer_duration"}, "streams": [1]}, "constraints": [{"kind": "max_requests", "stopping_scope": "current", "count": 18}]

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json §L1-1
> "seed": {"kind": "static", "value": 42}

측정 형상은 셀 testlog 가 정리한다:

> [원문] testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1
> | 형상 | input 1024 / output 256 (ignore_eos) · 요청 16 + 워밍업 2 · 레벨 1·2·4·8(16 은 max-num-seqs 8 로 제외) · 반복 3 | bench_knobs |

**lite 레그(cold · warm)** — 같은 스윕이 부른 `lite_bench.sh@803064d23d51` 의 원문이다(서빙 컨테이너 안에서 `vllm bench serve` · 이 원문에 `--temperature` 가 없다):

> [원문] lite_bench.sh@803064d23d51 §L172-177
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

조건 목록:
- **도구와 버전**: 동시성 스윕 = GuideLLM `0.7.3`(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · digest 관측 `bench_tool_granite42-8b-v031.json`) · lite = 서빙 이미지 안 `vllm bench serve`.
- **엔드포인트**: `openai-chat`(`/v1/chat/completions` · chat 템플릿 적용) — 두 레그 공통.
- **입력/출력 토큰**: 스윕 = CNN/DailyMail `article` 을 concatenate 로 본문 1024 토큰에 맞추고 출력 256 고정(ignore_eos). 서버가 받은 실측 = prompt 1039 · output 256(전 216 요청 · testlog §2 — 템플릿 15 토큰 포함). lite = random 512/128 · range-ratio 0(위 원문).
- **동시성 레벨**: 1 · 2 · 4 · 8(레벨 16 은 max-num-seqs 8 로 제외 · 판정점 = 1).
- **레벨당 요청 수**: 16 + 워밍업 2(GuideLLM 에는 비율 0.1111 과 상한 18 로 전달 — 위 config). lite = cold 1(워밍업 0) · warm 3(워밍업 1).
- **반복 수**: 3(레벨별 완주 3/3/3/3 · 종류 warm-rerun · 표의 값은 첫 완주 run). 레벨 1 재현 밴드 0.64%(리포트 반복 축 표 · measured n=3).
- **데이터 행 선택**: 런마다 겹치지 않는 연속 18 행(12 런) · seed 42 · 중복 샘플링 없음(03 데이터셋 출처 표 · testlog §1).
- **샘플링 온도**: lite 원문에는 `--temperature` 가 없고 클라이언트가 서버 기본을 쓴다고 경고했다(lite raw) · 서버는 `generation_config.json` 의 temperature 1.0 · top_p 0.95 로 기본을 덮는다(엔진 로그 L97). GuideLLM 결과 config 에는 temperature 칸이 없다 — 같은 서버 기본이 쓰였을 가능성이 있다(추론). ignore_eos 라 출력 길이는 온도와 무관하게 고정이다.
- **spec decode**: 꺼짐(`spec_on false`) · acceptance length 측정값 없음(루브릭은 미선언이라 1.0 을 쓴다 — 03 §3.1).
- **측정 노드 · 시각**: main · 2026-10-16T06:45:56Z(lite cold 직전) → 2026-10-16T07:25:36Z(스윕 색인 생성).
- **벤치 컨테이너 예산**: 4096 MiB(testlog §1 · host_budget).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 측정 · 없음(이 계보 안) · — · 비교 불가 — 이 캠페인의 1차 스윕은 lite 게이트(W2)에서 멈춰 GuideLLM 수치가 없다. 앞선 캠페인(plan_26101410)의 lite-only 측정 기록은 이 계보 밖이라 나란히 두지 않는다. 그래서 레벨별 이전 대 이 측정 표는 싣지 않는다.
- 같은 서빙의 lite 레그 · warm 12.57 t/s · cold TTFT 90.8 ms · 입력 random 512 대 CNN 1024 · 출력 128 대 256 · 도구 vllm bench 대 GuideLLM · 요청 수 3 대 16 · 비교 불가 — 부하 조건(입출력 길이 · 도구 · 요청 수)이 다르다(03 §3.4 · testlog §2).
- 자매 셀 minicpm5-2b(같은 캠페인 · 같은 프리셋 · c=1 decode 38.13 t/s) · 비교 불가(무효) — 강한 일치 키 `model` 이 다르다(testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §2). 순위로 읽지 않는다.
- 합성 입력 셀 · 계보에 없음 · 데이터셋이 다르다 · 비교 불가 — CNN 셀은 CNN 셀끼리만 비교한다(testlog §4 · 합성 대조는 이번 범위 밖).
- 외부 레퍼런스 · 없음 · — · 판정 불가 — 리포트가 `E-not-attempted` 로 판정을 roofline-only 로 강등했다(bench_report_26101616_granite-4.2-8b_GB10_0.31.0 §판정).

**손으로 승계된 판정 입력**: acceptance length 1.0(spec 미선언 · 선언값 · 03 §3.1) · primary 의 계수 realistic_fraction 0.35(expected_achievable = R_fp × 0.35 · 대역폭 273.0 GB/s lookup:gb10 — `roofline.json` 기록 · 0.35 를 고른 근거는 계보 밖).

**판정 등급**: 판정은 `PASS`(verdict_rule 출력)이고 권위는 루브릭 권한 `weak`(primary 5.43 t/s · floor 4.62 t/s · 측정/primary 비 2.295) — 인증서는 explicit ∧ PASS 에서만 나오므로 발행되지 않았다(03 §3.3 아래 사실 블록). 판정은 발행 게이트가 아니라 표시다. 위 현행 full 정의 사실 블록대로 이 측정은 **현행 full 정의를 충족한다**(반복 3 ≥ 3 · 도구 guidellm). 기준이 루프라인 하나뿐이라 외부 baseline 으로 쓸 수 있는 값은 아니다. 다른 HW · 다른 vLLM 버전 · 다른 데이터셋/프리셋 · 다른 서빙 노브에서는 다시 재야 하고, explicit 권한 판정이 필요하면 외부 레퍼런스(E)부터 붙여 다시 판정한다.
