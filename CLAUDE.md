# CLAUDE.md — vLLM 업스트림 추적 · 컨테이너 버전관리 에이전트

> 이 워크스페이스의 Claude는 **임의 사용자 환경에서 NGC 기반 vLLM 컨테이너와 서빙전략을 생성하는
> 이식 가능한 코드 에이전트**다(업스트림 vLLM 릴리즈 추적 → 컨테이너 버전관리 포함).
> 사용자의 노드 구성·네트워크·경로 같은 환경 구체값은 `manifest.yaml`에서 읽는다(테라포밍 스킬이 생성).
> 이 파일은 세션마다 로드되는 "항상 들고 있어야 할 사실"만 담는다.
> 다단계 절차(전파 워크플로)는 `.claude/rules/workflow.md`에 있다.
> 근거: Ouroboros 인터뷰 → Seed (비추적 `seed/` 참조 — 배포본엔 부재 가능).

## 정체성

- **업스트림(진실의 원천)**: vLLM — https://github.com/vllm-project/vllm
  - "버전" = GitHub Release 태그. **pre-release도 추적 대상**.
- **관리 대상**: 커스텀 Docker 컨테이너 = NGC PyTorch 베이스 + vLLM(**prebuilt wheel 또는 소스빌드**) + 주변 의존성.
- **토폴로지**: 사용자 환경에 따라 단일노드 또는 분산(다노드)으로 결정된다. 노드 수·역할(main/sub)·인터커넥트는
  헌법에 박지 않고 `manifest.yaml`의 `topology`·`nodes[]`에서 읽는다(테라포밍이 자동탐지+인터뷰로 채움).
- **브랜치**: `single-node` = 단일노드 전용 · `multi-node` = 분산 전용. 두 브랜치는 버전 핀이 독립.

## 핵심 사실 (항상 보유)

- **레이어 커플링 규칙(가장 중요)**: 대상 vLLM 버전의 `pyproject.toml` `[build-system].requires`에
  **명시된 torch 버전**을 먼저 확인하고, 그 torch 버전과 NGC 컨테이너의 `NVIDIA_PYTORCH_BUILD_VERSION`
  **접두어**가 일치하는 NGC PyTorch 베이스 태그를 선정한다.
  (접미어 `+해시`·빌드메타는 무시. 예: vLLM 0.21.0 → torch 2.11.0 → `nvcr.io/nvidia/pytorch:26.03-py3`)
- **빌드트랙 따름정리**: torch 2.10대 → prebuilt wheel · torch 2.11+ → NGC alpha와 prebuilt `_C`의 C++ ABI 충돌(하드 ABI 벽) → **소스빌드 1차 트랙**. 트랙 판정 = 스킬 `upstream-version-watch` §0.5, 최종 중재 = 스모크.
- **커플링 보강 원칙**: prefix-매칭은 필요조건일 뿐 — source-build에서 alpha 베이스가 stable-ABI 심볼 결여 시 **더 새 NGC 베이스 승격이 정당**(전방호환). 절차 = `.claude/rules/workflow.md` S3, 키잉 = 스킬 §4.6.
- **이미지 네이밍 불변식**: `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}`(예 `0.23.0-cu132-aarch64-source`). 모델-키잉 금지(과거 난립 원인) — 한 이미지가 모든 모델을 서빙. 태그 산정 = `render_dockerfile.py`.
- **산출물 통로 불변식 (single/multi 혼재 차단)**: 빌드/렌더 산출물(Dockerfile·compose·requirements·모델 configs·envs)은 **`output/<topology>/`(single|multi)** 에 둔다. **통로 껍데기(`.gitkeep`)만 추적·생성물 비추적** → 단일/멀티 산출물이 켜켜이 쌓여도 경로 격리로 서로 침범 못 함. 빌드 = `docker compose -f output/<topology>/docker-compose.yaml …`. 예외: multi 손작성 컨테이너 정의는 `*.template` 졸업 전까지 추적(정본 — Plan 2서 ignore 강등). 근거: `docs/plan/plan_2026062312_1`.
- **통합메모리 gmu 따름정리**: 통합메모리 호스트(GB10 등)에서는 gpu-memory-utilization을 반드시 명시 emit — 기본 0.92는 통합메모리에서 OOM(스킬 `vllm-recipe-explorer` §5).
- **인코딩 자산 따름정리**: 모델 가중치뿐 아니라 런타임 인코딩 자산(tiktoken o200k/harmony)도 에어갭 사전적재 대상(스킬 `vllm-recipe-explorer` §5).
- **빌드 입력**: `CPU_ARCH=$(uname -m)` · `CUDA_VERSION`(예 129) · GitHub Releases pre-built wheel.
  wheel은 `pip install --no-deps`로 설치하고, **그 전에 `/etc/pip/constraint.txt`를 비운다**(NGC 핀 충돌 회피).
