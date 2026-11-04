# Rust 설치·빌드·기동 — §2.5.2

새 Linux PC와 기존 노드의 Rust pin 변경·도구 결손을 이 절차로 처리한다. HW Flag와 독립된 하네스 실행환경이다.
main 및 등록된 sub가 각자 실행하며, topology별 서브 역할·기존 승인·캠페인·native 권한을 보존한다.
버전·컴포넌트·지원 target의 정본은 `scripts/rust_toolchain.pin`이다.

## 현재 노드 준비

프로젝트 루트에서 실행한다. `--runtime-home DIR`은 필요한 경우에만 지정한다.

```bash
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh status
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh install
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh check
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh exec -- cargo --version
```

`status`는 설치된 도구를 읽어 missing/partial/stale/foreign/ready와 원인을 반환한다.
도구 부재는 exit 3이며 검사 단계의 정상 안내다. 설치·복구는 명시 `install`만 수행한다.
`install`은 공식 rustup을 HTTPS로 내려받아 버전 pin, minimal 프로필, rustfmt·Clippy를 준비한다.
이미 유효한 설치면 다시 내려받지 않는다. 버전 변경·중단된 설치는 같은 노드의 rustup으로 복구한다.

기본 설치 위치는 프로젝트 `.claude/runtime/rust/`다. helper는 이 아래의 CARGO_HOME·RUSTUP_HOME과 pin을
자식 프로세스에 전달하고 `RUSTUP_AUTO_INSTALL=0`으로 암묵 toolchain 설치를 막는다.
`HOME`·셸 프로필·전역 Rust 기본값·기존 인증은 수정하지 않는다. 새 shell에서 PATH를 수동 설정하거나
`source`하지 않아도 helper의 절대경로와 `exec -- <명령>`을 사용하면 같은 환경이 선택된다.
`exec`가 실행하는 사용자 명령 자체의 네트워크 사용은 그 명령의 계약을 따른다.

`check`는 버전 목록뿐 아니라 임시 std-only Cargo 프로젝트를 offline 빌드하고 native 결과를 실행한다.
fmt와 Clippy도 실행하며, 자신의 임시 probe만 정리한다. 이 검사를 통과하지 않은 노드를 Rust 준비 완료로
기록하지 않는다. 별도 ready flag를 만들지 않는다. 실패 시 실제 원인·명령·종료를 testlog에 기록한다.

## 업무 실행파일 빌드·기동

도구체인 검증 뒤에는 **그 노드에 배달된 Rust 업무 패키지**를 빌드한다. main과
`adversarial-benchmark`를 받는 a2a-agent sub는 다음을 수행한다.

```bash
bash .claude/skills/adversarial-benchmark/scripts/build_rust.sh
# build_rust.sh가 owner의 모든 native CLI 자체검사까지 수행한다.
```

owner의 build 진입점은 위 helper의 동일 관리 환경으로 `cargo build --release --locked`를 실행하고
native 자체검사를 수행한다. Cargo.lock의 공개 의존성을 첫 빌드 때 내려받을 수 있어야 한다.
캐시가 준비된 환경에서는 `build_rust.sh --offline`을 쓸 수 있으며, 캐시 결손은 실패로 반환한다.
다른 관리 runtime 위치가 필요한 검증은 `--runtime-home DIR`로 명시한다.
모든 생성물은 owner의 ignored `rust/target/`에 두고, 실행 런처는 컴파일 결과를 직접 기동한다.
일반 기동이 설치/다운로드/컴파일을 수행하거나 Python으로 대체하지 않는다.

tool_plane이 런타임 스킬 0종인 ray-worker sub는 이 업무 패키지 빌드가 적용되지 않는다.
도구체인 install/check는 그 노드에도 적용한다. 실제 전달 대상에 owner 패키지가 있어야 하는데
없거나 lockfile/빌드/기동 검사가 실패하면 해당 하네스 배포 완료로 기록하지 않는다.
std-only probe 통과는 어플리케이션 이행 완료가 아니다.

## 서브와 다른 PC 배포

새 checkout에도 helper·pin·이 참조가 있어야 한다. `terraforming_node` 전체를 서브에 복제하지 않는다.
기존 renderer의 support delivery가 helper·pin·이 참조와 generated CLAUDE 안내를 전달한다.
컴파일 결과·메인 runtime home·개인 `.cargo`/`.rustup`·인증은 배달하지 않는다.
두 topology의 서브 모두 자기 노드에서 위 install → check를 수행하고,
자기 tool_plane에 배달된 Rust 업무 패키지의 build → native 검사를 이어 수행한다.

메인 워커는 기존 인가·dry-run → apply 오버레이로 자산을 배달하고 기존 A2A 경로로 서브에 설치/검증을
지시한다. 실제 명령·pin·host·빌드/실행 결과는 서브 testlog로 발행하고 문서기반으로 회수한다.
메인 설치 성공이나 렌더 fixture 성공을 실제 서브 설치 성공으로 대신하지 않는다.
기존 HW Flag가 FRESH여도 신규 Rust 실행환경은 이 절차를 거친 뒤 하네스 배포 완료를 판단한다.

## 지원 환경과 실패

지원은 Linux GNU의 aarch64와 x86_64다. Rust 실행뿐 아니라 C 링커(`cc`)가 필요하다.
링커/curl/시스템 전제가 없으면 해당 원인과 설치할 도구를 반환하며 sudo나 패키지 관리자를 자동 실행하지 않는다.
운영자 권한 작업이 필요하면 기존 request 절차로 전제·명령·성공판정·회수물을 전달한다.
다른 OS/아키텍처·musl은 이 helper의 지원 범위 밖이며 실제 판정을 보존한다.

exit 2는 사용법/pin, 3은 설치 필요, 4는 플랫폼/호스트 전제, 5는 다운로드, 6은 설치, 7은 probe 실패다.
실패한 설치 위치는 지우지 않고 다음 명시 install에서 복구한다.
egress가 없는 노드는 운영자가 준비한 `--rustup-init-file <실행파일>`과 pin 배포 파일/허용 미러가 필요하다.
이 옵션은 installer 다운로드만 생략하며 toolchain까지 완전한 offline 번들을 제공하는 기능이 아니다.
기존 `RUSTUP_DIST_SERVER`/`RUSTUP_UPDATE_ROOT`와 proxy 환경을 상속하며 자격증명을 출력하지 않는다.

공식 계약: [rustup 설치 위치](https://rust-lang.github.io/rustup/installation/index.html),
[설치 옵션](https://rust-lang.github.io/rustup/installation/other.html),
[프로필/컴포넌트](https://rust-lang.github.io/rustup/concepts/profiles.html),
[암묵 설치 환경변수](https://rust-lang.github.io/rustup/environment-variables.html).
