# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 | 출처 |
|---|---|---|
| `measured_utc` | 2026-10-10T07:45:58Z | sweep_index.generated_utc(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `accept_len` | 미기재 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json measured_accept_len = null(측정값 없음) · rubric.spec_axis: declared=off · accept_len=1.0 · accept_len_source=declared-absent · missing=false — spec 미선언이라 루브릭이 1.0 을 쓴다(선언값 · 측정 아님 · 결손 아님) |
| `benchmark_mode` | full | measurement_config.bench_mode(bench_report(bench_report_26101016_gpt-oss-120b_GB10_0.19.1.md)) |
| `decode_tps_conc1` | 33.4 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) measured_decode_tps |
| `floor_tps` | 28.5 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.floor |
| `generated_utc` | 2026-10-10T07:45:58Z | — |
| `lite.cold_ttft_ms` | 70.4 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.engine_kv_gib` | 30.0 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.engine_kv_tokens` | 873808 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.engine_reserved_total_gib` | 95.96 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.engine_weights_gib` | 65.96 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.gen_src` | median_tpot | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.gen_tps` | 35.11 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `lite.kv_gib` | 30.0 | sweep_index.lite(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| `primary_source` | E(external_reference) | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.source |
| `primary_tps` | 33.53 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.primary |
| `ratio_M_over_primary` | 0.996 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.ratio_M_over_primary |
| `rubric_authority` | weak | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.authority |
| `spec_on` | false | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) measured_spec_on |
| `tolerance` | 0.15 | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) rubric.tolerance |
| `verdict` | PASS | output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) verdict |
| `verdict_point_level` | 1 | sweep_index(output/single/benchlog/sweep_gptoss120b-0191-c0 · output) |
| `source` | 판정 원천(verdict.json) + sweep_index(output/single/benchlog/sweep_gptoss120b-0191-c0 · output) | — |

**`levels`**(5행 · 출처 sweep_index(output/single/benchlog/sweep_gptoss120b-0191-c0 · output))

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 미기재 | 16 | 33.4 | 33.39 | 0 | 28.685213537777173 | 1 | true | 33.48658484352922 | 29.941911809146404 | 351.60136222839355 |
| 2 | ok | 미기재 | 16 | 30.35 | 30.32 | 0 | 32.72883284325693 | 2 | true | 60.62603728790118 | 32.94531162828207 | 86.77053451538086 |
| 4 | ok | 미기재 | 15 | 21.16 | 22.03 | 0 | 46.962631449979895 | 4 | true | 68.01817747558152 | 47.252120450139046 | 95.24822235107422 |
| 8 | ok | 미기재 | 16 | 15.36 | 16.16 | 0 | 64.89680982103535 | 8 | true | 97.4357278107571 | 65.1104561984539 | 112.10131645202637 |
| 16 | ok | 미기재 | 17 | 10.27 | 11.13 | 1 | 97.13385058384316 | 16 | true | 130.40111741294254 | 97.41169959306717 | 144.04892921447754 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26101016_gpt-oss-120b_GB10_0.19.1.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 33.40 | 33.49 | 167.56 | 351.60 | 28.69 | 16/0 |
| 2 | 30.35 | 60.63 | 303.37 | 86.77 | 32.73 | 16/0 |
| 4 | 21.16 | 68.02 | 340.36 | 95.25 | 46.96 | 15/0 |
| 8 | 15.36 | 97.44 | 487.56 | 112.10 | 64.90 | 16/0 |
| 16 | 10.27 | 130.40 | 652.51 | 144.05 | 97.13 | 17/1 |

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
| `source` | bench_report(bench_report_26101016_gpt-oss-120b_GB10_0.19.1.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> 인증서 비발행 판정(결손 아님) — `PASS` — 출처 output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) verdict · 인증서는 explicit ∧ PASS 판정에서만 나온다(루브릭 권한 `weak`).

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

