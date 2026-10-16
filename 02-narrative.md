# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 10건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26101620_49_30_lfm25_26b_v031_CNN_hint셀.md` | testlog | 26101620 | 0 | 1296 | 0.31.0 · 26b · easy-vllm:0.31.0-cu133-aarch64-source · lfm25 · lfm25-26b-v031 · v031 | — |
| 2 | `docs/testlog/testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검.md` | testlog | 26101623 | 0 | 3985 | 0.31.0 · 26b · lfm25 · lfm25-26b-v031 · v031 | — |
| 3 | `docs/devlog/devlog_26101620_49_30_lfm25_26b_v031_CNN_hint셀.md` | devlog | 26101620 | 0 | 961 | 26b · lfm25 · lfm25-26b-v031 · v031 | — |
| 4 | `docs/benchmark/bench_report_26101620_lfm2.5-2.6b_GB10_0.31.0.md` | bench_report | 26101620 | 0 | 6504 | 0.31.0 · easy-vllm:0.31.0-cu133-aarch64-source · v031 | — |
| 5 | `docs/plan/plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행.md` | plan | 26101616 | 0 | 2294 | 0.31.0 | — |
| 6 | `docs/plan/plan_26101622_후속4건_안정화_데이터셋knob_lite_hint_기준선.md` | plan | 26101622 | 0 | 9601 | 0.31.0 | — |
| 7 | `docs/benchmark/bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0.md` | bench_report | 26101623 | 0 | 3202 | 0.31.0 | — |
| 8 | `docs/devlog/devlog_26101623_q6_서브_lite셀_저작.md` | devlog | 26101623 | 0 | 838 | — | — |
| 9 | `docs/testlog/testlog_26101620_p10_cnn_fullbench.md` | testlog | 26101620 | 1 | 2935 | 0.31.0 · 26b · lfm25 · lfm25-26b-v031 · v031 | — |
| 10 | `docs/testlog/testlog_26101622_Q2_서브_hint_결손5건_정체.md` (모호 해소) | testlog | 26101622 | 1 | 5463 | 0.31.0 · 26b · easy-vllm:0.31.0-cu133-aarch64-source · lfm25 · lfm25-26b-v031 · v031 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

(1) 목표 — LFM2.5-2.6B 를 NVIDIA GB10 1장에서 vLLM v0.31.0 으로 서빙한 셀을, 서브가 스스로 발행하는 hint 태그의 lite-only 셀로 내는 것이다. 이 계보의 출발점은 데이터셋 knob 개정분을 서브에 배달해 서브 2셀을 자체 발행하는 계획이었고(plan_26101616 P12), 이어 plan_26101622 가 기준선 ②(서브 lite 셀도 같은 모양으로 발행)의 검증 셀로 서브 lite-only 셀을 두었다(같은 plan 의 실행표 Q6 ⓐ). 서브 hint 결손 5건의 조사·처방은 같은 plan 의 F1 절이며 lite 셀을 지정하는 절은 아니다.
(2) HW 출발 조건 — GB10 1장, 단일노드, TP=1(00 §0.2). 노드 메모리 총량은 01 §1.4 이벤트 표의 예산 선언 행에 mem_total_mib=124608 로 기록돼 있다(원장 선언값 칸). 인터커넥트는 single 이라 해당 없음. 드라이버는 manifest 선언값이다(00 §0.4). 호스트 OS 는 미기록이다.
(3) 모델 구조 — 체크포인트는 LFM2.5-2.6B, bf16 비양자화다. 서빙 세션 엔진 로그가 Checkpoint size: 5.02 GiB 와 Resolved architecture: Lfm2ForCausalLM 를 기록한다. 크기를 어떻게 쟀는지와 파라미터 수·층 구성 수치는 미기록이다.
(4) 예상·관측된 장벽 — 이 계보에서 관측된 것은 W1~W5 다. 앞의 둘(W1·W2)은 CNN 측정 단계 기록이고 이 lite 셀의 측정은 거치지 않았다.
(5) 외부 선례·사람의 결정 — 이 계보에서 사람의 결정은 plan 승인 문장 둘뿐이다(승인 — 그대로 진행). 외부 선례는 계보 문서에 없다.

