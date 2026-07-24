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
  근거: `docs/plan/plan_26062711`(첫 적용 = 모델구동 패치 + KV 이식성).

## 핵심 사실 (항상 보유)

- **레이어 커플링 규칙(가장 중요)**: 대상 vLLM 버전의 `pyproject.toml` `[build-system].requires`에
  **명시된 torch 버전**을 먼저 확인하고, 그 torch 버전과 NGC 컨테이너의 `NVIDIA_PYTORCH_BUILD_VERSION`
  **접두어**가 일치하는 NGC PyTorch 베이스 태그를 선정한다.
  (접미어 `+해시`·빌드메타는 무시. 예: vLLM 0.21.0 → torch 2.11.0 → `nvcr.io/nvidia/pytorch:26.03-py3`)
- **빌드트랙 따름정리**: torch 2.10대 → prebuilt wheel · torch 2.11+ → NGC alpha와 prebuilt `_C`의 C++ ABI 충돌(하드 ABI 벽) → **소스빌드 1차 트랙**. 트랙 판정 = 스킬 `upstream-version-watch` §0.5, 최종 중재 = 스모크.
- **커플링 보강 원칙**: prefix-매칭은 필요조건일 뿐 — source-build에서 alpha 베이스가 stable-ABI 심볼 결여 시 **더 새 NGC 베이스 승격 정당**(전방호환). 절차·키잉 상세 = `.claude/rules/workflow.md` S3 · 스킬 §4.6.
- **전방호환 시도-우선 따름정리 (plan_26070208)**: 호환성이 range/전방호환 성질인 도메인(CUDA minor·manylinux·torch prefix·미검증 (NGC×vLLM) 키·미검증 플랫폼 프리셋)에서 **버전/아치 불일치는 하드월 *증거* 없인 사전 기각 금지** — 실재 자산이면 시도하고 **스모크/classify_failure 가 중재**한다. 게이트: fail-loud 가드는 "미검증" 신호이지 "불가" 판정이 아니며, 우회는 HITL(예 render `--allow-unvalidated`·NCCL `generic` 프리셋·`--bw-floor` 재설정). 준거 = `resolve_wheel.py` 실자산-독해 · `failure_patterns.yaml` forward-compat "신호" · 사례 장부 = `.claude/rules/references.md` 호환성 노트.
- **이미지 네이밍 불변식**: `easy-vllm:{vllm}-cu{cuda}-{arch}-{track}`(예 `0.23.0-cu132-aarch64-source`). 모델-키잉 금지(과거 난립 원인) — 한 이미지가 모든 모델을 서빙. 태그 산정 = `render_dockerfile.py`.
- **아치-enablement 변종 트랙 따름정리 (도커 패치 범위 상한 = 포크 핀)**: stock vLLM이 **구조적 불가**(arch-wall)일 때 도커 패치 범위는 **deps-패치 → 소스-게이트 패치 → vLLM 소스-repo 오버라이드(포크 SHA 핀, `VLLM_REPO`/`VLLM_REF` build-arg) → 체크포인트-교체** 사다리로 확장한다 → `…-source-sm12x` 류 **superset 변종 이미지**(모델-키잉 ✗). 게이트: HITL 핀-오버라이드(무증거 ✗·testlog → `resolved.json` `source_build_variants`) · **단일 변종-트랙** · **승격 전 기존모델 회귀 재스모크**. 절차 정본 = `.claude/rules/workflow.md` S3 · 스킬 upstream §3.6(ii)·§4.6.
- **변종이미지 build-plane ≠ serve-plane 따름정리**: **이미지 정체성(IMAGE_TAG·`VLLM_REPO`/`VLLM_REF`)=클러스터-와이드 Band2 · 모델 serve config(CONFIG_FILE·트리플렛)=Band3**. ∴ 슬레이브 Band2-only 는 *serve-plane* 불변식이지 *build-plane* 아님(슬레이브는 트리플렛 없이 변종 이미지를 빌드·기동). 절차 정본 = workflow.md S2.5.
- **산출물 통로 불변식 (single/multi 혼재 차단)**: 빌드/렌더 산출물(+**manifest 실값**)은 **`output/<topology>/`(single|multi)** 에 둔다 — 통로 껍데기(`.gitkeep`)만 추적·생성물 비추적. **통로는 체크아웃 브랜치가 선택**(브랜치=통로 선택기일 뿐 · 토폴로지는 인터뷰 결정). 예외: multi 손작성 컨테이너 정의·build_patches 는 추적(정본). 근거 = `plan_26062312`·`plan_26062315` · 절차 = workflow S4.
- **serve-time env 통로 불변식 (결함#2 codify)**: serve 변수치환값(`NAS_MODEL_PATH`·`TIKTOKEN_HOST_PATH`) 해소 = **env-주입 > manifest 정본 > 리터럴 default**(compose 평면 — config.yaml 불참: config override 는 recipe 호스트파싱 평면 한정 = §manifest→서빙전략 배선). materialize 절차(`--materialize-env`) = 스킬 upstream §2.5 · workflow S2.
- **KV 절대클램프 따름정리 (이식성 — host≠target 확장, `plan_26070809_47_07`)**: 최종 recipe 의 KV 는 **측정된
  타겟-GPU당 `kv-cache-memory-bytes`(절대값)** 로 제어한다(gmu-derived KV=호스트 VRAM 차이로 비이식·OOM) —
  **단 `gpu-memory-utilization` 도 함께 emit**: gmu=startup free-memory 게이트+총-cap(통합메모리는 **gmu ≤ 0.90
  명시 필수**), clamp=KV 사이징·이식성. **host≠target 이식 시 예산 = 타겟 GPU VRAM × target_gmu**(host-측정
  불변량인 weights/overhead/per-token-KV 를 타겟 예산에 이전 — 근사 아니라 불변량 이전, 판정 반전 아님). 타겟
  스펙은 **config-time 의도**(`recipe-explorer` 의 `config.yaml` — manifest 와 직교, HW*사실*≠타겟*의도*).
  상세(cache.py 근거·free/total 수치·산정식) = 스킬 `vllm-recipe-explorer` §5 · `plan_26062711` Part 2 ·
  `plan_26070809_47_07`.
- **모델구동 런타임 패치 따름정리**: 구동 불가 모델의 런타임 호환 = **stock 이미지 + 런타임 패치**(`<model>_patch.py` — 휘발·비추적·환경/bump 마다 재유도·carry-forward ✗·메인 저작→하향 배달; 빌드타임 파생-이미지 ✗ = 이미지 네이밍 불변식). 결정론 arming(.pth 자동적용) = `arm_patch.sh`/serve_runner. 단 source-build 패치는 빌드타임이라 추적 `Dockerfile.source-build` 동결(평면별 지속성 비대칭). 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_26062711` Part 1·3.
- **per-model 3+1+1 아티팩트 따름정리**: 모델 구동 아티팩트 = **3(트리플렛 `<model>.{yaml,sh}`·`.env.<model>`) + 1(`<model>_patch.py`) + 1(빌드-바깥 패치 `output/<t>/build_patches/<NN>-*.sh`)** — 슬롯별 평면·소유·전달 비대칭: 3·+1(patch.py)=`vllm-recipe-explorer` 소유(런타임·휘발 — 트리플렛 Band3·patch.py 만 배달 특례) / +1(빌드-바깥)=`upstream-version-watch` 소유(빌드·동결 — native lib 은 몽키패치 불가·모델-키잉 ✗). **발견 ≠ 소유**(crosscheck 는 발견·분류·핸드오프만 — 이미지에 넣는 책임은 upstream). 상세 = 스킬 recipe §2①.5·§5 · upstream §4.7 · `plan_26062812`.
- **인코딩 자산 따름정리**: 런타임 인코딩 자산(tiktoken o200k/harmony)은 서빙 컨테이너가 런타임에 fetch 하지 않도록(read-only 마운트 불변식) 가중치와 함께 **사전적재**한다(managed=NAS·ephemeral/custom=지정경로). '에어갭 사전적재'는 이 **런타임 마운트 순수성**을 가리키는 명칭이지 환경 offline 이 아니다 — 상세 = 스킬 `vllm-recipe-explorer` §5.
- **near-max batch 측정 따름정리**: near-max batch·절대 KV 클램프는 **측정(serve KV log 또는 Phase-2)으로만** 산정한다 — per-token KV 공식은 과대추정 상한일 뿐(**측정 > 공식**, formula-우선 batch 금지). 숫자증거·상세 = 스킬 `vllm-recipe-explorer` §1·§5.
- **적대적 성능 검증 따름정리 (기능≠성능 · self-preference 차단)**: **기능 스모크만으로 done 선언 금지 — 성능은 `adversarial-benchmark` 적대 루브릭 게이트로 별도 검증**한다(3중 방어막 (a)루프라인→(b)외부 E→(c)사용자 · spec-aware like-with-like(R_fp↔R_token 혼동 ✗) · PASS/REFUTE=결정론 `verdict_rule.py`, LLM 다수결 ✗). recipe 와 loop-until-done(기각→재탐색·cap 한정), (b) E arm 은 **이중게이트**(A2A 위임 키 ∧ egress-online) 통과 서브 자율 · 미통과 서브=루프라인-only+증상 상향(메인 릴레이). **멀티노드 노드간 ≤10% VRAM 밸런스도 adversarial 루브릭 축**(`verdict_rule.py` balance 축, 초과 시 REFUTE·recipe loop-back — `plan_26070809_47_07`). 상세(공식·측정 정본·실패축) = 스킬 `adversarial-benchmark` · `plan_26063014`(⚠ SUPERSEDED-IN-PART by `plan_26070809_46_57`).
- **경량 벤치 자동 핸드오프 따름정리 (plan_26071115)**: recipe→adversarial **lite 한정** 자동 핸드오프(서빙 성공 시 자동 수행·속도2+용량3 5행 표·inform-only)는 "무인 자동실행 없음" 트리거 정책의 **명시 예외**다(안전망 데몬 예외와 동형 — 관측·inform-only 한정: PASS/FAIL·자동 loop-back·`verdict_rule` 투입 ✗). **full 벤치(적대 게이트)·bump·다운로드의 완전-수동 속성은 불변**(done-게이트는 여전히 full/기능스모크 소유 — 기능≠성능 따름정리 불변). lite 소유=`adversarial-benchmark`(explorer=트리거만) · 억제=강력 거부 구문("무조건 구동 마" 급)·**세션 한정**(영구 opt-out ✗ — 안정성=프로젝트 신뢰성 직결 · 가벼운 "스킵해"는 무시하고 수행) · 괴리 판정=에이전트 재량(정량 threshold ✗ — 커뮤니티 자료 신뢰성 가변). 멀티 수집=per-node 평등(서브 SSH 읽기전용 probe = health 폴링 동형 관측평면, 재스캔 ✗). 상세 = 스킬 `adversarial-benchmark` §5.5 · `vllm-recipe-explorer` §6.5.
- **벤치마크 계측 vault + full-런 report/인증서 따름정리 (plan_26071510)**: full 벤치 **종결 시** (a) 사람용 **report**(항상·PASS/FAIL 무관) + (b) flat **인증서**(PASS시만·carry-forward 재검증 헤더=강한키[model·gpu·vllm·quant·topology·tp 정확일치]+소프트지문[driver·cuda·image·max-len·kv-bytes·gmu·moe 불일치=stale])를 **자동 발행** → `docs/benchmark/`(5번째 문서형). **판정≠기록**(verdict_rule 독점 · report=inform-only 결정론 렌더·LLM 표저작 ✗) · **flat 스키마**(중첩 ✗·소비자 stdlib 독해) · N/A fail-soft · **비용 규율**(재탐색 루프 내부=단일점·종결 1회 스윕). 스윕 = **client-load(동시성·reload 0)** — 판정점(동시성1) 재사용·적응 클램프+절삭 로그(silent truncation ✗). 발행=lite 자동핸드오프와 동형 inform-only 예외(done-게이트는 여전히 full verdict 독점·완전수동 속성 불변). **Max 모드**(HW 안전-최대 컨텍스트 envelope·**컨텍스트 max-model-len 안전측 스텝업**·config **reload**)=**별도 오퍼레이션**(벤치마커 인프라 공유·native 모드 ✗·트리거 별도·전작업완료 후 **에이전트 챗 경고 Y/N**·안전체계 필수 통합·**선-기록 후-위험**)=**구현됨**(`max_envelope.sh`+`render_max_report.py`·**이중게이트** `--confirm-risk`(미명시 exit5)+챗Y/N·라이브E2E 후속·드라이버 580.159.03 재검증 vehicle). 상세=스킬 `adversarial-benchmark` §5.6·§8.5 · Root=`seed/letter_2026071516_1`.
- **모델별 서빙전략 독립 따름정리 (carry-forward 금지 · 가장 중요)**: 한 모델의 서빙전략(MoE 백엔드·플래그·이미지 트랙)은 **이전 모델의 전략과 *아예* 달라질 수 있다 — carry-forward 절대 금지**. 정답은 **모델×하드웨어마다 reference-grounded(oracle/소스 직독)로 재확정**한다. 실증: Qwen3.5-122B NVFP4 → `flashinfer_cutlass`(triton 금지) ↔ DeepSeek-V4 MXFP4 → **HUMMING(명시 `--moe-backend humming`)** — *동일* `--moe-backend auto`가 122B선 정답이지만 DS4 sm_121선 MARLIN-repack→통합메모리 OOM(호스트 하드다운). ⟹ **arch-walled 환경에서 `auto` 신뢰 금지**(walled fallback=repack-OOM): 비-repack 경로를 oracle 직독으로 명시 선택. **옛 단일-모델 교훈(예 "triton 강제 금지(122B 교훈)")은 *그 모델·양자화 맥락 한정*이지 전역 금지가 아니다** — 맥락이 분기하면 둘 다 참. 상세 = 스킬 `vllm-recipe-explorer` §5 · `plan_26062818`(Route B) · `testlog_26062823`.
- **호스트 안전체계 따름정리 (plan_26071019)**: 통합메모리 노드의 호스트 하드다운은 **계층 방어**로 차단한다 — ① mem_watchdog **systemd 상시(광역 @vllm)** + 하네스 협역 인스턴스(run_trial·multinode_smoke 자동 기동) ② 컨테이너 `oom_score_adj` 800(커널 OOM킬러 진범 타게팅 — #5 오발 교정) ③ earlyoom 최후선 ④ **로드-전 RAM 게이트**(체크포인트 index `total_size` ÷ TP + floor vs MemAvailable → drop-caches 1회 → 거부; du 금지 — `testlog_26070814`). 설치 = terraforming **세션 최종 선택조항**(Flag 발급 *후* Y/N · 보험판매 톤 · 강제·차단 ✗ — plan_26071115; **HITL sudo** `install_host_safety.sh`·서브 동일). **Flag 와 독립**(안전체계 미설치여도 Flag valid) · opt-out = manifest `host_safety.installed:false`(중립 기록) + **통합메모리 노드만** 서빙 시 에이전트 채팅 1줄 경고(serve 스크립트/로그 배너 코드변경 ✗ — 시끄러운 경험 방지) · **하네스 협역 워치독은 설치와 무관하게 작동**(opt-out 노드도 trial 중 보호 · 로드-전 RAM 게이트 drop 헬퍼만 부재 시 graceful skip·음성정직). **안전망 데몬(관측+보호킬)은 "무인 자동실행 없음" 원칙의 명시 예외**(bump 완전-수동 속성은 보존). 페이지캐시 드랍 sudo 위임 = 고정 헬퍼 `vllm-drop-caches` 단일 경로만. 워치독 정지 = PID 기반만(**pkill -f 금지** — 자기참조 exit144). 절차 정본 = workflow.md S3 · 스킬 `terraforming_node` §2.6(세션 최종 Y/N 선택조항) · `plan_26071019`·`plan_26071115`.
- **manifest→서빙전략 배선 불변식 (plan_26063018)**: HW사실·획득모드는 **manifest 단일계약**에서 소비한다(브랜치⇒추론 금지의 TP 축 연장). **TP = `config.tensor_parallel_size(명시 override) > manifest(nodes 있으면 len(nodes)×gpus_per_node · nodes 비면 topology=single→1) > 최종폴백 1`**(`roofline.py:196-209` 기검증 패턴 *그대로* — **git 브랜치 폴백 없음**: 브랜치⇒TP 추론이 보고된 버그였음; 최종폴백 1=안전·과대구독 ✗) · **NAS 경로 = manifest.nas_model_path(단일 권위); recipe 호스트파싱 = `config.yaml > env > manifest > /mnt/models` · serve-time(compose)·check_smoke = `env > manifest > /mnt/models`(config.yaml 은 recipe 평면 한정·compose 미도달 — §serve-time env 통로 불변식)** · **가드(결정론 후보탈락)**: `tp > 가용 GPU 합` · `num_key_value_heads % tp != 0`. 실증 결함: `recipe.py` 가 git 브랜치로만 TP 결정 → 1-GPU 머신에 TP=2/8 제안(같은 프로젝트 `roofline.py`·`check_smoke_model.py` 는 이미 manifest 배선 — `recipe.py` 만 laggard). 상세 = 스킬 `vllm-recipe-explorer` · `plan_26063018`.
- **빌드 입력**: `CPU_ARCH=$(uname -m)` · `CUDA_VERSION`(예 129) · GitHub Releases pre-built wheel.
  wheel은 `pip install --no-deps`로 설치하고, **그 전에 `/etc/pip/constraint.txt`를 비운다**(NGC 핀 충돌 회피). 상세 = 스킬 `upstream-version-watch`.
- **주변 의존성 원천**: vLLM `requirements/{common,cuda,build}.txt` + `pyproject.toml` → `requirements.txt` 재생성(절차 = `.claude/rules/workflow.md` S1).
- **모델 획득 모드 따름정리 3종 (managed|ephemeral|custom · *모델 weights 획득* 한정)**: manifest `model_source` 가 다운로드 정책을 정한다 — **managed**(관리 NAS read-only·무단 HF ✗) · **ephemeral**(컨테이너 임시·down→삭제 — **다수 기본**) · **custom**(사용자 지정 경로 영속·볼륨마운트). 게이트: 모델 부재 시 **per-event 사용자 승인**으로만 모드 기본위치에 다운로드('사전승인'=모드 허가일 뿐 무인 포괄허가 ✗). **단 *서빙전략 수립*의 외부 교차검증(HF 카드·vLLM GitHub)은 획득모드와 무관하게 항상 의무**(**모델획득 격리 한정** — 외부접속 전반 차단 아님). 절차 정본 = workflow.md §모델/안전 · `plan_26063018`.
- **자기개선 루프 사서(wiki-desk) 따름정리**: docs 작업이력은 `__llm-wiki`(비추적·메인 단독) 도서관으로 관리 — 사서가 **결정론 관계그래프 기반 authority-ranked 정제맥락**을 발현한다(실행진실>계획의도 · ROOT 헌법 비인덱싱=반-확증편향 · 원본 비복사 · 음성정직 · 결정론 엣지만 — supersede 는 문서 헤더 소급 배너의 literal grep 으로 색인, docs.md §3). **발동 시점**: 인터뷰 착수·plan 작성·서빙전략 수립·bump·토폴로지 변경 + 새 doc 발행 시 증분 입고. **Flag 게이트 밖·항상 가용**. 상세 = 스킬 `wiki-desk` SKILL.md · `plan_26062809`.

## 스킬 오케스트레이션 / 진입 척추 (파이프라인 순서 · fresh-clone 능동발동 · escalation)

> 근거: `docs/plan/plan_26063009_44_23`(A부 — 진입/순서) · `plan_26063009_19_14`(B부 — escalation 역루프, codify 완료). 그라운딩 = `seed/session_record_2026063008`(첫 테라포밍 배포본에서 밟은 진입 갭 2건).

- **파이프라인 의존순서**: `terraforming_node` → `upstream-version-watch` → `vllm-recipe-explorer` → `adversarial-benchmark`. "우선순위"=**의존순서**(앞이 뒤의 전제)이지 충돌 승자 아님. 발동지점: Flag 발급 후 빌드 질문=upstream(**제안만** — 빌드/bump 실행은 사람 지시) · 서빙전략 지시=recipe · 성능 의심=adversarial(recipe 와 loop-until-done). 상세 = `plan_26063009_44_23` · `plan_26063014`.
- **테라포밍-완수 Flag 게이트 따름정리 (init/runtime 2-모드)**: `terraforming_node` 가 first — **완수 시에만** manifest 에 Flag attestation 기입(보수적·fail-closed-on-flag). **Flag 부재 시 3 런타임 스킬은 info-only**(정보·조언·HF조회 OK / 환경특정 deliverable 생성 ✗ — HW사실 없이 그럴듯한 답 날조 금지) + redirect 로 terraforming 유도. 강제 2층 = 결정론(`recipe.py`·`run_bench.sh` 진입 게이트 비0종료) + 페르소나(말로 새는 generic 명령 봉쇄). wiki-desk 는 게이트 밖. 절차 정본 = workflow.md §init/runtime · 스킬 terraforming §0.5 · `plan_26063018`.
- **A2A-위임 Flag 따름정리 (서브 양성 키 · fail-closed)**: 서브(A2A 피어)는 **메인이 HW 동질성 단언(`cpu_arch`·`gpu_model`·`gpus_per_node` 정확일치 — 동질 GPU only) 통과 후 발급한 양성 위임 키** 없이는 info-only("부재=면제" fail-open ✗ · 서브 자가스캔 ✗ · `terraforming_node` 영구 main-only). **enforcement 2지점**: multi=`sync_to_sub` 전파 게이트(키 없으면 거부+HITL) / single-sub-control=`recipe.py`·`run_bench.sh` serve 게이트(동형 fail-closed). **UNIQUE 키 불변식**: 메인 Flag(`terraforming.complete`@manifest) ≠ 서브 키 — 평면 교차 통과 ✗. 상세(키 거주지·발급) = `scan_node.py`·`render_sub_env`·`sync_to_sub.sh` · `plan_26063021_14_37`.
- **fresh-clone 능동발동**: 미테라포밍 신호(`config.yaml` 부재 ∧ `output/single/manifest.yaml` 부재 ∧ `output/multi/manifest.yaml` 부재 — 또는 Flag attestation 부재) 감지 시, "이제 뭐해야해" 류 **및 구체 서빙요청("X 모델 띄워줘/실행법 찾아")** 모두에 **온보딩(`terraforming_node`)을 능동 제안**한다(구체 서빙요청은 info-only redirect 로 유도 — §테라포밍-완수 Flag 게이트). 능동성 고도 = **제안**(감지→제안→인터뷰→승인→스캔). "완전 수동" 트리거 정책은 *vLLM bump* 한정이지 온보딩이 아니며, "무단 스캔 금지"는 스캔이 인터뷰+승인 뒤이므로 보존(지적1 해소).
- **토폴로지는 인터뷰로 결정**(브랜치⇒토폴로지 추론 ✗): `terraforming_node` 첫 동작 = 토폴로지(single/multi) 인터뷰. 브랜치는 작업공간 선택기일 뿐. 미선언 시 emit fail-closed(scan `emit_gate`) · 브랜치≠토폴로지 시 HITL 브랜치전환. 상세 = 스킬 `terraforming_node` §0.5(지적2 해소).
- **escalation 역루프 따름정리 (recipe→upstream · 발견≠소유)**: 새 모델이 현 vLLM 으로 *구조적* 불가일 때 recipe 가 **구동불가 증상 ∧ 외부 확증 둘 다**(단일 신호 ✗)로 발견 → **사용자 명시 승인** → upstream 이 버전핀 소유·처방(**3출구**: (i) 공식 bump / (ii) 포크핀 변종 트랙 / (iii) 음성정직 — references.md §5 최소범위 레시피 수행 후에만) → rebuild 후 recipe 재개. 게이트: 드문 예외 경로(대다수 모델은 그냥 뜸·매 요청 외부리서치 ✗) · 무승인 자동 escalate ✗ · 순환=reconciliation_cap · 최종 중재=스모크 · **egress-restricted 서브=증상 상향만 · egress-online+위임 서브=발견(리서치) 자율**(단 bump 실행=메인+승인). 절차 정본 = workflow.md §escalation · 스킬 recipe §5.5 ↔ upstream §3.6 · `plan_26063009_19_14`.

## 버전 핀 / 트리거 정책

- **완전 수동**: 사람이 신규 vLLM을 감지(모델 구동 실패 또는 GitHub 확인) → "업데이트" 지시 → 에이전트 실행.
  자동 폴링·webhook·cron 없음. patch/minor/major 무관하게 항상 사람 지시로 시작.
  - **예외(escalation 역루프 — 감지 주체만 에이전트, 실행은 불변)**: escalation 역루프에선 *발견(감지)*이 `vllm-recipe-explorer`(에이전트) 측일 수 있다(서빙전략 수립 중 구동불가 증상 + 외부 교차검증). **단 upstream 실행(bump/빌드)은 여전히 명시 승인 게이트** — 무인 자동 bump ✗(이 "완전 수동"의 핵심속성=무인 자동실행 없음은 보존). 상세 = §스킬 오케스트레이션 척추 §escalation 역루프 따름정리.
- **config.yaml(영속)** 이 대상 버전 · 단일/멀티노드 스모크 모델명을 지정(스킬에서 사용, Step 2). **NAS 경로·HW사실의 단일 권위는 manifest**(§포인터 원칙·§manifest→서빙전략 배선 불변식) — config.yaml 의 NAS 값은 *override-only*(`config > env > manifest > default` — recipe 호스트파싱 평면 한정 · serve-time/compose 는 config 불참 = §serve-time env 통로 불변식; 이로써 §config.yaml↔§포인터 원칙 모순 해소).

## 배포 / 환경 (manifest)

- **이식 모델**: 배포 단위 = 스켈레톤 + 생성엔진(완성품 아님). 누구든 클론 후 자기 환경을 테라포밍해 쓴다.
- **포인터 원칙**: 환경 구체값(노드 IP·호스트명·인터커넥트·NAS 경로·origin)은 **헌법에 두지 않고** `output/<topology>/manifest.yaml`(브랜치 파생 통로)에서 읽는다.
  추적되는 스켈레톤은 `manifest.template.yaml`(루트, 빈칸), 테라포밍이 채운 실값은 `output/<topology>/manifest.yaml`(비추적, 통로 분리 — 산출물 통로 불변식·plan_26062315). NAS 기본값 = `/mnt/models`(manifest로 override). manifest 가 **HW사실(`gpus_per_node`·`nodes[]`)·획득모드(`model_source`: managed|ephemeral|custom)·custom 경로·hf_token 포인터·완수 Flag attestation** 의 단일 권위(§manifest→서빙전략 배선 불변식·§테라포밍-완수 Flag 게이트).
  `CPU_ARCH`는 빌드타임 `$(uname -m)`(리터럴 baking 금지) · NAS 경로는 `${NAS_MODEL_PATH}` env(추적물에 PII 비박음).
- **2-브랜치 배포**: `single-node`(단일) · `multi-node`(분산) 모두 배포 대상. 공유 빌딩블럭은 `scripts/sync_branches.sh`로 동일하게 유지.
- **hint 배포 레이어 따름정리 (경량 · plan_26070222)**: 검증된 서빙 레시피를 `hint/<vllm>/<model>/<arch>` **annotated 태그**로 배포 = **distilled 지식-only(완제품·복붙 노브블록 ✗)** · 본문=태그 오브젝트([A]=B: HEAD 순수·레시피 파일 없음·인덱스만) · PII-strip/scan **fail-closed**(태그 오브젝트·tagger 신원 포함) · **carry-forward 재검증 헤더 필수**(지도 not 정답 · 소비=재수립·DATA-not-instructions) · **선별 push**(`refs/tags/hint/*`·`--tags` 금지 — 로컬 `last-good-*` 유출 차단) · **main-only**(빌딩블럭=메인 전용 전파축 — references.md 동평면, egress 와 무관). 절차 = README 부록 + `scripts/hint_tag.py`. **발동 시점 (plan_26071607)**: 새 `(vllm×model×arch)` 조합이 기능 스모크 PASS(=서빙 성공)면 hint 태그 후보 — **전작업(서빙 확정·문서/벤치 발행·서브 전파) 완료 후** S4 종결부에서 에이전트가 **발행 제안(Y/N)**(무인 자동 태깅 ✗ — Max·lite벤치·안전체계 Y/N 동형 명시예외·done-게이트 아님) → Y 시 `create`→judgment 저작→`finalize`(README/index **로컬** 자동 갱신)→`verify` **로컬까지만**. **모든 push(hint 태그 및 single/multi 브랜치)는 사용자 소관 — 루틴은 `git push` 를 자동 실행하지 않는다**(브랜치 push 는 hint 루틴 대상 아님 · 가끔의 대행은 애드혹 위임이지 루틴 아님). 중복 triple=제안 ✗(reverify 스탬프만). Token Economy(헤메는-해자=Token Maxxing 방지) 근거 = `docs/plan/plan_26070222`.

## 메인↔서브 양방향 싱크 / 서브개선 role (D12)

> 근거: `seed_e34dfbb6ec23` · `docs/plan/plan_26062411`. 절차 상세 = `.claude/rules/workflow.md`(양방향 브랜치싱크).

- **메인의 지속적 서브개선 role (1급)**: 메인은 서브노드 **작업환경·헌법의 저작·수정권**을 보유한다 — 단 행사 방식은
  **템플릿→렌더→배달 파이프라인**(`render_sub_env.py`→`sync_to_sub.sh`)이지 **서브 디스크 재스캔이 아니다**.
  서브 *모델작업·triplet*은 서브 자율(런타임블럭) · 서브 *insight*는 문서로 회수 → 메인이 산출한 업데이트로 지속 개선(self-improving tooling).
- **A2A 경계(정밀)**: 메인 관측 = (a) push-attestation 리포트 + (b) 서브 `docs/` 로컬 미러 **열람**(`fetch_sub_docs.sh`).
  메인은 서브 **작업코드/설정을 재스캔·직접교정하지 않는다**. env/헌법 수정은 위 하향 파이프라인으로만(저작권위 ↔ 재스캔금지 공존).
  **메인 = 턴제 메시지브로커**(사용자↔메인↔서브, 실시간 X — 릴레이 HITL 은 `node_id`+출처 라벨로 메인/서브 혼동 없이 표면화). 상세 = `plan_26070809_46_57`.
- **서브 능력 = manifest 사실 따름정리**: 서브 네트워크/모델정책은 상수가 아니라 terraforming egress 스캔이 검증한
  `output/<topology>/manifest.yaml` attestation 이 단일 권위(포인터 원칙의 서브-축 연장). 상세 = `plan_26070809_46_57`.
- **서브 git = 로컬 전용**: 서브 워크스페이스는 `git init` 된 로컬 레포(`single`·`multi` 두 브랜치). **origin 영구 미설정**(push/pull/fetch/remote/clone deny — 방어심층, 진짜 구속은 "원격 없음" + 페르소나). git 역할 = 브랜치전환(모델로드 전략 분기) + 로컬 history/롤백 — **회수 vehicle 아님**.
- **하향(메인→서브)**: 브랜치별 rsync 배달 + 스크립트저작 `[sync]` 커밋(main-canonical, 겹침=sub-yields), dirty 트리=**fail-closed**(auto-stash 금지). 절차 = workflow.md B1.
- **상향(서브→메인) = 문서기반 only**: 서브 insight 를 `docs/` 규약으로 발행 → A2A 리포트로 경로 전달 → 메인 `fetch_sub_docs.sh` 미러 열람 → **HITL 재저작**(메인 템플릿/헌법/스킬). patch/추출층 없음. 절차 = workflow.md B2.
- **패치 전파(D12 연장)**: 모델구동 런타임 패치(`<model>_patch.py`)는 **메인 저작 → 하향 배달**(`sync_to_sub.sh` set; 슬레이브가 받는 *최초 model-keyed 파일*). 서브는 **패치 코드 저작 ✗** — "패치 필요" 탐지를 docs insight 로 상향 보고만(상향 코드/패치 추출층 없음 — D12-06·13). arming 관용구도 메인 배달분(서브 즉흥 저작 ✗).
- **모델 트리플렛 전파 불변식 (D12-연장)**: 모델 트리플렛(`<model>.{yaml,sh}`·`.env.<model>`)은 **메인→서브 직접 전파 절대 금지**(Band3 구조적 배제 — `<model>_patch.py` 만 특례). **멀티 TP 슬레이브 = Band2-only**(`.env.cluster`+`.env.interconnect` 만으로 기동 — 슬레이브가 `.env.<model>` 의존하면 미완결 신호). 절차 정본 = workflow S2.5 · `plan_26062811_30_33`.
- **PII 격리**: 회수가 문서기반(코드/설정 미추출)이라 서브 `CLAUDE.md`의 bake 정체성(PII)이 **메인 추적물로 유입되지 않는다** — `포인터 원칙`의 연장. 서브 헌법은 서브에 잔류.
- **single-node 확장기능**: single-node=기본 독립운용. sub-control("서브 제어 + 수행피드백 수신")은 single-node가 획득하는 **'확장기능'**(헌법 기재). **활성 게이트=결정론**: `output/single/manifest.yaml` `nodes[]`에 sub 존재 여부(`sync_to_sub.sh` 가 읽어 판정 — 현재값은 헌법이 아니라 그 manifest 가 단일 권위·포인터 원칙). (단일 manifest 기입은 이제 `terraforming_node` §1S 단일 온보딩이 담당 — plan_26063009_44_23. 단 single-node 가 *sub-control* 확장기능을 HW탐지 전달로 활성화하는 메커니즘은 여전히 **미구현·파킹** — plan_26062411 §5.)
  - **라이브 형태 = A2A 모델서빙 위임(T3 검증, 0.23.0 E2E)**: 활성 시 메인이 서브에 A2A 태스크 발급(`ssh sub claude -p … --permission-mode acceptEdits`) → 서브가 자작 recipe + `--profile serve up -d` + 로컬 스모크 → push-attestation 1개 반환. 메인은 **리포트만 관측**(디스크 재스캔 X — A2A 경계). 실행평면 노드별 독립(교차검증). 절차 = 서브 `comms.md` serve 술어(single 분기).

## build / 검증 커맨드

- 산출물 통로: 빌드/서빙 산출물은 `output/<topology>/`(single|multi)에 위치 — 단일/멀티 혼재 차단. 통로 껍데기만 추적·생성물 비추적(plan_26062312).
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

- **로그=에이전트 / 자기개선 루프 두 기둥 (plan_26063018)**: work → 로그/문서 발행 → 기록 잔존("로그가 곧 에이전트다"). 자기개선 루프의 **자동 기둥** = `wiki-desk`+`__llm-wiki`(자동 색인) · **수동 기둥** = 헌법(로그 본 사람의 *출력*). plan 발행이 수동 기둥의 핵심 습관 — 배포자가 Agent 를 능숙히 다루는 길.
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
- **참조-그라운디드 해결**: 오류복구·진단 시 자기추론보다 **권위 참조**(업스트림 소스·이미지 내부·모델 config/chat_template·런타임 로그·레지스트리/헤더 · **서빙전략 수립 시 HF 모델카드·vLLM GitHub issue/release** — §모델 획득 모드 따름정리로 1급 편입) 우선 — 위 결정론 해소의 error-recovery 연장. **외부 레퍼런스 1차 진입점 = `.claude/rules/references.md`**(ID→URL 템플릿·검색 레시피 — "불가/미지원" 부정 결론은 §5 최소범위 레시피 수행+testlog 탐색증거 기록 후에만, 메인 한정). 상세는 각 스킬 error-recovery. (정확도 위해 토큰 증가 허용)
- 업스트림 핀/베이스 이미지를 가드레일·기록 없이 임의 변경 금지.
- 요청 범위 밖 기능·추상화 선반영 금지(Karpathy B2·B3).

## 롤백

- **last-good 앵커 = 로컬 스모크-통과 커밋**. 미커밋 작업분은 일회용 · `single-node`·`multi-node` 독립 롤백.
- 절차(`git reset --hard <last-good>`·`git tag last-good-<branch>`·origin = `manifest.origin_url`) 상세 = `.claude/rules/workflow.md` §실패/롤백.

## 스킬 / 도구 경계

- **커스텀(5-스킬 계층)**: `terraforming_node`(토폴로지-중립 온보딩 — 토폴로지 인터뷰·스캔·manifest 생성·서브 렌더 배달) · `upstream-version-watch`(버전해소·render·소스빌드·빌드/스모크) · `vllm-recipe-explorer`(모델 서빙전략 3종 세트·VRAM/KV trial) · `adversarial-benchmark`(서빙 성능 적대 게이트) · `wiki-desk`(docs 도서관 사서). **두 직교 축**: ① *서브 전달* 축 — 빌딩블럭(terraforming·upstream·wiki-desk)=메인 전용 / 런타임블럭(recipe·adversarial)=서브 복제(외부검색 E arm = 이중게이트 조건부 서브 자율 · 미통과 시 메인 릴레이) · ② *작업 허가* 축 — Flag-게이트(메인=`terraforming.complete` / 서브=A2A 위임 키·UNIQUE) / wiki-desk 는 게이트 밖. 각 스킬 상세 = 해당 SKILL.md frontmatter.
- **외부**: Docker 작성/문법검사 보조 — 후보 `netresearch/docker-development-skill` (설치정책 거쳐 도입).
- **MCP**: 현재 없음. (멀티노드 서브노드 직접 SSH 제어 = 구현됨: `.claude/skills/upstream-version-watch/scripts/sync_to_sub.sh` = **브랜치-aware 하향 오케스트레이터**(rsync 배달 + 스크립트저작 `[sync]` 커밋 + fail-closed dirty 핸드셰이크 + 멱등 git-init) · `fetch_sub_docs.sh` = **상향 문서회수 미러** · 서브 빌드워커 CC `ssh sub bash -lc "claude -p"`. SKILL.md §4.5 / workflow.md S2.5·S3 · 양방향 싱크 D12절차.)
- 참고: 의존성 재생성 엔진 = `.claude/skills/upstream-version-watch/scripts/regen_requirements.py`(wheel METADATA 권위 — 구 check_reqs.py 후계).

## 문서 발행 / 참조

- **3+2+1종 문서 역할**: `docs/plan/`(착수 전 계획·HITL) · `docs/devlog/`(작업 서사) · `docs/testlog/`(검증 증거·판정) + `docs/simlog/`(run 디렉토리 = 기계생성 원시증거 vault) + `docs/benchmark/`(full-런 계측 vault = 사람용 report[항상]+기계용 인증서[PASS시]·inform-only) + **`docs/report/`**(배포자 대상 아웃바운드 공지 — 앞 5종과 직교: 사람이 자유 발행·트리거 ✗·wiki 색인 ✗·메인 전용 · *유일하게 산출물째 git-추적*(배포돼야 도달) · 단일파일 반응형 HTML 권장 · PII 금지). 명명 = `docs/<type>/<type>_<YYMMDDHH>[_MM_SS]_<주제>.md`(**2자리 연도** · 동일 YYMMDDHH 충돌 시에만 `_MM_SS` · simlog 는 run 폴더 · benchmark 는 `bench_report_/benchmark_/max_envelope_<YYMMDDHH>_<model>_<gpu>_<vllm>` 평면파일 · report 는 `<주제-슬러그>.html` 날짜 ✗). 역할·명명·구조·브랜치 통합모델 상세 = `.claude/rules/docs.md`.
- 전파 워크플로 규칙: `.claude/rules/workflow.md`.
- 부트스트랩 근거: 비추적 `seed/`(카파시 규칙·워크플로 패턴·session_record 등 — 배포본엔 부재 가능) + 추적 `docs/plan/` 체인(wiki-desk 색인이 안정 앵커).

<!-- BEGIN HERMES-CLAUDE-CONTROL OPTIONAL -->
## Optional Hermes-Claude Control Integration (선택 · 메인테이너 로컬 전용 · control-plane 한정)

> 배포자 참고(먼저 읽기): 이 절은 메인테이너 개인의 **선택적** 제어면(claude-code-control)에 대한 안내다.
> 관련 아티팩트(`agent-card.json`·`.hermes-claude-control/`·`.claude/rules/hermes-claude-control.md`·
> `.claude/schemas/hermes-control-*`·`.claude/templates/hermes-control-*`)는 **메인테이너 로컬에만** 존재하며
> **이 배포본에는 없다**(git 비추적). **당신의 체크아웃에 claude-code-control 스킬이나 이 아티팩트가 없으면
> 이 절 전체를 무시하라** — 제품 동작·헌법·`.claude/skills/*` 의미에 아무 영향이 없다.

이 프로젝트는 선택적으로 `.hermes-claude-control/` 기반 메인 Hermes Agent ↔ Claude Code 제어 contract를 가질 수 있다. 이 통합은 `agent-card.json` capability가 **명시 호출될 때만** 적용되며, 기존 제품 작업 헌법과 `.claude/skills/*` 의미를 대체하지 않는다. run-history·runtime 상태는 실행 이력이므로 git 추적하지 않는다. 경계 정책 상세 = `docs/report/claude-control-main-only-local-overlay-policy.md`.
<!-- END HERMES-CLAUDE-CONTROL OPTIONAL -->
