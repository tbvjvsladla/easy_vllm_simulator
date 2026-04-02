# aarch64 플랫폼 서버 — 빌드 및 서빙 트러블슈팅 로그

## 환경

| 항목 | 값 |
|------|------|
| 플랫폼 | aarch64 (ARM) |
| NGC PyTorch | 26.01-py3 |
| vLLM | 0.18.1 |
| CUDA | 13.1 (Forward Compatibility 모드, 커널 드라이버 580.142) |

---

## 이슈 1: `docker compose --profile debug up` 시 container_name 검증 실패

### 에러

```
validating docker-compose.yaml: services.vllm-serve.container_name ''
does not match pattern '[a-zA-Z0-9][a-zA-Z0-9_.-]+'
```

### 원인

x86_64(WSL) 환경에서는 발생하지 않던 에러. aarch64 서버의 Docker Compose 버전이 더 엄격하여, `--profile debug`로 실행해도 전체 YAML을 파싱하면서 `vllm-serve` 서비스의 `${CONTAINER_NAME}`이 빈 문자열로 치환되어 검증 실패.

### 해결

`docker-compose.yaml`에서 변수 참조에 기본값을 추가:

```yaml
# 수정 전
container_name: ${CONTAINER_NAME}
ports:
  - "${SERVING_PORT}:8000"

# 수정 후
container_name: ${CONTAINER_NAME:-vllm-serve-container}
ports:
  - "${SERVING_PORT:-8080}:8000"
env_file:
  - envs/.env.${CONFIG_FILE:-default}
command: ["bash", "/app/configs/${CONFIG_FILE:-default}.sh"]
```

기본값은 YAML 파싱만 통과시키기 위한 것으로, `--profile serve` 실행 시에는 `--env-file`로 주입된 실제 값이 사용된다.

---

## 이슈 2: `COMPOSE_PROJECT_NAME` 대문자 사용 불가

### 에러

```
invalid project name "vllm_Qwen3.5-35B-A3B-serving_project":
must consist only of lowercase alphanumeric characters, hyphens,
and underscores as well as start with a letter or number
```

### 원인

Docker Compose 프로젝트명은 **소문자, 숫자, 하이픈(`-`), 언더스코어(`_`)만 허용**한다. 대문자와 점(`.`)은 사용 불가. x86_64(WSL) 환경에서는 경고 없이 통과했으나, aarch64 서버에서는 검증이 엄격하게 적용됨.

### 해결

env 파일의 `COMPOSE_PROJECT_NAME`을 소문자로 변경:

```bash
# 수정 전
COMPOSE_PROJECT_NAME=vllm_Qwen3.5-35B-A3B-serving_project

# 수정 후
COMPOSE_PROJECT_NAME=vllm_qwen35-35b-a3b_project
```

`CONTAINER_NAME`, `SERVING_MODEL_NAME` 등 다른 변수는 대문자/점 사용 가능.

---

## 이슈 3: 프로젝트명 변경 후 컨테이너 이름 충돌

### 에러

```
Error response from daemon: Conflict. The container name
"/Qwen3.5-35B-A3B-serving-container" is already in use by container "becfaddc..."
```

### 원인

이슈 2 해결을 위해 `COMPOSE_PROJECT_NAME`을 변경했으나, 이전 프로젝트명으로 생성된 컨테이너가 같은 `CONTAINER_NAME`으로 남아있어 충돌.

### 해결

이전 컨테이너를 제거 후 재실행:

```bash
docker rm Qwen3.5-35B-A3B-serving-container
docker compose --env-file envs/.env.Qwen3.5-35B-A3B-normal --profile serve up
```

---

## 이슈 4: 빌드 시간 차이

| 플랫폼 | 빌드 시간 | 비고 |
|--------|----------|------|
| x86_64 (WSL) | ~95초 | pip 캐시 활용 |
| aarch64 서버 | ~2305초 (약 38분) | ARM용 wheel 빌드 포함, 초회 빌드 |

aarch64에서는 일부 패키지가 pre-built wheel을 제공하지 않아 소스에서 빌드되므로 초회 빌드 시간이 길다. 이후 Docker 레이어 캐시가 적용되면 재빌드 시간은 크게 단축된다.

---

## 결과

상기 이슈 모두 해결 후 aarch64 플랫폼 서버에서 빌드 및 서빙 정상 동작 확인.
