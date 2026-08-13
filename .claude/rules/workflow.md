# workflow.md — 런타임 전파 계약

> 목표: 사람의 업데이트/서빙 지시를 검증 가능한 상태 전이로 바꾸고, 실패를 정확한 owner로 돌린다.
> 항상 참인 경계는 `CLAUDE.md`; 세부 실행은 아래 owner 스킬·스크립트가 소유한다.

## 상태·owner 계약

| 상태 | owner | 입력 | 출력 | 통과 게이트 | 실패 라우팅 |
|---|---|---|---|---|---|
| init | `terraforming_node` | 인터뷰 답·노드 사실 | manifest·완수 Flag | plan HITL→스캔→attestation | 스킬 §0.5·`manifest_contract.py` |
| S1 resolve | `upstream-version-watch` | 목표 vLLM·manifest | torch/NGC/CUDA/build-track/deps 해소값 | HITL ① | 스킬 `references/resolve-and-render.md` |
| S2 patch | `upstream-version-watch` | S1 해소값·두 topology | 렌더된 이미지/serve 입력 | 양 브랜치 diff HITL ② | 스킬 §2·renderer |
| S2.5 sync | `upstream-version-watch` | multi manifest·렌더 산출물 | 검증된 메인→서브 배달 | dry-run→apply·checksum | `sync_to_sub.sh` |
| S3 smoke | upstream + recipe | 렌더 산출물·모델·HW | 기능 스모크·분류·risk memo | 아래 HITL ③ | `classify_failure.py`·owner reference |
| S4 commit | 사람 + upstream | S3 PASS 증거 | 로컬 last-good·문서·선택적 전파 | 최종 HITL ④ | `policy:LAST_GOOD_ROLLBACK_ANCHOR` |
| benchmark | `adversarial-benchmark` | 성공 recipe·manifest | lite 관측 또는 full report/certificate | full 수동 게이트 | 해당 skill references/scripts |
| docs | `wiki-desk` + publisher | plan/raw/narrative | 규약 문서·evidence record | `.claude/rules/docs.md` | publisher stable rejection |

스킬 의존순서는 `terraforming_node` → `upstream-version-watch` → `vllm-recipe-explorer` → `adversarial-benchmark`; `wiki-desk`는 discovery/failure/publication sidecar이며 상태를 전이하지 않는다.

## 공통 진입 게이트

- S1–S4·escalation·B0–B3 전 tracked terraform validator를 실행한다. 서브 진입은 `policy:A2A_DELEGATION_KEY_FAIL_CLOSED`도 검증한다. topology는 인터뷰/manifest에서만 읽고 브랜치로 추론하지 않는다.
- Flag가 없으면 작업을 멈추고 다음 문구로 onboarding을 제안한다: *"HW스캔이 덜 되어(Flag 미발행) HW 스펙(GPU·OS)을 알기 어려워 모델 `<HF URL>` 의 정확한 서빙전략을 세우기 어렵습니다. `terraforming_node` 로 ① HW스캔 + ② 모델 다운로드 전략(관리 NAS 경로? 컨테이너 임시 다운로드(컨테이너 down 시 삭제)? 특정 경로 저장·마운트?)을 먼저 정합시다."*
- fresh clone 감지는 제안만 한다. 인터뷰→승인 뒤에만 스캔한다. 정본은 `policy:TERRAFORM_FLAG_GATE`와 terraforming 스킬 §0.5다.

## 필수 전이 spine

