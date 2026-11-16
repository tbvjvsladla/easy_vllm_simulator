---
name: terraforming_node
description: >-
  **토폴로지-중립 진입(온보딩) 오케스트레이터** — fresh-clone/미테라포밍 환경에서 능동 발동해 **첫 동작으로
  토폴로지(① 단일노드 ② N대 멀티노드)를 인터뷰로 확정**하고, 그에 따라 노드를 스캔·manifest.yaml(스킬 간
  단일 계약)을 채운다. **single → 단일노드 온보딩**(manifest nodes:[] 기본; 서브 등록 시 A2A 에이전트 제어만 활성 — 빌드킷 배달 평면은 dormant) · **multi →
  서브노드 스캔·연결/성능 검증 + 서브 코드에이전트(Claude Code·Codex) 작업환경 구축**(메인 렌더→전달→discovery·실제 업무 권한 카나리).
  "이제 뭐해야해", "환경 셋업", "온보딩", "테라포밍", "노드 스캔", "단일/멀티 결정", "manifest 생성",
  "서브노드 셋업", "인터커넥트 점검", "서브 에이전트 환경 구축", "서브 코드에이전트 셋업" 같은 지시·fresh-clone 감지에 발동.
  **무단 스캔 금지** — 토폴로지 인터뷰 → (multi면 5-전제조건 인터뷰) → 사용자 승인 후 스캔. 토폴로지 미선언 시
  emit fail-closed. 성능 미검증 멀티 진행은 fail-closed로 차단.
when_to_use: >-
  온보딩·테라포밍 외에도 이 스킬이 절차 정본인 세 영역에서 부른다 — (1) 노드 블랙박스: 호스트 안전체계 Y/N,
  ETA·열·전력 워치독, docs/logs 데이터 평면, L3 수행지시서. (2) 노드 오케스트레이터: 메인↔서브 제어 평면,
  A2A 위임·턴제 릴레이·세션 재개·감독 스텝(--supervise-step), 러너 사다리 회전, 서고 접근자(서브 wiki-desk → 메인 __llm-wiki ssh 게이트웨이 발급·회수).
  (3) 캠페인 체인: campaigns/ 배치·phase 전이·campaign_init writer·배정(assignments)·purge 게이트.
  하네스 추가·전환·권한 개정도 이 스킬이 주관한다: 기존 승인 복구 → 하네스 설정(Codex는 main/sub 공통 권한) → 실행환경 검증 → 실제 A2A 검증.
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
- **When to invoke** — fresh-clone(미테라포밍) 감지 · HW/네트워크/manifest 변경 · `scripts/staleness_gate.sh` 가 preflight 필요를 반환할 때. 하네스/provider·권한·프로젝트 신뢰·실행환경이 바뀌면 HW 재스캔 없이 §2.5.2로 들어간다. **조건부 preflight** 이지 매 작업 상시단계가 아니다. 기존 유효 승인·캠페인·로그인을 다시 요구하지 않는다. 블랙박스·오케스트레이션·캠페인 절차가 필요할 때는 아래 §라우팅 표의 reference 로 간다.
- **Inputs** — 사용자 인터뷰 답(토폴로지·모델 획득 모드·multi 5-전제조건) · 노드 실측 스캔 · 현재 git 브랜치 · (선택) 이전 attestation.
- **Outputs** — `output/<topology>/manifest.yaml`(HITL 반영) · Flag attestation · (multi) `output/<topology>/sub_provision/` 스테이징 + 카나리 리포트.
- **State transitions** — 산출물은 `execution-approved` 의 **전제**(HW 사실·Flag)를 만든다. runtime-ready/evidence-complete/promotion-ready 판정은 `.claude/policies/runtime/completion_gate.py` 소유.
- **Deterministic commands** — 아래 §공개 표면 표가 전수다. 플래그의 단일 권위는 각 스크립트의 `--help` 다(여기 손으로 적지 않는다).
- **Handoff contract** — Flag 발급 → `upstream-version-watch`(컨테이너 빌드) → `vllm-recipe-explorer`(서빙전략). 서브 전달차는 `upstream-version-watch/scripts/sync_to_sub.sh` 단일 경로.
- **Worker/checkpoint contract** — 남은 작업·동일 단계 실측·소진 원인·서브 제안으로 native 실행 예산을 매번 편성하되 A2A 누적 호출 횟수로 캠페인을 끊지 않는다. 정상 예산 소진 뒤 상태 회수·재호출은 세션 리볼버의 역할이며 메인 자체가 워커가 되어도 같다. 호출 횟수는 이력으로 보존한다. 실제 권한·정체성·중복 실행·미해결 실패 가드는 유지한다.
- **Owns (state)** — `manifest.yaml`(메인 + **서브 manifest** §2.7.10) · `terraforming-flag` · `agent-card`(unsigned capability/discovery metadata) · `sub-agent-env` · `node-identity`(§2.7.6) · `topology-axis-contract`(§2.7.0) · `sub-control-plane`(§2.7) · `grounding-exchange`(§2.7.8) · 캠페인 체인 writer(`campaign_init.py`) · 블랙박스 데이터 평면(`docs/logs/<node_id>/`).

