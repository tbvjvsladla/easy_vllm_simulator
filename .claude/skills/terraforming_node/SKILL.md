---
name: terraforming_node
description: >-
  **토폴로지-중립 진입(온보딩) 오케스트레이터** — fresh-clone/미테라포밍 환경에서 능동 발동해 **첫 동작으로
  토폴로지(① 단일노드 ② N대 멀티노드)를 인터뷰로 확정**하고, 그에 따라 노드를 스캔·manifest.yaml(스킬 간
  단일 계약)을 채운다. **single → 단일노드 온보딩**(manifest nodes:[], sub-control dormant) · **multi →
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
- **multi** → 진입 루틴(§1: 5-전제조건 인터뷰 + 스캔 + 연결/성능 검증) + **서브 에이전트 환경 구축**(§2: 메인 렌더 → 서브 전달 → 카나리).

이후 `upstream-version-watch`(컨테이너 빌드) · `vllm-recipe-explorer`(서빙전략) 와 **파이프라인 의존순서**로 협력한다(헌법 §스킬 오케스트레이션 / 진입 척추).

> **구현 범위 = 토폴로지 진입 인터뷰 + (single)단일 온보딩 + (multi)서브노드 진입·환경구축**(plan_2026062311_1 진입·라이브 testlog_2026062314_1 / plan_2026062408_1 환경구축 / **plan_2026063009_1 토폴로지-중립 재스코프**).
> 환경탐지 전반·하위스킬 호출 오케스트레이션은 점진 확장(헌법 §스킬 경계).

> 설계 원칙(하네스 엔지니어링): **스캔·게이트·렌더·일치단언은 결정론 스크립트**(`scripts/scan_node.py`·`scripts/render_sub_env.py`),
> **인터뷰·승인·HITL 판정은 페르소나(이 문서)**. 둘을 섞지 않는다.

## 0. 전제 / 입력
- **토폴로지-중립 진입**: single·multi **공통** 발동. (과거 "multi 전용·single 비활성(α)"는 plan_2026063009_1 에서 폐기 — single 진입점 부재 = chicken-and-egg 갭이었음: "single이냐 multi이냐"를 묻는 주체가 multi일 때만 발동했음.) **첫 동작 = 토폴로지 인터뷰(§0.5)**. 이하 §1(진입 루틴)·§2(서브 환경구축)는 **topology=multi 분기**, single 은 §0.5→§1S 로 짧게 완결.
- **SSH = Case A**(multi 한정): 메인↔서브 패스워드리스 SSH는 **사전조건**(검증만, 키 교환·물리망 설정은 안 함).
- 계약 스켈레톤 = `manifest.template.yaml`(루트, 추적). **실값 = `output/<topology>/manifest.yaml`**(브랜치 파생 통로 — single=`output/single/`·multi=`output/multi/`; 비추적, plan_2026062315_1). **(multi) 서브엔 manifest 를 전달하지 않는다**(메인 단일계약 — render-on-main, D10).
- 검증 정본 = `devlog_250422`(git f3583f0, in-history) 실측 + seed PDF(repo-외부).

## 0.5 토폴로지 진입 인터뷰 게이트 (판단 — **첫 동작**, fail-closed)

> **chicken-and-egg 차단(plan_2026063009_1 D2·D3·D4)**: "이 HW가 single이냐 multi이냐"는 **스캔보다 먼저 인터뷰로 결정**한다. 브랜치는 작업공간 선택기일 뿐 토폴로지를 *결정*하지 않는다(브랜치⇒토폴로지 추론 금지 — 이게 갭의 직접원인이었음).

