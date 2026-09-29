# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-graph-autotool` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-09-29T05:55:19Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26092913-hint-v7-e2e · ds4f0731-1m-spec7-roce · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json` — 시점 범위: 작성 2026-09-29T05:12:32Z(output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

DeepSeek-V4-Flash-0731(dense fp8 블록 · expert fp4)을 GB10 GPU 1개 × 노드 2개 · Ray TP=2 · vLLM v0.29.0rc6 소스빌드 · 컨텍스트 1048576 · fp8 KV · PLE 없음 · dspark spec k=7 · cudagraph · NCCL 내장 IB(RoCE) · tool calling 플래그를 켠 구성 그대로 서빙해 동시성 1 decode 30.95 t/s(GuideLLM full · 반복 3 · explore PASS)를 쟀다.
가장 비싼 벽은 커널 교체 뒤 Socket 에 묶였던 NCCL 전송을 manifest 선택(`NCCL_IB_DISABLE=0` · `NCCL_NET=IB`)으로 RoCE 에 되살린 것(W16 · W17)과 호스트 기동 밸리가 KV 클램프를 10 GiB 로 누른 것(W9 · W10)이다.
가장 오해하기 쉬운 값은 `kv-cache-memory-bytes`(V7 — KV 필요량이 아니라 기동 밸리가 정한 값)이고, 판정 입력 accept_len 은 GuideLLM 레벨이 아니라 lite warm 레그 실측의 기계 승계값이다(V11).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | deepseek-v4-flash-0731 | sweep_index.meta(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| HF repo | deepseek-ai/DeepSeek-V4-Flash-0731 | 체크포인트 .git/config remote url(huggingface.co · /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) |
| HF revision(체크포인트 git HEAD) | 7872f01b1d1fe23eabc4c98b48bffcef5a386062 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-29T05:52:10Z 이전 마지막 이동 2026-08-31T07:23:00Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) |
| base 슬러그 | deepseek-v4-flash-0731 | model(deepseek-v4-flash-0731) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | fp8 | sweep_index.meta(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.29.0 | sweep_index.meta(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.29.0rc6 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.29.0rc6(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | deepseek-v4-flash-0731 | 서빙 yaml model(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · hf_repo(deepseek-ai/DeepSeek-V4-Flash-0731) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qfp8-len1048576-kvfp8-spec7-graph-autotool | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | fp8 | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json quantization_config quant_method=fp8 · vocab quant[fp8]←'fp8' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · vocab kv[fp8]←'fp8' |
| 꼬리 `spec7` | dspark speculative decoding k=7 켜짐 — 같은 q·len·kv 의 spec off(eager) 셀과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `speculative-config.num_speculative_tokens` = `7` |
| 꼬리 `graph` | cudagraph 실행(enforce-eager false) — 같은 q·len·kv 의 eager 셀과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enforce-eager` = `false` |
| 꼬리 `autotool` | enable-auto-tool-choice 켜짐(tool calling 구성으로 측정) — spec·graph 가 같고 이 플래그 없이 잰 같은 q·len·kv 셀(09-09 대조군 · 09-28 앞선 판)과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enable-auto-tool-choice` = `true` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 조건에서만 성립하는 것.**
- 통합메모리 · 노드당 한 풀(예산 선언 입력 `mem_total_mib=124608` · 01 §1.4 행 79 — 선언에 쓴 값) → 가중치 적재 중 가용이 급락하는 기동 밸리와 KV 몫 · 벤치 상주분이 같은 풀을 누른다 → KV 클램프 · 동접의 바인딩이 KV 필요량이 아니라 호스트 기동 밸리 + 절대 플로어가 된다(W9 · W10) → 노드 메모리가 더 크거나 GPU 메모리가 분리된 HW 에서는 클램프 · 동시 시퀀스 상한이 전혀 달라진다 — devlog_26090912 §서사 · ds4f0731-1m-spec7-roce.yaml §L13-14 · plan_26092808 §4.
- 노드당 GPU 1개 · TP=2 → 모든 텐서 병렬 통신이 노드 간 RoCE 를 탄다. 이 셀은 커널 `7.0.0-1019-nvidia` 위에서 NCCL 내장 verbs(`Using network IB` · `GDR 0`)로 성립했고, 드라이버는 `DMA_BUF_SUPPORTED=0` · `GPU_DIRECT_RDMA_SUPPORTED=0` 을 보고했다 → GPU 메모리를 직접 등록하는 경로(GDR)가 켜지는 드라이버 · 이미지 조합이나 다른 전송(Socket · 외부 플러그인)에는 이 성립이 옮겨지지 않는다 — testlog_26092808 §3 · testlog_26092807 §1.
- sm_121a(GB10) 커널 경로 → KV 는 DeepseekV4 어텐션의 `fp8_ds_mla` 레이아웃 하나로만 서고(W8) · GB10 NVML 이 메모리/SM 클록을 노출하지 않아 humming MoE 튜닝이 NVML 가드(40 패치) 없이는 크래시했고(W2) · 이 모델의 TRITON Mxfp4 MoE 는 SILU 미지원으로 엔진이 거부했으며(R3) · 옛 DeepSeek-V4-Flash(다른 체크포인트 · 옛 vLLM)에서는 MXFP4 expert 를 MARLIN 이 repack 해 통합메모리 OOM 이 났다(W1 — 이 모델 · 이 버전에서는 재시도 0) → 다른 아키텍처는 어텐션 · MoE 후보부터 다르다 — testlog_26090904 §3 · 40-humming-nvml-gb10.sh §L8-13 · 20-triton-kernels.sh §L6-11.

**(나) 이 형상이 버티는 기전 대 다른 형상.**
- KV 클램프 18 GiB · 동접 4(768K 셀) — 클램프 · 동접 · 컨텍스트가 다름 — 벤치 상주분(4 GiB)이 예산에서 빠져 절대 플로어 10240 MiB 에 걸려 사살 — 확정(사살 관측) — devlog_26090912 §서사.
- KV 클램프 14 GiB · 동접 3(768K 셀) — 클램프 · 동접 · 컨텍스트가 다름 — 기동 밸리(~8.2 GiB)가 절대 플로어를 밑돌아 memwatch 사살 2회, 같은 밸리를 10 GiB 가 생존 — 사살 · 생존 대조는 확정 · 밸리의 구성은 devlog_26090912 가 '155GB 로드 페이지캐시 적층' 이라 적었으나 분해 관측은 없다(주장 · 미검증 — 회수 가능한 page cache 가 왜 플로어를 침범했는지는 미결 · Q2) — ds4f0731-1m-spec7-roce.yaml §L13-14.
- KV dtype auto · turboquant — KV dtype 만 다름 — DeepseekV4 어텐션이 `fp8_ds_mla` 페이지 레이아웃을 강제해 엔진 assert 로 거부 — 확정(엔진 로그 assert 원문 · 소스 판독) — testlog_26090904 §3.
- moe-backend triton — MoE 백엔드만 다름 — Mxfp4 TRITON 커널이 SILU 미지원이라 엔진 거부(768K 셀) — 확정 — sweep_map_26090912_b768k_levers §셀.
- moe-backend auto — MoE 백엔드만 다름 — 옛 DeepSeek-V4-Flash 에서 auto 오라클이 MARLIN 으로 떨어져 expert repack 트랜지언트(~37 GiB)가 통합메모리 한 풀을 넘음 — 그 모델에서는 확정 · **이 모델 · 이 버전에서는 시도 0 이라 미결**(yaml §L16 주석도 '재검증 대상') — 20-triton-kernels.sh §L9-11.
- NCCL `NCCL_IB_DISABLE=1` + `NCCL_NET` 미방출(09-28 앞선-1) — 전송 설정만 다름 — 엔진이 `Using network Socket` 으로 조용히 강등 — 관측 확정 · 기전(IB 끔 → SPCX 플러그인이 장치 거부 → Socket)은 testlog 의 서술 — testlog_26092808 §1.
- 이 형상 — 위 차이를 **모두** 반대로 둔 것(KV fp8 · 클램프 10 GiB · 동접 1 · humming + NVML 가드 · `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` · spec + cudagraph) — 이 run 시도 1 사살 · 트립 0(main 원장 기준 — sub 원장은 2026-09-11 뒤 미회수라 서브의 사살 · 트립은 관측 범위 밖) · `Using network IB` 2 / `Socket` 0 · `iova2` 0 — 확정 — 01 §1.4 · engine_v7 로그.
- 09-28 같은 레시피(tool-choice 없음 · 같은 이미지 digest · 같은 커널) — 엔진 non-default args 의 `enable_auto_tool_choice` 한 키만 다름 — 서빙은 같게 성립 · 동시성1 32.2 대 이 run 30.95 — 차이의 원인은 미분해(미결 · Q4) — bench_report_26092809 · bench_report_26092914.
- 09-09 같은 레시피(커널 6.17 계열 · 다른 이미지 digest · 서빙 성공 · 성능 판정은 소급 재판정 NEEDS_RUBRIC — M5) — 커널 · 이미지 재빌드 · NCCL env 일부 · tool-choice 플래그가 다름(전송은 같은 내장 IB · M1) — 동시성1 30.54 — 차이의 원인은 미분해(미결) — sweep_map_26090912_e1m_levers · testlog_26091412 §5.1.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 옛 DeepSeek-V4-Flash 서빙에서 MoE 가 MARLIN 으로 떨어져 호스트 하드다운 · 워치독 트립이 났다 | 공유 이미지에 TRITON 경로 패치(20 · 30) · 이 계보 레시피는 moe-backend humming 명시(이 모델에서 auto 재시도 0) | arch-locked | engine_v7 L893 Using 'HUMMING' Mxfp4 MoE backend · 이 run 시도 1 사살 0(02 §2.2 · main 원장 기준) |
| 2 | W2 | humming MoE 서빙이 가중치 적재 직후 EngineCore init 에서 크래시했다 | 40-humming-nvml-gb10.sh — 클록 쿼리 2개 가드 · GB10 폴백(273 GB/s · 1500 MHz) | arch-locked | engine_v7 L1065 humming GEMM config 재정의 뒤 L1344 init engine 완료(크래시 없음) |
| 3 | W3 | 멀티 TP=2 기동이 Ray 클러스터 형성 뒤에도 World size 오류로 죽었다 · 09-08 스모크는 러너 게이트가 rc=2 로 막았다 | yaml 에 tensor-parallel-size 2 · distributed-executor-backend ray 명시 · serve_runner 가 누락 시 fail-loud | judgment | 이 run 시도 1 스모크 PASS(serve_proof health 200 · 추론 1회) · engine_v7 L62 non-default args 에 distributed_executor_backend ray |
| 4 | W4 | 0.29.0rc6 1차 소스빌드가 양 노드 동일하게 의존 해소에서 실패했다 | constraint 를 소스트리 선언 0.6.18 에 양보(requirements.txt 머리 주석 · 2차 빌드) | arch-invariant | testlog_26090904 §1 2차 빌드 양노드 PASS · 01 §1.1 requirements.txt 쓰인 바이트 관측 |
| 5 | W5 | 첫 멀티 스모크가 컨테이너 이름 미설정으로 rc=2 차단됐다 | 셀 env 에 MASTER_CONTAINER_NAME · SLAVE_CONTAINER_NAME 을 관례 이름으로 명시 | arch-invariant | 실린 셀 env §L9-10 · 이 run 시도 1 스모크 PASS |
| 6 | W6 | 첫 멀티 스모크가 예산 선언 입력 부재로 rc=4 차단됐다 | 옛 hint 실측 10GiB 를 시드로 선언하고 멀티에서 직접 수렴(실측 재산정) | arch-scaled | testlog_26090904 §4 예산 선언→honored→up 순서 · 이 run 행 79~80 budget_declare · budget_honored |
| 7 | W7 | 첫 멀티 스모크에서 master 컨테이너가 즉사했다 | 주석 전행 분리 · 검증 추가 | arch-invariant | 실린 셀 env 의 주석이 모두 독립 줄 · 이 run 시도 1 스모크 PASS |
| 8 | W8 | KV dtype auto 트라이얼이 엔진 assert 로 거부됐다 · turboquant 도 같은 경로로 거부 | kv-cache-dtype fp8 고정 · 캠페인 KV 축 개정(4셀 void) | arch-locked | engine_v7 L891 Using DeepSeek's fp8_ds_mla KV cache format |
| 9 | W9 | 768K 셀 1차 수렴(18GiB · 동접 4) 서빙이 memwatch 에 사살됐다 | 벤치 상주를 포함해 재산출(2차 14GiB · 동접 3) | arch-scaled | devlog_26090912 §서사 수렴 2단계로 이행(그 뒤 W10) |
| 10 | W10 | 768K 셀 클램프 14 GiB 에서 기동 중 memwatch 가 두 번 컨테이너를 사살했다 | kv-cache-memory-bytes 10 GiB 수렴 · 예산 선언 · 벤치 전 페이지캐시 드롭 | arch-scaled | 이 run 시도 1 사살 · 트립 없음(02 §2.2 · main 원장 기준 — sub 는 관측 범위 밖) · devlog_26090912 §서사 3차 이후 전 서빙 밸리 생존 |
| 11 | W11 | 레버 스윕 진입이 호스트 열 게이트에 rc=6 으로 막혔다 | 열 임계 교정(warn 97 · hard 99 · 커밋 f02990c) · 양노드 배포(사람 sudo) | arch-locked | devlog_26090912 §서사 음성대조 106 kills → 0 · 이후 스윕 진입 |
| 12 | W12 | GuideLLM 이 deepseek_v4 토크나이저를 파싱하지 못해 full 측정이 서지 않았다 | run_bench.sh tokstage 배선 수정 | arch-invariant | 이 run full ×3 완주(03 §3.3 repeats_completed 3) · 측정 도구 원문 run_bench.sh@434fa6740831 |
| 13 | W13 | 사살 뒤 재기동에서 새 예산 선언이 막혔다(L1c) | 킬된 서빙은 --down 으로 먼저 회수하는 정규 절차 | arch-invariant | devlog_26090912 §재개 지침 · 이 run budget_clear 05:54:03Z(행 107) |
| 14 | W14 | 1M 기준 셀 측정이 serve_failed 로 끝났다 | 스모크 PASS 뒤 재측정(sweep_map_26090912_e1m_levers E0 measured) | arch-invariant | sweep_map_26090912_e1m_levers E0-base 축 근거 '1차는 기동 전 조기 발사로 serve_failed' |
| 15 | W15 | 스윕 정지 기록이 미래 타임스탬프로 거부됐다 | 주입값을 실제 시계에서 읽어 넣는다 | arch-invariant | devlog_26090912 §서사 ③ · 지도 2종 발행(complete) |
| 16 | W16 | 커널 7.0 위 멀티 TP=2 가 RoCE 경로에서 실패해 렌더러가 Socket 을 불변으로 박았다(09-17) | 렌더러 전송 선택 manifest.interconnect.nccl_transport(socket 기본 · rdma) 도입 · 서브는 sync_to_sub 재렌더 배달 | judgment | engine_v7 Using network IB 2 / Socket 0 · iova2 0건(L147 · L938) |
| 17 | W17 | rdma 로 선언한 09-28 기동(원장 앞선-1) 이 양 노드에서 Socket 전송으로 떨어졌다 | rdma = NCCL_IB_DISABLE=0 · NCCL_NET=IB 명시(8319b4f) · 앞선-1 은 --down 정식 회수 | judgment | engine_v7 L147 Using network IB · 01 §1.4 측정 env 관측 NCCL_IB_DISABLE 0(양 노드) |
| 18 | W18 | 09-28 full 벤치 1차 스윕(r1)이 부하 전에 measurement_void 로 끝났다 | --backend openai-chat 으로 r2 재초기화 · 레벨 1/2/4/8/16 × 반복 3 완주 | arch-invariant | sweep_map_26092809 셀 measured · 이 run sweep_map_26092914 레벨 run 시도 합 15 |
| 19 | W19 | 09-28 측정 뒤 상주 서빙에서 tools + tool_choice auto 요청이 HTTP 400 으로 거부됐다 | 서빙 yaml 에 enable-auto-tool-choice true 한 줄 — 09-28 은 측정 뒤 추가 · 이 판은 측정 구성에 포함 | arch-invariant | engine_v7 L62 non-default args enable_auto_tool_choice True · L1349 "auto" tool choice has been enabled · testlog_26092914_54_41_hint_v7_라이브E2E_DS4F0731_판정 §2 tool calling PASS |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 09-28 기동(원장 앞선-1) 설정을 09-09 PASS 의 NCCL 조건이라고 적었다 | 반증 | 09-09 lite_engine_e-1m-kvfp8-e1combo.log NCCL_IB_DISABLE set by environment to 0 · Using network IB · testlog_26092808 §1 정정 |
| M2 | 측정 이미지의 재빌드 날짜를 09-24 로 적었다 | 반증 | 01 §1.4 FACT:reproduce 빌드 층 시각 · testlog_26092807 §3 정정 |
| M3 | 측정 기록 · 계보 문서가 실행 드라이버를 580.173.02 로 적었다 | 반증 | output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json parity.driver main=sub 580.178.04 · 00 §0.4 driver_conflict |
| M4 | 09-28 devlog 가 그 측정의 인증서를 미발행이라고 적었다 | 반증 | 03 §3.3 FACT:bench_missing 인증서 비발행 판정 · 계보 evidence_candidates 의 인증서 경로 |
| M5 | 09-09 spec 셀 셋(이 레시피의 대조군 포함)이 verdict PASS(explore · floor 13.2)를 받았다 | 반증 | testlog_26091412 §5.1 변동 목록 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.29.0rc6 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 74c96922ecb9017f413318c76d1af83aa2ab45a5 | output/multi/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.29.0rc7.dev0+g74c96922e.d20260922 | output/multi/benchlog/engine_v7_ds4f0731-1m-spec7-roce.log:58(이 측정의 엔진 로그 기동 배너 · 같은 실행의 전체 사본 — 판별: EngineCore pid ['1272'] 일치 · 가중치 로드 줄 `09-29 05:02:04 … 355.11 seconds` 공유 · 사본 첫 시각 09-29 04:55:20 ≤ 꼬리 첫 시각 09-29 04:56:02 ≤ 측정 시작 09-29 05:13:42(꼬리 캡처 output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce/lite_engine_ds4f0731-1m-spec7-roce.log · 규칙 EngineCore pid 집합 일치 ∧ 가중치 로드 줄(시각·초) 공유 ∧ 사본 첫 시각 ≤ 꼬리 첫 시각(∧ ≤ 측정 시작))) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.29.0rc7.dev0+g74c96922e.d20260922.cu133 | attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json · 노드별 관측) — main=sub 일치 · 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log) |
| 체크포인트 revision(HF · git HEAD) | 7872f01b1d1fe23eabc4c98b48bffcef5a386062 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-29T05:52:10Z 이전 마지막 이동 2026-08-31T07:23:00Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.29.0rc6-cu133-aarch64-source | 셀 env(output/multi/envs/.env.ds4f0731-1m-spec7-roce) IMAGE_TAG |
| 이미지 digest | sha256:a2c4ca499fb2473191bcdfc7e192445750b4a7b6803cfe14e50b43f401000544 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log) · ⚠ 선언 580.173.02 과 다르다(driver_conflict) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log) |
| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: attestation parity.driver(output/multi/benchlog/attestation_ds4f0731-1m-spec7-roce.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-09-29T05:12:32Z < 측정 시작 2026-09-29T05:13:42Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26092914_54_41_hint_v7_live_ds4f/serve.log) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_ds4f0731-1m-spec7-roce · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `fp8 (fmt=e4m3 · weight_block_size=[128, 128])` | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json quantization_config quant_method |
| routed experts | `fp4` | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json expert_dtype |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json torch_dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 1개 · unknown 6개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build.** 실린 빌드 레시피 머리가 이유를 적는다: prebuilt vLLM wheel 의 `_C`(public torch 빌드)가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막히고, `_C` 를 NGC torch 에 맞춰 직접 컴파일하면 풀린다(`Dockerfile.source-build` §L28-31 — torch 2.11+ 구간). 캠페인 plan 도 같은 이유를 적었다(plan_26090819 §2 build track 행 'torch 핀 2.13.0(≥2.11) → NGC 베이스 torch와 prebuilt wheel ABI 불일치'). 이 이미지의 torch 는 NGC alpha `2.13.0a0+9186a08` 이다(위 표). 다른 트랙은 그 문장에서만 파생된다 — 대상 vLLM wheel 의 `_C` 가 베이스 torch 빌드와 ABI 가 맞을 때만 wheel 이 후보다(이 rc 는 wheel 자체가 미발행이었다 — plan_26090820 해소값 표). 같은 파일 §L39 는 이 키(26.07-py3 × 0.29.0rc6)를 **미검증 attempt-build** 로 경고하고 중재를 스모크에 맡긴다 — 09-08 스모크가 통과했고(testlog_26090904 판정 요약) 이 run 의 스모크도 통과했다.

