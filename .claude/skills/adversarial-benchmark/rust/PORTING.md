# benchmark Rust 이행 기록

범위는 기존 `scripts/sweep_stop.py`의 순수 판정·CLI와 `broad_search.sh`의 해당 호출/결과 표시다.
원본은 이행 대조 기준으로 보존한다. 다른 Python 도구·LLM provider·서빙 엔진의 이행 완료가 아니다.
이번 정지 판정 경계의 정본 호출/배치·독립 검증·전체 배포126검사를 통과했다.
좁은 화면의 도움말은 COLUMNS와 실제 stdout 터미널 폭을 반영한다.

## 기동과 빌드

Terraform `references/install/rust_runtime.md`의 노드별 install/check 후 owner의
`scripts/build_rust.sh`를 실행한다. Cargo.lock으로 의존성을 고정하고 현재 노드의 release를 생성한다.
캐시가 준비됐다면 `--offline`을 쓸 수 있다. `scripts/sweep_stop.sh`는 준비된 실행파일을 직접 기동하며
부재는 exit2와 빌드 안내다. 판정 때 설치/컴파일/Python 대체를 수행하지 않는다.

```bash
bash .claude/skills/adversarial-benchmark/scripts/build_rust.sh
bash .claude/skills/adversarial-benchmark/scripts/sweep_stop.sh --self-test
bash .claude/skills/terraforming_node/scripts/rust_bootstrap.sh exec -- \
  cargo test --locked --manifest-path .claude/skills/adversarial-benchmark/rust/Cargo.toml
```

`target/`는 로컬 생성물이며 다른 아키텍처의 노드에 배달하지 않는다. 소스·lockfile·검사 데이터·
[파생 코드 고지](notices/NOTICE.md)를 함께 배달하고 자기 노드에서 빌드한다.

## API와 보존 경계

`evaluate(&PyValue, &UtcInstant)`는 typed `Evaluation` 또는 `SweepError`를 반환한다.
`Evaluation::to_value()`는 원본과 같은 순서의 전체 출력 객체를 구성한다.
`parse_utc`는 주입 시각만 읽는다. 반복 하한은 build.rs가 실제 campaign schema에서 읽는다.

CLI의 기존 `--state PATH|-`, `--now-utc`, `--self-test`와0/3/2/1 종료를 보존한다.
추가 명령은 `report stopped-by|status-line --stop-json PATH`,
`report declared-repeats --state PATH`, `evaluate-jsonl`이다.
큰 정수·bool/float 구분·반복 부재·중립 outcome·정지4이유·오류 우선순위·JSON 순서/Unicode/개행을
원본과 대조했다. stdin/stdout의 UTF8 surrogateescape와 strict UTF8 파일 읽기를 구분한다.

호환 기준 환경은 실제 원본을 실행한 Linux GNU·CPython3.12.3의 UTF8 스트림/UCD15.0이다.
원본 JSON parse 깊이9997과 pretty 출력 깊이995, glibc의 비패딩 연도 표기를 재현한다.
다른 Python 빌드·locale·변경된 전역 interpreter 한도까지 같은 동작을 보증하지 않는다.
처리하지 않은 예외는 종료와 마지막 예외 종류/메시지를 보존하며 Python 소스 stack frame은 재현하지 않는다.
깊은 JSON을 library에서 직접 파싱하는 호출자는 충분한 stack이 필요하다. CLI는 전용 stack을 사용한다.

## 실행 근거와 잔여

후보는 Claude Sonnet5.5가 별도 임시 영역에서 작성하고 메인이 검토·정본 적용했다.
민감 정본 쓰기 보호를 변경하지 않았으며, 정본 반영을 native 편집 성공으로 귀속하지 않는다.
초회 독립 검증이 찾은 옵션·시각 구분자·깊이·바이트 I/O 차이를 수정하고 별도 Validator가 재확인했다.

