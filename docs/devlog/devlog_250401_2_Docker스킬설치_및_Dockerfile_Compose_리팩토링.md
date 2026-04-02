# devlog 250401 — Docker 스킬 설치 및 Dockerfile/Compose 리팩토링

## 작업 요약

이전 세션에서 설계한 SDD 구조를 실제 코드에 반영했다.
Docker 공식 스킬 패키지 설치, CLAUDE.md/SKILL.md 검토·수정, 그리고 Dockerfile + docker-compose.yaml을 개발전략에 맞게 리팩토링했다.

---

## 1. Docker 스킬 패키지 설치

### 시도 1: 플러그인 마켓플레이스 설치 (실패)

```bash
/plugin marketplace add OpenAEC-Foundation/Docker-Claude-Skill-Package
# → Error: Marketplace file not found at .../.claude-plugin/marketplace.json
```

**원인**: 이 레포는 마켓플레이스 플러그인(`.claude-plugin/marketplace.json` 필요)이 아니라 **마크다운 기반 스킬 패키지**다. Claude Code의 `/plugin marketplace add`는 마켓플레이스 구조를 가진 레포만 지원한다.

### 시도 2: Git Submodule 설치 (성공)

```bash
git submodule add https://github.com/OpenAEC-Foundation/Docker-Claude-Skill-Package.git .docker-skills
```

22개 스킬(5개 카테고리)이 `.docker-skills/skills/source/`에 설치되었고,
`.claude/CLAUDE.md`에서 참조 경로를 등록하여 Docker 관련 작업 시 스킬을 활용할 수 있게 했다.

**교훈**: Claude Code 스킬 패키지를 프로젝트 로컬로 설치할 때는 git submodule + CLAUDE.md 참조가 가장 실용적인 방법이다.

---

## 2. CLAUDE.md / SKILL.md 검토 및 수정

### 2-1. CLAUDE.md 파일 위치

`.claude/CLAUDE.md`는 프로젝트 루트의 `CLAUDE.md`와 동일한 스코프로 Claude Code가 자동 로드한다.
프로젝트 루트를 깔끔하게 유지하기 위해 `.claude/` 안에 배치했다.

### 2-2. 발견된 문제 및 수정

**문제 1: 파일 구조와 실제 경로 불일치**

CLAUDE.md에 `.claude/skills/vllm-serving/` (디렉토리)로 적혀 있었으나 실제 파일은 `.claude/skills/vllm-serving-SKILL.md` (단일 파일)이었다.

```diff
- │   └── skills/
- │       └── vllm-serving/       # 커스텀 스킬
+ │   └── skills/
+ │       └── vllm-serving-SKILL.md  # 커스텀 스킬
```

**문제 2: serve.sh 템플릿의 envsubst 누락**

SKILL.md의 serve.sh 템플릿에서 주석에는 `# envsubst로 환경변수 치환 후 vllm serve 실행`이라고 적혀 있었지만, 실제 코드에서 envsubst를 사용하지 않았다.

serve-args.yaml에 `${MODEL_PATH}`, `${SERVE_PORT}` 같은 셸 변수가 있는데, Python의 `yaml.safe_load`는 이를 리터럴 문자열로 읽는다.

```diff
- # serve-args.yaml에서 인자값 읽기
- # yq 또는 python yaml parser 사용
- parse_yaml_args() {
-     python3 -c "
- import yaml, sys
- with open('${ARGS_FILE}') as f:
-     args = yaml.safe_load(f)
+ # serve-args.yaml에서 환경변수 치환 후 인자값 읽기
+ parse_yaml_args() {
+     envsubst < "${ARGS_FILE}" | python3 -c "
+ import yaml, sys
+ args = yaml.safe_load(sys.stdin)
```

`envsubst`로 YAML을 파이프하여 환경변수를 실제로 치환한 뒤 Python으로 파싱하도록 수정했다.

---

## 3. docker-compose.yaml 리팩토링

### 3-1. Before (기존)

```yaml
services:
  vllm:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: vllm_server
    ports:
      - "7900:8000"
    volumes:
      - /home/ash/.../hf_model:/app/models
      - /home/ash/.../tiktoken_encodings:/encodings:ro
    environment:
      TIKTOKEN_ENCODINGS_BASE: /encodings
    ipc: host
    ulimits:
      memlock: -1
      stack: 67108864
    runtime: nvidia
    tty: true
    stdin_open: true
```

**문제점**: 단일 서비스, anchor/alias 없음, profiles 없음, serve 모드 미지원

### 3-2. After (리팩토링)

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
    ports:
      - "7900:8000"
    environment:
      TIKTOKEN_ENCODINGS_BASE: /encodings
    command: ["bash"]
    tty: true
    stdin_open: true

  vllm-serve:
    <<: *gpu-common
    container_name: vllm_serve
    profiles: [serve]
    ports:
      - "7900:8000"
    env_file:
      - envs/${MODEL_ENV}.env
    command: ["bash", "/app/configs/${MODEL_ENV}/serve.sh"]
