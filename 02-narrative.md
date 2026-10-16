# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 9건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite.md` | plan | 26101410 | 0 | 10727 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 2 | `docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md` | testlog | 26101619 | 0 | 2895 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 3 | `docs/devlog/devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031.md` | devlog | 26101620 | 0 | 2077 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 4 | `docs/benchmark/bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md` | bench_report | 26101616 | 0 | 6510 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 | — |
| 5 | `docs/plan/plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행.md` | plan | 26101616 | 0 | 2294 | 0.31.0 · 2b · minicpm5 | — |
| 6 | `docs/devlog/devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | devlog | 26101619 | 1 | 4789 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 7 | `docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md` | bench_report | 26101616 | 1 | 6903 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 | docs/benchmark/bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md · docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md |
| 8 | `docs/plan/plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | plan | 26101614 | 1 | 16488 | 0.31.0 · 2b · minicpm5 | docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md |
| 9 | `docs/benchmark/bench_report_26101619_minicpm5-2b_GB10_0.31.0.md` (모호 해소) | bench_report | 26101619 | 1 | 3202 | 0.31.0 · 2b · minicpm5 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**(1) 목표.** 계보의 출발은 vLLM 0.31.0 bump 와 14B 이하 텍스트 모델 4종의 양 노드 상주 서빙이다. 메인 노드에는 Granite-4.2-8B 와 이 셀의 MiniCPM5-2B 가 함께 올라갔고, 각 모델은 max-model-len 32768 · OpenAI 호환 API · 도구호출 · 추론 파서를 켠 채 "DGX Spark 64GB 1대" 를 타깃으로 시뮬레이션했다. 그 plan 의 벤치는 lite 뿐이었고 hint 발행은 범위 밖이었다.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1
> 3. NAS 에서 최근 다운로드한 **14B 이하 텍스트 생성 모델 4종**을 노드당 2종씩 **상주 서빙**한다 —
>    max-model-len **32768 고정**, LangGraph RAG 용으로 **OpenAI 호환 API + 도구호출(auto tool choice) + 추론 파서** 활성.
> 4. 각 모델은 **DGX Spark 64GB(OEM 전용 구성) 1대**를 타깃 GPU 로 시뮬레이션한다 — 노드 128GB 를 64/64 로 나눈다.
> 5. 벤치는 **lite 만**(full ✗). 종료 시 4개 서빙은 **내리지 않는다**.

이 셀의 측정은 그 뒤 계보(plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리)에서 왔다 — GuideLLM full-bench 의 합성 텍스트 고정을 풀어 데이터셋을 프리셋 knob 로 넣고, CNN/DailyMail 사용자 프리셋으로 **이미 상주하던** 서빙을 다시 쟀다(목적: 한국어 특화 모델 평가를 위해 특정 언어로 고정된 입력의 벤치가 필요하다 — 같은 plan §1). 서빙 설정은 이 두 번째 계보에서 바뀌지 않았다.

**(2) HW 출발 조건.** NVIDIA GB10 · 노드당 GPU 1 · 이 셀은 메인 노드 1대(single · TP=1 · 분산 없음) · 통합메모리 · 인터커넥트 해당 없음. 노드 메모리 총량은 블랙박스 예산 선언 행의 `mem_total_mib=124608`(선언 행에 실린 값 · 01 §1.4)이고 셀 config 는 `test_device_total_gib` 121.7 을 적는다. 호스트 층은 plan_26101410 머리 유효맥락의 "aarch64 · CUDA 13.2 드라이버 580.173.02" 다(이미지 층 CUDA 13.3.1.008 과 다른 층). 사람 결정 U3 이 모델당 메모리를 64GB Spark 1대 천장(≈47.7GiB)으로 묶었다.