```text
S1 resolve  → 결정론 스크립트로 vLLM/torch/NGC/CUDA/wheel-or-source/deps 해소
   verify   → 해소값 출력
   HITL 게이트 ① → 사람 확인
S2 patch    → single·multi를 같은 해소값에서 렌더; serve-time env는 manifest보다 우선
   verify   → 변경이 S1에 직결되고 topology 산출물 통로가 분리됨
   HITL 게이트 ② → 각 브랜치 diff 확인
S2.5 sync   → multi만 dry-run 후 선택적 tracked runtime·위임키·산출물 배달
S3 smoke    → 모델/NAS→RAM/host preflight→build/serve→health/inference
   verify   → 요구 topology 기능 스모크 PASS
   실패     → classify_failure.py reason에 따라 owner route
   HITL 게이트 ③ : 스모크 결과(+분류·risk-memo)를 사람이 확인 (smoke-before-commit)
S4 commit   → 스모크 통과분만 로컬 last-good 커밋
   verify   → docs/devlog·testlog에 버전·변경·스모크·anchor 기록
   HITL 게이트 ④ → 최종 커밋/전파 승인
```

- S2 runtime patch는 `validate_runtime_patch.py`→materialize/arm→S3 순서이며 `policy:RUNTIME_PATCH_NO_CARRY_FORWARD`; 모델 triplet은 `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION`; variant image/serve 입력은 `policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE`을 따른다.
- S3 모델 부재는 `policy:MODEL_ACQUISITION_TERNARY_GATE`; 모든 load/build/cleanup은 `policy:HOST_SAFETY_LAYERED_DEFENSE`; KV 산출은 `policy:KV_ABSOLUTE_CLAMP_PORTABILITY`. 실제 명령·health 판정·cleanup은 각 skill script가 소유한다.
- S4 뒤 공유 빌딩블럭 동기화는 사람 질의로만 `.claude/skills/upstream-version-watch/scripts/sync_branches.sh`; hint 후보는 `policy:HINT_TAG_ACTIVATION_GATE`와 `.claude/skills/upstream-version-watch/scripts/hint_tag.py`의 match→create→judgment→finalize→verify 순서만 허용한다.

## 실패 라우팅

| 신호 | owner/경로 | gate·출구 |
|---|---|---|
| requirements-fixable | upstream failure recovery | 조정→재빌드→S3, `reconciliation_cap` 소진 시 Model-C |
| source-build-class | upstream `references/source-build.md` | ABI 증거→HITL→동결→clean build→S3 |
| NGC base mismatch | `resolve_ngc_tag.py` + source-build owner | 후보 헤더/로그 증거→HITL→해소값 override→S3 |
| stock-구조적-불가 | upstream arch variant owner | 아래 사다리·증거·승인 후 S3 |
| multi OOM/NCCL/Ray | upstream multi reference | 증거 보존→Model-C |
| unknown | 사람 | `{proposed_class,evidence}`만 제시; 승인 전 무행동 |
| recipe 중 구조적 불가 발견 | recipe §5.5→upstream §3.6 | 공식 bump / 포크 SHA pin / 음성정직; cap 뒤 Model-C |

arch-wall은 단계를 건너뛰지 않는다: deps-패치 → 소스-게이트 패치 → **자체 이식** → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체. `stock-구조적-불가` 뒤 **참조-그라운디드 확증** → **testlog 기록 + 사람 승인** → canonical ledger `source_build_variants` → **Dockerfile build-arg 파라미터화** → **clean 빌드(양노드) → 스모크** 순서다. 정본은 `policy:ARCH_WALL_VARIANT_LADDER`, upstream skill §4.6과 `references/source-build.md`다.

### 변종 좌표의 거처 (2026-08-13 신설 · 신규 변종부터 적용)

포크 좌표(`VLLM_REPO`·`VLLM_REF`·변종 `IMAGE_TAG`)는 이름이 모델별일 뿐 성질은 **빌드 평면**이다. 그런데 `.env.<model>` 은 Band3(모델 트리플렛)라 `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION` 이 서브 전달을 막는다 — 그 결과 스모크가 슬레이브에 값을 몰래 보간하는 **그림자 배달 경로**가 자라났다(2026-08-13 발견).