- **0.5.1 fresh-clone 능동 발동**(헌법 §스킬 오케스트레이션 척추): 미테라포밍 신호 = `config.yaml` 부재 **AND** `output/single/manifest.yaml` 부재 **AND** `output/multi/manifest.yaml` 부재. 감지 시 일반 오리엔테이션("뭐 할까요?")보다 **온보딩을 능동 제안**한다. 단 능동성 고도 = **제안**(감지→제안→사용자 응답→인터뷰→승인→스캔) — 자동 스캔 ✗("무단 스캔 금지" 보존).
- **0.5.2 토폴로지 인터뷰**(가장 먼저): *"이 HW 환경은 ① 단일노드(이 머신 1대)인가, ② N대 PC를 고속망으로 묶은 멀티노드(DGX Spark식 병렬)인가?"* 사용자 선언이 `scan_node.py --topology <답>` 의 `declared` 입력이 된다.
- **0.5.3 fail-closed(D3)**: 토폴로지 미선언 시 스캔/emit 금지. 결정론 백스톱 = `scan_node.py --emit-manifest` 가 `--topology auto`면 **거부(비0 종료 3 — `emit_gate`, --self-test 회귀)**. 추정 토폴로지로 manifest 기입 불가.
- **0.5.4 브랜치 ≠ 토폴로지(D4)**: 선언 토폴로지가 현재 git 브랜치와 어긋나면(예: `single-node` 브랜치인데 "multi" 선언) → `evaluate_gate` 3자-일치 단언이 **fail-closed(blocked·비0)** + **HITL 브랜치전환 안내**(`git checkout <single-node|multi-node>` 후 재개 — 스크립트 자동전환 ✗: 워킹파일을 바꾸는 행위라 사람이 한다). 정렬 후 진행.
- **0.5.5 분기**: **single** → §1S 단일노드 온보딩(짧음) · **multi** → §1 멀티노드 진입 루틴(5-전제조건 인터뷰부터).
- **0.5.6 드리프트 가드(D7, 경량)**: 온보딩 단계상태를 추적한다 — `토폴로지 결정 → 모델획득모드 → 0차 init-plan → 스캔 → 게이트 → manifest+Flag → 호스트 안전체계(세션 최종 Y/N·선택)`(single) / `+ 5-전제조건 → 성능 → manifest+Flag → 서브 환경구축 → 카나리 → 호스트 안전체계(세션 최종 Y/N·선택)`(multi). **호스트 안전체계는 Flag 발급 이후 세션 최종 선택조항**(§2.6 — 표준 절차 완수와 분리, plan_2026071115_1). 사이드퀘스트(예: GPU/드라이버 디버깅) 후 **미완 단계로 복귀**(미완을 사람 머릿속에만 두지 않음). 범용 워크플로 todo 시스템은 범위 밖(헌법 파킹).
- **0.5.7 모델 획득 모드 인터뷰 (토폴로지 직후 · 헌법 §모델 획득 모드 따름정리 3종 · plan_2026063018_1)**: *"모델을 어떻게 확보하나?"* — **managed**(사전 다운로드된 관리 NAS 경로 read-only 마운트) · **ephemeral**(컨테이너 내부 HF 캐시 임시 다운로드, 컨테이너 down→삭제 · **다수 기본**) · **custom**(지정 경로 저장·볼륨마운트). scan 이 `nas_model_path` 존재를 bool 탐지해 *"아마도 managed"* **제안**(사실=탐지·제안=판단). 답 → manifest `model_source`(+ `nas_model_path`/`custom_model_paths`/`hf_token_env_file` 포인터). **이 인터뷰 없으면 `manifest_contract` 가 info-only 유지**(Flag complete 만으론 불충분 — model_source valid 이중요건). 토폴로지-무관(single·multi 공통).
- **0.5.8 0차 init-plan 발행 (스캔 前 · "로그=에이전트" 철학 · 헌법 계획 게이트)**: 스캔 착수 전, 에이전트가 인터뷰 답에서 **real `docs/plan/` init-plan 을 *대신 초안***(토폴로지·획득모드·스캔할 HW·branch 정합·완료 시 Flag) → **배포자 자기-HITL 승인**(*불편하지 않게 유도* — 무게가 아니라 경험; 약간의 강요는 의도된 철학). 이 plan 은 `wiki-desk` 가 색인(자기개선 루프 *자동 기둥*) → 배포자가 Agent 를 능숙히 다루는 *수동 기둥* 습관화. 승인 후 §1S/§1.3 스캔.

## 1S. 단일노드 온보딩 (topology=single — 짧은 경로)

