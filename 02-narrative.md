# 2. 서사 — 빌드와 서빙이 어떻게 진행되었나

> 이 절은 **전부 Agent 저작**이다. 기계는 여기 한 글자도 쓰지 않는다(plan §6).
> **원문 전재 금지**(plan Q2) — devlog 실물에는 운영자 경로·호스트 지문이 있어
> 그대로 옮기면 배포 PII 표면이 커진다. **재현에 필요한 교훈만 추려 새로 쓴다.**

## 읽을 원재료 (복사 대상 아님 · 포인터)

- devlog: `../devlog/devlog_26090914_run_trial_native모드_추가_및_광의탐색_셀A.B.C_구성.md`
- testlog: `../testlog/testlog_26090915_Qwen3.8.27B_광의탐색_3셀_FullBench_비교.md`

## 서사

**증상**: 이 프로젝트의 표준 재현 경로(NGC 베이스 컨테이너 → `docker compose up`)를 쓰려 했으나,
빌드 시작 전 `docker` 바이너리 자체가 없었다(devlog §"무엇을 했나" 1문단). 이 환경은 비특권
Docker 컨테이너라 Docker-in-Docker 가 구조적으로 불가능했다.

**원인**: 컨테이너 안에서 또 컨테이너를 띄우려는 시도였다 — 커널 cgroup 제어권이 없는 호스트에서는
근본적으로 막힌 길이다.

**해소**: vLLM 0.28.0 + torch 2.13.0(+cu129)을 프로젝트 전용 `.venv` 에 벤더 wheel로 직접 설치하고
(`uv pip install torch==2.13.0 --index-url https://download.pytorch.org/whl/cu129`, 이어서 vLLM
release wheel `--no-deps`), `vllm serve --config <triplet>.yaml --served-model-name Qwen3.8-27B
--enable-auto-tool-choice --tool-call-parser qwen3_coder --reasoning-parser qwen3` 를 `CUDA_VISIBLE_
DEVICES=0` 로 GPU 1장만 보이게 하고 **네이티브 프로세스**로 직접 띄웠다(devlog §"run_trial.py 변경
요지"). 측정 도구(GuideLLM)도 마찬가지로 `pip install guidellm`(container 아님)로 직접 실행했다.
이 대체 경로를 지원하려고 `run_trial.py`/`run_bench.sh`/`sweep_bench.sh` 에 `--exec-mode native`
를 신설했다(devlog 동절). Docker 가 있는 호스트에서는 이 우회가 전혀 필요 없다 — 표준
`docker compose up` 경로(01-artifacts.md 의 build_recipe/compose 슬롯)를 그대로 쓰면 된다.

## 되풀이하지 말 것

1. **`torchvision` 을 빼먹지 마라.** NGC 컨테이너 트랙에선 "base 제공분"이라 안 써도 됐지만, venv
   직접설치 트랙에선 실제로 없다. `Qwen3_5ForConditionalGeneration`(이 모델의 아키텍처 클래스)는
   텍스트 전용으로 써도 클래스 등록 단계에서 `qwen3_vl.py` → `Qwen2VLImageProcessor` 를 **정적
   import** 하고, 그게 torchvision 을 찾는다 — `--language-model-only` 플래그로도 이 import 자체는
   피할 수 없다(런타임이 아니라 클래스 로딩 시점이라). `uv pip install torchvision --index-url
   https://download.pytorch.org/whl/cu129 --no-deps` 로 torch 버전은 안 건드리고 추가하면 된다
   (testlog §환경 스냅샷 참조).
2. **`tensor-parallel-size` 를 반드시 트리플렛 yaml에 명시하라(이 노드 GPU 중 일부만 쓸 때).**
   생성기는 "TP=1은 vLLM 기본값이니 안 적는다"는 설계인데, 그 설계는 "이 노드의 GPU 전부를 쓴다"는
   전제 위에 있다. GPU 2장 중 1장만 쓰면 그 전제가 깨져 하류 스크립트(sweep_bench.sh)가 manifest
   의 `gpus_per_node`(=2)로 잘못 유추한다(testlog §알려진 결함/한계 참조). 명시하면 문제없다.
3. **"용처=Hermes" 를 vLLM 파서명으로 오독하지 마라.** 사용자가 서빙 UX 인터뷰에서 답하는 "용처"는
   클라이언트 측 사용 맥락(Hermes-Agent 연결 등)이지 vLLM `--tool-call-parser` 값이 아니다. 이
   모델의 실제 tool_call 출력은 `<tool_call><function=name><parameter=..>` XML 형식이고,
   vLLM 0.28.0 정적 레지스트리(`vllm/tool_parsers/__init__.py`)에서 `qwen3_coder`/`qwen3_xml` 이
   정확히 이 포맷을 위한 파서다(둘 다 `Qwen3EngineToolParser` 로 동일 구현). `hermes` 파서(JSON
   포맷)를 썼다면 tool_call 파싱이 깨졌을 것이다.
4. **`roofline.py` 의 R_fp 를 fp8 서빙의 물리 상한으로 곧이곧대로 믿지 마라.** 이 스크립트는 체크
   포인트의 원본(보통 bf16) 가중치 바이트만으로 계산해, fp8 로 서빙하면(실제 대역폭 요구가 절반)
   실측이 R_fp 를 가볍게 넘어서는 게 정상이다(testlog §외부 레퍼런스 절 참조). 판정기가 이걸
   "물리 초과"로 잡아 `NEEDS_RUBRIC` 을 낼 수 있다 — E 나 측정이 틀린 게 아니라 이 도구의 한계다.
