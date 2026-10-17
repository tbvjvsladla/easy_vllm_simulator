# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 13건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite.md` | plan | 26101410 | 0 | 10727 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | — |
| 2 | `docs/testlog/testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정.md` | testlog | 26101619 | 0 | 4032 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | docs/testlog/testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정.md |
| 3 | `docs/devlog/devlog_26101707_Q6_granite42_8b_v031_CNN_재측정.md` | devlog | 26101707 | 0 | 952 | 0.31.0 · 8b · granite42 · granite42-8b-v031 · v031 | — |
| 4 | `docs/testlog/testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정.md` | testlog | 26101707 | 0 | 3149 | 0.31.0 · 8b · granite42 · granite42-8b-v031 · v031 | — |
| 5 | `docs/benchmark/bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md` | bench_report | 26101616 | 0 | 6521 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 6 | `docs/benchmark/bench_report_26101707_granite-4.2-8b_GB10_0.31.0.md` | bench_report | 26101707 | 0 | 6510 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 7 | `docs/plan/plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행.md` | plan | 26101616 | 0 | 2294 | 0.31.0 · 8b | — |
| 8 | `docs/devlog/devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | devlog | 26101619 | 0 | 5118 | 0.31.0 | — |
| 9 | `docs/plan/plan_26101622_후속4건_안정화_데이터셋knob_lite_hint_기준선.md` | plan | 26101622 | 0 | 9601 | 0.31.0 | — |
| 10 | `docs/testlog/testlog_26101622_plan_26101614_종결판정_데이터셋knob_CNN_lite분리.md` | testlog | 26101622 | 1 | 6182 | 0.31.0 · 8b · granite42 · granite42-8b-v031 · v031 | — |
| 11 | `docs/plan/plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | plan | 26101614 | 1 | 16488 | 0.31.0 · 8b | docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md |
| 12 | `docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md` | testlog | 26101619 | 2 | 2895 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | — |
| 13 | `docs/testlog/testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정.md` (모호 해소) | testlog | 26101622 | 1 | 5774 | 0.31.0 · 8b | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

이 계보는 세 겹이다. 날짜가 가장 이른 plan_26101410(2026-10-14)은 vLLM 을 0.31.0 으로 올리고 NAS 의 14B 이하 텍스트 모델 4종을 두 노드에 둘씩 **상주 서빙**하는 것이 목표였다 — 메인 노드에 Granite-4.2-8B(포트 8001)와 MiniCPM5-2B(포트 8002), 컨텍스트 32768 고정, LangGraph RAG 용 OpenAI 호환 API + 도구호출 + 추론 파서, 벤치는 lite 만.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1
> 3. NAS 에서 최근 다운로드한 **14B 이하 텍스트 생성 모델 4종**을 노드당 2종씩 **상주 서빙**한다 —
>    max-model-len **32768 고정**, LangGraph RAG 용으로 **OpenAI 호환 API + 도구호출(auto tool choice) + 추론 파서** 활성.
> 4. 각 모델은 **DGX Spark 64GB(OEM 전용 구성) 1대**를 타깃 GPU 로 시뮬레이션한다 — 노드 128GB 를 64/64 로 나눈다.

plan_26101614(2026-10-16)는 서빙은 손대지 않고, 상주 중인 같은 서빙에 GuideLLM full-bench 를 실텍스트(CNN/DailyMail) 입력으로 거는 것이 목표였다 — 이 셀의 앞선 측정(10-16 · c=1 decode 12.46 t/s)이 그 결과다.

> [원문] plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §1
> 1. full-bench(GuideLLM)의 `--data kind=synthetic_text` 고정을 풀고 **데이터셋을 프리셋 knob 로 편입**한다.

이 페이로드의 수치를 만든 것은 그 뒤 겹이다 — 앞선 캠페인 종결 testlog 가 남긴 후속(설명되지 않은 prefix cache hit rate · 서브 hint 의 셀 입력 결손)을 계보 표 밖 plan_26101622(2026-10-16 승인 · testlog_26101707 머리가 가리킨다)가 F2 · F1 로 받아 조사 → 하네스 교정 → 검증 캠페인 `camp-26101623-baseline` 에서 이 셀을 같은 서빙 · 같은 프리셋으로 다시 쟀다.

> [원문] testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §1
> - 12 런 모두 `requests_delta = 18`(이 런의 요청만 · 다른 트래픽 혼입 0) · 행은 offset 2727 + 0..215 연속 · 런끼리 겹치지 않음.

**HW 출발 조건** — GB10 ×1 · 노드 1대(single · TP=1 · 분산 없음) · aarch64 통합메모리. 노드 메모리 총량은 블랙박스 원장 `budget_declare` 가 `mem_total_mib=124608` 로 적는다(01 §1.4). 드라이버 580.173.02 · 호스트 CUDA 13.2 는 plan_26101410 머리 유효맥락의 값이다(호스트 층 · 이미지 CUDA 13.3.1 과 다른 층). 사람은 모델당 64GB Spark 1대를 시뮬하기로 정했다(plan_26101410 §2 U3 — 가용 약 59.6GiB × target_gmu 0.80 ≈ 47.7GiB 천장). 대역폭은 plan_26101410 §3 이 웹 출처로 273 GB/s 라 적는다. 인터커넥트는 단일 노드라 해당 없다. 측정 동안 같은 노드에 MiniCPM5-2B(:8002)가 함께 상주했다(testlog_26101622_47_43 머리 유효맥락).

**모델 구조** — HF `ibm-granite/granite-4.2-8b`(revision `41a8a2d41c54ef4a71741b3e62604f4caaec9295` · base `granite-4.1-8b-base` · 00 §0.2). plan_26101410 §3 의 config 판독표가 `GraniteForCausalLM` · 40층 GQA kv8 · bf16 가중치 약 15.4GiB · 토큰당 KV 160KiB · 32k 1시퀀스 KV 5.0GiB 를 적는다(config 에서 계산한 값 · 측정 아님). 가중치 크기의 측정 쪽 근거는 엔진이 보고한 체크포인트 크기 16.38 GiB(엔진 로그 L55)와 적재 메모리 16.42 GiB(L65)다 — plan 의 약 15.4GiB 는 config 판독 추정값이고 엔진 보고와 약 1GiB 어긋난다(어긋난 이유는 계보에 없다). 양자화 없음(bf16 단일 · 00 §0.4) · speculative_config 없음(엔진 config L48) · PLE 없음. 서빙 컨텍스트 32768 은 사용자 요구다.