> §0.5 에서 single 확정 시. 서브가 없으므로 §1(5-전제조건/성능)·§2(서브 환경구축)는 **N/A**.

- **스캔**(결정론): `python3 scripts/scan_node.py --topology single` — interconnect 검증 skip(=α 정상).
- **게이트**: α — RoCE 하드웨어가 있어도 비blocking 경고(멀티 가능 머신의 단일 운용은 정상).
- **manifest 기입**(HITL): `--emit-manifest --topology single` 블록(YAML-valid · **single 시 `nodes: []` 도 결정론 emit** — dormant 게이트 동결, 수기 의존 ✗) → **사람 확인 후** `output/single/manifest.yaml` 반영. `nodes: []` → **sub-control dormant**(독립 self-containment 보존 — 헌법 §single-node 확장기능). **무증거 기입 금지.**
  - emit 블록은 **테라포밍 완수 Flag attestation**(`terraforming.complete/branch_verified`)을 §1.5 3자일치 통과 시에만 포함(보수적·미통과면 미발급) — **§0.5.7 `model_source` 도 함께 기입**해야 `manifest_contract` Flag valid(complete + valid model_source 이중요건). Flag 발급 = 3 런타임 스킬 작업 활성(헌법 §테라포밍-완수 Flag 게이트).
  - **Flag ↔ 호스트 안전체계 명시 분리**(plan_2026071115_1): Flag 발급은 표준 절차 완수이며 **안전체계 설치와 무관**(안전체계 미설치여도 Flag valid). 안전체계는 이 manifest+Flag 기입 **이후** §2.6 세션 최종 Y/N 선택조항에서 다룬다.
- **호스트 안전체계 세션 최종 Y/N** → **§2.6**(보험판매 톤 선택조항, 양 토폴로지 공통) 수행 후 완료.
- 온보딩 완료 → 파이프라인 다음 단계(`upstream-version-watch` 컨테이너 빌드).

## 1. 멀티노드 진입 루틴 (topology=multi — §0.5 에서 multi 확정 후)

> 핵심 불변식: **무단 자동스캔 금지** — "인터뷰 → 사용자 승인 → 스캔" 순서. **성능 미검증 멀티 진행 금지(fail-closed)**.

### 1.1 5-전제조건 인터뷰 (판단 — 사람에게 묻는다)
자동스캔 전에 멀티턴으로 확인:
①메인↔서브 **고속 RDMA 인터커넥트(예: ConnectX-7)** 연결 ②**네트워크** 구성 완료 ③메인↔서브 **SSH**(Case A) 구성 완료
④서브노드 **Claude Code 설치** ⑤서브 Claude Code **모델 연결**(로그인/API key — `claude -p` 실제 도달).

### 1.2 사용자 승인 게이트 (판단)
인터뷰 응답 수집 → 사용자가 **명시 승인**해야 스캔 시작.

### 1.3 스캔 (결정론 — `scripts/scan_node.py`)
- 로컬: `python3 scripts/scan_node.py --topology <single|multi> [--peer-ip <sub IP>] [--compose output/<topology>/docker-compose.yaml]`
- 탐지: cpu_arch(uname) · cuda(nvcc) · gpus(nvidia-smi) · interconnect(**/sys/class/infiniband + show_gids** — ibstat 비의존) · docker-compose NCCL 교차검증.
- 폴백: 탐지 도구 부재/빈 결과 → graceful(인터뷰 폴백 또는 α). hard-crash 금지.

### 1.4 성능 게이트 (결정론 판정 + cross-node 오케스트레이션)
- 합격선(plan §2.4): **포트당 ≥100 Gb/s & 합산 ≥180 Gb/s**(=200Gbps 풀대역폭 ~90%, devlog 218 기준).
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

