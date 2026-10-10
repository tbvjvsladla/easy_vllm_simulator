# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc-kv11g` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-10T23:06:14Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101104-hint-cd · c-tq4nc · sub · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_c-tq4nc/level_01/post_health_c-tq4nc.json` · `output/single/benchlog/sweep_c-tq4nc/level_01/guidellm_c-tq4nc.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_c-tq4nc/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Qwen3-4B(bf16)를 GB10 ×1 · TP=1 단일 노드에서 vLLM 소스빌드(빌드 입력 v0.26.0) 위에 컨텍스트 32768 · KV dtype turboquant_4bit_nc · 절대 KV 클램프로 서빙했고, full 벤치 동시성 1 decode 22.44 t/s 가 측정됐다(판정 PASS 는 weak 권한의 표시).
이 계보에서 서빙을 막은 벽은 없고 넘은 것은 측정 운영 쪽이다 — 지시문의 backend 값 오기(W3)와 하네스 10분 제한에 의한 스윕 중단(W4); 엔진은 TurboQuant 와 FlashAttention 3 이상의 비호환을 스스로 FA2 로 내렸다(W2).
가장 오해하기 쉬운 값은 24GB 시뮬로 정한 KV 클램프·gmu 로 GB10 에서 최적화된 적 없는 승계값이고(V2~V4), 쉘의 attention backend 핀은 엔진이 알 수 없는 변수라 경고했으며(W1), 판정점 runs 3회 동일 22.44 의 원인은 미확정이다(Q1).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3-4b | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc) |
| HF repo | Qwen/Qwen3-4B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) |
| HF revision(체크포인트 git HEAD) | 1cfa9a7208912126459214e8b04321603b3df60c | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T06:26:21Z 이전 마지막 이동 2026-08-10T02:50:42Z) |
| base_model | Qwen/Qwen3-4B-Base | 체크포인트 README base_model(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) |
| base 슬러그 | qwen3-4b | model(qwen3-4b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization = none(관측)(sweep_index.meta(output/single/benchlog/sweep_c-tq4nc).quantization = none · quantization_source = measured(engine log)) — 비양자화 관측이라 이름 q 축 토큰으로 옮겼다 · q 축 = /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.26.0 | sweep_index.meta(output/single/benchlog/sweep_c-tq4nc) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.26.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=미관측 · VLLM_REF=v0.26.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3-4b | 서빙 yaml model(output/single/configs/c-tq4nc.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · hf_repo(Qwen/Qwen3-4B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-sub-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvturboquant4bitnc-kv11g | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | sub | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/c-tq4nc.yaml) |
| 축 `kv` | turboquant4bitnc | 서빙 yaml kv-cache-dtype(output/single/configs/c-tq4nc.yaml) · vocab kv[turboquant4bitnc]←'turboquant_4bit_nc' |
| 꼬리 `kv11g` | KV 캐시를 절대 11 GiB(11811160064 바이트)로 클램프(kv-cache-memory-bytes) | `output/single/configs/c-tq4nc.yaml` · 키 `kv-cache-memory-bytes` = `11811160064` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 지도가 성립하는 조건**

- **24GB 카드를 흉내 낸 설정이 GB10 한 장에서 돌았다.** (조건 → 서빙 yaml 의 값이 24GB 예산(`rtx4090-24gb-sim` · 예산 24.0 GiB)으로 사이징된 이식 클램프다 → 거동: 측정 노드는 GB10 ×1 이고 엔진은 여유 메모리 114.16 GiB 를 보고했다(L77) → 달라질 점: 카드 메모리가 다르면 KV 클램프와 동시 시퀀스 수는 다시 도출해야 하고, 이 decode 수치는 24GB 카드 위의 성능이 아니다.) 근거 c-tq4nc.yaml §L1-9 · devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실.
- **KV 를 바이트로 고정하면 gmu 는 KV 크기를 정하지 않는다.** (조건 → `kv-cache-memory-bytes` 지정 → 거동: 엔진이 메모리 프로파일링을 건너뛰고 그 바이트를 예약하며 "gpu_memory_utilization config 를 따르지 않는다"고 로그가 적는다 → 달라질 점: free memory 가 작은 환경에서는 같은 바이트가 OOM 을 만들 수 있고, 엔진 문구가 "이전 실행과의 free memory 차이를 보고 바이트를 고치라"고 안내한다.) 근거 lite_engine_c-tq4nc.log §L77. yaml 주석의 gmu 역할("startup free-memory 게이트")은 선언이며 시험된 적이 없다.
- **TurboQuant KV 는 FlashAttention 3 이상과 같이 쓸 수 없다.** (조건 → KV dtype `turboquant_4bit_nc` → 거동: 엔진이 flash_attn 버전을 2 로 덮어쓴다 → 달라질 점: FA 버전 선택이 HW 별로 달라지는지는 이 계보가 GB10 한 장만 재서 관측하지 못했다.) 근거 lite_engine_c-tq4nc.log §L46 · §L59-60.

**(나) 이 형상이 버티는 기전 대 다른 형상**

- `a-kvauto` · `b-kvfp8` — 이 셀과 KV dtype 이 다른 1순위 config — 이 노드에 없어 서빙·측정 시도 0 이므로 실패 기전이 없다 — 지위 미결(대조 없음) — testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §2.
- 이 형상이 버틴 기전은 설명된 적이 없다 — 기동 1회에 READY 이고 사살이 없었다는 사실만 있다. 다른 값·형상을 시도한 같은 계보 기록이 없어 "무엇이 결정적이었는가"에 답할 대조가 없다(02 §2.5).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 러너가 고정한 attention backend 환경변수를 엔진이 알 수 없는 변수라고 경고(서빙은 정상 기동) | 해소하지 않음 — 러너의 핀을 그대로 두었고, 엔진 로그는 백엔드 선택 줄을 둘 남겼다(L58 FLASH_ATTN · L60 TURBOQUANT) | judgment | lite_engine_c-tq4nc.log §L58-60 |
| 2 | W2 | TurboQuant KV 사용 시 엔진이 flash_attn 버전을 2 로 덮어쓰는 경고를 낸다(기동은 성공) | 엔진이 자동으로 FA2 로 덮어씀 — 이 셀은 명시 플래그를 주지 않았다 | arch-invariant | lite_engine_c-tq4nc.log §L59-60 |
| 3 | W3 | 풀 벤치 첫 호출이 backend 인자 검증에서 거부됨(측정 전) | `--backend openai-chat` 과 `--knob-reason backend=chat_template` 로 분리해 재호출 | arch-invariant | testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §5. |
| 4 | W4 | 풀 스윕이 레벨 8 도중 종료됨(측정 미완주) | 같은 설정을 `setsid nohup` 으로 재실행해 완주(서빙 유지 · 최종 수치는 재실행분) | arch-invariant | testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3. |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 엔진 로그 gen max 와 클라이언트 decode 의 비율이 7~8 로 보였다(셀 C 의 비율과 의미가 다르다고 적힘) | 확증 | testlog_26101015_knob프리셋_E2E_셀D_qwen3-4b §3. · devlog_26101108_c-tq4nc_셀D_서빙_측정_knob해소 §불확실 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.26.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.26.1.dev0+g568afb3a1.d20260907 | output/single/benchlog/sweep_c-tq4nc/sweep_index.json:17 meta.vllm_build(sweep_bench 가 이 측정의 레벨 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡은 값 — 배너 전용 규칙은 아니다) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='0.26.1.dev0+g568afb3a1.d20260907' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 1cfa9a7208912126459214e8b04321603b3df60c | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T06:26:21Z 이전 마지막 이동 2026-08-10T02:50:42Z) |
| 빌드 트랙 | source-build | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님)) |
| Dockerfile(선택자) | Dockerfile.source-build | image-history:docker history build-arg 관측(VLLM_REF=v0.26.0 · VLLM_VERSION 없음 → 소스빌드 Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.26.0-cu132-aarch64-source | 셀 env(output/single/envs/.env.c-tq4nc) IMAGE_TAG |
| 이미지 digest | sha256:629942890c0570c8f1c3235bf6927ae4eb315e30336cd68551fdf474a60a44f7 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.12.0a0+5aff392 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.2.1.009 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.05 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/single/benchlog/sweep_c-tq4nc · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 미관측 | 미관측 |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Qwen/Qwen3/Qwen3-4B → <manifest.nas_model_path>/Qwen/Qwen3/Qwen3-4B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

**(1) 빌드 트랙.** source-build 다. 사유는 `Dockerfile.source-build` 헤더가 적는다: prebuilt vLLM wheel 의 `_C` 가 NGC alpha torch 와 C++ ABI 가 맞지 않는다는 것. 그 검증은 헤더가 vLLM 0.22.1 에서 한 것으로 적었고, 이 셀의 0.26.0 에서 wheel 을 시도해 본 기록은 없다. 트랙 판정 사실은 위 표의 트랙 행(docker history build-arg 관측)이 전부다. wheel 로 갈 수 있는 조건을 이 계보는 시험하지 않았다.

**(2) 버전 문자열 넷.** ① 빌드 입력 ref `v0.26.0` — docker history 의 `VLLM_REF` build-arg(관측). **재현 좌표는 이것이다.** ② 엔진 자기보고 `0.26.1.dev0+g568afb3a1.d20260907` — 엔진 로그 배너에서 도구가 정규식으로 잡은 값이며 빌드 입력이 아니다(02 Q4: 두 값이 같은 소스를 가리키는지는 확인되지 않았다). ③ 인증서 `vllm_version` — 이 셀은 weak 판정이라 인증서가 없어 부재. ④ 이름의 `0.26.0` 세그먼트 — ①에서 도구가 파생한 값. 40자 SHA 는 관측되지 않았다.

**(3) arch-locked.** `TORCH_CUDA_ARCH=12.1a`(build-arg) — 네 GPU 의 compute capability 를 확인해 맞는 값으로 다시 정한다. CPU arch `arm64`(이미지). 이미지 CUDA `13.2.1.009` · torch `2.12.0a0+5aff392` · NGC 베이스 `26.05`(이미지 환경). 호스트 드라이버 `580.173.02`. 재도출 방법(예: GPU 질의 도구로 compute capability 확인)은 **추론 — 이 계보에서 실행 검증 ✗**.

**(4) 선언 대 관측.** Dockerfile 선택자는 셀 env 에 선택자 변수가 없고 compose 의 리터럴이 늘 `Dockerfile.source-build` 라 표 자체가 "이 이미지를 지은 증거 아님"이라 적었다 — 실제 근거는 docker history build-arg 관측(`VLLM_REF=v0.26.0` · VLLM_VERSION 없음)이다. 드라이버는 manifest 선언을 옮긴 값이며 노드별 관측 칸은 `미관측`이다.

**(5) 미관측 좌표.** 호스트 OS: 계보 문서에 기록 없음. 노드 메모리: 블랙박스 예산 선언 행의 `mem_total_mib=124608`(선언 행의 필드) · 엔진 로그 `Available RAM: 105.82 GiB`(L61) · `Initial free memory 114.16 GiB`(L77). 인터커넥트: 단일 노드라 해당 없음. 호스트 좌표(드라이버)와 이미지 좌표(CUDA 13.2.1.009)는 다른 층이다.

**(6) 핵심 의존 핀.** requirements 가 `flashinfer-python==0.6.14` 를 고정한다(엔진 로그의 autotune 캐시 경로에도 0.6.14 가 나타난다). Ray 는 build-arg 2.48.0 이며 Dockerfile 헤더(D-A)가 멀티노드가 요구하는 의존을 공유 이미지가 담는다고 적었다 — 단일 노드에서는 미사용이고 이 모델의 요구가 아니라 공유 이미지의 기본값이다. flashinfer 핀이 이 모델 전용 요구라는 기록은 없다(wheel 메타 복제).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 5개 — `inherited` 5.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | lockset gmu_source=hand · 24GB 시뮬 셀에서 가져온 값이며 이 계보에서 바꿔 가며 잰 적이 없다. KV 를 바이트로 고정하면 엔진은 이 값을 따르지 않는다고 로그가 직접 말한다(L77) | V1 |
| `max-model-len` | 32768 | inherited | 24GB 시뮬 셀의 컨텍스트를 승계 — 다른 길이로 잰 기록이 없다. 엔진은 이 길이 기준 동시성 7.11x 를 보고했다(L79) | V2 |
| `max-num-seqs` | 8 | inherited | lockset batch_source=hand-lever (손레버) · 스윕 기록 없음. 벤치 레벨 16 이 이 상한 때문에 걸러졌다는 기록이 있다(testlog §5) | V3 |
| `kv-cache-memory-bytes` | 11811160064 | inherited | lockset kv_source=hand · 24GB 시뮬 예산에서 정한 11 GiB 클램프를 승계했다. GB10 에서 재도출한 기록이 없다. 엔진은 이 값으로 메모리 프로파일링을 건너뛰고 KV 를 예약했다(L77) | V4 |
| `kv-cache-dtype` | turboquant_4bit_nc | inherited | 이 셀을 정의하는 축으로 셀 생성 때 정해졌고, 같은 계보에서 다른 KV dtype 과 대조한 기록이 없다(대안 config 는 이 노드에 없었다 — testlog §2). TurboQuant 때문에 엔진이 FA2 로 내렸다(W2) | V5 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 다시 도출할 값(arch-scaled).** `kv-cache-memory-bytes`(V4) — 엔진 로그의 `GPU KV cache size` 토큰 수(L78)와 목표 컨텍스트로 다시 잡는다. `max-model-len`(V2)과 `max-num-seqs`(V3) — 엔진이 보고하는 "최대 길이 요청 기준 동시성"(L79)과 함께 본다. `gpu-memory-utilization`(V1) — KV 를 바이트로 고정하면 KV 크기를 정하지 않는다(L77). 이 넷은 24GB 시뮬 예산에서 승계한 값이다.

**② 필요조건으로 확인된 값(declared-requirement).** **없다.** 이 계보에는 노브를 빼거나 바꿔서 실패를 관측한 기록이 없다. 명시하면 오히려 깨지는 노브도 관측되지 않았다. `VLLM_ATTENTION_BACKEND` 는 엔진이 알 수 없는 변수라 경고했으므로(W1) 필요조건으로 복사하지 않는다.

**③ 승계값 · 사람이 손으로 고른 값(inherited).** V1~V5 전부. 셀 정의 축인 `kv-cache-dtype turboquant_4bit_nc`(V5)는 다른 dtype 과 대조한 적이 없다(R1: 대안 config 부재). 이유는 "조정이 필요 없어서"가 아니라 "조정한 기록이 없어서"다 — 최적이라는 뜻이 아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전과 경고 서명** — 같은 빌드 입력(v0.26.0)으로 짓고, 기동 로그에서 `Unknown vLLM environment variable`(W1)과 TurboQuant 의 FA2 하향 경고(W2)가 같은 문구로 나오는지 본다 → 같음: 지도 유효 / 경고 사라짐: 핀·플래그 정리 가능(재시험 필요) / 새 에러: 이 지도 폐기. 이 셀에는 자체 패치가 없어(1.1: 패치 슬롯 비어 있음) 패치 폐기 판정은 해당 없다.
2. **기동 성공 판정** — health 200 + 추론 1회, 그리고 엔진 서명 `Using TURBOQUANT attention backend`(L60) · `GPU KV cache size`(L78). 이 실행은 READY 140 s(testlog §2), 엔진 초기화 66.04 s(L90) → 한참 더 걸리거나 서명이 없으면 재도출 단계로.
3. **호스트 메모리 안전 예산 선언** — 이 노드는 서빙 전에 예산을 선언한다(이번 입력: overhead 18432 · weights 7672 · KV 11264 · 총량 124608 MiB 의 선언값 — 01 §1.4). 입력은 네 노드의 총량과 가중치·KV 선언이며, 이 18432 의 산출 근거는 계보에 없다 → 사살·트립이 나면 overhead 를 다시 잡는다.
4. **KV 재도출** — 다른 GPU 메모리에서는 `GPU KV cache size`(L78)와 최대 길이 동시성(L79)을 먼저 확인하고 바이트·시퀀스를 다시 정한다 → 24GB 카드면 Q5 가 열려 있다.
5. **성능 기대 확인** — 같은 입력 형상으로 재서 03 곡선과 비교하되 판정점 runs 의 동일값(Q1)은 재현성 증거로 쓰지 않는다 → 기대 이하면 아래 행선지.

실패 신호별 행선지: 빌드·버전 핀 문제 → `upstream-version-watch` · 서빙 설정·KV·노브 문제 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · 노드·토폴로지 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc-kv11g` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101104-hint-cd · c-tq4nc · sub |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101104_hint_cd__sub__c_tq4nc |
| work-manifest | docs/_evidence/camp_26101104_hint_cd__sub__c_tq4nc.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-10T19:53:02Z |
| 승인 발화(전사) | 사용자(plan 승인 팝업 응답 전사) — "승인" (plan_26101104 · C·D 는 single 서브 자체 발행 · 근거 발화 10-10 "이번 6개 모델에 대한 E2E결과는 각각 힌트 태그 발행해줘") |
| 현행 full 정의 충족(측정 등급) | 미관측 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(서브 · 셀 D · plan_26101104)`(저작자 `hint 저작 Agent(서브 · 셀 D · plan_26101104)` 아님) · 주장 64건 · 지적 8건 중 고침 8건 · 열림 0 · 2026-10-10T23:20:32Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_BUILD_LEDGER` | 빌드 원장(`/opt/easy-vllm/build_ledger.json`) 부재 — 원장 도입 전 이미지다. 패치 적용 결과는 `applied_set.status: unobservable` 과 라벨된 재구성으로만 말한다(X9). |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. |
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

이 hint 는 새 실험이 아니라 2026-10-10 에 이미 끝난 서빙·측정 기록(full 벤치 재실행분)을 발행용으로 정리한 것이다. 발행 과정에서 도구가 측정 판정(verdict)을 weak 권한으로 다시 낸 것은 2026-10-10T23:04:59Z 이고 보고서는 2026-10-10T23:05:19Z 에 그 판정으로 다시 발행했다(보고서 본문의 '생성일' 은 최초 렌더 시각 2026-10-10T06:26:21Z 그대로다). 원격 hint 태그 목록 조회(`git -C <hint 클론> ls-remote origin 'refs/tags/hint/*qwen3-4b*'`)에는 같은 모델의 0.26.0 태그가 4개(KV dtype auto · fp8 · turboquant 3bit · turboquant 4bit, arch 세그먼트는 `gb10-main-sim-kv24g` / `gb10-sub-sim-kv24g`)와 0.27.1 의 다른 HW 태그 1개 있었다. 이름만 확인했고 내용·수치는 읽거나 대조하지 않았다 — 수치를 직접 비교하지 말 것. 저작자와 사실 검증자는 다른 에이전트 세션이다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.26.0/qwen3-4b/gb10-1g1n-sub-native/qbf16-len32768-kvturboquant4bitnc-kv11g` — 꼬리 1토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `kv11g` | KV 캐시를 절대 11 GiB(11811160064 바이트)로 클램프(kv-cache-memory-bytes) | `output/single/configs/c-tq4nc.yaml` · 키 `kv-cache-memory-bytes` = `11811160064` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리 토큰 하나(`kv11g`)를 골랐다: 서빙 yaml 의 `kv-cache-memory-bytes` 가 11811160064 바이트(11 GiB)로 고정된 점이 이 셀의 가장 뚜렷한 KV 설정이다. 같은 모델·KV dtype·컨텍스트의 다른 셀이 다른 클램프를 쓰는지는 이 발행이 대조하지 못했다 — 꼬리는 구분 노브 후보이지 '유일한 차이'의 주장이 아니다. 후보 중 `plenone`(모델에 PLE 키 없음) · `specoff`(speculative 설정 없음) · `graph`(enforce-eager 부재 = 엔진 기본)는 설정의 부재일 뿐 이 셀을 다른 셀과 가르는 근거를 제시하지 못해 쓰지 않았다. attention backend 는 러너의 환경변수 핀을 엔진이 알 수 없다고 보고했으므로(W1) 이름 근거로 쓰지 않았다.
