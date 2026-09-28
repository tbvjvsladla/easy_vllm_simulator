# 02 · 계보 서사 — 이 셀의 형상에 도달하기까지

> 사람은 산문을, Agent 는 ` ```hint-event ` 블록을 읽는다(같은 사실의 두 표면). `kind: wall` 블록만 위에서부터 읽으면 넘은 벽의 순서가 나온다.
> `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 글자 그대로다 — 해설은 인용 밖에 있다.

<!-- FACT:lineage -->
계보 문서 12건 — 서사는 **이 목록 전체**를 읽고 쓴다(이 셀 1회분이 아니다). 순서 = 계층 → 깊이 → 관련도 → 날짜. 크기(바이트)는 읽기 예산용이다. 원시 증거 후보(엔진 · 빌드 로그)는 `LINEAGE.json` 의 `evidence_candidates` 다.

| # | 문서 | 종류 | 날짜 | 깊이 | 크기 | 셀 축 토큰 | 대체됨 |
|---|---|---|---|---|---|---|---|
| 1 | `docs/testlog/testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정.md` | testlog | 26092808 | 0 | 4787 | 0.29.0rc6 · 1m · ds4f0731 · ds4f0731-1m-spec7-roce · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce · spec7 | — |
| 2 | `docs/benchmark/sweep_map_26092809_ds4f0731_k70_roce_1m.md` | sweep_map | 26092809 | 0 | 3412 | 0.29.0rc6 · 1m · ds4f0731 · ds4f0731-1m-spec7-roce · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce · spec7 | — |
| 3 | `docs/testlog/testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정.md` | testlog | 26090912 | 0 | 4839 | 0.29.0rc6 · 1m · ds4f0731 · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce · spec7 | docs/testlog/testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정.md |
| 4 | `docs/plan/plan_26092808_커널7_0_DS4F0731_멀티캠페인_RoCE재현_1M서빙_풀벤치_hint.md` | plan | 26092808 | 0 | 4754 | 0.29.0rc6 · 1m · ds4f0731 · ds4f0731-1m-spec7-roce · roce · spec7 | — |
| 5 | `docs/testlog/testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정.md` | testlog | 26090904 | 0 | 5165 | 0.29.0rc6 · 1m · ds4f0731 · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce | — |
| 6 | `docs/testlog/testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증.md` | testlog | 26092807 | 0 | 4568 | 0.29.0rc6 · 1m · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce · spec7 | — |
| 7 | `docs/devlog/devlog_26092809_커널7_0_DS4F0731_RoCE_멀티캠페인_서사.md` | devlog | 26092809 | 0 | 4591 | 1m · ds4f0731 · ds4f0731-1m-spec7-roce · roce · spec7 | — |
| 8 | `docs/benchmark/bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0.md` | bench_report | 26092809 | 0 | 6584 | 0.29.0rc6 · easy-vllm:0.29.0rc6-cu133-aarch64-source · roce | — |
| 9 | `docs/plan/plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes.md` | plan | 26090819 | 1 | 5710 | 0.29.0rc6 · 1m · ds4f0731 · roce | — |
| 10 | `docs/testlog/testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정.md` | testlog | 26091412 | 1 | 31725 | 0.29.0rc6 · 1m · ds4f0731 · spec7 | — |
| 11 | `docs/benchmark/sweep_map_26090912_e1m_levers.md` | sweep_map | 26090912 | 1 | 4263 | 0.29.0rc6 · 1m · easy-vllm:0.29.0rc6-cu133-aarch64-source | — |
| 12 | `docs/benchmark/sweep_map_26090912_b768k_levers.md` | sweep_map | 26090912 | 1 | 8808 | 0.29.0rc6 · easy-vllm:0.29.0rc6-cu133-aarch64-source | — |
<!-- /FACT:lineage -->

## 2.1 출발점
> 이 절이 답하는 질문: 이 계보는 무엇을 목표로, 어떤 조건에서 출발했는가 — 첫 plan 은 무엇을 가정했고, 사람은 무엇을 결정했는가?

**목표.** 이 계보는 2026-09-08 에 DeepSeek-V4-Flash-0731 을 2×GB10 멀티노드(Ray TP=2 · RoCE)에서 최신 pre-release vLLM 0.29.0rc6 으로 서빙하고, 컨텍스트 {768K, 1M} × KV dtype {auto, fp8, turboquant_4bit_nc} 6셀을 광의탐색해 **Hermes Agent 용처의 최대 TPS 레시피**를 찾는 것으로 출발했다(plan_26090819 §1). 이 셀(`ds4f0731-1m-spec7-roce` · 2026-09-28)은 그 탐색이 1M 에서 고른 레시피(`e-1m-kvfp8-e1combo`)를 **값 변경 0** 으로 다시 띄운 통제된 재현이다 — 목적은 양 노드 커널을 `7.0.0-1019-nvidia` 로 올린 뒤 NVIDIA 포럼 383023 이 보고한 NCCL RoCE `ibv_reg_mr_iova2` ENOMEM 회귀가 이 클러스터 · 이 레시피에서 실제로 나는지 가르는 것이었다(plan_26092808 §1).

> [원문] plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §1. 목표
> 2×GB10 멀티(Ray TP=2 · RoCE) 패브릭에서 DeepSeek-V4-Flash-0731을 **최신 vLLM 0.29.0rc6**(pre-release 포함 최신, 2026-09-08 실측 태그)로 서빙하고,
> **context {768K, 1M} × KV cache dtype {무양자화(auto), fp8, turboquant_4bit_nc} = 6셀** 각각에 대해
> 광의의 탐색으로 **최대 TPS 레시피**를 찾는다. 동시성은 잔여 KV 캐시 최대활용으로 결정론 산출한다.

**HW 출발 조건.** NVIDIA GB10(sm_121a) · 노드당 GPU 1개 · 2노드 · 통합메모리(호스트와 GPU 가 한 풀) · 이 셀의 예산 선언이 기록한 노드 메모리 총량은 `mem_total_mib=124608`(01 §1.4 이벤트 표 — 선언 입력으로 쓴 관측값) · 인터커넥트는 RoCE v2(plan_26090819 §2 "RoCE 98.41Gb/s/포트 검증済") · 분산은 Ray executor TP=2. 호스트 층: driver 580.173.02(인증서 `driver_version` · testlog_26092808 유효맥락) · 커널은 계보 전반부(09-08~09-14)가 6.17 계열, 이 셀은 `7.0.0-1019-nvidia`(양 노드 · testlog_26092807 §1 — 2026-09-15 설치). 이미지 층(NGC 26.07 · torch 2.13.0a0 · CUDA 13.3.1)은 00 §0.4 표가 정본이다.

**모델 구조.** 체크포인트 `deepseek-ai/DeepSeek-V4-Flash-0731`(revision 은 00 §0.2 표) · 아키텍처 `DeepseekV4ForCausalLM` · 네이티브 컨텍스트 1,048,576(config `max_position_embeddings` — plan_26090819 §2 "둘 다 native 1M(YaRN×16) 이내"). 양자화: config `quantization_config` 는 fp8 블록(128×128 · ue8m0 스케일)이고 `expert_dtype` 은 fp4 다 — 엔진 로그도 dense 에 `DeepGemmFp8BlockScaledMMKernel`, MoE 에 `Using 'HUMMING' Mxfp4 MoE backend` 를 골랐다(lite_engine 로그 L272 · L277). plan_26090819 §2 는 이것을 "FP8 block MoE" 로 적었다(→ 02 §2.4 M4). 크기: plan 은 "가중치 155.4 GiB" 라 적었다. 이 셀의 예산 선언은 `weights_mib=79577`(01 §1.4 이벤트 표 — 선언값)이고, 그 원천은 로드-전 게이트가 **index `weight_map` 이 가리키는 샤드 파일 크기의 합**을 TP 로 나눈 값이다(`parse_model_config._native_weight_bytes` — `du` 는 `.git/lfs` 복제본까지 세어 두 배가 된다는 것이 그 함수 주석의 사유다 · 이 셀 발행 세션에서 체크포인트 디렉터리 `du --apparent-size` 는 311G 로 나왔다). 엔진은 rank 당 `Model loading took 79.04 GiB`(lite_engine 로그 L578 · L586 — target 과 dspark draft 두 번의 적재 뒤 보고한 측정값). MTP 층 1개(`num_nextn_predict_layers: 1`)가 있고, spec 은 별도 draft(dspark)로 켠다. PLE 가중치는 없다(00 §0.2 축 `ple`).