## Mandatory procedural spine

필수 순서다 — 앞 단계의 증거 없이 뒤 단계로 가지 않는다(무단 스캔·무증거 기입 차단이 이 순서에서 나온다).

1. **staleness 판정(조건부 preflight 진입)** — `bash .claude/skills/terraforming_node/scripts/staleness_gate.sh --topology <single|multi> --repo . --now <YYYY-MM-DD> [--observed <scan.json>]`. exit 0(`FRESH`)이면 HW 온보딩 재진입은 불요하다. 하네스/provider·권한·프로젝트 신뢰·실행환경 변경은 별도 축이므로 §2.5.2를 수행하고, 두 축 모두 변경이 없을 때 끝낸다. exit 4 면 reason code(`MANIFEST_ABSENT`/`FLAG_ABSENT`/`HW_DRIFT`/`ATTESTATION_STALE`)가 아래 단계의 착수 근거다.
2. **토폴로지 인터뷰**(§0.5.2) — 스캔보다 먼저. 미선언 시 emit fail-closed(§0.5.3), 브랜치 불일치 시 HITL 브랜치 전환(§0.5.4).
3. **모델 획득 모드 인터뷰**(§0.5.7) → **0차 init-plan 발행 + 자기-HITL 승인**(§0.5.8).
4. **스캔**(single=§1S · multi=§1.1 5-전제조건 인터뷰 → §1.2 사용자 승인 → §1.3 스캔 → §1.4 성능 게이트).
5. **게이트 + 3자-일치 단언**(§1.5) → 통과 시에만 **manifest + Flag 기입**(§1.6 / §1S, HITL).
6. **하네스 실행환경(main 및 등록된 sub)** — 역할별 업무 권한 → 렌더 → 공식 전달 → 노드별 Rust 설치·offline 빌드/기동 검사 → discovery·실제 권한 카나리(§2.3–§2.5.2). Rust는 `references/install/rust_runtime.md`의 배포 helper로 준비한다. **순서 의존**: 메인 자신의 Rust install → check → 정상 빌드(`build_rust.sh`·`build_rust_packages.sh`)는 `scan_node.py --emit-sub-manifest`·`render_sub_env.sh --sub-manifest|--self-test`·카나리 `discovery|permissions`·`relay.sh` 보다 **선행**한다(이들이 `manifest_contract`·`turn_budget` Rust 리더/브리지를 부른다). 새 클론의 진입(위 1–5: staleness·인터뷰·스캔·게이트·manifest+Flag)과 `rust_bootstrap.sh install|check` 는 어떤 ELF 도 요구하지 않는다 — Flag 가 없어도 온보딩에 들어갈 수 있고 검증기에 막히지 않는다. 표: `references/install/rust_runtime.md` §명령 순서와 ELF 의존. sudo 작업은 개정마다 생기는 것이 아니라 실제 호스트 전제조건 결손 때만 수행지시서로 발행한다. single의 등록된 A2A 서브도 이 단계에 포함하고 빌드킷 배달은 계속 dormant다. Rust 기동 검사·카나리 미통과 시 해당 하네스 배포 done 선언 ✗.
7. **호스트 안전체계 세션 최종 Y/N**(§2.6) — Flag 발급 **이후**의 독립 선택조항.


## Failure → reference routing

