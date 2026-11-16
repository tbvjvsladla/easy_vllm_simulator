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

`check`는 버전 목록뿐 아니라 임시 C 소스를 `cc`로 컴파일하고 `ar`로 정적 아카이브를 만든 뒤,
외부 Cargo 의존성 없는 Rust 프로젝트를 그 아카이브와 offline 연결해 native 함수를 호출한다.
fmt와 Clippy도 실행하며, 자신의 임시 probe만 정리한다. 이 검사를 통과하지 않은 노드를 Rust 준비 완료로
기록하지 않는다. 별도 ready flag를 만들지 않는다. 실패 시 실제 원인·명령·종료를 testlog에 기록한다.

## 업무 실행파일 빌드·기동

도구체인 검증 뒤에는 **그 노드에 배달된 Rust 업무 패키지**를 빌드한다. main과
`adversarial-benchmark`를 받는 a2a-agent sub는 다음을 수행한다.

```bash
bash .claude/skills/adversarial-benchmark/scripts/build_rust.sh
# build_rust.sh는 owner 자체검사 일부(57개 중 30개 실행파일 기동)와 native_health_probes.sh(manifest_contract 비공개 브리지 manifest_tp 2건 · 메인 평면의 turn_budget --self-test)를 수행한다. 전수 기동이 아니다.
bash .claude/skills/terraforming_node/scripts/build_rust_packages.sh
# 배달된 공유 Q/T library와 recipe P/upstream U 정상 release 실행파일을 빌드하고 순수 native CLI를 기동한다.
```

owner의 build 진입점은 위 helper의 동일 관리 환경으로(상속된 `EASY_VLLM_SOURCE_ROOT` 는 비운다 — 프로토타입 빌드는 cargo 를 명시 실행한다) `cargo build --release --locked`를 실행하고
위 자체검사와 native 건강 probe 를 수행한다. 어느 probe 가 실제로 돌았는지는 출력(`[native_probe] RAN/SKIPPED/NOT claimed`)에 남는다. Cargo.lock의 공개 의존성을 첫 빌드 때 내려받을 수 있어야 한다.
캐시가 준비된 환경에서는 `build_rust.sh --offline`을 쓸 수 있으며, 캐시 결손은 실패로 반환한다.
다른 관리 runtime 위치가 필요한 검증은 `--runtime-home DIR`로 명시한다.
Q/T/P/U helper도 `--offline`·`--runtime-home DIR`을 받는다. 실제 manifest의 Cargo metadata에서
required-features 없는 bin 목록을 파생하고 ELF·실행비트를 검사한다. 공유 Q는 B/P/U를 받는 평면에, 역할 정책 T(`.claude/rust/role-contract`)는 B/U를 받는
평면에 소스로 동행한다. B만 배달돼도 Q/T가 필요하다. wiki-only ray-worker에는 이 업무
라이브러리를 배달하지 않는다. tool_plane에 있는 owner의 manifest나
path dependency가 없으면 빌드·배달은 실패한다. B helper의 자체검사 단계는 유지한다.
모든 생성물은 각 package의 ignored `target/`에 두고, 실행 런처는 컴파일 결과를 직접 기동한다.
일반 기동이 설치/다운로드/컴파일을 수행하거나 Python으로 대체하지 않는다.

기존 `target/`이 있는 노드에 소스를 시각 보존 복사·오버레이한 경우에는 Cargo가 오래된 산출물을
최신으로 볼 수 있다. 새 모듈이 소스에 있는데 빌드가 모듈 부재로 실패하는 등 실제 소스와 빌드 결과가
어긋나면 첫 오류와 파일 시각을 보존하고, 영향 package의 release 산출물만 정리한 뒤 명시 빌드를 한다.
예를 들어 B의 `sweep-stop` 캐시를 정리하는 명령은 다음과 같다.

```bash
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh exec -- \
  cargo clean -p sweep-stop --release \
  --manifest-path .claude/skills/adversarial-benchmark/rust/Cargo.toml \
  --target-dir .claude/skills/adversarial-benchmark/rust/target
bash .claude/skills/adversarial-benchmark/scripts/build_rust.sh --offline
```