### 1.7 서브 work_dir 프로비저닝 게이트 (HITL — R2 불변식, plan_2026062320_1)
서브에 산출물을 복제하려면 서브 작업경로(`nodes[role=sub].work_dir`)가 있어야 한다. **기본값 = 메인 work_dir 와 동일**. 그러나:
- **경로 신설은 반드시 HITL** — `sync_to_sub.sh --apply` 는 서브 work_dir 부재 시 정지·질의(exit 5). 사람 승인(`--provision`) 시에만 `mkdir -p` 후 전송.
- **HITL 없는 자동 경로 신설 절대 금지.** 경로값은 manifest 에서만 해소(하드코딩 금지).

## 2. 서브 에이전트 환경 구축 (A2A-개념 — plan_2026062408_1)

> 서브노드 코드에이전트가 메인과 **불투명 피어**로 협업하려면, 서브 워크스페이스에 **페르소나·능력·권한·
> 런타임스킬·통신프로토콜**이 있어야 한다. 진입 루틴(§1)이 manifest 를 채운 뒤, 이 단계가 그 환경을
> **메인에서 렌더해(render-on-main) 서브로 전달하고 카나리로 검증**한다.

### 2.1 스킬 분류학 (결정론↔자율성 충돌 해소)
- **빌딩블럭**(메인 전용, 서브 전달 ✗): `terraforming_node`·`upstream-version-watch`.
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
| `tasks/` | 런타임 상태 스캐폴드(파일=세션) | 빈 디렉토리 |

추적 템플릿·정적자산은 `sub_node/`(CLAUDE.template.md·Agent_Card.template.json·settings.local.template.json·comms.md·task-report.schema.json·gitignore.template) — **PII-free**(IP·호스트 비박음, 렌더 시 manifest 에서 치환). (`docs.md`·`docs/*/example.md` 는 메인 정본을 D12 복제 — sub_node/ 외부 원천.)

### 2.3 렌더-온-메인 → 전달 (결정론 + HITL)
- **렌더**(결정론): `python3 scripts/render_sub_env.py --topology multi` → manifest 노드정체성을 템플릿에 치환,
  gitignored 스테이징 `output/multi/sub_provision/` 산출(서브 루트 미러). 미치환 placeholder·필수 누락 시 fail-loud.
- **전달**(HITL): `bash .claude/skills/upstream-version-watch/scripts/sync_to_sub.sh --apply --provision`
  — 메인 rsync(코드) **이후** 스테이징을 서브 루트로 **오버레이(--delete 없음)**. 빌딩블럭 스킬·manifest 는 전달 안 됨(런타임블럭만).
- **단일 전달차**: 코드+에이전트환경 모두 sync_to_sub.sh 한 경로. dry-run 기본 → 사람 검토 후 --apply.

### 2.4 A2A-개념 협업 계약 (서버 없음)
- 메인=client(Task 발급·리포트 검증·피드백) · 서브=remote(자율 수행·자기검증 리포트 1개). A2A 어휘 차용, HTTP 서버 ✗(전송=`ssh claude -p`).
- **검증 = push-attestation**: 서브가 self-verification(config-parse·schema·runner 문법·checksum·**로컬 스모크**)을 리포트에 담아 회신 → **메인은 리포트만 검증, 서브 워크스페이스 재스캔 ✗**.
- **성공술어**: phase 별(comms.md). 예: config = triplet 생성 + 로컬 스모크 응답("린트 통과 ≠ 서빙됨").
- **상태=파일**: `tasks/<context_id>.json`("파일=세션"). **max-turns=3**(reconciliation_cap) 소진 → status=failed → 메인 **Model-C(HITL)**.
- per-task 휘발값(모델명·예산·NAS 서브디렉토리)은 **Task Message** 로(manifest 복제 아님).

### 2.5 완료 게이트 — model-less 카나리 라운드트립 (R1 정합)
전달 후 메인이 **모델 없이** 부트스트랩 Task 1회: `ssh <sub> claude -p '<inspect bootstrap>' --output-format json`
→ 서브가 새 CLAUDE.md+런타임블럭+settings+comms 로드, **phase=inspect·status=completed + self_verification** 의 schema-valid 리포트 반환.
이로써 "구성된 환경이 프로토콜대로 작동함"을 전체로서 증명(권한행·skill YAML·페르소나 비준수 포착 — 체크섬이 못 잡는 것). 실패 시 max-turns→Model-C. **카나리 미통과 시 done 선언 금지.**

