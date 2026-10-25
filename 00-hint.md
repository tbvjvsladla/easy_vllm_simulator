# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-25T04:10:00Z |
| 캠페인 · 셀 · 노드 · 모드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-nv4-1m-tp2 · cluster · publication-replay |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json` — 시점 범위: 작성 2026-10-25T02:16:28Z(output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측 |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `REFUTE` — 출처 output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict |

> **REFUTE** — 이 셀의 성능 판정은 **기각**이다(측정은 됐으나 판정 기준 미달). 수치는 관측이지 baseline · 권고가 아니다 — 판정 출처 output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/verdict.json(output · verdict_rule 출력) verdict.
> 기각 사유: M(26.01) < 32.5×(1−0.15)=27.62 [E(external_reference)] (ratio 0.8) → 기대 이하
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-NVFP4 를 GB10 2노드(노드당 GPU 1 · Ray TP=2)에서 vLLM v0.31.0 stock 소스빌드로 max-model-len 1048576(YaRN f4) · KV fp8_e4m3 · PLE resident · spec off · eager 로 서빙했고, 판정점 동시성 1 decode 26.01 t/s(GuideLLM × 반복 3)는 E 32.5 대비 REFUTE(floor 27.62 · 표시만)다.
가장 비싼 벽은 0.31.0 이 YaRN 최대 길이를 factor 로 다시 늘리지 않아 1M 요청이 ValidationError 로 거부된 것이고 hf-overrides 에 max_position_embeddings 를 함께 실어 넘었다(W8 · V12) — 이전 vLLM 계보의 resident 1M 로드 사살(W6)은 KV · gmu · 배치를 다시 잡은 이 형상에서 TRIP 0 이었다.
가장 오해하기 쉬운 값은 KV 클램프와 max-num-seqs 1(V6 · V9 — 스윕 없이 손으로 고른 승계값)이고, E 32.5 는 출처 · 조건이 기록되지 않은 값이고 같은 값의 1노드 PLE mmap 수치로 보여(추론) like-with-like 로 볼 근거가 없다(03 §3.5 · 02 Q1).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T02:27:13Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/q38fn31-nv4-1m-tp2.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len1048576-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(자동 파생 — 측정 TP=2(sweep_index.meta.tensor_parallel_size(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · generated_utc = 측정 키)) > gpus_per_node=1(output/multi/manifest.yaml(manifest_contract 로더)) · 단일 노드 형상일 수 없다 · 일치 신호: 멀티 serve proof output/multi/benchlog/serve_proof_q38fn31-nv4-1m-tp2.json(kind=multinode_serve_proof) · 노드 정합 attestation output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json) · sweep meta gpu_model 일치 · target: output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/q38fn31-nv4-1m-tp2.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/q38fn31-nv4-1m-tp2.yaml) · vocab kv[fp8]←'fp8_e4m3' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만든 전제**

1. **통합메모리 + PLE 상주** — GB10 은 GPU 와 CPU 가 한 메모리 풀이라 PLE(47.68 GiB F8)를 CPU 로 내려도 거두는 것이 없고, 0.31.0 stock 에는 디스크 mmap 경로가 없다 → 이 셀은 PLE 를 장치에 상주시킨 채 노드당 61.93 GiB 를 올리고 KV · overhead 를 그 나머지에 맞췄다 → 전용 VRAM 이 분리된 HW 나 PLE mmap 이 있는 빌드에서는 예산 배분이 완전히 달라진다(testlog_26102420 §3 · testlog_26102406 §근거 4 · R2 · R3).
2. **노드당 GPU 1 · TP=2 가 노드 간 RoCE 를 탄다** — 모든 TP all-reduce 가 RoCE(NCCL `IB` · `GPU Direct RDMA Disabled` 줄이 서브 머리 캡처 lite_engine_head L172 ~ L182 와 메인 꼬리 캡처 lite_engine L570 ~ L574 에 있고 링 연결 줄은 `GDR 0` — 서브 L388 · 메인 L637)를 지나고 master 는 GPU 합류 리터럴 `2` 를 기다린다 → 노드 수 · 노드당 GPU 가 다르면 러너 리터럴 · NCCL env · 통신 지연이 모두 달라진다(01 §1.5 · plan_26090918 §1).
3. **노드 메모리 대비 가중치 비율** — 체크포인트 123.53 GiB 의 절반 남짓을 노드마다 상주시키고 남은 예산이 선언 floor 22206 MiB 다(선언값 · 02 §2.5) → 같은 체크포인트의 FP8 변종은 같은 조건에서 기동 전에 차단됐고(arm 상한 −6,064 MiB) 싱글 노드는 가중치만으로 넘쳤다 → 메모리가 큰 노드에서는 KV · gmu · 동시성 여유가 생기고, 작은 노드에서는 이 형상이 성립하지 않는다(testlog_26102414_q38fn_fp8 §근거 · R1).

**(나) 이 형상이 버티는 기전 대 다른 형상이 실패한 이유**

- 0.29.0rc6 NVFP4 resident 1M(`nv4-bf-1m-res` · `nv4-f8-1m-res`) — 그 셀들의 트리플렛(`output/multi/configs/nv4-{bf,f8}-1m-res.yaml` · 계보 표 밖 파일)과 비교하면 vLLM 0.29.0rc6 → 0.31.0 · gmu 0.85 → 0.80 · KV 21474836480(20 GiB) → 14 GiB · MTP k=3 → 없음 · max-num-seqs · max-num-batched-tokens 미설정(엔진 기본값) → 1 · 2048 이 다르다(enforce-eager 는 둘 다 true) — 그 셀은 로드 중 서브 MemAvailable 이 memwatch 절대 문턱(10,240 MiB) 아래로 내려가 사살됐고 이 셀은 TRIP 0 이다 · 어느 변경이 로드 피크를 낮췄는지는 대조가 없다 — 미결 — testlog_26091121 §비-measured 셀 · testlog_26102414 §1.
- 0.29.0rc6 NVFP4 resident 262k(KV 20,480 → 8,192 MiB · 배치 억제 · gmu 0.85 → 0.80) — 관측: KV 에서 회수한 12,288 MiB 를 로드 피크가 12,392 MiB 되먹었고 gmu 0.85 → 0.80 이 MemAvailable 최저 여유를 +5,952 MiB 로 열었다 — 관측은 확정(262k 셀 4단 대조) · 그 기전(vLLM 이 gmu 천장까지 쓴다 · 총량 cap)은 가설(강등)이다(총량 cap 을 거는 코드가 소스에 없다는 2026-09-14 정정 — vllm-recipe-explorer `references/kv-clamp.md` §1 · budget_preflight.py 머리 주석) · 1M 에 같은 관측이 성립하는지는 미결 — testlog_26091304 §3.2.
- FP8 resident 1M(0.31.0 자매 셀) — 가중치가 노드당 88,464 MiB(PLE 상주 포함)로 크다 — 예산 산식에서 arm 상한이 음수라 로드 0초에 차단 — 확정(산술 선판정 · 기동 대조 없음) — testlog_26102414_q38fn_fp8 §근거.
- 싱글 TP=1 NVFP4 1M(0.31.0) — 노드 수 1 — 가중치만으로 단일 노드 예산 초과 — 확정(산술 사전판정 · 기동 시도 0) — testlog_26102406 §근거.
- NVFP4 PLE mmap 형상(0.31.0 런타임 플러그인 · 다른 셀) — PLE 백엔드 · 셀 env · 런타임 패치가 다르다 — 테이블을 재료화하지 않아 상주 메모리가 줄었다고 계보 밖 문서가 적는다 · 이 셀과 같은 조건의 성능 대조는 계보 안에 없다 — 미결 — testlog_26102420 §3 · 01 §1.2.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 0.29.0rc6 stock 에서 NVFP4 혼합 체크포인트의 PLE 양자화 방식이 None 이 되어 로드 거부가 예상됨 | 0.29 계보는 빌드 패치 60(PLE mixed 라우트 + MTP 디스패치 + 리맵) · 0.31.0 은 stock 이 처리(이 셀 패치 0) | judgment | testlog_26090921 §판정 스모크 5차 PASS · 이 셀 측정 기동 lite_engine L631 Initialized PLE embedding ... Qwen4ExpPLEFp8EmbeddingMethod |
| 2 | W2 | P0-1 스모크 1차가 기동 전에 차단됨 | kv-cache-memory-bytes 절대 클램프 + 예산 선언(budget_declare) 후 진입 | arch-invariant | testlog_26090921 §판정 스모크 5차 PASS · 이 셀 원장 docs/logs/main/events/2026-10.jsonl 643행 budget_declare 와 watchdog.jsonl budget_honored |
| 3 | W3 | P0-1 스모크 2차가 MoE 백엔드 검사에서 fail-loud 로 종료 | moe-backend 를 명시하지 않음(auto) — 이 셀의 엔진은 FLASHINFER_CUTLASS 를 자동 선택 | arch-scaled | testlog_26090921 §판정 최종 설정 moe-backend 미설정(auto) · 이 셀 lite_engine L520 Using 'FLASHINFER_CUTLASS' NvFp4 MoE backend |
| 4 | W4 | P0-1 스모크 3차가 MoE 백엔드 가드 충돌로 종료 | moe-backend 를 명시하지 않음(auto) | judgment | testlog_26090921 §판정 스모크 5차 PASS(최종 설정 moe-backend 미설정) |
| 5 | W5 | P0-1 스모크 4차 중 cudagraph 캡처 구간에서 컨테이너가 워치독에 사살됨 | enforce-eager true 로 cudagraph 캡처 제거 | arch-scaled | testlog_26090921 §판정 스모크 5차(eager) PASS |
| 6 | W6 | 0.29.0rc6 의 NVFP4 · PLE resident · 1M 셀 2개가 로드 중 서브 노드에서 사살됨 | 이 셀은 KV fp8 14 GiB · gmu 0.80 · max-num-seqs 1 · max-num-batched-tokens 2048 로 재확정(설정 차이 전부는 02 §2.5) | arch-scaled | testlog_26102414 §1 선례 대비 TRIP 0 · 이 셀 원장 시도 5 사살 칸 없음(메인 원장 기준) |
| 7 | W7 | 멀티 스모크가 서브 통로 미완결로 기동 전에 멈춤 | sync_to_sub.sh --branch multi(dry-run 후 apply · 승인 앵커 plan_26102412) | judgment | testlog_26102414 §1 3차 health 200 · 스모크 rc=0 |
| 8 | W8 | 기동 인자 검증에서 max_model_len 1048576 이 derived 262144 보다 크다며 ValidationError 로 종료 | hf-overrides text_config 에 max_position_embeddings 1048576 을 rope_parameters(YaRN factor 4 · original 262144)와 함께 실음 | arch-invariant | 측정 기동 lite_engine_head L62 non-default args 의 max_position_embeddings 1048576 · L72 Using max model len 1048576 · testlog_26102414 §2 needle 1,001,130 PASS |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 하네스 교정 뒤 NVFP4 · PLE resident 셀 5건이 동일 floor 위에서 전부 워치독에 사살됨 | 반증 | testlog_26091304 §3.2(KV 회수가 피크에 먹힘 · gmu 0.85 → 0.80 이 5,562 MiB 를 엶) · perf_26091122 §4 정정 박스 |
| M2 | 1M 셀 needle 첫 시도가 FAIL | 반증 | testlog_26102414 §2 needle 표 |
| M3 | yaml 에 async-scheduling false 를 두었으나 엔진 non-default args 에 그 키가 없다 | 반증 | 측정 기동 lite_engine_head L62 non-default args 에 async_scheduling 없음 |
| M4 | 판정 testlog 가 전 셀의 bench 예산을 2048 MiB 로 기재 | 반증 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json L346-352 · budget_preflight.py 출력 bench_ok true |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | 빌드 원장 vllm.git_sha(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · measured build-time in-image · describe=v0.31.0 · parity.vllm_sha=equal — resolved.json 미바인딩) |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/lite_engine_head_q38fn31-nv4-1m-tp2.log:58(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · 노드별 관측) — main=sub 일치 · 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T02:27:13Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/multi/envs/.env.q38fn31-nv4-1m-tp2) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) |
| ⚠ 드라이버 관측 ≠ 선언 | 충돌 없음 — 관측 580.178.04 = 선언 580.178.04 | 관측: attestation parity.driver(output/multi/benchlog/attestation_q38fn31-nv4-1m-tp2.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 작성 2026-10-25T02:16:28Z < 측정 시작 2026-10-25T02:16:49Z — 측정 전(빌드·스모크)의 관측 · 원장 순서로 판별한 같은 기동의 관측(스모크 로그 미보존 · 약한 판별)(판별 근거 = attestation 범위 줄 · 원장 docs/logs/main/events/2026-10.jsonl) · 선언: sweep_index.meta.driver_version(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
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

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙** — source-build 다(위 표 트랙 행 · 출처 빌드 원장 `dockerfile`). 트랙 이유는 두 자리에 적혀 있다: 실린 Dockerfile 머리 주석(L29 ~ L32 — prebuilt wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막히므로 `_C` 를 NGC torch 에 맞춰 직접 컴파일한다)과 0.31.0 bump 서사(계보 밖 devlog_26101412 — "0.31.0 은 torch 핀이 0.30.0 과 같아 NGC 26.07 위 source-build 그대로"). 이 이미지는 그 bump 때 single-node 체크아웃의 `output/single/Dockerfile.source-build` 로 지어졌고 빌드 소요는 main 40분(01:08:32 → 01:48:59Z) · sub 37분이다(testlog_26101412 §1 · §2 P4). 첫 빌드는 v0.31.0 이 `use_existing_torch.py` 를 `tools/` 로 옮긴 탓에 `can't open file` 로 죽었고, 두 경로를 고르는 자기적응 단계로 고쳐 2차에 통과했다 — 실린 Dockerfile 에도 그 단계가 있다(L82 ~ L84). 다른 트랙으로 갈 조건은 기록이 없다.

