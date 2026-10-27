# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 11건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26102504_S5_full벤치_5셀_GuideLLM_판정_및_하네스_결함.md` | testlog | 26102504 | 0 | 9039 | 0.31.0 · ctx · easy-vllm:0.31.0-cu133-aarch64-source · gemma26b · gemma26b-ctx-single · single | — |
| 2 | `docs/testlog/testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정.md` | testlog | 26102711 | 0 | 11564 | 0.31.0 · ctx · easy-vllm:0.31.0-cu133-aarch64-source · gemma26b · gemma26b-ctx-single · single | — |
| 3 | `docs/plan/plan_26102700_코덱스_싱글8종_멀티4종_실측.md` | plan | 26102700 | 0 | 6647 | 0.31.0 · ctx · gemma26b · gemma26b-ctx-single · single | — |
| 4 | `docs/devlog/devlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_여정.md` | devlog | 26102711 | 0 | 3843 | 0.31.0 · ctx · gemma26b · gemma26b-ctx-single · single | — |
| 5 | `docs/devlog/devlog_26102504_PLE_mmap_재이식_full벤치_hint발행_서사.md` | devlog | 26102504 | 0 | 2920 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · single | — |
| 6 | `docs/benchmark/bench_report_26102509_gemma-4-26b-a4b-it_GB10_0.31.0.md` | bench_report | 26102509 | 0 | 6428 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · single | — |
| 7 | `docs/benchmark/bench_report_26102711_gemma-4-26b-a4b-it_GB10_0.31.0.md` | bench_report | 26102711 | 0 | 6424 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · single | — |
| 8 | `docs/plan/plan_26102420_PLE_mmap_재이식_full벤치_hint_Solar_슬롯화.md` | plan | 26102420 | 0 | 7459 | 0.31.0 | — |
| 9 | `docs/testlog/testlog_26102418_4모델_싱글멀티_연속캠페인_결과요약_종결.md` | testlog | 26102418 | 1 | 3124 | 0.31.0 · ctx · gemma26b | — |
| 10 | `docs/plan/plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증.md` | plan | 26102401 | 1 | 7359 | ctx · single | — |
| 11 | `docs/testlog/testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정.md` (모호 해소) | testlog | 26102412 | 2 | 1939 | 0.31.0 · ctx · easy-vllm:0.31.0-cu133-aarch64-source · gemma26b · gemma26b-ctx-single · single | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

첫plan_26102401은NAS기존4모델의single/multi최대context·양자화기동검증을목표로했다. Gemma는native262144 BF16하나였고YaRN확장은업스트림확증이있을때만,Full은별도셀승인으로정했다. testlog_26102412는기동수렴·240932토큰needle·lite25.11을기록한다. 후속plan_26102420의S5에서10월25일GuideLLM Full23.29를측정했다. 이번plan_26102700은공통Claude/Codex하네스전파후메인·서브8고유모델각각신규FullPASS와hint발행을요구한후속이다. 과거Full/needle를새성공으로세지않는다.

> [원문] plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §사용자 결정
> | context | 지원 최대치 기동 + health + 근접 길이 추론 1회 → **그 위에 lite/full 벤치까지**(full 은 별도 사람 승인) |

하드웨어는통합메모리GB10 GPU1·노드1·TP1이며인터커넥트는사용하지않는다. MemTotal124608MiB는기동러너관측이고driver580.178.04는manifest선언으로구분한다. 모델은Gemma4ForConditionalGeneration·BF16비양자화weights·30층중25sliding/5full·slidingwindow1024·expert128/top8이다. revision/양자화구성은00 FACT정본이다. head_dim256/512구조와현재엔진의TRITON_ATTN선택을기록하되명시env강제로부르지않는다. PLE·MTP/spec는선언하지않았다. index48.07GiB와엔진loading48.54GiB는생산자/방법을구분한다. 총/활성파라미터의독립계수와모델명의26B-A4B는같은생산자관측으로취급하지않는다.

