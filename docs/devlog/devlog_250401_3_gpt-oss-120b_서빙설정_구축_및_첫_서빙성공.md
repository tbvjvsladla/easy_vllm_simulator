# devlog 250401 — gpt-oss-120b 서빙 설정 구축 및 첫 서빙 성공

## 작업 요약

이전 세션에서 설계한 `envs/ → configs/(sh+yaml)` 구조를 실제로 구현하여, gpt-oss-120b 모델의 첫 vLLM 서빙에 성공했다.
YAML 설정 파일 검증, env/sh/yaml 3파일 체계 구축, docker-compose.yaml 연동, profile 충돌 해결까지 완료했다.

---

## 1. vLLM YAML 설정 파일 지원 확인

vLLM serve가 `--config` 플래그로 YAML 설정 파일을 네이티브 지원하는 것을 확인했다.

```bash
# 모델을 CLI에서 지정
vllm serve /app/models/gpt-oss-120b --config /app/configs/gpt-oss-120b-normal.yaml

# 모델도 YAML 안에 포함 가능 (v0.8.x+)
vllm serve --config /app/configs/gpt-oss-120b-normal.yaml
```

이 프로젝트에서는 모델 경로를 YAML에 포함하고, `--served-model-name`만 sh에서 CLI로 전달하는 방식을 채택했다.
(CLI 인자가 YAML보다 우선하므로, env에서 주입하는 동적 값은 CLI로 전달)

---

## 2. configs/gpt-oss-120b-normal.yaml 작성 및 검증

### 최초 작성 시 발견된 문제

**문제 1: Boolean 플래그 값 누락**

```yaml
# 잘못됨 — YAML 파서가 null로 해석
enable-auto-tool-choice

# 수정
enable-auto-tool-choice: true
```

**문제 2: 파일명 오타**

`gpt-oss-120b-noraml.yaml` → `gpt-oss-120b-normal.yaml` (noraml → normal)

### 최종 YAML 설정

```yaml
model: /app/models/gpt-oss-120b
host: 0.0.0.0
port: 8000
gpu-memory-utilization: 0.95
max-model-len: 65536
reasoning-parser: openai_gptoss
tool-call-parser: openai
enable-auto-tool-choice: true
```

순수 vLLM 엔진 설정만 포함. `served-model-name`은 env → sh → CLI 경로로 주입한다.

---

## 3. envs/.env.gpt-oss-120b-normal 작성 및 교정

### 발견된 문제: `=` 주변 공백

Docker Compose의 `env_file`은 `=` 주변 공백을 값에 포함시킨다.

```bash
# 잘못됨 — 값이 " 7900"이 됨
SERVING_PORT = 7900

# 수정
SERVING_PORT=7900
```

### 최종 env 파일

```bash
# 0) 프로젝트, 컨테이너, env파일 이름
COMPOSE_PROJECT_NAME=vllm_server_project
CONTAINER_NAME=gpt-oss-120b-serving-container
VERSION=1.0.0
NVIDIA_VISIBLE_DEVICES=all

# 1) 외부 노출 설정 (docker-compose 포트매핑)
SERVING_IP=0.0.0.0
SERVING_PORT=7900

# 2) LLM 서빙 설정
TIKTOKEN_ENABLED=true
SERVING_MODEL_NAME=gpt-oss-120b
CONFIG_FILE=gpt-oss-120b-normal
```

`CONFIG_FILE` 값이 sh/yaml 파일명과 env 파일명을 모두 결정하는 키 역할을 한다.

---

## 4. configs/gpt-oss-120b-normal.sh 작성

```bash
#!/bin/bash
# env 파일에서 주입된 변수: CONFIG_FILE, SERVING_MODEL_NAME, TIKTOKEN_ENABLED

# TIKTOKEN 환경변수 설정
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

# vllm serve 실행
vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
```

env → sh → yaml 체인:
- env에서 `CONFIG_FILE`, `SERVING_MODEL_NAME`, `TIKTOKEN_ENABLED` 주입
- sh에서 모델별 환경변수 설정 후, 같은 이름의 yaml을 `--config`로 로드
- `--served-model-name`은 CLI로 전달 (yaml보다 우선)

---

## 5. docker-compose.yaml 연동 수정

### 변경 1: `--env-file` 방식 채택

이전 세션의 `MODEL_ENV` 변수 방식에서, `--env-file`로 직접 env 파일을 지정하는 방식으로 변경했다.