새 checkout이나 시각 문제가 없는 빌드에는 이 정리가 필요하지 않다. 다른 owner는 해당 package·manifest·
target 경로를 사용한다. 소스·lockfile·노드의 의존성 cache나 전역 Rust 설정을 지우지 않는다. 재빌드 뒤에도
실패하면 그 원인을 반환하며, 일반 기동에서 자동 정리·재시도·Python 대체를 하지 않는다.

B의 노드 식별자 호출에 필요한 `node_identity_resolve`도 같은 `build_rust.sh`가 정상 실행파일로 함께
빌드한다. 새 PC와 sub에는 helper의 소스를 B 및 Q/T와 함께 전달하고 각 노드에서 컴파일한다.
helper 부재 오류는 이 명시 빌드 경로로 복구한다. 기존 `node_identity.sh`는 installer의 source 및
다른 소비자를 위한 호환 경로로 유지한다.

토폴로지 해소도 같은 B 빌드에서 `topology_parity`·`topology_parity_api`를 일반 실행파일로 함께 만든다.
`resolve_topology.sh`·`resolve_topology_api.sh`의 기본 호출은 이 노드에서 빌드한 Rust helper를 사용한다.
helper가 없으면 명시 빌드 필요 오류로 반환하며, 일반 기동이 설치·컴파일·Python 대체를 시작하지 않는다.

T의 source identity는 기존 정본 `.claude/policies/runtime/topology_parity.py`와
`.claude/skills/terraforming_node/scripts/campaign_init.py`를 컴파일 문맥으로 읽는다. 이 두 파일도
기존 공식 source overlay에 동행해야 하며, 별도 호환 사본이나 메인 ELF를 배달하지 않는다.
검토된 source와 동일한 경로는 Rust로 판정한다. 다른 source-version을 실제 원본으로 호출하는
호환 경로는 선택된 Python과 그 원본의 PyYAML 의존성이 필요하다. 호환 인터프리터는 기존 노드
Python 환경의 `python3`이며 필요한 경우 `RESOLVE_TOPOLOGY_COMPAT_PYTHON`으로 명시한다.
`RESOLVE_TOPOLOGY_PYTHON`을 비어 있지 않게 지정하면 기존 명시 Python 호출을 선택하므로,
새 PC의 기본 Rust 기동을 위해 이 값을 자동으로 `python3`으로 채우지 않는다.

반복 수와 프리셋의 캠페인 조회도 같은 B 빌드의 `repeat_axis`·`repeat_axis_api`·`bench_preset`·
`bench_preset_api` 실행파일을 사용한다. 네 shell 진입점은 기본 adapter 위치를
`REPEAT_AXIS_COMPAT_ADAPTER`로 전달하며, 검토된 정본 `campaign_init.py`의 조회는 공유 T에서
Rust 프로세스 안에 수행한다. 다른 소스나 읽을 수 없는 소스는 그 노드의 기존 Python과
`repeat_axis_owner_query.py`를 사용하는 호환 경로로 처리한다. 이 adapter 소스도 공식 배달에 포함한다.
새 PC의 기본 Rust 기동을 위해 `REPEAT_AXIS_CAMPAIGN_ADAPTER`나 `REPEAT_AXIS_PYTHON`을 자동으로
채우지 않는다. 사용자가 지정한 비어 있지 않은 adapter 또는 값이 지정된 Python 변수는 명시적
호환 경로를 선택한다. Python 변수의 빈 값도 지정으로 보며 기존 기동 실패를 반환한다.
Rust 기본 경로의 오류가 발생하면 Python으로 재시도하지 않는다. 임시 검증 probe는 배포하지 않는다.

`campaign_init.py`의 읽기 전용 두 형태(`--active`, `--derive KIND [--cell|--sweep|--context|--campaign-id VALUE]...`)는
같은 B 빌드의 `campaign_read` 실행파일이 처리하며, 호출부(`broad_search.sh`·`fetch_sub_docs.sh`의 네 지점)는
`terraforming_node/scripts/campaign_init.sh` 런처를 부른다. 런처는 실행파일이 없거나 실행 불가면 어떤 경로
선택보다 먼저 종료 상태 2와 빌드 안내를 반환하며(명시 Python 지정이 있어도 Python을 시작하지 않는다),
`CAMPAIGN_READ_COMPAT_OWNER`로 옆의 `campaign_init.py`를, 비어 있지 않은 `CAMPAIGN_READ_PYTHON`만 명시 의도로
전달한다. 정확한 ASCII 문법이 아닌 호출·명시 인터프리터·다른/읽을 수 없는/없는 소스·모델링되지 않은 텍스트
프로필은 실제 `campaign_init.py`를 `exec`로 실행하는 호환 경로(`CAMPAIGN_READ_COMPAT_PYTHON`, 기본 `python3`)이며
이 경로는 업무 처리 이후의 대체가 아니다. 네이티브 처리가 시작된 뒤의 거절·미처리 예외(전체 traceback)·인코딩 실패·
쓰기 실패는 Python으로 재시도하지 않는다. `CAMPAIGN_READ_ROUTE_OBSERVER=<파일>`은 실행마다 경로·결과·PID를 JSON 한 줄로
업무 출력 밖에 남긴다. 다른 `campaign_init.py` 호출(writer 전부)은 변하지 않는다.

