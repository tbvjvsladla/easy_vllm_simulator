# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 12건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite.md` | plan | 26101410 | 0 | 10727 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 2 | `docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md` | testlog | 26101619 | 0 | 2895 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 3 | `docs/devlog/devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031.md` | devlog | 26101620 | 0 | 2077 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 4 | `docs/devlog/devlog_26101621_lite셀_minicpm5_2b_v031.md` | devlog | 26101621 | 0 | 1202 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 5 | `docs/testlog/testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정.md` | testlog | 26101621 | 0 | 2748 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 6 | `docs/benchmark/bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md` | bench_report | 26101616 | 0 | 6510 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 | — |
| 7 | `docs/plan/plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행.md` | plan | 26101616 | 0 | 2294 | 0.31.0 · 2b · minicpm5 | — |
| 8 | `docs/benchmark/bench_report_26101621_minicpm5-2b_GB10_0.31.0.md` | bench_report | 26101621 | 0 | 3202 | 0.31.0 · 2b · minicpm5 | — |
| 9 | `docs/devlog/devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | devlog | 26101619 | 1 | 4789 | 0.31.0 · 2b · minicpm5 · minicpm5-2b-v031 · v031 | — |
| 10 | `docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md` | bench_report | 26101616 | 1 | 6903 | 0.31.0 · 2b · easy-vllm:0.31.0-cu133-aarch64-source · minicpm5 | docs/benchmark/bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0.md · docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md |
| 11 | `docs/plan/plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | plan | 26101614 | 1 | 16488 | 0.31.0 · 2b · minicpm5 | docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md |
| 12 | `docs/benchmark/bench_report_26101619_minicpm5-2b_GB10_0.31.0.md` (모호 해소) | bench_report | 26101619 | 1 | 3202 | 0.31.0 · 2b · minicpm5 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**(1) 목표.** 계보의 출발은 vLLM 0.31.0 bump 와 14B 이하 텍스트 모델 4종의 양 노드 상주 서빙이다. 메인 노드에는 Granite-4.2-8B 와 이 셀의 MiniCPM5-2B 가 함께 올라갔고, 각 모델은 max-model-len 32768 · OpenAI 호환 API · 도구호출 · 추론 파서를 켠 채 "DGX Spark 64GB 1대" 를 타깃으로 시뮬레이션했다. 그 plan 의 벤치는 lite 뿐이었다.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1
> 3. NAS 에서 최근 다운로드한 **14B 이하 텍스트 생성 모델 4종**을 노드당 2종씩 **상주 서빙**한다 —
>    max-model-len **32768 고정**, LangGraph RAG 용으로 **OpenAI 호환 API + 도구호출(auto tool choice) + 추론 파서** 활성.
> 4. 각 모델은 **DGX Spark 64GB(OEM 전용 구성) 1대**를 타깃 GPU 로 시뮬레이션한다 — 노드 128GB 를 64/64 로 나눈다.
> 5. 벤치는 **lite 만**(full ✗). 종료 시 4개 서빙은 **내리지 않는다**.

이 셀은 그 뒤 계보(plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리)에서 왔다 — 같은 plan 이 lite 정책을 개정했다: lite 는 측정을 그대로 두고(random 512/128 · 1회 · 데이터셋 knob ✗) hint 쪽만 바꿔 lite 셀 태그는 `03-benchmark.md` 를 싣지 않고 판정을 `lite(기동 가능)` 로 full 과 분리한다(U14). 이 셀은 그 lite 등급 hint 경로를 실셀로 검증하려고 같은 config 의 CNN full 셀과 별도로 둔 lite-only 셀이다(testlog_26101621_36_59 §2). 서빙 설정은 이 두 번째 계보에서 바뀌지 않았다.

