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
- `output/multi/configs/nv4-f8-512k-mmp.yaml`
- `output/multi/configs/nv4-f8-512k-mmp.sh`
- `output/multi/envs/.env.nv4-f8-512k-mmp`

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

- **triplet**: 서빙에 원리적으로 필수(면제 불가). 이 셀의 특징은 `hf-overrides`에 실린 YaRN
  rope 확장 인자(factor=2) — 262144 네이티브 컨텍스트를 524288로 확장하는 R8 교정의 실물이다.
- **runtime_patch**: 없음 — 이 모델·조합은 `<model>_patch.py` 형태의 processor/config shim을
  요구하지 않는다(vLLM 표준 rope_parameters 오버라이드만으로 충분).
- **build_patch_pre**: 있음(60/62/64 — nvfp4-mixed·ple-mmap·qsa-fp8kv) — NVFP4 체크포인트의
  MoE 믹스드 정밀도, PLE mmap 지원, QSA(quantized state attention) fp8 KV 경로가 소스 패치로
  컴파일 전에 들어간다. 이 셀은 PLE=mmap·kv=fp8_e4m3이므로 62·64 둘 다 실제로 먹었다.
- **build_patch_post**: 있음(DeepGEMM·triton-kernels·mxfp4-triton-sm121·humming-nvml) — 빌드
  바깥 네이티브 의존. 이 조합이 구체적으로 어느 패치를 쓰는지는 엔진 로그 교차검증 전까지는
  "관측불가"로 남긴다(2-signal — 파일 존재+선언뿐, 발화 로그 대사는 안 했다).
- **build_recipe**: 있음 — Dockerfile.source-build(vLLM 0.29.0rc6 소스빌드) + requirements.txt.
  이 이미지(digest sha256:85cef27...)는 캠페인 전 셀이 공유한다.
- **compose**: 있음 — 멀티노드 2노드 Ray 오케스트레이션(docker-compose.yaml). 마스터/슬레이브
  역할 분기, 워치독·예산게이트 스크립트가 이 위에서 동작한다.
- **fork_pin**: 없음(stock vLLM v0.29.0rc6, 포크 오버라이드 없음) — `.env.nv4-f8-512k-mmp`에
  `VARIANT=` 줄이 없는 것이 그 증거다.
