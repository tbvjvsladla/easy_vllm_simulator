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
- `output/single/configs/gpt-oss-20b-gb10-h100sim.yaml`
- `output/single/configs/gpt-oss-20b-gb10-h100sim.sh`
- `output/single/envs/.env.gpt-oss-20b-gb10-h100sim`

**build_recipe**
- `output/single/Dockerfile`
- `output/single/requirements.txt`

**compose**
- `output/single/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

**`triplet` — 적용.** 서빙에 원리적으로 필수다. 이 레시피의 특징은 `config.yaml` 이 **타겟 GPU 를
선언**한다는 점이다(H100 · 80 GiB/카드 · `target_gmu` 0.90 · cards_per_node 1). 측정은 GB10
통합메모리에서 이뤄졌고 **클램프만 타겟 예산**이다 — 두 자리를 섞으면 거짓이 된다
(`policy:KV_ABSOLUTE_CLAMP_PORTABILITY`). `.sh` 러너가 엔진 인자를, `.env` 가 컨테이너 이름·포트·
마운트 형상을 갖는다.

**`build_recipe` — 적용.** NGC `26.01-py3` 위에 vLLM 0.18.0 wheel 을 얹는 **순서**가 여기 있다.
① `pip install --no-deps` 로 얹어 NGC 의 torch 를 보존한다 — `--no-deps` 를 빼면 wheel 이 자기
torch 를 끌어와 NGC 빌드를 밀어낸다. ② **분산 런타임 스탠자**(`iproute2`·`netcat-openbsd`·`ray`)가
들어 있다. 단일노드에는 불필요해 보이지만 없으면 같은 이미지로 TP=2 를 시도할 때
`ray: command not found` 로 죽는다.

**`compose` — 적용.** 단일노드라도 서빙 성립 조건을 담는다. `oom_score_adj: 800`(통합메모리 압박 시
커널이 데스크톱이 아니라 vLLM 을 먼저 잡게), `memlock: -1`, 그리고 **JIT 캐시 영속 마운트**다.
캐시 마운트가 없으면 매 기동이 cold JIT 이 되어 시간뿐 아니라 **호스트 압박 리스크를 매번 새로 진다**.

**`runtime_patch` — 불해당.** 0.18.0 stock 이 processor·config 를 그대로 받는다. 서빙 로그에 shim
발화가 0회다 — *부재는 미판정이 아니라 "관측했는데 필요 없었다"* 다.

**`build_patch_pre` — 불해당.** torch 접두어 일치로 prebuilt wheel 트랙이 성립한다. 컴파일이 없으니
컴파일-전 슬롯이 성립할 자리가 없다.

**`build_patch_post` — 불해당.** 빌드-바깥 native 의존이 없다. 시험한 커널 축이 전부 stock 으로
낙찰됐기 때문이다(§2).

**`fork_pin` — 불해당 = stock.** arch-wall 이 없었으므로 사다리(deps 패치 → 소스 게이트 → 자체 이식 →
포크 핀)에 진입할 이유가 없었다. `.env` 에 `VARIANT=` 줄이 **없는 것**이 그 표현이다 — 값을 지우는
게 아니라 줄이 없는 것이 기본값이다.
