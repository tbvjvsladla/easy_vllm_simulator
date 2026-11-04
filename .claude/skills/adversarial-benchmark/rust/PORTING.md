# benchmark Rust 이행 기록

현재 native 범위는 `sweep_stop`, `parse_bench`, `roofline`, `verdict_rule`의 판정·측정 해석·CLI와 명시된 셸 호출부다. 아래 R1/R2 기록의 수치와 검사는 각 시점의 근거이며 R3 이후의 성능으로 소급하지 않는다.
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

Cargo 검사는 `checks/`에 두며 자동 `tests/` 발견을 끈다. R1 시점의 48검사에 원본465사례,
시각26945/JSON5163/Unicode경계4234 입력과 표적 CLI/스트림/깊이 회귀가 포함된다.
자체검사와 독립 검증·전체 과업의 수용은 별개다. 전체 근거/실패/진행 상태는 같은 과업의
운영 devlog/testlog와 상위 과업의 `harness2-work-policy-261104110714` 문서에 연결한다.

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

## roofline · verdict_rule (R3 기능 통합·로컬 연산 개선 PASS)

범위는 `scripts/roofline.py`(476줄)와 `scripts/verdict_rule.py`(981줄) 전체 — 함수/API와 CLI — 및 `judge_bench.sh`의 `SPEC_ROW`
계산과 `broad_search.sh`의 `rubric_authority` 확인이다. 원본 `.py`는 대조용으로 유지한다. 다른 Python 도구·실제 서빙 측정의 이행 완료가 아니다. 후보 자체검사·독립 표적 검증 이후 정본에 연결했고 아래 정본 통합 검사도 통과했다. 그 시점의 연산 회귀는 아래 최종 교정으로 재검증했다. 초기 I1/I2/Y1 및 후속 I3/I4/I5 실패 원시는 보존하고 같은 입력의 재검증으로 해소했다.

```bash
roofline --model-path PATH [--manifest M --tp N --accept-len L --accept-len-source S ...]   # 원본 옵션·약어·종료 0/2/1
roofline self-test                                  # Rust 전용: selftest_roofline.py 이식 9시나리오(R1~R9) — 합성 헤더 fixture, 측정 아님
roofline aggregate-moe --model-dir D --num-experts N --experts-per-token K    # _active_bytes_moe 를 JSON 으로 (Python 시험의 얇은 진입)
roofline yaml-load PATH                             # manifest 리더 전송로: yaml.safe_load 결과의 repr 또는 Class: message
verdict_rule --measured M --roofline R [--authority ... --reference-tps ...]                # 원본 CLI · --self-test(T1~T23)
verdict_rule evaluate-jsonl                         # Rust 전용: 각 줄 {id, operation, ...} → {id, kind: value|exception, ...}
verdict_rule spec-row --index P --measured P --human-value T --level-rel T   # judge_bench SPEC_ROW (US 구분 4필드)
verdict_rule authority-check --state PATH           # broad_search 의 rubric_authority 확인 (출력=권한 · 오류=exit 2)
roofline native-help · verdict_rule native-help
```

### 수정 내용 (두 번째·세 번째 묶음)

- **R3-I1 (`gpu_model: [}` 무한 대기)**: 부분집합 리더를 버리고 PyYAML 6.0.3 `SafeLoader` 파이프라인(Reader→Scanner→Parser→Composer→resolver→`SafeConstructor`)을
  Rust로 옮겼다(`src/pyyaml/`). 앵커/별칭/병합키/태그/지시자/여러 문서/블록·여러 줄 스칼라/YAML 1.1 해소와 실패 종류·메시지·행·열이 원본과 같다.
  외부 YAML 라이브러리·C 코드는 쓰지 않는다.
- **R3-I2 (`authority-check --state <디렉터리>`)**: 모든 JSON 문서 읽기가 하나의 `json_document` 를 거친다. 디렉터리는 `IsADirectoryError`.
- **I3 (중첩 깊이 · RecursionError)**: 고정 깊이 490을 버렸다. 원본은 PyYAML 이 연 **파이썬 프레임 수**가 한도를 넘을 때 `RecursionError` 를 낸다.
  포트는 원본이 파이썬 함수를 호출하는 자리마다 프레임을 연다(`src/pyyaml/frames.rs`; 스캐너·파서·리더·합성기·생성자와 Mark/Token/Event/Node/예외 생성자,
  `TextIOWrapper` 가 부르는 디코더 `getstate`/`decode` 포함). 한도는 `roofline.py` 의 `<module>`+`main` 2프레임을 뺀 998(`safe_load` = 1)이며
  실제 원본 CLI 의 12개 문서 모양에서 "스택이 999에 닿으면 거부"가 맞음을 이진 탐색으로 실측했다. 메시지는 항상 `RecursionError: maximum recursion depth exceeded`.
  489/490/491층 `ignored: [ ]` 는 원본처럼 읽히고 492층부터 거부된다. 이긴 오류(구문 오류 대 RecursionError)도 같은 호출 순서 덕분에 원본과 같다.
