# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 |
|---|---|
| `generated_utc` | 2026-09-23T08:10:39Z |
| `lite.cold_ttft_ms` | 1620.2 |
| `lite.gen_src` | median_tpot |
| `lite.gen_tps` | 20.36 |
| `lite.kv_gib` | 미기재 |
| `verdict_point_level` | 1 |
| `source` | sweep_index(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native · output) |

**`levels`**(5행)

| `level` | `status` | `accept_len` | `completed` | `decode_tps` | `decode_tps_mean` | `failed` | `itl_ms_median` | `max_concurrency` | `measurement_ok` | `output_throughput` | `tpot_ms_median` | `ttft_ms_median` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 1 | ok | 2.0806451612903225 | 16 | 22.27 | 22.03 | 0 | 40.971988790175494 | 1 | true | 22.15864026366414 | 44.89513300359249 | 1051.3358116149902 |
| 2 | ok | 2.0806451612903225 | 16 | 17.54 | 17.99 | 0 | 51.49115113651051 | 2 | true | 36.32722010003692 | 57.00383149087429 | 1179.8186302185059 |
| 4 | ok | 2.0806451612903225 | 16 | 13.0 | 12.91 | 0 | 72.35359958573883 | 4 | true | 44.65195646402134 | 76.91774610430002 | 1275.5355834960938 |
| 8 | ok | 2.0806451612903225 | 16 | 9.66 | 10.07 | 0 | 94.68322828704235 | 8 | true | 68.15399702513076 | 103.48732210695744 | 2102.550983428955 |
| 16 | ok | 2.0806451612903225 | 16 | 6.03 | 6.69 | 0 | 126.82093265009861 | 16 | true | 73.99731000931848 | 165.88659211993217 | 6377.687454223633 |
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## 부하 스윕 곡선 — 동시성별 (결정론 파싱 · 손저작 ✗)