> [원문] plan_26101616_데이터셋knob_서브배달_서브E2E_hint발행 §1. 범위(모 plan 과 같다 · 새 결정 없음)
> | P12 | hint 발행 — 메인 3셀(granite CNN · minicpm CNN · minicpm lite) · 서브 2셀(서브 자체 발행) · 셀별 push |

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-16T14:03:30Z(`camp-26101623-baseline` · evidence.event_campaign_boundary): campaigns/camp-26101623-baseline/campaign.yaml declared_utc 2026-10-16T14:03:30Z → 경계 = 앞선 값 2026-10-16T14:03:30Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-16T13:51:58Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`) — sub(마지막 행 2026-10-16T13:51:58Z < 측정 끝 2026-10-16T14:09:03Z): 마지막 행 뒤 행 없음(이 노드의 로컬 원장 · 조회 시각까지 관측 · 미회수 아님 — 행은 사건 · 6시간 갱신 때만 쓰인다)

_이번 캠페인(경계 2026-10-16T14:03:30Z 이후)의 기동 시도 미관측 — 아래는 같은 셀 이름의 앞선 캠페인 행뿐이다._

**같은 셀 이름 · 앞선 캠페인의 기동 1회**(캠페인 경계 2026-10-16T14:03:30Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `serve-lfm25-26b-v031` | 2026-10-14T02:51:50Z | 119회 · 첫 2026-10-14T02:51:51Z | 없음 | —(닫힘 행 없음) | 3 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

> 이 계보의 서빙 단계(기동·클램프 수렴)의 벽은 계보 문서에 포함되지 않아 수록하지 않는다(§0.8). 아래는 측정·발행 도구 쪽에서 관측된 벽이다. 이번 캠페인 기동 시도는 원장에 없다(상주 서버를 그대로 쟀다).

### W1 프리셋을 .claude 아래에 복사하는 안내가 쓰기 거부로 막힘 (CNN 측정 단계)
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

### W2 sweep_bench.sh 가 --topology 없이 중단 (CNN 측정 단계)
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
출처: [testlog_26101620_p10_cnn_fullbench §README 따라 하기 소감]
```

### W3 서브가 지은 이미지에 빌드 원장이 없음
서브 hint 의 결손 조사(Q2)가 두 상주 컨테이너 모두 `/opt/easy-vllm/` 디렉터리가 없다고 확인했고, 빌드에 쓴 `Dockerfile.source-build` 에 원장 스탠자가 0건이라고 적었다. 이어 이번 점검은 그 Dockerfile 이 렌더 산출이 아니라 옛 템플릿 기반의 부분 수정본임을 diff 로 확인했다(2026-10-14 갱신 · 커밋 067600f). 해소는 이 셀에서 하지 않았다 — 재빌드가 금지된 범위라 원장 없는 이미지 그대로 발행하며, 그래서 패치 적용 판정은 재구성이다. 이 이미지가 지어진 뒤에 원장 스탠자가 있는 렌더본이 별도로 있다는 것만 확인했다(렌더본으로 이미지를 다시 지어 보지는 않았다).

> [원문] testlog_26101622_Q2_서브_hint_결손5건_정체 §서빙 형태 (질문 3)
> - `output/single/Dockerfile.source-build`·`Dockerfile` 에 원장 스탠자(`[빌드 원장]`)가 0건 — render_dockerfile.py:840 이 주입하는 스탠자가 이 배달본에 없다(이미지가 이 파일로 지어졌으므로 원장이 없는 것은 필연).

```hint-event
id: W3
kind: wall
증상: 발행 결손에 HINT_MISSING_BUILD_LEDGER 가 남는다
서명: "원장 스탠자(`[빌드 원장]`)가 0건"
원인: 서브가 지은 Dockerfile.source-build 에 원장 스탠자가 없다 — 이미지가 이 파일로 지어졌으므로 원장이 없는 것은 필연(Q2 서술). 이 파일이 옛 템플릿 기반 부분 수정본이라는 점은 이번 렌더 점검의 diff 관측이다
해소: 이 셀에서는 해소하지 않았다(재빌드 ✗) — 결손으로 기재한 채 발행
검증: testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세 — 현 파일에 스탠자 없음, 렌더본에는 있음
전이등급: arch-invariant
출처: [testlog_26101622_Q2_서브_hint_결손5건_정체 §서빙 형태 (질문 3), testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세]
```

