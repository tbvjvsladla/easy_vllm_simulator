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
| `build_recipe` | build | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/single/native/configs/vela-qwen3-4b-vendor.yaml`
- `output/single/native/configs/vela-qwen3-4b-vendor.sh`
- `output/single/native/envs/.env.vela-qwen3-4b-vendor`

**build_recipe**
- `output/single/native/requirements.txt`

**compose**
- `output/single/native/configs/vela-qwen3-4b-vendor.sh`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

**triplet — 해당.** 서빙 설정·러너·환경 세 개가 없으면 이 조합을 세울 수 없다. 이 평면에서
러너는 특히 중요하다 — 기동 주체가 벤더 CLI 라서, 어떤 인자와 **어떤 환경변수**로 부르는지가
러너에만 적혀 있다. 그중 커널 모드 고정은 성능 재현의 필수 조건이다(서사 ④).
`.env` 는 **형상만** 실렸다(값은 각자 환경의 것) — 키 이름이 재현 정보이고 값은 지문이다.

**build_recipe — 해당.** 이 평면에는 이미지 빌드가 없다. 대신 **고정 의존 집합**이 재현 레시피다.
실린 핀 목록은 실제로 돈 환경의 실측이며 합성분이 없다. 엔진 버전이 **정확히** 맞아야 하는
조합이라(플러그인이 특정 버전 내부에 결합한다) 이 핀 집합이 곧 성립 조건이다.
⚠ 적용 증거는 **관측 불가(2-signal)** 다 — 이 슬롯의 관측원은 이미지 태그 대조인데 네이티브
평면에는 이미지가 없다. '없음' 이 아니라 '모름' 이며, 그 사실을 숨기지 않는다.

**compose — 해당(러너로 충족).** 이 슬롯의 뜻은 *기동 방법* 이고, 네이티브 평면의 기동 방법은
compose 파일이 아니라 러너다. 그래서 트리플렛 러너와 같은 파일을 가리킨다 — 중복이 아니라
**같은 사실을 두 뜻에서 요구**하는 것이며, 이 페이로드가 `plane=native` 로 그 사실을 밝힌다.
적용 증거는 build_recipe 와 같은 이유로 관측 불가다.

**runtime_patch — 불해당.** 이 서빙은 Python 런타임 패치를 쓰지 않았다. 플러그인이 이미 자기
결합을 수행하므로 그 위에 얹을 shim 이 필요하지 않았다. 파일도 없고 적용 흔적도 없다.

**build_patch_pre / build_patch_post — 불해당.** 소스를 컴파일하지 않았다. 배포본이 사전컴파일된
자산을 싣고 오고 엔진은 공식 배포판을 그대로 쓴다 — 패치를 끼울 컴파일 단계 자체가 없다.

**fork_pin — 불해당(stock).** 포크나 변종 핀을 쓰지 않았다. 공식 배포판 엔진 + 배포본 wheel 조합
그대로이며, `.env` 에 변종 선언 줄이 없는 것이 그 사실의 표현이다.
