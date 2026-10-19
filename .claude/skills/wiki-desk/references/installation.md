# 설치·초기화·제거 (TS판)

## 전제
- Node.js 24 이상. `.ts`를 빌드 없이 직접 실행한다. 런타임 npm 의존성은 없다. 없으면 차단 사유를 보고하고 자동 설치하지 않는다.
- 스킬은 **폴더 전체**가 한 단위다. `SKILL.md`만 복사하는 것은 설치가 아니다.
- 이 스킬은 Claude Code 프로젝트 스킬 경로(`.claude/skills/wiki-desk/`)에 둔다. 다른 host 경로·전역/profile 설정은 건드리지 않는다.

## 스킬 설치(코드)
1. 배포 태그 커밋을 `git archive <태그> | tar -x -C .claude/skills/wiki-desk/`로 푼다. 기존 동명 스킬이 있으면 덮어쓰지 않고 차이를 먼저 검토한다.
2. 설치본에서 `node scripts/package-manifest.ts --check`(파일 집합·sha256)와 `node --test`(결정론 회귀·위생·manifest 게이트 포함)를 확인한다.
3. 스킬은 Common 계층이다. 커밋·다른 sync 브랜치 동기화는 사용자 게이트를 따른다.

## 위키 초기화(브랜치별)
| 명령 | 쓰기 |
|---|---|
| `node .claude/skills/wiki-desk/src/cli.ts init --contract <합의 계약 파일>` | 없음 — 생성 계획 출력 |
| 위 명령 + `--apply` | `<wiki_dir>/contract.json`·`receipt.json`·`.gitignore`(캐시 제외)·`index.md`·`log.md` 생성 |
| `… sync` → `… sync --apply` | 카드 생성 |
| `… check` | 없음 |

- 기존 위키(`__llm-wiki/`·`__llm_wiki/` 어느 쪽이든)가 있으면 `init`은 거부한다(덮어쓰기 없음).
- `wiki_dir`는 계약의 리터럴 그대로다(`__llm-wiki`와 `__llm_wiki`는 다른 값).
- 적용 후 `status`·`check`·원문 불변(`git diff --diff-filter=MD -- <source_roots>` 공집합)을 확인한다.

## 제거
| 명령 | 동작 |
|---|---|
| `… remove` | 계획만 |
| `… remove --apply` | receipt에 기록되고 **변경되지 않은** wiki-desk 생성물(카드·보존 카드·`index.md`·`log.md`·`.gitignore`)과 색인 캐시를 제거. `contract.json`·`receipt.json`(사람 키만 더한 카드의 소유 기록만 남음)·사용자가 수정하거나 추가한 파일·원문은 보존. receipt가 없거나 위키 밖 경로를 담고 있으면 거부 |

거부되면 차이를 보고하고 파일을 보존한다. 원문은 어떤 제거 모드에서도 대상이 아니다.

remove 뒤 `sync --apply`를 실행하면 위키가 다시 채워진다(`index.md`·`log.md`·`.gitignore` 재생성. 사람 키만 더한 카드는 원문 변경 시 갱신, 그 밖의 사람 수정 카드는 `protected`). 위키를 완전히 없애려면 남은 `contract.json`·`receipt.json`·사람 파일을 확인한 뒤 직접 지운다.

## 인식 확인
파일 존재나 `status` 성공은 host가 스킬을 인식했다는 뜻이 아니다. 새 세션에서 스킬 목록·실제 로드를 따로 확인하고 결과를 분리해 기록한다.
