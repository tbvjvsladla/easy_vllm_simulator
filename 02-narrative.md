# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 10건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26090807_qwen3_4b_tq_kvquant_서브.md` | testlog | 26090807 | 0 | 3910 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 2 | `docs/testlog/testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b.md` | testlog | 26101015 | 0 | 3760 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 3 | `docs/devlog/devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소.md` | devlog | 26101108 | 0 | 3766 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 4 | `docs/plan/plan_26102718_Qwen3_4B_c_tq4nc_신규_Full_hint.md` | plan | 26102718 | 0 | 6843 | 0.26.0 · c-tq4nc · easy-vllm:0.26.0-cu132-aarch64-source · tq4nc | — |
| 5 | `docs/testlog/testlog_26101108_29_02_셀D_qwen3-4b_hint_자체발행.md` | testlog | 26101108 | 0 | 2480 | 0.26.0 · c-tq4nc · tq4nc | — |
| 6 | `docs/devlog/devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사.md` | devlog | 26102718 | 0 | 12974 | 0.26.0 · c-tq4nc · tq4nc | — |
| 7 | `docs/testlog/testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정.md` | testlog | 26102719 | 0 | 10525 | 0.26.0 · c-tq4nc · tq4nc | — |
| 8 | `docs/benchmark/bench_report_26101015_qwen3-4b_GB10_0.26.0.md` | bench_report | 26101015 | 0 | 7032 | 0.26.0 · easy-vllm:0.26.0-cu132-aarch64-source | — |
| 9 | `docs/benchmark/bench_report_26102719_qwen3-4b_GB10_0.26.0.md` | bench_report | 26102719 | 0 | 6261 | 0.26.0 · easy-vllm:0.26.0-cu132-aarch64-source | — |
| 10 | `docs/plan/plan_26101104_hint발행체제_개편_서브자체발행.md` | plan | 26101104 | 0 | 11774 | — | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

이 계보의 가장 이른 포함 모델 실측은9-08의 GB10 위 Qwen3-4B KV양자화 대조이며24GiB 캡은 이식 시뮬레이션이었다. 포함된 가장 이른 plan은10-11의 발행체제 개편이고, 원 모델 실험의 첫 계획이라고 부르지 않는다. 그 plan은 서브에 있는 관측과 발행 소유를 연결했다.

> [원문] plan_26101104_hint발행체제_개편_서브자체발행 §2. 결정
> 5. 서브 자체 발행 = **single 서브 한정**. 워크스페이스 git 밖 **별도 hint 클론**에서 태그 1개 push. 워크스페이스 git 은 local-only 유지.

이번 목적은 기존8모델 배정 중 이 셀 하나의 신규 Full과 새 hint다. HW는 GB10×1/노드1, MemTotal121.7GiB 통합메모리, TP1이다. NIC/RoCE는 manifest에 있지만 이 측정은 분산 실행이 아니다. 드라이버580.178.04는 manifest 선언이고 직접 nvidia-smi 관측은 실패해 따로 구분했다.

로컬 safetensors 헤더/카드 교차검증은 BF16398텐서/세 샤드,4,022,468,096 parameters,36층/32Q/8KV, 저장7.49GiB를 보고했다. 엔진 weights7.56GiB는 다른 측정 표면이다. MTP/spec는 선언하지 않았고 PLE 모델이 아니다. checkpoint 식별과 checkout 한계는0.4를 읽는다.

장벽은 W1~W5 순서다. 외부 원 모델카드는 chat/thinking/tools 근거이고, 이번 조건에 맞는 속도 E는 빈손이었다. 다른 checkpoint/HW 수치를 가져와 목표를 만들지 않았으며, 기존 사용자 승인을 재사용했다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-27T10:07:20Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `single8-qwen3-4b-a01`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-27T09:40:08Z | 1회 · 첫 2026-10-27T09:40:09Z | 없음 | `budget_clear` 2026-10-27T10:06:26Z | 4 |

