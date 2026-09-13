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
- `output/single/configs/nv4-dev-512k.yaml`
- `output/single/configs/nv4-dev-512k.sh`
- `output/single/envs/.env.nv4-dev-512k`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet — 적용.** 서빙에 원리적으로 필수다. 이 형상의 핵심은 세 줄이다 —
  `kv-cache-memory-bytes`(16 GiB 절대클램프) · `max-model-len: 524288`(YaRN factor 2) · `enforce-eager`.
  PLE 배치는 상주(오프로드 노브 없음) 로 선언된다. `max-num-seqs: 8` 도 이 트리플렛의 통제변인이다 —
  값이 다르면 부하 곡선이 비교 불가가 된다.
- **runtime_patch — 불해당.** Python processor/config shim 이 필요한 지점이 없었다. 이 아치 지원이
  엔진 커밋에 이미 들어와 있어 런타임에 모델 설정을 고칠 자리가 나오지 않았다.
- **build_patch_pre — 불해당.** 컴파일 전 소스 수정 없음. 선행 계획서가 자체이식하려던 NVFP4 PLE
  패치는 착수 시점에 upstream main 이 `ModelOptMixedPrecisionConfig` 로 정식화해 불필요해졌다.
- **build_patch_post — 불해당.** 빌드 바깥 native 의존을 설치할 일이 없었다. 이 트랙은 컨테이너를
  빌드하지 않고 배포 wheel 을 쓴다.
- **build_recipe — 적용.** 컨테이너를 만들지 않으므로 재현 입력은 **엔진 커밋 핀 하나**다.
  `wheels.vllm.ai` 인덱스는 **40자 full SHA** 여야 한다(7자 축약은 404).
- **compose — 적용.** compose 를 실행하지는 않지만 *어떤 env 가 필요한가*의 형상은 재현에 필수다.
  값은 각자 manifest 에서 온다 — 운영자 경로는 싣지 않는다.
- **fork_pin — 불해당.** 포크가 필요 없었다. 필요했던 것은 포크가 아니라 **더 새로운 upstream 커밋**이다.
