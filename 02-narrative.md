# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 4건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26101620_49_30_lfm25_26b_v031_CNN_hint셀.md` | testlog | 26101620 | 0 | 1296 | 0.31.0 · 26b · easy-vllm:0.31.0-cu133-aarch64-source · lfm25 · lfm25-26b-v031 · v031 | — |
| 2 | `docs/devlog/devlog_26101620_49_30_lfm25_26b_v031_CNN_hint셀.md` | devlog | 26101620 | 0 | 961 | 26b · lfm25 · lfm25-26b-v031 · v031 | — |
| 3 | `docs/benchmark/bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0.md` | bench_report | 26101620 | 0 | 6504 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · v031 | — |
| 4 | `docs/testlog/testlog_26101620_p10_cnn_fullbench.md` | testlog | 26101620 | 1 | 2935 | 0.31.0 · 26b · lfm25 · lfm25-26b-v031 · v031 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

(1) 목표 — LFM2.5-2.6B 를 NVIDIA GB10 1장에서 vLLM v0.31.0 으로 서빙한 셀을, CNN/DailyMail 3.0.0 test 프리셋(입력 1024 · 출력 256)으로 GuideLLM full-bench 하는 것이다. 이 계보는 그 측정 단계(P10)의 기록만 담는다.
(2) HW 출발 조건 — GB10 1장, 단일노드, TP=1(00 §0.2). 노드 메모리 총량은 01 §1.4 이벤트 표의 예산 선언 행에 mem_total_mib=124608 로 기록돼 있고(원장 선언값 칸) 서빙 yaml 주석은 호스트 121.7(GiB)을 적는다. 인터커넥트는 single 이라 해당 없음. 드라이버는 manifest 선언값이다(00 §0.4). 호스트 OS 는 미기록이다.
(3) 모델 구조 — 체크포인트는 LFM2.5-2.6B, bf16 비양자화다. 서빙 세션 엔진 로그가 Checkpoint size: 5.02 GiB 와 Resolved architecture: Lfm2ForCausalLM 를 기록한다. 크기를 어떻게 쟀는지(safetensors 헤더 합 대 du)와 파라미터 수·층 구성 수치는 미기록이다.
(4) 예상·관측된 장벽 — 이 계보에서 관측된 것은 W1, W2 뿐이다.
(5) 외부 선례·사람의 결정 — 이 계보에 없다. 메인이 attempt 1 의 권한 거부 뒤 "경로 방식" 을 지시했다는 것이 유일한 결정 기록이다.

> [원문] testlog_26101620_p10_cnn_fullbench §전제
> - 프리셋: `output/single/configs/cnn_preset.yaml`(경로 지정 · `.claude/**` 쓰기 거부로 attempt 1 중단 → 메인 지시로 경로 방식). `bench_preset.py validate` OK.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T11:50:00Z(`camp-26101616-cnn-hint` · evidence.event_campaign_boundary): campaigns/camp-26101616-cnn-hint/campaign.yaml declared_utc 2026-10-16T11:50:00Z → 경계 = 앞선 값 2026-10-16T11:50:00Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-16T11:21:58Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`) — ⚠ sub(마지막 행 2026-10-16T11:21:58Z < 측정 끝 2026-10-16T11:40:37Z): 그 뒤 그 노드의 선언 · 사건은 **관측 범위 밖**이다(없음이 아니다 · 원장 미회수)

_이번 캠페인(경계 2026-10-16T11:50:00Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T11:50:00Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-lfm25-26b-v031` | 2026-10-14T02:51:50Z | 114회 · 첫 2026-10-14T02:51:51Z | 없음 | —(닫힘 행 없음) | 3 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

이 셀의 서빙 단계(기동·클램프 수렴)의 벽은 이 태그의 계보 문서에 포함되지 않아 수록하지 않는다(§0.8). 아래는 CNN 측정 단계에서 관측된 벽이다. 이번 캠페인 기동 시도는 원장에 없다.

