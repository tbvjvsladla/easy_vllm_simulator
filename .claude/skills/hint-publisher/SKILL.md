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

## 1. 권한 비대칭 — 발행은 분산, 색인·배포는 중앙 (D8)

| 명령 | Contributor | 중앙 |
|---|---|---|
| `match` · `collect` | ○ | ○ |
| `create` · `seal` | ○ | ○ |
| `index` · `reindex` · `push` | **✗ fail-closed** | ○ |

집행은 `hints/.central_authority`(**gitignored**) 유무다 — 배포본에 실리지 않으므로
Contributor 환경에서는 존재할 수 없고 게이트가 자동으로 닫힌다. 신원 체계 없이 결정론으로 집행한다.

Contributor 흐름: `create` → 본문 저작 → `seal`(로컬 태그) → 태그 전달 → 중앙이 `index` → `push`.

## 2. 명령

```
match    --vllm --model --arch     근-미스 발견(모델 불일치는 기본 숨김)
collect  --model [--sd-only]       한 모델의 **전 이력**을 시간순 수집(family 해소) ★수신자용
create   --tag --hf-repo …         스캐폴드 생성(슬러그는 HF repo 에서 파생 — 발행자가 짓지 않는다)
seal     --tag --recipe …          PII 스캔 + 린트 L1–L5 + annotated 태그 (색인 ✗)
index    --tag                     index.json + HINTS.md 편입 (중앙 전용)
verify / reverify / reindex / push / pin-legacy
```

`bootstrap_families.py` — `hints/families.json` 생성·감사(중앙). HF 카드의 `base_model` 로 family 파생.

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
- tagger 신원은 공개값 `easy-vllm-simulator <hints@easy-vllm.invalid>`(운영자 신원은 PII 로 차단).
