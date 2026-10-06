# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8-spec3-densegemv-moeprep` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-06T01:00:31Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26100320-v41flash-2node-broad · i5-a-kern · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_i5-a-kern.json` — 시점 범위: 작성 2026-10-06T02:15:45Z(output/multi/benchlog/serve_proof_i5-a-kern.json 파일 mtime(파일시스템 관측 — 문서에 시각 칸이 없다 · 기록 필드 아님)) · 측정 창 대비 작성 2026-10-06T02:15:45Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 스모크 산출물 — 측정을 서빙한 기동의 자격 관측이 아니다 |
| 증거 등급(task_class) | hint_map_only |
| 측정 등급(bench_mode) | lite |
| 성능 판정(measurement.verdict) | `OBSERVATION-ONLY` — 출처 lite-only 셀 — verdict_rule 에 투입되지 않았다(경량 리포트 `inform-only` · 판정 원천 없음 · benchmark_mode=lite · report_kind=lite) |

> **OBSERVATION-ONLY** — 이 태그의 성능 수치는 **관측 게재**다(인증서 없음). baseline·권고로 읽지 마라 — baseline 을 주장하려면 full_benchmark 인증서가 필요하다.
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

DeepSeek-V4.1-Flash EXL3 2.0bpw(sfxnz 팩 A)를 GB10 GPU 1개 × 노드 2개 · Ray TP=2 · vLLM v0.30.0 소스빌드(SM12X_PORT=1 이식 이미지) · max-model-len 1048576 · fp8 KV · PLE 없음 · DSpark spec k=3 · cudagraph 로 서빙해 lite warm 동시성 1 gen 44.74 t/s(lite 스냅샷 · 반복 1 · OBSERVATION-ONLY)를 쟀다.
가장 비싼 벽은 CUDA graph 전부 logits NaN(W4 — 원인은 KV null 블록 오염 · PRE 89 로 해소)과 참조 런타임의 두 배가 넘던 스텝 비용(W6~W9 — 저자 커널 레버를 핀 커밋 그대로 빌드 슬롯 POST 73 · 74 · PRE 90 · 91 · 92 로 이식)이다.
가장 오해하기 쉬운 값은 `kv-cache-memory-bytes`(V9 — 필요량 역산이 아닌 손레버 승계값이고 이 값이 있으면 gmu 는 KV 를 정하지 않는다)이며, Engram prestage(88)는 graph NaN 의 처방이 아니다(M1 반증).

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | 인증서(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) |
| HF repo | 미관측 | 체크포인트 .git/config 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — HF repo 미관측 |
| HF revision(체크포인트 git HEAD) | 미관측 | 체크포인트 .git/HEAD 부재(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) — revision 미관측 |
| base_model | deepseek-ai/DeepSeek-V4.1-Flash | 체크포인트 README base_model(/app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) |
| base 슬러그 | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | model(deepseek-v4.1-flash-exl3-2.0bpw-sfxnz) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | exl3 | 인증서(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) |
| GPU | NVIDIA GB10 | 인증서(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) |
| 토폴로지 · TP | multi · TP=2 | 인증서(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.30.0 | 인증서(benchmark_26100516_deepseek-v4.1-flash-exl3-2.0bpw-sfxnz_GB10_0.30.0.yaml) · 생산자 `unverified`(0.4 표 `인증서 vllm_version` 행 — 엔진 자기보고는 그 표의 다른 행) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.30.0 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.30.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | deepseek-v4.1-flash-exl3-2.0bpw-sfxnz | 서빙 yaml model(output/multi/configs/i5-a-kern.yaml) · model_path basename(DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz) 소문자 = 인증서 model 키 규칙 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qexl3-len1048576-kvfp8-spec3-densegemv-moeprep | derived(q·len·kv) + tail(agent · naming.tail[] 근거) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 캠페인 control_variables.target_gpu(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 캠페인 control_variables.target_gpu(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 캠페인 control_variables.target_gpu(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 캠페인 control_variables.target_gpu(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(인증서 measured_node) · 인증서 gpu_model 일치 · target: 캠페인 control_variables.target_gpu(선언) · target_gpu 'NVIDIA GB10 x2 (TP=2)' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | exl3 | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json quantization_config quant_method=exl3 · vocab quant[exl3]←'exl3' |
| 축 `len` | 1048576 | 서빙 yaml max-model-len(output/multi/configs/i5-a-kern.yaml) |
| 축 `kv` | fp8 | 서빙 yaml kv-cache-dtype(output/multi/configs/i5-a-kern.yaml) · vocab kv[fp8]←'fp8' |
| 꼬리 `spec3` | DSpark speculative decoding k=3 — 같은 q·len·kv 로 n=0 · 5 · 10 을 쟀던 eager 셀들과 가른다 | `output/multi/configs/i5-a-kern.yaml` · 키 `speculative-config.num_speculative_tokens` = `3` |
| 꼬리 `densegemv` | 저자 dense GEMV 커널 레버 켜짐(PRE 92 · 전제 90 · 91) — dense MXFP8 가 cutlass · b12x 인 같은 q·len·kv 셀과 가른다 | `output/multi/envs/.env.i5-a-kern` · 키 `DSV41_DENSE_GEMV` = `1` |
| 꼬리 `moeprep` | MoE 입력 준비 1커널화 레버 켜짐(POST 74) — moe_prep_fused 없이 잰 같은 q·len·kv 셀(K5 · F2)과 가른다 | `output/multi/envs/.env.i5-a-kern` · 키 `DSV41_MOE_PREP_FUSED` = `1` |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 조건에서만 성립하는 것.**
- 통합메모리 · 노드당 한 풀(예산 선언 입력 `mem_total_mib=124608` — 선언값) → Engram 테이블(스모크 게이트 산정 193,365 MiB)과 가중치(랭크당 로드 74.64 GiB)를 같은 풀에 올릴 수 없어 디스크 기반 Engram(83 · 87 · 72)이 강제되고, KV 는 8 GiB 절대 클램프로 둔다 → GPU 메모리가 분리됐거나 노드 메모리가 더 큰 HW 에서는 Engram 상주 · offload · KV 크기의 판단이 전부 달라진다 — 83-engram-file-backend.sh §L9-11 · 02 §2.5.
- 노드당 GPU 1개 · TP=2 → 텐서 병렬 통신이 노드 간 RoCE 를 탄다. 측정 실행은 NCCL 이 외부 net 플러그인 SPCX(`NCCL_NET_PLUGIN=spcx` · 01 §1.4 env 관측)를 싣고 그 위에서 `Using network IB` · `GDR 0`(GPU Direct RDMA 꺼짐) · NCCL tail(BUFFSIZE 1M · LL128 끔 · NCHANNELS 8)로 돌았다 → SPCX 플러그인이 없는 이미지 · 다른 전송 · GDR 이 켜지는 조합에서는 랭크 간 AllReduce 대기(Q1 의 후보)가 달라진다 — 01 §1.5(5) · i5_master.log L159 · L490.
- sm_121a(GB10) 커널 경로 → dense MXFP8 은 SM120/121 에서 b12x small-M 타일 · 저자 dense GEMV(nvcc JIT `sm_121a`)로, MoE 는 EXL3 native p2b(5120×1152 · TP=2 형상)로 돈다. 84 shim(64-토큰 블록)은 SM12x 전용 분기다 → 다른 아키텍처에서는 커널 레버의 이득 · 필요성을 처음부터 다시 잰다(91 · 92 는 arch-locked) — testlog_26100417 §2 · 84-sm12x-block-shim.sh 헤더.
- 열 → 장문(1M) 시험은 열 하드다운 · 냉각 보강 뒤 한계 ≈125k 였고, 이 셀은 125k needle 도 열 가드 kill 로 미완이며 c=2 Full 은 두 번 끊겼다(W3 · W10 · testlog_26100510 §2 · §3) → 이 셀의 '1M' 은 기동 조건이지 1M 처리 검증이 아니다 — plan_26100319 §1 머리 · §7.

**(나) 이 형상이 버티는 기전 대 다른 형상.**
- eager(10-03 셀탐색) — graph 대신 eager · 커널 레버 0 — 기동은 버텼으나 lite 13.89~19.35 t/s 천장(스텝마다 커널 런치 노출) — 확정(측정) — plan_26100319 머리 '선행 증거'.
- graph · 89 없음 — KV null 블록 미정리 — 캡처 전 eager 더미 forward 가 block 0 에 남긴 쓰레기/NaN 을 sparse/paged 커널이 마스킹 슬롯에서 읽어 전 요청 NaN — 확정(census 실측 · zero 후 SMOKE PASS) — 89-kv-null-block-zero.sh §L8-11.
- graph · 88 prestage 만(89 없음) — Engram 조회만 캡처 밖으로 — NaN 이 풀리지 않음(원인 지목은 반증 · M1) — 확정 — 88-engram-prestage.sh §L47.
- Engram 상주(stock pinned UVA) — 83 · 87 · `hf-overrides` 없음 — 통합메모리 한 풀에 Engram 과 가중치를 같이 올려 OOM — 헤더의 설계 판단 · 이 계보의 기동 관측 없음(미결) — 83-engram-file-backend.sh §L9-11.
- graph + 89 + 이식 레버 일부(F1 18.49 → K1/K2 25.35 → K4 39.39 → K5 39.84 L.A.I.L) — 레버가 덜 쌓임 — 기전은 스텝 비용: dense MXFP8 cutlass 54 ms/step(61%) · MoE glue ~14 커널 · wo_a bf16 bmm — 확정(torch profiler 상대값) — testlog_26100417 §2 · devlog_26100610 §2.
- 이 형상 — 위를 모두 반대로 둔 것(graph + 89 + 88 + 디스크 Engram + POST 73 p2b 체인 + PRE 90 · 91 · 92 + POST 74 + spec k=3 + NCCL tail) — 측정 기동(시도 3)의 원장 사살 · 트립 행 0(세션 열 가드 kill 은 원장 밖 — 시도 3 도 lite 뒤 Full c=2 run3 에서 1회 kill · W10) · 양 랭크 레버 발화 서명 · lite pass — 확정 — 01 §1.4 · 01 §1.2 · testlog_26100510 §4.

형상을 연 변경은 둘로 갈린다: graph 를 연 것은 89 하나(+88 정합)이고, 처리량을 연 것은 레버 다섯(73 · 90 · 91 · 92 · 74)의 누적이다. 92 는 90 · 91 없이는 빌드가 die 한다.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | stock 0.30.0 이미지가 팩 A 의 exl3 양자화를 받을 경로를 갖고 있지 않았다(패키지 0개) | 빌드 슬롯 이식 — POST 70 exllamav3 · POST 73 vllm-exl3 플러그인 · PRE 80 지연 등록 훅 · PRE 81 MXFP8 lm_head 배선 · POST 79 자체검사(SM12X_PORT=1) | arch-invariant | i5_master.log L726 EXL3 MCG trellis engaged · L638 lm_head MXFP8 via ModelOptLinearMethod · smoke_i5.log L37 SMOKE PASS |
| 2 | W2 | Engram 테이블(원본 헤더 실측 189.1 GiB · 스모크 게이트 산정 193365 MiB)을 가중치와 함께 통합메모리에 올리면 노드 메모리를 넘는다(설계 판단) | PRE 83 file-backed Engram + POST 72 librow_store + PRE 87 embed 텐서 건너뛰기 + yaml hf-overrides engram_table_dir(노드 로컬 NVMe 스테이징) | arch-scaled | i5_master.log L708 · L716 file-backed Engram active · smoke_i5.log L19 engram=file-backed(−193365MiB) |
| 3 | W3 | 1M 장문 프롬프트 시험이 호스트 열 하드다운으로 끝났고 냉각 보강 뒤 열 한계는 약 125k 였다 | 세션 열 차단기(롤링 60s ≥92 °C 50s → 마스터 kill) 무장 · 장문 상한 ≤125k · 측정은 짧은 입력으로 | arch-locked | plan_26100319 §7 · 이 셀 기동 1 의 125k needle 열 abort(testlog_26100510 §2) · Full 측정의 차단기 발동(W10) |
| 4 | W4 | graph 모드(PIECEWISE · FULL_DECODE_ONLY · BREAKABLE=0) 전부 logits NaN · eager 만 정상이라 처리량이 eager 천장(lite 13.89~19.35 t/s)에 묶였다 | PRE 89 kv-null-block-zero(캡처 후 block 0 zero_) · PRE 88 Engram prestage 는 캡처 밖 조회의 정합 경로로 유지 | arch-invariant | i5_master.log L1006 kv null block zeroed after capture (tensors=10) · L1007 Graph capturing finished · smoke_i5.log L37 SMOKE PASS |
| 5 | W5 | 정식 재빌드에서 flashinfer-cubin 1.5 GB 휠 설치가 ≈3,600 s 지점에서 양 노드 4회 죽었다 | PRE 55 에 청크별 리다이렉트 재해석 + Range 이어받기 + sha256 검증 설치(fail-loud) | judgment | devlog_26100610 §2 F1 재빌드 성공 · 01 §1.1 55 applied(build-ledger) |
| 6 | W6 | graph NaN 을 푼 이식의 L.A.I.L 이 17.91 t/s 로 같은 HW 참조 런타임(R-1 44.50)의 절반 아래였다 | POST 73 저자 p2b 체인 8종(핀 3de91468 · sha256) · PRE 90 wo_a fp8 einsum + prepack · env DSV41_P2B_COOP=2 · DSV41_WOA_PREPACK=1 | arch-scaled | devlog_26100610 §2 K1/K2 L.A.I.L 25.35 · i5_master.log p2b coop dataflow kernel engaged · woa-requant fp8 einsum engaged |
| 7 | W7 | 커널 프로파일에서 우리 스텝 89 ms 중 dense MXFP8 GEMM(flashinfer cutlass)이 54 ms 를 차지했다 | PRE 91 저자 prefer_b12x_mxfp8 그대로 실행(backend auto) + 손잡이 EXL3PORT_MXFP8_BACKEND=auto | arch-locked | testlog_26100417 §3 dense 20.20 ms/step · L.A.I.L 39.39 · i5_master.log L717 exl3-port: mm_mxfp8 backend=auto |
| 8 | W8 | b12x 로 바꾼 뒤에도 dense 경로가 참조의 GEMV 커널보다 느렸다(K4 → K5 의 남은 몫) | PRE 92 저자 dense_gemv 그대로 실행 + 런타임 모듈 원문 설치(nvcc JIT sm_121a) + env DSV41_DENSE_GEMV=1 · 전제 PRE 90 · 91 | arch-locked | testlog_26100417 §5 lite 41.19 · L.A.I.L 39.84 · i5_master.log dense GEMV armed 9 모양 양 랭크 bitwise · graph replay ok |
| 9 | W9 | MoE 라우팅 glue 의 단일 블록 torch 커널 ~14개가 p2b 런치를 늦췄다(스텝 busy 51.2 ms) | POST 74 저자 moe_prep_fused 원문 설치 + exl3.py 무장 footer · env DSV41_MOE_PREP_FUSED=1 | arch-scaled | testlog_26100420 §2 스텝 busy 51.2 → 48.75 ms · i5_master.log L861 moe prep fused self-check bit-exact |
| 10 | W10 | 이 셀 Full(levels 1,2)이 기동 2 c=2 run2 · 기동 3 c=2 run3 에서 세션 열 가드 kill 로 끊겼다 | 두 스윕을 Full 근거에서 제외 · 기동 4 에서 c=2 를 빼고 c=1 만 반복 3(kill 없음 · bench_mode full) · 가드의 원장 배선은 후속 | arch-locked | testlog_26100510 §3 · §4 · §4.5(기동 4 롤링 최대 7 s · kill 없음) |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | CUDA graph 모드 전부 logits NaN · eager 정상 | 반증 | 88-engram-prestage.sh 헤더 GPU 실측 줄 · 89-kv-null-block-zero.sh 헤더 census 실측 · i5_master.log L1006 kv null block zeroed after capture |
| M2 | K1/K2 뒤에도 스텝이 참조보다 37 ms 길었다 | 반증 | devlog_26100610 §2 K3 · testlog_26100417 §2 커널 표 |
| M3 | 팩 A 의 o_proj wo_a 가 체크포인트에는 FP8 로 저장돼 있었다 | 반증 | 90-woa-fp8-einsum.sh 헤더 정정 줄 · i5_master.log L838 woa-requant fp8 einsum engaged(stage 1 은 이미 e4m3 면 no-op) |
| M4 | Full 리포트가 weak PASS 를 냈지만 측정 decode 39.68 t/s 가 루프라인 상한 R_fp 2.54 t/s 의 15배였다 | 반증 | testlog_26100417 §4 · testlog_26100420 §3 후속 항목 |
| M5 | 측정 리포트 스냅샷의 드라이버 행이 580.173.02 였다 | 반증 | 00 §0.4 드라이버 관측 ≠ 선언 행(attestation_i5-a-kern.json) |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.30.0 | docker history build-arg VLLM_REF |
| vLLM 소스 SHA(40자) | ced6857afa0ea7b2e3f0846a62e1394e90f15607 | output/multi/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 미관측 | 미관측 |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 미관측 | 미관측 — 측정 digest 미관측 · 관측 범위: output/multi/benchlog/lite_publish/i5-a-kern_20261005T072855Z/lite_engine_i5-a-kern.log = 보존 꼬리 창 tail-800(lite_bench.sh@dcb713a7436c L191) — 기동 배너 자리(로그 머리)는 창 밖이다 · 머리 캡처(lite_engine_head_*) 없는 측정이라 배너 부재를 관측하지 못했다 · 지위 `unobserved` |
| 인증서 `vllm_version`(강한 일치 키) | 0.30.0 | 생산자 `unverified` · 미검증 — sweep_bench.sh@dcb713a7436c 이미지 태그 규칙 결과 None ≠ 인증서 '0.30.0' · 배너 접두도 아니다 (더 높은 우선순위의 덮어쓰기(`--vllm-version` 인자 → VLLM_VER · EASY_VLLM_VERSION env · L582)는 측정 기록에 남지 않아 배제할 수 없다 — 덮어쓰기였을 수 있다) |
| wheel 메타(`vllm_build`) | 0.30.1.dev0+gced6857af.d20261004.cu133 | attestation nodes.<n>.vllm_dist_version(output/multi/benchlog/attestation_i5-a-kern.json · 노드별 관측) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측 |
| 체크포인트 revision(HF · git HEAD) | 미관측 | 미관측 |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.30.0-cu133-aarch64-source-sm12x | 셀 env(output/multi/envs/.env.i5-a-kern) IMAGE_TAG |
| 이미지 digest | sha256:e90626f88f339500fbeb94ad30862f77f695e58eca15b72f864736bdd0383016 | docker image inspect easy-vllm:0.30.0-cu133-aarch64-source-sm12x .Id · .Created 2026-10-04T12:52:29Z(현 로컬 이미지 관측 · 태그는 가변 포인터) · 묶인 attestation 노드별 image_id(output/multi/benchlog/attestation_i5-a-kern.json): main 같다 · sub 다르다(sha256:898be5ec7949…) — 노드별 로컬 빌드라 digest 가 노드마다 다른 것은 정상(요건은 동일 ABI 이지 동일 digest 가 아니다 · 이 행의 Id 는 메인 로컬 관측) · Created < 측정 2026-10-05T07:28:55Z → 측정 전 빌드 — 같은 빌드로 판정 가능(그 사이 재태깅은 관측 밖) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_i5-a-kern.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측 · ⚠ 선언 580.173.02 과 다르다(driver_conflict) |
| 드라이버(노드별 관측) | main=580.178.04 · sub=580.178.04 | attestation parity.driver(output/multi/benchlog/attestation_i5-a-kern.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 관측 580.178.04 ≠ 선언 580.173.02 | 관측: attestation parity.driver(output/multi/benchlog/attestation_i5-a-kern.json · 노드별 관측 · verdict=equal) — main=sub 일치 · 이 측정 실행과 같은 실행인지 미검증(같은 호스트의 관측) · 작성 2026-10-06T02:15:44Z > 측정 끝 2026-10-05T07:29:30Z — 측정 뒤 재기동의 관측 · 선언: output/multi/manifest.yaml(manifest_contract 로더) driver_version(스캔 시점 선언 — 측정 시각 값 미보장) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| quantization_config 선언 대상(양자화 층 · 제외 목록 밖) | `exl3 (weight_block_size=[32, 32])` | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json quantization_config quant_method |
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/quant_models/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz → <manifest.quant_model_path>/DeepSeek/DeepSeek-V4.1-Flash-EXL3-2.0bpw-sfxnz/config.json dtype |

**이 모델에 필요한 패치**(01 §1.1 관련성 · ① 패치 선언 대상 × 모델 config ② 엔진 로그 발화 — 둘 다 관측일 때만 확정): required 0개 · inactive-inferred 0개 · unknown 23개(unknown = 판정 신호 부족 — 필요 여부를 이 표로 단정하지 않는다).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

(1) **빌드 트랙 = source-build.** 실린 빌드 레시피 머리가 이유를 적는다: torch 2.11+ 구간에서 prebuilt vLLM wheel 의 `_C`(public torch 빌드)가 NGC alpha torch 와 C++ ABI 가 맞지 않아 import 단계에서 막히고, `_C` 를 NGC torch 에 맞춰 직접 컴파일하면 풀린다(`Dockerfile.source-build` §L28-31). 이 이미지의 torch 는 NGC alpha `2.13.0a0+9186a08`(위 표)라 그 구간이다. 이 셀에는 트랙 위에 하나가 더 얹혀 있다 — 이식 패치 전체가 빌드 슬롯(pre 14 · post 9)에 있고 build-arg `SM12X_PORT=1` 이 그 게이트다(01 §1.2). 다른 트랙으로 갈 조건도 같은 문장에서만 파생된다 — 대상 vLLM wheel 의 `_C` 가 베이스 torch 빌드와 ABI 가 맞을 때만 wheel 이 후보이고, 그 경우에도 이 셀의 pre 슬롯 소스 이식(80~92)은 wheel 트랙에 자리가 없다.

(2) **버전 문자열.** 빌드 입력 ref `v0.30.0`(docker history build-arg — 빌드한 쪽이 넣은 값) · 소스 SHA `ced6857afa0ea7b2e3f0846a62e1394e90f15607`(resolved.json 선언 · attestation 이 양 노드 `vllm_sha` 같음을 관측 — 지금의 attestation 파일은 측정 뒤 기동 4 의 관측이다) · 인증서 강한 키 `vllm_version` 0.30.0(lite 리포트 측정 환경 표가 `vllm_version_source = derived(envfile IMAGE_TAG)` 라고 적는다 — 측정 도구가 셀 env 의 이미지 태그에서 자른 값이지 엔진 자기보고가 아니다 · 위 표는 생산자를 `unverified` 로 남겼다) · wheel 메타 `0.30.1.dev0+gced6857af.d20261004.cu133`(attestation — `gced6857af` 가 같은 커밋 · `dev0` · `d20261004` 접미는 setuptools_scm 이 태그 위 수정된 작업트리(소스 패치)에 붙이는 꼴로 보인다 — 추론) · 엔진 기동 배너는 위 표가 미관측이지만 측정 기동의 엔진 로그 사본에 `Initializing a V1 LLM engine (v0.30.1.dev0+gced6857af.d20261004)` 가 있다(i5_master.log L87 — wheel 메타와 같은 문자열). **재현 좌표는 릴리스 태그 `v0.30.0` 또는 위 40자 SHA** 다.

(3) **arch-locked.** `TORCH_CUDA_ARCH=12.1a`(build-arg · GB10 sm_121a — 네 GPU 에서 `torch.cuda.get_device_capability()` 로 재도출) · exllamav3(70) · vllm-exl3(73)의 `built for 12.1a` 네이티브 · 92 의 런타임 nvcc JIT `compute_121a/sm_121a` · 84 의 SM12x 분기 · 91 의 SM120/121 b12x 선호 · CPU arch arm64(`uname -m`) · 드라이버 580 계열(관측 580.178.04 — `nvidia-smi` 로 보고 이미지 CUDA 13.3 을 지원하는지부터) · 공유 post 패치의 GB10 고정값(30 의 capability 상한 (13,0) · 40 의 NVML 폴백) · NCCL 자리(HCA · GID · iface)는 manifest `interconnect` 에서 다시 렌더한다 · 열 차단기 임계(롤링 60 s 중 ≥92 °C 50 s)는 이 유닛의 운영값이다.

(4) **선언 대 관측.** vLLM SHA 는 resolved.json(선언)과 attestation(관측)이 같다. 드라이버는 **다르다** — 측정 리포트 스냅샷 · manifest 의 580.173.02 는 선언이고 양 노드 attestation 관측은 580.178.04 다(위 표 `driver_conflict` · M5). 서빙 이미지의 실제는 관측 쪽이다. image digest 는 main 로컬 관측과 sub attestation 이 다르다 — 노드별 로컬 빌드라 정상이고 같아야 하는 ABI(build_ledger identity · torch · vllm_sha · driver)는 equal 이다(01 §1.5). NGC 베이스는 image Env 와 resolved.json 이 접두 일치한다. requirements.txt 는 빌드 원장 sha256 으로 쓰인 바이트임이 관측됐다(01 §1.1). attestation 은 범위 줄이 붙어 있다 — 지금 파일은 측정 **뒤** 재기동(기동 4 · 2026-10-06T02:15:44Z)의 관측이라 바인딩 측정 기동의 관측이 아니다. 기동 3 의 스모크도 4축 equal 이었다고 testlog_26100510 §4 가 적는다.

(5) **환경 좌표.** 호스트 층: 드라이버 관측 580.178.04(attestation · 양 노드) · 엔진 로그의 NCCL `cudaDriverVersion 13030`(드라이버가 보고한 CUDA API 버전 — i5_master.log L106) · 노드 메모리 총량 124,608 MiB(예산 선언 입력이고, 바인딩 lite raw 의 양 노드 `ram_total_kib` 127598768 관측과 일치) · 인터커넥트 RoCE 2장치(NCCL `Using network IB` · GDR 0 — 01 §1.5) · 호스트 OS 배포판 · 커널 버전은 이 계보에 **미기록**이다. 이미지 층(CUDA 13.3.1.008 · torch 2.13.0a0 · NGC 26.07 · NCCL `2.30.7+cuda13.3` — i5_master.log L107)과 섞지 않는다.

(6) **핵심 의존 핀.** 이 모델의 요구: exllamav3 1.4.5 @ e648f1a1(70 · 원장) · vllm-exl3 0.4.2 @ d3cfd394(73 · 원장) · sfxnz 저장소 @ 3de91468(73 p2b 체인 · 74 · 90 · 91 · 92 — PROVENANCE 트리 digest + 스크립트별 sha256) · Mia 커밋(83 · 72 · 70 패치 스크립트 — sha256) · flashinfer-python `0.6.18.post1`(requirements.txt — dense MXFP8 `mm_mxfp8` 경로를 91 · 92 가 편집하는 패키지 · 공유 이미지 핀이기도 하다). 공유 이미지의 기본값: Ray `2.48.0`(build-arg — 멀티 TP 가 쓰지만 단일/멀티 공유) · DeepGEMM nv_dev `a6b593d2826719dcf4892609af7b84ee23aaf32a`(10 — DeepseekV4 계열 arch-enablement · 90 의 `fp8_einsum` 이 `vllm.utils.deep_gemm` 을 거치므로 이 셀도 deep_gemm 을 쓰는 것으로 보이나 그 경로가 10 의 빌드를 쓰는지는 확인하지 않았다 — 추론) · humming-kernels `[cu12]==0.1.12` · nvidia-cutlass-dsl `4.7.1`(requirements.txt) · vendored triton_kernels(20).

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 20개 — `tuned` 2 · `inherited` 15 · `declared-requirement` 3.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | 캠페인 공통조건(D1 멀티노드 · 노드 2 × GPU 1)에서 파생 · 계보 전 셀이 같은 값 · 다른 값을 시도한 기록 없음 | V1 |
| `distributed-executor-backend` | ray | declared-requirement | 없으면 vLLM 이 multiprocessing 으로 가 World size 가 노드당 GPU(1)를 넘어 죽는다 — 러너 헤더의 실측 기록 · 러너가 이 줄이 없으면 fail-loud 로 멈춘다 | V2 |
| `quantization` | exl3 | declared-requirement | W1 — 팩 A 의 quant_method 가 exl3 이고 stock 0.30.0 은 그 경로가 없었다 · 80 의 지연 등록 훅은 이 값으로 get_quantization_config 가 불릴 때 플러그인을 등록한다(85 · 86 의 조건부 경로도 이 이름으로 켜진다) | V3 |
| `tokenizer-mode` | deepseek_v41 | inherited | 0.30.0 이 가진 V4.1 토크나이저 모드 · 저자 레시피 계열 승계 · 이 계보에서 바꿔 잰 기록 없음 | V4 |
| `hf-overrides` | {"engram_table_dir":"/app/ple_mmap/v41-engram-a"} | declared-requirement | W2 — 이 키가 83 디스패처 · 87 embed 건너뛰기를 켠다 · 없으면 stock Engram 이 테이블을 pinned UVA 로 올려 OOM 이라는 것이 83 헤더의 기록(이 계보가 빼고 기동한 관측은 없다 — 헤더 기록 근거) · 경로는 노드 로컬 스테이징 자리라 환경마다 다시 정한다 | V5 |
| `block-size` | 64 | inherited | 84 SM12x 64-토큰 블록 shim 과 짝인 저자/Mia 계열 값 · 근거 assert 는 문자열 실측이지만 이 계보에서 다른 값으로 기동한 관측 없음(84 헤더 UNTESTED) | V6 |
| `kernel-config` | {"sparse_indexer_topk_backend":"per_row"} | inherited | 84 가 버린 persistent_topk 스탠자의 0.30.0 대체 런타임 노브 · 승계 · 다른 값 시도 기록 없음 | V7 |
| `kv-cache-dtype` | fp8 | inherited | 계보 전 셀이 KV fp8 8 GiB(testlog 유효맥락) · plan 의 KV 축(fp8 ↔ 모델 기본)은 이 계보에서 시도 기록 없음 · 엔진은 fp8_ds_mla KV 형식을 썼다 | V8 |
| `kv-cache-memory-bytes` | 8589934592 | inherited | 손레버(lockset kv_source=hand) · 계보 내내 8 GiB 고정 · 필요량 산식은 계보에 없고 엔진 보고는 KV 2,571,042 토큰 = 1M 요청 기준 2.45배 | V9 |
| `gpu-memory-utilization` | 0.80 | inherited | 손레버(lockset gmu_source=hand) · 바꿔 잰 기록 없음 · KV 절대 클램프가 있어 엔진이 메모리 프로파일링을 건너뛰고 이 값을 존중하지 않는다(엔진 로그) | V10 |
| `max-model-len` | 1048576 | inherited | 캠페인 공통조건(D1 · 1M) · 셀 성립 기준이 '1M 기동' · 이 셀에서 조정 0 · 1M 프롬프트 처리는 별개(W3 · Q5) | V11 |
| `max-num-seqs` | 2 | inherited | 손레버(lockset batch_source=hand-lever) · plan 의 동시성 축 1~2 중 저자 값 2 · 스윕 기록 없음 | V12 |
| `max-num-batched-tokens` | 1536 | inherited | 승계 · 바꿔 잰 기록 없음 · 엔진이 spec 설정 기준으로 max_num_scheduled_tokens 를 1536 으로 둔다고 경고했다 | V13 |
| `enforce-eager` | false | tuned | 10-03 셀탐색은 eager(graph NaN · lite 13.89~19.35 t/s) → 89 로 NaN 을 푼 뒤 graph 로 바꿔 계보에서 두 값을 모두 잰 기록이 있다(W4) | V14 |
| `compilation-config` | {"cudagraph_mode":"FULL_AND_PIECEWISE","cudagraph_capture_sizes":[1,3,4,6,8],"custom_ops":["all"]} | inherited | 저자 그래프 설정(R-1 · yaml 머리 주석) 승계 · capture sizes 는 DSpark k=3 정합(I-1) · 다른 모드 시도(PIECEWISE · FULL_DECODE_ONLY)는 NaN 진단이었지 성능 비교가 아니다 | V15 |
| `no-enable-flashinfer-autotune` | true | inherited | 승계 · 이 계보에서 켜고 잰 기록 없음 | V16 |
| `reasoning-parser` | deepseek_v41 | inherited | 0.30.0 의 V4.1 파서 · 기능 노브(성능 무관) · 승계 | V17 |
| `tool-call-parser` | deepseek_v41 | inherited | 0.30.0 의 V4.1 파서 · 기능 노브 · 승계 · 도구 호출을 시험한 기록은 계보에 없다 | V18 |
| `enable-auto-tool-choice` | true | inherited | 기능 노브 승계 · 끄고 잰 기록 없음 | V19 |
| `speculative-config` | {"method":"dspark","num_speculative_tokens":3,"draft_sample_method":"greedy"} | tuned | DSpark n=0 · 3 · 5 · 10 을 lite 로 잰 기록(eager · 10-03)이 있고 k=3 은 저자 graph 기록과 capture sizes 정합으로 골랐다(R4) · graph 상태 k 재비교는 없다 | V20 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

**① 네 환경에서 반드시 다시 도출할 값.** `kv-cache-memory-bytes`(V9) — 예산 선언(바닥 = 총량 − 가중치 − KV − overhead)을 먼저 세우고 작은 클램프로 띄워 엔진 로그의 `GPU KV cache size … tokens` · `Maximum concurrency for <max-len> tokens per request` 배수로 목표 컨텍스트 대비를 본다(이 셀: 8 GiB → 2,571,042 토큰 · 1M 기준 2.45배). `max-num-seqs`(V12) — 그 배수와 열 여유 안에서. `max-model-len`(V11) — 기동 조건과 장문 처리(≤125k · 열)를 따로 검증한다. `tensor-parallel-size`(V1) — manifest 의 노드 × GPU 에서 파생한다. `hf-overrides` 의 Engram 경로(V5) — 네 노드의 로컬 스테이징 자리로. `gpu-memory-utilization`(V10) — 클램프가 있으면 KV 를 정하지 않는다(엔진 로그) · 바꿔 잰 적 없는 승계값이다.

**② 필요조건으로 확인된 값.** `distributed-executor-backend: ray`(V2 — 없으면 World size > GPU(1) · 러너가 fail-loud 로 막는다 · serve_runner.sh 헤더) · `quantization: exl3`(V3 — W1 · 이 이름으로 80 등록 훅과 85 · 86 의 조건부 경로가 켜진다) · `hf-overrides` engram_table_dir(V5 — W2 · 디스크 Engram 을 켜는 유일한 스위치 · 근거는 83 헤더의 OOM 기록이고 이 계보가 빼고 기동한 관측은 없다). yaml 밖이지만 빼면 깨지는 것: 셀 env `SM12X_PORT=1`(없으면 이식 패치 전부 skip — 01 §1.2) · `VLLM_RAY_EXTRA_ENV_VAR_PREFIXES_TO_COPY` 의 `DSV41_` · `EXL3PORT_`(없으면 서브 랭크 레버가 꺼진다). **명시하면 깨지는 노브**: `EXL3PORT_MXFP8_BACKEND=cutlass` 를 `DSV41_DENSE_GEMV=1` 과 함께 두면 dense GEMV 자가시험 기준이 cutlass 가 되어 전 shape 가 비트 불일치로 b12x 에 남는다(92 헤더 toggles 줄 — 헤더 기록 · 이 계보의 관측 아님) · `DSV41_DENSE_DG_SMALLM` 은 0/미설정 유지(같은 호출을 다투면 저자가 거부 — 92 헤더).

**③ 조정된 적 없는 승계값.** `kv-cache-dtype fp8`(V8) · `block-size 64`(V6 — 84 shim 짝) · `kernel-config` per_row(V7) · `max-num-batched-tokens 1536`(V13) · `compilation-config`(V15 — 저자 그래프 설정) · `no-enable-flashinfer-autotune`(V16) · 파서 · 도구 노브(V17~V19) · `max-num-seqs 2`(V12 · 손레버) · `kv-cache-memory-bytes`(V9 · 손레버) · `gpu-memory-utilization 0.80`(V10 · 손레버). 모두 이 계보에서 다른 값으로 잰 기록이 없다 — 최적이라는 뜻이 아니라 '이 값으로 섰다' 는 뜻이다. 조정된 것은 `enforce-eager false`(V14 — eager 대 graph)와 `speculative-config` k=3(V20 — eager 에서 n 을 바꿔 잰 기록)뿐이고, 커널 레버 env 는 켬/끔 두 값으로만 비교했다(02 §2.5).

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **버전 · 자체 패치 필요성** → 대상 vLLM 소스에서 `quantization` 레지스트리에 exl3 가 있는지 · `models/deepseek_v41/nvidia/engram.py` 서브클래스 구조 · `FlashInferCutlassMxfp8LinearKernel` 의 `mm_mxfp8` backend 인자 · `model_runner` 캡처 루틴(89 앵커)을 grep 으로 본다 → 그대로(앵커 일치 · 업스트림 미흡수) / 재도출(앵커 불일치 — 이식 패치는 이 경우 **빌드를 멈춘다**(FAIL exit 1) · skip 으로 조용히 빠지지 않는다 · 01 §1.2) / 이 지도 폐기(업스트림이 EXL3 · 디스크 Engram 을 정식 지원 — 그때는 stock 경로로 다시 세운다). 84 · 85 · 82 · 86 은 필요성이 미증명이라(Q6) 상위 버전에서는 빼고 스모크하는 것부터 시험할 후보다.
2. **HW 아키텍처** → `torch.cuda.get_device_capability()` · `uname -m` · `nvidia-smi` 드라이버 → sm_121a · arm64 · 580 계열이면 그대로 / 다르면 `TORCH_CUDA_ARCH` · 70 · 73 · 92 의 아키텍처 빌드 대상과 91 · 84 의 SM12x 분기를 재도출 / 통합메모리가 아니면 Engram · KV 판단(00 §0.2 (가))을 다시 세운다.
3. **노드 수 · 인터커넥트** → manifest 노드 × GPU 와 serve_runner 의 GPU 합류 리터럴(1.5 표 `runner_gpu_join_literal`) · 기동 로그의 `Using network IB` · `GDR` 값 → TP=2 · RoCE 면 그대로 / 노드 · GPU 수가 다르면 TP · 합류 리터럴 · KV 를 재도출 / `Using network Socket` 으로 내려가면 전송 설정부터 고친다(성능 비교 무효).
4. **호스트 메모리 안전 예산** → 기동 **전** 양 노드에 예산 선언(입력: 상주 가중치 = Engram 을 뺀 체크포인트 ÷ TP · KV 클램프 · overhead · 노드 총량) → 바닥이 워치독 가드 최소 위면 진행 / 아니면 KV · 동시 시퀀스를 줄여 다시 선언 / 상주 가중치만으로 바닥이 안 나오면 이 형상 폐기(R1 부류).
5. **기동 성공 판정** → health 200 + chat 추론 1회(`SMOKE PASS`) + attestation ABI equal + 패치 발화 서명(`kv null block zeroed after capture` · `engram prestage armed` · `file-backed Engram active` · `dense GEMV armed`(양 랭크) · `moe prep fused self-check bit-exact` · `mm_mxfp8 backend=auto` · `woa-requant fp8 einsum engaged` · `p2b coop dataflow kernel engaged`) → 다 보이면 그대로 / 서명 일부 부재면 그 레버가 꺼진 것(env 전달 · 게이트 확인) / 첫 응답이 NaN · 깨진 텍스트면 graph 를 의심하고 eager 로 대조(W4).
6. **성능** → 같은 도구(lite 512→128 · conc 1)로 재고 L.A.I.L 처럼 다른 지표와 섞지 않는다 → 이 지도의 수치(OBSERVATION-ONLY)와 같은 범위면 참고 / 크게 낮으면 레버 발화부터 확인 / 장문 · Full 은 열 차단기를 무장하고 레벨 사이 휴지를 둔다(W3 · W10).

실패 신호별 행선지: 빌드 · 버전 핀 · 앵커 불일치 · 의존 휠 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · 레버 env 문제 → `vllm-recipe-explorer` · 성능 기대 이하 · 판정 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 · 열 · 호스트 안전 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8-spec3-densegemv-moeprep` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26100320-v41flash-2node-broad · i5-a-kern · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26100320_v41flash_2node_broad__cluster__i5_a_kern |
| work-manifest | docs/_evidence/camp_26100320_v41flash_2node_broad__cluster__i5_a_kern.work-manifest.json |
| 증거 등급(task_class) | hint_map_only |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-05T00:10:41Z |
| 승인 발화(전사) | 운영자 발행 팝업 응답(2026-10-04): '승인 — i5-a-kern 발행' · 선행 채팅 '판정이 난 이후에는 hint 태그를 각 레시피별로 발행하면 될거같아' |
| 현행 full 정의 충족(측정 등급) | 아니오 · 이 측정의 반복 1 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실 검증 Agent(fact-checker)`(저작자 `hint 저작 Agent(publisher-author)` 아님) · 주장 198건 · 지적 17건 중 고침 17건 · 열림 0 · 2026-10-06T05:05:36Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `BENCH_MODE_LITE` | full bench 정의(lite ∪ GuideLLM × 반복 ≥3)를 충족하지 않았다고 **기재된** 측정이다 — 선언된 lite-only 셀이거나 반복 불성립(기계 이벤트)으로 강등된 셀이다. 수치는 lite 스냅샷(또는 강등 셀의 대표 run 1회)이라 산포 추정치·인증서가 없다. 어느 쪽인지·강등 사유는 측정 구성 표(bench_mode_kind · downgrade_reason)가 말한다. bench_mode 를 읽지 못한 측정(미확정·미기재)에는 붙이지 않는다 — 모름을 lite 로 접으면 합성이다(그 사실은 카탈로그 bench_mode 칸이 말한다). |
| `HINT_MISSING_CELL_CONFIG` | 셀 config 부재 — 선언 축(declared_axes·target_gpu)을 읽지 못한다. |
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

발행 경위(사실): 같은 레버 구성은 먼저 진단 셀(K6 · 진단 이미지 v11probe)로 쟀고, 정식 재빌드 셀(F2)은 K5 구성의 재현(POST 74 · `DSV41_MOE_PREP_FUSED` 없음)이었다. 캠페인 개정으로 더한 이 셀들은 배정(assignments)에 없어 hint 발행 도구가 셀 배정 부재로 막았다. 그래서 이름을 고쳐 붙이지 않고 최종 레시피를 계획 배정 셀(I-5 + I-6)로 **새로 측정**했다(testlog_26100420 §2.5). 그 셀은 네 번 기동됐다(testlog_26100510): 이 hint 의 측정 바인딩은 기동 3 의 lite(declared-lite · 2026-10-05T07:28:55Z)이고, 기동 2 · 3 의 Full 은 열 가드 kill 로 근거에서 빠졌으며, full 등급 측정은 측정 뒤 기동 4 의 Full c=1(34.41 cold · 40.83 · 39.80 t/s · weak PASS · 인증서 없음)뿐이다 — 계보 문서로만 실렸다. 의견: 기동 4 의 c=1 warm 수치(약 40 t/s)가 이 레시피의 더 안정된 대표값에 가깝지만, 다른 기동 · 다른 도구 · 다른 부하이고 그 판정은 루프라인 결함(M4)으로 정보가 없다(03 §3.5). AGPL 코드(vllm-exl3 · Mia 계열)는 zip 에 동봉하지 않고 핀 커밋에서 받아 sha256 검증하는 스크립트만 실었다(plan_26100208 §8 D6).

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.30.0/deepseek-v4.1-flash-exl3-2.0bpw-sfxnz/gb10-1g2n-cluster-native/qexl3-len1048576-kvfp8-spec3-densegemv-moeprep` — 꼬리 3토큰.

| 토큰 | 뜻 | 근거(서빙 설정) |
|---|---|---|
| `spec3` | DSpark speculative decoding k=3 — 같은 q·len·kv 로 n=0 · 5 · 10 을 쟀던 eager 셀들과 가른다 | `output/multi/configs/i5-a-kern.yaml` · 키 `speculative-config.num_speculative_tokens` = `3` |
| `densegemv` | 저자 dense GEMV 커널 레버 켜짐(PRE 92 · 전제 90 · 91) — dense MXFP8 가 cutlass · b12x 인 같은 q·len·kv 셀과 가른다 | `output/multi/envs/.env.i5-a-kern` · 키 `DSV41_DENSE_GEMV` = `1` |
| `moeprep` | MoE 입력 준비 1커널화 레버 켜짐(POST 74) — moe_prep_fused 없이 잰 같은 q·len·kv 셀(K5 · F2)과 가른다 | `output/multi/envs/.env.i5-a-kern` · 키 `DSV41_MOE_PREP_FUSED` = `1` |
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

꼬리는 `spec3` · `densegemv` · `moeprep` 셋이다. `spec3`(yaml `speculative-config.num_speculative_tokens` = 3)는 같은 q·len·kv 로 n=0 · 5 · 10 을 쟀던 10-03 eager 셀들과 가르고, `densegemv`(셀 env `DSV41_DENSE_GEMV=1`)와 `moeprep`(셀 env `DSV41_MOE_PREP_FUSED=1`)는 커널 레버가 덜 쌓인 같은 q·len·kv 셀들(K4 · K5 · F2 — dense GEMV 또는 moe_prep_fused 없음)과 가른다. 후보 `plenone` 은 이 모델에 PLE 가 없어 모든 자매 셀이 같은 값이라 가르지 않고, `graph` 는 eager 셀과의 차이를 `densegemv` 가 이미 함의해(트랙 K 레버는 모두 graph 위에서만 잼) 빼고 셋을 v6 순서로 쓰지 않았다. testlog_26100420 §2.6 이 이 셀과 같은 env 라고 적은 K6 셀은 진단 이미지(태그 v11probe)에서 쟀고 발행되지 않았다(00 §0.8 · 소프트 지문 전체 대조는 03 §3.5).
