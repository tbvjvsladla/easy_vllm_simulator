# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 1-signal(file-presence) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 없음 | 1-signal(absence) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 없음 | 1-signal(absence) |
| `fork_pin` | build | upstream-version-watch | 없음 | 1-signal(absence) |

## 파일

**triplet**
- `output/single/configs/gpt-oss-120b-gb10.yaml`
- `output/single/configs/gpt-oss-120b-gb10.sh`
- `output/single/envs/.env.gpt-oss-120b-gb10`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

### `triplet` — 필수, 면제 없음

서빙에 원리적으로 필요하다. 이 조합에서 트리플렛이 실제로 담은 결정은 셋이다.

- **`max_model_len 131072` · `gpu_memory_utilization 0.90`** — 사용자 선택이지만 상한은 안전체계가
  정했다. 통합메모리에서 `gmu 0.95` 는 선언 바닥의 `arm_ceiling` 이 최소치 미만이 되어 **호스트
  예산 선언 자체가 거부**된다(근거: devlog §결정과 근거). 즉 0.90 은 취향이 아니라 **선언 가능한
  최대치**였다. 통합메모리 노드에서 gmu 를 올릴 때 이 벽을 먼저 만난다.
- **KV `fp8` + `kv_cache_memory_bytes` 절대값** — 비율이 아니라 절대 바이트로 잡는다
  (`policy:KV_ABSOLUTE_CLAMP_PORTABILITY`). fp16→fp8 전환으로 용량이 **436,896 → 873,808 토큰**
  으로 2배가 됐는데 **속도는 +0.8% 뿐**이었다(근거: testlog §측정값). 디코드가 KV 가 아니라
  **가중치 대역폭**에 묶여 있다는 실측이며, 이 모델에서 KV dtype 을 성능 레버로 기대하지 마라.
- **`.env` 는 이미지 태그를 명시한다** — 비우면 compose 가 낡은 기본값으로 조용히 폴백해
  **다른 vLLM 버전을 측정**한다. 벤치가 거짓말하는 가장 값싼 경로다.

### `runtime_patch` — 불해당

vLLM 0.19.1 의 stock 코드경로가 이 체크포인트를 그대로 서빙했다. processor/config shim 이 필요한
지점이 없었다. **부재는 미판정이 아니라 이 경우엔 확인된 불해당**이다 — Phase-2 수렴이 trial 1/3 에서
`correction_history: []` 로 끝났고 분류기도 `none` 을 냈다(근거: testlog §단계표).

### `build_patch_pre` / `build_patch_post` — 불해당 (트랙에서 파생)

**prebuilt wheel 트랙이므로 컴파일이 없다.** 빌드 패치는 정의상 소스 컴파일 전/후에 끼어드는
자리인데 그 자리가 존재하지 않는다. `TORCH_CUDA_ARCH_LIST` 도 불요다. 소스빌드 트랙으로 옮기면
이 두 슬롯의 판정이 통째로 달라지므로, **트랙이 다르면 이 항목을 그대로 가져가지 마라.**

### `fork_pin` — 불해당 (stock)

`.env` 에 `VARIANT=` 줄이 없다 = stock 이다. 0.19.1 stock 이 gpt-oss-120b MXFP4 를 sm_121 에서
그대로 서빙했고, 아치월도 포크 의존도 없었다. **줄의 부재가 곧 기본값**이라는 규약 덕에 "포크를
안 썼다"가 파일에 값을 적지 않는 것으로 표현된다.

### 이 목록에 없는 것 — 에어갭 자산 1건

슬롯 3+1+1 어디에도 안 들어가지만 **없으면 서빙이 죽는 자산**이 하나 있다: harmony/o200k 인코딩
파일이다. 이미지에 번들되지 않으므로 호스트에서 마운트해야 한다. 3+1+1 은 *코드·설정*의 분류이지
*자산*의 분류가 아니다 — 자세한 증상은 항목2 를 보라.