- **좌표는 `.claude/policies/arch_variant_ledger.json`(tracked·Band2)에 등재한다.** 스키마는 기존 `source_build_variants` 항목(`variant_id`·`vllm_repo`·`vllm_ref`·`image_tag`·`evidence`·`status`)을 따른다.
- **`.env.<model>` 에는 `VARIANT=<variant_id>` 한 줄만** 둔다. 줄이 **없으면 stock** 이다 — 부재가 기본값이므로 "까다로운 절차를 벗겨낸 상태"가 값 수정이 아니라 줄 삭제로 표현되고, 오설정이 구조적으로 어려워진다.
- 이득: 좌표가 Band2 로 전달되므로 서브가 **트리플렛 전파 금지를 깨지 않고** 변종을 재현한다. 이미지:모델은 N:M 이며 N≪M 이다(2026-08-13 실측: 모델 env 25종 → 이미지 9종, 변종 접미사는 4종뿐).
- ⚠ **과잉 후보**: 변종 생성 빈도가 낮으면 등재 절차가 이득을 못 낼 수 있다. 변종 하나 만드는 손이 이전보다 늘면 이 규약을 버리고 build-arg 직접 지정으로 복귀한다(`plan_26081310` §철회 조건).

## 3+1+1 슬롯 판정 (2026-08-02 명문화)

판정 기준은 **"무엇을 고치나"가 아니라 "언제 성립해야 하나"**다. 위상을 틀리면 **조용히 무효화**된다.

| 슬롯 | 성립 시점 | 담는 것 | owner |
|---|---|---|---|
| 트리플렛 3 (`<model>.{yaml,sh}` · `.env.<model>`) | serve | 서빙 설정·러너·환경 | recipe/upstream |
| +1 런타임 패치 (`<model>_patch.py`) | serve(arming) | Python processor/config shim | `policy:RUNTIME_PATCH_NO_CARRY_FORWARD` |
| +1 빌드 패치 · **pre** (`build_patches_src/<NN>-*.sh`) ⚠**미검증 슬롯** | **컴파일 전** | vLLM **소스** 수정(Python/C++). `_C` 재컴파일이 필요한 csrc의 **유일한** 자리 | upstream `references/source-build.md` §4 |
| +1 빌드 패치 · **post** (`build_patches/<NN>-*.sh`) | **컴파일 후** | 빌드-바깥 native 의존(lib/커널) 설치 | 동상 |

- **위상 오배정의 실증**: 동일 게이트 완화를 런타임 `.pth`로 시도했으나 **vllm/ray 워커가 site-init을 안 타 불발**했고(발화 0회), 소스 패치는 전 프로세스에 균일하게 먹었다(`build_patches/30-mxfp4-triton-sm121.sh` 헤더). **런타임 슬롯은 "먹었는지"를 반드시 로그로 실증**하라.
- **native lib 은 `patch.py` ✗**(Python 몽키패치 불가) · **소스 수정은 post 슬롯 ✗**(컴파일이 이미 끝났다).
- ⚠ **`build_patches_src/` 는 미검증 슬롯이다** — 배관(Dockerfile 스탠자·순서·fail-loud 게이트)은 실제로 동작함이 확인됐으나(적용 92파일·무결성 검증 통과), **그 위에서 서빙에 성공한 사례가 아직 없다**(첫 사용 = `testlog_26080223`, 이식 3회 실패). 쓰는 사람이 **첫 검증자**이며, 실패해도 슬롯 탓인지 이식 내용 탓인지 먼저 갈라야 한다. 성공 사례가 나오면 이 표기를 지운다.

## 자체 이식 칸 — 착수 전 선판정

포크 전체를 핀하기 전에 **참조 포크를 분석해 우리 빌드 패치안을 만든다**. 단 아래 신호에 걸리면 **착수하지 말고 포크 핀으로 간다**(2026-08-02 실증 — `testlog_26080223`):