**stock 에서 예상 · 관측된 장벽.** (1) 0.25.1 시대 arch-wall(flashinfer `decode_dsv4` page_block64) — 0.29.0rc6 stock 에서 해소됐다(testlog_26090904 판정 요약) · (2) flashinfer 핀 충돌로 소스빌드 실패(W1) · (3) KV dtype 이 fp8(fp8_ds_mla) 하나로만 성립(W5) · (4) 멀티 트리플렛 배선 결함 · 예산 파라미터 부재 · env 인라인 주석(W2~W4) · (5) 768K/1M 기동 밸리가 호스트 절대 플로어를 침범(W7) · (6) 09-17 커널 7.0 위에서 RoCE 경로가 `ibv_reg_mr_iova2` 로 실패해 Socket 기준선으로 후퇴(W11) — 이 셀의 출발점이다.

**선례와 사람의 결정.** 계보가 인용하는 선례는 0.25.1 시대의 arch-wall(flashinfer `decode_dsv4` page_block64 · plan_26090819 §3)과 MoE auto → MARLIN repack OOM 전력(트리플렛 yaml L16 주석의 `hint/0.25.1`)이다 — 이 저장소의 DS4F hint 태그는 0.29.0rc6 계열 6건뿐이고 0.25.1 · 0.26.x DS4F hint 태그는 없다. plan_26090819 §3 은 "0.29.0rc6 stock 에서 해소됐는지는 스모크가 유일한 중재자" 라 적고 벽이 재현되면 변종 사다리(deps → 소스-게이트 → 자체 이식 → 포크 핀)를 쓰도록 사람이 범위를 승인했다. 스모크가 stock 으로 통과해 사다리는 쓰지 않았다(R7). 이 셀에서 사람은 (a) 커널 다운그레이드 전에 회귀를 먼저 재현해 보기로 했고(plan_26092808 §1), (b) 레시피는 09-09 태그 값 그대로 두고 NCCL 전송만 RoCE 로 바꾸기로 했으며, (c) plan 이 계획한 생성물 일시 수정(plan_26092808 §2 NCCL 행 — '렌더러 불변 무수정')은 서브 배달이 재렌더라 닿지 않아, plan 이후의 사용자 결정으로 전송을 렌더러의 정식 선택(`manifest.interconnect.nccl_transport`)으로 바꿨다(devlog_26092809 §무엇을 했나 3 · 커밋 2e91a1e 메시지).

## 2.2 벽과 해소
> 아래는 블랙박스 원장에서 기계가 묶은 이 셀의 기동 시도다 — 서사에 "몇 번 띄웠다" 를 적기 전에 대조한다(행 전문은 01 §1.4).

<!-- FACT:event_attempts -->
**이 셀의 기동 시도**(블랙박스 원장 · 01 §1.4 행 전문)

| 시도 | 노드 | label | 선언(UTC) | 갱신(renew) | 사살 · 트립 · 거부 | 닫힘 | 그 밖 행 |
|---|---|---|---|---|---|---|---|
| 1 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:32:54Z | 없음 | 없음 | `budget_clear` 2026-09-27T23:34:58Z | 3 |
| 2 | main | `smoke-ds4f0731-1m-spec7-roce` | 2026-09-27T23:37:13Z | 17회 · 첫 2026-09-27T23:54:55Z | 없음 | —(닫힘 행 없음) | 10 |
<!-- /FACT:event_attempts -->

> 이 절이 답하는 질문: 이 셀의 형상에 도달하기까지 어떤 벽을, 어떤 순서로, 무엇으로 넘었는가? (계보 전체 — 이 셀 1회분이 아니다)

벽은 넘은 순서(계보 날짜순)로 적는다. W1~W10 은 09-08~09-09 광의탐색 캠페인(`camp-26090819`)이 이 레시피 형상에 도달하며 넘은 벽이고, W11~W13 은 커널 7.0 위에서 이 셀이 넘은 벽이다. 이 셀의 기동 시도는 위 표대로 **2회**다(시도 1 은 W12 로 로드 도중 회수 · 시도 2 가 판정 대상).

### W1 소스빌드 의존 해소 실패 — flashinfer 핀 충돌
1차 소스빌드가 양 노드에서 똑같이 `ResolutionImpossible` 로 멈췄다. 대상 vLLM 이 선언한 flashinfer 핀과 이미지 constraint(0.28.0 wheel 기준선)의 핀이 달랐다. `==` 핀 전수비교에서 유일한 충돌임을 먼저 보이고 constraint 를 상류 선언에 양보했다 — 양 노드 동일 실패였으므로 노드 환경 차이가 아니라고 판정했다. 부작용: 실린 `requirements.txt` 머리 주석이 그 양보를 기록한다(단 이 바이트가 이미지에 쓰인 것인지는 01 §1.1 이 '관측 아니오' 로 남겼다).

> [원문] testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §1.
> - 1차 빌드: 양노드 동일 `ResolutionImpossible` — vLLM 0.29.0rc6 선언 `flashinfer-python==0.6.18` vs constraint `==0.6.16.post3`(0.28.0 wheel baseline). **서브 동일 실패 = 환경 불일치 아님** 확정.

```hint-event
id: W1
kind: wall
증상: 1차 소스빌드가 양 노드에서 의존 해소 단계에서 실패했다
서명: "1차 빌드: 양노드 동일 `ResolutionImpossible`"
원인: vLLM 0.29.0rc6 선언 flashinfer-python==0.6.18 과 constraint 의 ==0.6.16.post3 충돌(== 핀 전수비교의 유일 충돌)
해소: constraint 를 상류 선언(0.6.18)에 양보 — requirements.txt 머리 주석 · 2차 빌드 양 노드 PASS
검증: testlog_26090904 §1 2차 빌드 PASS · 프로브 flashinfer 0.6.18
전이등급: arch-invariant
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §1., requirements.txt]
```

### W2 멀티 트리플렛 배선 공백 — 컨테이너 이름 쌍 · TP · ray backend 미emit
첫 멀티 스모크가 `MASTER_CONTAINER_NAME` 미설정으로, 이어서 `distributed-executor-backend` 누락으로 멈췄다. 레시피 생성기가 멀티용 master/slave 컨테이너 이름 쌍과 `tensor-parallel-size`·`distributed-executor-backend` 를 내지 않는 같은 공백의 두 변종이었다(출처가 둘을 한 공백으로 묶었다). 해소는 관례 이름 보수와 두 노브의 명시다 — 트리플렛 yaml 의 두 줄 주석이 그 흔적이다.

```hint-event
id: W2
kind: wall
증상: 멀티 스모크가 기동 전에 rc=2 로 두 번 멈췄다(컨테이너 이름 · executor backend)
서명: "rc=2 `MASTER_CONTAINER_NAME 미설정`"
원인: gen_recipe_set 이 multi 트리플렛(master/slave 컨테이너명 쌍 · TP · ray backend)을 emit 하지 않는 배선 공백
해소: 관례 <cell>-master/slave-container 보수 · yaml 에 tensor-parallel-size 2 + distributed-executor-backend ray 명시
검증: testlog_26090904 §4 스모크 PASS(rc=0)
전이등급: judgment
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2., ds4f0731-1m-spec7-roce.yaml]
```

### W3 예산 파라미터 부재 — 단일노드 트라이얼 불가 모델
DS4F(155 GiB)는 단일노드에서 KV 클램프를 선측정할 수 없어 스모크가 예산 선언 단계에서 막혔다. 게이트가 제 일을 한 정상 차단이며, 하네스 hint 의 실측값을 **시드**로 선언하고 멀티에서 직접 수렴시키는 경로로 넘었다(시드 10 GiB 는 이후 W7 에서 다시 정해졌다).

