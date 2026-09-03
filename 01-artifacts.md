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
- `output/single/configs/gpt-oss-120b-gb10.yaml`
- `output/single/configs/gpt-oss-120b-gb10.sh`
- `output/single/envs/.env.gpt-oss-120b-gb10`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

**triplet — 해당.** 단일노드 최대 컨텍스트(131072) 구성이다. `gpu-memory-utilization 0.90` ·
`max-num-seqs 16` · `kv-cache-memory-bytes` **30 GiB 총량** · `port 8000`(브리지 네트워크 + 8080:8000
매핑이라 컨테이너 안쪽 포트다) 넷이 이 토폴로지에 묶여 있다. 같은 모델의 멀티노드 힌트는 이 넷이
전부 다른 값을 갖는다 — 옮기지 말고 그쪽 힌트를 보라.

**runtime_patch — 불해당.** stock harmony/tiktoken 경로가 그대로 서빙했다. Python 몽키패치가 필요한
지점이 없었고, 없는 패치 자리를 남기면 다음 사람이 그것을 필수 단계로 읽는다.

**build_patch_pre · build_patch_post — 불해당.** wheel 트랙이라 vLLM 을 컴파일하지 않으므로 컴파일
전/후 패치가 걸릴 자리가 없다. 활성 Dockerfile 이 두 디렉터리를 참조하지 않으며, 그래서 슬롯이
불해당이고 **archive 에도 싣지 않는다** — 실행되지 않은 패치는 재현 지침이 아니다.

**build_recipe — 해당.** 이미지를 다시 지으려면 필요하다. 이 트랙의 핵심은 **wheel 을 `--no-deps`
로 붙이고 의존성은 그 wheel 의 메타데이터에서 생성한 requirements 로 따로 넣는 것**이다 — 그래야
NGC 베이스가 제공하는 torch 를 덮지 않는다.

**compose — 해당.** 단일노드는 브리지 네트워크에 포트 매핑이 있다. 그 사실이 트리플렛 `port` 값의
의미를 정하므로 기동 방법을 빼면 재현이 성립하지 않는다. env 형상 템플릿을 함께 싣는 이유는
**어떤 변수가 필요한지**가 재현 정보이기 때문이다 — 값은 각자 manifest 에서 온다.

**fork_pin — 불해당.** stock 이다. 모델 env 에 `VARIANT=` 줄이 없는 것이 그 선언이며, 부재가
기본값이므로 포크 좌표를 걷어내는 일은 값 수정이 아니라 줄 삭제로 표현된다.