**(2) HW 출발 조건.** NVIDIA GB10 · 노드당 GPU 1 · 이 셀은 메인 노드 1대(single · TP=1 · 분산 없음) · 통합메모리 · 인터커넥트 해당 없음. 노드 메모리 총량은 블랙박스 예산 선언 행의 `mem_total_mib=124608`(선언 행에 실린 값 · 01 §1.4)이고 셀 config 는 `test_device_total_gib` 121.7 을 적는다. 호스트 층은 plan_26101410 머리 유효맥락의 "aarch64 · CUDA 13.2 드라이버 580.173.02" 다(이미지 층 CUDA 13.3.1.008 과 다른 층). 사람 결정 U3 이 모델당 메모리를 64GB Spark 1대 천장(≈47.7GiB)으로 묶었다.

**(3) 모델 구조.** 체크포인트 `openbmb/MiniCPM5-2B`(revision `f97400052a43d642bbc6e9975e2397e3ae6a6b52` · 00 §0.4) · `LlamaForCausalLM` · 42층 · KV 헤드 2 · bf16 비양자화(체크포인트 config 에 quantization_config 부재 — 00 §0.2) · PLE 키 없음(00 §0.9 후보 표 출처) · spec decode 미사용(엔진 config `speculative_config=None`). 체크포인트 크기는 엔진 로그가 4.69 GiB 로 기록하고(lite_engine_head L55 `Checkpoint size: 4.69 GiB`) plan_26101410 §모델 4종의 ≈4.4GiB 는 config 판독 산식이다. L62 의 4.75 GiB 는 크기가 아니라 로드 뒤 메모리 증가분이다(파일시스템 du 는 쓰지 않았다). 토큰당 KV 42KiB · 32k 1시퀀스 1.3GiB(같은 표 · 산식). 네이티브 컨텍스트는 체크포인트 config.json 의 `max_position_embeddings` 131072 다(같은 파일 `head_dim` 128 · 서빙 max-model-len 32768 은 그보다 짧게 고정한 값). 총 파라미터 수는 config 에 직접 키가 없다 — 체크포인트 `model.safetensors.index.json` 의 metadata.total_size 5033512960 B 를 bf16 2 B 로 나누면 ≈2.52B(산식)이고, 체크포인트 README L47 은 "It is a dense 2B Transformer" 로 선언한다(모델 카드 선언).

**(4) stock 에서 예상 · 관측된 장벽.** 빌드는 stock(빌드 패치 0)으로 계획됐고, 0.30.0→0.31.0 버전 델타 판정이 A축 UNDETERMINED 라 시도-빌드 출구를 사람이 승인했다(plan_26101410 §4 D1). 서빙 쪽 장벽은 관측되지 않았다 — 이 셀의 벽은 두 번째 계보의 측정 · 발행 하네스에서 나왔다(2.2).

