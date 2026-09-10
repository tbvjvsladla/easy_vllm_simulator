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
- `sync_staging/sub_slots_camp26090721/configs/d-tq3nc.yaml`
- `sync_staging/sub_slots_camp26090721/configs/d-tq3nc.sh`
- `sync_staging/sub_slots_camp26090721/envs/.env.d-tq3nc`

**build_recipe**
- `sync_staging/sub_slots_camp26090721/Dockerfile`
- `sync_staging/sub_slots_camp26090721/Dockerfile.source-build`
- `sync_staging/sub_slots_camp26090721/requirements.txt`

**compose**
- `sync_staging/sub_slots_camp26090721/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **`triplet`** — 적용: serve 시점 성립분. 이 셀을 가르는 유일한 축이 `kv-cache-dtype: turboquant_3bit_nc` 이고 나머지(max-model-len 32768 · kv-cache-memory-bytes 11811160064 · gmu 0.9)는 4군 공통 통제변인이다 · 서브 셀이라 메인이 회수 문서에서 재저작했다.
- **`runtime_patch`** — 불해당: stock 0.26.0 이 이 모델을 그대로 서빙했다 — processor/config shim 을 arming 한 적이 없고, 없어야 재현된다.
- **`build_patch_pre`** — 불해당: 소스 수정 없이 컴파일됐다. `build_patches_src/` 는 비어 있고 활성 Dockerfile 이 참조는 하되 적용할 파일이 0건이다.
- **`build_patch_post`** — 불해당: 빌드-바깥 native 의존 설치가 필요 없었다(추가 lib/커널 0건).
- **`build_recipe`** — 적용: source-build 트랙이라 이미지가 곧 실험 조건이다 — NGC 26.05-py3 위에서 vLLM `v0.26.0`(568afb3a) 를 컴파일한 레시피 없이는 같은 엔진이 재현되지 않는다.
- **`compose`** — 적용: KV 절대 클램프를 건 컨테이너를 어떻게 띄우는지가 이 측정의 절반이다. `.env` 실물은 배포하지 않고 변수 형상만 template 로 싣는다.
- **`fork_pin`** — 불해당: stock 이다 — `.env` 에 `VARIANT=` 줄이 없다. 포크 의존을 만들지 않았다.

> 이 태그가 가르는 것은 **KV 캐시 dtype 하나**다. 나머지 슬롯이 전부 비어 있다는 사실 자체가 결과다 — 이 모델·이 엔진에서 2.0~3.5× 용량은 **패치 없이** 얻어진다(arch-invariant: dtype 수용 여부는 엔진 기능이고 압축비는 층 구조에서 나온다).
