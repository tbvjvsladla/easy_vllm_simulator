# benchmark Rust 이행 기록

현재 native 범위는 `sweep_stop`, `parse_bench`, `roofline`, `verdict_rule`, `parse_guidellm`, `repeat_axis`의 판정·측정 해석·CLI와 명시된 셸 호출부다. 아래 R1/R2 기록의 수치와 검사는 각 시점의 근거이며 R3 이후의 성능으로 소급하지 않는다.
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

## R4 parse_guidellm (후보 · 정본 적용 전)

상태: Builder 후보. 루트의 독립 검증·정본 연결·성능 수용은 아직이다. 이 절은 앞선 R1/R2/R3 기록을 대체하지 않는다.
범위는 `scripts/parse_guidellm.py` 전체(626줄 — 함수 `build`·`classify_errors`·`inherit_accept_len`·`prefix_cache_run`, CLI, 실제
자체검사 **G1~G29**(요청서의 G1~G25가 아니다))와 `sweep_bench.sh` `_spec_axis_args`의 인라인 Python 본문이다. 원본 `.py`는 대조 기준으로 둔다.

```bash
parse_guidellm --benchmarks-json P (--accept-len-src P | --spec-axis-absent) [--engine-log P] [--benchmark-index N]
               [--post-health-json P] [--prefix-cache-json P] [--max-error-rate R] [--json] [-h]   # 원본 옵션·약어·종료 0/2/1
parse_guidellm --self-test                       # 원본 G1~G29 그대로(전사까지 바이트 동일)
parse_guidellm evaluate-jsonl                    # Rust 전용: {id, operation, arguments} → {id, kind: value|exception, ...}
parse_guidellm spec-source --lite-raw PATH       # Rust 전용: sweep_bench 인라인 본문(성공=warm 경로 한 줄 · 실패=Class: message, exit 1)
parse_guidellm bench-tool                        # Rust 전용: BENCH_TOOL 토큰(guidellm) 하나
parse_guidellm native-help
```

### 구조(책임)

| 모듈 | 책임 |
|---|---|
| `src/guidellm.rs` | 도메인: `build`(전체 순서 있는 측정 객체) · `classify_errors`(`ErrorSplit`) · `inherit_accept_len`(`SpecAxis`) · `carry_prefix_cache`; `Failure`(메시지가 PyStr — argv 서로게이트가 메시지에 남는다) |
| `src/guidellm_cli.rs` | argparse 면 + 입력 읽기 순서(accept-len-src → engine log → post-health → benchmarks → prefix-cache) · 종료 분류(`Termination::Handled` exit 2 / `Uncaught` exit 1+traceback) · `evaluate-jsonl` · `spec-source` |
| `src/guidellm_selftest.rs` | G1~G29. G11/G29는 **실행 시** 실제 owner 표본을 읽는다 |
| `src/help_layouts_parse_guidellm.rs` | 원본 argparse 출력의 폭별 표(COLUMNS 1..480 관측, 319 이상 상수) — 생성 데이터 |
| `src/bin/parse_guidellm.rs`, `src/bin/parse_guidellm_bench.rs` | 실행파일, 전체 결과 생성 시간 측정기(측정 전용) |

재사용: `PyValue/PyStr/PyDict/lit!`(정적 리터럴), `argv`(Spec 표만 추가), `PyException`·`float_from_int/str`·`round_to_two_places`·`format_fixed`,
`pyjson`(strict UTF-8·깊이 9997·`LoadError`), `engine_log`(정규식 동치 매처), `Streams`/stdout 모델. 기존 R1/R2/R3 동작·데이터는 바꾸지 않았다(공유 파일 변경은 `argv.rs`의 PARSE_GUIDELLM 표 추가, `lib.rs` 모듈 등록, `Cargo.toml` bin/test 등록뿐).

### 보존한 원본 특성(고치지 않음)

- 전체 키 순서·`bool/int/float`·NaN/Inf: `error_rate` 등은 `float(failed)/float(total)`, 거대 정수는 `OverflowError`(exit 1), `request_latency_ms_p99 = p99_s*1000.0`.
- 엔진 로그: `OSError`만 WARN(`engine-log: ...`) — 정규식이 `,` 하나에도 맞아 `float('')`이 `ValueError`로 **탈출**한다(exit 1, parse_bench와 다름). `round(max,2)`, 거대 자릿수는 inf.
- 도구경계 분류: 패턴 목록 **그리고** `output_tokens`(또는 `output_metrics.text_tokens`) 둘 다일 때만 면제, `server_alive_at_bench_end=False`면 면제 무효. 판정의 `.lower()`는 `İ`(→`i`+결합점)·켈빈 기호(→`k`)까지 파이썬과 같다. 레코드/정보 모양이 틀리면 원본처럼 `AttributeError`가 탈출한다(`rec.get`이 앞).
- `streams` 모양별 실패: 문자열은 문자를, dict는 `KeyError`, 숫자는 `TypeError: object of type 'int' has no len()`, 빈 값은 `request_concurrency` 중앙값으로 대체.
- `inherit_accept_len`: 수용길이 키도 `completed`도 없으면 거부(lite raw 포인터 문서). `bool`은 수가 아니다. `prefix_cache_run`: `isinstance(r,(int,float))`가 `bool`을 포함해 `True`도 경고(`1.00`), 키는 닫힌 목록 순서로만 싣는다.
- `--json`은 받지만 출력은 항상 JSON. `--benchmark-index -1`은 argparse가 값으로 소비해 `범위 밖` ValueError(exit 2).

### Rust 전용 면과 한계

- (**SUPERSEDED-IN-PART — 아래 "R4 교정(2차)"가 이 항목의 타입 제한을 철회한다**) `evaluate-jsonl`은 값 전체(순서 있는 객체)를 반환한다. 전송 인자 중 `engine_max`는 null/float, `max_error_rate`는 float/int, `spec`은 객체/null, `server_alive_at_bench_end`는 null/bool, `benchmark_index`는 정수만 받는다(그 밖은 `TypeError` 예외 기록 — 의도한 명시 오류). 기본값은 원본 시그니처(`0`/`None`/`None`/`0.0`/`None`)이고 `doc`/`src`/`path`/`run`은 필수(`KeyError`).
- (**한계 항목은 아래 "R4 교정(2차)"에서 해소 — 파일 기술자 포인터는 이제 재현한다**) `spec-source`는 파일 읽기·JSON 오류 클래스를 원본 인라인 본문처럼 `FileNotFoundError`/`JSONDecodeError`/`UnicodeDecodeError`/`ValueError`로 낸다. 바깥 Bash의 `-z`/`! -s` 우회(부재·빈 raw)는 이 도움말 밖이다. **한계**: `bench_warm_json`이 정수/불리언(파일 기술자 번호)이고 그 기술자가 비어 있지 않은 일반 파일을 가리키면 재현하지 않고 `NotImplementedError`(exit 1)로 끝난다(원본은 그 기술자를 읽는다 — 실사용 입력이 아니다). 그 밖의 비문자열 포인터(`list/dict/float`의 `TypeError`, 범위 밖 정수의 `OverflowError`, 존재하지 않는 기술자의 `ValueError`)는 원본과 같다.
- 자체검사 실행 전제: **owner 표본 파일**. 런처가 `PARSE_GUIDELLM_FIXTURE=<SDIR>/../fixtures/guidellm_benchmarks_sample.json`을 내보내고, 직접 실행은 `<skill>/rust/target/release/parse_guidellm` 배치에서 `<skill>/fixtures/`를 유도한다. 둘 다 파일이 아니면 원본처럼 `G11` 실패(`[FAIL] G11 ... 부재`, exit 1, G14 뒤 게이트에서 정지). 컴파일에 표본을 넣지 않아 삭제가 가려지지 않는다. 전역 환경·설정은 바꾸지 않는다.
- 이식 대상 소비자(정본 연결은 루트): `sweep_bench.sh` 메인 파서 호출과 `_spec_axis_args`, `classify_cell.py`의 `BENCH_TOOL`(`parse_guidellm bench-tool`), `hintlib.evidence`의 `build(gdoc).completed`, 상속 직접 시험(`selftest_sweep_meta`), 반복 픽스처 복사. 이 후보는 그 파일들을 수정하지 않았다.

### 후보 검증(이 절의 수치는 후보 자체 측정이며 루트 수용이 아니다)

| 검사 | 결과 |
|---|---|
| 루트 `check_r4_external.py`(릴리스, `PARSE_GUIDELLM_FIXTURE` 지정) | API 145/145 · CLI 140 · engine 13 · malformed 3 · spec-source 18 · depth 48 + boundary 44 전부 일치. 환경 변수 없이 실행한 **최초 시도는 `0128-cli`(`--self-test`)만 불일치(표본 부재 G11)**로 원시 보존 |
| 원본 자체검사 대조 | 전사(stdout) 바이트 동일, stderr 동일, exit 0 |
| 추가 차분(원본 vs 후보 실시간) | `diff_extra` 379건(구조·타입·argparse·유니코드·거대 정수·서로게이트) 0 불일치 · `spec-source` 51건 0 불일치(최초 1건: 범위 밖 정수 기술자 `OverflowError` — 수정 후 0) |
| 패키지 `guidellm_cases` | 8검사: API 145 전체 순서 비교 · frozen 174 · depth 92 · 추가 379 · 도움말 132(COLUMNS 22값) · 자체검사 전사 · 표본 부재 실패 · 네이티브 단어 |
| `cargo fmt --check` · `clippy --all-targets -D warnings` · release/debug `cargo test` | 통과(전체 136검사, 0 실패). clippy 최초 시도 실패 2건(`unnecessary_map_or`)은 원시 보존 후 수정 |
| 공유 파일 변경 회귀 | 루트 R3 348/184/150 · R2 221 · R1 543 전부 일치 |
| 성능(후보 표본, 루트 수용 아님) | 사전 고정 34 value 케이스, 전체 결과 생성+해제(JSON·기동 제외), 교대 11라운드×200: Python 중앙값 4540 ns/값, Rust 1283 ns/값(약 3.5배). 실제 CLI 진입점은 따로 측정하지 않았다 |

### 알려진 한계·잔여

① 원본 stack frame은 재현하지 않는다(클래스/메시지와 traceback 앞 텍스트만). ② 기준 환경은 CPython 3.12.3·Linux GNU·UTF-8 스트림이다. ③ `evaluate-jsonl`의 비지원 전송 타입은 명시 오류다. ④ 도움말 표는 COLUMNS 319 이상 상수이며 700/1000/5000을 재확인했다.
⑤ 정본 연결(런처 `parse_guidellm.sh`, `sweep_bench.sh`/`classify_cell.py`/`hintlib.evidence` 호출부, `build_rust.sh` 자체검사, 배포 검사·스킬 문서)과 독립 검증·실제 CLI 성능 비교는 루트 작업이다. 6프로파일·실제 E2E·나머지 Python 이행은 열려 있다.

## R4 교정(2차) — 원시 JSON 인자 · 파일 기술자 포인터 · JSON 전송 (후보 · 정본 적용 전)

독립 검증의 2건(I1 `server_alive_at_bench_end=0`을 새 타입 검사로 거부, I2 `benchmark_index=1.0`의 실패 시점·문구)과 새 원본 관측 12+6건이 같은 결함의 증상이었다 —
전송/라이브러리가 원본 Python API의 **원시 JSON 인자**를 `Option<bool>`/`BigInt`/`f64`/`SpecAxis`로 좁혔다. 초기 후보의 해당 서술은 위 절에 보존하고 여기서 철회한다.

### 1. API: 인자를 좁히지 않는다(`src/guidellm.rs`)

`BuildRequest`의 다섯 필드(`benchmark_index`·`engine_max`·`spec`·`max_error_rate`·`server_alive_at_bench_end`)는 모두 호출자가 준 `PyValue`다. 검사는 원본의 **연산 지점**에서만 한다.

| 인자 | 원본 지점 | 결과 |
|---|---|---|
| `doc` / `benchmarks` | 맨 앞 `ValueError` | 다른 인자 오류보다 먼저(`doc-before-args`, `runs-before-spec`) |
| `benchmark_index` | `0 <= i < len(runs)` → `runs[i]` | 순서 안 맞는 형은 `TypeError`(`int`와 비교), 범위 밖은 `%d` 렌더링(float 절단·`NaN`/`inf` 오류) 뒤 `ValueError`, 범위 안 float는 `TypeError: list indices must be integers or slices, not float`, bool은 0/1 |
| `engine_max` | `if engine_max and decode_tps` → `/` | 원시 그대로 출력(int/bool/str 포함); 참이고 디코드가 있을 때만 나눗셈 — 문자열 등은 `TypeError: unsupported operand type(s) for /`, int 초과는 `OverflowError` |
| `spec` | `spec or {}` → `.get` | 거짓값(`False`/`[]`/`""`/`0`)은 미지정, 참인 비매핑은 분류·분해 뒤에 `AttributeError` |
| `max_error_rate` | `measurement_ok` 단락 평가 | 원시 그대로 출력, 비교는 `successful`·`decode_tps` 뒤 — `None`은 그 지점에서만 `TypeError` |
| `server_alive_at_bench_end` | `is not False` / `is False` / `is True` | **`Liveness::{Dead, Alive, Unobserved}`** — `0`·`""`·`null`·그 밖은 모두 관측 없음; 원시값(`0`)을 그대로 저장 |
| `classify_errors`의 `benchmark_index` | `run["benchmarks"][i]` `except (KeyError, IndexError, TypeError)` | 리스트는 int/bool 위치, 매핑은 문자열 키, 그 밖은 `None` |
| `inherit_accept_len`의 `path` | `%s` | 어떤 JSON 값이든 `str()` 모양으로 메시지·출처에 |

CLI는 같은 필드에 타입 값(`int`/`float`/`bool`/`None`)을 넣을 뿐이며 argparse·post-health의 원래 타입 검사는 그대로다. `ErrorRateLimit`·`Option<bool>` 같은 좁은 타입은 제거했다.
차분: 원본 함수를 프로세스 안에서 호출한 8388건(17개 문서 × 인자 하나씩 바꾼 형 변주 + 문서당 무작위 120 + classify/inherit 형 조합)을 두 시드(1, 2)로 돌려 0 불일치.

### 2. 파일 기술자 포인터(`spec-source`)

`bench_warm_json`이 정수/불리언이면 `os.stat(fd)`·`open(fd)`처럼 **그 번호의 열린 기술자**를 쓴다. 근거 5건(`regular-zero/offset`, `malformed-json`, `empty-file`, `write-only`)의 exit/stdout/stderr와 공유 offset 부작용을 그대로 재현한다.
- 상태(`isfile`·`getsize`)는 프로세스 기술자 표(`/proc/self/fd/N` = 그 기술자의 `fstat`)로 읽는다 — **Linux `/proc` 전제**(원본 실행 환경과 같다). 닫힌/음수 번호는 `ValueError: bench_warm_json 부재/빈 파일: N`, C `int` 밖은 `OverflowError`, 실수·리스트·매핑은 `TypeError`(`os.stat`의 문구) — 모두 원본과 같다.
- 읽기는 `/proc/self/fd/N`을 **다시 열지 않는다**(offset 0이 되기 때문). 한 곳의 `unsafe`(`read_descriptor`)로 번호의 소유를 `File`로 옮겨 **현재 offset부터 끝까지** 읽고 기술자를 닫는다(Python의 `open(fd)`가 소유를 넘겨받아 닫는 것과 같다). 안전 계약: 직전에 열린 기술자임을 확인했고, 이 프로세스에서 다른 Rust 핸들이 소유하지 않으며(표준 스트림은 표준 라이브러리가 닫지 않는다), 한 번만 소비한다. 쓰기 전용 기술자는 `OSError: [Errno 9] Bad file descriptor`(파일명 없음).
- 검증: `fd_driver`(원본 인라인 본문 vs 후보, 새 기술자를 `dup2`+`pass_fds`로 주고 시작 offset을 맞춘 뒤 **종료 후 offset 비교**) 27건 0 불일치(근거 5건은 기대값과도 대조 + 추가 22: 중간 offset·EOF·RDWR·UTF-8 오류·BOM·CRLF·범위 밖·stdin/stderr 등). 패키지 시험은 셸이 기술자 3을 열어 같은 순서를 재현한다.
- 남은 한정: 기술자가 일반 파일이 아니거나(파이프 등) 비어 있으면 원본처럼 `부재/빈 파일` 오류, **더 이상 `NotImplementedError`는 없다**.