예상벽은통합메모리KV예산과native최대길이검증이었다. 기존계보는외부Eempty였지만이번bounded검색은BF16/unquantized 단일DGXSpark 약24t/s의1차기록을채택했다(testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §3). 이외부E의batchtokens4096·부하/소프트지문은우리와같다고확인하지않았으며재현주장이아니다. 이번은weak권한을유지한다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계 미관측** — 재생 모드(캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다) — 캠페인 선언을 관측하지 못한다 · 경계 미관측 — 라벨(셀 이름)로 묶은 행 전부를 이 셀의 행으로 센다(같은 셀 이름의 앞선 캠페인 행이 섞였을 수 있다)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-27T02:51:28Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T21:48:33Z | 1회 · 첫 2026-10-23T21:48:34Z | 없음 | `budget_clear` 2026-10-23T22:03:55Z | 5 |
| 2 | main | `camp-26102401-gemma26b`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-23T23:52:08Z | 8회 · 첫 2026-10-23T23:52:10Z | 없음 | `budget_clear` 2026-10-24T03:50:45Z | 5 |
| 3 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:02:06Z | 1회 · 첫 2026-10-25T00:02:08Z | 없음 | `budget_clear` 2026-10-25T00:09:42Z | 5 |
| 4 | main | `serve-gemma26b-ctx-single` | 2026-10-25T00:11:00Z | 17회 · 첫 2026-10-25T00:11:01Z | 없음 | `budget_clear` 2026-10-25T00:44:20Z | 5 |
| 5 | main | `serve-gemma26b-ctx-single` | 2026-10-27T01:35:55Z | 18회 · 첫 2026-10-27T01:35:56Z | 없음 | `budget_clear` 2026-10-27T02:51:28Z | 12 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

W1·W2는10월23~24일형상수렴의과거벽이다. 재생 FACT는 캠페인 경계를 읽지 않아 역사적 5개 묶음을 싣는다. 이번 캠페인 측정은 2026-10-27T01:35:55Z 시작 세션 1회에 귀속되고, budget_clear 02:51:28Z까지의 마지막 묶음이다. 이 묶음에는 사살·트립·거부가 없으며 과거 행을 새 실패나 기동으로 더하지 않는다.

### W1 KV 클램프 부재 뒤 호스트 보호 종료

과거 첫 트라이얼은 KV 절대값이 비어 gmu로 큰 KV를 예약했고, 로드 뒤 MemAvailable이 보호 문턱 아래로 내려갔다. 후속 클램프 형상은 같은 길이의 필요량에서 도출했다. 선언 생략과 실제 사살 규칙을 나누어 아래에 기록한다.

```hint-event
id: W1
kind: wall
증상: 과거KV절대클램프null트라이얼이KV52.89GiB적재뒤memwatch종료
서명: "TRIP MemAvailable=9962MiB < 10240MiB"
원인: 클램프부재에서gmu0.85로큰KV를예약한뒤MemAvailable절대문턱위반관측; 선언생략과종료규칙은구분
해소: 목표262144×batch1의KV3368402176절대클램프를넣어재기동
검증: trial02전용스모크통과와과거engineKV3.14GiB; 이번도같은클램프의신규health/FullPASS
전이등급: arch-scaled
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/trial01_memwatch.log, docs/simlog/2026102406_2_gemma4-26b-a4b-262k/run_summary.json, testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §4]
```

> [원문] trial01_memwatch.log §L76-78
> [mem-watchdog] HB MemAvailable=10792MiB min=10792MiB 2026-10-23T21:41:08Z
> [mem-watchdog] TRIP MemAvailable=9962MiB < 10240MiB → docker kill 4c681a4ea495 2026-10-23T21:41:21Z
> [mem-watchdog] 4c681a4ea495

동일컨테이너를겨눈상주unit원장은21:41:20Z·9988MiB, 위trialwatchdog는21:41:21Z·9962MiB다. 한측정으로섞지않는다. kv_clamp_absent는budget_skipped사유이고종료는절대MemAvailable규칙이다. 선언존재자체가종료를막았다고쓰지않는다. 회수불가능메모리의세부기전은별도대조없는추론이다.

### W2 내부 포트와 호스트 포트를 혼동한 health 불도달

클램프 수렴 뒤 정규 기동은 엔진 API가 떠도 컨테이너 내부 포트가 매핑 목적지와 달라 health 대기가 끝났다. yaml의 내부8000 교정으로 다음 기동이 통과한 기록이며 메모리 실패가 아니다.

```hint-event
id: W2
kind: wall
증상: 과거정규기동rc5 health대기timeout; 엔진은내부8912에서기동
서명: "Starting vLLM server on http://0.0.0.0:8912"
원인: 생성기가host8912를yaml내부포트에써compose host8912→container8000매핑과불일치
해소: 이셀yamlport를8000으로교정; host포트는셀env8912유지
검증: 다음과거기동health200470초; 현재도교정된yaml/공식health성공
전이등급: arch-invariant
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1, output/single/benchlog/serve_fail_gemma26b-ctx-single/engine_timeout_20261023T220342Z.log, gemma26b-ctx-single.yaml, testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §1]
```

