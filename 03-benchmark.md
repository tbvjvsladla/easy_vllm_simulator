# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T07:14:23Z | sweep_index.generated_utc(output/single/benchlog/sweep_granite42-30b-v030) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md)) |
| `decode_tps_conc1` | 3.93 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 1.39 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T07:14:23Z | — |
| `lite.cold_ttft_ms` | 530.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_kv_gib` | 33.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_kv_tokens` | 135168 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_reserved_total_gib` | 87.58 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.engine_weights_gib` | 54.58 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.gen_tps` | 3.98 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `lite.kv_gib` | 33.0 | sweep_index.lite(output/single/benchlog/sweep_granite42-30b-v030) |
| `primary_source` | expected_achievable(roofline×MBU) | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 1.63 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 2.411 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output) | — |

**`levels`**(4행 · 출처 sweep_index(output/single/benchlog/sweep_granite42-30b-v030 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 3.93 | 3.93 | 0 | 251.94850809433882 | 1 | true | 3.9292259505267677 | 254.51106298714876 | 905.1012992858887 |
| 2 | ok | 미기재 | 15 | 3.96 | 3.96 | 0 | 250.79980457530303 | 2 | true | 7.427531340494134 | 252.80635803937912 | 764.44411277771 |
| 4 | ok | 미기재 | 18 | 3.92 | 3.92 | 0 | 253.8047061246984 | 4 | true | 14.130962016898557 | 255.07835298776627 | 583.141565322876 |
| 8 | ok | 미기재 | 16 | 3.83 | 3.84 | 0 | 259.6872152066698 | 8 | true | 20.684343369008886 | 261.13647408783436 | 763.0157470703125 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 3.93 | 3.93 | 19.88 | 905.10 | 251.95 | 16/0 |
| 2 | 3.96 | 7.43 | 37.57 | 764.44 | 250.80 | 15/0 |
| 4 | 3.92 | 14.13 | 71.48 | 583.14 | 253.80 | 18/0 |
| 8 | 3.83 | 20.68 | 104.63 | 763.02 | 259.69 | 16/0 |

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
| `source` | bench_report(bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 2661d614cf52..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 2661d614cf52..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `803064d23d51` 2026-10-06T06:17:10Z | 2661d614cf52..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@803064d23d51` | `git show 803064d23d516fc72600077cf65e9f6c2a8775d5:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `2661d614cf52` 2026-10-10T04:53:51Z | 2661d614cf52..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@2661d614cf52` | `git show 2661d614cf52ca976fd3b41e2ba6a85ccf8a6d30:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_granite42-30b-v030/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

측정을 실제로 시작한 명령 줄은 셀 판정 testlog 에 남아 있다(완주한 2차 스윕):

> [원문] testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §2. 측정
> bash .claude/skills/adversarial-benchmark/scripts/sweep_bench.sh granite42-30b-v030 --topology single --tool guidellm --backend openai-chat --knob-reason backend=chat_template --bench-budget-mib 4096 --knob-reason bench_budget_mib=host_budget

동시성 스윕 레그(레벨 1 · 2 · 4 · 8 × 반복 3)는 그 판본의 `run_bench.sh` 가 GuideLLM 컨테이너로 부른다. bench JSON 에 없는 인자 — `ignore_eos` 와 워밍업 비율 · 합성 데이터 형상 · 요청 수 제약 — 는 도구 원문이 정했다:

> [원문] run_bench.sh@2661d614cf52 §L398-403
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

이 레그는 temperature 를 넘기지 않는다 — 서버는 모델의 `generation_config.json` 기본값(temperature 1.0 · top_p 0.95)으로 샘플링한다(01 §1.4 (f) · 엔진 로그 L103). `TOTAL_REQ = NPROMPTS + WARMUPS` · `WARM_FRAC = WARMUPS/TOTAL_REQ` 다(같은 원문 L328-329).

lite 선행 레그(cold 1요청 · warm burst · 동시성 1)는 `lite_bench.sh` 가 서빙 컨테이너 안에서 `vllm bench serve` 로 부른다 — 입출력 길이가 스윕 레그와 다르다:

> [원문] lite_bench.sh@803064d23d51 §L172-176
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

조건 목록:
- 도구 · 버전: 동시성 스윕 = GuideLLM 0.7.3(03 §3.3 · `run_bench.sh@2661d614cf52`) · lite 레그 = 서빙 이미지 안의 `vllm bench serve`(`lite_bench.sh@803064d23d51`) · 스윕 조립 = `sweep_bench.sh@2661d614cf52`.
- 입력/출력 토큰: 스윕 = 1024 / 256(프리셋 default_preset · `sweep_index.bench_knobs`) · lite = 512 / 128(range-ratio 0). 01 §1.4 의 재구성 명령에 보이는 527 은 chat 템플릿이 붙은 실측 평균(total ÷ completed)이며 도구 인자가 아니다.
- 동시성 레벨: 1 · 2 · 4 · 8(16 은 max-num-seqs 8 초과로 상한 필터 제외).
- 레벨당 요청 수: num_prompts 16 + warmups 2(GuideLLM 에는 `max_requests` = num_prompts + warmups 와 워밍업 비율로 넘긴다) — 대표 run 의 완료 수는 레벨마다 16 · 15 · 18 · 16 으로 요청 16 과 다르다(02 Q3).
- 반복 수: 3(완주 3 · 03 §3.3) · 종류 warm-rerun(같은 running serve 에 재실행) · lite 레그는 1회.
- warm/cold: 스윕은 warm(워밍업 2) · lite 는 cold(1요청 · 워밍업 0) + warm burst(워밍업 1).
- 백엔드: openai-chat(chat 템플릿 모델 · 근거 코드 chat_template) · 벤치 컨테이너 메모리 상한 4096 MiB(`--bench-budget-mib`).
- spec decode: 꺼짐(서빙 설정에 speculative-config 없음 · acceptance length 미기재).
- 측정 노드: main(단일 노드).
- 측정 시각(UTC): 2026-10-10T07:14:23Z(sweep_index generated_utc) · lite 측정 시작 2026-10-10T05:15:57Z(01 §1.4 bench 단계).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- 같은 셀의 이전 GuideLLM 측정 · 없음 · — · 비교 대상 없음 — 6셀 testlog 셀 A 행 "GuideLLM 기록 없음"(testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §2). 따라서 레벨별 나란히 표를 만들 이전 full 측정이 없다.
- 같은 셀의 이전 lite 측정 ① 2026-10-01 01:42~02:23Z(계보 밖 docs/testlog/testlog_26100111_v030_싱글_이종모델_동시서빙_서브지침배달_판정.md L13 · L27) · warm 3.90 t/s · 이미지가 D4 의 `sha256:cdb1a38b…`(이 측정은 `sha256:123c331f…`) · 비교 불가(대조 불완전 — 소프트 지문 키 전부를 대조하지 못했다).
- 같은 셀의 이전 lite 측정 ② 2026-10-01 08:26Z 기동(원장 9행 · 앞선-2 · 계보 밖 docs/testlog/testlog_26100117_하네스결함교정_재빌드일관성_E2E_판정.md §1 E3 행 · §2) · lite 3.89 t/s · cold TTFT 510 ms · READY 490 s · **이 측정과 같은 이미지 `123c331f…`** · 비교 불가(대조 불완전) — 이미지는 같지만 그 문서가 이 draft 의 LINEAGE 에 없어 나머지 소프트 지문 키(KV · gmu · max-len 등)를 기계 대조하지 못했다. 참고로 이 측정의 lite warm 은 3.98 t/s · cold TTFT 530 ms(03 §3.1)이며, 차이를 재현이라 부르지 않는다.
- 같은 E2E 의 다른 셀(gpt-oss-120b 33.40 · Qwen3.8-Flash-Next 37.63 · DeepSeek-V4.1-Flash 35.87 등) · — · 모델 · vLLM 버전 · TP 가 다르다 · 무효 — 강한 일치 키(`model` · `vllm_version` · `tensor_parallel_size`) 불일치(testlog_26101016 §2).
- 자매 셀(같은 모델의 다른 형상) · 없음 · — · 비교 대상 없음 — 이 계보에 Granite 의 다른 양자화 · 컨텍스트 · KV 셀이 없다.
- 외부 레퍼런스 · 없음 · — · 비교 대상 없음 — E 검색 결과 없음(셀 판정 testlog §3). "대역폭 상한 추정 ~4.5 t/s" 는 실측이 아니라 추정이므로 비교 대상으로 쓰지 않는다(02 Q1).
- 루프라인(결정론 상한) · R_fp 4.66 t/s · expected_achievable 1.63 t/s · 비교 기준이 아니라 의심 임계 · 판정 입력 — 측정 3.93 은 R_fp 를 넘지 않는다(verdict.json `physical.exceeds` false). 대역폭 입력은 GB10 lookup 273 GB/s 이고 실측이 아니다(roofline.json `bandwidth_source`).

판정 산정에서 손으로 승계된 입력은 없다 — spec 이 꺼져 있어 acceptance length 는 1.0(`declared-absent` · 결손 아님)이고, MBU 계수는 루프라인 도구 안의 값이며 외부 레퍼런스 E 와 사용자 목표 c 는 둘 다 unset 이다(verdict.json `candidates`).

판정 등급: 루브릭 권한 **weak** 의 PASS(3.93 ≥ floor 1.39 · ratio 2.411 · 기준 = expected_achievable(roofline×MBU))이고, 인증서는 explicit ∧ PASS 에서만 나오므로 발행되지 않았다(03 §3.3 아래 사실 블록). 현행 full 정의(lite 게이트 → GuideLLM × 반복 ≥3)는 **충족**이다(위 사실 블록: 반복 3 · 도구 guidellm). 다만 기준이 외부 실측이 아닌 루프라인 바닥이라 발행 plan 스스로 의미가 약하다고 적었다(02 Q2) — 성능 "완료" 의 근거가 아니라 측정이 성립했다는 기록이다. 이미지 · 드라이버 · KV 클램프 · max-num-seqs 중 하나라도 바뀌면 다시 잰다.