### 3. 일반 JSON 전송(`evaluate-jsonl` opt-in)

요청 `{id, operation, arguments_json: {이름: 원시 JSON 텍스트}, result_json: true}` (`arguments`와 상호 배타). 각 인자를 **따로** 같은 깊이 문맥(`loads_text`, 9997)으로 읽어 요청이 문서를 한 단계 더 중첩하지 않는다 — 같은 전체 라이브러리 함수를 부른다. 응답은 이 모드에서만 `{id, encoding: "json-values-v1", kind: "value", value_json}`(전체 결과의 compact ASCII JSON 텍스트) 또는 `{id, encoding, kind: "exception", type, message}`다. `arguments` 표준 레코드(163 근거)는 바이트·스키마 동일(encoding 필드 없음). 잘못된 원시 JSON은 `JSONDecodeError`, 너무 깊으면 `RecursionError`, 둘 다 예외 레코드다. 비즈니스 규칙·캐시·결과 투영은 없다.
- 실제 root 초안 어댑터(`parse_guidellm_native.py` + `parse_guidellm.sh`, 수정 없이 복사)를 **새 owner 배치 임시 샌드박스**(`scripts/`·`rust/target/release/`·`fixtures/`, 명시 `PARSE_GUIDELLM_BIN`)에서 후보로 검증: 32검사 0 실패 — 9996 중첩 문서 `build.completed == 16`·전체 결과 원본과 동일(표준 봉투의 `json.dumps`는 RecursionError임을 같은 실행에서 확인), NaN/Inf/서로게이트/거대 정수, `ValueError`/`TypeError`/`KeyError`/`OverflowError`/`AttributeError` 사상, 무작위 300건 원시형 대조, `bench-tool`, **전송 인코딩이 없는 낡은 바이너리는 `RuntimeError`(재빌드 안내)·부재 바이너리는 `ImportError`**.
- 한계: 현재 Python 소비자가 쓰는 JSON 값만 전달한다. 임의 Python 객체(바이트·집합 등) 직렬화는 주장하지 않는다.

### 4. 검증 요약(후보 자체 측정 — 루트 수용 아님)

| 검사 | 결과 |
|---|---|
| 루트 `check_r4_external.py`(릴리스, `PARSE_GUIDELLM_FIXTURE` 지정) | API 163/163(원본 145 + 형 12 + 경로·인덱스 6)·CLI 140·engine 13·malformed 3·spec-source 18·depth 48+44 모두 일치 |
| 독립 2건(`api-liveness-zero-is-not-False`, `api-float-index-typeerror`) | 기대 레코드와 문자열까지 동일 |
| 패키지 `guidellm_cases` | 14검사(기존 8 + 원시 형 API 12+6+1200 · 독립 2건 · 기술자 5 · 숫자 포인터 7 · 전송 깊이 · 전송 특수값/오류) |
| fmt · clippy `--all-targets -D warnings` (bench bin 포함) · release/debug 전체 `cargo test` | 통과, 각 142검사 0 실패(최초 clippy 실패 `useless_format` 2건 원시 보존 후 수정) |
| R1 543 · R2 221 · R3 348/184/150 | 전부 일치 |
| 성능(후보 표본, 루트 수용 아님) | 동일 34 value 케이스·전체 결과 생성+해제: Python 4567 ns/값, Rust 1191 ns/값(약 3.8배; 교정 전 1283) |

### 잔여

원시 인자 의미의 정본은 원본 Python이다 — 위 차분이 닿지 않은 형 조합이 남았을 수 있다. 정본 연결(런처·`sweep_bench`·`classify_cell`·`hintlib.evidence` 어댑터 설치·`build_rust.sh`·배포 검사)과 독립 재검증·실제 CLI 성능은 루트 작업이다.

## R4 정본 연결 검증 진행

위는 Builder의 후보 자체검사 기록이다. root 고정163 API/266 CLI·helper·depth, FD5의 출력/종료/공유 offset, 별도 Validator의 원본 I1/I2·FD·실제 bridge6검사는 PASS다. 이제 source와13경로연결안을검토해정본에반영했고 managed빌드/통합/배포/실제성능은아직검증중이다. 원본 Python은참조용으로남기며실제호출은native로연결한다.

## R4 정본 적용과 검증 완료

전체 GuideLLM build/inherit_accept_len/classify_errors·CLI·G1~G29 자체검사를 Rust로 연결했다. sweep_bench의 실제 파서/승계 경로, classify_cell 토큰, hint qualification의 owner build가 native를 호출한다. 얇은 Python bridge는 JSON/subprocess 전달과 원래 예외 복구만 맡고 규칙·기본값·원본 fallback을 갖지 않는다. 기존 parse_guidellm.py는 원본 대조용으로 보존했다.

최초 독립 I1(생존값 0은 False 객체와 다름), I2(1.0 인덱스의 실제 list 오류 순서)를 수정했고 같은 원본 입력으로 재검증했다. 원시 JSON 인자를 함수의 실제 사용 지점까지 유지한다. spec-source의 상속 FD는 현재 공유 offset에서 읽고 닫는다. root FD5검사는 출력·종료와 부모에서 관측한 offset까지 일치했다. opt-in arguments_json/result_json 전송은 원본이 처리하는 깊은 doc에 envelope가 먼저 실패하던 실제 결함을 교정한다. 원래 API 레코드 형식은 유지하며 임의 Python 객체 전체 지원을 주장하지 않는다.

root 실제 관리 빌드와 다섯 just-built 자체검사, fmt·release Cargo142·all-target Clippy, 고정163 API/266 CLI·helper·depth, FD5, 이전 R1/R2/R3 및 sweep_meta/repeats/precheck/cell_args와 classify 자체검사, 전체 배포128이 PASS다. 독립 검증은 초기 6개 교정 재현과 최종 7개 실제 적용 경계(힌트 native 판정·ValueError 차단 포함)를 통과했다. 권한 보호를 우회하지 않고 root가 정본에 따로 적용했다.

배포 준비 중 원본 입력16개의 .log 확장자가 Git 제외 규칙에 걸리는 실패를 확인했다. payload를 유지한 .txt와 metadata의 가역 경로 치환으로 정리하고 무참조 생성 transcript13개는 제외했다. 의도적인 invalid UTF8 음성 입력3개는 bytes 그대로 보존했다. Git index의 소스만 새 root에 복사한325파일(그중 Rust323)에서 외부 source-root env 없이 offline locked build·관련16검사·실제 fixture 자체검사를 통과했다. 독립 검토도 index/worktree/freshcopy와 원본 payload를 대조했다. force-add나 ignore 정책 변경은 없다.

사전 고정34 value(23 정상/11 measurement_ok=false)의 전체 결과 생성·해제를 11교차round×200회 비교했다. Python 4439.240441ns/Rust1229.8ns, 약3.6097배였다. 실제 Bash CLI12×11교차round는22.57226217ms/4.063543ms, 약5.5548배이며 stdout/stderr/종료가 모두 고정 원본과 같았다. 로컬 표본이며 LLM·서빙·통계적 우월성이나 다른 입력 전체의 속도를 의미하지 않는다.

원시 근거는 상위 task_id 문서의 R4 수행·검증 항목에서 조회한다. native 반환, 정본 검사, 배포·새 복사 검사, 성능 계약과 결과 및 독립 prototype/followup/canonical 보고서를 연결했고 초기 실패도 보존했다. 로컬 작업 폴더의 구체 경로는 해당 과업 문서가 소유한다.

native 두 묶음은 같은 session 53e74e77-0636-425d-8e65-fcee80c91764, 관측126턴·reported duration1997.185초(실제 wrapper wall2003.930초), permission_denials0이었다. 실제 모델 claude-sonnet-5-5를 관측했으며 요청 high를 별도 실제 effort 관측으로 채우지 않는다. 이는 구현 작업 귀속이며 6프로필 업무 평가가 아니다. 전체 S1~S7·하네스·6프로필·실제 Long E2E는 계속 진행한다.

갱신: 2026-11-05T00:23:14+09:00


## R5 반복 축(`repeat_axis`) 이행 — 후보 기록

상태: 후보(Builder) 자체검사. 정본 적용·정본 연결·별도 Validator·성능 수용은 이 기록의 범위가 아니다.
원본 `scripts/repeat_axis.py`(R4 시점 정본과 같은 내용)의 공개 API·상수·CLI 세 명령·자체검사 A1/B/P/R/G/H 전체를 이식했다. 원본은 대조 기준으로 보존한다.

### 구조와 소유

| Rust | 책임 | 원본 대응 |
|---|---|---|
| `repeat_axis` | 순수 API: 상수, `parse_repeats`, `band_pct`, `summarize`(→ `Summary`/`Stop`), `repetition_established`(→ `RepetitionVerdict`) | 같은 이름의 함수·상수 |
| `repeat_owner` | `load_campaign_budgets`, `resolve_repeats`(argv > 캠페인 선언 > 프리셋 기본값), `CampaignOwner` 질의 경계, `RepoRoot` | 같은 이름의 함수 |
| `repeat_fs` | `record_run`, `aggregate_level`(제자리 쓰기·멱등) | 같은 이름의 함수 |
| `repeat_cli` | argparse 최상위+하위 파서·`resolve`/`record-run`/`aggregate-level`/`--self-test` | `main` |
| `repeat_selftest` | 실제 A1·B·P·R·G·H 실행(원본 출력 재생이 아님) | `_self_test` |
| `repeat_api_cli` / 실행파일 `repeat_axis_api` | `evaluate-jsonl`(모든 공개 연산·`arguments_json`/`result_json` 전송), `constants`, `native-help` | 없음(Rust 전용) |
| `pyfs` · `pysort` | pathlib/`open()` · `list.sort` 비교 일정 전체(run 탐색·minrun·이진 삽입·powersort 병합·merge_lo/hi·galloping)와 `==`/`<` 의미 | CPython 3.12.3 `listobject.c` |
| 실행파일 `repeat_axis`, `repeat_axis_bench` | 원본 CLI와 같은 표면 / 순수 API 시간 측정 | |

Rust 전용 명령은 원본이 쓰는 위치(하위 명령 단어)와 충돌하므로 `repeat_axis` 가 아니라 별도 실행파일 `repeat_axis_api` 에 둔다. `repeat_axis` 는 원본이 거부하는 단어를 그대로 거부한다.
반복 하한 `FULL_REPEATS_MIN` 은 build.rs가 실제 캠페인 스키마에서 읽은 값이며 숫자는 코드에 없다. 자체검사 A1은 실행 시점의 스키마와 이 상수를 비교하고, 스키마가 없으면 FAIL이다.
반복 로직(해소 우선순위·밴드·집계·요약·반복 조건·원시 `campaign_id` 의 참/선택/순서)은 전부 Rust가 소유한다.

### 미이행 소유자(S4까지) — 전체 런타임이 Python-free 가 아니다

활성 캠페인·경로 파생·예약 ID·`BOOTSTRAP` 의 소유자는 아직 이행하지 않은 `campaign_init.py` 다. Rust는 그 규칙을 복제하지 않고 `CampaignOwner` 로만 묻는다. 유일한 구현 `PythonCampaignOwner` 는 환경 변수 `REPEAT_AXIS_CAMPAIGN_ADAPTER` 의 좁은 질의 어댑터(Python)를 띄워 실제 `campaign_init.py` 를 원본처럼 적재한 뒤 `active_campaign_id()`, `BOOTSTRAP`, `RESERVED_IDS`, `derive_path("root", camp_id=…)` 의 결과나 예외만 중계한다. 어댑터에는 반복 로직이 없다. 소유자 파일이 있는데 어댑터가 없거나 시작되지 않으면 `CampaignOwnerAdapterError` 로 이름을 남기고 멈춘다(`repeat_axis.py` 나 다른 Python 반복 계산으로 대체하지 않는다). 소유자 파일이 없으면 원본처럼 어댑터 없이 "해소자 부재" 대체 경로로 간다. 저장소 루트는 `REPEAT_AXIS_REPO_ROOT`(또는 API의 `repo_root`)로 주며 필요한 연산에서만 요구한다.
어댑터 시제품은 후보 밖(검토용)에 있다. 정본 통합은 설치된 소유자 위치에서 런처가 어댑터 경로와 저장소 루트를 정하게 하고, 소유자가 이행되면 어댑터를 제거한다.

### 보존 경계

* 정수/bool/float/원시 문자열 인자 구분, 유니코드 숫자, 큰 정수 한도(4300자리), NaN·Inf, CPython 3.12 `sum`의 보정 합산, `round(x, 4)`.
* 필드 순서(요약·집계 문서·`runs[]`), 재집계 시 소유 키를 문서 끝으로 옮기는 멱등성, 파일 바이트·`json.dumps(indent=2)`·끝 개행.
* 변경 순서와 부분 실패 상태: 디렉터리 생성 → 파일 열기(생성/절단) → 인코딩 → 쓰기. 인코딩 불가 문서는 빈 파일을 남기고, 집계 실패는 아무것도 쓰지 않는다.
* 첫 오류의 종류와 메시지(예외 클래스 이름·`str(e)`), 하위 파서 오류·`-h`·미인식 인자가 최상위 파서에서 나는 순서, usage/help의 COLUMNS 폭 의존, 종료 코드 0/2/1.
* 잡히지 않은 예외: 종료 1과 마지막 `Class: message` 줄만 재현한다(파이썬 스택 프레임은 아니다).

### 알려진 한계

* 정렬은 길이 제한이 없다. 이전 후보의 63개 이하 정확·64개 이상 안정 병합 정렬 한계는 제거했다(아래 "R5 보정 1").
* 소유자 속성은 JSON으로 중계 가능한 값만 다룬다. `SyntaxError` 계열의 잡히지 않은 마지막 줄은 `str(e)` 의 ` (파일, line N)` 꼬리를 떼어 재현한다.
* `aggregate_level` 의 `run_NN` 중복(`run_02`·`run_002`)은 디렉터리 나열 순서를 그대로 따른다(원본과 같은 OS 나열 순서).
* 기준 환경은 Linux GNU·CPython3.12.3이다. 다른 인터프리터 한도나 locale은 보증하지 않는다.

### 자체검사 결과(후보)