`manifest_contract.py`의 명령줄 게이트 판정(`--topology single|multi`·`--repo VALUE`·`--require-flag`·`--json`, 분리 토큰·임의 순서·각 1회)은
같은 B 빌드의 `manifest_contract` 실행파일이 처리하며, 호출부(`run_bench.sh`·`lite_bench.sh`의 Flag 게이트 두 지점)는
`terraforming_node/scripts/manifest_contract.sh` 런처를 부른다. 두 호출부는 게이트 판정 전에 `MANIFEST_CONTRACT_VALIDATE_ONLY=1`로
가용성을 한 번 확인하고, 런타임이 없으면 런처의 안내를 보이며 종료 상태 2를 전파한다(게이트 호출은 출력과 상태를 버리고 비0을
모두 info-only 4로 읽으므로 부재가 "Flag 미발급"으로 가려지지 않게 한다). 런처는 `MANIFEST_CONTRACT_COMPAT_OWNER`로 옆의
`manifest_contract.py`를, 비어 있지 않은 `MANIFEST_CONTRACT_PYTHON`만 명시 의도로 전달하고 `MANIFEST_CONTRACT_COMPAT_PYTHON`(기본
`python3`)이 다른 호환 경로의 인터프리터다. 경로 선택(`LegacyExplicit`·`LegacyShape`·`LegacySourceVariant`·`LegacyProfile`·
`NativeCurrentSource`)은 업무 처리 전에만 정해지고 `MANIFEST_CONTRACT_ROUTE_OBSERVER=<파일>`이 실행마다 한 줄을 남긴다. 네이티브
처리가 시작된 뒤에는 Python으로 재시도하지 않는다 — 거절·미처리 예외 traceback·인코딩/쓰기 실패는 네이티브가 답하고, 캡처된 원본이 없는
형태는 숨은 대체 없이 `[manifest_contract] ERROR unsupported shape …` 종료 상태 2로 드러낸다. 프로세스 안에서 `manifest_contract.py`를
import하는 사용자(`render_sub_env`·`scan_node`·`render_dockerfile`·`harness_permission_probe`·`publish_install_request`·claim predicates·hint evidence)는
이제 **얇은 import 어댑터**를 import 한다: 판정은 같은 `manifest_contract` 실행파일의 비공개 브리지(`MANIFEST_CONTRACT_BRIDGE`, 타입 그래프 JSON)가 하고,
어댑터는 타입 전달·결과 계획의 기계적 조립·예외 재구성만 한다(`_load_manifest` 는 원본처럼 호출 시점에 `import yaml`). 그러므로 이 Rust 빌드(+ `manifest_contract.sh` 와
`manifest_contract_compat_owner.py`)는 **노드의 import 사용자에게도 필수 의존**이며 HW 스캔·Flag 와 무관하다(Flag 가 없어도 import 가 브리지를 부른다). 실행파일이 없으면
어댑터는 상태 2 의 빌드 안내와 함께 눈에 띄게 실패한다(Python 대체 경로 없음). 검토된 원본은 `manifest_contract_compat_owner.py` 에 바이트 그대로 있고(`REVIEWED_MANIFEST_CONTRACT`
가 이 파일에 묶인다) 사전 분류된 레거시 경로(CLI 의 `-h`·`--self-test`·약어·사용 오류, 어댑터 `main` 의 같은 형태)만 쓴다. `verify_distribution` 의 호출은 어댑터 파일 경로를 CLI 로 쓴다.
**공개 진입 사슬(2026-11-09 v8)**: `python3 manifest_contract.py …` 는 `__main__` 이 **같은 PID 로 Rust 실행파일에 직접 exec** 한다(Python → Rust · 셸 비경유 — 호출자가 cwd 를 지운 뒤에도 셸의 `shell-init`/`chdir` getcwd 경고 바이트가 stderr 에 끼지 않는다). 환경은 `manifest_contract.sh` 가 만드는 것과 같다(운반 변수 · 기본값 · unset 규칙). 실행파일 부재/실행 불가(상태 2 + 빌드 안내)·`MANIFEST_CONTRACT_VALIDATE_ONLY=1` 가용성 확인·직접 exec 실패는 **런처가 단일 자리로 답한다**(exec 실패 자체의 안내는 없다 — 셸의 `cannot execute` 상태 126/127 그대로). 독립 `manifest_contract.sh` 호출은 Bash → Rust 이고, 프로세스 안 import 어댑터의 비공개 브리지는 절대 경로 `/bin/bash` 로 런처를 시작한다. 노드에서 빌드한 ELF 를 복사해 배포하지 않는다.

