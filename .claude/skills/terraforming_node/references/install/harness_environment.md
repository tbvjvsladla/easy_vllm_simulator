# 하네스 실행환경과 업무 권한 — §2.5.2

이 절차의 owner는 `terraforming_node`다. 신규 설치뿐 아니라 provider·설정·역할·프로젝트 신뢰·호스트 실행환경이
달라졌을 때도 여기로 들어온다. HW `FRESH`는 이 검증을 대신하지 않는다. 기존 manifest·캠페인·유효 승인·인증은
복구해 이어 쓰고, 하네스 변경만으로 다시 스캔하거나 승인 전체를 초기화하지 않는다.

단일 a2a-agent의 Codex 역할 설정은 일반 Docker build에 필요한 `~/.docker/buildx/activity`와
`~/.docker/buildx/refs`만 쓰기로 연다. 실제 build에서 이 bookkeeping 경로의 read-only 거부가 관측되면
메인이 역할 설정을 정식 배포하고 새 native 권한 카나리와 build 재시도로 확인한다. Docker 설정·builder
정의와 나머지 HOME은 쓰기 범위에 넣지 않으며 sudo나 환경변수 변경으로 우회하지 않는다. 실제 호스트
전제조건이 없을 때만 그 결손을 기존 request 계약으로 보고한다.

## 계약과 집행 표면

동일 권한의 기준은 **같은 역할에 승인된 업무를 수행하고 같은 도메인 소유·승인 경계를 지키는 것**이다.
SSH는 endpoint 인증/전송이다. Claude의 tool allow/deny·managed 정책과 Codex의 filesystem/network 정책은
서로 다른 집행 방식이다. named profile은 현재 Codex 실행 방식의 선택이며 SSH/A2A 프로토콜의 필수조건이 아니다.
Claude가 OS sandbox를 사용한다고 추정하거나, Codex만 강한 격리를 거는 것을 기능 동등성이라고 부르지 않는다.

| 업무 | 공통 권한 계약 | 실제 검증/한계 |
|---|---|---|
| 지침·manifest·역할·리포트 | 해당 노드의 배달 지침을 읽고 schema-valid 회신 | discovery와 native 실행 결과를 별도 확인 |
| 산출물·로컬 git | 승인된 workspace/scratch 쓰기 및 기존 local-only git | 전용 임시 파일·객체로 실제 쓰기; 원격 push 시험 금지 |
| 빌드킷 | single은 자기 저작, multi worker는 배달 정본 사용 | `node_role_contract`에서 파생; 반대 topology 권한을 섞지 않음 |
| canonical/정체성 | 서브의 자의 수정 금지 | 도구 deny와 OS write-open 차단의 관측을 구분; 실제 내용을 바꾸지 않음 |
| NAS·Docker·도서관 | 승인된 입력 읽기·실행 endpoint·사서 질의 | Docker daemon/원격 프로세스 전체가 파일 sandbox에 갇힌다고 주장하지 않음 |
| hint 발행 | single sub만 별도 외부 clone으로 자기 승인 셀 발행 | clone·git-dir·common-dir의 실제 경로; 메인 발행 경로는 기존 publisher 계약; 실제 태그는 캠페인 발행 단계에서만 |

## 메인 실행 순서

1. **복구와 범위:** manifest의 대상과 `node_role_contract` 역할, 기존 승인·배정·진행 증거를 읽는다.
   필요한 provider/model과 실행 방식을 명시한다. 하네스 기능 시험을 HW Flag나 Full bench 완료로 세지 않는다.
2. **노드 로컬 경로 관측:** 서브가 `check_sub_hint_equip.py`의 비밀 없는 관측을 docs로 회신한다.
   single 외부 clone 쓰기 입력은 `render_sub_env.py --hint-write-scopes <JSON>`에 명시한다.
   관측 JSON은 `schema_version:1`, `topology:single`, `target:{host,ssh_user,work_dir}`, `observed_home_dir`,
   `hint_clone_dir`, `git_dir`, `git_common_dir`, `observation_source`, `observed_utc`를 가진다.
   메인은 `status=observed`를 확인한 원 관측을 불변 보존하고 위 9개 키만 새 scope 입력에 선택한 뒤 기존 승인의
   `approved_by`, `approved_utc`, `evidence`를 연결한다. `status`·`target_source`·`observed_hostname`·`observed_user`는
   원 관측에만 남긴다(renderer 입력은 정확한 키 집합을 검증한다). 시각은 UTC `Z` 형식이다. 승인을 자동 생성하거나 재질의하는 필드가 아니다.
   host/ssh_user는 manifest의 정확한 대상과 결속하고 hostname/로그인명 실측과 혼동하지 않는다.
   경로는 서브가 해소한 절대경로이며 메인 HOME·로컬 존재 여부로 추측하지 않는다. 토큰/전체 env 내용은 회수하지 않는다.
   fresh 설치에 clone 자체가 없고 선택한 하네스도 그 외부 쓰기를 못 하면 §2.5.1의 기존 장착 도구를 사용한
   **일반 사용자 초기화 request**로 exact 경로/프로젝트 원격을 먼저 장착한다. sudo나 전체 HOME 쓰기 권한은 필요 없다.
   없는 git-dir를 관측값처럼 만들거나 다른 provider의 초기화를 Codex 성공으로 세지 않는다. 장착 후 실제 경로를 회수한다.