(2) **버전 문자열 다섯.** 빌드 입력 ref `v0.29.0rc6`(docker history build-arg — 빌드한 쪽이 넣은 값) · 소스 SHA `74c96922ecb9017f413318c76d1af83aa2ab45a5`(resolved.json 선언 · attestation 이 양 노드 `vllm_sha` 같음을 관측) · 측정 강식별 키 `vllm_version` 0.29.0(측정 도구 `sweep_bench.sh` 가 **이미지 태그를 x.y.z 로 자른 값** — 엔진 자기보고가 아니고 rc 접미가 사라졌다 · 이 run 은 인증서가 없어 위 표의 인증서 행은 미관측) · wheel 메타 `0.29.0rc7.dev0+g74c96922e.d20260922.cu133`(attestation — `g74c96922e` 가 같은 커밋을 가리킨다 · `rc7.dev0` · `d20260922` 접미는 setuptools_scm 이 태그 커밋 위 수정된 작업트리(소스 패치 적용)에 붙이는 꼴로 보인다 — 추론) · 엔진 기동 배너 `0.29.0rc7.dev0+g74c96922e.d20260922`(이 측정의 전체 엔진 로그 사본 engine_v7 L58 — 위 표 지위 `this-measurement` · wheel 메타와 같은 문자열). **재현 좌표는 릴리스 태그 `v0.29.0rc6` 또는 40자 SHA** 다.

