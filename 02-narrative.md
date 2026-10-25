# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 7건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함.md` | testlog | 26102504 | 0 | 3189 | 0.31.0 · ctx · easy-vllm:0.31.0-cu133-aarch64-source · gemma26b · gemma26b-ctx-single · single | — |
| 2 | `docs/devlog/devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사.md` | devlog | 26102504 | 0 | 1524 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · single | — |
| 3 | `docs/benchmark/bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md` | bench_report | 26102509 | 0 | 6428 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · single | — |
| 4 | `docs/plan/plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화.md` | plan | 26102420 | 0 | 6650 | 0.31.0 | — |
| 5 | `docs/testlog/testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결.md` | testlog | 26102418 | 1 | 3124 | 0.31.0 · ctx · gemma26b | — |
| 6 | `docs/plan/plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증.md` | plan | 26102401 | 1 | 7359 | ctx · single | — |
| 7 | `docs/testlog/testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정.md` (모호 해소) | testlog | 26102412 | 2 | 1939 | 0.31.0 · ctx · easy-vllm:0.31.0-cu133-aarch64-source · gemma26b · gemma26b-ctx-single · single | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

이 파일의 `lite_engine L<n>` 은 측정 기동의 엔진 로그 `output/single/benchlog/sweep_gemma26b-ctx-single/lite_engine_gemma26b-ctx-single.log` 줄 번호다(01 §1.4 머리).

**목표** — 계보 표에서 날짜가 가장 이른 plan 은 plan_26102401(2026-10-24 KST)이다. 4모델(Qwen3.8-Flash-Next · Nemotron-3.5-Super-VL · Gemma-4-26B-A4B-it · Solar-Open2)을 싱글 노드에서 먼저 기동하고, 지원 최대 context 로 health + 근접 길이 추론 1회를 통과시킨 뒤 lite/full 벤치까지 올리는 캠페인이었다. 이 셀은 그 가운데 Gemma-4-26B-A4B-it 을 싱글 GB10 1대(TP=1)에서 max-model-len 262144 로 띄운 셀이다.

> [원문] plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §사용자 결정
> | context | 지원 최대치 기동 + health + 근접 길이 추론 1회 → **그 위에 lite/full 벤치까지**(full 은 별도 사람 승인) |
> | 배치 | 모델별 순차 기동, 메인/서브에 병렬 배정(노드당 한 번에 1모델, Gemma 만 보조 동거 후보) |

**HW 출발 조건** — NVIDIA GB10 · 노드당 GPU 1 · 이 셀은 노드 1대(00 §0.2 축 표 · single TP=1)이고 서브 노드 · Ray · 노드 간 인터커넥트를 쓰지 않는다. 통합메모리이며 첫 plan 은 노드당 통합메모리를 `~121GiB` 로 적었다(plan_26102401 §NAS 사실). 실측 MemTotal 은 이 측정의 lite raw `nodes[0].ram_total_kib 127598772`(KiB = 124608 MiB · 121.69 GiB)이고 트라이얼 요약도 `budget_source … measured(/proc/meminfo MemTotal …)` · 121.69 GiB 를 적는다(run_summary L73 ~ L74) — 블랙박스 원장 예산 선언의 `mem_total_mib 124608` 은 이 실측과 같은 값을 선언에 옮긴 것이다(02 §2.5). 측정 엔진 로그는 `Initial free memory 112.81 GiB` 를 찍었다(lite_engine L84 · 관측). 호스트 드라이버는 manifest 선언 580.178.04 를 측정 도구가 옮긴 값이고 노드 관측은 없다(00 §0.4). 호스트 OS 는 이 계보에서 찾지 못했다(**미기록**). 이미지 좌표(CUDA 13.3.1 · torch 2.13.0a0 · NGC 26.07)는 00 §0.4 의 별도 층이다.

