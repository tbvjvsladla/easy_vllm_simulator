# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T05:29:12Z | sweep_index.generated_utc(output/single/benchlog/sweep_ornith15-35b-v030) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md)) |
| `decode_tps_conc1` | 30.82 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 10.41 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T05:29:12Z | — |
| `lite.cold_ttft_ms` | 479.1 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.engine_kv_gib` | 8.0 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.engine_kv_tokens` | 408122 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.engine_reserved_total_gib` | 72.69 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.engine_weights_gib` | 64.69 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.gen_tps` | 32.83 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `lite.kv_gib` | 8.0 | sweep_index.lite(output/single/benchlog/sweep_ornith15-35b-v030) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 12.25 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.516 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_ornith15-35b-v030 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_ornith15-35b-v030 · output) | — |

**`levels`**(3행 · 출처 sweep_index(output/single/benchlog/sweep_ornith15-35b-v030 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 30.82 | 30.81 | 0 | 31.176155688715916 | 1 | true | 30.891174657026816 | 32.446395605802536 | 351.37009620666504 |
| 2 | ok | 미기재 | 16 | 26.34 | 26.3 | 0 | 36.22900925430597 | 2 | true | 52.86952805296369 | 37.97017876058817 | 424.03531074523926 |
| 4 | ok | 미기재 | 18 | 18.24 | 18.88 | 0 | 51.69275695202398 | 4 | true | 70.4740649582842 | 54.83571719378233 | 882.8597068786621 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 30.82 | 30.89 | 155.66 | 351.37 | 31.18 | 16/0 |
| 2 | 26.34 | 52.87 | 266.41 | 424.04 | 36.23 | 16/0 |
| 4 | 18.24 | 70.47 | 355.12 | 882.86 | 51.69 | 18/0 |

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
| `source` | bench_report(bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_ornith15-35b-v030/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | e2b67d3320fd..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | e2b67d3320fd..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | e2b67d3320fd..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `e2b67d3320fd` 2026-10-10T04:54:37Z | e2b67d3320fd..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@e2b67d3320fd` | `git show e2b67d3320fd68531d9c6bc717aecfb3961239a9:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_ornith15-35b-v030/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**조건 목록.** 측정 노드 sub(single · GB10 ×1) · 측정 시각 2026-10-10T05:29:12Z(스윕 색인 generated_utc). full 구간 도구 guidellm 0.7.3(벤치 리포트) · 백엔드 openai-chat · 동시성 레벨 1 · 2 · 4(8 · 16 은 `max-num-seqs 4` 상한으로 제외) · 입력 1024 · 출력 256 토큰(합성 텍스트) · 레벨당 요청 16 · warmup 2 · 반복 3(모두 프리셋 기본값, 아래 표) · spec decode 꺼짐(서빙 yaml · 러너에 speculative-config 없음, 판정기에 선언 부재로 승계) · acceptance length 미기재(spec off 라 1.0 이 선언값). lite 구간은 같은 스윕이 포함해 실행한 것이다(`vllm bench serve`, 입력 512 · 출력 128 · 동시성 1 · `--ignore-eos`, 아래 원문).

**도구가 정한 인자(레그별).** guidellm 레그(`run_bench.sh`): ignore_eos 를 backend extras 로 켜고, 프로파일 `kind=concurrent`, 데이터 `kind=synthetic_text`, 제약 `kind=max_requests`(총 요청 = 요청 수 + warmup), 컨테이너는 `HF_HUB_OFFLINE=1` 이다. **temperature · seed · random-range-ratio 는 이 레그의 명령 원문에 없다**(guidellm 쪽에는 해당 인자를 넘기지 않음 — 도구 기본값이며 값은 원문 없음). lite 레그(`lite_bench.sh`): `--random-range-ratio 0`, `--request-rate inf`, `--ignore-eos`, `--max-concurrency 1`; temperature 는 이 레그 원문에 없다.

**측정 구성 원문(프리셋 해소 결과, 10-10 testlog):**

> [원문] testlog_26101014_knob프리셋_E2E_셀C_ornith §4. sweep_index.bench_knobs
> | knob | value | source | grade | reason | filtered_out |
> |---|---|---|---|---|---|
> | levels | [1,2,4] | preset | auto(1/2/4 모두 auto) | preset_default | 8, 16 — `> max-num-seqs 4` (serving_cap le_max_num_seqs(4), yaml 선언 출처) |
> | input_len | 1024 | preset | auto | preset_default | — (input+output ≤ max_model_len 262144 적용) |
> | output_len | 256 | preset | auto | preset_default | — |
> | num_prompts | 16 | preset | auto | preset_default | — |
> | warmups | 2 | preset | auto | preset_default | — |
> | repeats | 3 | preset | auto | preset_default | — (defaulted: 활성 캠페인 없음 _bootstrap) |
> | backend | openai-chat | agent | auto | chat_template | — |
> | bench_budget_mib | 8192 | agent | auto | host_budget | — |

**full 레그 명령 원문(도구 판본 `run_bench.sh@e2b67d3320fd`):**

> [원문] run_bench.sh@e2b67d3320fd §L394-405
>     --memory "${BENCH_BUDGET_MIB}m" --memory-swap "${BENCH_BUDGET_MIB}m" \
>     -v "$TOKSTAGE:/tok:ro" -v "$ABS_OUTDIR:/out" \
>     -e HOME=/tmp -e XDG_CACHE_HOME=/tmp/.cache \
>     -e HF_HUB_OFFLINE=1 -e TRANSFORMERS_OFFLINE=1 \
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \
>     --output "kind=json,path=/out/$(basename "$BJSON")" \
>     --disable-progress \

**warmup 처리 원문** — 총 요청에 warmup 을 더해 보낸다:

> [원문] run_bench.sh@e2b67d3320fd §L324-329
>   # warmup 단위 변환. vLLM 은 **요청 수**로, GuideLLM 은 **비율/시간**으로 warmup 을 센다.
>   # 같은 숫자를 그대로 넘기면 "2 요청"이 "2 초"가 되어 조용히 다른 것을 잰다 — 비율로 옮기고
>   # 총 요청을 warmup 만큼 늘려 **집계 대상 수를 보존**한다(근사이며, 실제 집계 수는 산출물이 밝힌다).
>   local TOTAL_REQ WARM_FRAC
>   TOTAL_REQ=$(( NPROMPTS + WARMUPS ))
>   WARM_FRAC="$(python3 -c "print(round($WARMUPS/max($TOTAL_REQ,1), 4))")"

**lite 레그 명령 원문(`lite_bench.sh@e2b67d3320fd`):**

> [원문] lite_bench.sh@e2b67d3320fd §L172-177
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

위 두 도구 원문은 측정 시각 이전 마지막 커밋의 바이트다(3.4 표). 01 §1.4 의 `vllm bench serve` 재구성 명령은 JSON 필드에서 복원한 것이라 입력 길이(524 · 523)가 chat 템플릿이 붙은 실측 평균이고 위 원문(512)과 다르다. 이 차이는 입력 길이 정의 차이로 보이며 확인되지 않았다.


## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md` 원문 그대로):