(3) **arch-locked.** `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 sm_121a — 네 GPU 에서 `torch.cuda.get_device_capability()` 로 재도출) · CPU arch arm64(`uname -m`) · 드라이버 580 계열(관측 580.178.04 — 네 호스트 `nvidia-smi` 로 보고 이미지 CUDA 13.3 을 지원하는지부터 확인) · 공유 패치 안의 GB10 고정값(10 의 `TORCH_CUDA_ARCH_LIST` 12.1a · 30 의 capability 상한 `(13, 0)` · 40 의 NVML 폴백 273 GB/s · 1500 MHz — 01 §1.2) · NCCL 자리(HCA · GID · iface)는 manifest `interconnect` 에서 다시 파생한다 · 호스트 열 게이트 임계는 유닛 교정값이다(W11).

(4) **선언 대 관측.** vLLM SHA 는 resolved.json(선언)과 attestation(관측)이 같다. 드라이버는 **다르다** — 측정 기록(sweep meta · bench_report 스냅샷)과 계보 testlog 의 580.173.02 는 manifest 선언을 옮긴 값이고, 양 노드 attestation 관측은 580.178.04 다(위 표 `driver_conflict` · M3). 서빙 이미지의 실제는 관측 쪽이다. 이 run 의 attestation(05:12:32Z · 측정 시작 05:13:42Z 전)은 측정을 서빙한 같은 기동의 스모크 직후 관측이다(FACT 판별 — 스모크 로그 SMOKE PASS → attestation 작성 → `--keep-up` 순서 · 그 기동의 예산 선언 창이 측정 끝까지 닫힘 없이 이어짐 · 측정 엔진 APIServer pid 하나). vLLM 빌드 문자열은 측정 실행의 엔진 기동 배너도 같은 값을 직접 보인다. NGC 베이스는 image Env 와 resolved.json 이 접두 일치한다. requirements.txt 는 빌드 원장 sha256 으로 쓰인 바이트임이 관측됐다(01 §1.1).

(5) **환경 좌표.** 호스트 층: 드라이버 관측 580.178.04(attestation · 양 노드) — 계보 문서는 580.173.02 로 적었다(testlog_26092808 머리 유효맥락 · testlog_26092807 머리 — 선언값) · 커널 `7.0.0-1019-nvidia`(양 노드 · testlog_26092807 §1 — dpkg 2026-09-15 설치) · 노드 메모리 총량 124,608 MiB(예산 선언 입력 · 01 §1.4 행 79) · 인터커넥트 RoCE v2 2포트 · GID 3(testlog_26092807 머리) · 포트당 98.41Gb/s 검증(plan_26090819 §2) · 모델리스 all-reduce 1 GiB busbw 21.92 GiB/s(testlog_26092807 §2) · 호스트 CUDA 는 sweep meta `cuda_version` 132(manifest 선언 옮김 — 측정 아님)이고 testlog_26092807 §1 은 host libcuda 13000 · compat 13030 을 적었다. 이미지 층(CUDA 13.3.1.008 · torch 2.13.0a0 · NGC 26.07 · NCCL `2.30.7+cuda13.3` 엔진 로그 echo)은 층을 섞지 않는다. OS 배포판 이름은 계보에 미기록이다.

(6) **핵심 의존 핀.** Ray `2.48.0`(Dockerfile `ARG RAY_VERSION` §L221 — 멀티 TP 가 쓰지만 단일/멀티가 공유하는 이미지 기본값) · flashinfer-python `0.6.18`(requirements.txt §L21 — constraint 를 vLLM 선언에 양보한 값 · W4 · 공유 이미지 핀) · humming-kernels `[cu12]==0.1.12`(requirements.txt §L23 — **이 모델의 MoE 백엔드** · V9 · 40 패치 대상) · DeepGEMM `nv_dev` `a6b593d2826719dcf4892609af7b84ee23aaf32a`(10-deepgemm.sh §L23 — DeepseekV4 FP8 블록 GEMM 경로 · 공유 이미지의 arch-enablement) · vendored `triton_kernels`(20 — 이 셀은 TRITON MoE 를 쓰지 않는다). 이 모델의 요구로 읽을 것은 humming 과 DeepGEMM 경로뿐이고, 나머지는 한 이미지가 모든 모델을 서빙하는 공유 기본값이다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 14개 — `tuned` 3 · `inherited` 8 · `declared-requirement` 3.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | manifest 파생(노드 2 × GPU 1) · 대조군 트리플렛 값 그대로 · 이 계보에서 다른 값을 시도한 기록 없음(누락 시 실패는 executor backend 쪽 — W3) | V1 |
| `distributed-executor-backend` | ray | declared-requirement | W3 — 없으면 vLLM 이 multiprocessing 으로 가 World size > GPU(1) 로 죽는다(serve_runner.sh 헤더 2026-09-04 실측 · 09-08 스모크 rc=2 · 러너가 fail-loud 로 막는다) | V2 |
| `gpu-memory-utilization` | 0.85 | inherited | 캠페인 셀 config target_gmu 선언값을 대조군부터 그대로 승계 · 이 계보에서 바꿔 잰 기록 없음 · 뺐을 때의 실패 관측 없음(후보표의 declared-requirement 는 lockset 출처 표지에서 나온 것) | V3 |
| `max-model-len` | 1048576 | inherited | 캠페인 셀 축 1M · 모델 config max_position_embeddings 1048576 과 같다 · 768K 는 다른 셀이지 이 셀의 조정이 아니다 · 이 셀에서 조정 0 | V4 |
| `quantization` | fp8 | inherited | 체크포인트 quantization_config(fp8 블록)와 같은 값을 명시한 승계 · 다른 값 시도 기록 없음(expert 는 fp4 · 00 §0.4 양자화 구성) | V5 |
| `kv-cache-dtype` | fp8 | declared-requirement | W8 — auto 는 엔진 assert 로 거부 · turboquant 도 같은 경로로 거부 · DeepseekV4 어텐션이 fp8_ds_mla 레이아웃을 강제해 fp8 이 유일 지원 경로 | V6 |
| `kv-cache-memory-bytes` | 10737418240 | tuned | W9 · W10 — 768K 셀에서 18 GiB(벤치 상주 누락 사살) → 14 GiB(기동 밸리 사살 2회) → 10 GiB(같은 밸리 생존) 3차 수렴 · 1M 셀이 그 10 GiB 를 승계 · 1M 요청 하나 대비 엔진 보고 1.85x | V7 |
| `max-num-seqs` | 1 | inherited | 손레버 — yaml 주석이 10GiB ÷ 1M 요청 필요량 = 1.73x 를 내림해 1 · devlog 가 1M 동접 2 는 밸리 + 벤치 상주로 플로어 미달이라 산출(R6) · 1M 에서 2 를 띄워 잰 스윕 기록 없음 | V8 |
| `moe-backend` | humming | inherited | 대조군 레시피 승계 · 이 모델 · 0.29.0rc6 에서 triton 은 엔진 거부(R3 · 768K 셀) · auto 는 시도 0(yaml 주석 '재검증 대상') — auto 의 MARLIN repack OOM(W1)은 옛 체크포인트 · 옛 vLLM 의 실패 · 빼면(auto) 어떻게 되는지는 미관측 · humming 이면 GB10 NVML 가드(W2 · 40 패치)가 짝 | V9 |
| `enforce-eager` | false | tuned | 768K 에서 graph 셀(26.14)을 spec 셀(29.72) · 결합 셀(31.12)과 같은 클램프 · seqs 로 쟀다(기준 셀 대비 +52.8% 는 클램프 14→10 GiB · seqs 3→2 가 함께 바뀐 비교) · 1M 은 eager · spec off 기준 셀 대 spec + graph 셀로 함께 바꿔 쟀다(16.67 → 30.54) · 1M 에서 graph 단독 기여는 미분해 · 이 셀에서 조정 0 | V10 |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":7} | tuned | 768K 에서 spec 셀(29.72)을 graph · 결합 셀과 같은 클램프 · seqs 로 쟀다(기준 대비 +73.7% 는 클램프 · seqs 도 함께 바뀐 비교) · 1M 에서 graph 와 함께 바꿔 쟀다 · k=7 자체는 스윕 0(dspark_block_size 5 이상 조건) · 판정에 쓴 accept_len 2.015789473684211 은 lite warm 레그(vllm bench serve · 3요청) 실측을 판정 레벨로 기계 승계한 값 | V11 |
| `reasoning-parser` | deepseek_v4 | inherited | Hermes 용처로 대조군 레시피가 등록명을 확인해 넣은 값의 승계 · 다른 값 시도 0 · 09-28 상주 chat 에서 reasoning 분리 관측 · 이 run 스모크 reasoning_len 95 | V12 |
| `tool-call-parser` | deepseek_v4 | inherited | 대조군 레시피 승계 · 다른 값 시도 0 · 단독으로는 tool_choice auto 를 받지 못한다(W19 — enable-auto-tool-choice 가 함께 있어야 한다) | V13 |
| `enable-auto-tool-choice` | true | declared-requirement | W19 — tool call 용처의 필요조건(없으면 tool_choice auto 가 HTTP 400 · 09-28 관측) · 이 판에서는 측정 구성의 일부(engine_v7 L62 non-default args) — 성능 수치는 이 줄을 넣은 채 잰 것 | V14 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 반드시 다시 도출할 값.** `kv-cache-memory-bytes`(V7 · 바인딩은 호스트 기동 밸리 + 벤치 상주) — 예산 선언(바닥 = 총량 − 가중치 − KV − overhead)을 먼저 세우고 벤치 상주분을 넣어 작은 클램프로 띄워 기동 중 가용 최저점을 본 뒤 올린다 · 엔진 로그 `GPU KV cache size` 줄의 토큰 수와 `Maximum concurrency` 배수로 목표 컨텍스트 대비를 본다. `max-num-seqs`(V8) — 그 배수를 내림하고 밸리 · 벤치 상주를 더해 플로어를 넘지 않는 값으로 정한다. `gpu-memory-utilization`(V3) — 바꿔 잰 적 없는 승계값이다. `max-model-len`(V4) — 용처의 컨텍스트 요구(모델 네이티브 1,048,576 이내). `tensor-parallel-size`(V1) — manifest 의 노드 × GPU 에서 파생한다.

**② 필요조건으로 확인된 값.** `kv-cache-dtype` fp8(V6 · W8 — auto 는 엔진 assert 로 거부 · fp8_ds_mla 외 경로 없음) · `distributed-executor-backend` ray(V2 · W3 — 없으면 World size > GPU 로 죽는다 · serve_runner.sh 가 fail-loud 로 막는다) · `enable-auto-tool-choice` true(V14 · W19 — tool call 용처에서 없으면 `tool_choice:"auto"` 가 HTTP 400 · 이 판은 이 줄을 넣은 채 측정했다). `moe-backend` humming(V9)은 필요조건으로 **확인되지 않았다** — triton 으로 바꾸면 엔진이 거부한 기록(R3)은 있으나 빼면(auto) 어떻게 되는지는 이 모델 · 이 버전에서 시도 0 이고, auto 의 MARLIN OOM(W1)은 다른 체크포인트 · 옛 vLLM 의 실패다. 다만 humming 을 쓴다면 GB10 에서는 40 NVML 가드가 함께 있어야 선다(W2). 이 표 **밖**의 전송 노브: NCCL 은 `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` 를 명시해야 RoCE 가 선다 — **`NCCL_IB_DISABLE=1` 을 명시하면 조용히 Socket 으로 떨어진다**(W17).

**③ 조정된 적 없는 승계값.** `max-num-seqs` 1(V8 · 산출로 정한 손레버 — 최적이 아니며 동시성 2 이상에서 요청당 decode 가 나뉜다) · `gpu-memory-utilization` 0.85(V3) · `quantization` fp8(V5 — 체크포인트 선언의 반복) · `moe-backend` humming(V9 — triton 거부 외에 대안이 관측되지 않은 승계값) · `tensor-parallel-size` 2(V1 — 토폴로지 파생) · `reasoning-parser` · `tool-call-parser` deepseek_v4(V12 · V13 — 등록명 확인 뒤 넣은 값). 이 값들은 이 계보에서 바꿔 본 적이 없다 — 필요조건으로 복사하지 말고 네 용처에서 다시 고른다. `speculative-config` 의 k=7(V11)과 `enforce-eager` false(V10)는 on/off 를 잰 `tuned` 이지만 k 축은 스윕되지 않았고 1M 에서는 두 노브를 한꺼번에 바꿔 쟀다(Q7).

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **NCCL 전송** → 기동 로그에서 `Using network IB` 줄과 `iova2` · `Cannot allocate memory` · `NV_ERR` 부재를 본다(양 노드 · Ray 가 줄을 접는다 — `[repeated Nx across cluster]`) → IB 면 그대로 · `Using network Socket` 이면 manifest `interconnect.nccl_transport: rdma` 로 재렌더 · 정식 배달 후 재기동(W16 · W17) · `iova2` 가 나면 이 지도를 폐기하고 커널 · 드라이버 조합부터 다시 본다(Q1).
2. **GDR 상태** → 로그 `Connected all rings, use ring PXN 0 GDR 0` 의 GDR 값과 CUDA attr `DMA_BUF_SUPPORTED` · `GPU_DIRECT_RDMA_SUPPORTED` → GDR 0 이면 이 셀과 같은 경로 · GDR 이 켜지면 이 셀의 회귀 미재현 판정이 옮겨지지 않는다 — 재검증(Q3).
3. **KV 경로 · MoE 경로** → 로그 `Using DeepSeek's fp8_ds_mla KV cache format.` · `Using 'HUMMING' Mxfp4 MoE backend.` · `Attempting to override humming GEMM config` 뒤 크래시 없음 → 같으면 그대로 · 상위 vLLM 이 다른 KV 포맷을 허용하면 V6 를 다시 재고, MoE 백엔드(V9)는 이 모델에서 triton 이 거부됐고 auto 는 검증된 적이 없으니 바꿀 때 엔진 로그와 호스트 예산으로 먼저 본다(R1 · R2 · R3 재개조건).
4. **공유 패치가 상위 vLLM 에서도 필요한가** → 01 §1.2 의 트립와이어별 종료 경로를 본다: 40 은 이미 가드됐으면 skip · 예상 블록이 없으면 빌드 중단 · 30 은 이미 완화됐으면 skip · 예상 문자열이 없으면 빌드 중단 · 10 은 오버라이드한 nv_dev 소스의 SM120 심볼만 보므로 vLLM 자체 deepgemm 핀이 SM120 커널을 담게 됐는지는 **감지하지 못한다**(사람이 상류 핀 확인) · qwen4exp 60 · 62 · 64 는 상류 머지를 감지하면 빌드를 멈춘다(그때 패치를 뺀다 — 이 모델의 요구가 아니다) → 필요 없어졌으면 빼고 스모크 · 필요하면 유지.
5. **기동 성공 판정** → health 200 + chat 추론 1회 + 위 1 · 3 의 로그 서명 · (tool call 용처면) 로그 `"auto" tool choice has been enabled.` 와 tools + `tool_choice:"auto"` 요청 1회가 finish `tool_calls` 로 돌아오는지(이 run 은 통과 — testlog_26092914_54_41_hint_v7_라이브E2E_DS4F0731_판정 §2 · simlog `toolcall.txt`) → 셋 다 통과해야 성립. 측정은 이 판정 뒤에만 붙인다(W14).
6. **호스트 메모리 안전 예산** → 예산 선언 없이 기동하지 않는다 · 입력은 노드 총량 · 체크포인트 ÷ TP · KV 클램프 · overhead(이 셀 선언 13312 MiB) · 벤치 상주분 → 기동 중 워치독 트립이 나면 `--down` 으로 먼저 회수하고(W13) 클램프를 줄여(W9 · W10) 선언을 다시 세운다 · 벤치 전 양 노드 페이지캐시 드롭(01 §1.6).