**(3) 모델 구조.** 체크포인트 `openbmb/MiniCPM5-2B`(revision `f97400052a43d642bbc6e9975e2397e3ae6a6b52` · 00 §0.2) · `LlamaForCausalLM` · 42층 · KV 헤드 2 · bf16 비양자화(체크포인트 config 에 quantization_config 부재 — 00 §0.2) · PLE 키 없음(00 §0.9 후보 표 출처) · spec decode 미사용(엔진 config `speculative_config=None`). 체크포인트 크기는 엔진 로그가 4.69 GiB 로 기록하고(lite_engine_head L55 `Checkpoint size: 4.69 GiB` · 측정 roofline.json active_bytes 5033512960 과 같은 크기) plan_26101410 §모델 4종의 ≈4.4GiB 는 config 판독 산식이다. L62 의 4.75 GiB 는 크기가 아니라 로드 뒤 메모리 증가분이다(파일시스템 du 는 쓰지 않았다). 토큰당 KV 42KiB · 32k 1시퀀스 1.3GiB(같은 표 · 산식). 네이티브 컨텍스트는 체크포인트 config.json 의 `max_position_embeddings` 131072 다(같은 파일 `head_dim` 128 · 서빙 max-model-len 32768 은 그보다 짧게 고정한 값). 총 파라미터 수는 config 에 직접 키가 없어 미기록이다.

**(4) stock 에서 예상 · 관측된 장벽.** 빌드는 stock(빌드 패치 0)으로 계획됐고, 0.30.0→0.31.0 버전 델타 판정이 A축 UNDETERMINED 라 시도-빌드 출구를 사람이 승인했다(plan_26101410 §4 D1). 서빙 쪽 장벽은 관측되지 않았다 — 이 셀의 벽은 두 번째 계보의 측정 하네스에서 나왔다(2.2).

**(5) 외부 선례와 사람의 결정.** 이 모델의 외부 서빙 선례는 계보에 없다. 사람 결정은 모델 선정(U1 · 계열당 1종 · 최신 다운로드순) · vLLM 0.31.0(U2) · 64GB 천장(U3) · 동시성 요구 8(D5)이고, 두 번째 계보에서는 입력만 실제 텍스트 · 출력은 ignore_eos 고정(U1) · `abisee/cnn_dailymail` 3.0.0 test 의 `article` 만 · input 1024 / output 256(U16)을 골랐다. 포크 · 자체이식 경로는 쓰지 않았다(stock).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T11:50:00Z(`camp-26101616-cnn-hint` · evidence.event_campaign_boundary): campaigns/camp-26101616-cnn-hint/campaign.yaml declared_utc 2026-10-16T11:50:00Z → 경계 = 앞선 값 2026-10-16T11:50:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T08:04:04Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

_이번 캠페인(경계 2026-10-16T11:50:00Z 이후)의 기동 시도 미관측 — 아래는 경계 이전 행뿐이다._

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-16T11:50:00Z 이전 — 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 1 |
| 앞선-2 | main | `serve-main-combined-granite-minicpm`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T02:04:02Z | 51회 · 첫 2026-10-14T02:04:03Z | 없음 | —(닫힘 행 없음) | 1 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

이 셀의 계보에서 넘은 벽은 둘이고, 둘 다 서빙이 아니라 **측정 하네스**의 결함이다. 서빙 자체는 2026-10-14 에 합산 예산 선언 창(`serve-main-combined-granite-minicpm` · 갱신 51회 · 사살 · 트립 없음)으로 섰고, 위 표가 이 셀 라벨의 기동 시도를 0 으로 세는 것은 그 서빙이 합산 라벨로 선언됐기 때문이다(01 §1.4 (c)). 측정(2026-10-16)은 그 상주 세션 위에서 세 번 돌았다 — 1차 스윕은 lite 게이트에서 멈췄고(W1), 2차 스윕은 완주했으나 입력이 반복돼 폐기됐고(W2), 3차 스윕이 이 페이로드의 측정이다.

### W1 lite 게이트 measurement_path_failed — 엔진 로그 꼬리만 파싱 (1차 스윕 · 06:44Z)

full-bench 는 lite 레그를 진입 게이트로 먼저 돈다. 1차 스윕의 lite 요청은 전부 성공했는데(warm 38.88 t/s) KV 지표만 N/A 로 남아 lite 판정이 `measurement_path_failed` 가 됐고 GuideLLM 에 들어가지 못했다. 원인은 lite_metrics 가 엔진 로그를 꼬리(tail -800)로만 파싱한 것이다 — KV 예약 줄은 기동 때 한 번만 찍히는데(lite_engine_head L74 · 10-14 02:05:10) 서빙이 2일 상주하는 동안 꼬리 창 밖으로 밀려났다. 해소는 커밋 0f8800c(머리 캡처를 먼저 읽고 비면 꼬리로 메움 · 값은 파싱 그대로)이고, 재판정은 pass(KV 35.0 GiB)였다. 통과는 최종 스윕 lite 레그의 `lite_verdict: pass (exit 0)` 와 KV 35.0 GiB 로 확인했다(lite_leg.log).