- 카나리 통과 → **호스트 안전체계 세션 최종 Y/N**(§2.6, 양노드) 수행 후 온보딩 완료.

## 2.6 호스트 안전체계 — 세션 최종 선택조항 (Y/N · 양 토폴로지 공통 · 보험판매 톤 · plan_2026071115_1)

> **양 토폴로지 공통 최종 스텝**: single 은 §1S manifest+Flag 기입 직후 · multi 는 §2.5 카나리 통과 직후 이 절로 온다.
> **Flag 발급 이후**에 오는 **독립 Y/N 선택조항**이다 — 표준 온보딩(스캔·게이트·manifest·Flag)은 이미 완수됐고, 안전체계는 **선택**이다(설치 안 해도 Flag valid). 헌법 §호스트 안전체계 따름정리(선택화).

- **성격**: 절차적 필수 스텝이 아니라 **보험판매식 Y/N 권유**다 — 배포 사용자에게 상시 데몬 설치를 강제하면 반발이 있으므로, *혜택을 서술*해 권하되 **강제·차단·반복 잔소리 금지**(D2·D3·D33). "설치하면 안정성이 향상된다"는 톤으로 꼬시되 거부는 존중한다.
- **① 혜택 서술(talking point 예시 — 실문구는 재량)**:
  - "768k prefill 사건에서 워치독이 컨테이너를 먼저 정리해 호스트를 지킨 실적이 있습니다 — 설치하면 이 보호막이 상시 작동합니다."
  - "kdump 가 있으면 시스템이 멈춰도 사후 분석용 덤프가 남아 원인을 추적할 수 있습니다."
  - (통합메모리 GPU 한정) "이 GPU 는 시스템 메모리를 공유해 GPU OOM 이 곧 호스트 다운입니다 — 워치독이 그 직전에 개입합니다."
- **② Y 분기(설치)**: 승인 시 **사용자 실행** `sudo bash scripts/install_host_safety.sh --apply [--with-kdump]`(dry-run 선행 가능 — 무엇을·왜·트레이드오프 고지) → **에이전트 무인 sudo 실행 ✗**(실행 주체는 사람). 검증(결정론): `systemctl is-active easy-vllm-memwatch` = active · `sudo -n /usr/local/sbin/vllm-drop-caches` 무암호 동작. 설치 계층 = mem_watchdog systemd 상시(관측+보호킬 — "무인 자동실행 없음" 원칙의 **명시 예외**) · earlyoom 최후선 · sudoers 단일 헬퍼(`vllm-drop-caches` 경로 1개만 NOPASSWD) · (선택) kdump(**재부팅 1회** + crashkernel RAM 예약). → manifest `host_safety.installed: true`.
- **③ N 분기(미설치·opt-out)**: **차단 없음** — 서빙은 정상 진행. manifest `host_safety.installed: false`(중립 기록 — 비난 톤 ✗). 재권유는 **세션당 1회 이하**(시끄러움 방지).
  - **통합메모리 노드 한정 후속 경고**: opt-out + 통합메모리(GPU OOM=호스트 하드다운 위험) 노드는, 이후 서빙 기동 직전 **에이전트 채팅창 1줄** 안내만 한다("워치독 미설치 상태 — 통합메모리라 OOM 시 호스트 다운 위험, `install_host_safety.sh` 로 언제든 보강 가능"). **serve 스크립트/로그 배너 코드변경 ✗**(시끄러운 경험 방지 — D31·NG-5). **discrete GPU 노드는 무경고.**