실패 신호별 행선지: 빌드 · 버전 핀 · 공유 패치 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · tool call 플래그 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · NCCL 전송 · 인터커넥트 · 커널 · 노드 · 열 게이트 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-graph-autotool` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26092913-hint-v7-e2e · ds4f0731-1m-spec7-roce · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26092913_hint_v7_e2e__cluster__ds4f0731_1m_spec7_roce |
| work-manifest | docs/_evidence/camp_26092913_hint_v7_e2e__cluster__ds4f0731_1m_spec7_roce.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-09-29T04:52:40Z |
| 승인 발화(전사) | 사용자(발화 전사, plan_26092908 G4) — "모든 안을 승인하고 E2E테스트도 필요하다면 진행해" — G4 셀 발행 사전승인 포함 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:63`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(비저작)`(저작자 `hint 발행 저작 Agent` 아님) · 주장 245건 · 지적 11건 중 고침 11건 · 열림 0 · 2026-09-29T06:38:58Z |
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

**발행 경위(사실).** 이 페이로드는 같은 셀의 **새 판**이다 — 같은 셀 이름으로 2026-09-28 에 잰 앞선 판(v6 형식 캠페인 발행)이 이미 있고, 이 판은 2026-09-29 캠페인 `camp-26092913-hint-v7-e2e` 에서 같은 레시피를 다시 띄워 잰 새 측정을 v7 형식으로 싣는다. 앞선 판과 다른 점은 셋이다. ① 서빙 yaml 의 `enable-auto-tool-choice: true` 가 이 판에서는 **측정 구성의 일부**다(측정 엔진 non-default args · engine_v7 L62) — 앞선 판은 그 줄 없이 재고 측정 뒤에 더했다. ② 플래그를 넣은 구성의 tool call 은 앞선 판도 측정 뒤 재기동(원장 앞선-3)에서 이미 PASS 로 관측했다(testlog_26092808 §6) — 앞선 판이 남긴 빈칸은 그 구성의 **성능**이었고, 이 판이 full ×3 으로 채웠다(02 §2.7 머리). ③ attestation · serve_proof 는 두 판 모두 측정 기동의 스모크 직후 · 측정 전에 쓰였다(앞선 판의 attestation 작성 2026-09-27T23:54:55Z · 앞선-2 창 안). 다른 점은 앞선 판이 '같은 실행인지 미검증' 으로 남긴 것을 이 판은 스모크 로그로 같은 기동이라 판별했다는 것이다(01 §1.1 attestation 범위 줄). 옛 태그는 불변이라 고치지 않는다.
**이 run 문서.** 판정 문서 `testlog_26092914_54_41_hint_v7_라이브E2E_DS4F0731_판정` · 서사 `devlog_26092914_hint_v7_구현_재생_라이브_서사` · 원시 증거 `docs/simlog/26092914_54_41_hint_v7_live_ds4f/`(서빙 로그 · 벤치 로그 · tool call 응답)는 계보 표에 있다 — tool call 결과와 READY 는 02 W19 · 01 §1.4 가 원문 발췌로 인용한다. (저작 중 판정 문서가 명명 규약 밖 이름이라 계보에서 빠졌던 것을 개명으로 바로잡은 뒤 다시 파생한 판이다.)
**의견.** 이 판은 Hermes 류 tool call 용처에 그대로 쓸 수 있는 구성의 측정이다. 앞선 판보다 동시성 1 수치가 낮지만 이 판 자신의 반복 산포 안이라 플래그의 성능 영향으로 읽지 말 것을 권한다(Q4 · 03 §3.5).

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-spec7-graph-autotool` — 꼬리 3토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `spec7` | dspark speculative decoding k=7 켜짐 — 같은 q·len·kv 의 spec off(eager) 셀과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `speculative-config.num_speculative_tokens` = `7` |
| `graph` | cudagraph 실행(enforce-eager false) — 같은 q·len·kv 의 eager 셀과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enforce-eager` = `false` |
| `autotool` | enable-auto-tool-choice 켜짐(tool calling 구성으로 측정) — spec·graph 가 같고 이 플래그 없이 잰 같은 q·len·kv 셀(09-09 대조군 · 09-28 앞선 판)과 가른다 | `output/multi/configs/ds4f0731-1m-spec7-roce.yaml` · 키 `enable-auto-tool-choice` = `true` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 `spec7` · `graph` · `autotool` 세 토큰이다. 결정론부가 같은 q·len·kv(fp8 · 1M · fp8 KV) 셀은 카탈로그에 셋 — 1M spec off · eager 셀, 1M spec k=7 · graph 셀(09-09 대조군), 같은 셀의 앞선 판(09-28) — 이고, spec off · eager 셀과 가르는 노브가 `speculative-config.num_speculative_tokens` 7 과 `enforce-eager` false, spec · graph 가 같은 뒤의 두 셀과 가르는 노브가 `enable-auto-tool-choice` true 다(서빙 설정 대조 · 09-28 앞선 판은 측정 엔진 non-default args 에 이 줄이 없다 — engine_roce 대 engine_v7 · 09-09 대조군은 그 레거시 태그에 실린 트리플렛 `e-1m-kvfp8-e1combo.yaml` 에 이 줄이 없다 — 지금 저장소의 같은 yaml 에는 이 줄이 있으나 파일 mtime 2026-09-09T06:15:31Z 가 그 측정(sweep generated_utc 2026-09-09T03:37:55Z) 뒤라 측정 뒤 추가다).
tool-choice 플래그는 API 층이라 측정 형상(KV · 커널 경로)을 바꾸지 않지만, 발행된 같은 q·len·kv 태그 중 tool call 이 서는 구성으로 측정된 것은 이 셀뿐이고 그 차이를 가를 다른 노브가 없어 꼬리에 넣었다. 후보 `plenone` 은 이 모델에 PLE 가 없어 어느 셀도 가르지 않아 뺐고, RoCE(NCCL 전송)는 서빙 yaml · 셀 env · 러너가 아니라 manifest 에서 파생되는 `.env.interconnect` 에 있어 꼬리 근거 파일 밖이다.
