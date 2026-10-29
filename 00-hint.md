# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610291845` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-29T09:45:40Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102918-handoff-single2 · lfm25-26b-v031 · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/level_01/post_health_lfm25-26b-v031.json` · `output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/level_01/guidellm_lfm25-26b-v031.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

LFM2.5-2.6B BF16를 단일 GB10에서 기존 source-build 이미지로 서빙하고 기본 부하의 Full 반복을 측정했다. 동시성 1 판정점 33.74 tok/s는 weak 기준 PASS다. 서빙 노브는 승계값이며 다른 환경에서는 메모리와 기능을 다시 검증해야 한다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | lfm2.5-2.6b | sweep_index.meta(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| HF repo | LiquidAI/LFM2.5-2.6B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) |
| HF revision(체크포인트 git HEAD) | ab00687315bc1298e9d54e9c4b611dde9867ccc2 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T09:43:49Z 이전 마지막 이동 2026-08-11T01:46:56Z) |
| base_model | LiquidAI/LFM2.5-2.6B-Base | 체크포인트 README base_model(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) |
| base 슬러그 | lfm2.5-2.6b | model(lfm2.5-2.6b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3)) · q 축 = /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · exact_tag=미관측 · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | lfm2.5-2.6b | 서빙 yaml model(output/single/configs/lfm25-26b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · hf_repo(LiquidAI/LFM2.5-2.6B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto-t2610291845 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/lfm25-26b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/lfm25-26b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| timestamp | `t2610291845` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 통합메모리 단일 노드이므로 호스트 MemAvailable과 엔진 가중치/KV가 같은 메모리 풀의 안전 예산에 들어간다. 이전 공존 서빙에서는 시작 free-memory 검사에서 GMU 0.80이 거부됐고 0.39로 다시 검증했다. 이 실패는 다른 GPU의 VRAM 한도나 분산 통신 실패로 전이할 수 없다(testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite §2. 트라이얼 이력).

체크포인트는 비양자화 BF16이고 conv/GQA 혼합 구조다. 이전 테스트에서 KV 10 GiB에서 37.9 GiB로 바뀐 것은 가중치 양자화 변경이 아니라 절대 KV 예산 변경이었다. 지금은 그 값을 승계해 새 기본 부하만 검증했다. dtype·GPU·메모리 구조가 바뀌면 가중치 크기와 KV/overhead를 각각 다시 관측해야 한다(동일 testlog §1. 트리플렛·§2. 트라이얼 이력).

- 이전 GMU 0.80·KV 10 GiB·다른 모델 공존 → GMU 0.39·KV 10 GiB: 바뀐 GMU가 시작 검사 문턱을 낮췄고 functional 통과를 관측했다. 지위 확정은 그때의 free-memory 거부에 한정한다(W1).
- 그다음 GMU 0.39·KV 37.9 GiB: 절대 캐시가 커져 점유 델타가 늘었다. 이전 KV-fit 기록은 필요량 근거이지 이 크기가 최소라는 증거가 아니다(V4).
- 이번 실행은 다른 모델 공존 없이 같은 서빙 노브를 채택하고 overhead 선언과 benchmark 입력/데이터셋을 새로 정했다. 과거 공존 예산을 이번 실측으로 옮기지 않았다. 개별 변화의 속도 인과는 대조 실험이 없어 미결이다(testlog_26102716_lfm25_default_full_신규판정 §Protocol and external comparison).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 과거 free-memory 시작 거부 | 동일 KV10GiB에서 lockset GMU0.39로 재시도해 functional 통과를 관측했다. | arch-scaled | testlog_26101412_lfm25-26b-v031_싱글서브_서빙_lite |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 처음에 선언192를 실제 완료 수로 전사한 진행 추정 | 반증 | testlog_26102716_lfm25_default_full_신규판정 §Actual results |
| M2 | 과거 lockset 각인과 배포 GMU 출처가 달랐음 | 반증 | devlog_26102716_lfm25_default_full_신규실측 §Grounded recipe retained |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | 미관측 | 빌드 입력/원장에 결속된 SHA 미관측 — runtime checkout 관측과 컴파일 바이트 증명은 별개 |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) | naming.vllm_build_input(kind=release) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_lfm25_handoff2_20261029_run3/lite_engine_head_lfm25-26b-v031.log:49 · 기동 2026-10-29T09:26:26Z(컨테이너 시계 UTC 가정) · docs/logs/sub/sessions/serve-handoff2-lfm25.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 미관측 | 미관측 — 결속된 명시적 배포 메타 기록 · attestation vllm_dist_version · native pip freeze · 빌드 원장 vllm.dist_version에 값 없음(runtime_version 기록은 배포 메타로 간주하지 않는다)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | ab00687315bc1298e9d54e9c4b611dde9867ccc2 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T09:43:49Z 이전 마지막 이동 2026-08-11T01:46:56Z) |
| 빌드 트랙 | source-build | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측)) |
| Dockerfile(선택자) | Dockerfile.source-build | image-history:docker history build-arg 관측(VLLM_REF=v0.31.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(이 파일만 단독 적용하고 override 가 없을 때 Dockerfile.source-build 선택 · 과거 빌드에 이 compose 또는 override 를 사용했는지는 미관측) |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.lfm25-26b-v031) IMAGE_TAG |
| 이미지 digest | sha256:667eee7bd3b4e84b204ad4887bd811c87dd3537428d026eaeb580ab825d88c8e | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 이 측정에 결속된 노드별 입력 미관측 | 이 측정에 결속된 노드별 구조화 관측 입력 없음(다른 시점 관측의 부재를 뜻하지 않음) |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 이 측정에 결속된 관측 입력 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_lfm25_handoff2_20261029_run3 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Liquid-AI/LFM2.5-2.6B → <manifest.nas_model_path>/Liquid-AI/LFM2.5-2.6B/config.json dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

