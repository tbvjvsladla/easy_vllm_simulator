# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 5건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26101014_knob프리셋_E2E_셀C_ornith.md` | testlog | 26101014 | 0 | 2785 | 0.30.0 · 35b · easy-vllm:0.30.0-cu133-aarch64-source · ornith15 · ornith15-35b-v030 · v030 | — |
| 2 | `docs/devlog/devlog_26101107_ornith15-35b-v030_셀C_서빙_측정_knob해소.md` | devlog | 26101107 | 0 | 2395 | 0.30.0 · 35b · easy-vllm:0.30.0-cu133-aarch64-source · ornith15 · ornith15-35b-v030 · v030 | — |
| 3 | `docs/benchmark/bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0.md` | bench_report | 26101014 | 0 | 5905 | 0.30.0 · 35b · easy-vllm:0.30.0-cu133-aarch64-source | — |
| 4 | `docs/plan/plan_26101104_hint발행체제_개편_서브자체발행.md` | plan | 26101104 | 0 | 11774 | — | — |
| 5 | `docs/testlog/testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite.md` | testlog | 26100111 | 1 | 6482 | 0.30.0 · 35b · easy-vllm:0.30.0-cu133-aarch64-source · ornith15 · ornith15-35b-v030 · v030 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**목표.** Ornith-1.5-35B-A3B(체크포인트 구조는 서빙 yaml 머리 주석 기준 Qwen3_5MoeForConditionalGeneration · 하이브리드 GDN 3 : full attention 1 · bf16 · 네이티브 컨텍스트 262144)를 GB10 1장 · TP=1 단일 노드에서 vLLM 0.30.0 으로 서빙하는 것이다. 이 계보의 출발 문서는 10-01 서브 서빙 testlog(`testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite`)다. 이 모델 계보를 연 캠페인은 `camp-26100110-v030-single-dual`(declared_utc 2026-10-01T01:37:01Z)이고, 그 선언의 plan_ref 는 `docs/plan/plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인.md`, 매트릭스는 vLLM 0.30.0 × Ornith-1.5-35B-A3B(이 서브 배정 모드 AUTO), 예산 `smoke_budget_overhead_mib=16384`다(10-10 기동의 `--overhead-mib 16384` 출처도 이 선언이다). **그 plan 본문은 이 서브 트리에 배달되지 않아 읽을 수 없다** — plan 의 가정과 사람의 결정은 이 지도에서 읽을 수 없다(미기록이 아니라 서브에 미배달). 이 계보 표 5건 가운데 plan 은 이 지도를 발행하는 plan_26101104 하나뿐이고 나머지는 testlog · devlog · 벤치 리포트다.

**HW 출발 조건.** NVIDIA GB10 1장 · 단일 노드(sub). 노드 메모리 총량은 같은 testlog §6 에 124,608 MiB(`free` 기준 총량)로 기록돼 있고, 서빙 yaml 머리 주석은 통합메모리 예산을 121.69 GiB 로 적었다(두 값은 단위 · 산정 방식이 달라 나란히 두되 하나로 합치지 않는다). 드라이버는 manifest 선언 580.173.02 이고 노드별 관측값은 없다(00 §0.4). 호스트 보호 장치(`easy-vllm-memwatch`)가 켜져 있었고, 원장은 그 규칙을 둘로 기록한다 — 절대 임계(MemAvailable 10240 MiB)와 하락 속도 기반 예측 규칙(W2). 후자는 11 GiB 대에서도 컨테이너를 `docker kill` 했다. 인터커넥트 · 분산 방식은 해당 없음(single · TP=1).

**모델 구조.** 서빙 yaml 머리 주석(작성자의 선언)은 가중치를 bf16 약 67 GiB 로 적었고, 엔진 로그 실측은 적재 64.69 GiB(10-01 성공 시도의 엔진 로그 `Model loading took … 287.27 seconds`, testlog §4; 10-10 실행은 312.91 초로 달리 기록 — 01 §1.4 (a))다 — 선언과 실측이 다르며 이 지도는 후자를 쓴다. 체크포인트 HF repo 와 revision 은 00 §0.2 사실 블록이 정본이다. 총/활성 파라미터 수 · MTP 층 유무 · 체크포인트 크기를 safetensors 헤더 합으로 잰 기록은 계보에 **없다**(미기록). 루프라인 산정이 쓴 활성 바이트는 7.267 GiB(dense 5.39 GiB + (8/256) × routed 60.00 GiB, 03 §3.5)이다. 양자화 층 구성은 비양자화(체크포인트 config 에 quantization_config 부재 관측, 00 §0.2).