- **주변 의존성 원천**: vLLM의 `requirements/{common,cuda,build}.txt` + `pyproject.toml`. `requirements.txt`에 반영.
- **모델**: 폐쇄망 전제. 사람이 사전 다운로드해 NAS에 둔 모델을 read-only 마운트. 런타임 다운로드 없음.

## 버전 핀 / 트리거 정책

- **완전 수동**: 사람이 신규 vLLM을 감지(모델 구동 실패 또는 GitHub 확인) → "업데이트" 지시 → 에이전트 실행.
  자동 폴링·webhook·cron 없음. patch/minor/major 무관하게 항상 사람 지시로 시작.
- **config.yaml(영속)** 이 대상 버전 · 단일/멀티노드 스모크 모델명 · NAS 경로를 지정(스킬에서 사용, Step 2).

## 배포 / 환경 (manifest)

- **이식 모델**: 배포 단위 = 스켈레톤 + 생성엔진(완성품 아님). 누구든 클론 후 자기 환경을 테라포밍해 쓴다.
- **포인터 원칙**: 환경 구체값(노드 IP·호스트명·인터커넥트·NAS 경로·origin)은 **헌법에 두지 않고** `manifest.yaml`에서 읽는다.
  추적되는 스켈레톤은 `manifest.template.yaml`(빈칸), 테라포밍이 채운 `manifest.yaml`은 비추적. NAS 기본값 = `/mnt/models`(manifest로 override).
  `CPU_ARCH`는 빌드타임 `$(uname -m)`(리터럴 baking 금지) · NAS 경로는 `${NAS_MODEL_PATH}` env(추적물에 PII 비박음).
- **2-브랜치 배포**: `single-node`(단일) · `multi-node`(분산) 모두 배포 대상. 공유 빌딩블럭은 `scripts/sync_branches.sh`로 동일하게 유지.

## build / 검증 커맨드

- 산출물 통로: 빌드/서빙 산출물은 `output/<topology>/`(single|multi)에 위치 — 단일/멀티 혼재 차단. 통로 껍데기만 추적·생성물 비추적(plan_2026062312_1).
- 이미지 빌드(디버그): `docker compose -f output/<topology>/docker-compose.yaml --profile debug build`
- 서빙: `docker compose -f output/<topology>/docker-compose.yaml --env-file output/<topology>/envs/.env.<config> --profile <serve|master|slave> up`
- **스모크(합격 신호)**: `vllm serve /app/models/<모델디렉토리>` 후 **프롬프트 1회 → 비어있지 않은 완성 응답 1회**.
  단일노드·멀티노드 **양쪽** 스모크가 통과해야 bump 완료. (상세 절차: `.claude/rules/workflow.md`)

## 검증 게이트 (Goal-Driven — Karpathy B4)

- 컨테이너 변경은 위 스모크 통과 전 **done 선언 금지**.
- **smoke-before-commit**: 로컬 빌드+스모크 통과 코드만 last-good 커밋으로 남긴다.
  (origin은 사용자 환경 값(`manifest.origin_url`)에서 설정 — 없으면 로컬 전용. 멀티노드 서브 전파는 rsync `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh`.)
- 버전 bump는 사람 지시(핀 정책)와 단계별 HITL 게이트를 거친다.

## 계획 게이트 (체화 규율)

- container-gen · serving-strategy · branch-sync · terraforming 작업은 반드시 `docs/plan/` 문서를 **먼저 발행**하고
  **사람 검토(HITL)** 후 진행한다. 테라포밍된 환경에서도 이 문서 발행 규칙대로 작업하는 것이 **정본**이며
  루틴화한다(self-improving tooling). 문서 규약 상세: `.claude/rules/docs.md`.

## 금지 (안전 · 재현성)