| 검사 | 결과 |
|---|---|
| fmt `--check` · clippy `--all-targets -D warnings`(bench 포함) | 통과(최초 clippy 실패 `unused_must_use` 1건 원시 보존 후 수정) |
| 패키지 `repeat_axis_cases`(이전 단계 기록) | 11검사 통과: 고정 API103(전체 순서 있는 결과 비교) · 파일시스템 CLI42(전후 트리 바이트·stdout·종료·stderr) · 소유 캠페인 질의82(원본 트리 불변) · argparse 경계454 · 소유자 변종60 · 상수 · 자체검사 원본 출력 재현+스키마 부재/소유자 원본 부재 실패 · 어댑터 부재/시작 실패/루트 미설정 |
| 원본과 무작위 차분 | API 26000(요약·반복조건·밴드·값역; 예외 종류 5종 포함) 불일치 0 · CLI 800 불일치 0 · 파일시스템 750 불일치 0 · 특이 입출력 21(무효 UTF-8 인자·읽기 전용·심볼릭 링크·권한) 불일치 0 |
| 기존 R1~R4 검사 | 공통 소스 변경은 `argv.rs`(repeat_axis 파서 표 추가), `lib.rs`(모듈 등록), `Cargo.toml`(실행파일·검사 등록)뿐이며 전체 `cargo test` 로 재검증 |

이전 후보 단계에서 `package_paths` 와 `stdout_boundary`(pty 1건)가 각각 R4 문서 문구·PTY FD 상속 때문에 실패했고, 두 원인은 위 "R5 보정 2"와 R4 문단 교정으로 처리했다(검사 기준은 바꾸지 않았다).

### 성능 측정 경계

`repeat_axis_bench` 는 케이스 파일의 인자를 시간 밖에서 연산별로 바인딩하고, 시간 안에서 전체 타입 결과와 순서 있는 값을 만들어 `black_box` 로 소비한 뒤 해제한다. 측정 전에 모든 케이스를 `expected` 와 대조하고 어긋나면 측정을 거부한다. 후보 시점 실행은 동작 확인용 한 번이며 속도 향상을 주장하지 않는다(정본 비교는 루트가 별도로 수행).

### R5 보정 1 — 모든 길이의 `list.sort` 비교 일정 이식

root가 원본에서 고정한 `summarize` 8건(길이 63/64/65/80/100/128, NaN 레벨)에서 이전 후보는 6건 불일치였다. 64개 이상에서 CPython의
galloping 병합 대신 안정 병합 정렬을 쓴 것이 원인이었고, 길이 상한·NaN 특례·기대 변경 없이 CPython 3.12.3의 정렬 알고리즘 전체를
이식했다: `count_run`, `merge_compute_minrun`, `binarysort`, powersort 병합 정책(`powerloop`·`found_new_run`·`merge_force_collapse`),
`merge_at`, `merge_lo`/`merge_hi`(`MIN_GALLOP`·적응형 `min_gallop`), `gallop_left`/`gallop_right`. 비교는 같은 쌍을 같은 순서로 하므로
NaN 같은 비전순 키의 배치와 비교 불가 키의 첫 `TypeError` 도 같다. 키 비교 자체(`==`/`<`)는 기존 `PyValue` 비교기를 그대로 쓴다.
사용자 정의 객체 실행·`reverse`·keyfunc 는 이식 대상이 아니다(JSON 값 영역에서 쓰이지 않는다).
증거: 원본 CPython이 `__lt__` 로 기록한 비교열 439건(길이 0~300, 63/64·127~129·255~257 경계, run/gallop/병합 전이, 내림·중복·톱니·블록·
오르간 파이프·동일 길이 run, NaN 위치별, 혼합 숫자·문자열·리스트 키, 위치별 첫 오류)을 Rust가 쌍 단위로 재현한다
(`sort_schedule`). 변이 대조(MIN_GALLOP, 적응 감소 제거, minrun 경계, run 동률, merge_lo/hi 선택, gallop 경계)는 모두 이 검사가 실패시키며
동치 변이 1건(powersort `>` 대 `>=`: CPython 자체 단언이 동률 불가를 보장)만 통과한다. 동일 길이 run 사례는 `merge_force_collapse` 동률 변이가
처음 살아남은 뒤 추가했다. 고정 8건 전체 순서 결과 비교와 길이 2~260 `summarize` 14000건(값 2421 포함, NaN·중복·혼합 키) 차분은 불일치 0이다.

### R5 보정 2 — 검사 PTY 보조의 master FD 상속

공유 검사 보조 `checks/common.rs` 의 `open_pty` 가 master 를 CLOEXEC 없이 만들어, 병렬 검사 thread 가 그동안 exec 한 무관한 자식이 master 를
물고 있으면 소유자가 master 를 닫아도 터미널이 살아 있어 `pb-pty-gone-8191` 이 EIO traceback(1) 대신 정상 0 을 냈다(독립 조사로 같은 결과의
메커니즘을 입증; 과거 사건의 특정 자식 PID 는 입증되지 않음). master 생성에 `OpenptFlags::CLOEXEC` 만 추가했고 sink·오류 기대와 런타임 stdout
정책은 그대로다. 새 검사 `pty_inheritance` 는 실제 exec 한 자식의 `/proc/<pid>/fd` 를 읽어 (1) CLOEXEC 없는 master 는 상속된다(양성 대조),
보조의 master 는 상속되지 않는다, (2) 살아 있는 무관한 자식이 있어도 master 를 버린 slave 에 실제 `parse_bench` 의 8191 본문이 EIO 로 실패한다를
확인한다. CLOEXEC 를 임시 제거하면 (1)에서 실패함을 확인한 뒤 복원했다. 플래그 존재만 단언하지 않는다.

### R5 보정 — R4 PORTING 문구

정본에서 서술만 교정된 R4 원시 근거 문단(로컬 경로 제거)을 그대로 가져왔고 R4·R5 이력은 유지한다. `package_paths` 기준은 바꾸지 않았다.

## R5 정본 계산·호출 검증과 S4 잔여

반복 선언 해소·경계 기록·레벨 집계·밴드·요약·판정점 반복 조건과 실제 자체검사를 Rust로 연결했다. 기존 repeat_axis.py는 원본 기준으로 보존한다. legacy CLI는 repeat_axis.sh, 별도 JSON/상수 질의는 repeat_axis_api.sh다. broad_search·sweep_bench·classify_cell·bench_preset이 정본 native를 사용하며 격리 사본도 두 실행파일/launchers/소유 질의 전달부를 포함한다.

최초 후보는63개 이하에서만 원본과 같은 정렬을 사용했다. root가 구현 후 추가한 실제 원본8관측 중6불일치를 재현했고, 공식 CPython3.12.3의 run/minrun·powersort·merge/gallop 비교 순서를 이식해 같은8건이 모두 일치했다. 소유자 변종 초기3건과 argparse 단독--도 교정했다. source 영역의 길이 상한·NaN 입력 특례·기준 완화는 없다. 공유 PTY test helper는 masterFD가 실제 exec child로 상속돼0/빈stderr를 만들 수 있음을 독립적으로 재현한 뒤 CLOEXEC를 추가했다. 새 회귀는 실제 자식 FD와 writer EIO/1을 확인한다. 과거 실패의 특정 holderPID까지 증명하지 않았다.

root는31Rust경로·18호출/설치경로를 검토·반영했다. 설치 레이아웃이 source/adapter를 제공하는 Cargo test 기본 바인딩만 root가 추가했고 실제 파일 부재를 여전히 실패로 처리한다. 관리 offline build와 여섯 just-built 자체검사, fmt/release Cargo163/Clippy, 실제 launchers103API·42filesystem·82campaign, 기존 프리셋 해소 전체67, sweep_meta/repeats/precheck/cellargs·bench_preset/lite_report·classify/topology 및 전체 배포128이 PASS다. 이전 R1~R4 runtime source와 argv의 기존 줄이 같아 해당 외부 차분 근거는 재사용했다.

독립 검증은 원본6개, 수정 후 정렬5관측과 실제 후보 helper, 정본 연결7관측/12명령을 각기 수행했다. 부분 쓰기/링크/Unicode순서와 실제 bridge/프리셋/분류기, 양성·원래 오류·실행파일 부재를 확인했다. JSON bridge는 원본 값과 보고된 예외명/문자열/catch계층을 전달하며 원본 Python 예외 객체의 전체 생성 인자/속성까지 보존한다고 하지 않는다. 복잡한 builtin 오류의 module 표기는 builtins로 유지해 실제 소비자의 오류 이름을 보존한다.

Git index363파일(Rust348과 실제 소유 의존15)만 새 공백 경로로 복사해 기존 node-local managed toolchain/cache를 명시 재사용했다. 외부 task/source-root/binary override를 제거한 offline source build·여섯 실제 자체검사·관련 Cargo16이 PASS다. target/cache/인증 파일은 복사하지 않았다. 최초 새 복사 뒤 bridge의 오류 module metadata 한 줄이 바뀌어 현재 index와 차이1개가 생겼다. 이를 동기화하고 새 경로에서 actual bridge 양성/invalidUTF8 ACTIVE의 예외명·module·문자열·catch계층이 원본과 같은지 좁게 확인했다. Rust source·기존 build 결과는 그대로 재사용했다.

사전 고정72개의 모든 value 결과 생성·해제를11교차round×200으로 측정했다. Python1136.558889ns/Rust739.555ns, 약1.5368배였다. 실제 Bash CLI5입력×11교차round의 round평균 중앙값은29.4498744ms/11.409153ms, 약2.58125배며 stdout/stderr/종료/최종트리가 모두 같다. 5입력은 record1·aggregate1·argparse거부1·resolve2이고 I/O가 수행되는 곳의 I/O를 포함했다. 로컬 표본이며 서빙/LLM시간이나 통계적 우월성으로 확대하지 않는다.

개별 결과를 숨기지 않는다. 아직 Python 캠페인 소유자를 질의하는 기본 해소041은22.810927ms→36.625875ms로 약1.61배 느렸다. 나머지4경로는 각3.95/7.03/5.13/4.66배였다. 순수 연산·대표 CLI 집계의 개선과 모든 경로의 개선은 구분한다. S4가 소유 캠페인을 Rust로 전환한 뒤 같은041/전체5입력으로 이 잔여를 다시 측정해야 하며 전체 Rust 기동·성능 완료는 아직 아니다. 캠페인 활성/경로/예약 규칙을 반복 모듈에 복제해 속도를 메우지 않는다.

실제 두 native 묶음은 같은 session16c9b5c5-d1a8-4c11-a464-c2c61ebfa7cd,84+32=116턴·reported2687.835초(wrapper2693.017초), denials0이었다. actualmodel claude-sonnet-5-5와 high요청을 구분했다. cumulative cost를 합산하지 않으며 이 작성 작업을6프로필 평가로 세지 않는다. 최초 원시·교정·초회 root checker 상대경로 설정오류와 실제 차이를 구분해 보존했다.

근거는 상위 task_id 수행·검증 문서의 R5 절에 연결한다. 정본 검사·배포·새 복사·성능 원시와 독립 baseline/prototype/fix1fix2/canonical, CPython 공식 source/license 및 초기 실패를 같은 과업에 보존했다. 이어서 R6 프리셋, 나머지 S1~S7·하네스·6프로필·실제 Long E2E를 계속한다.

갱신: 2026-11-05T01:59:11+09:00

## R6 프리셋 해소(`bench_preset`) 이행 — 후보 기록

상태: 후보(정본 적용 전). 범위는 원본 `bench_preset.py` 전체(상수·닫힌 스키마·바인딩/프리셋 로더·검증·서빙 상한·값 해소·데이터셋 실행 검사·CLI·자체검사)다.
`dataset_prep`·`campaign_init` 전체 이행과 launcher/소비자(`sweep_bench`·`broad_search`·`run_bench`) 연결은 이 단위의 범위 밖이며 연결되지 않은 경계로 남는다.

### 원본→Rust 대응

| 원본 | Rust | 비고 |
|---|---|---|
| `validate_preset` · `_dataset_shape_problem` · `dataset_cross_problems` | `bench_preset::{validate_preset, dataset_shape_problem, dataset_cross_problems}` | 문서는 `PyValue`로 받고 Python의 `==`(bool/int/float 동치)·진리값·`in`·`repr`로 판정한다. 해시 불가 값은 같은 `TypeError`다 |
| `load_bindings` · `preset_path` · `load_preset` · `_load_yaml` | 동명 함수 | YAML은 `yaml.safe_load(str)` 경로다: 표식 이름 `<unicode string>`, 파일 전체 엄격 UTF-8 해독 뒤 읽는다(`pyyaml::safe_load_text`). 호출 스택 기준선은 명령별로 다르다(`CallDepth`) |
| `serving_caps` · `_grep_yaml_int` | 동명 함수 | `re.search` 정규식의 역추적(값 앞 공백 되돌림)을 손으로 재현했다. 대체 해독(`errors="replace"`)·범용 개행 변환 포함 |
| `_parse_cli_value` · `_norm` · `_parse_knob_reasons` · `_parse_pins` · `_campaign_pins` | 동명 함수 | `json.dumps(sort_keys=True)`는 `dumps_sorted`(키 정렬 시점의 TypeError 포함) |
| `check_dataset_runtime` | 동명 함수 | `is_file`→`os.access`→1 MiB 단위 읽기와 증분 SHA-256(`sha2 =0.10.9`, 정확 고정) →`stat` 순서. 다이제스트를 저장하거나 재사용하지 않는다 |
| `resolve` | `bench_preset::resolve` | 반복 수는 R5 `resolve_repeats`, 캠페인 선언은 R5 `load_campaign_budgets`와 질의 어댑터(S4 이전의 Python 소유자)를 그대로 쓴다. `FULL_REPEATS_MIN`도 R5 상수다 |
| `_cmd_resolve`·`main` | `bench_preset_cli` (`bench_preset` 실행파일) | argparse 사용법/도움말/오류는 원본 출력으로 캡처한 표(`help_layouts_bench_preset.rs`, COLUMNS 1..480, `resolve`는 1..1500) |
| `--self-test` / `selftest_bench_preset.py` | `bench_preset_selftest` | K1~K4·V1~V15·R*·D1~D11을 실제 설치된 config·캠페인 스키마·소유자 복사본으로 평가한다(통과 출력을 찍지 않는다) |
| (Rust 전용) | `bench_preset_api` (`evaluate-jsonl` · `constants` · `native-help`) | R5와 같은 전송. `arguments_json`·`result_json` 옵트인 깊이 보존 |
| (Rust 전용) | `bench_preset_bench` | 인자 준비는 타이머 밖, 함수 본문·파일 I/O·해시·소유자 질의는 타이머 안, 결과는 `black_box` 뒤 해제. 특정 사례를 모른다 |

### 바인딩과 환경 계약

원본은 자기 파일 위치로 `SKILL_DIR`/`REPO_ROOT`를 찾았다. native 실행파일은 `BENCH_PRESET_SKILL_DIR`, `BENCH_PRESET_REPO_ROOT`(없으면 R5의 `REPEAT_AXIS_REPO_ROOT`)를
받고(경로는 원본의 `Path(__file__).resolve()`처럼 정규화) 없으면 이름 있는 `BenchPresetConfigurationError`/`RepeatAxisConfigurationError`로 거부한다.
캠페인 소유자는 R5와 같이 `REPEAT_AXIS_CAMPAIGN_ADAPTER`·`REPEAT_AXIS_PYTHON`으로 질의하며 `repeat_axis.py`·`bench_preset.py`로 폴백하지 않는다.
설치된 소유자/어댑터/config가 없으면 검사는 `NOT_RUN`으로 실패한다(숨은 임시 의존 없음).

### R1~R5 공유 파일의 변경(추가만)

* `argv.rs`: `Kind::Append`(`action="append"`)와 `Matches::values_of`, `bench_preset` 파서 4개 표. 기존 파서의 동작은 그대로다(전체 기존 검사 재실행).
* `pyvalue.rs`: `keyed_entries`를 `pub(crate)`로(비문자열 키의 값 복원).
* `pyyaml/{mod,reader}.rs`: `str` 원본 입구 `safe_load_text`/`Reader::from_text`(기존 파일 스트림 입구는 그대로).
* `repeat_api_cli.rs`: 호출 바인딩·레코드 보조를 `pub(crate)`로.