**stock 상태에서 예상 · 관측된 장벽.** (1) 첫 `single_serve_up.sh` 가 그라운딩 기록 부재로 거부됐다(W1). (2) 기본 MoE 백엔드로는 컴파일 직후 호스트 메모리 급락 → 워치독 사살이 3회 반복됐다(W2). (3) 같은 config 의 열린 세션이 누적돼 종료 단계가 실패했다(W3).

**외부 선례와 사람의 결정.** 이 계보에는 외부 선례 문서가 없다. 도서관 인용도 0건이다(`--ground` 가 사서 도구 부재로 `unresolved · 참조 0건`을 기록, testlog §2). 결정 근거는 실측과 vLLM 소스(`config/kernel.py` MoEBackend)라고 testlog 가 적었다.

계보에서 가장 이른 문서에 적힌 이 셀의 형상 · 결과 요약 원문:

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2. 트리플렛
> ## 2. 트리플렛 (`output/single/`)
> - `configs/ornith15-35b-v030.{yaml,sh}` · `envs/.env.ornith15-35b-v030` (포트 8903 · served-model-name `ornith-1.5-35b-a3b`).
> - 최종 yaml: gmu 0.80 · max-model-len 262144 · **kv-cache-memory-bytes 8 GiB(8589934592)** · max-num-seqs 4 · max-num-batched-tokens 4096 · `limit-mm-per-prompt {image:0, video:0}`(텍스트 전용) · **`moe-backend: triton`** · `reasoning-parser qwen3` · `tool-call-parser qwen3_xml` · auto-tool-choice.
> - 도서관: `--ground` 는 사서 도구 부재(`wiki-desk/scripts/init_wiki_desk.py` 없음)로 `unresolved · 참조 0건` 이 기록됐다(그라운딩 백스톱 통과 조건). 도서관 인용 없음 — 결정 근거는 아래 실측과 vLLM 소스(`config/kernel.py` MoEBackend)다.


## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-10T05:05:03Z(`camp-26101104-hint-cd` · evidence.event_campaign_boundary): campaigns/camp-26101104-hint-cd/campaign.yaml declared_utc 2026-10-10T19:53:02Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:05:03Z → 경계 = 앞선 값 2026-10-10T05:05:03Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): sub = 2026-06-13T22:25:01Z → 2026-10-10T15:17:46Z(`docs/logs/sub/events/2026-06.jsonl` · `docs/logs/sub/events/2026-07.jsonl` · `docs/logs/sub/events/2026-09.jsonl` · `docs/logs/sub/events/2026-10.jsonl` · `docs/logs/sub/events/thermal.jsonl` · `docs/logs/sub/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | sub | `serve-ornith15-35b-v030` | 2026-10-10T05:05:03Z | 1회 · 첫 2026-10-10T05:05:05Z | 없음 | `budget_clear` 2026-10-10T05:31:39Z | 12 |