**모델 구조** — HF `google/gemma-4-26B-A4B-it` · revision `01e5b3ee840d3a9e0b0b493c593e85398a30ef75` · base `google/gemma-4-26B-A4B`(00 §0.2). 체크포인트 config 를 저작 시 직접 읽은 관측(계보 문서는 층 구성을 적지 않았다): 아키텍처 `Gemma4ForConditionalGeneration` · 30층 = `sliding_attention` 25 + `full_attention` 5 · sliding window 1024 · MoE expert 128 · 토큰당 8 · head_dim 256(sliding) / 512(full) · `max_position_embeddings 262144` · rope 는 full 층 `proportional`(partial_rotary_factor 0.25) · sliding 층 `default` · PLE 키(`ple_layer_ids` 등) 없음(00 §0.9 후보 표 근거 칸도 부재 관측) · MTP 층 키 없음. 엔진은 이 헤드 차원 이질성 때문에 attention 백엔드를 TRITON_ATTN 으로 강제했다(lite_engine L48). 크기는 safetensors index `metadata.total_size` 51,611,872,412 B(저작 시 관측 · `total_parameters` 26,544,131,376)이고 측정 엔진 로그도 `Checkpoint size: 48.07 GiB` 를 찍는다(lite_engine L64). 첫 plan 표의 NAS `du` 97G 는 이 값의 약 2배다 — 같은 plan 의 정정 기록은 Qwen 크기에 대해 "du 크기는 부풀려졌다" 고 적었다(plan_26102401 §정정·진행 기록). 양자화는 없다(config 에 `quantization_config` 부재 · 기본 dtype bfloat16 · 00 §0.4) — "A4B" 는 활성 약 4B 라는 이름이다. 판정기 roofline 은 활성 가중치를 `dense 5.53GiB + (0/128)×routed 42.54GiB` 로 셌다(roofline.json notes) — routed expert 를 0개로 센 것은 생산자 roofline.py 결함이다(top-k 를 `num_experts_per_tok` 류 키에서만 찾아 이 config 의 `top_k_experts: 8` 을 읽지 못했다 · 03 §3.5).

**stock 상태의 장벽**(자세히는 2.2)
- 첫 기동 트라이얼은 KV 절대 클램프 없이 gmu 0.85 만으로 떠서 엔진이 KV 52.89 GiB 를 잡았고, 호스트 memwatch 가 MemAvailable 문턱 아래에서 컨테이너를 죽였다(W1).
- 레시피 생성기가 서빙 yaml 의 `port` 에 호스트 포트(8912)를 적어 컨테이너 안 health 프로브가 닿지 못했다(W2).
- YaRN 은 config · 모델 카드에 선언이 없어 262144 를 넘는 확장을 시도하지 않았다(R1) · NAS 에 Gemma 양자화본이 없어 양자화 비교 셀은 만들지 않았다(R2).

**선례와 사람의 결정** — 이 계보는 Gemma 에 대해 외부 성능 선례를 기록하지 않았고(판정기 E 검색도 `empty` · bench_report §판정), 첫 plan 은 YaRN 지원 여부를 모델 카드 · 업스트림으로 확증하고 미지원이면 "YaRN 미지원 · 최대 N" 으로 정직 기록하라고 정했다(plan_26102401 §⚠ 사전에 짚을 전제 1). 사람은 full 벤치를 셀별 별도 승인으로 남겼고(plan_26102401 §사용자 결정), 캠페인 종결 뒤 plan_26102420 이 S5 로 통과 셀의 full 벤치를 셀별 사람 승인 단계로 잡았다(§단계 · 이 셀의 승인 발화 전사는 계보에 없다) — 이 셀의 full 측정은 그 S5 묶음이며 캠페인 밖(ACTIVE=_bootstrap)에서 잰 것이다(testlog_26102504 유효맥락 · 00 머리 사실 블록). 첫 plan 이 적은 "Gemma 보조 동거" 후보는 이 계보에서 시도 기록이 없다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계 미관측** — 재생 모드(캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음)) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-25T03:13:55Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T21:48:33Z | 1회 · 첫 2026-10-23T21:48:34Z | 없음 | `budget_clear` 2026-10-23T22:03:55Z | 5 |
| 2 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T23:52:08Z | 8회 · 첫 2026-10-23T23:52:10Z | 없음 | `budget_clear` 2026-10-24T03:50:45Z | 5 |
| 3 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:02:06Z | 1회 · 첫 2026-10-25T00:02:08Z | 없음 | `budget_clear` 2026-10-25T00:09:42Z | 5 |
| 4 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:11:00Z | 17회 · 첫 2026-10-25T00:11:01Z | 없음 | `budget_clear` 2026-10-25T00:44:20Z | 12 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

