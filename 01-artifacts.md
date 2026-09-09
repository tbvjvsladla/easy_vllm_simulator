# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 3-signal(file+evidence+declaration) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 없음 | 2-signal(file+declaration) |
| `build_recipe` | build | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/single/configs/b-fp8w-kvfp8-262k.yaml`
- `output/single/configs/b-fp8w-kvfp8-262k.sh`
- `output/single/envs/.env.b-fp8w-kvfp8-262k`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet — 적용, 그대로 재현 가능**: `quantization: fp8` · `max-model-len: 262144` ·
  `kv-cache-memory-bytes: 54737327753`(절대 KV 클램프, batch=6 기준 Phase-2 실측 수렴값) ·
  `kv-cache-dtype: fp8` · `tensor-parallel-size: 1`(GPU 2장 중 1장만 사용 — 명시 필수, 안 적으면
  `gpus_per_node` 로 오해된다) · `--tool-call-parser qwen3_coder --reasoning-parser qwen3`(모델의
  실제 tool_call/reasoning 출력 포맷과 vLLM 0.28.0 정적 레지스트리 대조로 확정 — "hermes" 아님).
  이 값들은 GPU/양자화/컨텍스트 조합이 같다면 docker든 native든 그대로 유효하다.
- **build_recipe/compose — 조건부 적용(이 환경에서는 미사용)**: 이 저장소의 표준 재현 경로는
  이 Dockerfile/compose 로 NGC 베이스 컨테이너를 빌드해 서빙하는 것이다(Docker 가용 호스트라면
  이 경로를 그대로 쓰면 된다). 단 **이 hint 를 만든 실제 환경은 Docker-in-Docker 가 불가능한
  비특권 컨테이너**였고, 여기서는 vLLM 0.28.0 + torch 2.13.0(+cu129)을 벤더 wheel로 `.venv` 에
  직접 설치하고 `vllm serve` 를 네이티브 프로세스로 띄웠다(GPU 격리는 `CUDA_VISIBLE_DEVICES=0`).
  이 대체 경로의 정확한 커맨드·env 변수·안전장치는 `02-narrative.md` 참조 — Docker 가 있는 호스트
  라면 이 슬롯(Dockerfile/compose)이 정본이고 네이티브 경로는 불필요하다.
- **fork_pin — 불해당(stock)**: vLLM 공식 릴리즈 wheel(0.28.0)을 그대로 썼다. 소스 패치·포크 핀
  없음.
- **runtime_patch — 불해당**: 이 모델(Qwen3.8-27B, qwen3_5 하이브리드)은 서빙 시 별도 config
  shim/monkeypatch가 필요 없었다. `gdn-prefill-backend: triton` 등은 vLLM CLI 플래그로 직접
  지정되며 별도 patch.py 파일이 필요 없다(트리플렛 yaml 안에 포함).
- **build_patch_pre/post — 불해당**: 소스 빌드를 하지 않았다(prebuilt wheel 직접설치 트랙).
