# workflow.md — 런타임 전파 계약

> 목표: 사람의 업데이트/서빙 지시를 검증 가능한 상태 전이로 바꾸고, 실패를 정확한 owner로 돌린다.
> 항상 참인 경계는 `CLAUDE.md`; 세부 실행은 아래 owner 스킬·스크립트가 소유한다.

## 상태·owner 계약

| 상태 | owner | 입력 | 출력 | 통과 게이트 | 실패 라우팅 |
|---|---|---|---|---|---|
| init | `terraforming_node` | 인터뷰 답·노드 사실 | manifest·완수 Flag | plan HITL→스캔→attestation | 스킬 §0.5·`manifest_contract.py` |
| S1 resolve | `upstream-version-watch` | 목표 vLLM·manifest | torch/NGC/CUDA/build-track/deps 해소값 | HITL ① | 스킬 `references/resolve-and-render.md` |
| S1.5 judge | `upstream-version-watch` | from/to ref·resolved·(이식 트랙이면)PROVENANCE | `resolved.json#upstream_delta` 3축 attestation | HITL ①.5 | `judge_version_delta.py`(비-0 = 수집 실패) |
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
S1.5 judge  → `judge_version_delta.py` 로 버전 델타 3축 판정(A 빌드입력 · B 이식 스코프 · C 모델 코드경로)
   verify   → `resolved.json#upstream_delta` 발행; 사실 행은 이 JSON을 그대로 테이블화(손저작 금지)
   HITL 게이트 ①.5 → 사람 확인: ⓐ axis_A 가 정말 ∅ 인지 ⓑ axis_B.silent_revert_risk 처리 계획
                     ⓒ unknown[] 이 비었는지. **UNDETERMINED 면 렌더 진입 금지**
   (조건부) HITL 게이트 ①.6 → 출구① 상속을 쓸 때만: `INHERITED_SOURCE_BUILD_KEYS` 한 줄을 **사람이 손으로** 추가
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
- S4 뒤 공유 빌딩블럭 동기화는 사람 질의로만 `.claude/skills/upstream-version-watch/scripts/sync_branches.sh`; hint 후보는 `policy:HINT_TAG_ACTIVATION_GATE`와 `.claude/skills/hint-publisher/scripts/hint_tag.py`의 match→create→judgment→finalize→verify 순서만 허용한다.

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

arch-wall은 단계를 건너뛰지 않는다: deps-패치 → 소스-게이트 패치 → **자체 이식** → **소스-repo 오버라이드(포크 핀)** → 체크포인트-교체. `stock-구조적-불가` 뒤 **참조-그라운디드 확증** → **testlog 기록 + 사람 승인** → canonical ledger `source_build_variants` → **Dockerfile build-arg 파라미터화** → **clean 빌드(양노드) → 스모크** 순서다. 정본은 `policy:ARCH_WALL_VARIANT_LADDER`, upstream skill §escalation 수신 (ii) 커스텀/포크핀과 `references/source-build.md` §5다.

### 변종 좌표의 거처 (2026-08-13 신설 · 신규 변종부터 적용)

포크 좌표(`VLLM_REPO`·`VLLM_REF`·변종 `IMAGE_TAG`)는 이름이 모델별일 뿐 성질은 **빌드 평면**이다. 그런데 `.env.<model>` 은 Band3(모델 트리플렛)라 `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION` 이 서브 전달을 막는다 — 그 결과 스모크가 슬레이브에 값을 몰래 보간하는 **그림자 배달 경로**가 자라났다(2026-08-13 발견).

- **좌표는 `.claude/policies/arch_variant_ledger.json`(tracked·Band2)에 등재한다.** 스키마는 기존 `source_build_variants` 항목(`variant_id`·`vllm_repo`·`vllm_ref`·`image_tag`·`evidence`·`status`)을 따른다.
- **`.env.<model>` 에는 `VARIANT=<variant_id>` 한 줄만** 둔다. 줄이 **없으면 stock** 이다 — 부재가 기본값이므로 "까다로운 절차를 벗겨낸 상태"가 값 수정이 아니라 줄 삭제로 표현되고, 오설정이 구조적으로 어려워진다.
- 이득: 좌표가 Band2 로 전달되므로 서브가 **트리플렛 전파 금지를 깨지 않고** 변종을 재현한다. 이미지:모델은 N:M 이며 N≪M 이다(2026-08-13 실측: 모델 env 25종 → 이미지 9종, 변종 접미사는 4종뿐).
- ⚠ **과잉 후보**: 변종 생성 빈도가 낮으면 등재 절차가 이득을 못 낼 수 있다. 변종 하나 만드는 손이 이전보다 늘면 이 규약을 버리고 build-arg 직접 지정으로 복귀한다(`plan_26081310` §철회 조건).

