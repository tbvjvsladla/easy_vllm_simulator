---
name: hint-publisher
description: 검증된 서빙 레시피를 `hint/<vllm>/<model>/<arch>` 태그로 발행·색인·배포한다. "hint 태그 발행", "힌트 내보내기", "이 레시피 배포하자", "hint 색인 갱신", "이 모델 이력 모아줘" 같은 지시에 발동. 수신자(배포받은 코드에이전트)의 토큰노믹스가 목적이며, 발행은 분산·색인/배포는 중앙집중이다.
---

# hint-publisher — 검증된 레시피의 배포 sidecar

> 정본 계약: `hints/HINT_ISSUANCE_CONTRACT.md` · 색인 규칙: `docs/plan/plan_26082008` ·
> 승격 근거: `docs/plan/plan_26082009` · 발동 게이트: `policy:HINT_TAG_ACTIVATION_GATE`

## 0. 무엇인가

**배포받은 다른 코드에이전트가 토큰을 아끼도록 "정답지의 일부를 살짝 보여주는" 자료.**
완제품이 아니라 **여정의 지도**다. 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹으므로,
이미 뚫어본 벽 순서를 넘겨주는 것이 가장 큰 절약이다.

**상태를 전이하지 않는다** — workflow.md 의 S1→S4 spine 밖, S4 이후에 붙는 배포 행위다
(`wiki-desk` 와 같은 sidecar). 발행 조건은 다른 스킬이 만든다:

| 계약 §3 조건 | 만드는 주체 |
|---|---|
| A. 서빙 성공(`health_ok`·`functional_smoke_passed`) | `vllm-recipe-explorer` |
| B. lite 정량지표 | `adversarial-benchmark` |
| 제안 트리거(bump 종결) | `upstream-version-watch` |

## 1. 권한 — "누가 어느 원격의 카탈로그를 소유하는가" (D8 · 2026-08-20 재해석)

| 명령 | 권위 선언 없음 | 권위 선언 있음 |
|---|---|---|
| `match` · `collect` · `orphans` | ○ | ○ |
| `create` · `seal` | ○ | ○ |
| `index` · `reindex` · `push` | **✗ fail-closed** | ○ |

집행은 `hints/.central_authority`(**비추적**) 유무다. 신원 체계 없이 결정론으로 집행한다.

**마커는 자격증명이 아니라 역할 선언이다.** 비추적인 이유는 "누가 권위인가"가 배포본에 실려
복사되면 안 되기 때문이다 — **각 저장소가 자기 원격에 대해 스스로 선언**해야 한다.

- **자기 원격을 소유한 프로젝트**(포크·다운스트림 포함)는 그 원격의 hint 카탈로그 권위다.
  온보딩에서 한 번 선언한다:
  ```bash
  printf '%s\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > hints/.central_authority
  ```
  **선언은 권한이자 책임이다** — 그 원격의 `index.json`·`HINTS.md` 정합을 떠안는다.
- **상류에 기여하는 입장**이면 선언하지 말고 `seal` 로 로컬 태그까지만 만들어 상류에 전달한다.

> ⚠ **어느 쪽도 아니면 우회하지 말고 어느 쪽인지부터 정하라.** 맨 `git tag -a` + `git push` 로 만든
> 태그는 evidence-binding 이 없어 수신자 쪽 `reindex` 가 `status: unbound` 로 **격리**하고
> `collect` 가 *"근거로 쓰지 마라"* 경고를 붙인다. 2026-08-20 실제 발생 — 정문이 잠긴 채
> 여는 법이 안 적혀 있었고(이 절이 그 교정이다), 그래서 태그 4건이 그렇게 만들어졌다.

## 2. 발행 절차 — 필수 입력을 숨기지 않는다

> ⚠ **`create`·`seal`·`verify`·`push`·`reindex` 는 전부 `--manifest`(promotion-ready
> work-manifest)를 required 로 요구한다.** 종전 이 문서는 그 사실도, 만드는 법도 적지 않아
> 흐름대로 따라오면 `error: the following arguments are required: --topology, --manifest` 에서
> 막혔다. 그 침묵이 우회의 직접 원인이었다(plan_26082017 W1).

### 2.1 전제 — 발행 조건(계약 §3)이 곧 manifest 의 재료다

| 계약 §3 | 만드는 주체 | 산출물 |
|---|---|---|
| A. 서빙 성공(`health_ok`·`functional_smoke_passed`) | `vllm-recipe-explorer` | `docs/simlog/<run>/` |
| B. lite 정량지표(full 이면 자동 충족) | `adversarial-benchmark` | `docs/benchmark/` report·인증서 |

**A·B 가 없으면 manifest 를 만들 수 없다. 그건 도구 문제가 아니라 *발행 조건 미충족* 이다** —
우회하지 말고 A·B 를 먼저 채운다(D3: 정식 경로가 막히면 경로를 고친다).

### 2.2 manifest 생성 → 승인