- **I4 (`!!binary "=a=="` 등)**: `decode_base64` 가 CPython 3.12.3 `binascii.a2b_base64` 비엄격 모드의 상태기계(패딩/건너뛰기/1글자 군 오류의 글자 수)를 그대로 따른다.
- **I5 (비ASCII `!!binary`)**: `str.encode('ascii')` 의 `UnicodeEncodeError` 문구 — 첫 연속 비ASCII 구간, 한 글자는 `\xNN`/`\uNNNN`/`\UNNNNNNNN` 이스케이프, 여러 글자는 `position a-b`.
- **별칭 공유 (NODE_BUDGET 삭제)**: 임의의 노드 수 예산을 없앴다. 둘 이상이 가리키는 노드는 한 번만 만들어 `PyValue::Shared(Arc<…>)` 로 공유하며(파이썬 객체 공유와 같다),
  자기 자신을 포함하는 노드는 기존의 `[...]`/`{...}` 표지를 유지한다. 순환 안의 공유 값은 "어느 노드가 열려 있었는가"(표지 낸 노드/펼친 노드)가 같을 때만 재사용한다.
  루트의 628바이트·26 collection probe 가 자식 전용 한도(주소공간 1.5 GiB·CPU 3초·core 0)에서 원본과 stdout/stderr/exit 가 같다.
  `PyValue::Shared` 는 YAML 리더만 만들며 `peel()` 로 투명하게 읽힌다(`verdict::get`·`repr`·JSON·동등성). 새 변종이 R1/R2/JSON 경로에 영향 없음은 아래 회귀로 확인했다.
- **CLI 검사 엄격화 / `f.read(n)` 모델 / `roofline self-test`**: 이전 묶음 그대로(편차 허용 없음; set 순서 프로브 2건만 `set_order_criteria.json`; 1 TiB 상한 없음).

### 검사 결과 (각 명령·rc·출력은 과업의 검사 원시 `v*`/`w*`/`x*`/`y*` 와 `deep-check-run*`)

| 검사 | 결과 |
|---|---|
| `cargo fmt --check` | rc 0 (`y1`) |
| `cargo clippy --all-targets --locked -- -D warnings` | rc 0 (`y2`). 앞선 시도 `v2`(rc 101)·`w17-clippy`(`Ok(…?)` 불필요, rc 101)는 보존 |
| `cargo build --release --locked` | rc 0 (`y3`) |
| `cargo test --locked` debug / `--release` | 각 120 통과 · 0 실패 (`y4`/`y5`) — 새 `yaml_depth` 3개, `pyyaml` 단위 2개 포함 |
| native 자체검사 | `roofline self-test`, `verdict_rule --self-test`, `parse_bench self-test`, `sweep_stop --self-test` 모두 rc 0 (`y6`) |
| 루트 `check_r3_external.py` | API 348/348 · CLI 184/184 · SPEC_ROW 150/150 (`x2-r3-ext`) |
| 루트 `check_r2_external.py` / `check_r1_external.py` | 221/221 · 543 + CLI 8 (`x2-r2-ext`, `x2-r1-ext`) — `PyValue` 변경의 회귀 증거 |
| 추가 대조 (`diff_extra.py`) | API 2299 · CLI 429+30 · SPEC_ROW 368+4 중 불일치는 set 순서 프로브 2건뿐(`x3`) |
| YAML 리더 | 원본 PyYAML 캡처 3208 문서(직접 204 + 파편 1500 + binary/비ASCII 1484 + alias 20)의 값/오류 문구 동일 |
| 호출 구조 | 위 3208 문서의 호출 순서(깊이·함수명)와 최대 프레임 수가 원본 profile 추적과 동일 (`x4`; 시험 `yaml_depth`) |
| 경계 깊이 (`deep_check.py`) | 7개 모양 × 약 45개 바닥 내용 × 한도 직전·직후 = 602 쌍에서 최대 스택·결과·원본 CLI exit/stdout/stderr 동일 (`deep-check-run2`, `run3`) |
| 별칭 공유 CLI (`alias_cli_check.py`) | 사용 필드/미사용 필드에 공유·순환 공유 그래프를 둔 21개 매니페스트 × 2 = 42회 원본 대조 일치 |
| 독립 검증자의 기록 | `commands.json` 51 + `limits-commands.json` 13쌍을 현재 후보로 재생 64건 일치 (`independent_replay.py`) |
| portable | 공백 경로의 깨끗한 사본에서 `--offline --locked` release 빌드·자체검사·`yaml_depth`/`yaml_reader`/`package_paths` 통과 (`y7`/`y8`); `package_paths` 는 개발자 경로·scratch 이름 부재 |