> 미관측

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 미관측 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 미관측 | .claude/rules/docs.md 에서 full 정의 문장(`full 계측(…)`)을 읽지 못했다 — 대조하지 않는다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 측정(10-01 서브 서빙 testlog의 lite 한 레벨) · warm 생성 31.62 t/s · 같은 도구(`lite_bench.sh`) · 같은 지표(median_tpot) · **비교 가능(lite 끼리, 단발 단일 레벨)** — 10-01 원시 warm JSON 의 median_tpot_ms 31.6245 에서 1000 ÷ 31.6245 = 31.62 t/s 가 testlog 값과 일치하고 output_throughput(29.08)과는 다르므로 10-01 도 median_tpot 기준이다(10-10 의 `gen_src=median_tpot` 와 같은 지표). 도구 판본은 10-01(커밋 84942a9)과 10-10(e2b67d3320fd)이 달라도 차이는 엔진 로그 머리 캡처 추가뿐이고 벤치 명령은 같다. 이전 측정은 lite 동시성 1 한 레벨뿐이고 인증서가 없어 강한 일치 키 · 소프트 지문 대조는 하지 않았다(그래서 '형상 비교' 수준이다). 순위를 매기지 않고 나란히 둔다:

| 동시성 | 이 측정(lite warm 생성) | 이전 측정(lite warm) | 비 |
|---|---|---|---|
| 1 | 32.83 | 31.62 | 1.038 |

  비 = 32.83 ÷ 31.62(나눗셈 한 번, 소수 3자리). cold TTFT 도 두 측정이 다르다(10-01 원시 259.18 ms, 이 측정 479.1 ms — Q3). 이 비교에는 **재현 밴드가 없다**: lite 구간은 단발(burst N=3) 측정이고 반복 밴드가 측정된 적이 없다. 아래 guidellm 밴드(0.16 % · 0.11 % · 0.44 %)는 다른 도구의 반복 3회 밴드라 여기에 대보지 않는다 — 비 1.038 이 '밴드 안'인지 '밖'인지는 판단할 수 없고, 차이의 원인은 설명되지 않았다(같은 10-10 서빙 안에서도 lite 32.83 과 guidellm 30.82 는 도구 차이로 다르다).
