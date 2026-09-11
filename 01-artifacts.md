# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 3-signal(file+evidence+declaration) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `build_recipe` | build | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/multi/configs/nv4-bf-512k-mmp.yaml`
- `output/multi/configs/nv4-bf-512k-mmp.sh`
- `output/multi/envs/.env.nv4-bf-512k-mmp`

**build_patch_pre**
- `output/multi/build_patches_src/50-dsv4-sm12x-port.sh`
- `output/multi/build_patches_src/55-src-deps-authority.sh`
- `output/multi/build_patches_src/60-qwen4exp-nvfp4-mixed.sh`
- `output/multi/build_patches_src/62-qwen4exp-ple-mmap.sh`
- `output/multi/build_patches_src/64-qwen4exp-qsa-fp8kv.sh`

**build_patch_post**
- `output/multi/build_patches/10-deepgemm.sh`
- `output/multi/build_patches/20-triton-kernels.sh`
- `output/multi/build_patches/30-mxfp4-triton-sm121.sh`
- `output/multi/build_patches/40-humming-nvml-gb10.sh`

**build_recipe**
- `output/multi/Dockerfile`
- `output/multi/Dockerfile.source-build`
- `output/multi/requirements.txt`

**compose**
- `output/multi/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet**: 이 셀의 서빙 파라미터를 고정한다 — 특히 **`hf-overrides` rope 확장 인자**
  (`{"text_config":{"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],
  "partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":2,
  "original_max_position_embeddings":262144}}}`)가 **이 셀의 재현에 절대 필수**다 — 없으면
  ValidationError 즉사(R8, §2 참조). A층, 면제 불가.
- **runtime_patch**: 불해당. serve-time shim 불요.
- **build_patch_pre**: NVFP4 mixed(60)·PLE mmap(62)·QSA fp8 KV(64) — 이미지 공통, kv=auto 라
  64 는 실질 미사용.
- **build_patch_post**: 0.29.0rc6 표준 빌드 구성, 이 셀 전용 아님.
- **build_recipe / compose**: 클러스터-와이드 이미지 정체성 — 멀티 TP=2 양 노드 동일 빌드 전제.
- **fork_pin**: 불해당(stock). YaRN 확장은 서빙 인자만으로 해결되며 포크가 필요 없었다.