### 남은 한계 (조용히 줄이지 않고 이름으로 기재)

① 자기 자신을 포함하는 YAML 노드는 `[...]`/`{...}` 표지 값으로 나타난다(원본은 순환 객체). 관측되는 범위 — `len`, `int()` 오류와 `repr`, JSON 의 `Circular reference detected` — 는 원본과 같다.
② 한도는 `roofline.py` 가 `main()` 에서 `yaml.safe_load` 를 부르는 CPython 3.12.3 환경(호출 스택 2 + 한도 1000)이며, 다른 호출 경로·다른 한도 설정까지 같다고 주장하지 않는다.
③ 생성자 단계의 프레임은 `flatten_mapping` 재귀(병합 사슬)와 컨테이너 생성기만 모델링했다. 그 밖의 생성자 호출은 얕아 한도에 닿지 않는다.
④ weight_map 값 혼합 타입의 `TypeError` 문구는 원본도 실행마다(해시 시드) 달라 두 관측 중 하나로만 판정한다.
⑤ int/int 나눗셈은 정확 반올림이나 `k/n` 처럼 작은 값에서만 검증했다. ⑥ 기준 환경은 CPython 3.12.3·PyYAML 6.0.3·Linux GNU aarch64 이다.
⑦ 별도 Validator가 최종 19실행/18관측에서 I3/I4/I5·공유 별칭·기존 실패 경로를 확인했다. 전체 정본 통합 판정은 별도다. 6프로파일 E2E·원격 서브 배포는 하지 않았다.

### 정본 연결

`roofline.sh`/`verdict_rule.sh`는 명시된 override 또는 현재 노드의 owner release를 직접 실행한다. 부재는 exit2와 Terraform/build 안내이며 Python 대체는 없다. `judge_bench.sh`의 SPEC_ROW/roofline/verdict와 `broad_search.sh`의 authority 확인을 native owner로 연결했다. 원본 Python은 참조용으로 유지한다.

빌드 자체검사는 상속된 override와 무관하게 방금 빌드한 네 실행파일을 명시한다. 정지 판정은 정상 continue0/stop3만 허용하고, 그 밖의 native 실패는 caller2로 중단한다. 기존1/120 실패를 통과시키던 동작을 바꾼 의도적인 신뢰성 개선이며 해당 두 상태를 원본과 동일하다고 주장하지 않는다. 기존2의 진단과0/3 동작은 보존한다.

### R3 정본 기능 검증과 성능 기준선

관리 Rust offline release와 네 just-built 자체검사, fmt/release120/Clippy를 통과했다. 실제 Bash launcher 경유 외부348 API/184 CLI/150 SPEC_ROW, R2 221 및 R1 543+CLI8이 일치했다. 기존 header assert와 native 계산, judge J1~J12 및 precheck/cell args/repeats 통합이 통과했고 전체 배포128검사도 통과했다. 복사 목록의 첫 항목을 Bash로 실행하던 테스트 init 실패는 broad_search.sh 명시 경로로 고쳐 재검증했다. 실패 원시는 보존했다.

별도 Validator는 정본12명령/17검사에서 두 런처 정상/부재2·Python 대체 없음, judge의 실제 native 체인·양 단계 실패 시 파일 보존, broad authority의 native 확인을 검증했다. 빌드 helper와 stop 가드는 검증된 draft와 byte동일하여 앞선 근거를 재사용했다.

