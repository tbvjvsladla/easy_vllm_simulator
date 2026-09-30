---
name: terraforming_node
description: >-
  **토폴로지-중립 진입(온보딩) 오케스트레이터** — fresh-clone/미테라포밍 환경에서 능동 발동해 **첫 동작으로
  토폴로지(① 단일노드 ② N대 멀티노드)를 인터뷰로 확정**하고, 그에 따라 노드를 스캔·manifest.yaml(스킬 간
  단일 계약)을 채운다. **single → 단일노드 온보딩**(manifest nodes:[] 기본; 서브 등록 시 A2A 에이전트 제어만 활성 — 빌드킷 배달 평면은 dormant) · **multi →
  서브노드 스캔·연결/성능 검증 + 서브 코드에이전트(Claude Code) 작업환경 구축**(메인 렌더→전달→model-less 카나리).
  "이제 뭐해야해", "환경 셋업", "온보딩", "테라포밍", "노드 스캔", "단일/멀티 결정", "manifest 생성",
  "서브노드 셋업", "인터커넥트 점검", "서브 에이전트 환경 구축", "서브 코드에이전트 셋업" 같은 지시·fresh-clone 감지에 발동.
  **무단 스캔 금지** — 토폴로지 인터뷰 → (multi면 5-전제조건 인터뷰) → 사용자 승인 후 스캔. 토폴로지 미선언 시
  emit fail-closed. 성능 미검증 멀티 진행은 fail-closed로 차단.
when_to_use: >-
  온보딩·테라포밍 외에도 이 스킬이 절차 정본인 세 영역에서 부른다 — (1) 노드 블랙박스: 호스트 안전체계 Y/N,
  ETA·열·전력 워치독, docs/logs 데이터 평면, L3 수행지시서. (2) 노드 오케스트레이터: 메인↔서브 제어 평면,
  A2A 위임·턴제 릴레이·세션 재개·감독 스텝(--supervise-step), 러너 사다리 회전, 그라운딩 교환(library_request).
  (3) 캠페인 체인: campaigns/ 배치·phase 전이·campaign_init writer·배정(assignments)·purge 게이트.
---

# terraforming_node

**모든 환경 셋업의 토폴로지-중립 진입점**이자, 그 뒤 노드를 다루는 세 하위 기능 — **설치 · 블랙박스 ·
오케스트레이션(+캠페인 체인)** — 의 절차 정본이다. 판단의 문은 이 파일 하나이고, 절차 본문은
`references/` 의 네 폴더에 있다. 먼저 토폴로지를 인터뷰로 확정(§0.5)한 뒤 분기한다:

- **single** → 단일노드 온보딩(§1S). 서브를 등록하면 **A2A 에이전트 제어**가 열리지만 멀티의 배달·버전동기 평면은 열리지 않는다(§2.7.0).
- **multi** → 진입 루틴(§1) + **서브 에이전트 환경 구축**(§2: 메인 렌더 → 서브 전달 → 카나리).

이후 `upstream-version-watch`(컨테이너 빌드) · `vllm-recipe-explorer`(서빙전략)와 파이프라인 의존순서로 협력한다.
설계 원칙: **스캔·게이트·렌더·일치단언은 결정론 스크립트**, **인터뷰·승인·HITL 판정은 이 페르소나**. 둘을 섞지 않는다.

## Contract

- **Goal** — 토폴로지를 인터뷰로 확정하고 노드 사실을 스캔해 `output/<topology>/manifest.yaml` + 테라포밍-완수 Flag 를 발급한다(멀티면 서브 에이전트 작업환경까지).
- **When to invoke** — fresh-clone(미테라포밍) 감지 · HW/네트워크/manifest 변경 · `scripts/staleness_gate.py` 가 preflight 필요를 반환할 때. **조건부 preflight** 이지 매 작업 상시단계가 아니다. 블랙박스·오케스트레이션·캠페인 절차가 필요할 때는 아래 §라우팅 표의 reference 로 간다.
- **Inputs** — 사용자 인터뷰 답(토폴로지·모델 획득 모드·multi 5-전제조건) · 노드 실측 스캔 · 현재 git 브랜치 · (선택) 이전 attestation.
- **Outputs** — `output/<topology>/manifest.yaml`(HITL 반영) · Flag attestation · (multi) `output/<topology>/sub_provision/` 스테이징 + 카나리 리포트.
- **State transitions** — 산출물은 `execution-approved` 의 **전제**(HW 사실·Flag)를 만든다. runtime-ready/evidence-complete/promotion-ready 판정은 `.claude/policies/runtime/completion_gate.py` 소유.
- **Deterministic commands** — 아래 §공개 표면 표가 전수다. 플래그의 단일 권위는 각 스크립트의 `--help` 다(여기 손으로 적지 않는다).
- **Handoff contract** — Flag 발급 → `upstream-version-watch`(컨테이너 빌드) → `vllm-recipe-explorer`(서빙전략). 서브 전달차는 `upstream-version-watch/scripts/sync_to_sub.sh` 단일 경로.
- **Owns (state)** — `manifest.yaml`(메인 + **서브 manifest** §2.7.10) · `terraforming-flag` · `agent-card`(unsigned capability/discovery metadata) · `sub-agent-env` · `node-identity`(§2.7.6) · `topology-axis-contract`(§2.7.0) · `sub-control-plane`(§2.7) · `grounding-exchange`(§2.7.8) · 캠페인 체인 writer(`campaign_init.py`) · 블랙박스 데이터 평면(`docs/logs/<node_id>/`).

