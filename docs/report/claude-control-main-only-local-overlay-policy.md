# Claude Control 메인노드 전용 로컬 오버레이 정책

> 대상: `easy_vllm_simulator` 메인테이너와 배포자  
> 결론: `claude-code-control` 통합은 **메인노드 메인테이너의 로컬 오버레이**로만 유지한다. 프로젝트 배포물, Git 원격 저장소, 서브노드 작업환경에는 포함하지 않는다.

## 1. 결정 요약

`easy_vllm_simulator`는 `single-node`와 `multi-node` 두 배포 브랜치를 운용하지만, `claude-code-control`은 프로젝트 기능이나 배포자용 에이전트 기능이 아니다. 이것은 메인테이너가 자신의 Hermes Agent에서 **메인노드의 프로젝트 Claude Code만 제어하기 위한 개인 제어면**이다.

따라서 다음 정책을 채택한다.

1. `claude-code-control` 스킬 본체는 Hermes의 로컬 스킬 디렉터리에만 설치한다.
2. 프로젝트에 설치되는 Agent Card, control contract, 실행 상태와 스키마도 Git 비추적 로컬 파일로 유지한다.
3. 이 로컬 오버레이는 브랜치에 속하지 않으므로 `single-node`와 `multi-node` 전환 시 같은 working tree에서 자연스럽게 지속된다.
4. 통합 아티팩트를 `.claude/skills/upstream-version-watch/scripts/sync_branches.sh`의 동기화 대상에 추가하지 않는다.
5. `render_sub_env.py`와 `sync_to_sub.sh`의 서브노드 전파 경로에도 통합 아티팩트를 추가하지 않는다.
6. Git push 결과와 서브노드 staging 결과를 각각 독립 검증한다. **Git 비추적만으로 rsync 전파까지 자동 차단된다고 간주하지 않는다.**

즉, **통합 자체는 브랜치 동기화하지 않는다.** 브랜치에 속하지 않는 로컬 오버레이로 만들면 두 브랜치에서 같은 사본을 사용하게 되므로 별도 동기화가 필요 없다.

## 2. 두 설계안 비교

| 항목 | A. Git 추적 + 두 브랜치 동기화 + 서브 exclude | B. 메인테이너 로컬 비추적 오버레이 |
|---|---|---|
| 두 브랜치에서 사용 | 가능하지만 매 변경마다 동기화 필요 | 같은 working tree의 로컬 파일이 브랜치 전환 후에도 지속 |
| 원격 push | 설치 흔적과 계약 파일이 배포자에게 전달됨 | 추적되지 않으므로 push 대상이 아님 |
| 배포자 경험 | 사용하지 않는 Hermes/Claude 제어 계약이 노이즈가 됨 | 프로젝트 본래 기능만 배포됨 |
| 서브노드 차단 | 별도 exclude와 회귀 테스트 필요 | 현재 고정 allowlist에는 애초에 진입하지 않음. 단 독립 검증은 유지 |
| 운영 복잡도 | 브랜치 sync, 원격 배포, 서브 exclude 세 축 관리 | 로컬 ignore + 전파 allowlist 검증의 두 축 |
| 설치 제거 | 브랜치별 커밋과 배포 이력까지 정리 필요 | 메인테이너 로컬 파일만 제거 |
| 권고 | 채택하지 않음 | **채택** |

### A안을 채택하지 않는 이유

A안은 기술적으로 가능하지만 요구사항과 맞지 않는다. Git에 추적하는 순간 다음 정보가 배포 계약 일부가 된다.

- Hermes와 Claude Code를 연결하는 Agent Card
- control-state와 HITL 계약
- 실행 이력 디렉터리와 스키마
- 프로젝트가 특정 외부 오케스트레이터를 전제한다는 인상

배포자는 이 제어면 없이도 `easy_vllm_simulator`를 사용할 수 있어야 한다. 선택적 개인 도구를 두 배포 브랜치에 싣는 것은 재현성을 높이는 것이 아니라 배포 표면과 인지 부채를 늘린다.