**캠페인 경계 이전 기동 3회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀 라벨로 묶인 2회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 1회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-c-tq4nc` | 2026-09-07T20:06:36Z | 3회 · 첫 2026-09-07T20:06:37Z | 없음 | `budget_clear` 2026-09-07T21:12:30Z | 4 |
| 앞선-2 | sub | `camp-26090814.cell-c-tq4nc.rerun`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-09-08T06:39:09Z | 3회 · 첫 2026-09-08T06:39:10Z | 없음 | `budget_clear` 2026-09-08T07:41:39Z | 4 |
| 앞선-3 | sub | `serve-c-tq4nc` | 2026-10-10T05:33:14Z | 2회 · 첫 2026-10-10T05:33:15Z | 없음 | `budget_clear` 2026-10-10T06:26:39Z | 4 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 이전 source-build 의존 핀 충돌

과거 기록은 낡은 transformers<5 핀이0.26과 충돌했다고 적었다. 당시 regen_requirements.py로 wheel METADATA에서 요구를 다시 생성했다. 이번 requirements 사본 불일치는 별도 재현 결손이다.

```hint-event
id: "W1"
kind: "wall"
증상: "이전 source-build 의존 핀 충돌"
서명: "- 빌드 특이사항: ① build_patches{,_src} 빈 컨텍스트(.gitkeep, stock no-op) ② requirements.txt 를 regen_requirements.py 로 v0.26.0 wheel METADATA 에서 재생성(transformers>=5.5.3,<5.15.0 — 침체본의 `<5` 핀이 0.26 과 충돌했음)"
원인: "과거 기록은 낡은 transformers<5 핀이0.26과 충돌했다고 적었다."
해소: "당시 regen_requirements.py로 wheel METADATA에서 요구를 다시 생성했다. 이번 requirements 사본 불일치는 별도 재현 결손이다."
검증: "testlog_26090807_qwen3_4b_tq_kvquant_서브 §환경 스냅샷"
전이등급: "arch-locked"
출처: ["testlog_26090807_qwen3_4b_tq_kvquant_서브 §환경 스냅샷"]
```

> [원문] testlog_26090807_qwen3_4b_tq_kvquant_서브 §환경 스냅샷
> - 빌드 특이사항: ① build_patches{,_src} 빈 컨텍스트(.gitkeep, stock no-op) ② requirements.txt 를 regen_requirements.py 로 v0.26.0 wheel METADATA 에서 재생성(transformers>=5.5.3,<5.15.0 — 침체본의 `<5` 핀이 0.26 과 충돌했음)

### W2 합성 backend 값 거부

과거 호출은 backend 값에 reason을 합쳐 값역 검사를 통과하지 못했다. openai-chat 값과 backend=chat_template reason을 분리했고 이번에도 그 정식 인자를 사용했다.

```hint-event
id: "W2"
kind: "wall"
증상: "합성 backend 값 거부"
서명: "- 첫 호출 `--backend openai-chat+chat_template` 은 `알 수 없는 --backend (openai|openai-chat)` 로 거부됨. 지시문의 `+chat_template` 은 backend 값이 아니라 reason 코드 → `--backend openai-chat --knob-reason backend=chat_template` 로 수정."
원인: "과거 호출은 backend 값에 reason을 합쳐 값역 검사를 통과하지 못했다."
해소: "openai-chat 값과 backend=chat_template reason을 분리했고 이번에도 그 정식 인자를 사용했다."
검증: "testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건"
전이등급: "judgment"
출처: ["testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건"]
```

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건
> - 첫 호출 `--backend openai-chat+chat_template` 은 `알 수 없는 --backend (openai|openai-chat)` 로 거부됨. 지시문의 `+chat_template` 은 backend 값이 아니라 reason 코드 → `--backend openai-chat --knob-reason backend=chat_template` 로 수정.

### W3 이전 하네스 제한으로 스윕 중단

과거 기록은 레벨8 중 하네스 백그라운드10분 제한 종료라고 적었다. 과거 setsid nohup 재실행은 역사 기록이며 이번 권한으로 승계하지 않았다. 이번에는 wall-time3600초의 연결된 공식 실행에서 새 raw를 완주했다.

```hint-event
id: "W3"
kind: "wall"
증상: "이전 하네스 제한으로 스윕 중단"
서명: "- 첫 풀 스윕이 레벨 8 도중 하네스 백그라운드 10분 제한으로 종료됨 → 같은 설정으로 `setsid nohup` 재실행하여 완주(최종 수치는 재실행분). 서빙은 유지한 채 재측정."
원인: "과거 기록은 레벨8 중 하네스 백그라운드10분 제한 종료라고 적었다."
해소: "과거 setsid nohup 재실행은 역사 기록이며 이번 권한으로 승계하지 않았다. 이번에는 wall-time3600초의 연결된 공식 실행에서 새 raw를 완주했다."
검증: "testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건"
전이등급: "judgment"
출처: ["testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건"]
```

> [원문] testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건
> - 첫 풀 스윕이 레벨 8 도중 하네스 백그라운드 10분 제한으로 종료됨 → 같은 설정으로 `setsid nohup` 재실행하여 완주(최종 수치는 재실행분). 서빙은 유지한 채 재측정.

### W4 관측이 서브에 있는 발행 경계

과거 main-only 발행기로는 서브 로컬 serve 관측을 확인할 수 없었다. 10-11 계획은 single 서브가 기존 승인·별도 clone·독립 검증으로 자기 셀을 발행하고 카탈로그는 메인이 소유하도록 했다. 새 캠페인은 그 승인 운반체를 보존한다.

```hint-event
id: "W4"
kind: "wall"
증상: "관측이 서브에 있는 발행 경계"
서명: "| P3 | 서브에서만 잰 셀은 발행 불가 | C Ornith · D Qwen3-4B | `HINT_QUALIFICATION_UNOBSERVED`(D-e) — 발행은 main-only 인데 serve 관측 원시는 서브 밖으로 못 나온다 |"
원인: "과거 main-only 발행기로는 서브 로컬 serve 관측을 확인할 수 없었다."
해소: "10-11 계획은 single 서브가 기존 승인·별도 clone·독립 검증으로 자기 셀을 발행하고 카탈로그는 메인이 소유하도록 했다. 새 캠페인은 그 승인 운반체를 보존한다."
검증: "plan_26101104_hint발행체제_개편_서브자체발행 §1. 무엇이 문제였나"
전이등급: "judgment"
출처: ["plan_26101104_hint발행체제_개편_서브자체발행 §1. 무엇이 문제였나"]
```

> [원문] plan_26101104_hint발행체제_개편_서브자체발행 §1. 무엇이 문제였나
> | P3 | 서브에서만 잰 셀은 발행 불가 | C Ornith · D Qwen3-4B | `HINT_QUALIFICATION_UNOBSERVED`(D-e) — 발행은 main-only 인데 serve 관측 원시는 서브 밖으로 못 나온다 |

### W5 적용 가능한 외부 E 공백

이번 검색 후보의 모델/HW/runtime/부하가 달라 수치 E로 채택하지 않았다. e-search empty와 weak 루프라인 fallback을 공식 판정에 남겼다. 외부 E가 있다고 바꾸지 않았다.

```hint-event
id: "W5"
kind: "wall"
증상: "적용 가능한 외부 E 공백"
서명: "2026-10-27 실제7질의·출처는 external_search.json. 원 Qwen3 모델카드는 chat/thinking/tools 근거로 채택했다. H20 벤더 속도표, GB10의 다른 Instruct-2507/NVFP4, 다른 runtime/부하 후보는 E로 채택하지 않았다. 맞는 E를 못 얻어 weak 공식 fallback을 유지했다."
원인: "이번 검색 후보의 모델/HW/runtime/부하가 달라 수치 E로 채택하지 않았다."
해소: "e-search empty와 weak 루프라인 fallback을 공식 판정에 남겼다. 외부 E가 있다고 바꾸지 않았다."
검증: "testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §외부 검색"
전이등급: "judgment"
출처: ["testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §외부 검색"]
```

> [원문] docs/testlog/testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정.md §외부 검색
> 2026-10-27 실제7질의·출처는 external_search.json. 원 Qwen3 모델카드는 chat/thinking/tools 근거로 채택했다. H20 벤더 속도표, GB10의 다른 Instruct-2507/NVFP4, 다른 runtime/부하 후보는 E로 채택하지 않았다. 맞는 E를 못 얻어 weak 공식 fallback을 유지했다.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: "R1"
kind: "rejected"
시도: "과거 합성 backend openai-chat+chat_template 호출"
기각사유: "값역 검사 거부 후 backend와 reason을 분리했다; 이번 신규 실패로 세지 않음"
출처: ["testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §4. 운영 사건"]
```