## Mandatory procedural spine

필수 순서다 — 앞 단계의 증거 없이 뒤 단계로 가지 않는다(무단 스캔·무증거 기입 차단이 이 순서에서 나온다).

1. **staleness 판정(조건부 preflight 진입)** — `python3 .claude/skills/terraforming_node/scripts/staleness_gate.py --topology <single|multi> --repo . --now <YYYY-MM-DD> [--observed <scan.json>]`. exit 0(`FRESH`)이면 재진입 불요 — 여기서 끝낸다. exit 4 면 reason code(`MANIFEST_ABSENT`/`FLAG_ABSENT`/`HW_DRIFT`/`ATTESTATION_STALE`)가 아래 단계의 착수 근거다.
2. **토폴로지 인터뷰**(§0.5.2) — 스캔보다 먼저. 미선언 시 emit fail-closed(§0.5.3), 브랜치 불일치 시 HITL 브랜치 전환(§0.5.4).
3. **모델 획득 모드 인터뷰**(§0.5.7) → **0차 init-plan 발행 + 자기-HITL 승인**(§0.5.8).
4. **스캔**(single=§1S · multi=§1.1 5-전제조건 인터뷰 → §1.2 사용자 승인 → §1.3 스캔 → §1.4 성능 게이트).
5. **게이트 + 3자-일치 단언**(§1.5) → 통과 시에만 **manifest + Flag 기입**(§1.6 / §1S, HITL).
6. **(multi) 서브 에이전트 환경** — 렌더 → 전달 → **model-less 카나리**(§2.3–§2.5). 카나리 미통과 시 done 선언 ✗.
7. **호스트 안전체계 세션 최종 Y/N**(§2.6) — Flag 발급 **이후**의 독립 선택조항.


## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| 재진입이 필요한지 불명(HW·manifest·attestation 변화 판정) | `.claude/skills/terraforming_node/scripts/staleness_gate.py` |
| 스캔/게이트/3자-일치 blocked(비0 종료) · emit fail-closed | `.claude/skills/terraforming_node/scripts/scan_node.py` · `references/install/onboarding.md` |
| 서브 환경 렌더 실패(미치환 placeholder·필수 필드 누락) | `.claude/skills/terraforming_node/scripts/render_sub_env.py` · `references/install/sub_agent_env.md` |
| 서브 위임/카나리의 provider 실행문법이 필요 | `.claude/skills/terraforming_node/references/orchestration/agent-control-adapter.md` |
| Flag 미발급이라 런타임 스킬이 info-only 로 떨어짐 | `.claude/skills/terraforming_node/scripts/manifest_contract.py` |
| 서브에 무엇을 해도 되는지 모호(저작/스캔/정비) · 평면 A/B 혼동 · 서브 git 교착 | §2.7 → `references/orchestration/control_planes.md` |
| "고쳤는데 안 갔다" / "안 고쳤는데 갔다"(커밋 vs 인덱스 vs 파일시스템) | §2.7.4 → `references/orchestration/control_planes.md` |
| `docs/logs/<node_id>` 경로가 노드마다 갈림 · hostname 이 경로에 샘 | §2.7.6 → `references/orchestration/topology_identity.md` |
| 싱글인데 멀티의 배달·버전동기 개념이 끼어듦 · `role: sub` 의 의미가 모호 | §2.7.0 → `references/orchestration/topology_identity.md` + `scripts/node_role_contract.py` |
| 서브 결정의 근거가 불명 · 인용 없는 빌드/서빙 결정 | §2.7.8 → `references/orchestration/grounding_exchange.md` + `scripts/library_exchange.py` |
| 위임이 소진·중단됨 · 재개 여부 · 러너(백엔드×모델) 실패 | §2.7.7a · §2.7.11 → `references/orchestration/a2a_relay.md` |
| 워치독 트립·사살 · 블랙박스 미설치/검증 실패 | §2.6 → `references/blackbox/host_safety.md` · `docs/logs` 평면은 `references/blackbox/logs_plane.md` |
| 캠페인 상태가 비었거나 어긋남 · purge 가 안 열림 | `references/campaign/` (layout · phases · writers · assignment_supervision · purge) |