빌드 입력 릴리스와 설치 런타임의 버전은 위 생산자별 사실 표에서 구별한다. 이 실행은 이전 source-build 이미지를 재사용했으며 재빌드하지 않았다. 빌드 원장이 없어 패치 적용과 컴파일 바이트는 미관측이다. 다른 하드웨어에서는 아키텍처, 메모리, 드라이버와 이미지 ABI를 다시 확인한다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | 이번에는 hand-authored lockset의 hand 값 승계. 과거 free-memory 실패 뒤 선택된 값이며 이번 GMU 탐색은 하지 않았다. | V1 |
| `max-model-len` | 32768 | inherited | 이전 로컬 recipe/이번 선언에서 승계한 목표 길이. 체크포인트 최대 길이나 이번 최대치 탐색 결과가 아니다. | V2 |
| `max-num-seqs` | 8 | inherited | 손레버 batch8 승계. 이번에는 capacity filter 및 c1/2/4/8 부하만 검증했다. | V3 |
| `kv-cache-memory-bytes` | 40694815129 | inherited | 손레버 절대 clamp 승계. 과거10GiB/37.9GiB 시험은 있으나 이번에 최소 KV를 수렴한 것은 아니다. | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① 이식 시 다시 산정할 값은 GMU·max-model-len·max-num-seqs·절대 KV clamp다. 노드 총량/시작 여유와 체크포인트 크기, 엔진 KV 토큰·로드 후 메모리, 대표 요청 길이를 관측한 뒤 안전 예산과 동시성을 다시 선택한다(V1~V4).

② 이번 캠페인에서 노브 제거 대조로 입증한 declared-requirement는 없다. qwen3 reasoning/lfm2 tool 파서는 실제 세 기능 smoke를 통과했으나 제거 시 실패하는지를 이번에 재지 않았으므로 필요조건으로 승격하지 않는다(testlog_26102716_lfm25_default_full_신규판정, devlog_26102716_lfm25_default_full_신규실측 §Observed serve and functional smoke).

③ 네 개의 표 노브는 이번 실행의 inherited 손레버다. 과거 GMU/KV 시험 근거는 계보에 남지만, 이번 c1/2/4/8 부하 스윕은 그 서빙 노브의 최적화 스윕이 아니다. c16은 용량 필터로 제외됐고 실패했다고 세지 않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 체크포인트와 대상 엔진 지원을 확인한다.
2. 노드 메모리, 가중치 및 KV 예산을 다시 산정하고 레시피를 준비한다.
3. health 200 및 completion, reasoning, tool 기능을 검증한다.
4. 동일 부하와 반복으로 성능을 재고 세션을 정규 정리한다. 실패는 서빙 레시피, 빌드 버전 또는 벤치마크 담당 경로로 각각 돌린다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610291845` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102918-handoff-single2 · lfm25-26b-v031 · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102918_handoff_single2__sub__lfm25_26b_v031 |
| work-manifest | docs/_evidence/camp_26102918_handoff_single2__sub__lfm25_26b_v031.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-29T09:21:56Z |
| 승인 발화(전사) | 사용자: 인계 문서를 읽고 싱글브랜치 캠페인 E2E 진행; 승인된 main/sub 각 1셀 Full 및 hint 발행 범위 재개 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/skills/adversarial-benchmark/SKILL.md:33`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `handoff2-lfm25-independent-checker`(저작자 `handoff2-lfm25-author` 아님) · 주장 56건 · 지적 4건 중 고침 4건 · 열림 0 · 2026-10-29T09:47:39Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 도입 시점과 부재 원인은 미관측이다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
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
**확정 이름** `hint/0.31.0/lfm2.5-2.6b/gb10-1g1n-sub-native/qbf16-len32768-kvauto-t2610291845` — 꼬리 0토큰 · timestamp `t2610291845`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610291845` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

tail.json은 빈 목록으로 명시한다. PLE 부재·spec off·기본 graph는 기존 동일 결정론부의 셀과 이번 실행을 가르는 새 노브가 아니며, 기본 부하의 신규 측정이라는 사실은 서사와 측정 바인딩에 기록한다. 이름 충돌의 timestamp는 continue가 파생하게 둔다.
