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
- `output/single/configs/a0-kvfp8-attnauto-moeauto.yaml`
- `output/single/configs/a0-kvfp8-attnauto-moeauto.sh`
- `output/single/envs/.env.a0-kvfp8-attnauto-moeauto`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **`triplet` (해당)** — 이 노드의 서빙은 KV 절대클램프 하나로 서고 넘어졌다. `.yaml` 이 든
  `kv_cache_memory_bytes = 52,567,672,969 B`(50,133 MiB)와 `max_num_seqs = 16` 이 그 값이며,
  둘 중 하나만 바꿔도 이 호스트에서는 서지 않거나 워치독에 사살된다. 러너 `.sh` 는 예산 선언 →
  워치독 → 로드 순서를 강제한다(선언 없이 로드하면 방어가 없다).
- **`build_recipe` (해당)** — stock vLLM 0.18.0 prebuilt wheel 트랙이다. 이미지를 지은 레시피가
  없으면 같은 `_C` ABI 를 재현할 수 없으므로 이 슬롯은 선언으로 면제되지 않는다.
- **`compose` (해당)** — 기동 방법 자체다. 단일 노드라도 `docker-compose.yaml` 이 GPU 예약·
  마운트·포트를 든다. 이것 없이는 "무엇을 어떻게 띄웠나"가 남지 않는다.
- **`runtime_patch` (불해당)** — 이 조합은 Python processor/config shim 이 필요 없었다.
  gpt-oss 의 harmony 파서는 stock 이 이미 든다. **없어서 못 쓴 것이 아니라 쓸 일이 없었다.**
- **`build_patch_pre` / `build_patch_post` (불해당)** — arch-wall 을 만나지 않았다. stock 0.18.0 이
  sm_121 에서 그대로 컴파일·기동됐고, 소스 수정도 빌드-바깥 native 의존 설치도 없었다.
- **`fork_pin` (불해당)** — `.env` 에 `VARIANT=` 줄이 없다. **줄이 없는 것이 stock 이라는 선언**이며,
  포크 좌표를 쓰지 않았다는 사실이 그 부재로 표현된다(변종 좌표 거처 규약).
