# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 2-signal(file+declaration) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 없음 | 2-signal(file+declaration) |
| `build_recipe` | build | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/single/configs/fp8-off-512k.yaml`
- `output/single/configs/fp8-off-512k.sh`
- `output/single/envs/.env.fp8-off-512k`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

이 레시피에 **실제로 필요했던 것**과, 없는 것이 왜 없는지를 적는다.

- **`triplet`(적용)** — `fp8-off-512k.{yaml,sh}` + `.env`. 이 조합의 핵심은 세 줄이다:
  `kv-cache-memory-bytes`(절대 클램프) · `hf-overrides`(YaRN) · `enforce-eager`.
  ★ **`hf-overrides` 가 없으면 512K 는 로드 진입에서 죽는다.** 그리고 그 값에
  `max_position_embeddings: 524288` 이 **함께** 들어가야 한다 — vLLM 은 `rope_type` 이 yarn
  계열이면 factor 를 곱하지 않고 이 필드를 그대로 상한으로 쓴다(`config/model.py::
  _get_and_verify_max_len` 의 명시 분기). `rope_parameters` 만 보내면 **확장이 조용히 무효**가
  되고, 게이트는 "rope 인자가 있다"만 보므로 통과시킨다(근거: `devlog_26091314` §되풀이하지 말 것 4).
- **`build_recipe`(적용)** — 이 트랙은 컨테이너를 만들지 않는다. 재현 입력은 **엔진 커밋 핀**이다:
  `https://wheels.vllm.ai/30118ba27d1d923bdd91f97d945528dcb4a862c1`. 7자 축약 SHA 는 404 다.
- **`compose`(적용 · 형상만)** — 이 트랙은 compose 를 쓰지 않고 `vllm serve` 를 호스트 프로세스로
  띄운다. 대신 어떤 env 가 필요한지의 **형상**을 싣는다(값은 각자 manifest 에서 온다).
- **`runtime_patch`(불해당)** — Python processor/config shim 이 필요한 지점이 없었다. 아치 지원이
  엔진에 이미 있고, 모델 설정을 런타임에 고쳐야 할 자리가 나오지 않았다.
- **`build_patch_pre` / `build_patch_post`(불해당)** — 선행 계획서는 자체이식 3종(NVFP4 PLE ·
  PLE 오프로드 · KV fp8)을 계획했으나, 앞의 둘은 **upstream main 에 정식 기능으로 들어와** 패치가
  불필요해졌고(`ModelOptMixedPrecisionConfig` · `Qwen4ExpPLEPinnedHostEmbedding`), KV fp8 은
  upstream 도 여전히 막혀 있어 **패치로 열 수 있는 것이 아니다**(아래 §2).
- **`fork_pin`(불해당)** — 포크가 필요 없었다. 필요한 것은 포크가 아니라 **더 새로운 커밋**이었다.