`turn_budget.py`(턴 예산 선언 검증기)의 업무는 같은 B 빌드의 `turn_budget` 실행파일(순수 업무는 공유 T `role-contract::turn_budget`)이 처리한다. 공개 CLI 는 `terraforming_node/scripts/turn_budget.sh` 런처(또는 그것을 exec 하는 `turn_budget.py`)이고, 기존 Python importer 세 곳(`relay`·`bootstrap_canary`·`selftest_harness_switch`)은 같은 이름·서명의 얇은 어댑터 `turn_budget.py` 를 import 하며 어댑터는 비공개 브리지(`TURN_BUDGET_BRIDGE`, 타입 태그 JSON · 정수 16진 전송)로 같은 실행파일을 부른다. 실행파일이 없거나 실행 불가면 런처가 상태 2 와 빌드 안내로 **먼저** 실패하며 Python 대체 경로는 없다(검증된 원본 소스의 사본을 숨은 대체로 실행하지 않는다). 소유자 경로 미설정·모델링되지 않은 텍스트 프로필은 업무 전에 보이는 제어 실패(상태 2)이고 `TURN_BUDGET_ROUTE_OBSERVER=<파일>` 이 실행마다 경로·PID 를 JSON 한 줄로 남긴다. 기본 스키마는 `TURN_BUDGET_OWNER_PATH`(어댑터가 `os.path.abspath(__file__)` 로 내보냄)의 어휘적 위 네 단계이며 호출마다 다시 읽는다. 어댑터가 전달하지 않는 값 모양(정확한 기본 타입·문자열 키 dict·`os.PathLike` 밖의 객체, int 경로)은 업무 전에 `NotImplementedError` 로 거절한다. in-process 예외의 traceback 바이트(U2)는 재현하지 않으며, 세 importer 를 포팅하기 전까지 어댑터는 잔여 Python 이다.

