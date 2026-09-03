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
- `output/single/configs/gpt-oss-20b-gb10.yaml`
- `output/single/configs/gpt-oss-20b-gb10.sh`
- `output/single/envs/.env.gpt-oss-20b-gb10`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet(적용)**: mxfp4 prequantized·GQA(sliding+full 12/12) 모델을 "최대 컨텍스트"
  전략(max-model-len=131072)으로 서빙하려면 절대 KV 클램프(`kv-cache-memory-bytes`)·batch(`max-num-seqs`)·
  실측 attention backend가 트리플렛 안에 고정돼 있어야 재현된다. 이 값들은 전부 Phase 2 실측 산물이라
  파일 없이는 재현이 원리적으로 불가능하다(devlog §2).
- **runtime_patch(불해당)**: gpt-oss-20b는 Python processor/config 불일치가 없었다 — vLLM 0.18.0의
  기본 harmony/tiktoken 처리 경로를 그대로 썼고, 별도 shim이 arm된 적이 없다(testlog "실서빙(최종)
  헬스·기능 확인" 절 — 코드 수정 없이 표준 트리플렛만으로 기동).
- **build_patch_pre/post(불해당)**: 이 캠페인의 vLLM 버전 변경(0.19.1→0.18.0)은 **다운그레이드**였고
  wheel 트랙(torch 2.10대)을 그대로 유지했다 — 소스 컴파일 자체가 없었으므로 pre/post 빌드 패치가
  성립할 자리가 없다(devlog §1 "S1~S4" — `build_track.decision=wheel` 불변).
- **build_recipe(적용)**: 공유 이미지 자체가 0.19.1→0.18.0으로 바뀌었으므로 `Dockerfile`·
  `requirements.txt`가 이 hint의 재현에 필수다. ⚠ **compose의 `build:` 스탠자는 이 Dockerfile을
  가리키지 않는다**(아래 서사 참조) — 이미지 실물은 `docker build -f Dockerfile`로 직접 만들어야
  했다(devlog §1 "함정").
- **compose(적용)**: 단일노드 표준 기동 경로(`docker compose --env-file .env --env-file
  envs/.env.<config> --profile serve up`)가 이 조합에서 그대로 성립함을 실측했다(NAS 마운트 정합
  포함 — devlog §1 "함정"에서 project `.env` 누락 시 오마운트가 재현됨을 실제로 겪었다).
- **fork_pin(불해당)**: stock vLLM 0.18.0으로 서빙됐다 — 포크·변종 이미지 태그가 필요하지 않았다
  (`.env`에 `VARIANT=` 줄 없음이 곧 stock 선언).