(2) **vLLM 버전 문자열** — 넷이다. 빌드 입력 ref `v0.31.0` 은 docker history build-arg `VLLM_REF`(셀 env 도 같은 값)이고 **재현 좌표는 이것 또는 40자 SHA `db9527a46873454610df6dbedf79a36d6bf1a7f6`**(빌드 원장 `vllm.git_sha` · 양 노드 equal)다. 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 와 wheel 메타 `0.31.1.dev0+gdb9527a46.d20261014.cu133` 은 소스 트리가 빌드 때 스스로 붙인 개발 버전 문자열이라 빌드 입력이 아니다(`gdb9527a46` 은 위 SHA 의 앞부분이다). 측정 강식별 키 `vllm_version` 0.31.0 은 측정 도구가 sweep meta 에 적은 값이다 — 인증서는 없다(위 표 `absent`).

(3) **arch-locked 값** — `TORCH_CUDA_ARCH=12.1a`(build-arg · 네 GPU 의 compute capability 로 다시 정한다 — `nvidia-smi --query-gpu=compute_cap` 등) · CPU arch `arm64`(image inspect · 네 호스트 `uname -m` 과 맞는 NGC 베이스를 고른다) · 드라이버 580.178.04(호스트 층 · 이미지 CUDA 13.3 과 맞는 드라이버 계열을 네 호스트에서 확인한다) · NGC 베이스 `26.07`(이미지 CUDA 13.3.1 · torch 2.13.0a0 을 함께 결정한다).

