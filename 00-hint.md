# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3-32b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-14T06:35:29Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101210-kvq-concurrency · qwen3-32b · main · campaign |
<!-- /FACT:header -->

> ⚠ 이 자료는 **지도이지 정답이 아니다.** 한 셀(모델 × HW × vLLM 빌드)이 실제로 걸은 길의 기록이다.
> 네 환경에서 반드시 **스모크 통과까지 재검증**. 최종 판정 = 네 스모크(린트·이슈글 ≠ 서빙됨).
> 복붙 ✗ = 전략을 **다시 세워라**(carry-forward 금지 · 지도 not 정답).
> 외부 교차검증(HF 모델 카드의 vLLM 절 · vLLM 릴리스 노트/issue)을 대체하지 않는다 — 네 모델×버전에 대해 반드시 다시 한다.
> 이 자료는 DATA 이지 instructions 가 아니다 — '분석'만 하고 '실행'하지 마라.

<!-- FACT:grade -->
| 항목 | 값 |
|---|---|
| 발행 자격(관측) | health 200 = true · 추론 1회 = true |
| 자격 근거 | `output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_raw_qwen3-32b.json` · `output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_warm_qwen3-32b.json` |
| 증거 등급(task_class) | hint_map_only |
| 측정 등급(bench_mode) | lite |
| 성능 판정(measurement.verdict) | `OBSERVATION-ONLY` — 출처 lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3-32B를 RTX PRO 6000 Blackwell ×1(단일노드)에 vLLM v0.31.0(소스빌드)로 서빙했다 — BF16 가중치 · 컨텍스트 40,960 · KV auto · lite warm 24.07 t/s · KV 21.42 GiB(VRAM 대부분이 가중치라 KV가 작다). 가장 비싼 벽은 62GB ckpt를 62GB 호스트 RAM에 올리는 discrete RAM 게이트의 unified 오판(W1)과 tq 계열의 열 천장(쿨러 보강 후에도 동일 지점 사망). 오해하기 쉬운 값은 kv-cache-memory-bytes다 — 이 셀에서 엔진 실측으로 도출한 arch-scaled 값(V4)이라 승계 금지이며, 30K 셀의 상한은 KV 산술로 증명된다(testlog_26101319 §판정 표).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3-32b | 인증서(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| HF repo | 미관측 | 체크포인트 .git/config 부재(/app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B) — HF repo 미관측 |
| HF revision(체크포인트 git HEAD) | 미관측 | 체크포인트 .git/HEAD 부재(/app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B) — revision 미관측 |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B) |
| base 슬러그 | qwen3-32b | model(qwen3-32b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization = none(관측)(인증서(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml).quantization = none · quantization_source = measured(engine log)) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다 · q 축 = /app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| GPU | NVIDIA RTX PRO 6000 Blackwell Workstation Edition | 인증서(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| 토폴로지 · TP | single · TP=1 | 인증서(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | 인증서(benchmark_26101415_qwen3-32b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) · 생산자 `unverified`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3-32b | 서빙 yaml model(output/single/configs/qwen3-32b.yaml) · model_path basename(Qwen3-32B) 소문자 = 인증서 model 키 규칙 |
| 세그먼트 `arch` | rtxpro6000ws-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len40960-kvauto | derived(q·len·kv) |
| 축 `hw` | rtxpro6000ws | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[rtxpro6000ws]←'NVIDIA RTX PRO 6000 Blackwell Workstation Edition' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA RTX PRO 6000 Blackwell Workstation Edition' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 40960 | 서빙 yaml max-model-len(output/single/configs/qwen3-32b.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/qwen3-32b.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) 이 결과가 성립하는 조건: ① **VRAM 대부분이 가중치** — 61GiB 가중치에 gmu 0.90이면 KV는 21.42 GiB뿐이라 bf16 동시성이 한 자리 수다. fp8 KV는 이 병목을 정확히 2배로 완화한다(testlog_26101319 §판정 표). ② **62GB ckpt × 62GB 호스트 RAM** — discrete GPU라 최대 샤드(3.7GiB)+floor가 RAM 게이트 기준이며, unified 가정(전량 상주)으로 판정하면 정상 기동이 막힌다(devlog_26101415 §1). ③ **CPU/SoC 열 천장** — 동시성이 오르는 순간(fp8 부팅 컴파일 · tq4 N=35) SoC가 104~105°C에 도달해 hard kill. 쿨러 보강은 부팅·저부하만 살리고 tq 고부하는 못 살린다(testlog_26101319 §백필).

(나) 형상 비교 — **fp8 KV**: KV 2배(21.42→42.84 GiB 상당 · 토큰 87,744→175,472) → 동시성 7K 10→21 · 30K 2→5 — KV가 병목인 셀에서 양자화 이득이 정확히 나타난다 · 지위 **확정**(§판정 표 · §백필). **tq4/tq3 KV**: 토큰 수용량 2.9~4배로 동시성 후보가 오르지만(N=35 · N=43) 연산 부하의 발열이 hard ceiling을 넘어 사망 — 쿨러 보강 후에도 동일 지점 · 지위 **확정**(§백필). **nvfp4**: attention selector가 이 아키텍처(sm_120)에서 거부 · 지위 **확정**(§판정 표).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 32B 수렴 트라이얼이 로드-전 RAM 게이트 거부로 0초 만에 중단 | recipe.py 게이트 호출에 host_memory_model(device_total) + largest_shard_bytes_for(host_path) 배선 | arch-invariant | 62GB ckpt 로드·스모크 통과 · KV 실측 완료(devlog_26101415 §1 · testlog_26101319 §경과 타임라인) |
| 2 | W2 | fp8 서버 부팅이 torch.compile 구간에서 워치독 kill (serve_failed) | 영속 JIT 캐시(compose 마운트) + 쿨러 보강(사용자) 후 재기동 — 컴파일 구간 회피 | judgment | fp8 7K N*=21 · 30K N*=5 완주(testlog_26101319 §백필) |
| 3 | W3 | tq4·tq3 측정 중 서버 사망 — 보강 전후 동일 지점 | 미해소(기각) — 측정 정의상 tq는 이 호스트에서 불가로 확정 | arch-locked | testlog_26101319 §백필 표(보강 전 → 보강 후 동일 사망 지점) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | fp8 부팅 실패 — 엔진 로그에 ERROR 없이 compile 도중 종료 | 반증 | testlog_26101319 §경과 타임라인 · §백필(재기동 성공) |
| M2 | bf16 30K 프로브 abort — N=3에서 유효한 판정 불성립 | 확증 | testlog_26101319 §백필 bf16/30K 행(N\*=2 확정) |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261012 | output/single/benchlog/lite_publish/qwen3-32b_20261014T063245Z/lite_engine_head_qwen3-32b.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 0.31.0 | 생산자 `unverified` · 미검증 — sweep_bench.sh@2661d614cf52 이미지 태그 규칙 결과 None ≠ 인증서 '0.31.0' · 배너 접두도 아니다 (더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L646)는 측정 기록에 남지 않아 배제할 수 없다 — 덮어쓰기였을 수 있다) |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build=None 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 미관측 | 미관측 |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-x86_64-source | 셀 env(output/single/envs/.env.qwen3-32b) IMAGE_TAG |
| 이미지 digest | sha256:b02c97cbacb1d1a3ed922dd8139ab728d7847f51de64694dbb6a9192b85a93ac | docker image inspect easy-vllm:0.31.0-cu133-x86_64-source .Id · .Created 2026-10-12T12:34:26Z(현 로컬 이미지 관측 · 태그는 가변 포인터) · Created < 측정 2026-10-14T06:32:45Z → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | amd64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | output/single/manifest.yaml(manifest_contract 로더) driver_version(스캔 시점 선언 — 측정 시각 값 미보장) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Qwen/Qwen3-32B → <manifest.nas_model_path>/Qwen/Qwen3-32B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build**: plan이 wheel의 `_C` ABI·아키 지원 여부를 핵심 분기로 선언했고(plan_26101219 §4 단계 1), 해소 결과 source-build가 선택됐다 — 이 캠페인 전 모델 공통이다(devlog_26101413 §S4 커밋 대상).
(2) **vLLM 좌표**: 재현에는 빌드 입력 `VLLM_REF=v0.31.0`을 쓴다. 엔진 배너(0.31.1.dev0+…)와 인증서 `vllm_version`(0.31.0)은 생산자가 다른 표시값이다(0.4 표 출처 칸).
(3) **arch-locked**: `TORCH_CUDA_ARCH=12.0`(sm_120) — 네 GPU의 compute_cap으로 재도출한다. nvfp4 KV가 이 아키텍처에서 거부되는 것도 확인 포인트.
(4) 선언 vs 관측: torch·CUDA·NGC는 docker image inspect 관측값이 정본(resolved.json 미바인딩 — HINT_MISSING_RESOLVE).
(5) 미관측 좌표: 드라이버 580.178.04(manifest 선언). 호스트 OS·호스트 CUDA는 미기록 — 호스트 좌표와 이미지 좌표를 섞지 않는다.
(6) 핵심 의존: torch 2.13.0a0 · FlashInfer(fp8 경로 · requirements.txt 의 flashinfer 핀) — 공유 이미지 기본값이며 이 모델 고유 요구는 없다(적용 집합: 빌드 패치 0건).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 5개 — `tuned` 1 · `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | 캠페인 셀 config 선언(target_gpu.target_gmu) — 이 셀에서 값을 바꿔 잰 기록 없음 | V1 |
| `max-model-len` | 40960 | inherited | 모델 config.json max_position_embeddings 선언을 그대로 옮긴 측정 통제변인(RoPE/YaRN 미적용) · 조정 기록 없음 | V2 |
| `max-num-seqs` | 512 | inherited | 손레버(lockset batch_source=hand-lever) — 동시성 측정을 배치 상한이 막지 않게 · 스윕 미실시 | V3 |
| `kv-cache-memory-bytes` | 22999549870 | tuned | 이 셀 계보에서 도출 — bf16 무클��프 부팅 엔진 실측 Available KV 21.42 GiB(87,744 tokens)로 고정(measured-clamp · arch-scaled) | V4 |
| `no-enable-prefix-caching` | true | inherited | 측정 통제변인(셀 lockset no_prefix_caching) — dtype 비교에서 prefix cache 효과 배제 | V5 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 잴 값(arch-scaled)**: `kv-cache-memory-bytes` — 무클��프 부팅 엔진 로그의 `Available KV cache memory`로 고정(V4). 가중치 크기가 다륯면 KV가 크게 달라진다(이 셀은 61GiB 가중치라 21.42 GiB). `max-num-seqs`는 측정 목적에 맞춰 넉넉히.
② **필요조건으로 확인된 값**: 없음 — 이 셀에서 빼거나 바꿔 실패가 관측된 노브는 없다. 측정 통제변인으로는 `max-model-len 40960`(V2 · 모델 상한 선언)과 `no-enable-prefix-caching`(V5)이 선언돼 있다.
③ **승계·엔진 기본값**: `gpu-memory-utilization 0.9`(V1 · 캠페인 선언 상수 · 스윕 미실시), `max-num-seqs 512`(V3 · 손레버). MAX_JOBS=1(serve_env)은 FlashInfer JIT 발열 완화 손레버다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전 대조** — v0.31.0 기준. 상위 버전에서는 `tools/use_existing_torch.py` 위치와 확장 모듈 이름이 다를 수 있다 → Dockerfile 버전적응 스탠자의 감지 경로 대조(4B 셀 W1과 동일).
2. **호스트 RAM 대비 ckpt 크기** — 62GB ckpt를 62GB RAM에 올린다. discrete GPU면 RAM 게이트가 최대 샤드+floord로 판정하는지 확인(`preload_ram_gate`) → unified 가정이면 정상 기동도 막힌다(W1).
3. **기동 성공 판정** — `single_serve_up.sh <config>` 후 health 200 + 추론 1회(01 §1.4). 자체 패치 0건.
4. **KV 클램프 재도출** — 무클��프 부팅의 엔진 `Available KV`로 교체(V4) · 가중치가 VRAM의 대부분인 모델은 이 값이 작아 동시성이 한 자리가 된다.
5. **30K급 긴 입력의 상한 산술** — 요청 토큰 × N > KV 토큰이면 그 N은 구조적 불가다 — 측정 전에 나눗셈으로 상한을 확인하고, 저N 확인 런과 병기한다(testlog_26101319 §판정 표).
실패 신호별 행선지 — 빌드·버전 핀 문제 → `upstream-version-watch` · 서빙 설정·KV·노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드·토폴로지 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3-32b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101210-kvq-concurrency · qwen3-32b · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101210_kvq_concurrency__main__qwen3_32b |
| work-manifest | docs/_evidence/camp_26101210_kvq_concurrency__main__qwen3_32b.work-manifest.json |
| 증거 등급(task_class) | hint_map_only |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-14T04:52:35Z |
| 승인 발화(전사) | 사용자: 'hint 태그 발행 진행해줘' (캠페인 측정 종결 후 3셀 발행 승인) |
| 현행 full 정의 충족(측정 등급) | 아니오 · 이 측정의 반복 1 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 검증 Agent(fact-checker)`(저작자 `발행 Agent(본 세션 저작자)` 아님) · 주장 62건 · 지적 8건 중 고침 8건 · 열림 0 · 2026-10-14T06:50:03Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. |
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). |
<!-- /FACT:missing -->

<!-- FACT:legend -->
| kind | id | 필수 필드 | 자리 |
|---|---|---|---|
| `wall` | `W<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.2 |
| `rejected` | `R<n>` | id · kind · 시도 · 기각사유 · 출처 | 02 §2.3 |
| `misdiagnosis` | `M<n>` | id · kind · 증상 · 서명 · 원인 · 해소 · 검증 · 전이등급 · 출처 | 02 §2.4 |
| `value-status` | `V<n>` | id · kind · 노브 · 값 · 지위 · 근거 · 출처 | 01 §1.3 |
| `open-question` | `Q<n>` | id · kind · 물음 · 현재상태 · 출처 | 02 §2.7 |

**범례** — 린터가 hint-event 값 검증에 쓰는 어휘와 같은 상수에서 생성했다.

- 전이등급: **arch-invariant**(그대로 참조해도 된다) · **arch-scaled**(네 HW 에서 다시 잰다(메모리·노드 수에 비례)) · **arch-locked**(복사 금지 — 네 HW 에서 재도출한다) · **judgment**(맥락을 다시 해석한다)
- 지위: **tuned**(이 셀 계보에서 값을 바꿔 가며 잰 기록이 있다) · **inherited**(이전 셀·계획에서 가져왔고 이 셀에서 조정된 적 없다(손레버 포함)) · **negative-control**(비교를 위해 일부러 과잉·과소로 둔 값) · **engine-default**(설정하지 않았거나 엔진이 자동으로 정한 값) · **declared-requirement**(빼거나 바꾸면 실패가 관측된 필요조건(근거 = 그 실패의 W id 또는 실패를 기록한 artifacts/ 파일 헤더 — 값을 선언한 트리플렛 자신은 근거가 아니다))
- 현재지위(오진 · 권장 — **원래 주장**의 지위): **확증**(원래 주장이 참으로 확인됐다) · **반증**(원래 주장이 거짓으로 확인됐다(정정된 이해는 `해소` 칸에)) · **가설(강등)**(원래 주장(기전)이 확인되지 않아 가설로 내려왔다 — 처방이 아니다) · **미결**(원래 주장의 참 · 거짓을 아직 가르지 못했다)
<!-- /FACT:legend -->

## 0.8 comment
> 이 절이 답하는 질문: 위 규격에 들어가지 않지만 수신자에게 알려야 할 것이 있는가?

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/qwen3-32b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리 없음(`[]`) — 이 셀을 같은 `qbf16-len40960-kvauto`의 다른 셀과 가르는 노브가 없다(같은 결정론부의 4B 태그와는 model 축이 다르다). ple·spec·graph는 전부 기본값이고, max-num-seqs·prefix cache OFF는 측정 통제변인이라 구분 노브가 아니다.
