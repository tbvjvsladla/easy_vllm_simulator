# 편지 — claude-code-control 유지보수자께: 로컬 오버레이 사용례에서 발견한 개선점

> 발신: `easy_vllm_simulator` 메인테이너
> 수신: `claude-code-control`(Hermes → Claude Code 제어면 스킬) 유지보수자
> 성격: 동료 피드백(비난 아님). 이 프로젝트에 claude-code-control 을 **메인테이너 로컬·메인노드 전용 오버레이**로 통합하며 얻은 실측 관찰입니다.
> 근거 문서: `docs/report/claude-control-main-only-local-overlay-policy.md`(경계 정책) · `docs/plan/plan_2026072308_1`(적용 계획).

## 0. 먼저 — 이미 잘 되어 있는 것

설치/제거의 데이터 안전 설계는 견고했습니다. 그대로 유지해 주세요.

- **manifest-v3 provenance**(`created`/`preserved` + 내용 해시): remover 가 *자기가 만든 것*과 *원래 있던 것*을 정확히 구별.
- **원자적 remove**: 마커 손상 시 삭제 전 up-front refuse, `preserved` 절대 미삭제, `created` 도 해시 일치 시에만 삭제(수정됐으면 미터치+경고).
- **이중 설치 고아 방지**(provenance carry-forward), 서브에이전트 상태(`static/runtime/runs`) 기본 보존(`--purge-state` 로만 삭제).

이 셋 덕분에 "완전 제거"는 신뢰할 수 있었습니다. 아래 두 가지만 보완하면 **로컬-오버레이 사용례**가 마찰 없이 성립합니다.

## 1. install 이 *추적 파일 2개*를 건드리는 것이 로컬-오버레이 정책과 구조 충돌

현재 `install_integration_artifacts.sh` 는 대상 워크스페이스의 **git 추적 파일 2개**를 수정합니다.

- `.gitignore` 에 6줄 추가(`!.claude/schemas/…` 재포함 ×4 + `.hermes-claude-control/{runtime,runs}` ×2).
- `CLAUDE.md` 에 optional integration 마커 블록 주입.

이 두 파일은 이 프로젝트에서 **브랜치 동기화(`sync_branches.sh` allowlist)와 원격 push 의 대상**입니다. 그래서 통합을 "이 워크스페이스에서만 쓰는 개인 제어면"으로 유지하려는 순간, install 이 강제로 **배포 tree 를 오염**시킵니다 — 두 배포 브랜치와 다른 배포자에게 개인 통합 흔적이 전파됩니다.

우리 쪽 회피책(이 프로젝트에서 적용 완료):
- `.gitignore` 6줄은 되돌리고, 본체 비추적은 저장소 `.gitignore` 대신 **`.git/info/exclude`**(저장소 밖 로컬 Git 정책)로 고정 — `.gitignore` 변경 자체가 "이 프로젝트가 통합을 안다"는 설치 정보를 배포하기 때문.
- `CLAUDE.md` 마커는 **제거하지 않고 자기-무력화 구문으로 재설계해 유지**(아래 §3).

상류에 제안:
- **제안 A — `--local-overlay`(가칭) 설치 모드**: (a) `.gitignore` 를 수정하지 않고(대신 로그로 `.git/info/exclude` 등록을 *안내*만), (b) `CLAUDE.md` 마커 주입을 opt-in 으로(또는 아예 생략), (c) 그래도 설치물 전량을 manifest 에 기록해 remove 는 여전히 깔끔. 즉 "추적 파일 무수정"이 기본인 모드.
- **제안 B — 재발방지 관측점**: install 이 추적 파일을 건드릴 때, 대상이 git 레포면 "이 변경은 브랜치/원격으로 전파될 수 있음. 로컬 전용을 원하면 `--local-overlay` 를 쓰라"는 경고를 1줄 출력.

## 2. 제거(완전 역연산)의 *서브에이전트 실행 경로* 보증

remover 의 원자성·provenance semantics 는 셸에서 직접 돌릴 때는 검증됐지만, **Hermes 가 Claude Code 서브에이전트에게 remove 를 위임**하는 경로에서도 동일하게 발현되어야 합니다. 위임 실행 시 서브에이전트가:

- 반드시 **dry-run 먼저** 돌리고 `preserved`/해시-불일치/`--purge-state` 미지정 항목을 삭제 후보에서 제외하는지,
- 마커 손상 refuse(원자성)를 **무시하고 강행하지 않는지**,
- "깔끔한 삭제 또는 정확한 복구"만 수행하고 반쯤 제거된 워크스페이스를 남기지 않는지

를 보장하는 것이 핵심입니다. 현재 `docs/REMOVE_INTEGRATION_GUIDE.md` 의 *code-agent checklist* 는 훌륭하지만, 이걸 **서브에이전트 부트스트랩 가이드(`SUBAGENT_BOOTSTRAP_GUIDE.md`)에 명시 연결**해 위임 실행 시 서브에이전트가 반드시 이 체크리스트를 로드·준수하도록 하면 좋겠습니다.

제안:
- **제안 C** — `SUBAGENT_BOOTSTRAP_GUIDE.md` 에 "remove 위임 시 준수 체크리스트"(dry-run 우선 · preserved/해시 규칙 · 마커 refuse 존중 · 원자성)를 편입하고, remove 결과를 서브에이전트가 **독립 검증**(self-report 불신 원칙의 remove 축)하도록 명문화.

## 3. 마커 기본 문구를 자기-무력화(self-neutralizing) 형으로

현재 기본 마커는 "이 프로젝트는 선택적으로 … 가질 수 있다"까지만 말합니다. 배포되는 헌법(`CLAUDE.md`)은 *모든* 배포자 에이전트 컨텍스트에 바인딩 로드되므로, **통합이 없는 배포자**에게는 이 마커가 근거 없는 지시로 읽힐 여지가 있습니다.

우리가 채택한 재설계 문구(참고용):

```text
> 배포자 참고: 관련 아티팩트는 메인테이너 로컬에만 존재하며 이 배포본에는 없다(git 비추적).
> claude-code-control 스킬/아티팩트가 없으면 이 절 전체를 무시하라 — 제품 동작·헌법 무영향.
```

- **제안 D** — 기본 마커에 "이 스킬/아티팩트가 없는 체크아웃이면 이 절을 무시하라"는 **자기-무력화 문장**을 포함. 그러면 마커가 전파돼도 배포자 에이전트가 안전히 무시하고, 로컬 유지 vs 완전 제거 사이의 선택이 덜 예민해집니다.

## 4. 요약

| # | 개선점 | 효과 |
|---|---|---|
| A | `--local-overlay` 설치 모드(추적 파일 무수정 기본) | 로컬-오버레이 사용례에서 배포 tree 오염 0 |
| B | 추적 파일 수정 시 전파 경고 1줄 | 재발방지(설치자 인지) |
| C | remove 체크리스트를 서브에이전트 부트스트랩에 연결 + 위임 remove 독립검증 | 완전 역연산의 위임 실행 보증 |
| D | 자기-무력화 기본 마커 | 통합 없는 배포자 안전 무시 |

A·D 는 "install 이 추적 tree 를 안 건드리거나, 건드려도 무해"하게 만드는 **근본 축**이고, B·C 는 그 보강입니다. 견고한 provenance/원자성 설계 위에 이 넷만 얹으면, claude-code-control 을 개인 로컬 오버레이로 쓰는 사용자가 배포 계약을 흔들지 않고도 쾌적하게 쓸 수 있습니다.

감사합니다.