`scan_node.py` 의 **P1 단계**(순수 판정·manifest 조립·파일 판독 공개 16개 이름과 도달 가능한 헬퍼 4개)는 같은 B 빌드의 `scan_node` 실행파일(순수 업무는 공유 T `role-contract::scan_node`·`scan_node_manifest`·`scan_node_files`)이 처리한다. 실행파일은 **비공개 브리지**(`SCAN_NODE_BRIDGE`, 타입 태그 JSON · 정수 16진 전송 · 호출자 객체의 공유 동일성 plan: 요청의 번호 붙은 객체(`id`/`ref`)와 응답의 `argref`)와 자기 검사 `--p1-self-check` 만 가진다(브리지 환경도 자기 검사 인자도 없는 시작은 상태 3 의 보이는 제어 실패). 기존 Python importer(현재 `claim_predicates` 가 `emit_manifest_block`·`read_manifest_field`·`evaluate_gate`·`emit_gate` 를 부른다)는 같은 이름·서명의 얇은 어댑터 `scan_node.py` 를 import 하고, 어댑터는 `terraforming_node/scripts/scan_node.sh` 런처로 같은 실행파일을 부른다. 어댑터는 호출자의 실제 객체를 번호 붙여 보내고 응답 plan 의 `argref` 를 그 실제 객체로 되돌려 조립할 뿐이다(반환값이 입력 객체인 관계 · NaN 동일성 · `AttributeError.name/obj` 가 호출자 객체를 가리키는 관계가 원본과 같다). 업무 선택은 Rust 에만 있다. 런처는 실행파일이 없거나 실행 불가·ELF 가 아니면 상태 2 와 빌드 안내로 **먼저** 실패하며 P1 의 Python 대체 경로는 없다. 실행파일 복사·일반 시작 중 암묵 빌드는 없다. 두 경계는 명시 인자다: 파일은 실제 OS(errno·메시지 그대로 `ManifestUnreadable` 의 `OSError` 원인으로), 시계는 시스템 로컬 시간대의 `%Y%m%d%H`(`localtime_r`; `datetime.now()` 와 같은 원천). `SCAN_NODE_ROUTE_OBSERVER=<파일>` 은 브리지 호출마다 `start`(업무 전)·`end`(결과) 두 줄(실제 pid·ppid·선언된 소유자 경로·route·shape·outcome)을, compat 경로는 같은 형식으로 `CompatOriginalLoad`(이름공간 배관)·`CompatOriginalCli` 를 남긴다. **P1 만의 이행이다 — 전체 스캐너도 Python-free 도 아니다.** P1 밖(하드웨어 프로브·SSH·네트워크 수집·argparse `main`·`_self_test`)은 정확한 원본 소유자 `scan_node_original.py` 로 가는 명시적 compat 경로다: 어댑터를 올릴 때 그 소유자를 모듈 이름 `scan_node` 로 실행해(프레임의 파일은 원본 그대로) 공개·비공개 이름을 이 모듈에 배관하므로 `from scan_node import *` 를 포함한 보통의 export 가 원본과 같고, `ManifestUnreadable` 은 공개 클래스 하나다(compat 전역에도 같은 클래스). 원본 소유자 안의 P1 함수는 원본 Python 그대로 돈다 — compat 의 전체 CLI·`--self-test` 는 P1 네이티브의 증거가 아니다. 네이티브 호출이 실패하면 Python 으로 재시도하지 않는다. **검사**: 네이티브 P1 은 `scan_node.sh --p1-self-check`(내장된 저장 raw 107 행 · `build_rust.sh` 가 빌드 뒤 실행 · `verify_distribution` 의 `terraform_scan_p1_native_selfcheck`)가 확인하고, `verify_distribution` 의 `terraform_scan_selftest_legacy` 는 원본 소유자의 레거시 자기시험일 뿐이다. 정책 문구 바인딩은 P1 문구는 T Rust 소스(`scan_node_manifest.rs`), 미포팅 문구는 `scan_node_original.py` 를 읽는다. 어댑터가 전달하지 않는 값(정확한 기본 타입·문자열 키 dict 밖의 객체, `os.PathLike`, int 경로=fd, 자기 참조)은 업무 전에 `NotImplementedError` 로 거절한다. 잔여: in-process 예외의 traceback 프레임(U2 · 열림 · 위조하지 않음) · 딕셔너리 키 문자열의 객체 동일성.

호출부의 가용성 확인: `CAMPAIGN_READ_VALIDATE_ONLY=1`은 런처의 조건부 환경 변수로, 같은 ELF·실행 비트 검사 뒤에
네이티브·Python 업무 없이 0을 반환한다(값 1일 때만 · 다른 값과 모든 인자 형태는 종전 경로). 호출부 둘(`broad_search.sh`의
`cell` 분기, `fetch_sub_docs.sh`)은 첫 캠페인 읽기·부작용(서빙·측정·서브 접속) 전에 이것을 한 번, stderr 를 버리지
않고 호출하며 실패 상태를 그대로 전파한다. 런타임이 있으면 이후 업무 오류·대체 처리는 종전과 같다. 이는 가용성 확인이며
건강 증명이 아니고, 실행파일 경로를 호출부에 복제하지 않는다.

역할 정책 T는 Q에만 의존하며, B와 U가 같은 정책 라이브러리를 사용한다. U 빌드는
`node_role_contract`와 `upstream_role_api`를 포함한 일반 실행파일을 Cargo metadata에서 파생한다.
메인의 직접 런처는 다음과 같다.

```bash
bash .claude/skills/terraforming_node/scripts/node_role_contract.sh --help
```

