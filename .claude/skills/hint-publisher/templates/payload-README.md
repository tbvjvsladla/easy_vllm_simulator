# hint 페이로드 — `{{TAG}}`

> 이 커밋은 hint 태그 **하나의 페이로드**다 — 프로젝트 본체가 아니다(본체는 `single-node` · `multi-node` 브랜치).
> 태그의 zip(archive) 하나가 곧 이 셀의 지도 · 서사 · 재현 키트다. 형식 `{{FORMAT}}` · 태그 이름 문법 `{{GRAMMAR}}` · 생성 `{{GENERATED_UTC}}`.

{{BANNER}}

{{VERDICT}}

## 이 태그 이름 읽는 법

{{NAME_GUIDE}}

## 읽는 순서

| 순서 | 파일 | 담긴 것 |
|---|---|---|
| 1 | `00-hint.md` | 지도 — 요약 · 유효맥락 · 벽 지도 요약 · 결정론 해소값 · 서빙 노브와 값의 지위 · 재검증 · 메타 · 이름 꼬리 |
| 2 | `02-narrative.md` | 계보 서사 — 출발점 · 벽과 해소 · 기각된 시도 · 오진과 정정 · 값의 이력 · 되풀이하지 말 것 · 열린 물음 |
| 3 | `01-artifacts.md` | 산출물 — 적용 판정 · 슬롯별 적용 사유 · 값의 지위표 · 재현 절차{{SUB_RECIPE_ROW}} |
| 4 | `03-benchmark.md` | 측정 — 결정론 표 · 부하 곡선 · 측정 구성 · 측정 명령 원문 · like-with-like |
| — | `PAYLOAD.json` · `LINEAGE.json` · `PROVENANCE.json` | 기계 사실(명명 축별 출처 · 판정 포함) · 서사가 읽은 계보 문서 **요약**(stem · 역할 · 날짜 · 발췌 수 · 봉인 출처 sha256) · 앵커 |
| — | `artifacts/<슬롯>/` | 재현 실물 — 빌드 때 적용된 것은 전부 싣고 파일마다 **검증 표시**(`verified` · `generated-unverified`)와 **관련성**(`required` · `inactive-inferred` · `unknown`)을 단다 |

## 슬롯 → 빌드 컨텍스트

zip 의 슬롯 폴더는 Dockerfile · compose 가 기대하는 빌드 컨텍스트(`output/<토폴로지>/`) 자리와 이름이 다르다 — 옮길 자리는 아래와 같다
(파일별 전체 표는 `01-artifacts.md` §1.4 · 원천 = 실린 Dockerfile 의 COPY · compose 의 env_file · volumes).

{{CONTEXT_MAP}}

## 텍스트의 세 층

- `<!-- FACT:<id> -->` … `<!-- /FACT:<id> -->` 안쪽은 기계가 증거에서 **파싱만** 한 사실이다(합성 ✗ · 재계산 ✗). 값 옆의 출처 열이 그 값을 낸 파일 · 명령이다.
  `03-benchmark.md` §3.2 의 부하 곡선은 `<!-- BENCH_SECTION -->` 다음 줄부터 다음 챕터 헤딩 직전까지가 기계 렌더다 — 원천 리포트를 가진 쪽은 `render_bench_section.py --verify --section 03-benchmark.md --report <리포트>` 로 diff 0 을 다시 확인할 수 있다.
- `> 이 절이 답하는 질문: …` 아래는 발행 Agent 가 계보 문서를 읽고 쓴 산문이다. 질문 줄은 발행 때 봉인된 기재 지시의 요지다.
- `artifacts/` 파일 첫 줄의 `⚠ generated-unverified — … · 생성: <renderer|agent>` 는 **실행 검증되지 않은** 생성물 표시다(native 셀의
  Docker 형 compose 등). 같은 표시가 `PAYLOAD.json` 슬롯 파일 기록과 `01-artifacts.md` 의 `검증` 열에 있다 — 발행 전에 셋의 일치를 대조했다.
- `> [원문] <문서 stem> §<절>` 인용은 출처 문서(기계 치환 후)의 **글자 그대로**다 — 발행 전에 린터가 원문과 대조했다. 치환 자리표시: `<manifest.<필드>>` · `<repo>` · `<home>` · `<node:<역할>>` · `<priv-ip>` · `<abs-path>` · `<host>` · `<redacted>`.

## Agent 가 읽는 법 — hint-event 블록

` ```hint-event ` 블록은 산문과 같은 사실의 기계 표면이다. 제한 YAML: 한 줄 `키: 값` · 리스트는 `[a, b]` 인라인만 · `#` 이후 주석.

{{EVENT_KINDS}}

- 넘은 벽의 순서: `02-narrative.md` 에서 `kind: wall` 블록을 위에서부터 읽는다(00 §0.3 이 그 요약표다).
- 값의 지위: `01-artifacts.md` 의 `kind: value-status` 블록 — 지위가 `tuned` 가 아닌 값은 이 셀에서 조정된 적이 없다.

{{LEGEND}}

## zip 사용법

- 태그의 zip(또는 `git archive <태그>`)을 풀어 추적 트리 **밖**(예: `seed/hints/<태그 경로>/`)에 둔다 — 추적 트리를 오염시키지 않고 그라운딩 자리로 쓴다.
- 이 브랜치의 트리는 발행마다 allowlist 로 **새로 짓는다** — N 번째 archive 에 이전 태그의 파일이 딸려 오지 않는다(이력은 커밋 · 태그로 남는다).
- 성능 수치를 비교할 때는 `03-benchmark.md` 의 측정 구성 · like-with-like 부터 본다 — 조건이 다르면 같은 모델 · 같은 HW 라도 수치가 몇 배씩 갈린다.
- 옛 문법(v6 · 4·5세그먼트 · 노드축 없는 arch) 태그는 읽기 전용으로 남아 있다 — 이 형식(`{{FORMAT}}`)은 신규 발행분부터다. 옛 태그는 교정·리콜하지 않는다.
- 태그 annotation 끝의 증거 주소(`bench_ref` · `manifest_ref`)는 **발행 저장소의 로컬 경로**다 — 받는 쪽에서 해소하는 대상이 아니다.
