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
- `output/single/configs/h-fp8w-kvfp8-yarn1m-tp2.yaml`
- `output/single/configs/h-fp8w-kvfp8-yarn1m-tp2.sh`
- `output/single/envs/.env.h-fp8w-kvfp8-yarn1m-tp2`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet — 적용, 그대로 재현 가능**: `quantization: fp8`(가중치) · `max-model-len:
  1000000`(YaRN factor=4.0) · `hf-overrides`(rope_parameters YaRN 오버라이드 — 모델의 실제
  `config.json#text_config.rope_parameters` 값을 그대로 쓰고 `rope_type` 만 default→yarn,
  `factor=4.0`) · `serve_env: VLLM_ALLOW_LONG_MAX_MODEL_LEN=1` · KV **fp8 양자화** ·
  `kv-cache-memory-bytes: 67517949317`(절대 KV 클램프, batch=4 기준 Phase-2 실측 수렴값) ·
  **`tensor-parallel-size: 2`(GPU 2장 전부 사용, 명시 필수)** · `--tool-call-parser
  qwen3_coder --reasoning-parser qwen3`.
- **build_recipe/compose — 조건부 적용(이 환경에서는 미사용)**: Docker-in-Docker 불가 호스트라
  venv 직접설치+네이티브 프로세스로 대체(02-narrative.md 참조). Docker 가용 호스트에서는 이
  슬롯이 정본.
- **fork_pin — 불해당(stock)**: vLLM 공식 릴리즈 wheel(0.28.0). 포크·소스패치 없음.
- **runtime_patch — 불해당**: `hf-overrides` 는 vLLM CLI 플래그 그대로 — 별도 config shim
  불요, 트리플렛 SERVE_KNOB 확장으로 성립.
- **build_patch_pre/post — 불해당**: 소스빌드 안 함(prebuilt wheel 직접설치 트랙).
