# hint-publisher — 코드레벨 감사

> 감사 시각: 2026-09-01 06:00 KST (KST 절대시각 `26090106`)
> 대상 브랜치: `single-node` · 대상 커밋: `d9fbdfb`
> 감사 범위: `.claude/skills/hint-publisher/` 전량 —
> `scripts/hint_tag.py`(2158행) · `scripts/bootstrap_families.py`(320행) · `SKILL.md` · `templates/hint_recipe.template.md`
> 판정 기준 정본: `hints/HINT_ISSUANCE_CONTRACT.md`(v2.1) · `CLAUDE.md` · `.claude/rules/workflow.md` · `.claude/rules/docs.md`
> 감사 방식: PR Review Toolkit 전문 에이전트 5종 병렬 정적감사 + 감사자 직접 실행 실증
> 침습도: **read-only** — 대상 파일 및 저장소 상태를 변경하지 않았다.
>
> **개정 2026-09-01 08시 KST — 해소 상태 덧씌움.** 본 감사 이후 `plan_26090107` Phase 1 이 실행되어
> 21건 중 8건이 해소, 2건이 부분 해소됐다. **원 판정문은 한 글자도 고치지 않았다** — 감사는 그 시점의
> 기록이고, 고쳐 쓰면 무엇이 있었는지가 사라진다. 대신 아래 §해소 상태를 덧씌운다(규약 §공통 발행
> 계약의 *"단순 보완은 citation만 추가"* 에 해당하며, 판정을 뒤집은 것이 아니라 **처방이 집행된 것**이다).

---

## 결론

**hint-publisher 의 게이트는 "설계는 정확하고 배선이 비어 있다."**

발견 21건 중 **로직 오류는 소수이고 대부분이 배선 공백**이다 — 호출자 0, 게이트가 문의 반대쪽,
필수 입력이 생성기에서 누락, 검사기가 자기 산출물을 안 봄, 값을 받아놓고 버림. 이 분포는
이 프로젝트가 반복 기록한 **"만든 것과 도는 것은 다르다"** 계열 그대로다.

계약 v2 가 v1 을 고친 핵심은 *차단 사유를 형식 미비 → 증거 부재로 옮긴다* 였는데, **그 증거 검사가
태그를 실제로 만드는 명령에 연결되어 있지 않다.** 그 결과 계약 §2 가 지목한 유일한 위협
("서빙 실패를 성공으로 허위기재한 정보의 배포")에 대한 실질 방어가 우회 가능한 상태다.

동시에 **이미 발생한 배포물 손상 4건**이 확인됐다 — 배포 카탈로그 `HINTS.md` 58행 중 17행이
렌더에서 소실되고, 수신자용 `collect` 가 실재하는 태그를 침묵 누락하며, SD 능력 색인이 전량 무력화됐다.

이 판정은 **hint 레이어의 설계 가치를 부정하지 않는다.** 등급 분류(v1 빈티지 면제 · 수신자 평면
부재 강등 · 승인 드리프트 동결)는 실제 운영 사고에서 도출된 정교한 설계이며, 결함은 그 설계가
**무형식 dict/str/문자열 메시지 위에 얹혀 있다**는 구현 층위에 있다.

---

## 감사 방법과 증거 등급

전문 에이전트 5종을 동일 대상에 병렬 투입하고, 그 결과 중 파급이 큰 주장은 감사자가 직접 실행해
확증했다. 본문의 각 발견에는 증거 등급을 병기한다.

| 등급 | 뜻 |
|---|---|
| **실측** | 감사자가 이 저장소에서 직접 실행해 관측한 결과 |
| **대조** | 감사자가 코드를 직접 읽어 확인한 사실 |
| **정적** | 에이전트의 코드 독해 결과 — 감사자가 재실행하지 않음 |

투입 에이전트: `code-reviewer` · `silent-failure-hunter` · `comment-analyzer` ·
`pr-test-analyzer` · `type-design-analyzer`.

### 감사 기준선 (실측)

| 항목 | 값 |
|---|---|
| 로컬 hint 태그 | 62종 |
| `hints/index.json` 항목 | 58종 (전부 `status: active`) |
| 색인 미등재 태그 | 5종 |
| 색인에 있으나 로컬 태그 없음 | 1종 |
| `hints/.central_authority` | **부재** → `index`/`reindex`/`push` 가 현재 닫힘 |
| `.claude/pii_terms.txt` | **부재** (비추적 설계 · `.gitignore` 등재) |
| `hints/evidence_drift_pins.json` | **부재** → 드리프트 핀 기구 전체가 사문 |
| family 색인 | 29 family / 36 멤버 |

---

## 발견 일람

