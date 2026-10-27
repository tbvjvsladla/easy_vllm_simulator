# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-t2610271816` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-27T09:16:56Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8(봉인 페이로드 347611393052:PAYLOAD.json campaign.id(재생 전용 대체)) · granite42-30b-v030 · main · publication-replay |
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
| 자격 근거 | `output/single/benchlog/sweep_granite42-30b-v030/level_01/post_health_granite42-30b-v030.json` · `output/single/benchlog/sweep_granite42-30b-v030/level_01/guidellm_granite42-30b-v030.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_granite42-30b-v030/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Granite-4.2-30B dense BF16를 GB10 GPU1개·노드1대 TP1에서 v0.30.0 source-build·길이131072·KVauto·PLE없음·specoff·graph 형상으로 서빙했고, 원래 캠페인의 Full 동시성1 decode 3.9 t/s를 기록했다. 이번 정정판은 그 측정을 재사용한다.
계보의 KV 부족 두 회차(W1·W2)는 절대 클램프를 늘려 넘겼고, 벤치 예산 차단(W3)은 허용 선택지 안의 작은 클라이언트 몫으로 해소했다.
gmu는 승계값(V2)이며 KV 클램프의 과거 조정(V4)을 이번 새 최적화나 모든 환경의 최소 필요량으로 읽지 않는다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | granite-4.2-30b | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| HF repo | ibm-granite/granite-4.2-30b | 체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) |
| HF revision(체크포인트 git HEAD) | 70fc514076785017d93087f3d8b0676426b6b355 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T01:17:15Z 이전 마지막 이동 2026-08-26T00:27:52Z) |
| base_model | ibm-granite/granite-4.1-30b-base | 체크포인트 README base_model(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) |
| base 슬러그 | granite-4.2-30b | model(granite-4.2-30b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030)) · q 축 = /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.30.0 | sweep_index.meta(output/single/benchlog/sweep_granite42-30b-v030) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.30.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=빌드 원장(docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.git_sha(resolved.json 미바인딩 또는 to_sha 없음) · VLLM_REF=v0.30.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | granite-4.2-30b | 서빙 yaml model(output/single/configs/granite42-30b-v030.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · hf_repo(ibm-granite/granite-4.2-30b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len131072-kvauto-t2610271816 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-30b-v030/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-30b-v030/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-30b-v030/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-30b-v030/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-30b-v030/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 131072 | 서빙 yaml max-model-len(output/single/configs/granite42-30b-v030.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype(output/single/configs/granite42-30b-v030.yaml) · vocab kv[auto]←'auto' |
| timestamp | `t2610271816` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 통합메모리에서는 가중치·KV·호스트 오버헤드·벤치 클라이언트가 같은 총량을 나눈다. 과거 선언 바닥16543MiB는 벤치 몫 상한6303MiB도 제한했고, 이번도 bench4096MiB를 선언했다. 전용 VRAM 환경으로 옮길 때 같은 호스트 산식을 그대로 쓰지 않는다(testlog_26101017_08_29_knob프리셋_E2E_셀A_granite42_30b_v030_판정 §4, testlog_26102710_granite42_30b_v030_코덱스_메인_Full_판정 §1·§2).

KV16GiB는131072 한 시퀀스가 들어가지 않아 기동 거부됐고,32GiB는 엔진 계산상131056토큰으로16토큰 부족했다.33GiB에서135168토큰·1.03x를 보고하며 기동했다. 거부와 통과는 관측이지만16토큰이 빠진 기전은 미결이다. YAML 주석의 블록 정렬 설명을 확정 처방으로 승격하지 않는다(W1·W2).

이 모델은 dense BF16라 과거 스트림당 decode가3.83~3.96으로 평탄했다. 대역폭 한계라는 계보의 해석을 참고하되 다른 HW에서 비례 성능이 보장된다고 주장하지 않는다. CIFS 가중치·auto-prefetch 비활성·수분 로드는 실제 로그 관측이며, prefetch on 대조 없이 로드 시간의 단일 원인을 확정하지 않는다(bench_report_26101016_granite-4.2-30b_GB10_0.30.0, 현재 testlog의 엔진 관측).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W1 | 과거KV절대클램프16GiB에서131072길이로기동거부 | KV클램프를늘림; 최종33GiB는W2 | arch-invariant | 현재엔진도33GiB·135168토큰·131072기준1.03x를보고하며health성공 |
| 2 | W2 | 과거32GiB에서수용131056토큰으로목표131072보다16토큰부족해기동거부 | kv-cache-memory-bytes35433480192로여유를늘려기동 | arch-invariant | 현재엔진135168토큰과health200; 이번새클램프조정은없음 |
| 3 | W3 | 과거bench_budget_mib8192에서레벨1측정전bench=BLOCK | 프리셋허용선택지4096으로다시선택해Full완주 | arch-scaled | 과거Full기록과이번bench4096신규FullPASS; 8192벤치는측정전에막혀실제GPU사살관측없음 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 과거판정문서가실제측정을서빙한2차기동의READY도510초로기록 | 반증 | 과거devlog의정정과현재serve로그DONE줄 |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.30.0 | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | ced6857afa0ea7b2e3f0846a62e1394e90f15607 | 빌드 원장(docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.git_sha(resolved.json 미바인딩 또는 to_sha 없음) |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) · 관측: 빌드 원장 describe `v0.30.0` | 빌드 원장(docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.describe(git describe --tags · 빌드 시점) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.30.1.dev0+gced6857af.d20261001 | output/single/benchlog/sweep_granite42-30b-v030/lite_engine_granite42-30b-v030.log:48 · 기동 2026-10-26T23:07:38Z(컨테이너 시계 UTC 가정) · docs/logs/main/sessions/serve-granite42-30b-v030-20261026T230708Z.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.30.1.dev0+gced6857af.d20261001.cu133 | 빌드 원장(docker-image(main-local):sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | 70fc514076785017d93087f3d8b0676426b6b355 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-27T01:17:15Z 이전 마지막 이동 2026-08-26T00:27:52Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.30.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.granite42-30b-v030) IMAGE_TAG |
| 이미지 digest | sha256:123c331f7a5266b069fcb8b9b28009e862d5662f50dcd12e9321fa08937c1c16 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | 26.07 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-30b-v030 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 관측 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-30b-v030 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/IBM/Granite-4.2/granite-4.2-30b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-30b/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

source-build는0.30.0 torch핀2.13.0과NGC alpha torch의 prebuilt wheel C++ABI 벽 때문에 선택했다(plan_26100107_vllm030_bump_서브지침배달_멀티싱글_E2E캠페인 §3). 재현 입력은 v0.30.0 또는 위 FACT가 이미지 빌드 원장에 연결한40자SHA다. 엔진0.30.1.dev0 계열 자기보고와 이미지태그0.30.0, wheel메타는 다른 생산자이므로 구분한다.

전역 resolved.json이0.31.0이면 이0.30.0 이미지의 해소값으로 묶지 않는다. 해당 결손이 있으면 그대로 기재하고 이미지 build-arg/ref·원장 관측을 사용한다.0.31 SHA를0.30 이미지에 옮기지 않는다. 실린 Dockerfile의 리비전·검증 지위도01 §1.1이 정본이다.

arm64·TORCH_CUDA_ARCH12.1a는이GB10 이미지 좌표다. 다른 GPU/CPU에는 manifest·지원 컴파일 target을 다시 대조한다. 현재 driver580.178.04와hostCUDA132는manifest선언이고, imageCUDA13.3.1.008·torch2.13.0a0+9186a08는이미지 관측이다. 호스트OS는이계보에 미기록이다. Torch외 실린 requirements 제약은 flashinfer-python==0.6.18.post1 · humming-kernels[cu12]==0.1.12 · transformers>=5.10.4,<5.15.0 이며 새 설치 버전 실측으로 부르지 않는다. Ray2.48.0은 공유 이미지의 멀티 지원 의존이며 이TP1 모델 고유 필요조건은 아니다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 6개 — `tuned` 1 · `inherited` 4 · `engine-default` 1.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `dtype` | bfloat16 | inherited | 체크포인트 dtype을 승계했고 이번에 다른 dtype대조 없음 | V1 |
| `gpu-memory-utilization` | 0.80 | inherited | 과거 셀의 손 선언 천장 승계; 이번 조정 없고 절대KV클램프가 KV산정에 우선함 | V2 |
| `max-model-len` | 131072 | inherited | 네이티브 길이를 그대로 승계; 이번 길이 레버 대조 없음 | V3 |
| `kv-cache-memory-bytes` | 35433480192 | tuned | 과거16GiB거부(W1)와32GiB16토큰미달거부(W2) 뒤33GiB에서 기동; 이번은 같은값의 새 Full검증이며 새조정 아님 | V4 |
| `kv-cache-dtype` | auto | engine-default | auto선택을 명시했고 현재 엔진 관측은 testlog에 따로 기록; 다른dtype대조 없음 | V5 |
| `max-num-seqs` | 8 | inherited | 값의 최초 산정 출처는 계보에 미기록이며 기존값 승계; 이번에 레버조정 없고 레벨16필터 상한으로 작용 | V6 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① kv-cache-memory-bytes·max-model-len·max-num-seqs는 대상 모델의 토큰당 KV와 목표 길이/동시요청에서 다시 도출하고 엔진의 수용 토큰과 host 예산을 함께 확인한다. 통합메모리의 클라이언트 몫도 새 총량에서 도출한다(W1~W3).
② 이 표에 declared-requirement는 없다. KV 하한 실패는 과거 값을 바꿔 가며 조정한 V4의 근거이며, 정확한33GiB가 모든환경 필수라는 증명이 아니다.
③ dtype·gmu·길이·max-num-seqs는승계, KVdtype auto는엔진 기본 선택이다. 이번에는 서빙 노브를 바꾸지 않았다. 과거 KV조정 이력과 이번 신규 Full 검증을 구분한다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 체크포인트 revision·dtype·64층GQA와목표길이를00 §0.2/트리플렛과 대조한다 → 다르면 모델별로 재도출한다.
2. 빌드ref·torch/CPU/GPU좌표·이미지원장·실린레시피 검증표를 확인한다 → 다른 resolved.json을 억지로 묶지 않는다. 상위 vLLM에서는 inline strip-hoist의 트립와이어와 skip근거를 다시 확인한다. 이셀 고유 런타임 패치는 없다.
3. 가중치·KV·overhead·총량과 bench몫으로 안전 예산을 선언한다 → 사전 차단이면 우회하지 않고 허용 선택지/서빙 예산을 재도출한다(W3).
4. 공식 기동 뒤 health200+추론1회를 확인한다 → 포트·마운트·모델폴더의 reasoning parser plugin도 확인하며 실패하면 그 관측을 보존한다.
5. lite 뒤 같은 부하 Full3회씩·fresh측정키·posthealth·오류를 확인한다 → 다른 세션/형상에서 수치를 그대로 재사용하지 않는다.

빌드·버전핀은upstream-version-watch, KV/서빙노브는vllm-recipe-explorer, 기대이하성능은adversarial-benchmark, 노드·토폴로지는terraforming_node로 라우팅한다. 위는재검증 경로 설명이며 자동 실행 지시가 아니다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-t2610271816` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8(봉인 페이로드 347611393052:PAYLOAD.json campaign.id(재생 전용 대체)) · granite42-30b-v030 · main |
| 발행 모드 | publication-replay |
| 발행 토픽 | camp_26102705_codex_single8__main__granite42_30b_v030 |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__main__granite42_30b_v030.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `campaign_evidence_fix (independent FC6 correction checker)`(저작자 `main_permission_review (FC6 correction author)` 아님) · 주장 584건 · 지적 0건 중 고침 0건 · 열림 0 · 2026-10-27T09:50:59Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 측정 캠페인 camp-26102705-codex-single8 의 인스턴스가 보존돼 있다(campaigns/camp-26102705-codex-single8/cells/granite42-30b-v030) — 재생 모드는 인스턴스를 읽지 않는다(캠페인 모드 발행이면 읽힌다 · output/single/benchlog/sweep_granite42-30b-v030/sweep_index.json bench_knobs.campaign(측정 시점 활성 캠페인 · bench_preset)) |
| `HINT_MISSING_RESOLVE` | `output/<t>/resolved.json` 을 이 빌드에 묶지 못했다(부재 또는 다른 vLLM 버전의 해소값) — torch·NGC·CUDA 는 이미지 관측값만 싣는다. | — |
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