### W4 정본 렌더 호출이 서브에서 성립하지 않음
지시된 정본 렌더(`--canonical-kind source-build`)는 `--resolved` 를 금지하고 `--topology` 를 요구했으며, 인자를 맞춰도 `assets/current-production-resolution.json` 이 서브에 배달되지 않아 읽기에 실패했다. 같은 템플릿을 `--template` 과 `--resolved` 로 직접 렌더하는 길로 넘었고 렌더는 성공했다(스탠자 있음).

> [원문] testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세
> - 지시된 `--canonical-kind source-build` 호출은 **서브에서 성립하지 않았다**: (a) `--resolved` 를 금지하고 `--topology` 를 요구하며, (b) `assets/current-production-resolution.json` 이 서브에 배달되지 않아 `shared production resolution unreadable (ENOENT)`.

```hint-event
id: W4
kind: wall
증상: 정본 렌더 호출이 서브에서 실패했다
서명: "shared production resolution unreadable (ENOENT)"
원인: current-production-resolution.json 이 서브에 배달되지 않았다 — 배달 목록 쪽 사정이며 서브 설정 오류는 아니다(추론 아님: 파일 부재를 ENOENT 로 관측)
해소: 같은 템플릿을 --template 과 --resolved 로 직접 렌더(임시 경로)
검증: testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세 — 렌더 성공, 스탠자 존재
전이등급: judgment
출처: [testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세]
```

### W5 torch_cuda_arch 가 null 인데 렌더가 ARG TORCH_CUDA_ARCH=None 을 씀
서브 manifest 에 compute_capability 가 없어 resolve_build_track 이 torch_cuda_arch 를 null 로 냈고, 렌더는 이를 실패시키지 않고 문자열 None 으로 ARG 를 썼다. 사람이 값을 추측해 채우지 않고 resolved.json 에 null 그대로 두었다. 이 렌더본으로 이미지를 짓지 않았으므로 빌드가 실제로 어떻게 되는지는 관측하지 않았다.

> [원문] testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세
> - **`torch_cuda_arch=null` 인데 렌더가 fail-loud 하지 않고 `ARG TORCH_CUDA_ARCH=None` 을 썼다**(`str(None)`). 헬프/문서는 빈값이면 fail-loud 라 하나 실제는 문자열 "None". 개선 후보.