## § 라우팅 — 번호는 안정 식별자다

§ 번호와 제목은 이 파일에 남고 본문은 reference 에 있다. 외부 문서·코드의 `SKILL.md §x` 인용은 여기서 해소된다.

### 설치 — `references/install/`

## 0. 전제 / 입력
→ `references/install/onboarding.md`

## 0.5 토폴로지 진입 인터뷰 게이트 (판단 — **첫 동작**, fail-closed)
→ `references/install/onboarding.md` (0.5.1–0.5.9). **첫 동작은 토폴로지 인터뷰다** — 미선언이면 스캔·emit 은 fail-closed 이고, 브랜치로 토폴로지를 추론하지 않는다.

## 1S. 단일노드 온보딩 (topology=single — 짧은 경로)
→ `references/install/onboarding.md`

## 1. 멀티노드 진입 루틴 (topology=multi — §0.5 에서 multi 확정 후)
→ `references/install/onboarding.md` — ### 1.1 5-전제조건 인터뷰 · ### 1.2 사용자 승인 게이트 · ### 1.3 스캔 · ### 1.4 성능 게이트 · ### 1.5 토폴로지 게이트 + 3자-일치 단언 · ### 1.6 manifest 기입 · ### 1.7 서브 work_dir 프로비저닝 게이트

## 2. 서브 에이전트 환경 구축 (A2A-개념)
→ `references/install/sub_agent_env.md` — ### 2.1 스킬 분류학 · ### 2.2 구축 아티팩트 · ### 2.3 렌더-온-메인 → 전달(인가 체인) · ### 2.4 A2A-개념 협업 계약 · ### 2.5 완료 게이트 — model-less 카나리 라운드트립. **카나리 미통과 = done ✗.**

### 블랙박스 — `references/blackbox/`

## 2.6 호스트 안전체계 — 세션 최종 선택조항 (Y/N · 양 토폴로지 공통)

> **양 토폴로지 공통 최종 스텝**: single 은 §1S manifest+Flag 기입 직후 · multi 는 §2.5 카나리 통과 직후 이 절로 온다.
> **Flag 발급 이후**에 오는 **독립 Y/N 선택조항**이다 — 표준 온보딩(스캔·게이트·manifest·Flag)은 이미 완수됐고, 안전체계는 **선택**이다(설치 안 해도 Flag valid).

- 거부(opt-out)는 존중한다. 통합메모리 노드의 후속 안내는 서빙 기동 직전 **채팅 1줄**뿐이다 — **serve 스크립트/로그 배너 코드변경 ✗**. **discrete GPU 노드는 무경고.**
- 설치·검증은 사람이 sudo 로 한다(에이전트 무인 sudo ✗). 절차·레벨(L1/L2/L3)·자산 표 → `references/blackbox/host_safety.md` · ### 2.6.1 노드블랙박스 설치 — `docs/request/` 수행지시서 위임도 같은 파일.
- 자산 뿌리: `.claude/skills/terraforming_node/scripts/node_blackbox/`(현행 설치자·워치독·데이터 평면) · `.claude/skills/terraforming_node/scripts/host_safety/`(협역 워치독 정본 `mem_watchdog.sh` + 승계된 레거시).
- 데이터 평면 `docs/logs/<node_id>/` 의 경로·포맷·수명 → `references/blackbox/logs_plane.md`.

### 오케스트레이션 — `references/orchestration/`

## 2.7 노드 제어 규약 — 메인↔서브 평면·범주·권위 (정본)
→ `references/orchestration/control_planes.md`. **§2.7.0 분기표를 먼저 읽는다** — 어느 sub 인지가 정해져야 어느 평면인지가 정해진다.

