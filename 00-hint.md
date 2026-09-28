# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plenone-spec7-graph` |
| 형식 | `hint-payload/v6` · 이름 문법 `v6` |
| 생성(UTC · 주입) | 2026-09-28T00:39:52Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26092808-ds4f-k70-roce · ds4f0731-1m-spec7-roce · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_ds4f0731-1m-spec7-roce.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

DeepSeek-V4-Flash-0731(dense FP8 블록 · expert fp4)을 GB10 GPU 1개 × 노드 2개 · Ray TP=2 · vLLM v0.29.0rc6 소스빌드로 1048576 컨텍스트 · fp8_ds_mla KV · PLE 없음 · dspark spec k=7 · cudagraph 형상으로 서빙해 동시성 1 decode 32.2 t/s(GuideLLM full · explore PASS)를 쟀다.
가장 비싼 벽은 커널 업그레이드 뒤 RoCE 실패로 Socket 에 묶였던 NCCL 전송을 manifest 선택(rdma = `NCCL_IB_DISABLE=0` · `NCCL_NET=IB`)으로 되살린 것(W11 · W12)과, 호스트 기동 밸리가 KV 클램프를 10 GiB 로 누른 것(W7)이다.
가장 오해하기 쉬운 값은 `kv-cache-memory-bytes` — KV 필요량이 아니라 기동 밸리가 정한 값(V7)이고, `max-num-seqs` 1 은 손레버 승계(V8)이며, tool call 용처는 `enable-auto-tool-choice` 가 없어 거부된다(Q3).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | deepseek-v4-flash-0731 | 인증서(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) |
| HF repo | deepseek-ai/DeepSeek-V4-Flash-0731 | 체크포인트 .git/config remote url(huggingface.co · /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) |
| HF revision(체크포인트 git HEAD) | 7872f01b1d1fe23eabc4c98b48bffcef5a386062 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-28T00:35:13Z 이전 마지막 이동 2026-08-31T07:23:00Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) |
| base 슬러그 | deepseek-v4-flash-0731 | model(deepseek-v4-flash-0731) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | fp8 | 인증서(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) |
| GPU | NVIDIA GB10 | 인증서(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) |
| 토폴로지 · TP | multi · TP=2 | 인증서(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.29.0 | 인증서(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml) · 생산자 `image-tag-line`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행) |

