# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 |
|---|---|
| `measured_utc` | 2026-09-28T00:35:13Z |
| `accept_len` | 2.238372093023256 |
| `bench_tool` | guidellm |
| `bench_tool_version` | 0.7.3 |
| `benchmark_mode` | full |
| `decode_tps_conc1` | 32.2 |
| `floor_tps` | 29.55 |
| `generated_utc` | 2026-09-28T00:35:13Z |
| `lite.cold_ttft_ms` | 7419.7 |
| `lite.engine_kv_gib` | 10.0 |
| `lite.engine_kv_tokens` | 1941478 |
| `lite.engine_reserved_total_gib` | 89.04 |
| `lite.engine_weights_gib` | 79.04 |
| `lite.gen_src` | median_tpot |
| `lite.gen_tps` | 31.82 |
| `lite.kv_gib` | 10.0 |
| `lite_cold_ttft_ms` | 7419.7 |
| `lite_gen_src` | median_tpot |
| `lite_gen_tps_warm` | 31.82 |
| `lite_included` | true |
| `lite_kv_gib` | 10.0 |
| `primary_source` | expected_achievable(roofline×MBU) |
| `primary_tps` | 34.77 |
| `ratio_M_over_primary` | 0.926 |
| `rubric_authority` | explore |
| `spec_on` | true |
| `sweep_levels` | 1,2,4,8,16 |
| `sweep_truncated` | N/A |
| `tolerance` | 0.15 |
| `verdict` | PASS |
| `verdict_point_level` | 1 |
| `source` | certificate(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) + sweep_index(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · output) |

