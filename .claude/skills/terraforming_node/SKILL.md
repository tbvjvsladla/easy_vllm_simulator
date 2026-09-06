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
---

# terraforming_node

**모든 환경 셋업의 토폴로지-중립 진입점**이다. vLLM 컨테이너·서빙 작업에 앞서 이 노드(들)의 환경 구체값
(토폴로지·노드·인터커넥트·NAS·CUDA)을 알아야 하고, 멀티면 서브노드 코드에이전트의 A2A 협업 작업환경도
있어야 한다. terraforming_node 는 **먼저 토폴로지를 인터뷰로 확정**(§0.5)한 뒤 분기한다:

- **single** → 단일노드 온보딩(§1S: 스캔 → `manifest.yaml`(nodes:[]) → 완료). 서브가 없으니 §1·§2 는 N/A.
  서브를 등록하면 **A2A 에이전트 제어**가 열리지만 멀티의 배달·버전동기 평면은 열리지 않는다(§2.7.0).
- **multi** → 진입 루틴(§1: 5-전제조건 인터뷰 + 스캔 + 연결/성능 검증) + **서브 에이전트 환경 구축**(§2: 메인 렌더 → 서브 전달 → 카나리).

이후 `upstream-version-watch`(컨테이너 빌드) · `vllm-recipe-explorer`(서빙전략) 와 **파이프라인 의존순서**로 협력한다(헌법 §스킬 오케스트레이션 / 진입 척추).

> **구현 범위 = 토폴로지 진입 인터뷰 + (single)단일 온보딩 + (multi)서브노드 진입·환경구축**(plan_26062311 진입·라이브 testlog_26062314 / plan_26062408 환경구축 / **plan_26063009_44_23 토폴로지-중립 재스코프**).
> 환경탐지 전반·하위스킬 호출 오케스트레이션은 점진 확장(헌법 §스킬 경계).

> 설계 원칙(하네스 엔지니어링): **스캔·게이트·렌더·일치단언은 결정론 스크립트**(`scripts/scan_node.py`·`scripts/render_sub_env.py`),
> **인터뷰·승인·HITL 판정은 페르소나(이 문서)**. 둘을 섞지 않는다.

## Contract

- **Goal** — 토폴로지를 인터뷰로 확정하고 노드 사실을 스캔해 `output/<topology>/manifest.yaml` + 테라포밍-완수 Flag 를 발급한다(멀티면 서브 에이전트 작업환경까지).
- **When to invoke** — fresh-clone(미테라포밍) 감지 · HW/네트워크/manifest 변경 · `scripts/staleness_gate.py` 가 preflight 필요를 반환할 때. **조건부 preflight** 이지 매 작업 상시단계가 아니다.
- **Inputs** — 사용자 인터뷰 답(토폴로지·모델 획득 모드·multi 5-전제조건) · 노드 실측 스캔 · 현재 git 브랜치 · (선택) 이전 attestation.
- **Outputs** — `output/<topology>/manifest.yaml`(HITL 반영) · Flag attestation · (multi) `output/multi/sub_provision/` 스테이징 + 카나리 리포트.
- **Mandatory procedural spine** — 아래 §Mandatory procedural spine 의 7단계(순서 고정).
- **State transitions** — 산출물 자체는 상태가 아니라 `execution-approved` 의 **전제**(HW 사실·Flag)를 만든다. runtime-ready/evidence-complete/promotion-ready 판정은 `.claude/policies/runtime/completion_gate.py` 소유.
- **HITL/safety boundaries** — 무단 스캔 ✗ · 무증거 manifest 기입 ✗ · 서브 work_dir 자동 신설 ✗ · 에이전트 무인 sudo ✗(§5 금지).
- **Failure → reference routing** — 아래 §Failure → reference routing 표(증상 → 정확 경로).
- **Deterministic commands** — `scripts/staleness_gate.py`(조건부 preflight 트리거) · `scripts/scan_node.py`(스캔·게이트·3자일치·emit · **`--emit-sub-manifest`**) · `scripts/render_sub_env.py`(서브 환경 렌더 · 카드 서명 · 서브 manifest 배달) · `scripts/manifest_contract.py`(Flag 리더) · **`scripts/agent_card_contract.py`**(Agent_Card v2 계약·JWS 서명/검증).
- **Handoff contract** — Flag 발급 → `upstream-version-watch`(컨테이너 빌드) → `vllm-recipe-explorer`(서빙전략). 서브 전달차는 `upstream-version-watch/scripts/sync_to_sub.sh` 단일 경로.
- **Owns (state)** — `manifest.yaml`(메인 + **서브 manifest** §2.7.10) · `terraforming-flag` · `a2a-delegation-key` · **`agent-card`(v2 · 서명키)** · `sub-agent-env` · **`node-identity`**(§2.7.6 role+rank 스킴) · **`topology-axis-contract`**(§2.7.0 sub_mode·배달 평면 판정) · **`sub-control-plane`**(§2.7 평면 A/B·3범주·B0–B3·권위 평면·A2A 제어명령) · **`grounding-exchange`**(§2.7.8 claim/reference/citation)

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
| 스캔/게이트/3자-일치 blocked(비0 종료) · emit fail-closed | `.claude/skills/terraforming_node/scripts/scan_node.py` |
| 서브 환경 렌더 실패(미치환 placeholder·필수 필드 누락) | `.claude/skills/terraforming_node/scripts/render_sub_env.py` |
| 서브 위임/카나리의 provider 실행문법이 필요 | `.claude/skills/terraforming_node/references/agent-control-adapter.md` |
| Flag 미발급이라 런타임 스킬이 info-only 로 떨어짐 | `.claude/skills/terraforming_node/scripts/manifest_contract.py` |
| 서브에 무엇을 해도 되는지 모호(저작/스캔/정비) · 평면 A/B 혼동 · 서브 git 교착 | 이 문서 **§2.7 노드 제어 규약**(정본) |
| "고쳤는데 안 갔다" / "안 고쳤는데 갔다"(커밋 vs 인덱스 vs 파일시스템) | 이 문서 **§2.7.4 권위 평면 계약** |
| `docs/logs/<node_id>` 경로가 노드마다 갈림 · hostname 이 경로에 샘 | 이 문서 **§2.7.6 node-identity** |
| 싱글인데 멀티의 배달·버전동기 개념이 끼어듦 · `role: sub` 의 의미가 모호 | 이 문서 **§2.7.0 토폴로지 분기** + `scripts/node_role_contract.py` |
| 서브 결정의 근거가 불명 · 인용 없는 빌드/서빙 결정 | 이 문서 **§2.7.8 그라운딩 교환** + `scripts/library_exchange.py` |

## 0. 전제 / 입력
- **토폴로지-중립 진입**: single·multi **공통** 발동. (과거 "multi 전용·single 비활성(α)"는 plan_26063009_44_23 에서 폐기 — single 진입점 부재 = chicken-and-egg 갭이었음: "single이냐 multi이냐"를 묻는 주체가 multi일 때만 발동했음.) **첫 동작 = 토폴로지 인터뷰(§0.5)**. 이하 §1(진입 루틴)·§2(서브 환경구축)는 **topology=multi 분기**, single 은 §0.5→§1S 로 짧게 완결.
- **SSH = Case A**(multi 한정): 메인↔서브 패스워드리스 SSH는 **사전조건**(검증만, 키 교환·물리망 설정은 안 함).
- 계약 스켈레톤 = `manifest.template.yaml`(루트, 추적). **실값 = `output/<topology>/manifest.yaml`**(브랜치 파생 통로 — single=`output/single/`·multi=`output/multi/`; 비추적, plan_26062315). **(multi) 서브엔 manifest 를 전달하지 않는다**(메인 단일계약 — render-on-main, D10).
- 검증 정본 = `devlog_250422`(git f3583f0, in-history) 실측 + seed PDF(repo-외부).

## 0.5 토폴로지 진입 인터뷰 게이트 (판단 — **첫 동작**, fail-closed)

> **chicken-and-egg 차단(plan_26063009_44_23 D2·D3·D4)**: "이 HW가 single이냐 multi이냐"는 **스캔보다 먼저 인터뷰로 결정**한다. 브랜치는 작업공간 선택기일 뿐 토폴로지를 *결정*하지 않는다(브랜치⇒토폴로지 추론 금지 — 이게 갭의 직접원인이었음).

- **0.5.1 fresh-clone 능동 발동**(헌법 §스킬 오케스트레이션 척추): 미테라포밍 신호 = `config.yaml` 부재 **AND** `output/single/manifest.yaml` 부재 **AND** `output/multi/manifest.yaml` 부재. 감지 시 일반 오리엔테이션("뭐 할까요?")보다 **온보딩을 능동 제안**한다. 단 능동성 고도 = **제안**(감지→제안→사용자 응답→인터뷰→승인→스캔) — 자동 스캔 ✗("무단 스캔 금지" 보존).
- **0.5.2 토폴로지 인터뷰**(가장 먼저): *"이 HW 환경은 ① 단일노드(이 머신 1대)인가, ② N대 PC를 고속망으로 묶은 멀티노드(DGX Spark식 병렬)인가?"* 사용자 선언이 `scan_node.py --topology <답>` 의 `declared` 입력이 된다.
- **0.5.3 fail-closed(D3)**: 토폴로지 미선언 시 스캔/emit 금지. 결정론 백스톱 = `scan_node.py --emit-manifest` 가 `--topology auto`면 **거부(비0 종료 3 — `emit_gate`, --self-test 회귀)**. 추정 토폴로지로 manifest 기입 불가.
- **0.5.4 브랜치 ≠ 토폴로지(D4)**: 선언 토폴로지가 현재 git 브랜치와 어긋나면(예: `single-node` 브랜치인데 "multi" 선언) → `evaluate_gate` 3자-일치 단언이 **fail-closed(blocked·비0)** + **HITL 브랜치전환 안내**(`git checkout <single-node|multi-node>` 후 재개 — 스크립트 자동전환 ✗: 워킹파일을 바꾸는 행위라 사람이 한다). 정렬 후 진행.
- **0.5.5 분기**: **single** → §1S 단일노드 온보딩(짧음) · **multi** → §1 멀티노드 진입 루틴(5-전제조건 인터뷰부터).
- **0.5.6 드리프트 가드(D7, 경량)**: 온보딩 단계상태를 추적한다 — `토폴로지 결정 → 모델획득모드 → 0차 init-plan → 스캔 → 게이트 → manifest+Flag → 호스트 안전체계(세션 최종 Y/N·선택)`(single) / `+ 5-전제조건 → 성능 → manifest+Flag → 서브 환경구축 → 카나리 → 호스트 안전체계(세션 최종 Y/N·선택)`(multi). **호스트 안전체계는 Flag 발급 이후 세션 최종 선택조항**(§2.6 — 표준 절차 완수와 분리, plan_26071115). 사이드퀘스트(예: GPU/드라이버 디버깅) 후 **미완 단계로 복귀**(미완을 사람 머릿속에만 두지 않음). 범용 워크플로 todo 시스템은 범위 밖(헌법 파킹).
- **0.5.7 모델 획득 모드 인터뷰 (토폴로지 직후 · 헌법 §모델 획득 모드 따름정리 3종 · plan_26063018)**: *"모델을 어떻게 확보하나?"* — **managed**(사전 다운로드된 관리 NAS 경로 read-only 마운트) · **ephemeral**(컨테이너 내부 HF 캐시 임시 다운로드, 컨테이너 down→삭제 · **다수 기본**) · **custom**(지정 경로 저장·볼륨마운트). scan 이 `nas_model_path` 존재를 bool 탐지해 *"아마도 managed"* **제안**(사실=탐지·제안=판단). 답 → manifest `model_source`(+ `nas_model_path`/`custom_model_paths`/`hf_token_env_file` 포인터). **이 인터뷰 없으면 `manifest_contract` 가 info-only 유지**(Flag complete 만으론 불충분 — model_source valid 이중요건). 토폴로지-무관(single·multi 공통).
- **0.5.8 0차 init-plan 발행 (스캔 前 · "로그=에이전트" 철학 · 헌법 계획 게이트)**: 스캔 착수 전, 에이전트가 인터뷰 답에서 **real `docs/plan/` init-plan 을 *대신 초안***(토폴로지·획득모드·스캔할 HW·branch 정합·완료 시 Flag) → **배포자 자기-HITL 승인**(*불편하지 않게 유도* — 무게가 아니라 경험; 약간의 강요는 의도된 철학). 이 plan 은 `wiki-desk` 가 색인(자기개선 루프 *자동 기둥*) → 배포자가 Agent 를 능숙히 다루는 *수동 기둥* 습관화. 승인 후 §1S/§1.3 스캔.
- **0.5.9 훅 배선 (클론 로컬 · 토폴로지 중립 · 1회)**: `git config core.hooksPath .claude/hooks`. 훅 파일(`.claude/hooks/pre-commit`)은 추적 빌딩블럭이라 클론에 실리지만 **이 설정은 `.git/config` 라 실리지 않는다** — 설정하지 않으면 커밋 병목의 tripwire 가 배포본에서 한 번도 돌지 않는다("만든 것과 도는 것은 다르다"). 확인: `python3 .claude/policies/runtime/runtime_selftest.py --tripwires-only` 가 WARN 없이 `[tripwire] PASS` 를 낸다(미배선이면 WARN, FAIL 아님).