3. **렌더와 전달:** 두 provider 설정을 같은 역할/명시 범위에서 파생한다. 공식 `sync_to_sub.sh`에도
   동일한 `--hint-write-scopes`를 넘긴다. multi에 그 입력을 쓰거나 전체 HOME·시스템 루트·workspace 상위를
   쓰기 허용하는 것은 거부한다. scope 미지정은 과거 렌더 호환일 뿐 single hint 기능 준비의 증거가 아니다.
   일반 정책 렌더·오버레이는 기존 승인된 전달 경로이며 sudo 설치가 아니다. 사용자 설정/managed/자격증명을 복사하지 않는다.
   메인 `vllm-main`도 승인된 로컬 git·하네스 진입점(`.agents`·`.codex`) 쓰기와 A2A SSH에 필요한 network 접근을 선언한다. network 활성화는
   원격 side effect 승인이나 domain filtering이 아니며, 사용자/managed 정책과 도메인 승인 경계는 계속 적용된다.
4. **환경 전제조건:** 대상 main/sub 각각 CLI 지원, 인증, 실제 프로젝트 신뢰/설정 로딩, 명령 기동을 확인한다.
   Codex named 선택을 legacy `sandbox_mode`가 덮거나 managed 정책이 거부하면 request로 표면화한다.
   샌드박스 기동이 실패하면 `templates/requests/codex_linux_sandbox.md`에서 관측한 환경에 맞는 새 request를 발행한다.
   Ubuntu의 bwrap 사례를 모든 OS에 설치하지 않는다. 기존 정책을 덮거나 전역 userns 제한을 끄지 않는다.
5. **discovery → permissions:** `bootstrap_canary.py`의 두 phase를 요청한 provider/model로 실행한다.
   target `main`은 해당 명시 workspace에서 local, `sub`는 manifest SSH 경로다. Codex permissions phase는 선택한 named
   profile을 쓰고, 기존 Claude 호출은 native 계약을 유지한다. 단순 최종 답변 대신 실제 tool receipt와 probe 관측을 검증한다.
6. **완료와 인계:** native 성공·task-report schema·업무 기능·집행 관측을 구분해 testlog에 남긴다.
   증거 부재·미실행·권한 거부는 완료가 아니다. completion evidence는 기존 `evidence_publisher`/`completion_gate`가
   소유하고 새로운 ready flag를 만들지 않는다. 설치 성공만으로 캠페인·실제 hint 발행·분산 성능을 완료 처리하지 않는다.

## 카나리 실행과 증거

공개 진입점 `bootstrap_canary.py --help`가 옵션 정본이다. `--phase discovery`는 읽기/발견,
`--phase permissions`는 승인된 가역 업무 확인이다. `--target main|sub`를 명시하고 예산도 선언한다.
Codex는 wall-time/dispatch, Claude는 native-turn/time 계약을 사용하며 실제 argv는 provider adapter만 만든다.
permissions의 `harness_permission_probe.py`는 요청된 native 하네스 **안에서** 실행한다. 운영자 셸에서 직접
같은 helper를 실행한 결과를 그 하네스의 성공으로 세지 않는다. 읽기만 하는 가짜 회신·누락된 도구 결과·다른
대상의 probe·schema 오류는 통과하지 못해야 한다. 관측 결과는 실제 사용한 실행 표면과 함께 남긴다.

카나리는 모델 weights를 로드하거나 컨테이너를 기동하거나 원격 태그를 쓰지 않는다. 기존 정본 파일의
보호를 검사할 때 내용 쓰기·교체는 하지 않는다. 직접 OS open 결과는 Claude의 native Edit/Write deny를
증명하지 못한다. 사용자/managed 정책 전체의 동등성과 Docker/Ray 원격 동작은 별도 실제 업무 증거가 필요하다.
중단·재개·세션 결속은 기존 provider/relay 검사와 실제 종료 회수로 확인하고 새 provider로 성공을 대체하지 않는다.

## 장시간 캠페인과 승인 지속성