**`levels`**(5행)

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.238372093023256 | 16 | 32.2 | 32.17 | 0 | 28.650825163897345 | 1 | true | 32.31990784865666 | 31.05470910668373 | 632.169246673584 |
| 2 | ok | 2.238372093023256 | 17 | 16.2 | 16.09 | 0 | 30.502641902250403 | 2 | true | 32.27151705186906 | 61.71696446835995 | 8218.917608261108 |
| 4 | ok | 2.238372093023256 | 18 | 8.53 | 9.12 | 0 | 28.762664046942017 | 4 | true | 33.29128519756649 | 117.18443501740694 | 22664.736032485962 |
| 8 | ok | 2.238372093023256 | 18 | 4.35 | 5.32 | 0 | 29.025830474554322 | 8 | true | 33.648744055134365 | 229.89172209054232 | 51163.946866989136 |
| 16 | ok | 2.238372093023256 | 18 | 3.69 | 3.52 | 0 | 28.810880698409736 | 16 | true | 32.53787325669283 | 270.9591891616583 | 62887.30812072754 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 32.20 | 32.32 | 172.08 | 632.17 | 28.65 | 16/0 |
| 2 | 16.20 | 32.27 | 171.82 | 8218.92 | 30.50 | 17/0 |
| 4 | 8.53 | 33.29 | 177.25 | 22664.74 | 28.76 | 18/0 |
| 8 | 4.35 | 33.65 | 179.15 | 51163.95 | 29.03 | 18/0 |
| 16 | 3.69 | 32.54 | 173.24 | 62887.31 | 28.81 | 18/0 |

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
| `source` | bench_report(bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
**측정 결손**: _없음 — 측정 결손 코드(`HINT_MISSING_CERTIFICATE` · `HINT_MISSING_BENCH_REPORT` · `HINT_MISSING_SWEEP_LEVELS` · `HINT_MISSING_LITE` · `HINT_MISSING_MEASURED_NODE` · `BENCH_MODE_LITE` · `HINT_MISSING_SWEEP` · `HINT_MISSING_SWEEP_RAW`)를 대조한 결과 0건이다(없음도 적는다)._
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 4건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `a21e66eb26e5` 2026-09-23T06:53:32Z | 8319b4f05b68..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@a21e66eb26e5` | `git show a21e66eb26e551a8803750cdad06ffc03bbd352d:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | 8319b4f05b68..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `fcb0754fffe1` 2026-09-23T01:56:11Z | 8319b4f05b68..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@fcb0754fffe1` | `git show fcb0754fffe107962034fec748ed8823756c93b6:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `692297be56a3` 2026-09-14T13:23:51Z | 8319b4f05b68..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@692297be56a3` | `git show 692297be56a38ed79ff6521e31c6586bd153437c:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

이 셀의 full 스윕은 `broad_search.sh cell`(드라이버 — 이 셀은 호출 기록이 있다: devlog_26092809 §무엇을 했나 5 · sweep_map_26092809 · `--backend openai-chat`)이 `sweep_bench.sh` 를 부르고, 그것이 레벨마다 `run_bench.sh`(GuideLLM) · 선행 1회 `lite_bench.sh`(`vllm bench serve`)를 부른 결과다. 실행한 명령 줄 원문은 계보 문서에 없다 — 아래는 측정 시각 이전 마지막 커밋의 도구 원문이고, 01 §1.4 bench 단계의 두 줄은 lite JSON 에서 재구성한 명령이다.

스윕 기본값(레벨 · 입출력 길이 · 프롬프트 수 · 워밍업) — 이 셀은 레벨 · 길이를 덮어쓰지 않았다(3.1 `sweep_levels` 1,2,4,8,16 · bench_report 측정 환경 스냅샷 `입력 길이(sweep)` 1024):

> [원문] sweep_bench.sh@a21e66eb26e5 §L55-55
> TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0

full 레그(GuideLLM) — 도구가 정한 인자는 `ignore_eos:true` · 동시 스트림 프로파일 · 합성 텍스트 데이터 · 요청 수 상한(프롬프트 + 워밍업):

> [원문] run_bench.sh@434fa6740831 §L381-386
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

lite 레그(`vllm bench serve`) — 입력 512 · 출력 128 · range-ratio 0 · 동시성 1 · `--ignore-eos`(temperature 인자는 이 판본 lite 명령에 없다 · full 레그의 `vllm bench serve` 경로에는 `--temperature 0` 이 있으나 이 셀 full 레그는 GuideLLM 이다):

> [원문] lite_bench.sh@fcb0754fffe1 §L136-141
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url $BASE_URL --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

조건 요약:
- 도구 · 버전: GuideLLM 0.7.3(full 레그 · 3.3 표) · lite 는 서빙 이미지 안 `vllm bench serve`.
- 입출력: full 1024 / 256 토큰(합성 텍스트) · lite 512 / 128(01 §1.4 재구성의 595 / 128 은 JSON 합계 ÷ 완료 수라 요청 원문 길이가 아니다 — 512 와의 차이가 chat 템플릿 토큰이라는 것은 추론).
- 레벨 · 반복: 동시성 1/2/4/8/16 · 레벨마다 같은 serve 에 반복 3(warm-rerun · 3.3 `repeats` 3) · lite 선행 레그 1회.
- spec: on(dspark k=7) · 판정 레벨 accept_len 2.238372093023256(실측 승계 · 3.1).
- 요청 포맷: `--backend openai-chat`(devlog_26092809 §무엇을 했나 5 — 09-09 기본과 같은 포맷).
- reload 없음 · request-rate inf.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:63` 원문 그대로):

> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

**비교 대상**(강한 일치 키 = model · gpu_model · vllm_version · quantization · topology · tensor_parallel_size — 이 인증서 머리 주석의 규칙)

- 09-09 대조군 `E1-combo`(= 셀 `e-1m-kvfp8-e1combo` · 같은 레시피) · 동시성1 30.54 t/s · 강한 키 6개 같음(sweep map 측정 조건의 `vllm_version` 0.29.0 · `gpu_model` NVIDIA GB10 · 좌표 quantization fp8 · TP 2 · 모델 동일) · 소프트 지문 대조: `image_digest` **다름**(09-09 `sha256:4c9ab74a…` · 이 셀 `sha256:a2c4ca49…`) · max_model_len 1048576 · max_num_seqs 1 · kv_cache_memory_bytes 10737418240 · kv_cache_dtype fp8 · gmu 0.85 · moe humming · attention N/A · bench_tool guidellm 0.7.3 은 같음 · driver_version 580.173.02 · cuda_version 132 · enforce_eager false 도 같음(09-09 sweep_index meta · `output/multi/benchlog/sweep_e-1m-kvfp8-e1combo/sweep_index.json`) · 이 셀 인증서의 ple_mode(resident) · ngc_base_tag(N/A)는 09-09 meta 에 키가 없어 대조 불가 · 부하 조건(입력 1024 · 출력 256 · 레벨 · spec k=7)은 같다 · 커널(6.17 계열 → 7.0.0-1019)은 인증서 키가 아니지만 달라졌다 · NCCL 전송은 같으나 NCCL env 일부(DMABUF_ENABLE=0 이 이 셀에만 · GDR_LEVEL/C2C/READ 선언값 · NET=IB 명시)가 다르다(M2) · **비교 가능**(강한 키 일치 · 소프트 지문 image_digest 불일치로 stale 경고 등급) — sweep_map_26090912_e1m_levers §셀.
- 09-09 768K 자매 `L4-combo` · 동시성1 31.12 · 강한 키 같음 · 소프트 지문 max_model_len(786432) · max_num_seqs(2) · image_digest 다름(셋 다 소프트 지문 키) · 부하 조건(GuideLLM 0.7.3 · 1024/256 · 레벨 1~16 · spec k=7) 같음 · **비교 가능(형상 비교 · stale 경고 등급 — 컨텍스트 · 동시 시퀀스가 다른 형상이라 순위 ✗)** — sweep_map_26090912_b768k_levers §셀.
- 09-09 1M 기본 `E0-base` · 동시성1 16.67 · spec off · eager — 소프트 지문 enforce_eager 와 spec 이 다름 · **비교 불가(레버 기준선)** — sweep_map_26090912_e1m_levers §셀.
- 외부 레퍼런스 · 없음 · E-search 상태 no · **비교 대상 없음** — bench_report_26092809 §판정.

**같은 레시피의 이전 측정과 레벨 전부**(값은 두 원천 글자 그대로 · 비 = 이 측정 ÷ 이전 · 소수 셋째 자리 반올림):

| 동시성 | 이 측정 | 이전 측정(09-09 E1-combo) | 비 |
|---|---|---|---|
| 1 | 32.2 | 30.54 | 1.054 |
| 2 | 16.2 | 15.77 | 1.027 |
| 4 | 8.53 | 8.28 | 1.030 |
| 8 | 4.35 | 4.5 | 0.967 |
| 16 | 3.69 | 3.53 | 1.045 |

동시성1 의 차이(비 1.054)는 이 셀 판정점 재현 밴드 3.15%(3.1 · bench_report §반복 축)보다 커서 "같은 범위 재현" 이라 부르지 않는다. 동시성 4 는 이 셀의 밴드(6.53%) 안이다. acceptance length 는 비교 조건이 아니라 출력이다 — 이 셀 판정 레벨 2.238372093023256 은 이 스윕 lite warm 값의 승계이고(level_01 measured.json `spec_axis_source` = inherited(lite_warm)), 09-09 는 판정 레벨이 null 이며 같은 스윕 lite warm 실측이 2.1648 이다(testlog_26091412 §5.1). 두 값은 **같은 lite warm 레그**(같은 lite_bench · 3요청 · openai-chat)의 비교다. 그 비 1.034(소수 셋째 자리)를 처리량 비 1.054 옆에 둔다 — 처리량 차이의 일부(약 3.4%p)를 설명하지만 전부는 아니다(**부분 설명**). 나머지 후보(커널 · 이미지 digest · NCCL env 일부 · 측정 도구 판본)는 분해되지 않았다(Q4).

**판정 등급.** 인증서 `benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml` · verdict PASS · 권위 **explore**(문턱 없는 지도 — 게이트가 아니라 서술) · primary = expected_achievable(roofline×MBU) 34.77 · floor 29.55 · ratio 0.926 · 판정 입력의 accept_len 은 실측 승계(손 승계 입력 없음). 외부 레퍼런스 없이 roofline 만으로 섰다(경고 `E-not-attempted` · Q5). 위 사실 블록대로 **현행 full 정의를 충족한다**(반복 3 ≥ 3 · GuideLLM). waiver 아님 · 관측 게재 아님. 재측정이 필요한 조건: 소프트 지문(driver · cuda · image digest · max-len · kv-bytes · gmu · moe) 중 하나라도 네 환경에서 다르면 stale 이다 — 특히 image digest 와 NCCL 전송(`Using network` 줄)을 먼저 대조한다.