> 단일 running serve 에 **동시 요청 수만** 바꿔 잰 곡선이다(reload 없음 · request-rate=inf).
> `동시성=1` 행이 인증서의 판정점이며, 나머지 행은 그 레시피가 **부하에서 어떻게 되는지**를 말한다.
> 출처: `bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report) · 소수 2자리 표시 반올림 · 결측은 `N/A`(0 이 아니다).
> 이 표는 스크립트가 파싱해 렌더한다 — 손으로 옮긴 수치가 아니며 `--verify` 가 diff 0 을 요구한다.

| 동시성 | decode t/s | 출력 tok/s | 총 tok/s | TTFT p50(ms) | ITL p50(ms) | 완료/실패 |
|---|---|---|---|---|---|---|
| 1 ★판정점 | 22.27 | 22.16 | 115.29 | 1051.34 | 40.97 | 16/0 |
| 2 | 17.54 | 36.33 | 189.02 | 1179.82 | 51.49 | 16/0 |
| 4 | 13 | 44.65 | 232.33 | 1275.54 | 72.35 | 16/0 |
| 8 | 9.66 | 68.15 | 354.61 | 2102.55 | 94.68 | 16/0 |
| 16 | 6.03 | 74.00 | 385.02 | 6377.69 | 126.82 | 16/0 |

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
| `source` | bench_report(bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
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
| `sweep_bench.sh` | sweep | `.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` | `a21e66eb26e5` 2026-09-23T06:53:32Z | a21e66eb26e5..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `sweep_bench.sh@a21e66eb26e5` | `git show a21e66eb26e551a8803750cdad06ffc03bbd352d:.claude/skills/adversarial-benchmark/scripts/sweep_bench.sh` |
| `run_bench.sh` | bench | `.claude/skills/adversarial-benchmark/scripts/run_bench.sh` | `434fa6740831` 2026-09-22T16:32:50Z | a21e66eb26e5..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `run_bench.sh@434fa6740831` | `git show 434fa6740831c1a3ef91131bd8f13abf95ebed2a:.claude/skills/adversarial-benchmark/scripts/run_bench.sh` |
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `fcb0754fffe1` 2026-09-23T01:56:11Z | a21e66eb26e5..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@fcb0754fffe1` | `git show fcb0754fffe107962034fec748ed8823756c93b6:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |
| `broad_search.sh` | driver-candidate | `.claude/skills/adversarial-benchmark/scripts/broad_search.sh` | `692297be56a3` 2026-09-14T13:23:51Z | a21e66eb26e5..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `broad_search.sh@692297be56a3` | `git show 692297be56a38ed79ff6521e31c6586bd153437c:.claude/skills/adversarial-benchmark/scripts/broad_search.sh` |

- `sweep_bench.sh` — 스윕 색인 조립자(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native/sweep_index.json · generated_utc = measured_utc 조인)
- `run_bench.sh` — 그 판본 sweep_bench.sh 가 레벨마다 부른다(`"$SDIR/run_bench.sh"`) · 색인에 레벨 기록
- `lite_bench.sh` — 그 판본 sweep_bench.sh 가 lite 레그로 부른다(`"$SDIR/lite_bench.sh"`) · 색인에 lite 기록
- `broad_search.sh` — 그 판본에서 sweep_bench.sh 를 부르는 스크립트 · 넘기는 --tool 상수 ['guidellm'] 가 이 측정과 어긋나지 않는다 — 호출 기록은 없다(드라이버였는지 미검증)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 그 판본의 .claude/skills/adversarial-benchmark/scripts/ 안에서 sweep_bench.sh 를 부르는 드라이버: broad_search.sh 후보(미검증)
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**실행 원문 명령 줄은 계보에 없다** — 이 측정의 스윕 호출 줄(인자 전체)은 어느 계보 문서에도 글자 그대로 남지 않았다(스윕 로그 `docs/simlog/26092313_native_N1_live/run2_n1-2609230653/benchmark.log` 첫 줄이 `config=nv4-bf-262k-mmp-native topo=multi levels=[1 2 4 8 16] in=1024 out=256 n=16 warmup=2 tool=guidellm bench_budget=8192MiB` 를 적었다 — 계보 목록 밖). 01 §1.4 의 bench 단계 재구성은 lite 두 줄뿐이다. 아래는 측정 시각 이전 마지막 커밋의 도구 원문이다.

native 평면에서는 스윕이 호스트 엔드포인트와 전용 클라이언트를 요구한다:

> [원문] sweep_bench.sh@a21e66eb26e5 §L55-55
> TOPO=""; SERVE_PLANE="docker"; HOST_ENDPOINT=""; CLIENT_VLLM=""; LEVELS="1,2,4,8,16"; ILEN=1024; OLEN=256; NPROMPTS=16; WARMUPS=2; VLLM_VER=""; DRYRUN=0; REASSEMBLE=0

**레벨 레그(동시성 1 · 2 · 4 · 8 · 16 · 레벨마다 반복 3)** — GuideLLM 호출(Docker 셀과 같은 판본):

> [원문] run_bench.sh@434fa6740831 §L381-386
>     --entrypoint guidellm "$IMAGE" run \
>     --backend "kind=openai_http,target=$BASE_URL,model=$MODEL_NAME,request_format=$ENDPOINT,extras={\"ignore_eos\":true}" \
>     --profile "kind=concurrent,streams=$CONC,warmup=$WARM_FRAC" \
>     --data "kind=synthetic_text,prompt_tokens=$ILEN,output_tokens=$OLEN" \
>     --tokenizer "kind=huggingface_auto,model=/tok" \
>     --constraint "kind=max_requests,count=$TOTAL_REQ" \

**lite 레그(cold 1요청 · warm 3요청 · 1회)** — native 분기는 컨테이너 안이 아니라 호스트의 클라이언트(`$CLIENT_VLLM` = 정문이 만든 래퍼 `bin/vllm-client`)로 부른다:

> [원문] lite_bench.sh@fcb0754fffe1 §L144-148
>     "$CLIENT_VLLM" bench serve --backend "$BACKEND" --base-url "$BASE_URL" --endpoint "$LITE_ENDPOINT" \
>       --model "$MODEL_NAME" --tokenizer "$MODEL_PATH" --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts "$2" --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups "$3" \
>       --save-result --result-dir "$(dirname "$1")" --result-filename "$(basename "$1")"

**조건 목록**
- 도구 · 버전: 레벨 레그 = GuideLLM 0.7.3(이미지 `ghcr.io/vllm-project/guidellm:v0.7.3` · 3.3 표) · lite 레그 = 호스트 venv 의 `vllm bench serve`(래퍼 경유 · 도구 버전 자기보고 없음).
- 엔드포인트: 레벨 레그 `/v1/chat/completions` · lite 레그 openai-chat(`/v1/chat/completions` — lite_bench L63-65 의 매핑 · 01 §1.4 재구성 명령의 `--backend openai-chat`).
- 입출력: 레벨 레그 합성 텍스트 1024 / 256(`ignore_eos` true) · lite random 512 / 128(`--random-range-ratio 0` · `--ignore-eos`).
- 요청 수 · warmup: 레벨당 16 + warmup 2(GuideLLM 에는 비율로 전달) · lite cold 1 · warm 3.
- 샘플링: GuideLLM 호출에는 temperature 인자가 없다(원문 없음 — 도구 기본값) · seed 원문 없음.
- 반복: 3(`warm-rerun` · 같은 running serve) · 완주 3 · 레벨 run 시도 합 15 — 3.3 표와 같다. 동시성 1 의 세 반복은 22.27 / 26.32 / 22.28 t/s(밴드 17.14%)이고 표 · 판정점은 첫 완주 run 의 값이다(원천 리포트 반복 축 표).
- spec decode: 켜짐 · MTP k=3 · 판정에 쓴 수용길이 2.08(판정 레벨 measured.json 실측 승계 · judge 로그 `source=measured`).
- 벤치 예산: 스윕은 `bench_budget=8192MiB` 를 받았지만 native 평면에서는 run_bench 가 Docker 서빙 예산 대조를 건너뛴다(스윕 로그 `native: Docker serve-budget inspection skipped` — 계보 목록 밖).
- 측정 노드: cluster(메인 호스트 엔드포인트 `127.0.0.1:8080` 에 붙은 클라이언트 · TP=2 쌍).
- 측정 시각: 07:40:31Z → 08:10:39Z(testlog_26092317 §2 · `generated_utc` 08:10:39Z). lite cold 1620.2ms 는 서빙 직후 스모크 추론 1회 뒤의 첫 lite 요청이다.

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:63` 원문 그대로):

> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 3 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | guidellm | bench_report(bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| 현행 full 정의 충족 | 예 | 판정점 반복 3 ≥ 3 · 도구 guidellm |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 측정에는 인증서가 없으므로 이 측정 쪽 지문은 스윕 meta · 원천 리포트의 측정 환경 표에서 읽는다.

- **같은 레시피 Docker 셀 D1**(`nv4-bf-262k-mmp` · 2026-09-23 · 같은 이미지 태그 · 같은 서빙 노브 · GuideLLM full 5레벨×3 · 판정점 20.98 t/s · floor 25.33 · REFUTE) · 강한 일치 키 6개(model · gpu_model · vllm_version 0.29.0 · quantization N/A · topology multi · TP 2) 동일 · 소프트 지문 다른 키 = `image_digest`(D1 `a2c4ca49…` 대 이 측정 meta `NA` — 설치 원천은 같은 태그의 노드별 로컬 이미지) · `moe_backend`(D1 `flashinfer_cutlass`(엔진 로그 실측) 대 이 측정 meta `NA` — 스윕이 native 엔진 로그를 수집하지 못했을 뿐, 정문 보존 로그는 NvFp4 `FLASHINFER_CUTLASS` · Fp8 `TRITON` 으로 D1 과 같은 선택을 기록했다 · 01 §1.4(f)) · 실행 평면(Docker 대 native · 인증서 지문 키는 아니다) · 지문 밖 런타임 관측 차이 둘(엔진 로그 — 이 측정 쪽은 정문 보존 로그 · 계보 목록 밖): 가중치 파일시스템 CIFS(컨테이너 안) 대 AUTOFS(호스트) · NCCL 넷 플러그인 `spcx` echo(D1) 대 `NET/Plugin: Could not find: libnccl-net.so`(N1 — 둘 다 네트워크 `Socket`) — driver_version(양쪽 580.173.02 manifest 선언) · cuda_version · image_tag · max_model_len · max_num_seqs · kv bytes · kv dtype · gmu · enforce_eager · ple_mode · bench_tool(guidellm 0.7.3) 은 같다 · 부하 조건: 도구 · 엔드포인트(chat) · 입출력 1024/256 · 레벨당 16 + warmup 2 · 동시성 5레벨 · 반복 3 · spec on(MTP k=3) 모두 같다 · **비교 가능(형상 비교 · stale — 다른 키 image_digest · moe_backend 는 이 측정 쪽 미관측)**.
- **같은 셀 run 1**(native · 같은 설치 · 같은 노브 · 판정점 21.16 · floor 24.62 · REFUTE) · 강한 키 · 소프트 지문 · 부하 조건 동일 · **비교 가능** — 다만 run 1 은 down attestation 이 FAIL_CLOSED 인 실패 증거이며(W13) 첫 스윕이 클라이언트 결함으로 중단된 뒤 재스윕한 값이다(W12).
- 2026-09-12 인증 측정(`benchmark_26091304` · Docker · 38.81 t/s) · 강한 키 동일 · 부하 조건: 측정 도구 vllm-bench-serve 대 GuideLLM · 엔드포인트 /v1/completions 대 chat · **비교 불가**(도구 · 엔드포인트가 다르다 — testlog_26091304 §1).
- 외부 레퍼런스: 이 형상(2노드 · PLE mmap · MTP on)의 like-with-like 외부 수치는 계보에 없다 — NVIDIA 공식 2 Spark 53.7 median 은 PLE resident · KV fp8 구성이라 **비교 불가**(testlog_26091304 §2).

**같은 레시피의 다른 측정 — 레벨 전부**(값은 원천 글자 그대로 · 비 = 이 측정 ÷ 대상 · 소수 3자리 반올림).

| 동시성 | 이 측정(run 2) | D1 Docker | 비 |
|---|---|---|---|
| 1 | 22.27 | 20.98 | 1.061 |
| 2 | 17.54 | 17.87 | 0.982 |
| 4 | 13.0 | 11.71 | 1.110 |
| 8 | 9.66 | 9.96 | 0.970 |
| 16 | 6.03 | 6.06 | 0.995 |

| 동시성 | 이 측정(run 2) | run 1(native) | 비 |
|---|---|---|---|
| 1 | 22.27 | 21.16 | 1.052 |
| 2 | 17.54 | 17.65 | 0.994 |
| 4 | 13.0 | 12.64 | 1.028 |
| 8 | 9.66 | 9.26 | 1.043 |
| 16 | 6.03 | 5.64 | 1.069 |

(run 1 · D1 레벨 값은 각 스윕 색인 — `docs/simlog/26092313_native_N1_live/run1_n1-2609230450/n1_sweep/sweep_index.json` · `docs/simlog/26092300_hint_publisher_G1G3_E2E/d1_sweep/sweep_index.json` · 계보 목록 밖.)

- 수용길이(spec 은 셋 다 켜짐 · MTP k=3): 이 측정 2.08(2.0806…) · D1 1.891 · run 1 1.837 — 비(이 측정 ÷ 대상) 1.100 · 1.132. 동시성 1 decode 비는 1.061 · 1.052 로 수용길이 비보다 작다 — 처리량 차이가 수용길이 비로 설명되는지는 **판정 불가**다: 이 측정의 판정점 반복 밴드가 17.14% 로 두 비의 차이보다 넓다(D1 3.12% · run 1 7.94%).
- 자체 재현 밴드: 이 측정 동시성 1 의 밴드 17.14%(n=3). D1 과의 차이(판정점 +6.1%)는 이 밴드 안이다 — "Docker 와 native 가 같은 대역" 이라는 testlog 의 관측은 이 범위에서만 성립한다. 38.81 과의 차이는 밴드를 크게 넘지만 비교 불가 조건이다(Q1).
- 판정 산정 입력: primary = expected_achievable 32.79(R_fp 45.03 × 수용길이 2.08 × 0.35) · tolerance 0.15 → floor 27.87 · ratio 0.679. 수용길이는 자동 승계(손 입력 없음). **floor 가 run 마다 다른 것(24.62 · 25.33 · 27.87)은 루브릭이 그 run 의 측정 수용길이를 곱하기 때문**이다 — 셀 형상의 차이가 아니다(testlog_26092317 §3). 외부 레퍼런스 E 는 검색되지 않았다(warning E-not-attempted).

**판정 등급.** 판정 권위 = explore(`verdict_rule.py` · roofline×MBU 가 primary · E 미시도로 roofline-only 강등). 결과 **REFUTE** — 유효 측정(15회 레벨 측정 완주 · 실패 0 · 자동 down attestation PASS)이 탐색 합격선 아래라는 뜻이며 기능 실패가 아니다. 인증서는 full PASS 에서만 나오므로 없다. waiver 없음. 현행 full 정의 충족: **예**(위 사실 블록 — 반복 3 ≥ 3 · 도구 guidellm). 이 수치는 **관측 게재(OBSERVATION-ONLY)** 이지 baseline 이 아니다 — 22.27 을 기준선으로 쓰지 말고, 과거 38.81 이 이 빌드에서 재현된다고 가정하지도 말라. 재측정 조건: E 검색을 포함한 적대 재판정 · 반복 밴드가 좁아지는지(반복 수를 늘려) 확인 · 도구 · 엔드포인트를 바꿔 잰 대조(02 Q1~Q3).