(4) **선언 대 관측** — 이 셀은 `resolved.json` 을 이 빌드에 묶지 못했다(결손 `HINT_MISSING_RESOLVE`) — torch · CUDA · NGC 는 image inspect 관측값만 있고 비교할 선언값이 없다. 실린 레시피는 single-node 개정 `12ef50a89b86:output/single/` 의 바이트다 — 이 개정은 이미지 Created(2026-10-14T01:45:19Z) 뒤인 03:07:26Z 커밋이고, 빌드 때 입력은 계보 밖 testlog_26101412 유효맥락이 'single-node 체크아웃(`689610f` + 미커밋 렌더)' 으로 적는다. 그 가운데 requirements.txt 는 빌드 원장 sha256 과 일치하고(verified), Dockerfile 은 docker history 와 16/17 만 맞는다(가드 echo 한 단계 차이 · generated-unverified · 01 §1.1 · §1.2). 서빙 이미지의 실제는 image inspect · 빌드 원장 · 이미지 안 pip-freeze(관측)다.

(5) **미관측 환경 좌표** — 호스트 OS 는 이 계보에서 찾지 못했다(**미기록**). 노드당 메모리 총량은 `MemTotal ≈121 GiB`(testlog_26102406 유효맥락)이고 원장 선언 `mem_total_mib 124608` 은 선언값이다. 인터커넥트는 `output/multi/manifest.yaml`(측정 전 스캔)이 `type: RoCE v2` · `bandwidth_gbps: 218.33`(`bandwidth_source: measured:ib_write_bw-injected`)으로 기록하고 이 측정의 판정기도 그 값을 썼다(roofline.json `interconnect_gbps` 218.33) — plan_26090918 §1 의 `196.88Gbps` 는 2026-09-09 의 옛 기재다. 측정 실행 NCCL 은 `IB` 경로다(01 §1.4 env 표). 드라이버는 manifest 가 양 노드 `580.178.04` 로 선언했고 attestation 관측과 같다(위 표 드라이버 행 · 충돌 없음). 호스트 좌표와 이미지 좌표는 다른 층이다 — 스냅샷의 `cuda_version 132` 는 manifest 의 호스트 층 값(서브 `cuda_version_raw: 13.2` · 드라이버가 노출하는 CUDA)이고 이미지 CUDA 13.3.1 과 다르다.

