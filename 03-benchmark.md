# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T06:26:21Z | sweep_index.generated_utc(output/single/benchlog/sweep_c-tq4nc) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_c-tq4nc/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101015_qwen3-4b_GB10_0.26.0.md)) |
| `decode_tps_conc1` | 22.44 | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 10.1 | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T06:26:21Z | — |
| `lite.cold_ttft_ms` | 55.1 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.engine_kv_gib` | 11.0 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.engine_kv_tokens` | 232992 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.engine_reserved_total_gib` | 18.56 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.engine_weights_gib` | 7.56 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.gen_tps` | 22.95 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `lite.kv_gib` | 11.0 | sweep_index.lite(output/single/benchlog/sweep_c-tq4nc) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 11.88 | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 1.889 | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_c-tq4nc · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_c-tq4nc · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_c-tq4nc · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 22.44 | 22.46 | 0 | 44.329787235634 | 1 | true | 22.475302752907897 | 44.558046385645866 | 102.67353057861328 |
| 2 | ok | 미기재 | 16 | 26.26 | 26.28 | 0 | 37.53043062546674 | 2 | true | 52.60993355440351 | 38.077631033957005 | 175.77838897705078 |
| 4 | ok | 미기재 | 18 | 25.0 | 25.13 | 0 | 39.46316382464241 | 4 | true | 91.06203215008067 | 40.00672046095133 | 167.72174835205078 |
| 8 | ok | 미기재 | 18 | 22.77 | 23.11 | 0 | 43.26679005342371 | 8 | true | 143.7863447871369 | 43.9260583370924 | 207.92651176452637 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101015_qwen3-4b_GB10_0.26.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 22.44 | 22.48 | 113.08 | 102.67 | 44.33 | 16/0 |
| 2 | 26.26 | 52.61 | 264.69 | 175.78 | 37.53 | 16/0 |
| 4 | 25 | 91.06 | 458.16 | 167.72 | 39.46 | 18/0 |
| 8 | 22.77 | 143.79 | 723.43 | 207.93 | 43.27 | 18/0 |

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
| `source` | bench_report(bench_report_26101015_qwen3-4b_GB10_0.26.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | db34119fd7b3..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | db34119fd7b3..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | db34119fd7b3..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | db34119fd7b3..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_c-tq4nc/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

- 도구 · 버전: GuideLLM 0.7.3 (3.3 측정 구성) · 백엔드 `openai-chat`(chat 템플릿 엔드포인트) · 풀 레그 + lite 레그. 아래 첫 발췌가 testlog 의 측정 머리 줄과 첫 두 줄이다.
- 입력·출력 길이 · 반복: 입력 1024 · 출력 256 · 요청 16 · 워밍업 2 · 반복 3 · 레벨 [1,2,4,8](testlog §5 knob 표 — 전부 프리셋 기본이며 backend 와 bench_budget_mib 만 agent 선택). 레벨 16 은 `max-num-seqs 8` 상한으로 걸러졌다.
- spec 상태: 꺼짐(`spec_on=false`, 03 사실 블록).
- **도구가 정한 인자 — 풀 레그(guidellm)**: 아래 `run_bench.sh` 발췌가 원문이다. 프로파일은 동시 스트림 수(`streams=$CONC`)와 워밍업 비율, 데이터는 합성 텍스트(`prompt_tokens=$ILEN`, `output_tokens=$OLEN`), 종료 조건은 요청 수 상한이고, 요청 본문 extras 는 `ignore_eos: true` 하나다. 이 발췌 범위에는 샘플링 temperature 인자가 없다 — 엔진 로그는 모델 generation_config 의 temperature 0.6 · top_k 20 · top_p 0.95 가 기본으로 덮였다고 보고하므로(L96) 클라이언트가 별도 값을 보냈는지는 이 발췌만으로 알 수 없다.
- **도구가 정한 인자 — lite 레그(`vllm bench serve`)**: `lite_bench.sh` 발췌가 원문이다. 랜덤 데이터 입력 512 · 출력 128, `--random-range-ratio 0`, 동시성 1, `--ignore-eos`, 워밍업 수는 호출 인자. (01 §1.4 의 재구성 명령이 보인 입력 520 은 chat 템플릿이 붙은 실측 토큰 평균이고 도구 원문의 인자는 512 다 — 두 숫자는 같은 설정의 두 표기다.)

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3. 측정
> ## 3. 측정 (full-bench · 프리셋 기본 · guidellm 0.7.3 · openai-chat)
> - 판정점 레벨 1 **decode 22.44 t/s**, runs[] = 22.44 / 22.44 / 22.44, **repro_band_pct 0.0 %**. TTFT median 102.7 ms, completed 16 / failed 0.

> [원문] run_bench.sh@e2b67d3320fd §L398-403
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

> [원문] lite_bench.sh@e2b67d3320fd §L180-184
>     "$CLIENT_VLLM" bench serve --backend "$BACKEND" --base-url "$BASE_URL" --endpoint "$LITE_ENDPOINT" \
>       --model "$MODEL_NAME" --tokenizer "$MODEL_PATH" --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts "$2" --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups "$3" \
>       --save-result --result-dir "$(dirname "$1")" --result-filename "$(basename "$1")"

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §5. sweep_index.bench_knobs
> ## 5. sweep_index.bench_knobs (resolution_order: cli_pin > campaign > agent > preset)
> | knob | value | source | grade | reason | filtered_out |

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md` 원문 그대로):

