# divergence 의미 분류

- 정본: plan 26111805 「보존 계약」("divergence의 의미 분류는 c20이 더한다"), interview D3·D8. 쓴 cell: c20-contract.
- 형식 정본은 캠페인 T의 FORMAT.md 「divergence.md」다. 기계 표 머리 `| id | kind | path | before | after | reason | approval_key |`와
  `kind` 값(`golden`|`parity`|`output`|`fixed-surface`)은 그대로 둔다.
- `kind`와 `meaning`의 짝 규칙은 T가 검사한다(오케스트레이터 몫, g-c0 검토 1회차 V13). 짝 규칙은 `strictness` ⇒ `output`, `fixed-surface` ⇔ `surface-change`, `defect-fix` ⇒ `golden`·`parity`·`output`이다.
  이 문서와 `types.md`는 그 규칙과 같게 쓴다. 머리말 낱말 목록을 FORMAT에 넣을지는 오케스트레이터가 정한다.
- 출력 호환 수준(L1 바이트 고정·L2 값 고정·L3 자유)은 `types.md` 1절이다.

## 넣는 방식 — `reason` 머리말

기계 표의 열을 늘리지 않는다(머리가 바뀌면 지금 표 파서가 거절한다). 대신 `reason` 칸을 `<meaning>: <설명>`으로 쓴다. `<meaning>`은 아래 닫힌 목록의 한 낱말이다.
`residue_keep.tsv`의 `reason = <category>[: 설명]`과 같은 관례다. 검사기가 생기면 머리말이 목록 밖이거나 `kind`와 맞지 않는 행을 실패로 본다.

## 의미 분류

| meaning | 무엇이 달라졌나 | 허용 `kind` | 출력 수준 | 행에 꼭 적을 것 | 승인 묶음 |
|---|---|---|---|---|---|
| `cpython-semantics` | Python 의미 재현을 버려서 생긴 차이: 문자열 코드 포인트·서로게이트, `repr`, BigInt·4300자리 상한, float `repr`, powersort 비교 순서, `json.dumps` 바이트, argparse 접두 축약·오류 순서, CPython Unicode 15 표 | `golden`·`parity`·`output` | L2·L3 | 버린 Python 의미의 이름과 그 의미를 쓰던 소비자가 없다는 근거(`consumers.tsv` 행 또는 "소비자 없음") | 크레이트·골든 묶음당 한 행 가능 |
| `wording` | usage·help·오류 문구와 배치만 다름 | `golden`·`parity` | L3 | 바뀐 골든 경로 목록(`;`), 소비자가 문구를 읽지 않는다는 근거 | 크레이트당 한 행 가능 |
| `layout` | 값은 같고 바이트 배치만 다름: JSON 공백·구분자·키 순서·`\uXXXX` 이스케이프·float 지수 표기 | `golden`·`parity` | L2 | 파싱한 값이 같다는 확인 방법(예: 두 출력을 JSON으로 읽어 비교한 명령) | 크레이트당 한 행 가능 |
| `yaml-1.2` | YAML 1.1 해소를 1.2 core schema로 바꿔 생긴 차이: `yes/no/on/off`, 타임스탬프, 앞자리 0·60진 수. 입력 거부(중복 키·태그·순환·비유한)는 `strictness`이고, `<<` 병합은 h15로 지금 값을 지킨다(`types.md` 5절) | `output`·`golden`·`parity` | L2 | `yaml_inputs.tsv`의 해당 행(경로)과 처분(이전·퇴역) | 입력 파일 묶음당 한 행 |
| `strictness` | 전에 받던 입력을 이제 거부: `NaN`·`Infinity` JSON, 비UTF-8 텍스트 인자·값, 범위 밖 정수, 중복 키(derive 구조체 포함), 순환 YAML, core 밖 YAML 태그, 비문자열 YAML 키, 반복 플래그 | `output` | L1(종료 코드)·L3(문구) | 거부 때의 종료 코드와, 저장소 안 그 입력을 만드는 생산자가 없다는 근거 | 사례당 한 행 |
| `surface-change` | 고정 표면 자체를 바꿈: 서브커맨드·플래그 이름, 종료 코드 의미, JSON 키·타입, reason code, 파싱되는 줄, stdout/stderr 분리, 런처 환경 변수 | `fixed-surface` | L1·L2 | 같은 cell에서 갱신한 소비자 커밋, 원장 행, 바뀐 `consumers.tsv` 행 | 변경당 한 행(묶지 않는다) |
| `retired` | 검사·시험·입력을 없애고 대체하지 않음(오라클 시험, 1.1 픽스처, Python 흉내 자기검사) | `output`·`golden`·`parity` | — | 없앤 대상과, 같은 성질을 지키는 다른 시험(있으면 `test_delta.tsv` 행) | 대상 묶음당 한 행 |
| `defect-fix` | 이전 동작이 결함이었다(재현 근거가 있는 버그) | `golden`·`parity`·`output` | 모두 | 결함 재현 근거(시험 이름·입력)와 새 동작을 고정한 시험 | 사례당 한 행 |

