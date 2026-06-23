---
name: terraforming
description: >-
  임의 사용자 환경을 스캔·인터뷰해 manifest.yaml(스킬 간 단일 계약)을 채우는 오케스트레이터.
  현재 구현 = **멀티노드 진입 루틴**(메인↔서브 연결·성능 검증 → manifest 기입). "테라포밍",
  "환경 셋업", "노드 스캔", "멀티노드 구성 검증", "메인 서브 연결 확인", "manifest 생성",
  "인터커넥트 점검" 같은 지시에 발동. 무단 스캔 금지 — 5-전제조건 인터뷰 + 사용자 승인 후 스캔.
  성능 미검증 멀티 진행은 fail-closed로 차단(성능저하 트랩 방지).
---

# terraforming

임의 환경에서 **이식 가능한 vLLM 컨테이너/서빙**을 쓰려면 그 환경의 구체값(토폴로지·노드·인터커넥트·
NAS·CUDA)을 먼저 알아야 한다. terraforming 은 이를 **스캔(자동탐지) + 인터뷰(사람 판단)** 로 채워
`output/<topology>/manifest.yaml`(스킬 간 단일 계약)을 산출하고, 이후 `upstream-version-watch`·
`vllm-recipe-explorer` 를 호출하는 **오케스트레이터**다.

> **현재 구현 범위 = 멀티노드 진입 루틴**(plan_2026062311_1, 실 2노드 라이브 검증 testlog_2026062314_1).
> 환경탐지 전반·하위스킬 호출 오케스트레이션은 설계됨(헌법 §스킬 경계, P5) — 점진 확장.

> 설계 원칙(하네스 엔지니어링): **스캔·게이트·일치단언은 결정론 스크립트**(`scripts/scan_node.py`),
> **인터뷰·승인·HITL 판정은 페르소나(이 문서)**. 둘을 섞지 않는다.

## 0. 전제 / 입력
- **SSH = Case A**: 메인↔서브 패스워드리스 SSH는 **사전조건**(테라포밍은 검증만, 키 교환·물리망 설정은 안 함).
- 계약 스켈레톤 = `manifest.template.yaml`(루트, 추적). **실값 = `output/<topology>/manifest.yaml`**(비추적, 통로 분리 — topology=브랜치 파생, plan_2026062315_1).
- 검증 정본 = `devlog_250422`(git f3583f0, in-history) 실측 + seed PDF(repo-외부, 동기화 제외).

## 1. 멀티노드 진입 루틴 (절차)

> 핵심 불변식: **무단 자동스캔 금지** — 반드시 "인터뷰 → 사용자 승인 → 스캔" 순서.
> 그리고 **성능 미검증 멀티 진행 금지(fail-closed)** — 고속 인터커넥트가 합격선 미달이면 멀티-ready 거부(성능저하 트랩 방지).

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
- **α**(topology=single): interconnect 검증 skip(정상). RoCE 하드웨어 존재해도 비blocking 경고(멀티 가능 머신의 단일 운용=정상).
- **multi-ready**(topology=multi): RoCE 존재 + peer 도달 + 대역폭 합격선 → ready.
- **γ fail-closed**(multi): RoCE 부재 / peer 미도달 / 대역폭 미달 → **멀티-ready manifest 미생성 + blocked + 비0 종료**(β 부분캡처는 진단용).
- **3자-일치 단언**: `git branch ⇒ topology` ↔ `output/<topology>/manifest.yaml` ↔ scan(인터커넥트 유무) 불일치 시 blocked(single/multi 혼재 차단).

### 1.6 manifest 기입 (HITL)
검증 통과 시 `scan_node.py --emit-manifest` 가 topology+interconnect 블록 산출 → **사람 확인 후** `output/<topology>/manifest.yaml` 반영. **무증거 기입 금지.**

### 1.7 서브 work_dir 프로비저닝 게이트 (HITL — R2 불변식, plan_2026062320_1)
서브노드에 메인 산출물을 복제하려면 서브 작업경로(`nodes[role=sub].work_dir`)가 있어야 한다. **기본값 = 메인 work_dir 와 동일**(예 main `~/ws_docker/easy_vllm_simulator` → sub 동일 경로). 그러나:
- **경로 신설은 반드시 HITL** — `sync_to_sub.sh --apply` 는 서브 work_dir 부재 시 **정지 + "이 경로를 신설할까요?" 질의**(exit 5). 사람 승인(`--provision`) 시에만 `mkdir -p` 후 전송.
- **HITL 없는 자동 경로 신설 절대 금지.** 경로값은 manifest 에서만 해소(하드코딩 금지 — 포인터 원칙).

## 2. 결정론 vs 판단 분리
| 결정론 (`scripts/scan_node.py`) | 판단 (이 페르소나) |
|---|---|
| 스캔(sysfs/show_gids/nvcc) · 교차검증 · α/γ/multi-ready 게이트 · 3자-일치 단언 · manifest 블록 산출 | 5-전제조건 인터뷰 · 사용자 승인 게이트 · ib_write_bw 오케스트레이션 · manifest 최종 기입 승인 · 모호 시 중단·질의 |

회귀 고정: `python3 scripts/scan_node.py --self-test`(게이트 9케이스, 하드웨어 불요).

## 3. 보조 파일
- `scripts/scan_node.py` — 결정론 스캔 코어(`--topology`·`--peer-ip`·`--bandwidth-gbps`·`--bw-floor`·`--manifest`·`--emit-manifest`·`--self-test`).
- 메인↔서브 코드 전파·서빙 스모크는 `upstream-version-watch`(`sync_to_sub.sh`·`multinode_serve_smoke.sh`) 담당.

## 4. 금지
- 사용자 승인 없는 자동스캔 / 서브노드 무단 프로빙.
- **HITL 없는 서브 work_dir 자동 신설**(R2 위반 — §1.7. 부재 시 정지·질의, `--provision` 승인 전 무신설).
- 성능(ib_write_bw) 미검증 멀티-ready manifest 기입(fail-closed 위반).
- 무증거 manifest 오버라이드 / topology를 브랜치와 어긋나게 기입(3자-일치 위반).
- SSH 키 교환·물리망 구성 대행(Case A — 검증·가이드까지만).

## 5. 참조
- 진입 루틴 계획·검증: `docs/plan/plan_2026062311_1` · `docs/testlog/testlog_2026062314_1` · `docs/devlog/devlog_2026062314_1`.
- 통로/이관: `plan_2026062312_1`(output/) · `plan_2026062315_1`(manifest) · CLAUDE.md "산출물 통로 불변식".
- 다음(설계됨): docker-compose NCCL → manifest-driven 렌더(Plan 2) · 하위스킬 호출 오케스트레이션 전반.