**기동 시도와 벽의 대응** — 위 표는 시도 4개를 묶는다. 시도 1 · 2 는 캠페인 라벨(`camp-26102401-gemma26b`)이라 기계가 "이 셀의 선언으로 세지 않는다" 고 적었지만, 같은 행의 `serve_start` 세션 이름은 `serve-gemma26b-ctx-single-…` 이다(01 §1.4 표 #2 · #10). 시각으로 대응이 확인되는 것: 시도 1(2026-10-23T21:48:33Z ~ 22:03:55Z · serve_stop duration 922 s)은 `serve_fail_gemma26b-ctx-single/engine_timeout_20261023T220342Z.log` 의 컨테이너 시각 `10-23 21:48:49` 기동 인자 줄(`'port': 8912`)을 담는다(W2) · 시도 2(23:52:08Z ~ 2026-10-24T03:50:45Z)는 캠페인 lite raw(`output/single/benchlog/lite_raw_gemma26b-ctx-single.json` · measured_utc 2026-10-24T03:49:19Z)를 창 안에 품으므로 testlog_26102412 의 "정규 기동 재시도: health 200 (470 s)" 과 needle · lite(25.11 t/s) 회차로 읽힌다 · 시도 4(2026-10-25T00:11:00Z ~ 00:44:20Z)는 이 페이로드의 full 측정이다(measured_utc 00:44:08Z 가 창 안). 시도 3(00:02:06Z ~ 00:09:42Z · 456 s · 측정 없이 닫힘)은 계보 문서 어디에도 기록이 없다 — 실패였는지 수동 중단이었는지 **미확인**이다(Q2). 이 표 밖에도 원장에는 W1 의 트라이얼 행(2026-10.jsonl L491 `budget_skipped` · L492 trial02 `budget_declare` · L493 `watchdog_trip`)이 있다 — 라벨이 `trial01/02-gemma-4-26b-a4b` 라 기계 묶음에 들지 않았다.

### W1 KV 절대 클램프 없이 기동 → 엔진이 KV 52.89 GiB 를 잡고 memwatch 가 사살

캠페인의 첫 실기동 트라이얼(trial01 · 2026-10-23T21:21Z ~)은 `kv-cache-memory-bytes` 가 비어 있었다(trial01 후보 yaml `kv_cache_memory_bytes: null` · trial02 후보 yaml 과의 차이는 그 한 줄뿐이다 — 저작 시 diff). 원장은 KV 클램프가 없다는 이유로 예산 선언을 생략했고(`budget_skipped` · reason `kv_clamp_absent` · 2026-10.jsonl L491), 엔진은 gmu 0.85 로 KV 52.89 GiB(4,508,059 tokens)를 잡았다(trial01_profile.json). 로드 뒤 MemAvailable 이 10,240 MiB 문턱 아래로 내려가 컨테이너가 사살됐다. 같은 컨테이너(`4c681a4ea495`)를 겨눈 기록이 두 워치독 인스턴스에서 따로 남았다 — 상주 systemd 유닛(원장 2026-10.jsonl L493 `watchdog_trip` · source `unit:easy-vllm-memwatch` · rule absolute · 2026-10-23T21:41:20Z · mem_avail_mib 9988 · kill_ack L494)과 트라이얼 하네스가 띄운 인스턴스(trial01_memwatch.log · L1 `filter='vllm_trial01'` · 아래 발췌 21:41:21Z · 9962)다. 수치 · 시각이 다르므로 한 사건의 한 측정으로 섞지 않는다. 트라이얼 분류기는 OOM 예외가 없고 KV 할당 기록이 있으므로 CUDA OOM 이 아니라 외부 종료라고 판정했다:

> [원문] trial01_memwatch.log §L76-78
> [mem-watchdog] HB MemAvailable=10792MiB min=10792MiB 2026-10-23T21:41:08Z
> [mem-watchdog] TRIP MemAvailable=9962MiB < 10240MiB → docker kill 4c681a4ea495 2026-10-23T21:41:21Z
> [mem-watchdog] 4c681a4ea495

해소는 KV 절대 클램프 3,368,402,176 B(= 262144 토큰 × batch 1 에 필요한 양 · run_summary after_note)를 넣는 것이었다 — 클램프가 생기면서 예산 선언도 생략되지 않게 됐다(L491 의 생략 사유가 클램프 부재다). 선언은 사살을 막은 수단으로 기록되지 않았다: 사살한 memwatch(mem_watchdog.sh)는 MemAvailable 절대 임계만 보고 선언 · 무장 상한을 읽지 않는다. trial02 는 예산을 선언했고(L492 · floor 47373 · 선언값) chat · 툴콜 · reasoning 스모크를 통과했다(trial02_smoke.json `passed: true`). 왜 통하는가: 클램프가 있으면 엔진은 gmu 로 KV 를 산정하지 않고 지정 바이트만 잡는다고 스스로 기록한다(측정 기동 lite_engine L84 `reserved 3.14 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling. This does not respect the gpu_memory_utilization config`). 통합메모리라 KV 52.89 GiB 와 가중치 48.54 GiB 가 같은 호스트 풀에서 나가고(분류기 기록 "상주 KV+가중치 101.43 GiB / 안전예산 103.4 GiB (98%)"), 그 결과 OS 몫이 문턱 밑으로 눌렸다 — 엔진이 할당해 쥔 KV 는 커널이 회수할 수 있는 page cache 가 아니라는 기전은 이 계보가 대조로 보이지 않았다(추론).

```hint-event
id: W1
kind: wall
증상: KV 클램프 없이 뜬 트라이얼이 로드 뒤 호스트 memwatch 에 사살됨(예산 선언은 kv_clamp_absent 로 생략)
서명: "[mem-watchdog] TRIP MemAvailable=9962MiB < 10240MiB → docker kill 4c681a4ea495"
원인: kv-cache-memory-bytes 부재로 엔진이 gmu 0.85 기준 KV 52.89 GiB 를 잡아 통합메모리 MemAvailable 이 memwatch 절대 문턱(10240 MiB) 아래로 내려갔다
해소: kv-cache-memory-bytes 3368402176(262144 × batch 1 필요량) 절대 클램프 후 재기동(클램프가 있어 예산 선언도 생략되지 않았다)
검증: trial02_smoke.json passed true · 측정 기동 lite_engine L84 reserved 3.14 GiB for KV Cache as specified by kv_cache_memory_bytes
전이등급: arch-scaled
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/trial01_memwatch.log, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, docs/logs/main/events/2026-10.jsonl]
```

### W2 서빙 yaml 의 `port` 에 호스트 포트 — 컨테이너 안 health 불도달

트라이얼 수렴 뒤 `single_serve_up.sh` 정규 기동 1차가 rc=5 로 끝났다(testlog_26102412 §1). 엔진 자체는 정상 기동해 API 서버까지 떴지만 컨테이너 안에서 8912 를 들었고, compose 는 `${SERVING_PORT}:8000` 으로 호스트 8912 → 컨테이너 8000 을 매핑하므로 health 프로브가 닿을 곳이 없었다(compose L52 · 셀 env `SERVING_PORT=8912`). 원인은 레시피 생성기가 호스트 포트를 컨테이너 내부 포트 칸에 쓴 것이라고 testlog 가 적었다. 해소는 이 트리플렛 yaml 의 `port` 를 8000 으로 손으로 고친 것이고(yaml L10 주석이 그 교정을 적는다) 생성기 결함은 미수정 후속이다. 통과 판정은 다음 기동의 health 200(470 s)이었다.

```hint-event
id: W2
kind: wall
증상: single_serve_up.sh 1차 rc=5 — 엔진은 떴으나 health 프로브가 닿지 않아 기동 대기 타임아웃
서명: "Starting vLLM server on http://0.0.0.0:8912"
원인: 레시피 생성기가 서빙 yaml port 칸에 호스트 포트(8912)를 적어 컨테이너가 8912 를 들었고 compose 는 호스트 8912 를 컨테이너 8000 으로 매핑했다
해소: 이 트리플렛 yaml 의 port 를 8000 으로 수작업 교정(생성기 결함은 미수정 후속)
검증: testlog_26102412 §1. 기동 정규 기동 재시도 health 200 (470 s) rc=0
전이등급: arch-invariant
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동, output/single/benchlog/serve_fail_gemma26b-ctx-single/engine_timeout_20261023T220342Z.log, gemma26b-ctx-single.yaml, docker-compose.yaml]
```

이 셀의 측정 기동(시도 4)에서는 사살 · 트립 · 거부가 없다(위 표). 로드 중 `watchdog_highrate_hold`(rate 29684 > max_rate 20768 MiB/s · `legacy_rule_would_trip=true`)가 한 번 찍혔지만 사살이 아니라 보류다(01 §1.4 표 #37) — 벽으로 세지 않는다.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: YaRN rope 오버라이드로 262144 를 넘는 max-model-len 확장
기각사유: 모델 카드 · config 에 YaRN 선언이 없어(p-RoPE · native 256K) 추측 factor 를 주입하지 않기로 했다 — 기동해 실패한 것이 아니라 근거 부재로 시도하지 않았다(시도 0)
재개조건: 업스트림 · 모델 카드가 공식 YaRN(또는 다른 확장) factor 를 선언할 때
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동, plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §⚠ 사전에 짚을 전제 (착수 전 확증 대상)]
```

```hint-event
id: R2
kind: rejected
시도: 양자화본(FP8 · NVFP4 등)과의 비교 셀
기각사유: NAS 에 Gemma 양자화본이 없어 셀을 만들지 않았다(시도 0 · 실패 아님) — 이 셀은 BF16 원본 하나다
재개조건: 같은 체크포인트의 양자화본을 획득 게이트로 들였을 때
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §NAS 사실 (실측 2026-10-24)]
```

```hint-event
id: R3
kind: rejected
시도: sweep_bench.sh 기본 도구(--tool 미지정 = vllm)로 잰 full 벤치
기각사유: full 정의(GuideLLM × 반복 ≥3)가 아니라서 무효 처리했다 — 무효가 된 것은 S5 첫 패스의 멀티 3셀이고, 이 셀의 측정 색인은 measured_tool guidellm · bench_tool_version 0.7.3 이다
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사 §시도 · 폐기]
```

**요약** — 이 셀의 형상을 좁힌 것은 실패한 대안이 아니라 근거의 부재다: YaRN 선언이 없어 컨텍스트가 native 262144 에 묶였고(R1), 양자화본이 없어 BF16 하나만 잴 수 있었다(R2). 메모리 쪽 형상은 W1 의 KV 클램프 부재 실패 하나가 정했고, KV · gmu · 동시성의 레버 스윕은 계획도 시도도 없다(02 §2.5). R3 은 이 셀이 아니라 같은 S5 묶음의 다른 셀에서 버려진 측정 경로다 — 이 셀의 수치가 그 경로에서 나오지 않았다는 확인으로 싣는다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 W1 요약 줄의 괄호 `kv_clamp_absent` — 무엇의 사유인지 모호한 기재

캠페인 testlog 는 첫 트라이얼을 한 줄로 요약했다:

> [원문] testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동
> - 1차 시도: 예산 선언 없이 진입 → 워치독 사살(`kv_clamp_absent`). 2차: 측정 KV 클램프로 수렴(simlog `docs/simlog/2026102406_2_gemma4-26b-a4b-262k/`).

이 문장은 원장과 모순되지 않는다 — '예산 선언 없이 진입' 은 L491 `budget_skipped`(reason `kv_clamp_absent`), '워치독 사살' 은 L493 `watchdog_trip`(source `unit:easy-vllm-memwatch` · rule absolute · MemAvailable 9988 < 10240 MiB)과 순서대로 맞는다. 다만 괄호가 '사살' 바로 뒤에 붙어 있어 사살 사유처럼 읽힐 수 있다. 원장으로 보면 `kv_clamp_absent` 는 선언 생략의 사유이고, 사살은 MemAvailable 절대 임계 워치독의 발화다 — 그 워치독은 선언 · 무장 상한을 읽지 않으므로 선언 부재가 사살 경로의 일부였다는 근거는 없다. 수신자는 괄호를 사살 사유로 읽지 말고 근본을 KV 클램프 부재(W1)로 읽는다.

```hint-event
id: M1
kind: misdiagnosis
증상: 첫 트라이얼이 로드 뒤 컨테이너 사살로 끝남
서명: "1차 시도: 예산 선언 없이 진입 → 워치독 사살(`kv_clamp_absent`)"
원인: 선언 생략과 사살을 요약 한 줄에 순서대로 적으면서 선언 생략 사유(kv_clamp_absent)를 사살 뒤 괄호에 두었다 — 원장과 모순은 없고 읽기가 모호하다
해소: kv_clamp_absent 는 같은 트라이얼의 budget_skipped 사유이고 사살은 unit:easy-vllm-memwatch 의 rule absolute(MemAvailable 9988 < 10240 MiB) 발화다 — 근본은 클램프 부재로 KV 52.89 GiB 가 잡힌 것(W1)
검증: docs/logs/main/events/2026-10.jsonl L491 budget_skipped reason kv_clamp_absent · L493 watchdog_trip source unit:easy-vllm-memwatch rule absolute
전이등급: judgment
현재지위: 미결
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동, docs/logs/main/events/2026-10.jsonl, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/trial01_memwatch.log]
```

현재지위 `미결` 은 원문이 괄호를 사살 사유로 주장했는지 원문만으로 가를 수 없다는 뜻이다 — 원문의 사건 순서 · 클램프 처방은 원장과 맞는다.

**교정 묶음** — 계보의 교정 목록은 둘이다. testlog_26102504 §2 하네스 결함 5건 중 이 셀에 닿는 것은 #1(기본 도구 vllm — 이 셀은 guidellm 로 재서 무효 대상 아님 · R3)과 #2(bench 예산 4096 이 BLOCK — testlog 는 "arm_ceiling 한도 초과" 라고 적지만 벤치 진입 게이트의 산식은 `floor − bench ≥ abs_band` 다 · Qwen TP1 · Solar multi 사례이며 이 셀의 색인은 bench_budget_mib 4096 을 적고 측정은 완주했다 · 01 §1.4 (d) · Q3)이고, #3(E 무효 → `--e-search empty`)은 Qwen 싱글 사례, #4(judge 신선도)는 Solar 사례, #5 는 서브 러너다. testlog_26102418 §하네스 교정 4건 중 이 셀에 영향을 준 것은 1번(생성기 포트 결함 = W2)뿐이다 — 2번(YaRN 의미 변경)은 YaRN 을 쓰지 않은 이 셀과 무관하고 3 · 4번은 멀티 서브 통로다.
- testlog_26102418 하네스 교정 1 — 이 셀의 yaml `port` — 8912 → 8000(수작업 · 생성기 미수정)

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 클램프(V3)** — 처음 값은 없음(null)이었고 엔진이 gmu 0.85 로 KV 52.89 GiB 를 잡아 사살됐다(W1). 트라이얼 루프가 필요량으로 클램프를 정했다:

> [원문] run_summary.json §L56-56
>       "after_note": "kv=min(required=3368402176, max_safe=56790205072)=3368402176. (max_model_len=262144×batch=1) (잠정 클램프 — batch 산출 전이다. 통과 트라이얼에서 산출 batch 로 재산정한다)",

"잠정 클램프" 라고 적혔지만 최종 후보(run_summary L11)와 이 셀의 yaml 은 같은 3368402176 이고 다른 값으로 바꿔 잰 기록은 없다. 측정 기동에서 엔진은 이 클램프로 KV 267,398 tokens · 262,144 토큰 요청 기준 동시성 1.02x 를 잡았다(lite_engine L85) — 필요량 대비 약 1배(max-model-len 한 요청분)이고 음성대조로 일부러 키운 값이 아니다.

**gmu(V1)** — 0.85 다. yaml 머리는 출처를 `gmu_source=hand` 로, 트라이얼 요약은 `deploy_gmu_source: config.safety_margin 승계` 로 적는다(run_summary L67) — 두 기록이 말하는 출처가 다르지만 어느 쪽이든 이 셀에서 바꿔 잰 기록은 없다. 클램프가 있으면 엔진은 KV 산정에 이 값을 쓰지 않는다고 기록한다(lite_engine L84). 0.85 가 기동 전 free-memory 게이트로 작용하는지는 yaml L11 주석이 "기전 미확정" 으로 적는다.

**max-model-len(V2) · kv-cache-dtype(V4)** — 262144 는 config 의 `max_position_embeddings` 이고 YaRN 을 적용하지 않아 그 이상을 시도하지 않았다(R1). fp8 은 Phase 1 후보가 고른 KV dtype 이고(run_summary L10 `kv_cache_quant: fp8`) auto 와 비교한 기록이 없다. 엔진은 fp8 KV 가 적절한 scaling factor 없이 정확도 저하를 낼 수 있다고 경고한다(lite_engine L45) — 이 계보는 정확도 비교를 하지 않았다.

**호스트 메모리 예산(선언값)** — 원장 선언은 `floor = mem_total − weights − kv − overhead` 꼴이고, 측정 기동의 선언은 다음과 같다:

> [원문] 2026-10.jsonl §L601-601
> {"expires_epoch": 1792894260, "floor_mib": 53743, "kind": "budget_declare", "kv_mib": 3212, "label": "serve-gemma26b-ctx-single", "mem_total_mib": 124608, "overhead_mib": 18432, "source": "blackbox_session", "ts": "2026-10-25T00:11:00Z", "weights_mib": 49221}

124608 − 49221 − 3212 − 18432 = 53743 MiB 이고 무장 상한은 50671 MiB 다(watchdog.jsonl budget_honored · 01 §1.4 표 #34). 캠페인 기동(시도 1 · 2)의 선언은 overhead 24800 · floor 47375 였다(01 §1.4 표 #1 · #9) — overhead 를 18432 로 바꾼 이유는 계보에 **미기록**이다. 선언 weights 49221 MiB 와 엔진 실측 `Model loading took 48.54 GiB`(lite_engine L75)는 서로 다른 생산자의 값이다(선언 대 관측).

**graph · 스윕** — yaml 에 `enforce-eager` 가 없어 엔진 기본값으로 CUDA graph 를 캡처했다(PIECEWISE 51 · FULL 35 · lite_engine L98 · L99). 캡처 스파이크로 막힌 기록은 없다. KV · gmu · 동시 시퀀스의 레버 스윕은 계획도 시도도 없다 — 네 노브 모두 승계값이다(01 §1.3).

**이 형상이 버티는 기전 대 다른 형상이 실패한 이유**
- trial01(KV 클램프 없음 · 예산 선언 없음) 대 이 셀(클램프 3368402176 · 선언 있음) — 두 후보 yaml 의 차이는 `kv_cache_memory_bytes` 한 줄이고, 그 밖에 예산 선언 유무가 다르다 — 클램프가 없으면 엔진이 gmu 0.85 로 KV 52.89 GiB 를 잡아 가중치와 함께 통합메모리 MemAvailable 을 memwatch 절대 문턱 아래로 눌렀다 · 클램프가 있으면 KV 는 3.14 GiB 로 고정되고 엔진은 프로파일링을 건너뛴다 — 확정(트라이얼 대조 · 원장 trip 행 · 엔진 로그 L84) — testlog_26102412 §1. 기동 · W1.
- 캠페인 정규 기동 1차(yaml port 8912) 대 이후 기동(port 8000) — 메모리 형상은 같고 포트 칸만 다르다 — 컨테이너 내부 포트가 compose 매핑 대상(8000)과 달라 health 가 닿지 않았다 · 메모리 기전이 아니다 — 확정(엔진 로그 `'port': 8912` · 다음 기동 health 200) — testlog_26102412 §1. 기동 · W2.
- 다른 KV 값 · gmu · 동시 시퀀스 · 배치 토큰 형상 — 시도 0 — 대조가 없다 — 미결.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- KV 절대 클램프 없이 gmu 만으로 기동하지 않는다 — W1(KV 52.89 GiB → memwatch 사살) — 목표 컨텍스트 × 동시 요청 수의 필요량으로 `kv-cache-memory-bytes` 를 정하고 예산을 선언한 뒤 띄운다.
- 예산 선언이 생략된 채 기동하지 않는다 — W1(클램프가 없으면 선언 자체가 `kv_clamp_absent` 로 생략된다 · 2026-10.jsonl L491) — 클램프를 먼저 정하고 `budget_preflight.py` 로 floor · 무장 상한을 본 뒤 선언한다. 선언은 하네스의 기동 절차이고, 이 셀을 죽인 MemAvailable 절대 임계 워치독은 선언과 무관하게 돈다.
- 레시피 생성기가 쓴 yaml `port` 를 그대로 믿지 않는다 — W2(호스트 포트 8912 가 컨테이너 포트 칸에 들어가 health 불도달) — yaml 은 컨테이너 내부 포트 8000, 호스트 포트는 셀 env `SERVING_PORT` 로 둔다.
- 요약 문서의 괄호 하나로 사살 사유를 정하지 않는다 — M1 — 원장 `watchdog_trip` 의 `rule` · `mem_avail_mib` 와 `budget_*` 행을 따로 읽는다.
- YaRN factor 를 추측으로 주입하지 않는다 — R1 — 모델 카드 · 업스트림이 선언한 확장만 쓴다(없으면 native 최대로 정직 기록).
- full 벤치를 `--tool` 없이 돌리지 않는다 — R3(기본 도구 vllm 측정은 full 이 아니라 무효) — `--tool guidellm` 을 명시하고 색인의 `measured_tool` 을 확인한다.
- NAS `du` 로 체크포인트 크기를 판정하지 않는다 — plan_26102401 §NAS 사실(du 97G) 대 safetensors `total_size`(48.07 GiB · 엔진 로그 L64) — index 의 `metadata.total_size` 나 엔진의 `Checkpoint size` 줄을 쓴다.
- 엔진 기동 배너 `0.31.1.dev0+gdb9527a46…` 를 빌드 입력으로 적지 않는다 — 00 §0.4 — 재현 좌표는 `v0.31.0` 또는 40자 SHA 다.
- 캠페인 lite 25.11 t/s 를 full 판정점 23.29 t/s 와 같은 축에 놓지 않는다 — 03 §3.5 · testlog_26102412 §3. lite — 도구 · 입출력 길이 · 기동 회차가 다르다.
- 로드 중 `watchdog_highrate_hold` 를 사살로 읽지 않는다 — 01 §1.4 표 #37(보류 · 사살 0) — 같은 창의 `watchdog_trip` · `*_kill_ack` 유무로 가른다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: max-model-len 262144 의 완전 한계(262k 근처 프롬프트)에서도 추론이 성립하는가
현재상태: 증분 needle 은 240,932 토큰(262,144 의 92%)까지 PASS · 사살 0 · 그 위는 미시험
다음관측: 260k 전후 프롬프트 1회 needle(증분 프리필 · 예산 선언 하) — 엔진 KV 267,398 tokens 안에서 끝나는지
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §2. 근접 길이 needle, testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결 §결과표]
```

