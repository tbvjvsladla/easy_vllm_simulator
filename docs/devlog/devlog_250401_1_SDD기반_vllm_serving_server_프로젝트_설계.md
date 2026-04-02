# devlog 250401 — vllm_serving_server SDD 기반 프로젝트 설계

## 작업 요약

Claude Code + SDD(Skill-Driven Development) 방식으로 `vllm_serving_server` 프로젝트의 기반 설계를 수행했다.
NGC PyTorch 컨테이너 위에 최신 vLLM을 직접 설치하는 범용 LLM 서빙 컨테이너 프로젝트이며,
오늘은 코드 구현이 아닌 **프로젝트 구조 설계와 Claude Code 연동 환경 세팅**에 집중했다.

---

## 1. 프로젝트 배경 정리

### 왜 이 프로젝트가 필요한가

NGC에서 제공하는 공식 vLLM 컨테이너(`nvcr.io/nvidia/vllm`)는 업데이트가 느리다.
2026년 3월 기준 NGC 최신 vLLM 컨테이너(26.02-py3)에 탑재된 vLLM은 `0.15.1` 버전인데,
실제 vLLM 최신 릴리즈는 `0.18.0`이다.

이를 해결하기 위해 **PyTorch만 설치된 NGC 컨테이너를 베이스로, vLLM pre-built wheel을 직접 설치**하는 방식을 채택한다.

### 핵심 제약사항

| 제약 | 이유 |
|------|------|
| 아키텍처 하드코딩 금지 (`uname -m` 자동감지) | x86_64(WSL) + aarch64(ARM Surface) 범용 |
| HuggingFace 캐시 사용 금지 | 폐쇄망 환경 대비 — 항상 볼륨마운트 방식 |
| vLLM-PyTorch 호환성 매트릭스 확인 필수 | vLLM 0.18.0 → torch==2.10.0 → NGC 26.01-py3 |

### 현재 작업 환경

- **이전**: ARM Surface
- **현재**: Windows → WSL Ubuntu → RTX 4090 Blackwell

---

## 2. Claude Code 환경 세팅

### 2-1. Docker 공식 스킬 설치