(6) **핵심 의존 핀** — Ray 2.48.0(build-arg `RAY_VERSION` · Dockerfile 머리 주석 "멀티 0.22.1 검증치" — 공유 이미지의 분산 서빙 기본값이지 이 모델의 요구가 아니다) · NCCL 2.30.7+cuda13.3(측정 env 관측 · 엔진이 자기보고) · 이미지 안 설치 목록 `/opt/easy-vllm/pip-freeze.txt`(관측 · attestation `build_ledger.pip_freeze_sha256` 과 같은 파일)은 `flashinfer-python==0.7.0.post1` · `transformers==5.14.1` · `compressed-tensors==0.17.0` · `humming-kernels==0.1.16` · `ray==2.48.0` 을 적는다(측정 엔진 autotune 캐시 경로 `flashinfer_autotune_cache/0.7.0.post1` · lite_engine L710 와도 맞는다). 실린 requirements.txt(원장 sha256 일치)도 `flashinfer-python==0.7.0.post1` · `humming-kernels[cu12]==0.1.16` 을 적고, transformers 는 `<5.18.0,>=5.10.4,<5.15.0` constraint 다. 이 모델 전용 핀은 없다 — 전부 공유 이미지의 의존이다. 이 모델이 실제로 쓴 커널 경로는 FlashInfer CUTLASS NVFP4 MoE · FlashInfer GDN prefill · FlashAttention 2 다(01 §1.4 (f)).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 12개 — `inherited` 10 · `engine-default` 1 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 싱글 TP=1 은 산술 사전판정으로 기동 없이 배제됐다(R1 · 관측된 실패 아님) · 멀티 캠페인 plan 이 TP=2 로 정했고 다른 값 시도 0 | V1 |
| `distributed-executor-backend` | ray | inherited | 멀티 브랜치 분산 서빙의 표준 실행기(서브는 Ray 워커) · 다른 실행기 시도 0 | V2 |
| `gpu-memory-utilization` | 0.80 | inherited | 0.29 의 262k resident 셀에서 0.85 → 0.80 으로 낮춰 로드 피크 여유를 연 값의 승계 · 이 셀에서 바꿔 잰 기록 없음 · 이 셀 엔진은 KV 절대 클램프 때문에 KV 산정에 이 값을 쓰지 않는다고 기록(lite_engine L685 · L687) | V3 |
| `max-model-len` | 1048576 | inherited | 캠페인 목표값(지원 최대 context · YaRN f4) — 기동 검증 1,001,130 토큰 needle PASS · 다른 길이로 바꿔 잰 기록 없음 | V4 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 첫 plan 이 1M 을 fp8 KV 전제로 계획한 값 · 0.29 1M resident 사살(W6)은 auto 와 fp8 둘 다에서 났으므로 dtype 이 그 실패를 가른 것은 아니다 · 이 셀에서 auto 로 잰 기록 없음 | V5 |
| `kv-cache-memory-bytes` | 15032385536 | inherited | 손레버 — 0.29 의 KV 20 GiB 사살 선례보다 줄여 14 GiB 로 한 번 정했고 스윕 0 · 엔진 용량은 1M 요청 2.01개분(max-num-seqs 1 대비 약 2배 · 음성대조 기록 없음) | V6 |
| `enforce-eager` | true | inherited | 0.29 계보 스모크 4차의 cudagraph 캡처 피크 사살(W5 · MTP k=3 · gmu 0.85 · KV 20 GiB 조건)에서 승계 · 이 셀 조건에서 cudagraph 시도 0 | V7 |
| `async-scheduling` | false | engine-default | yaml 의 false 가 엔진 non-default args 에 실리지 않았다 — 실효값은 엔진 기본값 규칙으로 켜짐이다(M3 · resolve 값을 찍는 줄은 없고 비활성 경고도 없다) | V8 |
| `max-num-seqs` | 1 | inherited | 손레버 — 고른 이유는 계보에 기록이 없고 스윕 0 · 이 값 때문에 full 스윕 동시성 레벨 2 이상이 걸러졌다 | V9 |
| `max-num-batched-tokens` | 2048 | inherited | 0.29 res 후속 캠페인 확정 조건(262k)의 값 승계 · 이 셀에서 바꿔 잰 기록 없음 | V10 |
| `engram-config` | {"cpu_offload": false} | inherited | PinnedHost offload 는 GB10 통합메모리에서 절감 0 이라는 메모리 구조 판단(R3) · 켜서 실패한 관측은 없다 · 측정 기동에서 PLE 는 장치 상주(weight_device=cuda:0 · pinned=False) | V11 |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | declared-requirement | W8 — max_position_embeddings 없이 rope_parameters 만 실은 기동이 ValidationError(derived 262144)로 종료됐다 | V12 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 반드시 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V6) · `gpu-memory-utilization`(V3) · `max-model-len`(V4) · `max-num-seqs`(V9) · `max-num-batched-tokens`(V10). 절차: 먼저 `budget_preflight.py` 에 네 노드의 MemTotal · 상주 가중치(체크포인트 ÷ TP · PLE resident 면 PLE 포함) · KV · overhead 를 넣어 floor · arm 상한이 선언 가능한지 본다 → 기동 뒤 엔진 로그의 `GPU KV cache size: … tokens, Maximum concurrency for … tokens per request` 줄로 목표 컨텍스트 × 동시 요청 수가 들어가는지 확인한다(이 셀은 14 GiB 에서 1M 요청 2.01개분). 로드 피크가 문턱에 닿으면 KV 계단보다 gmu 천장 · 배치 레버를 함께 본다(R5).

