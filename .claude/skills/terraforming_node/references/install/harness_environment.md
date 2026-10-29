# 하네스 실행환경과 업무 권한 — §2.5.2

이 절차의 owner는 `terraforming_node`다. 신규 설치 및 provider·설정·프로젝트 신뢰·호스트 실행환경 변경 때
적용한다. HW Flag가 FRESH여도 하네스 검증을 대신하지 않는다. 기존 manifest·캠페인·유효 승인·인증을
복구해 이어 쓰며, 하네스 변경만으로 HW를 다시 스캔하거나 업무 승인을 초기화하지 않는다.

## Codex 설치·config.toml 정책

메인과 서브 모두 신뢰된 프로젝트 `.codex/config.toml`에 아래 기본값을 설치한다.

```toml
default_permissions = ":danger-full-access"
approval_policy = "never"
```

`vllm-main`·`vllm-sub` 같은 별도 `[permissions.*]` 프로필은 생성하지 않는다. 메인 정본은
`.codex/config.toml`, 서브 정본은 `sub_node/codex.config.template.toml`이며 renderer와 공식 sync로 배달한다.
기존 프로젝트의 역할별 프로필은 이 정본으로 교체한다. 사용자 설정·managed 정책·자격증명은 보존하며
서브로 복사하지 않는다. `sandbox_mode`·`[sandbox_workspace_write]`·CLI `--sandbox`는 기본 권한을
덮을 수 있으므로 표준 운영 설정에 섞지 않는다. 상위 설정/managed 제약으로 선택이 거부되면 그 출처와
실제 오류를 기존 input-required/HITL 계약으로 보고한다.

`:danger-full-access`는 로컬 sandbox 제한을 제거하는 의도된 설치 기본값이다. bwrap user namespace를
요구하는 역할별 workspace sandbox를 표준 설치에서 사용하지 않으며, 그 준비를 위해 AppArmor 프로필을
추가하거나 userns 제한을 바꾸는 sudo 절차도 요구하지 않는다. `bwrap: setting up uid map: Permission denied`가
계속 관측되면 실제 선택·legacy override·프로젝트 신뢰·설정 로딩을 먼저 확인한다. 다른 sandbox 실행이나
호스트 자체의 실패까지 이 설정으로 해결됐다고 단정하지 않는다.

## 역할·승인과 실행 권한

main/sub 정체성과 업무 권한의 권위는 manifest, `node_role_contract`, SSH endpoint 인증,
노드 오케스트레이터·캠페인 계약이다. 전체 접근은 서브의 메인 소유 파일 수정·무단 스캔·원격 git push·
무승인 모델 다운로드·sudo·hint 발행을 허가하지 않는다. 기존 지침·rules·managed 정책을 그대로 따른다.
Codex의 OS 파일 차단으로 역할 경계를 증명한다고 주장하지 않는다. Claude의 기존 tool allow/deny 계약은
그대로이며 두 하네스의 실제 실행 증거를 구분한다.

## 설치와 검증 순서

1. manifest의 대상, 역할, 기존 승인·배정·진행 아티팩트를 복구하고 실행 provider/model을 명시한다.
2. 메인은 프로젝트 정본, 서브는 renderer와 공식 `sync_to_sub.sh`로 설정·지침·발견 wrapper·rules를 전달한다.
   single 외부 hint clone의 실제 clone/git-dir/common-dir 관측과 기존 승인은 `--hint-write-scopes`로
   renderer와 sync에 연결해 보존한다. 이 기록은 발행 대상/승인 결속이며 Codex filesystem 허용 목록이 아니다.
   multi에 single hint 발행 범위를 추가하지 않는다. clone 초기 장착은 기존 §2.5.1 절차를 따른다.
3. 새 native 프로세스에서 CLI 지원·인증·프로젝트 신뢰·설정 로딩과 실제 선택을 확인한다.
   `config/read`·`permissionProfile/list`가 제공되면 project layer, `:danger-full-access`, `never`,
   legacy override 여부를 관측한다. 명령 성공만으로 설정 선택을 증명하지 않는다.
4. `bootstrap_canary.py --phase discovery` 후 `--phase permissions --target main|sub`를 실행한다.
   운영/permissions Codex 호출은 `--permission-profile :danger-full-access`와 명시 model·wall-time 예산을
   사용한다. 읽기 전용 discovery는 기존 `--sandbox-mode read-only`를 명시적으로 선택할 수 있지만,
   그 성공으로 운영 권한을 입증하지 않는다. CLI 옵션 정본은 각 스크립트 `--help`다.
5. permissions helper는 요청한 native 하네스 안에서 실행한다. 실제 도구 receipt·nonce·대상·schema·
   scratch/local git·Docker 조회·도서관 질의·역할별 기능을 확인한다. canonical/build 파일은 truncate나
   write 없이 OS write-open 후 닫아 보존을 확인한다. Codex 기대값은 main/sub 모두 allowed이며,
   이는 역할 소유나 OS 격리의 증거가 아니다. Claude의 OS 관측으로 native Edit/Write deny를 증명하지 않는다.
6. native 성공·task-report·업무 기능·관측 한계를 testlog에 기록한다. 실제 배포 후 카나리 미통과는 done이
   아니다. 설치 성공을 serving·Full benchmark·실제 hint 발행 완료로 세거나 별도 ready flag를 만들지 않는다.

카나리는 모델 weights 로드·컨테이너 기동·원격 태그 쓰기를 하지 않는다. 운영자 셸의 helper 직접 실행이나
다른 provider의 성공을 요청한 하네스의 성공으로 대체하지 않는다.

## 세션과 승인 지속성

`approval_policy = "never"`는 native 도구 승인 대기를 없애는 기본값이다. 업무 승인은 캠페인 아티팩트에서
복구한다. 재개·새 세션·하네스 전환 자체를 이유로 같은 승인을 반복 요청하지 않는다. Claude 로컬 tool 규칙은
`.claude/settings.local.json`, Codex native 재승인 규칙은 `$CODEX_HOME/rules/default.rules`가 소유한다.
한 provider의 승인 이력을 다른 provider에 복사하거나 셸 prefix 허용을 도메인 승인으로 해석하지 않는다.

기존 역할별 permission selection으로 결속된 native 세션을 새 선택으로 그대로 resume하지 않는다.
캠페인·체크포인트를 보존하고 새 선택의 native 세션으로 인계한다. `/permissions` 변경이 기존 보조 프로세스나
다음 `codex exec`까지 갱신했다는 증거는 아니다. relay의 provider·대상·권한 결속, 중복 실행·실패 복구·
동적 예산 가드는 유지한다. 실패를 무한 재시도하거나 관측하지 않은 장기 무중단 완료를 보장하지 않는다.

## 실패와 사람 수행 request

설정 로딩·managed 거부·인증·호스트 결손은 실제 원인과 실행 증거로 보고한다.
`templates/requests/codex_linux_sandbox.md`는 기존 경로 호환을 위한 설정 진단 request이며
표준 Codex 설치에서 bwrap/AppArmor sudo 설치를 지시하지 않는다. 별도 호스트 변경이 실제 필요하면
원인·대상·영향·검증·회수물을 갖춘 새로운 request를 발행한다.

공식 계약: [Codex permissions](https://learn.chatgpt.com/docs/permissions).
