# 03 · 측정 — 결정론 표 · 측정 구성 · like-with-like

> 이 파일의 표는 인증서 · 리포트 · 스윕 산출물에서 **파싱만** 한 것이다. 합성하지 않았고, 재계산하지 않았다. 결측은 `미기재` · `N/A` 로 남는다(0 이 아니다).

## 3.1 측정 결과
<!-- FACT:measurement -->
| 키 | 값 |
|---|---|
| `source` | absent(인증서·스윕 모두 없다) |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:measurement -->

## 3.2 부하 곡선
> 아래 절은 `render_bench_section.py` 가 바인딩된 리포트(또는 스윕 색인)에서 렌더했다 — 발행 린터가 같은 원천으로 다시 렌더해 diff 0 을 요구한다. 이 절은 다음 챕터 헤딩 직전까지이며 닫힘 표지가 없다(`render_bench_section.py --verify --section 03-benchmark.md` 가 절 제목부터 다음 `## ` 직전까지 잘라 대조한다).

<!-- BENCH_SECTION -->

## lite 지표 — 서빙 성공 직후 스냅샷 (결정론 파싱 · 손저작 ✗)

> 서빙 성공 직후의 **lite 스냅샷**이다 — cold 1회 + warm burst 1회. 동시성 곡선·반복·판정·인증서가 없다.
> 성능 baseline 이 아니며(OBSERVATION-ONLY) 수신자는 자기 환경에서 재측정한다.
> 출처: `bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` (bench_report(lite)) · 표는 리포트의 행을 그대로 옮긴다(재산정 ✗ · `--verify` diff 0).

| 메트릭 | Main | Sub |
|---|---|---|
| gen tokens/sec (warm) [master] | 18.77 t/s | — |
| cold-start TTFT [master] | 679 ms | — |
| GPU VRAM 점유 | ~59.0 GiB (serve-log 분해; nvidia-smi N/A) | N/A (통합메모리 nvidia-smi 미보고 · serve-log 분해 없음) |
| KV cache 점유 | 20.0 GiB (클러스터) | ≈ ÷TP (클러스터 공유) |
| 시스템 RAM 점유 | 79.5 GiB (65%) | 75.9 GiB (62%) |

