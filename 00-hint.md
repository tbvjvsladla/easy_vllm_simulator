# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-16T11:45:28Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101616-cnn-hint · minicpm5-2b-v031 · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_minicpm5-2b-v031/level_01/post_health_minicpm5-2b-v031.json` · `output/single/benchlog/sweep_minicpm5-2b-v031/level_01/guidellm_minicpm5-2b-v031.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_minicpm5-2b-v031/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

minicpm5-2b(openbmb/MiniCPM5-2B · bf16)를 NVIDIA GB10 ×1 GPU ×1 노드에서 vLLM v0.31.0 소스빌드(stock · 빌드 패치 0)로 max-model-len 32768 · KV dtype auto · PLE 없음 · spec off · cudagraph 형상으로 상주 서빙했고, CNN/DailyMail 입력 full-bench 판정점(동시성 1)은 decode 38.13 t/s 다(PASS · 루브릭 권한 weak · floor 16.13).
가장 비싼 벽은 서빙이 아니라 측정 하네스였다 — lite 게이트가 오래 상주한 서빙의 엔진 로그 꼬리만 읽어 measurement_path_failed 로 막았고(W1 · 머리 우선 파싱으로 해소), 목표 길이 정확일치 비교가 BOS 를 세는 토크나이저에서 행을 거의 다 버려 같은 프롬프트를 반복시켰다(W2 · 분포 최빈값 기준 · 재측정).
가장 오해하기 쉬운 값은 KV 절대클램프(V4 — 앞선 캠페인 승계 · 이 계보에서 조정 없음 · 필요량보다 크다)와 교정 전 측정의 TTFT(SUPERSEDED 리포트 · prefix cache 효과로 보이나 원인 미조사 추론 — M2 · Q1)다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | minicpm5-2b | sweep_index.meta(output/single/benchlog/sweep_minicpm5-2b-v031) |
| HF repo | openbmb/MiniCPM5-2B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) |
| HF revision(체크포인트 git HEAD) | f97400052a43d642bbc6e9975e2397e3ae6a6b52 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T07:54:23Z 이전 마지막 이동 2026-10-07T05:21:19Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) |
| base 슬러그 | minicpm5-2b | model(minicpm5-2b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_minicpm5-2b-v031)) · q 축 = /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_minicpm5-2b-v031) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_minicpm5-2b-v031) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_minicpm5-2b-v031) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | minicpm5-2b | 서빙 yaml model(output/single/configs/minicpm5-2b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · hf_repo(openbmb/MiniCPM5-2B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/minicpm5-2b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/minicpm5-2b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만든 전제**

1. **통합메모리 노드에 두 모델을 함께 상주시키는 메모리 설계** — GB10 은 CPU·GPU 가 한 메모리를 쓰고, 이 셀은 같은 노드의 다른 모델(Granite-4.2-8B · 포트 8001)과 공존하도록 "64GB Spark 1대 천장(≈47.7GiB)" 을 호스트 총량으로 환산한 gpu-memory-utilization 0.39 + KV 절대클램프로 묶였다 → vLLM 의 기동 free-memory 게이트를 두 인스턴스가 함께 통과하고, KV 크기는 gmu 가 아니라 클램프가 정한다(엔진 로그 "skipped memory profiling" · 01 §1.4) → 전용 VRAM GPU 나 단독 상주에서는 0.39 라는 환산의 근거가 사라지므로 gmu · KV 를 처음부터 다시 정한다. 근거 plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §2(U3) · §4(D4).
2. **측정 중 같은 노드에 다른 상주 서빙이 있었다** — 측정 시점에 메인 노드에는 granite(8001)와 이 모델(8002)이 함께 떠 있었고(plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 상주 서빙 목록), lite 레그의 시스템 RAM 점유 96.2 GiB(79%)는 두 서빙을 합한 노드 관측이다(bench_report_26101616_54_23_minicpm5-2b_GB10_0.31.0 §lite 지표) → 단독 서빙 노드에서는 RAM 여유 · 벤치 예산 게이트 결과가 다르다. 다만 granite 의 CNN 스윕은 07:25:36Z(그 셀 sweep_index generated_utc)에 끝났고 granite 컨테이너 엔진 로그의 마지막 통계 줄은 07:25:48Z `Running: 0 reqs` 다(그 뒤 요청 0건 — 사실 검증이 확인한 계보 밖 기록) → 이 셀의 2차 · 3차 스윕 동안 granite 는 유휴였고, 남는 차이는 메모리 상주뿐이다.
3. **토크나이저가 길이를 세는 방식** — 이 모델의 토크나이저는 BOS 를 세어 concatenate 1024 전처리 행이 1025 토큰으로 나왔다 → 행 선택 기준이 정확일치이면 목표 길이 행이 사라진다(W2) → 다른 모델은 같은 프리셋에서도 다른 목표 길이 · 다른 실측 입력 토큰을 낸다. 근거 testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §1.

**(나) 이 형상이 버티는 기전 대 다른 형상**

- 다른 서빙 형상 — 이 계보는 PLE · offload · 다른 KV 값 · 양자화 · eager 를 하나도 시도하지 않았다 — 비교할 실패 형상이 없다 — 지위 해당 없음 — testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3(사건은 측정 하네스 2건뿐) · plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §4.
- 교정 전 측정(2차 스윕) — 이 셀과 다른 것: 입력 행이 11 개로 줄어 반복됐다(측정 구성 표에서 다른 키는 `dataset_duplicate_sampling` True/False 하나 · 서빙 형상 · 도구 · 반복 · 측정 환경 스냅샷은 두 리포트가 같다) — 기전 후보: 반복된 프롬프트의 접두 KV 블록을 prefix cache 가 재사용해 prefill 을 건너뛰면 TTFT 가 줄어든다(엔진 설정 enable_prefix_caching=True 는 관측) — 지위 미결(추론 · 원인 미조사 — 엔진 로그의 기동 이후 누적 prefix cache hit rate 가 2차 스윕 뒤 정점 76.6% 로 오르고 3차 스윕 동안 51.5% 까지 내린 추이만 있고 prefix cache 를 끈 대조는 없다) — testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3 · devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 1차 스윕 lite 레그 요청은 전부 성공했으나 KV 지표가 N/A 로 남아 lite 판정이 GuideLLM 진입을 막았다 | 커밋 0f8800c — KV · 가중치 파싱이 엔진 로그 머리 캡처를 먼저 읽고 비면 꼬리로 메운다 · 재판정 pass(KV 35.0 GiB) | arch-invariant | 최종 스윕 lite_leg.log lite_verdict pass (exit 0) · KV cache 점유 35.0 GiB |
| 2 | W2 | 2차 스윕은 full 완주 · PASS 였으나 스윕 meta 가 dataset_duplicate_sampling True — 12 런이 같은 11 프롬프트를 돌려 썼다 | 목표 길이를 전처리 분포 최빈값으로 고른다(dataset_prep target_rows · 자체검사 T1~T4) · 3차 스윕 재측정 · 앞선 리포트 SUPERSEDED | arch-invariant | 최종 스윕 런 사이드카 prompt_tokens_count_target 1025 · rows_available 6254 · duplicate_sampling false · 12 런 전부 겹치지 않는 연속 행(testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §4) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 1차 스윕 lite 판정이 measurement_path_failed 로 full 진입을 막았다 | 반증 | devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 · 커밋 0f8800c |
| M2 | 2차 스윕이 같은 11 프롬프트를 반복했는데도 full 완주 · PASS 리포트가 발행됐다 | 반증 | testlog_26101619_54_16_CNN_fullbench_셀_minicpm5_2b_v031_판정 §3 · §4 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_minicpm5-2b-v031/lite_engine_head_minicpm5-2b-v031.log:38(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | f97400052a43d642bbc6e9975e2397e3ae6a6b52 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-16T07:54:23Z 이전 마지막 이동 2026-10-07T05:21:19Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.minicpm5-2b-v031) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/single/benchlog/sweep_minicpm5-2b-v031 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 — source-build.** 계보의 판정 문장은 plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §3 사실 표의 "source-build(torch ≥2.11 ABI 벽) · wheel 은 `manylinux_2_28_aarch64`(cuXXX 없음)" 한 줄이다(같은 표: vLLM 0.31.0 의 torch 핀 `==2.13.0` · NGC 베이스 `26.07-py3` 의 torch 2.13.0a0+9186a08). 위 표의 트랙 행 출처는 build-ledger.dockerfile 이다. 다른 트랙으로 갈 수 있는 조건은 같은 판정 문장(torch ≥2.11 이면 ABI 벽)이 성립하지 않는 경우뿐이며, 이 계보는 그 조건을 따로 시험하지 않았다.

(2) **vLLM 버전 문자열과 생산자.** 재현 좌표는 빌드 입력 ref `v0.31.0`(docker history build-arg VLLM_REF) 또는 40자 SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`(output/single/resolved.json upstream_delta.to_sha)다. 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 는 지은 엔진이 기동 때 스스로 보고한 문자열이라(생산자 = 엔진 로그 기동 배너) 빌드 입력이 아니다 — 안의 `g` 뒤 단축 해시가 40자 SHA 의 앞부분과 같다는 대조에만 쓴다. 인증서 강한 키 `vllm_version` 은 미관측이다(인증서 비발행 — 루브릭 권한 weak). 리포트 측정 환경 표의 `vllm_version 0.31.0` 은 측정 도구가 채운 값이고 full 리포트는 출처를 싣지 않았다 — 같은 config 의 lite 리포트는 같은 키의 출처를 `derived(envfile IMAGE_TAG)` 로 적는다(bench_report_26101619_minicpm5-2b_GB10_0.31.0 §측정 환경 스냅샷). wheel 메타는 미관측(소스빌드).

(3) **arch-locked 값과 재도출.** `TORCH_CUDA_ARCH=12.1a`(docker history build-arg — 01 §1.4 build 명령) → 네 GPU 의 compute capability 로 다시 정한다(`nvidia-smi --query-gpu=compute_cap --format=csv` — 확인 방법 · 이 계보에서 실행한 명령 아님). CPU arch `arm64`(image inspect) → `uname -m` 과 NGC 베이스의 아키텍처를 맞춘다. 이미지 CUDA 13.3.1.008 · torch 2.13.0a0+9186a08 은 NGC 베이스가 정한 이미지 층 값이다 → 네 호스트 드라이버가 받는 NGC 태그를 upstream-version-watch 의 해소 스크립트로 다시 고른다. 드라이버 580.173.02 는 manifest 선언을 옮긴 값이다(관측 아님) → 네 호스트에서 `nvidia-smi` 로 확인한다.

(4) **선언 대 관측.** NGC 베이스는 선언(resolved.json `ngc_base.image`)과 관측(image inspect `NVIDIA_PYTORCH_VERSION`)이 접두 일치한다. 갈리는 것은 Dockerfile 이다 — 실린 `Dockerfile.source-build`(worktree)는 측정 이미지 docker history 의 RUN·COPY 와 16/17 만 맞고 마지막 커밋이 이미지 Created 뒤다(01 §1.1 레시피 경고). 서빙 이미지의 실제는 docker history 쪽이다 — 예로 실린 파일의 `ARG BUILD_JOBS=16` 기본값 대 history build-arg `BUILD_JOBS=8`.

(5) **사실 블록이 미관측으로 둔 환경 좌표.** 호스트 층: plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite 머리 유효맥락이 "aarch64 · CUDA 13.2 드라이버 580.173.02" 를 적는다(선언 문서 · 노드별 관측 아님). 노드 메모리 총량: 블랙박스 예산 선언 행의 `mem_total_mib=124608`(01 §1.4 이벤트 표 — 선언 행에 실린 값) · 셀 config `test_device_total_gib` 121.7. 호스트 OS: GuideLLM 결과 JSON 의 platform 문자열 `Linux-7.0.0-1019-nvidia-aarch64-with-glibc2.41`(벤치 컨테이너가 본 커널 · glibc 는 컨테이너 것) — 배포판 이름은 미기록. 인터커넥트: 해당 없음(single · TP=1). 이미지 층(CUDA 13.3.1.008 · torch)과 호스트 층(드라이버 · 호스트 CUDA 13.2)은 다른 층이다.

(6) **핵심 의존 핀.** `flashinfer-python==0.7.0.post1`(artifacts/build_recipe/requirements.txt — 머리 주석대로 vLLM 0.31.0 wheel Requires-Dist 를 옮긴 목록이라 공유 이미지의 기본값이지 이 모델의 요구가 아니다) · `ray==2.48.0`(build-arg RAY_VERSION — Dockerfile 주석대로 2노드 Ray 분산 서빙용 공유 구성 · 이 셀은 TP=1 single 이다) · torch 는 NGC 베이스의 것을 그대로 쓴다(Dockerfile 의 use_existing_torch 단계). Triton · DeepGEMM 핀은 실린 레시피에 없다. 엔진은 attention 으로 FlashAttention version 2 를 골랐다(lite_engine_head L54).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | 사람 결정 D4 — 64GB Spark 천장(≈47.7GiB)을 호스트 총량으로 환산한 값(두 인스턴스 기동 게이트 공존용) · 이 계보에서 바꿔 잰 기록 없음 · KV 클램프 지정으로 엔진이 이 값을 KV 산정에 쓰지 않는다고 로그에 명시(L74) | V1 |
| `max-model-len` | 32768 | inherited | 사용자 지시로 4모델 공통 고정(LangGraph RAG 용) · 줄이거나 늘려 잰 기록 없음 | V2 |
| `max-num-seqs` | 8 | inherited | 동시성 요구 8(D5 사람 결정)을 그대로 씀 · 바꿔 잰 기록 없음 · 이 값 때문에 프리셋 스윕 레벨 16 이 걸러졌다(sweep_index bench_knobs.levels.filtered_out) | V3 |
| `kv-cache-memory-bytes` | 37580963840 | inherited | D4 절대 클램프 방침으로 앞선 캠페인에서 손 산정한 35.0 GiB(계보 표 9건 안에는 산정 기록이 없고, 이 계보 밖 문서 devlog_26101412 · testlog_26101412 가 KV = 천장 − 가중치 − 오버헤드 · max_safe 선택 · kv_source=hand 로 기록한다) · 이 계보에서 조정 없음 · 엔진 보고 873,808 토큰 · 32768 기준 최대 동시성 26.67x 로 max-num-seqs 8 보다 크다(음성대조라는 기록 없음) | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V4) · `gpu-memory-utilization`(V1) · `max-num-seqs`(V3) · `max-model-len`(V2). 절차: 모델 config 의 층 수 · KV 헤드 · head_dim 으로 토큰당 KV 바이트를 잡고(이 모델은 plan_26101410 §모델 4종이 42KiB · 32k 1시퀀스 1.3GiB 로 적었다) 목표 컨텍스트 × 동시 시퀀스 수로 필요량을 정한 뒤, 기동 엔진 로그의 `GPU KV cache size … Maximum concurrency` 줄로 확인한다. gmu 는 네 호스트 총량과 같은 노드의 다른 상주 프로세스로 다시 환산한다.

② **필요조건으로 확인된 값(declared-requirement)** — 없다. 이 계보에는 서빙 노브를 빼거나 바꿔 실패한 기록이 없다(벽 W1 · W2 는 측정 하네스 쪽이다). 명시하면 깨지는 노브도 관측되지 않았다. 러너 플래그(`--tool-call-parser minicpm5` · `--reasoning-parser qwen3` · `--enable-auto-tool-choice`)는 §1.3 지위표 밖이다.

③ **승계값(inherited)** — 네 노브 모두 앞선 상주 서빙 캠페인(plan_26101410)에서 정한 값을 이 계보가 그대로 썼다. 32768 고정 · 동시성 요구 8 · 64GB 천장 환산 0.39 는 사람 결정이고, KV 클램프는 그 캠페인의 손 산정값이다(계보 표 안에는 산정 기록이 없고, 이 계보 밖 문서 devlog_26101412 가 "천장 − 가중치 − 오버헤드" · kv_source=hand 로 기록한다) — 어느 것도 최적을 찾는 스윕을 거치지 않았다. 특히 KV 35.0 GiB 는 엔진 보고로 32768 기준 최대 동시성 26.67x 라 max-num-seqs 8 보다 훨씬 크다(계보 표 안에는 이유가 없고, 이 계보 밖 문서 devlog_26101412 가 사용자의 "64/64 할당" 의도에 맞춰 min(required, max_safe) 대신 max_safe 를 택했다고 기록한다 — 음성대조라는 기록은 없다).

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **상위 vLLM 에서 자체 패치가 필요한가** → 이 셀은 빌드 패치 · 런타임 패치 · 포크 핀이 0 이고 인라인 strip-hoist 는 원장이 skipped 로 기록했다(01 §1.1) → 확인: 새 이미지 빌드 로그의 `[patch] strip-hoist applied to N file(s)` 줄과 이미지 안 빌드 원장(`/opt/easy-vllm/build_ledger.json`) → 분기: skipped 면 그대로 / applied 면 NGC torch 가 바뀐 것이므로 S1 해소부터 다시(upstream-version-watch) / 빌드 실패면 이 지도 폐기.
2. **기동 성공 판정** → health 200 + chat 추론 1회(발행 자격 근거 파일과 같은 술어) + 도구호출을 쓰면 `tool_calls` 파싱 1회(plan_26101410 §6 AC3) · 엔진 로그 `reserved 35.0 GiB memory for KV Cache as specified by kv_cache_memory_bytes` 류 줄과 `GPU KV cache size` 줄 · 패치 발화 서명은 해당 없음(패치 0) → 분기: KV 줄의 최대 동시성이 네 max-num-seqs × 컨텍스트를 못 담으면 V3 · V4 재도출 / 기동 거부면 vllm-recipe-explorer 로.
3. **호스트 메모리 안전 예산 선언** → 통합메모리 호스트면 필요하다 · 산식 입력 = 가중치 · KV 클램프 · overhead · MemTotal(01 §1.4 이벤트 표의 `budget_declare` 행) · 같은 노드에 다른 모델이 있으면 한 선언에 합산(plan_26101410 §4 D6) → 분기: 단독 상주면 이 셀의 합산 overhead(60002 MiB · 선언값)를 옮기지 말고 다시 잰다.
4. **측정 데이터셋 행 선택** → 벤치 도구가 목표 길이를 분포 최빈값으로 고르는 판본(커밋 7db9d67 이후)인지 · 런 사이드카(`level_*/dataset_*.json`)의 `duplicate_sampling` 이 false 이고 `prompt_tokens_count_target` 이 네 토크나이저의 분포와 맞는지 → 분기: true 면 그 측정 폐기 · 재측정(W2).
5. **lite 게이트** → 오래 상주한 서빙이면 lite raw 의 KV 가 N/A 가 아닌지(머리 우선 파싱 판본 · 커밋 0f8800c 이후) → 분기: N/A 로 `measurement_path_failed` 면 서빙이 아니라 하네스 수리(W1).

실패 신호별 행선지 —
- 빌드 · 버전 핀 문제 → `upstream-version-watch`
- 서빙 설정 · KV · 노브 문제 → `vllm-recipe-explorer`
- 성능 기대 이하 → `adversarial-benchmark`
- 노드 · 토폴로지 · 인터커넥트 문제 → `terraforming_node`

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101616-cnn-hint · minicpm5-2b-v031 · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101616_cnn_hint__main__minicpm5_2b_v031 |
| work-manifest | docs/_evidence/camp_26101616_cnn_hint__main__minicpm5_2b_v031.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-16T11:48:00Z |
| 승인 발화(전사) | 사용자(캠페인 개설 팝업 발화 전사) — "열기 · 5셀 발행 승인 (Recommended)" |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `메인 검증 Agent(minicpm 사실 검증 세션)`(저작자 `메인 저작 Agent(minicpm 저작 세션)` 아님) · 주장 132건 · 지적 8건 중 고침 8건 · 열림 0 · 2026-10-16T12:11:49Z |
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

- (사실) 이 태그는 캠페인 `camp-26101616-cnn-hint` 의 메인 CNN full-bench 셀이다. 같은 config 로 lite 만 잰 셀(경량 리포트 bench_report_26101619 · lite 등급 인증서)은 셀 id = config 이름 규칙 때문에 이 캠페인에 넣지 못해 별도 경로로 발행한다(devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §남은 것) — 두 태그는 서빙 설정이 같고 측정 등급이 다르다.
- (사실) 측정은 커밋 7db9d67 · 0f8800c · d4327a9 직전의 워킹트리로 돌았다(testlog_26101619_54_16 머리). 그래서 03 §3.4 의 도구 원문 스냅샷(측정 시각 이전 마지막 **커밋**)은 데이터셋 knob 이전 판본이다 — 이 측정을 실제로 돌린 바이트는 그 뒤 커밋 쪽이다.
- (의견) 교정 전 리포트의 TTFT 를 인용하지 말 것을 권한다 — 03 §3.5 표에 기록으로만 남겼다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 빈 목록이다. 결정론부가 같은 셀은 같은 config 로 따로 발행하는 lite-only 셀 하나뿐인데(devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §남은 것) 서빙 yaml · 셀 env · 러너가 같아 가를 서빙 노브가 없다 — 둘의 차이는 측정 등급과 데이터셋이고, 데이터셋 이력은 이름이 아니라 03 에만 둔다(plan_26101614_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §2 U9). 후보 `plenone`(PLE 키가 없는 모델이라 가르는 것이 없다) · `specoff` · `graph` 는 이 계보의 어느 셀도 다른 값을 쓰지 않아 구분 노브가 아니다.
