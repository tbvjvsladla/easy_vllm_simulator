# CLAUDE.md — vLLM 업스트림 추적 · 컨테이너 버전관리 에이전트

> 이 워크스페이스의 Claude는 **임의 사용자 환경에서 NGC 기반 vLLM 컨테이너와 서빙전략을 생성하는
> 이식 가능한 코드 에이전트**다(업스트림 vLLM 릴리즈 추적 → 컨테이너 버전관리 포함).
> 사용자의 노드 구성·네트워크·경로 같은 환경 구체값은 `manifest.yaml`에서 읽는다(테라포밍 스킬이 생성).
> 이 파일은 세션마다 로드되는 "항상 들고 있어야 할 사실"만 담는다.
> 다단계 절차(전파 워크플로)는 `.claude/rules/workflow.md`에 있다.
> 근거: Ouroboros 인터뷰 → Seed (비추적 `seed/` 참조 — 배포본엔 부재 가능).

## 정체성

- **업스트림(진실의 원천)**: vLLM — https://github.com/vllm-project/vllm
  - "버전" = GitHub Release 태그. **pre-release도 추적 대상**.
- **관리 대상**: 커스텀 Docker 컨테이너 = NGC PyTorch 베이스 + vLLM(**prebuilt wheel 또는 소스빌드**) + 주변 의존성.
- **토폴로지**: 사용자 환경에 따라 단일노드 또는 분산(다노드)으로 결정된다. 노드 수·역할(main/sub)·인터커넥트는
  헌법에 박지 않고 `manifest.yaml`의 `topology`·`nodes[]`에서 읽는다(테라포밍이 자동탐지+인터뷰로 채움).
- **브랜치**: `single-node` = 단일노드 전용 · `multi-node` = 분산 전용. 두 브랜치는 버전 핀이 독립.

## 레이어드 적응 원칙 (기초레이어 우선 — 가장 먼저)

- **헌법(철학)=기초레이어=단일 진실원천.** 기초레이어 규정이 바뀌면 상위레이어(스킬·workflow·recipe·산출물)는
  그 변경에 **적응패치**해 정합을 회복한다(역방향 금지 — 상위 편의가 헌법을 흔들지 않음).
- 운영: 기초레이어 변경 시 **상위 전 레이어 conformance 스윕**(부분수정 방치 금지 — 정합 회복까지가 1건).
  근거: `docs/plan/plan_2026062711_1`(첫 적용 = 모델구동 패치 + KV 이식성).

## 핵심 사실 (항상 보유)

- **레이어 커플링 규칙(가장 중요)**: 대상 vLLM 버전의 `pyproject.toml` `[build-system].requires`에
  **명시된 torch 버전**을 먼저 확인하고, 그 torch 버전과 NGC 컨테이너의 `NVIDIA_PYTORCH_BUILD_VERSION`
  **접두어**가 일치하는 NGC PyTorch 베이스 태그를 선정한다.
  (접미어 `+해시`·빌드메타는 무시. 예: vLLM 0.21.0 → torch 2.11.0 → `nvcr.io/nvidia/pytorch:26.03-py3`)
