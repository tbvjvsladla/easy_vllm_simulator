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
- `output/multi/configs/nv4-bf-1m-mmp.yaml`
- `output/multi/configs/nv4-bf-1m-mmp.sh`
- `output/multi/envs/.env.nv4-bf-1m-mmp`

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

- **triplet**: 서빙 재현에 원리적으로 필수. config.yaml에 R8 YaRN 번역기 오버라이드(`hf-overrides`,
  factor=4, original_max_position_embeddings=262144)가 인라인돼 있어, 이 파일 없이는 1m 컨텍스트
  진입 자체가 불가능하다(오버라이드 없이 서빙하면 위치 인코딩이 공식 상한을 벗어나 붕괴).
- **runtime_patch**: 불해당. YaRN 번역은 config.yaml의 hf-overrides로 vLLM 엔진 초기화 시점에
  주입되며, 별도 Python processor/config shim이 필요 없다.
- **build_patch_pre**: NVFP4 MoE 커널 활성화(60)와 PLE mmap 지원(62)이 컴파일 전 소스 패치로
  필요하다 — NVFP4 체크포인트를 이 하드웨어(sm_121)에서 서빙하려면 두 패치 모두 전제조건이다.
- **build_patch_post**: 빌드-바깥 네이티브 의존(DeepGEMM/Triton 커널 등)이 이미지에 포함돼야
  런타임에 관측 가능한 성능(spec decoding 가속 등)이 나온다. 이 특정 셀에서 발화 여부를 직접
  관측하지는 못했다(2-signal).
- **build_recipe**: 이미지 재현의 원리적 필수 슬롯 — Dockerfile.source-build가 vLLM 0.29.0rc6
  소스빌드 경로를 정의한다.
- **compose**: 멀티노드(TP=2, Ray head/worker) 오케스트레이션 필수 — 단일 컨테이너로는 이
  모델(2×GB10 분산)을 서빙할 수 없다.
- **fork_pin**: 불해당. stock vLLM v0.29.0rc6 위에서 서빙됐다 — 포크/커스텀 리포지토리를 쓰지
  않았다(`.env`에 `VARIANT=` 줄 없음이 곧 stock 선언).