**명명 축** — 태그 이름은 도구가 이 축들에서 전량 파생했다(발행자 입력 ✗).

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.29.0rc6 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.29.0rc6(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | deepseek-v4-flash-0731 | 서빙 yaml model(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · hf_repo(deepseek-ai/DeepSeek-V4-Flash-0731) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qfp8-len1048576-kvfp8-plenone-spec7-graph | derived(q·len·kv·ple·spec·graph) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · 인증서 gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `q` | fp8 | /app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731/config.json quantization_config quant_method=fp8 · vocab quant[fp8]←'fp8' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · vocab kv[fp8]←'fp8' |
| 축 `ple` | none | 모델 config PLE 키['ple_layer_ids', 'ple_embed_dim'] 부재 관측(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · vocab ple[none]←'none' |
| 축 `spec` | 7 | 서빙 yaml speculative-config.num_speculative_tokens(output/multi/configs/ds4f0731-1m-spec7-roce.yaml · method=dspark) |
| 축 `graph` | graph | 서빙 yaml enforce-eager: false(output/multi/configs/ds4f0731-1m-spec7-roce.yaml) · vocab graph[graph]←'graph' |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 조건에서만 성립하는 것.**
- 통합메모리(노드당 한 풀 · 선언 입력 `mem_total_mib=124608`) → 기동 중 가용 급락(기동 밸리)과 KV 몫이 같은 풀을 눌러, KV 클램프의 바인딩이 KV 필요량이 아니라 호스트 기동 밸리가 된다(밸리의 구성이 페이지캐시 적층이라는 서술은 계보 밖 devlog_26090912 · 미결) → 노드 메모리가 더 크거나 GPU 메모리가 분리된 HW 에서는 클램프 · 동시성 상한이 전혀 달라진다(testlog_26090912 §판정 · W7).
- 노드당 GPU 1개 · TP=2 → 모든 텐서 병렬 통신이 노드 간 RoCE 를 탄다. 이 셀은 커널 `7.0.0-1019-nvidia` 위에서 NCCL 내장 IB(`Using network IB` · `GDR 0`)로 성립했고, 드라이버는 DMA-BUF/GDR 미지원을 보고했다 → GDR 이 켜지는 드라이버 · 이미지 조합이나 다른 NCCL 전송(Socket · 외부 플러그인)에서는 이 성립이 옮겨지지 않는다(testlog_26092808 §2 · §3 · testlog_26092807 §1).
- sm_121a(SM120 계열) → DeepseekV4 어텐션이 fp8_ds_mla 페이지 레이아웃을 강제해 KV 는 fp8 하나만 선다(아키텍처 기전 · W5) → 다른 아키텍처는 어텐션 후보가 다르다(testlog_26090904 §3). MoE 는 아키텍처 기전이 아니다 — triton 은 MXFP4 TRITON 커널의 SILU 활성 미지원으로 거부됐고 auto 는 이 계보 재시도 0(옛 계보 MARLIN OOM 전력)이라, 시도한 두 후보 중 humming 만 남았다(sweep_map_26090912_b768k_levers · W8 · R3 · R4 · cutlass 등은 시도 0).

**(나) 이 형상이 버티는 기전 대 다른 형상.**
- KV 클램프 18 GiB(768K) — 벤치 상주분 미산입으로 절대 플로어에 걸려 사살 1회 — 확정(관측) — `docs/devlog/devlog_26090912_ds4f0731_광의탐색_셀루프_서사.md` §수렴의 3단계(계보 밖).
- KV 클램프 14 GiB(768K) — 기동 밸리가 KV 몫과 겹쳐 가용이 절대 플로어 아래로 — 사살 2회 · 10 GiB 생존 대조로 확정 · 밸리 구성(페이지캐시 적층)은 계보 밖 devlog 서술로 미결 — testlog_26090912 §판정 · W7.
- kv auto · turboquant — KV dtype 만 다름 — 커널 페이지 포맷(fp8_ds_mla 강제) 때문에 기동 자체가 assert — 확정 — testlog_26090904 §3.
- moe triton — MoE 백엔드만 다름 — TRITON MXFP4 커널이 SILU 활성을 지원하지 않아 엔진 거부 — 확정 — sweep_map_26090912_b768k_levers §셀.
- spec off · eager(1M E0-base) — spec 과 graph 두 노브가 다름 — 매 토큰 forward 1회 · 그래프 없이 커널 런치 비용을 치러 동시성1 decode 가 절반가량 — 확정(측정) · 기전 서술은 추론 — sweep_map_26090912_e1m_levers §셀.
- NCCL `IB_DISABLE=1` + `NET` 미방출(이 셀 시도 1) — 전송 설정만 다름 — 내장 IB 가 꺼지고 SPCX 플러그인이 장치를 건너뛰어 Socket 으로 떨어짐 — 관측은 확정 · Socket 으로 떨어진 기전은 추론 — testlog_26092808 §1 · W12.
- 이 형상 — 위 차이를 **모두** 반대로 둔 것(KV fp8 · 클램프 10 GiB · humming · spec+graph · 내장 IB 명시) — 선언 바닥 21,479 MiB 위에서 사살 0 · `Using network IB` · iova2 0 — 확정 — testlog_26092808 §2.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 1차 소스빌드가 양 노드에서 의존 해소 단계에서 실패했다 | constraint 를 상류 선언(0.6.18)에 양보 — requirements.txt 머리 주석 · 2차 빌드 양 노드 PASS | arch-invariant | testlog_26090904 §1 2차 빌드 PASS · 프로브 flashinfer 0.6.18 |
| 2 | W2 | 멀티 스모크가 기동 전에 rc=2 로 두 번 멈췄다(컨테이너 이름 · executor backend) | 관례 <cell>-master/slave-container 보수 · yaml 에 tensor-parallel-size 2 + distributed-executor-backend ray 명시 | judgment | testlog_26090904 §4 스모크 PASS(rc=0) |
| 3 | W3 | 멀티 스모크가 예산 선언 단계에서 rc=4 로 로드 전에 멈췄다 | hint 실측값을 시드로 선언하고 멀티 실측으로 재산정하는 경로 확정 | judgment | testlog_26090904 §4 예산 선언→honored→up 순서 ✓ |
| 4 | W4 | master 컨테이너가 기동 직후 즉사했다 | 주석 전행 분리 · 검증 추가 | arch-invariant | testlog_26090904 §4 스모크 PASS |
| 5 | W5 | kv-cache-dtype auto 트라이얼에서 엔진이 기동 중 assert 로 죽었다 | kv-cache-dtype fp8 고정 · layer-1 개정 kv_dtype_axis=["fp8"](사람 승인) · auto/turboquant 셀 void | arch-locked | testlog_26090904 §3 · 이 셀 lite_engine 로그 L275 fp8_ds_mla 선택 |
| 6 | W6 | 벤치 셀 진입이 rc=6 으로 거부됐다 | 교정(warn 97·hard 99 · 커밋 f02990c) · 양 노드 배포 | arch-locked | testlog_26090912 운영 사건 표 |
| 7 | W7 | 768K 셀 클램프 14 GiB 에서 기동 직후 memwatch 가 두 번 컨테이너를 사살했다(18 GiB 사살 1회는 벤치 상주분 미산입으로 별개) | kv-cache-memory-bytes 10 GiB 수렴 · 벤치 전 페이지캐시 드롭 배선 | arch-scaled | testlog_26090912 운영 사건 표 · 이 셀 기동 시도 2 사살 없음(01 §1.4) |
| 8 | W8 | moe-backend triton 셀이 서빙 전에 실패했다(rc=3 · serve_failed) | moe-backend humming 유지 | arch-locked | sweep_map_26090912_b768k_levers §셀 L4-combo measured(humming) |
| 9 | W9 | GuideLLM 레그가 이 모델 토크나이저를 파싱하지 못했다 | run_bench.sh 의 토크나이저 스테이징(tokstage) 배선 수정 | arch-invariant | testlog_26090912 운영 사건 표 · 이 셀 full 스윕 완주(03 §3.1) |
| 10 | W10 | 1M 기본 셀 측정이 기동 전 발사돼 serve_failed · 상주 뒤 갱신 루프가 다음 선언과 교착 | 재측정 · 정규 --down 회수 절차(컨테이너 · 워치독 · 캐시 · 예산 선언을 함께 회수) | judgment | sweep_map_26090912_e1m_levers E0-base measured(재측정) · testlog_26090912 운영 사건 표 |
| 11 | W11 | 커널 7.0 위 멀티 TP=2 가 RoCE 경로에서 실패해 Socket 기준선으로 후퇴했다(09-17) | 렌더러 전송 선택 manifest.interconnect.nccl_transport(socket 기본 · rdma) 도입(2e91a1e → 8319b4f) · 서브는 sync_to_sub 재렌더 배달 | judgment | testlog_26092808 §2 Using network IB · iova2 0건 |
| 12 | W12 | rdma 로 선언한 기동 시도 1 이 양 노드에서 Socket 전송으로 떨어졌다 | rdma = NCCL_IB_DISABLE=0 · NCCL_NET=IB 명시(8319b4f) · 시도 1 은 --down 정식 회수 | judgment | lite_engine_ds4f0731-1m-spec7-roce.log L322 Using network IB |
| 13 | W13 | 벤치 r1 스윕이 부하 전에 measurement_void 로 끝났다 | --backend openai-chat(09-09 스윕이 쓴 기본과 같은 포맷)으로 r2 재초기화 · 측정 | arch-invariant | sweep_map_26092809_ds4f0731_k70_roce_1m 셀 measured · 03 §3.1 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 09-09 E1-combo(1M spec+graph)가 floor 13.2 로 verdict PASS | 반증 | testlog_26091412 §5.1 |
| M2 | 이 셀의 기동 시도 1 설정을 09-09 PASS 의 NCCL 조건이라고 적었다 | 반증 | git show d8c7b78(render_dockerfile.py NCCL_IB_DISABLE "0"→"1") · 09-09 lite_engine_e-1m-kvfp8-e1combo.log 의 NCCL_IB_DISABLE set by environment to 0 · Using network IB |
| M3 | 측정 이미지의 재빌드 날짜를 09-24 로 적었다 | 반증 | 01 §1.4 FACT:reproduce 빌드 층 시각 |
| M4 | 체크포인트 양자화를 FP8 block MoE 로 기술했다 | 반증 | lite_engine_ds4f0731-1m-spec7-roce.log L277 · 체크포인트 config.json expert_dtype |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.29.0rc6 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | 74c96922ecb9017f413318c76d1af83aa2ab45a5 | output/multi/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 미관측 | 미관측 — 이 측정 로그는 꼬리 캡처라 배너가 없고(sweep meta.vllm_build 도 NA) 같은 digest 를 기록·추론할 수 있는 다른 로그에도 배너가 없다 · 지위 `unobserved` |
| 인증서 `vllm_version`(강한 일치 키) | 0.29.0 | 생산자 `image-tag-line` · sweep_bench.sh@a21e66eb26e5 L522·L555: 선언 IMAGE_TAG (sweep_index.meta.image_tag_declared easy-vllm:0.29.0rc6-cu133-aarch64-source)에 그 판본의 정규식 `easy-vllm:([0-9]+\.[0-9]+\.[0-9]+)` → 0.29.0 = sweep_index.meta.vllm_version(인증서는 그 옮김) — **엔진 자기보고가 아니다**(빌드 입력 이미지 태그를 x.y.z 로 자른 값 · rc/dev 접미가 사라진다). 더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L553)는 측정 기록에 남지 않아 배제할 수 없다 |
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — 측정 경로에 wheel 배포 메타 관측자가 없다(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | 7872f01b1d1fe23eabc4c98b48bffcef5a386062 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731 → <manifest.nas_model_path>/DeepSeek/DeepSeek-V4/DeepSeek-V4-Flash-0731) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-28T00:35:13Z 이전 마지막 이동 2026-08-31T07:23:00Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.29.0rc6-cu133-aarch64-source | 셀 env(output/multi/envs/.env.ds4f0731-1m-spec7-roce) IMAGE_TAG |
| 이미지 digest | sha256:a2c4ca499fb2473191bcdfc7e192445750b4a7b6803cfe14e50b43f401000544 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | 인증서 driver_version(benchmark_26092809_deepseek-v4-flash-0731_GB10_0.29.0.yaml · 측정 기록) |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build.** 첫 plan 이 이유를 적었다: 대상 vLLM 이 핀한 torch 2.13.0(≥2.11)이 NGC 베이스 torch 와 prebuilt wheel ABI 가 맞지 않는다(plan_26090819 §2 build track 행). 소스빌드는 NGC 베이스의 torch 를 그대로 쓰므로 torch 접두 일치 불변식이 적용되지 않고, 최종 중재는 스모크다. 다른 트랙으로 갈 조건은 그 문장에서 파생된다 — 대상 vLLM 이 선언한 torch 와 베이스 torch 의 wheel ABI 가 맞을 때만 wheel 이 후보가 된다.

(2) **버전 문자열 넷.** 빌드 입력 ref `v0.29.0rc6`(docker history build-arg — 빌드한 사람이 넣은 값) · 소스 SHA `74c96922ecb9017f413318c76d1af83aa2ab45a5`(resolved.json) · 인증서 강한 키 `vllm_version` 0.29.0(측정 도구가 **이미지 태그에서 잘라 만든 값** — 엔진 자기보고가 아니고 rc 접미가 사라졌다) · 엔진 기동 배너(미관측 — 꼬리 캡처). 계보의 빌드 testlog 는 이미지 안 프로브를 `0.29.0rc7.dev0+g74c96922e`(=rc6 태그 커밋)로 기록했다(testlog_26090904 §1 — 09-09 빌드 이미지 · 이 셀 digest 와 다르다). **재현 좌표는 릴리스 태그 `v0.29.0rc6` 또는 40자 SHA** 다.

(3) **arch-locked.** `TORCH_CUDA_ARCH=12.1a`(GB10 sm_121a — 네 GPU 의 compute capability 로 재도출: `torch.cuda.get_device_capability()`) · CPU arch arm64(aarch64 — `uname -m`) · 드라이버 580.173.02(호스트 `nvidia-smi` · 네 호스트 드라이버가 이미지 CUDA 13.3 을 지원하는지부터 확인) · 커널 `7.0.0-1019-nvidia`(호스트 `uname -r` — 이 셀의 성립 조건이지만 인증서 키가 아니다). NCCL 전송의 자리(HCA · GID · iface)는 manifest `interconnect` 에서 다시 파생한다.

(4) **선언 대 관측.** vLLM SHA 는 resolved.json(선언)이 출처이고, 이미지 안 프로브가 같은 커밋을 보였다는 관측은 09-09 빌드 이미지의 것이다(testlog_26090904 §1). 이 셀의 측정 이미지는 digest `sha256:a2c4ca49…` 로 09-09 digest 와 다르며(03 §3.5), 층 시각상 2026-09-22 에 다시 지어졌다(01 §1.4). 양 노드 vLLM SHA 가 같다는 관측은 attestation 의 것이다(범위 주의 · Q7). 둘 사이에 다른 커밋이 쓰였다는 증거는 없다.

(5) **환경 좌표.** 드라이버 580.173.02(인증서 · 호스트 층) · 호스트 OS 커널 `7.0.0-1019-nvidia`(testlog_26092807 §1 · 호스트 층) · 노드 메모리 총량 124,608 MiB(예산 선언 입력 · 01 §1.4) · 인터커넥트 RoCE v2 · 2포트(plan_26090819 §2 "RoCE 98.41Gb/s/포트 검증済" · testlog_26092807 유효맥락 GID 3). 이미지 층은 위 표(CUDA 13.3.1 · torch 2.13.0a0 · NGC 26.07). 호스트 CUDA 는 manifest 선언 `cuda_version` 132 · `nodes[].cuda_version_raw` 13.2(인증서 소프트 지문 `cuda_version` 132 가 이 선언값 — 측정 아님)이고, testlog_26092807 §1 은 host libcuda 13000 · compat 13030 을 기록한다.

(6) **핵심 의존 핀.** Ray `2.48.0`(Dockerfile ARG `RAY_VERSION` · 멀티 Ray TP 가 쓴다 — 공유 이미지 기본) · flashinfer-python `0.6.18`(requirements.txt — constraint 를 상류 선언에 양보한 값 · W1) · humming-kernels `[cu12]==0.1.12`(requirements.txt — 이 모델의 MoE 백엔드 · V9 · 40-humming-nvml-gb10.sh 가드 대상) · DeepGEMM `nv_dev` a6b593d(10-deepgemm.sh — 이 모델의 dense FP8 블록 GEMM 경로 · 공유 이미지의 arch-enablement) · NCCL 2.30.7(이미지 동봉 · 엔진 로그 `nccl:version`). requirements.txt 바이트가 이미지에 쓰인 것인지는 관측되지 않았다(01 §1.1).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 13개 — `tuned` 3 · `inherited` 6 · `declared-requirement` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | declared-requirement | W2 — 멀티 트리플렛에서 TP·ray backend 가 빠져 스모크가 rc=2 로 멈췄고 명시로 넘었다 · 값 2 는 manifest 파생(노드 2 × GPU 1) | V1 |
| `distributed-executor-backend` | ray | declared-requirement | W2 — 누락 시 rc=2(yaml 주석: 없으면 mp 로 가서 World size>GPU(1)) | V2 |
| `gpu-memory-utilization` | 0.85 | inherited | 셀 config target_gmu 선언을 09-09 태그에서 승계 · 두 레버 스윕의 전 셀이 같은 값(바꿔 잰 기록 0) · 뺐을 때의 실패 관측 없음 | V3 |
| `max-model-len` | 1048576 | inherited | 사용자 지정 컨텍스트 축(768K · 1M 두 값)의 1M · 모델 네이티브 한계와 같다 · 이 셀에서 조정 0 | V4 |
| `quantization` | fp8 | inherited | 체크포인트 quantization_config(fp8 블록)와 같은 값을 명시한 승계 · 다른 값을 시도한 기록 없음(expert 는 fp4 — M4) | V5 |
| `kv-cache-dtype` | fp8 | declared-requirement | W5 — auto 는 엔진 assert 로 거부 · turboquant 도 같은 경로로 구조적 거부(fp8_ds_mla 유일) | V6 |
| `kv-cache-memory-bytes` | 10737418240 | tuned | 계보 768K 셀에서 18→14→10 GiB 로 바꿔 가며 잰 기록(W7 · 18 GiB 사살 1회 · 14 GiB 사살 2회 뒤 수렴) · 이 셀은 그 10 GiB 를 승계 · 1M 요청 하나 대비 엔진 보고 1.85x | V7 |
| `max-num-seqs` | 1 | inherited | 손레버(lockset batch_source=hand-lever) — yaml 주석의 1.73x 를 내림한 값 · 스윕 기록 0 | V8 |
| `moe-backend` | humming | declared-requirement | W8 — triton 으로 바꾸면 엔진 거부(SILU 미지원) · auto 는 MARLIN repack OOM 전력(20-triton-kernels.sh 헤더 · 이 계보 재시도 0) | V9 |
| `enforce-eager` | false | tuned | 768K 스윕에서 eager(L0 17.11) 대 cudagraph(L2 26.14) 를 바꿔 잰 기록 · 1M 은 결합 셀로만 재어 승계 | V10 |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":7} | tuned | spec on/off 를 바꿔 잰 기록(768K L0 17.11 → L1 29.72 · 1M E0 16.67 → E1 30.54) · k=7 자체는 스윕 0(dspark_block_size 5 이상 조건의 09-09 검증값) | V11 |
| `reasoning-parser` | deepseek_v4 | inherited | Hermes 용처로 09-09 레시피가 등록명을 확인해 넣은 값의 승계 · 다른 값 시도 0 · 이 셀 상주 chat 에서 reasoning 분리 관측(testlog_26092808 §5.) | V12 |
| `tool-call-parser` | deepseek_v4 | inherited | 09-09 레시피 승계 · 단독으로는 tool_choice auto 를 받지 못한다(enable-auto-tool-choice 부재 → HTTP 400 관측 · Q3) | V13 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 반드시 다시 도출할 값.** `kv-cache-memory-bytes`(V7 · tuned 이지만 바인딩은 호스트 기동 밸리) — 먼저 예산 선언(바닥 = 총량 − 가중치 − KV − overhead)을 세우고 작은 클램프로 띄워 기동 중 가용 최저점을 잰 뒤 올린다 · 엔진 로그 `GPU KV cache size` 줄의 토큰 수 ÷ 목표 컨텍스트로 동시성을 본다. `max-num-seqs`(V8) — 그 비를 내림해 정한다. `gpu-memory-utilization`(V3) — 통합메모리 타겟 선언값이지 측정 결과가 아니다. `max-model-len`(V4) — 용처의 컨텍스트 요구. `tensor-parallel-size`(V1)는 manifest 의 노드 × GPU 에서 파생한다.

**② 필요조건으로 확인된 값.** `kv-cache-dtype` fp8(V6 · W5 — auto · turboquant 는 기동 assert) · `moe-backend` humming(V9 · W8 — triton 은 엔진 거부 · auto 는 MARLIN repack OOM 전력) · `tensor-parallel-size` 2 와 `distributed-executor-backend` ray(V1 · V2 · W2 — 멀티에서 빠지면 rc=2). 이 표 **밖**의 전송 노브: NCCL 은 `NCCL_IB_DISABLE=0` · `NCCL_NET=IB` 를 명시해야 RoCE 가 선다 — `IB_DISABLE=1` 을 두면 조용히 Socket 이 된다(W12). tool call 용처라면 이 셀에 **없는** `enable-auto-tool-choice` 를 더해야 한다 — 없으면 `tool_choice:"auto"` 가 HTTP 400 으로 거부된다(관측 · Q3 · 성능 영향은 없을 것으로 추정 · 미측정).

**③ 조정된 적 없는 승계값.** `max-num-seqs` 1(V8 · 손레버 — 최적이 아니며 동시성 2 이상에서 요청당 decode 가 나뉜다) · `gpu-memory-utilization` 0.85(V3) · `quantization` fp8(V5 — 체크포인트 선언의 반복) · `reasoning-parser` · `tool-call-parser` deepseek_v4(V12 · V13 — 등록명 확인 뒤 넣은 값). 이 값들은 이 셀에서 바꿔 본 적이 없다 — 필요조건으로 복사하지 말고 네 용처에서 다시 고른다. `speculative-config` 의 k=7(V11)도 k 축은 스윕되지 않았다(on/off 만 잰 tuned).

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **NCCL 전송** → 기동 로그에서 `Using network IB` 줄과 `iova2` · `Cannot allocate memory` · `NV_ERR` 부재를 본다(양 노드 · Ray 접힘 주의) → IB 면 그대로 · Socket 이면 manifest `interconnect.nccl_transport: rdma` 로 재렌더 · 배달 후 재기동(W12) · iova2 가 나면 이 지도 폐기하고 커널 · 드라이버 조합부터 다시 본다(W11 · Q1).
2. **GDR 상태** → 로그 `Connected all rings, use ring PXN 0 GDR 0` 의 GDR 값 · 드라이버의 `DMA_BUF_SUPPORTED` 조회 → GDR 0 이면 이 셀과 같은 경로 · GDR 이 켜지면 이 셀의 미재현 판정이 옮겨지지 않는다 — 재측정(Q2).
3. **KV 경로 · MoE 경로** → 로그 `Using DeepSeek's fp8_ds_mla KV cache format.` · `Using 'HUMMING' Mxfp4 MoE backend.` → 같으면 그대로 · 상위 vLLM 에서 다른 KV 포맷 · MoE 백엔드가 허용되면 V6 · V9 를 다시 잰다(R1 · R3 재개조건).
4. **공유 패치의 필요** → 상위 vLLM 에서 10-deepgemm(vLLM 자체 deepgemm 핀이 SM120 실행커널을 담게 됐는지 — 헤더의 probe 는 오버라이드한 nv_dev 소스만 보므로 이를 감지하지 못한다 · 자동 트립와이어 없음 · 사람이 상류 핀을 확인) · 40-humming-nvml(humming 이 NVML NotSupported 를 스스로 처리하는지) 을 확인 → 필요 없어졌으면 빼고 스모크 · 필요하면 유지. qwen4exp 패치 3개와 20 · 30 은 이 모델에서 불활성으로 **추론**된다(발화 서명 없음 · 01 §1.2).
5. **기동 성공 판정** → health 200 + chat 추론 1회 + 위 1 · 3 의 로그 서명 · (tool call 용처면) tools + `tool_choice:"auto"` 요청 1회 → 셋 다 통과해야 성립 · tool 요청이 400 이면 `enable-auto-tool-choice` 추가(Q3).
6. **호스트 메모리 안전 예산** → 예산 선언 없이 기동하지 않는다 · 입력은 노드 총량 · 체크포인트 ÷ TP · KV 클램프 · overhead(이 셀 선언 13312 MiB) → 기동 중 워치독 트립이 나면 클램프를 줄이고(W7) 선언을 다시 세운다.

실패 신호별 행선지: 빌드 · 버전 핀 · 공유 패치 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · tool call 플래그 → `vllm-recipe-explorer` · 성능 기대 이하 → `adversarial-benchmark` · NCCL 전송 · 인터커넥트 · 커널 · 노드 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10-1g2n-cluster-native/qfp8-len1048576-kvfp8-plenone-spec7-graph` |
| 형식 | `hint-payload/v6` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26092808-ds4f-k70-roce · ds4f0731-1m-spec7-roce · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26092808_ds4f_k70_roce__cluster__ds4f0731_1m_spec7_roce |
| work-manifest | docs/_evidence/camp_26092808_ds4f_k70_roce__cluster__ds4f0731_1m_spec7_roce.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-09-27T23:25:32Z |
| 승인 발화(전사) | 사용자(선언 확인 팝업 전사) — "발행 승인 (셀 모두)": full bench 끝난 셀을 hint 태그로 발행·push 승인, Socket 대체 셀 발생 시 포함 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:63`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실검증 Agent(general-purpose)`(저작자 `hint 저작 Agent(fork)` 아님) · 주장 164건 · 지적 17건 중 고침 17건 · 열림 0 · 2026-09-28T01:06:26Z |
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

**발행 경위(사실).** 이 태그의 셀은 09-09 캠페인 셀 `e-1m-kvfp8-e1combo`(옛 태그 `hint/0.29.0rc6/deepseek-v4-flash-0731/gb10x2-cluster-native/spec7-graph1-len1m`)의 트리플렛을 셀 id 만 바꿔 다시 띄운 통제된 재현이다. 옛 태그의 verdict PASS 는 09-14 소급 재판정에서 accept_len 자리표시자 결함으로 뒤집혔고(M1), 옛 태그는 불변이라 고치지 않았다 — 이 태그가 교정된 체인의 재측정이다. 인증서는 explore PASS 라 자동 발행되지 않아(자동 발행은 explicit ∧ PASS) 발행기를 직접 불러 냈다. 벤치 1차 스윕(r1)은 호출자 `--backend` 누락으로 부하 전 무효였다(W13). tool call 거부(Q3)는 벤치 뒤 상주 서빙에서 관측한 것이며 측정 조건에 영향이 없다.

**알려진 오기(사실).** "09-09 경로 = IB_DISABLE=1 · IBext"(M2)와 "09-24 재빌드"(M3)는 틀렸다. testlog_26092808 §1(★정정 항목)과 testlog_26092807 §3 은 2026-09-28 페이로드 저작 직후 정정됐다. 정정되지 않은 채 남은 자리: plan_26092808 §2 NCCL 행 · 커밋 2e91a1e 메시지 · devlog_26092809 §무엇을 했나 3('09-24 재빌드') — 렌더러 8319b4f 주석은 ab1fade 에서 정정됐다. 이 페이로드의 M 블록이 정정 기록이다.

**의견.** 이 셀은 "커널 7.0 에서 RoCE 가 깨진다" 는 보고가 이 워크로드 · 이 이미지에서는 재현되지 않는다는 근거이며, 커널 다운그레이드 결정의 근거로만 좁게 써야 한다(GDR 경로를 타는 조합은 별개).