**stock 상태의 장벽** — 계보 표 12건 안에는 서빙 쪽 장벽 기록이 없다(빌드 패치 0 의 stock 소스빌드 · plan_26101410 §4 D2). 앞선 캠페인의 서빙 쪽 사건(언클램프 KV 트라이얼의 워치독 사살 등)은 계보 표 밖 testlog_26101412 에 있어 이 페이로드의 W 블록으로 세지 않았다. 이 계보에서 넘은 벽은 전부 **측정 경로**다 — 10-16 캠페인의 GuideLLM 데이터 인자 스키마(W1) · lite 게이트 파서(W2) · 열 매핑 CLI 분할(W3) · 기본 열 매핑 교체(W4), 그리고 이번 재측정의 엔진 로그 범위(W5) · prefix cache 재적중(W6).

**선례와 사람의 결정** — 이 모델 · 이 HW 의 외부 커뮤니티 구현이나 수치는 계보에 없다. 사람이 고른 경로는 결정 표에 있다: 메모리는 64GB Spark 시뮬 · gmu 는 호스트 환산 · KV 는 절대 클램프 · 동시성 요구 8(plan_26101410 §4 D4 · D5), 데이터셋은 입력만 실텍스트 · 출력 ignore_eos 고정 · 판정기 불변(plan_26101614 §2 U1) · CNN/DailyMail `article` 원문 · 입력 1024 / 출력 256(U16) · hint 이름 불변(U9). 이번 겹에서는 Q1 조사 뒤 처방 ⓑ(실행 차분)+ⓒ(실행 범위 로그)를 고른 사람 결정(testlog_26101622_47_43 §5)과 1차 재측정 뒤의 사람 결정 "스윕별 행 오프셋 + 재측정"(testlog_26101707 §2)이 이 측정의 형상을 정했다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T14:03:30Z(`camp-26101623-baseline` · evidence.event_campaign_boundary): campaigns/camp-26101623-baseline/campaign.yaml declared_utc 2026-10-16T14:03:30Z → 경계 = 앞선 값 2026-10-16T14:03:30Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T16:42:14Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`) — main(마지막 행 2026-10-16T16:42:14Z < 측정 끝 2026-10-16T22:23:36Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

_이번 캠페인(경계 2026-10-16T14:03:30Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T14:03:30Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031` | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 2 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

벽 W1~W4 는 10-16 캠페인(앞선 측정)의 측정 경로 벽이고 devlog §3 표의 항목을 W1 = D1 · W2 = D4 · W3 = D2 · W4 = D3 으로 배치했다 — W1(P2 오프라인 spike)이 W2(P7 1차 스윕)보다 먼저인 것은 plan 단계 번호로 확정이고, W3 · W4 가 W2 의 앞인지 뒤인지는 계보에 기록이 없다. W5 · W6 은 이번 재측정 겹의 벽이고 시각순(Q1 조사 → ecb7dca → 1차 재측정 → 3fa6eac → 2차 스윕)이다. 위 기동 시도 표대로 어느 겹도 서빙을 다시 띄우지 않았다 — 벽은 모두 같은 상주 서빙 위에서 측정을 다시 건 것이다.

### W1 GuideLLM v0.7.3 의 데이터 인자 스키마가 문서와 다르다
데이터셋 knob 를 배선하기 전 고정 GuideLLM 이미지로 오프라인 전처리 spike(P2 · granite 토크나이저 · concatenate)를 돌렸을 때 드러났다 — seed 인자는 문서의 `seed=` 꼴이 아니라 `kind=static,value=N` 이고, 단일 parquet 는 `split` 지정 없이는 DatasetDict 로 읽혀 매퍼가 죽는다. 원인은 이미지 안 실제 스키마와 공개 문서의 불일치였고, 처분은 "고정 이미지의 실제 스키마로 배선" 이다(devlog §3 D1). 이번 측정의 결과 JSON config 도 그 꼴이다:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json §L1-1
> "data": [{"kind": "json_file", "load_kwargs": {"split": "train"}, "path": "/out/dataset_rows.jsonl"}]

```hint-event
id: W1
kind: wall
증상: P2 오프라인 spike 에서 GuideLLM 데이터 로더 · 매퍼가 문서대로 준 인자로 동작하지 않았다
서명: "GuideLLM `--seed` 는 `kind=static,value=N`(문서의 `seed=` ✗)"
원인: 고정 이미지(GuideLLM v0.7.3)의 실제 인자 스키마가 공개 문서와 달랐다 · 단일 parquet 는 split 지정 없이 DatasetDict 로 읽힌다
해소: 이미지의 실제 스키마로 배선(seed = kind=static · load_kwargs split=train)
검증: 측정 결과 JSON config 의 data load_kwargs split train 과 seed kind static value 42 기록
전이등급: arch-invariant
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json]
```

### W2 lite 게이트가 상주 서빙에서 KV 를 못 찾아 full 진입을 막았다
10-16 P7 1차 스윕에서 lite 레그 요청은 전부 성공했는데 lite 판정이 `measurement_path_failed`(KV N/A)로 끝나 GuideLLM 이 시작되지 않았다. 원인은 lite 지표 파서가 엔진 로그 **꼬리**만 읽은 것이다 — 기동 때 찍히는 KV 라인이 오래 상주한 서빙에서는 꼬리에 없다. 머리 캡처 파일은 이미 있었는데 KV 파싱에 배선되지 않았다:

> [원문] testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3
> | E1 | 1차 스윕: lite 게이트 `measurement_path_failed`(요청 전부 성공 · KV N/A) → GuideLLM 미진입 | lite_metrics 가 엔진 로그 **꼬리**만 파싱 — 상주 2일 서빙은 기동 KV 라인이 꼬리에 없다(머리 캡처는 있었으나 KV 파싱 미배선). 교정 0f8800c 후 재판정 pass · 재스윕(`p7_try1_lite_parser_defect.log`) |

측정 도구 원문도 두 캡처가 따로 있음을 보인다 — 꼬리 800줄과 머리 N줄(이 lite 원문은 이번 측정 판본에서도 같다 · 03 §3.4):

> [원문] lite_bench.sh@803064d23d51 §L196-198
> if [ "$SERVE_PLANE" = "docker" ]; then
>   docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true
>   docker logs "$CTR" 2>&1 | head -n "$ELOG_HEAD_N" > "$ELOG_HEAD" || true   # head 가 일찍 닫아 SIGPIPE — 실패로 읽지 않는다

해소는 커밋 0f8800c(lite 파서가 머리를 먼저 읽고 비면 꼬리로 메움)이고 재판정이 pass 로 바뀌었다(devlog §3 D4). 이번 측정의 lite 판정도 pass 다(lite raw). 이 벽은 서빙 결함이 아니라 하네스 결함이라 재빌드 · 서빙 재기동 없이 넘었다.

