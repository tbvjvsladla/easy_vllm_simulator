# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 14건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인.md` | plan | 26100107 | 0 | 13097 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source · granite42 · granite42-30b-v030 · v030 | — |
| 2 | `docs/testlog/testlog_26100117_하네스결함교정_재빌드일관성_E2E_판정.md` | testlog | 26100117 | 0 | 8955 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source · granite42 · granite42-30b-v030 · v030 | — |
| 3 | `docs/testlog/testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정.md` | testlog | 26102710 | 0 | 8300 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source · granite42 · granite42-30b-v030 · v030 | — |
| 4 | `docs/devlog/devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030.md` | devlog | 26101017 | 0 | 2090 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 5 | `docs/testlog/testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정.md` | testlog | 26101017 | 0 | 3355 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 6 | `docs/plan/plan_26102700_코덱스_싱글8종_멀티4종_실측.md` | plan | 26102700 | 0 | 6647 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 7 | `docs/devlog/devlog_26102710_granite42_30b_v030_코덱스_메인_Full_여정.md` | devlog | 26102710 | 0 | 2130 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 8 | `docs/plan/plan_26101017_knob_E2E_hint발행.md` | plan | 26101017 | 0 | 3616 | 30b · granite42 · granite42-30b-v030 · v030 | — |
| 9 | `docs/benchmark/bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md` | bench_report | 26101016 | 0 | 5952 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source | — |
| 10 | `docs/benchmark/bench_report_26102710_granite-4.2-30b_GB10_0.30.0.md` | bench_report | 26102710 | 0 | 6363 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source | — |
| 11 | `docs/testlog/testlog_26100111_v030_싱글_이종모델_동시서빙_서브지침배달_판정.md` | testlog | 26100111 | 1 | 7294 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source · granite42 · granite42-30b-v030 · v030 | — |
| 12 | `docs/plan/plan_26100113_v030캠페인_후속_하네스결함교정_공통층동기화_서브정비.md` | plan | 26100113 | 1 | 14385 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 13 | `docs/plan/plan_26101011_벤치_knob_프리셋_자산화.md` | plan | 26101011 | 1 | 17408 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 14 | `docs/testlog/testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정.md` | testlog | 26101016 | 1 | 8811 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

첫 plan_26100107은0.30.0이미지와서브지침배달검증을위해메인Granite-4.2-30B·서브Ornith를선택했고Granite는lite까지만예정했다(§2·§8). 이후plan_26101011의벤치knob프리셋E2E6셀중A로기존레시피의첫GuideLLM Full을측정했다. 이번plan_26102700은Claude/Codex공통하네스전파후메인·서브서로다른8모델각각신규Full성공과hint발행을요구한후속이다. 과거Full을이번성공으로대신하지않는다.

> [원문] plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §2. 사용자 결정
> | U1 | 싱글: 메인 **Granite-4.2-30b**(dense · bf16 54.5GiB) · 서브 **Ornith-1.5-35B-A3B**(하이브리드 MoE · bf16 67.0GiB) — NAS 에서 노드 1대 적재 가능한 최신 두 계열 |

GB10 GPU1·노드1·TP1·통합메모리이며노드간인터커넥트는사용하지않는다. HF ibm-granite/granite-4.2-30b는GraniteForCausalLM·64층fullGQA·kv_heads8·head_dim128·네이티브131072, bf16비양자화dense다. 체크포인트revision은00 §0.2정본이며토큰당KV256KiB는계보산식이다. safetensors index54.53GiB와현재엔진checkpoint54.53GiB/적재54.58GiB는방법이구분되는관측이다. PLE·spec는선언하지않았다. parser는모델폴더granite_thinking_parser.py와qwen3_coder다(plan_26100107 §3).

노드 MemTotal 124608MiB는 이번 기동 러너의 관측이며 가중치·KV·overhead로 만든 보호 선언과 구분한다(01 §1.4). 현재 driver580.178.04는 manifest 선언이고 독립 현장 드라이버 재측정으로 부르지 않는다. 총/활성 파라미터의 독립 계수는 계보에 미기록이다. dense 구조상 둘이 같다는 구조 해석과 모델명30B를 정확 계수 실측으로 섞지 않는다. 이번 기능 검증은 completion이며 MTP나 도구 기능을 추가 입증한 것은 아니다.

그plan의D4는멀티이미지재사용계획이었다. 실제후속testlog_26100117 §1C2·§2는사용자재빌드선택후빈패치슬롯single렌더로--no-cache40분빌드, 이미지123c331f…생성을기록한다. 멀티cdb1a38b…계획과실제single이미지를같다고쓰지않는다. 이번기존이미지재사용의구체digest/빌드원장/실린바이트검증지위는01 FACT를따른다.

