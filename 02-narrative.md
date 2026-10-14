# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 5건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 발행자 평면 draft `inputs/LINEAGE.full.json` 의 `evidence_candidates` 다(페이로드 `LINEAGE.json` 은 수신자 요약 — plan_26092908 §4.6).

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/plan/plan_26101219_vllm_0_31_0_KV양자화_최대동시성_벤치_계획.md` | plan | 26101219 | 0 | 7451 | 0.31.0 · 4b · qwen3 · qwen3-4b | — |
| 2 | `docs/testlog/testlog_26101317_qwen3-4b_KV양자화_동시성_판정.md` | testlog | 26101317 | 0 | 3334 | 0.31.0 · 4b · qwen3 · qwen3-4b | — |
| 3 | `docs/benchmark/bench_report_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.md` | bench_report | 26101413 | 0 | 2993 | 0.31.0 · 4b · qwen3 · qwen3-4b | — |
| 4 | `docs/devlog/devlog_26101413_kvq_concurrency_캠페인_종결.md` | devlog | 26101413 | 0 | 2979 | 0.31.0 · 4b | — |
| 5 | `docs/report/perf_26101413_KV양자화_최대동시성_벤치_결과.md` | report | 26101413 | 1 | 3578 | 0.31.0 · 4b · qwen3 · qwen3-4b | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

(1) **목표** — vLLM 0.31.0을 NGC 베이스에 빌드하고, 이 셀(Qwen3-4B)을 포함한 3개 모델에서 KV 캐시 dtype 5종 × 입력 길이별 최대 동시성을 실측한다. 가중치는 원본 BF16, 양자화 대상은 KV 캐시뿐이고 dtype 간 다른 변수는 고정한다(비교 공정성).

(2) **HW 출발 조건** — NVIDIA RTX PRO 6000 Blackwell Workstation Edition ×1(VRAM 95.6GiB) · 단일노드 TP=1 · 호스트 RAM 62GB(discrete — 통합메모리 아님) · 드라이버 580.178.04(manifest 선언) · 인터커넥트는 단일 GPU라 무관 · 분산 없음.

(3) **모델 구조** — Qwen/Qwen3-4B(HF revision 미관측 · 체크포인트 .git 부재) · BF16 약 8GB · 36층 full-attention(GQA 8 KV heads) · 네이티브 컨텍스트 40,960(RoPE/YaRN 확장 미적용) · MTP 층 없음 · 양자화 없음(config.json에 quantization_config 부재 관측 — 00 §0.2).

(4) **출발 시 예상 장벽** — torch 2.13 선언과 NGC 베이스의 ABI 불일치 가능성(빌드 트랙 분기) · sm_120의 최신 아키텍처 지원 여부 · 호스트 열·전력 한계(사전에 L1 안전체계 설치 결정).

(5) **외부 선례와 사람의 결정** — plan은 착수 전 HITL 합의 문서로, 사람은 KV dtype 5종 · 출력 1024 고정 · RoPE/YaRN 미적용 · 모델 순서(4B → 32B → Muse)와 테스트 완료 후 가중치 삭제를 인터뷰로 확정했다.

> [원문] plan_26101219_vllm_0_31_0_KV양자화_최대동시성_벤치_계획 §1. 목표
> vLLM **0.31.0**(GitHub 정식 릴리스 2026-10-05, pre-release 아님)을 NGC 베이스 위에 빌드하고, 3개 모델 각각에 대해
> KV 캐시 dtype 5종별로 입력 길이 7K / 30K / 130K 의 **최대 동시성**을 실측한다. 모델이 지원하지 않는 길이는 `N/A`(사유 기재)로 남긴다.

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

> **캠페인 경계** 2026-10-12T10:49:53Z(`camp-26101210-kvq-concurrency` · evidence.event_campaign_boundary): campaigns/camp-26101210-kvq-concurrency/campaign.yaml declared_utc 2026-10-12T10:49:53Z · 이 발행이 묶인 측정을 서빙한 budget_declare 창 시작 2026-10-14T04:54:06Z → 경계 = 앞선 값 2026-10-12T10:49:53Z(이전 행 = 같은 셀 이름 · 앞선 캠페인)

> **원장 관측 범위**(노드별 첫 행 → 마지막 행 · evidence.event_ledger_spans): main = 2026-10-12T10:41:33Z → 2026-10-14T04:56:14Z(`docs/logs/main/events/2026-10.jsonl` · `docs/logs/main/events/thermal.jsonl` · `docs/logs/main/events/watchdog.jsonl`)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `serve-qwen3-4b` | 2026-10-14T04:54:06Z | 1회 · 첫 2026-10-14T04:54:06Z | 없음 | `budget_clear` 2026-10-14T04:56:14Z | 4 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

### W1 v0.31.0에서 `use_existing_torch.py` 가 `tools/` 로 이동 — 첫 빌드 실패

vLLM v0.31.0 소스 트리에서 `use_existing_torch.py`가 저장소 루트에서 `tools/`로 이동했고 확장 모듈 이름도 바뀌었다(`_C_stable_libtorch` 등). 기존 Dockerfile 스탠자는 루트 경로만 호출해 첫 빌드가 실패했다. 해소는 템플릿을 두 경로 모두 감지하는 버전적응형으로 고친 것이다(없으면 exit 1로 fail-loud). 이 셀은 torch 2.13 선언 때문에 source-build 트랙이라 이 스크립트가 NGC torch를 유지하는 핵심이다. 판정은 이후 전 모델 기동이 이 이미지로 성공한 것으로 확인했다.

```hint-event
id: W1
kind: wall
증상: vLLM v0.31.0 소스빌드가 use_existing_torch.py 를 찾지 못해 실패
서명: "Dockerfile 템플릿 버전적응(use_existing_torch tools/ 이동)"
원인: v0.31.0에서 스크립트가 tools/ 로 이동하고 확장 모듈이 개명됐는데 템플릿이 옛 경로만 호출
해소: Dockerfile.source-build.template 을 버전적응형(if tools/ … elif 루트 … else exit 1)으로 수정
검증: 이후 이미지 빌드 성공 · 전 dtype 기동 통과(devlog_26101413 §S4 커밋 대상)
전이등급: arch-invariant
출처: [devlog_26101413_kvq_concurrency_캠페인_종결 §S4 커밋 대상, artifacts/build_recipe/Dockerfile.source-build]
```

### W2 FlashInfer/nvcc JIT 컴파일 발열이 열 워치독 사살을 부른다

fp8과 tq 계열 기동 때 torch.compile·autotune이 CPU를 순간 과열해 열 워치독이 서빙 컨테이너를 kill 했다(GPU는 한가한데 SoC가 100°C를 넘는다). 해소는 네 겹이다 — ① serve_env `MAX_JOBS=1`로 nvcc/autotune 팬아웃을 직렬화(트리플렛 .sh) ② JIT/컴파일 캐시를 호스트에 영속 마운트해 재기동 시 cold JIT를 피함(compose `./cache/vllm` · `./cache/flashinfer`) ③ 벤치 클라이언트를 E-core(8-19)에 피닝 ④ 측정 런 사이 CPU ≤55°C 쿨다운 게이트. 왜 통하는가: JIT 발열은 순간 병렬 컴파일이 만드는 피크라, 직렬화와 캐시 재사용으로 피크 자체를 없앤다. 판정은 완화 적용 뒤 bf16/fp8 구간 thermal_trip 누계가 0으로 완주한 것이다.

```hint-event
id: W2
kind: wall
증상: 기동·컴파일 구간에 열 워치독이 서빙 컨테이너를 kill (GPU 저전력 · 순수 CPU 발열)
서명: "3차 런 전체 thermal_trip 누계 240 → 240 (bf16/fp8 구간 신규 트립 0, tq4 구간에서만 2회)"
원인: torch.compile·FlashInfer autotune의 순간 CPU 팬아웃이 SoC hard 임계를 넘음
해소: serve_env MAX_JOBS=1 · JIT 캐시 영속 마운트 · E-core 피닝 · 런당 쿨다운 ≤55°C
검증: 3차 런 bf16/fp8 구간 thermal_trip 증가 0(testlog_26101317 §열 사망 증거 (tq4))
전이등급: judgment
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4), artifacts/compose/docker-compose.yaml, artifacts/triplet/qwen3-4b-kv-bf16.sh]
```

### W3 열전력 워치독의 전력 보정이 실측과 어긋났다

호스트 안전체계(L1)의 GPU 전력 축이 실제 GPU 거동(정상 부하에서 551~600W 도달)과 어긋나 있어, 보정값을 올바르게 다시 잡았다: gpu_pwr_w=700(사실상 비활성) · soc_warn 100 · soc_hard 103. 잘못된 초기 제안(540W)은 정상 GPU 전력을 오작동으로 볼 값이라 폐기됐다(M1). 재보정 후 파라미터에는 `declared(override)` provenance를 각인했다(M2 — 측정값이 아니라 사람 재정임을 표시). 판정은 재보정 후 bf16/fp8 측정이 GPU 전력 트립 0으로 완주한 것이다.

> [원문] testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4)
> - 용량 관측: 1회차 running_max 176 · KV 77% — **용량이 아니라 순수 발열이 벽**
> - tq 커널(TurboQuant)이 bf16/fp8 대비 연산 밀도가 높아 E-core 피닝+쿨다운 완화 하에서도 SoC 지속 트립
> - 3차 런 전체 thermal_trip 누계 240 → 240 (bf16/fp8 구간 신규 트립 0, tq4 구간에서만 2회)

```hint-event
id: W3
kind: wall
증상: GPU 전력 축이 정상 부하를 오작동으로 트립할 수 있는 잘못된 보정
서명: "워치독 파라미터: soc_warn 100 / soc_hard 103 (2026-10-13 사용자 재보정, `declared(override)`), gpu_pwr_w 700 (사실상 비활성)"
원인: 초기 전력 보정이 이 GPU(RTX PRO 6000 · 600W 한계)의 실제 전력 거동(정상 부하 551~600W)과 불일치
해소: gpu_pwr_w=700 · soc_warn 100 · soc_hard 103으로 재보정(사용자 적용) · declared(override) provenance 각인
검증: 재보정 후 bf16/fp8 완주 · GPU 전력 트립 0(testlog_26101317 §열 사망 증거 (tq4))
전이등급: arch-locked
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4)]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: tq4(turboquant_4bit_nc)·tq3(turboquant_3bit_nc) KV로 동시성 측정 — N=200(7K)·N=51(30K)에서 재시도, 쿨러 보강 후 백필에서 tq3 N=239·N=61까지
기각사유: 전부 SoC hard ceiling(101~106°C) 워치독 사살 — 토큰 수용량 2.9~3.5배가 만든 고동시성 연산 부하를 이 호스트 열 예산이 못 받는다(쿨러 보강 후에도 동일 지점 사망)
재개조건: SoC 열 마진이 훨씬 큰 호스트(더 강한 CPU 쿨링 또는 낮은 부하 임계 설계)
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §판정 표 · §백필]
```
```hint-event
id: R2
kind: rejected
시도: nvfp4 KV 캐시 dtype으로 서빙
기각사유: attention selector가 이 아키텍처(sm_120)에서 kv_cache_dtype=nvfp4를 받을 백엔드를 찾지 못해 기동 실패 — run1·run2에서 2회 serve_failed로 확정
재개조건: vLLM 업스트림이 nvfp4 KV를 sm_120 계열에서 지원하는 버전
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §판정 표]
```
**요약** — 이 셀의 형상은 **KV dtype 선택지를 bf16/fp8로 좁힌 세 가지 기각**으로 결정됐다: tq는 열, nvfp4는 아키텍처 — 그리고 JIT 병렬도는 1로. 다시 말해 이 셀의 "정답"은 양자화를 더 하는 것이 아니라 열 예산 안에 드는 dtype(fp8)까지다.

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 GPU 전력 상한 540W 제안 — 실제 GPU 거동을 오독

FlashInfer JIT 사살을 처음 분석할 때 GPU 전력 상한을 540W로 제안했으나, 이는 틀린 진단이었다 — 이 GPU(RTX PRO 6000, 600W 한계)는 정상 벤치 부하에서 551~600W에 도달하므로 540W 상한은 정상 거동을 오작동으로 사살한다. 정정은 W3의 재보정(gpu_pwr_w=700 · 사실상 비활성)이다. 원인 축이 GPU 전력이 아니라 CPU/SoC 온도였음이 뒤의 관측(GPU 79~88W에서도 사살)으로 확정됐다.

```hint-event
id: M1
kind: misdiagnosis
증상: JIT·벤치 구간에 워치독이 서빙 컨테이너를 kill
서명: "gpu_pwr_w 700 (사실상 비활성)"
원인: 초기 분석이 사살 신호를 GPU 전력으로 오독 — 실제는 SoC 온도 축이었고, GPU는 600W 한계 이내 정상
해소: gpu_pwr_w=700으로 재보정(전력 축 사실상 해제)하고 SoC 축(100/103)을 실측 중심으로 유지
검증: testlog_26101317 §열 사망 증거 (tq4) — GPU 79W 저전력에서도 SoC 규칙으로 사살이 기록됨
전이등급: judgment
현재지위: 반증
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4), devlog_26101413_kvq_concurrency_캠페인_종결 §경과·결정 기록]
```

### M2 워치독 재보정 파라미터를 "measured"로 잘못 표기

재보정값(초기 540/100/103)을 열 파라미터 파일에 `measured` provenance로 적었으나, 그 값들은 측정이 아니라 사람이 고른 재정(override)이었다. 정정은 provenance 체계 자체를 고친 것이다 — `blackbox_thermal.py`에 `--basis` 인자를 추가해 기본값을 바꾸는 재emit은 근거 입력을 요구하고 `declared(override)`로 각인하게 했다(자체검사 47건 통과).

```hint-event
id: M2
kind: misdiagnosis
증상: 열 파라미터 파일에 재보정값이 measured 로 기재
서명: "- `.claude/skills/terraforming_node/`: blackbox_thermal provenance/--basis"
원인: 사람 재정값을 측정값처럼 표기 — 출처 표시 규율(결정론 산출물은 출처를 표시한다) 위반
해소: --basis 필수화 + declared(override) 각인 + CLI 거부 게이트
검증: devlog_26101413 §S4 커밋 대상 · testlog_26101317 의 `declared(override)` 표기
전이등급: arch-invariant
현재지위: 반증
출처: [devlog_26101413_kvq_concurrency_캠페인_종결 §S4 커밋 대상, testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4)]
```

> [원문] testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §열 사망 증거 (tq4)
> - 워치독 파라미터: soc_warn 100 / soc_hard 103 (2026-10-13 사용자 재보정, `declared(override)`), gpu_pwr_w 700 (사실상 비활성)

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

**KV 클램프(V4)** — 처음엔 gmu 0.90 추정에 맡기는 안도 검토됐으나, 캠페인 규약(KV_ABSOLUTE_CLAMP_PORTABILITY)대로 bf16 무클��프 부팅의 엔진 실측 `Available KV 76.06 GiB`(553,840 tokens)를 절대 바이트(81,668,803,133)로 고정했다. dtype을 fp8로 바꾸면 같은 바이트에 토큰이 정확히 2배(1,107,696)로 들어간다 — 필요량 대비 배수 개념이 아니라 "같은 예산에 몇 토큰이 들어가는가"가 dtype의 변수다.

**gmu 0.90(V1) · max-num-seqs 512(V3)** — 둘 다 스윕 없이 정한 값이다. gmu는 캠페인 선언 상수, max-num-seqs는 동시성 상한이 배치를 막지 않게 한 손레버다. 이 값들은 최적이 아니라 통제변인이다.

**no-enable-prefix-caching(V5)** — dtype 간 비교에서 prefix cache 효과를 배제하기 위한 인터뷰 확정 통제변인. 실서빙에서는 켜는 편이 유리할 수 있으나 이 측정의 N*는 꺼진 상태의 값이다.

> [원문] testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §판정 표
> | bf16(auto) | **69** ✅ | **17** ✅ | kv_tokens 553,840 |
> | fp8 | **138** ✅ | **35** ✅ | kv_tokens 1,107,696 (bf16의 정확히 2배) |

**버티는 기전 대 실패 기전** — fp8 형상: KV 바이트 동일 · dtype만 fp8 → 토큰 2배 · 동시성 2배 · 열 예산 안 — **확정**(testlog_26101317 §판정 표). tq 형상: KV 바이트 동일 · dtype turboquant → 토큰 2.9~3.5배 → 동시성 후보 N=200~239 → SoC hard ceiling 사살 — **확정**(§열 사망 증거 · §백필). nvfp4: dtype 변경만으로 attention backend 해소 불가 — **확정**(§판정 표).

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- JIT/autotune을 병렬(MAX_JOBS>1)로 돌리지 말 것 — W2 — MAX_JOBS=1 고정, 캐시는 호스트에 영속화
- 측정 연속 런을 쿨다운 없이 돌리지 말 것 — W2 — 런 사이 CPU ≤55°C 게이트
- GPU 전력 상한을 부하 실측 없이 낮게 잡지 말 것 — M1 — 정상 부하의 전력 범위를 먼저 관측하고 상한을 정한다
- 사람이 고른 재정값을 measured로 표기하지 말 것 — M2 — declared(override) + --basis 근거
- tq 계열 KV를 이 호스트에서 동시성 측정하지 말 것 — R1 — 열 마진이 큰 호스트에서만 시도
- fp8 기동 실패를 모델 문제로 오진하지 말 것 — W2 — 엔진 로그 끝이 torch.compile/autotune 구간이면 열 문제다
- KV 클램프를 다른 환경 값으로 승계하지 말 것 — V4 — 무클��프 부팅의 엔진 실측으로 매번 재도출
- prefix cache를 측정마다 다르게 두지 말 것 — V5 — 비교 측정은 통제변인을 고정하고 시작한다
- N*+1 포화 증거 없이 N*를 선언하지 말 것 — testlog_26101317 §측정 정의 — 최대 N의 반복 3회 + 그 위의 포화를 함께 둔다

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: tq 계열 KV는 어느 정도의 열 마진에서 측정 가능한가 — 이 호스트의 105°C crit보다 여유 있는 호스트에서 N*가 얼마나 나오는가
현재상태: 이 호스트에서는 tq4 N=200·tq3 N=239(4B)에서 SoC hard 사살 — 쿨러 보강 후에도 동일. 동시성 상한이 열에 의해 미측정
다음관측: 더 강한 CPU 쿨링의 호스트에서 tq4 7K 프로브 1회
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §백필]
```
```hint-event
id: Q2
kind: open-question
물음: nvfp4_4over6 등 nvfp4 변종이 sm_120에서 동작하는가
현재상태: 기본 nvfp4는 attention selector 거부로 확정. 변종은 미시도
다음관측: 업스트림 릴리스 노트의 nvfp4 sm_120 지원 여부 확인 후 변종 기동 1회
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §판정 표]
```
```hint-event
id: Q3
kind: open-question
물음: 벽시계 예산(셀당)의 실측 교정치는 얼마가 적정한가 — 이번 캠페인의 측정 시간이 계획 예산을 크게 초과했다
현재상태: bf16 셀 66분 · fp8 7K 86분 · fp8 30K 82분 관측(열 쿨다운 포함) — 미교정
다음관측: 다음 캠페인 계획에 이 관측치를 대입
출처: [testlog_26101317_qwen3-4b_KV양자화_동시성_판정 §후속]
```

해설 — 셋 다 이 셀의 서빙을 막는 물음이 아니라 다음 환경·계획의 입력이다. Q1·Q2는 형상 선택지를 다시 열 수 있는 조건이고, Q3는 측정 비용 추정의 문제다.