```hint-event
id: W2
kind: wall
증상: lite 요청은 전부 성공했는데 lite 판정 measurement_path_failed(KV N/A)로 GuideLLM full 스윕이 시작되지 않았다
서명: "1차 스윕: lite 게이트 `measurement_path_failed`(요청 전부 성공 · KV N/A) → GuideLLM 미진입"
원인: lite 지표 파서가 엔진 로그 꼬리(tail -800)만 파싱했고 오래 상주한 서빙의 기동 KV 라인은 꼬리에 없었다 · 머리 캡처는 있었으나 KV 파싱에 배선되지 않은 침묵 누락
해소: 커밋 0f8800c — lite 파서가 엔진 로그 머리를 먼저 읽고 비면 꼬리로 메운다 · 재빌드와 서빙 재기동 없음
검증: 재판정 pass · 머리 로그의 KV 예약 줄(L77-78) · 이번 측정 lite 판정 pass
전이등급: arch-invariant
출처: [testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3, devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, lite_bench.sh@803064d23d51, output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log]
```

### W3 열 매핑을 CLI 문자열로 주면 쉼표에서 쪼개져 깨진다
GuideLLM 의 `kind=..,column_mappings={..}` 꼴 인자는 CLI 가 쉼표에서 쪼개 다중 키 dict 를 깨뜨린다 — 10-16 granite 동시성 4 라이브 실행 1회가 실패했다(devlog §3 D2). 처분은 인자를 JSON 객체 한 덩이로 넘기는 것이다. 그 실패 런의 로그는 계보에 남아 있지 않아 오류 서명은 devlog 의 기술로 대신한다. 통과는 결과 JSON 의 `column_mappings` 가 세 키를 온전히 든 것으로 확인된다(아래 W4 발췌).

```hint-event
id: W3
kind: wall
증상: granite 동시성 4 의 GuideLLM 라이브 실행 1회가 데이터 열 매핑 인자 해석에서 실패했다
서명: "`kind=..,column_mappings={..}` 는 CLI 가 쉼표에서 쪼개 다중 키 dict 가 깨진다"
원인: GuideLLM CLI 가 kind=…,key=value 꼴 인자를 쉼표로 분할해 dict 값 안의 쉼표까지 잘랐다
해소: 열 매핑 인자를 JSON 객체 한 덩이로 넘긴다
검증: 측정 결과 JSON config 의 column_mappings 가 세 키를 온전히 기록 · 전 레벨 완료 16 실패 0
전이등급: arch-invariant
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json]
```

### W4 열 매핑을 주면 기본 매핑 전체가 교체돼 출력 길이가 풀린다
이 벽은 실패로 맞기 전에 이미지 소스 판독으로 잡았다(devlog §3 D3 — 이 계보에 이 함정이 실제로 발화한 런의 기록은 없다). `column_mappings` 를 주면 GuideLLM 의 기본 매핑이 통째로 바뀌어, 출력 길이 열이 빠지면 요청에서 max_tokens 가 사라지고 ignore_eos 때문에 컨텍스트 끝까지 생성한다. 처분은 실행 시점에 본문 · 입력 토큰 수 · 출력 토큰 수 세 열을 모두 명시하는 것이다:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json §L1-1
> "data_column_mapper": {"kind": "generative_column_mapper", "column_mappings": {"text_column": "article", "prompt_tokens_count_column": "prompt_tokens_count", "output_tokens_count_column": "output_tokens_count"}}

통과 술어는 실측 출력 토큰이다 — 이번 스윕의 216 요청 모두 output 256 이었다(결과 JSON 관측 · 10-16 측정도 같다 · testlog_26101619 §2).

```hint-event
id: W4
kind: wall
증상: 열 매핑을 주면 출력 길이 열이 빠져 max_tokens 없이 컨텍스트 끝까지 생성할 수 있음이 이미지 소스 판독에서 드러났다(실패 런 기록 없음)
서명: "출력 길이 열이 빠지면 max_tokens 가 사라지고 ignore_eos 가 컨텍스트 끝까지 생성"
원인: GuideLLM 에 column_mappings 를 주면 기본 매핑 전체가 교체된다
해소: 실행 시점에 text · prompt_tokens_count · output_tokens_count 세 열을 모두 명시
검증: 측정 결과 JSON config 의 세 열 매핑 · 실측 output 256(전 216 요청)
전이등급: arch-invariant
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2, output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json]
```

### W5 상주 서버의 실행별 엔진 로그가 그 실행의 범위가 아니었다
앞선 캠페인 종결 testlog 가 "행이 겹치지 않는데 prefix cache hit rate 가 64.5~65.0%" 를 미해결로 남겼고(M2), 그것을 파던 Q1 조사가 더 근본적인 결함을 찾았다. `run_bench.sh` 는 실행 뒤 `docker logs <ctr> | tail -800` 으로 엔진 로그를 담았는데, 2일 넘게 상주한 서버에서 그 800줄은 전날부터의 · 다른 클라이언트의 트래픽까지 덮는다 — granite 레벨 01 로그의 POST 61건 중 이 실행분은 18건이었다. 실행별 산출물처럼 레벨 디렉터리에 놓인 로그를 읽는 소비자(엔진 throughput 파서 · hint 발췌)가 다른 실행의 줄을 이 셀에 귀속할 수 있었다. 원인 확정은 로그 첫 줄 · 끝 줄 시각(10-15 15:31 → 10-16 06:53 등)과 POST 줄 대사였다(testlog_26101622_47_43 §2 · §3). 앞선 측정의 레벨 1 `engine_gen_throughput_max` 51.2 가 c=1 decode 12.46 의 4.11배로 찍힌 것도 같은 결함의 흔적으로 읽힌다(simlog `level_01_measured.json` client_engine_agreement — 이 해석은 추론).

해소는 처방 ⓑ+ⓒ(사람 결정)를 구현한 ecb7dca 다 — 엔진 로그는 실행 시작 시각부터만 담고(`docker logs --since`), prefix cache 적중은 로그 창 통계가 아니라 `/metrics` 카운터의 실행 전후 차분으로 잰다:

> [원문] run_bench.sh@3fa6eac00a18 §L456-464
> # ── 실행 범위 관측 (2026-10-16 · plan_26101622 F2) ────────────────────────────────────
> #   ① 엔진 로그는 이 실행이 시작한 시각부터만 담는다(`docker logs --since`). 종전 `tail -800` 은 상주
> #      서버에서 **전날·다른 클라이언트 트래픽**까지 덮어(실측: granite level 01 POST 61건 중 이 실행분 18),
> #      엔진 로그 파서(engine_gen_throughput_max)와 hint 발췌가 다른 실행의 줄을 이 실행에 귀속했다.
> #   ② prefix cache 적중은 엔진 로그의 "Prefix cache hit rate"(최근 요청 창 통계 · 이전 트래픽 포함)가 아니라
> #      `/metrics` 카운터의 **실행 전후 차분**으로 잰다 — 같은 프롬프트 반복(중복 샘플링 결함 E2) 같은 측정
> #      오염을 기계가 볼 수 있게 한다. 서버 전체 카운터이므로 이 창의 다른 요청도 섞인다 → requests_delta 를
> #      함께 적어 섞였는지 읽게 한다. 판정 규칙은 이 값을 쓰지 않는다(기록 전용).
> BENCH_SINCE="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