| # | 심각도 | 발견 | 증거 |
|---|---|---|---|
| ① | 1 | 계약 §3 발행조건이 태그 생성 경로에 배선되지 않음 | 대조 |
| ② | 1 | 메시지 문자열 주입으로 차단이 경고로 강등됨 | **실측** |
| ③ | 1 | 봉인 검사가 피검사자 통제 필드로 켜지고 꺼짐 · footer 가 본문을 안 묶음 | 대조 + **실측** |
| ④ | 1 | `index` 가 무검증으로 배포 카탈로그에 `active` 를 박음 | 대조 |
| ⑤ | 2 | `HINTS.md` 58행 중 17행이 렌더 소실 | **실측** |
| ⑥ | 2 | `collect` 가 실재 태그를 침묵 누락 (계약 §1 의무 파손) | **실측** |
| ⑦ | 2 | `collect --sd-only` 거짓 음성 | **실측** |
| ⑧ | 2 | `sd_capability` 29/29 family 전량 `unknown` (부분 보존에 의한 퇴화) | **실측** |
| ⑨ | 3 | `create` 가 출력하는 다음-단계 명령이 항상 실패 | 대조 |
| ⑩ | 3 | `finalize` 가 태그 생성 뒤 권위 검사 → 부분적용 + 재시도 차단 | 대조 |
| ⑪ | 3 | `bootstrap --write` 가 `--root` 없이 family 그래프를 파괴 | 정적 |
| ⑫ | 3 | `pii_terms` 부재 시 PII 를 볼 수 있는 명령만 죽고 `push` 는 통과 | 대조 |
| ⑬ | 3 | `_hints_regen` 이 마커 유실 시 카탈로그를 비우고 성공 문구 출력 | 정적 |
| ⑭ | 3 | `drift_warn` 이 4개 소비지 중 3곳에서 증발 | 대조 |
| ⑮~㉑ | 4 | 주석·문서의 사실 어긋남 7계열 | 대조 |

---

## 해소 상태 (2026-09-01 08시 KST · `plan_26090107` Phase 1 이후)

**8 해소 · 2 부분 해소 · 11 미해소.** 각 해소는 **음성대조**(일부러 깨뜨려 빨간불 확인)로 검증했다 —
교정이 성공하면 시험이 죽으므로, 통과만으로는 배선을 증명하지 못한다.

| # | 상태 | 무엇이 바뀌었나 · 검증 |
|---|---|---|
| ① | **미해소** | 게이트 배선은 Phase 4. 현행 자체검사 프로브가 항상 `create→seal` 이라 **이 우회를 구조적으로 못 본다**(감사 §이 감사 자체의 결함과 같은 계열) |
| ② | **해소** | 검증기가 `(code, message)` 튜플을 내고 판정은 **code 로만** 한다. 같은 입력에서 옛 판정 `True`(등급 뒤집힘) vs 새 판정 `blocking` 을 대조 실증. 미등재 부재코드가 조용히 통과하지 않는 tripwire 포함 — **4/4 PASS** |
| ③ | **부분 해소** | era gate(`LINT_ERA_EFFECTIVE_EPOCH`·`_tagger_epoch`) 제거 → 피검사자가 자기 검사를 끄는 경로가 사라졌고 이제 전 태그가 예외 없이 봉인 검사를 받는다. **footer 가 레시피 본문을 묶지 않는 문제는 미해소**(Phase 4) |
| ④ | **미해소 · 설계로 대체 예정** | `plan_26090107` D1.1 로 카탈로그 진실원천이 **원격 발행 태그**가 되면 "로컬 상태를 신뢰"하는 이 결함이 구조적으로 소멸한다. 현행 코드에서는 그대로 |
| ⑤ | **미해소 · 초기화로 소멸 예정** | 실측 재확인(개정 시점): `HINTS.md` 58행 · 미닫힘 주석 시작 4곳(135·136·144·145). 데이터 손상이므로 태그 초기화 시 함께 해소된다. **소실의 기전 쪽은 ⑬ 에서 닫았다** |
| ⑥ | **미해소** | `collect` 는 여전히 index+families 만 읽는다. `orphans` 가 검출은 하지만 침묵 누락 자체는 남아 있다 |
| ⑦ | **해소** | `enabled is True` 만 남기고, `None`(판정 대기)로 빠진 건수와 **태그명을 stderr 로 고지**한다. `False`(실제로 끔)와 `None`(모름)을 같은 취급하던 것이 거짓 음성의 기전이었다 — "없다 ≠ 모른다" |
| ⑧ | **해소** | 부분 보존 결함 교정: `families[*].sd_capability` 중 `source` 가 `judgment|manual` 인 것은 기계가 덮지 않는다. 라이브 왕복시험(판정 주입 → `--write` → 보존 확인)으로 검증 |
| ⑨ | **미해소** | `follow_up` 에 `--hf-repo`/`--model-path` 가 여전히 없다(`hint_tag.py:681-684`). Phase 1 에서는 `--allow-new-slug` 만 제거했다. **2줄 교정으로 닫을 수 있다** |
| ⑩ | **미해소** | `cmd_finalize` 의 태그 생성 → `_require_central` 순서 그대로. Phase 4 |
| ⑪ | **부분 해소** | 사람/Agent 확정 판정(`source: judgment|manual`) 멤버는 이제 보존된다(중복 제거 포함, 음성대조 검증). **카드파생 멤버 파괴와 `--root` 미지정 가드 부재는 미해소** |
| ⑫ | **미해소** | `push` 의 PII 스캔 호출 0회 그대로 |
| ⑬ | **해소** | `_hints_regen` 이 `bool` 을 반환하고, 파일·마커 부재를 **fail-loud 로 고지**한다. 마커 부재 시 기존 행을 지우지 않고 먼저 반환하므로 **파괴 자체가 일어나지 않는다**. 호출부 3곳(`cmd_finalize`·`cmd_index`·`cmd_reindex`)이 실제 결과대로 보고한다 — 3조건 × 2명령 **6/6 PASS** |
| ⑭ | **해소(등급 제거로)** | `drift_warn` 등급 자체를 삭제했다. 분류기가 4-튜플 → 2-튜플(`blocking`·`unverifiable`)로 축소되어 "받아놓고 안 쓰는 자리"가 없어졌다 |
| ⑮ | **해소** | `classify_evidence_problems` 전면 재작성 — docstring 이 실제 반환 2-튜플과 일치 |
| ⑯ | **해소** | 드리프트 통과 경로가 제거되어 분류표의 거짓 진술이 사라졌다 |
| ⑰ | **해소** | 없는 계약 개정(`계약 §5 개정 2026-08-20`)을 인용하던 주석이 그 분기와 함께 삭제됐다 |
| ⑱ | **미해소** | `index` 는 여전히 `--manifest` 를 받지 않는다(`hint_tag.py:1902-1905`). 모듈 주석·`completion_gate.py` 주석의 거짓 진술도 그대로 |
| ⑲ | **미해소** | 모듈 docstring 의 서브커맨드 열거는 여전히 실제(**11개**)와 다르다. `pin-legacy` 제거로 12 → 11 이 되어 **어긋남의 내용만 바뀌었다** |
| ⑳ | **미해소** | `cmd_reindex` 의 docstring 이 `_require_central(...)` 뒤에 있어 `__doc__ is None`(`hint_tag.py:1761-1763`) |
| ㉑ | **미해소** | `--cleanup` 정정이 나머지 3곳으로 전파되지 않은 상태 그대로 |

