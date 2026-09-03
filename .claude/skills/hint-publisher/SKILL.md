---
name: hint-publisher
description: 검증된 서빙 지식을 `hint/<vllm>/<model>/<arch>` 태그로 **저작·발행·배포**한다. 태그는 hint 전용 브랜치의 페이로드 커밋을 가리키므로 archive 가 곧 재현 키트다. "hint 태그 발행", "힌트 내보내기", "이 레시피 배포하자", "카탈로그 갱신", "이 모델 이력 모아줘" 같은 지시에 발동. 수신자(배포받은 코드에이전트)의 토큰노믹스가 목적이며, 저작은 메인 단독·카탈로그는 원격 발행 태그에서 파생한다.
---

# hint-publisher — hint 브랜치를 관리하며 배포 포맷을 저작하는 스킬

> 정본 계약: `hints/HINT_ISSUANCE_CONTRACT.md` · 아키텍처: `docs/plan/plan_26090107`
> 감사: `docs/report/audit_26090106`(21건) · `docs/report/audit_26090118`(재감사) ·
> 발동 게이트: `policy:HINT_TAG_ACTIVATION_GATE`

## 0. 무엇인가

**배포받은 다른 코드에이전트가 토큰을 아끼도록 "정답지의 일부를 살짝 보여주는" 자료.**
완제품이 아니라 **여정의 지도**다. 에이전트 작업은 *탐색*이 입력 토큰의 60~70%를 먹으므로,
이미 뚫어본 벽 순서를 넘겨주는 것이 가장 큰 절약이다.

**2026-09-01 정체 전환** — 태그는 더 이상 코드 브랜치 커밋을 가리키지 않는다.

```
옛:  hint 태그 → single-node 커밋   → archive = 프로젝트 스냅샷(.claude/ 가 63%)
새:  hint 태그 → hint 브랜치 커밋   → archive = **정확히 그 모델의 재현 키트**
```

`Source code (zip/tar.gz)` 를 받으면 3항목 문서 + 재현 자산 실물만 들어 있다.
이 전환이 없으면 *원하는 것은 빠지고 원치 않는 것은 들어가는* 상태가 된다 —
트리플렛·서사는 `.gitignore` 때문에 코드 커밋에 **없고**, 스킬 코드는 **다 들어간다**.

**상태를 전이하지 않는다** — workflow.md 의 S1→S4 spine 밖, S4 이후에 붙는다. 발행 조건은
다른 스킬이 만든다:

| 계약 §3 조건 | 만드는 주체 |
|---|---|
| A. 서빙 성공(`health_ok`·`functional_smoke_passed`) | `vllm-recipe-explorer` |
| B. lite 정량지표 | `adversarial-benchmark` |
| 제안 트리거(bump 종결) | `upstream-version-watch` |

## 1. 권한 — "누가 어느 원격의 카탈로그를 소유하는가"

| 명령 | 권위 선언 없음 | 권위 선언 있음 |
|---|---|---|
| `match` · `collect` · `orphans` | ○ | ○ |
| `hint_collect` · `hint_branch` · `seal` | ○ | ○ |
| `hint_catalog derive` · `push` | **✗ fail-closed** | ○ |

집행은 `hints/.central_authority`(**비추적**) 유무다. 신원 체계 없이 결정론으로 집행한다.

**마커는 자격증명이 아니라 역할 선언이다.** 비추적인 이유는 "누가 권위인가"가 배포본에 실려
복사되면 안 되기 때문이다 — **각 저장소가 자기 원격에 대해 스스로 선언**해야 한다.

- **자기 원격을 소유한 프로젝트**(포크·다운스트림 포함)는 그 원격의 hint 카탈로그 권위다:
  ```bash
  printf '%s\n' '이 체크아웃이 자기 원격의 hint 색인·배포 권위다.' > hints/.central_authority
  ```
  **선언은 권한이자 책임이다** — 그 원격의 `index.json`·`HINTS.md` 정합을 떠안는다.