- **④ 파급 정밀화(opt-out 이어도 보호 일부 유지)**: 하네스 **협역 워치독**(`run_trial`·`multinode_serve_smoke.sh` 자동 기동)은 레포 내장 스크립트라 **설치와 무관하게 계속 작동**(opt-out 사용자도 trial 중 보호 유지). 로드-전 RAM 게이트(⑤.5)의 `vllm-drop-caches` 자동 드랍만 헬퍼 부재로 skip 되며, 게이트는 이를 **음성정직으로 보고**(드랍 없이 재측정 → 부족 시 기동 거부 exit 7 유지 — `preload_ram_gate.try_drop_caches` 기구현 graceful).
- **⑤ 멀티노드 변형**: 양노드(메인+서브) 각각 동일 Y/N. **서브 설치는 렌더 배달분**(`scripts/install_host_safety.sh` — §2 오버레이 셋 포함)으로 **서브에서 사용자가 실행**(A2A 경계 — 메인 sudo 대행 ✗). manifest `nodes[].host_safety.installed` 로 **노드별 독립** 기록.

## 3. 결정론 vs 판단 분리
| 결정론 (스크립트) | 판단 (이 페르소나) |
|---|---|
| `scan_node.py`(스캔·게이트·3자-일치·manifest 블록·**emit_gate=토폴로지 미선언 emit fail-closed**) · `render_sub_env.py`(manifest→10아티팩트 렌더/복제·미치환/필수 검증) · sync_to_sub 체크섬 · `install_host_safety.sh`(설치·검증 — 실행 트리거는 HITL) | **토폴로지 진입 인터뷰(§0.5)** · fresh-clone 온보딩 능동제안 · 5-전제조건 인터뷰 · 사용자 승인 · 브랜치≠토폴로지 시 브랜치전환 안내 · ib_write_bw 오케스트레이션 · **호스트 안전체계 세션 최종 Y/N 설명·승인(§2.6 — 보험판매 톤 선택조항)** · manifest 기입 승인 · 전달(--provision) 승인 · 카나리 결과 판정 · 모호 시 중단·질의 |

회귀 고정: `python3 scripts/scan_node.py --self-test`(게이트 9 + emit_gate fail-closed 4 + emit-block None-leak 2 = 15케이스) · `python3 scripts/render_sub_env.py --self-test`(렌더 4케이스). 둘 다 하드웨어 불요.

## 4. 보조 파일
- `scripts/scan_node.py` — 결정론 스캔 코어(`--topology`·`--peer-ip`·`--bandwidth-gbps`·`--bw-floor`·`--emit-manifest`·`--self-test`).
- `scripts/render_sub_env.py` — 결정론 렌더러(manifest→`output/multi/sub_provision/` 스테이징·`--self-test`).
- `sub_node/` — 추적 PII-free 템플릿·정적계약: `CLAUDE.template.md`·`Agent_Card.template.json`·`settings.local.template.json`·`comms.md`·`task-report.schema.json`·`gitignore.template`.
- 메인↔서브 [전달]·[서빙 스모크]는 `upstream-version-watch`(`sync_to_sub.sh` — `--provision` 에 에이전트환경 오버레이 포함 · `multinode_serve_smoke.sh`).
- 호스트 안전체계(레포 루트 `scripts/`): `install_host_safety.sh`(결정론 설치자 — HITL sudo) · `mem_watchdog.sh`(광역/협역 이중 모드) · `systemd/easy-vllm-memwatch.service` · `host/vllm-drop-caches.sh`. 근거 = plan_2026071019_1.

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
- 진입 루틴: `docs/plan/plan_2026062311_1` · `testlog_2026062314_1` · `devlog_2026062314_1`.
- **토폴로지-중립 재스코프**(개명 `terraforming_subnode`→`terraforming_node` · single 진입·토폴로지 인터뷰 게이트(§0.5)·fresh-clone 능동발동·드리프트 가드·scan `emit_gate` fail-closed): `docs/plan/plan_2026063009_1` · 그라운딩 `seed/session_record_2026063008`.
- 에이전트 환경 구축: `docs/plan/plan_2026062408_1`(개명+A2A) · 시드 `seed_9507ae1edc40` · 인터뷰 `interview_20260623_220341`.
- 통로/이관: `plan_2026062312_1`(output/) · `plan_2026062315_1`(manifest) · CLAUDE.md "산출물 통로 불변식".
- 다음(설계됨): docker-compose NCCL → manifest-driven 렌더(Plan 2) · recipe-explorer Phase-2 서브 자율 깊이(D8) · 하위스킬 호출 오케스트레이션 전반.
