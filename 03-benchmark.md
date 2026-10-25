# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-25T03:13:30Z | sweep_index.generated_utc(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `accept_len` | 미기재 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md)) |
| `decode_tps_conc1` | 16.67 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 7.81 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-25T03:13:30Z | — |
| `lite.cold_ttft_ms` | 745.2 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.engine_kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/solar31-nv4-1m-tp2.yaml) |
| `lite.engine_kv_tokens` | 1206177 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.engine_reserved_total_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.engine_weights_gib` | 71.6 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.gen_tps` | 17.39 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) |
| `lite.kv_gib` | 14.0 | sweep_index.lite(output/multi/benchlog/sweep_solar31-nv4-1m-tp2) — 서빙 yaml `kv-cache-memory-bytes: 15032385536`(14.0 GiB)의 **선언 클램프를 엔진이 되읽은 값**(측정 아님 · output/multi/configs/solar31-nv4-1m-tp2.yaml) |
| `primary_source` | expected_achievable(roofline×MBU) | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 9.19 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.814 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · output) | — |

**`levels`**(1행 · 출처 sweep_index(output/multi/benchlog/sweep_solar31-nv4-1m-tp2 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 16.67 | 16.66 | 0 | 57.93059947443943 | 1 | true | 16.70250885852934 | 59.992922469973564 | 603.3868789672852 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 16.67 | 16.70 | 84.10 | 603.39 | 57.93 | 16/0 |

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
| `source` | bench_report(bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md) |
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
| `source` | bench_report(bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md) 측정 구성 표 |
<!-- /FACT:dataset_provenance -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/multi/benchlog/sweep_solar31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_solar31-nv4-1m-tp2/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

측정 명령 줄의 실행 원문(로그에 남은 명령)은 계보에 없다 — 아래는 측정 시각 이전 마지막 커밋의 **도구 원문**과 측정 산출물이 기록한 조건이다. 레그마다 도구가 다르다.

**full 레그(동시성 스윕 · 판정점)** — `sweep_bench.sh` 가 레벨마다 `run_bench.sh` 를 부르고, `--tool guidellm` 이면 GuideLLM 컨테이너를 이렇게 띄운다:

> [원문] run_bench.sh@29cbdbc89a3d §L445-451
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "$DATA_ARG" "${DATA_EXTRA[@]}" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \

`TOTAL_REQ = NPROMPTS + WARMUPS` · `WARM_FRAC = WARMUPS / TOTAL_REQ`(같은 도구 L345-346) · synthetic 이면 `DATA_ARG="kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN"`(L407). 이 측정이 넘긴 값은 sweep_index `bench_knobs`(프리셋 `default_preset`)에 기록돼 있다: levels [1] · input_len 1024 · output_len 256 · num_prompts 16 · warmups 2 · repeats 3 · backend openai-chat · bench_budget_mib 2048. 레벨 2 · 4 · 8 · 16 은 서빙 yaml `max-num-seqs 1` 보다 커서 걸러졌다(같은 기록의 filtered_out) — 레벨이 하나뿐인 이유다. 데이터셋 사이드카:

> [원문] output/multi/benchlog/sweep_solar31-nv4-1m-tp2/level_01/dataset_solar31-nv4-1m-tp2.json §L2-4
>   "dataset_kind": "synthetic",
>   "input_len": 1024,
>   "output_len": 256,

**lite 레그(cold · warm)** — `sweep_bench.sh` 가 lite 레그로 `lite_bench.sh` 를 부른다. docker 평면이면 서빙 컨테이너 안에서:

> [원문] lite_bench.sh@d33b0878a6c3 §L177-181
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

cold = num-prompts 1 · warmups 0, warm = burst N=3 · warmups 1(같은 도구 L229 · L231 · lite raw `burst_n: 3` · `serve_plane: docker`). 01 §1.4 의 재구성 명령이 `--random-input-len 521` 인 것은 JSON 의 total ÷ completed 평균(chat 템플릿 포함)이라서이고 도구 원문 인자는 512 다.

**조건 목록**
- 도구 · 버전: full = GuideLLM 0.7.3(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · digest 기록 일치 — `level_01/bench_tool_*.json`) · lite = 서빙 이미지 안 `vllm bench serve`.
- 입출력 토큰: full = synthetic 1024 / 256 · lite = random 512 / 128(range-ratio 0).
- 동시성 레벨: 1 하나(full · lite 모두 동시성 1).
- 레벨당 요청 수: full = 16(+ warmup 2 · 완료 16 / 실패 0) · lite = cold 1 · warm 3.
- 반복 수: 3(종류 warm-rerun · 완주 3/3 · decode 16.67 / 16.66 / 16.60 · 밴드 0.42%).
- warm/cold: full 은 같은 running serve 의 warm 재실행 · lite 는 cold(무-warmup 단일요청 · TTFT 745 ms) + warm burst(gen 17.39 t/s).
- 도구가 정한 인자: full = `extras={"ignore_eos":true}` · temperature 인자 없음(도구 원문에 없다 — 서버 · 도구 기본값) · seed 없음 · lite = `--ignore-eos` · `--random-range-ratio 0` · `--trust-remote-code` · temperature 인자 없음(lite 도구 원문 L180-181 에 없다).
- spec decode: 꺼짐(설정에 speculative-config 없음 · 이식본이 받지 않는다 — 02 R7) · acceptance length 미기재(루브릭은 spec 미선언이라 1.0 을 쓴다 — 선언값).
- 측정 노드: cluster(TP=2 · main+sub 한 측정 정체성) · 클라이언트는 main 의 `localhost:8080`.
- 측정 시각(UTC): lite 시작 2026-10-25T02:58:03Z → 스윕 색인 2026-10-25T03:13:30Z(full run 1 = 02:59:09Z~03:03:58Z · `level_01/repeat_run.json`).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26102512_solar-open2-250b-nota-nvfp4_GB10_0.31.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 full 측정 · 첫 패스(vLLM bench · 구 리포트 `26102505_*` · `26102506_*`) · — · 도구가 다르다(full 정의 밖) · **비교 불가** — 무효로 폐기된 측정이다(testlog_26102504 §2 표 1번 행 · 02 R5).
- 같은 셀의 판정 · 실패한 sweep 뒤 과거 산출물 PASS · — · 측정 아님 · **비교 불가** — 폐기 후 재측정(02 M2).
- 같은 셀의 이전 lite(V2 이식 전 구성) · warm gen 18.25 t/s · cold TTFT 687 ms · 같은 이미지 · 같은 레시피 노브 16개에 logits processor 없음 · chat-template kwargs 없음 · 러너 `--no-async-scheduling` 없음 · lite 도구의 커밋된 바이트는 같다(lite_bench.sh 마지막 커밋 d33b0878a6c3 = 2026-10-17 · 두 측정 모두 그 뒤 · 3.4 스냅샷 표 · 측정 시각의 워킹트리 편집은 미관측) · **비교 가능(형상 비교 · stale)** — 인증서가 없어 소프트 지문 블록 대조는 할 수 없고, 서빙 구성 차이 3개를 위에 전부 적었다(testlog_26102418 §5).

| 지표(lite) | 이 측정의 lite 레그 | 이전 lite | 비(이 측정 ÷ 이전 · 소수 3자리) |
|---|---|---|---|
| warm gen t/s | 17.39 | 18.25 | 0.953 |
| cold TTFT ms | 745.2 | 687 | 1.085 |

  차이(gen −4.7%)는 이 측정 full 반복 밴드 0.42% 를 넘는다 — 같은 범위 재현이라 부르지 않는다. 원인은 설명되지 않았다(세 설정이 함께 바뀌었다 · 02 Q6). 계보 경유 문서 testlog_26102500(읽기 목록 밖)은 그 사이 S4 구성 lite 17.42 t/s 를 "노이즈 범위" 라 적었으나 밴드 근거가 없다.
- 자매 셀(같은 모델의 다른 형상) · 없음 · — · 이 계보에 Solar-Open2 의 다른 형상 셀이 없다.
- 같은 testlog 의 다른 4셀(Qwen3.8 FP8/NVFP4 · Gemma) · 16.72~26.01 t/s · 강한 키 `model` 이 다르다 · **비교 불가(무효)** — 모델이 다르다(testlog_26102504 §1).
- 외부 레퍼런스 · 없음(E-search empty) · — · 판정은 roofline-only 다(bench_report §판정 warning).
- 계보의 스모크 생성 16.1 t/s(testlog_26102418 §3) · 측정 도구 · 부하 조건 미기재 · **비교 불가** — 스모크 출력이지 측정이 아니다.

**판정 산정의 손 승계 입력** — expected 9.19 t/s = 루프라인 R_fp 26.25 t/s × `realistic_fraction` 0.35(roofline.json · 측정값이 아니라 계수) · 대역폭 273 GB/s 는 `lookup:gb10`(조회표 · 측정 아님) · acceptance length 1.0 은 spec 미선언에 따른 선언값(`declared-absent`)이다. 활성 바이트(19.362 GiB · dense + 8/320 routed)는 도구가 체크포인트에서 계산했다.

**판정 등급** — 판정 `PASS`(측정 16.67 t/s ≥ 하한 7.81 t/s · ratio 1.814)이지만 권위는 **weak** 다: 외부 레퍼런스가 없어 roofline×계수 기준선만으로 판정했고, 루브릭 권한이 explicit 이 아니라 **인증서는 발행되지 않았다**(03 §3.3 비발행 판정 · 결손 아님). 판정은 발행 게이트가 아니라 표시다. 현행 full 정의(GuideLLM × 반복 ≥3)는 **충족**이다(위 사실 블록). 한계: 합성 데이터셋 · 동시성 1 레벨 하나 · 단일 세션 측정이다(testlog_26102504 §1 한계). 재측정이 필요한 조건: 외부 레퍼런스가 생길 때 · max-num-seqs 를 올려 부하 곡선이 필요할 때 · 서빙 구성(logits processor · 스케줄링 · eager)을 바꿀 때.
