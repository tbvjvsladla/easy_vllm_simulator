# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 |
|---|---|
| `generated_utc` | 2026-09-23T00:46:29Z |
| `lite.cold_ttft_ms` | 1654.3 |
| `lite.engine_kv_gib` | 20.0 |
| `lite.engine_kv_tokens` | 1343310 |
| `lite.engine_reserved_total_gib` | 59.05 |
| `lite.engine_weights_gib` | 39.05 |
| `lite.gen_src` | median_tpot |
| `lite.gen_tps` | 20.16 |
| `lite.kv_gib` | 20.0 |
| `verdict_point_level` | 1 |
| `source` | sweep_index(output/multi/benchlog/sweep_nv4-bf-262k-mmp · output) |

**`levels`**(5행)

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 1.891089108910891 | 16 | 20.98 | 21.1 | 0 | 43.60977434644512 | 1 | true | 21.217912464578706 | 47.6599233224988 | 1077.6431560516357 |
| 2 | ok | 1.891089108910891 | 16 | 17.87 | 17.72 | 0 | 51.662374945247876 | 2 | true | 35.60823870651653 | 55.97250256687403 | 1145.5445289611816 |
| 4 | ok | 1.891089108910891 | 16 | 11.71 | 12.18 | 0 | 79.53620424457625 | 4 | true | 42.40717943512994 | 85.40844917297363 | 1285.7983112335205 |
| 8 | ok | 1.891089108910891 | 16 | 9.96 | 10.03 | 0 | 89.87207693212173 | 8 | true | 57.41591236577993 | 100.42194183915854 | 2146.3093757629395 |
| 16 | ok | 1.891089108910891 | 18 | 6.06 | 6.64 | 0 | 124.09704058778053 | 16 | true | 79.32202723207973 | 165.13907816261053 | 7760.097980499268 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26092309_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 20.98 | 21.22 | 110.40 | 1077.64 | 43.61 | 16/0 |
| 2 | 17.87 | 35.61 | 185.27 | 1145.54 | 51.66 | 16/0 |
| 4 | 11.71 | 42.41 | 220.65 | 1285.80 | 79.54 | 16/0 |
| 8 | 9.96 | 57.42 | 298.74 | 2146.31 | 89.87 | 16/0 |
| 16 | 6.06 | 79.32 | 412.72 | 7760.10 | 124.10 | 18/0 |

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
| `source` | bench_report(bench_report_26092309_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> **인증서가 없다.** 인증서는 full 모드 verdict==PASS 일 때만 나오며 그 발행은 `adversarial-benchmark` 의 책임이다.
> 부재는 '성능이 나빴다' 가 아니라 '**그 형태로 판정되지 않았다**' 는 뜻이다 — 위 수치는 관측이지 판정이 아니다.

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_CERTIFICATE` | 인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. 인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다. |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | `546393d901ee` 2026-09-23T01:45:47Z | `sweep_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | df9a0781c546..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | df9a0781c546..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `692297be56a3` 2026-09-14T13:23:51Z | df9a0781c546..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@692297be56a3` | `git show 692297be56a38ed79ff6521e31c6586bd153437c:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_nv4-bf-262k-mmp/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**실행 원문 명령 줄은 계보에 없다** — 이 측정의 드라이버 호출 줄(sweep_bench 인자 전체)은 어느 문서에도 글자 그대로 남지 않았다. 아래는 측정 시각 이전 마지막 커밋의 도구 원문이다. 01 §1.4 bench 단계의 재구성 명령은 lite 레그 두 줄뿐이고, 동시성 레벨 레그는 GuideLLM 이다(bench JSON 재구성이 아니다). 위 도구 표가 기록한 sweep_bench.sh 의 측정 뒤 첫 변경(546393d901ee)은 레벨 측정 전에 도구별 옛 raw 파일(`bench_<cell>.json` · `guidellm_<cell>.json`)을 지우는 정리 3줄이다(`git show 546393d901ee -- .claude/skills/adversarial-benchmark/scripts/sweep_bench.sh`). 이 측정의 레벨 자리에는 09-12 vllm bench 의 `bench_nv4-bf-262k-mmp.json` 이 남아 있었지만, 레벨 수치(`level_NN/measured.json`)는 `bench_tool: guidellm` 원시에서 나왔다 — 이 변경은 이 측정이 무엇을 쟀는지를 바꾸지 않는다.

**레벨 레그(동시성 1 · 2 · 4 · 8 · 16 · 레벨마다 반복 3)** — `sweep_bench.sh` 가 레벨마다 `run_bench.sh --tool guidellm` 을 부른다. 기본 입력 조건:

> [원문] sweep_bench.sh@434fa6740831 §L55-55
> TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0

GuideLLM 호출(warmup 은 요청 수가 아니라 비율로 바꿔 넘기고 총 요청을 그만큼 늘린다 — §L307-312):

> [원문] run_bench.sh@434fa6740831 §L381-386
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

**lite 레그(cold 1요청 · warm 3요청 · 동시성 1 · 1회)** — `lite_bench.sh` 의 `vllm bench serve`:

> [원문] lite_bench.sh@434fa6740831 §L136-141
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url $BASE_URL --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

**조건 목록**
- 도구 · 버전: 레벨 레그 = GuideLLM 0.7.3(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · 스윕 meta 에 digest 실측) · lite 레그 = 서빙 이미지 안 `vllm bench serve`.
- 엔드포인트: 레벨 레그 `/v1/chat/completions`(스윕 meta `bench_endpoint`) · lite 레그도 `/v1/chat/completions`(bench JSON `backend: openai-chat` → `lite_bench.sh@434fa6740831` L64 매핑 · `lite_raw` JSON `endpoint` 기록). lite JSON 의 입력 길이 566/565 는 도구 입력 512 에 채팅 템플릿이 더해진 값으로 보인다(추론 — 템플릿 토큰 수를 잰 기록은 없다).
- 입출력: 레벨 레그 합성 텍스트 1024 / 256 토큰(`ignore_eos` true) · lite 레그 random 512 / 128(`--random-range-ratio 0` · `--ignore-eos`).
- 요청 수 · warmup: 레벨당 16 + warmup 2(GuideLLM 에는 비율 `WARM_FRAC` 로 전달 · 총 요청 18) · 동시성 16 레벨은 완료 18 로 기록됐다(03 §3.1). lite = cold 1 요청 warmup 0 · warm 3 요청 warmup 1.
- 샘플링 temperature: 레벨 레그 GuideLLM 호출에는 temperature 인자가 없다(원문 없음 — 도구 기본값). seed 도 원문 없음.
- 반복: 3(`warm-rerun` · 같은 running serve · 선언 출처 캠페인 budgets.repeats=3) · 완주 3 — 3.3 표와 같다.
- spec decode: 켜짐 · MTP k=3(서빙 yaml) · 판정에 쓴 수용길이 1.891(lite warm 레그에서 승계한 실측 — 레벨 레그 자체의 수용길이가 아니다).
- 측정 노드: cluster(메인 API 서버에 붙은 클라이언트 · TP=2 쌍이 하나의 측정 정체성).
- 측정 시각: 스윕 조립 `generated_utc` 2026-09-23T00:46:29Z · 서빙 창 2026-09-22T23:42:54Z ~ 2026-09-23T00:48:05Z(01 §1.4 원장).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:63` 원문 그대로):

> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26092309_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26092309_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 측정에는 인증서가 없으므로 비교의 이 측정 쪽 지문은 스윕 meta(3.1 출처 · bench_report_26092309 측정 환경 표)에서 읽는다.

- 같은 셀 2026-09-12 인증 측정(`benchmark_26091304` · camp-26091216 A1) · 38.81 t/s · 강한 일치 키 6개(model · gpu_model · vllm_version 0.29.0 · quantization N/A · topology multi · TP 2) 동일 · 소프트 지문 중 다른 키 = `image_digest`(85cef278… → a2c4ca49…) · `bench_tool`(vllm-bench-serve → guidellm) · `bench_tool_version`(N/A → 0.7.3) — driver_version · cuda_version · image_tag · max_model_len · max_num_seqs · kv_cache_memory_bytes · kv_cache_dtype · gmu · moe_backend · enforce_eager · attention_backend · ple_mode 는 같다 · ngc_base_tag 는 이전 N/A · 이 측정 meta 에 키 없음(양쪽 미기록)(단 driver_version 은 양쪽 모두 manifest 선언값이고 이 실행 attestation 관측은 580.178.04 — 00 §0.4) · 부하 조건: 측정 도구가 다르고 엔드포인트가 /v1/completions(testlog_26091304 §1) 대 /v1/chat/completions, 동시성 1 의 (총 ÷ 출력) 비가 5.0(174.95 ÷ 34.99) 대 5.2(110.40 ÷ 21.22) · **비교 불가** — 도구 · 요청당 토큰 구성이 다르다(bench_report_26091304 · 이 파일 3.2).
- 같은 셀 2026-09-10 레거시 측정(`benchmark_26091011` · camp-26090918) · 35.39 t/s · 강한 키 동일 · 소프트 지문 다른 키 = `image_digest`(85cef278… → a2c4ca49…) · `moe_backend`(N/A → flashinfer_cutlass) · `bench_tool`(vllm-bench-serve → guidellm) · `bench_tool_version` · `ple_mode`(인증서에 키 없음 → mmap) · ngc_base_tag(N/A → 키 없음) — 나머지 소프트 키는 같다 · 부하 조건: (총 ÷ 출력) 비는 5.2 로 같지만 측정 도구가 다르다 · **비교 불가** — 도구가 다르다. 그 측정의 합격선(expected 3.02)은 루프라인 결함 위의 값이다(02 M4).
- 자매 셀 `nv4-bf-262k-res-kv8g-gmu80`(`benchmark_26091223`) · 46.70 t/s · 소프트 지문 다른 키 = image_digest · max_num_seqs(8) · kv_cache_memory_bytes(8589934592) · gmu(0.80) · bench_tool(vllm-bench-serve) · bench_tool_version · ple_mode(resident) · ngc_base_tag(N/A → 키 없음), 서빙 노브 max-num-batched-tokens 2048 도 다르다(testlog_26091304 §0) · **비교 불가** — 도구가 다르다. 형상 비교로도 PLE 방식 · KV · gmu · 배치 노브가 한꺼번에 달라 어느 하나의 효과로 읽을 수 없다.
- 자매 셀 `nv4-f8-262k-mmp`(`benchmark_26091111`) · 38.18 t/s · 소프트 지문 다른 키 = image_digest · kv_cache_dtype(fp8_e4m3) · moe_backend(triton) · bench_tool(vllm-bench-serve) · bench_tool_version · ngc_base_tag(N/A → 키 없음) · **비교 불가** — 도구가 다르다.
- 외부 레퍼런스(NVIDIA 공식 NVFP4 · 동일 체크포인트 · testlog_26091304 §2 인용): 2 Spark SPEED 53.7 median(PLE resident · MTP3 · KV fp8) · 1 Spark 32.5 median(PLE mmap) · **비교 불가** — 앞은 PLE 방식 · KV dtype 이 다르고 뒤는 노드 수가 다르며, 둘 다 측정 도구 · 요청 구성이 기록되지 않았다. 이 셀 형상(2노드 · mmap)의 like-with-like 외부 수치는 없다.

**같은 셀의 이전 측정 — 레벨 전부**(값은 두 원천의 글자 그대로 · 비 = 이 측정 ÷ 이전 · 소수 3자리 반올림). 위 판정대로 **비교 불가**이며, 이 표는 차이의 크기를 보이려는 것이지 같은 조건의 재현 비교가 아니다.

| 동시성 | 이 측정 | 이전 측정(2026-09-12 · 38.81 인증) | 비 |
|---|---|---|---|
| 1 | 20.98 | 38.81 | 0.541 |
| 2 | 17.87 | 31.1 | 0.575 |
| 4 | 11.71 | 23.05 | 0.508 |
| 8 | 9.96 | 16.73 | 0.595 |
| 16 | 6.06 | 10.93 | 0.554 |

| 동시성 | 이 측정 | 이전 측정(2026-09-10 · 35.39 레거시) | 비 |
|---|---|---|---|
| 1 | 20.98 | 35.39 | 0.593 |
| 2 | 17.87 | 29.53 | 0.605 |
| 4 | 11.71 | 22.64 | 0.517 |
| 8 | 9.96 | 15.9 | 0.626 |
| 16 | 6.06 | 11.51 | 0.526 |

- 수용길이(spec 은 셋 다 켜짐 · MTP k=3): 이 측정 1.891 · 09-12 2.593 · 09-10 2.379 — 비(이 측정 ÷ 이전) 0.729 · 0.795. 동시성 1 decode 비 0.541 · 0.593 보다 수용길이 비가 커서 **처리량 차이가 수용길이 비로 설명되지 않는다**(나머지 원인은 분석되지 않았다 — 02 Q1). 이 측정의 수용길이는 lite warm 레그에서 승계한 값이라 GuideLLM 레그 자체의 값과 같다는 보장도 없다.
- 자체 재현 밴드: 이 측정 동시성 1 의 반복 밴드는 3.12%(n=3 · bench_report_26092309 반복 축 표)다. 38.81 → 20.98 의 차이는 이 밴드를 크게 넘으므로 "같은 범위 재현" 이 아니다 — 차이는 수치로 위와 같고 원인은 설명되지 않았다.
- 판정 산정 입력: primary = expected_achievable 29.8(R_fp 45.03 × 수용길이 1.891 × 0.35) · tolerance 0.15 → floor 25.33 · ratio 0.704. 수용길이는 이번에 손으로 넘기지 않고 판정 레벨 measured.json 에서 자동 승계했다(원천 = lite warm). 외부 레퍼런스 E 는 검색되지 않았다(verdict warning E-not-attempted).

**판정 등급.** 판정 권위 = explore(`verdict_rule.py` · roofline×MBU 가 primary · E 미시도로 roofline-only 로 강등). 결과 **REFUTE** — 유효 측정(15회 레벨 측정 완주 · 실패 0)이 탐색 합격선 아래라는 뜻이며 기능 실패가 아니다. 인증서는 full PASS 에서만 나오므로 없다. waiver 없음. 현행 full 정의 충족: **예**(위 사실 블록 — 반복 3 ≥ 3 · 도구 guidellm). 이 수치는 **관측 게재(OBSERVATION-ONLY)** 이지 baseline 이 아니다 — 20.98 을 기준선으로 쓰지 말고, 38.81 이 이 빌드에서 재현된다고 가정하지도 말라. 재측정 조건: E 검색을 포함한 적대 재검증, 그리고 같은 serve 에서 도구 · 엔드포인트를 바꿔 잰 대조(02 Q1 · Q2).