승인 팝업의 저장과 업무 승인의 권위는 구분한다. Claude의 로컬 tool 규칙은 `.claude/settings.local.json`,
Codex의 native 재승인 생략 규칙은 `$CODEX_HOME/rules/default.rules`(기본 `~/.codex/rules/default.rules`), 프로젝트의 실행 기본값은
`.codex/config.toml`이 소유한다. 한 provider의 승인 이력을 다른 파일에 복사하거나, 셸 prefix 허용을
다운로드·sudo·태그 발행의 도메인 승인으로 재해석하지 않는다. 기존 유효 업무 승인은 캠페인 아티팩트에서
복구한다. 하네스 전환·새 세션·이미 승인된 단계의 재개만을 이유로 같은 승인을 다시 받지 않는다.

main/sub의 프로젝트 기본 `approval_policy = "never"`는 승인된 장기 업무에서 native 도구 팝업 대기를
없애는 설정이다. 권한을 추가하거나 실패를 성공으로 바꾸지 않는다. 부족한 파일·network 범위, 신뢰 미로딩,
호스트 sandbox 기동 실패는 계속 실패하며, 메인 소유 설정 수리와 실제 호스트 request를 구분해 처리한다.
메인은 공통 하네스의 저작 owner이므로 `.agents`·`.codex` 쓰기가 양성대조이고 서브에서는 같은 경로의
거부가 음성대조다. main의 두 경로 쓰기 성공만으로 외부 쓰기 격리 전체를 증명했다고 하지 않는다.

다음 세션의 준비는 다음 관측으로 판단한다.

1. 새 native 프로세스의 `config/read`·`permissionProfile/list`에서 신뢰된 project layer, 역할별 named
   profile, `approval_policy`와 legacy override 여부를 확인한다. 사용자/managed 계층을 덮지 않는다.
2. 실제 선택된 profile의 명령 기동과 역할별 업무 카나리를 수행한다. 메인의 외부 hint clone·git common
   directory·branch worktree가 필요하면 실제 경로와 기존 승인에 결속한 권한을 별도로 확인한다. 전체 HOME
   쓰기를 대신 열거나 단순 설정 로딩을 실행 성공으로 세지 않는다.
3. 대화 중 `/permissions` 변경은 기존 보조 세션·별도 `codex exec`·다음 새 세션까지 갱신되었다는 증거가
   아니다. 남아 있는 보조 세션의 실제 선택을 확인하고, 변경 전 선택으로 정지한 세션은 인계 자료를 보존한
   후 현재 선택을 받는 새 세션으로 재위임한다. 원격 실행은 기존 relay 세션 결속·복구 가드를 유지한다.
4. 승인된 실행의 주기 관측과 기술적 정정은 메인이 맡는다. 새 범위/예산/실제 사용자 선택이 필요한 경우만
   기존 HITL 계약으로 올린다. `never` 때문에 숨은 실패를 무한 재시도하거나 사용자에게 같은 도구 승인을
   반복 요청하지 않는다. 긴 실행의 자동 재개는 기존 감독 스텝·예산·복구 가드에 종속되며 무제한 재개가 아니다.

이 준비는 다음 세션의 기본값과 관측된 기능에 대한 증거다. 아직 실행하지 않은 2일 캠페인의 무중단 완료나
인증 만료·호스트 장애까지 해결했다는 보증으로 확대하지 않는다.

## 사람 수행 request

스킬 개정이 곧 sudo 설치 필요라는 뜻은 아니다. **확인된 시스템 전제조건 결손**에만 sudo 설치가 필요하다.
메인이 원인·대상·영향·핀·검증·실패 분기·회수물을 갖춘 request를 발행하고 사람이 대상 노드에서 수행한다.
설치자는 기존 파일/정책 충돌을 덮지 않으며 부분 실패를 보존해 보고한다. 변경의 회수/롤백도 실제 생성물과
현재 사용 여부를 확인한 별도 절차다. 무조건 삭제·자동 rollback하지 않는다.

Ubuntu 24.04 bwrap request는 정상 기동이면 설치를 건너뛰고, 이미 확인된 userns/AppArmor 결손일 때만
고정 Ubuntu 패키지에서 전용 프로필을 추출해 신규 추가한다. 이는 `/usr/bin/bwrap` 사용자 전체에 영향을 주는
호스트 변경이다. `/usr/bin/true` 성공 뒤에도 실제 named 권한·A2A 카나리가 남는다. 회수 전에는 미완료 상태를 보존한다.

공식 계약: [Codex permissions](https://learn.chatgpt.com/docs/permissions),
[Codex Linux prerequisites](https://learn.chatgpt.com/docs/sandboxing#prerequisites),
[Claude sandbox](https://code.claude.com/docs/en/sandboxing).