| 실패 신호 | 라우팅 대상 (정확 경로) |
|---|---|
| 재진입이 필요한지 불명(HW·manifest·attestation 변화 판정) | `.claude/skills/terraforming_node/scripts/staleness_gate.sh` |
| 스캔/게이트/3자-일치 blocked(비0 종료) · emit fail-closed | `.claude/skills/terraforming_node/scripts/scan_node.py` · `references/install/onboarding.md` |
| 서브 환경 렌더 실패(미치환 placeholder·필수 필드 누락) | `.claude/skills/terraforming_node/scripts/render_sub_env.sh` · `references/install/sub_agent_env.md` |
| 서브 위임/카나리의 provider 실행문법이 필요 | `.claude/skills/terraforming_node/references/orchestration/agent-control-adapter.md` |
| 하네스 전환·공통 Codex 권한·설정 로딩·실행 실패 | `references/install/harness_environment.md` · `templates/requests/codex_linux_sandbox.md` |
| Rust가 없는 신규 PC·Rust pin 변경·main/sub 컴파일/기동 준비 | `references/install/rust_runtime.md` |
| Flag 미발급이라 런타임 스킬이 info-only 로 떨어짐 | `.claude/skills/terraforming_node/scripts/manifest_contract.py` |
| 서브에 무엇을 해도 되는지 모호(저작/스캔/정비) · 평면 A/B 혼동 · 서브 git 교착 | §2.7 → `references/orchestration/control_planes.md` |
| "고쳤는데 안 갔다" / "안 고쳤는데 갔다"(커밋 vs 인덱스 vs 파일시스템) | §2.7.4 → `references/orchestration/control_planes.md` |
| `docs/logs/<node_id>` 경로가 노드마다 갈림 · hostname 이 경로에 샘 | §2.7.6 → `references/orchestration/topology_identity.md` |
| 싱글인데 멀티의 배달·버전동기 개념이 끼어듦 · `role: sub` 의 의미가 모호 | §2.7.0 → `references/orchestration/topology_identity.md` + `scripts/node_role_contract.py` |
| 서브 결정의 근거가 불명 · 인용 없는 빌드/서빙 결정 · 서브가 서고에 닿지 못함 | §2.7.8 → `references/orchestration/grounding_exchange.md` + `scripts/library_access.sh` |
| 위임이 소진·중단됨 · 재개 여부 · 러너(백엔드×모델) 실패 | §2.7.7a · §2.7.11 → `references/orchestration/a2a_relay.md` |
| 워치독 트립·사살 · 블랙박스 미설치/검증 실패 | §2.6 → `references/blackbox/host_safety.md` · `docs/logs` 평면은 `references/blackbox/logs_plane.md` |
| 캠페인 상태가 비었거나 어긋남 · purge 가 안 열림 · 다른 캠페인으로 넘어가야 함(보류·재개) | `references/campaign/` (layout · phases · writers · assignment_supervision · purge) |

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
→ `references/install/sub_agent_env.md` — ### 2.1 스킬 분류학 · ### 2.2 구축 아티팩트 · ### 2.3 렌더-온-메인 → 전달(인가 체인) · ### 2.4 A2A-개념 협업 계약 · ### 2.5 완료 게이트 — model-less 카나리 라운드트립 · ### 2.5.1 hint 발행 장착(single 서브 한정 · 클론·역할 선언·토큰 존재 확인 · 2026-10-11). **카나리 미통과 = done ✗.**

### 2.5.2 하네스별 실행환경·업무 권한 동등성

Rust 설치·노드별 검증·새 프로세스 실행은 `references/install/rust_runtime.md`가 소유한다.
각 노드의 프로젝트 관리 환경을 준비하며 HW 재스캔이나 별도 ready flag를 만들지 않는다.
→ `references/install/harness_environment.md`. Codex는 main/sub 모두 `default_permissions = ":danger-full-access"`와 `approval_policy = "never"`를 설치하며 역할별 named 프로필을 생성하지 않는다. 정체성·업무 소유는 manifest·SSH·노드 제어 계약으로 검증한다. 공통 업무·소유·승인을 provider별 실행 계약에 연결한다. 테라포밍이 설정·조건부 환경 복구 request·검증을 소유하며, 호스트 정책 설치는 **사람이 sudo로 수행**한다. 에이전트는 설치 요청을 발행하고 회수 증거를 판정한다. HW Flag와 독립된 하네스 검증이며 별도 비공식 ready flag를 발급하지 않는다.

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
→ `references/orchestration/a2a_relay.md` — 2.7.7a 턴제 릴레이와 자율 재개. **재개의 기본은 `relay.sh --supervise-step <camp> --apply` 자동 재발급**이다. 전진이 보여도 정본의 승인·중단·실패 복구 가드를 통과해야 한다. 사람 확인과 명시적 수동 복구 조건은 해당 reference를 따른다.

### 2.7.8 서고 접근자 — 서브 wiki-desk ↔ 메인 서고(같은 CLI · 경로만 다름)
→ `references/orchestration/grounding_exchange.md` — manifest `library` 접근자 · ssh 강제명령 게이트웨이 · 발급·회수(`library_access.sh` · HITL) · 그라운딩 게이트 대칭. **누락은 기계가 fail-closed 로 잡고 거짓은 사람이 리뷰한다.** (2026-10-19 · A2A 3메시지 교환 은퇴)