통과 술어: 이번 레벨 1 대표 run 의 엔진 로그는 실행 범위 62줄(POST 18 · run_02 63줄 · run_03 61줄 — 원시 재계수 · testlog_26101707 §1 의 '63줄' 은 1차 폐기 스윕 값을 적은 Q1 testlog §5 의 수와 같아 승계로 보인다)이고 c=1 의 `engine_gen_throughput_max` 12.5 가 decode 12.36 과 맞는다(testlog_26101707 §1). 차분 값은 판정에 쓰이지 않는 기록 전용이라 판정은 바뀌지 않는다. 남은 범위: 같은 스윕의 lite 레그는 이 판본에서도 여전히 `tail -800` 으로 엔진 로그를 담는다(위 W2 발췌 L197 · Q5).

```hint-event
id: W5
kind: wall
증상: 상주 서버에서 레벨별 엔진 로그가 그 실행의 요청 외에 전날 · 다른 클라이언트 트래픽을 포함했다(granite 레벨 01 POST 61건 중 이 실행분 18)
서명: "`engine_<cfg>.log` 은 실행 산출물처럼 레벨 디렉터리에 있지만, 상주 서버에서는"
원인: run_bench 가 실행 뒤 docker logs | tail -800 으로 엔진 로그를 담아 상주 서버의 이전 트래픽까지 실행 산출물에 섞였다
해소: ecb7dca — docker logs --since 실행 시작 시각으로 실행 범위 캡처 · /metrics prefix cache 카운터 실행 전후 차분을 prefix_cache_<cfg>.json 과 measured.json prefix_cache_run 으로 기록(기록 전용)
검증: 레벨 1 대표 run 실행 범위 로그 62줄(POST 18) · c=1 engine_gen_throughput_max 12.5 와 decode 12.36 일치 · 12 런 requests_delta 18
전이등급: arch-invariant
출처: [testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §3, testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §1, run_bench.sh@3fa6eac00a18]
```

### W6 1차 재측정의 레벨 1 첫 런이 앞선 조사가 보낸 같은 행에 재적중했다
ecb7dca 뒤 첫 재측정 스윕(14:04Z~ · salt 없음 · 행 0..215)에서 새 가드가 바로 일을 했다 — 레벨 1 run 1 의 실행 차분 적중률이 0.9856(requests_delta 18)으로 찍혔다. 원인은 그 직전(13:46Z) Q1 조사가 eviction 확인용으로 같은 행 0..17 을 granite 에 직접 보내 캐시에 다시 올려 둔 것이다 — 조사 자체가 다음 측정을 오염시켰다. 그 런이 대표 run 이라 레벨 1 TTFT 가 95.2 ms 로 잡혔고(깨끗한 run 2 · 3 = 262.8 · 263.4 ms) decode 는 12.47 / 12.37 / 12.35 로 영향이 거의 없었다 — 판정도 같았다:

> [원문] testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2
> 1차 스윕(salt 없음 · 행 0..215)의 level 1 run 1 은 `hit_rate_run 0.9856` 이었다 — Q1 조사(`testlog_26101622_47_43_Q1_…`)가 13:46Z 에 같은 행 0..17 을
> granite 에 직접 보내 cache 에 올려 두었기 때문이다. 그 런이 대표값이 되어 level 1 TTFT 가 95.2 ms 로 잡혔다(깨끗한 run 2·3 = 262.8 · 263.4 ms). decode 는
> 12.47 / 12.37 / 12.35 로 영향이 거의 없었고 판정도 같았다. 나머지 11 런은 적중률 0.0.

직접 원인은 조사 요청이지만 구조 원인은 행 선택이 스윕마다 같다는 것이다(첫 런 = 늘 행 0..n-1) — 같은 config 를 캐시가 비기 전에 다시 재면 조사가 없어도 같은 프롬프트가 적중한다. 서버 쪽 초기화(`/reset_prefix_cache`)는 쓸 수 없었다(R5). 사람 결정("스윕별 행 오프셋 + 재측정")으로 3fa6eac 가 스윕 시작 UTC 를 salt 로 한 결정론 시작 오프셋을 넣었다(스윕 안 런끼리는 여전히 겹치지 않는다 · salt 부재 = 종전 거동):

> [원문] sweep_bench.sh@3fa6eac00a18 §L261-261
> DATASET_SWEEP_SALT="$(date -u +%FT%TZ)"   # 스윕마다 다른 행 시작점(prefix cache 재적중 회피 · sidecar 에 기록 · plan_26101622 F2 후속)

2차 스윕(salt 2026-10-16T21:44:27Z · offset 2727)은 12 런 모두 적중 0.0 · requests_delta 18 이었다 — 레벨 1 대표 런의 차분 원문:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/prefix_cache_granite42-8b-v031.json §L7-10
>   "queries_delta": 18702,
>   "hits_delta": 0,
>   "requests_delta": 18,
>   "hit_rate_run": 0.0

1차 스윕의 원시 산출물은 같은 디렉터리를 2차가 덮어 이 발행 시점에 남아 있지 않다 — 1차 수치는 testlog · devlog 의 기록으로만 인용한다.

```hint-event
id: W6
kind: wall
증상: 1차 재측정 레벨 1 run 1 의 실행 차분 적중률 0.9856 · 대표 TTFT 95.2 ms(깨끗한 run 262.8 · 263.4 ms)
서명: "1차 스윕(salt 없음 · 행 0..215)의 level 1 run 1 은 `hit_rate_run 0.9856` 이었다"
원인: 직전 Q1 조사가 같은 행 0..17 을 같은 서버에 보내 캐시에 올려 둔 것이 직접 원인 · 행 선택이 스윕마다 같다(첫 런 = 늘 행 0..n-1)는 것이 구조 원인
해소: 1차 스윕 폐기 · 3fa6eac 스윕 시작 UTC salt 기반 결정론 행 시작 오프셋(sidecar sweep_salt · sweep_offset) · 사람 결정으로 재측정
검증: 2차 스윕 12 런 모두 hit_rate_run 0.0 · requests_delta 18 · 행 offset 2727 + 0..215
전이등급: arch-invariant
출처: [testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2, testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §5, sweep_bench.sh@3fa6eac00a18, output/single/benchlog/sweep_granite42-8b-v031/level_01/prefix_cache_granite42-8b-v031.json]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: plan 단계표가 적은 스윕 레벨 1 · 2 · 4 · 8 · 16 중 레벨 16
기각사유: 실행 전에 스윕 도구가 서빙 상한(max-num-seqs 8)을 넘는 레벨로 자동 제외했다 — 시도해서 실패한 것이 아니다
재개조건: max-num-seqs 를 16 이상으로 다시 도출한 서빙에서
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §5, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1, output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json]
```

