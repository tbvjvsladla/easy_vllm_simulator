# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-25T02:27:13Z | sweep_index.generated_utc(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `accept_len` | 미기재 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 26.01 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 27.62 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-25T02:27:13Z | — |
| `lite.cold_ttft_ms` | 578.4 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.engine_kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn31-nv4-1m-tp2.yaml) |
| `lite.engine_kv_tokens` | 2112392 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.engine_reserved_total_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.engine_weights_gib` | 61.93 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.gen_tps` | 27.46 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| `lite.kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/q38fn31-nv4-1m-tp2.yaml) |
| `primary_source` | E(external_reference) | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 32.5 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.8 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | REFUTE | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · output) |
| `verdict_reasons` | M(26.01) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.8) → 기대 이하 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 26.01 | 25.65 | 0 | 37.15228753931382 | 1 | true | 25.724483461465894 | 38.444445468485355 | 374.50408935546875 |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(26.01) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.8) → 기대 이하
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 26.01 | 25.72 | 133.85 | 374.50 | 37.15 | 16/0 |

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
| `source` | bench_report(bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 셀의 명령 줄 원문(셸 히스토리 · 로그에 남은 호출 줄)은 계보에 없다. 판정점(full · GuideLLM)과 lite 레그는 각각 다른 도구가 불렀으므로 도구 원문을 레그별로 인용한다. 아래는 그 판본 `run_bench.sh` 의 GuideLLM 호출 줄이다 — 스윕 색인(`bench_knobs`)이 이 셀에 넘긴 값은 동시성 `1` · 입력 `1024` · 출력 `256` · 요청 `16` · warmup `2` · 반복 `3` · 데이터셋 `synthetic` 이다.

> [원문] run_bench.sh@29cbdbc89a3d §L445-450
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

같은 판본에서 synthetic 이면 `DATA_ARG="kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN"`(L407)이고, warmup 은 요청 수가 아니라 비율로 옮겨 `TOTAL_REQ = NPROMPTS + WARMUPS` · `WARM_FRAC = WARMUPS / TOTAL_REQ` 로 넘긴다(L345 · L346). GuideLLM 호출에는 temperature · seed 인자가 없다 — 서버 기본값(측정 기동 로그가 `generation_config.json` 에서 temperature 1.0 · top_k 20 · top_p 0.95 로 덮였다고 적는다 · lite_engine L739)이 쓰였을 것이다(추론). lite 레그는 서버 컨테이너 안에서 `vllm bench serve` 를 부른다:

> [원문] lite_bench.sh@d33b0878a6c3 §L177-181
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

cold 는 `_bench "$COLD" 1 0`(요청 1 · warmup 0) · warm 은 `_bench "$WARM" "$BURST_N" 1`(기본 `BURST_N=3` · warmup 1)이다(lite_bench.sh@d33b0878a6c3 L229 · L231 · L42). lite 호출에도 `--temperature` 가 없다.

**조건 목록**
- 도구와 버전: 판정점 = GuideLLM 0.7.3(`ghcr.io/vllm-project/guidellm:v0.7.3` · digest 일치 확인 · 레벨 1 bench_tool JSON) · lite = 이미지 안 `vllm bench serve`(vLLM 0.31.1.dev0 빌드).
- 입력/출력 토큰 길이: 판정점 synthetic prompt_tokens 1024 · output_tokens 256(스윕 색인 bench_knobs · 도구 원문 L407) · lite random 512 / 128 · range-ratio 0(도구 원문 L180). 01 §1.4 의 재구성 명령이 적은 566 · 565 는 chat 템플릿을 포함한 실측 평균이다.
- 동시성 레벨: 1 만(프리셋 2 · 4 · 8 · 16 은 max-num-seqs 1 로 걸러짐 · 03 §3.2).
- 레벨당 요청 수: 16(+warmup 2 → GuideLLM max_requests 18).
- 반복 수: 3(3/3 완주 · run 순 decode 26.01 / 25.33 / 25.11 · 재현 밴드 3.53% — bench_report_26102511 §반복 축). 표 수치는 대표 run(첫 완주) 값이다.
- warm/cold: lite 는 cold 단일 요청(warmup 0) + warm 3요청(warmup 1) · GuideLLM 은 warmup 비율 2/18.
- ignore-eos: 두 레그 모두 켬(GuideLLM `extras={"ignore_eos":true}` · lite `--ignore-eos`).
- spec decode: 꺼짐(선언 off · 측정 `spec_on false`) · acceptance length 미기재(루브릭은 선언 부재라 1.0 을 쓴다 · 측정값 아님).
- 엔드포인트 · 백엔드: `/v1/chat/completions` · `openai-chat`(bench_tool JSON `endpoint` · 스윕 색인 `backend`).
- 측정 노드: cluster(TP=2 쌍 · 스윕 meta `measured_node`).
- 측정 시각(UTC): 2026-10-25T02:16:49Z(lite raw measured_utc) → 2026-10-25T02:27:13Z(sweep_index generated_utc).
- 벤치 클라이언트 메모리 예산: 스윕 색인 기록 4096 MiB(02 M4 · 실제 `--memory` 값의 로그 관측 없음).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102511_qwen3.8-flash-next-nvfp4_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 셀은 인증서가 없다(REFUTE · 권한 weak — 인증서는 explicit ∧ PASS 에서만 나온다). 그래서 비교의 강한 키 · 소프트 지문은 바인딩된 bench_report_26102511 의 '측정 환경 스냅샷' 표(model · gpu_model · vllm_version · quantization · topology · tensor_parallel_size + driver · cuda · image_tag · max_model_len · max_num_seqs · kv_cache_memory_bytes · kv_cache_dtype · gpu_memory_utilization · moe_backend · enforce_eager)와 스윕 색인 meta(ple_mode · spec_declared · bench_tool)로 대조했다.

**같은 셀의 이전 측정**

- 무효 첫 full 패스(`bench_report_26102505_qwen3.8-flash-next-nvfp4_GB10_0.31.0` · 2026-10-24T20:50:28Z 생성 · 계보 표 밖) · 판정점 26.92 t/s · 강한 키 6개와 위 소프트 지문 키는 같은 값이고(같은 이미지 태그 · 같은 yaml 값) **측정 도구가 다르다**(vllm-bench-serve 대 guidellm) · **비교 불가** — 부하 조건(도구)이 다르고 그 측정은 full 정의 미충족으로 폐기됐다(R6 · testlog_26102504 §2 항목 1). 원천 레벨은 둘 다 동시성 1 하나뿐이다.

| 동시성 | 이 측정 | 이전 측정(무효 · vLLM bench) | 비 |
|---|---|---|---|
| 1 | 26.01 | 26.92 | 0.966 |

  (비 = 이 측정 ÷ 이전 · 소수 셋째 자리 반올림 · 도구가 달라 차이를 해석하지 않는다.)
- 캠페인 lite(시도 2 · 2026-10-24T05:51:18Z) · warm 25.91 t/s · cold TTFT 439 ms(testlog_26102414 §3) 대 이 측정의 lite 레그 warm 27.46 t/s · cold TTFT 578.4 ms(03 §3.1) · 도구(lite_bench.sh · 그 사이 바뀐 커밋 없음 — 03 §3.4 스냅샷 '다음 변경' 칸) · 이미지 · yaml 값은 같고 서버 기동이 다르다(시도 2 대 시도 5) · **비교 가능** — warm 은 이 측정이 1.06배(27.46 ÷ 25.91 · 소수 둘째 자리) · cold TTFT 는 1.32배(578.4 ÷ 439)다. lite 는 반복이 없어 재현 밴드가 정의되지 않고 차이의 원인은 설명되지 않았다 — "같은 범위 재현" 이라 부르지 않는다.

**자매 셀(같은 이미지 · 같은 날 · testlog_26102504 §1)**

- `q38fn31-fp8-1m-tp2`(FP8 체크포인트 · PLE mmap · TP=2) · 22.78 t/s · 강한 키 `model` 이 다르다(qwen3.8-flash-next-fp8) · **비교 불가(무효)** — 다른 체크포인트다. 같은 E 32.5 로 REFUTE(ratio 0.701)였다는 사실만 나란히 적는다.
- `q38fn31-nv4-mmap-1m-tp1`(NVFP4 · PLE mmap · 싱글 TP=1) · 16.72 t/s · 강한 키 `topology` · `tensor_parallel_size` 가 다르다 · **비교 불가(무효)**.
- `q38fn31-nv4-mmap-1m-tp2`(NVFP4 · PLE mmap · TP=2 동치 시험 셀) · full 측정 없음 · 비교 대상 수치가 없다.

**이전 vLLM 의 같은 모델 멀티 측정**

- 0.30.0 `q38fn030-nv4-res-262k`(bench_report_26101013) · 37.63 t/s · 강한 키 `vllm_version` 이 다르다(0.30.0) · **비교 불가(무효)** — 그 측정은 MTP on(accept_len 2.08) · max_model_len 262144 · KV auto 8 GiB · max_num_seqs 8 이기도 하다.
- 0.29.0rc6 resident 262k(testlog_26091304 · 인증서 benchmark_26091223) · 46.70 t/s · 강한 키 `vllm_version` 이 다르다 · **비교 불가(무효)** — MTP3 · 262k · KV 8 GiB 이기도 하다.

**외부 레퍼런스**

- E 32.5 t/s(이 판정의 primary) · 판정 testlog 는 "이 세션이 고른 외부 레퍼런스" 라고만 적는다 · 같은 값을 res 후속 캠페인은 NVIDIA 포럼의 같은 체크포인트 **1 Spark median(PLE mmap)** 으로 적었다(testlog_26091304 §2 · 같은 출처인지는 추론) · 노드 수(1 대 2) · PLE 방식(mmap 대 resident)이 다르고 spec 상태 · 컨텍스트는 이 계보에 기록이 없다 · **비교 불가** — like-with-like 기준이 아니다. 이 REFUTE 는 "그 외부 수치보다 느리다" 는 표시이고 형상 결함의 증거가 아니다(02 Q1).
- 같은 출처의 2 Spark SPEED 53.7 median(PLE resident · MTP3 · KV fp8) · 노드 수 · PLE 방식은 같고 spec 이 다르다(MTP3 대 off) · 컨텍스트 미기록 · **비교 불가**.

**손으로 승계된 판정 입력** — E 32.5(세션이 고른 값 · 위) · accept_len 1.0(spec 미선언이라 루브릭이 쓰는 선언값 · 측정 아님 · 결손 아님 · 03 §3.1) · tolerance 0.15. 루프라인 상한 R_fp 45.03 t/s · expected_achievable 15.76 t/s 는 판정기가 결정론으로 낸 값이다(verdict.json).

**판정 등급** — 판정 권위는 verdict_rule 의 루브릭(권한 **weak** · primary = E)이고 결과는 **REFUTE**(26.01 < floor 27.62 · ratio 0.8)다. 인증서는 없고 footer 는 bench_report 를 묶는다. 이 판정은 발행 게이트가 아니라 **표시**다 — 서빙은 성립했고(health 200 + 추론 1회) 측정은 유효하다(GuideLLM × 반복 3/3 · 에러율 0). 수치는 관측이지 baseline · 권고가 아니다. 위 현행 full 정의 사실 블록의 충족 여부는 **예**(반복 3 ≥ 3 · 도구 guidellm)이다. 재측정이 필요한 조건: 형상 노브(enforce-eager · spec · max-num-seqs · KV 클램프)를 바꿀 때 · like-with-like 외부 기준(2노드 · resident · spec off · 1M)을 찾았을 때 · 동시성 곡선이 필요할 때(이 측정은 레벨 1 뿐).