> [원문] devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정
> 1. **1차 스윕(06:44Z)** — lite 게이트가 `measurement_path_failed` 로 막아 GuideLLM 에 들어가지 못했다. 요청은 전부 성공했고(warm 38.88 t/s)
>    KV 만 N/A 였다. lite_metrics 가 엔진 로그 꼬리(tail -800)만 파싱해 상주 2일 서빙의 기동 KV 라인을 못 봤다 → 머리 우선 파싱(0f8800c) ·
>    재판정 pass(KV 35.0 GiB).

```hint-event
id: W1
kind: wall
증상: 1차 스윕 lite 레그 요청은 전부 성공했으나 KV 지표가 N/A 로 남아 lite 판정이 GuideLLM 진입을 막았다
서명: "lite 게이트가 `measurement_path_failed` 로 막아 GuideLLM 에 들어가지 못했다"
원인: lite_metrics 가 엔진 로그 꼬리(tail -800)만 파싱해 2일 상주 서빙에서 기동 때 한 번 찍힌 KV 예약 줄을 보지 못했다(하네스 결함 · 서빙은 정상)
해소: 커밋 0f8800c — KV · 가중치 파싱이 엔진 로그 머리 캡처를 먼저 읽고 비면 꼬리로 메운다 · 재판정 pass(KV 35.0 GiB)
검증: 최종 스윕 lite_leg.log lite_verdict pass (exit 0) · KV cache 점유 35.0 GiB
전이등급: arch-invariant
출처: [devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정, testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3, output/single/benchlog/sweep_minicpm5-2b-v031/lite_leg.log, output/single/benchlog/sweep_minicpm5-2b-v031/lite_engine_head_minicpm5-2b-v031.log]
```

### W2 행 선택이 목표 길이를 정확일치로 비교 — BOS 를 세는 토크나이저에서 11 행만 남아 프롬프트 반복 (2차 스윕 · 07:26Z)

2차 스윕은 full 로 완주했고(c=1 38.58 t/s) 판정도 PASS 였지만 스윕 meta 가 `dataset_duplicate_sampling: True` 를 적었다. 런 사이드카가 `rows_available 11` 을 말했다 — 이 모델의 토크나이저는 BOS 를 세어 concatenate 1024 전처리 행 대부분(6254 개)이 `prompt_tokens_count 1025` 였는데, 행 선택은 목표 길이를 `input_len` 과 정확일치(`== 1024`)로 비교해 6265 행 중 11 행만 남겼고, 12 런이 같은 11 프롬프트를 돌려 썼다. 원인은 사이드카 기록과 전처리 분포로 확정했다(testlog · devlog 의 판정). 해소는 목표 길이를 전처리 분포의 **최빈값**으로 고르는 것(dataset_prep `target_rows` · 자체검사 T1~T4)과 3차 스윕 재측정이며, 앞선 리포트에는 SUPERSEDED 헤더를 달았다. 통과는 최종 스윕 런 사이드카 12건 전부가 겹치지 않는 연속 행이라는 확인으로 판정했다(testlog §4).

> [원문] testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3
> | E2 | 2차 스윕: `dataset_duplicate_sampling: True` — 행 선택이 `prompt_tokens_count == 1024` 로 비교해 6265 행 중 11 행만 남았고 같은 11 프롬프트가 반복됐다 | 목표 길이 = 분포 최빈값으로 교정(dataset_prep `target_rows` · 자체검사 T1~T4) · **재측정**. 교정 전후 c=1: decode 38.58 → 38.13 t/s · TTFT p50 64.9 → 106.7 ms — 반복 프롬프트에서 prefix cache 가 TTFT 를 줄였을 가능성이 높다(원인 미조사 · 추론). 앞선 리포트에 SUPERSEDED 헤더 |

