# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 7건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26102720_GPT_OSS_20B_Full_hint_게이트복구.md` | plan | 26102720 | 0 | 5357 | 20b · easy-vllm:0.18.0-cu130-aarch64-wheel · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 2 | `docs/benchmark/bench_report_26102816_gpt-oss-20b_GB10_0.18.0.md` | bench_report | 26102816 | 0 | 6428 | 20b · easy-vllm:0.18.0-cu130-aarch64-wheel · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 3 | `docs/plan/plan_26102816_GPT_OSS_20B_재개_Full_hint.md` | plan | 26102816 | 0 | 5157 | 20b · easy-vllm:0.18.0-cu130-aarch64-wheel · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 4 | `docs/testlog/testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단.md` | testlog | 26102720 | 0 | 4225 | 20b · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 5 | `docs/devlog/devlog_26102816_GPT_OSS_20B_신규_Full_서빙_여정.md` | devlog | 26102816 | 0 | 2662 | 20b · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 6 | `docs/testlog/testlog_26102816_GPT_OSS_20B_신규_Full_판정.md` | testlog | 26102816 | 0 | 4669 | 20b · gb10 · gpt · gpt-oss-20b-gb10 · oss | — |
| 7 | `docs/devlog/devlog_26102720_GPT_OSS_20B_producer_기술인계.md` | devlog | 26102720 | 1 | 1851 | 20b · gpt · oss | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

최초 계획은 crosscheck가 통과하기 전 서빙을 중지했다. 이후 정본이 수정되어 현재 입력으로 재확인했다.

> [원문] plan_26102720_GPT_OSS_20B_Full_hint_게이트복구 §복구 뒤 실행 순서
> 1. 정본 parse→crosscheck를 재실행해 MISMATCH 해소 여부를 확인한다. 메모리 입력 native_weight_bytes는 논리 파라미터 수와 별개로 보존해야 한다. Triton special-dep 경보는 후보 runtime import/op 확인으로 분류한다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-28T07:31:54Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-28T07:50:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-gpt-oss-20b-gb10` | 2026-10-28T07:31:54Z | 1회 · 첫 2026-10-28T07:31:55Z | 없음 | `budget_clear` 2026-10-28T07:50:46Z | 11 |

**같은 셀 이름 · 앞선 캠페인의 기동 13회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T09:27:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T09:29:34Z | 2 |
| 앞선-2 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T09:48:01Z | 없음 | 없음 | `budget_clear` 2026-09-17T09:50:27Z | 2 |
| 앞선-3 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:07:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:09:31Z | 2 |
| 앞선-4 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:30:12Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:32:33Z | 2 |
| 앞선-5 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:44:24Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:46:45Z | 2 |
| 앞선-6 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:53:41Z | 없음 | 없음 | `budget_clear` 2026-09-17T10:56:00Z | 2 |
| 앞선-7 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T10:59:36Z | 없음 | 없음 | `budget_clear` 2026-09-17T11:02:17Z | 2 |
| 앞선-8 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T11:07:14Z | 없음 | 없음 | `budget_clear` 2026-09-17T11:10:08Z | 2 |
| 앞선-9 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-17T11:11:10Z | 1회 · 첫 2026-09-17T11:13:48Z | 없음 | `budget_clear` 2026-09-17T11:15:50Z | 2 |
| 앞선-10 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T15:21:55Z | 없음 | 없음 | `budget_clear` 2026-09-29T15:30:25Z | 2 |
| 앞선-11 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T15:58:19Z | 1회 · 첫 2026-09-29T16:00:49Z | 없음 | `budget_clear` 2026-09-29T16:02:49Z | 2 |
| 앞선-12 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T18:29:06Z | 없음 | 없음 | `budget_clear` 2026-09-29T18:29:23Z | 2 |
| 앞선-13 | sub | `smoke-gpt-oss-20b-gb10` | 2026-09-29T19:02:09Z | 없음 | 없음 | `budget_clear` 2026-09-29T19:04:57Z | 2 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 · 파라미터 교차검증
기존 producer가 혼합 packing 전체 바이트를 단일 폭으로 환산해 외부 총계와 비교했다. 수정된 producer의 재수집은 외부 차이 0으로 통과했다.

```hint-event
id: W1
kind: wall
증상: crosscheck MISMATCH
서명: "파서 num_params | 27,522,633,808 | 전체 바이트÷0.5"
원인: 혼합 BF16과 packed MXFP4를 단일 폭으로 환산한 값을 총계로 비교
해소: 정본 MXFP4 구조 계산으로 재수집
검증: testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산
전이등급: judgment
출처: [testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단 §실제 결과, testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산]
```

> [원문] testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단 §실제 결과
> | 파서 num_params | 27,522,633,808 | 전체 바이트÷0.5, weight_basis=disk_total_prequant |

### W2 · 요청 실패
두 개의 실제 JSONDecodeError를 producer는 tool-boundary로 분류했다. 분류는 인과 확증이 아니다. 판정은 weak PASS이지만 오류는 남아 있다.

```hint-event
id: W2
kind: wall
증상: 두 run에서 각각 요청 실패 1건
서명: "동시성2 첫 run과 동시성4 세 번째 run 각각 GuideLLM JSONDecodeError 1건"
원인: 미결
해소: 해소되지 않음, producer는 경계 오류로 분류
검증: testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행
전이등급: judgment
출처: [testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행]
```

> [원문] testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행
> 15개 raw run의 completed 합은 254, server requests delta 합은 268다. 동시성2 첫 run과 동시성4 세 번째 run 각각 GuideLLM JSONDecodeError 1건이 있어 producer가 각각 tool-boundary로 분류했고 각 server_errors=0·measurement_ok=true다. 전체 오류0으로 쓰지 않는다. 분류명은 producer 판정이지 인과 확증이 아니다. 상세는 `docs/simlog/single8_gptoss20b_20261027T112514Z/attempt3/boundary_analysis.json`.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 혼합 가중치 바이트를 단일 FP4 폭으로 환산한 총계 교차검증
기각사유: 실제 BF16과 packed block의 논리 수와 외부 총계가 일치하지 않았다
재개조건: 구조를 검증하는 정본 producer 사용
출처: [testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단 §실제 결과, testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산]
```