### 2.7.9 노드 오케스트레이터 — Phase 감독
→ `references/campaign/phases.md` · 특화층 오케스트레이션 전략은 `references/orchestration.topology.md`(브랜치별 내용 · 싱크 비전파).

### 2.7.10 Agent_Card v2 (A2A 1.0.1) · 서브 manifest
→ `references/install/sub_agent_env.md`. 카드는 **unsigned** capability metadata 다.

### 2.7.11 러너 사다리 — A2A 위임의 **실행자 축**과 회전
→ `references/orchestration/a2a_relay.md`. 회전은 러너 평면 실패에서만 · 예산 사건 ✗ · 한 바퀴 소진 = 차단성 HITL.

### 캠페인 체인 — `references/campaign/`

캠페인 단계 간 정보는 아티팩트가 나른다. 배치·수명 `layout.md` · phase 전이 `phases.md` · 상태를 쓰는 손 `writers.md` ·
배정·전이 모드·감독 `assignment_supervision.md` · purge·종결·수명 전이(보류·재개·포기·선점) `purge.md`. 불변식은 `.claude/rules/workflow.md` §캠페인 아티팩트 체인.

## 3. 결정론 vs 판단 분리

결정론 = §공개 표면 표의 스크립트(스캔·게이트·렌더·계약 판정·그라운딩 누락 판정·체크섬). 판단 = 이 페르소나:
토폴로지 진입 인터뷰(§0.5) · fresh-clone 온보딩 능동제안 · 5-전제조건 인터뷰 · 사용자 승인 · 브랜치≠토폴로지 시 전환 안내 ·
ib_write_bw 오케스트레이션 · 호스트 안전체계 Y/N 설명·승인(§2.6) · manifest 기입 승인 · 전달(--provision) 승인 · 카나리 결과 판정 ·
인용의 진위 리뷰(§2.7.8) · 모호 시 중단·질의. 회귀 실행자는 `bash .claude/policies/runtime/verify_distribution.sh`(Rust `.claude/rust/policy-distribution` · 스크립트 self-test 목록의 정본)이며
`bash .claude/skills/terraforming_node/scripts/agent_control.sh --self-test` 는 `.claude/policies/runtime/runtime_selftest.sh` 가 부른다.

## 4. 공개 표면 — 외부(다른 스킬·기초층·훅·서브 렌더 트리)가 부르는 진입점 전수

호출자 목록은 적지 않는다(파생값 — `antipattern_scan.py --tripwire` 의 `public_surface` 검사가 이 표 밖의 외부 호출을 막는다).
배달 = `render_sub_env.sh` 가 서브 렌더 트리에 싣는지.