```hint-event
id: W5
kind: wall
증상: 렌더본 Dockerfile 에 ARG TORCH_CUDA_ARCH=None 이 쓰였다
서명: "ARG TORCH_CUDA_ARCH=None"
원인: manifest 에 compute_capability 가 없어 torch_cuda_arch=null, 렌더가 str(None) 으로 옮겼다(코드 경로 str(None) 은 렌더 점검의 추정 설명이다)
해소: 해소하지 않았다 — resolved.json 에 null 을 그대로 기재했다
검증: testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세
전이등급: arch-locked
출처: [testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세]
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

```hint-event
id: R2
kind: rejected
시도: 정본 렌더 호출(--canonical-kind source-build)로 렌더본을 만드는 것
기각사유: 서브에서 호출 인자 규칙과 미배달 파일 때문에 성립하지 않았다(W4)
재개조건: 서브에 current-production-resolution.json 이 배달될 때
출처: [testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세]
```

다른 기각 시도는 이 계보 문서에 없다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

```hint-event
id: M1
kind: misdiagnosis
증상: 서브 hint 의 HINT_MISSING_RESOLVE 결손이 서브 평면에서 해소 불가인 항목으로 읽혔다
서명: "RESOLVE 가 가장 근접(추측)"
원인: 결손 조사(Q2)가 resolved.json 을 메인 소유 산출물로 보고 서브에서 성립하지 않을 것이라 추측했다 — 조사 문서가 스스로 추측이라 표시했다
해소: 서브가 upstream-version-watch 의 resolve 스크립트 네 개를 직접 돌려 output/single/resolved.json 을 만들었고 발행기의 RESOLVE 결손 코드가 사라졌다(사후 실행이라 빌드 때의 해소 기록은 아니다)
검증: testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표 — resolve 대조 일치, publish 결손 목록에 RESOLVE 없음
전이등급: judgment
현재지위: 반증
출처: [testlog_26101622_Q2_서브_hint_결손5건_정체 §요약, testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표]
```

> [원문] testlog_26101622_Q2_서브_hint_결손5건_정체 §요약
> - ⓒ(서브 평면에서 성립 불가): RESOLVE 가 가장 근접(추측).

원래 주장의 지위는 반증이다 — 서브 평면에서 resolved.json 은 만들어진다. 다만 그것이 이미지 빌드 때의 해소와 같은 기록이라는 뜻은 아니며(사후 실행), `torch_cuda_arch` 는 null 로 남았다(W5).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

이 셀의 서빙 노브 4개는 전부 승계값이며(V1~V4) 이 계보에는 값을 바꿔 잰 기록이 없다. 호스트 메모리 예산 산식·KV 필요량 파생·스윕 계획/시도 기록이 계보 문서에 없으므로 필요량 대비 배수는 적지 않는다(새 계산 금지). yaml 주석이 KV 클램프를 절대값(바이트)으로 적는 것만 확인된다. 엔진 로그는 "reserved 37.9 GiB memory for KV Cache as specified by kv_cache_memory_bytes config and skipped memory profiling" 이라 적는다 — lite 표의 KV cache 점유 37.9 GiB 는 측정이 아니라 yaml 클램프를 엔진이 되읽은 값이다. 실측은 "Model loading took 5.05 GiB" 와 클램프에서 파생된 GPU KV cache size 2,476,552 tokens 쪽이다. "이 값이 버티는 기전 대 실패 기전" 은 비교 시도가 없어 적지 않는다(지위: 미결).

> [원문] bench_report_26101623_lfm2.5-2.6b_GB10_0.31.0 §lite 지표 (서빙 성공 직후 스냅샷 · inform-only)
> | KV cache 점유 | 37.9 GiB |

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 프리셋을 .claude 아래로 복사하는 안내를 서브에서 따르지 말 것 — W1 — `--preset` 에 경로를 준다.
- `sweep_bench.sh` 를 `--topology` 없이 돌리지 말 것 — W2 — `--topology single` 을 붙인다.
- 원장 스탠자가 없는 Dockerfile 로 지은 이미지에서 발행 결손 BUILD_LEDGER 가 사라질 것이라 기대하지 말 것 — W3.
- 정본 렌더 호출을 서브에서 그대로 따라 하지 말 것 — W4 — 미배달 파일 때문에 실패한다.
- compute_capability 가 없는 manifest 로 렌더한 Dockerfile 을 그대로 빌드에 쓰지 말 것 — W5 — ARG 가 문자열 None 이 된다.
- resolve 사후 실행 값을 빌드 때의 해소 기록으로 읽지 말 것 — M1 — resolved.json 의 provenance 가 사후 실행임을 적는다.
- 이 lite 수치(warm 36.25 t/s)를 성능 baseline 으로 쓰지 말 것 — 00 머리 lite 측정 블록 — 판정이 아닌 관측이다.
- 서빙 노브를 필요조건으로 복사하지 말 것 — V1~V4 — 전부 승계값이다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: manifest 에 compute_capability 가 있으면 torch_cuda_arch 가 12.1a 로 해소되고 렌더본이 이미지와 같은 build-arg 를 내는가
현재상태: 이번에는 manifest 에 값이 없어 null 로 해소됐다(W5). 이미지 build-arg 관측값 12.1a 와 일치하는지는 관측하지 않았다
다음관측: manifest 에 compute_capability 를 채운 뒤 resolve_build_track 을 다시 돌린다
출처: [testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §결과 표]
```

```hint-event
id: Q2
kind: open-question
물음: 원장 스탠자가 있는 렌더본으로 이미지를 다시 지으면 BUILD_LEDGER 결손이 사라지고 서빙 거동이 같은가
현재상태: 렌더본은 만들었으나(스탠자 있음) 이 이미지를 다시 짓지 않았다. 재빌드 뒤 거동은 관측하지 않았다
다음관측: 메인이 재빌드를 허용하면 렌더본으로 빌드해 원장 존재와 lite 재측정을 확인한다
출처: [testlog_26101623_q6_서브_lite셀_materialize_resolve_렌더점검 §렌더 점검 상세]
```

```hint-event
id: Q3
kind: open-question
물음: 이 셀의 서빙 단계(기동·클램프 수렴·벽)에 벽이 있었는가
현재상태: 이 태그 계보에는 서빙 단계를 서술한 문서가 없다. 앞선 캠페인의 기동 1회가 원장에 있으나(01 §1.4) 그 기록을 서술한 문서는 계보에 없다
다음관측: 앞선 캠페인 서빙 문서를 계보에 보충해 재발행한다
출처: [testlog_26101622_Q2_서브_hint_결손5건_정체 §서빙 형태 (질문 3)]
```