```hint-event
id: W3
kind: wall
증상: 멀티 스모크가 예산 선언 단계에서 rc=4 로 로드 전에 멈췄다
서명: "rc=4 `budget_params_missing`"
원인: 155 GiB 체크포인트는 단일노드 trial 로 KV 클램프를 선측정할 수 없어 예산 입력이 없었다
해소: hint 실측값을 시드로 선언하고 멀티 실측으로 재산정하는 경로 확정
검증: testlog_26090904 §4 예산 선언→honored→up 순서 ✓
전이등급: judgment
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2.]
```

### W4 master 즉사 — env 값 뒤 인라인 주석
env 파일의 값 뒤에 붙은 인라인 주석을 로더가 값의 일부로 읽어 master 가 즉사했다. 주석을 행 단위로 분리해 넘었다. 이 셀의 `.env.<cell>` 이 주석을 전부 별도 줄에 두는 이유다.

```hint-event
id: W4
kind: wall
증상: master 컨테이너가 기동 직후 즉사했다
서명: "을 val()가 값으로 읽음(자기사례 — 주석 3곳)"
원인: env 값 뒤 인라인 주석이 값으로 파싱됐다
해소: 주석 전행 분리 · 검증 추가
검증: testlog_26090904 §4 스모크 PASS
전이등급: arch-invariant
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §2.]
```

### W5 KV dtype — fp8_ds_mla 외 거부
KV dtype 축 3군 중 auto 트라이얼이 엔진 assert 로 죽었다. 소스 판독 결과 sm_121a 에서 선택되는 SM120 FlashInfer 어텐션이 fp8_ds_mla 페이지 레이아웃을 강제하고 KV dtype 이 fp8 로 시작하는지 assert 한다 — 배선이 아니라 커널 페이지 포맷 제약이다. 사람 승인으로 KV 축을 fp8 하나로 개정했고 4셀을 void 로 기록했다. 이 셀 엔진 로그에도 같은 경로가 `Using DeepSeek's fp8_ds_mla KV cache format.`(L275)로 찍힌다.

> [원문] testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3.
> - 트라이얼-1c(kv auto) 엔진 로그: `AssertionError: DeepseekV4 fp8_ds_mla layout only supports fp8 kv-cache, got auto` (`vllm/models/deepseek_v4/attention.py:106`)

```hint-event
id: W5
kind: wall
증상: kv-cache-dtype auto 트라이얼에서 엔진이 기동 중 assert 로 죽었다
서명: "AssertionError: DeepseekV4 fp8_ds_mla layout only supports fp8 kv-cache, got auto"
원인: sm_121a → DeepseekV4FlashInferSM120Attention → use_fp8_ds_mla_layout=True 강제 → kv_cache_dtype fp8 assert(커널 페이지 포맷 제약)
해소: kv-cache-dtype fp8 고정 · layer-1 개정 kv_dtype_axis=["fp8"](사람 승인) · auto/turboquant 셀 void
검증: testlog_26090904 §3 · 이 셀 lite_engine 로그 L275 fp8_ds_mla 선택
전이등급: arch-locked
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3., lite_engine_ds4f0731-1m-spec7-roce.log]
```

### W6 SoC 열 임계 미교정 거부
광의탐색 측정 진입이 SoC 열 파라미터가 교정되지 않았다는 이유로 rc=6 에서 멈췄다. 교정(warn 97 · hard 99)과 양 노드 배포 뒤 진입했다 — 측정 인프라의 벽이지 모델의 벽이 아니다.

```hint-event
id: W6
kind: wall
증상: 벤치 셀 진입이 rc=6 으로 거부됐다
서명: "SoC 열 임계 미교정 거부(rc=6)"
원인: 블랙박스 열 워치독의 SoC 임계가 미교정(외부 보고 역산값) 상태였다
해소: 교정(warn 97·hard 99 · 커밋 f02990c) · 양 노드 배포
검증: testlog_26090912 운영 사건 표
전이등급: arch-locked
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정]
```

### W7 기동 밸리 — memwatch 킬 2회 · KV 클램프 18→14→10 GiB
768K 셀의 KV 클램프는 18 → 14 → 10 GiB 로 수렴했고 그 사이 memwatch 사살은 **3회**였다. 18 GiB(1차) 사살 1회는 벤치 상주분(4 GiB)을 예산에 넣지 않은 탓이었고, **기동 밸리 사살 2회는 둘 다 14 GiB(2차 · L1 서빙)**에서 났다(`docs/devlog/devlog_26090912_ds4f0731_광의탐색_셀루프_서사.md` §수렴의 3단계 · 계보 밖 · 트리플렛 yaml L13 도 '14GiB 시 밸리 ~8.2GiB … memwatch 킬 2회' 로 14 GiB 에 귀속한다). 10 GiB 시도가 같은 밸리를 생존한 대조로 바인딩이 KV 용량이 아니라 호스트 기동 밸리 + 절대 플로어임이 정해졌다. 해소는 클램프 10 GiB 수렴과 벤치 전 캐시 드롭 배선이다. **기전의 지위**: '밸리 = 가중치 로드의 페이지캐시 적층'이라는 기전 서술은 계보 밖 devlog_26090912 에만 있고 인용한 yaml 주석 · 운영 사건 표에는 없다 — 관측은 사살 · 생존 대조이고 기전 분해는 미결이다. 이 셀은 10 GiB 에서 사살 0 으로 그 형상을 다시 버텼다(01 §1.4 기동 시도 표).

> [원문] testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정
> | memwatch 킬 2회(기동 밸리) | 3차 수렴: 클램프 18→14→10GiB(binding=밸리) · 캐시 드롭 배선 |

```hint-event
id: W7
kind: wall
증상: 768K 셀 클램프 14 GiB 에서 기동 직후 memwatch 가 두 번 컨테이너를 사살했다(18 GiB 사살 1회는 벤치 상주분 미산입으로 별개)
서명: "memwatch 킬 2회(기동 밸리)"
원인: 호스트 기동 밸리(가용 급락)가 KV 몫과 겹쳐 가용이 절대 플로어 아래로 내려갔다 — 10 GiB 생존 대조로 확정 · 밸리의 구성(페이지캐시 적층)은 계보 밖 devlog 서술로 미결
해소: kv-cache-memory-bytes 10 GiB 수렴 · 벤치 전 페이지캐시 드롭 배선
검증: testlog_26090912 운영 사건 표 · 이 셀 기동 시도 2 사살 없음(01 §1.4)
전이등급: arch-scaled
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정, ds4f0731-1m-spec7-roce.yaml]
```

### W8 moe-backend triton — 엔진 거부
768K 레버 스윕의 `L3-moe`(moe-backend=triton)가 서빙에 도달하지 못했다. 엔진이 TRITON(mxfp4 커널)은 이 모델의 SILU 활성을 지원하지 않는다고 거부했다. humming 을 유지해 넘었다(이 셀도 `Using 'HUMMING' Mxfp4 MoE backend.` · L277).

```hint-event
id: W8
kind: wall
증상: moe-backend triton 셀이 서빙 전에 실패했다(rc=3 · serve_failed)
서명: "TRITON(mxfp4 커널)은 MoEActivation.SILU 미지원"
원인: TRITON MXFP4 MoE 커널이 이 모델의 SILU 활성을 지원하지 않는다(엔진 거부)
해소: moe-backend humming 유지
검증: sweep_map_26090912_b768k_levers §셀 L4-combo measured(humming)
전이등급: arch-locked
출처: [sweep_map_26090912_b768k_levers §셀 (실행 순서)]
```

### W9 guidellm 이 deepseek_v4 토크나이저를 파싱하지 못함
GuideLLM 레그가 deepseek_v4 를 파싱하지 못해 full 레그가 서지 못했다(운영 사건 표). 조치는 `run_bench.sh` 의 토크나이저 스테이징(tokstage) 배선 수정이다 — 무엇이 파싱을 막았는지의 기전은 출처에 없다(아래 원인 칸은 추론). 이 셀의 측정도 그 배선을 탔다(01 §1.4 · 03 §3.4 `--tokenizer "kind=huggingface_auto,model=/tok"`).

