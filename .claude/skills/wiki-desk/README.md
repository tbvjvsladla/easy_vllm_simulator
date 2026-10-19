# wiki-desk

> **에이전트가 기억에 기대어 답하지 않고, 프로젝트 원문의 위치와 근거 줄범위로 돌아가 읽게 하는 지식 사서 스킬입니다.**
>
> 배포 버전 **3.0.0 (TypeScript)** · 브랜치 `wiki-desk_TS` · Node.js 24 · Claude Code 프로젝트 스킬용 전체 폴더 패키지

## 브랜치 안내 — 같은 사서, 두 구현

| 브랜치 | 구현 | 대상 | 상태 |
|---|---|---|---|
| `wiki-desk_PY` | Python 3.10+ · PyYAML | Hermes · Codex · Claude Code 공통 seed, 설치기·이관·형식 교정 포함 | 1.1.0 |
| `wiki-desk_TS` (이 브랜치) | TypeScript (Node.js 24, 런타임 의존성 0) | Claude Code 프로젝트 스킬, 포인터 카드 + 결정론 검색 | 3.0.0 |

이 판은 1.1.0을 TypeScript로 **재설계**한 것입니다. 단순 번역이 아니라 카드 형식·검색 알고리즘·소유권 기록 위치가 바뀌었습니다. 계약 파일·위키 배치·명령이 달라 **한쪽 설치본을 다른 쪽 명령으로 관리할 수 없습니다.**

### 3.0.0에서 바뀐 것 (2.0.0 설치본은 재구축 필요)

- **카드 키 변경(breaking)**: 원문 frontmatter 보존 키가 2.0.0의 프로젝트 전용 namespace 키(번호 접두 키와 그 `_unknown` 짝)에서 프로젝트 중립 이름 `source_meta`/`source_meta_unknown`으로 바뀌었습니다. 호환 분기는 두지 않았으므로 2.0.0으로 만든 카드는 3.0.0에서 `card.invalid`로 보고됩니다.
- **배포 위생**: 특정 프로젝트 식별자를 코드·문서·시험에서 제거했고, `scripts/sanitize-check.ts`(사이트명·개인 경로·사설 IP·프로젝트 식별자 0건)와 `PACKAGE_MANIFEST.json`(파일별 sha256)을 추가했습니다. 예제 계약은 합성 예시입니다.
- **2.0.0 설치본 이행**: 스킬 폴더를 3.0.0으로 교체한 뒤 `remove --apply` → `sync --apply`로 카드를 다시 만듭니다. `remove`는 사람이 고친 카드·`contract.json`을 보존하므로, 남은 카드에 이전 키가 있으면 수정분을 옮긴 뒤 직접 지우고 다시 sync합니다.

## 해결하려는 문제 — "아마 그렇게 결정했을 것"과 "이 문서 41~58줄에 그렇게 적혀 있다"는 다릅니다

프로젝트가 길어지면 결정·승인·결과가 여러 문서에 흩어집니다. 에이전트에게 "예전에 이거 어떻게 정했지?"라고 물으면 그럴듯한 답이 나오지만, 어느 원문에 근거했는지, 그 결정이 나중에 뒤집혔는지는 따로 확인해야 합니다.

wiki-desk는 그 확인을 돕는 **사서**입니다. 원문을 복사하거나 LLM으로 요약해 새 정답지를 만들지 않습니다. 원문은 제자리에 두고, 원문마다 "어디 있고 어떤 종류·권위인지"만 적은 **포인터 카드**를 만듭니다. 질의하면 후보 문서와 함께 **근거 구간 `경로:시작줄-끝줄`**을 돌려주고, 에이전트는 그 구간을 직접 읽고 답합니다.

## 큰 그림 — 원문은 불변, 카드는 주소, 검색은 결정론

```text
질문 → 에이전트 → wiki-desk query(후보 문서 + 근거 span) → read(원문 구간) → 근거 경로와 함께 답변
```