```hint-event
id: R2
kind: rejected
시도: 전처리 결과에서 목표 길이 행을 고를 때 prompt 토큰 수가 input_len 과 정확히 같은 행만 고르는 기준
기각사유: BOS 를 세는 토크나이저(minicpm 1025)에서 11 행만 남아 같은 프롬프트가 반복됐다 — 최빈값 기준으로 교체 · 이 셀(granite)은 6318 행 중 목표 1024 행이 6317 이라 옛 기준에서도 행 부족이 없었다
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1]
```

```hint-event
id: R3
kind: rejected
시도: full-bench 입력을 GuideLLM 합성 텍스트(--data kind=synthetic_text)로 고정하던 옛 기준
기각사유: 특정 언어로 고정된 입력의 벤치가 필요해 데이터셋을 프리셋 knob 로 열었다 — 합성 기본값은 기본 프리셋에 남고 이 셀은 CNN 프리셋으로 쟀다
재개조건: 합성 입력 대조가 필요하면 기본 프리셋으로 같은 서빙을 다시 잰다(범위 밖이었다)
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §1, plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2]
```

```hint-event
id: R4
kind: rejected
시도: ecb7dca 뒤 첫 재측정 스윕(salt 없음 · 행 0..215 · 레벨 1·2·4·8 × 반복 3)의 결과를 이 셀의 측정으로 쓰는 것
기각사유: 레벨 1 run 1 이 직전 조사가 보낸 같은 행에 재적중(실행 차분 0.9856)해 대표 TTFT 가 95.2 ms 로 잡혔다 — decode · 판정은 같았지만 TTFT 가 오염돼 스윕 전체를 폐기하고 행 오프셋 반영 뒤 다시 쟀다
출처: [testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2, testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §5]
```

```hint-event
id: R5
kind: rejected
시도: 측정 전에 서버의 prefix cache 를 비우는 서버 쪽 초기화(/reset_prefix_cache)
기각사유: 이 서버는 DEV_MODE 가 아니라 그 엔드포인트가 404 를 돌려 쓸 수 없었다 — 대신 클라이언트 쪽 행 시작점을 스윕마다 바꿨다(W6)
재개조건: DEV_MODE 로 띄운 서버(운영 서빙에는 권하지 않는다 — 의견)
출처: [testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §5]
```

```hint-event
id: R6
kind: rejected
시도: 합성(synthetic) 기본 프리셋에도 같은 스윕별 행 오프셋 성질을 바꿔 넣는 것
기각사유: 기본 프리셋의 GuideLLM 명령이 개정 전과 바이트 동일해야 한다는 합격 기준(A1) 때문에 바꾸지 않았다 — 같은 성질이 있으면 이제 prefix_cache_run 이 기록으로 드러낸다
재개조건: A1 을 사람이 풀 때
출처: [testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2]
```

이 셀의 서빙 형상을 좁힌 기각은 이 계보에 없다 — 서빙 값은 앞선 계획에서 그대로 왔다. 좁혀진 것은 측정 형상이다: 스윕 레벨은 서빙 상한 8 에서 잘렸고(R1), 입력은 합성에서 CNN 기사 본문으로 바뀌었으며(R3), 행 선택 기준은 다른 셀의 실패로 교체됐고(R2 · 이 셀에는 영향 없음), 이번 겹에서는 오염된 첫 재측정을 버리고(R4) 서버 쪽 초기화 대신 클라이언트 쪽 행 시작점으로 재적중을 피했다(R5 · R6 은 기본 프리셋을 그대로 둔 결정).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

**M1** — 데이터셋 knob 계획은 실측 입력 토큰이 전처리 목표와 같을 것으로 적었다. 실제 prompt 토큰은 1039 였다 — 전처리 본문 1024 에 openai-chat 경로의 chat 템플릿 15 토큰이 붙는다. 같은 경로의 합성 입력에도 템플릿이 붙으므로 데이터셋 탓이 아니라 엔드포인트 탓이다(이번 스윕 216 요청도 1039).

```hint-event
id: M1
kind: misdiagnosis
증상: 합격 기준이 실측 입력 토큰을 전처리 목표 길이와 같은 값으로 기대했다
서명: "실측 입력 토큰 = 1024 · 출력 = 256 · verdict 산출"
원인: 전처리 목표 길이(본문 토큰)와 서버가 실제로 받는 프롬프트 토큰(chat 템플릿 포함)을 같은 것으로 봤다
해소: 실측 입력은 1039 = 본문 1024 + chat 템플릿 15(openai-chat 경로) · 출력 256 은 기대대로
검증: testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2 · §4
전이등급: judgment
현재지위: 반증
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §6, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §4]
```

**M2** — 앞선 캠페인 종결 testlog 는 레벨 01 엔진 로그(tail-800)에 찍힌 hit rate(granite 레벨 1 끝 64.5~65.0% · 리포트에는 이 지표가 없다)를 이 셀 요청의 성질처럼 다루고, "행이 겹치지 않는데도 높은 이유" 를 chat 템플릿 공통 접두 · 기사 이어 붙임 경계 같은 데이터 쪽 가설로 남겼다. Q1 조사가 `/metrics` 카운터 차분으로 가른 결과는 H1 확증이다 — 그 값은 엔진 로그의 최근 요청 창 통계이고, CNN 행은 처음 보낼 때 적중 0 이며(대조군 즉시 재전송 98.6% 로 측정 수단 작동 확인), 로그 값은 CNN 실행이 진행될수록 오히려 내려갔다(적중 0 인 요청이 창을 희석):

> [원문] testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §2
> | granite | 01 | 10-15 15:31 → 10-16 06:53 | 70.4% → 65.0% |
> | granite | 02 | 10-15 15:58 → 10-16 07:09 | 71.4% → 46.7% |
> | granite | 04 | 10-15 16:18 → 10-16 07:17 | 72.1% → 36.5% |
> | granite | 08 | 10-15 16:36 → 10-16 07:22 | 72.7% → 29.9% |

같은 해석이 앞선 판의 열린 물음에도 실려 있었다(이 셀 앞선 판 hint 의 Q1 — 발행 절차 사정은 00 §0.8). 이번 측정에서도 레벨 1 실행 범위 엔진 로그는 `Prefix cache hit rate: 20.8%` 를 찍지만 같은 런의 실행 차분은 0.0 이다 — 로그 값은 실행 귀속 지표가 아니다.

```hint-event
id: M2
kind: misdiagnosis
증상: CNN 셀 레벨 01 엔진 로그(tail-800)와 그것을 옮긴 종결 testlog 에 prefix cache hit rate 64.5~65.0%(granite 레벨 1 끝)가 찍혔고 행이 겹치지 않는데 높은 이유를 데이터 쪽에서 찾았다
서명: "행이 겹치지 않는데도 높은 이유 미조사(chat 템플릿 공통 접두 · 기사 이어 붙임 경계 등 가설)"
원인: 엔진 로그의 Prefix cache hit rate 는 최근 요청 창의 누적 통계라 상주 서버의 이전 트래픽을 포함한다 — 그것을 이 실행의 적중으로 읽었다
해소: H1 확증 — 로그 값은 이 셀 요청에 귀속되지 않는다 · CNN 행은 처음 보낼 때 적중 0 · 실행 귀속 적중은 /metrics 실행 차분(prefix_cache_run)으로만 잰다
검증: testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §1 · §2 · 이번 측정 12 런 실행 차분 0.0
전이등급: arch-invariant
현재지위: 반증
출처: [testlog_26101622_plan_26101614_종결판정_데이터셋knob_CNN_lite분리 §4, testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §2, output/single/benchlog/sweep_granite42-8b-v031/level_01/engine_granite42-8b-v031.log]
```