교정 뒤 최종 스윕의 런 사이드카(레벨 1 · 첫 런)는 목표 1025 · 풀 6254 행 · 중복 없음을 적는다:

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/level_01/dataset_minicpm5-2b-v031.json §L17-21
>   "rows_available": 6254,
>   "rows_prepared": 6265,
>   "selection": "contiguous(rows 0..17 · 다른 런과 겹치지 않음)",
>   "prompt_tokens_count_target": 1025,
>   "duplicate_sampling": false,

**교정 전후 TTFT 차이는 원인을 조사하지 않았다.** c=1 decode 는 38.58 → 38.13 t/s 로 거의 같은데 TTFT p50 은 64.9 → 106.7 ms 로 크게 달라졌고, 계보는 이를 "반복 프롬프트에서 prefix cache 가 TTFT 를 줄였을 가능성" 으로 **추론**했다. 이 저작에서 엔진 로그를 대조한 관측이 그 추론과 어긋나지 않는다는 것만 덧붙인다 — 엔진 설정은 `enable_prefix_caching=True` 다. 엔진이 찍는 prefix cache hit rate 는 구간 값이 아니다: 측정 이미지의 vLLM 소스(db9527a `vllm/v1/metrics/stats.py` `CachingMetrics(max_recent_requests=1000)` — 사실 검증이 확인한 계보 밖 출처)상 최근 약 1000 요청의 누적 비율이고, 이 serve 세션은 10-14 기동 뒤 3차 스윕 끝까지 1000 요청 미만을 받았으므로 사실상 **기동 이후 누적값**이다. 그 누적값은 CNN 측정 전(10-15 16:50:02) 이미 40.6% 였고, 2차 스윕 시작 무렵 43.0%(이월값)에서 2차 스윕 뒤 정점 76.6%(07:41:45)까지 올랐다가 3차 스윕 동안 51.5%(07:53:05)까지 내려갔다 — 2차 구간에 적중이 많았고 3차 구간에 적었다는 **방향**만 읽을 수 있고, 각 수치를 그 스윕 자신의 적중률로 읽을 수는 없다:

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/level_08/engine_minicpm5-2b-v031.log §L488-488
> (APIServer pid=184) INFO 10-16 07:41:45 [loggers.py:320] Engine 000: Avg prompt throughput: 2.4 tokens/s, Avg generation throughput: 38.8 tokens/s, Running: 1 reqs, Waiting: 0 reqs, GPU KV cache usage: 0.1%, Prefix cache hit rate: 76.6%

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/level_08/engine_minicpm5-2b-v031.log §L771-771
> (APIServer pid=184) INFO 10-16 07:53:05 [loggers.py:320] Engine 000: Avg prompt throughput: 206.6 tokens/s, Avg generation throughput: 56.6 tokens/s, Running: 0 reqs, Waiting: 0 reqs, GPU KV cache usage: 0.0%, Prefix cache hit rate: 51.5%

누적 추이는 정황이고 prefix cache 를 끈 대조 측정은 없다 — 그래서 TTFT 차이의 원인은 여전히 **미결**이다(Q1). 레벨이 높을수록 두 측정의 차이가 더 크다(03 §3.5 표).

```hint-event
id: W2
kind: wall
증상: 2차 스윕은 full 완주 · PASS 였으나 스윕 meta 가 dataset_duplicate_sampling True — 12 런이 같은 11 프롬프트를 돌려 썼다
서명: "행 선택이 `prompt_tokens_count == 1024` 로 비교해 6265 행 중 11 행만 남았고 같은 11 프롬프트가 반복됐다"
원인: 이 모델의 토크나이저가 BOS 를 세어 concatenate 1024 전처리 행 대부분이 prompt_tokens_count 1025 였는데 행 선택이 목표 길이를 input_len 과 정확일치(== 1024)로 비교했다
해소: 목표 길이를 전처리 분포 최빈값으로 고른다(dataset_prep target_rows · 자체검사 T1~T4) · 3차 스윕 재측정 · 앞선 리포트 SUPERSEDED
검증: 최종 스윕 런 사이드카 prompt_tokens_count_target 1025 · rows_available 6254 · duplicate_sampling false · 12 런 전부 겹치지 않는 연속 행(testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §4)
전이등급: arch-invariant
출처: [testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3, devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정, output/single/benchlog/sweep_minicpm5-2b-v031/level_01/dataset_minicpm5-2b-v031.json, bench_report_26101616_minicpm5-2b_GB10_0.31.0]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 2차 스윕(행 선택 결함 · 11 프롬프트 반복) 측정을 셀의 full-bench 결과로 쓰기 — c=1 decode 38.58 t/s · TTFT p50 64.9 ms · PASS
기각사유: dataset_duplicate_sampling True 로 입력 다양성이 무너져 prefix cache 영향을 배제할 수 없다 — 리포트에 SUPERSEDED 를 달고 교정 뒤 재측정으로 대체
출처: [bench_report_26101616_minicpm5-2b_GB10_0.31.0, testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3]
```