**② 필요조건으로 확인된 값(declared-requirement)** — `hf-overrides`(V12) 하나다. 0.31.0 에서 `text_config.max_position_embeddings` 없이 YaRN rope_parameters 만 실으면 기동 인자 검증이 derived 262144 로 1M 을 거부한다(W8). **명시하면 깨질 수 있는 노브**: `moe-backend` — 0.29.0rc6 + 빌드 패치 60 조건에서 triton(NVFP4 MoE 미지원 fail-loud)과 cutlass(패치 B1 가드 충돌)가 깨졌다(W3 · W4). 0.31.0 에서는 명시해 본 기록이 없고 이 셀 엔진의 후보 목록에는 `VLLM_CUTLASS` 도 있다(lite_engine L520) — 0.31 에서 깨진다는 처방이 아니라, 이 셀은 비워 두어 엔진이 FLASHINFER_CUTLASS 를 골랐다는 사실만 확인된 것이다. 길이 검사를 `VLLM_ALLOW_LONG_MAX_MODEL_LEN` 으로 끄는 것도 이 계보는 쓰지 않았다(R4).

**③ 조정된 적 없는 승계값** — `tensor-parallel-size 2` · `distributed-executor-backend ray`(V1 · V2 — 싱글이 산술로 배제돼 정해진 형상) · `kv-cache-dtype fp8_e4m3`(V5 — 1M 의 계획 전제) · `kv-cache-memory-bytes`(V6 — 손레버 · 스윕 0) · `enforce-eager true`(V7 — 다른 조건의 캡처 피크 사살에서 승계 · 이 조건에서 cudagraph 시도 0) · `max-num-seqs 1`(V9 — 손레버 · 고른 이유 미기록 · 동시성 곡선을 지웠다) · `max-num-batched-tokens 2048`(V10) · `engram-config cpu_offload false`(V11 — 통합메모리 판단) · `gpu-memory-utilization 0.80`(V3 — 262k 셀에서 효과가 확정된 값의 승계 · 이 셀 엔진은 KV 산정에 쓰지 않는다). 이 값들은 최적이라는 뜻이 아니다 — 이 셀에서 한 번 성립한 조합일 뿐이다. `async-scheduling false`(V8)는 엔진 인자로 넘어가지 않은 줄이라 지위가 engine-default 이고, 실효값은 비동기 스케줄링 **켜짐**이다(M3). 음성대조로 일부러 둔 값은 없다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **대상 vLLM 의 YaRN 최대 길이 의미** → 확인: 같은 hf-overrides 로 기동해 기동 인자 줄(`non-default args`)과 `Using max model len 1048576` 이 찍히는지, `ValidationError … derived max_model_len` 가 나오는지 본다 → 그대로(줄이 찍히고 기동) / 재도출(거부되면 그 버전의 `_get_and_verify_max_len` 이 YaRN factor 를 어떻게 다루는지 소스를 읽고 max_position_embeddings 오버라이드 필요 여부를 다시 정한다 · W8) / 폐기(rope_parameters 형식 자체가 바뀌었으면 이 hf-overrides 는 쓰지 않는다).
2. **체크포인트 PLE · MoE 가 stock 으로 로드되는가** → 확인: 엔진 로그 `Initialized PLE embedding … quantization_method=…` 와 `NvFp4 MoE backend out of potential backends` 줄 → 그대로(이 셀처럼 PLE FP8 방식 · MoE auto 선택) / 재도출(PLE 양자화 방식이 None 이거나 로드 실패면 0.29 계보의 W1 처럼 패치가 필요하다 → `upstream-version-watch`) / 폐기 없음. 이 셀은 자체 패치가 0 이라 상위 버전에서 "패치가 여전히 필요한가" 를 따질 대상은 없다.
3. **호스트 메모리 안전 예산** → 확인: `budget_preflight.py --mem-total-mib … --weights-mib … --kv-mib … --overhead-mib …` 로 `declare_ok` · `arm_ceiling_mib` 를 보고 선언 뒤 기동한다(입력 = 노드 MemTotal · 상주 가중치 · KV 클램프 · overhead 선언 · 이 셀 floor 22206 MiB) → 그대로(선언 가능) / 재도출(arm 상한이 최소 미달이면 KV · overhead 를 다시 정하거나 PLE mmap 셀로 간다) / 폐기(가중치만으로 노드 예산을 넘으면 이 형상은 그 HW 에서 성립하지 않는다 — R1 · FP8 자매 셀).
4. **기동 성공 판정** → 확인: health 200 + 추론 1회(스모크 serve proof) · 장문은 증분 프리필 needle(thinking off · temperature 0 — M2)로 네이티브 262,144 를 넘는 길이까지 · 기동 중 `shm_broadcast` 60초 경고는 KV 예약 · `Starting vLLM server` 줄까지 기다린다(이 셀 23분 23초) → 그대로 / 재도출(장문에서 검색이 깨지면 YaRN 적용 여부부터 · 02 Q4) / 폐기(1M 근방에서 검색이 유지되지 않으면 이 컨텍스트 주장을 버린다).
5. **노드 · 인터커넥트** → 확인: 노드당 GPU · 노드 수가 이 셀(1 × 2)과 같은지 · `serve_runner.sh` 의 GPU 합류 리터럴(`2`) · 양 노드 vLLM SHA · torch · driver 일치(attestation) · NCCL `nccl:network` 줄 → 그대로 / 재도출(노드 형상이 다르면 러너 리터럴 · TP · NCCL env 를 다시 정한다) / 폐기 없음.
6. **성능 기대** → 확인: 같은 GuideLLM 프리셋으로 판정점을 재고 like-with-like 외부 기준(노드 수 · PLE 방식 · spec 상태 · 컨텍스트가 같은 것)을 찾는다 → 이 셀의 26.01 t/s 와 REFUTE 는 출처 · 조건이 기록되지 않은 E(값이 같은 1노드 mmap 수치로 보인다 · 추론) 대비 표시라 그대로 옮기지 않는다(03 §3.5).

