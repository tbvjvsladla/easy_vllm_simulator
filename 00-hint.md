# 00 · hint 지도 — 가장 먼저 읽는 파일

<!-- FACT:header -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610271816` |
| 형식 | `hint-payload/v7` · 이름 문법 `v7` |
| 생성(UTC · 주입) | 2026-10-27T09:16:06Z |
| 캠페인 · 셀 · 노드 · 모드 | camp-26102705-codex-single8(봉인 페이로드 f94c9fb9b204:PAYLOAD.json campaign.id(재생 전용 대체)) · granite42-8b-v031 · main · publication-replay |
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
| 자격 근거 | `output/single/benchlog/sweep_granite42-8b-v031/level_01/post_health_granite42-8b-v031.json` · `output/single/benchlog/sweep_granite42-8b-v031/level_01/guidellm_granite42-8b-v031.json` |
| 증거 등급(task_class) | full_benchmark |
| 측정 등급(bench_mode) | full |
| 셀 등급(cell_grade) | `full` — 성능 판정 · 부하 곡선 · 데이터셋 출처는 `03-benchmark.md` |
| 성능 판정(measurement.verdict) | `PASS` — 출처 output/single/benchlog/sweep_granite42-8b-v031/verdict.json(output · verdict_rule 출력) verdict |
<!-- /FACT:grade -->

<!-- FACT:lite_summary -->
<!-- /FACT:lite_summary -->

## 0.1 요약
> 이 절이 답하는 질문: 이 셀은 무엇을 서빙했고, 무엇이 결정적이었으며, 수신자는 무엇을 가장 조심해야 하는가?

Granite-4.2-8B를GB10 GPU1개·노드1대 TP1에서v0.31.0 source-build·bf16·길이32768·KVauto·PLE없음·specoff·graph기본으로서빙했고 이 정정판이 재사용하는 Full의 동시성1 decode12.44 t/s를 기록했다.
과거언클램프보호종료(W1),측정로그범위(W6),CNN캐시재적중(W7)을교정한계보이며근거는 기존 캠페인의 synthetic 측정이며 이 정정판에서 재측정하지 않았다.
gmu0.39와KV절대클램프는승계값(V1·V4)이며최적값이나제거시필요조건으로확인한값이아니다.

## 0.2 유효맥락
<!-- FACT:context -->
| 항목 | 값 | 출처 |
|---|---|---|
| 모델 | granite-4.2-8b | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| HF repo | ibm-granite/granite-4.2-8b | 체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) |
| HF revision(체크포인트 git HEAD) | 41a8a2d41c54ef4a71741b3e62604f4caaec9295 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-26T23:00:21Z 이전 마지막 이동 2026-08-26T00:28:11Z) |
| base_model | ibm-granite/granite-4.1-8b-base | 체크포인트 README base_model(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) |
| base 슬러그 | granite-4.2-8b | model(granite-4.2-8b) − vocab quant_suffixes(naming.base_slug) |
| 양자화 | bf16 | naming-axis(q) — 측정 identity quantization 미관측(sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031)) · q 축 = /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' — 인증서 quantization 이 N/A 라 명명 축 q(체크포인트 config 파생)를 옮겼다 |
| GPU | NVIDIA GB10 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| 토폴로지 · TP | single · TP=1 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |
| 실행 평면 | docker | artifacts.plane_of |
| vLLM(측정 강식별 키 `vllm_version`) | 0.31.0 | sweep_index.meta(output/single/benchlog/sweep_granite42-8b-v031) |

**명명 축** — 태그 이름 = 결정론부(vllm · model · arch · q · len · kv — 도구가 증거에서 파생 · 발행자 입력 ✗) + 꼬리(발행 Agent 가 고르고 서빙 설정 file · key · value 로 근거 대조 · 0.9) + 중복 시 timestamp.

| 축 | 값 | 출처 |
|---|---|---|
| 세그먼트 `vllm` | 0.31.0 | track=선택자 Dockerfile.source-build(docker history build-arg 관측) · ref=docker history build-arg VLLM_REF · version=VLLM_VERSION 미관측 · repo=docker history build-arg VLLM_REPO · sha=output/single/resolved.json upstream_delta.to_sha · VLLM_REF=v0.31.0(업스트림 릴리스 태그 · v 제거) |
| 세그먼트 `model` | granite-4.2-8b | 서빙 yaml model(output/single/configs/granite42-8b-v031.yaml) · hf_repo=체크포인트 .git/config remote url(huggingface.co · /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · hf_repo(ibm-granite/granite-4.2-8b) 마지막 성분 소문자 |
| 세그먼트 `arch` | gb10-1g1n-main-native | derived(hw·gpus_per_node·nodes·role·target·plane) |
| 세그먼트 `recipe` | qbf16-len32768-kvauto-t2610271816 | derived(q·len·kv) + timestamp(이름 중복 · generated_utc KST) |
| 축 `hw` | gb10 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-8b-v031/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · vocab hw[gb10]←'NVIDIA GB10' |
| 축 `gpus_per_node` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-8b-v031/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `nodes` | 1 | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-8b-v031/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `role` | main | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-8b-v031/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) |
| 축 `target` | native | output/single/manifest.yaml(manifest_contract 로더) gpu_model·gpus_per_node·nodes[] · 발행 노드 축(발행 인자 --node) · sweep meta gpu_model 일치 · target: output/single/benchlog/sweep_granite42-8b-v031/roofline.json gpu_model — 루브릭 계산 HW(측정 호스트 manifest) · 캠페인 camp-26102705-codex-single8 인스턴스 보존 · 재생 모드는 읽지 않았다 로 타겟 선언 미관측(재생 전용 대체 · 선언 아님) · target_gpu 'NVIDIA GB10' = 호스트 hw → native |
| 축 `plane` | docker | artifacts.plane_of → cell-env(IMAGE_TAG\|BUILD_DOCKERFILE) · vocab plane[docker]→'' |
| 축 `q` | bf16 | /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json quantization_config·hf_quant_config.json 부재 관측 → 비양자화 · vocab quant[bf16]←'none' |
| 축 `len` | 32768 | 서빙 yaml max-model-len(output/single/configs/granite42-8b-v031.yaml) |
| 축 `kv` | auto | 서빙 yaml kv-cache-dtype 부재 관측(output/single/configs/granite42-8b-v031.yaml) → 엔진 기본값 auto · vocab kv[auto]←'auto' |
| timestamp | `t2610271816` | 같은 이름이 이미 있어 붙였다(이 발행의 generated_utc 를 KST 로 · 결정론) |
<!-- /FACT:context -->

> 이 절이 답하는 질문: 이 지도는 어떤 조건에서만 성립하며, 이 형상은 어떤 기전으로 버티고 다른 형상은 왜 실패했는가?

GB10 통합메모리에서weights·KV·hostoverhead는같은물리총량에영향을준다. 과거64GB목표와현재호스트총량은다르며전용VRAM환경에는그산식을그대로옮기지않는다(plan_26101410_vllm031_bump_14B이하_4종_양노드_서빙_lite §3·§4). 현재singleTP1은멀티인터커넥트관측이아니다.

절대KV23.5GiB는예약을고정하고gmu0.39는별도의기동검사/선언이다. 과거언클램프0.80은KV78GiB를잡아ETA보호에끝났으며, 잠정클램프와최종hand예산으로넘겼다(W1). 물리상한과목표천장환산, KV설정과선언overhead가함께바뀐경로라한노브만성공원인으로단정하지않는다. 현재값의최적성은미결이다.

이번엔진은dtype=torch.bfloat16·quantization=None·kv_cache_dtype=auto·enforce_eager=False·FLASH_ATTN/FlashAttention2·graph캡처0.22GiB를기록한다. sweep/report의일부NA는미전사이며실제로그관측부재가아니다. 일반completion에는reasoning필드/245reasoning토큰이관측됐고전용reasoning시험N/A와구분한다(testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정의기동/스모크원문).

## 0.3 벽 지도 요약
> 02-narrative.md §2.2 의 `kind: wall` 블록(과 §2.4 의 `kind: misdiagnosis` 블록)에서 기계가 생성한 표다(손저작 ✗ · 순서 = 넘은 순서). 원인·해소의 전문과 원문 발췌는 02 에 있다.

<!-- FACT:wall_map -->
| 순서 | id | 증상 | 해소 | 전이등급 | 검증 |
|---|---|---|---|---|---|
| 1 | W0 | 0.31.0 공유 이미지 첫 빌드가 use_existing_torch.py 경로에서 실패했다 | tools/와 루트 경로를 확인하고 둘 다 없으면 fail-loud하는 템플릿으로 교정 | arch-invariant | testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §2 메인 빌드2차 rc0 |
| 2 | W1 | 과거 Granite 언클램프 트라이얼은 KV78GiB 할당 중 ETA 워치독에 사살됐다 | 절대 KV 클램프와 hand 산정으로 전환; 현재23.5GiB 형상을 승계 | arch-scaled | testlog_26101412_v031_14B이하_4종_양노드_상주서빙_판정 §2 정규 스모크 health·추론PASS |
| 3 | W2 | 과거 P2 오프라인 spike에서 문서의 seed/parquet 인자 꼴이 맞지 않았다 | 실제 이미지 스키마로 seed와 split을 명시 | arch-invariant | devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 D1 배선 및 CNN 완주 |
| 4 | W3 | 과거 CNN 1차는 요청 성공에도 lite KV가 N/A여서 Full 진입이 막혔다 | 0f8800c 머리 우선 파싱 후 꼬리 보완 | arch-invariant | testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §3 재판정 pass |
| 5 | W4 | 과거 granite c4 라이브 한 번이 열 매핑 해석에서 실패했다 | JSON 객체 인자 한 덩이로 전송 | arch-invariant | devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 D2 처분 및 CNN 측정 완주 |
| 6 | W5 | 이미지 소스 판독에서 출력 길이 열이 빠질 위험을 발견했다; 이 오류가 발화한 런은 기록되지 않았다 | text·prompt_count·output_count 세 열을 명시 | arch-invariant | devlog_26101619_GuideLLM_데이터셋knob_CNN프리셋_lite등급분리 §3 D3 및 output256 기록 |
| 7 | W6 | 과거 레벨별 engine 로그가 전날·다른 클라이언트 요청까지 포함했다 | ecb7dca 실행 시작 --since 캡처와 /metrics 실행 차분 | arch-invariant | testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §1 엔진·클라이언트 정합 및 requests_delta18 |
| 8 | W7 | Q1 조사 뒤 첫 재측정 c1 run1이 같은 행에 재적중했다 | 그 스윕 폐기·UTC salt 기반 시작 오프셋3fa6eac·재측정 | arch-invariant | testlog_26101707_Q6_granite42_8b_v031_CNN_재측정_판정 §1 2차12런 적중0.0·offset2727 |

**오진 · 정정**(02 §2.4 — 현재지위가 `가설(강등)` 인 기전은 처방이 아니다)

| id | 증상 | 현재지위 | 검증 |
|---|---|---|---|
| M1 | 과거 계획은 실측 입력토큰을1024로 기대 | 반증 | testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 §2·§4 |
| M2 | 과거 높은 로그 hit rate를 CNN 요청의 성질처럼 해석 | 반증 | testlog_26101622_47_43_Q1_prefix_cache_hit_rate_귀속_판정 §1·§2 |
| M3 | Q1이 최초요청 적중0 대조를 과거CNN4셀 전체에 일반화 | 반증 | testlog_26101619_CNN_fullbench_셀_granite42_8b_v031_판정 머리 SUPERSEDED-IN-PART |
<!-- /FACT:wall_map -->

## 0.4 결정론 해소값
<!-- FACT:resolved -->
| 항목 | 값 | 출처 |
|---|---|---|
| vLLM 빌드 입력(종류) | release | naming.vllm_build_input |
| vLLM 빌드 입력(ref) | v0.31.0 | docker history build-arg VLLM_REF |
| vLLM 빌드 계보 SHA(40자) | db9527a46873454610df6dbedf79a36d6bf1a7f6 | output/single/resolved.json upstream_delta.to_sha |
| 직전 릴리스(prev_release) | 해당 없음 — 릴리스 태그 빌드(이름 = 릴리스 자체 · `<직전 릴리스>-g<sha>` 는 커밋 핀에만) · 관측: 빌드 원장 describe `v0.31.0` · 업스트림 델타 from_ref `v0.30.0`(이 빌드가 올라온 이전 릴리스) | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.describe(git describe --tags · 빌드 시점) · output/single/resolved.json upstream_delta.from_ref(해소 기록) |
| vLLM 엔진 자기보고(엔진 로그 기동 배너) | 0.31.1.dev0+gdb9527a46.d20261014 | output/single/benchlog/sweep_granite42-8b-v031/lite_engine_granite42-8b-v031.log:48 · 기동 2026-10-26T22:15:01Z(컨테이너 시계 UTC 가정) · docs/logs/main/sessions/serve-granite42-8b-v031-20261026T221438Z.json — 측정 창을 포함한 현재 serve 세션 안·첫 측정 이전 기동 배너 · 같은 값의 다른 로그 3건 · 지위 `this-measurement` |
| 인증서 `vllm_version`(강한 일치 키) | 미관측 | 생산자 `absent` · 인증서 vllm_version 없음 |
| wheel 메타(`vllm_build`) | 0.31.1.dev0+gdb9527a46.d20261014.cu133 | 빌드 원장(docker-image(main-local):sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034:/opt/easy-vllm/build_ledger.json · measured(build-time, in-image)) vllm.dist_version(빌드 시점 measured · 이미지 안 배포 메타) |
| 체크포인트 revision(HF · git HEAD) | 41a8a2d41c54ef4a71741b3e62604f4caaec9295 | 체크포인트 .git HEAD → refs/heads/main → loose ref(/app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b) · refs/remotes/origin/main 와 일치(받아 온 원격 커밋) — checkout 커밋(작업트리 일치는 미검증) · reflog: 측정 시점 checkout = 지금 HEAD(measured_utc 2026-10-26T23:00:21Z 이전 마지막 이동 2026-08-26T00:28:11Z) |
| 빌드 트랙 | source-build | artifacts 선택자(build-ledger.dockerfile) |
| Dockerfile(선택자) | Dockerfile.source-build | build-ledger.dockerfile |
| vLLM 저장소 | https://github.com/vllm-project/vllm.git | docker history build-arg VLLM_REPO |
| 이미지 태그 | easy-vllm:0.31.0-cu133-aarch64-source | 셀 env(output/single/envs/.env.granite42-8b-v031) IMAGE_TAG |
| 이미지 digest | sha256:339737df6571ce00993e731ef064a8db66085beaa650e4aa63bdb24f414b2034 | sweep_index.meta.image_digest(measured(docker inspect .Image)) |
| torch | 2.13.0a0+9186a08 | docker image inspect Config.Env PYTORCH_VERSION |
| CUDA | 13.3.1.008 | docker image inspect Config.Env CUDA_VERSION |
| NGC 베이스 | nvcr.io/nvidia/pytorch:26.07-py3 | docker image inspect Config.Env NVIDIA_PYTORCH_VERSION = resolved.json ngc_base.image 접두 일치 |
| CPU arch | arm64 | docker image inspect Architecture |
| 드라이버 | 580.178.04 | sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-8b-v031 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 드라이버(노드별 관측) | 미관측 | 미관측 |
| ⚠ 드라이버 관측 ≠ 선언 | 대조 불가 — 선언 580.178.04 · 관측 없음 | 선언: sweep_index.meta.driver_version(output/single/benchlog/sweep_granite42-8b-v031 · 측정 시 조립 — sweep_bench.sh 가 manifest 에서 읽는다) (manifest 선언 옮김) |
| 호스트 OS | 미관측 | 미관측 |

> 재현 좌표는 빌드 입력(릴리스 태그 또는 40자 SHA)이다 — 엔진 자기보고 · 인증서 · wheel 의 버전 문자열은 빌드 입력이 아니다(각 값을 만든 생산자는 출처 칸).

**양자화 구성**(체크포인트 config 관측 · 이름의 `q` 축은 선언 방식 하나 — ModelOpt MIXED 는 층 개수 우세가 보조 규칙)

| 범위 | dtype | 출처 |
|---|---|---|
| 기본 dtype(비양자화 가중치·계산) | `bfloat16` | /app/models/IBM/Granite-4.2/granite-4.2-8b → <manifest.nas_model_path>/IBM/Granite-4.2/granite-4.2-8b/config.json torch_dtype |

**이 모델에 필요한 패치**: 실린 빌드 패치 없음(또는 파일 기록 미관측 — 01 §1.1).
<!-- /FACT:resolved -->

> 이 절이 답하는 질문: 위 값들 중 무엇을 그대로 참조하고 무엇을 네 환경에서 다시 도출해야 하며, 이 빌드 트랙은 왜 골랐는가?

source-build는NGCalpha torch와prebuiltwheel의C++ABI벽에따른공유이미지선택이다(plan_26101410 §3·Dockerfile헤더). 재현입력은v0.31.0또는FACT의40자SHA다. 엔진dev버전은기동배너, wheel메타는이미지배포메타, sweep0.31.0은태그해소다. 인증서는이번weak판정에없다.

arm64와TORCH_CUDA_ARCH12.1a는이GB10이미지좌표이며다른CPU/GPU에는manifest와컴파일target을다시확인한다. driver580.178.04는manifest선언·독립노드관측없음, hostCUDA132와imageCUDA13.3.1.008은다른층이다. torch2.13.0a0+9186a08는imageinspect다.

Dockerfile검증경고와선택리비전은01 §1.1을따른다. requirements의FlashInfer0.7.0.post1·humming-kernels0.1.16·transformers<5.15.0은공유빌드입력제약이며설치버전실측으로승격하지않는다. Ray2.48.0은공유멀티지원의존이지이TP1모델필수조건이아니다. hostOS는현재증거에서미관측이다.

## 0.5 서빙 노브와 값의 지위
> 01-artifacts.md §1.3 의 `kind: value-status` 블록에서 기계가 생성한 요약이다. 지위가 `tuned` · `declared-requirement` 가 아닌 값은 **이 셀에서 조정된 적도, 빼면 깨진다고 확인된 적도 없다** — 필요조건으로 복사하지 마라. `declared-requirement` 는 빼거나 바꾸면 실패가 관측된 값이다(근거의 W id · artifacts 헤더를 보고 네 환경에서도 그 실패 조건이 성립하는지 판단한다).

<!-- FACT:knob_status -->
노브 4개 — `inherited` 4.

| 노브 | 값 | 지위 | 근거 | id |
|---|---|---|---|---|
| `gpu-memory-utilization` | 0.39 | inherited | 과거64GB목표와두모델공존의호스트환산손값승계; 이번조정없음 | V1 |
| `max-model-len` | 32768 | inherited | 사람RAG길이요구승계; 다른값대조없음 | V2 |
| `max-num-seqs` | 8 | inherited | 사람동시성요구8승계; 계획산식의실행관측은주장하지않음 | V3 |
| `kv-cache-memory-bytes` | 25232932864 | inherited | 과거목표천장의max_safe손산정승계; 이번조정없음 | V4 |
<!-- /FACT:knob_status -->

> 이 절이 답하는 질문: 이 레시피를 다른 환경으로 옮길 때 어떤 노브를 먼저 다시 재야 하고, 어떤 노브는 빼면 깨지는가?

① gpu-memory-utilization·kv-cache-memory-bytes·max-model-len·max-num-seqs는대상총량·목표길이·동시요청의KV필요량과보호여유로다시도출하고엔진관측과대조한다.
② 이셀표에는제거/변경실패로확인된declared-requirement노브가없다. W1은과거손수렴경로이며현재값최적성증명이아니다.
③ 네값모두inherited다. 과거64GB목표와RAG요구에서온손값이며이번단독셀은같은값의신규Full을확인했다. negative-control이나최적값으로승격하지않는다.

## 0.6 재검증 · 라우팅
- 이 자료를 `vllm-recipe-explorer` 인터뷰의 **warm-start 근거로만** 투입하고, 평소대로 plan-gate → 레시피 수렴 → 스모크 게이트를 그대로 밟아라(가속이지 우회 ✗).

> 이 절이 답하는 질문: 다른 환경(다른 HW · 다른 vLLM 버전 · 다른 노드 수)에서 이 지도를 쓸 때 무엇부터, 어떤 순서로 확인해야 하며, 실패하면 어디로 돌리는가?

1. 체크포인트revision·구조·dtype를00 §0.2와대조한다 → 다르면형상을재도출한다.
2. 빌드ref·torch/CPU/GPU좌표와01의Dockerfile검증경고를읽는다 → 정확빌드바이트없이검증된재현빌드라고부르지않는다. 상위vLLM에서는W0경로와strip-hoist의skip근거를다시확인하며자체패치를강제하지않는다.
3. 총량·가중치·KV·overhead로안전예산을선언하고공식기동한다 → health200+추론1회가없으면서빙성공이아니다. 현재별도런타임패치가없음을서명부재와구분한다.
4. tool/reasoning기능이필요하면capability를선언해별도시험한다 → 이번completion관측을전용시험PASS로쓰지않는다.
5. lite와Full3회씩·fresh측정키·posthealth·오류·데이터셋출처·prefixcache실행차분을대조한다 → 불성립이면재도출하고옛태그나수치로대신하지않는다.

빌드·버전핀문제는upstream-version-watch,서빙KV/노브는vllm-recipe-explorer,기대이하성능은adversarial-benchmark,노드·토폴로지는terraforming_node로라우팅한다. 위는재검증경로설명이며자동실행지시가아니다.

## 0.7 메타
<!-- FACT:meta -->
| 항목 | 값 |
|---|---|
| 태그 | `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610271816` |
| 형식 | `hint-payload/v7` |
| 앵커 | `PROVENANCE.json` — 봉인 때 hint 브랜치 페이로드 커밋에 묶인다 |
| 캠페인 · 셀 · 노드 | camp-26102705-codex-single8(봉인 페이로드 f94c9fb9b204:PAYLOAD.json campaign.id(재생 전용 대체)) · granite42-8b-v031 · main |
| 발행 모드 | publication-replay |
| 발행 토픽 | camp_26102705_codex_single8__main__granite42_8b_v031 |
| work-manifest | docs/_evidence/camp_26102705_codex_single8__main__granite42_8b_v031.work-manifest.json |
| 증거 등급(task_class) | full_benchmark |
| 승인 출처 | cli:--approved-by |
| 승인 시각(UTC) | 2026-10-26T21:47:32Z |
| 승인 발화(전사) | 사용자: 싱글 메인·서브 각각 서로 다른 4개 모델 총 8개 고유 모델, 모든 모델 Full bench 성공 후 hint 태그 발행까지 수행 |
| 현행 full 정의 충족(측정 등급) | 예 · 이 측정의 반복 3 · 정의 출처 `.claude/rules/docs.md:42`(03 §3.5) |
| 독립 사실 검증(발행 전) | 통과 — 검증자 `campaign_evidence_fix (independent FC6 correction checker)`(저작자 `main_permission_review (FC6 correction author)` 아님) · 주장 606건 · 지적 0건 중 고침 0건 · 열림 0 · 2026-10-27T09:47:14Z |
<!-- /FACT:meta -->

<!-- FACT:missing -->
**결손** — 발행을 막지 않고 기재한다(부재와 실패는 다른 사실이다 · 차단은 양성 검출만).

| 코드 | 뜻 | 이 셀의 사유 |
|---|---|---|
| `HINT_MISSING_CAMPAIGN_INSTANCE` | 캠페인 인스턴스를 읽지 않았다(발행 기록 재생 — purge · 캠페인 밖 측정 · 보존됐으나 재생 모드라 읽지 않음 중 어느 쪽인지는 이 셀의 사유) — 셀 lockset·config·cell.status·journey·grounding 이 이 페이로드에 없다. 값의 지위와 여정 한 줄은 문서(testlog·devlog) 서사에서만 복원된다. | 측정 캠페인 camp-26102705-codex-single8 의 인스턴스가 보존돼 있다(campaigns/camp-26102705-codex-single8/cells/granite42-8b-v031) — 재생 모드는 인스턴스를 읽지 않는다(캠페인 모드 발행이면 읽힌다 · output/single/benchlog/sweep_granite42-8b-v031/sweep_index.json bench_knobs.campaign(측정 시점 활성 캠페인 · bench_preset)) |
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

이 판은 재현 절차의 full 인증서 발행 조건을 바로잡은 정정판이다. 생산자 ae5c598의 새 FACT는 루브릭 권한 explicit과 PASS가 함께 필요함을 명시한다(01 §1.4·03 §3.3). 이 셀은 weak PASS여서 인증서 없이 bench_report를 바인딩한다. 근거 측정은 2026-10-26T23:00:21Z의 기존 Full이며, 이번 정정 작업에서 새 기동·벤치·성능 측정은 수행하지 않았다. 서사의 이번 실행은 그 기존 캠페인 측정을 가리킨다. 앞선 봉인 draft와 태그는 보존한다.

이 판은 publication-replay다. `HINT_MISSING_CAMPAIGN_INSTANCE`는 보존 중인 캠페인 인스턴스를 이 재생 경로가 읽지 않아 config·lockset·상태·여정을 싣지 않았다는 범위 표기다. 원장 FACT도 캠페인 경계를 적용하지 않으므로 과거 동명·중첩 라벨의 묶음을 이번 측정의 기동 횟수로 합산하지 않는다. 이번 측정을 서빙한 현재 세션은 01 §1.4의 세션 출처와 testlog_26102708_granite42_8b_v031_코덱스_메인_Full_판정에 연결된다.

정적 도구 선택기의 범위와 실제 호출 관측을 구분한 앞선 교정도 유지한다. 측정 후 도구 개정 이력과 현재 세션에 묶인 엔진 배너 출처를 새 생산자가 갱신했으며, 당시 측정 수치나 실행 바이트를 새로 만든 것이 아니다. 새 초안은 독립 Agent의 7부류 사실검증 대상으로 넘긴다.

## 0.9 이름 꼬리
> 태그 이름의 결정론부(`<vllm>/<model>/<arch>/q·len·kv`)는 도구가 정한다. 꼬리는 이 셀을 **같은 q·len·kv 의 다른 셀과 가르는 노브**를 발행 Agent 가 골라 근거와 함께 적는 자리다(plan_26092908 §4.1 U5). 아래 표는 publish 때 기계가 낸 **후보**이고(강제 ✗), `continue` 가 draft `inputs/tail.json` 을 근거 대조한 뒤 확정 꼬리로 바꾼다. 같은 이름이 이미 있으면 끝에 발행 시각 `-t<YYMMDDHHMM>`(KST)이 붙는다.

<!-- FACT:name_tail -->
**확정 이름** `hint/0.31.0/granite-4.2-8b/gb10-1g1n-main-native/qbf16-len32768-kvauto-t2610271816` — 꼬리 0토큰 · timestamp `t2610271816`.

_빈 꼬리 — 발행 Agent 가 가르는 노브가 없다고 명시했다(`inputs/tail.json` = `[]`)._

- `t2610271816` = 같은 이름(결정론부 + 꼬리)이 원격 · 로컬에 이미 있어 붙인 발행 시각(generated_utc 의 KST `YYMMDDHHMM`) — 같은 셀의 새 판이다(옛 태그는 교정하지 않는다).
<!-- /FACT:name_tail -->

> 이 절이 답하는 질문: 이 셀을 같은 q·len·kv 의 다른 셀과 가르는 노브는 무엇이며 그것을 이름 꼬리로 어떻게 적는가 — 가르는 노브가 없으면 왜 빈 꼬리인가?

inputs/tail.json은 빈 목록이다. 직전 봉인판과 같은 측정·서빙 노브를 사용하고 인증서 발행 조건의 문구를 교정하므로 새 형상을 가르는 노브가 없다. PLE·spec·graph 조건도 동일하다. 중복 이름을 구별할 발행 시각은 공식 도구가 파생한다.