이번 판은 기존 Full 측정에서 발행한 힌트의 인증서 조건 설명을 교정한다. full 인증서는 루브릭 권한이 explicit이고 판정이 PASS일 때만 발행된다. 이 셀은 weak PASS이므로 bench_report를 바인딩하며 full 인증서 발행 대상이 아니다. 산문의 “이번 실행”은 측정 완료 키 2026-10-27T01:17:15Z의 원래 캠페인 실행을 뜻한다. 이번 정정판 저작에서 모델 기동·벤치·성능 측정을 추가하지 않았다.

공식 발행 기록 재생은 보존된 캠페인 인스턴스를 읽지 않는다. 그래서 캠페인 입력·경계 결손이 표시되며, 이는 인스턴스가 삭제됐다는 뜻이 아니다. 원장 표에는 과거 동일 이름 또는 연관 라벨의 8개 묶음이 포함되므로 이번 캠페인 기동 1회와 구분한다. 새 생산자는 실제 측정을 포함한 세션 시작 시각과 해당 세션의 엔진 배너를 근거로 선택한다. 서빙 상한 10분19초는 세션 시작→lite 시작 간격이며, 기동 스크립트의 health 대기 510초와 끝점이 다르다. 이전 판의 예산 선언→lite 상한 10분20초와의 1초 차이는 시작 근거를 정밀화한 결과다. 측정 뒤 도구 개정 이력도 새 발행 시점에 관측되지만 측정 당시 원문을 바꾸지는 않는다.

기존 봉인판과 태그는 유지한다. 새 FACT는 교정된 생산자가 파생했고, 저자는 새 PROMPT 산문·원문 발췌·빈 꼬리만 저작한다. 독립 7부류 사실 검증은 저자와 다른 Agent가 수행한다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.30.0/granite-4.2-30b/gb10-1g1n-main-native/qbf16-len131072-kvauto-t2610271816` — 꼬리 0토큰 · timestamp `t2610271816`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610271816` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json은 빈 목록이다. 직전 봉인판과 같은 측정·서빙 노브를 사용하고 인증서 발행 조건의 문구를 교정하므로 새 형상을 가르는 노브가 없다. BF16·길이·KV dtype·절대클램프·PLE·spec·graph 조건도 동일하다. 중복 이름을 구별할 발행 시각은 공식 도구가 파생한다.