### 2.7.0 토폴로지 분기 — **제1축**
→ `references/orchestration/topology_identity.md`

### 2.7.1 권한 평면 A/B
### 2.7.2 메인이 서브에 하는 행위의 3범주
### 2.7.3 메인↔서브 B0–B3 상태 표
### 2.7.4 권위 평면 계약 — 어느 도구가 무엇을 읽는가
### 2.7.5 sync 절차의 평면 분리
→ 위 다섯 절 모두 `references/orchestration/control_planes.md`

### 2.7.6 노드 정체성(node-identity) — `role` + `rank` 스킴
→ `references/orchestration/topology_identity.md` — (a) node_id — 이름 · (b) rank — 위치(멀티 전용) · (c) 렌더 산출물에 실릴 형태

### 2.7.7 A2A 제어명령 프로토콜
→ `references/orchestration/a2a_relay.md` — 2.7.7a 턴제 릴레이와 자율 재개. **재개의 기본은 `relay.py --supervise-step <camp> --apply` 자동 재발급**이고, 비용 상한·통신/모델 붕괴·전진 없음만 사람에게 팝업한다(그때의 수동 경로 = `--continue --apply`).

### 2.7.8 그라운딩 교환 포맷 — 서브 요청 → 메인 색인 → 반출
→ `references/orchestration/grounding_exchange.md` — 세 메시지 · 증거그래프 어휘 대응 · 하드게이트(세 질문 + Freshness) · 배선. **누락은 기계가 fail-closed 로 잡고 거짓은 사람이 리뷰한다.**

### 2.7.9 노드 오케스트레이터 — Phase 감독
→ `references/campaign/phases.md` · 특화층 오케스트레이션 전략은 `references/orchestration.topology.md`(브랜치별 내용 · 싱크 비전파).

### 2.7.10 Agent_Card v2 (A2A 1.0.1) · 서브 manifest
→ `references/install/sub_agent_env.md`. 카드는 **unsigned** capability metadata 다.

### 2.7.11 러너 사다리 — A2A 위임의 **실행자 축**과 회전
→ `references/orchestration/a2a_relay.md`. 회전은 러너 평면 실패에서만 · 예산 사건 ✗ · 한 바퀴 소진 = 차단성 HITL.

### 캠페인 체인 — `references/campaign/`

캠페인 단계 간 정보는 아티팩트가 나른다. 배치·수명 `layout.md` · phase 전이 `phases.md` · 상태를 쓰는 손 `writers.md` ·
배정·전이 모드·감독 `assignment_supervision.md` · purge·종결 `purge.md`. 불변식은 `.claude/rules/workflow.md` §캠페인 아티팩트 체인.

## 3. 결정론 vs 판단 분리

결정론 = §공개 표면 표의 스크립트(스캔·게이트·렌더·계약 판정·그라운딩 누락 판정·체크섬). 판단 = 이 페르소나:
토폴로지 진입 인터뷰(§0.5) · fresh-clone 온보딩 능동제안 · 5-전제조건 인터뷰 · 사용자 승인 · 브랜치≠토폴로지 시 전환 안내 ·
ib_write_bw 오케스트레이션 · 호스트 안전체계 Y/N 설명·승인(§2.6) · manifest 기입 승인 · 전달(--provision) 승인 · 카나리 결과 판정 ·
인용의 진위 리뷰(§2.7.8) · 모호 시 중단·질의. 회귀 실행자는 `.claude/policies/runtime/verify_distribution.py`(스크립트 self-test 목록의 정본)이며
`agent_control.py --self-test` 는 `.claude/policies/runtime/runtime_selftest.py` 가 부른다.

## 4. 공개 표면 — 외부(다른 스킬·기초층·훅·서브 렌더 트리)가 부르는 진입점 전수

호출자 목록은 적지 않는다(파생값 — `antipattern_scan.py --tripwire` 의 `public_surface` 검사가 이 표 밖의 외부 호출을 막는다).
배달 = `render_sub_env.py` 가 서브 렌더 트리에 싣는지.