### 검증(이번 작성 단위가 직접 관측한 것)

고정 원본 관측을 그대로 재생했다: API 746개(검증 목록 25·`_parse_cli_value` 721) 전체 레코드, 해소/데이터셋 67개 전체 레코드와 트리 불변, CLI 45개의
종료 코드·stdout·stderr·전체 before/after 트리, 원본 자체검사 전사. 각 검사는 `checks/preset_*.rs`(Cargo)와 작업 폴더의 외부 재생 스크립트로 두 번 확인했다.
관리 `fmt`/`clippy --all-targets -D warnings`/debug 전체 Cargo(기존 163 + 새 9, 라이브러리 75)가 통과했다. 초기 실패(도움말 표 범위 부족, 재생 스크립트의 상대 경로 2건,
미포함 traceback 모듈 접두 2건, Clippy 2건, 보조 fixture 환경 미설정 NOT_RUN)와 교정은 작업 폴더 로그에 그대로 남아 있다.

### 알려진 한계와 미해결

* 해소 결과의 비문자열 dict 키 정렬 순서는 CPython의 집합 반복 순서를 모사하지 않는다(선언 순서를 쓴다; 혼합 형 키의 TypeError 문구는 집합 순서에 따라 원본도 달라질 수 있다).
* `json.dumps(sort_keys=True)`의 인코더 재귀 한계 오류는 재현하지 않는다(YAML 읽기가 먼저 같은 깊이에서 거부한다).
* 처리되지 않은 예외의 traceback은 프레임을 재현하지 않고 앞 텍스트와 최종 `Class: message` 줄을 보존한다(R5와 같은 규칙; `repeat_axis.RepeatAxisError`의 모듈 접두 포함).
* launcher/소비자 연결, 정본 적용, 독립 검증, 실제 계약 성능(정식 임시 레이아웃의 원본 831 기준선 포함)은 root 단계다. 이 단위에서는 스모크 실행만 했다.
* 기본 해소의 외부 Python 질의 지연(R5 041)은 S4 의존으로 그대로 남는다. 결과 캐시나 규칙 복제로 가리지 않았다.

## R6 정본 연결 이후 검사와 첫 성능 판정

프리셋 런처와 실제 sweep/broad-search 소비자, 복사 fixture·배포검사·명시적 node-local build의 일곱 번째 자체검사를 정본에 연결했다.
관리 build7자체검사·fmt/release Cargo172·Clippy, 고정746API/67해소/45CLI의 전체값·종료·파일결과, 독립 원본6경계와 설치 런처2경계,
전체배포128이 PASS다. 추적 소스386파일(Rust366/설치소유의존20)만 새 공백 경로에 복사하고 기존 관리 도구체인/cache를 명시 재사용한
source build7자체검사·관련 Cargo11도 PASS다. 전역 설정이나 배포 target/cache/인증을 복사하지 않았다.

첫 계약 성능은 기능 PASS와 다르다. pure652×200×11의 Python1819.047960ns/Rust1906.973367ns=0.953893배, 전체해소18값은
67.856966/124.622359ms=0.544501배였다. 전체 파일해시1/16/64MiB는0.644766/3.566663ms,7.709196/30.000562ms,
29.640115/119.364523ms로 회귀다. 실제CLI6개 집계83.656411/74.252723ms=1.126644배지만 개별024/043의 회귀도 보존한다.
원본 결과와 최종 파일은 모두 같다. 순수연산·ARM 해시 경로를 같은 세션에서 교정 중이며, Python 소유자 의존은 S4 후속에 남는다.
이 기록은 초기 관측이며 성능 완료나 전체 과업 완료가 아니다. 상세 원시·실행 조건은 상위 과업 문서의 R6 기능/성능 절에 연결한다.

### R6 성능 보정 — 검증 경로의 복사 제거와 해시 가속(후보)

기능 변경은 없다. 고정 원본 관측(API 746·해소 67·CLI 45·자체검사 전사)은 그대로 통과하고, 같은 원본이 답한 가장자리 문서 78건(키 순서 뒤집기,
`bool`/`float` 값 동치, 해시 불가 값, 큰 정수, 데이터셋 형태 등 · `checks/data/preset/validate_edge.jsonl`)을 `preset_api_cases`가 새로 재생한다.

* 원인: `validate_preset`이 문서의 knob·옵션·근거 목록·기본값을 값마다 깊게 복사하고, 키 계약을 통과 경로에서도 키 벡터를 만들어 확인하고,
  성공하는 knob마다 위치 문자열을 만들었다. 이제 문서를 제자리에서 읽는다: 키 집합은 선언 순서로 한 번 훑어 확인하고(어긋날 때만 원본과 같은 누락·미지 계산),
  옵션 값·등급·기본값은 빌려 쓰며(`Cow`), 메시지와 `repr`은 위반이 있을 때만 만든다. 값 동치 규칙(`True == 1 == 1.0`, 해시 불가 `TypeError`, 중복 키가
  첫 철자를 지키고 마지막 등급을 가진다)과 오류·위반 순서는 그대로다. `_parse_cli_value`는 knob 조회·공백 판정에서 문자열 복사를 없앴다.
* 해시: `sha2 =0.10.9`의 `asm` 기능을 켰다(정확 고정 · 공식 전이 의존 `sha2-asm`·`cc`·`shlex`·`find-msvc-tools`가 잠금 파일에 추가). aarch64에서는
  같은 크레이트의 SHA2 명령 백엔드가 실행 시 CPU 기능 감지로 선택되고 감지 실패 시 소프트 구현으로 돌아간다(`target-cpu` 지정 없음 · 노드 로컬 빌드 유지).
  직접 구현한 암호 코드나 Python 위임은 없다.

로컬 스모크(이 작업 단위의 관측이며 계약 성능이 아니다): 순수 652개×200회에서 원본 약 1791 ns/case, Rust 약 409 ns/case(약 4.4배). 같은 입력의 보정 전 Rust는
약 1.9 µs/case였다. 해시는 보정 전 소프트 119 ms(64 MiB)에서 asm 약 33 ms로 줄었으나 `hashlib`(OpenSSL) 약 29 ms보다 약 13% 느리다 — 해당 크레이트의
압축 함수 자체가 한계다(읽기는 약 2.3 ms, LTO·codegen-units=1에서도 변화 없음). 이를 넘기려면 다른 암호 라이브러리의 별도 승인이 필요하며 이 후보는 그 선택을 하지 않았다.
기본 해소의 외부 Python 질의 지연(S4 의존)은 그대로이며 캐시·규칙 복제로 가리지 않았다.

### R6 기능 보정 2건과 해시 백엔드 확정(후보)

**1. NaN의 객체 정체성 — [REJECTED_REVIEW: payload 비트 부호화 시도, 아래 "객체 정체성 최종"이 대체].** 독립 검증이 원본 해소(`resolve`)에서 `warmups` 기본값과 옵션 값이 JSON의 `NaN`일 때 값 `NaN`·등급 `auto`를 내는데
이전 후보는 `PresetError`(선택지 밖)를 낸 것을 확인했다. CPython의 `in`·dict 조회·컨테이너 동등 비교는 `==` 전에 "같은 객체인가"를 먼저 묻고,
JSON 디코더의 `NaN`은 프로세스 전체에서 한 객체이며 YAML `.nan`은 `SafeConstructor.nan_value` 한 객체, `!!float nan`은 노드마다 새 객체(별칭은 그 노드의 객체)다
(관측은 `checks/preset_nan_identity.rs`에 고정). `f64`는 출처를 담지 못하므로 `pyvalue::NanOrigin`이 NaN의 사용되지 않는 payload 비트에 출처를 기록한다
(`JsonConstant`·`YamlConstant`·`Node{load,node}`, 태그 없는 NaN은 새 객체). 숫자 `==`는 그대로다(`nan != nan`) — 정체성은 `pysort::same_or_eq`와 컨테이너 항목
비교에서만 쓰이고, `bench_preset`의 목록/딕셔너리 소속 판정(옵션 키 맵·기본값·근거 코드·`dict.fromkeys` 중복 제거)이 그것을 쓴다. 출력·정렬·`repr`은 달라지지 않는다.
JSON 디코더와 YAML 읽기는 각각 위 출처의 NaN을 만든다(공유 런타임 수정: `pyvalue`·`pyjson`·`pysort`·`pyyaml/compose`). 전체 Cargo(기존 NaN/키/JSON 경계 포함)와
고정 746/67/45·자체검사·독립 직접 해소 4건이 통과한다. 기존 R3 키 관측(1/2/1/2)과 직렬화 동작은 바뀌지 않았다.

**2. 검사 등록.** `checks/preset_api_cases.rs`가 고정 원본 관측 78건(`validate_edge.jsonl`)을 실제로 재생한다(이전 기록의 등록 주장은 틀렸다 —
파일이 실제로는 746건만 읽었다). 독립 검증의 직접 해소 4건(통과 3 + NaN 1)은 `preset_resolution_cases`의 `independent_cases.json`으로 등록했다.

**3. 해시 백엔드(최종).** 단계 기록: 처음 계획은 `sha2 =0.10.9`(소프트)였고, 다음 보정에서 `asm` 기능을 켰으나 aarch64의 해당 백엔드는 `hashlib`(OpenSSL)보다
약 13% 느렸다. 승인된 예외로 데이터셋 SHA-256은 `ring =0.17.14`(`digest::Context::new(&SHA256)`/`update`/`finish`, 1 MiB 조각)로 교체했다. 활성 백엔드는 `ring`뿐이며
`sha2`와 그 전이 의존은 잠금 파일에서 제거됐다. ring은 자체 aarch64/x86_64 어셈블리와 실행 시 CPU 기능 감지를 쓰고(Linux는 보조 벡터, x86은 cpuid) 감지되지 않으면
범용 구현으로 돌아간다. `target-cpu`·LTO·전역 플래그는 쓰지 않았고 노드 로컬 소스 빌드는 그대로다(C/어셈블리 컴파일러는 기존 것을 쓴다). 잠금 파일의 Windows 전용 항목은
다른 대상의 기록일 뿐 Linux 빌드에서 컴파일되지 않는다. 로컬 스모크(계약 성능 아님): 1/16/64 MiB 약 0.45/7.18/28.9 ms(같은 입력의 `hashlib` 약 0.46/7.25/29.1 ms).

### R6 NaN 객체 정체성 최종(후보) — payload 비트 시도의 대체

앞 절의 payload 비트 부호화는 검토에서 거부됐다(`Node{load,node}`가 각 24비트로 잘려 `{0,7}`과 `{16777216,7}`이 같은 비트가 되는 결정적 충돌 — 승인되지 않은 정체성 한계).
기록은 지우지 않고 그 시도를 `REJECTED_REVIEW`로 표시했다. 최종 방식은 `f64`에 아무것도 싣지 않는다:

* NaN 객체 = `Arc`로 감싼 NaN을 `PyValue::Shared`로 나른다(YAML 별칭이 이미 쓰는 운반체). 값 복제는 참조 복제이므로 한 객체는 한 객체로 남고,
  따로 만든 두 `Arc`는 두 객체다. 정체성은 할당 자체라 번호·횟수·슬롯이 없고 충돌도 없다. 전역 등록부도 없다(JSON/YAML 상수만 프로세스 전역 `OnceLock`의 `Arc` 하나씩).
* 출처: JSON `NaN` = JSON 상수 하나(`pyvalue::json_nan_constant`, 문서가 달라도 같은 객체), YAML `.nan` = YAML 상수 하나(`yaml_nan_constant`, JSON과 다른 객체),
  그 밖의 YAML NaN(`!!float nan` 등) = 노드마다 새 `Arc`(노드 값이 한 번만 만들어져 캐시되므로 별칭이 같은 객체를 재사용한다 — 조립 단계에서 벗겨지지 않는다),
  운반체가 없는 `PyValue::Float(NaN)`은 아무도 공유하지 않는 객체다.
* 범위: 운반체는 `pyjson::loads_objects`/`loads_text_objects`와 `pyyaml::safe_load_text`(= `yaml.safe_load(str)`)만 만든다. 기존 소비자 대부분이 JSON 값을 `PyValue::Float`로 직접 대조하므로
  일반 `loads`와 파일 YAML 읽기는 그대로 평범한 `Float`를 낸다(그 소비자들의 동작·직렬화는 불변). 프리셋 프로그램(`bench_preset_api`의 요청 줄과 `arguments_json`, `--pin column_map=JSON`, 프리셋 YAML)만 운반체를 쓴다.
* 판정: 숫자 `==`는 그대로 거짓(`py_eq`), `pysort::same_or_eq`와 컨테이너 항목 비교·딕셔너리/목록 소속 판정만 "같은 객체"를 먼저 본다. YAML dict 키의 NaN은 기존 `nan_origin` 키 정체성(R3 관측 1/2/1/2)을 그대로 쓴다.
* 검증: `checks/preset_nan_identity.rs`(JSON 상수가 문서 사이에서 한 객체 · 일반 디코더는 `Float` 유지 · YAML `.nan` 싱글턴/노드별 새 객체/별칭/문서 간/JSON과 구별 · 2000개 객체의 비충돌 · 복제는 같은 객체, 숫자 `==`는 거짓),
  독립 직접 해소 4건과 고정 746/67/45·78·자체검사 전사, 전체 release Cargo 179개, 관련 debug 검사. JSON 상수를 일반 `Float`로 되돌리면 독립 04와 JSON 정체성 검사가 실패함을 확인했다(복원 후 `cmp` 동일).

## R6 정본 최종 계산·연결 검사

정본에서 build7실제 자체검사/fmt/release Cargo179/Clippy, 원본746API/67해소/45CLI, 공유모듈영향 R1~R5 고정회귀,
추가독립4/4와 전체배포128을 확인했다. 78원본edge/독립4/NaN객체5는 실제등록·실행됐다. Gitindex소스389파일(Rust369/소유의존20)만
새공백경로로복사한 offline source build7자체검사/관련Cargo18도 통과했다. 기존 관리 도구체인/cache는 명시 재사용했고
컴파일target·cache·인증을 배달하지 않았다. 최종 active해시는 ring=0.17.14, 객체정체성은 R6 opt-in Shared Arc다.

같은계약11교차round의 pure652×200: Python1822.581894ns/Rust415.718880ns=4.384169배.
전체read/hash/stat/반환1/16/64MiB는0.645288/0.569383ms,7.708748/7.474297ms,29.737327/29.357274ms(각1.133311/1.031368/1.012946배).
실제BashCLI6개 집계83.30146167/74.90080217ms=1.112157배, 모든출력/종료/파일이같다. 초기회귀원시도보존한다.

전체해소18값은67.57986556/121.285660ms=0.557196배, CLI024/035/043은0.873034/0.792860/0.803665배로느리다.
아직Python인캠페인소유자질의비용이며 S4후같은입력을재측정할열린항목이다. 반복/캠페인규칙복제나cache로가리지않았다.
상태는 계산핵심검증/연결완료·소유의존지연OPEN이다. 로컬표본을서빙/LLM이나통계적우월성으로확대하지않는다.
상세원시·검사주체·초기실패·native실행분모는 상위과업문서의R6최종절에연결한다. 전체과업은계속한다.

## R7 lite 지표(`lite_metrics`) 이행 — 후보 기록 (정본 적용 전)

상태: **후보 변환·자체검사 COMPLETE, 정본 연결·독립 검증·전체 배포·성능은 NOT_RUN(root 소유)**. 원본은 head `831068cb5777798cc8209374ecb42e0657d671a4`의
`lite_metrics.py` 405줄(`git show`로 읽은 참조이며 Python 런타임 대체 경로가 아니다). 이 후보는 6프로필 평가가 아니며 전체 과업 완료 선언이 아니다.

