# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3-4b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-14T05:01:28Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101210-kvq-concurrency · qwen3-4b · main · campaign |
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
| 자격 근거 | `output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_raw_qwen3-4b.json` · `output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_warm_qwen3-4b.json` |
| 증거 등급(task_class) | hint_map_only |
| 측정 등급(bench_mode) | lite |
| 성능 판정(measurement.verdict) | `OBSERVATION-ONLY` — 출처 lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3-4B를 RTX PRO 6000 Blackwell ×1(단일노드)에 vLLM v0.31.0(소스빌드)로 서빙했다 — BF16 가중치 · 컨텍스트 40,960 · KV auto · lite warm 158.41 t/s · KV 76.06 GiB. 가장 비싼 벽은 FlashInfer/nvcc JIT 컴파일 발열에 의한 워치독 사살(W2 — MAX_JOBS=1 + JIT 캐시 영속화로 해소)과 열 워치독의 잘못된 전력 보정(M1). 오해하기 쉬운 값은 kv-cache-memory-bytes다 — 승계값이 아니라 이 셀에서 엔진 실측으로 도출한 arch-scaled 값(V4)이라 네 환경에서는 반드시 다시 재야 한다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3-4b | 인증서(benchmark_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| HF repo | 미관측 | 체크포인트 .git/config 부재(/app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B) — HF repo 미관측 |
| HF revision(체크포인트 git HEAD) | 미관측 | 체크포인트 .git/HEAD 부재(/app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B) — revision 미관측 |
| base_model | Qwen/Qwen3-4B-Base | 체크포인트 README base_model(/app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B) |
| base 슬러그 | qwen3-4b | model(qwen3-4b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization = none(관측)(인증서(benchmark_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml).quantization = none · quantization_source = measured(engine log)) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다 · q 축 = /app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| GPU | NVIDIA RTX PRO 6000 Blackwell Workstation Edition | 인증서(benchmark_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| 토폴로지 · TP | single · TP=1 | 인증서(benchmark_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | 인증서(benchmark_26101413_qwen3-4b_RTXPRO6000BlackwellWorkstationEdition_0.31.0.yaml) · 생산자 `unverified`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3-4b | 서빙 yaml model(output/single/configs/qwen3-4b.yaml) · model_path basename(Qwen3-4B) 소문자 = 인증서 model 키 규칙 |
| 세그먼트 `arch` | rtxpro6000ws-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len40960-kvauto | derived(q·len·kv) |
| 축 `hw` | rtxpro6000ws | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[rtxpro6000ws]←'NVIDIA RTX PRO 6000 Blackwell Workstation Edition' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA RTX PRO 6000 Blackwell Workstation Edition' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 40960 | 서빙 yaml max-model-len(output/single/configs/qwen3-4b.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/qwen3-4b.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) 이 결과가 성립하는 조건: ① **discrete GPU + 작은 호스트 RAM(62GB)** — 이 노드는 VRAM 95.6GiB와 호스트 RAM이 분리돼 있어, 통합메모리 가정의 RAM 게이트(가중치 전량 상주)는 정상 기동을 막는다. 호스트 상주 상한은 최대 safetensors 샤드이고 KV는 VRAM에 있다(testlog_26101319 §경과 타임라인 — RAM 게이트 결함 수정). ② **CPU/SoC 열 예산이 실질 동시성 천장** — SoC 100°C(warn)·103°C(hard) 워치독이 armed 상태이고, 벤치 부하에서 SoC가 101~106°C에 도달한다. GPU 전력은 문제가 아니다(600W 한계 이내 정상). 다른 HW(더 큰 쿨링 마진)에서는 tq 계열로 사망한 셀도 측정 가능하다(testlog_26101317 §열 사망 증거 · §백필).

(나) 형상 비교 — **fp8 KV**: KV 바이트가 같고 dtype만 다름 → 토큰 수용량이 정확히 2배(553,840 → 1,107,696)가 되고 동시성도 2배(69→138, 17→35) — 양자화 오버헤드가 열 예산 안에 들어온다 · 지위 **확정**(testlog_26101317 §판정 표). **tq4/tq3 KV**: 토큰 수용량 2.9~3.5배로 동시성 후보가 크게 오르지만(N=200, N=239) 그 연산 부하의 CPU/SoC 발열이 hard ceiling을 넘어 전 셀 워치독 사살 — 쿨러 보강 후에도 동일 지점에서 사망 · 지위 **확정**(testlog_26101317 §열 사망 증거 · §백필). **nvfp4 KV**: attention selector가 이 아키텍처(sm_120)의 백엔드를 거부해 기동 자체가 안 된다 · 지위 **확정**(testlog_26101317 §판정 표).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | vLLM v0.31.0 소스빌드가 use_existing_torch.py 를 찾지 못해 실패 | Dockerfile.source-build.template 을 버전적응형(if tools/ … elif 루트 … else exit 1)으로 수정 | arch-invariant | 이후 이미지 빌드 성공 · 전 dtype 기동 통과(devlog_26101413 §S4 커밋 대상) |
| 2 | W2 | 기동·컴파일 구간에 열 워치독이 서빙 컨테이너를 kill (GPU 저전력 · 순수 CPU 발열) | serve_env MAX_JOBS=1 · JIT 캐시 영속 마운트 · E-core 피닝 · 런당 쿨다운 ≤55°C | judgment | 3차 런 bf16/fp8 구간 thermal_trip 증가 0(testlog_26101317 §열 사망 증거 (tq4)) |
| 3 | W3 | GPU 전력 축이 정상 부하를 오작동으로 트립할 수 있는 잘못된 보정 | gpu_pwr_w=700 · soc_warn 100 · soc_hard 103으로 재보정(사용자 적용) · declared(override) provenance 각인 | arch-locked | 재보정 후 bf16/fp8 완주 · GPU 전력 트립 0(testlog_26101317 §열 사망 증거 (tq4)) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | JIT·벤치 구간에 워치독이 서빙 컨테이너를 kill | 반증 | testlog_26101317 §열 사망 증거 (tq4) — GPU 79W 저전력에서도 SoC 규칙으로 사살이 기록됨 |
| M2 | 열 파라미터 파일에 재보정값이 measured 로 기재 | 반증 | devlog_26101413 §S4 커밋 대상 · testlog_26101317 의 `declared(override)` 표기 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261012 | output/single/benchlog/lite_publish/qwen3-4b_20261014T045542Z/lite_engine_head_qwen3-4b.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 0.31.0 | 생산자 `unverified` · 미검증 — sweep_bench.sh@2661d614cf52 이미지 태그 규칙 결과 None ≠ 인증서 '0.31.0' · 배너 접두도 아니다 (더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L646)는 측정 기록에 남지 않아 배제할 수 없다 — 덮어쓰기였을 수 있다) |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build=None 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 미관측 | 미관측 |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-x86_64-source | 셀 env(output/single/envs/.env.qwen3-4b) IMAGE_TAG |
| 이미지 digest | sha256:b02c97cbacb1d1a3ed922dd8139ab728d7847f51de64694dbb6a9192b85a93ac | docker image inspect easy-vllm:0.31.0-cu133-x86_64-source .Id · .Created 2026-10-12T12:34:26Z(현 로컬 이미지 관측 · 태그는 가변 포인터) · Created < 측정 2026-10-14T04:55:42Z → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖) |
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
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Qwen/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3-4B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build**: plan이 wheel의 `_C` ABI·아키 지원 여부를 핵심 분기로 선언했고(plan_26101219 §4 단계), 해소 결과 source-build가 선택됐다 — NGC 26.07의 torch 2.13.0a0를 그대로 쓴다(devlog_26101413 §S4 커밋 대상). wheel 트랙으로 갈 수 있는 조건은 대상 vLLM의 torch 선언과 NGC 베이스의 torch 빌드버전이 접두어 일치할 때뿐이다(워크플로 불변식).
(2) **vLLM 좌표**: 재현에는 빌드 입력 `VLLM_REF=v0.31.0`(릴리스 태그)를 쓴다. 엔진 기동 배너의 `0.31.1.dev0+gdb9527a46.d20261012`는 그 태그 체크아웃의 자기보고이고, 인증서의 `vllm_version=0.31.0`은 측정 도구가 이미지 태그 규칙으로 만든 값이다(0.4 표 출처 칸) — 셋은 생산자가 다륯다.
(3) **arch-locked**: `TORCH_CUDA_ARCH=12.0`(RTX PRO 6000 = sm_120) — 네 GPU의 compute capability를 `nvidia-smi --query-gpu=compute_cap`으로 확인해 재도출한다. CPU arch amd64 · 드라이버 계열 580.x 는 사실 블록의 manifest 선언값이며, 네 환경 값으로 대체한다.
(4) 선언 vs 관측: torch 2.13.0a0+9186a08 · CUDA 13.3.1.008 · NGC 26.07은 docker image inspect의 관측값(사실 블록)이고 resolved.json은 묶이지 못했다(HINT_MISSING_RESOLVE) — 이미지 관측이 서빙 실물의 정본이다.
(5) 미관측 좌표: 드라이버는 `580.178.04`(manifest 선언 — 스캔 시점)로 기록돼 있다. 호스트 OS · 호스트 CUDA는 이 캠페인 문서에 없다(미기록). 이 둘은 호스트 좌표이며 위 이미지 좌표와 섞지 않는다.
(6) 핵심 의존 핀: FlashInfer(fp8 KV의 attention backend)·flash-attn(bf16)·Triton·torch 2.13.0a0 — 공유 이미지 기본값이며 이 모델 고유 요구는 없다(적용 집합: 빌드 패치 0건).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 5개 — `tuned` 1 · `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | 캠페인 선언 상수(config.yaml target_gmu) — 이 셀에서 값을 바꿔 잰 기록 없음 | V1 |
| `max-model-len` | 40960 | inherited | 모델 config.json max_position_embeddings 선언을 그대로 옮긴 측정 통제변인(RoPE/YaRN 확장 미적용 결정) — 조정 기록 없음 | V2 |
| `max-num-seqs` | 512 | inherited | 손레버(lockset batch_source=hand-lever) — 동시성 측정을 배치 상한이 막지 않게 넉넉히 둔 값 · 스윕 미실시 | V3 |
| `kv-cache-memory-bytes` | 81668803133 | tuned | 이 셀 계보에서 도출 — bf16 무클��프 부팅 엔진 실측 Available KV 76.06 GiB로 고정(measured-clamp · arch-scaled) | V4 |
| `no-enable-prefix-caching` | true | inherited | 측정 통제변인(인터뷰 확정) — dtype 간 비교에서 prefix cache 효과를 배제하기 위한 선언 · 서빙 필요조건은 아님 | V5 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 잴 값(arch-scaled)**: `kv-cache-memory-bytes` — 무클��프 부팅 후 엔진 로그의 `Available KV cache memory`를 읽어 그 값으로 고정한다(V4). `gpu-memory-utilization`은 VRAM 총량이 달라지면 함께 본다. `max-num-seqs`는 측정 목적(동시성 상한을 배치가 막지 않게)에 맞춰 넉넉히 둔다.
② **필요조건으로 확인된 값**: 없음 — 이 셀에서 빼거나 바꿔 실패가 관측된 노브는 없다(W 시리즈의 실패는 노브가 아니라 환경·하네스였다). 단, 측정 재현을 위한 통제변인으로는 `no-enable-prefix-caching`(V5)과 `max-num-seqs 512`(V3)가 선언돼 있다 — 빼면 N*가 달라진다(측정 정의가 바뀌는 것이지 서빙이 깨지는 것은 아니다).
③ **승계·엔진 기본값**: `max-model-len 40960`은 모델 config 선언을 그대로 옮긴 것(V2 · 손으로 튠 값이 아니다), `gpu-memory-utilization 0.9`는 캠페인 선언 상수(V1 · 스윕 미실시)다. MAX_JOBS=1(셀 .sh의 serve_env)은 FlashInfer JIT 발열 완화용 손레버 — 열 마진이 큰 호스트에서는 불필요할 수 있다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전 대조** — 이 태그는 v0.31.0 릴리스 기준이다. 상위 버전에서는 `tools/use_existing_torch.py` 위치와 확장 모듈 이름(`_C_stable_libtorch` 등)이 또 바뀌었을 수 있다 → Dockerfile.source-build의 버전적응 스탠자가 감지하는 경로를 대조 → 같으면 그대로, 다륯면 해당 버전에 맞게 수정(W1).
2. **기동 성공 판정** — `single_serve_up.sh <config>` 후 health 200 + 추론 1회(01 §1.4). 이 셀은 자체 패치가 0건이라 발화 서명을 확인할 패치가 없다.
3. **KV 클램프 재도출** — 무클��프 부팅의 엔진 로그 `Available KV cache memory` 행을 읽어 `.yaml`의 `kv-cache-memory-bytes`를 교체(V4) → 그대로 / 달라지면 재도출.
4. **호스트 RAM 안전** — discrete GPU 호스트면 RAM 게이트가 최대 샤드 + floor로 판정하는지 확인(`preload_ram_gate`). 통합메모리 호스트에서는 이 셀의 discrete 경로가 적용되지 않는다 → 예산 산식이 달라진다(가중치 전량 상주).
5. **열 예산 확인** — SoC/CPU 온도 워치독이 있으면 경고·hard 임계를 확인하고, 고동시성 측정 전에 쿨링 마진을 본다(이 셀의 tq 계열 사망이 그 신호다) → 마진이 작으면 tq 계열 측정을 포기하거나 부하를 낮춘다.
실패 신호별 행선지 — 빌드·버전 핀 문제 → `upstream-version-watch` · 서빙 설정·KV·노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드·토폴로지 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3-4b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101210-kvq-concurrency · qwen3-4b · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101210_kvq_concurrency__main__qwen3_4b |
| work-manifest | docs/_evidence/camp_26101210_kvq_concurrency__main__qwen3_4b.work-manifest.json |
| 증거 등급(task_class) | hint_map_only |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-14T04:52:35Z |
| 승인 발화(전사) | 사용자: 'hint 태그 발행 진행해줘' (캠페인 측정 종결 후 3셀 발행 승인) |
| 현행 full 정의 충족(측정 등급) | 아니오 · 이 측정의 반복 1 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 검증 Agent(fact-checker)`(저작자 `발행 Agent(본 세션 저작자)` 아님) · 주장 56건 · 지적 8건 중 고침 8건 · 열림 0 · 2026-10-14T06:30:00Z |
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
**확정 이름** `hint/0.31.0/qwen3-4b/rtxpro6000ws-1g1n-main-native/qbf16-len40960-kvauto` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리 없음(`[]`) — 이 셀을 같은 `qbf16-len40960-kvauto`의 다른 셀과 가르는 노브가 없다. 후보의 ple·spec·graph는 이 모델·설정에서 전부 기본값(PLE 키 부재 · speculative 미설정 · enforce-eager 미설정)이라 구분 축이 아니다. max-num-seqs 512·prefix cache OFF는 측정 통제변인일 뿐 서빙 형상을 가르는 노브가 아니라서 꼬리로 쓰지 않는다.
