# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 7건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인.md` | plan | 26100107 | 0 | 13097 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source · granite42 · granite42-30b-v030 · v030 | — |
| 2 | `docs/devlog/devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030.md` | devlog | 26101017 | 0 | 2090 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 3 | `docs/testlog/testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정.md` | testlog | 26101017 | 0 | 3355 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 4 | `docs/plan/plan_26101017_knob_E2E_hint발행.md` | plan | 26101017 | 0 | 3616 | 30b · granite42 · granite42-30b-v030 · v030 | — |
| 5 | `docs/benchmark/bench_report_26101016_granite-4.2-30b_GB10_0.30.0.md` | bench_report | 26101016 | 0 | 5952 | 0.30.0 · 30b · easy-vllm:0.30.0-cu133-aarch64-source | — |
| 6 | `docs/plan/plan_26101011_벤치_knob_프리셋_자산화.md` | plan | 26101011 | 1 | 17408 | 0.30.0 · 30b · granite42 · granite42-30b-v030 · v030 | — |
| 7 | `docs/testlog/testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정.md` | testlog | 26101016 | 1 | 6790 | 30b · granite42 · granite42-30b-v030 · v030 | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**목표.** 이 셀의 출발은 2026-10-01 의 vLLM 0.30.0 bump 캠페인 plan 이다. 그 plan 은 싱글 토폴로지에서 메인 노드가 IBM Granite-4.2-30b(dense · bf16)를, 서브 노드가 다른 계열 모델을 동시에 서빙하는 것을 목표로 삼았고, 목적은 성능 판정이 아니라 0.30.0 이미지와 서브 지침 배달의 검증이었다(같은 plan §8 은 Granite 를 "느린 디코드(~5 t/s 추정) — lite 까지만 · 성능 판정 아님" 으로 묶었다). 이 셀(2026-10-10)은 그 서빙 레시피(트리플렛)를 바꾸지 않고, full-bench 측정 형상을 코드 리터럴에서 프리셋 파일로 옮긴 변경의 라이브 E2E 6셀 중 셀 A 로 다시 띄워 full-bench(GuideLLM · 반복 3)로 잰 것이다(plan_26101011_벤치_knob_프리셋_자산화 §5 · devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030 §왜 이 셀인가).

> [원문] plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §2. 사용자 결정
> | U1 | 싱글: 메인 **Granite-4.2-30b**(dense · bf16 54.5GiB) · 서브 **Ornith-1.5-35B-A3B**(하이브리드 MoE · bf16 67.0GiB) — NAS 에서 노드 1대 적재 가능한 최신 두 계열 |

**HW 출발 조건.** NVIDIA GB10 · 노드당 GPU 1 · 노드 1대(메인) · TP=1 이다(00 §0.2 사실 블록). 노드 메모리는 통합메모리이며 기동 스크립트가 읽은 총량은 `mem_total=124608MiB` 이다(측정을 서빙한 2차 기동의 로그 `docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log` L8 · L10 의 `전제=unified-memory`). 인터커넥트는 단일 노드라 쓰지 않는다. 드라이버 580.173.02 는 manifest 선언값을 측정 도구가 옮긴 것이고 노드 관측값이 아니다(00 §0.4). 분산 방식은 없다(Ray 미사용 · TP=1).

**모델 구조.** 체크포인트는 HF `ibm-granite/granite-4.2-30b` revision `70fc514076785017d93087f3d8b0676426b6b355`(base `ibm-granite/granite-4.1-30b-base`)다(00 §0.2). 2026-10-01 판독으로 `GraniteForCausalLM` · 64층 full GQA(kv_heads 8 · head_dim 128) · 네이티브 컨텍스트 131072 · 토큰당 KV 256KiB(bf16)이고(plan_26100107 §3 · 트리플렛 yaml 머리 주석 L2), MTP 층과 PLE 는 없다(체크포인트 config.json 에 MTP/nextn 계열 키 부재 · 00 §0.9 후보 표: PLE 키 부재 관측 — 서빙 설정에 speculative-config 가 없는 것은 spec decode 를 켜지 않았다는 뜻일 뿐이다). 전 층이 bf16 이고 양자화 구성은 없다(00 §0.4 양자화 구성 표). 체크포인트 크기 54.53 GiB 는 safetensors index 의 `total_size` 로 쟀고(yaml 머리 L4 `index total_size`), 이 측정의 엔진 로그도 같은 값을 보고했다(`Checkpoint size: 54.53 GiB` · 적재 후 `54.58 GiB`). 파일시스템 du 와 대조한 기록은 계보에 없다. 총/활성 파라미터 수는 계보 문서에 수치로 적혀 있지 않다(dense 이므로 둘이 같다는 것만 구조에서 나온다). 추론 파서는 모델 폴더에 동봉된 플러그인 `granite_thinking_parser.py` 이고 툴 파서는 `qwen3_coder` 다(plan_26100107 §3).