측정 부하(입력 1024 · 출력 256)는 짧은 요청이라 장문에서의 decode 속도는 이 페이로드가 말하지 않는다.

```hint-event
id: Q2
kind: open-question
물음: 원장 시도 3(2026-10-25T00:02:06Z ~ 00:09:42Z · serve_stop 456 s · 측정 없음)은 무엇이었나
현재상태: 같은 라벨 serve-gemma26b-ctx-single 로 선언 · 기동 후 7분 36초에 닫혔고 사살 · 트립 행은 없다 · 계보 문서 어디에도 이 기동의 기록이 없다
다음관측: 그 창의 엔진 로그 또는 운영 기록 — 수동 중단인지 측정 진입 실패인지
출처: [docs/logs/main/events/2026-10.jsonl, testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §1. 결과표]
```

```hint-event
id: Q3
kind: open-question
물음: 이 셀의 벤치 컨테이너 예산은 2048 MiB 였나 4096 MiB 였나
현재상태: S5 testlog 머리는 측정 구성으로 bench 예산 2048 MiB 를 적고 하네스 결함 #2 에서 4096 이 BLOCK 된 셀을 Qwen TP1 · Solar multi 로 든다 · 이 셀의 스윕 색인 bench_knobs 는 bench_budget_mib 4096 을 적는다 · 어느 쪽이든 선언 floor 53743 에서 벤치 진입 게이트(floor − bench ≥ abs_band 10240)는 통과한다(budget_preflight.py 저작 시 실행)
다음관측: 레벨 run 의 guidellm 컨테이너 --memory 값이 남은 실행 로그
출처: [testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함 §2. 하네스 결함 · 처방, output/single/benchlog/sweep_gemma26b-ctx-single/sweep_index.json]
```