**같은 셀 이름 · 앞선 캠페인의 기동 5회**(캠페인 경계 2026-10-10T05:05:03Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:43:16Z | 없음 | 없음 | —(닫힘 행 없음) | 2 |
| 앞선-2 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:47:22Z | 1회 · 첫 2026-10-01T01:47:23Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T01:58:12Z | 5 |
| 앞선-3 | sub | `ornith15-v030`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T01:58:34Z | 1회 · 첫 2026-10-01T01:58:36Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T02:05:23Z | 5 |
| 앞선-4 | sub | `ornith15-v030-r3`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:05:49Z | 1회 · 첫 2026-10-01T02:05:51Z | `watchdog_kill_ack` 1 · `watchdog_trip` 1 | `budget_clear` 2026-10-01T02:12:23Z | 5 |
| 앞선-5 | sub | `ornith15-v030-r4`(라벨 다름 — 같은 노드 · 이 셀 라벨 아님) | 2026-10-01T02:13:25Z | 1회 · 첫 2026-10-01T02:13:26Z | 없음 | `budget_clear` 2026-10-01T02:22:21Z | 5 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

이 셀의 기동 시도 표(위, 블랙박스 원장)는 **이번 캠페인 창의 1회**만 센다. 아래 벽 W1~W3 는 그보다 앞선 10-01 서브 서빙(`testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite`, 서빙 시도 4회 중 4번째 성공)에서 나온 것이고, 원장의 '라벨 다름' 5개 선언 창(위 시도 표 앞선-1~5)에 대응한다: 앞선-1 은 시도 0(갱신 없이 다음 선언에 의해 닫힘), 앞선-2 · 앞선-3 은 시도 1 · 2(KV 16 GiB 선언, 각각 트립 1회), 앞선-4(라벨 r3)는 시도 3(트립 1회), 앞선-5(라벨 r4)는 시도 4(트립 없음, 성공). 선언 시각 · KV 선언값 · 트립 유무가 testlog §3 표와 맞는다. 10-10 의 기동 1회(`testlog_26101014_knob프리셋_E2E_셀C_ornith` §2)는 벽 없이 READY 에 도달했다.

### W1 첫 기동이 그라운딩 기록 부재로 거부됨
서빙을 띄우는 `single_serve_up.sh` 가 결정의 근거 기록(그라운딩)이 없다는 이유로 fail-closed 로 종료(exit 4)했다. `--ground` 로 해소 기록을 남기자 통과했다. 이 노드에는 사서 도구가 없어 해소 결과는 `unresolved · 참조 0건`이었다(그라운딩 백스톱은 통과 조건만 충족). 모델 · 형상과 무관한 절차 벽이고 원인은 testlog 가 직접 적었다. 통과는 같은 스크립트 재실행이 서빙 단계로 넘어간 것으로 판정했다.

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3. 서빙 시도 이력
> | # | 구성 | 결과 |
> |---|---|---|
> | 0 | 첫 `single_serve_up.sh` | exit 4 — **그라운딩 기록 없음**(`LIBRARY_GROUNDING_FAIL_CLOSED`). `--ground` 로 해소 |
> | 1 | KV 16 GiB · seqs 8 · mm on · MoE auto(FlashInfer CUTLASS) | 로드 316 s → torch.compile 직후 사망, exit 137 |
> | 2 | 동일(컴파일 캐시 적중) | 동일 지점 사망 |
> | 3 | KV 8 GiB · seqs 4 · batched 4096 · mm off · MoE auto | 동일 지점 사망 |
> | 4 | 3 + **`moe-backend: triton`** | **READY**(health 200) |

```hint-event
id: W1
kind: wall
증상: 첫 single_serve_up.sh 호출이 서빙 전에 종료(exit 4)했다
서명: "exit 4 — **그라운딩 기록 없음**(`LIBRARY_GROUNDING_FAIL_CLOSED`)"
원인: 서빙 결정에 대한 그라운딩 기록이 없으면 서빙 스크립트가 fail-closed 로 막는다(testlog 가 적은 사유)
해소: --ground 로 해소 기록을 먼저 남긴다(사서 도구 부재로 unresolved · 참조 0건)
검증: testlog_26100111 §3 시도 1 이후 기동 진행
전이등급: arch-invariant
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

### W2 기본 MoE 백엔드(auto)에서 컴파일 직후 호스트 워치독 사살 ×3
시도 1 · 2 는 KV 클램프 16 GiB · 동시 시퀀스 8 · 멀티모달 입력 허용 · MoE 백엔드 auto 였고 로드 후 torch.compile 직후 사망(exit 137)했다. 시도 3 은 KV 8 GiB · 시퀀스 4 · 배치 토큰 4096 · 멀티모달 끔으로 줄였지만 같은 지점에서 죽었다. 사살 주체는 엔진이 아니라 호스트 `easy-vllm-memwatch` 이고, 원장에 기록된 3회의 트립은 규칙과 시점 mem_avail 이 다르다: 시도 1 은 절대 임계(rule=absolute)가 10022 MiB 에서, 시도 2 와 3 은 속도 기반 예측(rule=eta)이 11483 MiB(2149 MiB/s)와 11085 MiB(2342 MiB/s)에서 발화했다(둘 다 10240 MiB 위, 01 §1.4 이벤트 8 · 19 · 29행). testlog 는 어느 시도의 것인지 밝히지 않은 '시리즈' 하나로 로드 후 약 46 GiB 에서 컴파일 직후 30 초 만에 36 → 19 → 트립 12 GiB 로 떨어졌다고 적었다. KV · 시퀀스 · 멀티모달을 줄여도 같은 지점에서 죽었으므로 원인은 그쪽이 아니라 MoE 백엔드 auto 가 고른 FlashInfer CUTLASS 의 프로파일/워밍업 구간으로 좁혀졌다(testlog 표 기준 시도 3 이 KV · 시퀀스 · 배치 토큰 · 멀티모달을 동시에 바꿨고 시도 1 · 2 의 배치 토큰 값은 표에 없어 미기록이므로, 바뀐 노브 수도 확정되지 않으며 KV · 멀티모달 감축의 개별 기여는 분리되지 않았다). 시도 4 는 시도 3 에 `moe-backend: triton` 한 변인만 더해 READY 에 도달했다. **FlashInfer 가 왜 급락을 만드는지(워크스페이스인지 autotune 인지)의 기전은 미확정**이다 — 컨테이너가 죽기 전 로그에 에러가 남지 않았고 autotune 만 끄는 시험은 하지 않았다(Q1). 통과는 health 200 과 chat completion 1회(`finish_reason=stop`)로 판정했다.

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3. 서빙 시도 이력
> - 사망 원인: 호스트 `easy-vllm-memwatch`(MemAvailable < 10240 MiB → `docker kill`, `docker_kill` 이벤트 · exit 137). 시리즈: 로드 후 MemAvailable ≈ 46 GiB 유지 → **컴파일 직후 30 s 만에 36→19→(trip) 12 GiB**. KV 클램프·seqs·mm 를 줄여도(2→3번) 같은 지점에서 죽었으므로 **원인은 KV/mm 가 아니라 MoE 백엔드 `auto` → FlashInfer CUTLASS Unquantized MoE 의 프로파일/워밍업 구간**으로 좁혀졌다(3번에서 변인을 한꺼번에 바꿨기에 KV·mm 감축의 개별 기여는 분리하지 못했다 — 4번은 3번에 triton 한 변인만 추가).

```hint-event
id: W2
kind: wall
증상: 컴파일 직후 컨테이너가 exit 137 로 사망(호스트 워치독 docker kill)
서명: "컴파일 직후 30 s 만에 36→19→(trip) 12 GiB"
원인: 호스트 memwatch 가 사살(원장: 절대 임계 rule=absolute 1회 mem_avail 10022 MiB, 속도 예측 rule=eta 2회 mem_avail 11483 · 11085 MiB). 급락 구간은 MoE 백엔드 auto(FlashInfer CUTLASS Unquantized MoE)의 프로파일/워밍업으로 좁혀짐(시도 3 → 4 의 단일 변인 대조). 급락 기전 자체는 미확정
해소: serving yaml 에 moe-backend: triton 지정
검증: testlog_26100111 §3 시도 4 READY(health 200) · 10-10 testlog §2 READY
전이등급: arch-locked
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3, ornith15-35b-v030.yaml]
```

### W3 같은 config 의 열린 세션 누적으로 종료(down) 단계 실패
시도 0(그라운딩 STOP)이 세션을 연 채 종료했고, 시도 1 이 두 번째 세션을 열어 롤백 down 의 5/5 단계가 실패했다. `single_serve_down.sh` 는 같은 config 의 열린 세션이 둘이면 `--session-id` 없이는 종료하지 못한다. 그라운딩 검사를 세션 시작 앞으로 옮기면 누적이 없다는 것이 testlog 의 제안이며, 이 제안이 반영됐는지는 계보에서 확인되지 않는다. 10-10 의 기동은 세션이 하나여서 `single_serve_down.sh` 6/6 로 정리됐다(testlog_26101014_knob프리셋_E2E_셀C_ornith §5).

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §7. 제안 (메인 HITL 회수용)
> 4. `single_serve_down.sh` 는 같은 config 의 열린 세션이 2개면 `--session-id` 없이는 종료 실패한다 — 시도 0(그라운딩 STOP)이 세션을 연 채 종료했고(4/7 이후 STOP 이 롤백 없이 exit 4), 시도 1 이 두 번째 세션을 열어 롤백 down 의 5/5 단계가 FAIL 했다. 그라운딩 검사를 세션 시작 앞으로 옮기면 누적이 없다.

```hint-event
id: W3
kind: wall
증상: 롤백 down 의 5/5 단계가 FAIL, 세션이 남음
서명: "같은 config 의 열린 세션이 2개면 `--session-id` 없이는 종료 실패한다"
원인: 시도 0 이 세션을 연 채 종료하고 시도 1 이 두 번째 세션을 열어 같은 config 세션이 둘이 됨
해소: --session-id 로 세션을 지정해 종료(제안은 그라운딩 검사를 세션 시작 앞으로 이동 — 반영 여부 미확인)
검증: 10-10 testlog §5 단일 세션 6/6 정리(세션이 하나인 경우)
전이등급: arch-invariant
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §7, testlog_26101014_knob프리셋_E2E_셀C_ornith §5]
```


## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3. 서빙 시도 이력
> | 1 | KV 16 GiB · seqs 8 · mm on · MoE auto(FlashInfer CUTLASS) | 로드 316 s → torch.compile 직후 사망, exit 137 |
> | 2 | 동일(컴파일 캐시 적중) | 동일 지점 사망 |
> | 3 | KV 8 GiB · seqs 4 · batched 4096 · mm off · MoE auto | 동일 지점 사망 |

```hint-event
id: R1
kind: rejected
시도: KV 클램프 16 GiB · 동시 시퀀스 8 · 멀티모달 허용 · MoE 백엔드 auto 로 기동(시도 1, 같은 구성의 컴파일 캐시 적중 재시도가 시도 2)
기각사유: 두 번 모두 torch.compile 직후 exit 137(호스트 워치독 사살)
재개조건: MoE 백엔드를 triton 으로 고정한 뒤에는 이 구성(KV 16 GiB · 시퀀스 8)이 버티는지 시험된 적이 없다 — 필요하면 새로 잰다
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