### B안이 브랜치 동기화를 자연스럽게 해결하는 이유

Git 비추적 파일은 브랜치 tree의 구성원이 아니다. 경로 충돌이 없는 한 같은 working tree에서 `single-node`와 `multi-node`를 전환해도 로컬 파일은 남는다.

따라서 관계는 다음과 같다.

```text
Git tracked project
├─ single-node
└─ multi-node

Maintainer-local overlay (branch 밖)
└─ claude-code-control integration
   ├─ single-node checkout에서 사용
   └─ multi-node checkout에서 같은 사본 사용
```

이 모델에서는 통합 파일을 두 번 복사하거나 `sync_branches.sh`에 등록하지 않는다. **동기화 대상에서 제거하는 것이 곧 두 브랜치 공용화 방식**이다.

## 3. 세 가지 경계를 분리해서 판단한다

### 3.1 브랜치 경계

`.claude/skills/upstream-version-watch/scripts/sync_branches.sh`는 source branch의 **Git 추적 allowlist**를 destination branch로 복사한다. 대표 대상은 다음과 같다.

- `CLAUDE.md`
- `.claude/rules`
- `.claude/skills`
- 공용 scripts와 templates
- `docs/report/`

따라서 control marker가 `CLAUDE.md`에 남거나 control rule이 `.claude/rules`에서 추적되면 브랜치 동기화에 자연스럽게 포함된다. 통합 *본체*는 아래 조건으로 비추적화하되, `CLAUDE.md` 마커는 **자기-무력화 형태로 재설계해 의도적으로 유지**한다.

- 추적되는 `CLAUDE.md`에는 **자기-무력화(self-neutralizing) 옵셔널 마커만** 남긴다 — "claude-code-control 스킬/아티팩트가 없는 배포자 체크아웃이면 이 절 전체를 무시하라"고 명시해, 브랜치 동기화로 전파돼도 배포자 에이전트가 안전히 무시한다(제품 동작·헌법 무영향). 통합 *본체*는 남기지 않는다.
- `.claude/rules/hermes-claude-control.md`는 로컬 exclude 대상으로 유지하고 커밋하지 않는다.

통합 경로를 `sync_branches.sh` allowlist에 추가해서는 안 된다.

### 3.2 원격 저장소 경계

Git push는 커밋된 tree만 전송한다. 따라서 완전한 비추적 상태를 유지하면 원격 저장소에는 통합 아티팩트가 들어가지 않는다.

단, 저장소의 `.gitignore`에 control 전용 패턴을 추가하는 방식은 권장하지 않는다. 파일 본체가 없어도 `.gitignore` 변경 자체가 “이 프로젝트가 해당 통합을 안다”는 설치 정보를 배포하기 때문이다.

권장 방식은 저장소 밖의 로컬 Git 설정인 다음 파일을 이용하는 것이다.

```text
.git/info/exclude
```

예시 로컬 exclude:

```gitignore
/agent-card.json
/.hermes-claude-control/
/.claude/rules/hermes-claude-control.md
```

`.claude/schemas/`와 `.claude/templates/`는 프로젝트의 기존 `.gitignore`가 `.claude/*`를 기본 제외한다. 설치기가 추가한 아래 재포함 규칙을 저장소 `.gitignore`에서 제거하면 control 스키마와 템플릿도 다시 비추적 상태가 된다.

```gitignore
!.claude/schemas/
!.claude/schemas/**
!.claude/templates/
!.claude/templates/**
```

아래 runtime 전용 규칙도 control 설치 흔적이므로 저장소 `.gitignore`에 두지 않고, 전체 control 디렉터리를 `.git/info/exclude`에서 제외한다.

```gitignore
.hermes-claude-control/runtime/
.hermes-claude-control/runs/
```

### 3.3 서브노드 경계

Git ignore와 rsync exclude는 서로 다른 제어면이다.

- Git ignore: 브랜치와 원격 저장소 유입을 제어한다.
- 렌더/rsync allowlist: 서브노드 파일 전파를 제어한다.

현재 서브노드 전파는 저장소 전체를 복사하지 않는다.

