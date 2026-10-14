# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 5건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101219_vllm_0_31_0_KV양자화_최대동시성_벤치_계획.md` | plan | 26101219 | 0 | 7451 | 0.31.0 · 30b · glimmer · muse · muse-glimmer-30b | — |
| 2 | `docs/testlog/testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정.md` | testlog | 26101413 | 0 | 2298 | 0.31.0 · 30b · glimmer · muse · muse-glimmer-30b | — |
| 3 | `docs/benchmark/bench_report_26101414_muse-glimmer-30b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md` | bench_report | 26101414 | 0 | 3017 | 0.31.0 · 30b · glimmer · muse · muse-glimmer-30b | — |
| 4 | `docs/devlog/devlog_26101414_muse_glimmer_30b_측정_서사.md` | devlog | 26101414 | 0 | 1802 | 30b · glimmer · muse · muse-glimmer-30b | — |
| 5 | `docs/report/perf_26101413_KV양자화_최대동시성_벤치_결과.md` | report | 26101413 | 2 | 3578 | 0.31.0 · 30b · glimmer · muse · muse-glimmer-30b | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

(1) **목표** — 캠페인의 세 번째 셀: vLLM 0.31.0 위에 Muse-Glimmer-30B(멀티모달 → 텍스트 전용 서빙)를 띄우고 KV dtype 5종 × 입력 7K/30K/130K의 최대 동시성을 실측한다.

(2) **HW 출발 조건** — 4B·32B와 동일 노드: RTX PRO 6000 Blackwell ×1(VRAM 95.6GiB) · 호스트 RAM 62GB(discrete) · 드라이버 580.178.04 · 단일 TP=1. 이 셀에서는 호스트 RAM이 처음으로 진짜 제약이 된다(47.6GB 단일 샤드).

(3) **모델 구조** — meta-models/Muse-Glimmer-30B(revision a4e59da · sha256 전수 검증) · BF16 59.6GB · **hybrid: sliding_attention 39층 + full_attention 13층** · 네이티브 컨텍스트 131,072 · 체크포인트는 **47.6GB 단일 샤드** 구성 — 이 사실이 셀의 모든 예산 문제의 뿌리다. 130,000+1,024=131,024 ≤ 131,072(여유 48토큰) 확인.

(4) **출발 시 예상 장벽** — 단일 샤드 × 작은 호스트 RAM의 서빙 예산 · hybrid 구조의 KV 양자화 지원 여부(계획 시 미지) · 130K 프리필의 열 부하.

(5) **외부 선례와 사람의 결정** — plan에서 Muse는 텍스트 전용(language_model_only)으로 서빙하기로 인터뷰 확정. 다운로드 시점은 32B 테스트 후로, 삭제는 테스트 완료 후로 정했다(최종적으로는 보관 결정으로 바뀜 — devlog_26101414 §7).