1. **PR 이 상위 스택 리팩터를 전제**하는가 — PR 델타 **밖** 파일이 PR 변경의 호출자/피호출자면 스택 경계가 갈라진다
2. **되돌림 비율** — "드리프트 위에 얹힘"으로 제외해야 하는 파일이 많을수록 그 브랜치 상태에 깊이 묶여 있다
3. **merge-base 거리** — 목표 태그↔merge-base 드리프트가 PR 델타보다 크게 우세하면 불리하다

착수 시 검증 순서(비용 절감용이며 **가능성 판정은 못 한다**): merge-base 분리 → 버킷 분류 → 3-way → 이식후 심볼 triage → **pyflakes 기준선 대비 증분** → 부재 모듈 검사 → AST 시그니처 대조 → import 시험 → **서빙 스모크(유일한 중재자)**.

## 메인↔서브 B0–B3

| 상태 | 입력→출력 | gate | 실패/owner |
|---|---|---|---|
| B0 bootstrap | 최초 배달→서브 local git | dry-run→HITL apply | `policy:SUB_GIT_LOCAL_ONLY` |
| B1 하향 | canonical render/output→서브 | dirty→보존 후 진행·checksum | `policy:SUB_SYNC_DIRTY_AUTOSAVE`; `sync_to_sub.sh` |
| B2 상향 | 서브 docs path→`sync_staging/sub_docs`→HITL 재저작 | **상향 회수(서브→메인) = 문서기반 only** | `fetch_sub_docs.sh`; patch/code 직접 회수 금지 |
| B3 경계 | attestation+미러 docs→메인 관측 | A2A key | 메인은 서브 디스크 재스캔/직접교정 금지 |

서브 개선은 `[improve]` history에 남고 메인 저작권은 template→render→delivery로만 행사한다. egress-restricted 서브의 외부조사 실패도 docs로만 상향한다.

### 권한 평면 A/B (2026-08-13 신설 · `plan_26081313`)

| | 평면 A: 사용자↔메인 | 평면 B: 메인↔서브 |
|---|---|---|
| 상대 | 독립 소유권 주체(사람) | 메인이 렌더·배달해 만든 작업환경 |
| 게이트 성격 | **승인** — 대행 시 자기승인 순환 | **소실 방지** — 대행이 정상(다른 주체 없음) |
| git 의 역할 | 사용자의 작업 이력·소유 표현 | **메인의 서브 추적용 관측 장치** |
| 예 | `execution_approval`·HITL 게이트 | `SUB_SYNC_DIRTY_AUTOSAVE` |

B 에 남는 게이트의 근거는 승인이 아니라 **격리(PII — 상향 문서기반 only)** 와 **범위(레시피 계열만 — `MODEL_TRIPLET_NO_SUB_PROPAGATION`)** 여야 한다. 승인 논리로 세운 것은 교착을 만든다.

### 권위 평면 계약 — 어느 도구가 무엇을 읽는가

| 도구 | 읽는 평면 | 귀결 |
|---|---|---|
| `sync_branches.sh` | **커밋**(`git checkout <branch> -- <path>`) | 미커밋 변경은 전파되지 않는다 |
| `sync_to_sub.sh` | **인덱스**(`ls-files \| checkout-index`) | `git add` 만으로 배달된다(커밋 불요) |
| 렌더·빌드 | **파일시스템** | 인덱스와 갈리면 `info:` 만 찍고 진행한다 |

같은 파일이 세 평면에서 서로 다른 값을 가질 수 있다. **새 도구는 이 표에 행을 추가한다** — 명문화 없이는 "고쳤는데 안 갔다"와 "안 고쳤는데 갔다"가 반복된다(2026-08-13 실제 발생).

### 결정론 규율 — 출처 표시와 4종 안티패턴 (2026-08-13 신설 · `plan_26081314`)

**규율**: 결정론으로 계산한 값과 그렇지 않은 값은 **데이터에서 구분되어야 한다.** 구분이 없으면 하류 판정·인증서가 무엇을 근거로 삼았는지 알 수 없고, 헌법의 `합성 금지`·`측정 > 공식`이 집행 불가가 된다. (근거 규율은 `seed/Chain of Code…pdf` Fig.1 — 인터프리터 실행분과 LM 에뮬레이션분을 program state 에서 가른다. **기법이 아니라 이 구분 규율만** 이식한다.)