```hint-event
id: R2
kind: rejected
시도: GuideLLM 열 매핑을 `kind=..,column_mappings={..}` 쉼표 문법으로 넘기기
기각사유: CLI 가 쉼표에서 쪼개 다중 키 dict 가 깨졌다 — 라이브 1회 실패는 자매 셀(granite c=4)에서 관측 · 이 셀의 측정은 JSON 객체 한 덩이로 넘긴 판본에서 돌았다
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3]
```

```hint-event
id: R3
kind: rejected
시도: column_mappings 에 본문 열(`article`)만 매핑하기
기각사유: 매핑을 주면 기본 매핑 전체가 교체돼 출력 길이 열이 빠지고 max_tokens 가 사라져 ignore_eos 가 컨텍스트 끝까지 생성한다(이미지 소스 판독) — 실행 시점에는 text · prompt_count · output_count 세 열을 명시
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3]
```

```hint-event
id: R4
kind: rejected
시도: GuideLLM 문서의 `seed=` 문법과 단일 parquet 를 load_kwargs 없이 읽기
기각사유: 고정 이미지에서는 `--seed` 가 `kind=static,value=N` 이고 단일 parquet 는 `load_kwargs={"split":"train"}` 없이는 DatasetDict 라 매퍼가 죽었다(오프라인 spike)
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3]
```

이 셀의 **서빙** 형상에서 기각된 시도는 없다 — 서빙 설정은 앞선 캠페인 값 그대로이고 이 계보는 그것을 바꿔 보지 않았다(plan_26101410 · testlog_26101619_54_16 §3). 형상을 좁힌 것은 측정 쪽이다: 반복 입력 측정의 폐기(R1)와 GuideLLM 데이터셋 배선의 세 가지 문법 · 매핑 함정(R2~R4 — 하네스 공통 · R2 의 라이브 실패는 자매 셀 관측)이 이 측정의 입력 경로를 지금의 json_file + 세 열 매핑 + static seed 로 정했다(03 §3.4 결과 JSON 원문).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

**M1 — "측정 경로 불성립" 판정.** 1차 스윕에서 lite 판정은 `measurement_path_failed`(요청이 서버에 닿지 못했거나 측정기가 값을 못 냄)였지만, 실제로는 요청이 전부 성공했고 KV 한 지표만 파서가 못 읽었다. 판정이 서빙이나 측정 경로가 아니라 **파서의 읽기 창**을 가리키고 있었다. 정정은 커밋 0f8800c 와 재판정 pass 다(W1).

```hint-event
id: M1
kind: misdiagnosis
증상: 1차 스윕 lite 판정이 measurement_path_failed 로 full 진입을 막았다
서명: "lite KV 파싱이 엔진 로그 꼬리만 읽어 상주 서빙에서 늘 measurement_path_failed"
원인: 기동 때 한 번만 찍히는 KV 줄을 꼬리 파싱이 놓쳐 지표 하나가 N/A 가 된 것을 측정 경로 실패로 분류했다
해소: 측정 경로는 성립했다(요청 전부 성공 · warm 38.88 t/s) — 머리 우선 파싱 뒤 재판정 pass(KV 35.0 GiB)
검증: devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 · 커밋 0f8800c
전이등급: arch-invariant
현재지위: 반증
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정]
```