[OpenAEC-Foundation/Docker-Claude-Skill-Package](https://github.com/OpenAEC-Foundation/Docker-Claude-Skill-Package)를
git submodule로 설치했다. Docker Engine 24+ / Docker Compose v2 기준으로 검증된 22개 스킬 패키지.

```bash
git submodule add https://github.com/OpenAEC-Foundation/Docker-Claude-Skill-Package.git .docker-skills
```

설치 후 `.claude/CLAUDE.md`에서 5개 카테고리로 참조 경로를 등록:

| 카테고리 | 경로 | 용도 |
|----------|------|------|
| core | `.docker-skills/skills/source/docker-core/` | 아키텍쳐, 네트워킹, 보안 |
| syntax | `.docker-skills/skills/source/docker-syntax/` | Dockerfile 문법, Compose 디렉티브, CLI |
| impl | `.docker-skills/skills/source/docker-impl/` | 빌드 최적화, CI/CD, 프로덕션 배포 |
| errors | `.docker-skills/skills/source/docker-errors/` | 빌드/런타임/네트워크/Compose 트러블슈팅 |
| agents | `.docker-skills/skills/source/docker-agents/` | Dockerfile 생성기, 리뷰 에이전트 |

### 2-2. 파일 역할 분리 (SDD 설계)

Claude Code가 읽는 파일을 **컨텍스트 파일**과 **워크플로 스킬**로 분리했다:

- **`.claude/CLAUDE.md`** — 매 세션마다 자동 로드. 프로젝트 개요, 제약사항, 파일 역할, 준수사항 기록
- **`.claude/skills/vllm-serving/SKILL.md`** — 트리거 시 로드. 서빙 설정 생성, 트러블슈팅 워크플로 기록

분리 근거:
- CLAUDE.md = "Claude Code가 항상 알아야 하는 것" (프로젝트 규칙, 구조)
- SKILL.md = "특정 작업 요청 시 활성화되는 것" (반복 워크플로)

---

## 3. 프로젝트 구조 설계

### 3-1. 최종 디렉토리 구조

```
vllm_serving_server/
├── .claude/
│   ├── CLAUDE.md                  # 프로젝트 가이드라인
│   ├── settings.local.json
│   └── skills/
│       └── vllm-serving/
│           └── SKILL.md           # 커스텀 스킬
├── .docker-skills/                # [submodule] Docker 공식 스킬
├── Dockerfile                     # NGC PyTorch base → vLLM 설치
├── docker-compose.yaml            # profiles 기반 interactive/serve 분기
├── requirements.txt               # vLLM 의존성 라이브러리
├── envs/                          # 모델별 서빙 환경 정의서
│   └── <model-name>.env
└── configs/                       # 모델별 서빙 설정
    └── <model-name>/
        ├── serve.sh               # vllm serve 실행 스크립트
        └── serve-args.yaml        # vllm serve 인자값
```

### 3-2. 모델 서빙 체인

```
envs/<model>.env  →  configs/<model>/serve.sh  →  configs/<model>/serve-args.yaml  →  vllm serve
```

- **env**: 어떤 모델을, 어느 포트로 서빙할지 정의 (진입점)
- **serve.sh**: env 변수를 받아 vllm serve 명령어를 조립·실행
- **serve-args.yaml**: vllm serve에 전달할 상세 인자값 (메모리, 병렬화, 파서 등)

---

## 4. docker-compose.yaml 설계 — profiles + anchor/alias

오늘 설계에서 가장 중요한 결정 사항. 컨테이너 실행 모드를 **Docker Compose profiles**로 분기하되,
공통 설정 중복을 **YAML anchor/alias**로 제거한다.

### 4-1. YAML anchor/alias 개념

```yaml
# & (앵커)  = 이 블록에 이름을 붙여둔다
# * (알리아스) = 이름 붙여둔 블록을 여기에 복사한다
# << (머지 키) = 복사한 내용을 현재 블록에 합친다
```

`x-` 접두어는 Docker Compose의 extension field로, 서비스로 해석되지 않는 순수 앵커 저장소 역할을 한다.

### 4-2. 실행 모드

| 모드 | 명령어 | 동작 |
|------|--------|------|
| bash (interactive) | `docker compose --profile interactive up` | 컨테이너 bash 진입, 수동 디버깅 |
| 서빙 (serve) | `MODEL_ENV=gpt-oss-120b docker compose --profile serve up` | env → sh → yaml → vllm serve |

### 4-3. 예시 docker-compose.yaml

```yaml
# ============================================================
# docker-compose.yaml — vLLM Serving Server
# ============================================================
# 사용법:
#   bash 모드:  docker compose --profile interactive up
#   서빙 모드:  MODEL_ENV=gpt-oss-120b docker compose --profile serve up
# ============================================================

# ----------------------------------------------------------
# 공통 설정 앵커 정의 (x- 접두어 = extension field)
# ----------------------------------------------------------
x-gpu-common: &gpu-common
  build:
    context: .
    dockerfile: Dockerfile
  container_name: vllm_server
  runtime: nvidia
  ipc: host                          # shared memory 공유 → GPU 통신 안정성
  ulimits:
    memlock: -1                      # pinned memory / NCCL 안정성
    stack: 67108864                  # 대형 모델/네이티브 라이브러리 안정성
  volumes:
    - /home/ash/ws_docker/coga_triton_server/model_repo/hf_model:/app/models
    - /home/ash/ws_docker/coga_triton_server/model_repo/tiktoken_encodings:/encodings:ro

# ----------------------------------------------------------
# 서비스 정의
# ----------------------------------------------------------
services:

  # === Interactive (bash) 모드 ===
  # docker compose --profile interactive up
  vllm:
    <<: *gpu-common                  # 공통 설정 머지
    profiles: ["interactive"]
    ports:
      - "7900:8000"                  # bash에서 수동 서빙 시 사용할 포트
    tty: true
    stdin_open: true
    # command 미지정 → Dockerfile의 기본 쉘(bash) 진입

  # === Serve 모드 ===
  # MODEL_ENV=gpt-oss-120b docker compose --profile serve up
  vllm-serve:
    <<: *gpu-common                  # 공통 설정 머지
    profiles: ["serve"]
    container_name: vllm_serve       # interactive와 이름 구분
    env_file:
      - envs/${MODEL_ENV:?MODEL_ENV is required}.env  # 미지정 시 에러
    ports:
      - "${SERVE_PORT:-8000}:${SERVE_PORT:-8000}"     # env에서 포트 동적 할당
    command:
      - bash
      - -c
      - |
        set -euo pipefail
        echo "=== Loading config from $${CONFIG_DIR} ==="
        chmod +x /workspace/$${CONFIG_DIR}/serve.sh
        exec bash /workspace/$${CONFIG_DIR}/serve.sh
```

**설계 포인트:**

- `x-gpu-common` 앵커에 GPU/볼륨/빌드 설정을 한 번만 정의, 양쪽 서비스에서 `<<: *gpu-common`으로 머지
- 설정 변경이 필요하면 앵커 블록만 수정 → 모든 서비스에 자동 반영
- `container_name`은 머지 후 오버라이드 가능 (vllm_server → vllm_serve)
- `${MODEL_ENV:?MODEL_ENV is required}` — bash parameter expansion으로 env 파일 미지정 방지

### 4-4. 예시 Dockerfile

```dockerfile
FROM nvcr.io/nvidia/pytorch:26.01-py3

ARG VLLM_VERSION=0.18.0
ARG CUDA_VERSION=130

# Step 1: pip constraints 해제
# NGC 컨테이너의 constraint.txt가 대규모 라이브러리 설치 시 의존성 충돌을 유발
RUN cp /etc/pip/constraint.txt /etc/pip/constraint.txt.bak \
    && : > /etc/pip/constraint.txt

# Step 2: vLLM pre-built wheel 설치 (--no-deps)
# uname -m으로 CPU 아키텍처 자동 감지 → x86_64/aarch64 모두 대응
RUN CPU_ARCH=$(uname -m) \
    && pip install --no-deps \
      "https://github.com/vllm-project/vllm/releases/download/v${VLLM_VERSION}/vllm-${VLLM_VERSION}+cu${CUDA_VERSION}-cp38-abi3-manylinux_2_35_${CPU_ARCH}.whl"

# Step 3: 의존성 라이브러리 설치
COPY requirements.txt /tmp/requirements.txt
RUN pip install -r /tmp/requirements.txt

WORKDIR /workspace
```

**설계 포인트:**

- `ARG`로 vLLM/CUDA 버전을 파라미터화 → 버전 업그레이드 시 이 값만 변경
- constraints 해제 → wheel 설치 → 의존성 설치 3단계 분리
- `uname -m`으로 아키텍처 자동감지 — ARM Surface(aarch64)와 WSL(x86_64) 양쪽 대응
- `WORKDIR /workspace` — NGC 컨테이너의 기본 작업 디렉토리 관례

---

## 5. 커스텀 스킬 설계 (vllm-serving)

### 트리거 조건

- "이 모델 서빙해줘", "서빙 설정 만들어줘" → 워크플로 1 (서빙 설정 생성)
- "서빙이 안돼", "OOM 에러", "모델 로딩 실패" → 워크플로 2 (트러블슈팅)
- "vLLM 업그레이드하고 싶어" → 워크플로 3 (버전 업그레이드)

### 워크플로 요약

| # | 워크플로 | 입력 | 출력 |
|---|---------|------|------|
| 1 | 새 모델 서빙 설정 생성 | 모델명, 포트, 특수 옵션 | env + serve.sh + serve-args.yaml |
| 2 | 서빙 에러 트러블슈팅 | 에러 로그/메시지 | 원인 진단 + 해결 방법 |
| 3 | vLLM 버전 업그레이드 | 타겟 vLLM 버전 | Dockerfile ARG 수정 + requirements.txt 업데이트 |

### 에러 메시지 빠른 매핑 (스킬 내 레퍼런스)

| 에러 키워드 | 원인 | 해결 |
|------------|------|------|
| `CUDA OOM` | GPU 메모리 부족 | gpu-memory-utilization ↓, max-model-len ↓ |
| `No module named` | 의존성 누락 | requirements.txt 추가 후 재빌드 |
| `Could not find model` | 경로 오류 | 볼륨 마운트 + MODEL_PATH 확인 |
| `Address already in use` | 포트 충돌 | env에서 SERVE_PORT 변경 |
| `tiktoken` 에러 | 환경변수 미설정 | TIKTOKEN_ENCODINGS_BASE 추가 |

---

## 6. 다음 작업 (TODO)

- [ ] 설계한 docker-compose.yaml을 실제 프로젝트에 반영하고 빌드 테스트
- [ ] `envs/` + `configs/` 디렉토리 생성 및 첫 번째 모델 서빙 설정 작성
- [ ] serve.sh 템플릿의 yaml 파싱 로직 검증 (NGC 컨테이너 내 PyYAML 존재 확인)
- [ ] 커스텀 스킬(vllm-serving/SKILL.md)을 `.claude/skills/`에 배치 후 트리거 테스트
- [ ] requirements.txt를 Claude Code와의 대화로 점진적 업데이트
- [ ] WSL + RTX 4090 환경에서 첫 모델 서빙 성공 확인