## 1S. 단일노드 온보딩 (topology=single — 짧은 경로)

> §0.5 에서 single 확정 시. 서브가 없으므로 §1(5-전제조건/성능)·§2(서브 환경구축)는 **N/A**.

- **스캔**(결정론): `python3 scripts/scan_node.py --topology single` — interconnect 검증 skip(=α 정상).
- **게이트**: α — RoCE 하드웨어가 있어도 비blocking 경고(멀티 가능 머신의 단일 운용은 정상).
- **manifest 기입**(HITL): `--emit-manifest --topology single` 블록(YAML-valid · **single 시 `nodes: []` 도 결정론 emit** — dormant 게이트 동결, 수기 의존 ✗) → **사람 확인 후** `output/single/manifest.yaml` 반영. `nodes: []` → **서브 미등록 dormant**(독립 self-containment 보존). ⚠ dormant 의 정확한 뜻은 "서브가 없다"이지 "서브를 등록하면 안 된다"가 아니다 — 등록 시 열리는 것은 **A2A 에이전트 제어**뿐이고 빌드킷 배달 평면은 여전히 dormant 다(§2.7.0 · 헌법 §single-node 확장기능). **무증거 기입 금지.**
  - emit 블록은 **테라포밍 완수 Flag attestation**(`terraforming.complete/branch_verified`)을 §1.5 3자일치 통과 시에만 포함(보수적·미통과면 미발급) — **§0.5.7 `model_source` 도 함께 기입**해야 `manifest_contract` Flag valid(complete + valid model_source 이중요건). Flag 발급 = 3 런타임 스킬 작업 활성(헌법 §테라포밍-완수 Flag 게이트).
  - **Flag ↔ 호스트 안전체계 명시 분리**(plan_26071115): Flag 발급은 표준 절차 완수이며 **안전체계 설치와 무관**(안전체계 미설치여도 Flag valid). 안전체계는 이 manifest+Flag 기입 **이후** §2.6 세션 최종 Y/N 선택조항에서 다룬다.
- **호스트 안전체계 세션 최종 Y/N** → **§2.6**(선택조항, 양 토폴로지 공통) 수행 후 완료.
- 온보딩 완료 → 파이프라인 다음 단계(`upstream-version-watch` 컨테이너 빌드).

## 1. 멀티노드 진입 루틴 (topology=multi — §0.5 에서 multi 확정 후)

> 핵심 불변식: **무단 자동스캔 금지** — "인터뷰 → 사용자 승인 → 스캔" 순서. **성능 미검증 멀티 진행 금지(fail-closed)**.

### 1.1 5-전제조건 인터뷰 (판단 — 사람에게 묻는다)
자동스캔 전에 멀티턴으로 확인:
①메인↔서브 **고속 RDMA 인터커넥트(예: ConnectX-7)** 연결 ②**네트워크** 구성 완료 ③메인↔서브 **SSH**(Case A) 구성 완료
④서브노드 **코드에이전트 CLI 설치** ⑤서브 코드에이전트 **모델 연결**(로그인/API key — `probe(reachable)` 실제 도달).
provider 별 실행문법은 `references/agent-control-adapter.md` 에서만 해소한다(본문 inline ✗).

### 1.2 사용자 승인 게이트 (판단)
인터뷰 응답 수집 → 사용자가 **명시 승인**해야 스캔 시작.

### 1.3 스캔 (결정론 — `scripts/scan_node.py`)
- **라이브 멀티 명령(정본)**: `--peer-ip` 와 `--peer-ssh` 는 **서로 대체 불가한 별개 플래그**이며 **둘 다** 필요하다.
  ```bash
  python3 scripts/scan_node.py --topology multi \
      --peer-ip <sub IP> --peer-ssh <user>@<sub> --sub-work-dir <서브 프로젝트 절대경로> \
      --check-egress --model-source <managed|ephemeral|custom> \
      [--bandwidth-gbps <합산 실측> --per-port-gbps <포트당 실측>] [--emit-manifest]
  ```
  - `--peer-ip` 만: 게이트는 통과하지만 **서브 HW 동질성 검증이 통째로 생략**돼 `hw_verified` 미발급 → A2A 위임 키가 영구히 안 나온다(서브 info-only).
  - `--peer-ssh` 만: `nodes[]` 자체가 만들어지지 않고 도달성 미검증으로 **γ blocked(exit 2)**.
  - 2026-09-03 이전에는 이 조합이 저장소 어디에도 적혀 있지 않았다(B1) — 두 실패 모두 조용했다.
- 단일: `python3 scripts/scan_node.py --topology single [--compose output/single/docker-compose.yaml]`
- 탐지: cpu_arch(uname) · cuda(nvcc) · gpus(nvidia-smi) · interconnect(**/sys/class/infiniband + show_gids** — ibstat 비의존) · docker-compose NCCL 교차검증.
- 폴백: 탐지 도구 부재/빈 결과 → graceful(인터뷰 폴백 또는 α). hard-crash 금지.

### 1.4 성능 게이트 (결정론 판정 + cross-node 오케스트레이션)
- 합격선(plan §2.4): **포트당 ≥100 Gb/s & 합산 ≥180 Gb/s**(=200Gbps 풀대역폭 ~90%, devlog 218 기준).
  두 조건 모두 `evaluate_gate` 가 집행한다(`--per-port-gbps`/`--per-port-floor` · 2026-09-03 B4 이전에는
  **합산만** 코드에 있어 포트당 조건이 집행 불가였다). **미측정은 통과가 아니다** — `bandwidth_gbps` 가
  없으면 `pending-perf` + **exit 2**(fail-closed). 미평가 축은 `per_port_evaluated:false` 로 음성정직 표기된다.
  **이 기본값은 200Gbps RoCE 플랫폼 파생 상수** — 저속-그러나-가용 인터커넥트(예 100GbE)는 불가 판정이
  아니라 `--bw-floor <합산 line-rate×0.9>` 재설정 대상(전방호환 시도-우선 따름정리 — 시도 차단 금지;
  단 낮춘 합격선은 분산서빙 성능 기대치도 비례 하향됨을 HITL 에 고지).
- 측정 = `ib_write_bw`(서버 on 서브, 클라 on 메인). **견고 패턴**(원격 detach 함정 회피):
  ```bash
  ssh <sub> 'ib_write_bw -d <hca> -F' >/tmp/srv.log 2>&1 &   # 로컬에서 SSH 백그라운드(서버는 서브 foreground 유지)
  sleep 3; ib_write_bw -d <hca> -F <sub_RoCE_IP>             # 클라(메인) — BW average[MB/sec] ×8/1000 = Gb/s
  ```
  도메인별 측정 → 합산. 결과를 `scan_node.py --bandwidth-gbps <합산>` 으로 주입해 게이트 최종판정.

### 1.5 토폴로지 게이트 + 3자-일치 단언 (결정론 — `scan_node.evaluate_gate`)
> **토폴로지 무관 공통**: α(single) 분기·3자-일치 단언은 §1S 단일 경로도 사용한다 — 본 절은 §1(multi) 아래 있으나 게이트 로직 자체는 양 토폴로지 공통(single 독자도 참조).
- **α**(topology=single): interconnect 검증 skip(정상). RoCE 하드웨어 존재해도 비blocking 경고.
- **multi-ready**(topology=multi): RoCE 존재 + peer 도달 + 대역폭 합격선 → ready.
- **γ fail-closed**(multi): RoCE 부재 / peer 미도달 / 대역폭 미달 → **멀티-ready manifest 미생성 + blocked + 비0 종료**.
- **3자-일치 단언**: `git branch ⇒ topology` ↔ `output/<topology>/manifest.yaml` ↔ scan(인터커넥트 유무) 불일치 시 blocked.

### 1.6 manifest 기입 (HITL)
검증 통과 시 `scan_node.py --emit-manifest` 가 topology+interconnect 블록 산출 → **사람 확인 후** `output/multi/manifest.yaml` 반영. **무증거 기입 금지.**

### 1.7 서브 work_dir 프로비저닝 게이트 (HITL — R2 불변식, plan_26062320)
서브에 산출물을 복제하려면 서브 작업경로(`nodes[role=sub].work_dir`)가 있어야 한다. **기본값 = 메인 work_dir 와 동일**. 그러나:
- **경로 신설은 반드시 HITL** — `sync_to_sub.sh --apply` 는 서브 work_dir 부재 시 정지·질의(exit 5). 사람 승인(`--provision`) 시에만 `mkdir -p` 후 전송.
- **HITL 없는 자동 경로 신설 절대 금지.** 경로값은 manifest 에서만 해소(하드코딩 금지).

## 2. 서브 에이전트 환경 구축 (A2A-개념 — plan_26062408)

> 서브노드 코드에이전트가 메인과 **불투명 피어**로 협업하려면, 서브 워크스페이스에 **페르소나·능력·권한·
> 런타임스킬·통신프로토콜**이 있어야 한다. 진입 루틴(§1)이 manifest 를 채운 뒤, 이 단계가 그 환경을
> **메인에서 렌더해(render-on-main) 서브로 전달하고 카나리로 검증**한다.

### 2.1 스킬 분류학 (결정론↔자율성 충돌 해소)
- **빌딩블럭**(메인 전용, 서브 전달 ✗): `terraforming_node`. 온보딩·노드 계약·서브 정체성 판정은
  영구히 메인 단독이다(서브 자가스캔 ✗).
- **`upstream-version-watch` 는 스킬 전체가 아니라 경로 단위로 갈린다**(2026-09-03 개정 · `plan_26090317` P2).
  한 스킬 안에 성질이 다른 둘이 섞여 있었고, "전체를 주느냐 마느냐" 로 물으면 어느 답도 옳지 않았다:
  - **서브에 간다(a2a-agent 만)** — 해소·렌더 능력: `resolve_*`·`render_dockerfile`·`regen_requirements`·
    `classify_failure`·`check_smoke_model`·`references/*`. 싱글 서브가 "모델+엔진 핀을 받아 자율 빌드→서빙→벤치"
    를 하려면 이것이 있어야 한다(사용자 범위 선언).
  - **서브에 가지 않는다(전 모드)** — 노드 간 오케스트레이션: `sync_to_sub.sh`·`sync_branches.sh`·
    `fetch_sub_docs.sh`·`smoke_clone.sh`·`multinode_*_smoke.sh`. 이것들은 **메인이 서브를 향해** 쓰는
    도구다. 서브가 들면 배달 방향이 뒤집히고(§2.7.1 권한 평면), 서브가 다른 노드를 향해 쓰는 경로가 생긴다.
  - 정본 = `render_sub_env.RUNTIME_BLOCK_EXCLUDES` (닫힌 목록 · 자체검사가 새 스크립트의 미분류를 fail-loud).
- **ray-worker 서브는 런타임 스킬 0종**이다 — 정본(Dockerfile·compose·serve_runner)을 재현하는 워커이지
  전략을 세우는 주체가 아니다. 판정 정본 = `node_role_contract.tool_plane`.
- **런타임블럭**(서브 복제 ✓): `vllm-recipe-explorer`. 서브가 **동일 결정론 엔진**을 자기 모델에 자율 실행 → 자율=실행 주체, 방법=결정론(헌법 "확률론 추론 금지" 보존).