```hint-event
id: W9
kind: wall
증상: GuideLLM 레그가 이 모델 토크나이저를 파싱하지 못했다
서명: "guidellm deepseek_v4 파싱 불가"
원인: 측정 배선 결함(출처는 "tokstage 배선 수정" 만 적는다 — 토크나이저 원천에 닿지 못했다는 기전은 추론)
해소: run_bench.sh 의 토크나이저 스테이징(tokstage) 배선 수정
검증: testlog_26090912 운영 사건 표 · 이 셀 full 스윕 완주(03 §3.1)
전이등급: arch-invariant
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정, run_bench.sh@434fa6740831]
```

### W10 1M 기본 셀 조기 발사 · 갱신 루프 잔존 교착
1M 기본 셀(`E0-base`) 1차 측정이 기동 완료 전에 발사돼 serve_failed 로 기록됐고 재측정했다. 또 `--keep-up` 뒤 남은 예산 갱신 루프가 다음 선언과 교착해 정규 `--down` 회수 절차가 확립됐다. 둘 다 운영 절차의 벽이며, 이 셀이 `--down` 으로 시도 1 을 회수한 절차가 그 결과다.

```hint-event
id: W10
kind: wall
증상: 1M 기본 셀 측정이 기동 전 발사돼 serve_failed · 상주 뒤 갱신 루프가 다음 선언과 교착
서명: "E0 조기 발사(serve_failed 1회)"
원인: 측정 진입이 서빙 준비 판정보다 앞섰다 · 상주 회수 경로가 루프를 거두지 않았다
해소: 재측정 · 정규 --down 회수 절차(컨테이너 · 워치독 · 캐시 · 예산 선언을 함께 회수)
검증: sweep_map_26090912_e1m_levers E0-base measured(재측정) · testlog_26090912 운영 사건 표
전이등급: judgment
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정, sweep_map_26090912_e1m_levers §셀 (실행 순서)]
```

### W11 커널 7.0 위 RoCE 경로 실패 → Socket 기준선 후퇴(09-17)
2026-09-15 양 노드가 커널 `7.0.0-1019-nvidia` 로 올라간 뒤, 09-17 멀티 캠페인에서 TP=2 의 RoCE/verbs 경로가 `ibv_reg_mr_iova2` 메모리 등록 실패로 죽었다. GDR 세부 노브 OFAT 로도 막히지 않아 렌더러가 `NCCL_IB_DISABLE=1` · `NCCL_NET=Socket` 을 불변으로 박았다(커밋 `d8c7b78` · `782fd70`). 이 셀은 그 불변 때문에 RoCE 로 돌 정식 통로가 없어서, 전송을 manifest 에서 고르게 하는 렌더러 교정으로 넘었다. **원인의 지위**: 09-17 의 원시 엔진 로그는 남지 않아 원인은 확정되지 않았다(testlog_26092807 §3 · Q1). 이 셀은 같은 커널에서 RoCE 로 서빙에 성공했다(W13).

> [원문] testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증 §1.
> 타임라인: `dpkg.log` 2026-09-15 06:24 KST `linux-image-7.0.0-1019-nvidia` 설치 → 06:28 부팅 → 2026-09-17
> `testlog_26091721` 에서 RoCE 경로 `ibv_reg_mr_iova2` 실패로 Socket 기준선 후퇴(렌더러 `782fd70`·`fe1bc42`, 현재도 `NCCL_NET=Socket` 불변).

```hint-event
id: W11
kind: wall
증상: 커널 7.0 위 멀티 TP=2 가 RoCE 경로에서 실패해 Socket 기준선으로 후퇴했다(09-17)
서명: "RoCE 경로 `ibv_reg_mr_iova2` 실패로 Socket 기준선 후퇴"
원인: 미확정 — 09-17 원시 엔진 로그 부재 · 포럼 383023 은 CMA=0(CONFIG_CMA_SIZE_MBYTES 128→0)을 지목(이 클러스터에 조건 실재)
해소: 렌더러 전송 선택 manifest.interconnect.nccl_transport(socket 기본 · rdma) 도입(2e91a1e → 8319b4f) · 서브는 sync_to_sub 재렌더 배달
검증: testlog_26092808 §2 Using network IB · iova2 0건
전이등급: judgment
출처: [testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증 §1., testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §2.]
```

### W12 시도 1 — rdma 전송이 조용히 Socket 으로 떨어짐
전송 교정의 첫 정의(`2e91a1e`)는 rdma 를 "`NCCL_IB_DISABLE=1` 유지 + `NCCL_NET` 미방출" 로 두었다. 기동 시도 1 에서 양 노드 엔진이 `Using network Socket` 을 골랐다 — `NCCL_IB_DISABLE=1` 이 내장 IB 를 끄고, 남은 외부 플러그인(이미지 `NCCL_NET_PLUGIN=spcx`)은 이 장치를 거부해 건너뛰므로 Socket 만 남는다(추론 — 시도 1 의 로그 사본은 싣지 않았고 testlog 의 관측 요약만 있다). 로드 도중 정식 `--down` 으로 회수했고(01 §1.4 시도 1 `budget_clear`), rdma 를 `NCCL_IB_DISABLE=0` · `NCCL_NET=IB`(NCCL 내장 verbs)로 재정의했다(`8319b4f`). 시도 2 에서도 SPCX 스킵 경고는 그대로 찍힌다 — 무해하며 벽이 아니다.

> [원문] lite_engine_ds4f0731-1m-spec7-roce.log §L311-311
> (EngineCore pid=1271) (RayWorkerProc pid=2472) 38:29] <node:main>:2472:2472 [0] init.cc:449 NCCL WARN Spectrum-X (SPCX) NCCL plugin is not supported on device:<nic:cluster>, skipping .. [repeated 3x across cluster]

```hint-event
id: W12
kind: wall
증상: rdma 로 선언한 기동 시도 1 이 양 노드에서 Socket 전송으로 떨어졌다
서명: "Spectrum-X (SPCX) NCCL plugin is not supported on device"
원인: IB_DISABLE=1 이 내장 IB 를 끄고 SPCX 플러그인이 장치를 건너뛰어 Socket 만 남았다(추론 · testlog_26092808 §1 관측 요약)
해소: rdma = NCCL_IB_DISABLE=0 · NCCL_NET=IB 명시(8319b4f) · 시도 1 은 --down 정식 회수
검증: lite_engine_ds4f0731-1m-spec7-roce.log L322 Using network IB
전이등급: judgment
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §1., lite_engine_ds4f0731-1m-spec7-roce.log]
```

### W13 시도 2 — 커널 7.0 · RoCE 로 1M 분산 서빙 성립 · 벤치 r1 무효
시도 2 는 양 노드에서 `Using network IB`(L322)로 통신기를 연결했고, 포럼이 실패를 보고한 구간(가중치 적재 → 프로파일링 · KV 할당)을 `ibv_reg_mr_iova2`·ENOMEM·NV_ERR 0건으로 지났다(`GPU KV cache size: 1,941,478 tokens` · L628). 선언부터 API 서버 기동까지 17분 16초(01 §1.4 행 16)였고 스모크 PASS 다. 이어 full 벤치 1차 스윕(r1)은 내가 `--backend` 를 빠뜨려 부하 전에 measurement_void 로 끝났고, 같은 선언으로 재초기화한 r2 가 측정이다 — r1 은 측정 사실이 아니다.

> [원문] testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §2.
> | `Using network IB` / `Using network Socket` | 2 / 0 |
> | `iova2` · `Cannot allocate memory` · `NV_ERR` | 0 · 0 · 0 |

```hint-event
id: W13
kind: wall
증상: 벤치 r1 스윕이 부하 전에 measurement_void 로 끝났다
서명: "호출자 --backend 누락으로 부하 전 measurement_void"
원인: sweep_bench 가 --backend(요청 포맷 · 기본값 없음)를 요구하는데 호출자가 넘기지 않았다
해소: --backend openai-chat(09-09 스윕이 쓴 기본과 같은 포맷)으로 r2 재초기화 · 측정
검증: sweep_map_26092809_ds4f0731_k70_roce_1m 셀 measured · 03 §3.1
전이등급: arch-invariant
출처: [sweep_map_26092809_ds4f0731_k70_roce_1m §셀 (실행 순서), devlog_26092809_커널7_0_DS4F0731_RoCE_멀티캠페인_서사 §무엇을 했나]
```