**M3** — 같은 Q1 판정은 거기서 한 걸음 더 나가 "CNN full 4셀의 TTFT·decode 측정은 prefix cache 의 도움을 받지 않았다" 고 적었다. 근거는 "처음 보내면 적중 0" 이라는 대조였다. 그 뒤(2026-10-17 · 이 판 publish 직후) 앞선 셀 판정 testlog 머리에 SUPERSEDED-IN-PART 표기가 붙어 granite 레벨 1 첫 런에 대해서는 이 주장을 뒤집었다:

> [원문] testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §testlog — CNN/DailyMail full-bench 셀 granite42-8b-v031 판정
> > **SUPERSEDED-IN-PART** → `docs/testlog/testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정.md`(2026-10-17): level 1 대표 TTFT 93.6 ms 는 run 1 이 prefix cache 의 도움을 받은 값이다(run 2·3 = 262.5 · 261.0 ms · 같은 날 앞선 라이브 시도가 같은 행 0..17 을 보낸 것으로 추정 — 추론). decode 12.46 t/s 와 PASS 판정은 유효하다. 같은 날 minicpm·ornith·lfm 셀은 run 간 TTFT 가 고르다(오염 없음). 원격 태그는 리콜하지 않는다 — 새 판이 대체한다.

이 정정의 근거 등급을 구분해 둔다. **적중 판단**은 TTFT 모양이다 — 앞선 run 1 · 2 · 3 의 TTFT 93.58 · 262.49 · 260.95 ms(decode 12.46 · 12.38 · 12.38 · simlog `level_01_measured.json` runs)가 1차 재측정에서 카운터로 재적중이 확인된 런(W6: 95.2 대 262.8 · 263.4 ms · decode 12.47 대 12.37 · 12.35)과 같은 모양이고 같은 날 다른 셀은 run 간 TTFT 가 고르다. 당시에는 실행 차분 카운터가 없었으므로 카운터 근거는 없다. **원인**(같은 날 앞선 라이브 시도가 같은 행을 보냈다)은 표기 자신이 추론이라 밝힌 추정이다. decode 와 PASS 는 유효하다는 것이 표기의 결론이다. 덧붙여 재측정 판정 testlog §1 은 앞선 셀을 "TTFT 미기재" 로 적었지만 앞선 셀 testlog §2 는 TTFT p50 93.6 ms 를 기록하고 있다(기록된 값을 미기재로 적은 사례).

```hint-event
id: M3
kind: misdiagnosis
증상: 앞선(10-16) granite CNN 측정의 레벨 1 대표 run 1 TTFT 93.58 ms 가 run 2 · 3(262.49 · 260.95 ms)과 갈라진다
서명: "CNN full 4셀의 TTFT·decode 측정은 prefix cache 의 도움을 받지 않았다"
원인: 처음 보낸 행의 적중 0 이라는 대조 결과를 앞선 측정 전체로 일반화했다 — 앞선 측정의 첫 런 행이 그 전에 서버에 갔는지는 확인하지 않았다
해소: 앞선 셀 판정 testlog 의 SUPERSEDED-IN-PART 표기가 granite 레벨 1 run 1 을 prefix cache 도움을 받은 값으로 정정했다 · 근거는 TTFT 모양(카운터 없음) · 원인(같은 날 앞선 라이브 시도의 같은 행 전송)은 추정 · decode 12.46 과 PASS 는 유효
검증: testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 머리 표기 · simlog level_01_measured.json runs 의 TTFT 모양 · 이번 측정 12 런 실행 차분 0.0
전이등급: judgment
현재지위: 반증
출처: [testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §1, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §testlog — CNN/DailyMail full-bench 셀 granite42-8b-v031 판정, docs/simlog/26101620_camp_26101616_cnn_hint__main__granite42_8b_v031/level_01_measured.json, testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2]
```

M3 의 현재지위 `반증` 은 granite 레벨 1 첫 런에 한한 것이고 TTFT 모양 근거의 반증이다(카운터 근거 ✗) — 원인이 무엇이었는지는 Q1 에 미결로 남긴다.

교정 묶음: devlog_26101619 §3 의 D1~D9(9항목) · 종결 testlog_26101622 §3 의 L1~L8(8항목) · Q1 testlog §4 의 처방 ⓐ~ⓒ(3항목)를 훑었다. 이 셀의 판정 · 수치 · 이름에 닿은 항목만 적는다.
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D1 — GuideLLM 데이터 인자 배선 — 문서 꼴 → 이미지 실제 스키마(W1)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D2 · D3(= testlog_26101622_plan_26101614_종결판정_데이터셋knob_CNN_lite분리 L1) — 열 매핑 — 쉼표 분할 문자열 · 기본 매핑 교체 위험 → JSON 객체 · 세 열 명시(W3 · W4)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D4(= 종결 testlog L2) — lite 판정 — measurement_path_failed → pass(W2)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D6 — 리포트 · 03 의 dataset_tokenizer — 호스트 절대경로 → 컨테이너 경로(`/app/models/…`)
- testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 ⓑ · ⓒ — 실행별 엔진 로그 · prefix cache 지표 — tail-800 창 통계 → 실행 범위 로그 · `/metrics` 실행 차분(W5 · M2) · ⓐ(문서만)는 고르지 않았다
- 그 밖(D5 · D7~D9 · 종결 testlog L3~L8)은 이 셀의 판정 · 수치 · 이름을 바꾸지 않았다(L6~L8 은 hint 발행기 · FACT 결함으로 페이로드 형식에만 닿는다).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**gpu-memory-utilization(V1)** — 처음부터 0.39 다. 64GB Spark 시뮬 천장을 물리 호스트 기준으로 환산해 같은 노드 두 인스턴스가 공존하게 한 값이고(plan_26101410 §4 D4) 이 계보에서 바뀐 적이 없다.

**max-model-len(V2)** — 사용자 요구로 32768 고정이고 바뀐 적이 없다(plan_26101410 §1).