- **상류에 기여하는 입장**이면 선언하지 말고 `seal` 로 로컬 태그까지만 만들어 상류에 전달한다.

> ⚠ **어느 쪽도 아니면 우회하지 말고 어느 쪽인지부터 정하라.** 맨 `git tag -a` + `git push` 로
> 만든 태그는 evidence-binding 이 없고, 카탈로그가 **원격 발행 태그에서 파생**되므로 그런 태그도
> 목록에는 들어간다 — 그러나 본문이 계약을 못 지켜 `verify` 가 잡는다. 정문으로 가라.

## 2. 발행 절차

세 스크립트가 **소유를 나눈다**. 경계를 흐리면 게이트가 엉뚱한 것을 지킨다.

| 스크립트 | 소유 |
|---|---|
| `hint_collect.py` | **사실 수확 + 저작 스캐폴드** — 슬롯 발견 · 인증서 파싱 · 3신호 대사 |
| `hint_branch.py` | **트리** — allowlist 구성 · 페이로드 PII · 앵커 3중 · hint 커밋 |
| `hint_tag.py` | **태그 본문** — 린터 L1–L5 · evidence footer · annotated 태그 · push |
| `hint_catalog.py` | **카탈로그** — 원격 발행 태그에서 파생 |

### 2.1 전제 — 발행 조건(계약 §3)이 곧 manifest 의 재료다

`create`·`seal`·`verify`·`push` 는 전부 `--manifest`(promotion-ready work-manifest)를
**required** 로 요구한다. 만드는 법은 §2.2.

`--manifest` 는 **hint 전용 `promotion_target` 블록**을 담아야 한다 — 범용 승격 manifest 로는
안 된다(blocker 1). 그 블록의 `anchor` 는 **태그가 가리킬 hint 커밋**이다:

```json
"promotion_target": {"kind": "hint", "tag": "hint/<vllm>/<model>/<arch>",
                     "topology": "<토폴로지 라벨>", "anchor": "<hint 커밋 SHA>"}
```

### 2.2 manifest 생성 → 승인

```bash
# 1) 증거 발행 → work-manifest (publisher 소유 · .claude/rules/docs.md §publisher 계약)
python3 .claude/policies/runtime/evidence_publisher.py ...  finalize
# 2) 승인 확인 (read-only · 여기서 통과해야 hint 명령이 받는다)
python3 .claude/policies/runtime/completion_gate.py authorize --manifest $M \
        --mode promotion --action hint_finalize
```

### 2.3 저작 → 트리 → 태그 → 카탈로그

```bash
M=<work-manifest.json>; P=<페이로드 디렉터리(비어 있어야 함)>

# ① 수확 + 스캐폴드 — 기계가 사실을 채우고 판단 자리는 <<AGENT:…>> 마커로 남긴다
python3 .../hint_collect.py collect --manifest $M --config-name <트리플렛 basename> \
        --out $P --generated-kst <YYYY-MM-DDTHH:MM:SS>

# ② 저작 — 마커를 채우고 slots.declaration.json 의 applicable/rationale 을 기입한다
#    항목1 적용사유 · 항목2 서사(인용 필수 · 원문 전재 ✗) · 항목3 like-with-like 한정자

# ③ 저작 완료 게이트 + 슬롯 3신호 대사
python3 .../hint_collect.py check --payload $P

# ④ 트리 확정 → hint 브랜치 커밋 (메인 워킹트리를 건드리지 않는다 · 배관만 쓴다)
python3 .../hint_branch.py publish --payload $P --manifest $P/files.txt \
        --tag <태그> --anchor <소스 커밋> --message <커밋메시지 파일> --generated-kst <…>

# ⑤ 태그 본문 — 린터 L1–L5 + PII 4종 + evidence footer. `--commit` = ④가 만든 hint 커밋
python3 .../hint_tag.py seal --tag <태그> --recipe <본문.md> --topology '<라벨>' \
        --manifest $M --commit <hint 커밋 SHA> --hf-repo <org>/<name>

# ⑥ push (태그 + hint 브랜치) → ⑦ 카탈로그 파생
python3 .../hint_tag.py push --tag '<태그>' --manifest $M --apply
python3 .../hint_catalog.py derive --remote <원격> --generated-kst <…>
```