## 2.3 기각된 시도 · 반증된 축
> 이 절이 답하는 질문: 무엇을 시도했다가 버렸고, 어떤 축이 반증됐는가?

```hint-event
id: R1
kind: rejected
시도: kv-cache-dtype auto(무양자화)로 트라이얼-1c
기각사유: 엔진이 기동 중 assert 로 거부했다(fp8_ds_mla 레이아웃은 fp8 KV 만) — 해당 셀 void
재개조건: 상위 vLLM 이 DeepseekV4 SM120 어텐션에 fp8 외 KV 페이지 포맷을 추가하면(attention.py 의 assert 가 사라지면)
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3.]
```

```hint-event
id: R2
kind: rejected
시도: kv-cache-dtype turboquant_4bit_nc
기각사유: 같은 assert 경로로 구조적 거부 — turboquant 는 dense/GQA 전용이고 MLA sparse indexer KV 에 상류 정의가 없다(출처 판정) · 셀 void
재개조건: 상류가 MLA sparse KV 용 turboquant 정의를 추가하면
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §3.]
```

```hint-event
id: R3
kind: rejected
시도: moe-backend triton(768K 레버 스윕 L3-moe)
기각사유: 엔진이 TRITON(mxfp4 커널)의 SILU 미지원으로 거부 — 서빙 미성립(rc=3)
재개조건: TRITON MXFP4 MoE 커널이 SILU 활성을 지원하면
출처: [sweep_map_26090912_b768k_levers §셀 (실행 순서)]
```

```hint-event
id: R4
kind: rejected
시도: moe-backend auto
기각사유: auto 가 MARLIN 으로 가서 expert 를 repack 하며 통합메모리 OOM 을 낸 전력(0.25.1 계보 · yaml 주석 "재검증 대상") — 이 계보에서 재시도 0 회(승계된 기각)
재개조건: 이 모델 · 버전에서 auto 가 MARLIN 이 아닌 경로를 고르는지 엔진 로그로 먼저 확인한 뒤
출처: [ds4f0731-1m-spec7-roce.yaml, 20-triton-kernels.sh]
```

```hint-event
id: R5
kind: rejected
시도: KV 클램프 18 GiB → 14 GiB(768K 셀)
기각사유: 18 GiB 는 벤치 상주분 미산입으로 사살 1회 · 14 GiB 는 기동 밸리에서 호스트 가용이 절대 플로어를 밑돌아 사살 2회 — 10 GiB 로 수렴
재개조건: 노드 메모리가 더 크거나 기동 밸리가 줄어든 환경(가중치 적재 경로 변경)에서 다시 잰다
출처: [testlog_26090912_ds4f0731_029rc6_광의탐색_종합판정 §판정]
```

```hint-event
id: R6
kind: rejected
시도: spec off · eager 기본 형상(1M E0-base · 768K L0-base)
기각사유: 측정은 성립했으나 동시성1 decode 가 1M 16.67 · 768K 17.11 t/s 로 spec+graph 결합(E1-combo 30.54 · L4-combo 31.12)보다 낮아 승자에서 제외(순위가 아니라 레시피 선택)
출처: [sweep_map_26090912_e1m_levers §셀 (실행 순서), sweep_map_26090912_b768k_levers §셀 (실행 순서)]
```

```hint-event
id: R7
kind: rejected
시도: arch-wall 변종 사다리(자체 이식 · 포크 핀) 준비
기각사유: stock 0.29.0rc6 이 스모크를 통과해 불필요 — 옛 decode_dsv4 page_block64 벽이 해소됐다
출처: [testlog_26090904_ds4f0731_029rc6_attempt_build_스모크_판정 §판정 요약, plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §3.]
```

```hint-event
id: R8
kind: rejected
시도: NCCL 전송 = NCCL_IB_DISABLE=1 · NCCL_NET 미방출(render 2e91a1e · 이 셀 기동 시도 1)
기각사유: 엔진이 조용히 Socket 전송을 골라 RoCE 시험이 성립하지 않았다 — 로드 도중 --down 회수
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §1.]
```

```hint-event
id: R9
kind: rejected
시도: 커널 다운그레이드(포럼 383023 권고 6.17.0-1032 · 잔존 6.17.0-1031)
기각사유: 실행하지 않았다 — 이 레시피 · 이미지에서 회귀가 재현되지 않아(W13) 이 워크로드에 한해 불요로 판정
재개조건: GDR(DMA-BUF) 이 켜지는 드라이버 · 이미지 조합에서 ibv_reg_mr_iova2 가 재현되면
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §3.]
```

**무엇이 형상을 좁혔나.** KV 는 커널 페이지 포맷 때문에 fp8 하나로 좁혀졌고(R1 · R2), MoE 는 시도한 두 후보(triton · auto 전력) 중 humming 만 남았다(R3 · R4 · cutlass 등 다른 백엔드는 이 계보에서 시도 0). 클램프는 호스트 기동 밸리가 10 GiB 로 눌렀으며(R5), 그 위에서 spec(dspark k=7)과 cudagraph 를 결합한 형상이 두 컨텍스트 모두에서 기본 형상을 이겼다(R6). 이 셀은 그 형상을 그대로 두고 NCCL 전송만 바꿨다(R8 → W12). 커널은 되돌리지 않았다(R9).

## 2.4 오진과 정정
> 이 절이 답하는 질문: 계보 중 무엇을 잘못 진단했고, 어떻게 정정됐으며, 지금 그 주장의 지위는 무엇인가?

### M1 09-09 승자 셀의 verdict PASS 는 accept_len 1.0 자리표시자 위에 섰다
09-09 광의탐색은 `E1-combo`(= 대조군 태그의 셀 `e-1m-kvfp8-e1combo`)를 floor 13.2 로 PASS 판정했다. 09-14 하네스 교정 항목 1 이 소급 재판정해 보니, spec(dspark k=7)을 켠 셀인데 판정 레벨의 수용길이가 비어 있었고 판정기는 조용히 1.0 을 넣어 합격선을 세웠다. 같은 스윕 lite warm 에는 실측 수용길이 2.1648 이 있었다 — 결손은 측정 부재가 아니라 GuideLLM 파서에 lite raw 포인터 문서가 넘어간 승계 배선 결함이었다. 교정 뒤 재판정은 `NEEDS_RUBRIC · SPEC_ACCEPT_LEN_MISSING`(패스 B 전제)이다. **이 셀은 그 후속을 닫는다** — 교정된 체인으로 같은 레시피를 다시 재어 판정 레벨 accept_len 2.238372093023256(실측 승계 · 03 §3.1)로 floor 29.55 에서 PASS(explore)다.

> [원문] testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정 §5.1
> | 3 | `multi/e-1m-kvfp8-e1combo` | PASS (floor 13.20 · source expected) | **NEEDS_RUBRIC · `SPEC_ACCEPT_LEN_MISSING`** (accept_len_source=absent) | on(k=7) | None | 2.1648 | 15.53 → 15.53(accept 1.0 자리표시자) | 인증서 없음 |

```hint-event
id: M1
kind: misdiagnosis
증상: 09-09 E1-combo(1M spec+graph)가 floor 13.2 로 verdict PASS
서명: "15.53 → 15.53(accept 1.0 자리표시자)"
원인: spec 선언 셀의 판정 레벨 accept_len 이 null 로 승계돼 판정기가 1.0 을 조용히 넣었다(sweep_bench 가 GuideLLM 파서에 lite raw 포인터 문서를 넘김)
해소: 09-14 교정(실측 승계 · spec 선언 지문 · SPEC_ACCEPT_LEN_MISSING) · 이 셀이 교정된 체인으로 재측정 — accept_len 2.238372093023256 · floor 29.55 · PASS(explore)
검증: testlog_26091412 §5.1 #3 · 03 §3.1 이 셀 측정
전이등급: judgment
현재지위: 반증
출처: [testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정 §5.1, sweep_map_26090912_e1m_levers §셀 (실행 순서)]
```

