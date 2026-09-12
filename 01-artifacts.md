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
- `output/multi/configs/nv4-bf-262k-res-kv8g-gmu80.yaml`
- `output/multi/configs/nv4-bf-262k-res-kv8g-gmu80.sh`
- `output/multi/envs/.env.nv4-bf-262k-res-kv8g-gmu80`

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

- **triplet (3-signal)** — 이 레시피의 정체 그 자체다. `gpu-memory-utilization: 0.80` ·
  `kv-cache-memory-bytes: 8589934592` · `max-num-seqs: 8` · `max-num-batched-tokens: 2048` 네 값이
  res 를 여는 조합이며, 하나라도 되돌리면 워치독 사정거리로 들어간다(02 서사 참조). `.env` 에
  `VLLM_PLE_MMAP` 줄이 **없다**는 사실이 곧 PLE resident 선언이다 — 부재가 기본값이다.
- **build_patch_pre (3-signal)** — `60-qwen4exp-nvfp4-mixed.sh`(NVFP4 혼합정밀 로더)와
  `64-qwen4exp-qsa-fp8kv.sh` 는 이 아키텍처가 vLLM 0.29.0rc6 에서 서는 전제다.
  `62-qwen4exp-ple-mmap.sh` 는 이 셀에서 **켜지 않았지만** 포함한다 — 같은 이미지가 mmap 자매셀도
  서빙하며, 스위치는 빌드가 아니라 `.env` 가 쥔다(빌드/서브 평면 분리).
- **build_patch_post (2-signal)** — 컴파일 이후 native 의존(deepgemm·triton 커널·mxfp4 sm121·
  humming NVML). 서빙 로그로 개별 발화를 관측하지 않았으므로 **관측불가**로 정직하게 표시한다.
- **build_recipe / compose (3-signal)** — 이미지를 재현하는 최소 집합. 이미지는 **전송하지 않고
  각 노드가 자기 것을 빌드한다**(이 배포 라인의 불변식) — 그래서 digest 가 아니라 이 레시피가
  재현의 단위다.
- **runtime_patch — 불해당** — 이 모델은 serve 시점 Python shim 이 필요 없다. 부재는 통과가 아니라
  **해당 없음**이며, 있었다면 `policy:RUNTIME_PATCH_NO_CARRY_FORWARD` 에 따라 이 셀 전용으로만 쓴다.
- **fork_pin — 불해당(stock)** — 포크 핀 없이 상류 `v0.29.0rc6` 태그로 빌드했다. `.env` 에
  `VARIANT=` 줄이 없는 것이 그 선언이다.
