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
- `output/multi/configs/b0-kvfp8-attnauto-moeauto.yaml`
- `output/multi/configs/b0-kvfp8-attnauto-moeauto.sh`
- `output/multi/envs/.env.b0-kvfp8-attnauto-moeauto`

**build_patch_pre**
- `output/multi/build_patches_src/50-dsv4-sm12x-port.sh`
- `output/multi/build_patches_src/55-src-deps-authority.sh`

**build_patch_post**
- `output/multi/build_patches/10-deepgemm.sh`
- `output/multi/build_patches/20-triton-kernels.sh`
- `output/multi/build_patches/30-mxfp4-triton-sm121.sh`
- `output/multi/build_patches/40-humming-nvml-gb10.sh`

**build_recipe**
- `output/multi/Dockerfile`
- `output/multi/requirements.txt`

**compose**
- `output/multi/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **`triplet` (해당)** — 이 판을 성립시킨 것은 세 값이다: KV 절대클램프 **55,993 MiB/노드**,
  `distributed-executor-backend: ray`, `tensor-parallel-size: 2`. 뒤의 둘은 **yaml 에 함께** 있어야
  한다 — TP>1 인데 분산 스탠자가 없으면 조용히 단일 노드로 뜬다. `.env` 의
  `MASTER_CONTAINER_NAME`/`SLAVE_CONTAINER_NAME` 도 이 슬롯에 산다(없으면 워치독 필터가 비어
  방어가 꺼진다).
- **`build_recipe` (해당)** — stock vLLM 0.19.0 prebuilt wheel. **양 노드가 같은 이미지를 각자
  빌드해야 한다** — 노드 간 이미지 전송은 금지다. 그러려면 이미지를 지은 레시피가 페이로드에 있어야 한다.
- **`compose` (해당)** — 멀티에서는 기동 방법이 곧 토폴로지다. `network_mode: host` · Ray head/worker
  역할 · 포트가 이 파일에 있고, 이것 없이는 "같은 이미지"만으로 재현되지 않는다.
- **`runtime_patch` (불해당)** — Python shim 이 필요 없었다. 0.19.0 stock 이 harmony 를 든다.
- **`build_patch_pre` / `build_patch_post` (불해당 · 파일은 있다)** — `output/multi/build_patches_src`
  에 2건, `build_patches` 에 4건이 남아 있으나 **활성 wheel 레시피가 그 디렉터리를 참조하지 않는다**.
  wheel 트랙은 컴파일 자체가 없으므로 하나도 실행되지 않았다. 먹지 않은 패치를 재현지침으로 배포하면
  다음 사람이 그것이 필요하다고 믿는다 — 그래서 **불해당**이다(수집기가 `excluded_by_recipe` 로 사유를
  남긴다).
- **`fork_pin` (불해당)** — `.env` 에 `VARIANT=` 줄이 없다 = stock. arch-wall 사다리에 진입하지 않았다.