1. `render_sub_env.py`가 정해진 서브용 템플릿, 규칙, schema와 런타임 스킬만 `output/<topology>/sub_provision/`에 렌더한다.
2. `sync_to_sub.sh`가 `output/<topology>/`의 Band2 allowlist와 위 staging overlay만 전송한다.
3. `docs/`는 하향 빌드 rsync에서 제외된다.
4. 메인 전용 `docs/report/`는 서브 렌더 대상이 아니다.

다음 메인 control 경로는 현재 렌더/전송 allowlist에 포함되지 않는다.

```text
agent-card.json
.hermes-claude-control/
.claude/rules/hermes-claude-control.md
.claude/schemas/hermes-control-run-report.schema.json
.claude/templates/hermes-control-run-report.template.md
.claude/templates/hermes-control-completion-flag.template.json
```

이 상태를 유지한다. 향후 `render_sub_env.py` 또는 `sync_to_sub.sh`를 “저장소 루트 전체 복사” 방식으로 확장해서는 안 된다. 전파 전 staging에 위 경로가 하나라도 나타나면 fail-closed 해야 한다.

## 4. 추적 파일에서 되돌려야 할 설치 흔적

현재 로컬 설치 직후 상태에서는 아직 원하는 정책이 완성되지 않았다. 커밋 또는 push 전에 다음을 정리해야 한다.

### `CLAUDE.md`

설치기가 추가한 optional integration marker block은 **제거하지 않고, 자기-무력화(self-neutralizing) 구문으로 재설계해 유지한다**(메인테이너 결정 — `docs/plan/plan_2026072308_1`). 재설계 마커는 다음 형태다.

```text
<!-- BEGIN HERMES-CLAUDE-CONTROL OPTIONAL -->
## Optional Hermes-Claude Control Integration (선택 · 메인테이너 로컬 전용 · control-plane 한정)
> 배포자 참고: 관련 아티팩트는 메인테이너 로컬에만 존재하며 이 배포본에는 없다(git 비추적).
> claude-code-control 스킬/아티팩트가 없으면 이 절 전체를 무시하라 — 제품 동작·헌법 무영향.
… 경계 정책 상세 = docs/report/claude-control-main-only-local-overlay-policy.md.
<!-- END HERMES-CLAUDE-CONTROL OPTIONAL -->
```

이 마커는 브랜치 동기화로 두 브랜치·배포자에게 전파되지만, 통합 *본체*(`agent-card.json`·`.hermes-claude-control/`·control rule/schema/template)는 비추적 로컬 오버레이로 남으므로, 배포되는 것은 이 "없으면 무시하라"는 마커 텍스트뿐이다. 배포자 에이전트의 토큰이 소량 소비될 수 있으나 제품 동작에는 영향이 없다.

### `.gitignore`

설치기가 추가한 control 관련 6개 규칙을 제거한다.

```gitignore
!.claude/schemas/
!.claude/schemas/**
!.claude/templates/
!.claude/templates/**
.hermes-claude-control/runtime/
.hermes-claude-control/runs/
```

그 대신 메인테이너 로컬 `.git/info/exclude`를 사용한다.

### 개인 설치 가이드

`docs/report/hermes-slack-claude-control-installation-guide.md`는 개인 Hermes/Slack 설치 절차이므로 배포자용 report로 커밋하지 않는다. `docs/report/`는 프로젝트 규약상 **추적·배포되는 예외 채널**이라 이 경로에 둔 채 `git add -A`를 사용하면 원격에 포함될 수 있다.

개인 설치 가이드는 다음 중 하나로 옮긴다.

- `.hermes-claude-control/private-docs/`
- 저장소 바깥의 개인 운영 문서 디렉터리

반면 현재 문서(`claude-control-main-only-local-overlay-policy.md`)는 “외부 오케스트레이터가 배포 계약이 아니다”라는 프로젝트 경계를 배포자에게 알리는 정책 문서이므로 의도적으로 추적한다.

## 5. 권장 최종 레이아웃

