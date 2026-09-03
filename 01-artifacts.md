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
- `output/multi/configs/gpt-oss-20b-gb10.yaml`
- `output/multi/configs/gpt-oss-20b-gb10.sh`
- `output/multi/envs/.env.gpt-oss-20b-gb10`

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

**triplet — 해당.** TP=2 서빙의 전부가 이 세 파일이다. `tensor-parallel-size: 2` 만으로는 뜨지
않고 `distributed-executor-backend: ray` 가 함께 있어야 한다. `gpu-memory-utilization` 은 단일노드
값(0.90)을 못 쓰고 0.80 이며, `kv-cache-memory-bytes` 는 **워커당** 값(5 GiB), `port` 는 8080 이다.
★ 이 조합의 `max-model-len` 은 **4096** 으로, 같은 모델의 단일노드 힌트(131072)와 다르다 — 분산
경로 자체를 재는 것이 목적이었기 때문이다. 컨텍스트를 늘리려면 KV 를 다시 실측하라(§3 참조).

**runtime_patch — 불해당.** stock harmony/tiktoken 경로가 그대로 서빙했다. 필요 없는 패치 자리를
남겨 두면 다음 사람이 그것을 필수 단계로 읽는다.

**build_patch_pre · build_patch_post — 불해당.** wheel 트랙은 vLLM 을 컴파일하지 않으므로 컴파일
전/후 패치가 걸릴 자리가 없다. 저장소에는 소스빌드 트랙용 패치가 렌더돼 있지만 이 이미지를 지은
Dockerfile 은 그 디렉터리들을 참조하지 않는다 — 그래서 슬롯이 불해당이고 **archive 에도 없다**.

**build_recipe — 해당.** 이미지를 다시 지으려면 반드시 필요하다. `pip install ray` 한 줄이 멀티의
필수 조건이다 — 0.18.0 wheel 은 ray 를 의존성으로 선언하지 않고 설치가 `--no-deps` 라 더더욱 들어오지
않는다. 단일노드용 같은 버전 이미지로 멀티를 띄우면 `ray: command not found` 로 죽는다.

**compose — 해당.** 멀티 compose 는 `network_mode: host` 라 포트 매핑이 없고, 그 사실이 트리플렛
`port` 값의 의미를 바꾼다. env 형상 템플릿을 함께 싣는 이유는 **어떤 변수가 필요한지**가 재현
정보이기 때문이다 — 값은 각자 manifest 에서 온다.

**fork_pin — 불해당.** stock 이다. 모델 env 에 `VARIANT=` 줄이 없는 것이 그 선언이며, 부재가
기본값이므로 포크 좌표를 걷어내는 일은 값 수정이 아니라 줄 삭제로 표현된다.