**max-num-seqs(V3)** — 8 이다. plan_26101410 §4 D5 는 실행 전에 `min(8, KV_fit@8192)` 산식이 정한다고 적었지만, 실행에서 explorer 가 이 산식으로 계산했다는 기록은 계보에 없다(계보 표 밖 devlog_26101412 · testlog_26101412 는 KV · gmu 의 손 산정만 적고 max-num-seqs 의 산정 기록은 없다). 이 셀에서는 바뀐 적이 없고, 측정에서는 스윕 레벨 상한으로 작용했다(R1).

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4
> | D4 | 타깃 GPU 블록 `target_gpu: GB10-64GB`(vram 64GB · target_gmu 0.80 · 대역폭 273GB/s `web` 출처 표시) · 배포 yaml `gpu-memory-utilization` 은 **물리 호스트 기준 환산값**(천장 ÷ 호스트 총량 ≈0.39)으로 둬 두 인스턴스가 공존 · KV 는 **절대 클램프**(`--kv-cache-memory-bytes`)로 고정 | `policy:KV_ABSOLUTE_CLAMP_PORTABILITY` · 두 인스턴스 기동 검사 |
> | D5 | 동시성 요구 = **8**(LangGraph 에이전트·RAG 병렬 호출 가정) · 대표 요청 길이 **8192 토큰** · `max_num_seqs = min(8, KV_fit@8192)` — explorer 산식(§8)이 결정 · 32k 단일 요청은 항상 1개 이상 수용 | explorer §4.2 |

**kv-cache-memory-bytes(V4)** — 25232932864 바이트 절대 클램프다. plan_26101410 §4 D4 는 클램프라는 방식만 정했고 값은 없다. KV 필요량의 출처 계산은 plan_26101410 §3 표의 토큰당 160KiB · 32k 1시퀀스 5.0GiB 이고, 엔진이 그 클램프를 되읽어 잡은 크기와 32k 요청 기준 배수는 엔진 로그가 적는다:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log §L77-78
> (EngineCore pid=277) INFO 10-14 02:02:04 [gpu_worker.py:603] Initial free memory 113.32 GiB, reserved 23.5 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling. This does not respect the gpu_memory_utilization config. Only use kv_cache_memory_bytes config when you want manual control of KV cache memory size. If OOM'ed, check the difference of initial free memory between the current run and the previous run where kv_cache_memory_bytes is suggested and update it correspondingly.
> (EngineCore pid=277) INFO 10-14 02:02:04 [kv_cache_utils.py:2464] GPU KV cache size: 154,000 tokens, Maximum concurrency for 32,768 tokens per request: 4.70x

즉 32k 단일 요청 필요량의 4.70배다(엔진 산정 · 선언의 되읽기). 음성대조 기록은 없지만 **일부러 크게 잡은 기록은 있다** — 계보 표 밖 devlog_26101412 가 사용자 "64/64 할당" 의도에 맞춰 KV 를 min(required, max_safe) 대신 max_safe(천장 − 가중치 − 오버헤드 7,600MiB)로 택했다고 적는다. 4.70배는 필요량에서 나온 값이 아니라 그 선택의 결과다. 값의 유래는 계보 표 밖 문서(devlog_26101412 · testlog_26101412)가 기록한다 — 앞선 캠페인에서 explorer 2-위상이 수렴하지 않아 KV 를 손으로 잡아 lockset 에 `kv_source=hand` 로 표시했다. 이 셀(재측정)은 이 값에 손대지 않았다.

**호스트 메모리 예산** — 원장 `budget_declare` 는 floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608 을 적는다(선언값 · 01 §1.4). 다섯 값은 바닥 = 총량 − 가중치 − KV − overhead 꼴과 맞는다(선언값끼리의 대조 — 산식 원문은 이 계보에 없다). 엔진 쪽은 가중치 16.42 GiB(실측) · KV 23.5 GiB(클램프 되읽기)다. 같은 노드 둘째 모델이 뜨면서 이 선언은 합산 선언 `serve-main-combined-granite-minicpm`(02:04:02Z · 원장)으로 교체됐다(plan_26101410 §4 D6).

**graph/eager** — 설정하지 않았고 엔진 기본 cudagraph(FULL_AND_PIECEWISE)로 떴다 · 캡처 31 s · 0.29 GiB(엔진 로그 L89). eager 대조는 없다.

**측정 입력의 값(서빙 노브 아님)** — 데이터 행 시작점: 앞선 측정과 1차 재측정은 행 0..215, 이번 측정은 스윕 salt `2026-10-16T21:44:27Z` 에서 도구가 낸 offset 2727 + 0..215 다(런별 사이드카). 반복 수 3 은 앞선 측정에서는 프리셋 기본값(defaulted)이었고 이번에는 캠페인 선언(declared · `budgets.repeats`)이다(두 리포트 측정 구성 표 `repeats_source`) — 값은 같다.

**판정에 손으로 승계된 입력** — spec 미선언이라 루브릭이 acceptance length 1.0 을 쓴다(03 §3.1 — 선언값 · 측정 아님). primary 는 roofline × 계수이고 그 값은 `roofline.json` 이 기록한다: expected_achievable 5.43 = R_fp 15.53 × realistic_fraction(리포트 루프라인 표). 계수를 고른 근거는 이 계보 밖이다.

