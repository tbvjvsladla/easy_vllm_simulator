# vLLM Serving Server

NGC PyTorch 컨테이너 위에 최신 vLLM을 직접 설치한 범용 LLM 서빙 컨테이너.

NGC 공식 vLLM 컨테이너는 업데이트가 느려 구 버전이 탑재되어 있으므로, PyTorch base 위에 vLLM pre-built wheel을 직접 설치하는 방식을 사용한다.

## 버전 정보

| 구성 요소 | 버전 |
|-----------|------|
| NGC PyTorch | 26.01-py3 |
| vLLM | 0.19.0 |
| CUDA | 13.0 |
| 아키텍처 | x86_64 / aarch64 자동 감지 |

## 사전 준비

### 모델 다운로드

이 프로젝트는 **폐쇄망 환경을 전제**로 설계되어, HuggingFace 런타임 다운로드를 사용하지 않는다.
모델 파일을 사전에 호스트 머신에 다운로드해 두어야 한다.

```bash
# 예시: huggingface-cli로 사전 다운로드
huggingface-cli download <model-repo> --local-dir /path/to/models/<model-name>
```

### tiktoken 인코딩 파일 (해당 모델만)

tiktoken 기반 토크나이저를 사용하는 모델(GPT 계열 등)은 인코딩 파일도 사전 다운로드가 필요하다.

---

## !! 배포 시 반드시 수정해야 하는 항목

### `docker-compose.yaml` 볼륨 경로

`docker-compose.yaml`의 `x-gpu-common` 블록에 아래 볼륨 마운트가 **절대경로로 하드코딩**되어 있다.

```yaml
x-gpu-common: &gpu-common
  # ...
  volumes:
    - /home/ash/ws_docker/coga_triton_server/model_repo/hf_model:/app/models          # <-- 변경 필수
    - /home/ash/ws_docker/coga_triton_server/model_repo/tiktoken_encodings:/encodings:ro  # <-- 변경 필수
    - ./configs:/app/configs:ro
```

**배포 환경의 실제 경로로 반드시 변경할 것:**

```yaml
  volumes:
    - <호스트의 모델 디렉토리>:/app/models
    - <호스트의 tiktoken 인코딩 디렉토리>:/encodings:ro    # tiktoken 미사용 시 제거 가능
    - ./configs:/app/configs:ro
```

| 컨테이너 경로 | 용도 | 필수 여부 |
|---------------|------|-----------|
| `/app/models` | HuggingFace 모델 파일 | 필수 |
| `/encodings` | tiktoken 인코딩 파일 | 모델에 따라 선택 |
| `/app/configs` | 서빙 설정 (sh, yaml) | 필수 (자동 마운트) |

> `/app/models` 아래에 모델별 디렉토리가 위치해야 한다.
> 예: `/app/models/gpt-oss-120b/`, `/app/models/llama-3-70b/`

---

## 사용법

### 프로젝트 구조

```
vllm_serving_server/
├── Dockerfile
├── docker-compose.yaml
├── requirements.txt
├── envs/                       # 서빙 환경 정의 (.env 파일)
│   └── .env.<config-name>
└── configs/                    # 서빙 설정 (sh + yaml)
    ├── <config-name>.sh
    └── <config-name>.yaml
```

하나의 서빙 설정은 `<config-name>`을 키로 env, sh, yaml 3개 파일로 구성된다.

### 서빙 실행

```bash
docker compose --env-file envs/.env.<config-name> --profile serve up
```

```bash
# 예시: gpt-oss-120b 모델의 normal 프로파일로 서빙
docker compose --env-file envs/.env.gpt-oss-120b-normal --profile serve up
```

### 서빙 컨테이너 접속

```bash
docker exec -it <container-name> bash
```

### 디버깅용 bash 모드

서빙 없이 컨테이너만 띄워 내부 확인이 필요할 때:

```bash
docker compose --profile debug up
```

별도 터미널에서 컨테이너에 접속:

```bash
docker exec -it vllm_server bash
```

#### 컨테이너 내부 검증 체크리스트

```bash
# 1) vLLM 설치 확인
python -c "import vllm; print(vllm.__version__)"

# 2) GPU 인식 확인
python -c "import torch; print(torch.cuda.device_count(), torch.cuda.get_device_name(0))"

# 3) 모델 볼륨 마운트 확인
ls /app/models/

# 4) 서빙 설정 파일 마운트 확인
ls /app/configs/

# 5) 수동 서빙 테스트 (모델 로딩 및 추론 확인)
vllm serve /app/models/<모델디렉토리명> --host 0.0.0.0 --port 8000
```

### 이미지 재빌드

```bash
docker compose --profile debug build
```

### 로그 확인

```bash
docker logs -f <container-name>
```

---

## 새 모델 서빙 설정 추가

`<config-name>`은 `모델명-프로파일명` 형식을 따른다 (예: `gpt-oss-120b-normal`).

### 1. env 파일: `envs/.env.<config-name>`

```bash
COMPOSE_PROJECT_NAME=vllm_server_project
CONTAINER_NAME=<모델명>-serving-container
VERSION=1.0.0
NVIDIA_VISIBLE_DEVICES=all

SERVING_IP=0.0.0.0
SERVING_PORT=<호스트포트>

TIKTOKEN_ENABLED=true
SERVING_MODEL_NAME=<API에 노출할 모델명>
CONFIG_FILE=<config-name>
```

> `=` 주변에 공백을 넣지 않는다. Docker Compose가 공백을 값에 포함시킨다.

### 2. sh 파일: `configs/<config-name>.sh`

```bash
#!/bin/bash
if [ "$TIKTOKEN_ENABLED" = "true" ]; then
    export TIKTOKEN_ENCODINGS_BASE=/encodings
    export TIKTOKEN_RS_CACHE_DIR=/encodings
fi

vllm serve --config "/app/configs/${CONFIG_FILE}.yaml" \
    --served-model-name "$SERVING_MODEL_NAME"
```

### 3. yaml 파일: `configs/<config-name>.yaml`

```yaml
model: /app/models/<모델디렉토리명>
host: 0.0.0.0
port: 8000
gpu-memory-utilization: 0.95
max-model-len: <컨텍스트길이>
```

순수 vLLM 엔진 설정만 작성한다. `served-model-name`은 sh에서 CLI로 주입되므로 yaml에 포함하지 않는다.

---

## 개발 문서

설계 배경, 의사결정 기록, 트러블슈팅은 `docs/` 디렉토리를 참조:

```
docs/
├── devlog/     # 개발 로그 (설계, 리팩토링, 서빙 구축 과정)
└── testlog/    # 테스트 로그 (통신 테스트 결과)
```