## 3.3 측정 구성
<!-- FACT:measurement_config -->
| 키 | 값 |
|---|---|
| `bench_mode` | lite |
| `bench_mode_kind` | declared-lite |
| `downgrade_reason` | 미기재 |
| `bench_tool` | vllm-bench-serve |
| `bench_tool_version` | 미기재 |
| `repeats` | 1 |
| `repeats_completed` | 1 |
| `source` | bench_report(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
<!-- /FACT:measurement_config -->

<!-- FACT:bench_missing -->
> **인증서가 없다.** 인증서는 full 모드 verdict==PASS 일 때만 나오며 그 발행은 `adversarial-benchmark` 의 책임이다.
> 부재는 '성능이 나빴다' 가 아니라 '**그 형태로 판정되지 않았다**' 는 뜻이다 — 위 수치는 관측이지 판정이 아니다.

**측정 결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_CERTIFICATE` | 인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. 인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다. |
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:bench_missing -->

## 3.4 측정 명령 원문
<!-- FACT:tool_snapshots -->
**측정 도구 원문 1건**(측정 시각 이전 마지막 커밋의 바이트 · 발췌 머리 = `> [원문] <이름>@<rev12> §L<a>-<b>` · 다음 변경 = 측정 뒤 이 파일을 처음 바꾼 커밋 — 두 측정 사이에 도구가 바뀌었는지는 이 칸으로 가른다)

| 도구 | 역할 | 저장소 경로 | 리비전(커밋 UTC) | 다음 변경 | 발췌 출처 토큰 | 다시 얻기 |
|---|---|---|---|---|---|---|
| `lite_bench.sh` | lite | `.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` | `fcb0754fffe1` 2026-09-23T01:56:11Z | fcb0754fffe1..HEAD 경로 이력에 변경 없음(지금 HEAD 까지 같은 바이트) | `lite_bench.sh@fcb0754fffe1` | `git show fcb0754fffe107962034fec748ed8823756c93b6:.claude/skills/adversarial-benchmark/scripts/lite_bench.sh` |

- `lite_bench.sh` — 경량 리포트 셀(lite_bench.sh 가 리포트를 발행한다)
- `다시 얻기` = 그 커밋을 가진 클론에서의 명령이다(발행 원격 도달은 이 표가 판정하지 않았다).
- 측정 시각의 워킹트리(미커밋 편집)는 관측 대상 밖 — 이 바이트는 그때의 **커밋된** 판본이다
<!-- /FACT:tool_snapshots -->

> 이 절이 답하는 질문: 이 수치는 정확히 어떤 명령 · 도구 · 버전 · 입력 조건으로 쟀는가?

**실행 원문 명령 줄.** lite 레그의 호출 줄(`lite_bench.sh` 인자)은 이 셀의 판정 testlog(`docs/testlog/testlog_26092311_hint_publisher_G3_D2_lite_판정.md` — 이 페이로드 계보 목록 밖)에 `lite_bench.sh nv4-f8-262k-mmp --topology multi --backend openai-chat --publish-report` 로 적혀 있다. 01 §1.4 의 bench 단계는 명령 미관측이다. 아래는 측정 시각 이전 마지막 커밋의 도구 원문(`lite_bench.sh@fcb0754fffe1`)이다.

backend 에서 엔드포인트가 정해진다:

> [원문] lite_bench.sh@fcb0754fffe1 §L63-65
> case "$BACKEND" in
>   openai-chat) LITE_ENDPOINT=/v1/chat/completions ;;
>   openai)      LITE_ENDPOINT=/v1/completions ;;

cold · warm 두 레그가 같은 `vllm bench serve` 를 인자만 바꿔 부른다(cold = 1요청 · warmup 0, warm = N=3 · warmup 1):

> [원문] lite_bench.sh@fcb0754fffe1 §L136-141
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url $BASE_URL --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \
>       --save-result --result-dir /tmp --result-filename 'lite_tmp.json'" \

이 발행은 lite 를 **두 번** 돌렸다. 리포트 · 3.2 표의 수치는 실행 ② 의 것이다(warm gen t/s 는 warm 레그의 median TPOT 에서 산정 — `gen_src: median_tpot`). 실행 ② warm 레그:

> [원문] docs/simlog/26092310_hint_publisher_G3_D2_lite/d2_lite_2.log §L113-121
> Median TPOT (ms):                        53.27     
> P99 TPOT (ms):                           59.88     
> ---------------Inter-token Latency----------------
> Mean ITL (ms):                           97.15     
> Median ITL (ms):                         97.16     
> P99 ITL (ms):                            120.27    
> ---------------Speculative Decoding---------------
> Acceptance rate (%):                     27.01     
> Acceptance length:                       1.81      

실행 ① warm 레그(서브 probe 가 실패한 교정 전 실행 · 같은 serve · 실행 ② 직전):

> [원문] docs/simlog/26092310_hint_publisher_G3_D2_lite/d2_lite.log §L113-121
> Median TPOT (ms):                        53.20     
> P99 TPOT (ms):                           55.38     
> ---------------Inter-token Latency----------------
> Mean ITL (ms):                           90.74     
> Median ITL (ms):                         92.00     
> P99 ITL (ms):                            104.07    
> ---------------Speculative Decoding---------------
> Acceptance rate (%):                     24.74     
> Acceptance length:                       1.74      

**조건 목록**
- 도구 · 버전: `vllm bench serve`(서빙 이미지 안 · 도구가 버전을 자기보고하지 않는다 — 3.3 `bench_tool_version` 미기재) · 드라이버 스크립트 `lite_bench.sh@fcb0754fffe1`. 실행 ① 은 교정 전 판본으로 돌았고 두 판본의 차이는 서브 probe 파서뿐이다(02 W10).
- backend · 엔드포인트: `openai-chat` → `/v1/chat/completions`(lite_raw JSON · 실행 ② Namespace).
- 입출력: random 512 / 128 토큰 · `--random-range-ratio 0` · `--ignore-eos`. 실행 ② warm 의 총 입력 1,695 토큰(3요청)은 도구 입력 512 에 채팅 템플릿이 더해진 값이다(추론 — 템플릿 토큰을 따로 잰 기록 없음).
- 요청 수 · warmup: cold 1요청 · warmup 0 / warm 3요청 · warmup 1 · 동시성 1 · request-rate inf. 부하 레벨은 1개뿐이다(`HINT_MISSING_SWEEP_LEVELS`).
- 샘플링: temperature 인자 없음(Namespace `temperature=None` — 원문에 값 없음) · `seed=0`(실행 ② Namespace · `d2_lite_2.log` L10).
- 반복: 1(각 실행 cold 1 + warm 1) — 3.3 표와 같다. 두 실행은 반복이 아니라 결함 교정 전후의 재실행이다.
- cold 대 warm: 실행 ① cold TTFT 1746.76ms(서빙 직후 첫 요청에 가깝다) · 실행 ② cold TTFT 679.05ms(실행 ① 직후 같은 serve — **cold 가 아니다**) · warm gen 은 실행 ① 18.80 · 실행 ② 18.77 t/s(`d2_lite.log` · `d2_lite_2.log` 표).
- spec decode: 켜짐 · MTP k=3. 수용률 · 수용길이는 레그마다 다르다 — 실행 ① cold 32.81% / 1.98 · warm 24.74% / 1.74 · 실행 ② cold 33.33% / 2.00 · warm 27.01% / 1.81. 리포트 수치(실행 ②)와 짝이 되는 warm 값은 27.01% / 1.81 이다.
- 측정 노드: cluster(메인 API 서버에 붙은 클라이언트 · TP=2 쌍).
- 측정 시각: 실행 ② `measured_utc` 2026-09-23T01:56:16Z(lite_raw JSON) · 실행 ① 은 그 직전(판정 testlog 가 01:54:20Z 로 적었다) · 서빙 창 01:23:48Z ~ 01:57:43Z(01 §1.4 원장).

## 3.5 like-with-like
<!-- FACT:bench_definition -->
**현행 full 정의**(`.claude/rules/docs.md:63` 원문 그대로):

> full 계측(`lite ∪ GuideLLM × 반복 ≥3`)

| 항목 | 값 | 출처 · 사유 |
|---|---|---|
| 이 측정의 반복 수 | 1 | 측정 구성 표 repeats_completed(완주 · bench_report(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md)) |
| 현행 정의의 반복 요건 | 3 | 위 정의 문장 |
| 이 측정의 도구 | vllm-bench-serve | bench_report(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| 현행 full 정의 충족 | 아니오 | 측정 구성 bench_mode=lite — full 측정이 아니다 · 판정점 반복 1 < 현행 정의 3 · 측정 도구 vllm-bench-serve — 현행 정의의 full 레그(GuideLLM)가 아니다 |
<!-- /FACT:bench_definition -->

> 이 절이 답하는 질문: 이 수치를 다른 수치(이전 측정 · 자매 셀 · 외부 레퍼런스 · 다른 vLLM 버전의 hint)와 나란히 놓아도 되며, 이 측정은 어떤 판정 등급인가?

이 측정은 인증서 · 스윕이 없는 lite 스냅샷 1회이므로 비교의 이 측정 쪽 지문은 경량 리포트의 측정 환경 표와 lite raw JSON 에서 읽는다.

- 같은 셀 2026-09-11 r2 인증 측정(`benchmark_26091111` · full `vllm bench serve` 5레벨 · 판정점 38.18 t/s) · 강한 일치 키 6개(model · gpu_model · vllm_version 0.29.0 · quantization N/A · topology multi · TP 2) 동일 · 소프트 지문: 인증서 쪽은 image_digest 85cef278… 인데 이 측정의 사실 블록은 digest 를 싣지 않았다(검증자가 메인에서 `docker image inspect` 로 본 이 태그의 digest 는 a2c4ca49… — D1 측정 digest 와 같다 · 서브는 미확인) · driver_version 은 인증서 580.173.02(manifest 선언) 대 이 실행 attestation 관측 580.178.04 · moe_backend 는 인증서가 엔진 로그에서 `triton` 으로 적었고 이 실행 엔진은 주 experts `FLASHINFER_CUTLASS` NvFp4 · MTP `TRITON` Fp8 을 골랐다(01 §1.4(f) — 이 측정은 그 키를 기록하지 않았다) · max_model_len · kv bytes · kv dtype fp8_e4m3 · gmu · enforce_eager · ple_mode mmap 은 트리플렛이 같다 · 부하 조건: 레벨 레그는 1024/256 · 16요청 · `/v1/completions`(testlog_26091114 측정 사양) 대 이 lite 512/128 · 3요청 · chat — **비교 불가**(도구 레그 · 입출력 · 엔드포인트가 다르다).
- 같은 셀 2026-09-11 r2 의 lite 레그 · warm 28.06 t/s · cold TTFT 306ms(`docs/simlog/26091114_qwen38fn_r2_nv4_f8_262k_mmp/sweep_index.json` lite) · 도구는 같은 `lite_bench.sh` 의 vllm bench serve 다. 그 색인에는 backend · 엔드포인트 칸이 없지만, r2 판정 testlog 가 적은 명령은 `sweep_bench.sh <config> --topology multi --backend openai` 이고(testlog_26091114 명령) 그 시점 판본의 sweep_bench 는 `--backend` 를 lite_bench 에 넘기며 lite_bench 는 `openai` 를 `/v1/completions` 로 매핑한다(검증자가 r2 기동 2026-09-11T01:42Z 이전 마지막 커밋 — sweep_bench `8ca23d496f24` L170 · lite_bench `0df14da468f9` L31-32 — 로 대조) — 그 lite 레그는 `/v1/completions`(채팅 템플릿 없음) 대 이 측정 `/v1/chat/completions` 이다 · **비교 불가** — 엔드포인트(요청당 입력 구성)가 다르다. 참고로 두 값의 비(이 측정 ÷ 이전)는 0.669 이며 원인은 설명되지 않았다(02 Q1).
- 같은 셀 2026-09-10 첫 캠페인 lite · warm 26.73 t/s · cold TTFT 1481ms(testlog_26091009 셀 4) · 같은 이유로 **비교 불가**(그 판본의 부하 조건 미기록 · 당시 이미지 digest 85cef278…).
- 자매 셀 D1 `nv4-bf-262k-mmp` 의 lite 레그(같은 날 · 같은 이미지 · KV auto) · warm 20.16 t/s(`docs/benchmark/bench_report_26092309_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md` lite 표 — 이 페이로드 계보 목록 밖) · 부하 조건: 같은 lite_bench 레그 · openai-chat · warm 3요청 · 총 입력 1,695 토큰으로 같고, 드라이버 판본은 434fa6740831 대 fcb0754fffe1(차이는 서브 probe 파서뿐) · 이 측정의 경량 리포트는 소프트 지문 블록을 싣지 않으므로 "다른 것은 KV dtype 하나" 는 지문 대조가 아니라 두 트리플렛 diff(yaml 은 `kv-cache-dtype` 한 줄 · env 는 셀 이름 줄만 다르다 — 검증자 대조)와 같은 이미지 태그 · 원장 identity 에서 나온다 · 실행 맥락도 다르다 — D1 lite 는 서빙 직후 full 스윕의 첫 레그(GuideLLM 레벨보다 앞)였고, 이 측정은 같은 serve 에서 두 번째로 돈 lite(실행 ②)다 · **비교 가능(형상 비교 · stale — 선언 설정 차이는 kv_cache_dtype 하나) · 반복 1** — 비 18.77 ÷ 20.16 = 0.931, 수용길이 비 1.81 ÷ 1.891 = 0.957(D1 lite warm 1.891 · 소수 3자리). 처리량 차이는 수용길이 비만으로는 설명되지 않는다고도, 된다고도 가를 수 없다 — **판정 불가**: 양쪽 반복 1회라 재현 밴드가 없고, 약 7% 차이가 잡음 안인지 모른다. D1 의 GuideLLM full 수치(판정점 20.98 t/s)와는 도구 · 부하 · 반복이 달라 나란히 놓지 않는다.
- 외부 레퍼런스(blazux · 싱글노드): fp8 KV 가 디코드 −10% · KV 풀 ×1.9(plan_26090918 §2.3) · **비교 불가** — 노드 수(1 대 2) · DRAFT_VOCAB 등 구성이 다르고 측정 조건이 기록되지 않았다.

**같은 셀의 이전 측정 — 레벨 전부.** 이 측정에는 부하 레벨이 없다(lite 1레벨). 2026-09-11 인증 측정의 레벨을 참고로 싣되 비는 계산하지 않는다 — lite warm(512/128 · N=3)은 어느 레벨과도 같은 부하가 아니다.

| 동시성 | 이 측정 | 이전 측정(2026-09-11 · 38.18 인증) | 비 |
|---|---|---|---|
| 1 | 레벨 측정 없음(lite warm 18.77) | 38.18 | 비교 불가 |
| 2 | 없음 | 32.97 | — |
| 4 | 없음 | 24.78 | — |
| 8 | 없음 | 16.98 | — |
| 16 | 없음 | 11.66 | — |

- 수용길이(spec 은 모두 켜짐 · MTP k=3): 이 측정 warm 1.81(실행 ②) · 2026-09-11 인증 측정 2.4643(레벨 1 · 인증서 `accept_len`) — 측정 레그가 달라 비를 내지 않는다.
- 판정 산정 입력: 이 셀은 lite-only 라 verdict 가 없다 — 손으로 승계된 판정 입력도 없다.

**판정 등급.** 판정 권위 = 없음(관측 게재 · `hint_map_only`). 인증서 · verdict · 루프라인 판정이 없고 waiver 도 없다. 현행 full 정의 충족: **아니오**(위 사실 블록 — 반복 1 < 3 · 도구가 GuideLLM full 레그가 아니다 · 선언된 lite-only 셀). 이 수치는 **OBSERVATION-ONLY** 이지 baseline 이 아니다 — 18.77 t/s 를 기준선으로 쓰지 말고, 2026-09-11 의 38.18 이 이 빌드에서 재현된다고 가정하지도 말라. 재측정 조건: 이 셀을 full(GuideLLM × 반복 3 · 5레벨)로 재고 E 검색을 포함한 적대 판정을 받는 것, 그리고 cold 값은 새 기동 직후 첫 실행에서만 얻는 것(02 Q2).
