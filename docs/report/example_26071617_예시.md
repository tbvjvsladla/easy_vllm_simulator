# report/ — 배포자 대상 공지 채널 (6번째 문서형)

> 이 `example.md` 는 **추적되는 스켈레톤**이다(폴더 구조 + 작성 규칙). 그리고 이 폴더는 **다른 5종과 달리
> 산출물도 함께 추적·배포된다**(아래 §추적 예외). 근거: `docs/plan/plan_2026071617_1`.
> 참고 예제: `seed/rtxpro6000-benchmark-explorer.html`(비추적 seed — **배포본엔 부재 가능**).

## 역할 (5종과 직교 — 작업이력이 아니라 아웃바운드 공지)

메인테이너가 **배포자(이 레포를 클론해 쓰는 사람)에게 알리고 싶은 내용**을 자유롭게 싣는 채널.

| | 기존 5종 (plan·devlog·testlog·simlog·benchmark) | **report/** |
|---|---|---|
| 저자 | 에이전트(작업 중 발행) | **사람(메인테이너)** — 자유 발행 |
| 방향 | 내부(작업이력·증거·자기개선 루프 입력) | **외부**(배포자에게 나가는 공지) |
| 추적 | ✗ 비추적(`docs/*/*`) — example.md 만 | **✓ 추적**(산출물 포함 — 배포돼야 도달) |
| 트리거 | 스킬·워크플로 루틴 | **없음**(루틴·자동발행 ✗ — 사람이 내킬 때) |
| 판정 | plan=계획 / testlog=판정 / benchmark=inform-only | 판정 평면 아님(공지) |

∴ **wiki-desk 색인 대상이 아니다**(작업이력 그래프의 노드가 아님) · **서브노드로 전파되지 않는다**
(메인 전용 평면 — `references.md` 와 동형).

## 추적 예외 (⚠ 정합 함정 — 지우지 말 것)

`.gitignore` 의 `docs/*/*` 는 작업문서를 무시하지만, report 산출물은 **`!docs/report/*` 로 재포함**한다.

- **사유**: 무시된 파일은 클론에 안 실린다 = **아무도 못 알린다** → 폴더의 존재 목적과 모순.
- **이것은 docs.md §4 "새 폴더마다 전용 gitignore 규칙 추가 금지"의 명시 예외다.** 그 금지는 *산출물을
  무시*하려는 폴더(simlog·benchmark)가 `docs/*/*` 로 이미 커버되니 중복 규칙을 막으려는 것 —
  report 는 **요구가 정반대(추적)** 라 전용 예외가 유일한 수단. **정합 위반으로 오인해 제거 ✗.**
- **대가**: 추적물이라 `docs/` 의 *gitignore-persist 로 브랜치 자동 통합* 성질을 **못 받는다** →
  빌딩블럭(`CLAUDE.md`·`.claude/`)과 동형으로 `.claude/skills/upstream-version-watch/scripts/sync_branches.sh` 수동 동기화로 양 브랜치 동일성 유지.

## 명명 규칙

```
docs/report/<주제-슬러그>.<html|md>
```
- **kebab-case 주제 슬러그** · **날짜·seq 없음**(다른 5종의 `<type>_<YYYYMMDDHH>_<seq>_` 미적용).
  근거: report 는 README 처럼 **갱신·덮어쓰기로 최신본 1개**를 유지하는 성격 — 시점 기록이 목적이 아니다
  (benchmark 가 대상조합 키로 date 명명을 벗어난 것과 동형: *산출물 성격이 명명을 정한다*).
- 예: `rtxpro6000-benchmark-explorer.html` · `gb10-serving-guide.html`

## 형식 — 단일파일 반응형 HTML **권장**(강제 아님)

다양한 형태의 HTML 을 자유롭게 배포할 수 있다. 아래는 참고 예제에서 관측된 성질이며 **권장 기본값**이다
(예제 1건을 넘어선 투기적 규정은 두지 않는다 — 필요해지면 그때 구체화).

- **self-contained 단일 파일**: CSS·JS 를 문서 내 인라인, 데이터도 문서 내 리터럴(예제는 `const M = [...]`).
  외부 CDN·웹폰트·이미지·`fetch` **0** → 배포자가 파일 하나만 열면 그대로 동작(에셋 폴더 불요).
- **반응형**: `<meta name="viewport" content="width=device-width, initial-scale=1">` +
  `@media` 로 좁은 화면에서 grid 붕괴(예제: `max-width:860px` → 1열) + 넓은 표는 `overflow-x:auto` 로
  **자체 스크롤**(페이지 가로 스크롤 ✗).
- `<html lang="ko">` · `<meta charset="utf-8">` · `<title>` 명시.
- **출처 명기**: 수치를 실으면 어디서 나온 계측인지 footer 에 기록(예제: adversarial-benchmark full 스윕 +
  soak CSV). 참조 체인은 `.claude/rules/docs.md` §3.

## PII 금지 (추적·배포물이므로 필수)

report 는 **클론에 실려 나가는** 파일이다 — 노드 IP·호스트명·NAS 실경로·계정 등 환경 구체값을 박지 않는다
(헌법 §포인터 원칙). 시각화는 호스트 정보가 섞이기 쉬우니 특히 주의.

- 게이트: `.claude/skills/upstream-version-watch/scripts/smoke_clone.sh` **A4** 가 추적/비무시 트리를 **확장자 무관 전수 grep** 하므로
  `.html` 도 자동 커버된다(`.claude/pii_terms.txt` 리터럴 + private-IP 패턴).
- HW **스펙**(GPU 모델·VRAM·측정치)은 PII 가 아니다 — 예제처럼 자유롭게 실어도 된다.