### M2 "09-09 조건 = IB_DISABLE=1 + NET 미방출" 과 "09-09 에는 IBext_v11" 은 틀렸다
이 셀의 plan(§2 NCCL 행 · 정정 안 됨)과 testlog 초판(§1 · 2026-09-28 정정됨)은 09-09 경로를 "`NCCL_IB_DISABLE=1` + 플러그인 선택(HPC-X IBext RDMA)" 이라 적고 기동 시도 1 을 그 모방이라 했다. 거짓이다. 렌더러 이력에서 `NCCL_IB_DISABLE` 을 1 로 바꾼 것은 09-17 커밋 `d8c7b78`(그 diff 가 `"0"` → `"1"`)이고, 09-09 대조군의 엔진 로그(`output/multi/benchlog/sweep_e-1m-kvfp8-e1combo/lite_engine_e-1m-kvfp8-e1combo.log` · 이 페이로드의 계보 밖)는 `NCCL_IB_DISABLE set by environment to 0.` · `Loaded net plugin SPCX (v12)` · SPCX 장치 스킵 경고 1건 · `Using network IB` 를 기록한다. 즉 **09-09 PASS 와 이 셀의 시도 2 는 같은 NCCL 전송(SPCX 스킵 → 내장 IB · `GDR 0`)** 이고, 시도 1 이 오히려 09-09 와 다른 조건이었다. `IBext_v11` 은 09-17 OFAT 기록(testlog_26091721)의 서술이다. 이 정정은 비교의 변수를 좁힌다 — 09-09 대 이 셀의 **전송**(내장 IB · SPCX 스킵 · `GDR 0`)은 같다. 그러나 NCCL env 는 다르다: 이 셀 엔진 로그는 `NCCL_DMABUF_ENABLE` 0 을 echo 하나 09-09 캡처에는 그 줄이 없다(fe1bc42 · 09-17 도입), 렌더러 선언값도 `NCCL_NET_GDR_LEVEL` SYS→LOC · `NCCL_NET_GDR_C2C` 1→0 · `NCCL_NET_GDR_READ` 1→0(bb254f9 · 09-17 — 로그 echo 없음, 선언 측 차이)과 `NCCL_NET=IB` 명시(8319b4f)가 바뀌었다. 바뀐 변수는 커널 · 이미지 재빌드 · 이 NCCL env 일부다(03 §3.5 · testlog_26092808 §1 정정).

```hint-event
id: M2
kind: misdiagnosis
증상: 이 셀의 기동 시도 1 설정을 09-09 PASS 의 NCCL 조건이라고 적었다
서명: "플러그인 선택 = 09-09 경로(HPC-X IBext RDMA)"
원인: 09-17 에 바뀐(d8c7b78) 렌더러 불변 IB_DISABLE=1 을 09-09 에도 있던 값으로 착각했다 · 09-09 엔진 로그를 대조하지 않았다
해소: 09-09 조건 = NCCL_IB_DISABLE=0 · NCCL_NET 미방출 · SPCX(v12) 로드 후 장치 스킵 → Using network IB · GDR 0 — 이 셀 시도 2 와 같은 전송
검증: git show d8c7b78(render_dockerfile.py NCCL_IB_DISABLE "0"→"1") · 09-09 lite_engine_e-1m-kvfp8-e1combo.log 의 NCCL_IB_DISABLE set by environment to 0 · Using network IB
전이등급: judgment
현재지위: 반증
출처: [plan_26092808_커널7_0_DS4F0731_멀티캠페인_RoCE재현_1M서빙_풀벤치_hint §2., testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §1.]
```

### M3 "이미지 0.29.0rc6 은 09-24 재빌드" — 마지막 층 시각은 09-22T23:38Z
초판 1차 검증 testlog 와 devlog_26092809 §무엇을 했나 3 은 측정 이미지를 "09-24 재빌드" 라 적었다(`docker images` 의 "4 days ago" 를 읽은 것 — 추론). testlog_26092807 §3 은 2026-09-28 에 '2026-09-23 08:38 KST 재빌드(docker inspect Created)' 로 정정됐고, devlog 는 정정되지 않았다. 측정 이미지 digest 의 docker history 는 우리 Dockerfile 층의 끝 층 CreatedAt 을 `2026-09-22T23:38:10Z` 로 기록한다(01 §1.4 빌드 층 시각). 09-09 측정 이미지 digest(`sha256:4c9ab74a…`)와 이 셀의 digest(`sha256:a2c4ca49…`)가 다르다는 사실은 그대로다.

```hint-event
id: M3
kind: misdiagnosis
증상: 측정 이미지의 재빌드 날짜를 09-24 로 적었다
서명: "현 이미지(09-24 재빌드"
원인: docker images 의 상대 시각 표기를 날짜로 옮겼다(추론)
해소: docker history 끝 층 CreatedAt 2026-09-22T23:38:10Z(= 09-23 08:38 KST · 01 §1.4 · testlog_26092807 §3 정정과 일치) · 09-09 digest 와 다른 이미지라는 사실은 유지
검증: 01 §1.4 FACT:reproduce 빌드 층 시각
전이등급: judgment
현재지위: 반증
출처: [devlog_26092809_커널7_0_DS4F0731_RoCE_멀티캠페인_서사 §무엇을 했나, testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증 §3.]
```

### M4 "FP8 block MoE" — expert 는 fp4 이고 엔진은 Mxfp4 MoE 경로를 탄다
첫 plan 은 체크포인트를 "FP8 block MoE" 로 적었다. config 의 `quantization_config` 는 fp8 블록이지만 `expert_dtype` 은 fp4 이고, 엔진은 dense 에 FP8 블록 GEMM, MoE 에 `Using 'HUMMING' Mxfp4 MoE backend.` 를 골랐다(lite_engine 로그 L272 · L277). 태그 이름의 `q` 축(fp8)은 `quantization_config.quant_method` 에서 파생된 값이라 expert 형식을 말하지 않는다.

```hint-event
id: M4
kind: misdiagnosis
증상: 체크포인트 양자화를 FP8 block MoE 로 기술했다
서명: "가중치 155.4 GiB(FP8 block MoE)"
원인: config quantization_config(fp8 블록)만 읽고 expert_dtype 을 보지 않았다(추론)
해소: dense = FP8 블록(128×128) · expert = fp4(config expert_dtype) · 엔진 MoE 경로 = HUMMING Mxfp4
검증: lite_engine_ds4f0731-1m-spec7-roce.log L277 · 체크포인트 config.json expert_dtype
전이등급: arch-invariant
현재지위: 반증
출처: [plan_26090819_ds4f0731_멀티TP2_KV3군_1M768K_광의탐색_Hermes §2., lite_engine_ds4f0731-1m-spec7-roce.log]
```

**교정 묶음.** 계보의 교정 묶음은 하네스 교정(`plan_26091407` — testlog_26091412 머리의 근거 plan)이며, 계보에 실린 것은 그 항목 1 의 testlog(testlog_26091412) 하나다. 그 문서의 verdict 변동 4건 중 이 셀의 계보에 닿는 것은 1건이다.
- testlog_26091412 항목 1 · §5.1 #3 — `e-1m-kvfp8-e1combo` 판정 — PASS(floor 13.20 · accept 1.0 자리표시자) → NEEDS_RUBRIC · SPEC_ACCEPT_LEN_MISSING(패스 B) → 이 셀 재측정으로 PASS(floor 29.55 · 실측 accept_len)
- 같은 목록의 #1 · #2(`b-768k-kvfp8-l1spec` · `l4combo`)는 768K 자매 셀이라 이 셀의 판정 · 수치를 바꾸지 않는다. 다른 항목의 문서는 계보에 없어 검토하지 않았다(검토한 항목 1개 · verdict 변동 행 4건).

## 2.5 값의 이력
> 이 절이 답하는 질문: 이 셀의 핵심 값들은 어떻게 정해졌고, 그 값의 형상이 버티는 기전은 무엇이며, 어떤 값이 조정된 적 없이 승계됐는가?

