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
- `output/multi/configs/gpt-oss-120b-gb10-0190-b1.yaml`
- `output/multi/configs/gpt-oss-120b-gb10-0190-b1.sh`
- `output/multi/envs/.env.gpt-oss-120b-gb10-0190-b1`

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

**`triplet` — 적용.** TP=2 분산 서빙의 설정이다. `config.yaml` 이 **타겟 GPU 를 선언**하고
(H100 · 80 GiB/카드 · `target_gmu` 0.90 · cards_per_node 1) 그 예산에서 파생한 **노드당 KV 클램프
24,644 MiB** 를 절대값으로 고정한다. 측정은 GB10×2 통합메모리에서 났고 **클램프만 타겟 예산**이다
(`policy:KV_ABSOLUTE_CLAMP_PORTABILITY`). `.env` 는 master/slave 컨테이너 이름과 Ray 포트 형상을 갖는다.

**`build_recipe` — 적용.** 분산에서는 이 슬롯의 성격이 단일노드와 다르다 — **두 노드가 각자 같은
이미지를 재현해야** 집단 연산 ABI 가 맞는다. 이미지를 전송하지 않기 때문이다. 그래서
`ray`·`iproute2`·`netcat-openbsd` 스탠자가 **TP=2 의 전제**다. 이 스탠자는 원래 wheel 트랙 Dockerfile
에 없어 손으로 얹혀 있었고, 재렌더 한 번이면 조용히 사라져 slave 가 `ray: command not found` 로
죽는 구조였다 — 이번에 템플릿에 편입했다.

**`compose` — 적용.** master/slave 두 서비스와 Ray head↔worker 배선이 여기 있다. 단일노드 compose
로는 재현되지 않는다.

**`build_patch_post` — 불해당(단, 파일은 있다).** `output/multi/build_patches/` 에 4건이 존재하지만
**활성 레시피가 그것을 참조하지 않는다.** 존재를 곧 적용으로 읽으면 *먹지 않은 패치를 재현지침으로
배포*하게 되므로 불해당이다. 이 구분이 이 슬롯 판정의 핵심이다.

**`runtime_patch` — 불해당.** 0.19.0 stock 이 gpt-oss-120b 의 processor·config 를 그대로 받는다.

**`build_patch_pre` — 불해당.** prebuilt wheel 트랙이라 컴파일 자체가 없다.

**`fork_pin` — 불해당 = stock.** arch-wall 이 없었다. `.env` 에 `VARIANT=` 줄이 **없는 것**이 그 표현이다.