### 이 개정이 근거로 삼은 검증

| 검사 | 결과 |
|---|---|
| `runtime_selftest.py` | PASS (교정 도중 **실제 회귀 1건을 잡아냄** — 제거된 플래그를 프로브가 계속 쓰고 있었다) |
| E2E 전 명령 표면(격리 저장소) | **20/20 PASS** — `create→seal→index→verify→reindex→match→collect→--json→--sd-only→orphans` |
| `HINTS.md` 경로 (3조건 × 2명령) | 6/6 |
| 분류기 주입 차단 | 4/4 |
| 슬러그 충돌 대조 | 4/4 (역사적 분열 2건 재현 차단 · 신규 슬러그 통과) |
| AST 미정의 참조 (3파일) | 0건 (검사기 자체도 음성대조로 검증) |

> **E2E 가 닿지 못한 곳을 명시한다** — ① 의 우회(`create` 를 건너뛴 `seal`)는 프로브 구조상 보이지
> 않고, `push` 는 네트워크라 미실행이다. 20/20 은 **현행 형태가 동작한다**는 뜻이지 ①이 막혔다는
> 뜻이 아니다.

### 부수로 제거된 것 (감사 항목 밖 · 사용자 지시)

| 대상 | 근거 |
|---|---|
| `CANONICAL_SLUGS` + `canonicalize` + `--allow-new-slug` | 파생 가능한데 손으로 적은 것. 이 감사가 실측한 대로 **발행된 32 슬러그 중 27종이 이미 표 밖**이었다 — 표는 집행되지 않고 있었다. 철자 충돌 대조를 **발행된 태그에서 파생**하도록 대체 |
| `MANUAL_FAMILIES`·`KNOWN_CARDLESS` | 스크립트에 박힌 특정 LLM 모델명. 판정의 거처를 파일로 옮기고 보존 규칙을 붙였다 |
| SD 산문추론 정규식 3종 | ⑦ 거짓 음성의 원인. SD **능력** 판정은 인용과 함께 Agent 가 한다 |
| 레거시 핀 5종 · 드리프트 3종 · `cmd_pin_legacy` | 하위호환 전용 — 태그 전량 초기화로 소급 대상이 0 |

`classify()` 가 **"판정 불가"를 `manual`(사람이 선언함)로 라벨**하던 것도 함께 고쳤다(`unresolved` 분리).
카드가 없는 호스트에서 36/36 이 `수동` 으로 찍혀 마치 전부 사람이 승인한 것처럼 보였다 — 출처 표시가
거짓을 말하면 §결정론 규율이 집행 불가가 된다. 이 감사 항목에는 없던 것으로, Phase 1 실행 중 발견했다.

---

## 심각도 1 — 계약 §2 위협을 직접 관통

### ① 계약 §3 발행조건이 태그를 만드는 명령에 배선되어 있지 않다 (대조)

호출부 전량:

```
_require_serving_evidence  → hint_tag.py:648  cmd_create        ← 유일한 호출자
_require_perf_warning      → hint_tag.py:738  cmd_finalize      ← 반대쪽에만
completion_gate.py 의 'lite' 토큰(literal 제외) → 0건
```

계약 §3 의 조건 A(서빙 성공)·B(lite 정량지표)를 검사하는 `_require_serving_evidence` 가
**부작용이 스캐폴드 파일 하나뿐인 `create` 에만** 걸려 있다. `finalize`/`seal` 은
`--recipe <임의경로>` 를 받으므로 `create` 를 건너뛰는 데 비용이 0이다.

조건 A 는 `completion_gate.py` 가 부분적으로 받는다(`health_ok`/`oom_killed`/`functional_smoke_passed`).
**조건 B 는 저장소 어디에서도 받지 않는다** — `lite_included`·`lite_gen_tps_warm` 을 검사하는
코드는 `hint_tag.py:1147,1150` 뿐이고 그것이 `create` 안에 있다. 따라서 `SKILL.md` §2.1 의
*"A·B 가 없으면 manifest 를 만들 수 없다"* 는 **B 에 대해 거짓**이다.

**실패 시나리오**: 서빙 성공·스모크 통과로 A 충족(승격 게이트 통과) → lite 벤치 실패로 인증서에
`lite_included: false` → 발행자가 `create` 를 건너뛰고 레시피를 직접 저작해 `seal` 직행 → **통과**.
`seal` 이 도는 검사는 승격게이트·promotion_target 바인딩·footer 해소·린트·PII 뿐이며,
footer 해소는 인증서 **파일 바이트를 해시할 뿐 내용을 읽지 않는다.**