- **스킬 폴더(`.claude/skills/wiki-desk/`)**: `SKILL.md`(사서 계약·질의 루프), `src/`(CLI와 모듈), `references/`(OKF·권위·설치·유지보수), `tests/`(node:test), `assets/`(계약 예제). 브랜치마다 같은 코드를 둡니다.
- **위키(`__llm-wiki/`, 프로젝트 루트)**: 브랜치마다 따로 갖는 지식층입니다.
  - `contract.json` — 프로젝트와 합의한 계약(추적)
  - `receipt.json` — wiki-desk가 만든 파일과 그 hash(추적). 사람이 고친 카드를 덮어쓰지 않기 위한 소유권 기록입니다.
  - `cards/<원문 경로>.card.md` — 원문 1건당 OKF 0.2 카드 1장. frontmatter 값은 원문 것을 그대로 옮기고, `description`은 원문에 있으면 복사·없으면 null입니다.
  - `index.md`, `log.md` — OKF 예약 파일
  - `.cache/index.json` — 파생 검색 색인(추적 제외, 질의 때 바뀐 원문만 재색인)
- **검색**: BM25(제목·헤딩·메타·본문 가중) + 마크다운 헤딩 트리. 한국어는 2글자 조각, 영문은 식별자 단위로 색인합니다. 임베딩·LLM 호출이 없어 같은 질의는 같은 결과를 냅니다.

경로·hash가 맞는다는 사실은 **원문 내용을 사람이 검토·승인했다는 뜻이 아닙니다.** 검색 점수와 권위 순위는 읽는 순서일 뿐 정답 판정이 아닙니다.

### 1.1.0(Python)과 무엇이 다른가요?

| 항목 | 1.1.0 | 3.0.0 |
|---|---|---|
| 실행 | Python 3.10+ · PyYAML | Node.js 24+, `.ts` 직접 실행, npm 설치 없음 |
| 위키 내용 | OKF 위키 문서 + 원문 registry | 원문 1건당 포인터 카드 |
| 질의 결과 | 일치 후보·경로·coverage | 후보 + 헤딩 경로가 붙은 근거 span |
| 계약·소유권 위치 | 스킬 안 `project/` (receipt 추적 제외) | 위키 안 `contract.json`·`receipt.json` (둘 다 추적) |
| 사람 수정 보호 | receipt sticky 보호 | 카드별 `protected`, 사람 확인(`verified`) 카드는 `reverify` |
| 범위 | 설치기·adopt·rebuild·1.0 이관·format | init·sync·query·read·tree·check·status·remove (1.0 이관·format 없음) |

## 사용자와 에이전트의 대화로 보는 사용법

아래는 사용 흐름을 설명하는 **예시 대화이며 실제 실행 기록은 아닙니다.**

> **사용자**: 멀티턴 기본값을 어디서 끄기로 정했는지 근거 문서 찾아줘.
>
> **에이전트**: wiki-desk로 "멀티턴 기본값", "multi-turn default" 두 표현을 함께 질의하겠습니다. 결과는 후보일 뿐이라, 상위 span을 원문에서 직접 읽어 확인하겠습니다.
>
> **에이전트**: `docs/plan/…_설계.md:41-58`에 기본값 OFF 결정이 있고, 이후 devlog에서 그대로 유지됐습니다. 두 문서의 해당 줄을 근거로 답합니다. 카드 메타가 원문보다 오래됐다는 `stale` 표시는 없었습니다.

## 설치 여정 1 — 스킬 폴더 넣기

준비물은 Node.js 24 이상뿐입니다. 없으면 차단 사유를 보고하고 자동 설치하지 않습니다.

**폴더 전체가 한 단위입니다.** `SKILL.md`만 복사하면 CLI가 빠집니다. 기존 동명 스킬이 있으면 덮어쓰지 말고 차이를 먼저 검토하세요.

배포 단위는 **태그된 커밋**입니다. `git archive`로 받으면 `PACKAGE_MANIFEST.json`과 바이트 단위로 같은 트리가 설치됩니다.

```bash
(
  set -euo pipefail
  SOURCE="/absolute/path/to/wiki-desk"     # 이 저장소를 받은 폴더(git clone)
  VERSION="v3.0.0"
  PROJECT="/absolute/path/to/project"      # 설치할 프로젝트 루트
  DEST="$PROJECT/.claude/skills/wiki-desk"
  if [ -e "$DEST" ] || [ -L "$DEST" ]; then
    printf '중단: 기존 스킬을 보존합니다: %s\n' "$DEST" >&2
    exit 1
  fi
  mkdir -p "$DEST"
  git -C "$SOURCE" archive "$VERSION" | tar -x -C "$DEST"
  cd "$DEST"
  node scripts/package-manifest.ts --check   # 파일 집합·sha256 대조
  node --test
)
```