```hint-event
id: Q4
kind: open-question
물음: 외부 레퍼런스(E)가 있었다면 이 셀의 판정은 무엇이었나
현재상태: E 검색 empty — 판정은 roofline 기대치 16.1 t/s(R_fp 45.99 × realistic_fraction 0.35)의 하한 13.69 대비 PASS(ratio 1.447 · 권한 weak) · 인증서 없음
다음관측: 같은 체크포인트 · GB10 1대 · BF16 의 외부 decode 수치(측정 조건 포함)
출처: [bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0 §판정 (표시만 — verdict_rule.py 결과), output/single/benchlog/sweep_gemma26b-ctx-single/roofline.json]
```

```hint-event
id: Q5
kind: open-question
물음: 캠페인 기동과 측정 기동의 예산 overhead 가 24800 에서 18432 MiB 로 바뀐 이유는 무엇인가
현재상태: 원장 선언값만 있다(시도 1 · 2 overhead 24800 floor 47375 · 시도 3 · 4 overhead 18432 floor 53743) · 계보 문서에 근거가 없다 · 측정 기동은 사살 없이 완주했다
출처: [docs/logs/main/events/2026-10.jsonl]
```

현행 full 정의(03 §3.5 사실 블록)로 이 측정은 충족이다(반복 3 · GuideLLM) — 열린 물음이 아니다.