**회귀망이 이 우회를 볼 수 없는 이유**: `.claude/policies/runtime/runtime_selftest.py` 의
`_hint_publish_probe` 는 항상 `create → seal` 순으로 돌고 `create` 실패 시 조기 반환한다.
즉 **"create 를 건너뛴 seal"이 픽스처 모양에 존재하지 않는다.**

**처방**: `cmd_finalize` 의 `_require_hint_promotion_target` 직후에
`_require_serving_evidence("hint_finalize", manifest)` 배선(한 줄). 단
`_require_serving_evidence` 가 인증서 경로를 `<repo>/docs/_evidence/` 로 **손으로 적고**
`.resolve()`(심링크 추종)를 쓰므로, 배선과 함께 `manifest_path` 인자를 받아
`resolve_and_stat_evidence` 를 쓰도록 고치는 것이 맞다 — 기준 디렉터리는
`manifest.parent` 로 **파생 가능한데 손으로 적힌** 값이다(workflow.md 하드코딩 결함 칸).

### ② 판정이 사람이 읽는 메시지 문자열로 내려진다 — 데이터 주입으로 차단이 뒤집힌다 (실측)

감사자가 모듈을 적재해 `classify_evidence_problems` 를 직접 호출한 결과:

```
무해한 경로명   blocking=1  legacy=0  drift=0  unverifiable=0
주입된 경로명   blocking=0  legacy=0  drift=0  unverifiable=1
   ↑ footer 의 certificate_ref 값에 '_REF_ABSENT' 문자열을 포함시킨 경우

MALFORMED 를 MISSING 으로 읽는 술어(only_missing) = True
   ↑ footer 불량 라인 텍스트를 'HINT_EVIDENCE_BINDING_MISSING' 으로 둔 경우
```

`classify_evidence_problems` 는 `all("_REF_ABSENT" in p ...)` ·
`all("HINT_EVIDENCE_BINDING_MISSING" in p ...)` 같은 **substring 검사**로 등급을 정하는데,
그 메시지에는 **태그 저자가 통제하는 footer 값이 f-string 으로 보간**돼 있다.

강등의 대가가 크다 — `unverifiable` 은 차단이 아니고, `collect` 가 수신자에게
*"결함이라는 뜻은 아니다. 평소대로 쓰되"* 를 출력한다. 즉 **위조된 인증서 바인딩이
"이 체크아웃에 파일이 없을 뿐"으로 둔갑한다.**

두 번째 경로는 더 길다: MALFORMED 를 MISSING 으로 위장 → `cmd_pin_legacy` 가
MISSING/MALFORMED 를 `footer is None` 하나로 뭉개므로 **손저작 footer 가 "v1 빈티지"로 핀 등재**
→ 이후 `only_missing` + 핀 SHA 일치 → **`legacy_warn` = 경고만, push 통과**.

**같은 파일이 이 위험을 이미 경고한다.** `cmd_reindex` 주석: *"메시지 문자열을 파싱해 태그를
캐내지 않는다 — 포맷이 바뀌면 조용히 어긋난다."* 그런데 바로 아래에서 `_why[len(_t)+2:]` 로
접두를 슬라이스하고, 그 값을 만든 함수 전체가 substring 파싱이다.

부수 위험: 메시지 문구를 `"a != b"` 에서 `"observed=a expected=b"` 로 바꾸는 **표현상의 리팩터**만으로
`_DRIFT_SHA_RE` 가 어긋나 등재된 드리프트 승인이 조용히 무효화된다(정적).

**처방(저비용)**: 진입부에서 `^(?P<tag>\S+): (?P<code>HINT_[A-Z_]+) ` 로 파싱해 **코드를 접두에서만**
읽게 강제 + 미지 코드 거부. 정규식 한 줄로 위 두 강등이 닫힌다.
**처방(구조)**: `EvidenceProblem(code, tag, detail)` 로 코드와 표현을 분리.

### ③ 봉인 검사가 피검사자가 쓴 날짜로 켜지고 꺼진다 · footer 가 본문을 묶지 않는다 (대조 + 실측)

`unsealed_reason()` 은 `taggerdate` 가 `LINT_ERA_EFFECTIVE_EPOCH` 보다 앞서면 **아무것도 검사하지
않고 통과**시킨다. `taggerdate` 는 태그 오브젝트 내부 값이며 `GIT_COMMITTER_DATE` 로 자유롭게 설정된다.

**같은 파일이 정반대를 못박고 있다** — `_tag_is_pre_effective` 주석:
*"이 함수는 진단에만 쓴다. 자동 핀에 쓰지 않는다 — tagger 날짜는 태그 오브젝트 안에 있어
신규 위조 태그가 날짜를 소급해 빈티지를 참칭할 수 있다."* 규율이 파일 안에서 갈렸다.

이것이 결정적인 이유는 **evidence footer 9필드 어디에도 레시피 본문의 digest 가 없기 때문**이다.
manifest·certificate·identity·anchor 는 묶이는데 **배포되는 페이로드 자체가 안 묶인다.**
따라서 본문 진실성을 보는 검사는 `unsealed_reason` 하나뿐이고, 그것이 환경변수 한 줄로 꺼진다.

문턱의 두께 (실측):

```
린터 시대게이트 임계 이전(검사 면제) 48종 / 이후(검사 대상) 14종 / 총 62종
임계 ±1h 내 7종:  -409s ~ -394s  (hint/0.27.0/hy3/gb10x2 계열)
```