| 진입점 | 소유 § | 배달 | 비고 |
|---|---|---|---|
| `scripts/campaign_init.py` | 캠페인 체인 | ✓ | 캠페인 상태의 **유일한 writer**. 사람 승인 인자 필수 동사만 적는다: `--revise` · `--reentry-decide` · `--hint-approve`. 나머지는 `--help` |
| `scripts/campaign_template_validator.py` | 캠페인 체인 | ✓ | 선언·인스턴스 술어(P1~P3) |
| `scripts/manifest_contract.py` | §0.5 · §1S · §2.7.10 | ✓ | Flag·`issued_by: main` readiness 리더 |
| `scripts/node_role_contract.py` | §2.7.0 · §2.7.6 | — | 토폴로지 축 계약(sub_mode·rank·배달 평면)의 단일 소유자 |
| `scripts/scan_node.py` | §1.3–§1.5 · §2.7.10 | — | 스캔·게이트·3자일치·emit(서브 manifest 포함) |
| `scripts/render_sub_env.py` | §2.3 | — | 서브 환경·Card·서브 manifest 렌더 |
| `scripts/staleness_gate.py` | spine 1 · §0.5 | — | 조건부 preflight 트리거 |
| `scripts/bootstrap_canary.py` | §2.5 | — | model-less 카나리 실행자 |
| `scripts/relay.py` | §2.7.7a · §2.7.11 | — | 턴제 릴레이·감독 스텝·러너 사다리 |
| `scripts/turn_budget.py` | §2.7.7a | — | 턴 예산 선언 검증기 |
| `scripts/agent_control.py` · `scripts/providers/` | §2.7.7 | — | provider-neutral 전송 orchestrator · Claude CLI 구문의 유일 발행처는 `providers/claude_code.py` |
| `scripts/library_exchange.py` · `scripts/library_relay.py` | §2.7.8 | — | 그라운딩 교환 판정 · 사서 통로 |
| `scripts/node_blackbox/node_identity.sh` | §2.7.6 | ✓ | node_id 해소(실패하면 죽는다) |
| `scripts/node_blackbox/`(런타임: `blackbox_session.py` · `budget_renew_loop.sh` · `blackbox_eta.py` · `regen_envelope.py` · `blackbox_thermal.py` · `mem_watchdog_eta.sh` · `thermal_watchdog.sh` · `agent_guard.py`) | §2.6 | ✓ | `references/blackbox/host_safety.md` |
| `scripts/node_blackbox/`(설치: `install_node_blackbox.sh` · `verify_node_blackbox.sh` · `purge_host_safety.sh` · `publish_install_request.py`) | §2.6 · §2.6.1 | ✓ | HITL sudo |
| `.claude/skills/terraforming_node/scripts/host_safety/` | §2.6 | ✓ | 협역 워치독 정본 `mem_watchdog.sh` · 레거시 설치자(`install_host_safety.sh` — 정책 술어가 본문을 읽으므로 유지) |
| `scripts/topology_parity.py` | — | — | **shim** — 정본은 `.claude/policies/runtime/topology_parity.py`(기초층). 다음 브랜치 싱크에서 제거 |

## 5. 금지

- 사용자 승인 없는 자동스캔 / 서브노드 무단 프로빙. 서브 조사는 A2A·문서 회수로 한다.
- **HITL 없는 서브 work_dir 자동 신설**(§1.7). 성능(ib_write_bw) 미검증 멀티-ready 기입(fail-closed).
- 무증거 manifest 오버라이드 / topology 를 브랜치와 어긋나게 기입(3자-일치 위반).
- **빌딩블럭 스킬(terraforming_node·upstream-version-watch)·메인 manifest 를 서브에 전달 금지**(런타임블럭·렌더 산출물만 — §2.1/§2.3). 서브가 받는 것은 메인이 실측·발급한 **서브 manifest**(§2.7.10 · 설치 오버레이)뿐이다.
- **무증거 빈 정체성 렌더 금지**(render_sub_env.py 필수 필드 누락 시 fail-loud) · 템플릿에 IP·호스트 baking 금지(PII-free).
- **카나리(§2.5) 미통과 시 done 선언 금지** · 서브 워크스페이스 재스캔으로 "검증" 대체 금지(push-attestation 위반).
- SSH 키 교환·물리망 구성 대행(검증·가이드까지만).
- **에이전트의 무인 sudo 실행 금지** — 호스트 안전체계 설치(`install_node_blackbox.sh --apply`)의 실행 주체는 항상 사람(설명·승인·검증까지가 스킬 역할).

## 6. 참조

- 이관 전 원문(개정 이력 포함): `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` · 이력은 `git log -- .claude/skills/terraforming_node/`.
- 경계 재정리 근거: `docs/plan/plan_26093022_terraforming_node_헌법경계_계층화.md`.
