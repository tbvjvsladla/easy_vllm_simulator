# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610291840` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-29T09:40:00Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102918-handoff-single2 · minicpm5-2b-v031 · main · campaign |
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
| 자격 근거 | `output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/level_01/post_health_minicpm5-2b-v031.json` · `output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/level_01/guidellm_minicpm5-2b-v031.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

MiniCPM5-2B를 GB10 GPU1개·노드1대 TP1에서 v0.31.0 source-build·bf16·길이32768·KVauto·PLE없음·specoff·graph기본으로 새 기동해 Full 동시성1 decode36.98 t/s를 기록했다.
공유이미지 빌드입력경로(W0), 장기로그 KV파싱(W1), CNN행선택(W2)을 교정한 과거 계보를 재사용했으며 이번 입력은 같은 레시피의 신규 synthetic Full이다.
gmu0.39와 절대KV클램프는 승계값(V1·V4)으로 이번 최적화나 필요조건 검증 결과가 아니다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | minicpm5-2b | sweep_index.meta(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| HF repo | openbmb/MiniCPM5-2B | 체크포인트 .git/config remote url(huggingface.co · /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) |
| HF revision(체크포인트 git HEAD) | f97400052a43d642bbc6e9975e2397e3ae6a6b52 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T09:39:38Z 이전 마지막 이동 2026-10-07T05:21:19Z) |
| base_model | 미관측 | README 머리 YAML 에 base_model 없음(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) |
| base 슬러그 | minicpm5-2b | model(minicpm5-2b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z)) · q 축 = /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · exact_tag=미관측 · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | minicpm5-2b | 서빙 yaml model(output/single/configs/minicpm5-2b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · hf_repo(openbmb/MiniCPM5-2B) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto-t2610291840 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(캠페인 assignments(배정 SSOT)) · sweep meta gpu_model 일치 · target: 셀 config target_gpu.gpu_model(선언) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/minicpm5-2b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/minicpm5-2b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| timestamp | `t2610291840` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 통합메모리에서는weights·KV·hostoverhead가같은물리총량에영향을준다. 과거64GB목표천장과현재물리총량은다르므로다른메모리구조에서는예산을다시낸다(plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §3·§4, testlog_26102918_minicpm5_2b_E2E_Full_판정 §기동과 기능). 이번은singleTP1이므로멀티TP인터커넥트결론이없다.

기존절대KV클램프35GiB가KV예약을고정하고gmu0.39는별도기동검사에쓰인다는로그관측이있다. 다른값을뺀대조가없어최소필요량은미결이다. 과거Granite언클램프실패는같은공유프로젝트의자매모델이므로MiniCPM실패기전으로일반화하지않는다(testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §2·§4). PLE/offload/eager대조형상은이셀에서새로시도하지않았다.

새 엔진은 dtype=torch.bfloat16, quantization=None, kv_cache_dtype=auto, enforce_eager=False와 FLASH_ATTN/FlashAttention2를 기록했다. 엔진이 되읽은 KV35GiB는 선언한 절대클램프이며 최소필요량을 새로 측정한 값이 아니다. 엔진 로그의 로드 메모리4.75GiB와 GPU KV토큰873808은 별도 관측이다. completion smoke의 실제 응답 성공과 capability 비선언에 따른 tool/reasoning 전용시험 N/A를 구분한다(testlog_26102918_minicpm5_2b_E2E_Full_판정 §기동과 기능 · 현재 engine.log/smoke.json).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W0 | 과거 0.31.0 공유 이미지 첫 빌드가 use_existing_torch.py 경로에서 실패 | tools/와 루트 경로를 순서대로 확인하고 둘 다 없으면 fail-loud하는 템플릿으로 교정 | arch-invariant | testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §2 P4 메인 빌드 2차 rc=0 |
| 2 | W1 | 과거 CNN 1차 스윕의 lite KV가 N/A여서 GuideLLM 진입 불가 | 0f8800c 머리 우선 파싱 후 비면 꼬리 보완 | arch-invariant | devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 재판정 pass(KV 35.0 GiB) |
| 3 | W2 | 과거 CNN 2차 스윕에서 11 프롬프트가 반복됨 | 전처리 분포 최빈값을 목표로 사용하고 SUPERSEDED 표기 뒤 재측정 | arch-invariant | devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 3차 12런 비중첩 |
| 4 | W3 | 과거 별도 lite-only 발행에서 raw 조인 불성립 | 폴더 이동 대신 beta 규약 경로로 새 측정하고 공식 evidence-untag/add로 포인터 교체 | arch-invariant | testlog_26101621_36_59_lite셀_minicpm5_2b_v031_판정 §3 새 lite raw와 리포트 등록 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | CNN 2차가 Full PASS여서 유효 측정처럼 보였음 | 반증 | devlog_26101620_CNN_fullbench_셀_minicpm5_2b_v031 §이 셀의 여정 3차 비중첩 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) · 관측: 빌드 원장 describe `v0.31.0` · 업스트림 델타 from_ref `v0.30.0`(이 빌드가 올라온 이전 릴리스) | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) · measured(build-time, in-image)) vllm.describe(git describe --tags · 빌드 시점) · output/single/resolved.json upstream_delta.from_ref(해소 기록) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z/lite_engine_head_minicpm5-2b-v031.log:48 · 기동 2026-10-29T09:24:08Z(컨테이너 시계 UTC 가정) · docs/logs/main/sessions/serve-minicpm5-2b-v031-20261029T092333Z.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| vLLM 설치 distribution 메타(출처별 관측 · 다운로드 wheel 증명 아님) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · 빌드 시 원장(후속 이미지 층은 별도 관측) · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | f97400052a43d642bbc6e9975e2397e3ae6a6b52 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-29T09:39:38Z 이전 마지막 이동 2026-10-07T05:21:19Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.minicpm5-2b-v031) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 이 측정에 결속된 노드별 입력 미관측 | 이 측정에 결속된 노드별 구조화 관측 입력 없음(다른 시점 관측의 부재를 뜻하지 않음) |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 이 측정에 결속된 관측 입력 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_handoff2_minicpm5_20261029T092610Z · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/Openbmb/MiniCPM5/MiniCPM5-2B → <manifest.nas_model_path>/Openbmb/MiniCPM5/MiniCPM5-2B/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

source-build는 NGC alpha torch와 prebuilt wheel C++ ABI 불일치벽에 따른 기존 선택을 재사용했다(artifacts/build_recipe/Dockerfile.source-build헤더·plan_26101410 §3). 재현입력은v0.31.0 또는위40자소스SHA다. 엔진dev버전은기동배너, wheel메타는이미지배포메타, sweep0.31.0은이미지태그해소라는서로다른생산자다. 이Full에는인증서가없다.

arm64·TORCH_CUDA_ARCH12.1a는이GB10이미지의좌표다. 다른CPU/GPU에는manifest와지원컴파일target을다시대조한다. 호스트driver580.178.04는manifest선언이고독립노드관측은없다; 과거plan의580.173.02는과거값이다. 이미지CUDA13.3.1.008·torch2.13.0a0+9186a08는imageinspect관측이다.

Dockerfile은16/17history일치의미검증참고형상이다. requirements는원장hash와일치한다. requirements의flashinfer-python==0.7.0.post1·humming-kernels[cu12]==0.1.16·transformers<5.15.0는공유빌드입력제약이며현재설치버전실측으로승격하지않는다. 재구성build의Ray2.48.0도공유멀티지원의존이며TP1모델자체필요조건은아니다. hostOS는현재증거에서미관측이다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | 과거 64GB 목표 천장 환산값; 이번 단독셀에서 조정하지 않음 | V1 |
| `max-model-len` | 32768 | inherited | 과거 사람의 RAG 길이 고정 요구 승계; 이번 레버 대조 없음 | V2 |
| `max-num-seqs` | 8 | inherited | 과거 사람의 동시성 요구 8(D5)을 승계; 계획은 대표8192길이와 min(8, KV_fit@8192) 산식을 제안했지만 실행 산식 결과를 새로 관측한 주장이 아님; 이번 레버 대조 없음 | V3 |
| `kv-cache-memory-bytes` | 37580963840 | inherited | 과거 목표 천장에서 max_safe를 선택한 hand값 승계; 이번 최소필요량 검증 없음 | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① gpu-memory-utilization·kv-cache-memory-bytes·max-model-len·max-num-seqs는대상총량·목표컨텍스트·동시요청의KV필요량과보호여유로다시도출한다. 산식은02 §2.5의계보를출발점으로삼고실제로그와대조한다.
② 제거시실패가확인된declared-requirement노브는이셀표에없다. W0~W3은빌드/측정/증거경로교정이며새serving필수노브증명이아니다.
③ 네값모두inherited다. 과거64GB목표와RAG요구에서왔고이번단독셀은같은값을검증했다. negative-control로선언하지않았으며최적값이아니다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 체크포인트revision·dtype·구조를00 §0.2와대조한다 → 다르면이형상폐기,같으면manifest와이미지좌표검토로간다.
2. source빌드ref·torch/CPU/GPUtarget과Dockerfile16/17경고를확인한다 → 정확빌드바이트없으면재빌드검증을새로수행한다. 상위버전에서는W0의tools경로지원과inline strip-hoist의skip근거를다시보고불필요패치를강제하지않는다.
3. weights·KV·overhead·총량으로안전예산선언후공식기동한다 → health200+추론1회가없으면서빙성공으로세지않는다. 이셀별도런타임패치는없다.
4. parser기능이필요하면capability를선언해전용시험한다 → 이번completion-only관측을tool/reasoningPASS로쓰지않는다.
5. lite뒤동일부하Full3회씩과fresh측정키·posthealth·오류·원시출처를대조한다 → 불성립이면재도출하고옛결과를대신쓰지않는다.

빌드·버전핀은upstream-version-watch,서빙KV/노브는vllm-recipe-explorer,기대이하성능은adversarial-benchmark,노드·토폴로지는terraforming_node로라우팅한다. 명령은재검증절차의설명이며자동실행지시가아니다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610291840` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102918-handoff-single2 · minicpm5-2b-v031 · main |
| 발행 모드 | campaign |
| 발행 토픽 | camp_26102918_handoff_single2__main__minicpm5_2b_v031 |
| work-manifest | docs/_evidence/camp_26102918_handoff_single2__main__minicpm5_2b_v031.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | campaign:hint_targets |
| 승인 시각(UTC) | 2026-10-29T09:21:56Z |
| 승인 발화(전사) | 사용자: 인계 문서를 읽고 싱글브랜치 캠페인 E2E 진행; 승인된 main/sub 각 1셀 Full 및 hint 발행 범위 재개 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `root_independent_factchecker`(저작자 `main_hint_author` 아님) · 주장 52건 · 지적 3건 중 고침 3건 · 열림 0 · 2026-10-29T09:46:04Z |
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

이번 발행은 새 2셀 E2E의 메인 한 셀을 요약한다. 과거 8셀 캠페인의 유효 이미지·같은 모델 레시피·bounded E-empty 판단은 출처를 밝히고 재사용했으며, 새 build 또는 새 외부 검색으로 기록하지 않는다. 반면 health·실제 추론·lite와 GuideLLM12run은 이번 새 실행이다. 현재 사용자 재개 지시와 새 캠페인 hint 사전승인이 발행 근거이며, 과거 승인이나 태그를 새 성과로 세지 않는다. 모델·서빙의 계보는 02에, 이번 노드간 인수 규약 개정의 승인 범위는 새 plan에 남긴다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/minicpm5-2b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610291840` — 꼬리 0토큰 · timestamp `t2610291840`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610291840` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json은 빈 목록이다. 이전 같은 결정론부 셀과 serving 노브가 같으며 이번에는 새로운 구분 노브를 선택하지 않았다. plenone/specoff/graph도 같은 조건이라 꼬리로 쓰지 않는다. 새로운 측정 시각·캠페인은 03과 FACT에서 구분하며, 중복 이름의 발행 시각은 공식 도구가 붙인다.
