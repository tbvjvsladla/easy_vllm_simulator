# 설치 — 토폴로지 인터뷰·스캔·게이트·manifest·Flag

> terraforming_node 스킬 reference — SKILL.md §0 · §0.5 · §1S · §1 · §1.1–§1.7 의 본문이다. SKILL.md(라우터)는 § 번호·제목·포인터만 든다.
> 이관 전 원문: `git show dcb713a:.claude/skills/terraforming_node/SKILL.md` (plan_26093022 Step 4 — 본문 바이트는 그대로 옮겼다).

## 0. 전제 / 입력
- **토폴로지-중립 진입**: single·multi **공통** 발동. (과거 "multi 전용·single 비활성(α)"는 plan_26063009_44_23 에서 폐기 — single 진입점 부재 = chicken-and-egg 갭이었음: "single이냐 multi이냐"를 묻는 주체가 multi일 때만 발동했음.) **첫 동작 = 토폴로지 인터뷰(§0.5)**. 이하 §1(진입 루틴)·§2(서브 환경구축)는 **topology=multi 분기**, single 은 §0.5→§1S 로 짧게 완결.
- **SSH = Case A**(multi 한정): 메인↔서브 패스워드리스 SSH는 **사전조건**(검증만, 키 교환·물리망 설정은 안 함).
- 계약 스켈레톤 = `manifest.template.yaml`(루트, 추적). **실값 = `output/<topology>/manifest.yaml`**(브랜치 파생 통로 — single=`output/single/`·multi=`output/multi/`; 비추적, plan_26062315). **(multi) 서브엔 메인 manifest 를 전달하지 않는다**(메인 단일계약 — render-on-main, D10 · single 서브가 받는 **서브 manifest** 는 §2.7.10).
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
provider 별 실행문법은 `references/orchestration/agent-control-adapter.md` 에서만 해소한다(본문 inline ✗).

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
  - `--peer-ip` 만: 게이트는 통과하지만 **서브 HW 동질성 검증이 통째로 생략**돼 `hw_verified`와 메인 발급 서브 manifest readiness가 성립하지 않는다(서브 info-only).
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


## 보조 스크립트 메모 (옛 §4 에서 이관)

- `scripts/staleness_gate.py` — **조건부 preflight 결정론 트리거**(`--topology`·`--repo`·`--observed`·`--now`·`--max-age-days`·`--self-test`). 3축(manifest/Flag · HW 드리프트 · attestation 나이) → 안정 reason code. 미평가 축은 `skipped:*` 로 음성정직 표기(조용한 통과 ✗). **HW 축의 interconnect 6필드는 topology=single 에서 비교하지 않는다**(2026-08-21 · approved_by AhnSangHun) — single 은 분산서빙을 안 해 interconnect 를 쓰지 않으므로 manifest 의 "미사용" 선언 ↔ 실측 RoCE 발산은 의도된 것이다. `scan_node.evaluate_gate` 의 single 의미론(RoCE 존재 = warning, gate note "interconnect 스캔 skip(실패 아님)")과 정합. **multi 는 유지**(텐서패브릭이므로 완화 ✗). 제외 적용 시 `notes` 에 표기한다(침묵 ✗).