```hint-event
id: "R2"
kind: "rejected"
시도: "이번 외부검색 후보의 속도를 수치 E로 채택할지 검토"
기각사유: "다른 Instruct-2507/NVFP4/H20 또는 다른 runtime/부하라 이 recipe의 E로 쓰지 않았다; 실행 실패가 아님"
출처: ["testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §외부 검색"]
```

과거3bit 자매 셀은 성공 관측이므로 기각 축으로 바꾸지 않는다. 이번에 새 PLE/offload/다른 runtime을 실행하지 않았다. 반증되지 않은 축을 실패라고 적지 않는다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

과거0.0% 밴드를 완전한 재현성 보증으로 읽을 근거는 없다. 아래 원 주장 자체가 가능성이라고 적힌 가설이며 원인은 미확정으로 남았다.

```hint-event
id: "M1"
kind: "misdiagnosis"
증상: "과거 판정점 decode 세 값이 소수2자리까지 같음"
서명: "- ⚠ **판정점 runs 3회가 22.44 / 22.44 / 22.44 로 소수 2자리까지 완전 동일**(밴드 0.0 %). 원시 파일은 run 마다 md5·시각이 달라 별개 실행이고, 같은 실행들의 output_throughput(22.475 / 22.447 / 22.443)·tpot 은 서로 다르다. `decode_tps` 칸의 해상도 효과일 가능성이 있으나 **가설이며 원인은 미확정**이다. 다른 레벨 밴드는 0.04~0.12 %."
원인: "반올림/해상도 설명은 가설이며 원인 대조가 없다"
해소: "다른 raw 지표가 다르다는 관측과 구분하고 해상도 가설을 확정 처방으로 쓰지 않음"
검증: "devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §측정"
전이등급: "judgment"
현재지위: "가설(강등)"
출처: ["devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §측정"]
```