**M2 — "전처리 행의 프롬프트 토큰 = input_len" 가정.** 데이터셋 knob 를 설계한 plan 은 오프라인 전처리 spike 의 확인 항목을 "각 행 prompt 토큰 = 1024" 로 적었고, 행 선택도 그 가정대로 `== input_len` 으로 짰다. plan 이 정한 spike 토크나이저는 granite 의 것이었다. 이 모델의 토크나이저에서는 같은 전처리가 1025 토큰 행을 냈다(BOS 셈) — 원래 가정은 토크나이저에 따라 거짓이다. 정정은 목표 길이 = 분포 최빈값이다(W2). spike 가 granite 에서 1024 를 확인했는지 자체는 이 계보 문서가 따로 적지 않았다.

> [원문] plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §5
> | P2 | **오프라인 전처리 spike**: 고정 이미지 · `--network none` · granite 토크나이저 · concatenate 1024/256 · seed 고정 | 출력 jsonl 행 수 · 각 행 prompt 토큰 = 1024 확인 · 열 매핑 동작 | 실패 = **중단·보고**(exit ②) |

```hint-event
id: M2
kind: misdiagnosis
증상: 2차 스윕이 같은 11 프롬프트를 반복했는데도 full 완주 · PASS 리포트가 발행됐다
서명: "출력 jsonl 행 수 · 각 행 prompt 토큰 = 1024 확인"
원인: 전처리 행의 프롬프트 토큰이 언제나 input_len 과 같다고 가정하고 행 선택을 정확일치로 짰다 — plan 이 정한 spike 토크나이저는 다른 모델(granite)의 것이었다
해소: BOS 를 세는 토크나이저는 1025 를 낸다 — 목표 길이를 분포 최빈값으로 고르고 재측정 · 교정 전 리포트 SUPERSEDED
검증: testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3 · §4
전이등급: judgment
현재지위: 반증
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §5, testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3]
```

교정 묶음 — devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 의 D1~D9(9항목)을 훑었다. 이 셀의 판정 · 수치 · 기록을 바꾼 항목:

- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D4 — lite 판정 — measurement_path_failed → pass(KV N/A → 35.0 GiB)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D5 — 측정 입력 · 수치 — 11 프롬프트 반복 → 겹치지 않는 행 · c=1 decode 38.58 → 38.13 t/s · TTFT p50 64.9 → 106.7 ms
D3(열 매핑 세 열 명시 — 이미지 소스 판독으로 드러난 실행 전 교정) · D6(`dataset_tokenizer` 를 컨테이너 경로로)은 하네스 공통 교정이다 — 이 셀은 교정된 판본으로만 측정했고(1차 스윕은 GuideLLM 에 들어가지 않았다 · 두 리포트 모두 이미 `/app/models/…`), 교정 전 상태가 이 셀에서 관측된 적이 없다. D1 · D2 는 배선 문법(R4 · R2), D7~D9 는 문서 · 배달 사정이라 이 셀의 수치를 바꾸지 않았다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 절대클램프(V4)** — 처음 값이 이 계보에서 바뀐 적이 없다: `kv-cache-memory-bytes 37580963840`(35.0 GiB)은 plan_26101410 D4 의 절대 클램프 방침으로 앞선 캠페인에서 정해졌고, 이 계보는 그 서빙을 그대로 쟀다. 필요량은 plan 의 산식(토큰당 42KiB · 32k 1시퀀스 1.3GiB)이고, 엔진은 그 클램프에서 873,808 토큰 · 32768 기준 최대 동시성 26.67x 를 보고했다 — max-num-seqs 8 로 쓰는 몫보다 훨씬 크다. 그렇게 크게 둔 이유는 계보 표 9건 안에는 없다 — 이 계보 밖 문서(devlog_26101412 · testlog_26101412)가 KV 를 천장 − 가중치 − 오버헤드로 손 산정하고 사용자의 "64/64 할당" 의도에 맞춰 max_safe 를 택했다(kv_source=hand)고 기록한다. 음성대조라는 기록은 없다.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §모델 4종
> | 메인 | MiniCPM5-2B | `LlamaForCausalLM` · 42층 kv2 | ≈4.4GiB | 42KiB | 1.3GiB | `minicpm5xml`(모델카드: XML `<function name=…>`) · `<think>` |

> [원문] output/single/benchlog/sweep_minicpm5-2b-v031/lite_engine_head_minicpm5-2b-v031.log §L75-75
> (EngineCore pid=343) INFO 10-14 02:05:10 [kv_cache_utils.py:2464] GPU KV cache size: 873,808 tokens, Maximum concurrency for 32,768 tokens per request: 26.67x