```bash
# 1) 증거 발행 → work-manifest 생성 (publisher 가 소유 · .claude/rules/docs.md §publisher 계약)
python3 .claude/policies/runtime/evidence_publisher.py finalize ...   # → work-manifest JSON

# 2) 승인 확인 (read-only · 여기서 통과해야 hint 명령이 받는다)
python3 .claude/policies/runtime/completion_gate.py authorize \
    --manifest <work-manifest.json> --mode promotion --action hint_create
```
`--action` 은 `hint_create`·`hint_finalize`·`hint_verify`·`hint_reindex`·`hint_push`·`hint_reverify`
중 실행할 명령에 맞춘다(`index` 는 이 게이트를 타지 않는다 — 권위 선언만 본다).

### 2.3 태그 발행

```bash
M=<work-manifest.json>
hint_tag.py create --tag hint/<vllm>/<model>/<arch> --topology 'single 1노드' \
                   --hf-repo <org>/<name> --manifest $M          # → 레시피 스캐폴드(7절)
#   ↓ TODO(judgment) 슬롯을 실측으로 채운다. **`## 1. 벽 지도` 가 주 페이로드다** — 증상→원인→해소.
hint_tag.py seal   --tag <동일> --recipe <레시피.md> --topology '<동일>' --manifest $M
#   ↑ L1–L5 린터 + PII 4종 + evidence-binding footer 를 여기서 **fail-closed** 로 건다.
hint_tag.py index  --tag <동일>                                   # 권위 선언 필요
hint_tag.py push   --tag 'hint/<vllm>/<model>/*' --manifest $M --apply   # 권위 선언 필요
```

### 2.4 명령 일람

```
match    --vllm --model --arch     근-미스 발견(모델 불일치는 기본 숨김)
collect  --model [--sd-only]       한 모델의 **전 이력**을 시간순 수집(family 해소) ★수신자용
orphans  [--remote [NAME]]         태그↔index↔families 3중 대사 ★"빠짐없는 수집"의 확인 자리
create   --tag --topology --manifest [--hf-repo]   스캐폴드(슬러그는 HF repo 에서 파생)
seal     --tag --recipe --topology --manifest      PII+린트 L1–L5+footer → annotated 태그(색인 ✗)
index    --tag [--topology]        index.json + HINTS.md 편입 (권위 선언 필요)
reindex  --manifest [--strict]     전 태그에서 카탈로그 재생성. 기본=불량분 격리(status unbound)
verify / reverify / push / pin-legacy
```

`bootstrap_families.py` — `hints/families.json` 생성·감사. HF 카드의 `base_model` 로 family 파생.

## 3. 두 층 — 슬러그와 family

| 층 | 결정 | 가변성 | 거처 |
|---|---|---|---|
| **슬러그** | HF repo 이름에서 **도구가 파생** | 발행 후 불변 | 태그 이름 |
| **family** | `base_model` 전이 해소 → 후보 → 사람 승인 | 자유 수정 | `hints/families.json` |

양자화·리비전·SD 초안 모델은 **슬러그로는 여럿, family 로는 하나**다(사용자 D1·D7).
family 가 `index.json` 이 아니라 별도 파일인 이유: `reindex` 가 index 를 태그에서 **재생성**하므로
거기 두면 파괴된다(D9).

**family 는 동일성 선언이지 우열 판단이 아니다.** 무엇이 패턴이고 무엇이 안티패턴인지는
시간축을 봐야 알고 그건 수신자만 볼 수 있다 — 발행자는 **빠짐없는 수집**만 보장한다(D3).

## 4. 일관성은 린터가 보장한다 (템플릿이 아니라)

> **실증**: 템플릿은 처음부터 있었고 7개 절을 정의했다. 그런데 발행된 **49/49 태그에 헤딩이 0개**,
> 밀도는 **12배** 벌어졌다. 집행되던 유일한 검사(`TODO(judgment`)만 100% 지켜졌다.

`seal` 이 fail-closed 로 강제한다: **L1** 필수 절 존재 · **L2** 절 최소 밀도 · **L3** 전이등급 태깅 ·
**L4** TODO 잔존 ✗ · **L5** generic 문구뿐인 절 ✗ · PII 4종.
**`## 8. comment` 만 자유**다 — 발행자 간 편차를 이 한 칸에 가둔다.

## 5. 경계

- **무인 자동 태깅 ✗** — 제안(Y/N)만 한다. push 는 전부 사용자 소관.
- **서브 노드는 발행하지 않는다** — main-only(계약 §C4, 구조적으로 SSH 경로가 없다).
- **태그는 불변** — 발행 후 이름·본문을 고치지 않는다. 교정은 새 태그 또는 색인(추적·가변)으로 한다.
- **격리는 삭제가 아니다** — 증거 바인딩 불량 태그는 `status: unbound` 로 카탈로그에 남되 `collect` 가
  경고를 붙인다. 조용히 빼면 "기록이 원래 없었던 것"과 구분되지 않는다(plan_26082017 §4.3).
- tagger 신원은 공개값 `easy-vllm-simulator <hints@easy-vllm.invalid>`(운영자 신원은 PII 로 차단).
