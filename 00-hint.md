# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.19.1/gpt-oss-120b/gb10-1g1n-main-native/qmxfp4-len131072-kvfp8` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-10T09:07:43Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26101017-knob-e2e-single · gptoss120b-0191-c0 · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_gptoss120b-0191-c0/level_01/post_health_gptoss120b-0191-c0.json` · `output/single/benchlog/sweep_gptoss120b-0191-c0/level_01/guidellm_gptoss120b-0191-c0.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_gptoss120b-0191-c0/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

OpenAI gpt-oss-120b(MXFP4 · harmony)를 GB10 GPU 1개 × 노드 1대에서 stock vLLM 0.19.1 wheel 이미지로 서빙했다 — 컨텍스트 131072 · KV fp8 · PLE 없음 · spec off · cudagraph, 동시성 1 decode 33.40 t/s(GuideLLM 0.7.3 · 반복 3 · 완결 엔드포인트 · weak PASS · floor 28.5).
가장 비싼 벽은 harmony 가 chat 엔드포인트에서 ignore-eos 를 무력화해 디코드를 왜곡한 측정 경로(W1 · `--backend openai` 로 해소)와 KV 절대클램프 없는 진입이 예산 선언을 건너뛰어 워치독에 사살된 것(W2 · 클램프 승계)이며, 이번 회차는 벤치 예산 게이트의 교집합 공(W5)을 프리셋 선택지 추가로 넘었다.
가장 오해하기 쉬운 값은 판정 기준 E 33.53 t/s 로, 이 셀이 잰 값이 아니라 조건 미상의 외부 수치를 2026-09-01 판정에서 손으로 승계한 것이다(03 §3.5) — KV 클램프 30 GiB 도 키의 존재만 필요조건이고 값은 승계값이다(V4).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | gpt-oss-120b | sweep_index.meta(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| HF repo | openai/gpt-oss-120b | 체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) |
| HF revision(체크포인트 git HEAD) | b5c939de8f754692c1647ca79fbf85e8c1e70f8a | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T07:45:58Z 이전 마지막 이동 2025-09-17T02:41:16Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) |
| base 슬러그 | gpt-oss-120b | model(gpt-oss-120b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | mxfp4 | sweep_index.meta(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_gptoss120b-0191-c0) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.19.1 | sweep_index.meta(output/single/benchlog/sweep_gptoss120b-0191-c0) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.19.1 | track=선택자 Dockerfile(docker history build-arg 관측) · ref=VLLM_REF 미관측 · version=docker history build-arg VLLM_VERSION · repo=VLLM_REPO 미관측 · sha=미관측 · VLLM_VERSION=0.19.1(wheel 릴리스) |
| 세그먼트 `model` | gpt-oss-120b | 서빙 yaml model(output/single/configs/gptoss120b-0191-c0.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · hf_repo(openai/gpt-oss-120b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qmxfp4-len131072-kvfp8 | derived(q·len·kv) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | mxfp4 | /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b/config.json quantization_config quant_method=mxfp4 · vocab quant[mxfp4]←'mxfp4' |
| 축 `len` | 131072 | 서빙 yaml max-model-len(output/single/configs/gptoss120b-0191-c0.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/single/configs/gptoss120b-0191-c0.yaml) · vocab kv[fp8]←'fp8' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만든 것**

1. **통합메모리(호스트 RAM 과 GPU 가 한 풀)에서 가중치 · KV 가 호스트 바닥을 직접 깎는다** → 이번 예산 선언(선언값 — 총량 124608 · weights 62226(체크포인트 크기 · 엔진 실측 상주 65.96 GiB 보다 작다) · KV 30720 · overhead 18432 MiB)의 바닥이 13230MiB 로 트립선(10240MiB) 바로 위에 붙었고, 그래서 벤치 컨테이너 몫이 2990MiB 로 묶였다(W5). 실측 상주를 넣으면 바닥은 더 낮아진다. 별개로 2026-09-01 의 예산 조건에서는 gmu 0.95 가 예산 선언 거부로 막혔다(이번 바닥의 결과가 아니다) → GPU 메모리와 호스트 RAM 이 분리된 HW 에서는 이 결합이 사라져 KV · gmu 상한이 VRAM 으로만 정해지고 벤치 몫 제약도 다르다(devlog_26090117_15_53_gptoss120b_단일노드_E2E_서사 §결정과 근거 · docs/simlog/26101012_벤치_knob_프리셋_E2E/b_sweep_try1_budget_block.log §L9-11).
2. **GPU 가 sm_121(GB10)** → vLLM 0.19.1 의 커널 게이트가 어텐션 후보를 TRITON_ATTN 하나로, mxfp4 MoE 를 MARLIN 하나로 좁힌다(triton · flashinfer_trtllm 은 디바이스 미지원 · flashinfer_cutlass 는 스킴 미지원) → triton MoE 게이트 범위(capability 9.0~11.0) 안의 GPU 에서는 다른 커널 경로가 열리고 속도 · 메모리가 달라진다(sweep_map_26090504_gptoss120b_0191_single_완주 §3. · §6. · plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.5).
3. **배치1 디코드가 가중치 대역폭에 묶임** → 토큰당 active 5.738 GiB 를 GB10 스펙 대역폭으로 읽는 상한이 44.31 t/s 이고 측정은 그 약 3/4 다. KV dtype 을 바꿔도 속도가 그대로였던 것도 같은 이유다 → 대역폭이 다른 HW 에서는 판정점이 대역폭에 비례해 달라진다고 보는 것이 출발점이다(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §판정이 REFUTE → PASS 로 뒤집힌 경위 · §참조 그라운드 재탐색의 소득과 한계).

**(나) 이 형상이 버티는 기전 대 다른 형상이 실패한 이유**(02 §2.5 의 요약)

- KV auto(bf16) — dtype 만 다름 — 가중치 대역폭이 지배라 속도 동급 · 같은 30 GiB 풀의 KV 토큰은 절반 — 확정 — sweep_map_26090504 §2. · testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §참조 그라운드 재탐색의 소득과 한계.
- KV fp8_e5m2 — dtype 만 다름 — 어텐션 forward 의 dtype assert 로 로드 전 실패 — 확정 — sweep_map_26090504 §4.
- MoE triton · flashinfer_trtllm · flashinfer_cutlass — `--moe-backend` 만 다름 — 커널이 이 디바이스 또는 mxfp4 스킴을 받지 않아 기동 30~39초에 예외 — 확정 — sweep_map_26090504 §3. · §5. · §6.
- KV 클램프 없는 진입(C0 첫 시도) — `kv_cache_memory_bytes` 부재(kv auto · seqs · batched 누락은 별개 실행인 C0 1차 수렴본의 일이다 — R9) — 예산 선언이 생략돼 로드 피크(회수할 수 없는 상주 가중치 · KV 할당이 쌓이는 구간)의 MemAvailable 급감을 워치독이 사살 — 확정 — plan_26090419 §8.4.
- overhead 12288(바닥 19376) 대 18432(바닥 13230) — 선언 overhead 와 총량 표기(124610 → 124608)가 다르다 — 서빙 기동이 있었던 창(09-04 앞선-2 · 앞선-3 · 이번 시도 1 · 앞선-1 은 선언 직후 0초에 clear 되고 serve_start 가 없어 기동이 아니다)은 모두 사살 없이 닫혔고 13230 쪽은 벤치 몫이 2990MiB 로 묶였다 — 서빙 생존은 확정 · 어느 쪽이 실제 overhead 에 가까운지는 미결 — 02 §2.5 · audit_26090513_광의탐색_캠페인_라이브_결함_전수감사 §2. 라이브가 낸 결함.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 동시성 1 디코드가 클라이언트 10.33 t/s 대 엔진 로그 34.1 t/s 로 3.3배 갈렸다 | run_bench·sweep_bench·lite_bench 에 --backend 신설 → harmony 는 --backend openai(/v1/completions)로 측정 · 이후 harmony×openai-chat 은 run_bench 가 거부 | arch-invariant | 교정 후 교차검증 ratio 3.3 → 1.01 · 생성 토큰 331 → 2048 (testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §하네스 결함 3건) · 이번 측정 b_sweep.log L10 endpoint=/v1/completions |
| 2 | W2 | C0 첫 materialize 의 정상 로드(상주 99.45 GiB)가 8분 뒤 SIGKILL(137)로 죽었다 | 2026-09-01 PASS 인증서의 kv-cache-memory-bytes 32212254720 을 승계하고 셀 후보 스키마에서 필수화 | arch-scaled | 이 셀 라벨의 serve_start 가 있는 기동(01 §1.4 원장 앞선-2 · 앞선-3 · 이번 시도 1)은 예산 선언 창 안에서 사살·트립 없이 닫혔다 — 다른 라벨의 기동은 이 표가 말하지 않는다 · 2026-09-01 은 클램프가 있어도 디스크 크기 선언 탓에 로드 막바지 재무장이 있었다(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §안전층 실적) |
| 3 | W3 | GuideLLM 첫 셀에서 harmony chat 경로 측정이 서버 파서 파손으로 성립하지 않았다 | 측정 엔드포인트를 /v1/completions 로 통제변인에 고정 · run_bench 가 harmony×openai-chat 을 거부(2026-09-05 승격) | arch-invariant | sweep_map_26090504 C0·C2 measured(측정 엔드포인트 /v1/completions) · 이번 측정 레벨 5개 measurement_ok |
| 4 | W4 | GuideLLM 벤치가 측정을 끝낸 뒤 결과 JSON 저장에서 죽어 기록이 남지 않았다 | docker run --user "$(id -u):$(id -g)" 로 호스트 사용자로 실행(run_bench L392-L393) | arch-invariant | 이번 측정 레벨 5개 × 반복 3 의 guidellm JSON 이 sweep 디렉터리에 남았다(03 §3.1 levels 표) |
| 5 | W5 | 1차 스윕이 lite 통과 뒤 레벨 1 진입에서 run_bench exit 5 로 멈췄다(측정 0건) | 사용자 승인으로 default_preset bench_budget_mib 에 2048(auto) 추가 → --bench-budget-mib 2048 로 재측정(측정은 미커밋 워킹트리 프리셋 · 커밋 d298699 는 측정 3분 뒤) | arch-scaled | b_sweep.log L9 bench=OK · 레벨 1·2·4·8·16 × 반복 3 완주(testlog_26101017_08_29_knob프리셋_E2E_셀B_gptoss120b_0191_c0_판정 §2) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 동시성 1 디코드 34.26 t/s 가 E 59 대비 ratio 0.581 로 REFUTE 판정됐다 | 반증 | testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §판정이 REFUTE → PASS 로 뒤집힌 경위 |
| M2 | 같은 인증서 안에서 lite warm gen 16.43 t/s 와 판정점 decode 34.55 t/s 가 2배 넘게 갈렸다 | 반증 | testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §하네스 결함 3건 H-3 |
| M3 | 레시피 러너는 FLASHINFER 를 요청하는데 엔진 로그는 TRITON_ATTN 을 썼다 | 반증 | plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.5 · 이번 측정 엔진 로그 "Using TRITON_ATTN attention backend out of potential backends: ['TRITON_ATTN']" |
| M4 | --cap 1 로 materialize 한 C0 첫 시도가 예산 선언 없이 진입해 사살됐다(W2) | 반증 | plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §8.4 |
| M5 | overhead 12288 로 선언하면 바닥이 실제보다 높게 잡혔다 | 반증 | audit_26090513_광의탐색_캠페인_라이브_결함_전수감사 §2. 라이브가 낸 결함 F4 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | wheel | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | 0.19.1 | VLLM_REF 미관측 |
| vLLM 소스 SHA(40자) | 미관측 | 미관측 |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.19.1 | output/single/benchlog/sweep_gptoss120b-0191-c0/sweep_index.json:17 meta.vllm_build(sweep_bench 가 이 측정의 레벨 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡은 값 — 배너 전용 규칙은 아니다) · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — attestation vllm_dist_version · native pip freeze 모두 없다(sweep_index.meta.vllm_build='0.19.1' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | b5c939de8f754692c1647ca79fbf85e8c1e70f8a | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-10T07:45:58Z 이전 마지막 이동 2025-09-17T02:41:16Z) |
| 빌드 트랙 | wheel | artifacts 선택자(image-history:docker history build-arg 관측(VLLM_VERSION=0.19.1 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이 이미지는 compose build 로 지어지지 않았다)) |
| Dockerfile(선택자) | Dockerfile | image-history:docker history build-arg 관측(VLLM_VERSION=0.19.1 · VLLM_REF 없음 → wheel Dockerfile) — 셀 env 선택자 없음 · output/single/docker-compose.yaml 리터럴 build.dockerfile: Dockerfile.source-build(선택자 변수 없음 — compose build 는 늘 Dockerfile.source-build 를 짓는다 · 이 이미지를 지은 증거 아님) · ⚠ compose 리터럴 Dockerfile.source-build ≠ 관측 Dockerfile(이 이미지는 compose build 로 지어지지 않았다) |
| vLLM 저장소 | 미관측 | 미관측 |
| 이미지 태그 | easy-vllm:0.19.1-cu130-aarch64-wheel | 셀 env(output/single/envs/.env.gptoss120b-0191-c0) IMAGE_TAG |
| 이미지 digest | sha256:026e9d87403db08a6c4d4ced91ac49f3efcafaf78fdc206dc66048d8914abd52 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.10.0a0+a36e1d3 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.1.1.006 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.01 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_head_gptoss120b-0191-c0.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) · ⚠ 선언 580.173.02 과 다르다(driver_conflict) |
| 드라이버(노드별 관측) | main=580.178.04 | 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_head_gptoss120b-0191-c0.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) |
| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: 측정 엔진 로그 기동 배너 'kernel driver version'(output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_head_gptoss120b-0191-c0.log:30 · 컨테이너가 보고한 호스트 커널 드라이버 · 이 측정의 엔진 로그 17개 중 그 줄이 있는 로그 모두 같은 값) · 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_gptoss120b-0191-c0 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `mxfp4` | /app/models/OpenAI/gpt-oss-120b → <manifest.nas_model_path>/OpenAI/gpt-oss-120b/config.json quantization_config quant_method |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = wheel** — 이 vLLM 은 torch 2.10.0 을 핀하고 NGC 26.01-py3 의 torch 가 2.10 계열이라 prebuilt wheel 의 ABI 가 맞는다. 사용자가 "최신이 아니라 wheel 로 설치 가능한 버전" 을 지정했고 경계를 실측으로 찾았다 — testlog_26090117_gptoss120b_단일노드_FullE2E_판정 §판정 S1 행 "0.19.1=2.10.0 마지막 · 0.20.0=2.11.0". 같은 문장에서 파생하면, torch 2.11 이상을 핀하는 버전(0.20.0+)으로 가면 wheel 트랙이 아니다. 위 표의 트랙 행은 docker history build-arg(VLLM_VERSION=0.19.1 · VLLM_REF 없음)로 그 트랙을 관측했다.

(2) **vLLM 버전 문자열** — 빌드 입력 0.19.1 은 docker history 의 build-arg `VLLM_VERSION` 이 만든 관측값이다. "엔진 자기보고 0.19.1" 은 sweep_bench 가 엔진 로그에서 `v<x.y.z>` 정규식으로 잡은 값이고(배너 전용 규칙 아님 — 위 표 출처), 엔진 로그 배너도 `version 0.19.1` 을 찍었다. 인증서는 발행되지 않아(weak) 강한 키 `vllm_version` 이 없고, wheel 메타는 미관측이다. 재현 좌표는 **GitHub 릴리스 v0.19.1 의 wheel**(`vllm-0.19.1+cu130-…-aarch64.whl` 꼴 · Dockerfile 이 릴리스 자산 URL 로 받는다)이다 — 40자 소스 SHA 는 이 트랙에서 쓰지 않았고 기록도 없다.

(3) **arch-locked 값과 재도출** — CPU arch `arm64`: Dockerfile 이 빌드 시 `uname -m` 으로 wheel 이름을 고르므로 네 호스트에서 지으면 자동으로 맞는다. `CUDA_VERSION=130`(cu130 wheel): 네 NGC 베이스의 CUDA 메이저에 맞는 wheel 자산이 릴리스에 있는지 확인한다. NGC 베이스 26.01(torch 2.10.0a0+a36e1d3 · CUDA 13.1.1.006 — image inspect 관측): 대상 vLLM 이 핀한 torch 와 접두가 맞는 NGC 태그를 결정론 해소기로 다시 고른다. GPU 아키텍처 sm_121: 엔진 로그의 어텐션 potential backends 목록과 MoE 백엔드 선택 줄로 네 GPU 에서 열리는 커널을 다시 확인한다. 드라이버 계열: 네 호스트의 `nvidia-smi` 로 읽는다.

(4) **선언 대 관측** — 해소 산출물(resolved.json)이 이 빌드에 묶이지 않아(결손 `HINT_MISSING_RESOLVE`) torch · NGC · CUDA 는 이미지 inspect 관측값만 있다. 측정 이미지의 torch 는 image inspect 와 엔진 로그 머리(`NVIDIA Release 26.01` · `PyTorch Version 2.10.0a0+a36e1d3`)가 같은 값을 보인다. 2026-09-01 testlog 의 `2.10.0a0+nv26.01` 은 2026-09-01 에 지은 다른 이미지(측정 이미지는 2026-09-03 재빌드)의 기록이라 이 이미지와 견주지 않는다. Dockerfile 의 ARG 기본값은 0.18.0 이지만 실제 이미지는 build-arg 0.19.1 로 지어졌다(docker history 관측이 정본). 드라이버는 위 표가 manifest 선언(580.173.02)을 옮겼고, 이번 측정의 엔진 로그 머리는 `Using CUDA 13.1 driver version 590.48.01 with kernel driver version 580.178.04` 를 찍었다 — 컨테이너가 관측한 커널 드라이버는 580.178.04 다(02 Q4).

(5) **미관측 환경 좌표** — 드라이버(노드 관측): 위 (4)의 엔진 로그 줄이 컨테이너에서 본 커널 드라이버 580.178.04 를 기록한다(output/single/benchlog/sweep_gptoss120b-0191-c0/lite_engine_gptoss120b-0191-c0.log L30). 노드 메모리 총량: 124610MiB(sweep_map_26090504 머리 표 통제변인) · 이번 예산 선언 124608MiB(선언값). 인터커넥트: 해당 없음(single · TP=1). 호스트 OS: 미기록. 호스트 CUDA: 계보가 `CUDA 132` 로 적은 스캔값(testlog_26090117_gptoss120b_단일노드_FullE2E_판정 머리) — 이미지 CUDA 13.1.1 과는 다른 층이다.

(6) **핵심 의존 핀** — `ray==2.48.0`(Dockerfile build-arg · 분산 서빙용 공유 이미지 스탠자 — 이 single 셀은 쓰지 않는다) · `flashinfer-python==0.6.6`(requirements.txt — 이미지에 있지만 이 셀의 어텐션 · MoE 경로는 TRITON_ATTN · MARLIN 이라 쓰이지 않는다) · triton 3.6.0(이미지에 존재 · TRITON_ATTN 경로 — plan_26090419_광의탐색_캠페인안_gptoss120b_0191 §2.2) · deep_gemm 부재(같은 절). 이 모델 고유의 요구로 추가된 핀은 없다 — 전부 공유 이미지의 기본값이다. 주의: requirements.txt 머리 주석은 출처를 0.18.0 wheel METADATA 로 적는다(01 §1.2 · 02 Q7).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 7개 — `tuned` 1 · `inherited` 5 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.9 | inherited | 손레버 — 2026-09-01 사용자 선택 · 스윕 0회 · 0.95 는 예산 선언 거부로 막힘(R3) · 엔진은 KV 클램프가 있으면 gmu 를 KV 산정에 쓰지 않는다고 로그에 적었다 | V1 |
| `max-model-len` | 131072 | inherited | 모델 max_position_embeddings 와 같은 값으로 고정(광의탐색의 통제변인 · 축 아님) · 2026-09-01 사용자 선택 131K 승계 · 스윕 0회 | V2 |
| `max-num-seqs` | 16 | inherited | 2026-09-01 인증서 기준선 값을 광의탐색이 공통 노브로 승계 · 조정 기록 없음 · 엔진 보고 전장(131072) 동시성 12.54x 보다 크다 — 짧은 요청 부하 전제 | V3 |
| `kv-cache-memory-bytes` | 32212254720 | declared-requirement | 키의 존재가 필요조건 — 없으면 예산 선언이 생략돼 정상 로드가 워치독에 사살됐다(W2). 값 30 GiB 자체는 2026-09-01 레시피에서 승계했고 조정된 적 없다(산식 미기록) | V4 |
| `kv-cache-dtype` | fp8 | tuned | fp16(2026-09-01 · 436,896 토큰 · 34.26 t/s) 대 fp8(873,808 토큰 · 34.55 t/s) · 2026-09-04 auto(C2 · 32.71) 대 fp8(C0 · 32.83) · fp8_e5m2(C1) 로드 실패 — 값을 바꿔 잰 기록이 있다 | V5 |
| `quantization` | mxfp4 | inherited | 체크포인트 quantization_config quant_method 와 같은 값(warm-start hint 의 모델 축에서 승계) · 다른 값을 시도한 기록 없음 | V6 |
| `max-num-batched-tokens` | 8192 | inherited | 2026-09-01 기준선 값을 광의탐색이 공통 노브로 승계 · 조정 기록 없음(누락한 C0 1차는 R9 로 폐기) | V7 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① **반드시 다시 도출할 값(arch-scaled)** — kv-cache-memory-bytes(V4)의 크기 · gpu-memory-utilization(V1) · max-num-seqs(V3) · 그리고 노브 밖의 호스트 예산 선언(weights · overhead). 절차: 기동 후 엔진 로그의 `Model loading took … GiB`(상주 가중치) · `GPU KV cache size: … tokens` · `Maximum concurrency for … tokens per request` 를 읽고, 호스트 바닥 = 총량 − 상주 가중치 − KV − overhead 가 트립선 위 여유를 두는지 확인한 뒤, 필요하면 KV 클램프를 낮춰 바닥을 올린다.

② **필요조건(declared-requirement)** — kv-cache-memory-bytes 는 **키가 있어야** 한다: 없으면 예산 선언이 생략돼 정상 로드가 워치독에 사살됐다(W2). 값 30 GiB 자체는 필요조건이 아니다. 명시하면 오히려 깨지는 노브: `--moe-backend` 를 triton · flashinfer_cutlass · flashinfer_trtllm 으로 명시하면 기동이 즉사하고(R6~R8) `kv-cache-dtype fp8_e5m2` 도 로드 전 실패한다(R5) — 이 HW 에서는 MoE 를 지정하지 않아 엔진이 MARLIN 을 고르게 둔다. 측정 쪽에서는 harmony 계열에 `--backend openai-chat` 을 쓰면 측정이 무효가 된다(W1 · W3).

③ **조정된 적 없는 승계값 · 손레버(inherited)** — gmu 0.9(V1 · 사용자 손레버 · 엔진은 KV 클램프가 있으면 KV 산정에 쓰지 않는다) · max-model-len 131072(V2 · 모델 최대 고정) · max-num-seqs 16(V3) · max-num-batched-tokens 8192(V7) · quantization mxfp4(V6 · 체크포인트 선언과 같은 값). 앞의 셋은 2026-09-01 기준선에서 승계됐고 스윕한 기록이 없다 — 최적값이라는 뜻이 아니다. 조정된 값은 kv-cache-dtype fp8(V5 · tuned) 하나다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전 · 빌드 트랙** → 대상 vLLM 의 torch 핀과 NGC 베이스 torch 접두가 맞는지 결정론 해소기로 본다 → 맞으면 wheel 트랙 그대로 · torch 2.11+ 이면 source-build 트랙으로 재도출 · 이 셀에 자체 패치가 없으므로 "패치가 여전히 필요한가" 는 묻지 않아도 된다(01 §1.1 — 빌드 · 런타임 패치 0).
2. **커널 경로** → 첫 기동 엔진 로그의 `Using … attention backend out of potential backends: [...]` 와 `Using '…' Mxfp4 MoE backend.` 를 읽는다 → TRITON_ATTN · MARLIN 이면 이 지도의 비교가 그대로 성립 · 후보가 늘었으면 R1 · R6~R8 을 재개 후보로 · 그 줄이 없거나 기동이 즉사하면 이 지도 폐기(다른 커널 세대).
3. **호스트 메모리 안전 예산** → 통합메모리 HW 면 예산 선언이 반드시 필요하다(W2). 산식 입력 = 총량 · 상주 가중치(엔진 로그 실측을 쓴다 — 체크포인트 크기는 이 모델에서 약 5.2 GiB 작다) · KV 클램프 바이트 · overhead(근거와 함께 선언) → 선언이 수락되고(원장 `budget_honored`) 바닥이 트립선 위 여유를 두면 진행 · 바닥이 트립선 밴드에 붙으면 KV 클램프를 낮춰 재도출 · 분리형 GPU 메모리 HW 면 이 결합 자체를 다시 판단.
4. **기동 성공 판정** → health 200 + 추론 1회 + 엔진 로그에 `reserved … GiB memory for KV Cache as specified by kv_cache_memory_bytes config` · `GPU KV cache size` 줄 → 셋이 다 있으면 통과 · 패치 발화 서명은 해당 없음 · 로드 중 `watchdog_highrate_hold` 는 선언이 있으면 정상이다(01 §1.4 (c)).
5. **측정 경로** → harmony 계열이므로 `--backend openai`(완결 엔드포인트)로 재고, 벤치 컨테이너 몫 ≤ 바닥 − 절대밴드인지 먼저 계산한다(W5) → 클라이언트 decode 와 엔진 로그 generation throughput 이 같은 자릿수면 진행 · 수 배 갈리면 측정 경로 결함(W1)이니 판정하지 않는다.
6. **판정 기준** → E 33.53 은 조건 미상의 외부 수치다(03 §3.5) → 네 HW 에서는 대역폭 기반 상한(R_fp)으로 E 의 물리 타당성을 먼저 검산하고, 같은 HW 의 like-with-like 레퍼런스를 다시 찾는다 · 찾지 못하면 weak 판정으로만 쓴다.

실패 신호별 행선지: 빌드 · 버전 핀 · wheel/소스 트랙 문제 → `upstream-version-watch` · 서빙 설정 · KV 클램프 · 노브 · 커널 선택 문제 → `vllm-recipe-explorer` · 성능 기대 이하 · 측정 경로 의심 → `adversarial-benchmark` · 노드 · 토폴로지 · 드라이버 불일치(02 Q4) · 호스트 안전체계 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.19.1/gpt-oss-120b/gb10-1g1n-main-native/qmxfp4-len131072-kvfp8` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26101017-knob-e2e-single · gptoss120b-0191-c0 · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26101017_knob_e2e_single__main__gptoss120b_0191_c0 |
| work-manifest | docs/_evidence/camp_26101017_knob_e2e_single__main__gptoss120b_0191_c0.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-10T08:04:57Z |
| 승인 발화(전사) | 사용자(선언 확인 팝업 응답 전사) — "일단 4건 발행한 뒤 …" (plan_26101017 · 6셀 E2E hint 발행 지시) |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(검증 서브에이전트)`(저작자 `저작 Agent(hint 저작 서브에이전트)` 아님) · 주장 142건 · 지적 16건 중 고침 16건 · 열림 0 · 2026-10-10T09:34:48Z |
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

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.19.1/gpt-oss-120b/gb10-1g1n-main-native/qmxfp4-len131072-kvfp8` — 꼬리 0토큰.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 빈 목록(`[]`)이다. 결정론부(vLLM 0.19.1 · gpt-oss-120b · GB10 1GPU 1노드 main native · mxfp4 · 131072 · fp8)가 같은 다른 셀이 카탈로그에 없다 — 같은 계보의 2026-09-04 C0 는 이 셀과 같은 트리플렛이고, 자매 셀 C2 는 KV 가 auto 라 결정론부(kv 축)에서 이미 갈린다. 후보 셋은 가르는 노브가 아니다 — PLE none 은 이 모델에 PLE 가 없다는 사실이고, spec off · cudagraph 는 이 모델의 모든 셀이 공유하는 상태라 같은 q·len·kv 의 다른 셀과 구분해 주지 않는다.
