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
- `output/single/configs/d-fp8w-kvfp8-yarn524k.yaml`
- `output/single/configs/d-fp8w-kvfp8-yarn524k.sh`
- `output/single/envs/.env.d-fp8w-kvfp8-yarn524k`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet — 적용, 그대로 재현 가능**: `quantization: fp8`(가중치) · `max-model-len: 524288`
  (YaRN factor=2.0) · `hf-overrides`(rope_parameters YaRN 오버라이드 — 모델의 실제
  `config.json#text_config.rope_parameters` 값을 그대로 쓰고 `rope_type` 만 default→yarn,
  `factor=2.0`) · `serve_env: VLLM_ALLOW_LONG_MAX_MODEL_LEN=1` · KV **fp8 양자화** ·
  `kv-cache-memory-bytes: 53622289385`(절대 KV 클램프, batch=3 기준 Phase-2 실측 수렴값) ·
  `tensor-parallel-size: 1`(GPU 2장 중 1장만 — 명시 필수) · `--tool-call-parser qwen3_coder
  --reasoning-parser qwen3`.
- **build_recipe/compose — 조건부 적용(이 환경에서는 미사용)**: 자매 hint
  `hint/0.28.0/qwen3.8-27b/rtxpro6000-main-native/qfp8-len262144-kvfp8`(셀 B) 와 완전히 같은
  이유로 미사용 — Docker-in-Docker 불가 호스트라 venv 직접설치+네이티브 프로세스로 대체
  (02-narrative.md 참조). Docker 가용 호스트에서는 이 슬롯이 정본.
- **fork_pin — 불해당(stock)**: vLLM 공식 릴리즈 wheel(0.28.0). 포크·소스패치 없음.
- **runtime_patch — 불해당**: `hf-overrides` 는 vLLM 이 CLI 플래그로 직접 받는 값이라 별도
  Python config shim/monkeypatch 가 불필요하다 — 트리플렛(yaml/sh) 확장으로 성립한다(이번
  캠페인에서 `gen_recipe_set.py`/`run_trial.py` 에 SERVE_KNOB 로 추가함).
- **build_patch_pre/post — 불해당**: 소스빌드 안 함(prebuilt wheel 직접설치 트랙).