| 진입점 | 소유 § | 배달 | 비고 |
|---|---|---|---|
| `scripts/campaign_init.py` | 캠페인 체인 | ✓ | 캠페인 상태의 **유일한 writer**. 사람 승인 인자 필수 동사만 적는다: `--revise` · `--reentry-decide` · `--hint-approve` · 수명 전이(`--init --suspend-current` · `--suspend` · `--resume-campaign` · `--abandon` · `--answer-query` · `--dispose` · `--sub-unconverged-accept`) · 배정 변경(`--assign-set`). 나머지는 `--help` |
| `scripts/selftest_campaign_lifecycle.sh` (Rust `rust/src/selftests/campaign_lifecycle.rs` · 원본 `scripts/selftest_campaign_lifecycle.py` 보존) | 캠페인 체인 | — | 수명 상태(보류·재개·포기·선점 · 서브 종결 전파 · 재배정) 자체검사 · 회귀 실행자 `verify_distribution.py` |
| `scripts/campaign_init.sh` | 캠페인 체인 | ✓ | 읽기 두 형태(`--active`·`--derive KIND …`)와 모델링한 쓰기 형태(`--init`·`--close`·`--phase-set`·`--cell-set`·`--write-brief`·`--assigned-cells`)를 공유 B 의 `campaign_init` 실행파일(노드에서 빌드)로 처리하는 런처 · 실행파일 부재/실행 불가는 어떤 경로 선택보다 먼저 exit 2 + 빌드 안내(명시 Python 지정이 있어도 Python 을 시작하지 않는다) · `CAMPAIGN_READ_VALIDATE_ONLY=1` 은 가용성 확인(값 1만 · 건강 증명 아님) · 다른 형태·명시 인터프리터·다른/읽을 수 없는 소스·미모델링 텍스트 프로필은 실제 `campaign_init.py` 로 가는 명시 호환 경로이며 업무 처리 전에만 정해진다 · 배달되는 것은 이 런처뿐이고 B 산출물은 B 를 받아 자체 빌드한 노드에만 있다(ray-worker 는 런타임 업무 0·호출자 없음) · 모델링하지 않은 쓰기 동사는 이 런처를 거쳐 `campaign_init.py` 로 간다(호출부는 `.sh` 하나만 부른다) |
| `scripts/campaign_template_validator.sh` | 캠페인 체인 | ✓ | `--acceptance INSTANCE`(합격 술어 P4·P5·P6)를 공유 B 의 `campaign_template_validator` 실행파일(노드에서 빌드)로 처리하는 런처 · 실행파일 부재/실행 불가는 exit 2 + 빌드 안내(명시 Python 지정이 있어도 Python 을 시작하지 않는다) · `CAMPAIGN_VALIDATOR_VALIDATE_ONLY=1` 은 가용성 확인 · `--campaign`·`--instance`·`--template`·`--selftest` 와 명시 인터프리터(`CAMPAIGN_VALIDATOR_PYTHON`)는 실제 `campaign_template_validator.py` 로 가는 명시 호환 경로 |
| `scripts/campaign_template_validator.py` | 캠페인 체인 | ✓ | 선언·인스턴스 술어(P1~P3) |
| `scripts/manifest_contract.py` | §0.5 · §1S · §2.7.10 | ✓ | Flag·`issued_by: main` readiness 리더 — **얇은 import 어댑터**: 기존 in-process importer(`render_sub_env`·`scan_node`·`render_dockerfile`·`harness_permission_probe`·`publish_install_request`·claim predicates·hint evidence)가 같은 이름·서명·상수(`VALID_MODES` 리터럴 · 원본 `__doc__`)로 import 하며, 판정은 `manifest_contract` 실행파일의 비공개 브리지(`MANIFEST_CONTRACT_BRIDGE`)가 한다(타입 전달·결과 계획 조립·정확한 예외 재구성만 · `_load_manifest` 는 원본처럼 호출 시점에 `import yaml`). U2(in-process traceback 프레임)·U3(지원 타입 집합 밖)은 열림 |
| `scripts/manifest_contract_compat_owner.py` | §2.7.10 | ✓ | 검토된 **원본 바이트(e84fa13c…)** 그대로 — 어댑터 `main` 이 **업무 전에 분류한** 레거시 형태(`-h`·`--self-test`·약어·사용 오류)와 CLI 의 명시 레거시 경로(공개 이름으로 실행)만 쓴다. 보존일 뿐 Python-free 주장이 아니다 |
| `scripts/manifest_contract.sh` | §0.5 · §1S · §2.7.10 | ✓ | `manifest_contract.py --topology T --repo R [--require-flag] [--json]` 게이트 판정을 공유 B 의 `manifest_contract` 실행파일(노드에서 빌드)로 처리하는 런처 · 독립 호출은 Bash → Rust(공개 `manifest_contract.py` CLI 는 `__main__` 이 같은 PID 로 Rust 에 **직접 exec** 하고 이 런처는 부재 안내·가용성 확인·exec 실패 경로의 단일 자리다) · 실행파일 부재/실행 불가는 어떤 경로 선택보다 먼저 exit 2 + 빌드 안내(Flag 미발급 4·manifest 부재 3 과 구별 · 명시 Python 지정이 있어도 Python 을 시작하지 않는다) · `MANIFEST_CONTRACT_VALIDATE_ONLY=1` 은 가용성 확인(값 1만 · 건강 증명 아님) · 정확한 문법(`--topology single\|multi`·`--repo VALUE`·`--require-flag`·`--json` 분리 토큰·임의 순서·각 1회) 밖의 형태(`-h`·`--self-test`·약어·`--x=y`·중복)·명시 인터프리터·다른/읽을 수 없는 소스·미모델링 텍스트 프로필은 실제 `manifest_contract.py` 로 가는 명시 호환 경로이며 업무 처리 전에만 정해진다 · 프로세스 안 import 사용자(render_sub_env·scan_node·harness_permission_probe·publish_install_request·render_dockerfile·hintlib·claim_predicates)와 `verify_distribution` 호출은 `.py` 를 그대로 쓴다(이관 전) — 그 호출(`manifest_contract.py` CLI)은 Python → Rust 직접 exec 이다 |
| `scripts/node_role_contract.py` | §2.7.0 · §2.7.6 | ✓ | 최초 Rust 빌드 전 bootstrap·기존 Python import 호환 진입점; native 역할 정책은 공유 `.claude/rust/role-contract` 라이브러리 |
| `scripts/node_role_contract.sh` | §2.7.0 · §2.7.6 | — | 설치·빌드 후 일반 Rust 역할 CLI 직접 기동; U 배달 서브의 직접 실행 경로는 `references/install/rust_runtime.md` |
| `scripts/check_sub_hint_equip.sh` (원본 `scripts/check_sub_hint_equip.py` 보존) | §2.5.1 | ✓(single) | single 서브 hint 발행 장착 판정·장착(`--clone`·`--write-push-authority`) · JSON 보고 · 비밀 미출력 · 사람 요청 템플릿 `templates/requests/sub_hint_push_equip.md` |
| `scripts/scan_node.py` | §1.3–§1.5 · §2.7.10 | — | 얇은 Python import 어댑터: P1 판정·조립과 지원되는 P2 probe API는 같은 공개 이름·서명으로 Rust 비공개 브리지를 호출한다. Python은 타입/객체참조·예외·I/O 전달을 맡고 probe 판정은 Rust가 소유한다. 일반 CLI도 `scan_node.sh` → 노드 로컬 Rust 실행파일로 기동한다. 원본 자기시험(`--self-test`와 argparse의 비모호 약어)은 업무 전 명시 호환 경로에서 정확한 `scan_node_original.py`를 실행하며, Rust 시작 후 Python 재시도는 없다. 미지원 getpass NSS 경계와 Python traceback 프레임 차이는 남아 있다 |
| `scripts/scan_node_original.py` | §1.3–§1.5 | — | 원본 소유자(바이트 동일 · sha256 `2d9e8a86…`). P1 밖 하드웨어 프로브·SSH/네트워크 수집·argparse `main`·`_self_test` 의 compat 대상이며 P1 네이티브의 증거가 아니다 |
| `scripts/scan_node.sh` | §1.3–§1.5 | ✓(벤치 owner가 가는 서브 한정) | 노드 로컬 `scan_node` Rust 실행파일 런처: P1/P2 브리지·일반 CLI·`--p1-self-check`를 제공한다. 배달 후 Sub의 기본 건강 검사는 P1 자체검사에 한정한다. 실행파일 없음/ELF 아님은 상태2와 빌드 안내이며 Python 경로는 없다. `SCAN_NODE_VALIDATE_ONLY=1`은 가용성만, `SCAN_NODE_OWNER_PATH`는 어댑터의 어휘적 출처다 |
| `scripts/render_sub_env.sh` (원본 `scripts/render_sub_env.py` 보존) | §2.3 | — | 서브 환경·Card·서브 manifest 렌더(Rust `terraforming_node/rust`) |
| `scripts/staleness_gate.sh` (원본 `scripts/staleness_gate.py` 보존) | spine 1 · §0.5 | — | 조건부 preflight 트리거 |
| `scripts/bootstrap_canary.sh` (원본 `scripts/bootstrap_canary.py` 보존) | §2.5 | — | model-less 카나리 실행자 |
| `scripts/harness_permission_probe.sh` (원본 `scripts/harness_permission_probe.py` 보존) | §2.5.2 | ✓ | 요청된 native 하네스 안에서 실행하는 업무 권한 관측; 자체 실행만으로 A2A 성공 선언 금지 |
| `scripts/rust_bootstrap.sh` | §2.5.2 | ✓ | 노드 로컬 Rust 설치·status·C 컴파일/아카이브와 Rust 연결 offline build/run 검사·명시 실행; pin은 동봉된 `rust_toolchain.pin` |
| `scripts/rust_toolchain.pin` | §2.5.2 | ✓ | Rust 버전·컴포넌트·지원 target 정본 |
| `scripts/build_rust_packages.sh` | §2.5.2 | ✓ | 명시 install/check 후 배달된 공유 Q·역할 정책 T·recipe P·upstream U 소스의 locked release 빌드와 순수 정상 ELF 기동; 일반 평가에서 자동 실행하지 않는다 |
| `scripts/selftest_rust_bootstrap.sh` | §2.5.2 | — | 네트워크 없는 설치/실패/재실행·환경 격리 계약 회귀 |
| `scripts/selftest_bootstrap_permissions.sh` (Rust `rust/src/selftests/` · 원본 `scripts/selftest_bootstrap_permissions.py` 보존) | §2.5.2 | — | 카나리 증거·음성대조 회귀 |
| `scripts/selftest_sub_hint_write_scopes.sh` (Rust `rust/src/selftests/` · 원본 `scripts/selftest_sub_hint_write_scopes.py` 보존) | §2.5.2 | — | 외부 hint 경로의 역할·대상·범위 검증 회귀 |
| `scripts/selftest_claude_native_events.sh` (Rust `rust/src/selftests/` · 원본 `scripts/selftest_claude_native_events.py` 보존) | §2.5.2 | — | opt-in native 도구 실행 기록과 기존 Claude 호출 호환 회귀 |
| `scripts/relay.sh` (원본 `scripts/relay.py` 보존) | §2.7.7a · §2.7.11 | — | 턴제 릴레이·감독 스텝·러너 사다리 |
| `scripts/turn_budget.py` | §2.7.7a | — | 하네스별 native-turn·wall-time 예산 선언 검증기. **얇은 import 어댑터**: CLI 는 `turn_budget.sh` 로 exec, `schema_cap`·`schema_max_turns`·`declare`·`REPO`·`REQUEST_SCHEMA` 는 기존 importer(relay·bootstrap_canary·selftest_harness_switch)용으로 같은 이름·서명을 유지하며 비공개 브리지로 Rust 를 부른다(업무 로직 ✗ · 타입 전달·정확한 예외 재구성·어휘적 출처만) |
| `scripts/turn_budget.sh` | §2.7.7a | — | `turn_budget` Rust 실행파일 런처(실행파일 없음/실행 불가 = 상태 2 + 빌드 안내 · Python 경로 없음 · `TURN_BUDGET_VALIDATE_ONLY=1` 은 가용성 확인일 뿐 업무 통과가 아니다 · `TURN_BUDGET_OWNER_PATH`=기본 스키마의 어휘적 출처) |
| `scripts/agent_control.sh` (Rust `rust/src/agent_control.rs` · 원본 `scripts/agent_control.py` · `scripts/providers/` 보존) · `rust/src/provider_claude_code.rs` · `rust/src/provider_codex_cli.rs` | §2.7.7 | — | provider-neutral 전송 orchestrator · native 실행 구문은 각 provider adapter만 발행 · v1 native-turn / v2 wall-time 계약 |
| `scripts/selftest_harness_switch.sh` (Rust `rust/src/selftests/harness_switch.rs` · 부분 이식 — 이식 안 된 dispatch 경로 사례는 모듈 문서에 열거 · 원본 `scripts/selftest_harness_switch.py` 보존) | §2.7.7 | — | 하네스 전환·예산·세션 결속·중복 실행·원장 경쟁 음성대조 |
| `scripts/library_access.sh` | §2.7.8 | — | 서브 서고 접근점 발급·회수(키·강제명령·manifest library 블록 · HITL) — `terraforming_node/rust` 의 `library_access` Rust 실행파일 런처(부재/ELF 아님 = 상태 2 + 빌드 안내 · Python 경로 없음) |
| `scripts/library_access.py` | §2.7.8 | — | 원본 보존(대조 golden `parity` s5 의 출처 · 정책 증거 원장 `claim_bindings.json` 의 evidence 경로) — 호출은 `.sh` 로 한다 |
| `scripts/audit_session_ssh.sh` | §2.7.7 | — | 세션 transcript 의 허가 통로 밖 ssh 감사(술어 A1) — `audit_session_ssh` Rust 실행파일 런처(부재 = 상태 2 + 빌드 안내 · Python 경로 없음) |
| `scripts/audit_session_ssh.py` | §2.7.7 | — | 원본 보존(대조 golden 의 출처) — 호출은 `.sh` 로 한다 |
| `scripts/agent_card_contract.sh` | §2.7.10 | — | Agent Card metadata 검증기(`validate --card` · `--self-test`) — `agent_card_contract` Rust 실행파일 런처(부재 = 상태 2 + 빌드 안내 · Python 경로 없음) |
| `scripts/agent_card_contract.py` | §2.7.10 | — | 원본 보존(대조 golden 의 출처) · Rust `render_sub_env` 는 이 계약을 in-process(Rust `agent_card`)로 든다 |
| `scripts/node_blackbox/logs_lifecycle.sh` | §2.6 | — | `docs/logs` 수명 집행 — `logs_lifecycle` Rust 실행파일 런처. 설치기는 같은 실행파일을 `easy-vllm-bb-lifecycle` 로 배치한다. 원본 `logs_lifecycle.py` 는 대조 golden 의 출처로 보존 |
| `scripts/node_blackbox/logs_lifecycle.py` | §2.6 | — | 원본 보존(대조 golden 의 출처) — 호출·설치는 Rust 실행파일로 한다 |
| `scripts/node_blackbox/seed_from_journal.sh` | §2.6 | — | Phase 0 수확기(레거시 저널 → `docs/logs/<node>/seed/`) — `seed_from_journal` Rust 실행파일 런처(부재 = 상태 2 + 빌드 안내 · Python 경로 없음) |
| `scripts/node_blackbox/seed_from_journal.py` | §2.6 | — | 원본 보존(대조 golden 의 출처) — 호출은 `.sh` 로 한다 |
| `scripts/node_blackbox/adversarial_stress.sh` | §2.6 | — | 방어층 시험용 가압기(벤치마커 ✗) — `adversarial_stress` Rust 실행파일 런처(평문 `http://` 엔드포인트 · 부재 = 상태 2 + 빌드 안내 · Python 경로 없음) |
| `scripts/node_blackbox/adversarial_stress.py` | §2.6 | — | 원본 보존(대조 golden 의 출처) — 호출은 `.sh` 로 한다 |
| `scripts/node_blackbox/node_identity.sh` | §2.7.6 | ✓ | node_id 해소(실패하면 죽는다) |
| `scripts/node_blackbox/`(런타임: `blackbox_session.sh` · `budget_renew_loop.sh` · `blackbox_eta.sh` · `regen_envelope.sh` · `blackbox_thermal.sh` · `mem_watchdog_eta.sh` · `thermal_watchdog.sh` · `agent_guard.sh` — `.sh` 는 Rust 실행파일 런처) | §2.6 | ✓ | `references/blackbox/host_safety.md` |
| `scripts/node_blackbox/`(설치: `install_node_blackbox.sh` · `verify_node_blackbox.sh` · `purge_host_safety.sh` · `publish_install_request.sh`) | §2.6 · §2.6.1 | ✓ | HITL sudo |
| `.claude/skills/terraforming_node/scripts/host_safety/` | §2.6 | ✓ | 협역 워치독 정본 `mem_watchdog.sh` · 레거시 설치자(`install_host_safety.sh` — 정책 술어가 본문을 읽으므로 유지) |