사전 고정100입력의 전체 build_verdict 객체 생성(입출력 JSON 제외), 11교차 round×200반복은 Python 중앙값2.7905μs/Rust32.5627μs로 Rust가 약11.67배 느렸다. 이 연산 개선 목표는 FAIL이며 후보 최적화를 계속한다. 별도로 실제 Bash 진입점10개×11교차 round는 모든 출력이 일치했고 Python26.3787ms/Rust4.2094ms(약6.27배)였다. CLI 기동 비용의 절감으로 연산 지연을 숨기지 않는다. 이 수치는 현재 로컬 결정론 단위의 측정이며 LLM·서빙·원격 배포 결과가 아니다. 원시 기준과 실패 결과는 상위 과업의 `r3-benchmark-*`에 유지한다.

## 성능·키 교정 후보의 정본 검증 진행

아래는 Builder의 자체검사와 후보 측정이다. 정본8경로에 반영해 실제 영향 검사 중이며 새 독립·정본·11round 측정은 아직 수용 전이다. 앞선5acbf9e의 고정 기능 검사 결과는 그 범위에서 보존하되 새 R3-K1 문자열 키 충돌은 FAIL로 기록했고 이 후보에서 타입으로 구분해 수정했다.

## 성능 교정 (R3 · 순수 계산)

정본 통합 뒤 얼린 비교에서 전체 `build_verdict` 값 구성(미리 파싱한 입력 → 전체 순서 있는 값, 직렬화·파싱·프로세스 시작 제외) 비용이 Python 2 790 ns/값,
Rust 32 563 ns/값(약 11.7배 느림)으로 나왔다. 시작 시간 이득(CLI 4.2ms 대 26.4ms)은 이 계산 비용의 대체가 아니므로 같은 경계에서 고쳤다.

**원인(프로파일)**: 표집(gdb 반복 중단, 45~80 표본/단계) 결과 `PyDict::insert` 안의 `same_key` 가 비교마다 두 키를 `String` 으로 변환(`to_utf8`)하는 것이
첫 단계 표본의 약 65%였다. 그다음은 리터럴 키·메시지를 매번 `Vec<u32>` 로 다시 만드는 `PyStr::new`(재할당 포함), 원자적 참조 계수, 딕트·후보 목록의 불필요한 복제,
BigInt 로 하는 고정소수 반올림, 값마다 두 번 도는 `float_repr` 의 서식 할당이었다.

**수정(일반적·유지 가능한 것만, 케이스 식별·결과 캐시·근사 없음)**:
- `same_key`: 표지가 붙은 비문자열 키 둘이 아니면 일반 문자열 동등으로 바로 비교(할당 없음) — 의미 동일.
- `PyStr`: 소유(`Owned`)/정적(`Static`) 이중 표현. 소스에 적힌 리터럴은 사용처마다 한 번만 변환해 프로그램 수명 동안 유지하는 버퍼를 포인터로 복제(`lit!`; 사용처 수만큼, 무한 인터닝 아님),
  쓰기 전에는 복사. `PyStr::new`/`push_str`/`message` 는 용량을 미리 잡는다. ASCII 비교 경로.
- 딕트: 소스에 적힌 서로 다른 키로 만드는 `dict_of` 는 중복 확인 없이 항목을 받는다(디버그 빌드가 주장을 검사). 배열 인자로 `Vec` 할당 제거, `updated`/후보 목록/`spec_axis` 는 복제 대신 소유권 이동.
- 숫자: `round_to_places`/`format_fixed` 는 128비트 정수 정확 경로(미충족 시 기존 BigInt 경로); 반올림 결과는 정확한 피연산자 나눗셈. 불가능한 동점은 증명된 조건(`tie_possible`)으로만 건너뛰고,
  `float_repr` 는 스택 버퍼와 한 번의 출력 할당.
- 위 각각에 기존 구현(BigInt·할당 경로)을 기준으로 삼는 차분 시험을 두었다(`pynumber`·`pyvalue` 단위 시험: 수십만 입력, 동점·부분정규·무작위 비트).

**측정**(`r3-perf-probe`: 얼린 100 triplet × 200, 워밍업 1회, Rust 는 `black_box` 로 전체 결과 소비 — 루트 벤치 소스와 같은 경계·같은 잠금 의존성; 교대 6회):

| | ns/값 |
|---|---|
| Python 전체 `build_verdict` | 2 781 (중앙값; 2 763~2 791) |
| Rust 수정 전 | 32 425 (32 333~32 494) → Python 의 11.7배 |
| Rust 수정 후 | 1 749 (1 719~1 760) → Python 의 0.63배(약 1.59배 빠름) |

