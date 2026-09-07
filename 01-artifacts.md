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
- `sync_staging/sub_slots/configs/s1-native-fp8.yaml`
- `sync_staging/sub_slots/configs/s1-native-fp8.sh`
- `sync_staging/sub_slots/envs/.env.s1-native-fp8`

**build_recipe**
- `sync_staging/sub_slots/Dockerfile`

**compose**
- `sync_staging/sub_slots/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

> **이 슬롯들은 서브 노드에서 왔고, 메인이 문서기반으로 재저작한 것이다.** 상향 회수는 문서기반
> only 이고 코드·설정의 직접 회수는 금지다 — 서브가 자기 트리플렛을 testlog 로 발행하고
> (`testlog_26090717_camp7_s1-native-fp8_트리플렛_문서회수.md`) 메인이 그 코드펜스를 읽어 재저작했다.
> 값은 한 글자도 고치지 않았다.

- **`triplet` (해당)** — 이 판의 성립 조건은 KV 절대클램프 **64,426,421,846 B(61,442 MiB)** @
  `max_model_len 131072` · `batch 20` 이다. 형제 판(`gb10-main-native`)이 KV 50,133 MiB · batch 16
  인 것과 대비된다 — **batch 를 내주고 컨텍스트를 얻는 선택**이 이 트리플렛에 박혀 있다.
  러너 `.sh` 는 tiktoken 환경변수 주입 → 런타임 패치 arming → attention 백엔드 고정 → serve 순서를
  강제한다(그 순서가 곧 이 셀의 재현 절차다).
- **`build_recipe` (해당)** — stock vLLM 0.18.0 prebuilt wheel. 서브는 **자기 이미지를 자율 빌드**했다
  (노드 간 이미지 전송은 영구 금지다). 그러려면 이미지를 지은 레시피가 페이로드에 있어야 한다.
- **`compose` (해당)** — 기동 방법. 서브가 문서에 함께 남긴 사실 하나: compose 의
  `build.dockerfile` 기본값이 `Dockerfile.source-build` 인데 그 파일이 이 트리에 **없다**.
  이 셀은 `IMAGE_TAG` override 로 떠서 영향이 없었지만, 기본값으로 빌드하려는 사람은 여기서 막힌다.
  서브는 그것을 **고치지 않고 사실만** 남겼다(관측과 교정을 섞지 않았다).
- **`runtime_patch` (불해당)** — `s1-native-fp8_patch.py` 가 없다. 그래서 러너의 `arm_patch.sh`
  호출은 이 셀에서 **no-op** 이다. 부재가 곧 불해당이며, 러너에 호출이 있다는 것이 패치가 있다는
  뜻은 아니다.
- **`build_patch_pre` / `build_patch_post` (불해당)** — 서브가 `output/single/` 전체를 `find` 로
  확인해 매치 0건임을 문서에 남겼다. arch-wall 미조우 · stock 이다.
- **`fork_pin` (불해당)** — `.env` 에 `VARIANT=` 줄이 없다 = stock.