```text
$HERMES_HOME/skills/.../claude-code-control/     # 개인 Hermes 설치, 프로젝트 밖

<easy_vllm_simulator>/                           # 메인노드 working tree
├─ CLAUDE.md                                     # tracked — 자기-무력화 마커 유지(브랜치 싱크·배포)
├─ agent-card.json                               # local-only
├─ .hermes-claude-control/                       # local-only
│  └─ private-docs/…installation-guide.md        # local-only (이동된 개인 설치가이드)
├─ .claude/rules/hermes-claude-control.md        # local-only
├─ .claude/schemas/hermes-control-*.json         # local-only
├─ .claude/templates/hermes-control-*            # local-only
├─ .git/info/exclude                             # local-only Git 정책
└─ docs/report/
   ├─ claude-control-main-only-local-overlay-policy.md  # 의도적으로 추적·배포
   └─ <상류 개선 편지>.md                          # 의도적으로 추적·배포(claude-code-control 피드백)
```

서브노드에는 위 local-only 경로를 만들지 않는다. 서브노드는 기존 프로젝트의 서브 전용 `Agent_Card.json`, 서브 헌법, comms 계약과 허용된 런타임 블록만 받는다.

## 6. 적용 순서

1. 현재 integration 파일을 백업한다.
2. 추적 파일 정리 — `.gitignore`는 설치기 6줄을 되돌리고(`git checkout -- .gitignore`), `CLAUDE.md` 마커는 **제거 대신 자기-무력화 구문으로 재설계해 유지**한다(§4).
3. `.git/info/exclude`에 메인 전용 local-only 경로를 등록한다.
4. 개인 설치 가이드를 `docs/report/` 밖의 local-only 위치로 옮긴다.
5. 아래 검증을 `single-node`에서 수행한다.
6. `multi-node`로 전환한 뒤 같은 local-only 파일이 유지되고 Git status에 나타나지 않는지 다시 검증한다.
7. 이 정책 문서만 일반 report 브랜치 동기화 절차로 `multi-node`에 동기화한다.
8. 서브 staging과 `sync_to_sub.sh --dry-run`에서 control 경로가 0건인지 확인한다.
9. 검증 전에는 integration 관련 파일을 commit/push하지 않는다.

`docs/report/`는 추적 예외이므로 이 정책 문서는 기존 브랜치 동기화 도구를 사용한다. 정본이 `single-node`, 대상이 `multi-node`라면 대상 브랜치를 checkout한 상태에서 명시적으로 다음 변수를 사용한다.

```bash
SRC_BRANCH=single-node DST_BRANCH=multi-node bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh
SRC_BRANCH=single-node DST_BRANCH=multi-node bash .claude/skills/upstream-version-watch/scripts/sync_branches.sh --apply
```

첫 명령은 dry-run이고 두 번째 명령은 working tree를 갱신한다. 커밋과 push는 사람이 diff를 검토한 뒤 수행한다.

## 7. 검증 게이트

### 7.1 로컬 파일은 존재하지만 Git에는 보이지 않아야 한다

```bash
test -f agent-card.json
test -d .hermes-claude-control
test -f .claude/rules/hermes-claude-control.md

git status --short
git check-ignore -v agent-card.json
git check-ignore -v .hermes-claude-control/runtime/control_status.json
git check-ignore -v .claude/rules/hermes-claude-control.md
```

합격 조건:

- local-only 파일은 실제로 존재한다.
- `git status --short`에는 local-only 파일이 나타나지 않는다.
- `git check-ignore -v`는 저장소 `.gitignore`의 control 전용 규칙이 아니라 로컬 `.git/info/exclude` 또는 기존 범용 `.claude/*` 규칙을 근거로 표시한다.

### 7.2 Git index와 커밋 tree에 없어야 한다

```bash
if git ls-files --error-unmatch agent-card.json >/dev/null 2>&1; then
  echo "FAIL: agent-card.json is tracked"
  exit 1
fi

if git ls-files | grep -E '(^|/)(\.hermes-claude-control|hermes-claude-control\.md|hermes-control-(run-report|completion-flag))'; then
  echo "FAIL: Claude control integration is tracked"
  exit 1
fi
```