### W1 프리셋을 .claude 아래에 복사하는 안내가 쓰기 거부로 막힘
README 의 프리셋 작성 절차(cp)를 따르던 attempt 1 이 서브의 .claude/** 쓰기 거부에 막혀 중단했다. 메인이 경로 지정 방식을 지시했고, 프리셋을 `output/single/configs/` 에 두자 `bench_preset.py validate` 가 통과했다.

> [원문] testlog_26101620_p10_cnn_fullbench §전제
> - 프리셋: `output/single/configs/cnn_preset.yaml`(경로 지정 · `.claude/**` 쓰기 거부로 attempt 1 중단 → 메인 지시로 경로 방식). `bench_preset.py validate` OK.

```hint-event
id: W1
kind: wall
증상: CNN 프리셋 작성 단계에서 attempt 1 이 중단됐다
서명: ".claude/**` 쓰기 거부로 attempt 1 중단"
원인: README §4.2 의 cp 단계가 프리셋을 .claude 아래에 두게 하는데 서브는 .claude/** 쓰기가 막혀 있다(testlog 소감 1)
해소: 프리셋을 output/single/configs/cnn_preset.yaml 에 두고 --preset 에 경로로 지정(메인 지시)
검증: testlog_26101620_p10_cnn_fullbench §전제 — bench_preset.py validate OK, 이후 sweep 완주
전이등급: arch-invariant
출처: [testlog_26101620_p10_cnn_fullbench §전제]
```

### W2 sweep_bench.sh 가 --topology 없이 중단
sweep_bench.sh 는 `--topology single` 이 없으면 4자일치 판정자가 없다며 중단한다. README §4.3 명령 예시에 이 인자가 없어 그대로 따르면 막힌다(testlog 소감 2). 인자를 붙여 넘었다.

```hint-event
id: W2
kind: wall
증상: sweep_bench.sh 가 --topology 없이 시작되자 중단했다
서명: "4자일치 판정자(topology_parity.py)가 없다"
원인: 서브 배달분에 4자일치 판정자가 없어 topology 를 인자로 주지 않으면 판정할 수 없다(testlog 소감 2 — 서술 그대로)
해소: sweep_bench.sh 에 --topology single 을 붙인다
검증: testlog_26101620_p10_cnn_fullbench §전제 — 명령에 --topology single 포함, sweep 완주
전이등급: judgment
출처: [testlog_26101620_p10_cnn_fullbench §README]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: README §4.2 방식 — 프리셋을 .claude 아래로 cp 해서 이름(NAME)으로 지정
기각사유: 서브의 .claude/** 쓰기 거부로 attempt 1 이 중단됐다(testlog 소감 1)
재개조건: 서브에 .claude/** 쓰기 권한이 있을 때
출처: [testlog_26101620_p10_cnn_fullbench §전제]
```

다른 기각 시도는 이 계보 문서에 없다(기각된 시도 없음 — 근거: 계보 4건). 이 셀의 측정 형상을 좁힌 것은 프리셋 경로 지정과 --topology 인자뿐이다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

오진으로 정정된 주장은 이 계보 문서 4건에 없다(오진 없음 · 교정 묶음 없음). 다만 판정 자체의 한계로 판정이 roofline-only 로 강등됐다는 경고가 리포트에 있다 — 이것은 정정이 아니라 한계 기재다(Q1).

> [원문] bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0 §판정
> | E-search 상태 | no |
> | ⚠ warning | E-not-attempted: 판정이 roofline-only로 강등됨 — 메인 인스턴스는 외부검색(E) 수행 후 판정(egress-restricted 서브만 예외·--e-search empty 로 기록) |

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

값마다 처음 값과 바뀐 이유를 적을 기록이 이 계보에 없다 — 전 노브가 승계값이다(V1~V4). 호스트 메모리 예산 산식·KV 필요량 파생·스윕 계획/시도 기록이 계보 문서에 없으므로 필요량 대비 배수는 적지 않는다(새 계산 금지). yaml 주석이 KV 클램프를 절대값(바이트)으로 적는 것만 확인된다. 엔진 로그는 "reserved 37.9 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling" 이라 적는다 — 03 §3.1 의 lite.engine_kv_gib 37.9 는 측정이 아니라 yaml 클램프를 엔진이 되읽은 값이다. 실측은 "Model loading took 5.05 GiB" 와 클램프에서 파생된 GPU KV cache size 2,476,552 tokens 쪽이다. "이 값이 버티는 기전 대 실패 기전" 은 비교 시도가 없어 적지 않는다(지위: 미결).

> [원문] testlog_26101620_p10_cnn_fullbench §lfm25-26b-v031
> - 판정 PASS (authority=weak, expected_achievable 17.71 · floor 15.05) · c=1 decode 35.32 t/s · 반복 3/3 완주(밴드 0.085%) · 완료 16/실패 0.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 프리셋을 .claude 아래로 복사하는 안내를 서브에서 따르지 말 것 — W1 — `--preset` 에 경로를 준다.
- `sweep_bench.sh` 를 `--topology` 없이 돌리지 말 것 — W2 — `--topology single` 을 붙인다.
- 프리셋 방식을 기각한 R1 의 cp 경로를 그대로 쓰지 말 것 — R1 — 경로 지정.
- 합성(random) 입력 수치와 이 CNN 수치를 나란히 놓지 말 것 — testlog_26101620_p10_cnn_fullbench §전제 — 데이터셋 출처(03 §3.3)를 확인하고 비교한다.
- 이 판정(weak)을 외부검색(E)을 거친 판정으로 읽지 말 것 — Q1 — 판정 등급은 03 §3.5.
- 서빙 노브를 필요조건으로 복사하지 말 것 — V1~V4 — 전부 승계값이다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 이 판정은 외부검색(E)을 거치지 않은 roofline-only 다 — E 판정을 붙이면 PASS 가 유지되는가
현재상태: 리포트 경고 E-not-attempted. 측정 decode 35.32 t/s 는 primary 17.71 t/s 의 1.994 배다. 이 기대치가 낮게 잡힌 이유는 계보 문서에 설명이 없다
다음관측: 외부 레퍼런스를 찾아 judge_bench 를 다시 돌린다
출처: [bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0 §판정]
```

```hint-event
id: Q2
kind: open-question
물음: 이 셀의 서빙 단계(기동·클램프 수렴·벽)가 이 태그에 수록되지 않았다 — 서빙 단계에 벽이 있었는가
현재상태: 이 태그 계보는 CNN 측정 단계 문서 4건뿐이다. 이 셀의 앞선 캠페인 기동 1회가 원장에 있으나(01 §1.4) 그 기록을 서술한 문서는 계보에 없다
다음관측: 앞선 캠페인 서빙 testlog 를 계보에 보충해 재발행한다
출처: [testlog_26101620_p10_cnn_fullbench §전제]
```

```hint-event
id: Q3
kind: open-question
물음: 현행 full 정의(.claude/rules/docs.md)로 이 측정의 등급은 무엇인가
현재상태: 03 §3.5 사실 블록이 정의 문장을 읽지 못해 충족 여부를 판정하지 못했다(미관측). 이 측정은 반복 3, 도구 guidellm 이다
다음관측: 발행 환경에서 정의 문장을 읽어 대조한다
출처: [bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0 §측정 구성]
```