**호스트 메모리 예산** — 원장의 합산 선언은 `floor_mib=23966` = 총량 124608 − 가중치 4800 − KV 35840 − overhead 60002 꼴이다(모두 **선언값** · 01 §1.4 원장 행 4). overhead 60002 는 D6 처방대로 granite 기동 뒤 실측 used 와 이 모델의 컨테이너 오버헤드를 합친 값이라 이 모델 혼자의 overhead 가 아니다. 엔진 실측은 로드 메모리 4.75 GiB · KV 35.0 GiB · 예약 합 39.75 GiB 다(03 §3.1).

**gpu-memory-utilization(V1)** — 0.39 는 D4 의 "천장 ÷ 호스트 총량 ≈0.39" 환산값(사람 결정)이고 이 계보에서 바뀌지 않았다. KV 를 클램프로 지정했으므로 엔진은 이 값으로 KV 를 산정하지 않는다(엔진 로그 L74).

**max-model-len(V2) · max-num-seqs(V3)** — 32768 은 사용자 지시 고정, 8 은 D5 의 동시성 요구 8 그대로다. 프리셋 스윕 레벨은 1·2·4·8·16 이 기본이었지만 16 은 max-num-seqs 8 을 넘어 걸러졌다(sweep_index `bench_knobs.levels.filtered_out`) — 8 초과 동시성은 계획만 있고 시도 0 이다.

**eager/graph** — 엔진 기본 cudagraph(FULL_AND_PIECEWISE · capture sizes 1~16)를 그대로 썼다. 엔진 로그는 그래프 캡처가 33 초 · 0.23 GiB 를 썼다고 적는다(lite_engine_head L86). eager 를 대조한 기록은 없다 — graph 를 고른 이유가 실측 비교에서 나온 것이 아니라 엔진 기본값이다.

**판정 산정에 승계된 입력** — spec 이 꺼져 있어 루브릭은 acceptance length 1.0 을 선언값으로 쓴다(03 §3.1 `accept_len` · 측정 아님 · 결손 아님). 측정 형상 input 1024 / output 256 은 사람 결정(plan_26101614 U16)이고 bench_budget 4096 MiB 는 재현 명령의 knob 사유 `host_budget` 로 고른 값이다(devlog_26101619 §5).

