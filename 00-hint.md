# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-pleresident` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-10T20:23:08Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101104-hint-m1 · q38fn030-nv4-res-262k · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json` — 시점 범위: 작성 2026-10-10T03:50:09Z(output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn030-nv4-res-262k/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(37.63) < 53.7×(1−0.15)=45.65 [E(external_reference)] (ratio 0.701) → 기대 이하
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-NVFP4(ModelOpt mixed)를 GB10 2노드(노드당 GPU 1 · Ray TP=2)에서 vLLM v0.30.0 소스빌드로 서빙했다 — 컨텍스트 262144 · KV auto · PLE resident · MTP k=3 · eager · 동시성 1 decode 37.63 t/s(GuideLLM full · 판정 weak REFUTE · floor 45.65 미달).
가장 비싼 벽은 PLE resident 의 로드 피크 사살(W6 · W7)로, KV 회수(V6) · 배치 억제(V10 · V11) · gpu-memory-utilization 천장 하향(V3)을 함께 적용해 넘었고(KV 계단만 더 내리는 것은 무의미했다), 0.30.0 에서는 빌드의 libdw-dev(W9)와 PLE offload 새 기본값 차단(W10)이 더해졌다.
가장 오해하기 쉬운 것은 판정 기준 E — 외부 2노드 게시값을 손으로 승계한 것이고 KV fp8 구성이라 이 셀(KV auto)과 축이 다르다(M6 · Q1) · REFUTE 는 재검증 신호이지 서빙 실패가 아니며 KV 클램프(V6)는 필요량이 아니라 생존 계단값이다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T04:07:13Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_q38fn030-nv4-res-262k)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.30.0 | sweep_index.meta(output/multi/benchlog/sweep_q38fn030-nv4-res-262k) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.30.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.30.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/q38fn030-nv4-res-262k.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len262144-kvauto-pleresident | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn030-nv4-res-262k.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/multi/configs/q38fn030-nv4-res-262k.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype(output/multi/configs/q38fn030-nv4-res-262k.yaml) · vocab kv[auto]←'auto' |
| 꼬리 `pleresident` | PLE(per-layer embedding) n-gram 테이블을 GPU 메모리에 상주(resident)시킨 셀 — 같은 q·len·kv 의 PLE mmap · offload 셀과 구분하는 PLE 배치 축 | `campaigns/camp-26101104-hint-m1/cells/q38fn030-nv4-res-262k/config.yaml` · 키 `declared_axes.ple_mode` = `resident` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 조건에서만 성립하게 만든 전제**

1. **통합메모리 노드에서 PLE 를 상주시킨다** → PLE n-gram 테이블(47.7GiB · TP 로 반씩)이 가중치와 같은 풀에 들어가 노드당 적재 62.89GiB, 로드 피크가 호스트 워치독의 절대 임계(MemAvailable 10,240MiB)와 경쟁한다 → 별도 host RAM 이 있는 HW 에서는 피크가 GPU 메모리와 host 메모리로 갈려 같은 생사선이 성립하지 않을 것이다(추론). 같은 이유로 UVA CPU offload 는 GB10 에서 메모리를 거두지 못한다는 것이 plan 시점 판단이다 — `testlog_26091304` §3.2 · `plan_26090918` §2.3.
2. **노드당 GPU 1 · 2노드라 TP=2 가 노드 간 RoCE 를 탄다** → 매 층 all-reduce 가 NCCL(IB/RoCE · `NCCL_NET_PLUGIN=spcx`)로 노드를 건너고, 두 노드의 vLLM SHA · torch · driver 가 lockstep 으로 같아야 한다 → 한 노드에 GPU 가 둘 이상이면 같은 TP=2 가 노드 안에서 끝나 통신 경로 · 디코드가 달라질 것이다(추론) — `plan_26090918` §1 · 01 §1.4 측정 env 표.
3. **노드 메모리 총량 대비 가중치 비율이 크다** → 선언 바닥 28,350MiB 는 KV 8GiB · overhead 24,800 을 빼고 남은 몫이고 KV 를 늘리거나 gmu 를 올리면 로드 피크가 그 몫을 먹는다 → 메모리가 큰 노드 · 노드가 더 많은 구성에서는 KV · gmu 를 다시 도출해야 한다 — `testlog_26091304` §3.1 · 01 §1.4 이벤트 #21.
4. **양자화 층이 섞여 있다**(주 experts NVFP4 · PLE FP8 · MTP experts FP8_PB_WO) → MoE 백엔드가 둘로 갈린다(NvFp4 = FLASHINFER_CUTLASS · Fp8 = TRITON · 후자는 GB10 튜닝 config 없음 경고) → 하나로 명시하면 깨졌고(W4), 다른 GPU 에서는 엔진이 고르는 경로가 다를 수 있다 — `devlog_26091001` §결정·시도·폐기 · 측정 엔진 로그.