이 셀의 값은 전부 09-09 대조군 태그의 트리플렛에서 **재명명만 하고 승계**했다(plan_26092808 §2 "값 변경 0"). 아래는 그 값들이 계보에서 어떻게 정해졌는지다.

**호스트 메모리 예산(선언).** 산식은 바닥 = 총량 − 가중치 − KV − overhead 이고, 이 셀의 선언 대입값은 `mem_total_mib=124608 − weights_mib=79577 − kv_mib=10240 − overhead_mib=13312 = floor_mib=21479`(01 §1.4 이벤트 표 · 선언값 — 측정 아님), arm 상한 18,407 MiB 다. overhead 13,312 는 09-09 스모크가 쓴 12,265 에 여유를 얹은 선언이다(plan_26092808 §2). 이 선언은 양 노드에서 honored 됐고 사살 · 트립은 0 이었다(01 §1.4 · testlog_26092808 §2). 로드 중 `watchdog_highrate_hold`(`legacy_rule_would_trip=true`)가 시도 1 · 2 에서 각 1회 찍혔다 — 옛 규칙이면 사살했을 가용 급락률을 선언 창이 보류한 기록이다(01 §1.4 행 3 · 9).

**KV 클램프 `kv-cache-memory-bytes` 10737418240 (V7).** 768K 셀에서 18 → 14 → 10 GiB 로 세 번 수렴했다 — 바인딩은 KV 필요량이 아니라 호스트 기동 밸리다(W7). 트리플렛 주석이 그 산식을 적는다.

> [원문] ds4f0731-1m-spec7-roce.yaml §L13-15
> kv-cache-memory-bytes: 10737418240   # 수렴 클램프 = 10GiB (3차 수렴 · binding = 호스트 기동 밸리: 14GiB 시 밸리 ~8.2GiB 가 절대플로어 10240 하회 → memwatch 킬 2회 실측
> #   · kv ≤ 124610-79577-12265-(10240+밸리마진 8GiB+벤치 4GiB) ≈ 12288MiB → 10GiB(trial-2 동밸리 생존 실증) · bytes/token 6069)
> max-num-seqs: 1                       # 동접 = 잔여 KV 최대활용 · 1M×6069B=5.91GiB → 10GiB/5.91 = 1.73x → 1 (bytes/token 1M 재측정 후 재산정)

주석의 6069 B/token 은 768K 실측이고, 1M 실측은 5529 B/token 이다(testlog_26090912 §판정의 KV 줄). 이 셀의 엔진은 같은 10 GiB 에서 `GPU KV cache size: 1,941,478 tokens` · 1M 기준 **1.85x** 를 보고했다(lite_engine L628) — 1M 요청 하나의 필요량 대비 설정값의 배수는 엔진이 보고한 이 1.85 다. 이 값은 이 셀 계보에서 조정된 적이 있어(768K 수렴) `tuned` 로 둔다.

**동시 시퀀스 `max-num-seqs` 1 (V8).** 위 주석이 1.73x 를 내림해 1 로 정한 **손레버**다(lockset `batch_source=hand-lever`). 엔진 보고 1.85x 로 다시 재도 1 이다. 스윕 기록이 없으므로 `inherited`. 이 값 때문에 동시성 2 이상에서 총 출력이 약 33 tok/s 로 평탄하고 요청당 decode 만 나뉜다(03 §3.2).

**eager/graph `enforce-eager` false (V10) · spec `speculative-config` dspark k=7 (V11).** 768K 레버 스윕이 두 레버를 따로 · 함께 켜 봤다: 동시성1 기본 17.11 → spec 29.72 · graph 26.14 · 결합 31.12, 1M 은 기본 16.67 → 결합 30.54(sweep_map_26090912_b768k_levers · e1m_levers 동시성 축). 두 값은 계보에서 바꿔 가며 잰 기록이 있어 `tuned` 다. 단 k=7 자체는 스윕되지 않았다 — k 는 `dspark_block_size`(5) 이상이어야 한다는 yaml 주석 · 09-09 검증값을 따른 것이다.

**판정 입력 accept_len.** 09-09 판정은 1.0 자리표시자 위에 섰고(M1) 이 셀은 판정 레벨의 실측 승계값 2.238372093023256 을 썼다(03 §3.1 · bench_report 루프라인 "spec(MTP) on (측정)"). 손 승계 입력은 없다.

**gmu 0.85 (V3) · max-model-len 1048576 (V4) · MoE humming (V9) · KV fp8 (V6).** gmu 는 셀 config `target_gmu` 선언이고 09-09 두 스윕의 모든 셀이 같은 값이었다(스윕 기록 없음 → `inherited`). max-model-len 은 plan_26090819 §2 의 사용자 지정 컨텍스트 축이다. KV fp8 과 humming 은 다른 값이 실패로 관측된 필요조건이다(W5 · W8).

**버티는 기전 대 실패한 형상.**
- 이 형상(1M · fp8_ds_mla KV · 클램프 10 GiB · seqs 1 · humming · spec+graph · TP=2 RoCE) — 관측: 사살 · 트립 0(01 §1.4 기동 시도 2) · 원장이 남긴 실측 가용은 로드 중 `watchdog_highrate_hold` 순간값(69,032 · 80,175 MiB)뿐이다. 선언 바닥 21,479 MiB > arm 상한 18,407 MiB 는 둘 다 선언이며 선언이 honored 되는 조건이지 비발화의 관측 기전이 아니다 — 형상이 버틴 것은 확정(사살 0), 가용이 상한 아래로 내려가지 않았다는 연속 관측은 원장에 없다(미결).
- KV 클램프 18 GiB(768K) — 벤치 상주분(4 GiB)을 예산에 넣지 않아 절대 플로어에 걸려 사살 1회 — 확정(관측) — `docs/devlog/devlog_26090912_ds4f0731_광의탐색_셀루프_서사.md` §수렴의 3단계 · 계보 밖.
- KV 클램프 14 GiB(768K) — 기동 밸리가 KV 몫과 겹쳐 가용이 절대 플로어 아래로 — 사살 2회 · 10 GiB 생존 대조로 확정 · 밸리의 구성(페이지캐시 적층)은 계보 밖 devlog 서술로 미결 — W7 · testlog_26090912 §판정 · ds4f0731-1m-spec7-roce.yaml L13.
- kv auto/turboquant — 커널 페이지 포맷(fp8_ds_mla 강제) 때문에 기동 자체가 거부 — 확정 — W5.
- moe triton — MXFP4 TRITON 커널의 SILU 미지원 — 확정(엔진 거부) — W8.
- NCCL Socket 전송(09-17 기준선) — 이 형상에서 1M 수치는 이 계보에 없다(09-17 측정은 다른 모델) — 미결.
- NCCL IB_DISABLE=1 + NET 미방출 — 전송이 Socket 으로 떨어져 형상이 달라진다(시도 1) — 확정(관측) · 기전(SPCX 스킵 뒤 Socket) 은 추론 — W12.

## 2.6 되풀이하지 말 것
> 이 절이 답하는 질문: 다음 사람이 같은 비용을 치르지 않으려면 무엇을 하지 말아야 하는가?