- `sweep_bench.sh` — 스윕 색인 조립자(output/single/benchlog/sweep_gptoss120b-0191-c0/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

실행한 측정 명령은 셀 판정 testlog 에 남아 있다(GuideLLM 스윕 · 판정점 = 동시성 1 재사용):

> [원문] testlog_26101017_08_29_knob프리셋_E2E_셀B_gptoss120b_0191_c0_판정 §2. 측정
> ```bash
> bash .claude/skills/adversarial-benchmark/scripts/sweep_bench.sh gptoss120b-0191-c0 --topology single --tool guidellm --backend openai --knob-reason backend=harmony_completion --bench-budget-mib 2048 --knob-reason bench_budget_mib=host_budget
> ```

**동시성 스윕 레그(GuideLLM · run_bench.sh 가 부른다)** — bench JSON 에 없는 인자는 측정 도구 원문에서 읽는다. GuideLLM 에는 `ignore_eos` 가 backend extras 로 들어가고, 입출력 길이는 합성 데이터 지정으로 고정되며, warmup 은 요청 수가 아니라 비율로 넘어간다:

> [원문] run_bench.sh@2661d614cf52 §L399-403
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

GuideLLM 호출에는 temperature · seed · range-ratio 인자가 없다(위 원문 · 도구 기본값에 맡긴다 — 그 기본값은 이 페이로드가 확인하지 않았다).

**lite 레그(vLLM bench serve · lite_bench.sh 가 서빙 컨테이너 안에서 부른다)** — cold(요청 1 · warmup 0)와 warm burst(요청 3 · warmup 1):

> [원문] lite_bench.sh@803064d23d51 §L173-176
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

lite 레그에는 temperature 인자가 없다(위 원문). 01 §1.4 의 lite 재구성 명령도 입력 512 · 출력 128 을 냈으니 원문과 맞는다.

조건 목록:
- 도구와 버전: 동시성 스윕 = GuideLLM 0.7.3(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · 3.3 측정 구성) · lite = 서빙 컨테이너 안의 vLLM 0.19.1 `vllm bench serve`.
- 엔드포인트: 두 레그 모두 `/v1/completions`(`--backend openai` · `b_sweep.log` L10).
- 입력/출력 토큰: 스윕 1024 / 256(프리셋 `default_preset` 기본 · `b_sweep.log` L2) · lite 512 / 128.
- 동시성 레벨: 1 · 2 · 4 · 8 · 16(max-num-seqs 16 · 상한 필터 제외 없음).
- 레벨당 요청: 16 + warmup 2(총 18 요청 상한 · warmup 비율 = 2 ÷ 18) · 집계 완료 수는 레벨마다 15~17(03 §3.1 levels 표 — 레벨 16 은 실패 1건).
- 반복: 레벨마다 3회 · 종류 `warm-rerun`(같은 serve 연속) · 출처 `defaulted(preset default_preset 기본 3 · 활성 캠페인 없음(_bootstrap))` · 완주 3/3. 스윕 표와 판정점은 첫 완주 run 의 값이다(bench_report_26101016 §반복 축 · 재현 밴드).
- warm/cold: 스윕은 warmup 이 있는 warm 측정 · cold TTFT(70.4 ms)는 lite cold 레그(warmup 0 단일 요청)에서만 잰다.
- 벤치 컨테이너 메모리 상한: 2048MiB(W5).
- spec decode: 꺼짐(`spec_on: false` · acceptance length 미기재 — 해당 없음).
- 측정 노드: main(single · TP=1).
- 측정 시각(UTC): lite 시작 2026-10-10T07:25:51Z → 스윕 색인 2026-10-10T07:45:58Z(01 §1.4 bench 행).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:42` 원문 그대로):

> full 계측(lite 게이트 → `GuideLLM × 반복 ≥3` · lite ⊂ full)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26101016_gpt-oss-120b_GB10_0.19.1.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26101016_gpt-oss-120b_GB10_0.19.1.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

- **같은 셀의 이전 측정(2026-09-04 광의탐색 C0)** · 동시성 1 32.83 t/s · 같은 트리플렛 좌표(지도의 kv fp8 · kv-bytes 32212254720 · max-len 131072 · seqs 16 · gmu 0.9 · moe marlin · attention triton_attn · 이미지 태그와 digest `sha256:026e9d87…` 동일) · 같은 도구(GuideLLM 0.7.3) · 같은 엔드포인트(`/v1/completions`) · 형상은 셀 판정이 "같은 형상" 으로 적었다 · **비교 가능(형상 비교)** — 단 그 측정은 인증서가 없어(권한 explore) 소프트 지문 중 driver · cuda · ngc · enforce_eager 를 대조할 수 없고, 반복 수와 재현 밴드 기록이 없다. 원천: sweep_map_26090504_gptoss120b_0191_single_완주 §1. · testlog_26101017_08_29 머리.

  | 동시성 | 이 측정 | 이전 측정(09-04) | 비 |
  |---|---|---|---|
  | 1 | 33.40 | 32.83 | 1.017 |
  | 2 | 30.35 | 29.62 | 1.025 |
  | 4 | 21.16 | 21.47 | 0.986 |
  | 8 | 15.36 | 14.78 | 1.039 |
  | 16 | 10.27 | 9.96 | 1.031 |

  (비 = 이 측정 ÷ 이전 · 소수 3자리 반올림.) 판정점 차이 +1.7% 는 이 측정 자신의 판정점 재현 밴드 3.94%(run 3개 33.40 / 34.75 / 34.66) 안이다. 다른 레벨은 이 측정의 레벨별 밴드와 견주면 레벨 4(−1.4% · 밴드 4.20%)만 밴드 안이고 레벨 2(+2.5% · 밴드 0.23%) · 레벨 8(+3.9% · 밴드 2.22%) · 레벨 16(+3.1% · 밴드 2.22%)은 밴드를 넘는다 — 그 차이의 원인은 설명되지 않았다(두 측정 사이에 달라진 것으로 확인되는 것은 반복 축 도입 · 프리셋 계층 · overhead 선언이고, 09-04 의 벤치 컨테이너 메모리 상한은 기록에 없다 — 어느 것이 원인인지 가르는 대조는 없다). 이전 측정의 밴드는 없으므로 이것을 "같은 범위 재현" 이라고 부르지 않는다.
- **같은 모델의 이전 인증(2026-09-01 · 인증서 benchmark_26090117)** · 동시성 1 34.55 t/s · 강한 키 6개(model · gpu · vllm 0.19.1 · mxfp4 · single · TP=1) 일치 · 소프트 지문 키 전부(driver_version · cuda_version · image_tag · max_model_len · max_num_seqs · kv_cache_memory_bytes · kv_cache_dtype · gpu_memory_utilization · moe_backend · enforce_eager · ngc_base_tag)를 대조하면 — 표기상 같은 것은 driver 580.173.02 · cuda 132 · image_tag · max_model_len · max_num_seqs · kv bytes · kv fp8 · gmu 0.9 · enforce_eager N/A 이고, 다른 것은 moe_backend(N/A 대 marlin — 그쪽은 선언만 봐서 비었다 · plan_26090419 §8.1)다 · ngc_base_tag 는 인증서 N/A(리포트에 키 없음)라 대조 불가 · driver 는 양쪽 모두 manifest 에서 옮긴 선언값이고 이번 엔진 로그는 커널 드라이버 580.178.04 를 관측했다 · image_tag 는 같지만 측정 이미지는 2026-09-03 재빌드(인증 측정 2026-09-01 보다 뒤)이고 인증서에 image_digest 가 없어 같은 이미지라고 말할 수 없다 — 따라서 stale 이기도 하다 · 측정 도구가 다르다 — 그 인증서에는 bench_tool 키가 없고, GuideLLM 전환은 2026-09-04 의 일이므로 vLLM bench serve 로 쟀다고 추론한다 · 요청당 토큰 구성(총 tok/s ÷ 출력 tok/s)은 레벨 1 에서 양쪽 모두 약 5.0 으로 같다 · **비교 불가**(측정 도구가 다르다) — 참고로만: 34.55 대 33.40. 원천: bench_report_26090117_gpt-oss-120b_GB10_0.19.1 · docs/benchmark/benchmark_26090117_gpt-oss-120b_GB10_0.19.1.yaml.
- **자매 셀(2026-09-04 C2 · kv auto=bf16 · 트리플렛 gptoss120b-0191-c2)** · 동시성 1 32.71 t/s(C0 32.83) · 소프트 지문 kv_cache_dtype 하나가 다르다(지도 좌표 기준 · 그 밖의 좌표는 같다) · **비교 가능(형상 비교 · stale: kv_cache_dtype)** — 같은 날 같은 도구에서 KV dtype 은 속도를 바꾸지 않았고 용량만 절반이었다. 이 측정과 직접 견주지 않는다(측정일 · 반복 수가 다르다). 원천: sweep_map_26090504 §2.
- **외부 레퍼런스 E 33.53 t/s** · 같은 DGX Spark GB10 · 같은 gpt-oss-120B MXFP4 · vLLM 공식 nvcr.io 이미지 · 동시성 1 · 그 측정의 도구 · 입출력 길이 · vLLM 버전은 계보에 없다 · **비교 불가**(부하 조건 미상) — 그런데도 이 셀의 판정 primary 로 쓰였다. 원천: testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §판정이 REFUTE → PASS 로 뒤집힌 경위.
- **같은 모델의 다른 vLLM 버전 hint(0.18.0 · 0.19.0 2노드 TP=2 · 0.26.0 single)** · 강한 키 vllm_version(그리고 2노드 쪽은 topology · TP)이 다르다 · **무효**. 0.26.0 single 의 11.24 t/s 는 입력 길이도 2048 대 1024 로 다르다(devlog_26090117_15_53 §재개 지침).

**손으로 승계된 판정 입력**: 루브릭 primary E 33.53 t/s(2026-09-01 판정에서 `--reference-tps 33.53` 으로 승계 · testlog_26101017_08_29 §3) · tolerance 0.15 → floor 28.5 · ratio 0.996. spec 이 꺼져 있어 acceptance length 승계는 없다. 루프라인 expected_achievable 15.51 t/s 는 판정 primary 가 아니다(primary 는 E).

**판정 등급**: 권한 `weak` 의 PASS(33.40 ≥ floor 28.5) — 인증서는 explicit ∧ PASS 에서만 나오므로 발행되지 않았고 판정 표면은 bench_report 의 판정 표다(3.3 아래 사실 블록). 현행 full 정의는 **충족한다**(위 사실 블록 — 반복 3 ≥ 3 · 도구 guidellm). waiver · 관측 게재가 아니다. 재측정이 필요한 조건: 드라이버 · 이미지 digest · KV 클램프 · overhead 선언이 바뀔 때, 그리고 E 33.53 의 측정 조건을 확인하지 못한 채 explicit 권한 판정이 필요할 때.