실제 발행이 문턱을 **초 단위로 스치고** 있다 — 게이트가 우연에 의존한다.

**보조 사실**: v2 태그 전량이 이 체크아웃에서 `*_REF_ABSENT` 로 떨어진다(증거가 배포되지 않으므로
설계대로다). 그러므로 digest 층은 수신자 평면에서 구조적으로 무력이고, `unsealed_reason` 이
**실질적으로 유일한 본문 방어**라는 사실의 무게가 여기서 결정된다.

**처방**: footer 에 `body_sha256` 추가(본문을 증거에 묶는다) + epoch 게이트를 **핀 목록 등재 여부**로
대체(사람 게이트 = `_tag_is_pre_effective` 가 이미 지키는 규율).

### ④ `index` 가 무검증으로 배포 카탈로그에 `status: "active"` 를 박는다 (대조)

`cmd_index` 는 `_require_central` 과 annotated 여부 두 가지만 본다.
승격게이트 ✗ · `_validate_hint_tag_evidence` ✗ · `unsealed_reason` ✗ · PII ✗ · 린트 ✗ ·
`--manifest` 인자 자체가 파서에 없고 `HINT_ACTION_FOR_CMD` 에도 없다.

**그런데 `SKILL.md` §2.3 이 규정한 정규 절차가 `index` 다.** 같은 태그를 `cmd_reindex` 는
`status: "unbound"` + 사유 + 경고로 격리하고 `collect` 가 *"서빙전략 근거로 쓰지 마라"* 를 붙이는데,
문서가 시키는 명령은 격리하지 않는 쪽이다. **격리된 태그를 `index --tag` 로 다시 넣으면
격리가 세탁되고 `unbound_reason` 이 사라진다.**

추가로 `anchor` 를 검증되지 않은 본문 텍스트에서 `re.findall(r"^(\w+):\s*(.+)$")` 로 캐낸다 —
손으로 `anchor: <임의 40hex>` 를 적으면 그대로 카탈로그 앵커가 된다. (`\w` 가 한글을 매치하는
탓에 footer 없는 실제 태그에서 캐낸 키가 `['완료']` 하나였다는 관측도 나왔다.)

**모듈 주석과 `completion_gate.py` 주석 양쪽이** *"index.json/HINTS.md 를 변경하는 모든 서브커맨드는
`--manifest` 를 요구하고 승격 게이트를 통과해야 한다"* 고 선언한다 — `index` 가 정면 위반이다.
(`SKILL.md` §2.2 는 승격게이트 면제를 **의도**로 적고 있으므로 면제 자체는 결함이 아니다.
결함은 **증거·상태 판정까지 함께 빠진 것**과 주석이 거짓이라는 것이다.)

---

## 심각도 2 — 이미 발생한 배포물 손상

### ⑤ `HINTS.md` 58행 중 17행이 렌더에서 소실된다 (실측)

```
brief 에 '<!--' 를 담은 행: 135, 136, 144, 145
  첫 발생 135행 → 다음 '-->' 는 152행
  ⇒ HTML 주석으로 먹히는 hint 행: 17 / 58
```

사슬:

1. 린터 L1 을 **지키게 하려고** 템플릿 맨 위에 경고 HTML 주석 3줄을 추가했다.
2. `_brief_of` 의 *"첫 비어있지 않은 줄 = brief"* 라는 **위치 규약**이 깨졌다
   (주석은 여전히 "템플릿 첫 줄 = `{{BRIEF}}`" 라고 적혀 있다).
3. `hints/index.json` 4개 항목의 `brief` 가 그 경고문이 됐다.
4. `_hints_row` 가 brief 를 **이스케이프 없이** 마크다운 표 셀에 넣는다.
5. 닫히지 않은 `<!--` 가 `<!-- hint-index:rows -->` 의 `-->` 까지 삼킨다.

메모리에 축적된 **"성공이 자기 검사를 깨뜨린다"** 의 정확한 사례다.

별개 원인(`cat-file -p` 가 오브젝트 전문을 반환)으로 `brief` 가 `object <sha>` 인 항목이 7건 더 있어,
**58행 중 11행의 brief 가 손상** 상태다.

### ⑥ `collect` 가 실재 태그를 침묵 누락한다 — 계약 §1 의 유일한 의무가 깨져 있다 (실측)

`collect --model gemma-4-E2B-it` 이 3종을 낸다. 그러나 `hint/0.27.0/gemma-4-E2B-it/sm_120` 은
실재하는 annotated 태그다(색인 미등재). 이런 태그가 **5건**이다.

`collect` 가 index+families 만 읽는 것은 결정론·토큰절약의 설계 의도이고, `orphans` 가
*"그 침묵을 소리로 바꾸는 자리"* 라고 docstring 이 말한다. 그러나 **수신자는 `orphans` 를 돌릴
이유를 모른다.** 태그 수와 색인 수를 비교하는 한 줄이면 `collect` 가 스스로 경고할 수 있다.

계약 §1 은 발행자의 의무를 **빠짐없는 수집** 하나로 규정한다. 그 의무가 지금 깨져 있고,
깨졌다는 사실이 수신자 평면에서 관측 불가하다.

### ⑦ `collect --sd-only` 가 거짓 음성을 낸다 (실측)

```
$ hint_tag.py collect --model qwen3.8-27b --sd-only
[collect] 해당 태그 없음.
```

그 family 태그 본문에는 *"MTP n=3 만으로 기준선 대비 2.18×"* · `num_spec_tokens=3` ·
`Loading drafter model...` 이 실려 있다.