## 규칙

1. `kind = fixed-surface`이면 `meaning`은 `surface-change`다. 거꾸로 `meaning`이 `surface-change`이면 `kind`는 `fixed-surface`다.
   다른 `meaning`(결함 수정 포함)이 고정 표면을 건드리면 행을 둘로 나눈다: 고정 표면 쪽은 `fixed-surface`/`surface-change` 행, 나머지는 원래 의미의 행이다.
   그래서 `defect-fix`의 허용 `kind`에 `fixed-surface`는 없다.
2. 같은 차이가 두 의미에 걸치면 사람에게 더 무거운 쪽을 쓴다: `surface-change` > `strictness` > `defect-fix` > `yaml-1.2` > `cpython-semantics` > `retired` > `layout` > `wording`.
3. `wording`·`layout`은 소비자가 없다는 근거가 있을 때만 쓴다. 셸 소비자가 grep하는 줄이 바뀌었으면 `surface-change`다(`consumers.tsv` `reads`에 `stdout-text`·`stderr`).
4. 묶음 행(`approval_key` 하나에 여러 골든)은 `path`에 크레이트 디렉터리나 골든 폴더를 적고, `before`·`after`에 대표 사례 하나, `reason`에 전체 목록을 둘 위치(cell 폴더 파일)를 적는다.
5. 시험을 없애거나 바꿔 통과 수가 줄면 divergence 행과 별개로 `test_delta.tsv` 행이 필요하다(형식은 FORMAT.md). 같은 `approval_key`를 두 파일에 함께 쓰지 않는다.
6. 전에 받던 입력을 새로 거부(종료 2)하는 것은 저장소 안에 그 입력을 만들거나 넘기는 생산자·소비자가 없으면 `output`/`strictness` 한 행이고, 있으면 `fixed-surface`/`surface-change`다(`types.md` 1절, 4회차).
   기대값을 거부로 바꾼 퇴역 픽스처(`types.md` 5절)는 시험을 빼지 않으므로 `test_delta.tsv`가 없다.

## 예

| id | kind | path | before | after | reason | approval_key |
|---|---|---|---|---|---|---|
| d1 | golden | .claude/rust/policy-gate/checks/data/dn/ | `usage: <옛 prog 이름> …` | `Usage: doc_naming …` | wording: clap 도움말로 바뀐 골든 3개(목록은 cell 폴더 golden_list.txt), 소비자 없음(consumers.tsv doc_naming 행) | dn-help |
| d2 | output | output/single/manifest.yaml | `on` → bool | `on` → 문자열 | yaml-1.2: yaml_inputs.tsv 처분 이전, wd-yaml 3자 대조 equal | m-yaml12 |
| d3 | fixed-surface | .claude/policies/runtime/completion_gate.sh | `--json-out` | `--json-output` | surface-change: 소비자 3곳 같은 커밋 갱신, 원장 행 기록 | cg-flag |

위 표는 예시다(실제 행이 아니다). 이 파일은 캠페인 `divergence.md`가 아니므로 approvals 검사가 읽지 않는다.
