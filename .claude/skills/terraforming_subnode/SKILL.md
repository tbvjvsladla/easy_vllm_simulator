---
name: terraforming_subnode
description: >-
  멀티노드 **서브노드**를 스캔·인터뷰해 manifest.yaml(스킬 간 단일 계약)을 채우고, 메인↔서브 연결·성능을
  검증한 뒤 **서브노드 코드에이전트(Claude Code) 작업환경을 구축**하는 오케스트레이터. 싱글노드엔 발동 안 함.
  구축물 = 메인에서 렌더해 전달하는 5+아티팩트(서브 페르소나 CLAUDE.md·Agent_Card.json·스코프드
  settings.local.json·런타임블럭 vllm-recipe-explorer·A2A-개념 통신프로토콜) + model-less 카나리 검증.
  "테라포밍", "서브노드 셋업", "노드 스캔", "멀티노드 구성 검증", "메인 서브 연결 확인", "manifest 생성",
  "인터커넥트 점검", "서브 에이전트 환경 구축", "서브 코드에이전트 셋업" 같은 지시에 발동.
  무단 스캔 금지 — 5-전제조건 인터뷰 + 사용자 승인 후 스캔. 성능 미검증 멀티 진행은 fail-closed로 차단.
---

# terraforming_subnode

멀티노드 분산 vLLM 을 쓰려면 (1) 서브노드 환경 구체값(토폴로지·노드·인터커넥트·NAS·CUDA)을 알아야 하고,
(2) 서브노드의 **코드에이전트가 메인과 A2A-개념으로 협업할 작업환경**이 있어야 한다. terraforming_subnode 는
**둘 다** 책임진다 — 진입 루틴(스캔+인터뷰 → `manifest.yaml`)과 **서브 에이전트 환경 구축**(메인에서 렌더 →
서브 전달 → 카나리 검증). **싱글노드엔 발동하지 않는다**(서브가 없으므로). 이후 `upstream-version-watch`·
`vllm-recipe-explorer` 와 협력한다.

> **구현 범위 = 멀티노드 진입 루틴 + 서브 에이전트 환경 구축**(plan_2026062311_1 진입·라이브 testlog_2026062314_1 / plan_2026062408_1 환경구축).
> 환경탐지 전반·하위스킬 호출 오케스트레이션은 점진 확장(헌법 §스킬 경계).

> 설계 원칙(하네스 엔지니어링): **스캔·게이트·렌더·일치단언은 결정론 스크립트**(`scripts/scan_node.py`·`scripts/render_sub_env.py`),
> **인터뷰·승인·HITL 판정은 페르소나(이 문서)**. 둘을 섞지 않는다.

## 0. 전제 / 입력
- **멀티노드 전용**: topology=multi 에서만 발동. single 이면 서브가 없어 이 스킬은 비활성(α).
- **SSH = Case A**: 메인↔서브 패스워드리스 SSH는 **사전조건**(검증만, 키 교환·물리망 설정은 안 함).
- 계약 스켈레톤 = `manifest.template.yaml`(루트, 추적). **실값 = `output/multi/manifest.yaml`**(비추적, 통로 분리, plan_2026062315_1). **서브엔 manifest 를 전달하지 않는다**(메인 단일계약 — render-on-main, D10).
- 검증 정본 = `devlog_250422`(git f3583f0, in-history) 실측 + seed PDF(repo-외부).

## 1. 멀티노드 진입 루틴 (절차)

> 핵심 불변식: **무단 자동스캔 금지** — "인터뷰 → 사용자 승인 → 스캔" 순서. **성능 미검증 멀티 진행 금지(fail-closed)**.

### 1.1 5-전제조건 인터뷰 (판단 — 사람에게 묻는다)
자동스캔 전에 멀티턴으로 확인:
①메인↔서브 **ConnectX-7(고속 인터커넥트)** 연결 ②**네트워크** 구성 완료 ③메인↔서브 **SSH**(Case A) 구성 완료
④서브노드 **Claude Code 설치** ⑤서브 Claude Code **모델 연결**(로그인/API key — `claude -p` 실제 도달).

### 1.2 사용자 승인 게이트 (판단)
인터뷰 응답 수집 → 사용자가 **명시 승인**해야 스캔 시작.

### 1.3 스캔 (결정론 — `scripts/scan_node.py`)
- 로컬: `python3 scripts/scan_node.py --topology <single|multi> [--peer-ip <sub IP>] [--compose output/<topology>/docker-compose.yaml]`
- 탐지: cpu_arch(uname) · cuda(nvcc) · gpus(nvidia-smi) · interconnect(**/sys/class/infiniband + show_gids** — ibstat 비의존) · docker-compose NCCL 교차검증.
- 폴백: 탐지 도구 부재/빈 결과 → graceful(인터뷰 폴백 또는 α). hard-crash 금지.

### 1.4 성능 게이트 (결정론 판정 + cross-node 오케스트레이션)
- 합격선(plan §2.4): **포트당 ≥100 Gb/s & 합산 ≥180 Gb/s**(=200Gbps 풀대역폭 ~90%, devlog 218 기준).
- 측정 = `ib_write_bw`(서버 on 서브, 클라 on 메인). **견고 패턴**(원격 detach 함정 회피):
  ```bash
  ssh <sub> 'ib_write_bw -d <hca> -F' >/tmp/srv.log 2>&1 &   # 로컬에서 SSH 백그라운드(서버는 서브 foreground 유지)
  sleep 3; ib_write_bw -d <hca> -F <sub_RoCE_IP>             # 클라(메인) — BW average[MB/sec] ×8/1000 = Gb/s
  ```
  도메인별 측정 → 합산. 결과를 `scan_node.py --bandwidth-gbps <합산>` 으로 주입해 게이트 최종판정.