실패 신호별 행선지: 빌드 · 버전 핀 · PLE/MoE 로드 실패 → `upstream-version-watch` · 서빙 설정 · KV · YaRN 오버라이드 · 노브 → `vllm-recipe-explorer` · 성능 기대 이하(REFUTE 의 원인 분리 · Q1) → `adversarial-benchmark` · 노드 · 서브 배달 · 인터커넥트(W7 류) → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | 캠페인 밖(`_bootstrap` · output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-nv4-1m-tp2 · cluster |
| 발행 모드 | publication-replay |
| 발행 토픽 | q38fn31_nv4_1m_tp2 |
| work-manifest | docs/_evidence/q38fn31_nv4_1m_tp2.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-25T04:00:00Z |
| 승인 발화(전사) | 사용자(발화 요지): 5셀 hint 발행 제안(Qwen FP8·NVFP4 멀티, Solar 멀티, Gemma 싱글, Qwen NVFP4 싱글 mmap)에 '오케이 힌트 태그도 발행해줘' — 태그 push 는 발행 직전 재확인 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 520건 · 지적 15건 중 고침 15건 · 열림 0 · 2026-10-25T13:46:14Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다(output/multi/benchlog/sweep_q38fn31-nv4-1m-tp2/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · 같은 셀의 앞선 캠페인 인스턴스 campaigns/camp-26102412-multi-sol-q38/cells/q38fn-nv4-1m-tp2(journey 마지막 campaign_closed) 는 보존돼 있으나 이 측정의 것이 아니라 읽지 않았다 |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. | — |
| `HINT_MISSING_SWEEP_LEVELS` | 부하 레벨이 1개 이하 — 부하 거동을 알 수 없다(경량 리포트는 동시성 곡선을 재지 않는다). | — |
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
**확정 이름** `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len1048576-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

빈 꼬리(`inputs/tail.json` = `[]`)다. 결정론부가 같은 자매 셀은 PLE mmap 동치 시험 셀 `q38fn31-nv4-mmap-1m-tp2` 인데, 그 셀을 가르는 키(`VLLM_PLE_MMAP=1` · `RUNTIME_PATCH_STEM` 등)는 그 셀의 env 에만 있고 이 셀은 그 키가 **없는** 것으로 resident 가 된다 — 이 셀의 서빙 yaml · 셀 env · 러너에는 `pleresident` 를 근거 지을 키 · 값이 없어(yaml 머리의 `PLE=resident` 는 주석이다) 그 후보를 쓰지 않는다. `specoff` 도 speculative-config 키의 부재에서 나온 후보라 근거 키가 없고, `eager`(`enforce-eager: true`)는 근거 키가 있지만 자매 셀도 같은 값이라 이 셀을 가르지 않는다.