당시예상벽은dense대역폭추정(~5t/s),131072수용KV,호스트와클라이언트의통합예산이었다. 이번외부동일GB10 BF16 decode E는bounded검색결과없고숫자사용자목표도없어weak expected기준을사용한다(testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §3). 추정상한을외부실측으로전환하지않는다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-26T21:46:47Z(`camp-26102705-codex-single8` · evidence.event_campaign_boundary): campaigns/camp-26102705-codex-single8/campaign.yaml declared_utc 2026-10-26T21:46:47Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-26T23:07:07Z → 경계 = 앞선 값 2026-10-26T21:46:47Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-27T01:59:43Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-granite42-30b-v030` | 2026-10-26T23:07:07Z | 17회 · 첫 2026-10-26T23:07:09Z | 없음 | `budget_clear` 2026-10-27T01:22:48Z | 12 |

**캠페인 경계 이전 기동 7회**(캠페인 경계 2026-10-26T21:46:47Z 이전 — 이 셀 라벨로 묶인 3회 · 라벨 다름(같은 노드 · 창 밖 이 셀 행과 시간이 겹치는 다른 라벨 선언 창) 4회 · 어느 쪽도 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | `camp-26100110-granite`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:42:26Z | 1회 · 첫 2026-10-01T01:42:27Z | 없음 | `budget_clear` 2026-10-01T01:49:52Z | 5 |
| 앞선-2 | main | `camp-26100110-granite-try2`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:50:26Z | 1회 · 첫 2026-10-01T01:50:28Z | 없음 | `budget_clear` 2026-10-01T01:57:22Z | 5 |
| 앞선-3 | main | `camp-26100110-granite-try3`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:57:43Z | 1회 · 첫 2026-10-01T01:57:44Z | 없음 | `budget_clear` 2026-10-01T02:04:49Z | 5 |
| 앞선-4 | main | `camp-26100110-granite-try4`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:05:04Z | 1회 · 첫 2026-10-01T02:05:05Z | 없음 | `budget_clear` 2026-10-01T02:23:54Z | 6 |
| 앞선-5 | main | `serve-granite42-30b-v030` | 2026-10-01T08:26:06Z | 1회 · 첫 2026-10-01T08:26:08Z | 없음 | `budget_clear` 2026-10-01T08:41:10Z | 6 |
| 앞선-6 | main | `serve-granite42-30b-v030` | 2026-10-10T04:55:14Z | 1회 · 첫 2026-10-10T04:55:15Z | 없음 | `budget_clear` 2026-10-10T05:07:27Z | 5 |
| 앞선-7 | main | `serve-granite42-30b-v030` | 2026-10-10T05:07:56Z | 17회 · 첫 2026-10-10T05:07:57Z | 없음 | `budget_clear` 2026-10-10T07:15:02Z | 6 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

W1·W2는10월1일기동계보, W3는10월10일벤치프리셋계보다. 이번캠페인에서새로재현한실패가아니다. 현재기동시도·갱신·닫힘은위FACT와01원장의이번캠페인경계를따른다.

### W1 16GiB KV로 native 길이를 수용하지 못함

과거 첫 클램프는 목표 한 시퀀스의 산식보다 작아 엔진이 기동을 거부했다. 이 회차 자체의 전량 엔진 원문은 이 계보에 없고 yaml 주석과 판정 문서가 기록한 거부를 사용한다. 다음 용량으로 늘린 경로가 W2이며 호스트 사살로 바꾸어 부르지 않는다.

```hint-event
id: W1
kind: wall
증상: 과거KV절대클램프16GiB에서131072길이로기동거부
서명: "kv_cache: 33.0 GiB 절대 클램프 — 131072 1시퀀스 + 블록 올림 여유(try1 16GiB 거부"
원인: 토큰당KV256KiB와131072길이의산식은32GiB로16GiB보다큼
해소: KV클램프를늘림; 최종33GiB는W2
검증: 현재엔진도33GiB·135168토큰·131072기준1.03x를보고하며health성공
전이등급: arch-invariant
출처: [granite42-30b-v030.yaml, plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §4]
```

### W2 32GiB에서도 수용 토큰이 16개 부족함

다음32GiB는 산식상 한 시퀀스와 같았지만 엔진 계산은131056으로 거부됐다. 33GiB에서 수용135168을 보고하며 통과한 것은 관측이고, 16토큰이 빠진 이유는 미결이다. 여유를 더한 관측을 모든 환경의 정확한 최소값으로 일반화하지 않는다.

```hint-event
id: W2
kind: wall
증상: 과거32GiB에서수용131056토큰으로목표131072보다16토큰부족해기동거부
서명: "131056 토큰 = 16 토큰 미달 거부"
원인: 엔진수용검사의16토큰미달은관측; 빠진기전은미결이며블록정렬은yaml가설
해소: kv-cache-memory-bytes35433480192로여유를늘려기동
검증: 현재엔진135168토큰과health200; 이번새클램프조정은없음
전이등급: arch-invariant
출처: [granite42-30b-v030.yaml, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §4]
```

과거yaml주석의try번호와10월1일testlog번호는서로다를수있어용량과서명으로식별한다. 필요한정확여유의기전이나최소33GiB를일반필수조건으로단정하지않는다.

### W3 벤치8192MiB가 예산 사전검사에 막힘

10월10일의 첫 Full 시도는 lite 뒤 레벨1을 시작하기 전에 클라이언트 예산으로 막혔다. 가드는 허용된 프리셋 선택지4096으로 낮춘 다음 완주를 허용했으며, 이번도 그4096을 사용했다. 측정 전에 차단됐으므로8192에서 실제 서빙이 죽었다고 쓰지 않는다.

```hint-event
id: W3
kind: wall
증상: 과거bench_budget_mib8192에서레벨1측정전bench=BLOCK
서명: "벤치 진입 불가: 바닥 16543MiB − 벤치 8192MiB = 8351MiB < 절대밴드 10240MiB"
원인: 선언바닥에서클라이언트몫을빼면보호절대밴드보다작음; 상한6303MiB
해소: 프리셋허용선택지4096으로다시선택해Full완주
검증: 과거Full기록과이번bench4096신규FullPASS; 8192벤치는측정전에막혀실제GPU사살관측없음
전이등급: arch-scaled
출처: [docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log, testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §4, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §2]
```

> [원문] docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log §L9-13
> [budget-preflight] weights=(선언된 바닥 사용) floor=16543MiB arm_ceiling=13471MiB declare=OK bench=BLOCK
>   - 벤치 진입 불가: 바닥 16543MiB − 벤치 8192MiB = 8351MiB < 절대밴드 10240MiB. 벤치 예산 상한은 이 구성에서 6303MiB 다 — 그보다 크면 벤치 컨테이너가 뜨는 순간 워치독 사정거리에 들어간다(camp-26090918 `fp8-bf-262k-mmp` 실측: 서빙은 성공하고 벤치가 죽었다).
> [run_bench] STOP(벤치 예산 게이트): 이 몫으로 벤치를 띄우면 서빙이 사정거리에 든다.
> [run_bench]   벤치를 **시작하지 않았다**. --bench-budget-mib 를 위 상한 이하로 낮추거나
> [run_bench]   KV 절대클램프를 낮춰 바닥을 올려라(그래야 둘 다 산다).

게이트문구의다른캠페인사살을이셀의실패로옮기지않는다. 당시예산차단뒤서빙을재기동한이유는계보미기록이며추정으로채우지않는다.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: 과거8192MiB벤치몫선택
기각사유: W3예산사전검사정상차단; 선언바닥−벤치가절대밴드미만
재개조건: 서빙예산또는호스트총량을다시도출해공식게이트통과
출처: [testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §4]
```

