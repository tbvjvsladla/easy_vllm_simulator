# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g1n-main-native/qnvfp4-len1048576-kvfp8-plemmap` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-25T04:10:00Z |
| 캠페인 · 셀 · 노드 · 모드 | 캠페인 밖(`_bootstrap` · output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-nv4-mmap-1m-tp1 · main · publication-replay |
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
| 자격 근거 | `output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/level_01/post_health_q38fn31-nv4-mmap-1m-tp1.json` · `output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/level_01/guidellm_q38fn31-nv4-mmap-1m-tp1.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3.8-Flash-Next-NVFP4(nvidia)를 GB10 1대(싱글 TP=1) · vLLM `v0.31.0` 소스빌드 공유 이미지에서 max-model-len 1048576(YaRN factor 4) · KV fp8_e4m3 · eager · spec decode 없음 · **PLE 테이블을 디스크 mmap 으로 읽는 런타임 플러그인**으로 서빙했고, GuideLLM 동시성 1 decode 16.72 t/s 는 roofline 하한 6.7 대비 weak PASS 다(외부 기준 E 는 이 형상의 물리상한을 넘어 무효 · M2).
가장 비싼 벽은 PLE 상주 싱글의 산술 사전판정 불가(W3)와 빌드 패치 62 의 앵커 전멸(W5)이며, stock 임베딩 클래스를 바꾸는 플러그인 `q38fn31-nv4-mmap-1m-tp1_patch.py`(`VLLM_PLE_MMAP=1` · 노드 로컬 스테이징 · 이미지 재빌드 없음)와 single 하네스 교정(W6 · 로드-전 RAM 게이트)으로 넘었다.
가장 오해하기 쉬운 값: `max-num-seqs: 1`(V7)이 부하 곡선을 한 점으로 만들었고 KV 클램프(V4)는 다른 셀 실측에서 옮긴 손계산이며, yaml 노브 중 필요조건은 `hf-overrides`(V10) 하나다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T01:22:24Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len1048576-kvfp8-plemmap | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 밖 측정(측정 시점 ACTIVE=_bootstrap · 이 측정의 캠페인 인스턴스 없음) 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/single/configs/q38fn31-nv4-mmap-1m-tp1.yaml) · vocab kv[fp8]←'fp8_e4m3' |
| 꼬리 `plemmap` | PLE n-gram 테이블을 상주 대신 로컬 디스크 mmap 으로 읽는 런타임 플러그인 활성(같은 HW 의 stock 상주 싱글 형상은 산술 사전판정 불가) | `output/single/envs/.env.q38fn31-nv4-mmap-1m-tp1` · 키 `VLLM_PLE_MMAP` = `1` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

(가) **이 조건에서만 성립하는 것**
- **통합메모리(호스트 RAM = GPU 메모리) 단일 노드** → `v0.31.0` stock 의 PLE 백엔드 둘(Device · PinnedHost)은 47.68 GiB 테이블을 모두 같은 풀에 재료화하므로, 체크포인트 123.53 GiB 가 노드 예산 121.7 GiB 를 넘는 싱글 TP=1 은 상주로는 불가이고 디스크 mmap 만 길을 연다. 호스트 RAM 이 GPU 메모리와 분리된 HW 에서는 PinnedHost 가 GPU 메모리를 덜 쓸 수 있어 이 플러그인의 이유가 약해진다(추론 · 계보에 대조 없음). 근거 `testlog_26102406_q38fn_nv4_싱글_사전판정_불가_멀티이관` §근거 · `testlog_26102420_PLE_mmap_0.31.0_재이식_S1_선판정_런타임패치_슬롯_정정` §3.
- **노드 메모리 대비 가중치 비율이 빠듯하다** → mmap 형상에서도 엔진 모델 적재가 74.14 GiB 이고 KV 15.5 GiB 를 더하면 엔진 예약 합 89.64 GiB 다(03 §3.1). 예산 선언(선언값)은 weights 77705 · KV 15872 · overhead 18432 MiB 로 바닥 12599 MiB 이고, 이 바닥 때문에 벤치 몫이 2359 MiB 이하로 묶였다(02 W7). lite 측정 시 시스템 RAM 은 101.4 GiB(83%)였다(`bench_report_26102510_qwen3.8-flash-next-nvfp4_GB10_0.31.0` §lite 지표). 노드 메모리가 더 작으면 이 형상도 다시 재야 하고, 크면 상주 형상(R1)이 열릴 수 있다.
- **노드당 GPU 1 · 노드 1 · TP=1** → 플러그인의 rank 마스크 · all-reduce 는 타지 않고(`_reduce` 항등), 조회마다 호스트에서 행을 모아 장치로 올리는 동기화만 남는다(eager 전용 이유 · 번들 `backend.py`). CUDA graph · TP 가 다르면 디코드 비용이 달라진다(크기는 미측정 · 02 Q1 · Q2).
- **가중치가 NAS(CIFS)에 있고 체크포인트가 가용 RAM 보다 크다** → 엔진이 auto-prefetch 를 껐다. 로그가 적는 조건은 둘이다 — CIFS 가 인식된 네트워크 FS(NFS/Lustre)가 아니고, 그리고 체크포인트 123.57 GiB 가 가용 RAM 34.44 GiB 의 90% 를 넘는다(측정 엔진 로그 L99). 가중치 적재는 521.92 s 였다(L114). 적재 시간이 FS 때문이라는 것은 대조가 없는 **추론**이다(같은 체크포인트를 다른 FS 에서 읽은 기록이 계보에 없다). PLE 스테이징은 노드 로컬 디스크다(01 §1.6 (b)).

(나) **이 형상이 버티는 기전 대 다른 형상**
- PLE 상주(stock Device · `cpu_offload false`) — 테이블을 프로세스 텐서로 둔다 — 체크포인트 전량이 단일 노드 예산을 넘는 산술로 기동 없이 불가 — 산술은 확정 · 실기동 거동은 미결 — `testlog_26102406_…` §근거 · 02 R1.
- PinnedHost(`cpu_offload true`) — 테이블을 pinned 호스트 텐서로 둔다 — 통합메모리라 같은 풀을 써 절감 0 — 미결(소스 판독 · 기동 대조 없음) — `testlog_26102420_…` §3 · 02 R2.
- 디스크 mmap(이 셀) — 테이블은 로컬 디스크 파일 페이지로 남고 요청 행만 읽는다 — 엔진 적재 74.14 GiB · 예산 선언 수리 · 원장 사살 0 · health 200 + 추론 · full 3/3 으로 버텼다 — 버틴 사실은 확정(엔진 로그 · 원장 · 측정)이다. 페이지 캐시(회수 가능) 대 상주 anonymous(회수 불가)가 로드 피크 완충을 가른다는 기전은 이전 vLLM 멀티 NVFP4 셀의 MemAvail 산술 대조에 근거하고(`perf_26091305_qwen38fn_res최소조건_후속캠페인` §3), 이 셀에서는 미검증이다 — 02 W5.
- 멀티 TP=2 상주(같은 이미지 자매 셀) — 노드 2대로 나눠 PLE 를 상주시킨다 — 서빙됐다 · 이 셀과 토폴로지가 달라 이 셀의 기전 근거로 쓰지 않는다 — `testlog_26102414_q38fn_nv4_멀티_TP2_1M_YaRN_기동_needle_lite_판정`.
- 형상을 연 설정 변경 전부(사전판정 → 이 셀): 런타임 패치 파일 신설 · 셀 env `VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR=/app/ple_mmap/q38fn-nvfp4` · `VLLM_PLE_MMAP_VERIFY=16` · `RUNTIME_PATCH_STEM` · single compose `/app/ple_mmap` 볼륨과 manifest `ple_mmap_host_path`(W6) · 로드-전 RAM 게이트의 PLE mmap 선언 보정(커밋 20c7690 · 02 §2.2 블록 없는 항목) · yaml `max_position_embeddings` 동시 오버라이드(W4). 사전판정 쪽은 기동 설정이 없어 노브 전수 대조는 할 수 없다 — 02 §2.5.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 이전 vLLM(0.29.0rc6) stock 의 PLE 양자화 메서드 선택이 FP8 전용이라 NVFP4 mixed 체크포인트 PLE 를 거부할 것으로 판독(실패 관측 아님) | 이전 vLLM — 빌드 패치 60-nvfp4-mixed(PLE FP8 라우트 + PR#55513 이식 + MTP 리맵) · v0.31.0 — 패치 없이 stock 이 PLE · NVFP4 MoE 를 적재(이 셀 이미지는 빌드 패치 0 · spec off 라 MTP 경로는 미관측) | judgment | 이 셀 측정 기동 엔진 로그 Initialized PLE embedding … quantization_method=Qwen4ExpPLEFp8EmbeddingMethod(lite_engine L94) · Model loading took 74.14 GiB(L116) |
| 2 | W2 | YaRN 확장 길이를 선언한 셀이 serve 인자에 rope 오버라이드 없이 기동되어 max_model_len 검증에서 종료(관측 1셀) | 교정 R8 — hf-overrides 에 rope_parameters(mrope_interleaved · mrope_section · partial_rotary_factor 0.25 · rope_theta · rope_type yarn · factor · original_max_position_embeddings 262144) | arch-invariant | testlog_26091121_qwen38fn_22셀재수행_캠페인완주_판정 §R8 실서빙 재검증 — factor=4 완결 |
| 3 | W3 | NVFP4 싱글 TP=1 셀을 기동 없이 사전 불가 판정(체크포인트가 노드 예산 초과 · estimate 18 후보 전부 FAIL) | PLE 테이블을 디스크 mmap 으로 읽는 런타임 플러그인(W5) — 예산 weights 를 PLE 제외값(77705 MiB · 선언)으로 낮춰 선언 수리 | arch-scaled | 원장 budget_honored floor_mib 12599 · arm_ceiling_mib 9527(docs/logs/main/events/watchdog.jsonl) · 측정 기동 Model loading took 74.14 GiB(lite_engine L116) · health 200 + 추론 1회 |
| 4 | W4 | rope_parameters 만 오버라이드한 1M 요청이 v0.31.0 에서 ValidationError(derived 길이 262144) — 자매 셀(멀티 TP=2 NVFP4) 관측 | hf-overrides text_config 에 max_position_embeddings=1048576 을 rope_parameters 와 함께 싣는다(우회 env 미사용) | arch-invariant | 이 셀 측정 기동 엔진 로그 Using max model len 1048576(lite_engine L54) · GPU KV cache size 1,237,563 tokens · 1,048,576 tokens per request 기준 1.18x(L130) |
| 5 | W5 | 이전 vLLM 의 PLE mmap 빌드 패치 62 를 v0.31.0 소스에 얹을 수 없음(소스 판독 · 기동 없음) | out-of-tree 플러그인 qwen_ple_mmap_plugin 0.1.0(q38fn31-nv4-mmap-1m-tp1_patch.py) — register() 가 Device/PinnedHost 전역을 Qwen4ExpPLEMmapEmbedding 으로 교체 · VLLM_PLE_MMAP=1 일 때만 · 런타임 패치 슬롯 · 이미지 재빌드 없음 | arch-locked | 측정 기동 엔진 로그 PLE mmap backend installed(lite_engine L75) · 128 shards in 1 files(L93) · Model loading took 74.14 GiB(L116) · verify OK(L128 · L141 · L198~L211) · FAILED 0줄 |
| 6 | W6 | single 통로에서 PLE mmap 스테이징을 컨테이너에 넣을 볼륨이 없음(single 템플릿 · manifest 필드 부재) | single compose 템플릿에 ${PLE_MMAP_HOST_PATH}:/app/ple_mmap:ro 추가 · manifest ple_mmap_host_path 기입(커밋 20c7690) | judgment | 측정 기동 엔진 로그 dir=/app/ple_mmap/q38fn-nvfp4 에서 128 shards in 1 files(lite_engine L93) |
| 7 | W7 | full 벤치 진입 시 bench 예산 4096 MiB 가 벤치 진입 게이트에 막힘(벤치 시작 안 함) | bench 예산을 2048 MiB 로 선언(sweep_index bench_knobs.bench_budget_mib 2048) — 서빙 형상 · KV 는 바꾸지 않음 | arch-scaled | level 1 반복 3/3 완주 · server_alive_at_bench_end true(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | full 벤치 진입 시 bench 예산 4096 MiB 가 막힘 | 반증 | run_bench.sh@3fa6eac00a18 L378-382(게이트 호출 · rc 5 정지) · sweep_index bench_knobs.bench_budget_mib 2048 |
| M2 | Qwen 싱글 TP=1 full 판정에서 외부 기준 E 가 이 형상의 물리상한을 넘어 판정 불성립 | 확증 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/verdict.json(e_search empty · rubric.physical.limit_tps 22.52 · exceeds false) |
| M3 | NVFP4 싱글 TP=1 셀을 void 처분 · 재시도 안 함으로 닫음 | 반증 | 이 셀 health 200 + 추론 1회(level_01 post_health · guidellm json) · full 반복 3/3 · 측정 기동 엔진 로그 Model loading took 74.14 GiB(lite_engine L116) |
| M4 | yaml 에 async-scheduling false 를 두었으나 엔진 non-default args 에 그 키가 없다 | 반증 | 측정 기동 엔진 로그 non-default args(lite_engine L43)에 async_scheduling 부재 · 비활성 경고 0건 · 이미지 소스 scheduler.py:209 · api_utils.py:208-218 · vllm.py:1576-1635(저작 때 판독) |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) · 관측: 빌드 원장 describe `v0.31.0` · 업스트림 델타 from_ref `v0.30.0`(이 빌드가 올라온 이전 릴리스) | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.describe(git describe --tags · 빌드 시점) · output/single/resolved.json upstream_delta.from_ref(해소 기록) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/lite_engine_head_q38fn31-nv4-mmap-1m-tp1.log:39(이 측정의 엔진 로그 기동 배너) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-25T01:22:24Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.q38fn31-nv4-mmap-1m-tp1) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 관측 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
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

(1) **빌드 트랙** — source-build 다(위 표 · 선택자 `Dockerfile.source-build`). 트랙 판정 사유는 빌드 레시피 머리 주석이 적는다 — torch 2.11+ 구간에서는 prebuilt vLLM wheel 의 `_C` 가 NGC alpha torch(이 이미지는 위 표의 2.13.0a0+9186a08)와 C++ ABI 가 맞지 않아 import 단계에서 막히므로, `_C` 를 그 torch 에 맞춰 직접 컴파일한다(`artifacts/build_recipe/Dockerfile.source-build` L29-32). 그 문장에서 파생하면 wheel 트랙으로 갈 조건은 "그 wheel 의 `_C` 가 베이스 torch 와 ABI 가 맞을 때" 이고, 최종 중재는 어느 트랙이든 스모크다. 이 셀은 이미지를 새로 짓지 않았다(공유 이미지 · PLE mmap 은 런타임 패치).

(2) **vLLM 버전 문자열** — 빌드 입력은 docker history build-arg `VLLM_REF=v0.31.0` 이다(위 표). 엔진 기동 배너 `0.31.1.dev0+gdb9527a46.d20261014` 는 빌드된 소스 트리에서 매겨진 개발 버전 문자열이고 생산자는 엔진 기동 배너다(lite_engine_head L39 · `dev0` · `d<날짜>` 꼬리의 뜻은 문자열 모양에서 읽은 해석). 0.2 의 `vllm_version` 0.31.0 은 측정 도구의 스윕 meta 값이다. 인증서 `vllm_version` 은 인증서가 없어 비었다. 설치 배포판 메타는 측정 이미지 안 빌드 원장 `/opt/easy-vllm/build_ledger.json`(01 §1.1 이 적용 증거로 쓰는 그 원장)의 `vllm` 블록에 기록돼 있다 — `dist_version 0.31.1.dev0+gdb9527a46.d20261014.cu133` · `git_sha db9527a46873454610df6dbedf79a36d6bf1a7f6` · `describe v0.31.0` · editable install(`file:///workspace/vllm-src`)(저작 때 시작하지 않는 컨테이너 create · cp 로 판독). 생산자는 빌드 시점의 원장이고, 엔진 배너와 같은 문자열에 `.cu133` 이 붙은 형태다. 그 `git_sha` 는 아래 40자 SHA 와 같다. **재현 좌표는 릴리스 태그 `v0.31.0`** 이고, 40자로는 `db9527a46873454610df6dbedf79a36d6bf1a7f6`(위 표 `vLLM 소스 SHA(40자)` · `output/single/resolved.json` `upstream_delta.to_sha`)이다. 배너의 `gdb9527a46` 은 그 SHA 의 앞부분이다.

(3) **arch-locked 값과 재도출**
- `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 compute capability) → 네 GPU 의 compute capability 로 다시 정한다.
- CPU arch arm64(이미지 · 베이스 `nvcr.io/nvidia/pytorch:26.07-py3` 의 arm64 판) → 네 호스트 `uname -m` 에 맞는 베이스를 고른다.
- 드라이버 580.178.04 계열(호스트 · 선언) → 네 드라이버가 이미지 CUDA 13.3 을 받는지 확인한다.
- 플러그인이 찾는 PLE 샤드 레이아웃(NVFP4 판 = 1파일 128샤드) → 네 체크포인트의 safetensors 헤더로 다시 확인한다(01 §1.6 (b)).
- 통합메모리 전제(PinnedHost 무의미 · 예산 산식의 MemTotal = 디바이스 풀) → 분리 메모리 HW 에서는 그 전제부터 다시 판단한다(R2).

(4) **선언 대 관측** — `resolved.json` 의 NGC 베이스는 이미지 `NVIDIA_PYTORCH_VERSION` 관측과 접두 일치한다(위 표 NGC 행). 실린 Dockerfile 은 작업트리 판이고 이미지 docker history 와 16/17 일치라 `generated-unverified` 다(가드 echo 한 단계 차이 · 01 §1.1 · §1.2). `ARG VLLM_REF` 기본값 v0.31.0 과 docker history build-arg `VLLM_REF=v0.31.0` 은 같다. `requirements.txt` 는 빌드 원장 sha256 일치로 `verified` 다. 서빙 이미지의 실제는 관측 쪽(docker image inspect · 빌드 원장)이다.

(5) **사실 블록이 비운 환경 좌표** — 호스트 OS 는 계보에 **미기록**이다. 노드 메모리의 직접 증거는 예산 원장의 `mem_total_mib 124608` 이다 — 러너가 기동 때 이 노드의 `/proc/meminfo` MemTotal 을 MiB 로 내림해 넘긴 값이다(`single_serve_up.sh:229`). `plan_26091108_qwen38fn_22셀_재수행_캠페인` §2 의 `MemTotal 124,610 MiB`(121.7 GiB · 다른 캠페인 기록)와의 2 MiB 차이는 반올림 · 시점 차이로 보인다(추론). 사전판정 문서는 이 노드를 "MemTotal ≈121 GiB 통합메모리" 로 적는다(`testlog_26102406_…` 유효맥락). 인터커넥트는 해당 없음(단일 노드). CUDA 는 층이 다르다 — 스윕 meta `cuda_version 132` 는 manifest 선언을 옮긴 호스트 층 값이고, 이미지 층은 CUDA 13.3.1.008 · torch 2.13.0a0+9186a08 · NGC 26.07 이다(위 표). 둘을 섞지 않는다. 드라이버의 노드별 관측은 없다(위 표 ⚠ 행).

(6) **핵심 의존 핀** — 관측된 것: Ray `RAY_VERSION=2.48.0`(build-arg · 공유 이미지의 멀티용 기본값 · 이 싱글 셀은 Ray 를 쓰지 않는다) · FlashInfer 0.7.0.post1(엔진 autotune 캐시 경로 `flashinfer_autotune_cache/0.7.0.post1/121a/…` · lite_engine L135 — 실린 `requirements.txt` L20 `flashinfer-python==0.7.0.post1` 과 같다) · 측정 클라이언트 GuideLLM 0.7.3(이미지 digest 기록). `requirements.txt` 는 원장 sha256 일치라 그 핀이 이 이미지가 받은 값이다 — `compressed-tensors==0.17.0`(L14) · `transformers<5.18.0,>=5.10.4,<5.15.0`(L73) 등. 이것들은 공유 이미지의 기본 의존이지 이 모델의 요구가 아니다. 이 모델의 요구는 런타임 플러그인 하나(`qwen_ple_mmap_plugin` 0.1.0 · numpy · torch · vllm 은 이미지 것을 import)다. 이미지 `/opt/easy-vllm/pip-freeze.txt`(빌드 원장이 sha256 으로 묶은 파일 · 저작 때 create · cp 로 판독)는 Triton 을 `triton-3.7.1+gitf797708c.nv26.7`(+ `triton_kernels 1.0.0+gitf797708c.nv26.7` · `nv26.7` 꼬리라 NGC 베이스가 넣은 판으로 보인다 · 추론)로 적고, DeepGEMM 은 별도 배포본으로 적지 않는다. 같은 파일은 transformers 실제 설치판을 5.14.1 로 적는다(requirements 의 범위 안).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 10개 — `inherited` 9 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.80 | inherited | 멀티 셀들과 같은 값 · 이 셀에서 바꿔 잰 기록 없음 · KV 절대 클램프가 있어 엔진이 프로파일링을 건너뛰고 gmu 를 KV 산정에 쓰지 않는다고 기록(lite_engine L129) | V1 |
| `max-model-len` | 1048576 | inherited | 셀의 목표 길이(사람이 정한 셀 축 · YaRN f4 로 연다) · 길이를 바꿔 잰 스윕 없음 · 엔진 Using max model len 1048576(lite_engine L54) | V2 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 멀티 1M 셀들에서 승계(1M 은 fp8 KV 전제로 계획됨) · 이 셀에서 bf16 과 대조한 적 없음 · v0.31.0 stock 이 그대로 받음(lite_engine L61) | V3 |
| `kv-cache-memory-bytes` | 16642998272 | inherited | 손레버 — yaml 주석이 다른 셀(멀티 FP8) 실측 7,625 B/token/GPU 를 TP1 로 옮겨 계산 · 줄이거나 키워 본 시도 없음 · 엔진은 1,237,563 토큰(1M 요청 하나 기준 1.18x)을 잡음(lite_engine L130) · 음성대조 아님 | V4 |
| `enforce-eager` | true | inherited | 플러그인 v1 제약 — 번들 backend.py 가 enforce_eager 아니면 생성자에서 RuntimeError(소스 · 이 셀에서 graph 기동 실패를 관측한 기록 없음 · 그래서 declared-requirement 로 올리지 않음) | V5 |
| `async-scheduling` | false | inherited | 이전 vLLM 의 MTP+async 회피 조치에서 승계 · 엔진 non-default args 에 실리지 않음 — 값은 기본값 None 이고 엔진 규칙상 async on 으로 돌았다(무효 줄 · M4 · 소스 판독 + 로그 부재) | V6 |
| `max-num-seqs` | 1 | inherited | 1M 요청 하나를 목표로 한 셀 선언 · 바꿔 잰 기록 없음 · 이 값 때문에 full 벤치 동시성 2·4·8·16 이 걸러져 레벨 1개(sweep_index bench_knobs.levels.filtered_out) | V7 |
| `max-num-batched-tokens` | 2048 | inherited | 손레버 — 근거 문서 없이 둔 값 · 스윕 없음 · 엔진 FlashInfer autotune 이 2048 토큰 버킷까지 돎(lite_engine L138) | V8 |
| `engram-config` | {"cpu_offload": false} | inherited | PLE 상주 셀(Device 백엔드 선택)에서 승계 · VLLM_PLE_MMAP=1 이면 플러그인이 Device · PinnedHost 전역을 모두 교체해 이 값과 무관(번들 install 주석) · 엔진 Resolved Engram configuration cpu_offload=False(lite_engine L63) | V9 |
| `hf-overrides` | {"text_config":{"max_position_embeddings":1048576,"rope_parameters":{"mrope_interleaved":true,"mrope_section":[11,11,10],"partial_rotary_factor":0.25,"rope_theta":10000000,"rope_type":"yarn","factor":4,"original_max_position_embeddings":262144}}} | declared-requirement | W4 — max_position_embeddings 없이 rope_parameters 만 두면 v0.31.0 이 1M 을 ValidationError 로 거부(같은 모델 · 같은 이미지의 멀티 NVFP4 셀에서 관측 · 이 셀에서는 관측 안 함) · W2 — rope 오버라이드 자체가 없으면 262144 초과 길이가 거부(이전 vLLM 관측) | V10 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **네 환경에서 다시 도출할 값(arch-scaled)** — `kv-cache-memory-bytes`(V4) · `gpu-memory-utilization`(V1) · `max-num-seqs`(V7) · `max-model-len`(V2) · `max-num-batched-tokens`(V8). 도출 절차:
- 노드 가중치는 엔진 `Model loading took … GiB` 실측으로 잡는다(이 형상은 PLE 를 빼고 센다 · 이 셀 74.14 GiB).
- KV 는 엔진 `GPU KV cache size: N tokens` 를 클램프 바이트로 나눈 토큰당 바이트 × 목표 컨텍스트 × 동시 시퀀스로 잡는다 — yaml 주석의 계산은 다른 셀 값이라 쓰지 않는다(V4).
- 남는 호스트 메모리를 예산 선언으로 확인하고, 벤치 몫은 `floor − bench ≥ abs_band` 로 따로 확인한다(W7).

이 셀의 클램프는 1M 요청 하나를 1.18배로 덮는 값이다(V4).

② **필요조건으로 확인된 값(declared-requirement)** — `hf-overrides`(V10) 하나다. rope 오버라이드가 없으면 262144 초과 길이가 거부됐고(W2 · 이전 vLLM 멀티 셀), `max_position_embeddings` 를 빼면 `v0.31.0` 에서 1M 이 ValidationError 였다(W4 · 같은 이미지 멀티 NVFP4 셀). 둘 다 이 셀에서는 관측되지 않았다. yaml 노브는 아니지만 빼면 형상이 무너지는 것은 셀 env 의 플러그인 활성 줄이다(`VLLM_PLE_MMAP=1` · `VLLM_PLE_MMAP_DIR` — 빼면 W3 의 상주 형상). **설정하지 않아야 하는 것**: CUDA graph(`enforce-eager` 해제) — 플러그인 v1 이 생성자에서 `RuntimeError` 로 막는다(번들 소스 · 실기동 관측 없음 · V5). **효과가 없는 것**: yaml 의 `async-scheduling: false` — 엔진에 닿지 않았고 측정은 async on 상태다(M4 · V6 · 소스 판독 + 로그 부재) · PLE mmap 활성 시 `engram-config` 의 `cpu_offload`(플러그인이 두 백엔드를 다 교체 · V9).

③ **조정된 적 없는 승계값(inherited)** — `kv-cache-dtype` fp8_e4m3(V3) · `enforce-eager`(V5 · 플러그인 제약) · `async-scheduling`(V6) · `engram-config`(V9). ① 의 값들도 지위는 전부 inherited 다(이 셀 스윕 0). "이 값으로 한 번 떴다" 는 뜻이지 최적이라는 뜻이 아니다. `kv-cache-memory-bytes`(V4) · `max-num-batched-tokens`(V8)는 손레버이고, `max-num-seqs: 1`(V7)은 1M 단일 요청 셀 선언이라 부하 곡선을 한 점으로 만들었다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **대상 vLLM 의 PLE 임베딩 구조가 이 플러그인과 맞는가** — 확인 방법: 대상 버전 소스(또는 이미지 안 `/workspace/vllm-src`)에서 `vllm/models/qwen4_exp/common/ngram_embedding.py` 의 `Qwen4ExpPLEEmbedding` 과 `vllm/models/qwen4_exp/nvidia/ngram_embedding.py` 의 모듈 전역 `Qwen4ExpPLEDeviceEmbedding` · `Qwen4ExpPLEPinnedHostEmbedding` 을 grep 하고, 디스크 기반 백엔드(`VLLM_PLE_MMAP` 류)가 새로 생겼는지 본다(S1 판정 방법 · W5 · 02 Q6). 분기:
   - 디스크 기반 백엔드가 정식으로 있으면 → 플러그인을 **빼고** 정식 경로로 레시피를 다시 세운다.
   - ABC · 전역 이름이 그대로면 → 플러그인을 그대로 시도하되 스모크로 중재한다.
   - 이름 · 생성자가 바뀌었으면 → 번들(01 §1.2 해제 명령)을 그 API 로 다시 이식한다(지도는 참고로만).
2. **노드 메모리가 PLE 상주를 덮는가** — 확인 방법: 예산 선판정(`budget_preflight.py`)에 노드 MemTotal · `(ckpt − PLE×[mmap]) ÷ tp` · KV · overhead 를 넣는다(체크포인트 크기는 index `total_size`). 분기: 상주(PLE 포함)로도 바닥이 양수면 → mmap 이 필요 없을 수 있다(R1 재개조건 · 상주로 재도출). 통합메모리이고 상주가 음수면 → mmap 형상. 호스트 RAM · GPU 메모리가 분리된 HW 면 → stock PinnedHost 도 후보(R2 재개조건).
3. **스테이징이 노드 로컬 디스크에 있고 NVFP4 판을 가리키는가** — 확인 방법: `<manifest.ple_mmap_host_path>/q38fn-nvfp4/` 에 PLE 샤드가 든 safetensors 가 있는지, 원본과 `cmp` 되는지, manifest `ple_mmap_host_path` 와 셀 env `VLLM_PLE_MMAP_DIR` 이 맞는지 본다(01 §1.6 (b)). 분기: 맞으면 그대로 · 경로가 비거나 다른 판이면 기동하지 않고 고친다.
4. **기동 성공 판정** — health 200 + 추론 1회 + 엔진 로그 `PLE mmap backend installed` + `128 shards in 1 files … dir=<스테이징>` + `verify OK`(`VLLM_PLE_MMAP_VERIFY>0` 일 때) · `verify FAILED` 0줄. 분기: 설치 줄이 없으면 → `[arm_patch] runtime patch armed` 줄과 패치 파일 이름(`CONFIG_FILE` 과 같은 stem)을 확인한다(01 §1.2) · FAILED 가 있으면 → 이 지도 폐기(행 저장소 불일치).
5. **호스트 메모리 예산 선언** — 확인 방법: 러너로 기동해 원장 `budget_honored` 를 확인한다. 입력은 PLE 제외 weights · KV 클램프 · overhead(이 셀 18432 MiB 는 출처 미기록 손선언 — 네 노드에서 로드 뒤 `(MemTotal − MemAvailable) − weights − kv` 로 다시 잰다) · MemTotal. 벤치 전에 `budget_preflight.py --floor-mib <floor> --bench-budget-mib <몫>` 으로 `floor − bench ≥ abs_band` 를 따로 확인한다(W7). 분기: 선언이 거부되면 → KV 를 줄이거나 형상을 바꾼다 · 가드를 우회하지 않는다.
6. **1M 길이** — 확인 방법: 엔진 `Using max model len 1048576` 과 `non-default args` 의 `hf_overrides`(max_position_embeddings + rope_parameters)를 본다 · `/v1/models` 의 `max_model_len` · 증분 프리필 needle. 분기: ValidationError 면 → W4 대로 `max_position_embeddings` 를 함께 오버라이드한다(우회 env ✗ · R4).

실패 신호별 행선지:
- 빌드 · 버전 핀 · 상위 버전의 PLE 구조 → `upstream-version-watch`
- 서빙 설정 · KV · 노브 · 런타임 패치 내용 · 스테이징 → `vllm-recipe-explorer`
- 성능 기대 이하 · 판정 기준(E) 선택 → `adversarial-benchmark`
- 노드 · 토폴로지 · 호스트 메모리 가드 → `terraforming_node`

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g1n-main-native/qnvfp4-len1048576-kvfp8-plemmap` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | 캠페인 밖(`_bootstrap` · output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) · q38fn31-nv4-mmap-1m-tp1 · main |
| 발행 모드 | publication-replay |
| 발행 토픽 | q38fn31_nv4_mmap_1m_tp1 |
| work-manifest | docs/_evidence/q38fn31_nv4_mmap_1m_tp1.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-25T04:00:00Z |
| 승인 발화(전사) | 사용자(발화 요지): 5셀 hint 발행 제안(Qwen FP8·NVFP4 멀티, Solar 멀티, Gemma 싱글, Qwen NVFP4 싱글 mmap)에 '오케이 힌트 태그도 발행해줘' — 태그 push 는 발행 직전 재확인 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 360건 · 지적 15건 중 고침 15건 · 열림 0 · 2026-10-25T15:11:59Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 캠페인 밖 측정 — purge 된 인스턴스가 아니라 이 측정의 인스턴스는 애초에 없다(output/single/benchlog/sweep_q38fn31-nv4-mmap-1m-tp1/sweep_index.json bench_knobs.campaign = null · campaign_pins_note '활성 캠페인 없음(_bootstrap)'(측정 시점 ACTIVE 해소 · repeat_axis)) |
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

- 발행 절차의 사정(사실): 이 페이로드는 **발행 기록 재생**(publication-replay)이고, 측정 자체가 캠페인 밖(`_bootstrap`)이라 캠페인 인스턴스가 애초에 없다. 그래서 셀 lockset · config · 캠페인 선언은 실리지 않았고 값의 지위는 문서 서사와 측정 산출물에서 복원했다(00 결손 `HINT_MISSING_CAMPAIGN_INSTANCE`).
- 계보 읽기 목록 밖 문서(사실): 이 셀의 S2~S4 판정(플러그인 구현 · 1M needle 단계 · S3 lite · READY 시간 · 메모리 사용 · 하네스 교정 #3 · #4 · yaml `async-scheduling: false` 무효 주장 — 이 페이로드는 M4 에서 이미지 소스와 측정 로그로 따로 판정했다)을 적은 `testlog_26102500_PLE_mmap_재이식_Qwen_FP8멀티_NVFP4싱글_Solar_완결이식_S2_S4_판정` 은 계보 그래프에서 경유(transit · 기반 슬러그 필터 불일치)로만 잡혔다. 그래서 발췌 · 사건 출처로 쓸 수 없었다. 산문에서 그 문서를 가리킨 자리는 모두 "계보 경유 · 읽기 목록 밖" 으로 표시했고, 같은 사실은 가능한 한 계보 안 문서(`plan_26102420_…` 정정 기록 · `testlog_26102504_…`)와 이 측정의 엔진 로그 · 원장 · 커밋 20c7690 으로 근거를 댔다. 로드-전 RAM 게이트 교정이 사건 블록 없이 산문으로만 실린 이유도 이것이다(02 §2.2).
- 의견: 이 셀 재현 키트의 중심은 빌드가 아니라 런타임 패치 한 파일과 노드 로컬 스테이징, 그리고 single 하네스의 두 교정이다. 01 §1.2 해제 명령으로 소스를 읽고, 네 vLLM 의 PLE 임베딩 구조부터 확인하라(0.6 1번).

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/qwen3.8-flash-next-nvfp4/gb10-1g1n-main-native/qnvfp4-len1048576-kvfp8-plemmap` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `plemmap` | PLE n-gram 테이블을 상주 대신 로컬 디스크 mmap 으로 읽는 런타임 플러그인 활성(같은 HW 의 stock 상주 싱글 형상은 산술 사전판정 불가) | `output/single/envs/.env.q38fn31-nv4-mmap-1m-tp1` · 키 `VLLM_PLE_MMAP` = `1` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 `plemmap` 하나다(근거 = 셀 env `VLLM_PLE_MMAP=1`). 결정론부가 같은 형상은 stock PLE 상주 NVFP4 싱글 TP=1 셀인데, 그 셀은 같은 HW 에서 산술 사전판정으로 불가였다(R1 · W3) — 이 셀을 그것과 가르는 노브가 PLE 적재 방식이고, 카탈로그의 같은 모델 계열 태그도 PLE 방식을 꼬리 토큰(`plemmap` · `pleresident`)으로 가른다. `eager` 는 쓰지 않았다 — 플러그인이 eager 를 강제하므로(V5) mmap 형상 안에서 고를 수 있는 노브가 아니고 짝 셀도 없다. `specoff` 도 쓰지 않았다 — 이 계보의 `v0.31.0` 문서에 이 모델을 spec decode 로 띄운 싱글 기동 기록이 없어 가를 상대가 없다.