원인: `tag_sd` 가 `--scan-sd` 시점 스냅샷(49종)이라 이후 발행분 **14종이 미수록**이고,
`_sd_brief` 가 미수록을 `-` 로 찍으며 `--sd-only` 필터가 falsy 로 탈락시킨다.
`-` 하나가 **"SD 없음"과 "색인이 낡음"을 구분하지 못한다** —
`docs.md` 가 명문화한 *부재와 결측의 구분* 규율 위반이며, 수신자의 1순위 질의에 대한 오답이다.

### ⑧ `sd_capability` 가 29/29 family 전량 `unknown` (실측)

```
family sd_capability.mode : unknown 29
tag_sd.mode               : mtp-internal 26 · unknown 20 · draft-external 3
tag_sd 커버리지            : 49 / 62 태그
```

tag 층 판정은 살아 있는데 family 층만 전부 비었다. 원인은 `bootstrap_families.py` 의
**부분 보존**이다 — `--scan-sd` 없이 `--write` 를 돌리면 `tag_sd` 는 이전 파일에서 승계하지만
(주석이 명시적으로 그렇게 짰다) `build()` 가 만든 `sd_capability` 는 **항상 기본값**이고 승계하지 않는다.
관측된 상태가 정확히 이 시나리오의 지문이다.

같은 근거("스캔하지 않은 실행이 이전 결과를 지우면 안 된다")가 한쪽에만 적용됐고,
그 결과 *"태그 본문을 열지 않고 SD 능력을 답한다"* 는 색인 필드의 설계 목적이 통째로 무력화됐다.

---

## 심각도 3 — 절차가 실행 불가하거나 파괴적

| # | 결함 | 상세 |
|---|---|---|
| ⑨ | **`create` 의 다음-단계 명령이 항상 실패** (대조) | `follow_up` 에 `--hf-repo`/`--model-path` 가 없는데 `cmd_finalize` 가 `require_derived_slug` 로 그것을 요구한다. 이 파일 스스로 *"create's emitted follow-up finalize command must stay literally executable"* 를 **닫힌 blocker 로 선언**했다. R1 슬러그 파생 도입 때 생성기만 갱신되지 않았다. `SKILL.md` §2.3·§2.4 의 `seal` 시그니처에도 같은 누락이 있다 |
| ⑩ | **`finalize` 가 태그 생성 뒤 권위 검사** (대조) | annotated 태그를 만든 **다음** `_require_central` 이 죽는다. 권위 미선언 환경에서 태그만 남고, 권위 선언 후 재실행은 `expect_absent` 로 막히며 `seal` 도 같은 이유로 막힌다. **탈출구 `git tag -d` 가 어느 안내에도 없다.** 승격게이트가 지킨 "거부 시 부작용 0" 규율의 유일한 예외이며, D3 법칙(*우회 잔재는 다음 배달의 차단 사유*)에 걸린다 |
| ⑪ | **`bootstrap --write` 가 family 그래프를 파괴** (정적) | `--root` 없이 실행하면 카드 스캔 루프가 한 번도 돌지 않아 전 슬러그가 단독 family 가 된다(복수-멤버 family 7 → 1, 자동판별 멤버 → 0). 에러도 백업도 대조 출력도 없다. 그런데 `collect` 가 스스로 안내하는 호출법에 `--root` 가 없고 `SKILL.md` 에도 없다. `git tag -l` 실패 시에도 같은 파괴가 일어난다(returncode 미확인) |
| ⑫ | **`pii_terms` 부재 시 PII 를 볼 수 있는 명령만 죽는다** (대조) | `finalize`/`seal` 은 `die`, `verify` 는 상시 FAIL, **`push` 는 영향 없음**(PII 스캔 호출 0회). 그 파일은 비추적이라 **모든 배포 클론의 기본 상태가 부재**다. `SKILL.md` §1 이 경고한 바로 그 우회(맨 `git tag -a`)의 다음 유인이 되며, 계약 §5 가 v2 에서도 유지한 "본문·tagger PII = 차단"이 배포 시점에 집행자를 갖지 못한다 |
| ⑬ | **`_hints_regen` 의 침묵 파괴** (정적) | `HINTS.md` 부재면 조용히 반환하고, 마커가 유실되면 기존 hint 행을 **전부 제거한 뒤 재삽입 지점을 못 찾는다**. 두 경우 모두 호출부는 *"index.json + HINTS.md 갱신 완료"* 를 출력한다. `verify` 도 `orphans` 도 `HINTS.md` 를 한 번도 읽지 않으므로 릴리즈 게이트가 빈 카탈로그를 보고 PASS 를 찍는다 |
| ⑭ | **`drift_warn` 이 3곳에서 증발** (대조) | `classify_evidence_problems` 의 4-튜플 중 3번째를 실제로 보고하는 곳은 `_require_all_hint_tags_evidence_valid` 하나뿐이다. `verify`·`reverify`·`reindex` 는 받아놓고 쓰지 않는다. 릴리즈 게이트인 `verify` 가 드리프트를 한 마디도 하지 않는다. 무명 튜플이라 잊어도 신호가 없다 |

추가로 `reverify` 는 `unverifiable` 판정도 버린다 — 증거를 대조할 수 없는 태그에 **경고 없이**
`last_verified = 오늘` 을 찍는다. 실패 신호(`anchor_reachable`)는 `_hints_row` 도 `collect` 도
렌더하지 않는 필드에 적히고, 성공처럼 보이는 스탬프만 갱신된다.

---

## 심각도 4 — 주석·문서의 사실 어긋남

