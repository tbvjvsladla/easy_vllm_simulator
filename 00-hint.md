# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8-plemmap-spec3-eager` |
| 형식 | `hint-payload/v6` · 이름 문법 `v6` |
| 생성(UTC · 주입) | 2026-09-23T01:59:30Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26092301-hint-v6-e2e · nv4-f8-262k-mmp · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_nv4-f8-262k-mmp.json` |
| 증거 등급(task_class) | hint_map_only |
| 측정 등급(bench_mode) | lite |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

`nvidia/Qwen3.8-Flash-Next-NVFP4` 를 GB10 1GPU × 2노드(Ray TP=2)에서 vLLM `v0.29.0rc6` 소스빌드로 서빙했다 — NVFP4 · max-model-len 262144 · KV fp8_e4m3 · PLE NVMe mmap · MTP k=3 · eager, lite 관측 warm decode 18.77 t/s(동시성 1 · MTP on · 반복 1 · OBSERVATION-ONLY · 판정 없음).
가장 비싼 벽은 stock 이 NVFP4 mixed 체크포인트를 싣지 못한 것(W1 → 빌드 패치 60), fp8 KV 를 QSA 가 거부한 것(W8 → 64), 통합메모리에서 PLE 테이블을 거둘 경로가 없던 것(W6 → 62 mmap + 노드 로컬 NVMe)이다.
오해 위험: KV 20GiB 클램프(V6)는 필요량을 크게 넘는 승계 손레버이고, 리포트의 cold TTFT 는 연속 두 번째 실행의 값이라 cold 가 아니다(Q2) — 이 lite 수치를 기준선이나 full · 인증 수치와 나란히 쓰지 말 것.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | bench_report 측정 환경 표(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-23T01:56:16Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(bench_report 측정 환경 표(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md)) · q 축 = /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | bench_report 측정 환경 표(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| 토폴로지 · TP | multi · TP=2 | bench_report 측정 환경 표(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.29.0 | bench_report 측정 환경 표(bench_report_26092310_56_16_qwen3.8-flash-next-nvfp4_GB10_0.29.0.md) |

**명명 축** — 태그 이름은 도구가 이 축들에서 전량 파생했다(발행자 입력 ✗).

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.29.0rc6 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.29.0rc6(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/nv4-f8-262k-mmp.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target) |
| 세그먼트 `recipe` | qnvfp4-len262144-kvfp8-plemmap-spec3-eager | derived(q·len·kv·ple·spec·graph) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `q` | nvfp4 | /app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/multi/configs/nv4-f8-262k-mmp.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/nv4-f8-262k-mmp.yaml) · vocab kv[fp8]←'fp8_e4m3' |
| 축 `ple` | mmap | 셀 env VLLM_PLE_MMAP=1(output/multi/envs/.env.nv4-f8-262k-mmp) · vocab ple[mmap]←'mmap' |
| 축 `spec` | 3 | 서빙 yaml speculative-config.num_speculative_tokens(output/multi/configs/nv4-f8-262k-mmp.yaml · method=mtp) |
| 축 `graph` | eager | 서빙 yaml enforce-eager: true(output/multi/configs/nv4-f8-262k-mmp.yaml) · vocab graph[eager]←'eager' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만드는 전제**
1. **통합메모리(노드 MemTotal 약 124,610 MiB 한 풀)** → 가중치 · KV · 런타임 overhead · PLE 가 같은 풀을 나눠 쓰고, 호스트 워치독은 MemAvailable 로 판정한다 → UVA CPU offload 는 같은 풀을 쓰므로 메모리를 거두지 못한다고 판단해(구조 추론 — offload 를 실제로 잰 적은 없다 · R3) PLE 를 빼는 길로 NVMe mmap 을 택했다. 이산 GPU HW 에서는 offload 가 실제로 메모리를 옮기고 예산 산식(`MemTotal − weights − kv − overhead`)의 입력도 달라진다 — plan_26090918 §2.3 · plan_26091216 §2.1.
2. **노드당 GPU 1개 × 2노드** → TP=2 의 collective 가 노드 간 NCCL(측정 실행은 `Socket` · IB 비활성)로 가고, 엔진은 custom all-reduce(MNNVL 부재)와 FlashInfer all-reduce(world_size=2)를 껐다 → 한 노드에 GPU 가 둘 이상인 HW 에서는 decode 거동이 다를 수 있다(이 계보에서 잰 적은 없다) — 01 §1.4(f) · plan_26090918 §5.
3. **섞인 양자화 + fp8 KV 의 QSA 경로** → 이 vLLM 에서는 자체이식 패치 60(가중치)과 64(QSA fp8 KV 읽기)가 없으면 기동하지 않는다. 64 는 참조가 sm_121 공유메모리 99KiB 벽 때문에 block_n 을 반감한 판본이라(64 헤더 — 참조의 관측) 다른 GPU 에서는 커널 타일 조건이 다시 정해진다. 커널 경로도 둘로 갈린다(주 experts `FLASHINFER_CUTLASS` NvFp4 · MTP `TRITON` Fp8 에 GB10 튜닝 파일 없는 기본 설정) — testlog_26091001 §3 · 01 §1.4(f).

**(나) 이 형상이 버티는 기전 대 다른 형상**(같은 모델 · 같은 NVFP4 · 262k — 다른 양자화 · 다른 모델 비교는 근거로 쓰지 않는다 · 상세는 02 §2.5)
- `nv4-f8-262k-res`(KV fp8 · 20,480 · gmu 0.85 · PLE 상주) — 이 셀과 PLE 방식만 다르다(상주 가중치 선언 63,266 대 38,852MiB) — r2 캠페인에서 로드 중 MemAvailable 10,190MiB 로 워치독 사살(관측 · 확정) · 상주 PLE 가 회수 불가능한 메모리라 로드 피크를 받치지 못한다는 기전은 plan 의 추론이라 가설 — testlog_26091114 비-measured 표 · plan_26091216 §2.3.
- **이 셀(KV fp8 · PLE mmap · KV 20,480 · gmu 0.85)** — 서빙 3회(2026-09-10 · 09-11 · 09-23) 모두 성립 — 예산 바닥이 넉넉한 것은 선언 산식상 확정 · mmap 페이지가 회수 가능한 page cache 라 워치독 여유가 된다는 설명은 페이지 종류를 잰 적이 없는 가설 — plan_26091216 §2.3 · testlog_26091001 §2.
- `nv4-bf-262k-mmp`(KV auto · 나머지 동일 · 같은 날 같은 이미지 D1) — 둘 다 서빙 성립 · KV dtype 만 달라 엔진 KV 토큰이 1,343,310 대 2,280,398 로 달랐다(관측) · 그 비가 출처의 토큰당 바이트 비와 맞지 않는 까닭은 미결 — 02 §2.5 · 02 Q6.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | NVFP4 mixed 체크포인트의 PLE(FP8)·MTP(FP8_PB_WO) 가중치가 stock 경로에서 로드되지 않는다 | 빌드 패치 60-qwen4exp-nvfp4-mixed.sh(pre · hunk A + B1/B2) | arch-invariant | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 2 | W2 | 스모크 1차가 로드 전에 게이트에서 멈췄다 | 스모크 스크립트 경로로 양 노드 예산 선언 + kv-cache-memory-bytes 고정 | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 3 | W3 | 스모크 2차에서 엔진이 기동 중 fail-loud 로 죽었다 | moe-backend 를 명시하지 않는다(auto) | arch-locked | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 4 | W4 | 스모크 3차가 moe-backend cutlass 명시로 실패했다 | moe-backend 를 명시하지 않는다(auto) | arch-invariant | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 5 | W5 | fp8 KV 스모크 1차가 그래프 캡처 구간에서 양 노드 동시 워치독 트립(docker kill) | enforce-eager: true(캡처 비활성) | arch-scaled | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-3 PASS |
| 6 | W6 | PLE 47.7GiB 가 상주 가중치에 들어가 노드당 KV · overhead 몫을 잠식한다 — stock 에 거둘 경로가 없다 | 빌드 패치 62-qwen4exp-ple-mmap.sh(TP=2 rank-aware masking 자체 설계) + VLLM_PLE_MMAP=1 + 노드 로컬 NVMe 스테이징 | arch-scaled | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-2 PASS · 양 rank PLE mmap 로그 |
| 7 | W7 | 62 패치 적용 이미지의 첫 스모크가 AttributeError 로 실패 | rng[0]/rng[1] 로 교정(62 패치 안) | arch-invariant | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-2 PASS |
| 8 | W8 | kv-cache-dtype fp8_e4m3 로는 stock QSA 경로가 기동을 거부한다 | 빌드 패치 64-qwen4exp-qsa-fp8kv.sh(가드 7곳 확장 + _cast_kv_tile 디콴트) | arch-locked | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-3 PASS |
| 9 | W9 | 첫 캠페인 이 셀의 부하 스윕이 3회 모두 죽어 lite 결과만 남았다 | r2 캠페인이 vllm bench serve 스윕으로 재측정해 5레벨 완주 · 이후 GuideLLM 벤치 몫을 예산 바닥과 잇는 진입 게이트(R3) 신설 | arch-scaled | testlog_26091114_qwen38fn_22셀재수행_7셀체크포인트_판정 §판정 결과 |
| 10 | W10 | lite 실행 ① 에서 서브 SSH probe 가 실패해 서브 열이 비었다 | lite_bench.sh 수정(fcb0754 — 따옴표 · 주석 제거) 후 lite 재실행 ② | arch-invariant | docs/simlog/26092310_hint_publisher_G3_D2_lite/d2_lite/lite_raw_nv4-f8-262k-mmp.json 의 sub probe_ok=true |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 실패 셀 컨테이너가 사라졌고 로그가 부족했다 — 미실행 16셀에도 사인이 적혔다 | 반증 | testlog_26091109_하네스교정_R0R10_역채점_음성대조 §5. R4 — 분류기는 옳았는데 **serve 실패 분기가 사살을 무시**했다 |
| M2 | 이 셀의 r2 판정이 floor 2.57 로 PASS 였다(측정이 합격선의 12배) | 반증 | testlog_26091412_하네스교정_항목1_acceptlen승계_소급재판정 §5.2 판정 불변 · 합격선 재산정 (spec on ∧ 실측 있음 ∧ 원 accept_len 1.0 — 11건) |
| M3 | lite 표의 서브 RAM · VRAM 칸이 늘 비어 "SSH probe 실패" 로 기재됐다 | 반증 | lite_bench.sh@fcb0754fffe1 §L185-187 · docs/simlog/26092310_hint_publisher_G3_D2_lite/d2_lite_2.log |
| M4 | fp8 KV 가 bf16 KV 와 토큰이 정확히 같지 않으면 실패로 분류한다는 검증 원칙 | 반증 | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §§1. ★ 중재 술어 재설계 — "토큰 정확 일치"는 이 스택에서 도달 불가 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.29.0rc6 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 74c96922ecb9017f413318c76d1af83aa2ab45a5 | output/multi/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 미관측 | 미관측 — 측정 digest 미관측 · 지위 `unobserved` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — 측정 경로에 wheel 배포 메타 관측자가 없다(sweep_index.meta.vllm_build=None 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/quant_models/Qwen/Qwen3.8-Flash-Next-NVFP4 → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-23T01:56:16Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.29.0rc6-cu133-aarch64-source | 셀 env(output/multi/envs/.env.nv4-f8-262k-mmp) IMAGE_TAG |
| 이미지 digest | 미관측 | 미관측 |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | output/multi/manifest.yaml(manifest_contract 로더) driver_version(스캔 시점 선언 — 측정 시각 값 미보장) |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

**(1) 빌드 트랙.** source-build 다. 해소 기록의 판정 문장은 "torch 2.13 >= 2.11 → 하드 ABI 벽(_C 를 NGC torch 에 링크) → source-build" 이고, 같은 기록은 `0.29.0rc6` prebuilt wheel 이 404(rc 미발행)라 wheel 을 소비하지 않는다고 적었다(`output/multi/resolved.json` `build_track.rationale` · `wheel.target_wheel_note`). 다른 트랙(prebuilt wheel)으로 갈 수 있는 조건도 그 문장에서 나온다 — 대상 vLLM 이 선언한 torch 가 2.11 미만이어서 그 ABI 벽이 성립하지 않고, 그 버전의 wheel 이 실제로 발행돼 있을 때다. 판정의 최종 중재는 어느 트랙이든 스모크다.

**(2) vLLM 버전 문자열과 생산자.** 빌드 입력 ref = `v0.29.0rc6`(docker history build-arg `VLLM_REF`) · 소스 SHA = `74c96922ecb9017f413318c76d1af83aa2ab45a5`(resolved.json · 양 노드 attestation 관측과 같다). 0.2 표의 `vllm_version` `0.29.0` 은 엔진 자기보고가 아니다 — 경량 리포트가 env 파일의 `IMAGE_TAG` 에서 파생한 값이다(bench_report_26092310_56_16 측정 환경 표 `vllm_version_source: derived(envfile IMAGE_TAG)`). 엔진 기동 배너 · 인증서 · wheel 메타는 이 발행에서 관측되지 않았다(위 표). 계보 plan 은 같은 SHA 로 지은 2026-09-09 이미지의 엔진 실체를 `0.29.0rc7.dev0+g74c96922e` 로 적었다(plan_26090918 §2.2 — 소스 빌드의 dev 문자열 · 이 이미지의 배너는 관측되지 않았다). **재현 좌표는 `v0.29.0rc6` 또는 위 40자 SHA** 다.

**(3) arch-locked 값과 재도출.**
- `TORCH_CUDA_ARCH=12.1a`(GB10 sm_121a): 네 GPU 의 compute capability(`nvidia-smi --query-gpu=compute_cap`)로 다시 정한다. 64 패치의 sm_121 공유메모리 조정과 post 패치 20 · 30 · 40 도 SM12x · GB10 을 전제로 한 것이라 다른 GPU 에서는 필요 여부부터 다시 본다.
- CPU arch `arm64`(aarch64): 네 호스트 플랫폼의 NGC 이미지 아키텍처로.
- NGC 베이스 `26.07-py3`(torch `2.13.0a0+9186a08` · 이미지 CUDA `13.3.1.008`): 대상 vLLM 의 torch 핀에서 NGC 태그를 접두로 해소하는 결정론 스크립트(upstream-version-watch `resolve_ngc_tag.py`)로 다시 푼다.
- 호스트 드라이버: 네 호스트의 `nvidia-smi` 값을 쓰고, 멀티노드면 노드 간 같은 값인지 attestation 으로 본다.

**(4) 선언 대 관측.** 드라이버가 갈라진다 — 표의 `580.173.02` 는 manifest 의 `driver_version` **선언**(스캔 시점 값 · 표 출처 칸)이고, 이 발행의 serve attestation 은 양 노드에서 `580.178.04` 를 **관측**했다(01 §1.5). 서빙 호스트의 실제는 관측값이다. torch 는 해소 핀 `2.13.0`(선언)과 이미지 `2.13.0a0+9186a08`(관측)이 접두 일치한다. 이미지 digest 는 이 발행 사실 블록에 실리지 않았다 — 발행 전 독립 사실 검증이 메인에서 `docker image inspect` 로 본 이 태그의 digest 는 `sha256:a2c4ca49…`(D1 메인 측정 digest 와 같다)이고, 이 발행의 스모크 로그(`d2_serve.log`)에는 빌드 단계가 없다(서브 digest 는 확인하지 않았다 · D1 에서는 노드마다 달랐다 — 로컬 빌드).

**(5) 환경 좌표.** 드라이버는 (4). 노드 메모리 총량 MemTotal 124,610 MiB(testlog_26091114 환경 스냅샷 · 이 발행의 예산 선언은 124,608). 인터커넥트는 plan 이 RoCE 196.88Gbps 로 적었고(plan_26090918 §1) 측정 실행의 NCCL 은 `NCCL_IB_DISABLE=1` · 네트워크 `Socket` 이었다(01 §1.4). OS 배포판 · 커널은 계보 문서에 미기록이다. 이 값들은 호스트 층이고 이미지 층(CUDA 13.3.1.008 · torch)과 섞지 않는다.

**(6) 핵심 의존 핀.** 이 모델 · 이 셀 고유의 요구는 의존 핀이 아니라 pre 패치 60 · 62 · 64(1.2)다 — 특히 64 는 fp8 KV 셀에서만 활성이다. 나머지는 공유 이미지의 기본값이다 — Ray `2.48.0`(Dockerfile `ARG RAY_VERSION` · 주석 "멀티 0.22.1 검증치, 상류 reqs 미선언") · flashinfer-python `0.6.18`(requirements.txt — 0.28.0 wheel 기준선 0.6.16.post3 을 대상 vLLM 0.29.0rc6 선언에 양보) · humming-kernels `0.1.12`(이 셀 경로에서 HUMMING 백엔드 미선택) · DeepGEMM nv_dev `a6b593d`(post 패치 10). Triton 은 NGC 베이스 번들을 쓰며 그 버전은 이 페이로드에 기록되지 않았다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 9개 — `inherited` 8 · `declared-requirement` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | manifest 토폴로지(노드 2 × 노드당 GPU 1)에서 정해진 값 — 이 계보에서 바꿔 잰 기록 0 | V1 |
| `distributed-executor-backend` | ray | declared-requirement | 없으면 vLLM 이 multiprocessing 경로로 가 'World size > available GPUs (1)' 로 죽는다고 serve_runner.sh 헤더가 기록했고 러너가 그 경우 기동을 멈춘다 | V2 |
| `gpu-memory-utilization` | 0.85 | inherited | P0-1 스모크 최종 설정에서 승계 · 이 셀에서 바꿔 잰 기록 0 · kv-cache-memory-bytes 를 준 이 구성에서 엔진은 KV 예약에 gmu 를 따르지 않는다고 로그에 적었다 | V3 |
| `max-model-len` | 262144 | inherited | 모델 네이티브 컨텍스트(YaRN 없이) · 캠페인 셀 축으로 선언된 통제변인 — 이 셀에서 조정 0 | V4 |
| `kv-cache-dtype` | fp8_e4m3 | inherited | 캠페인 매트릭스가 선언한 셀 축(kv=fp8) — 이 셀에서 대안과 비교해 고른 값이 아니다 · stock 은 이 값을 거부하므로 64 패치가 전제(W8) | V5 |
| `kv-cache-memory-bytes` | 21474836480 | inherited | 첫 캠페인 cell 4 트리플렛부터 승계된 손레버(lockset kv_source=hand) · fp8 262k 필요량 1,536MiB/노드를 크게 넘는다 · 스윕 0 | V6 |
| `enforce-eager` | true | inherited | W5(P0-1 · P0-3 fp8 KV 스모크의 그래프 캡처 트립) 이후 승계 통제변인 — 이 셀(mmp)에서 eager 를 끄고 잰 기록 0 | V7 |
| `async-scheduling` | false | inherited | 외부 선례의 MTP+async 금지 조합을 피하려는 명시(R4) — 우리 계보에서 켜서 실패한 관측은 없다 | V8 |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | inherited | 사람 결정(MTP 켠 채 · 2026-09-12)으로 고정된 통제변인 · plan 이 레버로 적은 MTP k 스윕은 이 셀에서 시도 0 | V9 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

- **① 네 환경에서 반드시 다시 도출할 값(arch-scaled)**: `kv-cache-memory-bytes`(V6) — 목표 컨텍스트 × 토큰당 KV 바이트로 필요량을 먼저 구하고(출처 산식: fp8 KV 262k 에 1,536MiB/노드 — 02 §2.5 인용) 그 위에 호스트 예산 바닥이 절대밴드를 넘는지로 상한을 정한다. `gpu-memory-utilization`(V3)은 KV 를 절대값으로 고정한 이 구성에서 엔진이 KV 예약에 따르지 않는다고 적었으니 로드 피크 최저 MemAvail 을 보며 다시 잰다. `max-model-len`(V4)은 네 목표와 YaRN 여부로, `tensor-parallel-size`(V1)는 네 manifest 의 노드 × GPU 수로 정해진다.
- **② 필요조건(declared-requirement)**: `distributed-executor-backend: ray`(V2) — 빼면 multiprocessing 경로로 가 'World size > available GPUs (1)' 로 죽는다고 `serve_runner.sh` 헤더가 기록했다. **명시하면 오히려 깨지는 노브**: `moe-backend` — triton 명시는 NVFP4 MoE 미지원으로, cutlass 명시는 60 패치 가드와 충돌로 실패했다(W3 · W4). 표에 없는 채로(auto) 둔다. `kv-cache-dtype: fp8_e4m3`(V5)은 필요조건이 아니라 셀 축이지만, 이 값을 쓰려면 64 패치가 필요조건이다(W8).
- **③ 조정된 적 없는 값(inherited · engine-default)**: `kv-cache-memory-bytes` 20GiB(V6)는 첫 캠페인 트리플렛부터 승계된 손레버로 fp8 필요량을 크게 넘는다 — 최적이 아니다. `enforce-eager`(V7)는 캡처 스파이크 트립(W5 — fp8 KV 스모크 포함) 이후 승계됐고 이 셀에서 끄고 잰 적이 없다. `async-scheduling: false`(V8)는 외부 선례의 금지 조합 회피용 명시다. MTP k=3(V9)은 사람 결정으로 고정된 통제변인이며 k 스윕은 0회다. 어느 것도 이 셀에서 대안과 비교돼 고른 값이 아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **vLLM 버전과 자체이식 패치의 필요성** → 대상 vLLM 소스트리에서 세 pre 패치의 트립와이어 needle 을 grep 한다(60: PLE mixed 라우트 · `FP8_BLOCK_SCALES` MoE 라우트 · `"quantized_layers",` 리맵 · 62: `ple_mmap` 모듈 · `qwen4_exp_ple_mmap_lookup` · 64: QSA 소유 파일의 `"fp8_e4m3"` · ops 의 `_cast_kv_tile`)와 PR #55513 머지 여부를 본다 → 전부 없으면 패치를 이식해 빌드(트립와이어가 빌드를 멈추면 그 패치는 빼고 stock 으로 스모크) · 일부만 있으면 해당 패치만 제거 · 모델 아치(`qwen4_exp`) 자체가 없으면 이 지도 폐기.
2. **메모리 구조와 노드 수** → manifest 의 노드 수 × 노드당 GPU · 통합메모리 여부 · MemTotal 을 본다 → 같은 통합메모리 2노드면 PLE mmap 경로 그대로 · 노드당 GPU 가 다르면 `serve_runner.sh` L79 의 GPU 합류 리터럴을 고치고 TP 를 재도출 · 이산 GPU 면 PLE 적재 방식을 다시 세운다(R3 재개조건).
3. **스테이징 · 마운트** → 양 노드에서 가중치 마운트 경로와 PLE 스테이징(로컬 NVMe · 같은 경로)이 실재하는지 파일 목록으로 확인 → 없으면 스테이징부터 · NAS 위 mmap 은 하지 않는다.
4. **호스트 메모리 안전 예산** → `floor = MemTotal − weights − kv − overhead` 에 네 값을 넣는다(weights = `(ckpt − PLE) ÷ tp` · KV dtype 은 weights 에 들어가지 않는다 · overhead 는 네 HW · 이 구성에서 used − weights − KV 로 다시 잰다) → 바닥이 절대밴드 + 가드 최소 위면 스모크 스크립트 경로로 선언 후 기동 · 아니면 KV 클램프부터 낮춘다(fp8 필요량은 bf16 의 절반이다 — 02 §2.5 인용). compose 직접 기동은 하지 않는다.
5. **기동 성공 판정** → health 200 + 추론 1회(스모크 스크립트의 serve proof) + 양 rank 엔진 로그의 `PLE mmap patch applied` · `PLE mmap: layer 1, 128 shards … tp_world 2` + 엔진 KV 크기 줄(`GPU KV cache size` — 이 줄은 dtype 을 찍지 않는다 · 같은 클램프의 kv auto 보다 토큰 수가 커야 한다)과 attention 백엔드가 QSA(`QWEN4_EXP_EXP_QSA_STATE`) 인지 + 멀티노드면 attestation 4축 equal → 하나라도 빠지면 이 지도의 해당 벽(W1 · W6 · W8)으로 돌아간다. 이 발행 실행에서는 적재 뒤 `shm_broadcast` 대기 경고가 60초 단위로 14회 찍혔고(01 §1.4) 서버가 뒤이어 떴다.
6. **정확성 · 성능** → fp8 KV 에 MTP 를 켠 조건의 정확성은 이 계보에서 검증되지 않았다(Q3) — 네 환경에서 kv auto 대비 술어 B 비교부터 한다. 이 페이로드의 18.77 t/s 는 lite 관측(반복 1)이다 → 네 환경에서 full 측정(GuideLLM × 반복 3)으로 새로 판정하고, cold 값은 새 기동 직후 첫 실행에서만 잰다(Q2).

실패 신호별 행선지: 빌드 · 버전 핀 · 패치 트립와이어 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · 예산 문제 → `vllm-recipe-explorer` · 성능 기대 이하 · 판정 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 · 서브 배달 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native/qnvfp4-len262144-kvfp8-plemmap-spec3-eager` |
| 형식 | `hint-payload/v6` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26092301-hint-v6-e2e · nv4-f8-262k-mmp · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26092301_hint_v6_e2e__cluster__nv4_f8_262k_mmp |
| work-manifest | docs/_evidence/camp_26092301_hint_v6_e2e__cluster__nv4_f8_262k_mmp.work-manifest.json |
| 증거 등급(task_class) | hint_map_only |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-09-23T01:19:47Z |
| 승인 발화(전사) | 사용자(발화 전사, G4) — D1 은 hint 퍼블리셔 E2E 이므로 성능하락 원인 분석 없이 REFUTE 로 처분·보존하고, 서빙은 성공했으므로 hint 태그를 발행한다. 이렇게 작업하고 나머지 캠페인(D2·N1)을 같은 방식으로 진행한다 |
| 현행 full 정의 충족(측정 등급) | 아니오 · 이 측정의 반복 1 · 정의 출처 `.claude/rules/docs.md:63`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 150건 · 지적 13건 중 고침 13건 · 열림 0 · 2026-09-23T02:52:32Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_CERTIFICATE` | 인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. 인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다. |
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

- **발행 경위(사실)**: 이 태그는 hint-publisher 재구성(plan_26092119)의 E2E 캠페인 `camp-26092301-hint-v6-e2e` 의 D2 셀 발행이다. D2 는 선언부터 lite-only 셀이라(캠페인 통제변인 "Docker lite-only · same image") 판정 · 인증서 없이 `hint_map_only` 로 발행된다. 사람(사용자)은 발행 확인 팝업(2026-09-23T01:19:47Z · 캠페인 선언 `hint_targets` 승인 전사 · 승인 셀 D1 · D2 · N1)에서 "D1 은 hint 퍼블리셔 E2E 이므로 성능하락 원인 분석 없이 REFUTE 로 처분·보존하고, 서빙은 성공했으므로 hint 태그를 발행한다. 이렇게 작업하고 나머지 캠페인(D2·N1)을 같은 방식으로 진행한다" 고 결정했다.
- **측정 경위(사실)**: lite 는 두 번 돌았다 — 실행 ① 은 측정 도구의 서브 probe 결함(W10)으로 서브 열이 비었고, 도구를 고친 뒤 실행 ② 가 리포트의 수치다. 실행 ② 의 cold 값은 실행 ① 직후라 cold 가 아니다(Q2).
- **옛 태그와의 관계(사실)**: 같은 셀 형상은 옛 이름 문법의 태그 `hint/0.29.0/qwen3.8-flash-next-nvfp4/gb10x2-cluster-native/len262144-kvfp8e4m3-plemmap`(2026-09-11 r2 · 38.18 t/s 인증서)으로 발행된 적이 있다(testlog_26091412 §5.2 10행). 이 태그는 v6 이름 문법의 새 판이며 옛 태그를 고치거나 대체하지 않는다(리콜 금지). 두 태그의 측정은 모드(full 대 lite) · 도구 레그 · 부하가 달라 나란히 놓을 수 없다(03 §3.5).
- **의견**: 이 발행의 가치는 lite 수치가 아니라 재현 키트 · 벽 지도 · 값의 지위다.
