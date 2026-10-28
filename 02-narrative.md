# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 4건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26102907_GPT120_v0320_Full_판정.md` | testlog | 26102907 | 0 | 1466 | 0191 · c0 · easy-vllm:0.32.0-cu133-aarch64-source-deps1 · gptoss120b · gptoss120b-0191-c0 | — |
| 2 | `docs/plan/plan_26102905_GPT120_서브_v0320_실행계획.md` | plan | 26102905 | 0 | 3655 | 0191 · c0 · gptoss120b · gptoss120b-0191-c0 | — |
| 3 | `docs/benchmark/bench_report_26102907_gpt-oss-120b_GB10_0.32.0.md` | bench_report | 26102907 | 0 | 6420 | easy-vllm:0.32.0-cu133-aarch64-source-deps1 | — |
| 4 | `docs/devlog/devlog_26102907_GPT120_v0320_실측과_인계.md` | devlog | 26102907 | 0 | 949 | — | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

목표는 독립 GB10에서 사용자 지정 최신 runtime으로 실제 서빙과 Full을 검증하는 것이었다. 초기 계획은 최신 소스 빌드와 모델 경로, 메모리 여유를 아직 미검증으로 두었다.

> [원문] testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정
> 두 trial 수렴 및 정식 서비스 health, completion 기능 PASS; tool/reasoning 전용 테스트 미실행.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-28T22:12:04Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `gpt120-v0320-deps1`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-28T21:43:22Z | 1회 · 첫 2026-10-28T21:43:24Z | 없음 | `budget_clear` 2026-10-28T22:12:03Z | 5 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 의존성 정합
원본 이미지의 pip check는 pytest 부재와 datasets가 요구하는 hub 범위의 충돌을 각각 보고했다. 별도 dependency layer에서 pytest와 호환 hub를 선택했고 이후 pip check가 통과했다. 이 검사는 모델 기능 통과와 별개다. 원본 build ledger는 후속 계층을 기록하지 않는다.

> [원문] docs/simlog/single8_gptoss120b_20261028T135226Z/build_result_context/runtime_probe.txt §L111-114
> -- pip check --
> triton-kernels 1.0.0+gitf797708c.nv26.7 requires pytest, which is not installed.
> datasets 5.0.0 has requirement huggingface-hub<2.0,>=0.25.0, but you have huggingface-hub 2.3.0.
> pip_check_rc=1

> [원문] docs/simlog/single8_gptoss120b_20261028T135226Z/build_dependency_repair/probe.txt §L1-1
> No broken requirements found.

```hint-event
id: W1
kind: wall
증상: 원본 이미지 의존성 정합 확인 필요
서명: "triton-kernels 1.0.0+gitf797708c.nv26.7 requires pytest, which is not installed."
원인: 빌드 이후 설치 의존성의 상충
해소: 별도 dependency repair 계층과 pip check
검증: testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정
전이등급: judgment
출처: [docs/simlog/single8_gptoss120b_20261028T135226Z/build_result_context/runtime_probe.txt §L111-114, docs/simlog/single8_gptoss120b_20261028T135226Z/build_dependency_repair/probe.txt §L1-1]
```

> [원문] testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정
> 두 trial 수렴 및 정식 서비스 health, completion 기능 PASS; tool/reasoning 전용 테스트 미실행.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

기존 저버전은 사용자 최신 버전 지시로 대체됐다. 이는 해당 버전의 기능 실패 판정이 아니다. [plan_26102905_GPT120_서브_v0320_실행계획 §GPT-OSS-120B sub v0.32.0 실행계획]

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

기록된 계보에서 이 셀의 오진을 확정할 근거는 찾지 못했다. source target과 엔진 SCM 표기는 서로 다른 생산자의 값이므로 동일 릴리스 표기로 합치지 않는다.

> [원문] testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정
> 두 trial 수렴 및 정식 서비스 health, completion 기능 PASS; tool/reasoning 전용 테스트 미실행.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

V1과 V2는 손으로 고른 값이고 V3은 첫 trial의 엔진 KV 보고로 산출한 뒤 두 번째 trial에서 검증했다. V4는 checkpoint 형식이다. 호스트 예산 overhead는 선언값이며 실측 non-KV overhead로 주장하지 않는다. 그래프와 다른 형상의 대조는 미결이다.

> [원문] testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정
> 두 trial 수렴 및 정식 서비스 health, completion 기능 PASS; tool/reasoning 전용 테스트 미실행.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

이전 Full 또는 다른 모델의 KV와 처리량을 그대로 가져오지 않는다. 기존 STOP과 원시 측정을 덮어쓰지 않는다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 동시성 2의 클라이언트 JSON 오류 원인은 무엇인가?
현재상태: 두 반복에서 각각 1건 발생했고 post-health는 정상이다.
다음관측: 같은 실행 조건의 클라이언트와 서버 원시 로그를 대조한다.
출처: [testlog_26102907_GPT120_v0320_Full_판정 §GPT-OSS-120B GB10 v0.32.0 sub 신규 Full 판정]
```

동시성 16의 두 반복은 warmup 0 및 18건으로 보고됐다. 선언 형상과 관측을 구분한다.