U를 배달받은 서브는 전체 terraforming 스킬을 받지 않아도 자기 노드에서 빌드한 실행파일을 기동한다.

```bash
.claude/skills/upstream-version-watch/rust/runtime/target/release/node_role_contract --help
```

이 직접 기동은 설치·빌드·Python 역할 판정으로 대체하지 않는다. 최초 install/check/build가
native 역할 실행파일을 선행 요구하지 않도록 기존 Python bootstrap/import 경로는 유지한다.
Rust 역할 판정과 bootstrap 호환 경로의 적용 범위는 해당 과업의 검증 결과에서 구분한다.

HTTP/TLS를 쓰는 Rust 업무 의존성도 같은 Cargo.lock과 노드별 관리 cache에 포함한다.
첫 공개 의존성 fetch와 업무 중 upstream 조회는 별개 실행으로 기록한다. main의 cache나 인증을
새 PC·sub에 복사하지 않으며, 명시 첫 빌드에서 필요한 의존성을 준비한 뒤 offline 빌드를 확인한다.
TLS는 그 노드의 신뢰 저장소와 해당 프로세스에 선언된 CA/proxy 환경을 사용한다. 임시 검증 CA는
검사 자식 환경에만 전달하고 전역 trust나 TLS 검증을 바꾸지 않는다. 단위 self-test의 주입 응답은
실제 HTTPS 기동 증거가 아니므로 업무의 실제 HTTP 경로를 별도로 확인한 범위를 testlog에 남긴다.

배포 소스의 HTTPS 회귀 검사는 실행 시 임시 CA를 만드는 `openssl` 실행파일과 loopback 소켓을 사용한다.
새 PC·sub에서 이 검사를 수행하기 전에 `openssl version`을 확인한다. 결손이면 기존 호스트 전제조건
request 절차로 필요한 도구를 준비하고 실제 검사를 회수한다. Rust 업무 실행파일의 운영 TLS는 Rust로
동작하며 OpenSSL 개발 라이브러리나 Python 실행을 런타임 대체로 요구하지 않는다.

tool_plane이 런타임 스킬 0종인 ray-worker sub는 이 업무 패키지 빌드가 적용되지 않는다.
도구체인 install/check는 그 노드에도 적용한다. 실제 전달 대상에 owner 패키지가 있어야 하는데
없거나 lockfile/빌드/기동 검사가 실패하면 해당 하네스 배포 완료로 기록하지 않는다.
Rust/C probe 통과는 어플리케이션 이행 완료가 아니다.

## 명령 순서와 ELF 의존 (새 클론)

Rust 설치(`rust_bootstrap.sh install|check`)는 어떤 검증기·Flag·ELF 에도 의존하지 않는다 — 순환이 없다. 호스트에는 `/bin/bash` 와 이 helper 가 직접 부르는 외부 명령이 있어야 한다: coreutils(`uname`·`head`·`tr`·`cat`·`env`·`realpath`·`mktemp`·`mkdir`·`rm`·`chmod`) · `grep` · `sed` · `awk`, 그리고 install 의 `curl`·`sha256sum`, check/설치 링크의 `cc`·`ar`(`ldd` 는 있을 때만 musl 판별에 쓴다). 최소 호스트라는 주장이 아니라 **실제 선행조건 목록**이며, 쓰기 직전의 가드가 부족한 도구를 이름으로 알리고 rc 4 로 멈춘다(호스트 전수 스캔은 하지 않는다). 스크립트 위치는 `dirname` 없이 bash 내장(`BASH_SOURCE`·`cd`·`pwd`)으로 해소하며 `/` 직하 절대경로도 처리한다 — 이 규약은 `rust_bootstrap.sh`·`build_rust.sh`·`build_rust_packages.sh`·`native_health_probes.sh` 와 네 launcher(`scan_node.sh`·`turn_budget.sh`·`campaign_init.sh`·`manifest_contract.sh`)에만 적용된다(기존 다른 스크립트는 그대로 `dirname` 을 쓴다). 표의 "ELF 필요" 명령은 해당 노드에서 정상 빌드를 마친 뒤에만 쓴다. 없으면 상태 2 의 빌드 안내(또는 어댑터의 `RuntimeError`)로 눈에 띄게 실패하며 Python 대체는 없다.