- 커널 7.0 에서 RoCE 가 깨진다고 보고 곧장 커널을 되돌리지 마라 — W13 · R9(이 레시피 · 이미지는 `Using network IB` 로 1M 서빙 · full 벤치 완주) — 먼저 엔진 로그의 `Using network` 줄과 `iova2` 유무를 네 조합에서 본다.
- rdma 를 "IB_DISABLE 유지 + NET 미방출" 로 선언하지 마라 — W12 · R8(SPCX 플러그인이 장치를 건너뛰면 조용히 Socket) — `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` 를 명시하고 엔진 로그 `Using network IB` 로 실증한다.
- 생성물 `.env.interconnect` 를 손으로 고쳐 서브에 보내려 하지 마라 — W11(sync_to_sub 는 렌더러로 다시 만들어 배달한다 · 손수정은 서브에 닿지 않는다) — `manifest.interconnect.nccl_transport` 를 고치고 정식 배달한다.
- KV dtype 을 auto · turboquant 로 시도하지 마라 — W5 · R1 · R2 — fp8(fp8_ds_mla)만 성립한다.
- moe-backend 를 triton 이나 auto 로 바꾸지 마라 — W8 · R3 · R4 — humming 을 유지하고, 바꿀 거면 엔진 로그의 MoE 경로 줄부터 본다.
- KV 클램프를 필요량(1M 요청당) 기준으로만 키우지 마라 — W7 · R5(바인딩은 기동 밸리) — 호스트 예산 선언(바닥 = 총량 − 가중치 − KV − overhead)을 먼저 세우고 기동 밸리를 잰다.
- 예산 선언 없이 기동하지 마라 — W3 · W7 — `SMOKE_BUDGET_OVERHEAD_MIB` · `READY_MAX` 를 선언하고 멀티 스모크 정문으로 띄운다(compose 직접 기동 ✗).
- 09-09 태그의 verdict PASS(floor 13.2)를 성능 근거로 옮기지 마라 — M1(accept 1.0 자리표시자) — 이 셀의 floor 29.55 · 실측 accept_len 판정을 쓴다.
- 트리플렛 yaml 3번째 줄 주석의 `max_model_len=786432` 와 KV 주석의 6069 B/token 을 이 셀 값으로 읽지 마라 — 768K 시절 주석이 승계된 것이다(ds4f0731-1m-spec7-roce.yaml L3 · L14) — 실제 값은 yaml 키(1048576)와 1M 실측(5529 B/token · testlog_26090912)이다.
- 엔진 기동 중 `No available shared memory broadcast block found in 60 seconds` 경고를 hang 으로 보지 마라 — 이 셀에서 2회(1분) 뒤 `init engine … took 195.43 s` 로 정상 진행했다(lite_engine L682 · L726 · L730) — 경고가 수 분 이상 이어질 때만 벽으로 본다.
- `--backend` 없이 full 스윕을 부르지 마라 — W13 — DeepSeek 계열은 `--backend openai-chat`(09-09 기본과 같은 포맷).
- Hermes 류 tool call 용처에 이 레시피를 그대로 상주시키지 마라 — Q3(`tool_choice:"auto"` 가 HTTP 400) — `enable-auto-tool-choice` 를 더해 다시 띄우고 tool call 1회를 스모크에 넣는다.

## 2.7 열린 물음
> 이 절이 답하는 질문: 아직 설명되지 않았거나 시도되지 않은 것은 무엇인가?

```hint-event
id: Q1
kind: open-question
물음: 09-17 커널 7.0 위 RoCE 실패(ibv_reg_mr_iova2)는 무엇이 만들었나 — 이 셀은 같은 커널 · 같은 내장 IB 에서 성공했다
현재상태: 09-17 원시 엔진 로그 부재 · 당시 설정은 GDR 세부 노브 OFAT 중이었고 이미지는 이 셀과 달랐다 — 이 셀 이미지(a2c4ca49…)는 2026-09-22T23:38:10Z 에 만들어져 09-17 에는 존재하지 않았다(01 §1.4 빌드 층 시각 · 09-17 이미지의 digest 자체는 미기록 · 09-09 digest 4c9ab74a…) · 이 셀과 1차 모델리스 시험 5조건은 모두 iova2 0건
다음관측: 09-17 의 NCCL env(OFAT 조합)를 커널 7.0 에서 그대로 재현하고 엔진 로그 `Using network` · `GDR` · iova2 줄을 본다
출처: [testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증 §3., testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §3.]
```
포럼의 실패 서명(`NVRM … NV_ERR_NO_MEMORY`)은 GPU 메모리 등록 쪽이고, 이 셀은 그 경로를 타지 않았다(`GDR 0`) — 미재현의 가장 유력한 이유라는 것은 추정이다.

```hint-event
id: Q2
kind: open-question
물음: 드라이버가 GB10 에 DMA_BUF_SUPPORTED · GPU_DIRECT_RDMA_SUPPORTED 를 0 으로 보고하는 것은 커널 7.0 에서만인가
현재상태: 커널 7.0 에서 0 / 0(host libcuda · compat 동일) · 6.17 에서는 미측정 · 09-09(6.17) 엔진 로그도 GDR 0
다음관측: 6.17 부팅에서 같은 cuDeviceGetAttribute 조회
출처: [testlog_26092807_커널7_0_NCCL_RoCE_회귀_1차검증 §1.]
```

```hint-event
id: Q3
kind: open-question
물음: 이 레시피는 tool call 용처(Hermes Agent)에서 tool_choice auto 를 받지 못한다 — enable-auto-tool-choice 를 더하면 성능 · 기동이 달라지나
현재상태: 벤치 뒤 상주 서빙에서 chat + tools + tool_choice auto 요청이 HTTP 400("auto" tool choice requires --enable-auto-tool-choice and --tool-call-parser to be set)으로 거부됐다(관측 · curl 1회) · tool-call-parser deepseek_v4 는 있다 · 성능 영향은 없을 것으로 추정(API 층 플래그 · 미측정) · 재기동은 하지 않았다
다음관측: enable-auto-tool-choice: true 를 더해 재기동 → tool call 1회 스모크 → lite 1회로 decode 대조
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §5.]
```

```hint-event
id: Q4
kind: open-question
물음: 동시성1 decode 가 09-09 대조군보다 높은 차이는 무엇이 만들었나
현재상태: 이 셀 32.2 · 09-09 E1-combo 30.54(비 1.054) — 이 셀 판정점 재현 밴드 3.15% 보다 크다 · 두 측정의 전송은 같다(내장 IB · M2) · 달라진 것은 커널 · 이미지 digest · NCCL env 일부(DMABUF_ENABLE=0 이 이 셀에만 · GDR_LEVEL/C2C/READ 선언값 · NET=IB 명시 — M2) · 측정 도구 판본 · lite warm accept_len(이 셀 2.238 · 09-09 2.1648 · 같은 lite warm 레그 · 비 1.034) — 원인 미분해
다음관측: 같은 이미지 digest 로 커널만 바꾼 대조 또는 accept_len 을 고정한 비교
출처: [sweep_map_26090912_e1m_levers §셀 (실행 순서), bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0 §반복 축, testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정 §5.1]
```

```hint-event
id: Q5
kind: open-question
물음: 이 판정은 외부 레퍼런스(E) 없이 roofline 만으로 섰다 — 외부 수치 대비 위치는
현재상태: verdict PASS(explore) · primary = expected_achievable(roofline×MBU) 34.77 · E-search 상태 no · 경고 E-not-attempted
다음관측: 같은 모델 · 2×GB10 · TP=2 외부 보고 수치 수집 뒤 재판정
출처: [bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0 §판정]
```

```hint-event
id: Q6
kind: open-question
물음: cold TTFT 7419.7 ms(첫 요청)는 무엇이 차지하나 — 판정점 TTFT p50 은 632 ms 다
현재상태: lite cold 1회 관측 · 분해 없음(컴파일 · 캐시 워밍 여부 미관측)
다음관측: 재기동 직후 첫 요청의 엔진 로그 타임스탬프 분해
출처: [bench_report_26092809_deepseek-v4-flash-0731_GB10_0.29.0 §lite 지표]
```

```hint-event
id: Q7
kind: open-question
물음: 노드 ABI 동일성 관측(attestation)이 이 측정 실행과 같은 실행의 것인가
현재상태: attestation v2 는 build_ledger · driver · torch · vllm_sha 전부 equal · 시각 기록은 같은 실행을 가리킨다 — master 컨테이너 기동 23:37:18Z(시도 2 선언 23:37:13Z 직후 · 같은 컨테이너가 상주) → attestation mtime 23:54:55Z(provenance 'docker exec in each running container') → 측정 23:57:04Z~00:35:13Z · 그 사이 budget_clear · 재선언 없음(01 §1.4 행 6~33) · 한계는 파일에 실행 id 결속 필드가 없다는 것(다음 실행이 덮는다)
다음관측: 측정 실행 id 를 attestation 에 결속하는 producer
출처: [testlog_26092808_커널7_0_DS4F0731_RoCE_1M서빙_회귀판정 §2.]
```