```hint-event
id: R2
kind: rejected
시도: 32GiB를131072한시퀀스의정확KV필요량으로선언
기각사유: 실제엔진수용131056으로16토큰미달거부; 계산필요량만으로기동성공을보장못함
출처: [granite42-30b-v030.yaml]
```

레벨16은max-num-seqs8을넘겨실행전필터된것이며실패한측정으로세지않는다. 이번은이전에통과한서빙형상을새Full로검증했고새값탐색이나거부실험은수행하지않았다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

```hint-event
id: M1
kind: misdiagnosis
증상: 과거판정문서가실제측정을서빙한2차기동의READY도510초로기록
서명: "2026-10-10 정정: 초판은 2차를 510 s 로 잘못 적었다"
원인: 1차기동소요를다음기동에복사한기재오류로보임(추론); 정정문은오기경위를적지않음
해소: 2차실제로그는475초; 이번은새serve로그의510초로따로기록
검증: 과거devlog의정정과현재serve로그DONE줄
전이등급: judgment
현재지위: 반증
출처: [devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §2]
```

10월1일의멀티이미지재사용D4는계획이며후속재빌드후의actualsingle이미지가정본이다. 또CIFS에서auto-prefetch비활성·수분로드는관측이나로컬대조없이로드시간의단일원인으로확정하지않는다. 엔진의일부Triton import경고만으로전체서빙실패를단정하지않고health·응답·Full관측과대조한다. 같은표면문구를원장미관측이나미실행의증거로뒤집지않는다.
> [원문] devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030 §무엇을 했나
> 1. 서빙: READY 475 s(2차 기동 · 측정을 서빙한 기동 — 원시 a_serve2.log `health 대기 : DONE (475s)`) · 1차 기동은 510 s(예산 게이트 차단으로 측정 없이 내림 · a_serve.log). 2026-10-10 정정: 초판은 2차를 510 s 로 잘못 적었다(hint 저작 Agent 지적).