## 3+1+1 슬롯 판정 (2026-08-02 명문화)

판정 기준은 **"무엇을 고치나"가 아니라 "언제 성립해야 하나"**다. 위상을 틀리면 **조용히 무효화**된다.

| 슬롯 | 성립 시점 | 담는 것 | **owner(단일)** | 적용 정책 |
|---|---|---|---|---|
| 트리플렛 3 (`<model>.{yaml,sh}` · `.env.<model>`) | serve | 서빙 설정·러너·환경 | **`vllm-recipe-explorer`** | `policy:MODEL_TRIPLET_NO_SUB_PROPAGATION` |
| +1 런타임 패치 (`<model>_patch.py`) | serve(arming) | Python processor/config shim | **`vllm-recipe-explorer`** | `policy:RUNTIME_PATCH_NO_CARRY_FORWARD` |
| +1 빌드 패치 · **pre** (`build_patches_src/<NN>-*.sh`) ⚠**미검증 슬롯** | **컴파일 전** | vLLM **소스** 수정(Python/C++). `_C` 재컴파일이 필요한 csrc의 **유일한** 자리 | **`upstream-version-watch`** (`references/source-build.md` §4) | `policy:VARIANT_IMAGE_BUILD_VS_SERVE_PLANE` |
| +1 빌드 패치 · **post** (`build_patches/<NN>-*.sh`) | **컴파일 후** | 빌드-바깥 native 의존(lib/커널) 설치 | **`upstream-version-watch`** (동상) | 동상 |
| (바깥) 포크 핀 · arch-wall 변종 | 빌드 | `VLLM_REPO`/`VLLM_REF`·변종 `IMAGE_TAG` | **`upstream-version-watch`** | `policy:ARCH_WALL_VARIANT_LADDER` |

> **owner 단일화(2026-08-15 · `plan_26081514` Q3/Step 4)**: 이전 표는 트리플렛 owner 가 `recipe/upstream`
> 으로 모호했고 런타임 패치 owner 자리에 **정책 ID**가 적혀 있어(정책은 owner 가 아니다) 소유 주체가
> 없었다. 판정 기준은 **성립 시점**이다 — serve 시점 성립분은 explorer, 빌드 시점 성립분은 upstream.
>
> **발견 ≠ 소유 경로**: explorer 가 "이 모델은 포크/빌드패치가 필요하다"를 **발견**하면
> (`vllm-recipe-explorer` §5.5 발견 술어 2개 동시 충족) → 사용자 명시 승인 → `upstream-version-watch`
> §escalation 으로 **핸드오프**한다. explorer 는 버전핀 변경·이미지 빌드를 **하지 않는다**. 왕복은
> 발견 1회 → 소유 1회이며, 핑퐁이 생기면 그것은 owner 표가 아니라 증거가 부족한 것이다.

- **위상 오배정의 실증**: 동일 게이트 완화를 런타임 `.pth`로 시도했으나 **vllm/ray 워커가 site-init을 안 타 불발**했고(발화 0회), 소스 패치는 전 프로세스에 균일하게 먹었다(`build_patches/30-mxfp4-triton-sm121.sh` 헤더). **런타임 슬롯은 "먹었는지"를 반드시 로그로 실증**하라.
- **native lib 은 `patch.py` ✗**(Python 몽키패치 불가) · **소스 수정은 post 슬롯 ✗**(컴파일이 이미 끝났다).
- ⚠ **`build_patches_src/` 는 미검증 슬롯이다** — 배관(Dockerfile 스탠자·순서·fail-loud 게이트)은 실제로 동작함이 확인됐으나(적용 92파일·무결성 검증 통과), **그 위에서 서빙에 성공한 사례가 아직 없다**(첫 사용 = `testlog_26080223`, 이식 3회 실패). 쓰는 사람이 **첫 검증자**이며, 실패해도 슬롯 탓인지 이식 내용 탓인지 먼저 갈라야 한다. 성공 사례가 나오면 이 표기를 지운다.

## 자체 이식 칸 — 착수 전 선판정

포크 전체를 핀하기 전에 **참조 포크를 분석해 우리 빌드 패치안을 만든다**. 단 아래 신호에 걸리면 **착수하지 말고 포크 핀으로 간다**(2026-08-02 실증 — `testlog_26080223`):

1. **PR 이 상위 스택 리팩터를 전제**하는가 — PR 델타 **밖** 파일이 PR 변경의 호출자/피호출자면 스택 경계가 갈라진다
2. **되돌림 비율** — "드리프트 위에 얹힘"으로 제외해야 하는 파일이 많을수록 그 브랜치 상태에 깊이 묶여 있다
3. **merge-base 거리** — 목표 태그↔merge-base 드리프트가 PR 델타보다 크게 우세하면 불리하다