**(5) 외부 선례와 사람의 결정.** 이 모델의 외부 서빙 선례는 계보에 없다. 사람 결정은 모델 선정(U1) · vLLM 0.31.0(U2) · 64GB 천장(U3) · 동시성 요구 8(D5)이고, 두 번째 계보에서는 lite 정책 정의(U14 — full 없이 hint 를 발행할 최소 기준점 · 이 HW 에서 LLM 사용 가능 최소조건 증명)와 이름 규약 불변 · 겹치면 발행 시각 꼬리(U9)를 골랐다. 포크 · 자체이식 경로는 쓰지 않았다(stock).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T12:40:00Z(`camp-26101621-minicpm-lite` · evidence.event_campaign_boundary): campaigns/camp-26101621-minicpm-lite/campaign.yaml declared_utc 2026-10-16T12:40:00Z → 경계 = 앞선 값 2026-10-16T12:40:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T08:04:04Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) — main(마지막 행 2026-10-16T08:04:04Z < 측정 끝 2026-10-16T12:38:49Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

_이번 캠페인(경계 2026-10-16T12:40:00Z 이후)의 기동 시도 미관측 — 아래는 경계 이전 행뿐이다._

**캠페인 경계 이전 기동 2회**(캠페인 경계 2026-10-16T12:40:00Z 이전 — 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 2회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 1 |
| 앞선-2 | main | `serve-main-combined-granite-minicpm`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-14T02:04:02Z | 51회 · 첫 2026-10-14T02:04:03Z | 없음 | —(닫힘 행 없음) | 1 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

이 셀의 계보에서 넘은 벽은 둘이고, 둘 다 서빙이 아니라 **측정 · 발행 하네스**의 결함이다. 서빙 자체는 2026-10-14 에 합산 예산 선언 창(`serve-main-combined-granite-minicpm` · 갱신 51회 · 사살 · 트립 없음)으로 섰고, 위 표가 이 셀의 기동 시도를 0 으로 세는 첫 사유는 캠페인 경계(2026-10-16T12:40:00Z)다 — 10-14 기동은 경계 이전이라 라벨이 같았어도 이 셀의 시도로 세지 않으며, 합산 라벨은 그다음 사유다(01 §1.4 (c)). 그 상주 세션에서 lite 는 여러 번 돌았다 — 앞선 캠페인의 lite(10-14 02:07:53Z · warm 38.74 t/s — 이 계보 밖 testlog_26101412 §3) · CNN full 스윕의 lite 레그(10-16 06:44Z 등 — devlog_26101620 §이 셀의 여정) · 그리고 이 lite-only 셀의 두 런이다. 이 셀의 10:45Z 첫 런은 산출물 위치 때문에 발행 증거가 되지 못했고(W2), 12:38Z 두 번째 런이 이 페이로드의 측정이다.

### W1 lite 게이트 measurement_path_failed — 엔진 로그 꼬리만 파싱 (같은 서빙의 CNN full 스윕 lite 레그 · 06:44Z)

이 벽은 이 lite-only 셀의 런이 아니라, 같은 서빙에 같은 lite 도구를 먼저 돌린 CNN full 스윕의 lite 레그에서 관측됐다. 요청은 전부 성공했는데(warm 38.88 t/s) KV 지표만 N/A 로 남아 lite 판정이 `measurement_path_failed` 가 됐다. 원인은 lite_metrics 가 엔진 로그를 꼬리(tail -800)로만 파싱한 것이다 — KV 예약 줄은 기동 때 한 번만 찍히는데(lite_engine_head L74 · 10-14 02:05:10) 서빙이 2일 상주하는 동안 꼬리 창 밖으로 밀려났다. 해소는 커밋 0f8800c(머리 캡처를 먼저 읽고 비면 꼬리로 메움)이고, 이 셀의 두 lite 런은 모두 그 교정 뒤에 돌았다(devlog_26101621 §여정 2). 이 셀이 교정 없이 돌았다면 같은 판정이 났을 것이라는 것은 같은 서빙 · 같은 도구라서 나오는 **추론**이다. 통과는 lite_metrics 가 이 셀 엔진 로그 머리(L74)에서 파싱해 경량 리포트(KV cache 점유)와 lite 인증서(`lite_kv_gib`)에 실은 KV 35.0 GiB(testlog_26101621_36_59 §1 "엔진 로그 머리 · 0f8800c 교정 뒤 파싱")와 lite_verdict pass 로 확인했다 — lite raw 자체에는 KV 키가 없고 엔진 로그 경로만 있다.

> [원문] devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정
> 1. **1차 스윕(06:44Z)** — lite 게이트가 `measurement_path_failed` 로 막아 GuideLLM 에 들어가지 못했다. 요청은 전부 성공했고(warm 38.88 t/s)
>    KV 만 N/A 였다. lite_metrics 가 엔진 로그 꼬리(tail -800)만 파싱해 상주 2일 서빙의 기동 KV 라인을 못 봤다 → 머리 우선 파싱(0f8800c) ·
>    재판정 pass(KV 35.0 GiB).

```hint-event
id: W1
kind: wall
증상: 같은 서빙의 CNN full 스윕 lite 레그에서 요청은 전부 성공했으나 KV 지표가 N/A 로 남아 lite 판정이 measurement_path_failed 가 됐다
서명: "lite 게이트가 `measurement_path_failed` 로 막아 GuideLLM 에 들어가지 못했다"
원인: lite_metrics 가 엔진 로그 꼬리(tail -800)만 파싱해 2일 상주 서빙에서 기동 때 한 번 찍힌 KV 예약 줄을 보지 못했다(하네스 결함 · 서빙은 정상)
해소: 커밋 0f8800c — KV · 가중치 파싱이 엔진 로그 머리 캡처를 먼저 읽고 비면 꼬리로 메운다 · 이 셀의 두 lite 런은 교정 뒤에 돌았다
검증: testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §1 KV cache 점유 35.0 GiB(엔진 로그 머리 · 0f8800c 교정 뒤 파싱) · lite pass
전이등급: arch-invariant
출처: [devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정, devlog_26101621_lite셀_minicpm5_2b_v031 §여정, testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §1, output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_engine_head_minicpm5-2b-v031.log]
```

### W2 첫 lite 런의 산출물이 규약 경로 밖 — hint 발행기가 조인하지 못함 (10:45Z → 12:38Z 재측정)

첫 lite 런은 `--out-dir` 로 산출물을 규약 경로(`lite_publish/<셀>_<UTC>/`) 밖에 두었고, 경량 리포트 · lite 인증서까지 발행됐지만(bench_report_26101619 · benchmark_26101619) hint 발행기가 lite raw 를 찾지 못해 발행 자격을 관측하지 못했다(`HINT_QUALIFICATION_UNOBSERVED`). testlog 는 옛 raw 가 측정 파일을 옛 디렉터리 이름이 박힌 저장소 상대경로로 가리켜 폴더를 옮기면 참조가 끊기므로 이동 대신 규약 경로로 재측정했다고 적는다(재측정본 raw 는 절대경로를 쓴다). 캠페인 포인터는 옛 산출물을 `--evidence-untag`(사유 기록)로 해제하고 새 산출물로 등록했다. 통과는 위 등급 블록의 발행 자격 근거가 규약 경로의 lite raw · warm JSON 을 가리키는 것으로 판정된다.

> [원문] testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §3
> 첫 런(10:45:28Z · `--out-dir output/single/benchlog/lite_minicpm5-2b-v031_26101616` · warm 39.22 t/s · 리포트 `bench_report_26101619_minicpm5-2b` ·
> 인증서 `benchmark_26101619_minicpm5-2b`)은 lite raw 를 β 규약 경로 밖에 두어 hint 발행기가 조인하지 못했다(`HINT_QUALIFICATION_UNOBSERVED` ·
> `lite_raw_minicpm5-2b-v031.json 부재(조인 키 없음)`). raw 가 측정 파일을 옛 디렉터리 이름이 박힌 저장소 상대경로(`output/single/benchlog/lite_minicpm5-2b-v031_26101616/…`)로 가리켜 폴더를 옮기면 참조가 끊기므로 이동 대신 규약 경로로 재측정했고(재측정본 raw 는 절대경로를 쓴다), 캠페인 포인터는
> 옛 산출물을 `--evidence-untag`(사유 기록)로 해제하고 새 산출물로 등록했다. 두 런의 수치는 거의 같다(39.22 / 39.20 t/s).

```hint-event
id: W2
kind: wall
증상: 첫 lite 런(10:45Z)은 리포트 · 인증서까지 냈으나 hint 발행기가 발행 자격을 관측하지 못했다
서명: "lite raw 를 β 규약 경로 밖에 두어 hint 발행기가 조인하지 못했다"
원인: 첫 런이 --out-dir 로 산출물을 규약 경로(lite_publish/<셀>_<UTC>/) 밖에 두어 발행기의 조인 키(lite raw)가 없었다
해소: 규약 경로로 재측정(12:38Z · --publish-report) · 옛 산출물은 --evidence-untag 로 캠페인 포인터에서 해제하고 새 산출물을 등록
검증: 00 등급 블록 발행 자격 근거 = output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_raw_minicpm5-2b-v031.json
전이등급: arch-invariant
출처: [testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §3, output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_raw_minicpm5-2b-v031.json, output/single/benchlog/lite_minicpm5-2b-v031_26101616/lite_raw_minicpm5-2b-v031.json]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 첫 lite 런(10:45Z · --out-dir · warm 39.22 t/s)의 경량 리포트 · lite 인증서를 이 셀의 hint 증거로 쓰기
기각사유: 산출물이 규약 경로 밖이라 발행기가 조인하지 못했다(HINT_QUALIFICATION_UNOBSERVED) — 캠페인 포인터에서 --evidence-untag 로 해제
출처: [testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §3, bench_report_26101619_minicpm5-2b_GB10_0.31.0]
```

```hint-event
id: R2
kind: rejected
시도: 첫 런의 산출물 폴더를 규약 경로로 옮겨 조인시키기
기각사유: 옛 raw 가 측정 파일을 옛 디렉터리 이름이 박힌 저장소 상대경로로 가리켜 폴더를 옮기면 참조가 끊기므로 규약 경로로 재측정했다(testlog 의 판단 — 이동을 실제로 시도한 기록은 없다)
출처: [testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §3]
```

이 셀의 **서빙** 형상에서 기각된 시도는 없다 — 서빙 설정은 앞선 캠페인 값 그대로이고 이 계보는 그것을 바꿔 보지 않았다(plan_26101410 · testlog_26101621_36_59 §3). lite 의 측정 형상도 바꾸지 않았다(random 512/128 · 1회 · 데이터셋 knob ✗ — plan_26101614 §2 U14). 이 셀을 좁힌 것은 발행 증거의 위치뿐이다(R1 · R2).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

**M1 — "측정 경로 불성립" 판정.** 같은 서빙의 CNN full 스윕 lite 레그에서 lite 판정은 `measurement_path_failed` 였지만, 실제로는 요청이 전부 성공했고 KV 한 지표만 파서가 못 읽었다. 판정이 서빙이나 측정 경로가 아니라 **파서의 읽기 창**을 가리키고 있었다. 정정은 커밋 0f8800c 와 재판정 pass 다(W1). 이 셀의 런은 교정 뒤라 이 오진을 직접 겪지 않았다.

> [원문] devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3
> | D4 | lite KV 파싱이 엔진 로그 꼬리만 읽어 상주 서빙에서 늘 measurement_path_failed | P7 1차 | 0f8800c(머리 우선) |

```hint-event
id: M1
kind: misdiagnosis
증상: 같은 서빙의 CNN full 스윕 lite 레그에서 lite 판정이 measurement_path_failed 로 나왔다
서명: "lite KV 파싱이 엔진 로그 꼬리만 읽어 상주 서빙에서 늘 measurement_path_failed"
원인: 기동 때 한 번만 찍히는 KV 줄을 꼬리 파싱이 놓쳐 지표 하나가 N/A 가 된 것을 측정 경로 실패로 분류했다
해소: 측정 경로는 성립했다(요청 전부 성공 · warm 38.88 t/s) — 머리 우선 파싱 뒤 재판정 pass(KV 35.0 GiB)
검증: devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 · 커밋 0f8800c
전이등급: arch-invariant
현재지위: 반증
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정]
```

교정 묶음 — devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 의 D1~D9(9항목)을 훑었다. 이 셀의 판정 · 기록을 바꾼 항목은 하나다:

- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D4 — lite KV 파싱 — 꼬리만(같은 서빙에서 measurement_path_failed) → 머리 우선(이 셀 KV 35.0 GiB · pass)

나머지 D1~D3 · D5~D9 는 GuideLLM 데이터셋 배선 · 문서 · 배달 사정이다 — lite 는 데이터셋 knob 를 쓰지 않으므로(U14) 이 셀의 측정에 닿지 않는다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 절대클램프(V4)** — 처음 값이 이 계보에서 바뀐 적이 없다: `kv-cache-memory-bytes 37580963840`(35.0 GiB)은 plan_26101410 D4 의 절대 클램프 방침으로 앞선 캠페인에서 정해졌고, 이 셀은 그 서빙에 lite 를 보냈다. 필요량은 plan 의 산식(토큰당 42KiB · 32k 1시퀀스 1.3GiB)이고, 엔진은 그 클램프에서 873,808 토큰 · 32768 기준 최대 동시성 26.67x 를 보고했다 — max-num-seqs 8 로 쓰는 몫보다 훨씬 크다. 그렇게 크게 둔 이유는 계보 표 안에는 없다 — 이 계보 밖 문서(devlog_26101412 · testlog_26101412)가 KV 를 천장 − 가중치 − 오버헤드로 손 산정하고 사용자의 "64/64 할당" 의도에 맞춰 max_safe 를 택했다(kv_source=hand)고 기록한다. 음성대조라는 기록은 없다.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §모델 4종
> | 메인 | MiniCPM5-2B | `LlamaForCausalLM` · 42층 kv2 | ≈4.4GiB | 42KiB | 1.3GiB | `minicpm5xml`(모델카드: XML `<function name=…>`) · `<think>` |

> [원문] output/single/benchlog/lite_publish/minicpm5-2b-v031_20261016T123814Z/lite_engine_head_minicpm5-2b-v031.log §L75-75
> (EngineCore pid=343) INFO 10-14 02:05:10 [kv_cache_utils.py:2464] GPU KV cache size: 873,808 tokens, Maximum concurrency for 32,768 tokens per request: 26.67x

**호스트 메모리 예산** — 원장의 합산 선언은 `floor_mib=23966` = 총량 124608 − 가중치 4800 − KV 35840 − overhead 60002 꼴이다(모두 **선언값** · 01 §1.4 원장 행 4). overhead 60002 는 D6 처방대로 granite 기동 뒤 실측 used 와 이 모델의 컨테이너 오버헤드를 합친 값이라 이 모델 혼자의 overhead 가 아니다. 엔진 실측은 로드 메모리 4.75 GiB 다(lite_engine_head L62). KV 35.0 GiB(L74)는 선언 클램프 37580963840 B 를 그대로 예약했다는 엔진 보고이지 선언과 독립된 측정이 아니다.

**gpu-memory-utilization(V1)** — 0.39 는 D4 의 "천장 ÷ 호스트 총량 ≈0.39" 환산값(사람 결정)이고 이 계보에서 바뀌지 않았다. KV 를 클램프로 지정했으므로 엔진은 이 값으로 KV 를 산정하지 않는다(엔진 로그 L74).

**max-model-len(V2) · max-num-seqs(V3)** — 32768 은 사용자 지시 고정, 8 은 D5 의 동시성 요구 8 그대로다. lite 는 512/128 입력을 동시성 1 로 보내 두 값 모두 한계까지 시험하지 않았다.

**eager/graph** — 엔진 기본 cudagraph(FULL_AND_PIECEWISE · capture sizes 1~16)를 그대로 썼다. 엔진 로그는 그래프 캡처가 33 초 · 0.23 GiB 를 썼다고 적는다(lite_engine_head L86). eager 를 대조한 기록은 없다 — graph 를 고른 이유가 실측 비교에서 나온 것이 아니라 엔진 기본값이다.

**lite 측정 형상** — random 입력 512 · 출력 128 · range-ratio 0 · ignore-eos · 동시성 1 · cold 1 요청(warmup 0) + warm 3 요청(warmup 1)이며 temperature 인자는 도구 원문에 없다(아래 원문). 이 형상은 사람 결정 U14 가 고정한 것이고 이 셀이 고른 값이 아니다:

> [원문] lite_bench.sh@803064d23d51 §L172-176
>     docker exec "$CTR" bash -lc "cd /tmp && vllm bench serve \
>       --backend $BACKEND --base-url http://localhost:$INPORT --endpoint $LITE_ENDPOINT \
>       --model '$MODEL_NAME' --tokenizer '$MODEL_PATH' --trust-remote-code \
>       --dataset-name random --random-input-len 512 --random-output-len 128 --random-range-ratio 0 \
>       --num-prompts $2 --max-concurrency 1 --request-rate inf --ignore-eos --num-warmups $3 \

**이 형상이 버티는 기전 대 다른 형상** — 이 계보는 서빙 값(KV · gmu · 동시 시퀀스 · 배치 토큰)이나 형상(PLE · offload)을 바꿔 시도하지 않았으므로 "열렸다" 고 말할 비교가 없다 — 지위 해당 없음. 같은 lite 를 두 번 돌린 차이는 산출물 위치뿐이고 수치는 warm 39.22 / 39.20 t/s 로 거의 같다(testlog_26101621_36_59 §3).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- lite 수치를 성능 baseline · 권고로 쓰지 않는다 — 00 등급 블록(OBSERVATION-ONLY · 성능 판정 아님) — 성능이 필요하면 full 측정을 한다.
- lite 수치를 같은 config 의 CNN full 셀 수치와 나란히 놓지 않는다 — plan_26101614 §2 U14(lite random 512/128 · 1회) 대 U16 · P7 · testlog_26101619_54_16 §2(CNN 1024/256 · 동시성 1·2·4·8 · 반복 3) — 같은 형상끼리만 비교한다.
- 오래 상주한 서빙의 lite 판정을 엔진 로그 꼬리 파싱 판본으로 믿지 않는다 — W1 · M1 — 머리 우선 파싱 판본(커밋 0f8800c 이후)인지 확인하고, `measurement_path_failed` 면 요청 성공 여부부터 본다.
- hint 증거로 쓸 lite 를 `--out-dir` 로 규약 경로 밖에 두지 않는다 — W2 · R1 — `--publish-report` 기본 경로(`lite_publish/<셀>_<UTC>/`)에 둔다.
- 경로가 틀린 산출물을 폴더 이동으로 살리려 하지 않는다 — R2(옛 raw 가 옛 디렉터리 이름의 상대경로로 측정 파일을 가리킴) — 규약 경로로 재측정한다.
- 엔진 자기보고 `0.31.1.dev0+…` 나 lite 인증서의 `vllm_version`(생산자 lite-report-identity — IMAGE_TAG 에서 자른 값)을 빌드 입력으로 쓰지 않는다 — 00 §0.4 — `v0.31.0` 또는 40자 SHA 로 재현한다.
- 합산 예산 선언의 overhead(60002 MiB)를 단독 서빙에 옮기지 않는다 — plan_26101410 §4 D6 — 네 노드에서 다시 잰다.
- KV 35.0 GiB 를 필요조건으로 복사하지 않는다 — V4(승계 · 필요량보다 큼) — 토큰당 KV × 컨텍스트 × 동시성으로 다시 도출한다.
- 실린 Dockerfile 을 측정 이미지의 바이트로 믿지 않는다 — 01 §1.1(history 16/17) — docker history build-arg 와 대조한다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 이 서빙은 동시성 1 · 짧은 입력 밖에서도 버티는가 — lite 셀만으로는 답이 없다
현재상태: lite 는 cold 1 + warm 3 요청 · 동시성 1 · random 512/128 만 보냈고 pass 했다(warm 39.2 t/s) · 부하 거동은 이 태그 밖(같은 config 의 full 셀)의 다른 형상 측정이 말한다
다음관측: 이 형상 그대로 동시성을 올린 full 측정(GuideLLM × 반복 ≥3)
출처: [testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §1, plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2]
```

```hint-event
id: Q2
kind: open-question
물음: 같은 노드에 granite 서빙이 메모리에 상주한 것만으로 lite 결과가 달라지는가
현재상태: 측정(12:38Z) 때 granite(8001)도 같은 노드에 떠 있었으나 이 저작의 컨테이너 로그 확인(12:30~12:45Z · 계보 밖 관측)으로 요청 0건이었다 · 남는 것은 메모리 상주(노드 RAM 96.0 GiB 에 포함)이고 단독 상태와의 대조는 없다
다음관측: granite 를 내린 단독 상태에서 같은 lite 를 다시 돌려 비교
출처: [testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §1, plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3]
```

lite 등급 셀이라 현행 full 정의 충족 여부는 물음이 아니다(full 측정이 아니다). 상위 vLLM 에서의 자체 패치 필요성은 이 셀에 패치가 0 이라 해당하지 않는다.
