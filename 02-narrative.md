# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 7건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite.md` | plan | 26101410 | 0 | 10727 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | — |
| 2 | `docs/testlog/testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정.md` | testlog | 26101619 | 0 | 3497 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | — |
| 3 | `docs/devlog/devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | devlog | 26101619 | 0 | 4789 | 0.31.0 · 8b · granite42 · granite42-8b-v031 · v031 | — |
| 4 | `docs/benchmark/bench_report_26101616_granite-4.2-8b_GB10_0.31.0.md` | bench_report | 26101616 | 0 | 6521 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source | — |
| 5 | `docs/plan/plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행.md` | plan | 26101616 | 0 | 2294 | 0.31.0 · 8b | — |
| 6 | `docs/testlog/testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정.md` | testlog | 26101619 | 1 | 2895 | 0.31.0 · 8b · easy-vllm:0.31.0-cu133-aarch64-source · granite42 · granite42-8b-v031 · v031 | — |
| 7 | `docs/plan/plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리.md` | plan | 26101614 | 1 | 16488 | 0.31.0 · 8b | docs/benchmark/bench_report_26101616_minicpm5-2b_GB10_0.31.0.md |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

이 계보는 두 겹이다. 날짜가 가장 이른 plan_26101410(2026-10-14)은 vLLM 을 0.31.0 으로 올리고 NAS 의 14B 이하 텍스트 모델 4종을 두 노드에 둘씩 **상주 서빙**하는 것이 목표였다 — 메인 노드에 Granite-4.2-8B(포트 8001)와 MiniCPM5-2B(포트 8002), 컨텍스트 32768 고정, LangGraph RAG 용 OpenAI 호환 API + 도구호출 + 추론 파서, 벤치는 lite 만.

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §1
> 3. NAS 에서 최근 다운로드한 **14B 이하 텍스트 생성 모델 4종**을 노드당 2종씩 **상주 서빙**한다 —
>    max-model-len **32768 고정**, LangGraph RAG 용으로 **OpenAI 호환 API + 도구호출(auto tool choice) + 추론 파서** 활성.
> 4. 각 모델은 **DGX Spark 64GB(OEM 전용 구성) 1대**를 타깃 GPU 로 시뮬레이션한다 — 노드 128GB 를 64/64 로 나눈다.

이 셀의 수치를 만든 것은 둘째 겹인 plan_26101614(2026-10-16)다 — 서빙은 손대지 않고, 2일째 상주 중인 같은 서빙에 GuideLLM full-bench 를 실텍스트 입력으로 거는 것이 목표였다.

> [원문] plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §1
> 1. full-bench(GuideLLM)의 `--data kind=synthetic_text` 고정을 풀고 **데이터셋을 프리셋 knob 로 편입**한다.

> [원문] plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §1
> 4. 장기 목적: 한국어 파인튜닝·특화 모델 평가. 난수 합성 텍스트가 아니라 **특정 언어로 고정된 입력**의 벤치가 필요하다.

**HW 출발 조건** — GB10 ×1 · 노드 1대(single · TP=1 · 분산 없음) · aarch64 통합메모리. 노드 메모리 총량은 블랙박스 원장 `budget_declare` 가 `mem_total_mib=124608` 로 적는다(01 §1.4). 드라이버 580.173.02 · 호스트 CUDA 13.2 는 plan_26101410 머리 유효맥락의 값이다(호스트 층 · 이미지 CUDA 13.3.1 과 다른 층). 사람은 모델당 64GB Spark 1대를 시뮬하기로 정했다(plan_26101410 §2 U3 — 가용 약 59.6GiB × target_gmu 0.80 ≈ 47.7GiB 천장). 대역폭은 plan_26101410 §3 이 웹 출처로 273 GB/s 라 적는다. 인터커넥트는 단일 노드라 해당 없다.

**모델 구조** — HF `ibm-granite/granite-4.2-8b`(revision `41a8a2d41c54ef4a71741b3e62604f4caaec9295` · base `granite-4.1-8b-base` · 00 §0.2). plan_26101410 §3 의 config 판독표가 `GraniteForCausalLM` · 40층 GQA kv8 · bf16 가중치 약 15.4GiB · 토큰당 KV 160KiB · 32k 1시퀀스 KV 5.0GiB 를 적는다(config 에서 계산한 값). 가중치 크기의 측정값은 이 셀 루프라인 산출 `roofline.json` 의 체크포인트 가중치 바이트 합 active_bytes=17583185920(active_gib 16.376 · dense 전 가중치)이고, 엔진 보고 체크포인트 크기 16.38 GiB(엔진 로그 L55)와 맞는다 · 적재 메모리는 16.42 GiB(L65). plan 의 약 15.4GiB 는 config 판독 추정값이다(측정 아님). 양자화 없음(bf16 단일 · 00 §0.4) · speculative_config 없음(엔진 config L48) · PLE 없음. 네이티브 컨텍스트는 체크포인트 config.json 의 max_position_embeddings 131072(rope_scaling null)이다 — 서빙 32768 은 사용자 요구로 그 1/4 로 깎은 값이다.

**stock 상태의 장벽** — 계보 표 7건 안에는 서빙 쪽 장벽 기록이 없다(빌드 패치 0 의 stock 소스빌드 · plan_26101410 §4 D2). 다만 같은 블랙박스 원장과 계보 표 밖 testlog_26101412 가 앞선 캠페인의 서빙 쪽 사건을 기록한다: trial01(예산 선언 생략 `kv_clamp_absent`)이 언클램프 KV 할당 중 ETA 워치독 `watchdog_trip rule=eta` → `docker_kill`(2026-10-14T01:52:50Z · 원장 watchdog.jsonl) · trial02(잠정 KV 클램프 kv_mib 5223)는 기능 OK 이나 오버헤드 실측 부재로 손 수렴 — 이 사건들은 이 페이로드의 W 블록으로 세지 않았다. 이번 캠페인에서 넘은 벽은 전부 **측정 경로**다 — GuideLLM 데이터 인자 스키마(W1) · lite 게이트 파서(W2) · 열 매핑 CLI 분할(W3) · 기본 열 매핑 교체(W4).

**선례와 사람의 결정** — 이 모델 · 이 HW 의 외부 커뮤니티 구현이나 수치는 계보에 없다. 사람이 고른 경로는 결정 표에 있다: 메모리는 64GB Spark 시뮬 · gmu 는 호스트 환산 · KV 는 절대 클램프 · 동시성 요구 8(plan_26101410 §4 D4 · D5), 데이터셋은 입력만 실텍스트 · 출력 ignore_eos 고정 · 판정기 불변(plan_26101614 §2 U1) · CNN/DailyMail `article` 원문 · 입력 1024 / 출력 256(U16) · hint 이름 불변(U9).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T11:50:00Z(`camp-26101616-cnn-hint` · evidence.event_campaign_boundary): campaigns/camp-26101616-cnn-hint/campaign.yaml declared_utc 2026-10-16T11:50:00Z → 경계 = 앞선 값 2026-10-16T11:50:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-16T08:04:04Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

_이번 캠페인(경계 2026-10-16T11:50:00Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T11:50:00Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `serve-granite42-8b-v031` | 2026-10-14T01:59:39Z | 1회 · 첫 2026-10-14T01:59:41Z | 없음 | —(닫힘 행 없음) | 2 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

벽은 devlog §3 표의 항목을 W1 = D1 · W2 = D4 · W3 = D2 · W4 = D3 으로 배치했다 — W1(P2 오프라인 spike)이 W2(P7 1차 스윕)보다 먼저인 것은 plan 단계 번호로 확정이고, W3 의 라이브 실패와 W4 의 소스 판독이 W2 의 앞인지 뒤인지는 계보에 기록이 없다(W3 이 동시성 4 에서 GuideLLM 실행 중 깨졌다는 것은 그 실행이 lite 게이트를 지났다는 뜻일 수 있다 — 추론). 위 기동 시도 표대로 이 캠페인은 서빙을 다시 띄우지 않았다 — 벽은 모두 같은 상주 서빙 위에서 측정을 다시 건 것이다.

### W1 GuideLLM v0.7.3 의 데이터 인자 스키마가 문서와 다르다
데이터셋 knob 를 배선하기 전 고정 GuideLLM 이미지로 오프라인 전처리 spike(P2 · granite 토크나이저 · concatenate)를 돌렸을 때 드러났다 — seed 인자는 문서의 `seed=` 꼴이 아니라 `kind=static,value=N` 이고, 단일 parquet 는 `split` 지정 없이는 DatasetDict 로 읽혀 매퍼가 죽는다. 원인은 이미지 안 실제 스키마와 공개 문서의 불일치였고, 처분은 "고정 이미지의 실제 스키마로 배선" 이다(devlog §3 D1). 통과는 측정 결과 JSON 의 config 가 그 꼴로 기록된 것으로 확인된다:

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

### W2 lite 게이트가 2일 상주 서빙에서 KV 를 못 찾아 full 진입을 막았다
P7 1차 스윕에서 lite 레그 요청은 전부 성공했는데 lite 판정이 `measurement_path_failed`(KV N/A)로 끝나 GuideLLM 이 시작되지 않았다. 원인은 lite 지표 파서가 엔진 로그 **꼬리**만 읽은 것이다 — 기동 때 찍히는 KV 라인이 2일 상주한 서빙에서는 꼬리에 없다. 머리 캡처 파일은 이미 있었는데 KV 파싱에 배선되지 않았다:

> [원문] testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3
> | E1 | 1차 스윕: lite 게이트 `measurement_path_failed`(요청 전부 성공 · KV N/A) → GuideLLM 미진입 | lite_metrics 가 엔진 로그 **꼬리**만 파싱 — 상주 2일 서빙은 기동 KV 라인이 꼬리에 없다(머리 캡처는 있었으나 KV 파싱 미배선). 교정 0f8800c 후 재판정 pass · 재스윕(`p7_try1_lite_parser_defect.log`) |

측정 도구 원문도 두 캡처가 따로 있었음을 보인다 — 꼬리 800줄과 머리 N줄:

> [원문] lite_bench.sh@803064d23d51 §L196-198
> if [ "$SERVE_PLANE" = "docker" ]; then
>   docker logs "$CTR" 2>&1 | tail -800 > "$ELOG" || true
>   docker logs "$CTR" 2>&1 | head -n "$ELOG_HEAD_N" > "$ELOG_HEAD" || true   # head 가 일찍 닫아 SIGPIPE — 실패로 읽지 않는다

해소는 커밋 0f8800c(lite 파서가 머리를 먼저 읽고 비면 꼬리로 메움)이고 재판정이 pass 로 바뀌었다(devlog §3 D4). 통과 술어는 lite 판정 pass 와 머리 로그의 KV 줄이다(아래 2.5 발췌 L77-78 · 03 의 lite KV 23.5 GiB). 이 벽은 서빙 결함이 아니라 하네스 결함이라 재빌드 · 서빙 재기동 없이 넘었다. 원래 1차 스윕 로그 파일은 이 발행 시점에 남아 있지 않다(testlog 가 이름만 적는다).

```hint-event
id: W2
kind: wall
증상: lite 요청은 전부 성공했는데 lite 판정 measurement_path_failed(KV N/A)로 GuideLLM full 스윕이 시작되지 않았다
서명: "1차 스윕: lite 게이트 `measurement_path_failed`(요청 전부 성공 · KV N/A) → GuideLLM 미진입"
원인: lite 지표 파서가 엔진 로그 꼬리(tail -800)만 파싱했고 2일 상주한 서빙의 기동 KV 라인은 꼬리에 없었다 · 머리 캡처는 있었으나 KV 파싱에 배선되지 않은 침묵 누락
해소: 커밋 0f8800c — lite 파서가 엔진 로그 머리를 먼저 읽고 비면 꼬리로 메운다 · 재빌드와 서빙 재기동 없음
검증: 재판정 pass · 머리 로그의 KV 예약 줄(L77-78) · lite KV 23.5 GiB 기록
전이등급: arch-invariant
출처: [testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3, devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, lite_bench.sh@803064d23d51, output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log]
```

### W3 열 매핑을 CLI 문자열로 주면 쉼표에서 쪼개져 깨진다
GuideLLM 의 `kind=..,column_mappings={..}` 꼴 인자는 CLI 가 쉼표에서 쪼개 다중 키 dict 를 깨뜨린다 — granite 동시성 4 라이브 실행 1회가 실패했다(devlog §3 D2). 처분은 인자를 JSON 객체 한 덩이로 넘기는 것이다. 그 실패 런의 로그는 계보에 남아 있지 않아 오류 서명은 devlog 의 기술로 대신한다. 통과는 위 W1 발췌와 같은 결과 JSON 의 `column_mappings` 가 세 키를 온전히 든 것으로 확인된다(아래 W4 발췌).

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
이 벽은 실패로 맞기 전에 이미지 소스 판독으로 잡았다(devlog §3 D3 — 이 계보에 이 함정이 실제로 발화한 런의 기록은 없다). `column_mappings` 를 주면 GuideLLM 의 기본 매핑이 통째로 바뀌어, 출력 길이 열이 빠지면 요청에서 max_tokens 가 사라지고 ignore_eos 때문에 컨텍스트 끝까지 생성한다 — 출력 256 고정이라는 측정 형상이 조용히 무너지는 함정이다. 처분은 실행 시점에 본문 · 입력 토큰 수 · 출력 토큰 수 세 열을 모두 명시하는 것이다:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json §L1-1
> "data_column_mapper": {"kind": "generative_column_mapper", "column_mappings": {"text_column": "article", "prompt_tokens_count_column": "prompt_tokens_count", "output_tokens_count_column": "output_tokens_count"}}

통과 술어는 실측 출력 토큰이다 — 전 216 요청의 output 이 256 이었다(testlog §2).

```hint-event
id: W4
kind: wall
증상: 열 매핑을 주면 출력 길이 열이 빠져 max_tokens 없이 컨텍스트 끝까지 생성할 수 있음이 이미지 소스 판독에서 드러났다(실패 런 기록 없음)
서명: "출력 길이 열이 빠지면 max_tokens 가 사라지고 ignore_eos 가 컨텍스트 끝까지 생성"
원인: GuideLLM 에 column_mappings 를 주면 기본 매핑 전체가 교체된다
해소: 실행 시점에 text · prompt_tokens_count · output_tokens_count 세 열을 모두 명시
검증: 측정 결과 JSON config 의 세 열 매핑 · 실측 output 256(testlog 2절 · 전 216 요청)
전이등급: arch-invariant
출처: [devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2, output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json]
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
재개조건: 합성 입력 대조가 필요하면 기본 프리셋으로 같은 서빙을 다시 잰다(이번 범위 밖)
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §1, plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2]
```

이 셀의 서빙 형상을 좁힌 기각은 이 계보에 없다 — 서빙 값은 앞선 계획에서 그대로 왔다. 좁혀진 것은 측정 형상이다: 스윕 레벨은 서빙 상한 8 에서 잘렸고(R1), 입력은 합성에서 CNN 기사 본문으로 바뀌었으며(R3), 행 선택 기준은 다른 셀의 실패로 교체됐다(R2 · 이 셀에는 영향 없음).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

데이터셋 knob 계획은 실측 입력 토큰이 전처리 목표와 같을 것으로 적었다. 실제 216 요청의 prompt 토큰은 1039 였다 — 전처리 본문 1024 에 openai-chat 경로의 chat 템플릿 15 토큰이 붙는다. 같은 경로의 합성 입력에도 템플릿이 붙으므로 데이터셋 탓이 아니라 엔드포인트 탓이다.

> [원문] testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §4
> 합격 A4(plan_26101614 §6): 리포트 측정 구성 표에 dataset_* 10행 · 실측 output 256 · verdict 산출 — 충족. 실측 input 은 1024 가 아니라 1039 다(채팅 템플릿 포함 · 합성 입력도 같은 경로에서 템플릿이 붙는다). CNN 셀은 CNN 셀끼리만 비교한다(합성 셀과 비교 ✗).

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

교정 묶음: devlog §3 의 D1~D9 9항목을 훑었다. 이 셀의 판정 · 수치 · 이름에 닿은 항목만 적는다.
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D1 — GuideLLM 데이터 인자 배선 — 문서 꼴 → 이미지 실제 스키마(W1)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D2 — 열 매핑 인자 — 쉼표 분할 문자열 → JSON 객체(W3)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D3 — 열 매핑 — 기본 매핑 교체 위험 → 세 열 명시(W4)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D4 — lite 판정 — measurement_path_failed → pass(W2)
- devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 D6 — 리포트 · 03 의 dataset_tokenizer — 호스트 절대경로 → 컨테이너 경로(`/app/models/…`)
- D5(minicpm 행 선택 · R2) · D7(README 경로 리터럴) · D8(승인 절 원자) · D9(서브 프리셋 위치)는 이 셀의 판정 · 수치 · 이름을 바꾸지 않았다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**gpu-memory-utilization(V1)** — 처음부터 0.39 다. 64GB Spark 시뮬 천장을 물리 호스트 기준으로 환산해 같은 노드 두 인스턴스가 공존하게 한 값이고(plan_26101410 §4 D4) 이 계보에서 바뀐 적이 없다.

**max-model-len(V2)** — 사용자 요구로 32768 고정이고 바뀐 적이 없다(plan_26101410 §1).

**max-num-seqs(V3)** — 8 이다. plan_26101410 §4 D5 는 실행 전에 `min(8, KV_fit@8192)` 산식이 정한다고 적었지만, 실행에서 explorer 가 이 산식으로 계산했다는 기록은 없다(앞선 캠페인의 explorer 트라이얼은 수렴하지 않았고 lockset 은 손으로 썼다 — 계보 표 밖 devlog_26101412). 이 셀에서는 바뀐 적이 없고, 이번 측정에서는 스윕 레벨 상한으로 작용했다(R1).

> [원문] plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4
> | D4 | 타깃 GPU 블록 `target_gpu: GB10-64GB`(vram 64GB · target_gmu 0.80 · 대역폭 273GB/s `web` 출처 표시) · 배포 yaml `gpu-memory-utilization` 은 **물리 호스트 기준 환산값**(천장 ÷ 호스트 총량 ≈0.39)으로 둬 두 인스턴스가 공존 · KV 는 **절대 클램프**(`--kv-cache-memory-bytes`)로 고정 | `policy:KV_ABSOLUTE_CLAMP_PORTABILITY` · 두 인스턴스 기동 검사 |
> | D5 | 동시성 요구 = **8**(LangGraph 에이전트·RAG 병렬 호출 가정) · 대표 요청 길이 **8192 토큰** · `max_num_seqs = min(8, KV_fit@8192)` — explorer 산식(§8)이 결정 · 32k 단일 요청은 항상 1개 이상 수용 | explorer §4.2 |

**kv-cache-memory-bytes(V4)** — 25232932864 바이트 절대 클램프다. plan_26101410 §4 D4 는 클램프라는 방식만 정했고 값은 없다. KV 필요량의 출처 계산은 plan_26101410 §3 표의 토큰당 160KiB · 32k 1시퀀스 5.0GiB 이고, 엔진이 실제로 잡은 크기와 32k 요청 기준 배수는 엔진 로그가 적는다:

> [원문] output/single/benchlog/sweep_granite42-8b-v031/lite_engine_head_granite42-8b-v031.log §L77-78
> (EngineCore pid=277) INFO 10-14 02:02:04 [gpu_worker.py:603] Initial free memory 113.32 GiB, reserved 23.5 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling. This does not respect the gpu_memory_utilization config. Only use kv_cache_memory_bytes config when you want manual control of KV cache memory size. If OOM'ed, check the difference of initial free memory between the current run and the previous run where kv_cache_memory_bytes is suggested and update it correspondingly.
> (EngineCore pid=277) INFO 10-14 02:02:04 [kv_cache_utils.py:2464] GPU KV cache size: 154,000 tokens, Maximum concurrency for 32,768 tokens per request: 4.70x

즉 32k 단일 요청 필요량의 4.70배를 잡았다(엔진 산정). 음성대조로 일부러 키운 값이라는 기록은 없다. 값의 유래는 계보 표 밖 문서(devlog_26101412 · testlog_26101412)가 기록한다 — 앞선 캠페인에서 explorer 2-위상이 수렴하지 않아(언클램프 트라이얼은 워치독에 사살 · 클램프 트라이얼은 프로파일링 생략) 오버헤드를 클램프 트라이얼의 MemAvailable 차분 7,600MiB 로 재고, KV 를 천장 − 가중치 − 오버헤드의 max_safe 로 손으로 잡아 lockset 에 `kv_source=hand` 로 표시했다(사용자 64/64 할당 의도). 원장 선언 overhead_mib=7600 이 그 값과 같다.

**호스트 메모리 예산** — 원장 `budget_declare` 는 floor_mib=76176 · weights_mib=16768 · kv_mib=24064 · overhead_mib=7600 · mem_total_mib=124608 을 적는다(선언값 · 01 §1.4). 다섯 값은 바닥 = 총량 − 가중치 − KV − overhead 꼴과 맞는다(선언값끼리의 대조 — 산식 원문은 이 계보에 없다). 엔진 실측은 가중치 16.42 GiB · KV 23.5 GiB 다. 같은 노드 둘째 모델이 뜨면서 이 선언은 합산 선언 `serve-main-combined-granite-minicpm`(02:04:02Z · 원장)으로 교체됐다(plan_26101410 §4 D6).

**graph/eager** — 설정하지 않았고 엔진 기본 cudagraph(FULL_AND_PIECEWISE)로 떴다 · 캡처 31 s · 0.29 GiB(엔진 로그 L89). eager 대조는 없다.

**판정에 손으로 승계된 입력** — spec 미선언이라 루브릭이 acceptance length 1.0 을 쓴다(03 §3.1 — 선언값 · 측정 아님). primary 는 roofline × 계수이고 그 값은 `roofline.json` 이 기록한다: realistic_fraction 0.35(expected_achievable 5.43 = R_fp 15.53 × 0.35) · 대역폭 273.0 GB/s(bandwidth_source lookup:gb10). 0.35 를 고른 근거는 이 계보 밖이다.

**이 형상이 버티는 기전 대 다른 형상** — 같은 계보에서 이 모델로 다른 KV · gmu · 동시 시퀀스 · 배치 토큰이나 다른 형상을 시도한 셀이 없다 — 미결(근거 testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1 · plan_26101410 §4). 이 형상이 "버틴다" 는 것은 2일 상주 · 벤치 뒤 health 200 · OOM 없음(01 §1.4)까지가 관측이고, 다른 값에서 무엇이 깨지는지는 대조가 없어 말할 수 없다.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 오래 상주한 서빙에 lite 를 걸면서 엔진 로그 꼬리만 파싱하지 말 것 — W2 · testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3 — 기동 사실(KV · 가중치 · 배너)은 머리 캡처에서 읽는다(머리 우선 · 비면 꼬리).
- lite 가 `measurement_path_failed` 인데 요청이 전부 성공했을 때 서빙을 재빌드 · 재기동하지 말 것 — W2 — 하네스(파서)를 먼저 고친다.
- GuideLLM 인자를 공개 문서 꼴로 쓰지 말 것 — W1 · devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 — 고정 이미지의 실제 스키마(`--help` · 소스)로 배선하고 결과 JSON 의 config 로 확인한다.
- 열 매핑을 `kind=..,column_mappings={..}` 문자열로 넘기지 말 것 — W3 — JSON 객체 한 덩이로 넘긴다.
- 열 매핑에 본문 열 하나만 주지 말 것 — W4 — 입력 · 출력 토큰 수 열까지 셋을 명시하고 실측 output 이 목표(256)와 같은지 본다.
- 전처리 목표 길이(1024)를 서버가 받는 입력 토큰으로 착각하지 말 것 — M1 · testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §4 — chat 경로는 템플릿 토큰이 붙는다(이 셀 1039).
- 목표 길이를 `== input_len` 으로 고르지 말 것 — R2 · devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 — BOS 를 세는 토크나이저에서 행이 고갈된다 · 분포 최빈값 기준.
- CNN 셀 수치를 합성 입력 셀 수치와 나란히 놓지 말 것 — testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §4 — CNN 셀끼리만 비교한다.
- 엔진 기동 배너 `0.31.1.dev0+…` 를 빌드 입력으로 쓰지 말 것 — 00 §0.4 — 재현은 `v0.31.0` 또는 40자 SHA 로.
- 같은 노드에 둘째 모델을 띄우면서 첫째의 예산 선언이 그대로 유효하다고 믿지 말 것 — plan_26101410 §3 · §4 D6 — 선언 파일은 노드당 1개라 둘째가 덮는다 · 합산 선언으로 다시 세운다.
- 이 셀의 KV 클램프 · gmu 를 다른 HW 로 복사하지 말 것 — V1 · V4 — 128GB급 노드를 64GB 둘로 나눈 이 노드의 사정에서 나온 승계값이다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 최종 스윕 전 같은 서빙에 들어간 요청 150건과 prefix cache 가 이 측정의 TTFT 에 영향을 줬는가
현재상태: 01 §1.4 가 최종 스윕 첫 측정 전 completions 요청 150건을 관측했다 · prefix caching 켜짐(엔진 config) · 레벨 1 엔진 로그 끝의 Prefix cache hit rate 가 64.5~65.0% 다 · 런마다 CNN 행은 겹치지 않지만 앞선(끊긴) 스윕이 같은 행을 썼는지와 hit rate 의 집계 창은 이 계보가 확인하지 않았다
다음관측: 새로 띄운 서빙에서 같은 프리셋으로 레벨 1 을 다시 재 TTFT p50 과 hit rate 를 대조
출처: [output/single/benchlog/sweep_granite42-8b-v031/level_01/engine_granite42-8b-v031.log, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1]
```