착수 시 검증 순서(비용 절감용이며 **가능성 판정은 못 한다**): merge-base 분리 → 버킷 분류 → 3-way → 이식후 심볼 triage → **pyflakes 기준선 대비 증분** → 부재 모듈 검사 → AST 시그니처 대조 → import 시험 → **서빙 스모크(유일한 중재자)**.

## 메인↔서브 노드 제어 — 정본은 `terraforming_node` SKILL.md §2.7

> **2026-08-15 이관**(`plan_26081514` Step 1·3). 아래 5주제는 **노드 도메인 절차**이므로 스킬이 소유한다.
> workflow.md 는 전이 spine 만 갖고, 절차 본문은 중복하지 않는다(중복은 갈라져 침묵 누락을 만든다).

| 주제 | 정본 | 이관 전 원문 |
|---|---|---|
| 메인↔서브 B0–B3 상태 표 | `terraforming_node` SKILL.md **§2.7.3** | `workflow.md@0d8f542eafa3` §메인↔서브 B0–B3 |
| 권한 평면 A/B(승인 vs 소실방지) | 동 **§2.7.1** | 동 §권한 평면 A/B |
| 저작·스캔·정비 3범주 | 동 **§2.7.2** | 동 §3범주 |
| 권위 평면 계약(커밋/인덱스/파일시스템) | 동 **§2.7.4** | 동 §권위 평면 계약 |
| sync 절차의 평면 분리 · node-identity · A2A 제어명령 | 동 **§2.7.5–§2.7.7** | (신설) |

- 전이 표(§상태·owner 계약)의 **S2.5 sync** 행과 §공통 진입 게이트의 A2A 검증은 그대로 이 문서가 갖는다 —
  *언제* 하는지는 spine 이고, *어떻게·어떤 권한으로* 하는지가 스킬이다.
- 헌법(`CLAUDE.md`)에는 이 주제의 **"왜"** 5불변식만 남는다(무단스캔 금지 · 관측장치+완전조작권한+A2A해제 ·
  처방주체 선기재 · D3 경로수리 · 발견≠소유).

### 결정론 규율 — 출처 표시와 4종 안티패턴 (2026-08-13 신설 · `plan_26081314`)

**규율**: 결정론으로 계산한 값과 그렇지 않은 값은 **데이터에서 구분되어야 한다.** 구분이 없으면 하류 판정·인증서가 무엇을 근거로 삼았는지 알 수 없고, 헌법의 `합성 금지`·`측정 > 공식`이 집행 불가가 된다. (근거 규율은 `seed/Chain of Code…pdf` Fig.1 — 인터프리터 실행분과 LM 에뮬레이션분을 program state 에서 가른다. **기법이 아니라 이 구분 규율만** 이식한다.)

- 산출물은 출처를 스스로 밝힌다: `provenance`(`measured`/`mock`/`dry-run`) · `tp_source` · `bandwidth_source` 처럼 **값 옆에 출처 필드**를 둔다.
- 금지가 아니라 **표시**가 처방이다 — mock 모드 자체는 배선 점검에 필요하다. 문제는 그것이 *실측인 척하는 것*이다.

**4종 안티패턴 판정표** — 뭉뚱그리면 교정이 과잉이 된다.

> **양태는 `금지`가 아니라 `자제`다**(2026-08-14 사용자 결정). 네 패턴 모두 **상황에 따라 허용**되며,
> 아래 표의 **정당** 칸이 그 허용 범위다. 따라서 "이건 폴백이니까 안 됨" 같은 **패턴 이름만으로 하는
> 기각은 무효**이고, 기각하려면 **결함 칸의 어느 조건에 걸리는지**를 지목해야 한다. 반대로 정당 칸에
> 해당하면 그대로 쓰되 §결정론 규율의 **출처 표시**(`provenance`·`*_source`)를 붙인다 — 처방은
> 제거가 아니라 표시다. 이 완화는 4종 안티패턴에만 적용되며, 독립 근거를 가진 도메인 규칙
> (예: `check_smoke_model.py` 의 루트경로 폴백 금지 = fail-loud 게이트, manifest 포인터 원칙)은
> **그대로 금지로 남는다** — 그것들은 안티패턴이라서가 아니라 **안전·정합 근거로** 금지된 것이다.

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

### git 단일 권위 — 2문항 판정표와 백업 4형태 (2026-09-03 신설 · `policy:GIT_SINGLE_AUTHORITY`)