**1. 출력 경로.** `r7-candidate/`: 새 소스 `src/lite_metrics.rs`(계산·읽기·렌더·판정) · `src/lite_engine_log.rs`(엔진 로그 패턴) · `src/lite_metrics_cli.rs`(옛 CLI) ·
`src/lite_metrics_api_cli.rs`(API 전송/상수/native-help/self-test) · `src/lite_metrics_selftest.rs` · `src/help_layouts_lite_metrics.rs`(원본에서 캡처한 생성 데이터) ·
`src/bin/{lite_metrics,lite_metrics_api,lite_metrics_bench}.rs`, 검사 `checks/lite_metrics_{cases,files}.rs`와 `checks/data/lite_metrics/`. 의존성은 늘리지 않았다
(`Cargo.lock` 불변, `--locked` 빌드).

**2. 원본 → Rust 대응.**

| 원본 | Rust |
|---|---|
| `_load_json` | `lite_metrics::load_json` |
| `_num` | `number_or_none` |
| `gen_tps_from_bench` / `accept_len_from_bench` / `cold_ttft_ms` | 같은 이름, 결과는 `GenTps` / `AcceptLength` / `Option<f64>` |
| `_KV_PATTERNS` · `_KV_TOKENS_PAT` · `_WEIGHTS_PAT` · `_NONTORCH_PAT` · `_grep_first` | `lite_engine_log` 손작성 매처(우선순위 순서·게으른 `[^\n]*?`·유니코드 `\d`/`\s` 보존). 공개 상수는 원본 패턴 문자열 그대로 |
| `parse_engine_log` | `parse_engine_log` → `EngineLog` |
| `node_capacity` | `node_capacity` → `NodeCapacity` |
| `render_single` / `render_multi` | 같은 이름(원시 값 인자) |
| `build` | `build` → `LiteMetrics`(`to_value`가 원본 dict 순서) |
| `_requests_ok` / `judge_lite` / `_verdict` / `read_lite_verdict` | `requests_ok`(`RequestsCheck`) / `judge_lite`(`Judgement`, `LiteVerdict`) / `verdict_record` / `read_lite_verdict` |
| `LITE_VERDICTS` · `LITE_EXIT` · `GIB` · `_INITIAL_TEST_FAILED` · `_CONNECT_ERROR_RE` | `lite_metrics` 상수, `lite_metrics_api constants`가 한 JSON으로 노출(정규식은 패턴 문자열) |
| `main`(argparse `--raw-json`·`--json`·`--judge`) | `lite_metrics_cli::run`(옛 CLI만), 사용법·도움말은 `argv::LITE_METRICS` + 폭별 캡처 51단계 |

**3. 보존한 계약(원본 버그 포함, 바꾸지 않았다).** 값은 `PyValue` 원시 입력으로 받아 원본이 실제 연산에서 내는 예외·truthiness를 그 자리에서 낸다: `.get`의 `AttributeError`,
`bool`이 `int`, `float("1_0")`·전각 숫자, `NaN`/`inf`의 `round`·서식, 거대한 `int`의 `OverflowError`, `%d`/`format`의 `TypeError`/`ValueError`, `open(경로)`의
`TypeError`(비-문자 경로)·`ValueError`(NUL·음수 기술자)와 `OSError`(→ fail-soft). `_load_json`은 `OSError`/`ValueError`만, `parse_engine_log`는 `OSError`만 삼킨다.
엔진 로그는 `errors="replace"`와 범용 개행(`\r\n`·`\r` → `\n`)으로 읽는다. `reserved_total_gib`는 CPython 3.12의 보정 합산(`sum`)을 따른다. 원본 신뢰성 관측
(`NaN` 용량이 `pass`, `bool` 요청 수 통과, `--judge` tmp가 raw를 가리키는 자기 링크)은 호환 이행에서 고치지 않았고 별도 신뢰성 단계에서 다룬다. `--judge`는 verdict가
6/7이어도 종료 0이며 호출부가 raw의 `lite_verdict`에서 종료코드를 파생한다.
**파일 경계**: `open(PATH+".tmp","w")` → 점진 `json.dump(indent=1, ensure_ascii=False)` → `os.replace` 순서. 심볼릭 링크 raw는 일반 파일로 교체되고, tmp 링크는 따라 쓴 뒤 링크째 이동하며,
tmp가 디렉터리면 열기에서 `IsADirectoryError`(변화 0), 짝 없는 서로게이트는 그 문자열 조각 앞까지(독립 기준선 703바이트와 같은 접두)만 tmp에 남고 `UnicodeEncodeError`로 끝난다.
**읽기 방식**: 파일 JSON은 `pyfs`의 `json.load` 판(엄격 UTF-8, `NaN` 허용, 일반 `Float`)이다. NaN 객체 정체성이 필요한 규칙이 없어(`failed in (0, None)` 등은 값 비교) R6 `loads_objects`는
API 전송(행·`arguments_json`)에만 쓰고 파일 읽기에는 쓰지 않는다.

**4. 공유 모듈 변경과 이유.** (a) `pyvalue::dumps_indented(indent)` + `Layout::Pretty{indent}`: `--judge`의 `json.dump(indent=1)`에 필요(기존 `dumps_pretty`는 `indent=2` 그대로, 영향 검사 =
JSON 직렬화를 쓰는 R1~R6 전부). (b) `pynumber::format_fixed`/`round_to_places`의 `places == 0`: `'%.0f'`는 소수점이 없어야 하나 기존은 `5.0`을 냈다(원본 `:.0f` 필요, 기존 소비자 없음). (c)
`engine_log::matches_ignoring_case`를 `pub(crate)`로: 연결 오류 정규식 `re.I`의 `i`/`s` 등가(`İ ı ſ`)에 같은 매처를 쓴다(다른 모듈 패턴으로 대체하지 않았다). (d) `argv::LITE_METRICS`·`lib.rs`·
`Cargo.toml`의 bin/test 등록. 반복/캠페인/프리셋 규칙은 복제하지 않았다.

**5. 외부 표면.** 옛 `lite_metrics`는 원본 CLI만 받는다(`self-test` 등은 원본의 `unrecognized arguments`). Rust 전용은 `lite_metrics_api`: `evaluate-jsonl`(행 `{id,operation,arguments}` →
`{id,kind:value,value}` 또는 `{id,kind:exception,type,message}`, `SystemExit`은 별도 `type`, 선택 `arguments_json`·`result_json`), 연산 15개
(`gen_tps_from_bench accept_len_from_bench cold_ttft_ms parse_engine_log node_capacity render_single render_multi build judge_lite read_lite_verdict _load_json _num _requests_ok _verdict constants`),
`constants`, `native-help`, `self-test`(핵심 연산 실평가 + 소유자 D1 픽스처 `LITE_METRICS_FIXTURE` 필수, 부재/형제 파일 부재는 실패, 건너뛰기 없음). 출처 라벨 `lite_metrics.judge_lite(raw)`는 그대로다.
`lite_metrics_bench`는 인자만 타이머 밖에서 준비하고 전체 결과를 `black_box`·drop 하는 일반 하네스이며(케이스 인식 없음) 이 후보는 성능을 주장하지 않는다(스모크만).

**6. 연결되지 않은 경계(root).** 런처 `lite_metrics.sh`/`lite_metrics_api.sh`, `lite_metrics_native.py` 브리지, `lite_bench.sh`·Python 소비자·인라인 호출부·검증 스크립트의 실제 배선, 여덟 번째 자체검사,
통합 초안 12경로 적용은 이 후보에 없다. 브리지 프로토타입도 만들지 않았다.

**7. 실제 검사(작업 경로 작업 검사 폴더, 로그 `logs/`, 도구체인 = 관리 Rust 1.99.1, `rust_bootstrap.sh exec`, offline·locked, target = 작업 검사 폴더의 `cargo-target`).**
환경변수: `EASY_VLLM_SOURCE_ROOT`/`BENCH_PRESET_REPO_ROOT`/`REPEAT_AXIS_REPO_ROOT`=실제 대상, `REPEAT_AXIS_CAMPAIGN_ADAPTER`, `PARSE_GUIDELLM_FIXTURE`, `LITE_METRICS_FIXTURE`(`cargo_env.sh`).

| 검사 | 결과 |
|---|---|
| `cargo fmt -- --check` | PASS (`fmt-check-attempt4`) |
| `cargo build --release --bins` | PASS (`build-release-attempt3`) |
| `cargo clippy --release --all-targets -- -D warnings` | PASS (`clippy-attempt3`; pedantic 탐침으로 린트가 실제로 도는 것도 확인) |
| `cargo test --release` 전체 | PASS 48 suite · 195 테스트(기존 179 + 새 16: cases 3, files 5, `lite_engine_log` 단위 8) (`test-all-release-attempt2`) |
| 새 검사 debug(`--lib` + 두 검사) | PASS 83+3+5 (`test-lite-debug-attempt1`, 산술 오버플로 확인) |
| 고정 원본 API 132 + CLI 18(외부 `check_r7_external.py`, `result.json`) | PASS 132/132 · 18/18 (`external-attempt2`; 첫 시도는 131/132 FAIL — `ttfts:[NaN]`이 `Shared` NaN을 `float()`에 넘겨 `TypeError`, `float_call(first.peel())`로 해소, `external-attempt1` 보존) |
| 같은 고정 데이터의 인트리 재생(`checks/lite_metrics_cases.rs`) | PASS API 132 · CLI 18 · 전/후 트리 전체 |
| 독립 기준선 FS 5건(`compare_baseline_fs.py`, 전체 전/후 바이트·링크·모드·출력·종료) | PASS 5/5 (`fs-baseline-attempt1`) |
| 추가 원본 포획 306행(`odd_cases.jsonl`: 숫자 문자열·거대 정수·NaN/inf/-0.0·기술자/비경로 인자·CRLF/전각/잘못된 UTF-8 로그·보정 합산·정수 나눗셈·예외 전수) | PASS 306/306 (`test-lite-cases-attempt3`) |
| 차분 탐침(원본 in-process 관측기 ↔ API): 임의 형 입력 3910/5874/5880/5854행, 임의 엔진 로그 24 000개 | 불일치 0 (`parity_probe.py`, `engine_fuzz.py`; 탐침 자체가 `built` 예외 행을 건너뛰어야 했던 하네스 결함 1건은 수정·기록) |
| CLI argv 차분 33줄(약어·`=`·`--`·빈 경로·디렉터리·반복 옵션 등) | 32 일치 + 1건(`--js` = `--json` 약어)은 원본의 `engine` 집합 순서 비결정만 다름(파싱 비교로 일치) (`cli_probe.py`) |
| `lite_metrics_api self-test`(`LITE_METRICS_FIXTURE`=소유자 D1) | PASS 15 항목, 환경변수 미설정이면 FAIL 항목 + 비-0 (로그 `self-test-with-fixture`; 미설정 `self-test-without-fixture` 비-0; 형제 파일 부재 `self-test-missing-sibling` 비-0) |
| 성능 | NOT_RUN (root가 사전 선언된 세 경계로 측정, `lite_metrics_bench`는 스모크 실행만) |

**8. 알려진 한계와 미해결.**
(1) [정정됨, 아래 R7.9] 정수 경로 기술자 읽기는 처음에 `/proc/self/fd/N`을 다시 열어 오프셋 0에서 읽고 기술자를 닫지 않았다 — 원본 `open(fd)`와 달랐다.
(2) 정수÷정수 나눗셈은 정확 반올림이지만 결과가 비정규 수일 때의 이중 반올림은 CPython과 같은지
확인하지 못했다(입력 경계 밖). (3) `engine` 하위 맵 키 순서는 고정 순서다(원본은 `set` 순서라 프로세스마다 다름, 계약대로 비순서 비교). (4) traceback 프레임은 재현하지 않고 앞부분과 마지막 `Class: message`만
같다. (5) `_grep_first`는 일반 정규식을 받는 API 연산으로 노출하지 않았다(패턴 상수만). (6) 포획한 원본 traceback 한 건의 개발자 절대경로는 이식 가능한 검사 데이터에서 `scripts/lite_metrics.py`로 치환했다(비교는
traceback 앞부분과 마지막 줄만 쓰므로 판정 대상 불변). (7) 이 기록의 어떤 숫자도 실제 서빙·LLM 측정이나 통계적 우월성 주장이 아니다.

### R7.9 기술자 인자(`open(fd)`) 정정 — root 검토 지적 반영

**문제.** 0이 아닌 `int`/`bool` 경로는 원본에서 `open(fd)`이다: 그 기술자의 **공유 현재 오프셋**부터 끝까지 읽고(오프셋은 끝에 남는다), 읽기·디코드가 실패해도 그 번호를 **닫는다**.
첫 구현은 `/proc/self/fd/N`을 새로 열어 오프셋 0에서 읽고 닫지 않았다. `0`/`False`는 거짓이라 열기 전에 반환한다(stdin은 그대로).

**원본 포획(후보 수정 전).** `fd_cases_definition.py` + `capture_fd_original.py`(원본만 사용, 케이스마다 격리된 자식 Python 프로세스)가 22개 시나리오의 결과·복제 기술자로 본 공유 오프셋·열림 여부·해제 번호 재사용·
자식 stdout이던 파일의 오프셋/바이트를 기록했다(명령·원시 출력: 작업 검사 폴더 `fd-original/`, `fd-original-v2/`; 앞 20건은 v2에서 동일). 현재 오프셋 유효 JSON, 오프셋 0, EOF, 깨진/빈/잘못된 UTF-8, 쓰기 전용, 같은 번호 재호출과 재사용, 이미 닫힌 번호,
`parse_engine_log` 0이 아닌 오프셋·CRLF·잘못된 UTF-8·쓰기 전용·재호출, `0`/`False`(stdio 불변), 읽기-쓰기 stdout(오프셋 2에서 `int 1`과 `True`)·쓰기 전용 stdout, 디렉터리 기술자(`open`이 소유 전에 `IsADirectoryError` →
**닫히지 않음**).

**수정.** `lite_metrics::read_descriptor`가 R4 `guidellm_cli::read_descriptor`와 같은 소유권 방식으로 `File::from_raw_fd`를 쓴다. 닫히는 규칙은 원본과 같다: 열려 있지 않은 번호와 디렉터리는 `/proc/self/fd/N`의 `stat`(R4와 같은 탐침)으로 **`from_raw_fd` 이전에** 거른다
(`EBADF`/`EISDIR`, 소유권을 잡지 않으므로 **닫지 않는다**). 통과한 비디렉터리 기술자만 `File::from_raw_fd`로 넘겨받고, 그 뒤에는 읽기가 성공하든 실패하든 `File`의 drop이 닫는다(다시 열지 않으므로 공유 오프셋 유지).
(R7.9 초판은 `from_raw_fd` 뒤에 `metadata`로 거르고 `into_raw_fd`로 돌려줬다 — 생성 시점에 유효·열림 전제를 어기는 순서여서 root 검토로 이 순서로 바꿨다.) `unsafe` 계약은 함수 문서와 `SAFETY` 주석에 있다: 유효·열림은 검증으로, 소유는 원본 계약으로 충족한다 — 원시 읽기가 가리킨 번호는 이 한 번의 읽기에 넘겨지고, 프로그램에
그 번호의 다른 소유자가 없으며 이 함수가 도는 동안 다른 스레드가 그 번호를 열거나 닫거나 읽지 않는다(검사와 인수 사이의 동시 close는 이 계약이 배제하는 사용이다). `_load_json`/`parse_engine_log`의 `OSError`·`ValueError` 처리 구분은 그대로다.

