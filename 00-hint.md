# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native-bare/qnvfp4-len262144-kvauto-plemmap-spec3-eager` |
| 형식 | `hint-payload/v6` · 이름 문법 `v6` |
| 생성(UTC · 주입) | 2026-09-23T08:16:21Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26092301-hint-v6-e2e · nv4-bf-262k-mmp-native · cluster · campaign |
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
| 자격 근거 | `output/multi/benchlog/serve_proof_nv4-bf-262k-mmp-native.json` · `output/multi/benchlog/serve_proof_nv4-bf-262k-mmp-native.json#cleanup_attestation_path → docs/simlog/26092315_native_N1/cleanup_attestation.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
<!-- /FACT:grade -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

`nvidia/Qwen3.8-Flash-Next-NVFP4` 를 GB10 1GPU × 2노드(Ray TP=2)에서 **Docker 없이** 호스트 venv 로 서빙했다 — 각 노드가 자기 로컬 이미지(vLLM `v0.29.0rc6` 소스빌드)에서 재컴파일 없이 재포장한 오프라인 wheelhouse 로 설치 · 노브는 Docker 셀과 동일(NVFP4 · 262144 · KV auto · PLE mmap · MTP k=3 · eager) · 판정점 동시성 1 decode 22.27 t/s(MTP on · GuideLLM full 반복 3).
가장 비싼 벽은 native 평면 자체였다 — 이미지 재포장 게이트의 RECORD 불일치(W8 → 사람 승인 + 이미지 동등성 게이트)와 정문 · 삭제기 · 측정 클라이언트의 라이브 결함들(W9~W13)이며, 모델 · 버전 벽(W1 · W6)의 해소는 이미지 바이트째 따라왔다.
성능은 탐색 합격선 대비 REFUTE(인증서 없음 · Docker 셀과 같은 대역)이고 합격선은 run 마다 측정 수용길이로 움직인다 — 이 수치를 기준선으로 쓰지 말 것(Q1 · Q2) · KV 20GiB 클램프(V6)는 필요량을 크게 넘는 승계 손레버다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | qwen3.8-flash-next-nvfp4 | sweep_index.meta(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native) |
| HF repo | nvidia/Qwen3.8-Flash-Next-NVFP4 | 체크포인트 .git/config remote url(huggingface.co · native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| HF revision(체크포인트 git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-23T08:10:39Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| base_model | Qwen/Qwen3.8-Flash-Next | 체크포인트 README base_model(native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) |
| base 슬러그 | qwen3.8-flash-next | model(qwen3.8-flash-next-nvfp4) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | nvfp4 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native)) · q 축 = native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native) |
| 토폴로지 · TP | multi · TP=2 | sweep_index.meta(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native) |
| 실행 평면 | native | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.29.0 | sweep_index.meta(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native) |

**명명 축** — 태그 이름은 도구가 이 축들에서 전량 파생했다(발행자 입력 ✗).

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.29.0rc6 | track=선택자 Dockerfile.source-build · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/multi/resolved.json upstream_delta.to_sha · VLLM_REF=v0.29.0rc6(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | qwen3.8-flash-next-nvfp4 | 서빙 yaml model(output/multi/configs/nv4-bf-262k-mmp-native.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · hf_repo(nvidia/Qwen3.8-Flash-Next-NVFP4) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g2n-cluster-native-bare | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qnvfp4-len262144-kvauto-plemmap-spec3-eager | derived(q·len·kv·ple·spec·graph) |
| 축 `hw` | gb10 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 2 | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | cluster | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/multi/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `q` | nvfp4 | native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4/config.json quantization_config(modelopt MIXED_PRECISION) quantized_layers 우세 NVFP4 48/50 · vocab quant[nvfp4]←'modelopt-dominant:NVFP4' |
| 축 `len` | 262144 | 서빙 yaml max-model-len(output/multi/configs/nv4-bf-262k-mmp-native.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype(output/multi/configs/nv4-bf-262k-mmp-native.yaml) · vocab kv[auto]←'auto' |
| 축 `ple` | mmap | 셀 env VLLM_PLE_MMAP=1(output/multi/envs/.env.nv4-bf-262k-mmp-native) · vocab ple[mmap]←'mmap' |
| 축 `spec` | 3 | 서빙 yaml speculative-config.num_speculative_tokens(output/multi/configs/nv4-bf-262k-mmp-native.yaml · method=mtp) |
| 축 `graph` | eager | 서빙 yaml enforce-eager: true(output/multi/configs/nv4-bf-262k-mmp-native.yaml) · vocab graph[eager]←'eager' |
| 축 `plane` | native | artifacts.plane_of → evidence.plane(declared) · env: IMAGE_TAG/BUILD_DOCKERFILE 없음(output/multi/envs/.env.nv4-bf-262k-mmp-native) · serve_proof output/multi/benchlog/serve_proof_nv4-bf-262k-mmp-native.json#plane(observed native) · vocab plane[native]→'bare' |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

**(가) 이 결과를 이 조건에서만 성립하게 만드는 전제**
1. **설치 입력 = 같은 태그의 로컬 Docker 이미지** → native 설치본은 이미지 설치 트리를 RECORD 대로 재포장한 것이라 이미지와 바이트가 같다(승인한 5 파일 포함) → 이미지가 없거나 다른 이미지(다른 NGC 베이스 · 다른 빌드 패치)면 재포장 게이트가 다른 불일치를 내고 승인 선언은 digest 가 달라 도구가 거부한다 — plan_26092311 §3 N-D1 · §N1 라이브 게이트 뒤 결정.
2. **CUDA 사용자 라이브러리 = 이미지 번들, 호스트는 드라이버만** → verify 가 산출한 `LD_LIBRARY_PATH`("compat-first" — 이미지 `cuda/compat` 우선)로 `ldd` 미해소 0 · import/CUDA 를 통과했다 → 호스트 드라이버 계열이 이미지의 compat 라이브러리와 맞지 않는 HW 에서는 이 순서가 깨질 수 있다(이 계보에서 잰 적은 없다 · 추론) — plan_26092311 N-D2.
3. **통합메모리(노드 MemTotal 약 124,610 MiB 한 풀) · 노드당 GPU 1개 × 2노드** → 예산 산식 · PLE mmap · 노드 간 NCCL 이 Docker 셀과 같은 조건에서 돌았다(UVA offload 는 구조 추론으로 배제 · R2) → 이산 GPU · 다른 노드 수에서는 PLE 방식과 TP 를 다시 세운다 — plan_26090918 §2.3 · §5.

**(나) 이 형상이 버티는 기전 대 다른 형상 · 평면**(같은 모델 · 같은 NVFP4 · 262k — 상세는 02 §2.5)
- D1(Docker · 같은 이미지 · 같은 노브) — 평면만 다르다(컨테이너 대 호스트 프로세스) — 둘 다 서빙 성립(관측 · 확정) · 판정점이 같은 대역(20.98 대 22.27)이지만 이 측정의 반복 밴드가 17.14% 라 평면이 decode 에 주는 영향은 미결 — testlog_26092317 §3.
- 같은 셀 run 1(native · 같은 설치) — 측정은 성립했지만 정리 증명(attestation)이 실패해 발행하지 않았다 — 수용길이 1.837 대 2.08 로 합격선이 달랐다(관측 · 확정 · 원인 미결) — testlog_26092317 §2.
- Docker 계보의 PLE resident 자매 형상과 KV · gmu 설정 비교는 D1 페이로드 02 §2.5 에 있고 이 셀이 새로 잰 것은 없다 — mmap 페이지를 회수 가능한 page cache 로 설명하는 것은 가설이다 — plan_26091216 §2.3.

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | NVFP4 mixed 체크포인트의 PLE(FP8)·MTP(FP8_PB_WO) 가중치가 stock 경로에서 로드되지 않는다 | 빌드 패치 60-qwen4exp-nvfp4-mixed.sh(이미지에 구워짐 → native 는 재포장으로 승계) | arch-invariant | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 2 | W2 | 스모크 1차가 로드 전에 게이트에서 멈췄다 | 양 노드 예산 선언 + kv-cache-memory-bytes 고정(native 정문도 로드 전 선언) | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 3 | W3 | 스모크 2차에서 엔진이 기동 중 fail-loud 로 죽었다 | moe-backend 를 명시하지 않는다(auto) | arch-locked | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 4 | W4 | 스모크 3차가 moe-backend cutlass 명시로 실패했다 | moe-backend 를 명시하지 않는다(auto) | arch-invariant | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차 PASS |
| 5 | W5 | 스모크 4차 그래프 캡처 구간에서 호스트 워치독이 docker kill | enforce-eager: true(캡처 비활성) | arch-scaled | testlog_26090921_p0-1_nvfp4_mixed_패치_스모크_판정 §판정 스모크 5차(eager) PASS |
| 6 | W6 | PLE 47.7GiB 가 상주 가중치에 들어가 노드당 KV · overhead 몫을 잠식한다 — stock 에 거둘 경로가 없다 | 빌드 패치 62-qwen4exp-ple-mmap.sh(이미지에 구워짐 → native 재포장으로 승계) + VLLM_PLE_MMAP=1 + 노드 로컬 NVMe 스테이징 | arch-scaled | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-2 PASS |
| 7 | W7 | 62 패치 적용 이미지의 첫 스모크가 AttributeError 로 실패 | rng[0]/rng[1] 로 교정(62 패치 안) | arch-invariant | testlog_26091001_p0-2_ple_mmap_p0-3_qsa_fp8kv_스모크_판정 §판정 P0-2 PASS |
| 8 | W8 | 메인 wheelhouse 재포장이 RECORD 불일치 5 파일과 이미지 고유 의존성 충돌로 FAIL 정지 | 사람 결정(O-N2) — 5 파일 사유별 승인 + 이미지 동등성 게이트(설치 집합 == 이미지 핀 ∧ pip check ⊆ 선언된 이미지 고유 충돌) | arch-locked | testlog_26092317_native_N1_분산서빙_판정 §2. wheelhouse 게이트 |
| 9 | W9 | 시도 1 이 install 단계에서 ssh 호출 실패로 끝났다(rc=3) | 요청을 stdin 으로 넘긴다(05f8bde) | arch-invariant | testlog_26092317_native_N1_분산서빙_판정 §2. 시도 2 부터 install 단계 통과 |
| 10 | W10 | 시도 2 에서 서브 wheelhouse 재포장이 승인 선언 불일치로 거부됐다 | 승인 선언 images[] 에 노드별 digest 등재 · 목록 밖 불일치는 계속 실패(d031c35) | arch-locked | testlog_26092317_native_N1_분산서빙_판정 §2. run 1 · run 2 up PASS |
| 11 | W11 | 시도 2 의 자동 down attestation 이 FAIL_CLOSED(run root 잔존 · 프로세스 · GPU 0) | 트리 안 링크는 링크만 unlink(d031c35) · 고친 down 으로 잔재 정리 | arch-invariant | testlog_26092317_native_N1_분산서빙_판정 §2. 시도 2 잔재 PASS 정리 |
| 12 | W12 | run 1 첫 스윕의 lite 레그가 빈 JSON 을 남기고 spec 축이 부재로 강등됐다 | up 이 서버와 같은 env -i 환경의 래퍼 bin/vllm-client 를 만들어 --client-vllm 으로 넘긴다(c3408cb) | arch-invariant | testlog_26092317_native_N1_분산서빙_판정 §2. run 1 재스윕 · run 2 full 5레벨×3 완주 |
| 13 | W13 | run 1 의 down attestation 이 FAIL_CLOSED 로 끝나 잔재 0 을 도구가 증명하지 못했다 | 소켓 · FIFO unlink · 마커는 마지막에 삭제(a21e66e) · run 1 잔재는 정확한 run-id 경로만 수동 복구 · 사용자 결정으로 전체 재실행 | arch-invariant | testlog_26092317_native_N1_분산서빙_판정 §2. run 2 자동 down attestation PASS |
| 14 | W14 | native 스윕 색인의 강한 일치 키 vllm_version 이 NA 로 조립됐다 | serve proof 의 wheelhouse 원천 이미지 태그를 이미지 라인으로 쓴다(a21e66e) | arch-invariant | bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0 §측정 환경 스냅샷 vllm_version 0.29.0 |
| 15 | W15 | 발행기가 native 셀의 이미지 digest 를 관측하지 않고 넘어갔다(평면 판정도 docker 로 오독) | 결측 표지는 부재로 · 원천 이미지는 build_identity 의 native_wheelhouse_source 표지로 평면 신호에서 뺀다(200541c) | arch-invariant | 이 페이로드 00 §0.2 실행 평면 native · 명명 축 plane native |
| 16 | W16 | native 셀의 full 리포트에 호스트 운영자 경로가 실려 발행 PII 게이트가 막았다 | 리포트 표시만 manifest 필드 표지로 바꾼다(81d4a55) — 서명은 수정 뒤 표시다 | arch-invariant | bench_report_26092317_qwen3.8-flash-next-nvfp4_GB10_0.29.0 §측정 환경 스냅샷 |
| 17 | W17 | 실린 pip freeze 가 발행 PII 게이트(private-ipv4)에 걸렸다 | == 바로 뒤의 3옥텟 버전 모양만 제외(39c4826) — 4옥텟 주소 · host= 뒤 주소 · 대역 표기는 그대로 잡는다 | arch-invariant | 이 페이로드에 pip-freeze-main.txt · pip-freeze-sub.txt 가 실렸다 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | native 분산 정문이 커밋돼 있어 N1 을 바로 돌릴 수 있다고 보였다 | 반증 | testlog_26092317_native_N1_분산서빙_판정 §3. 판정 — run 2 가 도구 스스로 up → full×3 → down PASS |
| M2 | 스윕 로그가 lite 레그를 성공(✓)으로 표시했다 | 반증 | devlog_26092317_native_N1_구현_라이브_서사 §6. 다음 |
| M3 | 같은 셀 형상의 판정점이 이전 인증 측정(38.81)보다 크게 낮은 채 Docker 평면에서 관측됐다 | 미결 | testlog_26092317_native_N1_분산서빙_판정 §3. 판정 |
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
| wheel 메타(`vllm_build`) | 미관측 | 미관측 — 측정 경로에 wheel 배포 메타 관측자가 없다(sweep_index.meta.vllm_build='NA' 는 sweep_bench 가 엔진 로그에서 `v<x.y.z…>` 정규식으로 잡는 값이다 → engine_self_report 의 원천) |
| 체크포인트 revision(HF · git HEAD) | fc694b54fb0174e0913e6adf86691ef85a4ead47 | 체크포인트 .git HEAD → refs/heads/main → loose ref(native model: → <manifest.quant_model_path>/Qwen/Qwen3.8-Flash-Next-NVFP4) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-09-23T08:10:39Z 이전 마지막 이동 2026-09-08T10:37:13Z) |
| 빌드 트랙 | native | evidence.plane(declared) |
| Dockerfile(선택자) | 미관측 | 미관측 |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.29.0rc6-cu133-aarch64-source | sweep_index.meta.image_tag(measured(native serve proof · wheelhouse 원천 이미지)) |
| 이미지 digest | 미관측 | 미관측 |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.173.02 | sweep_index.meta.driver_version(output/multi/benchlog/sweep_nv4-bf-262k-mmp-native · 측정 시 조립) |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

**(1) 빌드 트랙.** 사실 블록의 트랙은 `native` 이다 — 그러나 native 는 빌드하지 않는다. 설치 원천 이미지는 `source-build` 트랙이며(해소 기록의 판정 문장 "torch 2.13 >= 2.11 → 하드 ABI 벽(_C 를 NGC torch 에 링크) → source-build" · `output/multi/resolved.json` `build_track.rationale`), native 셀은 그 이미지의 설치본을 재컴파일 없이 재포장한다(plan_26092311 N-D1 · R4). 그러니 이 셀을 재현하려면 먼저 이미지를 source-build 로 지어야 한다(Docker 셀 D1 의 빌드 절차). 다른 트랙(공식 wheel 설치)으로 갈 수 있는 조건은 원천 이미지의 트랙 판정에서 나온다 — 대상 vLLM 의 torch 가 2.11 미만이어서 그 ABI 벽이 없고 그 버전의 wheel 이 발행돼 있을 때다(해소 기록은 `0.29.0rc6` wheel 이 404 라고 적었다).

**(2) vLLM 버전 문자열과 생산자.** 빌드 입력 ref = `v0.29.0rc6` · 소스 SHA = `74c96922ecb9017f413318c76d1af83aa2ab45a5`(위 표 — 원천 이미지 기준). 0.2 표의 `vllm_version` `0.29.0` 은 측정 도구가 serve proof 의 wheelhouse 원천 이미지 태그에서 잘라 만든 값이다(스윕 meta `image_tag_source: measured(native serve proof · wheelhouse 원천 이미지)` · W14). 설치된 배포 메타는 pip freeze 의 `vllm==0.29.0rc7.dev0+g74c96922e.d20260922.cu133` 이다(1.2 — 소스 빌드의 dev 문자열 · `+g` 뒤는 SHA 앞 9자). 인증서는 없다. 엔진 기동 배너는 위 사실 블록에 실리지 않았지만(도구가 native 스윕의 빈 엔진 로그 자리만 본다) 정문이 보존한 엔진 로그 `docs/simlog/26092315_native_N1/logs/main-vllm-serve.log`(계보 목록 밖) L5 · L52 가 `version 0.29.0rc7.dev0+g74c96922e.d20260922` 를 찍었다 — pip freeze 와 같은 dev 문자열이다. **재현 좌표는 `v0.29.0rc6` 또는 위 40자 SHA** 다 — `0.29.0` 이나 `0.29.0rc7.dev0…` 로 wheel · 태그를 찾지 마라.

**(3) arch-locked 값과 재도출.**
- 이미지 층(`TORCH_CUDA_ARCH=12.1a` · CPU `arm64` · NGC `26.07-py3` · torch `2.13.0a0+9186a08` · CUDA `13.3.1.008`)은 원천 이미지에서 온다 — 네 GPU compute capability · 플랫폼으로 이미지를 다시 짓고 NGC 태그는 결정론 해소 스크립트(upstream-version-watch `resolve_ngc_tag.py`)로 다시 푼다.
- 호스트 층: Python 3.12(venv) · NVIDIA 드라이버(네 `nvidia-smi` 값) · 이미지 digest(노드마다 다르다 — 승인 선언 `images[]` 에 네 digest 를 새로 등재해야 도구가 받는다).

**(4) 선언 대 관측.** 드라이버 — 표의 `580.173.02` 는 스윕 meta 가 manifest 의 `driver_version` 을 옮긴 **선언**이다(`sweep_bench.sh` 의 manifest 읽기 · D1 페이로드 00 §0.4). 이 셀의 계보 문서는 양 노드 580.178.04 를 적었다(testlog_26092317 · plan_26092311 유효맥락) — 서빙 호스트의 실제는 그쪽이다. 이미지 digest 는 이 셀의 스윕 meta 에서 `NA`(관측 불가)이고, 설치 원천의 노드별 digest 는 승인 선언이 적었다(메인 `a2c4ca49…` · 서브 `87b6a67d…`). torch 는 해소 핀 `2.13.0`(선언)과 설치본 `2.13.0a0+9186a08b2c.nv26.7.59513937`(pip freeze 관측)이 접두 일치한다.

**(5) 환경 좌표.** 드라이버는 (4). 노드 메모리 총량 MemTotal 124,610 MiB(testlog_26091109 유효맥락 · 이 셀의 예산 선언은 124,608). 호스트 CUDA 13.0/13.2 · Python 3.12.3(plan_26092311 유효맥락 — 호스트 CUDA 툴킷은 쓰지 않는다). 인터커넥트는 plan 이 RoCE 196.88Gbps 로 적었고, 스윕 산출물에는 측정 env echo 가 없지만 정문이 보존한 엔진 로그(계보 목록 밖)는 양 노드의 NCCL echo(`NCCL_IB_DISABLE=1` · `NCCL_DMABUF_ENABLE=0` · `NCCL_CROSS_NIC=1` · 네트워크 `Socket` · NCCL `2.30.7+cuda13.3`)를 기록했다(01 §1.5(5)). OS 배포판 · 커널은 계보 문서에 미기록이다.

**(6) 핵심 의존 핀**(pip freeze · 양 노드 동일): `ray==2.48.0` · `flashinfer-python==0.6.18` · `triton==3.7.1+gitf797708c.nv26.7`(NGC 번들) · `deep_gemm==2.5.0+a6b593d` · `humming-kernels==0.1.12` · `protobuf==6.33.6` 와 `grpcio-tools==1.83.0`(선언 충돌 — 이미지 고유로 승인). 이 모델 고유의 요구는 핀이 아니라 이미지에 구워진 빌드 패치 60 · 62 다(W1 · W6) · 나머지는 공유 이미지의 기본값이다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 9개 — `inherited` 8 · `engine-default` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `tensor-parallel-size` | 2 | inherited | manifest 토폴로지(노드 2 × 노드당 GPU 1)에서 정해진 값 — 계보에서 바꿔 잰 기록 0 | V1 |
| `distributed-executor-backend` | ray | inherited | 멀티노드 TP 의 Ray 경로 — Docker 원본에서 렌더로 승계 · native 정문도 Ray head/worker 를 띄운다 · 빼고 잰 기록은 이 셀에 없다 | V2 |
| `gpu-memory-utilization` | 0.85 | inherited | P0-1 스모크 최종 설정에서 승계 · 이 셀에서 바꿔 잰 기록 0 · lockset gmu_source=target_gmu 는 선언이지 관측된 실패가 아니다 | V3 |
| `max-model-len` | 262144 | inherited | 모델 네이티브 컨텍스트(YaRN 없이) · 캠페인 셀 축 통제변인 — 조정 0 | V4 |
| `kv-cache-dtype` | auto | engine-default | auto = 엔진이 모델 dtype(bf16)을 쓴다 | V5 |
| `kv-cache-memory-bytes` | 21474836480 | inherited | Docker 원본 트리플렛에서 조정 없이 승계한 손레버(lockset kv_source=hand) · 262k bf16 필요량 3,072MiB/노드를 크게 넘는다 · 음성대조 의도는 원본을 쓴 res-min 캠페인의 것 · 스윕 0 | V6 |
| `enforce-eager` | true | inherited | W5(P0-1 · PLE resident 구성의 그래프 캡처 트립) 이후 승계 통제변인 — 이 셀에서 eager 를 끄고 잰 기록 0 | V7 |
| `async-scheduling` | false | inherited | 외부 선례의 MTP+async 금지 조합 회피용 명시(R4 · Docker 계보) — 우리 계보에서 켜서 실패한 관측은 없다 | V8 |
| `speculative-config` | {"method":"mtp","num_speculative_tokens":3} | inherited | 사람 결정(MTP 켠 채 · 2026-09-12)으로 고정된 통제변인 · plan 이 레버로 적은 MTP k 스윕은 시도 0 | V9 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

- **① 네 환경에서 반드시 다시 도출할 값(arch-scaled)**: `kv-cache-memory-bytes`(V6) — 목표 컨텍스트 × 토큰당 KV 바이트로 필요량을 먼저 구하고(출처 산식: 262k bf16 에 3,072MiB/노드 — 02 §2.5) 호스트 예산 바닥이 절대밴드를 넘는지로 상한을 정한다. `gpu-memory-utilization`(V3)은 KV 를 절대값으로 고정한 이 구성에서 역할이 갈리지 않았으니 로드 피크 최저 MemAvail 을 보며 다시 잰다. `max-model-len`(V4) · `tensor-parallel-size`(V1)는 네 목표 · manifest 로 정해진다.
- **② 필요조건(declared-requirement)**: 이 셀에는 없다 — 빼서 실패를 관측한 노브가 이 셀 계보에 없다. **명시하면 오히려 깨지는 노브**: `moe-backend` — triton 명시는 NVFP4 MoE 미지원, cutlass 명시는 60 패치 가드와 충돌(W3 · W4 · Docker 계보). native 에서 노브보다 먼저 필요조건인 것은 설치 · 런타임이다 — 이미지 동등 설치(W8) · 서버와 같은 런타임 env 의 측정 클라이언트(W12).
- **③ 조정된 적 없는 값(inherited · engine-default)**: 노브 9개 전부 Docker 원본에서 렌더로 승계됐다. `kv-cache-memory-bytes` 20GiB(V6)는 필요량을 크게 넘는 손레버 · `enforce-eager`(V7)는 다른 구성의 캡처 트립 이후 승계 · `async-scheduling: false`(V8)는 외부 선례 회피용 명시 · MTP k=3(V9)은 사람 결정 · `kv-cache-dtype: auto`(V5)는 엔진 기본 dtype 이다. 어느 것도 이 셀에서 대안과 비교돼 고른 값이 아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. **원천 이미지** → 각 노드에 같은 태그의 로컬 이미지가 있는가(`docker image inspect`)와 그 이미지가 어떤 빌드 패치로 지어졌는가(Docker 셀 D1 페이로드의 빌드 원장) → 있으면 그대로 · 없으면 D1 절차로 source-build 부터 · 목표 vLLM 이 바뀌면 이 지도 폐기(Docker 셀 지도부터 다시).
2. **재포장 게이트 #1** → `native_wheelhouse.py build` 를 네 이미지에 돌려 RECORD 불일치 목록을 본다 → 이 페이로드의 5 파일(humming 1 · NGC 후처리 4)과 같으면 사유를 확인해 승인 선언에 네 digest 를 등재 · 다르면 사람이 사유별로 다시 승인하거나 native 방식 자체를 다시 plan(plan_26092311 철회 조건) · 다운로드 · 재컴파일로 우회하지 않는다.
3. **설치 게이트** → `native_wheelhouse.py verify` 의 게이트 전부(closure · 설치 집합 == 핀 · pip check ⊆ 선언 · ldd 미해소 0 · import vllm,torch + CUDA) → 하나라도 FAIL 이면 서빙으로 가지 않는다 · 호스트 드라이버와 이미지 compat 라이브러리의 짝부터 본다.
4. **호스트 메모리 안전 예산** → `floor = MemTotal − weights − kv − overhead` 에 네 값을 넣는다(weights = `(ckpt − PLE) ÷ tp` · overhead 는 네 HW 에서 다시 잰다) → 정문 up 이 RAM 게이트와 선언을 로드 전에 밟는다 · 정문 밖에서 손으로 `vllm serve` 를 띄우지 않는다(pgid 워치독이 붙지 않는다).
5. **기동 · 정리 성공 판정** → serve proof(health 200 + 추론 1회) + 보존 엔진 로그의 `PLE mmap: layer 1, 128 shards … tp_world 2`(양 rank) + 측정 뒤 정문 down 의 cleanup attestation PASS(양 노드 run root 부재 · owned 프로세스 0 · GPU 0 · `unknown: []`) → attestation 이 FAIL 이면 그 run 은 발행하지 않는다(R6) · 잔재는 run-id 로 정해진 경로만 다룬다.
6. **성능** → 이 페이로드의 22.27 t/s 는 탐색 합격선 대비 REFUTE · 반복 밴드 17.14% 인 관측이다 → 네 환경에서 측정 클라이언트를 서버와 같은 런타임 env 의 래퍼로 두고(W12) full 측정 · E 검색 포함 적대 판정으로 새로 판정한다.

실패 신호별 행선지: 이미지 빌드 · 버전 핀 · 패치 · wheelhouse 재포장 문제 → `upstream-version-watch` · 서빙 설정 · KV · 노브 · 예산 문제 → `vllm-recipe-explorer` · 성능 기대 이하 · 판정 → `adversarial-benchmark` · 노드 · 토폴로지 · 인터커넥트 · 서브 배달 문제 → `terraforming_node`.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.29.0rc6/qwen3.8-flash-next-nvfp4/gb10-1g2n-cluster-native-bare/qnvfp4-len262144-kvauto-plemmap-spec3-eager` |
| 형식 | `hint-payload/v6` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26092301-hint-v6-e2e · nv4-bf-262k-mmp-native · cluster |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26092301_hint_v6_e2e__cluster__nv4_bf_262k_mmp_native |
| work-manifest | docs/_evidence/camp_26092301_hint_v6_e2e__cluster__nv4_bf_262k_mmp_native.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-09-23T01:19:47Z |
| 승인 발화(전사) | 사용자(발화 전사, G4) — D1 은 hint 퍼블리셔 E2E 이므로 성능하락 원인 분석 없이 REFUTE 로 처분·보존하고, 서빙은 성공했으므로 hint 태그를 발행한다. 이렇게 작업하고 나머지 캠페인(D2·N1)을 같은 방식으로 진행한다 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:63`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `독립 사실검증 에이전트`(저작자 `hint 저작 에이전트` 아님) · 주장 155건 · 지적 7건 중 고침 7건 · 열림 0 · 2026-09-23T08:36:48Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 |
|---|---|
| `HINT_MISSING_CERTIFICATE` | 인증서 부재 — full PASS 가 아니었거나 벤치마커가 발행하지 않았다. 인증서 발행은 adversarial-benchmark 의 책임이지 발행기의 책임이 아니다. |
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

- **발행 경위(사실)**: 이 태그는 hint-publisher 재구성 E2E 캠페인 `camp-26092301-hint-v6-e2e` 의 N1 셀 발행이다. 발행 승인 전사(캠페인 선언 `hint_targets[0].approval` · 2026-09-23T01:19:47Z · 승인 셀에 이 셀 포함): `사용자(발화 전사, G4) — D1 은 hint 퍼블리셔 E2E 이므로 성능하락 원인 분석 없이 REFUTE 로 처분·보존하고, 서빙은 성공했으므로 hint 태그를 발행한다. 이렇게 작업하고 나머지 캠페인(D2·N1)을 같은 방식으로 진행한다`. 그래서 이 페이로드도 REFUTE 의 원인을 담지 않고 열린 물음(02 Q1 · Q2)으로 남긴다.
- **이름(사실)**: arch 끝의 `-bare` 는 실행 평면이 native(Docker 없음)라는 표지다 — 사람 결정 `사용자(팝업 전사) — G-N0 "승인" · O-N1 "A: arch에 -bare (Recommended)"`(plan_26092311 · 2026-09-23T03:18:21Z). 같은 노브의 Docker 태그(D1 · arch `gb10-1g2n-cluster-native`)와 이름이 겹치지 않게 하려는 것이며, 끝의 `native`(target — 호스트 GPU 와 같은 HW)와 `-bare`(평면)는 다른 축이다.
- **run 선택(사실)**: 이 태그의 측정은 run 2 다. run 1 은 측정이 성립했지만 정리 증명이 FAIL_CLOSED 였고 사용자가 전체 재실행을 골랐다(testlog_26092317 §2 — 전사 원문은 계보에 없다) — run 1 은 실패 증거로만 쓴다.
- **의견**: 이 발행의 가치는 수치가 아니라 "Docker 없이 같은 이미지 바이트로 분산 서빙하고 스스로 정리를 증명하는" 재현 경로와 그 벽 지도다. 이 zip 만으로는 설치가 돌지 않는다 — 원천 이미지와 저장소 도구(`native_wheelhouse.py` · `native_multinode_serve.py` · 승인 선언)가 필요하다.