> [원문] plan_26101219_vllm_0_31_0_KV양자화_최대동시성_벤치_계획 §3. 실험 매트릭스 (실측 가능한 길이)
> | Muse-Glimmer-30B (~60GB, 멀티모달→**텍스트 전용** 서빙) | 131,072 | 7K, 30K, 130K | 5종 | 5 | 15 |

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-12T10:49:53Z(`camp-26101210-kvq-concurrency` · evidence.event_campaign_boundary): campaigns/camp-26101210-kvq-concurrency/campaign.yaml declared_utc 2026-10-12T10:49:53Z → 경계 = 앞선 값 2026-10-12T10:49:53Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-10-12T10:41:33Z → 2026-10-14T05:26:00Z(`docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-muse-glimmer-30b` | —(선언 행 없음) | 없음 | 없음 | —(닫힘 행 없음) | 3 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 47.6GB 단일 샤드가 서빙 예산 수학과 교착한다

Muse의 체크포인트는 47.6GB 단일 샤드다. discrete RAM 게이트는 최대 샤드를 호스트 상주 상한으로 보는데, 62GB 호스트 RAM에서 샤드(47.6GB)를 빼면 워치독 arm 상한과 벤치 밴드가 동시에 성립하지 않는다 — overhead를 올리면 arm 상한이 가드 최소 아래로 물러나고, 내리면 벤치 진입 밴드가 부족하다. 로드는 0초도 시작하지 않고 선판정이 멈췄다(rc=4). 해소는 사람 승인의 묘보호 경로다 — `single_serve_up.sh --no-budget`을 신설해 예산 선언·선판정·갱신 루프를 건��뛰되 budget-skip 이벤트로 기록하고, 열 워치독은 유지했다. 통과 판정은 이후 bf16 3개 길이 셀의 완주다.

```hint-event
id: W1
kind: wall
증상: 서빙 예산 선판정이 거부(rc=4) — 오버헤드를 어떻게 잡아도 arm 상한과 벤치 밴드가 동시에 성립하지 않음
서명: "⚠ **묘보호(--no-budget) 경로**: 47.6GB 단일 샤드 × 62GB RAM으로 서빙 예산 수학 교착"
원인: 47.6GB 단일 샤드가 62GB 호스트 RAM의 예산 산식(arm 상한 ↔ 벤치 밴드 트레이드오프)과 구조적으로 교착
해소: single_serve_up.sh --no-budget 신설(budget-skip 이벤트 기록 · 열 워치독 유지) + 사용자 승인
검증: bf16 7K·30K·130K 완주(testlog_26101413 §판정 표)
전이등급: arch-scaled
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §머리, devlog_26101414_muse_glimmer_30b_측정_서사 §3]
```

### W2 hybrid sliding-window는 KV 양자화 백엔드가 없다

tq4·tq3·nvfp4 트리플렛으로 기동을 시도했으나 셋 모두 엔진 초기화에서 죽었다. 원인은 열이 아니라 attention selector다 — hybrid sliding-window 층 구성에서 해당 KV dtype을 받는 백엔드가 업스트림에 없다. dtype 선택지는 bf16/fp8로 확정됐다.

```hint-event
id: W2
kind: wall
증상: tq4·tq3·nvfp4 기동이 EngineCore 초기화 실패로 즉사
서명: "구조적 미지원: `No valid attention backend … turboquant_4bit_nc`"
원인: hybrid sliding-window 층에서 tq·nvfp4 KV를 받는 attention backend가 업스트림에 없음(4B/32B의 nvfp4 sm_120 거부와는 별개 축)
해소: dtype 선택지를 bf16/fp8로 확정 — 업스트림 지원 대기
검증: testlog_26101413 §판정 표 · engine_exited 로그의 ValueError 행
전이등급: arch-locked
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```

> [원문] testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표
> | tq4 | ❌ | ❌ | ❌ | 구조적 미지원: `No valid attention backend … turboquant_4bit_nc` |
> | tq3 | ❌ | ❌ | ❌ | 동상 (`turboquant_3bit_nc`) |
> | nvfp4 | ❌ | ❌ | ❌ | 동상 (`nvfp4`) + sm_120 |

### W3 KV 예측 N부터 시작하는 갤럽이 서버를 즉사시킨다

fp8의 KV 토큰(330만)으로 계산한 프로브 시작 N은 411 — 엔진은 수용하지만 그 동시성의 연산 부하를 호스트가 못 버텨 약 4분 만에 열사망했다(탐색 시작→사망 약 253초). 이 패턴이 파일럿 자동모드에서 반복(2회 사망 → abort)돼, 측정은 --start-n으로 보수적 시작점(bf16 실측 N* 부근)을 주는 수동 방식으로 전환했다. 왜 통하는가: 열 천장 안에서 시작해 통과하면 올리고, 사망 지점이 곧 상한 증거가 된다.

```hint-event
id: W3
kind: wall
증상: fp8 7K N=411 gallop이 173초 만에 SoC hard kill — 파일럿 abort
서명: "fp8의 KV 예측 N(411)부터 시작하는 파일럿 갤럽이 즉사를 반복"
원인: KV 수용량으로 정한 시작 동시성이 호스트 열 천장을 크게 초과 — 이 모델은 KV가 풍부해 병목이 KV가 아니라 열
해소: --start-n 수동 지정(bf16 실측 N* 부근부터) · 셀 단위 수동 측정으로 전환
검증: fp8 7K N*=96 확정(testlog_26101413 §판정 표)
전이등급: judgment
출처: [devlog_26101414_muse_glimmer_30b_측정_서사 §4, testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: tq4·tq3 KV로 서빙 (웜 캐시 재시도 포함)
기각사유: 열이 아니라 attention selector 거부 — hybrid 구조의 구조적 미지원(W2). 4B/32B의 열사망과는 원인이 다르다
재개조건: vLLM 업스트림이 hybrid sliding-window 모델의 tq KV를 지원하는 버전
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```
```hint-event
id: R2
kind: rejected
시도: fp8 KV로 30K · 130K 측정 — N=48 → N=24로 낮춰 재시도
기각사유: 30K 프리필 부하에서 N=48·N=24 모두 SoC hard 사망 — fp8(FlashInfer)의 발열이 bf16(FLASH_ATTN)보다 커서 긴 입력을 못 버틴다
재개조건: 열 마진이 큰 호스트
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```
```hint-event
id: R3
kind: rejected
시도: 서빙 예산 overhead를 5022~8094MiB로 조정해 보호 상태 벤치 확보
기각사유: 수학적으로 답이 없음 — overhead를 올리면 arm 상한이 가드 최소를 하회하고, 내리면 벤치 밴드가 부족(W1). 안전장치를 깎지 않고 --no-budget 승인 경로를 택했다
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §머리, devlog_26101414_muse_glimmer_30b_측정_서사 §3]
```

**요약** — 이 셀의 형상은 세 기각으로 좁혀졌다: tq는 업스트림 미지원, fp8의 긴 입력은 열, 보호 상태 벤치는 RAM 수학 불가. 남은 것은 bf16 전 구간과 fp8 7K다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 fp8이 bf16보다 낫다는 직관 — 이 셀에서는 반대

4B/32B에서 fp8 KV는 동시성을 정확히 2배로 했기에, Muse에서도 이득이라는 기대가 자연스러웠다. 실측은 역전이었다 — 7K N*=96으로 bf16(166)의 58%다. 원인: 이 모델은 hybrid라 KV가 이미 풍부해(165만 토큰) 동시성 병목이 KV가 아니라 열이고, fp8의 백엔드가 bf16의 백엔드보다 발열이 크다(testlog_26101413 §구조 판정 2번). 양자화 이득은 KV가 병목일 때만 성립한다.

```hint-event
id: M1
kind: misdiagnosis
증상: fp8 KV 적용 — 동시성이 bf16보다 낮게 측정
서명: "FlashInfer 백엔드가 FLASH_ATTN보다 발열이 커, KV 2배 이득이 이 호스트 열 천장(105°C hard)에 전부 상쇄"
원인: KV가 풍부한 모델에서 양자화 이득이 성립한다는 직관 — 실제 병목은 KV가 아니라 열
해소: dtype 평가는 KV 토큰 여유와 열 프로파일을 함께 본다 — bf16이 이 셀의 최선
검증: testlog_26101413 §판정 표 · §구조 판정
전이등급: judgment
현재지위: 반증
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §구조 판정]
```

> [원문] testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §구조 판정
> 2. **fp8은 7K에서조차 bf16보다 낮다**(96 < 166) — FlashInfer 백엔드가 FLASH_ATTN보다 발열이 커, KV 2배 이득이 이 호스트 열 천장(105°C hard)에 전부 상쇄. 30K 프리필에서는 N=24도 불가.

### M2 Muse 측정 초기의 게이트 연쇄 실패를 벽 오진으로 읽다

첫 파일럿(r2~r4)은 전 dtype이 기록상 "실패"였으나, 그 대부분은 측정 불가가 아니라 게이트 사슬이었다 — 예산 선판정 rc=4(overhead 16384 고정) → 벤치 예산 게이트 rc=3(벤치 8192MiB 부족) → arm 상한 rc=4(overhead 8094 과다). 원인은 하네스 파라미터의 고정값이 47.6GB 샤드 모델을 가정하지 않은 것이고, W1의 --no-budget과 파라미터화(OVERHEAD_MIB · BENCH_BUDGET_MIB)로 해소됐다. 실패 기록은 그대로 보존(_gateblocked_r4 디렉터리)해 "측정 0"이 아니라 "게이트 차단"으로 귀속한다.

```hint-event
id: M2
kind: misdiagnosis
증상: 파일럿 r2~r4 전 셀 실패 — bf16조차 측정 불가로 보임
서명: "47.6GB 단일 샤드 × 62GB RAM으로 서빙 예산 수학 교착(arm 상한과 벤치 밴드가 트레이드오프)"
원인: 하네스 고정 파라미터(overhead 16384 · 벤치 8192)가 단일 대형 샤드 모델을 가정하지 않음 — 모델 문제가 아니라 게이트 교착
해소: --no-budget 경로 + 파일럿 드라이버 파라미터화 · 게이트 차단 잔재는 별도 보존
검증: r5(bf16 완주) · testlog_26101413 §판정 표
전이등급: arch-invariant
현재지위: 반증
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §머리, devlog_26101414_muse_glimmer_30b_측정_서사 §3]
```

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 클램프(V4)** — bf16 무클��프 부팅 실측 Available KV 29.17 GiB = **1,654,121 tokens**. 같은 VRAM의 4B(553,840)보다 3배 많은데, 이유는 full-attention 층이 13/52뿐인 hybrid 구조다 — KV를 지는 층이 적을수록 토큰당 비용이 줄어 같은 바이트에 더 많이 들어간다. fp8이면 약 2배(3,308,637 — 4B/32B와 달리 hybrid 블록 배수라 정확한 정수배는 아니다 · probe 관측값). 단, 이 풍부함이 fp8 이득을 무의미하게 만든다(M1).

**max-model-len 131072(V2)** — 130,000+1,024=131,024 ≤ 131,072의 여유 48토큰. 토크나이저 오차에 민감하다는 plan의 주의가 있었고, 실측으로 130K 셀이 성립함을 확인했다.

**language-model-only(V5)** — 멀티모달 모델을 텍스트 전용으로 서빙하는 결정. 이 노브가 서빙 형상을 정의한다.

> [원문] testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표
> | bf16(auto) | **166** ✅ | **52** ✅ | **12** ✅ | 130K: N=12 ×3회 성공, N=15 열사망(상한 증거) |
> | fp8 | **96** ✅ | ❌ 열 한계 | ❌ 열 한계 | 7K N=96 ×3회, N=120 사망 · 30K N=48·24 사망 |

**버티는 기전 대 실패 기전** — bf16 형상: FLASH_ATTN · KV 풍부 · 130K에서도 N=12까지 생존(130K N=15는 열사망이 상한) — **확정**(§판정 표). fp8 형상: FlashInfer 발열로 7K조차 bf16보다 낮고 30K는 불가 — **확정**(§판정 표). tq·nvfp4: selector 거부 — **확정**(W2).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- KV 예측 N부터 갤럽을 시작하지 말 것 — W3 — KV가 풍부한 모델은 --start-n을 보수적으로
- 단일 대형 샤드 모델을 고정 overhead로 기동하지 말 것 — W1 — 샤드+RAM으로 예산 성립을 먼저 계산하고, 교착이면 승인 경로
- fp8이 항상 이득이라고 가정하지 말 것 — M1 — KV 토큰 여유를 먼저 본다(풍부하면 양자화 이득 없음)
- 게이트 차단을 측정 실패로 기록하지 말 것 — M2 — 차단 잔재를 보존하고 귀속을 구분한다
- tq·nvfp4를 hybrid 모델에 시도하지 말 것 — W2 — attention selector 지원을 먼저 확인
- 멀티모달 모델을 language-model-only 없이 텍스트 측정하지 말 것 — V5 — 형상이 달라진다
- 130K를 plan 산식(여유 48토큰) 확인 없이 요청하지 말 것 — V2 — 모델 상한+출력 길이를 먼저 더해 본다
- N*+1 증거 없이 상한을 선언하지 말 것 — bf16 130K는 N=12 ×3회 + N=15 열사망을 함께 기록(testlog_26101413 §판정 표)

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 업스트림이 hybrid sliding-window 모델의 tq·nvfp4 KV를 지원하게 되는가 — 지원되면 이 셀의 dtype 선택지가 열린다
현재상태: v0.31.0 attention selector 거부 확인 · 후속 버전 미확인
다음관측: 신버전 릴리스 노트의 hybrid·sliding-window KV dtype 지원 여부
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```
```hint-event
id: Q2
kind: open-question
물음: bf16 130K의 진짜 상한은 어디인가 — N=12와 N=15 사이(13~14)는 미측정
현재상태: N=12 ×3회 통과 · N=15 열사망 — 사이 구간은 열과 용량이 혼재해 미증명으로 기재
다음관측: 열 마진이 큰 호스트에서 N=13·14 확인 런
출처: [testlog_26101413_muse-glimmer-30b_KV양자화_동시성_판정 §판정 표]
```
```hint-event
id: Q3
kind: open-question
물음: --no-budget 경로의 장기 지위 — Muse를 상시 서빙한다면 RAM 예산 없이 운영하는 것이 적절한가
현재상태: 측정 한정 승인으로 사용 · 운영 경로는 미결정
다음관측: 운영 전환 시 RAM 증설 또는 샤드 분할 재패키징 여부 결정
출처: [devlog_26101414_muse_glimmer_30b_측정_서사 §3]
```

해설 — Q1은 형상 선택지, Q2는 상한 해상도, Q3는 운영 안전의 문제다. 어느 것도 이 측정의 유효성을 흔들지 않는다.