생성기후속수정의완료여부는이포트수작업교정과다른문제다. 과거watchdog_highrate_hold는보류이며사살로세지않는다.

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: YaRNfactor를추측해native262144초과확장
기각사유: 모델카드/config의공식선언없어미시도; 실패실험이아님
재개조건: 업스트림이나모델카드가공식확장을선언
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증, testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1]
```

```hint-event
id: R2
kind: rejected
시도: Gemma양자화weights와비교
기각사유: 당시NAS에해당양자화본이없어셀미작성; BF16weights와KVfp8을구분
출처: [plan_26102401_4모델_싱글멀티_최대context_양자화_기동검증 §NAS 사실]
```

S5의기본vllm도구Full무효는다른멀티3셀의폐기경로였다. 이셀과이번은GuideLLM0.7.3각3회로관측하며다른셀실패를현재실패로옮기지않는다(testlog_26102504 §2). 이번maxseq미선언은엔진동시성상한무제한품질증명이아니다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

```hint-event
id: M1
kind: misdiagnosis
증상: 과거요약괄호가kv_clamp_absent를사살사유처럼읽게함
서명: "1차 시도: 예산 선언 없이 진입 → 워치독 사살(`kv_clamp_absent`)"
원인: 선언생략사유와사살을한문장에나란히적어귀속모호
해소: budget_skipped.reason은kv_clamp_absent, watchdog_trip.rule은absolute와MemAvailable9988로분리
검증: 과거원장L491/L493과trialwatchdog9962별도대조
전이등급: judgment
현재지위: 미결
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1, docs/logs/main/events/2026-10.jsonl]
```

> [원문] testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §1. 기동
> - 1차 시도: 예산 선언 없이 진입 → 워치독 사살(`kv_clamp_absent`). 2차: 측정 KV 클램프로 수렴(simlog `docs/simlog/2026102406_2_gemma4-26b-a4b-262k/`).

미결은원문이괄호를사살원인이라고단정했는지가모호하다는뜻이다. 이번은클램프와절대종료규칙을분리한다. 또S5후속정정은top_k_experts누락으로activeweights과소산정→R_fp과대→expected/floor과대였음을기록한다(testlog_26102504 §4). 이전hint의방향잘못된설명을반복하지않는다. 이번측정은교정된별칭경로와E24를사용하나기준을맞추려고낮춘것이아니다.

judge신선도결함은S5다른셀의이전verdict재사용문제였고이번은freshgenerated키와bench_mode결속을검사한다. 서브러너·YaRN·PLEmmap다른모델교정은이Gemma런타임패치로귀속하지않는다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

gmu0.85는yamlhand/과거safety_margin승계로이번대조없다. native262144와KVfp8도승계다. 절대KV3368402176은과거min(required,max_safe)에서262144×batch1필요량으로도출했고다른바이트값스윕은없다. 과거engine267398토큰/262144기준1.02x관측과이번로그를구분한다. 정확값최적성이나긴요청2개수용을주장하지않는다.

> [원문] run_summary.json §L56-56
>       "after_note": "kv=min(required=3368402176, max_safe=56790205072)=3368402176. (max_model_len=262144×batch=1) (잠정 클램프 — batch 산출 전이다. 통과 트라이얼에서 산출 batch 로 재산정한다)",

과거overhead24800→18432변경이유는미해소였고이번18432도선언값이다. 선언weights/KV/floor와실제loading/graph관측은testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §1·§4에서구분한다. gmu기동게이트주석은한관측과기전미확정을함께적으므로일반vLLM법칙으로승격하지않는다.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 클램프없이gmu만복사하지말것 — W1 — 대상hybrid KV식과host예산을다시도출한다.
- kv_clamp_absent를사살규칙으로읽지말것 — M1 — budget행과watchdog행을분리한다.
- host포트를yaml내부포트에넣지말것 — W2 — compose목적포트와대조한다.
- YaRNfactor를추측하지말것 — R1 — 공식선언없으면native최대로기록한다.
- fp8KV성능/기능통과를정확도증명으로쓰지말것 — V4 — 별도품질대조가필요하다.
- NASdu를checkpoint크기로옮기지말것 — 1.6 — index/엔진관측과측정법을기록한다.
- 과거needle·lite·전용parser성공을이번Full과합치지말것 — 3.5 — 실행과부하를따로기록한다.
- 스키마NA와실제로그관측부재를같다고쓰지말것 — testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §4.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: native최대길이근처와긴요청동시성·fp8정확도는어떤가
현재상태: 과거needle240932PASS; 이번신규Full은1024/256이며최대길이품질대조없음
다음관측: 별도예산선언과승인범위의nearmax/정확도대조
출처: [testlog_26102412_gemma26b_싱글_262k_기동_근접추론_lite_판정 §2, testlog_26102711_gemma26b_ctx_single_코덱스_메인_Full_판정 §2]
```

```hint-event
id: Q2
kind: open-question
물음: 과거00:02:06Z→00:09:42Z측정없이닫힌기동은무엇이었나
현재상태: 10월25일같은라벨원장닫힘만관측; 실패/수동중단원인미확인
다음관측: 해당시각엔진/운영로그대조; 이번시도로세지않음
출처: [docs/logs/main/events/2026-10.jsonl]
```
