# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090602_캠페인재개_2브랜치_서사.md`
- testlog: `../testlog/testlog_26090602_캠페인재개_2브랜치_광의탐색_판정.md`

## 서사

### 증상 — 0.19.1 은 2노드로 뜨지 않는다

이 저장소의 선행 실측에서 vLLM **0.19.1 은 분산 서빙이 성립하지 않았다**(모델 무관 ·
`config/utils.py:318`). 캠페인은 0.19.1 을 목표로 시작했고, 사용자 판단으로 **"0.19.0 탐침 →
불가 시 0.18.0"** 순서를 밟았다.

### 원인 규명 — 회귀 구간을 좁혔다

0.19.0 으로 탐침한 결과 **2노드 분산이 성립한다.** 양 rank 컴파일 · `/health` 200 · 그리고
**실제 추론 응답**까지 확인했다(testlog §2.0). 따라서 회귀는 **(0.19.0, 0.19.1] 구간**에서
들어왔다. 0.18.0 으로 후퇴할 필요가 없었다.

### 이 버전에서 커널 축은 열리지 않는다 — 다만 **소리를 낸다**

sm_121a(GB10)에서 시험한 축의 결과다(testlog §2 표):

- `attention_backend=FLASHINFER` → 실측 `triton_attn`. **선언이 조용히 무시된다.**
- `moe_backend=triton` → `ValueError: Mxfp4 MoE backend 'TRITON' does not support the deployment
  configuration since kernel does not support current device cuda.`
- `VLLM_USE_FLASHINFER_MOE_MXFP4_BF16=1` → `ValueError: ... the current device capability is not
  supported`
- KV `fp8_e5m2` → serve 실패(사인 미회수 · testlog §2.4)

**거부는 개선이다.** 같은 MoE 선언이 0.18.0 에서는 조용히 `marlin` 으로 되돌아갔고, 계측이
실측-대-선언을 대사하지 않으면 운영자는 자기가 쓰지 않는 커널을 쓰고 있다고 믿게 된다
(testlog §1.2·§2.1).

⚠ **귀속의 한계**: 그 두 관측은 버전뿐 아니라 **모델(20b↔120b)과 토폴로지(single↔TP=2)도 함께
바뀐** 비교다. 0.19.0 의 거부 메시지가 `current device capability` 를 근거로 삼는다는 점은 기전이
디바이스 능력 게이트임을 가리키지만, **버전 단독 귀속은 아직 증명되지 않았다**(testlog §2.1).

### 성능 — 축을 움직여도 1.2% 안이다

`conc=1` 기준 50.26(KV auto) / 50.58(KV fp8 기준선) / 50.86(KV fp8 + FLASHINFER 선언, 실측
triton_attn). 세 값이 **1.2% 폭** 안에 있다. 단일노드 캠페인(0.18.0 · 20b)이 2.4% 폭이었던 것과
같은 결론이다 — **이 하드웨어에서 커널 축은 성능 레버가 아니다.**

### 분산 평면의 경계가 실제로 지켜졌다

- 서브 노드의 멀티 브랜치 추적 파일은 **50건이고 `.claude/skills/` 는 0건**이다. 런타임 스킬이
  배달되지 않는다(testlog §3).
- 서브의 `output/` 추적분 13건은 **전부 빌드킷**이다. `configs/`·`envs/`·`benchlog/` 는 0건 —
  모델 트리플렛이 전파되지 않는다(`policy:MODEL_TRIPLET_NO_SUB_PROPAGATION`).
- **이미지는 전송하지 않는다.** 같은 태그인데 이미지 ID 가 노드마다 다르다는 것이 그 증거다
  (testlog §3.1). 크기 차이(33.4 GB vs 21.9 GB)는 스토리지 드라이버 차이일 뿐이므로
  **완료 판정에 크기를 쓰면 위양성**이다.

## 되풀이하지 말 것

1. **0.19.1 로 2노드를 시도하지 마라.** 모델과 무관하게 성립하지 않는다. 이 계열에서 분산이
   필요하면 **0.19.0** 을 쓴다.
2. **`moe_backend=triton` 과 mxfp4 FlashInfer 스위치는 sm_121a 에서 열리지 않는다.** 0.19.0 은
   `ValueError` 로 즉시 죽으므로 로드 시간을 낭비하지 마라.
3. **KV `fp8_e5m2` 를 쓰지 마라.** 이 계열에서 열리지 않는다.
4. **`SMOKE_BUDGET_OVERHEAD_MIB` 를 선언하고 들어가라.** 기본값 사용은 금지되어 있어 스모크가
   막힌다. gpt-oss-120b/GB10 실측값은 **17,971 MiB** 다.
5. **`docker compose down` 을 직접 호출하지 마라.** 예산 선언이 stale 로 남아 다음 기동의
   clear/declare 와 레이스가 난다. 반드시 제공된 teardown 경로를 쓴다.
6. **서빙 실패를 teardown 으로 덮지 마라.** 컨테이너를 내리면 엔진 로그가 함께 사라져 사인이
   증발한다 — 실제로 셀 하나(KV `fp8_e5m2`)의 원인을 영영 회수하지 못했다(testlog §2.4).
   로그 보존을 먼저 배선한 뒤 실패를 다뤄야 한다.
7. **간헐적 스트리밍 절단이 있다.** 서버측 로그는 조용한데 클라이언트가
   `peer closed connection without sending complete message body` 를 받는다. 동시성이 올라갈수록
   빈도가 는다(관측: level_08 에서 5.6%). 측정이 무효가 되면 재측정하되, **재조립
   (`--reassemble-only`)으로 대신하지 마라** — 낡은 산출물에서 성공해 새 측정이 안 된다.
