# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-kv33g` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-10T08:57:46Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101017-knob-e2e-single · granite42-30b-v030 · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_granite42-30b-v030/level_01/post_health_granite42-30b-v030.json` · `output/single/benchlog/sweep_granite42-30b-v030/level_01/guidellm_granite42-30b-v030.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

IBM Granite-4.2-30B(bf16 dense)를 GB10 1노드(메인 · TP=1)에서 vLLM v0.30.0 source-build 이미지로 서빙했고, 컨텍스트 131072 · KV auto · spec off · CUDA graph 형상의 full-bench(GuideLLM · 반복 3) 판정점은 동시성 1 decode 3.93 t/s 다(weak PASS · floor 1.39).
가장 비싼 벽은 KV 절대클램프 산정이다 — 16GiB(W1)와 16 토큰 모자란 32GiB(W2)가 기동 거부됐고 33.0 GiB 에서 섰으며, 측정에서는 통합메모리 예산 게이트가 큰 벤치 몫을 막아 더 작은 프리셋 선택지로 다시 골랐다(W3).
가장 오해하기 쉬운 값은 gpu-memory-utilization 0.80(V2) — 조정된 적 없는 승계값이고 KV 클램프가 있으면 엔진이 KV 산정에 쓰지 않는다; floor 1.39 도 외부 실측이 아닌 루프라인 바닥이다(Q2).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | granite-4.2-30b | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| HF repo | ibm-granite/granite-4.2-30b | 체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) |
| HF revision(체크포인트 git HEAD) | 70fc514076785017d93087f3d8b0676426b6b355 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T07:14:23Z 이전 마지막 이동 2026-08-26T00:27:52Z) |
| base_model | ibm-granite/granite-4.1-30b-base | 체크포인트 README base_model(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) |
| base 슬러그 | granite-4.2-30b | model(granite-4.2-30b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization = none(관측)(sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030).quantization = none · quantization_source = measured(engine log)) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다 · q 축 = /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.30.0 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.30.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.30.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | granite-4.2-30b | 서빙 yaml model(output/single/configs/granite42-30b-v030.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · hf_repo(ibm-granite/granite-4.2-30b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len131072-kvauto-kv33g | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 131072 | 서빙 yaml max-model-len(output/single/configs/granite42-30b-v030.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype(output/single/configs/granite42-30b-v030.yaml) · vocab kv[auto]←'auto' |
| 꼬리 `kv33g` | KV 절대클램프 kv-cache-memory-bytes 35433480192(33.0 GiB) — 131072 1시퀀스 + 블록 올림 여유 | `output/single/configs/granite42-30b-v030.yaml` · 키 `kv-cache-memory-bytes` = `35433480192` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만든 것**

1. **통합메모리 한 덩어리(기동 스크립트가 읽은 총량 124608 MiB)** → 가중치 · KV · 서빙 overhead · 벤치 클라이언트가 같은 호스트 메모리를 나눠, KV 클램프 33.0 GiB 를 둔 이 서빙의 선언 바닥(16543 MiB)이 벤치 컨테이너 몫의 상한(6303 MiB)까지 정했다 → GPU 메모리와 호스트 RAM 이 분리된 HW 에서는 KV 가 호스트 예산에서 빠지므로 이 산술과 상한이 달라진다(추론). 근거 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_sweep_try1_budget_block.log §L9-13 · a_serve2.log §L8-13(측정을 서빙한 2차 기동).
2. **dense bf16 54.53 GiB 를 토큰마다 읽는 디코드** → 동시성 1~8 에서 스트림당 decode 가 3.83~3.96 t/s 로 평탄하고 출력 처리량만 늘었다(3.93 → 20.68 tok/s) — 계보는 이를 대역폭 한계로 기재했다 → 메모리 대역폭이 다른 HW 에서는 판정점 수치가 대역폭에 비례해 달라진다고 보는 것이 자연스럽지만 이 셀에서 검증한 것은 아니다(추론 · 상한 추정치끼리도 어긋난다 — 02 Q1). 근거 testlog_26101016_벤치_knob_프리셋_E2E_6셀_판정 §4 · bench_report_26101016_granite-4.2-30b_GB10_0.30.0 §부하 스윕 곡선.
3. **가중치가 CIFS 위 · 체크포인트가 가용 RAM 의 90% 초과** → 엔진은 두 조건(CIFS 가 인식되는 네트워크 FS(NFS/Lustre)가 아니고 · 54.53 GiB 가 가용 RAM 58.74 GiB 의 90% 를 넘는다)으로 auto-prefetch 를 껐고, 가중치 적재는 370.04 s 였다 → 파일시스템이나 가용 RAM 이 다른 HW 에서는 prefetch 여부가 달라진다. 370 s 가 prefetch 꺼짐 때문이라는 인과는 대조(prefetch on 적재)가 없는 추론이다. 근거 output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log §L57-58 · §L73.

**(나) 이 형상이 버티는 기전 대 다른 형상이 실패한 이유**

- `KV 절대클램프 16GiB` — 이 셀과 다른 것: kv-cache-memory-bytes 하나 — 131072 토큰 1시퀀스 KV 를 담지 못해 엔진의 기동 시 수용 검사가 거부(메모리 고갈이 아니라 사전 거부) — 확정 — granite42-30b-v030.yaml §L5-6 · 02 W1.
- `KV 절대클램프 32GiB` — 다른 것: kv-cache-memory-bytes 하나 — 엔진이 131056 토큰만 수용 가능하다고 계산해 같은 검사에서 16 토큰 미달 — 거부는 확정 · 16 토큰이 빠진 기전(yaml 주석의 블록 정렬 해석)은 미결 — granite42-30b-v030.yaml §L5-6 · 02 W2.
- `KV 33.0 GiB(이 셀)` — 135,168 토큰을 잡아 131072 기준 1.03x 로 검사 통과 · 엔진은 이 값을 받으면 gpu-memory-utilization 을 따르지 않는다 — 확정 — lite_engine 로그 §L81-82.
- `벤치 예산 8192 MiB` — 다른 것: 벤치 컨테이너 메모리 상한 하나(서빙 설정 동일) — 서빙 바닥에서 벤치 몫을 빼면 워치독 절대밴드 아래로 내려간다는 게이트 산술로 측정 전 차단 — 차단 판정은 확정 · "벤치가 실제로 서빙을 죽인다" 는 기전은 이 셀에서 관측되지 않아 미결(게이트가 인용한 실측은 다른 캠페인) — a_sweep_try1_budget_block.log §L9-13 · 02 W3.
- `overhead 16384(2026-10-01) 대 18432(이 측정)` — 다른 것: overhead 선언과 그에 따른 바닥(18591 대 16543) — 두 기동 모두 원장에 사살 · 트립이 없어 어느 쪽이 필요한지 갈리지 않았다 — 미결 — 01 §1.4 원장 9행 · 18행.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | KV 절대클램프 16GiB 로 기동하자 vLLM 이 기동을 거부했다 | kv-cache-memory-bytes 를 키움 — 최종 35433480192(33.0 GiB · W2 참조) | arch-invariant | 이 측정 엔진 로그 L82 GPU KV cache size 135168 tokens · 최대 동시성 1.03x |
| 2 | W2 | KV 절대클램프 32GiB 로 다시 기동하자 엔진이 131056 토큰만 수용 가능하다고 계산해 다시 거부했다 | 여유를 더해 kv-cache-memory-bytes 35433480192(33.0 GiB) — 그 값에서 기동 성공(관측) | arch-invariant | 이 측정 엔진 로그 L81 reserved 33.0 GiB memory for KV Cache · L82 135168 tokens 1.03x |
| 3 | W3 | 1차 스윕 레벨 1 진입 전 run_bench 예산 사전검사가 bench=BLOCK 으로 벤치를 시작하지 않았다(sweep_rc 3) | bench_budget_mib 를 프리셋 선택지 안 4096 으로 다시 골라 완주 | arch-scaled | testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §2 sweep_index.bench_knobs bench_budget_mib 4096 · bench_mode full · 판정점 반복 3/3 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 측정 서빙(2차 기동)의 READY 소요를 판정 문서 초판이 1차 기동과 같은 510 s 로 적었다 | 반증 | testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §1 정정문 · a_serve2.log L21 · lite 엔진 로그 L104 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.30.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | ced6857afa0ea7b2e3f0846a62e1394e90f15607 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.30.1.dev0+gced6857af.d20261001 | output/single/benchlog/sweep_granite42-30b-v030/sweep_index.json:17 meta.vllm_build(sweep_bench 가 이 측정의 레벨 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡은 값 — 배너 전용 규칙은 아니다) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='0.30.1.dev0+gced6857af.d20261001' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 70fc514076785017d93087f3d8b0676426b6b355 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T07:14:23Z 이전 마지막 이동 2026-08-26T00:27:52Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.30.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.granite42-30b-v030) IMAGE_TAG |
| 이미지 digest | sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-30b-v030 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build.** 0.30.0 의 torch 핀은 `==2.13.0` 이고 NGC 26.07 베이스의 torch 는 `2.13.0a0+9186a08` 이며, 해소 스크립트가 "source-build(torch ≥2.11 ABI 벽) · wheel(cu129) 은 ABI 벽으로 불용" 으로 판정했다(plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3 빌드 트랙 행). 다른 트랙으로 갈 수 있는 조건도 그 판정 문장에서 나온다 — prebuilt wheel 이 네 베이스 torch 와 ABI 가 맞을 때다(트랙 판정은 `upstream-version-watch` 의 결정론 스크립트가 소유).

(2) **vLLM 버전 문자열과 생산자.** 빌드 입력 ref `v0.30.0` 은 docker history 의 build-arg `VLLM_REF` 관측이고, 40자 SHA `ced6857afa0ea7b2e3f0846a62e1394e90f15607` 은 resolved.json 의 해소값이다. 엔진 기동 배너 `0.30.1.dev0+gced6857af.d20261001` 은 빌드된 패키지가 스스로 보고한 문자열을 측정 도구가 엔진 로그에서 정규식으로 잡은 값이다(같은 SHA 접두 `ced6857af` 를 품는다). 인증서 `vllm_version` 은 인증서가 발행되지 않아 없다(weak PASS). wheel 메타는 미관측이다. **재현 좌표는 릴리스 태그 `v0.30.0` 또는 40자 SHA** 이고, `0.30.1.dev0…` 를 pip/wheel 버전으로 찾지 마라. 배너의 `.dev0` · `.d20261001` 꼬리가 무엇을 뜻하는지(빌드 시 작업트리 상태 등)는 계보가 말하지 않는다.

(3) **arch-locked 값과 재도출.**
- `TORCH_CUDA_ARCH=12.1a`(build-arg) — 네 GPU 의 compute capability 로 다시 정한다(`nvidia-smi --query-gpu=compute_cap` 등).
- CPU arch `arm64`(image inspect) — 네 호스트 아키텍처의 NGC 베이스를 고른다.
- NGC 베이스 `nvcr.io/nvidia/pytorch:26.07-py3`(image inspect = resolved.json 접두 일치) — 대상 vLLM 의 torch 핀으로 결정론 해소를 다시 돌린다(26.08/26.09 는 torch 2.14 — plan_26100107 §3 NGC 행).
- 드라이버 580.173.02 — 호스트 층이며 이미지가 아니다. 네 호스트의 드라이버가 이미지 CUDA 13.3.1 을 받는지 확인한다.

(4) **선언 대 관측.** NGC 베이스 · torch · CUDA 는 image inspect 관측과 resolved.json 선언이 일치한다. 드라이버는 manifest 선언을 측정 도구가 옮긴 값이고 노드별 관측은 없다(위 표 `드라이버(노드별 관측)` 미관측). 측정 환경 스냅샷의 `cuda_version 132` 는 측정 도구가 manifest 에서 옮긴 값이다(`sweep_bench.sh@2661d614cf52` L936 `"cuda_version": grep_yaml(mftext, "cuda_version")` · manifest 의 스캔값 = 호스트 드라이버 CUDA 13.2) — 측정 시점 관측이 아니라 드라이버 행과 같은 manifest 선언이며, 이미지 CUDA 13.3.1 과는 다른 층이다. 계보 plan 의 D4 는 "멀티가 빌드한 0.30.0 이미지를 그대로 쓴다" 였지만, 측정 이미지는 `output/single/Dockerfile.source-build` 와 docker history 가 17/17 일치하고 층 창이 2026-10-01T07:38Z~08:14Z 인 같은 태그의 빌드다(01 §1.1 · §1.4). 계보 밖 기록(docs/testlog/testlog_26100117_하네스결함교정_재빌드일관성_E2E_판정.md §1 C2 행)에 따르면 이것은 사람이 HITL ② 에서 고른 single 렌더 · 빈 패치 슬롯 · `--no-cache` 재빌드이고 D4 이미지(`cdb1a38b…`)는 그때 호스트에서 사라졌다 — 서빙 이미지의 실제는 관측 쪽(digest `sha256:123c331f…` · 이 빌드 원장)이다.

(5) **미관측 환경 좌표.** 호스트 OS 는 계보 어디에도 없다(미기록). 노드 메모리 총량은 기동 스크립트가 읽은 `mem_total=124608MiB`(측정을 서빙한 2차 기동 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/a_serve2.log §L8)이고 통합메모리다(같은 로그 L10). 인터커넥트는 단일 노드라 해당 없다. 노드별 드라이버 관측은 미기록이다.

(6) **핵심 의존 핀.** `flashinfer-python==0.6.18.post1`(requirements.txt — v0.30.0 wheel METADATA · 엔진은 top-p/top-k 샘플링과 autotune 에 FlashInfer 를 썼다) · `transformers>=5.10.4,<5.15.0`(같은 파일) · `ray==2.48.0`(Dockerfile build-arg `RAY_VERSION` — 공유 이미지의 기본값이며 이 단일 노드 셀은 Ray 를 쓰지 않는다) · 어텐션은 vLLM 내장 FlashAttention 2(엔진 자동 선택). 이 셀에서 모델 고유로 요구된 의존은 없다 — 위 핀은 전부 0.30.0 공유 이미지의 것이다. 추론 파서 플러그인은 의존 패키지가 아니라 체크포인트 폴더의 파일이다(01 §1.6).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 6개 — `tuned` 1 · `inherited` 4 · `engine-default` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `dtype` | bfloat16 | inherited | 체크포인트 torch_dtype(bfloat16)을 그대로 옮긴 값 · 이 셀에서 조정 기록 없음 | V1 |
| `gpu-memory-utilization` | 0.80 | inherited | 2026-10-01 셀 저작에서 정한 천장을 승계 · 조정 기록 없음 · kv-cache-memory-bytes 지정으로 엔진이 KV 산정에 이 값을 쓰지 않는다고 로그에 명시(L81) | V2 |
| `max-model-len` | 131072 | inherited | 모델 네이티브 컨텍스트(max_position 131072)를 그대로 씀 · 줄이거나 늘려 잰 기록 없음 | V3 |
| `kv-cache-memory-bytes` | 35433480192 | tuned | 2026-10-01 에 16GiB(거부 W1) → 32GiB(16 토큰 미달 거부 W2) → 33.0 GiB 로 바꿔 가며 기동 · 엔진 보고 135168 토큰 · 131072 기준 1.03x | V4 |
| `kv-cache-dtype` | auto | engine-default | auto = 엔진이 모델 dtype 을 따른다 · 측정 도구가 엔진 로그에서 auto 를 관측(kv_cache_dtype_source measured) | V5 |
| `max-num-seqs` | 8 | inherited | 값의 출처를 적은 계보 문서 없음 · 이 셀에서 조정 기록 없음 · 측정에서는 프리셋 레벨 16 을 거르는 상한으로만 작용 | V6 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 도출할 값(arch-scaled).** `kv-cache-memory-bytes`(V4) — 모델 config 의 층 수 · kv_heads · head_dim · KV dtype 에서 토큰당 KV 바이트를 잡고 × 목표 컨텍스트로 1시퀀스 하한을 정한 뒤 **블록 올림 여유를 더해** 선언하고, 엔진 로그의 `GPU KV cache size … Maximum concurrency` 가 1.0x 이상인지로 확인한다(W1 · W2). 통합메모리 노드면 그 값으로 호스트 예산 바닥(총량 − 가중치 − KV − overhead)과 벤치 몫 상한도 다시 계산한다(W3). `max-num-seqs`(V6)는 KV 동시성(1.03x)과 함께 네 동시성 목표에서 다시 정한다. `max-model-len`(V3)은 목표 컨텍스트이며 바꾸면 KV 하한이 따라 바뀐다.

② **필요조건으로 확인된 값(declared-requirement).** 이 셀에는 없다. 바꿔서 실패가 관측된 노브는 KV 클램프 하나이고 그것은 값을 바꿔 가며 잰 tuned(V4)로 적었다 — 하한을 밑돌면 기동 거부(W1 · W2)가 근거다. **명시하면 오히려 헛도는 노브**: kv-cache-memory-bytes 가 있는 한 `gpu-memory-utilization` 은 KV 산정에 쓰이지 않는다(엔진 로그 L81) — 둘을 함께 조정하는 레시피로 옮기지 마라. 추론 · 툴 파서 인자는 빼면 깨지는지 시험한 기록이 없다(01 §1.2).

③ **조정된 적 없는 승계값 · 엔진 기본값.** `dtype` bfloat16(V1 — 체크포인트 dtype 그대로) · `gpu-memory-utilization` 0.80(V2 — 2026-10-01 셀 저작 때 통합메모리 안전 여유로 정한 천장 · 이 셀에서 조정 0 · 위 ② 의 이유로 효과도 없다) · `max-model-len` 131072(V3 — 모델 네이티브) · `max-num-seqs` 8(V6 — 출처 미기록)은 최적이라는 뜻이 아니다 · `kv-cache-dtype` auto(V5)는 엔진이 모델 dtype 을 따르게 둔 것이다. 이 셀의 통제변인은 측정 형상 프리셋이었고 서빙 노브는 아무것도 스윕하지 않았다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **빌드 트랙과 베이스** → 대상 vLLM 의 torch 핀과 네 NGC 베이스 torch 를 `upstream-version-watch` 의 결정론 해소 스크립트로 다시 해소한다(이 셀은 0.30.0 · torch 2.13 · NGC 26.07 → source-build — 0.4 (1)) → 같은 결과면 `artifacts/build_recipe/` 를 출발점으로 · 트랙이 바뀌면 재도출 · 상위 vLLM 이면 빌드 레시피만 재도출하고 이 지도의 KV 산정 절차는 그대로 쓴다. 이 셀은 빌드 · 런타임 패치를 싣지 않았으므로(01 §1.1) "자체 패치가 상위 버전에서 여전히 필요한가" 는 물을 것이 없다 — 인라인 `strip-hoist` 는 트립와이어가 맞지 않으면 skip 한다(01 §1.2).
2. **모델 구조와 KV 하한** → 체크포인트 config 의 층 · kv_heads · head_dim · torch_dtype 과 목표 컨텍스트로 1시퀀스 KV 를 잡고 블록 여유를 더해 클램프를 선언 → 엔진 로그 `GPU KV cache size: … tokens, Maximum concurrency for <ctx> tokens per request: <x>x` 에서 x ≥ 1.0 이면 그대로 · 기동 거부면 클램프를 올려 재시도(W1 · W2) · 네 메모리에 하한이 안 들어가면 컨텍스트를 줄이는 재전략.
3. **호스트 메모리 안전 예산** → 통합메모리 노드면 `single_serve_up.sh` 처럼 바닥 = 총량 − 가중치(체크포인트 크기 ÷ TP) − KV 클램프 − overhead 를 선언하고 기동한다(입력 4개 · overhead 는 선언값 — 2.5). 분리 메모리 HW 면 이 산술은 재도출. 기동 로그의 `floor=… arm_ceiling=…` 와 원장 `budget_declare` 행으로 확인.
4. **기동 성공 판정** → health 200 + 추론 1회. 이 셀은 패치 발화 서명이 없으므로 엔진 로그 non-default args 에 클램프 · 파서 인자가 찍혔는지(이 측정 L42)와 `reserved … for KV Cache as specified by kv_cache_memory_bytes`(L81)로 설정 도달을 본다. 네트워크 FS 위 가중치면 적재가 수 분 걸리는 것은 정상이다(L57-58 · L73) → 통과면 측정으로 · 실패면 아래 행선지.
5. **측정 전 벤치 몫** → full-bench 를 GuideLLM 으로 돌릴 때 `--bench-budget-mib` 를 서빙 바닥 − 절대밴드 이하로 고른다(`run_bench.sh` 의 예산 사전검사가 상한을 출력한다 · W3) → 게이트 BLOCK 이면 게이트가 말한 상한 이하로 다시 고르거나 KV 클램프를 낮춘다.
6. **판정 권위** → 외부 레퍼런스(E)나 사용자 목표(c)가 없으면 이 셀처럼 루프라인 바닥 weak 판정이 된다(Q2) → 성능 완료를 주장하려면 E 또는 c 를 세우고 explicit 권한으로 다시 판정한다.

실패 신호별 행선지: 빌드 · 버전 핀 · 베이스 불일치 → `upstream-version-watch` · KV 클램프 거부 · 서빙 노브 · 파서 → `vllm-recipe-explorer` · 처리량 기대 이하 · 판정 권위 → `adversarial-benchmark` · 노드 메모리 · 토폴로지 · 마운트 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-kv33g` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101017-knob-e2e-single · granite42-30b-v030 · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101017_knob_e2e_single__main__granite42_30b_v030 |
| work-manifest | docs/_evidence/camp_26101017_knob_e2e_single__main__granite42_30b_v030.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-10T08:04:57Z |
| 승인 발화(전사) | 사용자(선언 확인 팝업 응답 전사) — "일단 4건 발행한 뒤 …" (plan_26101017 · 6셀 E2E hint 발행 지시) |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(검증 서브에이전트)`(저작자 `저작 Agent(hint 저작 서브에이전트)` 아님) · 주장 162건 · 지적 17건 중 고침 17건 · 열림 0 · 2026-10-10T09:33:02Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손**: _없음 — 결손 코드표를 전수 대조한 결과 0건이다(침묵은 '없음' 과 구분되지 않는다 — 없음도 적는다)._
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

- (사실) 이 셀은 캠페인 밖(`_bootstrap`)에서 측정됐고, 발행을 위해 측정 뒤(2026-10-10T08:04:57Z)에 캠페인을 선언했다. 그래서 원장 표의 캠페인 경계는 측정 서빙의 예산 선언 시각으로 잡혔고, 같은 날 04:55Z 의 이번 E2E 1차 기동이 "앞선 캠페인" 묶음에 들어갔다(02 §2.2 서두).
- (사실) 이 셀 최초 서빙의 판정 · 서사 문서(2026-10-01)는 이 draft 의 계보 간선에 잡히지 않아 발췌 · 출처로 쓰지 않았다. 그 회차의 KV 클램프 이력은 계보 안에서는 서빙 yaml 머리 주석이 유일한 기록이다.
- (의견) 판정은 weak PASS 이고 기준이 루프라인 바닥이라, 이 태그를 성능 기준선으로 쓰기보다 KV 클램프 산정과 통합메모리 예산 산술의 지도로 읽기를 권한다.


## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-kv33g` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `kv33g` | KV 절대클램프 kv-cache-memory-bytes 35433480192(33.0 GiB) — 131072 1시퀀스 + 블록 올림 여유 | `output/single/configs/granite42-30b-v030.yaml` · 키 `kv-cache-memory-bytes` = `35433480192` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리 토큰 `kv33g` 하나를 골랐다 — 이 셀의 형상을 실제로 정한 노브는 2026-10-01 에 값을 바꿔 가며 잡은 KV 절대클램프(`kv-cache-memory-bytes` 35433480192 = 33.0 GiB · V4)이고, 같은 모델 · 같은 q·len·kv 의 다른 셀이 생기면 가장 먼저 갈릴 값이 이것이다(카탈로그에 결정론부가 같은 태그는 아직 없다). 후보 `plenone` 은 모델에 PLE 가 없어 가르는 것이 없고, `specoff` 는 이 계보에 spec 을 켠 셀이 없어 구분 노브가 아니며, `graph` 는 엔진 기본값(enforce-eager 미설정)이라 쓰지 않았다. 추론 파서 플러그인도 구분 노브 후보였으나 러너 플래그가 꼬리 근거 스냅샷에 잡히지 않아(`inputs/tail_sources.json` 에 `--enable-auto-tool-choice` 만 있다) 근거 대조를 통과할 수 없으므로 넣지 않았다.