추가 계보의 교정 묶음도 셀 소속을 가른다. plan_26100113의 F3(backend 인자 오류의 예산 소비), F4(weak 권한 원천), F6(grounding 전 세션 생성), F8(실패 엔진 전량 로그)는 이 메인 경로와 연결되지만 이번 새 실패가 아니다. F7은 서브 완수/거부 집계 문제이며 Granite 엔진 실패가 아니다. testlog_26100117은 E1·E2·E4의 차단/보존과 실제 C2 재빌드를 검증했다. 6셀 testlog의 E1은 이 모델 W3, E2~E5는 다른 모델/서브 사건이다. 해당 문서 §6의 FACT 생산자 11건 교정도 런타임 패치로 세지 않는다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

V1 dtypebf16·V2 gmu0.80·V3 길이131072·V6 maxseq8은승계했고이번에조정하지않았다. V5 KVauto는엔진기본선택이다. V4 35433480192는과거16→32→33GiB기동조정이라tuned로분류하되이번새최적화가아님을명시한다. 최초maxseq8산정근거와과거overhead16384→18432변경이유는계보에서미해소다.

이번총124608−weights55841−KV33792−overhead18432=floor16543MiB, arm13471MiB는보호선언산식이다. 엔진실측checkpoint54.53GiB·loading54.58GiB·KV33GiB와서로같은측정처럼섞지않는다. bench4096은상한6303아래이며, gmu0.80과절대KV클램프를동일역할로복사하지않는다(testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §1·§4).
> [원문] granite42-30b-v030.yaml §L5-6
> #   kv_cache: 33.0 GiB 절대 클램프 — 131072 1시퀀스 + 블록 올림 여유(try1 16GiB 거부 · try2 32GiB 는 블록 정렬로 131056 토큰 = 16 토큰 미달 거부 · 2026-10-01 실측)
> #   gmu 0.80 천장(통합메모리 · 호스트 안전 여유)

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 네이티브길이의산식만으로KV를딱맞춰복사하지말것 — W1·W2 — 엔진수용토큰과host예산을함께다시확인한다.
- 통합메모리의bench몫을서빙바닥과따로고르지말것 — W3 — 공식예산사전검사상한을따른다.
- 기동소요를다른회차에서복사하지말것 — M1 — 이번serve로그와시각을읽는다.
- 0.31 resolved를0.30이미지에붙이지말것 — 00 §0.4 — 빌드ref·원장·이미지정체를분리한다.
- CIFS로드원인과Triton경고를확정실패로일반화하지말것 — 2.4 — 대조없는기전은추론으로남긴다.
- 설정131072·maxseq8을긴컨텍스트8동시요청실증으로읽지말것 — Q3 — 이번부하는1024/256이다.
- metadata NA를로그관측부재로바꾸지말것 — testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §4 — 원문과스키마전사범위를구분한다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 32GiB수용토큰이정확산식보다16작은기전과최소안전여유는무엇인가
현재상태: 131056거부와33GiB통과관측; 블록정렬설명은가설
다음관측: 같은엔진의KV할당원시로그와블록산식을대조하는별도연구
출처: [granite42-30b-v030.yaml]
```

```hint-event
id: Q2
kind: open-question
물음: dense30B의실제대역폭상한과외부성능기준은무엇인가
현재상태: E검색empty·weak expected판정;lookup대역폭과과거4.5/5tps는추정이며실측아님
다음관측: 조건을명시한외부동일모델실측또는노드실측대역폭
출처: [testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §3, bench_report_26101016_granite-4.2-30b_GB10_0.30.0 §판정]
```

```hint-event
id: Q3
kind: open-question
물음: 긴컨텍스트동시요청과overhead최적값은어떤가
현재상태: KV135168토큰·131072기준1.03x와설정maxseq8관측; 신규Full은1024/256만검증; overhead18432는선언
다음관측: 별도승인범위의긴입력대기열·선점·MemAvailable피크대조
출처: [testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §1·§2·§4]
```