**stock 상태에서 예상된 장벽.** (i) 메모리 대역폭에 묶인 느린 디코드(plan §8 의 ~5 t/s 추정) · (ii) 131072 컨텍스트 1시퀀스를 담을 KV 절대클램프의 산정(2.2 W1 · W2) · (iii) 통합메모리에서 서빙 · 벤치 클라이언트가 같은 호스트 메모리를 나누는 예산(2.2 W3). 빌드 쪽 장벽은 이 셀 계보에 기록이 없다 — 이미지는 같은 날 멀티 캠페인이 0.30.0 으로 빌드한 것을 싱글이 그대로 쓰기로 했다(아래 D4).

> [원문] plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §4. 결정 요청
> | D4 | **싱글 메인은 멀티가 빌드한 0.30.0 이미지를 그대로 쓴다**(같은 호스트·같은 베이스·post 패치 10~40 은 GB10 일반 교정) — 셀 기록에 `image_digest`·빌드 원장을 싣는다. 싱글 렌더는 compose·env 통로만 |

**외부 선례와 사람의 결정.** GB10 위 bf16 Granite 의 외부 실측치는 찾지 못했다(셀 판정 testlog §3: 외부 레퍼런스 검색 결과 없음 · 대역폭 상한 추정 ~4.5 t/s 뿐). 사람의 결정은 모델 선택(U1)과 이미지 재사용(D4)이었고, 2026-10-10 에는 "과거 서빙 실적이 있고 이미지가 남아 있는 모델에서 골랐다"(devlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030 §왜 이 셀인가)는 이유로 E2E 셀 A 가 됐다 — plan_26101011 §5 는 "이미지·과거 실적이 확인된 것 중에서 골랐다" 와 셀 A 행 "10-01 서빙 실적(GuideLLM 밴드 없음)" 으로 적는다. 주의: 측정한 이미지는 D4 의 멀티 빌드 이미지가 아니다. 계보 밖 기록(docs/testlog/testlog_26100117_하네스결함교정_재빌드일관성_E2E_판정.md §1 C2 행 · §2)에 따르면 2026-10-01 HITL ② 에서 사람이 재빌드를 골라 single 렌더(빈 패치 슬롯)로 `--no-cache` 빌드(07:38→08:18Z)를 했고 그 결과가 이 측정의 `sha256:123c331f…` 이며, D4 의 멀티 빌드 이미지(`cdb1a38b…` · post 패치 10~40)는 그때 태그를 잃고 호스트에서 사라졌다. 이 셀 계보 안에서는 빌드 원장에 post 패치 기록이 없고 층 창이 2026-10-01T07:38Z~08:14Z 라는 관측(01 §1.1 · §1.4)이 이와 맞는다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-10T05:07:56Z(`camp-26101017-knob-e2e-single` · evidence.event_campaign_boundary): campaigns/camp-26101017-knob-e2e-single/campaign.yaml declared_utc 2026-10-10T08:04:57Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-10T05:07:56Z → 경계 = 앞선 값 2026-10-10T05:07:56Z(이전 행 = 같은 셀 이름 · 앞선 캠페인) · 단 이전 기동 중 1건은 캠페인 선언 전 · 같은 셀 기동(세션 id 가 이 셀 기록에 관측: serve-granite42-30b-v030-20261010T045514Z = docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log:15)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-07-31T03:27:43Z → 2026-10-10T07:46:20Z(`docs/logs/main/events/2026-07.jsonl` · `docs/logs/main/events/2026-08.jsonl` · `docs/logs/main/events/2026-09.jsonl` · `docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-granite42-30b-v030` | 2026-10-10T04:55:14Z | 1회 · 첫 2026-10-10T04:55:15Z | 없음 | `budget_clear` 2026-10-10T05:07:27Z | 5 |
| 2 | main | `serve-granite42-30b-v030` | 2026-10-10T05:07:56Z | 17회 · 첫 2026-10-10T05:07:57Z | 없음 | `budget_clear` 2026-10-10T07:15:02Z | 13 |

**같은 셀 이름 · 앞선 캠페인의 기동 2회**(캠페인 경계 2026-10-10T05:07:56Z 이전 — 라벨이 같아 원장에서 함께 묶였을 뿐 이 셀의 시도로 세지 않는다 · 행 전문 = 01 §1.4)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 앞선-1 | main | — | —(선언 행 없음) | 없음 | 없음 | —(닫힘 행 없음) | 8 |
| 앞선-2 | main | `serve-granite42-30b-v030` | 2026-10-01T08:26:06Z | 1회 · 첫 2026-10-01T08:26:08Z | 없음 | `budget_clear` 2026-10-01T08:41:10Z | 6 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

이 셀의 계보에서 넘은 벽은 셋이다. W1 · W2 는 2026-10-01 최초 서빙에서 KV 절대클램프를 잡다가 vLLM 이 기동을 거부한 두 회차이고, W3 는 2026-10-10 측정에서 벤치 예산 게이트가 측정 전에 멈춘 것이다. 셋 다 호스트 위험이 아니라 가드의 정상 차단이었다. 위 원장 표의 "이 셀의 기동 시도" 는 측정을 서빙한 1회(2026-10-10T05:07:56Z 선언 · 갱신 17회 · 사살 없음)이고, 같은 날 04:55:14Z 의 앞선-3 은 도구가 캠페인 경계 이전이라 "앞선 캠페인" 으로 분류했지만 시각상 셀 판정 testlog 가 말하는 이번 E2E 의 1차 기동이다(관측 — 1차 기동 로그 a_serve.log L15 · L22 의 세션 id `…20261010T045514Z` 가 원장 앞선-3 행과 일치하고 셀 판정 testlog §1 이 1차 기동을 적는다). 2026-10-01 의 W1 · W2 회차는 원장에 예산 선언 행이 없는 앞선-1 묶음(serve_start/stop 4쌍)에 들어 있을 것으로 보이나, 회차와 행의 대응은 계보 안 문서가 적지 않았다.

### W1 KV 절대클램프 16GiB — 131072 1시퀀스를 담지 못해 기동 거부 (2026-10-01)

최초 서빙 레시피를 저작하면서 KV 클램프를 16GiB 로 선언했다가 vLLM 이 기동을 거부했다. 계보 안에서 이 회차를 기록한 것은 서빙 yaml 머리 주석 하나뿐이다(당시 엔진 로그는 이 draft 의 계보 · 증거 후보에 없다). 원인은 토큰당 KV 256KiB(plan_26100107 §3) × max-model-len 131072 한 시퀀스가 16GiB 보다 크다는 것이고, 해소는 클램프를 키우는 것이었다(최종 33.0 GiB — W2). 통과 판정은 이 측정의 엔진 로그가 KV 를 135,168 토큰으로 잡고 131,072 토큰 요청 기준 최대 동시성 1.03x 를 보고한 것이다.

```hint-event
id: W1
kind: wall
증상: KV 절대클램프 16GiB 로 기동하자 vLLM 이 기동을 거부했다
서명: "kv_cache: 33.0 GiB 절대 클램프 — 131072 1시퀀스 + 블록 올림 여유(try1 16GiB 거부"
원인: 토큰당 KV 256KiB × max-model-len 131072 한 시퀀스가 16GiB 를 넘는다(필요량 미달)
해소: kv-cache-memory-bytes 를 키움 — 최종 35433480192(33.0 GiB · W2 참조)
검증: 이 측정 엔진 로그 L82 GPU KV cache size 135168 tokens · 최대 동시성 1.03x
전이등급: arch-invariant
출처: [granite42-30b-v030.yaml, plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3, output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log]
```

### W2 KV 절대클램프 32GiB — 수용 토큰 16 개 미달로 기동 거부 (2026-10-01)

필요량에 맞춰 32GiB 로 다시 잡자, 담을 수 있는 토큰이 131056 으로 계산돼 max-model-len 131072 에 16 토큰 모자라 다시 거부됐다(거부와 131056 은 관측). 해소는 블록 올림 여유를 더한 33.0 GiB(`35433480192` 바이트)였고, 그 값으로 섰다. 거부가 메모리 고갈이 아니라 엔진의 기동 시 수용 검사였다는 것은 관측이지만, 16 토큰이 왜 빠졌는지의 기전은 **미결**이다 — yaml 주석은 "블록 정렬" 로 해석했으나, plan 의 토큰당 KV 256KiB 로는 32GiB 가 정확히 131072 토큰이라 단순 블록 정렬 손실로 설명되지 않는다(사실 검증 지적 · 계보 밖 원시 엔진 로그는 추정 최대 길이 131056 만 말한다). "블록 여유를 더해 선언" 처방의 근거는 33.0 GiB 에서 섰다는 관측뿐이다. 같은 줄이 이 회차를 `try2` 로 부르는데, 계보 밖의 2026-10-01 판정 testlog 는 같은 32GiB 회차를 try3 으로 번호 매겼다(이 draft 의 LINEAGE 에 없는 문서 — 번호 차이만 기재하고 인용하지 않는다).

> [원문] granite42-30b-v030.yaml §L5-6
> #   kv_cache: 33.0 GiB 절대 클램프 — 131072 1시퀀스 + 블록 올림 여유(try1 16GiB 거부 · try2 32GiB 는 블록 정렬로 131056 토큰 = 16 토큰 미달 거부 · 2026-10-01 실측)
> #   gmu 0.80 천장(통합메모리 · 호스트 안전 여유)

```hint-event
id: W2
kind: wall
증상: KV 절대클램프 32GiB 로 다시 기동하자 엔진이 131056 토큰만 수용 가능하다고 계산해 다시 거부했다
서명: "try2 32GiB 는 블록 정렬로 131056 토큰 = 16 토큰 미달 거부"
원인: 32GiB 에서 엔진이 계산한 수용 토큰이 131056 으로 max-model-len 에 16 토큰 미달 — 16 토큰이 빠진 기전(yaml 주석의 블록 정렬 해석)은 미결
해소: 여유를 더해 kv-cache-memory-bytes 35433480192(33.0 GiB) — 그 값에서 기동 성공(관측)
검증: 이 측정 엔진 로그 L81 reserved 33.0 GiB memory for KV Cache · L82 135168 tokens 1.03x
전이등급: arch-invariant
출처: [granite42-30b-v030.yaml, output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log]
```

### W3 벤치 예산 게이트 BLOCK — 벤치 8192 MiB 가 서빙을 워치독 사정거리에 넣는다 (2026-10-10 · 1차 스윕)

E2E 1차 스윕에서 Agent 는 프리셋 선택지 안의 `bench_budget_mib` 8192 를 골랐다. lite 레그는 통과했지만(lite 게이트 pass) 레벨 1 진입 직전 `run_bench` 의 예산 사전검사가 막았다. 통합메모리에서 이 서빙의 선언 바닥은 16,543 MiB 이고(총량 124608 − 가중치 55841 − KV 33792 − overhead 18432 · 모두 선언값 · 2.5), 그 바닥에서 벤치 컨테이너 몫 8192 를 빼면 절대밴드 10240 아래로 내려가므로 게이트가 벤치를 시작하지 않았다. 해소는 프리셋 선택지 안에서 4096 을 다시 고른 것이다(plan_26101011 §3.2 의 `bench_budget_mib` 선택지 `4096·8192 auto`). 부작용으로 lite 레그 1회가 낭비됐고(testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §5), 1차 기동은 예산 게이트 차단으로 측정 없이 05:07:27Z 에 내려갔고(셀 판정 testlog §1) 05:07:56Z 에 다시 선언 · 기동됐다(원장) — 같은 서빙 위에서 스윕만 다시 돌리지 않고 재기동한 이유는 계보에 적혀 있지 않다. 게이트가 인용한 "벤치가 죽는다" 는 기전은 다른 캠페인의 실측이며, 이 셀에서 8192 벤치가 실제로 서빙을 죽이는지는 관측되지 않았다(게이트가 측정 전에 멈췄다).

> [원문] docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log §L9-13
> [budget-preflight] weights=(선언된 바닥 사용) floor=16543MiB arm_ceiling=13471MiB declare=OK bench=BLOCK
>   - 벤치 진입 불가: 바닥 16543MiB − 벤치 8192MiB = 8351MiB < 절대밴드 10240MiB. 벤치 예산 상한은 이 구성에서 6303MiB 다 — 그보다 크면 벤치 컨테이너가 뜨는 순간 워치독 사정거리에 들어간다(camp-26090918 `fp8-bf-262k-mmp` 실측: 서빙은 성공하고 벤치가 죽었다).
> [run_bench] STOP(벤치 예산 게이트): 이 몫으로 벤치를 띄우면 서빙이 사정거리에 든다.
> [run_bench]   벤치를 **시작하지 않았다**. --bench-budget-mib 를 위 상한 이하로 낮추거나
> [run_bench]   KV 절대클램프를 낮춰 바닥을 올려라(그래야 둘 다 산다).

```hint-event
id: W3
kind: wall
증상: 1차 스윕 레벨 1 진입 전 run_bench 예산 사전검사가 bench=BLOCK 으로 벤치를 시작하지 않았다(sweep_rc 3)
서명: "벤치 진입 불가: 바닥 16543MiB − 벤치 8192MiB = 8351MiB < 절대밴드 10240MiB"
원인: 통합메모리에서 서빙 선언 바닥 16543 MiB 에서 벤치 컨테이너 몫 8192 MiB 를 빼면 절대밴드 10240 MiB 미만(상한 6303 MiB)
해소: bench_budget_mib 를 프리셋 선택지 안 4096 으로 다시 골라 완주
검증: testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §2 sweep_index.bench_knobs bench_budget_mib 4096 · bench_mode full · 판정점 반복 3/3
전이등급: arch-scaled
출처: [docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log, testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §4, testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §3]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: full-bench 1차 스윕에서 bench_budget_mib 8192(Agent 선택 · 근거 코드 host_budget)
기각사유: run_bench 예산 사전검사가 바닥 16543 − 8192 = 8351 < 절대밴드 10240 으로 측정 전 차단(상한 6303 MiB) — 게이트가 옳다고 판정(정상 차단)
재개조건: KV 절대클램프를 낮춰 서빙 선언 바닥이 올라가거나 호스트 메모리 총량이 큰 HW
출처: [testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §4, docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log]
```

```hint-event
id: R2
kind: rejected
시도: KV 절대클램프를 131072 1시퀀스의 정확 필요량(32GiB)으로 선언하는 산정 방식
기각사유: 엔진 계산 수용 토큰 131056 으로 16 토큰 미달 거부(W2 · 빠진 기전은 미결) — 33.0 GiB 로 여유를 더해 섰다
출처: [granite42-30b-v030.yaml]
```

```hint-event
id: R3
kind: rejected
시도: KV 절대클램프 16GiB(최초 선언)
기각사유: 131072 1시퀀스 필요량 미달로 vLLM 이 기동 거부(W1)
출처: [granite42-30b-v030.yaml]
```

이 셀의 형상을 좁힌 것은 두 가지다. 서빙 쪽은 "131072 컨텍스트 1시퀀스 + 블록 여유" 라는 KV 하한(R2 · R3)이고, 측정 쪽은 그 KV 가 만든 통합메모리 바닥이 벤치 클라이언트 몫의 상한을 6303 MiB 로 묶은 것(R1)이다. 프리셋의 레벨 16 은 시도된 적이 없다 — 서빙 yaml 의 max-num-seqs 8 을 넘어 측정 전 상한 필터가 걸렀으므로(testlog_26101016 §2.1) 기각이 아니라 범위 밖이다. 서빙 레시피 자체를 바꾼 시도는 2026-10-10 계보에 없다(devlog 최종 상태: 트리플렛 무변경).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

이 셀 계보의 오진은 한 건이다. 셀 판정 testlog 초판은 이번 측정을 서빙한 2차 기동의 READY 를 510 s 로 잘못 적었다(정정문 "초판은 2차를 510 s 로 잘못 적었다" — 초판 본문은 남아 있지 않다). 그러나 2차 기동의 원시 로그는 health 대기 475 s 를 기록했고, 원장의 예산 선언(05:07:56Z · 2026-10.jsonl)에서 엔진 로그의 API 서버 기동 줄(05:15:53 · lite 엔진 로그 L104)까지가 7m57s 다(01 §1.4 이벤트 표 38행 — 그 행의 출처는 원장이 아니라 엔진 로그다). 510 s 는 1차 기동(이 셀 측정에 쓰이지 않은 서빙)의 값이다. 이 지적에 따라 2026-10-10 에 출처 문서가 정정됐다 — 셀 판정 testlog §1 · 셀 devlog · 6셀 묶음 testlog 의 셀 A 행이 이제 2차 475 s · 1차 510 s 를 적고 초판의 오기를 밝힌다. 이 페이로드는 측정 서빙의 기동 소요로 475 s 를 쓴다.

> [원문] docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log §L21-22
> [single-up] 7/7 health 대기     : DONE (475s)
> [single-up] READY container=granite42-30b-v030-serving-container port=8000 node_id=main session=serve-granite42-30b-v030-20261010T050756Z

```hint-event
id: M1
kind: misdiagnosis
증상: 측정 서빙(2차 기동)의 READY 소요를 판정 문서 초판이 1차 기동과 같은 510 s 로 적었다
서명: "2026-10-10 정정: 초판은 2차를 510 s 로 잘못 적었다"
원인: 두 기동 로그 중 1차(a_serve.log)의 값을 2차에도 옮긴 기재 오류로 보인다(추론 — 정정문도 경위는 적지 않았다)
해소: 2차 기동(세션 …20261010T050756Z) health 대기 = 475 s(a_serve2.log L21) · 1차 = 510 s(a_serve.log L21) · 원장 선언(05:07:56Z) → 엔진 로그 API 서버 기동(L104) = 7m57s
검증: testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1 정정문 · a_serve2.log L21 · lite 엔진 로그 L104
전이등급: judgment
현재지위: 반증
출처: [testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1, docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log, docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve.log]
```

교정 묶음(항목 id 가 붙은 교정 목록)은 이 계보에 없다. 6셀 묶음 testlog 의 사건표 E1~E5(5건)를 훑었고 이 셀에 해당하는 것은 E1(1차 스윕 벤치 예산 게이트 차단 — W3 · R1) 하나다. 루프라인 바닥(floor 1.39)의 의미가 약하다는 plan 의 위험 기재는 오진이 아니라 판정 표면의 한계이므로 2.7 Q2 에 둔다.

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 절대클램프(V4).** 처음 16GiB → 거부(W1) → 32GiB(1시퀀스 정확 필요량) → 16 토큰 미달 거부(W2 · 빠진 기전 미결) → 33.0 GiB(`35433480192`)로 섰다. 필요량의 근거는 plan 의 토큰당 KV 256KiB(bf16) × 컨텍스트 131072 이고, 설정값이 필요량에 대해 갖는 여유는 엔진이 스스로 보고한 "131,072 토큰 요청 기준 최대 동시성 1.03x" 가 정본이다(새로 계산하지 않는다). 즉 이 값은 음성대조용 과잉값이 아니라 1시퀀스 하한에 블록 여유만 더한 값이며, 2026-10-01 에 값을 바꿔 가며 잰 기록이 있으므로 tuned 다. 이 측정 엔진은 이 값을 받아 메모리 프로파일링을 건너뛰고 gpu-memory-utilization 을 따르지 않는다고 명시했다(아래 V2 와 연결).

> [원문] output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log §L82-82
> (EngineCore pid=276) INFO 10-10 05:14:39 [kv_cache_utils.py:2395] GPU KV cache size: 135,168 tokens, Maximum concurrency for 131,072 tokens per request: 1.03x

**호스트 메모리 예산 산식.** 기동 스크립트의 선언은 바닥 = 총량 − 가중치 − KV − overhead 꼴이다. 이 측정의 대입값은 총량 124608 · 가중치 55841(체크포인트 크기 선언 · ckpt÷tp) · KV 33792 · overhead 18432 → 바닥 16543 MiB · 워치독 arm 상한 13471 MiB 이고, 전부 **선언값**이다(측정 아님). 엔진 로그 실측은 가중치 적재 54.58 GiB · KV 33.0 GiB 이며 lite 의 용량 분해는 예약 합계 ~87.6 GiB · 시스템 RAM 98.0 GiB(81%) 다(03 §3.1). overhead 18432 는 셀 판정 testlog 의 기동 명령 `--overhead-mib 18432` 로 사람이 넣은 값이고, 2026-10-01 의 같은 셀 기동(원장 9행)은 overhead 16384 · 바닥 18591 로 선언했다 — 두 값을 바꾼 이유는 계보에 없다. gmu 0.80 의 "예상 vLLM 몫" 줄은 게이트가 아닌 기재다(로그가 `게이트 ✗` 로 표시).

> [원문] docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log §L8-13
> [single-up] 2/7 모델·RAM 게이트 : DONE (상주 ckpt=55841MiB ÷ tp=1 → weights=55841MiB · ple=resident · kv=33792MiB · mem_total=124608MiB)
> [single-up]     floor=16543MiB arm_ceiling=13471MiB (가드 최소 8192MiB)
> [single-up]     declared-gmu 기재(게이트 ✗): gmu=0.8 예상 vLLM 몫=99686MiB 잔차(몫−weights−kv)=10053MiB provenance=declared status=recorded 전제=unified-memory
> [single-up]     선언 발행: <repo>/docs/logs/main/serve_budget.env
> [single-up]       예상 상주 108065 MiB → 예상 바닥 16543 MiB · 만료 epoch 1791616076 (TTL 7200s)
> [single-up]       워치독 예상 arm 상한 = 16543 - 3072 = 13471 MiB

**gpu-memory-utilization 0.80(V2).** 2026-10-01 셀 저작 단계에서 "gmu 0.80 천장(통합메모리 · 호스트 안전 여유)" 으로 정한 값을 승계했다(plan_26100107 §Phase S S4 · yaml 머리 L6). 이 셀에서 바꿔 잰 적이 없고, kv-cache-memory-bytes 가 지정돼 있어 엔진은 이 값을 KV 산정에 쓰지 않는다(엔진 로그 L81 `This does not respect the gpu_memory_utilization config`).

**max-model-len 131072(V3) · dtype bfloat16(V1) · kv-cache-dtype auto(V5).** 셋 다 모델의 네이티브 값(컨텍스트 131072 · torch_dtype bfloat16)을 그대로 옮긴 승계값이거나 엔진 선택값이다. kv-cache-dtype `auto` 는 엔진이 모델 dtype 을 따르게 둔 것이다(측정 도구가 엔진 로그에서 `auto` 를 관측 · 00 §0.2 출처).

**max-num-seqs 8(V6).** 값의 출처를 적은 계보 문서는 없다. 이 셀에서 바꿔 잰 적도 없다. 측정에서는 프리셋 레벨 16 을 거르는 상한으로만 작용했다(testlog_26101016 §2.1). KV 가 131072 기준 1.03x 이므로 긴 요청 여러 개가 동시에 들어오면 엔진이 대기열로 돌릴 것으로 보인다(추론 — 관측되지 않았다 · Q4) — 측정 형상(입력 1024 · 출력 256)에서는 문제되지 않았다(동시성 8 완료 16/0).

**graph/eager.** 서빙 yaml 에 enforce-eager 가 없어 엔진 기본(CUDA graph)으로 돌았다 — 캡처 32 s · 0.53 GiB(엔진 로그 L94). 캡처 스파이크 때문에 eager 를 고려한 기록은 없다.

**스윕을 계획했으나 시도하지 않은 레버.** 서빙 레버 스윕은 계획도 시도도 없다(2026-10-10 의 통제변인은 측정 형상 프리셋이었다 — 셀 config `declared_axes.control_variable`). 그러므로 V1~V3 · V5 · V6 은 이 셀에서 조정된 적 없는 승계값이다. 판정 산정에 손으로 승계된 입력은 없다 — spec 이 꺼져 있어 acceptance length 는 선언 부재(1.0 · `declared-absent`)로 처리됐고, 외부 레퍼런스 E 는 검색 결과 없음이다(verdict.json).

**이 값 · 형상이 버티는 기전 대 다른 값이 실패한 이유.**
- KV 16GiB — 바뀐 것: kv-cache-memory-bytes 하나 — 엔진의 기동 시 수용 검사가 1시퀀스(131072 토큰) KV 를 담을 수 없다고 거부(메모리 고갈이 아니라 사전 거부) — 확정 — granite42-30b-v030.yaml L5.
- KV 32GiB — 바뀐 것: kv-cache-memory-bytes 하나 — 엔진이 131056 토큰만 수용 가능하다고 계산해 같은 수용 검사에서 16 토큰 미달(거부는 확정) · 16 토큰이 빠진 기전(블록 정렬 해석)은 미결 — granite42-30b-v030.yaml L5.
- KV 33.0 GiB(현재) — 135,168 토큰 확보로 수용 검사를 통과 — 확정 — 이 측정 엔진 로그 L82.
- 벤치 예산 8192 대 4096 — 바뀐 것: 벤치 컨테이너의 메모리 상한 하나(서빙 설정 동일) — 통합메모리에서 서빙 바닥과 벤치 몫이 같은 호스트 메모리를 나누므로 8192 는 워치독 절대밴드 아래로 바닥을 깎는다는 게이트 산술로 차단 — 게이트 판정은 확정이나 "벤치가 실제로 서빙을 죽인다" 는 기전은 이 셀에서 관측되지 않아 미결(게이트가 인용한 실측은 다른 캠페인) — W3.
- overhead 16384(2026-10-01) 대 18432(2026-10-10) — 바뀐 것: overhead 선언값과 그에 따른 바닥(18591 → 16543) — 두 기동 모두 원장에 사살 · 트립 행이 없어 어느 쪽이 필요조건인지 가르지 못했다 — 미결 — 01 §1.4 원장 9행 · 18행.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- KV 절대클램프를 1시퀀스 정확 필요량으로 선언하지 마라 — W2 · R2 — 블록 올림 여유를 더해 선언하고, 엔진 로그의 `GPU KV cache size … Maximum concurrency … 1.0Nx` 가 1.0x 이상인지로 확인한다.
- KV 클램프를 컨텍스트 필요량 아래로 두고 기동하지 마라 — W1 · R3 — 토큰당 KV 바이트(모델 config 의 층 · kv_heads · head_dim · dtype)와 목표 컨텍스트에서 하한을 먼저 잡는다.
- kv-cache-memory-bytes 를 둔 채 gpu-memory-utilization 을 바꿔 KV 를 조절하려 하지 마라 — V2 · 엔진 로그 L81 — 엔진이 gmu 를 따르지 않으므로 KV 는 클램프 값으로만 움직인다.
- 통합메모리 노드에서 벤치 예산(bench_budget_mib)을 서빙 바닥과 따로 고르지 마라 — W3 · R1 — 서빙 선언 바닥 − 벤치 몫이 워치독 절대밴드 이상인지 먼저 계산하고(게이트가 상한을 출력한다) 그 이하에서 고른다.
- 예산 선언 없이 기동하지 마라 — testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1 — `single_serve_up.sh` 처럼 바닥 · arm 상한을 선언하는 경로로만 띄운다(overhead 는 선언값이며 실측이 아니다 · 2.5).
- 기동 소요를 판정 문서의 한 줄에서 옮기지 마라 — M1 — 회차별 기동 로그(health 대기 줄)와 원장 선언 시각 · 엔진 로그의 API 서버 기동 줄로 확인한다.
- 엔진 기동 배너의 버전 문자열(`0.30.1.dev0+gced6857af…`)을 빌드 입력으로 쓰지 마라 — 00 §0.4 — 재현 좌표는 릴리스 태그 `v0.30.0` 또는 40자 SHA 다.
- 가중치를 네트워크 파일시스템에서 바로 읽는 기동을 느린 것이 벽이라고 오해하지 마라 — 이 측정 엔진 로그 L57-58 · L73 — CIFS 이면서 체크포인트가 가용 RAM 의 90% 를 넘으면 auto-prefetch 가 꺼지고 이 측정의 적재는 370 s 였다(정상 · 두 숫자 사이 인과는 대조 없는 추론). 줄이려면 로컬 디스크로 옮기는 것이 대신할 일이다(이 셀에서 시도하지 않은 추론).
- 기동 로그의 `Failed to import Triton kernels … triton_kernels.matmul_ogs` ERROR 를 벽으로 보지 마라 — 이 측정 엔진 로그 L49 · L56 — 이 셀은 그 뒤 정상 기동 · 측정했다. 이 줄이 가리키는 MXFP4 경로를 bf16 dense 모델이 쓰지 않는다는 것은 추론이다.
- 다른 셀의 측정값을 이 셀의 근거로 옮기지 마라 — 03 §3.5 — 같은 E2E 의 다른 셀은 강한 일치 키(모델 · vLLM)가 달라 비교가 무효다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 이 셀의 decode 물리 상한은 얼마이며 3.93 t/s 는 그 몇 % 인가 — 계보의 세 추정이 서로 다르다
현재상태: 셀 판정 testlog 는 대역폭 상한 추정 ~4.5 t/s · 6셀 testlog §4 는 대역폭 한계 ~5 t/s 의 ~78 % · 루프라인은 R_fp 4.66 t/s(대역폭 273 GB/s lookup · active 54.532 GiB) — 셋 다 추정이고 실측 대역폭이 아니다 · 측정은 레벨 1~8 에서 3.83~3.96 으로 평탄
다음관측: 이 노드의 실측 메모리 대역폭(STREAM 류) 1회로 R_fp 의 대역폭 입력을 lookup 에서 measured 로 바꾼다
출처: [testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §3, testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §4, output/single/benchlog/sweep_granite42-30b-v030/roofline.json]
```

```hint-event
id: Q2
kind: open-question
물음: PASS 의 기준인 expected_achievable 바닥(floor 1.39)이 이 셀에서 의미 있는 합격선인가
현재상태: 외부 레퍼런스 E 검색 결과 없음 → 루프라인×MBU 바닥으로 판정(ratio 2.411) · 발행 plan 이 이 바닥을 의미가 약하다고 기재 · 권한 weak 라 인증서 없음
다음관측: GB10 bf16 dense 30B 급의 외부 실측치(E) 1건 또는 사용자 목표 c 를 정해 explicit 권한으로 재판정
출처: [plan_26101017_knob_E2E_hint발행 §3, bench_report_26101016_granite-4.2-30b_GB10_0.30.0 §판정]
```

```hint-event
id: Q3
kind: open-question
물음: 레벨 2 의 완료 15 · 레벨 4 의 완료 18 은 요청 16 과 왜 다른가
현재상태: 대표 run 기준 완료/실패 L1 16/0 · L2 15/0 · L4 18/0 · L8 16/0 · 실패 0 · 6셀 testlog 는 다른 셀(C·D)의 completed 18 을 워밍업 집계 혼입으로 보인다고만 적었다(원인 미조사)
다음관측: 레벨 2 · 4 의 guidellm JSON 에서 warmup 구간 요청이 completed 에 섞였는지 요청별 타임스탬프로 확인
출처: [bench_report_26101016_granite-4.2-30b_GB10_0.30.0 §부하 스윕 곡선, testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §4]
```

```hint-event
id: Q4
kind: open-question
물음: 긴 컨텍스트 요청이 동시에 들어올 때 이 레시피는 어떻게 거동하는가
현재상태: KV 135168 토큰 · 131072 기준 최대 동시성 1.03x · max-num-seqs 8 · 측정은 입력 1024 · 출력 256 에서만 했다 — 긴 요청의 동시 처리는 관측되지 않았다
다음관측: 입력 32k 이상 요청 2개 동시 투입 시 대기열 · 선점 로그와 decode t/s
출처: [output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log, testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §2]
```

```hint-event
id: Q5
kind: open-question
물음: 호스트 예산의 overhead 선언(16384 → 18432 MiB)은 실측으로 얼마가 필요한가
현재상태: 두 값 모두 선언이며 바꾼 이유는 계보에 없다 · 두 기동 모두 원장에 사살 · 트립 없음 · 이 측정의 시스템 RAM 점유 98.0 GiB(81%)
다음관측: 기동 피크 구간 MemAvailable 최저값 실측으로 overhead 를 measured 로 대체
출처: [testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1, docs/logs/main/events/2026-10.jsonl]
```

Q1 은 판정 등급(Q2)과 이어져 있다 — 물리 상한을 실측하면 expected 바닥의 입력도 바뀐다. 상위 vLLM 버전의 자체 패치 필요성은 묻지 않는다: 이 셀은 런타임 · 빌드 패치를 싣지 않았다(01 §1.1). 현행 full 정의로 이 측정은 충족(반복 3 · GuideLLM)이다(03 §3.5 사실 블록).