**이 형상이 버티는 기전 대 다른 형상** — 이 계보는 서빙 값(KV · gmu · 동시 시퀀스 · 배치 토큰)이나 형상(PLE · offload)을 바꿔 시도하지 않았으므로 "열렸다" 고 말할 비교가 없다 — 지위 해당 없음. 측정 조건만 하나 달랐다: 교정 전 2차 스윕은 입력 행이 반복됐고(`dataset_duplicate_sampling` True — 두 리포트의 측정 구성 · 측정 환경 표에서 다른 키는 이것 하나) TTFT 가 짧았다. 기전 후보는 반복 프롬프트의 KV 블록 재사용(prefix cache)이며 지위는 **미결**이다(추론 · 원인 미조사 — W2 의 hit rate 관측은 정황일 뿐 대조 실험이 아니다 · testlog_26101619_54_16 §3).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 오래 상주한 서빙의 lite 판정을 엔진 로그 꼬리 파싱 판본으로 믿지 않는다 — W1 · M1 — 머리 우선 파싱 판본(커밋 0f8800c 이후)인지 확인하고, `measurement_path_failed` 면 요청 성공 여부부터 본다.
- 데이터셋 행을 목표 길이 정확일치(`== input_len`)로 고르지 않는다 — W2 · M2 — 토크나이저 분포의 최빈값으로 고르고 런 사이드카 `duplicate_sampling` 을 확인한다.
- `duplicate_sampling: True` 인 측정을 판정 · 비교 근거로 쓰지 않는다 — R1 — 재측정한다.
- 교정 전 리포트의 TTFT(64.9 ms)를 이 셀의 TTFT 로 옮기지 않는다 — R1 · Q1 — 현행 측정(03 §3.2)을 쓴다.
- 한 토크나이저로 한 전처리 spike 결과를 다른 모델에 일반화하지 않는다 — M2(plan_26101614 §5 P2 는 granite 토크나이저) — 모델마다 사이드카의 목표 길이를 본다.
- GuideLLM 열 매핑을 쉼표 문법으로 넘기거나 본문 열만 매핑하지 않는다 — R2 · R3 — JSON 한 덩이 · 세 열 명시.
- 엔진 자기보고 `0.31.1.dev0+…` 를 빌드 입력으로 쓰지 않는다 — 00 §0.4 — `v0.31.0` 또는 40자 SHA 로 재현한다.
- 합산 예산 선언의 overhead(60002 MiB)를 단독 서빙에 옮기지 않는다 — plan_26101410 §4 D6 — 네 노드에서 다시 잰다.
- KV 35.0 GiB 를 필요조건으로 복사하지 않는다 — V4(승계 · 필요량보다 큼) — 토큰당 KV × 컨텍스트 × 동시성으로 다시 도출한다.
- 자매 셀(granite)의 수치 · 토큰 수를 이 셀 근거로 옮기지 않는다 — testlog_26101619_54_16 §1(같은 프리셋이어도 토크나이저가 다르다) — 이 셀의 사이드카 · 리포트를 쓴다.
- 실린 Dockerfile 을 측정 이미지의 바이트로 믿지 않는다 — 01 §1.1(history 16/17) — docker history build-arg 와 대조한다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 교정 전후 TTFT p50 차이(c=1 64.9 → 106.7 ms · 높은 레벨일수록 더 큼)의 원인은 prefix cache 인가
현재상태: 계보는 반복 프롬프트에서 prefix cache 가 TTFT 를 줄였을 가능성으로 추론했고 원인은 조사하지 않았다 · 엔진 로그 hit rate(최근 약 1000 요청 롤링 · 이 세션에선 기동 이후 누적)가 2차 스윕 뒤 정점 76.6% 로 오르고 3차 스윕 동안 51.5% 까지 내린 추이만 있다(구간별 적중률 아님)
다음관측: 같은 반복 입력으로 prefix caching 을 끈 서빙과 켠 서빙의 TTFT 를 나란히 잰다
출처: [testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3, devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정]
```

```hint-event
id: Q2
kind: open-question
물음: 동시성 8 초과(프리셋 기본 레벨 16)에서 이 서빙은 어떻게 되는가
현재상태: 레벨 16 은 max-num-seqs 8 을 넘어 스윕이 걸렀다 · 측정한 레벨은 1 · 2 · 4 · 8 이고 decode t/s 는 c=2 45.19 에서 c=8 40.41 로 내려간다
다음관측: max-num-seqs 를 올린 별도 셀에서 레벨 16 측정
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §5, bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0 §부하 스윕 곡선]
```

```hint-event
id: Q3
kind: open-question
물음: 외부 레퍼런스(E)를 넣으면 이 판정은 어떻게 되는가
현재상태: E-search 상태 no · 판정은 roofline-only weak(primary 18.98 · floor 16.13 · ratio 2.009)로 PASS · 인증서 비발행
다음관측: 이 모델 · GB10 의 외부 처리량 레퍼런스를 찾아 explicit 권한으로 재판정
출처: [bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0 §판정]
```

```hint-event
id: Q4
kind: open-question
물음: 같은 노드에 granite 서빙이 메모리에 상주한 것만으로 이 측정이 달라지는가
현재상태: 측정 동안 granite(8001)도 같은 노드에 떠 있었으나 그 CNN 스윕은 07:25:36Z 에 끝났고 엔진 로그 마지막 통계 줄이 07:25:48Z Running: 0 reqs 다(이 계보 밖 기록) — 이 셀의 2차 · 3차 스윕 동안 요청 없이 유휴였다 · 남는 것은 메모리 상주(노드 RAM 96.2 GiB 에 포함)이고 단독 상태와의 대조는 없다
다음관측: granite 를 내린 단독 상태에서 같은 프리셋으로 재측정해 레벨별로 비교
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0 §lite 지표]
```

현행 full 정의(03 §3.5 사실 블록)로 이 측정은 충족이다 — 열린 물음이 아니다. 상위 vLLM 에서의 자체 패치 필요성은 이 셀에 패치가 0 이라 해당하지 않는다.
