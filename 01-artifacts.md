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
- `output/multi/configs/nv4-f8-262k-mmp.yaml`
- `output/multi/configs/nv4-f8-262k-mmp.sh`
- `output/multi/envs/.env.nv4-f8-262k-mmp`

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

- **triplet**: 이 셀의 서빙 파라미터(kv-cache-dtype=fp8_e4m3, kv-cache-memory-bytes=20GiB,
  gpu-memory-utilization=0.85, MTP num_speculative_tokens=3, enforce-eager)를 고정한다 — 재현의
  최소 필수 자리(A층, 면제 불가).
- **runtime_patch**: 불해당. 이 모델(Qwen4ExpForConditionalGeneration)은 serve-time 프로세서/설정
  shim 이 필요 없다 — 표준 vLLM 경로로 로드된다.
- **build_patch_pre**: NVFP4 mixed-precision 지원(60), PLE mmap 지원(62), QSA fp8 KV 지원(64)이
  이 셀의 축 조합(variant=nv4 · kv=fp8_e4m3 · ple=mmap)을 각각 켠다 — 셋 다 컴파일 전 소스 패치라
  post 슬롯으로는 대체 불가.
- **build_patch_post**: DeepGEMM/Triton 커널·mxfp4 sm121 포트·humming NVML — 이 vLLM 버전
  (0.29.0rc6)의 표준 빌드 구성이며 이 셀에 특화된 것은 아니다.
- **build_recipe / compose**: 클러스터-와이드 이미지 정체성(BUILD_DOCKERFILE·VLLM_PRETEND_VERSION
  등)이 양 노드에 동일해야 하는 멀티 TP=2 전제(§노드 제어 규약)의 최소 필수 자리.
- **fork_pin**: 불해당(stock). `.env` 에 `VARIANT=` 줄이 없다 — 이 축은 포크를 요구하지 않는다.