### 2.2 구축 5+아티팩트 (서브 워크스페이스 레이아웃)
| 경로(서브 루트) | 내용 | 전달타입 |
|---|---|---|
| `CLAUDE.md` | 페르소나(Karpathy B1–B4, **노드정체성 bake**, 자율 triplet 저작 + 자기교정) | 렌더(gitignored) |
| `Agent_Card.json` | A2A 능력카드(skills[]·노드정체성) | 렌더(gitignored) |
| `.claude/settings.local.json` | **스코프드** 권한(블랭킷 ✗) | 렌더(gitignored) |
| `.claude/skills/vllm-recipe-explorer/` | 런타임블럭(git-tracked만) | 복제 |
| `.claude/rules/comms.md` | 통신 정적계약 | 복제 |
| `.claude/rules/docs.md` | 문서발행 규약(D12 — 서브 동일 규약 발행 → 상향 문서기반 회수) | 복제(메인 `.claude/rules/docs.md`) |
| `.claude/schemas/task-report.schema.json` | 자기검증 스키마 | 복제 |
| `.gitignore` | 서브 로컬 git 추적규칙(D12 — docs persist·생성물 무시) | 복제(`sub_node/gitignore.template`) |
| `docs/{plan,devlog,testlog,simlog}/example.md` | 발행 스켈레톤(D12 — 서브 insight 문서) | 복제(메인 docs/*/example.md) |
| `campaigns/_template/**` · `campaigns/_bootstrap/relay/` | 캠페인 뼈대 + 릴레이 원장 스캐폴드(파일=세션) | 메인 뼈대 복제 + 빈 디렉토리 |

추적 템플릿·정적자산은 `sub_node/`(CLAUDE.template.md·Agent_Card.template.json·settings.local.template.json·comms.md·task-report.schema.json·gitignore.template) — **PII-free**(IP·호스트 비박음, 렌더 시 manifest 에서 치환). (`docs.md`·`docs/*/example.md` 는 메인 정본을 D12 복제 — sub_node/ 외부 원천.)

### 2.3 렌더-온-메인 → 전달 (결정론 + HITL)
- **렌더**(결정론): `python3 scripts/render_sub_env.py --topology multi` → manifest 노드정체성을 템플릿에 치환,
  gitignored 스테이징 `output/multi/sub_provision/` 산출(서브 루트 미러). 미치환 placeholder·필수 누락 시 fail-loud.
- **전달**(HITL) — **인가 체인이 먼저다**. `sync_to_sub.sh` 는 `--mode` 와 `--manifest` 를 **필수**로 받고,
  `completion_gate.py authorize --action sync_to_sub` 로 포워딩해 **discovery·ssh·rsync 이전에** fail-closed 한다.
  2026-09-03 이전에는 이 문서·렌더러 출력·스크립트 자기안내 **세 곳 모두** 그 두 인자를 빠뜨려, 안내대로 치면
  `CLI_USAGE_ERROR` 로 거부됐다(B0).

  ```bash
  # ① 증거 레코드 발행(work-manifest 의 뼈대)
  python3 .claude/policies/runtime/evidence_publisher.py init       --task-class harness_change --topic <주제>       --generated-utc <YYYY-MM-DDTHH:MM:SSZ> --identity-json <identity.json>
  # ② 사람이 plan 문서에 `## Execution approval` 앵커 + 원자 3종을 적고(approved_by/approved_at_utc/allowed_action),
  #    그 값을 work-manifest 의 execution_approval 에 기입한다(plan_sha256 = 그 plan 바이트의 sha256).
  #    → allowed_actions 에 `sync_to_sub` 가 있어야 인가가 열린다.
  # ③ 전달
  bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh       --mode experimental --manifest <work-manifest.json> --apply --provision --branch <multi|single|both>
  ```
  — 메인 rsync(코드) **이후** 스테이징을 서브 루트로 **오버레이(--delete 없음)**. 빌딩블럭 스킬·manifest 는 전달 안 됨(런타임블럭만).
  - `--mode promotion` 은 verify 가 `promotion-ready` 에 도달한 경우에만 열린다(hint·last-good 평면). 서브 배달은 통상 `experimental`.
- **단일 전달차**: 코드+에이전트환경 모두 sync_to_sub.sh 한 경로. dry-run 기본 → 사람 검토 후 --apply.

### 2.4 A2A-개념 협업 계약 (서버 없음)
- 메인=client(Task 발급·리포트 검증·피드백) · 서브=remote(자율 수행·자기검증 리포트 1개). A2A 어휘 차용, HTTP 서버 ✗(전송=SSH 단발 `delegate(task)` — provider 문법은 `references/agent-control-adapter.md`).
- **검증 = push-attestation**: 서브가 self-verification(config-parse·schema·runner 문법·checksum·**로컬 스모크**)을 리포트에 담아 회신 → **메인은 리포트만 검증, 서브 워크스페이스 재스캔 ✗**.
- **성공술어**: phase 별(comms.md). 예: config = triplet 생성 + 로컬 스모크 응답("린트 통과 ≠ 서빙됨").
- **상태=파일**: `campaigns/<camp-id>/relay/<context_id>.json`("파일=세션" · 2026-09-06 루트 `tasks/` 에서 이관 · 활성 캠페인 부재 시 `_bootstrap`). 턴 예산은 **메인이 매 attempt 선언한다**(`--max-turns`·`--timeout-seconds`·`--budget-source`) — 옛 `max-turns=3`(2026-09-03 폐기)에 이어 그 대체물이던 **grade 표도 2026-09-05 폐기**됐다(표가 실측 없이 정본 행세를 했고 교정 소비자가 0 이었다 · `audit_26090515` G-A2). `scripts/turn_budget.py` 는 이제 선언을 **검증**만 한다(상한은 요청 스키마에서 읽는다). 소진은 terminal 이고 다음은 **더 큰 예산의 새 attempt** 이며, 그 이어붙이기는 `scripts/relay.py --continue` 가 **본문을 조립**한다 (사람은 답·승인만 — §2.7.7a).
- per-task 휘발값(모델명·예산·NAS 서브디렉토리)은 **Task Message** 로(manifest 복제 아님).

### 2.5 완료 게이트 — model-less 카나리 라운드트립 (R1 정합)
전달 후 메인이 **모델 없이** 부트스트랩 Task 1회. **실행자 = `scripts/bootstrap_canary.py`**(2026-09-03 신설 · S1):
manifest 의 `nodes[sub]` 에서 host·ssh_user·work_dir 를 읽고 `node_role_contract` 가 정한 `sub_mode` 로
**정체성에 맞는 카나리 문구**를 골라 request 를 조립한다(ray-worker 에게 "런타임 스킬 3종" 을 묻지 않는다).
```bash
python3 scripts/bootstrap_canary.py --topology <single|multi> \
    --max-turns 10 --timeout-seconds 600 --budget-source "선언: 카나리 1왕복(인스펙트 전용)" \
    --emit /tmp/canary.json            # 조립(결정론)
python3 scripts/bootstrap_canary.py --topology <single|multi> \
    --max-turns 10 --timeout-seconds 600 --budget-source "..." --invoke   # HITL 승인 뒤 실행
```
turn 예산은 **선언**이다 — 등급표가 사라졌으므로 부르는 쪽이 값과 근거를 함께 준다(미선언 = fail-loud).
서브 미등록·`__REQUIRED__` 센티넬 잔존 시 **조립 자체를 거부**한다(틀린 계정/경로로 접속하지 않는다).
이전 판본은 이 자리에 `bootstrap_canary()` 라고만 적혀 있었고 **생산자가 0개**였다(실행자 없는 금지 — §2.7.1 위반).
(실행문법 = `references/agent-control-adapter.md` §2)
→ 서브가 새 CLAUDE.md+런타임블럭+settings+comms 로드, **phase=inspect·status=completed + self_verification** 의 schema-valid 리포트 반환.
이로써 "구성된 환경이 프로토콜대로 작동함"을 전체로서 증명(권한행·skill YAML·페르소나 비준수 포착 — 체크섬이 못 잡는 것). 실패 시 max-turns→Model-C. **카나리 미통과 시 done 선언 금지.**

- 카나리 통과 → **호스트 안전체계 세션 최종 Y/N**(§2.6, 양노드) 수행 후 온보딩 완료.

## 2.6 호스트 안전체계 — 세션 최종 선택조항 (Y/N · 양 토폴로지 공통 · plan_26071115)

> **양 토폴로지 공통 최종 스텝**: single 은 §1S manifest+Flag 기입 직후 · multi 는 §2.5 카나리 통과 직후 이 절로 온다.
> **Flag 발급 이후**에 오는 **독립 Y/N 선택조항**이다 — 표준 온보딩(스캔·게이트·manifest·Flag)은 이미 완수됐고, 안전체계는 **선택**이다(설치 안 해도 Flag valid). 헌법 §호스트 안전체계 따름정리(선택화).

- **성격**: 절차적 필수 스텝이 아니라 **선택(Y/N)**이다 — 설치를 강제하지 않고, 혜택을 서술해 권하되 거부는 존중한다.
- **① 혜택 서술(talking point 예시 — 실문구는 재량)**:
  - "768k prefill 사건에서 워치독이 컨테이너를 먼저 정리해 호스트를 지킨 실적이 있습니다 — 설치하면 이 보호막이 상시 작동합니다."
  - "설치하면 **하드다운의 블랙박스**가 남습니다 — 하드다운은 디스크에 로그를 쓸 시간조차 주지 않고 끝나는데, efi_pstore(단일)·netconsole(멀티)이 그 순간의 커널 메시지를 노드 **밖**·**전원 밖**에 남깁니다. 안 남기면 원인 추적이 원천 불가입니다."
  - "1초 샘플 시계열과 포락선이 쌓여 **로드 ETA 를 예측**하게 됩니다 — 정상 로드를 사고로 오인해 죽이는 일이 줄어듭니다."
  - (통합메모리 GPU 한정) "이 GPU 는 시스템 메모리를 공유해 GPU OOM 이 곧 호스트 다운입니다 — 워치독이 그 직전에 개입합니다."
- **② Y 분기(설치)**: 승인 시 **사용자 실행** — 설치자는 **노드블랙박스**다(아래 ⚠ 승계 참고):

  ```bash
  sudo bash .claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh --level L3           # dry-run(기본)
  sudo bash .claude/skills/terraforming_node/scripts/node_blackbox/install_node_blackbox.sh --apply --level=L3   # 적용
  ```

  → **에이전트 무인 sudo 실행 ✗**(실행 주체는 사람). dry-run 선행으로 무엇을·왜·트레이드오프를 고지한다.
  **L3 를 포함하면 채팅이 아니라 `docs/request/` 수행지시서로 위임한다(§2.6.1).**
  - **레벨**(재부팅 필요 여부가 자연 경계 · 배치와 활성화는 분리 — 파일·유닛은 레벨 무관하게 항상 도착):
    - `L1` **무재부팅** — 수집기(1초 샘플) · **ETA 워치독** · 이벤트 통합 · 로그 수명 집행 · sudoers 단일 헬퍼(`vllm-drop-caches` 경로 1개만 NOPASSWD) · **earlyoom**(프로세스-레벨 최후선 — 빌드 평면까지 커버)
    - `L2` **무재부팅·peer 필요** — netconsole 교차 스트리밍(**multi 전용** · single 은 `N/A` 로 정직 기록)
    - `L3` **재부팅 1회** — 사후 포착 = **efi_pstore** 확보. **crashkernel(2.25 GiB 예약)·ramoops 를 설정하는 게 아니라 제거한다.**
  - **검증(결정론)**: `bash .claude/skills/terraforming_node/scripts/node_blackbox/verify_node_blackbox.sh --check`.
    **상태 권위는 `installed: true` 가 아니라 `docs/logs/<node_id>/capture_verified.json` 이다** — "ready" 는 주장이지 증거가 아니다(2026-07-30 하드다운 2회에서 kdump 는 양노드 `ready to kdump` 였는데 vmcore 0건).
  - → manifest `host_safety.installed: true`.

  > ⚠ **승계(2026-08-18 개정 · `plan_26073109`)** — 이 절은 예전에 `host_safety/install_host_safety.sh
  > --apply [--with-kdump]` 를 지시했다. 그것이 만들던 것(mem_watchdog·earlyoom·kdump·netconsole·임시
  > telemetry)은 **노드블랙박스 단일 패키지로 승격**됐다: `purge_host_safety.sh` 가 구 설치물을 잔재
  > 없이 걷어낸 뒤 `install_node_blackbox.sh` 가 설치한다.
  >
  > **특히 `--with-kdump` 는 이제 권하지 않는다 — 해로운 것으로 판정됐다.** 강제 크래시 5회
  > (`testlog_26073113`)가 뒤집은 결과: ① vmcore 0/4(makedumpfile 이 커널 6.17 미지원 — 구조적)
  > ② **kdump 가 무장돼 있으면 pstore 가 원천 차단된다**(`crash_kexec_post_notifiers=N` 이라 `panic()`
  > 이 `kmsg_dump` 보다 먼저 kexec 로 점프해 돌아오지 않는다) ③ 그 대가로 2.25 GiB 를 **상시** 예약한다.
  > 즉 kdump 는 *유일하게 작동하는 사후 포착 수단을 죽이면서* 메모리를 먹었다. ramoops 도 이 플랫폼에선
  > 불가(정상 재부팅만으로 헤더가 깨진다 = 펌웨어가 리셋 때 DRAM 을 초기화). **efi_pstore 는 kdump 를
  > 내린 상태에서 패닉 19 레코드를 포착했다(1/1).**
  >
  > **이 절의 Y/N 선택조항 성격·무인 sudo 금지는 그대로 유효하다** — 설치 대상만 바뀌었다.
  > 레거시 설치물이 남아 있는 노드는 `purge_host_safety.sh --require-seed` 를 **먼저** 돌린다(저널 수확
  > 선행 게이트 — mem_watchdog 저널은 포락선의 유일한 초기 데이터이고 유닛 제거 후 vacuum 되면 복구 불가).
- **③ N 분기(미설치·opt-out)**: 서빙은 정상 진행. manifest `host_safety.installed: false` 로 기록한다. 재권유는 **세션당 1회 이하**.
  - **통합메모리 노드 한정 후속 경고**: opt-out + 통합메모리(GPU OOM=호스트 하드다운 위험) 노드는, 이후 서빙 기동 직전 **에이전트 채팅창 1줄** 안내만 한다("워치독 미설치 상태 — 통합메모리라 OOM 시 호스트 다운 위험, `install_host_safety.sh` 로 언제든 보강 가능"). **serve 스크립트/로그 배너 코드변경 ✗**(시끄러운 경험 방지 — D31·NG-5). **discrete GPU 노드는 무경고.**
- **④ 파급 정밀화(opt-out 이어도 보호 일부 유지)**: 하네스 **협역 워치독**(`run_trial`·`multinode_serve_smoke.sh` 자동 기동)은 레포 내장 스크립트라 **설치와 무관하게 계속 작동**(opt-out 사용자도 trial 중 보호 유지). 로드-전 RAM 게이트(⑤.5)의 `vllm-drop-caches` 자동 드랍만 헬퍼 부재로 skip 되며, 게이트는 이를 **음성정직으로 보고**(드랍 없이 재측정 → 부족 시 기동 거부 exit 7 유지 — `preload_ram_gate.try_drop_caches` 기구현 graceful).
- **⑤ 멀티노드 변형**: 양노드(메인+서브) 각각 동일 Y/N. **서브 설치는 렌더 배달분**(`.claude/runtime/node_blackbox/install_node_blackbox.sh` — `render_sub_env.py` §4.6 이 `node_identity.sh` 를 포함해 배달한다)으로 **서브에서 사용자가 실행**(A2A 경계 — 메인 sudo 대행 ✗). manifest `nodes[].host_safety.installed` 로 **노드별 독립** 기록. **L2(netconsole)는 멀티에서만 성립**하므로 양노드 peer 지정이 필요하다.

### 2.6.1 노드블랙박스 설치 — `docs/request/` 수행지시서로 위임 (2026-08-18 신설 · `plan_26081716`)

> §2.6 Y 분기의 **레벨 정의·설치 명령은 그 절이 소유**한다. 이 절은 그중 **L3(재부팅 포함)** 를
> 채팅이 아니라 문서로 위임하는 배선만 다룬다.

- **① 발행 스텝(L3 포함 시)**: 사용자가 Y 로 답하고 그 범위에 **L3 가 포함되면**, 채팅으로 명령을
  나열하는 대신 **수행지시서를 발행**한다:

  ```bash
  python3 .claude/skills/terraforming_node/scripts/node_blackbox/publish_install_request.py \
      --generated-utc <YYYY-MM-DDTHH:MM:SSZ> --level L3   # --topology 는 manifest 에서 읽는다
  ```

  → `docs/request/request_<YYMMDDHH>_노드블랙박스_L3_설치_수행절차.md`(명명은 `doc_naming` 결정론).
  **시각은 주입 전용이다**(벽시계 금지 — `staleness_gate.py --max-age-days` 선례). 발행기는 설치자·
  검증기의 인터페이스를 실물 대조하고 어긋나면 **fail-closed**(exit 3)로 죽는다 — 낡은 지시서를
  내지 않는다.
- **왜 문서인가**: L3 는 재부팅을 요구하는데 **에이전트는 그 호스트 위에서 돈다.** 재부팅과 함께
  세션이 소멸하고 컨텍스트(어디까지 했는지·무엇을 검증할지·회수물이 무엇인지)가 함께 사라진다.
  **문서는 디스크에 남아 재부팅을 생존한다.** 멀티노드면 사람이 두 노드를 오가야 하므로 이득이 두 배다.
  지시서는 **진행 체크박스**를 자기 안에 담아 *상태*까지 생존시킨다.
- **② 발행하지 않는 경우**: `L1`·`L2` 만이면 발행하지 않는다(발행기가 exit 1 로 거절 —
  `--force` 로만 우회). 재부팅이 없으면 세션이 죽지 않아 문서의 존재 이유가 약하고, §2.6 의
  선택(Y/N) 성격과도 맞다. 채팅 안내로 족하다.
- **③ 회수 스텝**: 사람이 "완료"를 알리면 에이전트가 **정해진 경로에서 직접 읽는다**(채팅 붙여넣기
  요구 ✗ — 재부팅 생존 목적과 정합):

  | # | 회수물 | 경로 |
  |---|---|---|
  | 1 | 검증 출력 | `verify_node_blackbox.sh --check` 재실행으로 재현 가능 |
  | 2 | proof-of-capture 판정 | `docs/logs/<node_id>/capture_verified.json` |

  `<node_id>` 해소는 `node_identity.sh` 단일 소유(§2.7.6 — 각자 파싱 금지). 멀티노드면 **노드마다**
  생성된다. **`installed: true` 가 아니라 `capture_verified` 가 상태 권위**다(주장 ≠ 증거).
- **④ evidence chain 밖이다**: request 는 판정도 계측도 아니라 `completion_gate` 를 타지 않는다
  (`.claude/rules/docs.md` §request). 회수물이 돌아오면 그때 testlog/devlog 로 chain 에 편입한다.
- **⑤ 무인 sudo 금지는 그대로다**: 발행은 에이전트가, **실행은 사람이** 한다. 멀티노드에서 메인이
  서브의 `sudo` 를 대행하지 않는 경계(§2.6 ⑤)도 유지된다 — 지시서가 양노드 절차를 한 문서에 담을 뿐
  실행 주체를 바꾸지 않는다.

## 2.7 노드 제어 규약 — 메인↔서브 평면·범주·권위 (정본 · 2026-08-15 이관)

> **이관 근거** `plan_26081514_헌법스킬_책임범위_재설정_{설계,구현}.md` Q2/Step 1. 헌법(`CLAUDE.md`)은
> **"왜"**(항상 참인 원칙)만 갖고, **"어떻게"**(도메인 절차)는 이 스킬이 소유한다. 아래 각 절의 `원문:`
> 은 이관 직전 원본의 git blob SHA — 정합이 깨지면 `git cat-file -p <sha>` 로 복원한다.
>
> **이 절이 이 스킬에 사는 이유**: 노드(메인·서브)는 terraforming 이 스캔·렌더·배달로 **만든** 대상이다.
> 그 대상을 어떤 평면에서 어떤 권한으로 다루는지는 노드 도메인의 절차이지 모든 도메인의 철학이 아니다.
>
> ⚠ **§2.7.1–§2.7.7 은 멀티 기준으로 쓰였다.** 어느 절이 싱글에도 적용되는지는 **§2.7.0 분기표가
> 먼저 정한다** — 이 순서를 뒤집으면(절부터 읽고 토폴로지를 나중에 따지면) 멀티의 배달·동기 개념이
> 싱글로 새어 든다. 그것이 2026-08-22 진단의 5증상이었다(`plan_26082214` §0).

### 2.7.0 토폴로지 분기 — **제1축** (신설 2026-08-22 · `plan_26082214` §4.2)

> 헌법 **불변식 A** 의 "어떻게". 헌법은 *왜 토폴로지가 제1축인가*를 말하고, 이 절은 *그래서 어느
> 절·어느 도구가 켜지는가*를 정한다. 외부 그라운딩 정본 = `docs/report/node-identity-topology-grounding.md`.

**한 단어가 두 존재를 덮고 있었다.** `output/single/manifest.yaml` 과 `output/multi/manifest.yaml` 은
같은 `nodes[].role: main|sub` 스킴을 쓰지만, 두 통로의 "sub" 는 본질이 다르다:

| | **multi 의 sub** = Ray 워커 | **single 의 sub** = A2A 원격 에이전트 |
|---|---|---|
| 정체성 권위 | **manifest `nodes[]` 인덱스(rank) + role** | **`Agent_Card.json`**(A2A 1.0.1 계약: 능력·엔드포인트·**서명**) + **서브 manifest**(`self_role: sub` · HW·경로·획득 모드 — terraforming 실측·발급 · §2.7.10) |
| 외부 정본 | NCCL rank + uniqueId · Ray head/worker | A2A Client·AgentCard·Task |
| sub↔sub 통신 | 대칭 collective(집단 연산) | **없음** — 각자 메인하고만 대화 |
| 제어 평면 | head 종속(SSH 제어) | client→server 호출(A2A Task 위임) |
| 버전·드라이버 | **동기 필수**(집단 연산 ABI 정합) | **독립**(핀 커플링 없음) |
| 빌드 흐름 | 메인 빌드 → `sync_to_sub` 전파 → **동일 빌드킷에서 파생된 동일 ABI** 분산 | 노드별 **독립 병렬 빌드**(서브가 자기 빌드킷 자율 저작) |
| `sub_mode` | `ray-worker` | `a2a-agent` |

- **버전 싱크의 이유는 로드 균형이 아니라 집단 연산의 lockstep 정합**이다(그라운딩 §6.1). 그래서 싱글엔
  적용될 이유가 애초에 없다 — 싱글은 collective 에 참여하지 않는다.
- ⚠ **"동일 이미지"가 아니라 "동일 ABI"다**(2026-08-22 실측 정밀화 · `testlog_26082215` §4.7 M-4).
  분산 서빙에 실제로 참여한 두 컨테이너의 **image digest 는 서로 달랐다**(`58fd5b62…` vs `c5487620…`)
  — 각 노드가 로컬에서 자기 빌드를 하므로 digest 일치는 애초에 성립하지 않는다. 정확히 일치해야 하는
  것은 **집단 연산 ABI 3종**(vLLM git SHA · torch · driver)이고, 실측에서 그 셋은 완전히 일치했다.
  digest 를 커플링 판정 기준으로 쓰면 정상 배포를 불일치로 오판한다.
- **`role: sub` 의 *존재*는 정체성을 말하지 않는다.** 판정 입력은 언제나 **`sub_mode`** 다.

**결정론 판정기 = `scripts/node_role_contract.py`**(이 계약의 단일 소유자):

| 판정 | 호출 | 산출 |
|---|---|---|
| 이 sub 는 무엇인가 | `resolve_sub_mode(topology, declared)` | `{value, source}` — `derived-from-topology` \| `declared-and-agrees` |
| 집단 안의 위치 | `resolve_rank(topology, nodes, role)` | multi=`nodes[]` 인덱스 · single=`None` + `not-applicable:single-a2a-agent` |
| 정체성 권위는 어디인가 | `identity_authority(topology)` | `agent-card` \| `manifest-rank-and-role` |
| **빌드킷 배달 평면이 켜지나** | `delivery_plane(topology, sub_mode)` | `active`(ray-worker) \| `dormant`(a2a-agent) |

```bash
# 셸 소비자는 한 줄 값으로 묻는다. 위반이면 값을 찍지 않고 종료코드 5.
python3 .claude/skills/terraforming_node/scripts/node_role_contract.py \
        evaluate --topology single --repo . --field delivery_plane --format value    # → dormant
```

> ✅ **배선 완료(2026-08-22 · W-1)** — `sync_to_sub.sh:_single_extension_active` 는 이제 위 판정기를
> 호출하고 그 답(`delivery_plane`)만 비교한다. `role: sub` 의 *존재* 는 더 이상 판정 입력이 아니다.
> 판정기 부재·파싱 실패·계약 위반은 전부 **dormant(fail-closed)** 이며 사유를 stderr 로 밝힌다.
> dry-run/B1 안내문도 판정기가 답한 **출처**를 그대로 인용한다(`source=sub-mode:a2a-agent` ·
> `no-sub-registered` · `fail-closed:*`) — "nodes[] 비어있음" 이라는 옛 모델 문구는 제거됐다.
> 이전 상태의 실증은 `testlog_26082215` §4.2(같은 평면이 **34회 발화**)다.

- **`sub_mode` 는 파생값이다 — manifest 선언은 tripwire다.** 사상이 1:1 이라 topology 만 알면 계산된다.
  필수 손저작으로 만들면 헌법 §판정표의 *"파생 가능한데 손으로 적은 것"*(하드코딩 **결함**)이 된다.
  선언은 **선택**이며, 선언되면 파생값과 **일치해야 한다**(어긋나면 `SUB_MODE_TOPOLOGY_CONFLICT`
  fail-closed). 선언의 값어치는 값 전달이 아니라 **혼동 지점에서 의미를 읽히게 하고 변경 시 리뷰를
  강제하는 것**이다(판정표의 tripwire **정당** 칸).
- **fail-closed 방향은 언제나 dormant** 다 — topology 미해소·`sub_mode` 미확정이면 배달은 열리지 않는다.

**절별 적용 범위** — 아래 표가 §2.7.1–§2.7.7 을 토폴로지로 가른다:

| 절 | multi | single | 싱글 분기 주석 |
|---|---|---|---|
| §2.7.1 권한 평면 A/B | ✔ | ✔ | 평면 정의는 토폴로지 무관(승인 vs 소실방지) |
| §2.7.2 저작/스캔/정비 3범주 | ✔ | ✔ | **무단 스캔 금지는 양쪽 공통** — A2A 불투명 피어 원칙이 같은 결론을 낸다 |
| §2.7.3 B0–B3 상태 표 | ✔ | ✔ **B1 배달 ✔**(2026-09-03 라이브) | 싱글의 B1 은 **에이전트 환경 오버레이**(CLAUDE.md·comms·runtime 스킬·블랙박스·위임키) 배달이다. **빌드킷 평면은 `sub_mode=a2a-agent` 에서 설계상 dormant** — 싱글 서브는 자기 빌드킷을 자율 저작한다(불변식 A). 판정은 `node_role_contract` 의 `delivery_plane` 이 소유하고 `sync_to_sub` 는 그 값을 인용만 한다. 2026-09-03 이전의 "B1 ✗" 는 오버레이 평면이 없던 시절의 관측이다(`plan_26090317` P3 실증: 정착·카나리·릴레이 2회·오버레이 재배달) |
| §2.7.4 권위 평면 계약 | ✔ | ✔ | 커밋/인덱스/파일시스템 구분은 토폴로지 무관 |
| §2.7.5 sync 절차 평면 분리 | ✔ | ✔ | 하위행위·게이트 동일. 싱글은 **에이전트 환경 오버레이 평면**이 하나 더 있어 빌드킷 평면과 분리 배달된다(`SKIP_BUILDKIT` · plane-aware checksum · 2026-09-03) |
| §2.7.6 node-identity | ✔ rank+role | ✔ AgentCard | 같은 절 안에서 갈린다(아래) |
| §2.7.7 A2A 제어명령 | ✔ | ✔ | 싱글에서 **더** 중심적이다 — 제어 자체가 A2A Task 이므로 |
| §2.7.8 그라운딩 교환 | ✔ | ✔ | 도서관 비대칭은 토폴로지 무관(메인 단독) |

- **싱글에서 서브를 "제어"한다는 말의 의미**: 메인은 A2A Task 를 발급하고(§2.7.7 제어명령은 그 특수형),
  서브는 자율 수행 후 push-attestation 리포트를 돌려준다. 메인이 서브의 **빌드 산출물을 밀어 넣지
  않는다** — 그건 멀티의 평면이다.

### 2.7.1 권한 평면 A/B

> 원문: `CLAUDE.md@68dd64c8eea3` §불변식 · `.claude/rules/workflow.md@0d8f542eafa3` §권한 평면 A/B(2026-08-13 신설 · `plan_26081313`)

| | 평면 A: 사용자↔메인 | 평면 B: 메인↔서브 |
|---|---|---|
| 상대 | 독립 소유권 주체(사람) | 메인이 렌더·배달해 만든 작업환경 |
| 게이트 성격 | **승인** — 대행 시 자기승인 순환 | **소실 방지** — 대행이 정상(다른 주체 없음) |
| git 의 역할 | 사용자의 작업 이력·소유 표현 | **메인의 서브 추적용 관측 장치** |
| 예 | `execution_approval`·HITL 게이트 | `SUB_SYNC_DIRTY_AUTOSAVE` |

- **A 의 승인 규칙을 B 에 투영하지 않는다** — 존재하지 않는 승인 주체를 요구해 교착이 된다(2026-08-13 실증 · `plan_26081313`).
- **서브 git 은 거버넌스 주체가 아니라 관측 장치**다. 메인은 서브 git 에 **완전한 조작 권한**(`commit`·`checkout`·`clean`)을 갖는다.
- B 에 남는 게이트의 근거는 승인이 아니라 **격리**(PII — 상향 문서기반 only)와 **범위**(레시피 계열만 — `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION`)여야 한다.
- **새 게이트를 B 에 놓을 때는 그 처방을 *누가 실행하는가*를 먼저 적는다** — 메인이 아니면 교착이다(헌법 불변식 ③의 도메인 적용).

### 2.7.2 메인이 서브에 하는 행위의 3범주

> 원문: `CLAUDE.md@68dd64c8eea3` §불변식 · `.claude/rules/workflow.md@0d8f542eafa3` §3범주(2026-08-14 신설 · `plan_26081409` C)

*"서브 디스크를 무단 스캔하거나 직접 교정하지 않는다"* 한 문장이 **두 관심사**(격리·저작)를 묶어, **어느 쪽도 아닌 행위**까지 막아 교착을 만들었다. 세 범주로 가른다:

| 범주 | 무엇 | 허용 | 근거 |
|---|---|---|---|
| **저작** | 서브 작업환경·헌법 콘텐츠를 **만들거나 고침** | 템플릿→렌더→배달 **only** | 재현성(직접 교정은 드리프트) |
| **스캔** | 서브 디스크 내용을 **읽음** | **금지**(상향 회수는 문서기반) | PII 격리 |
| **정비** | **정보량 0**인 상태 결손 복구(빈 디렉터리·mode 오차) | **허용** · 로그 필수 | 배달 전제 복원 |

- 정비가 저작도 스캔도 아닌 이유: **만들지 않고**(제거·정정만) **읽지 않는다**(내용이 없다). **정보량 0** 이 범주의 경계다 — 바이트를 가진 것을 지우는 것은 정비가 아니라 삭제이며 `ALLOW_DELETE` 게이트가 관할한다.
- **실증**(2026-08-14): 롤백 스냅샷이 `find -type f -o -type l` 로 파일·심링크만 담는데 **빈 디렉터리는 파일 경로로 함의되지 않는 유일한 디렉터리**라 복원 불가 → 검증기가 배달을 거부 → 복구 경로가 없어 **사람의 ssh `rmdir`** 이 필요했다. 메인은 그 권한을 이미 갖고 있었으나 **파이프라인이 쓸 줄 몰랐다.**
- ⚠ **정비의 범위는 전송 범위와 일치**해야 한다. `BAND2_EXCLUDED_TOP` 아래는 rsync 가 안 건드리므로 정비도 안 건드린다 — 최상위 디렉터리 자신뿐 아니라 **하위 전부**다(2026-08-14 초판이 이걸 어겨 `cache/vllm/…` 를 정비했다. 대조실험이 잡았다).

### 2.7.3 메인↔서브 B0–B3 상태 표

> 원문: `.claude/rules/workflow.md@0d8f542eafa3` §메인↔서브 B0–B3

| 상태 | 입력→출력 | gate | 실패/owner |
|---|---|---|---|
| B0 bootstrap | 최초 배달→서브 local git | dry-run→HITL apply | `policy:SUB_GIT_LOCAL_ONLY` |
| B1 하향 | canonical render/output→서브 | dirty→보존 후 진행·checksum | `policy:SUB_SYNC_DIRTY_AUTOSAVE`; `upstream-version-watch/scripts/sync_to_sub.sh` |
| B2 상향 | 서브 docs path→`sync_staging/sub_docs`→HITL 재저작 | **상향 회수(서브→메인) = 문서기반 only** | `fetch_sub_docs.sh`; patch/code 직접 회수 금지 |
| B3 경계 | attestation+미러 docs→메인 관측 | A2A 정체성(`policy:A2A_IDENTITY_PROOF_FAIL_CLOSED`) | 메인은 서브 디스크 재스캔 금지(§2.7.2 스캔) |

- 서브 개선은 `[improve]` history 에 남고 메인 저작권은 template→render→delivery 로만 행사한다(§2.7.2 저작).
- egress-restricted 서브의 외부조사 실패도 **docs 로만** 상향한다.
- **공통 진입 게이트**: B0–B3 진입 전 tracked terraform validator + `policy:A2A_IDENTITY_PROOF_FAIL_CLOSED` 검증(`workflow.md` §공통 진입 게이트).

### 2.7.4 권위 평면 계약 — 어느 도구가 무엇을 읽는가

> 원문: `.claude/rules/workflow.md@0d8f542eafa3` §권위 평면 계약

| 도구 | 읽는 평면 | 귀결 |
|---|---|---|
| `sync_branches.sh` | **커밋**(`git checkout <branch> -- <path>`) | 미커밋 변경은 전파되지 않는다 |
| `sync_to_sub.sh` | **인덱스**(`ls-files \| checkout-index`) | `git add` 만으로 배달된다(커밋 불요) |
| 렌더·빌드 | **파일시스템** | 인덱스와 갈리면 `info:` 만 찍고 진행한다 |

같은 파일이 세 평면에서 서로 다른 값을 가질 수 있다. **새 도구는 이 표에 행을 추가한다** — 명문화 없이는 "고쳤는데 안 갔다"와 "안 고쳤는데 갔다"가 반복된다(2026-08-13 실제 발생: `55-src-deps-authority.sh` 배달 누락).

### 2.7.5 sync 절차의 평면 분리

> 원문: `.claude/rules/workflow.md@0d8f542eafa3` S2.5 + §권한 평면 A/B (분리 명문화는 `plan_26081514` Step 1 신설)

한 번의 sync 안에 **두 평면이 섞여 있다.** 섞은 채로 게이트를 걸면 어느 쪽 규칙을 적용할지 모호해진다.

| sync 하위행위 | 평면 | 게이트 | 실행 주체 |
|---|---|---|---|
| **전달 콘텐츠 결정**(무엇을 보낼지·핀·해소값) | **A** | 사용자 서명(HITL 게이트 ②) | 사람 |
| **dry-run → apply** | **A** | 사용자 승인 | 사람 |
| 서브 dirty 보존(`[autosave]` commit) | **B** | 소실 방지 — **메인 자율** | 메인 |
| 서브 git `checkout`·`clean`(unstick) | **B** | 소실 방지 — **메인 자율** | 메인 |
| 서브 빈 디렉터리 정비(§2.7.2) | **B** | 로그 필수 — **메인 자율** | 메인 |

### 2.7.6 노드 정체성(node-identity) — `role` + `rank` 스킴

> 원문: `docs/plan/plan_26081514_노드정체성_명시화_role스킴_PII구조해소.md` §3 (R 스킴).
> **실장 완료** — `scripts/node_blackbox/node_identity.sh`(node_id 단일 해소기, 소비자 4종이 source)
> + `scripts/node_role_contract.py`(rank·sub_mode·정체성 권위). 2026-08-22 `plan_26082214` §4.2 에서
> **토폴로지 축**(rank vs AgentCard)을 얹어 완성했다. 종전의 "⚠ 미실장" 표기는 이로써 해소된다.

**정체성은 두 질문으로 갈린다** — *"이 노드를 무엇이라 부르는가"*(node_id, 경로 성분)와 *"이 노드는
집단의 어디인가"*(rank, 멀티 전용). 앞엣것은 토폴로지 무관이고, 뒤엣것은 **§2.7.0 제1축에서 갈린다.**

#### (a) node_id — 이름 (양 토폴로지 공통)

```text
node_id ::= manifest nodes[].role 슬러그
정규식  ::= ^[a-z][a-z0-9-]{0,31}$          (fail-closed 검증)
경로    ::= docs/logs/<node_id>             (2노드에서 main | sub)
```

| 항목 | 규약 |
|---|---|
| **메인측 권위** | `output/<topology>/manifest.yaml` 의 `nodes[].role` |
| **서브측 권위** | 배달된 **서브 manifest** `output/<topology>/manifest.yaml` 의 최상위 `self_role: sub`(terraforming 실측·발급 · §2.7.10). 2026-09-05 이전에는 `Agent_Card.json:node_identity.role` 이었으나 Agent_Card v2 는 A2A 평면 계약이라 정체성 필드를 갖지 않는다 |
| **기본값** | **없음.** 해소 실패 시 `$(hostname)` 로 떨어지지 않고 **fail-loud 종료** |
| **해소 우선순위** | ① 명시 `--node-id=<slug>` ② `output/*/manifest.yaml` 의 `self_role`(서브측 · 여러 manifest 가 갈리면 fail-loud) ③ `output/*/manifest.yaml` 의 유일한 `role: main` ④ fail-loud |
| **hostname 의 남은 자리** | gitignored 렌더 산출물의 **비권위 속성**(`Agent_Card.json:supportedInterfaces[].url` 의 호스트 성분 · 서브 manifest `nodes[].hostname`)뿐. **경로 성분·문서 산문에는 등장 금지** |
| **의미 스코프** | **클러스터 스코프**(누구의 디스크에서 보든 같은 이름). 메인의 `docs/logs/sub/` = 서브 미러, 서브의 `docs/logs/sub/` = 정본. 경로 동일, 권위만 다름 |

- **기본값 제거가 스킴의 핵심이었고, 지금은 제거되어 있다.** 옛 `NODE_ID="$(hostname)"` 은 헌법이 금지한 *"결정·게이트·안전 경로의 침묵 폴백"*(§결정론 규율 4종 안티패턴 판정표의 **결함** 칸)이었다 — 틀려도 조용히 새 로그 트리를 만들고, 워치독은 아무도 안 보는 곳에 기록하며, 관측 공백은 사고가 나야 발견된다. 현행 배선은 `ni_resolve_node_id` 하나가 해소하고 **실패하면 죽는다**(`node_identity.sh:ni_resolve_node_id` 마지막 분기 = 옛 hostname 자리).
- **각자 파싱 금지.** 소비자(`install_node_blackbox.sh`·`verify_node_blackbox.sh`·`purge_host_safety.sh`·`multinode_serve_smoke.sh`)는 이 파일을 source 하고 `ni_resolve_node_id` 만 부른다 — 5개 스크립트가 제각기 `$(hostname)` 을 파생하던 것이 원래 문제였다.

#### (b) rank — 위치 (**멀티 전용** · 신설 2026-08-22)

```text
rank ::= manifest nodes[] 배열 인덱스 (0..n-1)
권위 ::= scripts/node_role_contract.py :: resolve_rank(topology, nodes, role)
```

| 토폴로지 | rank | `source` 값 | 근거 |
|---|---|---|---|
| **multi** | `nodes[]` 인덱스 | `manifest-nodes-index` | NCCL "n개 디바이스 각각에 0..n-1 의 고유 rank" 차용 |
| **single** | **`None`** | `not-applicable:single-a2a-agent` | A2A 는 집단이 아니다 — 정체성은 AgentCard이지 위치가 아니다 |

- **싱글에서 rank 를 조용히 0 으로 채우지 않는다.** 그건 "싱글 sub 가 집단의 0번"이라는 거짓말이고,
  하류가 그 값을 TP·NCCL 입력으로 오해할 수 있다. `None` + 출처 문자열이 **음성정직** 표기다
  (`staleness_gate` 의 `skipped:*` 선례와 동형).
- ⚠ **알려진 경계 — N>2**: 현행 role 어휘는 `{main, sub}` 닫힌 목록이라 **서브가 2대 이상이면 슬러그가
  겹치고 node_id 도 겹친다**(로그 트리 혼합). `resolve_rank` 는 조용히 첫 항목을 고르지 않고
  `RANK_AMBIGUOUS_ROLE` 로 죽는다. 어휘를 넓히려면 **세 공동 소유자가 함께** 바뀐다 —
  `node_role_contract.py`(SUB_MODE/role) · `scan_node.py` manifest 게이트 · `node_identity.sh:NI_MAIN_ROLE`.
- multi 에서 `role: main` 이 인덱스 0 이 아니면 **위반이 아니라 note** 다(Ray/NCCL 은 head 가 배열
  첫 항목일 것을 요구하지 않는다). 다만 관례와 어긋나므로 침묵하지 않는다.

#### (c) 렌더 산출물에 실릴 형태 — ✅ **배선 완료**(2026-08-22 · W-2 → 2026-09-05 Agent_Card v2 로 이관)

Agent_Card v2 는 A2A 1.0.1 표준 필드만 최상위에 둔다. 토폴로지 축 해소값은 표준이 정한 확장 자리
`capabilities.extensions[]` 의 **`urn:easy-vllm:ext:node-role:v1`** 하나에 **값과 출처를 함께** 싣는다 —
`rank`/`sub_mode` 는 파생값이라 출처 없이는 하류가 측정·선언·파생을 구분하지 못한다(헌법 §결정론 규율 "출처 표시").
검증기·서명기 = `scripts/agent_card_contract.py`(§2.7.10):

```jsonc
"capabilities": { "extensions": [ {
  "uri": "urn:easy-vllm:ext:node-role:v1", "required": true,
  "params": {
    "topology": "single",
    "sub_mode": "a2a-agent", "sub_mode_source": "derived-from-topology",   // 선언되어 일치하면 declared-and-agrees
    "rank": null, "rank_source": "not-applicable:single-a2a-agent",          // multi 면 nodes[] 인덱스
    "identity_authority": "agent-card", "delivery_plane": "dormant",
    "tool_plane": ["vllm-recipe-explorer", "adversarial-benchmark", "upstream-version-watch"],
    "tool_plane_source": "sub-mode:a2a-agent"
  } } ] }
```

- 값의 산출 권위는 `node_role_contract.py` 이며 렌더러는 그것을 **적기만** 한다(두 번째 파생 구현 ✗).
  배선: `render_sub_env.py::_contract_placeholders` 가 `evaluate_manifest` 를 불러 네 값을
  `{{SUB_MODE}}`·`{{SUB_MODE_SOURCE}}`·`{{SUB_RANK}}`·`{{SUB_RANK_SOURCE}}` 로 옮긴다.
  **회귀핀**: `render_sub_env.py --self-test` 가 렌더 산출물과 판정기 산출의 **문자 그대로 일치**를
  검사한다(`testlog_26082215` S-11 FAIL 재발 차단).
- `rank` 는 JSON **정수 또는 null** 이지 문자열이 아니다 — 템플릿에서 따옴표 없이 치환된다.
  `"null"` 로 실으면 하류가 rank 를 문자열로 읽어 TP·NCCL 입력으로 오해할 수 있다.
- 계약 위반(예: single 에 `sub_mode: ray-worker` 선언)이면 네 값이 비고, 렌더러는 **필수 필드 누락**
  경로로 **fail-loud 종료**한다(빈 정체성 렌더 금지). 위반 코드도 함께 출력한다.
- 카드의 서술 자체도 토폴로지로 갈랐다(W-3): `topology_contract.{a2a-agent,ray-worker}` 블록이
  분리돼 있고, 종전의 *"현재 브랜치로 분기"*·*"메인 정본 빌드를 byte-equiv 재현"* 문구는 제거됐다
  (브랜치 추론은 헌법 금지이고, single 서브는 재현자가 아니라 **자율 저작자**다).
- `hostname` 은 여기에만 남고 **경로 성분이 되지 않는다.**
- **PII 구조 해소**: `hostname` 이 경로에서 사라지면 `spark-host` 패턴은 애초에 걸릴 것이 없다. `docs.md` 의 `<node_id>` 규약은 문자 그대로 유효하게 남는다(개정 대상은 규약이 아니라 `<node_id>` 의 **정의**뿐).
- **예외**: `--confirm=CRASH-$(hostname)` 파괴적 확인 토큰은 PII 스캔 평면 밖이라 **의도적으로 hostname 을 남긴다**(경로·기록은 role, 파괴적 확인만 호스트). 최종 결정은 해당 plan 의 G2.

### 2.7.7 A2A 제어명령 프로토콜 (신설 — 교착 해제)

> 신설: `plan_26081514_…_구현.md` Step 1. §2.7.1 의 *"서브 git 은 관측 장치이고 메인은 완전한 조작 권한을 갖는다"* 를 **실행 가능한 명령 시퀀스**로 만든 것. 이것이 없으면 원칙은 있는데 처방 주체가 없어 §2.7.2 실증과 같은 교착이 반복된다.

**전제**: `policy:A2A_IDENTITY_PROOF_FAIL_CLOSED` 통과(정체성 증명 부재·위조 → 진입 금지) · 대상은 **서브 git 과 정보량 0 상태 결손만**(콘텐츠 저작은 §2.7.2 저작 경로로).

| 명령 | 언제 | 무엇 | 평면 |
|---|---|---|---|
| `sub.git.status` | 배달 전 상시 | 서브 워킹트리 dirty 여부 관측 | B(관측) |
| `sub.git.autosave` | dirty 감지 | `git add -A && git commit -m '[autosave] pre-sync'` — **소실 방지·메인 자율** | B |
| `sub.git.unstick` | 배달 검증기가 거부 | `git checkout -- <path>` / `git clean -fd <path>` — **경로 한정**, 전역 금지 | B |
| `sub.fs.repair` | 빈 디렉터리·mode 오차 | `rmdir`/`chmod` — **정보량 0 한정**(§2.7.2 정비) · 로그 필수 | B |
| `sub.git.log` | 상향 관측 | 서브 작업 이력 조회(내용 아닌 이력) | B(관측) |

- **경로 한정이 안전장치다** — `unstick`·`repair` 는 배달 대상 경로에만 건다. `BAND2_EXCLUDED_TOP` 및 그 **하위 전부**는 대상 밖(§2.7.2 ⚠).
- **모든 제어명령은 로그를 남긴다** — 침묵 조작은 "원래 그랬던 것"과 구분되지 않는다.
- **바이트를 가진 것의 삭제는 이 프로토콜 밖**이다 — `ALLOW_DELETE` 게이트 관할이며, 그 안내문을 그대로 따르면 파괴가 완성되는 형태였던 선례(D5)가 있으므로 **안내문 복창 금지**.
- 실행문법(provider 별 `claude -p` 호출 형태)은 `references/agent-control-adapter.md` 에서만 해소한다.

#### 2.7.7a 턴제 릴레이와 자율 재개 (2026-09-04 · `plan_26090412`)

**소진·유보로 끊긴 작업을 다음 attempt 가 이어받는다.** 전송(같은 세션 `--resume`)은 2026-09-03 부터
실동했으나 **이어붙이는 실행자가 없어**(호출자 0) 사람이 본문을 손저작해야 했다 — 그 자리를 `relay.py`
가 채운다.

| 명령 | 언제 | 무엇 | 평면 |
|---|---|---|---|
| `relay.py --task ... --max-turns N --timeout-seconds N --budget-source "..." --resume new` | 첫 위임 | **선언한 예산·재개**로 delegate · 원장 개설(`campaigns/<camp-id>/relay/<ctx>.json`) | B |
| `relay.py --continue` | 소진·유보 뒤 | **원장에서 본문을 조립**해 미리보기 + 예산·세션 **사실** 표시(기본 dry-run) | B |
| `relay.py --continue --apply --max-turns N --timeout-seconds N --budget-source "..." --resume <id\|new>` | 사람이 본문을 승인 | 조립 본문 + 새로 선언한 예산 + **선언한 세션**으로 delegate | B |

- **재개는 선언이다**(2026-09-05 · 축 F): 종전에는 `latest_session_id()` 가 원장을 보고 코드 규칙으로
  정했고 그 규칙이 라이브에서 두 번 어긋났다(완결 뒤 옛 세션 반환 · 소진 세션 무조건 폐기). 이제
  dry-run 이 **마지막 알려진 세션과 그 맥락**을 보여주고, `--resume <session_id|new>` 로 선언하지 않으면
  fail-loud 한다. 리포트 없이 끝난 턴은 `campaigns/<camp-id>/relay/pending_hitl.json` 에 그 세션 id 를 남긴다(침묵 종결 ✗).
- **원장은 append-only 다**: attempt 마다 `request_path`(보낸 요청 원문)·`report_path`·`end_reason`·
  `started_utc`/`ended_utc`(메인 실측)·`duration_ms`/`duration_api_ms`(provider 보고)를 적는다.
  **정지 시간 = wall − api** 이며 두 값의 출처가 다르므로 섞지 않는다.

- **조립기는 합성하지 않는다** — 직전 attempt 의 제어 상태(원장) · 서브가 보낸 `artifacts[]`·
  `next_steps`·`notes` · 사람이 `campaigns/<camp-id>/relay/pending_hitl.json` 에 적은 `answer` · 원 지시. 그 넷뿐이다.
  메인이 추측한 진행상황을 본문에 적으면 그것이 SILENT_FALLBACK 이다.
- **정지 조건 둘**(루프를 만들면서 정지 조건을 미루지 않는다): ⓐ 직전이 `completed` 면 이을 중단점이
  없다 ⓑ **차단성 요청에 답이 없으면** 진행하지 않는다. ⓑ가 사람의 승인 정문이다 — **답이 곧 승인**이다.
  종전의 ⓒ("전진 없는 attempt 3회 = `MAX_ATTEMPTS_BEFORE_HITL`")는 2026-09-05 삭제됐다 — 그 3 은 어떤
  실측에서도 오지 않았고 정지 결정은 이미 `--apply` 가 쥐고 있었다. 전진 없는 연속 수는 이제 dry-run 이
  **보여주고**(집행된 예산 바닥·직전 소진 여부와 함께), 멈출지는 그 화면을 본 쪽이 정한다.
- **우선순위는 서브의 선언에서 나온다**: `library_request[].blocking` 을 메인이 **읽는다**(2026-09-04
  이전에는 읽는 코드가 0 이었다). 대기 목록은 `blocking` 우선, 그 안에서 attempt 순이다.
  요청 표면화는 **`hitl.needed` 를 요구하지 않는다** — 규약대로 `library_request[]` 만 실은 턴이
  침묵 누락되던 결함의 교정이다.
- **예산 바닥은 집행된 사실이다**: 한 context 안에서 예산은 내려가지 않으며, **서브에 닿지 못한
  attempt**(전송·스키마 실패)의 요청 예산은 바닥으로 치지 않는다(거절된 요청이 릴레이를 잠그던 형태).

### 2.7.8 그라운딩 교환 포맷 — 서브 요청 → 메인 색인 → 반출 (신설 2026-08-22 · `plan_26082214` §4.2)

> 헌법 **불변식 B** 의 "어떻게". 헌법은 *왜 인용 없는 결정이 성립하지 않는가*를 말하고, 이 절은
> *그래서 무엇을 어떤 모양으로 주고받고 무엇이 기계 판정인가*를 정한다.
> 계약 = `sub_node/library-exchange.schema.json`(추적 PII-free 정적계약, 서브로 복제) ·
> 판정기 = `scripts/library_exchange.py`(**메인 전용** — 서브는 판정 주체가 아니다).
> 원리 차용 = `docs/report/ttsc-evidence-graph-principles-implementation.md` §1·§5.

**분업이 설계의 전부다 — *누락은 기계가, 거짓은 사람이.*** 서브가 "이 근거로 이렇게 빌드했다"고 말할
때, *그 근거가 실제로 반출된 항목인지·빠진 인용은 없는지*는 결정론으로 답할 수 있다. 반면 *그 근거가
이 결정을 정말 뒷받침하는가*는 사람이 읽어야 한다. 판정기는 앞엣것만 한다 — 뒤엣것을 흉내 내면 판정이
확률론이 되고 게이트가 무의미해진다.

**지식 비대칭은 유지한다.** 도서관(`__llm-wiki`)과 사서(`wiki-desk`)는 **메인 단독**이며 서브로
복제되지 않는다("DB 한 곳, 차등 접근권한"). 그래서 메인이 내보내는 것은 도서관이 아니라
**경로 + 앵커 + digest + 발췌**다 — 반출을 복제로 바꾸는 것이 비대칭을 깨는 실제 경로이므로,
발췌 예산 초과는 `EXPORT_EXCEEDS_EXCERPT_BUDGET` 로 거부한다.

#### 세 메시지 (전송은 신설하지 않는다 — 기존 A2A 통로에 실어 보낸다)

| # | `kind` | 방향 | 필수 블록 | 뜻 |
|---|---|---|---|---|
| ① | `library.citation.request` | 서브 → 메인 | `claim` · `query` | "이 **결정**을 뒷받침할 근거를 달라". 서브는 도서관을 뒤지지 않는다 — 무엇을 찾는지만 말한다 |
| ② | `library.resolution.export` | 메인 → 서브 | `resolution` · `references` | 사서가 색인한 결과 중 **반출 가능한 항목**. `status` ∈ resolved\|partial\|**unresolved**\|refused |
| ③ | `library.citation.attestation` | 서브 → 메인 | `claim` · `citations` · `decision` | "이 결정은 이 ref 들을 인용해 내렸다"는 자기귀속 |

- 셋은 `exchange_id` 로 묶인다. 갈리면 `EXCHANGE_ID_MISMATCH` — 어느 반출이 어느 결정을 뒷받침했는지
  추적 불가가 되므로 의미 판정을 **아예 진행하지 않는다**.
- **한 오브젝트는 한 종류만 담는다**(`KIND_BLOCK_FOREIGN`). 스키마의 `additionalProperties:false` 는
  "계약에 없는 키"만 막고 "이 종류에 없어야 할 블록"은 못 막으므로, 그 경계는 판정기가 닫는다.

#### 증거그래프 어휘 대응

| 증거그래프 | 여기서 | 내용 |
|---|---|---|
| **claim** (빚을 지는 쪽) | `claim` | 인용 의무를 지는 **결정**. `kind` ∈ build-decision · serve-decision · version-pin · patch-slot · *informational* |
| **reference** (빚의 원천) | `references[]` | 도서관 항목. `ref_id` · `path` · `anchor` · **`digest`** · `excerpt` · `authority` |
| **acknowledgement** | `citations[]` | claim → reference 의무 이행. `ref_id` · `digest` · `reason` |
| **resolution** | `resolution.status` | 사서의 해소 상태. **`unresolved` 는 정직한 공백**이지 실패가 아니다 |

- `authority` 는 wiki-desk 규약(**실행진실 > 계획의도**)을 그대로 쓴다 — 서브가 상충하는 참조를
  받았을 때의 우선순위다.
- `informational` 은 **인용 의무 모집단 밖**이다(질의만 하고 결정하지 않는 claim). 그것으로 결정을
  채택하면 의무를 우회하는 셈이라 `HOST_INELIGIBLE` 로 막는다.

#### 하드게이트 — 세 질문 + Freshness

```bash
python3 .claude/skills/terraforming_node/scripts/library_exchange.py \
        gate --request <req.json> --export <exp.json> --attestation <att.json>
# 0 = 통과 · 5 = 게이트 위반(fail-closed) · 2 = 사용오류
```

| 질문 | 무엇을 묻나 | 위반 코드 |
|---|---|---|
| **Resolution** | 모든 인용이 반출 항목 **정확히 하나**로 해소되는가 | `CITATION_UNRESOLVED` · `EXPORT_DUPLICATE_REF_ID` |
| **Host eligibility** | 인용을 단 결정이 **의무 모집단**인가 | `HOST_INELIGIBLE` |
| **Coverage** | 채택된 모든 결정이 **최소 하나**의 인용을 갖는가 | **`GROUNDING_OMISSION`** |
| *+ Freshness* | 인용 시점 digest 가 반출 시점 digest 와 같은가 | `CITATION_STALE` |

- **`GROUNDING_OMISSION` 이 불변식 B 의 집행점이다.** *"아무것도 인용하지 않는 산출물은 '필요했다'는
  증거가 없고, 반출되지 않은 것을 인용하는 산출물은 '무엇'도 증명하지 못한다."*
- **도서관에 근거가 없을 때의 올바른 행동은 강행이 아니다.** `resolution.status` 가
  `unresolved`/`refused` 인데 서브가 `decision.accepted: true` 를 내면 omission 이며, 판정기가
  그 경우 **`accepted:false` + HITL 에스컬레이션**이 정답임을 메시지에 함께 적는다.
- **Freshness 는 `requireReview` 차용**이다 — 인용된 것이 바뀌면 인용은 만료된다. 조용히 유효한 척하면
  낡은 근거 위에 선 결정이 초록불로 남는다.
- 판정기는 **서브 디스크를 읽지 않는다**(메시지만 본다) — §2.7.2 스캔 금지와 push-attestation 보존.

#### 배선 — 어디서 도는가

| 단계 | 주체 | 행위 |
|---|---|---|
| ① 요청 | 서브(A2A remote agent) | `library.citation.request` 를 task-report 통로로 반환 |
| ② 색인·반출 | **메인 + `wiki-desk`** | 사서가 색인 → 반출 가능분만 `library.resolution.export` 로 회신 |
| ③ 귀속 | 서브 | 결정과 함께 `library.citation.attestation` 반환 |
| ④ **판정** | **메인** | `library_exchange.py receive` — 통과해야 결정이 성립. 실패는 서브로 feedback |

- ✅ **자동 호출 배선 완료**(2026-08-22 · W-5). ④ 의 정문은 `gate` 가 아니라 **`receive`** 다 —
  `gate` 는 "이 셋이 정합한가"만 알고 *"지금 물어야 하는가"* 를 모른다. 그래서 2026-08-22 E2E 에서
  gate 를 돌린 주체는 파이프라인이 아니라 **사람**이었다(`testlog_26082215` §4.5).
  `receive` 는 리포트 자신에서 **인용 의무를 판정**하고(`phase ∈ {config,build,serve}` ∧
  `status=completed`), 의무가 있는데 교환 3메시지가 없으면 `GROUNDING_EXCHANGE_ABSENT` 로 거부한다.

```bash
# 위임과 수신을 한 명령으로 묶는다 — 두 단계로 두면 두 번째를 건너뛸 수 있다.
python3 .claude/skills/terraforming_node/scripts/library_exchange.py receive \
        --invoke-request <agent-control-request.json> --exchange-dir <교환 3메시지 디렉터리>
# 이미 받아 둔 산출로 판정할 때:
#   receive --agent-control-result <result.json> …   (result.output 에서 리포트를 꺼낸다)
#   receive --report <task-report.json> …
```

- **결속을 함께 본다**: `attestation.node_id` 가 리포트 `node_id` 와 다르면 `EXCHANGE_NODE_MISMATCH`
  로 거부한다 — 다른 노드/다른 결정의 통과한 교환을 재사용해 게이트를 우회하는 경로를 닫는다
  (M-2 가 노출한 `execution_approval` 재인가 결함과 **같은 형태**이며, 그쪽은 W-8 로 남아 있다).
- **유보에는 의무가 없다**: `input-required`·`failed` 는 채택이 아니므로 인용을 요구하지 않는다.
  요구하면 *"막혔다"고 정직하게 보고하는 경로가 오히려 벌을 받는다.*
- 전송은 여전히 헌법 소유다 — `receive` 는 `agent_control.py invoke`(provider-neutral)를 부르고
  **자기 전송을 만들지 않는다**(헌법=왜 / 스킬=어떻게 경계 유지).
- 양 토폴로지 공통이다(§2.7.0 분기표) — 도서관 비대칭은 sub 가 Ray 워커든 A2A 에이전트든 같다.

### 2.7.9 노드 오케스트레이터 — Phase 감독 (신설 2026-09-05 · `plan_26090516` §7.4 · H1)

축 C/D 결정(2026-09-05): 런타임 3종 스킬이 도는 Phase 에는 **`output/**` 산출물이 메인↔서브 데이터 통신으로
오가지 않는다.** 완수 뒤 발행 Phase 에만, 발행에 필요한 정보가 **문서 평면으로** 서브→메인에 온다.
헌법의 "상향 회수 = 문서기반" 불변식은 개정하지 않는다 — 오케스트레이터는 그 불변식을 Phase 로 집행한다.

| Phase | 서브 | 메인(오케스트레이터) | 노드 간 이동 |
|---|---|---|---|
| **install** | (사람 request) 클론 → 카나리 | terraforming: `--peer-ssh` 실측 → 서브 manifest 발행(`scan_node.py --emit-sub-manifest`) → Agent_Card v2 렌더·서명 → **설치 오버레이** 배달(`sync_to_sub --provision`) → model-less 카나리 | 설치 산출물(오버레이)만 |
| **config · build · serve · bench** | 자율(도서관 인용은 `library_request[]` 로 요청) | 릴레이 감독(원장 append-only · 재개 결정은 에이전트 · §2.7.7) | **없음** — `output/**` 이동 ✗ · 이미지 전송 ✗ |
| **publish** | `docs/` 에 문서 발행: sweep map · benchmark 인증서/리포트 · devlog/testlog · **hint 입력 사이드카** `docs/benchmark/hint_inputs_<measured_utc>/`(렌더된 Dockerfile·compose·3+1+1·바깥영역 산출물의 사본) → task-report 에 경로 | `fetch_sub_docs.sh` 로 `docs/` 회수(simlog raw 제외) → devlog·benchmark 저작(서브 결과 신뢰 — raw 재요구 ✗) → hint 발행 입력 | 문서 평면만 |

- **책임 = terraforming_node** (사용자 결정 · 발견≠소유). 인증서가 무엇을 담는가는 `adversarial-benchmark`,
  hint 가 무엇을 싣는가는 `hint-publisher` 가 소유한다 — 오케스트레이터는 경계·Phase 전이·형식 검사만 집행한다.
- Phase 전이의 근거는 서브 task-report 의 `phase`·`status`(schema-valid)다. 메인은 서브 디스크가 아니라 **리포트와 문서**를 검증한다.

### 2.7.10 Agent_Card v2 (A2A 1.0.1) · 서브 manifest · 서명 (신설 2026-09-05 · `plan_26090516` §7.2–7.3 · H1)

| 항목 | 정본 | 내용 |
|---|---|---|
| **표준** | `a2aproject/A2A` v1.0.1 `specification/a2a.proto`(JSON camelCase) | 필수: `name`·`description`·`supportedInterfaces[]`·`version`·`capabilities`·`defaultInputModes[]`·`defaultOutputModes[]`·`skills[]`. 선택: `provider`·`documentationUrl`·`securitySchemes`·`securityRequirements`·`signatures[]`·`iconUrl`. **표준 밖 최상위 키 금지** |
| **전송** | `supportedInterfaces[0]` | `url: ssh://<user>@<host>` · `protocolBinding: urn:easy-vllm:a2a-binding:ssh-claude-p:v1`(커스텀 바인딩은 **URI** — 표준 §5.8·§12.7) · `protocolVersion: "1.0"`. 전송 인증 = SSH 공개키(표준 SecurityScheme 5종 밖 → `securitySchemes` 비움 · 차이는 comms.md 가 문서화) |
| **노드 역할** | `capabilities.extensions[urn:easy-vllm:ext:node-role:v1]` | §2.7.6(c). 값은 `node_role_contract.py` 해소 · 렌더러는 적기만 |
| **skills 6** | `inspect`·`config`·`build`·`serve`·`bench`·`publish` | `publish` = §2.7.9 발행 Phase. 6종 미만이면 계약 위반 |
| **HW 사실** | **카드에 없다** → 서브 manifest | 카드 = "무엇을 할 수 있나"(A2A 평면) · manifest = "무엇 위에서 도나"(HW·경로·획득 모드) |
| **서브 manifest** | `scan_node.py --topology single --peer-ssh <sub> --model-source <m> --emit-sub-manifest output/single/sub_manifest.yaml` | 메인이 `--peer-ssh` 로 실측(HW 5종·모델 환경·egress)해 조립 · 스키마 = 메인 manifest + `self_role: sub` + `terraforming.issued_by: main`. 서브는 HW 스캔 권위 데이터를 스스로 만들지 않는다. `render_sub_env.py --sub-manifest` 가 스테이징 `output/<topology>/manifest.yaml` 로 넣고 **설치 오버레이**가 배달한다(빌드킷 배달 평면 D10 과 무관) |
| **서명** | `agent_card_contract.py keygen/sign/verify` | A2A §8.4: `signatures[]` = JWS(`EdDSA`/Ed25519 · `typ: JOSE` · `kid` = JWK 지문) over JCS(RFC 8785) 정규화(카드 − `signatures` − 기본값 필드). 키 = 설치 때 에이전트가 1회 생성(`output/<topology>/a2a_signing/` · 패스프레이즈 없음 · 0600 · 비추적). 공개키(JWK)는 `.claude/a2a/trusted_keys.json` 으로 서브에 배달. **사람 개입 0** — 매 작업·세션·캠페인마다 묻게 되면 서명을 걷어낸다(H1 조건 · plan §5) |
| **검증기** | `agent_card_contract.py validate/verify` · `render_sub_env --self-test` | proto 필수 집합·바인딩 URI·확장 params 계약·float 금지·서명 왕복·위조 감지. 렌더는 서명 직후 자기 검증(RED 를 배달 전에) |
| **정체성 증명** | ③단계(E1) | 위임키(실행 허가)는 **서명 카드 검증**으로 격하된다 — 게이트는 서브 manifest 의 Flag(`issued_by: main`)로 통과하고, 카드 서명이 "메인 발급"을 증명한다 |

## 3. 결정론 vs 판단 분리
| 결정론 (스크립트) | 판단 (이 페르소나) |
|---|---|
| **`staleness_gate.py`(조건부 preflight 트리거 — manifest/Flag/HW드리프트/attestation 나이 3축, `--now` 주입·벽시계 ✗)** · `scan_node.py`(스캔·게이트·3자-일치·manifest 블록·**emit_gate=토폴로지 미선언 emit fail-closed**) · `render_sub_env.py`(manifest→10아티팩트 렌더/복제·미치환/필수 검증) · **`node_role_contract.py`**(토폴로지 축 계약 — sub_mode 파생/선언일치·rank·정체성 권위·배달 평면, 출처 필드 동반) · **`library_exchange.py`**(그라운딩 3질문+Freshness — **누락** 판정만; "이 근거가 정말 뒷받침하나"는 판단 칸) · sync_to_sub 체크섬 · `install_host_safety.sh`(설치·검증 — 실행 트리거는 HITL) | **토폴로지 진입 인터뷰(§0.5)** · fresh-clone 온보딩 능동제안 · 5-전제조건 인터뷰 · 사용자 승인 · 브랜치≠토폴로지 시 브랜치전환 안내 · ib_write_bw 오케스트레이션 · **호스트 안전체계 세션 최종 Y/N 설명·승인(§2.6 — 선택조항)** · manifest 기입 승인 · 전달(--provision) 승인 · 카나리 결과 판정 · **인용의 진위 리뷰(§2.7.8 — 거짓은 사람이 본다)** · 모호 시 중단·질의 |

회귀 고정(전부 하드웨어·네트워크 불요) — **케이스 수는 여기 적지 않는다**(파생 가능한 값을 손으로 적으면 반드시 낡는다.
2026-09-03 이전 목록은 34/21/6/17 이라 적혀 있었고 실측은 43/26/8/36 이었다). 실행자는
`.claude/policies/runtime/verify_distribution.py` 이며 **7종 전부**가 그 하네스에서 돈다(2026-09-03 S3 배선 —
그전에는 scan·render 2종만 돌았고, 불변식 B 의 유일한 기계 집행점인 `library_exchange` 를 포함해 4종에 호출자가 0이었다):
`scan_node.py` · `render_sub_env.py` · `node_role_contract.py` · `library_exchange.py` · `staleness_gate.py` ·
`manifest_contract.py` · `node_blackbox/node_identity.sh` (+ `bootstrap_canary.py`).

## 4. 보조 파일
- `scripts/staleness_gate.py` — **조건부 preflight 결정론 트리거**(`--topology`·`--repo`·`--observed`·`--now`·`--max-age-days`·`--self-test`). 3축(manifest/Flag · HW 드리프트 · attestation 나이) → 안정 reason code. 미평가 축은 `skipped:*` 로 음성정직 표기(조용한 통과 ✗). **HW 축의 interconnect 6필드는 topology=single 에서 비교하지 않는다**(2026-08-21 · approved_by AhnSangHun) — single 은 분산서빙을 안 해 interconnect 를 쓰지 않으므로 manifest 의 "미사용" 선언 ↔ 실측 RoCE 발산은 의도된 것이다. `scan_node.evaluate_gate` 의 single 의미론(RoCE 존재 = warning, gate note "interconnect 스캔 skip(실패 아님)")과 정합. **multi 는 유지**(텐서패브릭이므로 완화 ✗). 제외 적용 시 `notes` 에 표기한다(침묵 ✗).
- `references/agent-control-adapter.md` — **provider 전용 실행문법 경계**(서브 위임·카나리). 본문은 의도만, 문법은 여기서만.
- `scripts/scan_node.py` — 결정론 스캔 코어. 플래그 전수: `--topology`·`--peer-ip`·`--peer-port`·`--peer-ssh`·`--sub-work-dir`·`--compose`·`--env-interconnect`·`--manifest`·`--bandwidth-gbps`·`--per-port-gbps`·`--per-port-floor`·`--bw-floor`·`--check-egress`·`--model-source`·`--emit-manifest`·`--self-test`. (2026-09-03 이전 목록은 6개만 적어 라이브 조합을 감췄다 — B1.)
- `scripts/bootstrap_canary.py` — **§2.5 완료 게이트 실행자**(`--topology`·`--manifest`·`--emit`·`--invoke`·`--self-test`).
- `scripts/render_sub_env.py` — 결정론 렌더러(manifest→`output/multi/sub_provision/` 스테이징·`--self-test`).
- `scripts/node_role_contract.py` — **토폴로지 축 노드 계약의 단일 소유자**(§2.7.0·§2.7.6b). `evaluate --topology <t> --field {sub_mode,rank,identity_authority,delivery_plane} --format {json,value}` · `--self-test`. 배달 평면 판정의 **정본**이며 `role: sub` 존재로 추론하지 않는다. ✅ 소비자 배선 완료(2026-08-22): `sync_to_sub.sh:_single_extension_active`(배달 평면) · `render_sub_env.py::_contract_placeholders`(Agent_Card 4필드). ⚠ venv `-S` shim 을 포함한 `load_yaml` 을 자체 보유한다 — `staleness_gate._load_yaml` 과 **같은 shim 이 두 곳에 있다**. 지금은 의도된 비결합(preflight 게이트가 이 파일 부재로 죽지 않게)이며, 갈라지면 신호는 두 파서의 판정 불일치로 온다.
- `scripts/relay.py` — **메인↔서브 턴제 릴레이 실행자**(§2.7.7a). `--task|--task-file` 첫 위임 · `--continue [--apply]` 자율 재개 · `--emit-only` · `--self-test`. 원장 `campaigns/<camp-id>/relay/<ctx>.json`, 대기 요청 같은 자리의 `pending_hitl.json`(사람이 `answer` 를 적는 자리). 경로 파생의 단일 소유자는 `scripts/campaign_init.py --derive`.
- `scripts/turn_budget.py` — **턴 예산 선언 검증기**(`--max-turns`·`--timeout-seconds`·`--source`·`--caps`·`--self-test`). 값을 만들지 않는다 — 미선언·상한초과·출처없음은 fail-loud. 전송 상한은 요청 스키마에서 **읽는다**(상수 복제 ✗). 등급표는 2026-09-05 폐기(G-A2).
- `scripts/library_exchange.py` — **그라운딩 교환 판정기**(§2.7.8, 메인 전용). `validate --file <msg>` · `gate --request/--export/--attestation` · `--self-test`. 서브 디스크를 읽지 않는다(메시지만 본다).
- `sub_node/` — 추적 PII-free 템플릿·정적계약: `CLAUDE.template.md`·`Agent_Card.template.json`·`settings.local.template.json`·`comms.md`·`task-report.schema.json`·**`library-exchange.schema.json`**(§2.7.8 그라운딩 교환)·`gitignore.template`.
- 메인↔서브 [전달]·[서빙 스모크]는 `upstream-version-watch`(`sync_to_sub.sh` — `--provision` 에 에이전트환경 오버레이 포함 · `multinode_serve_smoke.sh`).
- **호스트 안전체계 — 설치자는 승계됐고 협역 워치독은 정본으로 남았다**(2026-08-18 정밀화). 두 디렉터리를 뭉뚱그리면 살아 있는 자산을 레거시로 오인해 **보호층을 걷어내게 된다.** 두 뿌리는 `.claude/skills/terraforming_node/scripts/node_blackbox/` 와 `.claude/skills/terraforming_node/scripts/host_safety/` 이며, 레포 루트 `scripts/` 가 아니다(자기완결 재배치 완료 — 그 경로는 tombstone).

  | 자산 | 상태 | 소유·호출자 |
  |---|---|---|
  | `node_blackbox/install_node_blackbox.sh` · `verify_node_blackbox.sh` | **현행 설치자·검증기**(L1/L2/L3) | §2.6 Y 분기 · HITL sudo |
  | `node_blackbox/publish_install_request.py` | **현행** — L3 지시서 발행 | §2.6.1 |
  | `node_blackbox/mem_watchdog_eta.sh` · `blackbox_*.py` · `logs_lifecycle.py` · `regen_envelope.py` · `node_identity.sh` | **현행** — systemd 상시층·데이터 평면 | 설치자가 배치 |
  | `host_safety/mem_watchdog.sh` | ★ **정본으로 유지** — **협역(harness-scoped) 워치독**. `policy:HOST_SAFETY_LAYERED_DEFENSE.C1` 이 요구하는 계층이며, `multinode_serve_smoke.sh:274-276` 이 이를 *canonical* 로 부르고 **부재 시 exit 2** 로 죽는다 | `run_trial` · `multinode_serve_smoke.sh` 가 자동 기동 |
  | `host_safety/install_host_safety.sh` · `install_netconsole.sh` · `systemd/` · `host/` | **승계됨**(`plan_26073109`) | 잔재는 `node_blackbox/purge_host_safety.sh --require-seed` 로 제거 |

  - 서브 materialization: 블랙박스 = `.claude/runtime/node_blackbox/` · 협역 워치독 = `.claude/runtime/host_safety/mem_watchdog.sh`(둘 다 `render_sub_env.py` 가 배달).
  - ⚠ **`--keep-up` 상주 서빙 중에는 협역 워치독이 `PPID=1` 로 떠 있는 것이 정상이다** — 스모크 스크립트가 워치독만 남기고 종료하는 설계다. **`docker ps` 로 그 필터에 매칭되는 실행 중 컨테이너가 있으면 좀비가 아니다.** 정리는 `--down`(정식 teardown 5단계) 또는 `reap_stale_watchdogs` 가 하며, **손으로 `kill` 하면 살아 있는 서빙의 보호층이 사라진다.**
  - 근거 = `plan_26071019`(계층 방어) · `plan_26073109`(블랙박스 승격) · `testlog_26073113`(kdump 반증).

## 5. 금지
- 사용자 승인 없는 자동스캔 / 서브노드 무단 프로빙.
- **HITL 없는 서브 work_dir 자동 신설**(R2 — §1.7). 성능(ib_write_bw) 미검증 멀티-ready 기입(fail-closed).
- 무증거 manifest 오버라이드 / topology를 브랜치와 어긋나게 기입(3자-일치 위반).
- **빌딩블럭 스킬(terraforming_node·upstream-version-watch)·manifest 를 서브에 전달 금지**(런타임블럭·렌더 산출물만 — 2.1/2.3).
- **무증거 빈 정체성 렌더 금지**(render_sub_env.py 필수 필드 누락 시 fail-loud) · 템플릿에 IP·호스트 baking 금지(PII-free).
- **카나리(§2.5) 미통과 시 done 선언 금지** · 서브 워크스페이스 재스캔으로 "검증" 대체 금지(push-attestation 위반).
- SSH 키 교환·물리망 구성 대행(Case A — 검증·가이드까지만).
- **에이전트의 무인 sudo 실행 금지** — 호스트 안전체계 설치(`install_host_safety.sh --apply`)의 실행 주체는 항상 사람(HITL — 설명·승인·검증까지가 스킬 역할).

## 6. 참조
- 진입 루틴: `docs/plan/plan_26062311` · `testlog_26062314` · `devlog_26062314`.
- **토폴로지-중립 재스코프**(개명 `terraforming_subnode`→`terraforming_node` · single 진입·토폴로지 인터뷰 게이트(§0.5)·fresh-clone 능동발동·드리프트 가드·scan `emit_gate` fail-closed): `docs/plan/plan_26063009_44_23` · 그라운딩 `seed/session_record_2026063008`.
- 에이전트 환경 구축: `docs/plan/plan_26062408`(개명+A2A) · 시드 `seed_9507ae1edc40` · 인터뷰 `interview_20260623_220341`.
- 통로/이관: `plan_26062312`(output/) · `plan_26062315`(manifest) · CLAUDE.md "산출물 통로 불변식".
- 다음(설계됨): docker-compose NCCL → manifest-driven 렌더(Plan 2) · recipe-explorer Phase-2 서브 자율 깊이(D8) · 하위스킬 호출 오케스트레이션 전반.