```hint-event
id: Q2
kind: open-question
물음: 외부 레퍼런스(E) 없이 루프라인만으로 낸 판정 기준(primary 5.43 t/s)이 이 모델 · HW 에서 적정한가
현재상태: 리포트가 E-not-attempted 경고와 함께 판정을 roofline-only 로 강등했다 · 루브릭 권한 weak · 측정/primary 비 2.295 · 계수 realistic_fraction 0.35 와 대역폭 273.0(lookup:gb10)은 roofline.json 이 기록하나 0.35 의 근거는 계보 밖이다
다음관측: 외부 검색으로 같은 HW 의 granite-4.2-8b bf16 decode 수치를 찾아 explicit 권한으로 재판정
출처: [bench_report_26101616_granite-4.2-8b_GB10_0.31.0 §판정]
```

```hint-event
id: Q3
kind: open-question
물음: CNN 실텍스트 입력이 합성 입력 대비 decode 처리량 · TTFT 를 바꾸는가
현재상태: 합성 대조 셀은 이번 범위 밖이었다 · 같은 서빙의 lite 레그(random 512/128 · warm 12.57 t/s)는 입력 길이 · 도구가 달라 비교 조건이 아니다
다음관측: 같은 서빙 · 같은 형상(1024/256 · 레벨 1~8 · 반복 3)으로 기본(합성) 프리셋 스윕
출처: [plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2]
```