- **무인(unattended) 자동 다운로드 금지가 기본** — 서빙대상 모델 부재 시 **사용자 승인 게이트** 후 (관리경로 존재 → 그 경로 영속 다운로드 / 부재 → 컨테이너 내부 HF cache 임시) 허용. 무인 자동 다운로드 절대 금지(절차: `.claude/rules/workflow.md` §모델/안전 가드).
- 외부/공식 스킬·플러그인·MCP를 사용자 검수 없이 **임의 설치 금지** (propose → review → install).
- **빌딩블럭**(`CLAUDE.md`·`.claude/`)은 이제 **추적·배포 대상**이다(이식 가능한 스켈레톤+생성엔진).
  단 사적/생성물(`.claude/settings.local.json`·`skills/*/config.yaml`·`manifest.yaml`·`envs/.env.*`·`seed/`·
  `sub_node/CLAUDE.md` 실값)은 **비추적**. 브랜치 간 공유 콘텐츠는 `scripts/sync_branches.sh`로
  동기화한다(수동 — 모든 작업 종료 후 사람 질의로 실행).
- 버전 문자열 해소(torch 핀·NGC 태그)를 **확률론적 추론으로 처리 금지** → **결정론적 스크립트**로(하네스 엔지니어링).
- 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경 금지.
- 요청 범위 밖 기능·추상화 선반영 금지(Karpathy B2·B3).

## 롤백

- **last-good 앵커 = 로컬 스모크-통과 커밋**. 미커밋 작업분은 일회용.
- 실패 시: `git reset --hard <last-good-commit>`로 복귀(잔여 로컬 상태 0). `single-node`·`multi-node` 독립 롤백.
  복귀 기준이 더 필요하면 통과분에 태그(예: `git tag last-good-<branch>`)를 둔다.
- (참고) origin은 사용자 환경 값(`manifest.origin_url`)에서 설정: `git remote add origin <manifest.origin_url>`.

## 스킬 / 도구 경계

- **커스텀(3-스킬 계층)**: `terraforming`(오케스트레이터 — 환경탐지 → `manifest.yaml`(스킬 간 단일 계약) 생성 → `upstream-version-watch`·`vllm-recipe-explorer` 호출; **설계됨·미구현(P5)**) → `upstream-version-watch`(버전해소 + render + 소스빌드 + 빌드/스모크) · `vllm-recipe-explorer`(모델 yaml/sh/env + VRAM/KV trial + tiktoken 사전적재). 결정론 vs 판단 분리는 각 스킬 내부.
- **외부**: Docker 작성/문법검사 보조 — 후보 `netresearch/docker-development-skill` (설치정책 거쳐 도입).
- **MCP**: 현재 없음. (멀티노드 서브노드 직접 SSH 제어 = 구현됨: `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh` rsync 전달 +
  서브 빌드워커 CC `ssh sub bash -lc "claude -p"`. SKILL.md §4.5 / workflow.md S2.5·S3.)
- 참고: 기존 `configs/check_reqs.py`(의존성 차이 분석 스크립트)를 결정론적 resolve 기반으로 재활용.

## 문서 발행 / 참조

- **3종 문서 역할**: `docs/plan/`=착수 전 계획(HITL 검토) · `docs/devlog/`=작업 내역 서사 · `docs/testlog/`=빌드/검증 증거·판정.
- **명명 규칙**: `docs/<type>/<type>_<YYYYMMDDHH>_<seq>_<주제>.md` (type별 서브디렉토리+접두사 · 절대일시 YYYYMMDDHH(시각까지) · 일련번호 seq · 한국어 밑줄 주제).
- **브랜치 통합 모델**: 작업 문서(`docs/<type>/*.md`)는 **gitignore → 브랜치 전환에 persist·자동 통합**(빌딩블럭과 동일). 추적·배포는 **폴더 스켈레톤 + 각 폴더 `example.md` 1개**만. 구현체(Dockerfile/compose/configs/envs)는 브랜치별 독립(통합 안 함).
- **문서 작성 규약 상세**: `.claude/rules/docs.md` (역할·명명·구조·통합모델 명문화).
- 전파 워크플로 규칙: `.claude/rules/workflow.md`.
- 부트스트랩 근거: 비추적 `seed/` 참조(GUIDE·question-catalog·REFERENCES·Seed) — 배포본엔 부재 가능.