중간 단계: 8 110 (`same_key`) → 4 650 (리터럴 공유) → 3 430 (정적 저장) → 2 700 → 1 950 → 1 830 → 1 750.
이것은 로컬 순수 계산 경계의 표본이며 CLI 시작·서빙 성능이나 통계적 우월성을 뜻하지 않는다. 새 코드는 루트 독립/정본 수용 전이다.

## 키 종류 교정 (R3-K1)

**결함**: YAML 의 비문자열 키(`true:`, `1:`, 날짜)를 `\0yaml-key:<정체성>\0<repr>` 모양의 *문자열*로 `PyDict` 에 저장했고, 키를 읽을 때 문자열 내용만 보고 비문자열 키로 간주했다.
그래서 JSON 에서 온 평범한 문자열 키 `"\u0000yaml-key:num:1\u0000True"` 가 `true` 키로 바뀌고(원본은 그대로 보존), 이유 문구의 repr 도 `{True: 5}` 로 변했으며,
정체성 부분이 같은 서로 다른 두 문자열은 삽입 때 하나로 합쳐졌다. 접두사를 다른 것으로 바꾸는 방식은 같은 결함이 남으므로 쓰지 않았다.

**설계**(`src/pyvalue.rs`, `src/pyyaml/compose.rs`): 키의 종류를 내용이 아니라 **타입**으로 가른다.
- `NonStringKey { identity, repr }` 와 `DictKey::{Str, Other}` 를 두었다. `identity` 는 파이썬 키 동등(`1`·`1.0`·`True` 하나, `nan` 은 자기와도 다름), `repr` 는 `repr(key)` 이다.
- `PyDict` 는 항목(`Vec<(PyStr, PyValue)>`, 비문자열 키는 `repr` 텍스트)과 **위치로 색인한 비문자열 키 목록**(`others`, 오름차순)을 가진다. 목록은 비문자열 키가 있는 딕트에만 있어 JSON 객체는 추가 데이터도
  추가 검사도 없다. 문자열 삽입·`get` 은 비문자열 항목을 건너뛰므로 문자열이 비문자열 키와 섞이지 않는다. 비문자열 키는 `insert_key(DictKey::Other)` 로만 들어가며 YAML 구성기(`dict_key`)만 만든다.
- 동등 키는 첫 위치와 첫 표기를 유지하고 값만 마지막 것으로 바꾼다(파이썬 dict 와 같다). `repr`/JSON 출력은 항목의 종류로 갈린다: 비문자열 키는 `repr` 와 JSON 철자(`true`, `null`, `Infinity`…, date/datetime/bytes 는 `TypeError`)를,
  문자열은 평소처럼 `repr`/JSON 이스케이프를 쓴다. 서로게이트·NUL·유니코드 문자열은 그냥 문자열이다.
- 제거: `NON_STRING_KEY` 접두사, `key_repr`, `json_key(&PyStr)`, `is_marked`, `same_key`.

**검사**: 새 `checks/key_encoding.rs` + `checks/data/roofline_verdict/key_encoding_cases.json`(입력 28개 파일, 23 사례, 원본이 낸 exit/stdout/stderr): 기준 사전 관측 10(CLI 6 + PyYAML repr 4), 루트 프로브, 독립 K1 + 음성 대조,
추가 원본 캡처 10(표지처럼 보이는 문자열의 여러 모양, 서로게이트 접두사 — 원본 exit 1 `UnicodeEncodeError`, 중첩, `True`/`true` 문자열 공존). 단위 시험 2개(문자열 키 내용 비교/비문자열 키 정체성, 표지 모양 문자열은 문자열일 뿐).
교정 전 정본 5acbf9e binary 로 같은 13개 관측을 재생하면 10건 불일치, 교정 후 0건이다(`k1-evidence/replay-before-canonical-5acbf9e.txt`, `replay-final.txt`).

**검증**(원시는 과업의 `k*` 파일): fmt `--check` rc 0 · Clippy `-D warnings` rc 0 · release 빌드 rc 0 · `cargo test` release/debug 각 127 통과 0 실패(YAML 리더 3208 문서의 비문자열 키 포함) · 네이티브 자체검사 4종 rc 0 ·
루트 R3 348/348, 184/184, 150/150 · R2 221/221 · R1 543+CLI 8 · 추가 대조 불일치는 set 순서 프로브 2건뿐 · alias 42회·독립 기록 64건 불일치 0.
같은 전체 결과 경계의 순수 계산(얼린 100 triplet, 교대 6회): Python 2 789 ns/값, Rust 1 749 ns/값(0.63배) — 직전 성능 교정 결과(1 749)와 같아 회귀 없음. 이는 루트 측정이 아니며 수용 주장이 아니다.