**(나) 이 형상이 버티는 기전 대 다른 형상**

- PLE resident · KV 20GiB · gmu 0.85(0.29 의 res 셀들) — 이 셀과 KV · gmu · 배치(max-num-seqs · batched) 셋이 다르다 — 로드 중 MemAvailable 이 절대 임계 밑으로 내려가 systemd L1 워치독이 사살(원장 TRIP 관측) · 같은 KV · gmu 의 KV auto 셀은 24셀 캠페인에서 살아 측정됐고 KV fp8 판은 −50MiB 로 사살됐을 만큼 마진이 얇았다 — 확정 — `testlog_26091121` §패턴 최종 확정 · `testlog_26091114` §비-measured 셀 · `sweep_map_26091008_nv4-262k-res`.
- resident · KV 8GiB · gmu 0.85 · 배치 미설정(B1-a · `nv4-bf-262k-res` · KV auto) — 이 셀과는 gmu(0.85)와 배치(미설정) 두 노브가 다르고 KV 크기는 같다(앞 행의 사살 기준점 `nv4-f8-262k-res` 와는 KV 크기 20,480 → 8,192 와 KV dtype fp8 → auto 가 함께 다르다) — KV 에서 뺀 12,288MiB 를 로드 피크가 12,392MiB 되먹어 다시 사살(상쇄량 관측) · "다른 버퍼가 gmu 천장까지 채운다" 는 출처의 해석 — 확정(관측) — `testlog_26091304` §3.2.
- resident · KV 8GiB · 배치 8/2048 · gmu 0.85(B1-b) — 이 셀과 gmu 만 다르다 — +390MiB 로 생존했으나 재현성 없는 마진 — 확정 — 같은 절.
- **이 셀 형상**(resident · KV 8GiB · 배치 8/2048 · gmu 0.80) — 위 셋에서 KV 회수 · 배치 억제 · gmu 하향을 **모두** 적용했다 — 0.29.0rc6 의 같은 노브 형상(B1-c)에서 gmu 0.05 하향이 MemAvailable 5,562MiB 를 열어 최저 16,192MiB(+5,952) · 0.30.0 의 같은 셀 이름 기동 2회(앞선 캠페인 1 · 이 캠페인 1) 모두 사살 0(이 페이로드는 0.30.0 기동의 최저 MemAvailable 을 싣지 않았다) — 확정 — 같은 절 · 01 §1.4 기동 시도 표.
- PLE NVMe mmap(0.29 · 패치 62) — PLE 를 page cache(회수 가능)로 읽는다 — 기동 구간 used 60GB 대 resident 104GB(노드당 약 44GiB 회수 관측)로 262k~1m 11/11 성공 — 확정 — `testlog_26091001` §2 · `testlog_26091121`. 0.30.0 에서는 패치 62 가 parked 라 아직 다시 열리지 않았다(R6).
- resident 가 mmap 보다 빠른 이유(매 토큰 룩업이 NVMe gather 를 타지 않는다) — 대조 실험 없음 — 미결 — `testlog_26091304` §1.
- "resident 는 PLE 를 회수 불가 anonymous 메모리로 든다" 는 plan 의 해석 — 확인 실험 없음 — 미결 — `plan_26091216` §2.3.
- PLE UVA offload(0.30.0 기본값) — 이 셀과 PLE 위치만 다르다 — 통합메모리에서 무의미하다는 판단만 있고 띄워 잰 적은 없다 — 미결 — `plan_26090918` §2.3 · W10.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | stock 0.29.0rc6 에서 NVFP4 mixed 체크포인트의 PLE · MTP experts 가 로드되지 않을 것으로 판독됐다 | 빌드 pre 패치 60-nvfp4-mixed 자체이식(A PLE 라우트 · B1 FP8_PB_WO MoE · B2 MTP 리맵) — 0.30.0 에서는 상류 흡수로 불요 | judgment | testlog_26090921 §판정 스모크 5차 PASS · 0.30.0 은 testlog_26100109 §2 M8 AC3(quantization=modelopt_mixed · 패치 60 없음) |
| 2 | W2 | 스모크 1차가 로드 전에 FAIL | 예산 선언(weights · kv · overhead) + kv-cache-memory-bytes 명시 | arch-invariant | testlog_26090921 §판정 스모크 2차부터 로드 진입 |
| 3 | W3 | 스모크 2차 기동 실패(엔진 fail-loud) | moe-backend 명시를 버렸다(W4 뒤 auto 확정) | judgment | testlog_26090921 §판정 스모크 3차로 진행 |
| 4 | W4 | 스모크 3차 기동 실패 | moe-backend 미설정(auto) — 서빙 yaml 에 moe-backend 키가 없다 | judgment | testlog_26090921 §판정 스모크 5차 PASS(최종 설정 moe-backend 미설정) |
| 5 | W5 | 스모크 4차가 로드 뒤 그래프 캡처 구간에서 호스트 워치독에 사살됐다 | enforce-eager true(캡처 생략) · 선언 overhead 를 이 아치 실측 기반으로 재산정 | arch-scaled | testlog_26090921 §판정 스모크 5차(eager) PASS · testlog_26091001 §3 eager 추가 후 통과 |
| 6 | W6 | NVFP4 PLE resident 셀이 예산 선판정을 통과하고도 로드 중 컨테이너가 사라졌다 | 이 구성으로는 해소하지 못했다 — res 최소조건 캠페인(W7)으로 넘어갔다 | arch-scaled | docs/logs/main/events/2026-09.jsonl 의 TRIP · kill_ack 행(testlog_26091114 · testlog_26091121 표) |
| 7 | W7 | KV 를 8GiB 로 회수한 resident 셀(B1-a)이 여전히 로드 중 사살됐다 | max-num-seqs 8 · max-num-batched-tokens 2048(배치 억제) + gpu-memory-utilization 0.85 → 0.80 + KV 8GiB/노드 | arch-scaled | testlog_26091304 §3.2 B1-c 최저 MemAvailable 16,192MiB(+5,952) · 0.30.0 의 같은 셀 이름 기동 2회(앞선 캠페인 1 · 이 캠페인 1) 모두 사살 0(01 §1.4 기동 시도 표) |
| 8 | W8 | 0.29.0rc6 기준 자체이식 패치 3종이 0.30.0 소스에 적용되지 않는다 | 60 · 62 · 64 를 빌드 컨텍스트 밖 parked 디렉터리로 이동(D3) — 이 셀(resident · KV auto)에는 셋 다 불요 | judgment | testlog_26100109 §2 M7 빌드 2차 양 노드 OK · M8 AC3 패치 60 없이 modelopt_mixed 로딩 |
| 9 | W9 | 0.30.0 source-build 가 양 노드에서 같은 자리에서 실패 | Dockerfile.source-build 템플릿 ccache 설치 줄에 libdw-dev 추가(사람 승인) | arch-locked | testlog_26100109 §2 M7 빌드 2차 양 노드 빌드 OK |
| 10 | W10 | (선제) 0.30.0 기본값으로는 PLE resident 셀이 의도와 다른 offload 형상으로 뜬다고 판독됐다 | 서빙 yaml engram-config {"cpu_offload": false} 명시 | judgment | testlog_26100109 §2 M8 AC3(PLE Qwen4ExpPLEFp8EmbeddingMethod weight_device=cuda:0 pinned=False · EngramConfig(cpu_offload=False)) — 노브를 뺀 음성대조는 없다 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 24셀 중 21셀 void · 컨테이너가 로그를 덜 남기고 사라졌다 | 반증 | testlog_26091109 §5 R4 역채점 5/5 |
| M2 | 셀 1 의 판정 표에 accept_len 이 비어 spec off 로 읽혔다 | 반증 | testlog_26091412 §5.1 verdict 변동 4건 중 4번 |
| M3 | NVFP4 res 가 예산 선판정을 통과하고도 로드 중 사살됐다 | 반증 | plan_26091216 §2.2(이미지 안 vllm/models/qwen4_exp/common/ple.py 판독) |
| M4 | res 사살 시점 회수불가 메모리에서 weights 를 뺀 51,154MiB 가 선언 overhead 24,800 을 크게 넘었다 | 반증 | testlog_26091304 §3.3(res overhead 상수 처방은 틀렸다) · §3.2 B1-a/b/c |
| M5 | 측정 decode 가 루프라인 물리 상한을 넘었다 | 반증 | testlog_26091304 §5 교정 전후 표 · PLE 없는 모델 회귀 0 |
| M6 | 첫 판정이 NEEDS_RUBRIC | 반증 | testlog_26091304 §2 재판정 |
| M7 | 0.30.0 첫 full bench 판정이 explore PASS 로 났다 | 반증 | testlog_26100109 §2 M9 판정(선언 권한) · §3 F4 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.30.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | ced6857afa0ea7b2e3f0846a62e1394e90f15607 | output/multi/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.30.1.dev0+gced6857af.d20261001 | output/multi/benchlog/sweep_q38fn030-nv4-res-262k/lite_engine_head_q38fn030-nv4-res-262k.log:58(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 노드별 상이 — 값을 고르지 않는다: attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json · 노드별 관측) — ⚠ 노드별 상이(main=0.30.1.dev0+gced6857af.d20261001.cu133 · sub=0.30.1.dev0+gced6857af.d20260930.cu133 · 값 = main) · 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log)(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T04:07:13Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.30.0-cu133-aarch64-source | 셀 env(output/multi/envs/.env.q38fn030-nv4-res-262k) IMAGE_TAG |
| 이미지 digest | sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log) · ⚠ 선언 580.173.02 과 다르다(driver_conflict) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log) |
| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: attestation parity.driver(output/multi/benchlog/attestation_q38fn030-nv4-res-262k.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-10T03:50:09Z < 측정 시작 2026-10-10T03:50:30Z — 측정 전(빌드·스모크)의 관측 · 측정을 서빙한 같은 기동의 스모크 직후 관측(판별 근거 = attestation 범위 줄 · 스모크 로그 docs/simlog/26101012_벤치_knob_프리셋_E2E/m1_serve.log) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_q38fn030-nv4-res-262k · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantized_layers 48개(예 model.language_model.layers.0.mlp.experts) | `NVFP4` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| quantized_layers 1개(예 model.language_model.layers.1.ple.ple_embedding.ngram_embedding) | `FP8` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| quantized_layers 1개(예 mtp.layers.0.mlp.experts) | `FP8_PB_WO` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config quantized_layers(modelopt MIXED_PRECISION) |
| 양자화 제외 모듈 292개 | `unquantized(아래 기본 dtype)` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config ignore·exclude_modules |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json dtype |

**이 모델에 필요한 패치**: 대표(main) 측정 이미지 원장 기준 실린 빌드 패치 없음 · **노드 비대칭** — `pre` main(대표) 원장: 이 단계 행 없음 / sub 원장: skipped 50-dsv4-sm12x-port.sh, 55-src-deps-authority.sh · `post` main(대표) 원장: 이 단계 행 없음 / sub 원장: applied 10-deepgemm.sh, 20-triton-kernels.sh, 30-mxfp4-triton-sm121.sh, 40-humming-nvml-gb10.sh(원장 관측 — 필요 판정이 아니다 · 01 §1.1 노드별 원장).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build.** vLLM 0.30.0 이 요구하는 torch 는 `==2.13.0` 이고 "빌드 트랙 | source-build(torch ≥2.11 ABI 벽) · wheel(cu129) 은 ABI 벽으로 불용" 이다(`plan_26100107` §3). 그래서 NGC 26.07(torch 2.13.0a0+9186a08 · 위 표) 위에서 소스빌드한다. 다른 트랙으로 가는 조건은 그 판정 문장에서만 파생한다 — 쓰려는 vLLM 의 prebuilt wheel 이 네 베이스의 torch 와 ABI 가 맞을 때(트랙 판정 스크립트 `resolve_build_track.py` · `resolve_wheel.py` 로 다시 판정)뿐이다.

(2) **vLLM 버전 문자열.** 빌드 입력 ref `v0.30.0`(docker history build-arg — 관측) · 소스 SHA `ced6857afa0ea7b2e3f0846a62e1394e90f15607`(`output/multi/resolved.json` — 해소 선언값이며, attestation 이 양 노드 이미지 안 vLLM SHA 가 같은 값임을 관측했다 · 01 §1.5). 엔진 기동 배너 `0.30.1.dev0+gced6857af.d20261001` 은 setuptools-scm 이 태그 SHA 에 dev 접미 · 빌드 날짜를 붙인 엔진 자기보고이고(`testlog_26100109` §1), wheel 메타는 노드마다 날짜 접미만 다르다(main `d20261001` · sub `d20260930`). 인증서 `vllm_version` 은 인증서가 없어 미관측이고, 벤치 리포트의 `vllm_version 0.30.0` 은 측정 도구가 조립한 메타다. 재현은 `v0.30.0` 또는 위 40자 SHA 로 한다.

(3) **arch-locked 값과 재도출.** `TORCH_CUDA_ARCH=12.1a`(GB10 · build-arg) → 네 GPU 의 compute capability(`nvidia-smi --query-gpu=compute_cap --format=csv`)로 다시 정한다. CPU arch `arm64`(image inspect) → 네 호스트 아키텍처에 맞는 NGC 베이스 태그를 고른다. 드라이버 580.178.04(관측) → 네 호스트의 `nvidia-smi` 값이며 양 노드가 같아야 한다. NGC 베이스 `26.07-py3`(이미지 CUDA 13.3.1 · torch 2.13.0a0+9186a08) → 네 vLLM 이 요구하는 torch 로 `resolve_ngc_tag.py` 를 다시 돌린다(`plan_26100107` §3).

(4) **선언 대 관측.** 드라이버는 manifest 선언 580.173.02 와 attestation 관측 580.178.04 가 다르다 — 서빙 호스트의 실제는 관측값이다(벤치 리포트 · sweep meta 의 `driver_version` 580.173.02 는 manifest 를 옮긴 선언이다). 소스 SHA 는 선언(resolved.json)과 관측(attestation 양 노드)이 일치한다. 이미지 digest 는 측정 시 `docker inspect` 관측값이며, 같은 태그의 2026-10-01 측정 이미지와 다르다(3.5).

(5) **미관측 환경 좌표.** 호스트 OS — 계보 문서에 미기록. 노드 메모리 총량 — 예산 선언 `mem_total_mib=124608`(01 §1.4 이벤트 #22) · plan 의 "노드 물리 121GB"(`plan_26090918` §2.1). 인터커넥트 — RoCE 196.88Gbps(`plan_26090918` §1) · 측정 시 NCCL `network IB`(RoCE 장치 · 01 §1.4 env 표). 호스트 CUDA — 계보의 N1 plan 유효맥락이 "호스트 CUDA 13.0/13.2" 를, 같은 문서 F4 가 "호스트 드라이버 580.178.04(CUDA 13.2)" 를 적는다(`plan_26092311` · 2026-09-23 관측 기록) · 두 벤치 리포트 스냅샷의 `cuda_version 132` 는 manifest 선언(`cuda_version_raw 13.2`)을 옮긴 값이다(선언). 위 표의 CUDA 13.3.1 은 이미지 층이며, 이미지는 CUDA compat 라이브러리로 호스트 드라이버 위에서 돈다(`plan_26092311` F4).

(6) **torch 외 핵심 의존 핀.** `RAY_VERSION=2.48.0`(Dockerfile ARG · 2노드 Ray TP 의 런타임 의존 — 토폴로지 요구이고 공유 이미지에 들어간다) · `flashinfer-python==0.6.18.post1`(requirements — v0.30.0 wheel METADATA 핀 · 이 셀의 NvFp4 MoE 가 `FLASHINFER_CUTLASS` 를 탔다) · `humming-kernels[cu12]==0.1.12` · `nvidia-cutlass-dsl==4.7.1`(같은 METADATA 핀 · 공유 이미지 기본값) · `libdw-dev`(apt · 0.30.0 deepgemm 빌드 요구 — W9). 이 모델 고유의 의존 핀은 없다 — 위는 모두 vLLM 0.30.0 · 멀티 토폴로지의 공유 이미지 요구다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 12개 — `tuned` 4 · `inherited` 6 · `engine-default` 1 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 토폴로지 결정(노드당 GPU 1 × 2노드) — 캠페인 출발부터 TP=2 고정이며 다른 값을 잰 기록이 없다 | V1 |
| `distributed-executor-backend` | ray | inherited | 멀티 서빙 경로(serve_runner.sh 의 Ray head/worker)가 전제하는 값 — 다른 executor 를 시도한 기록 없음 | V2 |
| `gpu-memory-utilization` | 0.80 | tuned | 0.85 → 0.80 으로 바꿔 최저 MemAvailable 이 +390 → +5,952MiB 로 열렸다(B1-c · W7) — 0.80 보다 낮은 값은 재지 않았다 | V3 |
| `max-model-len` | 262144 | inherited | 사람이 고른 이 셀의 통제변인(네이티브 최대 위치) — 512k/1m 은 YaRN 오버라이드가 필요한 다른 셀이고 이 셀에서 조정하지 않았다 | V4 |
| `kv-cache-dtype` | auto | engine-default | auto = 엔진이 고른다(엔진 config kv_cache_dtype=auto · bf16 계산) — fp8 은 0.30.0 에서 패치 64 parked 로 열리지 않았다(R7) | V5 |
| `kv-cache-memory-bytes` | 8589934592 | tuned | 20,480 → 8,192MiB/노드로 바꿔 생사를 쟀다(B1-a) — 262k bf16 필요량 3,072MiB/노드의 약 2.7배로 최적값 측정은 없다 · 더 낮추는 것은 무의미 판정(R4) | V6 |
| `enforce-eager` | true | declared-requirement | W5 — 그래프 캡처 구간이 워치독 임계를 넘어 사살됐다(KV 20GiB · gmu 0.85 · 0.29.0rc6 조건) · 지금 형상에서 캡처가 들어가는지는 미결(Q3) | V7 |
| `async-scheduling` | false | inherited | 커뮤니티 선례의 MTP+async 문맥 손상 함정을 피하는 예방값 — 0.29 는 mtp 와 함께 async 를 자동으로 끄지 않아 명시했다 · 이 계보에서 켜고 잰 기록은 없다 | V8 |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | inherited | 커뮤니티 선례의 MTP k=3 — k ∈ {1,2,3} 레버는 plan 레버 서브스윕에 있었으나 이 res 계열의 레버 지도는 L0-base 1셀만 시도(계획/시도/남음 1/1/0) · k 를 바꿔 잰 기록 없음 | V9 |
| `max-num-seqs` | 8 | tuned | 미설정 → 8 로 배치 억제(B1-b · 실효 544MiB 의 일부)로 사살 −154 → 생존 +390 — 16 이상은 재지 않았다(Q5) · 이 셀 스윕의 레벨 16 을 제외시킨 상한 | V10 |
| `max-num-batched-tokens` | 2048 | tuned | max-num-seqs 와 함께 B1-b 배치 억제로 넣어 생사를 쟀다 — 0.30.0 엔진은 MTP k=3 때문에 max_num_scheduled_tokens 를 2048 로 잡는다고 경고한다 | V11 |
| `engram-config` | {"cpu_offload": false} | inherited | 0.30.0 의 VLLM_PLE_CPU_OFFLOAD 기본 1 을 코드 판독으로 막은 손레버(W10) — 빼고 띄운 음성대조 없음 · 엔진 로그 EngramConfig(cpu_offload=False) 와 PLE weight_device=cuda:0 로 적용은 관측됨 | V12 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 반드시 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V6) · `gpu-memory-utilization`(V3) · `max-num-seqs` · `max-num-batched-tokens`(V10 · V11) · `max-model-len`(V4). KV 는 토큰당 KV(이 모델은 full_attention 12층만 KV 를 만든다 — bf16 클러스터 24KiB/토큰 · `plan_26091216` §2.4) × 목표 컨텍스트로 필요량을 내고, 예산 바닥(`MemTotal − weights − kv − overhead`)이 선언 조건을 넘는지 본 뒤, 실제 로드에서 최저 MemAvailable 을 재서 gmu 를 정한다. 세 값은 B1 에서 **함께** 바뀌어 생존을 만들었다(W7).

② **필요조건으로 기록된 값(declared-requirement)** — `enforce-eager: true`(V7 · 근거 W5: 그래프 캡처 구간에서 워치독 사살 — 단 KV 20GiB · gmu 0.85 · 0.29.0rc6 조건의 관측이라 네 예산에서 그 실패 조건이 성립하는지 먼저 본다). **명시하면 오히려 깨지는 노브**: `moe-backend`(W3 triton = NVFP4 MoE 미지원 · W4 cutlass = MTP experts 경로 가드 충돌 — 설정하지 않는다).

③ **조정된 적 없는 승계값 · 손레버** — `tensor-parallel-size 2` · `distributed-executor-backend ray`(V1 · V2 — 토폴로지가 정한 값) · `async-scheduling false`(V8 — MTP+async 함정을 피하는 예방값) · `speculative-config` MTP k=3(V9 — 커뮤니티 선례 · k 레버 시도 0) · `engram-config` cpu_offload false(V12 — 0.30.0 기본값을 코드 판독으로 막은 손레버 · 음성대조 없음) · `kv-cache-dtype auto`(V5 — 엔진 기본값). 이 값들은 최적이라서가 아니라 바꿔 본 적이 없어서 그 자리에 있다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전과 PLE 경로** → 쓰려는 vLLM 에서 NVFP4 mixed 체크포인트를 패치 없이 로딩하는지(엔진 로그 `quantization=modelopt_mixed` 로 기동) · PLE offload 기본값이 무엇인지(`vllm/envs.py` 의 `VLLM_PLE_CPU_OFFLOAD` · 엔진 로그 `Resolved Engram configuration`) → 그대로(0.30.0 과 같다) / 재도출(기본값 · 키 이름이 바뀌었으면 resident 를 고정하는 방법을 그 버전 소스에서 다시 찾는다) / 0.29 계열이면 이 지도를 쓰지 않는다 — 그 버전은 자체이식 패치 60 이 필요했다(W1).
2. **빌드 입력** → NGC 베이스의 torch 와 vLLM 이 요구하는 torch(`resolve_torch_pin.py` · `resolve_build_track.py`) · deepgemm 의 elfutils 요구(빌드 로그에 `elfutils/libdwfl.h`) → 그대로 / 재도출(트랙 · 베이스 태그 · apt 줄) / 빌드가 다른 의존으로 깨지면 `upstream-version-watch`.
3. **호스트 메모리 안전 예산** → 기동 전에 예산을 선언하고(`multinode_serve_smoke.sh` 가 weights · kv · overhead 로 바닥을 낸다 · 입력 = 체크포인트 크기 ÷ TP(resident 면 PLE 포함) · KV 클램프 · overhead) honored 를 확인한다 → 바닥이 선언 조건을 넘으면 진행 / 못 넘으면 KV · gmu 재도출 / 로드 중 사살이면 블랙박스 원장의 TRIP 행으로 확인하고 gmu 부터 내린다(W7).
4. **기동 성공 판정** → health 200 + 추론 1회(chat `finish_reason=stop`) · 형상 서명(PLE `weight_device=cuda:0` · `EngramConfig(cpu_offload=False)` · `GPU KV cache size`) — 이 셀에는 발화를 확인할 자체 패치가 없다 → 셋 다 보이면 통과 / PLE 가 pinned host 로 뜨면 형상이 다르다(1번으로) / `shm_broadcast` 경고가 이어지는 동안은 hang 으로 보지 않는다(이 셀 12분 36초).
5. **노드 정합(멀티)** → attestation 의 vLLM SHA · torch · driver · 빌드 원장 → 앞의 셋이 다르면 기동하지 않는다 / 빌드 원장만 다르면 차이를 기록하고 진행하되 측정 해석에 남긴다(Q4) / 노드 수 · 노드당 GPU 가 다르면 `serve_runner.sh` 의 GPU 합류 리터럴부터 고친다(1.5).
6. **성능 해석** → 같은 프리셋(1024/256/16 · 반복 3 · GuideLLM)으로 재고, 비교 대상의 강한 키 · 소프트 지문 · 부하 조건을 대조한다(3.5) → 같은 조건이면 비교 / 다르면 나란히 놓지 않는다 / REFUTE 의 원인 추적은 Q1 · Q2 의 다음관측부터.

실패 신호별 행선지: 빌드 · 버전 핀 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-pleresident` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101104-hint-m1 · q38fn030-nv4-res-262k · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101104_hint_m1__cluster__q38fn030_nv4_res_262k |
| work-manifest | docs/_evidence/camp_26101104_hint_m1__cluster__q38fn030_nv4_res_262k.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-10T19:53:02Z |
| 승인 발화(전사) | 사용자(plan 승인 팝업 응답 전사) — "승인" (plan_26101104 · M1 은 현 측정값으로 새 게이트 발행 · 인터뷰 결정) |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(M1 · plan_26101104)`(저작자 `hint 저작 Agent(M1 · plan_26101104)` 아님) · 주장 192건 · 지적 14건 중 고침 14건 · 열림 0 · 2026-10-10T20:57:42Z |
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

- **발행 경위(사실)**: 이 셀은 2026-10-10 벤치 knob 프리셋 E2E 6셀 중 하나로 측정됐고, 사용자 지시("이번 6개 모델에 대한 E2E결과는 각각 힌트 태그 발행해줘")로 셀 단위 발행 대상이 됐다(`plan_26101017_knob_E2E_hint발행` §1). 측정의 목적은 프리셋 계층의 라이브 검증이었고 레시피(트리플렛)는 바꾸지 않았다(`devlog_26101017` 최종 상태).
- **판정과 발행의 관계(사실)**: 이 발행은 성능 판정과 무관하게 지도를 싣는 현행 발행 규칙을 따른다 — REFUTE 는 위 배너와 판정 표면(00 등급 블록 · 03)에 그대로 표시된다. 서빙과 full 측정은 성립했고 판정은 외부 기준 대비 기대 이하라는 표시다.
- **옛 태그와의 관계(사실)**: 같은 모델 · 같은 q·len·kv 의 GB10 클러스터 태그가 카탈로그에 있다 — vLLM 0.29.0 의 PLE resident 판(res 최소조건 캠페인 발행분 · `testlog_26091304`)과 0.29.0 · 0.29.0rc6 의 PLE mmap 판들. 이 페이로드는 그것들을 고치거나 대체하지 않는다 — 0.29.0 resident 판과는 vLLM 버전과 PLE 를 고정하는 방법(0.30.0 은 `engram-config`)이 다르고, mmap 판들과는 PLE 형상이 다르다.
- **의견**: 수신자가 이 지도에서 가장 먼저 가져갈 것은 수치가 아니라 resident 를 살린 세 노브의 조합(V3 · V6 · V10 · V11)과, 0.30.0 에서 resident 를 고정하는 `engram-config`(V12)다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.30.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvauto-pleresident` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `pleresident` | PLE(per-layer embedding) n-gram 테이블을 GPU 메모리에 상주(resident)시킨 셀 — 같은 q·len·kv 의 PLE mmap · offload 셀과 구분하는 PLE 배치 축 | `campaigns/camp-26101104-hint-m1/cells/q38fn030-nv4-res-262k/config.yaml` · 키 `declared_axes.ple_mode` = `resident` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 `pleresident` 하나다(근거: 셀 config `declared_axes.ple_mode` = `resident`). 결정론부(vLLM 0.30.0 포함)가 같은 태그는 아직 없다 — 이 토큰은 지금 있는 형제를 가르기보다, 이 모델 계보에서 생사와 속도를 갈라 온 PLE 배치 축(resident · mmap · 0.30.0 의 offload 기본값)을 이름에 남겨 같은 q·len·kv 의 mmap · offload 셀이 생기면 구분되게 한다. 카탈로그에는 같은 모델 · NVFP4 · 262144 · KV auto 의 GB10 클러스터 resident 태그(vLLM 0.29.0 판)와 mmap 태그(0.29.0 · 0.29.0rc6 판)가 있다.
후보 중 `spec3`(MTP k=3) · `eager` 는 이 계보의 GB10 클러스터 셀들이 모두 같은 값이라 PLE 배치처럼 셀을 가르는 축이 아니어서 쓰지 않았다.