- 산출물은 출처를 스스로 밝힌다: `provenance`(`measured`/`mock`/`dry-run`) · `tp_source` · `bandwidth_source` 처럼 **값 옆에 출처 필드**를 둔다.
- 금지가 아니라 **표시**가 처방이다 — mock 모드 자체는 배선 점검에 필요하다. 문제는 그것이 *실측인 척하는 것*이다.

**4종 안티패턴 판정표** — 뭉뚱그리면 교정이 과잉이 된다.

| 패턴 | **정당** | **결함** |
|---|---|---|
| 모킹 | 테스트 평면의 격리(외부 의존 차단) | **프로덕션 산출물과 같은 모양으로 나오는 것** |
| 폴백 | fail-loud 폴백(대체 후 로그·예외), 순수 파서의 `None`(호출부가 fail-closed 처리) | **결정·게이트·안전 경로에서 원인을 삼키는 침묵 폴백** |
| 하드코딩 | tripwire(변경 시 리뷰를 강제하는 닫힌 목록) | **파생 가능한데 손으로 적은 것** |
| 매직넘버 | 그 파일에서만 쓰는 국소 상수 | **같은 개념이 두 곳 이상에 손으로 적힌 값** |

- **단일 소유가 불가능하면 교차검증이 차선이다** — 정적 파일끼리는 한쪽이 다른 쪽을 생성할 수 없다. `assert_band2_top_gitignore_parity()`(BAND2_TOP ↔ `.gitignore`)가 그 예이며, 두 목록이 갈라져 생긴 침묵 누락이 실증 근거다.
- **개념 중복은 값 스캔으로 찾지 못한다** — 같은 값이 여러 파일에 있어도 대개 무관하다(`0.9` = ratio 허용범위 vs bw-floor 계수). 발견 계기는 **술어/검증 실패**이며, 신호가 온 것만 단일 소유로 승격한다.

### 막힘 3분류 — 대응이 다르다

| 분류 | 정체 | 대응 |
|---|---|---|
| **정상 차단** | 가드가 제 일을 함 | **우회하지 않는다.** 규칙대로 해소한다 |
| **침묵 누락** | 배선 부재로 조용히 안 감 | 배선을 만든다(예: `build_patches_src` 3곳 누락) |
| **오배달** | 권위 평면 오해로 엉뚱한 것을 보냄 | 위 계약 표로 평면을 확인한다 |

구분하지 않으면 사람도 에이전트도 우회를 택한다. D5 는 안내(`ALLOW_DELETE=<n>`)를 그대로 따르면 파괴가 완성되는 형태였다 — **안내문이 분류를 잘못 말하면 가드가 있어도 사고가 난다.**

## 완료 조건

컨테이너 변경은 S3 PASS 전 done이 아니다. 실패는 partial apply 없이 last-good로 복구하고(`policy:LAST_GOOD_ROLLBACK_ANCHOR`), build/검증 증거는 `docs/testlog/`, 전파 서사는 `docs/devlog/`에 남긴다. bump·full benchmark·모델 다운로드는 명시된 사람 승인 없이는 실행하지 않는다.

S2.5 가 서브에 배달한 내용은 메인의 S4 커밋 전까지 **미검증 후보**다. S3 실패로 메인이 커밋 없이
last-good 로 복구하면, 서브의 `[sync]` 커밋은 이력으로만 남기고 내용은 메인 정본에 수렴시켜야 한다
— 서브는 미검증 후보를 last-good 앵커·태그로 승격하지 않으며, 되돌림은 다음 B1 전파가 index 권위로
수행한다(2026-07-30 쌍노드 하드다운 때 현실화: 서브만 메인 미커밋 내용의 커밋을 보유).