> **앵커가 둘이다 — 헷갈리지 마라.**
> `--anchor`(④) = 산출물을 **만든** 소스 커밋. 재현하려면 체크아웃할 상태다.
> `--commit`(⑤) = 태그가 **가리킬** hint 페이로드 커밋.
> 전자는 ⓐ hint 커밋 메시지 · ⓑ 태그 본문 · ⓒ `PROVENANCE.json` 셋에 **모두** 있어야 한다.

### 2.4 명령 일람

```
hint_collect  collect --manifest --config-name --out --generated-kst [--topology]
              check   --payload                     저작 완료 + 3신호 대사 fail-closed
hint_branch   publish --payload --manifest --tag --anchor --message --generated-kst
              build-tree / verify-tree              트리 전수 == allowlist 완전일치
hint_catalog  derive  --remote --generated-kst      원격 발행 태그 → index.json + HINTS.md
hint_tag      create / seal / verify / reverify / push / match / collect / orphans
              ✗ index · reindex — **폐쇄됨**(D1.1). 카탈로그는 hint_catalog derive 가 소유한다
```

`bootstrap_families.py` — `hints/families.json` 생성·감사. HF 카드의 `base_model` 로 family 파생.
`--scan-sd` 로 태그 본문에서 SD 능력을 수확한다. **`source: judgment|manual` 항목은 기계가
덮지 않는다**(감사 ⑧ 교정) — 사람 판정은 태그보다 오래 산다.

## 3. 페이로드 구성

| # | 항목 | 소유 |
|---|---|---|
| 1 | `01-artifacts.md` — 슬롯 판정 + **적용/불해당 사유** | 결정론(표) + Agent(사유) |
| 2 | `02-narrative.md` — 증상→원인→해소 + "되풀이하지 말 것" | **Agent**(인용 필수 · 원문 전재 ✗) |
| 3 | `03-benchmark.md` — 수치 + like-with-like 한정자 | 결정론(파싱 · 합성 ✗) + Agent(한정자) |

기계 사실은 `PAYLOAD.json`, Agent 선언은 `slots.declaration.json`, 앵커는 `PROVENANCE.json`.
**재현 자산 실물**은 `artifacts/` 아래 슬롯별로 놓인다:

```
artifacts/triplet/       설정·러너·모델 env       (serve)
artifacts/runtime_patch/ processor/config shim    (serve arming)
artifacts/build_patch_*  소스/네이티브 패치        (컴파일 전/후)
artifacts/build_recipe/  Dockerfile · requirements (build)
artifacts/compose/       compose + **env 형상 템플릿** (serve orchestration)
```

> ⚠ **토폴로지 `.env` 실물은 배포하지 않는다** — 운영자 NAS 루트·tiktoken 절대경로를 담는다.
> 대신 값만 `<manifest.<field>>` 로 가린 **형상**을 넣는다. 어떤 변수가 필요한지는 재현에
> 필수 정보이고, 값은 각자 manifest 에서 온다.

### 3.1 슬롯 3신호 대사

**파일 존재 × 적용 증거 × Agent 선언.** 셋이 어긋나면 차단이다.

| 파일 | 적용증거 | 선언 | 판정 |
|---|---|---|---|
| 부재 | 부재 | 불해당 | `not-applicable` 통과 |
| 부재 | **있음** | — | **차단** — 진짜 누락 |
| 있음 | **부재** | — | **차단** — 먹지 않은 패치를 재현지침으로 배포하게 된다 |