**등록 검사.** `checks/lite_metrics_descriptors.rs`(`harness = false`, `Cargo.toml` 등록): 같은 실행 파일이 검사자이자 시나리오마다 다시 실행되는 격리 자식이다. 닫는 시나리오(fd 1 포함)가 실행기의 기술자를 건드리지 못하고
libtest 배너가 리디렉션된 stdout을 오염시키지 않는다. 자식은 결과 파일로만 관측을 남긴다. 수정 전 코드로 먼저 실행해 실패를 보존했다(`test-fd-descriptors-attempt1-BEFORE-FIX`: 오프셋·닫힘·쓰기 전용·0이 아닌 오프셋 파싱·stdout 읽기
불일치). 수정 후 22/22 일치(`test-fd-descriptors-attempt3`).

**경계.** 격리 자식에서 확인한 것은 위 22개 시나리오다. 소켓·파이프·tty 기술자, 논블로킹 기술자의 읽기 중 `EAGAIN`, 다른 스레드가 같은 번호를 동시에 쓰는 경쟁은 포획·검사하지 않았다(그런 번호는 원본도 같은 방식으로 닫는다고 가정하지 않는다).
기술자 0/1/2의 비격리 호출(실행 중인 호출부의 stdin/stdout/stderr)은 원본처럼 stdout/stderr를 닫을 수 있다 — 호출부가 그런 번호를 원시 JSON에 쓰지 않는다는 것은 이 후보가 보장하지 않는다.


## R7 정본 연결·최종 측정 — 2026-11-05T04:37:01+09:00

상위 과업의 계획·개발기록·검증 문서에 세부 근거를 보존한다. 현재 R7은 두 셸 런처·계산 없는 Python JSON bridge·실제 소비자에 연결됐다. 자동 빌드/설치나 Python fallback은 없다. 기존 실행 비트는 유지했고 CRLF 입력 하나는 같은 디렉터리의 .gitattributes -text로 추적 bytes를 보존한다.

관리 source build8실제 자체검사(D1 포함), fmt/release195+별도FD22/Clippy, 고정132API/18CLI, 독립5FS+3FD, 실제연결3control, 전체배포128, 새 공백 경로의 Gitindex432파일(Rust406/의존26) source build8/관련Cargo10+FD22가 PASS다. 원본의 NaN/boolean 요청 통과·tmp 자기symlink는 후속 신뢰성 개선 대상으로 보존했다. FD는 단독 소유 전달·동시close/use없음 조건이며 검사는 그 범위를 넘는 보증이 아니다.

품질 후 고정11교차round pure92×200: Python490.530924ns/Rust273.985761ns=1.790352배. 실제읽기21:12777.047619ns/6531ns=1.956369배. 실제BashCLI6의 round평균중앙값26.173297ms/4.078241ms=6.417791배. 개별 정상·판정5개는3.77~4.85배, raw배열 오류1개는15.16배이며 오류traceback프레임 전문은 비교범위밖이다. 집계를 모든 정상경로의 개선으로 확대하지 않는다. 로컬 표본이고 실제서빙·LLM·통계적 우월성 주장은 아니다.

이 연결 단위는 CONNECTED_CORE_VERIFIED이며 전체 Rust/물리노드/프로필/LongE2E 수용은 아직 아니다.

## R8 셀 분류(`classify_cell`) 이행 — 후보 기록 (정본 적용 전)

상태: **후보 변환·자체검사 COMPLETE(아래 표), 정본 연결·독립 검증·전체 배포·성능은 NOT_RUN(root 소유)**. 원본은 head `831068cb5777798cc8209374ecb42e0657d671a4` 의 `classify_cell.py` 1515줄(자체검사 83 포함) 전체다. 이 후보는 6프로필 평가·전체 과업 완료 선언이 아니다. 후속 교정 2회(역할 소유자 전송·정수 한도 상속, 아래 R8.2) 반영본이다.

### R8.1 원본 → Rust 대응

| 원본 | Rust |
|---|---|
| 상수(`OUTCOME_*`·`KILL_EVENT_KINDS`·`BENCH_MODE_*`·`DOWNGRADE_*`·`CORRELATION_*`·`NODE_SCAN_*`·`LITE_VOID_REASONS`…) | `classify_cell` 상수, `classify_cell_api constants` 가 한 JSON 으로 노출 |
| `classify` · `_miss_source` | `classify` · `miss_source` (임의 JSON 값 인자: 원본 동적 타이핑의 `TypeError`/`KeyError`/`AttributeError` 문구까지) |
| `kill_events_in_window` · `_newest_event_ts` · `correlate_nodes` · `_utc` · `_iso` | 같은 이름. 시각은 `utc::UtcInstant`(마이크로초·타임존·`astimezone` 범위), 허용오차는 `pydyn::timedelta_seconds_micros`(CPython `timedelta(seconds=…)` 생성 규칙: int/bool 정확, float 반올림, 범위 `OverflowError`), `datetime` 이 아닌 값은 `Moment::Other` 로 사용 시점에 Python 연산 `TypeError` |
| `events_scan_summary` · `bench_mode_kind` · `declared_lite_record` · `_bench_mode_record` | 같은 이름(`BenchModeFields`) |
| `bench_mode_record_path` · `read_bench_mode_record` · `write_bench_mode_record` | 같은 이름. 쓰기 = `mkstemp` 동형(0600·무작위 이름) → `json.dump(indent=2)` **청크 단위** 인코딩(짝 없는 서로게이트의 위치 = 그 청크 안 위치) → `os.replace`, 실패 시 임시 파일 제거 |
| `read_cell_lite` · `_read_events` · `scan_node_events` | 같은 이름. glob 은 `os.path`/`fnmatch`/`glob` 이식(`ospath`: 경로·노드 이름의 `[`·`*`·`?` 는 패턴, 숨김·디렉터리 제외, 정렬) |
| `classify_bench_mode` | 같은 이름. 반복 요약·판정점 조건 = 기존 Rust `repeat_axis::{summarize, repetition_established}`, 도구 토큰 = `guidellm::BENCH_TOOL`, lite 판정 어휘 = `lite_metrics::read_lite_verdict` 를 **직접** 호출(규칙 복제 0, Python 폴백 0) |
| `events_node_plan` · `resolve_self_node` · `_node_identity_resolver` · `_node_role_contract` | `events_node_plan` · `Owners::{resolve_self_node, node_identity_resolver, open_role_contract}` (아래 R8.2) |
| `main`(argparse 17옵션 · 두 모드) | `classify_cell_cli::run` + `argv::CLASSIFY_CELL`(폭별 148 레이아웃 캡처 `help_layouts_classify_cell.rs`) |
| `_self_test`(83 검사) | `classify_cell_selftest`(원본 점검을 **실제 연산으로** 이식 · 이름은 원본 AST 에서 추출한 `classify_cell_selftest_names`) |

외부 표면: 옛 `classify_cell`(원본 CLI 만) · `classify_cell_api`(`evaluate-jsonl` · `constants` · `self-test` · `native-help`; 행 `{id,operation,arguments}`, 선택 `arguments_json`·`result_json`; 경고가 있으면 행에 `"stderr"`; **datetime 은 `{"datetime_utc": ISO}`** 로 전송) · `classify_cell_bench`(인자 준비만 타이머 밖, 함수 본문·파일 읽기/쓰기/정리·소유자 질의·반환/해제가 안; 캐시·사례 인식 없음).

### R8.2 소유자 경계(OPEN_S3) — 규칙 복제 없음

* 설치 위치: 원본은 `__file__` 로 소유자를 찾았다. native 는 `CLASSIFY_CELL_SCRIPTS_DIR`(없으면 `…/adversarial-benchmark/rust/target/<profile>/` 설치 배치, 그다음 `EASY_VLLM_SOURCE_ROOT`)를 `abspath` 로 정규화해 같은 상대 경로(`../../terraforming_node/scripts/…`, 런타임 배달본 `../../../runtime/node_blackbox/…`)를 쓴다. 호출자 cwd 는 쓰지 않는다. 미설정 + 소유자가 필요한 연산 = 이름 있는 `ClassifyCellConfigurationError`(침묵 "부재" 아님).
* `node_identity.sh`: **실제** `bash <resolver> --resolve --repo REPO` 를 원본과 같은 60초 한도·`text=True` 의미로 실행하고, 원본 실패 문구(`rc=…`·stderr 앞 두 줄·`실행 실패: …`·`timed out after 60 seconds`)를 그대로 싣는다.
* `node_role_contract.py`: 조회 어댑터(Python, 임시) `classify_cell_role_query.py`(`CLASSIFY_CELL_ROLE_ADAPTER`, 미지정이면 scripts 폴더 옆 같은 이름, 인터프리터 `CLASSIFY_CELL_PYTHON`)가 소유자의 `load_yaml`/`evaluate_manifest` 를 그대로 부르고 **문서·결과를 소유자 프로세스에 둔다**. Rust 는 분류기 코드가 실제로 만지는 것만 기계 프리미티브(`get`·`items`·타입/길이·`materialize`)로 지연 조회한다: `doc.get("nodes")`, 각 노드 `role`, `result["violations"]`, 각 위반 `code`. 읽지 않는 메타데이터(float 로 못 바꾸는 큰 정수 키·자기참조 앵커)는 계획을 바꿀 수 없다. 소유자 예외는 원본 보고 문구로, 소유자 부재·적재 실패는 원본의 "부재"로, 어댑터 설정 누락·통신 실패는 `RoleContractAdapterError`(폴백 없음). 이 경계의 지연(프로세스 시작)은 **OPEN_S3** 이며 S3 뒤 같은 입력으로 다시 잰다.
* **R8.2 교정(root 검토)**: 첫 전송은 문서 전체를 타입 JSON 으로 복사해 유효 매니페스트 둘(미사용 `2**2048` 정수 키, 미사용 `&loop [*loop]`)을 `manifest 판독 실패` 로 거부했다(원본은 유효 계획). 초기 실패 기록 작업 폴더 `logs/role-rows-initial-failure.json`·초기 어댑터 `logs/classify_cell_role_query.initial-protocol.py.txt` 보존. 새 프로토콜 후 같은 두 원본 행 PASS(`role-rows-after-fix-*.json`), 숫자 키 동일성은 정확(float 변환 없음). 등록 검사: `checks/classify_cell_cases.rs::valid_manifests_with_unread_metadata_equal_the_original`(실제 소유자·어댑터, 원본 기대값 `role_transport_cases.json`), `checks/classify_cell_files.rs::metadata_the_classifier_never_reads_cannot_change_a_valid_plan` · `owner_answers_and_exceptions_become_the_originals_plan_reports`. 어댑터의 테스트 사본은 `checks/data/classify_cell/classify_cell_role_query.py`(root 가 적용하는 `scripts/classify_cell_role_query.py` 와 같은 내용이어야 한다).

* **R8.2 교정 2(root 검토, 정수 문자열 한도)**: 어댑터가 시작 시 `sys.set_int_max_str_digits(0)` 을 전역으로 바꿔 소유자의 YAML 파서 동작이 달라졌다(원본은 상속된 4300 을 적용해 5000자리 십진 값 매니페스트를 `Exceeds the limit (4300 digits)…` 로 거부, 후보는 유효 계획). 이는 JSON 봉투 한도가 아니다(JSON 으로 가는 것은 매니페스트 경로뿐). **전역 설정 변경을 제거**했다: 소유자의 적재·`load_yaml`·`evaluate_manifest` 는 인터프리터의 상속 설정으로 실행되고, 어댑터는 정수를 **16진 문자열**로 보내(십진 변환·한도 무관) 직렬화에 한도 결정이 없다(어댑터에 4300 같은 규칙 상수 없음). 초기 실패 `logs/digit-row-initial-failure.json`·초기 어댑터 `logs/classify_cell_role_query.setdigits0.py.txt` 보존, 수정 후 같은 원본 행 PASS(`digit-row-after-fix.json`), 앞선 미사용 메타데이터 두 행 PASS 유지. 등록 검사: `classify_cell_cases.rs::valid_manifests_with_unread_metadata_equal_the_original`(세 번째 행 = 5000자리 값, 원본 기대값), `classify_cell_files.rs::a_huge_integer_the_classifier_reads_crosses_the_wire_exactly_and_the_limit_stays_inherited`(한도 상속 = 소유자가 `get_int_max_str_digits()==4300` 을 보고, 한도 밖 정수 값이 16진으로 정확히 건너옴). 어댑터 초안과 크레이트 사본은 동일하다.

### R8.3 공유 모듈 변경(추가·가시성만)과 영향

`utc.rs`(`parse_instant`·`InstantFailure`·`UtcInstant::shifted_by_micros` 추가) · `lite_metrics.rs`(`open_target`·`read_descriptor`·`OpenTarget` 를 `pub(crate)`) · `argv.rs`(`CLASSIFY_CELL` 표) · `lib.rs`·`Cargo.toml`(모듈·bin 3·test 2 등록; 의존성·`Cargo.lock` 불변, `--locked`). 영향 검사 = 전체 `cargo test --release`(기존 R1~R7 검사 포함) 226 통과.

### R8.4 보존한 원본 특이 동작(고치지 않음)

`measure_rc` `absent`/빈 문자열 = 부재(0 과 다름) · `--sweep-index ""` 는 셀 종결 모드로 떨어짐 · 빈 `--measure-rc` 처리 순서 · `float('nan')`/`True` 등 `!= 0` 동적 의미 · 경고(`WARN events 판독 실패`)는 쓰이는 순서대로 stderr, 이후 예외는 traceback(마지막 `Class: message` 줄만) · `bench_mode.json` 최종 모드 0600(`mkstemp`) · 4300자리 초과 정수·재귀 한도(995단계)는 `json` 한도 그대로 · K-mirror 는 `fetch_sub_docs.sh` 부재(서브 배달 트리 — 메인 전용 스크립트)면 원본처럼 `[SKIP]` 줄을 찍는다(메인 체크아웃에서 SKIP 이면 scripts 경로가 틀린 것이고 F00 도 실패한다).

### R8.5 실제 검사(작업 폴더, 로그 `logs/`, 도구체인 = 관리 Rust 1.99.1 `rust_bootstrap.sh exec`, offline·locked, 외부 target)

| 검사 | 결과 |
|---|---|
| `cargo fmt -- --check` | PASS (`fmt-check-final`) |
| `cargo clippy --all-targets -- -D warnings` | PASS (`clippy-final`; 첫 시도 FAIL 3건·1건 교정 `clippy-attempt1/2` 보존) |
| `cargo build --release --bins` | PASS (`build-release-final`) |
| `cargo test --release` 전체 | PASS 53 suite · 226 테스트 · 실패 0 (`test-all-release-final`; 변경 전 첫 전체 `test-all-release-attempt1` 도 PASS) |
| 고정 원본 API 121+45(+레코드 8 트리)·CLI 17+3 (자체 재생 `check_r8_own.py`, 루트 기준은 읽기 전용) | PASS 121/121 · 45/45 · 8/8 · 17/17 · 3/3 (`own-replay-release-final`) |
| 원본 `--self-test` 대 native `--self-test` | exit 0 · 83 PASS · 전사 바이트 동일 · SKIP 0 (`selftest-release-final.*`); 소유자 없는 scripts 경로는 FAIL 7 + SKIP·exit 1 로 침묵하지 않음(`selftest-negative-empty-owners.*`) |
| 역할 전송 원본 2행(root 검토 기록) | 초기 FAIL 2 보존 → 수정 후 PASS 2/2 |
| 표적 차분(자체 증거, 기준 변경 아님): 비정상형·값 API 1462행, CLI 155건(argv 약어·허용오차·시각·파일 형태·링크·권한·glob·역할 계획), 도움말 144(폭 18×8) | 불일치 0 · 파일 시스템 전/후 동일 (`odd-run-final`, `cli-odd-final`) |
| 성능 | NOT_RUN (`classify_cell_bench` 스모크만: 20행 2×3 실행, 수치 주장 없음) |