이 프로젝트에서 주석은 결정 기록이다. 따라서 **주석이 코드와 어긋나면 스타일 문제가 아니라 거짓 증거**이며,
다음 사람이 그것을 근거로 판단한다.

| # | 위치 | 주장 | 실제 |
|---|---|---|---|
| ⑮ | `classify_evidence_problems` docstring | 반환 `(blocking, legacy_warn)` 2-튜플 | 4-튜플. 호출부 5곳 전부 4개로 언패킹 |
| ⑯ | 동상 분류표 | `forged / drifted → blocking` | 등재된 증거-보전 드리프트는 **통과**. "변조가 통과할 수 있나"를 감사하는 사람이 이 문장을 읽고 "아니오"라고 답하게 된다 |
| ⑰ | `hint_tag.py` 드리프트 분기 | `계약 §5 개정 2026-08-20` 인용 | **그런 개정이 없다.** 계약은 지금도 `drifted = 차단`. 코드가 정본 계약을 위반하는데 주석이 리뷰어의 대조를 막는다 |
| ⑱ | 모듈 주석 + `completion_gate.py` 주석 | *"index.json/HINTS.md 를 바꾸는 모든 서브커맨드가 게이트를 탄다"* | `index` 는 `--manifest` 자체가 없다. 두 파일이 같은 거짓을 재진술한다 |
| ⑲ | 모듈 docstring | 서브커맨드 6개 열거 | 실제 12개. 수신자 정본 진입점 `collect` 와 대사기 `orphans` 가 누락 — 도구가 있는데 없다고 판단하게 만든다 |
| ⑳ | `cmd_reindex` | 설계 근거 docstring | `_require_central(...)` **뒤에** 있어 `__doc__ is None`. blocker-2 교정의 핵심 근거가 introspection 에서 사라진다 |
| ㉑ | 템플릿 헤더 · `REQUIRED_SECTIONS` 주석 · `SKILL.md` §4 | *"49/49 태그에 헤딩이 0개 → 템플릿은 무력하다"* | 진범은 `git tag -a -F -` 의 기본 `--cleanup=strip`. `cmd_finalize` 주석이 재현까지 해서 인정했으나 **그 정정이 나머지 3곳으로 전파되지 않았다** |

⑮·⑯·⑰·⑱ 은 **감사 방해**로 분류한다 — 보안 판정을 뒤집는 거짓 진술이다.
㉑ 은 삭제가 아니라 **반증 병기**(`SUPERSEDED-IN-PART` 와 같은 논리)로 보존하는 것이 규약에 맞다.

---

## 방어층 실효 분석

이 게이트의 방어층은 4개다. 현 체크아웃의 실효를 합치면:

| 층 | 설계 | 현 실효 |
|---|---|---|
| ① 승격 게이트 (`completion_gate`) | 계약 §3 A·B 강제 | **A 만**. B(lite)는 검사자가 `create` 밖에 없음 |
| ② evidence-binding digest | manifest·인증서·identity 위조 차단 | 수신자 평면에서 **구조적 무력**(증거 미배포 = 설계대로) |
| ③ `unsealed_reason` 린트 | 도구 미경유 태그 검출 | **taggerdate 로 꺼짐** · `push`/`index` 에 미배선 |
| ④ PII 스캔 | 배포물 PII 차단 | 소스 파일이 비추적 → **배포 클론에서 항상 부재**, `push` 에는 애초에 없음 |

**남는 실효 방어가 사실상 ③ 하나이고, 그마저 배포 명령과 색인 명령에 연결되어 있지 않다.**

---

## 체계적 진단

발견의 분포가 진단이다. **로직 오류가 아니라 배선 공백**이 대부분이며, 그 공백의 형태가 반복된다:

- 호출자 0 (①)
- 게이트가 문의 반대쪽 (④ · ⑫)
- 필수 입력이 생성기에서 누락 (⑨)
- 검사기가 자기 산출물을 안 봄 (⑬)
- 값을 받아놓고 버림 (⑭)
- 보호를 한쪽에만 적용 (⑧)

그 아래에 공통 원인이 하나 있다. **교정이 매번 "함수 하나로 모으기"에서 멈췄고, 그 함수들이
주고받는 값은 여전히 무형식 `dict`/`str`/`tuple` 이다.** `_binding_artifact_path` 단일화,
`classify_evidence_problems` 단일 권위화는 모두 "같은 판정이 두 곳" 사고의 옳은 사후 교정이었지만,
**소유의 단위가 함수가 아니라 타입이었어야 했다.** 값이 dict/str 로 남아 있는 한 "같은 개념 여러 곳"은
함수 경계를 지나 **접근 방어 스타일 · 메시지 문구 · 표기 관습**의 층위에서 재생산된다.

실증 3건:

- **접근 스타일**: `perf_waiver` 를 한 곳은 truthy 로, 한 곳은 dict 로 읽는다 →
  `perf_waiver: true` 하나로 한쪽은 열리고 한쪽은 트레이스백.
- **메시지 문구**: ② 의 등급 강등 — 판정 데이터가 표현 계층을 경유한다.
- **표기 관습**: `topology` 자유 문자열이 `'Multi …'` 하나로 `TOPO_DIR` 을 발산시키고
  (`git checkout <tag> -- output/single/build_patches/` 라는 잘못된 재현 명령이 배포 본문에 박힌다),
  `'TP=2'` 표기로 **TP 교차검증이 조용히 생략**된다. 검사를 끄는 방법이 "표기를 조금 다르게 쓰는 것"이다.