### 1.5 토폴로지 게이트 + 3자-일치 단언 (결정론 — `scan_node.evaluate_gate`)
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
- **빌딩블럭**(메인 전용, 서브 전달 ✗): `terraforming_subnode`·`upstream-version-watch`.
- **런타임블럭**(서브 복제 ✓): `vllm-recipe-explorer`. 서브가 **동일 결정론 엔진**을 자기 모델에 자율 실행 → 자율=실행 주체, 방법=결정론(헌법 "확률론 추론 금지" 보존).

### 2.2 구축 5+아티팩트 (서브 워크스페이스 레이아웃)
| 경로(서브 루트) | 내용 | 전달타입 |
|---|---|---|
| `CLAUDE.md` | 페르소나(Karpathy B1–B4, **노드정체성 bake**, 자율 triplet 저작 + 자기교정) | 렌더(gitignored) |
| `Agent_Card.json` | A2A 능력카드(skills[]·노드정체성) | 렌더(gitignored) |
| `.claude/settings.local.json` | **스코프드** 권한(블랭킷 ✗) | 렌더(gitignored) |
| `.claude/skills/vllm-recipe-explorer/` | 런타임블럭(git-tracked만) | 복제 |
| `.claude/rules/comms.md` | 통신 정적계약 | 복제 |
| `.claude/schemas/task-report.schema.json` | 자기검증 스키마 | 복제 |
| `tasks/` | 런타임 상태 스캐폴드(파일=세션) | 빈 디렉토리 |

추적 템플릿·정적자산은 `sub_node/`(CLAUDE.template.md·Agent_Card.template.json·settings.local.template.json·comms.md·task-report.schema.json) — **PII-free**(IP·호스트 비박음, 렌더 시 manifest 에서 치환).

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

## 3. 결정론 vs 판단 분리
| 결정론 (스크립트) | 판단 (이 페르소나) |
|---|---|
| `scan_node.py`(스캔·게이트·3자-일치·manifest 블록) · `render_sub_env.py`(manifest→7아티팩트 렌더/복제·미치환/필수 검증) · sync_to_sub 체크섬 | 5-전제조건 인터뷰 · 사용자 승인 · ib_write_bw 오케스트레이션 · manifest 기입 승인 · 전달(--provision) 승인 · 카나리 결과 판정 · 모호 시 중단·질의 |

회귀 고정: `python3 scripts/scan_node.py --self-test`(게이트 9케이스) · `python3 scripts/render_sub_env.py --self-test`(렌더 4케이스). 둘 다 하드웨어 불요.

## 4. 보조 파일
- `scripts/scan_node.py` — 결정론 스캔 코어(`--topology`·`--peer-ip`·`--bandwidth-gbps`·`--bw-floor`·`--emit-manifest`·`--self-test`).
- `scripts/render_sub_env.py` — 결정론 렌더러(manifest→`output/multi/sub_provision/` 스테이징·`--self-test`).
- `sub_node/` — 추적 PII-free 템플릿·정적계약: `CLAUDE.template.md`·`Agent_Card.template.json`·`settings.local.template.json`·`comms.md`·`task-report.schema.json`.
- 메인↔서브 [전달]·[서빙 스모크]는 `upstream-version-watch`(`sync_to_sub.sh` — `--provision` 에 에이전트환경 오버레이 포함 · `multinode_serve_smoke.sh`).

## 5. 금지
- 사용자 승인 없는 자동스캔 / 서브노드 무단 프로빙.
- **HITL 없는 서브 work_dir 자동 신설**(R2 — §1.7). 성능(ib_write_bw) 미검증 멀티-ready 기입(fail-closed).
- 무증거 manifest 오버라이드 / topology를 브랜치와 어긋나게 기입(3자-일치 위반).
- **빌딩블럭 스킬(terraforming_subnode·upstream-version-watch)·manifest 를 서브에 전달 금지**(런타임블럭·렌더 산출물만 — 2.1/2.3).
- **무증거 빈 정체성 렌더 금지**(render_sub_env.py 필수 필드 누락 시 fail-loud) · 템플릿에 IP·호스트 baking 금지(PII-free).
- **카나리(§2.5) 미통과 시 done 선언 금지** · 서브 워크스페이스 재스캔으로 "검증" 대체 금지(push-attestation 위반).
- SSH 키 교환·물리망 구성 대행(Case A — 검증·가이드까지만).

## 6. 참조
- 진입 루틴: `docs/plan/plan_2026062311_1` · `testlog_2026062314_1` · `devlog_2026062314_1`.
- 에이전트 환경 구축: `docs/plan/plan_2026062408_1`(개명+A2A) · 시드 `seed_9507ae1edc40` · 인터뷰 `interview_20260623_220341`.
- 통로/이관: `plan_2026062312_1`(output/) · `plan_2026062315_1`(manifest) · CLAUDE.md "산출물 통로 불변식".
- 다음(설계됨): docker-compose NCCL → manifest-driven 렌더(Plan 2) · recipe-explorer Phase-2 서브 자율 깊이(D8) · 하위스킬 호출 오케스트레이션 전반.