### R8.6 알려진 한계·미연결(root)

(1) 런처 `classify_cell.sh`/`classify_cell_api.sh`·Python 브리지·`build_rust.sh` 9번째 검사·소비자 15곳 배선·정본 어댑터 적용은 root 단계다(초안만 작업 폴더 `drafts/`). (2) **JSON 봉투 한도**: API 행·`arguments_json` 에 4300자리 초과 십진 정수를 실으면 전송 디코더가 원본 `json.loads` 처럼 거부해 그런 값을 API 인자로는 검증하지 못했다. 이것은 소유자 YAML 파서의 정수 한도와 별개다: 매니페스트 **내용**의 큰 정수는 JSON 을 거치지 않고(경로만 전송) 소유자가 상속 한도로 처리한다(R8.2 교정 2). (3) `datetime` 이 아닌 값이 `datetime` 자리에 오는 경우 오류 문구는 기록한 Python 연산 기준이며 두 인자가 동시에 틀린 순서는 대표 사례만 대조했다. (4) 임의 폭 도움말은 COLUMNS 1..700 캡처(468 이상 동일)다. (5) 소유자 조회 지연(어댑터·`node_identity.sh` 프로세스)은 OPEN_S3. (6) 어떤 수치도 서빙·LLM·통계적 우월성 주장이 아니다.

## R8 순수 계산 경로 성능 교정 (후보 · 정본 적용 전)

배경: root 의 사전 선언 경계 `pure 127 값 × 200 × 11` 교차 측정에서 Python 1526.8 ns / Rust 1799.1 ns (Rust 17.8% 느림, 0.8487×)였다(`r8-benchmark-results-v3`, 첫 측정은 보존). 이 절은 **순수 계산 경로만** 고친다. 전체 IO 22·CLI `bench-multi` 의 소유자 질의 비용(OPEN_S3)은 건드리지 않았고 숨기지도 않았다.

**관찰(자체, 선택 근거).** `classify_cell_bench` 를 id 별로 돌려 연산별 비용을 쟀다: 교정 전 `classify`(86행) 102.5 µs/패스(행당 ≈1.2 µs, Python ≈0.43 µs 로 가장 큰 격차), `classify_bench_mode`(22행) 135 µs, `events_scan_summary` 4.6 µs. 원인은 호출 경로의 일반 비용이었다: (1) 바인딩된 인자마다 값 전체 복제(`Bound::get`; `kill_hits` 목록 등), (2) `classify` 의 목록 복제 순회, (3) 고정 문구·사전 키를 호출마다 `str → 코드포인트` 변환·할당(`Text::lit`·`PyDict::set(PyStr::new)`), (4) 키마다 선형 중복 검사하는 사전 조립, (5) `classify_bench_mode` 안의 불필요한 `clone`(선언·`stop`·`hits`).

**변경(정확한 델타, 동작 불변).**

| 파일 | 변경 |
|---|---|
| `src/repeat_api_cli.rs` | `Bound::get_ref`·`get_or_ref` 추가(읽기만 하는 연산이 인자를 빌림; 기존 `get` 불변) |
| `src/pydyn.rs` | `Items`·`iterate_borrowed`(목록/튜플 순회를 복제 없이), `Text::fixed`(원본 고정 문구를 복사로 덧붙임), `Text::val` 이 문자열·정수를 중간 문자열 없이 덧붙임 |
| `src/classify_cell.rs` | `classify` 재작성(빌린 순회·`first_kill` 참조·`cell_result` = 원본 키 순서의 5항목 사전, 고정 문구는 `lit!` = 소스 상수당 한 번 변환되는 기존 관용), `BenchModeFields::into_record` 사전 조립을 `from_distinct_entries` 로, `classify_bench_mode` 의 복제 제거(선언·`stop`·`hits`·`tool`)와 고정 문구 `lit!`, `miss_source`·`correlation_value` 상수화 |
| `src/classify_cell_api_cli.rs` | `classify`·`classify_bench_mode`·`events_scan_summary`·`bench_mode_kind`·`declared_lite_record` 가 인자를 `get_ref` 로 빌림(바인딩·디스패치 작업 자체는 그대로) |

**하지 않은 것.** 입출력/결과 캐시, 특정 행·ID 를 아는 코드, 입력 좁힘, 타이머 밖 이동, 벤치 전용 경로, 소유자(Python) 규칙 복제 — 없음. 결과 값·오류 종류·메시지·키 순서·부작용은 그대로이며 `lit!` 상수는 소스에 적힌 고정 텍스트일 뿐 입력이나 결과의 캐시가 아니다.

**자체 측정(root 최종 성능 아님; 같은 준비 입력, 같은 127값×200, 교차 실행).** 교정 후 Rust 중앙값 1069.8 ns/값(9회 1061.7~1079.5) vs 같은 환경 Python 1521.2 ns/값 → 1.42× (root v3 의 17.8% 열세와 반대). 연산별(교정 후, µs/패스): `classify` 34.1(이전 102.5) · `classify_bench_mode` 91.5(135) · `events_scan_summary` 1.6(4.6) · `declared_lite_record` 1.1 · `bench_mode_kind` 0.8. 교정 전 바이너리는 같은 방식에서 ≈1800 ns 였다(`logs/perf-own-pure-before.txt`). 단일 1회·짧은 실행에서 가끔 한 번 더 느린 값이 섞인다(CPU 상태; 위 9회에 해당 값은 없었음) — root 의 11회 중앙값 측정이 기준이다. 첫 실행 산출물: `perf/classify_cell_bench.before`(교정 전 실행파일), `perf/src-before-perf-snapshot`(교정 전 소스).

**검증(교정 후 소스).** `cargo fmt -- --check` PASS · `clippy --all-targets -D warnings` PASS · `build --release --bins` PASS · `cargo test --release` 전체 53 suite · 226 테스트 · 실패 0 · 고정 원본 API 121+45(+레코드 8)·CLI 17+3 PASS · 역할 소유자 원본 3행(미사용 키·순환·5000자리 값) PASS · `--self-test` 83 PASS 전사 동일 · 표적 차분 API 1462·CLI 155 불일치 0.

**한계.** 전체 IO 22 와 `bench-multi` 의 열세는 소유자 질의(어댑터·`node_identity.sh` 프로세스) 비용이라 이 교정 범위 밖이며 OPEN_S3 로 남는다(소유자 이행 뒤 같은 입력으로 재측정). 어떤 수치도 서빙·LLM·통계적 우월성 주장이 아니다.

## R8 성능 측정기(`classify_cell_bench`) 관측 검증 보강 (후보 · 정본 적용 전)

배경: 독립 소스 검토 F1 — 이전 측정기는 `evaluate_operation` 의 `Result`/`Diagnostics` 를 검사 없이 버려, 시간이 잰 호출이 성공했는지·기대한 값을 돌려줬는지 연결이 없었다. 이는 **관측기 공백**이며 v3 의 실제 호출이 실패했다는 증거가 아니다. v3 원시 파일은 그대로 보존된다. 이 절 이전의 모든 기록(R8.1~R8.6 · 성능 교정)은 그대로다. 변경 전 측정기 소스는 작업 폴더 `perf/classify_cell_bench.rs.before-observer` 에 보존했다.

변경(측정기 한 파일 + 점검): `src/bin/classify_cell_bench.rs` 만. 생산 코드는 `expected_json` 을 모른다.

* 행 `{id, operation, arguments[, expected_json]}`. `expected_json` 은 문자열(`json.dumps(value, ensure_ascii=True, separators=(',', ':'))`)이며 문자열이 아니면(숫자·null·객체 등) 행 준비에서 거부한다.
* **시간 밖의 같은 한 번의 워밍 호출**(추가 호출 없음)이 `Ok` 이고 stderr 진단이 비어야 하며, `expected_json` 이 있으면 반환 **전체 값**을 같은 컴팩트 ASCII JSON 으로 직렬화해 정확히 비교한다(없으면 `Ok`·무진단만 요구). 불일치·`Err`·진단은 종료 코드 1, 타이밍 JSON 없음.
* **시간 안의 호출**은 여전히 디스패치·인자 바인딩·함수 본문·파일/소유자·전체 반환값의 `black_box`·해제를 모두 포함한다. 각 호출은 `Err` 이거나 진단이 있으면 "깨끗하지 않음"을 세고(플래그 두 개 읽기뿐), 하나라도 있으면 프로세스가 종료 코드 1 로 실패하며 **성공 타이밍 JSON 을 내지 않는다**.
* 문서 정정: 이전 머리말의 "바인딩된 인자" 표현은 부정확했다 — 바인딩·디스패치(`Signature::bind`)는 타이머 **안**이다. 소유자 질의 비용은 현재 런타임 비용에 **포함**되며 OPEN_S3 이다.

점검(`checks/classify_cell_bench.rs` + 측정기 단위 시험): 원본 고정 값 10행의 전체 `expected_json` 수용(`data/classify_cell/bench_expected.jsonl`), 행마다 한 글자 바꾼 기대값 거부(무타이밍), 잘못된 `expected_json` 타입 5종 거부, 알 수 없는 연산·인자 누락 거부, `Ok` 이지만 진단을 쓰는 호출(`_read_events` 없는 경로) 거부, 기대값 없는 깨끗한 행 수용; 단위 시험은 세 번째 호출에서만 실패하는 주입 호출이 전체 실행을 실패시키고(모든 timed 호출은 그대로 수행) 타이밍을 돌려주지 않음과, 실제 호출이 알 수 없는 연산을 "깨끗하지 않음"으로 센다는 것을 확인한다. 일회성 확인: 사전 선언 pure 127행 전부의 원본 기대값(컴팩트 JSON)이 워밍 검증을 통과(`logs`의 인자 파일 `perf/pure-expected-calls.jsonl`, 1×1 실행, 시간 주장 아님). 이 측정기 보강은 생산 경로를 바꾸지 않는다(검사 후 자체 확인 ≈1058~1062 ns/값(3회, `logs/perf-own-pure-after-observer.txt`), root 의 11회 중앙값이 기준).


## R8 root integration and measured boundaries

CONNECTED_CORE_VERIFIED_DEPENDENCY_LATENCY_OPEN. The classifier is connected through installed Bash launchers and a transport-only Python bridge to the actual campaign, sweep, reporting, hint and evidence callers. Original Python reference files remain available. Runtime launchers fail explicitly when the binary is missing; they neither build nor fall back to Python classification.

The final canonical managed source build ran nine real self-tests, including the original classifier 83 scenarios. fmt, release Cargo 235 tests across 54 suites plus the separate 22 descriptor scenarios, Clippy, fixed external 166 API / 20 CLI / 3 owner transport boundaries and all 128 distribution checks passed. A fresh space-containing path built from 462 Git-index source files (429 Rust plus 33 owner dependencies), without copied target/cache/credentials, passed build and registered classifier/observer/package checks. Existing managed toolchain/cache reuse was explicit. Existing executable modes were preserved.

Independent source review found that the initial timing helper dropped errors/diagnostics and did not connect expected values. This does not establish an actual error in the old run. Raw initial observations remain preserved with that limitation. The corrected generic helper checks each original full value on its existing single warm call and refuses timings on any timed error/diagnostic. Independent review closed that gap using source, original-data metadata and existing control logs; it did not rerun workloads. Production computation never reads expected values.

After quality, the same 127 values x 200 iterations x 11 alternating rounds gave Python 1521.187402 ns/value and Rust 1072.589803 ns/value (1.418238x). Dispatch, binding, full production result creation and drop remain timed. The actual six Bash CLI cases gave round-mean median 34.311961 / 12.794162 ms (2.681845x). Every CLI invocation checked full output, exit and after-tree. These are local observations, not serving or statistical claims.

Regressions stay OPEN_S3: actual owner/file/atomic-write 22 calls gave 2.459533 / 5.813904 ms (0.423043x); bench-multi CLI gave 43.630557 / 55.462552 ms (0.786667x). The pending Python node-role and identity owner costs are included. Repeat the identical 22-call and six-CLI boundaries after the real owner migration, without copied decision rules or caches. Whole-project Rust, harness, profile and real Long E2E acceptance remain incomplete.

## R9 보고·인증서·지도·명명 이행 (후보 · 정본 적용 전)

상태: **COMPLETE (이 변환 범위의 후보)** — 전체 과업·여섯 프로파일·실제 Long E2E 의 완료가 아니다. 대상은 원본 다섯 파일 전부다:
`render_report.py`(672) · `render_sweep_map.py`(394) · `render_max_report.py`(123) · `publish_benchmark_record.py`(434) · 벤치 `doc_naming.py`(305).
원본(기준 `831068cb`)은 이행 대조 기준으로 보존하며 런타임 대체 경로는 없다. 산문 명명 TypeScript 소유자는 범위 밖이다.

### 인터페이스 델타

| 산출 | 내용 |
|---|---|
| CLI 5 | `render_report` · `render_sweep_map`(+`--self-test` R1~R18) · `render_max_report` · `publish_benchmark_record` — 원본 argparse(prog 이름·약어·도움말/사용법 폭 1..700 캡처·오류 문구)·출력·종료코드(0/2/3/1)·실패 시점을 그대로 재현. `bench_doc_naming` — 원본 `doc_naming` 의 인자 없는 자체검사를 **어떤 인자가 와도** 실행(원본 동작). |
| 라이브러리 | `bench_naming`(명명 소유자) · `render_report` · `render_sweep_map` · `render_max_report` · `publish_record` · 공용 `report_support`(`na`·`fnum`·`_cell`·`scalar`·읽기/로드/키 조회)·`pyformat`(Python `str % float`)·`report_cli`(실행 결과 수집·종료 규약)·`reporting_cli`·`reporting_selftest`. |
| 범용 API | `bench_reporting_api evaluate-jsonl` — 행 `{id, operation, arguments}`; `operation` 은 `<owner>.<function>`(예 `render_report.build_md`, `doc_naming.bench_filename`) 또는 맨 함수명 + `owner` 필드. 결과 `{id,kind,value}` / `{id,kind:exception,type,message}` + (표준오류를 썼을 때) `stderr`. 튜플은 JSON 배열, 누락·미지·타입 오류는 Python 호출 오류와 같은 TypeError. 소유자: `render_report` `render_sweep_map` `render_max_report` `publish_benchmark_record` `doc_naming`. `arguments_json`/`result_json` 옵션은 다른 API 와 같다. |
| 시간 측정 | `bench_reporting_bench ROWS.jsonl ROUNDS ITERATIONS [ID...]` — 같은 연산 디스패치, 사례 지식 없음; 준비(행 파싱)는 밖, 디스패치·바인딩·본문·파일·전체 값 생성/해제는 안; 시간 밖 워밍 1회가 `Ok`·무진단·`expected_json`(컴팩트 ASCII) 전체 일치를 요구하고 시간 안의 `Err`/진단은 종료 1 · 타이밍 없음. |
| 공유 런타임 편집(가산·가시성) | `pyvalue.rs` `dumps_unicode`(= `json.dumps(ensure_ascii=False)`) 추가 · `pyjson.rs` `translate_newlines` 와 `lite_metrics_cli.rs` `unencodable_chunk` 를 `pub(crate)` · `argv.rs` 에 4개 `Spec` + 인덱스 모듈 · `lib.rs` 모듈 등록 · `Cargo.toml` bin 7 + `[[test]]` 3 · 생성 데이터 `help_layouts_*`(4) · `unicode_digit_tables`(`str.isdigit` 용 보조 표). 의존성 추가 없음(기존 lock 그대로). |