- 벤치 owner 의 build→native 자체검사가 원래 정본 경로에서 읽는 기초층 의존 3종(`topology_parity.sh` · `node_blackbox/node_identity.sh` · `scan_node.sh`)은 닫힌 목록 `render_sub_env.rs::SUB_BENCH_FOUNDATION_RUNTIME` 으로 adversarial-benchmark 가 가는 서브에만 배달된다. 오케스트레이터가 아니며 `.claude/runtime/node_blackbox/` 의 설치용 사본과는 다른 자리다.
- 서브 배달에는 `.py` 가 없다(plan_26111615) — 하네스 프로그램은 `.sh` 런처 + 그것을 빌드하는 Rust 크레이트 **소스**(경로 의존 폐포 포함 · `target/` 제외)로만 가고, 서브가 배달된 `build_rust_packages.sh`·adversarial-benchmark `build_rust.sh` 로 자기 아키텍처용 ELF 를 빌드한다. 런타임블럭 복제는 추적 `.py` 를 건너뛴다(`render_sub_env.rs::is_retired_py`).

## 5. 금지

- 사용자 승인 없는 자동스캔 / 서브노드 무단 프로빙. 서브 조사는 A2A·문서 회수로 한다.
- **HITL 없는 서브 work_dir 자동 신설**(§1.7). 성능(ib_write_bw) 미검증 멀티-ready 기입(fail-closed).
- 무증거 manifest 오버라이드 / topology 를 브랜치와 어긋나게 기입(3자-일치 위반).
- **빌딩블럭 스킬(terraforming_node)·메인 manifest 를 서브에 전달 금지**(런타임블럭·렌더 산출물만 — §2.1/§2.3). 서브가 받는 것은 메인이 실측·발급한 **서브 manifest**(§2.7.10 · 설치 오버레이)뿐이다.
- **무증거 빈 정체성 렌더 금지**(render_sub_env.sh 필수 필드 누락 시 fail-loud) · 템플릿에 IP·호스트 baking 금지(PII-free).
- **카나리(§2.5) 미통과 시 done 선언 금지** · 서브 워크스페이스 재스캔으로 "검증" 대체 금지(push-attestation 위반).
- SSH 키 교환·물리망 구성 대행(검증·가이드까지만).
- **에이전트의 무인 sudo 실행 금지** — 호스트 안전체계 설치(`install_node_blackbox.sh --apply`)의 실행 주체는 항상 사람(설명·승인·검증까지가 스킬 역할).

## 6. 참조

- 이관 전 원문(개정 이력 포함): `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` · 이력은 `git log -- .claude/skills/terraforming_node/`.
- 경계 재정리 근거: `docs/plan/plan_26093022_terraforming_node_헌법경계_계층화.md`.
