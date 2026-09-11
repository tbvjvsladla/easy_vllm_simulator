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
- `output/multi/configs/fp8-bf-512k-mmp.yaml`
- `output/multi/configs/fp8-bf-512k-mmp.sh`
- `output/multi/envs/.env.fp8-bf-512k-mmp`

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

- **triplet**: 서빙에 원리적으로 필수(면제 불가). `hf-overrides`에 YaRN factor=2 rope 확장
  인자가 실려 있다 — R8 교정의 재현 실물.
- **runtime_patch**: 없음 — 이 모델·조합은 processor/config shim을 요구하지 않는다.
- **build_patch_pre**: 있음(60/62/64) — FP8 체크포인트의 MoE 믹스드 정밀도·PLE mmap·QSA 경로가
  소스 패치로 컴파일 전에 들어간다. 이 셀은 kv=auto라 64(qsa-fp8kv)는 실질 미사용.
- **build_patch_post**: 있음(DeepGEMM 등) — 빌드 바깥 네이티브 의존, 이미지 공통. 발화 여부는
  엔진 로그 교차검증 전까지 관측불가(2-signal).
- **build_recipe**: 있음 — Dockerfile.source-build(vLLM 0.29.0rc6 소스빌드). 캠페인 전 셀 공유.
- **compose**: 있음 — 멀티노드 2노드 Ray 오케스트레이션. 마스터/슬레이브 역할 분기.
- **fork_pin**: 없음(stock vLLM v0.29.0rc6) — `.env`에 `VARIANT=` 줄 부재가 그 증거.
