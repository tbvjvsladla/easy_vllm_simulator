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
- `output/multi/configs/nv4-bf-262k-mmp.yaml`
- `output/multi/configs/nv4-bf-262k-mmp.sh`
- `output/multi/envs/.env.nv4-bf-262k-mmp`

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

- **triplet (3-signal)** — `VLLM_PLE_MMAP=1` 과 `VLLM_PLE_MMAP_DIR` 두 줄이 이 트랙의 정체다.
  그 줄이 있으면 n-gram 테이블 47.68 GiB 가 상주에서 빠져 예산 floor 가 16,064 → 40,478 로 열린다.
  스테이징 디렉터리(NVMe)가 실재해야 하며, 없으면 로드가 디스크를 못 찾는다.
- **build_patch_pre (3-signal)** — `62-qwen4exp-ple-mmap.sh` 가 **이 트랙의 전제**다(자매
  resident 태그에서는 포함하되 켜지 않는다). `60-nvfp4-mixed`·`64-qsa-fp8kv` 는 아키텍처가
  0.29.0rc6 에서 서는 조건.
- **build_patch_post (2-signal)** — 컴파일 이후 native 의존. 서빙 로그로 개별 발화를 관측하지
  않아 **관측불가**로 표시한다(부재와 미관측은 다른 사실이다).
- **build_recipe / compose (3-signal)** — 이미지를 전송하지 않고 각 노드가 빌드하므로 재현 단위는
  digest 가 아니라 이 레시피다.
- **runtime_patch — 불해당** · **fork_pin — 불해당(stock)**: `.env` 에 `VARIANT=` 줄이 없는 것이
  stock 선언이다.