정본 release에서 외부543입력(value/예외 type/message), stdin/file CLI8건을 대조했다.
기존 precheck/cell args/sweep repeats 통합3개는 실제 native 경로로 통과했다.
precheck 복사 목록 변경으로 생긴 초회 실패는 주입 대상 명시경로 수정 후 통과했고 최초 실패를 보존했다.
독립 검증은 종료0/3/2, report3종, 상태 보존/sidecar 절단, native 부재 실패와 Python 대체 부재를 확인했다.
새 공백 경로에서 소스 offline build/native 실행도 확인했다. 실제 원격 서브 배포는 별도 후속 범위다.

Cargo 검사는 `checks/`에 두며 자동 `tests/` 발견을 끈다. 현재48검사에 원본465사례,
시각26945/JSON5163/Unicode경계4234 입력과 표적 CLI/스트림/깊이 회귀가 포함된다.
자체검사와 독립 검증·전체 과업의 수용은 별개다. 전체 근거/실패/진행 상태는 같은 과업의
운영 devlog/testlog와 val_agent의 `harness2-work-policy-261104110714` 문서에 연결한다.

`sweep_stop_bench`는 typed evaluate, 전체 ordered value 구성, parse/evaluate/render를 분리한다.
전체 결과를 black_box로 소비하며 Python dict 반환과의 주 비교는 `evaluate_to_value`다.
기존 bool만 소비한 벤치 수치는 폐기했다. 동일100셀·11×200회 비교의 중앙값은
Python 전체 결과 구성21.61μs/Rust6.56μs(약3.30배), 실제 셸 런처를 포함한 CLI20.18ms/4.24ms(약4.76배)였다.
이 입력의 로컬 표본값이며 LLM/네트워크/서빙 시간이나 통계적 우월성을 뜻하지 않는다.

## R2 parse_bench 정본 통합

상태: 이 두 번째 변환 단위의 정본 통합·적용 검사 COMPLETE. 전체 과업은 계속 진행한다.
`scripts/parse_bench.py`는 대조용으로 보존하고, 실제 sweep의 vLLM 측정 파서는
`scripts/parse_bench.sh` → 같은 노드에서 빌드한 native `parse_bench`를 호출한다.
명시 `PARSE_BENCH_BIN` override와 부재 exit2/빌드안내를 제공하고 Python 대체는 하지 않는다.
기존 build 진입점이 두 native selftest를 실행하며 Terraform 설치 절차에도 흡수됐다.

`Measurement`, 숫자 변환/반올림, engine-log 파싱, argv/출력은 책임을 나누었다.
미사용 `--json`을 포함한 기존 CLI, 필드 순서/값과 오류·WARN 우선순위를 유지했다.
숫자 float repr의 동률 잠복결함은 원본 counterexample과 회귀로 수정했다.
고정221 CLI는 실제 셸 런처에서도221/221이며 기존 R1 543+CLI8도 통과했다.
관리 Rust1.99.1에서 fmt·release Cargo88·Clippy, repeats 통합, 전체 배포127/127이 PASS다.
별도 Validator는 새80쌍/출력42회·정본 경계8실행을 수행했다.

공통 stdout의 write/flush 오류를 무시하던 R1/R2 결함을 수정했다. 원본의 작은 출력 flush120과
큰 즉시 write1, WARN 선행·정상 pipe/file을 재검증했다. 원본이 일부 중간 버퍼 구간에서
출력 유실에도0/3으로 끝나는 동작은 의도적으로120+실제 오류로 개선했다.
이 구간을 원본 동등 PASS로 세지 않는다. 초기 stdout 결함·패키지 개인경로 실패 원시도 보존했다.
표준 GNU/UTF8/default buffering 맥락을 비교했으며 PYTHONUNBUFFERED·다른 locale/인터프리터,
처음부터 닫힌 stdout의 help 차이까지 동등하다고 주장하지 않는다.

패키지 CLI 자료624개 중 traceback86개는 비교제외 source frame만 제거했고 모든 비교필드를 보존했다.
외부 원시는 수정하지 않았다. 실행·비교·제외 근거는 상위 과업의 `r2-*`와 독립 R2 보고서에 있다.
동일12입력·11round·교차순서의 실제 셸진입점 포함 CLI 중앙값은 Python21.57ms/Rust4.05ms로
약5.33배였다. 이 로컬 입력의 비용이며 LLM·서빙 성능이나 원격 배포의 증거가 아니다.