`workflow.md` 의 *"단일 소유가 불가능하면 교차검증이 차선"* 이 여기에 그대로 적용된다.

---

## 권고 (노력 대비 차단력 순)

| 순위 | 조치 | 근거 |
|---|---|---|
| 1 | `cmd_finalize` 에 `_require_serving_evidence` 배선 (+ 인증서 경로를 `manifest.parent` 기준 하드닝 resolver 로) | 계약 §2 주 방어선이 실제 발행 경로에 없다 |
| 2 | `classify_evidence_problems` 가 코드를 **접두에서만** 읽게 + 미지 코드 거부 | 실측된 등급 강등 2건이 정규식 한 줄로 닫힌다 |
| 3 | `cmd_index` 에 `_validate_hint_tag_evidence` + `unsealed_reason` 배선, `reindex` 와 **같은** status 판정 사용 | 격리 세탁 경로 차단 (승격게이트 면제는 유지 가능) |
| 4 | `_brief_of` 결과 검증 + `_hints_row` 이스케이프 | 이미 깨진 배포물의 복구 |
| 5 | footer 에 `body_sha256` 추가 + epoch 게이트를 핀 등재 여부로 대체 | 본문을 증거에 묶고, 피검사자 통제 필드를 게이트에서 제거 |
| 6 | `cmd_reindex` 의 status 승계에서 파생 상태 제외 | 격리 신호의 영구 노후(끈적임) 방지 |
| 7 | `bootstrap` — `--root` 부재 fail-loud · `families` 승계·병합 · `--write` 제거 보고 | 추적 산출물의 침묵 파괴 방지 |
| 8 | 주석 정정 7계열 (⑮~㉑) — 삭제가 아니라 **반증 병기** | 거짓 증거 제거 |

### 교정 시 필수 규율

1·3 번은 **음성 대조**(위반 태그를 만들어 실제로 차단되는지) 없이 "배선했다"를 주장할 수 없다.
회귀망은 이미 존재하므로(`runtime_selftest.py` 의 `_hint_repo`/`_hint_publish_probe`),
**`create` 를 건너뛴 `seal`** 모양의 픽스처를 추가하는 것이 1번의 회귀망이다.
현행 픽스처는 항상 `create → seal` 순서라 이 우회를 구조적으로 볼 수 없다.

또한 `verify_distribution.py` 의 셀프테스트 열거에 hint 평면이 없다 —
`terraforming_node` 2종 · `adversarial-benchmark` 2종 · `vllm-recipe-explorer` 3종은 등재돼 있고
hint 는 `gitless_hint_match`(rc 확인만)뿐이다. 등재가 곧 배선이다.

---

## 이 감사 자체의 결함 (기록)

감사자가 최초 조사에서 **"hint_tag.py 를 검증하는 테스트가 0건"이라고 잘못 판정**했다.
실제로는 `runtime_selftest.py` 에 H0(단위 조합표) + H1~H5(E2E) 회귀와 2026-08-24 음성대조 기록이 있다.

원인은 저장소 전역 `grep` 이 `hint_tag` 를 담은 **27개 파일 중 5개만** 반환했고 감사자가 그것을
전수로 취급한 것이다. 그 틀린 기준선이 5개 에이전트 전부에 전달됐고 그중 하나의 결론이 오염됐다.
`pr-test-analyzer` 만이 지시대로 기준선을 **반증**했다.

이 사건은 본 감사가 다루는 결함과 같은 계열이다 — **필터된 결과를 전수로 취급한 침묵 누락.**
교훈: 부정 주장("~가 없다")은 도구 하나의 출력으로 확정하지 않는다.

---

## 최종 판정

> ⓘ 이 판정은 **2026-09-01 06시 감사 시점** 기준이다. 이후 8건 해소·2건 부분 해소됐다 —
> §해소 상태 참조. 판정 자체를 뒤집은 것이 아니라 처방이 집행된 것이므로 본문은 그대로 둔다.

1. **hint 레이어의 설계는 정당하고 정교하다.** 등급 분류(v1 빈티지 면제 · 수신자 평면 부재 강등 ·
   승인 드리프트 동결), 권한 비대칭, `--cleanup=verbatim` 교정, `_SECTION_ANCHOR` 의 무손실 증명은
   모두 실제 사고에서 도출된 모범 사례다.

2. **그러나 배포 전 상태로는 승인할 수 없다.** 계약 §2 가 지목한 유일한 위협에 대한 실질 방어가
   우회 가능하고(①), 판정이 데이터 주입으로 뒤집히며(②), 마지막 검출기가 피검사자 통제 필드로
   꺼진다(③). 셋은 각각 독립적으로 "허위 서빙 주장의 배포"를 허용한다.

3. **이미 발생한 손상은 즉시 복구 가능하다.** ⑤~⑧ 은 데이터·렌더 층 결함이며 태그 오브젝트
   자체는 무결하다. 권고 4번과 `reindex` 재실행으로 회복된다.

4. **권고 1~3 은 각각 수 줄이다.** 이 감사에서 가장 값싼 조치가 가장 큰 차단력을 갖는다 —
   결함이 로직이 아니라 배선이기 때문이다.

---

> 이 감사는 read-only 로 수행되었으며 대상 파일과 저장소 상태를 변경하지 않았다.
> 실측 항목은 감사 시점(2026-09-01 06:00 KST · 커밋 `d9fbdfb` · 브랜치 `single-node`)의
> 로컬 상태에 대한 관측이며, 태그 62종·색인 58종을 기준으로 한다.
> 정적 항목은 코드 독해 결과로, 재실행으로 확증하지 않았다.