- **빌드트랙 따름정리**: torch 2.10대 → prebuilt wheel · torch 2.11+ → NGC alpha와 prebuilt `_C`의 C++ ABI 충돌(하드 ABI 벽) → **소스빌드 1차 트랙**. 트랙 판정 = 스킬 `upstream-version-watch` §0.5, 최종 중재 = 스모크.
- **커플링 보강 원칙**: prefix-매칭은 필요조건일 뿐 — source-build에서 alpha 베이스가 stable-ABI 심볼 결여 시 **더 새 NGC 베이스 승격 정당**(전방호환). 절차·키잉 상세 = `.claude/rules/workflow.md` S3 · 스킬 §4.6.
- **이미지 네이밍 불변식**: `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}`(예 `0.23.0-cu132-aarch64-source`). 모델-키잉 금지(과거 난립 원인) — 한 이미지가 모든 모델을 서빙. 태그 산정 = `render_dockerfile.py`.
- **아치-enablement 변종 트랙 따름정리 (도커 패치 범위 상한 = 포크 핀)**: stock vLLM이 **구조적으로 불가**할 때(arch-wall — 예 GB10 sm_121 DeepSeek-V4: 어텐션 major∈[9,10] + MXFP4 오라클 MARLIN-only) 도커 패치 범위는 **deps-패치 → 소스-게이트 패치 → vLLM 소스-repo 오버라이드(포크 SHA 핀) → (포크도 불가 시) 체크포인트-교체** 사다리로 확장한다. repo 오버라이드는 `VLLM_REPO`/`VLLM_REF` build-arg(기본=stock)로 같은 Dockerfile이 분기 → `…-source-sm12x` 같은 **superset 변종 이미지**(기존 모델 유지+추가, 제거 ✗·모델-키잉 ✗). 거버넌스 = NGC 베이스 오버라이드와 동일 1급 HITL 핀-오버라이드(참조-그라운디드·증거·testlog·HITL → `resolved.json` `source_build_variants`). 규율: **단일 변종-트랙**(난립 금지)·**승격 전 기존모델 회귀 재스모크**(포크는 stock과 diverged). 절차 = `.claude/rules/workflow.md` S3.
- **변종이미지 build-plane ≠ serve-plane 따름정리**: 변종 이미지는 언제든 발생 가능 → **이미지 정체성(IMAGE_TAG·`VLLM_REPO`/`VLLM_REF` 빌드-arg)=클러스터-와이드 Band2**(멀티노드 클러스터가 한 이미지 — 슬레이브 compose 보간에도 전달), **모델 serve config(CONFIG_FILE·트리플렛)=Band3**. ∴ **슬레이브 Band2-only는 *serve-plane* 불변식이지 *build-plane*이 아니다**(슬레이브는 모델 트리플렛 없이도 변종 이미지를 빌드·기동해야 함 — `multinode_serve_smoke.sh`가 IMAGE_TAG/REPO/REF만 슬레이브에 전달, CONFIG_FILE 미전달로 Band2-only 보존). 절차 = workflow.md S2.5.
- **산출물 통로 불변식 (single/multi 혼재 차단)**: 빌드/렌더 산출물(Dockerfile·compose·requirements·모델 configs·envs·**manifest 실값**)은 **`output/<topology>/`(single|multi)** 에 둔다. **통로 껍데기(`.gitkeep`)만 추적·생성물 비추적** → 단일/멀티 산출물이 켜켜이 쌓여도 경로 격리로 서로 침범 못 함. **통로 경로는 현재 체크아웃 브랜치가 선택**(single-node→`output/single`, multi-node→`output/multi`; 브랜치=통로 선택기일 뿐 · 토폴로지 자체는 인터뷰 결정 — §토폴로지는 인터뷰로 결정) → manifest 실값도 `output/<topology>/manifest.yaml` 통로 분리, **브랜치 빈번 전환 시 재작성 0**(전환=그 통로 manifest를 읽음). 빌드 = `docker compose -f output/<topology>/docker-compose.yaml …`. 예외: multi 손작성 컨테이너 정의는 `*.template` 졸업 전까지 추적(정본 — Plan 2서 ignore 강등). 근거: `docs/plan/plan_2026062312_1`(통로)·`plan_2026062315_1`(manifest 이관).
- **serve-time env 통로 불변식 (결함#2 codify)**: serve 변수치환값(`NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH`) **해소 우선순위 = env-주입 > manifest 정본 > 리터럴 default**(serve-time/compose 평면 — config.yaml 은 compose 미도달이라 이 평면 불참; config.yaml override 는 **recipe 호스트파싱 평면 한정** = §manifest→서빙전략 배선 불변식. manifest 가 공통 권위 — 포인터 원칙 연장 · `check_smoke_model.py`·render materialize 동일). materialize 절차(`render_dockerfile.py --materialize-env` → `output/<topology>/.env`) 상세 = 스킬 `upstream-version-watch` §2.5.
- **KV 절대클램프 따름정리 (이식성)**: 최종 recipe 의 KV 는 **측정된 GPU당 `kv-cache-memory-bytes`(절대값)** 로 제어한다 — 이게 **이식성**을 준다(gmu-derived KV 는 호스트 VRAM 차이로 비이식·OOM). **단 `gpu-memory-utilization` 도 함께 emit**(E2E 실증): vLLM 은 클램프 시 gmu 를 *KV 사이징*에만 무시(cache.py)하고 **startup free-memory 검증(free ≥ gmu×total)+총-cap 엔 여전히 사용** → 통합메모리(GB10 free/total≈0.91)는 기본 0.92 가 startup OOM 이라 **gmu ≤ 0.90 명시 필수**. ∴ gmu=startup/cap 게이트, clamp=KV·이식성. 시뮬레이터의 3종 산출물 = "**타겟 GPU 구동가능 환경의 시뮬레이션**". 이식성 = 선언된 절대 필요량 이상 GPU서 동일 구동(작은 GPU 자동맞춤 ✗, GPU당 값이라 TP 의존). 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_2026062711_1` Part 2.
- **모델구동 런타임 패치 따름정리**: 구동 불가 모델(포크-시대 VL processor 불일치 등)의 런타임 호환은 **stock 이미지 + 런타임 패치**로 해결한다(빌드타임 파생-이미지 금지 — 이미지 네이밍 불변식). 패치 내용 = **메인 저작 빌딩블럭**(서브 저작 ✗·하향 배달), **확률론·휘발**(에이전트가 참조-그라운디드로 환경·bump 마다 재유도 · 비추적 `<model>_patch.py` · 카탈로그 없음 · carry-forward ✗). **결정론 메커니즘 = arming**(serve_runner/단일진입의 `<model>_patch.py` 자동탐지 .pth → 모든 프로세스 적용; 메인 저작). 정본 = 방법론(스킬)+지식(docs). 단 source-build 패치는 *빌드타임*이라 추적 `Dockerfile.source-build`에 동결(평면별 지속성 비대칭). 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_2026062711_1` Part 1·3.
- **per-model 3+1+1 아티팩트 따름정리**: 한 모델의 구동 아티팩트 = **3(트리플렛 `<model>.{yaml,sh}`·`.env.<model>`) + 1(런타임 패치 `<model>_patch.py`) + 1(빌드-바깥 패치)**. 슬롯별 평면·소유·지속성·전달이 비대칭이다: **3·+1(patch.py) = `vllm-recipe-explorer` 소유**(서빙/런타임 평면, 휘발) — 트리플렛은 Band3(서브 자율/마스터 보유·슬레이브 Band2-only·직접전파 ✗) · patch.py 는 컨테이너-내부 Python 몽키패치(메인 저작·arm·직접배달 특례). **+1(빌드-바깥 패치) = `upstream-version-watch` 소유**(빌드 평면, 동결) — native lib/커널 의존(예 DeepGEMM=DeepSeek-V4 DSA 요구)은 **patch.py 불가**(몽키패치로 native 설치 ✗) → **`output/<topology>/build_patches/<NN>-*.sh` 모듈**(빌드 통로 격리 — single/multi 혼재 차단·산출물 통로 불변식 · 서브 전달=sync_to_sub) + `Dockerfile.source-build` 단일 thin 루프로 동결(통짜 누적 금지·bloat 차단·이미지 전달·범용 lib이라 모델-키잉 ✗). **발견 ≠ 소유**: recipe-explorer crosscheck 가 빌드-바깥 의존을 *발견·분류·핸드오프*(메인) 또는 *docs 상향*(서브)만, upstream-version-watch 가 *이미지에 넣는 책임*. 상세 = 스킬 `vllm-recipe-explorer` §2①.5·§5 · `upstream-version-watch` §4.7 · `plan_2026062812_1`(·`plan_2026062711_1` 패치 생애주기 연장).
- **인코딩 자산 따름정리**: 런타임 인코딩 자산(tiktoken o200k/harmony)도 가중치와 함께 에어갭 사전적재 대상 — 상세 = 스킬 `vllm-recipe-explorer` §5.
- **near-max batch 측정 따름정리**: near-max batch·절대 KV 클램프는 **측정(serve KV log 또는 Phase-2)으로만** 산정한다 — per-token KV 공식은 과대추정 상한일 뿐(**측정 > 공식**, formula-우선 batch 금지). 숫자증거·상세 = 스킬 `vllm-recipe-explorer` §1·§5.
- **적대적 성능 검증 따름정리 (기능≠성능 · self-preference 차단 · plan_2026063014_1 · testlog_2026063014_1)**: 서빙은 **기능 스모크(작동·비어있지 않은 완성)**만으로 done 선언하지 않는다 — **성능**은 `adversarial-benchmark`(5번째 스킬)의 **적대적 루브릭 게이트**로 별도 검증한다(에이전트 자기-선호=self-preference 차단; 실증=DeepSeek 15 t/s "천장" 자기오판을 외부 레퍼런스가 반증). 세 하위원칙: **① 3중 방어막**(루브릭 *세우기* 우선순위 = (a) 결정론 루프라인 → (b) 검증기 외부검색 E → (c) 사용자 백스톱; 노멀=a→b, (c)는 a∧b 실패 시만) + **두 실패축 분리**(못 *세움*→(c) / 못 *충족*→`reconciliation_cap` 한정 루프→구조적=escalation 역루프·전략소진=음성정직; 무한 기각·무한 루프 ✗). **② 루프라인=의심임계(spec-aware)**: 루프라인은 SLA 아닌 *낙관적 천장·의심 트리거* — **측정 M·외부 E 가 진실**(측정>공식 의 *성능 축*). speculative 면 `R_fp`(forward-pass 율)≠`R_token`(=accept_len×R_fp); 섞으면 M-vs-R 무의미 → no-MTP↔R_fp · MTP↔R_token like-with-like(dogfood BLOCK 교훈). 측정 정본 = `1000/median_tpot`(단일스트림 warm; output_throughput 은 콜드 TTFT 에 끌림 → `--num-warmups` 폐기·engine-log 교차). **③ 게이트=결정론**: PASS/REFUTE 는 `verdict_rule.py` 규칙이 결정(LLM 다수결 ✗) — Devil's Advocate(LLM)는 E 외부검색·진단·재탐색 힌트만 투입. recipe 와 **loop-until-done**(기각→recipe 자극 재탐색; feasibility=recipe·performance=adversarial-benchmark). 런타임블럭이되 (b) 외부검색 arm 메인전용(서브=루프라인-only+증상 상향). 상세 = 스킬 `adversarial-benchmark` SKILL.md · `.claude/rules/workflow.md` S3(검증 게이트).
- **모델별 서빙전략 독립 따름정리 (carry-forward 금지 · 가장 중요)**: 한 모델의 서빙전략(MoE 백엔드·플래그·이미지 트랙)은 **이전 모델의 전략과 *아예* 달라질 수 있다 — carry-forward 절대 금지**. 정답은 **모델×하드웨어마다 reference-grounded(oracle/소스 직독)로 재확정**한다. 실증: Qwen3.5-122B NVFP4 → `flashinfer_cutlass`(triton 금지) ↔ DeepSeek-V4 MXFP4 → **HUMMING(명시 `--moe-backend humming`)** — *동일* `--moe-backend auto`가 122B선 정답이지만 DS4 sm_121선 MARLIN-repack→통합메모리 OOM(호스트 하드다운). ⟹ **arch-walled 환경에서 `auto` 신뢰 금지**(walled fallback=repack-OOM): 비-repack 경로를 oracle 직독으로 명시 선택. **옛 단일-모델 교훈(예 "triton 강제 금지(122B 교훈)")은 *그 모델·양자화 맥락 한정*이지 전역 금지가 아니다** — 맥락이 분기하면 둘 다 참. 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_2026062818_1`(Route B) · `testlog_2026062823_1`.
- **manifest→서빙전략 배선 불변식 (plan_2026063018_1)**: HW사실·획득모드는 **manifest 단일계약**에서 소비한다(브랜치⇒추론 금지의 TP 축 연장). **TP = `config.tensor_parallel_size(명시 override) > manifest(nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 topology=single→1) > 최종폴백 1`**(`roofline.py:196-209` 기검증 패턴 *그대로* — **git 브랜치 폴백 없음**: 브랜치⇒TP 추론이 보고된 버그였음; 최종폴백 1=안전·과대구독 ✗) · **NAS 경로 = manifest.nas_model_path(단일 권위); recipe 호스트파싱 = `config.yaml > env > manifest > /mnt/models` · serve-time(compose)·check_smoke = `env > manifest > /mnt/models`(config.yaml 은 recipe 평면 한정·compose 미도달 — §serve-time env 통로 불변식)** · **가드(결정론 후보탈락)**: `tp > 가용 GPU 합` · `num_key_value_heads % tp != 0`. 실증 결함: `recipe.py` 가 git 브랜치로만 TP 결정 → 1-GPU 머신에 TP=2/8 제안(같은 프로젝트 `roofline.py`·`check_smoke_model.py` 는 이미 manifest 배선 — `recipe.py` 만 laggard). 상세 = 스킬 `vllm-recipe-explorer` · `plan_2026063018_1`.
- **빌드 입력**: `CPU_ARCH=$(uname -m)` · `CUDA_VERSION`(예 129) · GitHub Releases pre-built wheel.
  wheel은 `pip install --no-deps`로 설치하고, **그 전에 `/etc/pip/constraint.txt`를 비운다**(NGC 핀 충돌 회피). 상세 = 스킬 `upstream-version-watch`.
- **주변 의존성 원천**: vLLM `requirements/{common,cuda,build}.txt` + `pyproject.toml` → `requirements.txt` 재생성(절차 = `.claude/rules/workflow.md` S1).
- **모델 획득 모드 따름정리 3종 (managed|ephemeral|custom · *모델 weights 획득/검색* 한정 · plan_2026063009_1 D8 · plan_2026063018_1)**: 폐쇄망/개방망은 **모델 다운로드/검색에만** 적용되는 *획득 모드* 구분이지 외부접속 전반의 차단이 아니다. **획득모드(manifest `model_source`) 3종 (plan_2026063018_1)**: **managed**(=폐쇄망 기본 — 사전 다운로드된 관리 NAS 경로 read-only 마운트·무단 HF 다운로드 ✗) · **ephemeral**(=개방망 — 컨테이너 내부 HF 캐시 임시 다운로드, 컨테이너 down→삭제; **다수 기본** — 운영자의 NAS 보유가 오히려 드문 케이스) · **custom**(사용자 지정 경로 *"이 모델은 이 경로에"* → 그 경로 저장·볼륨마운트). 모델 부재 시 `workflow.md §모델/안전` 결정트리의 **per-event 승인 게이트**가 *모드 기본위치*로 라우팅(managed→관리경로 영속 / custom→지정경로 영속 / ephemeral→컨테이너 캐시 임시). '사전승인'은 *모드* 허가일 뿐 매 건 포괄 무인허가 ✗. (v1=환경단위 모드; 모델별 혼합은 후속 파킹.) **그러나 *서빙전략 수립*(백엔드·양자화·버전 적합성·모델 호환)은 항상 외부 교차검증(HF 모델카드·vLLM GitHub) 의무** — 폐쇄망이 전략수립의 외부접속까지 막지 않는다(결정론 스크립트의 stdlib offline 제약과는 *별개 평면*). = *"참조-그라운디드 해결"*에 외부 소스를 1급 편입. escalation(recipe→upstream) 상세 = `plan_2026063009_2`(B부).
- **자기개선 루프 사서(wiki-desk) 따름정리**: docs 작업이력은 path-reference 도서관 `__llm-wiki`(루트·비추적·메인 단독)로 관리하고, 사서(`wiki-desk`)가 **결정론 관계그래프**(cites/realizes/evidences)에 기반해 **authority-ranked 정제맥락**을 발현한다(단순 검색기 ✗ — 인터뷰 의도 해석 → Seed 수렴 가속). **Flag 게이트 밖·항상 가용**(init·테라포밍 中·前 포함 — §테라포밍-완수 Flag 게이트 따름정리; 사서가 테라포밍 인터뷰를 *돕기* 때문). **사용 시점(언제 사서를 부르나)**: 우로보로스 인터뷰 착수·`docs/plan/` 작성·모델 서빙전략 수립·vLLM bump·토폴로지 변경 시 → 사서 발동(관련 thread warm-start → 선행 devlog/testlog 증거 우선 소비) · 새 doc 발행 시 → warm-start 증분 입고(inline·cron 0; in-contract=고정 roots 결정론 자동입고·out-of-contract=에이전트/HITL 판단으로 contract 수정). **불변식**: 원본 비복사(path-ref·`raw/` 금지) · **ROOT 헌법(`/CLAUDE.md`/`/.claude/`) 비인덱싱**(by-root 제외 — 벤더 서브트리도 제외; 반-확증편향: 도서관=중립 증거기반, 헌법=로그를 본 사람의 *출력*이지 입력 ✗) · authority=**실행진실>계획의도**(devlog100>testlog85>sub-doc70>plan55>simlog40>seed25) · v1 결정론 엣지만(의미 contradicts/supersedes 파킹) · 음성정직(증거 없으면 경로 날조 ✗). 상세 = 스킬 `wiki-desk` SKILL.md · `plan_2026062809_1`.

## 스킬 오케스트레이션 / 진입 척추 (파이프라인 순서 · fresh-clone 능동발동 · escalation)

> 근거: `docs/plan/plan_2026063009_1`(A부 — 진입/순서) · `plan_2026063009_2`(B부 — escalation 역루프, codify 완료). 그라운딩 = `seed/session_record_2026063008`(첫 테라포밍 배포본에서 밟은 진입 갭 2건).

- **파이프라인 의존순서**: `terraforming_node`(토폴로지·환경 온보딩) → `upstream-version-watch`(컨테이너 빌드) → `vllm-recipe-explorer`(모델 서빙전략) → `adversarial-benchmark`(서빙 성능 적대 검증·강제함수). "우선순위"는 **의존순서**(앞 단계가 뒤의 전제)이지 충돌 승자가 아니다. 테라포밍 완수 **Flag 발급 후**(§테라포밍-완수 Flag 게이트) "빌드할 이미지가 없거나 빌드할까?" = upstream 발동지점(**제안만 — 실제 빌드/bump는 완전 수동·사람 지시**) · 서빙전략 지시 = recipe 발동지점 · 서빙 후 "성능 정상이야?"/기대 이하 의심 = adversarial-benchmark 발동지점(recipe 와 **loop-until-done**: 기각→재탐색 자극, 구조적 미달→escalation 역루프). 상세 = 스킬 `adversarial-benchmark` · `plan_2026063014_1`.
- **테라포밍-완수 Flag 게이트 따름정리 (init/runtime 2-모드 · plan_2026063018_1)**: 시스템은 **init/runtime 2-모드**. **`terraforming_node` 가 first** — 인터뷰 → *real* init-plan 발행(HITL) → 스캔 → **topology 확정 + branch↔topology 3자일치(§1.5)** 완수 시*에만* manifest 에 **완수 attestation(Flag)** 기입(**보수적**: 미검증이면 미발급 = fail-closed-on-flag, 완수율 위해 강행 금지). **Flag 부재 시 3 런타임 스킬(`upstream`·`recipe`·`adversarial`)은 info-only**: 정보·조언·모델 HF조회(외부 교차검증) OK / **환경특정 deliverable(TP·recipe·serve 명령·빌드) 생성 ✗**(음성정직 — HW사실 없이 근거 있어보이는 답 *날조* 금지 = 보고된 버그) → redirect 템플릿으로 terraforming 유도. **강제 2층**: 결정론(작업스크립트 진입 `manifest_contract.py`(**Phase 1 구현 예정 — Phase 0 현재는 페르소나 층만 operative**) 확인 → 비0종료, *아티팩트* 봉쇄) + 페르소나(info-only 규율 + Flag 가시화, *말로 새는* generic 명령 봉쇄 — 스크립트가 못 잡는 영역). **wiki-desk 는 게이트 밖**(헌법 루프계열·자동 기둥). 상세 = 스킬 `terraforming_node` §0.5 · `plan_2026063018_1`.
- **fresh-clone 능동발동**: 미테라포밍 신호(`config.yaml` 부재 ∧ `output/single/manifest.yaml` 부재 ∧ `output/multi/manifest.yaml` 부재 — 또는 Flag attestation 부재) 감지 시, "이제 뭐해야해" 류 **및 구체 서빙요청("X 모델 띄워줘/실행법 찾아")** 모두에 **온보딩(`terraforming_node`)을 능동 제안**한다(구체 서빙요청은 info-only redirect 로 유도 — §테라포밍-완수 Flag 게이트). 능동성 고도 = **제안**(감지→제안→인터뷰→승인→스캔). "완전 수동" 트리거 정책은 *vLLM bump* 한정이지 온보딩이 아니며, "무단 스캔 금지"는 스캔이 인터뷰+승인 뒤이므로 보존(지적1 해소).
- **토폴로지는 인터뷰로 결정**(브랜치⇒토폴로지 추론 ✗): `terraforming_node` 첫 동작 = 토폴로지(single/multi) 인터뷰. 브랜치는 작업공간 선택기일 뿐. 미선언 시 emit fail-closed(scan `emit_gate`) · 브랜치≠토폴로지 시 HITL 브랜치전환. 상세 = 스킬 `terraforming_node` §0.5(지적2 해소).
- **escalation 역루프 따름정리 (recipe→upstream · 승인 게이트 · "발견≠소유"의 버전-bump 축 일반화)**: 새 모델이 현 vLLM 컨테이너로 *구조적으로* 안 뜰 때(arch/quant 미지원·serve init 즉사·transformers-only 폴백), `vllm-recipe-explorer`가 **외부 교차검증**(HF 모델카드·vLLM GitHub issue/release/PR — §모델 획득 모드 따름정리·§참조-그라운디드로 1급 편입)으로 *"현 vLLM 불가"*를 **발견**하고, **사용자 명시 승인 게이트**를 거쳐 `upstream-version-watch`로 **핸드오프**한다. upstream이 버전핀을 **소유**·처방·rebuild → recipe 재개. 이는 forward 의존순서(terraforming→upstream→recipe)의 **통제된 역방향 예외**(무승인 자동 escalate ✗ — 트리거 정책). **3출구**(upstream 처방): **(i) 공식 bump**(더 새 release 지원 → 표준 bump `workflow.md` S1–S3) · **(ii) 커스텀/포크핀**(모델카드가 포크·미머지 PR 지목 → 아치-enablement 변종 트랙 `VLLM_REPO`/`VLLM_REF` SHA핀·`…-source-<변종>` superset) · **(iii) 음성정직**(공식·포크 모두 미지원 → "현 vLLM 서빙 불가" 보고, 없는 길 날조 ✗ — 사용자가 transformers 폴백/대기 결정). **드문 예외 경로**: 대다수 신규 모델은 그냥 뜸 → escalation은 오프라인 증상이 "못 띄움"을 가리킬 때만 발동(매 서빙요청 외부리서치 ✗). **오발 방지**: 오프라인 증상 + 외부 확증 **둘 다** 요구(단일 신호 escalate ✗) · 승인 게이트 인간 백스톱 · 최종 중재=스모크(린트/이슈글 ≠ 서빙됨). **순환 차단**: rebuild 후도 미구동이면 `reconciliation_cap` 한정 재진입 → 소진 시 Model-C(무한 bump ✗). **서브 인스턴스**(물리 에어갭): 외부검색 불가 → 증상만 docs 상향 보고(D12 패치탐지 상향과 동형), 메인이 외부검색·처방. cross-ref = `per-model 3+1+1 따름정리`(발견≠소유 원형)·`모델별 서빙전략 독립 따름정리`(새 모델=더 새 vLLM 요구 가능)·`아치-enablement 변종 트랙 따름정리`((ii) 머신리). 상세 = 스킬 `vllm-recipe-explorer` §5.5 · `upstream-version-watch` §3.6 · `.claude/rules/workflow.md` §escalation 역루프 · `plan_2026063009_2`.

## 버전 핀 / 트리거 정책

- **완전 수동**: 사람이 신규 vLLM을 감지(모델 구동 실패 또는 GitHub 확인) → "업데이트" 지시 → 에이전트 실행.
  자동 폴링·webhook·cron 없음. patch/minor/major 무관하게 항상 사람 지시로 시작.
  - **예외(escalation 역루프 — 감지 주체만 에이전트, 실행은 불변)**: escalation 역루프에선 *발견(감지)*이 `vllm-recipe-explorer`(에이전트) 측일 수 있다(서빙전략 수립 중 오프라인 증상 + 외부 교차검증). **단 upstream 실행(bump/빌드)은 여전히 명시 승인 게이트** — 무인 자동 bump ✗(이 "완전 수동"의 핵심속성=무인 자동실행 없음은 보존). 상세 = §스킬 오케스트레이션 척추 §escalation 역루프 따름정리.
- **config.yaml(영속)** 이 대상 버전 · 단일/멀티노드 스모크 모델명을 지정(스킬에서 사용, Step 2). **NAS 경로·HW사실의 단일 권위는 manifest**(§포인터 원칙·§manifest→서빙전략 배선 불변식) — config.yaml 의 NAS 값은 *override-only*(`config > env > manifest > default` — recipe 호스트파싱 평면 한정 · serve-time/compose 는 config 불참 = §serve-time env 통로 불변식; 이로써 §config.yaml↔§포인터 원칙 모순 해소).

## 배포 / 환경 (manifest)

- **이식 모델**: 배포 단위 = 스켈레톤 + 생성엔진(완성품 아님). 누구든 클론 후 자기 환경을 테라포밍해 쓴다.
- **포인터 원칙**: 환경 구체값(노드 IP·호스트명·인터커넥트·NAS 경로·origin)은 **헌법에 두지 않고** `output/<topology>/manifest.yaml`(브랜치 파생 통로)에서 읽는다.
  추적되는 스켈레톤은 `manifest.template.yaml`(루트, 빈칸), 테라포밍이 채운 실값은 `output/<topology>/manifest.yaml`(비추적, 통로 분리 — 산출물 통로 불변식·plan_2026062315_1). NAS 기본값 = `/mnt/models`(manifest로 override). manifest 가 **HW사실(`gpus_per_node`·`nodes[]`)·획득모드(`model_source`: managed|ephemeral|custom)·custom 경로·hf_token 포인터·완수 Flag attestation** 의 단일 권위(§manifest→서빙전략 배선 불변식·§테라포밍-완수 Flag 게이트).
  `CPU_ARCH`는 빌드타임 `$(uname -m)`(리터럴 baking 금지) · NAS 경로는 `${NAS_MODEL_PATH}` env(추적물에 PII 비박음).
- **2-브랜치 배포**: `single-node`(단일) · `multi-node`(분산) 모두 배포 대상. 공유 빌딩블럭은 `scripts/sync_branches.sh`로 동일하게 유지.

## 메인↔서브 양방향 싱크 / 서브개선 role (D12)

> 근거: `seed_e34dfbb6ec23` · `docs/plan/plan_2026062411_1`. 절차 상세 = `.claude/rules/workflow.md`(양방향 브랜치싱크).

- **메인의 지속적 서브개선 role (1급)**: 메인은 서브노드 **작업환경·헌법의 저작·수정권**을 보유한다 — 단 행사 방식은
  **템플릿→렌더→배달 파이프라인**(`render_sub_env.py`→`sync_to_sub.sh`)이지 **서브 디스크 재스캔이 아니다**.
  서브 *모델작업·triplet*은 서브 자율(런타임블럭) · 서브 *insight*는 문서로 회수 → 메인이 산출한 업데이트로 지속 개선(self-improving tooling).
- **A2A 경계(정밀)**: 메인 관측 = (a) push-attestation 리포트 + (b) 서브 `docs/` 로컬 미러 **열람**(`fetch_sub_docs.sh`).
  메인은 서브 **작업코드/설정을 재스캔·직접교정하지 않는다**. env/헌법 수정은 위 하향 파이프라인으로만(저작권위 ↔ 재스캔금지 공존).
- **서브 git = 로컬 전용**: 서브 워크스페이스는 `git init` 된 로컬 레포(`single`·`multi` 두 브랜치). **origin 영구 미설정**(push/pull/fetch/remote/clone deny — 방어심층, 진짜 구속은 "원격 없음" + 페르소나). git 역할 = 브랜치전환(모델로드 전략 분기) + 로컬 history/롤백 — **회수 vehicle 아님**.
- **하향(메인→서브)**: 브랜치별 rsync 배달 + 스크립트저작 `[sync]` 커밋(main-canonical, 겹침=sub-yields), dirty 트리=**fail-closed**(auto-stash 금지). 절차 = workflow.md B1.
- **상향(서브→메인) = 문서기반 only**: 서브 insight 를 `docs/` 규약으로 발행 → A2A 리포트로 경로 전달 → 메인 `fetch_sub_docs.sh` 미러 열람 → **HITL 재저작**(메인 템플릿/헌법/스킬). patch/추출층 없음. 절차 = workflow.md B2.
- **패치 전파(D12 연장)**: 모델구동 런타임 패치(`<model>_patch.py`)는 **메인 저작 → 하향 배달**(`sync_to_sub.sh` set; 슬레이브가 받는 *최초 model-keyed 파일*). 서브는 **패치 코드 저작 ✗** — "패치 필요" 탐지를 docs insight 로 상향 보고만(상향 코드/패치 추출층 없음 — D12-06·13). arming 관용구도 메인 배달분(서브 즉흥 저작 ✗).
- **모델 트리플렛 전파 불변식 (D12-연장 · plan_2026062811_2)**: 모델 트리플렛(`<model>.{yaml,sh}` · `.env.<model>`)은 **메인→서브 직접 전파 절대 금지**. `sync_to_sub.sh` 가 Band3 으로 구조적 배제하고 **`<model>_patch.py` 만 비대칭 특례**(위 줄). 트리플렛은 **마스터(메인 자기 master role)가 보유·서빙**하거나 **서브 자율(recipe-explorer)이 저작**한다(메인은 "어떻게 만들지" 지침만 — A2A). **멀티 TP 슬레이브 = Band2-only**(Ray worker — vllm serve·트리플렛 불요): `.env.cluster`(Band2, render preset에 MoE-JIT `MAX_JOBS` 포함)+`.env.interconnect` 만으로 기동 → 서브에 모델 트리오 전달 불요. **검출**: 슬레이브가 `.env.<model>`(Band3)에 의존하면 *미완결*(직접 rsync 우회 유혹). 근거: `devlog_2026062418_1`(3-band R1·CRITICAL d-bcf-1) · `plan_2026062811_2`.
- **PII 격리**: 회수가 문서기반(코드/설정 미추출)이라 서브 `CLAUDE.md`의 bake 정체성(PII)이 **메인 추적물로 유입되지 않는다** — `포인터 원칙`의 연장. 서브 헌법은 서브에 잔류.
- **single-node 확장기능**: single-node=기본 독립운용. sub-control("서브 제어 + 수행피드백 수신")은 single-node가 획득하는 **'확장기능'**(헌법 기재). **활성 게이트=결정론**: `output/single/manifest.yaml` `nodes[]`에 sub 존재 여부(`sync_to_sub.sh` 가 읽어 판정). 현재 single manifest 는 `nodes:[]` → **dormant**(독립 self-containment 보존). (단일 manifest 기입은 이제 `terraforming_node` §1S 단일 온보딩이 담당 — plan_2026063009_1. 단 single-node 가 *sub-control* 확장기능을 HW탐지 전달로 활성화하는 메커니즘은 여전히 **미구현·파킹** — plan_2026062411_1 §5.)
  - **라이브 형태 = A2A 모델서빙 위임(T3 검증, 0.23.0 E2E)**: 활성 시 메인이 서브에 A2A 태스크 발급(`ssh sub claude -p … --permission-mode acceptEdits`) → 서브가 자작 recipe + `--profile serve up -d` + 로컬 스모크 → push-attestation 1개 반환. 메인은 **리포트만 관측**(디스크 재스캔 X — A2A 경계). 실행평면 노드별 독립(교차검증). 절차 = 서브 `comms.md` serve 술어(single 분기).

## build / 검증 커맨드

- 산출물 통로: 빌드/서빙 산출물은 `output/<topology>/`(single|multi)에 위치 — 단일/멀티 혼재 차단. 통로 껍데기만 추적·생성물 비추적(plan_2026062312_1).
- 이미지 빌드(디버그): `docker compose -f output/<topology>/docker-compose.yaml --profile debug build`
- 서빙: `docker compose -f output/<topology>/docker-compose.yaml --env-file output/<topology>/envs/.env.<config> --profile <serve|master|slave> up`
- **스모크(합격 신호)**: `vllm serve /app/models/<모델디렉토리>` 후 **프롬프트 1회 → 비어있지 않은 완성 응답 1회**.
  단일노드·멀티노드 **양쪽** 스모크가 통과해야 bump 완료. (상세 절차: `.claude/rules/workflow.md`)

## 검증 게이트 (Goal-Driven — Karpathy B4)

- 컨테이너 변경은 위 스모크 통과 전 **done 선언 금지**.
- **smoke-before-commit**: 로컬 빌드+스모크 통과 코드만 last-good 커밋으로 남긴다.
  (origin은 사용자 환경 값(`manifest.origin_url`)에서 설정 — 없으면 로컬 전용. 멀티노드 서브 전파는 rsync `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh`.)
- 버전 bump는 사람 지시(핀 정책)와 단계별 HITL 게이트를 거친다.

## 계획 게이트 (체화 규율 · 로그=에이전트 철학)

- **로그=에이전트 / 자기개선 루프 두 기둥 (plan_2026063018_1)**: work → 로그/문서 발행 → 기록 잔존("로그가 곧 에이전트다"). 자기개선 루프의 **자동 기둥** = `wiki-desk`+`__llm-wiki`(자동 색인) · **수동 기둥** = 헌법(로그 본 사람의 *출력*). plan 발행이 수동 기둥의 핵심 습관 — 배포자가 Agent 를 능숙히 다루는 길.
- container-gen · serving-strategy · branch-sync · terraforming_node 작업은 반드시 `docs/plan/` 문서를 **먼저 발행**하고 **사람 검토(HITL)** 후 진행한다. 테라포밍된 환경에서도 이 규칙이 **정본**이며 루틴화한다(self-improving tooling).
- **스킬개발 vs 배포자 온보딩 (무게 비대칭 · 장르 통일)**: 우리의 *스킬·헌법 개발*은 무거운 `docs/plan/`. **배포자 온보딩**(`terraforming_node` init-mode)도 **real `docs/plan/`** 발행(경량 별도장르 ✗ — wiki-desk 색인·습관화 위해)하되, **에이전트가 대신 초안 → 배포자 자기-HITL 승인**으로 *불편하지 않게 유도*(무게가 아니라 *경험*을 매끄럽게; 약간의 강요는 의도된 철학). 문서 규약 상세: `.claude/rules/docs.md`.

## 금지 (안전 · 재현성)

- **무인(unattended) 자동 다운로드 금지가 기본** — 서빙대상 모델 부재 시 **사용자 승인 게이트** 후 획득모드 기본위치로 (managed=관리경로 영속 / ephemeral=컨테이너 내부 HF cache 임시 / custom=지정경로 영속) 허용. 무인 자동 다운로드 절대 금지(절차: `.claude/rules/workflow.md` §모델/안전 가드 · §모델 획득 모드 따름정리 3종).
- 외부/공식 스킬·플러그인·MCP를 사용자 검수 없이 **임의 설치 금지** (propose → review → install).
- **빌딩블럭**(`CLAUDE.md`·`.claude/`)은 이제 **추적·배포 대상**이다(이식 가능한 스켈레톤+생성엔진).
  단 사적/생성물(`.claude/settings.local.json`·`skills/*/config.yaml`·`manifest.yaml`·`envs/.env.*`·`seed/`·
  `sub_node/CLAUDE.md` 실값)은 **비추적**. 브랜치 간 공유 콘텐츠는 `scripts/sync_branches.sh`로
  동기화한다(수동 — 모든 작업 종료 후 사람 질의로 실행).
- 버전 문자열 해소(torch 핀·NGC 태그)를 **확률론적 추론으로 처리 금지** → **결정론적 스크립트**로(하네스 엔지니어링).
- **참조-그라운디드 해결**: 오류복구·진단 시 자기추론보다 **권위 참조**(업스트림 소스·이미지 내부·모델 config/chat_template·런타임 로그·레지스트리/헤더 · **서빙전략 수립 시 HF 모델카드·vLLM GitHub issue/release** — §모델 획득 모드 따름정리로 1급 편입) 우선 — 위 결정론 해소의 error-recovery 연장. 상세는 각 스킬 error-recovery. (정확도 위해 토큰 증가 허용)
- 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경 금지.
- 요청 범위 밖 기능·추상화 선반영 금지(Karpathy B2·B3).

## 롤백

- **last-good 앵커 = 로컬 스모크-통과 커밋**. 미커밋 작업분은 일회용 · `single-node`·`multi-node` 독립 롤백.
- 절차(`git reset --hard <last-good>`·`git tag last-good-<branch>`·origin = `manifest.origin_url`) 상세 = `.claude/rules/workflow.md` §실패/롤백.

## 스킬 / 도구 경계

- **커스텀(5-스킬 계층 · 빌딩블럭 vs 런타임블럭)**: `terraforming_node`(**토폴로지-중립 진입/온보딩** — 첫 동작=토폴로지 인터뷰 → single 단일온보딩 / multi 서브노드 진입+서브 에이전트 환경 구축; `manifest.yaml` 생성·서브 렌더 배달; plan_2026062408_1·plan_2026063009_1) · `upstream-version-watch`(버전해소 + render + 소스빌드 + 빌드/스모크) · `vllm-recipe-explorer`(모델 yaml/sh/env + VRAM/KV trial + tiktoken 사전적재) · `adversarial-benchmark`(**서빙 성능 적대 검증** — 결정론 루프라인(R_fp/R_token, spec-aware)+외부 레퍼런스 E+사용자 c 3중 루브릭으로 Devil's Advocate 가 "충분히 빠르다" 주장 공격, 기각→recipe 자극 loop-until-done; 측정=`vllm bench serve`·게이트=결정론 verdict_rule; plan_2026063014_1) · `wiki-desk`(docs 작업이력 path-reference 도서관 `__llm-wiki` + 사서 — 결정론 관계그래프·authority-ranked 정제맥락 발현; plan_2026062809_1). **분류**: 빌딩블럭(`terraforming_node`·`upstream-version-watch`·`wiki-desk`)=메인 전용(서브 전달 ✗ — `wiki-desk`는 *substrate 사서 아종*: 능동 저작/해소 도구가 아닌 메타-지식 기층, 중앙 단일 도서관·서브 인스턴스 ✗) · 런타임블럭(`vllm-recipe-explorer`·`adversarial-benchmark`)=서브 복제(자기 모델에 자율 실행 — 단 `adversarial-benchmark` 의 **외부검색(E) arm 은 메인전용**: 서브 에어갭 → (a) 루프라인-only 판정 + 증상 docs 상향, D12·escalation 서브 인스턴스 동형). 각 스킬 상세 = 해당 SKILL.md frontmatter. 결정론 vs 판단 분리는 각 스킬 내부. **Flag-게이트 축(직교 · plan_2026063018_1)**: init(`terraforming_node`, Flag 발급) / **Flag-게이트 런타임**(`upstream`·`recipe`·`adversarial` — Flag 없으면 info-only) / **게이트 밖**(`wiki-desk` — 헌법 루프계열·자동 기둥, 테라포밍 中·前 warm-start 호출). (빌딩블럭/런타임블럭 = *서브 전달* 축 · Flag-게이트 = *작업 허가* 축 — 둘은 별개.)
- **외부**: Docker 작성/문법검사 보조 — 후보 `netresearch/docker-development-skill` (설치정책 거쳐 도입).
- **MCP**: 현재 없음. (멀티노드 서브노드 직접 SSH 제어 = 구현됨: `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh` = **브랜치-aware 하향 오케스트레이터**(rsync 배달 + 스크립트저작 `[sync]` 커밋 + fail-closed dirty 핸드셰이크 + 멱등 git-init) · `fetch_sub_docs.sh` = **상향 문서회수 미러** · 서브 빌드워커 CC `ssh sub bash -lc "claude -p"`. SKILL.md §4.5 / workflow.md S2.5·S3 · 양방향 싱크 D12절차.)
- 참고: 기존 `configs/check_reqs.py`(의존성 차이 분석 스크립트)를 결정론적 resolve 기반으로 재활용.

## 문서 발행 / 참조

- **3종 문서 역할**: `docs/plan/`(착수 전 계획·HITL) · `docs/devlog/`(작업 서사) · `docs/testlog/`(검증 증거·판정). 명명 = `docs/<type>/<type>_<YYYYMMDDHH>_<seq>_<주제>.md`. 역할·명명·구조·브랜치 통합모델 상세 = `.claude/rules/docs.md`.
- 전파 워크플로 규칙: `.claude/rules/workflow.md`.
- 부트스트랩 근거: 비추적 `seed/` 참조(GUIDE·question-catalog·REFERENCES·Seed) — 배포본엔 부재 가능.