> 미관측

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101015_qwen3-4b_GB10_0.26.0.md)) |
| 현행 정의의 반복 요건 | 미관측 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101015_qwen3-4b_GB10_0.26.0.md) |
| 현행 full 정의 충족 | 미관측 | .claude/rules/docs.md 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 측정 · 비교 불가 — 이 계보 문서에 이전 측정의 값이 없다. testlog 는 과거 GuideLLM 기록이 있었고 형상이 입력 8192 / 출력 1024 / n=32 로 이번(1024/256/16)과 **다르다**고 적었다: 이전 값도 이 페이로드에 없고 형상도 달라 레벨 표를 나란히 둘 수 없다.
- 같은 모델의 기존 hint 태그(원격 목록에서 이름만 확인 — 0.26.0 의 KV dtype auto · fp8 · turboquant 3bit · turboquant 4bit 와 다른 arch 세그먼트, 0.27.1 의 다른 HW; 00 §0.8) · 비교 불가 — 이 발행은 그 태그들의 수치와 인증서를 읽지 않았고 소프트 지문(driver · image · max_model_len · KV 클램프 등)을 대조하지 않았다. "~만 다르다" 라고 적지 않는다.
- 외부 레퍼런스 · 비교 불가 — 조사하지 않았다. 판정 후보 표에서 외부 레퍼런스 칸은 미설정이다(`verdict.json` rubric candidates 의 `E(external_reference)` state unset).
- 자체 재현 밴드 · 레벨 1 0.0 % · 레벨 2 0.0381 % · 레벨 4 0.1201 % · 레벨 8 0.0439 %(testlog §3). **레벨 1 의 밴드 0.0 % 는 세 실행의 decode 값이 소수 2자리까지 같다는 뜻이며 원인이 미확정이다(Q1).** 같은 세 실행의 output_throughput 은 서로 다르므로 이 0.0 을 "완벽한 재현"의 증거로 읽지 않는다. 3.2 표는 대표 실행(첫 완주 실행) 하나의 값이고 반복 값 전부가 아니다.

**판정 등급.** 판정은 weak 권한의 PASS 다 — 판정 점 22.44 대 floor 10.1(루브릭 primary 는 roofline × MBU 로 산정한 expected_achievable 11.88, 비율 1.889 — 03 사실 블록). 인증서 발행은 explicit 권한 ∧ PASS 일 때만이므로 이 셀에는 인증서가 없다(3.3 사실 블록). 따라서 이 수치는 다른 hint 와 순위를 매기는 기준(baseline)이 아니라 이 셀의 **관측 게재**다. 손으로 승계된 판정 입력은 없다(spec 꺼짐이라 accept_len 은 선언 부재 1.0). 현행 full 정의 충족 여부는 사실 블록이 `미관측`(정의 원문을 읽지 못함)이라 판정하지 않는다 — 이 측정이 반복 3 · 도구 guidellm 이라는 사실만 있다. 재측정이 필요한 조건: GPU 메모리가 다른 카드, 다른 입력 형상, 다른 vLLM 빌드, 그리고 Q1 의 원인이 밝혀진 뒤의 밴드 재확인.