| 단계 | 명령 | Rust ELF |
|---|---|---|
| 진입 | `scan_node.py --topology single\|multi [--emit-manifest]` · `node_role_contract.py` · `topology_parity.py` | 불필요(Python / 원본 소유자) |
| 진입(조건부 preflight) | `staleness_gate.sh` | **terraforming_node/rust**(`staleness_gate` ELF) |
| 설치 | `rust_bootstrap.sh install → check` | 불필요 |
| 빌드 | `build_rust.sh`(B) → `build_rust_packages.sh`(Q/T 라이브러리 · P · U) | 산출 |
| 서브 manifest·렌더 | `scan_node.py --emit-sub-manifest` · `render_sub_env.sh --sub-manifest\|--self-test` · `render_dockerfile.sh` · `publish_install_request.sh` | **manifest_contract**(메인) |
| 카나리·위임 | `bootstrap_canary.sh --phase discovery\|permissions` · `relay.sh` | **turn_budget**(메인) · **terraforming_node/rust**(`bootstrap_canary` ELF) |
| 벤치 Flag 게이트 | `run_bench.sh` · `lite_bench.sh` (`manifest_contract.sh`) | **manifest_contract**(그 노드) |
| 정책·배포 검사 | `claim_predicates.py`(모듈 적재는 ELF 없이 되나 술어 실행은 필요) · `verify_distribution.py` 의 render/turn_budget/relay/canary/scan P1 자체검사 | **manifest_contract · turn_budget · scan_node** |
| pre-commit 훅 | `.claude/hooks/pre-commit` → `runtime_selftest.sh --tripwires-only` (crate `.claude/rust/policy-selftest` 의 `runtime_selftest` ELF · 2026-11-16 Rust 이식 — Python 을 띄우지 않는다) | **policy-selftest**(`build_rust_packages.sh` 정상 빌드) — 훅 배선(onboarding 0.5.9)은 그 노드의 정상 빌드 뒤에 한다. 미빌드 노드에서 훅을 켠 채 커밋이 거부되는 것은 정상 차단이며 `--no-verify` 로 우회하지 않는다 |
| 서브 권한 카나리 | `harness_permission_probe.sh` | **terraforming_node/rust**(그 노드 · `harness_permission_probe` ELF) — 2026-11-16 Rust 이식: manifest·compose YAML 은 ELF 가 직접 읽는다(예전의 배달 Python 리더 `manifest_contract_compat_owner.py` 실행·무결성 핀은 실행할 Python 이 없어져 걷혔다). ELF 가 없거나 실행 불가면 런처가 status 2 + 빌드 안내로 멈춘다 — `unobserved` 로 가리지 않는다 |

wiki-only ray-worker(`tool_plane`=wiki-desk)는 B/Q/T/P/U 를 받지 않는다: 도구체인 install/check 와 "불필요" 열의 명령만 쓰며 메인 ELF 를 복사하지 않는다. 서브의 권한 카나리 ELF(`harness_permission_probe`)가 없으면 런처는 `unobserved` 로 가리지 않고 분명한 오류(status 2)로 멈춘다.

## 서브와 다른 PC 배포

새 PC는 해당 노드에서 명시 install → check → B build_rust.sh → Q/T/P/U build_rust_packages.sh
순서로 준비한다. 등록 sub도 기존 공식 overlay/A2A 경로로 배달된 소스에 이 두 build를 직접 수행하고
결과를 반환한다. 이미 검증된 byte동일 B 결과는 과업의 영향 판정에 따라 재사용한다. 각 helper의
첫 공개 의존성 fetch는 별도 실제 실행으로 기록하고, 캐시 준비 뒤에는 `--offline`을 쓴다.
컴파일 ELF·cache·toolchain·인증·사용자 shell 설정을 소스 배달에 복사하지 않는다.

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

지원은 Linux GNU의 aarch64와 x86_64다. Rust 실행뿐 아니라 C 컴파일러·링커(`cc`)와 정적 아카이브 도구(`ar`)가 필요하다.
해시 같은 native C/어셈블리 의존성도 각 노드에서 빌드하므로 컴파일러의 존재만으로 준비 완료로 보지 않는다.
컴파일러/링커/아카이브/curl 등의 전제가 없으면 원인과 설치할 도구(build-essential·binutils 등)를 반환하며 sudo나 패키지 관리자를 자동 실행하지 않는다.
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