```hint-event
id: R2
kind: rejected
시도: KV 클램프 8 GiB · 동시 시퀀스 4 · 배치 토큰 4096 · 멀티모달 끔으로 줄이되 MoE 백엔드는 auto 유지(시도 3)
기각사유: 줄여도 같은 지점에서 사살 — KV · 시퀀스 · 멀티모달 크기가 사살의 원인이 아님을 보임(testlog 표 기준 KV · 시퀀스 · 배치 토큰 · 멀티모달이 동시에 바뀌어 개별 기여는 분리 못 함)
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

기각된 시도는 두 건이며 둘 다 'MoE 백엔드는 그대로 두고 크기만 줄이는' 접근이다. 이 둘이 형상을 좁혀 남은 원인 후보를 MoE 백엔드로 모았고, 시도 4 가 triton 한 변인만 바꿔 통과했다. 계보에는 이 밖에 기각된 시도의 기록이 없다(검토한 문서 5건).


## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

**M1 — 멀티모달 프로파일 피크가 사살 원인이라는 판단.** 서빙 yaml 의 `limit-mm-per-prompt` 위 주석은 이미지 프로파일 피크를 제거하는 것을 사살 대책으로 적었다(시도 1 · 2 가 사살된 직후의 판단). 시도 3 이 멀티모달을 끈 채로도 같은 지점에서 죽어 이 판단은 원인으로는 기각됐다. 그러나 yaml 주석은 갱신되지 않아 **실린 yaml 에 옛 판단이 그대로 남아 있다** — 수신자가 주석을 사실로 읽지 않도록 이 절이 정정한다. 멀티모달 끔이 사살 회피에 일부라도 기여했는지는 분리되지 않았다(미결).

> [원문] ornith15-35b-v030.yaml §L12-12
> # 텍스트 전용 서빙 — 멀티모달 인코더 프로파일(16384 tok 이미지) 피크 제거(attempt1·2 가 memwatch 10 GiB 바닥에 걸려 사살됨)

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3. 서빙 시도 이력
> - 사망 원인: 호스트 `easy-vllm-memwatch`(MemAvailable < 10240 MiB → `docker kill`, `docker_kill` 이벤트 · exit 137). 시리즈: 로드 후 MemAvailable ≈ 46 GiB 유지 → **컴파일 직후 30 s 만에 36→19→(trip) 12 GiB**. KV 클램프·seqs·mm 를 줄여도(2→3번) 같은 지점에서 죽었으므로 **원인은 KV/mm 가 아니라 MoE 백엔드 `auto` → FlashInfer CUTLASS Unquantized MoE 의 프로파일/워밍업 구간**으로 좁혀졌다(3번에서 변인을 한꺼번에 바꿨기에 KV·mm 감축의 개별 기여는 분리하지 못했다 — 4번은 3번에 triton 한 변인만 추가).

```hint-event
id: M1
kind: misdiagnosis
증상: 시도 1 · 2 가 컴파일 직후 사살됨(exit 137)
서명: "멀티모달 인코더 프로파일(16384 tok 이미지) 피크 제거"
원인: 사살 직후 멀티모달 프로파일 피크를 원인으로 짚었다(yaml 주석에 남음). 같은 지점의 사망이 반복되는데도 변인(MoE 백엔드)을 아직 건드리기 전의 판단이었다는 해석은 추론이다
해소: 멀티모달을 끄고도 죽었으므로(시도 3) 원인 후보에서 내리고 MoE 백엔드를 바꿔 통과시켰다(시도 4)
검증: testlog_26100111 §3
전이등급: judgment
현재지위: 반증
출처: [ornith15-35b-v030.yaml, testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

교정 묶음(항목 id 가 붙은 교정 목록)은 계보 5건에 없다. 검토한 문서 5건 중 이 셀의 판정 · 수치 · 이름을 바꾼 교정 항목은 없다.


## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**호스트 메모리 예산(선언).** 10-10 기동은 `--overhead-mib 16384`(10-01 testlog 에 기록된 선언값)로 예산을 선언했고, 원장의 선언 행은 floor_mib=31460 · weights_mib=68572 · kv_mib=8192 · overhead_mib=16384 · mem_total_mib=124608 이다(01 §1.4 이벤트 42행 — **선언값이지 측정이 아니다**). 엔진 로그 실측은 가중치 적재 64.69 GiB · KV 8.0 GiB · 총 예약 72.69 GiB(03 §3.1 `lite.*`)로, 가중치는 선언(68572 MiB)보다 작다. 10-01 testlog §6 은 floor 23,268 MiB 를 성공 구간과 비교했는데, 원장(01 §1.4 이벤트 1 · 4 · 14 · 24 · 34행)을 보면 floor 가 두 값이다: 시도 0~2(라벨 ornith15-v030)는 kv_mib=16384 로 floor 23268(= 124608 − 68572 − 16384 − 16384), 시도 3(ornith15-v030-r3)·시도 4(ornith15-v030-r4)와 10-10 은 kv_mib=8192 로 floor 31460 이다. 즉 차이는 KV 선언 16 GiB 대 8 GiB 이고 overhead 는 모두 16384 다. testlog §6 이 성공 구간을 23,268 과 견준 것은 성공 시도(시도 4)의 선언(31460)이 아니라 KV 16 GiB 시도의 선언값이다.

**KV 클램프 8 GiB (V6).** 처음 16 GiB(시도 1 · 2) → 8 GiB(시도 3 · 4)로 줄였고, 기전상 사살 회피에는 효과가 없었다(R1 · R2). 지금 값은 이 셀에서 값을 바꿔 가며 잰 기록이 없어 승계값이다. 엔진 로그는 이 값에서 408,122 토큰을 확보하고 262,144 토큰 요청 기준 최대 동시성 1.56x 라고 보고한다(01 §1.4 이벤트 51행) — 1.56x 는 엔진 자신의 표시이고 이 지도가 재계산한 것이 아니다.

**gmu 0.80 (V1).** yaml 주석이 'startup free-memory 게이트 + 총 cap'이며 실제 KV 는 절대 클램프가 제어한다고 적었다. 처음 값은 그 주석의 산식(weights ~67 GiB + KV 8 GiB + 여유 → 상한 97.4 GiB)에서 손으로 정한 것이고 스윕한 기록이 없다.

**동시 시퀀스 4 · 배치 토큰 4096 (V3 · V4).** 시도 3 에서 8 → 4 로 줄이며 같은 때 정해졌고 이후 조정되지 않았다. 10-10 측정의 동시성 레벨은 이 값을 상한으로 걸러 1 · 2 · 4 만 잰다(8 · 16 은 `> max-num-seqs 4` 로 제외, testlog_26101014_knob프리셋_E2E_셀C_ornith §4).

**MoE 백엔드 triton (V11).** 시도 4 가 시도 3 에 이 한 변인만 더했다. 이 값이 버티는 기전과 auto 가 실패하는 기전은 W2 와 Q1 대로 확정되지 않았다.

**자체 보고된 산식 원문:**

> [원문] testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §2. 트리플렛
> - 최종 yaml: gmu 0.80 · max-model-len 262144 · **kv-cache-memory-bytes 8 GiB(8589934592)** · max-num-seqs 4 · max-num-batched-tokens 4096 · `limit-mm-per-prompt {image:0, video:0}`(텍스트 전용) · **`moe-backend: triton`** · `reasoning-parser qwen3` · `tool-call-parser qwen3_xml` · auto-tool-choice.

**버티는 기전 대 실패 기전.** (1) `moe-backend: triton` — 시도 4 통과, 서빙 중 MemAvailable 최저 약 37.2 GiB(성공 구간 하트비트, testlog §6) — 기전: 미확정, 지위: 현상 확정 · 기전 미결. (2) auto(FlashInfer CUTLASS) — 컴파일 직후 30 초에 약 46 → 12 GiB 급락 — 기전: 프로파일/워밍업 구간으로 좁혀짐, 워크스페이스 대 autotune 미확정 — 지위: 현상 확정 · 기전 미결. (3) KV 16 → 8 GiB · 시퀀스 8 → 4 · 멀티모달 끔 — auto 위에서는 효과 없음 — 지위: 확정(시도 3 대 시도 4 대조), 단 개별 기여 분리 불가. 근거: testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3 · §6.


## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 호스트 워치독 임계가 있는 호스트에서 `moe-backend` 를 auto 로 두고 기동하지 말 것 — W2: 컴파일 직후 급락으로 3회 사살 — 대신 `moe-backend: triton` 을 명시하고 MemAvailable 시계열을 관찰한다.
- 줄이면 해결될 거라 보고 KV · 시퀀스 · 멀티모달만 줄이지 말 것 — R1 · R2: 줄여도 같은 지점에서 사망 — 대신 한 번에 한 변인만 바꾼다(시도 3 이 여러 노브를 동시에 바꿔 개별 기여를 잃었다).
- yaml 주석의 원인 설명을 사실로 읽지 말 것 — M1: 실린 주석이 갱신되지 않은 옛 판단이다 — 대신 testlog §3 의 시도 이력을 읽는다.
- 엔진 자기보고 버전 문자열로 이미지가 0.30.0 인지 판정하지 말 것 — Q4(`testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite` §1): 문자열은 dev 표기이고 소스 트리는 v0.30.0 태그다 — 대신 빌드 입력(VLLM_REF)과 소스 git describe 로 확인한다.
- 실린 Dockerfile · requirements 로 이 이미지를 그대로 재현할 수 있다고 믿지 말 것 — 01 §1.1 경고: 실린 바이트가 이미지 history 와 11/20 만 일치 — 대신 docker history 의 build-arg 와 01 §1.4 의 명령을 기준으로 삼는다.
- 예산 선언 없이 기동하지 말 것 — 01 §1.4 이벤트 44행: 호스트 워치독이 예산 선언(budget_declare)을 읽고 arm 천장을 정한다 — 대신 `single_serve_up.sh` 로 띄우고 같은 config 의 세션을 하나만 유지한다(W3).
- lite 수치와 full 수치를 같은 표에 순위로 놓지 말 것 — 03 §3.5: 측정 도구 · 구성이 다르다 — 대신 같은 bench_mode 안에서만 비교한다.
- 판정 PASS 를 외부 레퍼런스로 검증된 성능으로 읽지 말 것 — Q7(벤치 리포트 경고 E-not-attempted): roofline-only 판정이다 — 대신 판정을 표시로만 읽고 네 환경에서 재측정한다.


## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: moe-backend auto(FlashInfer CUTLASS) 의 컴파일 직후 메모리 급락 기전은 워크스페이스인가 autotune 인가
현재상태: 컨테이너가 죽기 전 로그에 에러가 없고, enable_flashinfer_autotune=false 단독 시험은 하지 않았다. 트립은 3회(원장: 절대 임계 1회 mem_avail 10022 MiB, 예측 규칙 2회 11483 · 11085 MiB)이고, 로드 후 약 46 GiB 가 컴파일 직후 30 초 만에 36 → 19 → 12 GiB 로 떨어진 시계열은 testlog 가 어느 시도의 것인지 밝히지 않은 '시리즈' 하나다
다음관측: 호스트 보호 장치를 선언한 상태에서 FlashInfer autotune 만 끈 기동을 한 번 시험하고 MemAvailable 시계열을 비교한다
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §3]
```

```hint-event
id: Q2
kind: open-question
물음: 레벨 4 에서 completed 가 요청 수(16)보다 큰 18 로 집계된 이유
현재상태: 도구 집계 현상으로 보이나 확인하지 않았다고 10-10 testlog 가 적었다. failed 는 0 이다. 측정 도구는 총 요청을 요청 수 + warmup(16 + 2 = 18)로 보내므로 레벨 4 의 18 은 그 총 요청 수와 같다 — 레벨 1 · 2 는 16 이라 설명이 완전하지 않다(추론)
다음관측: guidellm 레벨 4 원시 JSON 의 requests 목록 길이와 warmup 포함 여부를 대조한다
출처: [testlog_26101014_knob프리셋_E2E_셀C_ornith §3, testlog_26101014_knob프리셋_E2E_셀C_ornith §6]
```

```hint-event
id: Q3
kind: open-question
물음: 같은 셀의 lite 구간이 10-01 과 10-10 에서 다르게 나온 이유(cold TTFT 259 ms 대 479 ms, warm 생성 31.62 대 32.83 t/s)
현재상태: 두 측정 모두 lite_bench 의 median_tpot 기준 단발 측정이고 벤치 명령은 같다(도구 판본 차이는 엔진 로그 머리 캡처 추가뿐). 그런데도 차이가 나며 lite 에는 반복 밴드가 없어 이 차이가 측정 잡음인지 환경 차이(컴파일 캐시 상태, 가중치 적재 시간 287 대 313 초)인지 가르지 못했다
다음관측: 같은 서빙에서 lite 를 3회 이상 반복해 lite 자체 밴드를 잰다
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §5, testlog_26101014_knob프리셋_E2E_셀C_ornith §3]
```

```hint-event
id: Q4
kind: open-question
물음: 엔진 자기보고 버전 문자열(dev 표기)이 빌드 부산물인지, 소스가 정말 v0.30.0 태그와 같은지에 대한 메인 확인
현재상태: 서브는 컨테이너 안 소스 트리의 git describe 가 v0.30.0 임을 관측하고 표기 부산물로 판단했으나, '메인 확인 필요'로 남겼다. 이미지 compose 라벨이 multi 평면 산출로 보이는 점도 출처 미확인이다
다음관측: 메인이 이미지 digest 로 소스 SHA 와 빌드 이력을 대조한다
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §1]
```

```hint-event
id: Q5
kind: open-question
물음: 실린 빌드 레시피(Dockerfile · requirements)가 측정 이미지를 지은 바이트인가, 그리고 빌드 패치 4건이 이 모델에서 실제로 발화했는가
현재상태: 실린 Dockerfile 은 이미지 history 와 11/20 만 일치하고 requirements 는 이미지 안 바이트와 다르다. 실린 Dockerfile 머리는 다른 NGC 베이스 · 다른 vLLM 버전을 가리킨다. 패치 4건은 헤더가 DeepSeek-V4 계열 대상이라고 적었고, 이 셀의 모델 구조와 일치하지 않는다. 20 번 패치의 적용은 미관측이고 나머지는 재구성 · 이미지 탐침이다
다음관측: 측정 이미지에서 원장(build_ledger)을 읽거나 docker history 전체를 보관해 레시피를 재렌더한 결과와 대조한다
출처: [Dockerfile.source-build, 10-deepgemm.sh, 20-triton-kernels.sh]
```

```hint-event
id: Q6
kind: open-question
물음: 더 높은 vLLM 버전에서 moe-backend auto 가 여전히 컴파일 직후 급락하는가, 그리고 triton 지정이 여전히 필요한가
현재상태: 이 계보는 0.30.0 한 버전에서만 관측했다. 상위 버전 · 다른 HW 관측은 계보에 없다
다음관측: 상위 버전 이미지로 같은 yaml 에서 moe-backend 만 auto 로 바꿔 한 번 기동한다
출처: [testlog_26100111_ornith15-35b-v030_싱글서브_서빙_lite §7]
```

```hint-event
id: Q7
kind: open-question
물음: 판정 PASS 의 기준이 지나치게 낮은가 — 측정이 루브릭 기대치의 약 2.5배인 이유
현재상태: 기대치(primary)는 roofline × 실현 계수(realistic_fraction)로 산정된 값이고 외부 레퍼런스 검색은 하지 않았다(벤치 리포트 경고 E-not-attempted). 따라서 PASS 는 성능 우수의 증거가 아니라 하한(floor) 위라는 표시다
다음관측: 외부 레퍼런스(같은 HW · 같은 형상)를 검색해 판정을 explicit 권한으로 다시 낸다
출처: [bench_report_26101014_ornith-1.5-35b-a3b_GB10_0.30.0 §판정]
```

열린 물음 7건이다. 이 물음들의 지위는 모두 미결이며, 가장 큰 불확실은 Q1(급락 기전)과 Q5(레시피 충실도)다.