> [원문] docs/devlog/devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소.md §측정
> - ⚠ **판정점 runs 3회가 22.44 / 22.44 / 22.44 로 소수 2자리까지 완전 동일**(밴드 0.0 %). 원시 파일은 run 마다 md5·시각이 달라 별개 실행이고, 같은 실행들의 output_throughput(22.475 / 22.447 / 22.443)·tpot 은 서로 다르다. `decode_tps` 칸의 해상도 효과일 가능성이 있으나 **가설이며 원인은 미확정**이다. 다른 레벨 밴드는 0.04~0.12 %.

과거 자체발행 testlog의 F1~F8 교정 묶음을 훑었다. 그중 F5/F6은 producer 교정+refacts, F1~F4/F7/F8은 산문 정정으로 기록돼 있다. 세부 항목별 원 주장이 이 목록에 전부 실리지 않아 각 인과를 새로 만들지 않는다. 이 셀 관련 요지는 M1과 당시 엔진 peak/decode 비율이 다른 시간 창이라는 정정이다. 이번 부하에 그 peak를 옮기지 않는다(testlog_26101108_29_02_셀D_qwen3-4b_hint_자체발행 §사실 검증 요약).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

V1~V5는 이전 triplet에서 유지했다. gmu/문맥/시퀀스/KV/dtype를 이번에 최적화하지 않았다. 24GiB 이식 실험의 clamp를 GB10에서 재검증한 것이며 다른 GPU의 적정값이라는 증거는 없다.

