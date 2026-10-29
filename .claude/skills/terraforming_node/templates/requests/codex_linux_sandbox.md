# Codex 실행 설정 진단 request 템플릿

> 기존 경로 호환용이다. bwrap/AppArmor sudo 설치 절차는 은퇴했다.
> main/sub 표준 설치는 `default_permissions = ":danger-full-access"`, `approval_policy = "never"`다.
> 실제 request는 관측한 대상·버전·오류·회수 경로를 채워 `docs/request/`에 발행한다.

## 전제와 준비물

manifest에서 대상 노드·work_dir·역할을 해소하고 해당 노드의 일반 사용자 터미널에서 수행한다.
프로젝트 신뢰와 기존 인증을 사용하며 사용자/managed 설정·자격증명을 덮거나 복사하지 않는다.
예상 소요는 로컬 설정 확인 수분이며 실제 native 카나리에는 선택 모델의 사용량이 발생할 수 있다.

## 설정 확인

대상 workspace에서 아래 읽기 명령으로 CLI 버전과 project 설정을 확인한다.

```bash
codex --version
python3 - <<'PYCONFIG'
import pathlib, tomllib
config = tomllib.loads(pathlib.Path('.codex/config.toml').read_text())
print({key: config.get(key) for key in ('default_permissions', 'approval_policy', 'sandbox_mode')})
assert config.get('default_permissions') == ':danger-full-access'
assert config.get('approval_policy') == 'never'
assert not any(key in config for key in ('permissions', 'sandbox_mode', 'sandbox_workspace_write'))
PYCONFIG
```

성공 기준은 공통 기본값과 legacy/custom profile 부재다. 불일치하면 메인 owner가 정본·렌더·공식 전달로
프로젝트 설정을 갱신한다. 이것만으로 effective 설정이나 native 실행 성공을 주장하지 않는다.

## 실제 선택과 실행

새 native 프로세스에서 신뢰된 project layer와 effective 선택을 관측한다. 제공되는
`config/read`·`permissionProfile/list` 결과에서 필요한 필드만 회수하고 비밀은 출력하지 않는다.
메인은 `references/install/harness_environment.md`에 따라 명시 provider/model·예산으로 discovery와
permissions 카나리를 수행하고 실제 도구 receipt·task-report를 검증한다.

`bwrap: setting up uid map: Permission denied`가 계속되면 legacy CLI/config override와 실제 실행
경로를 확인한다. managed 거부·미지원 CLI·미신뢰 프로젝트·다른 sandbox 실패면 원 오류를 회신한다.
전역 userns/AppArmor 변경이나 sudo 설치로 자동 복구하지 않는다.

## 회수물

대상 manifest 포인터·CLI 버전·비밀 없는 project/effective 설정 관측·실제 argv·원 오류/종료 코드·
카나리 결과와 로그 포인터를 회수한다. 설치·native 실행·A2A 왕복·캠페인 완료를 구분해 기록한다.