- ★ **`triplet`·`build_recipe`·`compose` 는 선언으로 면제 불가**다. 재현에 원리적으로 필수이므로
  부재는 언제나 누락이고, Agent 가 "불해당"이라 해도 통과시키지 않는다.
- 적용 증거의 **관측원이 없으면 `None`(모름)이지 `False`(없음)가 아니다.** 둘을 뭉개면 거짓
  음성이 난다(감사 ⑦ 실증). 그 경우 `slot_confidence` 가 **2-signal** 로 떨어지고, 그 사실이
  산출물에 표시된다 — 강도가 다른 판정을 섞으면 하류가 근거를 알 수 없다.

## 4. 두 층 — 슬러그와 family

| 층 | 결정 | 가변성 | 거처 |
|---|---|---|---|
| **슬러그** | HF repo 이름에서 **도구가 파생** | 발행 후 불변 | 태그 이름 |
| **family** | `base_model` 전이 해소 → 후보 → 사람 승인 | 자유 수정 | `hints/families.json` |

양자화·리비전·SD 초안 모델은 **슬러그로는 여럿, family 로는 하나**다.
family 가 `index.json` 이 아니라 별도 파일인 이유: 카탈로그는 **원격 태그에서 재생성**되므로
거기 두면 파괴된다.

**family 는 동일성 선언이지 우열 판단이 아니다.** 무엇이 패턴이고 무엇이 안티패턴인지는
시간축을 봐야 알고 그건 수신자만 볼 수 있다 — 발행자는 **빠짐없는 수집**만 보장한다.

## 5. 일관성은 린터가 보장한다 (템플릿이 아니라)

> **실증**: 템플릿은 처음부터 있었고 7개 절을 정의했다. 그런데 발행된 **49/49 태그에 헤딩이 0개**,
> 밀도는 **12배** 벌어졌다. 집행되던 유일한 검사(`TODO(judgment`)만 100% 지켜졌다.

`seal` 이 fail-closed 로 강제한다: **L1** 필수 절 존재 · **L2** 절 최소 밀도 · **L3** 전이등급 태깅 ·
**L4** TODO 잔존 ✗ · **L5** generic 문구뿐인 절 ✗ · PII 4종.
**`## 8. comment` 만 자유**다 — 발행자 간 편차를 이 한 칸에 가둔다.

트리 쪽에도 같은 규율이 걸린다: **allowlist 완전일치**(초과 0 · 부족 0) · 금지 최상위(`.claude`
등) 거부 · 페이로드 PII 4종 · 앵커 3중 일치. **선언이 아니라 결과를 검사한다.**

## 6. 경계

- **무인 자동 태깅 ✗** — 제안(Y/N)만 한다.
- **서브 노드는 발행하지 않는다** — main-only(계약 §C4). 서브 산출물은 문서기반 회수 뒤
  **메인이 재저작**한다(`docs.md` 상향 회수 규약). 코드·설정 직접 회수 금지.
- **태그는 불변** — 발행 후 이름·본문을 고치지 않는다. 교정은 새 태그로 한다.
- **카탈로그는 손으로 쓰지 않는다** — 진실원천은 `git ls-remote`. 원격 조회에 실패하면
  캐시로 대체하지 않고 **중단**한다. 낡은 카탈로그는 부재와 구분되지 않는다.
- **hint 브랜치를 메인 워킹트리에 체크아웃하지 않는다** — 배관만 쓴다(체크아웃 0회).
  브랜치 전환이 산출물을 파괴한 실측 이력이 있다.
- **hint 브랜치는 빌딩블럭이 아니라 산출물**이므로 `sync_branches` 대상이 아니다.
- tagger 신원의 **기본값이 프로젝트 합성 신원**이다(`DEFAULT_TAGGER_*`). 실명을 쓰려면
  `--tagger-name/--tagger-email` 로 **명시**해야 한다 — 잊으면 새는 구조를 뒤집은 것이다.