```hint-event
id: Q4
kind: open-question
물음: 측정이 실제로 실행한 측정 도구의 바이트는 무엇이었나
현재상태: 03 §3.4 의 run_bench.sh · sweep_bench.sh 스냅샷은 데이터셋 knob 커밋(7db9d67) 이전 판본이다 · testlog 는 측정이 커밋 직전 워킹트리(같은 내용)였다고 적지만 워킹트리 바이트는 기계 관측 밖이다 · 실제 데이터 인자는 결과 JSON config 가 기록한다
다음관측: 7db9d67 판본 도구로 같은 셀을 다시 재 결과 JSON config 와 수치를 대조
출처: [testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정, output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json]
```

```hint-event
id: Q5
kind: open-question
물음: 동시성 16 이상에서 이 레시피는 어떻게 되는가
현재상태: max-num-seqs 8 때문에 레벨 16 은 제외됐다 · 레벨 8 에서 decode 11.11 t/s · TTFT p50 1264.79 ms 까지 관측
다음관측: max-num-seqs 를 다시 도출한 서빙에서 레벨 16 측정
출처: [output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json, testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §1]
```

- Q1 은 이 셀의 TTFT 해석에 걸린 유일한 미확인 교란이다. Q4 는 수신자가 같은 도구 바이트로 재현하려 할 때 걸린다.