- 자매 셀(같은 모델의 다른 형상) · 계보에 없음 · **비교 대상 없음** — MoE 백엔드 auto 형상은 기동이 실패했다(W2)고 testlog 가 적었고, 이 형상에서 FlashInfer 경로와 triton 경로의 t/s 는 백엔드가 달라 비교 대상이 아니라고 적었다.
- 외부 레퍼런스(E) · 없음 · **비교 불가** — 판정이 roofline-only 로 강등됐다(벤치 리포트 경고 E-not-attempted). 다른 HW · 다른 vLLM 버전의 hint 와의 비교도 이 계보에 없다.
- 이 측정 안의 재현 밴드(자체 밴드): 레벨 1 은 3회 30.82 / 30.83 / 30.87 · 밴드 0.1621 %, 레벨 2 는 26.34 / 26.37 / 26.37 · 0.1138 %, 레벨 4 는 18.24 / 18.32 / 18.25 · 0.4379 %(testlog_26101014_knob프리셋_E2E_셀C_ornith §3). 엔진 대조 ratio 1.01.

**판정 입력.** 판정은 루브릭 권한 weak 의 `expected_achievable(roofline×MBU)` 이다: primary 12.25 t/s · floor 10.41 t/s(tolerance 0.15) · 측정 ÷ primary = 2.516. primary 는 roofline(활성 바이트 7.267 GiB = dense 5.39 GiB + (8/256) × routed 60.00 GiB, 대역폭은 GB10 룩업값 273 GB/s) 곱하기 실현 계수(realistic_fraction)이며, 이 입력들은 roofline 산출물에서 왔고 이 zip 에 실리지 않는다. spec 이 꺼져 있어 손으로 승계한 acceptance length 는 없다(선언 부재 1.0). 측정이 primary 의 2.5배 남짓이므로 PASS 는 하한 위에 있다는 표시이지 성능 우수의 증거가 아니다(Q7).

**레벨 4 집계 18건.** 요청 16건인데 집계가 18인 점은 `run_bench.sh` 가 총 요청을 요청 수 + warmup(16 + 2 = 18)로 보내는 것과 숫자가 같다(위 warmup 원문). 레벨 4 에서만 warmup 이 집계에서 빠지지 않았을 가능성이 있으나 레벨 1 · 2 는 16 이므로 설명이 완전하지 않다 — **추론**이며 Q2 로 남긴다.

**판정 등급.** 권위: 루브릭 권한 weak 의 `PASS` — 인증서는 explicit ∧ PASS 에서만 나오므로 **인증서 비발행**이다(결손 아님). 판정은 발행 게이트가 아니라 표시이며 REFUTE 였어도 수치는 관측으로 게재된다. bench_mode 는 full(레벨 1 완주 3회 ≥ 도구의 full 정의 3, downgrade 없음). 위 현행 full 정의 사실 블록은 정의 문장을 읽지 못해 충족 여부가 **미관측**이다 — 이 지도는 충족했다고 말하지 않는다. 재측정이 필요한 조건: 다른 도구 판본, 다른 vLLM 버전, 다른 호스트 메모리 구조, 외부 레퍼런스로 판정을 다시 낼 때.

