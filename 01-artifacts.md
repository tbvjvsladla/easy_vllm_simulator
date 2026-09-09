# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 2-signal(file+declaration) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 없음(미적용 제외) | 2-signal(file+declaration) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 없음(미적용 제외) | 2-signal(file+declaration) |
| `build_recipe` | build | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/multi/configs/b-768k-kvfp8-l4combo.yaml`
- `output/multi/configs/b-768k-kvfp8-l4combo.sh`
- `output/multi/envs/.env.b-768k-kvfp8-l4combo`

**build_recipe**
- `output/multi/Dockerfile`
- `output/multi/Dockerfile.source-build`
- `output/multi/requirements.txt`

**compose**
- `output/multi/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

- **triplet** — 서빙의 전부: fp8 KV(fp8_ds_mla로 강제 해소되는 유일 경로) · dspark nspec7(+73.7% 레버) · cudagraph(+52.8% 레버) · moe humming(auto는 MARLIN-repack OOM 전력) · TP=2 ray(멀티 필수) · 10GiB 절대 KV 클램프(호스트 밸리 binding) · parser deepseek_v4(Hermes 용처).
- **runtime_patch** — 불해당: stock 0.29.0rc6이 DS4F를 네이티브 지원해 serve 시점 shim이 필요 없었다.
- **build_patch_pre/post** — 불해당: **stock 빌드**(build_patch_selectors 부재). 트리에 있던 0.27.x sm12x 이본들은 이 태그의 빌드에서 한 번도 적용되지 않았으므로 페이로드에서 제외했다(먹지 않은 패치를 배포하지 않는다).
- **build_recipe** — 빌드 재현의 핵심: NGC 26.07(torch 2.13.0a0)×vLLM 0.29.0rc6 커플링과 requirements constraint(0.28.0 baseline + flashinfer 0.6.18 양보)가 ResolutionImpossible 회피의 실체다.
- **compose** — master/slave Ray 오케스트레이션 재현 필수(NCCL/RoCE env 포함).
- **fork_pin** — 불해당: stock vllm-project/vllm @ v0.29.0rc6. 0.25.1 시대와 달리 포크 불요임을 스모크가 증명했다.