테스트 통과는 패키지 회귀 확인일 뿐입니다. Claude Code가 스킬을 실제로 인식하는지는 **새 세션의 스킬 목록**에서 따로 확인합니다.

## 설치 여정 2 — 프로젝트와 계약을 합의하고 위키 만들기

### 계약

[`assets/project-contract.example.json`](assets/project-contract.example.json)은 **합성 예시**입니다. 그대로 승인된 계약으로 쓰지 말고, 아래 항목을 사용자와 합의해 바꾸세요.

| key | 뜻 |
|---|---|
| `wiki_dir` | `__llm-wiki` 또는 `__llm_wiki`. 리터럴 그대로 쓰고 서로 바꾸지 않습니다. |
| `source_roots`, `excluded_roots`, `source_suffixes` | 원문 범위(예: `docs`, `letter`, `.md`). 심볼릭링크·특수 파일 원문은 거부합니다. |
| `authority_rules`, `fallback` | 파일명 패턴별 문서 종류·권위 순위·역할. 첫 일치 규칙을 적용합니다. |
| `frontmatter_aliases` | 원문 frontmatter 키의 별칭(예: `related_plan` ← `plan`) |
| `filename_pattern` | 파일명 규칙 정규식. named group `prefix`·`yymmddhh`·`title`이 필요하고, 어긋나면 카드에 `needs_review` |
| `layout` | `cards_dir`·`cache_dir`·`archive_dir`. 서로 겹치거나 예약 이름을 쓰면 거부합니다. |
| `search` | BM25 `k1`·`b`, 필드 가중치, 권위 가중, span 최대 줄 수, 결과 수 |
| `copy_policy` | `path_reference`만 지원 |

알 수 없는 확장 키는 보존합니다.

### 계획 → 적용

프로젝트 루트에서 실행합니다. 모든 쓰기 명령은 `--apply` 없이 계획만 출력합니다.

```bash
node .claude/skills/wiki-desk/src/cli.ts init --contract <합의한 계약 파일>            # 계획
node .claude/skills/wiki-desk/src/cli.ts init --contract <합의한 계약 파일> --apply    # 위키 골격 생성
node .claude/skills/wiki-desk/src/cli.ts sync                                         # 카드 생성 계획
node .claude/skills/wiki-desk/src/cli.ts sync --apply                                 # 카드 생성
node .claude/skills/wiki-desk/src/cli.ts check                                        # OKF 0.2 MUST·누락·고아·stale
```

위키(`__llm-wiki/`·`__llm_wiki/` 어느 쪽이든)가 이미 있으면 `init`은 거부합니다. 적용 뒤 `git diff --diff-filter=MD -- <source_roots>`가 비어 있는지로 원문 불변을 확인하세요.

## 실제 작업 여정 — 질의 → 원문 읽기 → 입고

### 1. 질의하고 근거 구간을 직접 읽습니다

```bash
node .claude/skills/wiki-desk/src/cli.ts query --terms "멀티턴 기본값" --terms "multi-turn default"
node .claude/skills/wiki-desk/src/cli.ts read docs/devlog/<file>.md:41-58
node .claude/skills/wiki-desk/src/cli.ts tree docs/devlog/<file>.md
```

- `--terms`를 여러 번 주면 OR 검색입니다. 동의어·한영 표현을 함께 넘기세요.
- 결과에는 `results[].spans[].ref`(근거 구간)와 함께 `unmatched_tokens`·`stale`·`missing_cards`·`invalid_cards`가 나옵니다. 색인할 토큰이 하나도 없으면 거부합니다.
- **명령은 한 줄에 하나씩** 실행합니다. 셸 변수(`W=...; $W`)나 `cd ... &&`로 묶으면 서브에이전트의 Bash 허용 규칙(`Bash(node .claude/skills/wiki-desk/src/cli.ts:*)`)에 걸리지 않아 거부될 수 있습니다.

### 2. 문서를 발행할 때 입고합니다

자동 hook은 없습니다. 작업 기록 문서를 발행하는 절차 안에서 사람이나 에이전트가 직접 실행합니다.