무결성 해시·사본을 새로 두려 할 때, 그리고 이미 있는 것을 걷어낼지 판정할 때 **두 문항만** 묻는다.
판정은 **검증이 실행되는 그 노드**를 기준으로 한다(git 이 없는 노드에서는 Q1 이 아니오다).

| # | 질문 | 예 | 아니오 |
|---|---|---|---|
| **Q1** | 그 노드의 git 이 이 대상을 **blob 으로 드는가**(추적물인가) | 중복층 | ↓ |
| **Q2** | 추적 입력만으로 **결정론적으로 재생성**되는가 | 중복층 | 맹점층 |

- **하나라도 예 → 중복층**: 그 층을 삭제하고 판정을 git(또는 재생성)으로 되돌린다. 추적물의
  digest 를 두 번째 자리에 다시 적는 것은 단일 권위를 깨고 **두 자리가 갈라지는 침묵 누락**을 만든다.
- **둘 다 아니오 → 맹점층**: git 이 바이트를 들지 않는 입력(상류 소스 payload·포크 핀·인용 만료
  대상)이므로 digest 를 **유지**한다. 이 자리의 해시는 중복이 아니라 유일한 증거다.
- 재현성 증명은 digest 대조가 아니라 **재빌드 + 스모크**다 — 같은 입력에서 같은 산출이 나오는지는
  실행이 답하며, 추적물끼리의 digest 대조는 그 질문에 답하지 못한다.

**백업 4형태** — 아래 넷은 모두 같은 결함(git 이 이미 드는 것을 두 번째 자리에 복제)의 변종이다.

| 형태 | 정체 | 대신 |
|---|---|---|
| `.bak`/`.orig` 류 사본 | 편집 전 파일을 옆에 복제 | `git diff` · `git checkout -- <path>` |
| backup 브랜치 | 상태를 별도 ref 로 고정 | 커밋 이력(앵커는 커밋이다) |
| backup 폴더 | 트리 전체를 디렉터리로 복제 | `git worktree` · 이력 |
| digest 재기재 | 추적물의 해시를 데이터 파일에 다시 적음 | `git ls-files --error-unmatch` · `git hash-object` 대조 |

- **적용 범위**: 이 저장소 · 서브 워크트리 · 산출물 디렉터리. 음성대조가 필요하면 저장소 밖
  임시 디렉터리에서 하고, 워킹트리에 잔재를 남기지 않는다.
- **범위 밖**: 호스트 `/boot` 의 grub 백업(부팅 복구 수단이며 git 평면이 아니다) · `seed/`
  (사용자 보관소이자 비추적 평면 — 이 규약의 대상이 아니다).

## 완료 조건

컨테이너 변경은 S3 PASS 전 done이 아니다. 실패는 partial apply 없이 last-good로 복구하고(`policy:LAST_GOOD_ROLLBACK_ANCHOR`), build/검증 증거는 `docs/testlog/`, 전파 서사는 `docs/devlog/`에 남긴다. bump·full benchmark·모델 다운로드는 명시된 사람 승인 없이는 실행하지 않는다.

**작업 단위가 끝나면 미커밋 0 이다** — 워킹트리에 남은 변경은 백업 사본이 자라는 자리이고, 다음
작업자에게는 "누가 언제 왜" 가 없는 상태로 보인다. 끝내지 못한 작업은 커밋하거나 되돌리며, 둘 다
아닌 채로 넘기지 않는다(`policy:GIT_SINGLE_AUTHORITY`).

S2.5 가 서브에 배달한 내용은 메인의 S4 커밋 전까지 **미검증 후보**다. S3 실패로 메인이 커밋 없이
last-good 로 복구하면, 서브의 `[sync]` 커밋은 이력으로만 남기고 내용은 메인 정본에 수렴시켜야 한다
— 서브는 미검증 후보를 last-good 앵커 커밋으로 승격하지 않으며, 되돌림은 다음 B1 전파가 index 권위로
수행한다(2026-07-30 쌍노드 하드다운 때 현실화: 서브만 메인 미커밋 내용의 커밋을 보유).

> **last-good 는 커밋이지 태그·ref 가 아니다**(2026-09-03 정정 · `policy:LAST_GOOD_ROLLBACK_ANCHOR`).
> 앵커는 스모크를 통과한 **마지막 로컬 커밋**이며, 이를 가리키는 별도의 태그나 브랜치를 만들지 않는다
> — 만들면 그 순간 같은 사실이 두 자리에 적힌다(`policy:GIT_SINGLE_AUTHORITY`). 메인의 운영 refs 는
> 브랜치 `single-node`·`multi-node`·`hint` 와 `hint/*` 태그뿐이다.