### 원본 → Rust 대응

`doc_naming`: `kst_tokens` `gpu_key` `measurement_ts` `scan_bench_dir` `bench_filename` + 예외 `NamingCollisionExhausted`/`NamingSourceUnreadable`(클래스명이 곧 정체성; 브리지가 한 번만 정의해 `.claude/policies/runtime/doc_naming.py` 가 같은 객체를 재수출) · `_self_test` → `reporting_selftest::run_naming_self_test`(임시 디렉터리를 원본처럼 **남긴다**).
`render_report`: `load` `na` `fnum` `repo_root` `_bench_mode_view` `_repetition_section` `_cell` `measurement_config_section` `full_measurement_config` `manifest_path_labels` `_label_path` `build_md` `_read_text` `_grep_yaml` `_grep_env` `lite_identity`(+`LiteIdentityError`) `lite_measurement_config` `build_lite_md` `main_lite`/`main` → `reporting_cli::run_render_report`.
`render_sweep_map`: `assert_no_ranking` `build` `render_markdown` `_na` `_repetition_line` `RenderError` `_self_test`(R1~R18) `main`.
`render_max_report`: `load` `na` `repo_root` `build_md` `main`. `publish_benchmark_record`: `load` `repo_root` `scalar` `build_yaml` `build_lite_yaml` `main_lite_raw` `_gamma_lite_certificate` `_full_definition_unmet` `_register_campaign_evidence` `main`.
분류기·lite 지표는 이미 이행된 **프로세스 안** 소유자(`classify_cell`·`lite_metrics`)를 직접 호출한다(서브프로세스 계산 없음).

### 보존한 특이 동작

* 파일 읽기: 텍스트는 보편 개행 번역, `_read_text` 는 `errors="replace"`, 스캔은 `errors="ignore"`, 로드는 엄격 UTF-8; 읽기 실패 문구는 원본(`[Errno 2] No such file or directory: '경로'`, JSON·UnicodeDecodeError 문구)과 같고 `load` 는 `[<프로그램>] ERROR <라벨>: <사유>` 후 `SystemExit(2)`.
* 명명: 슬롯은 종류 무관 합집합, 시각을 못 읽는 점유자는 덮어쓰기도 접미도 하지 않고 거부, 측정 시각은 본문의 `measured_utc|generated_utc|생성일` 뒤 첫 UTC 초; 접미 슬롯이 다른 측정이면 `NamingCollisionExhausted`. `existing` 가 dict 가 아니면 원본의 TypeError/AttributeError. 호출자가 잡지 않은 명명 예외의 추적 줄은 원본처럼 `doc_naming.` 접두를 가진다.
* 보고서: 값 없음 = `N/A`(`None`·`""`·`NA`·`N/A`, 공백·대소문자 무시), 표 셀의 `|` 이스케이프·공백 접기, `fnum` 의 임의 `fmt` 는 Python `%` 서식(오류 TypeError/ValueError 는 `N/A`, OverflowError·ZeroDivisionError 는 전파), 출처 필드와 기재·판정 구분, 관례 위치의 `bench_mode` 기록 부재는 `미확정 — 사유` 로 발행하고 **명시한** 경로가 쓸 수 없으면 종료 2.
* 지도: 순위 키 폐쇄 목록(소문자 비교, 임의 깊이)·표 머리 순위 열 금지·미완 지도 발행·자기 산출물 거부(종료 3)·출력 파일은 본문을 렌더하기 **전에** 만들어진다(렌더 실패 시 빈 파일이 남는다).
* 인증서: 평평한 YAML 직접 방출(스칼라 인용 규칙 포함), 강등 lite(γ)·전용 lite(β)·full 순서와 각 미발행 사유·종료 0/3, **인증서 쓰기 → 캠페인 증거 등재** 순서, 등재 소유자(`campaign_init.py`)는 설치된 저장소 경로에서 **실제 CLI 로** 호출(`PUBLISH_BENCHMARK_PYTHON`, 기본 `python3`), 부재는 경고 후 계속, 실패는 stderr 경고, 규칙 복제·캐시 없음.
* 처리하지 않은 예외는 추적 머리 표지 + 마지막 `Class: message` 와 종료 1(파이썬 스택 프레임은 재현하지 않음).

### 알려진 한계·미연결(root)

(1) 런처·`build_rust.sh` 검사 등록·소비자 배선·정본 적용은 root 단계다(초안: 공통 전송 브리지 `reporting_native.py`, API 런처, CLI 런처 템플릿 — 작업 폴더 `drafts/`). (2) 캠페인 소유자는 아직 Python 이다: `PUBLISH_BENCHMARK_PYTHON`(없으면 `python3`)이 필요하고 그 호출 비용은 인증서 쓰기 경계에 **포함**된다. (3) `repo_root` 는 `//x` 같은 자기 부모 경로에서 원본이 영원히 도는 것과 달리 작업 디렉터리를 돌려준다. (4) `--out-json` 의 `RecursionError`/정수 길이 오류는 원본이 파일에 일부를 쓴 뒤 실패하는 구간을 쓰지 않고 실패한다(이미 이행한 `lite_metrics` 와 같은 한계). (5) `assert_no_ranking` 의 파이썬 재귀 한도(RecursionError)는 재현하지 않는다. (6) 도움말 폭은 COLUMNS 1..700 캡처에서 상수 구간까지(render_report 233 · sweep_map 127 · max 87 · publish 162 이상 동일). (7) 보편 소문자·`isdigit` 는 Rust 표준 유니코드와 CPython 3.12 UCD 15.0 의 버전 차이만큼 극히 드문 문자에서 갈릴 수 있다.

### 검사

| 검사 | 결과 |
|---|---|
| `cargo fmt -- --check` | PASS |
| `cargo clippy --all-targets -- -D warnings` | PASS |
| `cargo build --release --bins` · `cargo test --release --no-fail-fast` | PASS (64 suite · 243 테스트 · 실패 0; R1~R8 suite 포함 영향 확인) |
| 고정 원본 API 203(순수) + 129(명명·lite 문서) | PASS 332/332 (값·예외 클래스·메시지·`stderr`) |
| 고정 원본 파일 IO 39(읽기·식별 우선순위·스캔·전/후 트리 전체) | PASS 39/39 |
| 고정 원본 CLI 83 + 8 (자체검사 R1~R18·명명 자체검사·인자 무시·실제 소유자 3) | PASS 91/91 (종료·stdout·stderr(추적 투영)·전체 트리) |
| 이식한 Cargo 점검 | `reporting_api_cases` 3 · `reporting_io_cases` 1 · `reporting_cli_cases` 1 + 측정기 단위 3 |
| 표적 차분(자체 증거, 기준 변경 아님): 값 변이 9000행 · 서식/명명/달력/유니코드/키 조회 20000행 · 실제 파일 식별·라벨·읽기·스캔 6000행 | 불일치 0 (첫 변이 실행의 불일치 42 = 키 조회 잘못된 타입 문구, 수정 후 0; 첫 실패 로그 보존) |
| 성능(자체 구현 증거, root 최종 아님) | 고정 id 36 보고/지도/인증서 값 × 200 × 11: Rust 4.6 µs / Python 8.4 µs 값당 (≈1.8×), 명명 82 값: 0.41 / 1.92 µs (≈4.7×). 개별 사례에 Python 보다 느린 것이 남아 있음(주로 지도 `build`·작은 `build_md`) — root 의 11회 측정이 기준 |

### R9 정정 (독립 검토·root 재귀 관측 반영 · 후보 · 정본 적용 전)

앞 절의 한계 (4)(5)는 **특성 기술이었지 면제가 아니었고**, 원본 먼저 관측한 실제 불일치로 확인되어 아래처럼 고쳤다. 앞 절의 기록은 이력으로 남긴다.

1. **JSON 지도 쓰기는 원본처럼 chunk 단위 스트리밍이다**(`src/json_stream.rs`). 이전 후보는 문서를 통째로 직렬화·인코딩한 뒤 한 번에 썼기 때문에 인코딩 불가 문자열(lone surrogate) 앞의 chunk 가 파일에 남지 않았고 오류 위치도 문서 전체 기준이었다. 이제 `json.encoder._make_iterencode` 의 chunk 경계(딕셔너리 키·`": "`·값·구분자는 각자 한 chunk, **리스트 항목은 앞 구분자와 한 chunk**)대로 만들고 chunk 마다 엄격 UTF-8 로 인코딩해 파일에 쓴다. 실패한 chunk 의 앞은 파일에 남고(닫을 때 flush) 오류 위치는 그 chunk 안의 위치다. 파일은 `open(path, "w")` 처럼 직접 열어 잘라내며(심볼릭 링크를 따라가 같은 inode), Markdown 을 렌더 **전에** 열고 닫은 뒤에 JSON 을 연다. 원자 교체·전체 사전 인코딩은 하지 않는다. 정수 자릿수 한도·직렬화 불가 값도 해당 chunk 에서 실패한다.
2. **파이썬 프레임 한도를 구조적으로 재현한다.** `assert_no_ranking` 은 값마다(스칼라 포함) 한 프레임을 쓰므로 `main()` 호출은 3번째 프레임에서 시작해 1000 을 넘으면 `RecursionError: maximum recursion depth exceeded` 로 **어떤 출력 파일도 건드리기 전에** 끝난다(라이브러리 API 호출은 2번째 프레임 기준). `json.dump` 는 컨테이너마다 제너레이터 프레임 하나이고 최외곽 컨테이너가 5번째이며, 한도를 넘는 컨테이너를 **만드는 시점**에 실패해 그 앞 chunk 는 이미 쓰였다. 실수(float)는 인코더의 파이썬 보조 함수 `floatstr` 를 거치므로 한 프레임 더 쓴다(문자열·정수는 내장 호출). 원본 관측: 깊이 992 전체 기록, 993/994 는 JSON 1 000 545 바이트 prefix 후 실패(Markdown 은 완성), 995 이상은 출력 불변. 이 규칙은 지도 CLI 의 `assert_no_ranking`/JSON 쓰기에만 걸리며 다른 모드는 바뀌지 않는다. `ranking` 위반만 종료 3 이고 `RecursionError` 는 처리되지 않은 예외(종료 1)다. Markdown 의 `json.dumps` 는 C 인코더(한도 약 10 000)라 이 경로에서는 앞선 프레임 검사가 먼저 걸린다.
3. **상수 내보내기**: `bench_reporting_api constants` 가 기존 `MEASUREMENT_CONFIG_TITLE`·`LITE_BENCH_TOOL`(및 lite 선언 문구 3개)도 낸다(새 리터럴 없음; 소비자 브리지는 이 명령에서 읽는다).
4. **`repo_root` 자기 부모 경로(`//x`)**: 원본은 `.git` 을 못 찾은 채 끝나지 않는 반복이고 후보는 작업 디렉터리를 돌려준다. 이것은 **동치가 아니라 의도한 종료 보장**이며(root 의 전체 신뢰성 범위 기록) 정상적으로 `.git` 을 찾거나 못 찾는 일반 경로의 동작은 원본과 같다. 끝나지 않는 반복을 재현하지 않는다.

검증: 원본 먼저 관측한 깊이 900~997 프로브 10개(`map.md`/`map.json` 바이트 포함)·독립 R9-I1 원본 1개(링크 대상 정확히 1000바이트, 위치 7, inode·링크 유지)·CLI 수준 표적 차분 300건(lone surrogate·깊은 dict/list/float 를 키·값·리스트 항목·doc 수준 필드에 배치)에서 바이트까지 같다. 회귀는 `checks/reporting_stream_cases.rs`(5)와 `json_stream` 단위 시험(3)이다.

### R9 정정 2 — 닫기(flush) 실패의 우선순위

지도 JSON 쓰기에서 덤프 실패와 닫기 실패가 함께 있을 때 이전 후보는 덤프 실패를 먼저 돌려줬다. 원본의 `with open(...)` 는 예외가 풀리는 동안 파일을 닫고, 그 닫기(flush) 실패가 앞선 덤프 오류를 **대체**한다. 그래서 인코딩 불가 chunk 앞부분이 버퍼에만 있고 닫을 때 `ENOSPC`(예: 가득 찬 장치)가 나면 최종 예외는 `UnicodeEncodeError` 가 아니라 `OSError: [Errno 28] No space left on device` 다. 이제 닫기는 덤프 성공·실패와 무관하게 호출되고 닫기 실패가 우선한다(`closed?` 후 `dumped?`). 추가 flush·타이머 이동·특수 경로는 없고, 일반 파일의 Unicode prefix·재귀 동작과 chunk 의미는 그대로다. 회귀: `checks/reporting_stream_cases.rs` 의 `a_close_failure_replaces_an_earlier_dump_failure_like_the_original`(surrogate 입력 + `/dev/full`)과 `a_plain_dump_to_a_full_disk_fails_with_the_same_close_error`(`/dev/full` 필요, 없으면 NOT_RUN 으로 실패).


## R9 정본 통합 판정

보고서·지도·envelope·인증서·벤치 명명 기능을 Rust CLI/단일 JSON transport로 실제 호출부에 연결했다.
정본 offline build11 자체검사·fmt·release253(65 suites)+별도FD22·Clippy, 외부371API/91CLI, 독립3개
및 close 오류2개와 recursion3개, 설치 bridge10개와 격리된 naming shim import가 PASS다.
Git-index 소스508파일(Rust463+의존45)만 새 공백 경로로 옮긴 offline build11과 관련 Cargo5targets도 PASS다.
첫 배포129는 sibling import/검사 사본 의존성 누락으로125/4 FAIL이었다. shim 로드경로와 lineage 자체검사
closure를 교정하고 유효125개 및 hint 내부211개는 재사용, 영향3개 명령과 lineage 하위검사를 재실행해129항목을 충족했다.
이는 두 번째 전체 배포 실행을 뜻하지 않는다. 앞선 실패 근거는 외부 수행기록에 남아 있다.

사전 고정11교차round의 전체 값/반환/해제 성능은 full render36×200: Python7723.997222ns/
Rust4724.098056ns=1.635021배, naming82×200:1840.642805ns/725.438659ns=2.537282배,
실제 파일I/O25:15779.76ns/11740.12ns=1.344088배다. 실제 Bash CLI7의 round평균 중앙값은
45.005490ms/21.932023ms=2.052045배이며 개별7개도 모두1배 이상이다. 각 호출 전체값·출력·종료·파일을 확인했다.
함수66측정 뒤 CLI 임시부모 생성 누락으로 실행기가 중단돼 그66개를 모두 보존·재사용하고 CLI는 새 작업폴더에서 처음 수행했다.
캐시·입력축소·측정경계 이동은 없으며, warm 전체반환 대조와 timed 오류 시 timing 거부를 유지한다.
이 수치는 현재 로컬 경계의 결과이며 LLM·서빙·통계적 일반화 성과가 아니다.

남은 소유자는 실제 Python campaign_init(S4)이며 그 비용도 CLI에 포함된다. S6 hint lineage가 쓰는
기존 Python benchmark 문법은 소비자 이행 단계까지 명시적으로 남아 있다. retained reference 전체제거나
Python 사용0건을 주장하지 않는다. repo_root의 self-parent 무한대기 종료는 의도된 신뢰성 차이로 따로 기록한다.
전체 S1~S7·하네스·6프로필·실제LongE2E 수용은 계속 미완료다.