```bash
node .claude/skills/wiki-desk/src/cli.ts sync            # 계획
node .claude/skills/wiki-desk/src/cli.ts sync --apply
```

| 보고 | 뜻 | 조치 |
|---|---|---|
| `protected` | 사람이 고친 카드·`index.md`·`log.md` | 덮어쓰지 않습니다. 수정분을 옮긴 뒤 카드를 지우고 다시 sync하면 재생성됩니다. |
| `reverify` | 사람 확인(`verified`)이 있는 카드의 원문이 바뀜 | 확인 기록을 새 내용에 옮기지 않습니다. `verified`를 따로 보관·삭제 → sync → 새 원문 확인 후 다시 기록합니다. |
| `orphans_blocked` | 원문이 지워지거나 옮겨짐 | 검토 후 `--accept-removed <경로>` 또는 `--relocate OLD=NEW`. 카드는 `archive_dir`에 보존됩니다. |

### 3. 제거해도 사람 것은 남깁니다

```bash
node .claude/skills/wiki-desk/src/cli.ts remove            # 계획
node .claude/skills/wiki-desk/src/cli.ts remove --apply
```

receipt에 기록되고 **바뀌지 않은** 생성물과 색인 캐시만 지웁니다. `contract.json`·사람이 고치거나 추가한 파일·원문은 보존합니다. receipt가 없거나 위키 밖 경로를 담고 있으면 거부합니다.

## 성능 메모

- 색인과 질의는 결정론이며 LLM·임베딩 호출이 없습니다. 바뀐 원문만 재색인합니다.
- 수백 건 규모의 문서 모음에서는 응답 시간이 색인보다 **에이전트의 대화 턴 수**에 좌우되기 쉽습니다. 이 도구의 주된 가치는 속도보다 **근거 위치를 남기고 다시 찾게 하는 것**과 입고 규율에 있습니다.
- 자기 코퍼스로 재려면 `node tests/perf.probe.ts <git 저장소> <커밋> <원문 루트(쉼표 구분)> <질의>...`를 실행합니다(`node --test` 대상 아님).

## 안전 경계와 검증 방법

- 원문은 어떤 명령에서도 쓰기 대상이 아닙니다. 쓰기는 `--apply`가 있을 때만, 트랜잭션(사전 이미지 기록·원자적 교체·실패 시 rollback)으로 수행합니다.
- daemon·cron·hook·자동 갱신·전역 설정 편집은 없습니다.
- 확인한 실행 환경은 Linux/WSL의 Claude Code입니다. `.agents/skills`(Hermes/Codex) 경로, Windows·macOS는 미검증입니다.
- 1.0.x 이관과 OKF 형식 교정(`format`)은 범위 밖입니다. 필요하면 `wiki-desk_PY`를 쓰세요.
- 알려진 미수정 사항: sync가 소유를 판정한 뒤 쓰기까지 약 0.1초 사이에 사람이 같은 카드를 저장하면 덮어쓸 수 있음, 계약 JSON의 중복 키를 거부하지 않음, 상시 타입 검사 단계 없음(TypeScript strict 점검은 개발 중 1회 통과).

```bash
cd .claude/skills/wiki-desk && node --test      # 결정론 회귀(LLM·네트워크 없음)
node scripts/sanitize-check.ts                  # 배포 위생 0건
node scripts/package-manifest.ts --check        # manifest 대조
```

개발 중 추가로 OKF 공식 예제 81건 round-trip 무손실, 코드 리뷰 6종 5라운드(치명·주요 결함 0)를 확인했습니다. 시험 통과는 실제 프로젝트에서의 인식·사용 성공을 뜻하지 않습니다.

## 더 읽을 문서

| 파일 | 읽는 이유 |
|---|---|
| [SKILL.md](SKILL.md) | 에이전트가 따르는 질의 루프와 명령 |
| [OKF](references/okf.md) | 카드 형식과 OKF 0.2 MUST 조항 |
| [권위](references/authority.md) | 권위 순서와 승인·검토의 경계 |
| [설치와 인식](references/installation.md) | 설치·초기화·제거 세부 |
| [유지보수](references/maintenance.md) | sync·protected·reverify·고아 처리 |

라이선스: [MIT](LICENSE).