```

### 3-3. 설계 결정 기록

**결정 1: vllm 서비스에 profile을 부여하지 않음**

초기에는 `profiles: [interactive]`를 부여했으나, Docker Compose에서 profile이 있는 서비스는 `docker compose up`만으로 시작되지 않는다.
bash 모드가 기본 동작이므로 profile을 제거하여 `docker compose up`으로 바로 진입하도록 변경했다.

- `docker compose up` → bash 모드 (기본)
- `MODEL_ENV=xxx docker compose --profile serve up` → 서빙 모드

**결정 2: `${MODEL_ENV:?}` 가드 제거**

Docker Compose는 profile과 무관하게 **모든 서비스의 변수를 파싱 시점에 치환**한다.
`${MODEL_ENV:?MODEL_ENV is required}` 문법은 vllm-serve 서비스가 비활성 상태여도 파싱 에러를 발생시킨다.

```bash
$ docker compose up
# → error while interpolating services.vllm-serve.env_file.[]:
#   required variable MODEL_ENV is missing a value
```

`${MODEL_ENV}`로 변경하여, MODEL_ENV 미지정 시 env 파일(`envs/.env`)이 존재하지 않아 자연스럽게 실패하도록 했다. serve 프로파일을 사용하지 않으면 해당 서비스가 무시되므로 문제없다.

**결정 3: 명시적 `command: ["bash"]` 추가**

`tty: true` + `stdin_open: true`만으로는 `docker compose up` 시 bash 셸에 진입하지 않는다.
NGC 베이스 이미지의 기본 CMD에 의존하지 않고 명시적으로 bash를 지정했다.
컨테이너가 bash 대기 상태로 뜨면 `docker exec -it vllm_server bash`로 접속한다.

---

## 4. Dockerfile 리팩토링

### 변경 사항

| 항목 | Before | After | 근거 |
|------|--------|-------|------|
| BuildKit 선언 | 없음 | `# syntax=docker/dockerfile:1` | Docker 스킬: ALWAYS 포함 필수 |
| pip 캐시 | 없음 | `--mount=type=cache,target=/root/.cache/pip` | requirements.txt 재설치 시 캐시 활용 |
| 포트 문서화 | 없음 | `EXPOSE 8000` | 서빙 포트 메타데이터 |

---

## 5. .dockerignore 생성

Docker 빌드 최적화 스킬에서 "ALWAYS create a `.dockerignore`"를 권장.
빌드 컨텍스트에서 불필요한 파일을 제외하여 빌드 속도를 개선한다.

```
.git
.gitmodules
.claude/
.docker-skills/
*.md
LICENSE
envs/
configs/
docker-compose.yaml
.dockerignore
.vscode/
.idea/
```

`envs/`와 `configs/`는 빌드 시 불필요(볼륨 마운트로 제공)하므로 제외.

---

## 6. CLAUDE.md 업데이트 반영 사항

리팩토링 결과에 맞춰 CLAUDE.md의 다음 항목들을 수정했다:

- 실행 모드 설명: `docker compose --profile interactive up` → `docker compose up`
- 서비스 설명: `vllm (interactive profile)` → `vllm (profile 없음, 기본 서비스)`
- 자주 사용하는 명령어: bash 모드 명령어 수정
- MODEL_ENV 안전장치 설명: `:?` 가드 → 자연 실패 방식

---

## 7. 최종 파일 현황

```
vllm_serving_server/
├── .claude/
│   ├── CLAUDE.md                    # 프로젝트 가이드라인 (수정됨)
│   ├── settings.local.json
│   └── skills/
│       └── vllm-serving-SKILL.md    # 커스텀 스킬 (수정됨)
├── .docker-skills/                  # [submodule] Docker 공식 스킬 (신규)
├── .dockerignore                    # 빌드 컨텍스트 제외 목록 (신규)
├── .gitmodules                      # submodule 정의 (자동 생성)
├── Dockerfile                       # BuildKit + pip cache 적용 (수정됨)
├── docker-compose.yaml              # anchor/alias + profiles 적용 (수정됨)
├── requirements.txt
└── docs/
    └── devlog/
```

---

## 8. 다음 작업 (TODO)

- [ ] `docker compose up -d` → `docker exec -it vllm_server bash`로 bash 모드 진입 테스트
- [ ] 컨테이너 내부에서 `python -c "import vllm; print(vllm.__version__)"` 확인
- [ ] `envs/` + `configs/` 디렉토리 생성 및 첫 번째 모델 서빙 설정 작성
- [ ] serve.sh 템플릿의 envsubst + yaml 파싱 로직 검증
- [ ] `MODEL_ENV=xxx docker compose --profile serve up`으로 서빙 모드 E2E 테스트