```bash
# 이전 방식
MODEL_ENV=gpt-oss-120b docker compose --profile serve up

# 현재 방식
docker compose --env-file envs/.env.gpt-oss-120b-normal --profile serve up
```

`--env-file`이 env 파일의 모든 변수를 YAML 치환에 사용하므로, `CONFIG_FILE`, `CONTAINER_NAME`, `SERVING_PORT`가 동적으로 반영된다.

### 변경 2: vllm(bash) 서비스에 `profiles: [debug]` 추가

**문제**: `--profile serve`로 실행해도 profile이 없는 `vllm` 서비스가 항상 함께 생성되어, 기존 `vllm_server` 컨테이너와 이름 충돌 발생.

```
Error response from daemon: Conflict. The container name "/vllm_server" is already in use
```

**해결**: 서빙 컨테이너에 `docker exec -it <name> bash`로 접속하면 되므로 별도 bash 컨테이너는 불필요. `vllm` 서비스에 `profiles: [debug]`를 부여하여 명시적 요청 시에만 생성되도록 변경했다.

### 최종 docker-compose.yaml

```yaml
x-gpu-common: &gpu-common
  build:
    context: .
    dockerfile: Dockerfile
  runtime: nvidia
  ipc: host
  ulimits:
    memlock: -1
    stack: 67108864
  volumes:
    - /home/ash/.../hf_model:/app/models
    - /home/ash/.../tiktoken_encodings:/encodings:ro
    - ./configs:/app/configs:ro

services:
  vllm:
    <<: *gpu-common
    container_name: vllm_server
    profiles: [debug]
    ports:
      - "7900:8000"
    environment:
      TIKTOKEN_ENCODINGS_BASE: /encodings
    command: ["bash"]
    tty: true
    stdin_open: true

  vllm-serve:
    <<: *gpu-common
    container_name: ${CONTAINER_NAME}
    profiles: [serve]
    ports:
      - "${SERVING_PORT}:8000"
    env_file:
      - envs/.env.${CONFIG_FILE}
    command: ["bash", "/app/configs/${CONFIG_FILE}.sh"]
```

---

## 6. 실행 및 서빙 성공 확인

```bash
docker compose --env-file envs/.env.gpt-oss-120b-normal --profile serve up
```

- 빌드 95초 완료 (캐시 히트 + requirements.txt만 재설치)
- `gpt-oss-120b-serving-container` (ID: f43bf888eaf3) 정상 생성 및 서빙 확인

---

## 7. 최종 파일 현황

```
vllm_serving_server/
├── .claude/CLAUDE.md               # 프로젝트 가이드라인 (flat 구조로 업데이트)
├── Dockerfile
├── docker-compose.yaml             # debug/serve 양쪽 모두 profiles 적용
├── requirements.txt
├── envs/
│   └── .env.gpt-oss-120b-normal    # 서빙 환경 정의
├── configs/
│   ├── gpt-oss-120b-normal.sh      # 서빙 진입 스크립트
│   └── gpt-oss-120b-normal.yaml    # vLLM 엔진 설정
└── docs/
    ├── devlog/
    │   ├── devlog_250401_1_SDD기반_vllm_serving_server_프로젝트_설계.md
    │   ├── devlog_250401_2_Docker스킬설치_및_Dockerfile_Compose_리팩토링.md
    │   └── devlog_250401_3_gpt-oss-120b_서빙설정_구축_및_첫_서빙성공.md  ← 이 문서
    └── testlog/
        └── testlog_250401_gpt-oss-120b_bash통신테스트.md
```

---

## 8. 실행 명령어 정리

```bash
# 서빙 모드
docker compose --env-file envs/.env.gpt-oss-120b-normal --profile serve up

# 서빙 컨테이너에 bash 접속
docker exec -it gpt-oss-120b-serving-container bash

# 디버깅용 bash 모드 (필요할 때만)
docker compose --profile debug up
```

---

## 9. 다음 작업 (TODO)

- [ ] 서빙 상태에서 API 통신 테스트 (curl, OpenAI 호환 엔드포인트)
- [ ] 다른 서빙 프로파일 추가 (예: gpt-oss-120b-short, speculative decoding 등)
- [ ] CLAUDE.md의 실행 명령어 섹션을 debug/serve 양쪽 반영하여 최종 업데이트