**한계**: 비문자열 키의 `identity` 는 구성기가 만드는 문자열(`num:…`, `float:…`, `other:…`)이며 *비문자열 키끼리* 비교에만 쓰인다. 문자열 키와는 타입으로 구분되므로 어떤 문자열도 이를 흉내 낼 수 없다.
`PyDict::iter()` 는 비문자열 키를 `repr` 텍스트로 돌려준다 — 현재 이를 순회하는 곳은 JSON 에서 온 딕트뿐이다. 새 코드는 루트 독립/정본 수용 전이다.

## K2/K3 키 범위·정체성 최종 교정

구성기의 유한 정수 float 정체성은 int_from_float의 정확한 BigInt 값이며 i128 포화 변환을 제거했다. NaN은 .nan 클래스 공유 객체와 나머지 scalar node의 새 객체를 구분하고, alias/merge는 원래 node를 재사용한다. 문자열 키와 비문자열 타입 분리는 유지한다. 단일 로드/문서의 현재 소비 경계이며 다른 문서 사이 객체를 합치는 API까지 보증하지 않는다.

새 key_range21사례의 원본 raw stdout을 패키지로 보존했다. 후보 자체 debug/release128·고정대조·자체검사는 통과했다. 별도 Validator는 K2와 NaN 생성4경로의 같은5재현에서 exit/stdout/stderr byte동일 및 키 수1/2/1/2를 확인했다. production source는 compose.rs만 바뀌었고 숫자8함수/기타 source는 동일하여 독립 숫자36·K1·iter 근거를 재사용했다. 정본 빌드·영향 검사는 진행 중이며 최종11round 속도 수용 전이다.

## R3 최종 교정의 정본 판정 — 2026-11-04T13:44:45Z

R3 단위는 기능·정본 연결·필요한 독립 검증·로컬 성능 비교를 완료했다. 초기 기능/성능 FAIL과 중간 자체측정은 시점별 기록으로 보존하며 아래 실제 정본 결과로 현재 판정한다. 전체 프로젝트 이행이나 원격/6프로필/Long E2E 완료가 아니다.

- 최종 managed offline release와 네 just-built 자체검사, fmt/release128/Clippy PASS. K2/K3 수정 뒤 실제 정본 launcher의 같은5재현도 exact PASS. 앞선 고정348/184/150·R2 221·R1 543+8·5통합·새 key10·배포128은 관련 source/연결이 유지되는 범위로 재사용했다. 최종 production delta는 YAML compose의 키 정체성뿐이다.
- 별도 Validator: 숫자36대표·동일프로세스 제어·K1/음성/YAML 키 구분·iter 현재 소비 경로·최종 K2/K3 같은5재현 PASS, 보호 source/bin 변화0. 숫자8함수/나머지 source가 같아 재사용했다.
- 원래 runner byte동일 사본·같은 frozen100슬롯(93고유)·200반복·11교차round의 full build_verdict: Python 2.7854μs/Rust 2.3208μs, Python/Rust 1.2002배. JSON 입력/출력·프로세스 기동은 이 API timer 밖이며 전체 결과 생성/해제는 안이다.
- 같은 실제 Bash CLI10입력×11교차round: Python 23.3001ms/Rust 4.0605ms, 5.7383배. 모든 실제 출력/오류/종료는 frozen 원본과 일치했다.
- Builder 자체 약1.75μs와 root 최종2.32μs를 합치거나 더 좋은 값만 최종으로 고르지 않는다. 최종값은 정본의 별도11round다. 측정환경은 Linux GNU aarch64/CPython3.12.3/PyYAML6.0.3/관리Rust1.99.1이다. 표본 범위의 측정이며 통계적 우월성·LLM/서빙 가속을 주장하지 않는다.

원시: 상위 과업 temp의 r3-key-range-canonical/commands.json·fixed-reproductions/, r3-post-key-benchmark/quality-gate.json·r3-benchmark-results/, 별도 r3-independent-261104의 perf-numeric/와 key-encoding/range-final/. 원래 r3-benchmark-results/의 순수 연산11.67배 지연 FAIL은 그대로 유지한다.
