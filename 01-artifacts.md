# 1. 산출물 — 무엇이 실제로 쓰였나

> 슬롯 분류는 **경로 규약에서 파생**한 결정론 산출이다(헌법 3+1+1).
> **판정 강도가 슬롯마다 다르다** — `slot_confidence` 를 함께 읽어라.
> 판정 기준은 *"무엇을 고치나"가 아니라 "언제 성립해야 하나"* 다.

| 슬롯 | 성립 시점 | owner | 존재 | 판정 강도 |
|---|---|---|---|---|
| `triplet` | serve | vllm-recipe-explorer | 있음 | 2-signal(file+declaration) |
| `runtime_patch` | serve(arming) | vllm-recipe-explorer | 없음 | 2-signal(file+declaration) |
| `build_patch_pre` | 컴파일 전 | upstream-version-watch | 있음 | 3-signal(file+evidence+declaration) |
| `build_patch_post` | 컴파일 후 | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `build_recipe` | build | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `compose` | serve(orchestration) | upstream-version-watch | 있음 | 2-signal(file+declaration) |
| `fork_pin` | build | upstream-version-watch | 없음 | 3-signal(file+evidence+declaration) |

## 파일

**triplet**
- `output/multi/configs/nv4-bf-262k-mmp.yaml`
- `output/multi/configs/nv4-bf-262k-mmp.sh`
- `output/multi/envs/.env.nv4-bf-262k-mmp`

**build_patch_pre**
- `output/multi/build_patches_src/50-dsv4-sm12x-port.sh`
- `output/multi/build_patches_src/55-src-deps-authority.sh`
- `output/multi/build_patches_src/60-qwen4exp-nvfp4-mixed.sh`
- `output/multi/build_patches_src/62-qwen4exp-ple-mmap.sh`
- `output/multi/build_patches_src/64-qwen4exp-qsa-fp8kv.sh`

**build_patch_post**
- `output/multi/build_patches/10-deepgemm.sh`
- `output/multi/build_patches/20-triton-kernels.sh`
- `output/multi/build_patches/30-mxfp4-triton-sm121.sh`
- `output/multi/build_patches/40-humming-nvml-gb10.sh`

**build_recipe**
- `output/multi/Dockerfile`
- `output/multi/Dockerfile.source-build`
- `output/multi/requirements.txt`

**compose**
- `output/multi/docker-compose.yaml`

**fork_pin** — 없음 = **stock**. `.env` 에 `VARIANT=` 줄이 없는 것이 기본값이다.

## 적용 사유 (Agent)

**triplet — 적용.** 이 셀의 서빙 형상 전체가 여기 있다: `max-model-len 262144` · `tensor-parallel-size 2` ·
`gpu-memory-utilization 0.85` · `kv-cache-dtype auto(BF16)`. 특히 **262144 는 임의로 고른 값이 아니라
모델의 `max_position_embeddings` 상한 그 자체**이며, 이 캠페인에서 그보다 큰 값을 시도한 8셀이 전부
`ValidationError` 로 무너졌다(../testlog/testlog_26091009_qwen38fn_24셀_판정.md §void 셀 · NVFP4 512k/1m). 이 파일을 받는 사람은 그 벽을
다시 확인할 필요가 없다.

**runtime_patch — 불해당.** 이 모델은 serve 시점 Python shim 이 필요하지 않았다. 필요한 교정은 전부
**컴파일 전**에 성립해야 하는 것이었고, 그것을 런타임 슬롯으로 시도했다면 위상 오배정으로 조용히
무효화됐을 것이다(헌법 3+1+1).

**build_patch_pre — 적용.** 이 캠페인의 핵심이다. 다섯 중 셋(`60-qwen4exp-nvfp4-mixed` ·
`62-qwen4exp-ple-mmap` · `64-qwen4exp-qsa-fp8kv`)이 이 모델을 위해 **자체 이식**한 것이고, 포크 핀
없이 stock 소스 위에서 성립했다(../testlog/testlog_26091009_qwen38fn_24셀_판정.md §1 패치 검증 인용). `_C` 재컴파일이 필요한 csrc 변경을 담으므로
컴파일 후 슬롯으로는 원리적으로 대체할 수 없다.

**build_patch_post — 적용.** 컴파일 바깥 native 의존(DeepGEMM · Triton 커널 · sm_121 MXFP4 ·
GB10 NVML)을 설치한다. 소스가 아니라 **환경**을 고치는 것이라 pre 슬롯에 두면 자리가 틀린다.

**build_recipe — 적용.** 위 패치들이 실제로 먹은 이미지를 지은 레시피다. 이것 없이는 패치 목록만 있고
그것을 어떤 베이스·어떤 순서로 얹었는지가 없어 재현이 성립하지 않는다.

**compose — 적용.** TP=2 멀티 노드 기동이라 master/slave 두 컨테이너의 배선(Ray 랑데부·포트·마운트)이
serve 성립의 전제다. 단일 노드처럼 `vllm serve` 한 줄로 뜨지 않는다.

**fork_pin — 불해당.** `.env` 에 `VARIANT=` 줄이 없다 = **stock**. 이 모델은 arch-wall 을 만나 포크로
도망칠 필요가 없었고, 필요한 것은 전부 자체 이식으로 stock 위에서 성립했다. 부재가 곧 기본값이다.
