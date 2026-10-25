# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-25T01:51:17Z | sweep_index.generated_utc(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `accept_len` | 미기재 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 22.78 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 27.62 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-25T01:51:17Z | — |
| `lite.cold_ttft_ms` | 1111.4 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.engine_kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn31-fp8-1m-tp2.yaml) |
| `lite.engine_kv_tokens` | 2112392 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.engine_reserved_total_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.engine_weights_gib` | 62.75 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.gen_tps` | 22.68 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) |
| `lite.kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn31-fp8-1m-tp2.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 32.5 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.701 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · output) |
| `verdict_reasons` | M(22.78) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.701) → 기대 이하 | output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 22.78 | 22.66 | 0 | 42.002045874502144 | 1 | true | 22.738149867707236 | 43.899113312363625 | 525.4318714141846 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(22.78) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.701) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 22.78 | 22.74 | 118.31 | 525.43 | 42.00 | 16/0 |

> ⚠ 레벨이 **하나뿐**이다 — 이 레시피의 부하 거동은 이 hint 로 알 수 없다. 곡선이 필요하면 재측정해야 한다(부재를 성능 판정으로 읽지 말 것).

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
| `source` | bench_report(bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | 0daa0d097080..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `29cbdbc89a3d` 2026-10-17T02:03:29Z | 0daa0d097080..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@29cbdbc89a3d` | `git show 29cbdbc89a3d694097e94a9f9d6c56dde4bc1ee0:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `d33b0878a6c3` 2026-10-17T13:18:03Z | 0daa0d097080..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@d33b0878a6c3` | `git show d33b0878a6c37dc7d89aec06861ba438ceffb4de:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `dacb77675e63` 2026-10-10T03:17:48Z | 0daa0d097080..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@dacb77675e63` | `git show dacb77675e63af79a4be9d23020fb91eb6093ad2:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_q38fn31-fp8-1m-tp2/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

측정 명령 줄 원문(셸 히스토리 · 실행 로그 줄)은 계보에 없다. 아래는 측정 시각 직전 커밋의 **도구 원문**이고, 각 레그를 부른 도구의 해당 줄이다. 셀 스윕은 `sweep_bench.sh` 가 조립했다(03 §3.4 사실 블록). 첫 full 패스는 그 스크립트의 기본 도구(`TOOL="vllm"` · 아래 발췌)로 재서 무효였고(02 R5), 이 측정은 `--tool guidellm` 을 명시했다(`testlog_26102504_…` 측정 구성 · 3.3 `bench_tool guidellm`).

> [원문] sweep_bench.sh@d33b0878a6c3 §L80-84
> #   lite 레그는 이 노브와 무관하게 **언제나 `vllm bench serve`** 다(도구 구성 `full = lite ∪ GuideLLM` · 반복 축은 위 ★).
> #   두 레그를 이질적으로 유지하는 것이 설계이며, 그 이질성이 실결함 2건을 잡았다(2026-09-01·09-03).
> #   `--tool guidellm` 은 **레벨 측정만** 옮긴다.
> TOOL="vllm"
> BENCH_BUDGET_MIB=""

레벨 측정(동시성 1 · GuideLLM) — `run_bench.sh` 의 GuideLLM 가지:

> [원문] run_bench.sh@29cbdbc89a3d §L439-450
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

합성 데이터 인자는 같은 파일 L406-407 이다(`DATA_ARG="kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN"`). warmup 은 요청 수를 비율로 바꾸고, 총 요청을 warmup 만큼 늘린다(L341-346).

lite 레그(cold · warm · vllm bench serve) — `lite_bench.sh`:

> [원문] lite_bench.sh@d33b0878a6c3 §L177-182
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

**조건 목록**
- 도구 · 버전: 레벨 = GuideLLM 0.7.3(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · digest 기록 일치 · level_01 `bench_tool_*.json`)이고, 엔드포인트는 `/v1/chat/completions` 다. lite = `vllm bench serve`(측정 이미지 안 · 위 발췌)다.
- 입력/출력 토큰: 레벨은 합성 1024 / 256 이다(level_01 `dataset_*.json` · 리포트 측정 환경 `입력 길이(sweep) 1024`). 같은 레벨의 총 tok/s ÷ 출력 tok/s 는 118.31 ÷ 22.74 로 약 5.2 다. 1024+256 꼴에 chat 템플릿 토큰이 더해진 구성과 정합한다(해석). lite 는 random 512 / 128 이다(도구 원문). bench JSON 평균은 566 · 565 / 128(chat 템플릿 포함 · 01 §1.4 재구성).
- 동시성 레벨: 1 하나뿐이다(3.2 경고 · `HINT_MISSING_SWEEP_LEVELS`).
- 레벨당 요청 수: 16 이 집계됐다(level_01 `measured.json` `num_prompts 16 · completed 16 · failed 0`). warmup 은 2 다(`sweep_index.json` `bench_knobs.knobs.warmups` = 2 · source preset). 도구가 총 요청을 16 + 2 = 18 로 늘리고(`run_bench.sh@29cbdbc89a3d` L345), 같은 run 의 서버 카운터 차분 `requests_delta 18`(prefix_cache_run)과 일치한다.
- 반복 수: 3/3 완주다(3.3 · run 시작 01:40:45Z · 01:44:19Z · 01:47:50Z · decode 22.78 / 22.90 / 23.27 · 재현 밴드 2.13% measured n=3). 판정점과 표는 대표 run(첫 완주 run) 1개의 값이다.
- warm/cold: lite cold = 요청 1 · warmup 0 이고, warm = 요청 `BURST_N` · warmup 1 이다(`lite_bench.sh@d33b0878a6c3` L229 · L231 · warm JSON 은 3요청 완주 · 01 §1.4). 레벨은 같은 상주 서버의 warm 재실행(`warm-rerun`)이다.
- 샘플링: 도구 원문의 GuideLLM 백엔드 인자는 `ignore_eos: true` 만 싣고, temperature · seed 인자는 없다. 서버 기본 샘플링은 엔진 로그가 `generation_config.json` 으로 덮였다고 적는다(lite_engine L740 · temperature 1.0 · top_k 20 · top_p 0.95). 요청이 temperature 를 실제로 실었는지는 미관측이다. lite 레그도 temperature 인자가 없다(위 발췌).
- spec decode: 꺼짐이다(서빙 yaml · 러너에 speculative-config 없음 · 엔진 `speculative_config=None` head L97). acceptance length 는 측정값이 없다(3.1 `accept_len 미기재` — 루브릭은 spec 미선언이라 1.0 을 쓴다 · 선언값).
- bench 예산: 4096 MiB 다(`sweep_index.json` `bench_knobs.knobs.bench_budget_mib` · source agent · reason host_budget — `sweep_bench.sh@d33b0878a6c3` 가 run_bench `--bench-budget-mib` 로 넘긴다 · testlog_26102504 머리의 2048 은 세션 요약이며 이 셀 값이 아니다). 벤치 진입 게이트는 `floor − bench ≥ abs_band` 이고, 이 셀 바닥 21422 MiB 에서 상한은 11182 MiB 다(02 §2.5 · 저작 때 `budget_preflight.py` 계산).
- 측정 노드: cluster 다(GuideLLM 은 `--network host` 로 API 서버가 뜬 main 에서 돈다 · `run_bench.sh@29cbdbc89a3d` L357-358).
- 측정 시각(UTC): 2026-10-25T01:39:47Z(lite cold 직전) → 01:51:17Z(sweep_index 생성).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102510_qwen3.8-flash-next-fp8_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 첫 full 패스(`bench_report_26102505_qwen3.8-flash-next-fp8_GB10_0.31.0` · vLLM bench) · 값은 싣지 않는다 · 측정 도구가 다르다(vllm-bench-serve 대 guidellm) · **비교 불가** — 부하 조건(도구)이 다르고, 계보 판정이 그 패스를 무효로 선언했다(`testlog_26102504_…` §2 결함 1 · 02 R5). 그 리포트는 계보 읽기 목록 밖이라 레벨 표를 나란히 싣지 않는다. 이 셀에 남은 유효한 이전 full 측정은 없다.
- 같은 셀의 S3 lite(원장 시도 2 의 기동 · raw `output/multi/benchlog/lite_raw_q38fn31-fp8-1m-tp2.json` measured_utc 2026-10-24T14:03:52Z · 계보 경유 `testlog_26102500_…` 읽기 목록 밖 결과표 22.66 t/s) 대 이 측정의 lite 레그(warm 22.68 t/s · 3.1 `lite.gen_tps`) · 같은 이미지 · 같은 lite 도구, 기동이 다르다. 그때의 config · env 가 이 측정과 바이트로 같은지는 대조하지 않았다 · 비교 가능(lite 끼리 · 조건 동일성 미확인 — 비 22.68 ÷ 22.66 ≈ 1.00 · 소수 2자리). 단 lite 는 full 판정이 아니다.
- 자매 셀 `q38fn31-nv4-1m-tp2`(NVFP4 · PLE 상주 · 같은 캠페인) · 동시성 1 decode 26.01 t/s · REFUTE ratio 0.80 · 강한 일치 키 `model`(nvfp4 대 fp8)이 다르다 · **무효**(이 비교에서) — 체크포인트 · 양자화 · PLE 방식이 한꺼번에 달라 동시성 1 에서 NVFP4 상주 26.01 대 이 셀 22.78 이라는 사실만 기재한다. 원인을 PLE mmap 비용으로 돌릴 수 없다(02 Q1 · 출처 `testlog_26102504_…` §1).
- 자매 셀 `q38fn31-nv4-mmap-1m-tp1`(NVFP4 · PLE mmap · 싱글 TP=1) · 16.72 t/s · PASS(roofline 하한) · 강한 키 `model` · `topology` · `tensor_parallel_size` 가 다르다 · **무효**(같은 출처).
- 이전 vLLM 의 같은 형상 셀 `fp8-f8-1m-mmp`(인증서 `benchmark_26091121_qwen3.8-flash-next-fp8_GB10_0.29.0.yaml`) · 33.27 t/s · 강한 일치 키 `vllm_version`(0.29.0 대 0.31.0)이 다르다 · **무효** — 덧붙여 소프트 지문 · 부하 조건도 다르다. driver 580.173.02 · image · KV 바이트 21474836480 · gmu 0.85 · moe_backend triton · bench_tool vllm-bench-serve · max_num_seqs N/A · spec on(acceptance length 2.14) · 동시성 레벨 1~16. `fp8-bf-1m-mmp`(`benchmark_26091120_…` · 35.39 t/s)도 같은 이유로 무효다.
- 외부 레퍼런스 E 32.5 t/s(판정 primary) · NVIDIA `Qwen3.8-Flash-Next-NVFP4` 1 Spark PLE mmap 성공 사례(`testlog_26102406_…` 근거 5 · `plan_26102401_…` 정정 기록) · 체크포인트(NVFP4) · 노드 수(1) · TP(1)가 다르고, 계보의 `perf_26091305_qwen38fn_res최소조건_후속캠페인` §4 표가 그 사례를 PLE mmap · **MTP4(spec on)** · KV fp8 로 적는다 — 이 셀은 spec off(엔진 `speculative_config=None`)다. 입출력 · 도구는 그 표에도 없다 · **비교 불가**(강한 키 · spec 상태가 다르다) — 이 세션이 고른 기준이고(`testlog_26102504_…` §1), weak 권한 루브릭이 그것을 정본으로 썼을 뿐이다.
- 루프라인(이 측정의 결정론 상한) · R_fp 41.5 t/s · expected_achievable 14.52 t/s(realistic_fraction 0.35) · 같은 측정 · 비교 가능(의심 임계) — 측정 22.78 은 expected 를 넘고 물리 상한 R_fp 아래다(`physical.exceeds=false`). PLE 47.68 GiB 는 active 바이트에서 뺐다(roofline notes).

판정 손 승계 입력: acceptance length 는 측정값이 없고, spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 결손 아님 · 3.1). 외부 기준 E 32.5 는 사람(세션)이 고른 값이다. tolerance 0.15 는 루브릭 기본값이다.

**판정 등급.**
- 판정 권위는 `verdict_rule.py` 의 결정론 판정이고, 루브릭 권한은 weak 다(외부 레퍼런스 E 가 정본). 결과는 **REFUTE** 다. 측정 22.78 이 하한 27.62(=32.5×(1−0.15)) 미달이고 ratio 0.701 이다.
- REFUTE 는 발행 게이트가 아니라 **표시**다. 서빙은 성립했다(health 200 + 추론 1회 · 1M needle 통과는 정정 기록). 수치는 관측이지 baseline · 권고가 아니다.
- 인증서는 explicit ∧ PASS 일 때만 나오므로 없다(3.3 비발행 판정).
- 현행 full 정의(위 사실 블록)는 **충족**이다(GuideLLM · 판정점 반복 3 ≥ 3).
- 재측정이 필요한 조건은 셋이다. ① 같은 체크포인트 · 같은 HW 조건의 기준(또는 사용자 목표 c)이 생겨 루브릭이 바뀔 때. ② 부하 거동이 필요할 때 — 동시성 다레벨 스윕이 없다(max-num-seqs 1 형상). ③ 플러그인 v2(CUDA graph) · MTP 처럼 형상이 바뀔 때.