정본 계산을 고쳐 다시 검사했고 tolerance를 낮추거나 넓혀 통과시키지 않았다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

이전 producer의 총계 대입이 구조를 반영하지 못했다. 파일 바이트를 버린 것이 아니라 논리 파라미터를 별도로 계산하도록 고쳤다.

```hint-event
id: M1
kind: misdiagnosis
증상: 기존 파서의 논리 총계 대입이 외부 총계와 불일치
서명: "파서 num_params | 27,522,633,808 | 전체 바이트÷0.5"
원인: BF16과 packed block 및 scale의 서로 다른 의미를 단일 폭으로 환산
해소: 헤더 구조 기반 논리 총계로 교차검증
검증: testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산
전이등급: judgment
현재지위: 반증
출처: [testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단 §실제 결과, testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산]
```

> [원문] testlog_26102720_GPT_OSS_20B_MXFP4_crosscheck_중단 §실제 결과
> | 파서 num_params | 27,522,633,808 | 전체 바이트÷0.5, weight_basis=disk_total_prequant |

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

V1~V6은 기존 후보와 모델 선언을 이어받았다. KV와 batch는 동일 모델 과거 근거의 손레버이며 이번 trial은 해당 조합을 확인했을 뿐 대체값과 실패 기전을 비교하지 않았다. 총 메모리 산식과 KV 필요량 대비 배수는 이 계보에서 확정하지 못했다. non-KV overhead는 측정되지 않았고 예약은 선언이다.

> [원문] devlog_26102816_GPT_OSS_20B_신규_Full_서빙_여정 §재개와 설정
> 이 모델·GB10·vLLM0.18.0의 과거 시험 `docs/testlog/testlog_26090308_gpt_oss_20b_gb10_0180.md:13-35`에 따라 KV48GiB/batch16을 후보로 삼았다. 로컬 trial은 completion·reasoning PASS와 KV capacity를 실측했으며 non-KV overhead는 null이다. 이후 실제 서비스는 native MXFP4, fp8 KV48GiB, context131072, gmu0.85, batch16으로 기동했고 trial 엔진은 TRITON_ATTN/Marlin을 보고했다. batch와 KV는 hand 입력을 이번 trial로 재검증했으며 자동 탐색 최적값이 아니다.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 전체 바이트를 단일 폭으로 나누어 혼합 모델 논리 총계로 쓰지 말 것 — W1, M1 — 구조 검증 producer를 사용한다.
- 경계 분류를 오류 없음으로 바꾸지 말 것 — W2 — 실제 실패와 판정 양쪽을 남긴다.
- 이전 방식으로 gate를 우회하지 말 것 — R1 — 수정된 producer로 재검사한다.
- 손레버를 최적점으로 복사하지 말 것 — V3, V4 — 대상 노드에서 검증한다.
- 모듈 버전을 배포 metadata나 빌드 원장으로 대체하지 말 것 — testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산 — 각 수집 출처를 구분한다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 두 JSONDecodeError의 원인은 무엇인가
현재상태: 두 run에서 각각 실패 1건이고 producer는 tool-boundary로 분류
다음관측: 재현 가능한 원시 응답과 클라이언트 파서 경계를 대조
출처: [testlog_26102816_GPT_OSS_20B_신규_Full_판정 §이번 실행]
```

```hint-event
id: Q2
kind: open-question
물음: 이 이미지의 빌드 원장과 컴파일 바이트를 확인할 수 있는가
현재상태: 빌드 원장과 source checkout은 미관측
다음관측: 소유자가 원래 빌드 산출물과 attestation 확인
출처: [testlog_26102816_GPT_OSS_20B_신규_Full_판정 §모델·기능·예산]
```