> [원문] docs/devlog/devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사.md §기동과 기능
> 공식 single_serve_up 호출09:40:08Z–09:42:35Z, ready145초(상한900), overhead13508MiB를 명시했다. overhead는 사전 baseline5316MiB+runtime allowance8192MiB의 **선언**이고 실측 엔진 overhead가 아니다. 실제 NAS RAM gate required17913MiB 대비119257MiB로 통과했다. blackbox floor92164MiB/arm89092MiB는 보호 예산 산식의 선언이다. 안전 값을 끄거나 낮추지 않았다.

원시 single-up의 선언 산식은 총124608−weights7672−KV11264−overhead13508=바닥92164MiB다. overhead13508은 baseline5316+allowance8192의 선언이다. 엔진은 고정KV11GiB와232,992토큰을 보고했다. 요청 길이 대비 KV 여유가 있다는 관측은 있지만 필요한 최소 KV 바이트/배수는 이번에 sweep하지 않아 미관측이며 새 공식 계산으로 채우지 않는다. graph는 기존 기본 형상이고 캡처 대 eager 대조는 하지 않았다.

같은 계보3bit 대4bit는 dtype 전체가 다르며 압축 메타/연산 비용 기전은 제안된 설명이지 이번 대조 확정이 아니다. PLE/offload 실패 기전은 이번 관측이 없다. spec off여서 루브릭 accept_len1.0은 선언 부재의 정의값이고 실측 acceptance length가 아니다. E와 사용자 목표는 없다.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- backend 값에 reason을 합치지 않는다 — W2/R1 — 정식 값과 reason을 분리한다.
- 과거 하네스 분리 재실행 권한을 물려받지 않는다 — W3 — 현재 예산과 새 원시 경로를 쓴다.
- 원 모델과 다른 체크포인트의 수치를 E로 옮기지 않는다 — W5/R2 — 조건을 검색·기록한다.
-0.0% 밴드를 완벽 재현성이라고 부르지 않는다 — M1 — raw 지표와 해상도 한계를 분리한다.
- 설치 문자열을 빌드 입력 ref로 치환하지 않는다 — 00§0.4 — history 입력과 runtime 관측을 나란히 둔다.
- gmu0.9를 실제 상주량으로 읽지 않는다 — devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §기동과 기능 — 고정KV와 profiling skip 로그를 확인한다.
- 다른 세션을 넓게 정리하지 않는다 — testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §정리 — 이번 session-id만 지정한다.
- requirements 참고 사본을 원 이미지 바이트라고 부르지 않는다 —01§1.1 — 불일치 해소 후 재빌드/스모크한다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: "Q1"
kind: "open-question"
물음: "높은 동시성의 parser 완료18과 선언16의 집계 차이 원인은 무엇인가"
현재상태: "이번 레벨4/8은 각18, 서버 증가분18, 워밍업 선언2; 원인 미확정"
다음관측: "동일 run의 GuideLLM warmup 및 집계 경계를 요청별로 대조"
출처: ["testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §이번 실행"]
```

```hint-event
id: "Q2"
kind: "open-question"
물음: "runtime source checkout과 실제 컴파일 바이트가 동일한가"
현재상태: "모듈 자기보고·checkout·source describe는 수집됐으나 build ledger와 compiled-byte attestation은 미관측이다. 현재 session/window 결속과는 별개다."
다음관측: "별도 owner의 빌드 원장·컴파일 입력과 실제 이미지 바이트 결속을 확인한다. 이번에는 재빌드하지 않는다."
출처: ["devlog_26102718_Qwen3_4B_c_tq4nc_신규실측_서사 §레시피·모델·이미지", "testlog_26102719_Qwen3_4B_c_tq4nc_신규_Full_판정 §공식 판정과 출처"]
```

```hint-event
id: "Q3"
kind: "open-question"
물음: "상위 runtime/다른 GPU에서 같은 KV와 source 트랙이 적절한가"
현재상태: "이번에는 다른 runtime·HW·KV최소값·멀티노드 ABI를 재지 않았다"
다음관측: "별도 승인 범위에서 upstream 지원/핀과 최소 smoke/메모리 관측"
출처: ["plan_26102718_Qwen3_4B_c_tq4nc_신규_Full_hint §발행과 정리"]
```