**이 형상이 버티는 기전 대 다른 형상** — 같은 계보에서 이 모델로 다른 KV · gmu · 동시 시퀀스 · 배치 토큰이나 다른 서빙 형상을 시도한 셀이 없다 — 미결(근거 testlog_26101619 §1 · plan_26101410 §4). 이 형상이 "버틴다" 는 것은 이틀 넘는 상주 · 벤치 뒤 health 200(01 §1.4)까지가 관측이고, 다른 값에서 무엇이 깨지는지는 대조가 없어 말할 수 없다. 이번 겹에서 바뀐 것은 서빙 형상이 아니라 측정 입력(행 시작점)이고, 그 차이가 만든 결과 차이(TTFT 95.2 대 263 ms)의 기전은 prefix cache 재적중으로 확정됐다(W6 · 실행 차분).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 상주 서버의 엔진 로그를 `tail -N` 으로 담아 그 실행의 사실로 읽지 말 것 — W5 · testlog_26101622_47_43 §3 — 실행 시작 시각부터(`docker logs --since`) 담고, 줄 시각 범위가 실행 창 안인지 본다.
- 엔진 로그의 `Prefix cache hit rate` 를 이 실행의 적중률로 리포트에 옮기지 말 것 — M2 — `/metrics` 카운터의 실행 전후 차분(queries · hits · requests_delta)으로 재고 requests_delta 가 그 런의 요청 수와 같은지 같이 본다.
- 측정 직전에 같은 서버로 데이터셋 행을 직접 보내는 조사를 하지 말 것 — W6 · testlog_26101707 §2 — 조사가 필요하면 스윕이 쓰지 않는 행을 쓰거나 측정 뒤에 한다(Q1 조사는 미사용 행 3000..3017 을 썼지만 eviction 확인용 0..17 전송이 다음 측정을 오염시켰다).
- 스윕마다 같은 데이터 행(첫 런 = 행 0..n-1)으로 같은 config 를 다시 재지 말 것 — W6 · R4 — 스윕 시작 시각 salt 같은 결정론 오프셋으로 행 시작점을 바꾸고 사이드카에 기록한다.
- 대표 run 하나의 TTFT 를 런별 값 확인 없이 기준점으로 쓰지 말 것 — W6 · M3 — 런별 TTFT 와 실행 차분 적중을 함께 보고, 첫 런만 크게 낮으면 그 스윕을 버린다.
- 서버 쪽 `/reset_prefix_cache` 로 캐시를 비울 수 있다고 가정하지 말 것 — R5 — DEV_MODE 가 아니면 404 다 · 클라이언트 쪽 행 시작점으로 피한다.
- 오래 상주한 서빙에 lite 를 걸면서 엔진 로그 꼬리만 파싱하지 말 것 — W2 · testlog_26101619 §3 — 기동 사실(KV · 가중치 · 배너)은 머리 캡처에서 읽는다.
- GuideLLM 인자를 공개 문서 꼴로 쓰지 말 것 — W1 · W3 · W4 · devlog_26101619 §3 — 고정 이미지의 실제 스키마로 배선하고, 열 매핑은 JSON 객체 한 덩이로 세 열을 다 명시한 뒤 결과 JSON 의 config 와 실측 output 256 으로 확인한다.
- 전처리 목표 길이(1024)를 서버가 받는 입력 토큰으로 착각하지 말 것 — M1 · testlog_26101619 §4 — chat 경로는 템플릿 토큰이 붙는다(이 셀 1039).
- CNN 셀 수치를 합성 입력 셀 수치와 나란히 놓지 말 것 — R3 · testlog_26101619 §4 — CNN 셀끼리만 비교한다.
- 엔진 기동 배너 `0.31.1.dev0+…` 를 빌드 입력으로 쓰지 말 것 — 00 §0.4 — 재현은 `v0.31.0` 또는 40자 SHA 로.
- 이 셀의 KV 클램프 · gmu 를 다른 HW 로 복사하지 말 것 — V1 · V4 — 128GB급 노드를 64GB 둘로 나눈 이 노드의 사정에서 나온 승계값이다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 앞선(10-16) 측정의 레벨 1 run 1 에 같은 행 0..17 을 미리 보낸 앞선 트래픽은 무엇이었나
현재상태: 앞선 셀 판정 testlog 의 SUPERSEDED-IN-PART 표기가 run 1(TTFT 93.58 ms · run 2 · 3 = 262.49 · 260.95 ms)을 prefix cache 도움을 받은 값으로 정정했다 — 근거는 TTFT 모양이고 당시 실행 차분 카운터는 없었다 · 원인은 같은 날 앞선 라이브 시도로 추정(추론)될 뿐 그 시도의 요청 기록은 계보에 없다(앞선 판 Q1 도 끊긴 스윕이 같은 행을 썼는지 미확인으로 적었다)
다음관측: 없음 — 앞선 측정의 서버 상태는 다시 관측할 수 없다 · 이후 측정은 prefix_cache_run 으로 런마다 가른다
출처: [testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §testlog — CNN/DailyMail full-bench 셀 granite42-8b-v031 판정, docs/simlog/26101620_camp_26101616_cnn_hint__main__granite42_8b_v031/level_01_measured.json, testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2]
```

```hint-event
id: Q2
kind: open-question
물음: 외부 레퍼런스(E) 없이 루프라인만으로 낸 판정 기준(primary 5.43 t/s)이 이 모델 · HW 에서 적정한가
현재상태: 리포트가 E-not-attempted 경고와 함께 판정을 roofline-only 로 강등했다 · 루브릭 권한 weak · 측정/primary 비 2.276 · 계수의 근거는 계보 밖이다
다음관측: 외부 검색으로 같은 HW 의 granite-4.2-8b bf16 decode 수치를 찾아 explicit 권한으로 재판정
출처: [bench_report_26101707_granite-4.2-8b_GB10_0.31.0 §판정]
```

```hint-event
id: Q3
kind: open-question
물음: CNN 실텍스트 입력이 합성 입력 대비 decode 처리량 · TTFT 를 바꾸는가
현재상태: 합성 대조 셀은 범위 밖이었다 · 같은 서빙의 lite 레그(random 512/128 · warm 12.58 t/s)는 입력 길이 · 도구가 달라 비교 조건이 아니다
다음관측: 같은 서빙 · 같은 형상(1024/256 · 레벨 1~8 · 반복 3)으로 기본(합성) 프리셋 스윕
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2, bench_report_26101707_granite-4.2-8b_GB10_0.31.0 §lite 지표]
```

```hint-event
id: Q4
kind: open-question
물음: 동시성 16 이상에서 이 레시피는 어떻게 되는가
현재상태: max-num-seqs 8 때문에 레벨 16 은 제외됐다 · 레벨 8 에서 decode 11.18 t/s · TTFT p50 1266.30 ms 까지 관측
다음관측: max-num-seqs 를 다시 도출한 서빙에서 레벨 16 측정
출처: [output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1]
```

```hint-event
id: Q5
kind: open-question
물음: lite 레그의 cold TTFT 가 앞선 측정(리포트 91 ms)과 이번 측정(리포트 146 ms)에서 왜 다른가
현재상태: 두 lite 레그는 같은 서빙 · 같은 도구 원문(lite_bench.sh 같은 판본 · random 512/128)이다 · lite 레그는 실행 차분 가드가 없고 엔진 로그도 여전히 tail-800 이라 적중 여부를 볼 수 없다 · 도구 원문에 seed 인자가 없어 매번 같은 난수 프롬프트일 수 있다는 것은 추론이다
다음관측: lite 레그에도 /metrics 실행 차분과 실행 범위 로그를 붙여 cold 요청의 적중을 기록
출처: [bench_report_26101707_granite-4.2-8b_GB10_0.31.0 §lite 지표, bench_report_26101616_granite-4.2-8b_GB10_0.31.0 §lite 지표, lite_bench.sh@803064d23d51]
```

```hint-event
id: Q6
kind: open-question
물음: 합성 기본 프리셋의 GuideLLM 스윕도 같은 재적중 성질을 갖는가
현재상태: A1(명령 바이트 동일) 때문에 기본 프리셋에는 행 오프셋을 넣지 않았다 · 같은 성질이 있으면 prefix_cache_run 이 기록하도록만 해 두었고 실제로 기본 프리셋을 같은 서버에 두 번 연속 잰 관측은 이 계보에 없다
다음관측: 기본 프리셋 스윕을 같은 서버에서 연달아 두 번 돌려 둘째 스윕 첫 런의 hit_rate_run 확인
출처: [testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §2]
```

- Q1 은 이 셀의 두 판(앞선 판 · 이 판)을 TTFT 로 나란히 놓을 때 걸린다 — decode 비교에는 걸리지 않는다(03 §3.5). Q5 는 lite TTFT 를 기준점으로 쓸 때 걸린다.