### 7.3 push 직전 staged diff에 없어야 한다

```bash
git diff --cached --name-only | grep -E '(^agent-card\.json$|^\.hermes-claude-control/|^\.claude/rules/hermes-claude-control\.md$|^\.claude/(schemas|templates)/hermes-control-)' \
  && { echo "FAIL: private control artifact staged"; exit 1; } \
  || true
```

### 7.4 두 브랜치 모두 원격 tree에 없어야 한다

```bash
for branch in single-node multi-node; do
  git ls-tree -r --name-only "origin/$branch" \
    | grep -E '(^agent-card\.json$|^\.hermes-claude-control/|^\.claude/rules/hermes-claude-control\.md$|^\.claude/(schemas|templates)/hermes-control-)' \
    && { echo "FAIL: origin/$branch contains private control artifacts"; exit 1; } \
    || true
done
```

### 7.5 서브노드 staging에 없어야 한다

서브 렌더 후 다음과 같이 검사한다.

```bash
python3 - <<'PY'
from pathlib import Path

for root in (Path("output/single/sub_provision"), Path("output/multi/sub_provision")):
    if not root.exists():
        continue
    forbidden = []
    for p in root.rglob("*"):
        rel = p.relative_to(root).as_posix()
        if (
            rel == "agent-card.json"
            or rel.startswith(".hermes-claude-control/")
            or rel == ".claude/rules/hermes-claude-control.md"
            or rel.startswith(".claude/schemas/hermes-control-")
            or rel.startswith(".claude/templates/hermes-control-")
        ):
            forbidden.append(rel)
    if forbidden:
        raise SystemExit(f"FAIL {root}: {forbidden}")
    print(f"PASS {root}: private control artifacts 0")
PY
```

`sync_to_sub.sh`는 먼저 dry-run으로 검토한다. 출력에 위 금지 경로가 나타나면 apply하지 않는다.

## 8. 운영 불변식

- `claude-code-control`의 제어 대상은 **메인노드의 프로젝트 Claude Code 하나**다.
- 서브노드 Claude는 기존 프로젝트의 A2A/서브 계약으로만 운영한다.
- 메인 Hermes가 `claude-code-control`을 통해 서브노드 Claude를 직접 제어하는 capability를 추가하지 않는다.
- 개인 control 아티팩트를 `sync_branches.sh`, `render_sub_env.py`, `sync_to_sub.sh` allowlist에 추가하지 않는다.
- 저장소 `.gitignore`, `CLAUDE.md`, 배포 README에 개인 설치 상태를 기록하지 않는다.
- 브랜치 공용성이 필요하다는 이유로 개인 오버레이를 Git 추적물로 승격하지 않는다.
- Git 추적 차단과 서브 전파 차단은 별도 검증한다.
- 배포자가 Hermes를 사용하지 않아도 프로젝트 기능과 문서 계약이 완전해야 한다.

## 9. 최종 판정

사용자의 가설은 절반 이상 맞다.

> “Claude Control 구성요소를 Git 추적에서 완전히 제외하면 브랜치 동기화가 자연스럽게 되지 않는가?”

**브랜치와 원격 저장소에 대해서는 맞다.** 로컬 비추적 오버레이는 브랜치 밖에 있으므로 두 브랜치에서 같은 사본을 사용하고 push에도 포함되지 않는다.

다만 **서브노드 전파는 Git과 별도 축**이다. rsync가 untracked 파일까지 복사하도록 작성되어 있다면 Git 비추적만으로는 막을 수 없다. 이 프로젝트의 현재 구현은 고정 렌더 staging과 Band2 allowlist를 사용하므로 control 파일이 전파 경로에 들어가지 않지만, 이 사실을 독립 검증하고 향후에도 allowlist 방식을 유지해야 한다.

따라서 최종 권고는 다음 한 문장으로 정리된다.

> **Claude Control은 메인테이너 로컬·메인노드 전용 오버레이로 유지하고, Git tree와 서브노드 staging 양쪽에서 존재 0건을 검증한다.**
